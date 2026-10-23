//! The coder of a cockpit's mission result tracker (docs/formats/checkpoint.md).
//!
//! Stage H slice H6. The cockpits section (slice H2) calls it. Every field is
//! mutable state: the check cadence, the latches and the home base the tracker
//! was built with (a plane's home can change when a seat is handed a plane, so
//! it is coded, not rebuilt).

use super::Tracker;

tore_sim::checkpoint_struct!(Tracker {
    enabled,
    next_check,
    succeeded,
    announced,
    home,
    home_base,
});

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::checkpoint::{Models, from_bytes, round_trip, to_bytes};

    fn models() -> Models {
        Models::default()
    }

    /// A tracker with every field away from its start value.
    fn lived_in() -> Tracker {
        let mut tracker = Tracker::new(Some([1250.5, -3.0, f64::MIN_POSITIVE]));
        tracker.enabled = Some(true);
        tracker.next_check = 123.456;
        tracker.succeeded = true;
        tracker.announced = true;
        tracker.home = true;
        tracker
    }

    #[test]
    fn a_tracker_round_trips_in_every_state() {
        for tracker in [
            Tracker::default(),
            Tracker::new(None),
            Tracker::new(Some([0., 0., 0.])),
            lived_in(),
            Tracker {
                enabled: Some(false),
                next_check: f64::INFINITY,
                ..Tracker::default()
            },
        ] {
            let copy = round_trip(&tracker, &models()).unwrap();
            assert_eq!(copy, tracker);
            assert_eq!(
                to_bytes(&copy, &models()).unwrap(),
                to_bytes(&tracker, &models()).unwrap()
            );
        }
    }

    #[test]
    fn a_restored_tracker_makes_the_same_checks() {
        // The check cadence and the latches decide when the calls come, so a
        // restored tracker must agree with the original on what it says.
        let tracker = lived_in();
        let copy: Tracker = round_trip(&tracker, &models()).unwrap();
        assert_eq!(copy.status(), tracker.status());
        assert!(copy.status().succeeded);
        assert!(copy.status().home);
    }

    #[test]
    fn damaged_bytes_are_refused_or_decode_without_a_panic() {
        let coded = to_bytes(&lived_in(), &models()).unwrap();
        for cut in 0..coded.body.len() {
            let mut shorter = coded.clone();
            shorter.body.truncate(cut);
            let _ = from_bytes::<Tracker>(&shorter, &models());
        }
    }
}
