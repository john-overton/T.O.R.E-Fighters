//! The coders of the AI's wing state (B43, B46, docs/formats/checkpoint.md):
//! the recipient state a controller keeps for wing orders, the formation
//! variation it draws offsets with, and the wing requests and target orders
//! held in a controller's recipient state or its last intent batch.
//!
//! `Formation` and `WingControl` are leaf types coded in
//! `checkpoint_shared.rs`. The module's other types (outcomes, sender state,
//! player orders, settings applied) are produced and consumed inside one call.

use super::{
    ClassPolicy, FormationVariation, RecipientState, SpacingAxis, TargetId, TargetOrder,
    WingRequest,
};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

crate::checkpoint_tuple!(TargetId(id));
crate::checkpoint_tuple!(ClassPolicy(policy));

crate::checkpoint_enum!(SpacingAxis {
    Horizontal = 0,
    Vertical = 1,
});

impl Checkpoint for TargetOrder {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::HoldFire => s.writer().write_varint(0),
            Self::FreeSelection => s.writer().write_varint(1),
            Self::ClassPolicy(policy) => {
                s.writer().write_varint(2);
                policy.save(s, None)?;
            }
            Self::ConcreteTarget(target) => {
                s.writer().write_varint(3);
                target.save(s, None)?;
            }
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::HoldFire,
            1 => Self::FreeSelection,
            2 => Self::ClassPolicy(Checkpoint::load(l, None)?),
            3 => Self::ConcreteTarget(Checkpoint::load(l, None)?),
            other => return invalid(format!("TargetOrder has no variant {other}")),
        })
    }
}

impl Checkpoint for WingRequest {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::Break {
                heading_offset_deg,
                pitch_deg,
            } => {
                s.writer().write_varint(0);
                heading_offset_deg.save(s, None)?;
                pitch_deg.save(s, None)?;
            }
            Self::Approach {
                heading_deg,
                pitch_deg,
                speed,
            } => {
                s.writer().write_varint(1);
                heading_deg.save(s, None)?;
                pitch_deg.save(s, None)?;
                speed.save(s, None)?;
            }
            Self::Spacing { axis, feet } => {
                s.writer().write_varint(2);
                axis.save(s, None)?;
                feet.save(s, None)?;
            }
            Self::FormationSelection(formation) => {
                s.writer().write_varint(3);
                formation.save(s, None)?;
            }
            Self::WingControl(control) => {
                s.writer().write_varint(4);
                control.save(s, None)?;
            }
            Self::TargetAssignment(order) => {
                s.writer().write_varint(5);
                order.save(s, None)?;
            }
            Self::Land(order) => {
                s.writer().write_varint(6);
                order.save(s, None)?;
            }
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::Break {
                heading_offset_deg: Checkpoint::load(l, None)?,
                pitch_deg: Checkpoint::load(l, None)?,
            },
            1 => Self::Approach {
                heading_deg: Checkpoint::load(l, None)?,
                pitch_deg: Checkpoint::load(l, None)?,
                speed: Checkpoint::load(l, None)?,
            },
            2 => Self::Spacing {
                axis: Checkpoint::load(l, None)?,
                feet: Checkpoint::load(l, None)?,
            },
            3 => Self::FormationSelection(Checkpoint::load(l, None)?),
            4 => Self::WingControl(Checkpoint::load(l, None)?),
            5 => Self::TargetAssignment(Checkpoint::load(l, None)?),
            6 => Self::Land(Checkpoint::load(l, None)?),
            other => return invalid(format!("WingRequest has no variant {other}")),
        })
    }
}

// The offset and the next draw's deadline decide the formation point and
// when the controller's random stream is drawn from again.
crate::checkpoint_struct!(FormationVariation {
    offset_ft,
    next_deadline,
});

crate::checkpoint_struct!(RecipientState {
    human_controlled,
    maneuver_state,
    target,
    body_heading_deg,
    speed_limits,
    active_command,
    formation,
    wing_control,
    horizontal_spacing_ft,
    vertical_spacing_ft,
    target_order,
    target_deadline,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ScalarSpeed;
    use crate::ai::airfield::LandingOrder;
    use crate::ai::wing::{Formation, WingControl};
    use crate::checkpoint::{Models, round_trip};

    fn same<T: Checkpoint + PartialEq + std::fmt::Debug>(value: T) {
        let copy = round_trip(&value, &Models::default()).unwrap();
        assert_eq!(copy, value);
    }

    #[test]
    fn every_wing_request_and_order_round_trips() {
        for order in [
            TargetOrder::HoldFire,
            TargetOrder::FreeSelection,
            TargetOrder::ClassPolicy(ClassPolicy(7)),
            TargetOrder::ConcreteTarget(TargetId(412)),
        ] {
            same(order);
        }
        same(SpacingAxis::Horizontal);
        same(SpacingAxis::Vertical);
        let runway = crate::ai::airfield::RunwayView {
            airport: 7,
            object: 70,
            center: [1., 2., 3.],
            heading: 90.,
            length_ft: 8_000.,
            elevation_ft: 120.,
            anchors: None,
        };
        for request in [
            WingRequest::Break {
                heading_offset_deg: -90,
                pitch_deg: 10,
            },
            WingRequest::Approach {
                heading_deg: 45,
                pitch_deg: 5,
                speed: ScalarSpeed(0.),
            },
            WingRequest::Spacing {
                axis: SpacingAxis::Vertical,
                feet: -512,
            },
            WingRequest::FormationSelection(Formation::LineAstern),
            WingRequest::WingControl(WingControl::Tight),
            WingRequest::TargetAssignment(TargetOrder::HoldFire),
            WingRequest::Land(LandingOrder {
                runway,
                reason: crate::ai::airfield::LandingReason::BugOut,
            }),
        ] {
            same(request);
        }
    }

    #[test]
    fn a_formation_variation_keeps_its_offset_and_deadline() {
        let mut variation = FormationVariation::new(10);
        let mut random = crate::ai::DecisionRandom::seeded(5);
        let mut tick = 0;
        while !variation.advance(tick, &mut random) {
            tick += 30;
        }
        same(variation);
    }
}
