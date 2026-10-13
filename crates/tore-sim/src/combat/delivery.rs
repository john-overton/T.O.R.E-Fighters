//! The air-to-ground delivery solution: what the HUD's bomb and rocket sights
//! draw and what the level release acts on (docs/spec/air-to-ground.md, "The
//! HUD in air-to-ground modes").
//!
//! Combat works it out for each ownship every tick, for the selected station,
//! from the launcher of that tick ([`solve`]); the cockpit readout carries it
//! to the HUD, so the sight and the release cannot disagree.
//!
//! - **CCIP** (the bomb sight routine; docs/formats/weapons.md, "Bomb sight"):
//!   the mean impact point of a bomb released now, from
//!   [`ReleaseState::from_station`] and [`predict_impact`], with the range arc
//!   of the retail pipper glyph ([`arc_steps`]). The point is where the bomb's
//!   path meets the terrain along the whole fall (opinionated, John,
//!   2026-10-11; retail takes the ground under the aircraft for all of it).
//! - **Rockets** (opinionated, agent, decision 12): the same pipper at the
//!   ground impact of a rocket fired now, flown through the very steps combat
//!   gives an unguided round ([`rocket_impact`]).
//! - **LVL** (opinionated, John, 2026-10-10; the design is the agent's): the
//!   impact compared with a ground target, as an along-track error, a
//!   cross-track error, a time to release and an azimuth steering correction
//!   ([`LevelRelease`]). The impact is the bomb's mean path brought down to
//!   the target's ground ([`Delivery::impact_at`]), not to the ground under
//!   the aircraft as the CCIP pipper's is, so a target on higher or lower
//!   ground is still hit (agent, slice ST-LV). While the release cue lies
//!   ahead a held trigger waits for it and releases there
//!   ([`LevelRelease::due`]).
//! - **Warnings** (opinionated, agent, decision 15): LOW when the bomb would
//!   land before its fuze arms, FRAG when its burst would reach the aircraft
//!   flying on without descending ([`flown`]), OVERFLY when LVL has passed
//!   the target.
//! - **Reach** (opinionated, agent; John's "range indicator for when to
//!   drop"): the ground ranges a guided bomb can still make good from a
//!   release, searched with the guided weapon's own fly-out ([`Reach::search`]).
//!
//! Coordinates are the live adapter's: feet, x and z horizontal, y up,
//! headings from +z toward +x.
use super::ballistics::{self, Impact, ReleaseState, predict_impact};
use super::live::{Launcher, Station};
use super::{FallState, axial_speed, commanded_speed, engine_phase, launch_speed, removal_due};
use crate::attitude::Vector;
use tore_formats::weapons::Weapon;

/// The steps of the pipper's inner arc: glyph `0x20` is the bare ring, `0x2f`
/// the closed arc (retail data, HUDSYM11 glyphs 0x20 to 0x2f).
pub const ARC_STEPS: u8 = 15;
/// LVL is chosen for a newly selected bomb when a ground target is designated
/// and the flight path is within this many degrees of level (fitted).
pub const LEVEL_CHOICE_DEGREES: f64 = 10.;
/// The release marker sits at the top of the stem while the release is this
/// many seconds or more away, and slides linearly to the crossbar at 0 s
/// (opinionated, agent).
pub const MARKER_SECONDS: f64 = 15.;
/// Farther out than this the stem is drawn dashed (opinionated, agent).
pub const DASHED_SECONDS: f64 = 60.;
/// The dot trail's spacing along the fall: retail draws one every 64 of its
/// ticks, a quarter second (retail data, `0x409807`).
pub const TRAIL_SECONDS: f64 = 0.25;
/// How many ticks of ground travel past the cue a held trigger may still
/// release in, so a cue that falls between two ticks is not missed (fitted).
pub const RELEASE_WINDOW_TICKS: f64 = 3.;
/// A held trigger releases once a pass: not again while a bomb this station
/// released this long ago or less is still falling (fitted).
pub const RELEASE_GUARD_TICKS: u64 = 120;
/// A rocket's fly-out is cut off here; its lifetime ends it sooner.
const ROCKET_TICKS: u64 = 120 * 60;

/// The bomb sight's mode, chosen by Ctrl+M (docs/spec/air-to-ground.md,
/// "Status word and mode choice").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeliveryMode {
    /// The dive pipper: the retail continuously computed impact point.
    #[default]
    Ccip,
    /// The level release "T".
    Level,
}
crate::checkpoint_enum!(DeliveryMode { Ccip = 0, Level = 1 });

impl DeliveryMode {
    /// The HUD status word.
    pub fn label(self) -> &'static str {
        match self {
            Self::Ccip => "CCIP",
            Self::Level => "LVL",
        }
    }
    /// The other mode: Ctrl+M.
    pub fn toggled(self) -> Self {
        match self {
            Self::Ccip => Self::Level,
            Self::Level => Self::Ccip,
        }
    }
    /// The mode a bomb starts in when it is selected: LVL with a ground target
    /// designated and the flight path (radians above the horizon) within
    /// [`LEVEL_CHOICE_DEGREES`] of level, else CCIP (fitted).
    pub fn chosen(ground_target: bool, flight_path: f64) -> Self {
        if ground_target && flight_path.abs() <= LEVEL_CHOICE_DEGREES.to_radians() {
            Self::Level
        } else {
            Self::Ccip
        }
    }
}

/// An advisory the sight raises in the readiness slot (opinionated, agent,
/// decision 15). None of them stops a release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Warning {
    /// The predicted fall is shorter than the record's fuze arm time.
    Low,
    /// The aircraft, flying on along its track without descending
    /// ([`flown`]), would be inside the record's collateral radius of the
    /// impact when the round bursts.
    Frag,
    /// LVL: the target is behind the impact point.
    Overfly,
}

impl Warning {
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Frag => "FRAG",
            Self::Overfly => "OVERFLY",
        }
    }
}

/// Retail `_Dist` (`0x4c66cc`): the largest axis difference plus a quarter of
/// each of the other two. The pipper's arc compares this, not the true
/// distance, with the weapon's range.
pub fn retail_distance(a: Vector, b: Vector) -> f64 {
    let mut d: [f64; 3] = std::array::from_fn(|i| (a[i] - b[i]).abs());
    d.sort_by(f64::total_cmp);
    d[2] + (d[0] + d[1]) / 4.
}

/// How many steps of the pipper's inner arc show at `distance` from a weapon
/// whose zone-1 maximum range is `maximum` (retail data, `0x409886` to
/// `0x4098dd`): all [`ARC_STEPS`] inside 0.4 of the maximum, then one fewer
/// each fourteenth of the remaining 0.6, down to 1 just short of the maximum,
/// and none (the bare ring) at the maximum and beyond.
pub fn arc_steps(distance: f64, maximum: f64) -> u8 {
    if maximum.is_nan() || maximum <= 0. || !distance.is_finite() || distance >= maximum {
        return 0;
    }
    let near = maximum * 0.4;
    if distance <= near {
        return ARC_STEPS;
    }
    // Retail `0x2e + trunc(14 (0.4 R - d) / (0.6 R))`, truncated toward zero.
    let lost = (14. * (distance - near) / (maximum - near)).floor() as i32;
    (14 - lost).clamp(1, 14) as u8
}

/// The impact point and its range arc, for a bomb or a rocket.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ccip {
    /// Where the round would leave the aircraft now, without a bomb's scatter:
    /// a bomb's fall line dots are `BombMotion::new(release).position(t)`.
    pub release: ReleaseState,
    /// The mean impact on the ground under the release point.
    pub impact: Vector,
    /// Seconds from release to impact.
    pub seconds: f64,
    /// Retail `_Dist` from the aircraft to the impact, feet: the arc's range.
    pub range_ft: f64,
    /// The true slant range to the impact, feet: the HUD's `R`.
    pub slant_ft: f64,
    /// The record's zone-1 maximum range, feet.
    pub maximum_ft: f64,
    /// Steps of the inner arc, 0 to [`ARC_STEPS`]: glyph `0x20 + arc`.
    pub arc: u8,
}

impl Ccip {
    fn new(launcher: &Launcher, release: ReleaseState, impact: Impact, weapon: &Weapon) -> Self {
        let maximum_ft = f64::from(weapon.seeker.zones[1].maximum_range.max(0));
        let range_ft = retail_distance(launcher.position, impact.position);
        Self {
            release,
            impact: impact.position,
            seconds: impact.seconds,
            range_ft,
            slant_ft: length(sub(impact.position, launcher.position)),
            maximum_ft,
            arc: arc_steps(range_ft, maximum_ft),
        }
    }
    /// Inside the record's zone-1 maximum range: IN RNG.
    pub fn in_range(&self) -> bool {
        self.maximum_ft > 0. && self.range_ft < self.maximum_ft
    }
    /// The retail symbol glyph for the pipper, `0x20` to `0x2f`.
    pub fn glyph(&self) -> u8 {
        0x20 + self.arc.min(ARC_STEPS)
    }
    /// The fraction of the inner arc drawn, 0 to 1.
    pub fn arc_fraction(&self) -> f64 {
        f64::from(self.arc.min(ARC_STEPS)) / f64::from(ARC_STEPS)
    }
    /// A bomb's fall line as retail's dot trail draws it: its mean path every
    /// [`TRAIL_SECONDS`] from release to impact.
    pub fn trail(&self) -> impl Iterator<Item = Vector> + '_ {
        let motion = ballistics::BombMotion::new(self.release);
        let steps = (self.seconds / TRAIL_SECONDS).floor().max(0.) as usize;
        (0..=steps.min(4096)).map(move |k| motion.position(k as f64 * TRAIL_SECONDS))
    }
}

/// The level release solution: the impact on the target's ground against
/// the target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelRelease {
    /// The target point on the ground, where a moving target will be when
    /// the bomb lands (led by the fall).
    pub target: Vector,
    /// D: how far the target lies beyond the impact point (on the target's
    /// ground) along the impact's ground track, feet; negative once it is
    /// overflown.
    pub along_ft: f64,
    /// C: how far the target lies right of that track, feet.
    pub cross_ft: f64,
    /// D over the ground speed, seconds, assuming the flight path is held
    /// (fitted); negative once overflown.
    pub release_seconds: f64,
    /// The turn that would put the impact track through the target, radians,
    /// positive right: the stem stands this far from the flight path marker.
    pub steering_rad: f64,
    /// The slant range to the target, feet: the HUD's `R`.
    pub slant_ft: f64,
    /// The miss a held trigger accepts: the larger of the record's collateral
    /// and fuze radii, feet (decision 2 names the collateral radius; the fuze
    /// radius is added so a record without collateral still auto-releases,
    /// fitted).
    pub radius_ft: f64,
    /// Ground speed, feet a second.
    pub ground_speed_fps: f64,
}

impl LevelRelease {
    /// The ground travel of one tick, feet.
    fn tick_ft(&self) -> f64 {
        self.ground_speed_fps / 120.
    }
    /// The target is behind the impact point: OVERFLY.
    pub fn overflown(&self) -> bool {
        self.along_ft < 0.
    }
    /// LVL holds a bomb's trigger for the cue: the cue has not passed by more
    /// than the release window. Past it a press releases at once.
    pub fn takes_trigger(&self) -> bool {
        self.along_ft > -RELEASE_WINDOW_TICKS * self.tick_ft()
    }
    /// A held trigger releases now: the impact point has reached the target
    /// (within the release window) with the cross-track miss inside
    /// [`Self::radius_ft`].
    pub fn due(&self) -> bool {
        self.along_ft <= 0. && self.takes_trigger() && self.cross_ft.abs() <= self.radius_ft
    }
    /// Where the release marker rides on the stem: 1 at the top while the
    /// release is [`MARKER_SECONDS`] or more away, 0 on the crossbar.
    pub fn marker_fraction(&self) -> f64 {
        (self.release_seconds / MARKER_SECONDS).clamp(0., 1.)
    }
    /// More than [`DASHED_SECONDS`] out: the stem is dashed.
    pub fn far(&self) -> bool {
        self.release_seconds > DASHED_SECONDS
    }
}

/// What the HUD's bomb or rocket sight shows for the selected station.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
    /// The station it is for.
    pub station: usize,
    /// The bomb sight's mode; always CCIP for rockets.
    pub mode: DeliveryMode,
    /// A rocket pod (ARM status, rocket pipper) rather than a bomb.
    pub rocket: bool,
    /// The impact point, when the round comes down at all.
    pub ccip: Option<Ccip>,
    /// The level solution, in LVL with a ground target.
    pub level: Option<LevelRelease>,
    /// Advisories, in [`Warning`] order.
    pub warnings: Vec<Warning>,
    /// Seconds until the last bomb this aircraft released lands: `IMP`.
    pub falling: Option<f64>,
}

impl Delivery {
    /// While LVL holds `station`'s trigger, whether a held trigger releases
    /// now; `None` when the trigger works as a plain press.
    pub fn level_trigger(&self, station: usize) -> Option<bool> {
        (self.station == station && self.mode == DeliveryMode::Level && !self.rocket)
            .then_some(self.level.as_ref())
            .flatten()
            .filter(|level| level.takes_trigger())
            .map(LevelRelease::due)
    }
    /// The first warning, for the readiness slot.
    pub fn warning(&self) -> Option<Warning> {
        self.warnings.first().copied()
    }
    /// Where the selected round comes down at the ground height `height`:
    /// for a bomb its mean path brought down to that height, for a rocket its
    /// flown impact (a rocket is already flown into the real terrain). The
    /// level T judges a bomb against its target's ground this way; the CCIP
    /// pipper and the AI's CCIP cue work out where the path meets the
    /// terrain instead ([`Delivery::cue`]). `None` without an impact.
    pub fn impact_at(&self, height: f64) -> Option<Impact> {
        let ccip = self.ccip.as_ref()?;
        if self.rocket {
            return Some(Impact {
                position: ccip.impact,
                seconds: ccip.seconds,
            });
        }
        ballistics::impact_at(ccip.release, height)
    }
    /// The impact the AI's release is judged on (docs/spec/ai-strike.md): in
    /// LVL the bomb's mean path brought down to the target's ground
    /// (`target_height`, as the level T is timed); otherwise the pipper's own
    /// impact, where the path meets the terrain. `None` without one.
    pub fn cue(&self, target_height: f64) -> Option<Impact> {
        if self.mode == DeliveryMode::Level && !self.rocket {
            return self.impact_at(target_height);
        }
        self.ccip.as_ref().map(|ccip| Impact {
            position: ccip.impact,
            seconds: ccip.seconds,
        })
    }
}

/// Combat's clock at the step being solved, so a rocket's fly-out takes the
/// same service steps the round would.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    /// The step's tick before it advanced: its quarter second is `tick / 30`.
    pub tick: u64,
    /// The service units this step moves rounds by.
    pub service: i16,
    /// The service remainder after this step.
    pub remainder: u16,
}

impl Clock {
    fn next(self) -> Self {
        let total = self.remainder + 256;
        Self {
            tick: self.tick + 1,
            service: (total / 120) as i16,
            remainder: total % 120,
        }
    }
}

/// What [`solve`] needs to know of the ownship this tick.
#[derive(Clone, Copy)]
pub struct Situation<'a> {
    pub index: usize,
    pub station: &'a Station,
    /// Where the store that leaves next hangs, feet right, up and forward
    /// of the aircraft: its side of a paired station
    /// ([`super::hardpoints::Rack::next_release_mount`]). The sight predicts
    /// from it (B3 follow-up, John 2026-10-12).
    pub mount: Vector,
    pub launcher: &'a Launcher,
    pub mode: DeliveryMode,
    /// The designated ground target's position, if any.
    pub target: Option<Vector>,
    /// Its velocity, feet a second (zero for a pinned point): the level T
    /// leads a moving target by its fall.
    pub target_velocity: Vector,
    pub clock: Clock,
    /// Seconds until the aircraft's last released bomb lands.
    pub falling: Option<f64>,
    /// A laser glide bomb is solved as a bomb on the bomb equation, the way
    /// an AI that drops it by hand with no spot sees it (slice ST-G). Never
    /// set for a human's HUD, which shows a guided bomb the guided sight.
    pub ballistic: bool,
    /// A rocket's pipper carries on past the round's lifetime to the ground
    /// ([`rocket_impact_beyond`]): set for a human's HUD, never for the AI,
    /// whose release gate reads the in-life impact only.
    pub beyond_life: bool,
}

/// The solution for the selected station, or `None` when it holds neither an
/// unguided bomb nor a rocket pod.
pub fn solve(s: Situation<'_>, ground: impl Fn(f64, f64) -> f64) -> Option<Delivery> {
    let w = &s.station.weapon;
    let bomb = ballistics::is_bomb(w) || (s.ballistic && super::guided::is_glide_bomb(w));
    let rocket = !bomb && ballistics::is_pod(w) && w.seeker.signature == 0;
    if !bomb && !rocket {
        return None;
    }
    let ccip = if bomb {
        let release = ReleaseState::from_mount(s.launcher, s.station, s.mount, 0);
        predict_impact(release, &ground).map(|impact| Ccip::new(s.launcher, release, impact, w))
    } else {
        let fly = if s.beyond_life {
            rocket_impact_beyond
        } else {
            rocket_impact
        };
        fly(w, s.launcher, s.mount, s.clock, &ground)
            .map(|(release, impact)| Ccip::new(s.launcher, release, impact, w))
    };
    let mode = if bomb { s.mode } else { DeliveryMode::Ccip };
    let level = (mode == DeliveryMode::Level)
        .then(|| {
            let release = ccip.as_ref()?.release;
            let (led, impact) = led_impact(release, s.target?, s.target_velocity, &ground)?;
            level_release(s.launcher, w, release.position, impact, led)
        })
        .flatten();
    let mut warnings = Vec::new();
    if let Some(c) = &ccip {
        if c.seconds < f64::from(w.damage.fuze_arm_t) / 4. {
            warnings.push(Warning::Low);
        }
        let radius = f64::from(w.damage.collateral_radius.max(0));
        if radius > 0. && length(sub(flown(s.launcher, c.seconds), c.impact)) < radius {
            warnings.push(Warning::Frag);
        }
    }
    if level.is_some_and(|l| l.overflown()) {
        warnings.push(Warning::Overfly);
    }
    Some(Delivery {
        station: s.index,
        mode,
        rocket,
        ccip,
        level,
        warnings,
        falling: s.falling,
    })
}

/// Where FRAG takes the aircraft to be `seconds` on: along its ground track
/// at its ground speed, and no lower than it is now, so a dive is taken to be
/// pulled out at once while a climb keeps climbing (fitted; flying the dive
/// on would put every dive release inside its own splash).
pub fn flown(launcher: &Launcher, seconds: f64) -> Vector {
    let p = launcher.position;
    let v = launcher.velocity;
    [
        p[0] + v[0] * seconds,
        p[1] + (v[1] * seconds).max(0.),
        p[2] + v[2] * seconds,
    ]
}

/// Where a bomb released as `release` lands on the target's ground, and where
/// the target will be by then: a target moving at `velocity` is led by the
/// fall (the fall to its ground, then once more to the ground at the led
/// point). Agent decision (slice ST-LV): the T is a release cue for a
/// designated object, and without the lead a column moving at 50 ft/s drew
/// the bombs some 900 ft behind it from 5,000 ft. A standing target or a
/// pinned point is not led.
fn led_impact(
    release: ReleaseState,
    target: Vector,
    velocity: Vector,
    ground: &impl Fn(f64, f64) -> f64,
) -> Option<(Vector, Vector)> {
    let first = ballistics::impact_at(release, ground(target[0], target[2]))?;
    let led = [
        target[0] + velocity[0] * first.seconds,
        target[1],
        target[2] + velocity[2] * first.seconds,
    ];
    if led == target {
        return Some((target, first.position));
    }
    let impact = ballistics::impact_at(release, ground(led[0], led[2]))?;
    let led = [
        target[0] + velocity[0] * impact.seconds,
        target[1],
        target[2] + velocity[2] * impact.seconds,
    ];
    Some((led, impact.position))
}

/// The level solution for a bomb released at `from` landing at `impact` (on
/// the target's ground), against `target` (led by the fall).
fn level_release(
    launcher: &Launcher,
    weapon: &Weapon,
    from: Vector,
    impact: Vector,
    target: Vector,
) -> Option<LevelRelease> {
    let throw = [impact[0] - from[0], impact[2] - from[2]];
    // The impact's ground track: from the release point through the impact,
    // or along the ground speed when the bomb falls straight down.
    let track = if throw[0].hypot(throw[1]) > 1. {
        throw
    } else {
        [launcher.velocity[0], launcher.velocity[2]]
    };
    let norm = track[0].hypot(track[1]);
    if norm.is_nan() || norm <= 1e-9 {
        return None;
    }
    let u = [track[0] / norm, track[1] / norm];
    let right = [u[1], -u[0]];
    let gap = [target[0] - impact[0], target[2] - impact[2]];
    let along_ft = gap[0] * u[0] + gap[1] * u[1];
    let cross_ft = gap[0] * right[0] + gap[1] * right[1];
    let ground_speed_fps = launcher.velocity[0].hypot(launcher.velocity[2]);
    let release_seconds = if ground_speed_fps > 1. {
        along_ft / ground_speed_fps
    } else if along_ft >= 0. {
        f64::INFINITY
    } else {
        f64::NEG_INFINITY
    };
    let to_target = [target[0] - from[0], target[2] - from[2]];
    let steering_rad = wrap(to_target[0].atan2(to_target[1]) - u[0].atan2(u[1]));
    let radius_ft = f64::from(
        weapon
            .damage
            .collateral_radius
            .max(weapon.damage.fuze_radius)
            .max(0),
    );
    Some(LevelRelease {
        target,
        along_ft,
        cross_ft,
        release_seconds,
        steering_rad,
        slant_ft: length(sub(target, launcher.position)),
        radius_ft,
        ground_speed_fps,
    })
}

/// Where a rocket fired from `mount` (feet right, up and forward) now meets the ground, and the state it
/// leaves in: the unguided round's own steps, tick by tick (the speed command
/// of flag 0x40, travel along the launch direction by the service units, the
/// gravity flag's sink), from the clock of this step, against the terrain
/// test combat uses. `None` when its lifetime ends first. The AI's release
/// gate reads this one.
pub fn rocket_impact(
    weapon: &Weapon,
    launcher: &Launcher,
    mount: Vector,
    clock: Clock,
    ground: &impl Fn(f64, f64) -> f64,
) -> Option<(ReleaseState, Impact)> {
    fly_rocket(weapon, launcher, mount, clock, ground, false)
}

/// The rocket pipper's point for a human's HUD: [`rocket_impact`] when the
/// round reaches the ground in its life, else the same flight carried on past
/// the lifetime (the 20 s of the rocket records) in coarse steps until it
/// meets the terrain, so the pipper has a ground point however high or flat
/// the pass (opinionated, John 2026-10-12: "the rocket pipper should always
/// be visible"; the continuation and its limits are the agent's). When it
/// does not come down within [`BEYOND_SECONDS`] (a nose above the horizon, a
/// very high pass) or leaves the world's height, the point is where the round
/// stood when its life ended. `None` only when the round's motion cannot be
/// worked out.
pub fn rocket_impact_beyond(
    weapon: &Weapon,
    launcher: &Launcher,
    mount: Vector,
    clock: Clock,
    ground: &impl Fn(f64, f64) -> f64,
) -> Option<(ReleaseState, Impact)> {
    fly_rocket(weapon, launcher, mount, clock, ground, true)
}

/// How long the pipper's continuation flies on, seconds (fitted).
pub const BEYOND_SECONDS: f64 = 300.;
/// The continuation's step, in combat's 120 Hz ticks: a tenth of a second
/// (fitted). Inside the lifetime the steps are combat's own.
const BEYOND_STEP_TICKS: u64 = 12;

fn fly_rocket(
    weapon: &Weapon,
    launcher: &Launcher,
    mount: Vector,
    clock: Clock,
    ground: &impl Fn(f64, f64) -> f64,
    beyond: bool,
) -> Option<(ReleaseState, Impact)> {
    let m = &weapon.movement;
    let basis = launcher.basis;
    let mut position: Vector = std::array::from_fn(|i| {
        launcher.position[i]
            + basis.right[i] * mount[0]
            + basis.up[i] * mount[1]
            + basis.forward[i] * mount[2]
    });
    let direction = basis.forward;
    let mut speed_f8 = launch_speed(m, (launcher.speed_fps * 256.) as i32).ok()? * 256;
    let release = ReleaseState::new(position, direction, f64::from(speed_f8) / 256.);
    let launched_t = (clock.tick / 30) as u16;
    let mut fall = FallState::default();
    let mut clock = clock;
    // Ticks flown, fractional once the steps go coarse.
    let mut elapsed = 0.;
    // Where the round stood when its lifetime ended, the pipper's fallback.
    let mut life_end = None;
    let limit = if beyond {
        BEYOND_SECONDS * 120.
    } else {
        ROCKET_TICKS as f64
    };
    while elapsed < limit {
        let now = (clock.tick / 30) as u16;
        let height_f8 = (position[1] * 256.) as i32;
        let expired = removal_due(m, now, launched_t, height_f8);
        let coarse = expired && now.wrapping_sub(launched_t) >= m.remove_t;
        if expired && !beyond {
            return None;
        }
        if coarse && life_end.is_none() {
            life_end = Some(Impact {
                position,
                seconds: elapsed / 120.,
            });
        }
        if expired && height_f8 > 0x0186_a000 {
            break;
        }
        let (service, ticks) = if coarse {
            // 256 service units are a second: a step of `BEYOND_STEP_TICKS`.
            ((BEYOND_STEP_TICKS * 256 / 120) as i16, BEYOND_STEP_TICKS)
        } else {
            (clock.service, 1)
        };
        let phase = engine_phase(m, now, launched_t);
        if weapon.flags & 0x40 != 0 {
            let target = commanded_speed(m, phase, speed_f8, (position[1] * 256.) as i32) as i16;
            speed_f8 = axial_speed(m, speed_f8, target, false, service).ok()?;
        }
        let previous = position;
        let distance = f64::from(speed_f8) * f64::from(service) / 65536.;
        for i in 0..3 {
            position[i] += direction[i] * distance;
        }
        position[1] = f64::from(
            fall.advance(
                weapon.flags & 4 != 0,
                phase,
                service,
                (position[1] * 256.) as i32,
            )
            .ok()?,
        ) / 256.;
        if let Some(at) = super::live::terrain_hit(previous, position, ground) {
            let point = std::array::from_fn(|i| previous[i] + (position[i] - previous[i]) * at);
            return Some((
                release,
                Impact {
                    position: point,
                    seconds: (elapsed + at * ticks as f64) / 120.,
                },
            ));
        }
        elapsed += ticks as f64;
        clock = if coarse {
            Clock {
                tick: clock.tick + ticks,
                ..clock
            }
        } else {
            clock.next()
        };
    }
    // No ground within reach of the continuation (a nose above the horizon,
    // or too high and flat to come down in time): the pipper marks where the
    // round would be when its life ended, so it is still on the HUD.
    life_end.map(|impact| (release, impact))
}

/// A guided bomb's reach: the nearest and farthest ground range ahead of a
/// release at which it can still hit, for the HUD's release range scale
/// (docs/spec/air-to-ground.md, "Guided bomb release range indicator").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    pub minimum_ft: f64,
    pub maximum_ft: f64,
}

impl Reach {
    /// The ground range bisection stops at, feet.
    pub const TOLERANCE_FT: f64 = 10.;

    /// Whether a target `range_ft` ahead lies inside the reach: IN RNG.
    pub fn contains(&self, range_ft: f64) -> bool {
        (self.minimum_ft..=self.maximum_ft).contains(&range_ft)
    }
    /// Where `range_ft` sits on the scale: 0 at the minimum, 1 at the maximum,
    /// clamped.
    pub fn fraction(&self, range_ft: f64) -> f64 {
        let span = self.maximum_ft - self.minimum_ft;
        if span > 0. {
            ((range_ft - self.minimum_ft) / span).clamp(0., 1.)
        } else {
            0.
        }
    }
    /// Searches the reach of a guided bomb released as `release` over ground
    /// at height `ground`, out to `limit_ft`. `hits(range)` flies the guided
    /// motion (the caller's, slice W1) at a point `range` feet ahead along the
    /// release's ground track and says whether it gets there; the reach is
    /// taken to be one interval about the unguided throw, which the bomb
    /// always makes good. `None` when even that throw is missed.
    ///
    /// Deterministic: the same release and fly-out give the same reach, in a
    /// bounded number of fly-outs (an outward doubling, then bisection to
    /// [`Self::TOLERANCE_FT`]).
    pub fn search(
        release: ReleaseState,
        ground: f64,
        limit_ft: f64,
        mut hits: impl FnMut(f64) -> bool,
    ) -> Option<Self> {
        let impact = predict_impact(release, |_, _| ground)?;
        let throw = (impact.position[0] - release.position[0])
            .hypot(impact.position[2] - release.position[2]);
        let limit = limit_ft.max(throw);
        if !hits(throw) {
            return None;
        }
        let step = (throw * 0.25).max(500.);
        let maximum = edge(throw, limit, step, &mut hits);
        let minimum = edge(throw, 0., step, &mut hits);
        Some(Self {
            minimum_ft: minimum,
            maximum_ft: maximum,
        })
    }
}

/// The last range `hits` accepts going from `start` (accepted) toward `end`:
/// steps that double, then bisection to [`Reach::TOLERANCE_FT`].
fn edge(start: f64, end: f64, step: f64, hits: &mut impl FnMut(f64) -> bool) -> f64 {
    let sign = if end >= start { 1. } else { -1. };
    let (mut good, mut step) = (start, step);
    let bad = loop {
        let next = good + sign * step;
        if (next - end) * sign >= 0. {
            if hits(end) {
                return end;
            }
            break end;
        }
        if !hits(next) {
            break next;
        }
        good = next;
        step *= 2.;
    };
    let mut bad = bad;
    for _ in 0..64 {
        if (bad - good).abs() <= Reach::TOLERANCE_FT {
            break;
        }
        let mid = (good + bad) / 2.;
        if hits(mid) {
            good = mid;
        } else {
            bad = mid;
        }
    }
    good
}

fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}

fn length(v: Vector) -> f64 {
    v.iter().map(|c| c * c).sum::<f64>().sqrt()
}

fn wrap(angle: f64) -> f64 {
    (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

#[cfg(test)]
#[path = "delivery_tests.rs"]
mod tests;
