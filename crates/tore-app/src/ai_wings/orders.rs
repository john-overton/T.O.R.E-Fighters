//! Player delivery uses the same B46 receiver as AI team requests.
use super::*;
use tore_sim::{
    ai::{
        airfield::{AirfieldAnchors, LandingOrder, LandingReason, Phase, RunwayView},
        engagement::{Assignment, Role, Stance},
        wing::{
            self, PlayerOrder, ReceiverOutcome, SpacingAxis, TargetId, TargetOrder, WingControl,
            WingRequest,
        },
    },
    airport::{Allegiance, Scene, Service},
};

pub struct OrderReport {
    pub message: String,
    /// The player's own order call, played at once. The wingman's reply is
    /// a [`Chatter`] event delivered through the radio channel.
    pub radio: Vec<&'static str>,
}

/// The player's aircraft as the landing-priority rule sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PlayerLanding {
    position: [f64; 3],
    /// Height above the surface below, feet.
    agl_ft: f64,
    gear_down: bool,
    speed_fps: f64,
    /// Alive, not crashed and not ejected.
    flying: bool,
    /// Wheels on the ground.
    on_ground: bool,
    /// Horizontal velocity, ft/s: [x, z].
    track: [f64; 2],
}

/// `spec-derived` (docs/formats/ai.md, "Airfield takeoff and landing
/// sequences", APLanding): the player counts as landing, and AI aircraft
/// hold at marshal, with gear down, below 4,000 ft above the ground, at no
/// more than 953 ft/s and within 25,000 ft of the nearest friendly airport.
pub const PRIORITY_MAX_AGL_FT: f64 = 4_000.;
/// See [`PRIORITY_MAX_AGL_FT`].
pub const PRIORITY_MAX_SPEED_FPS: f64 = 953.;
/// See [`PRIORITY_MAX_AGL_FT`].
pub const PRIORITY_RANGE_FT: f64 = 25_000.;
/// `fitted` (agent decision, 2026-09-23): retail keeps a player who has
/// just taken off in climb-out (0x12, which does not block the runway) until
/// it is "aligned with the approach" (callback +0x10, not decoded). Here that
/// means a horizontal track within this many degrees of a runway direction,
/// with that runway's nearer end still ahead.
pub const PRIORITY_ALIGNED_DEG: f64 = 30.;

/// Where "land at selected airport" sends the wing: the airport the player
/// selected for the tower (Shift-A) and one of its usable runways.
#[derive(Clone, Debug, PartialEq)]
pub struct LandingSite {
    pub name: String,
    pub runway: RunwayView,
}

impl AiWings {
    /// Resolve the tower's selected airport for a wing landing order.
    ///
    /// `opinionated` (agent decision, 2026-09-23): wingmen follow the tower's
    /// own acceptance rule, so a hostile, unknown or unpermitted neutral
    /// airport is refused. They use the player's cleared runway when the
    /// player holds a clearance there, otherwise the longest usable runway
    /// (lowest object id on a tie).
    pub fn landing_site(
        scene: &Scene,
        anchors: &BTreeMap<u32, AirfieldAnchors>,
        service: &Service,
    ) -> Result<LandingSite, String> {
        let airport = service
            .selected()
            .and_then(|id| scene.airports.iter().find(|a| a.id == id))
            .ok_or("Wing order unavailable: no airport selected (Shift-A selects one)")?;
        let name = &airport.name;
        match airport.allegiance {
            Allegiance::Hostile => {
                return Err(format!("Wing order unavailable: {name} is hostile"));
            }
            Allegiance::Unknown => {
                return Err(format!(
                    "Wing order unavailable: {name} allegiance is unknown"
                ));
            }
            Allegiance::Neutral if !airport.neutral_permission => {
                return Err(format!(
                    "Wing order unavailable: {name} has not granted landing permission"
                ));
            }
            Allegiance::Friendly | Allegiance::Neutral => {}
        }
        let cleared = service
            .clearance()
            .filter(|(id, runway, _)| {
                *id == airport.id && service.usable(*runway) && !scene.vertical_pad(*runway)
            })
            .and_then(|(_, runway, _)| scene.runway(runway));
        let runway = cleared
            .or_else(|| {
                airport
                    .runway_objects
                    .iter()
                    .filter(|id| service.usable(**id) && !scene.vertical_pad(**id))
                    .filter_map(|id| scene.runway(*id))
                    .min_by(|a, b| {
                        b.length_ft
                            .total_cmp(&a.length_ft)
                            .then(a.object.cmp(&b.object))
                    })
            })
            .ok_or_else(|| {
                format!("Wing order unavailable: {name} has no runway your wingmen can land on")
            })?;
        let mut runway_view = RunwayView::from(runway);
        runway_view.anchors = anchors.get(&runway.object).copied();
        Ok(LandingSite {
            name: name.clone(),
            runway: runway_view,
        })
    }

    /// Re-evaluate the player's landing priority for this tick and pass it
    /// to the AI ([`Self::set_priority_landing`]). Friendly means the
    /// tower's own rule: a friendly airport or a neutral one that grants
    /// permission.
    ///
    /// Spec-derived (docs/formats/ai.md, `_ServicePlayer`): a player on the
    /// ground is in the takeoff states, not landing; a rolling player blocks
    /// the runway through the runway-free gate instead. After a takeoff the
    /// player stays in climb-out, which does not block, until it is aligned
    /// with an approach ([`PRIORITY_ALIGNED_DEG`], fitted) or leaves the
    /// landing condition.
    ///
    /// `fitted` (agent decision, 2026-09-23): an airport's distance is the
    /// horizontal distance to its nearest usable runway centre, because the
    /// scene has no single airport position, and an airport with no usable
    /// runway is skipped.
    pub fn update_player_landing(
        &mut self,
        scene: &Scene,
        service: &Service,
        flight: &flight::State,
        surface_ft: f64,
    ) {
        let player = PlayerLanding {
            position: flight.position,
            agl_ft: flight.position[1] - surface_ft,
            gear_down: flight.gear_down,
            speed_fps: flight.speed,
            flying: !flight.crashed && !flight.systems.pilot.dead && flight.escape.is_none(),
            on_ground: flight.supported_at(surface_ft),
            track: [flight.velocity[0], flight.velocity[2]],
        };
        let (airport, departing) =
            Self::landing_priority(scene, service, player, self.player_departing);
        self.player_departing = departing;
        self.set_priority_landing(airport);
    }

    /// The priority airport and whether the player is still climbing out
    /// after a takeoff.
    fn landing_priority(
        scene: &Scene,
        service: &Service,
        player: PlayerLanding,
        departing: bool,
    ) -> (Option<u32>, bool) {
        if player.flying && player.on_ground {
            return (None, true);
        }
        if !player.flying
            || !player.gear_down
            || player.agl_ft >= PRIORITY_MAX_AGL_FT
            || player.speed_fps > PRIORITY_MAX_SPEED_FPS
        {
            return (None, false);
        }
        let [x, _, z] = player.position;
        let Some((_, airport)) = scene
            .airports
            .iter()
            .filter(|a| {
                a.allegiance == Allegiance::Friendly
                    || (a.allegiance == Allegiance::Neutral && a.neutral_permission)
            })
            .filter_map(|a| {
                a.runway_objects
                    .iter()
                    .filter(|id| service.usable(**id))
                    .filter_map(|id| scene.runway(*id))
                    .map(|r| (r.approach_center[0] - x).hypot(r.approach_center[2] - z))
                    .min_by(f64::total_cmp)
                    .map(|distance| (distance, a.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .filter(|(distance, _)| *distance <= PRIORITY_RANGE_FT)
        else {
            return (None, false);
        };
        if departing && !Self::aligned(scene, service, airport, player) {
            return (None, true);
        }
        (Some(airport), false)
    }

    /// Whether the player's track lines up with one of the airport's usable
    /// runways, either direction, with the nearer runway end still ahead.
    fn aligned(scene: &Scene, service: &Service, airport: u32, player: PlayerLanding) -> bool {
        let [vx, vz] = player.track;
        let speed = vx.hypot(vz);
        if speed < 1. {
            return false;
        }
        let [x, _, z] = player.position;
        scene
            .airports
            .iter()
            .filter(|a| a.id == airport)
            .flat_map(|a| a.runway_objects.iter())
            .filter(|id| service.usable(**id))
            .filter_map(|id| scene.runway(*id))
            .any(|r| {
                let (sin, cos) = r.heading.sin_cos();
                let along_track = (vx * sin + vz * cos) / speed;
                let direction = along_track.signum();
                let ahead = ((r.approach_center[0] - x) * sin + (r.approach_center[2] - z) * cos)
                    * direction;
                along_track.abs() >= PRIORITY_ALIGNED_DEG.to_radians().cos()
                    && ahead > r.length_ft * 0.5
            })
    }

    /// The player is landing at this airport: AI aircraft landing there hold
    /// at marshal until it clears (manual p.65).
    pub fn set_priority_landing(&mut self, airport: Option<u32>) {
        self.mission.set_priority_landing(airport);
    }

    /// FA Alt+T: the formation after the one the first addressed wingman
    /// flies, cycling echelon, line abreast, line astern.
    pub fn next_formation(&self, recipient: Option<u8>) -> wing::Formation {
        let current = self
            .mission
            .actors()
            .iter()
            .filter(|a| a.alive() && a.identity().side == FRIENDLY_SIDE && a.identity().wing == 0)
            .filter(|a| recipient.is_none_or(|wanted| a.identity().member == wanted))
            .min_by_key(|a| a.identity().member)
            .and_then(|a| a.controller().ordered_formation())
            .unwrap_or(self.mission.formation());
        let all = wing::Formation::ALL;
        let index = all.iter().position(|f| *f == current).unwrap_or(0);
        all[(index + 1) % all.len()]
    }

    /// [`Self::command_at`] without a landing site, as the tests use it.
    #[cfg(test)]
    pub fn command(
        &mut self,
        order: PlayerOrder,
        selected: Option<u32>,
        recipient: Option<u8>,
    ) -> AppResult<OrderReport> {
        self.command_at(order, selected, recipient, None)
    }

    /// [`Self::command`] with the resolved [`LandingSite`] that "land at
    /// selected airport" needs. Other orders ignore `site`.
    pub fn command_at(
        &mut self,
        order: PlayerOrder,
        selected: Option<u32>,
        recipient: Option<u8>,
        site: Option<&LandingSite>,
    ) -> AppResult<OrderReport> {
        if matches!(order, PlayerOrder::BugOut | PlayerOrder::LandAtSelected) {
            return self.command_landing(order, recipient, site);
        }
        let mut members: Vec<_> = self
            .mission
            .actors()
            .iter()
            .filter(|a| a.alive() && a.identity().side == FRIENDLY_SIDE && a.identity().wing == 0)
            .map(|a| (a.identity().member, a.id()))
            .collect();
        members.sort_unstable();
        let first = members.first().map(|(_, id)| *id);
        members.retain(|(member, _)| recipient.is_none_or(|wanted| *member == wanted));
        if members.is_empty() {
            return Ok(unavailable_no_wingmen());
        }
        // A bugged-out wingman no longer answers orders (manual p.160).
        let before = members.len();
        members.retain(|(_, id)| !self.mission.actor(*id).unwrap().bugged_out());
        let bugged_out = before - members.len();
        if members.is_empty() {
            return Ok(OrderReport {
                message: bugged_out_notice(bugged_out, recipient),
                radio: vec![],
            });
        }
        let needs_target = matches!(
            order,
            PlayerOrder::EngageMyTarget
                | PlayerOrder::EngageFromFormation
                | PlayerOrder::Approach(_)
        );
        let target = selected.filter(|id| {
            self.mission
                .actor(*id)
                .is_some_and(|a| a.alive() && a.identity().side != FRIENDLY_SIDE)
        });
        if needs_target && target.is_none() {
            return Ok(OrderReport {
                message: "Wing order unavailable: no valid hostile target".into(),
                radio: vec![],
            });
        }
        let mut applied = 0;
        let mut rejected = 0;
        let mut no_motion = 0;
        let mut sender = None;
        for (_, id) in &members {
            let actor = self.mission.actor(*id).unwrap();
            let (control, horizontal, vertical) = actor.controller().wing_settings();
            let old_control = control.unwrap_or(WingControl::Loose);
            let control =
                wing::apply_control_side_effect(order, old_control).map_err(|e| e.to_string())?;
            let horizontal = if order == PlayerOrder::Spacing {
                wing::toggle_horizontal_spacing(horizontal.unwrap_or(512))
            } else {
                horizontal.unwrap_or(512)
            };
            let vertical = if order == PlayerOrder::Stacking {
                wing::cycle_vertical_stacking(vertical.unwrap_or(0)).map_err(|e| e.to_string())?
            } else {
                vertical.unwrap_or(0)
            };
            let request = match order {
                PlayerOrder::BugOut | PlayerOrder::LandAtSelected => {
                    unreachable!("landing orders take command_landing")
                }
                PlayerOrder::Break(b) => b.request(),
                PlayerOrder::Formation(f) => WingRequest::FormationSelection(f),
                PlayerOrder::Spacing => WingRequest::Spacing {
                    axis: SpacingAxis::Horizontal,
                    feet: horizontal,
                },
                PlayerOrder::Stacking => WingRequest::Spacing {
                    axis: SpacingAxis::Vertical,
                    feet: vertical,
                },
                PlayerOrder::ControlToggle => WingRequest::WingControl(control),
                PlayerOrder::Disengage => WingRequest::TargetAssignment(TargetOrder::HoldFire),
                PlayerOrder::AttackOnContact | PlayerOrder::ProtectMe => {
                    WingRequest::TargetAssignment(TargetOrder::FreeSelection)
                }
                PlayerOrder::EngageMyTarget | PlayerOrder::EngageFromFormation => {
                    WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(
                        target.unwrap(),
                    )))
                }
                PlayerOrder::Approach(a) => {
                    let target_actor = self.mission.actor(target.unwrap()).unwrap();
                    let own = actor.flight().position;
                    let point = target_actor.flight().position;
                    let dx = point[0] - own[0];
                    let dy = point[1] - own[1];
                    let dz = point[2] - own[2];
                    a.request(
                        dx.atan2(dz).to_degrees().round() as i32,
                        dy.atan2(dx.hypot(dz)).to_degrees().round() as i32,
                        ScalarSpeed(0.),
                    )
                }
            };
            if sender.is_none() {
                sender = Some(sender_stem(order, horizontal, vertical, control));
            }
            if needs_target
                && actor.sensors().is_some_and(|s| {
                    !s.contacts().iter().any(|c| Some(c.id) == target)
                        && !s.visual().iter().any(|c| Some(c.id) == target)
                })
            {
                rejected += 1;
                continue;
            }
            // Apply the silent control side effect before installing motion.
            // Break has no side effect, so it cannot cancel itself here.
            if control != old_control {
                self.mission
                    .order(*id, WingRequest::WingControl(control))
                    .unwrap()
                    .map_err(|e| e.to_string())?;
            }
            if matches!(order, PlayerOrder::Approach(_)) {
                self.mission
                    .order(
                        *id,
                        WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(
                            target.unwrap(),
                        ))),
                    )
                    .unwrap()
                    .map_err(|e| e.to_string())?;
            }
            let outcome = self
                .mission
                .order(*id, request)
                .unwrap()
                .map_err(|e| e.to_string())?;
            if matches!(order, PlayerOrder::Formation(_) | PlayerOrder::Disengage)
                && !matches!(outcome, ReceiverOutcome::Rejected(_))
            {
                // The sim has already debited a launched burst. Stop its
                // remaining physical gun shots when recall is accepted.
                // Intentional cancellation is not a failed projectile launch.
                self.pending_guns
                    .retain(|(queued_actor, _), _| *queued_actor != *id);
            }
            match outcome {
                ReceiverOutcome::Applied(_) | ReceiverOutcome::MotionInstalled(_) => {
                    if let Some(assignment) = mission_assignment(order, target) {
                        self.mission
                            .actor_mut(*id)
                            .unwrap()
                            .set_assignment(assignment);
                    }
                    applied += 1;
                    if let PlayerOrder::Approach(a) = order {
                        self.mission.track_ordered_approach(
                            *id,
                            target.unwrap(),
                            f64::from(a.heading_offset_deg()),
                            f64::from(a.pitch_offset_deg()),
                        );
                    }
                    // The first wingman's reply goes through the radio
                    // channel two seconds later (docs/spec/radio-chatter.md).
                    if Some(*id) == first {
                        match order {
                            PlayerOrder::EngageMyTarget
                            | PlayerOrder::EngageFromFormation
                            | PlayerOrder::AttackOnContact => self.engaged(*id, target),
                            PlayerOrder::ProtectMe => {
                                self.chat(Chatter::Showtime { speaker: *id });
                            }
                            _ => {}
                        }
                    }
                }
                ReceiverOutcome::Rejected(_) => rejected += 1,
                ReceiverOutcome::AppliedNoMotion => {
                    if let Some(assignment) = mission_assignment(order, target) {
                        self.mission
                            .actor_mut(*id)
                            .unwrap()
                            .set_assignment(assignment);
                    }
                    no_motion += 1;
                }
            }
        }
        let mut radio = Vec::new();
        if let Some(stem) = sender.flatten() {
            radio.push(stem);
        }
        let mut message = format!(
            "{}: {applied} applied, {rejected} rejected, {no_motion} without motion",
            order_label(order)
        );
        if bugged_out > 0 {
            message.push_str(&format!(", {bugged_out} bugged out"));
        }
        Ok(OrderReport { message, radio })
    }

    /// Bug out and land at the selected airport: each addressed wingman
    /// leaves the wing to land. The runway is its own home runway for a bug
    /// out and the player's chosen airport for an ordered landing.
    fn command_landing(
        &mut self,
        order: PlayerOrder,
        recipient: Option<u8>,
        site: Option<&LandingSite>,
    ) -> AppResult<OrderReport> {
        let mut members: Vec<_> = self
            .mission
            .actors()
            .iter()
            .filter(|a| a.alive() && a.identity().side == FRIENDLY_SIDE && a.identity().wing == 0)
            .filter(|a| recipient.is_none_or(|wanted| a.identity().member == wanted))
            .map(|a| (a.identity().member, a.id()))
            .collect();
        members.sort_unstable();
        if members.is_empty() {
            return Ok(unavailable_no_wingmen());
        }
        let (label, reason) = match (order, site) {
            (PlayerOrder::BugOut, _) => ("Bug out".to_owned(), LandingReason::BugOut),
            (_, Some(site)) => (format!("Land at {}", site.name), LandingReason::Ordered),
            (_, None) => {
                return Ok(OrderReport {
                    message: "Wing order unavailable: no airport selected (Shift-A selects one)"
                        .into(),
                    radio: vec![],
                });
            }
        };
        let (mut accepted, mut rejected, mut no_base, mut bugged_out, mut human) = (0, 0, 0, 0, 0);
        let (mut busy, mut landed) = (0, 0);
        for (_, id) in members {
            let actor = self.mission.actor(id).unwrap();
            if actor.identity().human_controlled {
                human += 1;
                continue;
            }
            if actor.bugged_out() {
                bugged_out += 1;
                continue;
            }
            // Retail ignores bug out in any airport state or on the ground
            // (B46 subcode 0x10, docs/formats/ai.md). The private route home
            // is free flight, so it does not count; a parked aircraft has
            // already landed.
            let on_ground = actor
                .flight()
                .research
                .as_ref()
                .is_some_and(|r| r.on_ground);
            let phase = actor.airfield_phase();
            if phase == Some(Phase::Parked) {
                landed += 1;
                continue;
            }
            if reason == LandingReason::BugOut
                && (phase.is_some_and(|p| p != Phase::Inbound) || on_ground)
            {
                busy += 1;
                continue;
            }
            let runway = match site {
                Some(site) if reason == LandingReason::Ordered => Some(site.runway),
                _ => actor.home_runway().copied(),
            };
            let Some(runway) = runway else {
                no_base += 1;
                continue;
            };
            let outcome = self
                .mission
                .order(id, WingRequest::Land(LandingOrder { runway, reason }))
                .unwrap()
                .map_err(|e| e.to_string())?;
            if let ReceiverOutcome::Rejected(why) = outcome {
                match why {
                    wing::RejectReason::BuggedOut => bugged_out += 1,
                    wing::RejectReason::Landed => landed += 1,
                    _ => rejected += 1,
                }
            } else {
                // A wingman leaving to land stops its remaining queued gun
                // shots, as a recall does.
                self.pending_guns
                    .retain(|(queued_actor, _), _| *queued_actor != id);
                accepted += 1;
            }
        }
        if accepted == 0 && rejected == 0 && no_base == 0 && human == 0 && busy == 0 && landed == 0
        {
            return Ok(OrderReport {
                message: bugged_out_notice(bugged_out, recipient),
                radio: vec![],
            });
        }
        let mut parts = vec![format!(
            "{accepted} {}",
            if reason == LandingReason::BugOut {
                "returning to base"
            } else {
                "landing"
            }
        )];
        if no_base > 0 {
            parts.push(format!("{no_base} with no base"));
        }
        if busy > 0 {
            parts.push(format!("{busy} already taking off or landing"));
        }
        if landed > 0 {
            parts.push(format!("{landed} already landed"));
        }
        if bugged_out > 0 {
            parts.push(format!("{bugged_out} bugged out"));
        }
        if rejected > 0 {
            parts.push(format!("{rejected} rejected"));
        }
        if human > 0 {
            parts.push(format!("{human} flown by a human"));
        }
        Ok(OrderReport {
            message: format!("{label}: {}", parts.join(", ")),
            radio: vec![],
        })
    }
}

fn unavailable_no_wingmen() -> OrderReport {
    OrderReport {
        message: "Wing order unavailable: no addressed wingmen".into(),
        radio: vec![],
    }
}

/// Every addressed wingman has bugged out and no longer answers.
fn bugged_out_notice(count: usize, recipient: Option<u8>) -> String {
    match recipient {
        Some(member) => {
            format!("Wing order unavailable: wingman {member} bugged out and no longer answers")
        }
        None if count == 1 => {
            "Wing order unavailable: the wingman bugged out and no longer answers".into()
        }
        None => "Wing order unavailable: all wingmen bugged out and no longer answer".into(),
    }
}

fn mission_assignment(order: PlayerOrder, target: Option<u32>) -> Option<Assignment> {
    match order {
        PlayerOrder::ProtectMe => Some(Assignment {
            role: Role::Escort,
            stance: Stance::ProtectAssigned,
            protected_ids: vec![PLAYER_ID],
            ..Assignment::default()
        }),
        PlayerOrder::EngageMyTarget => Some(Assignment {
            role: Role::Intercept,
            stance: Stance::EngageAssigned,
            destroy_ids: target.into_iter().collect(),
            ..Assignment::default()
        }),
        PlayerOrder::AttackOnContact => Some(Assignment::default()),
        PlayerOrder::Disengage
        | PlayerOrder::EngageFromFormation
        | PlayerOrder::Break(_)
        | PlayerOrder::Approach(_)
        | PlayerOrder::Formation(_)
        | PlayerOrder::Spacing
        | PlayerOrder::Stacking
        | PlayerOrder::ControlToggle
        | PlayerOrder::BugOut
        | PlayerOrder::LandAtSelected => None,
    }
}

fn sender_stem(
    order: PlayerOrder,
    horizontal: i32,
    vertical: i32,
    control: WingControl,
) -> Option<&'static str> {
    use wing::{Formation as F, PlayerApproach as A, PlayerBreak as B};
    Some(match order {
        PlayerOrder::Break(b) => match b {
            B::Left => "^BREAKLF",
            B::Right => "^BREAKRT",
            B::High => "^BREAKHI",
            B::Low => "^BREAKLO",
            B::Straight => "^STEADY",
        },
        PlayerOrder::Approach(a) => match a {
            A::Left => "^APPRCLF",
            A::Right => "^APPRCRT",
            A::High => "^APPRCHI",
            A::Low => "^APPRCLO",
        },
        PlayerOrder::Formation(f) => match f {
            F::Echelon => "^ECHFORM",
            F::LineAbreast => "^ABRFORM",
            F::LineAstern => "^ASTFORM",
        },
        PlayerOrder::Spacing => {
            if horizontal < 1000 {
                "^TIGHTEN"
            } else {
                "^CBTSPRD"
            }
        }
        PlayerOrder::Stacking => {
            if vertical > 0 {
                "^FORMHI"
            } else if vertical < 0 {
                "^FORMLOW"
            } else {
                "^FORMLVL"
            }
        }
        PlayerOrder::ControlToggle => {
            if control == WingControl::Loose {
                "^LOSFORM"
            } else {
                "^MEDFORM"
            }
        }
        PlayerOrder::Disengage => "^DISENG",
        PlayerOrder::ProtectMe => "^CLRMY6",
        // "Attack" alone for attack on contact (docs/spec/radio-chatter.md).
        PlayerOrder::EngageMyTarget | PlayerOrder::AttackOnContact => "^ATTACK",
        // Sender mapping for engage from formation is not yet established.
        PlayerOrder::EngageFromFormation => return None,
        // No reviewed sender recording for these orders.
        PlayerOrder::BugOut | PlayerOrder::LandAtSelected => return None,
    })
}

fn order_label(order: PlayerOrder) -> &'static str {
    use wing::{Formation as F, PlayerApproach as A, PlayerBreak as B};
    match order {
        PlayerOrder::BugOut => "Bug out",
        PlayerOrder::LandAtSelected => "Land at selected airport",
        PlayerOrder::Break(b) => match b {
            B::Left => "Break left",
            B::Right => "Break right",
            B::High => "Break high",
            B::Low => "Break low",
            B::Straight => "Steady",
        },
        PlayerOrder::Approach(a) => match a {
            A::Left => "Approach left",
            A::Right => "Approach right",
            A::High => "Approach high",
            A::Low => "Approach low",
        },
        PlayerOrder::Formation(f) => match f {
            F::Echelon => "Echelon",
            F::LineAbreast => "Line abreast",
            F::LineAstern => "Line astern",
        },
        PlayerOrder::Spacing => "Spacing",
        PlayerOrder::Stacking => "Stacking",
        PlayerOrder::ControlToggle => "Wing control",
        PlayerOrder::Disengage => "Disengage",
        PlayerOrder::ProtectMe => "Protect me",
        PlayerOrder::EngageMyTarget => "Engage my target",
        PlayerOrder::AttackOnContact => "Attack on contact",
        PlayerOrder::EngageFromFormation => "Engage from formation",
    }
}

#[cfg(test)]
mod engagement_tests {
    use super::*;

    #[test]
    fn accepted_policy_orders_map_to_persistent_assignments() {
        let protect = mission_assignment(PlayerOrder::ProtectMe, Some(9)).unwrap();
        assert_eq!(protect.role, Role::Escort);
        assert_eq!(protect.stance, Stance::ProtectAssigned);
        assert_eq!(protect.protected_ids, [PLAYER_ID]);
        assert!(protect.destroy_ids.is_empty());

        let engage = mission_assignment(PlayerOrder::EngageMyTarget, Some(9)).unwrap();
        assert_eq!(engage.role, Role::Intercept);
        assert_eq!(engage.destroy_ids, [9]);

        assert_eq!(
            mission_assignment(PlayerOrder::AttackOnContact, None),
            Some(Assignment::default())
        );
        assert_eq!(mission_assignment(PlayerOrder::Disengage, None), None);
    }

    #[test]
    fn routine_motion_and_formation_orders_do_not_rewrite_mission_policy() {
        use wing::{Formation, PlayerBreak};
        for order in [
            PlayerOrder::Break(PlayerBreak::Left),
            PlayerOrder::Formation(Formation::Echelon),
            PlayerOrder::Spacing,
            PlayerOrder::Stacking,
            PlayerOrder::ControlToggle,
        ] {
            assert_eq!(mission_assignment(order, Some(9)), None);
        }
    }
}

#[cfg(test)]
mod landing_tests {
    use super::*;
    use tore_sim::{
        ai::launch::{WingId, WingSelection, resolve_wings},
        airport::{
            Aircraft as Plane, Airport, Command, OrientedBox, Runway, SourceKey, StaticObject,
        },
        combat::missiles::{TargetRole, seeker::Heat},
    };

    fn strip(id: u32, airport: u32, x: f64, length_ft: f64) -> (StaticObject, Runway) {
        let bounds = OrientedBox {
            center: [x, 100., 0.],
            half: [100., 10., length_ft / 2.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        };
        (
            StaticObject {
                id,
                source: SourceKey {
                    layout: "X.MM".into(),
                    ordinal: id,
                },
                name: "Strip".into(),
                object_type: "STRIP.OT".into(),
                bounds,
                hit_points: 100,
                category: 0x100,
                radar_signature: 100.,
                infrared_signature: 100.,
                runway: true,
            },
            Runway {
                object: id,
                airport,
                name: format!("{id}"),
                surface: bounds,
                approach_center: bounds.center,
                elevation_ft: 100.,
                heading: 0.,
                length_ft,
            },
        )
    }

    /// Field (7): a long runway far east and a short one at the origin.
    /// Hostile (8), an unpermitted neutral (9) and a field without a
    /// runway (10).
    fn scene() -> Scene {
        let strips = [
            strip(1000, 7, 50_000., 10_000.),
            strip(1001, 7, 0., 6_000.),
            strip(1002, 8, 90_000., 8_000.),
            strip(1003, 9, 120_000., 8_000.),
        ];
        let airport = |id, name: &str, runway_objects, allegiance| Airport {
            id,
            name: name.into(),
            runway_objects,
            allegiance,
            neutral_permission: false,
        };
        Scene {
            objects: strips.iter().map(|(o, _)| o.clone()).collect(),
            runways: strips.iter().map(|(_, r)| r.clone()).collect(),
            airports: vec![
                airport(7, "Field", vec![1000, 1001], Allegiance::Friendly),
                airport(8, "Enemy Base", vec![1002], Allegiance::Hostile),
                airport(9, "Neutral Base", vec![1003], Allegiance::Neutral),
                airport(10, "Grass Field", vec![], Allegiance::Friendly),
            ],
        }
    }

    fn plane() -> Plane {
        Plane {
            position: [0., 3000., -20_000.],
            forward: [0., 0., 1.],
            nav_mode: true,
            gear_down: true,
            supported: false,
            alive: true,
            speed_fps: 300.,
        }
    }

    fn select(scene: &Scene, service: &mut Service, airport: u32) {
        service.command(scene, plane(), Command::SelectAirport(airport));
    }

    #[test]
    fn landing_site_needs_a_selected_airport_that_accepts_the_wing() {
        let scene = scene();
        let mut service = Service::new(&scene).unwrap();
        let err = AiWings::landing_site(&scene, &BTreeMap::new(), &service).unwrap_err();
        assert!(
            err.contains("no airport selected") && err.contains("Shift-A"),
            "{err}"
        );

        select(&scene, &mut service, 8);
        assert!(
            AiWings::landing_site(&scene, &BTreeMap::new(), &service)
                .unwrap_err()
                .contains("hostile")
        );
        select(&scene, &mut service, 9);
        assert!(
            AiWings::landing_site(&scene, &BTreeMap::new(), &service)
                .unwrap_err()
                .contains("permission")
        );
        select(&scene, &mut service, 10);
        assert!(
            AiWings::landing_site(&scene, &BTreeMap::new(), &service)
                .unwrap_err()
                .contains("no runway your wingmen can land on")
        );

        // The longest usable runway, unless the player is cleared elsewhere.
        select(&scene, &mut service, 7);
        let site = AiWings::landing_site(&scene, &BTreeMap::new(), &service).unwrap();
        assert_eq!(site.name, "Field");
        assert_eq!(site.runway, RunwayView::from(scene.runway(1000).unwrap()));
        service.command(&scene, plane(), Command::RequestLanding);
        assert_eq!(service.clearance().map(|c| c.1), Some(1001));
        let site = AiWings::landing_site(&scene, &BTreeMap::new(), &service).unwrap();
        assert_eq!(
            site.runway.object, 1001,
            "wingmen follow the player's runway"
        );

        // A destroyed runway is skipped.
        service.command(&scene, plane(), Command::CancelApproach);
        service.damage(1000, 1000);
        let site = AiWings::landing_site(&scene, &BTreeMap::new(), &service).unwrap();
        assert_eq!(site.runway.object, 1001);
        service.damage(1001, 1000);
        assert!(AiWings::landing_site(&scene, &BTreeMap::new(), &service).is_err());
    }

    /// Regression, found at Goose Green (LFA, 2026-09-23): wingmen sent to
    /// land on a vertical pad crashed. Conventional aircraft never land on
    /// one (DTSTRP), so the wing lands on the airport's other runway, and an
    /// airport with only a pad refuses the order.
    #[test]
    fn wingmen_never_land_on_a_vertical_pad() {
        let mut scene = scene();
        let (mut pad, pad_runway) = strip(1004, 7, 20_000., 800.);
        pad.object_type = "DTSTRP.OT".into();
        scene.objects.push(pad);
        scene.runways.push(pad_runway);
        scene.airports[0].runway_objects.push(1004);
        assert!(scene.vertical_pad(1004) && !scene.vertical_pad(1000));
        let mut service = Service::new(&scene).unwrap();
        select(&scene, &mut service, 7);
        let site = AiWings::landing_site(&scene, &BTreeMap::new(), &service).unwrap();
        assert_ne!(site.runway.object, 1004);
        // An airport with nothing but a pad.
        scene.airports[0].runway_objects = vec![1004];
        let mut service = Service::new(&scene).unwrap();
        select(&scene, &mut service, 7);
        let err = AiWings::landing_site(&scene, &BTreeMap::new(), &service).unwrap_err();
        assert!(err.contains("no runway your wingmen can land on"), "{err}");
        // Nor is a pad anyone's home.
        let fields = Airfields::from_scene(&scene, None);
        assert!(fields.runways.iter().all(|r| r.view.object != 1004));
    }

    #[test]
    fn player_landing_priority_follows_the_retail_condition() {
        let mut scene = scene();
        let mut service = Service::new(&scene).unwrap();
        let landing = PlayerLanding {
            position: [0., 1500., -20_000.],
            agl_ft: 1400.,
            gear_down: true,
            speed_fps: 300.,
            flying: true,
            on_ground: false,
            track: [0., 300.],
        };
        let at = |scene: &Scene, service: &Service, player| {
            AiWings::landing_priority(scene, service, player, false).0
        };
        assert_eq!(at(&scene, &service, landing), Some(7));
        // Each retail gate on its own releases priority.
        for player in [
            PlayerLanding {
                gear_down: false,
                ..landing
            },
            PlayerLanding {
                agl_ft: PRIORITY_MAX_AGL_FT,
                ..landing
            },
            PlayerLanding {
                speed_fps: PRIORITY_MAX_SPEED_FPS + 1.,
                ..landing
            },
            PlayerLanding {
                flying: false,
                ..landing
            },
            PlayerLanding {
                position: [0., 1500., -(PRIORITY_RANGE_FT + 1.)],
                ..landing
            },
        ] {
            assert_eq!(at(&scene, &service, player), None, "{player:?}");
        }
        // Hostile and unpermitted neutral airports never give priority.
        let hostile = PlayerLanding {
            position: [90_000., 1500., 0.],
            ..landing
        };
        assert_eq!(at(&scene, &service, hostile), None);
        let neutral = PlayerLanding {
            position: [120_000., 1500., 10_000.],
            ..landing
        };
        assert_eq!(at(&scene, &service, neutral), None);
        scene.airports[2].neutral_permission = true;
        assert_eq!(at(&scene, &service, neutral), Some(9));
        // An airport whose runways are all destroyed is skipped.
        service.damage(1000, 1000);
        service.damage(1001, 1000);
        assert_eq!(at(&scene, &service, landing), None);
    }

    /// Regression, found on a real airport (2026-09-23): a player who took
    /// off with the gear still down claimed landing priority, and parked AI
    /// wingmen, which need the runway free, could not follow until the gear
    /// came up. Retail keeps the player in climb-out, which does not block,
    /// until it lines up on an approach.
    #[test]
    fn a_player_climbing_out_gear_down_does_not_claim_landing_priority() {
        let scene = scene();
        let service = Service::new(&scene).unwrap();
        let step =
            |player, departing| AiWings::landing_priority(&scene, &service, player, departing);
        // Parked, gear down on runway 1001 (centre at the origin, 6,000 ft
        // long, heading north): no priority; the runway-free gate covers a
        // rolling player.
        let parked = PlayerLanding {
            position: [0., 8., -2_900.],
            agl_ft: 0.,
            gear_down: true,
            speed_fps: 0.,
            flying: true,
            on_ground: true,
            track: [0., 0.],
        };
        assert_eq!(step(parked, false), (None, true));
        // Just airborne past mid-field, gear down, heading north: climb-out.
        let climbing = PlayerLanding {
            position: [0., 300., 1_000.],
            agl_ft: 300.,
            speed_fps: 250.,
            on_ground: false,
            track: [0., 250.],
            ..parked
        };
        assert_eq!(step(climbing, true), (None, true));
        // Still gear down, turned back and lined up on the south end from
        // 12,000 ft out: priority from here on.
        let aligned = PlayerLanding {
            position: [0., 1_200., 12_000.],
            agl_ft: 1_200.,
            track: [20., -250.],
            ..climbing
        };
        assert_eq!(step(aligned, true), (Some(7), false));
        // Abeam the field, not aligned, but no longer departing: priority.
        let abeam = PlayerLanding {
            position: [8_000., 1_200., 0.],
            track: [0., 250.],
            ..aligned
        };
        assert_eq!(step(abeam, true), (None, true));
        assert_eq!(step(abeam, false), (Some(7), false));
        // Raising the gear ends the climb-out: lowering it later near the
        // field gives priority without lining up again.
        let clean = PlayerLanding {
            gear_down: false,
            ..abeam
        };
        assert_eq!(step(clean, true), (None, false));
    }

    fn target(id: u32, position: [f64; 3], yaw: f64) -> live::Target {
        let basis = Basis::new(yaw, 0., 0.);
        live::Target {
            aircraft: Some(AircraftId::F18),
            role: TargetRole::Aircraft,
            heat: Heat::Engine {
                on: true,
                throttle: 0.7,
                afterburner: false,
            },
            radar_emitting: false,
            id,
            position,
            velocity: basis.forward.map(|v| v * 300.),
            basis,
            configuration: sensors::Configuration::CLEAN,
            signature: sensors::SignatureProfile::default(),
            jammer: None,
            jammer_active: false,
            airborne: true,
            on_ground: false,
            radius: 28.,
            hp: 100,
            initial_hp: 100,
            fragment_offsets: [[0.; 3]; 2],
            wreck: None,
            wreck_power: tore_sim::wreck::Power::default(),
            fragment_released: false,
            localized_damage: live::LocalizedDamage::default(),
            category: 0,
        }
    }

    /// The player's wing with AI wingmen 1 and 2, and an enemy pair.
    fn bridge() -> AiWings {
        let selections =
            [(launch::Side::Friendly, 0u8), (launch::Side::Enemy, 0)].map(|(side, index)| {
                WingSelection {
                    wing: WingId::new(side, index).unwrap(),
                    aircraft: AircraftId::F18,
                    count: 2,
                    skill_level: 1,
                }
            });
        let wings = resolve_wings(&selections, None).unwrap();
        let targets = vec![
            target(1, [0., 20000., 0.], 0.),
            target(2, [1500., 20000., 0.], 0.),
            target(3, [0., 20000., 40000.], std::f64::consts::PI),
            target(4, [1500., 20000., 40000.], std::f64::consts::PI),
        ];
        AiWings::build_with(&wings, &targets, 0, |_| {
            Ok((crate::flight::animation_tests::profile(), None))
        })
        .unwrap()
    }

    fn view(object: u32, x: f64) -> RunwayView {
        RunwayView {
            airport: 7,
            object,
            center: [x, 100., 0.],
            heading: 0.,
            length_ft: 8000.,
            elevation_ft: 100.,
            anchors: None,
        }
    }

    #[test]
    fn bug_out_sends_each_wingman_to_its_own_home_runway() {
        let mut wings = bridge();
        let home = view(500, 10_000.);
        wings
            .mission
            .actor_mut(1)
            .unwrap()
            .set_home_runway(Some(home));
        wings.mission.actor_mut(2).unwrap().set_home_runway(None);
        let report = wings.command(PlayerOrder::BugOut, None, None).unwrap();
        assert_eq!(
            report.message,
            "Bug out: 1 returning to base, 1 with no base"
        );
        assert!(report.radio.is_empty(), "no reviewed bug-out recording");
        let actor = wings.mission.actor(1).unwrap();
        assert!(actor.bugged_out());
        assert_eq!(
            actor.landing_order(),
            Some(&LandingOrder {
                runway: home,
                reason: LandingReason::BugOut,
            })
        );
        assert!(!wings.mission.actor(2).unwrap().bugged_out());

        // The bugged-out wingman no longer answers: it is skipped and named.
        let report = wings.command(PlayerOrder::BugOut, None, Some(1)).unwrap();
        assert_eq!(
            report.message,
            "Wing order unavailable: wingman 1 bugged out and no longer answers"
        );
        let report = wings.command(PlayerOrder::Disengage, None, None).unwrap();
        assert!(
            report.message.ends_with(", 1 bugged out"),
            "{}",
            report.message
        );
        let report = wings
            .command(
                PlayerOrder::Formation(wing::Formation::Echelon),
                None,
                Some(1),
            )
            .unwrap();
        assert!(report.message.contains("bugged out and no longer answers"));
    }

    /// Regression (review, 2026-09-23): bug out was refused on the private
    /// route home, which the simulation and the spec treat as free flight.
    #[test]
    fn bug_out_is_accepted_on_the_route_home() {
        let mut wings = bridge();
        let site = LandingSite {
            name: "Field".into(),
            runway: view(1000, 500_000.),
        };
        wings
            .command_at(PlayerOrder::LandAtSelected, None, Some(2), Some(&site))
            .unwrap();
        wings
            .mission
            .step(&[], &|_, _| 0., tore_sim::ai::threat::TimeOfDay(0))
            .unwrap();
        assert_eq!(
            wings.mission.actor(2).unwrap().airfield_phase(),
            Some(Phase::Inbound)
        );
        wings
            .mission
            .actor_mut(2)
            .unwrap()
            .set_home_runway(Some(view(500, 10_000.)));
        let report = wings.command(PlayerOrder::BugOut, None, Some(2)).unwrap();
        assert_eq!(report.message, "Bug out: 1 returning to base");
        assert!(wings.mission.actor(2).unwrap().bugged_out());
    }

    #[test]
    fn land_at_selected_uses_the_player_chosen_runway() {
        let mut wings = bridge();
        let report = wings
            .command(PlayerOrder::LandAtSelected, None, Some(2))
            .unwrap();
        assert!(report.message.contains("Shift-A"), "{}", report.message);
        assert!(wings.mission.actor(2).unwrap().landing_order().is_none());

        let site = LandingSite {
            name: "Field".into(),
            runway: view(1000, 50_000.),
        };
        let report = wings
            .command_at(PlayerOrder::LandAtSelected, None, Some(2), Some(&site))
            .unwrap();
        assert_eq!(report.message, "Land at Field: 1 landing");
        assert_eq!(
            wings.mission.actor(2).unwrap().landing_order(),
            Some(&LandingOrder {
                runway: site.runway,
                reason: LandingReason::Ordered,
            })
        );
        assert!(
            wings.mission.actor(1).unwrap().landing_order().is_none(),
            "only the addressed wingman lands"
        );
    }
}
