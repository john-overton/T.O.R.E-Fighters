//! Player delivery uses the same B46 receiver as AI team requests.
use super::*;
use tore_sim::ai::{
    engagement::{Assignment, Role, Stance},
    wing::{
        self, PlayerOrder, ReceiverOutcome, SpacingAxis, TargetId, TargetOrder, WingControl,
        WingRequest,
    },
};

pub struct OrderReport {
    pub message: String,
    pub radio: Vec<&'static str>,
}

impl AiWings {
    pub fn command(
        &mut self,
        order: PlayerOrder,
        selected: Option<u32>,
        recipient: Option<u8>,
    ) -> AppResult<OrderReport> {
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
            return Ok(OrderReport {
                message: "Wing order unavailable: no addressed wingmen".into(),
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
        let mut reply = None;
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
                    if Some(*id) == first {
                        reply = match order {
                            PlayerOrder::EngageMyTarget => Some("^ENGAGE"),
                            PlayerOrder::ProtectMe => Some("^SHWTIME"),
                            _ => None,
                        };
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
        if let Some(stem) = reply {
            radio.push(stem);
        }
        Ok(OrderReport {
            message: format!(
                "{}: {applied} applied, {rejected} rejected, {no_motion} without motion",
                order_label(order)
            ),
            radio,
        })
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
        | PlayerOrder::ControlToggle => None,
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
        PlayerOrder::EngageMyTarget => "^ATTACK",
        // Sender mapping for policy orders is not yet established.
        PlayerOrder::AttackOnContact | PlayerOrder::EngageFromFormation => return None,
    })
}

fn order_label(order: PlayerOrder) -> &'static str {
    use wing::{Formation as F, PlayerApproach as A, PlayerBreak as B};
    match order {
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
