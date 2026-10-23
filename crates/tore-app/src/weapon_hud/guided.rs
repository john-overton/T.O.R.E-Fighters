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
//! takes the line. When the aim is in range but masked, MASK stays visible on
//! the readiness row below IN RNG. Layout fitted; the cue is opinionated (John,
//! 2026-10-10, decision 4: "a range indicator for when to drop").
//!
//! A guided bomb in LVL also gets the level release T (slice FX-H; spec
//! "Guided bomb release range indicator"): the status word reads LVL, the
//! crossbar stands across the flight path marker, the stem runs to the aim's
//! bearing and the release marker slides down it as the aim closes on the
//! bar `REL m:ss` away. A held trigger releases at the bar.
use super::bombs::{self, Tee};
use crate::{frame::FlightFrame, hud::Paint};
use tore_formats::{font::Font, text::GlyphCodes, weapons::Weapon};
use tore_sim::combat::{
    guided::{Advisory, GuidedSight},
    live,
    missiles::NMI,
};

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
    zoom: f64,
) {
    let ro = &frame.readout;
    let s = frame.presented();
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
    paint.text(
        font,
        if sight.level.is_some() { "LVL" } else { "ARM" },
        207,
        259,
    );
    paint.text(
        font,
        &format!("{} {}", ro.stores.rounds(ro.stores.selected()), w.hud_name),
        207,
        271,
    );
    // True when row 283 shows the advisory, so row 295 need not repeat it.
    let mut advisory_above = false;
    let to_bar = sight
        .level
        .and(sight.release_seconds())
        .filter(|seconds| *seconds > 0.);
    if sight.level.is_some() && sight.overflown() {
        paint.text(font, "OVERFLY", 207, 283);
    } else if let Some(seconds) = to_bar {
        paint.text(font, &format!("REL {}", bombs::clock(seconds)), 207, 283);
    } else if sight.in_reach {
        if ro.sensors.tick % 60 < 30 {
            paint.text(font, "IN RNG", 207, 283);
        }
    } else if let Some(advisory) = sight.advisory {
        paint.text(font, advisory.label(), 207, 283);
        advisory_above = true;
    }
    if sight.level.is_some()
        && ro.stores.rounds(ro.stores.selected()) > 0
        && let Some(fpm) = bombs::flight_path_marker(s, zoom)
        && let Some(level) = sight.level
    {
        let tee = Tee {
            steering_rad: level.steering_rad,
            marker: sight.marker_fraction(),
            far: sight.far(),
            overflown: sight.overflown(),
        };
        bombs::draw_level(paint, s, zoom, Some(fpm), &tee);
    }
    let ready = ro.estimates.readiness;
    if let Some(warning) = additional_warning(ready, advisory_above, sight.advisory) {
        paint.text(font, warning, 207, 295);
    }
}

/// A range cue, a release timer and a visibility warning are independent.
/// Keep release failures first and avoid repeating an advisory already drawn
/// above (`advisory_above`); an advisory that row 283 gave way to (IN RNG,
/// REL or OVERFLY) is shown here instead.
fn additional_warning(
    ready: live::Readiness,
    advisory_above: bool,
    advisory: Option<Advisory>,
) -> Option<&'static str> {
    let repeats = advisory_above && advisory.is_some_and(|a| a.label() == ready.label());
    if !matches!(
        ready,
        live::Readiness::Ready | live::Readiness::TargetDestroyed
    ) && !repeats
    {
        Some(ready.label())
    } else if advisory_above {
        None
    } else {
        advisory.map(Advisory::label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_remains_visible_beside_an_energy_only_in_range_cue() {
        assert_eq!(
            additional_warning(live::Readiness::Ready, false, Some(Advisory::Masked)),
            Some("MASK")
        );
        assert_eq!(
            additional_warning(live::Readiness::Ready, true, Some(Advisory::Masked)),
            None,
            "the main row already shows MASK"
        );
        assert_eq!(
            additional_warning(live::Readiness::BayClosed, false, Some(Advisory::Masked)),
            Some("OPENING BAY")
        );
        assert_eq!(
            additional_warning(live::Readiness::NoTarget, true, Some(Advisory::NoTarget)),
            None,
            "do not repeat NO TARGET on both rows"
        );
        // Row 283 shows REL or OVERFLY (no advisory above): MASK is not lost.
        assert_eq!(
            additional_warning(live::Readiness::Ready, false, Some(Advisory::Masked)),
            Some("MASK")
        );
        assert_eq!(
            additional_warning(live::Readiness::Ready, false, None),
            None
        );
    }
}
