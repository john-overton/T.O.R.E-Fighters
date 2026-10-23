//! Chat on the network simulator (slice EF6's acceptance): the host's
//! routing of every receiver in the lobby and in flight (two sides, two
//! wings, a target flown by a human and by the AI, a player with no plane,
//! a departed player), the limits, the quick messages and the system line
//! back to the sender. Synthetic resources, the client sessions driven
//! through their chat calls.

use super::tests::{Rig, level_script, spec};
use super::*;
use crate::host::{HostLog, OpenPlanes, StartMode};
use crate::wire::chat::{ChatFrom, MAX_TEXT, NO_ONE_HEARS, Receiver, Standing, Tone};
use tore_net::sim::LinkConfig;
use tore_sim::ai::launch::Side;

const MS: Duration = Duration::from_millis(1);

/// Friendly wing 1 of two, friendly wing 2 of one, the enemy's wing 1 of
/// three, 2 nm apart so a designation by identity can reach.
fn mission() -> MissionSpec {
    let mut spec = spec(2, 3, 2);
    spec.wings[1].count = 1;
    spec
}

/// A game where every plane is open to humans, flying from the start.
fn flying_rig() -> Rig {
    Rig::with_config(
        mission(),
        LinkConfig::for_round_trip(30 * MS, 0., 0., 0.),
        5,
        |config| {
            config.open_planes = OpenPlanes::All;
            config.start = StartMode::Now;
        },
    )
}

/// The planes of a wing, by plane number.
fn wing(rig: &Rig, side: Side, index: u8) -> Vec<u32> {
    rig.host
        .world()
        .roster
        .planes()
        .iter()
        .filter(|p| p.slot.wing.side == side && p.slot.wing.index == index)
        .map(|p| p.id.0)
        .collect()
}

/// A player named `callsign` who takes `plane` and flies level.
fn pilot(rig: &mut Rig, callsign: &str, plane: u32) -> usize {
    let callsign = callsign.to_owned();
    rig.join(
        move |c| {
            c.callsign = callsign;
            c.plane = Some(plane);
        },
        level_script(),
    )
}

/// A player in the lobby who takes no slot.
fn lobby_player(rig: &mut Rig, callsign: &str) -> usize {
    let callsign = callsign.to_owned();
    rig.join(
        move |c| {
            c.callsign = callsign;
            c.auto_ready = false;
        },
        level_script(),
    )
}

/// Everyone's chat lines so far.
fn lines(rig: &Rig, player: usize) -> Vec<ChatLine> {
    rig.players[player]
        .events
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Chat(line) => Some(line.clone()),
            _ => None,
        })
        .collect()
}

/// What the player has seen of its own chat, as text, oldest first.
fn texts(rig: &Rig, player: usize) -> Vec<String> {
    lines(rig, player).iter().map(ChatLine::display).collect()
}

fn from(line: &ChatLine) -> (&str, Standing, bool) {
    match &line.from {
        ChatFrom::Player {
            callsign,
            standing,
            you,
        } => (callsign, *standing, *you),
        ChatFrom::System => ("", Standing::Neutral, false),
    }
}

/// The four players of the flight: Viper leads friendly wing 1 with Cobra
/// beside it, Hawk leads friendly wing 2, Raven leads the enemy's wing 1,
/// whose second plane the AI flies.
struct Flight {
    rig: Rig,
    viper: usize,
    cobra: usize,
    hawk: usize,
    raven: usize,
    /// The enemy's AI plane.
    enemy_ai: u32,
    /// Raven's plane.
    raven_plane: u32,
}

fn flight() -> Flight {
    let mut rig = flying_rig();
    let f1 = wing(&rig, Side::Friendly, 0);
    let f2 = wing(&rig, Side::Friendly, 1);
    let e1 = wing(&rig, Side::Enemy, 0);
    assert_eq!((f1.len(), f2.len(), e1.len()), (2, 1, 3));
    let viper = pilot(&mut rig, "Viper", f1[0]);
    let cobra = pilot(&mut rig, "Cobra", f1[1]);
    let hawk = pilot(&mut rig, "Hawk", f2[0]);
    let raven = pilot(&mut rig, "Raven", e1[0]);
    let all = [viper, cobra, hawk, raven];
    assert!(
        rig.run_until(Duration::from_secs(8), |r| all.iter().all(|&p| r.seated(p))),
        "everyone flies"
    );
    rig.run(Duration::from_secs(1));
    Flight {
        rig,
        viper,
        cobra,
        hawk,
        raven,
        enemy_ai: e1[1],
        raven_plane: e1[0],
    }
}

impl Flight {
    /// Viper's plane has `plane` designated.
    fn designate(&mut self, plane: u32) {
        let own = self.rig.players[self.viper].client.seat().unwrap().1;
        self.rig.host.test_designations.insert(own, plane);
    }

    fn say(&mut self, player: usize, receiver: Receiver, text: &str) {
        self.rig.players[player]
            .client
            .chat(receiver, text)
            .expect("the line goes");
        self.rig.run(Duration::from_millis(300));
    }
}

#[test]
fn all_reaches_everyone_in_the_lobby_and_the_sender_sees_its_own_line() {
    let mut rig = Rig::new(mission(), LinkConfig::PERFECT, 3);
    let a = lobby_player(&mut rig, "Viper");
    let b = lobby_player(&mut rig, "Cobra");
    let c = lobby_player(&mut rig, "Hawk");
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        [a, b, c]
            .iter()
            .all(|&p| r.players[p].client.phase() == ClientPhase::Lobby)
    }));
    rig.players[a]
        .client
        .chat(Receiver::All, "  Anyone for a mission? ")
        .unwrap();
    rig.run(Duration::from_millis(200));
    for p in [b, c] {
        let got = lines(&rig, p);
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].text, "Anyone for a mission?", "cut of its spaces");
        assert_eq!(from(&got[0]), ("Viper", Standing::Neutral, false));
        assert_eq!(got[0].receiver, Receiver::All);
        assert_eq!(got[0].tone(), Tone::Everyone);
    }
    let own = lines(&rig, a);
    assert_eq!(own.len(), 1, "its own line back, and nothing else: {own:?}");
    assert!(from(&own[0]).2);
    assert_eq!(own[0].display(), "YOU TO ALL: Anyone for a mission?");
    // Chat does not change the lobby.
    assert!(rig.logs.iter().any(|l| matches!(l,
        HostLog::Chat { callsign, receiver: Receiver::All, heard: 2, .. } if callsign == "Viper")));
}

#[test]
fn before_flight_only_all_goes_and_the_host_says_so() {
    let mut rig = Rig::new(mission(), LinkConfig::PERFECT, 3);
    let a = lobby_player(&mut rig, "Viper");
    let b = lobby_player(&mut rig, "Cobra");
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[a].client.phase() == ClientPhase::Lobby
            && r.players[b].client.phase() == ClientPhase::Lobby
    }));
    // The game checks it itself...
    assert_eq!(
        rig.players[a].client.chat(Receiver::Friendlies, "hello"),
        Err(Refusal::OnlyAll)
    );
    // ...and the host holds to it when a game does not.
    let now = rig.net.now();
    rig.players[a].client.request(
        now,
        Message::ChatSend(ChatSend::typed(Receiver::Wing, "hello")),
    );
    rig.run(Duration::from_millis(200));
    let told = lines(&rig, a);
    assert_eq!(told.len(), 1, "{told:?}");
    assert_eq!(told[0].tone(), Tone::System);
    assert_eq!(told[0].text, Refusal::OnlyAll.text());
    assert!(lines(&rig, b).is_empty(), "no one else heard it");
}

#[test]
fn friendlies_enemies_wing_and_all_reach_their_own_in_flight() {
    let mut f = flight();
    let (viper, cobra, hawk, raven) = (f.viper, f.cobra, f.hawk, f.raven);

    // Friendlies: Cobra and Hawk, both on Viper's side; not the enemy.
    f.say(viper, Receiver::Friendlies, "Form up");
    assert_eq!(texts(&f.rig, cobra), ["Viper TO FRIENDLIES: Form up"]);
    assert_eq!(texts(&f.rig, hawk), ["Viper TO FRIENDLIES: Form up"]);
    assert!(lines(&f.rig, raven).is_empty());
    assert_eq!(texts(&f.rig, viper), ["YOU TO FRIENDLIES: Form up"]);
    assert_eq!(from(&lines(&f.rig, cobra)[0]).1, Standing::Own);
    assert_eq!(lines(&f.rig, cobra)[0].tone(), Tone::Own);

    // Enemies: only Raven, whose line is from the other side.
    f.say(viper, Receiver::Enemies, "Go home");
    assert_eq!(texts(&f.rig, raven), ["Viper TO ENEMIES: Go home"]);
    let heard = lines(&f.rig, raven);
    assert_eq!(from(&heard[0]).1, Standing::Enemy);
    assert_eq!(heard[0].tone(), Tone::Enemy);
    assert_eq!(texts(&f.rig, cobra).len(), 1, "Cobra did not hear it");
    assert_eq!(texts(&f.rig, hawk).len(), 1);

    // Wing: only Cobra, who flies with Viper; Hawk's wing is another.
    f.say(viper, Receiver::Wing, "Break left");
    assert_eq!(
        texts(&f.rig, cobra).last().unwrap(),
        "Viper TO WING: Break left"
    );
    assert_eq!(texts(&f.rig, hawk).len(), 1);
    assert_eq!(texts(&f.rig, raven).len(), 1);

    // All: everyone, the enemy's blue-to-it line from the other side red.
    f.say(raven, Receiver::All, "Hello all");
    for p in [viper, cobra, hawk] {
        let last = lines(&f.rig, p).pop().unwrap();
        assert_eq!(last.display(), "Raven TO ALL: Hello all");
        assert_eq!(from(&last).1, Standing::Enemy);
        assert_eq!(last.tone(), Tone::Enemy);
    }
    let own = lines(&f.rig, raven).pop().unwrap();
    assert_eq!(own.tone(), Tone::Everyone, "its own side's line to all");
    f.say(cobra, Receiver::All, "Hello back");
    let heard = lines(&f.rig, viper).pop().unwrap();
    assert_eq!(heard.tone(), Tone::Everyone);
    assert_eq!(lines(&f.rig, raven).pop().unwrap().tone(), Tone::Enemy);
}

#[test]
fn target_goes_to_the_human_flying_the_designated_plane_and_only_to_a_human() {
    let mut f = flight();
    let (viper, cobra, raven) = (f.viper, f.cobra, f.raven);

    // Viper designates Raven's plane, which a human flies.
    f.designate(f.raven_plane);
    f.say(viper, Receiver::Target, "I've got you");
    assert_eq!(texts(&f.rig, raven), ["Viper TO YOU: I've got you"]);
    assert!(lines(&f.rig, cobra).is_empty(), "no one else heard it");
    assert_eq!(texts(&f.rig, viper), ["YOU TO TARGET: I've got you"]);

    // Then the enemy's AI plane: no human to hear it.
    f.designate(f.enemy_ai);
    f.say(viper, Receiver::Target, "Anyone there?");
    let told = texts(&f.rig, viper);
    assert_eq!(
        &told[1..],
        ["YOU TO TARGET: Anyone there?", NO_ONE_HEARS],
        "its own line, then the host's words"
    );
    assert_eq!(texts(&f.rig, raven).len(), 1);
}

#[test]
fn target_with_nothing_designated_is_refused() {
    let mut f = flight();
    let hawk = f.hawk;
    f.say(hawk, Receiver::Target, "Hello?");
    assert_eq!(texts(&f.rig, hawk), [Refusal::NoTarget.text()]);
    // The refused line is not counted against the rate.
    for n in 0..5 {
        f.say(hawk, Receiver::Friendlies, &format!("line {n}"));
    }
    assert_eq!(texts(&f.rig, hawk).len(), 6);
}

#[test]
fn a_line_nobody_hears_comes_back_with_the_hosts_words() {
    let mut f = flight();
    // Hawk flies alone in friendly wing 2.
    let hawk = f.hawk;
    f.say(hawk, Receiver::Wing, "Anyone in my wing?");
    assert_eq!(
        texts(&f.rig, hawk),
        ["YOU TO WING: Anyone in my wing?", NO_ONE_HEARS]
    );
    assert_eq!(lines(&f.rig, hawk)[1].tone(), Tone::System);
    // The log has both who and to whom, and that no one heard.
    assert!(f.rig.logs.iter().any(|l| matches!(l,
        HostLog::Chat { callsign, receiver: Receiver::Wing, heard: 0, text, .. }
            if callsign == "Hawk" && text == "Anyone in my wing?")));
}

#[test]
fn observers_talk_among_themselves_and_hear_the_flyers_but_never_reach_them() {
    let mut f = flight();
    let (viper, hawk) = (f.viper, f.hawk);
    // Two players who join and take no slot while the mission flies are
    // observers (John, 2026-10-01).
    let (late, later) = (
        lobby_player(&mut f.rig, "Late"),
        lobby_player(&mut f.rig, "Later"),
    );
    let ok = f.rig.run_until(Duration::from_secs(10), |r| {
        [late, later]
            .iter()
            .all(|&p| r.players[p].client.phase() == ClientPhase::Lobby)
    });
    assert!(ok, "both observers are in the lobby");
    // They hear a flyer's All, with no side to colour by, and not its
    // team talk.
    f.say(viper, Receiver::All, "Welcome");
    for p in [late, later] {
        assert_eq!(texts(&f.rig, p), ["Viper TO ALL: Welcome"]);
        assert_eq!(from(&lines(&f.rig, p)[0]).1, Standing::Neutral);
    }
    f.say(viper, Receiver::Friendlies, "Team talk");
    assert_eq!(texts(&f.rig, late).len(), 1, "Friendlies are the flyers'");
    // An observer's All reaches the other observer and no flyer.
    let flyers_before = (lines(&f.rig, viper).len(), lines(&f.rig, hawk).len());
    f.say(late, Receiver::All, "Good luck");
    assert_eq!(
        texts(&f.rig, later).last().unwrap(),
        "Late TO ALL: Good luck"
    );
    assert_eq!(texts(&f.rig, late).last().unwrap(), "YOU TO ALL: Good luck");
    assert_eq!(
        (lines(&f.rig, viper).len(), lines(&f.rig, hawk).len()),
        flyers_before,
        "no flyer hears an observer"
    );
    // The host logged it as heard by the one other observer.
    assert!(f.rig.logs.iter().any(|l| matches!(l,
        HostLog::Chat { callsign, heard: 1, .. } if callsign == "Late")));
    // And the host refuses an observer anything but All.
    let now = f.rig.net.now();
    f.rig.players[late].client.request(
        now,
        Message::ChatSend(ChatSend::typed(Receiver::Enemies, "Boo")),
    );
    f.rig.run(Duration::from_millis(300));
    assert_eq!(texts(&f.rig, late).last().unwrap(), Refusal::OnlyAll.text());
}

#[test]
fn an_observer_alone_hears_no_one_and_everyone_hears_everyone_with_nothing_flying() {
    let mut f = flight();
    let alone = lobby_player(&mut f.rig, "Alone");
    assert!(f.rig.run_until(Duration::from_secs(3), |r| {
        r.players[alone].client.phase() == ClientPhase::Lobby
    }));
    f.say(alone, Receiver::All, "Anyone?");
    assert_eq!(
        texts(&f.rig, alone),
        ["YOU TO ALL: Anyone?", NO_ONE_HEARS],
        "no flyer hears it, and there is no other observer"
    );
    assert!(lines(&f.rig, f.hawk).is_empty());
    // Nothing flying: the lobby is one room (the lobby test above).
    let mut rig = Rig::new(mission(), LinkConfig::PERFECT, 3);
    let a = lobby_player(&mut rig, "A");
    let b = lobby_player(&mut rig, "B");
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        [a, b]
            .iter()
            .all(|&p| r.players[p].client.phase() == ClientPhase::Lobby)
    }));
    rig.players[a].client.chat(Receiver::All, "hi").unwrap();
    rig.run(Duration::from_millis(200));
    assert_eq!(texts(&rig, b), ["A TO ALL: hi"]);
}

#[test]
fn a_departed_player_no_longer_hears_and_the_line_finds_the_rest() {
    let mut f = flight();
    let (viper, cobra, hawk) = (f.viper, f.cobra, f.hawk);
    let now = f.rig.net.now();
    f.rig.players[cobra].client.leave_game(now);
    assert!(f.rig.run_until(Duration::from_secs(7), |r| r.closed(cobra)));
    let before = lines(&f.rig, cobra).len();
    f.say(viper, Receiver::Friendlies, "Where did Cobra go");
    assert_eq!(
        texts(&f.rig, hawk),
        ["Viper TO FRIENDLIES: Where did Cobra go"]
    );
    assert_eq!(lines(&f.rig, cobra).len(), before);
    // Cobra's plane is the AI's now: Viper's wing hears no human.
    f.say(viper, Receiver::Wing, "Cobra?");
    assert_eq!(*texts(&f.rig, viper).last().unwrap(), NO_ONE_HEARS);
    assert!(
        !f.rig
            .logs
            .iter()
            .any(|l| matches!(l, HostLog::Fault { .. }))
    );
}

#[test]
fn a_quick_message_carries_its_sound_to_the_receivers_and_not_back_to_the_sender() {
    let mut f = flight();
    let (viper, cobra, hawk, raven) = (f.viper, f.cobra, f.hawk, f.raven);
    let shipped = tore_formats::chat::parse(
        b"send to friendlies\\Get this guy off me\\^offme.5k\r\nsend to all\\Splash one bandit!\r\n",
    );
    assert_eq!(shipped.len(), 2);
    f.rig.players[viper]
        .client
        .chat_quick(5, &shipped[0], Receiver::All)
        .unwrap();
    f.rig.run(Duration::from_millis(300));
    let heard = lines(&f.rig, cobra).pop().unwrap();
    assert_eq!(heard.display(), "Viper TO FRIENDLIES: Get this guy off me");
    assert_eq!(heard.sound.as_deref(), Some("^OFFME.5K"));
    assert_eq!(
        lines(&f.rig, hawk).pop().unwrap().sound.as_deref(),
        Some("^OFFME.5K")
    );
    assert!(lines(&f.rig, raven).is_empty(), "the line's own receiver");
    let own = lines(&f.rig, viper).pop().unwrap();
    assert_eq!(own.text, "Get this guy off me");
    assert_eq!(own.sound, None, "the sender does not hear its own");

    // A line with no receiver of its own goes where the player picked, and
    // one with no sound is text only.
    f.rig.players[cobra]
        .client
        .chat_quick(2, &shipped[1], Receiver::Enemies)
        .unwrap();
    f.rig.run(Duration::from_millis(300));
    let heard = lines(&f.rig, raven).pop().unwrap();
    assert_eq!(heard.display(), "Cobra TO ALL: Splash one bandit!");
    assert_eq!(heard.sound, None);

    // The host refuses a number or a sound that is not one (a game that is
    // not this one).
    let bad = |number: u8, sound: Option<&str>| {
        Message::ChatSend(ChatSend {
            receiver: Receiver::All,
            text: "Hi".into(),
            quick: Some(crate::wire::chat::Quick {
                number,
                sound: sound.map(str::to_owned),
            }),
        })
    };
    let now = f.rig.net.now();
    f.rig.players[hawk].client.request(now, bad(13, None));
    f.rig.run(Duration::from_millis(300));
    f.rig.players[hawk]
        .client
        .request(f.rig.net.now(), bad(3, Some("..\\EVIL.5K")));
    f.rig.run(Duration::from_millis(1100));
    let told = texts(&f.rig, hawk);
    assert_eq!(told.len(), 4, "two heard, two refused: {told:?}");
    assert!(told[2..].iter().all(|t| t == Refusal::BadQuick.text()));
}

#[test]
fn the_limits_a_line_80_characters_printable_and_five_in_five_seconds() {
    let mut f = flight();
    let (viper, cobra) = (f.viper, f.cobra);
    // The game refuses what it can tell is wrong.
    assert_eq!(
        f.rig.players[viper]
            .client
            .chat(Receiver::All, &"x".repeat(MAX_TEXT + 1)),
        Err(Refusal::TooLong)
    );
    assert_eq!(
        f.rig.players[viper].client.chat(Receiver::All, "caf\u{e9}"),
        Err(Refusal::Unprintable)
    );
    assert_eq!(
        f.rig.players[viper].client.chat(Receiver::All, "   "),
        Err(Refusal::Empty)
    );
    // And the host holds a game that does not.
    let now = f.rig.net.now();
    for text in [
        "x".repeat(MAX_TEXT + 1),
        "bell\u{7}".to_owned(),
        "   ".to_owned(),
    ] {
        f.rig.players[viper].client.request(
            now,
            Message::ChatSend(ChatSend::typed(Receiver::All, &text)),
        );
    }
    f.rig.run(Duration::from_millis(1100));
    assert_eq!(
        texts(&f.rig, viper),
        [Refusal::TooLong.text(), Refusal::Unprintable.text()],
        "an empty line is dropped without a word"
    );
    assert!(lines(&f.rig, cobra).is_empty());
    // 80 characters go.
    f.say(viper, Receiver::All, &"y".repeat(MAX_TEXT));
    assert_eq!(lines(&f.rig, cobra).len(), 1);

    // Five lines in five seconds: the sixth is refused to the sender.
    let start = lines(&f.rig, cobra).len();
    for n in 0..5 {
        f.rig.players[cobra]
            .client
            .chat(Receiver::All, &format!("line {n}"))
            .unwrap();
    }
    f.rig.run(Duration::from_millis(300));
    assert_eq!(lines(&f.rig, viper).len(), start + 5 + 2, "five heard");
    f.rig.players[cobra]
        .client
        .chat(Receiver::All, "line 5")
        .unwrap();
    f.rig.run(Duration::from_millis(300));
    let told = lines(&f.rig, cobra);
    assert_eq!(told.last().unwrap().text, Refusal::TooFast.text());
    assert_eq!(told.last().unwrap().tone(), Tone::System);
    assert_eq!(
        lines(&f.rig, viper).len(),
        start + 5 + 2,
        "the sixth was not sent on"
    );
    // After the window it goes again.
    f.rig.run(Duration::from_secs(5));
    f.say(cobra, Receiver::All, "line 6");
    assert_eq!(lines(&f.rig, viper).last().unwrap().text, "line 6");
    assert!(
        f.rig.logs.iter().any(|l| matches!(l,
            HostLog::Lobby { callsign, event: crate::host::LobbyEvent::Refused { reason, .. }, .. }
                if callsign == "Cobra" && reason == Refusal::TooFast.text())),
        "the refusal is in the log"
    );
}
