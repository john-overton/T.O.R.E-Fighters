//! The coders of a cockpit's crew voice (docs/formats/checkpoint.md).
//!
//! Stage H slice H7. [`CrewVoice`](super::CrewVoice) sits in a cockpit, so
//! the cockpits section (slice H2) calls its coder. Skipped:
//!
//! - `CrewVoice::gate`: why-record (the comment gate last journaled; no rule
//!   reads it, it only decides whether the next change is journaled). A
//!   restored voice journals its first gate again.

use super::{CrewVoice, Situation};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

impl Checkpoint for Situation {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let (number, ahead) = match self {
            Situation::Clear => (0, None),
            Situation::Silent => (1, None),
            Situation::Surface { ahead } => (2, Some(*ahead)),
            Situation::HeadOn => (3, None),
            Situation::Offensive => (4, None),
            Situation::Defensive => (5, None),
            Situation::Neutral => (6, None),
        };
        s.writer().write_varint(number);
        if let Some(ahead) = ahead {
            ahead.save(s, None)?;
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Situation::Clear,
            1 => Situation::Silent,
            2 => Situation::Surface {
                ahead: Checkpoint::load(l, None)?,
            },
            3 => Situation::HeadOn,
            4 => Situation::Offensive,
            5 => Situation::Defensive,
            6 => Situation::Neutral,
            other => return invalid(format!("Situation has no variant {other}")),
        })
    }
}

tore_sim::checkpoint_struct!(CrewVoice {
    seat,
    plane,
    crew,
    fighter,
    next,
    last_situation,
    last_nm,
    last_g,
    crossings,
    crossing_minute,
    wet,
    fuel_said,
    warned,
    crashed,
    home,
} skip {
    // Why-record: the comment gate last journaled.
    gate = None,
});

#[cfg(test)]
mod tests {
    use super::super::{Body, Incoming, Input, Target, TargetKind};
    use super::*;
    use crate::comms::{Comms, Crew, Phrases, Route};
    use crate::seats::SeatId;
    use tore_sim::ai::route::FuelState;
    use tore_sim::attitude::{Basis, Vector};
    use tore_sim::checkpoint::{Coded, Models, from_bytes, to_bytes};

    fn models() -> Models {
        Models::default()
    }

    fn coded(voice: &CrewVoice) -> Coded {
        to_bytes(voice, &models()).expect("it codes")
    }

    fn restored(voice: &CrewVoice) -> CrewVoice {
        from_bytes(&coded(voice), &models()).expect("it decodes")
    }

    /// A voice with every field away from its start value.
    fn lived_in() -> CrewVoice {
        let mut voice = CrewVoice::with(Some(Crew::Rio), true).for_seat(SeatId(1), 9);
        voice.next = 123.456;
        voice.last_situation = Some(Situation::Surface { ahead: true });
        voice.last_nm = Some(14);
        voice.last_g = Some(-3);
        voice.crossings = 4;
        voice.crossing_minute = 17;
        voice.wet = Some(false);
        voice.fuel_said = 2;
        voice.warned.extend([4, 9, 200]);
        voice.crashed = false;
        voice.home = Some([1.5, -2.25, f64::MIN_POSITIVE]);
        voice
    }

    #[test]
    fn a_lived_in_crew_voice_round_trips_equal_but_for_its_gate() {
        let mut voice = lived_in();
        voice.gate = Some(None);
        let copy = restored(&voice);
        assert_eq!(copy.gate, None, "the gate is a why-record");
        voice.gate = None;
        assert_eq!(copy, voice);
        assert_eq!(coded(&copy), coded(&voice));
    }

    #[test]
    fn a_fresh_voice_keeps_its_infinite_wait_and_its_sentinel_minute() {
        for crew in [None, Some(Crew::Rio), Some(Crew::CoPilot)] {
            let voice = CrewVoice::with(crew, crew.is_none());
            let copy = restored(&voice);
            assert_eq!(copy, voice);
            assert_eq!(copy.next, f64::NEG_INFINITY);
            assert_eq!(copy.crossing_minute, i64::MIN);
        }
    }

    #[test]
    fn every_situation_round_trips() {
        for situation in [
            Situation::Clear,
            Situation::Silent,
            Situation::Surface { ahead: true },
            Situation::Surface { ahead: false },
            Situation::HeadOn,
            Situation::Offensive,
            Situation::Defensive,
            Situation::Neutral,
        ] {
            let mut voice = lived_in();
            voice.last_situation = Some(situation);
            assert_eq!(restored(&voice).last_situation, Some(situation));
        }
    }

    fn body(position: Vector, heading_deg: f64) -> Body {
        Body {
            position,
            basis: Basis::new(heading_deg.to_radians(), 0., 0.),
            speed: 600.,
        }
    }

    /// What the crew notices at `tick` of a scripted flight: a target that
    /// comes, turns and goes, G swings, water below, fuel running down and a
    /// missile at the end.
    fn script(tick: u32) -> Input {
        let now = 1. + f64::from(tick) / 120.;
        let seconds = f64::from(tick) / 120.;
        let north = 40_000. - 4_000. * seconds;
        let target_heading = (seconds * 25.) % 360.;
        let target = (tick % 1_500 > 200).then_some(Target {
            id: 7,
            kind: TargetKind::Aircraft {
                flying: true,
                ace: false,
            },
            body: body([3_000., 10_000., north], target_heading),
        });
        Input {
            now,
            clock_minute: (seconds / 60.) as i64,
            crashed: tick > 2_700,
            ejected: false,
            pilot_dead: false,
            free_flight: true,
            doomed: false,
            g: 1. + 6. * (seconds / 3.).sin().abs(),
            own: body([0., 10_000., 0.], 0.),
            designated: target.map(|t| t.id),
            target,
            gun_selected: tick % 700 > 350,
            missile_would_lock: tick % 500 > 250,
            corner_speed: 700.,
            over_water: Some((tick / 600).is_multiple_of(2)),
            fuel: match tick / 700 {
                0 => FuelState::Ok,
                1 => FuelState::Caution,
                2 => FuelState::Bingo,
                _ => FuelState::Critical,
            },
            incoming: if tick > 1_900 {
                vec![Incoming {
                    id: 4,
                    age: 1.2,
                    signature: 2,
                }]
            } else {
                Vec::new()
            },
            wingman: None,
        }
    }

    type Lines = Vec<(SeatId, String, Route)>;

    /// The deliveries of one tick, without their origins, which the step
    /// makes anew.
    fn step(voice: &mut CrewVoice, comms: &mut Comms, tick: u32) -> Lines {
        let input = script(tick);
        voice.step(&input, comms, &Phrases::new());
        comms
            .due(input.now)
            .into_iter()
            .map(|d| (d.seat, d.call.line(), d.call.route))
            .collect()
    }

    #[test]
    fn a_restored_voice_speaks_on_identically() {
        let mut voice = CrewVoice::with(Some(Crew::Rio), true).for_seat(SeatId(1), 9);
        let mut comms = Comms::with_seats(5, [SeatId(1)]);
        let mut spoken = 0;
        let mut seen: Vec<(u32, Lines)> = Vec::new();
        for tick in 0..1_000 {
            spoken += step(&mut voice, &mut comms, tick).len();
        }
        assert!(
            spoken >= 1,
            "the crew spoke before the checkpoint: {spoken}"
        );
        // Restore the voice and its channel; both copies then run on alike.
        let mut copy = restored(&voice);
        let mut copy_comms: Comms =
            from_bytes(&to_bytes(&comms, &models()).unwrap(), &models()).unwrap();
        for tick in 1_000..3_000 {
            let (a, b) = (
                step(&mut voice, &mut comms, tick),
                step(&mut copy, &mut copy_comms, tick),
            );
            assert_eq!(a, b, "the crew's lines agree at tick {tick}");
            if !a.is_empty() {
                seen.push((tick, a));
            }
        }
        assert!(
            seen.len() > 3,
            "the crew spoke after the checkpoint: {}",
            seen.len()
        );
        assert_eq!(coded(&copy).body.len(), coded(&voice).body.len());
        assert_eq!(coded(&copy), coded(&voice));
    }

    #[test]
    fn damaged_voices_are_refused_without_a_panic() {
        let good = coded(&lived_in());
        for cut in 0..good.body.len() {
            let damaged = Coded {
                body: good.body[..cut].to_vec(),
                records: good.records.clone(),
            };
            assert!(from_bytes::<CrewVoice>(&damaged, &models()).is_err());
        }
        for at in 0..good.body.len() {
            let mut body = good.body.clone();
            body[at] ^= 0xff;
            let _ = from_bytes::<CrewVoice>(
                &Coded {
                    body,
                    records: Vec::new(),
                },
                &models(),
            );
        }
    }
}
