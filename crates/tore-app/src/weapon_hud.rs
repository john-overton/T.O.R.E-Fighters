//! Manual-style weapon symbols with fitted layout and simulation-owned state.
use crate::{
    combat, flight,
    hud::{self, Paint},
    terrain::Camera,
};
use tore_formats::font::Font;
use tore_sim::{
    attitude::{Basis, Vector, dot},
    combat::{
        live,
        missiles::{self, Guidance, LaunchMode, seeker::Status},
    },
};

pub fn mode_hit(point: (f64, f64), size: [f64; 2]) -> bool {
    let scale = (size[0] / 640.).min(size[1] / 480.) * crate::flight_canvas::HUD_SCALE;
    let x = (point.0 - (size[0] - 640. * scale) / 2.) / scale;
    let y = (point.1 - (size[1] - 480. * scale) / 2.) / scale;
    (178. ..320.).contains(&x) && (99. ..114.).contains(&y)
}
fn projected(direction: Vector, camera: &Camera, zoom: f64) -> Option<(f64, f64)> {
    let bearing = direction[0].atan2(direction[2]) - f64::from(camera.yaw);
    let elevation = direction[1].atan2(direction[0].hypot(direction[2]));
    hud::project(
        f64::from(camera.pitch),
        -f64::from(camera.roll),
        bearing,
        elevation,
        zoom,
    )
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
    camera: &Camera,
    color: [u8; 3],
    zoom: f64,
) {
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
    // Fitted translucent backing keeps weapon details readable when the
    // original cockpit HUD and bank scale are magnified by camera zoom.
    paint.color = [0, 0, 0, 230];
    paint.rect(176, 337, 288, 73);
    paint.rect(176, 98, 148, 33);
    paint.color = [color[0], color[1], color[2], 255];
    let mode = if state.weapon_rules == missiles::Rules::Compatibility {
        "COMPATIBILITY"
    } else {
        state.launch_mode.label()
    };
    paint.text(font, mode, 178, 101);
    let status = match (profile.guidance, state.mounted.status) {
        (Guidance::Infrared, Status::Locked) => "IR LOCK",
        (_, Status::Pitbull) => "RADAR LOCK",
        (_, Status::Search) => "SEARCH",
        (_, status) => status.label(),
    };
    paint.text(font, status, 178, 115);
    let cap = if state.launch_mode == LaunchMode::Boresight && !state.mounted.acquired {
        profile.search_cap()
    } else {
        std::f64::consts::PI
    };
    let zone = &w.seeker.zones[0];
    let cone = boundary(
        l.basis,
        missiles::half_angle(zone.heading).min(cap),
        missiles::half_angle(zone.pitch).min(cap),
    );
    for pair in cone.windows(2) {
        if let (Some(a), Some(b)) = (
            projected(pair[0], camera, zoom),
            projected(pair[1], camera, zoom),
        ) {
            paint.line(a, b);
        }
    }
    if let Some((x, y)) = projected(l.basis.forward, camera, zoom) {
        for i in 0..32 {
            let a = f64::from(i) * std::f64::consts::TAU / 32.;
            let b = f64::from(i + 1) * std::f64::consts::TAU / 32.;
            paint.line(
                (x + 9. * a.cos(), y + 9. * a.sin()),
                (x + 9. * b.cos(), y + 9. * b.sin()),
            );
        }
    }
    let observed = state
        .mounted
        .observation
        .map(|o| (o.position, o.velocity))
        .or_else(|| {
            state
                .designated()
                .filter(|_| state.launch_mode == LaunchMode::Cued)
                .and_then(|id| state.sensors.observation(id))
                .map(|c| (c.position, c.velocity))
        });
    if let Some((position, velocity)) = observed {
        if let Some((x, y)) = projected(
            missiles::sub(position, camera.position.map(f64::from)),
            camera,
            zoom,
        ) {
            for (a, b) in [
                ((-7., -7.), (7., -7.)),
                ((7., -7.), (7., 7.)),
                ((7., 7.), (-7., 7.)),
                ((-7., 7.), (-7., -7.)),
            ] {
                paint.line((x + a.0, y + a.1), (x + b.0, y + b.1));
            }
            if matches!(state.mounted.status, Status::Locked | Status::Pitbull) {
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
        let closing = missiles::closure(s.position, s.velocity, position, velocity) / 1.68781;
        paint.text(
            font,
            &format!("R {:.1} C {closing:+.0}", range / missiles::NMI),
            178,
            352,
        );
        let min = f64::from(w.seeker.zones[1].minimum_range);
        let max = f64::from(w.seeker.zones[1].maximum_range);
        paint.line((451., 178.), (451., 257.));
        paint.line((445., 178.), (451., 178.));
        paint.line((445., 257.), (451., 257.));
        paint.text(font, &format!("{:.1}", max / missiles::NMI), 420, 166);
        paint.text(font, &format!("{:.1}", min / missiles::NMI), 420, 259);
        if (min..=max).contains(&range) && max > min {
            let y = 257. - 79. * (range - min) / (max - min);
            paint.line((441., y - 4.), (447., y));
            paint.line((447., y), (441., y + 4.));
        }
        let aspect = dot(
            tore_sim::attitude::unit(velocity),
            tore_sim::attitude::unit(missiles::sub(s.position, position)),
        )
        .clamp(-1., 1.)
        .acos()
        .to_degrees();
        if missiles::length(velocity) > 1e-9 {
            paint.text(font, &format!("ASP {aspect:.0}"), 368, 352);
        }
        match state.mounted_solution(l) {
            Some(solution) => paint.text(font, &format!("EST {:.1}S", solution.seconds), 178, 364),
            None => paint.text(font, "NO SOLUTION", 178, 364),
        }
    } else {
        paint.text(font, "R -- C -- EST --", 178, 352);
    }
    paint.text(
        font,
        &format!("{} {}", w.name, state.rounds(state.selected)),
        178,
        340,
    );
    let ready = state.readiness(l);
    let permission =
        if ready == live::Readiness::Ready && state.launch_mode == LaunchMode::Boresight {
            "BORESIGHT READY"
        } else if ready == live::Readiness::Ready {
            "IN RNG"
        } else {
            ready.label()
        };
    paint.text(font, permission, 300, 340);
    paint.text(font, "P HIT --", 368, 364);
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
        let motor = match missiles::phase(
            &state.configuration().stations[shot.station].weapon.movement,
            shot.age,
        ) {
            tore_sim::combat::EnginePhase::BeforeIgnition => "WAIT",
            tore_sim::combat::EnginePhase::Powered => "BURN",
            tore_sim::combat::EnginePhase::Coast => "COAST",
        };
        paint.text(
            font,
            &format!(
                "#{} {} {} {motor} {remaining:.0}S",
                shot.id,
                state.configuration().stations[shot.station].weapon.name,
                if f.seeker.status == Status::Search && f.profile.guidance != Guidance::Active {
                    "SEARCH"
                } else {
                    f.seeker.status.label()
                }
            ),
            178,
            376 + row as i32 * 10,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
            let scale = (size[0] / 640f64).min(size[1] / 480.) * crate::flight_canvas::HUD_SCALE;
            assert!(mode_hit(
                (
                    (size[0] - 640. * scale) / 2. + 200. * scale,
                    (size[1] - 480. * scale) / 2. + 105. * scale
                ),
                size
            ));
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
