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
//! or the wire.
//!
//! Layout and wording are agent decisions (nr-fltv, 2026-10-13), labels kept
//! close to real sensor conventions where they are known (WHOT/BHOT, wide,
//! medium and narrow fields of view, AGL), the rest our own data; original
//! drawing.
use crate::camera::{Camera, Locate};
use crate::instruments::gunsight::Page;
use tore_sim::{
    attitude::Vector,
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

/// White hot or black hot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Polarity {
    #[default]
    WhiteHot,
    BlackHot,
}
impl Polarity {
    pub fn toggled(self) -> Self {
        match self {
            Self::WhiteHot => Self::BlackHot,
            Self::BlackHot => Self::WhiteHot,
        }
    }
    /// The display's own label.
    pub fn label(self) -> &'static str {
        match self {
            Self::WhiteHot => "WHOT",
            Self::BlackHot => "BHOT",
        }
    }
}

/// The console's presentation state on this machine: polarity and the
/// static schedule. Not saved, not sent.
#[derive(Clone, Debug)]
pub struct Console {
    pub polarity: Polarity,
    pub noise: crate::crt::Static,
    /// The console's own wall clock, for the static and the blink.
    epoch: std::time::Instant,
}
impl Default for Console {
    fn default() -> Self {
        Self {
            polarity: Polarity::WhiteHot,
            noise: crate::crt::Static::new(0x0ac1_3005, 0.),
            epoch: std::time::Instant::now(),
        }
    }
}
impl Console {
    /// Seconds on the console's clock.
    pub fn seconds(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }
    /// This frame's CRT settings at `seconds` on the frame clock.
    pub fn crt_frame(&mut self, seconds: f64) -> crate::crt::Frame {
        crt_frame(seconds, self.noise.level(seconds), self.polarity)
    }
}

/// The console's display gain and lift: the picture's levels stretched as a
/// thermal camera's automatic gain would (agent decision, fitted on renders).
pub const GAIN: f32 = 1.5;
pub const LIFT: f32 = -0.2;

/// The console's CRT settings for a frame: the sensor look, the static
/// level, the polarity and the display gain.
pub fn crt_frame(seconds: f64, static_level: f32, polarity: Polarity) -> crate::crt::Frame {
    let mut frame = crate::crt::Frame::new(crate::crt::SENSOR, seconds, static_level);
    frame.invert = polarity == Polarity::BlackHot;
    frame.gain = GAIN;
    frame.lift = LIFT;
    frame
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
    pub polarity: Polarity,
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
        polarity: Polarity,
        seconds: f64,
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
        Self {
            polarity,
            mode,
            zoom: page.zoom,
            fov_deg: gunship::field_of_view(page.zoom).to_degrees(),
            heading_deg: air.map_or(flight.yaw.to_degrees(), |a| a.heading_true_deg),
            agl_ft: air.map_or(flight.position[1] - ground_ft, |a| a.altitude_agl_ft),
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
        &format!("{}  {}  IR", sym.mode.label(), field_name(sym.zoom)),
        left,
        top,
        s,
        1,
    );
    m.text(&format!("FOV {:.1}*", sym.fov_deg), left, top + row, s, 1);
    // SEARCH (no point held): the commanded heading, altitude and speed
    // the autopilot flies straight on (slice nr-cona).
    if let Some(ap) = sym.autopilot
        && ap.phase == tore_sim::autopilot::orbit::ConsolePhase::Search
    {
        m.text("SEARCH  P PINS", left, top + 2 * row, s, 1);
        m.text(
            &format!(
                "HDG {:03.0}  ALT {:.0}  {:.0}KT",
                ap.heading_deg, ap.altitude_ft, ap.speed_kt
            ),
            left,
            top + 3 * row,
            s,
            1,
        );
    }
    if let Some(orbit) = sym.orbit {
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
    right_text(&mut m, sym.polarity.label(), top + 3 * row);

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
    gimbal(&mut m, sym, (w * 0.86, h * 0.72), h * 0.095, t, s);

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

/// The gimbal: the lower hemisphere seen from above, the nose up. The rim
/// is the camera's limit (the aircraft's horizontal plane), the centre
/// straight down, the dashed ring 45 degrees down. A sector shows where the
/// lead gun can bear; the arm and the square are the sensor's line, dots the
/// guns' trains. Near the limit the rim beside the sensor thickens, and at
/// the limit it flashes with LIMIT beside it.
fn gimbal(m: &mut Mask, sym: &Symbology, c: (f64, f64), r: f64, t: i32, s: i32) {
    let top = gunship::GIMBAL_TOP;
    // Radius for an elevation: 0 at nadir, r at the camera's top.
    let radius = |elevation: f64| {
        r * ((elevation + std::f64::consts::FRAC_PI_2) / (top + std::f64::consts::FRAC_PI_2))
            .clamp(0., 1.)
    };
    let at = |heading: f64, elevation: f64| {
        let k = radius(elevation);
        (c.0 + k * heading.sin(), c.1 - k * heading.cos())
    };
    let thin = (t / 2).max(1);
    m.circle(c, r, thin, false);
    m.circle(c, radius(-std::f64::consts::FRAC_PI_4), thin, true);
    // The aircraft: a small cross, the nose up.
    let k = r * 0.12;
    m.line((c.0, c.1 - k), (c.0, c.1 + k), thin);
    m.line((c.0 - k * 0.8, c.1), (c.0 + k * 0.8, c.1), thin);
    // The lead gun's arc: heading about abeam left, elevation from its lower
    // limit up to the camera's top.
    if let Some(slot) = sym.lead.or(sym.guns.iter().position(|g| g.fitted)) {
        let neutral = -std::f64::consts::FRAC_PI_2;
        let (half_h, half_e) = (gunship::HEADING_ARC[slot], gunship::ELEVATION_ARC[slot]);
        let inner = radius(-half_e);
        m.arc(c, inner, neutral - half_h, neutral + half_h, thin, true);
        for heading in [neutral - half_h, neutral + half_h] {
            let a = (c.0 + inner * heading.sin(), c.1 - inner * heading.cos());
            let b = (c.0 + r * heading.sin(), c.1 - r * heading.cos());
            m.line(a, b, thin);
        }
    }
    // The guns' trains.
    for (slot, gun) in sym.guns.iter().enumerate() {
        if gun.fitted && gun.linked {
            let p = at(sym.trains[slot][0], sym.trains[slot][1].min(top));
            m.rect(p.0 as i32 - t, p.1 as i32 - t, 2 * t, 2 * t, 1);
        }
    }
    // The sensor: an arm from the centre and a square at its end.
    let p = at(sym.look[0], sym.look[1]);
    m.line(c, p, thin);
    square(m, p, r * 0.09, t, true);
    // Near the limit: the rim beside the sensor thickens; at it, it flashes.
    let near = sym.limit_nearness();
    if near > 0. && (!sym.gimbal_limit || sym.blink) {
        let span = 0.35 + 0.25 * near;
        let weight = t + (near * f64::from(2 * t)).round() as i32;
        m.arc(c, r, sym.look[0] - span, sym.look[0] + span, weight, false);
    }
    // Labels.
    let label = "GIMBAL";
    m.text(
        label,
        (c.0 - f64::from(text_width(label, s)) / 2.) as i32,
        (c.1 - r) as i32 - ROW * s,
        s,
        1,
    );
    if sym.gimbal_limit && sym.blink {
        let warn = "LIMIT";
        m.text(
            warn,
            (c.0 - f64::from(text_width(warn, s)) / 2.) as i32,
            (c.1 + r) as i32 + 3 * s,
            s,
            1,
        );
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
            polarity: Polarity::WhiteHot,
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
            autopilot: None,
            blink: true,
        }
    }

    #[test]
    fn every_label_the_console_writes_has_a_glyph() {
        let text = "MAN RTN PNT TRK WFOV MFOV NFOV E2 E4 E8 IR FOV 7.5* HDG AGL KTAS WHOT \
                    BHOT AZ EL LOS TGT RNG NM BRG 9:00 LO HI > 25MM 40MM 105MM ---- RDY SLEW \
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
    fn polarity_toggles_and_names_itself() {
        let p = Polarity::default();
        assert_eq!(p.label(), "WHOT");
        assert_eq!(p.toggled().label(), "BHOT");
        assert_eq!(p.toggled().toggled(), p);
        let mut console = Console::default();
        assert!(!console.crt_frame(0.).invert);
        console.polarity = Polarity::BlackHot;
        assert!(console.crt_frame(0.).invert);
    }
}
