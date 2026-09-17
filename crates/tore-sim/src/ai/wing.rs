//! Wing commands: the sender side and formation variation (B43) and the
//! wing-command receiver contract (B46), from
//! [`docs/spec/ai.md`](../../../../docs/spec/ai.md).
//!
//! The sender never moves anybody: it produces typed [`WingRequest`]s that a
//! host delivers to recipients. The receiver applies settings and reports one
//! of four distinct outcomes (applied, rejected, applied without motion, or
//! motion installed); the original handler's Boolean is not reproduced. The
//! formation table geometry, formation speed regulation and the meaning of the
//! opaque maneuver states are unresolved and stay explicit inputs or
//! [`AiError::UnspecifiedRule`].
//!
//! Fitted choices (agent decisions, 2026-09-17), each where the spec is silent:
//!
//! - Slot scaling maps lateral and forward to horizontal spacing and vertical
//!   to vertical spacing; variation components are added in that slot order.
//! - The human-control check precedes the maneuver state gate, so a human in
//!   an ineligible state gets `AppliedNoMotion` rather than `Rejected`.
//! - Approach heading is treated as absolute and wrapped to 0..=359 with
//!   pitch bounded per B13; its motion duration is reported as unspecified.

use super::{AiError, DecisionRandom, QUARTER_SECOND_TICKS, Result, ScalarSpeed, SpeedLimits};

/// Opaque target identity supplied by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TargetId(pub u32);
/// Opaque formation identity; the formation table is unresolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FormationId(pub u8);
/// Opaque wing-control setting; its values are not named by the spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WingControl(pub u8);
/// Opaque class/policy request payload (B46 "class/policy request").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClassPolicy(pub u8);

/// B43: spacing requests are clamped to this inclusive range in feet.
pub const MIN_SPACING_FT: u32 = 512;
pub const MAX_SPACING_FT: u32 = 20000;
/// B46: nominal duration of a break motion.
pub const BREAK_SECONDS: u32 = 5;
/// B43: nominal duration of the formation request.
pub const FORMATION_REQUEST_SECONDS: u32 = 3;
/// B46: nominal target-related deadline set by a concrete target assignment.
pub const CONCRETE_TARGET_SECONDS: u32 = 20;
/// B43: the next variation deadline advances by 1..=10 simulation seconds.
pub const VARIATION_MIN_SECONDS: i32 = 1;
pub const VARIATION_MAX_SECONDS: i32 = 10;

fn quarter_clock(tick: u64) -> u64 {
    tick / QUARTER_SECOND_TICKS
}

/// B43: clamp a spacing request to 512 through 20000 feet.
pub fn request_spacing(feet: i64) -> u32 {
    feet.clamp(i64::from(MIN_SPACING_FT), i64::from(MAX_SPACING_FT)) as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpacingAxis {
    Horizontal,
    Vertical,
}

/// Target/order assignment kinds distinguished by B46.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetOrder {
    HoldFire,
    FreeSelection,
    ClassPolicy(ClassPolicy),
    ConcreteTarget(TargetId),
}

/// A wing request delivered through wing events (B43, B46).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WingRequest {
    /// Heading offset relative to the recipient's own body heading, and pitch.
    Break {
        heading_offset_deg: i32,
        pitch_deg: i32,
    },
    /// Heading, pitch and a speed in the recovered domain; zero selects corner.
    Approach {
        heading_deg: i32,
        pitch_deg: i32,
        speed: ScalarSpeed,
    },
    Spacing {
        axis: SpacingAxis,
        feet: u32,
    },
    FormationSelection(FormationId),
    WingControl(WingControl),
    TargetAssignment(TargetOrder),
}

// ---------------------------------------------------------------------------
// B43: sender side
// ---------------------------------------------------------------------------

/// What the sending script knows about itself and its wing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SenderState {
    pub is_leader: bool,
    pub target: Option<TargetId>,
    /// Wingmen in wing order; the approach command reads the first one.
    pub wingman_targets: Vec<Option<TargetId>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendRejection {
    NotWingLeader,
    NoWingman,
}

/// Result of a sender-side command.
#[derive(Clone, Debug, PartialEq)]
pub enum SendOutcome {
    /// Requests to deliver, in order.
    Sent(Vec<WingRequest>),
    Rejected(SendRejection),
}

/// B43: the spacing setters act only for a wing leader.
pub fn set_spacing(sender: &SenderState, axis: SpacingAxis, feet: i64) -> SendOutcome {
    if !sender.is_leader {
        return SendOutcome::Rejected(SendRejection::NotWingLeader);
    }
    SendOutcome::Sent(vec![WingRequest::Spacing {
        axis,
        feet: request_spacing(feet),
    }])
}

/// Approach parameters as the script supplies them. Their bounds are
/// recorded as "bounded" without numbers, so none are applied here; the
/// receiver bounds pitch and wraps heading when it constructs motion (B13).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ApproachParameters {
    pub heading_deg: i32,
    pub pitch_deg: i32,
    pub speed: ScalarSpeed,
}

/// B43 approach command: leader with at least one wingman; a target
/// assignment precedes the approach when the first wingman's target differs
/// from the sender's.
pub fn approach_command(sender: &SenderState, params: ApproachParameters) -> Result<SendOutcome> {
    if !sender.is_leader {
        return Ok(SendOutcome::Rejected(SendRejection::NotWingLeader));
    }
    let Some(&first_wingman_target) = sender.wingman_targets.first() else {
        return Ok(SendOutcome::Rejected(SendRejection::NoWingman));
    };
    let mut requests = Vec::with_capacity(2);
    if first_wingman_target != sender.target {
        let Some(target) = sender.target else {
            return Err(AiError::UnspecifiedRule(
                "B43 approach target assignment when the sender has no target",
            ));
        };
        requests.push(WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(
            target,
        )));
    }
    requests.push(WingRequest::Approach {
        heading_deg: params.heading_deg,
        pitch_deg: params.pitch_deg,
        speed: params.speed,
    });
    Ok(SendOutcome::Sent(requests))
}

// ---------------------------------------------------------------------------
// B43: formation variation and formation point
// ---------------------------------------------------------------------------

/// Persistent formation offset state. The offset changes only when the
/// deadline is reached; the next deadline advances from the previous deadline,
/// so a long gap in service is caught up one draw per call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormationVariation {
    offset_ft: [i32; 3],
    next_deadline: u64,
}
impl FormationVariation {
    /// Start with a zero offset and the first draw due at `first_due_tick`.
    pub fn new(first_due_tick: u64) -> Self {
        Self {
            offset_ft: [0; 3],
            next_deadline: quarter_clock(first_due_tick),
        }
    }
    /// Current offset in feet, component order as recovered (first component
    /// -15..=14, the other two -50..=49).
    pub fn offset_ft(&self) -> [i32; 3] {
        self.offset_ft
    }
    /// Next deadline in quarter-second counts.
    pub fn next_deadline(&self) -> u64 {
        self.next_deadline
    }
    /// Draw a new offset when the deadline has been reached; returns whether
    /// a draw happened.
    pub fn advance(&mut self, tick: u64, random: &mut DecisionRandom) -> bool {
        if quarter_clock(tick) < self.next_deadline {
            return false;
        }
        self.offset_ft = [
            random.range(-15, 14),
            random.range(-50, 49),
            random.range(-50, 49),
        ];
        let seconds = random.range(VARIATION_MIN_SECONDS, VARIATION_MAX_SECONDS);
        self.next_deadline += 4 * u64::from(seconds.unsigned_abs());
        true
    }
}

/// One formation-table slot as unitless multipliers of the spacings. The
/// table itself is unresolved, so the host supplies the slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormationSlot {
    /// Positive to the reference aircraft's right; scaled by horizontal spacing.
    pub lateral: f64,
    /// Positive ahead of the reference aircraft; scaled by horizontal spacing.
    pub forward: f64,
    /// Positive above; scaled by vertical spacing.
    pub vertical: f64,
}

/// Formation point relative to the reference aircraft, in feet: east, north,
/// up. The slot is scaled by the spacings, rotated horizontally with the
/// reference heading (degrees clockwise from north) and offset by the
/// variation components in slot order (lateral, forward, vertical).
pub fn formation_point(
    slot: FormationSlot,
    horizontal_spacing_ft: u32,
    vertical_spacing_ft: u32,
    reference_heading_deg: f64,
    variation: &FormationVariation,
) -> [f64; 3] {
    let offset = variation.offset_ft();
    let lateral = slot.lateral * f64::from(horizontal_spacing_ft) + f64::from(offset[0]);
    let forward = slot.forward * f64::from(horizontal_spacing_ft) + f64::from(offset[1]);
    let vertical = slot.vertical * f64::from(vertical_spacing_ft) + f64::from(offset[2]);
    let (sin, cos) = reference_heading_deg.to_radians().sin_cos();
    [
        forward * sin + lateral * cos,
        forward * cos - lateral * sin,
        vertical,
    ]
}

/// B43: the formation request's own position-regulating speed mode is
/// unresolved; there is no speed to return.
pub fn formation_request_speed() -> Result<ScalarSpeed> {
    Err(AiError::UnspecifiedRule(
        "B43 formation position-regulating speed mode",
    ))
}

// ---------------------------------------------------------------------------
// B46: receiver side
// ---------------------------------------------------------------------------

/// The recipient as the event receiver sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecipientState {
    pub human_controlled: bool,
    /// Opaque original maneuver state number; do not name these.
    pub maneuver_state: u32,
    pub target: Option<TargetId>,
    pub body_heading_deg: i32,
    pub speed_limits: SpeedLimits,
    /// Whether an ordinary-state command is active (its producer is the host).
    pub active_command: bool,
    pub formation: Option<FormationId>,
    pub wing_control: Option<WingControl>,
    pub horizontal_spacing_ft: Option<u32>,
    pub vertical_spacing_ft: Option<u32>,
    pub target_order: Option<TargetOrder>,
    /// Nominal target-related deadline in quarter counts (B46). Expiry
    /// meaning is unresolved; the target is not forgotten here.
    pub target_deadline: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankSelection {
    Automatic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionDuration {
    NominalSeconds(u32),
    /// The spec does not give the approach motion a duration.
    Unspecified,
}

/// The motion request installed on an AI recipient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionSummary {
    /// Absolute heading wrapped into 0..=359 (B13).
    pub heading_deg: i32,
    /// Pitch bounded to -90..=90 (B13).
    pub pitch_deg: i32,
    pub speed: ScalarSpeed,
    pub bank: BankSelection,
    pub duration: MotionDuration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectReason {
    /// The maneuver eligibility gate rejected the recipient's state.
    IneligibleState(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppliedSetting {
    Spacing { axis: SpacingAxis, feet: u32 },
    FormationSelection { cleared_active_command: bool },
    WingControl,
    TargetOrder { deadline: Option<u64> },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReceiverOutcome {
    Applied(AppliedSetting),
    Rejected(RejectReason),
    /// Accepted without installing motion (human recipient, targetless approach).
    AppliedNoMotion,
    MotionInstalled(MotionSummary),
}

fn wrap_heading(deg: i32) -> i32 {
    deg.rem_euclid(360)
}

/// B46 maneuver eligibility gate over the opaque state numbers: 1..=18 and
/// 21..=30 are rejected; 19 and 20 may transition. Other numbers are outside
/// the reviewed range.
fn maneuver_eligible(state: u32) -> Result<bool> {
    match state {
        1..=18 | 21..=30 => Ok(false),
        19 | 20 => Ok(true),
        _ => Err(AiError::UnspecifiedRule(
            "B46 maneuver gate for states outside 1..=30",
        )),
    }
}

/// B46 receiver. Settings are applied to `recipient`; motion is reported,
/// not executed. `tick` samples the clock for the concrete-target deadline.
pub fn receive(
    request: WingRequest,
    recipient: &mut RecipientState,
    tick: u64,
) -> Result<ReceiverOutcome> {
    Ok(match request {
        WingRequest::Break {
            heading_offset_deg,
            pitch_deg,
        } => {
            if recipient.human_controlled {
                return Ok(ReceiverOutcome::AppliedNoMotion);
            }
            if !maneuver_eligible(recipient.maneuver_state)? {
                return Ok(ReceiverOutcome::Rejected(RejectReason::IneligibleState(
                    recipient.maneuver_state,
                )));
            }
            ReceiverOutcome::MotionInstalled(MotionSummary {
                heading_deg: wrap_heading(recipient.body_heading_deg + heading_offset_deg),
                pitch_deg: pitch_deg.clamp(-90, 90),
                speed: recipient.speed_limits.corner,
                bank: BankSelection::Automatic,
                duration: MotionDuration::NominalSeconds(BREAK_SECONDS),
            })
        }
        WingRequest::Approach {
            heading_deg,
            pitch_deg,
            speed,
        } => {
            if recipient.human_controlled {
                return Ok(ReceiverOutcome::AppliedNoMotion);
            }
            if !maneuver_eligible(recipient.maneuver_state)? {
                return Ok(ReceiverOutcome::Rejected(RejectReason::IneligibleState(
                    recipient.maneuver_state,
                )));
            }
            if recipient.target.is_none() {
                return Ok(ReceiverOutcome::AppliedNoMotion);
            }
            if speed.0 < 0.0 {
                return Err(AiError::InvalidInput("approach speed must not be negative"));
            }
            let limits = recipient.speed_limits;
            let speed = if speed.0 == 0.0 {
                limits.corner
            } else {
                speed.max(limits.minimum).min(limits.maximum)
            };
            ReceiverOutcome::MotionInstalled(MotionSummary {
                heading_deg: wrap_heading(heading_deg),
                pitch_deg: pitch_deg.clamp(-90, 90),
                speed,
                bank: BankSelection::Automatic,
                duration: MotionDuration::Unspecified,
            })
        }
        WingRequest::Spacing { axis, feet } => {
            match axis {
                SpacingAxis::Horizontal => recipient.horizontal_spacing_ft = Some(feet),
                SpacingAxis::Vertical => recipient.vertical_spacing_ft = Some(feet),
            }
            ReceiverOutcome::Applied(AppliedSetting::Spacing { axis, feet })
        }
        WingRequest::FormationSelection(formation) => {
            recipient.formation = Some(formation);
            let cleared_active_command = recipient.active_command;
            recipient.active_command = false;
            ReceiverOutcome::Applied(AppliedSetting::FormationSelection {
                cleared_active_command,
            })
        }
        WingRequest::WingControl(control) => {
            recipient.wing_control = Some(control);
            recipient.active_command = false;
            ReceiverOutcome::Applied(AppliedSetting::WingControl)
        }
        WingRequest::TargetAssignment(order) => {
            recipient.target_order = Some(order);
            let deadline = match order {
                TargetOrder::ConcreteTarget(target) => {
                    recipient.target = Some(target);
                    let deadline = quarter_clock(tick) + 4 * u64::from(CONCRETE_TARGET_SECONDS);
                    recipient.target_deadline = Some(deadline);
                    Some(deadline)
                }
                TargetOrder::HoldFire
                | TargetOrder::FreeSelection
                | TargetOrder::ClassPolicy(_) => None,
            };
            ReceiverOutcome::Applied(AppliedSetting::TargetOrder { deadline })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const Q: u64 = QUARTER_SECOND_TICKS;

    fn leader() -> SenderState {
        SenderState {
            is_leader: true,
            target: Some(TargetId(3)),
            wingman_targets: vec![Some(TargetId(3))],
        }
    }

    #[test]
    fn spacing_clamps_at_the_recovered_bounds() {
        assert_eq!(request_spacing(511), 512);
        assert_eq!(request_spacing(512), 512);
        assert_eq!(request_spacing(20000), 20000);
        assert_eq!(request_spacing(20001), 20000);
        assert_eq!(request_spacing(-5), 512);
    }

    #[test]
    fn spacing_setters_act_only_for_the_leader() {
        assert_eq!(
            set_spacing(&leader(), SpacingAxis::Horizontal, 100),
            SendOutcome::Sent(vec![WingRequest::Spacing {
                axis: SpacingAxis::Horizontal,
                feet: 512
            }])
        );
        let wingman = SenderState {
            is_leader: false,
            ..leader()
        };
        assert_eq!(
            set_spacing(&wingman, SpacingAxis::Vertical, 1000),
            SendOutcome::Rejected(SendRejection::NotWingLeader)
        );
    }

    #[test]
    fn approach_command_compares_first_wingman_target() {
        let params = ApproachParameters {
            heading_deg: 30,
            pitch_deg: -5,
            speed: ScalarSpeed(0.0),
        };
        let approach = WingRequest::Approach {
            heading_deg: 30,
            pitch_deg: -5,
            speed: ScalarSpeed(0.0),
        };
        assert_eq!(
            approach_command(&leader(), params).unwrap(),
            SendOutcome::Sent(vec![approach])
        );
        let differing = SenderState {
            wingman_targets: vec![Some(TargetId(9)), Some(TargetId(3))],
            ..leader()
        };
        assert_eq!(
            approach_command(&differing, params).unwrap(),
            SendOutcome::Sent(vec![
                WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(3))),
                approach,
            ])
        );
        let untargeted_wingman = SenderState {
            wingman_targets: vec![None],
            ..leader()
        };
        assert!(matches!(
            approach_command(&untargeted_wingman, params).unwrap(),
            SendOutcome::Sent(ref r) if r.len() == 2
        ));
        let alone = SenderState {
            wingman_targets: vec![],
            ..leader()
        };
        assert_eq!(
            approach_command(&alone, params).unwrap(),
            SendOutcome::Rejected(SendRejection::NoWingman)
        );
        let not_leader = SenderState {
            is_leader: false,
            ..leader()
        };
        assert_eq!(
            approach_command(&not_leader, params).unwrap(),
            SendOutcome::Rejected(SendRejection::NotWingLeader)
        );
        let no_target = SenderState {
            target: None,
            ..leader()
        };
        assert!(approach_command(&no_target, params).is_err());
    }

    #[test]
    fn variation_deadline_advances_from_previous_deadline() {
        let mut random = DecisionRandom::seeded(11);
        let mut previous = 0;
        let mut variation = FormationVariation::new(0);
        for _ in 0..200 {
            let deadline = variation.next_deadline();
            assert!(deadline >= previous);
            // Serve exactly on the deadline: the next one is 1..=10 s later.
            assert!(variation.advance(deadline * Q, &mut random));
            let step = variation.next_deadline() - deadline;
            assert!((4..=40).contains(&step) && step.is_multiple_of(4), "{step}");
            let [a, b, c] = variation.offset_ft();
            assert!((-15..=14).contains(&a));
            assert!((-50..=49).contains(&b));
            assert!((-50..=49).contains(&c));
            previous = deadline;
        }
    }

    #[test]
    fn variation_does_not_redraw_before_deadline_and_catches_up_after_a_gap() {
        let mut random = DecisionRandom::seeded(5);
        let mut variation = FormationVariation::new(8 * Q);
        assert!(!variation.advance(8 * Q - 1, &mut random));
        assert_eq!(variation.offset_ft(), [0; 3]);
        assert!(variation.advance(8 * Q, &mut random));
        let first = variation.offset_ft();
        let deadline = variation.next_deadline();
        assert!(!variation.advance(deadline * Q - 1, &mut random));
        assert_eq!(variation.offset_ft(), first);
        // A long gap: the deadline keeps stepping from its old value, so it
        // can stay behind the clock and be served again immediately.
        let far = deadline + 400;
        assert!(variation.advance(far * Q, &mut random));
        assert!(variation.next_deadline() <= deadline + 40);
        assert!(variation.next_deadline() < far);
        assert!(variation.advance(far * Q, &mut random));
    }

    #[test]
    fn formation_point_scales_rotates_and_offsets() {
        let variation = FormationVariation::new(0);
        let slot = FormationSlot {
            lateral: 1.0,
            forward: -0.5,
            vertical: 0.25,
        };
        let point = formation_point(slot, 1000, 400, 0.0, &variation);
        assert_eq!(point, [1000.0, -500.0, 100.0]);
        let rotated = formation_point(slot, 1000, 400, 90.0, &variation);
        assert!((rotated[0] - -500.0).abs() < 1e-9);
        assert!((rotated[1] - -1000.0).abs() < 1e-9);
        assert_eq!(rotated[2], 100.0);
        assert!(formation_request_speed().is_err());
    }

    fn recipient() -> RecipientState {
        RecipientState {
            human_controlled: false,
            maneuver_state: 19,
            target: Some(TargetId(3)),
            body_heading_deg: 350,
            speed_limits: SpeedLimits {
                minimum: ScalarSpeed(100.0),
                maximum: ScalarSpeed(600.0),
                corner: ScalarSpeed(350.0),
            },
            active_command: true,
            formation: None,
            wing_control: None,
            horizontal_spacing_ft: None,
            vertical_spacing_ft: None,
            target_order: None,
            target_deadline: None,
        }
    }
    const BREAK: WingRequest = WingRequest::Break {
        heading_offset_deg: 170,
        pitch_deg: 100,
    };
    const APPROACH: WingRequest = WingRequest::Approach {
        heading_deg: 400,
        pitch_deg: 10,
        speed: ScalarSpeed(0.0),
    };

    #[test]
    fn human_recipients_never_get_motion() {
        let mut human = RecipientState {
            human_controlled: true,
            ..recipient()
        };
        assert_eq!(
            receive(BREAK, &mut human, 0).unwrap(),
            ReceiverOutcome::AppliedNoMotion
        );
        assert_eq!(
            receive(APPROACH, &mut human, 0).unwrap(),
            ReceiverOutcome::AppliedNoMotion
        );
    }

    #[test]
    fn break_installs_five_second_corner_speed_motion() {
        let mut ai = recipient();
        assert_eq!(
            receive(BREAK, &mut ai, 0).unwrap(),
            ReceiverOutcome::MotionInstalled(MotionSummary {
                heading_deg: 160,
                pitch_deg: 90,
                speed: ScalarSpeed(350.0),
                bank: BankSelection::Automatic,
                duration: MotionDuration::NominalSeconds(5),
            })
        );
        let mut untargeted = RecipientState {
            target: None,
            ..recipient()
        };
        assert!(matches!(
            receive(BREAK, &mut untargeted, 0).unwrap(),
            ReceiverOutcome::MotionInstalled(_)
        ));
    }

    #[test]
    fn maneuver_gate_boundaries() {
        for (state, eligible) in [
            (18, false),
            (19, true),
            (20, true),
            (21, false),
            (1, false),
            (30, false),
        ] {
            let mut ai = RecipientState {
                maneuver_state: state,
                ..recipient()
            };
            let outcome = receive(BREAK, &mut ai, 0).unwrap();
            if eligible {
                assert!(
                    matches!(outcome, ReceiverOutcome::MotionInstalled(_)),
                    "{state}"
                );
            } else {
                assert_eq!(
                    outcome,
                    ReceiverOutcome::Rejected(RejectReason::IneligibleState(state))
                );
            }
        }
        let mut outside = RecipientState {
            maneuver_state: 0,
            ..recipient()
        };
        assert!(receive(BREAK, &mut outside, 0).is_err());
    }

    #[test]
    fn approach_needs_a_target_and_bounds_speed() {
        let mut untargeted = RecipientState {
            target: None,
            ..recipient()
        };
        assert_eq!(
            receive(APPROACH, &mut untargeted, 0).unwrap(),
            ReceiverOutcome::AppliedNoMotion
        );
        let mut ai = recipient();
        assert_eq!(
            receive(APPROACH, &mut ai, 0).unwrap(),
            ReceiverOutcome::MotionInstalled(MotionSummary {
                heading_deg: 40,
                pitch_deg: 10,
                speed: ScalarSpeed(350.0),
                bank: BankSelection::Automatic,
                duration: MotionDuration::Unspecified,
            })
        );
        let speed_of = |speed: f64| {
            let request = WingRequest::Approach {
                heading_deg: 0,
                pitch_deg: 0,
                speed: ScalarSpeed(speed),
            };
            match receive(request, &mut recipient(), 0).unwrap() {
                ReceiverOutcome::MotionInstalled(motion) => motion.speed,
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(speed_of(50.0), ScalarSpeed(100.0));
        assert_eq!(speed_of(100.0), ScalarSpeed(100.0));
        assert_eq!(speed_of(250.0), ScalarSpeed(250.0));
        assert_eq!(speed_of(600.0), ScalarSpeed(600.0));
        assert_eq!(speed_of(900.0), ScalarSpeed(600.0));
    }

    #[test]
    fn settings_apply_regardless_of_motion_eligibility() {
        let mut ai = RecipientState {
            maneuver_state: 5,
            human_controlled: true,
            ..recipient()
        };
        let spacing = WingRequest::Spacing {
            axis: SpacingAxis::Vertical,
            feet: 700,
        };
        assert_eq!(
            receive(spacing, &mut ai, 0).unwrap(),
            ReceiverOutcome::Applied(AppliedSetting::Spacing {
                axis: SpacingAxis::Vertical,
                feet: 700
            })
        );
        assert_eq!(ai.vertical_spacing_ft, Some(700));
        assert_eq!(
            receive(WingRequest::FormationSelection(FormationId(2)), &mut ai, 0).unwrap(),
            ReceiverOutcome::Applied(AppliedSetting::FormationSelection {
                cleared_active_command: true
            })
        );
        assert_eq!(ai.formation, Some(FormationId(2)));
        assert!(!ai.active_command);
        ai.active_command = true;
        assert_eq!(
            receive(WingRequest::WingControl(WingControl(1)), &mut ai, 0).unwrap(),
            ReceiverOutcome::Applied(AppliedSetting::WingControl)
        );
        assert_eq!(ai.wing_control, Some(WingControl(1)));
        assert!(!ai.active_command);
    }

    #[test]
    fn concrete_target_sets_a_twenty_second_deadline() {
        let mut ai = recipient();
        let tick = 6 * Q;
        assert_eq!(
            receive(
                WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(8))),
                &mut ai,
                tick
            )
            .unwrap(),
            ReceiverOutcome::Applied(AppliedSetting::TargetOrder { deadline: Some(86) })
        );
        assert_eq!(ai.target, Some(TargetId(8)));
        assert_eq!(ai.target_deadline, Some(86));
        for order in [
            TargetOrder::HoldFire,
            TargetOrder::FreeSelection,
            TargetOrder::ClassPolicy(ClassPolicy(4)),
        ] {
            assert_eq!(
                receive(WingRequest::TargetAssignment(order), &mut ai, tick).unwrap(),
                ReceiverOutcome::Applied(AppliedSetting::TargetOrder { deadline: None })
            );
            assert_eq!(ai.target_order, Some(order));
        }
        // Nothing here forgets the target at expiry.
        assert_eq!(ai.target, Some(TargetId(8)));
    }
}
