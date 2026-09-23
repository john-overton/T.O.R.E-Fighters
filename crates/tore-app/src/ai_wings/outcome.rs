//! In-flight mission result cadence for situation music (SUCC and HOME).
//!
//! The result itself comes from the debrief evaluator
//! ([`crate::debrief::capture`]), so the music and the debrief always agree.
//! This file keeps the parts only flight needs: the 4 second check, the rule
//! that a result already decided at flight start disables SUCC and HOME, the
//! home check and the two radio calls.
/// The mission result and home check run this often, game seconds.
pub const CHECK_S: f64 = 4.;
/// Home: this close to the home base...
pub const HOME_RANGE_FT: f64 = 42_240.;
/// ...and below this altitude (above sea level; retail datum unknown).
pub const HOME_CEILING_FT: f64 = 20_000.;
pub const MISSION_ACCOMPLISHED: &str = "^MISSACC";
pub const ALMOST_HOME: &str = "^ALMSTHM";

/// What the music needs from the mission this step.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub succeeded: bool,
    /// Latched once reached; HOME itself still plays once.
    pub home: bool,
    /// Radio calls to queue now.
    pub radio: Vec<&'static str>,
}

/// Mission result cadence, success-disabled rule and home check for one flight.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracker {
    /// None before the first check; false when the result was already
    /// decided at flight start or there is no mission.
    enabled: Option<bool>,
    next_check: f64,
    succeeded: bool,
    announced: bool,
    home: bool,
    home_base: Option<[f64; 3]>,
}
impl Tracker {
    pub fn new(home_base: Option<[f64; 3]>) -> Self {
        Self {
            home_base,
            ..Self::default()
        }
    }
    /// `evaluate` is None when the flight has no mission.
    pub fn step(
        &mut self,
        now: f64,
        succeeded: Option<impl FnOnce() -> bool>,
        position: [f64; 3],
        airborne: bool,
    ) -> Status {
        let mut radio = Vec::new();
        if now >= self.next_check {
            self.next_check = now + CHECK_S;
            match (self.enabled, succeeded) {
                // Success already reached at flight start disables SUCC and
                // HOME. The debrief evaluator cannot report an in-flight
                // failure, so an already failed start is not detected.
                (None, Some(succeeded)) => self.enabled = Some(!succeeded()),
                (None, None) => self.enabled = Some(false),
                (Some(true), Some(succeeded)) => {
                    self.succeeded = succeeded();
                    if self.succeeded {
                        if !self.announced {
                            self.announced = true;
                            radio.push(MISSION_ACCOMPLISHED);
                        }
                        if !self.home
                            && self.home_base.is_some_and(|base| {
                                let range = (base[0] - position[0]).hypot(base[2] - position[2]);
                                range.hypot(base[1] - position[1]) < HOME_RANGE_FT
                                    && position[1] < HOME_CEILING_FT
                            })
                        {
                            self.home = true;
                            if airborne {
                                radio.push(ALMOST_HOME);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Status {
            succeeded: self.succeeded,
            home: self.home,
            radio,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_is_disabled_when_decided_at_flight_start() {
        let mut t = Tracker::new(Some([0.; 3]));
        let decided = t.step(0., Some(|| true), [0.; 3], true);
        assert_eq!(decided, Status::default());
        let later = t.step(8., Some(|| true), [0.; 3], true);
        assert!(!later.succeeded && !later.home && later.radio.is_empty());
        let mut none = Tracker::new(None);
        none.step(0., None::<fn() -> bool>, [0.; 3], true);
        assert!(!none.step(4., Some(|| true), [0.; 3], true).succeeded);
    }

    #[test]
    fn four_second_cadence_radio_and_home() {
        let mut t = Tracker::new(Some([0., 0., 0.]));
        let far = [50_000., 5_000., 0.];
        assert!(t.step(0., Some(|| false), far, true).radio.is_empty());
        // Targets die at 1 s; nothing is re-evaluated before 4 s.
        assert!(!t.step(3.99, Some(|| true), far, true).succeeded);
        let s = t.step(4., Some(|| true), far, true);
        assert!(s.succeeded && !s.home);
        assert_eq!(s.radio, [MISSION_ACCOMPLISHED]);
        // Within 42,240 ft but too high, then low enough.
        let high = [30_000., 20_000., 0.];
        assert!(!t.step(8., Some(|| true), high, true).home);
        let near = [30_000., 19_999., 0.];
        let s = t.step(12., Some(|| true), near, true);
        assert!(s.home);
        assert_eq!(s.radio, [ALMOST_HOME]);
        let s = t.step(16., Some(|| true), far, true);
        assert!(
            s.home && s.radio.is_empty(),
            "home is latched and called once"
        );
        // A later failure clears success but not the home latch.
        assert!(!t.step(20., Some(|| false), far, true).succeeded);
        // No home base: HOME never.
        let mut t = Tracker::new(None);
        t.step(0., Some(|| false), [0.; 3], true);
        assert!(!t.step(4., Some(|| true), [0.; 3], true).home);
        // On the ground the home condition is reached without the radio call.
        let mut t = Tracker::new(Some([0.; 3]));
        t.step(0., Some(|| false), [0.; 3], false);
        let s = t.step(4., Some(|| true), [0.; 3], false);
        assert!(s.home);
        assert_eq!(s.radio, [MISSION_ACCOMPLISHED]);
    }
}
