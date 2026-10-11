//! The bridge half of the strike tactics (docs/spec/ai-strike.md, slice
//! ST-T). The tactics live with each striker
//! ([`tore_sim::ai::strike::Strike`]); this module, before every mission step:
//!
//! - describes each striker's stores to it (what kind each is, its cadence
//!   and fire range), with its bombing class and whether it has a laser
//!   designator of its own;
//! - sets its strike cockpit to what the tactics selected (target, store,
//!   CCIP or LVL, the laser on by hand for a laser store), so the cockpit's
//!   sight, laser and solutions run as a human's do;
//! - works out the solution from that cockpit (the bomb or rocket cue's
//!   impact, the shared release test with the pass's aim and gate, the
//!   guided reach, a seeker's lock) and hands it to the striker.
//!
//! The striker's release then leaves through the shared release
//! ([`super::release`]) for bombs, rockets and guided bombs, and through
//! [`AiWings::release_seeker`] for its seeker missiles. With no striker it
//! costs one scan of the actors and changes nothing.
use super::AiWings;
use tore_sim::ai::strike::{self, Inputs, Solution, Store, StoreKind};
use tore_sim::ai::weapon_service::{Rounds, StationId};
use tore_sim::attitude::Vector;
use tore_sim::combat::delivery::Warning;
use tore_sim::combat::live::{self, ReleaseAim, StrikeTarget};
use tore_sim::combat::missiles::{self, Flight, LaunchMode, Motion, seeker};
use tore_sim::combat::{FallState, ballistics, guided, launch_speed};

/// The anti-radiation and radar seeker signatures (docs/formats/radar.md).
const ANTI_RADIATION: u8 = 4;
const RADAR: u8 = 3;

/// One step's strike work, before the mission steps.
pub(super) fn step(
    wings: &mut AiWings,
    state: &mut live::State,
    ground: &(dyn Fn(f64, f64) -> f64 + Sync),
) {
    let strikers: Vec<u32> = wings
        .mission
        .actors()
        .iter()
        .filter(|a| a.strikes() && a.airfield_phase().is_none())
        .map(|a| a.id())
        .collect();
    for id in strikers {
        let Some(config) = wings.configs.get(&id) else {
            continue;
        };
        let Some(actor) = wings.mission.actor(id) else {
            continue;
        };
        let stores = describe(config, actor.stations());
        let launcher = crate::combat::launcher(actor.flight());
        let selection = actor.strike().and_then(strike::Strike::selection);
        let side = state
            .targets
            .iter()
            .find(|t| t.id == id)
            .map_or(live::NO_SIDE, |t| t.side);
        let designator = config.sight_sensor().is_some_and(|s| s.laser);
        // A cockpit a human left keeps the human's bomb sight mode.
        let kept = state.kept(id).is_some();
        let solution = selection.and_then(|sel| {
            let station = usize::from(sel.station.0);
            let w = &config.stations.get(station)?.weapon;
            let cockpit = state.ai_cockpit_mut(id, side, config)?;
            cockpit.target = Some(StrikeTarget::Object(sel.target));
            cockpit.selected = station;
            cockpit.armed = true;
            // The AI never changes the mode of a cockpit a human left.
            if !kept {
                cockpit.delivery_mode = sel.mode;
            }
            if sel.laser && designator {
                cockpit.laser.manual = Some(true);
            }
            let row = state.targets.iter().find(|t| t.id == sel.target)?;
            let (position, velocity, alive) = (row.position, row.velocity, row.hp > 0);
            let emitting = row.radar_emitting;
            let guided_in_flight = state
                .projectiles
                .iter()
                .any(|p| p.owner == id && p.target == Some(sel.target) && p.guidance.is_some());
            let (impact, seconds, warning, due) = if sel.kind == StoreKind::Gun {
                // The gun's ground cue (slice ST-T2); the striker judges its
                // own bursts.
                let mount = config.stations[station].mount;
                let cue = tore_sim::combat::gunsight::ground_cue(w, &launcher, mount, ground)
                    .ok()
                    .flatten();
                (cue.map(|c| c.0), cue.map(|c| c.1), false, false)
            } else if sel.kind.seeker() {
                let due = seeker_due(
                    sel.kind,
                    w,
                    config.stations[station].mount,
                    &launcher,
                    position,
                    emitting,
                );
                (None, None, false, due)
            } else {
                let delivery = state.strike_delivery(id, config, &launcher, &ground);
                // The cue on the target's ground, as the release test judges
                // it (slice ST-LV).
                let cue = delivery
                    .as_ref()
                    .and_then(|d| d.impact_at(ground(position[0], position[2])));
                let warning = delivery.as_ref().is_some_and(|d| {
                    d.warnings
                        .iter()
                        .any(|w| matches!(w, Warning::Low | Warning::Frag))
                });
                let aim = ReleaseAim {
                    offset_ft: sel.aim_ft,
                    gate_ft: sel.gate_ft,
                };
                let due = state.strike_release_due(id, config, &launcher, aim, &ground);
                (
                    cue.map(|c| c.position),
                    cue.map(|c| c.seconds),
                    warning,
                    due,
                )
            };
            Some(Solution {
                station: sel.station,
                target: sel.target,
                target_position: position,
                target_velocity: velocity,
                target_alive: alive,
                impact,
                impact_seconds: seconds,
                due,
                warning,
                guided_in_flight,
            })
        });
        let inputs = Inputs {
            class: config.strike_class,
            stores,
            designator,
            mission_height_ft: wings.mission_height_ft,
            solution,
        };
        if let Some(actor) = wings.mission.actor_mut(id) {
            actor.set_strike_inputs(inputs);
        }
    }
}

/// What each of a striker's ground stores is to the tactics: its gun too
/// (slice ST-T2); air-to-air stores are left out.
fn describe(
    config: &live::Configuration,
    specs: &[tore_sim::ai::mission::StationSpec],
) -> Vec<Store> {
    config
        .stations
        .iter()
        .enumerate()
        .filter_map(|(index, station)| {
            let w = &station.weapon;
            let gun = live::is_gun(w);
            let kind = if gun { StoreKind::Gun } else { kind(w)? };
            let spec = specs.iter().find(|s| usize::from(s.station.0) == index)?;
            let rounds = match spec.store.rounds {
                Rounds::Finite(n) => n,
                Rounds::Unlimited => u32::MAX,
            };
            let zone = w.seeker.zones[1];
            let period =
                u64::from(w.burst.game_burst_t.max(1)) * tore_sim::ai::QUARTER_SECOND_TICKS;
            // A gun fires its rounds evenly through the record's period, the
            // AI gun employment's cadence (docs/spec/ai-gun-employment.md).
            let cadence_ticks = if gun {
                let rounds = u64::from(w.burst.actual_rounds_per_game.max(1))
                    * u64::from(w.burst.game_rounds_in_burst.max(1));
                (period / rounds).max(1)
            } else {
                period
            };
            Some(Store {
                station: StationId(index as u8),
                kind,
                rounds: if spec.store.inhibited { 0 } else { rounds },
                cadence_ticks,
                fire_range_ft: strike::GUIDED_CLOSE_FT.max(f64::from(zone.maximum_range) / 2.),
                minimum_range_ft: f64::from(zone.minimum_range.max(0)),
                maximum_range_ft: f64::from(zone.maximum_range.max(0)),
                round_speed_fps: (kind == StoreKind::Rocket)
                    .then(|| f64::from(w.movement.maximum_speed)),
                release_speed_share: match w.movement.launch_retard {
                    0 => 1.,
                    retard => f64::from(retard) / 100.,
                },
                fuze_arm_s: f64::from(w.damage.fuze_arm_t) / 4.,
                frag_radius_ft: f64::from(w.damage.collateral_radius.max(0)),
            })
        })
        .collect()
}

/// A store's kind, or `None` for a gun or an air-to-air store.
pub(super) fn kind(w: &tore_formats::weapons::Weapon) -> Option<StoreKind> {
    if live::is_gun(w) || w.flags & 0x20000 == 0 {
        return None;
    }
    if let Some(guidance) = guided::kind(w) {
        return Some(match guidance {
            missiles::Guidance::Laser => StoreKind::Laser,
            missiles::Guidance::Coordinate => StoreKind::Coordinate,
            _ => StoreKind::Command,
        });
    }
    if ballistics::is_bomb(w) {
        return Some(StoreKind::Bomb);
    }
    if ballistics::is_pod(w) && w.seeker.signature == 0 {
        return Some(StoreKind::Rocket);
    }
    match w.seeker.signature {
        0 => None,
        ANTI_RADIATION => Some(StoreKind::AntiRadiation),
        RADAR => Some(StoreKind::RadarAgm),
        _ => Some(StoreKind::Seeker),
    }
}

/// A seeker missile's lock on the target (docs/formats/weapons.md, "AI
/// release against ground targets"): inside zone 1 from its mount, within
/// the AI's fire range (10,000 ft or half the zone-1 maximum, whichever is
/// farther), and for an anti-radiation missile a target that emits.
fn seeker_due(
    kind: StoreKind,
    w: &tore_formats::weapons::Weapon,
    mount: Vector,
    launcher: &live::Launcher,
    target: Vector,
    emitting: bool,
) -> bool {
    let zone = w.seeker.zones[1];
    let basis = launcher.basis;
    let origin: Vector = std::array::from_fn(|i| {
        launcher.position[i]
            + basis.right[i] * mount[0]
            + basis.up[i] * mount[1]
            + basis.forward[i] * mount[2]
    });
    let range = missiles::length(missiles::sub(target, origin));
    let fire_range = strike::GUIDED_CLOSE_FT.max(f64::from(zone.maximum_range) / 2.);
    range <= fire_range
        && missiles::geometry(&zone, origin, basis, target, None)
        && (kind != StoreKind::AntiRadiation || emitting)
}

impl AiWings {
    /// The mission's airborne height, feet (the Quick Mission altitude
    /// setting), for a heavy striker's release floor; the host sets it
    /// before every step.
    pub fn set_mission_height(&mut self, height_ft: Option<f64>) {
        self.mission_height_ft = height_ft;
    }

    /// Whether `event` is a striker's gun burst at its strike target (a gun
    /// run, slice ST-T2), which [`Self::release_strafe`] makes.
    pub(super) fn striker_gun(&self, event: &tore_sim::ai::mission::LaunchEvent) -> bool {
        self.mission
            .actor(event.actor)
            .and_then(|a| a.strike())
            .and_then(strike::Strike::selection)
            .is_some_and(|sel| {
                sel.kind == StoreKind::Gun
                    && sel.station == event.station
                    && sel.target == event.target
            })
    }

    /// One round of a striker's gun run (slice ST-T2): fired along the
    /// barrel from the gun's mount as every AI gun round is (the round, its
    /// tracer and its debit as [`Self::realise`] makes them), with no air
    /// target to track: the striker aimed the gun's ground cue itself.
    /// Returns how many it made (0 or 1).
    pub(super) fn release_strafe(
        &mut self,
        event: &tore_sim::ai::mission::LaunchEvent,
        projectiles: &mut Vec<live::Projectile>,
        weapon: &tore_formats::weapons::Weapon,
        ordinal: u64,
    ) -> u32 {
        let Some(actor) = self.mission.actor(event.actor) else {
            return 0;
        };
        if !actor.alive() || projectiles.len() >= live::MAX_PROJECTILES {
            self.dropped_launches += 1;
            return 0;
        }
        let flight = actor.flight();
        let basis = tore_sim::attitude::Basis::new(flight.yaw, flight.pitch, flight.bank);
        let mount = actor
            .stations()
            .iter()
            .find(|s| s.station == event.station)
            .map_or([0.; 3], |s| s.mount);
        let origin: Vector = std::array::from_fn(|i| {
            flight.position[i]
                + basis.right[i] * mount[0]
                + basis.up[i] * mount[1]
                + basis.forward[i] * mount[2]
        });
        let Ok(speed) = launch_speed(&weapon.movement, (flight.speed * 256.) as i32) else {
            self.dropped_launches += 1;
            return 0;
        };
        let launched = (self.mission.tick() / 30) as u16;
        if !self
            .mission
            .actor_mut(event.actor)
            .is_some_and(|a| a.debit_gun_round(event.station))
        {
            return 0;
        }
        let id = self.next_projectile_id;
        self.next_projectile_id = self.next_projectile_id.wrapping_add(1);
        projectiles.push(live::Projectile {
            id,
            owner: event.actor,
            weapon: Some(weapon.clone()),
            guidance: None,
            motion: None,
            guidance_ticks: None,
            age: 0,
            incoming: None,
            station: usize::from(event.station.0),
            position: origin,
            previous: origin,
            direction: basis.forward,
            speed_f8: speed * 256,
            launched_t: launched,
            target: None,
            fall: FallState::default(),
            gun_round: Some(
                (ordinal % u64::from(weapon.burst.actual_rounds_per_game.max(1))) as u8,
            ),
            tracer: ordinal.is_multiple_of(3),
        });
        self.ai_shots.insert(id, event.actor);
        self.realised_launches += 1;
        1
    }

    /// Forces AI aircraft `id`'s pass and store (the headless driver's
    /// coverage runs, docs/testing/lane-flight.md). Returns false for an id
    /// that is not a striker.
    pub fn force_strike(
        &mut self,
        id: u32,
        pass: Option<strike::Pass>,
        station: Option<usize>,
    ) -> bool {
        match self.mission.actor_mut(id) {
            Some(actor) if actor.strikes() => {
                actor.force_strike(pass, station.map(|s| StationId(s as u8)));
                true
            }
            _ => false,
        }
    }

    /// Whether `event` is a striker's seeker missile at its strike target,
    /// which [`Self::release_seeker`] makes.
    pub(super) fn striker_seeker(
        &self,
        event: &tore_sim::ai::mission::LaunchEvent,
        weapon: &tore_formats::weapons::Weapon,
    ) -> bool {
        kind(weapon).is_some_and(StoreKind::seeker)
            && self
                .mission
                .actor(event.actor)
                .and_then(|a| a.strike())
                .and_then(|s| s.target())
                == Some(event.target)
    }

    /// A striker's seeker missile (Maverick, radar AGM, HARM) at its ground
    /// target: the AI's own missile release, as [`Self::realise`] makes it,
    /// with the launch observation taken from the target as the striker's
    /// cockpit holds it (its ground picture's target, which its seeker
    /// locked) rather than from the air picture. Returns how many it made.
    pub(super) fn release_seeker(
        &mut self,
        event: &tore_sim::ai::mission::LaunchEvent,
        state: &mut live::State,
        weapon: &tore_formats::weapons::Weapon,
        station: usize,
        humans: &[(u32, Vector)],
    ) -> u32 {
        let Some(actor) = self.mission.actor(event.actor) else {
            self.dropped_launches += event.projectiles;
            return 0;
        };
        let Some(row) = state.targets.iter().find(|t| t.id == event.target) else {
            self.dropped_launches += event.projectiles;
            return 0;
        };
        let (aim, velocity) = (row.position, row.velocity);
        let flight = actor.flight();
        let origin = flight.position;
        let direction = missiles::sub(aim, origin);
        let length = missiles::length(direction);
        if !length.is_finite() || length <= 0. {
            self.dropped_launches += event.projectiles;
            return 0;
        }
        let direction = direction.map(|v| v / length);
        let Ok(speed) = launch_speed(&weapon.movement, (flight.speed * 256.) as i32) else {
            self.dropped_launches += event.projectiles;
            return 0;
        };
        let profile = (self.weapon_rules == missiles::Rules::Spec)
            .then(|| missiles::Profile::for_weapon(weapon))
            .flatten();
        let guidance = profile.map(|profile| {
            Flight::from_supported_launch(
                profile,
                LaunchMode::Cued,
                seeker::Observation {
                    id: event.target,
                    position: aim,
                    velocity,
                    quality: 1.0,
                    off_axis: 0.0,
                    range: length,
                },
                origin,
            )
        });
        let launch_velocity = flight.velocity;
        let launched = (self.mission.tick() / 30) as u16;
        let incoming = humans
            .iter()
            .any(|(id, _)| *id == event.target)
            .then_some(event.target);
        let mut emitted = 0;
        for _ in 0..event.projectiles {
            if state.projectiles.len() >= live::MAX_PROJECTILES {
                self.dropped_launches += 1;
                continue;
            }
            let id = self.next_projectile_id;
            self.next_projectile_id = self.next_projectile_id.wrapping_add(1);
            state.projectiles.push(live::Projectile {
                id,
                owner: event.actor,
                weapon: Some(weapon.clone()),
                guidance: guidance.clone(),
                motion: profile.map(|_| Motion::launch(weapon, launch_velocity, origin[1])),
                guidance_ticks: profile.map(|p| p.guidance_ticks),
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
                gun_round: None,
                tracer: false,
            });
            self.ai_shots.insert(id, event.actor);
            self.realised_launches += 1;
            emitted += 1;
        }
        emitted
    }
}
