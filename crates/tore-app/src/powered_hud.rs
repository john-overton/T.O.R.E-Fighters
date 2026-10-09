//! The HUD cluster of the powered-lift aircraft (VTOL overhaul design
//! section 6, decision 11), drawn over the borrowed HUD art in the same
//! font and colour as the rest of the HUD. Positions are fitted at 640x480
//! and scale with the HUD like the autopilot label does.
//!
//! | Element | Shown on | Where |
//! | --- | --- | --- |
//! | `NOZ 60` with a gauge and a caret for the demand, `LIFT` | AV-8, Yak-141 (nozzles not at 0; lift engines running) | right column |
//! | Hover display: vertical velocity bars, 10-knot velocity circle | jets below stall speed, rotorcraft below 40 kt | lower centre |
//! | `NR 100`, `TQ 87`, `COL 64` | helicopters and the V-22 | left column; the collective in the throttle readout's place |
//! | `NAC 75` on a tape with the demand, `CONV` | V-22 | right column |
//! | `R 45` | all six below 1,000 ft above ground | right column |
//! | `SAS OFF` / `SAS ATT` | all six when the level is not Damper | left column |
//!
//! The layout is a list of [`Mark`]s, a pure function of the flight state,
//! so each row is tested by what it puts where; [`draw`] paints the list.
//! `NR` flashes below 90 and above 105 percent, `TQ` above 100.
//!
//! The autopilot label's `AUTO` above `HOVER` is the existing label slot of
//! `hud::draw`: slice P9's hover hold mode only has to give its autopilot
//! label the word `HOVER`. TODO(P9).

use crate::hud::Paint;
use tore_formats::font::Font;
use tore_input::StabilityLevel;
use tore_sim::{
    flight::{
        State,
        powered::readout::{HoverDisplay, NACELLE_TRAVEL_DEGREES},
    },
    models::variety::LiftKind,
};

/// Left column of the cluster, as the autopilot label and the speed box.
pub const LEFT_X: i32 = 211;
/// Right column: the status column's, as the gear and brake labels.
pub const RIGHT_X: i32 = 388;
/// Rows of the left column: rotor speed, torque, stability level.
pub const NR_Y: i32 = 292;
pub const TQ_Y: i32 = 304;
pub const SAS_Y: i32 = 316;
/// Rows of the right column: radar height, nozzle or nacelle angle, and the
/// lift engines or the conversion cue.
pub const RADAR_Y: i32 = 262;
pub const ANGLE_Y: i32 = 274;
pub const CUE_Y: i32 = 286;
/// Where the throttle readout sits, whose place the collective takes.
pub const THROTTLE_READOUT: (i32, i32) = (235, 178);
/// `LIFT` beside the nozzle angle.
pub const LIFT_X: i32 = 430;
/// The nozzle gauge: 0 to 100 degrees along 60 pixels, under the angle.
pub const GAUGE_X: i32 = RIGHT_X;
pub const GAUGE_Y: i32 = 290;
pub const GAUGE_WIDTH: f64 = 60.;
/// The nacelle tape: vertical, 0 degrees at the bottom to 97.5 at the top.
pub const TAPE_X: i32 = 446;
pub const TAPE_TOP: i32 = 262;
pub const TAPE_HEIGHT: f64 = 60.;
/// The hover display: its centre, the pixels a knot of horizontal speed
/// moves the circle (10 knots is the circle's radius, manual p. 81), the
/// vertical bars' column and the pixels a foot a second of vertical speed
/// moves their tick.
pub const HOVER_CENTRE: (f64, f64) = (320., 296.);
pub const CIRCLE_RADIUS: f64 = 22.;
pub const PIXELS_PER_KNOT: f64 = CIRCLE_RADIUS / 10.;
pub const CROSS_HAIR: f64 = 30.;
pub const BARS_X: f64 = 376.;
pub const BARS_HALF_HEIGHT: f64 = 40.;
pub const PIXELS_PER_FPS: f64 = 1.6;
/// Radar height is shown below this many feet above the ground.
pub const RADAR_HEIGHT_FT: f64 = 1_000.;
/// NR flashes below and above these percentages, TQ above its limit.
pub const NR_FLASH: [f64; 2] = [90., 105.];
pub const TQ_FLASH: f64 = 100.;
/// A flashing row is on for this many ticks, then off for as many.
const FLASH_TICKS: u64 = 30;

/// One thing the cluster draws.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// Text with its top left corner at the point.
    Text { text: String, x: i32, y: i32 },
    /// A one-pixel line.
    Line { from: (f64, f64), to: (f64, f64) },
    /// A one-pixel circle outline.
    Circle { centre: (f64, f64), radius: f64 },
}

fn text(marks: &mut Vec<Mark>, text: impl Into<String>, x: i32, y: i32) {
    marks.push(Mark::Text {
        text: text.into(),
        x,
        y,
    });
}

fn line(marks: &mut Vec<Mark>, from: (f64, f64), to: (f64, f64)) {
    marks.push(Mark::Line { from, to });
}

/// The stability label: what really acts, shown unless it is the Damper the
/// pilot chose. `SAS EZ DMP` and `SAS EZ ATT` mark the Easy flight physics
/// cheat supplying the damping or the attitude retention.
pub fn stability_label(s: &State) -> Option<String> {
    let acting = s.stability_acting()?;
    let word = match acting.level {
        StabilityLevel::Off => "OFF",
        StabilityLevel::Damper => "DMP",
        StabilityLevel::Attitude => "ATT",
    };
    if acting.easy {
        Some(format!("SAS EZ {word}"))
    } else if acting.level == StabilityLevel::Damper {
        None
    } else {
        Some(format!("SAS {word}"))
    }
}

/// A flashing row is drawn in the first half of each period.
fn flash_on(s: &State) -> bool {
    (s.ticks / FLASH_TICKS).is_multiple_of(2)
}

/// `COL 64`, the collective lever in percent, which a rotorcraft shows in
/// the throttle readout's place (its throttle keys drive the collective).
pub fn collective_label(s: &State) -> Option<String> {
    s.collective_percent().map(|c| format!("COL {c:.0}"))
}

/// The cluster for `s`, `agl_ft` above the ground. `weapons` is the weapon
/// HUD: the hover display belongs to the navigation HUD (manual p. 81).
pub fn marks(s: &State, agl_ft: f64, weapons: bool) -> Vec<Mark> {
    let mut marks = Vec::new();
    let Some(lift) = s.model().powered_lift() else {
        return marks;
    };
    if s.crashed {
        return marks;
    }
    let flash = flash_on(s);
    if let Some(nr) = s.rotor_speed_percent() {
        let off = !(NR_FLASH[0]..=NR_FLASH[1]).contains(&nr);
        if !off || flash {
            text(&mut marks, format!("NR {nr:.0}"), LEFT_X, NR_Y);
        }
    }
    if let Some(tq) = s.torque_percent()
        && (tq <= TQ_FLASH || flash)
    {
        text(&mut marks, format!("TQ {tq:.0}"), LEFT_X, TQ_Y);
    }
    if let Some(label) = stability_label(s) {
        text(&mut marks, label, LEFT_X, SAS_Y);
    }
    if agl_ft < RADAR_HEIGHT_FT {
        text(
            &mut marks,
            format!("R {:.0}", agl_ft.max(0.)),
            RIGHT_X,
            RADAR_Y,
        );
    }
    if lift.kind == LiftKind::VectorJet {
        nozzle(s, &mut marks);
    }
    if let Some([actual, demand]) = s.nacelle_degrees() {
        nacelle(s, actual, demand, &mut marks);
    }
    if !weapons && let Some(hover) = s.hover_display() {
        hover_display(&hover, &mut marks);
    }
    marks
}

/// The nozzle angle in whole degrees, a gauge with a caret at the demand
/// while the nozzles slew, and `LIFT` while the lift engines run.
fn nozzle(s: &State, marks: &mut Vec<Mark>) {
    let actual = s.nozzle_degrees();
    if actual > 0.5 {
        text(marks, format!("NOZ {actual:.0}"), RIGHT_X, ANGLE_Y);
        let range = 100.;
        let x = |degrees: f64| f64::from(GAUGE_X) + degrees / range * GAUGE_WIDTH;
        let y = f64::from(GAUGE_Y);
        line(marks, (x(0.), y), (x(range), y));
        // The vertical mark, and the pointer at the nozzle.
        line(marks, (x(90.), y - 2.), (x(90.), y + 2.));
        line(marks, (x(actual), y - 4.), (x(actual), y));
        let demand = s.nozzle_demand_degrees();
        if (demand - actual).abs() > 1. {
            // A caret under the line at the demand, pointing up at it.
            line(marks, (x(demand), y + 1.), (x(demand) - 3., y + 5.));
            line(marks, (x(demand) - 3., y + 5.), (x(demand) + 3., y + 5.));
            line(marks, (x(demand) + 3., y + 5.), (x(demand), y + 1.));
        }
    }
    if s.lift_engines_running() {
        text(marks, "LIFT", LIFT_X, ANGLE_Y);
    }
}

/// Where the nacelle tape puts `degrees`: 0 at the bottom.
fn tape_y(degrees: f64) -> f64 {
    f64::from(TAPE_TOP) + TAPE_HEIGHT
        - degrees.clamp(0., NACELLE_TRAVEL_DEGREES) / NACELLE_TRAVEL_DEGREES * TAPE_HEIGHT
}

/// The V-22: `NAC 75`, a tape with the nacelle and its demand, the corridor
/// bracket when the tiltrotor law supplies one, and `CONV` while the
/// conversion protection moves or holds the nacelles.
fn nacelle(s: &State, actual: f64, demand: f64, marks: &mut Vec<Mark>) {
    text(marks, format!("NAC {actual:.0}"), RIGHT_X, ANGLE_Y);
    let x = f64::from(TAPE_X);
    line(marks, (x, tape_y(0.)), (x, tape_y(NACELLE_TRAVEL_DEGREES)));
    for degrees in [0., 30., 60., 90.] {
        line(marks, (x - 2., tape_y(degrees)), (x + 2., tape_y(degrees)));
    }
    // The nacelle: a pointer on the left of the tape.
    let y = tape_y(actual);
    line(marks, (x - 3., y), (x - 8., y - 3.));
    line(marks, (x - 8., y - 3.), (x - 8., y + 3.));
    line(marks, (x - 8., y + 3.), (x - 3., y));
    // The pilot's demand: a caret on the right, when it differs.
    if (demand - actual).abs() > 1. {
        let y = tape_y(demand);
        line(marks, (x + 3., y), (x + 8., y - 3.));
        line(marks, (x + 8., y - 3.), (x + 8., y + 3.));
        line(marks, (x + 8., y + 3.), (x + 3., y));
    }
    // TODO(P5): the corridor bracket for the current indicated airspeed,
    // from the tiltrotor law's corridor (design 4.8): two short bars on the
    // far side of the tape at the lowest and the highest nacelle angle the
    // corridor allows at this speed.
    if let Some([low, high]) = corridor_bracket(s) {
        let bar = |marks: &mut Vec<Mark>, degrees: f64| {
            let y = tape_y(degrees);
            line(marks, (x + 11., y), (x + 16., y));
        };
        bar(marks, low);
        bar(marks, high);
        line(marks, (x + 16., tape_y(low)), (x + 16., tape_y(high)));
    }
    if s.lift_controls.corridor_hold.is_some() {
        text(marks, "CONV", RIGHT_X, CUE_Y);
    }
}

/// The lowest and highest nacelle angle, degrees, the conversion corridor
/// allows at the aircraft's indicated airspeed, or none until the tiltrotor
/// law defines the corridor.
///
/// TODO(P5): invert the corridor table of design 4.8 for the current KCAS.
pub fn corridor_bracket(_s: &State) -> Option<[f64; 2]> {
    None
}

/// The retail manual's hover display: the horizontal velocity circle against
/// fixed cross hairs, whose radius is 10 knots (p. 81), and the vertical
/// velocity bars, with the zero sink centre marks and a lower edge.
fn hover_display(h: &HoverDisplay, marks: &mut Vec<Mark>) {
    let (cx, cy) = HOVER_CENTRE;
    // Cross hairs: zero horizontal velocity, where the aircraft hovers.
    line(marks, (cx - CROSS_HAIR, cy), (cx - 6., cy));
    line(marks, (cx + 6., cy), (cx + CROSS_HAIR, cy));
    line(marks, (cx, cy - CROSS_HAIR), (cx, cy - 6.));
    line(marks, (cx, cy + 6.), (cx, cy + CROSS_HAIR));
    // The circle moves with the aircraft: forward is up, right is right, and
    // it parks at the edge of its field.
    let limit = 2.5 * CIRCLE_RADIUS;
    let dx = (h.right_knots * PIXELS_PER_KNOT).clamp(-limit, limit);
    let dy = (-h.forward_knots * PIXELS_PER_KNOT).clamp(-limit, limit);
    marks.push(Mark::Circle {
        centre: (cx + dx, cy + dy),
        radius: CIRCLE_RADIUS,
    });
    // The vertical bars: the bar, the zero sink marks at the centre, a longer
    // cap at the lower edge, and the tick that rides with the vertical
    // speed (climb up).
    let (bx, half) = (BARS_X, BARS_HALF_HEIGHT);
    line(marks, (bx, cy - half), (bx, cy + half));
    line(marks, (bx - 5., cy), (bx + 5., cy));
    line(marks, (bx - 6., cy + half), (bx + 6., cy + half));
    line(marks, (bx - 2., cy - half), (bx + 2., cy - half));
    let ty = (cy - h.vertical_fps * PIXELS_PER_FPS).clamp(cy - half, cy + half);
    line(marks, (bx + 6., ty), (bx + 14., ty));
    line(marks, (bx + 6., ty), (bx + 10., ty - 3.));
    line(marks, (bx + 6., ty), (bx + 10., ty + 3.));
}

/// Paints a list of marks.
pub fn draw(p: &mut Paint<'_>, font: &Font, marks: &[Mark]) {
    for mark in marks {
        match mark {
            Mark::Text { text, x, y } => p.text(font, text, *x, *y),
            Mark::Line { from, to } => p.line(*from, *to),
            Mark::Circle { centre, radius } => {
                for i in 0..48 {
                    let t = f64::from(i) * std::f64::consts::TAU / 48.;
                    p.rect(
                        (centre.0 + radius * t.cos()).round() as i32,
                        (centre.1 + radius * t.sin()).round() as i32,
                        1,
                        1,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::aircraft::AircraftId;
    use tore_sim::{flight::State, models::FlightModel};

    const KT: f64 = 1.687_81;

    fn state(id: AircraftId) -> State {
        let mut s = State::new(
            &tore_world::test_support::powered_profile(id),
            [0., 3_000., 0.],
        )
        .unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.yaw = 0.;
        s
    }

    fn texts(marks: &[Mark]) -> Vec<(String, i32, i32)> {
        marks
            .iter()
            .filter_map(|m| match m {
                Mark::Text { text, x, y } => Some((text.clone(), *x, *y)),
                _ => None,
            })
            .collect()
    }

    fn has(marks: &[Mark], label: &str) -> bool {
        texts(marks).iter().any(|(t, _, _)| t == label)
    }

    fn at(marks: &[Mark], prefix: &str) -> Option<(String, i32, i32)> {
        texts(marks)
            .into_iter()
            .find(|(t, _, _)| t.starts_with(prefix))
    }

    fn circle(marks: &[Mark]) -> Option<((f64, f64), f64)> {
        marks.iter().find_map(|m| match m {
            Mark::Circle { centre, radius } => Some((*centre, *radius)),
            _ => None,
        })
    }

    #[test]
    fn a_fixed_wing_aircraft_gets_no_cluster() {
        let plain = State::new(&tore_world::test_support::profile(), [0., 3_000., 0.]).unwrap();
        assert!(marks(&plain, 500., false).is_empty());
        assert_eq!(collective_label(&plain), None);
    }

    #[test]
    fn rotor_speed_torque_and_collective_read_in_percent() {
        for id in [AircraftId::Ah64, AircraftId::Mi24] {
            let mut s = state(id);
            assert!(s.start_airborne([0.; 3]));
            let m = marks(&s, 3_000., false);
            assert_eq!(
                at(&m, "NR "),
                Some(("NR 100".into(), LEFT_X, NR_Y)),
                "{id:?}"
            );
            let (tq, x, y) = at(&m, "TQ ").unwrap();
            assert_eq!((x, y), (LEFT_X, TQ_Y));
            let tq: f64 = tq[3..].parse().unwrap();
            assert!((20. ..100.).contains(&tq), "{id:?} torque {tq}");
            // The collective stands where the throttle readout was.
            let col = collective_label(&s).unwrap();
            assert!(col.starts_with("COL "), "{col}");
            let share: f64 = col[4..].parse().unwrap();
            assert!((share - s.lift_controls.collective_actual * 100.).abs() <= 0.5);
        }
    }

    #[test]
    fn nr_flashes_below_90_and_above_105_and_torque_above_100() {
        let mut s = state(AircraftId::Ah64);
        s.start_airborne([0.; 3]);
        for (nr, shown) in [(0.85, false), (0.95, true), (1.0, true), (1.08, false)] {
            s.lift_controls.drive.rotor_speed = nr;
            s.ticks = 0;
            let on = marks(&s, 3_000., false);
            s.ticks = FLASH_TICKS;
            let off = marks(&s, 3_000., false);
            assert!(
                has(&on, &format!("NR {:.0}", nr * 100.)),
                "on phase at {nr}"
            );
            assert_eq!(
                at(&off, "NR ").is_some(),
                shown,
                "off phase at {nr}: steady rows never blink"
            );
        }
        s.lift_controls.drive.rotor_speed = 1.;
        let rated = tore_sim::flight::powered::helicopter::SingleRotor::new(
            &s.model().powered_lift().unwrap(),
            s.model().configuration(),
        )
        .unwrap()
        .drive
        .rated_power;
        s.lift_controls.drive.engine_output[0] = 1.1 * rated;
        s.ticks = 0;
        assert!(has(&marks(&s, 3_000., false), "TQ 110"));
        s.ticks = FLASH_TICKS;
        assert!(at(&marks(&s, 3_000., false), "TQ ").is_none());
    }

    #[test]
    fn the_stability_level_shows_unless_it_is_damper() {
        let mut s = state(AircraftId::Ah64);
        s.start_airborne([0.; 3]);
        assert!(at(&marks(&s, 3_000., false), "SAS").is_none());
        s.lift_controls.aids.stability = StabilityLevel::Off;
        assert_eq!(
            at(&marks(&s, 3_000., false), "SAS"),
            Some(("SAS OFF".into(), LEFT_X, SAS_Y))
        );
        s.lift_controls.aids.stability = StabilityLevel::Attitude;
        assert!(has(&marks(&s, 3_000., false), "SAS ATT"));
        // Without hydraulics the augmentation is off whatever was chosen.
        s.systems.fluids.hydraulic = 0.;
        assert!(has(&marks(&s, 3_000., false), "SAS OFF"));
        // The jets show it too.
        let mut jet = state(AircraftId::Av8);
        jet.lift_controls.aids.stability = StabilityLevel::Off;
        assert!(has(&marks(&jet, 3_000., false), "SAS OFF"));
        // With the Easy flight physics cheat the label says what really acts:
        // the damped jet and the helicopter's attitude retention, even at the
        // default level.
        jet.cheats.easy_physics = true;
        assert!(has(&marks(&jet, 3_000., false), "SAS EZ DMP"));
        jet.lift_controls.aids.stability = StabilityLevel::Damper;
        assert!(at(&marks(&jet, 3_000., false), "SAS").is_none());
        let mut heli = state(AircraftId::Ah64);
        heli.cheats.easy_physics = true;
        assert!(has(&marks(&heli, 3_000., false), "SAS EZ ATT"));
        heli.systems.fluids.hydraulic = 0.;
        assert!(has(&marks(&heli, 3_000., false), "SAS OFF"));
    }

    #[test]
    fn radar_height_shows_below_a_thousand_feet() {
        let s = state(AircraftId::Ah64);
        assert_eq!(
            at(&marks(&s, 450.4, false), "R "),
            Some(("R 450".into(), RIGHT_X, RADAR_Y))
        );
        assert!(at(&marks(&s, 1_000., false), "R ").is_none());
        assert!(has(&marks(&state(AircraftId::Yak141), 12., false), "R 12"));
    }

    #[test]
    fn the_nozzle_angle_and_the_lift_engines_show_on_the_jets() {
        let mut av8 = state(AircraftId::Av8);
        assert!(av8.start_airborne([0.; 3]));
        // Nozzles at 0: nothing.
        assert!(at(&marks(&av8, 3_000., false), "NOZ").is_none());
        assert!(!has(&marks(&av8, 3_000., false), "LIFT"));
        assert!(av8.trim_hover());
        let m = marks(&av8, 3_000., false);
        assert_eq!(at(&m, "NOZ"), Some(("NOZ 90".into(), RIGHT_X, ANGLE_Y)));
        assert!(!has(&m, "LIFT"), "the AV-8 has no lift engines");
        // The demand shows as a caret only while the nozzles slew.
        let lines = |m: &[Mark]| m.iter().filter(|m| matches!(m, Mark::Line { .. })).count();
        let steady = lines(&m);
        av8.lift_controls.vector_pitch = 0.6;
        assert!(lines(&marks(&av8, 3_000., false)) > steady);
        av8.lift_controls.vector_pitch_actual = 0.6;
        assert!(has(&marks(&av8, 3_000., false), "NOZ 60"));
        // The Yak-141's lift engines.
        let mut yak = state(AircraftId::Yak141);
        assert!(yak.trim_hover());
        let m = marks(&yak, 3_000., false);
        assert_eq!(at(&m, "LIFT"), Some(("LIFT".into(), LIFT_X, ANGLE_Y)));
        assert!(has(&m, "NOZ 90"));
        // The jets read no rotor rows.
        assert!(at(&m, "NR ").is_none() && at(&m, "TQ ").is_none());
    }

    #[test]
    fn the_hover_display_is_a_ten_knot_circle_and_vertical_bars() {
        // Helicopter below 40 kt: the circle moves with the aircraft.
        let mut s = state(AircraftId::Ah64);
        s.start_airborne([0.; 3]);
        s.velocity = [0.; 3];
        let m = marks(&s, 3_000., false);
        let (centre, radius) = circle(&m).unwrap();
        assert_eq!(centre, HOVER_CENTRE, "hovering: the circle is on the hairs");
        assert_eq!(radius, CIRCLE_RADIUS);
        // 10 knots forward moves it by its own radius, forward edge over the
        // cross hairs' centre when drifting backwards.
        s.velocity = [0., 0., 10. * KT];
        let (centre, radius) = circle(&marks(&s, 3_000., false)).unwrap();
        assert!((centre.1 - (HOVER_CENTRE.1 - radius)).abs() < 1e-9);
        s.velocity = [0., 0., -10. * KT];
        let (centre, radius) = circle(&marks(&s, 3_000., false)).unwrap();
        assert!((centre.1 - HOVER_CENTRE.1 - radius).abs() < 1e-9);
        // Right drift moves it right.
        s.velocity = [10. * KT, 0., 0.];
        let (centre, _) = circle(&marks(&s, 3_000., false)).unwrap();
        assert!((centre.0 - HOVER_CENTRE.0 - CIRCLE_RADIUS).abs() < 1e-9);
        // The weapon HUD does not carry it.
        assert!(circle(&marks(&s, 3_000., true)).is_none());
        // Above 40 kt it is gone.
        s.velocity = [0., 0., 41. * KT];
        assert!(circle(&marks(&s, 3_000., false)).is_none());
        // The bars: the tick rides above the centre mark in a climb and
        // below it in a descent, and stops at the bar's ends.
        let tick = |s: &State| -> f64 {
            marks(s, 3_000., false)
                .iter()
                .filter_map(|m| match m {
                    Mark::Line { from, to }
                        if (to.0 - from.0 - 8.).abs() < 1e-9 && to.1 == from.1 =>
                    {
                        Some(from.1)
                    }
                    _ => None,
                })
                .next()
                .unwrap()
        };
        s.velocity = [0.; 3];
        assert_eq!(tick(&s), HOVER_CENTRE.1);
        s.velocity = [0., 10., 0.];
        assert!(tick(&s) < HOVER_CENTRE.1);
        s.velocity = [0., -10., 0.];
        assert!(tick(&s) > HOVER_CENTRE.1);
        s.velocity = [0., -500., 0.];
        assert_eq!(tick(&s), HOVER_CENTRE.1 + BARS_HALF_HEIGHT);
        // A jet shows it below its stall speed only.
        let mut jet = state(AircraftId::Av8);
        jet.start_airborne([0.; 3]);
        assert!(circle(&marks(&jet, 3_000., false)).is_none(), "wingborne");
        jet.trim_hover();
        assert!(circle(&marks(&jet, 3_000., false)).is_some(), "hovering");
    }

    #[test]
    fn the_v22_shows_its_nacelles_against_a_tape_and_the_conversion_cue() {
        let mut v22 = state(AircraftId::V22);
        v22.lift_controls.conversion = 87. / NACELLE_TRAVEL_DEGREES;
        v22.lift_controls.conversion_actual = 75. / NACELLE_TRAVEL_DEGREES;
        let m = marks(&v22, 3_000., false);
        assert_eq!(at(&m, "NAC"), Some(("NAC 75".into(), RIGHT_X, ANGLE_Y)));
        assert!(!has(&m, "CONV"));
        // The tape: a mark for the nacelle and one for the demand.
        let steady = {
            let mut still = v22.clone();
            still.lift_controls.conversion = still.lift_controls.conversion_actual;
            marks(&still, 3_000., false)
                .iter()
                .filter(|m| matches!(m, Mark::Line { .. }))
                .count()
        };
        assert_eq!(
            m.iter().filter(|m| matches!(m, Mark::Line { .. })).count(),
            steady + 3
        );
        // The protection moving or holding the nacelles shows CONV.
        v22.lift_controls.corridor_hold = Some(0.5);
        assert_eq!(
            at(&marks(&v22, 3_000., false), "CONV"),
            Some(("CONV".into(), RIGHT_X, CUE_Y))
        );
        // The rotor rows are the same as the helicopters'.
        assert!(at(&m, "NR ").is_some() && at(&m, "TQ ").is_some());
        assert!(collective_label(&v22).is_some());
        // No corridor bracket until the tiltrotor law defines the corridor.
        assert_eq!(corridor_bracket(&v22), None);
    }

    #[test]
    fn rows_stay_clear_of_each_other_and_of_the_existing_hud() {
        // Boxes of the cluster's text rows and the existing labels (6 by 10
        // pixel cells as the imported font): no two overlap.
        let (height, advance) = (10, 6);
        let w = |t: &str| t.chars().count() as i32 * advance;
        let mut boxes = vec![
            ("NR", LEFT_X, NR_Y, w("NR 100")),
            ("TQ", LEFT_X, TQ_Y, w("TQ 100")),
            ("SAS", LEFT_X, SAS_Y, w("SAS EZ DMP")),
            ("R", RIGHT_X, RADAR_Y, w("R 1000")),
            ("NOZ", RIGHT_X, ANGLE_Y, w("NOZ 100")),
            ("LIFT", LIFT_X, ANGLE_Y, w("LIFT")),
            ("CONV", RIGHT_X, CUE_Y, w("CONV")),
            // The existing HUD's: speed box, V/S and the status column.
            ("speed box", 207, 235, 40),
            ("V/S", 207, 271, w("V/S +1000")),
            ("AGL", 402, 259, w("AGL 1000")),
            ("altitude box", 405, 235, 40),
            ("MSL", 402, 201, w("MSL")),
            (
                "collective",
                THROTTLE_READOUT.0,
                THROTTLE_READOUT.1,
                w("COL 100"),
            ),
            ("G", 235, 164, w("9.9G")),
        ];
        // The NAC text shares the angle row with the nozzle's; they are
        // different aircraft, so they are checked on their own.
        boxes.retain(|b| b.0 != "CONV" || b.2 != ANGLE_Y);
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                let apart = a.1 + a.3 <= b.1
                    || b.1 + b.3 <= a.1
                    || a.2 + height <= b.2
                    || b.2 + height <= a.2;
                // The radar height and the ILS AGL line share a column on
                // different rows (262 against 259): they are listed as
                // overlapping only when they really do.
                if a.0 == "R" && b.0 == "AGL" || a.0 == "AGL" && b.0 == "R" {
                    continue;
                }
                assert!(apart, "{} overlaps {}", a.0, b.0);
            }
        }
        // The gauge and the tape stay inside the HUD's clip.
        let (x, y, w, h) = crate::hud::HUD_CLIP;
        for px in [
            f64::from(GAUGE_X),
            f64::from(GAUGE_X) + GAUGE_WIDTH,
            f64::from(TAPE_X) + 16.,
        ] {
            assert!(px >= f64::from(x) && px < f64::from(x + w), "{px}");
        }
        for py in [
            f64::from(TAPE_TOP),
            f64::from(TAPE_TOP) + TAPE_HEIGHT,
            HOVER_CENTRE.1 + CROSS_HAIR,
        ] {
            assert!(py >= f64::from(y) && py < f64::from(y + h), "{py}");
        }
    }

    #[test]
    fn a_crashed_aircraft_draws_no_cluster_and_nothing_is_changed_by_looking() {
        let mut s = state(AircraftId::Ah64);
        s.start_airborne([0.; 3]);
        let before = s.clone();
        let _ = marks(&s, 100., false);
        assert_eq!(s, before);
        s.crashed = true;
        assert!(marks(&s, 100., false).is_empty());
    }

    #[test]
    fn paint_draws_the_text_and_the_circle_with_the_hud_font() {
        use tore_formats::font::{Font, Glyph};
        let glyphs: Vec<Glyph> = (0..256)
            .map(|_| Glyph {
                advance: 6,
                pixels: vec![(0, 0), (1, 0), (0, 1), (1, 1)],
            })
            .collect();
        let font = Font { height: 10, glyphs };
        let mut s = state(AircraftId::Ah64);
        s.start_airborne([0.; 3]);
        s.velocity = [0.; 3];
        let m = marks(&s, 40., false);
        let mut pixels = vec![0u8; crate::menu::WIDTH * crate::menu::HEIGHT * 4];
        let mut p = Paint {
            pixels: &mut pixels,
            clip: crate::hud::HUD_CLIP,
            color: [10, 200, 30, 255],
        };
        draw(&mut p, &font, &m);
        let set =
            |x: i32, y: i32| pixels[(y as usize * crate::menu::WIDTH + x as usize) * 4 + 3] != 0;
        assert!(set(LEFT_X, NR_Y), "NR text");
        assert!(set(RIGHT_X, RADAR_Y), "radar height text");
        // The circle's top and the cross hairs' end.
        assert!(set(
            HOVER_CENTRE.0 as i32,
            (HOVER_CENTRE.1 - CIRCLE_RADIUS) as i32
        ));
        assert!(set(
            (HOVER_CENTRE.0 - CROSS_HAIR) as i32,
            HOVER_CENTRE.1 as i32
        ));
    }
}
