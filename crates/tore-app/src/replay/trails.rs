//! Flight path trails: each aircraft's and guided weapon's recent path as a
//! thin line in its side's colour, drawn in the 3D view so terrain and
//! aircraft hide it. A trail is the path samples from the last few seconds
//! plus where the aircraft is drawn now, so it grows and shrinks with the
//! playhead in either direction. Included suggestion approved by John on
//! 2026-09-26; lengths, colours and width are agent choices.
use crate::camera::Camera;
use crate::replay::tracks::SAMPLE_TICKS;
use tore_replay::Side;

/// Trail lengths Shift+R steps through, in seconds.
pub const LENGTHS: [u64; 5] = [10, 30, 60, 120, 300];
/// The length a viewer starts with: 30 seconds.
pub const DEFAULT_LENGTH: usize = 1;
/// Most samples one trail draws; a longer one keeps every second, third
/// and so on, always the same ticks, so it never shimmers as it slides.
const MAX_POINTS: u64 = 240;
/// On-screen width, in pixels of a 480-line view.
const WIDTH: f64 = 1.5;

/// A side's colour: friendly blue, enemy red, neutral green, as the Tacview
/// export colours them, and grey when unknown.
pub fn side_color(side: Side) -> [u8; 3] {
    match side {
        Side::Friendly => [110, 170, 255],
        Side::Enemy => [255, 100, 90],
        Side::Neutral => [120, 230, 120],
        Side::Unknown => [220, 220, 220],
    }
}

/// A weapon's trail: its owner's colour, paler.
pub fn weapon_color(side: Side) -> [u8; 3] {
    side_color(side).map(|c| ((u16::from(c) + 255) / 2) as u8)
}

/// The points of a trail at `tick`: the samples of the last `seconds`
/// before it, then `head`, where the aircraft or weapon is drawn now.
pub fn path(samples: &[(u64, [f32; 3])], tick: u64, seconds: u64, head: [f64; 3]) -> Vec<[f64; 3]> {
    let span = seconds * 120;
    let from = tick.saturating_sub(span);
    let start = samples.partition_point(|(t, _)| *t < from);
    let end = samples.partition_point(|(t, _)| *t < tick);
    let grid = SAMPLE_TICKS * (span / SAMPLE_TICKS).div_ceil(MAX_POINTS).max(1);
    let mut points: Vec<[f64; 3]> = samples[start..end.max(start)]
        .iter()
        .enumerate()
        .filter(|(i, (t, _))| *i == 0 || t.is_multiple_of(grid))
        .map(|(_, (_, p))| p.map(f64::from))
        .collect();
    points.push(head);
    points
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

fn length(v: [f64; 3]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// A world position moved to be relative to the render `origin`.
pub fn relative(point: [f64; 3], origin: [f64; 3]) -> [f64; 3] {
    sub(point, origin)
}

/// `camera` with its position relative to the render `origin`: the eye that
/// matches combat geometry, which is built relative to the origin so its
/// vertices keep their precision far from the map's corner.
pub fn relative_camera(camera: &Camera, origin: [f64; 3]) -> Camera {
    let mut moved = Camera::new();
    moved.position = relative(camera.position, origin);
    [moved.yaw, moved.pitch, moved.roll] = [camera.yaw, camera.pitch, camera.roll];
    moved.zoom = camera.zoom;
    moved
}

/// [`ribbon`] for world positions: the points and the eye are moved by the
/// render `origin` first, because combat geometry is relative to it. Drawn
/// without that, a trail lands the origin's distance (up to several miles
/// along each axis) from the aircraft it follows.
pub fn ribbon_world(
    out: &mut Vec<f32>,
    points: &[[f64; 3]],
    color: [u8; 3],
    camera: &Camera,
    origin: [f64; 3],
    height: f64,
) {
    let points: Vec<[f64; 3]> = points.iter().map(|p| relative(*p, origin)).collect();
    ribbon(
        out,
        &points,
        color,
        &relative_camera(camera, origin),
        height,
    );
}

/// Appends camera-facing ribbons along `points` to combat geometry, in its
/// ten-float emissive vertex format, about [`WIDTH`] pixels wide on a view
/// `height` pixels tall at every distance. `points` and `camera` must be in
/// the space the geometry is in, relative to the render origin; use
/// [`ribbon_world`] for world positions.
pub fn ribbon(
    out: &mut Vec<f32>,
    points: &[[f64; 3]],
    color: [u8; 3],
    camera: &Camera,
    height: f64,
) {
    ribbon_wide(out, points, color, camera, height, WIDTH);
}

/// [`ribbon`] with a width of `width` pixels of a 480-line view.
pub fn ribbon_wide(
    out: &mut Vec<f32>,
    points: &[[f64; 3]],
    color: [u8; 3],
    camera: &Camera,
    height: f64,
    width: f64,
) {
    let eye = camera.position;
    let focal = height / 2. * 3f64.sqrt() * f64::from(camera.zoom.max(0.01));
    let pixels = width * height / 480.;
    let (sy, cy) = f64::from(camera.yaw).sin_cos();
    let (sp, cp) = f64::from(camera.pitch).sin_cos();
    let forward = [sy * cp, sp, cy * cp];
    // Half the width at a point's depth along the view, so the ribbon keeps
    // its width on screen near and far.
    let half = |p: [f64; 3]| {
        let depth = tore_sim::attitude::dot(sub(p, eye), forward).max(1.);
        pixels / 2. * depth / focal
    };
    let color = color.map(|c| f32::from(c) / 255.);
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let along = sub(b, a);
        if length(along) < 1e-3 {
            continue;
        }
        let mid: [f64; 3] = std::array::from_fn(|i| (a[i] + b[i]) / 2.);
        let view = sub(eye, mid);
        let side = tore_sim::attitude::cross(along, view);
        let norm = length(side);
        // Seen end on, a segment has no width to draw.
        if norm < 1e-9 * length(along) * length(view).max(1.) {
            continue;
        }
        let side = side.map(|v| v / norm);
        let (ha, hb) = (half(a), half(b));
        let corner = |p: [f64; 3], h: f64, sign: f64| -> [f64; 3] {
            std::array::from_fn(|i| p[i] + side[i] * h * sign)
        };
        for p in [
            corner(a, ha, -1.),
            corner(b, hb, -1.),
            corner(b, hb, 1.),
            corner(a, ha, -1.),
            corner(b, hb, 1.),
            corner(a, ha, 1.),
        ] {
            // Position, no texture, layer -6 (an emissive effect), the
            // colour, and -1: the colour is final, not a palette index.
            out.extend([
                p[0] as f32,
                p[1] as f32,
                p[2] as f32,
                0.,
                0.,
                -6.,
                color[0],
                color[1],
                color[2],
                -1.,
            ]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(to: u64) -> Vec<(u64, [f32; 3])> {
        (0..=to)
            .step_by(SAMPLE_TICKS as usize)
            .map(|t| (t, [t as f32, 5_000., 0.]))
            .collect()
    }

    #[test]
    fn a_trail_is_the_recent_samples_then_the_head() {
        let samples = samples(2_004);
        let points = path(&samples, 1_000, 5, [1_000.5, 5_000., 0.]);
        // 400 to 996, then the head.
        assert_eq!(points.len(), 51);
        assert_eq!(points[0], [408., 5_000., 0.]);
        assert_eq!(points[49], [996., 5_000., 0.]);
        assert_eq!(points[50], [1_000.5, 5_000., 0.]);
        // Early on the trail is as long as the flight.
        assert_eq!(path(&samples, 30, 5, [30., 5_000., 0.]).len(), 4);
        // Before any sample, only the head.
        assert_eq!(path(&[], 30, 5, [1.; 3]), vec![[1.; 3]]);
    }

    #[test]
    fn long_trails_thin_on_fixed_ticks_so_they_do_not_shimmer() {
        let samples = samples(120 * 400);
        let a = path(&samples, 120 * 350, 300, [0.; 3]);
        let b = path(&samples, 120 * 350 + 12, 300, [0.; 3]);
        assert!(a.len() as u64 <= MAX_POINTS + 2);
        // The middle of the trail is the same points one sample later.
        let inner = |p: &Vec<[f64; 3]>| p[1..p.len() - 1].to_vec();
        let (ia, ib) = (inner(&a), inner(&b));
        assert!(ia.iter().filter(|p| ib.contains(p)).count() >= ia.len() - 1);
    }

    #[test]
    fn ribbons_face_the_camera_at_a_constant_screen_width() {
        let mut camera = Camera::new();
        camera.position = [0., 5_000., -1_000.];
        camera.yaw = 0.;
        camera.pitch = 0.;
        camera.roll = 0.;
        let points = [
            [-500., 5_000., 1_000.],
            [0., 5_000., 1_000.],
            [500., 5_000., 4_000.],
        ];
        let mut out = Vec::new();
        ribbon(&mut out, &points, [255, 0, 0], &camera, 960.);
        assert_eq!(out.len(), 2 * 6 * 10);
        assert!(out.iter().all(|v| v.is_finite()));
        assert!(
            out.chunks_exact(10)
                .all(|v| v[5] == -6. && v[6] == 1. && v[9] == -1.)
        );
        // Each end's two edges project the same number of pixels apart,
        // near or far.
        for (edge_a, edge_b) in [(0, 5), (2, 1)] {
            let vertex =
                |i: usize| -> [f64; 3] { std::array::from_fn(|k| f64::from(out[i * 10 + k])) };
            let [pa, pb] =
                [vertex(edge_a), vertex(edge_b)].map(|p| camera.project([1280, 960], p).unwrap());
            let apart = (pa[0] - pb[0]).hypot(pa[1] - pb[1]);
            assert!((apart - 3.).abs() < 0.05, "{apart}");
        }
        // A segment of no length, or seen end on, draws nothing.
        let mut out = Vec::new();
        ribbon(
            &mut out,
            &[[0., 5_000., 0.], [0., 5_000., 0.]],
            [0; 3],
            &camera,
            960.,
        );
        ribbon(
            &mut out,
            &[[0., 5_000., 0.], [0., 5_000., 1_000.]],
            [0; 3],
            &camera,
            960.,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn a_trail_far_from_the_map_corner_lands_on_the_aircraft_it_follows() {
        // Combat geometry is relative to the render origin, a cell-rounded
        // copy of the eye. A trail drawn from world positions must be moved
        // by it, or it lands the origin's distance away.
        let mut camera = Camera::new();
        camera.position = [310_000.4, 8_000., -215_000.7];
        camera.yaw = 0.7;
        camera.pitch = -0.1;
        camera.roll = 0.;
        let origin = camera.position.map(|v| (v / 1024.).round() * 1024.);
        // A path ahead of and below the camera, ending at the aircraft.
        let points = [
            [310_900., 7_800., -214_000.],
            [311_300., 7_850., -213_500.],
            [311_800., 7_900., -213_000.],
        ];
        let mut out = Vec::new();
        ribbon_world(&mut out, &points, [255, 0, 0], &camera, origin, 960.);
        assert_eq!(out.len(), 2 * 6 * 10);
        assert!(out.iter().all(|v| v.is_finite()));
        // The vertices project, through the eye moved the same way, to the
        // pixels the world points project to through the real eye.
        let eye = relative_camera(&camera, origin);
        let size = [1280, 960];
        let mut hits = 0;
        for point in points {
            let wanted = camera.project(size, point).unwrap();
            let near = out.chunks_exact(10).any(|v| {
                let at = std::array::from_fn(|k| f64::from(v[k]));
                eye.project(size, at)
                    .is_some_and(|p| (p[0] - wanted[0]).hypot(p[1] - wanted[1]) < 3.)
            });
            hits += usize::from(near);
        }
        assert_eq!(hits, 3);
        // Every vertex is a few feet from its point in the origin's frame,
        // not hundreds of thousands of feet out.
        let local = points.map(|p| relative(p, origin));
        assert!(out.chunks_exact(10).all(|v| {
            let at: [f64; 3] = std::array::from_fn(|k| f64::from(v[k]));
            local.iter().any(|p| length(sub(at, *p)) < 100.)
        }));
        assert!(out.iter().step_by(10).all(|x| x.abs() < 10_000.));
    }

    #[test]
    fn the_relative_eye_keeps_its_aim() {
        let mut camera = Camera::new();
        camera.position = [5_000., 100., -7_000.];
        [camera.yaw, camera.pitch, camera.roll] = [1.2, 0.3, 0.1];
        camera.zoom = 2.;
        let moved = relative_camera(&camera, [4_096., 0., -8_192.]);
        assert_eq!(moved.position, [904., 100., 1_192.]);
        assert_eq!(
            [moved.yaw, moved.pitch, moved.roll, moved.zoom],
            [1.2, 0.3, 0.1, 2.]
        );
    }

    #[test]
    fn weapons_wear_their_owners_colour_paler() {
        assert_eq!(side_color(Side::Enemy), [255, 100, 90]);
        assert_eq!(weapon_color(Side::Enemy), [255, 177, 172]);
        assert_eq!(weapon_color(Side::Unknown), [237, 237, 237]);
    }
}
