//! Wing vapor trails, translated from the native streamer subsystem. Renderer
//! independent: this owns the position history and answers with world points.
//! `_StreamersInit@0` 0x4a0010, `_StreamersUpdate@0` 0x4a0250,
//! `_DrawStreamer@12` 0x49fd90 and the sample ring at 0x4124e0.

/// `0x4a0083`: ten retained entries per side.
pub const CAPACITY: usize = 10;
/// `0x4a0097`: a new history entry commits only every 25 native ticks.
pub const COMMIT_TICKS: i64 = 25;
/// `0x49fe8c`: the trail is six sampled points, so five drawn segments.
pub const POINTS: usize = 6;
/// `0x49fe1d`: one G is stored as 0x100, and the band is three G wide.
const TRIGGER_G: f64 = 3.;
/// `0x49fe40`: full length at three G beyond the trigger, capped at 64 ticks.
const FULL_TICKS: f64 = 64.;
/// `0x49fe5a`: the roll-rate divisor, 180 degrees in 8.8 fixed point.
const ROLL_FULL: f64 = 46080. / 256.;
/// `0x49fe6f`: the reduction saturates at half.
const MAX_REDUCTION: f64 = 0.5;

/// One side's position history. Entry zero is always the live point; older
/// entries shift down only once the commit interval has elapsed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trail {
    entries: [(i64, [f64; 3]); CAPACITY],
}

impl Trail {
    /// `@SampleInit@8`: every slot starts at the same tick and point.
    pub fn seeded(tick: i64, point: [f64; 3]) -> Self {
        Self {
            entries: [(tick, point); CAPACITY],
        }
    }

    /// `@SampleUpdate@8`: overwrite the live entry, and shift only on interval.
    pub fn update(&mut self, tick: i64, point: [f64; 3]) {
        self.entries[0] = (tick, point);
        if self.entries[1].0 + COMMIT_TICKS <= tick {
            self.entries.copy_within(..CAPACITY - 1, 1);
        }
    }

    /// `_SampleGet@12`: clamp below zero, walk from the oldest entry toward the
    /// newest until one is at or past the wanted tick, then blend that entry
    /// with the one immediately older than it.
    pub fn at(&self, tick: i64) -> [f64; 3] {
        let tick = tick.max(0);
        let mut newer = CAPACITY - 1;
        while newer > 0 && self.entries[newer].0 < tick {
            newer -= 1;
        }
        let older = (newer + 1).min(CAPACITY - 1);
        let (newer, older) = (self.entries[newer], self.entries[older]);
        let tick = if older.0 > tick {
            older.0
        } else {
            tick.min(newer.0)
        };
        let span = newer.0 - older.0;
        if span == 0 {
            return older.1;
        }
        let factor = (tick - older.0) as f64 / span as f64;
        std::array::from_fn(|i| older.1[i] + (newer.1[i] - older.1[i]) * factor)
    }
}

/// Both wingtip trails for one aircraft.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vapor {
    sides: [Trail; 2],
    tick: i64,
}

impl Vapor {
    pub fn seeded(points: [[f64; 3]; 2]) -> Self {
        Self {
            sides: [Trail::seeded(0, points[0]), Trail::seeded(0, points[1])],
            tick: 0,
        }
    }

    /// One environment tick, in the same 256-per-second units the clock uses.
    pub fn step(&mut self, tick: i64, points: [[f64; 3]; 2]) {
        self.tick = tick;
        for (trail, point) in self.sides.iter_mut().zip(points) {
            trail.update(tick, point);
        }
    }

    /// `0x49fe17`: the trail length in native ticks, or `None` for no trail.
    /// `g` is the load factor, `roll_rate` degrees per second.
    pub fn length(&self, g: f64, roll_rate: f64, night_hazing: bool) -> Option<f64> {
        if night_hazing || !g.is_finite() || !roll_rate.is_finite() {
            return None;
        }
        let excess = (g - 1.).abs() - TRIGGER_G;
        if excess <= 0. {
            return None;
        }
        let mut ticks = excess.min(TRIGGER_G) / TRIGGER_G * FULL_TICKS;
        // 0x49fe4d: gated in the original on an object flag whose meaning is
        // unresolved, so the reduction is applied unconditionally here.
        let reduction = (roll_rate.abs() / ROLL_FULL).min(1.) * MAX_REDUCTION;
        ticks -= ticks * reduction;
        Some(ticks)
    }

    /// `0x49fe8a`: six points reaching back over the trail length, wingtip first.
    /// Returns one polyline per side, or `None` when no trail is drawn.
    pub fn trail(&self, side: usize, g: f64, roll_rate: f64, hazing: bool) -> Option<Trail6> {
        let ticks = self.length(g, roll_rate, hazing)?;
        let trail = self.sides.get(side)?;
        Some(std::array::from_fn(|i| {
            let back = ticks * i as f64 / (POINTS - 1) as f64;
            trail.at(self.tick - back.round() as i64)
        }))
    }

    /// How far along the trail each drawn segment sits, for fading.
    pub fn segments(&self) -> usize {
        POINTS - 1
    }
}

/// The six sampled world points of one trail, wingtip first.
pub type Trail6 = [[f64; 3]; POINTS];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_commits_only_on_the_native_interval() {
        let mut trail = Trail::seeded(0, [0.; 3]);
        for tick in 1..=COMMIT_TICKS {
            trail.update(tick, [tick as f64, 0., 0.]);
        }
        // One commit has happened, so the oldest retained point is still the seed.
        assert_eq!(trail.at(0), [0.; 3]);
        assert_eq!(trail.at(COMMIT_TICKS)[0], COMMIT_TICKS as f64);
        let mid = trail.at(COMMIT_TICKS / 2)[0];
        assert!(mid > 0. && mid < COMMIT_TICKS as f64, "interpolated {mid}");
    }

    #[test]
    fn commit_copies_the_current_sample_before_the_next_interval() {
        let mut trail = Trail::seeded(0, [0.; 3]);
        trail.update(24, [24.; 3]);
        trail.update(25, [100.; 3]);
        // Retail writes entry zero before memmove, so both newest slots hold 25.
        assert_eq!(trail.entries[0], (25, [100.; 3]));
        assert_eq!(trail.entries[1], (25, [100.; 3]));
        trail.update(49, [200.; 3]);
        assert_eq!(trail.entries[1], (25, [100.; 3]));
        assert_eq!(trail.at(37), [150.; 3]);
        trail.update(50, [300.; 3]);
        assert_eq!(trail.entries[1], (50, [300.; 3]));
        assert_eq!(trail.entries[2], (25, [100.; 3]));
    }

    #[test]
    fn trail_needs_more_than_three_g_either_way() {
        let v = Vapor::seeded([[0.; 3]; 2]);
        for g in [1., 4., -2., 0., 3.9] {
            assert!(v.length(g, 0., false).is_none(), "{g} G must not trail");
        }
        assert!(v.length(4.1, 0., false).is_some());
        assert!(v.length(-2.1, 0., false).is_some());
        // Full length at six G beyond one, and no further growth past it.
        let full = v.length(7., 0., false).unwrap();
        assert!((full - FULL_TICKS).abs() < 1e-9);
        assert!((v.length(12., 0., false).unwrap() - FULL_TICKS).abs() < 1e-9);
    }

    #[test]
    fn night_hazing_and_roll_rate_shorten_or_remove_the_trail() {
        let v = Vapor::seeded([[0.; 3]; 2]);
        assert!(v.length(7., 0., true).is_none());
        let level = v.length(7., 0., false).unwrap();
        let rolling = v.length(7., 180., false).unwrap();
        assert!((rolling - level * 0.5).abs() < 1e-9, "{rolling} vs {level}");
        // The reduction saturates rather than inverting the trail.
        assert!(v.length(7., 100_000., false).unwrap() >= level * 0.5 - 1e-9);
        assert!(v.length(f64::NAN, 0., false).is_none());
    }

    #[test]
    fn six_points_reach_back_over_the_recovered_length() {
        let mut v = Vapor::seeded([[0.; 3]; 2]);
        for tick in 1..=200 {
            let x = tick as f64;
            v.step(tick, [[x, 0., 0.], [x, 10., 0.]]);
        }
        let trail = v.trail(1, 7., 0., false).unwrap();
        assert_eq!(trail.len(), POINTS);
        assert_eq!(trail[0], [200., 10., 0.]);
        // Points recede monotonically and span the full 64-tick length.
        for pair in trail.windows(2) {
            assert!(pair[1][0] <= pair[0][0]);
        }
        assert!((trail[POINTS - 1][0] - (200. - FULL_TICKS)).abs() < 1.);
        assert!(v.trail(2, 7., 0., false).is_none());
    }
}
