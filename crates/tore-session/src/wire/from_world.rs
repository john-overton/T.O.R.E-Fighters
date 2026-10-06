//! Filling the wire's plain data from `tore-world`'s: the entities from the
//! picture combat takes each tick, and the events from a tick's output.
//!
//! These are the host's conversions; which events go to which connection
//! (the seat's own cues, and the mission-wide ones to everyone) is the host
//! session's choice.

use super::WireResult;
use super::bits::{steps, turn16};
use super::entity::{
    AircraftState, DamageState, DebrisState, Devices, EngineState, Entity, EntityKind, EntityState,
    Motion, POSITION_STEP, PilotState, ProjectileState, RATE_STEP, SPEED_STEP, Status,
};
use super::events::{LinkEvent, Rumble, WireEvent};
use super::names::NameTable;
use super::priority::Relevance;
use tore_formats::aircraft::AircraftId;
use tore_sim::acoustics::Emission;
use tore_sim::combat::live::{DeviceRelease, EffectKind};
use tore_world::comms;
use tore_world::datalink::{DataLink, Entry};
use tore_world::seats::{PlaneId, SeatId};
use tore_world::snapshot::{
    AircraftPose, DebrisPose, Draw, EffectPose, MarkPose, PilotPose, ProjectilePose, RenderSnapshot,
};
use tore_world::world::{Cue, OrderReply, Release, World};

fn level(value: f64) -> u8 {
    if value.is_finite() {
        (value.clamp(0., 1.) * 255.).round() as u8
    } else {
        0
    }
}

fn surface(value: f64) -> i8 {
    if value.is_finite() {
        (value.clamp(-1., 1.) * 127.).round() as i8
    } else {
        0
    }
}

fn attitude(angles: [f64; 3]) -> [u16; 3] {
    angles.map(turn16)
}

fn position(p: [f64; 3]) -> [i64; 3] {
    p.map(|v| steps(v, POSITION_STEP))
}

/// An aircraft as drawn, quantized.
pub fn aircraft_state(pose: &AircraftPose) -> AircraftState {
    let devices = pose.devices.map(|d| Devices {
        levels: [d[0], d[1], d[2], d[3], d[4], d[5]].map(level),
        surfaces: [d[6], d[7], d[8]].map(surface),
        speed: steps(d[9], SPEED_STEP).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        throttle: level(d[10]),
    });
    let rate = |v: f64| steps(v, RATE_STEP).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    AircraftState {
        aircraft: pose.aircraft,
        motion: Motion::of(pose.position, pose.velocity),
        attitude: attitude(pose.attitude),
        devices,
        engine: EngineState {
            lit: pose.engine.lit,
            afterburner: pose.engine.afterburner,
            flame: pose.engine.flame,
            rates: pose.engine.rates.map(rate),
        },
        damage: DamageState {
            hp: pose.damage.hp,
            initial_hp: pose.damage.initial_hp,
            sections: pose.damage.sections,
            structural: pose.damage.structural,
        },
        status: Status {
            airborne: pose.airborne,
            crashed: pose.crashed,
            wreck: pose.wreck,
        },
    }
}

/// A projectile as drawn, quantized; `player` is the connection's plane, for
/// whether it is aimed at the player.
pub fn projectile_state(
    pose: &ProjectilePose,
    player: u32,
    names: &mut NameTable,
) -> WireResult<ProjectileState> {
    let velocity: [f64; 3] = std::array::from_fn(|i| {
        (pose.position[i] - pose.previous[i]) * tore_sim::flight::DT.recip()
    });
    let [x, y, z] = pose.direction;
    Ok(ProjectileState {
        owner: pose.owner,
        weapon: names.intern(&pose.weapon)?,
        shape: pose.shape.as_deref().map(|s| names.intern(s)).transpose()?,
        target: pose.target,
        aimed_at_player: pose.target == Some(player),
        motion: Motion::of(pose.position, velocity),
        direction: [turn16(x.atan2(z)), turn16(y.atan2(x.hypot(z)))],
    })
}

fn velocity_between(now: [f64; 3], then: Option<[f64; 3]>, ticks: u64) -> [f64; 3] {
    match then {
        Some(then) if ticks > 0 => {
            let seconds = ticks as f64 * tore_sim::flight::DT;
            std::array::from_fn(|i| (now[i] - then[i]) / seconds)
        }
        _ => [0.; 3],
    }
}

/// The picture the seat flying `plane` draws, built from `world` after a
/// step: its plane is the picture's player and every other plane, plane 0
/// and the other human-flown ones included, is a target
/// ([`tore_world::combat::Combat::snapshot`]). `None` when no cockpit flies
/// `plane`.
pub fn seat_picture(world: &World, plane: PlaneId) -> Option<RenderSnapshot> {
    let cockpit = world
        .cockpits
        .iter()
        .find(|cockpit| cockpit.plane == plane)?;
    world.combat.state.ownship(plane.0)?;
    Some(
        world
            .combat
            .snapshot(plane.0, &cockpit.flight, world.ai_wings.as_ref()),
    )
}

/// The plane an observer's entities are taken for (stage F phase 2): no
/// plane, so every aircraft and every ejected pilot is sent and no missile is
/// aimed at the observer.
pub const NO_PLANE: u32 = u32::MAX;

/// The picture an observer's stream is made from (stage F phase 2,
/// docs/ARCHITECTURE.md "The observer view"): every aircraft in the mission,
/// in real plane ids. While a human flies, it is the first such seat's
/// picture ([`seat_picture`]), whose player pose is an aircraft like any
/// other; with no human-flown plane it is built from combat's targets as
/// [`tore_world::combat::Combat::snapshot`] draws them, its player pose empty
/// (no aircraft). Only what [`entities`] reads is filled in: the aircraft,
/// projectiles, debris and pilots.
pub fn observer_picture(world: &World) -> RenderSnapshot {
    if let Some(picture) = world
        .cockpits
        .iter()
        .find_map(|cockpit| seat_picture(world, cockpit.plane))
    {
        return picture;
    }
    targets_picture(world)
}

/// [`observer_picture`] with no human-flown plane: combat's targets, the
/// projectiles, the debris and the AI's ejected pilots, as the combat
/// snapshot of a mission with drawn models gives them.
fn targets_picture(world: &World) -> RenderSnapshot {
    use tore_world::snapshot::{AircraftPose, Damage, Engine};
    let combat = &world.combat;
    let state = &combat.state;
    let actors = || {
        world
            .ai_wings
            .iter()
            .flat_map(|wings| wings.mission().actors())
            .filter(|actor| actor.alive())
    };
    let flying: std::collections::BTreeMap<u32, [f64; tore_world::snapshot::DEVICES]> = actors()
        .map(|actor| (actor.id(), tore_world::snapshot::devices(actor.flight())))
        .collect();
    let burning: std::collections::BTreeSet<u32> = actors()
        .filter(|actor| actor.flight().afterburner_active())
        .map(|actor| actor.id())
        .collect();
    RenderSnapshot {
        tick: state.tick(),
        player: AircraftPose::default(),
        targets: state
            .targets
            .iter()
            .map(|t| AircraftPose {
                id: t.id,
                aircraft: t.aircraft,
                draw: t.aircraft.map_or(Draw::Hidden, Draw::Model),
                position: t.position,
                attitude: tore_world::combat::target_pose(t, combat.ai_poses),
                velocity: t.velocity,
                devices: flying
                    .get(&t.id)
                    .copied()
                    .or_else(|| combat.current_target(t.id).and_then(|pose| pose.devices)),
                engine: Engine {
                    lit: true,
                    afterburner: false,
                    rates: [0.; 3],
                    flame: t.airborne && t.hp > 0 && burning.contains(&t.id),
                },
                damage: Damage {
                    hp: t.hp,
                    initial_hp: t.initial_hp,
                    sections: t.localized_damage.amounts,
                    structural: t.localized_damage.structural_section,
                },
                airborne: t.airborne,
                wreck: t.wreck.as_ref().map(|wreck| wreck.phase),
                crashed: t.hp <= 0,
            })
            .collect(),
        projectiles: state
            .projectiles
            .iter()
            .map(|p| {
                let weapon = state.weapon(p);
                ProjectilePose {
                    id: p.id,
                    owner: p.owner,
                    weapon: weapon.source.clone(),
                    shape: weapon.shape.clone(),
                    gun: tore_sim::combat::live::is_gun(weapon),
                    tracer: p.tracer,
                    position: p.position,
                    previous: p.previous,
                    direction: p.direction,
                    target: p.target,
                    incoming: p.incoming.is_some(),
                    speed_f8: p.speed_f8,
                }
            })
            .collect(),
        debris: state
            .debris
            .iter()
            .map(|piece| DebrisPose {
                owner: piece.owner,
                draw: Draw::Hidden,
                position: piece.position,
                attitude: piece.basis.angles(),
                variant: state
                    .targets
                    .iter()
                    .find(|target| target.id == piece.owner)
                    .and_then(|target| target.localized_damage.structural_section)
                    .map(|section| section as usize),
            })
            .collect(),
        pilots: world
            .ai_wings
            .iter()
            .flat_map(|wings| {
                [
                    wings
                        .escapees()
                        .map(|(owner, e)| (owner, false, e))
                        .collect::<Vec<_>>(),
                    wings
                        .crew_escapees()
                        .map(|(owner, e)| (owner, true, e))
                        .collect(),
                ]
                .concat()
            })
            .map(|(owner, crew, escape)| PilotPose {
                owner,
                position: escape.position,
                heading: escape.heading,
                phase: escape.phase,
                crew,
            })
            .collect(),
        ..RenderSnapshot::default()
    }
}

/// Every entity of `current` a client flying `player` draws: every aircraft
/// but its own (plane 0 included when another seat flies it), every missile,
/// bomb and rocket (gun rounds are burst events), every debris piece, and
/// every ejected pilot but its own (its escape is part of its plane's exact
/// state). `current` is the seat's picture ([`seat_picture`]); a picture
/// built for another plane serves too, since its player pose is taken as an
/// aircraft like any other. Debris and pilots take their velocity from
/// `previous`, the seat's picture a tick or more before. The entities come
/// in key order.
pub fn entities(
    current: &RenderSnapshot,
    previous: Option<&RenderSnapshot>,
    player: u32,
    names: &mut NameTable,
) -> WireResult<Vec<Entity>> {
    let mut out = Vec::new();
    let aircraft_of = |id: u32| -> Option<AircraftId> {
        std::iter::once(&current.player)
            .chain(&current.targets)
            .find(|pose| pose.id == id)
            .and_then(|pose| pose.aircraft)
    };
    for pose in std::iter::once(&current.player).chain(&current.targets) {
        if pose.aircraft.is_none() || pose.id == player {
            continue;
        }
        if out
            .iter()
            .any(|e: &Entity| e.id == pose.id && e.state.kind() == EntityKind::Aircraft)
        {
            continue;
        }
        out.push(Entity {
            id: pose.id,
            state: EntityState::Aircraft(aircraft_state(pose)),
        });
    }
    for pose in current.projectiles.iter().filter(|p| !p.gun) {
        out.push(Entity {
            id: pose.id,
            state: EntityState::Projectile(projectile_state(pose, player, names)?),
        });
    }
    let ticks = previous.map_or(0, |p| current.tick.saturating_sub(p.tick));
    for pose in &current.debris {
        let then = previous
            .and_then(|p| p.debris.iter().find(|d| d.owner == pose.owner))
            .map(|d| d.position);
        let model = match pose.draw {
            Draw::Model(id) => Some(id),
            Draw::Ownship | Draw::Hidden => aircraft_of(pose.owner),
        };
        if out
            .iter()
            .any(|e| e.id == pose.owner && e.state.kind() == EntityKind::Debris)
        {
            continue;
        }
        out.push(Entity {
            id: pose.owner,
            state: EntityState::Debris(debris_state(
                pose,
                model,
                velocity_between(pose.position, then, ticks),
            )),
        });
    }
    // The own pilot's escape is part of its plane's exact state, but the
    // second crew member of its plane is not: he is sent like any other.
    for pose in current
        .pilots
        .iter()
        .filter(|p| p.owner != player || p.crew)
    {
        let id = super::entity::pilot_id(pose.owner, pose.crew);
        let then = previous
            .and_then(|p| {
                p.pilots
                    .iter()
                    .find(|q| (q.owner, q.crew) == (pose.owner, pose.crew))
            })
            .map(|q| q.position);
        if out
            .iter()
            .any(|e| e.id == id && e.state.kind() == EntityKind::Pilot)
        {
            continue;
        }
        out.push(Entity {
            id,
            state: EntityState::Pilot(pilot_state(
                pose,
                velocity_between(pose.position, then, ticks),
            )),
        });
    }
    out.sort_by_key(Entity::key);
    Ok(out)
}

/// A debris piece, quantized.
pub fn debris_state(
    pose: &DebrisPose,
    model: Option<AircraftId>,
    velocity: [f64; 3],
) -> DebrisState {
    DebrisState {
        owner: pose.owner,
        model,
        variant: pose.variant.map(|v| v.min(7) as u8),
        motion: Motion::of(pose.position, velocity),
        attitude: attitude(pose.attitude),
    }
}

/// An ejected pilot, quantized.
pub fn pilot_state(pose: &PilotPose, velocity: [f64; 3]) -> PilotState {
    PilotState {
        owner: super::entity::pilot_id(pose.owner, pose.crew),
        motion: Motion::of(pose.position, velocity),
        heading: turn16(pose.heading),
        phase: pose.phase,
    }
}

/// The relevance a host can work out from positions alone: the distance
/// from the player's plane at `own` and whether a missile is aimed at
/// `player`. The host adds its own flight, the friendly sensors' tracks and
/// the view's subject.
pub fn distance_relevance(entity: &Entity, own: [f64; 3], player: u32) -> Relevance {
    let at = entity.state.motion().position_ft();
    let distance_ft = (0..3).map(|i| (at[i] - own[i]).powi(2)).sum::<f64>().sqrt();
    Relevance {
        distance_ft,
        aimed_at_player: matches!(entity.state, EntityState::Projectile(p) if p.target == Some(player)),
        ..Relevance::NEAR
    }
}

/// The event a cue makes for `seat`: the seat's own cues and the
/// mission-wide wing ejections; `None` for another seat's cues and for the
/// tick's markers, which only a local presenter uses.
pub fn cue_event(cue: &Cue, seat: SeatId, names: &mut NameTable) -> WireResult<Option<WireEvent>> {
    let mine = |s: &SeatId| *s == seat;
    Ok(match cue {
        Cue::Message { seat: s, text } if mine(s) => {
            Some(WireEvent::Message { text: text.clone() })
        }
        Cue::Feedback { seat: s, event } if mine(s) => Some(WireEvent::Feedback {
            rumble: Rumble::of(*event),
        }),
        Cue::Tower { seat: s, stem } if mine(s) => Some(WireEvent::Tower {
            stem: stem.map(|stem| names.intern(stem)).transpose()?,
        }),
        Cue::WeaponCycled { seat: s } if mine(s) => Some(WireEvent::WeaponCycled),
        Cue::WingEjection {
            id,
            message,
            friendly,
        } => Some(WireEvent::WingEjection {
            aircraft: *id,
            message: message.clone(),
            friendly: *friendly,
        }),
        Cue::Radio { seat: s, call } if mine(s) => Some(WireEvent::Radio {
            route: call.route,
            important: call.kind == comms::Kind::Important,
            net: call.net,
            label: call.label.clone(),
            text: call.text.clone(),
            stems: call
                .stems
                .iter()
                .take(super::limits::STEMS)
                .map(|stem| names.intern(stem))
                .collect::<WireResult<_>>()?,
        }),
        Cue::OrderVoice { seat: s, stems } if mine(s) => Some(WireEvent::OrderVoice {
            stems: stems
                .iter()
                .take(super::limits::STEMS)
                .map(|stem| names.intern(stem))
                .collect::<WireResult<_>>()?,
        }),
        _ => None,
    })
}

/// The data link's journal entries a seat flying `plane` is sent (slice G7):
/// those about a member of its own flight, in the journal's order. A plane
/// the picture does not know is sent none.
pub fn link_events(link: &DataLink, plane: u32, entries: &[Entry]) -> Vec<WireEvent> {
    let Some(flight) = link.member(plane).map(|m| m.flight) else {
        return Vec::new();
    };
    entries
        .iter()
        .map(LinkEvent::of)
        .filter(|event| {
            link.member(event.plane())
                .is_some_and(|member| member.flight == flight)
        })
        .map(WireEvent::Link)
        .collect()
}

/// A weapon release sound for its seat.
pub fn release_event(release: &Release, names: &mut NameTable) -> WireResult<WireEvent> {
    Ok(WireEvent::Release {
        sound: names.intern(&release.sound)?,
        station: release.station.min(255) as u8,
    })
}

/// What became of a wing order, for the seat that gave it.
pub fn order_event(reply: &OrderReply) -> WireEvent {
    WireEvent::OrderReply {
        order: reply.order,
        outcome: reply.outcome.clone(),
    }
}

/// A sound emission.
pub fn sound_event(emission: &Emission, from: Option<u32>) -> WireEvent {
    WireEvent::Sound {
        kind: emission.kind,
        position: position(emission.position),
        arrived: emission.arrived,
        from,
    }
}

/// A flash, hit or explosion as combat draws it.
pub fn effect_event(effect: &EffectPose) -> WireEvent {
    WireEvent::Effect {
        kind: effect.kind,
        position: position(effect.position),
        ticks: effect.ticks,
        blast: effect.blast,
    }
}

/// A crater or crash-site fire.
pub fn mark_event(mark: &MarkPose) -> WireEvent {
    WireEvent::Mark {
        kind: mark.kind,
        position: position(mark.position),
    }
}

/// Chaff or a flare leaving an aircraft.
pub fn countermeasure_event(release: &DeviceRelease) -> WireEvent {
    WireEvent::Countermeasure {
        aircraft: release.owner,
        flare: release.kind == EffectKind::Flare,
        position: position(release.release.position),
        velocity: release
            .release
            .velocity
            .map(|v| steps(v, super::entity::VELOCITY_STEP)),
        attitude: attitude(release.release.basis.angles()),
        number: release.number,
        left: release.left,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_world::mission::{MissionSpec, Skill, Start};
    use tore_world::seats::SeatInput;
    use tore_world::test_support::resources::{THEATER, resources};
    use tore_world::world::{MissionCommand, Seating, TickOutput};

    fn aircraft_ids(entities: &[Entity]) -> Vec<u32> {
        entities
            .iter()
            .filter(|e| e.state.kind() == EntityKind::Aircraft)
            .map(|e| e.id)
            .collect()
    }

    fn spec() -> MissionSpec {
        let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
        spec.wings[0].count = 2;
        spec.wings[3].count = 2;
        spec.wings[3].skill = Skill::Average;
        spec.separation_nm = 2;
        spec.start = Start::Airborne {
            altitude_ft: 10_000,
        };
        spec
    }

    /// Steps `world` 30 ticks, `takes` handing planes to seats on the first.
    fn fly(world: &mut World, takes: &[(u8, u32)], seats: &[u8]) {
        let mut out = TickOutput::default();
        let take: Vec<MissionCommand> = takes
            .iter()
            .map(|&(seat, plane)| MissionCommand::Take {
                seat: SeatId(seat),
                plane: PlaneId(plane),
            })
            .collect();
        for tick in 0..30 {
            let inputs: Vec<SeatInput> = seats
                .iter()
                .map(|&seat| SeatInput {
                    seat: SeatId(seat),
                    tick: world.tick(),
                    ..SeatInput::default()
                })
                .collect();
            let mission: &[MissionCommand] = if tick == 0 { &take } else { &[] };
            world
                .step_with(mission, &inputs, &mut out, |_, _| Ok(()))
                .unwrap();
        }
    }

    /// The aircraft the seat flying `plane` is sent.
    fn sent_to(world: &World, plane: u32) -> Vec<Entity> {
        let picture = seat_picture(world, PlaneId(plane)).unwrap();
        entities(&picture, None, plane, &mut NameTable::new()).unwrap()
    }

    fn check_plane_0_reaches(world: &World, plane: u32) {
        let lead = sent_to(world, plane)
            .into_iter()
            .find(|e| e.id == 0 && e.state.kind() == EntityKind::Aircraft)
            .expect("plane 0 is sent to the other seat");
        let cockpit = world
            .cockpits
            .iter()
            .find(|c| c.plane == PlaneId(0))
            .unwrap();
        assert_eq!(
            *lead.state.motion(),
            Motion::of(cockpit.flight.position, cockpit.flight.velocity)
        );
    }

    /// John, 2026-10-06: a two-seater's second crew member has a chute of his
    /// own. Every client gets it, the flying seat's own plane's too (the own
    /// pilot's chute is in the plane's exact state, his is not), under an id
    /// of its own, and a client reads the plane and the seat back.
    #[test]
    fn a_two_seaters_second_chute_has_an_id_of_its_own_and_reaches_every_client() {
        use tore_sim::ejection::Phase;
        let pose = |owner: u32, crew: bool, x: f64| PilotPose {
            owner,
            position: [x, 3000., 0.],
            heading: 0.5,
            phase: Phase::Seat,
            crew,
        };
        let current = RenderSnapshot {
            pilots: vec![
                pose(7, false, 0.),
                pose(9, false, 10.),
                pose(7, true, 20.),
                pose(9, true, 30.),
            ],
            ..RenderSnapshot::default()
        };
        let sent = entities(&current, None, 7, &mut NameTable::new()).unwrap();
        let pilots: Vec<(u32, u32)> = sent
            .iter()
            .filter_map(|e| match e.state {
                EntityState::Pilot(p) => Some((e.id, p.owner)),
                _ => None,
            })
            .collect();
        let crew = |owner| crate::wire::entity::pilot_id(owner, true);
        assert_eq!(
            pilots,
            [(9, 9), (crew(7), crew(7)), (crew(9), crew(9))],
            "the own pilot is in the exact state; the three others are sent, in id order"
        );
        assert!(crew(7) != 7 && crew(9) != 9);
        for (id, owner, seat) in [(9, 9, false), (crew(7), 7, true), (crew(9), 9, true)] {
            assert_eq!(crate::wire::entity::pilot_owner(id), (owner, seat));
        }
        // Other clients get the own plane's two chutes.
        let other = entities(&current, None, 3, &mut NameTable::new()).unwrap();
        assert_eq!(
            other
                .iter()
                .filter(|e| e.state.kind() == EntityKind::Pilot)
                .count(),
            4
        );
        // Through the wire coder both survive.
        let again: Vec<Entity> = other
            .iter()
            .filter(|e| e.state.kind() == EntityKind::Pilot)
            .copied()
            .collect();
        assert_eq!(again.len(), 4);
        assert!(again.windows(2).all(|w| w[0].key() < w[1].key()));
    }

    #[test]
    fn each_seat_gets_every_plane_but_its_own() {
        // Single player's mission with a second seat in plane 1.
        let map = resources();
        let mut world = World::new(&spec(), &map, Seating::SinglePlayer).unwrap();
        fly(&mut world, &[(1, 1)], &[0, 1]);
        assert!(
            seat_picture(&world, PlaneId(2)).is_none(),
            "the AI flies plane 2"
        );
        assert_eq!(aircraft_ids(&sent_to(&world, 0)), [1, 2, 3]);
        assert_eq!(aircraft_ids(&sent_to(&world, 1)), [0, 2, 3]);
        check_plane_0_reaches(&world, 1);

        // An open mission, as a server runs one: nobody at first, then two
        // seats in planes 0 and 1.
        let mut world = World::new(&spec(), &map, Seating::Open).unwrap();
        fly(&mut world, &[], &[]);
        assert!(seat_picture(&world, PlaneId(0)).is_none(), "no human yet");
        fly(&mut world, &[(1, 0), (2, 1)], &[1, 2]);
        assert_eq!(aircraft_ids(&sent_to(&world, 0)), [1, 2, 3]);
        assert_eq!(aircraft_ids(&sent_to(&world, 1)), [0, 2, 3]);
        check_plane_0_reaches(&world, 1);
    }

    /// What an observer is sent of `world`.
    fn observed(world: &World) -> Vec<Entity> {
        entities(
            &observer_picture(world),
            None,
            NO_PLANE,
            &mut NameTable::new(),
        )
        .unwrap()
    }

    #[test]
    fn an_observer_gets_every_plane_with_or_without_a_human() {
        // Nobody flies: the AI's aircraft from combat's targets.
        let map = resources();
        let mut world = World::new(&spec(), &map, Seating::Open).unwrap();
        fly(&mut world, &[], &[]);
        assert!(world.combat.state.ownships().is_empty());
        let alone = observed(&world);
        assert_eq!(aircraft_ids(&alone), [0, 1, 2, 3]);

        // Two humans, in planes 0 and 2: every plane, theirs included, at
        // their cockpits' positions, and the AI's as a seat sees them.
        fly(&mut world, &[(1, 0), (2, 2)], &[1, 2]);
        let all = observed(&world);
        assert_eq!(aircraft_ids(&all), [0, 1, 2, 3]);
        for plane in [0, 2] {
            let cockpit = world
                .cockpits
                .iter()
                .find(|c| c.plane == PlaneId(plane))
                .unwrap();
            let entity = all
                .iter()
                .find(|e| e.id == plane && e.state.kind() == EntityKind::Aircraft)
                .unwrap();
            assert_eq!(
                *entity.state.motion(),
                Motion::of(cockpit.flight.position, cockpit.flight.velocity)
            );
        }
        let seat = sent_to(&world, 0);
        for id in [1, 3] {
            let ai = |list: &[Entity]| {
                list.iter()
                    .find(|e| e.id == id && e.state.kind() == EntityKind::Aircraft)
                    .cloned()
            };
            assert_eq!(ai(&all), ai(&seat), "AI plane {id}");
            // Built from combat's targets alone, as with no human.
            let fallback = entities(
                &targets_picture(&world),
                None,
                NO_PLANE,
                &mut NameTable::new(),
            )
            .unwrap();
            assert_eq!(ai(&fallback), ai(&seat), "AI plane {id} from the targets");
        }
    }
}
