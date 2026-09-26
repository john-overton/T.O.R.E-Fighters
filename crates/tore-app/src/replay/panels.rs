//! Debug panels for the replay viewer and live flight: the AI thinking,
//! Telemetry and missile Guidance panels, which draw any display tree
//! (`ai.thought`, `flight.telemetry`, `weapon.guidance`) as an indented,
//! scrollable list of values with their "because" lines, and the Comms
//! panel, which lists every recorded comms and audio entry with its time,
//! kind, speaker, recipients, words, outcome and reason, filtered by kind
//! and aircraft and following the playhead.
//!
//! One code path serves both hosts: the replay reads trees and entries from
//! the recording at the playhead, live flight from the current tick, both
//! through [`Data`]. Everything draws in a 640x480 layer with the Controls
//! screen's colours and font. Up to two tree panels show at once, on the
//! left and right, plus the Comms panel along the bottom. Opinionated
//! addition requested by John on 2026-09-26; the layout, wording, colours
//! and every rule here are agent design decisions (2026-09-26).
use crate::controls_editor::{
    Editor, FOCUS, GOOD, HEADER, MUTED, PALE, PANEL, Rect, TITLE, WHITE, fit, inside, text_width,
};
use crate::menu::Canvas;
use crate::replay::clock;
use std::collections::HashMap;
use std::sync::Arc;
use tore_formats::font::Font;
use tore_replay::{Event, Node, Recording, TimedEvent, TreeSample, Value, vocab};

/// Panels draw into a 640x480 layer.
pub const WIDTH: i32 = 640;
pub const HEIGHT: i32 = 480;

/// Where panels may go in the layer: from `top` to `bottom`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub top: i32,
    pub bottom: i32,
}

/// The replay viewer: below the mission timer, above the transport bar.
pub const REPLAY: Layout = Layout {
    top: 28,
    bottom: 426,
};
/// Live flight: below the mission timer, down to the bottom edge.
pub const LIVE: Layout = Layout {
    top: 28,
    bottom: 472,
};

const MARGIN: i32 = 6;
const SIDE_WIDTH: i32 = 254;
const GAP: i32 = 4;
const COMMS_HEIGHT: i32 = 150;
const TITLE_HEIGHT: i32 = 14;
/// Tree values start this far from a panel's inner left edge.
const VALUE_X: i32 = 92;
/// Indent per tree level, and the deepest level indented.
const INDENT: i32 = 8;
const MAX_INDENT_DEPTH: i32 = 6;
/// Lines one mouse wheel notch scrolls.
const WHEEL_LINES: usize = 3;
/// A sample older than this is marked stale, in ticks (two seconds).
const STALE_TICKS: u64 = 240;
/// Repeats of one sound closer than this merge into one Comms row, ticks.
const RUN_TICKS: u64 = 60;

const BODY: [u8; 4] = [24, 34, 45, 240];
const NOTE: [u8; 4] = [168, 180, 196, 255];
const WARN: [u8; 4] = [255, 206, 84, 255];
const BAD: [u8; 4] = [240, 120, 100, 255];
const CHIP_OFF: [u8; 4] = [52, 68, 90, 255];

/// Which display tree a panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// An AI aircraft's thinking.
    Thought,
    /// Any aircraft's flight-model telemetry.
    Telemetry,
    /// A guided weapon's seeker and steering.
    Guidance,
}

impl Kind {
    pub fn channel(self) -> &'static str {
        match self {
            Self::Thought => vocab::channel::AI_THOUGHT,
            Self::Telemetry => vocab::channel::FLIGHT_TELEMETRY,
            Self::Guidance => vocab::channel::WEAPON_GUIDANCE,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Thought => "AI THINKING",
            Self::Telemetry => "TELEMETRY",
            Self::Guidance => "GUIDANCE",
        }
    }

    /// Aircraft panels follow the selected aircraft unless pinned; a
    /// guidance panel always stays on its missile.
    pub fn follows(self) -> bool {
        self != Self::Guidance
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
}

const SIDES: [Side; 2] = [Side::Left, Side::Right];

/// One open tree panel.
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub kind: Kind,
    /// The aircraft or projectile shown.
    pub subject: u32,
    /// A pinned panel stays on its aircraft when the selection changes.
    pub pinned: bool,
    /// First line shown.
    pub scroll: usize,
    /// Lines the last drawing held and showed, for scrolling.
    lines: usize,
    visible: usize,
    /// Opening order, so a new panel replaces the older unpinned one.
    opened: u64,
}

/// The comms kinds the Comms panel filters by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// Radio calls.
    Radio,
    /// Orders, requests, reports and each recipient's answer.
    Orders,
    /// Tower lines.
    Tower,
    /// Crew remarks and cockpit messages.
    Crew,
    /// Seeker tones, warnings, music and sound effects.
    Tones,
}

pub const CHANNELS: [Channel; 5] = [
    Channel::Radio,
    Channel::Orders,
    Channel::Tower,
    Channel::Crew,
    Channel::Tones,
];

impl Channel {
    /// The channel an event kind belongs to; `None` for events the Comms
    /// panel does not list.
    pub fn of(kind: &str) -> Option<Self> {
        use vocab::kind as k;
        Some(match kind {
            k::COMMS_RADIO => Self::Radio,
            k::COMMS_ORDER | k::COMMS_REQUEST | k::COMMS_REPORT | k::COMMS_DELIVERY => Self::Orders,
            k::COMMS_TOWER => Self::Tower,
            k::COMMS_CREW | k::COMMS_HUD => Self::Crew,
            _ if kind.starts_with("audio.") => Self::Tones,
            // Comms kinds added later list with the radio.
            _ if kind.starts_with("comms.") => Self::Radio,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Radio => "Radio",
            Self::Orders => "Orders",
            Self::Tower => "Tower",
            Self::Crew => "Crew",
            Self::Tones => "Tones",
        }
    }

    fn index(self) -> usize {
        CHANNELS.iter().position(|c| *c == self).unwrap_or(0)
    }

    fn color(self) -> [u8; 4] {
        match self {
            Self::Radio => [120, 190, 255, 255],
            Self::Orders => WARN,
            Self::Tower => GOOD,
            Self::Crew => WHITE,
            Self::Tones => [190, 170, 235, 255],
        }
    }
}

/// The Comms panel's filters and scrolling.
#[derive(Clone, Debug, PartialEq)]
pub struct CommsPanel {
    /// Which channels show, in [`CHANNELS`] order.
    pub shown: [bool; 5],
    /// Only entries sent by, addressed to or about this aircraft.
    pub aircraft: Option<u32>,
    /// While scrolled back: the entry index just past the newest row shown.
    /// `None` follows the playhead.
    pub anchor: Option<usize>,
}

impl Default for CommsPanel {
    fn default() -> Self {
        Self {
            shown: [true; 5],
            aircraft: None,
            anchor: None,
        }
    }
}

impl CommsPanel {
    /// Whether the kind filters let `event` through.
    pub fn passes_kind(&self, event: &Event) -> bool {
        Channel::of(&event.kind).is_some_and(|channel| self.shown[channel.index()])
    }

    /// Whether `event` involves the aircraft filtered for: sent by it,
    /// addressed to it or about it.
    pub fn passes_aircraft(&self, event: &Event) -> bool {
        match self.aircraft {
            None => true,
            Some(id) => {
                event.subject == Some(id)
                    || event.object == Some(id)
                    || event.id(vocab::field::ABOUT) == Some(id)
                    || event
                        .get(vocab::field::RECIPIENTS)
                        .and_then(Value::as_ids)
                        .is_some_and(|ids| ids.contains(&id))
            }
        }
    }
}

/// One Comms row: the entries shown together, oldest first, as indices
/// into the host's entries. Entries that share a message number make one
/// row: a line queued and then delivered, dropped or cut off, or an order,
/// request or report with each recipient's answer. The row sits where its
/// newest entry is and shows the latest outcome. The same sound repeated in
/// quick succession makes one row too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub entries: Vec<usize>,
}

impl Row {
    /// The oldest entry.
    #[cfg(test)]
    pub fn first(&self) -> usize {
        self.entries[0]
    }

    /// The newest entry, where the row sits.
    pub fn last(&self) -> usize {
        self.entries[self.entries.len() - 1]
    }
}

/// The message number that ties an entry to the rest of its line or
/// exchange.
fn message(event: &Event) -> Option<i64> {
    event.get(vocab::field::MESSAGE).and_then(Value::as_i64)
}

/// Sounds repeated in quick succession (a gun burst's release sounds and
/// impacts) merge into one row.
fn repeats(a: &TimedEvent, b: &TimedEvent) -> bool {
    let kind = a.event.kind.as_str();
    matches!(kind, vocab::kind::AUDIO_EFFECT | vocab::kind::AUDIO_RELEASE)
        && kind == b.event.kind
        && a.event.subject == b.event.subject
        && a.event.get(vocab::field::SOUND) == b.event.get(vocab::field::SOUND)
        && a.event.text == b.event.text
        && a.tick.abs_diff(b.tick) <= RUN_TICKS
}

/// How far back a row's earlier entries are looked for once the rows are
/// found, in ticks: longer than any queue holds a line (30 s).
const HISTORY_TICKS: u64 = 3_600;
/// The most entries one look at the list reads, so a narrow filter over a
/// long recording stays cheap.
const MAX_SCAN: usize = 50_000;

/// Where a backwards look at the entries stops starting rows.
#[derive(Clone, Copy, Debug)]
enum Limit {
    /// Once this many rows pass the filters.
    Rows(usize),
    /// Below this entry.
    From(usize),
}

/// The rows of entries before `end` that pass the filters, oldest first.
/// A row passes the aircraft filter when any of its entries does.
fn scan(events: &[TimedEvent], end: usize, filter: &CommsPanel, limit: Limit) -> Vec<Row> {
    let end = end.min(events.len());
    // Rows newest first, each with its entries newest first and whether
    // it passes the aircraft filter.
    let mut rows: Vec<(Vec<usize>, bool)> = Vec::new();
    let mut by_message: HashMap<i64, usize> = HashMap::new();
    let mut passing = 0;
    // Once set, no new rows start; earlier entries of the rows found are
    // still gathered back to this tick.
    let mut closed: Option<u64> = None;
    for i in (end.saturating_sub(MAX_SCAN)..end).rev() {
        let TimedEvent { tick, event } = &events[i];
        if closed.is_none() && matches!(limit, Limit::From(from) if i < from) {
            closed = Some(tick.saturating_sub(HISTORY_TICKS));
        }
        if closed.is_some_and(|until| *tick < until) {
            break;
        }
        if !filter.passes_kind(event) {
            continue;
        }
        let aircraft = filter.passes_aircraft(event);
        let number = message(event);
        let joins = match number {
            Some(number) => by_message.get(&number).copied(),
            None => rows.len().checked_sub(1).filter(|&at| {
                let oldest = rows[at].0[rows[at].0.len() - 1];
                message(&events[oldest].event).is_none() && repeats(&events[oldest], &events[i])
            }),
        };
        if let Some(at) = joins {
            let (entries, pass) = &mut rows[at];
            entries.push(i);
            if aircraft && !*pass {
                *pass = true;
                passing += 1;
            }
            continue;
        }
        if closed.is_some() {
            continue;
        }
        if let Some(number) = number {
            by_message.insert(number, rows.len());
        }
        rows.push((vec![i], aircraft));
        passing += usize::from(aircraft);
        if matches!(limit, Limit::Rows(count) if passing >= count) {
            closed = Some(tick.saturating_sub(HISTORY_TICKS));
        }
    }
    let mut out: Vec<Row> = rows
        .into_iter()
        .filter(|(_, pass)| *pass)
        .map(|(mut entries, _)| {
            entries.reverse();
            Row { entries }
        })
        .collect();
    if let Limit::Rows(count) = limit {
        out.truncate(count);
    }
    out.reverse();
    out
}

/// Up to `count` rows made of entries before `end`, oldest first.
pub fn rows_before(
    events: &[TimedEvent],
    end: usize,
    filter: &CommsPanel,
    count: usize,
) -> Vec<Row> {
    scan(events, end, filter, Limit::Rows(count))
}

/// The rows whose newest entry is from `from` up to `end`, oldest first:
/// scrolling forward.
pub fn rows_since(events: &[TimedEvent], from: usize, end: usize, filter: &CommsPanel) -> Vec<Row> {
    scan(events, end, filter, Limit::From(from))
}

/// What a host lends the panels each frame: the recording at the playhead,
/// or live flight at its latest tick.
pub trait Data {
    /// The tick the panels describe.
    fn now(&self) -> u64;
    /// The latest display tree of `subject` on `channel` at or before now,
    /// with the tick it was taken.
    fn tree(&mut self, subject: u32, channel: &str) -> Option<(u64, TreeSample)>;
    /// An aircraft's label, for aircraft named in trees and comms.
    fn name(&self, id: u32) -> String;
    /// A panel's subject: an aircraft's label and type, or a missile's
    /// weapon and shooter.
    fn title(&self, kind: Kind, subject: u32) -> String;
    /// Why a panel has no tree to show.
    fn missing(&self, kind: Kind, subject: u32) -> String;
    /// Comms and audio entries in time order, and how many are at or
    /// before now.
    fn comms(&self) -> (&[TimedEvent], usize);
    /// Aircraft the Comms panel's aircraft filter steps through.
    fn aircraft(&self) -> Vec<u32>;
}

/// A part of the panels a click can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Pin(Side),
    Close(Side),
    /// Anywhere else on a tree panel.
    Body(Side),
    CommsClose,
    /// A channel filter, in [`CHANNELS`] order.
    CommsChannel(usize),
    CommsAircraft,
    CommsBody,
}

/// Where each open panel sits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rects {
    pub sides: [Option<Rect>; 2],
    pub comms: Option<Rect>,
}

impl Rects {
    /// Every open panel's rectangle.
    pub fn all(&self) -> Vec<Rect> {
        self.sides
            .iter()
            .flatten()
            .chain(&self.comms)
            .copied()
            .collect()
    }
}

fn close_rect((x, y, w, _): Rect) -> Rect {
    (x + w - 16, y + 1, 14, TITLE_HEIGHT - 2)
}

fn pin_rect(r: Rect) -> Rect {
    let close = close_rect(r);
    (close.0 - 28, close.1, 26, close.3)
}

fn chip_rect((x, y, _, _): Rect, index: usize) -> Rect {
    (x + 6 + index as i32 * 48, y + TITLE_HEIGHT + 3, 44, 12)
}

fn aircraft_chip_rect(r: Rect) -> Rect {
    let last = chip_rect(r, CHANNELS.len());
    (last.0 + 8, last.1, 160, last.3)
}

/// The debug panels' state: which are open, pinned and scrolled.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panels {
    slots: [Option<Panel>; 2],
    comms: Option<CommsPanel>,
    /// Filters kept while the Comms panel is closed.
    comms_filters: CommsPanel,
    opened: u64,
    pub hover: Option<Hit>,
    pressed: Option<Hit>,
}

impl Panels {
    pub fn slot(&self, side: Side) -> Option<&Panel> {
        self.slots[side.index()].as_ref()
    }

    #[cfg(test)]
    pub fn comms(&self) -> Option<&CommsPanel> {
        self.comms.as_ref()
    }

    pub fn comms_open(&self) -> bool {
        self.comms.is_some()
    }

    /// Nothing is open.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none) && self.comms.is_none()
    }

    /// The side showing `kind` for `subject`, if one does.
    pub fn find(&self, kind: Kind, subject: u32) -> Option<Side> {
        SIDES.into_iter().find(|side| {
            self.slot(*side)
                .is_some_and(|p| p.kind == kind && p.subject == subject)
        })
    }

    /// Opens a panel of `kind` on `subject`: in a free side, else in place
    /// of the older unpinned panel. A panel already showing it stays.
    /// Refuses when both sides are pinned.
    pub fn open(&mut self, kind: Kind, subject: u32) -> Result<Side, &'static str> {
        if let Some(side) = self.find(kind, subject) {
            return Ok(side);
        }
        let side = SIDES
            .into_iter()
            .find(|s| self.slots[s.index()].is_none())
            .or_else(|| {
                SIDES
                    .into_iter()
                    .filter(|s| self.slots[s.index()].as_ref().is_some_and(|p| !p.pinned))
                    .min_by_key(|s| self.slots[s.index()].as_ref().map_or(0, |p| p.opened))
            })
            .ok_or("Both panels are pinned: close or unpin one first")?;
        self.opened += 1;
        self.slots[side.index()] = Some(Panel {
            kind,
            subject,
            pinned: false,
            scroll: 0,
            lines: 0,
            visible: 0,
            opened: self.opened,
        });
        Ok(side)
    }

    /// Opens `kind` on `subject`, or closes it when it is already open.
    /// True when it is open afterwards.
    pub fn toggle(&mut self, kind: Kind, subject: u32) -> Result<bool, &'static str> {
        if let Some(side) = self.find(kind, subject) {
            self.close(side);
            return Ok(false);
        }
        self.open(kind, subject).map(|_| true)
    }

    pub fn close(&mut self, side: Side) {
        self.slots[side.index()] = None;
    }

    pub fn close_all(&mut self) {
        self.slots = [None, None];
        self.close_comms();
    }

    /// Pins or unpins a side's panel.
    pub fn pin(&mut self, side: Side) {
        if let Some(panel) = &mut self.slots[side.index()] {
            panel.pinned = !panel.pinned;
        }
    }

    /// The selected aircraft changed: unpinned aircraft panels move to it.
    /// If that leaves two panels of one kind on one aircraft, the one that
    /// followed closes.
    pub fn follow(&mut self, selected: u32) {
        for side in SIDES {
            let Some(panel) = &self.slots[side.index()] else {
                continue;
            };
            if panel.pinned || !panel.kind.follows() || panel.subject == selected {
                continue;
            }
            let kind = panel.kind;
            let taken = SIDES.into_iter().any(|other| {
                other != side
                    && self
                        .slot(other)
                        .is_some_and(|p| p.kind == kind && p.subject == selected)
            });
            if taken {
                self.slots[side.index()] = None;
            } else if let Some(panel) = &mut self.slots[side.index()] {
                panel.subject = selected;
                panel.scroll = 0;
            }
        }
    }

    /// Opens the Comms panel, filtered to `aircraft` when given.
    pub fn open_comms(&mut self, aircraft: Option<u32>) {
        let mut comms = self
            .comms
            .take()
            .unwrap_or_else(|| self.comms_filters.clone());
        if aircraft.is_some() {
            comms.aircraft = aircraft;
        }
        comms.anchor = None;
        self.comms = Some(comms);
    }

    pub fn close_comms(&mut self) {
        if let Some(comms) = self.comms.take() {
            self.comms_filters = CommsPanel {
                anchor: None,
                ..comms
            };
        }
    }

    pub fn toggle_comms(&mut self) {
        if self.comms.is_some() {
            self.close_comms();
        } else {
            self.open_comms(None);
        }
    }

    /// Live flight trimmed `removed` entries off the front of its list: a
    /// scrolled-back Comms panel keeps showing the same rows.
    pub fn entries_removed(&mut self, removed: usize) {
        if let Some(anchor) = self.comms.as_mut().and_then(|c| c.anchor.as_mut()) {
            *anchor = anchor.saturating_sub(removed);
        }
    }

    /// Where the open panels sit in `layout`.
    pub fn rects(&self, layout: Layout) -> Rects {
        let comms = self.comms.is_some().then_some((
            MARGIN,
            layout.bottom - COMMS_HEIGHT,
            WIDTH - 2 * MARGIN,
            COMMS_HEIGHT,
        ));
        let bottom = comms.map_or(layout.bottom, |c| c.1 - GAP);
        let side = |i: usize, x: i32| {
            self.slots[i].as_ref().map(|_| {
                (
                    x,
                    layout.top,
                    SIDE_WIDTH,
                    (bottom - layout.top).max(TITLE_HEIGHT),
                )
            })
        };
        Rects {
            sides: [side(0, MARGIN), side(1, WIDTH - MARGIN - SIDE_WIDTH)],
            comms,
        }
    }

    /// The part of a panel under layer point `at`.
    pub fn hit(&self, layout: Layout, at: (f64, f64)) -> Option<Hit> {
        let rects = self.rects(layout);
        for side in SIDES {
            let Some(r) = rects.sides[side.index()] else {
                continue;
            };
            if !inside(at, r) {
                continue;
            }
            let panel = self.slot(side)?;
            return Some(if inside(at, close_rect(r)) {
                Hit::Close(side)
            } else if panel.kind.follows() && inside(at, pin_rect(r)) {
                Hit::Pin(side)
            } else {
                Hit::Body(side)
            });
        }
        let r = rects.comms?;
        if !inside(at, r) {
            return None;
        }
        if inside(at, close_rect(r)) {
            return Some(Hit::CommsClose);
        }
        if let Some(i) = (0..CHANNELS.len()).find(|i| inside(at, chip_rect(r, *i))) {
            return Some(Hit::CommsChannel(i));
        }
        if inside(at, aircraft_chip_rect(r)) {
            return Some(Hit::CommsAircraft);
        }
        Some(Hit::CommsBody)
    }

    /// The pointer moved to layer point `at` (`None` off the layer).
    pub fn pointer(&mut self, layout: Layout, at: Option<(f64, f64)>) {
        self.hover = at.and_then(|at| self.hit(layout, at));
    }

    /// The left button went down at `at`. True when a panel took it.
    pub fn down(&mut self, layout: Layout, at: Option<(f64, f64)>) -> bool {
        self.pressed = at.and_then(|at| self.hit(layout, at));
        self.pressed.is_some()
    }

    /// The left button came up at `at`: a click when it went down on the
    /// same control. True when a panel took the press.
    pub fn up(&mut self, layout: Layout, at: Option<(f64, f64)>, data: &dyn Data) -> bool {
        let Some(pressed) = self.pressed.take() else {
            return false;
        };
        if at.and_then(|at| self.hit(layout, at)) != Some(pressed) {
            return true;
        }
        match pressed {
            Hit::Pin(side) => self.pin(side),
            Hit::Close(side) => self.close(side),
            Hit::CommsClose => self.close_comms(),
            Hit::CommsChannel(i) => {
                if let Some(comms) = &mut self.comms {
                    comms.shown[i] = !comms.shown[i];
                    comms.anchor = None;
                }
            }
            Hit::CommsAircraft => {
                if let Some(comms) = &mut self.comms {
                    let list = data.aircraft();
                    comms.aircraft = match comms.aircraft {
                        None => list.first().copied(),
                        Some(id) => list.iter().skip_while(|a| **a != id).nth(1).copied(),
                    };
                    comms.anchor = None;
                }
            }
            Hit::Body(_) | Hit::CommsBody => {}
        }
        true
    }

    /// Mouse wheel notches at `at`, up positive. True when a panel under
    /// the pointer scrolled.
    pub fn wheel(
        &mut self,
        layout: Layout,
        at: Option<(f64, f64)>,
        notches: i32,
        data: &dyn Data,
    ) -> bool {
        let Some(hit) = at.and_then(|at| self.hit(layout, at)) else {
            return false;
        };
        let steps = notches.unsigned_abs() as usize * WHEEL_LINES;
        match hit {
            Hit::Pin(side) | Hit::Close(side) | Hit::Body(side) => {
                if let Some(panel) = &mut self.slots[side.index()] {
                    let most = panel.lines.saturating_sub(panel.visible);
                    panel.scroll = if notches > 0 {
                        panel.scroll.saturating_sub(steps)
                    } else {
                        (panel.scroll + steps).min(most)
                    };
                }
            }
            Hit::CommsClose | Hit::CommsChannel(_) | Hit::CommsAircraft | Hit::CommsBody => {
                let Some(comms) = &mut self.comms else {
                    return true;
                };
                let (events, now) = data.comms();
                let end = comms.anchor.unwrap_or(now).min(now);
                if notches > 0 {
                    // Back in time: the newest rows leave the bottom.
                    let rows = rows_before(events, end, comms, steps + 1);
                    if rows.len() > 1 {
                        let keep = rows.len() - 1 - steps.min(rows.len() - 1);
                        comms.anchor = Some(rows[keep].last() + 1);
                    }
                } else {
                    // Forward: the next rows join the bottom, and past the
                    // newest the list follows the playhead again.
                    let newer = rows_since(events, end, now, comms);
                    let steps = steps.max(1);
                    comms.anchor = (newer.len() > steps).then(|| newer[steps - 1].last() + 1);
                }
            }
        }
        true
    }

    /// Draws the open panels into a cleared 640x480 layer and returns where
    /// they are, for compositing.
    pub fn draw(
        &mut self,
        pixels: &mut [u8],
        font: &Font,
        layout: Layout,
        data: &mut dyn Data,
    ) -> Vec<Rect> {
        let rects = self.rects(layout);
        let hover = self.hover;
        let pressed = self.pressed;
        let lit = |hit: Hit| hover == Some(hit) || pressed == Some(hit);
        for side in SIDES {
            let (Some(r), Some(panel)) = (rects.sides[side.index()], &mut self.slots[side.index()])
            else {
                continue;
            };
            let tree = data.tree(panel.subject, panel.kind.channel());
            let subject = data.title(panel.kind, panel.subject);
            frame(pixels, font, r, panel.kind.title(), &subject);
            if panel.kind.follows() {
                small_button(
                    pixels,
                    font,
                    pin_rect(r),
                    "Pin",
                    panel.pinned || lit(Hit::Pin(side)),
                );
            }
            small_button(pixels, font, close_rect(r), "x", lit(Hit::Close(side)));
            draw_tree(pixels, font, r, panel, tree, data);
        }
        if let (Some(r), Some(comms)) = (rects.comms, &mut self.comms) {
            let title = match comms.aircraft {
                Some(id) => format!("for {}", data.name(id)),
                None => "every aircraft".into(),
            };
            frame(pixels, font, r, "COMMS", &title);
            small_button(pixels, font, close_rect(r), "x", lit(Hit::CommsClose));
            for (i, channel) in CHANNELS.iter().enumerate() {
                chip(
                    pixels,
                    font,
                    chip_rect(r, i),
                    channel.label(),
                    comms.shown[i],
                    lit(Hit::CommsChannel(i)),
                );
            }
            let who = match comms.aircraft {
                Some(id) => format!("Aircraft: {}", data.name(id)),
                None => "Aircraft: all".into(),
            };
            chip(
                pixels,
                font,
                aircraft_chip_rect(r),
                &who,
                comms.aircraft.is_some(),
                lit(Hit::CommsAircraft),
            );
            draw_comms(pixels, font, r, comms, &*data);
        }
        rects.all()
    }
}

fn text(pixels: &mut [u8], font: &Font, clip: Rect, color: [u8; 4], line: &str, at: (i32, i32)) {
    Editor::text(pixels, font, clip, color, line, at);
}

/// The height of one text line.
pub fn line_height(font: &Font) -> i32 {
    font.height as i32 + 3
}

/// A panel's background and title strip: the kind in the title colour,
/// then the subject.
fn frame(pixels: &mut [u8], font: &Font, r: Rect, kind: &str, subject: &str) {
    Canvas(pixels).rect(r, BODY);
    Canvas(pixels).rect((r.0, r.1, r.2, TITLE_HEIGHT), PANEL);
    let x = r.0 + 5;
    let y = r.1 + (TITLE_HEIGHT - font.height as i32) / 2;
    text(pixels, font, r, TITLE, kind, (x, y));
    let after = x + text_width(font, kind) + 8;
    let room = pin_rect(r).0 - 4 - after;
    text(
        pixels,
        font,
        r,
        WHITE,
        &fit(font, &ascii(subject), room),
        (after, y),
    );
}

fn small_button(pixels: &mut [u8], font: &Font, r: Rect, label: &str, lit: bool) {
    Canvas(pixels).rect(r, if lit { FOCUS } else { HEADER });
    let x = r.0 + (r.2 - text_width(font, label)) / 2;
    let y = r.1 + (r.3 - font.height as i32) / 2;
    text(
        pixels,
        font,
        r,
        if lit { WHITE } else { PALE },
        label,
        (x, y),
    );
}

fn chip(pixels: &mut [u8], font: &Font, r: Rect, label: &str, on: bool, hover: bool) {
    Canvas(pixels).rect(
        r,
        match (on, hover) {
            (_, true) => FOCUS,
            (true, false) => HEADER,
            (false, false) => CHIP_OFF,
        },
    );
    let label = fit(font, &ascii(label), r.2 - 4);
    let x = r.0 + (r.2 - text_width(font, &label)) / 2;
    let y = r.1 + (r.3 - font.height as i32) / 2;
    text(
        pixels,
        font,
        r,
        if on || hover { WHITE } else { MUTED },
        &label,
        (x, y),
    );
}

/// A vertical scroll bar along the right edge of `r`.
fn scroll_bar(pixels: &mut [u8], r: Rect, first: usize, shown: usize, total: usize) {
    if total <= shown || r.3 <= 0 {
        return;
    }
    let track = (r.0 + r.2 - 3, r.1, 2, r.3);
    Canvas(pixels).rect(track, HEADER);
    let h = (track.3 * shown as i32 / total as i32).max(6);
    let most = total - shown;
    let y = track.1 + (track.3 - h) * first.min(most) as i32 / most as i32;
    Canvas(pixels).rect((track.0, y, 2, h), TITLE);
}

/// Text the retail font can draw: ASCII, with common punctuation swapped
/// for its plain equivalent and anything else shown as `?`.
pub fn ascii(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            ' '..='~' => c,
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            '\u{b0}' => 'o',
            '\t' | '\n' | '\r' => ' ',
            _ => '?',
        })
        .collect()
}

/// `text` broken into lines no wider than `width`, at spaces where it can,
/// and inside a word too long for a line.
pub fn wrap(font: &Font, text: &str, width: i32) -> Vec<String> {
    let text = ascii(text);
    let width = width.max(8);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if text_width(font, &candidate) <= width {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        // A word wider than the line is split where it must be.
        for c in word.chars() {
            let mut grown = line.clone();
            grown.push(c);
            if text_width(font, &grown) > width && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                line.push(c);
            } else {
                line = grown;
            }
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// Rounded to `decimals`, trailing zeros trimmed, never `-0`.
fn trimmed(v: f64, decimals: usize) -> String {
    let mut text = format!("{v:.decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".into();
    }
    text
}

/// A whole number with thousands separators.
fn thousands(v: f64) -> String {
    let rounded = v.round();
    let digits = format!("{:.0}", rounded.abs());
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if rounded < 0. && out != "0" {
        format!("-{out}")
    } else {
        out
    }
}

/// A number as a panel shows it in `unit`: whole feet, pounds and knots
/// with thousands separators, tenths of miles, degrees and percent,
/// hundredths of G, seconds and ratios.
pub fn number(v: f64, unit: &str) -> String {
    use vocab::unit as u;
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0. { "infinity" } else { "-infinity" }.into();
    }
    match unit {
        u::FT | u::FT_S | u::KT | u::LB | u::LB_FT2 => thousands(v),
        u::NM | u::DEG | u::DEG_S | u::PERCENT => trimmed(v, 1),
        u::G | u::S | u::RATIO => trimmed(v, 2),
        _ if v.abs() >= 10_000. => thousands(v),
        _ => trimmed(v, 3),
    }
}

/// A number with its unit: `12,340 ft`, `6.4 G`, `x0.84`, `35%`.
fn with_unit(v: f64, unit: &str) -> String {
    let n = number(v, unit);
    match unit {
        "" => n,
        vocab::unit::RATIO => format!("x{n}"),
        vocab::unit::PERCENT => format!("{n}%"),
        vocab::unit::G => format!("{n} G"),
        _ => format!("{n} {unit}"),
    }
}

/// A tree value as a panel shows it, with its unit. Aircraft ids read as
/// their labels.
pub fn value_text(value: &Value, unit: &str, name: &dyn Fn(u32) -> String) -> String {
    match value {
        Value::None => String::new(),
        Value::Bool(v) => if *v { "yes" } else { "no" }.into(),
        Value::Int(v) => with_unit(*v as f64, unit),
        Value::Num(v) => with_unit(*v, unit),
        Value::Text(v) if unit.is_empty() => v.clone(),
        Value::Text(v) => format!("{v} {unit}"),
        Value::Id(id) => name(*id),
        Value::Ids(ids) if ids.is_empty() => "none".into(),
        Value::Ids(ids) => ids
            .iter()
            .map(|id| name(*id))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// One drawn line of a tree panel. Positions are from the panel's inner
/// left edge.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeLine {
    pub x: i32,
    pub text: String,
    pub color: [u8; 4],
    /// A value on the same line: where it starts, its text and colour.
    pub value: Option<(i32, String, [u8; 4])>,
}

impl TreeLine {
    fn plain(x: i32, text: String, color: [u8; 4]) -> Self {
        Self {
            x,
            text,
            color,
            value: None,
        }
    }
}

/// A tree's nodes as lines `width` wide: each label indented by its depth
/// with its value in a column beside it, and its "because" note on lines
/// of its own beneath.
pub fn tree_lines(
    nodes: &[Node],
    font: &Font,
    width: i32,
    name: &dyn Fn(u32) -> String,
) -> Vec<TreeLine> {
    let mut out = Vec::new();
    for node in nodes {
        let indent = i32::from(node.depth).min(MAX_INDENT_DEPTH) * INDENT;
        let label = ascii(&node.label);
        let label_color = if node.depth == 0 { TITLE } else { PALE };
        let value = ascii(&value_text(&node.value, &node.unit, name));
        let value_color = match &node.value {
            Value::Num(v) if !v.is_finite() => BAD,
            _ => WHITE,
        };
        // A label too long for its line wraps, the rest indented a level.
        let wrapped = |out: &mut Vec<TreeLine>| {
            for (i, part) in wrap(font, &label, width - indent - INDENT)
                .into_iter()
                .enumerate()
            {
                let x = if i == 0 { indent } else { indent + INDENT };
                out.push(TreeLine::plain(x, part, label_color));
            }
        };
        if value.is_empty() {
            if text_width(font, &label) <= width - indent {
                out.push(TreeLine::plain(indent, label.clone(), label_color));
            } else {
                wrapped(&mut out);
            }
        } else {
            let label_width = text_width(font, &label);
            let mut x = VALUE_X.max(indent + label_width + 6);
            let mut first = Some(label.clone());
            if x > width - 40 {
                // No room beside a long label: the value goes beneath it.
                first = None;
                wrapped(&mut out);
                x = (indent + INDENT).min(VALUE_X);
            }
            for part in wrap(font, &value, width - x) {
                let mut line =
                    TreeLine::plain(indent, first.take().unwrap_or_default(), label_color);
                line.value = Some((x, part, value_color));
                out.push(line);
            }
        }
        if !node.note.is_empty() {
            let x = indent + INDENT;
            for part in wrap(font, &format!("because {}", node.note), width - x) {
                out.push(TreeLine::plain(x, part, NOTE));
            }
        }
    }
    out
}

/// A tree panel's body: when the sample was taken, then its lines.
fn draw_tree(
    pixels: &mut [u8],
    font: &Font,
    r: Rect,
    panel: &mut Panel,
    tree: Option<(u64, TreeSample)>,
    data: &dyn Data,
) {
    let height = line_height(font);
    let inner = (
        r.0 + 6,
        r.1 + TITLE_HEIGHT + 3,
        r.2 - 14,
        r.3 - TITLE_HEIGHT - 6,
    );
    let now = data.now();
    let Some((tick, tree)) = tree else {
        panel.lines = 0;
        panel.visible = 0;
        for (i, line) in wrap(font, &data.missing(panel.kind, panel.subject), inner.2)
            .iter()
            .enumerate()
        {
            text(
                pixels,
                font,
                r,
                MUTED,
                line,
                (inner.0, inner.1 + i as i32 * height),
            );
        }
        return;
    };
    let age = now.saturating_sub(tick);
    let status = if age == 0 {
        format!("Sampled at {}", clock::timestamp(tick as f64))
    } else {
        format!(
            "Sampled at {}, {} s earlier",
            clock::timestamp(tick as f64),
            trimmed(age as f64 / 120., 2)
        )
    };
    text(
        pixels,
        font,
        r,
        if age > STALE_TICKS { WARN } else { MUTED },
        &fit(font, &status, inner.2),
        (inner.0, inner.1),
    );
    let name = |id: u32| data.name(id);
    let lines = tree_lines(&tree.nodes, font, inner.2, &name);
    let top = inner.1 + height + 2;
    let visible = ((inner.1 + inner.3 - top) / height).max(0) as usize;
    panel.lines = lines.len();
    panel.visible = visible;
    panel.scroll = panel.scroll.min(lines.len().saturating_sub(visible));
    let clip = (inner.0, top, inner.2, visible as i32 * height);
    for (row, line) in lines.iter().skip(panel.scroll).take(visible).enumerate() {
        let y = top + row as i32 * height;
        text(
            pixels,
            font,
            clip,
            line.color,
            &line.text,
            (inner.0 + line.x, y),
        );
        if let Some((x, value, color)) = &line.value {
            text(pixels, font, clip, *color, value, (inner.0 + x, y));
        }
    }
    scroll_bar(
        pixels,
        (r.0 + r.2 - 4, top, 4, visible as i32 * height),
        panel.scroll,
        visible,
        lines.len(),
    );
}

/// Comms kinds as the panel labels them.
pub fn kind_label(kind: &str) -> String {
    use vocab::kind as k;
    match kind {
        k::COMMS_RADIO => "RADIO",
        k::COMMS_CREW => "CREW",
        k::COMMS_TOWER => "TOWER",
        k::COMMS_HUD => "HUD",
        k::COMMS_ORDER => "ORDER",
        k::COMMS_REQUEST => "REQUEST",
        k::COMMS_REPORT => "REPORT",
        k::COMMS_DELIVERY => "ANSWER",
        k::AUDIO_TONE => "TONE",
        k::AUDIO_STALL_WARNING => "STALL",
        k::AUDIO_MUSIC => "MUSIC",
        k::AUDIO_EFFECT => "EFFECT",
        k::AUDIO_RELEASE => "RELEASE",
        k::AUDIO_EJECTION => "EJECT",
        k::AUDIO_DEVICE => "DEVICE",
        other => {
            return other
                .split_once('.')
                .map_or(other, |(_, name)| name)
                .to_ascii_uppercase();
        }
    }
    .into()
}

/// Fields a Comms row already shows in its own words.
const SHOWN_FIELDS: [&str; 20] = [
    vocab::field::SPEAKER,
    vocab::field::RECIPIENTS,
    vocab::field::ORDER,
    vocab::field::OUTCOME,
    vocab::field::REASON,
    vocab::field::TRIGGER,
    vocab::field::HEARD,
    vocab::field::WAIT_S,
    vocab::field::DUE_S,
    vocab::field::ROLLS,
    vocab::field::SOUND,
    vocab::field::TONE,
    vocab::field::ON,
    vocab::field::DEVICE,
    vocab::field::FROM,
    vocab::field::TO,
    vocab::field::X_FT,
    vocab::field::Y_FT,
    vocab::field::Z_FT,
    vocab::field::MESSAGE,
];

/// Fields left out as noise: the recordings a line plays, its route, its
/// producer and kind of call, and a recipient's place in the wing.
const HIDDEN_FIELDS: [&str; 5] = [
    vocab::field::STEMS,
    vocab::field::ROUTE,
    vocab::field::SOURCE,
    vocab::field::KIND,
    "member",
];

/// How an outcome reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    /// It went out or was acted on.
    Good,
    /// Still waiting, or some recipients took it and some did not.
    Pending,
    /// Held back, dropped, refused or cut off.
    Bad,
    /// Nothing to act on: a check that found nothing to say, a state
    /// change, a line said on a radio the player does not hear.
    Quiet,
}

impl Mood {
    pub fn of(outcome: &str) -> Self {
        use vocab::outcome as o;
        match outcome {
            o::DELIVERED | o::APPLIED | o::ANSWERED | o::HIT => Self::Good,
            o::QUEUED => Self::Pending,
            o::SILENT | o::NOTED | o::UNHEARD => Self::Quiet,
            _ => Self::Bad,
        }
    }

    fn color(self) -> [u8; 4] {
        match self {
            Self::Good => GOOD,
            Self::Pending => WARN,
            Self::Bad => BAD,
            Self::Quiet => MUTED,
        }
    }
}

/// A Comms row in words: what was said or played, its latest outcome, and
/// the reasons, the earlier states and each recipient's answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// When the row's newest entry happened.
    pub time: String,
    pub kind: String,
    pub channel: Option<Channel>,
    /// Who, to whom, and what.
    pub main: String,
    /// The latest outcome in words and how it reads.
    pub outcome: Option<(String, Mood)>,
    /// Why, the trigger, the rolls, the wait, earlier states and other
    /// fields.
    pub details: String,
    /// Each recipient's answer, and how it reads.
    pub answers: Vec<(String, Mood)>,
    /// The player could not hear it.
    pub unheard: bool,
}

fn field_text(value: &Value, name: &dyn Fn(u32) -> String) -> String {
    match value {
        Value::Num(v) => trimmed(*v, 2),
        other => value_text(other, "", name),
    }
}

/// What an entry says: its words, order, tone, music change, device or
/// sound.
fn content(event: &Event, name: &dyn Fn(u32) -> String) -> String {
    use vocab::field as f;
    let on_off = match event.flag(f::ON) {
        Some(true) => " on",
        Some(false) => " off",
        None => "",
    };
    if !event.text.is_empty() {
        event.text.clone()
    } else if let Some(order) = event.string(f::ORDER) {
        order.to_owned()
    } else if let Some(tone) = event.string(f::TONE) {
        format!("{tone}{on_off}")
    } else if event.kind == vocab::kind::AUDIO_STALL_WARNING {
        format!("stall warning{on_off}")
    } else if let (Some(from), Some(to)) = (event.get(f::FROM), event.get(f::TO)) {
        format!("{} -> {}", field_text(from, name), field_text(to, name))
    } else if let Some(device) = event.string(f::DEVICE) {
        device.to_owned()
    } else if let Some(sound) = event.string(f::SOUND) {
        sound.to_owned()
    } else {
        String::new()
    }
}

/// The outcomes of `answers` counted in the order they first appear:
/// `applied 1, rejected 1`, and how that reads.
fn answer_summary(answers: &[&TimedEvent]) -> Option<(String, Mood)> {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for answer in answers {
        let outcome = answer
            .event
            .string(vocab::field::OUTCOME)
            .unwrap_or("answered");
        match counts.iter_mut().find(|(o, _)| *o == outcome) {
            Some((_, n)) => *n += 1,
            None => counts.push((outcome, 1)),
        }
    }
    let moods: Vec<Mood> = counts.iter().map(|(o, _)| Mood::of(o)).collect();
    let mood = match moods.first()? {
        first if moods.iter().all(|m| m == first) => *first,
        _ => Mood::Pending,
    };
    let words = counts
        .iter()
        .map(|(o, n)| format!("{o} {n}"))
        .collect::<Vec<_>>()
        .join(", ");
    Some((words, mood))
}

/// Any other recorded field in words: `kept_s` 2 reads `kept 2 s`,
/// `launch_range_ft` 12000 reads `launch range 12000 ft`.
fn extra_text(field: &str, value: &str) -> String {
    for (suffix, unit) in [
        ("_s", "s"),
        ("_ft", "ft"),
        ("_kt", "kt"),
        ("_nm", "nm"),
        ("_deg", "deg"),
        ("_fps", "ft/s"),
        ("_lb", "lb"),
    ] {
        if let Some(name) = field.strip_suffix(suffix) {
            return format!("{} {value} {unit}", name.replace('_', " "));
        }
    }
    format!("{} {value}", field.replace('_', " "))
}

/// A Comms row in words.
pub fn entry(events: &[TimedEvent], row: &Row, name: &dyn Fn(u32) -> String) -> Entry {
    use vocab::{field as f, kind as k};
    let all: Vec<&TimedEvent> = row.entries.iter().map(|&i| &events[i]).collect();
    let (answers, said): (Vec<&TimedEvent>, Vec<&TimedEvent>) = all
        .iter()
        .copied()
        .partition(|e| e.event.kind == k::COMMS_DELIVERY && all.len() > 1);
    let main = said.first().copied().unwrap_or(all[0]);
    let last = said.last().copied().unwrap_or(main);
    let newest = all[all.len() - 1];
    let event = &main.event;
    let who = event
        .string(f::SPEAKER)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or_else(|| event.subject.map(name));
    let to = event
        .get(f::RECIPIENTS)
        .and_then(Value::as_ids)
        .filter(|ids| !ids.is_empty())
        .map(|ids| {
            ids.iter()
                .map(|id| name(*id))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .or_else(|| event.object.map(name));
    let mut words = content(event, name);
    if words.is_empty() {
        words = content(&last.event, name);
    }
    let sound_in_words = event
        .string(f::SOUND)
        .is_none_or(|sound| words.contains(sound));
    let mut main_text = who.unwrap_or_default();
    if let Some(to) = to {
        main_text = if main_text.is_empty() {
            format!("to {to}")
        } else {
            format!("{main_text} -> {to}")
        };
    }
    if !words.is_empty() {
        main_text = if main_text.is_empty() {
            words
        } else {
            format!("{main_text}: {words}")
        };
    }
    // A burst of one sound: how many and over how long.
    if all.len() > 1 && message(event).is_none() {
        main_text = format!(
            "{main_text} x{} over {} s",
            all.len(),
            trimmed((newest.tick - main.tick) as f64 / 120., 1)
        );
    }
    let heard = last.event.flag(f::HEARD);
    let spoken = matches!(
        last.event.kind.as_str(),
        k::COMMS_RADIO | k::COMMS_CREW | k::COMMS_TOWER
    );
    let own = last.event.string(f::OUTCOME);
    let outcome = if !answers.is_empty()
        && own.is_none_or(|o| o == vocab::outcome::QUEUED || o == vocab::outcome::ANSWERED)
    {
        answer_summary(&answers)
    } else {
        let mut words = own.map(str::to_owned).unwrap_or_default();
        let mood = own.map_or(Mood::Good, Mood::of);
        let mut add = |part: &str| {
            if !words.is_empty() {
                words.push_str(", ");
            }
            words.push_str(part);
        };
        if spoken && vocab::heard(&last.event) {
            add("heard by you");
        } else if heard == Some(false) && own != Some(vocab::outcome::UNHEARD) {
            add("not heard");
        }
        // Said, but not to the player: nothing went wrong.
        let mood = match (mood, heard) {
            (Mood::Good, Some(false)) => Mood::Quiet,
            (mood, _) => mood,
        };
        (!words.is_empty()).then_some((words, mood))
    };
    let mut details: Vec<String> = Vec::new();
    let mut reasons: Vec<String> = Vec::new();
    for said in said.iter().rev() {
        if let Some(reason) = said.event.get(f::REASON).map(|v| field_text(v, name))
            && !reasons.contains(&reason)
        {
            reasons.push(reason);
        }
    }
    for reason in reasons {
        details.push(format!("why: {reason}"));
    }
    if let Some(trigger) = event.get(f::TRIGGER).map(|v| field_text(v, name)) {
        details.push(format!("trigger: {trigger}"));
    }
    if let Some(rolls) = event.get(f::ROLLS).map(|v| field_text(v, name)) {
        details.push(format!("rolls: {rolls}"));
    }
    if let Some(wait) = last.event.num(f::WAIT_S) {
        details.push(format!("waited {} s", trimmed(wait, 2)));
    }
    // The line's earlier states, oldest first.
    for earlier in &said[..said.len().saturating_sub(1)] {
        let state = earlier.event.string(f::OUTCOME).unwrap_or("sent");
        let mut line = format!("{state} at {}", clock::timestamp(earlier.tick as f64));
        if let Some(due) = earlier.event.num(f::DUE_S) {
            line.push_str(&format!(", due in {} s", trimmed(due, 2)));
        }
        details.push(line);
    }
    if said.len() == 1
        && let Some(due) = last.event.num(f::DUE_S)
    {
        details.push(format!("due in {} s", trimmed(due, 2)));
    }
    if !sound_in_words {
        details.push(format!(
            "sound {}",
            event.string(f::SOUND).unwrap_or_default()
        ));
    }
    for (field, value) in &last.event.fields {
        let known = SHOWN_FIELDS.contains(&field.as_str())
            || (HIDDEN_FIELDS.contains(&field.as_str()) && last.event.kind.starts_with("comms."));
        if known {
            continue;
        }
        let value = field_text(value, name);
        if !value.is_empty() {
            details.push(extra_text(field, &value));
        }
    }
    let answers = answers
        .iter()
        .map(|answer| {
            let e = &answer.event;
            let outcome = e.string(f::OUTCOME).unwrap_or("answered");
            let mut line = format!(
                "{} {outcome}",
                e.subject.map_or_else(|| "someone".to_owned(), name)
            );
            if let Some(reason) = e.get(f::REASON).map(|v| field_text(v, name)) {
                line.push_str(&format!(": {reason}"));
            }
            if answer.tick != newest.tick {
                line.push_str(&format!(" ({})", clock::timestamp(answer.tick as f64)));
            }
            (line, Mood::of(outcome))
        })
        .collect();
    Entry {
        time: clock::timestamp(newest.tick as f64),
        kind: kind_label(&event.kind),
        channel: Channel::of(&event.kind),
        main: main_text,
        outcome,
        details: details.join("; "),
        answers,
        unheard: heard == Some(false),
    }
}

/// One Comms row's lines, with where each piece of text starts from the
/// panel's inner left edge.
pub(crate) fn entry_lines(
    font: &Font,
    entry: &Entry,
    width: i32,
) -> Vec<Vec<(i32, String, [u8; 4])>> {
    let text_x = 104;
    let room = width - text_x;
    let color = if entry.unheard { PALE } else { WHITE };
    let mut lines: Vec<Vec<(i32, String, [u8; 4])>> = Vec::new();
    let mut last_width = 0;
    for (i, part) in wrap(font, &entry.main, room).into_iter().enumerate() {
        let mut pieces = Vec::new();
        if i == 0 {
            pieces.push((0, entry.time.clone(), MUTED));
            pieces.push((
                48,
                entry.kind.clone(),
                entry.channel.map_or(PALE, Channel::color),
            ));
        }
        last_width = text_width(font, &part);
        pieces.push((text_x, part, color));
        lines.push(pieces);
    }
    // The outcome follows in brackets in its own colour: green when the
    // line went out, amber while it waits, red when it was held back or
    // refused.
    if let Some((outcome, mood)) = &entry.outcome {
        let tag = format!("[{outcome}]");
        let gap = if entry.main.is_empty() { 0 } else { 6 };
        match lines.last_mut() {
            Some(line) if last_width + gap + text_width(font, &tag) <= room => {
                line.push((text_x + last_width + gap, tag, mood.color()));
            }
            _ => {
                for part in wrap(font, &tag, room) {
                    lines.push(vec![(text_x, part, mood.color())]);
                }
            }
        }
    }
    if !entry.details.is_empty() {
        for part in wrap(font, &entry.details, room - 8) {
            lines.push(vec![(text_x + 8, part, NOTE)]);
        }
    }
    for (answer, mood) in &entry.answers {
        for part in wrap(font, answer, room - 8) {
            lines.push(vec![(text_x + 8, part, mood.color())]);
        }
    }
    lines
}

/// The Comms panel's body: rows up to the playhead (or up to where it was
/// scrolled back to), newest at the bottom.
fn draw_comms(pixels: &mut [u8], font: &Font, r: Rect, comms: &mut CommsPanel, data: &dyn Data) {
    let height = line_height(font);
    let top = r.1 + TITLE_HEIGHT + 18;
    let inner = (r.0 + 6, top, r.2 - 14, r.1 + r.3 - 3 - top);
    let visible = (inner.3 / height).max(0) as usize;
    let (events, now) = data.comms();
    if comms.anchor.is_some_and(|a| a >= now) {
        comms.anchor = None;
    }
    let end = comms.anchor.unwrap_or(now);
    let name = |id: u32| data.name(id);
    // Enough rows to fill the panel even when each takes one line; only
    // whole rows show, the newest at the bottom.
    let rows = rows_before(events, end, comms, visible);
    let mut lines: Vec<Vec<(i32, String, [u8; 4])>> = Vec::new();
    for row in rows.iter().rev() {
        let mut row_lines = entry_lines(font, &entry(events, row, &name), inner.2);
        if !lines.is_empty() && lines.len() + row_lines.len() > visible {
            break;
        }
        row_lines.append(&mut lines);
        lines = row_lines;
        if lines.len() >= visible {
            break;
        }
    }
    // A single row taller than the panel shows its start.
    lines.truncate(visible);
    if lines.is_empty() {
        let note = if comms.anchor.is_some() || events.is_empty() || now == 0 {
            "Nothing said yet"
        } else {
            "Nothing matches the filters yet"
        };
        text(pixels, font, r, MUTED, note, (inner.0, inner.1));
    }
    let clip = (inner.0, inner.1, inner.2, visible as i32 * height);
    for (row, pieces) in lines.iter().enumerate() {
        let y = inner.1 + row as i32 * height;
        for (x, part, color) in pieces {
            text(pixels, font, clip, *color, part, (inner.0 + x, y));
        }
    }
    if comms.anchor.is_some() {
        let note = "Scrolled back: wheel down to follow";
        let w = text_width(font, note) + 8;
        let at = (r.0 + r.2 - w - 4, r.1 + r.3 - height - 2, w, height);
        Canvas(pixels).rect(at, HEADER);
        text(pixels, font, at, WARN, note, (at.0 + 4, at.1 + 1));
    }
}

/// Display trees read from a recording at any tick, a chunk at a time:
/// [`Recording::tree`] decodes a chunk on every call, so each chunk's trees
/// are decoded once and kept while the playhead is near.
#[derive(Default)]
pub struct RecordedTrees {
    /// Decoded chunks, most recently used last.
    chunks: Vec<(usize, ChunkTrees)>,
    /// The latest sample before each chunk began, by subject, channel and
    /// chunk.
    before: HashMap<(u32, String, usize), Option<(u64, TreeSample)>>,
}

/// Every tree sample of one chunk, with its tick.
type ChunkTrees = Arc<Vec<(u64, TreeSample)>>;

/// Decoded chunks of trees kept around the playhead.
const TREE_CHUNKS: usize = 6;
/// Earlier samples remembered before the list starts over.
const BEFORE_LIMIT: usize = 512;

impl RecordedTrees {
    fn chunk(&mut self, recording: &Recording, index: usize) -> ChunkTrees {
        if let Some(at) = self.chunks.iter().position(|(i, _)| *i == index) {
            let entry = self.chunks.remove(at);
            let trees = Arc::clone(&entry.1);
            self.chunks.push(entry);
            return trees;
        }
        let trees = Arc::new(recording.chunk_trees(index).unwrap_or_else(|error| {
            log::warn!("Replay: the trees of chunk {index} are unreadable: {error}");
            Vec::new()
        }));
        if self.chunks.len() >= TREE_CHUNKS {
            self.chunks.remove(0);
        }
        self.chunks.push((index, Arc::clone(&trees)));
        trees
    }

    /// The latest sample of `subject` on `channel` at or before `tick`,
    /// with the tick it was taken. Inside a gap, the chunk before the gap
    /// answers.
    pub fn tree(
        &mut self,
        recording: &Recording,
        subject: u32,
        channel: &str,
        tick: u64,
    ) -> Option<(u64, TreeSample)> {
        let chunks = recording.chunks();
        let index = chunks
            .partition_point(|c| c.first_tick <= tick)
            .checked_sub(1)?;
        let trees = self.chunk(recording, index);
        if let Some((t, sample)) = trees
            .iter()
            .rev()
            .find(|(t, s)| *t <= tick && s.subject == subject && s.channel == channel)
        {
            return Some((*t, sample.clone()));
        }
        let key = (subject, channel.to_owned(), index);
        if let Some(found) = self.before.get(&key) {
            return found.clone();
        }
        let found = chunks[index]
            .first_tick
            .checked_sub(1)
            .and_then(|t| recording.tree(subject, channel, t).ok().flatten());
        if self.before.len() >= BEFORE_LIMIT {
            self.before.clear();
        }
        self.before.insert(key, found.clone());
        found
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tore_replay::Event;

    /// A synthetic font: every glyph five pixels wide and seven high.
    pub(crate) fn font() -> Font {
        Font {
            height: 7,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 5,
                    pixels: vec![(0, 0), (1, 1), (2, 2), (3, 3)],
                })
                .collect(),
        }
    }

    pub(crate) struct Fake {
        pub now: u64,
        pub trees: Vec<(u64, TreeSample)>,
        pub events: Vec<TimedEvent>,
    }

    impl Data for Fake {
        fn now(&self) -> u64 {
            self.now
        }
        fn tree(&mut self, subject: u32, channel: &str) -> Option<(u64, TreeSample)> {
            self.trees
                .iter()
                .rev()
                .find(|(t, s)| *t <= self.now && s.subject == subject && s.channel == channel)
                .cloned()
        }
        fn name(&self, id: u32) -> String {
            match id {
                0 => "You".into(),
                id => format!("Enemy 1-{id}"),
            }
        }
        fn title(&self, _: Kind, subject: u32) -> String {
            self.name(subject)
        }
        fn missing(&self, _: Kind, _: u32) -> String {
            "Nothing recorded".into()
        }
        fn comms(&self) -> (&[TimedEvent], usize) {
            let end = self.events.partition_point(|e| e.tick <= self.now);
            (&self.events, end)
        }
        fn aircraft(&self) -> Vec<u32> {
            vec![0, 1, 2]
        }
    }

    fn timed(tick: u64, event: Event) -> TimedEvent {
        TimedEvent { tick, event }
    }

    fn thought(subject: u32, lines: usize) -> TreeSample {
        TreeSample {
            subject,
            channel: vocab::channel::AI_THOUGHT.into(),
            nodes: (0..lines)
                .map(|i| Node::new((i % 3) as u8, &format!("Line {i}"), i as i64))
                .collect(),
        }
    }

    #[test]
    fn panels_open_left_then_right_and_replace_the_older_unpinned_one() {
        let mut p = Panels::default();
        assert_eq!(p.open(Kind::Thought, 1), Ok(Side::Left));
        assert_eq!(p.open(Kind::Telemetry, 1), Ok(Side::Right));
        // Already open: stays where it is.
        assert_eq!(p.open(Kind::Thought, 1), Ok(Side::Left));
        // A third replaces the older unpinned panel.
        assert_eq!(p.open(Kind::Guidance, 7), Ok(Side::Left));
        assert_eq!(p.slot(Side::Left).unwrap().kind, Kind::Guidance);
        p.pin(Side::Right);
        assert_eq!(p.open(Kind::Thought, 2), Ok(Side::Left));
        p.pin(Side::Left);
        assert!(p.open(Kind::Telemetry, 3).is_err());
        // Toggling an open panel closes it.
        assert_eq!(p.toggle(Kind::Thought, 2), Ok(false));
        assert!(p.slot(Side::Left).is_none());
        assert_eq!(p.toggle(Kind::Thought, 2), Ok(true));
        p.close_all();
        assert!(p.is_empty());
    }

    #[test]
    fn unpinned_aircraft_panels_follow_the_selection() {
        let mut p = Panels::default();
        p.open(Kind::Thought, 1).unwrap();
        p.open(Kind::Telemetry, 1).unwrap();
        p.pin(Side::Right);
        p.follow(2);
        assert_eq!(p.slot(Side::Left).unwrap().subject, 2);
        assert_eq!(p.slot(Side::Right).unwrap().subject, 1);
        // A guidance panel stays on its missile.
        p.close_all();
        p.open(Kind::Guidance, 7).unwrap();
        p.follow(3);
        assert_eq!(p.slot(Side::Left).unwrap().subject, 7);
        // Two thought panels would show the same aircraft: the follower
        // closes.
        p.close_all();
        p.open(Kind::Thought, 1).unwrap();
        p.open(Kind::Thought, 2).unwrap();
        p.pin(Side::Right);
        p.follow(2);
        assert!(p.slot(Side::Left).is_none());
        assert_eq!(p.slot(Side::Right).unwrap().subject, 2);
    }

    #[test]
    fn panel_rects_fit_the_layout_and_the_comms_panel_shortens_the_sides() {
        let mut p = Panels::default();
        p.open(Kind::Thought, 1).unwrap();
        p.open(Kind::Telemetry, 1).unwrap();
        let rects = p.rects(REPLAY);
        let [left, right] = rects.sides.map(Option::unwrap);
        assert_eq!(left, (6, 28, 254, 398));
        assert_eq!(right, (380, 28, 254, 398));
        assert!(rects.comms.is_none());
        p.open_comms(None);
        let rects = p.rects(REPLAY);
        let comms = rects.comms.unwrap();
        assert_eq!(comms, (6, 276, 628, 150));
        for side in rects.sides.iter().flatten() {
            assert_eq!(side.1 + side.3, comms.1 - GAP);
        }
        // Live flight has no transport bar.
        assert_eq!(p.rects(LIVE).comms.unwrap().1 + COMMS_HEIGHT, 472);
        // Every part lies inside the layer and the panels never overlap.
        for r in rects.all() {
            assert!(r.0 >= 0 && r.1 >= 0 && r.0 + r.2 <= WIDTH && r.1 + r.3 <= HEIGHT);
        }
        assert!(left.0 + left.2 < right.0);
    }

    #[test]
    fn clicks_pin_close_and_filter() {
        let mut p = Panels::default();
        let data = Fake {
            now: 0,
            trees: Vec::new(),
            events: Vec::new(),
        };
        p.open(Kind::Thought, 1).unwrap();
        p.open_comms(None);
        let rects = p.rects(REPLAY);
        let centre = |r: Rect| (f64::from(r.0 + r.2 / 2), f64::from(r.1 + r.3 / 2));
        let left = rects.sides[0].unwrap();
        let click = |p: &mut Panels, at| {
            assert!(p.down(REPLAY, Some(at)));
            assert!(p.up(REPLAY, Some(at), &data));
        };
        assert_eq!(
            p.hit(REPLAY, centre(pin_rect(left))),
            Some(Hit::Pin(Side::Left))
        );
        click(&mut p, centre(pin_rect(left)));
        assert!(p.slot(Side::Left).unwrap().pinned);
        // A press that leaves the control clicks nothing but is still taken.
        assert!(p.down(REPLAY, Some(centre(pin_rect(left)))));
        assert!(p.up(REPLAY, Some(centre(left)), &data));
        assert!(p.slot(Side::Left).unwrap().pinned);
        let comms = rects.comms.unwrap();
        click(&mut p, centre(chip_rect(comms, 4)));
        assert_eq!(p.comms().unwrap().shown, [true, true, true, true, false]);
        click(&mut p, centre(aircraft_chip_rect(comms)));
        assert_eq!(p.comms().unwrap().aircraft, Some(0));
        click(&mut p, centre(aircraft_chip_rect(comms)));
        click(&mut p, centre(aircraft_chip_rect(comms)));
        click(&mut p, centre(aircraft_chip_rect(comms)));
        assert_eq!(p.comms().unwrap().aircraft, None);
        click(&mut p, centre(close_rect(left)));
        assert!(p.slot(Side::Left).is_none());
        click(&mut p, centre(close_rect(comms)));
        assert!(!p.comms_open());
        // Filters survive closing and reopening.
        p.open_comms(Some(2));
        let comms = p.comms().unwrap();
        assert_eq!((comms.shown[4], comms.aircraft), (false, Some(2)));
        // Outside every panel nothing is taken.
        assert!(!p.down(REPLAY, Some((320., 100.))));
        assert!(!p.up(REPLAY, Some((320., 100.)), &data));
    }

    #[test]
    fn values_read_with_their_units() {
        let name = |id: u32| format!("A{id}");
        let v = |value: Value, unit: &str| value_text(&value, unit, &name);
        use vocab::unit as u;
        assert_eq!(v(Value::Num(12_340.4), u::FT), "12,340 ft");
        assert_eq!(v(Value::Num(6.4), u::G), "6.4 G");
        assert_eq!(v(Value::Num(0.84), u::RATIO), "x0.84");
        assert_eq!(v(Value::Num(35.), u::PERCENT), "35%");
        assert_eq!(v(Value::Num(6.14), u::NM), "6.1 nm");
        assert_eq!(v(Value::Num(-0.0001), u::DEG), "0 deg");
        assert_eq!(v(Value::Num(0.625), ""), "0.625");
        assert_eq!(v(Value::Int(18_400), ""), "18,400");
        assert_eq!(v(Value::Num(f64::NAN), u::KT), "NaN kt");
        assert_eq!(v(Value::Bool(true), ""), "yes");
        assert_eq!(v(Value::Id(3), ""), "A3");
        assert_eq!(v(Value::Ids(vec![1, 2]), ""), "A1, A2");
        assert_eq!(v(Value::Text("Pursuit".into()), ""), "Pursuit");
        assert_eq!(v(Value::None, u::FT), "");
        assert_eq!(ascii("a\u{2014}b \u{b0} é"), "a-b o ?");
    }

    #[test]
    fn tree_lines_indent_label_values_and_wrap_notes() {
        let font = font();
        let nodes = vec![
            Node::new(0, "Target", Value::Id(0))
                .with_note("priority Assigned, score 18,400 (next: Friendly 1-2, 26,100)"),
            Node::new(1, "Range", 6.1).with_unit(vocab::unit::NM),
            Node::new(0, "Effects applied this tick", Value::None),
            Node::new(1, "A label long enough to push its value under it", "x0.5"),
        ];
        let name = |id: u32| {
            if id == 0 {
                "You".into()
            } else {
                format!("{id}")
            }
        };
        let lines = tree_lines(&nodes, &font, 240, &name);
        assert_eq!(lines[0].text, "Target");
        assert_eq!(lines[0].value, Some((VALUE_X, "You".into(), WHITE)));
        // The note wraps beneath, indented one level.
        assert!(lines[1].text.starts_with("because priority"));
        assert_eq!(lines[1].x, INDENT);
        let notes = lines.iter().filter(|l| l.color == NOTE).count();
        assert!(notes >= 2, "{lines:?}");
        for line in &lines {
            assert!(line.x + text_width(&font, &line.text) <= 240, "{line:?}");
            if let Some((x, value, _)) = &line.value {
                assert!(x + text_width(&font, value) <= 240, "{line:?}");
            }
        }
        let range = lines.iter().find(|l| l.text == "Range").unwrap();
        assert_eq!(range.x, INDENT);
        assert_eq!(range.value.as_ref().unwrap().1, "6.1 nm");
        let header = lines
            .iter()
            .find(|l| l.text.starts_with("Effects"))
            .unwrap();
        assert!(header.value.is_none());
        // A label too long for a value beside it puts the value beneath.
        let at = lines
            .iter()
            .position(|l| l.text.starts_with("A label"))
            .unwrap();
        assert!(lines[at].value.is_none());
        let value = lines[at..].iter().find_map(|l| l.value.clone()).unwrap();
        assert_eq!(value.1, "x0.5");
        // A label too long for the panel wraps rather than being cut.
        let long = [Node::new(
            1,
            "Attack on Enemy 1-1 by an unknown attacker, shot 0",
            Value::None,
        )];
        let lines = tree_lines(&long, &font, 120, &name);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| !l.text.ends_with("..")));
        assert_eq!((lines[0].x, lines[1].x), (INDENT, 2 * INDENT));
        let joined: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(joined.join(" "), long[0].label);
    }

    #[test]
    fn wrapping_breaks_at_spaces_and_inside_long_words() {
        let font = font();
        assert_eq!(wrap(&font, "one two three", 40), ["one two", "three"]);
        assert_eq!(wrap(&font, "abcdefghijkl", 25), ["abcde", "fghij", "kl"]);
        assert_eq!(wrap(&font, "", 40), [""]);
    }

    #[test]
    fn tree_panels_scroll_and_clamp() {
        let font = font();
        let mut p = Panels::default();
        p.open(Kind::Thought, 1).unwrap();
        let mut data = Fake {
            now: 100,
            trees: vec![(90, thought(1, 200))],
            events: Vec::new(),
        };
        let mut pixels = vec![0; 640 * 480 * 4];
        let drawn = p.draw(&mut pixels, &font, REPLAY, &mut data);
        assert_eq!(drawn, [p.rects(REPLAY).sides[0].unwrap()]);
        let panel = p.slot(Side::Left).unwrap();
        assert_eq!(panel.lines, 200);
        let visible = panel.visible;
        assert!(visible > 20 && visible < 200);
        let inside = Some((100., 200.));
        assert!(p.wheel(REPLAY, inside, -2, &data));
        assert_eq!(p.slot(Side::Left).unwrap().scroll, 6);
        assert!(p.wheel(REPLAY, inside, -1_000, &data));
        assert_eq!(p.slot(Side::Left).unwrap().scroll, 200 - visible);
        assert!(p.wheel(REPLAY, inside, 1, &data));
        assert_eq!(p.slot(Side::Left).unwrap().scroll, 200 - visible - 3);
        // A shorter tree clamps the scroll when drawn.
        data.trees.push((100, thought(1, 10)));
        p.draw(&mut pixels, &font, REPLAY, &mut data);
        assert_eq!(p.slot(Side::Left).unwrap().scroll, 0);
        // The wheel off the panels does nothing.
        assert!(!p.wheel(REPLAY, Some((320., 200.)), 1, &data));
    }

    #[test]
    fn drawing_fills_only_the_panels() {
        let font = font();
        let mut p = Panels::default();
        p.open(Kind::Thought, 1).unwrap();
        p.open(Kind::Telemetry, 0).unwrap();
        p.open_comms(None);
        let mut data = Fake {
            now: 500,
            trees: vec![(400, thought(1, 5))],
            events: vec![timed(
                300,
                Event::new(vocab::kind::COMMS_RADIO)
                    .with_subject(1)
                    .with("speaker", "Enemy 1-1")
                    .with("heard", true)
                    .with("outcome", "delivered")
                    .with_text("Fox two"),
            )],
        };
        let mut pixels = vec![0; 640 * 480 * 4];
        let drawn = p.draw(&mut pixels, &font, REPLAY, &mut data);
        assert_eq!(drawn.len(), 3);
        for y in 0..480 {
            for x in 0..640 {
                let covered = pixels[(y * 640 + x) * 4 + 3] != 0;
                let inside = drawn.iter().any(|r| inside((x as f64, y as f64), *r));
                assert_eq!(covered, inside, "{x} {y}");
            }
        }
    }

    fn radio(tick: u64, speaker: u32, text: &str) -> TimedEvent {
        timed(
            tick,
            Event::new(vocab::kind::COMMS_RADIO)
                .with_subject(speaker)
                .with_text(text),
        )
    }

    fn release(tick: u64) -> TimedEvent {
        timed(
            tick,
            Event::new(vocab::kind::AUDIO_RELEASE)
                .with_subject(0)
                .with("sound", "&GUN.5K"),
        )
    }

    #[test]
    fn comms_filters_by_channel_and_aircraft() {
        let events = vec![
            radio(10, 1, "Contact"),
            timed(
                20,
                Event::new(vocab::kind::COMMS_ORDER)
                    .with_subject(0)
                    .with("recipients", vec![2u32, 3])
                    .with("order", "engage my target"),
            ),
            timed(
                30,
                Event::new(vocab::kind::COMMS_TOWER).with_text("Cleared"),
            ),
            timed(40, Event::new(vocab::kind::COMMS_HUD).with_text("Radar on")),
            timed(
                50,
                Event::new(vocab::kind::AUDIO_TONE)
                    .with_subject(0)
                    .with("tone", "lock")
                    .with("on", true),
            ),
            timed(60, Event::new(vocab::kind::WEAPON_LAUNCH).with_subject(1)),
        ];
        let mut filter = CommsPanel::default();
        let all = rows_before(&events, events.len(), &filter, 10);
        assert_eq!(
            all.iter().map(Row::first).collect::<Vec<_>>(),
            [0, 1, 2, 3, 4]
        );
        filter.shown[Channel::Tones.index()] = false;
        filter.shown[Channel::Crew.index()] = false;
        assert_eq!(rows_before(&events, events.len(), &filter, 10).len(), 3);
        // By aircraft: the sender, the object and every recipient.
        filter = CommsPanel {
            aircraft: Some(3),
            ..Default::default()
        };
        let rows = rows_before(&events, events.len(), &filter, 10);
        assert_eq!(rows.iter().map(Row::first).collect::<Vec<_>>(), [1]);
        filter.aircraft = Some(0);
        assert_eq!(rows_before(&events, events.len(), &filter, 10).len(), 2);
        // Only the newest rows before the end, and the rows since an entry.
        let filter = CommsPanel::default();
        let rows = rows_before(&events, 3, &filter, 2);
        assert_eq!(rows.iter().map(Row::first).collect::<Vec<_>>(), [1, 2]);
        let rows = rows_since(&events, 1, 4, &filter);
        assert_eq!(rows.iter().map(Row::first).collect::<Vec<_>>(), [1, 2, 3]);
    }

    /// A radio call's journal entry: `outcome` for call `message`.
    fn call(tick: u64, message: i64, outcome: &str) -> TimedEvent {
        let mut event = Event::new(vocab::kind::COMMS_RADIO)
            .with_subject(2)
            .with("speaker", "Red two")
            .with("message", message)
            .with("outcome", outcome)
            .with("trigger", "infrared missile release at aircraft 3")
            .with_text("Fox two");
        event = match outcome {
            "queued" => event.with("due_s", 0.4),
            "delivered" => event.with("heard", true).with("wait_s", 0.4),
            _ => event.with("heard", false).with("reason", "radio silence"),
        };
        timed(tick, event)
    }

    #[test]
    fn a_line_shows_once_with_its_latest_outcome_and_earlier_states() {
        let name = |id: u32| format!("A{id}");
        let events = vec![
            call(60, 7, "queued"),
            radio(62, 1, "Contact"),
            call(108, 7, "delivered"),
            call(200, 8, "queued"),
            call(210, 8, "dropped"),
        ];
        let filter = CommsPanel::default();
        let rows = rows_before(&events, events.len(), &filter, 10);
        let entries: Vec<Vec<usize>> = rows.iter().map(|r| r.entries.clone()).collect();
        assert_eq!(entries, [vec![1], vec![0, 2], vec![3, 4]]);
        let delivered = entry(&events, &rows[1], &name);
        assert_eq!(delivered.time, "00:00.9");
        assert_eq!(delivered.main, "Red two: Fox two");
        assert_eq!(
            delivered.outcome,
            Some(("delivered, heard by you".into(), Mood::Good))
        );
        assert_eq!(
            delivered.details,
            "trigger: infrared missile release at aircraft 3; waited 0.4 s; queued at 00:00.5, due in 0.4 s"
        );
        let dropped = entry(&events, &rows[2], &name);
        assert_eq!(
            dropped.outcome,
            Some(("dropped, not heard".into(), Mood::Bad))
        );
        assert!(dropped.details.starts_with("why: radio silence;"));
        assert!(dropped.unheard);
        // Before its delivery the line reads as queued, where it was queued.
        let rows = rows_before(&events, 2, &filter, 10);
        assert_eq!(
            rows.iter().map(|r| r.entries.clone()).collect::<Vec<_>>(),
            [vec![0], vec![1]]
        );
        let queued = entry(&events, &rows[0], &name);
        assert_eq!(queued.outcome, Some(("queued".into(), Mood::Pending)));
        assert!(queued.details.ends_with("due in 0.4 s"));
    }

    #[test]
    fn other_fields_read_with_their_units() {
        assert_eq!(extra_text("kept_s", "2"), "kept 2 s");
        assert_eq!(
            extra_text("launch_range_ft", "12000"),
            "launch range 12000 ft"
        );
        assert_eq!(extra_text("reply", "Engage"), "reply Engage");
        assert_eq!(extra_text("expires_s", "15"), "expires 15 s");
    }

    #[test]
    fn an_exchange_shows_each_recipients_answer() {
        let name = |id: u32| format!("Enemy 1-{id}");
        let report = timed(
            100,
            Event::new(vocab::kind::COMMS_REPORT)
                .with_subject(1)
                .with("message", 1i64 << 32)
                .with("recipients", vec![2u32, 3])
                .with("order", "attack report")
                .with("about", Value::Id(9))
                .with("outcome", "queued")
                .with("trigger", "an attack it saw")
                .with_text("attack on Enemy 1-1 by Enemy 1-9"),
        );
        let answer = |tick, id: u32, outcome: &str, reason: Option<&str>| {
            let mut event = Event::new(vocab::kind::COMMS_DELIVERY)
                .with_subject(id)
                .with_object(1)
                .with("message", 1i64 << 32)
                .with("outcome", outcome)
                .with_text("attack on Enemy 1-1 by Enemy 1-9");
            if let Some(reason) = reason {
                event = event.with("reason", reason);
            }
            timed(tick, event)
        };
        let events = vec![
            report,
            radio(101, 4, "Tally"),
            answer(101, 2, "delivered", None),
            answer(101, 3, "ignored", Some("holding formation since a recall")),
        ];
        let filter = CommsPanel::default();
        let rows = rows_before(&events, events.len(), &filter, 10);
        assert_eq!(rows.len(), 2);
        let e = entry(&events, &rows[1], &name);
        assert_eq!(e.kind, "REPORT");
        assert_eq!(
            e.main,
            "Enemy 1-1 -> Enemy 1-2, Enemy 1-3: attack on Enemy 1-1 by Enemy 1-9"
        );
        assert_eq!(
            e.outcome,
            Some(("delivered 1, ignored 1".into(), Mood::Pending))
        );
        assert_eq!(
            e.answers,
            [
                ("Enemy 1-2 delivered".to_owned(), Mood::Good),
                (
                    "Enemy 1-3 ignored: holding formation since a recall".to_owned(),
                    Mood::Bad
                ),
            ]
        );
        assert!(e.details.contains("trigger: an attack it saw"));
        // The aircraft filter keeps the whole exchange for any aircraft in
        // it: here a recipient named only by its answer.
        let only = |id| CommsPanel {
            aircraft: Some(id),
            ..Default::default()
        };
        let rows = rows_before(&events, events.len(), &only(3), 10);
        assert_eq!(
            rows.iter().map(|r| r.entries.clone()).collect::<Vec<_>>(),
            [vec![0, 2, 3]]
        );
        assert_eq!(rows_before(&events, events.len(), &only(9), 10).len(), 1);
        assert!(rows_before(&events, events.len(), &only(7), 10).is_empty());
        // Scrolling forward counts the exchange once.
        let rows = rows_since(&events, 1, events.len(), &filter);
        assert_eq!(rows.iter().map(Row::last).collect::<Vec<_>>(), [1, 3]);
    }

    #[test]
    fn repeated_sounds_merge_into_one_row() {
        let mut events: Vec<TimedEvent> = (0..30).map(|i| release(100 + i * 5)).collect();
        events.push(radio(260, 1, "Guns"));
        events.extend((0..3).map(|i| release(400 + i * 5)));
        let filter = CommsPanel::default();
        let rows = rows_before(&events, events.len(), &filter, 10);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            (rows[0].first(), rows[0].last(), rows[0].entries.len()),
            (0, 29, 30)
        );
        assert_eq!(rows[2].entries.len(), 3);
        let forward = rows_since(&events, 0, events.len(), &filter);
        assert_eq!(forward, rows);
        let name = |id: u32| format!("A{id}");
        let burst = entry(&events, &rows[0], &name);
        assert_eq!(burst.kind, "RELEASE");
        assert_eq!(burst.main, "A0: &GUN.5K x30 over 1.2 s");
        // A row sits, and reads, at its newest entry.
        assert_eq!(burst.time, "00:02.0");
    }

    #[test]
    fn comms_entries_read_as_who_to_whom_what_outcome_and_why() {
        let name = |id: u32| match id {
            0 => "You".to_owned(),
            id => format!("Friendly 1-{id}"),
        };
        let order = timed(
            144,
            Event::new(vocab::kind::COMMS_ORDER)
                .with_subject(0)
                .with("recipients", vec![2u32, 3])
                .with("order", "engage my target")
                .with("outcome", "rejected")
                .with("reason", "its sensors cannot see the target")
                .with("rolls", "none"),
        );
        let e = entry(
            std::slice::from_ref(&order),
            &Row { entries: vec![0] },
            &name,
        );
        assert_eq!(e.kind, "ORDER");
        assert_eq!(
            e.main,
            "You -> Friendly 1-2, Friendly 1-3: engage my target"
        );
        assert_eq!(e.outcome, Some(("rejected".into(), Mood::Bad)));
        assert_eq!(
            e.details,
            "why: its sensors cannot see the target; rolls: none"
        );
        assert_eq!(e.time, "00:01.2");
        let call = timed(
            10,
            Event::new(vocab::kind::COMMS_RADIO)
                .with("speaker", "Red two")
                .with("heard", false)
                .with("outcome", "delivered")
                .with("trigger", "missile release at aircraft 3")
                .with("wait_s", 0.4)
                .with_text("Fox two"),
        );
        let e = entry(
            std::slice::from_ref(&call),
            &Row { entries: vec![0] },
            &name,
        );
        assert_eq!(e.main, "Red two: Fox two");
        // Said on a radio the player does not hear: nothing went wrong.
        assert_eq!(
            e.outcome,
            Some(("delivered, not heard".into(), Mood::Quiet))
        );
        assert!(e.unheard);
        assert_eq!(
            e.details,
            "trigger: missile release at aircraft 3; waited 0.4 s"
        );
        let tone = timed(
            1,
            Event::new(vocab::kind::AUDIO_TONE)
                .with_subject(0)
                .with("tone", "seeker lock")
                .with("on", true),
        );
        let e = entry(
            std::slice::from_ref(&tone),
            &Row { entries: vec![0] },
            &name,
        );
        assert_eq!(
            (e.kind.as_str(), e.main.as_str()),
            ("TONE", "You: seeker lock on")
        );
        let music = timed(
            1,
            Event::new(vocab::kind::AUDIO_MUSIC)
                .with("from", "cruise")
                .with("to", "air combat")
                .with("reason", "designated enemy inside 40,000 ft"),
        );
        let e = entry(
            std::slice::from_ref(&music),
            &Row { entries: vec![0] },
            &name,
        );
        assert_eq!(e.main, "cruise -> air combat");
        assert_eq!(e.details, "why: designated enemy inside 40,000 ft");
    }

    #[test]
    fn a_comms_row_colours_its_outcome_and_wraps_its_reasons() {
        let font = font();
        let e = Entry {
            time: "00:12.0".into(),
            kind: "RADIO".into(),
            channel: Some(Channel::Radio),
            main: "Enemy 2-2: Fox one".into(),
            outcome: Some(("suppressed, not heard".into(), Mood::Bad)),
            details: "why: same-shooter limit (8 s)".into(),
            answers: vec![("Enemy 2-3 rejected".into(), Mood::Bad)],
            unheard: true,
        };
        let lines = entry_lines(&font, &e, 600);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[2], [(112, "Enemy 2-3 rejected".to_owned(), BAD)]);
        let first: Vec<_> = lines[0]
            .iter()
            .map(|(x, t, c)| (*x, t.as_str(), *c))
            .collect();
        assert_eq!(first[0], (0, "00:12.0", MUTED));
        assert_eq!(first[1], (48, "RADIO", Channel::Radio.color()));
        assert_eq!(first[2], (104, "Enemy 2-2: Fox one", PALE));
        assert_eq!(first[3], (104 + 18 * 5 + 6, "[suppressed, not heard]", BAD));
        assert_eq!(
            lines[1],
            [(112, "why: same-shooter limit (8 s)".to_owned(), NOTE)]
        );
        // No room beside the words: the outcome takes a line of its own.
        let narrow = entry_lines(&font, &e, 104 + 150);
        assert!(
            narrow
                .iter()
                .any(|l| l.len() == 1 && l[0].1.starts_with('[') && l[0].2 == BAD)
        );
    }

    #[test]
    fn the_comms_panel_follows_the_playhead_until_scrolled_back() {
        let font = font();
        let events: Vec<TimedEvent> = (0..40)
            .map(|i| radio(100 + i * 10, 1, &format!("Call {i}")))
            .collect();
        let mut data = Fake {
            now: 250,
            trees: Vec::new(),
            events,
        };
        let mut p = Panels::default();
        p.open_comms(None);
        let mut pixels = vec![0; 640 * 480 * 4];
        p.draw(&mut pixels, &font, REPLAY, &mut data);
        assert_eq!(p.comms().unwrap().anchor, None);
        let at = Some((320., 400.));
        // Back three rows from the 16 said by now.
        assert!(p.wheel(REPLAY, at, 1, &data));
        assert_eq!(p.comms().unwrap().anchor, Some(13));
        // Playback moving on leaves the scrolled-back view where it was.
        data.now = 400;
        p.draw(&mut pixels, &font, REPLAY, &mut data);
        assert_eq!(p.comms().unwrap().anchor, Some(13));
        // Forward again: back to following.
        assert!(p.wheel(REPLAY, at, -1, &data));
        assert_eq!(p.comms().unwrap().anchor, Some(16));
        assert!(p.wheel(REPLAY, at, -10, &data));
        assert_eq!(p.comms().unwrap().anchor, None);
        // Playing backwards before the anchor starts following again.
        assert!(p.wheel(REPLAY, at, 1, &data));
        let anchor = p.comms().unwrap().anchor.unwrap();
        data.now = 100 + (anchor as u64 - 2) * 10;
        p.draw(&mut pixels, &font, REPLAY, &mut data);
        assert_eq!(p.comms().unwrap().anchor, None);
        // Live flight trimming its list keeps the anchor on the same rows.
        assert!(p.wheel(REPLAY, at, 1, &data));
        let anchor = p.comms().unwrap().anchor.unwrap();
        p.entries_removed(5);
        assert_eq!(p.comms().unwrap().anchor, Some(anchor - 5));
    }

    #[test]
    fn recorded_trees_are_read_a_chunk_at_a_time_in_either_direction() {
        let dir = crate::replay::tests::TempDir::new("panel-trees");
        let recording = crate::replay::fixture::recording(dir.path(), "trees");
        let mut trees = RecordedTrees::default();
        let channel = vocab::channel::AI_THOUGHT;
        // The fixture samples aircraft 1's thinking every 30 ticks.
        for tick in [
            crate::replay::fixture::FIRST,
            64,
            200,
            399,
            430,
            700,
            900,
            650,
            20,
        ] {
            let expected = recording.tree(1, channel, tick).unwrap();
            assert_eq!(trees.tree(&recording, 1, channel, tick), expected, "{tick}");
        }
        assert!(trees.chunks.len() <= TREE_CHUNKS);
        assert_eq!(trees.tree(&recording, 9, channel, 500), None);
    }
}
