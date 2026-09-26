//! The debug panels in live flight, behind the authored Pref row "Debug
//! panels?": the mission timer, the right-click menu and the same AI
//! thinking, Telemetry, Guidance and Comms panels the replay viewer shows,
//! fed from the flight as it happens instead of from a recording. A
//! right-click without dragging opens the menu; a right-drag stays mouse
//! look. Nothing here changes the flight: the menu only moves the camera and
//! opens panels. Opinionated addition requested by John on 2026-09-26; the
//! details are agent design decisions (2026-09-26).
use crate::controls_editor::{Rect, TITLE, text_width};
use crate::flight_canvas::FlightCanvas;
use crate::menu::Canvas;
use crate::render_snapshot::RenderSnapshot;
use crate::replay::clock;
use crate::replay::context_menu::{self, Action, Menu, Outcome, Pickable, RightClick, Target};
use crate::replay::overlay::Placement;
use crate::replay::panels::{self, Data, Kind, Panels};
use crate::replay::viewer::{Label, Request};
use crate::terrain::Camera;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tore_formats::font::Font;
use tore_replay::{AircraftInfo, Event, Side, TimedEvent, TreeSample};

/// Comms and audio entries kept for the live Comms panel.
pub const COMMS_KEPT: usize = 512;
/// Entries dropped at once when the list is full.
const COMMS_TRIM: usize = 128;
/// Display trees kept, the latest of each aircraft or missile and channel.
const TREES_KEPT: usize = 512;

/// Who is in the flight, as the menus and panels name them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Roster {
    /// Every aircraft by id, as a recording would register it.
    pub info: BTreeMap<u32, AircraftInfo>,
    /// The aircraft the AI flies.
    pub ai: BTreeSet<u32>,
    /// Each weapon in flight: its shooter and name.
    pub missiles: BTreeMap<u32, (u32, String)>,
}

impl Roster {
    fn name(&self, id: u32) -> String {
        match self.info.get(&id) {
            Some(a) if !a.label.is_empty() => a.label.clone(),
            Some(a) if !a.name.is_empty() => a.name.clone(),
            _ => format!("Aircraft {id}"),
        }
    }

    fn entry(&self, id: u32) -> context_menu::Aircraft {
        let info = self.info.get(&id);
        context_menu::Aircraft {
            id,
            label: self.name(id),
            name: info.map(|a| a.name.clone()).unwrap_or_default(),
            side: info.map_or(
                if id == 0 {
                    Side::Friendly
                } else {
                    Side::Unknown
                },
                |a| match a.side {
                    Side::Unknown if id == 0 => Side::Friendly,
                    side => side,
                },
            ),
            wing: info.map_or(0, |a| a.wing),
            member: info.map_or(0, |a| a.member),
            ai: self.ai.contains(&id),
        }
    }
}

/// Live flight as the debug panels read it this frame.
pub struct Flight<'a> {
    /// The picture this frame draws.
    pub frame: &'a RenderSnapshot,
    /// The player's aircraft, for its name and the interface font.
    pub airframe: &'a crate::aircraft::Airframe,
    /// A Quick Mission rather than free flight.
    pub mission: bool,
    pub wings: Option<&'a crate::ai_wings::AiWings>,
    pub combat: &'a crate::combat::Combat,
    /// What the camera is on.
    pub reference: crate::flight_views::Reference,
    /// The camera is in the player's cockpit.
    pub cockpit: bool,
    pub camera: &'a Camera,
}

/// What a menu item asks of the flight's camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// The chase view (F10) on the aircraft.
    Follow(u32),
    /// The front view from the aircraft: the cockpit for the player.
    Cockpit(u32),
}

/// What took the left button's press, so its release goes there too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Menu,
    Panels,
}

/// The live flight's panels, menu and what they read.
pub struct Live {
    pub panels: Panels,
    pub menu: Option<Menu>,
    /// Name labels over the aircraft.
    pub labels: bool,
    /// The flight is being recorded, which is where the trees and entries
    /// come from.
    pub recording: bool,
    pub roster: Roster,
    /// Recent comms and audio entries, oldest first.
    comms: Vec<TimedEvent>,
    /// The latest display tree of each aircraft or missile and channel,
    /// with the tick it was built.
    trees: HashMap<(u32, String), (u64, TreeSample)>,
    right: Option<(RightClick, [f64; 2])>,
    left: Option<Owner>,
    /// Where the pointer is in the panel layer.
    point: Option<(f64, f64)>,
    /// What the last frame drew that a right-click can pick.
    pickables: Vec<Pickable>,
    /// The aircraft the camera is on, which unpinned panels follow.
    selected: Option<u32>,
    /// Panels asked for on the command line, opened on the first frame.
    pub requests: Vec<Request>,
    layer: Vec<u8>,
}

impl Default for Live {
    fn default() -> Self {
        Self {
            panels: Panels::default(),
            menu: None,
            labels: false,
            recording: true,
            roster: Roster::default(),
            comms: Vec::new(),
            trees: HashMap::new(),
            right: None,
            left: None,
            point: None,
            pickables: Vec::new(),
            selected: None,
            requests: Vec::new(),
            layer: vec![0; panels::WIDTH as usize * panels::HEIGHT as usize * 4],
        }
    }
}

/// What live flight lends the panels: the latest tick.
struct LiveData<'a> {
    now: u64,
    recording: bool,
    roster: &'a Roster,
    comms: &'a [TimedEvent],
    trees: &'a HashMap<(u32, String), (u64, TreeSample)>,
}

impl Data for LiveData<'_> {
    fn now(&self) -> u64 {
        self.now
    }

    fn tree(&mut self, subject: u32, channel: &str) -> Option<(u64, TreeSample)> {
        self.trees.get(&(subject, channel.to_owned())).cloned()
    }

    fn name(&self, id: u32) -> String {
        self.roster.name(id)
    }

    fn title(&self, kind: Kind, subject: u32) -> String {
        match kind {
            Kind::Guidance => match self.roster.missiles.get(&subject) {
                Some((owner, weapon)) => format!("{weapon} from {}", self.name(*owner)),
                None => format!("missile {subject}"),
            },
            Kind::Thought | Kind::Telemetry => self.roster.entry(subject).title(),
        }
    }

    fn missing(&self, kind: Kind, subject: u32) -> String {
        let who = self.name(subject);
        let here = self.roster.info.contains_key(&subject);
        if !self.recording {
            return "Mission recording is off (TORE_RECORD_MISSIONS=0), and the panels show what it records.".into();
        }
        match kind {
            Kind::Thought if subject == 0 => {
                "You fly this aircraft, so there is no AI thinking to show.".into()
            }
            Kind::Thought if here && !self.roster.ai.contains(&subject) => {
                format!("{who} is not flown by the AI, so there is no AI thinking to show.")
            }
            Kind::Guidance if !self.roster.missiles.contains_key(&subject) => {
                "That missile is no longer flying.".into()
            }
            _ if !here && kind != Kind::Guidance => format!("{who} is no longer in the flight."),
            Kind::Thought => format!("No AI thinking for {who} yet."),
            Kind::Telemetry => format!("No telemetry for {who} yet."),
            Kind::Guidance => "No guidance for this missile yet.".into(),
        }
    }

    fn comms(&self) -> (&[TimedEvent], usize) {
        (self.comms, self.comms.len())
    }

    fn aircraft(&self) -> Vec<u32> {
        self.roster.info.keys().copied().collect()
    }
}

/// The mission clock as the timer shows it.
pub fn timer_text(tick: u64) -> String {
    format!(
        "{}   tick {}",
        clock::timestamp(tick as f64),
        clock::grouped(tick)
    )
}

impl Live {
    /// Closes everything: a new flight, or the Pref row switched off. The
    /// name labels switch and panels asked for on the command line stay.
    pub fn reset(&mut self) {
        *self = Self {
            labels: self.labels,
            requests: std::mem::take(&mut self.requests),
            ..Self::default()
        };
    }

    /// Nothing open or pending: the flight draws as it always did.
    pub fn idle(&self) -> bool {
        self.panels.is_empty() && self.menu.is_none() && self.right.is_none()
    }

    /// A comms or audio entry that happened at `tick`, for the Comms panel.
    /// The oldest go when the list is full.
    pub fn note(&mut self, tick: u64, event: Event) {
        if panels::Channel::of(&event.kind).is_none() {
            return;
        }
        if self.comms.len() >= COMMS_KEPT {
            self.comms.drain(..COMMS_TRIM);
            self.panels.entries_removed(COMMS_TRIM);
        }
        self.comms.push(TimedEvent { tick, event });
    }

    /// A display tree the recorder built at `tick`: the panels show the
    /// latest of each aircraft or missile and channel.
    pub fn sample(&mut self, tick: u64, tree: TreeSample) {
        let key = (tree.subject, tree.channel.clone());
        if self.trees.len() >= TREES_KEPT && !self.trees.contains_key(&key) {
            // Forget the oldest, most likely a missile long gone.
            if let Some(oldest) = self
                .trees
                .iter()
                .min_by_key(|(_, (tick, _))| *tick)
                .map(|(key, _)| key.clone())
            {
                self.trees.remove(&oldest);
            }
        }
        self.trees.insert(key, (tick, tree));
    }

    /// The comms and audio entries kept, oldest first.
    #[cfg(test)]
    pub fn comms(&self) -> &[TimedEvent] {
        &self.comms
    }

    /// The camera now sits on `selected` (the player is 0): unpinned panels
    /// move to it when it changes.
    pub fn follow(&mut self, selected: u32) {
        if self.selected != Some(selected) {
            if self.selected.is_some() {
                self.panels.follow(selected);
            }
            self.selected = Some(selected);
        }
    }

    /// The right button went down or up at `window` in window pixels, at
    /// `point` in the view's pixels. `allowed` says a click may open the
    /// menu. Returns where a click landed, in view pixels, for
    /// [`Live::open_menu`].
    pub fn right(
        &mut self,
        pressed: bool,
        window: [f64; 2],
        point: Option<[f64; 2]>,
        slop: f64,
        allowed: bool,
    ) -> Option<[f64; 2]> {
        if pressed {
            self.right = point
                .filter(|_| allowed)
                .map(|p| (RightClick::press(window, slop), p));
            return None;
        }
        let (click, at) = self.right.take()?;
        click.release(window).then_some(at)
    }

    /// The pointer moved to `point` in view pixels (`None` off the view)
    /// on a view of `size`; `window` is the same point in window pixels.
    pub fn pointer(&mut self, point: Option<[f64; 2]>, window: [f64; 2], size: [u32; 2]) {
        if let Some((click, _)) = &mut self.right {
            click.moved(window);
        }
        let at = point.map(|p| Placement::new(size).centered(p));
        self.point = at;
        let over_menu = match &mut self.menu {
            Some(menu) => {
                menu.pointer(at);
                at.is_some_and(|at| menu.contains(at))
            }
            None => false,
        };
        self.panels.pointer(panels::LIVE, at.filter(|_| !over_menu));
    }

    /// The pointer is on a panel or the menu.
    pub fn covers(&self) -> bool {
        self.point.is_some_and(|at| {
            self.menu.as_ref().is_some_and(|menu| menu.contains(at))
                || self.panels.hit(panels::LIVE, at).is_some()
        })
    }

    /// Lets go of any press, when the window loses focus or the pointer
    /// leaves it.
    pub fn release(&mut self) {
        self.right = None;
        self.left = None;
        self.point = None;
        self.panels.hover = None;
    }

    /// Opens the menu for a right-click at `at` (view pixels) on a view of
    /// `size`: a panel's aircraft or missile when on a panel, otherwise the
    /// aircraft or missile drawn nearest the pointer through `camera`,
    /// otherwise the list of aircraft.
    pub fn open_menu(&mut self, at: [f64; 2], size: [u32; 2], camera: &Camera, font: &Font) {
        let placement = Placement::new(size);
        let layer = placement.centered(at);
        let on_panel = match self.panels.hit(panels::LIVE, layer) {
            Some(panels::Hit::Body(side) | panels::Hit::Pin(side) | panels::Hit::Close(side)) => {
                self.panels.slot(side).map(|p| match p.kind {
                    Kind::Guidance => Target::Missile(p.subject),
                    Kind::Thought | Kind::Telemetry => Target::Aircraft(p.subject),
                })
            }
            _ => None,
        };
        let target = on_panel.unwrap_or_else(|| {
            context_menu::pick(
                at,
                size,
                camera,
                &self.pickables,
                context_menu::PICK_RADIUS * placement.scale(),
            )
        });
        self.menu = Some(self.build_menu(target, layer, font));
    }

    fn build_menu(&self, target: Target, at: (f64, f64), font: &Font) -> Menu {
        let options = context_menu::Options {
            live: true,
            labels: self.labels,
            trails: false,
        };
        let (title, items) = match target {
            Target::Aircraft(id) => {
                let entry = self.roster.entry(id);
                (entry.title(), context_menu::aircraft_items(&entry, options))
            }
            Target::Missile(id) => {
                let owner = self.roster.missiles.get(&id);
                let title = owner.map_or_else(
                    || format!("Missile {id}"),
                    |(owner, weapon)| format!("{weapon} from {}", self.roster.name(*owner)),
                );
                let owner = owner.map(|(owner, _)| self.roster.entry(*owner));
                (
                    title,
                    context_menu::missile_items(id, owner.as_ref(), options),
                )
            }
            Target::Nothing => {
                let entries: Vec<_> = self
                    .pickables
                    .iter()
                    .filter_map(|p| match p.target {
                        Target::Aircraft(id) => Some(self.roster.entry(id)),
                        _ => None,
                    })
                    .collect();
                (
                    "Jump to an aircraft".to_owned(),
                    context_menu::jump_items(&entries, options),
                )
            }
        };
        Menu::new(target, title, items, at, font)
    }

    /// The menu on `id` where `camera` shows it, or else in the middle of
    /// a view of `size`.
    pub fn menu_on(&mut self, id: u32, size: [u32; 2], camera: &Camera, font: &Font) {
        let at = self
            .pickables
            .iter()
            .find(|p| p.target == Target::Aircraft(id))
            .and_then(|p| camera.project(size, p.position))
            .unwrap_or([f64::from(size[0]) / 2., f64::from(size[1]) / 2.]);
        let layer = Placement::new(size).centered(at);
        self.menu = Some(self.build_menu(Target::Aircraft(id), layer, font));
    }

    /// Does what a menu item says; camera changes go back to the flight.
    fn perform(&mut self, action: Action) -> Option<View> {
        let open = |panels: &mut Panels, kind, id| {
            // Both sides pinned: the item does nothing, as the replay's
            // message explains there.
            let _ = panels.open(kind, id);
        };
        match action {
            Action::Follow(id) | Action::Jump(id) => return Some(View::Follow(id)),
            Action::Cockpit(id) => return Some(View::Cockpit(id)),
            Action::Thought(id) => open(&mut self.panels, Kind::Thought, id),
            Action::Telemetry(id) => open(&mut self.panels, Kind::Telemetry, id),
            Action::Guidance(id) => open(&mut self.panels, Kind::Guidance, id),
            Action::Comms(id) => self.panels.open_comms(Some(id)),
            Action::Labels => self.labels = !self.labels,
            // Live flight has no drone or trails.
            Action::Drone(_) | Action::DroneMissile(_) | Action::Trails => {}
        }
        None
    }

    /// A key while the menu is open. `None` when the menu does not use it,
    /// so it goes on to the flight; otherwise any camera change the menu
    /// asks for.
    pub fn key(&mut self, name: &str, shift: bool) -> Option<Option<View>> {
        let menu = self.menu.as_mut()?;
        match menu.key(name, shift) {
            Outcome::Ignored => None,
            Outcome::Handled => Some(None),
            Outcome::Close => {
                self.menu = None;
                Some(None)
            }
            Outcome::Chosen(action) => {
                self.menu = None;
                Some(self.perform(action))
            }
        }
    }

    /// The left button went down or up at `point` in view pixels on a view
    /// of `size`. `None` when neither the menu nor a panel took it, so it
    /// goes on to the instruments; otherwise any camera change asked for.
    pub fn left(
        &mut self,
        pressed: bool,
        point: Option<[f64; 2]>,
        size: [u32; 2],
        now: u64,
    ) -> Option<Option<View>> {
        let at = point.map(|p| Placement::new(size).centered(p));
        if pressed {
            self.left = if let Some(menu) = &mut self.menu {
                if !menu.down(at) {
                    self.menu = None;
                }
                Some(Owner::Menu)
            } else if self.panels.down(panels::LIVE, at) {
                Some(Owner::Panels)
            } else {
                None
            };
            return self.left.map(|_| None);
        }
        match self.left.take()? {
            Owner::Menu => {
                let chosen = self.menu.as_mut().map(|menu| menu.up(at));
                if let Some(Outcome::Chosen(action)) = chosen {
                    self.menu = None;
                    return Some(self.perform(action));
                }
                Some(None)
            }
            Owner::Panels => {
                let data = LiveData {
                    now,
                    recording: self.recording,
                    roster: &self.roster,
                    comms: &self.comms,
                    trees: &self.trees,
                };
                self.panels.up(panels::LIVE, at, &data);
                Some(None)
            }
        }
    }

    /// Mouse wheel notches: true when the menu or a panel under the pointer
    /// took them.
    pub fn wheel(&mut self, notches: i32, now: u64) -> bool {
        if let Some(menu) = &mut self.menu
            && self.point.is_some_and(|at| menu.contains(at))
        {
            menu.wheel(notches);
            return true;
        }
        let data = LiveData {
            now,
            recording: self.recording,
            roster: &self.roster,
            comms: &self.comms,
            trees: &self.trees,
        };
        self.panels.wheel(panels::LIVE, self.point, notches, &data)
    }

    /// What a right-click can pick in `picture`, with the name labels
    /// drawn: every aircraft, not ground objects, and every weapon but gun
    /// rounds.
    pub fn picture(&mut self, picture: &RenderSnapshot, labels: &[Label]) {
        let labels: Vec<(u32, [f64; 4])> = labels.iter().map(|l| (l.id, l.rect())).collect();
        self.pickables = context_menu::pickables(picture, &labels, |pose| {
            pose.id == 0 || pose.aircraft.is_some()
        });
    }

    /// Opens the panels asked for on the command line: AI thinking on the
    /// first aircraft the AI flies, telemetry on the player, guidance on the
    /// newest missile in flight, the Comms panel, and the menu on the first
    /// AI aircraft.
    pub fn open_requests(&mut self, size: [u32; 2], camera: &Camera, font: &Font) {
        let first_ai = self.roster.ai.iter().next().copied();
        for request in std::mem::take(&mut self.requests) {
            match request {
                Request::Thought => {
                    let _ = self.panels.open(Kind::Thought, first_ai.unwrap_or(0));
                }
                Request::Telemetry => {
                    let _ = self.panels.open(Kind::Telemetry, 0);
                }
                Request::Guidance => {
                    if let Some(id) = self.roster.missiles.keys().next_back() {
                        let _ = self.panels.open(Kind::Guidance, *id);
                    }
                }
                Request::Comms => self.panels.open_comms(None),
                Request::Menu => self.menu_on(first_ai.unwrap_or(0), size, camera, font),
            }
        }
    }

    /// One frame of live flight: learns who is in the flight and what a
    /// right-click can pick, follows the camera, opens any panels asked
    /// for, then draws. Reads the flight, never changes it.
    pub fn frame(&mut self, canvas: &mut FlightCanvas, flight: &Flight) {
        let frame = flight.frame;
        let roster = crate::replay::recorder::roster(
            frame,
            &flight.airframe.profile.name,
            flight.mission,
            flight.wings,
            flight.combat.models(),
        );
        self.roster = Roster {
            ai: roster
                .iter()
                .filter(|a| a.id != 0 && flight.wings.is_some_and(|w| w.slot(a.id).is_some()))
                .map(|a| a.id)
                .collect(),
            info: roster.into_iter().map(|a| (a.id, a)).collect(),
            missiles: frame
                .projectiles
                .iter()
                .filter(|p| !p.gun)
                .map(|p| {
                    let weapon = p.weapon.trim_end_matches(".JT").to_owned();
                    (p.id, (p.owner, weapon))
                })
                .collect(),
        };
        // Unpinned panels follow the aircraft the camera is on.
        use crate::flight_views::Reference;
        let selected = match flight.reference {
            Reference::Player => Some(0),
            Reference::Aircraft(id) => Some(id),
            Reference::Target => flight.combat.state.display_target().map(|t| t.id),
            Reference::Missile => None,
        }
        .filter(|id| self.roster.info.contains_key(id));
        if let Some(id) = selected {
            self.follow(id);
        }
        let size = canvas.size;
        let font = &flight.airframe.font;
        let labels = if self.labels {
            let roster = &self.roster;
            crate::replay::viewer::name_labels(
                std::iter::once(&frame.player)
                    .filter(|_| !flight.cockpit)
                    .chain(frame.targets.iter().filter(|p| p.aircraft.is_some())),
                flight.camera,
                size,
                font,
                selected,
                &|id| {
                    let entry = roster.entry(id);
                    (entry.label, entry.side)
                },
            )
        } else {
            Vec::new()
        };
        self.picture(frame, &labels);
        if !self.requests.is_empty() {
            self.open_requests(size, flight.camera, font);
        }
        self.draw(canvas, font, flight.combat.state.tick(), &labels);
    }

    /// Draws the timer, the panels and the menu over the flight view, and
    /// the name labels when they are on.
    pub fn draw(&mut self, canvas: &mut FlightCanvas, font: &Font, now: u64, labels: &[Label]) {
        let scale = Placement::new(canvas.size).scale();
        for label in labels {
            canvas.text(font, &label.text, label.at, scale, label.color);
        }
        self.layer.fill(0);
        let timer = timer_text(now);
        let width = text_width(font, &timer) + 12;
        let timer_rect: Rect = (
            (panels::WIDTH - width) / 2,
            6,
            width,
            font.height as i32 + 8,
        );
        Canvas(&mut self.layer).rect(timer_rect, [0, 0, 0, 170]);
        crate::controls_editor::Editor::text(
            &mut self.layer,
            font,
            timer_rect,
            TITLE,
            &timer,
            (timer_rect.0 + 6, timer_rect.1 + 4),
        );
        let mut data = LiveData {
            now,
            recording: self.recording,
            roster: &self.roster,
            comms: &self.comms,
            trees: &self.trees,
        };
        let mut rects = self
            .panels
            .draw(&mut self.layer, font, panels::LIVE, &mut data);
        rects.push(timer_rect);
        if let Some(menu) = &self.menu {
            menu.draw(&mut self.layer, font);
            rects.push(menu.rect());
        }
        canvas.centered_rects(&self.layer, &rects);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::panels::tests::font;

    fn roster() -> Roster {
        let info = |id: u32, label: &str, side, wing: u16| AircraftInfo {
            id,
            label: label.into(),
            name: if id == 0 { "F/A-18D" } else { "MiG-29" }.into(),
            side,
            wing,
            member: id as u16,
            human: id == 0,
            ..Default::default()
        };
        Roster {
            info: [
                info(0, "You", Side::Friendly, 1),
                info(1, "Enemy 2-1", Side::Enemy, 2),
                info(2, "Enemy 2-2", Side::Enemy, 2),
            ]
            .into_iter()
            .map(|a| (a.id, a))
            .collect(),
            ai: [1, 2].into_iter().collect(),
            missiles: [(77, (1, "AA-10".to_owned()))].into_iter().collect(),
        }
    }

    #[test]
    fn a_right_click_opens_the_menu_and_a_right_drag_does_not() {
        let mut live = Live::default();
        // A click: pressed and released within the slop.
        assert_eq!(
            live.right(true, [100., 100.], Some([50., 50.]), 4., true),
            None
        );
        live.pointer(Some([51., 50.]), [101., 100.], [1280, 960]);
        assert_eq!(
            live.right(false, [101., 101.], Some([51., 51.]), 4., true),
            Some([50., 50.])
        );
        // A drag: mouse look carries on and no menu opens.
        live.right(true, [100., 100.], Some([50., 50.]), 4., true);
        live.pointer(Some([80., 50.]), [130., 100.], [1280, 960]);
        assert_eq!(
            live.right(false, [100., 100.], Some([50., 50.]), 4., true),
            None
        );
        // Over nothing, the pointer covers no panel.
        assert!(!live.covers());
        live.panels.open(Kind::Telemetry, 0).unwrap();
        live.pointer(Some([200., 400.]), [200., 400.], [1280, 960]);
        assert!(live.covers());
        live.release();
        assert!(!live.covers());
        // Not allowed (the right button is bound elsewhere): never a menu.
        live.right(true, [100., 100.], Some([50., 50.]), 4., false);
        assert_eq!(
            live.right(false, [100., 100.], Some([50., 50.]), 4., false),
            None
        );
        // A release with no press is nothing.
        assert_eq!(
            live.right(false, [100., 100.], Some([50., 50.]), 4., true),
            None
        );
    }

    #[test]
    fn the_live_menu_offers_live_items_and_moves_the_camera() {
        let font = font();
        let mut live = Live {
            roster: roster(),
            ..Default::default()
        };
        let picture = RenderSnapshot::default();
        live.picture(&picture, &[]);
        let camera = Camera::new();
        live.menu_on(1, [1280, 960], &camera, &font);
        let menu = live.menu.as_ref().unwrap();
        assert_eq!(menu.title, "Enemy 2-1  MiG-29");
        let labels: Vec<_> = menu.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Follow (chase view)",
                "Cockpit view",
                "AI thinking",
                "Telemetry",
                "Comms for this aircraft",
                "Name labels",
            ]
        );
        // Keys go to the menu while it is open; others go on to the flight.
        assert_eq!(live.key("g", false), None);
        assert_eq!(live.key("ArrowDown", false), Some(None));
        assert_eq!(live.key("Enter", false), Some(Some(View::Cockpit(1))));
        assert!(live.menu.is_none());
        assert_eq!(live.key("Enter", false), None);
        // AI thinking opens a panel; the camera is left alone.
        live.menu_on(1, [1280, 960], &camera, &font);
        for _ in 0..2 {
            live.key("ArrowDown", false);
        }
        assert_eq!(live.key("Space", false), Some(None));
        assert_eq!(live.panels.find(Kind::Thought, 1), Some(panels::Side::Left));
        // The player's menu has no AI thinking.
        live.menu_on(0, [1280, 960], &camera, &font);
        assert!(
            live.menu
                .as_ref()
                .unwrap()
                .items
                .iter()
                .all(|i| i.action != Some(Action::Thought(0)))
        );
        assert_eq!(live.key("Escape", false), Some(None));
        assert!(live.menu.is_none());
    }

    #[test]
    fn panels_follow_the_camera_and_reset_with_the_flight() {
        let mut live = Live {
            roster: roster(),
            ..Default::default()
        };
        live.follow(0);
        live.panels.open(Kind::Thought, 1).unwrap();
        // The first frame only notes where the camera is.
        assert_eq!(live.panels.slot(panels::Side::Left).unwrap().subject, 1);
        live.follow(0);
        assert_eq!(live.panels.slot(panels::Side::Left).unwrap().subject, 1);
        live.follow(2);
        assert_eq!(live.panels.slot(panels::Side::Left).unwrap().subject, 2);
        live.labels = true;
        live.requests = vec![Request::Comms];
        live.reset();
        assert!(live.idle() && live.labels);
        assert_eq!(live.requests, [Request::Comms]);
    }

    #[test]
    fn the_live_comms_list_keeps_the_newest_entries() {
        let mut live = Live::default();
        live.note(1, Event::new(tore_replay::vocab::kind::WEAPON_LAUNCH));
        assert!(live.comms().is_empty());
        for tick in 0..(COMMS_KEPT as u64 + 10) {
            live.note(
                tick,
                Event::new(tore_replay::vocab::kind::COMMS_RADIO).with_text("Fox two"),
            );
        }
        assert!(live.comms().len() <= COMMS_KEPT);
        assert_eq!(live.comms().last().unwrap().tick, COMMS_KEPT as u64 + 9);
        assert!(live.comms().windows(2).all(|w| w[0].tick < w[1].tick));
    }

    #[test]
    fn live_data_names_and_explains() {
        let roster = roster();
        let mut live = Live::default();
        let thought = |subject, activity: &str| TreeSample {
            subject,
            channel: tore_replay::vocab::channel::AI_THOUGHT.into(),
            nodes: vec![tore_replay::Node::new(0, "Activity", activity)],
        };
        live.sample(1_188, thought(1, "FORMATION"));
        live.sample(1_200, thought(1, "ATTACKING"));
        let mut data = LiveData {
            now: 1_210,
            recording: true,
            roster: &roster,
            comms: &[],
            trees: &live.trees,
        };
        // The latest tree of each aircraft, with when it was built.
        assert_eq!(
            data.tree(1, "ai.thought"),
            Some((1_200, thought(1, "ATTACKING")))
        );
        assert!(data.tree(2, "ai.thought").is_none());
        assert_eq!(data.title(Kind::Guidance, 77), "AA-10 from Enemy 2-1");
        assert_eq!(data.title(Kind::Telemetry, 0), "You  F/A-18D");
        assert!(
            data.missing(Kind::Thought, 0)
                .starts_with("You fly this aircraft")
        );
        assert!(data.missing(Kind::Guidance, 5).contains("no longer flying"));
        assert!(
            data.missing(Kind::Telemetry, 9)
                .contains("no longer in the flight")
        );
        assert_eq!(timer_text(90_540), "12:34.5   tick 90,540");
    }

    #[test]
    fn drawing_puts_the_timer_at_the_top_and_leaves_the_rest_clear() {
        let font = font();
        let mut live = Live {
            roster: roster(),
            ..Default::default()
        };
        let mut canvas = FlightCanvas::default();
        canvas.blank([1280, 960]);
        live.draw(&mut canvas, &font, 600, &[]);
        let covered = |c: &FlightCanvas, x: usize, y: usize| c.pixels[(y * 1280 + x) * 4 + 3] != 0;
        assert!(covered(&canvas, 640, 20));
        assert!(!covered(&canvas, 100, 500) && !covered(&canvas, 640, 700));
        live.panels.open(Kind::Telemetry, 0).unwrap();
        canvas.blank([1280, 960]);
        live.draw(&mut canvas, &font, 600, &[]);
        assert!(covered(&canvas, 100, 500));
    }

    #[test]
    fn the_live_trees_keep_the_latest_of_each_and_forget_the_oldest() {
        let mut live = Live::default();
        let guidance = |subject| TreeSample {
            subject,
            channel: tore_replay::vocab::channel::WEAPON_GUIDANCE.into(),
            nodes: Vec::new(),
        };
        for id in 0..(TREES_KEPT as u32 + 3) {
            live.sample(u64::from(id), guidance(id));
        }
        assert_eq!(live.trees.len(), TREES_KEPT);
        assert!(!live.trees.contains_key(&(0, "weapon.guidance".to_owned())));
        assert!(
            live.trees
                .contains_key(&(TREES_KEPT as u32 + 2, "weapon.guidance".to_owned()))
        );
    }
}
