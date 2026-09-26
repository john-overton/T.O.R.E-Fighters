//! The right-click menu of the replay viewer and of live flight with Debug
//! panels on: a right-click (press and release within a few pixels) picks
//! the aircraft or missile nearest the pointer through the camera's own
//! projection and offers what can be done with it; on empty space it lists
//! every aircraft by side and wing to jump to. A right-drag keeps looking
//! around. The menu draws in the 640x480 panel layer and is driven with the
//! keys and mouse the flight menu uses. Opinionated addition requested by
//! John on 2026-09-26; items, wording and the pick radius are agent design
//! decisions (2026-09-26).
use crate::controls_editor::{
    Editor, FOCUS, GOOD, HEADER, MUTED, PALE, PANEL, Rect, TITLE, WHITE, fit, inside, text_width,
};
use crate::menu::Canvas;
use crate::replay::panels::ascii;
use crate::terrain::Camera;
use tore_formats::font::Font;
use tore_replay::Side;

/// How far the pointer may move between pressing and releasing the right
/// button and still count as a click, in logical pixels.
pub const CLICK_SLOP: f64 = 4.;
/// How far from an aircraft or missile on screen a click still picks it,
/// in layer pixels (scaled with the view).
pub const PICK_RADIUS: f64 = 14.;
/// Rows the menu shows before it scrolls.
const MAX_ROWS: usize = 24;
const MIN_WIDTH: i32 = 150;
const MAX_WIDTH: i32 = 300;
const EDGE: i32 = 4;
const BODY: [u8; 4] = [20, 28, 38, 246];
const BORDER: [u8; 4] = [110, 130, 156, 255];

/// What a right-click landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Aircraft(u32),
    Missile(u32),
    Nothing,
}

/// What a menu item does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The chase view (F10) on the aircraft.
    Follow(u32),
    /// The front view from the aircraft: the cockpit for the player.
    Cockpit(u32),
    /// The follow drone beside the aircraft.
    Drone(u32),
    /// A free drone beside the missile.
    DroneMissile(u32),
    Thought(u32),
    Telemetry(u32),
    Guidance(u32),
    /// The Comms panel filtered to the aircraft.
    Comms(u32),
    Labels,
    Trails,
    /// Select the aircraft, keeping the camera as it is.
    Jump(u32),
}

/// One menu row: a heading, or an item with an optional detail on the
/// right (On or Off for a switch, the type for an aircraft).
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub label: String,
    pub detail: String,
    pub action: Option<Action>,
}

impl Item {
    fn new(label: &str, action: Action) -> Self {
        Self {
            label: label.to_owned(),
            detail: String::new(),
            action: Some(action),
        }
    }

    fn switch(label: &str, action: Action, on: bool) -> Self {
        Self {
            detail: if on { "On" } else { "Off" }.into(),
            ..Self::new(label, action)
        }
    }

    fn heading(label: String) -> Self {
        Self {
            label,
            detail: String::new(),
            action: None,
        }
    }
}

/// What the menu offers, which differs between a replay and live flight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Live flight: no drone and no trails.
    pub live: bool,
    pub labels: bool,
    pub trails: bool,
}

/// An aircraft as the menus list it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Aircraft {
    pub id: u32,
    pub label: String,
    /// The type, for example `MiG-29`.
    pub name: String,
    pub side: Side,
    pub wing: u16,
    pub member: u16,
    /// Flown by the AI, so it has thinking to show.
    pub ai: bool,
}

impl Aircraft {
    pub fn title(&self) -> String {
        if self.name.is_empty() {
            self.label.clone()
        } else {
            format!("{}  {}", self.label, self.name)
        }
    }
}

fn switches(items: &mut Vec<Item>, options: Options) {
    items.push(Item::switch("Name labels", Action::Labels, options.labels));
    if !options.live {
        items.push(Item::switch(
            "Flight path trails",
            Action::Trails,
            options.trails,
        ));
    }
}

/// The items for an aircraft.
pub fn aircraft_items(aircraft: &Aircraft, options: Options) -> Vec<Item> {
    let id = aircraft.id;
    let mut items = vec![
        Item::new("Follow (chase view)", Action::Follow(id)),
        Item::new("Cockpit view", Action::Cockpit(id)),
    ];
    if !options.live {
        items.push(Item::new("Drone here", Action::Drone(id)));
    }
    if aircraft.ai {
        items.push(Item::new("AI thinking", Action::Thought(id)));
    }
    items.push(Item::new("Telemetry", Action::Telemetry(id)));
    items.push(Item::new("Comms for this aircraft", Action::Comms(id)));
    switches(&mut items, options);
    items
}

/// The items for a missile fired by `owner`.
pub fn missile_items(missile: u32, owner: Option<&Aircraft>, options: Options) -> Vec<Item> {
    let mut items = vec![Item::new("Guidance", Action::Guidance(missile))];
    if !options.live {
        items.push(Item::new("Drone here", Action::DroneMissile(missile)));
    }
    if let Some(owner) = owner {
        let mut item = Item::new("Go to the shooter", Action::Jump(owner.id));
        item.detail = ascii(&owner.label);
        items.push(item);
    }
    switches(&mut items, options);
    items
}

fn side_order(side: Side) -> u8 {
    match side {
        Side::Friendly => 0,
        Side::Enemy => 1,
        Side::Neutral => 2,
        Side::Unknown => 3,
    }
}

fn side_title(side: Side) -> &'static str {
    match side {
        Side::Friendly => "FRIENDLY",
        Side::Enemy => "ENEMY",
        Side::Neutral => "NEUTRAL",
        Side::Unknown => "OTHER",
    }
}

/// Every aircraft, grouped by side and wing under a heading for each, to
/// jump to, then the switches.
pub fn jump_items(aircraft: &[Aircraft], options: Options) -> Vec<Item> {
    let mut sorted: Vec<&Aircraft> = aircraft.iter().collect();
    sorted.sort_by_key(|a| (side_order(a.side), a.wing, a.member, a.id));
    let mut items = Vec::new();
    let mut group = None;
    for a in sorted {
        let here = (a.side, a.wing);
        if group != Some(here) {
            group = Some(here);
            items.push(Item::heading(if a.wing == 0 {
                side_title(a.side).to_owned()
            } else {
                format!("{} WING {}", side_title(a.side), a.wing)
            }));
        }
        let mut item = Item::new(&ascii(&a.label), Action::Jump(a.id));
        item.detail = ascii(&a.name);
        items.push(item);
    }
    items.push(Item::heading("SHOW".into()));
    switches(&mut items, options);
    items
}

/// What a key or click did to the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The key is not one the menu uses.
    Ignored,
    /// Taken; the menu stays open.
    Handled,
    Close,
    Chosen(Action),
}

/// An open menu.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub target: Target,
    pub title: String,
    pub items: Vec<Item>,
    rect: Rect,
    row: i32,
    rows: usize,
    pub focus: usize,
    scroll: usize,
    pressed: Option<usize>,
}

impl Menu {
    /// A menu opened by a click at layer point `at`, placed beside the
    /// point and kept inside the layer; a point outside the layer (in the
    /// bands beside or above it) is brought to its edge first.
    pub fn new(
        target: Target,
        title: String,
        items: Vec<Item>,
        at: (f64, f64),
        font: &Font,
    ) -> Self {
        let row = font.height as i32 + 5;
        let title = ascii(&title);
        let width = items
            .iter()
            .map(|item| {
                text_width(font, &item.label)
                    + if item.detail.is_empty() {
                        0
                    } else {
                        text_width(font, &item.detail) + 16
                    }
            })
            .chain([text_width(font, &title)])
            .max()
            .unwrap_or(0)
            + 20;
        let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
        let rows = items.len().clamp(1, MAX_ROWS);
        let height = row * (rows as i32 + 1) + 2;
        let (x, y) = (
            at.0.clamp(0., f64::from(crate::replay::panels::WIDTH)) as i32,
            at.1.clamp(0., f64::from(crate::replay::panels::HEIGHT)) as i32,
        );
        let place = |at: i32, size: i32, limit: i32| {
            let start = if at + 2 + size <= limit - EDGE {
                at + 2
            } else {
                at - 2 - size
            };
            start.clamp(EDGE, (limit - EDGE - size).max(EDGE))
        };
        let mut menu = Self {
            target,
            title,
            rect: (
                place(x, width, crate::replay::panels::WIDTH),
                place(y, height, crate::replay::panels::HEIGHT),
                width,
                height,
            ),
            row,
            rows,
            focus: 0,
            scroll: 0,
            pressed: None,
            items,
        };
        menu.focus = menu.next(0, 1, true).unwrap_or(0);
        menu
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn contains(&self, at: (f64, f64)) -> bool {
        inside(at, self.rect)
    }

    fn selectable(&self, index: usize) -> bool {
        self.items.get(index).is_some_and(|i| i.action.is_some())
    }

    /// The next selectable item from `from` in `step` direction, counting
    /// `from` itself when `inclusive`, wrapping round.
    fn next(&self, from: usize, step: i64, inclusive: bool) -> Option<usize> {
        let len = self.items.len() as i64;
        if len == 0 {
            return None;
        }
        let start = if inclusive { 0 } else { 1 };
        (start..=len)
            .map(|k| (from as i64 + step * k).rem_euclid(len) as usize)
            .find(|i| self.selectable(*i))
    }

    /// Scrolls the focused item into view, with its group's heading when
    /// scrolling up to it.
    fn reveal(&mut self) {
        let top = match self.focus.checked_sub(1) {
            Some(above) if !self.selectable(above) => above,
            _ => self.focus,
        };
        if top < self.scroll {
            self.scroll = top;
        } else if self.focus >= self.scroll + self.rows {
            self.scroll = self.focus + 1 - self.rows;
        }
    }

    fn row_rect(&self, visible_row: usize) -> Rect {
        let (x, y, w, _) = self.rect;
        (
            x + 1,
            y + 1 + self.row * (visible_row as i32 + 1),
            w - 2,
            self.row,
        )
    }

    /// The item under layer point `at`.
    fn item_at(&self, at: (f64, f64)) -> Option<usize> {
        (0..self.rows)
            .find(|row| inside(at, self.row_rect(*row)))
            .map(|row| row + self.scroll)
            .filter(|i| *i < self.items.len())
    }

    fn choose(&self, index: usize) -> Outcome {
        self.items
            .get(index)
            .and_then(|i| i.action)
            .map_or(Outcome::Handled, Outcome::Chosen)
    }

    /// A key, named as the app names keys: Up, Down, Tab (Shift+Tab back),
    /// Home, End, PageUp and PageDown move; Enter, Space and Right choose;
    /// Esc and Left close.
    pub fn key(&mut self, key: &str, shift: bool) -> Outcome {
        let moved = match key {
            "ArrowDown" => self.next(self.focus, 1, false),
            "Tab" => self.next(self.focus, if shift { -1 } else { 1 }, false),
            "ArrowUp" => self.next(self.focus, -1, false),
            "Home" => self.next(0, 1, true),
            "End" => self.next(self.items.len().saturating_sub(1), -1, true),
            "PageDown" => {
                let target = (self.focus + self.rows).min(self.items.len().saturating_sub(1));
                self.next(target, -1, true)
            }
            "PageUp" => self.next(self.focus.saturating_sub(self.rows), 1, true),
            "Enter" | "Space" | "ArrowRight" => return self.choose(self.focus),
            "Escape" | "ArrowLeft" => return Outcome::Close,
            _ => return Outcome::Ignored,
        };
        if let Some(focus) = moved {
            self.focus = focus;
            self.reveal();
        }
        Outcome::Handled
    }

    /// The pointer moved: the item under it takes the focus.
    pub fn pointer(&mut self, at: Option<(f64, f64)>) {
        if let Some(index) = at.and_then(|at| self.item_at(at))
            && self.selectable(index)
        {
            self.focus = index;
        }
    }

    /// The left button went down. False when it was outside the menu,
    /// which closes it.
    pub fn down(&mut self, at: Option<(f64, f64)>) -> bool {
        self.pressed = at.and_then(|at| self.item_at(at));
        at.is_some_and(|at| self.contains(at))
    }

    /// The left button came up: an item is chosen when the press began and
    /// ended on it.
    pub fn up(&mut self, at: Option<(f64, f64)>) -> Outcome {
        let pressed = self.pressed.take();
        match (pressed, at.and_then(|at| self.item_at(at))) {
            (Some(a), Some(b)) if a == b => self.choose(a),
            _ => Outcome::Handled,
        }
    }

    /// Mouse wheel notches, up positive, scroll a long menu.
    pub fn wheel(&mut self, notches: i32) {
        let most = self.items.len().saturating_sub(self.rows);
        self.scroll = if notches > 0 {
            self.scroll.saturating_sub(notches as usize)
        } else {
            (self.scroll + notches.unsigned_abs() as usize).min(most)
        };
    }

    /// Draws the menu into the 640x480 layer.
    pub fn draw(&self, pixels: &mut [u8], font: &Font) {
        let r = self.rect;
        Canvas(pixels).rect(r, BODY);
        Canvas(pixels).outline(r, BORDER);
        let title = (r.0 + 1, r.1 + 1, r.2 - 2, self.row);
        Canvas(pixels).rect(title, PANEL);
        let dy = (self.row - font.height as i32) / 2;
        Editor::text(
            pixels,
            font,
            title,
            TITLE,
            &fit(font, &self.title, title.2 - 10),
            (title.0 + 5, title.1 + dy),
        );
        for row in 0..self.rows {
            let index = row + self.scroll;
            let Some(item) = self.items.get(index) else {
                break;
            };
            let rr = self.row_rect(row);
            let focused = index == self.focus && item.action.is_some();
            let (fill, color) = match (item.action, focused) {
                (None, _) => (Some(HEADER), TITLE),
                (Some(_), true) => (Some(FOCUS), WHITE),
                (Some(_), false) => (None, PALE),
            };
            if let Some(fill) = fill {
                Canvas(pixels).rect(rr, fill);
            }
            let detail_width = if item.detail.is_empty() {
                0
            } else {
                text_width(font, &item.detail) + 12
            };
            Editor::text(
                pixels,
                font,
                rr,
                color,
                &fit(font, &item.label, rr.2 - 12 - detail_width),
                (rr.0 + 6, rr.1 + dy),
            );
            if !item.detail.is_empty() {
                let detail_color = match (item.detail.as_str(), focused) {
                    (_, true) => WHITE,
                    ("On", false) => GOOD,
                    _ => MUTED,
                };
                let detail = fit(font, &item.detail, rr.2 / 2);
                Editor::text(
                    pixels,
                    font,
                    rr,
                    detail_color,
                    &detail,
                    (rr.0 + rr.2 - 6 - text_width(font, &detail), rr.1 + dy),
                );
            }
        }
        if self.items.len() > self.rows {
            let top = self.row_rect(0).1;
            let track = (r.0 + r.2 - 3, top, 2, self.row * self.rows as i32);
            Canvas(pixels).rect(track, HEADER);
            let h = (track.3 * self.rows as i32 / self.items.len() as i32).max(6);
            let most = self.items.len() - self.rows;
            let y = track.1 + (track.3 - h) * self.scroll as i32 / most as i32;
            Canvas(pixels).rect((track.0, y, 2, h), TITLE);
        }
    }
}

/// Something a right-click can pick, where it is and, for an aircraft, the
/// name label drawn over it (left, top, width, height in view pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pickable {
    pub target: Target,
    pub position: [f64; 3],
    pub label: Option<[f64; 4]>,
}

/// What a right-click can pick in `picture`: every aircraft `aircraft`
/// accepts that is not lying wrecked on the ground, with its name label
/// (`labels` by aircraft id) when one is drawn, and every weapon but gun
/// rounds.
pub fn pickables(
    picture: &crate::render_snapshot::RenderSnapshot,
    labels: &[(u32, [f64; 4])],
    aircraft: impl Fn(&crate::render_snapshot::AircraftPose) -> bool,
) -> Vec<Pickable> {
    std::iter::once(&picture.player)
        .chain(&picture.targets)
        .filter(|pose| (pose.airborne || !pose.crashed) && aircraft(pose))
        .map(|pose| Pickable {
            target: Target::Aircraft(pose.id),
            position: pose.position,
            label: labels
                .iter()
                .find(|(id, _)| *id == pose.id)
                .map(|(_, rect)| *rect),
        })
        .chain(
            picture
                .projectiles
                .iter()
                .filter(|p| !p.gun)
                .map(|p| Pickable {
                    target: Target::Missile(p.id),
                    position: p.position,
                    label: None,
                }),
        )
        .collect()
}

/// The aircraft or missile nearest `point` (view pixels) on a view of
/// `size`, through the camera that drew it, within `radius` view pixels of
/// where it is drawn or on its name label.
pub fn pick(
    point: [f64; 2],
    size: [u32; 2],
    camera: &Camera,
    things: &[Pickable],
    radius: f64,
) -> Target {
    let mut best: Option<(f64, Target)> = None;
    for thing in things {
        let on_label = thing.label.is_some_and(|[x, y, w, h]| {
            point[0] >= x && point[0] < x + w && point[1] >= y && point[1] < y + h
        });
        let distance = if on_label {
            Some(0.)
        } else {
            camera
                .project(size, thing.position)
                .map(|[x, y]| (x - point[0]).hypot(y - point[1]))
                .filter(|d| *d <= radius)
        };
        if let Some(distance) = distance
            && best.is_none_or(|(d, _)| distance < d)
        {
            best = Some((distance, thing.target));
        }
    }
    best.map_or(Target::Nothing, |(_, target)| target)
}

/// A right-button press being watched: released within [`CLICK_SLOP`] of
/// where it went down, having never strayed further, it is a click; once
/// it strays it is a drag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RightClick {
    start: [f64; 2],
    slop: f64,
    dragged: bool,
}

impl RightClick {
    /// Pressed at `at`; `slop` is the click distance in the same pixels.
    pub fn press(at: [f64; 2], slop: f64) -> Self {
        Self {
            start: at,
            slop,
            dragged: false,
        }
    }

    pub fn moved(&mut self, at: [f64; 2]) {
        if (at[0] - self.start[0]).hypot(at[1] - self.start[1]) > self.slop {
            self.dragged = true;
        }
    }

    /// Released at `at`: true for a click.
    pub fn release(mut self, at: [f64; 2]) -> bool {
        self.moved(at);
        !self.dragged
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::overlay::Placement;
    use crate::replay::panels::tests::font;

    fn fleet() -> Vec<Aircraft> {
        let a = |id, label: &str, side, wing, member| Aircraft {
            id,
            label: label.into(),
            name: "MiG-29".into(),
            side,
            wing,
            member,
            ai: id != 0,
        };
        vec![
            a(4, "Enemy 2-2", Side::Enemy, 2, 2),
            a(0, "You", Side::Friendly, 1, 1),
            a(3, "Enemy 2-1", Side::Enemy, 2, 1),
            a(1, "Friendly 1-2", Side::Friendly, 1, 2),
            a(9, "Straggler", Side::Unknown, 0, 0),
        ]
    }

    #[test]
    fn aircraft_menus_offer_what_fits_the_aircraft_and_the_host() {
        let fleet = fleet();
        let labels = |items: &[Item]| items.iter().map(|i| i.label.clone()).collect::<Vec<_>>();
        let replay = Options {
            labels: true,
            ..Default::default()
        };
        assert_eq!(
            labels(&aircraft_items(&fleet[2], replay)),
            [
                "Follow (chase view)",
                "Cockpit view",
                "Drone here",
                "AI thinking",
                "Telemetry",
                "Comms for this aircraft",
                "Name labels",
                "Flight path trails",
            ]
        );
        // The player has no AI thinking; live flight has no drone or trails.
        let live = Options {
            live: true,
            ..Default::default()
        };
        assert_eq!(
            labels(&aircraft_items(&fleet[1], live)),
            [
                "Follow (chase view)",
                "Cockpit view",
                "Telemetry",
                "Comms for this aircraft",
                "Name labels",
            ]
        );
        let items = aircraft_items(&fleet[2], replay);
        assert_eq!(items[6].detail, "On");
        assert_eq!(items[7].detail, "Off");
        let missile = missile_items(7, Some(&fleet[2]), replay);
        assert_eq!(missile[0].action, Some(Action::Guidance(7)));
        assert_eq!(missile[2].action, Some(Action::Jump(3)));
        assert_eq!(missile[2].detail, "Enemy 2-1");
    }

    #[test]
    fn empty_space_lists_aircraft_by_side_and_wing() {
        let items = jump_items(&fleet(), Options::default());
        let rows: Vec<(String, Option<Action>)> =
            items.iter().map(|i| (i.label.clone(), i.action)).collect();
        assert_eq!(
            rows[..9],
            [
                ("FRIENDLY WING 1".into(), None),
                ("You".into(), Some(Action::Jump(0))),
                ("Friendly 1-2".into(), Some(Action::Jump(1))),
                ("ENEMY WING 2".into(), None),
                ("Enemy 2-1".into(), Some(Action::Jump(3))),
                ("Enemy 2-2".into(), Some(Action::Jump(4))),
                ("OTHER".into(), None),
                ("Straggler".into(), Some(Action::Jump(9))),
                ("SHOW".into(), None),
            ]
        );
    }

    #[test]
    fn keys_move_over_headings_choose_and_close() {
        let font = font();
        let items = jump_items(&fleet(), Options::default());
        let mut menu = Menu::new(Target::Nothing, "Jump".into(), items, (100., 100.), &font);
        // The first selectable row takes the focus, skipping the heading.
        assert_eq!(menu.focus, 1);
        assert_eq!(menu.key("ArrowDown", false), Outcome::Handled);
        assert_eq!(menu.focus, 2);
        menu.key("ArrowDown", false);
        assert_eq!(menu.focus, 4);
        menu.key("ArrowUp", false);
        menu.key("ArrowUp", false);
        assert_eq!(menu.focus, 1);
        // Up from the first item wraps to the last.
        menu.key("ArrowUp", false);
        assert_eq!(menu.focus, menu.items.len() - 1);
        menu.key("Tab", false);
        assert_eq!(menu.focus, 1);
        menu.key("Tab", true);
        assert_eq!(menu.focus, menu.items.len() - 1);
        menu.key("Home", false);
        assert_eq!(menu.key("Enter", false), Outcome::Chosen(Action::Jump(0)));
        assert_eq!(menu.key("Space", false), Outcome::Chosen(Action::Jump(0)));
        menu.key("End", false);
        assert_eq!(
            menu.key("ArrowRight", false),
            Outcome::Chosen(Action::Trails)
        );
        assert_eq!(menu.key("Escape", false), Outcome::Close);
        assert_eq!(menu.key("ArrowLeft", false), Outcome::Close);
        assert_eq!(menu.key("j", false), Outcome::Ignored);
    }

    #[test]
    fn a_long_menu_scrolls_to_keep_the_focus_in_view() {
        let font = font();
        let many: Vec<Aircraft> = (0..40)
            .map(|id| Aircraft {
                id,
                label: format!("Aircraft {id}"),
                side: Side::Enemy,
                wing: 1 + id as u16 / 5,
                member: id as u16 % 5,
                ..Default::default()
            })
            .collect();
        let items = jump_items(&many, Options::default());
        assert!(items.len() > MAX_ROWS);
        let mut menu = Menu::new(Target::Nothing, "Jump".into(), items, (0., 0.), &font);
        let r = menu.rect();
        assert!(r.1 + r.3 <= 480 - EDGE);
        menu.key("End", false);
        assert!(menu.focus >= menu.scroll && menu.focus < menu.scroll + menu.rows);
        menu.key("Home", false);
        assert_eq!(menu.scroll, 0);
        menu.key("PageDown", false);
        assert!(menu.focus >= menu.rows - 1);
        menu.wheel(-100);
        assert_eq!(menu.scroll, menu.items.len() - menu.rows);
        menu.wheel(3);
        assert_eq!(menu.scroll, menu.items.len() - menu.rows - 3);
    }

    #[test]
    fn menus_stay_inside_the_layer_wherever_the_click_was() {
        let font = font();
        let items = || aircraft_items(&fleet()[2], Options::default());
        for at in [
            (10., 10.),
            (630., 470.),
            (-300., 240.),
            (900., -50.),
            (320., 479.),
        ] {
            let menu = Menu::new(Target::Aircraft(3), "Enemy".into(), items(), at, &font);
            let (x, y, w, h) = menu.rect();
            assert!(x >= EDGE && y >= EDGE, "{at:?}");
            assert!(x + w <= 640 - EDGE && y + h <= 480 - EDGE, "{at:?}");
        }
        // Beside the click: to the right and below when there is room,
        // flipped left and up near the far edges.
        let menu = Menu::new(
            Target::Aircraft(3),
            "E".into(),
            items(),
            (100., 100.),
            &font,
        );
        assert_eq!((menu.rect().0, menu.rect().1), (102, 102));
        let menu = Menu::new(
            Target::Aircraft(3),
            "E".into(),
            items(),
            (600., 450.),
            &font,
        );
        let (x, y, w, h) = menu.rect();
        assert_eq!((x + w, y + h), (598, 448));
    }

    #[test]
    fn a_click_needs_the_press_and_release_on_the_same_item() {
        let font = font();
        let items = aircraft_items(&fleet()[2], Options::default());
        let mut menu = Menu::new(
            Target::Aircraft(3),
            "Enemy 2-1".into(),
            items,
            (50., 50.),
            &font,
        );
        let centre = |r: Rect| Some((f64::from(r.0 + r.2 / 2), f64::from(r.1 + r.3 / 2)));
        let row = |i: usize| centre(menu.row_rect(i));
        let (third, fourth) = (row(3), row(4));
        menu.pointer(third);
        assert_eq!(menu.focus, 3);
        assert!(menu.down(third));
        assert_eq!(menu.up(fourth), Outcome::Handled);
        assert!(menu.down(third));
        assert_eq!(menu.up(third), Outcome::Chosen(Action::Thought(3)));
        // The title row chooses nothing; outside closes.
        let title = centre((menu.rect().0, menu.rect().1, menu.rect().2, menu.row));
        assert!(menu.down(title));
        assert_eq!(menu.up(title), Outcome::Handled);
        assert!(!menu.down(Some((600., 400.))));
        assert!(!menu.down(None));
    }

    #[test]
    fn the_menu_draws_inside_its_rectangle() {
        let font = font();
        let items = jump_items(&fleet(), Options::default());
        let menu = Menu::new(
            Target::Nothing,
            "Jump to an aircraft".into(),
            items,
            (200., 200.),
            &font,
        );
        let mut pixels = vec![0; 640 * 480 * 4];
        menu.draw(&mut pixels, &font);
        let r = menu.rect();
        for y in 0..480 {
            for x in 0..640 {
                let covered = pixels[(y * 640 + x) * 4 + 3] != 0;
                assert_eq!(covered, inside((x as f64, y as f64), r), "{x} {y}");
            }
        }
    }

    #[test]
    fn a_right_click_is_a_press_and_release_that_never_strays() {
        let click = RightClick::press([100., 100.], 4.);
        assert!(click.release([103., 102.]));
        let mut drag = RightClick::press([100., 100.], 4.);
        drag.moved([110., 100.]);
        // Coming back does not undo a drag.
        drag.moved([100., 100.]);
        assert!(!drag.release([100., 100.]));
        assert!(!RightClick::press([0., 0.], 4.).release([0., 5.]));
    }

    /// A camera at the origin looking north.
    fn camera() -> Camera {
        let mut camera = Camera::new();
        camera.position = [0.; 3];
        camera.yaw = 0.;
        camera.pitch = 0.;
        camera.roll = 0.;
        camera.zoom = 1.;
        camera
    }

    #[test]
    fn picking_uses_the_projection_on_any_view_shape() {
        let camera = camera();
        let things = [
            Pickable {
                target: Target::Aircraft(3),
                position: [200., 50., 2_000.],
                label: None,
            },
            Pickable {
                target: Target::Missile(7),
                position: [-300., 0., 3_000.],
                label: None,
            },
            // Behind the camera: never picked.
            Pickable {
                target: Target::Aircraft(4),
                position: [0., 0., -2_000.],
                label: None,
            },
        ];
        // 4:3, wide and tall views, as windowed, fullscreen and letterboxed
        // layers see them.
        for size in [
            [1280, 960],
            [1920, 1080],
            [3024, 1964],
            [800, 1200],
            [640, 480],
        ] {
            let scale = Placement::new(size).scale();
            let radius = PICK_RADIUS * scale;
            for (thing, target) in things.iter().zip([Target::Aircraft(3), Target::Missile(7)]) {
                let [x, y] = camera.project(size, thing.position).unwrap();
                assert_eq!(
                    pick([x, y], size, &camera, &things, radius),
                    target,
                    "{size:?}"
                );
                let near = [x + radius * 0.6, y - radius * 0.6];
                assert_eq!(
                    pick(near, size, &camera, &things, radius),
                    target,
                    "{size:?}"
                );
                let far = [x + radius * 1.5, y];
                assert_ne!(
                    pick(far, size, &camera, &things, radius),
                    target,
                    "{size:?}"
                );
            }
            let middle = [f64::from(size[0]) / 2., f64::from(size[1]) * 0.9];
            assert_eq!(
                pick(middle, size, &camera, &things, radius),
                Target::Nothing
            );
        }
        // The nearest wins, and a name label picks its aircraft.
        let size = [1280, 960];
        let [x, y] = camera.project(size, things[0].position).unwrap();
        let labelled = [
            things[0],
            Pickable {
                target: Target::Aircraft(5),
                position: [201., 50., 2_000.],
                label: Some([x + 100., y - 40., 60., 12.]),
            },
        ];
        assert_eq!(
            pick([x, y], size, &camera, &labelled, 20.),
            Target::Aircraft(3)
        );
        assert_eq!(
            pick([x + 130., y - 35.], size, &camera, &labelled, 20.),
            Target::Aircraft(5)
        );
    }
}
