//! Manual-style weapon symbols with fitted layout and simulation-owned state.
use crate::{
    combat, flight,
    hud::{self, Paint},
};
use tore_formats::font::Font;
use tore_sim::{
    attitude::{Basis, Vector, dot},
    combat::{
        gunsight, live,
        missiles::{self, Guidance, LaunchMode, seeker::Status},
    },
};

/// Reserve the weapon readout area even for a safe gun, keeping flight text clear.
pub fn active(state: &live::State) -> bool {
    let weapon = &state.configuration().stations[state.selected].weapon;
    live::is_gun(weapon) || (state.armed && missiles::Profile::for_weapon(weapon).is_some())
}

fn debug_point(point: (f64, f64), size: [f64; 2]) -> (f64, f64) {
    let scale = (size[0] / 640.).min(size[1] / 480.);
    (
        (point.0 - size[0] + 258. * scale) / scale,
        (point.1 - 8. * scale) / scale,
    )
}
pub fn mode_hit(point: (f64, f64), size: [f64; 2]) -> bool {
    let (x, y) = debug_point(point, size);
    (4. ..130.).contains(&x) && (4. ..18.).contains(&y)
}
pub fn release_hit(point: (f64, f64), size: [f64; 2]) -> bool {
    let (x, y) = debug_point(point, size);
    (132. ..246.).contains(&x) && (4. ..18.).contains(&y)
}
fn diamond_visible(locked: bool, radar_in_range: bool, tick: u64) -> bool {
    locked && (!radar_in_range || tick % 60 < 30)
}
fn projected(direction: Vector, s: &flight::State, zoom: f64) -> Option<(f64, f64)> {
    let bearing = direction[0].atan2(direction[2]) - s.yaw;
    let elevation = direction[1].atan2(direction[0].hypot(direction[2]));
    hud::project(s.pitch, s.bank, bearing, elevation, zoom)
}
/// The boundary uses the exact independent heading/elevation tests of the seeker.
fn boundary(basis: Basis, horizontal: f64, vertical: f64) -> Vec<Vector> {
    let mut points = Vec::new();
    for edge in 0..4 {
        for n in 0..=24 {
            let t = f64::from(n) / 24.;
            let (h, v) = match edge {
                0 => (-horizontal + 2. * horizontal * t, -vertical),
                1 => (horizontal, -vertical + 2. * vertical * t),
                2 => (horizontal - 2. * horizontal * t, vertical),
                _ => (-horizontal, vertical - 2. * vertical * t),
            };
            points.push(std::array::from_fn(|i| {
                basis.forward[i] * h.cos() * v.cos()
                    + basis.right[i] * h.sin() * v.cos()
                    + basis.up[i] * v.sin()
            }));
        }
    }
    points
}
#[allow(clippy::too_many_arguments)]
pub fn draw(
    pixels: &mut [u8],
    s: &flight::State,
    state: &live::State,
    font: &Font,
    color: [u8; 3],
    zoom: f64,
    nav_mode: bool,
) {
    draw_target(pixels, s, state, color, zoom);
    if nav_mode {
        return;
    }
    if live::is_gun(&state.configuration().stations[state.selected].weapon) {
        draw_gun(pixels, s, state, font, color, zoom);
        return;
    }
    if !active(state) {
        return;
    }
    let station = &state.configuration().stations[state.selected];
    let w = &station.weapon;
    let Some(profile) = missiles::Profile::for_weapon(w) else {
        return;
    };
    let l = combat::launcher(s);
    let mut paint = Paint {
        pixels,
        clip: hud::HUD_CLIP,
        color: [color[0], color[1], color[2], 255],
    };
    let bore = state.guidance_available(l) && state.launch_mode == LaunchMode::Boresight;
    let cap = if bore {
        profile.search_cap()
    } else {
        std::f64::consts::PI
    };
    let zone = &w.seeker.zones[0];
    let cone = if !state.guidance_available(l) {
        Vec::new()
    } else if bore {
        let angle = cap
            .min(missiles::half_angle(zone.heading))
            .min(missiles::half_angle(zone.pitch));
        (0..=96)
            .map(|i| {
                let a = f64::from(i) * std::f64::consts::TAU / 96.;
                std::array::from_fn(|j| {
                    l.basis.forward[j] * angle.cos()
                        + (l.basis.right[j] * a.cos() + l.basis.up[j] * a.sin()) * angle.sin()
                })
            })
            .collect()
    } else {
        boundary(
            l.basis,
            missiles::half_angle(zone.heading),
            missiles::half_angle(zone.pitch),
        )
    };
    for pair in cone.windows(2) {
        if let (Some(a), Some(b)) = (projected(pair[0], s, zoom), projected(pair[1], s, zoom)) {
            paint.line(a, b);
        }
    }
    if let Some((x, y)) = projected(l.basis.forward, s, zoom) {
        if bore {
            let radius = 240. * 3f64.sqrt() * cap.tan() * zoom;
            paint.text(font, "BORE", x as i32 - 12, (y - radius + 5.) as i32);
        }
        for i in 0..32 {
            let a = f64::from(i) * std::f64::consts::TAU / 32.;
            let b = f64::from(i + 1) * std::f64::consts::TAU / 32.;
            paint.line(
                (x + 9. * a.cos(), y + 9. * a.sin()),
                (x + 9. * b.cos(), y + 9. * b.sin()),
            );
        }
    }
    let observed = state.weapon_observation(l);
    let in_range = state.in_estimated_range(l);
    if let Some(observation) = observed {
        let position = observation.position;
        if let Some((x, y)) = projected(missiles::sub(position, s.position), s, zoom) {
            let radar_in_range =
                matches!(profile.guidance, Guidance::Active | Guidance::Supported) && in_range;
            if diamond_visible(
                bore || matches!(state.mounted.status, Status::Locked | Status::Pitbull),
                bore || radar_in_range,
                state.sensors.tick(),
            ) {
                for (a, b) in [
                    ((0., -7.), (7., 0.)),
                    ((7., 0.), (0., 7.)),
                    ((0., 7.), (-7., 0.)),
                    ((-7., 0.), (0., -7.)),
                ] {
                    paint.line((x + a.0, y + a.1), (x + b.0, y + b.1));
                }
            }
        }
        let range = missiles::length(missiles::sub(position, s.position));
        let min = f64::from(w.seeker.zones[1].minimum_range);
        let max = state.estimated_max_range(l).unwrap_or(0.);
        // Retail reference: range scale just inside the altitude tape.
        let text_width = |text: &str| {
            text.bytes()
                .map(|c| font.glyphs[c as usize].advance)
                .sum::<usize>() as i32
        };
        let right = 390;
        let x = f64::from(right);
        let (top, bottom) = (230., 282.);
        paint.line((x, top), (x, bottom));
        paint.line((x - 5., top), (x, top));
        paint.line((x - 5., bottom), (x, bottom));
        let maximum = if max > min {
            format!("{:.1}", max / missiles::NMI)
        } else {
            "--".into()
        };
        let minimum = format!("{:.1}", min / missiles::NMI);
        paint.text(font, &maximum, right - text_width(&maximum), 218);
        paint.text(font, &minimum, right - text_width(&minimum), 284);
        if max > min
            && let Some(band) = state.favorable_firing_band(l)
        {
            let scale_y =
                |range: f64| bottom - (bottom - top) * ((range - min) / (max - min)).clamp(0., 1.);
            let upper = scale_y(band.maximum);
            let lower = scale_y(band.minimum);
            paint.line((x - 6., upper), (x, upper));
            paint.line((x - 6., lower), (x, lower));
        }
        if max > min && (!bore || state.sensors.tick() % 60 < 30) {
            let y = bottom - (bottom - top) * ((range - min) / (max - min)).clamp(0., 1.);
            paint.line((x - 8., y - 3.), (x - 2., y));
            paint.line((x - 2., y), (x - 8., y + 3.));
            paint.line((x - 8., y + 3.), (x - 8., y - 3.));
        }
    }
    paint.text(font, "ARM", 207, 395);
    paint.text(
        font,
        &format!("{} {}", state.rounds(state.selected), w.hud_name),
        207,
        407,
    );
    let ready = state.readiness(l);
    let percent = format!("{}%", state.estimated_hit_percent(l));
    paint.text(font, &percent, 207, 422);
    if in_range && state.sensors.tick() % 60 < 30 {
        let width: usize = percent
            .bytes()
            .map(|c| font.glyphs[c as usize].advance)
            .sum();
        paint.text(font, "IN RNG", 211 + width as i32, 422);
    } else if !matches!(
        ready,
        live::Readiness::Ready | live::Readiness::TargetDestroyed
    ) {
        paint.text(font, ready.label(), 207, 437);
    }
    if matches!(profile.guidance, Guidance::Active | Guidance::Supported)
        && let Some(o) = observed
    {
        let closure = missiles::closure(s.position, s.velocity, o.position, o.velocity) / 1.68781;
        paint.text(font, &format!("R {:.1}", o.range / missiles::NMI), 402, 407);
        paint.text(font, &format!("C {closure:+.0}"), 402, 419);
        let aspect = if missiles::length(o.velocity) > 1e-9 {
            let forward = tore_sim::attitude::unit(o.velocity);
            let los = tore_sim::attitude::unit(missiles::sub(s.position, o.position));
            let angle = dot(forward, los).clamp(-1., 1.).acos().to_degrees();
            let side = if forward[2] * los[0] - forward[0] * los[2] >= 0. {
                "R"
            } else {
                "L"
            };
            format!("A {angle:.0}{side}")
        } else {
            "A --".into()
        };
        paint.text(font, &aspect, 402, 431);
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum TargetCue {
    Square((f64, f64)),
    Chevron {
        point: (f64, f64),
        direction: (f64, f64),
    },
}

/// The rear hemisphere keeps the bearing's sign instead of perspective-flipping.
fn target_cue(direction: Vector, basis: Basis, zoom: f64) -> Option<TargetCue> {
    if !direction.iter().all(|v| v.is_finite()) || dot(direction, direction) < 1e-12 {
        return None;
    }
    let x = dot(direction, basis.right);
    let y = -dot(direction, basis.up);
    let z = dot(direction, basis.forward);
    if z > 1e-9 {
        let focal = 240. * 3f64.sqrt() * zoom;
        let point = (320. + focal * x / z, 240. + focal * y / z);
        if (184. ..=456.).contains(&point.0)
            && (106. ..=f64::from(hud::AIM_BOTTOM - 10)).contains(&point.1)
        {
            return Some(TargetCue::Square(point));
        }
    }
    let length = x.hypot(y);
    let (dx, dy) = if length < 1e-9 {
        (1., 0.)
    } else {
        (x / length, y / length)
    };
    let tx = if dx.abs() < 1e-12 {
        f64::INFINITY
    } else {
        136. / dx.abs()
    };
    let ty = if dy.abs() < 1e-12 {
        f64::INFINITY
    } else if dy > 0. {
        (f64::from(hud::AIM_BOTTOM - 10) - 240.) / dy
    } else {
        -134. / dy
    };
    let distance = tx.min(ty);
    Some(TargetCue::Chevron {
        point: (320. + dx * distance, 240. + dy * distance),
        direction: (dx, dy),
    })
}
fn draw_target(
    pixels: &mut [u8],
    s: &flight::State,
    state: &live::State,
    color: [u8; 3],
    zoom: f64,
) {
    let Some(target) = state.display_target() else {
        return;
    };
    let Some(cue) = target_cue(
        missiles::sub(target.position, s.position),
        Basis::new(s.yaw, s.pitch, s.bank),
        zoom,
    ) else {
        return;
    };
    let mut paint = Paint {
        pixels,
        clip: (174, 96, 292, hud::AIM_BOTTOM - 96),
        color: [color[0], color[1], color[2], 255],
    };
    match cue {
        TargetCue::Square((x, y)) => {
            for (a, b) in [
                ((-7., -7.), (7., -7.)),
                ((7., -7.), (7., 7.)),
                ((7., 7.), (-7., 7.)),
                ((-7., 7.), (-7., -7.)),
            ] {
                paint.line((x + a.0, y + a.1), (x + b.0, y + b.1));
            }
        }
        TargetCue::Chevron {
            point: (x, y),
            direction: (dx, dy),
        } => {
            for side in [-1., 1.] {
                paint.line(
                    (x - dx * 8. - dy * side * 4., y - dy * 8. + dx * side * 4.),
                    (x, y),
                );
            }
        }
    }
}
fn draw_gun(
    pixels: &mut [u8],
    s: &flight::State,
    state: &live::State,
    font: &Font,
    color: [u8; 3],
    zoom: f64,
) {
    let mut paint = Paint {
        pixels,
        clip: hud::HUD_CLIP,
        color: [color[0], color[1], color[2], 255],
    };
    let station = &state.configuration().stations[state.selected];
    paint.text(
        font,
        &format!(
            "{} {}",
            state.rounds(state.selected),
            station.weapon.hud_name
        ),
        207,
        407,
    );
    paint.text(font, if state.armed { "ARM" } else { "SAFE" }, 207, 395);
    let l = combat::launcher(s);
    if !state.armed
        || !l.alive
        || state.rounds(state.selected) == 0
        || state.readiness(l) == live::Readiness::StationFailed
    {
        return;
    }
    let observation = state
        .designated()
        .and_then(|id| state.sensors.observation(id))
        .filter(|c| !c.destroyed);
    let radar = observation
        .filter(|c| {
            c.channel == tore_sim::sensors::Channel::Radar
                && state.sensors.operating(tore_sim::sensors::Channel::Radar)
                && l.radar
        })
        .map(|c| gunsight::TargetObservation {
            position: c.position,
            velocity: c.velocity,
        });
    let solution = gunsight::solve(&station.weapon, &l, station.mount, radar)
        .ok()
        .flatten();
    let Some(solution) = solution else {
        paint.text(font, "NO SOL", 207, 422);
        return;
    };
    paint.text(
        font,
        if solution.radar { "RADAR" } else { "1000 FT" },
        207,
        422,
    );
    let range = observation.map(|c| missiles::length(missiles::sub(c.position, s.position)));
    if let Some(range) = range {
        paint.text(font, &format!("R {:.2}", range / missiles::NMI), 402, 407);
    }
    let Some((x, y)) = projected(missiles::sub(solution.point, s.position), s, zoom) else {
        return;
    };
    if !(184. ..=456.).contains(&x) || !(106. ..=f64::from(hud::AIM_BOTTOM - 10)).contains(&y) {
        return;
    }
    paint.clip = (174, 96, 292, hud::AIM_BOTTOM - 96);
    let arc = range.map_or(0., |r| {
        gunsight::range_arc_fraction(r, solution.maximum_range_ft)
    });
    for segment in 0..64 {
        let a = f64::from(segment) * std::f64::consts::TAU / 64. - std::f64::consts::FRAC_PI_2;
        let b = f64::from(segment + 1) * std::f64::consts::TAU / 64. - std::f64::consts::FRAC_PI_2;
        paint.line(
            (x + 9. * a.cos(), y + 9. * a.sin()),
            (x + 9. * b.cos(), y + 9. * b.sin()),
        );
        if f64::from(segment) / 64. < arc {
            for radius in [10., 11.] {
                paint.line(
                    (x + radius * a.cos(), y + radius * a.sin()),
                    (x + radius * b.cos(), y + radius * b.sin()),
                );
            }
        }
    }
    paint.rect(x.round() as i32, y.round() as i32, 2, 2);
}

/// Diagnostic state and controls are composed at the window's upper right.
pub fn debug(
    pixels: &mut [u8],
    state: &live::State,
    s: &flight::State,
    font: &Font,
    color: [u8; 3],
    nav_mode: bool,
) {
    let mut paint = Paint {
        pixels,
        clip: (0, 0, 250, 96),
        color: [0, 0, 0, 190],
    };
    paint.rect(0, 0, 250, 96);
    paint.color = [color[0], color[1], color[2], 255];
    paint.text(
        font,
        if nav_mode {
            "NAV"
        } else if live::is_gun(&state.configuration().stations[state.selected].weapon) {
            "GUN"
        } else if state.weapon_rules == missiles::Rules::Compatibility {
            "COMPATIBILITY"
        } else {
            state.launch_mode.label()
        },
        4,
        4,
    );
    paint.text(font, "[RELEASE LOCK]", 132, 4);
    paint.text(
        font,
        if state.armed {
            state.mounted.status.label()
        } else {
            "SAFE"
        },
        4,
        18,
    );
    let l = combat::launcher(s);
    let details = if let Some(o) = state.weapon_observation(l) {
        let closing = missiles::closure(s.position, s.velocity, o.position, o.velocity) / 1.68781;
        format!("R {:.1}NM C {closing:+.0}KT", o.range / missiles::NMI)
    } else {
        "R -- C --".into()
    };
    paint.text(font, &details, 4, 32);
    let time = state
        .mounted_solution(l)
        .map_or_else(|| "EST --".into(), |s| format!("EST {:.1}S", s.seconds));
    paint.text(font, &time, 4, 44);
    if let Some(o) = state.weapon_observation(l)
        && missiles::length(o.velocity) > 1e-9
    {
        let aspect = dot(
            tore_sim::attitude::unit(o.velocity),
            tore_sim::attitude::unit(missiles::sub(s.position, o.position)),
        )
        .clamp(-1., 1.)
        .acos()
        .to_degrees();
        paint.text(font, &format!("ASP {aspect:.0}"), 132, 44);
    }
    for (row, shot) in state
        .projectiles
        .iter()
        .filter(|p| p.guidance.is_some())
        .rev()
        .take(3)
        .enumerate()
    {
        let f = shot.guidance.as_ref().unwrap();
        let remaining = shot
            .guidance_ticks
            .unwrap_or(f.profile.guidance_ticks)
            .saturating_sub(shot.age) as f64
            / 120.;
        let motor = match missiles::phase(&shot.weapon(state.configuration()).movement, shot.age) {
            tore_sim::combat::EnginePhase::BeforeIgnition => "WAIT",
            tore_sim::combat::EnginePhase::Powered => "BURN",
            tore_sim::combat::EnginePhase::Coast => "COAST",
        };
        paint.text(
            font,
            &format!(
                "#{} {} {} {motor} {remaining:.0}S",
                shot.id,
                shot.weapon(state.configuration()).hud_name,
                if f.seeker.status == Status::Search && f.profile.guidance != Guidance::Active {
                    "SEARCH"
                } else {
                    f.seeker.status.label()
                }
            ),
            4,
            58 + row as i32 * 11,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_square_and_chevron_follow_full_three_dimensional_attitude() {
        let basis = Basis::new(0., 0., 0.);
        assert_eq!(
            target_cue([0., 0., 1000.], basis, 1.),
            Some(TargetCue::Square((320., 240.)))
        );
        for (point, expected) in [
            ([1000., 0., 1000.], (456., 240.)),
            ([-1000., 0., 1000.], (184., 240.)),
            ([0., 1000., 1000.], (320., 106.)),
            ([0., -1000., 1000.], (320., f64::from(hud::AIM_BOTTOM - 10))),
            ([0., 0., -1000.], (456., 240.)),
            ([1000., 0., -1000.], (456., 240.)),
        ] {
            let Some(TargetCue::Chevron { point, .. }) = target_cue(point, basis, 1.) else {
                panic!("missing off-HUD chevron");
            };
            assert_eq!(point, expected);
        }
        for (yaw, pitch, bank) in [
            (1., 0.8, 1.4),
            (2., -1.5, 3.0),
            (0., 0., std::f64::consts::FRAC_PI_2),
        ] {
            let body = Basis::new(yaw, pitch, bank);
            assert!(matches!(
                target_cue(body.forward.map(|v| v * 1000.), body, 1.),
                Some(TargetCue::Square(_))
            ));
            let direction = std::array::from_fn(|i| {
                body.forward[i] * 1000. + body.right[i] * 500. + body.up[i] * 100.
            });
            let Some(TargetCue::Chevron { point, direction }) = target_cue(direction, body, 1.)
            else {
                panic!("rotated cue");
            };
            assert!((point.0 - 456.).abs() < 1e-8 && point.1 < 240.);
            assert!(direction.0 > 0. && direction.1 < 0.);
        }
        assert_eq!(target_cue([0.; 3], basis, 1.), None);
        assert_eq!(target_cue([f64::NAN, 0., 1.], basis, 1.), None);
    }
    #[test]
    fn target_cue_crosses_hud_edge_without_reversing_or_clamping_a_box() {
        let basis = Basis::new(0., 0., 0.);
        for zoom in [0.5, 1., 2., 4.] {
            let edge = 136. / (240. * 3f64.sqrt() * zoom);
            assert!(matches!(
                target_cue([edge - 1e-6, 0., 1.], basis, zoom),
                Some(TargetCue::Square(_))
            ));
            assert!(matches!(
                target_cue([edge + 1e-6, 0., 1.], basis, zoom),
                Some(TargetCue::Chevron { .. })
            ));
        }
    }

    #[test]
    fn radar_ready_diamond_blinks_twice_per_simulation_second() {
        for tick in 0..120 {
            assert_eq!(diamond_visible(true, true, tick), tick % 60 < 30);
            assert!(diamond_visible(true, false, tick));
            assert!(!diamond_visible(false, true, tick));
        }
    }
    #[test]
    fn search_boundary_projects_with_zoom_and_hit_region_tracks_aspect() {
        let b = Basis::new(0., 0., 0.);
        let cone = boundary(b, 3f64.to_radians(), 3f64.to_radians());
        for d in cone {
            let h = d[0].atan2(d[2]).abs().to_degrees();
            let v = d[1].atan2(d[0].hypot(d[2])).abs().to_degrees();
            assert!(h <= 3. + 1e-9 && v <= 3. + 1e-9);
        }
        for size in [[640., 480.], [1920., 1080.], [1080., 1920.]] {
            let scale = (size[0] / 640f64).min(size[1] / 480.);
            assert!(mode_hit((size[0] - 248. * scale, 18. * scale), size));
            assert!(release_hit((size[0] - 108. * scale, 18. * scale), size));
            assert!(!mode_hit((size[0] - 108. * scale, 18. * scale), size));
        }
        for size in [[640, 480], [1920, 1080], [1080, 1920]] {
            let mut canvas = crate::flight_canvas::FlightCanvas::default();
            canvas.size = size;
            for camera_zoom in [0.5, 1., 2., 4.] {
                let zoom = f64::from(canvas.hud_zoom(camera_zoom));
                let offset = hud::project(0., 0., 3f64.to_radians(), 0., zoom).unwrap().0 - 320.;
                let scale = (f64::from(size[0]) / 640.).min(f64::from(size[1]) / 480.)
                    * crate::flight_canvas::HUD_SCALE;
                let expected = f64::from(size[1]) / 2.
                    * 3f64.sqrt()
                    * 3f64.to_radians().tan()
                    * f64::from(camera_zoom);
                assert!((offset * scale - expected).abs() < 0.001);
            }
        }
        let at = |z| hud::project(0., 0., 3f64.to_radians(), 0., z).unwrap().0 - 320.;
        assert!((at(2.) - 2. * at(1.)).abs() < 1e-9);
    }
}
