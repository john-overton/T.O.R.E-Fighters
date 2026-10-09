//! Tests of the lobby screen's layout and its ready hint (lobby pass, slice
//! L2): the head lines that are gone, the scroll bars on Slots, Players and
//! Messages, where everything sits, and what the line above the buttons says
//! in each state. Built on the kit of synthetic pieces.
use super::facts::{self, Facts};
use super::preview::{crowd, sample, sample_pvp, slots};
use super::*;
use crate::widgets::{Rect, test_kit};
use tore_session::wire::messages::{LobbyPhase, StartRule};

fn screen_of(lobby: &LobbyState, hosting: bool) -> LobbyScreen {
    LobbyScreen::sample(Arc::new(test_kit::kit()), lobby.clone(), hosting)
}

fn drawn(screen: &LobbyScreen) -> Vec<u8> {
    let mut pixels = test_kit::blank();
    screen.draw(&mut Canvas(&mut pixels));
    pixels
}

/// How many pixels of `rect` differ between two pictures.
fn differing(a: &[u8], b: &[u8], (x, y, w, h): Rect) -> usize {
    let mut n = 0;
    for py in y..y + h {
        for px in x..x + w {
            if test_kit::at(a, px, py) != test_kit::at(b, px, py) {
                n += 1;
            }
        }
    }
    n
}

fn move_to(screen: &mut LobbyScreen, at: Point) {
    screen.moved(Some((f64::from(at.0), f64::from(at.1))));
}

fn click(screen: &mut LobbyScreen, at: Point) -> Option<Request> {
    move_to(screen, at);
    let down = screen.button(true);
    let up = screen.button(false);
    down.or(up)
}

/// A co-op lobby with `slots` planes and `players` players, the King holding
/// the first plane.
fn crowd_of(slots: u32, players: u8) -> LobbyState {
    crowd(1, slots, players, false)
}

// ---- where everything sits ----

#[test]
fn the_lists_and_their_bars_fit_inside_their_frames_and_the_screen() {
    let outer = (30, 100, 579, 355);
    let inside_rect = |inner: Rect, outer: Rect| {
        inner.0 >= outer.0
            && inner.1 >= outer.1
            && inner.0 + inner.2 <= outer.0 + outer.2
            && inner.1 + inner.3 <= outer.1 + outer.3
    };
    // A scroll bar is 34 wide and as high as five rows (89).
    let slots_list = (
        layout::SLOTS_LIST.0,
        layout::SLOTS_LIST.1,
        layout::SLOTS_WIDTH,
        89,
    );
    let slots_bar = (layout::SLOTS_BAR.0, layout::SLOTS_BAR.1, 34, 89);
    let players_list = (
        layout::PLAYERS_LIST.0,
        layout::PLAYERS_LIST.1,
        layout::PLAYERS_WIDTH,
        89,
    );
    let players_bar = (layout::PLAYERS_BAR.0, layout::PLAYERS_BAR.1, 34, 89);
    for (list, bar, frame) in [
        (slots_list, slots_bar, layout::SLOTS_FRAME),
        (players_list, players_bar, layout::PLAYERS_FRAME),
    ] {
        assert!(inside_rect(list, frame), "the rows sit in their frame");
        assert!(inside_rect(bar, frame), "the bar sits in the same frame");
        assert!(list.0 + list.2 <= bar.0, "the bar is beside the rows");
        // The bar fills the frame's inside to its right edge, give or take the
        // line.
        assert!(frame.0 + frame.2 - (bar.0 + bar.2) <= 6);
    }
    assert!(inside_rect(layout::SLOTS_FRAME, outer));
    assert!(inside_rect(layout::PLAYERS_FRAME, outer));
    // The two frames do not touch.
    assert!(layout::SLOTS_FRAME.0 + layout::SLOTS_FRAME.2 < layout::PLAYERS_FRAME.0);
    // The Messages box: its bar is flush right inside the box (its own L1
    // numbers), and the box is inside the frame.
    assert!(inside_rect(layout::MESSAGES, outer));
}

#[test]
fn the_head_runs_game_then_rules_with_room_for_the_side_boxes_below() {
    // Game at 102, Rules at 115 (13 apart, as the head's lines were).
    assert_eq!(layout::RULES.1 - 102, 13);
    // The side boxes' row is under the Rules line (a text line is 12 high)
    // and over the Slots frame, with the Slots heading on it.
    let (_, y, _, h) = layout::SIDE_BOXES;
    assert!(y >= layout::RULES.1 + 12, "the boxes clear the Rules line");
    assert!(y + h <= layout::SLOTS_FRAME.1, "and end above the frame");
    assert!(
        (y..y + h).contains(&layout::SLOTS_HEADING.1),
        "the heading is on the boxes' row"
    );
    // A retail check box is 28 high; the row holds one.
    assert!(h >= 28);
    // The row starts to the right of the heading and ends inside the frame.
    assert!(layout::SIDE_BOXES.0 > layout::SLOTS_HEADING.0 + 40);
    assert!(
        layout::SIDE_BOXES.0 + layout::SIDE_BOXES.2
            <= layout::SLOTS_FRAME.0 + layout::SLOTS_FRAME.2
    );
}

#[test]
fn the_bottom_rows_do_not_overlap() {
    // Chat line (18 high), ready hint (a dim line), then the buttons, whose
    // blue default is raised to 416.
    let chat_end = layout::CHAT_LINE.1 + 18;
    assert!(layout::READY_HINT.1 >= chat_end);
    assert!(layout::READY_HINT.1 + 13 <= BUTTON_Y - 3);
    assert!(layout::MESSAGES.1 + layout::MESSAGES.3 < layout::CHAT_LINE.1);
}

#[test]
fn every_button_is_in_the_same_place_for_everyone() {
    // The King's seven places, and the joiner's and the server's five with
    // Mission... in the third and Leave in the last.
    let king = screen_of(&sample(1), true);
    let joiner = screen_of(&sample(2), false);
    for id in [Id::Settings, Id::Loadout, Id::Ready, Id::Leave] {
        assert_eq!(
            king.button_ref(id).bounds().1,
            BUTTON_Y - 3,
            "{id:?} sits on the row"
        );
    }
    assert_eq!(king.button_ref(Id::Mission).bounds().0, SLOT_X[0]);
    assert_eq!(joiner.button_ref(Id::Mission).bounds().0, SLOT_X[2]);
    assert_eq!(joiner.button_ref(Id::Settings).bounds().0, SLOT_X[3]);
    assert_eq!(joiner.button_ref(Id::Leave).bounds().0, SLOT_X[6]);
    assert_eq!(king.button_ref(Id::Leave).bounds().0, SLOT_X[6]);
    // Mission... is shown, enabled, and pressable for the joiner.
    let mut joiner = joiner;
    let at = joiner.button_ref(Id::Mission).bounds();
    assert_eq!(
        click(&mut joiner, (at.0 + 20, at.1 + 10)),
        Some(Request::Mission),
        "the joiner presses Mission... in the bottom row"
    );
}

// ---- the removed lines ----

#[test]
fn the_head_no_longer_shows_the_mission_or_the_kings_rule() {
    let state = sample(1);
    let mut other = state.clone();
    other.summary = "A different mission summary that is long enough to be seen".into();
    other.players[0].callsign = "Iceman".into();
    other.players[0].loadout = true;
    let a = drawn(&screen_of(&state, true));
    let b = drawn(&screen_of(&other, true));
    // The two lines' places, from the Game line to the Slots heading: the
    // only difference allowed is the Players list (the other callsign) and
    // the slots' rows, below.
    let head = (30, 101, 579, 62);
    assert_eq!(
        differing(&a, &b, head),
        0,
        "the summary and the King's name are read nowhere in the head"
    );
}

#[test]
fn the_rules_line_stays_and_moves_up() {
    let mut state = sample(1);
    let before = drawn(&screen_of(&state, true));
    // PvP settings say more in Rules than co-op's do.
    state = sample_pvp(1);
    let after = drawn(&screen_of(&state, true));
    let rules_row = (45, layout::RULES.1, 549, 12);
    assert!(
        differing(&before, &after, rules_row) > 0,
        "the Rules line draws at its new place"
    );
    // Nothing of it remains at its old place (only its first 54 pixels: PvP
    // draws its side boxes on the Slots heading's row, beside the heading).
    assert_eq!(differing(&before, &after, (45, 141, 54, 9)), 0);
}

// ---- the scroll bars ----

#[test]
fn slots_scroll_with_the_wheel_the_track_and_the_knob() {
    let state = crowd_of(12, 3);
    let mut screen = screen_of(&state, true);
    assert_eq!(screen.slots.first_row(), 0);
    assert_eq!(screen.slots.visible_rows(), 5);
    // The wheel over the list slides the window by the notch's rows and does
    // not select anything.
    move_to(&mut screen, (100, 200));
    screen.wheel(-1);
    assert_eq!(screen.slots.first_row(), layout::WHEEL_ROWS);
    assert_eq!(screen.slots.selected(), None);
    screen.wheel(1);
    assert_eq!(screen.slots.first_row(), 0);
    // The wheel over the bar's art works too.
    move_to(&mut screen, (350, 200));
    screen.wheel(-2);
    assert_eq!(screen.slots.first_row(), 2 * layout::WHEEL_ROWS);
    // A press on the track below the knob pages down: to the end, 7 rows
    // (12 rows, 5 shown).
    click(&mut screen, (350, 250));
    assert_eq!(screen.slots.first_row(), 7);
    // And on the track above it pages back up.
    click(&mut screen, (350, 176));
    assert_eq!(screen.slots.first_row(), 2);
    // The knob's drag: grab it (the window is at row 2; the knob's top is
    // 6 + 48 * 2 / 7 below the bar's top) and take it to the bottom.
    let grab = (355, 168 + 6 + 14 + 5);
    move_to(&mut screen, grab);
    screen.button(true);
    move_to(&mut screen, (355, 168 + 89));
    assert_eq!(screen.slots.first_row(), 7, "the knob dragged to the end");
    move_to(&mut screen, (500, 100));
    assert_eq!(
        screen.slots.first_row(),
        0,
        "and to the top, off to the side"
    );
    // Letting go ends the drag.
    screen.button(false);
    move_to(&mut screen, (355, 168 + 89));
    assert_eq!(screen.slots.first_row(), 0, "released: the knob stays put");
}

#[test]
fn a_press_on_the_slots_bar_does_not_take_a_slot() {
    let state = crowd_of(12, 3);
    let mut screen = screen_of(&state, true);
    // Plane 5 is below the window; the bar's track is not a row.
    assert_eq!(click(&mut screen, (350, 250)), None);
    assert_eq!(screen.slots.selected(), None);
    // The track paged the window five rows, so the third row is plane 7.
    assert_eq!(screen.slots.first_row(), 5);
    let request = click(&mut screen, (100, 168 + 36 + 8));
    assert_eq!(request, Some(Request::Take(7)));
}

#[test]
fn a_cancelled_press_lets_go_of_a_held_knob() {
    let state = crowd_of(12, 3);
    let mut screen = screen_of(&state, true);
    move_to(&mut screen, (355, 168 + 6 + 5));
    screen.button(true);
    assert!(screen.slots.dragging());
    screen.cancel_press();
    assert!(!screen.slots.dragging());
}

#[test]
fn players_scroll_past_their_five_rows() {
    let state = crowd_of(6, 9);
    let mut screen = screen_of(&state, true);
    assert_eq!(screen.players.visible_rows(), 5);
    assert_eq!(screen.players.first_row(), 0);
    move_to(&mut screen, (450, 200));
    screen.wheel(-1);
    assert_eq!(screen.players.first_row(), layout::WHEEL_ROWS);
    // The track below the knob pages to the end: 9 rows, 4 past the first
    // window.
    click(&mut screen, (580, 250));
    assert_eq!(screen.players.first_row(), 4);
    // The knob drags too.
    move_to(&mut screen, (585, 168 + 6 + 48 + 5));
    screen.button(true);
    move_to(&mut screen, (585, 100));
    assert_eq!(screen.players.first_row(), 0);
    screen.button(false);
    // A click on a row selects the player: row 2 of the window is Pilot3.
    click(&mut screen, (450, 168 + 36 + 8));
    assert_eq!(screen.players.selected_row().unwrap().key, "3");
}

#[test]
fn the_bars_are_drawn_even_when_everything_fits() {
    // The track is there so the layout does not shift; the knob only when
    // the content does not fit.
    let few = drawn(&screen_of(&crowd_of(5, 1), true));
    let many = drawn(&screen_of(&crowd_of(12, 9), true));
    // Beside the slot rows, and beside the player rows.
    for rect in [(340, 170, 26, 85), (574, 170, 26, 85)] {
        assert!(
            differing(&few, &many, rect) > 0,
            "the knob shows when the list scrolls ({rect:?})"
        );
    }
    // No pager art is left where PREV, NEXT and PAGE were: the same picture
    // with one slot or twelve differs only by the rows, the bar and the box.
    let none = drawn(&screen_of(&crowd_of(1, 1), true));
    let more = drawn(&screen_of(&crowd_of(5, 1), true));
    assert_eq!(
        differing(&none, &more, (336, 168, 40, 94)),
        0,
        "a list that fits draws no knob and no page box"
    );
}

#[test]
fn messages_have_a_scroll_bar_that_takes_the_pointer() {
    let mut screen = screen_of(&sample(1), true);
    for n in 0..30 {
        screen.say(&format!("Line {n}"));
    }
    let bar_x = layout::MESSAGES.0 + layout::MESSAGES.2 - 20;
    // Grab the knob (at the bottom, for the newest lines) and drag it up.
    let (top, bottom) = (layout::MESSAGES.1, layout::MESSAGES.1 + layout::MESSAGES.3);
    move_to(&mut screen, (bar_x, bottom - 10));
    screen.button(true);
    assert!(screen.chat.messages.dragging());
    move_to(&mut screen, (bar_x, top));
    assert!(screen.chat.messages.scrolled_back() > 0);
    screen.button(false);
    assert!(!screen.chat.messages.dragging());
}

// ---- the ready hint ----

fn hint_of(state: &LobbyState, unable: Option<&str>) -> Option<&'static str> {
    facts::ready_hint(&Facts::of(Some(state), unable))
}

#[test]
fn a_joiner_with_no_slot_is_told_to_take_one_and_press_ready() {
    let mut state = sample(2);
    state.players[1].slot = None;
    state.players[1].ready = false;
    state.slots = slots(&[(0, 1)]);
    assert_eq!(
        hint_of(&state, None),
        Some("Take a slot and press Ready so the King can start the mission.")
    );
}

#[test]
fn a_joiner_holding_a_slot_is_told_to_press_ready() {
    let mut state = sample(2);
    state.players[1].ready = false;
    assert_eq!(
        hint_of(&state, None),
        Some("Press Ready so the King can start the mission.")
    );
}

#[test]
fn a_ready_joiner_is_told_the_king_starts_it() {
    let state = sample(2);
    assert!(state.players[1].ready);
    assert_eq!(
        hint_of(&state, None),
        Some("You are ready. The King starts the mission with Fly.")
    );
}

#[test]
fn the_king_is_told_to_press_fly_when_everyone_is_ready() {
    let mut state = sample(1);
    assert_eq!(
        hint_of(&state, None),
        Some("Press Fly when everyone holding a slot is ready.")
    );
    state.players[0].ready = true;
    assert_eq!(
        hint_of(&state, None),
        Some("Everyone is ready: press Fly to start the mission.")
    );
    // The King with no slot at all waits like any King.
    state.players[0].slot = None;
    state.players[0].ready = false;
    state.slots = slots(&[(1, 2)]);
    assert_eq!(
        hint_of(&state, None),
        Some("Everyone is ready: press Fly to start the mission.")
    );
}

#[test]
fn a_game_that_cannot_play_the_mission_says_so_in_every_state() {
    for you in [1, 2] {
        let state = sample(you);
        assert_eq!(
            hint_of(&state, Some("Your game has no F-14D Tomcat.")),
            Some("Your game cannot play this mission."),
            "the reason stays in the hint line above"
        );
    }
}

#[test]
fn a_flying_mission_has_its_own_words() {
    let mut state = sample(2);
    state.phase = LobbyPhase::Flying;
    state.players[1].slot = None;
    state.players[1].ready = false;
    assert_eq!(
        hint_of(&state, None),
        Some("The mission is flying: take a slot and press Join.")
    );
    state.players[1].slot = Some(1);
    assert_eq!(
        hint_of(&state, None),
        Some("The mission is flying: press Join to take your aircraft in.")
    );
    state.players[1].flying = true;
    assert_eq!(hint_of(&state, None), Some("You are flying the mission."));
    state.players[1].flying = false;
    state.players[1].observing = true;
    assert_eq!(hint_of(&state, None), Some("You are watching the mission."));
    // The King reads the flying words as anyone does.
    let mut king = sample(1);
    king.phase = LobbyPhase::Flying;
    king.players[0].slot = None;
    assert_eq!(
        hint_of(&king, None),
        Some("The mission is flying: take a slot and press Join.")
    );
}

#[test]
fn a_server_says_what_starts_its_mission() {
    let mut state = sample(2);
    state.king = None;
    state.host = None;
    state.start = StartRule::FirstReady;
    assert_eq!(
        hint_of(&state, None),
        Some("The mission starts as soon as the first player holding a slot is ready.")
    );
    state.start = StartRule::Flying;
    assert_eq!(
        hint_of(&state, None),
        Some("This server's mission is always flying: take a slot and press Ready to join it.")
    );
    // No King and a King's rule: nobody to name.
    state.start = StartRule::King;
    assert_eq!(
        hint_of(&state, None),
        Some("The mission starts when the King presses Fly and everyone holding a slot is ready.")
    );
}

#[test]
fn there_is_no_hint_before_the_lobby_arrives_or_after_the_mission() {
    assert_eq!(facts::ready_hint(&Facts::of(None, None)), None);
    let mut state = sample(2);
    state.phase = LobbyPhase::Ended;
    assert_eq!(hint_of(&state, None), None);
}

#[test]
fn the_hint_is_drawn_in_the_dim_face_above_the_buttons_and_follows_the_state() {
    let mut state = sample(2);
    state.players[1].ready = false;
    let waiting = drawn(&screen_of(&state, false));
    let hint_row = (45, layout::READY_HINT.1, 549, 12);
    let none = drawn(&LobbyScreen::sample(
        Arc::new(test_kit::kit()),
        {
            let mut ended = state.clone();
            ended.phase = LobbyPhase::Ended;
            ended
        },
        false,
    ));
    assert!(
        differing(&waiting, &none, hint_row) > 0,
        "the hint is drawn on its row"
    );
    // A ready joiner reads other words on the same row.
    state.players[1].ready = true;
    let ready = drawn(&screen_of(&state, false));
    assert!(differing(&waiting, &ready, hint_row) > 0);
    // Nothing is drawn between the hint's row and the chat line's bottom,
    // so the hint does not run into the chat box.
    assert_eq!(
        differing(&waiting, &none, (45, layout::CHAT_LINE.1, 549, 22)),
        0
    );
}

#[test]
fn a_long_hint_is_cut_to_the_text_width() {
    let state = sample(1);
    let screen = screen_of(&state, true);
    let pixels = drawn(&screen);
    // Nothing of the hint runs past 549 pixels from its left edge.
    let beyond = layout::READY_HINT.0 + layout::TEXT_WIDTH;
    let probe = |x: i32| test_kit::at(&pixels, x, layout::READY_HINT.1 + 6);
    assert_eq!(probe(beyond + 2), probe(beyond + 20));
}

#[test]
fn the_slots_and_players_headings_share_a_row_over_their_frames() {
    assert_eq!(layout::SLOTS_HEADING.1, layout::PLAYERS_HEADING.1);
    assert!(layout::PLAYERS_HEADING.0 >= layout::PLAYERS_FRAME.0);
    assert!(layout::SLOTS_HEADING.1 + 12 <= layout::SLOTS_FRAME.1);
}
