//! Actor-local sensing. Its inputs deliberately exclude live actors and the
//! mission, so observation can be prepared independently of ordered decisions.
use super::{WorldObject, distance};
use crate::ai::Experience;
use crate::ai::awareness::{self, Memory, Observation, ObservationSource, SourceTimestamps};
use crate::ai::controller::TargetView;
use crate::ai::targeting::Side;
use crate::sensors::{self, Observable, Observer, Sensors};

#[derive(Clone, Copy, Debug)]
pub(super) struct Context {
    pub actor: u32,
    pub side: Side,
    pub observer: Observer,
    pub selected: Option<u32>,
    pub experience: Experience,
    pub receive_emitters: bool,
    pub seeker_eligible: bool,
}

pub(super) struct Observed {
    pub targets: Vec<TargetView>,
    pub lookout: awareness::Lookout,
    pub emitters: Vec<sensors::passive::Emitter>,
    pub sources: Vec<(u32, SourceTimestamps)>,
}

/// Borrowed actor-local inputs captured before ordered actor decisions. Each
/// worker owns its resulting state; nothing is published until that actor
/// reaches the original observation call in its serial step.
pub(super) struct Input<'a> {
    pub index: usize,
    pub context: Context,
    pub sensors: Option<&'a Sensors>,
    pub memory: &'a Memory,
    pub use_surface: bool,
}

pub(super) struct Prepared {
    pub sensors: Option<Sensors>,
    pub memory: Memory,
    pub visual: Vec<awareness::VisualTrace>,
    pub observed: Observed,
    /// Decision-frame visibility only. Awareness keeps the original
    /// observation metadata, whose terrain_blocked field is still false.
    pub terrain_blocked: Vec<bool>,
}

impl Input<'_> {
    pub fn prepare(
        &self,
        tick: u64,
        world: &[WorldObject],
        terrain: &(dyn Fn(f64, f64) -> f64 + Sync),
        surface: &(dyn Fn(f64, f64) -> crate::research::Surface + Sync),
    ) -> Prepared {
        let runway_height = |x, z| surface(x, z).height;
        let ground: &dyn Fn(f64, f64) -> f64 = if self.use_surface {
            &runway_height
        } else {
            terrain
        };
        let mut sensors = self.sensors.cloned();
        let mut memory = self.memory.clone();
        // The serial actor preamble begins a new trace before observation.
        let mut visual = Vec::new();
        let observed = observe(
            self.context,
            &mut sensors,
            &mut memory,
            &mut visual,
            tick,
            world,
            ground,
        );
        let terrain_blocked = observed
            .targets
            .iter()
            .map(|target| {
                crate::combat::live::terrain_hit(
                    self.context.observer.position,
                    target.position,
                    &ground,
                )
                .is_some()
            })
            .collect();
        Prepared {
            sensors,
            memory,
            visual,
            observed,
            terrain_blocked,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn observe(
    context: Context,
    sensors: &mut Option<Sensors>,
    memory: &mut Memory,
    visual: &mut Vec<awareness::VisualTrace>,
    tick: u64,
    world: &[WorldObject],
    ground: &dyn Fn(f64, f64) -> f64,
) -> Observed {
    let live: Vec<u32> = world
        .iter()
        .filter(|o| o.alive && !o.destroyed)
        .map(|o| o.id)
        .collect();
    memory.prune_lifecycle(&live);
    let observer = context.observer;
    let attention = context
        .selected
        .and_then(|id| {
            memory
                .current_observations()
                .find(|s| s.target.id == id)
                .or_else(|| memory.snapshot(id))
        })
        .or_else(|| {
            memory
                .current_observations()
                .find(|s| s.target.side != context.side)
        })
        .map(|s| s.target.position);
    let lookout = awareness::Lookout::new(tick, observer.position, observer.basis, attention);
    let mut emitters = Vec::new();
    let contacts = if let Some(sensors) = sensors.as_mut() {
        let observables: Vec<Observable> = world
            .iter()
            .filter(|o| o.id != context.actor)
            .filter_map(|o| o.observable.clone())
            .collect();
        let obscured = |from, to| crate::combat::live::terrain_hit(from, to, &ground).is_some();
        let environment = sensors::Environment {
            ground,
            obscured: &obscured,
        };
        sensors.step(&observer, &observables, &environment);
        emitters = if context.receive_emitters {
            sensors::passive::emitters(&observer, &observables, sensors.contacts(), &environment)
        } else {
            Vec::new()
        };
        Some(sensors.contacts().to_vec())
    } else {
        None
    };
    let mut observations = Vec::new();
    // Aircraft on the ground are not air targets.
    for object in world.iter().filter(|o| {
        o.id != context.actor && o.alive && !o.destroyed && o.is_aircraft && !o.on_ground
    }) {
        if let Some(contacts) = &contacts {
            for contact in contacts
                .iter()
                .filter(|c| c.id == object.id && !c.destroyed)
            {
                if contact.channel == sensors::Channel::Visual
                    && lookout.check(
                        context.experience,
                        contact.position,
                        None,
                        crate::combat::live::terrain_hit(
                            observer.position,
                            contact.position,
                            &ground,
                        )
                        .is_none(),
                    ) != awareness::VisualResult::Visible
                {
                    continue;
                }
                observations.push(Observation {
                    target: context.target(object, contact.position, sensors),
                    velocity: contact.velocity,
                    source: match contact.channel {
                        sensors::Channel::Radar => ObservationSource::Radar,
                        sensors::Channel::Infrared => ObservationSource::Infrared,
                        sensors::Channel::Visual => ObservationSource::Visual,
                    },
                });
            }
            // Pilot attention has its own circular, skill-scaled cone.
            // Imported visual equipment remains unchanged for player use.
            // Cloud/night visibility is not supplied by this host yet;
            // the explicit None limit is the fitted clear-air assumption.
            if let Some(observable) = object.observable.as_ref()
                && !observable.destroyed
                && observable.airborne
            {
                let result = lookout.check(
                    context.experience,
                    observable.position,
                    None,
                    crate::combat::live::terrain_hit(
                        observer.position,
                        observable.position,
                        &ground,
                    )
                    .is_none(),
                );
                if visual.len() < 32 {
                    visual.push(awareness::VisualTrace {
                        id: object.id,
                        distance_ft: distance(observer.position, observable.position),
                        result,
                    });
                }
                if result == awareness::VisualResult::Visible {
                    observations.push(Observation {
                        target: context.target(object, observable.position, sensors),
                        velocity: observable.velocity,
                        source: ObservationSource::Visual,
                    });
                }
            }
        } else {
            // Explicit sensorless synthetic/replay fixtures supply the
            // whole permitted list. Production actor loading must never
            // select this path as a fallback after a sensor import error.
            observations.push(Observation {
                target: context.target(object, object.position, sensors),
                velocity: object.velocity,
                source: ObservationSource::Fixture,
            });
        }
    }
    memory.observe(tick, &observations);
    let sources = memory
        .current_observations()
        .take(32)
        .map(|s| (s.target.id, s.source_ticks))
        .collect();
    let targets = memory
        .current_observations()
        .map(|snapshot| snapshot.target)
        .collect();
    Observed {
        targets,
        lookout,
        emitters,
        sources,
    }
}

impl Context {
    fn target(
        self,
        object: &WorldObject,
        position: [f64; 3],
        sensors: &Option<Sensors>,
    ) -> TargetView {
        TargetView {
            id: object.id,
            side: object.side,
            position,
            heading_deg: object.heading_deg,
            pitch_deg: object.pitch_deg,
            speed: object.speed,
            maximum_speed: object.maximum_speed,
            is_aircraft: object.is_aircraft,
            is_fighter: object.is_fighter,
            human_controlled: object.human_controlled,
            valid: object.alive && !object.destroyed,
            type_allowed: true,
            seeker_eligible: self.seeker_eligible,
            wing_attackers: 0,
            terrain_blocked: false,
            sensor_supported: sensors.as_ref().is_none_or(|s| s.supports(object.id)),
        }
    }
}
