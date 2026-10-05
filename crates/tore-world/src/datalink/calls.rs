//! The assignment call: what a lead says when it gives a wingman a target, and
//! what each listener hears of it (slice G3a; the guide is `docs/DATALINK.md`,
//! "What the player hears").
//!
//! "Two, attack bandit, bearing 270, 15 miles, angels 20": the receiver, the
//! order, then where the target is, measured from the wingman who receives it.
//! A call to the whole flight is made once and each hearer hears the bearing,
//! range and height from its own aircraft, as each hearer of a contact report
//! hears its own clock position ([`hearers`]). A blanket attack order that
//! names no target is "Attack bandits" and carries no geometry
//! ([`blanket_phrase`]).
//!
//! Everything here is plain words and arithmetic: no state, no random number.
//! The stems are the recordings the original shipped: `^NUMnn` and the
//! falling `^NUMnnD`, `^MILEnn`, `^BEARING`, `^ANGELS` and the flight
//! colours.

use crate::{
    comms::{self, Call, Hearer, Kind, Phrase, Phrases},
    radio_calls::FLIGHTS,
    seats::SeatId,
};
use tore_sim::sensors::FEET_PER_NAUTICAL_MILE;

/// A target's place as the receiver hears it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    /// True bearing from the receiver to the target, whole degrees, 1 to 360
    /// (a bearing that rounds to north is said "three six zero"). It is said
    /// as three digits, one by one.
    pub bearing: u32,
    /// Distance in whole nautical miles; `None` under one mile, which the
    /// call leaves out.
    pub miles: Option<u32>,
    /// The target's height in thousands of feet, to the nearest thousand.
    pub angels: u32,
}

impl Geometry {
    /// The target at `to` as heard by a receiver at `from`. World positions
    /// are feet: x east, y up, z north.
    ///
    /// `fitted` (agent decision, 2026-10-05): the range is the distance over
    /// the ground, which goes with the bearing (the height is said apart), and
    /// is left out when it is under one nautical mile (the contact report
    /// measures in a straight line and rounds first, so it says "1 mile" for
    /// half a mile). The height is the target's own, not the difference, as
    /// the waypoint call's "angels" is the waypoint's.
    pub fn between(from: [f64; 3], to: [f64; 3]) -> Self {
        let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
        let degrees = dx.atan2(dz).to_degrees().round() as i64;
        let bearing = match degrees.rem_euclid(360) {
            0 => 360,
            degrees => degrees as u32,
        };
        let range = dx.hypot(dz) / FEET_PER_NAUTICAL_MILE;
        Self {
            bearing,
            miles: (range >= 1.).then(|| range.round() as u32),
            angels: (to[1] / 1_000.).round().max(0.) as u32,
        }
    }
}

/// Who the call is addressed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Addressee {
    /// One wingman, by its place in the flight from zero: member 1 is "Two".
    Wingman(u8),
    /// The whole flight, by the colour of its radio flight number from zero
    /// (0 is Red).
    Flight(u8),
}

const NUMBER_WORDS: [&str; 13] = [
    "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
    "Eleven", "Twelve",
];

/// The recordings of the flight colours. Orange, purple and yellow flights
/// have the words only.
const COLOUR_STEMS: [&str; 5] = ["^RED", "^BLUE", "^GREEN", "^BLACK", "^WHITE"];

fn addressee_phrase(addressee: Addressee) -> Phrase {
    match addressee {
        Addressee::Wingman(member) => {
            let position = u32::from(member) + 1;
            match NUMBER_WORDS.get(position as usize) {
                Some(word) => Phrase::default().raw(word, Some(&format!("^NUM{position:02}"))),
                None => comms::number(position, false),
            }
        }
        Addressee::Flight(flight) => match FLIGHTS.get(usize::from(flight)) {
            Some(colour) => {
                Phrase::default().raw(colour, COLOUR_STEMS.get(usize::from(flight)).copied())
            }
            // A flight past the eighth has no colour; TORE names it by
            // number, as the radio labels do.
            None => Phrase::default()
                .raw("Flight ", None)
                .join(comms::number(u32::from(flight) + 1, false)),
        },
    }
}

/// "Two, attack bandit, bearing 270, 15 miles, angels 20". The bearing is said
/// as real-world brevity has it, always three digits one by one ("zero one
/// six", "two seven zero"); the last digit of the bearing and the height's last
/// number fall in pitch, as in the waypoint call.
/// `phrases` gives the unit word of the range ("miles"); the rest of the text
/// is the call's own.
pub fn assignment_phrase(phrases: &Phrases, addressee: Addressee, geometry: Geometry) -> Phrase {
    let mut call = addressee_phrase(addressee)
        .raw(", attack ", Some("^ATTACK"))
        .raw("bandit", Some("^BANDIT"))
        .raw(", bearing ", Some("^BEARING"))
        .join(bearing_phrase(geometry.bearing));
    if let Some(miles) = geometry.miles {
        call = call.raw(", ", None).join(comms::miles(phrases, miles));
    }
    call.raw(", angels ", Some("^ANGELS"))
        .join(comms::number(geometry.angels, true))
}

/// A bearing as three digits, each its own recording (`^NUM00` is zero), the
/// last one falling: 16 is "zero one six", 5 is "zero zero five" (John,
/// 2026-10-05: real-world brevity, three digits). The text is the three digits.
fn bearing_phrase(bearing: u32) -> Phrase {
    let bearing = bearing.clamp(1, 360);
    let digits = [bearing / 100, bearing / 10 % 10, bearing % 10];
    let mut phrase = Phrase::default();
    for (index, digit) in digits.into_iter().enumerate() {
        let falling = if index == 2 { "D" } else { "" };
        phrase = phrase.raw(
            &digit.to_string(),
            Some(&format!("^NUM{digit:02}{falling}")),
        );
    }
    phrase
}

/// "Attack bandits": a blanket attack order that names no target and makes no
/// assignment.
pub fn blanket_phrase() -> Phrase {
    Phrase::default()
        .raw("Attack ", Some("^ATTACK"))
        .raw("bandits", Some("^BANDITS"))
}

/// The call as radio traffic. It is important: radio silence never drops an
/// order (docs/spec/radio-chatter.md, "Delivery rules"). Delivered, it holds
/// the hearer's channel like any radio line.
pub fn assignment_call(speaker: impl Into<String>, words: Phrase) -> Call {
    Call::new(speaker, words, Kind::Important)
}

/// A seat that hears the call, with the name it hears the speaker by and
/// where its aircraft is.
#[derive(Clone, Debug, PartialEq)]
pub struct Listener {
    pub seat: SeatId,
    pub label: String,
    pub position: [f64; 3],
}

/// One hearer for each listener, each with the call as its own aircraft hears
/// it: the bearing, range and height from where it flies to `target`.
pub fn hearers(
    phrases: &Phrases,
    addressee: Addressee,
    target: [f64; 3],
    listeners: &[Listener],
) -> Vec<Hearer> {
    listeners
        .iter()
        .map(|listener| {
            let geometry = Geometry::between(listener.position, target);
            Hearer::named(listener.seat, listener.label.clone())
                .saying(assignment_phrase(phrases, addressee, geometry))
        })
        .collect()
}

/// Every recording a call can use, so a call's stems can be handed on as the
/// `&'static str` names the order voice cue carries.
const STEMS: &[&str] = &[
    "^NUM00", "^NUM01", "^NUM02", "^NUM03", "^NUM04", "^NUM05", "^NUM06", "^NUM07", "^NUM08",
    "^NUM09", "^NUM10", "^NUM11", "^NUM12", "^NUM00D", "^NUM01D", "^NUM02D", "^NUM03D", "^NUM04D",
    "^NUM05D", "^NUM06D", "^NUM07D", "^NUM08D", "^NUM09D", "^NUM10D", "^NUM11D", "^NUM12D",
    "^MILE01", "^MILE02", "^MILE03", "^MILE04", "^MILE05", "^MILE06", "^MILE07", "^MILE08",
    "^MILE09", "^MILE10", "^MILE20", "^MILE30", "^MILES", "^ATTACK", "^BANDIT", "^BANDITS",
    "^BEARING", "^ANGELS", "^RED", "^BLUE", "^GREEN", "^BLACK", "^WHITE",
];

/// The static name of `stem`, if the call can use it.
pub fn intern(stem: &str) -> Option<&'static str> {
    STEMS.iter().copied().find(|known| *known == stem)
}

/// The stems of the assignment call for the order voice cue and the journal.
pub fn assignment_stems(
    phrases: &Phrases,
    addressee: Addressee,
    geometry: Geometry,
) -> Vec<&'static str> {
    stems_of(&assignment_phrase(phrases, addressee, geometry))
}

/// The stems of "Attack bandits".
pub fn blanket_stems() -> Vec<&'static str> {
    stems_of(&blanket_phrase())
}

fn stems_of(phrase: &Phrase) -> Vec<&'static str> {
    phrase
        .stems
        .iter()
        .filter_map(|stem| {
            let known = intern(stem);
            debug_assert!(known.is_some(), "the call used an unlisted stem {stem}");
            known
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phrases() -> Phrases {
        [("^MILE", " mile"), ("^MILES", " miles")]
            .into_iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    const NM: f64 = FEET_PER_NAUTICAL_MILE;

    /// A target `miles` nautical miles from the origin on `bearing`, at
    /// `height` feet (the receiver is at the origin, at height zero).
    fn target(bearing: f64, miles: f64, height: f64) -> [f64; 3] {
        let (sin, cos) = bearing.to_radians().sin_cos();
        [miles * NM * sin, height, miles * NM * cos]
    }

    fn call(addressee: Addressee, bearing: f64, miles: f64, height: f64) -> Phrase {
        let geometry = Geometry::between([0.; 3], target(bearing, miles, height));
        assignment_phrase(&phrases(), addressee, geometry)
    }

    #[test]
    fn the_example_call_reads_as_john_wrote_it() {
        let phrase = call(Addressee::Wingman(1), 270., 15., 20_000.);
        assert_eq!(
            phrase.text,
            "Two, attack bandit, bearing 270, 15 miles, angels 20"
        );
        assert_eq!(
            phrase.stems,
            [
                "^NUM02", "^ATTACK", "^BANDIT", "^BEARING", "^NUM02", "^NUM07", "^NUM00D",
                "^NUM01", "^NUM05", "^MILES", "^ANGELS", "^NUM02", "^NUM00D",
            ]
        );
    }

    #[test]
    fn bearings_are_three_digits_one_by_one_and_fall_on_the_last() {
        let bearing = |degrees| call(Addressee::Wingman(1), degrees, 15., 20_000.);
        // Real-world brevity (John, 2026-10-05): always three digits, with
        // the original's zero recording (`^NUM00`) for a leading zero.
        for (degrees, text, digits) in [
            (5., "005", ["^NUM00", "^NUM00", "^NUM05D"]),
            (16., "016", ["^NUM00", "^NUM01", "^NUM06D"]),
            (90., "090", ["^NUM00", "^NUM09", "^NUM00D"]),
            (270., "270", ["^NUM02", "^NUM07", "^NUM00D"]),
            (0.2, "360", ["^NUM03", "^NUM06", "^NUM00D"]),
            (123., "123", ["^NUM01", "^NUM02", "^NUM03D"]),
        ] {
            let phrase = bearing(degrees);
            assert!(
                phrase.text.contains(&format!("bearing {text},")),
                "{degrees}: {}",
                phrase.text
            );
            assert_eq!(&phrase.stems[3], "^BEARING");
            assert_eq!(&phrase.stems[4..7], digits, "{degrees}");
        }
        // The bearing is whole degrees, rounded, never 0 and never above 360.
        assert_eq!(
            Geometry::between([0.; 3], target(359.6, 5., 0.)).bearing,
            360
        );
        assert_eq!(
            Geometry::between([0.; 3], target(269.4, 5., 0.)).bearing,
            269
        );
    }

    #[test]
    fn ranges_use_the_miles_recordings_and_under_a_mile_is_left_out() {
        let with = |miles| call(Addressee::Wingman(1), 270., miles, 20_000.);
        let half = with(0.5);
        assert_eq!(
            half.text, "Two, attack bandit, bearing 270, angels 20",
            "half a mile is left out"
        );
        assert!(!half.stems.iter().any(|s| s.starts_with("^MILE")));
        let one = with(1.);
        assert!(one.text.contains(", 1 miles,") || one.text.contains(", 1 mile"));
        assert!(one.stems.contains(&"^MILE01".to_string()));
        let fifteen = with(15.);
        assert!(fifteen.text.contains(", 15 miles,"));
        assert_eq!(
            &fifteen.stems[7..10],
            ["^NUM01", "^NUM05", "^MILES"],
            "above ten, digits then the unit"
        );
        // Twenty and thirty have recordings of their own.
        assert!(with(20.).stems.contains(&"^MILE20".to_string()));
        assert!(with(30.).stems.contains(&"^MILE30".to_string()));
        // Whole miles, nearest.
        assert_eq!(
            Geometry::between([0.; 3], target(0., 14.6, 0.)).miles,
            Some(15)
        );
        assert_eq!(Geometry::between([0.; 3], target(0., 0.99, 0.)).miles, None);
    }

    #[test]
    fn heights_are_thousands_of_feet_and_zero_is_said() {
        let ground = call(Addressee::Wingman(1), 270., 15., 0.);
        assert!(ground.text.ends_with(", angels 0"), "{}", ground.text);
        assert_eq!(
            &ground.stems[ground.stems.len() - 2..],
            ["^ANGELS", "^NUM00D"]
        );
        let high = call(Addressee::Wingman(1), 270., 15., 20_000.);
        assert!(high.text.ends_with(", angels 20"));
        // The nearest thousand, and never below zero.
        assert_eq!(Geometry::between([0.; 3], [0., 19_600., 1.]).angels, 20);
        assert_eq!(Geometry::between([0.; 3], [0., -3_000., 1.]).angels, 0);
        // The target's own height, whatever the receiver's.
        assert_eq!(
            Geometry::between([0., 30_000., 0.], [0., 20_000., 1.]).angels,
            20
        );
    }

    #[test]
    fn the_addressee_is_the_wingman_or_the_flight_colour() {
        let one = call(Addressee::Wingman(3), 270., 15., 20_000.);
        assert!(one.text.starts_with("Four, attack bandit"));
        assert_eq!(one.stems[0], "^NUM04");
        for (flight, word, stem) in [
            (0, "Red", Some("^RED")),
            (1, "Blue", Some("^BLUE")),
            (4, "White", Some("^WHITE")),
            // Orange, purple and yellow have the words only.
            (5, "Orange", None),
            (7, "Yellow", None),
        ] {
            let phrase = call(Addressee::Flight(flight), 270., 15., 20_000.);
            assert!(
                phrase.text.starts_with(&format!("{word}, attack bandit")),
                "{}",
                phrase.text
            );
            assert_eq!(
                phrase.stems.first().map(String::as_str),
                stem.or(Some("^ATTACK"))
            );
        }
        let ninth = call(Addressee::Flight(8), 270., 15., 20_000.);
        assert!(ninth.text.starts_with("Flight 9, attack bandit"));
    }

    #[test]
    fn a_blanket_attack_order_is_attack_bandits_with_no_geometry() {
        let blanket = blanket_phrase();
        assert_eq!(blanket.text, "Attack bandits");
        assert_eq!(blanket.stems, ["^ATTACK", "^BANDITS"]);
        assert_eq!(blanket_stems(), ["^ATTACK", "^BANDITS"]);
    }

    #[test]
    fn every_hearer_hears_the_geometry_of_its_own_aircraft() {
        let target = [0., 20_000., 15. * NM];
        let listeners = [
            Listener {
                seat: SeatId(1),
                label: "Red one".into(),
                position: [0., 20_000., 0.],
            },
            // Ten miles ahead and a mile to the east of the first.
            Listener {
                seat: SeatId(2),
                label: "Red one".into(),
                position: [NM, 20_000., 10. * NM],
            },
        ];
        let hearers = hearers(&phrases(), Addressee::Flight(0), target, &listeners);
        assert_eq!(hearers.len(), 2);
        let words: Vec<_> = hearers
            .iter()
            .map(|h| h.words.as_ref().unwrap().text.clone())
            .collect();
        assert_eq!(
            words[0],
            "Red, attack bandit, bearing 360, 15 miles, angels 20"
        );
        assert_eq!(
            words[1],
            "Red, attack bandit, bearing 349, 5 miles, angels 20"
        );
        assert_eq!(hearers[0].seat, SeatId(1));
        assert_eq!(hearers[1].label.as_deref(), Some("Red one"));
    }

    #[test]
    fn the_call_is_important_and_holds_every_hearers_channel() {
        let listeners = [
            Listener {
                seat: SeatId(0),
                label: "Red one".into(),
                position: [0.; 3],
            },
            Listener {
                seat: SeatId(1),
                label: "Red one".into(),
                position: [0., 0., 5. * NM],
            },
        ];
        let target = [0., 20_000., 15. * NM];
        let mut radio = comms::Comms::with_seats(7, [SeatId(0), SeatId(1)]);
        // Radio silence never drops an order, as it drops chatter.
        radio.toggle_silence(SeatId(1));
        let call = assignment_call(
            "Red one",
            assignment_phrase(
                &phrases(),
                Addressee::Flight(0),
                Geometry::between([0.; 3], target),
            ),
        );
        assert_eq!(call.kind, Kind::Important);
        assert_eq!(call.route, comms::Route::Radio);
        let hearers = hearers(&phrases(), Addressee::Flight(0), target, &listeners);
        radio.send(1., call, &hearers);
        assert!(radio.channel_free(SeatId(0), 1.));
        let due = radio.due(1.);
        assert_eq!(due.len(), 2, "silence on one seat drops nothing");
        assert!(due[0].call.text.contains("15 miles"));
        assert!(
            due[1].call.text.contains("10 miles"),
            "{}",
            due[1].call.text
        );
        assert!(due.iter().all(|d| d.call.label == "Red one"));
        // Delivered, it holds the channel as any radio line does.
        assert!(!radio.channel_free(SeatId(0), 2.));
        assert!(!radio.channel_free(SeatId(1), 2.));
        assert!(radio.channel_free(SeatId(0), 1. + comms::BUSY_SECONDS));
    }

    #[test]
    fn every_stem_a_call_can_use_is_a_known_recording() {
        for degrees in 0..=720 {
            for (miles, height) in [(0., 0.), (1., 0.), (7., 9_000.), (123., 99_000.)] {
                let phrase = call(
                    Addressee::Wingman(1),
                    f64::from(degrees) / 2.,
                    miles,
                    height,
                );
                for stem in &phrase.stems {
                    assert!(intern(stem).is_some(), "{stem} is not in the table");
                }
            }
        }
        for miles in 0..=1_200 {
            let phrase = call(Addressee::Flight(0), 90., f64::from(miles) / 2., 40_000.);
            for stem in &phrase.stems {
                assert!(intern(stem).is_some(), "{stem} is not in the table");
            }
        }
        assert_eq!(intern("^NOPE"), None);
        // The stems handed to the order voice are the whole call.
        let geometry = Geometry::between([0.; 3], target(270., 15., 20_000.));
        assert_eq!(
            assignment_stems(&phrases(), Addressee::Wingman(1), geometry).len(),
            13
        );
    }
}
