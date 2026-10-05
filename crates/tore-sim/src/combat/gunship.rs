//! Player-directed AC-130 mounts and linked membership. No target selection.
//! Source installation and fitted laws: docs/spec/ac130-linked-guns.md.
use super::{
    gunsight,
    live::{Configuration, Launcher, Readiness},
};
use crate::attitude::{Vector, dot, unit};
use std::f64::consts::{FRAC_PI_2, PI};
use tore_formats::aircraft::AircraftId;

pub const GUNS: [&str; 3] = ["C_25.JT", "C_40.JT", "C_105.JT"];
pub const NAMES: [&str; 3] = ["25MM", "40MM", "105MM"];
/// Fitted pivots and muzzle tips selected from reviewed original barrel meshes.
/// Source axes: right, forward, up. Shared by shot spawn and the renderer.
pub const PIVOTS_SOURCE: [Vector; 3] = [[-9.5, 29., -12.], [-11., -7., -11.], [-9., -25., -11.5]];
pub const TIPS_SOURCE: [Vector; 3] = [[-16.5, 28., -14.], [-18., -7., -14.], [-21.5, -25., -14.5]];
pub const SOURCE_SCALE: f64 = 2. / 3.;
const HEADING_ARC: [f64; 3] = [
    60_f64.to_radians(),
    45_f64.to_radians(),
    25_f64.to_radians(),
];
const ELEVATION_ARC: [f64; 3] = [
    60_f64.to_radians(),
    45_f64.to_radians(),
    45_f64.to_radians(),
];
const SLEW_PER_TICK: f64 = 30_f64.to_radians() / 120.;
const AIM_TOLERANCE: f64 = 1_f64.to_radians();

/// Actual fixed-tick mount angles and linked membership, separate from selection.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub stations: [Option<usize>; 3],
    pub included: [bool; 3],
    pub headings: [f64; 3],
    pub elevations: [f64; 3],
    pub target: Option<u32>,
    pub status: [Readiness; 3],
}
impl State {
    pub fn new(config: &Configuration) -> Option<Self> {
        if config.aircraft != AircraftId::Ac130 {
            return None;
        }
        let stations = GUNS.map(|source| {
            config
                .stations
                .iter()
                .position(|s| s.weapon.source.eq_ignore_ascii_case(source))
        });
        let mut included = [false; 3];
        if let Some(slot) = stations.iter().position(Option::is_some) {
            included[slot] = true;
        }
        Some(Self {
            stations,
            included,
            headings: [-FRAC_PI_2; 3],
            elevations: [0.; 3],
            target: None,
            status: [Readiness::NoTarget; 3],
        })
    }
    pub fn slot(&self, station: usize) -> Option<usize> {
        self.stations.iter().position(|s| *s == Some(station))
    }
    pub fn mask(&self) -> u8 {
        self.included
            .iter()
            .enumerate()
            .fold(0, |mask, (i, on)| mask | (u8::from(*on) << i))
    }
    /// Six presentation values: heading/pi, elevation/(pi/2), per source slot.
    pub fn normalized_devices(&self) -> [f64; 6] {
        std::array::from_fn(|i| {
            if i % 2 == 0 {
                self.headings[i / 2] / PI
            } else {
                self.elevations[i / 2] / FRAC_PI_2
            }
        })
    }
    pub fn fire_stations(&self) -> Vec<usize> {
        self.stations
            .iter()
            .zip(self.included)
            .filter_map(|(station, on)| on.then_some(*station).flatten())
            .collect()
    }
    pub fn solo(&mut self, station: usize) {
        if let Some(slot) = self.slot(station)
            && self.included.iter().filter(|on| **on).count() <= 1
        {
            self.included = std::array::from_fn(|i| i == slot);
        }
    }
    pub fn toggle(&mut self, station: usize) {
        if let Some(slot) = self.slot(station) {
            self.included[slot] = !self.included[slot];
        }
    }
    /// Track only the observation supplied by the pilot's existing designation.
    pub fn update(
        &mut self,
        config: &Configuration,
        launcher: Launcher,
        target: Option<(u32, gunsight::TargetObservation)>,
        absent: Readiness,
        clear: impl Fn(Vector, Vector) -> bool,
    ) {
        self.target = target.map(|(id, _)| id);
        for slot in 0..3 {
            let Some(station) = self.stations[slot].and_then(|i| config.stations.get(i)) else {
                self.status[slot] = Readiness::Empty;
                continue;
            };
            let Some((_, observation)) = target else {
                self.status[slot] = absent;
                continue;
            };
            let mount = local_muzzle(slot, self.headings[slot], self.elevations[slot]);
            let Ok(Some(solution)) =
                gunsight::solve_observed(&station.weapon, &launcher, mount, Some(observation))
            else {
                self.status[slot] = Readiness::MaximumRange;
                continue;
            };
            let pivot = world_mount(launcher, pivot(slot));
            let toward: Vector = std::array::from_fn(|i| {
                observation.position[i]
                    + observation.velocity[i] * solution.seconds
                    + if i == 1 { solution.drop_ft } else { 0. }
                    - pivot[i]
            });
            let right = dot(toward, launcher.basis.right);
            let forward = dot(toward, launcher.basis.forward);
            let up = dot(toward, launcher.basis.up);
            let heading = right.atan2(forward);
            let elevation = up.atan2(right.hypot(forward));
            let lo = -FRAC_PI_2 - HEADING_ARC[slot];
            let hi = -FRAC_PI_2 + HEADING_ARC[slot];
            self.headings[slot] = approach(self.headings[slot], heading.clamp(lo, hi));
            self.elevations[slot] = approach(
                self.elevations[slot],
                elevation.clamp(-ELEVATION_ARC[slot], ELEVATION_ARC[slot]),
            );
            let muzzle = muzzle(slot, launcher, self.headings[slot], self.elevations[slot]);
            let zone = station.weapon.seeker.zones[1];
            self.status[slot] = if solution.range_ft < f64::from(zone.minimum_range.max(0)) {
                Readiness::MinimumRange
            } else if solution.range_ft > solution.maximum_range_ft {
                Readiness::MaximumRange
            } else if !(lo..=hi).contains(&heading) || elevation.abs() > ELEVATION_ARC[slot] {
                Readiness::GunArc
            } else if (heading - self.headings[slot]).abs() > AIM_TOLERANCE
                || (elevation - self.elevations[slot]).abs() > AIM_TOLERANCE
            {
                Readiness::GunSlewing
            } else if !clear_airframe(slot, self.headings[slot], self.elevations[slot])
                || !clear(muzzle, observation.position)
            {
                Readiness::GunObscured
            } else {
                Readiness::Ready
            };
        }
    }
}
fn approach(current: f64, demand: f64) -> f64 {
    current + (demand - current).clamp(-SLEW_PER_TICK, SLEW_PER_TICK)
}
pub fn pivot(slot: usize) -> Vector {
    let [x, forward, up] = PIVOTS_SOURCE[slot];
    [x * SOURCE_SCALE, up * SOURCE_SCALE, forward * SOURCE_SCALE]
}
pub fn barrel_length(slot: usize) -> f64 {
    PIVOTS_SOURCE[slot]
        .iter()
        .zip(TIPS_SOURCE[slot])
        .map(|(a, b)| (b - a).powi(2))
        .sum::<f64>()
        .sqrt()
        * SOURCE_SCALE
}
pub fn local_direction(heading: f64, elevation: f64) -> Vector {
    [
        heading.sin() * elevation.cos(),
        elevation.sin(),
        heading.cos() * elevation.cos(),
    ]
}
pub fn direction(launcher: Launcher, heading: f64, elevation: f64) -> Vector {
    let d = local_direction(heading, elevation);
    unit(std::array::from_fn(|i| {
        launcher.basis.right[i] * d[0]
            + launcher.basis.up[i] * d[1]
            + launcher.basis.forward[i] * d[2]
    }))
}
pub fn local_muzzle(slot: usize, heading: f64, elevation: f64) -> Vector {
    let d = local_direction(heading, elevation);
    let pivot = pivot(slot);
    let length = barrel_length(slot);
    std::array::from_fn(|i| pivot[i] + d[i] * length)
}
pub fn world_mount(launcher: Launcher, mount: Vector) -> Vector {
    std::array::from_fn(|i| {
        launcher.position[i]
            + launcher.basis.right[i] * mount[0]
            + launcher.basis.up[i] * mount[1]
            + launcher.basis.forward[i] * mount[2]
    })
}
pub fn muzzle(slot: usize, launcher: Launcher, heading: f64, elevation: f64) -> Vector {
    world_mount(launcher, local_muzzle(slot, heading, elevation))
}

/// Conservative fitted source-mesh volumes, in source right/forward/up axes.
/// The fuselage skin, left wing and two left nacelles must not lie ahead of a muzzle.
pub fn clear_airframe(slot: usize, heading: f64, elevation: f64) -> bool {
    let mount = local_muzzle(slot, heading, elevation);
    if mount[0] > -8. {
        return false;
    }
    let source = [
        mount[0] / SOURCE_SCALE,
        mount[2] / SOURCE_SCALE,
        mount[1] / SOURCE_SCALE,
    ];
    let d = local_direction(heading, elevation);
    let ray = [d[0], d[2], d[1]];
    let boxes = [
        ([-99., -25., 5.], [-11., 1., 7.]),
        ([-33., -25., -6.], [-18., 19., 11.]),
        ([-59., -25., -6.], [-44., 19., 11.]),
    ];
    !boxes
        .into_iter()
        .any(|(lo, hi)| ray_box(source, ray, lo, hi))
}
fn ray_box(origin: Vector, direction: Vector, lo: Vector, hi: Vector) -> bool {
    let mut near: f64 = 0.;
    let mut far: f64 = 512.;
    for i in 0..3 {
        if direction[i].abs() < 1e-12 {
            if origin[i] < lo[i] || origin[i] > hi[i] {
                return false;
            }
        } else {
            let a = (lo[i] - origin[i]) / direction[i];
            let b = (hi[i] - origin[i]) / direction[i];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return false;
            }
        }
    }
    far >= near
}
