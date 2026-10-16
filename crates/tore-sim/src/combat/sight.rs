//! The targeting sight: a gimballed camera line of sight that slews freely,
//! pins a ground point or tracks an object, on every aircraft with a laser
//! designator or a FLIR (docs/spec/air-to-ground.md, "The targeting sight").
//! It is the AC-130 gunsight generalised: the gunship keeps its own state
//! and steps through [`Sight::step`] with its dome sensor, so its behaviour
//! is unchanged (docs/spec/ac130-linked-guns.md, "The gunsight").
//!
//! Labels: the three modes, the keys, the ground march and the masking rule
//! are the AC-130 design (opinionated, John, 2026-10-09). The gimbal of each
//! sensor, the eye point and a fighter's default look are fitted from the
//! sensor records' fields (see [`SightSensor::from_record`]).
use super::{gunsight::TargetObservation, live::terrain_hit};
use crate::attitude::{Vector, cross, dot, unit};
use crate::combat::live::Launcher;
use std::f64::consts::PI;
use tore_formats::weapons::Seeker;

/// Free slew elevation stops short of straight down (fitted: avoids the
/// singular heading there). The camera's gimbal reaches nadir otherwise.
pub const LOOK_ELEVATION_LIMIT: f64 = 89. * PI / 180.;
/// The hemisphere gimbal's upper edge: the aircraft's horizontal plane.
pub const GIMBAL_TOP: f64 = 0.;
/// The target camera's zoom ladder: six steps, the widest 30 degrees tall,
/// each half the one before (fitted, agent choice; retail gives six steps).
pub const ZOOM_STEPS: u8 = 6;
pub const DEFAULT_ZOOM: u8 = 3;
const WIDEST_FIELD: f64 = 30. * PI / 180.;
/// Full deflection slews this fraction of the vertical field per second
/// (fitted).
pub const SLEW_FIELDS_PER_SECOND: f64 = 0.75;
/// The return to the default view travels at the fastest sight slew, full
/// deflection at the widest zoom step: 22.5 degrees a second, whatever the
/// zoom (agent choice; John, 2026-10-09: it never snaps).
pub const RETURN_PER_TICK: f64 = SLEW_FIELDS_PER_SECOND * WIDEST_FIELD / 120.;
/// The Backslash pick radius as a fraction of the field's height: the
/// pipper circle's 9 of the page's 114 pixels.
const PICK_FIELD_FRACTION: f64 = 9. / 114.;
/// How far along the line of sight the ground is searched (fitted: 40 nmi).
pub const SIGHT_RANGE_FT: f64 = 243_000.;
/// Terrain within this distance of a point on the ground does not mask it,
/// so a ground point is never masked by the ground it lies on (fitted).
pub const MASK_SHORT_FT: f64 = 25.;
/// A fighter's default look: straight ahead, 10 degrees down (fitted).
pub const FIGHTER_DEFAULT_LOOK: [f64; 2] = [0., -10. * PI / 180.];
/// A pod's eye sits this far below the aircraft's centre, feet (fitted: the
/// pod hangs under the fuselage).
pub const POD_EYE_DROP_FT: f64 = 4.;
/// Signatures of the sensor records that give a sight.
pub const LASER_SIGNATURE: u8 = 1;
pub const INFRARED_SIGNATURE: u8 = 2;

/// What the sight holds (opinionated, John, 2026-10-09).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SightMode {
    /// Body-relative look angles the pilot slews; the aim point is where the
    /// line of sight meets the ground.
    Free,
    /// A fixed world ground point. Slewing moves it.
    Pinned(Vector),
    /// An object, air or ground, followed at any range by its true position,
    /// like a pod track.
    Tracked(u32),
}
impl SightMode {
    pub fn tracked(self) -> Option<u32> {
        match self {
            Self::Tracked(id) => Some(id),
            Self::Free | Self::Pinned(_) => None,
        }
    }
    /// The sight holds a point: a track or a pin.
    pub fn holds(self) -> bool {
        !matches!(self, Self::Free)
    }
}

/// The seat's sight controls for a tick: normalized deflection (x right,
/// y up, -127 to 127) and the zoom step (1 to 6; 0 means the default step).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SightInput {
    pub deflection: [i8; 2],
    pub zoom: u8,
}

/// A sight key waiting for the next step, which has the terrain and the
/// objects to resolve it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SightRequest {
    /// Backslash: track the object under the crosshair, else pin the ground
    /// there.
    Designate,
    /// Shift+Backslash: pin the ground under the crosshair.
    Pin,
    /// `/`: track the next visible ground object to the right, wrapping to
    /// the leftmost; the first press takes the one nearest the line of
    /// sight (retail "IR/laser target"; the order is an agent choice that
    /// follows the Enter rule).
    Next,
}

/// A one-off message the sight raises for the target camera's activity line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// A pin was asked for with no ground on the line of sight.
    NoGroundPoint,
    /// The pilot slewed while tracking: L drops the target first.
    DropToSlew,
    /// The camera is stopped at the edge of its gimbal: the pilot is slewing
    /// against it, or the tracked target or pin lies outside it. Raised every
    /// tick it holds.
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

/// Where the camera can look, in the aircraft's own frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gimbal {
    /// The hemisphere below the aircraft: heading free through the full
    /// turn, elevation from the aircraft's horizontal plane down to nadir
    /// (the AC-130 dome and every external pod).
    Hemisphere,
    /// A cone about the nose for a built-in sensor: free slew within
    /// `search`, a track or pin followed (and lased) within `track`. Both are
    /// half-angles in radians, measured as the root sum square of the body
    /// heading and elevation (fitted).
    Cone { search: f64, track: f64 },
}
impl Gimbal {
    /// A look held inside the camera's travel.
    pub fn clamp(self, look: [f64; 2]) -> [f64; 2] {
        match self {
            Self::Hemisphere => [look[0], clamp_elevation(look[1])],
            Self::Cone { search, .. } => {
                let off = off_axis(look);
                let look = if off > search {
                    [look[0] * search / off, look[1] * search / off]
                } else {
                    look
                };
                [
                    look[0],
                    look[1].clamp(-LOOK_ELEVATION_LIMIT, LOOK_ELEVATION_LIMIT),
                ]
            }
        }
    }
    /// The bearing lies outside the camera's travel: the camera stops at its
    /// edge and the notice shows.
    pub fn beyond(self, look: [f64; 2]) -> bool {
        match self {
            Self::Hemisphere => look[1] > GIMBAL_TOP,
            Self::Cone { search, .. } => off_axis(look) > search,
        }
    }
    /// The sensor can follow (and a laser lase) a point at this bearing.
    pub fn follows(self, look: [f64; 2]) -> bool {
        match self {
            Self::Hemisphere => look[1] <= GIMBAL_TOP,
            Self::Cone { track, .. } => off_axis(look) <= track,
        }
    }
    /// A pin's slew from its own bearing `look` (which may lie outside the
    /// gimbal) to the asked-for `raw`: it may not move further outside than
    /// it already is. Returns the turned look and whether the gimbal cut the
    /// slew.
    pub fn slew_pin(self, look: [f64; 2], raw: [f64; 2]) -> ([f64; 2], bool) {
        match self {
            Self::Hemisphere => {
                let mut turned = raw;
                turned[1] = turned[1].clamp(-LOOK_ELEVATION_LIMIT, look[1].max(GIMBAL_TOP));
                (turned, raw[1] > turned[1])
            }
            Self::Cone { search, .. } => {
                let limit = off_axis(look).max(search);
                let off = off_axis(raw);
                if off > limit {
                    ([raw[0] * limit / off, raw[1] * limit / off], true)
                } else {
                    (raw, false)
                }
            }
        }
    }
}
/// The angle of a body-relative look from the nose, in the fitted cone
/// measure.
fn off_axis(look: [f64; 2]) -> f64 {
    look[0].hypot(look[1])
}

/// The sensor a sight looks through: whether it lases, how far, its gimbal,
/// its eye point and its default look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SightSensor {
    /// A laser designator (signature 1): the sight can lase.
    pub laser: bool,
    /// The record's zone 0 maximum range, feet: the laser's reach (10 nmi on
    /// every retail designator) or the FLIR's.
    pub range_ft: f64,
    pub gimbal: Gimbal,
    /// The eye in the aircraft's frame, host mount order (right, up,
    /// forward), feet.
    pub eye: Vector,
    /// The look the sight starts in and L returns to, [heading, elevation].
    pub default_look: [f64; 2],
}
impl SightSensor {
    /// The sight of a laser (signature 1) or FLIR (signature 2) sensor
    /// record, `None` for any other. A record with weight (an external
    /// store: Pave Spike, Pave Knife, AAS-38) or an all-round zone is a pod
    /// with the hemisphere gimbal; a built-in sensor gets a cone of its zone
    /// 0 (search) and zone 1 (track) headings (fitted from the SEE fields).
    pub fn from_record(seeker: &Seeker, weight: i32) -> Option<Self> {
        if !matches!(seeker.signature, LASER_SIGNATURE | INFRARED_SIGNATURE) {
            return None;
        }
        let angle = |v: i16| f64::from(v).abs() * PI / 32760.;
        let search = angle(seeker.zones[0].heading);
        let pod = weight > 0 || search >= PI / 2.;
        let track = match angle(seeker.zones[1].heading) {
            t if t > 0. => t.min(search),
            _ => search,
        };
        Some(Self {
            laser: seeker.signature == LASER_SIGNATURE,
            range_ft: f64::from(seeker.zones[0].maximum_range.max(0)),
            gimbal: if pod || search <= 0. {
                Gimbal::Hemisphere
            } else {
                Gimbal::Cone { search, track }
            },
            eye: if pod {
                [0., -POD_EYE_DROP_FT, 0.]
            } else {
                [0.; 3]
            },
            default_look: FIGHTER_DEFAULT_LOOK,
        })
    }
}

/// The laser pod the Load Ordnance page offers on a pod station (decision 7:
/// the pod loads on any pod station from the Air-to-Surface group, as the
/// retail manual instructs). The Pave Knife is no longer offered: its data
/// equals the Pave Spike's (same weight, range and hemisphere), so it added
/// nothing (John, 2026-10-16); see [`pod_record`].
pub const DESIGNATOR_PODS: [&str; 1] = ["PAVESPK.SEE"];

/// Pod records the page no longer offers, with the record each now loads as
/// (agent decision, 2026-10-16: an old saved loadout, mission line or lobby
/// request naming the Pave Knife loads the Pave Spike on the same station
/// instead of failing).
pub const RETIRED_PODS: [(&str, &str); 1] = [("PAVEKNF.SEE", "PAVESPK.SEE")];

/// The record a pod named in a loadout loads as: the name itself, or the
/// pod that replaced it.
pub fn pod_record(name: &str) -> &str {
    RETIRED_PODS
        .iter()
        .find(|(old, _)| old.eq_ignore_ascii_case(name))
        .map_or(name, |(_, now)| now)
}

/// A laser designator pod as a loadable store: its record, its sight and
/// its weight.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignatorStore {
    pub source: String,
    /// The record's display name ("Pave Spike").
    pub name: String,
    pub sensor: SightSensor,
    pub weight_lbs: i32,
    /// The equipment flags byte (bit 1: may hang on a station with the
    /// station's own bit 1, the retail equipment rule).
    pub flags: u8,
}
impl DesignatorStore {
    /// A pod from its SEE record. Refuses a record that is not a laser
    /// designator with weight.
    pub fn parse(source: &str, bytes: &[u8]) -> tore_formats::Result<Self> {
        let refuse = || {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "not a laser designator pod",
            )
        };
        let seeker = Seeker::parse(source, bytes)?;
        let equipment = tore_formats::aircraft::Equipment::parse(source, bytes)?;
        let weight_lbs = equipment
            .fields
            .get("weight")
            .ok_or_else(refuse)?
            .number()?;
        let flags = equipment
            .fields
            .get("flags")
            .and_then(|f| f.number().ok())
            .unwrap_or(0) as u8;
        let sensor = SightSensor::from_record(&seeker, weight_lbs)
            .filter(|s| s.laser && weight_lbs > 0)
            .ok_or_else(refuse)?;
        Ok(Self {
            source: source.to_ascii_uppercase(),
            name: if equipment.name.is_empty() {
                source.to_ascii_uppercase()
            } else {
                equipment.name.clone()
            },
            sensor,
            weight_lbs,
            flags,
        })
    }
}

/// A designator pod loaded on a weapon station, which then holds no
/// weapon.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignatorPod {
    /// The weapon station (a `live::Configuration::stations` index).
    pub station: usize,
    pub store: DesignatorStore,
}

/// The sight of an aircraft's own sensor records: a laser designator first
/// (Pave Spike on the F-16C and A-10, SU24L on the Su-25, KA50L on the
/// V-22), else a FLIR (AAS-38 on the Rafale, the internal FLIRs).
pub fn fixed_sight(
    a: &tore_formats::aircraft::Aircraft,
    mut read: impl FnMut(&str) -> tore_formats::Result<Vec<u8>>,
) -> tore_formats::Result<Option<SightSensor>> {
    let mut found: Option<SightSensor> = None;
    for name in a
        .hardpoints
        .iter()
        .filter_map(|h| h.store.as_deref())
        .filter(|n| n.to_ascii_uppercase().ends_with(".SEE"))
    {
        let bytes = read(name)?;
        let seeker = Seeker::parse(name, &bytes)?;
        if !matches!(seeker.signature, LASER_SIGNATURE | INFRARED_SIGNATURE) {
            continue;
        }
        let weight = tore_formats::aircraft::Equipment::parse(name, &bytes)?
            .fields
            .get("weight")
            .and_then(|w| w.number().ok())
            .unwrap_or(0);
        let Some(sensor) = SightSensor::from_record(&seeker, weight) else {
            continue;
        };
        if found.is_none_or(|f| sensor.laser && !f.laser) {
            found = Some(sensor);
        }
    }
    Ok(found)
}

impl super::live::Configuration {
    /// The sensor the aircraft's sight looks through: a loaded designator
    /// pod first, else its own laser or FLIR record. `None` without either.
    pub fn sight_sensor(&self) -> Option<SightSensor> {
        self.designator_pods
            .first()
            .map(|pod| pod.store.sensor)
            .or(self.sight)
    }
    /// The loaded designator pod's record, if any.
    pub fn designator_pod(&self) -> Option<&DesignatorStore> {
        self.designator_pods.first().map(|pod| &pod.store)
    }
}

/// A targeting sight: what it holds, where it looks, the seat's controls
/// and the last notice. Stepped at the fixed tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sight {
    pub sensor: SightSensor,
    /// What the sight holds.
    pub mode: SightMode,
    /// Body-relative [heading, elevation] of the camera's line of sight from
    /// the eye, kept current in every mode so a drop continues from it. It
    /// never leaves the gimbal; a track or pin outside it leaves the camera
    /// at the limit while `aim` stays on the true point.
    pub look: [f64; 2],
    /// The sight is travelling back to the sensor's default look at
    /// [`RETURN_PER_TICK`].
    pub returning: bool,
    /// The point the sight marks this tick.
    pub aim: Option<Vector>,
    /// The seat's sight controls, held until the host sets them again.
    pub input: SightInput,
    /// The last step saw the sight deflected (a slew while tracking says
    /// [`Notice::DropToSlew`] once per push).
    pub slew_held: bool,
    pub request: Option<SightRequest>,
    pub notice: Option<SightNotice>,
}
impl Sight {
    /// A sight at the sensor's default look, holding nothing.
    pub fn new(sensor: SightSensor) -> Self {
        Self {
            sensor,
            mode: SightMode::Free,
            look: sensor.default_look,
            returning: false,
            aim: None,
            input: SightInput::default(),
            slew_held: false,
            request: None,
            notice: None,
        }
    }
    /// The tracked object, if the sight holds one.
    pub fn target(&self) -> Option<u32> {
        self.mode.tracked()
    }
    /// The zoom step in use, 1 to 6.
    pub fn zoom(&self) -> u8 {
        zoom_step(self.input.zoom)
    }
    /// The point a track or pin holds, from the last step.
    pub fn held_point(&self) -> Option<Vector> {
        self.mode.holds().then_some(self.aim).flatten()
    }
    /// Follow an object from now on (a T, Enter, scope click or Backslash).
    pub fn track(&mut self, id: u32) {
        self.mode = SightMode::Tracked(id);
        self.returning = false;
        self.request = None;
    }
    /// L: drop a target or pin and slew freely from the current look; with
    /// nothing held, travel back to the default view.
    pub fn drop_hold(&mut self) {
        if self.request.take().is_some() {
            return;
        }
        match self.mode {
            SightMode::Tracked(_) | SightMode::Pinned(_) => {
                self.mode = SightMode::Free;
                self.returning = false;
            }
            SightMode::Free => self.returning = self.look != self.sensor.default_look,
        }
    }
    /// The eye in the world: every sight ray starts here.
    pub fn eye_position(&self, launcher: Launcher) -> Vector {
        world_mount(launcher, self.sensor.eye)
    }
    /// Move the sight for one tick and work out the aim point: resolve a
    /// pending key, end a track whose object is gone (pinning the ground
    /// under the line of sight), slew or travel home, then find where the
    /// line of sight meets the ground. With no ground on the line the aim
    /// point lies `sky_range` along it. Returns the aim point and its
    /// velocity, and whether a pipper leads it (a tracked object that flies
    /// or moves).
    pub fn step(
        &mut self,
        launcher: Launcher,
        objects: &[SightObject],
        ground: &impl Fn(f64, f64) -> f64,
        sky_range: f64,
        tick: u64,
    ) -> (TargetObservation, bool) {
        let gimbal = self.sensor.gimbal;
        let origin = self.eye_position(launcher);
        let found = |id: u32| objects.iter().find(|o| o.id == id && o.alive);
        // Set when the camera is held at the gimbal's edge this tick.
        let mut limited = false;
        self.look = gimbal.clamp(self.look);
        // A track ends only when its object is destroyed or removed; the
        // sight then holds the ground it was looking at.
        if let SightMode::Tracked(id) = self.mode {
            match found(id) {
                Some(object) => {
                    let at = body_angles(launcher, sub(object.position, origin));
                    limited |= gimbal.beyond(at);
                    self.look = gimbal.clamp(at);
                }
                None => match sight_ground(
                    origin,
                    direction(launcher, self.look[0], self.look[1]),
                    ground,
                ) {
                    Some(point) => self.mode = SightMode::Pinned(point),
                    None => self.mode = SightMode::Free,
                },
            }
        }
        // The pin's own bearing from the eye, which may lie outside the
        // gimbal while the camera stops at its edge.
        let mut pin_look = [0.; 2];
        if let SightMode::Pinned(point) = self.mode {
            pin_look = body_angles(launcher, sub(point, origin));
            limited |= gimbal.beyond(pin_look);
            self.look = gimbal.clamp(pin_look);
        }
        let line = direction(launcher, self.look[0], self.look[1]);
        match (self.request.take(), self.mode) {
            (None, _) | (Some(SightRequest::Designate), SightMode::Tracked(_)) => {}
            (Some(SightRequest::Next), _) => {
                let current = self.mode.tracked();
                if let Some(id) = next_visible(
                    origin,
                    launcher,
                    line,
                    &self.sensor,
                    current,
                    objects,
                    ground,
                ) {
                    self.track(id);
                    let object = found(id).expect("a visible object is alive");
                    let at = body_angles(launcher, sub(object.position, origin));
                    self.look = gimbal.clamp(at);
                }
            }
            (Some(request), _) => {
                let picked = (request == SightRequest::Designate)
                    .then(|| pick(origin, line, self.zoom(), objects, ground))
                    .flatten();
                if let Some(id) = picked {
                    self.track(id);
                } else if let Some(point) = sight_ground(origin, line, ground) {
                    self.mode = SightMode::Pinned(point);
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
        match self.mode {
            SightMode::Tracked(_) => {
                if deflected && !self.slew_held {
                    self.notice = Some(SightNotice {
                        notice: Notice::DropToSlew,
                        tick,
                    });
                }
            }
            SightMode::Free => {
                if deflected {
                    self.returning = false;
                    let turned = slew_raw(self.look, deflection, self.zoom());
                    limited |= gimbal.beyond(turned);
                    self.look = gimbal.clamp(turned);
                } else if self.returning {
                    self.look = homeward_to(self.look, self.sensor.default_look);
                    self.returning = self.look != self.sensor.default_look;
                }
            }
            SightMode::Pinned(_) => {
                if deflected {
                    let raw = slew_raw(pin_look, deflection, self.zoom());
                    let (turned, cut) = gimbal.slew_pin(pin_look, raw);
                    limited |= cut;
                    // A slew the limit cancels leaves the pin where it is.
                    let moved =
                        wrap(turned[0] - pin_look[0]).abs() + (turned[1] - pin_look[1]).abs();
                    if moved > 1e-12
                        && let Some(point) =
                            sight_ground(origin, direction(launcher, turned[0], turned[1]), ground)
                    {
                        self.mode = SightMode::Pinned(point);
                        self.look = gimbal.clamp(turned);
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
        let tracked = self.mode.tracked().and_then(found);
        let led = tracked.is_some_and(|o| o.airborne || o.velocity != [0.; 3]);
        let (position, velocity) = match (self.mode, tracked) {
            (SightMode::Tracked(_), Some(object)) => (object.position, object.velocity),
            (SightMode::Pinned(point), _) => (point, [0.; 3]),
            (SightMode::Free | SightMode::Tracked(_), _) => {
                let line = direction(launcher, self.look[0], self.look[1]);
                let point = sight_ground(origin, line, ground)
                    .unwrap_or_else(|| std::array::from_fn(|i| origin[i] + line[i] * sky_range));
                (point, [0.; 3])
            }
        };
        self.aim = Some(position);
        (TargetObservation { position, velocity }, led)
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
/// The look angles after one tick of slew before any gimbal: what the
/// pilot's slew asks for. The heading rate is divided by the cosine of
/// elevation (floored at a quarter) so the picture moves at the same rate
/// across the screen.
pub fn slew_raw(look: [f64; 2], deflection: [f64; 2], zoom: u8) -> [f64; 2] {
    let rate = SLEW_FIELDS_PER_SECOND * field_of_view(zoom) / 120.;
    let heading = look[0] + rate * deflection[0] / look[1].cos().max(0.25);
    [wrap(heading), look[1] + rate * deflection[1]]
}
/// The elevation of a camera look held inside the hemisphere gimbal.
pub fn clamp_elevation(elevation: f64) -> f64 {
    elevation.clamp(-LOOK_ELEVATION_LIMIT, GIMBAL_TOP)
}
/// One tick of the travel back to `home`, along the straight line between
/// the two in heading and elevation.
pub fn homeward_to(look: [f64; 2], home: [f64; 2]) -> [f64; 2] {
    let dh = wrap(home[0] - look[0]);
    let de = home[1] - look[1];
    let distance = dh.hypot(de);
    if distance <= RETURN_PER_TICK {
        return home;
    }
    [
        wrap(look[0] + dh / distance * RETURN_PER_TICK),
        look[1] + de / distance * RETURN_PER_TICK,
    ]
}
/// An angle in (-pi, pi].
pub fn wrap(angle: f64) -> f64 {
    let wrapped = (angle + PI).rem_euclid(2. * PI) - PI;
    if wrapped == -PI { PI } else { wrapped }
}
fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}
/// Body-relative [heading, elevation] of a world offset, as the true bearing:
/// it can lie outside the camera's gimbal; only the straight-down
/// singularity is kept off.
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
            let angle = angle_from(line, offset);
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
/// The angle between the unit `line` and an offset.
fn angle_from(line: Vector, offset: Vector) -> f64 {
    let across = cross(line, offset);
    dot(across, across).sqrt().atan2(dot(line, offset))
}
/// `/`: the visible ground objects are the standing, unfriendly objects on
/// the ground within the sensor's range, inside its gimbal and with a clear
/// terrain line from the eye. With the current track among them, the next
/// one to its right by body heading, wrapping to the leftmost; otherwise
/// the one nearest the line of sight. Ties go to the nearer object, then the
/// lower id.
pub fn next_visible(
    origin: Vector,
    launcher: Launcher,
    line: Vector,
    sensor: &SightSensor,
    current: Option<u32>,
    objects: &[SightObject],
    ground: &impl Fn(f64, f64) -> f64,
) -> Option<u32> {
    let mut visible: Vec<(f64, f64, f64, u32)> = objects
        .iter()
        .filter(|o| o.alive && !o.friendly && !o.airborne)
        .filter_map(|o| {
            let offset = sub(o.position, origin);
            let range = dot(offset, offset).sqrt();
            let look = body_angles(launcher, offset);
            (range > 0. && range <= sensor.range_ft && !sensor.gimbal.beyond(look)).then_some((
                look[0],
                angle_from(line, offset),
                range,
                o,
            ))
        })
        .filter(|(.., o)| terrain_hit(origin, short_of(origin, o.position), ground).is_none())
        .map(|(heading, angle, range, o)| (heading, angle, range, o.id))
        .collect();
    let order = |a: &(f64, f64, f64, u32), b: &(f64, f64, f64, u32)| {
        a.0.total_cmp(&b.0)
            .then(a.2.total_cmp(&b.2))
            .then(a.3.cmp(&b.3))
    };
    visible.sort_by(order);
    match current.and_then(|id| visible.iter().position(|v| v.3 == id)) {
        Some(at) => visible.get((at + 1) % visible.len()).map(|v| v.3),
        None => visible
            .iter()
            .min_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then(a.2.total_cmp(&b.2))
                    .then(a.3.cmp(&b.3))
            })
            .map(|v| v.3),
    }
}
/// The point [`MASK_SHORT_FT`] short of `to` on the line from `from`.
pub fn short_of(from: Vector, to: Vector) -> Vector {
    let offset = sub(to, from);
    let length = dot(offset, offset).sqrt();
    if length <= MASK_SHORT_FT {
        return from;
    }
    let keep = (length - MASK_SHORT_FT) / length;
    std::array::from_fn(|i| from[i] + offset[i] * keep)
}
/// The terrain leaves the line from `eye` to a ground point open (the
/// [`MASK_SHORT_FT`] rule).
pub fn clear_line(eye: Vector, point: Vector, ground: &impl Fn(f64, f64) -> f64) -> bool {
    terrain_hit(eye, short_of(eye, point), ground).is_none()
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
pub fn world_mount(launcher: Launcher, mount: Vector) -> Vector {
    std::array::from_fn(|i| {
        launcher.position[i]
            + launcher.basis.right[i] * mount[0]
            + launcher.basis.up[i] * mount[1]
            + launcher.basis.forward[i] * mount[2]
    })
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "sight_checkpoint.rs"]
mod checkpoint;

#[cfg(test)]
#[path = "sight_tests.rs"]
mod tests;
