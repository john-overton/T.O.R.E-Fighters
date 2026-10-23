use tore_replay::{feedback::Feedback, *};
#[test]
fn feedback_uses_existing_events_and_keeps_both_times() {
    let root = std::env::temp_dir().join(format!("tore-feedback-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let feedback = Feedback {
        tick: 12,
        submit_tick: 240,
        opened_at: 1234,
        submitted_at: 5678,
        player: "Viper".into(),
        seat: 2,
        plane: 7,
        aircraft: "F18.PT".into(),
        text: "é\n\"bump\"".into(),
    };
    for version in [2, 3, 4] {
        let path = root.join(format!("{version}.tore-replay"));
        let header = Header {
            format_version: version,
            world: World {
                surface: version == 3,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut writer = Writer::create(&path, &header).unwrap();
        let long = Feedback {
            text: "🛩".repeat(1000),
            ..feedback.clone()
        };
        writer
            .push(&Frame {
                tick: 239,
                events: vec![long.event()],
                ..Default::default()
            })
            .unwrap();
        writer
            .push(&Frame {
                tick: 240,
                events: vec![feedback.event()],
                ..Default::default()
            })
            .unwrap();
        writer.finish(&Footer::default()).unwrap();
        let r = Recording::open(&path).unwrap();
        assert_eq!(r.header().format_version, version);
        assert!(r.flagged());
        assert_eq!(r.feedback().collect::<Vec<_>>(), [long, feedback.clone()]);
        let mut out = Vec::new();
        export::feedback::write(&r, "flight.tore-replay", &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert_eq!(out.lines().count(), 2);
        assert!(out.contains("\"tick\":12"));
        assert!(out.contains("\"submit_tick\":240"));
        assert!(out.contains("é\\n\\\"bump\\\""));
    }
    let old = Recording::from_bytes(include_bytes!("golden/format2.tore-replay").to_vec()).unwrap();
    assert!(!old.flagged());
    assert_eq!(old.feedback().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn feedback_bounds_count_unicode_characters() {
    assert!(Feedback::valid_text(&"🛩".repeat(1000)));
    assert!(!Feedback::valid_text(&"a".repeat(1001)));
    assert!(!Feedback::valid_text(" \n "));
    assert!(!Feedback::valid_text("bad\0text"));
}
