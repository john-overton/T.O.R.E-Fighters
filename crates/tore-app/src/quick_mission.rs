//! Retail option values with an explicit editable setup; simulation capabilities
//! are validated separately. Popup frame/focus feedback are authored presentation.
use crate::{
    menu::{Action, Canvas, HEIGHT, Sprite, WIDTH, text_width},
    terrain::{Camera, World},
};
use std::collections::BTreeMap;
use tore_formats::{aircraft::AircraftId, ui::creator::Options};
use tore_sim::ai::{
    AiError,
    engagement::GroupObjective,
    experience::EnemySkillOverride,
    launch::{Side, WingId, WingLaunch, WingSelection, legacy_pairs, resolve_wings},
};
pub mod layout;
pub use layout::{EnemyAim, FEET_PER_NM, MapBounds, SEPARATION_NM};
type Rect = (i32, i32, i32, i32);
const POPUP: Rect = (185, 100, 270, 370);
const ROWS: usize = 15;
const ROW_BASE: usize = 100;
const OK: usize = 1;
const CANCEL: usize = 2;
const POP_OK: usize = 70;
const POP_CANCEL: usize = 71;
const UP: usize = 72;
const DOWN: usize = 73;
const OBJECTIVE_BASE: usize = 80;
const OBJECTIVE_COUNT: usize = 6;
const SURVIVAL_BASE: usize = OBJECTIVE_BASE + OBJECTIVE_COUNT;
#[derive(Clone, Debug)]
pub struct Draft {
    pub values: [usize; 35],
}
impl Default for Draft {
    fn default() -> Self {
        let mut values = [0; 35];
        for (id, value) in [
            (4, 1),
            (5, 1),
            (15, 1),
            (16, 1),
            (17, 2),
            (19, 1),
            (21, 2),
            (22, 2),
        ] {
            values[id] = value;
        }
        Self { values }
    }
}
pub struct QuickMission {
    /// Results of the mission just flown, shown over the creator until OK.
    pub debrief: Option<crate::debrief::Debrief>,
    pub ordnance: Option<crate::ordnance::Ordnance>,
    pub hover: Option<usize>,
    pressed: Option<usize>,
    right_pressed: Option<usize>,
    /// The selector's PREV/NEXT page rocker.
    rocker: crate::rocker::Rocker,
    pub focus: usize,
    pub selection: usize,
    pub aircraft_selection: usize,
    pub aircraft_names: Vec<String>,
    pub aircraft_files: Vec<String>,
    pub draft: Draft,
    /// Friendly groups 1 through 3, then enemy groups 1 through 3.
    pub group_objectives: [GroupObjective; OBJECTIVE_COUNT],
    /// Whole-group survival requirement, independent of its combat orders.
    pub group_must_survive: [bool; OBJECTIVE_COUNT],
    /// Mission-wide setting used when a group inherits its objective.
    pub ai_mission: crate::ai_wings::Preset,
    options: Options,
    theater_codes: Vec<String>,
    theater_catalog: Vec<String>,
    start_modes: Vec<String>,
    airport_names: Vec<Vec<String>>,
    airport_objects: Vec<Vec<u32>>,
    selector: Option<usize>,
    cursor: usize,
    scroll: usize,
    controls: Vec<(usize, Rect)>,
    pub notice: Option<String>,
    pub help: bool,
    pub shift: bool,
}
/// Maps a creator condition onto the six recovered source weather choices.
/// The two lists are both recovered but the engine holds no table joining
/// them, so this match is by label: dawn, clear, cloudy, foggy, sunset and
/// night each name one choice. The editor omits the duplicate overcast label.
pub fn condition(value: usize) -> Option<usize> {
    Some(match value {
        0 => 3,
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 4,
        5 => 5,
        _ => return None,
    })
}

impl QuickMission {
    pub fn new(id: AircraftId, mut options: Options, data: &BTreeMap<String, Vec<u8>>) -> Self {
        // Keep the imported option inventory intact; only the editor list drops
        // the duplicate. Draft indices below refer to this six-row list.
        options.fields[15]
            .retain(|label| !label.trim_end_matches('.').eq_ignore_ascii_case("overcast"));
        for field in [5, 8, 11, 22, 25, 28] {
            options.fields[field].truncate(4);
            options.fields[field].push("Dummy (400 KTS)".into());
        }
        // The retail separation list ends at 50; 200 and 300 are host entries
        // in the retail label style (John, 2026-09-23). Every entry is read as
        // nautical miles, as the manual states.
        options.fields[17].truncate(layout::RETAIL_SEPARATIONS);
        for nm in &SEPARATION_NM[options.fields[17].len()..] {
            options.fields[17].push(format!("{nm} miles"));
        }
        // Metadata for the full retail catalog is also cached. Only expose the
        // exact aircraft identities whose flight profiles were imported.
        let mut catalog: Vec<(String, String)> = AircraftId::SELECTABLE
            .into_iter()
            .filter(|id| {
                data.get(id.pt())
                    .and_then(|bytes| tore_formats::aircraft::Aircraft::parse(bytes).ok())
                    .is_some_and(|aircraft| aircraft.id == id.source())
            })
            .map(|id| (id.selection_key().to_string(), id.label().to_string()))
            .collect();
        catalog.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        let (aircraft_files, aircraft_names): (Vec<_>, Vec<_>) = catalog.into_iter().unzip();
        let selected = aircraft_files
            .iter()
            .position(|n| n == id.selection_key())
            .unwrap_or(0);
        let mut draft = Draft::default();
        for i in [6, 9, 12, 23, 26, 29] {
            draft.values[i] = selected;
        }
        let mut theater_codes: Vec<String> = source_theaters()
            .iter()
            .map(|code| code.to_string())
            .collect();
        options.fields[13].truncate(16);
        let catalog = tore_formats::theater::map_catalog(data).unwrap_or_default();
        for (code, label) in catalog.iter().filter(|(code, _)| code.starts_with('~')) {
            theater_codes.push(code.clone());
            options.fields[13].push(label.clone());
        }
        let mut theater_catalog: Vec<String> = tore_formats::theater::THEATERS
            .iter()
            .map(|(code, _)| code.to_string())
            .collect();
        theater_catalog.extend(
            catalog
                .into_iter()
                .filter(|(code, _)| code.starts_with('~'))
                .map(|(code, _)| code),
        );
        let mut airport_names = Vec::new();
        let mut airport_objects = Vec::new();
        let mut definitions = BTreeMap::new();
        for code in &theater_codes {
            let name = format!("{code}.MM");
            let mut names = Vec::new();
            let mut ids = Vec::new();
            if let Some(bytes) = data.get(&name)
                && let Ok(layout) = tore_formats::mission::Layout::parse(&name, bytes)
            {
                for p in layout.placements {
                    let airport = *definitions.entry(p.object_type.clone()).or_insert_with(|| {
                        data.get(&p.object_type)
                            .and_then(|b| tore_formats::static_object::Definition::parse(b).ok())
                            .is_some_and(|d| {
                                d.main_shape.is_some()
                                    && d.callbacks.iter().any(|c| c == "_STRIPProc")
                            })
                    });
                    if airport {
                        names.push(p.name.unwrap_or(p.object_type));
                        ids.push(0x4000_0000 + p.key.ordinal);
                    }
                }
            }
            airport_names.push(names);
            airport_objects.push(ids);
        }
        Self {
            debrief: None,
            ordnance: None,
            start_modes: vec!["Airborne".into(), "Ground".into()],
            airport_names,
            airport_objects,
            hover: None,
            pressed: None,
            right_pressed: None,
            rocker: Default::default(),
            focus: 6,
            selection: 0,
            aircraft_selection: selected,
            aircraft_names,
            aircraft_files,
            draft,
            group_objectives: [GroupObjective::Inherit; OBJECTIVE_COUNT],
            group_must_survive: [false; OBJECTIVE_COUNT],
            ai_mission: crate::ai_wings::Preset::Free,
            options,
            theater_codes,
            theater_catalog,
            selector: None,
            cursor: 0,
            scroll: 0,
            controls: vec![],
            notice: None,
            help: false,
            shift: false,
        }
    }
    pub fn theater(&mut self, index: usize) {
        if self.selection != index {
            self.draft.values[34] = 0;
        }
        self.selection = index;
        if let Some(code) = self.theater_catalog.get(index) {
            self.draft.values[13] = self
                .theater_codes
                .iter()
                .position(|c| c == code)
                .unwrap_or(0);
        }
        self.nationalities();
    }
    fn nationalities(&mut self) {
        self.draft.values[3] = 0;
        self.draft.values[20] =
            [10, 33, 14, 57, 3, 41, 23, 10, 20, 37, 34, 24, 9, 2, 10, 2][self.base_theater_index()];
    }
    pub fn player(&self) -> Option<AircraftId> {
        self.aircraft_files
            .get(self.draft.values[6])
            .and_then(|n| AircraftId::parse(n).ok())
    }
    pub fn guns_only(&self) -> bool {
        self.draft.values[19] == 0
    }
    fn base_theater_index(&self) -> usize {
        let base = self
            .theater_codes
            .get(self.draft.values[13])
            .and_then(|code| tore_formats::theater::base_theater(code));
        source_theaters()
            .iter()
            .position(|code| Some(*code) == base)
            .unwrap_or(0)
    }
    pub fn theater_index(&self) -> usize {
        self.theater_codes
            .get(self.draft.values[13])
            .and_then(|code| self.theater_catalog.iter().position(|c| c == code))
            .unwrap_or(0)
    }
    fn values(&self, id: usize) -> &[String] {
        if matches!(id, 6 | 9 | 12 | 23 | 26 | 29) {
            &self.aircraft_names
        } else if id == 33 {
            &self.start_modes
        } else if id == 34 {
            &self.airport_names[self.draft.values[13]]
        } else if id == 30 {
            &self.options.targets[self.base_theater_index()]
        } else {
            &self.options.fields[id]
        }
    }
    fn value(&self, id: usize) -> String {
        if let Some(group) = id
            .checked_sub(OBJECTIVE_BASE)
            .filter(|group| *group < OBJECTIVE_COUNT)
        {
            return self.objective_label(group);
        }
        if let Some(group) = id
            .checked_sub(SURVIVAL_BASE)
            .filter(|group| *group < OBJECTIVE_COUNT)
        {
            return if self.group_must_survive[group] {
                "required"
            } else {
                "optional"
            }
            .into();
        }
        self.values(id)
            .get(self.draft.values[id])
            .cloned()
            .unwrap_or_else(|| "Unavailable".into())
    }
    fn objective_choices(group: usize) -> Vec<(String, GroupObjective)> {
        let side = if group < 3 {
            Side::Friendly
        } else {
            Side::Enemy
        };
        let wing = group % 3;
        let opposing = if side == Side::Friendly {
            Side::Enemy
        } else {
            Side::Friendly
        };
        let mut choices = vec![
            ("Use mission setting".into(), GroupObjective::Inherit),
            (
                "Free fire (any opposing group)".into(),
                GroupObjective::Free,
            ),
            ("Combat air patrol".into(), GroupObjective::Cap),
        ];
        for index in 0..3 {
            let target = WingId::new(opposing, index).expect("fixed Quick Mission wing");
            choices.push((
                format!(
                    "Primary target: {} group {}",
                    if opposing == Side::Enemy {
                        "enemy"
                    } else {
                        "friendly"
                    },
                    index + 1
                ),
                GroupObjective::Intercept(target),
            ));
        }
        for index in 0..3 {
            if index != wing {
                let target = WingId::new(side, index as u8).expect("fixed Quick Mission wing");
                choices.push((
                    format!(
                        "Protect {} group {}",
                        if side == Side::Friendly {
                            "friendly"
                        } else {
                            "enemy"
                        },
                        index + 1
                    ),
                    GroupObjective::Escort(target),
                ));
            }
        }
        choices.extend([
            ("Self-defense".into(), GroupObjective::SelfDefense),
            ("Weapons hold".into(), GroupObjective::Hold),
        ]);
        choices
    }
    fn objective_prefix(&self, group: usize) -> String {
        let primary = matches!(self.group_objectives[group], GroupObjective::Intercept(_));
        match (group, primary) {
            (0, true) => "Your primary target is ".into(),
            (0, false) => "Your flight will ".into(),
            (_, true) => format!("Wing {}'s primary target is ", group % 3 + 1),
            (_, false) => format!("Wing {} will ", group % 3 + 1),
        }
    }
    fn objective_label(&self, group: usize) -> String {
        match self.group_objectives[group] {
            GroupObjective::Inherit => match self.ai_mission {
                crate::ai_wings::Preset::Free => "use free fire (mission)".into(),
                crate::ai_wings::Preset::Cap => "patrol (mission)".into(),
                crate::ai_wings::Preset::SelfDefense => "defend itself (mission)".into(),
                crate::ai_wings::Preset::Hold => "hold fire (mission)".into(),
                _ => "follow mission orders".into(),
            },
            GroupObjective::Free => "use free fire".into(),
            GroupObjective::Cap => "patrol the area".into(),
            GroupObjective::Intercept(wing) => format!(
                "{} group {}",
                if wing.side == Side::Enemy {
                    "enemy"
                } else {
                    "friendly"
                },
                wing.display_number()
            ),
            GroupObjective::Escort(wing) => format!(
                "protect {} group {}",
                if wing.side == Side::Friendly {
                    "friendly"
                } else {
                    "enemy"
                },
                wing.display_number()
            ),
            GroupObjective::SelfDefense => "defend itself".into(),
            GroupObjective::Hold => "hold fire".into(),
        }
    }
    fn selector_values(&self, id: usize) -> Vec<String> {
        if let Some(group) = id
            .checked_sub(OBJECTIVE_BASE)
            .filter(|g| *g < OBJECTIVE_COUNT)
        {
            Self::objective_choices(group)
                .into_iter()
                .map(|(label, _)| label)
                .collect()
        } else {
            self.values(id).to_vec()
        }
    }
    /// The six wing rows as the AI launch payload: side, aircraft, wing skill
    /// and one entry per member (`docs/spec/ai-experience.md`, "Experience
    /// channels"). Wing counts live in fields 4, 7, 10 (friendly) and 21, 24,
    /// 27 (enemy); each wing's skill is the next field and its aircraft the one
    /// after that. Friendly wing 1 loses one slot to the player. A wing whose
    /// aircraft choice does not resolve to a supported import is skipped, which
    /// `unsupported` reports separately before a mission may start.
    pub fn wing_launches(
        &self,
        enemy_override: Option<EnemySkillOverride>,
    ) -> Result<Vec<WingLaunch>, AiError> {
        let mut selections = Vec::with_capacity(6);
        for (field, side, index) in [
            (4, Side::Friendly, 0),
            (7, Side::Friendly, 1),
            (10, Side::Friendly, 2),
            (21, Side::Enemy, 0),
            (24, Side::Enemy, 1),
            (27, Side::Enemy, 2),
        ] {
            let Some(aircraft) = self
                .aircraft_files
                .get(self.draft.values[field + 2])
                .and_then(|name| AircraftId::parse(name).ok())
            else {
                continue;
            };
            selections.push(WingSelection {
                wing: WingId::new(side, index)?,
                aircraft,
                count: self.draft.values[field].saturating_sub(usize::from(field == 4)),
                skill_level: self.draft.values[field + 1] as i32,
            });
        }
        resolve_wings(&selections, enemy_override)
    }
    /// The aircraft/count pairs the current mission spawner still takes. This
    /// is the launch payload with side, member and experience dropped; it stays
    /// for the existing spawner call and must not grow new callers.
    ///
    /// `fitted`: an out-of-range wing skill or count would make the payload an
    /// error, and this infallible signature has nowhere to put one, so it
    /// launches nothing. Rule: the decoded setup screen offers four
    /// experience levels plus Dummy and counts 0 through 5 per wing (`docs/formats/quick-mission.md`),
    /// so the menu cannot reach that state; the empty result is a visible
    /// failure rather than a silent clamp if it ever does.
    pub fn dummy_wings(&self) -> Vec<(AircraftId, usize)> {
        self.wing_launches(None)
            .map(|wings| legacy_pairs(&wings))
            .unwrap_or_default()
    }
    pub fn ground_start(&self) -> bool {
        self.draft.values[33] == 1
    }
    pub fn ground_runway(&self) -> Option<u32> {
        self.ground_start()
            .then(|| {
                self.airport_objects[self.draft.values[13]]
                    .get(self.draft.values[34])
                    .copied()
            })
            .flatten()
    }
    pub fn choose_ground_runway(&mut self, object: u32) -> Result<(), String> {
        let index = self.airport_objects[self.draft.values[13]]
            .iter()
            .position(|id| *id == object)
            .ok_or_else(|| "No imported runway matches the chosen airport".to_string())?;
        self.apply(33, 1);
        self.apply(34, index);
        Ok(())
    }
    /// The chosen enemy separation in nautical miles (manual p.19). An index
    /// outside the table falls back to the 5 mile default instead of failing.
    pub fn separation_nm(&self) -> f64 {
        SEPARATION_NM
            .get(self.draft.values[17])
            .copied()
            .unwrap_or(SEPARATION_NM[Draft::default().values[17]])
    }
    pub fn separation_feet(&self) -> f64 {
        self.separation_nm() * FEET_PER_NM
    }
    /// Aircraft in the player's wing, the player included.
    pub fn player_wing_size(&self) -> usize {
        self.draft.values[4].clamp(1, 5)
    }
    pub fn unsupported(&self) -> Option<String> {
        if self.player().is_none() {
            return Some(
                "This aircraft is available for setup only. Choose an imported aircraft from the player list to fly."
                    .into(),
            );
        }
        if self.ground_start() && self.ground_runway().is_none() {
            return Some(
                "No imported runways are available in this theater. Choose Airborne.".into(),
            );
        }
        let v = &self.draft.values;
        for field in [4, 7, 10, 21, 24, 27] {
            if v[field] > 0
                && self
                    .aircraft_files
                    .get(v[field + 2])
                    .and_then(|name| AircraftId::parse(name).ok())
                    .is_none()
            {
                return Some(
                    "Choose a supported imported aircraft for every populated wing.".into(),
                );
            }
        }
        if v[30] != 0 || v[31] != 0 || v[32] != 0 {
            return Some(
                "Ground targets and defenses are not available yet. Select none to fly.".into(),
            );
        }
        if condition(v[15]).is_none() {
            return Some("Choose one of the six available weather conditions.".into());
        }
        None
    }
    fn apply(&mut self, id: usize, value: usize) {
        self.draft.values[id] = value;
        self.draft.values[4] = self.draft.values[4].max(1);
        if self.draft.values[30] == 0 {
            self.draft.values[31] = 0;
            self.draft.values[32] = 0;
        }
        if id == 13 {
            self.draft.values[34] = 0;
            self.draft.values[30] = 0;
            self.nationalities();
        }
        self.aircraft_selection = self.draft.values[6];
        self.selection = self.theater_index();
        self.notice = None;
    }
    fn open(&mut self, id: usize) {
        self.selector = Some(id);
        self.cursor = if let Some(group) = id
            .checked_sub(OBJECTIVE_BASE)
            .filter(|group| *group < OBJECTIVE_COUNT)
        {
            Self::objective_choices(group)
                .iter()
                .position(|(_, objective)| *objective == self.group_objectives[group])
                .unwrap_or(0)
        } else {
            self.draft.values[id]
        };
        self.scroll = self.cursor / ROWS * ROWS;
        self.hover = None;
        self.pressed = None;
        self.right_pressed = None;
        self.help = false;
    }
    pub fn preview_selector(&mut self, name: &str) -> crate::AppResult<()> {
        match name {
            "normal" | "ordnance" => {}
            "ordnance-empty" | "ordnance-drag" | "ordnance-message" | "ordnance-message-long" => {
                if let Some(ordnance) = &mut self.ordnance {
                    ordnance.preview(name);
                }
            }
            "objectives" => {
                self.draft.values[7] = 2;
                self.draft.values[24] = 2;
                self.group_objectives[0] = GroupObjective::Intercept(WingId::new(Side::Enemy, 0)?);
                self.group_objectives[1] = GroupObjective::Free;
                self.group_must_survive[1] = true;
                self.group_objectives[2] = GroupObjective::Escort(WingId::new(Side::Friendly, 1)?);
                self.group_objectives[3] =
                    GroupObjective::Intercept(WingId::new(Side::Friendly, 0)?);
                self.group_objectives[4] = GroupObjective::Free;
                self.group_objectives[5] = GroupObjective::Escort(WingId::new(Side::Enemy, 1)?);
            }
            "aircraft" => self.open(6),
            "theaters" => self.open(13),
            // Debrief pages are prepared by the snapshot host.
            state if state.starts_with("debrief") => {}
            "help" => self.help = true,
            "ground-start" => self.apply(33, 1),
            "airports" => {
                self.apply(33, 1);
                self.open(34);
            }
            name if name.starts_with("objective-") => {
                let group = name[10..]
                    .parse::<usize>()
                    .ok()
                    .filter(|group| (1..=OBJECTIVE_COUNT).contains(group))
                    .ok_or("objective snapshots use objective-1 through objective-6")?;
                self.open(OBJECTIVE_BASE + group - 1);
            }
            _ => {
                let id=name.strip_prefix("field-").and_then(|v|v.parse::<usize>().ok()).filter(|v|(3..35).contains(v)).ok_or("snapshot states: normal, aircraft, theaters, help, field-3 through field-34")?;
                self.open(id);
            }
        }
        Ok(())
    }
    pub fn pointer(&mut self, p: Option<(f64, f64)>) {
        if let Some(d) = &mut self.debrief {
            d.pointer(p);
            return;
        }
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            o.pointer(p);
            return;
        }
        self.hover = p.and_then(|p| {
            self.controls
                .iter()
                .rev()
                .find(|(_, r)| inside(p, *r))
                .map(|(i, _)| *i)
        });
    }
    pub fn down(&mut self) -> Action {
        self.right_pressed = None;
        if let Some(d) = &mut self.debrief {
            return d.down();
        }
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            return o.down();
        }
        self.pressed = self.hover;
        // The selector's rocker pages on press and tilts while held.
        match self.hover.filter(|_| self.selector.is_some()) {
            Some(id @ (UP | DOWN)) => self.rock(id, true),
            _ => Action::None,
        }
    }
    fn rock(&mut self, id: usize, held: bool) -> Action {
        self.rocker
            .push(id == DOWN, held, std::time::Instant::now());
        self.activate(id);
        Action::RockerDown
    }
    pub fn up(&mut self) -> Action {
        if let Some(d) = &mut self.debrief {
            return d.up().unwrap_or_else(|| self.close_debrief());
        }
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            return o.up();
        }
        let p = self.pressed.take();
        if matches!(p, Some(UP | DOWN)) && self.rocker.held() {
            self.rocker.release(std::time::Instant::now());
            return Action::RockerUp;
        }
        if let Some(i) = p.filter(|p| Some(*p) == self.hover) {
            self.activate(i)
        } else {
            Action::None
        }
    }
    pub fn right(&mut self, down: bool) -> Action {
        if let Some(d) = &mut self.debrief {
            self.right_pressed = None;
            return d.right(down);
        }
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            self.right_pressed = None;
            return o.right(down);
        }
        if self.selector.is_some() || self.help {
            self.right_pressed = None;
            return Action::None;
        }
        if down {
            self.pressed = None;
            self.right_pressed = self.hover.filter(|id| {
                (3..=34).contains(id)
                    || (OBJECTIVE_BASE..SURVIVAL_BASE + OBJECTIVE_COUNT).contains(id)
            });
            return Action::None;
        }
        let Some(id) = self
            .right_pressed
            .take()
            .filter(|id| Some(*id) == self.hover)
        else {
            return Action::None;
        };
        if id == 34 && !self.ground_start() {
            return Action::None;
        }
        if (SURVIVAL_BASE..SURVIVAL_BASE + OBJECTIVE_COUNT).contains(&id) {
            self.group_must_survive[id - SURVIVAL_BASE] ^= true;
            self.focus = id;
            return Action::Click;
        }
        if let Some(group) = id
            .checked_sub(OBJECTIVE_BASE)
            .filter(|group| *group < OBJECTIVE_COUNT)
        {
            let choices = Self::objective_choices(group);
            let current = choices
                .iter()
                .position(|(_, objective)| *objective == self.group_objectives[group])
                .unwrap_or(0);
            let previous = (current + choices.len() - 1) % choices.len();
            self.group_objectives[group] = choices[previous].1;
            self.focus = id;
            return Action::Click;
        }
        let n = self.values(id).len();
        let minimum = usize::from(id == 4);
        if n <= minimum {
            return Action::None;
        }
        let current = self.draft.values[id];
        let previous = if current <= minimum || current >= n {
            n - 1
        } else {
            current - 1
        };
        self.focus = id;
        self.apply(id, previous);
        Action::Click
    }
    /// Leaves the debrief for the creator, keeping the mission just flown.
    fn close_debrief(&mut self) -> Action {
        self.debrief = None;
        self.hover = None;
        self.pressed = None;
        Action::Click
    }
    pub fn cancel(&mut self) {
        if let Some(d) = &mut self.debrief {
            d.cancel();
        }
        if let Some(o) = &mut self.ordnance {
            o.cancel();
        }
        self.pressed = None;
        self.right_pressed = None;
        if self.rocker.held() {
            self.rocker.release(std::time::Instant::now());
        }
        self.hover = None;
        self.selector = None;
        self.help = false;
    }
    fn activate(&mut self, id: usize) -> Action {
        if let Some(field) = self.selector {
            match id {
                POP_OK => {
                    let values = self.selector_values(field);
                    if values.is_empty() {
                        return Action::None;
                    }
                    if let Some(group) = field
                        .checked_sub(OBJECTIVE_BASE)
                        .filter(|group| *group < OBJECTIVE_COUNT)
                    {
                        self.group_objectives[group] =
                            Self::objective_choices(group)[self.cursor].1;
                        self.notice = None;
                    } else {
                        self.apply(field, self.cursor);
                    }
                    self.selector = None;
                    self.focus = field;
                }
                POP_CANCEL => {
                    self.selector = None;
                    self.focus = field;
                }
                UP => self.scroll = self.scroll.saturating_sub(ROWS),
                DOWN => {
                    self.scroll = (self.scroll + ROWS)
                        .min(self.selector_values(field).len().saturating_sub(1) / ROWS * ROWS)
                }
                ROW_BASE.. => {
                    let index = self.scroll + id - ROW_BASE;
                    if index < self.selector_values(field).len() {
                        self.cursor = index;
                    }
                }
                _ => return Action::None,
            }
            self.hover = None;
            return Action::Click;
        }
        match id {
            0 => {
                self.help = !self.help;
            }
            60 => {
                self.notice=Some("Aircraft era filters are not available yet. The list shows only supported imported aircraft.".into());
            }
            61 => return Action::Exit,
            OK => {
                if let Some(message) = self.unsupported() {
                    self.notice = Some(message);
                } else {
                    return Action::Mission;
                }
            }
            CANCEL => return Action::Back,
            3..=34 => {
                if id == 34 && !self.ground_start() {
                    return Action::None;
                }
                self.focus = id;
                if matches!(id, 6 | 9 | 12 | 13 | 23 | 26 | 29 | 34) ^ self.shift {
                    self.open(id);
                } else {
                    let n = self.values(id).len();
                    if n > 0 {
                        self.apply(id, (self.draft.values[id] + 1) % n);
                    }
                }
            }
            OBJECTIVE_BASE..=85 => {
                self.focus = id;
                self.open(id);
            }
            SURVIVAL_BASE..=91 => {
                self.focus = id;
                self.group_must_survive[id - SURVIVAL_BASE] ^= true;
            }
            _ => return Action::None,
        }
        Action::Click
    }
    pub fn key(&mut self, key: &str, shift: bool) -> Action {
        if let Some(d) = &mut self.debrief {
            return d.key(key).unwrap_or_else(|| self.close_debrief());
        }
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            return o.key(key);
        }
        self.shift = shift;
        if key == "Escape" {
            if self.selector.is_some() || self.help || self.notice.is_some() {
                self.cancel();
                self.notice = None;
                return Action::None;
            }
            return Action::Back;
        }
        if let Some(field) = self.selector {
            let n = self.selector_values(field).len();
            if n == 0 {
                return Action::None;
            }
            match key {
                "ArrowDown" => self.cursor = (self.cursor + 1) % n,
                "ArrowUp" => self.cursor = (self.cursor + n - 1) % n,
                "Home" => self.cursor = 0,
                "End" => self.cursor = n - 1,
                "PageDown" | "PageUp" => {
                    let down = key == "PageDown";
                    self.cursor = if down {
                        (self.cursor + ROWS).min(n - 1)
                    } else {
                        self.cursor.saturating_sub(ROWS)
                    };
                    self.scroll = self.cursor / ROWS * ROWS;
                    self.rocker.push(down, false, std::time::Instant::now());
                    return Action::RockerDown;
                }
                "Enter" => return self.activate(POP_OK),
                _ => {}
            }
            self.scroll = self.cursor / ROWS * ROWS;
            return Action::None;
        }
        match key {
            "Tab" | "ArrowDown" | "ArrowUp" => {
                let backwards = shift || key == "ArrowUp";
                let focus_order: Vec<_> = (1..=if self.ground_start() { 34 } else { 33 })
                    .chain(OBJECTIVE_BASE..SURVIVAL_BASE + OBJECTIVE_COUNT)
                    .collect();
                self.focus = if let Some(position) =
                    focus_order.iter().position(|field| *field == self.focus)
                {
                    let next = if backwards {
                        (position + focus_order.len() - 1) % focus_order.len()
                    } else {
                        (position + 1) % focus_order.len()
                    };
                    focus_order[next]
                } else if backwards {
                    *focus_order.last().unwrap()
                } else {
                    focus_order[0]
                };
                self.hover = Some(self.focus);
            }
            "Enter" | " " => return self.activate(self.focus),
            _ => {}
        }
        Action::None
    }
    pub fn render(
        &mut self,
        pixels: &mut [u8],
        sprites: &BTreeMap<String, Sprite>,
        _world: &World,
    ) -> bool {
        let animating = self.rocker.advance(std::time::Instant::now());
        if let Some(d) = &mut self.debrief {
            return d.render(pixels);
        }
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            return o.render(pixels);
        }
        pixels.copy_from_slice(&sprites["QUIKMIS3.PIC"].rgba);
        self.controls.clear();
        let mut c = Canvas(pixels);
        let font = &sprites["QUICKFONT"];
        c.centered_text(
            &sprites["MENUFONT.PIC"],
            "Aircraft",
            (
                103,
                38,
                text_width(&sprites["MENUFONT.PIC"], "Aircraft"),
                20,
            ),
        );
        self.controls
            .extend([(0, (84, 35, 18, 24)), (60, (103, 35, 95, 24))]);
        c.text(font, "FRIENDLY SITUATION", 116, 104, None);
        c.text(font, "ENEMY SITUATION", 429, 104, None);
        self.line(
            &mut c,
            font,
            35,
            137,
            &[("Friendly forces are ", None), ("", Some(3))],
        );
        self.line(
            &mut c,
            font,
            340,
            137,
            &[("Enemy forces are ", None), ("", Some(20))],
        );
        for (x, start) in [(35, 4), (340, 21)] {
            for wing in 0..3 {
                let id = start + wing * 3;
                let y = 153 + wing as i32 * 14;
                self.line(
                    &mut c,
                    font,
                    x,
                    y,
                    &[
                        (&format!("Wing {}: ", wing + 1), None),
                        ("", Some(id)),
                        (" ", None),
                        ("", Some(id + 1)),
                        (" ", None),
                        ("", Some(id + 2)),
                    ],
                );
            }
            for wing in 0..3 {
                let group = wing + usize::from(start == 21) * 3;
                let prefix = self.objective_prefix(group);
                self.line(
                    &mut c,
                    font,
                    x,
                    201 + wing as i32 * 14,
                    &[
                        (&prefix, None),
                        ("", Some(OBJECTIVE_BASE + group)),
                        (".", None),
                    ],
                );
            }
        }
        for (x, side_offset) in [(35, 0), (340, 3)] {
            for wing in 0..3 {
                let group = side_offset + wing;
                let prefix = if group == 0 {
                    "Your group's survival is ".to_owned()
                } else {
                    format!("Wing {}'s survival is ", wing + 1)
                };
                self.line(
                    &mut c,
                    font,
                    x,
                    249 + wing as i32 * 14,
                    &[
                        (&prefix, None),
                        ("", Some(SURVIVAL_BASE + group)),
                        (".", None),
                    ],
                );
            }
        }
        for (y, parts) in [
            (301, vec![("You are flying over ", None), ("", Some(13))]),
            (
                315,
                vec![
                    (
                        if self.ground_start() {
                            "Airborne wings at "
                        } else {
                            "You are at "
                        },
                        None,
                    ),
                    ("", Some(14)),
                    (" feet. It is ", None),
                    ("", Some(15)),
                ],
            ),
            (329, vec![("Your situation is ", None), ("", Some(16))]),
            (
                343,
                vec![
                    ("You are ", None),
                    ("", Some(17)),
                    (" from enemy forces.", None),
                ],
            ),
            (371, vec![("You are carrying ", None), ("", Some(18))]),
            (385, vec![("Air combat is with ", None), ("", Some(19))]),
        ] {
            self.line(&mut c, font, 35, y, &parts);
        }
        self.line(&mut c, font, 35, 357, &[("Start: ", None), ("", Some(33))]);
        if self.ground_start() {
            self.line(
                &mut c,
                font,
                35,
                399,
                &[("Airport: ", None), ("", Some(34))],
            );
        }
        let (mut x, mut y) = (340, 301);
        for (text, id) in [
            ("Friendly ground target is ", None),
            ("", Some(30)),
            ("", Some(31)),
            ("defended by AAA and ", None),
            ("", Some(32)),
            ("defended by SAMs.", None),
        ] {
            let text = id.map(|i| self.value(i)).unwrap_or_else(|| text.into());
            for word in text.split_whitespace() {
                let width = text_width(font, word);
                if x + width > 605 {
                    x = 340;
                    y += 14;
                }
                if let Some(id) = id {
                    let rect = (
                        x - 1,
                        y - 1,
                        width + 2,
                        font.glyphs.iter().map(|g| g[2]).max().unwrap_or(9) as i32 + 2,
                    );
                    self.controls.push((id, rect));
                    c.rect(
                        rect,
                        if self.hover == Some(id) {
                            [127, 139, 144, 255]
                        } else {
                            [101, 107, 109, 255]
                        },
                    );
                    bevel(&mut c, rect, false);
                }
                c.text(font, word, x, y, None);
                x += width + text_width(font, " ") + if id.is_some() { 2 } else { 0 };
            }
        }
        self.button(&mut c, sprites, OK, "OK", (387, 419, 85, 24));
        self.button(&mut c, sprites, CANCEL, "Cancel", (492, 419, 85, 24));
        if let Some(message) = &self.notice {
            notice(&mut c, &sprites["SMLFONT.PIC"], message);
        }
        if self.help {
            c.rect((84, 60, 180, 25), [212, 215, 218, 255]);
            c.text(&sprites["MENUFONT.PIC"], "Exit to Desktop", 89, 64, None);
            self.controls = vec![(0, (84, 35, 18, 24)), (61, (84, 60, 180, 25))];
        }
        if let Some(field) = self.selector {
            let selector_values = self.selector_values(field);
            self.controls.clear();
            // Reuse the original metal panel texture, inset list wells and rocker.
            let background = &sprites["QUIKMIS3.PIC"];
            for y in 0..POPUP.3 {
                for x in 0..POPUP.2 {
                    let source = (((140 + y % 240) as usize * WIDTH) + (50 + x % 240) as usize) * 4;
                    c.rect(
                        (POPUP.0 + x, POPUP.1 + y, 1, 1),
                        background.rgba[source..source + 4].try_into().unwrap(),
                    );
                }
            }
            bevel(&mut c, POPUP, false);
            for row in 0..ROWS {
                let y = 116 + row as i32 * 18;
                c.rect((207, y, 226, 14), [12, 16, 16, 255]);
                bevel(&mut c, (207, y, 226, 14), true);
                let Some(text) = selector_values.get(self.scroll + row) else {
                    continue;
                };
                stripe(
                    &mut c,
                    (209, y + 1, 11, 12),
                    self.cursor == self.scroll + row,
                );
                c.text(font, &fit(font, text, 208), 223, y + 2, None);
            }
            for row in 0..ROWS.min(selector_values.len().saturating_sub(self.scroll)) {
                self.controls
                    .push((ROW_BASE + row, (207, 116 + row as i32 * 18, 226, 14)));
            }
            c.text(font, "PAGE", 277, 394, None);
            c.rect((307, 389, 50, 17), [12, 16, 16, 255]);
            bevel(&mut c, (307, 389, 50, 17), true);
            c.text(
                font,
                &format!(
                    "{} of {}",
                    self.scroll / ROWS + 1,
                    selector_values.len().div_ceil(ROWS).max(1)
                ),
                314,
                394,
                None,
            );
            c.text(font, "PREV", 380, 394, None);
            c.text(font, "NEXT", 380, 417, None);
            let rocker = &sprites[&self.rocker.sprite()];
            c.blit(rocker, (410, 393), 0, rocker.width, 1.);
            self.controls
                .extend([(UP, (410, 393, 18, 17)), (DOWN, (410, 410, 18, 17))]);
            if selector_values.is_empty() {
                c.text(font, "No available choices.", 209, 118, None);
            }
            self.button(&mut c, sprites, POP_OK, "OK", (217, 437, 85, 24));
            self.button(&mut c, sprites, POP_CANCEL, "Cancel", (312, 437, 85, 24));
        }
        animating
    }
    fn line(
        &mut self,
        c: &mut Canvas,
        font: &Sprite,
        x: i32,
        y: i32,
        parts: &[(&str, Option<usize>)],
    ) {
        let mut x = x;
        for (text, id) in parts {
            let text = id
                .map(|i| self.value(i))
                .unwrap_or_else(|| text.to_string());
            if id.is_some() {
                x += 2;
            }
            let text = fit(font, &text, if x < 320 { 299 - x } else { 605 - x });
            let width = text_width(font, &text);
            if let Some(id) = id {
                let r = (
                    x - 1,
                    y - 1,
                    width + 2,
                    font.glyphs.iter().map(|g| g[2]).max().unwrap_or(9) as i32 + 2,
                );
                self.controls.push((*id, r));
                c.rect(
                    r,
                    if self.hover == Some(*id) {
                        [127, 139, 144, 255]
                    } else {
                        [101, 107, 109, 255]
                    },
                );
                bevel(c, r, false);
            }
            c.text(font, &text, x, y, None);
            x += width + if id.is_some() { 2 } else { 0 };
        }
    }
    fn button(
        &mut self,
        c: &mut Canvas,
        sprites: &BTreeMap<String, Sprite>,
        id: usize,
        label: &str,
        r: Rect,
    ) {
        let hit = c.action_button(
            sprites,
            label,
            (r.0, r.1, r.2),
            id == OK || id == POP_OK,
            self.pressed == Some(id),
        );
        self.controls.push((id, hit));
    }
}
fn bevel(c: &mut Canvas, (x, y, w, h): Rect, inset: bool) {
    let light = [115, 120, 119, 255];
    let dark = [24, 27, 27, 255];
    let (top, bottom) = if inset { (dark, light) } else { (light, dark) };
    c.rect((x, y, w, 1), top);
    c.rect((x, y, 1, h), top);
    c.rect((x, y + h - 1, w, 1), bottom);
    c.rect((x + w - 1, y, 1, h), bottom);
}
fn stripe(c: &mut Canvas, (x, y, w, h): Rect, selected: bool) {
    // Fitted diagonal status marker, using the reference's blue/gold treatment.
    let colors = if selected {
        [[230, 181, 39, 255], [73, 53, 17, 255]]
    } else {
        [[180, 193, 215, 255], [58, 94, 168, 255]]
    };
    for yy in 0..h {
        for xx in 0..w {
            c.rect(
                (x + xx, y + yy, 1, 1),
                colors[((xx - yy).rem_euclid(6) / 3) as usize],
            );
        }
    }
}
fn source_theaters() -> [&'static str; 16] {
    [
        "BAL", "CUB", "EGY", "LFA", "FRA", "GRE", "IRA", "KURILE", "TVIET", "SPA", "APA", "PGU",
        "NSK", "WTA", "UKR", "VLA",
    ]
}
pub fn runway_pose(world: &World, object: u32) -> crate::AppResult<([f64; 3], f64)> {
    let runway = world
        .airport_scene
        .runway(object)
        .ok_or("Selected runway is unavailable; choose another airport.")?;
    let (position, heading) = runway.departure_pose();
    if !runway.surface.contains_horizontal(position[0], position[2]) {
        return Err("Selected runway has no supported departure point".into());
    }
    Ok((position, heading))
}

/// `fitted`, agent decision 2026-09-23: buildings are checked at this height
/// above the runway at every parking point, about a fighter's wheel-to-centre
/// clearance, before any aircraft is placed there.
const SLOT_PROBE_FT: f64 = 6.;

/// Where the player's wing parks for a ground start.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundLayout {
    pub object: u32,
    pub airport: u32,
    pub runway: tore_sim::ai::airfield::RunwayView,
    /// Departure heading down the runway, the leader's heading.
    pub heading: f64,
    /// True when the airport's own takeoff spot and parking slots are used;
    /// false for the fitted staggered runway fallback.
    pub anchored: bool,
    /// Staggered fallback only: distance between parked aircraft.
    pub spacing_ft: Option<f64>,
    /// Surface points, leader (the player) first.
    pub slots: Vec<[f64; 3]>,
    /// Heading of each slot.
    pub headings: Vec<f64>,
}

impl GroundLayout {
    /// The same slots as [right, forward] offsets from the player's slot.
    pub fn offsets(&self) -> Vec<[f64; 2]> {
        layout::relative_offsets(&self.slots, self.heading)
    }

    /// The departure handed to the AI wingmen.
    pub fn departure(&self) -> crate::ai_wings::Departure {
        crate::ai_wings::Departure {
            runway: self.runway,
            headings: self.headings.clone(),
            slots: self.slots.clone(),
        }
    }
}

/// Place `count` aircraft (the player's whole wing) for a ground start. With
/// the airport's own points the player takes the takeoff spot and wingmen the
/// parking slots; otherwise the wing parks staggered on the runway. Every slot
/// must be on the airport's paving, on a landable surface and clear of
/// buildings. If the staggered layout cannot fit, its spacing tightens, and
/// if none works the start is rejected with a message for the creator.
pub fn ground_layout(world: &World, object: u32, count: usize) -> crate::AppResult<GroundLayout> {
    let runway = world
        .airport_scene
        .runway(object)
        .ok_or("Selected runway is unavailable; choose another airport.")?;
    let view = world
        .runway_view(object)
        .ok_or("Selected runway is unavailable; choose another airport.")?;
    let buildings: Vec<u32> = world.airport_scene.objects.iter().map(|o| o.id).collect();
    let check = |p: [f64; 3]| -> Result<[f64; 3], String> {
        if !runway.surface.contains_horizontal(p[0], p[2]) {
            return Err(if count > 1 {
                "The selected runway is too short for your whole wing.".into()
            } else {
                "Selected runway has no supported departure point".into()
            });
        }
        let surface = world.surface(p[0], p[2]);
        if !surface.landable {
            return Err("Selected runway does not provide a ground surface".into());
        }
        let probe = [p[0], surface.height + SLOT_PROBE_FT, p[2]];
        if world
            .solid_contact(probe, probe, buildings.iter().copied())
            .is_some()
        {
            return Err("The runway start is obstructed. Choose another airport.".into());
        }
        Ok([p[0], surface.height, p[2]])
    };
    let heading = layout::departure_heading(runway);
    let mut check = check;
    if let Some(slots) = view
        .anchors
        .as_ref()
        .and_then(|anchors| layout::anchored_slots(anchors, count, &mut check))
    {
        let (slots, headings) = slots.into_iter().unzip();
        return Ok(GroundLayout {
            object,
            airport: runway.airport,
            runway: view,
            heading,
            anchored: true,
            spacing_ft: None,
            slots,
            headings,
        });
    }
    let (spacing_ft, slots) = layout::fit_runway_slots(runway, count, check).map_err(|why| {
        if count > 1 {
            format!("{why} Your wing of {count} could not be parked; choose another airport or fewer wingmen.")
        } else {
            why
        }
    })?;
    Ok(GroundLayout {
        object,
        airport: runway.airport,
        runway: view,
        heading,
        anchored: false,
        spacing_ft: Some(spacing_ft),
        headings: vec![heading; slots.len()],
        slots,
    })
}

/// Stand an aircraft in its parking slot: stationary, engine idling, gear and
/// flaps down, brakes set. Rejects a slot where the aircraft itself would sit
/// inside a building.
pub fn place_on_runway(
    world: &World,
    flight: &mut tore_sim::flight::State,
    layout: &GroundLayout,
    order: usize,
) -> crate::AppResult<()> {
    let position = *layout
        .slots
        .get(order)
        .ok_or("The ground start has no slot for this aircraft")?;
    let heading = layout
        .headings
        .get(order)
        .copied()
        .unwrap_or(layout.heading);
    let mut candidate = flight.clone();
    candidate.start_on_runway(position, heading)?;
    if world
        .solid_contact(
            candidate.position,
            candidate.position,
            world.airport_scene.objects.iter().map(|o| o.id),
        )
        .is_some()
    {
        return Err("The runway start is obstructed. Choose another airport.".into());
    }
    *flight = candidate;
    Ok(())
}

/// The player alone on the runway, as the `--ground-start` developer option
/// and the straight-flight fixtures use it.
pub fn apply_ground_start(
    world: &World,
    flight: &mut tore_sim::flight::State,
    object: u32,
) -> crate::AppResult<u32> {
    let layout = ground_layout(world, object, 1)?;
    place_on_runway(world, flight, &layout, 0)?;
    Ok(layout.airport)
}

/// Everything the creator decided about where aircraft start, kept so a
/// restart rebuilds exactly the same scene.
#[derive(Clone, Debug, PartialEq)]
pub struct MissionLayout {
    /// The player's wing parked on this runway, for a ground start.
    pub ground: Option<GroundLayout>,
    /// Airborne start only: the turn added to the player's starting heading,
    /// radians clockwise, so that the enemy ahead stays on the map. The whole
    /// airborne scene turns with the player.
    pub player_turn: f64,
    /// Where the enemy group sits relative to the player.
    pub enemy: EnemyAim,
}

impl MissionLayout {
    /// Plan a mission around `start`, the player's pose before any ground
    /// placement (a ground start takes the leader's runway slot instead).
    /// `group` is every enemy aircraft's offset from the enemy placement
    /// point ([`crate::ai_wings::enemy_group_offsets`]).
    pub fn plan(
        world: &World,
        start: &tore_sim::flight::State,
        ground: Option<GroundLayout>,
        group: &[[f64; 2]],
        separation_ft: f64,
    ) -> Self {
        let (reference, heading) = match &ground {
            Some(g) => ([g.slots[0][0], g.slots[0][2]], g.heading),
            None => ([start.position[0], start.position[2]], start.yaw),
        };
        let enemy =
            layout::aim_into_map(reference, heading, separation_ft, group, map_bounds(world));
        Self {
            player_turn: if ground.is_some() { 0. } else { enemy.turn },
            ground,
            enemy,
        }
    }

    pub fn spawn_plan(&self) -> crate::ai_wings::SpawnPlan {
        crate::ai_wings::SpawnPlan {
            separation_ft: self.enemy.distance_ft,
            // An airborne scene turns with the player; parked aircraft cannot,
            // so only the enemy bearing changes.
            enemy_turn: if self.ground.is_some() {
                self.enemy.turn
            } else {
                0.
            },
            runway_slots: self.ground.as_ref().map(GroundLayout::offsets),
        }
    }

    /// A line for the player when the chosen separation did not fit.
    pub fn notice(&self) -> Option<String> {
        self.enemy.shortened().then(|| {
            format!(
                "Enemy forces start {:.0} miles away: {:.0} miles does not fit this theater.",
                (self.enemy.distance_ft / FEET_PER_NM).floor(),
                self.enemy.requested_ft / FEET_PER_NM
            )
        })
    }
}

/// The usable map for starting aircraft: the terrain less one cell each side.
pub fn map_bounds(world: &World) -> MapBounds {
    let cell = f64::from(tore_formats::theater::CELL_FEET);
    MapBounds::from_cells(
        world.theater.cols,
        world.theater.rows,
        cell,
        layout::MAP_MARGIN_CELLS * cell,
    )
}

fn inside(p: (f64, f64), r: Rect) -> bool {
    p.0 >= r.0 as f64 && p.1 >= r.1 as f64 && p.0 < (r.0 + r.2) as f64 && p.1 < (r.1 + r.3) as f64
}
pub(crate) fn fit(font: &Sprite, text: &str, width: i32) -> String {
    let mut s = text.to_string();
    if text_width(font, &s) > width {
        while !s.is_empty() && text_width(font, &format!("{s}...")) > width {
            s.pop();
        }
        s.push_str("...");
    }
    s
}
pub fn notice(c: &mut Canvas, font: &Sprite, text: &str) {
    c.rect((30, 335, 580, 65), [35, 44, 46, 255]);
    let mut line = String::new();
    let mut y = 340;
    for word in text.split_whitespace() {
        let next = format!("{line}{word} ");
        if text_width(font, &next) > 565 {
            c.text(font, &line, 36, y, Some([235, 225, 179]));
            line.clear();
            y += 13;
        }
        line.push_str(word);
        line.push(' ');
    }
    c.text(font, &line, 36, y, Some([235, 225, 179]));
}
pub fn hud(pixels: &mut [u8], sprites: &BTreeMap<String, Sprite>, camera: &Camera, world: &World) {
    pixels.fill(0);
    let mut c = Canvas(pixels);
    let font = &sprites["SMLFONT.PIC"];
    c.rect((0, 0, WIDTH as i32, 37), [18, 26, 30, 230]);
    c.text(
        font,
        &format!(
            "{}   TERRAIN VIEWER     ALT {:.0} ft",
            world.theater.name.to_ascii_uppercase(),
            camera.position[1]
        ),
        12,
        8,
        Some([230, 234, 222]),
    );
    c.text(
        font,
        "Arrows: move   Shift: fast   Q/E: down/up   WASD: look   Esc: return",
        12,
        24,
        Some([210, 218, 220]),
    );
    c.rect((8, HEIGHT as i32 - 25, 365, 19), [18, 26, 30, 225]);
    c.text(
        font,
        "Retail terrain / midday preview - environment parity in progress",
        13,
        HEIGHT as i32 - 20,
        Some([210, 218, 220]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variant_selection_retains_layout_and_uses_base_country_and_target_lists() {
        let mut q = setup();
        q.theater_codes.push("~UKR1".into());
        q.theater_catalog.push("~UKR1".into());
        q.options.fields[13].push("Ukraine (UKR1)".into());
        q.airport_names.push(vec!["Variant runway".into()]);
        q.airport_objects.push(vec![0x4000_0003]);
        q.theater(16);
        assert_eq!(q.theater_index(), 16);
        assert_eq!(q.value(13), "Ukraine (UKR1)");
        let base = source_theaters()
            .iter()
            .position(|name| *name == "UKR")
            .unwrap();
        assert_eq!(q.base_theater_index(), base);
        assert_eq!(q.values(30), q.options.targets[base]);
        assert_eq!(q.values(34), ["Variant runway"]);
        q.apply(33, 1);
        assert_eq!(q.ground_runway(), Some(0x4000_0003));
        q.theater(0);
        assert_eq!(q.theater_index(), 0);
        assert_eq!(q.draft.values[34], 0);
    }
    fn setup() -> QuickMission {
        let mut options = Options {
            fields: vec![vec!["value".into(); 60]; 33],
            targets: vec![vec!["none".into(), "target".into()]; 16],
        };
        options.fields[15] = [
            "dawn",
            "clear",
            "cloudy",
            "overcast.",
            "foggy",
            "sunset",
            "night",
        ]
        .map(String::from)
        .to_vec();
        options.fields[4] = (0..6).map(|i| i.to_string()).collect();
        let mut q = QuickMission::new(AircraftId::F18, options, &BTreeMap::new());
        q.aircraft_names = vec!["Hornet".into(), "Rafale".into(), "Other".into()];
        q.aircraft_files = vec!["F18.PT".into(), "RAFALE.PT".into(), "OTHER.PT".into()];
        q
    }
    fn right_click(q: &mut QuickMission, id: usize) -> Action {
        q.hover = Some(id);
        assert!(matches!(q.right(true), Action::None));
        q.right(false)
    }
    #[test]
    fn group_objectives_are_independent_and_inactive_groups_keep_their_choice() {
        let mut q = setup();
        q.draft.values[7] = 0;
        q.open(OBJECTIVE_BASE + 1);
        q.cursor = QuickMission::objective_choices(1)
            .iter()
            .position(|(_, objective)| *objective == GroupObjective::Hold)
            .unwrap();
        q.activate(POP_OK);
        assert_eq!(q.group_objectives[1], GroupObjective::Hold);
        assert!(
            q.group_objectives
                .iter()
                .enumerate()
                .all(|(index, objective)| index == 1 || *objective == GroupObjective::Inherit)
        );
        q.draft.values[7] = 3;
        assert_eq!(q.group_objectives[1], GroupObjective::Hold);
    }

    #[test]
    fn survival_requirement_is_independent_of_orders_and_inactive_group_count() {
        let mut q = setup();
        let group = 1;
        q.draft.values[7] = 0;
        q.group_objectives[group] = GroupObjective::Intercept(WingId::new(Side::Enemy, 0).unwrap());
        assert!(matches!(q.activate(SURVIVAL_BASE + group), Action::Click));
        assert!(q.group_must_survive[group]);
        assert_eq!(q.value(SURVIVAL_BASE + group), "required");
        q.draft.values[7] = 3;
        assert!(q.group_must_survive[group]);
        assert_eq!(
            q.group_objectives[group],
            GroupObjective::Intercept(WingId::new(Side::Enemy, 0).unwrap())
        );
        assert!(matches!(
            right_click(&mut q, SURVIVAL_BASE + group),
            Action::Click
        ));
        assert!(!q.group_must_survive[group]);
        assert_eq!(q.value(SURVIVAL_BASE + group), "optional");
    }

    #[test]
    fn objective_choices_restrict_targets_by_side_and_exclude_self_escort() {
        for group in 0..OBJECTIVE_COUNT {
            let side = if group < 3 {
                Side::Friendly
            } else {
                Side::Enemy
            };
            let own_index = (group % 3) as u8;
            let choices = QuickMission::objective_choices(group);
            let intercepts: Vec<_> = choices
                .iter()
                .filter_map(|(_, objective)| match objective {
                    GroupObjective::Intercept(wing) => Some(*wing),
                    _ => None,
                })
                .collect();
            assert_eq!(intercepts.len(), 3);
            assert!(intercepts.iter().all(|wing| wing.side != side));
            let escorts: Vec<_> = choices
                .iter()
                .filter_map(|(_, objective)| match objective {
                    GroupObjective::Escort(wing) => Some(*wing),
                    _ => None,
                })
                .collect();
            assert_eq!(escorts.len(), 2);
            assert!(
                escorts
                    .iter()
                    .all(|wing| wing.side == side && wing.index != own_index)
            );
        }
    }

    #[test]
    fn objective_popup_and_right_click_cycle_only_the_addressed_group() {
        let mut q = setup();
        q.open(OBJECTIVE_BASE);
        q.cursor = 2;
        q.activate(POP_OK);
        assert_eq!(q.group_objectives[0], GroupObjective::Cap);
        assert!(matches!(right_click(&mut q, OBJECTIVE_BASE), Action::Click));
        assert_eq!(q.group_objectives[0], GroupObjective::Free);
        assert_eq!(q.group_objectives[1], GroupObjective::Inherit);
    }

    #[test]
    fn inherited_stamp_names_the_actual_mission_setting() {
        let mut q = setup();
        assert_eq!(q.objective_label(0), "use free fire (mission)");
        q.ai_mission = crate::ai_wings::Preset::Escort;
        assert_eq!(q.objective_label(5), "follow mission orders");
        q.ai_mission = crate::ai_wings::Preset::SelfDefense;
        assert_eq!(q.objective_label(2), "defend itself (mission)");
        q.group_objectives[2] = GroupObjective::Intercept(
            WingId::new(Side::Enemy, 2).expect("fixed Quick Mission wing"),
        );
        assert_eq!(q.objective_label(2), "enemy group 3");
    }
    #[test]
    fn all_six_skill_selectors_launch_dummy_members() {
        let mut q = setup();
        for field in [4, 7, 10, 21, 24, 27] {
            assert_eq!(q.values(field + 1)[4], "Dummy (400 KTS)");
            q.draft.values[field] = 2;
            q.draft.values[field + 1] = 4;
            q.draft.values[field + 2] = 0;
        }
        let wings = q
            .wing_launches(Some(EnemySkillOverride::AllAverage))
            .unwrap();
        assert_eq!(wings.len(), 6);
        assert_eq!(wings[0].count(), 1); // Human slot is excluded.
        assert!(wings.iter().all(|w| w.dummy));
        q.draft.values[22] = 3;
        assert!(!q.wing_launches(None).unwrap()[3].dummy);
    }

    #[test]
    fn right_click_reverses_values_wraps_and_keeps_player_count_positive() {
        let mut q = setup();
        q.apply(4, 2);
        assert!(matches!(right_click(&mut q, 4), Action::Click));
        assert_eq!(q.draft.values[4], 1);
        right_click(&mut q, 4);
        assert_eq!(q.draft.values[4], 5);
        q.apply(15, 1);
        right_click(&mut q, 15);
        assert_eq!(q.value(15), "dawn");
        right_click(&mut q, 15);
        assert_eq!(q.value(15), "night");
        q.activate(15); // Existing forward cycle reverses the last change.
        assert_eq!(q.value(15), "dawn");
        q.apply(30, 1);
        q.apply(31, 2);
        q.apply(32, 2);
        right_click(&mut q, 30);
        assert_eq!(&q.draft.values[30..33], &[0, 0, 0]);
        q.options.fields[14].clear();
        assert!(matches!(right_click(&mut q, 14), Action::None));
    }
    #[test]
    fn right_click_cannot_activate_actions_or_fields_behind_a_popup() {
        let mut q = setup();
        let before = q.draft.values;
        for id in [0, OK, CANCEL, 60, 61, POP_OK, POP_CANCEL, ROW_BASE] {
            assert!(matches!(right_click(&mut q, id), Action::None));
            assert_eq!(q.draft.values, before);
        }
        q.hover = Some(15);
        q.right(true);
        q.hover = Some(14);
        assert!(matches!(q.right(false), Action::None));
        q.hover = Some(15);
        q.right(true);
        q.cancel();
        q.hover = Some(15);
        assert!(matches!(q.right(false), Action::None));
        q.open(15);
        assert!(matches!(right_click(&mut q, 15), Action::None));
        assert_eq!(q.draft.values, before);
        q.cancel();
        q.help = true;
        assert!(matches!(right_click(&mut q, 15), Action::None));
        assert_eq!(q.draft.values, before);
    }
    #[test]
    fn ground_start_airport_picker_cancels_and_resets_on_theater_change() {
        let mut q = setup();
        q.airport_names[0] = vec!["First Field".into(), "Second Field".into()];
        q.airport_objects[0] = vec![0x40000000, 0x40000003];
        assert!(!q.ground_start());
        q.activate(33);
        assert!(q.ground_start());
        q.activate(34);
        assert_eq!(q.selector, Some(34));
        q.activate(ROW_BASE + 1);
        q.activate(POP_CANCEL);
        assert_eq!(q.ground_runway(), Some(0x40000000));
        q.activate(34);
        q.activate(ROW_BASE + 1);
        q.activate(POP_OK);
        assert_eq!(q.ground_runway(), Some(0x40000003));
        q.apply(13, 1);
        assert_eq!(q.draft.values[34], 0);
        assert!(q.ground_runway().is_none());
        assert!(q.unsupported().unwrap().contains("No imported runways"));
        q.apply(33, 0);
        assert!(q.unsupported().is_none());
    }
    #[test]
    fn tab_reaches_ground_controls_without_focusing_hidden_airport() {
        let mut q = setup();
        q.focus = 32;
        q.key("Tab", false);
        assert_eq!(q.focus, 33);
        q.key("Tab", false);
        assert_eq!(q.focus, OBJECTIVE_BASE);
        for expected in OBJECTIVE_BASE + 1..SURVIVAL_BASE + OBJECTIVE_COUNT {
            q.key("Tab", false);
            assert_eq!(q.focus, expected);
        }
        q.key("Tab", false);
        assert_eq!(q.focus, 1);
        q.apply(33, 1);
        q.focus = 33;
        q.key("Tab", false);
        assert_eq!(q.focus, 34);
        q.key("Tab", false);
        assert_eq!(q.focus, OBJECTIVE_BASE);
        q.key("ArrowUp", false);
        assert_eq!(q.focus, 34);
        q.focus = 1;
        q.key("Tab", true);
        assert_eq!(q.focus, SURVIVAL_BASE + OBJECTIVE_COUNT - 1);
        q.key("ArrowDown", false);
        assert_eq!(q.focus, 1);
    }
    #[test]
    fn selectors_page_and_cancel_without_changing_the_draft() {
        let mut q = setup();
        q.activate(13);
        assert_eq!(q.selector, Some(13));
        q.activate(DOWN);
        assert_eq!(q.scroll, 15);
        q.activate(ROW_BASE);
        assert_eq!(q.cursor, 15);
        q.activate(POP_CANCEL);
        assert_eq!(q.draft.values[13], 0);
        q.open(13);
        q.key("PageDown", false);
        assert_eq!((q.scroll, q.cursor), (15, 15));
        q.key("Enter", false);
        assert_eq!(q.draft.values[13], 15);
        q.aircraft_names.clear();
        q.aircraft_files.clear();
        q.open(6);
        assert_eq!(q.activate(POP_OK), Action::None);
        assert!(q.player().is_none());
    }
    #[test]
    fn catalog_metadata_and_variant_aliases_do_not_create_imported_aircraft() {
        let options = setup().options;
        let data = ["F18C.PT", "RAFALEE.PT", "RAFALEF.PT", "OTHER.PT", "F18.PT"]
            .into_iter().map(|name| (name.to_string(), format!(
                "[brent's_relocatable_format]\n:ot_names\nstring \"Plane\"\nstring \"Planes\"\nstring \"{name}\"\nend\n"
            ).into_bytes())).collect();
        let q = QuickMission::new(AircraftId::F18, options, &data);
        assert!(q.aircraft_files.is_empty());
    }
    #[test]
    fn draft_cancel_and_pointer_release_are_transactional() {
        let mut q = setup();
        q.open(6);
        q.key("ArrowDown", false);
        assert_eq!(q.draft.values[6], 0);
        q.key("Escape", false);
        assert_eq!(q.draft.values[6], 0);
        q.open(6);
        q.key("ArrowDown", false);
        q.key("Enter", false);
        assert_eq!(q.player(), Some(AircraftId::Rafale));
        q.controls = vec![(4, (10, 10, 20, 20))];
        q.pointer(Some((15., 15.)));
        q.down();
        q.pointer(None);
        assert_eq!(q.up(), Action::None);
        assert_eq!(q.draft.values[4], 1);
    }
    #[test]
    fn source_dependencies_and_theater_identity_are_preserved() {
        let mut q = setup();
        q.apply(4, 0);
        assert_eq!(q.draft.values[4], 1);
        q.draft.values[31] = 3;
        q.draft.values[32] = 3;
        q.apply(30, 0);
        assert_eq!(&q.draft.values[30..33], &[0, 0, 0]);
        for i in 0..16 {
            q.theater(i);
            assert_eq!(q.theater_index(), i);
        }
        q.apply(13, 3);
        assert_eq!(q.draft.values[20], 57);
        assert_eq!(q.draft.values[3], 0);
        q.apply(13, 9);
        assert_eq!(q.draft.values[20], 37);
    }
    #[test]
    fn all_wings_launch_with_the_selected_identities() {
        let mut q = setup();
        for field in [4, 7, 10, 21, 24, 27] {
            q.apply(field, 5);
        }
        q.apply(23, 1);
        assert!(q.unsupported().is_none());
        let wings = q.dummy_wings();
        assert_eq!(wings.iter().map(|(_, n)| n).sum::<usize>(), 29);
        assert_eq!(wings[0], (AircraftId::F18, 4));
        assert_eq!(wings[3], (AircraftId::Rafale, 5));
        q.apply(23, 2);
        assert!(q.unsupported().unwrap().contains("populated wing"));
    }
    #[test]
    fn wing_skill_selections_reach_the_launch_payload() {
        use tore_sim::ai::Experience;
        use tore_sim::ai::experience::ExperienceOrigin;
        let mut q = setup();
        // Every wing populated, with a different skill per wing row.
        for field in [4, 7, 10, 21, 24, 27] {
            q.apply(field, 5);
        }
        for (field, level) in [(5, 0), (8, 1), (11, 2), (22, 3), (25, 0), (28, 1)] {
            q.apply(field, level);
        }
        let wings = q.wing_launches(None).unwrap();
        assert_eq!(wings.len(), 6);
        let levels = [
            Experience::Novice,
            Experience::Average,
            Experience::Experienced,
            Experience::Ace,
            Experience::Novice,
            Experience::Average,
        ];
        for (wing, level) in wings.iter().zip(levels) {
            assert_eq!(wing.selected_level, level);
            assert!(!wing.is_empty());
            for member in &wing.members {
                assert_eq!(member.experience.level, level);
                assert_eq!(
                    member.experience.origin,
                    ExperienceOrigin::QuickMission { selected: level }
                );
            }
            assert!(wing.leader().is_some_and(|m| m.member == 0));
        }
        // Sides and the player's slot in friendly wing 1 are preserved.
        assert_eq!(wings[0].count(), 4);
        assert_eq!(wings[3].count(), 5);
        let sides: Vec<bool> = wings.iter().map(|w| w.wing.side.is_enemy()).collect();
        assert_eq!(sides, [false, false, false, true, true, true]);
        assert_eq!(
            wings.iter().map(|w| w.wing.index).collect::<Vec<_>>(),
            [0, 1, 2, 0, 1, 2]
        );
    }
    #[test]
    fn the_enemy_skill_override_changes_enemy_wings_only() {
        use tore_sim::ai::Experience;
        use tore_sim::ai::experience::ExperienceOrigin;
        let mut q = setup();
        for field in [4, 7, 10, 21, 24, 27] {
            q.apply(field, 5);
        }
        for field in [5, 8, 11, 22, 25, 28] {
            q.apply(field, 3);
        }
        let wings = q
            .wing_launches(Some(EnemySkillOverride::AllNovice))
            .unwrap();
        for wing in &wings {
            assert_eq!(wing.selected_level, Experience::Ace);
            let enemy = wing.wing.side.is_enemy();
            for member in &wing.members {
                if enemy {
                    assert_eq!(member.experience.level, Experience::Novice);
                    assert_eq!(member.experience.origin, ExperienceOrigin::EnemyOverride);
                } else {
                    assert_eq!(member.experience.level, Experience::Ace);
                    assert_eq!(
                        member.experience.origin,
                        ExperienceOrigin::QuickMission {
                            selected: Experience::Ace
                        }
                    );
                }
            }
        }
        // The legacy pairs are unchanged by the override.
        assert_eq!(legacy_pairs(&wings), q.dummy_wings());
    }
    #[test]
    fn separation_lists_eight_nautical_choices_and_never_panics() {
        let mut q = setup();
        let labels = q.values(17).to_vec();
        assert_eq!(labels.len(), 8);
        assert_eq!(&labels[6..], ["200 miles", "300 miles"]);
        for (index, nm) in SEPARATION_NM.into_iter().enumerate() {
            q.apply(17, index);
            assert_eq!(q.separation_nm(), nm);
            assert_eq!(q.separation_feet(), nm * 6_076.12);
        }
        // Default is 5 miles; an index past the table falls back to it.
        assert_eq!(Draft::default().values[17], 2);
        q.draft.values[17] = 99;
        assert_eq!(q.separation_feet(), 5. * FEET_PER_NM);
        // Clicking cycles through the host entries and wraps.
        q.draft.values[17] = 5;
        q.activate(17);
        assert_eq!(q.value(17), "200 miles");
        q.activate(17);
        q.activate(17);
        assert_eq!(q.draft.values[17], 0);
    }

    const RUNWAY: u32 = 0x4000_0001;

    /// The synthetic terrain with one 6000 ft north-facing runway, its near
    /// threshold 1096 ft into the map.
    fn airfield(length_ft: f64) -> World {
        use tore_sim::airport::{Airport, Allegiance, OrientedBox, SourceKey, StaticObject};
        let mut world = crate::terrain::tests::world();
        let surface = OrientedBox {
            center: [4096., 20., 4096.],
            half: [1500., 2., length_ft / 2.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        };
        let scene = &mut world.airport_scene;
        scene.objects.push(StaticObject {
            id: RUNWAY,
            source: SourceKey {
                layout: "T.MM".into(),
                ordinal: 1,
            },
            name: "Strip".into(),
            object_type: "RNWY1.OT".into(),
            bounds: surface,
            hit_points: 100,
            category: 0,
            radar_signature: 1.,
            infrared_signature: 0.,
            runway: true,
        });
        scene.runways.push(tore_sim::airport::Runway {
            object: RUNWAY,
            airport: 9,
            name: "Strip".into(),
            surface,
            approach_center: [4096., 20., 4096.],
            elevation_ft: 20.,
            heading: 0.,
            length_ft,
        });
        scene.airports.push(Airport {
            id: 9,
            name: "Strip".into(),
            runway_objects: vec![RUNWAY],
            allegiance: Allegiance::Neutral,
            neutral_permission: true,
        });
        world
    }

    fn hangar(world: &mut World, id: u32, along_ft: f64) {
        let near = 4096. - world.airport_scene.runways[0].length_ft / 2.;
        building(world, id, 4096., near + along_ft);
    }

    fn building(world: &mut World, id: u32, x: f64, z: f64) {
        world
            .airport_scene
            .objects
            .push(tore_sim::airport::StaticObject {
                id,
                source: tore_sim::airport::SourceKey {
                    layout: "T.MM".into(),
                    ordinal: id,
                },
                name: "Hangar".into(),
                object_type: "HANGR.OT".into(),
                bounds: tore_sim::airport::OrientedBox {
                    center: [x, 30., z],
                    half: [100., 30., 40.],
                    heading: 0.,
                    pitch: 0.,
                    bank: 0.,
                },
                hit_points: 100,
                category: 0x2000,
                radar_signature: 1.,
                infrared_signature: 0.,
                runway: false,
            });
    }

    #[test]
    fn a_ground_start_parks_the_whole_wing_with_the_player_in_front() {
        let world = airfield(6000.);
        let layout = ground_layout(&world, RUNWAY, 5).unwrap();
        assert_eq!((layout.airport, layout.spacing_ft), (9, Some(250.)));
        assert!(!layout.anchored);
        assert_eq!(layout.headings, [0.; 5]);
        let near = 4096. - 3000.;
        let expected = [
            (0., 1100.),
            (40., 850.),
            (-40., 600.),
            (40., 350.),
            (-40., 100.),
        ];
        for (slot, (right, along)) in layout.slots.iter().zip(expected) {
            assert!((slot[0] - 4096. - right).abs() < 1e-6);
            assert!((slot[2] - near - along).abs() < 1e-6);
            assert_eq!(slot[1], 20.);
        }
        assert_eq!(layout.offsets()[3], [40., -750.]);
        let departure = layout.departure();
        assert_eq!(departure.slots, layout.slots);
        assert_eq!(departure.runway.object, RUNWAY);
        // The player stands in the front slot on the researched model.
        let mut player =
            tore_sim::flight::State::new(&crate::flight::animation_tests::profile(), [0.; 3])
                .unwrap();
        assert!(place_on_runway(&world, &mut player, &layout, 0).is_err());
        player.enable_research(1).unwrap();
        place_on_runway(&world, &mut player, &layout, 0).unwrap();
        assert_eq!(
            [player.position[0], player.position[2]],
            [4096., near + 1100.]
        );
        assert!(player.brake_out && player.gear_down && player.speed == 0.);
        // Alone, the player keeps the original single-aircraft start point.
        let alone = ground_layout(&world, RUNWAY, 1).unwrap();
        assert_eq!(alone.slots[0][2], near + 100.);
    }

    #[test]
    fn airport_points_put_the_player_on_the_takeoff_spot_and_wingmen_in_parking() {
        use std::f64::consts::FRAC_PI_2;
        let mut world = airfield(6000.);
        let at = |x: f64, z: f64| [x, 20., z];
        world.airfield_anchors.insert(
            RUNWAY,
            tore_sim::ai::airfield::AirfieldAnchors {
                taxi_out: [
                    at(5000., 1300.),
                    at(4600., 1300.),
                    at(4300., 1300.),
                    at(4096., 1300.),
                ],
                takeoff_spot: at(4096., 1500.),
                takeoff_heading: 0.,
                landing_point: at(4096., 1340.),
                landing_heading: 0.,
                taxi_in: [
                    at(4096., 6000.),
                    at(4400., 6000.),
                    at(4400., 1400.),
                    at(5000., 1400.),
                ],
                parking: std::array::from_fn(|k| at(5000., 1500. + 200. * k as f64)),
                parking_heading: FRAC_PI_2,
            },
        );
        let layout = ground_layout(&world, RUNWAY, 3).unwrap();
        assert!(layout.anchored && layout.spacing_ft.is_none());
        assert_eq!(
            layout.slots,
            [at(4096., 1500.), at(5000., 1500.), at(5000., 1700.)]
        );
        assert_eq!(layout.headings, [0., FRAC_PI_2, FRAC_PI_2]);
        assert!(layout.runway.anchors.is_some());
        assert_eq!(layout.departure().headings, layout.headings);
        assert_eq!(layout.offsets()[1], [904., 0.]);
        let mut wingman =
            tore_sim::flight::State::new(&crate::flight::animation_tests::profile(), [0.; 3])
                .unwrap();
        wingman.enable_research(2).unwrap();
        place_on_runway(&world, &mut wingman, &layout, 1).unwrap();
        assert_eq!(wingman.yaw, FRAC_PI_2);
        // A building on parking slot 1 moves the wingmen along to 2 and 3.
        building(&mut world, 300, 5000., 1500.);
        let layout = ground_layout(&world, RUNWAY, 3).unwrap();
        assert_eq!(layout.slots[1..], [at(5000., 1700.), at(5000., 1900.)]);
        // A building on the takeoff spot falls back to the staggered runway
        // layout, which starts clear of it.
        building(&mut world, 301, 4096., 1500.);
        let layout = ground_layout(&world, RUNWAY, 3).unwrap();
        assert!(!layout.anchored);
        assert_eq!(layout.spacing_ft, Some(250.));
    }

    #[test]
    fn blocked_or_short_runways_tighten_the_wing_then_refuse_it() {
        // A hangar over the 250 ft leader slot moves everyone to 200 ft.
        let mut world = airfield(6000.);
        hangar(&mut world, 200, 1100.);
        let layout = ground_layout(&world, RUNWAY, 5).unwrap();
        assert_eq!(layout.spacing_ft, Some(200.));
        assert!((layout.slots[0][2] - (4096. - 3000.) - 900.).abs() < 1e-6);
        // A hangar on the last aircraft's slot leaves nothing to fall back to.
        hangar(&mut world, 201, 100.);
        let error = ground_layout(&world, RUNWAY, 5).unwrap_err().to_string();
        assert!(
            error.contains("obstructed") && error.contains("wing of 5"),
            "{error}"
        );
        // A 600 ft strip only fits five aircraft at 100 ft spacing.
        let short = airfield(600.);
        assert_eq!(
            ground_layout(&short, RUNWAY, 5).unwrap().spacing_ft,
            Some(100.)
        );
        let error = ground_layout(&airfield(300.), RUNWAY, 5)
            .unwrap_err()
            .to_string();
        assert!(error.contains("too short"), "{error}");
        assert!(ground_layout(&airfield(300.), RUNWAY, 1).is_ok());
    }

    #[test]
    fn the_layout_turns_the_scene_in_the_air_and_only_the_enemy_on_the_ground() {
        let world = airfield(6000.);
        let ground = ground_layout(&world, RUNWAY, 3).unwrap();
        let aim = EnemyAim {
            turn: 0.5,
            distance_ft: 100.5 * FEET_PER_NM,
            requested_ft: 300. * FEET_PER_NM,
        };
        let airborne = MissionLayout {
            ground: None,
            player_turn: aim.turn,
            enemy: aim,
        };
        let plan = airborne.spawn_plan();
        assert_eq!(plan.enemy_turn, 0.);
        assert_eq!(plan.separation_ft, 100.5 * FEET_PER_NM);
        assert!(plan.runway_slots.is_none());
        assert_eq!(
            airborne.notice().unwrap(),
            "Enemy forces start 100 miles away: 300 miles does not fit this theater."
        );
        let parked = MissionLayout {
            ground: Some(ground.clone()),
            player_turn: 0.,
            enemy: EnemyAim::straight(5. * FEET_PER_NM),
        };
        assert!(parked.notice().is_none());
        let parked = MissionLayout {
            enemy: aim,
            ..parked
        };
        let plan = parked.spawn_plan();
        assert_eq!(plan.enemy_turn, 0.5);
        assert_eq!(plan.runway_slots.unwrap(), ground.offsets());
        // Planning from the runway uses the leader's slot and runway heading.
        let start =
            tore_sim::flight::State::new(&crate::flight::animation_tests::profile(), [0.; 3])
                .unwrap();
        let planned = MissionLayout::plan(&world, &start, Some(ground), &[], 1000.);
        assert_eq!(planned.player_turn, 0.);
        assert_eq!(planned.enemy, EnemyAim::straight(1000.));
    }

    #[test]
    fn placeholders_cannot_silently_launch_as_a_supported_mission() {
        let mut q = setup();
        assert!(q.unsupported().is_none());
        assert_eq!(q.dummy_wings().iter().map(|(_, n)| n).sum::<usize>(), 2);
        q.apply(21, 0);
        assert!(q.unsupported().is_none());
        q.apply(6, 2);
        assert!(q.unsupported().unwrap().contains("setup only"));
        q.apply(6, 0);
        // Every editor row maps to its intended source weather, including night.
        assert_eq!(
            q.values(15),
            ["dawn", "clear", "cloudy", "foggy", "sunset", "night"]
        );
        for (value, source) in [3, 0, 1, 2, 4, 5].into_iter().enumerate() {
            assert_eq!(condition(value), Some(source));
            q.apply(15, value);
            assert!(q.unsupported().is_none(), "condition {value}");
        }
        q.apply(15, 6);
        assert!(q.unsupported().unwrap().contains("six available"));
        q.apply(15, 1);
        q.apply(30, 1);
        assert!(q.unsupported().unwrap().contains("Ground"));
    }
}
