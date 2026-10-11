//! Player-directed AC-130 mounts, linked membership and the gunsight's line of
//! sight. Source installation and fitted laws: docs/spec/ac130-linked-guns.md.
/// The sight, generalised for every aircraft with a laser or FLIR, lives in
/// [`super::sight`]; the gunship's names for its parts stay here.
pub use super::sight::{
    DEFAULT_ZOOM, GIMBAL_TOP, LOOK_ELEVATION_LIMIT, Notice, RETURN_PER_TICK, SIGHT_RANGE_FT,
    SightInput, SightMode as Sight, SightNotice, SightObject, SightRequest, ZOOM_STEPS,
    body_angles, clamp_elevation, direction, field_of_view, local_direction, pick, sight_ground,
    slew_raw, world_mount, zoom_step,
};
use super::{
    gunship_impact::{self, Impact, Shot},
    gunsight::{self, TargetObservation},
    live::{Configuration, Launcher, Readiness},
    sight::Gimbal,
};
use crate::attitude::{Vector, dot};
use std::f64::consts::{FRAC_PI_2, PI};
use tore_formats::{aircraft::AircraftId, weapons::Weapon};

pub const GUNS: [&str; 3] = ["C_25.JT", "C_40.JT", "C_105.JT"];
pub const NAMES: [&str; 3] = ["25MM", "40MM", "105MM"];

/// What each gun does at its trigger, by slot: the burst record
/// (`gameRoundsInBurst`, `actualRoundsPerGame`, `gameBurstT`) that gives the
/// AC-130U's cyclic rates. Opinionated (John, 2026-10-09: the retail 105
/// fired as fast as the 25), after the real weapons: the 25 mm GAU-12/U at
/// 1,800 rounds a minute, the 40 mm L/60 Bofors at 100 and the 105 mm M102
/// at 10. A held trigger spaces rounds `gameBurstT` x 30 ticks over
/// `gameRoundsInBurst` x `actualRoundsPerGame` apart (4, 72 and 720 ticks at
/// 120 Hz) and a fresh press never beats that. `actualRoundsPerGame` stays the retail 2, so
/// each round carries the damage it did and ammunition counts stay as loaded.
pub const CYCLE: [[u8; 3]; 3] = [[15, 2, 4], [5, 2, 24], [1, 2, 48]];

/// The AC-130 gun's fire rate, in rounds a minute, by slot (the figures
/// [`CYCLE`] is built to).
pub const ROUNDS_PER_MINUTE: [u32; 3] = [1_800, 100, 10];

/// Muzzle velocities in feet per second, by slot (opinionated, John,
/// 2026-10-09, after the real weapons): the GAU-12/U's 3,450, the L/60
/// Bofors's 2,870 and the M102's 1,620. Retail gives all three 2,933. A round
/// hardly slows in the retail record (7 feet per second every second), so
/// the final speed keeps the retail half of the muzzle velocity.
pub const MUZZLE_FPS: [i16; 3] = [3_450, 2_870, 1_620];

/// Puts the TORE cyclic rate and muzzle velocity on one of the three AC-130
/// gun records and leaves every other weapon alone. Retail gives all three
/// the same generic record (8 rounds a quarter second for the 25 and 40, a
/// second for the 105: 1,920, 1,920 and 480 rounds a minute, 2,933 feet a
/// second). The record keeps its 10 second life: the 105's slowest round
/// reaches the 13,000 foot sight limit in about 8 seconds.
pub fn apply_tore_record(weapon: &mut Weapon) {
    if let Some(slot) = GUNS
        .iter()
        .position(|gun| weapon.source.eq_ignore_ascii_case(gun))
    {
        let [in_burst, per_game, burst_t] = CYCLE[slot];
        weapon.burst.game_rounds_in_burst = in_burst;
        weapon.burst.actual_rounds_per_game = per_game;
        weapon.burst.game_burst_t = burst_t;
        let muzzle = MUZZLE_FPS[slot];
        let movement = &mut weapon.movement;
        movement.initial_speed = muzzle;
        movement.maximum_speed = movement.maximum_speed.max(muzzle);
        movement.corner_speed = movement.corner_speed.max(muzzle);
        movement.final_speed = muzzle / 2;
    }
}

/// A gun record's rounds a minute out of the barrel when held, from its burst
/// record: the retail figure for a record that has not been through
/// [`apply_tore_record`].
pub fn rounds_per_minute(weapon: &Weapon) -> f64 {
    let rounds = f64::from(weapon.burst.game_rounds_in_burst.max(1))
        * f64::from(weapon.burst.actual_rounds_per_game.max(1));
    // A burst time is a quarter second.
    240. * rounds / f64::from(weapon.burst.game_burst_t.max(1))
}
/// Fitted pivots and muzzle tips selected from reviewed original barrel meshes.
/// Source axes: right, forward, up. Shared by shot spawn and the renderer.
pub const PIVOTS_SOURCE: [Vector; 3] = [[-9.5, 29., -12.], [-11., -7., -11.], [-9., -25., -11.5]];
pub const TIPS_SOURCE: [Vector; 3] = [[-16.5, 28., -14.], [-18., -7., -14.], [-21.5, -25., -14.5]];
pub const SOURCE_SCALE: f64 = 2. / 3.;
/// Sensor dome D, the gunsight camera's eye: the round electro-optical
/// turret under the left side of the fuselage, just forward of the wing root,
/// at the middle of the left belly fairing (source X -9..-14, Y 4..26, Z -7..-16
/// in AC130.SH). Source axes: right, forward, up. Fitted from the reviewed
/// mesh and John's reference photo (2026-10-09): the ball hangs just below
/// the fairing's lower face, so the point sits 1.5 source units under it.
pub const EYE_SOURCE: Vector = [-13.5, 11., -15.5];
/// Each gun's heading half-arc either side of neutral (abeam left), radians,
/// by source slot; the target camera's gun box draws them.
pub const HEADING_ARC: [f64; 3] = [
    60_f64.to_radians(),
    45_f64.to_radians(),
    25_f64.to_radians(),
];
/// Each gun's elevation half-arc either side of level, radians.
pub const ELEVATION_ARC: [f64; 3] = [
    60_f64.to_radians(),
    45_f64.to_radians(),
    45_f64.to_radians(),
];
const SLEW_PER_TICK: f64 = 30_f64.to_radians() / 120.;
const AIM_TOLERANCE: f64 = 1_f64.to_radians();

/// The default view: 90 degrees left and 25 degrees down in the aircraft's
/// own frame, as [heading, elevation] (opinionated, John, 2026-10-09).
pub const DEFAULT_LOOK: [f64; 2] = [-FRAC_PI_2, -25. * PI / 180.];
/// The AC-130 dome's sensor for the shared sight: the hemisphere below the
/// aircraft (opinionated, John, 2026-10-09: the belly shows only a few
/// degrees of fuselage across the right side from the dome, see
/// docs/spec/ac130-linked-guns.md, so the boundary is the horizontal plane of
/// the aircraft), sensor dome D's eye and [`DEFAULT_LOOK`]. No laser. The
/// range is the AC130I FLIR record's search range, which only `/` reads.
pub fn sensor() -> super::sight::SightSensor {
    super::sight::SightSensor {
        laser: false,
        range_ft: 54_684.,
        gimbal: Gimbal::Hemisphere,
        eye: eye(),
        default_look: DEFAULT_LOOK,
    }
}
/// With no ground on the line of sight, the guns aim this far along it when
/// no gun reports a range (the shipped guns reach 13,000 feet).
const SKY_RANGE_FT: f64 = 13_000.;

/// Actual fixed-tick mount angles, linked membership and the sight.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub stations: [Option<usize>; 3],
    pub included: [bool; 3],
    pub headings: [f64; 3],
    pub elevations: [f64; 3],
    /// What the sight holds.
    pub sight: Sight,
    /// Body-relative [heading, elevation] of the camera's line of sight from
    /// the eye, kept current in every mode so a drop continues from it. It
    /// never leaves the gimbal's hemisphere ([`GIMBAL_TOP`] and below); a
    /// track or pin above it leaves the camera at the limit while `aim`
    /// stays on the true point.
    pub look: [f64; 2],
    /// The sight is travelling back to [`DEFAULT_LOOK`] at
    /// [`RETURN_PER_TICK`].
    pub returning: bool,
    /// The point the guns train on this tick.
    pub aim: Option<Vector>,
    /// Where each gun's rounds would end if it fired this tick at its actual
    /// train (the pipper, [`gunship_impact::impact`]): the linked guns and
    /// the candidate, `None` for the others. Derived, but kept so a restored
    /// checkpoint shows the same frame.
    pub impacts: [Option<Impact>; 3],
    /// The combat tick a round fired with these impacts would leave on (the
    /// tick before the step that computed them advanced the clock).
    pub impacts_tick: u64,
    pub status: [Readiness; 3],
    /// The seat's sight controls, held until the host sets them again.
    pub input: SightInput,
    /// The last step saw the sight deflected (a slew while tracking says
    /// [`Notice::DropToSlew`] once per push).
    pub slew_held: bool,
    pub request: Option<SightRequest>,
    pub notice: Option<SightNotice>,
}
impl Default for State {
    /// An AC-130 group with no guns installed, in the default view.
    fn default() -> Self {
        Self {
            stations: [None; 3],
            included: [false; 3],
            headings: [-FRAC_PI_2; 3],
            elevations: [0.; 3],
            sight: Sight::Free,
            look: DEFAULT_LOOK,
            returning: false,
            aim: None,
            impacts: [None; 3],
            impacts_tick: 0,
            status: [Readiness::NoTarget; 3],
            input: SightInput::default(),
            slew_held: false,
            request: None,
            notice: None,
        }
    }
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
            ..Self::default()
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
    /// The tracked object, if the sight holds one.
    pub fn target(&self) -> Option<u32> {
        self.sight.tracked()
    }
    /// The zoom step in use, 1 to 6.
    pub fn zoom(&self) -> u8 {
        zoom_step(self.input.zoom)
    }
    /// Follow an object from now on (a T, Enter, scope click or Backslash).
    pub fn track(&mut self, id: u32) {
        self.sight = Sight::Tracked(id);
        self.returning = false;
        self.request = None;
    }
    /// L: drop a target or pin and slew freely from the current look; with
    /// nothing held, travel back to the default view.
    pub fn drop_hold(&mut self) {
        if self.request.take().is_some() {
            return;
        }
        match self.sight {
            Sight::Tracked(_) | Sight::Pinned(_) => {
                self.sight = Sight::Free;
                self.returning = false;
            }
            Sight::Free => self.returning = self.look != DEFAULT_LOOK,
        }
    }
    /// Move the sight for one tick and work out the aim point: resolve a
    /// pending Backslash, end a track whose object is gone (pinning the
    /// ground under the line of sight), slew or travel home, then find
    /// where the line of sight meets the ground. Returns what the guns aim
    /// at, the point and its velocity, and whether the pipper leads it (a
    /// tracked object that flies or moves).
    pub fn step_sight(
        &mut self,
        config: &Configuration,
        launcher: Launcher,
        objects: &[SightObject],
        ground: &impl Fn(f64, f64) -> f64,
        tick: u64,
    ) -> (TargetObservation, bool) {
        let mut sight = self.shared_sight();
        let result = sight.step(launcher, objects, ground, sky_range(config), tick);
        self.sight = sight.mode;
        self.look = sight.look;
        self.returning = sight.returning;
        self.aim = sight.aim;
        self.input = sight.input;
        self.slew_held = sight.slew_held;
        self.request = sight.request;
        self.notice = sight.notice;
        result
    }
    /// The gunsight as the shared sight sees it.
    pub fn shared_sight(&self) -> super::sight::Sight {
        super::sight::Sight {
            sensor: sensor(),
            mode: self.sight,
            look: self.look,
            returning: self.returning,
            aim: self.aim,
            input: self.input,
            slew_held: self.slew_held,
            request: self.request,
            notice: self.notice,
        }
    }
    /// The pipper of every gun in `wanted` (the linked guns and the
    /// candidate) at its actual train, for a round leaving on `launch_tick`.
    /// `lead` is the tracked object a led pipper follows.
    pub fn evaluate_impacts(
        &mut self,
        config: &Configuration,
        launcher: Launcher,
        wanted: [bool; 3],
        lead: Option<TargetObservation>,
        launch_tick: u64,
        ground: &impl Fn(f64, f64) -> f64,
    ) {
        self.impacts_tick = launch_tick;
        self.impacts = std::array::from_fn(|slot| {
            let station = self.stations[slot].and_then(|i| config.stations.get(i))?;
            if !wanted[slot] {
                return None;
            }
            gunship_impact::impact(
                slot,
                &station.weapon,
                &launcher,
                self.headings[slot],
                self.elevations[slot],
                Shot {
                    target: lead,
                    launch_tick,
                },
                ground,
            )
        });
    }
    /// Train every gun toward the aim point and report its readiness. With
    /// no ballistic solution the guns still point along the plain line to
    /// the aim point and report MAX RANGE; outside an arc they stop at its
    /// edge. `clear` says whether terrain leaves the line between two points
    /// open. The status is a label: only [`Readiness::GunObscured`] (and the
    /// empty-station states) stop a gun firing, see
    /// [`Readiness::gun_may_fire`].
    pub fn update(
        &mut self,
        config: &Configuration,
        launcher: Launcher,
        aim: Option<TargetObservation>,
        clear: impl Fn(Vector, Vector) -> bool,
    ) {
        for slot in 0..3 {
            let Some(station) = self.stations[slot].and_then(|i| config.stations.get(i)) else {
                self.status[slot] = Readiness::Empty;
                continue;
            };
            let Some(observation) = aim else {
                self.status[slot] = Readiness::NoTarget;
                continue;
            };
            let mount = local_muzzle(slot, self.headings[slot], self.elevations[slot]);
            let solution =
                gunsight::solve_observed(&station.weapon, &launcher, mount, Some(observation))
                    .ok()
                    .flatten();
            let pivot = world_mount(launcher, pivot(slot));
            let toward: Vector = match solution {
                Some(solution) => std::array::from_fn(|i| {
                    observation.position[i]
                        + observation.velocity[i] * solution.seconds
                        + if i == 1 { solution.drop_ft } else { 0. }
                        - pivot[i]
                }),
                None => sub(observation.position, pivot),
            };
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
            // The gun's own airframe is the one geometric state that blocks
            // fire, so it leads the label whatever else is true of the shot.
            if !clear_airframe(slot, self.headings[slot], self.elevations[slot]) {
                self.status[slot] = Readiness::GunObscured;
                continue;
            }
            let Some(solution) = solution else {
                self.status[slot] = Readiness::MaximumRange;
                continue;
            };
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
            } else if !clear(muzzle, short_of(muzzle, observation.position)) {
                Readiness::TerrainMask
            } else {
                Readiness::Ready
            };
        }
    }
}
/// The look angles after one tick of slew at this deflection and zoom, held
/// inside the camera's gimbal. Heading wraps through a full turn; elevation
/// stops at the horizon ([`GIMBAL_TOP`]) and just short of nadir. The heading
/// rate is divided by the cosine of elevation (floored at a quarter) so the
/// picture moves at the same rate across the screen.
pub fn slewed(look: [f64; 2], deflection: [f64; 2], zoom: u8) -> [f64; 2] {
    clamp_look(slew_raw(look, deflection, zoom))
}
/// A pin's slew from its own bearing (which may lie above the gimbal): it may
/// not rise above the horizon, nor above where it already is.
pub fn slew_pin(look: [f64; 2], deflection: [f64; 2], zoom: u8) -> [f64; 2] {
    Gimbal::Hemisphere
        .slew_pin(look, slew_raw(look, deflection, zoom))
        .0
}
/// A look held inside the camera's gimbal: the hemisphere below the aircraft.
pub fn clamp_look(look: [f64; 2]) -> [f64; 2] {
    Gimbal::Hemisphere.clamp(look)
}
/// The sight's bearing is above the camera's gimbal: the camera stops at its
/// edge and the notice shows.
pub fn beyond_gimbal(look: [f64; 2]) -> bool {
    Gimbal::Hemisphere.beyond(look)
}
/// One tick of the travel back to the default view, along the straight line
/// between the two in heading and elevation.
pub fn homeward(look: [f64; 2]) -> [f64; 2] {
    super::sight::homeward_to(look, DEFAULT_LOOK)
}
use super::sight::short_of;
fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}
/// Where the guns aim along a line of sight with no ground: the longest
/// installed gun range.
fn sky_range(config: &Configuration) -> f64 {
    let range = GUNS
        .iter()
        .filter_map(|source| {
            config
                .stations
                .iter()
                .find(|s| s.weapon.source.eq_ignore_ascii_case(source))
        })
        .map(|s| f64::from(s.weapon.seeker.zones[1].maximum_range.max(0)))
        .fold(0., f64::max);
    if range > 0. { range } else { SKY_RANGE_FT }
}
fn approach(current: f64, demand: f64) -> f64 {
    current + (demand - current).clamp(-SLEW_PER_TICK, SLEW_PER_TICK)
}
/// The gunsight camera's eye in the aircraft's frame, host mount order
/// (right, up, forward), feet: sensor dome D.
pub fn eye() -> Vector {
    let [x, forward, up] = EYE_SOURCE;
    [x * SOURCE_SCALE, up * SOURCE_SCALE, forward * SOURCE_SCALE]
}
/// Where the camera's eye is in the world: every sight ray (designate, pin,
/// terrain mask of a pick, the camera) starts here, not at the aircraft's
/// centre.
pub fn eye_position(launcher: Launcher) -> Vector {
    world_mount(launcher, eye())
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
pub fn local_muzzle(slot: usize, heading: f64, elevation: f64) -> Vector {
    let d = local_direction(heading, elevation);
    let pivot = pivot(slot);
    let length = barrel_length(slot);
    std::array::from_fn(|i| pivot[i] + d[i] * length)
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
