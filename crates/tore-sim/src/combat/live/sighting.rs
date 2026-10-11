//! One ownship's targeting sight and laser for a tick, and the laser spots
//! every aircraft's laser weapons may home on (docs/spec/air-to-ground.md,
//! "The targeting sight", "The laser" and "Buddy lasing"). The AC-130's
//! gunsight steps with its guns (`step_gunship`); every other aircraft with
//! a laser designator or a FLIR steps here.
use super::cockpit::{Cockpits, StrikeCockpit};
use super::{Launcher, NO_SIDE, Ownship, Projectile, Side, State, Target, projectile_weapon};
use crate::attitude::Vector;
use crate::combat::{
    laser::{self, LaserInputs, LaserSpot},
    sight::{self, SightObject, SightRequest},
};
use crate::sensors::Weather;
use std::collections::BTreeSet;

/// What the sight and the laser read of the state this tick.
pub(super) struct Context<'a, G> {
    pub ground: &'a G,
    pub weather: &'a Weather,
    /// The aircraft with one of their own laser weapons in flight.
    pub laser_owners: &'a BTreeSet<u32>,
    pub tick: u64,
}

/// The aircraft with a laser sight in their strike cockpit (the ownships,
/// the aircraft the AI flies with a human's sight kept, the AI strikers)
/// with one of their own laser weapons in flight: their lasers stay on under
/// the automatic rule.
pub(super) fn laser_owners(cockpits: Cockpits<'_>, projectiles: &[Projectile]) -> BTreeSet<u32> {
    let ships = cockpits.ships;
    let lasing: BTreeSet<u32> = cockpits
        .iter()
        .filter(|(_, _, cockpit)| cockpit.sight.is_some_and(|sight| sight.sensor.laser))
        .map(|(aircraft, _, _)| aircraft)
        .collect();
    if lasing.is_empty() {
        return lasing;
    }
    projectiles
        .iter()
        .filter(|p| p.gun_round.is_none() && lasing.contains(&p.owner))
        .filter(|p| laser::is_laser_weapon(projectile_weapon(ships, p)))
        .map(|p| p.owner)
        .collect()
}

impl Ownship {
    /// A sight key (Backslash, Shift+Backslash, `/`) for the next step, on
    /// the AC-130's gunsight or the targeting sight. Nothing without one.
    pub(super) fn request_sight(&mut self, request: SightRequest) {
        if let Some(group) = &mut self.gunship {
            group.request = Some(request);
        }
        if let Some(sight) = &mut self.cockpit.sight {
            sight.request = Some(request);
        }
    }
    /// A laser weapon is selected and armed.
    pub fn laser_weapon_selected(&self) -> bool {
        self.cockpit.laser_weapon_armed(&self.config)
    }
}

/// The targeting sight and the laser of one ownship (not the AC-130) for a
/// tick. The sight sees the objects on the ground (an aircraft's sight never
/// tracks a flying aircraft, so the air-to-air designation rules stay as
/// they are). A designated ground object becomes the sight's track, and the
/// sight's track designates its object while it holds it, so the HUD, the
/// scope, the sight and the weapons agree (as on the AC-130; opinionated,
/// agent, for the slaving of the sight to a radar or visual designation).
pub(super) fn step<G: Fn(f64, f64) -> f64>(
    own: &mut Ownship,
    launcher: Launcher,
    targets: &[Target],
    cx: &Context<'_, G>,
) {
    let Some(sight) = own.cockpit.sight.as_mut() else {
        return;
    };
    let objects: Vec<SightObject> = targets
        .iter()
        .filter(|t| !t.airborne)
        .map(|t| SightObject {
            id: t.id,
            position: t.position,
            velocity: t.velocity,
            alive: t.hp > 0,
            airborne: false,
            friendly: own.friendlies.contains(&t.id) || (t.side != NO_SIDE && t.side == own.side),
        })
        .collect();
    if let Some(id) = own.sensors.selected()
        && sight.target() != Some(id)
        && objects.iter().any(|o| o.id == id && o.alive)
    {
        sight.track(id);
        own.sensors.hold(id);
    }
    let before = sight.target();
    let range = sight.sensor.range_ft;
    sight.step(launcher, &objects, cx.ground, range, cx.tick);
    let after = sight.target();
    let sensor = sight.sensor;
    let eye = sight.eye_position(launcher);
    let held = sight.held_point().map(|point| (point, after));
    if after != before {
        match after {
            Some(id) => own.sensors.hold(id),
            None if own.sensors.selected() == before => own.sensors.clear_selection(),
            None => {}
        }
        own.hud_selection = own.designated();
    }
    let alive = launcher.alive && own.hp > 0;
    let in_gimbal = held.is_some_and(|(point, _)| {
        sensor
            .gimbal
            .follows(sight::body_angles(launcher, sub(point, eye)))
    });
    let weather = held.map_or(1., |(point, _)| {
        laser::weather_scale(cx.weather, eye[1], point[1])
    });
    let automatic = own.laser_weapon_selected() || cx.laser_owners.contains(&own.aircraft);
    own.cockpit.laser.step(
        LaserInputs {
            designator: (sensor.laser && alive).then_some(sensor.range_ft),
            held,
            eye,
            in_gimbal,
            automatic,
            weather,
        },
        |from, to| sight::clear_line(from, to, cx.ground),
    );
}

/// The launcher an AI-flown aircraft's row stands for, for a sight a human
/// left in it: where it is and how it points. It carries nothing a sight
/// reads besides.
pub(super) fn row_launcher(row: &Target) -> Launcher {
    let v = row.velocity;
    Launcher {
        radar_power: false,
        position: row.position,
        basis: row.basis,
        speed_fps: (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
        velocity: v,
        bay_ready: false,
        radar: false,
        jammer: false,
        alive: row.hp > 0,
        body_present: row.hp > 0,
        controls: Default::default(),
    }
}

/// How the AI flies a strike cockpit's sight.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pilot {
    /// A human's cockpit kept while the AI flies (slice FX-J): the sight
    /// designates nothing and follows nothing new.
    Kept,
    /// An AI striker's own cockpit: the sight takes the cockpit's target
    /// object as its track, as a human's sight takes the designation.
    Striker,
}

/// The targeting sight and the laser of a strike cockpit the AI flies (its
/// row `row`), for a tick: a human's left cockpit (slice FX-J) or an AI
/// striker's. Nobody slews the sight or presses its keys: a track follows
/// its object, a pin stays on its ground and a free sight looks where it was
/// left; an AI striker's sight takes its target object. The laser keeps its
/// own rules: on by hand while the sight holds a point, off by hand, or on
/// by the automatic rule while one of the aircraft's own laser weapons is
/// in flight (the AI turns its laser on by hand). The sight designates
/// nothing: the AI's sensors are its own.
pub(super) fn step_flown<G: Fn(f64, f64) -> f64>(
    cockpit: &mut StrikeCockpit,
    (aircraft, side): (u32, Side),
    pilot: Pilot,
    row: &Target,
    targets: &[Target],
    cx: &Context<'_, G>,
) {
    let target = cockpit.target_object();
    let Some(sight) = cockpit.sight.as_mut() else {
        return;
    };
    let launcher = row_launcher(row);
    let objects: Vec<SightObject> = targets
        .iter()
        .filter(|t| !t.airborne && t.id != aircraft)
        .map(|t| SightObject {
            id: t.id,
            position: t.position,
            velocity: t.velocity,
            alive: t.hp > 0,
            airborne: false,
            friendly: t.side != NO_SIDE && t.side == side,
        })
        .collect();
    if pilot == Pilot::Striker
        && let Some(id) = target
        && sight.target() != Some(id)
        && objects.iter().any(|o| o.id == id && o.alive)
    {
        sight.track(id);
    }
    let range = sight.sensor.range_ft;
    sight.step(launcher, &objects, cx.ground, range, cx.tick);
    let sensor = sight.sensor;
    let eye = sight.eye_position(launcher);
    let held = sight.held_point().map(|point| (point, sight.target()));
    let in_gimbal = held.is_some_and(|(point, _)| {
        sensor
            .gimbal
            .follows(sight::body_angles(launcher, sub(point, eye)))
    });
    let weather = held.map_or(1., |(point, _)| {
        laser::weather_scale(cx.weather, eye[1], point[1])
    });
    cockpit.laser.step(
        LaserInputs {
            designator: (sensor.laser && launcher.alive).then_some(sensor.range_ft),
            held,
            eye,
            in_gimbal,
            automatic: cx.laser_owners.contains(&aircraft),
            weather,
        },
        |from, to| sight::clear_line(from, to, cx.ground),
    );
}

fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}

impl State {
    /// Lets go of the laser the AI turned on by hand (slice ST-L) in the own
    /// cockpits of every AI aircraft not in `striking`: an aircraft whose
    /// lase duty or strike has ended no longer steps in the strike bridge, so
    /// nothing else would turn its laser off. A cockpit a human left keeps
    /// its hand setting.
    pub fn release_ai_lasers(&mut self, striking: &[u32]) {
        for (aircraft, ai) in &mut self.ai_cockpits {
            if ai.cockpit.laser.manual == Some(true) && !striking.contains(aircraft) {
                ai.cockpit.laser.manual = None;
            }
        }
    }

    /// Every laser spot this tick: one for each aircraft that is lasing, a
    /// human's, one the AI flies with the laser its human left on, or an AI
    /// striker's.
    pub fn laser_spots(&self) -> Vec<LaserSpot> {
        super::guided::spots(self.cockpits_with(&self.ownships))
    }
    /// The spot `aircraft`'s laser weapons home on: its own while it lases,
    /// else the spot of another aircraft of its side nearest `from` (buddy
    /// lasing, decision 6: opinionated, John, 2026-10-10; retail has none).
    /// The flag says whether the spot is the aircraft's own. A spot of the
    /// other side, or of an aircraft with no side, is never used. Whether a
    /// weapon can see the spot (its seeker cone) is the weapon's question.
    pub fn laser_spot_for(&self, aircraft: u32, from: Vector) -> Option<(LaserSpot, bool)> {
        let own = self.ownships.iter().find(|own| own.aircraft == aircraft)?;
        if let Some(spot) = own.laser.spot(own.aircraft, own.side) {
            return Some((spot, true));
        }
        self.buddy_spot(aircraft, from).map(|spot| (spot, false))
    }
    /// The spot of another aircraft of `aircraft`'s side nearest `from`, if
    /// any is lasing: a buddy's spot. Never one of the other side, and none
    /// for an aircraft with no side.
    pub fn buddy_spot(&self, aircraft: u32, from: Vector) -> Option<LaserSpot> {
        let own = self.ownships.iter().find(|own| own.aircraft == aircraft)?;
        if own.side == NO_SIDE {
            return None;
        }
        self.laser_spots()
            .into_iter()
            .filter(|spot| spot.owner != aircraft && spot.side == own.side)
            .map(|spot| {
                let d: f64 = (0..3).map(|i| (spot.position[i] - from[i]).powi(2)).sum();
                (d, spot)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.owner.cmp(&b.1.owner)))
            .map(|(_, spot)| spot)
    }
}
