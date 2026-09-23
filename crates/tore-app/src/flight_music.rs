//! Situation music observations from authoritative flight state.
//!
//! Read-only: nothing here feeds back into the simulation, and it runs only
//! when audio exists, so headless and `--no-audio` runs are unchanged. One
//! call per fixed 120 Hz step; time is game time counted in those steps.
//! Rules and numbers are from `docs/spec/flight-music.md`; how each maps onto
//! TORE state is recorded there under "Current TORE state".
use crate::{
    ai_wings::{AiWings, PLAYER_ID, outcome},
    audio::situation::{self, AIM_MEMORY_S, AIM120_IGNORE_FT, AIR_RANGE_FT, HIT_HOLD_S},
    flight,
    terrain::World,
};
use tore_sim::ai::weapon_service::{self, Rounds, TargetClass};
use tore_sim::combat::{live, missiles::TargetRole};

pub struct Step {
    pub now: f64,
    pub inputs: situation::Inputs,
    /// Radio stems to queue now.
    pub radio: Vec<&'static str>,
}

#[derive(Default)]
pub struct Observer {
    steps: u64,
    hit: situation::Hold,
    aim: situation::Hold,
    airport: situation::Airport,
    outcome: outcome::Tracker,
}

/// `fitted`: the Quick Mission home base is the ground-start airport, placed
/// at the mean centre of its runways. An airborne start has no home base.
pub fn home_base(world: &World, airport: Option<u32>) -> Option<[f64; 3]> {
    let runways: Vec<_> = world
        .airport_scene
        .runways
        .iter()
        .filter(|runway| Some(runway.airport) == airport)
        .collect();
    (!runways.is_empty()).then(|| {
        std::array::from_fn(|i| {
            runways.iter().map(|r| r.surface.center[i]).sum::<f64>() / runways.len() as f64
        })
    })
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt()
}

impl Observer {
    pub fn new(home_base: Option<[f64; 3]>) -> Self {
        Self {
            outcome: outcome::Tracker::new(home_base),
            ..Self::default()
        }
    }

    pub fn step(
        &mut self,
        flight: &flight::State,
        combat: &live::State,
        events: &[live::Event],
        wings: Option<&AiWings>,
        world: &World,
        // Whether the mission has succeeded, from the debrief evaluator;
        // `None` without a mission. Called only on the 4 second cadence.
        mission: Option<&dyn Fn() -> bool>,
    ) -> Step {
        let now = self.steps as f64 * flight::DT;
        self.steps += 1;
        let position = flight.position;

        if events
            .iter()
            .any(|e| matches!(e, live::Event::PlayerDamaged(_)))
        {
            self.hit.refresh(now, HIT_HOLD_S);
        }

        // Designated target: live, the other side, and an aircraft. Without AI
        // wings every non-friendly target (range and fixture aircraft) counts
        // as the other side.
        let designated = combat
            .designated()
            .and_then(|id| combat.targets.iter().find(|t| t.id == id && t.hp > 0))
            .filter(|t| t.role == TargetRole::Aircraft)
            .filter(|t| {
                wings.map_or(!combat.friendlies.contains(&t.id), |w| {
                    w.slot(t.id).is_some_and(|slot| slot.side.is_enemy())
                })
            })
            .map(|t| distance(t.position, position));
        let air_target = designated.is_some_and(|range| range < AIR_RANGE_FT);
        let far_target = designated.is_some_and(|range| range >= AIR_RANGE_FT);

        // `fitted`: TORE's AI does not keep a selected station, so "missile
        // selected" is an alive AI aircraft whose target is the player and
        // which still carries a usable guided air-to-air store. The 1 s
        // final-attack memory has no TORE equivalent; 4 s is always used.
        if wings.is_some_and(|w| {
            w.mission().actors().iter().any(|actor| {
                actor.alive()
                    && actor.controller().target() == Some(PLAYER_ID)
                    && actor.stations().iter().any(|s| {
                        s.guided
                            && !s.store.inhibited
                            && weapon_service::store_eligible(s.capability, TargetClass::Air)
                            && !matches!(s.store.rounds, Rounds::Finite(0))
                    })
            })
        }) {
            self.aim.refresh(now, AIM_MEMORY_S);
        }

        // Missiles guided at the player, counted every step; an AIM-120
        // farther than 30,380 ft is not counted.
        let inbound = combat.projectiles.iter().any(|p| {
            p.incoming
                && p.target == Some(live::PLAYER_OWNER)
                && !(p
                    .weapon(combat.configuration())
                    .source
                    .eq_ignore_ascii_case("AIM120.JT")
                    && distance(p.position, position) > AIM120_IGNORE_FT)
        });

        let on_runway = world
            .airport_scene
            .runway_surface(position[0], position[2])
            .is_some_and(|(_, height)| flight.supported_at(height));
        let ground = f64::from(world.height(position[0] as f32, position[2] as f32));
        let (deck, launching) = self.airport.step(&situation::Ground {
            on_ground: on_runway,
            speed_fps: flight.speed,
            position,
            agl_ft: position[1] - ground,
            gear_down: flight.gear_down,
        });

        let airborne = !on_runway && !flight.research.as_ref().is_some_and(|r| r.on_ground);
        let status = self.outcome.step(now, mission, position, airborne);

        Step {
            now,
            inputs: situation::Inputs {
                succeeded: status.succeeded,
                ejected: flight.escape.is_some(),
                launching,
                air_target,
                hit_recently: self.hit.active(now),
                danger: far_target || self.aim.active(now) || inbound,
                home: status.home,
                deck,
            },
            radio: status.radio,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_and_guided_missiles_feed_air_and_danger() {
        let world = crate::terrain::tests::world();
        let mut combat = crate::ai_wings::tests::combat_fixture(true);
        let flight = flight::State::new(
            &crate::flight::animation_tests::profile(),
            [0., 20_000., 0.],
        )
        .unwrap();
        let mut observer = Observer::new(None);
        let step = |observer: &mut Observer, combat: &live::State, events: &[live::Event]| {
            observer.step(&flight, combat, events, None, &world, None)
        };
        let first = step(&mut observer, &combat, &[live::Event::PlayerDamaged(3)]);
        assert_eq!(first.now, 0.);
        assert!(first.inputs.hit_recently && !first.inputs.danger);
        for _ in 1..(30 * 120) {
            assert!(step(&mut observer, &combat, &[]).inputs.hit_recently);
        }
        let later = step(&mut observer, &combat, &[]);
        assert_eq!(later.now, 30.);
        assert!(
            !later.inputs.hit_recently,
            "the hold ends 30 game seconds after the hit"
        );
        assert!(!later.inputs.succeeded && !later.inputs.home, "no mission");
        assert!(later.radio.is_empty());

        // A guided round aimed at the player selects DANGER.
        combat.command(live::Command::Incoming, crate::combat::launcher(&flight));
        assert_eq!(combat.projectiles[0].target, Some(live::PLAYER_OWNER));
        assert!(step(&mut observer, &combat, &[]).inputs.danger);
        // An AIM-120 beyond 30,380 ft is not counted; inside, it is.
        let mut aim120 = combat.configuration().stations[0].weapon.clone();
        aim120.source = "AIM120.JT".into();
        combat.projectiles[0].weapon = Some(aim120);
        combat.projectiles[0].position = [30_381., 20_000., 0.];
        assert!(!step(&mut observer, &combat, &[]).inputs.danger);
        combat.projectiles[0].position = [30_379., 20_000., 0.];
        assert!(step(&mut observer, &combat, &[]).inputs.danger);
    }
}
