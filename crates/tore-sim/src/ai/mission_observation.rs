//! Actor-local sensing. Its inputs deliberately exclude live actors and the
//! mission, so observation can be prepared independently of ordered decisions.
use super::{WorldObject, distance};
use crate::ai::Experience;
use crate::ai::awareness::ground::{
    Channels, FIGHTER_THREAT_RANGE_FT, FighterThreat, GroundPicture, GroundScene, GroundTrack,
    Refresh, Subjects, off_nose_deg,
};
use crate::ai::awareness::{self, Memory, Observation, ObservationSource, SourceTimestamps};
use crate::ai::controller::TargetView;
use crate::ai::engagement::{StrikeObjective, StrikeRole};
use crate::ai::targeting::Side;
use crate::attitude::Vector;
use crate::sensors::{self, GroundObject, Mode, Observable, Observer, Sensors, track};

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
                        // AI sensors never select the HARM page; its emitters would be
                        // radar returns.
                        sensors::Channel::Radar | sensors::Channel::Harm => {
                            ObservationSource::Radar
                        }
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
            link_track: false,
        }
    }
}

// ---------------------------------------------------------------------------
// The ground picture of a striker (docs/spec/ai-strike.md, "What the AI
// knows"). Air-to-air actors have none and nothing below runs for them.

impl super::AiActor {
    /// The actor's air-to-ground role: CAP unless a mission or a test set
    /// Strike.
    pub fn strike_role(&self) -> StrikeRole {
        self.ground.as_ref().map_or(StrikeRole::Cap, |g| g.role())
    }
    /// Gives the actor a role. Strike starts a ground picture (briefed by the
    /// host at its next refresh); CAP drops it unless a ground order stands.
    pub fn set_strike_role(&mut self, role: StrikeRole) {
        match &mut self.ground {
            Some(picture) => picture.set_role(role),
            None if role == StrikeRole::Strike => {
                self.ground = Some(Box::new(GroundPicture::new(role)));
            }
            None => {}
        }
        self.tidy_ground();
    }
    /// The ordered ground target, if any.
    pub fn strike_order(&self) -> Option<u32> {
        self.ground.as_ref().and_then(|g| g.ordered())
    }
    /// Orders a ground target (a flight lead's order, a taken-over
    /// designation), or clears it. An order gives even a CAP actor a ground
    /// picture while it stands.
    pub fn set_strike_order(&mut self, target: Option<u32>) {
        match &mut self.ground {
            Some(picture) => picture.set_ordered(target),
            None if target.is_some() => {
                let mut picture = GroundPicture::new(StrikeRole::Cap);
                picture.set_ordered(target);
                self.ground = Some(Box::new(picture));
            }
            None => {}
        }
        self.tidy_ground();
    }
    /// Why this actor takes no flight lead's order onto the ground now: the
    /// receiver's own gates (a training target, a wingman that bugged out,
    /// damage recovery, a plane parked or in an airfield sequence) and an
    /// ordered landing, which would send a striker straight home.
    pub fn ground_order_refusal(&self) -> Option<crate::ai::wing::RejectReason> {
        use crate::ai::wing::RejectReason;
        if self.dummy {
            Some(RejectReason::Dummy)
        } else if self.bugged_out {
            Some(RejectReason::BuggedOut)
        } else if self.damage_return.is_some() {
            Some(RejectReason::DamageRecovery)
        } else if self.airfield_phase() == Some(crate::ai::airfield::Phase::Parked) {
            Some(RejectReason::Landed)
        } else if self.airfield_phase().is_some() || self.landing_order.is_some() {
            Some(RejectReason::OnAirfield)
        } else {
            None
        }
    }
    /// A flight lead's order onto one ground object ("Engage my target",
    /// Alt+F, slice ST-O): the object alone, replacing any class or lase
    /// order, with a fresh strike behind it (a finished strike is not
    /// resumed).
    pub fn order_strike(&mut self, target: u32) {
        self.lase = None;
        self.strike = None;
        if let Some(picture) = &mut self.ground {
            picture.set_class_order(Vec::new());
        }
        self.set_strike_order(Some(target));
    }
    /// A flight lead's order onto every object of a class (Alt+W, slice
    /// ST-O): the objects are the targets, nearest first, two to an object.
    /// Replaces any other ground order. With no targets it only clears.
    pub fn order_strike_class(&mut self, targets: Vec<u32>) {
        self.lase = None;
        self.strike = None;
        self.set_strike_order(None);
        if targets.is_empty() {
            self.tidy_ground();
            return;
        }
        let picture = self
            .ground
            .get_or_insert_with(|| Box::new(GroundPicture::new(StrikeRole::Cap)));
        picture.set_class_order(targets);
    }
    /// A flight lead's "Lase my target" (slice ST-O): the object the wingman
    /// lases. Replaces any strike order. Slice ST-L reads it to fly the
    /// lasing orbit.
    pub fn order_lase(&mut self, target: u32) {
        self.clear_ground_orders();
        self.lase = Some(target);
    }
    /// The object this wingman was ordered to lase, while the order stands.
    pub fn lase_order(&self) -> Option<u32> {
        self.lase
    }
    /// Puts back the ground orders a flight lead had given, as they stood
    /// (slice ST-K: a human's take and give-back keeps them): the ordered
    /// target, a class order's targets and the lase order. Unlike the order
    /// calls above it clears nothing else, so the caller passes the whole
    /// set.
    pub fn restore_ground_orders(
        &mut self,
        order: Option<u32>,
        class_order: Vec<u32>,
        lase: Option<u32>,
    ) {
        self.set_strike_order(order);
        match &mut self.ground {
            Some(picture) => picture.set_class_order(class_order),
            None if !class_order.is_empty() => {
                let mut picture = GroundPicture::new(StrikeRole::Cap);
                picture.set_class_order(class_order);
                self.ground = Some(Box::new(picture));
            }
            None => {}
        }
        self.lase = lase;
        self.tidy_ground();
    }
    /// Ends every order a flight lead gave onto the ground (a strike, a class
    /// or a lase order), and the strike they started. The Strike role and its
    /// briefing, which a mission set, are left alone.
    pub fn clear_ground_orders(&mut self) {
        self.lase = None;
        if let Some(picture) = &mut self.ground {
            picture.set_ordered(None);
            picture.set_class_order(Vec::new());
        }
        self.tidy_ground();
    }
    /// Whether a flight lead's ground order stands (a strike, a class or a
    /// lase order).
    pub fn has_ground_order(&self) -> bool {
        self.lase.is_some()
            || self
                .ground
                .as_ref()
                .is_some_and(|g| g.ordered().is_some() || !g.class_order().is_empty())
    }
    /// Briefs a striker on its objective. Returns false, changing nothing,
    /// for an actor without a ground picture.
    pub fn set_strike_objective(&mut self, objective: StrikeObjective) -> bool {
        match &mut self.ground {
            Some(picture) => {
                picture.set_objective(objective);
                true
            }
            None => false,
        }
    }
    /// A Strike actor the host has not briefed yet.
    pub fn needs_briefing(&self) -> bool {
        self.ground
            .as_ref()
            .is_some_and(|g| g.role() == StrikeRole::Strike && g.objective().is_none())
    }
    /// The striker's ground picture; `None` for an air-to-air actor.
    pub fn ground_picture(&self) -> Option<&GroundPicture> {
        self.ground.as_deref()
    }
    fn tidy_ground(&mut self) {
        if self.ground.as_ref().is_some_and(|g| !g.active()) {
            self.ground = None;
        }
    }
    /// Whether this actor perceives this tick: alive, flying, with its pilot.
    fn perceives_ground(&self) -> bool {
        self.alive() && !self.dummy && self.flight.escape.is_none()
    }
}

impl super::AiMission {
    /// Whether any striker's ground picture refreshes at this tick, so the
    /// host knows to describe the ground objects.
    pub fn ground_due(&self) -> bool {
        self.actors.iter().any(|actor| {
            actor.perceives_ground()
                && actor
                    .ground
                    .as_ref()
                    .is_some_and(|g| g.due(self.tick).is_some())
        })
    }

    /// Refreshes every striker's ground picture that is due at this tick:
    /// a rebuild every 6 s, a follow every quarter second between
    /// (docs/spec/ai-strike.md, "What the AI knows"). `ground` is the
    /// terrain height for sight lines and the radar's look-down.
    pub fn observe_ground(
        &mut self,
        scene: GroundScene<'_>,
        ground: &(dyn Fn(f64, f64) -> f64 + Sync),
    ) {
        let tick = self.tick;
        let subjects = Subjects::new(scene.subjects);
        // Neutral scenery never enters a picture (opinionated, agent): the AI
        // attacks only sided objects, and scenery would fill the 50-entry
        // lists ahead of them.
        let objects: Vec<GroundObject> = scene
            .subjects
            .iter()
            .filter(|s| s.side != crate::combat::live::NO_SIDE)
            .map(|s| s.object.clone())
            .collect();
        let obscured = |from: Vector, to: Vector| {
            crate::combat::live::terrain_hit(from, to, &ground).is_some()
        };
        let environment = sensors::Environment {
            ground,
            obscured: &obscured,
        };
        for actor in &mut self.actors {
            if !actor.perceives_ground() {
                continue;
            }
            let Some(due) = actor.ground.as_ref().and_then(|g| g.due(tick)) else {
                continue;
            };
            let context = actor.observation_context();
            let observer = context.observer;
            let eye = |object: &GroundObject| {
                actor
                    .sensors
                    .as_ref()
                    .is_some_and(|s| eye_sees(s, &observer, object, &environment))
            };
            match due {
                Refresh::Rebuild => {
                    let tracks = actor.sensors.as_ref().map_or_else(Vec::new, |s| {
                        ground_tracks(
                            s,
                            &observer,
                            &objects,
                            &subjects,
                            &environment,
                            scene.weather,
                        )
                    });
                    let fighters = fighters(&actor.awareness, context.side, observer.position);
                    if let Some(picture) = actor.ground.as_mut() {
                        picture.rebuild(tick, tracks, &subjects, fighters);
                    }
                }
                Refresh::Follow => {
                    let visible = |object: &GroundObject| eye(object);
                    if let Some(picture) = actor.ground.as_mut() {
                        picture.follow(tick, observer.position, &subjects, &visible);
                    }
                }
            }
        }
    }
}

/// The eye's rule for a ground object (fitted, from the player's): a
/// standing, listable object (the type's listing flag, or a ship) on the
/// ground, inside the visual sensor's volume and range with a clear terrain
/// line, and no farther than its own visibility distance (`maxVisDist`) when
/// the type gives one.
fn eye_sees(
    sensors: &Sensors,
    observer: &Observer,
    object: &GroundObject,
    environment: &sensors::Environment<'_>,
) -> bool {
    if object.destroyed
        || object.airborne
        || !(object.listed || object.class & track::class::SHIP != 0)
    {
        return false;
    }
    let limit = f64::from(object.max_visible) * track::VISIBILITY_UNIT_FT;
    (object.max_visible == 0 || distance(observer.position, object.position) <= limit)
        && sensors.sees_ground(observer, object, environment)
}

/// One rebuild of a striker's lists: the A-G radar list (radar powered and
/// working), the FLIR list (infrared working) and the eye, by the player's
/// list rules, merged by object, nearest first (equal ranges by id). The
/// AI reads all three at once without switching its scope (opinionated,
/// agent).
fn ground_tracks(
    sensors: &Sensors,
    observer: &Observer,
    objects: &[GroundObject],
    subjects: &Subjects<'_>,
    environment: &sensors::Environment<'_>,
    weather: &sensors::Weather,
) -> Vec<GroundTrack> {
    let mut tracks: std::collections::BTreeMap<u32, GroundTrack> = Default::default();
    let mut add = |id: u32, distance_ft: f64, channel: sensors::Channel| {
        let Some(subject) = subjects.get(id) else {
            return;
        };
        let object = &subject.object;
        let track = tracks.entry(id).or_insert(GroundTrack {
            id,
            side: subject.side,
            class: object.class,
            position: object.position,
            velocity: object.velocity,
            distance_ft,
            channels: Channels::default(),
            aircraft: object.aircraft,
            emitting: object.emitting,
            destroyed: object.destroyed,
        });
        match channel {
            sensors::Channel::Radar | sensors::Channel::Harm => track.channels.radar = true,
            sensors::Channel::Infrared => track.channels.infrared = true,
            sensors::Channel::Visual => track.channels.visual = true,
        }
    };
    let pages = [
        (
            observer.radar_powered && !observer.radar_failed,
            Mode::AirGround,
        ),
        (!observer.infrared_failed, Mode::Infrared),
    ];
    for (working, page) in pages {
        if !working {
            continue;
        }
        for listed in sensors.ground_list(page, observer, objects, environment, weather) {
            add(
                listed.id(),
                listed.contact.distance_ft,
                listed.contact.channel,
            );
        }
    }
    let mut seen: Vec<(f64, u32)> = objects
        .iter()
        .filter(|o| eye_sees(sensors, observer, o, environment))
        .map(|o| (distance(observer.position, o.position), o.id))
        .collect();
    seen.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for (distance_ft, id) in seen.into_iter().take(track::MAX_GROUND_CONTACTS) {
        add(id, distance_ft, sensors::Channel::Visual);
    }
    let mut tracks: Vec<GroundTrack> = tracks.into_values().collect();
    tracks.sort_by(|a, b| {
        a.distance_ft
            .total_cmp(&b.distance_ft)
            .then(a.id.cmp(&b.id))
    });
    tracks
}

/// The hostile fighters of the striker's current air picture within 30 nm,
/// nearest first.
fn fighters(memory: &Memory, own_side: Side, own: Vector) -> Vec<FighterThreat> {
    let mut fighters: Vec<FighterThreat> = memory
        .current_observations()
        .map(|snapshot| snapshot.target)
        .filter(|t| t.valid && t.is_aircraft && t.is_fighter && t.side != own_side)
        .map(|t| FighterThreat {
            id: t.id,
            side: t.side,
            position: t.position,
            distance_ft: distance(own, t.position),
            off_nose_deg: off_nose_deg(t.position, t.heading_deg, own),
        })
        .filter(|f| f.distance_ft <= FIGHTER_THREAT_RANGE_FT)
        .collect();
    fighters.sort_by(|a, b| {
        a.distance_ft
            .total_cmp(&b.distance_ft)
            .then(a.id.cmp(&b.id))
    });
    fighters
}

#[cfg(test)]
#[path = "mission_ground_tests.rs"]
mod ground_tests;
