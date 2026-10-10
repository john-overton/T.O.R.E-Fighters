//! Start placement with a ground target (docs/spec/surface-defenses.md,
//! "Start placement"; John, 2026-10-10): Red starts within 5 nm of the
//! target, defending it; Blue starts the mission's separation from Red,
//! heading at the target; the airfields a ground start may use are ranked
//! by their distance from the target.
use super::{centroid, distance2, ground::CELL_FT, sites::START_ORDINAL, trig, xz, *};
use crate::ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE};
use crate::surface::{StartPoints, Surface};

/// The point the starts follow: the targets' placed centroid, else every
/// template object's, else the template's `quickpos`.
fn target_point(surface: &Surface) -> Option<[i64; 2]> {
    let site = surface.template.as_ref()?;
    let targets = surface
        .template_units()
        .filter(|u| u.is_target())
        .map(|u| xz(u.position))
        .chain(
            surface
                .parked
                .iter()
                .filter(|p| p.target)
                .map(|p| xz(p.position)),
        );
    centroid(targets)
        .or_else(|| {
            centroid(
                surface
                    .template_units()
                    .map(|u| xz(u.position))
                    .chain(surface.parked.iter().map(|p| xz(p.position))),
            )
        })
        .or_else(|| site.quickpos.map(xz))
}

/// Blue's start and the ranked airfields, or `None` without a template.
pub(super) fn place(surface: &Surface, ground: &Ground<'_>) -> Option<StartPoints> {
    let site = surface.template.as_ref()?;
    let target = target_point(surface)?;
    let mut draws = Stream::new(
        site.settings.seed,
        &site.stem,
        START_ORDINAL,
        Purpose::Group,
    );
    let extent = ground.extent();
    let margin = START_MARGIN_CELLS * CELL_FT;
    // Red defends the target: a seeded spot within RED_START_NM of its
    // centroid that is on the map; the centroid itself when none of the
    // tries is.
    let red = (0..JITTER_CANDIDATES)
        .map(|_| {
            let d = disc(&mut draws, RED_START_NM * NM_FT);
            [target[0] + d[0], target[1] + d[1]]
        })
        .find(|at| ground.inside(*at, margin))
        .unwrap_or(target);
    // Blue starts the mission's separation from Red, toward Blue's side of
    // the front (toward the map's middle without one, fitted), spread by a
    // seeded turn.
    let toward = match &ground.front {
        Some(front) => [front.blue[0] - front.red[0], front.blue[1] - front.red[1]],
        None => [extent[0] / 2 - red[0], extent[1] / 2 - red[1]],
    };
    let bearing = trig::bearing(toward) + spread(&mut draws, BLUE_START_SPREAD_DEG);
    let separation = i64::from(site.settings.separation_nm) * NM_FT;
    // Off the map, the bearing is searched one degree at a time, clockwise
    // first, as the enemy placement does; when no bearing fits at the full
    // separation, the farthest that fits, a mile at a time.
    let turns: Vec<i32> = (0..=180)
        .flat_map(|step: i32| {
            std::iter::once(step).chain((step != 0 && step != 180).then_some(-step))
        })
        .collect();
    let mut fitted = None;
    let mut distance = separation;
    while fitted.is_none() && distance >= 0 {
        fitted = turns
            .iter()
            .map(|turn| bearing + turn)
            .map(|b| {
                let d = trig::along(b, distance);
                (b, [red[0] + d[0], red[1] + d[1]])
            })
            .find(|(_, at)| ground.inside(*at, margin));
        distance -= NM_FT;
    }
    let (_, blue) = fitted.unwrap_or((bearing, red));
    // Blue heads at the target.
    let heading = trig::bearing([target[0] - blue[0], target[1] - blue[1]]);
    // Airfields at least the minimum away, nearest first (lower id on a
    // tie), by the side that owns them; an unowned one by its side of the
    // front, or both sides' without a front.
    let floor = (AIRFIELD_MIN_NM * NM_FT).pow(2);
    let mut ranked: Vec<(i64, u32)> = ground
        .runways
        .iter()
        .map(|r| (distance2(r.at, target), r.id))
        .filter(|(d, _)| *d >= floor)
        .collect();
    ranked.sort_unstable();
    let (mut blue_fields, mut red_fields) = (Vec::new(), Vec::new());
    for (_, id) in ranked {
        let side = surface.side_of(id);
        let at = ground
            .runways
            .iter()
            .find(|r| r.id == id)
            .map_or(target, |r| r.at);
        let (blue_side, red_side) = if side == FRIENDLY_SIDE {
            (true, false)
        } else if side == ENEMY_SIDE {
            (false, true)
        } else {
            match &ground.front {
                Some(front) => (front.blue_side(at), !front.blue_side(at)),
                None => (true, true),
            }
        };
        if blue_side {
            blue_fields.push(id);
        }
        if red_side {
            red_fields.push(id);
        }
    }
    let whole = |v: i64| i32::try_from(v).unwrap_or(i32::MAX);
    Some(StartPoints {
        target: target.map(whole),
        red: red.map(whole),
        blue: blue.map(whole),
        blue_heading_deg: heading,
        blue_airfields: blue_fields,
        red_airfields: red_fields,
    })
}
