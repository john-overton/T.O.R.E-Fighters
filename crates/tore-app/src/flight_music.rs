//! Situation music observations from authoritative flight state.
//!
//! Read-only: nothing here feeds back into the simulation, and it runs only
//! when audio exists, so headless and `--no-audio` runs are unchanged. One
//! call per fixed 120 Hz step; time is game time counted in those steps.
//! Rules and numbers are from `docs/spec/flight-music.md`; how each maps onto
//! TORE state is recorded there under "Current TORE state".
//!
//! Each change in the inputs, and why each is on, is written as a journal
//! entry ([`Step::journal`]). The score the mixer then plays is decided in
//! the audio device and is not visible here.
use crate::{
    ai_wings::{AiWings, PLAYER_ID, outcome},
    audio::situation::{self, AIM_MEMORY_S, AIM120_IGNORE_FT, AIR_RANGE_FT, HIT_HOLD_S},
    comms::journal::{Audience, Cause, Entry, Music, Origin, Outcome, Source},
    comms::{Call, Kind, Phrase, Phrases},
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
    /// Journal entries of this step: the inputs changed, and why each is
    /// on. The host moves them into the channel's journal.
    #[allow(dead_code)] // Read by the mission recorder's host hook.
    pub journal: Vec<Entry>,
    /// The trigger of each `radio` stem, in the same order.
    #[allow(dead_code)] // Read by `radio_calls`, the host's hook.
    causes: Vec<Cause>,
}

impl Step {
    /// The calls for [`Self::radio`], exactly as the host sends them (the
    /// mission result 2 s after it is decided, "almost home" at once, both
    /// important), each with its trigger. `label` is the crew label or
    /// `YOU`.
    #[allow(dead_code)] // The host's hook: main.rs sends these today without a trigger.
    pub fn radio_calls(&self, label: &str, phrases: &Phrases) -> Vec<Call> {
        self.radio
            .iter()
            .zip(&self.causes)
            .map(|(stem, cause)| {
                let delay = if *stem == outcome::MISSION_ACCOMPLISHED {
                    2.
                } else {
                    0.
                };
                Call::new(label, Phrase::stem(phrases, stem), Kind::Important)
                    .after(delay)
                    .because(
                        Origin::of(Source::Radio, cause.clone())
                            .by(PLAYER_ID)
                            .to(Audience::Flight),
                    )
            })
            .collect()
    }
}

/// What the inputs were last journaled as: the asked rank, the inputs, and
/// the ids behind them. Ranges are left out so a moving target does not
/// make an entry every step.
#[derive(Clone, Debug, PartialEq)]
struct Seen {
    rank: situation::Rank,
    inputs: situation::Inputs,
    designated: Option<u32>,
    aiming: Vec<u32>,
    inbound: Vec<u32>,
}

#[derive(Default)]
pub struct Observer {
    steps: u64,
    hit: situation::Hold,
    aim: situation::Hold,
    airport: situation::Airport,
    outcome: outcome::Tracker,
    /// Journal only: the home base, the last hit and the inputs last
    /// journaled. No rule reads them.
    home_base: Option<[f64; 3]>,
    hit_at: Option<f64>,
    seen: Option<Seen>,
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
            home_base,
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
            self.hit_at = Some(now);
        }

        // Designated target: live, the other side, and an aircraft. Without AI
        // wings every non-friendly target (range and fixture aircraft) counts
        // as the other side.
        let designated_target = combat
            .designated()
            .and_then(|id| combat.targets.iter().find(|t| t.id == id && t.hp > 0))
            .filter(|t| t.role == TargetRole::Aircraft)
            .filter(|t| {
                wings.map_or(!combat.friendlies.contains(&t.id), |w| {
                    w.slot(t.id).is_some_and(|slot| slot.side.is_enemy())
                })
            })
            .map(|t| (t.id, distance(t.position, position)));
        let designated = designated_target.map(|(_, range)| range);
        let air_target = designated.is_some_and(|range| range < AIR_RANGE_FT);
        let far_target = designated.is_some_and(|range| range >= AIR_RANGE_FT);

        // `fitted`: TORE's AI does not keep a selected station, so "missile
        // selected" is an alive AI aircraft whose target is the player and
        // which still carries a usable guided air-to-air store. The 1 s
        // final-attack memory has no TORE equivalent; 4 s is always used.
        // Every such aircraft is listed for the journal.
        let aiming: Vec<u32> = wings.map_or_else(Vec::new, |w| {
            w.mission()
                .actors()
                .iter()
                .filter(|actor| {
                    actor.alive()
                        && actor.controller().target() == Some(PLAYER_ID)
                        && actor.stations().iter().any(|s| {
                            s.guided
                                && !s.store.inhibited
                                && weapon_service::store_eligible(s.capability, TargetClass::Air)
                                && !matches!(s.store.rounds, Rounds::Finite(0))
                        })
                })
                .map(|actor| actor.id())
                .collect()
        });
        if !aiming.is_empty() {
            self.aim.refresh(now, AIM_MEMORY_S);
        }

        // Missiles guided at the player, counted every step; an AIM-120
        // farther than 30,380 ft is not counted.
        let inbound_ids: Vec<u32> = combat
            .projectiles
            .iter()
            .filter(|p| {
                p.incoming
                    && p.target == Some(live::PLAYER_OWNER)
                    && !(p
                        .weapon(combat.configuration())
                        .source
                        .eq_ignore_ascii_case("AIM120.JT")
                        && distance(p.position, position) > AIM120_IGNORE_FT)
            })
            .map(|p| p.id)
            .collect();
        let inbound = !inbound_ids.is_empty();

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

        let inputs = situation::Inputs {
            succeeded: status.succeeded,
            ejected: flight.escape.is_some(),
            launching,
            air_target,
            hit_recently: self.hit.active(now),
            danger: far_target || self.aim.active(now) || inbound,
            home: status.home,
            deck,
        };
        let causes = status
            .radio
            .iter()
            .map(|stem| match *stem {
                outcome::ALMOST_HOME => {
                    let (range_ft, altitude_ft) = self.home_base.map_or((0., 0.), |base| {
                        let across = (base[0] - position[0]).hypot(base[2] - position[2]);
                        (across.hypot(base[1] - position[1]), position[1])
                    });
                    Cause::AlmostHome {
                        range_ft,
                        altitude_ft,
                    }
                }
                _ => Cause::MissionAccomplished,
            })
            .collect();
        let journal = self.journal(now, inputs, designated_target, aiming, inbound_ids);
        Step {
            now,
            inputs,
            radio: status.radio,
            journal,
            causes,
        }
    }

    /// One entry when the inputs, or the aircraft behind them, changed.
    /// Journal only.
    fn journal(
        &mut self,
        now: f64,
        inputs: situation::Inputs,
        designated: Option<(u32, f64)>,
        aiming: Vec<u32>,
        inbound: Vec<u32>,
    ) -> Vec<Entry> {
        // The rank the inputs ask for, top down, before the mixer's own
        // rules (lockout, once-per-flight scores, Valkyries, retries).
        let rank = situation::Selector::default().choose(&inputs);
        let seen = Seen {
            rank,
            inputs,
            designated: designated.map(|(id, _)| id),
            aiming: aiming.clone(),
            inbound: inbound.clone(),
        };
        if self.seen.as_ref() == Some(&seen) {
            return Vec::new();
        }
        let from = self.seen.replace(seen).map(|s| s.rank);
        let music = Music {
            from,
            to: rank,
            inputs,
            designated,
            aiming,
            inbound,
            hit_at: self.hit_at,
        };
        vec![
            Entry::note(
                now,
                format!("{rank:?}"),
                Origin::of(Source::Music, Cause::Music(Box::new(music))).to(Audience::Cockpit),
                Outcome::Noted,
            )
            .with_text(format!("{rank:?}").to_uppercase()),
        ]
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

    #[test]
    fn input_changes_are_journaled_with_the_reasons_behind_them() {
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
        let music = |step: &Step| match &step.journal[..] {
            [entry] => match &entry.origin.cause {
                Cause::Music(music) => (**music).clone(),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
        let first = music(&step(&mut observer, &combat, &[]));
        assert_eq!((first.from, first.to), (None, situation::Rank::Normal));
        assert!(
            step(&mut observer, &combat, &[]).journal.is_empty(),
            "nothing changed"
        );
        let hit = music(&step(
            &mut observer,
            &combat,
            &[live::Event::PlayerDamaged(3)],
        ));
        assert_eq!(
            (hit.from, hit.to),
            (Some(situation::Rank::Normal), situation::Rank::Air)
        );
        assert_eq!(hit.hit_at, Some(2. * flight::DT));
        combat.command(live::Command::Incoming, crate::combat::launcher(&flight));
        let inbound = music(&step(&mut observer, &combat, &[]));
        assert_eq!(inbound.inbound, [combat.projectiles[0].id]);
        assert!(inbound.inputs.danger);
        assert_eq!(inbound.to, situation::Rank::Air, "a hit outranks danger");
        assert!(
            Cause::Music(Box::new(inbound))
                .to_string()
                .contains("guided at you")
        );
    }

    #[test]
    fn result_calls_match_what_the_host_sends_and_carry_their_trigger() {
        let world = crate::terrain::tests::world();
        let combat = crate::ai_wings::tests::combat_fixture(true);
        let flight = flight::State::new(
            &crate::flight::animation_tests::profile(),
            [0., 5_000., 10_000.],
        )
        .unwrap();
        let mut observer = Observer::new(Some([0.; 3]));
        // Not yet at the first check, then a success on the 4 s cadence.
        let step = (0..600)
            .map(|i| {
                let succeeded = move || i > 0;
                let mission = Some(&succeeded as &dyn Fn() -> bool);
                observer.step(&flight, &combat, &[], None, &world, mission)
            })
            .find(|step| !step.radio.is_empty())
            .unwrap();
        assert_eq!(
            step.radio,
            [outcome::MISSION_ACCOMPLISHED, outcome::ALMOST_HOME]
        );
        let phrases: Phrases = [("^MISSACC", "Mission accomplished")]
            .into_iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let calls = step.radio_calls("RIO", &phrases);
        for (call, stem) in calls.iter().zip(&step.radio) {
            // What the host builds for each stem today.
            let delay = if *stem == outcome::MISSION_ACCOMPLISHED {
                2.
            } else {
                0.
            };
            let host = Call::new("RIO", Phrase::stem(&phrases, stem), Kind::Important).after(delay);
            assert_eq!(
                (&call.label, &call.text, &call.stems, call.kind),
                (&host.label, &host.text, &host.stems, host.kind)
            );
            assert_eq!((call.route, call.delay), (host.route, host.delay));
        }
        assert_eq!(calls[0].origin.cause, Cause::MissionAccomplished);
        assert_eq!(
            calls[1].origin.cause,
            Cause::AlmostHome {
                range_ft: 10_000f64.hypot(5_000.),
                altitude_ft: 5_000.
            }
        );
    }
}
