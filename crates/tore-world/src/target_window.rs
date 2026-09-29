//! Read-only target-window data: what the window says about the selected
//! target, built from simulation state with no clock, camera or drawing. The
//! refresh timer and the target camera are in `target_preview`. See
//! docs/spec/target-window.md.
use crate::ai_wings::AiWings;
use tore_sim::combat::live::Target;
use tore_sim::flight::State;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetObjective {
    Survive,
    Destroy,
}

pub struct Readout {
    pub id: u32,
    pub name: String,
    pub damage: f64,
    pub bearing: String,
    pub metric: String,
    pub objective: Option<TargetObjective>,
    pub activity: String,
    pub goal: &'static str,
    pub player_goal: bool,
    pub skill: Option<u8>,
}
impl Readout {
    pub fn new(target: &Target, player: &State, name: String) -> Self {
        let offset: [f64; 3] = std::array::from_fn(|i| target.position[i] - player.position[i]);
        let bearing = clock_bearing(offset[0].atan2(offset[2]) - player.yaw);
        let altitude = elevation_label(offset);
        let norm = |v: [f64; 3]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
        Self {
            id: target.id,
            name,
            damage: damage_fraction(target.hp, target.initial_hp),
            bearing: format!("{bearing}:00{altitude}"),
            metric: metric(player.ticks, norm(offset), norm(target.velocity)),
            objective: None,
            activity: String::new(),
            goal: "?",
            player_goal: false,
            skill: None,
        }
    }
    pub fn with_activity(&mut self, wings: &AiWings, viewer: u32) {
        self.objective = wings.target_objective(viewer, self.id);
        let Some(actor) = wings.mission().actor(self.id) else {
            return;
        };
        if actor.is_dummy() {
            self.activity = "DUMMY 400 KTS".into();
            self.goal = "N";
            self.skill = None;
            self.player_goal = false;
            return;
        }
        self.activity = actor.activity().label().to_ascii_uppercase();
        self.skill = Some(actor.controller().experience().level.level());
        (self.goal, self.player_goal) =
            activity_goal(actor.activity(), actor.controller().target());
    }
}
fn activity_goal(
    activity: tore_sim::ai::controller::Activity,
    target: Option<u32>,
) -> (&'static str, bool) {
    use tore_sim::ai::controller::Activity;
    let goal = match activity {
        Activity::Pursuing | Activity::Attacking => "A",
        Activity::Defending | Activity::Evading | Activity::Breaking => "E",
        Activity::Destroyed => "C",
        // Manual p.101: T takeoff, L land. The ground wait and taxi belong to
        // the takeoff sequence. Agent decision (2026-09-23): a landed aircraft
        // keeps L, because rollout and parking end the same landing sequence.
        Activity::Waiting | Activity::Taxiing | Activity::TakingOff => "T",
        Activity::HoldingMarshal | Activity::Landing | Activity::Landed => "L",
        Activity::Idle
        | Activity::Formation
        | Activity::Searching
        | Activity::Acquiring
        | Activity::Rejoining
        | Activity::ReturningToBase
        | Activity::OutOfFuel => "N",
    };
    (
        goal,
        goal == "A" && target == Some(crate::ai_wings::PLAYER_ID),
    )
}
/// User-requested ten-degree threshold from the world horizontal plane.
/// Comparing rise against run * tan(10 degrees) also handles overhead targets
/// and the coincident-position case without dividing by zero.
fn elevation_label(offset: [f64; 3]) -> &'static str {
    let limit = offset[0].hypot(offset[2]) * 10_f64.to_radians().tan();
    if offset[1] > limit {
        " HI"
    } else if offset[1] < -limit {
        " LO"
    } else {
        ""
    }
}
fn clock_bearing(relative: f64) -> u8 {
    let hour = (relative.rem_euclid(std::f64::consts::TAU) / (std::f64::consts::PI / 6.)).round()
        as u8
        % 12;
    if hour == 0 { 12 } else { hour }
}
fn damage_fraction(hp: i32, initial: i32) -> f64 {
    (1. - f64::from(hp) / f64::from(initial.max(1))).clamp(0., 1.)
}
fn metric(ticks: u64, distance_ft: f64, speed_fps: f64) -> String {
    let feet_per_nm = tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
    if (ticks / 360).is_multiple_of(2) {
        format!("{:.1} NM", distance_ft / feet_per_nm)
    } else {
        format!("{:.0} KTS", speed_fps * 3600. / feet_per_nm)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hi_lo_uses_strict_ten_degree_elevation_at_any_range() {
        for horizontal in [100., 1000., 10000.] {
            for (degrees, label) in [
                (-10.001_f64, " LO"),
                (-10., ""),
                (-9.999, ""),
                (0., ""),
                (9.999, ""),
                (10., ""),
                (10.001, " HI"),
            ] {
                let height = horizontal * degrees.to_radians().tan();
                assert_eq!(elevation_label([horizontal, height, 0.]), label);
                assert_eq!(elevation_label([0., height, -horizontal]), label);
            }
        }
        // Equal height differences have different labels at different ranges.
        assert_eq!(elevation_label([100., 100., 0.]), " HI");
        assert_eq!(elevation_label([1000., 100., 0.]), "");
        assert_eq!(elevation_label([10000., 600., 0.]), "");
        assert_eq!(elevation_label([0., 100., 0.]), " HI");
        assert_eq!(elevation_label([0., -100., 0.]), " LO");
        assert_eq!(elevation_label([0.; 3]), "");
    }

    #[test]
    fn goals_only_underline_confirmed_player_attacks() {
        use tore_sim::ai::controller::Activity;
        assert_eq!(activity_goal(Activity::Attacking, Some(0)), ("A", true));
        assert_eq!(activity_goal(Activity::Attacking, Some(7)), ("A", false));
        assert_eq!(activity_goal(Activity::Pursuing, None), ("A", false));
        // Selected attack target does not identify the threat being evaded.
        assert_eq!(activity_goal(Activity::Evading, Some(0)), ("E", false));
        assert_eq!(activity_goal(Activity::Searching, Some(0)), ("N", false));
        assert_eq!(activity_goal(Activity::Acquiring, Some(0)), ("N", false));
        assert_eq!(activity_goal(Activity::Rejoining, Some(0)), ("N", false));
        assert_eq!(activity_goal(Activity::ReturningToBase, None), ("N", false));
        assert_eq!(activity_goal(Activity::Destroyed, None), ("C", false));
    }
    #[test]
    fn airfield_activities_use_the_manual_takeoff_and_land_codes() {
        use tore_sim::ai::controller::Activity;
        for activity in [Activity::Waiting, Activity::Taxiing, Activity::TakingOff] {
            assert_eq!(activity_goal(activity, Some(0)), ("T", false));
        }
        for activity in [
            Activity::HoldingMarshal,
            Activity::Landing,
            Activity::Landed,
        ] {
            assert_eq!(activity_goal(activity, Some(0)), ("L", false));
        }
        // Returning to base is a withdrawal, not yet a landing phase.
        assert_eq!(activity_goal(Activity::ReturningToBase, None), ("N", false));
    }
    #[test]
    fn clock_wrap_and_cardinal_bearings() {
        for (degrees, hour) in [
            (0., 12),
            (90., 3),
            (180., 6),
            (-90., 9),
            (359., 12),
            (14., 12),
            (16., 1),
        ] {
            assert_eq!(clock_bearing(f64::to_radians(degrees)), hour);
        }
    }
    #[test]
    fn cycle_changes_at_exact_three_second_boundaries() {
        let nm = tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
        for tick in [0, 359, 720] {
            assert_eq!(metric(tick, nm * 6.2, nm * 254. / 3600.), "6.2 NM");
        }
        for tick in [360, 719] {
            assert_eq!(metric(tick, nm * 6.2, nm * 254. / 3600.), "254 KTS");
        }
    }
    #[test]
    fn damage_endpoints_and_clamping() {
        for (hp, expected) in [(100, 0.), (75, 0.25), (0, 1.), (-1, 1.), (101, 0.)] {
            assert_eq!(damage_fraction(hp, 100), expected);
        }
    }
}
