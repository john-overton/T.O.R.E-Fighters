//! M1e AI-6: the live hookup between `tore_sim::ai` and the running mission.
//!
//! This module is a bridge and nothing else. Every decision, every store debit
//! and every motion belongs to [`tore_sim::ai::mission`]; everything here only
//! moves data between that mission and the host's existing combat world:
//!
//! - one [`WorldObject`] snapshot per tick, built from the player's
//!   authoritative [`flight::State`] plus each AI actor's own flight state,
//! - each actor's pose mirrored into the matching `live::Target` so the
//!   existing renderer, sensors, missile collision and damage keep working off
//!   one world,
//! - damage mirrored back so a dead target kills its actor,
//! - each [`LaunchEvent`] realised as a real `live::Projectile` flown by the
//!   existing missile code, never by a second physics path,
//! - each missile turned into a [`ThreatReport`] delivered only to the aircraft
//!   it is aimed at (B47: no broadcast).
//!
//! The creator uses this bridge by default. `--fixture-wings` keeps the old
//! straight-flight launch and integration; ordinary free flight has no bridge.
//!
//! # Known limitations of the hookup
//!
//! The current combat bridge has these remaining limitations:
//!
//! - A `live::Projectile` can either hit the player (`incoming`) or hit a
//!   target, never both, and its weapon record is
//!   always read from the *player's* configuration. AI shots therefore fly the
//!   player's aircraft's missile, chosen by [`ai_station`].
//! - A radar-signature AI shot at another AI aircraft only keeps tracking while
//!   the *player's* sensors support that contact, because the unguided steering
//!   branch asks `state.sensors`. [`ai_station`] prefers an infrared store
//!   precisely so this does not bite in practice.

use std::collections::BTreeMap;

use tore_formats::aircraft::{Aircraft, AircraftId};
use tore_sim::{
    ai::{
        ScalarSpeed,
        controller::{
            Activity, ActorIdentity, BehaviorFamily, BehaviorProfile, MissionRole, ThreatReport,
        },
        launch::{self, WingLaunch},
        mission::{
            ActorSetup, AiActor, AiMission, LaunchEvent, WorldObject, simple_dispensers,
            simple_stations,
        },
        route,
        targeting::Side,
        threat::{SeekerClass, TimeOfDay},
        weapon_service::ActorId,
    },
    attitude::{Basis, Vector, unit},
    combat::{
        FallState, launch_speed,
        live::{self, MAX_PROJECTILES},
    },
    models::FlightModel,
    sensors::{self, Observable, Sensors},
};

use crate::{AppResult, flight, terrain::World};

/// `fitted`: the AI side numbers. `tore_sim::ai::targeting::Side` is an opaque
/// identity with no recovered numbering, so the host picks one. The player and
/// every friendly wing are side 1, every enemy wing is side 2. Rule: the
/// selector rejects same-side candidates and nothing else reads the value, so
/// any two distinct numbers are equivalent; these are chosen for readability.
pub const FRIENDLY_SIDE: Side = Side(1);
/// See [`FRIENDLY_SIDE`].
pub const ENEMY_SIDE: Side = Side(2);

/// The player's object id in every snapshot. `live::State` already reserves 0
/// for the player (`Projectile::target == Some(0)` is the player), and dummy
/// target ids start at 1, so actor ids and target ids are the same number.
pub const PLAYER_ID: u32 = 0;

/// Fitted Quick Mission placement, agent choice: use B43 echelon slots at
/// 512 ft spacing, level with the player. Friendly wing 1 occupies slots
/// behind the player. Wings 2 and 3 start 4096 ft behind and respectively
/// 4096 ft left and right. Enemy leaders start at the selected separation,
/// with wings 2 and 3 offset 4096 ft left and right, facing the player.
/// Original Quick Mission spawn geometry is unknown. These offsets make the
/// selected allies nearby instead of placing them in the enemy group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissionSpawn {
    pub offset: Vector,
    pub opposing: bool,
}

impl MissionSpawn {
    pub fn pose(self, position: Vector, basis: Basis) -> (Vector, Basis) {
        let position = std::array::from_fn(|i| {
            position[i] + basis.right[i] * self.offset[0] + basis.forward[i] * self.offset[2]
        });
        let heading = basis.angles()[0]
            + if self.opposing {
                std::f64::consts::PI
            } else {
                0.0
            };
        (position, Basis::new(heading, 0.0, 0.0))
    }
}

pub fn mission_spawns(wings: &[WingLaunch], separation_ft: f64) -> Vec<MissionSpawn> {
    use tore_sim::ai::wing::{Formation, formation_slot_point};
    wings
        .iter()
        .flat_map(|wing| {
            wing.members.iter().map(move |member| {
                let opposing = wing.wing.side.is_enemy();
                let slot = member.member + u8::from(!opposing && wing.wing.index == 0);
                let offset = if slot == 0 {
                    [0.0; 3]
                } else {
                    formation_slot_point(Formation::Echelon, slot, 512, 0)
                        .expect("validated Quick Mission member fits the formation table")
                };
                let lateral = match wing.wing.index {
                    1 => -4096.0,
                    2 => 4096.0,
                    _ => 0.0,
                };
                MissionSpawn {
                    offset: if opposing {
                        [lateral - offset[0], 0.0, separation_ft - offset[2]]
                    } else {
                        [
                            lateral + offset[0],
                            0.0,
                            offset[2] - if wing.wing.index == 0 { 0.0 } else { 4096.0 },
                        ]
                    },
                    opposing,
                }
            })
        })
        .collect()
}

/// `opinionated` (agent decision, 2026-09-17): AI projectiles take ids from
/// their own high range so they can never be confused with the player's shots,
/// whose ids are `live::State::shots`. Rule: the player fires far fewer than
/// sixteen million rounds in a session, so the two ranges cannot meet, and the
/// player's shot counter is left untouched by AI fire.
pub const AI_PROJECTILE_ID_BASE: u32 = 1 << 24;

/// `fitted`: the AI store fit. The Quick Mission screen carries no AI loadout
/// and `docs/spec/ai.md` does not specify one, so every AI fighter launches
/// with the reference fit `tore_sim::ai::mission::simple_stations` documents:
/// four guided air-to-air rounds and five hundred gun rounds. Rule: four
/// missiles is the smallest fit that lets B45's reattack rules be observed, and
/// the gun store exists so an actor out of missiles still has a weapon.
pub const AI_MISSILES: u32 = 4;
/// See [`AI_MISSILES`].
pub const AI_GUN_ROUNDS: u32 = 500;
/// See [`AI_MISSILES`]. Feet per second, the store speed the weapon service
/// uses for its own range and time-of-flight estimates.
pub const AI_STORE_SPEED: ScalarSpeed = ScalarSpeed(2000.0);
/// See [`AI_MISSILES`]: countermeasures per dispenser class.
pub const AI_DISPENSER_COUNT: u32 = 30;

/// `opinionated` (agent decision, 2026-09-17): the activity bar holds one line
/// for four seconds, so the bridge posts at most one AI line every this many
/// ticks. Rule: 240 ticks is two seconds at the fixed 120 Hz rate, which lets a
/// second line replace the first only after it has been readable for half its
/// life, instead of a new line every tick.
pub const MESSAGE_INTERVAL_TICKS: u64 = 240;

/// The message channel is opinionated too: only a change *into* one of these
/// activities is worth interrupting the player for. Formation, idle and pursuit
/// changes happen constantly and say nothing a player would act on.
fn worth_announcing(activity: Activity) -> bool {
    matches!(
        activity,
        Activity::Attacking | Activity::Defending | Activity::Breaking | Activity::Destroyed
    )
}

/// One AI aircraft's host-side identity, kept so the bridge can name it in a
/// message and find its target row without searching the launch payload again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    /// Actor id, which is also the `live::Target` id.
    pub id: u32,
    pub side: launch::Side,
    /// Wing number as the setup screen shows it, 1 through 3.
    pub wing_number: u8,
    /// Member number inside the wing, 1 based for display.
    pub member_number: u8,
    pub aircraft: AircraftId,
}

impl Slot {
    /// "Enemy 2-1", the wing and member a player would recognise.
    pub fn label(&self) -> String {
        let side = match self.side {
            launch::Side::Friendly => "Friendly",
            launch::Side::Enemy => "Enemy",
        };
        format!("{side} {}-{}", self.wing_number, self.member_number)
    }
}

/// The live AI bridge for one mission.
pub struct AiWings {
    mission: AiMission,
    slots: Vec<Slot>,
    /// Station on the *player's* configuration used to realise AI shots. See
    /// [`ai_station`].
    station: usize,
    /// Projectile ids already turned into threat reports.
    seen_projectiles: Vec<u32>,
    /// Projectile id to the actor that fired it, for B47 attribution. The
    /// player's shots are absent; the bridge remembers its own launchers.
    ai_shots: BTreeMap<u32, u32>,
    /// Last observed hit points per actor, for the damage mirror.
    last_hp: BTreeMap<u32, i32>,
    last_activity: BTreeMap<u32, Activity>,
    next_projectile_id: u32,
    last_message_tick: u64,
    /// Launch events that could not become a projectile, for honest reporting.
    pub dropped_launches: u32,
    /// Projectiles this bridge created.
    pub realised_launches: u32,
    /// The most recent B47 deliveries as (receiving actor, missile id). Only
    /// the aircraft a missile is aimed at ever appears here.
    threat_reports: Vec<(u32, u32)>,
    /// A line for `FlightUi::message`, taken by the host once.
    pending_message: Option<String>,
}

/// `fitted`: which of the player aircraft's stations an AI shot borrows.
///
/// A `live::Projectile` reads its weapon from `Configuration::stations`, which
/// is the player's aircraft, so an AI shot has to borrow one. Rule: prefer the
/// first infrared-signature store (`seeker.signature == 2`), because the
/// unguided steering branch only demands launcher illumination for radar
/// stores, so an infrared borrow tracks its target without the player's radar.
/// Failing that, take the first store with any seeker, and failing that station
/// zero. The known difference: AI aircraft all shoot the player's missile, not
/// the one their own record carries.
pub fn ai_station(config: &live::Configuration) -> usize {
    config
        .stations
        .iter()
        .position(|s| s.weapon.seeker.signature == 2)
        .or_else(|| {
            config
                .stations
                .iter()
                .position(|s| s.weapon.seeker.signature != 0)
        })
        .unwrap_or(0)
}

/// `fitted`: the per-actor decision seed.
///
/// Rule: `side * 1_000_000 + wing * 1_000 + member`, mixed with a fixed salt so
/// two actors never share a stream and the same setup screen always produces
/// the same run. The spec gives draw thresholds, never sequences, so any
/// injective derivation satisfies it; this one is readable in a probe dump.
pub fn actor_seed(side: launch::Side, wing: u8, member: u8) -> u64 {
    const SALT: u64 = 0x5749_4E47_5F41_4900; // "WING_AI\0"
    let side = u64::from(side.is_enemy());
    SALT ^ (side * 1_000_000 + u64::from(wing) * 1_000 + u64::from(member))
}

fn side_of(side: launch::Side) -> Side {
    if side.is_enemy() {
        ENEMY_SIDE
    } else {
        FRIENDLY_SIDE
    }
}

/// The widest maximum airspeed any loaded envelope permits at this altitude.
/// `fitted`: `WorldObject::maximum_speed` has no recovered producer, so the
/// bridge reads the same envelope block the flight model and the AI's own
/// `speed_limits` read, and falls back to the AI module's documented fixture
/// value when no envelope covers the altitude.
fn maximum_speed(state: &flight::State) -> ScalarSpeed {
    let envelopes = &state.model().configuration().aerodynamics.envelopes;
    let altitude = state.position[1];
    let best = envelopes
        .iter()
        .filter_map(|e| e.speeds(altitude))
        .map(|(_, maximum)| maximum)
        .fold(f64::NEG_INFINITY, f64::max);
    if best.is_finite() {
        ScalarSpeed(best)
    } else {
        ScalarSpeed(tore_sim::ai::mission::FALLBACK_MAXIMUM_FPS)
    }
}

impl AiWings {
    /// Build the bridge from a resolved launch payload and the targets the
    /// existing spawner has already placed.
    ///
    /// `targets` must be `combat.state.targets` immediately after
    /// `Combat::reset`, whose rows are the flattened wing members in payload
    /// order with id `index + 1`. Nothing here recomputes a spawn position: the
    /// AI aircraft start exactly where the fixtures would have started.
    pub fn build(
        wings: &[WingLaunch],
        targets: &[live::Target],
        config: &live::Configuration,
        resources: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<Self> {
        Self::build_with(wings, targets, ai_station(config), |id| {
            let bytes = resources
                .get(id.pt())
                .ok_or_else(|| format!("aircraft cache missing {}", id.pt()))?;
            let aircraft = Aircraft::parse(bytes)?;
            // A missing or unreviewed sensor record is not fatal: the actor
            // simply flies without its own sensors, which the AI documents as
            // the host-supplied permitted-target path.
            let found = sensors::SensorProfiles::from_source(&aircraft, |name| {
                resources
                    .get(name)
                    .cloned()
                    .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
            })
            .ok();
            Ok((aircraft, found))
        })
    }

    /// [`build`](Self::build) with the aircraft records supplied by the caller,
    /// so a test can build a mission from a synthetic profile and no media.
    pub fn build_with(
        wings: &[WingLaunch],
        targets: &[live::Target],
        station: usize,
        mut resolve: impl FnMut(AircraftId) -> AppResult<(Aircraft, Option<sensors::SensorProfiles>)>,
    ) -> AppResult<Self> {
        let mut mission = AiMission::new();
        // Opinionated host setup: level delta formations using B43's
        // alternating trailing slots, 512 ft spacing, independently per wing.
        mission.set_spacing(512, 0);
        mission.set_external_leader(FRIENDLY_SIDE, 0, PLAYER_ID);
        let mut slots = Vec::new();
        let mut profiles: Vec<(AircraftId, Aircraft, Option<sensors::SensorProfiles>)> = Vec::new();
        let mut index = 0usize;
        for wing in wings {
            if wing.is_empty() {
                continue;
            }
            if !profiles.iter().any(|(id, _, _)| *id == wing.aircraft) {
                let (aircraft, found) = resolve(wing.aircraft)?;
                profiles.push((wing.aircraft, aircraft, found));
            }
            let (_, aircraft, found) = profiles
                .iter()
                .find(|(id, _, _)| *id == wing.aircraft)
                .expect("just inserted");
            for member in &wing.members {
                // The player's wing reserves member zero for the human leader.
                let member_index = member.member
                    + u8::from(wing.wing.side == launch::Side::Friendly && wing.wing.index == 0);
                let target = targets.get(index).ok_or_else(|| {
                    format!(
                        "AI wing member {index} has no spawned target; the fixture spawner and the launch payload disagree"
                    )
                })?;
                let mut state = flight::State::new(aircraft, target.position)?;
                let [yaw, pitch, bank] = target.basis.angles();
                state.yaw = yaw;
                state.pitch = pitch;
                state.bank = bank;
                // `opinionated` (agent decision, 2026-09-17): the AI aircraft
                // keeps the flight model's own start airspeed rather than the
                // 300 ft/s drift velocity `live::State::add_dummy` gives a
                // straight-flight fixture. Rule: 300 ft/s is below the stall
                // speed of every ported fighter, so an actor handed it would
                // spend its first seconds recovering instead of flying. The
                // spawn *position* and *attitude* are unchanged.
                state.velocity = Basis::new(yaw, pitch, bank)
                    .forward
                    .map(|v| v * state.speed);
                let sensors = found.clone().map(Sensors::new);
                let setup = ActorSetup {
                    identity: ActorIdentity {
                        actor: ActorId(target.id),
                        side: side_of(wing.wing.side),
                        wing: wing.wing.index,
                        member: member_index,
                        aircraft: wing.aircraft,
                        human_controlled: false,
                    },
                    profile: BehaviorProfile {
                        family: BehaviorFamily::FighterStrike,
                        role: MissionRole::AirToAir,
                    },
                    experience: member.experience,
                    seed: actor_seed(wing.wing.side, wing.wing.index, member.member),
                    flight: state,
                    sensors,
                    stations: simple_stations(AI_MISSILES, AI_GUN_ROUNDS, AI_STORE_SPEED),
                    dispensers: simple_dispensers(AI_DISPENSER_COUNT),
                    wing_slot: member_index.max(1),
                    // `fitted`: a Quick Mission assigns no airfield, so the
                    // spawn point stands in as the home airport. Rule: B48 only
                    // needs somewhere to fly home to when fuel runs low, and
                    // the spawn point is the one position the setup screen
                    // actually decided.
                    home_airport: Some(route::Position {
                        x: target.position[0],
                        z: target.position[2],
                    }),
                };
                mission.push(AiActor::new(setup).map_err(|e| e.to_string())?);
                slots.push(Slot {
                    id: target.id,
                    side: wing.wing.side,
                    wing_number: wing.wing.display_number(),
                    member_number: member_index + 1,
                    aircraft: wing.aircraft,
                });
                index += 1;
            }
        }
        Ok(Self {
            mission,
            slots,
            station,
            seen_projectiles: Vec::new(),
            ai_shots: BTreeMap::new(),
            last_hp: BTreeMap::new(),
            last_activity: BTreeMap::new(),
            next_projectile_id: AI_PROJECTILE_ID_BASE,
            last_message_tick: 0,
            dropped_launches: 0,
            realised_launches: 0,
            threat_reports: Vec::new(),
            pending_message: None,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    pub fn mission(&self) -> &AiMission {
        &self.mission
    }

    /// Recent B47 deliveries, newest last, as (receiving actor, missile id).
    pub fn threat_reports(&self) -> &[(u32, u32)] {
        &self.threat_reports
    }

    pub fn slot(&self, id: u32) -> Option<&Slot> {
        self.slots.iter().find(|s| s.id == id)
    }

    /// Take the pending activity line, if the rate limiter released one.
    pub fn take_message(&mut self) -> Option<String> {
        self.pending_message.take()
    }

    /// One 120 Hz tick of AI, run immediately after `Combat::step`.
    ///
    /// The order matters: combat has already flown the player, the projectiles
    /// and the straight-line target integration, so the bridge reads the damage
    /// combat just applied, decides, and then writes the authoritative AI pose
    /// into the targets. Next tick's straight-line integration therefore starts
    /// from the true AI pose and advances it by the true AI velocity, which is
    /// what the missile collision sweep needs.
    pub fn step(
        &mut self,
        state: &mut live::State,
        player: &flight::State,
        world: &World,
    ) -> AppResult<()> {
        let ground = |x: f64, z: f64| f64::from(world.height(x as f32, z as f32));
        let object = self.player_object(player, state.player_hp, state.configuration());
        let output = self.advance(object, &mut state.targets, &ground)?;
        let station = self.station.min(state.configuration().stations.len() - 1);
        let weapon = state.configuration().stations[station].weapon.clone();
        for event in &output.launches {
            self.realise(
                event,
                &mut state.projectiles,
                &weapon,
                station,
                player.position,
            );
        }
        let stations = state.configuration().stations.clone();
        self.report_threats(&state.projectiles, |index| {
            if stations[index].weapon.seeker.signature == 3 {
                SeekerClass::Radar
            } else {
                SeekerClass::Infrared
            }
        });
        Ok(())
    }

    /// The AI half of one tick, with the combat world reduced to its target
    /// rows. This is what a headless test drives: damage in, one world
    /// snapshot, one mission step, pose out, activity line.
    pub fn advance(
        &mut self,
        player: WorldObject,
        targets: &mut [live::Target],
        ground: &dyn Fn(f64, f64) -> f64,
    ) -> AppResult<tore_sim::ai::mission::MissionOutput> {
        self.mirror_damage_in(targets);
        let objects = self.snapshot(player, targets);
        // `fitted`: `TimeOfDay` is an opaque host clock the AI only orders
        // against a mission hold time, so the mission tick is used directly. It
        // is monotonic and deterministic, which is all the ordering needs.
        let now = TimeOfDay(self.mission.tick());
        let output = self
            .mission
            .step(&objects, ground, now)
            .map_err(|e| e.to_string())?;
        self.mirror_pose_out(targets);
        self.announce(&output.activities);
        Ok(output)
    }

    /// The player as the AI sees it: an ordinary object on the friendly side,
    /// never a special case in the decision path.
    pub fn player_object(
        &self,
        player: &flight::State,
        player_hp: i32,
        config: &live::Configuration,
    ) -> WorldObject {
        WorldObject {
            id: PLAYER_ID,
            // The player is on the friendly side so friendly AI never shoots
            // at the human and enemy AI always may.
            side: FRIENDLY_SIDE,
            position: player.position,
            velocity: player.velocity,
            heading_deg: player.yaw.to_degrees(),
            pitch_deg: player.pitch.to_degrees(),
            speed: ScalarSpeed(player.speed),
            maximum_speed: maximum_speed(player),
            is_aircraft: true,
            is_fighter: true,
            human_controlled: true,
            alive: !player.crashed && player_hp > 0,
            destroyed: player_hp <= 0,
            observable: Some(Observable {
                id: PLAYER_ID,
                position: player.position,
                velocity: player.velocity,
                basis: Basis::new(player.yaw, player.pitch, player.bank),
                configuration: sensors::Configuration::CLEAN,
                signature: config.sensors.signature,
                jammer: config.sensors.jammer.clone(),
                jammer_active: player.jammer && player.engine,
                radar_emitting: player.radar && player.engine,
                airborne: true,
                destroyed: player_hp <= 0,
            }),
        }
    }

    /// Damage and death flow from the combat world into the actors: an actor
    /// whose target row lost hit points is told it was hit, and one whose row
    /// reached zero stops flying.
    fn mirror_damage_in(&mut self, targets: &[live::Target]) {
        for slot in &self.slots {
            let Some(target) = targets.iter().find(|t| t.id == slot.id) else {
                continue;
            };
            let previous = *self.last_hp.entry(slot.id).or_insert(target.hp);
            if target.hp < previous
                && let Some(actor) = self.mission.actor_mut(slot.id)
            {
                actor.report_hit();
            }
            self.last_hp.insert(slot.id, target.hp);
            if target.hp <= 0 && self.mission.actor(slot.id).is_some_and(AiActor::alive) {
                if let Some(actor) = self.mission.actor_mut(slot.id) {
                    actor.set_alive(false);
                }
                for other in self.mission.actors_mut() {
                    other.report_removed(slot.id);
                }
            }
        }
    }

    /// One world snapshot: the player first, then every AI aircraft.
    fn snapshot(&self, player: WorldObject, targets: &[live::Target]) -> Vec<WorldObject> {
        let mut objects = Vec::with_capacity(self.slots.len() + 1);
        objects.push(player);
        for slot in &self.slots {
            let Some(actor) = self.mission.actor(slot.id) else {
                continue;
            };
            let f = actor.flight();
            let target = targets.iter().find(|t| t.id == slot.id);
            objects.push(WorldObject {
                id: slot.id,
                side: side_of(slot.side),
                position: f.position,
                velocity: f.velocity,
                heading_deg: f.yaw.to_degrees(),
                pitch_deg: f.pitch.to_degrees(),
                speed: ScalarSpeed(f.speed),
                maximum_speed: maximum_speed(f),
                is_aircraft: true,
                is_fighter: true,
                human_controlled: false,
                alive: actor.alive(),
                destroyed: target.is_some_and(|t| t.hp <= 0),
                observable: target.map(|t| Observable {
                    id: t.id,
                    position: f.position,
                    velocity: f.velocity,
                    basis: Basis::new(f.yaw, f.pitch, f.bank),
                    configuration: t.configuration,
                    signature: t.signature,
                    jammer: t.jammer.clone(),
                    jammer_active: t.jammer_active,
                    radar_emitting: f.radar,
                    airborne: t.airborne,
                    destroyed: t.hp <= 0,
                }),
            });
        }
        objects
    }

    /// The AI pose becomes the target pose, so the renderer, the player's
    /// sensors, the missile collision sweep and the damage model all see one
    /// world with one authority per aircraft.
    fn mirror_pose_out(&self, targets: &mut [live::Target]) {
        for slot in &self.slots {
            let Some(actor) = self.mission.actor(slot.id) else {
                continue;
            };
            let Some(target) = targets.iter_mut().find(|t| t.id == slot.id) else {
                continue;
            };
            let f = actor.flight();
            target.position = f.position;
            target.velocity = f.velocity;
            target.basis = Basis::new(f.yaw, f.pitch, f.bank);
            target.radar_emitting = f.radar;
        }
    }

    /// Turn one AI launch into a real projectile flown by the existing combat
    /// code. The ammunition was already debited inside the AI, so nothing here
    /// touches a store.
    fn realise(
        &mut self,
        event: &LaunchEvent,
        projectiles: &mut Vec<live::Projectile>,
        weapon: &tore_formats::weapons::Weapon,
        station: usize,
        player_position: Vector,
    ) {
        let Some(actor) = self.mission.actor(event.actor) else {
            self.dropped_launches += event.projectiles;
            return;
        };
        let origin = actor.flight().position;
        let aim = if event.target == PLAYER_ID {
            player_position
        } else if let Some(other) = self.mission.actor(event.target) {
            other.flight().position
        } else {
            self.dropped_launches += event.projectiles;
            return;
        };
        let direction = unit([aim[0] - origin[0], aim[1] - origin[1], aim[2] - origin[2]]);
        if direction.iter().any(|v| !v.is_finite()) {
            self.dropped_launches += event.projectiles;
            return;
        }
        let Ok(speed) = launch_speed(&weapon.movement, (actor.flight().speed * 256.) as i32) else {
            self.dropped_launches += event.projectiles;
            return;
        };
        // `fitted`: an AI shot uses the unguided steering branch of
        // `live::State::step`, never the spec guidance model. Rule: the
        // guidance model searches `state.targets` only and consults the
        // player's own sensors for support, so it can neither see the player
        // nor track without the player's radar. The unguided branch steers with
        // the store's own turn rates toward whatever the shot was aimed at, and
        // it is the same branch the game's `Incoming` fixture already uses.
        // `live::State` keeps its tick private, but the bridge steps exactly
        // once per `Combat::step`, so the mission tick is the same number the
        // combat step will use for this projectile's age.
        let launched = (self.mission.tick() / 30) as u16;
        let incoming = event.target == PLAYER_ID;
        for _ in 0..event.projectiles {
            if projectiles.len() >= MAX_PROJECTILES {
                self.dropped_launches += 1;
                continue;
            }
            let id = self.next_projectile_id;
            self.next_projectile_id = self.next_projectile_id.wrapping_add(1);
            projectiles.push(live::Projectile {
                id,
                // Attribute the round to the AI actor that fired it, so an
                // AI kill never credits the player's score.
                owner: event.actor,
                guidance: None,
                motion: None,
                guidance_ticks: None,
                age: 0,
                incoming,
                station,
                position: origin,
                previous: origin,
                direction,
                speed_f8: speed * 256,
                launched_t: launched,
                target: Some(event.target),
                fall: FallState::default(),
            });
            self.ai_shots.insert(id, event.actor);
            self.realised_launches += 1;
        }
    }

    /// B47: every missile in the world is reported once, to its target only.
    /// Wingmen are not told, and neither is anyone else: only the aircraft the
    /// shot is actually aimed at receives a report.
    fn report_threats(
        &mut self,
        projectiles: &[live::Projectile],
        seeker_of: impl Fn(usize) -> SeekerClass,
    ) {
        let mut reports: Vec<(u32, ThreatReport)> = Vec::new();
        for projectile in projectiles {
            if self.seen_projectiles.contains(&projectile.id) {
                continue;
            }
            self.seen_projectiles.push(projectile.id);
            let Some(target) = projectile.target else {
                continue;
            };
            if target == PLAYER_ID {
                continue;
            }
            let Some(slot) = self.slot(target) else {
                continue;
            };
            let Some(actor) = self.mission.actor(target) else {
                continue;
            };
            let seeker = seeker_of(projectile.station);
            let launcher_id = self
                .ai_shots
                .get(&projectile.id)
                .copied()
                .unwrap_or(PLAYER_ID);
            let launcher_side = if launcher_id == PLAYER_ID {
                Some(launch::Side::Friendly)
            } else {
                self.slot(launcher_id).map(|s| s.side)
            };
            let position = actor.flight().position;
            let d = [
                position[0] - projectile.position[0],
                position[1] - projectile.position[1],
                position[2] - projectile.position[2],
            ];
            reports.push((
                target,
                ThreatReport {
                    missile_id: projectile.id,
                    seeker,
                    launcher_id,
                    launcher_same_side: launcher_side
                        .is_some_and(|side| side_of(side) == side_of(slot.side)),
                    distance_at_launch_ft: (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt(),
                    launch_tick: self.mission.tick(),
                },
            ));
        }
        for (id, report) in reports {
            if let Some(actor) = self.mission.actor_mut(id) {
                self.threat_reports.push((id, report.missile_id));
                actor.report_threat(report);
            }
        }
        // Bounded history: this is a report channel, not a log.
        const KEEP: usize = 64;
        if self.threat_reports.len() > KEEP {
            let drop = self.threat_reports.len() - KEEP;
            self.threat_reports.drain(..drop);
        }
        // The seen list only has to outlive the projectiles themselves.
        if self.seen_projectiles.len() > MAX_PROJECTILES * 4 {
            let live: Vec<u32> = projectiles.iter().map(|p| p.id).collect();
            self.seen_projectiles.retain(|id| live.contains(id));
            self.ai_shots.retain(|id, _| live.contains(id));
        }
    }

    /// `opinionated` (agent decision, 2026-09-17): the single in-flight message
    /// bar carries AI activity changes. Only a change into an activity
    /// [`worth_announcing`] posts, and at most one line per
    /// [`MESSAGE_INTERVAL_TICKS`].
    fn announce(&mut self, activities: &[(u32, Activity)]) {
        let tick = self.mission.tick();
        for (id, activity) in activities {
            let changed = self.last_activity.insert(*id, *activity) != Some(*activity);
            if !changed || !worth_announcing(*activity) {
                continue;
            }
            if tick < self.last_message_tick + MESSAGE_INTERVAL_TICKS && self.last_message_tick > 0
            {
                continue;
            }
            let Some(slot) = self.slot(*id) else { continue };
            self.pending_message = Some(format!("{}: {}", slot.label(), activity.label()));
            self.last_message_tick = tick.max(1);
        }
    }

    /// A compact deterministic line per actor, for the headless probe.
    pub fn probe_lines(&self) -> Vec<String> {
        self.slots
            .iter()
            .filter_map(|slot| {
                let actor = self.mission.actor(slot.id)?;
                let f = actor.flight();
                Some(format!(
                    "actor={} {} {:?} activity={} alive={} rounds={} x={:.1} y={:.1} z={:.1} hdg={:.1}",
                    slot.id,
                    slot.label(),
                    slot.aircraft,
                    actor.activity().label(),
                    actor.alive(),
                    actor.rounds_remaining(),
                    f.position[0],
                    f.position[1],
                    f.position[2],
                    f.yaw.to_degrees(),
                ))
            })
            .collect()
    }

    /// Every actor's position, for determinism and motion tests.
    pub fn positions(&self) -> Vec<Vector> {
        self.slots
            .iter()
            .filter_map(|s| self.mission.actor(s.id).map(|a| a.flight().position))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::{
        ai::{
            Experience,
            experience::{EnemySkillOverride, ExperienceOrigin},
            launch::{WingId, WingSelection, resolve_wings},
        },
        combat::missiles::{TargetRole, seeker::Heat},
    };

    #[test]
    fn six_full_wings_have_separate_delta_formations_and_29_ai_members() {
        let selections: Vec<_> = [launch::Side::Friendly, launch::Side::Enemy]
            .into_iter()
            .flat_map(|side| {
                (0..3).map(move |index| WingSelection {
                    wing: WingId::new(side, index).unwrap(),
                    aircraft: AircraftId::F18,
                    count: if side == launch::Side::Friendly && index == 0 {
                        4
                    } else {
                        5
                    },
                    skill_level: i32::from(index),
                })
            })
            .collect();
        let wings = resolve_wings(&selections, None).unwrap();
        let spawns = mission_spawns(&wings, 10560.0);
        assert_eq!(spawns.len(), 29);
        assert_eq!(spawns[0].offset, [512., 0., -512.]);
        assert_eq!(spawns[1].offset, [-512., 0., -512.]);
        assert_eq!(spawns[2].offset, [1024., 0., -1024.]);
        assert_eq!(spawns[3].offset, [-1024., 0., -1024.]);
        assert_eq!(spawns[4].offset, [-4096., 0., -4096.]);
        assert_eq!(spawns[9].offset, [4096., 0., -4096.]);
        assert_eq!(spawns[14].offset, [0., 0., 10560.]);
        assert_eq!(spawns[15].offset, [-512., 0., 11072.]);
        assert_eq!(spawns[16].offset, [512., 0., 11072.]);
        assert_eq!(spawns[19].offset, [-4096., 0., 10560.]);
        assert_eq!(spawns[24].offset, [4096., 0., 10560.]);
        let basis = Basis::new(std::f64::consts::FRAC_PI_2, 0., 0.);
        let targets: Vec<_> = spawns
            .iter()
            .enumerate()
            .map(|(i, spawn)| {
                let (position, attitude) = spawn.pose([100., 20000., 300.], basis);
                let mut row = target(i as u32 + 1, position, attitude.angles()[0]);
                row.basis = attitude;
                assert_eq!(position[1], 20000.);
                assert_eq!(spawn.opposing, i >= 14);
                assert!((attitude.forward[0] - if i >= 14 { -1. } else { 1. }).abs() < 1e-9);
                row
            })
            .collect();
        for (i, a) in targets.iter().enumerate() {
            for b in &targets[i + 1..] {
                let distance = (a.position[0] - b.position[0]).hypot(a.position[2] - b.position[2]);
                assert!(
                    distance >= 512.,
                    "overlapping wing slots: {} and {}",
                    a.id,
                    b.id
                );
            }
        }
        let bridge = AiWings::build_with(&wings, &targets, 0, |_| Ok((aircraft(), None))).unwrap();
        assert_eq!(bridge.len(), 29);
        for id in 1..=4 {
            assert!(!bridge.mission.actor(id).unwrap().identity().is_leader());
            assert_eq!(
                bridge.mission.actor(id).unwrap().identity().member,
                id as u8
            );
        }
        for id in [5, 10, 15, 20, 25] {
            assert!(bridge.mission.actor(id).unwrap().identity().is_leader());
        }
    }

    /// The fixture spawner's own rule, copied so a test can predict where a
    /// straight-flight target ends up: `live::State::step` adds
    /// `velocity / 120` to every live target once per tick.
    const FIXTURE_TICK_RATE: f64 = 120.0;

    fn aircraft() -> Aircraft {
        crate::flight::animation_tests::profile()
    }

    fn flat(_x: f64, _z: f64) -> f64 {
        0.0
    }

    fn target(id: u32, position: Vector, yaw: f64) -> live::Target {
        let basis = Basis::new(yaw, 0., 0.);
        live::Target {
            role: TargetRole::Aircraft,
            heat: Heat::Engine {
                on: true,
                throttle: 0.7,
                afterburner: false,
            },
            radar_emitting: false,
            id,
            position,
            velocity: basis.forward.map(|v| v * 300.),
            basis,
            configuration: sensors::Configuration::CLEAN,
            signature: sensors::SignatureProfile::default(),
            jammer: None,
            jammer_active: false,
            airborne: true,
            radius: 28.,
            hp: 100,
            initial_hp: 100,
            fragment_offset: [0.; 3],
            fragment_released: false,
            category: 0,
        }
    }

    /// Two friendly aircraft in wing 2 and two enemy aircraft in wing 1, the
    /// same shape `--ai-probe-ticks` flies.
    fn payload(enemy_override: Option<EnemySkillOverride>) -> Vec<WingLaunch> {
        let selections = [
            (launch::Side::Friendly, 1u8, 2usize, 1i32),
            (launch::Side::Enemy, 0, 2, 3),
        ]
        .map(|(side, index, count, skill_level)| WingSelection {
            wing: WingId::new(side, index).unwrap(),
            aircraft: AircraftId::F18,
            count,
            skill_level,
        });
        resolve_wings(&selections, enemy_override).unwrap()
    }

    /// The four rows `Combat::reset` would have spawned: friendly pair facing
    /// the enemy pair, which face back.
    fn spawned() -> Vec<live::Target> {
        vec![
            target(1, [0., 20000., 0.], 0.),
            target(2, [1500., 20000., 0.], 0.),
            target(3, [0., 20000., 40000.], std::f64::consts::PI),
            target(4, [1500., 20000., 40000.], std::f64::consts::PI),
        ]
    }

    fn build(enemy_override: Option<EnemySkillOverride>) -> (AiWings, Vec<live::Target>) {
        let targets = spawned();
        let wings = AiWings::build_with(&payload(enemy_override), &targets, 0, |_| {
            Ok((aircraft(), None))
        })
        .unwrap();
        (wings, targets)
    }

    fn player_object(position: Vector) -> WorldObject {
        WorldObject {
            id: PLAYER_ID,
            side: FRIENDLY_SIDE,
            position,
            velocity: [0., 0., 800.],
            heading_deg: 0.,
            pitch_deg: 0.,
            speed: ScalarSpeed(800.),
            maximum_speed: ScalarSpeed(1600.),
            is_aircraft: true,
            is_fighter: true,
            human_controlled: true,
            alive: true,
            destroyed: false,
            observable: None,
        }
    }

    fn run(wings: &mut AiWings, targets: &mut [live::Target], ticks: usize) {
        for _ in 0..ticks {
            wings
                .advance(player_object([0., 20000., -5000.]), targets, &flat)
                .unwrap();
        }
    }

    #[test]
    fn the_setup_draft_becomes_actors_with_the_right_sides_wings_and_members() {
        let (wings, _) = build(None);
        assert_eq!(wings.len(), 4);
        let labels: Vec<String> = wings.slots().iter().map(Slot::label).collect();
        assert_eq!(
            labels,
            ["Friendly 2-1", "Friendly 2-2", "Enemy 1-1", "Enemy 1-2"]
        );
        let ids: Vec<u32> = wings.slots().iter().map(|s| s.id).collect();
        assert_eq!(ids, [1, 2, 3, 4], "actor ids must be the target ids");
        for slot in wings.slots() {
            let actor = wings.mission().actor(slot.id).unwrap();
            let identity = actor.identity();
            assert_eq!(identity.side, side_of(slot.side));
            assert_eq!(identity.wing + 1, slot.wing_number);
            assert_eq!(identity.member + 1, slot.member_number);
            assert!(!identity.human_controlled, "no AI actor may be the player");
        }
        // The player and every friendly wing share one side, so friendly AI
        // cannot select the human as a target.
        assert_eq!(side_of(launch::Side::Friendly), FRIENDLY_SIDE);
        assert_ne!(side_of(launch::Side::Enemy), FRIENDLY_SIDE);
    }

    #[test]
    fn the_enemy_skill_override_reaches_the_actors_resolved_experience() {
        let (plain, _) = build(None);
        for slot in plain.slots() {
            let resolved = plain
                .mission()
                .actor(slot.id)
                .unwrap()
                .controller()
                .experience();
            let expected = match slot.side {
                launch::Side::Friendly => Experience::Average,
                launch::Side::Enemy => Experience::Ace,
            };
            assert_eq!(resolved.level, expected);
            assert!(matches!(
                resolved.origin,
                ExperienceOrigin::QuickMission { .. }
            ));
        }
        let (forced, _) = build(Some(EnemySkillOverride::AllNovice));
        for slot in forced.slots() {
            let resolved = forced
                .mission()
                .actor(slot.id)
                .unwrap()
                .controller()
                .experience();
            match slot.side {
                // Friendly wings are never touched by the enemy preference.
                launch::Side::Friendly => {
                    assert_eq!(resolved.level, Experience::Average);
                    assert!(matches!(
                        resolved.origin,
                        ExperienceOrigin::QuickMission { .. }
                    ));
                }
                launch::Side::Enemy => {
                    assert_eq!(resolved.level, Experience::Novice);
                    assert_eq!(resolved.origin, ExperienceOrigin::EnemyOverride);
                }
            }
        }
        let (average, _) = build(Some(EnemySkillOverride::AllAverage));
        let enemy = average.slots().iter().find(|s| s.side.is_enemy()).unwrap();
        assert_eq!(
            average
                .mission()
                .actor(enemy.id)
                .unwrap()
                .controller()
                .experience()
                .level,
            Experience::Average
        );
    }

    /// With `--fixture-wings` this bridge is absent, so a target row only
    /// ever receives the fixture integration. This test states that rule and
    /// the next one shows the bridge breaking it, which is the whole point of
    /// the flag.
    #[test]
    fn the_fixture_path_is_a_straight_line_when_the_bridge_is_absent() {
        let mut targets = spawned();
        let start: Vec<Vector> = targets.iter().map(|t| t.position).collect();
        let velocities: Vec<Vector> = targets.iter().map(|t| t.velocity).collect();
        for _ in 0..600 {
            for t in targets.iter_mut().filter(|t| t.hp > 0) {
                for i in 0..3 {
                    t.position[i] += t.velocity[i] / FIXTURE_TICK_RATE;
                }
            }
        }
        for ((target, from), velocity) in targets.iter().zip(&start).zip(&velocities) {
            for i in 0..3 {
                let expected = from[i] + velocity[i] * 600. / FIXTURE_TICK_RATE;
                assert!(
                    (target.position[i] - expected).abs() < 1e-6,
                    "fixture drifted from its straight line"
                );
            }
            assert_eq!(
                target.velocity, *velocity,
                "fixture velocity must not change"
            );
            assert_eq!(
                target.basis.angles(),
                Basis::new(
                    if velocity[2] < 0. {
                        std::f64::consts::PI
                    } else {
                        0.
                    },
                    0.,
                    0.
                )
                .angles(),
                "fixture attitude must not change"
            );
        }
    }

    #[test]
    fn ai_aircraft_move_and_leave_the_straight_line_fixture_path() {
        let (mut wings, mut targets) = build(None);
        let start = wings.positions();
        let fixture: Vec<Vector> = targets
            .iter()
            .map(|t| {
                std::array::from_fn(|i| t.position[i] + t.velocity[i] * 600. / FIXTURE_TICK_RATE)
            })
            .collect();
        run(&mut wings, &mut targets, 600);
        let after = wings.positions();
        for (index, (from, to)) in start.iter().zip(&after).enumerate() {
            let moved: f64 = (0..3)
                .map(|i| (to[i] - from[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            assert!(moved > 1000., "actor {index} barely moved: {moved} feet");
            let straight: f64 = (0..3)
                .map(|i| (to[i] - fixture[index][i]).powi(2))
                .sum::<f64>()
                .sqrt();
            assert!(
                straight > 100.,
                "actor {index} flew the fixture line: {straight} feet apart"
            );
        }
        // The combat world sees the AI pose, not the fixture pose.
        for (slot, target) in wings.slots().iter().zip(&targets) {
            let actor = wings.mission().actor(slot.id).unwrap();
            assert_eq!(target.position, actor.flight().position);
            assert_eq!(target.velocity, actor.flight().velocity);
        }
    }

    #[test]
    fn two_runs_with_the_same_inputs_are_identical() {
        let (mut first, mut first_targets) = build(None);
        let (mut second, mut second_targets) = build(None);
        run(&mut first, &mut first_targets, 900);
        run(&mut second, &mut second_targets, 900);
        assert_eq!(first.positions(), second.positions());
        assert_eq!(first.probe_lines(), second.probe_lines());
    }

    #[test]
    fn every_actor_gets_its_own_seed() {
        let mut seeds = Vec::new();
        for side in [launch::Side::Friendly, launch::Side::Enemy] {
            for wing in 0..3u8 {
                for member in 0..5u8 {
                    seeds.push(actor_seed(side, wing, member));
                }
            }
        }
        let mut sorted = seeds.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            seeds.len(),
            "two actors share a decision seed"
        );
    }

    /// A destroyed target row stops its actor, and a damaged one is reported as
    /// a hit exactly once per hit point drop.
    #[test]
    fn damage_flows_from_the_combat_world_into_the_actors() {
        let (mut wings, mut targets) = build(None);
        run(&mut wings, &mut targets, 10);
        assert!(wings.mission().actor(3).unwrap().alive());
        targets[2].hp = 0;
        run(&mut wings, &mut targets, 1);
        assert!(!wings.mission().actor(3).unwrap().alive());
        run(&mut wings, &mut targets, 5);
        // A dead actor is frozen: the bridge must not keep flying a wreck.
        let resting = wings.mission().actor(3).unwrap().flight().position;
        run(&mut wings, &mut targets, 60);
        assert_eq!(wings.mission().actor(3).unwrap().flight().position, resting);
    }

    /// B47: the report goes to the aircraft the missile is aimed at and to
    /// nobody else, wingmen included.
    #[test]
    fn a_threat_report_reaches_only_the_aimed_at_aircraft() {
        let (mut wings, mut targets) = build(None);
        run(&mut wings, &mut targets, 1);
        let shot = live::Projectile {
            id: 7,
            owner: 1,
            guidance: None,
            motion: None,
            guidance_ticks: None,
            age: 0,
            incoming: false,
            station: 0,
            position: [0., 20000., 30000.],
            previous: [0., 20000., 30000.],
            direction: [0., 0., 1.],
            speed_f8: 2000 * 256,
            launched_t: 0,
            target: Some(3),
            fall: FallState::default(),
        };
        wings.report_threats(std::slice::from_ref(&shot), |_| SeekerClass::Radar);
        // Only actor 3 was told; the others, its wingman included, were not.
        assert_eq!(
            wings.threat_reports(),
            [(3, 7)],
            "the warning was broadcast"
        );
        // The same projectile is never reported twice.
        wings.report_threats(std::slice::from_ref(&shot), |_| SeekerClass::Radar);
        assert_eq!(wings.threat_reports(), [(3, 7)]);
    }

    #[test]
    fn the_activity_line_is_rate_limited() {
        let (mut wings, _) = build(None);
        wings.announce(&[(1, Activity::Attacking)]);
        assert_eq!(
            wings.take_message().as_deref(),
            Some("Friendly 2-1: Attacking")
        );
        // A second change in the same window is dropped rather than replacing
        // the line the player is still reading.
        wings.announce(&[(2, Activity::Defending)]);
        assert!(wings.take_message().is_none());
        // Repeating the same activity is never a change.
        wings.announce(&[(1, Activity::Attacking)]);
        assert!(wings.take_message().is_none());
    }
}
