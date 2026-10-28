//! A helicopter's threat awareness (docs/spec/ai-strike.md, "Helicopter
//! threat awareness"): rings, the clear line, the attack plan, the hold
//! point and its terrain masking, the ambush ring. Synthetic geometry only.
use super::*;
use crate::ai::awareness::ground::DefenseFacts;
use crate::ai::targeting::Side;

fn defense(id: u32, at: Vector, missile: f64, gun: f64, working: bool) -> KnownDefense {
    KnownDefense {
        id,
        side: Side(2),
        position: at,
        facts: DefenseFacts {
            missile_reach_ft: missile,
            gun_reach_ft: gun,
            radar: true,
            working,
            ship: false,
            heading_deg: 0.,
        },
        briefed: false,
    }
}

fn ring(id: u32, at: Vector, radius: f64) -> Ring {
    Ring {
        id,
        centre: at,
        radius_ft: radius,
    }
}

#[test]
fn a_ring_is_the_reach_that_can_hurt_plus_the_margin() {
    let defenses = [
        defense(1, [0.; 3], 20_000., 3_000., true),
        // A launcher whose battery radar is dead keeps only its guns.
        defense(2, [0.; 3], 20_000., 3_000., false),
        // A lone search radar reaches nothing.
        defense(3, [0.; 3], 0., 0., true),
    ];
    let rings = rings(defenses.iter());
    assert_eq!(rings.len(), 2);
    assert_eq!(rings[0].radius_ft, 20_000. + RING_MARGIN_FT);
    assert_eq!(rings[1].radius_ft, 3_000. + RING_MARGIN_FT);
}

#[test]
fn a_line_is_clear_outside_every_ring_and_may_always_lead_out_of_one() {
    let rings = [ring(1, [0., 0., 10_000.], 3_000.)];
    // Straight through the ring: blocked; beside it: clear.
    assert!(!clear(&rings, [0.; 3], [0., 0., 20_000.]));
    assert!(clear(&rings, [5_000., 0., 0.], [5_000., 0., 20_000.]));
    // From inside, outward: clear; deeper in: blocked.
    assert!(clear(&rings, [0., 0., 9_000.], [0., 0., 0.]));
    assert!(!clear(&rings, [0., 0., 8_000.], [0., 0., 10_500.]));
}

#[test]
fn a_line_from_inside_a_ring_must_lead_straight_out_not_skirt_deeper_first() {
    // The Ukraine column's SA-14 of surface seed 16 in miniature: the
    // helicopter starts inside the ring and the target lies outside it, but
    // the line to the target passes nearer the site before it leaves.
    let rings = [ring(1, [0.; 3], 15_500.)];
    let own = [12_167., 0., 0.];
    let skirting = [-2_000., 0., 24_000.];
    assert!(horizontal(rings[0].centre, skirting) > 15_500.);
    assert!(!clear(&rings, own, skirting));
    // Away from the site it is clear, and a pop-up across it is not planned.
    assert!(clear(&rings, own, [30_000., 0., 4_000.]));
    assert_eq!(plan_attack(own, skirting, None, false, &rings), None);
}

#[test]
fn a_ring_learned_after_the_choice_blocks_the_pass_in_hand() {
    let own = [0., 1_500., 0.];
    let target = [0., 0., 24_000.];
    let at2 = Some((18_000., 10_000.));
    assert!(holds(Plan::PopUp, own, target, &[]));
    // A gun 2,650 ft beyond the target: its ring covers the target, so the
    // pop-up in hand no longer holds, though a stand-off from outside the
    // ring still stands (the old test kept the pop-up for that reason).
    let gun = [ring(1, [0., 0., 26_650.], 9_500.)];
    assert!(!holds(Plan::PopUp, own, target, &gun));
    let plan = plan_attack(own, target, at2, false, &gun);
    assert!(matches!(plan, Some(Plan::StandOff { .. })), "{plan:?}");
    // The plan chosen with the ring known holds while it is flown.
    assert!(holds(plan.unwrap(), own, target, &gun));
    // A straight stand-off whose carry-on would end in a new ring does not.
    let straight = Plan::StandOff {
        approach: None,
        range_ft: 10_800.,
    };
    assert!(holds(straight, own, target, &[]));
    let near = [ring(2, [0., 0., 16_000.], 3_000.)];
    assert!(!holds(straight, own, target, &near));
}

#[test]
fn the_plan_is_the_pop_up_when_clear_else_a_stand_off_from_outside_the_rings() {
    let own = [0., 1_500., 0.];
    let target = [0., 0., 24_000.];
    let at2 = Some((18_000., 10_000.));
    // Nothing around: the pop-up, or the straight stand-off when asked.
    assert_eq!(plan_attack(own, target, at2, false, &[]), Some(Plan::PopUp));
    assert_eq!(
        plan_attack(own, target, at2, true, &[]),
        Some(Plan::StandOff {
            approach: None,
            range_ft: 0.6 * 18_000.
        })
    );
    // A gun ring on the target: no pop-up and no rockets, but the AT-2
    // reaches from outside it.
    let gun = [ring(1, [0., 0., 24_500.], 7_000.)];
    assert_eq!(plan_attack(own, target, None, false, &gun), None);
    assert!(matches!(
        plan_attack(own, target, at2, false, &gun),
        Some(Plan::StandOff { .. })
    ));
    // A ring across the straight line: the firing point slides round the
    // target, off the line, and the way to it is clear.
    let across = [ring(2, [0., 0., 15_500.], 4_000.)];
    match plan_attack(own, target, at2, true, &across) {
        Some(Plan::StandOff {
            approach: Some(point),
            range_ft,
        }) => {
            assert!(inside(&across, point).is_none());
            assert!(clear(&across, own, point));
            assert!((horizontal(point, target) - range_ft).abs() < 1.);
        }
        other => panic!("{other:?}"),
    }
    // A SAM that out-ranges the AT-2 covering everything: no attack.
    let sam = [ring(3, [3_000., 0., 24_000.], 32_000.)];
    assert_eq!(plan_attack(own, target, at2, true, &sam), None);
}

#[test]
fn the_hold_point_leaves_room_for_its_circle_outside_every_ring() {
    let rings = [
        ring(1, [0., 0., 24_000.], 20_000.),
        ring(2, [6_000., 0., 4_000.], 3_000.),
    ];
    let own = [0., 1_500., 6_000.];
    let centre = hold_centre(own, &rings, None);
    for r in &rings {
        assert!(
            horizontal(r.centre, centre) >= r.radius_ft + HOLD_RADIUS_FT - 1.,
            "{centre:?} against {r:?}"
        );
    }
    // Already clear: it holds where it is.
    let far = [0., 1_500., -20_000.];
    assert_eq!(hold_centre(far, &rings, None), far);
}

#[test]
fn the_hold_point_slides_round_the_site_behind_a_ridge() {
    // A ridge east of the site's south line: the point due south of the site
    // is in the open, the point 30 degrees round toward the east (or 60)
    // lies behind the ridge.
    let site = [0., 0., 0.];
    let rings = [ring(1, site, 10_000.)];
    let ground = |x: f64, z: f64| {
        let bearing = x.atan2(z).to_degrees().rem_euclid(360.);
        let distance = x.hypot(z);
        if (100.0..170.).contains(&bearing) && (8_000.0..9_000.).contains(&distance) {
            3_000.
        } else {
            0.
        }
    };
    let own = [0., 1_500., -9_000.];
    let open = hold_centre(own, &rings, None);
    assert!(!masked(site, open, &ground));
    let hidden = hold_centre(own, &rings, Some(&ground));
    assert!(masked(site, hidden, &ground), "{hidden:?}");
    assert!(horizontal(site, hidden) >= 10_000. + HOLD_RADIUS_FT - 1.);
}

#[test]
fn an_ambush_ring_lies_toward_the_threat_and_is_deterministic() {
    let own = [1_000., 500., 2_000.];
    let east = ambush_ring(own, Some(90.));
    assert_eq!(east.id, AMBUSH_ID);
    assert!((east.centre[0] - (1_000. + AMBUSH_OFFSET_FT)).abs() < 1e-6);
    assert!((east.centre[2] - 2_000.).abs() < 1e-6);
    assert_eq!(east.radius_ft, AMBUSH_RADIUS_FT);
    assert_eq!(ambush_ring(own, None).centre, own);
    assert_eq!(ambush_ring(own, Some(90.)), east);
}

#[test]
fn the_decisions_have_fixed_reasons_and_words() {
    for (decision, reason) in [
        (Decision::HoldOff, "threat_hold_off"),
        (Decision::Resume, "threat_resume"),
        (Decision::Retarget, "threat_retarget"),
        (Decision::Home, "threat_home"),
        (Decision::Leave, "threat_leave"),
    ] {
        assert_eq!(decision.reason(), reason);
        assert!(!decision.words().is_empty());
    }
}
