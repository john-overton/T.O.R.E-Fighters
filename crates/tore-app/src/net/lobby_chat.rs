//! The lobby's chat (slice EF6), for the lobby screen (EF8) to place: the
//! Messages box with chat and system lines in their colours, and the chat
//! line under it. Enter sends to All, the only receiver in the lobby (as in
//! retail). It is the kit's [`MessageBox`] and [`TextField`] and nothing
//! else, so it takes the same keys and draws in the same retail pieces.
//!
//! How a screen uses it:
//!
//! ```ignore
//! // Once, with the screen's rectangles: NEWNET's Messages box and a chat line.
//! let mut chat = LobbyChat::new((45, 300, 549, 78), (45, 380), 54);
//! // Each session event:
//! ClientEvent::Chat(line) => chat.push(&kit, &line),
//! // The screen's own words (a player joined, a request was refused):
//! chat.system(&kit, "Slider joined the game.");
//! // Keys and text go to the focused part:
//! Id::Line => { chat.field.key(name); chat.field.text_input(text); }
//! Id::Messages => { chat.messages.key(name); }
//! // Enter in the line (the field answers Outcome::Activated):
//! chat.send(&kit, &mut session.client);
//! // Draw it back to front with the rest, passing the focus:
//! chat.draw(canvas, &kit, focus.marked(Id::Messages), focus.marked(Id::Line));
//! ```
//!
//! The box keeps 200 lines and scrolls back; the line holds up to 80
//! printable ASCII characters ([`tore_session::wire::chat::MAX_TEXT`]).
use crate::menu::Canvas;
use crate::net::chat::colour;
use crate::widgets::{Filter, Kit, MessageBox, Outcome, Point, Rect, TextField, tone};
use tore_session::Client;
use tore_session::wire::chat::{ChatLine, MAX_TEXT, Receiver};

/// The Messages box and the chat line under it.
#[derive(Clone, Debug)]
pub struct LobbyChat {
    /// The scrolling box; a screen gives it the focus and its keys.
    pub messages: MessageBox,
    /// The chat line: retail's edit control, up to 80 characters.
    pub field: TextField,
}

impl LobbyChat {
    /// A Messages box at `messages` and a chat line whose edit control is at
    /// `field`, showing `chars` characters at once (ten pixels each plus 16,
    /// 24 high; the text scrolls sideways past that).
    pub fn new(messages: Rect, field: Point, chars: i32) -> Self {
        Self {
            messages: MessageBox::new(messages),
            field: TextField::edit(field, chars, Filter::Text)
                .with_max(MAX_TEXT)
                .with_hint("type a message, Enter sends to all"),
        }
    }

    /// A chat line from the session ([`tore_session::ClientEvent::Chat`]),
    /// in the colour of its sender and receiver.
    pub fn push(&mut self, kit: &Kit, line: &ChatLine) {
        self.messages
            .push(kit, &line.display(), colour(line.tone()));
    }

    /// A line of the screen's own, in the system colour.
    pub fn system(&mut self, kit: &Kit, text: &str) {
        self.messages.push(kit, text, tone::SYSTEM);
    }

    /// The line's text taken for sending: trimmed, and the field emptied.
    /// `None` when there is nothing to send.
    pub fn take_text(&mut self) -> Option<String> {
        let text = self.field.text().trim().to_owned();
        self.field.set_text("");
        (!text.is_empty()).then_some(text)
    }

    /// Enter in the line: sends it to All through `client`. The host sends
    /// the line back (`YOU TO ALL: ...`) and the box shows it when the
    /// session event arrives; a line the game refuses (not connected) is
    /// said in the box at once.
    pub fn send(&mut self, kit: &Kit, client: &mut Client) {
        if let Some(text) = self.take_text()
            && let Err(refusal) = client.chat(Receiver::All, &text)
        {
            self.system(kit, refusal.text());
        }
    }

    /// Key `name` for the line, which answers `Activated` on Enter.
    pub fn key(&mut self, name: &str) -> Outcome {
        self.field.key(name)
    }

    /// Typed text for the line.
    pub fn text_input(&mut self, typed: &str) -> Outcome {
        self.field.text_input(typed)
    }

    /// Draws the box and the line; `box_focus` and `line_focus` say which
    /// has the keyboard.
    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit, box_focus: bool, line_focus: bool) {
        self.messages.draw(canvas, kit, box_focus);
        self.field.draw(canvas, kit, line_focus);
    }
}

/// The colour a line shows in the box, for a screen that draws its own.
pub fn line_colour(line: &ChatLine) -> [u8; 3] {
    colour(line.tone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::test_kit;
    use tore_session::wire::chat::{ChatFrom, Standing, Tone};

    fn line(standing: Standing, receiver: Receiver, you: bool, text: &str) -> ChatLine {
        ChatLine {
            from: ChatFrom::Player {
                callsign: "Slider".into(),
                standing,
                you,
            },
            receiver,
            text: text.into(),
            sound: None,
        }
    }

    #[test]
    fn lines_go_in_the_box_in_their_colours() {
        let kit = test_kit::kit();
        let mut chat = LobbyChat::new((45, 300, 549, 78), (45, 380), 54);
        chat.push(
            &kit,
            &line(Standing::Neutral, Receiver::All, false, "Hello all"),
        );
        chat.push(
            &kit,
            &line(Standing::Neutral, Receiver::All, true, "Hello back"),
        );
        chat.system(&kit, "Slider joined the game.");
        chat.push(
            &kit,
            &ChatLine::system("Chat takes letters, digits and punctuation only."),
        );
        let lines: Vec<_> = chat.messages.lines().collect();
        assert_eq!(lines[0], ("Slider TO ALL: Hello all", tone::ALL));
        assert_eq!(lines[1], ("YOU TO ALL: Hello back", tone::ALL));
        assert_eq!(lines[2], ("Slider joined the game.", tone::SYSTEM));
        assert_eq!(lines[3].1, tone::SYSTEM);
        assert_eq!(
            line_colour(&line(Standing::Own, Receiver::Wing, false, "x")),
            tone::OWN_SIDE
        );
        assert_eq!(
            line_colour(&line(Standing::Enemy, Receiver::All, false, "x")),
            tone::ENEMY
        );
        assert_eq!(colour(Tone::System), tone::SYSTEM);
    }

    #[test]
    fn the_line_takes_80_characters_and_enter_hands_them_over_trimmed() {
        let mut chat = LobbyChat::new((45, 300, 549, 78), (45, 380), 54);
        assert_eq!(chat.take_text(), None);
        chat.text_input("  Ready when you are ");
        chat.text_input(&"x".repeat(100));
        assert_eq!(chat.field.text().chars().count(), MAX_TEXT);
        assert_eq!(chat.key("Enter"), Outcome::Activated);
        let text = chat.take_text().expect("text");
        assert!(text.starts_with("Ready when you are"));
        assert!(text.chars().count() <= MAX_TEXT);
        assert!(chat.field.is_empty(), "the line empties when it is taken");
        // Spaces alone are nothing.
        chat.text_input("    ");
        assert_eq!(chat.take_text(), None);
        // Characters outside printable ASCII never get in.
        chat.text_input("caf\u{e9}\u{7}!");
        assert_eq!(chat.field.text(), "caf!");
    }

    #[test]
    fn drawing_puts_the_text_in_its_colour_in_the_box_and_the_line_under_it() {
        let kit = test_kit::kit();
        let mut chat = LobbyChat::new((45, 300, 549, 78), (45, 380), 54);
        chat.push(&kit, &line(Standing::Enemy, Receiver::All, false, "Boo"));
        chat.text_input("typed");
        let mut pixels = test_kit::blank();
        chat.draw(&mut Canvas(&mut pixels), &kit, false, true);
        // The synthetic font lights every glyph pixel: the enemy colour shows
        // in the box, and something is drawn in the field's rectangle.
        let lit = |rect: Rect, colour: [u8; 3]| {
            (rect.1..rect.1 + rect.3)
                .any(|y| (rect.0..rect.0 + rect.2).any(|x| test_kit::at(&pixels, x, y) == colour))
        };
        assert!(lit((45, 300, 549, 78), tone::ENEMY));
        assert!(
            lit((45, 380, 556, 24), [255, 255, 255]),
            "the typed text and caret"
        );
    }

    /// Writes 640 by 480 PPM renders of the component in a mock panel (the
    /// lobby's lower half: the retail background and panel, a Messages label,
    /// the box and the line) from the imported pieces:
    ///
    /// ```text
    /// TORE_DATA_DIR=... TORE_MOCK_OUT=out-dir cargo test -p tore-app --locked \
    ///     net::lobby_chat -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs an imported data profile (TORE_DATA_DIR) and TORE_MOCK_OUT"]
    fn render_the_lobby_chat_in_a_mock_panel() {
        use crate::menu::{HEIGHT, WIDTH, text_width};
        use crate::widgets::{Background, draw_panel};
        let dir = crate::assets::data_directory().expect("data directory");
        let assets =
            crate::assets::Assets::load(&dir).expect("imported pack with the multiplayer art");
        let kit = Kit::new(&assets.pics, &assets.multiplayer_resources, "MODEM3").expect("kit");
        let out =
            std::path::PathBuf::from(std::env::var_os("TORE_MOCK_OUT").expect("TORE_MOCK_OUT"));
        let mut chat = LobbyChat::new((45, 300, 549, 78), (45, 380), 54);
        let said = |callsign: &str, standing, receiver, you, text: &str| ChatLine {
            from: ChatFrom::Player {
                callsign: callsign.into(),
                standing,
                you,
            },
            receiver,
            text: text.into(),
            sound: None,
        };
        chat.system(&kit, "Viper joined the game.");
        for line in [
            said(
                "Hawk",
                Standing::Neutral,
                Receiver::All,
                false,
                "anyone up for the Kola mission tonight? two free slots left",
            ),
            said(
                "Viper",
                Standing::Neutral,
                Receiver::All,
                true,
                "count me in",
            ),
            said(
                "Cobra",
                Standing::Own,
                Receiver::Wing,
                false,
                "on your wing",
            ),
            said(
                "Raven",
                Standing::Enemy,
                Receiver::All,
                false,
                "you will not make it",
            ),
        ] {
            chat.push(&kit, &line);
        }
        chat.push(
            &kit,
            &ChatLine::system("Chat takes letters, digits and punctuation only."),
        );
        chat.push(&kit, &ChatLine::system("No one hears you."));
        chat.text_input("ready when you are, King");
        let mut pixels = vec![0u8; WIDTH * HEIGHT * 4];
        let mut canvas = Canvas(&mut pixels);
        Background::direct_connection().draw(&mut canvas, &kit);
        draw_panel(&mut canvas, &kit, (10, 80, 619, 395));
        let font = kit.sprite("PANELFNT");
        let title = "Lobby: chat component";
        canvas.text(
            font,
            title,
            10 + (619 - text_width(font, title)) / 2,
            87,
            None,
        );
        canvas.text(font, "Messages", 45, 289, None);
        chat.draw(&mut canvas, &kit, false, true);
        let mut ppm = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
        for px in pixels.chunks_exact(4) {
            ppm.extend_from_slice(&px[..3]);
        }
        std::fs::write(out.join("lobby-chat.ppm"), ppm).expect("write");
    }
}
