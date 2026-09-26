//! The two journals as recorded events, from synthetic entries.
use super::super::Recorder;
use super::*;
use crate::comms::{
    Call, Kind, Phrase,
    journal::{Answer, Music, Origin, Outcome as Said, Reason, Reply, Roll, Store, Test},
};
use tore_replay::{AircraftInfo, Side};
use tore_sim::ai::{
    engagement::ThreatReport as AttackReport,
    thought::{IgnoreReason, JournalEntry, Receipt},
    threat::{DeviceRelease, ScriptReason, WarningOutcome, WarningReaction},
    wing::PlayerOrder,
};

fn roster() -> Vec<AircraftInfo> {
    [
        (0, "You"),
        (3, "Enemy 1-1"),
        (4, "Enemy 1-2"),
        (5, "Friendly 1-2"),
    ]
    .into_iter()
    .map(|(id, label)| AircraftInfo {
        id,
        label: label.into(),
        side: Side::Enemy,
        ..AircraftInfo::default()
    })
    .collect()
}

fn attack() -> ObservedAttack {
    ObservedAttack {
        report: AttackReport {
            attacker_id: Some(0),
            defended_id: 3,
        },
        bearing_world_deg: Some(90.),
        observed_tick: 10,
        event_id: Some(77),
    }
}

fn entry(tick: u64, sender: u32, message: Message, receipts: &[(u32, Outcome)]) -> JournalEntry {
    JournalEntry {
        tick,
        sender: Some(sender),
        message,
        receipts: receipts
            .iter()
            .map(|&(actor, outcome)| Receipt { actor, outcome })
            .collect(),
    }
}

#[test]
fn ai_messages_become_reports_requests_answers_and_defenses() {
    let (mut recorder, _receiver) = Recorder::detached(64, &roster());
    let applied = Outcome::Order(ReceiverOutcome::Applied(AppliedSetting::TargetOrder {
        deadline: None,
    }));
    let batch = JournalBatch {
        entries: vec![
            // Queued for two recipients, then delivered to one of them.
            entry(
                10,
                3,
                Message::AttackEvidence(attack()),
                &[(3, Outcome::Queued), (4, Outcome::Queued)],
            ),
            entry(
                11,
                3,
                Message::AttackEvidence(attack()),
                &[(4, Outcome::Delivered)],
            ),
            entry(
                11,
                3,
                Message::AttackEvidence(attack()),
                &[(
                    3,
                    Outcome::Ignored(IgnoreReason::SeenBeforeRecall { recalled_at: 5 }),
                )],
            ),
            // A wing order with two answers.
            entry(
                11,
                3,
                Message::WingRequest(Box::new(WingRequest::TargetAssignment(
                    TargetOrder::FreeSelection,
                ))),
                &[
                    (4, applied),
                    (
                        5,
                        Outcome::Order(ReceiverOutcome::Rejected(RejectReason::BuggedOut)),
                    ),
                ],
            ),
            // A launch warning that arrived.
            entry(
                12,
                0,
                Message::MissileWarning(tore_sim::ai::controller::ThreatReport {
                    missile_id: 77,
                    seeker: SeekerClass::Radar,
                    launcher_id: 0,
                    launcher_same_side: false,
                    distance_at_launch_ft: 30_000.,
                    launch_tick: 6,
                }),
                &[(
                    4,
                    Outcome::WarningReceived {
                        outcome: WarningOutcome {
                            approach_abandoned: false,
                            devices: Some(DeviceRelease {
                                count: 2,
                                class: SeekerClass::Radar,
                            }),
                            reaction: WarningReaction::Maneuver {
                                reason: ScriptReason::RadarLaunch,
                                wing_reaction: true,
                            },
                        },
                        launch_call: true,
                    },
                )],
            ),
        ],
        dropped: 0,
    };
    let mut events = Vec::new();
    recorder.ai_journal(&batch, &Frame::default(), &mut events);
    let kinds: Vec<&str> = events.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            kind::COMMS_REPORT,
            kind::COMMS_DELIVERY,
            kind::COMMS_DELIVERY,
            kind::COMMS_REQUEST,
            kind::COMMS_DELIVERY,
            kind::COMMS_DELIVERY,
            kind::AI_DEFENSE,
        ]
    );
    // The report and its answers share one message number.
    let report = &events[0];
    let message = report.num(field::MESSAGE).unwrap();
    assert_eq!(report.get(field::RECIPIENTS), Some(&Value::Ids(vec![3, 4])));
    assert_eq!(report.string(field::OUTCOME), Some(outcome::QUEUED));
    assert_eq!(
        report.text,
        "attack on Enemy 1-1 by You (shot 77), bearing 90"
    );
    assert_eq!(events[1].num(field::MESSAGE), Some(message));
    assert_eq!((events[1].subject, events[1].object), (Some(4), Some(3)));
    assert_eq!(events[1].string(field::OUTCOME), Some(outcome::DELIVERED));
    assert_eq!(events[2].string(field::OUTCOME), Some(outcome::IGNORED));
    assert!(
        events[2]
            .string(field::REASON)
            .unwrap()
            .contains("before the recall")
    );
    // A request to two, answered one by one with the reason.
    assert_eq!(events[3].string(field::ORDER), Some("free selection"));
    assert_eq!(events[3].string(field::OUTCOME), Some(outcome::ANSWERED));
    assert_eq!(events[4].string(field::OUTCOME), Some(outcome::APPLIED));
    assert_eq!(events[5].string(field::OUTCOME), Some(outcome::REJECTED));
    assert_eq!(
        events[5].string(field::REASON),
        Some("it bugged out and no longer answers")
    );
    // The warning as a defensive reaction, with its delay.
    let defense = &events[6];
    assert_eq!((defense.subject, defense.object), (Some(4), Some(0)));
    assert_eq!(defense.id(field::THREAT), Some(77));
    assert!(
        defense
            .string(field::REACTION)
            .unwrap()
            .contains("chaff x2 scheduled")
    );
    assert_eq!(defense.num(field::DELAY_S), Some(0.05));
    // What Enemy 1-2 heard explains its next decision change.
    assert_eq!(
        recorder.why.news.get(&4).map(Vec::len),
        Some(3),
        "{:?}",
        recorder.why.news
    );
}

fn call(label: &str, text: &str, origin: Origin) -> Call {
    Call::new(
        label,
        Phrase {
            text: text.into(),
            stems: vec!["FOX2".into()],
        },
        Kind::Chatter,
    )
    .because(origin)
}

#[test]
fn comms_entries_become_radio_order_hud_and_music_events() {
    let (mut recorder, _receiver) = Recorder::detached(64, &roster());
    let release = Origin::of(
        Source::Radio,
        Cause::Release {
            target: Some(3),
            store: Store {
                flags: 1,
                seeker: 2,
                phoenix: false,
            },
        },
    )
    .by(5)
    .to(Audience::Flight)
    .rolls(vec![Roll::new("Fox call", 37, Test::Below(50))]);
    let fox = call("Blue two", "Fox two", release.clone());
    let order = Origin::of(
        Source::Order,
        Cause::Order {
            order: PlayerOrder::EngageMyTarget,
            selected: Some(3),
            target: Some(3),
        },
    )
    .by(0);
    let hud = Origin::of(
        Source::Hud,
        Cause::Activity {
            activity: tore_sim::ai::controller::Activity::Attacking,
        },
    )
    .by(5);
    let entries = vec![
        talk::Entry::call(
            1.,
            Some(7),
            &fox,
            Said::Queued {
                due: 1.4,
                expires: None,
            },
        ),
        talk::Entry::call(1.4, Some(7), &fox, Said::Delivered { waited: 0.4 }),
        talk::Entry::call(
            1.5,
            Some(8),
            &call("Green one", "Fox one", release),
            Said::Unheard(Reason::EnemyFlight),
        ),
        talk::Entry::note(
            2.,
            "YOU",
            order,
            Said::Answered {
                answers: vec![
                    Answer {
                        recipient: 5,
                        member: 1,
                        result: Answered::Receiver(ReceiverOutcome::AppliedNoMotion),
                        side: Vec::new(),
                    },
                    Answer {
                        recipient: 4,
                        member: 2,
                        result: Answered::CannotSeeTarget,
                        side: Vec::new(),
                    },
                ],
                reply: Reply::NotExpected,
            },
        )
        .with_text("Engage: 1 applied, 1 rejected"),
        // A HUD line the HUD then showed is recorded by the HUD itself.
        talk::Entry::note(
            2.5,
            "Friendly 1-2",
            hud.clone(),
            Said::Delivered { waited: 0. },
        )
        .with_text("Attacking"),
        talk::Entry::note(
            2.5,
            "Friendly 1-2",
            hud,
            Said::Suppressed(Reason::RateLimited {
                interval_s: 2.,
                remaining: 1.5,
            }),
        )
        .with_text("Attacking"),
        talk::Entry::note(
            3.,
            "Music",
            Origin::of(
                Source::Music,
                Cause::Music(Box::new(Music {
                    from: Some(crate::audio::situation::Rank::Normal),
                    to: crate::audio::situation::Rank::Air,
                    inputs: crate::audio::situation::Inputs {
                        air_target: true,
                        home: true,
                        ..Default::default()
                    },
                    designated: Some((3, 20_000.)),
                    aiming: Vec::new(),
                    inbound: Vec::new(),
                    hit_at: None,
                })),
            ),
            Said::Noted,
        ),
    ];
    recorder.comms(entries);
    let events = &recorder.early;
    let kinds: Vec<&str> = events.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            kind::COMMS_RADIO,
            kind::COMMS_RADIO,
            kind::COMMS_RADIO,
            kind::COMMS_ORDER,
            kind::COMMS_DELIVERY,
            kind::COMMS_DELIVERY,
            kind::COMMS_HUD,
            kind::AUDIO_MUSIC,
        ]
    );
    let queued = &events[0];
    assert_eq!(queued.string(field::OUTCOME), Some(outcome::QUEUED));
    assert_eq!(queued.num(field::DUE_S), Some(0.4));
    assert_eq!(queued.flag(field::HEARD), None, "not decided yet");
    let heard = &events[1];
    assert_eq!(heard.subject, Some(5));
    assert_eq!(heard.num(field::MESSAGE), Some(7.));
    assert_eq!(heard.flag(field::HEARD), Some(true));
    assert_eq!(heard.num(field::WAIT_S), Some(0.4));
    assert_eq!(heard.string(field::ROLLS), Some("roll 37 < 50: Fox call"));
    assert_eq!(
        heard.string(field::TRIGGER),
        Some("infrared missile release at aircraft 3")
    );
    assert_eq!(heard.string(field::AUDIENCE), Some("the flight"));
    assert_eq!(heard.text, "Fox two");
    let unheard = &events[2];
    assert_eq!(unheard.string(field::OUTCOME), Some(outcome::UNHEARD));
    assert_eq!(unheard.flag(field::HEARD), Some(false));
    assert!(unheard.string(field::REASON).is_some());
    // The order and one answer per addressed wingman.
    let order = &events[3];
    assert_eq!(order.string(field::ORDER), Some("EngageMyTarget"));
    assert_eq!(order.get(field::RECIPIENTS), Some(&Value::Ids(vec![5, 4])));
    assert_eq!(order.object, Some(3));
    let message = order.num(field::MESSAGE);
    assert!(
        events[4..6]
            .iter()
            .all(|e| e.num(field::MESSAGE) == message)
    );
    assert_eq!(events[4].string(field::OUTCOME), Some(outcome::APPLIED));
    assert_eq!(events[5].string(field::OUTCOME), Some(outcome::REJECTED));
    assert_eq!(
        events[5].string(field::REASON),
        Some("its sensors cannot see the target")
    );
    assert_eq!(events[6].string(field::OUTCOME), Some(outcome::SUPPRESSED));
    let music = &events[7];
    assert_eq!(
        (music.string(field::FROM), music.string(field::TO)),
        (Some("normal"), Some("air"))
    );
    assert!(
        music
            .string(field::REASON)
            .unwrap()
            .starts_with("inputs ask for Air")
    );
    // Every input, so a replay can hand the music the same ones.
    let on: Vec<&str> = vocab::music::ALL
        .into_iter()
        .filter(|name| music.flag(name).expect("every input is recorded"))
        .collect();
    assert_eq!(on, [vocab::music::AIR_TARGET, vocab::music::HOME]);
}

#[test]
fn tower_entries_a_replay_acts_on_have_fixed_triggers() {
    let (mut recorder, _receiver) = Recorder::detached(64, &roster());
    recorder.comms(vec![
        talk::Entry::tower_reply(1., "Cleared to land", Some("^CLRLAND")),
        talk::Entry::clearance_cancelled(2., "Landing clearance cancelled: runway unavailable"),
    ]);
    let [reply, cut] = &recorder.early[..] else {
        panic!("{:?}", recorder.early);
    };
    assert_eq!(reply.kind, kind::COMMS_TOWER);
    assert_eq!(
        reply.string(field::TRIGGER),
        Some(vocab::trigger::PLAYER_REQUEST)
    );
    assert_eq!(reply.string(field::ROUTE), Some(vocab::route::TOWER));
    assert_eq!(reply.string(field::STEMS), Some("^CLRLAND"));
    assert!(vocab::heard(reply));
    // Nothing was said: the tower speech playing was cut.
    assert_eq!(cut.kind, kind::COMMS_TOWER);
    assert_eq!(
        cut.string(field::TRIGGER),
        Some(vocab::trigger::CLEARANCE_CANCELLED)
    );
    assert_eq!(cut.string(field::OUTCOME), Some(outcome::CANCELLED));
    assert_eq!(cut.string(field::REASON), Some("the runway is unusable"));
    assert_eq!(cut.flag(field::HEARD), Some(false));
    assert!(!vocab::heard(cut));
}

#[test]
fn a_line_through_the_channel_is_heard_once_at_its_delivery() {
    let (mut recorder, _receiver) = Recorder::detached(64, &roster());
    let mut comms = crate::comms::Comms::new(1);
    let origin = Origin::of(Source::Radio, Cause::Unspecified).by(5);
    comms.send(1., call("Blue two", "Fox two", origin));
    assert_eq!(comms.due(1.).len(), 1);
    // The player's order voice then cut it off in the mixer.
    comms.cut_off(1.5, Reason::OrderVoice);
    recorder.drain_comms(&mut comms);
    let outcomes: Vec<Option<&str>> = recorder
        .early
        .iter()
        .map(|e| e.string(field::OUTCOME))
        .collect();
    assert_eq!(
        outcomes,
        [
            Some(outcome::QUEUED),
            Some(outcome::DELIVERED),
            Some(outcome::INTERRUPTED)
        ]
    );
    let heard: Vec<&Event> = recorder.early.iter().filter(|e| vocab::heard(e)).collect();
    assert_eq!(heard.len(), 1, "{:?}", recorder.early);
    assert_eq!(heard[0].string(field::OUTCOME), Some(outcome::DELIVERED));
    assert!(
        recorder
            .early
            .iter()
            .all(|e| e.num(field::MESSAGE) == heard[0].num(field::MESSAGE)),
        "every entry names the same line"
    );
}
