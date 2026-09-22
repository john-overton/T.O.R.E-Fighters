//! Read-only target-window presentation. See docs/spec/target-window.md.
use crate::{ai_wings::AiWings, flight::State};
use tore_sim::combat::live::Target;

/// Wall-clock preview cadence. Integer phase tracking avoids reducing a 24 Hz
/// schedule to 20 Hz when the host presents at 60 Hz. Missed frames are skipped.
pub struct Refresh {
    epoch: std::time::Instant,
    phase: u128,
}
impl Refresh {
    pub fn new() -> Self {
        Self {
            epoch: std::time::Instant::now(),
            phase: 0,
        }
    }
    pub fn due(&mut self, now: std::time::Instant) -> bool {
        // Permit one nanosecond of timestamp rounding at exact frame boundaries.
        let phase = (now.saturating_duration_since(self.epoch).as_nanos() + 1) * 24 / 1_000_000_000;
        if phase <= self.phase {
            return false;
        }
        self.phase = phase;
        true
    }
}

/// Camera on the player-to-target segment, at most one nautical mile from the
/// subject. Nearby targets retain the player-eye position. Roll stays level.
pub fn camera(eye: [f64; 3], target: [f64; 3]) -> crate::terrain::Camera {
    let delta: [f64; 3] = std::array::from_fn(|i| target[i] - eye[i]);
    let mut camera = crate::terrain::Camera::new();
    let distance = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
    let maximum = tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
    camera.position = if distance > maximum {
        std::array::from_fn(|i| (target[i] - delta[i] * maximum / distance) as f32)
    } else {
        eye.map(|v| v as f32)
    };
    camera.yaw = delta[0].atan2(delta[2]) as f32;
    camera.pitch = delta[1].atan2(delta[0].hypot(delta[2])) as f32;
    camera.weather_slot = 4;
    camera
}

/// Fit the actual projected silhouette between the text rows. These limits
/// leave five percent side margins and the top/bottom information bands.
pub fn fit(camera: &mut crate::terrain::Camera, points: impl IntoIterator<Item = [f64; 3]>) {
    use tore_sim::attitude::{Basis, dot};
    let basis = Basis::new(f64::from(camera.yaw), f64::from(camera.pitch), 0.);
    let eye = camera.position.map(f64::from);
    let mut zoom = f64::INFINITY;
    let mut nearest = f64::INFINITY;
    for point in points {
        let delta = std::array::from_fn(|i| point[i] - eye[i]);
        let depth = dot(delta, basis.forward);
        if depth <= 1. {
            continue;
        }
        nearest = nearest.min(depth);
        let x = dot(delta, basis.right).abs() * 3_f64.sqrt() / (138. / 114.) / depth;
        let y = dot(delta, basis.up).abs() * 3_f64.sqrt() / depth;
        zoom = zoom.min(0.90 / x).min(0.52 / y);
    }
    if zoom.is_finite() {
        camera.zoom = zoom.clamp(0.0001, 1_000_000.) as f32;
        camera.near_clip = (nearest * 0.5).clamp(1., 1_000_000.) as f32;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetObjective {
    Survive,
    Destroy,
}

/// Target camera readback alpha separates scenery (0) from aircraft and static
/// objects (255). Restore opaque alpha after darkening scenery by ten percent.
pub fn monochrome(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        let luminance =
            (u32::from(pixel[0]) * 77 + u32::from(pixel[1]) * 150 + u32::from(pixel[2]) * 29) / 256;
        let value = 80 + luminance * 175 / 255;
        let value = (value * (2295 + u32::from(pixel[3])) + 1275) / 2550;
        pixel[..3].fill(value as u8);
        pixel[3] = 255;
    }
}

pub struct Readout {
    pub id: u32,
    pub name: String,
    pub damage: f64,
    pub bearing: String,
    pub metric: String,
    pub objective: Option<TargetObjective>,
    pub activity: String,
    pub goal: &'static str,
    pub player_goal: bool,
    pub skill: Option<u8>,
}
impl Readout {
    pub fn new(target: &Target, player: &State, name: String) -> Self {
        let offset: [f64; 3] = std::array::from_fn(|i| target.position[i] - player.position[i]);
        let bearing = clock_bearing(offset[0].atan2(offset[2]) - player.yaw);
        let altitude = elevation_label(offset);
        let norm = |v: [f64; 3]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
        Self {
            id: target.id,
            name,
            damage: damage_fraction(target.hp, target.initial_hp),
            bearing: format!("{bearing}:00{altitude}"),
            metric: metric(player.ticks, norm(offset), norm(target.velocity)),
            objective: None,
            activity: String::new(),
            goal: "?",
            player_goal: false,
            skill: None,
        }
    }
    pub fn with_activity(&mut self, wings: &AiWings) {
        self.objective = wings.target_objective(self.id);
        let Some(actor) = wings.mission().actor(self.id) else {
            return;
        };
        if actor.is_dummy() {
            self.activity = "DUMMY 400 KTS".into();
            self.goal = "N";
            self.skill = None;
            self.player_goal = false;
            return;
        }
        self.activity = actor.activity().label().to_ascii_uppercase();
        self.skill = Some(actor.controller().experience().level.level());
        (self.goal, self.player_goal) =
            activity_goal(actor.activity(), actor.controller().target());
    }
}
fn activity_goal(
    activity: tore_sim::ai::controller::Activity,
    target: Option<u32>,
) -> (&'static str, bool) {
    use tore_sim::ai::controller::Activity;
    let goal = match activity {
        Activity::Pursuing | Activity::Attacking => "A",
        Activity::Defending | Activity::Evading | Activity::Breaking => "E",
        Activity::Destroyed => "C",
        Activity::Idle
        | Activity::Formation
        | Activity::Searching
        | Activity::Acquiring
        | Activity::Rejoining
        | Activity::ReturningToBase
        | Activity::OutOfFuel => "N",
    };
    (
        goal,
        goal == "A" && target == Some(crate::ai_wings::PLAYER_ID),
    )
}
/// User-requested ten-degree threshold from the world horizontal plane.
/// Comparing rise against run * tan(10 degrees) also handles overhead targets
/// and the coincident-position case without dividing by zero.
fn elevation_label(offset: [f64; 3]) -> &'static str {
    let limit = offset[0].hypot(offset[2]) * 10_f64.to_radians().tan();
    if offset[1] > limit {
        " HI"
    } else if offset[1] < -limit {
        " LO"
    } else {
        ""
    }
}
fn clock_bearing(relative: f64) -> u8 {
    let hour = (relative.rem_euclid(std::f64::consts::TAU) / (std::f64::consts::PI / 6.)).round()
        as u8
        % 12;
    if hour == 0 { 12 } else { hour }
}
fn damage_fraction(hp: i32, initial: i32) -> f64 {
    (1. - f64::from(hp) / f64::from(initial.max(1))).clamp(0., 1.)
}
fn metric(ticks: u64, distance_ft: f64, speed_fps: f64) -> String {
    let feet_per_nm = tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
    if (ticks / 360).is_multiple_of(2) {
        format!("{:.1} NM", distance_ft / feet_per_nm)
    } else {
        format!("{:.0} KTS", speed_fps * 3600. / feet_per_nm)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_requests_average_24_fps_without_catch_up_bursts() {
        use std::time::{Duration, Instant};
        for host_fps in [24, 30, 60, 144] {
            let epoch = Instant::now();
            let mut clock = Refresh { epoch, phase: 0 };
            let count = (0..=host_fps * 10)
                .filter(|frame| {
                    clock.due(
                        epoch + Duration::from_secs_f64(f64::from(*frame) / f64::from(host_fps)),
                    )
                })
                .count();
            assert_eq!(count, 240);
            assert!(clock.due(epoch + Duration::from_secs(30)));
            assert!(!clock.due(epoch + Duration::from_secs(30)));
        }
    }

    #[test]
    fn target_contrast_darkens_scenery_without_dimming_objects() {
        let mut pixels = [200, 200, 200, 0, 200, 200, 200, 255];
        monochrome(&mut pixels);
        assert_eq!(&pixels[..4], &[195, 195, 195, 255]);
        assert_eq!(&pixels[4..], &[217, 217, 217, 255]);
    }

    #[test]
    fn distant_target_surfaces_retain_distinct_depths() {
        let mut camera = camera([0.; 3], [0., 0., 60000.]);
        fit(&mut camera, [[-20., -10., 59950.], [20., 10., 60050.]]);
        assert_eq!(camera.near_clip, 3013.);
        let depth = |near: f32, z: f32| {
            let far = 2200000_f32;
            (far / (far - near) * z - near * far / (far - near)) / z
        };
        // The old one-foot near plane cannot resolve surfaces 1.5 inches apart.
        assert_eq!(depth(1., 60000.), depth(1., 60000.125));
        assert!(depth(camera.near_clip, 6076.) < depth(camera.near_clip, 6076.125));
        assert!(camera.near_clip < 6026.);
        assert_eq!(crate::terrain::Camera::new().near_clip, 1.);
    }

    #[test]
    fn hi_lo_uses_strict_ten_degree_elevation_at_any_range() {
        for horizontal in [100., 1000., 10000.] {
            for (degrees, label) in [
                (-10.001_f64, " LO"),
                (-10., ""),
                (-9.999, ""),
                (0., ""),
                (9.999, ""),
                (10., ""),
                (10.001, " HI"),
            ] {
                let height = horizontal * degrees.to_radians().tan();
                assert_eq!(elevation_label([horizontal, height, 0.]), label);
                assert_eq!(elevation_label([0., height, -horizontal]), label);
            }
        }
        // Equal height differences have different labels at different ranges.
        assert_eq!(elevation_label([100., 100., 0.]), " HI");
        assert_eq!(elevation_label([1000., 100., 0.]), "");
        assert_eq!(elevation_label([10000., 600., 0.]), "");
        assert_eq!(elevation_label([0., 100., 0.]), " HI");
        assert_eq!(elevation_label([0., -100., 0.]), " LO");
        assert_eq!(elevation_label([0.; 3]), "");
    }

    #[test]
    fn camera_stays_on_the_sight_line_within_one_nautical_mile() {
        use tore_sim::attitude::{Basis, dot};
        let eye = [1200., 4500., -300.];
        for target in [
            [1800., 6500., 400.],
            [0., 500., -5000.],
            [1200., 9000., -300.],
            [18000., -6500., 40000.],
            [1200., 4500., 5776.], // Exactly one nautical mile.
        ] {
            let camera = camera(eye, target);
            let delta = std::array::from_fn(|i| target[i] - eye[i]);
            let distance = dot(delta, delta).sqrt();
            let to_target = std::array::from_fn(|i| target[i] - f64::from(camera.position[i]));
            let remaining = dot(to_target, to_target).sqrt();
            assert!((remaining - distance.min(6076.)).abs() < 0.01);
            if distance <= 6076. {
                assert_eq!(camera.position, eye.map(|v| v as f32));
            }
            let along = dot(to_target, delta) / dot(delta, delta);
            assert!((0. ..=1.).contains(&along));
            let off_line: [f64; 3] = std::array::from_fn(|i| to_target[i] - along * delta[i]);
            assert!(dot(off_line, off_line).sqrt() < 0.01);
            let basis = Basis::new(f64::from(camera.yaw), f64::from(camera.pitch), 0.);
            assert!(dot(to_target, basis.right).abs() < 0.01);
            assert!(dot(to_target, basis.up).abs() < 0.01);
            assert!(dot(to_target, basis.forward) > 0.);
        }
        let coincident = camera(eye, eye);
        assert_eq!(coincident.position, eye.map(|v| v as f32));
        assert!(coincident.yaw.is_finite() && coincident.pitch.is_finite());
    }

    #[test]
    fn changing_range_and_aspect_keeps_model_filling_the_image() {
        let points: Vec<[f64; 3]> = (0..8)
            .map(|corner| {
                std::array::from_fn(|i| {
                    [20., 8., 35.][i] * if corner & (1 << i) == 0 { -1. } else { 1. }
                })
            })
            .collect();
        for eye in [
            [0., 0., -1000.],
            [0., 0., -10000.],
            [4000., 2000., 1000.],
            [-6000., -3000., 100.],
            [0., 5000., 0.],
        ] {
            let mut camera = camera(eye, [0.; 3]);
            fit(&mut camera, points.iter().copied());
            // Project through the same public camera matrix consumed by WGSL.
            let u = camera.uniform(138. / 114., [0.; 4], [0; 3]);
            let mut occupied = 0_f64;
            for point in &points {
                let delta: [f64; 3] = std::array::from_fn(|i| point[i] - f64::from(u[i]));
                let axis = |start: usize| {
                    (0..3)
                        .map(|i| delta[i] * f64::from(u[start + i]))
                        .sum::<f64>()
                };
                let depth = axis(12);
                let x = axis(4) * 3_f64.sqrt() * f64::from(u[11]) / f64::from(u[3]) / depth;
                let y = axis(8) * 3_f64.sqrt() * f64::from(u[11]) / depth;
                assert!(x.abs() <= 0.90001 && y.abs() <= 0.52001);
                occupied = occupied.max(x.abs() / 0.9).max(y.abs() / 0.52);
            }
            assert!(
                (occupied - 1.).abs() < 0.0001,
                "target must fill one available dimension: {occupied}"
            );
        }
    }

    #[test]
    fn goals_only_underline_confirmed_player_attacks() {
        use tore_sim::ai::controller::Activity;
        assert_eq!(activity_goal(Activity::Attacking, Some(0)), ("A", true));
        assert_eq!(activity_goal(Activity::Attacking, Some(7)), ("A", false));
        assert_eq!(activity_goal(Activity::Pursuing, None), ("A", false));
        // Selected attack target does not identify the threat being evaded.
        assert_eq!(activity_goal(Activity::Evading, Some(0)), ("E", false));
        assert_eq!(activity_goal(Activity::Searching, Some(0)), ("N", false));
        assert_eq!(activity_goal(Activity::Acquiring, Some(0)), ("N", false));
        assert_eq!(activity_goal(Activity::Rejoining, Some(0)), ("N", false));
        assert_eq!(activity_goal(Activity::ReturningToBase, None), ("N", false));
        assert_eq!(activity_goal(Activity::Destroyed, None), ("C", false));
    }
    #[test]
    fn clock_wrap_and_cardinal_bearings() {
        for (degrees, hour) in [
            (0., 12),
            (90., 3),
            (180., 6),
            (-90., 9),
            (359., 12),
            (14., 12),
            (16., 1),
        ] {
            assert_eq!(clock_bearing(f64::to_radians(degrees)), hour);
        }
    }
    #[test]
    fn cycle_changes_at_exact_three_second_boundaries() {
        let nm = tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
        for tick in [0, 359, 720] {
            assert_eq!(metric(tick, nm * 6.2, nm * 254. / 3600.), "6.2 NM");
        }
        for tick in [360, 719] {
            assert_eq!(metric(tick, nm * 6.2, nm * 254. / 3600.), "254 KTS");
        }
    }
    #[test]
    fn damage_endpoints_and_clamping() {
        for (hp, expected) in [(100, 0.), (75, 0.25), (0, 1.), (-1, 1.), (101, 0.)] {
            assert_eq!(damage_fraction(hp, 100), expected);
        }
    }
}
