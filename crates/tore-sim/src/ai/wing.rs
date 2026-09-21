//! Wing commands: the sender side, formation geometry, formation speed and
//! wing control (B43) and the wing-command receiver contract with the player
//! order values, radio phrases and rejoin rules (B46), from
//! [`docs/spec/ai.md`](../../../../docs/spec/ai.md).
//!
//! The sender never moves anybody: it produces typed [`WingRequest`]s that a
//! host delivers to recipients. The receiver applies settings and reports one
//! of four distinct outcomes (applied, rejected, applied without motion, or
//! motion installed); the original handler's Boolean is not reproduced. The
//! meaning of the opaque maneuver states, the approach steering point and the
//! mode 9 lead-projection entry stay explicit inputs or
//! [`AiError::UnspecifiedRule`].
//!
//! Frames: a formation slot point is expressed in the leader's heading-only
//! body frame as `[lateral, vertical, longitudinal]` feet, lateral positive to
//! the leader's right, vertical positive above, longitudinal positive ahead
//! (so behind is negative). World positions follow the host basis used by
//! `pursuit`: `[x, y, z]` with `y` up, forward at heading 0 is `+z`, right is
//! `+x`, heading clockwise seen from above.
//!
//! Fitted choices (agent decisions, 2026-09-17), each where the spec is silent:
//!
//! - The three variation draw components are applied in the order lateral
//!   (-15..=14), longitudinal, vertical (both -50..=49). The spec gives the
//!   ranges by draw order only, not which axis each feeds.
//! - The human-control check precedes the maneuver state gate, so a human in
//!   an ineligible state gets `AppliedNoMotion` rather than `Rejected`.
//! - Approach heading is treated as absolute and wrapped to 0..=359 with
//!   pitch bounded per B13; its motion duration is reported as unspecified.
//! - A break order leaves the control level unchanged: B43 lists every order
//!   that moves the level and break is not among them.
//! - Target sharing: the leader counts as an attacker of its own target and
//!   engages only while the attacker count (existing plus newly assigned
//!   wingmen) is still below the cap. B46 records loose-versus-medium
//!   self-engagement as open.

use super::{AiError, DecisionRandom, QUARTER_SECOND_TICKS, Result, ScalarSpeed, SpeedLimits};

/// Opaque target identity supplied by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TargetId(pub u32);
/// Opaque class/policy request payload (B46 "class/policy request").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClassPolicy(pub u8);

/// B43: script spacing requests are clamped to this inclusive range in feet.
pub const MIN_SPACING_FT: i32 = 512;
pub const MAX_SPACING_FT: i32 = 20000;
/// B43: the player's horizontal spacing order toggles between these values.
pub const PLAYER_SPACING_CLOSE_FT: i32 = 512;
pub const PLAYER_SPACING_SPREAD_FT: i32 = 2048;
/// B43: the player's stacking order steps by this much above or below level.
pub const PLAYER_STACKING_FT: i32 = 512;
/// B43: an idle non-wingman forces at least this horizontal spacing.
pub const IDLE_MIN_SPACING_FT: i32 = 1024;
/// B43: wingman slots run 1 through 9; slot 0 is the leader.
pub const MAX_WINGMAN_SLOT: u8 = 9;
/// B46: nominal duration of a break motion.
pub const BREAK_SECONDS: u32 = 5;
/// B43: nominal duration of the formation request.
pub const FORMATION_REQUEST_SECONDS: u32 = 3;
/// B46: nominal target-related deadline set by a concrete target assignment.
pub const CONCRETE_TARGET_SECONDS: u32 = 20;
/// B43: the next variation deadline advances by 1..=10 simulation seconds.
pub const VARIATION_MIN_SECONDS: i32 = 1;
pub const VARIATION_MAX_SECONDS: i32 = 10;
/// B46: a player approach order completes within this distance of its point.
pub const APPROACH_COMPLETE_WITHIN_FT: f64 = 2000.0;
/// B46: the spacing call says "Tighten up" below this spacing.
pub const TIGHTEN_UP_BELOW_FT: i32 = 1000;

fn quarter_clock(tick: u64) -> u64 {
    tick / QUARTER_SECOND_TICKS
}

/// B43: clamp a script spacing request to 512 through 20000 feet.
pub fn request_spacing(feet: i64) -> i32 {
    feet.clamp(i64::from(MIN_SPACING_FT), i64::from(MAX_SPACING_FT)) as i32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpacingAxis {
    Horizontal,
    Vertical,
}

// ---------------------------------------------------------------------------
// B43: formations, control levels and player orders
// ---------------------------------------------------------------------------

/// B43: the three formations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Formation {
    Echelon,
    LineAbreast,
    LineAstern,
}
impl Formation {
    pub const ALL: [Self; 3] = [Self::Echelon, Self::LineAbreast, Self::LineAstern];
    /// The name spoken and printed by the formation call.
    pub fn name(self) -> &'static str {
        match self {
            Self::Echelon => "Echelon",
            Self::LineAbreast => "Line abreast",
            Self::LineAstern => "Line astern",
        }
    }
}

/// B43: wing control levels, loosest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WingControl {
    Loose = 1,
    Medium = 2,
    Tight = 3,
}
impl WingControl {
    pub const ALL: [Self; 3] = [Self::Loose, Self::Medium, Self::Tight];
    pub fn name(self) -> &'static str {
        match self {
            Self::Loose => "Loose",
            Self::Medium => "Medium",
            Self::Tight => "Tight",
        }
    }
    pub fn level(self) -> u8 {
        self as u8
    }
}

/// B46: player break orders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlayerBreak {
    Left,
    Right,
    Low,
    High,
    Straight,
}
impl PlayerBreak {
    pub const ALL: [Self; 5] = [
        Self::Left,
        Self::Right,
        Self::Low,
        Self::High,
        Self::Straight,
    ];
    /// Heading change relative to the wingman's own body heading, degrees.
    pub fn heading_offset_deg(self) -> i32 {
        match self {
            Self::Left => -175,
            Self::Right => 170,
            Self::Low | Self::High | Self::Straight => 0,
        }
    }
    /// Pitch of the break request, degrees.
    pub fn pitch_deg(self) -> i32 {
        match self {
            Self::Low => -70,
            Self::High => 70,
            Self::Left | Self::Right | Self::Straight => 0,
        }
    }
    /// The wing request this order sends; the receiver makes it a nominal
    /// five-second motion at corner speed.
    pub fn request(self) -> WingRequest {
        WingRequest::Break {
            heading_offset_deg: self.heading_offset_deg(),
            pitch_deg: self.pitch_deg(),
        }
    }
}

/// B46: player approach orders, steering relative to the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlayerApproach {
    Left,
    Right,
    Low,
    High,
}
impl PlayerApproach {
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Low, Self::High];
    /// Heading offset from the bearing to the target, degrees.
    pub fn heading_offset_deg(self) -> i32 {
        match self {
            Self::Left => -45,
            Self::Right => 45,
            Self::Low | Self::High => 0,
        }
    }
    /// Pitch offset from the elevation to the target, degrees.
    pub fn pitch_offset_deg(self) -> i32 {
        match self {
            Self::Low => -35,
            Self::High => 35,
            Self::Left | Self::Right => 0,
        }
    }
    /// The wing request this order sends, given the bearing and elevation to
    /// the target in degrees. The speed is the script's choice; zero selects
    /// corner speed at the receiver. Whether the completed approach point is
    /// the target itself or displaced is open (B46).
    pub fn request(
        self,
        target_bearing_deg: i32,
        target_elevation_deg: i32,
        speed: ScalarSpeed,
    ) -> WingRequest {
        WingRequest::Approach {
            heading_deg: target_bearing_deg + self.heading_offset_deg(),
            pitch_deg: target_elevation_deg + self.pitch_offset_deg(),
            speed,
        }
    }
}

/// B46: an approach order completes within 2000 ft of its approach point.
pub fn approach_complete(distance_to_point_ft: f64) -> bool {
    distance_to_point_ft <= APPROACH_COMPLETE_WITHIN_FT
}

/// What the wingman does once an approach completes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterApproach {
    ReturnToFormation,
    ContinueAttack,
}

/// B46: after completion the wingman returns to formation unless it has an
/// attack assignment.
pub fn after_approach(has_attack_assignment: bool) -> AfterApproach {
    if has_attack_assignment {
        AfterApproach::ContinueAttack
    } else {
        AfterApproach::ReturnToFormation
    }
}

/// The player's wing orders with a recovered effect (B43, B46). Bug out is
/// not listed: its return-to-base helpers are open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlayerOrder {
    EngageMyTarget,
    ProtectMe,
    AttackOnContact,
    EngageFromFormation,
    Disengage,
    Break(PlayerBreak),
    Approach(PlayerApproach),
    Formation(Formation),
    Spacing,
    Stacking,
    ControlToggle,
}

/// B43: the player's horizontal spacing order toggles 512 and 2048 ft. Any
/// spacing other than 512 (a waypoint or script value included) toggles to
/// 512.
pub fn toggle_horizontal_spacing(current_ft: i32) -> i32 {
    if current_ft == PLAYER_SPACING_CLOSE_FT {
        PLAYER_SPACING_SPREAD_FT
    } else {
        PLAYER_SPACING_CLOSE_FT
    }
}

/// B43: the player's stacking order cycles level, 512 ft high, 512 ft low.
/// The step from a stacking outside that cycle is not recorded.
pub fn cycle_vertical_stacking(current_ft: i32) -> Result<i32> {
    match current_ft {
        0 => Ok(PLAYER_STACKING_FT),
        PLAYER_STACKING_FT => Ok(-PLAYER_STACKING_FT),
        v if v == -PLAYER_STACKING_FT => Ok(0),
        _ => Err(AiError::InvalidInput(
            "B43 stacking cycle from a value outside 0, 512 and -512 feet",
        )),
    }
}

/// B43: the settings an idle AI aircraft that is not a wingman and has no
/// command forces before holding heading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdleDefaults {
    pub control: WingControl,
    pub formation: Formation,
    pub horizontal_spacing_ft: i32,
}

/// B43: loose control, line astern and at least 1024 ft spacing.
pub fn idle_non_wingman_defaults(current_horizontal_spacing_ft: i32) -> IdleDefaults {
    IdleDefaults {
        control: WingControl::Loose,
        formation: Formation::LineAstern,
        horizontal_spacing_ft: current_horizontal_spacing_ft.max(IDLE_MIN_SPACING_FT),
    }
}

/// B43: the silent control side effect of a player order. Engage my target,
/// protect me, attack on contact and the approach orders lower control to
/// loose (only if higher); engage from formation, disengage, formation,
/// spacing and stacking raise it to medium (only if lower); the control key
/// toggles loose and medium. The toggle from tight is not recorded.
pub fn apply_control_side_effect(order: PlayerOrder, control: WingControl) -> Result<WingControl> {
    Ok(match order {
        PlayerOrder::EngageMyTarget
        | PlayerOrder::ProtectMe
        | PlayerOrder::AttackOnContact
        | PlayerOrder::Approach(_) => control.min(WingControl::Loose),
        PlayerOrder::EngageFromFormation
        | PlayerOrder::Disengage
        | PlayerOrder::Formation(_)
        | PlayerOrder::Spacing
        | PlayerOrder::Stacking => control.max(WingControl::Medium),
        PlayerOrder::ControlToggle => match control {
            WingControl::Loose => WingControl::Medium,
            WingControl::Medium => WingControl::Loose,
            WingControl::Tight => {
                return Err(AiError::UnspecifiedRule(
                    "B43 control toggle from tight control",
                ));
            }
        },
        PlayerOrder::Break(_) => control,
    })
}

/// B43: only loose control shares the leader's target with wingmen in formation.
pub fn leader_shares_target(control: WingControl) -> bool {
    control == WingControl::Loose
}

/// B41/B43: the waypoint's attacker allowance for one target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttackerCap {
    One,
    Two,
    Unlimited,
}
impl AttackerCap {
    /// The allowance helper's value: 1, 2 or 100.
    pub fn value(self) -> u32 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Unlimited => 100,
        }
    }
}

/// Result of the leader sharing its target under loose control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShareOutcome {
    /// Wingmen in formation newly assigned the leader's target.
    pub assigned: u32,
    /// Whether the leader engages the target itself (fitted, see module doc).
    pub leader_engages: bool,
}

/// B43: assign the leader's target to wingmen in formation until the cap is
/// filled, counting attackers already on it.
pub fn share_targets(
    cap: AttackerCap,
    already_attacking: u32,
    wingmen_in_formation: u32,
) -> ShareOutcome {
    let free = cap.value().saturating_sub(already_attacking);
    let assigned = wingmen_in_formation.min(free);
    ShareOutcome {
        assigned,
        leader_engages: already_attacking + assigned < cap.value(),
    }
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
    /// Horizontal spacing, or vertical stacking (negative is below the leader).
    Spacing {
        axis: SpacingAxis,
        feet: i32,
    },
    FormationSelection(Formation),
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

/// B43: the script spacing setters act only for a wing leader.
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
// B46: radio
// ---------------------------------------------------------------------------

/// B46: the player's call, printed and voiced when the order is sent whether
/// or not the wingman can comply. `horizontal_spacing_ft` is the spacing the
/// order sets; `control` is the level the control key sets. Only the spacing,
/// formation and control phrases are recorded.
pub fn sender_phrase(
    order: PlayerOrder,
    horizontal_spacing_ft: i32,
    control: WingControl,
) -> Result<&'static str> {
    match order {
        PlayerOrder::Spacing if horizontal_spacing_ft < TIGHTEN_UP_BELOW_FT => Ok("Tighten up"),
        PlayerOrder::Spacing => Ok("Combat spread"),
        PlayerOrder::Formation(formation) => Ok(formation.name()),
        PlayerOrder::ControlToggle => Ok(control.name()),
        PlayerOrder::EngageMyTarget
        | PlayerOrder::ProtectMe
        | PlayerOrder::AttackOnContact
        | PlayerOrder::EngageFromFormation
        | PlayerOrder::Disengage
        | PlayerOrder::Break(_)
        | PlayerOrder::Approach(_)
        | PlayerOrder::Stacking => Err(AiError::UnspecifiedRule(
            "B46 sender phrase text for this order is not recorded",
        )),
    }
}

/// B46: only the first wingman replies, and only to target assignments:
/// "Engaging" for a concrete target (heard only with radio traffic enabled)
/// and "Showtime!" for protect me. Every other order gets no spoken reply.
pub fn wingman_reply(
    order: PlayerOrder,
    radio_traffic_enabled: bool,
    is_first_wingman: bool,
) -> Option<&'static str> {
    if !is_first_wingman {
        return None;
    }
    match order {
        PlayerOrder::EngageMyTarget if radio_traffic_enabled => Some("Engaging"),
        PlayerOrder::ProtectMe => Some("Showtime!"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// B43: formation geometry, variation and formation speed
// ---------------------------------------------------------------------------

/// Host formation table, B43 except balanced line abreast (see spec).
/// Unitless multipliers `[lateral, vertical,
/// longitudinal]` of horizontal spacing H, vertical stacking V and H. Slot 0
/// is the leader; wingman slots run 1 through 9.
pub fn slot_multipliers(formation: Formation, slot: u8) -> Result<[i32; 3]> {
    if slot == 0 {
        return Ok([0; 3]);
    }
    if slot > MAX_WINGMAN_SLOT {
        return Err(AiError::InvalidInput("B43 formation slot above 9"));
    }
    let n = i32::from(slot);
    Ok(match formation {
        Formation::Echelon => {
            const VERTICAL: [i32; 9] = [1, -1, 2, -2, 3, 3, 4, 4, 5];
            let pair = (n + 1) / 2;
            let side = if n % 2 == 1 { 1 } else { -1 };
            [side * pair, VERTICAL[usize::from(slot - 1)], -pair]
        }
        // Opinionated, requested by John 2026-09-18: preserve echelon sides.
        // B43's original all-right row remains documented in the spec.
        Formation::LineAbreast => {
            let pair = (n + 1) / 2;
            let side = if n % 2 == 1 { 1 } else { -1 };
            [side * pair, n, 0]
        }
        Formation::LineAstern => [0, n, -2 * n],
    })
}

/// B43: a slot's position before variation, in the leader's heading-only
/// body frame `[lateral, vertical, longitudinal]` feet.
pub fn formation_slot_point(
    formation: Formation,
    slot: u8,
    horizontal_spacing_ft: i32,
    vertical_stacking_ft: i32,
) -> Result<[f64; 3]> {
    let [lateral, vertical, longitudinal] = slot_multipliers(formation, slot)?;
    Ok([
        f64::from(lateral * horizontal_spacing_ft),
        f64::from(vertical * vertical_stacking_ft),
        f64::from(longitudinal * horizontal_spacing_ft),
    ])
}

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
    /// Current offset in feet in draw order (first component -15..=14, the
    /// other two -50..=49). [`formation_point`] applies them as lateral,
    /// longitudinal, vertical (fitted).
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

/// Formation point relative to the leader in world feet `[x, y, z]`, `y` up.
/// `slot_point` is a body-frame point from [`formation_slot_point`]; the
/// variation offsets are added in the body frame and the horizontal pair is
/// rotated with the leader's heading (degrees clockwise seen from above).
pub fn formation_point(
    slot_point: [f64; 3],
    reference_heading_deg: f64,
    variation: &FormationVariation,
) -> [f64; 3] {
    let offset = variation.offset_ft();
    let lateral = slot_point[0] + f64::from(offset[0]);
    let vertical = slot_point[1] + f64::from(offset[2]);
    let longitudinal = slot_point[2] + f64::from(offset[1]);
    let (sin, cos) = reference_heading_deg.to_radians().sin_cos();
    [
        lateral * cos + longitudinal * sin,
        vertical,
        longitudinal * cos - lateral * sin,
    ]
}

/// B43 formation speed (mode 9): the B15 positive bands applied to the
/// spatial distance from the wingman to its slot point, on the leader's
/// speed, then clamped to own minimum and maximum. Modes 8 (pursuit
/// regulation in `pursuit`) and 9 share the band numbers and boundary
/// inclusivity; the negative bands are reachable only through a
/// lead-projection branch whose entry condition is open, so a distance to
/// the slot never selects them here.
pub fn formation_speed(
    distance_to_slot_ft: f64,
    leader_speed: ScalarSpeed,
    limits: &SpeedLimits,
) -> ScalarSpeed {
    let e = distance_to_slot_ft;
    let request = if e >= 5000. {
        limits.maximum
    } else if e > 1000. {
        leader_speed.plus(100.)
    } else if e > 250. {
        leader_speed.plus(50.)
    } else if e >= 50. {
        leader_speed.plus(25.)
    } else {
        leader_speed
    };
    request.max(limits.minimum).min(limits.maximum)
}

// ---------------------------------------------------------------------------
// B46: disengage, target loss
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WingRole {
    Leader,
    Wingman,
}

/// What a wingman is doing, as far as the wing orders decide it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WingActivity {
    Formation,
    Attacking,
    Approaching,
}

/// The wingman's order state touched by disengage and engage orders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WingmanStatus {
    pub activity: WingActivity,
    /// Set by disengage: no new target is chosen until the next engage order.
    pub hold_target_selection: bool,
}

/// B46: disengage puts the wingman back in formation at once and holds new
/// target selection.
pub fn disengage(status: &mut WingmanStatus) {
    status.activity = WingActivity::Formation;
    status.hold_target_selection = true;
}

/// B46: an engage order lifts the disengage hold.
pub fn engage_order_received(status: &mut WingmanStatus) {
    status.hold_target_selection = false;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetLossOutcome {
    ReturnToFormation,
    ResumeWaypoint,
}

/// B46: a wingman whose target is lost or destroyed returns to formation by
/// itself; a leader resumes its waypoint. There is no rejoin order or rejoin
/// distance.
pub fn on_target_lost(role: WingRole) -> TargetLossOutcome {
    match role {
        WingRole::Wingman => TargetLossOutcome::ReturnToFormation,
        WingRole::Leader => TargetLossOutcome::ResumeWaypoint,
    }
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
    pub formation: Option<Formation>,
    pub wing_control: Option<WingControl>,
    pub horizontal_spacing_ft: Option<i32>,
    /// Vertical stacking; negative is below the leader.
    pub vertical_spacing_ft: Option<i32>,
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
    /// Constant-motion training targets do not accept maneuver/formation orders.
    Dummy,
    /// The maneuver eligibility gate rejected the recipient's state.
    IneligibleState(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppliedSetting {
    Spacing { axis: SpacingAxis, feet: i32 },
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
                | TargetOrder::ClassPolicy(_) => {
                    recipient.target = None;
                    recipient.target_deadline = None;
                    None
                }
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

    fn limits() -> SpeedLimits {
        SpeedLimits {
            minimum: ScalarSpeed(100.0),
            maximum: ScalarSpeed(900.0),
            corner: ScalarSpeed(350.0),
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

    // -- formation table ----------------------------------------------------

    #[test]
    fn every_slot_multiplier_row_matches_the_table() {
        let echelon = [
            [1, 1, -1],
            [-1, -1, -1],
            [2, 2, -2],
            [-2, -2, -2],
            [3, 3, -3],
            [-3, 3, -3],
            [4, 4, -4],
            [-4, 4, -4],
            [5, 5, -5],
        ];
        for (i, row) in echelon.iter().enumerate() {
            let slot = i as u8 + 1;
            assert_eq!(
                slot_multipliers(Formation::Echelon, slot).unwrap(),
                *row,
                "echelon {slot}"
            );
            let n = i32::from(slot);
            assert_eq!(
                slot_multipliers(Formation::LineAbreast, slot).unwrap(),
                [row[0], n, 0]
            );
            assert_eq!(
                slot_multipliers(Formation::LineAstern, slot).unwrap(),
                [0, n, -2 * n]
            );
        }
        for formation in Formation::ALL {
            assert_eq!(slot_multipliers(formation, 0).unwrap(), [0; 3]);
            assert_eq!(
                slot_multipliers(formation, 10),
                Err(AiError::InvalidInput("B43 formation slot above 9"))
            );
        }
    }

    #[test]
    fn first_wingman_slot_points_at_player_spacing_values() {
        let point = |f, h, v| formation_slot_point(f, 1, h, v).unwrap();
        for (h, v, echelon, abreast, astern) in [
            (512, 0, [512., 0., -512.], [512., 0., 0.], [0., 0., -1024.]),
            (
                2048,
                512,
                [2048., 512., -2048.],
                [2048., 512., 0.],
                [0., 512., -4096.],
            ),
            (
                2048,
                -512,
                [2048., -512., -2048.],
                [2048., -512., 0.],
                [0., -512., -4096.],
            ),
            (
                512,
                512,
                [512., 512., -512.],
                [512., 512., 0.],
                [0., 512., -1024.],
            ),
            (
                512,
                -512,
                [512., -512., -512.],
                [512., -512., 0.],
                [0., -512., -1024.],
            ),
            (
                2048,
                0,
                [2048., 0., -2048.],
                [2048., 0., 0.],
                [0., 0., -4096.],
            ),
        ] {
            assert_eq!(point(Formation::Echelon, h, v), echelon, "echelon {h} {v}");
            assert_eq!(
                point(Formation::LineAbreast, h, v),
                abreast,
                "abreast {h} {v}"
            );
            assert_eq!(point(Formation::LineAstern, h, v), astern, "astern {h} {v}");
        }
        // Slot 2 in echelon is left, one low.
        assert_eq!(
            formation_slot_point(Formation::Echelon, 2, 512, 512).unwrap(),
            [-512., -512., -512.]
        );
        assert_eq!(
            formation_slot_point(Formation::LineAstern, 10, 512, 0),
            Err(AiError::InvalidInput("B43 formation slot above 9"))
        );
    }

    #[test]
    fn formation_names() {
        assert_eq!(Formation::Echelon.name(), "Echelon");
        assert_eq!(Formation::LineAbreast.name(), "Line abreast");
        assert_eq!(Formation::LineAstern.name(), "Line astern");
        assert_eq!(WingControl::Loose.name(), "Loose");
        assert_eq!(WingControl::Medium.name(), "Medium");
        assert_eq!(WingControl::Tight.name(), "Tight");
        assert_eq!(WingControl::ALL.map(WingControl::level), [1, 2, 3]);
    }

    // -- player spacing and stacking ----------------------------------------

    #[test]
    fn spacing_toggle_and_stacking_cycle() {
        assert_eq!(toggle_horizontal_spacing(512), 2048);
        assert_eq!(toggle_horizontal_spacing(2048), 512);
        // A waypoint or script spacing that is not 512 toggles to 512.
        assert_eq!(toggle_horizontal_spacing(5000), 512);
        assert_eq!(toggle_horizontal_spacing(1024), 512);
        let mut stacking = 0;
        let mut seen = Vec::new();
        for _ in 0..6 {
            stacking = cycle_vertical_stacking(stacking).unwrap();
            seen.push(stacking);
        }
        assert_eq!(seen, [512, -512, 0, 512, -512, 0]);
        assert!(cycle_vertical_stacking(1000).is_err());
    }

    #[test]
    fn idle_non_wingman_forces_loose_line_astern_and_spacing_floor() {
        assert_eq!(
            idle_non_wingman_defaults(512),
            IdleDefaults {
                control: WingControl::Loose,
                formation: Formation::LineAstern,
                horizontal_spacing_ft: 1024,
            }
        );
        assert_eq!(idle_non_wingman_defaults(1024).horizontal_spacing_ft, 1024);
        assert_eq!(idle_non_wingman_defaults(5000).horizontal_spacing_ft, 5000);
    }

    // -- control side effects and target sharing ----------------------------

    #[test]
    fn control_side_effects_for_each_order_at_each_level() {
        use WingControl::{Loose, Medium, Tight};
        let lowering = [
            PlayerOrder::EngageMyTarget,
            PlayerOrder::ProtectMe,
            PlayerOrder::AttackOnContact,
            PlayerOrder::Approach(PlayerApproach::Left),
            PlayerOrder::Approach(PlayerApproach::High),
        ];
        for order in lowering {
            for level in WingControl::ALL {
                assert_eq!(
                    apply_control_side_effect(order, level),
                    Ok(Loose),
                    "{order:?}"
                );
            }
        }
        let raising = [
            PlayerOrder::EngageFromFormation,
            PlayerOrder::Disengage,
            PlayerOrder::Formation(Formation::Echelon),
            PlayerOrder::Spacing,
            PlayerOrder::Stacking,
        ];
        for order in raising {
            assert_eq!(
                apply_control_side_effect(order, Loose),
                Ok(Medium),
                "{order:?}"
            );
            assert_eq!(apply_control_side_effect(order, Medium), Ok(Medium));
            assert_eq!(apply_control_side_effect(order, Tight), Ok(Tight));
        }
        assert_eq!(
            apply_control_side_effect(PlayerOrder::ControlToggle, Loose),
            Ok(Medium)
        );
        assert_eq!(
            apply_control_side_effect(PlayerOrder::ControlToggle, Medium),
            Ok(Loose)
        );
        assert!(apply_control_side_effect(PlayerOrder::ControlToggle, Tight).is_err());
        for level in WingControl::ALL {
            assert_eq!(
                apply_control_side_effect(PlayerOrder::Break(PlayerBreak::Left), level),
                Ok(level)
            );
        }
        assert!(leader_shares_target(Loose));
        assert!(!leader_shares_target(Medium));
        assert!(!leader_shares_target(Tight));
    }

    #[test]
    fn target_sharing_respects_the_attacker_cap() {
        assert_eq!(
            [AttackerCap::One, AttackerCap::Two, AttackerCap::Unlimited].map(AttackerCap::value),
            [1, 2, 100]
        );
        let share = |cap, attacking, wingmen| {
            let outcome = share_targets(cap, attacking, wingmen);
            (outcome.assigned, outcome.leader_engages)
        };
        // Cap 1: the leader alone fills it.
        assert_eq!(share(AttackerCap::One, 0, 3), (1, false));
        assert_eq!(share(AttackerCap::One, 1, 3), (0, false));
        assert_eq!(share(AttackerCap::One, 0, 0), (0, true));
        // Cap 2.
        assert_eq!(share(AttackerCap::Two, 0, 3), (2, false));
        assert_eq!(share(AttackerCap::Two, 0, 1), (1, true));
        assert_eq!(share(AttackerCap::Two, 1, 3), (1, false));
        assert_eq!(share(AttackerCap::Two, 2, 3), (0, false));
        assert_eq!(share(AttackerCap::Two, 5, 3), (0, false));
        // Unlimited (100).
        assert_eq!(share(AttackerCap::Unlimited, 0, 9), (9, true));
        assert_eq!(share(AttackerCap::Unlimited, 97, 9), (3, false));
        assert_eq!(share(AttackerCap::Unlimited, 99, 0), (0, true));
    }

    // -- variation and formation point --------------------------------------

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
    fn formation_point_rotates_with_heading_in_host_axes() {
        let variation = FormationVariation::new(0);
        let slot = formation_slot_point(Formation::Echelon, 1, 512, 512).unwrap();
        // Heading 0: forward is +z, so behind is -z and right is +x.
        assert_eq!(formation_point(slot, 0.0, &variation), [512., 512., -512.]);
        // Heading 90 (flying +x): behind is -x, right is -z.
        let east = formation_point(slot, 90.0, &variation);
        assert!((east[0] - -512.).abs() < 1e-9, "{east:?}");
        assert_eq!(east[1], 512.);
        assert!((east[2] - -512.).abs() < 1e-9, "{east:?}");
        // Heading 180 (flying -z): behind is +z, right is -x.
        let south = formation_point(slot, 180.0, &variation);
        assert!((south[0] - -512.).abs() < 1e-9, "{south:?}");
        assert!((south[2] - 512.).abs() < 1e-9, "{south:?}");
    }

    #[test]
    fn formation_point_adds_variation_in_body_frame() {
        let mut random = DecisionRandom::seeded(3);
        let mut variation = FormationVariation::new(0);
        assert!(variation.advance(0, &mut random));
        let [a, b, c] = variation.offset_ft().map(f64::from);
        let slot = formation_slot_point(Formation::LineAstern, 1, 1024, 0).unwrap();
        assert_eq!(formation_point(slot, 0.0, &variation), [a, c, -2048. + b]);
    }

    // -- mode 9 formation speed ---------------------------------------------

    #[test]
    fn formation_speed_bands_each_side_of_every_boundary() {
        let speed = |d| formation_speed(d, ScalarSpeed(500.), &limits());
        assert_eq!(speed(0.), ScalarSpeed(500.));
        assert_eq!(speed(49.), ScalarSpeed(500.));
        assert_eq!(speed(50.), ScalarSpeed(525.));
        assert_eq!(speed(250.), ScalarSpeed(525.));
        assert_eq!(speed(251.), ScalarSpeed(550.));
        assert_eq!(speed(1000.), ScalarSpeed(550.));
        assert_eq!(speed(1001.), ScalarSpeed(600.));
        assert_eq!(speed(4999.), ScalarSpeed(600.));
        assert_eq!(speed(5000.), ScalarSpeed(900.));
        assert_eq!(speed(50_000.), ScalarSpeed(900.));
    }

    #[test]
    fn formation_speed_clamps_to_own_limits() {
        let limits = limits();
        assert_eq!(
            formation_speed(3000., ScalarSpeed(850.), &limits),
            ScalarSpeed(900.)
        );
        assert_eq!(
            formation_speed(10., ScalarSpeed(20.), &limits),
            ScalarSpeed(100.)
        );
        assert_eq!(
            formation_speed(300., ScalarSpeed(30.), &limits),
            ScalarSpeed(100.)
        );
    }

    // -- player break and approach values -----------------------------------

    #[test]
    fn player_break_values() {
        let expect = |b: PlayerBreak, heading, pitch| {
            assert_eq!(
                b.request(),
                WingRequest::Break {
                    heading_offset_deg: heading,
                    pitch_deg: pitch
                },
                "{b:?}"
            );
        };
        expect(PlayerBreak::Left, -175, 0);
        expect(PlayerBreak::Right, 170, 0);
        expect(PlayerBreak::Low, 0, -70);
        expect(PlayerBreak::High, 0, 70);
        expect(PlayerBreak::Straight, 0, 0);
        // The receiver makes it five seconds at corner speed, own-body relative.
        let mut ai = recipient();
        assert_eq!(
            receive(PlayerBreak::Left.request(), &mut ai, 0).unwrap(),
            ReceiverOutcome::MotionInstalled(MotionSummary {
                heading_deg: 175,
                pitch_deg: 0,
                speed: ScalarSpeed(350.0),
                bank: BankSelection::Automatic,
                duration: MotionDuration::NominalSeconds(5),
            })
        );
    }

    #[test]
    fn player_approach_values() {
        let expect = |a: PlayerApproach, heading, pitch| {
            assert_eq!(
                a.request(100, 10, ScalarSpeed(0.0)),
                WingRequest::Approach {
                    heading_deg: 100 + heading,
                    pitch_deg: 10 + pitch,
                    speed: ScalarSpeed(0.0)
                },
                "{a:?}"
            );
        };
        expect(PlayerApproach::Left, -45, 0);
        expect(PlayerApproach::Right, 45, 0);
        expect(PlayerApproach::Low, 0, -35);
        expect(PlayerApproach::High, 0, 35);
    }

    #[test]
    fn approach_completes_within_two_thousand_feet() {
        assert!(approach_complete(1999.));
        assert!(approach_complete(2000.));
        assert!(!approach_complete(2001.));
        assert_eq!(after_approach(false), AfterApproach::ReturnToFormation);
        assert_eq!(after_approach(true), AfterApproach::ContinueAttack);
    }

    // -- radio ------------------------------------------------------------

    #[test]
    fn sender_phrases() {
        let phrase = |order, h| sender_phrase(order, h, WingControl::Medium);
        assert_eq!(phrase(PlayerOrder::Spacing, 512), Ok("Tighten up"));
        assert_eq!(phrase(PlayerOrder::Spacing, 999), Ok("Tighten up"));
        assert_eq!(phrase(PlayerOrder::Spacing, 1000), Ok("Combat spread"));
        assert_eq!(phrase(PlayerOrder::Spacing, 2048), Ok("Combat spread"));
        assert_eq!(
            phrase(PlayerOrder::Formation(Formation::LineAbreast), 512),
            Ok("Line abreast")
        );
        assert_eq!(phrase(PlayerOrder::ControlToggle, 512), Ok("Medium"));
        assert_eq!(
            sender_phrase(PlayerOrder::ControlToggle, 512, WingControl::Loose),
            Ok("Loose")
        );
        assert!(phrase(PlayerOrder::EngageMyTarget, 512).is_err());
        assert!(phrase(PlayerOrder::Stacking, 512).is_err());
    }

    #[test]
    fn wingman_reply_rules() {
        assert_eq!(
            wingman_reply(PlayerOrder::EngageMyTarget, true, true),
            Some("Engaging")
        );
        assert_eq!(
            wingman_reply(PlayerOrder::EngageMyTarget, false, true),
            None
        );
        assert_eq!(
            wingman_reply(PlayerOrder::EngageMyTarget, true, false),
            None
        );
        assert_eq!(
            wingman_reply(PlayerOrder::ProtectMe, false, true),
            Some("Showtime!")
        );
        assert_eq!(
            wingman_reply(PlayerOrder::ProtectMe, true, true),
            Some("Showtime!")
        );
        assert_eq!(wingman_reply(PlayerOrder::ProtectMe, true, false), None);
        for order in [
            PlayerOrder::AttackOnContact,
            PlayerOrder::EngageFromFormation,
            PlayerOrder::Disengage,
            PlayerOrder::Break(PlayerBreak::High),
            PlayerOrder::Approach(PlayerApproach::Right),
            PlayerOrder::Formation(Formation::Echelon),
            PlayerOrder::Spacing,
            PlayerOrder::Stacking,
            PlayerOrder::ControlToggle,
        ] {
            assert_eq!(wingman_reply(order, true, true), None, "{order:?}");
        }
    }

    // -- disengage and target loss ------------------------------------------

    #[test]
    fn disengage_returns_to_formation_and_holds_selection() {
        let mut status = WingmanStatus {
            activity: WingActivity::Attacking,
            hold_target_selection: false,
        };
        disengage(&mut status);
        assert_eq!(
            status,
            WingmanStatus {
                activity: WingActivity::Formation,
                hold_target_selection: true,
            }
        );
        engage_order_received(&mut status);
        assert!(!status.hold_target_selection);
        assert_eq!(status.activity, WingActivity::Formation);
    }

    #[test]
    fn target_loss_outcomes_by_role() {
        assert_eq!(
            on_target_lost(WingRole::Wingman),
            TargetLossOutcome::ReturnToFormation
        );
        assert_eq!(
            on_target_lost(WingRole::Leader),
            TargetLossOutcome::ResumeWaypoint
        );
    }

    // -- receiver -----------------------------------------------------------

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
            feet: -512,
        };
        assert_eq!(
            receive(spacing, &mut ai, 0).unwrap(),
            ReceiverOutcome::Applied(AppliedSetting::Spacing {
                axis: SpacingAxis::Vertical,
                feet: -512
            })
        );
        assert_eq!(ai.vertical_spacing_ft, Some(-512));
        assert_eq!(
            receive(
                WingRequest::FormationSelection(Formation::LineAbreast),
                &mut ai,
                0
            )
            .unwrap(),
            ReceiverOutcome::Applied(AppliedSetting::FormationSelection {
                cleared_active_command: true
            })
        );
        assert_eq!(ai.formation, Some(Formation::LineAbreast));
        assert!(!ai.active_command);
        ai.active_command = true;
        assert_eq!(
            receive(WingRequest::WingControl(WingControl::Loose), &mut ai, 0).unwrap(),
            ReceiverOutcome::Applied(AppliedSetting::WingControl)
        );
        assert_eq!(ai.wing_control, Some(WingControl::Loose));
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
        // Explicit cancellation clears the stale target, not deadline expiry.
        assert_eq!(ai.target, None);
        assert_eq!(ai.target_deadline, None);
    }
}
