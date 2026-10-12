use super::*;

fn own(heading: f64) -> Own {
    Own {
        position: [0., 1_000., 0.],
        heading_deg: heading,
        speed_ft_s: 230.,
        bank_deg: 0.,
        maximum_ft_s: 250.,
    }
}

fn threat(bearing: Option<f64>) -> Threat {
    Threat {
        bearing_deg: bearing,
        viewpoint: None,
        tie: 2,
    }
}

fn flat(_x: f64, _z: f64) -> f64 {
    0.
}

fn off(a: f64, b: f64) -> f64 {
    delta(a, b).abs()
}

#[test]
fn the_break_is_ninety_degrees_off_the_threats_bearing_on_the_nearer_side() {
    // A threat 30 degrees right of the nose: beams are 120 and 300; the nose
    // is on 0, so left (300) is 60 degrees of turn and right 120.
    let b = decide(own(0.), threat(Some(30.)), flat);
    assert!((b.heading_deg - 300.).abs() < 1e-9, "{b:?}");
    assert_eq!(b.offset_deg, 90.);
    // The mirror threat breaks the other way.
    let b = decide(own(0.), threat(Some(330.)), flat);
    assert!((b.heading_deg - 60.).abs() < 1e-9, "{b:?}");
}

#[test]
fn a_threat_dead_ahead_or_astern_breaks_by_the_tie_rule_every_time() {
    for bearing in [0., 180.] {
        let even = decide(own(0.), threat(Some(bearing)), flat);
        let again = decide(own(0.), threat(Some(bearing)), flat);
        assert_eq!(even, again);
        let mut odd_threat = threat(Some(bearing));
        odd_threat.tie = 3;
        let odd = decide(own(0.), odd_threat, flat);
        assert!((off(even.heading_deg, odd.heading_deg) - 180.).abs() < 1e-9);
        assert_eq!(off(even.heading_deg, bearing), 90.);
    }
}

#[test]
fn with_no_bearing_it_is_retails_plus_or_minus_ninety_from_the_current_heading() {
    let b = decide(own(40.), threat(None), flat);
    assert!(
        (off(b.heading_deg, 40.) - 90.).abs() < 1e-9,
        "{:.0}",
        b.heading_deg
    );
    assert_eq!(b.offset_deg, 90.);
}

#[test]
fn it_neither_dives_nor_zooms_and_comes_down_to_the_masking_height() {
    // 1,000 ft over flat ground, 200 ft wanted: down at the 10 degree limit.
    let b = decide(own(0.), threat(Some(90.)), flat);
    assert_eq!(b.flight_path_pitch_deg, -PITCH_LIMIT_DEG);
    // At the masking height it holds it.
    let mut low = own(0.);
    low.position[1] = MASK_AGL_FT;
    let b = decide(low, threat(Some(90.)), flat);
    assert_eq!(b.flight_path_pitch_deg, 0.);
    // Below it, it climbs back, but never steeper than the limit.
    low.position[1] = 0.;
    let b = decide(low, threat(Some(90.)), |_, _| 100.);
    assert!(b.flight_path_pitch_deg > 0. && b.flight_path_pitch_deg <= PITCH_LIMIT_DEG);
}

#[test]
fn it_does_not_come_down_faster_than_it_can_hold() {
    // Past its maximum speed the descent stops and it flies level (or up).
    let mut fast = own(0.);
    fast.speed_ft_s = 260.;
    let b = decide(fast, threat(Some(90.)), flat);
    assert_eq!(b.flight_path_pitch_deg, 0.);
    let mut at_max = own(0.);
    at_max.speed_ft_s = 250.;
    let b = decide(at_max, threat(Some(90.)), flat);
    assert_eq!(b.flight_path_pitch_deg, -PITCH_LIMIT_DEG);
}

#[test]
fn it_climbs_ahead_of_rising_ground_and_prefers_a_break_that_is_not_into_a_wall() {
    // A wall of high ground west of the helicopter, threat to the north:
    // the beams are east and west; west ends in the wall.
    let wall = |x: f64, _z: f64| if x < -2_000. { 5_000. } else { 0. };
    let mut helicopter = own(0.);
    helicopter.position[1] = 600.;
    let b = decide(helicopter, threat(Some(0.)), wall);
    assert!(b.heading_deg > 0. && b.heading_deg < 180., "{b:?}");
    assert_eq!(b.blocked_s, 0);
    // With a wall on both sides it says so and climbs at the limit.
    let boxed = |x: f64, _z: f64| if x.abs() > 600. { 5_000. } else { 0. };
    let b = decide(helicopter, threat(Some(0.)), boxed);
    assert!(b.blocked_s > 0, "{b:?}");
    assert_eq!(b.flight_path_pitch_deg, PITCH_LIMIT_DEG);
}

#[test]
fn it_takes_the_break_that_puts_terrain_between_it_and_the_site() {
    // The site is far north. A ridge 1,500 ft high stands north of the
    // helicopter on its east side only (x > 500): a break east is hidden
    // from the site, a break west is not. Both beams are 90 degrees of turn,
    // so only the mask can decide it, whichever side the tie rule would take.
    let ridge = |x: f64, z: f64| {
        if x > 500. && (3_000. ..4_000.).contains(&z) {
            1_500.
        } else {
            0.
        }
    };
    for tie in [2, 3] {
        let mut seen = threat(Some(0.));
        seen.viewpoint = Some([1_500., 0., 20_000.]);
        seen.tie = tie;
        let mut helicopter = own(0.);
        helicopter.position[1] = 800.;
        let b = decide(helicopter, seen, ridge);
        assert!(
            b.heading_deg > 0. && b.heading_deg < 180.,
            "tie {tie}: {b:?}"
        );
        assert!(b.masked_s > 0, "tie {tie}: {b:?}");
    }
}

#[test]
fn nothing_masked_means_the_plain_beam() {
    let mut seen = threat(Some(0.));
    seen.viewpoint = Some([0., 0., 15_000.]);
    let b = decide(own(0.), seen, flat);
    assert_eq!(b.masked_s, 0);
    assert_eq!(b.offset_deg, 90.);
}

#[test]
fn a_turn_begun_is_kept_when_the_threat_swaps() {
    // A threat dead ahead has two equal beams, 90 degrees either way; the tie
    // rule (even) would take the right one. Banked left, the helicopter
    // keeps turning left; banked right it turns right.
    let mut left = own(0.);
    left.bank_deg = -25.;
    let b = decide(left, threat(Some(0.)), flat);
    assert!((b.heading_deg - 270.).abs() < 1e-9, "{b:?}");
    let mut right = own(0.);
    right.bank_deg = 25.;
    let b = decide(right, threat(Some(0.)), flat);
    assert!((b.heading_deg - 90.).abs() < 1e-9, "{b:?}");
    // A shallow bank is not a commitment.
    let mut level = own(0.);
    level.bank_deg = -5.;
    let b = decide(level, threat(Some(0.)), flat);
    assert!((b.heading_deg - 90.).abs() < 1e-9, "{b:?}");
}

#[test]
fn the_search_is_deterministic() {
    let hills = |x: f64, z: f64| (x * 0.01).sin() * 50. + z * 0.01;
    let a = decide(own(10.), threat(Some(77.)), hills);
    let b = decide(own(10.), threat(Some(77.)), hills);
    assert_eq!(a, b);
}
