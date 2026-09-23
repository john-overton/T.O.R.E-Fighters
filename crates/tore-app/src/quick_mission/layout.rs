//! Quick Mission launch geometry, as plain functions of numbers: where the
//! enemy group goes so it stays on the map, and where the player's wing parks
//! for a ground start. The rules and numbers are in
//! `docs/spec/quick-mission-menu.md` ("Mission wings" and "Player ground
//! start"). Nothing here reads the world or a flight model, so every rule can
//! be tested with synthetic bounds and runways.

use tore_sim::{
    ai::airfield::AirfieldAnchors,
    airport::{ApproachEnd, Runway},
};

/// The manual (p.19) gives the separation in nautical miles.
pub const FEET_PER_NM: f64 = 6_076.12;

/// Separation choices in nautical miles. The first six are the retail list;
/// 200 and 300 are host additions requested by John on 2026-09-23.
pub const SEPARATION_NM: [f64; 8] = [1., 2., 5., 10., 20., 50., 200., 300.];
/// Retail entries in the imported separation list.
pub const RETAIL_SEPARATIONS: usize = 6;

/// `fitted`, agent decision 2026-09-23: keep every enemy aircraft at least one
/// terrain cell (8192 ft) inside the map edge, so none starts over the
/// repeated edge terrain beyond it.
pub const MAP_MARGIN_CELLS: f64 = 1.;
/// `fitted`, agent decision: bearings are searched in one-degree steps.
pub const AIM_STEP_DEG: i32 = 1;

/// `fitted`, agent decision 2026-09-23: runway parking for the player's wing.
/// Each aircraft stands this far behind the one ahead of it, trying the
/// larger spacing first.
pub const GROUND_SPACING_FT: [f64; 4] = [250., 200., 150., 100.];
/// Wingmen stand this far right (odd) or left (even) of the centerline. With
/// the widest selectable wingspan, the F-14's 64 ft spread, the wingtips stay
/// within 72 ft of the centerline, and alternate sides keep neighbours 80 ft
/// apart laterally as well as a full spacing apart along the runway.
pub const GROUND_LATERAL_FT: f64 = 40.;
/// Requested taxiway queue; fitted spacing and hold distance, in feet.
pub const TAXI_QUEUE_SPACING_FT: f64 = 200.;
pub const TAXI_QUEUE_STEP_FT: f64 = 50.;
pub const TAXI_QUEUE_HOLD_FT: f64 = 250.;

/// The part of the map an aircraft may start over, feet. X runs east and Z
/// north, as in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapBounds {
    pub min: [f64; 2],
    pub max: [f64; 2],
}

impl MapBounds {
    /// Terrain runs from 0 to `(cells - 1) * cell_ft` on each axis; the
    /// bounds are that rectangle shrunk by `margin_ft` on every side.
    pub fn from_cells(cols: usize, rows: usize, cell_ft: f64, margin_ft: f64) -> Self {
        let extent = |cells: usize| cells.saturating_sub(1) as f64 * cell_ft;
        Self {
            min: [margin_ft, margin_ft],
            max: [extent(cols) - margin_ft, extent(rows) - margin_ft],
        }
    }

    #[cfg(test)]
    pub fn contains(&self, x: f64, z: f64) -> bool {
        (self.min[0]..=self.max[0]).contains(&x) && (self.min[1]..=self.max[1]).contains(&z)
    }
}

/// Where the enemy group is placed relative to the friendly reference point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyAim {
    /// Clockwise rotation from the reference heading, radians.
    pub turn: f64,
    /// Distance actually used, feet.
    pub distance_ft: f64,
    /// Distance the player chose, feet.
    pub requested_ft: f64,
}

impl EnemyAim {
    pub fn straight(distance_ft: f64) -> Self {
        Self {
            turn: 0.,
            distance_ft,
            requested_ft: distance_ft,
        }
    }

    /// True when the chosen distance did not fit and a shorter one is used.
    pub fn shortened(&self) -> bool {
        self.distance_ft + 1. < self.requested_ft
    }
}

/// The farthest distance, up to `limit`, at which every point of the group
/// stays inside `bounds` along `bearing`. None if no distance in `0..=limit`
/// works. `group` holds each aircraft's [right, forward] offset from the
/// group's placement point, in a frame whose forward axis is the bearing.
fn reach(
    reference: [f64; 2],
    bearing: f64,
    group: &[[f64; 2]],
    bounds: MapBounds,
    limit: f64,
) -> Option<f64> {
    let (sin, cos) = bearing.sin_cos();
    let forward = [sin, cos];
    let right = [cos, -sin];
    let (mut low, mut high) = (0f64, limit);
    for [r, f] in group {
        for axis in 0..2 {
            // Position along this axis is `start + slope * distance`.
            let start = reference[axis] + right[axis] * r + forward[axis] * f;
            let slope = forward[axis];
            let (min, max) = (bounds.min[axis], bounds.max[axis]);
            if slope.abs() < 1e-12 {
                if start < min || start > max {
                    return None;
                }
                continue;
            }
            let (a, b) = ((min - start) / slope, (max - start) / slope);
            low = low.max(a.min(b));
            high = high.min(a.max(b));
        }
    }
    (low <= high).then_some(high)
}

/// Aim the enemy group into the map (John's request, 2026-09-23).
///
/// Straight ahead is kept when the whole group fits there at the chosen
/// distance. Otherwise the bearing closest to `heading` that fits is used,
/// searching one degree at a time, clockwise before counter-clockwise at equal
/// angles. If no bearing fits at the full distance, the bearing that allows the
/// longest distance is used at that distance (closest to `heading` on a tie).
/// `fitted`: the search order and the tie rules are agent choices.
pub fn aim_into_map(
    reference: [f64; 2],
    heading: f64,
    distance_ft: f64,
    group: &[[f64; 2]],
    bounds: MapBounds,
) -> EnemyAim {
    if group.is_empty() || !distance_ft.is_finite() || distance_ft <= 0. {
        return EnemyAim::straight(distance_ft);
    }
    let half_turn = 180 / AIM_STEP_DEG;
    let turns = (0..=half_turn).flat_map(|step| {
        let degrees = step * AIM_STEP_DEG;
        let both = step != 0 && step != half_turn;
        std::iter::once(degrees).chain(both.then_some(-degrees))
    });
    let mut best: Option<(f64, f64)> = None;
    for degrees in turns {
        let turn = f64::from(degrees).to_radians();
        let Some(reach) = reach(reference, heading + turn, group, bounds, distance_ft) else {
            continue;
        };
        if reach >= distance_ft - 1e-6 {
            return EnemyAim {
                turn,
                distance_ft,
                requested_ft: distance_ft,
            };
        }
        if best.is_none_or(|(_, farthest)| reach > farthest + 1e-6) {
            best = Some((turn, reach));
        }
    }
    // A reference point outside the usable map has nothing to aim at; keep
    // the chosen geometry rather than invent one.
    best.map_or(EnemyAim::straight(distance_ft), |(turn, reach)| EnemyAim {
        turn,
        distance_ft: reach,
        requested_ft: distance_ft,
    })
}

/// Each parked aircraft's [right, forward] offset from the leader's slot,
/// leader first: wingman 1 right, 2 left, 3 right, 4 left, each one `spacing`
/// farther back than the previous.
pub fn ground_slot_offsets(count: usize, spacing_ft: f64) -> Vec<[f64; 2]> {
    (0..count)
        .map(|order| {
            let side = match order {
                0 => 0.,
                n if n % 2 == 1 => 1.,
                _ => -1.,
            };
            [side * GROUND_LATERAL_FT, -(order as f64) * spacing_ft]
        })
        .collect()
}

/// Parking points on `runway`, leader first, at runway elevation. The last
/// aircraft stands at the existing single-aircraft start point (5% of the
/// runway length in from the near threshold, capped at 100 ft) so the leader
/// is `(count - 1) * spacing` farther down the runway.
pub fn runway_slots(runway: &Runway, count: usize, spacing_ft: f64) -> Vec<[f64; 3]> {
    let (last, heading) = runway.departure_pose();
    let forward = [heading.sin(), heading.cos()];
    let right = [heading.cos(), -heading.sin()];
    let lead = count.saturating_sub(1) as f64 * spacing_ft;
    ground_slot_offsets(count, spacing_ft)
        .into_iter()
        .map(|[r, f]| {
            let along = lead + f;
            [
                last[0] + forward[0] * along + right[0] * r,
                last[1],
                last[2] + forward[1] * along + right[1] * r,
            ]
        })
        .collect()
}

/// Choose the widest spacing at which every slot passes `check`, which
/// returns the slot resting on the surface or the reason it cannot be used.
/// Returns the spacing and the checked slots, or the first reason the
/// tightest spacing still failed.
pub fn fit_runway_slots(
    runway: &Runway,
    count: usize,
    mut check: impl FnMut([f64; 3]) -> Result<[f64; 3], String>,
) -> Result<(f64, Vec<[f64; 3]>), String> {
    let spacings: &[f64] = if count <= 1 {
        &GROUND_SPACING_FT[..1]
    } else {
        &GROUND_SPACING_FT
    };
    let mut reason = String::new();
    for &spacing in spacings {
        match runway_slots(runway, count, spacing)
            .into_iter()
            .map(&mut check)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(slots) => return Ok((spacing, slots)),
            Err(why) => reason = why,
        }
    }
    Err(reason)
}

/// The heading the wing departs on.
pub fn departure_heading(runway: &Runway) -> f64 {
    runway.approach_heading(ApproachEnd::Near)
}

/// Player on the reviewed takeoff spot; wingmen queued on the taxi-out path
/// (John, 2026-09-23). Positions are measured backwards from the last taxi
/// anchor, facing along that segment toward the runway. Blocked slots are
/// skipped and a queue that cannot fit falls back to the runway layout.
pub fn anchored_slots(
    anchors: &AirfieldAnchors,
    count: usize,
    mut check: impl FnMut([f64; 3]) -> Result<[f64; 3], String>,
) -> Option<Vec<([f64; 3], f64)>> {
    let mut slots = vec![(check(anchors.takeoff_spot).ok()?, anchors.takeoff_heading)];
    let segments: Vec<_> = (1..4)
        .rev()
        .map(|leg| {
            let end = anchors.taxi_out[leg];
            let start = anchors.taxi_out[leg - 1];
            let dx = end[0] - start[0];
            let dz = end[2] - start[2];
            (end, dx, dz, dx.hypot(dz))
        })
        .collect();
    let length: f64 = segments.iter().map(|s| s.3).sum();
    let mut offset = 0.;
    while slots.len() < count {
        let mut found = None;
        while offset <= length {
            let mut remaining = offset;
            for &(end, dx, dz, len) in &segments {
                if len > 0. && remaining <= len {
                    let p = [
                        end[0] - dx * remaining / len,
                        end[1],
                        end[2] - dz * remaining / len,
                    ];
                    let from_player =
                        (p[0] - anchors.takeoff_spot[0]).hypot(p[2] - anchors.takeoff_spot[2]);
                    if from_player >= TAXI_QUEUE_HOLD_FT
                        && slots.iter().all(|(other, _)| {
                            (other[0] - p[0]).hypot(other[2] - p[2]) >= TAXI_QUEUE_SPACING_FT
                        })
                        && let Ok(p) = check(p)
                    {
                        found = Some((p, dx.atan2(dz)));
                    }
                    break;
                }
                remaining -= len;
            }
            if found.is_some() {
                break;
            }
            offset += TAXI_QUEUE_STEP_FT;
        }
        slots.push(found?);
        offset += TAXI_QUEUE_SPACING_FT;
    }
    Some(slots)
}

/// Every slot as a [right, forward] offset from the leader's slot, in the
/// leader's frame.
pub fn relative_offsets(slots: &[[f64; 3]], heading: f64) -> Vec<[f64; 2]> {
    let Some(lead) = slots.first() else {
        return Vec::new();
    };
    let (sin, cos) = heading.sin_cos();
    slots
        .iter()
        .map(|p| {
            let d = [p[0] - lead[0], p[2] - lead[2]];
            [d[0] * cos - d[1] * sin, d[0] * sin + d[1] * cos]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::airport::OrientedBox;

    const CELL: f64 = 8192.;

    fn bounds() -> MapBounds {
        // A 208 by 200 cell theater with the one-cell margin.
        MapBounds::from_cells(208, 200, CELL, CELL)
    }

    fn runway(heading: f64, length_ft: f64) -> Runway {
        let center = [100_000., 50., 200_000.];
        Runway {
            object: 7,
            airport: 3,
            name: "Test".into(),
            surface: OrientedBox {
                center,
                half: [75., 5., length_ft / 2.],
                heading,
                pitch: 0.,
                bank: 0.,
            },
            approach_center: center,
            elevation_ft: 50.,
            heading,
            length_ft,
        }
    }

    #[test]
    fn separation_table_is_nautical_and_adds_200_and_300() {
        assert_eq!(SEPARATION_NM, [1., 2., 5., 10., 20., 50., 200., 300.]);
        assert_eq!(FEET_PER_NM, 6_076.12);
        assert_eq!(RETAIL_SEPARATIONS, 6);
    }

    #[test]
    fn bounds_follow_the_terrain_extent_and_margin() {
        let b = bounds();
        assert_eq!(b.min, [CELL, CELL]);
        assert_eq!(b.max, [206. * CELL, 198. * CELL]);
        assert!(b.contains(CELL, 198. * CELL));
        assert!(!b.contains(CELL - 1., CELL));
    }

    #[test]
    fn a_group_that_fits_keeps_the_forward_bearing() {
        let center = [104. * CELL, 100. * CELL];
        let group = [[0., 0.], [-4096., 0.], [4096., 0.], [512., 512.]];
        let aim = aim_into_map(center, 0.3, 50. * FEET_PER_NM, &group, bounds());
        assert_eq!(aim, EnemyAim::straight(50. * FEET_PER_NM));
        assert!(!aim.shortened());
    }

    #[test]
    fn a_group_off_the_map_turns_to_the_nearest_bearing_that_fits() {
        // 20 nm from the north edge, facing north, with 50 nm chosen.
        let b = bounds();
        let reference = [104. * CELL, b.max[1] - 20. * FEET_PER_NM];
        let group = [[0., 0.], [-4096., 0.], [4096., 0.]];
        let distance = 50. * FEET_PER_NM;
        let aim = aim_into_map(reference, 0., distance, &group, b);
        assert_eq!(aim.distance_ft, distance);
        assert!(!aim.shortened());
        let turn = aim.turn.to_degrees();
        // The clockwise side is preferred at equal angles.
        assert!(turn > 60. && turn < 90., "turned {turn} degrees");
        // Every aircraft is on the map at the full distance...
        let bearing = aim.turn;
        let (sin, cos) = bearing.sin_cos();
        for [r, f] in group {
            let x = reference[0] + cos * r + sin * (distance + f);
            let z = reference[1] - sin * r + cos * (distance + f);
            assert!(b.contains(x, z), "{x} {z}");
        }
        // ...and one degree less would put some of it off the map.
        assert!(
            reach(reference, bearing - 1f64.to_radians(), &group, b, distance).unwrap() < distance
        );
        // Mirror image: facing south next to the south edge, the same turn
        // but counter-clockwise from south would be equally close; the
        // clockwise one is chosen.
        let mirrored = [104. * CELL, b.min[1] + 20. * FEET_PER_NM];
        let aim = aim_into_map(mirrored, std::f64::consts::PI, distance, &group, b);
        assert!(aim.turn > 0.);
    }

    #[test]
    fn a_distance_longer_than_the_map_is_shortened_along_the_longest_line() {
        let b = bounds();
        let center = [104. * CELL, 100. * CELL];
        let group = [[0., 0.]];
        let aim = aim_into_map(center, 0., 300. * FEET_PER_NM, &group, b);
        assert!(aim.shortened());
        assert_eq!(aim.requested_ft, 300. * FEET_PER_NM);
        // The centre sits nearer the north-east corner, so the longest line
        // runs south-west, toward bearing 226 degrees (a turn of -134).
        let corner = (center[0] - b.min[0]).hypot(center[1] - b.min[1]);
        assert!(aim.distance_ft <= corner && aim.distance_ft > corner * 0.995);
        assert_eq!(aim.turn.to_degrees().round(), -134.);
        // No bearing, however long the search, reaches the chosen distance.
        for step in -180..=180 {
            let r = reach(
                center,
                f64::from(step).to_radians(),
                &group,
                b,
                300. * FEET_PER_NM,
            );
            assert!(r.unwrap() <= aim.distance_ft + 1e-6);
        }
    }

    #[test]
    fn a_reference_outside_the_usable_map_keeps_the_chosen_geometry() {
        let far = [-10. * CELL, -10. * CELL];
        let aim = aim_into_map(far, 0., 5. * FEET_PER_NM, &[[0., 0.]], bounds());
        assert_eq!(aim, EnemyAim::straight(5. * FEET_PER_NM));
        assert_eq!(
            aim_into_map(far, 0., 5., &[], bounds()),
            EnemyAim::straight(5.)
        );
    }

    #[test]
    fn ground_slots_stagger_behind_the_leader_and_keep_the_threshold_inset() {
        assert_eq!(
            ground_slot_offsets(5, 250.),
            [
                [0., 0.],
                [40., -250.],
                [-40., -500.],
                [40., -750.],
                [-40., -1000.]
            ]
        );
        for heading in [0., 1.1, 4.0] {
            let r = runway(heading, 8000.);
            let slots = runway_slots(&r, 5, 250.);
            let near = r.threshold(ApproachEnd::Near);
            let forward = [heading.sin(), heading.cos()];
            let right = [heading.cos(), -heading.sin()];
            let local = |p: [f64; 3]| {
                let d = [p[0] - near[0], p[2] - near[2]];
                (
                    d[0] * right[0] + d[1] * right[1],
                    d[0] * forward[0] + d[1] * forward[1],
                )
            };
            // The last aircraft stands at the existing 100 ft inset; the
            // leader, the player, is 1000 ft farther down the runway.
            let expected = [
                (0., 1100.),
                (40., 850.),
                (-40., 600.),
                (40., 350.),
                (-40., 100.),
            ];
            for (slot, (want_right, want_along)) in slots.iter().zip(expected) {
                let (got_right, got_along) = local(*slot);
                assert!((got_right - want_right).abs() < 1e-6);
                assert!((got_along - want_along).abs() < 1e-6);
                assert_eq!(slot[1], 50.);
            }
        }
        // A single aircraft starts exactly where the player always has.
        let r = runway(0.5, 8000.);
        assert_eq!(runway_slots(&r, 1, 250.)[0], r.departure_pose().0);
        // Short runways use the 5% inset.
        let short = runway(0., 1000.);
        let slots = runway_slots(&short, 2, 250.);
        let near = short.threshold(ApproachEnd::Near);
        assert!((slots[1][2] - near[2] - 50.).abs() < 1e-9);
    }

    fn anchors() -> AirfieldAnchors {
        let at = |x: f64, z: f64| [x, 50., z];
        AirfieldAnchors {
            taxi_out: [at(900., 0.), at(900., 100.), at(0., 100.), at(0., -200.)],
            takeoff_spot: at(0., 0.),
            takeoff_heading: 0.,
            landing_point: at(0., -160.),
            landing_heading: 0.,
            taxi_in: [
                at(0., 3000.),
                at(300., 3000.),
                at(300., 100.),
                at(900., 100.),
            ],
            parking: std::array::from_fn(|k| at(1000., 200. * k as f64)),
            parking_heading: std::f64::consts::FRAC_PI_2,
        }
    }

    #[test]
    fn anchored_queue_faces_the_runway_and_skips_obstacles() {
        let a = anchors();
        let open = |p: [f64; 3]| Ok([p[0], 49., p[2]]);
        let slots = anchored_slots(&a, 5, open).unwrap();
        assert_eq!(slots[0], ([0., 49., 0.], 0.));
        assert_eq!(slots[1].0, [250., 49., 100.]);
        for pair in slots[1..].windows(2) {
            assert!(
                (pair[0].0[0] - pair[1].0[0]).hypot(pair[0].0[2] - pair[1].0[2])
                    >= TAXI_QUEUE_SPACING_FT
            );
        }
        assert_eq!(slots[1].1, -std::f64::consts::FRAC_PI_2);
        let blocked = |p: [f64; 3]| {
            if (200. ..=300.).contains(&p[0]) {
                Err("blocked".into())
            } else {
                Ok(p)
            }
        };
        assert_eq!(
            anchored_slots(&a, 3, blocked).unwrap()[1].0,
            [350., 50., 100.]
        );
        assert!(anchored_slots(&a, 2, |_| Err("blocked".into())).is_none());
        assert!(anchored_slots(&a, 20, open).is_none());
        assert_eq!(
            relative_offsets(&slots.iter().map(|s| s.0).collect::<Vec<_>>(), 0.)[1],
            [250., 100.]
        );
    }

    #[test]
    fn blocked_slots_tighten_the_spacing_then_reject_the_start() {
        let r = runway(0., 8000.);
        let near = r.threshold(ApproachEnd::Near)[2];
        // An obstruction 1000 to 1200 ft past the threshold blocks the
        // 250 ft layout's leader (1100 ft) but not the 200 ft layout (900 ft).
        let blocked = |p: [f64; 3]| {
            if (1000.0..=1200.).contains(&(p[2] - near)) {
                Err("The runway start is obstructed.".to_string())
            } else {
                Ok([p[0], 49., p[2]])
            }
        };
        let (spacing, slots) = fit_runway_slots(&r, 5, blocked).unwrap();
        assert_eq!(spacing, 200.);
        assert_eq!(slots.len(), 5);
        assert!(slots.iter().all(|s| s[1] == 49.));
        assert!((slots[0][2] - near - 900.).abs() < 1e-6);
        // Blocking the inset point itself leaves no spacing to fall back to.
        let always = |p: [f64; 3]| {
            if p[2] - near < 150. {
                Err("The runway start is obstructed.".to_string())
            } else {
                Ok(p)
            }
        };
        assert_eq!(
            fit_runway_slots(&r, 5, always).unwrap_err(),
            "The runway start is obstructed."
        );
        // One aircraft is never retried at another spacing.
        let mut calls = 0;
        let _ = fit_runway_slots(&r, 1, |_| {
            calls += 1;
            Err("no".to_string())
        });
        assert_eq!(calls, 1);
    }
}
