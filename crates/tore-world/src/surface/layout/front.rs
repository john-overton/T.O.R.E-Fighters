//! The front line (docs/spec/surface-defenses.md, "The front line"; John,
//! 2026-10-11: a Quick Mission places opposing ground forces in contact so a
//! surface battle is visible).
//!
//! A mission with a ground target gets a few Blue and Red ground units
//! standing in two lines that face each other [`FRONT_GAP_FT`] apart, a few
//! miles from the targets. They are the theater's own equipment: Red draws
//! from the enemy nationality's equipment group (tanks, an armoured car and,
//! with AAA on, a gun), Blue from the Western group. They are no destroy
//! targets and no part of the objective; the surface units' own rules make
//! them fight (tanks and, now, gun AAA fire at ground units in reach).
//!
//! The site is the first of a seeded list of candidate centres that is level
//! land, stands clear of every layout object and runway, keeps
//! [`FRONT_CLEAR_NM`] from every template unit and every leg of a routed
//! unit's path (so no front unit reaches the targets), from both airborne
//! starts and from every runway a ground start may use, and holds every unit. Everything comes from
//! the template's own stream and integer arithmetic, so every machine places
//! the same front on the same foot.
use super::{
    Added, Inputs, Obstacles, Pose, added_unit_for, distance2, sites::FRONT_ORDINAL, spread,
    stands, trig, xz, *,
};
use crate::surface::{Surface, UnitId};
use tore_formats::quick_template::{Placeholder, tables};

/// What one side's line holds, in lateral order from the left: a light
/// vehicle on the flank, two tanks, and a gun when AAA is on.
const LINE: [Placeholder; 4] = [
    Placeholder::Afv,
    Placeholder::Tank,
    Placeholder::Tank,
    Placeholder::Aaa,
];

/// One unit of the front before it stands anywhere.
struct Member {
    /// Whose it is.
    red: bool,
    entry: std::sync::Arc<Entry>,
    /// Lateral place in the line, half-spacings from its middle.
    lateral: i64,
    /// Feet behind the line (away from the enemy).
    behind: i64,
    radius: i64,
}

/// Adds the front line (`added`), or notes why there is none.
pub(super) fn add(
    surface: &Surface,
    inputs: &mut Inputs<'_, '_>,
    obstacles: &Obstacles<'_>,
    added: &mut Vec<Added>,
    notes: &mut Vec<String>,
) {
    let Some(site) = &surface.template else {
        return;
    };
    if !site.settings.variation.front {
        return;
    }
    let Some(starts) = &surface.starts else {
        return;
    };
    let mut draws = Stream::new(
        site.settings.seed,
        &site.stem,
        FRONT_ORDINAL,
        Purpose::Group,
    );
    let members = members(surface, inputs, &mut draws);
    if !members.iter().any(|m| m.red) || !members.iter().any(|m| !m.red) {
        notes.push("no front line: the import lacks a side's equipment".to_owned());
        return;
    }
    let target = [i64::from(starts.target[0]), i64::from(starts.target[1])];
    // The front stands between the targets and Blue: toward the layout's
    // Blue centroid, else toward Blue's start.
    let blue_side = match &inputs.ground.front {
        Some(front) => front.blue,
        None => [i64::from(starts.blue[0]), i64::from(starts.blue[1])],
    };
    let toward_blue = trig::bearing([blue_side[0] - target[0], blue_side[1] - target[1]]);
    // Everything the front keeps clear of: the template's units and aircraft
    // as placed, every leg of a routed unit's path (review M3: a column or a
    // truck drives far from its start), both airborne starts, and the
    // runways a ground start of either side may use (review L1).
    let clear_of: Vec<[i64; 2]> = surface
        .template_units()
        .map(|u| xz(u.position))
        .chain(surface.parked.iter().map(|p| xz(p.position)))
        .collect();
    let paths: Vec<Vec<[i64; 2]>> = surface
        .template_units()
        .filter_map(|u| {
            let route = u.route.as_ref()?;
            Some(
                std::iter::once(xz(u.position))
                    .chain(route.legs().map(|leg| xz(leg.position)))
                    .collect(),
            )
        })
        .collect();
    let fields: Vec<&super::ground::Placed> = starts
        .blue_airfields
        .iter()
        .chain(&starts.red_airfields)
        .filter_map(|id| inputs.ground.runways.iter().find(|r| r.id == *id))
        .collect();
    let clear = FRONT_CLEAR_NM * NM_FT;
    let starts_at = [
        [i64::from(starts.red[0]), i64::from(starts.red[1])],
        [i64::from(starts.blue[0]), i64::from(starts.blue[1])],
    ];
    for [near, far] in FRONT_BANDS_NM {
        for _ in 0..FRONT_CANDIDATES {
            let radius = near * NM_FT + draws.below(((far - near) * NM_FT) as u64 + 1) as i64;
            let bearing = toward_blue + spread(&mut draws, 90);
            let offset = trig::along(bearing, radius);
            let centre = [target[0] + offset[0], target[1] + offset[1]];
            // The line's axis, Blue toward Red: from the centre toward the
            // targets, turned a little.
            let axis = trig::bearing([target[0] - centre[0], target[1] - centre[1]])
                + spread(&mut draws, 45);
            if clear_of
                .iter()
                .chain(&starts_at)
                .any(|p| distance2(*p, centre) < clear * clear)
                || paths.iter().any(|path| {
                    path.windows(2)
                        .any(|leg| near_leg(centre, leg[0], leg[1], clear))
                })
                || fields.iter().any(|runway| runway.within(centre, clear))
            {
                continue;
            }
            if let Some(poses) = poses(inputs, obstacles, &members, centre, axis, &mut draws) {
                for (n, (member, pose)) in members.iter().zip(poses).enumerate() {
                    let id = UnitId::front_line(n as u32).expect("a few front units");
                    let (nationality, side) = if member.red {
                        (
                            Some(site.settings.enemy_nationality as i32 | 0x80),
                            crate::ai_wings::ENEMY_SIDE,
                        )
                    } else {
                        (Some(0), crate::ai_wings::FRIENDLY_SIDE)
                    };
                    added.push(Added {
                        unit: added_unit_for(
                            id,
                            &member.entry,
                            pose,
                            nationality,
                            side,
                            crate::surface::resolve::DEFAULT_SKILL,
                        ),
                        source: format!("~{}.M", site.stem),
                    });
                }
                return;
            }
        }
    }
    notes.push("no front line: no site holds the two lines".to_owned());
}

/// Whether `point` lies closer than `clear` feet to the leg from `a` to `b`
/// (exact, in integers).
pub(super) fn near_leg(point: [i64; 2], a: [i64; 2], b: [i64; 2], clear: i64) -> bool {
    let wide = |v: i64| i128::from(v);
    let leg = [wide(b[0] - a[0]), wide(b[1] - a[1])];
    let to = [wide(point[0] - a[0]), wide(point[1] - a[1])];
    let length2 = leg[0] * leg[0] + leg[1] * leg[1];
    let along = to[0] * leg[0] + to[1] * leg[1];
    let clear2 = wide(clear) * wide(clear);
    if along <= 0 || length2 == 0 {
        to[0] * to[0] + to[1] * to[1] < clear2
    } else if along >= length2 {
        let from_b = [wide(point[0] - b[0]), wide(point[1] - b[1])];
        from_b[0] * from_b[0] + from_b[1] * from_b[1] < clear2
    } else {
        let cross = to[0] * leg[1] - to[1] * leg[0];
        cross * cross < clear2 * length2
    }
}

/// Both sides' units: Blue's, then Red's, each drawn from its equipment
/// group; a type the import cannot place gives way to the next in the list.
fn members(surface: &Surface, inputs: &mut Inputs<'_, '_>, draws: &mut Stream) -> Vec<Member> {
    let Some(site) = &surface.template else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for red in [false, true] {
        let group = if red { site.group } else { FRONT_BLUE_GROUP };
        for (slot, placeholder) in LINE.into_iter().enumerate() {
            // With AAA off there is no gun (the line keeps its tanks and vehicle).
            if placeholder == Placeholder::Aaa && site.settings.aaa == 0 {
                continue;
            }
            let Some(list) = tables::equipment(placeholder, group) else {
                continue;
            };
            let start = draws.below(list.len() as u64) as usize;
            let found =
                (0..list.len()).find_map(|step| (inputs.added)(list[(start + step) % list.len()]));
            let Some(entry) = found else {
                continue;
            };
            let radius = (inputs.types)(&entry.resource).footprint.radius();
            out.push(Member {
                red,
                entry,
                lateral: 2 * slot as i64 - 3,
                behind: match placeholder {
                    Placeholder::Afv => 200,
                    Placeholder::Aaa => 700,
                    _ => 0,
                },
                radius,
            });
        }
    }
    out
}

/// Where every member stands for a front centred at `centre` with its axis
/// along `axis` (Blue toward Red), or `None` when one has no place: each
/// tries its spot and a few seeded moves of [`FRONT_JITTER_FT`].
fn poses(
    inputs: &mut Inputs<'_, '_>,
    obstacles: &Obstacles<'_>,
    members: &[Member],
    centre: [i64; 2],
    axis: i32,
    draws: &mut Stream,
) -> Option<Vec<Pose>> {
    let mut out = Vec::with_capacity(members.len());
    for member in members {
        // Blue stands back from the middle toward Blue, Red toward Red.
        let (forward, facing) = if member.red {
            (FRONT_GAP_FT / 2 + member.behind, axis + 180)
        } else {
            (-(FRONT_GAP_FT / 2) - member.behind, axis)
        };
        let side = trig::along(axis + 90, member.lateral * FRONT_SPACING_FT / 2);
        let ahead = trig::along(axis, forward);
        let spot = [
            centre[0] + side[0] + ahead[0],
            centre[1] + side[1] + ahead[1],
        ];
        let mut chosen = None;
        for _ in 0..JITTER_CANDIDATES {
            let d = disc(draws, FRONT_JITTER_FT);
            let at = [spot[0] + d[0], spot[1] + d[1]];
            if stands(inputs.ground, obstacles, at, member.radius, false) {
                chosen = Some(Pose {
                    at,
                    heading: facing + spread(draws, FRONT_TURN_DEG),
                });
                break;
            }
        }
        out.push(chosen?);
    }
    // Every unit sees every unit of the other line over the ground.
    for (i, a) in members.iter().enumerate() {
        for (j, b) in members.iter().enumerate().skip(i + 1) {
            if a.red != b.red && !inputs.ground.sees(out[i].at, out[j].at, FRONT_EYE_FT) {
                return None;
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::near_leg;

    #[test]
    fn a_leg_is_near_along_its_length_and_at_its_ends_only() {
        let (a, b) = ([0, 0], [10_000, 0]);
        // Beside the middle of the leg.
        assert!(near_leg([5_000, 999], a, b, 1_000));
        assert!(!near_leg([5_000, 1_000], a, b, 1_000));
        // Past either end the distance is to that end.
        assert!(near_leg([10_600, 600], a, b, 1_000));
        assert!(!near_leg([10_800, 800], a, b, 1_000));
        assert!(near_leg([-700, 700], a, b, 1_000));
        assert!(!near_leg([-800, 800], a, b, 1_000));
        // A leg of no length is a point.
        assert!(near_leg([300, 400], a, a, 501));
        assert!(!near_leg([300, 400], a, a, 500));
    }
}
