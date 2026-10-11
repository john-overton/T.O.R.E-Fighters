//! Takeover (docs/spec/ai-strike.md, "Takeover and handback"; slice ST-K):
//! a strike that continues the attack a human left when it gave its
//! aircraft back to the AI.
//!
//! The host builds it from the cockpit the human left ([`Takeover`]): the
//! human's ground target, the station it had selected when that holds a
//! usable surface store, and its bomb sight mode. The strike then runs as
//! any other, with these differences (opinionated, John, 2026-10-11,
//! decisions 10 to 12, unless marked):
//!
//! - **Store**: the kept station while it has rounds, then the AI's choice.
//! - **Pass**: kept LVL flies a level run with a bomb or a guided bomb; kept
//!   CCIP takes the table's pass. The AI never changes the mode.
//! - **Safe first**: the strike attacks only while it is safe (the spec's
//!   safe test), checked every step until something leaves on the pass. If
//!   not it turns away and stands off, then
//!   resumes when safe; where an ordinary striker would go home after an
//!   unsafe defence, a takeover stands off too. The stand-off is a left
//!   circle about the target outside the reach of every known missile site
//!   covering it, plus a mile, and never inside the exit distance (fitted,
//!   agent), flown at corner speed holding height.
//! - **Guiding**: while one of its laser weapons is in flight the exit
//!   holds a level line on the release heading instead of jinking and
//!   climbing, so the target stays inside the designator's gimbal (below
//!   the wings) until impact, and no second laser weapon leaves the pass
//!   (fitted, agent).
//! - **Done**: when no target or store is left (and nothing sends it home)
//!   the strike ends and the aircraft takes up its ordinary AI role again
//!   ([`super::super::mission::AiActor`]'s strike step).
use super::*;

/// The stand-off circle lies this far outside the reach of the missile
/// sites covering the target (fitted, agent).
pub const STAND_OFF_MARGIN_FT: f64 = FEET_PER_NAUTICAL_MILE;

/// What a human left in the cockpit of an aircraft it gave back mid-attack,
/// for the AI to continue (see the module notes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Takeover {
    /// The human's ground target: its designation, or the object under its
    /// sight's pin.
    pub target: u32,
    /// The station the human had selected, when it holds a usable surface
    /// store.
    pub station: Option<StationId>,
    /// The bomb sight's mode as the human left it (Ctrl+M).
    pub mode: DeliveryMode,
    /// Standing off a target that is not safe, at this distance from it,
    /// feet; `None` while not standing off.
    stand_off_ft: Option<f64>,
}

impl Takeover {
    pub fn new(target: u32, station: Option<StationId>, mode: DeliveryMode) -> Self {
        Self {
            target,
            station,
            mode,
            stand_off_ft: None,
        }
    }
    /// The stand-off distance while the strike holds off an unsafe target.
    pub fn stand_off_ft(&self) -> Option<f64> {
        self.stand_off_ft
    }
}

impl Strike {
    /// A strike that continues a human's attack: aimed at its target from
    /// the first step, choosing at once when close enough.
    pub fn taken_over(seed: u64, takeover: Takeover) -> Self {
        Self {
            target: Some(takeover.target),
            takeover: Some(takeover),
            ..Self::new(seed)
        }
    }

    /// The human's attack this strike continues, if it is a takeover.
    pub fn takeover(&self) -> Option<&Takeover> {
        self.takeover.as_ref()
    }

    /// The kept station, while it has rounds and is usable (a laser store
    /// only with the aircraft's own designator, decision 9).
    pub(super) fn kept_store(&self, inputs: &Inputs) -> Option<Store> {
        let station = self.takeover?.station?;
        inputs
            .stores
            .iter()
            .find(|s| s.station == station && s.rounds > 0)
            .filter(|s| s.kind != StoreKind::Command)
            .filter(|s| s.kind != StoreKind::Laser || inputs.designator)
            .copied()
    }

    /// The pass the kept bomb sight mode asks for: LVL flies a level run
    /// with a bomb or a guided bomb; CCIP leaves it to the table.
    pub(super) fn kept_pass(&self, kind: StoreKind) -> Option<Pass> {
        let takeover = self.takeover?;
        (takeover.mode == DeliveryMode::Level
            && matches!(
                kind,
                StoreKind::Bomb | StoreKind::Laser | StoreKind::Coordinate
            ))
        .then_some(Pass::Level)
    }

    /// A takeover attacks only when safe: when it is not, it turns away
    /// from the target and stands off (returns true, and the choice does
    /// not happen).
    pub(super) fn holds_off(&mut self, cx: &Context<'_>) -> bool {
        if self.takeover.is_none() || self.safe(cx) {
            return false;
        }
        let Some(point) = self.target_point(cx) else {
            return false;
        };
        self.withdrawn = true;
        self.holding = None;
        self.exit_heading_deg = bearing_deg(point, cx.own.position);
        self.enter(Phase::Exit, cx.tick);
        self.begin_stand_off(cx, point);
        true
    }

    /// A takeover that learns its attack is not safe before anything has
    /// left (on the way in, choosing, or flying the pass) holds off at once.
    pub(super) fn recheck_safe(&mut self, cx: &Context<'_>) {
        let attacking = match self.phase {
            Phase::Ingress | Phase::Regain | Phase::TurnBack | Phase::Wait => true,
            Phase::Attack => self.released == 0,
            Phase::Exit | Phase::Defend | Phase::Home => false,
        };
        if attacking {
            self.holds_off(cx);
        }
    }

    /// Where the takeover's current target stands, as the strike knows it.
    fn target_point(&self, cx: &Context<'_>) -> Option<Vector> {
        let id = self.target?;
        known_position(cx.picture, id).or_else(|| {
            cx.inputs
                .solution
                .filter(|s| s.target == id)
                .map(|s| s.target_position)
        })
    }

    /// Starts (or keeps) standing off `point`: outside every known missile
    /// site covering it, plus [`STAND_OFF_MARGIN_FT`], and never inside the
    /// exit distance. Returns false for a strike that is not a takeover.
    pub(super) fn begin_stand_off(&mut self, cx: &Context<'_>, point: Vector) -> bool {
        let exit = self.exit_distance(cx);
        let Some(takeover) = self.takeover.as_mut() else {
            return false;
        };
        let covered = cx
            .picture
            .threats()
            .hostile_defenses(cx.side)
            .filter(|d| d.covers_with_missiles(point))
            .map(|d| horizontal(d.position, point) + d.facts.missile_reach_ft)
            .fold(0., f64::max);
        takeover.stand_off_ft = Some(exit.max(covered + STAND_OFF_MARGIN_FT));
        true
    }

    /// The stand-off ends: the strike turns back at its target.
    pub(super) fn end_stand_off(&mut self) {
        if let Some(takeover) = self.takeover.as_mut() {
            takeover.stand_off_ft = None;
        }
    }

    /// While standing off: away from the target until the circle, then a
    /// left circle about it, the heading blending from the tangent toward
    /// the centre by how far out it is (retail's circle rule, 45 to 135
    /// degrees off the bearing). Corner speed, holding the height it had.
    pub(super) fn stand_off_steer(
        &self,
        cx: &Context<'_>,
        target_at: Option<Vector>,
    ) -> Option<Steer> {
        let radius = self.takeover?.stand_off_ft?;
        let point = target_at?;
        let own = cx.own;
        let distance = horizontal(own.position, point);
        let inward = bearing_deg(own.position, point);
        let heading = if distance < radius {
            // Out first: straight away from the target.
            wrap_deg(inward + 180.)
        } else {
            let off = (90. - 45. * (distance - radius) / radius).clamp(45., 135.);
            wrap_deg(inward + off)
        };
        let altitude = self
            .cruise_ft
            .unwrap_or(own.position[1])
            .max(own.position[1]);
        Some(Steer {
            heading_deg: heading,
            pitch_deg: hold_pitch_deg(own.position[1], altitude),
            speed: SpeedRequest::Corner,
            afterburner: distance < radius,
        })
    }

    /// While one of its laser weapons guides: the exit holds a level line on
    /// its heading (see the module notes). `None` when nothing guides or the
    /// strike is not a takeover.
    pub(super) fn guide_steer(&self, cx: &Context<'_>) -> Option<Steer> {
        self.takeover?;
        let guiding = self.kind == Some(StoreKind::Laser)
            && cx
                .inputs
                .solution
                .is_some_and(|s| Some(s.target) == self.target && s.guided_in_flight);
        guiding.then(|| Steer {
            heading_deg: self.exit_heading_deg,
            pitch_deg: hold_pitch_deg(
                cx.own.position[1],
                self.hold_ft.unwrap_or(cx.own.position[1]),
            ),
            speed: SpeedRequest::Corner,
            afterburner: false,
        })
    }

    /// The takeover is over: no target or store is left and nothing else
    /// sent it home.
    pub fn takeover_done(&self) -> bool {
        self.takeover.is_some() && self.phase == Phase::Home
    }
}

crate::checkpoint_struct!(Takeover {
    target,
    station,
    mode,
    stand_off_ft,
});

#[cfg(test)]
#[path = "strike_takeover_tests.rs"]
mod tests;
