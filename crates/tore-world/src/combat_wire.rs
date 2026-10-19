//! The part of every seat's network picture that does not depend on the seat
//! (optimization slice S11): built once per tick however many seats are due,
//! instead of one whole [`RenderSnapshot`](crate::snapshot::RenderSnapshot)
//! per seat.
//!
//! A seat's picture ([`Combat::snapshot`]) differs from another seat's only in
//! its own plane: the player pose, the debris and second crew member of its
//! own plane, and its own plane left out of the other human-flown planes.
//! [`WirePicture`] is the picture as a seat that flies none of the human-flown
//! planes would draw it, holding only what the network reads (no stores, gun
//! rounds, effects, marks or models); `tore-session` adds each seat's own
//! plane back.

use super::{Combat, live};
use crate::snapshot::{AircraftPose, Damage, DebrisPose, Draw, Engine, PilotPose, ProjectilePose};
use crate::surface::SurfacePose;

/// One ejected pilot of a [`WirePicture`], with where the picture took it
/// from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WirePilot {
    pub pose: PilotPose,
    /// Taken from a human-flown plane's last pose; a seat flying that plane
    /// takes its own plane's second crew member from its cockpit instead.
    pub human: bool,
}

/// The seat-independent part of the seats' pictures at one tick: see the
/// module documentation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WirePicture {
    pub tick: u64,
    /// Combat's targets in target order, then every human-flown plane with
    /// a pose in aircraft id order, as [`Combat::snapshot`] draws them
    /// (stores left empty).
    pub aircraft: Vec<AircraftPose>,
    /// The missiles, bombs and rockets in flight (no gun rounds), in
    /// combat's order; `tracer`, `incoming`, `speed_f8` and `powered` are
    /// left unset.
    pub rounds: Vec<ProjectilePose>,
    /// Every debris piece, drawn and varied as for a seat that flies none of
    /// the human-flown planes.
    pub debris: Vec<DebrisPose>,
    /// Every ejected pilot: the pilots of the human-flown planes, the AI's,
    /// then the second crew members in the same order.
    pub pilots: Vec<WirePilot>,
    pub surface: Vec<SurfacePose>,
}

impl Combat {
    /// The seat-independent part of the seats' pictures now, `wings`
    /// supplying the AI aircraft's devices and ejected pilots. `None` while
    /// combat draws every target over the player's own state (a fixture
    /// mission with no target models), where each seat's targets differ.
    pub fn wire_picture(&self, wings: Option<&crate::ai_wings::AiWings>) -> Option<WirePicture> {
        if self.dummies.is_empty() {
            return None;
        }
        // Every human-flown plane with a pose, in aircraft id order.
        let humans: Vec<(&live::Ownship, &super::Pose)> = self
            .state
            .ownships()
            .iter()
            .filter_map(|own| Some((own, self.poses.get(&own.aircraft)?)))
            .collect();
        let human = |id: u32| humans.iter().find(|(own, _)| own.aircraft == id);
        let model = |id: u32| {
            id.checked_sub(self.first_row())
                .and_then(|index| self.dummies.get(index as usize))
                .map(|(model, _)| self.dummy_types[*model].profile.id)
        };
        let draw = |id: u32| match human(id) {
            Some((own, _)) => Draw::Model(own.configuration().aircraft),
            None => model(id).map_or(Draw::Hidden, Draw::Model),
        };
        // What each AI actor adds to its aircraft's pose, as the snapshot
        // finds it.
        struct Actor {
            id: u32,
            devices: Option<[f64; crate::snapshot::DEVICES]>,
            rotor: Option<(f64, f64, [[f64; 2]; 2])>,
            burning: bool,
        }
        let mut actors: Vec<Actor> = Vec::new();
        if let Some(wings) = wings {
            for actor in wings.mission().actors() {
                let alive = actor.alive();
                let flight = actor.flight();
                actors.push(Actor {
                    id: actor.id(),
                    devices: alive.then(|| crate::snapshot::devices(flight)),
                    rotor: if alive {
                        flight.rotor_speed_percent().map(|percent| {
                            let (turns, tilt) = crate::snapshot::rotor_pose(flight);
                            (percent / 100., turns, tilt)
                        })
                    } else {
                        None
                    },
                    burning: alive && flight.afterburner_active(),
                });
            }
            actors.sort_by_key(|actor| actor.id);
        }
        let actor_of = |id: u32| {
            actors
                .binary_search_by_key(&id, |actor| actor.id)
                .ok()
                .map(|index| &actors[index])
        };
        let gun_devices = |mut devices: [f64; crate::snapshot::DEVICES], own: &live::Ownship| {
            if let Some(gunship) = &own.gunship {
                devices[crate::snapshot::GUN_AIM..crate::snapshot::GUN_GROUP]
                    .copy_from_slice(&gunship.normalized_devices());
                devices[crate::snapshot::GUN_GROUP] = f64::from(gunship.mask());
            }
            devices
        };
        let model_engine = Engine {
            lit: true,
            afterburner: false,
            rates: [0.; 3],
            rotor: 0.,
            rotor_turns: 0.,
            rotor_tilt: [[0.; 2]; 2],
            flame: false,
        };
        let mut aircraft = Vec::with_capacity(self.state.targets.len() + humans.len());
        for t in &self.state.targets {
            let actor = actor_of(t.id);
            aircraft.push(AircraftPose {
                id: t.id,
                aircraft: t.aircraft,
                draw: draw(t.id),
                position: t.position,
                attitude: super::target_pose(t, self.ai_poses),
                velocity: t.velocity,
                devices: actor.and_then(|actor| actor.devices).or_else(|| {
                    self.render
                        .current_target(t.id)
                        .and_then(|pose| pose.devices)
                }),
                engine: Engine {
                    flame: t.airborne && t.hp > 0 && actor.is_some_and(|actor| actor.burning),
                    ..match actor.and_then(|actor| actor.rotor) {
                        Some((rotor, rotor_turns, rotor_tilt)) => Engine {
                            rotor,
                            rotor_turns,
                            rotor_tilt,
                            ..model_engine
                        },
                        None => model_engine,
                    }
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
                stores: Vec::new(),
            });
        }
        for (own, pose) in &humans {
            aircraft.push(AircraftPose {
                id: own.aircraft,
                aircraft: Some(own.configuration().aircraft),
                draw: Draw::Model(own.configuration().aircraft),
                position: pose.position,
                attitude: pose.attitude,
                velocity: pose.velocity,
                devices: Some(gun_devices(pose.devices, own)),
                engine: Engine {
                    flame: pose.engine.flame && own.hp > 0,
                    ..pose.engine
                },
                damage: Damage {
                    hp: own.hp,
                    initial_hp: own.configuration().damage_capacity,
                    sections: own.damage_amounts(),
                    structural: own.damage_section(),
                },
                airborne: true,
                wreck: pose.wreck,
                crashed: pose.crashed,
                stores: Vec::new(),
            });
        }
        let rounds = self
            .state
            .projectiles
            .iter()
            .filter_map(|p| {
                let weapon = self.state.weapon(p);
                (!live::is_gun(weapon)).then(|| ProjectilePose {
                    id: p.id,
                    owner: p.owner,
                    weapon: weapon.source.clone(),
                    shape: weapon.shape.clone(),
                    gun: false,
                    tracer: false,
                    position: p.position,
                    previous: p.previous,
                    direction: p.direction,
                    target: p.target,
                    incoming: false,
                    speed_f8: 0,
                    powered: false,
                })
            })
            .collect();
        // A piece of debris takes its owner's structural section from the
        // owner's first row, found by id as the snapshot finds it.
        let rows_by_id = (!self.state.debris.is_empty()).then(|| {
            let mut rows: Vec<(u32, usize)> = self
                .state
                .targets
                .iter()
                .enumerate()
                .map(|(index, target)| (target.id, index))
                .collect();
            rows.sort_unstable();
            rows
        });
        let first_row_of = |id: u32| {
            let rows = rows_by_id.as_deref()?;
            let at = rows.partition_point(|(row, _)| *row < id);
            rows.get(at)
                .filter(|(row, _)| *row == id)
                .map(|(_, index)| &self.state.targets[*index])
        };
        let debris = self
            .state
            .debris
            .iter()
            .map(|piece| DebrisPose {
                owner: piece.owner,
                draw: draw(piece.owner),
                position: piece.position,
                attitude: piece.basis.angles(),
                variant: if let Some((own, _)) = human(piece.owner) {
                    own.damage_section().map(|section| section as usize)
                } else if self.state.is_parked(piece.owner) {
                    Some(piece.variant)
                } else {
                    first_row_of(piece.owner)
                        .and_then(|target| target.localized_damage.structural_section)
                        .map(|section| section as usize)
                },
            })
            .collect();
        let pilot =
            |owner: u32, escape: &tore_sim::ejection::Escape, crew: bool, human: bool| WirePilot {
                pose: PilotPose {
                    owner,
                    position: escape.position,
                    heading: escape.heading,
                    phase: escape.phase,
                    crew,
                },
                human,
            };
        let pilots = humans
            .iter()
            .filter_map(|(own, pose)| {
                pose.escape
                    .as_ref()
                    .map(|escape| pilot(own.aircraft, escape, false, true))
            })
            .chain(
                wings
                    .into_iter()
                    .flat_map(|wings| wings.escapees())
                    .map(|(owner, escape)| pilot(owner, escape, false, false)),
            )
            .chain(humans.iter().filter_map(|(own, pose)| {
                pose.crew_escape
                    .as_ref()
                    .map(|escape| pilot(own.aircraft, escape, true, true))
            }))
            .chain(
                wings
                    .into_iter()
                    .flat_map(|wings| wings.crew_escapees())
                    .map(|(owner, escape)| pilot(owner, escape, true, false)),
            )
            .collect();
        Some(WirePicture {
            tick: self.state.tick(),
            aircraft,
            rounds,
            debris,
            pilots,
            surface: self.surface_poses(),
        })
    }
}
