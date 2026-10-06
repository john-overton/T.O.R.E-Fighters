//! The coders of the radio channels (the comms section): channels, pending
//! calls, cooldowns and the radio random stream (docs/formats/checkpoint.md).
//!
//! Stage H slice H7. Skipped, each with its class:
//!
//! - `Comms::journal`: why-record (the radio journal; the simulation never
//!   reads it, and `first_in_window` only decides whether to write a line).
//! - `Channel::recent`: why-record (the lines `cut_off` lists as
//!   interrupted, journal only).
//! - `Call::origin`: why-record (who made the call and why; delivery never
//!   reads it).
//!
//! The cooldown keys are `&'static str`, so they are coded as an index into
//! [`COOLDOWN_KEYS`] and an unknown key is an error, never a silent skip.

use super::{Call, Channel, Comms, Kind, Net, Pending, Route};
use crate::comms::journal::{Journal, Origin};
use crate::seats::SeatId;
use std::collections::{BTreeMap, VecDeque};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

/// Every cooldown key a rule gives [`Comms::cooldown`] or
/// [`Comms::seat_cooldown`], in coding order. Append new keys at the end: a
/// key not listed here makes a checkpoint fail with a named error, and a test
/// reads the rule files to catch one that is not.
pub(crate) const COOLDOWN_KEYS: [&str; 7] = [
    "radio-bombs",
    "radio-gun",
    "radio-unguided-hit",
    "radio-other-kill",
    "radio-friendly-fire",
    "crew-infrared-warning",
    "crew-radar-warning",
];

fn save_key(s: &mut Saver, key: &str) -> Result<(), CheckpointError> {
    let Some(index) = COOLDOWN_KEYS.iter().position(|known| *known == key) else {
        return invalid(format!(
            "the cooldown key {key:?} is not in the checkpoint's key table"
        ));
    };
    s.writer().write_varint(index as u64);
    Ok(())
}

fn load_key(l: &mut Loader<'_>) -> Result<&'static str, CheckpointError> {
    let index = l.reader().read_varint()?;
    match usize::try_from(index)
        .ok()
        .and_then(|i| COOLDOWN_KEYS.get(i))
    {
        Some(key) => Ok(key),
        None => invalid(format!("the cooldown key {index} is not in the key table")),
    }
}

tore_sim::checkpoint_enum!(Kind {
    Chatter = 0,
    Important = 1,
});

tore_sim::checkpoint_enum!(Route {
    Radio = 0,
    Airport = 1,
    Direct = 2,
});

tore_sim::checkpoint_enum!(Net {
    Wing = 0,
    Battle = 1,
});

tore_sim::checkpoint_struct!(Call {
    label,
    text,
    stems,
    kind,
    route,
    delay,
    net,
} skip {
    // Why-record: who made the call and why. Delivery never reads it.
    origin = Origin::default(),
});

tore_sim::checkpoint_struct!(Pending {
    due,
    sent,
    serial,
    call,
});

tore_sim::checkpoint_struct!(Channel {
    seat,
    radio_silence,
    battle,
    busy_until,
    pending,
} skip {
    // Why-record: the lines `cut_off` journals as interrupted.
    recent = VecDeque::new(),
});

impl Checkpoint for Comms {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let Comms {
            channels,
            cooldowns,
            seat_cooldowns,
            rng,
            serial,
            clock,
            // Why-record: the radio journal.
            journal: _,
        } = self;
        channels.save(s, None)?;
        s.count(cooldowns.len());
        for (key, until) in cooldowns {
            save_key(s, key)?;
            until.save(s, None)?;
        }
        s.count(seat_cooldowns.len());
        for ((seat, key), until) in seat_cooldowns {
            seat.save(s, None)?;
            save_key(s, key)?;
            until.save(s, None)?;
        }
        rng.save(s, None)?;
        serial.save(s, None)?;
        clock.save(s, None)?;
        Ok(())
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let channels: Vec<Channel> = Checkpoint::load(l, None)?;
        // `set_seats` keeps the channels in seat order, one for each seat;
        // a body that says otherwise was not written by this coder.
        if channels.windows(2).any(|pair| pair[0].seat >= pair[1].seat) {
            return invalid("the radio channels are not in strict seat order");
        }
        let mut cooldowns = BTreeMap::new();
        for _ in 0..l.count()? {
            let key = load_key(l)?;
            cooldowns.insert(key, f64::load(l, None)?);
        }
        let mut seat_cooldowns = BTreeMap::new();
        for _ in 0..l.count()? {
            let seat = SeatId::load(l, None)?;
            let key = load_key(l)?;
            seat_cooldowns.insert((seat, key), f64::load(l, None)?);
        }
        Ok(Comms {
            channels,
            cooldowns,
            seat_cooldowns,
            rng: Checkpoint::load(l, None)?,
            serial: Checkpoint::load(l, None)?,
            clock: Checkpoint::load(l, None)?,
            journal: Journal::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comms::{Hearer, Phrase};
    use tore_sim::checkpoint::{Coded, Models, from_bytes, to_bytes};

    const S0: SeatId = SeatId(0);
    const S1: SeatId = SeatId(1);

    fn models() -> Models {
        Models::default()
    }

    fn coded(comms: &Comms) -> Coded {
        to_bytes(comms, &models()).expect("a radio codes")
    }

    fn restored(comms: &Comms) -> Comms {
        from_bytes(&coded(comms), &models()).expect("a radio decodes")
    }

    fn say(label: &str, text: &str, kind: Kind) -> Call {
        Call::new(label, Phrase::default().raw(text, Some("^CONTACT")), kind)
    }

    /// Two seats, calls pending for each with delays, one radio silent, both
    /// kinds of cooldown running, calls delivered so the busy hold, the
    /// serial and the random stream have moved.
    fn busy_radio() -> Comms {
        let mut c = Comms::with_seats(0x9e37_79b9_7f4a_7c15, [S0, S1]);
        c.toggle_silence(S1);
        assert!(c.cooldown("radio-gun", 0., 4.));
        assert!(c.cooldown("radio-other-kill", 1., 4.));
        assert!(c.seat_cooldown(S0, "crew-infrared-warning", 1., 6.));
        assert!(c.seat_cooldown(S1, "crew-radar-warning", 2., 6.));
        c.send(
            1.,
            say("Red two", "Splash one", Kind::Important).after(0.5),
            &[Hearer::seat(S0), Hearer::named(S1, "Red leader")],
        );
        c.send(
            1.5,
            say("Red three", "Hit", Kind::Chatter).after(2.).airport(),
            &[Hearer::seat(S0), Hearer::seat(S1)],
        );
        c.send(
            2.,
            say("Blue", "Fox two", Kind::Important).after(5.).direct(),
            &[Hearer::seat(S1).saying(Phrase::default().raw("Fox three", None))],
        );
        // One delivery: the first call is due, the busy hold moves.
        assert_eq!(c.due(1.5).len(), 2);
        c.roll();
        c
    }

    #[test]
    fn a_radio_with_calls_pending_round_trips_and_delivers_on_identically() {
        let c = busy_radio();
        assert!(
            c.channels.iter().any(|ch| !ch.pending.is_empty()),
            "calls are waiting"
        );
        let mut copy = restored(&c);
        assert_eq!(coded(&copy), coded(&c), "a restored radio codes the same");
        let mut original = c;
        // Both radios, driven alike for ten seconds of 120 Hz ticks.
        let mut delivered = 0;
        for tick in 0..1200u32 {
            let now = 2. + f64::from(tick) / 120.;
            if tick % 97 == 0 {
                for radio in [&mut original, &mut copy] {
                    radio.send(
                        now,
                        say("Red two", "Contact", Kind::Chatter),
                        &[Hearer::seat(S0), Hearer::seat(S1)],
                    );
                    assert!(radio.roll() < 100);
                }
            }
            let (a, b) = (original.due(now), copy.due(now));
            assert_eq!(a, b, "the deliveries agree at tick {tick}");
            delivered += a.len();
            assert_eq!(
                original.cooling("radio-gun", now),
                copy.cooling("radio-gun", now)
            );
            assert_eq!(
                original.seat_remaining(S0, "crew-infrared-warning", now),
                copy.seat_remaining(S0, "crew-infrared-warning", now)
            );
            assert_eq!(original.channel_free(S1, now), copy.channel_free(S1, now));
        }
        assert!(delivered > 5, "the run delivered calls");
        assert_eq!(coded(&original), coded(&copy));
    }

    #[test]
    fn the_journal_and_the_call_origins_are_left_out() {
        let c = busy_radio();
        let copy = restored(&c);
        assert!(
            !c.journal().is_empty(),
            "the original has journal entries to leave out"
        );
        assert!(copy.journal().is_empty());
        let original = c.channels[0].pending.first().map(|p| {
            let mut call = p.call.clone();
            call.origin = Origin::default();
            call
        });
        assert!(original.is_some(), "seat 0 has a call waiting");
        assert_eq!(
            original,
            copy.channels[0].pending.first().map(|p| p.call.clone()),
            "a pending call equals the original but for its origin"
        );
    }

    #[test]
    fn the_battle_net_monitor_and_a_pending_calls_net_round_trip() {
        let mut c = Comms::with_seats(1, [S0, S1]);
        assert_eq!(c.toggle_battle(S1), "Monitoring battle net");
        c.send(
            0.,
            say("Blue one", "Contact", Kind::Chatter).after(2.),
            &[
                Hearer::seat(S0),
                Hearer::named(S1, "Net Blue one").on(Net::Battle),
            ],
        );
        let mut copy = restored(&c);
        assert_eq!(coded(&copy), coded(&c), "a restored radio codes the same");
        assert!(copy.monitors_battle(S1) && !copy.monitors_battle(S0));
        let due = copy.due(2.);
        assert_eq!(
            due.iter().map(|d| (d.seat, d.call.net)).collect::<Vec<_>>(),
            [(S0, Net::Wing), (S1, Net::Battle)]
        );
        assert_eq!(due, c.due(2.), "both deliver the same");
        // The monitor and the net change the coding: they are state.
        let mut other = Comms::with_seats(1, [S0, S1]);
        other.send(
            0.,
            say("Blue one", "Contact", Kind::Chatter).after(2.),
            &[Hearer::seat(S0), Hearer::named(S1, "Net Blue one")],
        );
        assert_ne!(coded(&other), coded(&Comms::with_seats(1, [S0, S1])));
        let wing_only = coded(&other);
        other.toggle_battle(S1);
        assert_ne!(coded(&other), wing_only, "the monitor is coded");
        // The same radio but for the call's net differs as well.
        assert_ne!(coded(&other), coded(&c), "a pending call's net is coded");
    }

    #[test]
    fn a_fresh_radio_and_one_with_no_seats_round_trip() {
        for c in [Comms::new(7), Comms::with_seats(3, [])] {
            let copy = restored(&c);
            assert_eq!(coded(&copy), coded(&c));
            assert_eq!(copy.seats().count(), c.seats().count());
        }
    }

    #[test]
    fn a_never_busy_channel_keeps_its_infinite_hold() {
        let copy = restored(&Comms::new(1));
        assert!(copy.channel_free(S0, f64::MIN));
        assert_eq!(copy.channels[0].busy_until, f64::NEG_INFINITY);
    }

    #[test]
    fn every_known_key_round_trips_and_an_unknown_one_is_an_error() {
        let mut c = Comms::with_seats(5, [S0]);
        for (n, key) in COOLDOWN_KEYS.iter().enumerate() {
            assert!(c.cooldown(key, 0., 1. + n as f64));
            assert!(c.seat_cooldown(S0, key, 0., 10. + n as f64));
        }
        let copy = restored(&c);
        assert_eq!(coded(&copy), coded(&c));
        for (n, key) in COOLDOWN_KEYS.iter().enumerate() {
            assert_eq!(copy.remaining(key, 0.), 1. + n as f64);
            assert_eq!(copy.seat_remaining(S0, key, 0.), 10. + n as f64);
        }
        c.cooldown("not-a-rule", 0., 1.);
        let error = to_bytes(&c, &models()).expect_err("an unknown key");
        assert!(error.to_string().contains("not-a-rule"), "{error}");
        let mut c = Comms::with_seats(5, [S0]);
        c.seat_cooldown(S0, "also-unknown", 0., 1.);
        assert!(to_bytes(&c, &models()).is_err());
    }

    /// Every cooldown key the rules use is in the table: the keys are string
    /// literals in `radio_calls.rs` and `crew_voice.rs`, named `radio-` and
    /// `crew-`.
    #[test]
    fn every_cooldown_key_the_rules_use_is_in_the_table() {
        let mut found = Vec::new();
        for source in [
            include_str!("radio_calls.rs"),
            include_str!("crew_voice.rs"),
        ] {
            // The rules come before their tests; keys in tests are not rules.
            let rules = source.split("\n#[cfg(test)]\nmod tests").next().unwrap();
            for part in rules.split('"').skip(1).step_by(2) {
                if (part.starts_with("radio-") || part.starts_with("crew-"))
                    && part.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
                {
                    found.push(part);
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        assert!(!found.is_empty(), "the rule files name their keys");
        for key in &found {
            assert!(
                COOLDOWN_KEYS.contains(key),
                "the cooldown key {key:?} is used by a rule but not in COOLDOWN_KEYS"
            );
        }
        for key in COOLDOWN_KEYS {
            assert!(found.contains(&key), "{key:?} is in the table but unused");
        }
    }

    #[test]
    fn damaged_radios_are_refused_without_a_panic() {
        let c = busy_radio();
        let good = coded(&c);
        // Every truncation, and every single-byte flip.
        for cut in 0..good.body.len() {
            let damaged = Coded {
                body: good.body[..cut].to_vec(),
                records: good.records.clone(),
            };
            assert!(from_bytes::<Comms>(&damaged, &models()).is_err());
        }
        for at in 0..good.body.len() {
            for flip in [0x01, 0x80, 0xff] {
                let mut body = good.body.clone();
                body[at] ^= flip;
                // Either refused or a different valid radio, never a panic.
                let _ = from_bytes::<Comms>(
                    &Coded {
                        body,
                        records: good.records.clone(),
                    },
                    &models(),
                );
            }
        }
    }

    #[test]
    fn channels_out_of_seat_order_are_refused() {
        let mut c = Comms::with_seats(1, [S0, S1]);
        c.channels.reverse();
        let coded = to_bytes(&c, &models()).unwrap();
        assert!(from_bytes::<Comms>(&coded, &models()).is_err());
    }
}
