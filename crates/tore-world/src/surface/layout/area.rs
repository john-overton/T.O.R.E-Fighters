//! Enemy area defenses (docs/spec/surface-defenses.md, "Area defenses";
//! John, 2026-10-12, B23): a Quick Mission with no ground target whose AAA
//! or SAM strength is above none still meets the enemy's SAM sites and guns
//! on Red's side of the front.
//!
//! - **Where.** Each site is a seeded point on Red's side of the theater's
//!   front axis, [`AREA_DEPTH_NM`] deep from its midpoint; one site in
//!   [`AREA_AIRFIELD_EVERY`] instead guards a Red airfield on Red's side,
//!   [`AREA_AIRFIELD_NM`] from its runway. Sites keep [`AREA_SITE_SPACING_NM`]
//!   apart.
//! - **How many.** [`AREA_SAM_SITES`] and [`AREA_AAA_SITES`] by the
//!   creator's strength, [`AREA_SITE_UNITS`] units each.
//! - **Which.** A uniform pick from the enemy nationality's equipment group
//!   for `<sam>` and `<aaa>`, as the targets' defenses; the battery rules
//!   then give an SA-6 (SA-2, SA-3, HAWK) site its radar.
//! - **How.** Standing still, no route, no supply truck, no front line: just
//!   the site. Never a destroy target.
//!
//! Every unit stands where a ground unit may ([`stands`]: on level land,
//! off the sea), [`AREA_OBJECT_CLEARANCE_FT`] clear of every layout object
//! (towns, buildings, airfield objects) and [`AREA_RUNWAY_CLEARANCE_FT`]
//! clear of every runway. Everything comes from the mission's surface seed
//! through one stream per site and integer arithmetic, so every machine and
//! every replay places the same sites on the same foot.
use super::{
    Added, Inputs, Obstacles, Pose, added_unit_for, distance2, ground::CELL_FT, ring, stands, trig,
    *,
};
use crate::ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE};
use crate::surface::{Surface, UnitId, resolve::AreaDefense};
use tore_formats::quick_template::{
    Placeholder,
    tables::{NIGHT_AAA, NIGHT_AAA_SKILL, equipment, group_of},
};

/// The stream ordinal of the site draws: site `k` keys `AREA_ORDINAL + k`
/// in the area's own stem, apart from every template's.
const AREA_ORDINAL: u32 = 0;

/// The stem of the area's streams: the layout's, marked as the area's.
fn stem(layout: &str) -> String {
    format!("AREA:{layout}")
}

/// Where a pass draws a site's centre: around an airfield's runway origin,
/// or in a depth band (feet past the front's midpoint toward Red).
#[derive(Clone, Copy)]
enum Pass {
    Field([i64; 2]),
    Band([i64; 2]),
}

/// One site as drawn: what it holds and where it may stand.
struct SiteDraw {
    placeholder: Placeholder,
    /// Guards a Red airfield (the runway's origin) rather than the band.
    airfield: Option<[i64; 2]>,
}

/// Why a mission's area defenses stand as they do, for the log and the
/// dump.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub sam_sites: usize,
    pub aaa_sites: usize,
    pub wanted_sam: usize,
    pub wanted_aaa: usize,
}

/// Adds the area defenses of `surface.area` (no ground target), placed and
/// ready to join the surface, or notes why there are none.
pub(super) fn add(
    surface: &Surface,
    inputs: &mut Inputs<'_, '_>,
    obstacles: &Obstacles<'_>,
    notes: &mut Vec<String>,
) -> Vec<Added> {
    let Some(area) = &surface.area else {
        return Vec::new();
    };
    if surface.template.is_some() {
        // A ground target's defenses are its own (never both).
        return Vec::new();
    }
    let Some(front) = inputs.ground.front else {
        notes.push("no area defenses: the theater has no front".to_owned());
        return Vec::new();
    };
    let Some(group) = group_of(area.enemy_nationality) else {
        notes.push("no area defenses: the enemy has no equipment group".to_owned());
        return Vec::new();
    };
    let stem = stem(inputs.layout);
    let axis = front.axis();
    let length = trig::isqrt((axis[0].pow(2) + axis[1].pow(2)) as u128) as i64;
    if length == 0 {
        notes.push("no area defenses: the front has no axis".to_owned());
        return Vec::new();
    }
    let mid = [
        (front.blue[0] + front.red[0]).div_euclid(2),
        (front.blue[1] + front.red[1]).div_euclid(2),
    ];
    // Depth in feet past the midpoint toward Red.
    let depth_ft = |p: [i64; 2]| (front.depth(p) / i128::from(2 * length)) as i64;
    let toward_red = trig::bearing(axis);
    let extent = inputs.ground.extent();
    let margin = AREA_EDGE_CELLS * CELL_FT;
    // The band runs across the whole map: as far each way as its farthest
    // corner from the midpoint.
    let reach = [[0, 0], [extent[0], 0], [0, extent[1]], extent]
        .iter()
        .map(|c| trig::isqrt(distance2(*c, mid) as u128) as i64)
        .max()
        .unwrap_or(0);
    // Red's airfields on Red's side, ascending id: the runway's own side,
    // an unowned one by its side of the front.
    let fields: Vec<[i64; 2]> = inputs
        .ground
        .runways
        .iter()
        .filter(|r| {
            let side = surface.side_of(r.id);
            side == ENEMY_SIDE || (side != FRIENDLY_SIDE && !front.blue_side(r.at))
        })
        .filter(|r| (1..=AREA_AIRFIELD_DEPTH_NM * NM_FT).contains(&depth_ft(r.at)))
        .map(|r| r.at)
        .collect();
    let draws = sites(area, &fields, &stem);
    let wanted_sam = draws
        .iter()
        .filter(|d| d.placeholder == Placeholder::Sam)
        .count();
    let wanted_aaa = draws.len() - wanted_sam;
    let mut placed_sites: Vec<[i64; 2]> = Vec::new();
    let mut out = Vec::new();
    let mut summary = Summary {
        wanted_sam,
        wanted_aaa,
        ..Summary::default()
    };
    let spacing = AREA_SITE_SPACING_NM * NM_FT;
    let bands = [AREA_DEPTH_NM, AREA_FALLBACK_DEPTH_NM].map(|b| b.map(|nm| nm * NM_FT));
    for (k, draw) in draws.iter().enumerate() {
        let mut stream = Stream::new(area.seed, &stem, AREA_ORDINAL + k as u32, Purpose::Group);
        let mut pick = Stream::new(area.seed, &stem, AREA_ORDINAL + k as u32, Purpose::Pick);
        let Some((entry, skill)) = kind(inputs, draw.placeholder, group, area, &mut pick) else {
            notes.push(format!(
                "area site {k}: the import has no <{}> of group {group}",
                draw.placeholder.name()
            ));
            continue;
        };
        let radius = (inputs.types)(&entry.resource).footprint.radius();
        // The passes: by its airfield when it guards one, then the band,
        // then deeper on Red's side when the band holds no site (an island
        // theater's band is mostly sea).
        let passes: Vec<Pass> = draw
            .airfield
            .map(Pass::Field)
            .into_iter()
            .chain(bands.map(Pass::Band))
            .collect();
        let mut found = None;
        'passes: for pass in passes {
            for _ in 0..AREA_CANDIDATES {
                let centre = match pass {
                    Pass::Field(field) => {
                        let (offset, _) = ring(&mut stream, AREA_AIRFIELD_NM.map(|nm| nm * NM_FT));
                        [field[0] + offset[0], field[1] + offset[1]]
                    }
                    Pass::Band([near, far]) => {
                        let depth = near + stream.below((far - near) as u64 + 1) as i64;
                        let lateral = stream.below(2 * reach as u64 + 1) as i64 - reach;
                        let a = trig::along(toward_red, depth);
                        let b = trig::along(toward_red + 90, lateral);
                        [mid[0] + a[0] + b[0], mid[1] + a[1] + b[1]]
                    }
                };
                if !inputs.ground.inside(centre, margin)
                    || depth_ft(centre) <= 0
                    || placed_sites
                        .iter()
                        .any(|p| distance2(*p, centre) < spacing * spacing)
                {
                    continue;
                }
                if let Some(poses) = members(inputs, obstacles, centre, radius, &mut stream) {
                    found = Some((centre, poses));
                    break 'passes;
                }
            }
        }
        let Some((centre, poses)) = found else {
            notes.push(format!(
                "area site {k}: no place for its <{}> on Red's side",
                draw.placeholder.name()
            ));
            continue;
        };
        placed_sites.push(centre);
        match draw.placeholder {
            Placeholder::Sam => summary.sam_sites += 1,
            _ => summary.aaa_sites += 1,
        }
        for pose in poses {
            let Some(id) = UnitId::area_defense(out.len() as u32) else {
                continue;
            };
            out.push(Added {
                unit: added_unit_for(
                    id,
                    &entry,
                    pose,
                    Some(area.enemy_nationality as i32 | 0x80),
                    ENEMY_SIDE,
                    skill,
                ),
                source: stem.clone(),
            });
        }
    }
    notes.push(format!(
        "area defenses: {} of {} SAM sites, {} of {} AAA sites, {} units",
        summary.sam_sites,
        summary.wanted_sam,
        summary.aaa_sites,
        summary.wanted_aaa,
        out.len()
    ));
    out
}

/// The sites a strength asks for, SAM sites first: every
/// [`AREA_AIRFIELD_EVERY`]th of each kind (the first included) guards a Red
/// airfield, taken in turn from a seeded first one; without one it stands
/// in the band.
fn sites(area: &AreaDefense, fields: &[[i64; 2]], stem: &str) -> Vec<SiteDraw> {
    let sam = AREA_SAM_SITES.get(area.sam).copied().unwrap_or(0);
    let aaa = AREA_AAA_SITES.get(area.aaa).copied().unwrap_or(0);
    let mut first = Stream::new(area.seed, stem, u32::MAX, Purpose::Group);
    let mut next_field = if fields.is_empty() {
        0
    } else {
        first.below(fields.len() as u64) as usize
    };
    let mut out = Vec::with_capacity(sam + aaa);
    for (placeholder, count) in [(Placeholder::Sam, sam), (Placeholder::Aaa, aaa)] {
        for n in 0..count {
            let airfield = (n % AREA_AIRFIELD_EVERY == 0 && !fields.is_empty()).then(|| {
                let at = fields[next_field % fields.len()];
                next_field += 1;
                at
            });
            out.push(SiteDraw {
                placeholder,
                airfield,
            });
        }
    }
    out
}

/// The type a site holds and its skill: a uniform pick from the enemy
/// group's list (the night rule's gun for `<aaa>` when it holds); a type the
/// import cannot place gives way to the next in the list.
fn kind(
    inputs: &mut Inputs<'_, '_>,
    placeholder: Placeholder,
    group: usize,
    area: &AreaDefense,
    pick: &mut Stream,
) -> Option<(std::sync::Arc<super::Entry>, i32)> {
    let night = placeholder == Placeholder::Aaa && area.night_stealth;
    let (list, skill) = if night {
        (NIGHT_AAA, NIGHT_AAA_SKILL)
    } else {
        (
            equipment(placeholder, group)?,
            crate::surface::resolve::DEFAULT_SKILL,
        )
    };
    let start = pick.below(list.len() as u64) as usize;
    (0..list.len())
        .find_map(|step| (inputs.added)(list[(start + step) % list.len()]))
        .map(|entry| (entry, skill))
}

/// Where a site's units stand around `centre`: each a seeded spot in the
/// [`AREA_MEMBER_FT`] ring, clear of the others, of every layout object by
/// [`AREA_OBJECT_CLEARANCE_FT`] and of every runway by
/// [`AREA_RUNWAY_CLEARANCE_FT`]; `None` when one finds no place.
fn members(
    inputs: &Inputs<'_, '_>,
    obstacles: &Obstacles<'_>,
    centre: [i64; 2],
    radius: i64,
    stream: &mut Stream,
) -> Option<Vec<Pose>> {
    let ground = inputs.ground;
    let mut out: Vec<Pose> = Vec::with_capacity(AREA_SITE_UNITS);
    for _ in 0..AREA_SITE_UNITS {
        let mut chosen = None;
        for _ in 0..JITTER_CANDIDATES {
            let (offset, bearing) = ring(stream, AREA_MEMBER_FT);
            let at = [centre[0] + offset[0], centre[1] + offset[1]];
            let apart = out
                .iter()
                .all(|p| distance2(p.at, at) >= (2 * radius + AREA_MEMBER_GAP_FT).pow(2));
            if apart
                && stands(ground, obstacles, at, radius, false)
                && !ground.near_object(at, AREA_OBJECT_CLEARANCE_FT)
                && !ground.near_runway(at, AREA_RUNWAY_CLEARANCE_FT + radius)
            {
                // Facing out from the site's middle.
                chosen = Some(Pose {
                    at,
                    heading: trig::wrap(bearing),
                });
                break;
            }
        }
        out.push(chosen?);
    }
    Some(out)
}

/// The placed area units of `surface`, ascending id.
pub fn units(surface: &Surface) -> impl Iterator<Item = &crate::surface::Unit> + '_ {
    surface
        .units
        .iter()
        .filter(|u| u.id.range() == crate::surface::IdRange::AreaDefense)
}

/// The area rules the placed area units break on `ground`: each must stand
/// inside the map, on Red's side of the front, where a ground unit may, and
/// clear of the layout's objects and runways by the area clearances. Empty
/// when every unit holds (the `surface-area-sweep` scenario).
pub fn audit(
    surface: &Surface,
    ground: &Ground<'_>,
    types: &mut dyn FnMut(&str) -> TypeInfo,
) -> Vec<String> {
    let mut out = Vec::new();
    let obstacles = Obstacles {
        fixed: &[],
        strips: &[],
    };
    for unit in units(surface) {
        let at = [i64::from(unit.position[0]), i64::from(unit.position[2])];
        let radius = types(&unit.resource).footprint.radius();
        let mut broken = Vec::new();
        if !ground.inside(at, AREA_EDGE_CELLS * CELL_FT - AREA_MEMBER_FT[1]) {
            broken.push("off the map");
        }
        if ground.front.is_some_and(|front| front.blue_side(at)) {
            broken.push("on Blue's side");
        }
        if !stands(ground, &obstacles, at, radius, false) {
            broken.push("where no ground unit stands");
        }
        if ground.near_object(at, AREA_OBJECT_CLEARANCE_FT) {
            broken.push("near a layout object");
        }
        if ground.near_runway(at, AREA_RUNWAY_CLEARANCE_FT + radius) {
            broken.push("near a runway");
        }
        if unit.side != ENEMY_SIDE || unit.is_target() {
            broken.push("not an enemy defense");
        }
        for why in broken {
            out.push(format!("{:#010x} {} {why}", unit.id.0, unit.resource));
        }
    }
    out
}
