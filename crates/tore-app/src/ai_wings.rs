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
//! Projectiles carry actor-owned weapon and fire-control observations into
//! the shared seeker lifecycle. AI defense receives the same bounded missile
//! information service as the player RWR. Compatibility steering remains an
//! explicit weapon-rules option.

mod engagement;
pub use engagement::Preset;
mod orders;
mod reports;

use std::collections::{BTreeMap, VecDeque};

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
        weapon_service::{ActorId, RequestId},
    },
    attitude::{Basis, Vector, unit},
    combat::{
        FallState, launch_speed,
        live::{self, MAX_PROJECTILES},
        missiles::{self, Flight, LaunchMode, Motion, Rules, seeker},
    },
    models::FlightModel,
    sensors::{self, Observable, Sensors},
};

use crate::{AppResult, flight, terrain::World};

fn terrain_visible(from: Vector, to: Vector, ground: &dyn Fn(f64, f64) -> f64) -> bool {
    (1..=8).all(|step| {
        let t = step as f64 / 8.;
        let point = std::array::from_fn::<_, 3, _>(|i| from[i] + (to[i] - from[i]) * t);
        point[1] > ground(point[0], point[2])
    })
}

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

/// Fitted synthetic fixture inventory. Imported missions replace it with each
/// aircraft's reviewed default weapon and dispenser records before stepping.
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
    mission_preset: Preset,
    reports: reports::Reports,
    formation_log: Option<std::io::BufWriter<std::fs::File>>,
    slots: Vec<Slot>,
    weapons: BTreeMap<(u32, u8), tore_formats::weapons::Weapon>,
    device_random: tore_sim::ai::DecisionRandom,
    device_effectiveness: BTreeMap<u32, (u8, u8)>,
    /// Projectile ids already turned into threat reports.
    seen_projectiles: Vec<u32>,
    /// Projectile id to the actor that fired it, for B47 attribution. The
    /// player's shots are absent; the bridge remembers its own launchers.
    ai_shots: BTreeMap<u32, u32>,
    /// Last observed hit points per actor, for the damage mirror.
    last_hp: BTreeMap<u32, i32>,
    last_activity: BTreeMap<u32, Activity>,
    next_projectile_id: u32,
    weapon_rules: Rules,
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
    pending_guns: BTreeMap<(u32, u8), PendingGun>,
}

struct PendingGun {
    groups: VecDeque<(u32, u16)>,
    next_scaled: u64,
    ordinal: u64,
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
        guns_only: bool,
        resources: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<Self> {
        let mut bridge = Self::build_with(wings, targets, 0, |id| {
            let bytes = resources
                .get(id.pt())
                .ok_or_else(|| format!("aircraft cache missing {}", id.pt()))?;
            let mut aircraft = Aircraft::parse(bytes)?;
            aircraft.id = id;
            let found = sensors::SensorProfiles::from_source(&aircraft, |name| {
                resources
                    .get(name)
                    .cloned()
                    .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
            })?;
            Ok((aircraft, Some(found)))
        })?;
        for actor in bridge.mission.actors_mut() {
            let aircraft = Aircraft::parse(&resources[actor.identity().aircraft.pt()])?;
            let config = live::Configuration::from_source(&aircraft, |name| {
                resources
                    .get(name)
                    .cloned()
                    .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
            })?;
            let mut stores = Vec::new();
            for (index, station) in config.stations.iter().enumerate() {
                let w = &station.weapon;
                let gun = w.source == aircraft.id.gun();
                let mut spec = if gun {
                    simple_stations(0, u32::from(station.count), AI_STORE_SPEED).remove(0)
                } else {
                    simple_stations(u32::from(station.count), 0, AI_STORE_SPEED).remove(0)
                };
                if guns_only && !gun {
                    spec.store.rounds = tore_sim::ai::weapon_service::Rounds::Finite(0);
                }
                spec.station = tore_sim::ai::weapon_service::StationId(index as u8);
                spec.guided = w.flags & 1 != 0;
                spec.capability = tore_sim::ai::weapon_service::StoreCapability {
                    air: w.flags & 0x10000 != 0,
                    surface: w.flags & 0x20000 != 0,
                };
                // The reviewed default inventory owns the record used at release.
                spec.debit = u32::from(w.burst.actual_rounds_per_game).max(1);
                spec.external_round_lbs = if station.internal {
                    0.0
                } else {
                    f64::from(w.weight.max(0))
                };
                // Fitted: one representative projectile per release, as in
                // the player live adapter. The source ammunition debit remains separate.
                spec.projectile_count = 1;
                spec.store_speed = ScalarSpeed(f64::from(w.movement.maximum_speed));
                spec.tracking_delay =
                    tore_sim::ai::weapon_service::Delay::quarters(u32::from(w.guidance.track_t));
                spec.damage_vs_category = f64::from(w.damage.by_class[0]);
                spec.mount = station.mount;
                spec.requires_radar = w.flags & 0x200 != 0;
                spec.requires_sensor = w.flags & 0x400 != 0;
                let zone = w.seeker.zones[1];
                spec.minimum_range_ft = f64::from(zone.minimum_range);
                spec.maximum_range_ft = Some(f64::from(zone.maximum_range));
                spec.employment_limit_deg = None;
                spec.employment_zone = Some(zone);
                bridge.weapons.insert((actor.id(), index as u8), w.clone());
                stores.push(spec);
            }
            let payload = f64::from(config.external_equipment_lbs)
                + stores
                    .iter()
                    .map(|s| match s.rounds() {
                        tore_sim::ai::weapon_service::Rounds::Finite(n) => {
                            f64::from(n) * s.external_round_lbs
                        }
                        tore_sim::ai::weapon_service::Rounds::Unlimited => 0.0,
                    })
                    .sum::<f64>();
            actor.flight_mut().set_payload(payload)?;
            actor.set_stations(stores);
            actor.set_dispensers(vec![
                tore_sim::ai::threat::DispenserStore {
                    class: SeekerClass::Infrared,
                    count: u32::from(config.ecm.flare[0]),
                },
                tore_sim::ai::threat::DispenserStore {
                    class: SeekerClass::Radar,
                    count: u32::from(config.ecm.chaff[0]),
                },
            ]);
            bridge
                .device_effectiveness
                .insert(actor.id(), (config.ecm.flare[1], config.ecm.chaff[1]));
        }
        Ok(bridge)
    }

    /// [`build`](Self::build) with the aircraft records supplied by the caller,
    /// so a test can build a mission from a synthetic profile and no media.
    pub fn build_with(
        wings: &[WingLaunch],
        targets: &[live::Target],
        _station: usize,
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
                let mut actor = AiActor::new(setup).map_err(|e| e.to_string())?;
                if wing.dummy {
                    actor.set_dummy();
                }
                mission.push(actor);
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
        mission.start_in_formation();
        let formation_log = std::env::var_os("TORE_FORMATION_TRACE").map(|path| {
            use std::io::Write;
            let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
            let mut log = std::io::BufWriter::new(file);
            writeln!(log, "tick,actor,phase,phase_seconds,slot_distance_ft,closure_fps,altitude_error_ft,predicted_separation_ft,yielding_to,x,y,z,speed_fps,bank_deg,g,pitch_input,roll_input,yaw_input,throttle,burner,aim_x,aim_y,aim_z")?;
            Ok::<_, std::io::Error>(log)
        }).transpose()?;
        Ok(Self {
            mission,
            mission_preset: Preset::Free,
            formation_log,
            slots,
            weapons: BTreeMap::new(),
            device_random: tore_sim::ai::DecisionRandom::seeded(0xdec0),
            device_effectiveness: BTreeMap::new(),
            seen_projectiles: Vec::new(),
            ai_shots: BTreeMap::new(),
            last_hp: BTreeMap::new(),
            last_activity: BTreeMap::new(),
            reports: reports::Reports::default(),
            next_projectile_id: AI_PROJECTILE_ID_BASE,
            weapon_rules: Rules::Spec,
            last_message_tick: 0,
            dropped_launches: 0,
            realised_launches: 0,
            threat_reports: Vec::new(),
            pending_message: None,
            pending_guns: BTreeMap::new(),
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
        self.reports.take().or_else(|| self.pending_message.take())
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
        self.weapon_rules = state.weapon_rules;
        self.mission
            .set_missiles(if state.weapon_rules == Rules::Spec {
                state.missile_snapshots(crate::combat::launcher(player))
            } else {
                Vec::new()
            });
        let object = self.player_object(player, state.player_hp, state.configuration());
        let output = self.advance(object, &mut state.targets, &ground)?;
        for event in &output.launches {
            if let Some(weapon) = self.weapons.get(&(event.actor, event.station.0)).cloned() {
                if live::is_gun(&weapon) {
                    let rounds = u16::from(weapon.burst.actual_rounds_per_game.max(1))
                        .saturating_mul(event.projectiles.min(u32::from(u16::MAX)) as u16);
                    let now = self.mission.tick();
                    let physical = u64::from(weapon.burst.actual_rounds_per_game.max(1))
                        * u64::from(weapon.burst.game_rounds_in_burst.max(1));
                    let pending = self
                        .pending_guns
                        .entry((event.actor, event.station.0))
                        .or_insert(PendingGun {
                            groups: VecDeque::new(),
                            next_scaled: now.saturating_mul(physical),
                            ordinal: 0,
                        });
                    if pending.groups.is_empty() {
                        pending.next_scaled = pending.next_scaled.max(now.saturating_mul(physical));
                    }
                    let queued: usize = pending
                        .groups
                        .iter()
                        .map(|(_, remaining)| usize::from(*remaining))
                        .sum();
                    let accepted = usize::from(rounds).min(MAX_PROJECTILES.saturating_sub(queued));
                    self.dropped_launches += u32::from(rounds) - accepted as u32;
                    if accepted > 0 {
                        pending.groups.push_back((event.target, accepted as u16));
                    }
                } else {
                    self.realise(
                        event,
                        &mut state.projectiles,
                        &weapon,
                        usize::from(event.station.0),
                        player.position,
                        None,
                    );
                }
            } else {
                self.dropped_launches += event.projectiles;
            }
        }
        let now = self.mission.tick();
        let keys: Vec<_> = self.pending_guns.keys().copied().collect();
        for key @ (actor, station) in keys {
            let Some(weapon) = self.weapons.get(&key).cloned() else {
                continue;
            };
            let physical = u64::from(weapon.burst.actual_rounds_per_game.max(1))
                * u64::from(weapon.burst.game_rounds_in_burst.max(1));
            let Some(mut pending) = self.pending_guns.remove(&key) else {
                continue;
            };
            if self.mission.actor(actor).is_none_or(|actor| !actor.alive()) {
                self.dropped_launches += pending
                    .groups
                    .iter()
                    .map(|(_, remaining)| u32::from(*remaining))
                    .sum::<u32>();
                pending.groups.clear();
            }
            if let Some((target, remaining)) = pending.groups.front_mut()
                && now.saturating_mul(physical) >= pending.next_scaled
            {
                let event = LaunchEvent {
                    actor,
                    station: tore_sim::ai::weapon_service::StationId(station),
                    target: *target,
                    request_id: RequestId(0),
                    projectiles: 1,
                };
                self.realise(
                    &event,
                    &mut state.projectiles,
                    &weapon,
                    usize::from(station),
                    player.position,
                    Some(pending.ordinal),
                );
                *remaining -= 1;
                pending.ordinal = pending.ordinal.wrapping_add(1);
                pending.next_scaled = pending
                    .next_scaled
                    .saturating_add(u64::from(weapon.burst.game_burst_t.max(1)).saturating_mul(30));
            }
            if pending
                .groups
                .front()
                .is_some_and(|(_, remaining)| *remaining == 0)
            {
                pending.groups.pop_front();
            }
            self.pending_guns.insert(key, pending);
        }
        for event in &output.devices {
            self.realise_device(event, state)?;
        }
        state.set_actor_supports(
            self.mission
                .actors()
                .iter()
                .filter(|a| a.alive())
                .map(|actor| {
                    let observation = actor
                        .controller()
                        .target()
                        .and_then(|id| {
                            actor
                                .awareness()
                                .current_observations()
                                .find(|record| record.target.id == id)
                        })
                        .map(|record| {
                            let delta =
                                missiles::sub(record.target.position, actor.flight().position);
                            seeker::Observation {
                                id: record.target.id,
                                position: record.target.position,
                                velocity: record.velocity,
                                quality: 1.0,
                                off_axis: 0.0,
                                range: missiles::length(delta),
                            }
                        });
                    live::ActorSupport {
                        owner: actor.id(),
                        supported: observation
                            .is_some_and(|o| actor.sensors().is_some_and(|s| s.supports(o.id))),
                        observation,
                        radar_position: actor.flight().position,
                        radar_emitting: actor.flight().radar
                            && actor.sensors().is_some_and(|s| {
                                matches!(s.mode(), Some(sensors::Mode::Rws | sensors::Mode::Tws))
                            }),
                    }
                }),
        );
        if state.weapon_rules == Rules::Compatibility {
            let stations = state.configuration().stations.clone();
            self.report_threats(&state.projectiles, |index| {
                match stations[index].weapon.seeker.signature {
                    2 => Some(SeekerClass::Infrared),
                    3 => Some(SeekerClass::Radar),
                    _ => None,
                }
            });
        } else {
            // Diagnostics list perceived incoming threats only. No launch
            // event is broadcast to an aircraft that cannot detect the missile.
            self.threat_reports = self
                .mission
                .actors()
                .iter()
                .flat_map(|actor| {
                    actor
                        .missile_threats()
                        .filter(|record| record.targeting_receiver)
                        .map(move |record| (actor.id(), record.missile_id))
                })
                .take(64)
                .collect();
        }
        self.report_perceived_attacks(state, player, &ground);
        Ok(())
    }

    /// Report observable attacks, never an opponent's private target choice.
    fn report_perceived_attacks(
        &mut self,
        state: &live::State,
        player: &flight::State,
        ground: &dyn Fn(f64, f64) -> f64,
    ) {
        use tore_sim::ai::{awareness, engagement::ThreatReport};
        use tore_sim::combat::threats::EvidenceSource;
        let mut reports = Vec::new();
        // The player's RWR may identify a supporting source only by a unique
        // independently observed hostile emitter at the received bearing.
        for record in state
            .missile_threats
            .records()
            .filter(|r| r.targeting_receiver && !r.stale)
        {
            let attacker_id = if record.source == EvidenceSource::ElectronicSupported {
                record.radar_bearing_deg.and_then(|bearing| {
                    let mut matches = state.emitters.iter().filter(|emitter| {
                        self.slot(emitter.id)
                            .is_some_and(|slot| slot.side == launch::Side::Enemy)
                            && state.sensors.observation(emitter.id).is_some()
                            && ((emitter.bearing_rad.to_degrees() - bearing + 180.)
                                .rem_euclid(360.)
                                - 180.)
                                .abs()
                                <= 2.
                    });
                    let first = matches.next()?;
                    matches.next().is_none().then_some(first.id)
                })
            } else {
                None
            };
            reports.push((
                PLAYER_ID,
                ThreatReport {
                    attacker_id,
                    defended_id: PLAYER_ID,
                },
                Some(
                    (player.yaw.to_degrees()
                        + record.radar_bearing_deg.unwrap_or(record.bearing_deg))
                    .rem_euclid(360.),
                ),
                Some(record.missile_id),
            ));
        }
        // A fresh, visibly departing missile or tracer can reveal its shooter
        // only when that aircraft is independently observed. Hidden projectile
        // target IDs do not participate in this association.
        for projectile in state.projectiles.iter().filter(|p| p.age <= 30) {
            let weapon = projectile.weapon(state.configuration());
            let gun = live::is_gun(weapon);
            if gun && !projectile.tracer {
                continue;
            }
            for receiver in std::iter::once(PLAYER_ID).chain(
                self.mission
                    .actors()
                    .iter()
                    .filter(|a| a.alive() && !a.is_dummy())
                    .map(AiActor::id),
            ) {
                if receiver == projectile.owner {
                    continue;
                }
                let (position, velocity, heading, pitch, skill, possible_shooters, incoming) =
                    if receiver == PLAYER_ID {
                        (
                            player.position,
                            player.velocity,
                            player.yaw.to_degrees(),
                            player.pitch.to_degrees(),
                            tore_sim::ai::Experience::Ace,
                            state
                                .targets
                                .iter()
                                .filter(|target| {
                                    self.slot(target.id)
                                        .is_some_and(|slot| slot.side == launch::Side::Enemy)
                                })
                                .filter_map(|target| {
                                    state
                                        .sensors
                                        .observation(target.id)
                                        .map(|o| (target.id, o.position))
                                })
                                .collect::<Vec<_>>(),
                            state.missile_threats.records().any(|r| {
                                r.missile_id == projectile.id && r.targeting_receiver && !r.stale
                            }),
                        )
                    } else {
                        let actor = self.mission.actor(receiver).unwrap();
                        (
                            actor.flight().position,
                            actor.flight().velocity,
                            actor.flight().yaw.to_degrees(),
                            actor.flight().pitch.to_degrees(),
                            actor.controller().experience().level,
                            actor
                                .awareness()
                                .current_observations()
                                .filter(|o| o.target.side != actor.identity().side)
                                .map(|o| (o.target.id, o.target.position))
                                .collect::<Vec<_>>(),
                            actor.missile_threats().any(|r| {
                                r.missile_id == projectile.id && r.targeting_receiver && !r.stale
                            }),
                        )
                    };
                if !awareness::visual_eligible(
                    skill,
                    position,
                    heading,
                    pitch,
                    projectile.position,
                    None,
                    terrain_visible(position, projectile.position, ground),
                ) {
                    continue;
                }
                let mut launch_sources = possible_shooters.into_iter().filter(|(_, shooter)| {
                    missiles::length(missiles::sub(*shooter, projectile.previous)) <= 1000.
                        && awareness::visual_eligible(
                            skill,
                            position,
                            heading,
                            pitch,
                            *shooter,
                            None,
                            terrain_visible(position, *shooter, ground),
                        )
                });
                let Some((shooter, _)) = launch_sources.next() else {
                    continue;
                };
                if launch_sources.next().is_some() {
                    continue;
                }
                let gun_incoming = if gun {
                    let delta = missiles::sub(projectile.position, position);
                    let movement = std::array::from_fn::<_, 3, _>(|i| {
                        (projectile.position[i] - projectile.previous[i]) * 120. - velocity[i]
                    });
                    let vv = tore_sim::attitude::dot(movement, movement);
                    let time = if vv > 0. {
                        -tore_sim::attitude::dot(delta, movement) / vv
                    } else {
                        -1.
                    };
                    let closest = std::array::from_fn::<_, 3, _>(|i| delta[i] + movement[i] * time);
                    (0. ..=15.).contains(&time) && missiles::length(closest) <= 1000.
                } else {
                    false
                };
                if !(incoming || gun_incoming) {
                    continue;
                }
                let delta = missiles::sub(projectile.position, position);
                reports.push((
                    receiver,
                    ThreatReport {
                        attacker_id: Some(shooter),
                        defended_id: receiver,
                    },
                    Some(delta[0].atan2(delta[2]).to_degrees().rem_euclid(360.)),
                    Some(projectile.id),
                ));
            }
        }
        for (receiver, report, bearing, event_id) in reports {
            self.mission
                .report_attack_evidence(receiver, report, bearing, event_id);
        }
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
        self.record_formation_trace();
        self.formation_reports();
        self.mirror_pose_out(targets);
        self.announce(&output.activities);
        Ok(output)
    }

    fn record_formation_trace(&mut self) {
        use std::io::Write;
        let Some(log) = self.formation_log.as_mut() else {
            return;
        };
        let tick = self.mission.tick();
        if !tick.is_multiple_of(12) {
            return;
        }
        let result = (|| -> std::io::Result<()> {
            for actor in self.mission.actors() {
                let Some(trace) = actor.controller().formation_trace() else {
                    continue;
                };
                let state = actor.flight();
                let input = actor.last_input();
                writeln!(
                    log,
                    "{},{},{:?},{:.3},{:.2},{:.2},{:.2},{:.2},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.3},{:.4},{:.4},{:.4},{:.4},{},{:.2},{:.2},{:.2}",
                    tick,
                    actor.id(),
                    trace.phase,
                    trace.phase_seconds,
                    trace.slot_distance_ft,
                    trace.closure_fps,
                    trace.altitude_error_ft,
                    trace.minimum_predicted_separation_ft,
                    trace.yielding_to.map_or(String::new(), |id| id.to_string()),
                    state.position[0],
                    state.position[1],
                    state.position[2],
                    state.speed,
                    state.bank.to_degrees(),
                    state.g,
                    input.pitch,
                    input.roll,
                    input.yaw,
                    state.throttle,
                    state.afterburner_active(),
                    trace.aim[0],
                    trace.aim[1],
                    trace.aim[2]
                )?;
            }
            if tick.is_multiple_of(120) {
                log.flush()?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("Formation trace disabled after write failure: {error}");
            self.formation_log = None;
        }
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

    /// A missile blast knocks an AI aircraft around, like the player's.
    pub fn jolt(&mut self, id: u32, from: [f64; 3], strength: f64) {
        if let Some(actor) = self.mission.actor_mut(id) {
            actor.flight_mut().jolt_from(from, strength);
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
            if let Some(actor) = self.mission.actor_mut(slot.id) {
                actor.flight_mut().damage_fraction = 1.0
                    - (f64::from(target.hp) / f64::from(target.initial_hp.max(1))).clamp(0.0, 1.0);
            }
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
    pub fn mirror_pose_out(&self, targets: &mut [live::Target]) {
        for slot in &self.slots {
            let Some(actor) = self.mission.actor(slot.id) else {
                continue;
            };
            let Some(target) = targets.iter_mut().find(|t| t.id == slot.id) else {
                continue;
            };
            // Combat owns falling wrecks after destruction.
            if !actor.alive() || target.hp <= 0 {
                continue;
            }
            let f = actor.flight();
            target.position = f.position;
            target.velocity = f.velocity;
            target.basis = Basis::new(f.yaw, f.pitch, f.bank);
            target.radar_emitting = f.radar;
            target.wreck_power = f.wreck_power(target.wreck_power.engine_count.max(1));
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
        gun_ordinal: Option<u64>,
    ) -> u32 {
        let Some(actor) = self.mission.actor(event.actor) else {
            self.dropped_launches += event.projectiles;
            return 0;
        };
        let origin = actor.flight().position;
        let observed = actor
            .awareness()
            .current_observations()
            .find(|record| record.target.id == event.target)
            .copied();
        let aim = if let Some(observation) = observed {
            observation.target.position
        } else if actor.sensors().is_none() {
            // Sensorless synthetic fixtures explicitly supply permitted world
            // targets. Live aircraft never use this fallback.
            if event.target == PLAYER_ID {
                player_position
            } else if let Some(other) = self.mission.actor(event.target) {
                other.flight().position
            } else {
                self.dropped_launches += event.projectiles;
                return 0;
            }
        } else {
            self.dropped_launches += event.projectiles;
            return 0;
        };
        let direction = unit([aim[0] - origin[0], aim[1] - origin[1], aim[2] - origin[2]]);
        if direction.iter().any(|v| !v.is_finite()) {
            self.dropped_launches += event.projectiles;
            return 0;
        }
        let Ok(speed) = launch_speed(&weapon.movement, (actor.flight().speed * 256.) as i32) else {
            self.dropped_launches += event.projectiles;
            return 0;
        };
        let profile = (self.weapon_rules == Rules::Spec)
            .then(|| missiles::Profile::for_weapon(weapon))
            .flatten();
        let guidance = profile.map(|profile| {
            Flight::from_supported_launch(
                profile,
                LaunchMode::Cued,
                seeker::Observation {
                    id: event.target,
                    position: aim,
                    velocity: observed.map_or([0.; 3], |record| record.velocity),
                    quality: 1.0,
                    off_axis: 0.0,
                    range: missiles::length(missiles::sub(aim, origin)),
                },
                origin,
            )
        });
        // Compatibility keeps its existing steering; reviewed profiles use
        // the same owner-aware seeker/propulsion lifecycle as player shots.
        let launched = (self.mission.tick() / 30) as u16;
        let incoming = event.target == PLAYER_ID;
        let mut emitted = 0;
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
                weapon: Some(weapon.clone()),
                guidance: guidance.clone(),
                motion: profile
                    .map(|_| Motion::new(&weapon.movement, actor.flight().velocity, origin[1])),
                guidance_ticks: profile.map(|p| p.guidance_ticks),
                age: 0,
                incoming,
                station,
                position: origin,
                previous: origin,
                direction,
                speed_f8: speed * 256,
                launched_t: launched,
                target: (weapon.seeker.signature != 0).then_some(event.target),
                fall: FallState::default(),
                gun_round: gun_ordinal.map(|ordinal| {
                    (ordinal % u64::from(weapon.burst.actual_rounds_per_game.max(1))) as u8
                }),
                tracer: gun_ordinal.is_some_and(|ordinal| ordinal.is_multiple_of(3)),
            });
            self.ai_shots.insert(id, event.actor);
            self.realised_launches += 1;
            emitted += 1;
        }
        emitted
    }

    fn realise_device(
        &mut self,
        event: &tore_sim::ai::mission::DeviceEvent,
        state: &mut live::State,
    ) -> AppResult<()> {
        use tore_sim::ai::threat::{self, DecoyOutcome, GuidingMissile};
        let Some(actor) = self.mission.actor(event.actor) else {
            return Ok(());
        };
        let effectiveness = self
            .device_effectiveness
            .get(&event.actor)
            .copied()
            .unwrap_or((100, 100));
        let effectiveness = match event.class {
            SeekerClass::Infrared => effectiveness.0,
            SeekerClass::Radar => effectiveness.1,
        };
        for _ in 0..event.released {
            state.effects.push(live::Effect {
                position: actor.flight().position,
                kind: match event.class {
                    SeekerClass::Infrared => live::EffectKind::Flare,
                    SeekerClass::Radar => live::EffectKind::Chaff,
                },
                ticks: 45,
            });
            let config = state.configuration().clone();
            for projectile in &mut state.projectiles {
                let weapon = projectile.weapon(&config);
                let class = match weapon.seeker.signature {
                    2 => SeekerClass::Infrared,
                    3 => SeekerClass::Radar,
                    _ => continue,
                };
                let missile = GuidingMissile {
                    seeker: class,
                    guiding_on_releaser: projectile.target == Some(event.actor)
                        && projectile.guidance.as_ref().is_none_or(|flight| {
                            flight.enabled
                                && flight.seeker.acquired
                                && flight.seeker.observation.is_some()
                        }),
                    decoy_susceptibility_percent: weapon.seeker.chaff_flare_chance,
                };
                if threat::decoy_missile(
                    &missile,
                    event.class,
                    effectiveness,
                    &mut self.device_random,
                )
                .map_err(|e| e.to_string())?
                    == DecoyOutcome::Decoyed
                {
                    projectile.target = None;
                    projectile.guidance = None;
                    // Fitted: coast after decoy, using the existing record lifetime.
                }
            }
        }
        Ok(())
    }

    /// B47: every missile in the world is reported once, to its target only.
    /// Wingmen are not told, and neither is anyone else: only the aircraft the
    /// shot is actually aimed at receives a report.
    fn report_threats(
        &mut self,
        projectiles: &[live::Projectile],
        seeker_of: impl Fn(usize) -> Option<SeekerClass>,
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
            let seeker = if let Some(weapon) = &projectile.weapon {
                match weapon.seeker.signature {
                    2 => SeekerClass::Infrared,
                    3 => SeekerClass::Radar,
                    _ => continue,
                }
            } else {
                let Some(class) = seeker_of(projectile.station) else {
                    continue;
                };
                class
            };
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

/// Imported-media validation, separate from synthetic model tests. No retail
/// bytes or generated assets are written by this probe.
pub fn roster_probe(
    ticks: usize,
    resources: &BTreeMap<String, Vec<u8>>,
    world: &World,
) -> AppResult<()> {
    use tore_sim::ai::{
        Experience,
        launch::{WingId, WingSelection, resolve_wings},
    };
    for id in AircraftId::ALL {
        let aircraft = Aircraft::parse(
            resources
                .get(id.pt())
                .ok_or_else(|| format!("missing {}", id.pt()))?,
        )?;
        let config = live::Configuration::from_source(&aircraft, |name| {
            resources
                .get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
        })?;
        for level in Experience::ALL {
            let wings = resolve_wings(
                &[
                    WingSelection {
                        wing: WingId::new(launch::Side::Friendly, 0)?,
                        aircraft: id,
                        count: 1,
                        skill_level: level.index() as i32,
                    },
                    WingSelection {
                        wing: WingId::new(launch::Side::Enemy, 0)?,
                        aircraft: id,
                        count: 1,
                        skill_level: level.index() as i32,
                    },
                ],
                None,
            )?;
            let mut combat = live::State::new(config.clone(), true)?;
            combat.add_dummy(&config, [512.0, 30000.0, -512.0], Basis::new(0.0, 0.0, 0.0));
            combat.add_dummy(
                &config,
                [0.0, 30000.0, 30000.0],
                Basis::new(std::f64::consts::PI, 0.0, 0.0),
            );
            let mut bridge = AiWings::build(&wings, &combat.targets, false, resources)?;
            for actor in bridge.mission.actors() {
                if actor.stations().len() != config.stations.len()
                    || actor
                        .stations()
                        .iter()
                        .zip(&config.stations)
                        .any(|(station, imported)| {
                            station.employment_zone != Some(imported.weapon.seeker.zones[1])
                                || station.rounds()
                                    != tore_sim::ai::weapon_service::Rounds::Finite(u32::from(
                                        imported.count,
                                    ))
                        })
                {
                    return Err(format!("AI inventory/envelope mismatch for {}", id.pt()).into());
                }
            }
            let mut player = flight::State::new(&aircraft, [0.0, 30000.0, 0.0])?;
            let mut peak_roll = 0.0f64;
            let mut peak_turn = 0.0f64;
            for _ in 0..ticks {
                let before: Vec<_> = bridge
                    .mission
                    .actors()
                    .iter()
                    .map(|a| (a.alive(), a.flight().clone()))
                    .collect();
                player.step(&flight::PilotInput::default(), |x, z| {
                    f64::from(world.height(x as f32, z as f32))
                });
                combat.step(false, crate::combat::launcher(&player), |x, z| {
                    f64::from(world.height(x as f32, z as f32))
                });
                bridge.step(&mut combat, &player, world)?;
                for (actor, (was_alive, mut replay)) in bridge.mission.actors().iter().zip(before) {
                    let bank = replay.bank;
                    let yaw = replay.yaw;
                    if was_alive && actor.alive() {
                        replay.damage_fraction = actor.flight().damage_fraction;
                        replay.payload_lbs = actor.flight().payload_lbs;
                        replay.step(actor.last_input(), |x, z| {
                            f64::from(world.height(x as f32, z as f32))
                        });
                        if &replay != actor.flight() {
                            return Err(
                                format!("AI input replay diverged {} {level:?}", id.pt()).into()
                            );
                        }
                    }
                    let f = actor.flight();
                    if !f.position.iter().all(|v| v.is_finite()) {
                        return Err(format!("nonfinite {} {level:?}", id.pt()).into());
                    }
                    let rate = |delta: f64| {
                        ((delta.to_degrees() + 180.0).rem_euclid(360.0) - 180.0).abs() * 120.0
                    };
                    peak_roll = peak_roll.max(rate(f.bank - bank));
                    peak_turn = peak_turn.max(rate(f.yaw - yaw));
                }
            }
            println!(
                "AI roster {} {level:?}: ticks={ticks} models=2 radar={} ir={} stores={} gun={} projectiles={} dropped={} peak_roll={peak_roll:.3} peak_turn={peak_turn:.3} PASS",
                id.pt(),
                config.sensors.radar.is_some(),
                config.sensors.infrared.is_some(),
                config.stations.len(),
                config.stations.iter().any(|s| s.weapon.source == id.gun()),
                bridge.realised_launches,
                bridge.dropped_launches
            );
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::terrain::tests::world;
    use tore_sim::{
        ai::{
            Experience,
            controller::TargetView,
            engagement::{GroupObjective, Policy, Priority},
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

    pub(crate) fn combat_fixture(guided: bool) -> live::State {
        use live::{Configuration, State, Station};
        use tore_formats::weapons::*;
        let zone = Zone {
            heading: 12000,
            pitch: 12000,
            minimum_range: 0,
            maximum_range: 10000,
            minimum_altitude: i32::MIN,
            maximum_altitude: i32::MAX,
        };
        let seeker = Seeker {
            flags: [0; 2],
            signature: if guided { 3 } else { 0 },
            look_down: 0,
            doppler_above: 0,
            doppler_below: 0,
            doppler_minimum_range: 0,
            all_aspect: 0,
            zones: [zone; 2],
            chaff_flare_chance: 0,
            deception_chance: 0,
        };
        let w = Weapon {
            source: "SYNTHETIC.JT".into(),
            name: "Synthetic".into(),
            hud_name: "SYN".into(),
            shape: None,
            fire_sound: None,
            native_callback: "_PROJProc".into(),
            flags: if guided { 0x240 } else { 0x844 },
            object_flags: 0,
            weight: 10,
            movement: Movement {
                minimum_speed: 10,
                corner_speed: 1000,
                maximum_speed: 2000,
                acceleration: 100,
                deceleration: 2,
                initial_speed: 1000,
                final_speed: 500,
                launch_retard: 100,
                ignite_t: 0,
                fuel_t: 10,
                remove_t: 20,
                powered_turn_rate: 10000,
                unpowered_turn_rate: 10000,
                performance_at_0: 100,
                performance_at_20: 100,
                cruise: [0; 4],
                jink: [0; 3],
            },
            burst: Burst {
                projectiles_in_pod: 1,
                actual_rounds_per_game: 2,
                game_rounds_in_burst: 1,
                game_rounds_in_carpet_burst: 1,
                game_burst_t: 1,
                reload_t: 0,
                startup_shots: 0,
                random_fire_percent: 0,
                offset_fire_percent: 0,
                offset_fire_heading: 0,
                offset_fire_pitch: 0,
                sine_pattern: [0; 4],
            },
            seeker,
            guidance: Guidance {
                track_t: 1,
                track_max_g_raw: 1,
                target_sun_chance: 0,
                max_aon: 0,
                chances: [100; 4],
                hit_modifiers: [0; 9],
            },
            damage: Damage {
                by_class: [10; 5],
                fuze_arm_t: 0,
                fuze_radius: 0,
                side_hit_fuze_failure: 0,
                collateral_radius: 0,
                collateral_percent: 0,
            },
            effects: Effects {
                object_explosion: 0,
                land_explosion: 0,
                water_explosion: 0,
                crater_size: 0,
                smoke: [0; 5],
                max_sound_distance: 0,
                frequency_adjustment: 0,
            },
        };
        State::new(
            Configuration {
                fragment_offsets: [[0.; 3]; 2],
                ecm: tore_formats::weapons::Countermeasures {
                    weight: 0,
                    flags: 0,
                    mode_flags: 0x10,
                    chaff: [0; 4],
                    flare: [0; 4],
                    radar_deception_chance: 30,
                    radar_signature_add: 0,
                    radar_noise_range: [0; 2],
                    infrared_deception_chance: 0,
                    infrared_signature_add: 0,
                    infrared_lose_lock_time: 0,
                },
                system_damage: [0x11; 45],
                damage_capacity: 30,
                afterburner_available: true,
                hardpoint_slots: vec![Some(0)],
                radar_hardpoint: 1,
                visual_hardpoint: 3,
                ecm_hardpoint: 2,
                aircraft: AircraftId::F18,
                stations: vec![Station {
                    weapon: w,
                    mount: [0.; 3],
                    count: 11,
                    internal: !guided,
                }],
                hit_points: 20,
                target_category: 0x80,
                external_equipment_lbs: 0,
                external_fuel_lbs: [0.; 9],
                engines: 1,
                wreck_power: tore_sim::wreck::Power::default(),
                infrared_hardpoint: None,
                rwr_hardpoint: None,
                sensors: sensors::SensorProfiles {
                    aircraft: AircraftId::F18,
                    radar: None,
                    infrared: None,
                    visual: None,
                    jammer: None,
                    signature: sensors::SignatureProfile::default(),
                },
            },
            true,
        )
        .unwrap()
    }
    fn aircraft() -> Aircraft {
        crate::flight::animation_tests::profile()
    }

    fn flat(_x: f64, _z: f64) -> f64 {
        0.0
    }

    fn target(id: u32, position: Vector, yaw: f64) -> live::Target {
        let basis = Basis::new(yaw, 0., 0.);
        live::Target {
            aircraft: Some(AircraftId::F18),
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
            fragment_offsets: [[0.; 3]; 2],
            wreck: None,
            wreck_power: tore_sim::wreck::Power::default(),
            fragment_released: false,
            localized_damage: live::LocalizedDamage::default(),
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

    #[test]
    fn player_group_objective_distinguishes_two_populated_enemy_groups_from_free_fire() {
        let selections = [
            (launch::Side::Friendly, 1u8),
            (launch::Side::Enemy, 0),
            (launch::Side::Enemy, 1),
        ]
        .map(|(side, index)| WingSelection {
            wing: WingId::new(side, index).unwrap(),
            aircraft: AircraftId::F18,
            count: 2,
            skill_level: 1,
        });
        let payload = resolve_wings(&selections, None).unwrap();
        let targets = vec![
            target(1, [0., 20000., 0.], 0.),
            target(2, [1500., 20000., 0.], 0.),
            target(3, [0., 20000., 40000.], std::f64::consts::PI),
            target(4, [1500., 20000., 40000.], std::f64::consts::PI),
            target(5, [8000., 20000., 40000.], std::f64::consts::PI),
            target(6, [9500., 20000., 40000.], std::f64::consts::PI),
        ];
        let mut wings =
            AiWings::build_with(&payload, &targets, 0, |_| Ok((aircraft(), None))).unwrap();
        let enemy_one = WingId::new(launch::Side::Enemy, 0).unwrap();
        let mut objectives = [GroupObjective::Inherit; 6];
        objectives[0] = GroupObjective::Intercept(enemy_one);
        objectives[1] = GroupObjective::Intercept(enemy_one);
        wings.apply_group_objectives(&objectives, [0., 20000., 0.]);

        assert_eq!(wings.mission.player_assignment().destroy_ids, [3, 4]);
        for id in [1, 2] {
            assert_eq!(
                wings.mission.actor(id).unwrap().assignment().destroy_ids,
                [3, 4]
            );
        }
        for id in [3, 4] {
            assert!(wings.objective_for_player(id));
        }
        for id in [5, 6] {
            assert!(!wings.objective_for_player(id));
        }
        let mut group_one = crate::target_window::Readout::new(
            &targets[2],
            wings.mission.actor(3).unwrap().flight(),
            "TEST".into(),
        );
        group_one.with_activity(&wings);
        let mut group_two = crate::target_window::Readout::new(
            &targets[4],
            wings.mission.actor(5).unwrap().flight(),
            "TEST".into(),
        );
        group_two.with_activity(&wings);
        assert_eq!(
            group_one.objective,
            Some(crate::target_window::TargetObjective::Destroy)
        );
        assert_eq!(group_two.objective, None);

        objectives[0] = GroupObjective::Free;
        wings.apply_group_objectives(&objectives, [0., 20000., 0.]);
        assert!(wings.mission.player_assignment().destroy_ids.is_empty());
        assert!(
            [3, 4, 5, 6]
                .into_iter()
                .all(|id| !wings.objective_for_player(id))
        );
        let contact = |id, position| TargetView {
            id,
            side: ENEMY_SIDE,
            position,
            heading_deg: 0.,
            pitch_deg: 0.,
            speed: ScalarSpeed(400.),
            maximum_speed: ScalarSpeed(800.),
            is_aircraft: true,
            is_fighter: true,
            human_controlled: false,
            valid: true,
            type_allowed: true,
            seeker_eligible: true,
            wing_attackers: 0,
            terrain_blocked: false,
            sensor_supported: true,
        };
        for candidate in [
            contact(3, [1000., 20000., 0.]),
            contact(5, [1000., 20000., 0.]),
        ] {
            let selected = Policy::default()
                .select(
                    PLAYER_ID,
                    FRIENDLY_SIDE,
                    [0., 20000., 0.],
                    wings.mission.player_assignment(),
                    &[candidate],
                    &[],
                    &[],
                    None,
                    1,
                )
                .unwrap();
            assert_eq!(selected.id, candidate.id);
            assert_eq!(selected.priority, Priority::Free);
        }
    }

    #[test]
    fn dummy_mode_reaches_live_targets_without_changing_other_wings() {
        let mut payload = payload(None);
        payload[1].dummy = true;
        let mut targets = spawned();
        let mut wings =
            AiWings::build_with(&payload, &targets, 0, |_| Ok((aircraft(), None))).unwrap();
        wings.mirror_pose_out(&mut targets);
        assert!(!wings.mission.actor(1).unwrap().is_dummy());
        assert!(wings.mission.actor(3).unwrap().is_dummy());
        let start = targets[2].position;
        let velocity = targets[2].velocity;
        for _ in 0..120 {
            wings
                .advance(
                    player_object([0., 20000., -20000.]),
                    &mut targets,
                    &|_, _| 0.,
                )
                .unwrap();
        }
        for i in 0..3 {
            assert!((targets[2].position[i] - start[i] - velocity[i]).abs() < 1e-7);
        }
        assert_eq!(targets[2].velocity, velocity);
        assert!(
            (tore_sim::attitude::dot(velocity, velocity).sqrt() - launch::DUMMY_SPEED_FPS).abs()
                < 1e-9
        );
        targets[2].hp = 0;
        let stopped = targets[2].position;
        wings
            .advance(
                player_object([0., 20000., -20000.]),
                &mut targets,
                &|_, _| 0.,
            )
            .unwrap();
        assert_eq!(targets[2].position, stopped);
        assert!(!wings.mission.actor(3).unwrap().alive());
    }

    pub(super) fn build(
        enemy_override: Option<EnemySkillOverride>,
    ) -> (AiWings, Vec<live::Target>) {
        let targets = spawned();
        let wings = AiWings::build_with(&payload(enemy_override), &targets, 0, |_| {
            Ok((aircraft(), None))
        })
        .unwrap();
        (wings, targets)
    }

    #[test]
    fn both_sides_start_neutral_with_free_fire_objectives() {
        let (mut wings, mut targets) = build(None);
        wings.apply_mission_preset(Preset::Free, [0., 20000., 0.]);
        wings.apply_group_objectives(&[GroupObjective::Inherit; 6], [0., 20000., 0.]);
        assert!(wings.mission.actors().iter().all(AiActor::is_neutral));
        for _ in 0..240 {
            let output = wings
                .advance(
                    player_object([0., 20000., -20000.]),
                    &mut targets,
                    &|_, _| 0.,
                )
                .unwrap();
            assert!(output.launches.is_empty());
        }
        assert!(wings.mission.actors().iter().all(AiActor::is_neutral));
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
    fn formation_trace_records_controls_without_changing_flight() {
        let selections = [WingSelection {
            wing: WingId::new(launch::Side::Friendly, 0).unwrap(),
            aircraft: AircraftId::F18,
            count: 2,
            skill_level: 2,
        }];
        let payload = resolve_wings(&selections, None).unwrap();
        let targets = vec![
            target(1, [512., 20000., -512.], 0.),
            target(2, [-512., 20000., -512.], 0.),
        ];
        let build =
            || AiWings::build_with(&payload, &targets, 0, |_| Ok((aircraft(), None))).unwrap();
        let mut logged = build();
        let mut plain = build();
        let path =
            std::env::temp_dir().join(format!("tore-formation-trace-{}.csv", std::process::id()));
        logged.formation_log = Some(std::io::BufWriter::new(
            std::fs::File::create(&path).unwrap(),
        ));
        let mut a = targets.clone();
        let mut b = targets;
        for tick in 0..120 {
            let player = player_object([0., 20000., 800. * tick as f64 / 120.]);
            logged.advance(player.clone(), &mut a, &flat).unwrap();
            plain.advance(player, &mut b, &flat).unwrap();
            for id in 1..=2 {
                assert_eq!(
                    logged.mission.actor(id).unwrap().flight(),
                    plain.mission.actor(id).unwrap().flight()
                );
            }
        }
        drop(logged);
        let rows = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(rows.lines().count(), 20);
        for row in rows.lines() {
            let columns: Vec<_> = row.split(',').collect();
            assert_eq!(columns.len(), 23);
            assert_eq!(
                columns[17].parse::<f64>().unwrap(),
                0.,
                "formation commands no rudder"
            );
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
            weapon: None,
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
            gun_round: None,
            tracer: false,
        };
        wings.report_threats(std::slice::from_ref(&shot), |_| Some(SeekerClass::Radar));
        // Only actor 3 was told; the others, its wingman included, were not.
        assert_eq!(
            wings.threat_reports(),
            [(3, 7)],
            "the warning was broadcast"
        );
        // The same projectile is never reported twice.
        wings.report_threats(std::slice::from_ref(&shot), |_| Some(SeekerClass::Radar));
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
    #[test]
    fn destroyed_airframe_falls_through_combat_bridge_order_and_restart() {
        for _restart in 0..2 {
            let (mut wings, targets) = build(None);
            let mut combat = combat_fixture(false);
            combat.targets = targets;
            combat.targets[2].position[1] = 1000.0;
            combat.targets[2].hp = 0;
            let player = flight::State::new(&aircraft(), [0.0, 20000.0, -5000.0]).unwrap();
            let mut previous = 1000.0;
            for tick in 0..1500 {
                combat.step(false, crate::combat::launcher(&player), flat);
                wings
                    .advance(player_object(player.position), &mut combat.targets, &flat)
                    .unwrap();
                let wreck = &combat.targets[2];
                assert!(wreck.position[1] <= previous, "wreck rose on tick {tick}");
                if tick == 120 {
                    assert!(wreck.position[1] < 990.0);
                }
                previous = wreck.position[1];
            }
            assert_eq!(combat.targets[2].position[1], 0.0);
            assert_eq!(combat.targets[2].velocity, [0.0; 3]);
            assert!(!wings.mission.actor(3).unwrap().alive());
        }
    }

    #[test]
    fn realised_gun_keeps_its_own_record_and_never_becomes_a_player_missile() {
        use tore_sim::ai::weapon_service::{RequestId, StationId};
        let (mut wings, _) = build(None);
        let mut combat = combat_fixture(true);
        let mut gun = combat_fixture(false).configuration().stations[0]
            .weapon
            .clone();
        gun.source = "SYNTHETIC-GUN.JT".into();
        let event = LaunchEvent {
            actor: 3,
            station: StationId(1),
            target: 0,
            request_id: RequestId(1),
            projectiles: 10,
        };
        let player = flight::State::new(&aircraft(), [0.0, 20000.0, -5000.0]).unwrap();
        wings.realise(
            &event,
            &mut combat.projectiles,
            &gun,
            1,
            player.position,
            None,
        );
        assert_eq!(combat.projectiles.len(), 10);
        assert!(
            combat
                .projectiles
                .iter()
                .all(|p| p.weapon.as_ref() == Some(&gun) && p.target.is_none())
        );
        // Station 1 does not even exist on the player. Stepping must use the owned record.
        combat.step(false, crate::combat::launcher(&player), flat);
        assert!(
            combat
                .projectiles
                .iter()
                .all(|p| p.weapon(combat.configuration()).seeker.signature == 0)
        );
    }

    #[test]
    fn canonical_gun_queue_preserves_fifo_phase_and_stops_for_dead_actor() {
        let (mut wings, targets) = build(None);
        for actor in wings.mission.actors_mut() {
            actor.set_stations(Vec::new());
        }
        let mut combat = combat_fixture(false);
        combat.targets = targets;
        let mut gun = combat.configuration().stations[0].weapon.clone();
        gun.source = "M61.JT".into();
        gun.burst.actual_rounds_per_game = 2;
        gun.burst.game_rounds_in_burst = 4;
        gun.burst.game_burst_t = 1;
        wings.weapons.insert((3, 0), gun);
        wings.pending_guns.insert(
            (3, 0),
            PendingGun {
                groups: VecDeque::from([(PLAYER_ID, 2), (4, 2)]),
                next_scaled: wings.mission.tick() * 8,
                ordinal: 0,
            },
        );
        let player = flight::State::new(&aircraft(), [0., 20000., -5000.]).unwrap();
        let mut release_ticks = Vec::new();
        for tick in 0..16 {
            let before = combat.projectiles.len();
            wings.step(&mut combat, &player, &world()).unwrap();
            let count = combat.projectiles.len() - before;
            assert!(count <= 1);
            if count == 1 {
                release_ticks.push(tick);
            }
        }
        assert!(
            release_ticks
                .windows(2)
                .all(|pair| (3..=4).contains(&(pair[1] - pair[0])))
        );
        let rounds: Vec<_> = combat.projectiles.iter().filter(|p| p.owner == 3).collect();
        assert_eq!(rounds.len(), 4);
        assert_eq!(
            rounds.iter().map(|p| p.incoming).collect::<Vec<_>>(),
            [true, true, false, false]
        );
        assert_eq!(
            rounds.iter().map(|p| p.gun_round).collect::<Vec<_>>(),
            [Some(0), Some(1), Some(0), Some(1)]
        );
        assert_eq!(
            rounds.iter().map(|p| p.tracer).collect::<Vec<_>>(),
            [true, false, false, true]
        );
        assert!(wings.pending_guns[&(3, 0)].groups.is_empty());
        assert_eq!(wings.pending_guns[&(3, 0)].ordinal, 4);

        wings.pending_guns.get_mut(&(3, 0)).unwrap().groups = VecDeque::from([(PLAYER_ID, 2)]);
        wings.mission.actor_mut(3).unwrap().set_alive(false);
        let dropped = wings.dropped_launches;
        wings.step(&mut combat, &player, &world()).unwrap();
        assert!(wings.pending_guns[&(3, 0)].groups.is_empty());
        assert_eq!(wings.dropped_launches, dropped + 2);
        assert_eq!(wings.pending_guns[&(3, 0)].ordinal, 4);
    }

    #[test]
    fn accepted_recall_cancels_remaining_physical_gun_burst() {
        use tore_sim::ai::wing::{Formation, PlayerOrder};
        for order in [
            PlayerOrder::Formation(Formation::Echelon),
            PlayerOrder::Disengage,
        ] {
            let mut selections = payload(None);
            selections[0].wing.index = 0;
            let mut wings =
                AiWings::build_with(&selections, &spawned(), 0, |_| Ok((aircraft(), None)))
                    .unwrap();
            for actor in wings.mission.actors_mut() {
                actor.set_stations(Vec::new());
            }
            let mut combat = combat_fixture(false);
            combat.targets = spawned();
            let gun = combat.configuration().stations[0].weapon.clone();
            wings.weapons.insert((1, 0), gun);
            wings.pending_guns.insert(
                (1, 0),
                PendingGun {
                    groups: VecDeque::from([(3, 3)]),
                    next_scaled: 0,
                    ordinal: 0,
                },
            );
            let player = flight::State::new(&aircraft(), [0., 20000., -5000.]).unwrap();
            wings.step(&mut combat, &player, &world()).unwrap();
            assert_eq!(combat.projectiles.len(), 1, "{order:?}");
            let rounds = wings.mission.actor(1).unwrap().rounds_remaining();
            let dropped = wings.dropped_launches;
            let report = wings.command(order, None, Some(1)).unwrap();
            assert!(report.message.contains("1 applied"), "{order:?}");
            assert!(!wings.pending_guns.contains_key(&(1, 0)));
            for _ in 0..20 {
                wings.step(&mut combat, &player, &world()).unwrap();
            }
            assert_eq!(combat.projectiles.len(), 1, "{order:?}");
            assert_eq!(wings.mission.actor(1).unwrap().rounds_remaining(), rounds);
            assert_eq!(wings.dropped_launches, dropped);
        }
    }

    #[test]
    fn normal_startup_selects_and_arms_canonical_gun() {
        let fixture = combat_fixture(false);
        let mut config = fixture.configuration().clone();
        config.stations[0].weapon.source = "M61.JT".into();
        let mut missile = config.stations[0].clone();
        missile.weapon.source = "AIM9M.JT".into();
        config.stations.insert(0, missile);
        let mut state = live::State::new(config, true).unwrap();
        state.selected = 0;
        state.armed = true;
        crate::combat::apply_startup_weapon_state(&mut state);
        assert_eq!(state.selected, 1);
        assert!(state.armed);
    }

    #[test]
    fn active_ai_launch_uses_owned_guidance_and_cannot_be_decoyed_before_pitbull() {
        use tore_sim::ai::{mission::DeviceEvent, weapon_service::StationId};
        let (mut wings, _) = build(None);
        let mut combat = combat_fixture(true);
        let mut weapon = combat.configuration().stations[0].weapon.clone();
        weapon.source = "AIM120.JT".into();
        weapon.seeker.signature = 3;
        weapon.seeker.chaff_flare_chance = 100;
        wings.device_effectiveness.insert(3, (100, 100));
        let event = LaunchEvent {
            actor: 1,
            station: StationId(0),
            target: 3,
            request_id: RequestId(1),
            projectiles: 1,
        };
        assert_eq!(
            wings.realise(&event, &mut combat.projectiles, &weapon, 0, [0.; 3], None),
            1
        );
        assert!(combat.projectiles[0].guidance.is_some());
        assert!(combat.projectiles[0].motion.is_some());
        let release = DeviceEvent {
            actor: 3,
            class: SeekerClass::Radar,
            released: 1,
        };
        wings.realise_device(&release, &mut combat).unwrap();
        assert_eq!(combat.projectiles[0].target, Some(3));
        let flight = combat.projectiles[0].guidance.as_mut().unwrap();
        flight.enabled = true;
        flight.seeker.acquired = true;
        flight.seeker.observation = Some(seeker::Observation {
            id: 3,
            position: [0.; 3],
            velocity: [0.; 3],
            quality: 1.,
            off_axis: 0.,
            range: 1000.,
        });
        wings.realise_device(&release, &mut combat).unwrap();
        assert_eq!(combat.projectiles[0].target, None);
        assert!(combat.projectiles[0].guidance.is_none());
        // The physical body remains visible after a successful decoy.
        assert_eq!(
            combat
                .missile_snapshots(crate::combat::launcher(
                    &flight::State::new(&aircraft(), [0., 20000., 0.]).unwrap()
                ))
                .len(),
            1
        );
        wings.weapon_rules = Rules::Compatibility;
        let mut compatibility = Vec::new();
        wings.realise(&event, &mut compatibility, &weapon, 0, [0.; 3], None);
        assert!(compatibility[0].guidance.is_none());
    }

    #[test]
    fn live_devices_decoy_only_matching_missiles_targeting_the_releaser() {
        use tore_sim::ai::{
            mission::DeviceEvent,
            weapon_service::{RequestId, StationId},
        };
        let (mut wings, _) = build(None);
        let mut combat = combat_fixture(true);
        let mut weapon = combat.configuration().stations[0].weapon.clone();
        weapon.seeker.signature = 2;
        weapon.seeker.chaff_flare_chance = 100;
        wings.device_effectiveness.insert(3, (100, 100));
        for target in [3, 4] {
            wings.realise(
                &LaunchEvent {
                    actor: 1,
                    station: StationId(0),
                    target,
                    request_id: RequestId(u64::from(target)),
                    projectiles: 1,
                },
                &mut combat.projectiles,
                &weapon,
                0,
                [0.0; 3],
                None,
            );
        }
        let mut radar = combat.projectiles[0].clone();
        radar.weapon.as_mut().unwrap().seeker.signature = 3;
        combat.projectiles.push(radar);
        wings
            .realise_device(
                &DeviceEvent {
                    actor: 3,
                    class: SeekerClass::Infrared,
                    released: 1,
                },
                &mut combat,
            )
            .unwrap();
        assert_eq!(combat.projectiles[0].target, None);
        assert_eq!(combat.projectiles[1].target, Some(4));
        assert_eq!(combat.projectiles[2].target, Some(3));
        assert_eq!(combat.effects.last().unwrap().kind, live::EffectKind::Flare);
    }
    #[test]
    fn finite_missile_depletion_is_followed_by_actor_owned_gun_fire() {
        use tore_sim::ai::wing::{TargetOrder, WingRequest};
        let (mut wings, mut targets) = build(None);
        wings
            .mission
            .order(3, WingRequest::TargetAssignment(TargetOrder::FreeSelection))
            .unwrap()
            .unwrap();
        for actor in wings.mission.actors_mut() {
            actor.set_stations(Vec::new());
        }
        wings
            .mission
            .actor_mut(3)
            .unwrap()
            .set_stations(simple_stations(1, 20, AI_STORE_SPEED));
        let mut fixed: Vec<_> = wings
            .mission
            .actors()
            .iter()
            .map(|a| (a.id(), a.flight().clone()))
            .collect();
        for (id, flight) in &mut fixed {
            flight.position = [
                if *id == 2 || *id == 4 { 50000.0 } else { 0.0 },
                20000.0,
                if *id >= 3 { 2000.0 } else { 0.0 },
            ];
        }
        let missile = combat_fixture(true).configuration().stations[0]
            .weapon
            .clone();
        let gun = combat_fixture(false).configuration().stations[0]
            .weapon
            .clone();
        let mut realised = Vec::new();
        let mut stations = Vec::new();
        for _ in 0..10000 {
            for (id, flight) in &fixed {
                *wings.mission.actor_mut(*id).unwrap().flight_mut() = flight.clone();
            }
            let output = wings
                .advance(player_object([0.0, 20000.0, 0.0]), &mut targets, &flat)
                .unwrap();
            for event in output.launches.iter().filter(|e| e.actor == 3) {
                stations.push(event.station.0);
                let weapon = if event.station.0 == 0 { &missile } else { &gun };
                wings.realise(
                    event,
                    &mut realised,
                    weapon,
                    usize::from(event.station.0),
                    [0.0, 20000.0, 0.0],
                    None,
                );
            }
            if wings.mission.actor(3).unwrap().rounds_remaining() == 0 {
                break;
            }
        }
        assert_eq!(stations, [0, 1, 1]);
        assert_eq!(realised.len(), 21);
        assert!(
            realised[1..]
                .iter()
                .all(|p| p.weapon.as_ref() == Some(&gun) && p.target.is_none())
        );
        assert_eq!(wings.mission.actor(3).unwrap().rounds_remaining(), 0);
    }
    #[test]
    fn targetless_protect_me_releases_only_the_addressed_wingman() {
        use tore_sim::ai::{
            engagement::{Role, Stance},
            wing::PlayerOrder,
        };
        let mut selections = payload(None);
        selections[0].wing.index = 0;
        let mut wings =
            AiWings::build_with(&selections, &spawned(), 0, |_| Ok((aircraft(), None))).unwrap();
        let report = wings
            .command(PlayerOrder::ProtectMe, None, Some(1))
            .unwrap();
        assert!(report.message.contains("1 applied"));
        assert!(!wings.mission.actor(1).unwrap().is_neutral());
        assert!(wings.mission.actor(2).unwrap().is_neutral());
        assert!(wings.mission.actor(3).unwrap().is_neutral());
        let assignment = wings.mission.actor(1).unwrap().assignment();
        assert_eq!(assignment.role, Role::Escort);
        assert_eq!(assignment.stance, Stance::ProtectAssigned);
        assert_eq!(assignment.protected_ids, [PLAYER_ID]);
    }

    #[test]
    fn attack_on_contact_releases_after_recall_but_rejected_engage_does_not() {
        use tore_sim::ai::engagement::Assignment;
        use tore_sim::ai::wing::{Formation, PlayerOrder};
        let mut selections = payload(None);
        selections[0].wing.index = 0;
        let mut wings =
            AiWings::build_with(&selections, &spawned(), 0, |_| Ok((aircraft(), None))).unwrap();
        let rejected = wings
            .command(PlayerOrder::EngageMyTarget, Some(999), Some(1))
            .unwrap();
        assert!(rejected.message.contains("no valid hostile target"));
        assert!(wings.mission.actor(1).unwrap().is_neutral());
        wings
            .command(PlayerOrder::AttackOnContact, None, Some(1))
            .unwrap();
        assert!(!wings.mission.actor(1).unwrap().is_neutral());
        assert_eq!(
            wings.mission.actor(1).unwrap().assignment(),
            &Assignment::default()
        );
        wings
            .command(PlayerOrder::Formation(Formation::Echelon), None, Some(1))
            .unwrap();
        assert!(wings.mission.actor(1).unwrap().is_neutral());
        wings
            .command(PlayerOrder::AttackOnContact, None, Some(1))
            .unwrap();
        assert!(!wings.mission.actor(1).unwrap().is_neutral());
        assert!(wings.mission.actor(2).unwrap().is_neutral());
        assert!(wings.mission.actor(3).unwrap().is_neutral());
    }

    #[test]
    fn player_commands_report_acceptance_cancel_and_stay_in_the_addressed_wing() {
        use tore_sim::ai::wing::{PlayerApproach, PlayerBreak, PlayerOrder as O};
        let mut selections = payload(None);
        selections[0].wing.index = 0;
        let mut bridge =
            AiWings::build_with(&selections, &spawned(), 0, |_| Ok((aircraft(), None))).unwrap();
        let before = bridge.mission.actor(1).unwrap().flight().clone();
        let report = bridge.command(O::EngageMyTarget, Some(3), Some(1)).unwrap();
        assert!(report.message.contains("1 applied"));
        assert_eq!(report.radio, ["^ATTACK", "^ENGAGE"]);
        assert_eq!(
            bridge.mission.actor(1).unwrap().controller().target(),
            Some(3)
        );
        assert_eq!(bridge.mission.actor(2).unwrap().controller().target(), None);
        assert_eq!(
            *bridge.mission.actor(1).unwrap().flight(),
            before,
            "delivery cannot move an aircraft"
        );
        assert!(
            bridge
                .command(O::EngageMyTarget, Some(2), None)
                .unwrap()
                .radio
                .is_empty()
        );
        let report = bridge.command(O::EngageMyTarget, Some(3), Some(2)).unwrap();
        assert_eq!(
            report.radio,
            ["^ATTACK"],
            "only the first living wingman replies"
        );
        let report = bridge
            .command(O::Approach(PlayerApproach::Left), Some(3), Some(1))
            .unwrap();
        assert_eq!(report.radio, ["^APPRCLF"]);
        let report = bridge
            .command(O::Break(PlayerBreak::Right), None, Some(1))
            .unwrap();
        assert_eq!(report.radio, ["^BREAKRT"]);
        assert!(report.message.contains("1 applied"));
        bridge.command(O::Spacing, None, None).unwrap();
        for id in [1, 2] {
            assert_eq!(
                bridge
                    .mission
                    .actor(id)
                    .unwrap()
                    .controller()
                    .wing_settings()
                    .1,
                Some(2048)
            );
        }
        bridge.command(O::Stacking, None, Some(2)).unwrap();
        assert_eq!(
            bridge
                .mission
                .actor(2)
                .unwrap()
                .controller()
                .wing_settings()
                .2,
            Some(512)
        );
        assert_eq!(
            bridge
                .mission
                .actor(1)
                .unwrap()
                .controller()
                .wing_settings()
                .2,
            None
        );
        let report = bridge.command(O::Disengage, None, None).unwrap();
        assert_eq!(report.radio, ["^DISENG"]);
        assert_eq!(bridge.mission.actor(1).unwrap().controller().target(), None);
        assert_eq!(bridge.mission.actor(2).unwrap().controller().target(), None);
        assert_eq!(
            bridge
                .mission
                .actor(3)
                .unwrap()
                .controller()
                .wing_settings(),
            (None, None, None)
        );
        use tore_sim::ai::wing::{TargetId, TargetOrder, WingRequest};
        bridge
            .mission
            .order(
                3,
                WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(PLAYER_ID))),
            )
            .unwrap()
            .unwrap();
        let protected = bridge.command(O::ProtectMe, None, None).unwrap();
        assert_eq!(protected.radio, ["^CLRMY6", "^SHWTIME"]);
        assert_eq!(
            bridge.mission.actor(1).unwrap().assignment().protected_ids,
            [PLAYER_ID]
        );
        bridge.mission.actor_mut(1).unwrap().set_alive(false);
        assert_eq!(
            bridge
                .command(O::EngageMyTarget, Some(3), None)
                .unwrap()
                .radio,
            ["^ATTACK", "^ENGAGE"]
        );
    }
    #[test]
    fn unobserved_target_is_rejected_without_reply_or_control_changes() {
        use tore_sim::ai::wing::PlayerOrder;
        let mut selections = payload(None);
        selections[0].wing.index = 0;
        let sensors = sensors::SensorProfiles {
            aircraft: AircraftId::F18,
            radar: None,
            infrared: None,
            visual: None,
            jammer: None,
            signature: sensors::SignatureProfile::default(),
        };
        let mut bridge = AiWings::build_with(&selections, &spawned(), 0, |_| {
            Ok((aircraft(), Some(sensors.clone())))
        })
        .unwrap();
        let report = bridge
            .command(PlayerOrder::EngageMyTarget, Some(3), None)
            .unwrap();
        assert!(report.message.contains("0 applied, 2 rejected"));
        assert_eq!(report.radio, ["^ATTACK"]);
        for id in [1, 2] {
            assert!(
                bridge
                    .mission
                    .actor(id)
                    .unwrap()
                    .controller()
                    .target()
                    .is_none()
            );
            assert_eq!(
                bridge
                    .mission
                    .actor(id)
                    .unwrap()
                    .controller()
                    .wing_settings(),
                (None, None, None)
            );
        }
    }
}
