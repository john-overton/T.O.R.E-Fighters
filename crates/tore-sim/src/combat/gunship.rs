//! Player-directed AC-130 mounts, linked membership and the gunsight's line of
//! sight. Source installation and fitted laws: docs/spec/ac130-linked-guns.md.
use super::{
    gunship_impact::{self, Impact, Shot},
    gunsight::{self, TargetObservation},
    live::{Configuration, Launcher, Readiness, terrain_hit},
};
use crate::attitude::{Vector, cross, dot, unit};
use std::f64::consts::{FRAC_PI_2, PI};
use tore_formats::aircraft::AircraftId;

pub const GUNS: [&str; 3] = ["C_25.JT", "C_40.JT", "C_105.JT"];
pub const NAMES: [&str; 3] = ["25MM", "40MM", "105MM"];
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

/// The default view: 90 degrees left and 25 degrees down in the aircraft's
/// own frame, as [heading, elevation] (opinionated, John, 2026-10-09).
pub const DEFAULT_LOOK: [f64; 2] = [-FRAC_PI_2, -25. * PI / 180.];
/// Free slew elevation stops short of straight down (fitted: avoids the
/// singular heading there). The camera's gimbal reaches nadir otherwise.
pub const LOOK_ELEVATION_LIMIT: f64 = 89. * PI / 180.;
/// The camera's gimbal covers the hemisphere below the aircraft: elevation
/// from here down through nadir, heading free across the full turn
/// (opinionated, John, 2026-10-09). The belly shows only a few degrees of
/// fuselage across the right side from the dome (see docs/spec/ac130-linked-guns.md),
/// so the boundary is the horizontal plane of the aircraft.
pub const GIMBAL_TOP: f64 = 0.;
/// The target camera's zoom ladder: six steps, the widest 30 degrees tall,
/// each half the one before (fitted, agent choice; retail gives six steps).
pub const ZOOM_STEPS: u8 = 6;
pub const DEFAULT_ZOOM: u8 = 3;
const WIDEST_FIELD: f64 = 30. * PI / 180.;
/// Full deflection slews this fraction of the vertical field per second
/// (fitted).
const SLEW_FIELDS_PER_SECOND: f64 = 0.75;
/// The return to the default view travels at the fastest sight slew, full
/// deflection at the widest zoom step: 22.5 degrees a second, whatever the
/// zoom (agent choice; John, 2026-10-09: it never snaps).
pub const RETURN_PER_TICK: f64 = SLEW_FIELDS_PER_SECOND * WIDEST_FIELD / 120.;
/// The Backslash pick radius as a fraction of the field's height: the
/// pipper circle's 9 of the page's 114 pixels.
const PICK_FIELD_FRACTION: f64 = 9. / 114.;
/// How far along the line of sight the ground is searched (fitted: 40 nmi).
pub const SIGHT_RANGE_FT: f64 = 243_000.;
/// With no ground on the line of sight, the guns aim this far along it when
/// no gun reports a range (the shipped guns reach 13,000 feet).
const SKY_RANGE_FT: f64 = 13_000.;
/// Terrain within this distance of a point on the ground does not mask it,
/// so a ground point is never masked by the ground it lies on (fitted).
const MASK_SHORT_FT: f64 = 25.;

/// What the sight holds (opinionated, John, 2026-10-09).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sight {
    /// Body-relative look angles the pilot slews; the aim point is where the
    /// line of sight meets the ground.
    Free,
    /// A fixed world ground point. Slewing moves it.
    Pinned(Vector),
    /// An object, air or ground, followed at any range by its true position,
    /// like a pod track.
    Tracked(u32),
}
impl Sight {
    pub fn tracked(self) -> Option<u32> {
        match self {
            Self::Tracked(id) => Some(id),
            Self::Free | Self::Pinned(_) => None,
        }
    }
}

/// The seat's sight controls for a tick: normalized deflection (x right,
/// y up, -127 to 127) and the zoom step (1 to 6; 0 means the default step).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SightInput {
    pub deflection: [i8; 2],
    pub zoom: u8,
}

/// A Backslash or Shift+Backslash waiting for the next step, which has the
/// terrain and the objects to resolve it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SightRequest {
    /// Track the object under the crosshair, else pin the ground there.
    Designate,
    /// Pin the ground under the crosshair.
    Pin,
}

/// A one-off message the sight raises for the target camera's activity line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// A pin was asked for with no ground on the line of sight.
    NoGroundPoint,
    /// The pilot slewed while tracking: L drops the target first.
    DropToSlew,
    /// The camera is stopped at the edge of its gimbal: the pilot is slewing
    /// against it, or the tracked target or pin lies above the hemisphere
    /// below the aircraft. Raised every tick it holds.
    GimbalLimit,
}
/// The last notice and the combat tick it was raised on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SightNotice {
    pub notice: Notice,
    pub tick: u64,
}

/// One object the sight may pick or follow, as combat sees it this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SightObject {
    pub id: u32,
    pub position: Vector,
    pub velocity: Vector,
    /// Not destroyed: hit points left.
    pub alive: bool,
    /// Flying: the pipper leads it even when it is still.
    pub airborne: bool,
    /// On the pilot's side; Backslash skips it.
    pub friendly: bool,
}

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
        let origin = eye_position(launcher);
        let found = |id: u32| objects.iter().find(|o| o.id == id && o.alive);
        // Set when the camera is held at the gimbal's edge this tick.
        let mut limited = false;
        self.look = clamp_look(self.look);
        // A track ends only when its object is destroyed or removed; the
        // sight then holds the ground it was looking at.
        if let Sight::Tracked(id) = self.sight {
            match found(id) {
                Some(object) => {
                    let at = body_angles(launcher, sub(object.position, origin));
                    limited |= at[1] > GIMBAL_TOP;
                    self.look = clamp_look(at);
                }
                None => match sight_ground(
                    origin,
                    direction(launcher, self.look[0], self.look[1]),
                    ground,
                ) {
                    Some(point) => self.sight = Sight::Pinned(point),
                    None => self.sight = Sight::Free,
                },
            }
        }
        // The pin's own bearing from the eye, which may lie above the
        // gimbal while the camera stops at its edge.
        let mut pin_look = [0.; 2];
        if let Sight::Pinned(point) = self.sight {
            pin_look = body_angles(launcher, sub(point, origin));
            limited |= pin_look[1] > GIMBAL_TOP;
            self.look = clamp_look(pin_look);
        }
        let line = direction(launcher, self.look[0], self.look[1]);
        match (self.request.take(), self.sight) {
            (None, _) | (Some(SightRequest::Designate), Sight::Tracked(_)) => {}
            (Some(request), _) => {
                let picked = (request == SightRequest::Designate)
                    .then(|| pick(origin, line, self.zoom(), objects, ground))
                    .flatten();
                if let Some(id) = picked {
                    self.track(id);
                } else if let Some(point) = sight_ground(origin, line, ground) {
                    self.sight = Sight::Pinned(point);
                    self.returning = false;
                    pin_look = self.look;
                } else {
                    self.notice = Some(SightNotice {
                        notice: Notice::NoGroundPoint,
                        tick,
                    });
                }
            }
        }
        let deflection = self.input.deflection.map(|v| f64::from(v.max(-127)) / 127.);
        let deflected = deflection != [0., 0.];
        match self.sight {
            Sight::Tracked(_) => {
                if deflected && !self.slew_held {
                    self.notice = Some(SightNotice {
                        notice: Notice::DropToSlew,
                        tick,
                    });
                }
            }
            Sight::Free => {
                if deflected {
                    self.returning = false;
                    let turned = slew_raw(self.look, deflection, self.zoom());
                    limited |= turned[1] > GIMBAL_TOP;
                    self.look = clamp_look(turned);
                } else if self.returning {
                    self.look = homeward(self.look);
                    self.returning = self.look != DEFAULT_LOOK;
                }
            }
            Sight::Pinned(_) => {
                if deflected {
                    let turned = slew_pin(pin_look, deflection, self.zoom());
                    limited |= slew_raw(pin_look, deflection, self.zoom())[1] > turned[1];
                    // A slew the limit cancels leaves the pin where it is.
                    let moved =
                        wrap(turned[0] - pin_look[0]).abs() + (turned[1] - pin_look[1]).abs();
                    if moved > 1e-12
                        && let Some(point) =
                            sight_ground(origin, direction(launcher, turned[0], turned[1]), ground)
                    {
                        self.sight = Sight::Pinned(point);
                        self.look = clamp_look(turned);
                    }
                }
            }
        }
        if limited && self.notice.is_none_or(|n| n.tick != tick) {
            self.notice = Some(SightNotice {
                notice: Notice::GimbalLimit,
                tick,
            });
        }
        self.slew_held = deflected;
        let tracked = self.sight.tracked().and_then(found);
        let led = tracked.is_some_and(|o| o.airborne || o.velocity != [0.; 3]);
        let (position, velocity) = match (self.sight, tracked) {
            (Sight::Tracked(_), Some(object)) => (object.position, object.velocity),
            (Sight::Pinned(point), _) => (point, [0.; 3]),
            (Sight::Free | Sight::Tracked(_), _) => {
                let line = direction(launcher, self.look[0], self.look[1]);
                let point = sight_ground(origin, line, ground).unwrap_or_else(|| {
                    let range = sky_range(config);
                    std::array::from_fn(|i| origin[i] + line[i] * range)
                });
                (point, [0.; 3])
            }
        };
        self.aim = Some(position);
        (TargetObservation { position, velocity }, led)
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
/// The zoom step a seat's raw value means: 0 is the default step.
pub fn zoom_step(raw: u8) -> u8 {
    if raw == 0 {
        DEFAULT_ZOOM
    } else {
        raw.min(ZOOM_STEPS)
    }
}
/// The target camera's vertical field of view at a zoom step, radians.
pub fn field_of_view(step: u8) -> f64 {
    WIDEST_FIELD / f64::from(1u32 << (zoom_step(step) - 1))
}
/// The look angles after one tick of slew at this deflection and zoom, held
/// inside the camera's gimbal. Heading wraps through a full turn; elevation
/// stops at the horizon ([`GIMBAL_TOP`]) and just short of nadir. The heading
/// rate is divided by the cosine of elevation (floored at a quarter) so the
/// picture moves at the same rate across the screen.
pub fn slewed(look: [f64; 2], deflection: [f64; 2], zoom: u8) -> [f64; 2] {
    clamp_look(slew_raw(look, deflection, zoom))
}
/// [`slewed`] before the gimbal clamp: what the pilot's slew asks for. Its
/// elevation above [`GIMBAL_TOP`] is a push against the limit.
pub fn slew_raw(look: [f64; 2], deflection: [f64; 2], zoom: u8) -> [f64; 2] {
    let rate = SLEW_FIELDS_PER_SECOND * field_of_view(zoom) / 120.;
    let heading = look[0] + rate * deflection[0] / look[1].cos().max(0.25);
    [wrap(heading), look[1] + rate * deflection[1]]
}
/// A pin's slew from its own bearing (which may lie above the gimbal): it may
/// not rise above the horizon, nor above where it already is.
pub fn slew_pin(look: [f64; 2], deflection: [f64; 2], zoom: u8) -> [f64; 2] {
    let mut turned = slew_raw(look, deflection, zoom);
    turned[1] = turned[1].clamp(-LOOK_ELEVATION_LIMIT, look[1].max(GIMBAL_TOP));
    turned
}
/// The elevation of a camera look held inside the gimbal.
pub fn clamp_elevation(elevation: f64) -> f64 {
    elevation.clamp(-LOOK_ELEVATION_LIMIT, GIMBAL_TOP)
}
/// A look held inside the camera's gimbal: the hemisphere below the aircraft.
pub fn clamp_look(look: [f64; 2]) -> [f64; 2] {
    [look[0], clamp_elevation(look[1])]
}
/// The sight's bearing is above the camera's gimbal: the camera stops at its
/// edge and the notice shows.
pub fn beyond_gimbal(look: [f64; 2]) -> bool {
    look[1] > GIMBAL_TOP
}
/// One tick of the travel back to the default view, along the straight line
/// between the two in heading and elevation.
pub fn homeward(look: [f64; 2]) -> [f64; 2] {
    let dh = wrap(DEFAULT_LOOK[0] - look[0]);
    let de = DEFAULT_LOOK[1] - look[1];
    let distance = dh.hypot(de);
    if distance <= RETURN_PER_TICK {
        return DEFAULT_LOOK;
    }
    [
        wrap(look[0] + dh / distance * RETURN_PER_TICK),
        look[1] + de / distance * RETURN_PER_TICK,
    ]
}
/// An angle in (-pi, pi].
fn wrap(angle: f64) -> f64 {
    let wrapped = (angle + PI).rem_euclid(2. * PI) - PI;
    if wrapped == -PI { PI } else { wrapped }
}
fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}
/// Body-relative [heading, elevation] of a world offset, as the true bearing:
/// it can lie above the camera's gimbal ([`clamp_look`] holds the camera
/// there); only the straight-down singularity is kept off.
pub fn body_angles(launcher: Launcher, offset: Vector) -> [f64; 2] {
    let right = dot(offset, launcher.basis.right);
    let forward = dot(offset, launcher.basis.forward);
    let up = dot(offset, launcher.basis.up);
    [
        right.atan2(forward),
        up.atan2(right.hypot(forward))
            .clamp(-LOOK_ELEVATION_LIMIT, LOOK_ELEVATION_LIMIT),
    ]
}
/// Where a line of sight from `origin` along the unit `line` first meets
/// the ground, within [`SIGHT_RANGE_FT`]. It marches in steps of half the
/// height above the ground (16 to 1,000 feet), then halves the last step
/// twenty times.
pub fn sight_ground(
    origin: Vector,
    line: Vector,
    ground: &impl Fn(f64, f64) -> f64,
) -> Option<Vector> {
    let at = |distance: f64| -> Vector { std::array::from_fn(|i| origin[i] + line[i] * distance) };
    let height = |p: Vector| p[1] - ground(p[0], p[2]);
    if height(origin) <= 0. {
        return None;
    }
    let mut near = 0.;
    let mut far = 0.;
    loop {
        let above = height(at(far));
        if above <= 0. {
            break;
        }
        if far >= SIGHT_RANGE_FT {
            return None;
        }
        near = far;
        far = (far + (above * 0.5).clamp(16., 1_000.)).min(SIGHT_RANGE_FT);
    }
    for _ in 0..20 {
        let middle = (near + far) * 0.5;
        if height(at(middle)) <= 0. {
            far = middle;
        } else {
            near = middle;
        }
    }
    Some(at(far))
}
/// Backslash: the object nearest the line of sight inside the pipper
/// circle's angular radius, then the nearest, then the lowest id. Friendly,
/// destroyed and terrain-masked objects are skipped. Any range.
pub fn pick(
    origin: Vector,
    line: Vector,
    zoom: u8,
    objects: &[SightObject],
    ground: &impl Fn(f64, f64) -> f64,
) -> Option<u32> {
    let radius = PICK_FIELD_FRACTION * field_of_view(zoom);
    objects
        .iter()
        .filter(|o| o.alive && !o.friendly)
        .filter_map(|o| {
            let offset = sub(o.position, origin);
            let range = dot(offset, offset).sqrt();
            let across = cross(line, offset);
            let angle = dot(across, across).sqrt().atan2(dot(line, offset));
            (angle <= radius && range > 0.).then_some((angle, range, o))
        })
        .filter(|(_, _, o)| terrain_hit(origin, short_of(origin, o.position), ground).is_none())
        .min_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then(a.2.id.cmp(&b.2.id))
        })
        .map(|(_, _, o)| o.id)
}
/// The point [`MASK_SHORT_FT`] short of `to` on the line from `from`.
fn short_of(from: Vector, to: Vector) -> Vector {
    let offset = sub(to, from);
    let length = dot(offset, offset).sqrt();
    if length <= MASK_SHORT_FT {
        return from;
    }
    let keep = (length - MASK_SHORT_FT) / length;
    std::array::from_fn(|i| from[i] + offset[i] * keep)
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
