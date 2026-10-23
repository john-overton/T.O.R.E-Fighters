//! In-flight situation music choice, from `docs/spec/flight-music.md`.
//!
//! Pure: no device, file, wall clock or simulation access. The host hands in
//! game-time observations each fixed step; the mixer reports what the score
//! player did. Provenance is spec-derived unless a comment says otherwise.

/// The nine situation scores by rank; a higher rank cuts in immediately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    Normal,
    Deck,
    Home,
    Danger,
    Air,
    Launch,
    Eject,
    Succ,
    Valk,
}
impl Rank {
    /// Index into `tore_formats::music::SCORES`. Rank is not that index.
    pub const fn score(self) -> usize {
        match self {
            Rank::Normal => 0,
            Rank::Air => 1,
            Rank::Danger => 2,
            Rank::Deck => 3,
            Rank::Launch => 4,
            Rank::Home => 5,
            Rank::Eject => 6,
            Rank::Succ => 7,
            Rank::Valk => 8,
        }
    }
}

/// Designated enemy aircraft closer than this selects AIR, farther DANGER.
pub const AIR_RANGE_FT: f64 = 40_000.;
/// A projectile hit on the player keeps AIR selected this long.
pub const HIT_HOLD_S: f64 = 30.;
/// An AI aircraft with the player as target and a missile ready warns this long.
pub const AIM_MEMORY_S: f64 = 4.;
/// An AIM-120 farther than this is not counted as guided at the player.
pub const AIM120_IGNORE_FT: f64 = 30_380.;
/// No new choice for this long after a score starts.
pub const LOCKOUT_S: f64 = 1.;
/// A score that could not play is not retried for this long.
pub const RETRY_S: f64 = 10.;

/// One fixed step of situation observations. `false` means "not observed",
/// never "invented"; see the spec's current TORE state for what feeds each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Inputs {
    pub succeeded: bool,
    pub ejected: bool,
    pub launching: bool,
    /// Designated live enemy aircraft within [`AIR_RANGE_FT`].
    pub air_target: bool,
    /// A projectile hit the player within [`HIT_HOLD_S`].
    pub hit_recently: bool,
    /// Designated enemy beyond air range, AI missile aim, or a missile guided
    /// at the player.
    pub danger: bool,
    /// The home condition has been reached this flight (latched by the host).
    pub home: bool,
    pub deck: bool,
}

/// What the score player did since the previous check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Playback {
    /// A score is still running; `false` after it ended or failed.
    pub playing: bool,
    /// The script passed a marked reevaluation boundary (F9).
    pub boundary: bool,
    /// The score stopped because a script or phrase is missing.
    pub failed: bool,
}

/// Transition state for one session. Only the Valkyries toggle survives a new flight.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selector {
    current: Option<Rank>,
    next_check: f64,
    succ_played: bool,
    home_played: bool,
    valkyries: bool,
}
impl Selector {
    /// Fresh flight: no score, both once-per-flight scores available.
    pub fn new_flight(&mut self) {
        *self = Self {
            valkyries: self.valkyries,
            ..Self::default()
        };
    }
    /// Ctrl+V. The caller stops the current score; the next check chooses again.
    pub fn toggle_valkyries(&mut self) -> bool {
        self.valkyries = !self.valkyries;
        self.current = None;
        self.valkyries
    }
    /// Music is off: nothing plays, and turning it on starts a fresh choice.
    pub fn silence(&mut self) {
        self.current = None;
    }
    #[cfg(test)]
    pub fn current(&self) -> Option<Rank> {
        self.current
    }
    /// Whether this step may choose; false during the lockout or failure retry.
    pub fn due(&self, now: f64) -> bool {
        now >= self.next_check
    }
    /// First matching condition, top down.
    pub fn choose(&self, inputs: &Inputs) -> Rank {
        if self.valkyries {
            Rank::Valk
        } else if inputs.succeeded && !self.succ_played {
            Rank::Succ
        } else if inputs.ejected {
            Rank::Eject
        } else if inputs.launching {
            Rank::Launch
        } else if inputs.air_target || inputs.hit_recently {
            Rank::Air
        } else if inputs.danger {
            Rank::Danger
        } else if inputs.home && !self.home_played {
            Rank::Home
        } else if inputs.deck {
            Rank::Deck
        } else {
            Rank::Normal
        }
    }
    /// The situation asks for a different score than the one playing, so the
    /// player should stop at the next marked boundary instead of starting the
    /// following phrase of the old score.
    pub fn waiting(&self, inputs: &Inputs) -> bool {
        self.current
            .is_some_and(|current| self.choose(inputs) != current)
    }
    /// One check. `Some(rank)` means start that score from its beginning,
    /// cutting whatever plays, with no fade.
    pub fn update(&mut self, now: f64, inputs: &Inputs, playback: Playback) -> Option<Rank> {
        if !self.due(now) {
            return None;
        }
        if playback.failed {
            // `fitted`: the spec's failed-load retry applied to a missing
            // script or phrase, which in TORE is only found while playing.
            self.current = None;
            self.next_check = now + RETRY_S;
            return None;
        }
        if !playback.playing {
            self.current = None;
        }
        let chosen = self.choose(inputs);
        if self.current == Some(chosen) {
            return None;
        }
        if playback.boundary {
            self.current = None;
        }
        if self.current.is_some_and(|current| chosen < current) {
            return None;
        }
        self.current = Some(chosen);
        match chosen {
            Rank::Succ => self.succ_played = true,
            Rank::Home => self.home_played = true,
            _ => {}
        }
        self.next_check = now + LOCKOUT_S;
        Some(chosen)
    }
}

/// Game-time hold: true until `seconds` after the latest refresh.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hold {
    until: Option<f64>,
}
impl Hold {
    pub fn refresh(&mut self, now: f64, seconds: f64) {
        self.until = Some(self.until.map_or(now + seconds, |u| u.max(now + seconds)));
    }
    pub fn active(&self, now: f64) -> bool {
        self.until.is_some_and(|until| now < until)
    }
}

/// Speed on the ground that separates parked from rolling, ft/s.
pub const ROLLING_FPS: f64 = 7.;
/// Climb-out window after takeoff.
pub const CLIMB_OUT_RANGE_FT: f64 = 25_000.;
pub const CLIMB_OUT_AGL_FT: f64 = 4_000.;
pub const CLIMB_OUT_MAX_FPS: f64 = 954.;

/// One fixed step of the player's airport situation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// Supported on an airport surface.
    pub on_ground: bool,
    pub speed_fps: f64,
    pub position: [f64; 3],
    pub agl_ft: f64,
    pub gear_down: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Airborne,
    Parked,
    TakeoffRoll,
    ClimbOut { liftoff: [f64; 3] },
    Rollout,
}

/// `fitted`: TORE has no retail airport state machine for the player, so this
/// follows the spec's DECK and LAUNCH table from observed ground contact.
/// The liftoff point stands in for the airport reference in the 25,000 ft
/// climb-out test. A bounce during a landing rollout counts as airborne, not
/// as a new takeoff (unknown in retail). No carrier or catapult exists.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Airport {
    phase: Option<Phase>,
}
impl Airport {
    /// Returns (deck, launching) for this step.
    pub fn step(&mut self, g: &Ground) -> (bool, bool) {
        let rolling = g.speed_fps >= ROLLING_FPS;
        let on_ground = |rolling: bool| {
            if rolling {
                Phase::TakeoffRoll
            } else {
                Phase::Parked
            }
        };
        let phase = match self.phase {
            None if g.on_ground => on_ground(rolling),
            None => Phase::Airborne,
            Some(Phase::Parked | Phase::TakeoffRoll) if !g.on_ground => Phase::ClimbOut {
                liftoff: g.position,
            },
            Some(Phase::Parked | Phase::TakeoffRoll | Phase::ClimbOut { .. }) if g.on_ground => {
                on_ground(rolling)
            }
            Some(Phase::ClimbOut { liftoff }) => {
                let range = liftoff
                    .iter()
                    .zip(g.position)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt();
                if range < CLIMB_OUT_RANGE_FT
                    && g.agl_ft < CLIMB_OUT_AGL_FT
                    && g.gear_down
                    && g.speed_fps <= CLIMB_OUT_MAX_FPS
                {
                    Phase::ClimbOut { liftoff }
                } else {
                    Phase::Airborne
                }
            }
            Some(Phase::Airborne) if g.on_ground => Phase::Rollout,
            Some(Phase::Rollout) if !g.on_ground => Phase::Airborne,
            Some(Phase::Rollout) if !rolling => Phase::Parked,
            Some(phase) => phase,
        };
        self.phase = Some(phase);
        match phase {
            Phase::Parked | Phase::Rollout => (true, false),
            Phase::TakeoffRoll | Phase::ClimbOut { .. } => (false, true),
            Phase::Airborne => (false, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PLAYING: Playback = Playback {
        playing: true,
        boundary: false,
        failed: false,
    };
    const BOUNDARY: Playback = Playback {
        playing: true,
        boundary: true,
        failed: false,
    };
    const ENDED: Playback = Playback {
        playing: false,
        boundary: false,
        failed: false,
    };

    #[test]
    fn rank_is_not_the_script_table_order() {
        assert_eq!(
            tore_formats::music::SCORES[Rank::Deck.score()],
            "M_DECK.MUS"
        );
        assert_eq!(
            tore_formats::music::SCORES[Rank::Home.score()],
            "M_HOME.MUS"
        );
        assert_eq!(tore_formats::music::SCORES[Rank::Air.score()], "M_AIR.MUS");
        assert!(Rank::Air > Rank::Danger && Rank::Danger > Rank::Home);
        assert!(Rank::Launch > Rank::Air && Rank::Succ > Rank::Eject);
    }

    #[test]
    fn first_match_order_follows_the_table() {
        let s = Selector::default();
        let all = Inputs {
            succeeded: true,
            ejected: true,
            launching: true,
            air_target: true,
            hit_recently: true,
            danger: true,
            home: true,
            deck: true,
        };
        assert_eq!(s.choose(&all), Rank::Succ);
        assert_eq!(
            s.choose(&Inputs {
                succeeded: false,
                ..all
            }),
            Rank::Eject
        );
        let combat = Inputs {
            danger: true,
            hit_recently: true,
            deck: true,
            ..Inputs::default()
        };
        assert_eq!(s.choose(&combat), Rank::Air);
        assert_eq!(s.choose(&Inputs::default()), Rank::Normal);
    }

    #[test]
    fn higher_rank_cuts_in_and_lower_rank_waits_for_a_boundary() {
        let mut s = Selector::default();
        assert_eq!(s.update(0., &Inputs::default(), ENDED), Some(Rank::Normal));
        let danger = Inputs {
            danger: true,
            ..Inputs::default()
        };
        // Lockout: one second after a start nothing changes.
        assert_eq!(s.update(0.5, &danger, PLAYING), None);
        assert_eq!(s.update(1.0, &danger, PLAYING), Some(Rank::Danger));
        let air = Inputs {
            air_target: true,
            ..danger
        };
        assert_eq!(s.update(2.5, &air, PLAYING), Some(Rank::Air));
        // AIR to DANGER is a downgrade and waits for the marked boundary.
        assert_eq!(s.update(10., &danger, PLAYING), None);
        assert!(s.waiting(&danger));
        assert_eq!(s.update(40., &danger, PLAYING), None);
        assert_eq!(s.update(40.1, &danger, BOUNDARY), Some(Rank::Danger));
        // Same score chosen again never restarts it, boundary or not.
        assert_eq!(s.update(80., &danger, BOUNDARY), None);
        assert!(!s.waiting(&danger));
        // A boundary may also choose any score, lower included.
        assert_eq!(
            s.update(90., &Inputs::default(), BOUNDARY),
            Some(Rank::Normal)
        );
    }

    #[test]
    fn success_and_home_play_once_per_flight() {
        let mut s = Selector::default();
        let success = Inputs {
            succeeded: true,
            home: true,
            ..Inputs::default()
        };
        assert_eq!(s.update(0., &success, ENDED), Some(Rank::Succ));
        // SUCC ends by itself after one phrase; HOME is next and also once.
        assert_eq!(s.update(30., &success, ENDED), Some(Rank::Home));
        assert_eq!(s.update(200., &success, ENDED), Some(Rank::Normal));
        assert_eq!(s.update(300., &success, ENDED), Some(Rank::Normal));
        s.new_flight();
        assert_eq!(s.update(0., &success, ENDED), Some(Rank::Succ));
    }

    #[test]
    fn launch_restarts_while_still_launching() {
        let mut s = Selector::default();
        let launching = Inputs {
            launching: true,
            ..Inputs::default()
        };
        assert_eq!(s.update(0., &launching, ENDED), Some(Rank::Launch));
        assert_eq!(s.update(60., &launching, PLAYING), None);
        assert_eq!(s.update(127., &launching, ENDED), Some(Rank::Launch));
        // Combat cannot interrupt LAUNCH.
        let fight = Inputs {
            air_target: true,
            ..launching
        };
        assert_eq!(s.update(130., &fight, PLAYING), None);
    }

    #[test]
    fn failed_scores_retry_after_ten_seconds_and_valkyries_persists() {
        let mut s = Selector::default();
        assert!(s.toggle_valkyries());
        assert_eq!(s.update(0., &Inputs::default(), ENDED), Some(Rank::Valk));
        let failed = Playback {
            failed: true,
            ..ENDED
        };
        assert_eq!(s.update(1., &Inputs::default(), failed), None);
        assert_eq!(s.update(10.9, &Inputs::default(), ENDED), None);
        assert_eq!(s.update(11., &Inputs::default(), ENDED), Some(Rank::Valk));
        s.new_flight();
        assert_eq!(s.update(0., &Inputs::default(), ENDED), Some(Rank::Valk));
        assert!(!s.toggle_valkyries());
        assert_eq!(s.current(), None);
        assert_eq!(s.update(1., &Inputs::default(), ENDED), Some(Rank::Normal));
    }

    #[test]
    fn hit_hold_lasts_thirty_game_seconds_from_the_latest_hit() {
        let mut hold = Hold::default();
        assert!(!hold.active(0.));
        hold.refresh(5., HIT_HOLD_S);
        assert!(hold.active(34.9));
        hold.refresh(20., HIT_HOLD_S);
        assert!(hold.active(49.9));
        assert!(!hold.active(50.));
    }

    #[test]
    fn airport_phases_give_deck_and_launch() {
        let mut a = Airport::default();
        let mut g = Ground {
            on_ground: true,
            speed_fps: 0.,
            position: [0.; 3],
            agl_ft: 0.,
            gear_down: true,
        };
        assert_eq!(a.step(&g), (true, false));
        g.speed_fps = 7.;
        assert_eq!(a.step(&g), (false, true));
        g.on_ground = false;
        g.speed_fps = 300.;
        g.agl_ft = 500.;
        assert_eq!(a.step(&g), (false, true));
        g.gear_down = false;
        assert_eq!(a.step(&g), (false, false));
        // Leaving the window is final: lowering the gear again is not launching.
        g.gear_down = true;
        assert_eq!(a.step(&g), (false, false));
        g.on_ground = true;
        g.speed_fps = 200.;
        assert_eq!(a.step(&g), (true, false), "touchdown and rollout");
        g.speed_fps = 6.9;
        assert_eq!(a.step(&g), (true, false));
        g.speed_fps = 20.;
        assert_eq!(a.step(&g), (false, true), "a new takeoff roll");
        // Climb-out ends beyond 25,000 ft, 4,000 ft above ground or 954 ft/s.
        let changes: [fn(&mut Ground); 3] = [
            |g| g.position[0] = 25_000.,
            |g| g.agl_ft = 4_000.,
            |g| g.speed_fps = 954.5,
        ];
        for change in changes {
            let mut a = Airport::default();
            let mut g = Ground {
                on_ground: true,
                speed_fps: 150.,
                position: [0.; 3],
                agl_ft: 0.,
                gear_down: true,
            };
            a.step(&g);
            g.on_ground = false;
            g.position[1] = 100.;
            assert_eq!(a.step(&g), (false, true));
            change(&mut g);
            assert_eq!(a.step(&g), (false, false));
        }
        // An airborne start is neither.
        let mut a = Airport::default();
        g.on_ground = false;
        assert_eq!(a.step(&g), (false, false));
    }
}
