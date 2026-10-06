//! The settings registry and store (slice F2-0).

use super::*;
use std::time::Duration;

#[test]
fn the_registry_is_numbered_one_to_twenty_two_in_order_with_unique_names() {
    for (index, setting) in REGISTRY.iter().enumerate() {
        assert_eq!(usize::from(setting.number), index + 1, "{}", setting.name);
        assert_eq!(super::setting(setting.number), Some(setting));
        assert_eq!(by_name(setting.name), Some(setting));
    }
    assert_eq!(super::setting(0), None);
    assert_eq!(super::setting(23), None);
    assert_eq!(by_name("cheats"), None);
}

#[test]
fn every_default_is_an_allowed_value() {
    for setting in &REGISTRY {
        assert!(setting.allows(setting.coop), "{} co-op", setting.name);
        assert!(setting.allows(setting.pvp), "{} pvp", setting.name);
        // Every word names an allowed value.
        for (value, _) in setting.words {
            assert!(setting.allows(*value), "{} word {value}", setting.name);
        }
    }
}

#[test]
fn the_defaults_follow_the_design_table() {
    let coop = Store::defaults(Mode::Coop);
    assert_eq!(coop.mode(), Mode::Coop);
    assert_eq!(coop.max_players(), 30);
    assert!(coop.join_in_progress());
    assert_eq!(coop.visibility(), Visibility::Local);
    assert_eq!(coop.password(), None);
    assert!(coop.friendly_fire());
    assert!(!coop.lock_sides());
    assert_eq!(coop.loadouts(), LoadoutRule::Own);
    assert_eq!(coop.respawn(), Respawn::None);
    assert_eq!(coop.lives(), None);
    assert_eq!(coop.revive_delay_seconds(), 0);
    assert_eq!(coop.revive_distance_nm(), 10);
    assert_eq!(coop.revive_weapons(), RevivalWeapons::Missiles);
    assert_eq!(coop.fight(), Fight::Sides);
    assert_eq!(coop.tally(), ScoreTally::Kills);
    assert_eq!(coop.time_limit_seconds(), None);
    assert_eq!(coop.kill_limit(), None);
    assert_eq!(coop.kill_owner(), KillOwner::Side);
    assert_eq!(coop.observer_delay_seconds(), 0);
    assert_eq!(coop.idle_ai_seconds(), Some(300));

    let pvp = Store::defaults(Mode::Pvp);
    assert_eq!(pvp.mode(), Mode::Pvp);
    assert!(pvp.lock_sides());
    assert_eq!(pvp.respawn(), Respawn::Revive);
    assert_eq!(pvp.lives(), None);
    assert_eq!(pvp.time_limit_seconds(), Some(600));
    assert_eq!(pvp.kill_limit(), Some(5));
    assert_eq!(pvp.kill_owner(), KillOwner::Side);
}

#[test]
fn values_read_and_print_in_words() {
    let mode = by_name("mode").unwrap();
    assert_eq!(mode.parse("co-op"), Some(0));
    assert_eq!(mode.parse("PvP"), Some(1));
    assert_eq!(mode.parse("2"), None);
    assert_eq!(mode.text(1), "pvp");
    assert_eq!(mode.values_text(), "co-op or pvp");

    let lives = by_name("lives").unwrap();
    assert_eq!(lives.parse("unlimited"), Some(UNLIMITED_LIVES));
    assert_eq!(lives.parse("10"), Some(10));
    assert_eq!(lives.parse("11"), None);
    assert_eq!(lives.values_text(), "0 to 10 or unlimited");

    let delay = by_name("revive-delay").unwrap();
    assert_eq!(delay.text(0), "none");
    assert_eq!(delay.text(60), "1 minute");
    assert_eq!(delay.text(300), "5 minutes");
    assert_eq!(
        delay.values_text(),
        "none, 1 minute, 2 minutes, 3 minutes, 4 minutes or 5 minutes"
    );
    let observer = by_name("observer-delay").unwrap();
    assert_eq!(observer.text(30), "30 seconds");
    // The idle time is minutes, never, 1, 2, 5 or 10, and 5 by default in
    // both modes (John, 2026-10-06; the list is an agent decision).
    let idle = by_name("idle-ai").unwrap();
    assert_eq!(idle.text(0), "never");
    assert_eq!(idle.text(60), "1 minute");
    assert_eq!(idle.text(300), "5 minutes");
    assert_eq!(
        idle.values_text(),
        "never, 1 minute, 2 minutes, 5 minutes or 10 minutes"
    );
    assert_eq!(idle.parse("never"), Some(0));
    assert_eq!(idle.parse("10"), None, "10 seconds is gone");
    assert_eq!(idle.parse("300"), Some(300));
    assert_eq!(idle.default_in(Mode::Coop), 300);
    assert_eq!(idle.default_in(Mode::Pvp), 300);
    assert!(!idle.allows(30));
    assert!(idle.allows(600));
    let distance = by_name("revive-distance").unwrap();
    assert_eq!(distance.text(40), "40 nm");
    assert_eq!(distance.parse("15"), None);
    assert_eq!(by_name("max-players").unwrap().values_text(), "1 to 30");
}

#[test]
fn the_registry_refuses_with_the_setting_and_its_values() {
    assert_eq!(refusal(number::MODE, 1), None);
    assert_eq!(
        refusal(number::REVIVE_DISTANCE, 15).as_deref(),
        Some("revive-distance is 1 nm, 5 nm, 10 nm, 20 nm or 40 nm.")
    );
    assert_eq!(
        refusal(number::MAX_PLAYERS, 0).as_deref(),
        Some("max-players is 1 to 30.")
    );
    assert_eq!(refusal(0, 0).as_deref(), Some("There is no setting 0."));
    assert_eq!(refusal(23, 0).as_deref(), Some("There is no setting 23."));
    assert!(refusal(number::PASSWORD, 1).is_some());
    // Public is the registry's since stage I lists games; whether a host can
    // list is the host's question (slice F2-1).
    assert_eq!(refusal(number::VISIBILITY, 2), None);
    assert_eq!(refusal(number::VISIBILITY, 0), None);
    assert_eq!(
        words(&[
            (number::MODE, 1),
            (number::TIME_LIMIT, 600),
            (number::LIVES, 255)
        ]),
        "mode pvp, time-limit 10 minutes, lives unlimited"
    );
}

#[test]
fn the_store_applies_all_or_none_and_a_new_mode_resets_the_lobby_settings() {
    let mut store = Store::defaults(Mode::Coop);
    store.set_password(Some("secret"));
    store
        .apply(&[(number::MAX_PLAYERS, 8), (number::FRIENDLY_FIRE, 0)])
        .unwrap();
    assert_eq!(store.max_players(), 8);
    assert!(!store.friendly_fire());

    // One refused value refuses them all.
    let before = store.clone();
    assert!(
        store
            .apply(&[(number::LIVES, 3), (number::KILL_LIMIT, 4)])
            .is_err()
    );
    assert_eq!(store, before);

    // PvP: the lobby's settings take PvP's defaults, then the values given;
    // the game's own (players, password) are kept.
    store
        .apply(&[(number::KILL_LIMIT, 10), (number::MODE, 1)])
        .unwrap();
    assert_eq!(store.mode(), Mode::Pvp);
    assert!(store.friendly_fire());
    assert!(store.lock_sides());
    assert_eq!(store.kill_limit(), Some(10));
    assert_eq!(store.time_limit_seconds(), Some(600));
    assert_eq!(store.max_players(), 8);
    assert_eq!(store.password(), Some("secret"));
}

#[test]
fn the_lobby_list_carries_every_setting_and_only_whether_a_password_is_set() {
    let mut store = Store::defaults(Mode::Coop);
    let list = store.lobby_list();
    assert_eq!(list.len(), REGISTRY.len());
    assert!(
        list.iter()
            .zip(1..)
            .all(|(&(number, _), expected)| number == expected)
    );
    assert_eq!(
        list[usize::from(number::PASSWORD) - 1],
        (number::PASSWORD, 0)
    );
    store.set_password(Some("secret"));
    assert_eq!(
        store.lobby_list()[usize::from(number::PASSWORD) - 1],
        (number::PASSWORD, 1)
    );
}

#[test]
fn a_host_starts_from_its_configuration() {
    let mut config = HostConfig::new(crate::host::BuildId {
        version: "0.1.3".into(),
        commit: "abc".into(),
        release: false,
    });
    config.name = "Viper's game".into();
    config.password = Some("pw".into());
    config.max_players = 12;
    config.time_limit = Some(Duration::from_secs(45 * 60));
    let store = Store::from_config(&config);
    assert_eq!(store.mode(), Mode::Coop);
    assert_eq!(store.name(), "Viper's game");
    assert_eq!(store.password(), Some("pw"));
    assert_eq!(store.max_players(), 12);
    // A dedicated server's own limit is kept as given.
    assert_eq!(store.time_limit_seconds(), Some(2_700));

    // A server's file sets the others (slice F2-1): the mode's defaults
    // first, then its values, and the limit still as given.
    config.settings = vec![(number::KILL_LIMIT, 3), (number::MODE, 1)];
    let store = Store::from_config(&config);
    assert_eq!(store.mode(), Mode::Pvp);
    assert_eq!(store.kill_limit(), Some(3));
    assert!(store.lock_sides(), "PvP's default");
    assert_eq!(store.time_limit_seconds(), Some(2_700));
    assert_eq!(store.max_players(), 12);
}

#[test]
fn the_choices_match_their_values() {
    for mode in [Mode::Coop, Mode::Pvp] {
        assert_eq!(Mode::from_value(mode.value()), Some(mode));
    }
    for respawn in [Respawn::None, Respawn::AiSlot, Respawn::Revive] {
        assert_eq!(Respawn::from_value(respawn.value()), Some(respawn));
    }
    for tally in [ScoreTally::Kills, ScoreTally::Damage, ScoreTally::Ratio] {
        assert_eq!(ScoreTally::from_value(tally.value()), Some(tally));
    }
    for owner in [KillOwner::Total, KillOwner::Side, KillOwner::Player] {
        assert_eq!(KillOwner::from_value(owner.value()), Some(owner));
    }
    assert_eq!(Fight::from_value(2), None);
    assert_eq!(
        Visibility::from_value(Visibility::Public.value()),
        Some(Visibility::Public)
    );
    assert_eq!(LoadoutRule::from_value(1), Some(LoadoutRule::Any));
}

#[test]
fn setting_21_is_the_host_calculated_or_a_pinned_player() {
    let host = super::setting(number::HOST).unwrap();
    assert_eq!(host.name, "host");
    assert_eq!(host.text(0), "calculated");
    assert!(host.allows(0) && host.allows(256) && !host.allows(257));
    assert_eq!(host.change, Change::AnyTime);
    let mut store = Store::defaults(Mode::Pvp);
    assert_eq!(store.pinned_host(), None);
    assert!(store.lobby_list().contains(&(number::HOST, 0)));
    // The registry takes a pin (the host checks the player, slice K6).
    assert_eq!(refusal(number::HOST, 4), None);
    assert_eq!(store.apply(&[(number::HOST, 4)]), Ok(()));
    assert_eq!(store.pinned_host(), Some(3));
    assert_eq!(store.apply(&[(number::HOST, 0)]), Ok(()));
    assert!(refusal(number::HOST, 257).is_some_and(|why| why.starts_with("host is")));
}

/// Slice R1 (John, 2026-10-06, Q56): the King's snapshot rate.
#[test]
fn the_snapshot_rate_offers_60_30_and_20_and_defaults_to_60() {
    let rate = by_name("snapshot-rate").unwrap();
    assert_eq!(rate.number, number::SNAPSHOT_RATE);
    assert_eq!(rate.change, Change::InLobby);
    assert!(!rate.pvp_only);
    assert_eq!(rate.coop, 60);
    assert_eq!(rate.pvp, 60);
    assert_eq!(KING_SNAPSHOT_RATES, [60, 30, 20]);
    for value in [60, 30, 20] {
        assert!(rate.allows(value), "{value}");
    }
    // The rates that divide 120 and are not offered, and the ends.
    for value in [0, 10, 12, 15, 24, 40, 61, 120] {
        assert!(!rate.allows(value), "{value}");
    }
    assert_eq!(rate.text(60), "60 a second");
    assert_eq!(rate.text(30), "30 a second");
    assert_eq!(
        rate.values_text(),
        "60 a second, 30 a second or 20 a second"
    );
    assert_eq!(rate.parse("30"), Some(30));
    assert_eq!(rate.parse("24"), None);
    assert_eq!(
        refusal(number::SNAPSHOT_RATE, 24).unwrap(),
        "snapshot-rate is 60 a second, 30 a second or 20 a second."
    );
    assert_eq!(refusal(number::SNAPSHOT_RATE, 20), None);
    assert_eq!(Store::defaults(Mode::Coop).snapshot_rate(), 60);
    assert_eq!(Store::defaults(Mode::Pvp).snapshot_rate(), 60);
    assert_eq!(
        words(&[(number::SNAPSHOT_RATE, 20)]),
        "snapshot-rate 20 a second"
    );
}

#[test]
fn the_store_applies_the_rate_and_a_new_mode_keeps_it() {
    let mut store = Store::defaults(Mode::Coop);
    store.apply(&[(number::SNAPSHOT_RATE, 30)]).unwrap();
    assert_eq!(store.snapshot_rate(), 30);
    assert_eq!(store.get(number::SNAPSHOT_RATE), Some(30));
    // A new mode resets what the King changes in the lobby, but not this.
    store.apply(&[(number::MODE, 1)]).unwrap();
    assert_eq!(store.mode(), Mode::Pvp);
    assert_eq!(store.snapshot_rate(), 30);
    // A refused list changes nothing.
    assert!(store.apply(&[(number::SNAPSHOT_RATE, 40)]).is_err());
    assert_eq!(store.snapshot_rate(), 30);
    assert_eq!(
        store.lobby_list().last(),
        Some(&(number::SNAPSHOT_RATE, 30)),
        "in the lobby state, last by number"
    );
}

#[test]
fn a_configurations_rate_is_kept_as_given_past_the_kings_list() {
    let mut config = HostConfig::new(crate::host::BuildId {
        version: "test".into(),
        commit: "test".into(),
        release: false,
    });
    assert_eq!(Store::from_config(&config).snapshot_rate(), 60);
    for rate in crate::host::config::SNAPSHOT_RATES {
        config.snapshot_rate = rate;
        let store = Store::from_config(&config);
        assert_eq!(store.snapshot_rate(), rate);
        assert_eq!(store.get(number::SNAPSHOT_RATE), Some(rate));
    }
    // A file may not set it by number: it has its own key.
    config.settings = vec![(number::SNAPSHOT_RATE, 30)];
    assert!(
        config
            .check_settings()
            .unwrap_err()
            .contains("its own field")
    );
}
