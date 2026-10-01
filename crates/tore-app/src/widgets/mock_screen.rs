//! A mock NETWORK CONNECTION screen made only of kit widgets at the spec's
//! rectangles, for the renders and the draw timing EF2 reports. It is test
//! code: the screens themselves arrive with EF7 and EF8.
//!
//! The ignored tests need an imported data profile with the multiplayer art
//! (`TORE_DATA_DIR`, imported since EF1):
//!
//! ```text
//! TORE_DATA_DIR=... TORE_MOCK_OUT=out-dir cargo test -p tore-app --locked \
//!     widgets::mock_screen -- --ignored --nocapture
//! ```
//!
//! They write 640 by 480 PPM files into `TORE_MOCK_OUT` (a folder that must
//! exist); the lead's notes turn them into PNGs beside John's screenshot.
use super::*;
use crate::menu::{Canvas, HEIGHT, WIDTH, text_width};
use std::time::{Duration, Instant};

/// The screen's widgets, by the id Tab visits them with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id {
    Callsign,
    Address,
    Full,
    Games,
    Players,
    Messages,
    New,
    Join,
    Options,
    Cancel,
}

pub struct Mock {
    /// Only what NETWORK CONNECTION itself has (no address field, check box
    /// or players rows), for the comparison with John's screenshot.
    pub retail: bool,
    pub background: Background,
    pub callsign: TextField,
    pub address: TextField,
    pub full: CheckBox,
    pub games: List,
    pub players: List,
    pub messages: MessageBox,
    pub new: Button,
    pub join: Button,
    pub options: Button,
    pub cancel: Button,
    pub focus: Focus<Id>,
}

/// The screen's own frame lines: the colour measured on John's screenshot.
const LINE: [u8; 4] = [174, 174, 174, 255];

impl Mock {
    /// NEWNET's rectangles (EF0): everything empty, as on John's screenshot.
    pub fn new(background: Background) -> Self {
        Self::build(background, false)
    }
    /// The screen as retail draws it: the same rectangles with none of the
    /// additions Direct Connection makes.
    pub fn retail(background: Background) -> Self {
        Self::build(background, true)
    }
    fn build(background: Background, retail: bool) -> Self {
        let games_columns = vec![
            Column {
                x: 0,
                width: 11,
                align: Align::Centre,
            },
            Column {
                x: 14,
                width: 84,
                align: Align::Left,
            },
            Column {
                x: 100,
                width: 22,
                align: Align::Right,
            },
            Column {
                x: 126,
                width: 46,
                align: Align::Left,
            },
        ];
        let players_columns = vec![
            Column {
                x: 0,
                width: 11,
                align: Align::Centre,
            },
            Column {
                x: 14,
                width: 120,
                align: Align::Left,
            },
            Column {
                x: 138,
                width: 90,
                align: Align::Left,
            },
        ];
        Self {
            retail,
            background,
            callsign: TextField::bar((88, 108), 139, Filter::Callsign).with_hint(if retail {
                ""
            } else {
                "your callsign"
            }),
            address: TextField::edit((110, 132), 20, Filter::Address).with_hint("host or address"),
            full: CheckBox::new((330, 130), "Show full games", false),
            games: List::new((48, 185), 200, 4)
                .with_pager(Pager::NEWNET)
                .with_columns(games_columns),
            players: List::new((346, 185), 242, 5).with_columns(players_columns),
            messages: MessageBox::newnet(),
            new: Button::new("New", (106, 419), 85).default_button(),
            join: Button::new("Join", (229, 419), 85),
            options: Button::new("Options", (352, 419), 85),
            cancel: Button::new("Cancel", (475, 419), 85),
            focus: Focus::new(
                vec![
                    Id::Callsign,
                    Id::Address,
                    Id::Full,
                    Id::Games,
                    Id::Players,
                    Id::Messages,
                    Id::New,
                    Id::Join,
                    Id::Options,
                    Id::Cancel,
                ],
                Some(Id::New),
            ),
        }
    }

    /// Sample content: a callsign, a typed address, five games on two pages
    /// with the second selected, its players, and coloured messages.
    pub fn populate(&mut self, kit: &Kit) {
        self.callsign.set_text("Maverick");
        self.address.set_text("games.example.org:26900");
        self.full.set_checked(true);
        let game = |name: &str, lock: bool, count: &str, state: &str| {
            Row::new(
                name,
                vec![
                    if lock {
                        Cell::Icon(Icon::Lock)
                    } else {
                        Cell::Empty
                    },
                    Cell::Text(name.into()),
                    Cell::Text(count.into()),
                    Cell::Text(state.into()),
                ],
            )
        };
        self.games.set_rows(vec![
            game("Iceman's lobby", false, "3/8", "Lobby"),
            game("Friday night", true, "5/8", "Flying"),
            game("Goose and co", false, "1/4", "Lobby"),
            game("Viper club", false, "7/8", "Lobby").dimmed(),
            game("Top Gun test", true, "2/6", "Lobby"),
        ]);
        self.games.select(1);
        let player = |icon: Option<Icon>, name: &str, aircraft: &str| {
            Row::new(
                name,
                vec![
                    icon.map_or(Cell::Empty, Cell::Icon),
                    Cell::Text(name.into()),
                    Cell::Text(aircraft.into()),
                ],
            )
        };
        self.players.set_rows(vec![
            player(Some(Icon::Crown), "Iceman", "F-14A"),
            player(Some(Icon::Ready), "Goose", "F-14A"),
            player(Some(Icon::Ready), "Slider", "F-16C"),
            player(None, "Viper", "AI"),
        ]);
        self.messages
            .push(kit, "Searching for games on the network...", tone::SYSTEM);
        self.messages.push(kit, "Found 5 games.", tone::SYSTEM);
        self.messages.push(
            kit,
            "Iceman: anyone up for the Kola mission tonight, we have two free F-14 slots and a long list of targets to clear",
            tone::ALL,
        );
        self.messages
            .push(kit, "Goose (to your side): on my wing", tone::OWN_SIDE);
        self.messages
            .push(kit, "Viper (enemy): you will not make it", tone::ENEMY);
        self.messages.push(
            kit,
            "Attempting connection to 'Friday night' at 192.168.1.20:26900",
            tone::SYSTEM,
        );
        self.messages.push(
            kit,
            "The game host has refused your connection.",
            tone::SYSTEM,
        );
        self.messages.push(
            kit,
            "Iceman: the briefing is up, take the F-14A slots first and the rest of you pick whatever is free, then press ready and I will start the mission when everybody has armed",
            tone::ALL,
        );
        self.messages
            .push(kit, "Slider joined the game.", tone::SYSTEM);
        self.messages
            .push(kit, "Slider (to your side): ready", tone::OWN_SIDE);
    }

    /// Draws the whole screen.
    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit) {
        self.background.draw(canvas, kit);
        draw_panel(canvas, kit, (10, 80, 619, 395));
        let font = kit.sprite("PANELFNT");
        // The title is centred in the panel, its top at y 87.
        let title = "TCP/IP Network connection";
        canvas.text(
            font,
            title,
            10 + (619 - text_width(font, title)) / 2,
            87,
            None,
        );
        canvas.outline((30, 100, 579, 355), LINE);
        for (label, x, y) in [
            ("Callsign:", 45, 110),
            ("Games", 45, 165),
            ("Players", 340, 165),
            ("Messages", 45, 304),
        ] {
            canvas.text(font, label, x, y, None);
        }
        canvas.outline((45, 180, 269, 105), LINE);
        canvas.rect((341, 181, 252, 103), [81, 81, 81, 255]);
        canvas.outline((340, 180, 254, 105), LINE);
        let marked = |id| self.focus.marked(id);
        self.callsign.draw(canvas, kit, marked(Id::Callsign));
        if !self.retail {
            canvas.text(font, "Connect to:", 45, 139, None);
            self.address.draw(canvas, kit, marked(Id::Address));
            self.full.draw(canvas, kit, marked(Id::Full));
            self.players.draw(canvas, kit, marked(Id::Players));
        }
        self.games.draw(canvas, kit, marked(Id::Games));
        self.messages.draw(canvas, kit, marked(Id::Messages));
        self.new.draw(canvas, kit, marked(Id::New));
        self.join.draw(canvas, kit, marked(Id::Join));
        self.options.draw(canvas, kit, marked(Id::Options));
        self.cancel.draw(canvas, kit, marked(Id::Cancel));
    }
}

fn save_ppm(path: &std::path::Path, pixels: &[u8]) {
    let mut out = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    for px in pixels.chunks_exact(4) {
        out.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, out).expect("write ppm");
}

/// The retail pieces from the imported profile, for the ignored tests.
fn real_kit(primary: &str) -> Kit {
    let dir = crate::assets::data_directory().expect("data directory");
    let assets = crate::assets::Assets::load(&dir).expect("imported pack with the multiplayer art");
    Kit::new(&assets.pics, &assets.multiplayer_resources, primary).expect("kit")
}

fn out_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("TORE_MOCK_OUT").expect("TORE_MOCK_OUT"))
}

#[test]
#[ignore = "needs an imported data profile (TORE_DATA_DIR) and TORE_MOCK_OUT"]
fn render_mock_screens() {
    let out = out_dir();
    let kit = real_kit("NETIPX3");
    let mut canvas = vec![0u8; WIDTH * HEIGHT * 4];
    // 1. The screenshot's state: nothing typed, nothing found.
    let mock = Mock::retail(Background::single("NETIPX3"));
    mock.draw(&mut Canvas(&mut canvas), &kit);
    save_ppm(&out.join("mock-network-empty.ppm"), &canvas);
    // 2. The same background, with content, the keyboard on the games list.
    let mut mock = Mock::new(Background::single("NETIPX3"));
    mock.populate(&kit);
    mock.focus.set(Id::Games);
    mock.focus.step(true, |_| true);
    mock.focus.set(Id::Games);
    mock.draw(&mut Canvas(&mut canvas), &kit);
    save_ppm(&out.join("mock-network-populated.ppm"), &canvas);
    // 3. John's approved look: MODEM3 under NETIPX3's title bar.
    let kit = real_kit("MODEM3");
    let mut mock = Mock::new(Background::direct_connection());
    mock.populate(&kit);
    mock.focus.set(Id::Callsign);
    mock.focus.step(true, |_| true);
    mock.focus.set(Id::Callsign);
    mock.draw(&mut Canvas(&mut canvas), &kit);
    save_ppm(&out.join("mock-direct-composed-kit.ppm"), &canvas);
}

#[test]
#[ignore = "needs an imported data profile (TORE_DATA_DIR)"]
fn time_mock_draw() {
    let started = Instant::now();
    let kit = real_kit("MODEM3");
    println!("kit built in {:?}", started.elapsed());
    let mut mock = Mock::new(Background::direct_connection());
    mock.populate(&kit);
    mock.focus.set(Id::Games);
    let mut canvas = vec![0u8; WIDTH * HEIGHT * 4];
    for _ in 0..20 {
        mock.draw(&mut Canvas(&mut canvas), &kit);
    }
    let runs = 500;
    let mut each = Vec::with_capacity(runs);
    for _ in 0..runs {
        let started = Instant::now();
        mock.draw(&mut Canvas(&mut canvas), &kit);
        each.push(started.elapsed());
    }
    each.sort();
    let total: Duration = each.iter().sum();
    println!(
        "mock screen draw, {runs} runs: mean {:?}, median {:?}, p95 {:?}, max {:?}",
        total / runs as u32,
        each[runs / 2],
        each[runs * 95 / 100],
        each[runs - 1]
    );
    // The same screen without the background and panel (the part a screen
    // repaints over a cached backdrop), for EF7's budget.
    let mut widgets_only = Vec::with_capacity(runs);
    for _ in 0..runs {
        let started = Instant::now();
        let mut c = Canvas(&mut canvas);
        mock.games.draw(&mut c, &kit, false);
        mock.players.draw(&mut c, &kit, false);
        mock.messages.draw(&mut c, &kit, false);
        mock.callsign.draw(&mut c, &kit, true);
        mock.address.draw(&mut c, &kit, false);
        mock.full.draw(&mut c, &kit, false);
        for b in [&mock.new, &mock.join, &mock.options, &mock.cancel] {
            b.draw(&mut c, &kit, false);
        }
        widgets_only.push(started.elapsed());
    }
    widgets_only.sort();
    println!(
        "widgets alone: median {:?}, p95 {:?}",
        widgets_only[runs / 2],
        widgets_only[runs * 95 / 100]
    );
}

/// A caption in the panel font.
fn caption(canvas: &mut Canvas, kit: &Kit, text: &str, x: i32, y: i32) {
    canvas.text(kit.sprite("PANELFNT"), text, x, y, None);
}

/// Every widget in each state it can be in, on the panel, for the review
/// sheets: buttons, check box frames and text fields on the first, lists and
/// message boxes on the second.
#[test]
#[ignore = "needs an imported data profile (TORE_DATA_DIR) and TORE_MOCK_OUT"]
fn render_state_sheets() {
    let out = out_dir();
    let kit = real_kit("NETIPX3");
    let now = Instant::now();
    let mut pixels = vec![0u8; WIDTH * HEIGHT * 4];
    let mut c = Canvas(&mut pixels);
    Background::single("NETIPX3").draw(&mut c, &kit);
    draw_panel(&mut c, &kit, (10, 10, 619, 460));

    // Buttons: the five looks of the plain and the default button.
    caption(&mut c, &kit, "BUTTONS", 30, 24);
    let looks = ["normal", "hover", "pressed", "focused", "disabled"];
    for (row, default) in [false, true].into_iter().enumerate() {
        for (i, look) in looks.iter().enumerate() {
            let x = 62 + i as i32 * 110;
            let y = 50 + row as i32 * 56;
            let mut b = Button::new(if default { "New" } else { "Join" }, (x, y), 85);
            if default {
                b = b.default_button();
            }
            match *look {
                "hover" => {
                    b.pointer_move(Some((x + 20, y + 10)));
                }
                "pressed" => {
                    b.press((x + 20, y + 10));
                }
                "disabled" => b.set_enabled(false),
                _ => {}
            }
            b.draw(&mut c, &kit, *look == "focused");
            caption(&mut c, &kit, look, x + 20, y + 34);
        }
    }

    // Check box: every frame, then focused, disabled and labelled.
    caption(
        &mut c,
        &kit,
        "CHECK BOX: frames 00 to 06 (on plays 01 to 06, off plays 05 to 00)",
        30,
        152,
    );
    for frame in 0..=6 {
        let at = (40 + frame * 52, 170);
        let mut lamp = CheckBox::new(at, "", false);
        if (1..6).contains(&frame) {
            // Play the on animation to this frame.
            lamp.toggle(now);
            lamp.advance(now + crate::rocker::Rocker::FRAME * (frame as u32 - 1));
        } else {
            lamp.set_checked(frame == 6);
        }
        lamp.draw(&mut c, &kit, false);
        caption(&mut c, &kit, &format!("0{frame}"), 46 + frame * 52, 204);
    }
    let on = CheckBox::new((420, 170), "Show full games", true);
    on.draw(&mut c, &kit, true);
    caption(&mut c, &kit, "focused, labelled", 420, 204);
    let mut off = CheckBox::new((540, 170), "Off", false);
    off.set_enabled(false);
    off.draw(&mut c, &kit, false);
    caption(&mut c, &kit, "disabled", 540, 204);

    // Text fields.
    caption(
        &mut c,
        &kit,
        "TEXT FIELD (edit control, 12 characters wide)",
        30,
        224,
    );
    let empty = TextField::edit((40, 242), 12, Filter::Address).with_hint("host or address");
    let mut typed = TextField::edit((190, 242), 12, Filter::Address);
    typed.set_text("192.168.1.20");
    let mut focused = TextField::edit((340, 242), 12, Filter::Address);
    focused.set_text("games.local");
    focused.key("Home");
    for _ in 0..5 {
        focused.key("ArrowRight");
    }
    let mut overflow = TextField::edit((490, 242), 12, Filter::Address);
    overflow.set_text("games.example-server.org:26900");
    let mut off = TextField::edit((40, 290), 12, Filter::Address);
    off.set_text("disabled");
    off.set_enabled(false);
    empty.draw(&mut c, &kit, false);
    typed.draw(&mut c, &kit, false);
    focused.draw(&mut c, &kit, true);
    overflow.draw(&mut c, &kit, true);
    off.draw(&mut c, &kit, false);
    for (text, x) in [
        ("empty, hint", 40),
        ("typed", 190),
        ("focused, caret mid-text", 340),
        ("scrolled, caret at end", 490),
    ] {
        caption(&mut c, &kit, text, x, 270);
    }
    caption(&mut c, &kit, "disabled", 40, 318);
    caption(&mut c, &kit, "TEXT FIELD (flat bar, 139 by 13)", 190, 296);
    let bar_empty = TextField::bar((190, 316), 139, Filter::Callsign).with_hint("your callsign");
    let mut bar_typed = TextField::bar((340, 316), 139, Filter::Callsign);
    bar_typed.set_text("Maverick");
    bar_empty.draw(&mut c, &kit, false);
    bar_typed.draw(&mut c, &kit, true);
    caption(&mut c, &kit, "empty, hint", 190, 333);
    caption(&mut c, &kit, "typed, focused", 340, 333);
    // The panel title.
    caption(
        &mut c,
        &kit,
        "FOCUS ORDER: Tab and Shift+Tab; the dotted mark shows after the first key",
        30,
        360,
    );
    save_ppm(&out.join("widget-states-1.ppm"), &pixels);

    // Sheet two: lists and the message box.
    let mut pixels = vec![0u8; WIDTH * HEIGHT * 4];
    let mut c = Canvas(&mut pixels);
    Background::single("NETIPX3").draw(&mut c, &kit);
    draw_panel(&mut c, &kit, (10, 10, 619, 460));
    let pager_at = |x: i32, y: i32| Pager {
        rocker: (x + 232, y + 15),
        prev: (x + 204, y + 16),
        next: (x + 204, y + 39),
        page_label: (x + 43, y + 80),
        counter_box: (x + 72, y + 76),
    };
    let columns = vec![
        Column {
            x: 0,
            width: 11,
            align: Align::Centre,
        },
        Column {
            x: 14,
            width: 84,
            align: Align::Left,
        },
        Column {
            x: 100,
            width: 22,
            align: Align::Right,
        },
        Column {
            x: 126,
            width: 46,
            align: Align::Left,
        },
    ];
    let game = |name: &str, lock: bool, count: &str, state: &str| {
        Row::new(
            name,
            vec![
                if lock {
                    Cell::Icon(Icon::Lock)
                } else {
                    Cell::Empty
                },
                Cell::Text(name.into()),
                Cell::Text(count.into()),
                Cell::Text(state.into()),
            ],
        )
    };
    let sample = vec![
        game("Iceman's lobby", false, "3/8", "Lobby"),
        game("Friday night", true, "5/8", "Flying"),
        game("Goose and co", false, "1/4", "Lobby"),
        game("Viper club", false, "7/8", "Lobby").dimmed(),
        game("Top Gun test", true, "2/6", "Lobby"),
        game("Maverick", false, "1/2", "Lobby"),
    ];
    caption(&mut c, &kit, "LIST (4 rows, 18 pixel pitch)", 30, 24);
    let cell = |i: usize| (30 + (i as i32 % 2) * 300, 46 + (i as i32 / 2) * 106);
    let mut lists: Vec<(List, &str, bool)> = Vec::new();
    for (i, caption_text) in [
        "empty: PAGE 1 of 0",
        "one page, no selection",
        "page 1 of 2, row 2 selected, focused",
        "page 2 of 2, dim row, key moved",
        "forty games: page 10 of 10",
    ]
    .iter()
    .enumerate()
    {
        let (x, y) = cell(i);
        let mut l = List::new((x, y), 200, 4)
            .with_pager(pager_at(x, y))
            .with_columns(columns.clone());
        match i {
            1 => l.set_rows(sample[..3].to_vec()),
            2 => {
                l.set_rows(sample.clone());
                l.select(1);
            }
            3 => {
                l.set_rows(sample.clone());
                l.key("End");
            }
            4 => l.set_rows(
                (0..40)
                    .map(|n| game(&format!("Game {n}"), n % 7 == 0, "2/8", "Lobby"))
                    .collect(),
            ),
            _ => {}
        }
        lists.push((l, caption_text, i == 2));
    }
    lists[4].0.key("End");
    for (i, (l, text, focused)) in lists.iter().enumerate() {
        let (x, y) = cell(i);
        l.draw(&mut c, &kit, *focused);
        caption(&mut c, &kit, text, x, y - 12);
    }
    let (x, y) = cell(5);
    caption(&mut c, &kit, "ICONS: lock, crown, ready", x, y - 12);
    let mut icons = List::new((x, y), 200, 4).with_columns(vec![
        Column {
            x: 0,
            width: 11,
            align: Align::Centre,
        },
        Column {
            x: 14,
            width: 100,
            align: Align::Left,
        },
    ]);
    icons.set_rows(vec![
        Row::new(
            "a",
            vec![Cell::Icon(Icon::Crown), Cell::Text("Iceman (King)".into())],
        ),
        Row::new(
            "b",
            vec![Cell::Icon(Icon::Ready), Cell::Text("Goose".into())],
        ),
        Row::new(
            "c",
            vec![Cell::Icon(Icon::Lock), Cell::Text("Password game".into())],
        ),
        Row::new("d", vec![Cell::Empty, Cell::Text("Slider".into())]),
    ]);
    icons.select(0);
    icons.draw(&mut c, &kit, false);

    caption(
        &mut c,
        &kit,
        "MESSAGE BOX: wrapped, coloured; scrolled back shows the bar",
        30,
        354,
    );
    let mut a = MessageBox::new((30, 374, 280, 88));
    let mut b = MessageBox::new((330, 374, 280, 88));
    for (text, tone) in [
        ("Found 5 games.", tone::SYSTEM),
        (
            "Iceman: anyone up for the Kola mission tonight, we have two free slots",
            tone::ALL,
        ),
        ("Goose (to your side): on my wing", tone::OWN_SIDE),
        ("Viper (enemy): you will not make it", tone::ENEMY),
    ] {
        a.push(&kit, text, tone);
    }
    for i in 0..14 {
        b.push(
            &kit,
            &format!("Line {i}: the box keeps up to 200 lines and scrolls back"),
            if i % 2 == 0 { tone::SYSTEM } else { tone::ALL },
        );
    }
    b.wheel(1);
    a.draw(&mut c, &kit, false);
    b.draw(&mut c, &kit, true);
    save_ppm(&out.join("widget-states-2.ppm"), &pixels);
}
