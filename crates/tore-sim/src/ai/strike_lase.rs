//! AI lasing (docs/spec/ai-strike.md, "Lasing", slice ST-L): the hold that
//! keeps an AI designator's laser spot on its target through a laser bomb's
//! fall, the lase duty other code can give an AI (a wing order, a wingmate's
//! bomb), and the few facts the host and the mission hand over for it.
//!
//! One [`Lase`] per lasing aircraft, inside its [`Strike`]. It has three
//! sources, one behaviour:
//!
//! - **Its own bomb.** An AI that releases a laser weapon with a designator
//!   of its own goes from the pass to [`Phase::Lase`] instead of the exit,
//!   and holds the spot on the target until the bomb is down.
//! - **A duty** ([`Strike::lase_for`]): "lase target X for N seconds, until
//!   the weapons guided on my spot have landed, or until the target dies".
//!   This is the API the wing order "Lase my target" calls.
//! - **The wing** (fitted): a designator aircraft with no store of its own
//!   to use lases the target of its wing's laser-bomb carriers, which have
//!   no designator.
//!
//! The hold is an offset turn: an orbit about the target of radius
//! max(3 nm, 1.2 turn radii) (spec, "Lase my target"), flown at a speed and
//! with a bank that keep the target inside the designator's gimbal. A pod's
//! gimbal is the hemisphere under the aircraft's wings, so the target must
//! stay below the wing plane: banked toward the target by `phi`, a target
//! `delta` below the horizon at azimuth `theta` from the nose is under the
//! wings while `tan(phi) <= tan(delta) / sin(theta)` (derived from the
//! body-frame elevation of the target). A built-in sensor's gimbal is a cone
//! about the nose, so it holds the nose on the target instead. The numbers
//! are fitted (agent); the radius is the spec's.
use super::*;
use crate::ai::ScalarSpeed;
use crate::combat::laser::Mask;
use crate::combat::sight::Gimbal;

/// The orbit's smallest radius (spec, "Lase my target").
pub const LASE_RADIUS_MIN_FT: f64 = 3. * FEET_PER_NAUTICAL_MILE;
/// The orbit's radius in turn radii at the hold's speed and the aircraft's G
/// limit, when that is more than [`LASE_RADIUS_MIN_FT`] (spec).
pub const LASE_RADIUS_TURNS: f64 = 1.2;
/// The orbit stays inside this share of the laser's reach (fitted).
pub const LASE_REACH_SHARE: f64 = 0.8;
/// Bank held back from the gimbal's limit, degrees (fitted).
pub const LASE_BANK_MARGIN_DEG: f64 = 5.;
/// The hold is not judged finished before this long (fitted): a bomb
/// released this tick is in the projectile list only from the next one.
pub const LASE_MIN_TICKS: u64 = 2 * 120;
/// A duty "until impact" with no weapon ever guided on it ends after this
/// long (fitted).
pub const LASE_WAIT_TICKS: u64 = 120 * 120;
/// A hold is never longer than this, except a duty "until the target
/// dies" (fitted).
pub const LASE_MAX_TICKS: u64 = 120 * 120;
/// A built-in sensor's hold stops this close to the target, feet, where the
/// target leaves the cone (fitted; the same distance as a guided pass's
/// exit).
pub const LASE_CONE_EXIT_FT: f64 = GUIDED_EXIT_FT;
/// How much the radius error turns the course, degrees per radius of error
/// (fitted), within -45 and +90 degrees.
const RADIAL_DEG: f64 = 60.;
/// The commanded heading change is at most this many degrees per second
/// away from the target's side (fitted).
const AWAY_DEG: f64 = 30.;
/// Standard gravity, ft/s^2, the adapter's own.
const G: f64 = 32.174;

/// How long a lase duty lasts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LaseUntil {
    /// This many seconds from the first step of the duty.
    Seconds(f64),
    /// Until every weapon guided on this aircraft's spot has landed (after
    /// at least one was); a duty none ever uses ends after two minutes.
    Impact,
    /// Until the target is destroyed, or the duty is cancelled.
    TargetDies,
}

/// What ends a lase (the stored form of [`LaseUntil`] and the other two
/// sources).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaseEnd {
    Timed,
    Impact,
    Dies,
}

/// The sensor a designator lases through, as the host describes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaseSensor {
    pub gimbal: Gimbal,
    /// The laser's reach in clear air, feet.
    pub reach_ft: f64,
}

/// What the host tells a striker about its laser and the spots around it
/// each step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaseInputs {
    /// The designator it lases with; `None` for an aircraft without one.
    pub sensor: Option<LaseSensor>,
    /// Why the laser was masked at the last step, if it was.
    pub mask: Option<Mask>,
    /// One of its own laser weapons is in flight and has not gone blind.
    pub own_flight: bool,
    /// A laser weapon of its side is guiding on this aircraft's spot.
    pub on_spot: bool,
    /// The objects a same-side spot of another aircraft lies on: a buddy's
    /// spot a laser carrier may drop on.
    pub spot_targets: Vec<u32>,
    /// Where the lased target stands and whether it still stands.
    pub target: Option<(Vector, bool)>,
}

/// What the mission tells a striker of its wing each step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WingFacts {
    /// A wing member lives with a designator of its own.
    pub designator: bool,
    /// A wing member with laser rounds and no designator lives: it needs a
    /// spot.
    pub carriers: bool,
    /// The target of such a member that has chosen it.
    pub escort: Option<u32>,
}

/// One lase in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lase {
    pub target: u32,
    pub(super) end: LaseEnd,
    /// The length of a timed lase, ticks.
    pub(super) ticks: u64,
    /// The tick the first step of the hold saw.
    pub(super) since: Option<u64>,
    /// The orbit turns toward the target: +1 right, -1 left.
    pub(super) side: f64,
    /// The altitude held, feet above sea level.
    pub(super) hold_ft: Option<f64>,
    /// The aircraft's own bomb: the hold ends in the exit.
    pub(super) own: bool,
    /// For the wing's carriers.
    pub(super) wing: bool,
    /// A weapon was seen guiding on the spot.
    pub(super) guided: bool,
}

impl Lase {
    fn new(target: u32, end: LaseEnd, ticks: u64, own: bool, wing: bool) -> Self {
        Self {
            target,
            end,
            ticks,
            since: None,
            side: 1.,
            hold_ft: None,
            own,
            wing,
            guided: false,
        }
    }
}

/// The orbit's radius: max(3 nm, 1.2 turn radii), kept inside the laser's
/// reach.
pub fn lase_radius_ft(speed_fps: f64, g_limit: f64, reach_ft: f64) -> f64 {
    let radius = LASE_RADIUS_MIN_FT.max(LASE_RADIUS_TURNS * turn_radius_ft(speed_fps, g_limit));
    if reach_ft > 0. {
        radius.min(LASE_REACH_SHARE * reach_ft)
    } else {
        radius
    }
}

/// The most the aircraft may bank toward a target `height_ft` below it and
/// `distance_ft` away, at azimuth `theta_deg` from the nose on the turn's
/// side, and still have the target under the wings (a pod's hemisphere),
/// less [`LASE_BANK_MARGIN_DEG`]. Zero when the target is not below.
pub fn bank_cap_deg(height_ft: f64, distance_ft: f64, theta_deg: f64) -> f64 {
    let delta = height_ft.atan2(distance_ft.max(1.));
    if delta <= 0. {
        return 0.;
    }
    let s = theta_deg.to_radians().sin();
    let cap = if s > 1e-3 {
        (delta.tan() / s).atan()
    } else {
        std::f64::consts::FRAC_PI_2
    };
    (cap.to_degrees() - LASE_BANK_MARGIN_DEG).clamp(0., 80.)
}

/// The speed at which a level orbit of `radius_ft` about a target
/// `height_ft` below keeps the target under the wings abeam (the bank of
/// the orbit, `v^2 / (g r)`, within the target's depression less the
/// margin), at most `corner_fps`.
pub fn orbit_speed_fps(height_ft: f64, radius_ft: f64, corner_fps: f64) -> f64 {
    let delta = height_ft.atan2(radius_ft.max(1.)).to_degrees() - LASE_BANK_MARGIN_DEG;
    if delta <= 0. {
        return corner_fps;
    }
    (G * radius_ft * delta.to_radians().tan())
        .sqrt()
        .min(corner_fps)
}

/// The heading change per second, degrees, a bank of `bank_deg` gives at
/// `speed_fps`: what the adapter's heading-error law turns into that bank
/// (`bank = atan(v * rate / g)`, a one second horizon).
pub fn turn_rate_deg(bank_deg: f64, speed_fps: f64) -> f64 {
    (G * bank_deg.to_radians().tan() / speed_fps.max(1.)).to_degrees()
}

impl Strike {
    /// Gives the aircraft a lase duty on `target` (the wing order "Lase my
    /// target", slice ST-O). It takes the flying from the next step: the
    /// orbit, the spot on the target, the end. A threat that makes the
    /// aircraft defend ends it. The aircraft needs a laser designator
    /// ([`LaseInputs::sensor`]); without one the duty ends at its first
    /// step.
    pub fn lase_for(&mut self, target: u32, until: LaseUntil) {
        let (end, ticks) = match until {
            LaseUntil::Seconds(s) => (LaseEnd::Timed, (s.max(0.) * 120.).round() as u64),
            LaseUntil::Impact => (LaseEnd::Impact, 0),
            LaseUntil::TargetDies => (LaseEnd::Dies, 0),
        };
        self.lase = Some(Lase::new(target, end, ticks, false, false));
        self.withdrawn = false;
        if self.phase != Phase::Defend {
            self.phase = Phase::Lase;
            self.segment = Segment::Approach;
        }
    }

    /// Ends a lase duty (the order changed). A hold on the aircraft's own
    /// bomb is not a duty and stays. Returns whether one ended.
    pub fn stop_lase(&mut self) -> bool {
        match self.lase {
            Some(l) if !l.own => {
                self.lase = None;
                if self.phase == Phase::Lase {
                    self.phase = Phase::Ingress;
                    self.segment = Segment::Approach;
                }
                true
            }
            _ => false,
        }
    }

    /// The AI turned the laser on by hand in a cockpit a human left (so it
    /// turns it off again, and leaves a human's own setting alone).
    pub fn laser_set(&self) -> bool {
        self.laser_set
    }

    /// Records whether the AI turned the laser on by hand.
    pub fn set_laser_set(&mut self, set: bool) {
        self.laser_set = set;
    }

    /// The object the aircraft is lasing for, if it has a lase in progress.
    pub fn lasing(&self) -> Option<u32> {
        self.lase.map(|l| l.target)
    }

    /// The object whose laser the cockpit must point its sight and its laser
    /// at now: the hold's target, or the target of a laser pass with a
    /// designator of its own. `None` when the laser is not wanted.
    pub fn laser_target(&self) -> Option<u32> {
        match self.phase {
            Phase::Lase => self.lase.map(|l| l.target),
            Phase::Attack if self.kind == Some(StoreKind::Laser) && self.designator => self.target,
            _ => None,
        }
    }

    /// The target of a laser carrier without a designator that has chosen
    /// one: a spot is needed there (the wing's designator lases for it).
    pub fn needs_spot(&self) -> Option<u32> {
        (self.kind == Some(StoreKind::Laser)
            && !self.designator
            && !matches!(self.phase, Phase::Home | Phase::Defend))
        .then_some(self.target)
        .flatten()
    }

    /// The striker has a laser store with rounds and no designator.
    pub fn is_carrier(&self) -> bool {
        self.laser_rounds && !self.designator && self.phase != Phase::Home
    }

    /// The striker has a designator and has not gone home.
    pub fn is_designator(&self) -> bool {
        self.designator && self.phase != Phase::Home
    }

    /// What the hold does after a laser release: the bomb is in the air, the
    /// spot must stay on the target (called from the pass).
    pub(super) fn hold_after_release(&mut self, cx: &Context<'_>) -> bool {
        let Some(target) = self.target else {
            return false;
        };
        if self.kind != Some(StoreKind::Laser) || cx.inputs.lase.sensor.is_none() {
            return false;
        }
        self.lase = Some(Lase::new(target, LaseEnd::Impact, 0, true, false));
        self.enter(Phase::Lase, cx.tick);
        true
    }

    /// The lase part of a step: starts the wing's lase when it is due, and
    /// flies the hold. `None` when the aircraft is not lasing this step and
    /// the ordinary step goes on.
    pub(super) fn step_lase(&mut self, cx: &Context<'_>) -> Option<Order> {
        if self.lase.is_none() && self.phase == Phase::Lase {
            self.enter(Phase::Ingress, cx.tick);
        }
        if self.lase.is_none() && self.escort_due(cx) {
            let target = cx.wing.escort?;
            self.lase = Some(Lase::new(target, LaseEnd::Impact, 0, false, true));
            self.phase = Phase::Lase;
            self.segment = Segment::Approach;
        }
        let lase = self.lase?;
        // A duty holds from wherever the aircraft was: after a defence it
        // resumes at ingress, a cone-gimbal duty turns away from the target
        // and comes back (the exit and the turn back are the ordinary ones).
        if matches!(
            self.phase,
            Phase::Ingress | Phase::Wait | Phase::Regain | Phase::TurnBack
        ) {
            self.enter(Phase::Lase, cx.tick);
        }
        if self.phase != Phase::Lase {
            return None;
        }
        let sensor = cx.inputs.lase.sensor;
        let (target_at, alive) = cx
            .inputs
            .lase
            .target
            .or_else(|| known_position(cx.picture, lase.target).map(|p| (p, true)))
            .map_or((None, false), |(p, alive)| (Some(p), alive));
        let li = &cx.inputs.lase;
        let lase = self.lase.as_mut()?;
        let since = *lase.since.get_or_insert(cx.tick);
        let elapsed = cx.tick.saturating_sub(since);
        if li.own_flight || li.on_spot {
            lase.guided = true;
        }
        let airborne = li.own_flight || li.on_spot;
        let wanted_by_wing = lase.wing && cx.wing.escort == Some(lase.target);
        let done = !alive
            || sensor.is_none()
            || match lase.end {
                LaseEnd::Timed => elapsed >= lase.ticks,
                LaseEnd::Dies => false,
                LaseEnd::Impact => {
                    elapsed >= LASE_MIN_TICKS
                        && !airborne
                        && !wanted_by_wing
                        && (lase.guided || lase.own || lase.wing || elapsed >= LASE_WAIT_TICKS)
                }
            }
            || (lase.end != LaseEnd::Dies && elapsed >= LASE_MAX_TICKS.max(lase.ticks));
        let own = lase.own;
        if done {
            self.lase = None;
            if own {
                self.exit(cx);
                let order = Order {
                    steer: Some(self.exit_steer(cx, target_at)),
                    release: None,
                    activity: Some(Activity::Attacking),
                };
                return Some(order);
            }
            self.enter(Phase::Ingress, cx.tick);
            return None;
        }
        let (sensor, target_at) = (sensor?, target_at?);
        let cone = matches!(sensor.gimbal, Gimbal::Cone { .. });
        if cone && horizontal(cx.own.position, target_at) < LASE_CONE_EXIT_FT {
            // A cone cannot follow a target it is flying over: an own bomb's
            // hold is over, a duty turns away and comes back.
            if own {
                self.lase = None;
            }
            self.exit(cx);
            return Some(Order {
                steer: Some(self.exit_steer(cx, Some(target_at))),
                release: None,
                activity: Some(Activity::Attacking),
            });
        }
        let steer = self.lase_steer(cx, target_at, sensor);
        Some(Order {
            steer: Some(steer),
            release: None,
            activity: Some(Activity::Attacking),
        })
    }

    /// The wing's lase is due: a designator with nothing of its own to
    /// strike with, and a carrier of its wing that has a target.
    fn escort_due(&self, cx: &Context<'_>) -> bool {
        cx.inputs.designator
            && cx.inputs.lase.sensor.is_some()
            && cx.wing.escort.is_some()
            && !matches!(self.phase, Phase::Home | Phase::Defend | Phase::Attack)
            && !cx
                .inputs
                .stores
                .iter()
                .any(|s| s.rounds > 0 && s.kind != StoreKind::Command)
    }

    /// Where the hold flies this step.
    fn lase_steer(&mut self, cx: &Context<'_>, target: Vector, sensor: LaseSensor) -> Steer {
        let own = cx.own;
        let masked = cx.inputs.lase.mask == Some(Mask::Gimbal);
        let ground = own.position[1] - own.agl_ft;
        let floor = self.floor(cx, self.threat);
        let lase = self.lase.as_mut().expect("a hold has its lase");
        let hold = *lase
            .hold_ft
            .get_or_insert(own.position[1].max(ground + floor));
        let pitch = hold_pitch_deg(own.position[1], hold.max(ground + floor));
        let to_target = bearing_deg(own.position, target);
        let distance = horizontal(own.position, target);
        if matches!(sensor.gimbal, Gimbal::Cone { .. }) {
            return Steer {
                heading_deg: to_target,
                pitch_deg: pitch,
                speed: SpeedRequest::Corner,
                afterburner: false,
            };
        }
        // The orbit turns toward the target's side, chosen when the hold
        // starts and kept.
        if lase.since == Some(cx.tick) {
            let off = angle_difference(own.heading_deg, to_target);
            lase.side = if off < 0. { -1. } else { 1. };
        }
        let side = lase.side;
        let speed = own.speed.0.max(100.);
        let radius = lase_radius_ft(speed, own.g_limit, sensor.reach_ft);
        let height = own.position[1] - target[1];
        // The course that follows the circle, turned toward the target when
        // outside it and away when inside.
        let alpha = ((distance - radius) / radius * RADIAL_DEG).clamp(-45., 90.);
        let course = wrap_deg(to_target - side * (90. - alpha));
        let err = angle_difference(own.heading_deg, course) * side;
        // The turn the circle itself needs, faded in over the last radius
        // of the approach.
        let fade = (2. - distance / radius).clamp(0., 1.);
        let feed = (speed / radius).to_degrees() * fade;
        let theta = angle_difference(own.heading_deg, to_target) * side;
        let cap = turn_rate_deg(bank_cap_deg(height, distance, theta), speed);
        let mut turn = (feed + err).clamp(-AWAY_DEG, cap.max(0.));
        if masked {
            turn = turn.min(0.);
        }
        let corner = own.limits.corner.0;
        let orbit = orbit_speed_fps(height, radius, corner);
        let slow = distance < 2.5 * radius && orbit < corner;
        Steer {
            heading_deg: wrap_deg(own.heading_deg + side * turn),
            pitch_deg: pitch,
            speed: if slow {
                SpeedRequest::Explicit(ScalarSpeed(orbit))
            } else {
                SpeedRequest::Corner
            },
            afterburner: false,
        }
    }
}
