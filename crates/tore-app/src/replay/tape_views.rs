//! The replay's HUD Camera and Sensor View (nr-replayviews, 2026-10-16):
//! the rules that need no renderer, so they can be tested alone.
//!
//! Opinionated (John, 2026-10-16): "the HUD Camera, which is like an old
//! style VHS recording of the HUD (just forward view, no cockpit etc),
//! chromatic aberration, colour graded to an old VHS style recording", then
//! "the HUD view can be zoomed in a bit, but give it a nice VHS texture
//! smoothing"; and "the Sensor View, which is available if the aircraft has
//! a targeting pod installed ... the same console view from the AC-130 from
//! the aspect of the aircraft ... show aircraft-specific info and if they are
//! lasing, TTI on release of ordnance, buddy lasing". Everything here (the
//! keys, the framing, the order the sight line is chosen in, the recorder
//! lines) is an agent decision.
//!
//! A recording holds no sight pointing (where a pod looked, its zoom or its
//! picture mode): the Sensor View works the line of sight out from what it
//! does hold (the laser spot, the designation, a guided weapon's aim, the
//! AC-130's orbit point, a landing) and says which on the picture.
use super::ground_marks::{Scene, Timed};
use crate::sensor_console::SensorMode;
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    attitude::Vector,
    combat::sight::{self, Gimbal, SightSensor},
};

/// The HUD camera and the Sensor View: view numbers after the object view.
pub const HUD_CAMERA: u8 = 13;
pub const SENSOR: u8 = 14;

/// The share of the picture's height the HUD's drawn area fills in the HUD
/// camera: framed on the HUD as a HUD recorder is (agent decision).
pub const HUD_FRAME_SHARE: f64 = 0.86;

/// The HUD camera's zoom (`Camera::zoom`, 1 a 60 degree field): the HUD's
/// drawn area ([`crate::hud::HUD_CLIP`], the same raster on every aircraft)
/// fills [`HUD_FRAME_SHARE`] of the height. About 1.6, a 39 degree field.
pub fn hud_camera_zoom() -> f32 {
    (HUD_FRAME_SHARE * 480. / (f64::from(crate::hud::HUD_CLIP.3) * crate::flight_canvas::HUD_SCALE))
        as f32
}

/// The vertical field of view for a camera zoom, degrees.
pub fn field_deg(zoom: f32) -> f64 {
    2. * (std::f64::consts::FRAC_PI_6.tan() / f64::from(zoom))
        .atan()
        .to_degrees()
}

/// The sight the Sensor View looks through.
#[derive(Clone, Debug, PartialEq)]
pub enum Sight {
    /// The AC-130's sensor console.
    Console,
    /// A pod or a built-in laser or FLIR: its name and its sensor.
    Pod { name: String, sensor: SightSensor },
}

/// The Pave Spike's sight: a pod with the hemisphere gimbal and a 10 nmi
/// laser (docs/spec/air-to-ground.md, "Who has a sight"). A loaded pod is
/// not in the recording; an aircraft without a sight of its own that lased
/// carried it, the only pod the game loads.
pub fn pave_spike() -> SightSensor {
    SightSensor {
        laser: true,
        range_ft: 60_760.,
        gimbal: Gimbal::Hemisphere,
        eye: [0., -sight::POD_EYE_DROP_FT, 0.],
        default_look: sight::FIGHTER_DEFAULT_LOOK,
    }
}

/// The sight an aircraft of type `id` has in the replay: the AC-130's
/// console; the sight of its own sensor records (`fixed`, a name and the
/// sensor); else the Pave Spike when the recording shows it lasing or
/// designating with a sight. `None` for the others, which the view refuses.
pub fn sight_for(
    id: AircraftId,
    fixed: Option<&(String, SightSensor)>,
    lased: bool,
) -> Option<Sight> {
    if id == AircraftId::Ac130 {
        return Some(Sight::Console);
    }
    if let Some((name, sensor)) = fixed {
        return Some(Sight::Pod {
            name: name.clone(),
            sensor: *sensor,
        });
    }
    lased.then(|| Sight::Pod {
        name: "PAVE SPIKE".into(),
        sensor: pave_spike(),
    })
}

/// Where the Sensor View's line of sight came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The aircraft's own laser spot.
    Spot,
    /// The object it designated.
    Designated,
    /// The ground point it designated (a pin).
    Point,
    /// The ground object the recording gives as its target (an AI's
    /// target, or a weapon's at launch).
    Target,
    /// Where its guided weapon steers.
    Weapon,
    /// The AC-130 orbit's centre.
    Orbit,
    /// Where its gun rounds (or other weapons) burst on the ground around
    /// now.
    Rounds,
    /// Where its bomb landed.
    Impact,
    /// Nothing recorded: the sight's default look.
    Default,
}

impl Source {
    /// The mode word at the top left.
    pub fn mode(self) -> &'static str {
        match self {
            Self::Spot => "LSR",
            Self::Designated | Self::Target => "TRK",
            Self::Point | Self::Orbit | Self::Rounds => "PNT",
            Self::Weapon => "WPN",
            Self::Impact => "IMP",
            Self::Default => "FWD",
        }
    }
    /// The notice that says the line is worked out, not recorded.
    pub fn notice(self) -> &'static str {
        match self {
            Self::Spot => "LOS FROM LASER SPOT",
            Self::Designated | Self::Point => "LOS FROM DESIGNATION",
            Self::Target => "LOS FROM RECORDED TARGET",
            Self::Weapon => "LOS FROM WEAPON AIM",
            Self::Orbit => "LOS FROM ORBIT POINT",
            Self::Rounds => "LOS FROM ROUND IMPACTS",
            Self::Impact => "LOS FROM BOMB IMPACT",
            Self::Default => "NO LOS RECORDED",
        }
    }
}

/// The line of sight: the point it looks at, and where that came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub point: Option<Vector>,
    pub source: Source,
}

/// What the recording shows of an aircraft at a tick, for its line of sight.
pub struct Inputs<'a> {
    pub aircraft: u32,
    pub scene: &'a Scene,
    /// Where a designated object is now (a ground unit, a building or an
    /// aircraft).
    pub object: &'a dyn Fn(u32) -> Option<Vector>,
    /// The AC-130's orbit centre, while its console's orbit runs.
    pub orbit: Option<Vector>,
    /// Where its rounds burst on the ground around now ([`rounds_point`]).
    pub rounds: Option<Vector>,
    /// Where the ground object the recording gives as its target is now.
    pub target: Option<Vector>,
}

/// The Sensor View's line of sight, in this order (agent decision, the
/// order a pod operator's attention goes): the aircraft's own laser spot,
/// its designation, its guided weapon's aim, its recorded ground target, the
/// orbit's centre, where its
/// rounds burst around now, its latest landing, else the default look.
pub fn sight_line(i: &Inputs) -> Line {
    let line = |point, source| Line {
        point: Some(point),
        source,
    };
    if let Some(spot) = i.scene.spots.iter().find(|s| s.owner == i.aircraft) {
        return line(spot.at, Source::Spot);
    }
    if let Some(d) = i
        .scene
        .designations
        .iter()
        .find(|d| d.aircraft == i.aircraft)
    {
        if let Some(at) = d.object.and_then(|id| (i.object)(id)) {
            return line(at, Source::Designated);
        }
        if let Some(at) = d.at {
            return line(at, Source::Point);
        }
    }
    if let Some(aim) = i
        .scene
        .weapons
        .iter()
        .filter(|w| w.owner == i.aircraft)
        .find_map(|w| w.aim)
    {
        return line(aim, Source::Weapon);
    }
    if let Some(target) = i.target {
        return line(target, Source::Target);
    }
    if let Some(orbit) = i.orbit {
        return line(orbit, Source::Orbit);
    }
    if let Some(rounds) = i.rounds {
        return line(rounds, Source::Rounds);
    }
    if let Some(impact) = i
        .scene
        .impacts
        .iter()
        .filter(|w| w.owner == i.aircraft)
        .next_back()
    {
        return line(impact.at, Source::Impact);
    }
    Line {
        point: None,
        source: Source::Default,
    }
}

/// Rounds burst within this window about the tick count toward the line of
/// sight: two seconds before and three after (the rounds in flight now land
/// within about three seconds), agent decision.
pub const ROUNDS_WINDOW: (u64, u64) = (240, 360);

/// Where an aircraft's rounds burst on the ground around `tick`, from its
/// recorded ground impacts in tick order: the middle of those within
/// [`ROUNDS_WINDOW`], so a gunner's aim reads from where the rounds went.
pub fn rounds_point(impacts: &[(u64, Vector)], tick: u64) -> Option<Vector> {
    let from = tick.saturating_sub(ROUNDS_WINDOW.0);
    let to = tick + ROUNDS_WINDOW.1;
    let start = impacts.partition_point(|(t, _)| *t < from);
    let end = impacts.partition_point(|(t, _)| *t <= to);
    let near = &impacts[start..end];
    if near.is_empty() {
        return None;
    }
    let n = near.len() as f64;
    Some(std::array::from_fn(|k| {
        near.iter().map(|(_, p)| p[k]).sum::<f64>() / n
    }))
}

/// The time to impact row for an aircraft's guided weapons at `tick`, from
/// the recorded landings (the recording knows when each one lands, so the
/// count is exact): the oldest in flight, `TTI 0:12`, with `X2` when
/// several fly; `IMPACT` for a second after the last one landed. As the
/// HUD's and the targeting page's row (`instruments::targeting::tti_text`).
pub fn tti(weapons: &[Timed], tick: u64) -> Option<String> {
    use crate::instruments::targeting::{Tti, tti_text};
    let guided = || weapons.iter().filter(|w| w.guided);
    let flying: Vec<_> = guided()
        .filter(|w| w.released <= tick && w.landed.is_some_and(|l| l > tick))
        .collect();
    if let Some(oldest) = flying.iter().min_by_key(|w| (w.released, w.projectile)) {
        let seconds = (oldest.landed.unwrap_or(tick) - tick) as f64 / 120.;
        let text = tti_text(Tti {
            seconds,
            impact: false,
        });
        return Some(if flying.len() > 1 {
            format!("{text} X{}", flying.len())
        } else {
            text
        });
    }
    guided()
        .filter_map(|w| w.landed)
        .filter(|l| *l <= tick && tick - *l < 120)
        .max()
        .map(|_| {
            tti_text(Tti {
                seconds: 0.,
                impact: true,
            })
        })
}

/// The replay's own picture keys in a sensor view: 1 to 4 for white hot,
/// black hot, TV and night vision (agent decision, the viewer's digits are
/// free), as well as Shift+F5 to Shift+F8 as in flight.
pub fn mode_key(name: &str, shift: bool) -> Option<SensorMode> {
    if shift {
        return SensorMode::of_function_key(name);
    }
    match name {
        "1" => Some(SensorMode::WhiteHot),
        "2" => Some(SensorMode::BlackHot),
        "3" => Some(SensorMode::Tv),
        "4" => Some(SensorMode::NightVision),
        _ => None,
    }
}

/// The tape counter: mission time as `H:MM:SS`.
pub fn counter(tick: u64) -> String {
    let seconds = tick / 120;
    format!(
        "{}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// The recorder's lines over the HUD camera's picture, burned in as a
/// recorder's are: REC with its light (blinking once a second) and the
/// counter at the top, the aircraft and its type at the bottom left, the
/// tape speed at the bottom right. Positions in pixels of `size`, the
/// console font's scale for it.
pub fn recorder_lines(
    tick: u64,
    label: &str,
    aircraft: &str,
    size: [u32; 2],
) -> (Vec<(String, i32, i32)>, i32) {
    let (w, h) = (size[0] as i32, size[1] as i32);
    let scale = ((f64::from(size[1]) / 360.).round() as i32).max(1);
    let row = 10 * scale;
    let left = w * 6 / 100;
    let right = w * 94 / 100;
    let top = h * 7 / 100;
    let bottom = h * 90 / 100;
    let light = tick % 120 < 80;
    let counter = counter(tick);
    let speed = "SP";
    let lines = vec![
        (format!("{} REC", if light { "*" } else { " " }), left, top),
        (
            counter.clone(),
            right - crate::sensor_console::caption_width(&counter, scale),
            top,
        ),
        (
            tore_formats::text::upper(&format!("{label}  {aircraft}")),
            left,
            bottom,
        ),
        (
            speed.to_string(),
            right - crate::sensor_console::caption_width(speed, scale),
            bottom,
        ),
        ("HUD".to_string(), left, top + row + scale * 2),
    ];
    (lines, scale)
}

/// Where a layer has anything drawn, per row: the first column and one past
/// the last, or `(0, 0)` for a row with nothing.
pub type Extents = Vec<(usize, usize)>;

/// The HUD's 640 x 480 raster (straight-alpha RGBA) placed on a picture of
/// `size` as the cockpit pass places it (`cockpit_renderer::hud_layout`):
/// centred, scaled by [`crate::flight_canvas::HUD_SCALE`] and the camera's
/// `zoom`, sampled smoothly. The result is straight-alpha RGBA of `size`.
#[cfg(test)]
fn hud_overlay(raster: &[u8], size: [u32; 2], zoom: f32) -> Vec<u8> {
    hud_overlay_rows(raster, size, zoom).0
}

/// [`hud_overlay`] and the extent of what it drew on each row. The HUD is
/// mostly empty, so only the rows and columns whose four neighbouring raster
/// pixels hold something are sampled (the rest are exactly what sampling
/// would leave: nothing).
fn hud_overlay_rows(raster: &[u8], size: [u32; 2], zoom: f32) -> (Vec<u8>, Extents) {
    let (w, h) = (size[0] as usize, size[1] as usize);
    let mut out = crate::sensor_console::spare_overlay(w * h * 4);
    let mut extents: Extents = vec![(0, 0); h];
    if raster.len() < 640 * 480 * 4 || w == 0 || h == 0 {
        return (out, extents);
    }
    let scale = (size[0] as f64 / 640.).min(size[1] as f64 / 480.)
        * crate::flight_canvas::HUD_SCALE
        * f64::from(zoom);
    let (cx, cy) = (w as f64 / 2., h as f64 / 2.);
    // Only the rows and columns the raster covers.
    let x0 = (cx - 320. * scale).floor().max(0.) as usize;
    let x1 = ((cx + 320. * scale).ceil() as usize).min(w);
    let y0 = (cy - 240. * scale).floor().max(0.) as usize;
    let y1 = ((cy + 240. * scale).ceil() as usize).min(h);
    // The raster premultiplied, and which eight-pixel blocks of each row
    // hold anything. The table stays all zero between calls.
    let mut texels = SCRATCH.with(|scratch| std::mem::take(&mut scratch.borrow_mut().texels));
    texels.resize(640 * 480, [0f32; 4]);
    let mut blocks = [0u128; 480];
    for (row, held) in blocks.iter_mut().enumerate() {
        for x in 0..640usize {
            let at = (row * 640 + x) * 4;
            if raster[at + 3] == 0 {
                continue;
            }
            let a = f32::from(raster[at + 3]) / 255.;
            texels[row * 640 + x] = [
                f32::from(raster[at]) * a,
                f32::from(raster[at + 1]) * a,
                f32::from(raster[at + 2]) * a,
                a,
            ];
            *held |= 1u128 << (x / 8);
        }
    }
    let texel = |x: i64, y: i64| -> [f32; 4] {
        if !(0..640).contains(&x) || !(0..480).contains(&y) {
            return [0.; 4];
        }
        texels[y as usize * 640 + x as usize]
    };
    let held = |y: i64| -> u128 {
        if (0..480).contains(&y) {
            blocks[y as usize]
        } else {
            0
        }
    };
    #[allow(clippy::needless_range_loop)]
    for y in y0..y1 {
        let ry = (y as f64 + 0.5 - cy) / scale + 240. - 0.5;
        let (fy, ty) = (ry.floor(), (ry - ry.floor()) as f32);
        let iy = fy as i64;
        let (mut lo, mut hi) = (usize::MAX, 0usize);
        // The blocks either raster row holds, in runs; each run's raster
        // columns, widened by the one the interpolation reaches, as output
        // columns.
        let mut mask = held(iy) | held(iy + 1);
        let mut done = x0;
        while mask != 0 {
            let first = mask.trailing_zeros();
            let run = (!(mask >> first)).trailing_zeros();
            mask = if first + run >= 128 {
                0
            } else {
                mask & !((1u128 << (first + run)) - 1)
            };
            let from = cx - 0.5 + ((first * 8) as f64 - 320.5) * scale;
            let to = cx - 0.5 + (((first + run) * 8 - 1) as f64 - 318.5) * scale;
            let xa = ((from.floor() as i64 - 1).max(done as i64) as usize).max(x0);
            let xb = ((to.ceil() as i64 + 1).max(0) as usize).min(x1);
            for x in xa..xb {
                let rx = (x as f64 + 0.5 - cx) / scale + 320. - 0.5;
                let (fx, tx) = (rx.floor(), (rx - rx.floor()) as f32);
                let ix = fx as i64;
                let (a, b, c, d) = (
                    texel(ix, iy),
                    texel(ix + 1, iy),
                    texel(ix, iy + 1),
                    texel(ix + 1, iy + 1),
                );
                let mut p = [0f32; 4];
                for k in 0..4 {
                    let top = a[k] + (b[k] - a[k]) * tx;
                    let bottom = c[k] + (d[k] - c[k]) * tx;
                    p[k] = top + (bottom - top) * ty;
                }
                if p[3] <= 0.002 {
                    continue;
                }
                let at = (y * w + x) * 4;
                for k in 0..3 {
                    out[at + k] = (p[k] / p[3]).round().clamp(0., 255.) as u8;
                }
                out[at + 3] = (p[3] * 255.).round().clamp(0., 255.) as u8;
                lo = lo.min(x);
                hi = x + 1;
            }
            done = done.max(xb);
        }
        if hi > lo {
            extents[y] = (lo, hi);
        }
    }
    // Back to zero, row by row where something was put.
    for (row, held) in blocks.iter().enumerate() {
        if *held != 0 {
            texels[row * 640..(row + 1) * 640].fill([0f32; 4]);
        }
    }
    SCRATCH.with(|scratch| scratch.borrow_mut().texels = texels);
    (out, extents)
}

/// Work memory the tape layer keeps between pictures, so that making one
/// does not ask the system for megabytes each time.
#[derive(Default)]
struct Scratch {
    /// The HUD raster premultiplied, 640 x 480; all zero between uses.
    texels: Vec<[f32; 4]>,
    /// The halo's alpha blurred along the rows; all zero between uses.
    across: Vec<f32>,
}
thread_local! {
    static SCRATCH: std::cell::RefCell<Scratch> = std::cell::RefCell::new(Scratch::default());
}

/// How the HUD camera's recorder sees the HUD (agent decision, tuned on
/// renders so the numbers read over a bright sky through the tape's
/// softness): the symbols a little bolder (each raster pixel spread to its
/// neighbours at this share), brightened toward white by this share, and a
/// soft dark halo round them of this radius (pixels at 1080 rows) and
/// strength, as the combiner's glass and the camera's exposure darken the
/// sky behind bright symbols.
pub const HUD_BOLD: f32 = 0.55;
pub const HUD_WHITEN: f32 = 0.45;
pub const HUD_HALO_PX: f64 = 4.;
pub const HUD_HALO: f32 = 0.62;

/// The HUD raster as the tape records it ([`HUD_BOLD`] and [`HUD_WHITEN`]
/// on the raster, then [`hud_overlay`], then the halo).
pub fn hud_tape_layer(raster: &[u8], size: [u32; 2], zoom: f32) -> Vec<u8> {
    let bold = bolder(raster);
    let (mut layer, rows) = hud_overlay_rows(&bold, size, zoom);
    halo(
        &mut layer,
        size,
        HUD_HALO_PX * f64::from(size[1]) / 1080.,
        HUD_HALO,
        &rows,
    );
    layer
}

/// The raster with every symbol bolder and whiter ([`HUD_BOLD`],
/// [`HUD_WHITEN`]).
fn bolder(raster: &[u8]) -> Vec<u8> {
    let mut bold = raster.to_vec();
    if raster.len() >= 640 * 480 * 4 {
        // Rows with something drawn in them; a pixel two rows from any has
        // nothing to spread and no colour that is ever seen.
        let drawn: Vec<bool> = (0..480usize)
            .map(|y| {
                raster[y * 640 * 4..(y + 1) * 640 * 4]
                    .chunks_exact(4)
                    .any(|p| p[3] != 0)
            })
            .collect();
        for y in 0..480usize {
            let near = |y: usize| drawn[y.saturating_sub(1)..(y + 2).min(480)].contains(&true);
            if !near(y) {
                continue;
            }
            for x in 0..640usize {
                let at = (y * 640 + x) * 4;
                // The strongest neighbour, at the bold share.
                let mut best = (0u8, [0u8; 3]);
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if (0..640).contains(&nx) && (0..480).contains(&ny) {
                        let n = (ny as usize * 640 + nx as usize) * 4;
                        if raster[n + 3] > best.0 {
                            best = (raster[n + 3], [raster[n], raster[n + 1], raster[n + 2]]);
                        }
                    }
                }
                let spread = (f32::from(best.0) * HUD_BOLD) as u8;
                if spread > bold[at + 3] {
                    bold[at..at + 3].copy_from_slice(&best.1);
                    bold[at + 3] = spread;
                }
                for k in 0..3 {
                    let v = f32::from(bold[at + k]);
                    bold[at + k] = (v + (255. - v) * HUD_WHITEN).round() as u8;
                }
            }
        }
    }
    bold
}

/// A soft dark halo under everything drawn on `layer` (straight-alpha RGBA
/// of `size`): its alpha box-blurred over `radius` pixels, at `strength`.
/// `rows` is where the layer has anything on each row; the blur only works
/// where that can reach, which is everything the halo can darken.
fn halo(layer: &mut [u8], size: [u32; 2], radius: f64, strength: f32, rows: &[(usize, usize)]) {
    let (w, h) = (size[0] as usize, size[1] as usize);
    let r = radius.round().max(1.) as usize;
    let width = (2 * r + 1) as f32;
    let Some(first) = rows.iter().position(|&(lo, hi)| hi > lo) else {
        return;
    };
    let last = rows.iter().rposition(|&(lo, hi)| hi > lo).unwrap_or(first);
    // Across: each row's alpha, blurred along the row.
    let mut across = SCRATCH.with(|scratch| std::mem::take(&mut scratch.borrow_mut().across));
    across.resize(w * h, 0.);
    let mut spans = vec![(0usize, 0usize); h];
    let (mut left, mut right) = (w, 0usize);
    for y in first..=last {
        let (lo, hi) = rows[y];
        if hi <= lo {
            continue;
        }
        let alpha = |x: usize| f32::from(layer[(y * w + x) * 4 + 3]) / 255.;
        let (from, to) = (lo.saturating_sub(r), (hi + r).min(w));
        let mut sum = 0f32;
        for x in from.saturating_sub(r).max(lo)..(from + r + 1).min(hi) {
            sum += alpha(x);
        }
        let row = &mut across[y * w..(y + 1) * w];
        #[allow(clippy::needless_range_loop)]
        for i in from..to {
            row[i] = sum / width;
            if i + r + 1 < hi {
                sum += alpha(i + r + 1);
            }
            if i >= r && i - r >= lo && i - r < hi {
                sum -= alpha(i - r);
            }
        }
        spans[y] = (from, to);
        left = left.min(from);
        right = right.max(to);
    }
    // Down: the columns blurred over the rows, the shade applied to the
    // layer a row at a time as the sums slide.
    let mut column = vec![0f32; w];
    let add = |column: &mut [f32], y: usize, sign: f32| {
        let (from, to) = spans[y];
        for x in from..to {
            column[x] += sign * across[y * w + x];
        }
    };
    let (top, bottom) = (first.saturating_sub(r), (last + r + 1).min(h));
    for y in top.saturating_sub(r)..(top + r + 1).min(h) {
        add(&mut column, y, 1.);
    }
    for y in top..bottom {
        let row = &mut layer[y * w * 4..(y + 1) * w * 4];
        for (p, &sum) in row[left * 4..right * 4]
            .chunks_exact_mut(4)
            .zip(&column[left..right])
        {
            let shade = (sum / width * 2.).min(1.) * strength;
            let a = f32::from(p[3]) / 255.;
            if shade <= a {
                continue;
            }
            // The symbol over its halo: the halo is black.
            let out = a + shade * (1. - a);
            for c in &mut p[..3] {
                *c = (f32::from(*c) * a / out).round() as u8;
            }
            p[3] = (out * 255.).round() as u8;
        }
        if y + r + 1 < h {
            add(&mut column, y + r + 1, 1.);
        }
        if y >= r {
            add(&mut column, y - r, -1.);
        }
    }
    // Back to zero, row by row where something was put.
    for (y, &(from, to)) in spans.iter().enumerate() {
        across[y * w + from..y * w + to].fill(0.);
    }
    SCRATCH.with(|scratch| scratch.borrow_mut().across = across);
}

/// Straight-alpha `top` over `under`, both RGBA of the same size.
pub fn over(under: &mut [u8], top: &[u8]) {
    for (u, t) in under.chunks_exact_mut(4).zip(top.chunks_exact(4)) {
        let a = f32::from(t[3]) / 255.;
        if a <= 0. {
            continue;
        }
        let b = f32::from(u[3]) / 255. * (1. - a);
        let out = a + b;
        for k in 0..3 {
            u[k] = ((f32::from(t[k]) * a + f32::from(u[k]) * b) / out).round() as u8;
        }
        u[3] = (out * 255.).round() as u8;
    }
}

/// [`over`] on the rows `rows` marks as holding anything.
pub fn over_rows(under: &mut [u8], top: &[u8], width: usize, rows: &[bool]) {
    for (y, _) in rows.iter().enumerate().filter(|(_, held)| **held) {
        let range = y * width * 4..(y + 1) * width * 4;
        over(&mut under[range.clone()], &top[range]);
    }
}

/// What a layer of recorder lines was made for: size, scale, lines.
type CaptionKey = ([u32; 2], i32, Vec<(String, i32, i32)>);

/// The recorder's lines as a layer, kept while their words, places and size
/// stay the same (the counter changes once a second, the record light twice
/// in three): the tape layer is made 24 times a second and the lines are
/// nearly always the ones it had.
#[derive(Default)]
pub struct Captions {
    key: Option<CaptionKey>,
    pixels: Vec<u8>,
    rows: Vec<bool>,
}

impl Captions {
    /// The layer of `lines` at `scale` on a picture of `size`, and which of
    /// its rows hold anything.
    pub fn layer(
        &mut self,
        size: [u32; 2],
        lines: &[(String, i32, i32)],
        scale: i32,
    ) -> (&[u8], &[bool]) {
        let current = self
            .key
            .as_ref()
            .is_some_and(|(s, k, l)| *s == size && *k == scale && l == lines);
        if !current {
            self.pixels = crate::sensor_console::captions(size, lines, scale);
            let width = size[0] as usize * 4;
            self.rows = self
                .pixels
                .chunks_exact(width.max(4))
                .map(|row| row.chunks_exact(4).any(|p| p[3] != 0))
                .collect();
            self.key = Some((size, scale, lines.to_vec()));
        }
        (&self.pixels, &self.rows)
    }
}

/// The overlay's size for a view of `size`: the same shape, at most 1080
/// rows (the tape keeps far less; drawing more costs time for nothing).
pub fn overlay_size(size: [u32; 2]) -> [u32; 2] {
    let rows = size[1].clamp(1, 1080);
    let columns = (f64::from(size[0]) * f64::from(rows) / f64::from(size[1].max(1))).round();
    [(columns as u32).max(1), rows]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::ground_marks::{Designation, Impact, Mode, Spot, Weapon};

    // The first, whole-picture versions of the HUD placement and the halo.
    /// The first implementation, kept to check the faster one against: the HUD's 640 x 480 raster (straight-alpha RGBA) placed on a picture of
    /// `size` as the cockpit pass places it (`cockpit_renderer::hud_layout`):
    /// centred, scaled by [`crate::flight_canvas::HUD_SCALE`] and the camera's
    /// `zoom`, sampled smoothly. The result is straight-alpha RGBA of `size`.
    fn hud_overlay_reference(raster: &[u8], size: [u32; 2], zoom: f32) -> Vec<u8> {
        let (w, h) = (size[0] as usize, size[1] as usize);
        let mut out = vec![0u8; w * h * 4];
        if raster.len() < 640 * 480 * 4 || w == 0 || h == 0 {
            return out;
        }
        let scale = (size[0] as f64 / 640.).min(size[1] as f64 / 480.)
            * crate::flight_canvas::HUD_SCALE
            * f64::from(zoom);
        let (cx, cy) = (w as f64 / 2., h as f64 / 2.);
        // Only the rows and columns the raster covers.
        let x0 = (cx - 320. * scale).floor().max(0.) as usize;
        let x1 = ((cx + 320. * scale).ceil() as usize).min(w);
        let y0 = (cy - 240. * scale).floor().max(0.) as usize;
        let y1 = ((cy + 240. * scale).ceil() as usize).min(h);
        let texel = |x: i64, y: i64| -> [f32; 4] {
            if !(0..640).contains(&x) || !(0..480).contains(&y) {
                return [0.; 4];
            }
            let at = (y as usize * 640 + x as usize) * 4;
            let a = f32::from(raster[at + 3]) / 255.;
            [
                f32::from(raster[at]) * a,
                f32::from(raster[at + 1]) * a,
                f32::from(raster[at + 2]) * a,
                a,
            ]
        };
        for y in y0..y1 {
            let ry = (y as f64 + 0.5 - cy) / scale + 240. - 0.5;
            let (fy, ty) = (ry.floor(), (ry - ry.floor()) as f32);
            for x in x0..x1 {
                let rx = (x as f64 + 0.5 - cx) / scale + 320. - 0.5;
                let (fx, tx) = (rx.floor(), (rx - rx.floor()) as f32);
                let (ix, iy) = (fx as i64, fy as i64);
                let (a, b, c, d) = (
                    texel(ix, iy),
                    texel(ix + 1, iy),
                    texel(ix, iy + 1),
                    texel(ix + 1, iy + 1),
                );
                let mut p = [0f32; 4];
                for k in 0..4 {
                    let top = a[k] + (b[k] - a[k]) * tx;
                    let bottom = c[k] + (d[k] - c[k]) * tx;
                    p[k] = top + (bottom - top) * ty;
                }
                if p[3] <= 0.002 {
                    continue;
                }
                let at = (y * w + x) * 4;
                for k in 0..3 {
                    out[at + k] = (p[k] / p[3]).round().clamp(0., 255.) as u8;
                }
                out[at + 3] = (p[3] * 255.).round().clamp(0., 255.) as u8;
            }
        }
        out
    }

    /// The first implementation, kept to check the faster one against: a soft dark halo under everything drawn on `layer` (straight-alpha RGBA
    /// of `size`): its alpha box-blurred over `radius` pixels, at `strength`.
    fn halo_reference(layer: &mut [u8], size: [u32; 2], radius: f64, strength: f32) {
        let (w, h) = (size[0] as usize, size[1] as usize);
        let r = radius.round().max(1.) as usize;
        let alpha: Vec<f32> = layer
            .chunks_exact(4)
            .map(|p| f32::from(p[3]) / 255.)
            .collect();
        let blur = |src: &[f32], horizontal: bool| -> Vec<f32> {
            let mut out = vec![0f32; src.len()];
            let (outer, inner) = if horizontal { (h, w) } else { (w, h) };
            let index = |o: usize, i: usize| if horizontal { o * w + i } else { i * w + o };
            for o in 0..outer {
                let mut sum = 0f32;
                for i in 0..inner.min(r + 1) {
                    sum += src[index(o, i)];
                }
                for i in 0..inner {
                    out[index(o, i)] = sum / (2 * r + 1) as f32;
                    if i + r + 1 < inner {
                        sum += src[index(o, i + r + 1)];
                    }
                    if i >= r {
                        sum -= src[index(o, i - r)];
                    }
                }
            }
            out
        };
        let soft = blur(&blur(&alpha, true), false);
        for (p, s) in layer.chunks_exact_mut(4).zip(soft) {
            let shade = (s * 2.).min(1.) * strength;
            let a = f32::from(p[3]) / 255.;
            if shade <= a {
                continue;
            }
            // The symbol over its halo: the halo is black.
            let out = a + shade * (1. - a);
            for c in &mut p[..3] {
                *c = (f32::from(*c) * a / out).round() as u8;
            }
            p[3] = (out * 255.).round() as u8;
        }
    }

    /// A HUD-like raster: lines, boxes and a little text-like clutter, with
    /// translucent edges, over the whole 640 x 480.
    fn sample_raster() -> Vec<u8> {
        let mut raster = vec![0u8; 640 * 480 * 4];
        let mut put = |x: usize, y: usize, a: u8| {
            let at = (y * 640 + x) * 4;
            raster[at..at + 4].copy_from_slice(&[40, 255, 60, a]);
        };
        for x in 100..540 {
            put(x, 100, 255);
            put(x, 101, 90);
        }
        for y in 150..400 {
            put(2, y, 200);
            put(637, y, 255);
            put(320, y, 255);
        }
        for k in 0..60 {
            put(10 + k, 5, 255);
            put(0, 0, 255);
            put(639, 479, 255);
            put(200 + k % 7, 300 + k / 7, 128);
        }
        raster
    }

    #[test]
    fn the_faster_hud_placement_draws_the_same_pixels() {
        let raster = sample_raster();
        for (size, zoom) in [
            ([1920, 1080], 1.6),
            ([1920, 1080], 1.),
            ([1280, 720], 2.4),
            ([1440, 1080], 0.7),
            ([800, 1200], 1.6),
            ([1920, 1080], 5.),
        ] {
            let now = hud_overlay(&raster, size, zoom);
            let then = hud_overlay_reference(&raster, size, zoom);
            assert_eq!(now.len(), then.len());
            let wrong = now.iter().zip(&then).filter(|(a, b)| a != b).count();
            assert_eq!(wrong, 0, "{size:?} zoom {zoom}: {wrong} bytes differ");
        }
        // Nothing drawn, nothing placed.
        let blank = vec![0u8; 640 * 480 * 4];
        assert!(
            hud_overlay(&blank, [1920, 1080], 1.6)
                .iter()
                .all(|b| *b == 0)
        );
    }

    #[test]
    fn the_faster_halo_matches_the_whole_picture_one() {
        let raster = bolder(&sample_raster());
        for (size, zoom) in [([1920, 1080], 1.6), ([1280, 720], 1.6), ([800, 1200], 2.)] {
            let (mut now, rows) = hud_overlay_rows(&raster, size, zoom);
            let mut then = now.clone();
            let radius = HUD_HALO_PX * f64::from(size[1]) / 1080.;
            halo(&mut now, size, radius, HUD_HALO, &rows);
            halo_reference(&mut then, size, radius, HUD_HALO);
            let worst = now
                .iter()
                .zip(&then)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            // The running sums add in another order: one step at most.
            assert!(worst <= 1, "{size:?}: off by {worst}");
            let far = now.iter().zip(&then).filter(|(a, b)| a != b).count();
            assert!(far * 1000 < now.len(), "{size:?}: {far} bytes differ");
        }
    }

    #[test]
    fn a_blank_layer_gets_no_halo() {
        let mut layer = vec![0u8; 64 * 32 * 4];
        halo(&mut layer, [64, 32], 4., HUD_HALO, &vec![(0, 0); 32]);
        assert!(layer.iter().all(|b| *b == 0));
    }

    fn fixed() -> (String, SightSensor) {
        ("AAS-38".into(), {
            let mut s = pave_spike();
            s.laser = false;
            s
        })
    }

    #[test]
    fn the_sensor_view_is_offered_by_sight() {
        // The AC-130 always has its console.
        assert_eq!(
            sight_for(AircraftId::Ac130, None, false),
            Some(Sight::Console)
        );
        // An aircraft's own record (a FLIR pod, an internal FLIR, a laser).
        let f = fixed();
        assert!(matches!(
            sight_for(AircraftId::Rafale, Some(&f), false),
            Some(Sight::Pod { name, sensor }) if name == "AAS-38" && !sensor.laser
        ));
        // A loaded pod: no record, but the recording shows the laser.
        assert!(matches!(
            sight_for(AircraftId::F18, None, true),
            Some(Sight::Pod { name, sensor }) if name == "PAVE SPIKE" && sensor.laser
        ));
        // Nothing at all: no view.
        assert_eq!(sight_for(AircraftId::Mig21, None, false), None);
    }

    #[test]
    fn the_line_of_sight_follows_the_laser_then_the_designation_then_the_weapon() {
        let object = |id: u32| (id == 9).then_some([10., 0., 20.]);
        let mut scene = Scene::default();
        let line_of = |scene: &Scene| {
            sight_line(&Inputs {
                aircraft: 1,
                scene,
                object: &object,
                orbit: None,
                rounds: None,
                target: None,
            })
        };
        assert_eq!(line_of(&scene).source, Source::Default);
        scene.impacts.push(Impact {
            projectile: 5,
            owner: 1,
            at: [1.; 3],
            aim: None,
            miss_ft: None,
            target_ft: None,
            blind: false,
            dud: false,
        });
        assert_eq!(line_of(&scene).source, Source::Impact);
        scene.weapons.push(Weapon {
            projectile: 6,
            owner: 1,
            guidance: crate::replay::ground_marks::Guidance::Laser,
            mode: Mode::Guiding,
            aim: Some([2.; 3]),
            spot_owner: Some(1),
            own: true,
            target: None,
            reason: String::new(),
        });
        assert_eq!(
            line_of(&scene),
            Line {
                point: Some([2.; 3]),
                source: Source::Weapon
            }
        );
        scene.designations.push(Designation {
            aircraft: 1,
            object: Some(9),
            at: None,
            channel: "sight".into(),
        });
        assert_eq!(
            line_of(&scene).point,
            Some([10., 0., 20.]),
            "the designated object where it is now"
        );
        // Another aircraft's spot is not this one's line.
        scene.spots.push(Spot {
            owner: 2,
            at: [3.; 3],
            lasing: true,
            mask: None,
        });
        assert_eq!(line_of(&scene).source, Source::Designated);
        scene.spots.push(Spot {
            owner: 1,
            at: [4.; 3],
            lasing: true,
            mask: None,
        });
        assert_eq!(
            line_of(&scene),
            Line {
                point: Some([4.; 3]),
                source: Source::Spot
            }
        );
        assert_eq!(Source::Spot.mode(), "LSR");
        assert_eq!(Source::Default.notice(), "NO LOS RECORDED");
    }

    #[test]
    fn rounds_give_the_gunners_aim_around_now() {
        let impacts = [(100, [0.; 3]), (1000, [10., 0., 0.]), (1100, [20., 0., 0.])];
        assert_eq!(rounds_point(&impacts, 1050), Some([15., 0., 0.]));
        assert_eq!(
            rounds_point(&impacts, 700),
            Some([10., 0., 0.]),
            "rounds in flight"
        );
        assert_eq!(rounds_point(&impacts, 500), None);
        assert_eq!(rounds_point(&[], 500), None);
    }

    #[test]
    fn time_to_impact_counts_down_to_the_recorded_landing() {
        let w = |projectile, released, landed| Timed {
            projectile,
            released,
            landed: Some(landed),
            guided: true,
        };
        let one = [w(1, 100, 100 + 12 * 120)];
        assert_eq!(tti(&one, 99), None, "before release");
        assert_eq!(tti(&one, 100).as_deref(), Some("TTI 0:12"));
        assert_eq!(tti(&one, 100 + 6 * 120 + 1).as_deref(), Some("TTI 0:06"));
        assert_eq!(tti(&one, 100 + 12 * 120).as_deref(), Some("IMPACT"));
        assert_eq!(tti(&one, 100 + 13 * 120), None, "a second after");
        // Several in flight: the oldest's time, and how many.
        let two = [w(1, 100, 2000), w(2, 200, 2600)];
        assert_eq!(tti(&two, 300).as_deref(), Some("TTI 0:15 X2"));
        assert_eq!(tti(&two, 2100).as_deref(), Some("TTI 0:05"));
        // An unguided bomb has no time to impact.
        let dumb = [Timed {
            guided: false,
            ..w(3, 100, 400)
        }];
        assert_eq!(tti(&dumb, 200), None);
    }

    #[test]
    fn picture_keys_choose_each_mode() {
        let all = SensorMode::ALL;
        for (i, key) in ["1", "2", "3", "4"].into_iter().enumerate() {
            assert_eq!(mode_key(key, false), Some(all[i]));
            assert_eq!(mode_key(key, true), None);
        }
        for (i, key) in ["F5", "F6", "F7", "F8"].into_iter().enumerate() {
            assert_eq!(mode_key(key, true), Some(all[i]));
            assert_eq!(mode_key(key, false), None);
            assert_eq!(SensorMode::of_action(all[i].action()), Some(all[i]));
        }
        assert_eq!(mode_key("5", false), None);
        assert_eq!(
            all.map(SensorMode::action),
            ["sight-whot", "sight-bhot", "sight-tv", "sight-nvg"]
        );
    }

    #[test]
    fn the_hud_camera_frames_the_hud() {
        let zoom = hud_camera_zoom();
        assert!((1.5..1.75).contains(&zoom), "{zoom}");
        let field = field_deg(zoom);
        assert!((35. ..42.).contains(&field), "{field}");
        // The HUD's drawn area is that share of a 1080-row picture.
        let rows = f64::from(crate::hud::HUD_CLIP.3) / 480.
            * 1080.
            * crate::flight_canvas::HUD_SCALE
            * f64::from(zoom);
        assert!((rows / 1080. - HUD_FRAME_SHARE).abs() < 1e-6);
    }

    #[test]
    fn the_tape_is_the_same_at_a_tick_and_moves_on_between_fields() {
        let a = crate::crt::Frame::tape(48_000, 7);
        assert_eq!(a, crate::crt::Frame::tape(48_000, 7));
        assert_eq!(a.seconds, 400.);
        assert!(a.colour && a.tape.is_some());
        // A tracking band comes in some windows only, faint, rolling up.
        let bands: Vec<_> = (0..200u64)
            .map(|w| {
                (0..crate::crt::TRACKING_WINDOW_TICKS)
                    .step_by(10)
                    .map(|t| crate::crt::TapeField::at(w * 900 + t, 7).tracking)
                    .fold(0f32, f32::max)
            })
            .collect();
        let with = bands.iter().filter(|b| **b > 0.).count();
        assert!((30..90).contains(&with), "{with} of 200 windows");
        assert!(
            bands
                .iter()
                .all(|b| *b <= crate::crt::TRACKING_STRENGTH + 1e-6)
        );
        // Dropouts are rare.
        let drops = (0..20_000u64)
            .filter(|f| crate::crt::TapeField::at(f * 2, 7).dropout.is_some())
            .count();
        assert!((200..900).contains(&drops), "{drops}");
    }

    #[test]
    fn the_hud_raster_lands_centred_and_scaled() {
        let mut raster = vec![0u8; 640 * 480 * 4];
        // A lit pixel at the raster's centre.
        for (x, y) in [(319, 239), (320, 239), (319, 240), (320, 240)] {
            let at = (y * 640 + x) * 4;
            raster[at..at + 4].copy_from_slice(&[0, 255, 0, 255]);
        }
        let out = hud_overlay(&raster, [1920, 1080], 1.);
        let at = (540 * 1920 + 960) * 4;
        assert_eq!(&out[at..at + 4], &[0, 255, 0, 255]);
        assert_eq!(out[3], 0, "the corner stays clear");
        assert_eq!(overlay_size([3840, 2160]), [1920, 1080]);
        assert_eq!(overlay_size([1280, 720]), [1280, 720]);
    }

    #[test]
    fn the_counter_reads_mission_time() {
        assert_eq!(counter(0), "0:00:00");
        assert_eq!(counter(120 * 3_725), "1:02:05");
        let (lines, scale) = recorder_lines(0, "You", "F-16C", [1920, 1080]);
        assert_eq!(scale, 3);
        assert_eq!(lines[0].0, "* REC");
        assert_eq!(lines[2].0, "YOU  F-16C");
    }
}
