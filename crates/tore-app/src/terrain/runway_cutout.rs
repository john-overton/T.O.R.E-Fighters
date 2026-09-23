//! Render-only, bounded terrain recesses under airport footprints. The runway
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
    let mut out = Vec::new();
    let Some(mut previous) = polygon.last() else {
        return out;
    };
    let mut before = edge_distance(runway, previous, edge);
    for vertex in polygon {
        let after = edge_distance(runway, vertex, edge);
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
