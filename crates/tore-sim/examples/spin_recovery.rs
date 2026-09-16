//! Seeded high-speed departure regression using a user-owned F14.PT or A4E.PT.
//! This setup is synthetic; it does not assert that retail enters this attitude.
use std::{fs::File, io::Read};
use tore_formats::{aircraft::Aircraft, flight_model::departure::DepartureMode};
use tore_sim::{
    attitude::{Basis, dot, unit},
    flight::{PilotInput, State},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: spin_recovery AIRCRAFT.PT")?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("PT exceeds size limit".into());
    }
    let aircraft = Aircraft::parse(&bytes)?;
    if !matches!(
        aircraft.id,
        tore_formats::aircraft::AircraftId::F14 | tore_formats::aircraft::AircraftId::A4E
    ) {
        return Err("this regression requires FA F14.PT or A4E.PT".into());
    }
    for scenario in ["full", "aligned", "release-early"] {
        let partial_aligned = scenario == "aligned";
        for speed in [300. * 5280. / 3600., 350. * 1.68781, 600. * 5280. / 3600.] {
            for direction in [-1, 1] {
                for throttle in [0., 0.4, 1.] {
                    let mut state = State::new(&aircraft, [0., 30000., 0.])?;
                    state.enable_research(1)?;
                    state.pitch = if partial_aligned {
                        0.
                    } else {
                        70_f64.to_radians()
                    };
                    state.speed = speed;
                    state.velocity = if partial_aligned {
                        Basis::new(state.yaw, state.pitch, state.bank)
                            .forward
                            .map(|v| v * speed)
                    } else {
                        [0., -speed, 0.]
                    };
                    state.throttle = throttle;
                    state.burner = false;
                    let r = state.research.as_mut().unwrap();
                    r.departure.mode = DepartureMode::Spinning;
                    r.spinning = direction;
                    let model = tore_sim::models::AircraftModel::for_aircraft(&aircraft)?;
                    use tore_sim::models::FlightModel;
                    r.spin_rate = f64::from(direction)
                        * tore_sim::research::Research::maximum_spin_rate(model.configuration())
                        * if partial_aligned { 0.5 } else { 1. };
                    let mut arrested_at = None;
                    let mut recovered_at = None;
                    let mut replay = state.clone();
                    let mut previous_fraction = state.research.as_ref().unwrap().spin_rate.abs();
                    for tick in 0..120 * 20 {
                        let input = if arrested_at.is_none()
                            && !partial_aligned
                            && (scenario != "release-early" || tick < 60)
                        {
                            PilotInput {
                                pitch: -1.,
                                yaw: -f64::from(direction),
                                ..Default::default()
                            }
                        } else {
                            PilotInput::default()
                        };
                        state.step(&input, |_, _| 0.);
                        replay.step(&input, |_, _| 0.);
                        assert_eq!(state, replay);
                        assert!(!state.crashed);
                        let r = state.research.as_ref().unwrap();
                        if scenario == "release-early" && tick >= 60 {
                            assert!(
                                r.spin_rate.abs() <= previous_fraction,
                                "releasing recovery controls rebuilt spin"
                            );
                        }
                        if r.spinning == 0 && arrested_at.is_none() {
                            arrested_at = Some(tick);
                        }
                        if arrested_at.is_some() {
                            assert!(
                                r.spin_rate.abs() <= previous_fraction,
                                "neutral controls restarted forced spin"
                            );
                        }
                        previous_fraction = r.spin_rate.abs();
                        if r.departure.mode == DepartureMode::Normal && recovered_at.is_none() {
                            recovered_at = Some(tick);
                        }
                    }
                    println!(
                        "scenario={scenario} speed_fps={speed:.1} direction={direction} throttle={throttle} arrest_s={:?} recovery_s={:?} final_alignment={:.3}",
                        arrested_at.map(|v| v as f64 / 120.),
                        recovered_at.map(|v| v as f64 / 120.),
                        dot(
                            Basis::new(state.yaw, state.pitch, state.bank).forward,
                            unit(state.velocity)
                        )
                    );
                    assert!(
                        recovered_at.is_some(),
                        "failed to stabilize after spin arrest"
                    );
                }
            }
        }
    }
    Ok(())
}
