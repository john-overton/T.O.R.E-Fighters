//! Routes, fuel and recovery (B48), from
//! [`docs/spec/ai.md`](../../../../docs/spec/ai.md).
//!
//! This component decides when a waypoint is complete, what a route command
//! asks for, whether a wingman joins its leader's landing, the fuel state, and
//! the private bingo route home. It computes nothing from the flight model:
//! endurance and time-to-home are inputs, the throttle search that produces
//! them belongs to the host. The octant geometry behind "waypoint behind the
//! aircraft" is provided as helpers, but the caller decides which vector and
//! which stored octant to compare.
//!
//! Fitted choices (agent decisions, 2026-09-17), each where the spec is silent:
//!
//! - Octant sectors start on the +x axis and advance toward +z in 45-degree
//!   steps; the spec only requires equal-or-neighbouring comparison.
//! - The leader's altitude variation is a whole-foot draw in -100..=100. The
//!   original's draw has 1/256 ft resolution; the difference is host precision.
//! - The altitude variation is applied before the landing minimum, so a
//!   landing waypoint is never commanded below 2000 ft after variation.
//! - The caution band is inclusive at time-to-home plus ten minutes (`<=`),
//!   the bingo band exclusive (`<`); the spec says "under" for both.

use super::{AiError, DecisionRandom, Result, ScalarSpeed, SpeedLimits};

/// B48: a landing waypoint within this distance hands off to the airport.
pub const LANDING_HANDOFF_FT: f64 = 60000.0;
/// B48: a wing leader varies commanded altitude only beyond this distance.
pub const ALTITUDE_VARIATION_MIN_DISTANCE_FT: f64 = 5000.0;
/// B48: altitude varies by up to this many feet either way.
pub const ALTITUDE_VARIATION_FT: i32 = 100;
/// B48: landing waypoints are never commanded below this altitude.
pub const LANDING_MIN_ALTITUDE_FT: f64 = 2000.0;
/// B48: each route command lasts a nominal five seconds.
pub const ROUTE_COMMAND_SECONDS: u32 = 5;
/// B48: join-landing distance to the leader.
pub const JOIN_LANDING_LEADER_FT: f64 = 10000.0;
/// B48: join-landing distance to the leader's airport.
pub const JOIN_LANDING_AIRPORT_FT: f64 = 40000.0;
/// B48: the one-fifth cruise rule applies at or above this speed.
pub const CRUISE_FIFTH_MIN_SPEED: ScalarSpeed = ScalarSpeed(75.0);
/// B48: critical below four minutes of endurance.
pub const CRITICAL_ENDURANCE_S: f64 = 240.0;
/// B48: bingo under time-to-home plus five minutes.
pub const BINGO_MARGIN_S: f64 = 300.0;
/// B48: caution under time-to-home plus ten minutes.
pub const CAUTION_MARGIN_S: f64 = 600.0;
/// B48: the private bingo route flies at 5000 to 10000 ft.
pub const BINGO_ROUTE_MIN_ALTITUDE_FT: u32 = 5000;
pub const BINGO_ROUTE_ALTITUDE_SPAN_FT: u32 = 5000;

/// One of eight 45-degree sectors, 0..=7.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Octant(u8);
impl Octant {
    pub fn new(sector: u8) -> Result<Self> {
        if sector < 8 {
            Ok(Self(sector))
        } else {
            Err(AiError::InvalidInput("octant outside 0..8"))
        }
    }
    pub fn sector(self) -> u8 {
        self.0
    }
    /// The sector 180 degrees away.
    pub fn opposite(self) -> Self {
        Self((self.0 + 4) % 8)
    }
}

/// Sector of a horizontal vector: sector 0 starts on the +x axis, sectors
/// advance toward +z every 45 degrees.
pub fn octant(dx: f64, dz: f64) -> Result<Octant> {
    if !(dx.is_finite() && dz.is_finite()) || (dx == 0.0 && dz == 0.0) {
        return Err(AiError::InvalidInput(
            "octant of a zero or non-finite vector",
        ));
    }
    let degrees = dz.atan2(dx).to_degrees().rem_euclid(360.0);
    Ok(Octant(((degrees / 45.0).floor() as u8) % 8))
}

/// B48: the waypoint is behind when the current sector equals the stored
/// opposite sector or one of its neighbours.
pub fn waypoint_behind(current: Octant, stored_opposite: Octant) -> bool {
    matches!((current.0 + 8 - stored_opposite.0) % 8, 0 | 1 | 7)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaypointKind {
    Ordinary,
    GoalObject {
        goal_destroyed: bool,
        /// Any member of the wing still has a usable weapon for the goal class.
        wing_has_usable_weapon: bool,
    },
    Ground {
        on_ground: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaypointCompletion {
    /// The caller's octant test, see [`waypoint_behind`].
    pub behind: bool,
    pub kind: WaypointKind,
}

/// B48: a waypoint completes when it is behind the aircraft, with the extra
/// goal-object and ground conditions.
pub fn waypoint_complete(inputs: &WaypointCompletion) -> bool {
    inputs.behind
        && match inputs.kind {
            WaypointKind::Ordinary => true,
            WaypointKind::GoalObject {
                goal_destroyed,
                wing_has_usable_weapon,
            } => goal_destroyed || !wing_has_usable_weapon,
            WaypointKind::Ground { on_ground } => on_ground,
        }
}

/// The current waypoint as the route command sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Waypoint {
    pub altitude_ft: f64,
    pub speed: ScalarSpeed,
    pub landing: bool,
    /// Horizontal distance from the aircraft.
    pub distance_ft: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouteInputs {
    /// `None` when the aircraft has no route.
    pub waypoint: Option<Waypoint>,
    pub has_airport: bool,
    pub wing_leader: bool,
    pub limits: SpeedLimits,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RouteCommand {
    /// No route: hold the current heading.
    HoldHeading { duration_seconds: u32 },
    /// Hand the aircraft to the airport landing sequence.
    LandingHandOff,
    Fly {
        altitude_ft: f64,
        speed: ScalarSpeed,
        duration_seconds: u32,
    },
}

/// B48: the command for the current waypoint.
pub fn route_command(inputs: &RouteInputs, random: &mut DecisionRandom) -> Result<RouteCommand> {
    let Some(waypoint) = inputs.waypoint else {
        return Ok(RouteCommand::HoldHeading {
            duration_seconds: ROUTE_COMMAND_SECONDS,
        });
    };
    if !(waypoint.distance_ft.is_finite() && waypoint.altitude_ft.is_finite()) {
        return Err(AiError::InvalidInput(
            "waypoint distance and altitude must be finite",
        ));
    }
    if waypoint.landing && inputs.has_airport && waypoint.distance_ft <= LANDING_HANDOFF_FT {
        return Ok(RouteCommand::LandingHandOff);
    }
    let mut altitude_ft = waypoint.altitude_ft;
    if inputs.wing_leader && waypoint.distance_ft >= ALTITUDE_VARIATION_MIN_DISTANCE_FT {
        altitude_ft += f64::from(
            random
                .site("waypoint altitude variation")
                .range(-ALTITUDE_VARIATION_FT, ALTITUDE_VARIATION_FT),
        );
    }
    if waypoint.landing {
        altitude_ft = altitude_ft.max(LANDING_MIN_ALTITUDE_FT);
    }
    let altitude_ft = altitude_ft.max(0.0);
    let speed = waypoint
        .speed
        .max(inputs.limits.minimum)
        .min(inputs.limits.maximum);
    Ok(RouteCommand::Fly {
        altitude_ft,
        speed,
        duration_seconds: ROUTE_COMMAND_SECONDS,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JoinLandingInputs {
    /// The leader is taking off or landing.
    pub leader_recovering: bool,
    pub distance_to_leader_ft: f64,
    /// Distance to the leader's airport, `None` when the leader has none.
    pub distance_to_leader_airport_ft: Option<f64>,
}

/// B48: an AI wingman joins the landing when its leader is taking off or
/// landing, within 10000 ft of the leader and 40000 ft of the leader's
/// airport. The caller applies this to AI wingmen only.
pub fn join_leader_landing(inputs: &JoinLandingInputs) -> bool {
    inputs.leader_recovering
        && inputs.distance_to_leader_ft <= JOIN_LANDING_LEADER_FT
        && inputs
            .distance_to_leader_airport_ft
            .is_some_and(|distance| distance <= JOIN_LANDING_AIRPORT_FT)
}

/// B48: cruise speed is the minimum plus one fifth of the envelope, or plus
/// half of it when the one-fifth value is under 75 ft/s.
pub fn cruise_speed(limits: &SpeedLimits) -> ScalarSpeed {
    let span = limits.maximum.0 - limits.minimum.0;
    let fifth = limits.minimum.plus(span / 5.0);
    if fifth >= CRUISE_FIFTH_MIN_SPEED {
        fifth
    } else {
        limits.minimum.plus(span / 2.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FuelState {
    /// No home airport: no time-to-home, no bingo or caution.
    NoManagement,
    OutOfFuel,
    Critical,
    Bingo,
    Caution,
    Ok,
}

/// B48: fuel state from endurance and time-to-home, both in seconds. The
/// host computes both at the cruise speed and lowest throttle that holds it.
pub fn fuel_state(endurance_s: f64, time_home_s: Option<f64>) -> Result<FuelState> {
    if !endurance_s.is_finite() || time_home_s.is_some_and(|t| !t.is_finite() || t < 0.0) {
        return Err(AiError::InvalidInput("fuel times must be finite"));
    }
    let Some(time_home_s) = time_home_s else {
        return Ok(FuelState::NoManagement);
    };
    Ok(if endurance_s <= 0.0 {
        FuelState::OutOfFuel
    } else if endurance_s < CRITICAL_ENDURANCE_S {
        FuelState::Critical
    } else if endurance_s < time_home_s + BINGO_MARGIN_S {
        FuelState::Bingo
    } else if endurance_s <= time_home_s + CAUTION_MARGIN_S {
        FuelState::Caution
    } else {
        FuelState::Ok
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FuelEvent {
    /// Internal fuel reached zero: the aircraft is lost, home airport or not.
    Lost,
}

/// B48: an aircraft whose internal fuel reaches zero is lost.
pub fn internal_fuel_event(internal_fuel_remaining: f64) -> Option<FuelEvent> {
    (internal_fuel_remaining <= 0.0).then_some(FuelEvent::Lost)
}

/// Horizontal position in feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub x: f64,
    pub z: f64,
}

/// Who flies the aircraft and its leader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WingCrew {
    pub wingman_ai: bool,
    pub leader_ai: bool,
}

/// The private landing route flown home on bingo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrivateRoute {
    pub destination: Position,
    pub altitude_ft: u32,
    pub speed: ScalarSpeed,
    pub landing: bool,
}

/// B48: an AI wingman whose leader is AI-controlled leaves for its home
/// airport on bingo at 5000 to 10000 ft and cruise speed. Leaders, singletons
/// and human-led wingmen get no route: their return is open.
pub fn bingo_route(
    crew: WingCrew,
    home_airport: Position,
    limits: &SpeedLimits,
    random: &mut DecisionRandom,
) -> Option<PrivateRoute> {
    if !(crew.wingman_ai && crew.leader_ai) {
        return None;
    }
    Some(PrivateRoute {
        destination: home_airport,
        altitude_ft: BINGO_ROUTE_MIN_ALTITUDE_FT
            + random
                .site("bingo route altitude")
                .below(BINGO_ROUTE_ALTITUDE_SPAN_FT),
        speed: cruise_speed(limits),
        landing: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(min: f64, max: f64) -> SpeedLimits {
        SpeedLimits {
            minimum: ScalarSpeed(min),
            maximum: ScalarSpeed(max),
            corner: ScalarSpeed((min + max) / 2.0),
        }
    }

    #[test]
    fn octants_cover_eight_sectors() {
        assert_eq!(octant(1.0, 0.0).unwrap().sector(), 0);
        assert_eq!(octant(1.0, 1.0).unwrap().sector(), 1);
        assert_eq!(octant(0.0, 1.0).unwrap().sector(), 2);
        assert_eq!(octant(-1.0, 0.0).unwrap().sector(), 4);
        assert_eq!(octant(0.0, -1.0).unwrap().sector(), 6);
        assert_eq!(octant(1.0, -0.1).unwrap().sector(), 7);
        assert_eq!(octant(1.0, 0.0).unwrap().opposite().sector(), 4);
        assert_eq!(octant(-1.0, -1.0).unwrap().opposite().sector(), 1);
        assert!(octant(0.0, 0.0).is_err());
        assert!(octant(f64::NAN, 1.0).is_err());
        assert!(Octant::new(8).is_err());
    }

    #[test]
    fn behind_is_equal_or_adjacent_with_wrap() {
        let o = |s| Octant::new(s).unwrap();
        assert!(waypoint_behind(o(4), o(4)));
        assert!(waypoint_behind(o(3), o(4)));
        assert!(waypoint_behind(o(5), o(4)));
        assert!(!waypoint_behind(o(2), o(4)));
        assert!(!waypoint_behind(o(0), o(4)));
        assert!(waypoint_behind(o(7), o(0)));
        assert!(waypoint_behind(o(0), o(7)));
        assert!(!waypoint_behind(o(1), o(7)));
    }

    #[test]
    fn completion_conditions() {
        let ordinary = WaypointCompletion {
            behind: true,
            kind: WaypointKind::Ordinary,
        };
        assert!(waypoint_complete(&ordinary));
        assert!(!waypoint_complete(&WaypointCompletion {
            behind: false,
            ..ordinary
        }));
        let goal = |goal_destroyed, wing_has_usable_weapon| WaypointCompletion {
            behind: true,
            kind: WaypointKind::GoalObject {
                goal_destroyed,
                wing_has_usable_weapon,
            },
        };
        assert!(waypoint_complete(&goal(true, true)));
        assert!(waypoint_complete(&goal(false, false)));
        assert!(!waypoint_complete(&goal(false, true)));
        assert!(!waypoint_complete(&WaypointCompletion {
            behind: false,
            ..goal(true, false)
        }));
        let ground = |on_ground| WaypointCompletion {
            behind: true,
            kind: WaypointKind::Ground { on_ground },
        };
        assert!(waypoint_complete(&ground(true)));
        assert!(!waypoint_complete(&ground(false)));
    }

    fn inputs(waypoint: Waypoint) -> RouteInputs {
        RouteInputs {
            waypoint: Some(waypoint),
            has_airport: true,
            wing_leader: false,
            limits: limits(200.0, 1000.0),
        }
    }

    fn landing(distance_ft: f64) -> Waypoint {
        Waypoint {
            altitude_ft: 1000.0,
            speed: ScalarSpeed(300.0),
            landing: true,
            distance_ft,
        }
    }

    #[test]
    fn landing_hand_off_at_sixty_thousand_feet() {
        let mut random = DecisionRandom::seeded(1);
        assert_eq!(
            route_command(&inputs(landing(60000.0)), &mut random),
            Ok(RouteCommand::LandingHandOff)
        );
        assert_eq!(
            route_command(&inputs(landing(60001.0)), &mut random),
            Ok(RouteCommand::Fly {
                altitude_ft: 2000.0,
                speed: ScalarSpeed(300.0),
                duration_seconds: 5,
            })
        );
        let mut no_airport = inputs(landing(100.0));
        no_airport.has_airport = false;
        assert!(matches!(
            route_command(&no_airport, &mut random),
            Ok(RouteCommand::Fly { .. })
        ));
    }

    #[test]
    fn landing_waypoints_are_never_commanded_below_two_thousand() {
        let mut random = DecisionRandom::seeded(4);
        let mut i = inputs(landing(70000.0));
        i.wing_leader = true;
        for _ in 0..200 {
            match route_command(&i, &mut random).unwrap() {
                RouteCommand::Fly { altitude_ft, .. } => assert_eq!(altitude_ft, 2000.0),
                other => panic!("{other:?}"),
            }
        }
    }

    fn cruise(distance_ft: f64) -> Waypoint {
        Waypoint {
            altitude_ft: 8000.0,
            speed: ScalarSpeed(500.0),
            landing: false,
            distance_ft,
        }
    }

    #[test]
    fn leader_altitude_varies_only_beyond_five_thousand_feet() {
        let mut random = DecisionRandom::seeded(8);
        let mut leader = inputs(cruise(5000.0));
        leader.wing_leader = true;
        let mut low = f64::MAX;
        let mut high = f64::MIN;
        for _ in 0..2000 {
            let RouteCommand::Fly { altitude_ft, .. } =
                route_command(&leader, &mut random).unwrap()
            else {
                panic!("expected fly");
            };
            assert!((7900.0..=8100.0).contains(&altitude_ft), "{altitude_ft}");
            low = low.min(altitude_ft);
            high = high.max(altitude_ft);
        }
        assert_eq!(low, 7900.0);
        assert_eq!(high, 8100.0);
        let mut near = leader;
        near.waypoint = Some(cruise(4999.0));
        let mut wingman = inputs(cruise(20000.0));
        wingman.wing_leader = false;
        for i in [near, wingman] {
            for _ in 0..50 {
                assert_eq!(
                    route_command(&i, &mut random),
                    Ok(RouteCommand::Fly {
                        altitude_ft: 8000.0,
                        speed: ScalarSpeed(500.0),
                        duration_seconds: 5,
                    })
                );
            }
        }
    }

    #[test]
    fn altitude_floor_is_zero_and_speed_is_clamped() {
        let mut random = DecisionRandom::seeded(2);
        let mut i = inputs(Waypoint {
            altitude_ft: -50.0,
            speed: ScalarSpeed(50.0),
            landing: false,
            distance_ft: 1000.0,
        });
        assert_eq!(
            route_command(&i, &mut random),
            Ok(RouteCommand::Fly {
                altitude_ft: 0.0,
                speed: ScalarSpeed(200.0),
                duration_seconds: ROUTE_COMMAND_SECONDS,
            })
        );
        i.waypoint = Some(Waypoint {
            speed: ScalarSpeed(5000.0),
            ..i.waypoint.unwrap()
        });
        assert!(matches!(
            route_command(&i, &mut random),
            Ok(RouteCommand::Fly { speed: ScalarSpeed(s), .. }) if s == 1000.0
        ));
        i.waypoint = None;
        assert_eq!(
            route_command(&i, &mut random),
            Ok(RouteCommand::HoldHeading {
                duration_seconds: 5
            })
        );
        i.waypoint = Some(Waypoint {
            distance_ft: f64::NAN,
            ..cruise(0.0)
        });
        assert!(route_command(&i, &mut random).is_err());
    }

    #[test]
    fn join_landing_boundaries() {
        let base = JoinLandingInputs {
            leader_recovering: true,
            distance_to_leader_ft: 10000.0,
            distance_to_leader_airport_ft: Some(40000.0),
        };
        assert!(join_leader_landing(&base));
        assert!(!join_leader_landing(&JoinLandingInputs {
            distance_to_leader_ft: 10001.0,
            ..base
        }));
        assert!(!join_leader_landing(&JoinLandingInputs {
            distance_to_leader_airport_ft: Some(40001.0),
            ..base
        }));
        assert!(!join_leader_landing(&JoinLandingInputs {
            distance_to_leader_airport_ft: None,
            ..base
        }));
        assert!(!join_leader_landing(&JoinLandingInputs {
            leader_recovering: false,
            ..base
        }));
    }

    #[test]
    fn cruise_speed_takes_a_fifth_or_a_half_at_the_boundary() {
        // 50 + (175 - 50) / 5 = 75: the one-fifth rule holds.
        assert_eq!(cruise_speed(&limits(50.0, 175.0)), ScalarSpeed(75.0));
        // 50 + (170 - 50) / 5 = 74: fall back to half the envelope, 110.
        assert_eq!(cruise_speed(&limits(50.0, 170.0)), ScalarSpeed(110.0));
        assert_eq!(cruise_speed(&limits(200.0, 1200.0)), ScalarSpeed(400.0));
    }

    #[test]
    fn fuel_thresholds_at_each_edge() {
        let home = Some(1000.0);
        assert_eq!(fuel_state(0.0, home), Ok(FuelState::OutOfFuel));
        assert_eq!(fuel_state(-1.0, home), Ok(FuelState::OutOfFuel));
        assert_eq!(fuel_state(0.5, home), Ok(FuelState::Critical));
        assert_eq!(fuel_state(239.9, home), Ok(FuelState::Critical));
        assert_eq!(fuel_state(240.0, home), Ok(FuelState::Bingo));
        assert_eq!(fuel_state(1299.9, home), Ok(FuelState::Bingo));
        assert_eq!(fuel_state(1300.0, home), Ok(FuelState::Caution));
        assert_eq!(fuel_state(1600.0, home), Ok(FuelState::Caution));
        assert_eq!(fuel_state(1600.1, home), Ok(FuelState::Ok));
        // Critical outranks bingo when time home is short.
        assert_eq!(fuel_state(100.0, Some(0.0)), Ok(FuelState::Critical));
        assert_eq!(fuel_state(100.0, None), Ok(FuelState::NoManagement));
        assert_eq!(fuel_state(0.0, None), Ok(FuelState::NoManagement));
        assert!(fuel_state(f64::NAN, home).is_err());
        assert!(fuel_state(100.0, Some(-1.0)).is_err());
        assert_eq!(internal_fuel_event(0.0), Some(FuelEvent::Lost));
        assert_eq!(internal_fuel_event(1.0), None);
    }

    #[test]
    fn bingo_route_only_for_ai_wingman_with_ai_leader() {
        let mut random = DecisionRandom::seeded(6);
        let home = Position { x: 1.0, z: 2.0 };
        let limits = limits(200.0, 1200.0);
        for (wingman_ai, leader_ai) in [(false, false), (false, true), (true, false)] {
            let crew = WingCrew {
                wingman_ai,
                leader_ai,
            };
            assert_eq!(bingo_route(crew, home, &limits, &mut random), None);
        }
        let crew = WingCrew {
            wingman_ai: true,
            leader_ai: true,
        };
        let mut low = u32::MAX;
        let mut high = 0;
        for _ in 0..5000 {
            let route = bingo_route(crew, home, &limits, &mut random).unwrap();
            assert_eq!(route.destination, home);
            assert!(route.landing);
            assert_eq!(route.speed, ScalarSpeed(400.0));
            assert!(
                (5000..10000).contains(&route.altitude_ft),
                "{}",
                route.altitude_ft
            );
            low = low.min(route.altitude_ft);
            high = high.max(route.altitude_ft);
        }
        assert!(low < 5100 && high > 9900, "{low}..{high}");
    }
}
