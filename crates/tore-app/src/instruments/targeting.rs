//! The targeting page: the TARGET CAM window of an aircraft with a laser or
//! FLIR sight, and of an aircraft with no sight that has a buddy's laser spot
//! to drop on (docs/spec/target-window.md, "Targeting page"; plan 3.4).
//!
//! Opinionated (John, 2026-10-10: the target display shows when it is lasing,
//! and shows when the spot is a buddy's; the layout is the agent's). It is the
//! AC-130 gunsight page's sibling: the same camera idea (the picture looks
//! along the sight), the same ink-with-a-white-halo marks, minus the guns.
//! What it adds is the laser row (LASE, BUDDY, LSR RDY, MASK), the laser range
//! and a slot for the time to impact of the aircraft's own laser or GPS
//! weapon in flight.
//!
//! The decision to show the page is [`form`]; the data is [`Page::new`] from
//! the readout; the host completes it with the presented look and camera
//! ([`Page::present`]); [`draw`] paints over the picture the window already
//! drew.
use super::{
    Raster,
    gunsight::{
        CENTRE, EYE_BITMAP, EyeIcon, INK, PageNotice, Pixels, SightView, W, WHITE, bearing_label,
        crosshair, square, stamp, text_width,
    },
};
use crate::target_window::Readout as TargetReadout;
use tore_formats::font::Font;
use tore_sim::{
    attitude::Vector,
    combat::{
        ballistics,
        laser::{self, LaserState, Mask},
        live::Configuration,
        sight::{Gimbal, Notice, SightMode},
    },
};
use tore_world::readout::{CockpitReadout, LaserReadout};

/// How long a one-off notice stays on the activity row, in combat ticks (2 s).
const NOTICE_TICKS: u64 = 240;
/// The sim's gimbal-limit notice is live for this many ticks after it is
/// raised (it is raised every tick the camera is held at its limit).
const LIMIT_NOTICE_TICKS: u64 = 30;
/// Degrees short of the gimbal's edge that count as against it.
const LIMIT_MARGIN: f64 = 0.5;
/// The laser cue blinks twice a second: lit for the first quarter second of
/// each half.
const BLINK_TICKS: u64 = 30;
/// A buddy-form picture frames a spot area this tall, feet (fitted).
const BUDDY_FRAME_FT: f64 = 500.;

/// Which page it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// The aircraft's own sight: free, pinned or tracking, from the pod or
    /// the nose.
    Sight,
    /// An aircraft with no sight looking at a buddy's laser spot.
    Buddy,
}

/// The weapon records that go for the ground and are neither bombs, rocket
/// pods nor laser weapons (the data carries no class; fitted from the plan's
/// path table until the weapon slices name one).
const GROUND_RECORDS: [&str; 14] = [
    "AGM65G", "AGM65A", "AGM84E", "AGM88", "AGM45", "AGM84A", "AM39", "AS16", "AS7", "AS15", "AT2",
    "GBU29", "GBU29P", "GBU30",
];

/// The selected weapon is for the ground: a bomb, a rocket pod, a laser
/// weapon or one of the ground missiles.
pub fn ground_weapon_selected(ro: &CockpitReadout, config: &Configuration) -> bool {
    let Some(station) = config.stations.get(ro.stores.selected()) else {
        return false;
    };
    let weapon = &station.weapon;
    let stem = weapon
        .source
        .split('.')
        .next()
        .unwrap_or(&weapon.source)
        .to_ascii_uppercase();
    ballistics::is_bomb(weapon)
        || ballistics::is_pod(weapon)
        || laser::is_laser_weapon(weapon)
        || GROUND_RECORDS.contains(&stem.as_str())
}

/// Whether the target display is the targeting page, and which form
/// (plan 3.4). `None` on the AC-130 (its gunsight page) and whenever it is the
/// plain target window.
///
/// Sight aircraft: the sight is pinned, tracking or slewed off its default,
/// the laser is on (lasing or masked), or a ground weapon is selected. An
/// aircraft with no sight gets the buddy form while a laser weapon is
/// selected and a buddy's spot is shown (the readout carries the laser then).
pub fn form(ro: &CockpitReadout, config: &Configuration) -> Option<Form> {
    if ro.gunsight.is_some() {
        return None;
    }
    match (&ro.sight, &ro.laser) {
        (Some(sight), laser) => {
            let lasing =
                laser.is_some_and(|l| matches!(l.state, LaserState::Lasing | LaserState::Masked));
            let wanted = !matches!(sight.mode, SightMode::Free)
                || sight.slewed()
                || lasing
                || ground_weapon_selected(ro, config);
            wanted.then_some(Form::Sight)
        }
        (None, Some(laser)) if !laser.designator && laser.spot.is_some() => Some(Form::Buddy),
        (None, _) => None,
    }
}

/// What the laser row says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// Lasing: the spot is on the sight's point. Blinks.
    Lase,
    /// The spot is someone else's. Blinks.
    Buddy,
    /// The sight holds a point and the laser is off.
    Ready,
    /// Wanted but blocked.
    Mask(Mask),
}

impl Cue {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lase => "LASE",
            Self::Buddy => "BUDDY",
            Self::Ready => "LSR RDY",
            Self::Mask(_) => "MASK",
        }
    }
    fn blinks(self) -> bool {
        matches!(self, Self::Lase | Self::Buddy)
    }
}

/// The time to impact slot: the oldest laser or GPS weapon of the aircraft in
/// flight, from the weapon slice's `WeaponTti` (W1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tti {
    /// Seconds to go.
    pub seconds: f64,
    /// The weapon has hit: IMPACT shows for a second.
    pub impact: bool,
}

/// The slot from the readout (slice W1): the oldest of the aircraft's guided
/// weapons in flight whose time can be worked out, else IMPACT for a second
/// after the last one ended.
fn tti_of(ro: &CockpitReadout) -> Option<Tti> {
    if let Some(seconds) = ro.weapon_tti.iter().find_map(|w| w.seconds) {
        return Some(Tti {
            seconds,
            impact: false,
        });
    }
    ro.guided_impact_tick
        .filter(|tick| ro.tick.saturating_sub(*tick) < 120)
        .map(|_| Tti {
            seconds: 0.,
            impact: true,
        })
}

/// `TTI m:ss` or `IMPACT`.
pub fn tti_text(tti: Tti) -> String {
    if tti.impact {
        return "IMPACT".into();
    }
    let whole = tti.seconds.max(0.).ceil() as u32;
    format!("TTI {}:{:02}", whole / 60, whole % 60)
}

/// Feet under a nautical mile, else nautical miles with one decimal.
pub fn range_text(feet: f64) -> String {
    let nmi = feet / tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
    if nmi < 1. {
        format!("{} FT", feet.round().max(0.) as u32)
    } else {
        format!("{nmi:.1} NM")
    }
}

/// Everything the targeting page draws that is not the camera's picture.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub form: Form,
    /// What the sight holds; a buddy form is `Free`.
    pub mode: SightMode,
    /// Body-relative [heading, elevation] of the line of sight, radians, as
    /// presented this frame (the buddy form has none).
    pub look: [f64; 2],
    pub zoom: u8,
    pub returning: bool,
    /// The sight's gimbal; the buddy form has none.
    pub gimbal: Option<Gimbal>,
    /// The point the pin or track square marks (a buddy spot in the buddy
    /// form).
    pub mark: Option<Vector>,
    /// The first laser row, and a BUDDY row beneath it when the spot of the
    /// first is the aircraft's own.
    pub cue: Option<Cue>,
    pub second_buddy: bool,
    /// The laser range to show, feet, or the sight range.
    pub range_ft: Option<f64>,
    pub tti: Option<Tti>,
    pub notice: Option<PageNotice>,
    pub gimbal_limit: bool,
    limit_notice: bool,
    pub eye: EyeIcon,
    /// Clock bearing with Hi/Lo, and the range text of the bottom rows when
    /// no tracked target window supplies them.
    pub bearing: String,
    pub range: String,
    /// The lasing pilot's callsign for a buddy spot, set by the host.
    pub owner: Option<String>,
    /// Combat tick of the readout: the blink's clock.
    pub tick: u64,
    pub view: Option<SightView>,
}

impl Page {
    /// The page's data from the readout; `None` when the display is not the
    /// targeting page.
    pub fn new(
        ro: &CockpitReadout,
        config: &Configuration,
        player: &crate::flight::State,
    ) -> Option<Self> {
        let form = form(ro, config)?;
        let laser = ro.laser.as_ref();
        let buddy_spot = laser.and_then(|l| l.spot.filter(|s| !s.own).or(l.buddy));
        let (cue, second_buddy) = laser_rows(laser, form, buddy_spot.is_some());
        let (mode, look, returning, zoom, gimbal, aim, notice_raw) = match &ro.sight {
            Some(s) if form == Form::Sight => (
                s.mode,
                s.look,
                s.returning,
                s.zoom,
                Some(s.sensor.gimbal),
                s.aim,
                s.notice,
            ),
            _ => (SightMode::Free, [0.; 2], false, 3, None, None, None),
        };
        let mark = match (form, mode) {
            (Form::Buddy, _) => buddy_spot.map(|s| s.spot.position),
            (_, SightMode::Pinned(point)) => Some(point),
            (_, SightMode::Tracked(_)) => ro.targets.display.as_ref().map(|t| t.position),
            (_, SightMode::Free) => None,
        };
        let now = ro.tick;
        let notice = notice_raw.and_then(|n| {
            if n.tick > now || now - n.tick >= NOTICE_TICKS {
                return None;
            }
            match n.notice {
                Notice::NoGroundPoint => Some(PageNotice::NoGroundPoint),
                Notice::DropToSlew => Some(PageNotice::DropToSlew),
                Notice::GimbalLimit => None,
            }
        });
        let limit_notice = notice_raw.is_some_and(|n| {
            n.notice == Notice::GimbalLimit && n.tick <= now && now - n.tick < LIMIT_NOTICE_TICKS
        });
        let point = mark.or(aim).unwrap_or(player.position);
        let offset: Vector = std::array::from_fn(|i| point[i] - player.position[i]);
        let straight = offset.iter().map(|v| v * v).sum::<f64>().sqrt();
        // The range the page reads: the laser's while it has a point or a
        // buddy's spot, else the line to the sight's point.
        let range_ft = match (cue, laser) {
            (Some(Cue::Buddy), Some(l)) => l.spot.map(|s| s.range_ft),
            (Some(_), Some(l)) if l.range_ft > 0. => Some(l.range_ft),
            _ => None,
        }
        .or(Some(straight));
        Some(Self {
            form,
            mode,
            look,
            zoom,
            returning,
            gimbal,
            mark,
            cue,
            second_buddy,
            range_ft,
            tti: tti_of(ro),
            notice,
            gimbal_limit: limit_notice || at_limit(gimbal, look),
            limit_notice,
            eye: super::gunsight::EYE_ICON,
            bearing: bearing_label(offset, player.yaw),
            range: range_text(straight),
            owner: None,
            tick: now,
            view: None,
        })
    }

    /// Completes the page with this frame's presented look, the client's zoom
    /// and the camera's view, the presented point of a tracked object and the
    /// lasing pilot's callsign.
    pub fn present(
        &mut self,
        look: [f64; 2],
        zoom: u8,
        view: SightView,
        mark: Option<Vector>,
        owner: Option<String>,
    ) {
        if self.form == Form::Sight {
            self.look = look;
            self.zoom = zoom;
            self.gimbal_limit = self.limit_notice || at_limit(self.gimbal, look);
        }
        self.view = Some(view);
        if mark.is_some() {
            self.mark = mark;
        }
        self.owner = owner;
    }

    fn masked(&self) -> Option<Mask> {
        match self.cue {
            Some(Cue::Mask(reason)) => Some(reason),
            _ => None,
        }
    }
}

/// The first laser row and whether a second BUDDY row sits under it.
fn laser_rows(laser: Option<&LaserReadout>, form: Form, buddy: bool) -> (Option<Cue>, bool) {
    let Some(laser) = laser else {
        return (None, false);
    };
    if form == Form::Buddy || !laser.designator {
        return (buddy.then_some(Cue::Buddy), false);
    }
    let own = match laser.state {
        LaserState::Lasing => Some(Cue::Lase),
        LaserState::Ready => Some(Cue::Ready),
        LaserState::Masked => Some(Cue::Mask(laser.mask.unwrap_or(Mask::Range))),
        LaserState::Off => None,
    };
    match own {
        Some(cue) => (Some(cue), buddy),
        None => (buddy.then_some(Cue::Buddy), false),
    }
}

/// The camera sits against its gimbal's edge, from the look alone.
fn at_limit(gimbal: Option<Gimbal>, look: [f64; 2]) -> bool {
    match gimbal {
        Some(Gimbal::Hemisphere) => look[1].to_degrees() >= -LIMIT_MARGIN,
        Some(Gimbal::Cone { search, .. }) => {
            look[0].hypot(look[1]).to_degrees() >= search.to_degrees() - LIMIT_MARGIN
        }
        None => false,
    }
}

/// The zoom step that frames a buddy's spot area at `distance` feet from the
/// camera, within the ladder (fitted: [`BUDDY_FRAME_FT`] tall).
pub fn buddy_zoom_step(distance: f64) -> u8 {
    let field = 2. * (BUDDY_FRAME_FT / 2. / distance.max(1.)).atan();
    (1..=6)
        .rev()
        .find(|step| tore_sim::combat::sight::field_of_view(*step) >= field)
        .unwrap_or(1)
}

/// A symbol drawn with a white halo in the status rows' manner.
fn haloed(r: &mut Raster, f: &Font, s: &str, x: i32, y: i32) {
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        r.text(f, s, x + dx, y + dy, WHITE);
    }
    r.text(f, s, x, y, INK);
}

fn centred(r: &mut Raster, f: &Font, s: &str, y: i32, left: i32, width: i32, halo: bool) {
    let x = left + (width - text_width(f, s)) / 2;
    if halo {
        haloed(r, f, s, x, y);
    } else {
        r.text(f, s, x, y, INK);
    }
}

/// Draws the page over the picture the caller already painted.
pub(super) fn draw(
    r: &mut Raster,
    f: &Font,
    page: &Page,
    target: Option<(&TargetReadout, &super::TargetLink)>,
) {
    let fit = |s: &str, limit: usize| super::fit_width(f, s, limit);
    let tracked = matches!(page.mode, SightMode::Tracked(_));
    let target = target.filter(|_| tracked && page.form == Form::Sight);
    // Name row.
    let buddy_row = page.cue == Some(Cue::Buddy);
    let name = match (page.form, target, page.mode) {
        // A buddy's spot: its pilot's callsign leads the page.
        (Form::Buddy, _, _) => page
            .owner
            .as_deref()
            .map_or_else(|| "LASER SPOT".to_string(), tore_formats::text::upper),
        _ if buddy_row && page.owner.is_some() => {
            tore_formats::text::upper(page.owner.as_deref().unwrap_or_default())
        }
        (_, Some((t, _)), _) => tore_formats::text::upper(&t.name),
        (_, None, SightMode::Tracked(_)) => "TRACK".into(),
        (_, None, SightMode::Pinned(_)) => "PIN".into(),
        (_, None, SightMode::Free) if page.returning => "RETURN".into(),
        (_, None, SightMode::Free) => "SLEW".into(),
    };
    centred(r, f, &fit(&name, 119), 3, 1, 120, false);
    // Activity row: a notice, the target's activity, or the zoom step.
    let activity = match (page.notice, target) {
        (Some(PageNotice::NoGroundPoint), _) => "NO GROUND POINT".to_string(),
        (Some(PageNotice::DropToSlew), _) => "L TO DROP".to_string(),
        (None, Some((t, _))) => t.activity.clone(),
        (None, None) if page.form == Form::Buddy || tracked => String::new(),
        (None, None) => format!("ZOOM {}", page.zoom),
    };
    centred(r, f, &fit(&activity, 119), 16, 1, 120, false);
    if let Some((t, _)) = target {
        r.text(f, t.goal, 124, 3, INK);
        if t.player_goal {
            r.rect(124, 13, 6, 1, INK);
        }
        for dot in 0..t.skill.unwrap_or(0) {
            r.rect(122 + i32::from(dot) * 4, -10, 2, 2, [230, 230, 230, 255]);
        }
        r.rect(133, 1, 4, 48, [240, 240, 240, 255]);
        r.rect(134, 2, 2, 46, [0, 0, 0, 255]);
        let filled = (t.damage * 46.).round() as i32;
        r.rect(134, 48 - filled, 2, filled, WHITE);
        let objective = match t.objective {
            Some(crate::target_window::TargetObjective::Survive) => "Obj: Survive",
            Some(crate::target_window::TargetObjective::Destroy) => "Obj: Destroy",
            None => "",
        };
        centred(r, f, objective, 82, 0, 134, false);
    }
    // Laser rows. LASE and BUDDY blink twice a second; the rest are steady.
    let lit = |cue: Cue| !cue.blinks() || page.tick % (2 * BLINK_TICKS) < BLINK_TICKS;
    if let Some(cue) = page.cue.filter(|cue| lit(*cue)) {
        centred(r, f, cue.label(), 28, 1, 120, true);
    }
    if page.second_buddy && lit(Cue::Buddy) {
        centred(r, f, "BUDDY", 39, 1, 120, true);
    }
    // Marks.
    let mut marks = Pixels::new();
    if let Some(view) = &page.view
        && let Some((x, y)) = page.mark.and_then(|point| view.project(point))
    {
        let side = if tracked { 10 } else { 6 };
        marks.extend(square(x.round() as i32, y.round() as i32, side));
    }
    marks.extend(crosshair());
    stamp(r, &marks);
    // Status row: the gimbal limit with the eye, else why the laser is masked.
    let status = if page.gimbal_limit || page.masked() == Some(Mask::Gimbal) {
        Some(("GIMBAL LIMIT", true))
    } else {
        match page.masked() {
            Some(Mask::Range) => Some(("OUT OF RANGE", false)),
            Some(Mask::Terrain) => Some(("TERRAIN MASK", false)),
            _ => None,
        }
    };
    if let Some((text, eye)) = status {
        let icon = match (eye, page.eye) {
            (false, _) => 0,
            (true, EyeIcon::Text) => text_width(f, "<o> "),
            (true, EyeIcon::Bitmap) => 9 + 3,
        };
        let left = CENTRE.0 - (icon + text_width(f, text)) / 2;
        if eye {
            match page.eye {
                EyeIcon::Text => haloed(r, f, "<o>", left, 72),
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
        haloed(r, f, text, left + icon, 72);
    }
    // Time to impact, under the objective row.
    if let Some(tti) = page.tti {
        centred(r, f, &tti_text(tti), 92, 0, W, true);
    }
    // Bottom rows: the clock bearing and the range. A laser range replaces the
    // target window's alternating metric.
    let (bearing, window_range) = match target {
        Some((t, _)) => (t.bearing.as_str(), Some(t.metric.as_str())),
        None => (page.bearing.as_str(), None),
    };
    let laser_range = page
        .cue
        .filter(|cue| !matches!(cue, Cue::Ready))
        .and(page.range_ft)
        .map(range_text);
    let range = laser_range
        .as_deref()
        .or(window_range)
        .unwrap_or(page.range.as_str());
    r.text(f, bearing, 1, 103, INK);
    r.text(f, range, 136 - text_width(f, range), 103, INK);
}

/// Preview modes for `--target-cam-preview` that draw the targeting page.
pub const PREVIEW_MODES: &str = "lase, lase-off, lase-mask, lase-mask-range, lase-mask-terrain, lase-pin, ready, slew, limit, tti, impact, buddy, both, buddy-nosight, flir, flir-tracked";

/// One synthetic page: see [`super::gunsight::Preview`].
pub struct Preview {
    pub page: Page,
    pub target: Option<(TargetReadout, super::TargetLink)>,
    pub scene: Vec<u8>,
}

/// A synthetic targeting page by name (see [`PREVIEW_MODES`]).
pub fn preview(mode: &str) -> Option<Preview> {
    let view = SightView {
        position: [0.; 3],
        yaw: 0.,
        pitch: 0.,
        zoom: 8.8,
    };
    let at = |dx: f64, dy: f64| -> Vector {
        let depth = 10_000.;
        let focal = f64::from(super::gunsight::H) / 2. * 3f64.sqrt() * f64::from(view.zoom);
        [dx * depth / focal, -dy * depth / focal, depth]
    };
    let degrees = |h: f64, e: f64| [h.to_radians(), e.to_radians()];
    let mut page = Page {
        form: Form::Sight,
        mode: SightMode::Tracked(7),
        look: degrees(12., -18.),
        zoom: 3,
        returning: false,
        gimbal: Some(Gimbal::Hemisphere),
        mark: Some(at(0., -1.)),
        cue: Some(Cue::Lase),
        second_buddy: false,
        range_ft: Some(26_000.),
        tti: None,
        notice: None,
        gimbal_limit: false,
        limit_notice: false,
        eye: super::gunsight::EYE_ICON,
        bearing: "1:00 LO".into(),
        range: "4.3 NM".into(),
        owner: None,
        tick: 0,
        view: Some(view),
    };
    let tank = || {
        (
            TargetReadout {
                id: 7,
                name: "T-72 TANK".into(),
                damage: 0.,
                bearing: "1:00 LO".into(),
                metric: "4.3 NM".into(),
                objective: Some(crate::target_window::TargetObjective::Destroy),
                activity: "MOVING 12 KTS".into(),
                goal: "N",
                player_goal: false,
                skill: None,
            },
            super::TargetLink::default(),
        )
    };
    let mut target = Some(tank());
    let mut object = Some((at(0., -1.), 11, 8));
    match mode {
        "lase" => {}
        "lase-off" => page.tick = 40,
        "lase-mask" | "lase-mask-range" => {
            page.cue = Some(Cue::Mask(Mask::Range));
            page.range_ft = Some(68_000.);
            page.range = "11.2 NM".into();
        }
        "lase-mask-terrain" => {
            page.cue = Some(Cue::Mask(Mask::Terrain));
        }
        "lase-pin" => {
            page.mode = SightMode::Pinned(at(10., -8.));
            page.mark = Some(at(10., -8.));
            target = None;
            object = None;
            page.bearing = "1:00 LO".into();
        }
        "ready" => {
            page.mode = SightMode::Pinned(at(0., -1.));
            page.cue = Some(Cue::Ready);
            target = None;
            object = None;
        }
        "slew" => {
            page.mode = SightMode::Free;
            page.mark = None;
            page.cue = None;
            page.zoom = 4;
            target = None;
            object = None;
        }
        "limit" => {
            page.mode = SightMode::Free;
            page.mark = None;
            page.look = degrees(-90., 0.);
            page.gimbal_limit = true;
            page.cue = None;
            target = None;
            object = None;
        }
        "tti" | "impact" => {
            page.tti = Some(Tti {
                seconds: 12.,
                impact: mode == "impact",
            });
        }
        "buddy" => {
            page.cue = Some(Cue::Buddy);
            page.owner = Some("VIPER 2".into());
            page.range_ft = Some(31_000.);
            page.range = "5.1 NM".into();
        }
        "both" => {
            page.second_buddy = true;
            page.owner = Some("VIPER 2".into());
        }
        "buddy-nosight" => {
            page.form = Form::Buddy;
            page.mode = SightMode::Free;
            page.gimbal = None;
            page.cue = Some(Cue::Buddy);
            page.owner = Some("VIPER 2".into());
            page.mark = Some(at(3., 0.));
            page.range_ft = Some(31_000.);
            page.range = "5.1 NM".into();
            page.bearing = "12:00 LO".into();
            page.tti = Some(Tti {
                seconds: 34.,
                impact: false,
            });
            target = None;
            object = Some((at(3., 0.), 9, 6));
        }
        "flir" => {
            page.cue = None;
            page.mode = SightMode::Pinned(at(10., -8.));
            page.mark = Some(at(10., -8.));
            page.range_ft = Some(40_000.);
            page.range = "6.6 NM".into();
            target = None;
            object = None;
        }
        "flir-tracked" => {
            page.cue = None;
            page.range_ft = Some(26_000.);
        }
        _ => return None,
    }
    Some(Preview {
        page,
        target,
        scene: super::gunsight::preview_scene(object),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::font::Glyph;

    /// Every glyph a solid 5 x 8 block, advance 6, so where text draws is
    /// easy to find.
    fn font() -> Font {
        Font {
            height: 10,
            glyphs: (0..256)
                .map(|_| Glyph {
                    advance: 6,
                    pixels: (0..5).flat_map(|x| (0..8).map(move |y| (x, y))).collect(),
                })
                .collect(),
        }
    }

    fn render(mode: &str) -> Raster {
        let shown = preview(mode).unwrap();
        let mut r = Raster::screen();
        r.rect(0, 0, W, super::super::gunsight::H, [185, 185, 185, 255]);
        let target = shown.target.as_ref().map(|(t, l)| (t, l));
        draw(&mut r, &font(), &shown.page, target);
        r
    }

    fn ink(r: &Raster, x: i32, y: i32, w: i32, h: i32) -> usize {
        (y..y + h)
            .flat_map(|yy| (x..x + w).map(move |xx| (xx, yy)))
            .filter(|(xx, yy)| r.at(*xx, *yy) == INK)
            .count()
    }

    #[test]
    fn every_preview_mode_draws() {
        for mode in PREVIEW_MODES
            .split(',')
            .map(|m| m.trim().split(' ').next().unwrap())
        {
            assert!(preview(mode).is_some(), "{mode}");
            render(mode);
        }
        assert!(preview("nonsense").is_none());
    }

    #[test]
    fn the_laser_row_names_each_state() {
        assert_eq!(Cue::Lase.label(), "LASE");
        assert_eq!(Cue::Buddy.label(), "BUDDY");
        assert_eq!(Cue::Ready.label(), "LSR RDY");
        assert_eq!(Cue::Mask(Mask::Range).label(), "MASK");
    }

    #[test]
    fn lase_and_buddy_blink_twice_a_second_and_the_rest_do_not() {
        // The laser row is 10 pixels tall at y 28: ink shows only while lit.
        let on = ink(&render("lase"), 1, 28, 120, 10);
        assert!(on > 0);
        // 40 ticks into the second: the dark half of the 60 tick cycle.
        assert_eq!(ink(&render("lase-off"), 1, 28, 120, 10), 0);
        // Steady cues draw in both halves.
        for tick in [0, 40] {
            let mut shown = preview("ready").unwrap();
            shown.page.tick = tick;
            let mut r = Raster::screen();
            r.rect(0, 0, W, super::super::gunsight::H, [185, 185, 185, 255]);
            draw(&mut r, &font(), &shown.page, None);
            assert!(ink(&r, 1, 28, 120, 10) > 0, "tick {tick}");
        }
    }

    #[test]
    fn the_range_reads_in_feet_under_a_mile() {
        assert_eq!(range_text(26_000.), "4.3 NM");
        assert_eq!(range_text(5_200.), "5200 FT");
        assert_eq!(range_text(6_076.), "1.0 NM");
        assert_eq!(range_text(0.), "0 FT");
    }

    #[test]
    fn the_time_to_impact_counts_down_and_flashes_impact() {
        let t = |seconds, impact| tti_text(Tti { seconds, impact });
        assert_eq!(t(12., false), "TTI 0:12");
        assert_eq!(t(61.2, false), "TTI 1:02");
        assert_eq!(t(0.4, false), "TTI 0:01");
        assert_eq!(t(0., true), "IMPACT");
    }

    #[test]
    fn a_buddy_row_sits_under_an_own_lase_and_masks_only_name_their_reason() {
        let both = preview("both").unwrap().page;
        assert_eq!(both.cue, Some(Cue::Lase));
        assert!(both.second_buddy);
        // Masked for terrain: the status row says why; masked for the gimbal:
        // the eye leads it.
        assert!(ink(&render("lase-mask-terrain"), 1, 72, 136, 10) > 0);
        assert_eq!(ink(&render("lase"), 1, 72, 136, 10), 0);
    }

    #[test]
    fn a_buddy_form_names_the_lasing_pilot() {
        let shown = preview("buddy-nosight").unwrap();
        assert_eq!(shown.page.form, Form::Buddy);
        assert_eq!(shown.page.owner.as_deref(), Some("VIPER 2"));
        // The name row is drawn from the callsign.
        assert!(ink(&render("buddy-nosight"), 1, 3, 120, 10) > 0);
    }

    #[test]
    fn the_zoom_step_for_a_buddy_spot_narrows_with_distance() {
        assert!(buddy_zoom_step(1_000.) < buddy_zoom_step(6_000.));
        assert_eq!(buddy_zoom_step(1.), 1);
        assert_eq!(buddy_zoom_step(1_000_000.), 6);
    }

    use tore_sim::combat::{laser::LaserSpot, live::Side};
    use tore_world::readout::{SightReadout, SpotReadout};

    /// A readout, its configuration and a flight state to build pages from.
    fn world() -> (CockpitReadout, Configuration, crate::flight::State) {
        let state = tore_world::test_support::combat_fixture(false);
        let config = state.own().configuration().clone();
        let flight =
            crate::flight::State::new(&tore_world::test_support::profile(), [0., 5000., 0.])
                .unwrap();
        let ro =
            tore_world::readout::build(&state, 0, crate::combat::launcher(&flight), None, None)
                .unwrap();
        (ro, config, flight)
    }
    fn sensor(laser: bool) -> tore_sim::combat::sight::SightSensor {
        tore_sim::combat::sight::SightSensor {
            laser,
            range_ft: 60_760.,
            gimbal: Gimbal::Hemisphere,
            eye: [0., -4., 0.],
            default_look: tore_sim::combat::sight::FIGHTER_DEFAULT_LOOK,
        }
    }
    fn sight(laser: bool, mode: SightMode, slewed: bool) -> SightReadout {
        let sensor = sensor(laser);
        SightReadout {
            mode,
            look: if slewed {
                [0.4, -0.5]
            } else {
                sensor.default_look
            },
            returning: false,
            aim: Some([0., 0., 20_000.]),
            notice: None,
            zoom: 3,
            sensor,
        }
    }
    fn laser(designator: bool, state: LaserState) -> LaserReadout {
        LaserReadout {
            designator,
            state,
            mask: None,
            manual: None,
            range_ft: 26_200.,
            limit_ft: 60_760.,
            spot: None,
            buddy: None,
        }
    }
    fn spot(own: bool, feet: f64) -> SpotReadout {
        SpotReadout {
            spot: LaserSpot {
                owner: if own { 0 } else { 4 },
                side: Side(1),
                position: [0., 0., 20_000.],
                target: None,
            },
            own,
            range_ft: feet,
        }
    }

    #[test]
    fn an_aircraft_with_no_sight_and_no_buddy_has_the_plain_window() {
        let (ro, config, _) = world();
        assert_eq!(form(&ro, &config), None);
    }

    #[test]
    fn a_sight_shows_the_page_when_pinned_tracking_slewed_lasing_or_a_ground_weapon_is_selected() {
        let (mut ro, mut config, _) = world();
        // At rest on its default look with nothing selected for the ground:
        // the plain window.
        ro.sight = Some(sight(true, SightMode::Free, false));
        ro.laser = Some(laser(true, LaserState::Off));
        assert_eq!(form(&ro, &config), None);
        // Slewed off the default.
        ro.sight = Some(sight(true, SightMode::Free, true));
        assert_eq!(form(&ro, &config), Some(Form::Sight));
        // Pinned or tracking, on the default look.
        for mode in [SightMode::Pinned([0., 0., 9_000.]), SightMode::Tracked(7)] {
            ro.sight = Some(sight(true, mode, false));
            assert_eq!(form(&ro, &config), Some(Form::Sight), "{mode:?}");
        }
        // Lasing or masked.
        ro.sight = Some(sight(true, SightMode::Free, false));
        for state in [LaserState::Lasing, LaserState::Masked] {
            ro.laser = Some(laser(true, state));
            assert_eq!(form(&ro, &config), Some(Form::Sight), "{state:?}");
        }
        // A ground weapon selected.
        ro.laser = Some(laser(true, LaserState::Off));
        let selected = ro.stores.selected();
        config.stations[selected].weapon.source = "AGM88.JT".into();
        assert_eq!(form(&ro, &config), Some(Form::Sight));
        config.stations[selected].weapon.source = "SYNTHETIC.JT".into();
        assert_eq!(form(&ro, &config), None);
    }

    #[test]
    fn a_ground_weapon_is_a_bomb_a_pod_a_laser_weapon_or_a_listed_missile() {
        let (ro, mut config, _) = world();
        let selected = ro.stores.selected();
        for (source, flags, ground) in [
            ("MK82.JT", 0x10, true),
            ("GBU10.JT", 0x400, false),
            ("AGM65G.JT", 0, true),
            ("AIM9.JT", 0, false),
        ] {
            config.stations[selected].weapon.source = source.into();
            config.stations[selected].weapon.flags = flags;
            assert_eq!(ground_weapon_selected(&ro, &config), ground, "{source}");
        }
    }

    #[test]
    fn the_gunsight_aircraft_keeps_its_own_page() {
        let (mut ro, config, _) = world();
        ro.sight = Some(sight(true, SightMode::Tracked(7), false));
        ro.gunsight = Some(tore_world::readout::GunsightReadout {
            sight: SightMode::Free,
            look: [0.; 2],
            returning: false,
            aim: None,
            impacts: [None; 3],
            impacts_tick: 0,
            status: [tore_sim::combat::live::Readiness::Ready; 3],
            notice: None,
            zoom: 3,
        });
        assert_eq!(form(&ro, &config), None);
    }

    #[test]
    fn an_aircraft_with_no_sight_gets_the_buddy_form_only_with_a_spot() {
        let (mut ro, config, _) = world();
        ro.laser = Some(laser(false, LaserState::Off));
        assert_eq!(form(&ro, &config), None);
        ro.laser = Some(LaserReadout {
            spot: Some(spot(false, 31_300.)),
            ..laser(false, LaserState::Off)
        });
        assert_eq!(form(&ro, &config), Some(Form::Buddy));
    }

    #[test]
    fn the_laser_row_follows_the_laser_state() {
        let (mut ro, config, flight) = world();
        ro.sight = Some(sight(true, SightMode::Pinned([0., 0., 20_000.]), false));
        let rows = |ro: &mut CockpitReadout, l: LaserReadout| {
            ro.laser = Some(l);
            let page = Page::new(ro, &config, &flight).unwrap();
            (page.cue, page.second_buddy, page.range_ft.unwrap().round())
        };
        let own = spot(true, 26_300.);
        let friend = spot(false, 31_300.);
        // Lasing: LASE with the laser's range; a buddy as well adds a row.
        let lasing = LaserReadout {
            spot: Some(own),
            ..laser(true, LaserState::Lasing)
        };
        assert_eq!(rows(&mut ro, lasing), (Some(Cue::Lase), false, 26_200.));
        let both = LaserReadout {
            buddy: Some(friend),
            ..lasing
        };
        assert_eq!(rows(&mut ro, both), (Some(Cue::Lase), true, 26_200.));
        // Ready and masked show their words; a masked laser says why.
        let ready = laser(true, LaserState::Ready);
        assert_eq!(rows(&mut ro, ready).0, Some(Cue::Ready));
        let masked = LaserReadout {
            mask: Some(Mask::Terrain),
            ..laser(true, LaserState::Masked)
        };
        assert_eq!(rows(&mut ro, masked).0, Some(Cue::Mask(Mask::Terrain)));
        // The laser off, a buddy's spot homed on: BUDDY with the range to it.
        let buddy_only = LaserReadout {
            spot: Some(friend),
            ..laser(true, LaserState::Off)
        };
        assert_eq!(
            rows(&mut ro, buddy_only),
            (Some(Cue::Buddy), false, 31_300.)
        );
        // No designator: the buddy form's row.
        ro.sight = None;
        let nosight = LaserReadout {
            spot: Some(friend),
            ..laser(false, LaserState::Off)
        };
        let page = {
            ro.laser = Some(nosight);
            Page::new(&ro, &config, &flight).unwrap()
        };
        assert_eq!((page.form, page.cue), (Form::Buddy, Some(Cue::Buddy)));
        assert_eq!(page.mark, Some(friend.spot.position));
    }

    #[test]
    fn the_page_reports_the_sensors_gimbal_limit_and_the_sims_notice() {
        let (mut ro, config, flight) = world();
        let mut s = sight(false, SightMode::Free, true);
        s.look = [0., 0.];
        ro.sight = Some(s);
        let page = Page::new(&ro, &config, &flight).unwrap();
        assert!(page.gimbal_limit);
        // Back inside the gimbal, the sim's notice alone raises it for a
        // short while after the last tick it was raised.
        s.look = [0., -0.4];
        s.notice = Some(tore_sim::combat::sight::SightNotice {
            notice: Notice::GimbalLimit,
            tick: ro.tick,
        });
        ro.sight = Some(s);
        assert!(Page::new(&ro, &config, &flight).unwrap().gimbal_limit);
        s.notice = None;
        ro.sight = Some(s);
        assert!(!Page::new(&ro, &config, &flight).unwrap().gimbal_limit);
    }

    #[test]
    fn the_time_to_impact_slot_draws_the_countdown_and_impact() {
        let draw_with = |tti: Option<Tti>| {
            let mut shown = preview("lase").unwrap();
            shown.page.tti = tti;
            let mut r = Raster::screen();
            r.rect(0, 0, W, super::super::gunsight::H, [185, 185, 185, 255]);
            draw(&mut r, &font(), &shown.page, None);
            ink(&r, 0, 92, 138, 10)
        };
        assert_eq!(draw_with(None), 0);
        assert!(
            draw_with(Some(Tti {
                seconds: 12.,
                impact: false
            })) > 0
        );
        assert!(
            draw_with(Some(Tti {
                seconds: 0.,
                impact: true
            })) > 0
        );
    }

    #[test]
    fn the_gimbal_limit_is_read_from_the_look() {
        assert!(at_limit(Some(Gimbal::Hemisphere), [0., 0.]));
        assert!(!at_limit(Some(Gimbal::Hemisphere), [0., -0.3]));
        let cone = Gimbal::Cone {
            search: 1.047,
            track: 0.785,
        };
        assert!(at_limit(Some(cone), [1.047, 0.]));
        assert!(!at_limit(Some(cone), [0.2, -0.2]));
        assert!(!at_limit(None, [0., 0.]));
    }

    #[test]
    fn the_tti_slot_reads_the_oldest_guided_weapon_then_impact_for_a_second() {
        let (mut ro, _, _) = world();
        assert_eq!(tti_of(&ro), None);
        let weapon = |seconds| tore_world::readout::WeaponTti {
            projectile: 1,
            station: 0,
            kind: tore_sim::combat::missiles::Guidance::Laser,
            guiding: true,
            seconds,
            position: [0.; 3],
        };
        ro.weapon_tti = vec![weapon(None), weapon(Some(12.4)), weapon(Some(3.))];
        let tti = tti_of(&ro).unwrap();
        assert_eq!((tti.seconds, tti.impact), (12.4, false));
        assert_eq!(tti_text(tti), "TTI 0:13");
        ro.weapon_tti.clear();
        ro.tick = 1_000;
        ro.guided_impact_tick = Some(950);
        assert!(tti_of(&ro).unwrap().impact);
        ro.guided_impact_tick = Some(800);
        assert_eq!(tti_of(&ro), None);
    }
}
