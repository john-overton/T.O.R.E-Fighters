//! Bounded hidden substrates under airport footprints and foundation faces. The runway
//! and flight support plane stay fixed. See docs/spec/airports.md.
use tore_sim::airport::Runway;

type Vertex = [f32; 10];
/// Fitted separation below the airport plane. No raised pavement or aircraft.
const RECESS_FT: f64 = 1.;

fn local(runway: &Runway, v: &Vertex) -> [f64; 2] {
    let (sin, cos) = runway.surface.heading.sin_cos();
    let x = f64::from(v[0]) - runway.surface.center[0];
    let z = f64::from(v[2]) - runway.surface.center[2];
    [x * cos - z * sin, x * sin + z * cos]
}
fn edge_distance(runway: &Runway, v: &Vertex, edge: usize) -> f64 {
    let p = local(runway, v);
    let axis = edge / 2;
    p[axis] * if edge.is_multiple_of(2) { 1. } else { -1. } - runway.surface.half[axis * 2]
}
fn clip(polygon: &[Vertex], runway: &Runway, edge: usize, inside: bool) -> Vec<Vertex> {
    clip_by(polygon, |v| edge_distance(runway, v, edge), inside)
}
fn clip_by(polygon: &[Vertex], distance: impl Fn(&Vertex) -> f64, inside: bool) -> Vec<Vertex> {
    let mut out = Vec::new();
    let Some(mut previous) = polygon.last() else {
        return out;
    };
    let mut before = distance(previous);
    for vertex in polygon {
        let after = distance(vertex);
        let keep_before = (before <= 0.) == inside;
        let keep_after = (after <= 0.) == inside;
        if keep_before != keep_after {
            let t = (before / (before - after)).clamp(0., 1.);
            out.push(std::array::from_fn(|i| {
                (f64::from(previous[i]) + (f64::from(vertex[i]) - f64::from(previous[i])) * t)
                    as f32
            }));
        }
        if keep_after {
            out.push(*vertex);
        }
        previous = vertex;
        before = after;
    }
    out
}
fn triangulate(out: &mut Vec<f32>, polygon: &[Vertex]) {
    for i in 1..polygon.len().saturating_sub(1) {
        out.extend(polygon[0]);
        out.extend(polygon[i]);
        out.extend(polygon[i + 1]);
    }
}
fn recess(out: &mut Vec<f32>, original: &[Vertex], runway: &Runway) {
    let lowered: Vec<Vertex> = original
        .iter()
        .map(|v| {
            let mut p = *v;
            let plane = runway
                .support_height(f64::from(v[0]), f64::from(v[2]))
                .expect("validated plane");
            p[1] = p[1].min((plane - RECESS_FT) as f32);
            p
        })
        .collect();
    triangulate(out, &lowered);
    // Close the boundary against higher outside terrain. Internal triangle
    // edges do not get walls, so source texture coordinates remain continuous.
    for i in 0..original.len() {
        let j = (i + 1) % original.len();
        if original[i][1] == lowered[i][1] && original[j][1] == lowered[j][1] {
            continue;
        }
        if (0..4).any(|edge| {
            edge_distance(runway, &original[i], edge).abs() <= 0.25
                && edge_distance(runway, &original[j], edge).abs() <= 0.25
        }) {
            triangulate(out, &[original[i], original[j], lowered[j], lowered[i]]);
        }
    }
}

pub(super) fn terrain(vertices: &[f32], runways: &[Runway]) -> Vec<f32> {
    let bounds: Vec<_> = runways
        .iter()
        .filter_map(|r| {
            r.support_height(r.surface.center[0], r.surface.center[2])?;
            let (sin, cos) = r.surface.heading.sin_cos();
            let x = cos.abs() * r.surface.half[0] + sin.abs() * r.surface.half[2];
            let z = sin.abs() * r.surface.half[0] + cos.abs() * r.surface.half[2];
            Some((
                r,
                [
                    r.surface.center[0] - x,
                    r.surface.center[2] - z,
                    r.surface.center[0] + x,
                    r.surface.center[2] + z,
                ],
            ))
        })
        .collect();
    let mut out = Vec::with_capacity(vertices.len());
    for triangle in vertices.chunks_exact(30) {
        let t: [Vertex; 3] =
            std::array::from_fn(|i| triangle[i * 10..i * 10 + 10].try_into().unwrap());
        let x0 = t
            .iter()
            .map(|v| f64::from(v[0]))
            .fold(f64::INFINITY, f64::min);
        let z0 = t
            .iter()
            .map(|v| f64::from(v[2]))
            .fold(f64::INFINITY, f64::min);
        let x1 = t
            .iter()
            .map(|v| f64::from(v[0]))
            .fold(f64::NEG_INFINITY, f64::max);
        let z1 = t
            .iter()
            .map(|v| f64::from(v[2]))
            .fold(f64::NEG_INFINITY, f64::max);
        let overlaps = |b: &[f64; 4]| x1 > b[0] && z1 > b[1] && x0 < b[2] && z0 < b[3];
        if !bounds.iter().any(|(_, b)| overlaps(b)) {
            out.extend_from_slice(triangle);
            continue;
        }
        let mut remaining = vec![t.to_vec()];
        for (runway, _) in bounds.iter().filter(|(_, b)| overlaps(b)) {
            let mut next = Vec::new();
            for mut polygon in remaining {
                for edge in 0..4 {
                    let outside = clip(&polygon, runway, edge, false);
                    if outside.len() >= 3 {
                        next.push(outside);
                    }
                    polygon = clip(&polygon, runway, edge, true);
                    if polygon.len() < 3 {
                        break;
                    }
                }
                if polygon.len() >= 3 {
                    recess(&mut out, &polygon, runway);
                }
            }
            remaining = next;
        }
        for polygon in remaining {
            triangulate(&mut out, &polygon);
        }
    }
    out
}

/// Recess only actual bottom-face coverage, including nonrectangular bases of
/// compound objects. The shared foundation already supplies physical support.
/// Holes between faces and bare ground beside a rotated shape stay untouched.
pub(super) fn pads(vertices: &[f32], covers: &[[[f32; 3]; 3]]) -> Vec<f32> {
    use std::collections::BTreeMap;
    const CELL: f32 = tore_formats::theater::CELL_FEET;
    let mut index: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    for (id, t) in covers.iter().enumerate() {
        let lo = [0, 2].map(|a| t.iter().map(|p| p[a]).fold(f32::INFINITY, f32::min));
        let hi = [0, 2].map(|a| t.iter().map(|p| p[a]).fold(f32::NEG_INFINITY, f32::max));
        for z in (lo[1] / CELL).floor() as i32..=(hi[1] / CELL).floor() as i32 {
            for x in (lo[0] / CELL).floor() as i32..=(hi[0] / CELL).floor() as i32 {
                index.entry((x, z)).or_default().push(id);
            }
        }
    }
    let mut out = Vec::with_capacity(vertices.len());
    for triangle in vertices.chunks_exact(30) {
        let t: [Vertex; 3] =
            std::array::from_fn(|i| triangle[i * 10..i * 10 + 10].try_into().unwrap());
        let key = (
            (t.iter().map(|v| f64::from(v[0])).sum::<f64>() / (3. * f64::from(CELL))).floor()
                as i32,
            (t.iter().map(|v| f64::from(v[2])).sum::<f64>() / (3. * f64::from(CELL))).floor()
                as i32,
        );
        let Some(ids) = index.get(&key) else {
            out.extend_from_slice(triangle);
            continue;
        };
        let low = [0, 2].map(|a| t.iter().map(|p| p[a]).fold(f32::INFINITY, f32::min));
        let high = [0, 2].map(|a| t.iter().map(|p| p[a]).fold(f32::NEG_INFINITY, f32::max));
        let mut remaining = vec![t.to_vec()];
        for id in ids {
            let cover = covers[*id];
            let cover_low =
                [0, 2].map(|a| cover.iter().map(|p| p[a]).fold(f32::INFINITY, f32::min));
            let cover_high =
                [0, 2].map(|a| cover.iter().map(|p| p[a]).fold(f32::NEG_INFINITY, f32::max));
            if (0..2).any(|a| high[a] <= cover_low[a] || low[a] >= cover_high[a]) {
                continue;
            }
            let cross = |a: [f32; 3], b: [f32; 3], x: f32, z: f32| {
                (f64::from(b[0]) - f64::from(a[0])) * (f64::from(z) - f64::from(a[2]))
                    - (f64::from(b[2]) - f64::from(a[2])) * (f64::from(x) - f64::from(a[0]))
            };
            let area = cross(cover[0], cover[1], cover[2][0], cover[2][2]);
            if area.abs() < 0.001 {
                continue;
            }
            let distance = |v: &Vertex, edge: usize| {
                let a = cover[edge];
                let b = cover[(edge + 1) % 3];
                let length =
                    (f64::from(b[0]) - f64::from(a[0])).hypot(f64::from(b[2]) - f64::from(a[2]));
                -area.signum() * cross(a, b, v[0], v[2]) / length
            };
            let mut next = Vec::new();
            for mut polygon in remaining {
                for edge in 0..3 {
                    let outside = clip_by(&polygon, |v| distance(v, edge), false);
                    if outside.len() >= 3 {
                        next.push(outside);
                    }
                    polygon = clip_by(&polygon, |v| distance(v, edge), true);
                    if polygon.len() < 3 {
                        break;
                    }
                }
                if polygon.len() >= 3 {
                    let lowered: Vec<Vertex> = polygon
                        .iter()
                        .map(|v| {
                            let mut p = *v;
                            p[1] = p[1].min(cover[0][1] - RECESS_FT as f32);
                            p
                        })
                        .collect();
                    triangulate(&mut out, &lowered);
                    for i in 0..polygon.len() {
                        let j = (i + 1) % polygon.len();
                        if (0..3).any(|e| {
                            distance(&polygon[i], e).abs() < 0.25
                                && distance(&polygon[j], e).abs() < 0.25
                        }) {
                            triangulate(
                                &mut out,
                                &[polygon[i], polygon[j], lowered[j], lowered[i]],
                            );
                        }
                    }
                }
            }
            remaining = next;
        }
        for polygon in remaining {
            triangulate(&mut out, &polygon);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::airport::OrientedBox;
    fn runway(heading: f64, pitch: f64) -> Runway {
        Runway {
            object: 1,
            airport: 1,
            name: "Synthetic".into(),
            surface: OrientedBox {
                center: [0.; 3],
                half: [10., 1., 20.],
                heading,
                pitch,
                bank: 0.,
            },
            approach_center: [0.; 3],
            elevation_ft: 0.,
            heading,
            length_ft: 40.,
        }
    }
    fn mesh(height: f32) -> Vec<f32> {
        [
            (-100., -100.),
            (-100., 100.),
            (100., -100.),
            (100., -100.),
            (-100., 100.),
            (100., 100.),
        ]
        .into_iter()
        .flat_map(|(x, z)| [x, height, z, x / 200., z / 200., 3., 0., 0., 0., 50.])
        .collect()
    }
    fn area(vertices: &[f32]) -> f64 {
        vertices
            .chunks_exact(30)
            .map(|t| {
                f64::from(((t[10] - t[0]) * (t[22] - t[2]) - (t[20] - t[0]) * (t[12] - t[2])).abs())
                    / 2.
            })
            .sum()
    }
    #[test]
    fn foundation_substrate_follows_triangular_faces_and_preserves_exposed_ground() {
        let mut source = mesh(0.);
        for v in source.chunks_exact_mut(10) {
            v[0] += 512.;
            v[2] += 512.;
        }
        let covers = [[[472., 0., 472.], [552., 0., 472.], [472., 0., 552.]]];
        let result = pads(&source, &covers);
        assert!((area(&result) - 40000.).abs() < 0.1);
        let mut recessed = 0.;
        for t in result.chunks_exact(30) {
            let a = area(t);
            if a < 0.001 {
                continue;
            }
            let x = (t[0] + t[10] + t[20]) / 3.;
            let z = (t[2] + t[12] + t[22]) / 3.;
            let inside = x >= 472. && z >= 472. && x + z <= 1024.;
            for v in t.chunks_exact(10) {
                assert_eq!(v[1], if inside { -1. } else { 0. });
                assert!((v[3] - (v[0] - 512.) / 200.).abs() < 1e-6);
                assert!((v[4] - (v[2] - 512.) / 200.).abs() < 1e-6);
            }
            if inside {
                recessed += a;
            }
        }
        assert!(
            (recessed - 3200.).abs() < 0.1,
            "the rectangle outside the triangular pad stays at ground height"
        );
        assert_eq!(pads(&source, &[]), source);
    }

    #[test]
    fn rotated_footprints_recess_only_the_inside_and_preserve_uv_and_area() {
        for angle in [0., 0.4, std::f64::consts::FRAC_PI_2] {
            let r = runway(angle, 0.08);
            let source = mesh(5.);
            let out = terrain(&source, std::slice::from_ref(&r));
            assert!((area(&out) - area(&source)).abs() < 0.1);
            let mut inside_area = 0.;
            let mut walls = 0;
            for triangle in out.chunks_exact(30) {
                for v in triangle.chunks_exact(10) {
                    assert!((v[3] - v[0] / 200.).abs() < 1e-6 && (v[4] - v[2] / 200.).abs() < 1e-6);
                    assert_eq!((v[5], v[9]), (3., 50.));
                }
                if area(triangle) < 0.001 {
                    walls += 1;
                    continue;
                }
                let x = (triangle[0] + triangle[10] + triangle[20]) / 3.;
                let z = (triangle[2] + triangle[12] + triangle[22]) / 3.;
                if r.surface.contains_horizontal(f64::from(x), f64::from(z)) {
                    inside_area += area(triangle);
                    for v in triangle.chunks_exact(10) {
                        assert!(
                            f64::from(v[1])
                                <= r.support_height(f64::from(v[0]), f64::from(v[2])).unwrap()
                                    - 0.99
                        );
                    }
                } else {
                    assert!(triangle.chunks_exact(10).all(|v| v[1] == 5.));
                }
            }
            assert!((inside_area - 800.).abs() < 0.01, "{inside_area}");
            assert!(walls > 0);
            assert_eq!(r.approach_center, [0.; 3]);
        }
    }
    #[test]
    fn empty_or_distant_airports_leave_the_mesh_identical_and_low_ground_is_not_raised() {
        let source = mesh(5.);
        assert_eq!(terrain(&source, &[]), source);
        let mut far = runway(0., 0.);
        far.surface.center[0] = 1000.;
        far.approach_center[0] = 1000.;
        assert_eq!(terrain(&source, &[far]), source);
        assert!(
            terrain(&mesh(-5.), &[runway(0., 0.)])
                .chunks_exact(10)
                .all(|v| v[1] == -5.)
        );
    }
}
