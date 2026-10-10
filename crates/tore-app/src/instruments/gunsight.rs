//! The AC-130 gunsight page: the TARGET CAM window when the aircraft is a
//! gunship (docs/spec/target-window.md, "AC-130 gunsight"). The picture is the
//! sight's camera; everything here is symbology drawn over it in ink with a
//! one pixel white halo, so it reads on dark and light ground alike.
//!
//! Opinionated (John, 2026-10-09): the layout follows the plan's section 2.4
//! at the page's 138 x 114 pixels. Gun list top left, name and activity rows
//! narrowed to make room, pipper at the guns' predicted impact, an arcs box
//! between the clock and the range.
use super::{Raster, SCREEN};
use crate::target_window::Readout as TargetReadout;
use tore_formats::{font::Font, text::GlyphCodes};
use tore_sim::{
    attitude::Vector,
    combat::{
        gunship::{self, Notice, Sight},
        gunship_impact::Impact,
        gunsight,
        live::Readiness,
    },
};

const INK: [u8; 4] = [20, 20, 20, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const GREY: [u8; 4] = [110, 110, 110, 255];
const BOX_FILL: [u8; 4] = [206, 206, 206, 255];
const W: i32 = SCREEN.2;
const H: i32 = SCREEN.3;
/// The camera centre, where the crosshair sits.
const CENTRE: (i32, i32) = (W / 2, H / 2);
/// The arcs box: x, y, width, height (plan 2.4).
const BOX: (i32, i32, i32, i32) = (45, 92, 46, 20);
/// How long a one-off notice stays on the activity row, in combat ticks (2 s).
const NOTICE_TICKS: u64 = 240;
/// The widest arc, in degrees either side of the neutral heading and
/// elevation: the box spans it (C_25). Forward is its right edge, aft its
/// left.
const BOX_HEADING: f64 = 60.;
const BOX_ELEVATION: f64 = 60.;
/// Each gun's arc half-widths, degrees: heading then elevation, the sim's own
/// limits (docs/spec/ac130-linked-guns.md).
fn arc(slot: usize) -> (f64, f64) {
    (
        gunship::HEADING_ARC[slot].to_degrees(),
        gunship::ELEVATION_ARC[slot].to_degrees(),
    )
}
/// The camera's own gimbal is the hemisphere below the aircraft: its look
/// elevation stops at the horizon (John, 2026-10-09; the sim clamps the look
/// there). The host's GIMBAL LIMIT notice says so authoritatively; the page
/// also reads the limit from its own predicted look, which leads the notice
/// by a round trip.
fn camera_top() -> f64 {
    gunship::GIMBAL_TOP.to_degrees()
}
/// Within this many degrees of the camera's limit the page says so.
const LIMIT_MARGIN: f64 = 0.5;
/// The GIMBAL LIMIT notice is raised every tick the limit holds, so it counts
/// as live for this many combat ticks after the last (a quarter second).
const LIMIT_NOTICE_TICKS: u64 = 30;

/// Where the sight's camera is and what it looks along: the page projects the
/// symbology through it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SightView {
    pub position: Vector,
    pub yaw: f32,
    pub pitch: f32,
    /// `Camera::zoom`: 1 is a 60 degree tall picture.
    pub zoom: f32,
}
impl SightView {
    /// Where a world point lands on the 138 x 114 picture, unclamped. `None`
    /// for a point level with or behind the camera.
    pub fn project(&self, point: Vector) -> Option<(f64, f64)> {
        let [x, y, z] = self.camera_space(point);
        (z > 1.).then(|| self.scale(x, y, z))
    }
    /// The picture's direction (right, up) toward a point, whatever side of
    /// the camera it is on; `None` only for the camera's own position.
    pub fn direction(&self, point: Vector) -> Option<(f64, f64)> {
        let [x, y, _] = self.camera_space(point);
        (x != 0. || y != 0.).then_some((x, y))
    }
    fn camera_space(&self, point: Vector) -> Vector {
        let (sy, cy) = f64::from(self.yaw).sin_cos();
        let (sp, cp) = f64::from(self.pitch).sin_cos();
        let right = [cy, 0., -sy];
        let up = [-sy * sp, cp, -cy * sp];
        let forward = [sy * cp, sp, cy * cp];
        let d: Vector = std::array::from_fn(|i| point[i] - self.position[i]);
        let dot = |a: Vector| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
        [dot(right), dot(up), dot(forward)]
    }
    fn scale(&self, x: f64, y: f64, z: f64) -> (f64, f64) {
        let focal = f64::from(H) / 2. * 3f64.sqrt() * f64::from(self.zoom);
        (
            f64::from(W) / 2. + focal * x / z,
            f64::from(H) / 2. - focal * y / z,
        )
    }
}

/// One gun of the three, as the list and the arcs box show it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gun {
    /// The aircraft carries this gun at all.
    pub fitted: bool,
    pub linked: bool,
    pub status: Readiness,
    /// The gun's maximum range in feet (its record's firing zone): the range
    /// ring's full scale.
    pub maximum_range_ft: f64,
}

/// A notice for the activity row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageNotice {
    NoGroundPoint,
    DropToSlew,
}

/// Everything the gunsight page draws that is not the camera's picture,
/// built from the readout (`combat_view::readout`) and completed by the host
/// with the presented look, the client's zoom and the camera's view.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub sight: Sight,
    /// Body-relative [heading, elevation] of the line of sight, radians, as
    /// presented this frame.
    pub look: [f64; 2],
    /// The client's zoom step, 1 to 6.
    pub zoom: u8,
    pub returning: bool,
    pub guns: [Gun; 3],
    /// The gun the Ctrl+7 selection points at.
    pub candidate: Option<usize>,
    pub impacts: [Option<Impact>; 3],
    /// Each gun's actual [heading, elevation], radians.
    pub trains: [[f64; 2]; 3],
    /// The sight point, for the pinned or tracked mark.
    pub mark: Option<Vector>,
    pub notice: Option<PageNotice>,
    /// The camera sits against its own gimbal limit.
    pub gimbal_limit: bool,
    /// The sim's own gimbal-limit notice is showing.
    limit_notice: bool,
    /// How the gimbal-limit notice draws its eye.
    pub eye: EyeIcon,
    /// Clock bearing with Hi/Lo, and the range to the sight point: the
    /// bottom rows of a free or pinned sight.
    pub bearing: String,
    pub range: String,
    pub view: Option<SightView>,
}

impl Page {
    /// The page's data from the readout. `None` off the AC-130.
    pub fn new(
        ro: &tore_world::readout::CockpitReadout,
        config: &tore_sim::combat::live::Configuration,
        player: &crate::flight::State,
    ) -> Option<Self> {
        let g = ro.gunsight.as_ref()?;
        let stores = &ro.stores;
        let station = |slot: usize| {
            config
                .stations
                .iter()
                .position(|s| s.weapon.source.eq_ignore_ascii_case(gunship::GUNS[slot]))
        };
        let guns = std::array::from_fn(|slot| Gun {
            fitted: station(slot).is_some(),
            linked: stores.gun_group & (1 << slot) != 0,
            status: g.status[slot],
            maximum_range_ft: station(slot).map_or(0., |i| {
                f64::from(
                    config.stations[i].weapon.seeker.zones[1]
                        .maximum_range
                        .max(0),
                )
            }),
        });
        let candidate = (0..3).find(|slot| station(*slot) == Some(stores.selected()));
        let trains = std::array::from_fn(|slot| {
            [
                stores.gun_aim[slot * 2] * std::f64::consts::PI,
                stores.gun_aim[slot * 2 + 1] * std::f64::consts::FRAC_PI_2,
            ]
        });
        let mark = match g.sight {
            Sight::Pinned(point) => Some(point),
            Sight::Tracked(_) => ro.targets.display.as_ref().map(|t| t.position),
            Sight::Free => None,
        };
        // The combat tick the readout describes is one past the tick its
        // impacts were computed for.
        let now = g.impacts_tick + 1;
        let notice = g.notice.and_then(|n| {
            if n.tick > now || now - n.tick >= NOTICE_TICKS {
                return None;
            }
            match n.notice {
                Notice::NoGroundPoint => Some(PageNotice::NoGroundPoint),
                Notice::DropToSlew => Some(PageNotice::DropToSlew),
                // Drawn as the gimbal limit mark, not on the activity row.
                Notice::GimbalLimit => None,
            }
        });
        // The host raises it every tick the camera is held at its limit, so
        // a short window (not the activity row's two seconds) keeps it live.
        let limit_notice = gimbal_notice_live(g.notice, now);
        let gimbal_limit = limit_notice || at_camera_limit(g.look);
        let point = g.aim.unwrap_or(player.position);
        let offset: Vector = std::array::from_fn(|i| point[i] - player.position[i]);
        let range = offset.iter().map(|v| v * v).sum::<f64>().sqrt();
        Some(Self {
            sight: g.sight,
            look: g.look,
            zoom: g.zoom,
            returning: g.returning,
            guns,
            candidate,
            impacts: g.impacts,
            trains,
            mark,
            notice,
            gimbal_limit,
            limit_notice,
            eye: EYE_ICON,
            bearing: bearing_label(offset, player.yaw),
            range: format!(
                "{:.1} NM",
                range / tore_sim::sensors::FEET_PER_NAUTICAL_MILE
            ),
            view: None,
        })
    }

    /// Completes the page with this frame's presented look, the client's
    /// zoom, the camera's view and the presented sight point (a tracked
    /// object's drawn position).
    pub fn present(&mut self, look: [f64; 2], zoom: u8, view: SightView, mark: Option<Vector>) {
        self.look = look;
        self.zoom = zoom;
        self.view = Some(view);
        if mark.is_some() {
            self.mark = mark;
        }
        self.gimbal_limit = self.limit_notice || at_camera_limit(look);
    }

    /// The gun whose pipper is the centre one and whose status the status
    /// row reports: the candidate, else the first linked gun.
    fn lead(&self) -> Option<usize> {
        self.candidate
            .filter(|slot| self.guns[*slot].fitted)
            .or_else(|| (0..3).find(|slot| self.guns[*slot].fitted && self.guns[*slot].linked))
    }
}

/// The host's GIMBAL LIMIT notice is live at readout tick `now`.
fn gimbal_notice_live(notice: Option<gunship::SightNotice>, now: u64) -> bool {
    notice.is_some_and(|n| {
        n.notice == Notice::GimbalLimit && n.tick <= now && now - n.tick < LIMIT_NOTICE_TICKS
    })
}

/// The camera's own gimbal limit, from the look alone.
fn at_camera_limit(look: [f64; 2]) -> bool {
    look[1].to_degrees() >= camera_top() - LIMIT_MARGIN
}

/// "9:00 LO": the clock bearing of an offset from the nose and the Hi/Lo
/// label against the level plane (the target window's rules).
fn bearing_label(offset: Vector, yaw: f64) -> String {
    let relative = offset[0].atan2(offset[2]) - yaw;
    let hour = (relative.rem_euclid(std::f64::consts::TAU) / (std::f64::consts::PI / 6.)).round()
        as u8
        % 12;
    let limit = offset[0].hypot(offset[2]) * 10_f64.to_radians().tan();
    let level = if offset[1] > limit {
        " HI"
    } else if offset[1] < -limit {
        " LO"
    } else {
        ""
    };
    format!("{}:00{level}", if hour == 0 { 12 } else { hour })
}

type Pixels = Vec<(i32, i32)>;

/// Draws a symbol: a white halo, then the ink.
fn stamp(r: &mut Raster, pixels: &Pixels) {
    let inside = |&(x, y): &(i32, i32)| (0..W).contains(&x) && (0..H).contains(&y);
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        for &(x, y) in pixels.iter().filter(|p| inside(p)) {
            r.rect(x + dx, y + dy, 1, 1, WHITE);
        }
    }
    for &(x, y) in pixels.iter().filter(|p| inside(p)) {
        r.rect(x, y, 1, 1, INK);
    }
}

/// The camera centre: four short ticks, 3 to 5 pixels out, with a clear gap
/// at the middle so the pipper's dot reads when it sits on the centre.
fn crosshair() -> Pixels {
    let (cx, cy) = CENTRE;
    let mut p = Vec::new();
    for d in 3..=5 {
        p.extend([(cx - d, cy), (cx + d, cy), (cx, cy - d), (cx, cy + d)]);
    }
    p
}

/// The pipper's thin ring: radius 9, one pixel thick (the fighter HUD's LCOS
/// circle).
const RING: f64 = 9.;
/// The range arc: two pixels thick, hugging the ring's outside (HUD radii 10
/// and 11).
const ARC_OUTER: f64 = 11.5;

/// The LCOS pipper (weapon_hud.rs `draw_gun_solution`): a thin ring, a bolder
/// range arc round it and a centre dot. The arc starts at twelve o'clock and
/// runs clockwise for `range_arc` of the circle: absent at the gun's maximum
/// range, half at half range, full inside 100 feet
/// ([`tore_sim::combat::gunsight::range_arc_fraction`]). `dashed` takes every
/// other 30 degrees from the thin ring (the range arc stays whole: it is the
/// range, not the state); `dot` draws the centre dot.
fn pipper(cx: i32, cy: i32, range_arc: f64, dashed: bool, dot: bool) -> Pixels {
    let mut p = Pixels::new();
    let reach = ARC_OUTER.ceil() as i32;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let d = f64::from(dx).hypot(f64::from(dy));
            // Clockwise from twelve o'clock, in degrees.
            let theta = f64::from(dx)
                .atan2(f64::from(-dy))
                .to_degrees()
                .rem_euclid(360.);
            // Dashes are 30 degrees long, centred on twelve o'clock.
            let gap = dashed && ((theta + 15.) / 30.).floor() as i32 % 2 == 1;
            let thin = !gap && (RING - 0.5..RING + 0.5).contains(&d);
            let arc = (RING + 0.5..ARC_OUTER).contains(&d) && theta / 360. < range_arc;
            if thin || arc {
                p.push((cx + dx, cy + dy));
            }
        }
    }
    if dot {
        p.extend([
            (cx, cy),
            (cx - 1, cy),
            (cx + 1, cy),
            (cx, cy - 1),
            (cx, cy + 1),
        ]);
    }
    p
}

/// Another linked gun's impact: a small hollow diamond (radius 3), so it
/// cannot be taken for the candidate's dot or ring.
fn other_impact(cx: i32, cy: i32) -> Pixels {
    let mut p = Pixels::new();
    for d in 0..3 {
        p.extend([
            (cx + d, cy - 3 + d),
            (cx + 3 - d, cy + d),
            (cx - d, cy + 3 - d),
            (cx - 3 + d, cy - d),
        ]);
    }
    p
}

/// A square outline `side` pixels across, centred.
fn square(cx: i32, cy: i32, side: i32) -> Pixels {
    let (lo, hi) = (-(side / 2), side / 2);
    let mut p = Vec::new();
    for d in lo..=hi {
        p.extend([
            (cx + d, cy + lo),
            (cx + d, cy + hi),
            (cx + lo, cy + d),
            (cx + hi, cy + d),
        ]);
    }
    p.sort_unstable();
    p.dedup();
    p
}

/// Where a pipper centre may sit: inside the page with room for its ring,
/// else on the page edge along the line from the middle.
fn park(point: (f64, f64)) -> (i32, i32) {
    let margin = 13.;
    let (cx, cy) = (f64::from(CENTRE.0), f64::from(CENTRE.1));
    let (dx, dy) = (point.0 - cx, point.1 - cy);
    let mut t = 1_f64;
    if dx.abs() > cx - margin {
        t = t.min((cx - margin) / dx.abs());
    }
    if dy.abs() > cy - margin {
        t = t.min((cy - margin) / dy.abs());
    }
    ((cx + dx * t).round() as i32, (cy + dy * t).round() as i32)
}

/// A pipper's picture position: its projection, or the page edge in its
/// direction when it falls off the page or behind the camera.
fn pipper_at(view: &SightView, point: Vector) -> (i32, i32) {
    if let Some(at) = view.project(point) {
        return park(at);
    }
    match view.direction(point) {
        Some((x, y)) => park((f64::from(CENTRE.0) + x * 1e6, f64::from(CENTRE.1) - y * 1e6)),
        None => CENTRE,
    }
}

/// How the camera-limit notice draws its eye (John, 2026-10-09): the plain
/// text "<o>" in the page's own font, or a hand-drawn pixel eyeball. The
/// one-line switch is [`EYE_ICON`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EyeIcon {
    /// "<o>": an almond and a pupil, in the page's font. No asset.
    Text,
    /// A 9 x 5 almond outline with a round pupil, placed by hand.
    Bitmap,
}
/// The default eye icon.
pub const EYE_ICON: EyeIcon = EyeIcon::Text;

/// The hand-drawn eyeball, row by row.
const EYE_BITMAP: [&str; 5] = [
    "..#####..",
    ".#..#..#.",
    "#..###..#",
    ".#..#..#.",
    "..#####..",
];

/// What the status row says, and whether the eye icon leads it.
fn status_row(page: &Page) -> (&'static str, bool) {
    if page.gimbal_limit {
        return ("GIMBAL LIMIT", true);
    }
    match page.lead() {
        Some(slot) => (page.guns[slot].status.label(), false),
        None => ("NO GUN", false),
    }
}

fn text_width(f: &Font, s: &str) -> i32 {
    s.glyph_codes()
        .map(|ch| f.glyphs[ch as usize].advance as i32)
        .sum()
}

/// Draws the page over the picture the caller already painted.
pub(super) fn draw(
    r: &mut Raster,
    f: &Font,
    page: &Page,
    target: Option<(&TargetReadout, &super::TargetLink)>,
) {
    let fit = |s: &str, limit: usize| super::fit_width(f, s, limit);
    // Rows: centre of the columns the gun list leaves free.
    let centred = |r: &mut Raster, s: &str, y: i32, left: i32, width: i32| {
        r.text(f, s, left + (width - text_width(f, s)) / 2, y, INK);
    };
    let tracked = matches!(page.sight, Sight::Tracked(_));
    let target = target.filter(|_| tracked);
    // Name and activity.
    let name = match (target, page.sight) {
        (Some((t, _)), _) => tore_formats::text::upper(&t.name),
        (None, Sight::Tracked(_)) => "TRACK".into(),
        (None, Sight::Pinned(_)) => "PINNED".into(),
        (None, Sight::Free) if page.returning => "RETURN".into(),
        (None, Sight::Free) => "SLEW".into(),
    };
    centred(r, &fit(&name, 100), 3, 22, 100);
    let activity = match (page.notice, target) {
        (Some(PageNotice::NoGroundPoint), _) => "NO GROUND POINT".to_string(),
        (Some(PageNotice::DropToSlew), _) => "L TO DROP".to_string(),
        (None, Some((t, _))) => t.activity.clone(),
        (None, None) if tracked => String::new(),
        (None, None) => format!("ZOOM {}", page.zoom),
    };
    centred(r, &fit(&activity, 100), 16, 22, 100);
    if let Some((t, link)) = target {
        let mut line = 28;
        for text in link.tag.iter().chain(&link.state) {
            centred(r, &fit(text, 100), line, 22, 100);
            line += 11;
        }
        r.text(f, t.goal, 124, 3, INK);
        if t.player_goal {
            r.rect(124, 13, 6, 1, INK);
        }
        for dot in 0..t.skill.unwrap_or(0) {
            r.rect(122 + i32::from(dot) * 4, -10, 2, 2, [230, 230, 230, 255]);
        }
        // Damage bar.
        r.rect(133, 1, 4, 48, [240, 240, 240, 255]);
        r.rect(134, 2, 2, 46, [0, 0, 0, 255]);
        let filled = (t.damage * 46.).round() as i32;
        r.rect(134, 48 - filled, 2, filled, WHITE);
        let objective = match t.objective {
            Some(crate::target_window::TargetObjective::Survive) => "Obj: Survive",
            Some(crate::target_window::TargetObjective::Destroy) => "Obj: Destroy",
            None => "",
        };
        centred(r, objective, 82, 0, 134);
    }
    // Gun list.
    for slot in 0..3 {
        gun_label(r, f, page, slot, 3 + slot as i32 * 11);
    }
    // Symbology.
    let mut marks = Pixels::new();
    // The centre ticks give way to the pipper's own dot when it sits on them.
    let mut centre_ticks = true;
    if let Some(view) = &page.view {
        if let Some((x, y)) = page.mark.and_then(|point| view.project(point)) {
            let (x, y) = (x.round() as i32, y.round() as i32);
            marks.extend(square(x, y, if tracked { 10 } else { 6 }));
        }
        let lead = page.lead();
        for slot in 0..3 {
            if Some(slot) == lead || !page.guns[slot].linked || !page.guns[slot].fitted {
                continue;
            }
            if let Some(impact) = page.impacts[slot] {
                let (x, y) = pipper_at(view, impact.point());
                marks.extend(other_impact(x, y));
            }
        }
        if let Some((slot, impact)) = lead.and_then(|s| Some((s, page.impacts[s]?))) {
            let (x, y) = pipper_at(view, impact.point());
            let status = page.guns[slot].status;
            let dashed = status == Readiness::GunArc
                || status == Readiness::MaximumRange
                || matches!(impact, Impact::Spent { .. });
            let dot =
                !(matches!(impact, Impact::Spent { .. }) || status == Readiness::MaximumRange);
            // The range arc follows the straight-line range to the impact;
            // rounds that spend in the air are at or past the maximum.
            let range_arc = match impact {
                Impact::Spent { .. } => 0.,
                _ => gunsight::range_arc_fraction(
                    impact.range_ft(),
                    page.guns[slot].maximum_range_ft,
                ),
            };
            marks.extend(pipper(x, y, range_arc, dashed, dot));
            centre_ticks = !(dot && (x - CENTRE.0).abs() <= 4 && (y - CENTRE.1).abs() <= 4);
        }
    }
    if centre_ticks {
        marks.extend(crosshair());
    }
    stamp(r, &marks);
    // Status row, with the eye icon ahead of the camera-limit notice.
    let (status, eye) = status_row(page);
    let icon_width = match (eye, page.eye) {
        (false, _) => 0,
        (true, EyeIcon::Text) => text_width(f, "<o> "),
        (true, EyeIcon::Bitmap) => 9 + 3,
    };
    let total = icon_width + text_width(f, status);
    let left = CENTRE.0 - total / 2;
    let haloed = |r: &mut Raster, s: &str, x: i32| {
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            r.text(f, s, x + dx, 72 + dy, WHITE);
        }
        r.text(f, s, x, 72, INK);
    };
    if eye {
        match page.eye {
            EyeIcon::Text => haloed(r, "<o>", left),
            EyeIcon::Bitmap => {
                let mut pixels = Pixels::new();
                for (y, row) in EYE_BITMAP.iter().enumerate() {
                    for (x, c) in row.chars().enumerate() {
                        if c == '#' {
                            pixels.push((left + x as i32, 73 + y as i32));
                        }
                    }
                }
                stamp(r, &pixels);
            }
        }
    }
    haloed(r, status, left + icon_width);
    // Bottom rows: clock, arcs box, range.
    let (bearing, range) = match target {
        Some((t, _)) => (t.bearing.as_str(), t.metric.as_str()),
        None => (page.bearing.as_str(), page.range.as_str()),
    };
    r.text(f, bearing, 1, 103, INK);
    r.text(f, range, 136 - text_width(f, range), 103, INK);
    arcs_box(r, page);
}

/// One gun's label and its state markings (plan 2.4).
fn gun_label(r: &mut Raster, f: &Font, page: &Page, slot: usize, y: i32) {
    let label = ["25", "40", "105"][slot];
    let gun = page.guns[slot];
    let width = text_width(f, label);
    // The digits are 8 rows tall; the box leaves two clear rows above and
    // below, so the 11 row pitch stacks the boxes edge to edge.
    let (x, box_w) = (3, width + 5);
    let (top, box_h) = (y - 2, 11);
    let candidate = page.candidate == Some(slot);
    let ink = if !gun.fitted || gun.status == Readiness::Empty {
        GREY
    } else {
        INK
    };
    if candidate && gun.fitted {
        r.rect(0, top, box_w, box_h, INK);
        r.text(f, label, x, y, WHITE);
    } else {
        r.text(f, label, x, y, ink);
    }
    // The box: solid when trained (a heavy right edge), dashed
    // while the gun slews, plain when it is linked and neither.
    let outline = |r: &mut Raster, dashed: bool, heavy: bool| {
        for d in 0..box_w {
            if !dashed || d % 4 < 2 {
                r.rect(d, top, 1, 1, INK);
                r.rect(d, top + box_h - 1, 1, 1, INK);
            }
        }
        for d in 0..box_h {
            if !dashed || d % 4 < 2 {
                r.rect(0, top + d, 1, 1, INK);
                r.rect(box_w - 1, top + d, 1, 1, INK);
                if heavy {
                    r.rect(box_w - 2, top + d, 1, 1, INK);
                }
            }
        }
    };
    if gun.linked && gun.fitted {
        match gun.status {
            Readiness::Ready if !candidate => outline(r, false, true),
            Readiness::GunSlewing => outline(r, true, false),
            _ if !candidate => outline(r, false, false),
            _ => {}
        }
    }
    let mid = y + 4;
    match gun.status {
        _ if !gun.fitted => r.rect(x, mid, width - 1, 1, GREY),
        Readiness::Empty => r.rect(x, mid, width - 1, 1, GREY),
        Readiness::GunArc | Readiness::MaximumRange | Readiness::MinimumRange => r.line(
            (x, y + 7),
            (x + width - 2, y),
            if candidate { WHITE } else { INK },
        ),
        _ => {}
    }
}

/// The arcs box between the clock and the range (plan 2.4).
fn arcs_box(r: &mut Raster, page: &Page) {
    let (bx, by, bw, bh) = BOX;
    r.rect(bx, by, bw, bh, BOX_FILL);
    let heading = |h: f64| -> f64 { wrap_degrees(h.to_degrees() + 90.) };
    // Forward (positive dh) is the right edge, aft the left (John, 2026-10-09).
    let to_x =
        |dh: f64| f64::from(bx + 1) + (BOX_HEADING + dh) / (2. * BOX_HEADING) * f64::from(bw - 3);
    let to_y =
        |e: f64| f64::from(by + 1) + (BOX_ELEVATION - e) / (2. * BOX_ELEVATION) * f64::from(bh - 3);
    let look_dh = heading(page.look[0]);
    let look_e = page.look[1].to_degrees();
    let lead = page.lead();
    let arc = arc(lead.unwrap_or(0));
    let outside = look_dh.abs() > arc.0 || look_e.abs() > arc.1;
    // The camera's own horizon limit.
    let horizon = to_y(camera_top()).round() as i32;
    for x in bx + 2..bx + bw - 2 {
        if page.gimbal_limit {
            r.rect(x, horizon, 1, 1, INK);
        } else if x % 3 != 0 {
            r.rect(x, horizon, 1, 1, GREY);
        }
    }
    // The candidate's own arc, in corner brackets.
    let (left, right) = (to_x(-arc.0).round() as i32, to_x(arc.0).round() as i32);
    let (top, bottom) = (to_y(arc.1).round() as i32, to_y(-arc.1).round() as i32);
    for (cx, dx) in [(left, 1), (right, -1)] {
        for (cy, dy) in [(top, 1), (bottom, -1)] {
            for d in 0..3 {
                r.rect(cx + d * dx, cy, 1, 1, GREY);
                r.rect(cx, cy + d * dy, 1, 1, GREY);
            }
        }
    }
    // Abeam ticks.
    let abeam = to_x(0.).round() as i32;
    r.rect(abeam, by + 1, 1, 2, GREY);
    r.rect(abeam, by + bh - 3, 1, 2, GREY);
    // Outline, dashed when the camera looks outside the gun's reach.
    for d in 0..bw {
        if !outside || d % 4 < 2 {
            r.rect(bx + d, by, 1, 1, INK);
            r.rect(bx + d, by + bh - 1, 1, 1, INK);
        }
    }
    for d in 0..bh {
        if !outside || d % 4 < 2 {
            r.rect(bx, by + d, 1, 1, INK);
            r.rect(bx + bw - 1, by + d, 1, 1, INK);
        }
    }
    // Gun trains: a 2 x 2 dot each, the candidate's a ring.
    let inside =
        |x: i32, y: i32| (bx + 1..bx + bw - 1).contains(&x) && (by + 1..by + bh - 1).contains(&y);
    for slot in 0..3 {
        let gun = page.guns[slot];
        if !gun.fitted || !(gun.linked || lead == Some(slot)) {
            continue;
        }
        let [h, e] = page.trains[slot];
        let (x, y) = (
            to_x(heading(h).clamp(-BOX_HEADING, BOX_HEADING)).round() as i32,
            to_y(e.to_degrees().clamp(-BOX_ELEVATION, BOX_ELEVATION)).round() as i32,
        );
        if lead == Some(slot) {
            for (dx, dy) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                if inside(x + dx, y + dy) {
                    r.rect(x + dx, y + dy, 1, 1, INK);
                }
            }
        } else {
            r.rect(x, y, 2, 2, INK);
        }
    }
    // Where the camera looks: a hollow square, or an arrowhead outside.
    let beyond_x = look_dh.abs() > BOX_HEADING;
    let beyond_y = look_e.abs() > BOX_ELEVATION;
    if !beyond_x && !beyond_y {
        let (x, y) = (to_x(look_dh).round() as i32, to_y(look_e).round() as i32);
        for d in -2..=2 {
            for (xx, yy) in [
                (x + d, y - 2),
                (x + d, y + 2),
                (x - 2, y + d),
                (x + 2, y + d),
            ] {
                if inside(xx, yy) {
                    r.rect(xx, yy, 1, 1, INK);
                }
            }
        }
        if page.gimbal_limit {
            for (dx, dy) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (0, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                if inside(x + dx, y + dy) {
                    r.rect(x + dx, y + dy, 1, 1, INK);
                }
            }
        }
    } else {
        let overshoot_x = look_dh.abs() / BOX_HEADING;
        let overshoot_y = look_e.abs() / BOX_ELEVATION;
        if beyond_x && overshoot_x >= overshoot_y {
            let y = to_y(look_e.clamp(-BOX_ELEVATION, BOX_ELEVATION)).round() as i32;
            // Forward (positive) is the right edge.
            let (x0, dir) = if look_dh > 0. {
                (bx + bw + 1, 1)
            } else {
                (bx - 2, -1)
            };
            for (col, half) in [(0, 2), (1, 1), (2, 0)] {
                r.rect(x0 + dir * col, y - half, 1, half * 2 + 1, INK);
            }
        } else {
            let x = to_x(look_dh.clamp(-BOX_HEADING, BOX_HEADING)).round() as i32;
            let (y0, dir) = if look_e > 0. {
                (by - 2, -1)
            } else {
                (by + bh, 1)
            };
            for (row, half) in [(0, 2), (1, 1), (2, 0)] {
                r.rect(x - half, y0 + dir * row, half * 2 + 1, 1, INK);
            }
        }
    }
}

/// An angle in degrees in (-180, 180].
fn wrap_degrees(angle: f64) -> f64 {
    let wrapped = (angle + 180.).rem_euclid(360.) - 180.;
    if wrapped == -180. { 180. } else { wrapped }
}

/// Preview modes for `--target-cam-preview`, which draw the page from a
/// synthetic readout on a synthetic grey scene (no retail scene, no GPU).
pub const PREVIEW_MODES: &str = "free, pinned, tracked, outside, range, close, mask, nolos, empty, returning, gimbal (gimbal-text, gimbal-bitmap force an eye icon), zoom1 to zoom6";

/// One synthetic page: the page, the target window's row for a tracked
/// sight, and the scene's RGBA picture.
pub struct Preview {
    pub page: Page,
    pub target: Option<(TargetReadout, super::TargetLink)>,
    pub scene: Vec<u8>,
}

/// A synthetic gunsight state by name (see [`PREVIEW_MODES`]).
pub fn preview(mode: &str) -> Option<Preview> {
    let view = SightView {
        position: [0.; 3],
        yaw: 0.,
        pitch: 0.,
        zoom: 8.8,
    };
    // A world point that lands `dx`, `dy` pixels from the page centre.
    let at = |dx: f64, dy: f64| -> Vector {
        let depth = 10_000.;
        let focal = f64::from(H) / 2. * 3f64.sqrt() * f64::from(view.zoom);
        [dx * depth / focal, -dy * depth / focal, depth]
    };
    // The guns' impact range sets the pipper's range arc: 13,000 feet is the
    // full scale, so 7,300 feet (1.2 NM) draws 44 percent of the circle.
    let ground_at = |dx: f64, dy: f64, range_ft: f64| Impact::Ground {
        point: at(dx, dy),
        seconds: range_ft / 2000.,
        range_ft,
    };
    let ground = |dx: f64, dy: f64| ground_at(dx, dy, 7_300.);
    let degrees = |h: f64, e: f64| [h.to_radians(), e.to_radians()];
    let mut page = Page {
        sight: Sight::Free,
        look: gunship::DEFAULT_LOOK,
        zoom: 3,
        returning: false,
        guns: [Gun {
            fitted: true,
            linked: true,
            status: Readiness::Ready,
            maximum_range_ft: 13_000.,
        }; 3],
        candidate: Some(0),
        impacts: [Some(ground(0., 0.)), None, Some(ground(16., -8.))],
        trains: [
            degrees(-90., -25.),
            degrees(-90., -20.),
            degrees(-92., -26.),
        ],
        mark: None,
        notice: None,
        gimbal_limit: false,
        limit_notice: false,
        eye: EYE_ICON,
        bearing: "9:00 LO".into(),
        range: "1.2 NM".into(),
        view: Some(view),
    };
    page.guns[1].linked = false;
    let mut target = None;
    let mut object = None;
    let status = |page: &mut Page, readiness: Readiness| {
        for gun in &mut page.guns {
            gun.status = readiness;
        }
    };
    match mode {
        "free" => {}
        "pinned" => {
            page.sight = Sight::Pinned(at(10., -8.));
            page.mark = Some(at(10., -8.));
            page.zoom = 4;
            page.look = degrees(-80., -30.);
            let gun = |linked, status| Gun {
                fitted: true,
                linked,
                status,
                maximum_range_ft: 13_000.,
            };
            page.guns = [
                gun(false, Readiness::Ready),
                gun(true, Readiness::GunSlewing),
                gun(false, Readiness::Empty),
            ];
            page.candidate = Some(1);
            page.impacts = [None, Some(ground_at(-4., 8., 4_900.)), None];
            page.trains[1] = degrees(-86., -28.);
            page.bearing = "8:00 LO".into();
            page.range = "0.8 NM".into();
        }
        "tracked" | "outside" => {
            page.sight = Sight::Tracked(7);
            page.mark = Some(at(0., -1.));
            page.impacts = [
                Some(ground_at(0., -1., 3_000.)),
                None,
                Some(ground_at(4., 1., 3_000.)),
            ];
            page.candidate = Some(2);
            page.bearing = "7:00 LO".into();
            page.range = "0.5 NM".into();
            object = Some((at(0., -1.), 11, 8));
            target = Some((
                TargetReadout {
                    id: 7,
                    name: "ZSU-23-4 SHILKA".into(),
                    damage: 0.2,
                    bearing: "7:00 LO".into(),
                    metric: "0.5 NM".into(),
                    objective: Some(crate::target_window::TargetObjective::Destroy),
                    activity: "MOVING 20 KTS".into(),
                    goal: "N",
                    player_goal: false,
                    skill: None,
                },
                super::TargetLink::default(),
            ));
            if mode == "outside" {
                page.look = degrees(-200., -20.);
                page.guns[2].status = Readiness::GunArc;
                page.guns[0].status = Readiness::GunArc;
                page.impacts = [
                    Some(ground_at(70., 6., 9_000.)),
                    None,
                    Some(ground_at(62., 10., 9_000.)),
                ];
                page.trains = [
                    degrees(-150., -18.),
                    degrees(-90., -20.),
                    degrees(-115., -20.),
                ];
                page.mark = Some(at(0., -1.));
            }
        }
        "range" => {
            status(&mut page, Readiness::MaximumRange);
            page.impacts[0] = Some(Impact::Spent {
                point: at(0., -22.),
                seconds: 10.,
                range_ft: 13_000.,
            });
            page.range = "2.4 NM".into();
        }
        "close" => {
            // Nearly full range arc: the guns' rounds land 2,000 feet out.
            page.impacts = [
                Some(ground_at(0., 0., 2_000.)),
                None,
                Some(ground_at(16., -8., 2_000.)),
            ];
            page.bearing = "9:00 LO".into();
            page.range = "0.3 NM".into();
        }
        "mask" => status(&mut page, Readiness::TerrainMask),
        "nolos" => {
            status(&mut page, Readiness::GunObscured);
            page.trains[0] = degrees(-60., -5.);
            page.look = degrees(-60., -5.);
        }
        "empty" => {
            status(&mut page, Readiness::Empty);
            page.impacts = [None; 3];
        }
        "returning" => {
            page.returning = true;
            page.look = degrees(-125., -8.);
            page.trains[0] = degrees(-112., -10.);
            page.bearing = "8:00".into();
            page.range = "6.2 NM".into();
        }
        "gimbal" | "gimbal-text" | "gimbal-bitmap" => {
            page.eye = match mode {
                "gimbal-text" => EyeIcon::Text,
                "gimbal-bitmap" => EyeIcon::Bitmap,
                _ => EYE_ICON,
            };
            page.look = degrees(-90., 0.);
            page.gimbal_limit = true;
            page.range = "13.0 NM".into();
            page.bearing = "9:00".into();
            page.impacts[0] = Some(Impact::Spent {
                point: at(0., -22.),
                seconds: 10.,
                range_ft: 13_000.,
            });
            page.guns[0].status = Readiness::MaximumRange;
            page.trains[0] = degrees(-90., 0.);
        }
        other => {
            let step: u8 = other.strip_prefix("zoom")?.parse().ok()?;
            if !(1..=6).contains(&step) {
                return None;
            }
            page.zoom = step;
        }
    }
    Some(Preview {
        page,
        target,
        scene: preview_scene(object),
    })
}

/// A synthetic grey scene: soft noise, a diagonal road, and a dark block for
/// the object (`centre x, y` pixels and half-size), drawn with alpha 255 like
/// an aircraft or static object in the real readback.
fn preview_scene(object: Option<(Vector, i32, i32)>) -> Vec<u8> {
    let (w, h) = (W as usize, H as usize);
    let mut pixels = vec![0u8; w * h * 4];
    let hash = |x: i32, y: i32| -> f64 {
        let mut n = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
        n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
        f64::from(n ^ (n >> 16)) / f64::from(u32::MAX)
    };
    for y in 0..h {
        for x in 0..w {
            // Value noise over 8 pixel cells, blended bilinearly.
            let (gx, gy) = (x as f64 / 8., y as f64 / 8.);
            let (ix, iy) = (gx.floor() as i32, gy.floor() as i32);
            let (fx, fy) = (gx - gx.floor(), gy - gy.floor());
            let mix = |a: f64, b: f64, t: f64| a + (b - a) * t;
            let top = mix(hash(ix, iy), hash(ix + 1, iy), fx);
            let bottom = mix(hash(ix, iy + 1), hash(ix + 1, iy + 1), fx);
            let mut value = 120. + 50. * mix(top, bottom, fy);
            // A road from the top left to the lower right, three pixels wide.
            let along = (x as f64 * 1.1 - y as f64 * 0.95 + 20.).abs();
            if along < 3. {
                value = 214.;
            }
            let i = (y * w + x) * 4;
            pixels[i..i + 3].fill(value as u8);
        }
    }
    if let Some((_, half_w, half_h)) = object {
        let (cx, cy) = (CENTRE.0, CENTRE.1 - 1);
        for y in cy - half_h / 2..cy + half_h / 2 {
            for x in cx - half_w / 2..cx + half_w / 2 {
                let i = (y as usize * w + x as usize) * 4;
                pixels[i..i + 4].copy_from_slice(&[60, 60, 60, 255]);
            }
        }
    }
    for i in 0..w * h {
        if pixels[i * 4 + 3] == 0 && pixels[i * 4] > 0 {
            pixels[i * 4 + 3] = 0;
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::font::Glyph;

    /// Every letter and digit a solid 5 x 8 block, advance 6: where text
    /// draws is easy to find.
    fn font() -> Font {
        Font {
            height: 10,
            glyphs: (0..256)
                .map(|c| Glyph {
                    advance: 6,
                    pixels: if c == 32 {
                        vec![]
                    } else {
                        (0..8).flat_map(|y| (0..5).map(move |x| (x, y))).collect()
                    },
                })
                .collect(),
        }
    }

    fn page(mode: &str) -> Page {
        preview(mode).expect("a preview mode").page
    }

    fn render(page: &Page) -> Raster {
        let mut r = Raster::screen();
        r.rect(0, 0, W, H, [185, 185, 185, 255]);
        draw(&mut r, &font(), page, None);
        r
    }

    fn ink(r: &Raster, x: i32, y: i32) -> bool {
        r.at(x, y) == INK
    }

    /// The ink pixels inside a screen rectangle.
    fn count(r: &Raster, x: i32, y: i32, w: i32, h: i32) -> usize {
        (y..y + h)
            .flat_map(|yy| (x..x + w).map(move |xx| (xx, yy)))
            .filter(|(xx, yy)| ink(r, *xx, *yy))
            .count()
    }

    #[test]
    fn a_trained_gun_puts_the_pipper_on_the_crosshair() {
        let r = render(&page("free"));
        // The centre dot (a small plus) and the thin ring nine pixels out.
        assert!(ink(&r, 69, 57) && ink(&r, 69 + 1, 57) && ink(&r, 69, 57 - 1));
        assert!(ink(&r, 69 + 9, 57) && ink(&r, 69 - 9, 57) && ink(&r, 69, 57 - 9));
        // The crosshair's ticks give way to the dot: nothing 3 to 5 out.
        assert!(!ink(&r, 69 + 4, 57) && !ink(&r, 69, 57 + 4));
        // A white halo surrounds the ink, and nothing sticks out past the ring.
        assert_eq!(r.at(69 + 12, 57), WHITE);
        assert_ne!(r.at(69 + 13, 57), WHITE);
        // The other linked gun's impact is a small diamond, offset 16 and 8.
        assert!(ink(&r, 69 + 16, 57 - 8 - 3) && ink(&r, 69 + 16 + 3, 57 - 8));
        assert!(!ink(&r, 69 + 16, 57 - 8));
    }

    #[test]
    fn the_range_arc_runs_clockwise_from_twelve_and_grows_as_the_range_closes() {
        // 7,300 feet of 13,000: 44 percent. Ten pixels out from the centre,
        // the band is ink at twelve and three o'clock and not at six or nine.
        let r = render(&page("free"));
        assert!(ink(&r, 69, 57 - 10) && ink(&r, 69 + 10, 57));
        assert!(!ink(&r, 69, 57 + 10) && !ink(&r, 69 - 10, 57));
        // The thin ring is whole either way.
        assert!(ink(&r, 69, 57 + 9) && ink(&r, 69 - 9, 57));
        // Closer in, the arc is longer: 2,000 feet draws about 85 percent, so
        // nine o'clock is covered and the last sliver before twelve is not.
        let close = render(&page("close"));
        assert!(ink(&close, 69 - 10, 57) && ink(&close, 69, 57 + 10));
        assert!(!ink(&close, 69 - 2, 57 - 10));
        // The scale is the shared LCOS fit: none at the maximum, half at half.
        assert_eq!(gunsight::range_arc_fraction(13_000., 13_000.), 0.);
        assert_eq!(gunsight::range_arc_fraction(6_500., 13_000.), 0.5);
        let count_band = |p: &Pixels| {
            p.iter()
                .filter(|(x, y)| f64::from(x - 69).hypot(f64::from(y - 57)) >= 9.5)
                .count()
        };
        let (none, half, full) = (
            pipper(69, 57, 0., false, true),
            pipper(69, 57, 0.5, false, true),
            pipper(69, 57, 1., false, true),
        );
        assert_eq!(count_band(&none), 0);
        assert!(count_band(&half) > 20 && count_band(&full) > 2 * count_band(&half) - 6);
        // A dashed pipper drops the thin ring's gaps but keeps the whole arc.
        let dashed = pipper(69, 57, 1., true, true);
        assert!(dashed.len() < full.len());
        assert_eq!(count_band(&dashed), count_band(&full));
    }

    #[test]
    fn a_pipper_off_the_page_parks_at_the_edge_and_the_arc_makes_it_dashed() {
        let mut p = page("outside");
        let live = render(&{
            let mut live = p.clone();
            live.guns[2].status = Readiness::Ready;
            live.guns[0].status = Readiness::Ready;
            live
        });
        let dashed = render(&p);
        // The other gun's impact is 70 pixels right of the centre: its
        // diamond parks with its right corner 13 pixels in from the page edge.
        let (px, py) = park((69. + 70., 57. + 6.));
        assert!(
            (W - 14..W).contains(&px) && ink(&live, px, py - 3),
            "diamond at {px},{py}"
        );
        // CANNOT BEAR dashes the thin ring: fewer ring pixels than the live one.
        let ring = |r: &Raster| count(r, W - 24, 57 + 6 - 11, 23, 23);
        assert!(
            ring(&dashed) < ring(&live),
            "{} {}",
            ring(&dashed),
            ring(&live)
        );
        // Behind the camera it is still parked, by direction.
        p.view = Some(SightView {
            yaw: std::f32::consts::PI,
            ..p.view.unwrap()
        });
        let behind = render(&p);
        assert!(count(&behind, 0, 0, W, 72) > 0);
    }

    #[test]
    fn beyond_range_the_ring_has_no_centre_dot() {
        let r = render(&page("range"));
        // The pipper is dashed at the last point the rounds reach, 22 above
        // the middle: no dot there and no range arc. The crosshair's ticks
        // still mark the centre.
        assert!(!ink(&r, 69, 57 - 22));
        // The thin ring is dashed 30 degrees on, 30 off, centred on twelve
        // o'clock: drawn at the top, a gap at three o'clock.
        assert!(ink(&r, 69, 57 - 22 - 9) && !ink(&r, 69 + 9, 57 - 22));
        assert!(!ink(&r, 69, 57 - 22 - 10));
        assert!(ink(&r, 69 + 4, 57) && ink(&r, 69, 57 + 4));
    }

    #[test]
    fn the_gun_list_shows_each_state() {
        let mut p = page("free");
        // Gun 25 (row y=3) is the candidate: an ink fill. Gun 105 (y=25) is
        // linked and trained: a box with a heavy right edge.
        let r = render(&p);
        assert!(ink(&r, 1, 1) && ink(&r, 1, 11));
        assert!(ink(&r, 0, 23) && ink(&r, 0, 33), "box left edge");
        let width = 18 + 5; // three glyphs of advance 6, plus the margins
        assert!(
            ink(&r, width - 1, 28) && ink(&r, width - 2, 28),
            "heavy right edge"
        );
        // Slewing dashes the box.
        p.guns[2].status = Readiness::GunSlewing;
        let slewing = render(&p);
        assert!(ink(&slewing, 0, 23) && !ink(&slewing, 0, 25));
        // Not linked: no box. Cannot bear: struck through.
        p.guns[2].linked = false;
        p.guns[2].status = Readiness::GunArc;
        let struck = render(&p);
        assert!(!ink(&struck, 0, 28) && !ink(&struck, 0, 23));
        assert!(
            ink(&struck, 3, 25 + 7),
            "the diagonal starts low on the left"
        );
        // Empty: a grey label struck flat.
        p.guns[2].status = Readiness::Empty;
        let empty = render(&p);
        assert_eq!(empty.at(5, 25 + 4), GREY);
        // A candidate that is not linked is still inverse.
        p.guns[1].linked = false;
        p.candidate = Some(1);
        let candidate = render(&p);
        assert!(ink(&candidate, 1, 12));
    }

    #[test]
    fn the_arcs_box_shows_trains_and_where_the_camera_looks() {
        let free = render(&page("free"));
        // The sight square sits in the lower middle for the default look
        // (abeam, 25 down): 5 x 5, hollow.
        let (cx, cy) = (45 + 1 + 21, 93 + 11);
        let _ = (cx, cy);
        assert!(count(&free, 46, 93, 44, 18) > 8);
        // Inside the candidate's arc the outline is solid: its corners and middle edges are ink.
        for x in [45, 55, 66, 77, 90] {
            assert!(ink(&free, x, 92), "top edge {x}");
        }
        // Outside it (a sight behind the aircraft) the outline dashes and an
        // arrowhead sits left of the box: aft is the left edge.
        let outside = render(&page("outside"));
        assert!(!(45..91).all(|x| ink(&outside, x, 92)));
        assert!(
            count(&outside, 42, 96, 3, 12) >= 3,
            "arrowhead left of the box"
        );
        assert_eq!(count(&free, 42, 96, 3, 12), 0);
        assert_eq!(count(&free, 91, 96, 6, 12), 0);
        // Forward is the right edge: a sight ahead puts the arrowhead right.
        let mut ahead = page("free");
        ahead.look = [(-10_f64).to_radians(), (-20_f64).to_radians()];
        let ahead = render(&ahead);
        assert!(count(&ahead, 91, 96, 6, 12) >= 3);
        assert_eq!(count(&ahead, 42, 96, 3, 12), 0);
    }

    #[test]
    fn the_arcs_box_puts_forward_on_the_right_and_aft_on_the_left() {
        // The 25 trained forward (heading -40) and aft (-140): the 2 x 2 dot
        // or ring lands in the right or left half of the box (45 to 90).
        let train = |heading: f64| {
            let mut p = page("free");
            p.look = gunship::DEFAULT_LOOK;
            p.trains[2] = [heading.to_radians(), (-25_f64).to_radians()];
            p.guns[2].linked = true;
            render(&p)
        };
        let side = |r: &Raster, x0: i32, x1: i32| count(r, x0, 93, x1 - x0, 18);
        // The default free view has the 25 (the candidate ring) and the 105
        // (a dot) both near the middle; compare the 105 as it moves.
        let (front, aft) = (train(-40.), train(-140.));
        assert!(
            side(&front, 78, 90) > side(&aft, 78, 90),
            "forward is right"
        );
        assert!(side(&aft, 46, 58) > side(&front, 46, 58), "aft is left");
        // The camera's own square follows: a look 40 degrees forward of abeam
        // sits right of the middle, 40 aft left of it.
        let square = |heading: f64| {
            let mut p = page("free");
            p.look = [heading.to_radians(), (-25_f64).to_radians()];
            render(&p)
        };
        let (front, aft) = (square(-50.), square(-130.));
        assert!(side(&front, 74, 90) > side(&aft, 74, 90));
        assert!(side(&aft, 46, 62) > side(&front, 46, 62));
    }

    #[test]
    fn the_camera_limit_is_a_solid_horizon_and_an_eye_in_the_status_row() {
        let free = render(&page("free"));
        let limit = render(&page("gimbal"));
        // The horizon is dotted until the camera reaches it, then solid.
        let horizon = 102; // elevation 0 is the box's middle row
        let dotted = (47..88).filter(|x| free.at(*x, horizon) == GREY).count();
        let solid = (47..88).filter(|x| ink(&limit, *x, horizon)).count();
        assert!(dotted >= 20, "{dotted}");
        assert!(solid >= 38, "{solid}");
        // The status row says GIMBAL LIMIT behind a 7 pixel eye.
        assert_eq!(status_row(&page("gimbal")), ("GIMBAL LIMIT", true));
        assert_eq!(status_row(&page("free")), ("READY", false));
        assert!(count(&limit, 0, 72, 40, 10) > 0, "the eye icon");
        assert!(count(&free, 0, 72, 40, 10) == 0);
        // The hand-drawn eye is a 9 x 5 bitmap left of the text; the text
        // eye is "<o>" in the page's own font. Both are ink, and differ.
        let mut bitmap = page("gimbal-bitmap");
        let drawn = render(&bitmap);
        bitmap.eye = EyeIcon::Text;
        assert_ne!(
            count(&drawn, 0, 72, 40, 10),
            count(&render(&bitmap), 0, 72, 40, 10)
        );
    }

    #[test]
    fn the_status_row_names_each_state_the_guns_report() {
        for (mode, label) in [
            ("range", "MAX RANGE"),
            ("mask", "TERRAIN MASK"),
            ("nolos", "NO LINE OF FIRE"),
            ("empty", "EMPTY"),
            ("outside", "CANNOT BEAR"),
        ] {
            assert_eq!(status_row(&page(mode)).0, label, "{mode}");
        }
        // Advisory labels do not block fire; the blocking ones do.
        assert!(Readiness::MaximumRange.gun_may_fire());
        assert!(Readiness::TerrainMask.gun_may_fire());
        assert!(!Readiness::GunObscured.gun_may_fire());
        assert!(!Readiness::Empty.gun_may_fire());
    }

    #[test]
    fn the_hosts_gimbal_notice_and_the_sims_arcs_drive_the_page() {
        let at = |notice, tick| Some(gunship::SightNotice { notice, tick });
        // Raised every tick the limit holds: live just after, gone a while on.
        assert!(gimbal_notice_live(at(Notice::GimbalLimit, 100), 100));
        assert!(gimbal_notice_live(at(Notice::GimbalLimit, 100), 120));
        assert!(!gimbal_notice_live(at(Notice::GimbalLimit, 100), 400));
        assert!(!gimbal_notice_live(at(Notice::DropToSlew, 100), 100));
        assert!(!gimbal_notice_live(None, 100));
        // The camera's top is the sim's hemisphere boundary; the gun arcs are
        // the sim's.
        assert_eq!(camera_top(), 0.);
        assert_eq!((arc(0).0.round(), arc(0).1.round()), (60., 60.));
        assert_eq!(arc(2).0.round(), 25.);
        assert_eq!(arc(1).1.round(), 45.);
    }

    #[test]
    fn the_camera_hits_its_limit_at_the_horizon() {
        assert!(!at_camera_limit(gunship::DEFAULT_LOOK));
        assert!(at_camera_limit([-1.5, 0.]));
        assert!(at_camera_limit([-1.5, 0.2]));
        assert!(!at_camera_limit([-1.5, (-3_f64).to_radians()]));
    }

    #[test]
    fn the_rows_follow_the_sight_mode() {
        let rows = |mode: &str| {
            let mut r = Raster::screen();
            let preview = preview(mode).unwrap();
            let target = preview.target.as_ref().map(|(t, l)| (t, l));
            draw(&mut r, &font(), &preview.page, target);
            r
        };
        // Name and zoom rows sit between the gun list and the damage bar.
        let free = rows("free");
        assert!(count(&free, 22, 3, 100, 8) > 0 && count(&free, 22, 16, 100, 8) > 0);
        assert_ne!(
            free.at(134, 3),
            [0, 0, 0, 255],
            "no damage bar without a target"
        );
        // A tracked sight keeps the target's name, activity, damage bar and objective.
        let tracked = rows("tracked");
        assert!(count(&tracked, 22, 3, 100, 8) > 0);
        assert_eq!(tracked.at(134, 3), [0, 0, 0, 255], "the damage bar");
        assert!(count(&tracked, 0, 82, 134, 10) > 0);
        // The notices replace the zoom text.
        let mut p = page("free");
        let plain = render(&p);
        p.notice = Some(PageNotice::NoGroundPoint);
        let notice = render(&p);
        assert_ne!(
            count(&plain, 22, 16, 100, 8),
            count(&notice, 22, 16, 100, 8)
        );
    }

    #[test]
    fn marks_sit_on_the_sight_point() {
        let tracked = page("tracked");
        let view = tracked.view.unwrap();
        let (x, y) = view.project(tracked.mark.unwrap()).unwrap();
        assert!((x - 69.).abs() < 0.5 && (y - 56.).abs() < 0.5, "{x} {y}");
        let r = render(&tracked);
        // An 11 pixel square: its left edge is five pixels left of the mark.
        assert!(ink(&r, 69 - 5, 56) && ink(&r, 69 + 5, 56));
        let pinned = render(&page("pinned"));
        // A 7 pixel square for a pin.
        let pin = page("pinned")
            .view
            .unwrap()
            .project(page("pinned").mark.unwrap())
            .unwrap();
        assert!(ink(&pinned, pin.0.round() as i32 - 3, pin.1.round() as i32));
    }

    #[test]
    fn every_preview_mode_draws_and_the_zoom_row_follows_the_step() {
        for mode in [
            "free",
            "pinned",
            "tracked",
            "outside",
            "range",
            "close",
            "mask",
            "nolos",
            "empty",
            "returning",
            "gimbal",
            "zoom1",
            "zoom2",
            "zoom3",
            "zoom4",
            "zoom5",
            "zoom6",
        ] {
            let preview = preview(mode).unwrap_or_else(|| panic!("{mode}"));
            assert_eq!(preview.scene.len(), 138 * 114 * 4);
        }
        assert!(preview("zoom7").is_none() && preview("nonsense").is_none());
        assert_eq!(page("zoom5").zoom, 5);
    }
}
