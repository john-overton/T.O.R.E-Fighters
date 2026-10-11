//! The HUD for a selected guided air-to-ground weapon: the release range
//! scale (docs/spec/air-to-ground.md, "Guided bomb release range
//! indicator"), its range readout and the advisories (slice W1). The laser
//! box, `LSR` and `TTI` belong to the HUD laser cues (slice P1).
//!
//! The scale is the missiles' range scale just inside the altitude tape
//! (x = 390, y = 230 to 282, fitted to the retail reference): the weapon's
//! reach from here, the farthest ground range at the top and the nearest at
//! the bottom, in nautical miles, with a caret at the aim's ground range and
//! `R 6.2` above it. IN RNG blinks at 2 Hz while the aim lies inside the
//! reach; otherwise the advisory (NO LASER, MASK, OUT OF REACH, NO TARGET)
//! takes the line. Layout fitted; the cue is opinionated (John,
//! 2026-10-10, decision 4: "a range indicator for when to drop").
use crate::{frame::FlightFrame, hud::Paint};
use tore_formats::{font::Font, text::GlyphCodes, weapons::Weapon};
use tore_sim::combat::{guided::GuidedSight, live, missiles::NMI};

/// The scale's x and its top and bottom, reference pixels.
const SCALE_X: i32 = 390;
const TOP: f64 = 230.;
const BOTTOM: f64 = 282.;

fn width(font: &Font, text: &str) -> i32 {
    text.glyph_codes()
        .map(|c| font.glyphs[c as usize].advance)
        .sum::<usize>() as i32
}

/// Where a ground range sits on the scale, clamped to it.
pub(super) fn scale_y(sight: &GuidedSight, range_ft: f64) -> Option<f64> {
    let reach = sight.reach?;
    (reach.maximum_ft > reach.minimum_ft)
        .then(|| BOTTOM - (BOTTOM - TOP) * reach.fraction(range_ft))
}

pub(super) fn draw(
    paint: &mut Paint<'_>,
    frame: &FlightFrame,
    font: &Font,
    w: &Weapon,
    sight: &GuidedSight,
) {
    let ro = &frame.readout;
    let x = f64::from(SCALE_X);
    paint.line((x, TOP), (x, BOTTOM));
    paint.line((x - 5., TOP), (x, TOP));
    paint.line((x - 5., BOTTOM), (x, BOTTOM));
    let (maximum, minimum) = match sight.reach {
        Some(reach) => (
            format!("{:.1}", reach.maximum_ft / NMI),
            format!("{:.1}", reach.minimum_ft / NMI),
        ),
        None => ("--".into(), "--".into()),
    };
    paint.text(font, &maximum, SCALE_X - width(font, &maximum), 218);
    paint.text(font, &minimum, SCALE_X - width(font, &minimum), 284);
    if let Some(range) = sight.ground_range_ft {
        let text = format!("R {:.1}", range / NMI);
        paint.text(font, &text, SCALE_X - width(font, &text), 206);
        if let Some(y) = scale_y(sight, range) {
            paint.line((x - 8., y - 3.), (x - 2., y));
            paint.line((x - 2., y), (x - 8., y + 3.));
            paint.line((x - 8., y + 3.), (x - 8., y - 3.));
        }
    }
    paint.text(font, "ARM", 207, 259);
    paint.text(
        font,
        &format!("{} {}", ro.stores.rounds(ro.stores.selected()), w.hud_name),
        207,
        271,
    );
    if sight.in_reach {
        if ro.sensors.tick % 60 < 30 {
            paint.text(font, "IN RNG", 207, 283);
        }
    } else if let Some(advisory) = sight.advisory {
        paint.text(font, advisory.label(), 207, 283);
    }
    let ready = ro.estimates.readiness;
    if !matches!(
        ready,
        live::Readiness::Ready | live::Readiness::TargetDestroyed
    ) && sight.advisory.is_none_or(|a| a.label() != ready.label())
    {
        paint.text(font, ready.label(), 207, 295);
    }
}
