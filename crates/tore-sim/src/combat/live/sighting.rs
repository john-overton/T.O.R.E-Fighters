//! One ownship's targeting sight and laser for a tick, and the laser spots
//! every aircraft's laser weapons may home on (docs/spec/air-to-ground.md,
//! "The targeting sight", "The laser" and "Buddy lasing"). The AC-130's
//! gunsight steps with its guns (`step_gunship`); every other aircraft with
//! a laser designator or a FLIR steps here.
use super::{Kept, Launcher, NO_SIDE, Ownship, Projectile, State, Target, projectile_weapon};
use crate::attitude::Vector;
use crate::combat::{
    laser::{self, LaserInputs, LaserSpot},
    sight::{self, SightObject, SightRequest},
};
use crate::sensors::Weather;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// What the sight and the laser read of the state this tick.
pub(super) struct Context<'a, G> {
    pub ground: &'a G,
    pub weather: &'a Weather,
    /// The aircraft with one of their own laser weapons in flight.
    pub laser_owners: &'a BTreeSet<u32>,
    pub tick: u64,
}

/// The ownships, and the aircraft the AI flies with a human's sight kept,
/// with one of their own laser weapons in flight: their lasers stay on under
/// the automatic rule.
pub(super) fn laser_owners(
    ships: &[Ownship],
    kept: &BTreeMap<u32, Kept>,
    projectiles: &[Projectile],
) -> BTreeSet<u32> {
    let lasing: BTreeSet<u32> = ships
        .iter()
        .filter(|own| own.sight.is_some_and(|sight| sight.sensor.laser))
        .map(|own| own.aircraft)
        .chain(
            kept.iter()
                .filter(|(_, kept)| kept.sight.is_some_and(|sight| sight.sensor.laser))
                .map(|(aircraft, _)| *aircraft),
        )
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
        if let Some(sight) = &mut self.sight {
            sight.request = Some(request);
        }
    }
    /// A laser weapon is selected and armed.
    pub fn laser_weapon_selected(&self) -> bool {
        self.armed
            && self
                .config
                .stations
                .get(self.selected)
                .is_some_and(|s| laser::is_laser_weapon(&s.weapon))
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
    let Some(sight) = own.sight.as_mut() else {
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
    own.laser.step(
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

/// The targeting sight and the laser a human left in an aircraft the AI
/// flies now (its row `row`), for a tick (slice FX-J). Nobody slews the
/// sight or presses its keys: a track follows its object, a pin stays on
/// its ground and a free sight looks where it was left. The laser keeps its
/// own rules: on by hand while the sight holds a point, off by hand, or on
/// by the automatic rule while one of the aircraft's own laser weapons is
/// in flight (no weapon is selected and armed while the AI flies). The
/// sight designates nothing: the AI's sensors are its own.
pub(super) fn step_kept<G: Fn(f64, f64) -> f64>(
    kept: &mut Kept,
    aircraft: u32,
    row: &Target,
    targets: &[Target],
    cx: &Context<'_, G>,
) {
    let Some(sight) = kept.sight.as_mut() else {
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
            friendly: t.side != NO_SIDE && t.side == kept.side,
        })
        .collect();
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
    kept.laser.step(
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
    /// Every laser spot this tick: one for each aircraft that is lasing, a
    /// human's or one the AI flies with the laser its human left on.
    pub fn laser_spots(&self) -> Vec<LaserSpot> {
        super::guided::spots(&self.ownships, &self.kept)
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
