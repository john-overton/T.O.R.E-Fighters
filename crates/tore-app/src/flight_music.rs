//! Situation music observations from authoritative flight state.
//!
//! Read-only: nothing here feeds back into the simulation, and it runs only
//! when audio exists, so headless and `--no-audio` runs are unchanged. One
//! call per fixed 120 Hz step; time is game time counted in those steps. The
//! mission result and home checks, and the two radio calls they send, are the
//! mission core's (`ai_wings::outcome`); the music reads their status.
//! Rules and numbers are from `docs/spec/flight-music.md`; how each maps onto
//! TORE state is recorded there under "Current TORE state".
//!
//! Each change in the inputs, and why each is on, is written as a journal
//! entry ([`Step::journal`]). The score the mixer then plays is decided in
//! the audio device and is not visible here.
use crate::{
    comms::journal::{Audience, Cause, Entry, Music, Origin, Outcome, Source},
    flight,
    frame::FlightFrame,
    situation::{self, AIM_MEMORY_S, AIM120_IGNORE_FT, AIR_RANGE_FT, HIT_HOLD_S},
    terrain::Terrain,
};
use tore_sim::combat::live;

pub struct Step {
    pub now: f64,
    pub inputs: situation::Inputs,
    /// Journal entries of this step: the inputs changed, and why each is
    /// on. The host hands them to the mission recording.
    pub journal: Vec<Entry>,
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
    /// Journal only: the last hit and the inputs last journaled. No rule
    /// reads them.
    hit_at: Option<f64>,
    seen: Option<Seen>,
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt()
}

impl Observer {
    pub fn new() -> Self {
        Self::default()
    }

    /// One step for the screen of the human the frame is for: its plane's
    /// damage, designated target, inbound missiles and the AI aircraft aimed
    /// at it, and the mission result and home latch, all from the frame's
    /// readout.
    pub fn step(&mut self, frame: &FlightFrame, events: &[live::Event], world: &Terrain) -> Step {
        let (plane, flight) = (frame.plane.0, frame.flight);
        let now = self.steps as f64 * flight::DT;
        self.steps += 1;
        let position = flight.position;

        if events.iter().any(
            |e| matches!(e, live::Event::OwnshipDamaged { aircraft, .. } if *aircraft == plane),
        ) {
            self.hit.refresh(now, HIT_HOLD_S);
            self.hit_at = Some(now);
        }

        // Designated target: live, the other side, and an aircraft, as the
        // readout decided.
        let music = &frame.readout.music;
        let designated_target = music
            .designated_enemy
            .map(|(id, target)| (id, distance(target, position)));
        let designated = designated_target.map(|(_, range)| range);
        let air_target = designated.is_some_and(|range| range < AIR_RANGE_FT);
        let far_target = designated.is_some_and(|range| range >= AIR_RANGE_FT);

        // `fitted`: TORE's AI does not keep a selected station, so "missile
        // selected" is an alive AI aircraft whose target is the player and
        // which still carries a usable guided air-to-air store. The 1 s
        // final-attack memory has no TORE equivalent; 4 s is always used.
        // Every such aircraft is listed for the journal.
        let aiming: Vec<u32> = music.aiming.clone();
        if !aiming.is_empty() {
            self.aim.refresh(now, AIM_MEMORY_S);
        }

        // Missiles guided at the player, counted every step; an AIM-120
        // farther than 30,380 ft is not counted.
        let inbound_ids: Vec<u32> = frame
            .readout
            .rwr
            .inbound
            .iter()
            .filter(|missile| {
                !(missile.aim120 && distance(missile.position, position) > AIM120_IGNORE_FT)
            })
            .map(|missile| missile.id)
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

        let inputs = situation::Inputs {
            succeeded: music.succeeded,
            ejected: flight.escape.is_some(),
            launching,
            air_target,
            hit_recently: self.hit.active(now),
            danger: far_target || self.aim.active(now) || inbound,
            home: music.home,
            deck,
        };
        let journal = self.journal(now, inputs, designated_target, aiming, inbound_ids);
        Step {
            now,
            inputs,
            journal,
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

    /// The frame parts a test's flight frame borrows.
    #[derive(Default)]
    struct Parts {
        picture: crate::snapshot::RenderSnapshot,
        smoke: tore_sim::combat::smoke::Smoke,
        devices: tore_sim::combat::countermeasures::Devices,
    }
    fn test_frame<'a>(
        parts: &'a Parts,
        flight: &'a flight::State,
        combat: &'a live::State,
    ) -> FlightFrame<'a> {
        FlightFrame {
            seat: crate::seats::SeatId(0),
            plane: crate::seats::PlaneId(0),
            flight,
            previous: flight,
            presented: std::borrow::Cow::Borrowed(flight),
            picture: &parts.picture,
            smoke: [&parts.smoke, &parts.smoke],
            devices: &parts.devices,
            config: combat.own().configuration(),
            readout: tore_world::frame::ReadoutSlot::ready(
                tore_world::readout::build(combat, 0, crate::combat::launcher(flight), None, None)
                    .unwrap(),
            ),
            tick_cues: &[],
        }
    }

    #[test]
    fn hits_and_guided_missiles_feed_air_and_danger() {
        let world = tore_world::test_support::terrain();
        let mut combat = tore_world::test_support::combat_fixture(true);
        let flight =
            flight::State::new(&tore_world::test_support::profile(), [0., 20_000., 0.]).unwrap();
        let mut observer = Observer::new();
        let parts = Parts::default();
        let step = |observer: &mut Observer, combat: &live::State, events: &[live::Event]| {
            observer.step(&test_frame(&parts, &flight, combat), events, &world)
        };
        let first = step(
            &mut observer,
            &combat,
            &[live::Event::OwnshipDamaged {
                aircraft: 0,
                amount: 3,
            }],
        );
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

        // A guided round aimed at the player selects DANGER.
        combat.command(0, live::Command::Incoming, crate::combat::launcher(&flight));
        assert_eq!(combat.projectiles[0].target, Some(0));
        assert!(step(&mut observer, &combat, &[]).inputs.danger);
        // An AIM-120 beyond 30,380 ft is not counted; inside, it is.
        let mut aim120 = combat.own().configuration().stations[0].weapon.clone();
        aim120.source = "AIM120.JT".into();
        combat.projectiles[0].weapon = Some(aim120);
        combat.projectiles[0].position = [30_381., 20_000., 0.];
        assert!(!step(&mut observer, &combat, &[]).inputs.danger);
        combat.projectiles[0].position = [30_379., 20_000., 0.];
        assert!(step(&mut observer, &combat, &[]).inputs.danger);
    }

    #[test]
    fn input_changes_are_journaled_with_the_reasons_behind_them() {
        let world = tore_world::test_support::terrain();
        let mut combat = tore_world::test_support::combat_fixture(true);
        let flight =
            flight::State::new(&tore_world::test_support::profile(), [0., 20_000., 0.]).unwrap();
        let mut observer = Observer::new();
        let parts = Parts::default();
        let step = |observer: &mut Observer, combat: &live::State, events: &[live::Event]| {
            observer.step(&test_frame(&parts, &flight, combat), events, &world)
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
            &[live::Event::OwnshipDamaged {
                aircraft: 0,
                amount: 3,
            }],
        ));
        assert_eq!(
            (hit.from, hit.to),
            (Some(situation::Rank::Normal), situation::Rank::Air)
        );
        assert_eq!(hit.hit_at, Some(2. * flight::DT));
        combat.command(0, live::Command::Incoming, crate::combat::launcher(&flight));
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
}
