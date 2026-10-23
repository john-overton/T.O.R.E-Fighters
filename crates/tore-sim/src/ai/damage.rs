//! Fitted pilot responses to existing component failures. The policy changes
//! intentions and inputs, never the damage model, inventory or achieved pose.
//! Contract: docs/spec/systems-damage.md#ai-pilot-response-to-faults.
use super::airfield::Phase;
use crate::{aircraft_systems::Systems, flight};
use tore_input::{PilotCommand, PilotInput, Switch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Fire,
    Pilot,
    Fuel,
    Engine,
    Oil,
    Hydraulics,
    Controls,
    Structure,
}
impl Reason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fire => "uncontained fire",
            Self::Pilot => "pilot wounded",
            Self::Fuel => "fuel leak or feed failure",
            Self::Engine => "engine damage",
            Self::Oil => "oil pressure loss",
            Self::Hydraulics => "hydraulic failure",
            Self::Controls => "flight controls damaged",
            Self::Structure => "wing or structure damaged",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Power {
    #[default]
    Normal,
    Protect,
    RestartIdle,
    RestartRaise,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Response {
    pub reason: Option<Reason>,
    pub power: Power,
    pub no_burner: bool,
    pub g_cap: Option<f64>,
}
impl Response {
    pub fn assess(s: &Systems, damage: f64) -> Self {
        let reason = if s.structure.burning() {
            Some(Reason::Fire)
        } else if s.pilot.wounded() {
            Some(Reason::Pilot)
        } else if s.has(1) || s.has(2) {
            Some(Reason::Fuel)
        } else if [4, 5, 6, 7, 9, 10].into_iter().any(|i| s.has(i)) {
            Some(Reason::Engine)
        } else if s.has(12) || s.has(13) || s.oil_pressure() < 1. {
            Some(Reason::Oil)
        } else if s.has(14) || s.has(15) {
            Some(Reason::Hydraulics)
        } else if (19..=24).chain(27..=29).any(|i| s.has(i)) {
            Some(Reason::Controls)
        } else if s.structure.wing_damage || s.has(30) {
            Some(Reason::Structure)
        } else {
            None
        };
        let power = if s.engine.flameout > 0. && s.engine.power > 0. {
            if s.engine.flameout > flight::DT {
                Power::RestartIdle
            } else {
                Power::RestartRaise
            }
        } else if s.has(7) || s.oil_pressure() < 1. {
            Power::Protect
        } else {
            Power::Normal
        };
        Self {
            reason,
            power,
            no_burner: reason.is_some() || s.has(8),
            g_cap: reason
                .map(|_| {
                    if s.has(30) {
                        (9. * (1. - damage)).max(2.) - 0.25
                    } else {
                        2.5
                    }
                })
                .map(|g| g.min(2.5)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Trace {
    pub response: Response,
    pub returning: Option<Reason>,
    pub home_known: bool,
    pub runway_known: bool,
    pub ground_hold: bool,
    pub safety_power: bool,
    pub throttle_requested: Option<f64>,
    pub throttle_locked: Option<f64>,
}

/// Retain military power for immediate flight safety. The damaged component
/// still accumulates exposure normally if that requires exceeding its limit.
pub fn safety_power(f: &flight::State, minimum: f64, agl: f64, phase: Option<Phase>) -> bool {
    f.speed <= minimum * 1.35
        || (agl < 500. && f.velocity[1] < -10.)
        || matches!(
            phase,
            Some(Phase::TakeoffRoll | Phase::ClimbOut | Phase::Final)
        )
}

pub fn controls(response: Response, f: &flight::State, safety: bool, input: &mut PilotInput) {
    if response.no_burner {
        input.commands.retain(|c| {
            !matches!(
                c,
                PilotCommand::Set(Switch::Burner, _) | PilotCommand::Toggle(Switch::Burner)
            )
        });
        input
            .commands
            .push(PilotCommand::Set(Switch::Burner, false));
    }
    let throttle = input.throttle.unwrap_or(f.throttle);
    match response.power {
        Power::Protect if !safety => input.throttle = Some(throttle.min(0.25)),
        Power::RestartIdle => input.throttle = Some(throttle.min(0.25)),
        Power::RestartRaise => input.throttle = Some(0.5),
        _ => {}
    }
}

pub fn fire_assessment(f: &flight::State, ground: f64) -> Option<crate::ejection::Assessment> {
    let seconds = f.systems.structure.fire_seconds()?;
    (!f.systems.pilot.dead
        && f.escape.is_none()
        && f.seat_available()
        && !f.wreck_gone()
        && !f.supported_at(ground)
        && f.position[1] > ground)
        .then_some(crate::ejection::Assessment {
            hazard: crate::ejection::Hazard::Fire,
            impact_seconds: seconds,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_faults_choose_responses_but_treated_wounds_and_equipment_do_not() {
        let mut s = Systems::default();
        assert_eq!(Response::assess(&s, 0.), Response::default());
        for index in [0, 16, 17, 18, 31, 32, 33, 36, 44] {
            s.hit(index, 0.8);
        }
        assert!(Response::assess(&s, 0.).reason.is_none());
        s.hit(34, 0.8);
        assert_eq!(Response::assess(&s, 0.).reason, Some(Reason::Pilot));
        s.pilot.advance(true);
        assert!(!s.pilot.wounded());
        assert!(Response::assess(&s, 0.).reason.is_none());
        for (index, reason) in [
            (1, Reason::Fuel),
            (2, Reason::Fuel),
            (5, Reason::Engine),
            (7, Reason::Engine),
            (9, Reason::Engine),
            (12, Reason::Oil),
            (14, Reason::Hydraulics),
            (19, Reason::Controls),
            (29, Reason::Controls),
            (25, Reason::Structure),
            (30, Reason::Structure),
            (11, Reason::Fire),
        ] {
            let mut s = Systems::default();
            s.hit(index, 0.8);
            assert_eq!(
                Response::assess(&s, 0.).reason,
                Some(reason),
                "fault {index}"
            );
        }
    }
    #[test]
    fn protective_throttle_preserves_compressor_and_slows_oil_heating() {
        for fault in [7, 12] {
            let mut careful = Systems::default();
            careful.hit(fault, 1.);
            let mut hard = careful.clone();
            let mut fuel = 10000.;
            assert_eq!(Response::assess(&careful, 0.).power, Power::Protect);
            for _ in 0..4800 {
                careful.advance(true, 0.25, 1., 0., false, &mut fuel);
                hard.advance(true, 1., 1., 0., false, &mut fuel);
            }
            assert!(careful.power_available() > 0.);
            if fault == 7 {
                assert_eq!(hard.power_available(), 0.);
            } else {
                assert!(careful.engine.temperature < hard.engine.temperature * 0.5);
            }
        }
    }
    #[test]
    fn restart_uses_the_component_timer_and_never_repairs_permanent_failure() {
        let mut s = Systems::default();
        s.hit(4, 1.);
        let mut fuel = 10000.;
        for _ in 0..721 {
            let response = Response::assess(&s, 0.);
            let throttle = match response.power {
                Power::RestartIdle => 0.25,
                Power::RestartRaise => 0.5,
                _ => 0.5,
            };
            s.advance(false, throttle, 1., 0., false, &mut fuel);
        }
        assert_eq!(s.engine.flameout, 0.);
        assert_eq!(s.power_available(), 1.);
        s.hit(2, 0.5);
        s.hit(4, 0.5);
        assert!(!matches!(
            Response::assess(&s, 0.).power,
            Power::RestartIdle | Power::RestartRaise
        ));
    }
    #[test]
    fn structural_limit_stays_below_the_existing_absolute_g_failure_threshold() {
        for damage in [0., 0.5, 0.8, 0.95] {
            let mut s = Systems::default();
            s.hit(30, 0.7);
            let cap = Response::assess(&s, damage).g_cap.unwrap();
            assert!(cap <= 2.5 && cap < (9. * (1. - damage)).max(2.));
            let mut fuel = 10000.;
            for _ in 0..480 {
                s.advance(true, 0.7, -cap, damage, false, &mut fuel);
            }
            assert!(!s.structure.failed);
        }
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "damage_checkpoint.rs"]
mod checkpoint;
