//! The laser on the HUD (docs/spec/air-to-ground.md, "Laser on the HUD";
//! plan 4.7). Opinionated (John, 2026-10-10: the HUD shows when the laser is
//! on, with a range indicator, and shows when the spot is someone else's).
//!
//! The spot is marked with the rectangular laser box (the aim box's outline,
//! 12 by 8 pixels) and a small letter beside it: **L** for the aircraft's own
//! spot, **B** for a buddy's. Off the HUD the mark becomes the aim box's edge
//! arrow. The weapon text gains `LSR 4.3` (laser range in nautical miles,
//! blinking twice a second while lasing), `LSR RDY`, `MASK`, and `BUD 4.3`
//! for a buddy's spot; the time to impact takes the line below when the
//! weapon slice supplies it.
use super::{TargetCue, target_cue};
use crate::{
    aim_box, flight,
    hud::{self, Paint},
};
use tore_formats::font::Font;
use tore_sim::{
    attitude::Basis,
    combat::{laser::LaserState, missiles},
};
use tore_world::readout::{CockpitReadout, LaserReadout, SpotReadout};

/// The text column's left edge (the weapon text's), and the first row below
/// the readiness label (rows at 259, 271, 283, 295).
const TEXT_X: i32 = 207;
const FIRST_ROW: i32 = 307;
const ROW: i32 = 12;
/// The blink: twice a second, lit the first half of each combat-tick half
/// second (120 ticks a second).
const BLINK_TICKS: u64 = 30;

/// A line of laser weapon text and whether it blinks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub blinks: bool,
}

fn nautical(feet: f64) -> String {
    format!("{:.1}", feet / missiles::NMI)
}

/// The text lines for a laser readout, own laser first, a buddy's after
/// it. Empty when the laser has nothing to say (no designator and no buddy
/// spot, or the laser is off with no spot).
pub fn lines(laser: &LaserReadout) -> Vec<Line> {
    let mut lines = Vec::new();
    if laser.designator {
        match laser.state {
            LaserState::Lasing => lines.push(Line {
                text: format!("LSR {}", nautical(laser.range_ft)),
                blinks: true,
            }),
            LaserState::Ready => lines.push(Line {
                text: "LSR RDY".into(),
                blinks: false,
            }),
            LaserState::Masked => lines.push(Line {
                text: "MASK".into(),
                blinks: false,
            }),
            LaserState::Off => {}
        }
    }
    // The spot a laser weapon homes on, when it is a buddy's, and a buddy's
    // beside the plane's own.
    let buddy: Option<&SpotReadout> = laser
        .spot
        .as_ref()
        .filter(|spot| !spot.own)
        .or(laser.buddy.as_ref());
    if let Some(spot) = buddy {
        lines.push(Line {
            text: format!("BUD {}", nautical(spot.range_ft)),
            blinks: true,
        });
    }
    lines
}

/// Whether a blinking line is lit at combat tick `tick`.
pub fn lit(tick: u64) -> bool {
    tick % (2 * BLINK_TICKS) < BLINK_TICKS
}

/// The spots to mark: each with the letter it wears.
pub fn marks(laser: &LaserReadout) -> Vec<(&SpotReadout, &'static str)> {
    let mut marks = Vec::new();
    for spot in laser.spot.iter().chain(laser.buddy.iter()) {
        marks.push((spot, if spot.own { "L" } else { "B" }));
    }
    marks
}

/// The time to impact row (opinionated, John, 2026-10-11: "time to impact on
/// hud"; layout agent): the plane's oldest guided weapon in flight whose time
/// is known, `TTI 0:12`, counting down, with `X3` after it when several fly
/// (the readout lists up to four); `IMPACT` for a second after the last one
/// ended. The same slot as the targeting page's `TTI` row.
pub fn tti_line(ro: &CockpitReadout) -> Option<String> {
    let tti = crate::instruments::targeting::tti_of(ro)?;
    Some(tti_row(tti, ro.weapon_tti.len()))
}

/// The row's text for the weapon shown and how many fly.
pub fn tti_row(tti: crate::instruments::targeting::Tti, flying: usize) -> String {
    let text = crate::instruments::targeting::tti_text(tti);
    if flying > 1 && !tti.impact {
        format!("{text} X{flying}")
    } else {
        text
    }
}

/// The row for `--hud-snapshot-state tti`, `tti-lsr` (under the laser rows),
/// `tti-many` and `tti-impact`.
pub fn tti_preview(state: &str) -> Option<String> {
    use crate::instruments::targeting::Tti;
    let at = |seconds, impact| Tti { seconds, impact };
    Some(match state {
        "tti" | "tti-lsr" => tti_row(at(72., false), 1),
        "tti-many" => tti_row(at(41.2, false), 3),
        "tti-impact" => tti_row(at(0., true), 0),
        _ => return None,
    })
}

/// Draws the laser's marks and text, and the time to impact row, on the HUD
/// pixels.
pub fn draw(
    pixels: &mut [u8],
    s: &flight::State,
    ro: &CockpitReadout,
    font: &Font,
    color: [u8; 3],
    zoom: f64,
) {
    draw_rows(
        pixels,
        s,
        ro.laser.as_ref(),
        ro.tick,
        tti_line(ro).as_deref(),
        font,
        color,
        zoom,
    );
}

/// [`draw`] from its parts: the laser readout, the combat tick and the time
/// to impact text.
#[allow(clippy::too_many_arguments)]
pub fn draw_rows(
    pixels: &mut [u8],
    s: &flight::State,
    laser: Option<&LaserReadout>,
    tick: u64,
    tti: Option<&str>,
    font: &Font,
    color: [u8; 3],
    zoom: f64,
) {
    let mut paint = Paint {
        pixels,
        clip: (174, 96, 292, hud::AIM_BOTTOM - 96),
        color: [color[0], color[1], color[2], 255],
    };
    let Some(laser) = laser else {
        paint.clip = hud::HUD_CLIP;
        if let Some(text) = tti {
            paint.text(font, text, TEXT_X, FIRST_ROW);
        }
        return;
    };
    for (spot, letter) in marks(laser) {
        let Some(cue) = target_cue(
            missiles::sub(spot.spot.position, s.position),
            Basis::new(s.yaw, s.pitch, s.bank),
            zoom,
        ) else {
            continue;
        };
        match cue {
            TargetCue::Square((x, y)) => {
                for (a, b) in aim_box::laser_box_strokes() {
                    paint.line((x + a.0, y + a.1), (x + b.0, y + b.1));
                }
                let (w, h) = aim_box::LASER_HALF;
                paint.text(
                    font,
                    letter,
                    (x + w + 3.).round() as i32,
                    (y - h).round() as i32 - 1,
                );
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
    paint.clip = hud::HUD_CLIP;
    for (row, line) in lines(laser).iter().enumerate() {
        if line.blinks && !lit(tick) {
            continue;
        }
        paint.text(font, &line.text, TEXT_X, FIRST_ROW + row as i32 * ROW);
    }
    // The time to impact stands under the laser rows (all of them, lit or
    // not in the blink, so it does not jump).
    if let Some(text) = tti {
        paint.text(
            font,
            text,
            TEXT_X,
            FIRST_ROW + lines(laser).len() as i32 * ROW,
        );
    }
}

/// A synthetic laser readout for `--hud-snapshot-state laser-*`, placed
/// ahead of the aircraft `s` (heading and position as the snapshot's flight
/// state has them).
pub fn preview(state: &str, s: &flight::State) -> Option<LaserReadout> {
    use tore_sim::combat::{laser::LaserSpot, live::Side};
    let ahead = |feet: f64, right: f64, down: f64| {
        [
            s.position[0] + right,
            s.position[1] - down,
            s.position[2] + feet,
        ]
    };
    let spot = |own: bool, position, feet: f64| SpotReadout {
        spot: LaserSpot {
            owner: if own { 0 } else { 5 },
            side: Side(1),
            position,
            target: None,
        },
        own,
        range_ft: feet,
    };
    let near = spot(true, ahead(26_000., 0., 3_000.), 26_200.);
    let side = spot(false, ahead(31_000., 1_800., 3_000.), 31_300.);
    let base = LaserReadout {
        designator: true,
        state: LaserState::Lasing,
        mask: None,
        manual: None,
        range_ft: 26_200.,
        limit_ft: 60_760.,
        spot: Some(near),
        buddy: None,
    };
    Some(match state {
        "laser-lsr" => base,
        "laser-ready" => LaserReadout {
            state: LaserState::Ready,
            spot: None,
            ..base
        },
        "laser-mask" => LaserReadout {
            state: LaserState::Masked,
            mask: Some(tore_sim::combat::laser::Mask::Terrain),
            spot: None,
            ..base
        },
        // A buddy's spot on an aircraft with no designator.
        "laser-bud" => LaserReadout {
            designator: false,
            state: LaserState::Off,
            spot: Some(side),
            range_ft: 0.,
            limit_ft: 0.,
            ..base
        },
        "laser-both" => LaserReadout {
            buddy: Some(side),
            ..base
        },
        // A spot well off the HUD to the right: the edge arrow.
        "laser-edge" => LaserReadout {
            spot: Some(spot(true, ahead(8_000., 9_000., 3_000.), 12_000.)),
            ..base
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::combat::{laser::LaserSpot, live::Side};

    fn readout(state: LaserState, designator: bool) -> LaserReadout {
        LaserReadout {
            designator,
            state,
            mask: None,
            manual: None,
            range_ft: 26_200.,
            limit_ft: 60_760.,
            spot: None,
            buddy: None,
        }
    }
    fn buddy(feet: f64) -> SpotReadout {
        SpotReadout {
            spot: LaserSpot {
                owner: 3,
                side: Side(1),
                position: [0., 0., 10_000.],
                target: None,
            },
            own: false,
            range_ft: feet,
        }
    }
    fn text(lines: &[Line]) -> Vec<&str> {
        lines.iter().map(|l| l.text.as_str()).collect()
    }

    #[test]
    fn the_text_names_each_laser_state() {
        assert_eq!(
            text(&lines(&readout(LaserState::Lasing, true))),
            ["LSR 4.3"]
        );
        assert_eq!(text(&lines(&readout(LaserState::Ready, true))), ["LSR RDY"]);
        assert_eq!(text(&lines(&readout(LaserState::Masked, true))), ["MASK"]);
        assert!(lines(&readout(LaserState::Off, true)).is_empty());
    }

    #[test]
    fn only_lasing_and_a_buddy_blink() {
        assert!(lines(&readout(LaserState::Lasing, true))[0].blinks);
        assert!(!lines(&readout(LaserState::Ready, true))[0].blinks);
        assert!(!lines(&readout(LaserState::Masked, true))[0].blinks);
        let mut laser = readout(LaserState::Off, false);
        laser.spot = Some(buddy(31_300.));
        assert!(lines(&laser)[0].blinks);
    }

    #[test]
    fn a_buddys_spot_reads_bud_and_wears_a_b() {
        let mut laser = readout(LaserState::Off, false);
        laser.spot = Some(buddy(31_300.));
        assert_eq!(text(&lines(&laser)), ["BUD 5.2"]);
        assert_eq!(
            marks(&laser).iter().map(|(_, l)| *l).collect::<Vec<_>>(),
            ["B"]
        );
    }

    #[test]
    fn an_own_lase_and_a_buddy_give_two_rows_and_two_boxes() {
        let mut laser = readout(LaserState::Lasing, true);
        laser.spot = Some(SpotReadout {
            own: true,
            ..buddy(26_200.)
        });
        laser.buddy = Some(buddy(31_300.));
        assert_eq!(text(&lines(&laser)), ["LSR 4.3", "BUD 5.2"]);
        assert_eq!(
            marks(&laser).iter().map(|(_, l)| *l).collect::<Vec<_>>(),
            ["L", "B"]
        );
    }

    #[test]
    fn the_blink_runs_twice_a_second() {
        let lit_ticks = (0..120).filter(|tick| lit(*tick)).count();
        assert_eq!(lit_ticks, 60);
        assert!(lit(0) && !lit(30) && lit(60) && !lit(90));
    }

    #[test]
    fn the_laser_box_is_twelve_by_eight() {
        let strokes = aim_box::laser_box_strokes();
        let xs: Vec<f64> = strokes.iter().flat_map(|(a, b)| [a.0, b.0]).collect();
        let ys: Vec<f64> = strokes.iter().flat_map(|(a, b)| [a.1, b.1]).collect();
        let span = |v: &[f64]| {
            v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert_eq!((span(&xs), span(&ys)), (12., 8.));
    }

    fn font() -> Font {
        Font {
            height: 8,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 6,
                    pixels: vec![(0, 0), (1, 0), (0, 1), (1, 1)],
                })
                .collect(),
        }
    }
    fn lit_pixels(pixels: &[u8], rect: (usize, usize, usize, usize)) -> usize {
        let (x, y, w, h) = rect;
        (y..y + h)
            .flat_map(|yy| (x..x + w).map(move |xx| (xx, yy)))
            .filter(|(xx, yy)| pixels[(yy * 640 + xx) * 4 + 3] != 0)
            .count()
    }
    fn render(state: &str, tick: u64) -> Vec<u8> {
        let profile = tore_world::test_support::profile();
        let mut flight = flight::State::new(&profile, [0., 3000., 0.]).unwrap();
        flight.yaw = 0.;
        flight.pitch = 0.;
        flight.bank = 0.;
        let laser = preview(
            if state == "tti-lsr" {
                "laser-lsr"
            } else {
                state
            },
            &flight,
        );
        let mut pixels = vec![0; 640 * 480 * 4];
        draw_rows(
            &mut pixels,
            &flight,
            laser.as_ref(),
            tick,
            tti_preview(state).as_deref(),
            &font(),
            [0, 255, 0],
            1.,
        );
        pixels
    }

    #[test]
    fn the_tti_row_reads_the_oldest_weapon_with_a_count_and_impact() {
        use crate::instruments::targeting::Tti;
        let at = |seconds, impact| Tti { seconds, impact };
        assert_eq!(tti_row(at(72., false), 1), "TTI 1:12");
        assert_eq!(tti_row(at(12.4, false), 0), "TTI 0:13");
        assert_eq!(tti_row(at(41.2, false), 3), "TTI 0:42 X3");
        assert_eq!(tti_row(at(0., true), 3), "IMPACT");
        assert_eq!(tti_preview("tti-many").as_deref(), Some("TTI 0:42 X3"));
        assert_eq!(tti_preview("laser-lsr"), None);
    }

    #[test]
    fn the_readouts_weapons_in_flight_make_the_row() {
        let state = tore_world::test_support::combat_fixture(false);
        let flight =
            flight::State::new(&tore_world::test_support::profile(), [0., 5000., 0.]).unwrap();
        let mut ro =
            tore_world::readout::build(&state, 0, crate::combat::launcher(&flight), None, None)
                .unwrap();
        assert_eq!(tti_line(&ro), None);
        let weapon = |seconds| tore_world::readout::WeaponTti {
            projectile: 1,
            station: 0,
            kind: missiles::Guidance::Laser,
            guiding: true,
            seconds,
            position: [0.; 3],
        };
        ro.weapon_tti = vec![weapon(None), weapon(Some(75.)), weapon(Some(20.))];
        assert_eq!(tti_line(&ro).as_deref(), Some("TTI 1:15 X3"));
        ro.weapon_tti.truncate(2);
        assert_eq!(tti_line(&ro).as_deref(), Some("TTI 1:15 X2"));
        ro.weapon_tti.clear();
        ro.tick = 1_000;
        ro.guided_impact_tick = Some(950);
        assert_eq!(tti_line(&ro).as_deref(), Some("IMPACT"));
    }

    #[test]
    fn the_tti_row_takes_the_first_row_alone_and_stands_under_the_laser_rows() {
        let alone = render("tti", 0);
        assert!(lit_pixels(&alone, (207, 307, 60, 8)) > 0);
        assert_eq!(lit_pixels(&alone, (207, 319, 60, 8)), 0);
        // With the laser lit the LSR row is first and the TTI row second,
        // and it does not blink with the laser's text.
        for tick in [0, 40] {
            let both = render("tti-lsr", tick);
            assert!(lit_pixels(&both, (207, 319, 60, 8)) > 0, "tick {tick}");
        }
        assert_eq!(lit_pixels(&render("tti-lsr", 40), (207, 307, 60, 8)), 0);
        // No weapon and no impact: nothing on the row.
        assert_eq!(lit_pixels(&render("forward", 0), (207, 307, 60, 24)), 0);
    }

    #[test]
    fn the_hud_draws_the_box_and_its_text_and_the_text_blinks() {
        // The text column: rows at 307 and 319 from x 207.
        let text = (207, 307, 60, 8);
        let lit = render("laser-lsr", 0);
        assert!(lit_pixels(&lit, text) > 0);
        assert!(lit_pixels(&lit, (174, 96, 292, 294)) > 0, "the box");
        // The dark half of the blink drops the text but keeps the box.
        let dark = render("laser-lsr", 40);
        assert_eq!(lit_pixels(&dark, text), 0);
        assert!(lit_pixels(&dark, (174, 96, 292, 200)) > 0);
        // LSR RDY and MASK do not blink and draw no box.
        for state in ["laser-ready", "laser-mask"] {
            for tick in [0, 40] {
                let pixels = render(state, tick);
                assert!(lit_pixels(&pixels, text) > 0, "{state} {tick}");
                assert_eq!(lit_pixels(&pixels, (174, 96, 292, 200)), 0, "{state}");
            }
        }
    }

    #[test]
    fn a_spot_off_the_hud_becomes_an_edge_arrow() {
        let pixels = render("laser-edge", 0);
        // The arrow sits on the HUD's right edge, clear of the box region.
        assert!(lit_pixels(&pixels, (440, 96, 30, 294)) > 0);
    }

    #[test]
    fn every_preview_state_builds() {
        let profile = tore_world::test_support::profile();
        let flight = flight::State::new(&profile, [0., 3000., 0.]).unwrap();
        for state in [
            "laser-lsr",
            "laser-ready",
            "laser-mask",
            "laser-bud",
            "laser-both",
            "laser-edge",
        ] {
            assert!(preview(state, &flight).is_some(), "{state}");
        }
        assert!(preview("laser-none", &flight).is_none());
    }
}
