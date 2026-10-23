//! End-to-end reliable feedback, authoritative recording, and disabled gate.
use super::*;
use tore_replay::feedback::Feedback;
#[test]
fn feedback_reaches_host_log_recording_and_sender_with_host_identity() {
    let folder = std::env::temp_dir().join(format!("tore-host-feedback-{}", std::process::id()));
    let mut rig = Rig::new(
        spec(1, 0, 20),
        HostConfig {
            feedback: true,
            listable: false,
            recording: Some(crate::recording::Options {
                folder: folder.clone(),
                kept: 0,
            }),
            ..config()
        },
        LinkConfig::one_way(MS),
    );
    let client = rig.join(|c| c.callsign = "Viper".into());
    assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(client)));
    rig.run(Duration::from_millis(200));
    assert!(rig.host.lobby_state(0).feedback);
    let at = rig.host.world().combat.render_snapshot().tick;
    rig.clients[client].send(&Message::FeedbackSend(Feedback {
        tick: at,
        opened_at: 10,
        submitted_at: 20,
        player: "spoof".into(),
        seat: 99,
        plane: 99,
        aircraft: "wrong".into(),
        text: "The runway bounced. é".into(),
        ..Default::default()
    }));
    rig.run(Duration::from_millis(500));
    let f = rig.clients[client]
        .feedback
        .first()
        .expect("reliable reply")
        .clone();
    assert_eq!(f.player, "Viper");
    assert_eq!(f.aircraft, "F18.PT");
    assert_eq!(f.seat, 0);
    assert_eq!(f.tick, at);
    assert!(f.submit_tick >= at);
    assert!(
        rig.logs
            .iter()
            .any(|e| matches!(e,HostLog::Feedback(g) if g==&f))
    );
    rig.host.stop();
    let paths: Vec<_> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "tore-replay"))
        .collect();
    assert_eq!(paths.len(), 1, "flagged survives kept=0");
    let r = tore_replay::Recording::open(&paths[0]).unwrap();
    assert!(r.complete());
    assert!(r.problems().is_empty(), "{:?}", r.problems());
    assert_eq!(r.feedback().collect::<Vec<_>>(), [f]);
    assert!(r.frame_count() > 20);
    assert!(
        r.frames(r.first_tick().unwrap(), r.last_tick().unwrap())
            .all(|f| f.is_ok())
    );
    assert!(
        std::fs::read_to_string(paths[0].with_extension("log"))
            .unwrap()
            .contains("The runway bounced")
    );
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn feedback_disabled_ignores_forged_requests_and_recording_is_off() {
    let mut rig = Rig::new(spec(1, 0, 20), config(), LinkConfig::one_way(MS));
    let client = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(client)));
    rig.clients[client].send(&Message::FeedbackSend(Feedback {
        text: "ignored".into(),
        ..Default::default()
    }));
    rig.run(Duration::from_millis(500));
    assert!(!rig.host.lobby_state(0).feedback);
    assert!(rig.host.recording.is_none());
    assert!(!rig.logs.iter().any(|e| matches!(e, HostLog::Feedback(_))));
    assert!(rig.clients[client].feedback.is_empty());
}

#[test]
fn feedback_recording_continues_without_cockpits_and_rotates_each_mission() {
    let folder = std::env::temp_dir().join(format!("tore-empty-feedback-{}", std::process::id()));
    let mut rig = Rig::new(
        spec(2, 0, 20),
        HostConfig {
            start: StartMode::Now,
            recording: Some(crate::recording::Options {
                folder: folder.clone(),
                kept: 10,
            }),
            ..config()
        },
        LinkConfig::one_way(MS),
    );
    rig.run(Duration::from_millis(300));
    let first = rig.host.world().tick();
    assert!(rig.host.world().cockpits.is_empty());
    rig.host.restart();
    rig.run(Duration::from_millis(300));
    rig.host.stop();
    let paths: Vec<_> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "tore-replay"))
        .collect();
    assert_eq!(paths.len(), 2);
    for path in paths {
        let r = tore_replay::Recording::open(path).unwrap();
        assert!(r.complete());
        assert!(r.frame_count() >= first - 2);
        assert_eq!(r.aircraft().count(), 2);
        assert!(
            r.frames(r.first_tick().unwrap(), r.last_tick().unwrap())
                .all(|f| f.unwrap().aircraft.len() == 2)
        );
    }
    assert!(
        !rig.logs
            .iter()
            .any(|l| matches!(l,HostLog::Note{text,..} if text.contains("Recording failed")))
    );
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn recording_finishes_and_joins_on_migration_step_down_and_host_drop() {
    for how in ["migration", "drop"] {
        let folder =
            std::env::temp_dir().join(format!("tore-recording-{how}-{}", std::process::id()));
        let mut rig = Rig::new(
            spec(2, 0, 20),
            HostConfig {
                start: StartMode::Now,
                recording: Some(crate::recording::Options {
                    folder: folder.clone(),
                    kept: 10,
                }),
                ..config()
            },
            LinkConfig::one_way(MS),
        );
        rig.run(Duration::from_millis(300));
        if how == "migration" {
            rig.host.step_down();
        }
        drop(rig);
        let paths: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "tore-replay"))
            .collect();
        assert_eq!(paths.len(), 1);
        let r = tore_replay::Recording::open(&paths[0]).unwrap();
        assert!(r.complete());
        assert!(r.frame_count() > 20);
        std::fs::remove_dir_all(folder).unwrap();
    }
}

#[test]
fn status_worst_tick_is_interval_maximum_and_resets() {
    let mut rig = Rig::new(spec(1, 0, 20), config(), LinkConfig::one_way(MS));
    rig.host.costs.ticks = 3;
    rig.host.costs.total = Duration::from_millis(60);
    rig.host.costs.max = Duration::from_micros(52_300);
    let status = rig.host.status(Duration::ZERO);
    assert_eq!(status.tick_cost_mean, Duration::from_millis(20));
    assert_eq!(status.tick_cost_max, Duration::from_micros(52_300));
    assert_eq!(
        rig.host.status(Duration::ZERO).tick_cost_max,
        Duration::ZERO
    );
}
