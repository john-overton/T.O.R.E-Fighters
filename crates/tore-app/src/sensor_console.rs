//! The AC-130 sensor console: the full-screen black-and-white thermal
//! picture from the sensor turret, with its own symbology, shown on a CRT
//! (docs/spec/target-window.md, "AC-130 sensor console").
//!
//! Opinionated (John, 2026-10-13): "a mode that lets the player go into a
//! black/white optical vision screen ... flight information, target info if
//! relevant, azimuth, range ... a gimbal animation to represent visible
//! gimbal limits ... switch between white hot and black hot ... nice static
//! occasionally and a bit of chromatic aberration and pixelation to make it
//! look like a CRT display", in the manner of the real AC-130U sensor
//! display, made semi-realistic. The console is where the player works the
//! guns while the aircraft flies its orbit (John, 2026-10-13 21:50).
//!
//! The camera is the gunsight's own (the sensor dome's eye, the sight's look
//! carried between ticks, a tracked object framed as the target camera
//! frames it). The world draws thermally ([`crate::camera::Camera::thermal`]),
//! the symbology is drawn here into an overlay, and [`crate::crt`] shows both.
//! The gunsight works unchanged: slew, pin, designate, zoom, gun selection
//! and the trigger. Presentation only: nothing here reaches the simulation
//! or the wire; the console's T designates through the seat's existing
//! command (main.rs).
//!
//! John's follow-ups (2026-10-14, slice nr-conb): no periodic static; a box
//! on the designated target that hides under the reticle; the A-G radar on
//! the gimbal graphic with a 5 / 10 / 25 nm range (Ctrl+0) and T stepping
//! through the contacts the sensor sees, each zoomed to fill; and the TV and
//! night vision pictures beside white hot and black hot.
//!
//! Layout and wording are agent decisions (nr-fltv, 2026-10-13), labels kept
//! close to real sensor conventions where they are known (WHOT/BHOT, wide,
//! medium and narrow fields of view, AGL), the rest our own data; original
//! drawing.
use crate::camera::{Camera, Locate};
use crate::instruments::gunsight::Page;
use tore_sim::sensors::GroundSymbol;
use tore_sim::{
    attitude::Vector,
    combat::live::Launcher,
    combat::{
        gunship::{self, Sight},
        gunship_impact::Impact,
        gunsight,
        live::Readiness,
    },
};

/// Whether the console shows for this cockpit: the AC-130's sensor console
/// is open, which is orbit hold engaged (slice nr-orbit: Ctrl+9 or Select +
/// Guide opens the console and engages the orbit; again hands the aircraft
/// back). `flight` is the presented flight: a client's prediction carries
/// the autopilot's mode from the host's exact own-plane state, so a client
/// seat shows the console exactly while the host flies the orbit.
pub fn console_active(
    readout: &tore_world::readout::CockpitReadout,
    flight: &tore_sim::flight::State,
) -> bool {
    readout.gunsight.is_some() && flight.autopilot.mode() == tore_sim::autopilot::Mode::Orbit
}

/// The orbit autopilot as the console shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitStatus {
    /// On the circle (within a quarter of the radius and 30 degrees of the
    /// circle's heading), else still joining it.
    pub settled: bool,
    /// The planned radius and height above the orbit's centre, feet.
    pub radius_ft: f64,
    pub height_ft: f64,
    /// Every gun and the camera can bear on the sight's point.
    pub bearing: bool,
}

/// Orbit hold's status for this flight (`flight::State::orbit_status`), while
/// it is engaged.
pub fn orbit_status(flight: &tore_sim::flight::State) -> Option<OrbitStatus> {
    flight.orbit_status().map(|s| OrbitStatus {
        settled: s.on_orbit,
        radius_ft: s.radius_ft,
        height_ft: s.height_ft,
        bearing: s.all_bearing(),
    })
}

/// The console's picture: the thermal camera in white hot or black hot, the
/// TV camera's ordinary picture, or night vision (an image intensifier:
/// visible light amplified, not heat). Ctrl+Backslash steps through them in
/// that order (John, 2026-10-14: "switch between white hot, dark hot, and
/// regular visual", then "night vision mode, that is not thermal").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SensorMode {
    #[default]
    WhiteHot,
    BlackHot,
    Tv,
    NightVision,
}
impl SensorMode {
    /// The next mode on the key.
    pub fn cycled(self) -> Self {
        match self {
            Self::WhiteHot => Self::BlackHot,
            Self::BlackHot => Self::Tv,
            Self::Tv => Self::NightVision,
            Self::NightVision => Self::WhiteHot,
        }
    }
    /// The display's own label.
    pub fn label(self) -> &'static str {
        match self {
            Self::WhiteHot => "WHOT",
            Self::BlackHot => "BHOT",
            Self::Tv => "TV",
            Self::NightVision => "NVG",
        }
    }
    /// The sensor's word in the top left block: IR for the thermal camera,
    /// TV for the television camera, NVG for the intensified picture (the
    /// same word as the mode label, so the two blocks never disagree).
    pub fn sensor(self) -> &'static str {
        match self {
            Self::WhiteHot | Self::BlackHot => "IR",
            Self::Tv => "TV",
            Self::NightVision => "NVG",
        }
    }
    /// The world draws thermally ([`crate::camera::Camera::thermal`]).
    pub fn thermal(self) -> bool {
        matches!(self, Self::WhiteHot | Self::BlackHot)
    }
}

/// The console's presentation state on this machine: the picture mode, the
/// static schedule and the radar's last range. Not saved, not sent.
#[derive(Clone, Debug)]
pub struct Console {
    pub mode: SensorMode,
    pub noise: crate::crt::Static,
    /// The console was showing last frame: its opening turns the A-G radar
    /// on once.
    pub was_open: bool,
    /// The console's own wall clock, for the static and the blink.
    epoch: std::time::Instant,
}
impl Default for Console {
    fn default() -> Self {
        let mut noise = crate::crt::Static::new(0x0ac1_3005, 0.);
        // No periodic static bursts on the console (John, 2026-10-14: "an
        // occasional console flicker, let's remove that"); a requested burst
        // still works.
        noise.occasional = false;
        Self {
            mode: SensorMode::WhiteHot,
            noise,
            was_open: false,
            epoch: std::time::Instant::now(),
        }
    }
}
impl Console {
    /// Seconds on the console's clock.
    pub fn seconds(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }
    /// This frame's CRT settings at `seconds` on the frame clock; `night`
    /// is how dark the scene is ([`night`]), which sets the night vision's
    /// gain.
    pub fn crt_frame(&mut self, seconds: f64, night: f32) -> crate::crt::Frame {
        crt_frame(seconds, self.noise.level(seconds), self.mode, night)
    }
}

/// How dark the scene is at `altitude_ft`, 0 by day to 1 at night: the sun
/// from 2 degrees above to 6 below the horizon, the rule the night glows and
/// the countermeasures' night light use (`device_night` in the shaders).
pub fn night(world: &tore_world::terrain::Terrain, altitude_ft: f64) -> f32 {
    let sun = world
        .weather
        .sample(altitude_ft)
        .as_ref()
        .and_then(|layer| crate::celestial::visual_sun_direction(layer, &world.weather))
        .map_or(-1., |sun| sun[1]);
    let (low, high) = (-0.104_528_f32, 0.034_899_5_f32);
    let t = ((sun - low) / (high - low)).clamp(0., 1.);
    1. - t * t * (3. - 2. * t)
}

/// The thermal display's gain and lift: the picture's levels stretched as a
/// thermal camera's automatic gain would (agent decision, fitted on renders).
pub const GAIN: f32 = 1.5;
pub const LIFT: f32 = -0.2;

/// Night vision (agent decision, fitted on renders): the intensifier's
/// automatic gain, from the day's (which still overexposes the picture: an
/// intensifier is made for the dark) to the night's, by how dark the scene
/// is ([`night`]).
pub const NVG_GAIN: (f32, f32) = (6., 200.);
/// The phosphor's green (P43-like, agent decision).
pub const NVG_TINT: [f32; 3] = [0.42, 1.0, 0.5];
/// The intensifier's grain at full gain on top of the CRT's own noise, and
/// the bloom strength round lights and fires.
pub const NVG_GRAIN: f32 = 0.05;
pub const NVG_BLOOM: f32 = 1.6;

/// The console's CRT settings for a frame: the sensor look, the static
/// level, the mode and the display gain. `night` is how dark the scene is
/// (only night vision reads it).
pub fn crt_frame(
    seconds: f64,
    static_level: f32,
    mode: SensorMode,
    night: f32,
) -> crate::crt::Frame {
    let mut frame = crate::crt::Frame::new(crate::crt::SENSOR, seconds, static_level);
    match mode {
        SensorMode::WhiteHot | SensorMode::BlackHot => {
            frame.invert = mode == SensorMode::BlackHot;
            frame.gain = GAIN;
            frame.lift = LIFT;
        }
        // The television camera's ordinary picture in its own colours
        // (agent decision: John asked for "regular visual").
        SensorMode::Tv => {
            frame.settings.monochrome = false;
            frame.colour = true;
        }
        SensorMode::NightVision => {
            let gain = nvg_gain(night);
            frame.gain = gain;
            frame.tint = NVG_TINT;
            frame.colour = true;
            frame.bloom = NVG_BLOOM;
            frame.shoulder = true;
            // Grain grows with the gain: the darker the scene, the noisier.
            let dark = (gain - NVG_GAIN.0) / (NVG_GAIN.1 - NVG_GAIN.0);
            frame.settings.noise += NVG_GRAIN * dark.clamp(0., 1.);
        }
    }
    frame
}

/// The night vision's gain for how dark the scene is, 0 day to 1 night.
pub fn nvg_gain(night: f32) -> f32 {
    NVG_GAIN.0 + (NVG_GAIN.1 - NVG_GAIN.0) * night.clamp(0., 1.)
}

/// The console radar's range settings: the flight A-G radar's first three
/// ladder steps, 5, 10 and 25 nm, 25 the console's maximum (John,
/// 2026-10-14). The console's range is the flight radar's own range setting
/// (`Instruments::radar_range`), capped here, so the two always match.
pub const RADAR_RANGES: usize = 3;
/// The range index the console shows for the flight radar's range index.
pub fn radar_range_index(radar_range: usize) -> usize {
    radar_range.min(RADAR_RANGES - 1)
}
/// The console's range key: the next of 5, 10 and 25 nm, wrapping.
pub fn next_radar_range(radar_range: usize) -> usize {
    (radar_range_index(radar_range) + 1) % RADAR_RANGES
}
/// Nautical miles for a console range index.
pub fn radar_range_nm(index: usize) -> f64 {
    tore_sim::sensors::RANGE_LADDER_NMI[radar_range_index(index)]
}

/// A point on the gimbal graphic, which is a plan view about the aircraft
/// (nose up): its bearing from the nose (radians, clockwise) and its
/// distance over the ground, feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanPoint {
    pub bearing: f64,
    pub ground_ft: f64,
}
impl PlanPoint {
    /// A world point seen from the aircraft at `origin` heading `heading`.
    pub fn of(origin: Vector, heading: f64, point: Vector) -> Self {
        let (dx, dz) = (point[0] - origin[0], point[2] - origin[2]);
        Self {
            bearing: sight_wrap(dx.atan2(dz) - heading),
            ground_ft: dx.hypot(dz),
        }
    }
    /// Where a line from the aircraft along world direction `line` meets
    /// level ground `agl_ft` below; a line at or above the horizon never
    /// does (infinitely far, drawn on the rim).
    pub fn along(heading: f64, agl_ft: f64, line: Vector) -> Self {
        let level = line[0].hypot(line[2]);
        let down = -line[1];
        Self {
            bearing: sight_wrap(line[0].atan2(line[2]) - heading),
            ground_ft: if down > 1e-6 {
                agl_ft.max(0.) * level / down
            } else {
                f64::INFINITY
            },
        }
    }
}
fn sight_wrap(angle: f64) -> f64 {
    tore_sim::combat::sight::wrap(angle)
}

/// The gimbal graphic's geometry, all as plan points: the sensor's aim, the
/// guns' trains, the lead gun's sector outline and the 45 degree ring.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub sensor: PlanPoint,
    pub trains: [PlanPoint; 3],
    /// The lead gun's sector, a closed outline.
    pub arc: Vec<PlanPoint>,
    /// The ground distance 45 degrees below the horizon: the height.
    pub ring_ft: f64,
}
impl Plan {
    /// The plan for the sensor at `look` with the guns at `trains`, `aim`
    /// the sight's point (the line's ground point when it has none) and
    /// `slot` the gun whose sector shows.
    pub fn new(
        launcher: Launcher,
        agl_ft: f64,
        look: [f64; 2],
        trains: [[f64; 2]; 3],
        aim: Option<Vector>,
        slot: Option<usize>,
    ) -> Self {
        let f = launcher.basis.forward;
        let heading = f[0].atan2(f[2]);
        let along =
            |h: f64, e: f64| PlanPoint::along(heading, agl_ft, gunship::direction(launcher, h, e));
        let sensor = aim.map_or_else(
            || along(look[0], look[1]),
            |a| PlanPoint::of(launcher.position, heading, a),
        );
        let trains = trains.map(|t| along(t[0], t[1].min(gunship::GIMBAL_TOP)));
        let arc = slot.map_or_else(Vec::new, |slot| {
            let neutral = -std::f64::consts::FRAC_PI_2;
            let (half_h, half_e) = (gunship::HEADING_ARC[slot], gunship::ELEVATION_ARC[slot]);
            let steps = 16;
            let mut outline = Vec::new();
            // Out along one edge, round the inner (steepest) arc, back out
            // along the other edge.
            for i in 0..=steps / 2 {
                let e = gunship::GIMBAL_TOP - (gunship::GIMBAL_TOP + half_e) * f64::from(i) / 8.;
                outline.push(along(neutral - half_h, e));
            }
            for i in 1..steps {
                let h = neutral - half_h + 2. * half_h * f64::from(i) / f64::from(steps);
                outline.push(along(h, -half_e));
            }
            for i in (0..=steps / 2).rev() {
                let e = gunship::GIMBAL_TOP - (gunship::GIMBAL_TOP + half_e) * f64::from(i) / 8.;
                outline.push(along(neutral + half_h, e));
            }
            outline
        });
        Self {
            sensor,
            trains,
            arc,
            ring_ft: agl_ft.max(0.),
        }
    }
}

/// One ground contact on the console radar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadarDot {
    pub at: PlanPoint,
    pub symbol: GroundSymbol,
    /// The designated target.
    pub selected: bool,
}

/// The console's air-to-ground radar: the flight A-G radar's own ground
/// list (the same detection rules and limits, host-built and on the wire),
/// inside the console's range.
#[derive(Clone, Debug, PartialEq)]
pub struct Radar {
    /// The A-G radar is on.
    pub on: bool,
    pub range_nm: f64,
    pub dots: Vec<RadarDot>,
}
impl Radar {
    /// The radar for this readout: the A-G page's contacts within
    /// `range_index`'s range of the aircraft at `launcher`.
    pub fn of(
        sensors: &tore_world::readout::SensorReadout,
        launcher: Launcher,
        range_index: usize,
        designated: Option<u32>,
    ) -> Self {
        let range_nm = radar_range_nm(range_index);
        let range_ft = range_nm * tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
        let f = launcher.basis.forward;
        let heading = f[0].atan2(f[2]);
        let on = sensors.air_ground;
        let dots = sensors
            .ground
            .iter()
            .filter(|_| on)
            .filter(|g| g.contact.channel == tore_sim::sensors::Channel::Radar)
            .map(|g| RadarDot {
                at: PlanPoint::of(launcher.position, heading, g.contact.position),
                symbol: g.symbol(),
                selected: designated == Some(g.id()),
            })
            .filter(|d| d.at.ground_ft <= range_ft)
            .collect();
        Self { on, range_nm, dots }
    }
}

/// The ground contacts T steps through in the console: on the A-G radar's
/// list, inside the console's range over the ground, not friendly, inside
/// the sensor's gimbal (at or below the aircraft's horizontal plane) and with
/// a clear terrain line from the sensor dome (`clear`, the sight's own
/// rule). Nearest the dome first, as the flight T.
pub fn in_view(
    sensors: &tore_world::readout::SensorReadout,
    launcher: Launcher,
    range_index: usize,
    clear: impl Fn(Vector, Vector) -> bool,
    friendly: impl Fn(u32) -> bool,
) -> Vec<u32> {
    if !sensors.air_ground {
        return Vec::new();
    }
    let range_ft = radar_range_nm(range_index) * tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
    let eye = gunship::eye_position(launcher);
    let f = launcher.basis.forward;
    let heading = f[0].atan2(f[2]);
    let mut ranked: Vec<(f64, u32)> = sensors
        .ground
        .iter()
        .filter(|g| g.contact.channel == tore_sim::sensors::Channel::Radar)
        .filter(|g| !g.contact.destroyed && !friendly(g.id()))
        .filter(|g| {
            PlanPoint::of(launcher.position, heading, g.contact.position).ground_ft <= range_ft
        })
        .filter_map(|g| {
            let p = g.contact.position;
            let offset: Vector = std::array::from_fn(|i| p[i] - eye[i]);
            let angles = tore_sim::combat::sight::body_angles(launcher, offset);
            (angles[1] <= gunship::GIMBAL_TOP && clear(eye, p)).then(|| {
                (
                    (0..3).map(|i| offset[i] * offset[i]).sum::<f64>().sqrt(),
                    g.id(),
                )
            })
        })
        .collect();
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    ranked.into_iter().map(|(_, id)| id).collect()
}

/// T (forward) or Shift+T: the one after `current` in `list`, wrapping; with
/// no current one on the list, the first (or the last).
pub fn cycle(list: &[u32], current: Option<u32>, forward: bool) -> Option<u32> {
    if list.is_empty() {
        return None;
    }
    let n = list.len();
    Some(
        match current.and_then(|id| list.iter().position(|v| *v == id)) {
            Some(i) if forward => list[(i + 1) % n],
            Some(i) => list[(i + n - 1) % n],
            None if forward => list[0],
            None => list[n - 1],
        },
    )
}

/// The size the zoom assumes for a target with no box of its own (an
/// aircraft), feet (agent decision).
pub const DEFAULT_EXTENT_FT: f64 = 60.;
/// How much of the picture's height a selected target fills at most when
/// the console zooms to it (agent decision).
pub const FILL: f64 = 0.6;
/// The zoom step the console's T leaves a target at: one step wider than
/// the narrowest step whose field still holds the target (`extent_ft`
/// across at `slant_ft`) within [`FILL`] of the picture's height, for some
/// context round it (John, 2026-10-14); the widest when none does.
pub fn fill_zoom(extent_ft: f64, slant_ft: f64) -> u8 {
    (1..=tore_sim::combat::sight::ZOOM_STEPS)
        .rev()
        .find(|&step| 2. * slant_ft * (gunship::field_of_view(step) / 2.).tan() * FILL >= extent_ft)
        .map_or(1, |step| step.saturating_sub(1).max(1))
}

/// The designated target's box: half its side as a share of the picture's
/// height. It hides while the reticle's centre lies inside it, that is while
/// the target is within this distance of the centre on both axes (agent
/// decision for "it disappears when the main reticle is over the target").
pub const BOX_HALF: f64 = 0.04;
/// Whether the box at screen point `p` shows on a picture of `size`.
pub fn box_shows(p: (f64, f64), size: [u32; 2]) -> bool {
    let (w, h) = (f64::from(size[0]), f64::from(size[1]));
    let half = h * BOX_HALF;
    (p.0 - w / 2.).abs() > half || (p.1 - h / 2.).abs() > half
}

/// How the sight holds its line, as the console's mode word says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Free slew: "MAN".
    Manual,
    /// Travelling back to the default view.
    Returning,
    /// A pinned ground point.
    Point,
    /// A tracked object.
    Track,
}
impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::Manual => "MAN",
            Self::Returning => "RTN",
            Self::Point => "PNT",
            Self::Track => "TRK",
        }
    }
}

/// The field-of-view name for a zoom step: wide, medium and narrow optical
/// fields, then the narrow field's electronic zoom (agent decision; the
/// AN/AAQ-26 class of sensor has three optical fields).
pub fn field_name(step: u8) -> &'static str {
    match step {
        0 | 1 => "WFOV",
        2 => "MFOV",
        3 => "NFOV",
        4 => "NFOV E2",
        5 => "NFOV E4",
        _ => "NFOV E8",
    }
}

/// One gun's line in the list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunLine {
    pub label: &'static str,
    pub fitted: bool,
    pub linked: bool,
    pub candidate: bool,
    pub rounds: Option<u32>,
    pub status: Readiness,
}

/// The designated target's lines.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetLine {
    /// Its type, when the sensors name it.
    pub name: String,
    pub range_ft: f64,
    /// Clock bearing with HI/LO.
    pub clock: String,
    /// True bearing from the aircraft, degrees.
    pub bearing_deg: f64,
}

/// The pipper: the lead gun's predicted impact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pipper {
    pub point: Vector,
    /// The share of the circle the range arc covers.
    pub range_arc: f64,
    pub dashed: bool,
    pub dot: bool,
}

/// Everything the console's symbology shows, in plain numbers, so the live
/// game and the offscreen renders draw from the same thing.
#[derive(Clone, Debug, PartialEq)]
pub struct Symbology {
    pub sensor: SensorMode,
    pub mode: Mode,
    pub zoom: u8,
    /// Vertical field of view, degrees.
    pub fov_deg: f64,
    /// The aircraft: true heading (degrees), height over the ground below
    /// and airspeed (true, knots).
    pub heading_deg: f64,
    pub agl_ft: f64,
    pub airspeed_kt: f64,
    /// The sensor's body-relative [heading, elevation], radians.
    pub look: [f64; 2],
    /// True bearing of the line of sight, degrees.
    pub los_deg: f64,
    /// Slant range from the sensor to the aim point.
    pub slant_ft: Option<f64>,
    pub target: Option<TargetLine>,
    pub guns: [GunLine; 3],
    /// The lead gun (the candidate, else the first linked one).
    pub lead: Option<usize>,
    /// Each gun's actual [heading, elevation], radians.
    pub trains: [[f64; 2]; 3],
    /// The sensor sits at its gimbal limit.
    pub gimbal_limit: bool,
    pub pipper: Option<Pipper>,
    /// The other linked guns' impacts.
    pub others: Vec<Vector>,
    /// The pinned point or the tracked object, and whether tracked.
    pub mark: Option<(Vector, bool)>,
    /// A one-off notice ("NO GROUND POINT", "L TO DROP").
    pub notice: Option<&'static str>,
    pub orbit: Option<OrbitStatus>,
    /// The designated target (the gunsight's track or any other
    /// designation), where the box goes.
    pub designated: Option<Vector>,
    /// The air-to-ground radar on the gimbal graphic.
    pub radar: Radar,
    /// The gimbal graphic's geometry.
    pub plan: Plan,
    /// The console's autopilot (slice nr-cona): SEARCH, ORBIT JOIN or ORBIT
    /// and the commanded heading, altitude and speed
    /// (`flight::State::console_status`).
    pub autopilot: Option<tore_sim::autopilot::orbit::ConsoleStatus>,
    /// The blink phase for flashing marks (on half of each half second).
    pub blink: bool,
}

/// How close to the camera's limit (degrees of elevation from the top) the
/// gimbal graphic starts to warn (agent decision).
pub const LIMIT_WARN_DEG: f64 = 10.;

impl Symbology {
    /// The console's data from the gunsight page and the flight.
    #[allow(clippy::too_many_arguments)]
    pub fn live(
        page: &Page,
        target: Option<&crate::target_window::Readout>,
        readout: &tore_world::readout::CockpitReadout,
        config: &tore_sim::combat::live::Configuration,
        flight: &tore_sim::flight::State,
        air: Option<&tore_sim::telemetry::AirData>,
        ground_ft: f64,
        sensor: SensorMode,
        seconds: f64,
        designated: Option<Vector>,
        radar_range: usize,
    ) -> Self {
        let station = |slot: usize| {
            config
                .stations
                .iter()
                .position(|s| s.weapon.source.eq_ignore_ascii_case(gunship::GUNS[slot]))
        };
        let guns = std::array::from_fn(|slot| GunLine {
            label: gunship::NAMES[slot],
            fitted: page.guns[slot].fitted,
            linked: page.guns[slot].linked,
            candidate: page.candidate == Some(slot),
            rounds: station(slot).map(|i| u32::from(readout.stores.rounds(i))),
            status: page.guns[slot].status,
        });
        let lead = page.lead();
        let pipper = lead.and_then(|slot| {
            let impact = page.impacts[slot]?;
            let status = page.guns[slot].status;
            let spent = matches!(impact, Impact::Spent { .. });
            Some(Pipper {
                point: impact.point(),
                range_arc: if spent {
                    0.
                } else {
                    gunsight::range_arc_fraction(
                        impact.range_ft(),
                        page.guns[slot].maximum_range_ft,
                    )
                },
                dashed: status == Readiness::GunArc || status == Readiness::MaximumRange || spent,
                dot: !(spent || status == Readiness::MaximumRange),
            })
        });
        let others = (0..3)
            .filter(|slot| {
                Some(*slot) != lead && page.guns[*slot].linked && page.guns[*slot].fitted
            })
            .filter_map(|slot| page.impacts[slot].map(|i| i.point()))
            .collect();
        let tracked = matches!(page.sight, Sight::Tracked(_));
        let mode = match page.sight {
            Sight::Tracked(_) => Mode::Track,
            Sight::Pinned(_) => Mode::Point,
            Sight::Free if page.returning => Mode::Returning,
            Sight::Free => Mode::Manual,
        };
        let launcher = crate::combat::launcher(flight);
        let eye = gunship::eye_position(launcher);
        let line = gunship::direction(launcher, page.look[0], page.look[1]);
        let aim = readout.gunsight.as_ref().and_then(|g| g.aim);
        let distance =
            |a: Vector, b: Vector| (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt();
        let target = target.filter(|_| tracked).map(|t| {
            let at = readout
                .targets
                .display
                .as_ref()
                .map_or(flight.position, |row| row.position);
            TargetLine {
                name: tore_formats::text::upper(&t.name),
                range_ft: distance(at, flight.position),
                clock: t.bearing.clone(),
                bearing_deg: true_bearing(flight.position, at),
            }
        });
        let notice = page.notice.map(|n| match n {
            crate::instruments::gunsight::PageNotice::NoGroundPoint => "NO GROUND POINT",
            crate::instruments::gunsight::PageNotice::DropToSlew => "L TO DROP",
        });
        let agl_ft = air.map_or(flight.position[1] - ground_ft, |a| a.altitude_agl_ft);
        let plan = Plan::new(
            launcher,
            agl_ft,
            page.look,
            page.trains,
            aim,
            lead.or(page.guns.iter().position(|g| g.fitted)),
        );
        Self {
            sensor,
            mode,
            zoom: page.zoom,
            fov_deg: gunship::field_of_view(page.zoom).to_degrees(),
            heading_deg: air.map_or(flight.yaw.to_degrees(), |a| a.heading_true_deg),
            agl_ft,
            airspeed_kt: air.map_or(flight.speed / 1.68781, |a| a.true_airspeed_knots),
            look: page.look,
            los_deg: line[0].atan2(line[2]).to_degrees().rem_euclid(360.),
            slant_ft: aim.map(|a| distance(a, eye)),
            target,
            guns,
            lead,
            trains: page.trains,
            gimbal_limit: page.gimbal_limit,
            pipper,
            others,
            mark: page.mark.map(|m| (m, tracked)),
            notice,
            orbit: orbit_status(flight),
            designated,
            radar: Radar::of(
                &readout.sensors,
                launcher,
                radar_range,
                readout.targets.designated,
            ),
            plan,
            autopilot: flight.console_status(),
            blink: (seconds * 2.).rem_euclid(1.) < 0.5,
        }
    }

    /// The status line: the camera limit, else the lead gun's state.
    pub fn status(&self) -> String {
        if self.gimbal_limit {
            return "GIMBAL LIMIT".into();
        }
        match self.lead {
            Some(slot) => format!(
                "{} {}",
                self.guns[slot].label,
                short(self.guns[slot].status)
            ),
            None => "NO GUN".into(),
        }
    }

    /// How near the sensor is to its elevation limit, 0 clear to 1 at it.
    pub fn limit_nearness(&self) -> f64 {
        let top = gunship::GIMBAL_TOP.to_degrees();
        let gap = top - self.look[1].to_degrees();
        if self.gimbal_limit {
            1.
        } else {
            (1. - gap / LIMIT_WARN_DEG).clamp(0., 1.)
        }
    }
}

/// True bearing from `from` to `to`, degrees (0 north, clockwise).
fn true_bearing(from: Vector, to: Vector) -> f64 {
    (to[0] - from[0])
        .atan2(to[2] - from[2])
        .to_degrees()
        .rem_euclid(360.)
}

/// A gun state as the list's short word.
fn short(status: Readiness) -> &'static str {
    match status {
        Readiness::Ready => "RDY",
        Readiness::GunSlewing => "SLEW",
        Readiness::GunArc => "NO ARC",
        Readiness::MaximumRange => "MAX RNG",
        Readiness::MinimumRange => "MIN RNG",
        Readiness::TerrainMask => "MASKED",
        Readiness::GunObscured => "BLOCKED",
        Readiness::Empty => "EMPTY",
        Readiness::GroupEmpty => "EMPTY",
        Readiness::Safe => "SAFE",
        Readiness::StationFailed => "FAIL",
        other => other.label(),
    }
}

// ---------------------------------------------------------------------------
// Drawing. The overlay is straight-alpha sRGB RGBA at the output size: pale
// ink with a dark outline, so it reads on white-hot and black-hot alike.
// ---------------------------------------------------------------------------

const INK: [u8; 4] = [226, 226, 222, 240];
const DARK: [u8; 4] = [10, 10, 10, 215];

/// A mask raster: 0 clear, 1 ink, 2 dark fill (text knocked out of a box).
struct Mask {
    w: i32,
    h: i32,
    cells: Vec<u8>,
}
impl Mask {
    fn new(size: [u32; 2]) -> Self {
        Self {
            w: size[0] as i32,
            h: size[1] as i32,
            cells: vec![0; size[0] as usize * size[1] as usize],
        }
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, v: u8) {
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in x.max(0)..(x + w).min(self.w) {
                self.cells[(yy * self.w + xx) as usize] = v;
            }
        }
    }
    /// A line `t` pixels thick.
    fn line(&mut self, a: (f64, f64), b: (f64, f64), t: i32) {
        let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 2.)
            .ceil()
            .max(1.) as i32;
        let half = t / 2;
        for i in 0..=steps {
            let f = f64::from(i) / f64::from(steps);
            let x = (a.0 + (b.0 - a.0) * f).round() as i32;
            let y = (a.1 + (b.1 - a.1) * f).round() as i32;
            self.rect(x - half, y - half, t, t, 1);
        }
    }
    /// A circle arc, radius `r`, from `start` to `end` (radians clockwise
    /// from twelve o'clock), `t` thick; `dash` draws every other 30 degrees.
    fn arc(&mut self, c: (f64, f64), r: f64, start: f64, end: f64, t: i32, dash: bool) {
        let steps = ((end - start).abs() * r * 1.5).ceil().max(8.) as i32;
        let half = t / 2;
        for i in 0..=steps {
            let a = start + (end - start) * f64::from(i) / f64::from(steps);
            if dash && (a.to_degrees() / 30.).rem_euclid(2.) >= 1. {
                continue;
            }
            let x = (c.0 + r * a.sin()).round() as i32;
            let y = (c.1 - r * a.cos()).round() as i32;
            self.rect(x - half, y - half, t, t, 1);
        }
    }
    fn circle(&mut self, c: (f64, f64), r: f64, t: i32, dash: bool) {
        self.arc(c, r, 0., std::f64::consts::TAU, t, dash);
    }
    /// Text in the console font, its top left at (x, y), `s` pixels a font
    /// pixel, in `v`.
    fn text(&mut self, text: &str, x: i32, y: i32, s: i32, v: u8) {
        let mut pen = x;
        for c in text.chars() {
            if let Some(rows) = glyph(c) {
                for (row, bits) in rows.iter().enumerate() {
                    for col in 0..5 {
                        if bits & (0x10 >> col) != 0 {
                            self.rect(pen + col * s, y + row as i32 * s, s, s, v);
                        }
                    }
                }
            }
            pen += ADVANCE * s;
        }
    }
    /// The overlay's pixels: ink, its dark outline `o` pixels wide, and the
    /// dark fills.
    fn pixels(&self, o: i32) -> Vec<u8> {
        let mut out = vec![0u8; (self.w * self.h * 4) as usize];
        for y in 0..self.h {
            for x in 0..self.w {
                let at = (y * self.w + x) as usize;
                let v = self.cells[at];
                let color = match v {
                    1 => Some(INK),
                    2 => Some(DARK),
                    _ => {
                        let mut near = false;
                        'search: for dy in -o..=o {
                            for dx in -o..=o {
                                let (nx, ny) = (x + dx, y + dy);
                                if nx >= 0
                                    && ny >= 0
                                    && nx < self.w
                                    && ny < self.h
                                    && self.cells[(ny * self.w + nx) as usize] == 1
                                {
                                    near = true;
                                    break 'search;
                                }
                            }
                        }
                        near.then_some(DARK)
                    }
                };
                if let Some(c) = color {
                    out[at * 4..at * 4 + 4].copy_from_slice(&c);
                }
            }
        }
        out
    }
}

/// A glyph's advance in font pixels (5 wide plus a gap).
const ADVANCE: i32 = 6;
/// A text row's height in font pixels (7 tall plus a gap).
const ROW: i32 = 10;

fn text_width(text: &str, s: i32) -> i32 {
    text.chars().count() as i32 * ADVANCE * s - s
}

/// The console's own 5 x 7 block font (original): rows top to bottom, the
/// high bit leftmost.
fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c.to_ascii_uppercase() {
        ' ' => [0; 7],
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1c, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1c],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0a, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        '.' => [0, 0, 0, 0, 0, 0x0c, 0x0c],
        ',' => [0, 0, 0, 0, 0x0c, 0x04, 0x08],
        ':' => [0, 0x0c, 0x0c, 0, 0x0c, 0x0c, 0],
        '-' => [0, 0, 0, 0x1f, 0, 0, 0],
        '+' => [0, 0x04, 0x04, 0x1f, 0x04, 0x04, 0],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '\'' => [0x04, 0x04, 0x08, 0, 0, 0, 0],
        '*' => [0x0c, 0x12, 0x12, 0x0c, 0, 0, 0],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        '#' => [0x0a, 0x0a, 0x1f, 0x0a, 0x1f, 0x0a, 0x0a],
        _ => return None,
    })
}

/// Pixels per font pixel for an output `height`: 4 at 1080 rows.
fn text_scale(height: u32) -> i32 {
    ((f64::from(height) / 270.).round() as i32).max(1)
}

/// The console's overlay for `camera` at `size`: straight-alpha sRGB RGBA.
pub fn draw(sym: &Symbology, camera: &Camera, size: [u32; 2]) -> Vec<u8> {
    let mut m = Mask::new(size);
    let (w, h) = (f64::from(size[0]), f64::from(size[1]));
    let s = text_scale(size[1]);
    // Line weight and outline: 2 pixels each at 1080 rows.
    let t = ((h / 540.).round() as i32).max(1);
    let o = ((h / 540.).round() as i32).max(1);
    let row = ROW * s;
    let left = (w * 0.06) as i32;
    let right = (w * 0.94) as i32;
    let top = (h * 0.10) as i32;
    let right_text = |m: &mut Mask, text: &str, y: i32| {
        m.text(text, right - text_width(text, s), y, s, 1);
    };

    // Top left: mode, field of view, sensor.
    m.text(
        &format!(
            "{}  {}  {}",
            sym.mode.label(),
            field_name(sym.zoom),
            sym.sensor.sensor()
        ),
        left,
        top,
        s,
        1,
    );
    m.text(&format!("FOV {:.1}*", sym.fov_deg), left, top + row, s, 1);
    // The console's flight status (slice nr-cona's `console_status`): the
    // phase, then in SEARCH the commanded heading, altitude and speed the
    // autopilot flies straight on, holding a point the orbit's radius and
    // height and the commanded speed.
    if let Some(ap) = sym.autopilot {
        use tore_sim::autopilot::orbit::ConsolePhase;
        let state = match (ap.phase, sym.orbit.map(|o| o.bearing)) {
            (ConsolePhase::Search, _) => "SEARCH  P PINS".to_string(),
            (ConsolePhase::Orbit, Some(true)) => "ORBIT  GUNS".to_string(),
            (ConsolePhase::Orbit, Some(false)) => "ORBIT  NO BEAR".to_string(),
            (phase, _) => phase.label().to_string(),
        };
        m.text(&state, left, top + 2 * row, s, 1);
        let values = match (ap.phase, ap.orbit) {
            (ConsolePhase::Search, _) | (_, None) => format!(
                "HDG {:03.0}  ALT {:.0}  {:.0}KT",
                ap.heading_deg.rem_euclid(360.),
                ap.altitude_ft,
                ap.speed_kt
            ),
            (_, Some(orbit)) => format!(
                "R {:.1}NM  H {:.0}  {:.0}KT",
                orbit.radius_ft / tore_sim::sensors::FEET_PER_NAUTICAL_MILE,
                ap.orbit_height_ft,
                ap.speed_kt
            ),
        };
        m.text(&values, left, top + 3 * row, s, 1);
    } else if let Some(orbit) = sym.orbit {
        let state = match (orbit.settled, orbit.bearing) {
            (false, _) => "ORBIT JOIN",
            (true, true) => "ORBIT  GUNS",
            (true, false) => "ORBIT  NO BEAR",
        };
        m.text(state, left, top + 2 * row, s, 1);
        m.text(
            &format!(
                "R {:.1}NM  H {:.0}",
                orbit.radius_ft / tore_sim::sensors::FEET_PER_NAUTICAL_MILE,
                orbit.height_ft
            ),
            left,
            top + 3 * row,
            s,
            1,
        );
    }

    // Top right: heading, height, airspeed, polarity.
    right_text(
        &mut m,
        &format!("HDG {:03.0}", sym.heading_deg.rem_euclid(360.)),
        top,
    );
    right_text(&mut m, &format!("{:.0} AGL", sym.agl_ft.max(0.)), top + row);
    right_text(
        &mut m,
        &format!("{:.0} KTAS", sym.airspeed_kt.max(0.)),
        top + 2 * row,
    );
    right_text(&mut m, sym.sensor.label(), top + 3 * row);

    // Left, middle: the sensor's angles.
    let az = sym.look[0].to_degrees().rem_euclid(360.);
    let el = sym.look[1].to_degrees();
    let mid = (h * 0.42) as i32;
    m.text(&format!("AZ {az:03.0}"), left, mid, s, 1);
    m.text(&format!("EL {el:+03.0}"), left, mid + row, s, 1);
    m.text(
        &format!("LOS {:03.0}", sym.los_deg),
        left,
        mid + 2 * row,
        s,
        1,
    );

    // Right, middle: the target.
    if let Some(target) = &sym.target {
        let name: String = target.name.chars().take(12).collect();
        right_text(&mut m, &format!("TGT {name}"), mid);
        right_text(
            &mut m,
            &format!(
                "RNG {:.1}NM",
                target.range_ft / tore_sim::sensors::FEET_PER_NAUTICAL_MILE
            ),
            mid + row,
        );
        right_text(
            &mut m,
            &format!("BRG {:03.0}", target.bearing_deg),
            mid + 2 * row,
        );
        right_text(&mut m, &target.clock, mid + 3 * row);
    }

    // Bottom left: the gun list; the candidate in a box, linked guns marked.
    let list = (h * 0.70) as i32;
    for (i, gun) in sym.guns.iter().enumerate() {
        let y = list + i as i32 * (row + s);
        let rounds = gun.rounds.map_or("----".to_string(), |r| format!("{r:4}"));
        let text = format!(
            "{}{:<5} {}",
            if gun.linked { ">" } else { " " },
            gun.label,
            rounds
        );
        if !gun.fitted {
            m.text(&text, left, y, s, 1);
            let width = text_width(&text, s);
            m.line(
                (f64::from(left), f64::from(y + 3 * s)),
                (f64::from(left + width), f64::from(y + 3 * s)),
                t,
            );
            continue;
        }
        if gun.candidate {
            let width = text_width(&text, s);
            m.rect(left - 2 * s, y - 2 * s, width + 4 * s, 11 * s, 1);
            m.text(&text, left, y, s, 2);
        } else {
            m.text(&text, left, y, s, 1);
        }
    }

    // Bottom centre: status and slant range.
    let status = sym.notice.map_or_else(|| sym.status(), str::to_string);
    let bottom = (h * 0.86) as i32;
    let flash = !sym.gimbal_limit || sym.blink;
    if flash {
        m.text(
            &status,
            (w / 2.) as i32 - text_width(&status, s) / 2,
            bottom,
            s,
            1,
        );
    }
    let slant = sym.slant_ft.map_or("RNG ----".into(), |ft| {
        format!(
            "RNG {:.0}FT {:.1}NM",
            ft,
            ft / tore_sim::sensors::FEET_PER_NAUTICAL_MILE
        )
    });
    m.text(
        &slant,
        (w / 2.) as i32 - text_width(&slant, s) / 2,
        bottom + row,
        s,
        1,
    );

    // The reticle.
    reticle(&mut m, (w / 2., h / 2.), h, t);

    // The mark, the other guns' impacts and the pipper.
    let place = |point: Vector| match camera.locate(size, point) {
        Some(Locate::On(p)) => Some((p[0], p[1])),
        _ => None,
    };
    if let Some((point, tracked)) = sym.mark
        && let Some(c) = place(point)
    {
        let half = h * if tracked { 0.03 } else { 0.018 };
        square(&mut m, c, half, t, tracked);
    }
    // The designated target's box, hidden while the reticle is over it; off
    // the picture, a small arrow at the edge points to it.
    if let Some(point) = sym.designated {
        match camera.locate(size, point) {
            Some(Locate::On(p)) => {
                let c = (p[0], p[1]);
                if box_shows(c, size) {
                    square(&mut m, c, h * BOX_HALF, t, true);
                }
            }
            Some(Locate::Edge(d)) => {
                let (cx, cy) = (w / 2., h / 2.);
                let k = ((cx - h * 0.05) / d[0].abs().max(1e-9))
                    .min((cy - h * 0.05) / d[1].abs().max(1e-9));
                let tip = (cx + d[0] * k, cy + d[1] * k);
                let len = h * 0.03;
                let back = (tip.0 - d[0] * len, tip.1 - d[1] * len);
                let side = (-d[1] * len * 0.6, d[0] * len * 0.6);
                m.line(tip, (back.0 + side.0, back.1 + side.1), t);
                m.line(tip, (back.0 - side.0, back.1 - side.1), t);
            }
            None => {}
        }
    }
    for point in &sym.others {
        if let Some(c) = place(*point) {
            diamond(&mut m, c, h * 0.012, t);
        }
    }
    if let Some(pipper) = sym.pipper {
        let at = match camera.locate(size, pipper.point) {
            Some(Locate::On(p)) => Some((p[0], p[1])),
            Some(Locate::Edge(d)) => {
                // Parked at the edge of the picture in its direction.
                let (cx, cy) = (w / 2., h / 2.);
                let k = ((cx - h * 0.08) / d[0].abs().max(1e-9))
                    .min((cy - h * 0.08) / d[1].abs().max(1e-9));
                Some((cx + d[0] * k, cy + d[1] * k))
            }
            None => None,
        };
        if let Some(c) = at {
            pipper_ring(&mut m, c, h, t, pipper);
        }
    }

    // Bottom right: the gimbal.
    gimbal(&mut m, sym, (w * 0.86, h * 0.72), h * GIMBAL_RADIUS, t, s);

    m.pixels(o)
}

/// An open crosshair: four arms with a clear centre, ticks along each arm
/// and a bar across each end.
fn reticle(m: &mut Mask, c: (f64, f64), h: f64, t: i32) {
    let thin = (t / 2).max(1);
    let gap = h * 0.035;
    let spacing = h * 0.055;
    let tick = h * 0.010;
    let end = h * 0.022;
    for (dx, dy, length) in [
        (1., 0., h * 0.25),
        (-1., 0., h * 0.25),
        (0., 1., h * 0.20),
        (0., -1., h * 0.20),
    ] {
        m.line(
            (c.0 + dx * gap, c.1 + dy * gap),
            (c.0 + dx * length, c.1 + dy * length),
            thin,
        );
        let mut d = gap + spacing;
        while d < length - spacing * 0.5 {
            let p = (c.0 + dx * d, c.1 + dy * d);
            m.line(
                (p.0 - dy * tick, p.1 - dx * tick),
                (p.0 + dy * tick, p.1 + dx * tick),
                thin,
            );
            d += spacing;
        }
        let p = (c.0 + dx * length, c.1 + dy * length);
        m.line(
            (p.0 - dy * end, p.1 - dx * end),
            (p.0 + dy * end, p.1 + dx * end),
            thin,
        );
    }
}

/// A square around a point: corners only for a pin, whole for a track.
fn square(m: &mut Mask, c: (f64, f64), half: f64, t: i32, whole: bool) {
    let corners = [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.)];
    for (i, (ax, ay)) in corners.iter().enumerate() {
        let (bx, by) = corners[(i + 1) % 4];
        let a = (c.0 + ax * half, c.1 + ay * half);
        let b = (c.0 + bx * half, c.1 + by * half);
        if whole {
            m.line(a, b, t);
        } else {
            let k = 0.35;
            m.line(a, (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k), t);
            m.line((b.0 + (a.0 - b.0) * k, b.1 + (a.1 - b.1) * k), b, t);
        }
    }
}

fn diamond(m: &mut Mask, c: (f64, f64), r: f64, t: i32) {
    let p = [(0., -r), (r, 0.), (0., r), (-r, 0.)];
    for i in 0..4 {
        let (a, b) = (p[i], p[(i + 1) % 4]);
        m.line((c.0 + a.0, c.1 + a.1), (c.0 + b.0, c.1 + b.1), t);
    }
}

/// The LCOS pipper at full size: a thin ring, a bolder range arc outside it
/// from twelve o'clock clockwise, and a centre dot.
fn pipper_ring(m: &mut Mask, c: (f64, f64), h: f64, t: i32, pipper: Pipper) {
    let r = h * 0.045;
    m.circle(c, r, (t / 2).max(1), pipper.dashed);
    if pipper.range_arc > 0. {
        m.arc(
            c,
            r + f64::from(t) * 1.5,
            0.,
            std::f64::consts::TAU * pipper.range_arc.min(1.),
            t,
            false,
        );
    }
    if pipper.dot {
        let d = t + 1;
        m.rect(c.0 as i32 - d / 2, c.1 as i32 - d / 2, d, d, 1);
    }
}

/// The gimbal graphic's radius, a share of the picture's height.
pub const GIMBAL_RADIUS: f64 = 0.11;

/// The gimbal and the radar: a plan view about the aircraft, the nose up,
/// out to the radar's range at the rim (agent decision: one scale for the
/// radar's dots and the sensor, so a dot under the sensor's square is what
/// the sensor sees). The centre is straight down, the rim the radar's range
/// and the camera's limit (a line at the horizon meets the ground beyond
/// any range), the dashed ring 45 degrees down. A small cross is the
/// aircraft. The lead gun's sector, the linked guns' trains and an arm to a
/// square at the sensor's aim move as they slew. The A-G radar's contacts
/// show as the A-G page draws them (dots, an X for SAM and AAA, a square for
/// an aircraft), the designated one between two bars. Near the camera's
/// limit the rim beside the sensor thickens; at it, it flashes with LIMIT
/// under the graphic. Under it, the radar's state and range.
fn gimbal(m: &mut Mask, sym: &Symbology, c: (f64, f64), r: f64, t: i32, s: i32) {
    let range_ft = sym.radar.range_nm * tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
    let scale = |ground_ft: f64| {
        if ground_ft.is_finite() {
            r * (ground_ft / range_ft).clamp(0., 1.)
        } else {
            r
        }
    };
    let at = |p: PlanPoint| {
        let k = scale(p.ground_ft);
        (c.0 + k * p.bearing.sin(), c.1 - k * p.bearing.cos())
    };
    let thin = (t / 2).max(1);
    m.circle(c, r, thin, false);
    let ring = scale(sym.plan.ring_ft);
    if ring > f64::from(3 * t) && ring < r {
        m.circle(c, ring, thin, true);
    }
    // The aircraft: a small cross, the nose up.
    let k = r * 0.1;
    m.line((c.0, c.1 - k), (c.0, c.1 + k), thin);
    m.line((c.0 - k * 0.8, c.1), (c.0 + k * 0.8, c.1), thin);
    // The lead gun's sector.
    for pair in sym.plan.arc.windows(2) {
        m.line(at(pair[0]), at(pair[1]), thin);
    }
    // The guns' trains: small open diamonds.
    for (slot, gun) in sym.guns.iter().enumerate() {
        if gun.fitted && gun.linked {
            diamond(m, at(sym.plan.trains[slot]), f64::from(2 * t), thin);
        }
    }
    // The sensor: an arm from the centre and corner marks round its aim,
    // open in the middle so the contacts under it show.
    let p = at(sym.plan.sensor);
    m.line(c, p, thin);
    square(m, p, r * 0.07, thin, false);
    // The radar's contacts, over the rest.
    for dot in &sym.radar.dots {
        let p = at(dot.at);
        let (x, y) = (p.0 as i32, p.1 as i32);
        match dot.symbol {
            GroundSymbol::Dot => m.rect(x - t, y - t, 2 * t, 2 * t, 1),
            GroundSymbol::Cross => {
                let a = f64::from(2 * t + 1);
                m.line((p.0 - a, p.1 - a), (p.0 + a, p.1 + a), thin);
                m.line((p.0 - a, p.1 + a), (p.0 + a, p.1 - a), thin);
            }
            GroundSymbol::Square => m.rect(x - 2 * t, y - 2 * t, 4 * t, 4 * t, 1),
        }
        if dot.selected {
            let gap = if dot.symbol == GroundSymbol::Dot {
                4 * t
            } else {
                6 * t
            };
            for side in [-1, 1] {
                m.rect(x + side * gap - thin / 2, y - 3 * t, thin.max(2), 6 * t, 1);
            }
        }
    }
    // Near the limit: the rim beside the sensor thickens; at it, it flashes.
    let near = sym.limit_nearness();
    let bearing = sym.plan.sensor.bearing;
    if near > 0. && (!sym.gimbal_limit || sym.blink) {
        let span = 0.35 + 0.25 * near;
        let weight = t + (near * f64::from(2 * t)).round() as i32;
        m.arc(c, r, bearing - span, bearing + span, weight, false);
    }
    // Labels.
    let centred = |m: &mut Mask, text: &str, y: i32| {
        m.text(
            text,
            (c.0 - f64::from(text_width(text, s)) / 2.) as i32,
            y,
            s,
            1,
        );
    };
    centred(m, "GIMBAL", (c.1 - r) as i32 - ROW * s);
    let radar = if sym.radar.on {
        format!("A-G {:.0}NM", sym.radar.range_nm)
    } else {
        "A-G OFF".to_string()
    };
    centred(m, &radar, (c.1 + r) as i32 + 3 * s);
    if sym.gimbal_limit && sym.blink {
        centred(m, "LIMIT", (c.1 + r) as i32 + 3 * s + ROW * s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Symbology {
        let gun = |label, candidate| GunLine {
            label,
            fitted: true,
            linked: true,
            candidate,
            rounds: Some(100),
            status: Readiness::Ready,
        };
        Symbology {
            sensor: SensorMode::WhiteHot,
            mode: Mode::Manual,
            zoom: 3,
            fov_deg: 7.5,
            heading_deg: 247.,
            agl_ft: 6301.,
            airspeed_kt: 180.,
            look: gunship::DEFAULT_LOOK,
            los_deg: 157.,
            slant_ft: Some(11_800.),
            target: None,
            guns: [gun("25MM", false), gun("40MM", false), gun("105MM", true)],
            lead: Some(2),
            trains: [gunship::DEFAULT_LOOK; 3],
            gimbal_limit: false,
            pipper: None,
            others: Vec::new(),
            mark: None,
            notice: None,
            orbit: None,
            designated: None,
            radar: Radar {
                on: true,
                range_nm: 10.,
                dots: vec![RadarDot {
                    at: PlanPoint {
                        bearing: -1.2,
                        ground_ft: 30_000.,
                    },
                    symbol: GroundSymbol::Cross,
                    selected: true,
                }],
            },
            plan: Plan {
                sensor: PlanPoint {
                    bearing: -1.5,
                    ground_ft: 9_000.,
                },
                trains: [PlanPoint {
                    bearing: -1.5,
                    ground_ft: 9_000.,
                }; 3],
                arc: Vec::new(),
                ring_ft: 6_300.,
            },
            autopilot: None,
            blink: true,
        }
    }

    #[test]
    fn every_label_the_console_writes_has_a_glyph() {
        let text = "MAN RTN PNT TRK WFOV MFOV NFOV E2 E4 E8 IR FOV 7.5* HDG AGL KTAS WHOT \
                    BHOT TV NVG A-G 25NM AZ EL LOS TGT RNG NM BRG 9:00 LO HI > 25MM 40MM 105MM ---- RDY SLEW \
                    NO ARC MAX MIN MASKED BLOCKED EMPTY SAFE FAIL GIMBAL LIMIT FT ORBIT JOIN \
                    OFF R H GUNS BEAR 0123456789 + - / ( ) % #";
        for c in text.chars() {
            assert!(glyph(c).is_some(), "{c:?}");
        }
        // Every Readiness label the list may fall back to.
        for status in [
            Readiness::NoTarget,
            Readiness::TrackLost,
            Readiness::Capacity,
        ] {
            for c in status.label().chars() {
                assert!(glyph(c).is_some(), "{c:?} in {}", status.label());
            }
        }
    }

    #[test]
    fn the_status_says_the_limit_first_then_the_lead_gun() {
        let mut sym = sample();
        assert_eq!(sym.status(), "105MM RDY");
        sym.guns[2].status = Readiness::GunArc;
        assert_eq!(sym.status(), "105MM NO ARC");
        sym.gimbal_limit = true;
        assert_eq!(sym.status(), "GIMBAL LIMIT");
    }

    #[test]
    fn the_limit_warning_grows_over_the_last_ten_degrees() {
        let mut sym = sample();
        sym.look[1] = -30_f64.to_radians();
        assert_eq!(sym.limit_nearness(), 0.);
        sym.look[1] = -5_f64.to_radians();
        assert!((sym.limit_nearness() - 0.5).abs() < 1e-9);
        sym.look[1] = 0.;
        assert!((sym.limit_nearness() - 1.).abs() < 1e-9);
    }

    #[test]
    fn the_fields_are_named_wide_medium_narrow_then_zoomed() {
        assert_eq!(
            (1..=6).map(field_name).collect::<Vec<_>>(),
            ["WFOV", "MFOV", "NFOV", "NFOV E2", "NFOV E4", "NFOV E8"]
        );
    }

    #[test]
    fn the_overlay_is_ink_with_a_dark_edge_and_clear_elsewhere() {
        let mut camera = Camera::new();
        camera.position = [0., 6000., 0.];
        camera.pitch = -0.4;
        let size = [480, 270];
        let pixels = draw(&sample(), &camera, size);
        assert_eq!(pixels.len(), 480 * 270 * 4);
        let ink = pixels.chunks_exact(4).filter(|p| p == &INK).count();
        let dark = pixels.chunks_exact(4).filter(|p| p == &DARK).count();
        let clear = pixels.chunks_exact(4).filter(|p| p[3] == 0).count();
        assert!(ink > 1000, "{ink}");
        assert!(dark > ink / 2, "{dark}");
        // Most of the picture stays clear for the sensor image.
        assert!(clear > 480 * 270 * 3 / 4, "{clear}");
        // The centre of the reticle is open.
        let centre = ((135 * 480 + 240) * 4) as usize;
        assert_eq!(pixels[centre + 3], 0);
    }

    #[test]
    fn the_key_steps_white_hot_black_hot_tv_night_vision() {
        let m = SensorMode::default();
        let labels: Vec<_> = std::iter::successors(Some(m), |m| Some(m.cycled()))
            .take(5)
            .map(SensorMode::label)
            .collect();
        assert_eq!(labels, ["WHOT", "BHOT", "TV", "NVG", "WHOT"]);
        let mut console = Console::default();
        let day = 0.;
        let whot = console.crt_frame(0., day);
        assert!(!whot.invert && whot.settings.monochrome);
        console.mode = SensorMode::BlackHot;
        assert!(console.crt_frame(0., day).invert);
        // TV: the scene's own colours, still through the CRT.
        console.mode = SensorMode::Tv;
        let tv = console.crt_frame(0., day);
        assert!(!tv.settings.monochrome && tv.colour && !tv.invert);
        assert_eq!((tv.gain, tv.lift, tv.tint), (1., 0., [1.; 3]));
        assert!(tv.settings.aberration_px > 0. && tv.settings.scanline > 0.);
        // Night vision: green, gain and grain grow as the light fades, and
        // the gain stays between its limits.
        console.mode = SensorMode::NightVision;
        let by_day = console.crt_frame(0., day);
        let by_night = console.crt_frame(0., 1.);
        assert_eq!(by_day.tint, NVG_TINT);
        assert!(by_day.settings.monochrome && by_day.bloom > 0. && by_day.shoulder);
        assert!(!tv.shoulder && !whot.shoulder);
        assert!(by_night.gain > by_day.gain);
        assert!(by_night.settings.noise > by_day.settings.noise);
        assert_eq!(nvg_gain(0.), NVG_GAIN.0);
        assert_eq!(nvg_gain(1.), NVG_GAIN.1);
        assert!(!SensorMode::Tv.thermal() && !SensorMode::NightVision.thermal());
        assert_eq!(
            [
                SensorMode::WhiteHot,
                SensorMode::BlackHot,
                SensorMode::Tv,
                SensorMode::NightVision
            ]
            .map(SensorMode::sensor),
            ["IR", "IR", "TV", "NVG"]
        );
        assert!(SensorMode::WhiteHot.thermal() && SensorMode::BlackHot.thermal());
    }

    #[test]
    fn the_console_has_no_periodic_static() {
        let mut console = Console::default();
        for i in 0..6000 {
            assert_eq!(console.noise.level(f64::from(i) * 0.05), 0.);
        }
        // A requested burst still shows.
        console.noise.burst(400., 1.);
        assert_eq!(console.noise.level(400.5), 1.);
    }

    #[test]
    fn the_range_key_cycles_5_10_25_and_caps_the_flight_radar_at_25() {
        assert_eq!(
            (0..3).map(radar_range_nm).collect::<Vec<_>>(),
            [5., 10., 25.]
        );
        assert_eq!([0, 1, 2, 3, 5].map(next_radar_range), [1, 2, 0, 0, 0]);
        // The flight radar at 50 or more shows as 25 in the console.
        assert_eq!(radar_range_nm(4), 25.);
    }

    #[test]
    fn t_cycles_wrapping_and_starts_at_the_nearest() {
        let list = [7, 3, 9];
        assert_eq!(cycle(&list, None, true), Some(7));
        assert_eq!(cycle(&list, None, false), Some(9));
        assert_eq!(cycle(&list, Some(7), true), Some(3));
        assert_eq!(cycle(&list, Some(9), true), Some(7));
        assert_eq!(cycle(&list, Some(7), false), Some(9));
        assert_eq!(cycle(&list, Some(42), true), Some(7));
        assert_eq!(cycle(&[], Some(7), true), None);
    }

    #[test]
    fn a_selected_target_is_zoomed_to_fill_within_the_ladder() {
        // A 30 ft tank at 9,000 ft: the narrowest field (0.94 degrees, 147
        // ft tall) holds it, and T stops one step wider, NFOV E4.
        assert_eq!(fill_zoom(30., 9_000.), 5);
        // A 250 ft hangar at the same range needs the field twice as wide
        // or more; again one step wider than the one that fills.
        let step = fill_zoom(250., 9_000.);
        assert!(step < 5);
        let field = |step| 2. * 9_000. * (gunship::field_of_view(step) / 2.).tan();
        assert!(field(step + 1) * FILL >= 250. && field(step + 2) * FILL < 250.);
        // Nothing fits closer than the widest field: the widest.
        assert_eq!(fill_zoom(5_000., 1_000.), 1);
    }

    #[test]
    fn the_box_hides_while_the_reticle_is_over_the_target() {
        let size = [1920, 1080];
        assert!(!box_shows((960., 540.), size));
        assert!(!box_shows((960. + 40., 540. - 40.), size));
        assert!(box_shows((960. + 50., 540.), size));
        assert!(box_shows((960., 540. + 50.), size));
    }

    #[test]
    fn plan_points_are_bearing_from_the_nose_and_ground_distance() {
        // Heading north; a point 3,000 ft east and 4,000 ft north.
        let p = PlanPoint::of([0., 5_000., 0.], 0., [3_000., 0., 4_000.]);
        assert!((p.ground_ft - 5_000.).abs() < 1e-6);
        assert!((p.bearing - 3_f64.atan2(4.)).abs() < 1e-9);
        // Heading east, the same point is 53 degrees left of the nose.
        let p = PlanPoint::of(
            [0., 5_000., 0.],
            std::f64::consts::FRAC_PI_2,
            [3_000., 0., 4_000.],
        );
        assert!((p.bearing + (std::f64::consts::FRAC_PI_2 - 3_f64.atan2(4.))).abs() < 1e-9);
        // 45 degrees down meets the ground the height away; level never.
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let p = PlanPoint::along(0., 6_000., [0., -s, s]);
        assert!((p.ground_ft - 6_000.).abs() < 1e-6);
        assert!(
            PlanPoint::along(0., 6_000., [1., 0., 0.])
                .ground_ft
                .is_infinite()
        );
    }

    #[test]
    fn the_overlay_draws_the_box_off_centre_and_not_on_the_reticle() {
        let mut camera = Camera::new();
        camera.position = [0., 6000., 0.];
        camera.yaw = 0.;
        camera.pitch = -0.4;
        let size = [480, 270];
        let ink = |sym: &Symbology| {
            draw(sym, &camera, size)
                .chunks_exact(4)
                .filter(|p| p == &INK)
                .count()
        };
        let plain = sample();
        let base = ink(&plain);
        // On the line of sight: hidden.
        let line = [0., -(0.4_f64).sin(), (0.4_f64).cos()];
        let mut sym = plain.clone();
        sym.designated = Some(std::array::from_fn(|i| {
            camera.position[i] + line[i] * 8_000.
        }));
        assert_eq!(ink(&sym), base);
        // Off to the side: drawn.
        let mut point = sym.designated.unwrap();
        point[0] += 1_500.;
        sym.designated = Some(point);
        assert!(ink(&sym) > base + 40);
    }
}
