//! The HUD cues of the air-to-ground missiles (slice W2,
//! docs/spec/air-to-ground.md, "Missiles and the HARM boresight cue").
//!
//! - **HARM boresight cue** (opinionated, John, 2026-10-10: "act kind of like
//!   boresite on a2a weapons where it shows a radiation source you can shoot
//!   at"). While an anti-radiation missile is armed every emitter its seeker
//!   receives inside its cone is drawn as the RWR's ground square at its
//!   direction: a square with a centre dot when its radar paints the plane
//!   (bright on the RWR), only its corners once its radar has gone off (dim).
//!   The candidate the trigger fires at carries the weapon diamond, which
//!   blinks in BORE (the shared missile code draws it on the readout's
//!   observation), and the mode word reads BORE or CUED above the bore ring.
//!   Layout fitted.
//! - **Maverick boresight** (opinionated, John accepted plan decision 10):
//!   an infrared air-to-ground missile in BORE draws exactly what an
//!   air-to-air missile in BORE draws, the 5 degree circle, BORE and the
//!   blinking diamond on the hottest ground object in it, through the shared
//!   code (`Profile::supports_boresight`).
use super::projected;
use crate::{
    frame::FlightFrame,
    hud::{self, Paint},
};
use tore_formats::font::Font;
use tore_sim::combat::missiles::{self, Profile};

/// Half the ground square's side, reference pixels (fitted to the RWR's
/// 7 pixel square).
const SQUARE: f64 = 3.5;

/// An anti-radiation surface missile: the HARM cue replaces the bore circle.
pub(super) fn is_harm(profile: Profile) -> bool {
    profile.holds_on_angles()
}

/// The HARM cue's emitters and its mode word; `ring` is where the bore ring
/// sits on the HUD.
pub(super) fn draw_harm(
    paint: &mut Paint<'_>,
    frame: &FlightFrame,
    font: &Font,
    zoom: f64,
    ring: Option<(f64, f64)>,
) {
    let ro = &frame.readout;
    let Some(cue) = &ro.estimates.harm else {
        return;
    };
    let s = frame.presented();
    for emitter in &cue.emitters {
        let Some((x, y)) = projected(missiles::sub(emitter.position, s.position), s, zoom) else {
            continue;
        };
        if !inside(x, y) {
            continue;
        }
        square(paint, (x, y), emitter.emitting);
        if emitter.painting && emitter.emitting {
            paint.rect(x.round() as i32 - 1, y.round() as i32 - 1, 2, 2);
        }
    }
    if let Some((x, y)) = ring {
        let word = if cue.cued { "CUED" } else { "BORE" };
        paint.text(font, word, x as i32 - 12, y as i32 - 22);
    }
}

/// Whether a HUD point lies in the region the HUD draws weapon cues in.
fn inside(x: f64, y: f64) -> bool {
    (174. ..=466.).contains(&x) && (96. ..=f64::from(hud::AIM_BOTTOM)).contains(&y)
}

/// The RWR's ground square: whole while the emitter emits, only its corners
/// once it has gone off.
fn square(paint: &mut Paint<'_>, (x, y): (f64, f64), emitting: bool) {
    let (l, r, t, b) = (x - SQUARE, x + SQUARE, y - SQUARE, y + SQUARE);
    if emitting {
        paint.line((l, t), (r, t));
        paint.line((r, t), (r, b));
        paint.line((r, b), (l, b));
        paint.line((l, b), (l, t));
    } else {
        let c = 1.5;
        for (cx, cy, dx, dy) in [(l, t, c, c), (r, t, -c, c), (r, b, -c, -c), (l, b, c, -c)] {
            paint.line((cx, cy), (cx + dx, cy));
            paint.line((cx, cy), (cx, cy + dy));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::combat::missiles::{Guidance, TargetRole};

    fn profile(role: TargetRole, guidance: Guidance) -> Profile {
        Profile {
            role,
            guidance,
            activation_ft: (guidance == Guidance::Active).then_some(8. * missiles::NMI),
            guidance_ticks: 1200,
            memory_ticks: missiles::MEMORY,
            radar_emissions: guidance == Guidance::Emitter,
            jammer_emissions: false,
        }
    }

    #[test]
    fn only_an_anti_radiation_surface_missile_takes_the_harm_cue() {
        assert!(is_harm(profile(TargetRole::Surface, Guidance::Emitter)));
        // The Mavericks and the SLAM take the shared BORE circle instead.
        let maverick = profile(TargetRole::Surface, Guidance::Infrared);
        assert!(!is_harm(maverick) && maverick.supports_boresight());
        for (role, guidance) in [
            (TargetRole::Aircraft, Guidance::Infrared),
            (TargetRole::Surface, Guidance::Active),
            (TargetRole::Surface, Guidance::Supported),
        ] {
            assert!(!is_harm(profile(role, guidance)), "{role:?} {guidance:?}");
        }
    }
}
