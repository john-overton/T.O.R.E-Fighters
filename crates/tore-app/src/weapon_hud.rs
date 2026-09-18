//! Manual-style weapon symbols with fitted layout and simulation-owned state.
use crate::{
    combat, flight,
    hud::{self, Paint},
};
use tore_formats::font::Font;
use tore_sim::{
    attitude::{Basis, Vector, dot},
    combat::{
        live,
        missiles::{self, Guidance, LaunchMode, seeker::Status},
    },
};

pub fn active(state: &live::State) -> bool {
    state.armed
        && missiles::Profile::for_weapon(&state.configuration().stations[state.selected].weapon)
            .is_some()
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
) {
    if !state.armed
        && combat::launcher(s).radar
        && state.sensors.operating(tore_sim::sensors::Channel::Radar)
        && let Some(contact) = state.designated().and_then(|id| state.sensors.contact(id))
        && contact.channel == tore_sim::sensors::Channel::Radar
        && let Some((x, y)) = projected(missiles::sub(contact.position, s.position), s, zoom)
    {
        let mut paint = Paint {
            pixels,
            clip: (174, 96, 292, 325),
            color: [color[0], color[1], color[2], 255],
        };
        for (a, b) in [
            ((-7., -7.), (7., -7.)),
            ((7., -7.), (7., 7.)),
            ((7., 7.), (-7., 7.)),
            ((-7., 7.), (-7., -7.)),
        ] {
            paint.line((x + a.0, y + a.1), (x + b.0, y + b.1));
        }
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
        clip: (174, 96, 292, 325),
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
            if !bore {
                for (a, b) in [
                    ((-7., -7.), (7., -7.)),
                    ((7., -7.), (7., 7.)),
                    ((7., 7.), (-7., 7.)),
                    ((-7., 7.), (-7., -7.)),
                ] {
                    paint.line((x + a.0, y + a.1), (x + b.0, y + b.1));
                }
            }
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
    paint.text(font, "ARM", 207, 279);
    paint.text(
        font,
        &format!("{} {}", state.rounds(state.selected), w.hud_name),
        207,
        291,
    );
    let ready = state.readiness(l);
    let percent = format!("{}%", state.estimated_hit_percent(l));
    paint.text(font, &percent, 207, 306);
    if in_range && state.sensors.tick() % 60 < 30 {
        let width: usize = percent
            .bytes()
            .map(|c| font.glyphs[c as usize].advance)
            .sum();
        paint.text(font, "IN RNG", 211 + width as i32, 306);
    } else if !matches!(
        ready,
        live::Readiness::Ready | live::Readiness::TargetDestroyed
    ) {
        paint.text(font, ready.label(), 207, 321);
    }
    if matches!(profile.guidance, Guidance::Active | Guidance::Supported)
        && let Some(o) = observed
    {
        let closure = missiles::closure(s.position, s.velocity, o.position, o.velocity) / 1.68781;
        paint.text(font, &format!("R {:.1}", o.range / missiles::NMI), 402, 291);
        paint.text(font, &format!("C {closure:+.0}"), 402, 303);
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
        paint.text(font, &aspect, 402, 315);
    }
}
/// Diagnostic state and controls are composed at the window's upper right.
pub fn debug(
    pixels: &mut [u8],
    state: &live::State,
    s: &flight::State,
    font: &Font,
    color: [u8; 3],
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
        if state.weapon_rules == missiles::Rules::Compatibility {
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
