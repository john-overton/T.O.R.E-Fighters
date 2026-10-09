//! Sides in the PvP lobby (lobby pass, slice L3): the colours and words of the
//! slots, the Players tint, the Bluefor and Redfor boxes in every state, the
//! filter of the slot list, and the clicks and keys that use them. Built on
//! the kit of synthetic pieces.
use super::facts::{self, Facts, SlotClick};
use super::preview::{crowd, pvp_crowd, sample, seat};
use super::sides::{self, BoxClick, SideBox, tone};
use super::*;
use crate::widgets::{Cell, Icon, Rect, Row, test_kit};
use tore_session::settings::{Sides, number};
use tore_session::wire::messages::{LobbyPhase, LobbySlot};

fn screen_of(lobby: &LobbyState, hosting: bool) -> LobbyScreen {
    LobbyScreen::sample(Arc::new(test_kit::kit()), lobby.clone(), hosting)
}

fn drawn(screen: &LobbyScreen) -> Vec<u8> {
    let mut pixels = test_kit::blank();
    screen.draw(&mut Canvas(&mut pixels));
    pixels
}

fn differing(a: &[u8], b: &[u8], (x, y, w, h): Rect) -> usize {
    (y..y + h)
        .flat_map(|py| (x..x + w).map(move |px| (px, py)))
        .filter(|&(px, py)| test_kit::at(a, px, py) != test_kit::at(b, px, py))
        .count()
}

fn click(screen: &mut LobbyScreen, at: Point) -> Option<Request> {
    screen.moved(Some((f64::from(at.0), f64::from(at.1))));
    let down = screen.button(true);
    let up = screen.button(false);
    down.or(up)
}

fn set_rule(lobby: &mut LobbyState, value: u32) {
    for entry in &mut lobby.settings {
        if entry.0 == number::LOCK_SIDES {
            entry.1 = value;
        }
    }
}

fn boxes(lobby: &LobbyState) -> [SideBox; 2] {
    sides::side_boxes(lobby, &Facts::of(Some(lobby), None)).expect("a PvP lobby has boxes")
}

fn text_of(row: &Row, column: usize) -> &str {
    match &row.cells[column] {
        Cell::Text(text) => text,
        other => panic!("not text: {other:?}"),
    }
}

fn row_of(rows: &[Row], plane: u32) -> &Row {
    rows.iter()
        .find(|r| r.key == plane.to_string())
        .unwrap_or_else(|| panic!("plane {plane} is not listed"))
}

const BLUE: usize = 0;
const RED: usize = 1;

// ---- the slots' colours and words ----

#[test]
fn a_taken_slot_is_a_bar_in_its_sides_strong_colour_with_white_text() {
    let rows = facts::slot_rows(&pvp_crowd(6));
    let blue = row_of(&rows, 1);
    assert_eq!(
        (blue.fill, blue.tint, blue.dim),
        (Some(tone::ROYAL_BLUE), Some(tone::WHITE), false)
    );
    let red = row_of(&rows, 5);
    assert_eq!(
        (red.fill, red.tint, red.dim),
        (Some(tone::BRIGHT_RED), Some(tone::WHITE), false)
    );
    assert_eq!(tone::ROYAL_BLUE, [36, 81, 186]);
    assert_eq!(tone::BRIGHT_RED, [210, 36, 40]);
}

#[test]
fn the_readers_own_slot_is_a_bar_too_with_the_own_arrow_and_no_green() {
    for (you, plane, fill) in [(1, 0, tone::ROYAL_BLUE), (3, 5, tone::BRIGHT_RED)] {
        let rows = facts::slot_rows(&pvp_crowd(you));
        let own = row_of(&rows, plane);
        assert_eq!(own.fill, Some(fill));
        assert_eq!(own.tint, Some(tone::WHITE));
        assert_eq!(own.cells[0], Cell::Icon(Icon::You));
    }
}

#[test]
fn an_open_slot_is_text_in_its_sides_light_colour() {
    let rows = facts::slot_rows(&pvp_crowd(6));
    let blue = row_of(&rows, 2);
    assert_eq!(
        (blue.fill, blue.tint, blue.dim),
        (None, Some(tone::LIGHT_BLUE), false)
    );
    let red = row_of(&rows, 7);
    assert_eq!(
        (red.fill, red.tint, red.dim),
        (None, Some(tone::LIGHT_RED), false)
    );
    assert_eq!(tone::LIGHT_BLUE, [134, 182, 223]);
    assert_eq!(tone::LIGHT_RED, [206, 113, 121]);
}

#[test]
fn a_slot_nobody_may_take_is_dimmed_and_one_kept_for_the_reader_is_open() {
    let mut lobby = pvp_crowd(6);
    lobby.slots[2].lock = Lock::Closed;
    lobby.slots[3].lock = Lock::Reserved("Pilot5".into());
    lobby.slots[4].lock = Lock::Reserved("Pilot6".into());
    lobby.slots[7].reserved = Some("Hawk".into());
    lobby.slots[8].reserved = Some("Pilot6".into());
    let rows = facts::slot_rows(&lobby);
    for plane in [2, 3, 7] {
        let row = row_of(&rows, plane);
        assert!(row.dim && row.fill.is_none(), "plane {plane}");
    }
    assert_eq!(
        row_of(&rows, 4).tint,
        Some(tone::LIGHT_BLUE),
        "kept for the reader"
    );
    assert_eq!(
        row_of(&rows, 8).tint,
        Some(tone::LIGHT_RED),
        "the AI keeps it for the reader"
    );
    assert_eq!(
        row_of(&rows, 8).cells[0],
        Cell::Icon(Icon::Lock),
        "with the lock mark"
    );
}

#[test]
fn a_player_who_is_away_keeps_the_bar_with_dimmed_white_text() {
    let mut lobby = pvp_crowd(6);
    lobby.players[3].away = true;
    let rows = facts::slot_rows(&lobby);
    let away = row_of(&rows, 6);
    assert_eq!(
        (away.fill, away.tint),
        (Some(tone::BRIGHT_RED), Some(tone::AWAY))
    );
    assert_eq!(text_of(away, 3), "AI (Pilot4 away)");
}

#[test]
fn pvp_wings_read_blue_and_red_and_co_op_keeps_wing() {
    let rows = facts::slot_rows(&pvp_crowd(6));
    assert_eq!(text_of(row_of(&rows, 1), 1), "Blue 1 #2");
    assert_eq!(text_of(row_of(&rows, 6), 1), "Red 1 #2");
    let slot = |plane: usize| -> LobbySlot { pvp_crowd(6).slots[plane].clone() };
    assert_eq!(sides::wing_label(&slot(9)), "Red 1 #5");
    let coop = facts::slot_rows(&sample(1));
    assert_eq!(text_of(&coop[1], 1), "Wing 1 #2");
}

#[test]
fn co_op_slots_are_coloured_as_before() {
    let rows = facts::slot_rows(&sample(1));
    assert_eq!(
        rows[0].tint,
        Some(crate::widgets::tone::OWN_SIDE),
        "the reader's own slot is green"
    );
    assert!(rows[1].dim, "another player's slot is dimmed");
    assert!(rows.iter().all(|r| r.fill.is_none()));
    assert!(
        rows[2].tint.is_none() && !rows[2].dim,
        "an open slot is plain"
    );
}

// ---- the filter ----

#[test]
fn with_no_side_the_list_shows_both_bluefor_first() {
    let rows = facts::slot_rows(&pvp_crowd(6));
    let planes: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(planes, ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]);
}

#[test]
fn on_a_side_the_list_shows_that_side_only() {
    let blue = facts::slot_rows(&pvp_crowd(1));
    assert_eq!(blue.len(), 5);
    assert!(blue.iter().all(|r| r.key.parse::<u32>().unwrap() < 5));
    let red = facts::slot_rows(&pvp_crowd(3));
    assert_eq!(red.len(), 5);
    assert!(red.iter().all(|r| r.key.parse::<u32>().unwrap() >= 5));
}

#[test]
fn leaving_the_side_shows_both_again_and_co_op_is_never_filtered() {
    let mut lobby = pvp_crowd(1);
    assert_eq!(facts::slot_rows(&lobby).len(), 5);
    lobby.players[0].slot = None;
    lobby.slots[0].holder = None;
    assert_eq!(facts::slot_rows(&lobby).len(), 10);
    assert_eq!(facts::slot_rows(&crowd(1, 10, 3, false)).len(), 10);
}

// ---- the Players list ----

#[test]
fn players_wear_the_light_colour_of_their_side() {
    let rows = facts::player_rows(&pvp_crowd(6));
    assert_eq!(rows[0].tint, Some(tone::LIGHT_BLUE), "Maverick, Bluefor");
    assert_eq!(rows[2].tint, Some(tone::LIGHT_RED), "Pilot3, Redfor");
    assert_eq!(rows[4].tint, None, "Pilot5 has no side");
    // The reader's own row wears its side too (the Slots list has the arrow).
    let own = facts::player_rows(&pvp_crowd(3));
    assert_eq!(own[2].tint, Some(tone::LIGHT_RED));
}

#[test]
fn an_unable_player_is_dimmed_in_pvp_so_red_means_redfor_only() {
    let mut lobby = pvp_crowd(6);
    lobby.players[4].unable = Some("Pilot5's game has no F/A-18D Hornet.".into());
    lobby.players[2].unable = Some("Pilot3's game has no F/A-18D Hornet.".into());
    let rows = facts::player_rows(&lobby);
    assert!(rows[4].dim && rows[2].dim);
    assert_eq!(rows[2].tint, None);
    // Co-op keeps the red.
    let mut coop = sample(1);
    coop.players[1].unable = Some("no".into());
    assert_eq!(
        facts::player_rows(&coop)[1].tint,
        Some(crate::widgets::tone::ENEMY)
    );
}

// ---- the boxes ----

fn refused(why: &str) -> BoxClick {
    BoxClick::Refused(why.to_owned())
}

#[test]
fn no_boxes_in_a_co_op_game() {
    let lobby = sample(1);
    assert!(sides::side_boxes(&lobby, &Facts::of(Some(&lobby), None)).is_none());
    assert!(screen_of(&lobby, true).side_boxes().is_none());
}

#[test]
fn no_side_yet_a_side_with_room_is_lit_and_a_click_joins_it() {
    let b = boxes(&pvp_crowd(6));
    for (i, side) in [(BLUE, Side::Friendly), (RED, Side::Enemy)] {
        assert!(b[i].lit && !b[i].checked, "box {i}");
        assert_eq!(b[i].click, BoxClick::Join(side));
    }
    assert_eq!(
        (b[BLUE].label.as_str(), b[RED].label.as_str()),
        ("Bluefor 2/5", "Redfor 2/5")
    );
}

#[test]
fn no_side_yet_a_full_side_is_greyed_and_says_so() {
    let mut lobby = pvp_crowd(6);
    seat(&mut lobby, 5, 7);
    lobby.slots[8].lock = Lock::Closed;
    lobby.slots[9].reserved = Some("Hawk".into());
    let b = boxes(&lobby);
    assert!(!b[RED].lit && !b[RED].checked);
    assert_eq!(b[RED].click, refused("Redfor is full."));
    assert_eq!(b[RED].label, "Redfor 5/5", "full shows in the count");
    assert!(b[BLUE].lit, "Bluefor still has room");
}

#[test]
fn a_slot_kept_for_the_reader_is_not_counted_as_taken() {
    let mut lobby = pvp_crowd(6);
    seat(&mut lobby, 5, 7);
    lobby.slots[8].lock = Lock::Closed;
    lobby.slots[9].lock = Lock::Reserved("Pilot6".into());
    let b = boxes(&lobby);
    assert!(b[RED].lit, "plane 9 is free to Pilot6");
    assert_eq!(b[RED].label, "Redfor 4/5");
}

#[test]
fn a_side_with_no_slots_players_may_take_says_that() {
    let mut lobby = pvp_crowd(6);
    lobby.slots.retain(|s| s.wing.side == Side::Friendly);
    let b = boxes(&lobby);
    assert!(!b[RED].lit);
    assert_eq!(
        b[RED].click,
        refused("Redfor has no slots players may take.")
    );
}

#[test]
fn on_your_side_the_box_is_checked_and_unchecking_leaves() {
    for (you, own, other) in [(1, BLUE, RED), (3, RED, BLUE)] {
        let b = boxes(&pvp_crowd(you));
        assert!(b[own].checked && b[own].lit, "you {you}");
        assert_eq!(b[own].click, BoxClick::Leave);
        assert!(!b[other].checked && !b[other].lit);
    }
}

#[test]
fn the_other_side_while_on_one_says_to_uncheck_first() {
    let b = boxes(&pvp_crowd(1));
    assert_eq!(b[RED].click, refused("Uncheck Bluefor first."));
    let b = boxes(&pvp_crowd(3));
    assert_eq!(b[BLUE].click, refused("Uncheck Redfor first."));
}

#[test]
fn the_king_who_hosts_uses_the_boxes_like_anyone() {
    let lobby = pvp_crowd(1);
    assert!(lobby.is_king());
    let mut screen = screen_of(&lobby, true);
    let at = screen.side_hit(0);
    assert_eq!(
        click(&mut screen, (at.0 + 5, at.1 + 5)),
        Some(Request::LeaveSlot)
    );
    // Without a slot the King joins a side by the box.
    let mut free = pvp_crowd(1);
    free.players[0].slot = None;
    free.slots[0].holder = None;
    let mut screen = screen_of(&free, true);
    let at = screen.side_hit(1);
    assert_eq!(
        click(&mut screen, (at.0 + 5, at.1 + 5)),
        Some(Request::Side(Side::Enemy))
    );
}

#[test]
fn balanced_greys_both_and_checks_the_assigned_side() {
    let mut lobby = pvp_crowd(3);
    set_rule(&mut lobby, 2);
    let b = boxes(&lobby);
    assert!(b[RED].checked && !b[BLUE].checked);
    for side_box in &b {
        assert!(!side_box.lit);
        assert_eq!(side_box.click, refused("Autobalance picks the sides."));
    }
    // The list follows the side held.
    assert_eq!(facts::slot_rows(&lobby).len(), 5);
}

#[test]
fn balanced_with_no_slot_checks_neither_and_the_hint_says_why() {
    let mut lobby = pvp_crowd(6);
    set_rule(&mut lobby, 2);
    let facts = Facts::of(Some(&lobby), None);
    let b = boxes(&lobby);
    assert!(b.iter().all(|x| !x.checked && !x.lit));
    assert_eq!(
        facts::hint(&facts),
        "Both sides are full: Autobalance seats you when a slot frees."
    );
    assert_eq!(sides::note(&facts), ["Balanced by the host", "Balanced"]);
}

#[test]
fn balanced_refuses_leaving_by_the_slot_too() {
    let mut lobby = pvp_crowd(3);
    set_rule(&mut lobby, 2);
    let facts = Facts::of(Some(&lobby), None);
    assert_eq!(
        facts::slot_click(&lobby, &facts, 5),
        SlotClick::Refused("Autobalance picks the sides.".into())
    );
    // A free slot of the reader's own side still moves it there.
    assert_eq!(facts::slot_click(&lobby, &facts, 7), SlotClick::Take(7));
    // And unlocked, one's own slot leaves.
    set_rule(&mut lobby, 1);
    let facts = Facts::of(Some(&lobby), None);
    assert_eq!(facts::slot_click(&lobby, &facts, 5), SlotClick::Leave);
}

#[test]
fn locked_in_flight_greys_both_and_shows_the_side_flown() {
    let mut lobby = pvp_crowd(3);
    lobby.phase = LobbyPhase::Flying;
    lobby.players[2].flying = true;
    let facts = Facts::of(Some(&lobby), None);
    let b = boxes(&lobby);
    assert!(b[RED].checked && !b[BLUE].checked);
    for side_box in &b {
        assert!(!side_box.lit);
        assert_eq!(
            side_box.click,
            refused("Sides are locked until the mission ends.")
        );
    }
    assert_eq!(sides::note(&facts), ["Locked for this mission", "Locked"]);
}

#[test]
fn free_sides_in_flight_still_make_a_flying_player_leave_the_aircraft_first() {
    let mut lobby = pvp_crowd(3);
    set_rule(&mut lobby, 0);
    lobby.phase = LobbyPhase::Flying;
    lobby.players[2].flying = true;
    let b = boxes(&lobby);
    assert!(b[RED].checked && !b[RED].lit);
    assert_eq!(
        b[BLUE].click,
        refused("Leave your aircraft before you change your slot.")
    );
}

#[test]
fn a_late_joiner_with_no_aircraft_yet_may_still_pick_a_side() {
    let mut lobby = pvp_crowd(6);
    lobby.phase = LobbyPhase::Flying;
    let facts = Facts::of(Some(&lobby), None);
    assert!(
        sides::note(&facts).is_empty(),
        "not flying yet, so the choice is open"
    );
    assert!(boxes(&lobby)[BLUE].lit);
}

#[test]
fn a_game_that_cannot_play_the_mission_sees_both_sides_greyed() {
    let lobby = pvp_crowd(6);
    let why = "Your game has no F/A-18D Hornet, which this mission flies.";
    let b = sides::side_boxes(&lobby, &Facts::of(Some(&lobby), Some(why))).unwrap();
    for side_box in &b {
        assert!(!side_box.lit && !side_box.checked);
        assert_eq!(side_box.click, refused(why));
    }
}

// ---- the words ----

#[test]
fn the_hint_for_a_player_with_no_side_names_the_boxes_in_pvp_only() {
    let facts = Facts::of(Some(&pvp_crowd(6)), None);
    assert_eq!(
        facts::hint(&facts),
        "Check Bluefor or Redfor to join a side, or click a free slot."
    );
    let mut coop = sample(2);
    coop.players[1].slot = None;
    coop.slots[1].holder = None;
    assert_eq!(
        facts::hint(&Facts::of(Some(&coop), None)),
        "Click a free slot to take it, then choose a loadout and press Ready."
    );
}

#[test]
fn the_rules_line_says_sides_locked_or_balanced() {
    let mut lobby = pvp_crowd(1);
    assert!(
        facts::settings_summary(&lobby)
            .unwrap()
            .ends_with("sides locked")
    );
    set_rule(&mut lobby, 2);
    assert!(
        facts::settings_summary(&lobby)
            .unwrap()
            .ends_with("sides balanced")
    );
    set_rule(&mut lobby, 0);
    let free = facts::settings_summary(&lobby).unwrap();
    assert!(
        !free.contains("sides locked") && !free.contains("sides balanced"),
        "{free}"
    );
}

#[test]
fn the_settings_panel_names_the_row_sides_with_the_players_words() {
    use settings_panel::{Context, Kind, row_label, row_value};
    let row = Kind::Setting(number::LOCK_SIDES);
    assert_eq!(row_label(row), "Sides");
    let mut lobby = pvp_crowd(1);
    for (value, words) in [
        (0, "Free"),
        (1, "Locked once flown"),
        (2, "Balanced by the host"),
    ] {
        set_rule(&mut lobby, value);
        assert_eq!(row_value(row, &Context::of(&lobby, None)), words);
    }
    assert_eq!(sides::rule(&lobby), Sides::Balanced);
}

// ---- the screen ----

#[test]
fn a_click_on_a_lit_box_asks_for_the_side_and_waits_for_the_host() {
    let lobby = pvp_crowd(6);
    let mut screen = screen_of(&lobby, false);
    let at = screen.side_hit(0);
    assert_eq!(
        click(&mut screen, (at.0 + 5, at.1 + 5)),
        Some(Request::Side(Side::Friendly))
    );
    // The lamp does not turn until the host's answer puts the player on it.
    assert!(!screen.side_check[0].checked());
    let mut seated = lobby;
    seat(&mut seated, 6, 3);
    screen.update(Some(&seated), None);
    assert!(screen.side_check[0].checked());
    assert_eq!(
        screen.slots.rows().len(),
        5,
        "the list is filtered to Bluefor"
    );
}

#[test]
fn a_click_on_a_greyed_box_says_why_and_sends_nothing() {
    let mut screen = screen_of(&pvp_crowd(1), true);
    let at = screen.side_hit(1);
    assert_eq!(click(&mut screen, (at.0 + 5, at.1 + 5)), None);
    assert_eq!(
        screen.message_lines().last().map(String::as_str),
        Some("Uncheck Bluefor first.")
    );
    // Letting go off the box does nothing.
    screen.moved(Some((f64::from(at.0 + 5), f64::from(at.1 + 5))));
    screen.button(true);
    screen.moved(Some((300.0, 300.0)));
    assert_eq!(screen.button(false), None);
}

#[test]
fn unchecking_your_side_by_the_box_leaves_the_slot() {
    let mut screen = screen_of(&pvp_crowd(3), false);
    let at = screen.side_hit(1);
    assert_eq!(
        click(&mut screen, (at.0 + 5, at.1 + 5)),
        Some(Request::LeaveSlot)
    );
}

#[test]
fn tab_reaches_the_boxes_that_can_be_used_and_space_toggles() {
    let lobby = pvp_crowd(6);
    let mut screen = screen_of(&lobby, false);
    screen.focus.set(Id::Slots);
    assert_eq!(screen.key("Tab", false), None);
    assert!(screen.focus.is(Id::BlueBox));
    assert_eq!(
        screen.key("Space", false),
        Some(Request::Side(Side::Friendly))
    );
    screen.key("Tab", false);
    assert!(screen.focus.is(Id::RedBox));
    assert_eq!(screen.key("Space", false), Some(Request::Side(Side::Enemy)));
    screen.key("Tab", false);
    assert!(screen.focus.is(Id::Players));
    // A greyed box is skipped.
    let mut screen = screen_of(&pvp_crowd(1), false);
    screen.focus.set(Id::Slots);
    screen.key("Tab", false);
    assert!(screen.focus.is(Id::BlueBox));
    screen.key("Tab", false);
    assert!(screen.focus.is(Id::Players), "Redfor is greyed");
    // In co-op both are skipped.
    let mut coop = screen_of(&sample(1), true);
    coop.focus.set(Id::Slots);
    coop.key("Tab", false);
    assert!(coop.focus.is(Id::Players));
}

#[test]
fn the_boxes_draw_in_pvp_only_and_greyed_ones_are_dimmer() {
    let coop = drawn(&screen_of(&sample(1), true));
    let open = drawn(&screen_of(&pvp_crowd(6), false));
    assert!(
        differing(&coop, &open, layout::SIDE_BOXES) > 0,
        "PvP draws the boxes"
    );
    let mut full = pvp_crowd(6);
    seat(&mut full, 5, 7);
    full.slots[8].lock = Lock::Closed;
    full.slots[9].lock = Lock::Closed;
    let greyed = drawn(&screen_of(&full, false));
    let red = (layout::BOXES[1].0, layout::BOXES[1].1, 100, 28);
    assert!(
        differing(&open, &greyed, red) > 0,
        "a full Redfor looks different"
    );
    let blue = (layout::BOXES[0].0, layout::BOXES[0].1, 105, 28);
    assert_eq!(
        differing(&open, &greyed, blue),
        0,
        "Bluefor still has room, so it is the same"
    );
}

#[test]
fn a_filled_row_paints_its_well_in_the_sides_colour() {
    let pixels = drawn(&screen_of(&pvp_crowd(1), true));
    // Row 0 is the King's own Bluefor slot, row 1 Pilot2's, row 2 open.
    let (x, y) = (layout::SLOTS_LIST.0 + 4, layout::SLOTS_LIST.1 + 8);
    assert_eq!(test_kit::at(&pixels, x, y), tone::ROYAL_BLUE);
    assert_eq!(test_kit::at(&pixels, x, y + 18), tone::ROYAL_BLUE);
    assert_ne!(test_kit::at(&pixels, x, y + 36), tone::ROYAL_BLUE);
    let red = drawn(&screen_of(&pvp_crowd(3), false));
    assert_eq!(test_kit::at(&red, x, y), tone::BRIGHT_RED);
}
