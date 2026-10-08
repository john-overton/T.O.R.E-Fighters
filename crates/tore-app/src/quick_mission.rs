//! Retail option values with an explicit editable setup; simulation capabilities
//! are validated separately. Popup frame/focus feedback are authored presentation.
use crate::{
    camera::Camera,
    menu::{Action, Canvas, HEIGHT, Sprite, WIDTH, text_width},
    mission_layout::{FEET_PER_NM, RETAIL_SEPARATIONS, SEPARATION_NM},
    terrain::Terrain,
};
use std::collections::BTreeMap;
use tore_formats::{aircraft::AircraftId, ui::creator::Options};
use tore_session::wire::messages::ItemKind;
use tore_sim::ai::{
    AiError,
    engagement::GroupObjective,
    experience::EnemySkillOverride,
    launch::{Side, WingId, WingLaunch, WingSelection, resolve_wings},
};
use tore_world::mission::{ALTITUDES_FT, Condition, MissionSpec, Skill, Start, WingSpec};
pub mod matrix;
/// The aircraft fields of the five wings the AI flies. Field 6 is friendly
/// wing 1's, which is also the player's own aircraft.
const AI_AIRCRAFT_FIELDS: [usize; 5] = [9, 12, 23, 26, 29];
/// The creator's aircraft list cut down to what the AI can fly.
fn ai_catalog(files: &[String], names: &[String]) -> (Vec<String>, Vec<String>) {
    files
        .iter()
        .zip(names)
        .filter(|(file, _)| AircraftId::parse(file).is_ok_and(AircraftId::ai_flyable))
        .map(|(file, name)| (file.clone(), name.clone()))
        .unzip()
}
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
const GROUND_SECTION: usize = 92;
const GROUND_NOTICE_OK: usize = 93;
const GROUND_SECTION_RECT: Rect = (334, 294, 278, 49);
const GROUND_NOTICE_RECT: Rect = (166, 202, 308, 88);
/// The tint of a list row the lobby's creator dims: what not everyone has.
const GAP_TEXT: [u8; 3] = [104, 110, 112];
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
    /// The debrief is a multiplayer flight's, which has no creator to go
    /// back to: closing it returns to the main menu.
    pub debrief_to_menu: bool,
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
    /// The aircraft the AI can fly (`AircraftId::ai_flyable`): the choices of
    /// the five wing fields that are not the player's (9, 12, 23, 26, 29). A
    /// draft value in those fields indexes this list, not the player's.
    pub wing_names: Vec<String>,
    pub wing_files: Vec<String>,
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
    /// Per theater, the short strips (object id and name) kept off the
    /// ground-start list: no start there for the player or any wing.
    short_strips: Vec<Vec<(u32, String)>>,
    selector: Option<usize>,
    cursor: usize,
    scroll: usize,
    controls: Vec<(usize, Rect)>,
    pub notice: Option<String>,
    ground_notice: bool,
    pub help: bool,
    pub shift: bool,
    /// The creator is open from a multiplayer lobby (EF8): its OK button
    /// reads Accept and sends the mission to the game, Start is locked to
    /// Airborne, and only what a host supports can be accepted.
    pub lobby: bool,
    /// The lobby's items not every player has (stage L, slice L4), each with
    /// the host's words for why it cannot be chosen: aircraft by their
    /// selection key, theaters by their code. Empty outside a lobby.
    gaps: BTreeMap<(ItemKind, String), String>,
}
/// What the creator keeps for the lobby's Cancel to put back: the draft and
/// everything beside it that Accept would send.
#[derive(Clone)]
pub struct Saved {
    draft: Draft,
    group_objectives: [GroupObjective; OBJECTIVE_COUNT],
    group_must_survive: [bool; OBJECTIVE_COUNT],
    ai_mission: crate::ai_wings::Preset,
}
/// Said when the lobby opens the creator, and when the locked Start is
/// touched.
pub const AIRBORNE_NOTICE: &str = "Multiplayer: everyone starts airborne, so Start is locked to Airborne. Accept sends this mission to the lobby.";
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
        // The retail list ends at 50; 100, 150, 200 and 300 are host entries
        // in the retail label style (John, 2026-09-23). Every entry is read as
        // nautical miles, as the manual states.
        options.fields[17].truncate(RETAIL_SEPARATIONS);
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
        let (wing_files, wing_names) = ai_catalog(&aircraft_files, &aircraft_names);
        let wing_selected = wing_files
            .iter()
            .position(|n| n == id.selection_key())
            .unwrap_or(0);
        let mut draft = Draft::default();
        draft.values[6] = selected;
        for i in AI_AIRCRAFT_FIELDS {
            draft.values[i] = wing_selected;
        }
        // Only the sixteen base theaters are offered to the player. The
        // imported `~` layout variants are incomplete (mostly one or two
        // airports); they stay reachable for probes through `--theater ~CODE`
        // (see `choose_theater_code`), but never appear in the list.
        let theater_codes: Vec<String> = source_theaters()
            .iter()
            .map(|code| code.to_string())
            .collect();
        options.fields[13].truncate(16);
        let theater_catalog: Vec<String> = tore_formats::theater::THEATERS
            .iter()
            .map(|(code, _)| code.to_string())
            .collect();
        let mut airport_names = Vec::new();
        let mut airport_objects = Vec::new();
        let mut short_strips = Vec::new();
        let mut definitions = BTreeMap::new();
        for code in &theater_codes {
            let (names, ids, short) = Self::airports_in(code, data, &mut definitions);
            airport_names.push(names);
            airport_objects.push(ids);
            short_strips.push(short);
        }
        Self {
            debrief: None,
            debrief_to_menu: false,
            ordnance: None,
            start_modes: vec!["Airborne".into(), "Ground".into()],
            airport_names,
            airport_objects,
            short_strips,
            hover: None,
            pressed: None,
            right_pressed: None,
            rocker: Default::default(),
            focus: 6,
            selection: 0,
            aircraft_selection: selected,
            aircraft_names,
            aircraft_files,
            wing_names,
            wing_files,
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
            ground_notice: false,
            help: false,
            shift: false,
            lobby: false,
            gaps: BTreeMap::new(),
        }
    }
    /// The ground-start airports of one theater layout (names and object ids),
    /// and the short strips left off that list. `fitted`, agent decision
    /// 2026-09-30, for John's decision of the same day: an airport whose runway
    /// is under `tore_sim::airport::SHORT_STRIP_FT` is no ground start, for the
    /// player or any wing. `definitions` caches, per object type, whether it is
    /// an airport and its runway length.
    fn airports_in(
        code: &str,
        data: &BTreeMap<String, Vec<u8>>,
        definitions: &mut BTreeMap<String, (bool, Option<f64>)>,
    ) -> (Vec<String>, Vec<u32>, Vec<(u32, String)>) {
        let name = format!("{code}.MM");
        let mut names = Vec::new();
        let mut ids = Vec::new();
        let mut short = Vec::new();
        if let Some(bytes) = data.get(&name)
            && let Ok(layout) = tore_formats::mission::Layout::parse(&name, bytes)
        {
            for p in layout.placements {
                let (airport, length) =
                    *definitions.entry(p.object_type.clone()).or_insert_with(|| {
                        let airport = data
                            .get(&p.object_type)
                            .and_then(|b| tore_formats::static_object::Definition::parse(b).ok())
                            .is_some_and(|d| {
                                d.main_shape.is_some()
                                    && d.callbacks.iter().any(|c| c == "_STRIPProc")
                            });
                        let length = airport
                            .then(|| tore_world::terrain::strip_length_ft(data, &p.object_type))
                            .flatten();
                        (airport, length)
                    });
                if airport {
                    let id = 0x4000_0000 + p.key.ordinal;
                    let label = p.name.unwrap_or(p.object_type);
                    // A strip whose shape cannot be measured stays listed.
                    if length.is_some_and(tore_sim::airport::short_strip_length) {
                        short.push((id, label));
                    } else {
                        names.push(label);
                        ids.push(id);
                    }
                }
            }
        }
        (names, ids, short)
    }
    /// Developer option: add one imported `~` layout variant (or, with
    /// `all`, every one) after the base theaters so a probe can run the
    /// creator on it. The player's theater list never contains them.
    pub fn add_developer_theaters(&mut self, data: &BTreeMap<String, Vec<u8>>, only: Option<&str>) {
        let catalog = tore_formats::theater::map_catalog(data).unwrap_or_default();
        let mut definitions = BTreeMap::new();
        for (code, label) in catalog
            .into_iter()
            .filter(|(code, _)| code.starts_with('~'))
        {
            if only.is_some_and(|only| only != code) || self.theater_codes.contains(&code) {
                continue;
            }
            let (names, ids, short) = Self::airports_in(&code, data, &mut definitions);
            self.airport_names.push(names);
            self.airport_objects.push(ids);
            self.short_strips.push(short);
            self.theater_codes.push(code.clone());
            self.theater_catalog.push(code);
            self.options.fields[13].push(label);
        }
    }
    /// Select a theater by code, as `--theater CODE` does. A `~` variant is
    /// added on demand (developer option); an unknown code selects the first
    /// theater.
    pub fn choose_theater_code(&mut self, code: &str, data: &BTreeMap<String, Vec<u8>>) {
        if code.starts_with('~') {
            self.add_developer_theaters(data, Some(code));
        }
        let index = self
            .theater_catalog
            .iter()
            .position(|c| c == code)
            .unwrap_or(0);
        self.theater(index);
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
    /// Every imported string the creator can show, with where it comes from,
    /// for the `--validate-text` scan.
    pub fn imported_strings(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (id, list) in self.options.fields.iter().enumerate() {
            for text in list {
                out.push((format!("creator field {id}"), text.clone()));
            }
        }
        for text in &self.aircraft_names {
            out.push(("creator aircraft".into(), text.clone()));
        }
        for (theater, names) in self.theater_codes.iter().zip(&self.airport_names) {
            for text in names {
                out.push((format!("airport in {theater}"), text.clone()));
            }
        }
        for (id, list) in self.options.targets.iter().enumerate() {
            for text in list {
                out.push((format!("ground target list {id}"), text.clone()));
            }
        }
        out
    }
    pub fn player(&self) -> Option<AircraftId> {
        self.aircraft_files
            .get(self.draft.values[6])
            .and_then(|n| AircraftId::parse(n).ok())
    }
    /// Rebuilds the AI's aircraft list from the player's.
    #[cfg(test)]
    fn refresh_wing_catalog(&mut self) {
        (self.wing_files, self.wing_names) = ai_catalog(&self.aircraft_files, &self.aircraft_names);
    }
    /// The aircraft the draft chose in an aircraft field (6, 9, 12, 23, 26 or
    /// 29), if it resolves to an import.
    fn wing_aircraft(&self, field: usize) -> Option<AircraftId> {
        let files = if field == 6 {
            &self.aircraft_files
        } else {
            &self.wing_files
        };
        files
            .get(self.draft.values[field])
            .and_then(|n| AircraftId::parse(n).ok())
    }
    /// The value that picks `id` in one of the AI's wing fields (9, 12, 23,
    /// 26 and 29), or `None` when the AI cannot fly it or it is not imported.
    pub fn ai_choice(&self, id: AircraftId) -> Option<usize> {
        self.wing_files.iter().position(|n| n == id.selection_key())
    }
    /// Whether the player's aircraft is one the AI cannot fly, so no AI
    /// wingman can share friendly wing 1 with the player.
    fn player_without_ai_wingmen(&self) -> Option<AircraftId> {
        self.player().filter(|id| !id.ai_flyable())
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
        if id == 6 {
            &self.aircraft_names
        } else if AI_AIRCRAFT_FIELDS.contains(&id) {
            &self.wing_names
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
    pub(crate) fn objective_choices(group: usize) -> Vec<(String, GroupObjective)> {
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
            let Some(aircraft) = self.wing_aircraft(field + 2) else {
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
    /// The creator's draft as a mission with stable names: the theater code,
    /// the condition, the start, the six wings by their aircraft keys, the
    /// standing orders and the groups' objectives. What belongs to the
    /// session rather than the creator (the flight models, the enemy skill
    /// override, the weather overrides, the cheats, the loadout) keeps the
    /// spec's defaults for the caller to set.
    pub fn mission_spec(&self) -> Result<MissionSpec, String> {
        let v = &self.draft.values;
        let player = self.player().ok_or("Choose an imported aircraft to fly.")?;
        let theater = self
            .theater_codes
            .get(v[13])
            .ok_or("Choose a theater.")?
            .clone();
        let mut spec = MissionSpec::new(&theater, player);
        spec.condition = condition(v[15])
            .and_then(Condition::from_index)
            .ok_or("Choose one of the six available weather conditions.")?;
        let altitude_ft = *ALTITUDES_FT
            .get(v[14])
            .ok_or("Choose one of the four altitudes.")?;
        spec.start = match self.ground_runway() {
            Some(runway) => Start::Ground {
                runway,
                altitude_ft,
            },
            None => Start::Airborne { altitude_ft },
        };
        spec.separation_nm = self.separation_nm() as u32;
        spec.preset = self.ai_mission;
        spec.guns_only = self.guns_only();
        for (index, field) in [4, 7, 10, 21, 24, 27].into_iter().enumerate() {
            // A wing whose aircraft choice does not resolve keeps the
            // player's type; `unsupported` refuses to fly one with aircraft.
            let aircraft = self.wing_aircraft(field + 2).unwrap_or(player);
            spec.wings[index] = WingSpec {
                aircraft,
                count: v[field].max(usize::from(index == 0)),
                skill: Skill::from_level(v[field + 1] as i32).ok_or("Choose a pilot skill.")?,
            };
        }
        spec.objectives = self.group_objectives;
        spec.must_survive = self.group_must_survive;
        Ok(spec)
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
    #[cfg(test)]
    pub fn dummy_wings(&self) -> Vec<(AircraftId, usize)> {
        self.wing_launches(None)
            .map(|wings| tore_sim::ai::launch::legacy_pairs(&wings))
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
        if let Some((_, name)) = self
            .short_strips
            .get(self.draft.values[13])
            .and_then(|list| list.iter().find(|(id, _)| *id == object))
        {
            return Err(format!(
                "{name} is a short strip: no ground start there. Choose a longer runway or Airborne."
            ));
        }
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
            if v[field] > 0 && self.wing_aircraft(field + 2).is_none() {
                return Some(
                    "Choose a supported imported aircraft for every populated wing.".into(),
                );
            }
        }
        if !self.lobby
            && v[4] > 1
            && let Some(player) = self.player_without_ai_wingmen()
        {
            return Some(format!(
                "The AI cannot fly the {} yet, so friendly wing 1 can only hold you. Set friendly wing 1 to one aircraft.",
                player.label()
            ));
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
    /// A sample of gaps for the previews: the second and fourth aircraft and
    /// the third theater, each as Hawk's game lacking it.
    fn preview_gaps(&mut self) {
        let mut gaps = BTreeMap::new();
        for index in [1, 3] {
            if let (Some(key), Some(label)) = (
                self.aircraft_files.get(index),
                self.aircraft_names.get(index),
            ) {
                gaps.insert(
                    (ItemKind::Aircraft, key.clone()),
                    format!("Not everyone can fly the {label}: Hawk's game has no {label}."),
                );
            }
        }
        if let Some(code) = self.theater_codes.get(2) {
            gaps.insert(
                (ItemKind::Theater, code.clone()),
                format!("Not everyone has {code}: Hawk's game has no {code}."),
            );
        }
        self.gaps = gaps;
    }
    /// What the lobby's Cancel puts back.
    pub fn save(&self) -> Saved {
        Saved {
            draft: self.draft.clone(),
            group_objectives: self.group_objectives,
            group_must_survive: self.group_must_survive,
            ai_mission: self.ai_mission,
        }
    }
    pub fn restore(&mut self, saved: Saved) {
        self.draft = saved.draft;
        self.group_objectives = saved.group_objectives;
        self.group_must_survive = saved.group_must_survive;
        self.ai_mission = saved.ai_mission;
        self.aircraft_selection = self.draft.values[6];
        self.selection = self.theater_index();
    }
    /// Opens the creator for a multiplayer lobby (EF8): Start becomes
    /// Airborne and stays there (John, 2026-09-28: everyone starts airborne),
    /// and the notice says so.
    pub fn enter_lobby(&mut self) {
        self.lobby = true;
        self.cancel();
        if self.draft.values[33] != 0 {
            self.apply(33, 0);
        }
        self.focus = 3;
        self.notice = Some(AIRBORNE_NOTICE.into());
    }
    /// Closes the lobby's creator.
    pub fn leave_lobby(&mut self) {
        self.lobby = false;
        self.cancel();
        self.notice = None;
        self.gaps.clear();
    }
    /// Takes the items not every player has (stage L, slice L4): `refusal`
    /// gives the host's words for an item of a kind and key when it is in a
    /// gap now (`Client::gap_refusal`). The creator asks it about each
    /// aircraft and theater it offers; the list dims those and a choice of
    /// one says why it cannot be made. Only the lobby's creator uses it.
    pub fn set_gaps(&mut self, refusal: impl Fn(ItemKind, &str) -> Option<String>) {
        let mut gaps = BTreeMap::new();
        for key in &self.aircraft_files {
            if let Some(words) = refusal(ItemKind::Aircraft, key) {
                gaps.insert((ItemKind::Aircraft, key.clone()), words);
            }
        }
        for code in self.theater_codes.iter().filter(|c| !c.starts_with('~')) {
            if let Some(words) = refusal(ItemKind::Theater, code) {
                gaps.insert((ItemKind::Theater, code.clone()), words);
            }
        }
        self.gaps = gaps;
    }
    /// Why the choice of value `index` in `field` cannot be made, in the
    /// host's words, when the creator is the lobby's and the aircraft or
    /// theater is in a gap.
    fn gap_in(&self, field: usize, index: usize) -> Option<&str> {
        if !self.lobby {
            return None;
        }
        let (kind, key) = if field == 6 {
            (ItemKind::Aircraft, self.aircraft_files.get(index)?)
        } else if AI_AIRCRAFT_FIELDS.contains(&field) {
            (ItemKind::Aircraft, self.wing_files.get(index)?)
        } else if field == 13 {
            (ItemKind::Theater, self.theater_codes.get(index)?)
        } else {
            return None;
        };
        self.gaps.get(&(kind, key.clone())).map(String::as_str)
    }
    /// A choice in a gap leaves the draft as it was and puts the host's
    /// words in the notice. True when it was refused.
    fn refuse_gap(&mut self, field: usize, index: usize) -> bool {
        let Some(words) = self.gap_in(field, index).map(str::to_owned) else {
            return false;
        };
        self.notice = Some(words);
        true
    }
    /// What the host would refuse in the draft for a gap: the first of the
    /// player's aircraft, each populated wing's and the theater that is in
    /// one, in the host's words.
    fn gap_problem(&self) -> Option<String> {
        let v = &self.draft.values;
        let wings = [4, 7, 10, 21, 24, 27]
            .into_iter()
            .filter(|field| v[*field] > 0 || *field == 4)
            .map(|field| (field + 2, v[field + 2]));
        std::iter::once((6, v[6]))
            .chain(wings)
            .chain(std::iter::once((13, v[13])))
            .find_map(|(field, index)| self.gap_in(field, index))
            .map(str::to_owned)
    }
    /// What a host will not take from the creator, in words (EF8): what
    /// single player cannot fly either, and a developer theater layout,
    /// which the mission's text form cannot name.
    pub fn lobby_problem(&self) -> Option<String> {
        if let Some(problem) = self.unsupported() {
            return Some(problem);
        }
        if self
            .theater_codes
            .get(self.draft.values[13])
            .is_some_and(|code| code.starts_with('~'))
        {
            return Some(
                "A developer theater layout cannot be hosted. Choose one of the sixteen theaters."
                    .into(),
            );
        }
        self.gap_problem()
    }
    /// The mission a host takes from this draft: [`QuickMission::mission_spec`]
    /// starting airborne, with what the lobby refuses reported first.
    pub fn lobby_spec(&self) -> Result<MissionSpec, String> {
        if let Some(problem) = self.lobby_problem() {
            return Err(problem);
        }
        let mut spec = self.mission_spec()?;
        if let Start::Ground { altitude_ft, .. } = spec.start {
            spec.start = Start::Airborne { altitude_ft };
        }
        // The host reads the mission as text: what the text cannot carry
        // would silently change on the way.
        match MissionSpec::from_text(&spec.to_text()) {
            Ok(read) if read == spec => Ok(spec),
            Ok(_) => Err("This mission cannot be sent to a host as it is.".into()),
            Err(error) => Err(error.to_string()),
        }
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
        // The AI cannot fly the player's aircraft yet: friendly wing 1 holds
        // just the player (a lobby's wing 1 is for humans, so it keeps its size).
        if !self.lobby
            && self.draft.values[4] > 1
            && let Some(player) = self.player_without_ai_wingmen()
        {
            self.draft.values[4] = 1;
            self.notice = Some(format!(
                "The AI cannot fly the {} yet, so friendly wing 1 is just you.",
                player.label()
            ));
        }
    }
    fn show_ground_notice(&mut self) {
        self.cancel();
        self.notice = None;
        self.ground_notice = true;
    }
    fn open(&mut self, id: usize) {
        if (30..=32).contains(&id) {
            self.show_ground_notice();
            return;
        }
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
            // The creator opened from a multiplayer lobby (EF8), and after the
            // host refused an Accept.
            "lobby-creator" => self.enter_lobby(),
            "lobby-creator-refused" => {
                self.enter_lobby();
                self.notice = Some(
                    "The mission could not be built: Choose a theater with enough room for the enemy separation."
                        .into(),
                );
            }
            // The lobby's creator with items not every player has (stage L,
            // slice L4): the aircraft list dimmed, and the notice a choice of
            // one leaves.
            "lobby-creator-gaps" | "lobby-creator-gap-notice" => {
                self.enter_lobby();
                self.preview_gaps();
                self.open(6);
                if name == "lobby-creator-gap-notice" {
                    self.cursor = (0..self.aircraft_files.len())
                        .find(|i| self.gap_in(6, *i).is_some())
                        .unwrap_or(0);
                    self.scroll = self.cursor / ROWS * ROWS;
                    self.activate(POP_OK);
                }
            }
            "lobby-creator-gap-theaters" => {
                self.enter_lobby();
                self.preview_gaps();
                self.open(13);
            }
            "lobby-ordnance" | "lobby-ordnance-refused" | "lobby-ordnance-cheat" => {}
            "lobby-ordnance-gaps" => {
                if let Some(ordnance) = &mut self.ordnance {
                    ordnance.preview(name);
                }
            }
            "ordnance-tanks"
            | "ordnance-empty"
            | "ordnance-drag"
            | "ordnance-message"
            | "ordnance-message-long" => {
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
            "ground-targets-unavailable" => self.show_ground_notice(),
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
                let id=name.strip_prefix("field-").and_then(|v|v.parse::<usize>().ok()).filter(|v|(3..35).contains(v)).ok_or("snapshot states: normal, aircraft, objectives, ground-start, airports, objective-1 through objective-6, theaters, help, field-3 through field-34")?;
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
        if self.selector.is_some() || self.help || self.ground_notice {
            self.right_pressed = None;
            return Action::None;
        }
        if down {
            self.pressed = None;
            self.right_pressed = self.hover.filter(|id| {
                (3..=34).contains(id)
                    || *id == GROUND_SECTION
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
        if matches!(id, 30..=32 | GROUND_SECTION) {
            self.show_ground_notice();
            return Action::Click;
        }
        if self.lobby && matches!(id, 33 | 34) {
            self.focus = id;
            self.notice = Some(AIRBORNE_NOTICE.into());
            return Action::Click;
        }
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
        if !self.refuse_gap(id, previous) {
            self.apply(id, previous);
        }
        Action::Click
    }
    /// Leaves the debrief for the creator, keeping the mission just flown, or
    /// for the main menu after a multiplayer flight.
    fn close_debrief(&mut self) -> Action {
        self.debrief = None;
        self.hover = None;
        self.pressed = None;
        if std::mem::take(&mut self.debrief_to_menu) {
            return Action::Back;
        }
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
        self.ground_notice = false;
    }
    fn activate(&mut self, id: usize) -> Action {
        if self.ground_notice {
            return if id == GROUND_NOTICE_OK {
                self.cancel();
                Action::Click
            } else {
                Action::None
            };
        }
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
                    } else if self.gap_in(field, self.cursor).is_some() {
                        // Not everyone has it (stage L): the choice stays as
                        // it was and the notice says who lacks it.
                        self.refuse_gap(field, self.cursor);
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
            30..=32 | GROUND_SECTION => self.show_ground_notice(),
            33 | 34 if self.lobby => {
                self.focus = id;
                self.notice = Some(AIRBORNE_NOTICE.into());
            }
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
                        let next = (self.draft.values[id] + 1) % n;
                        if !self.refuse_gap(id, next) {
                            self.apply(id, next);
                        }
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
        if self.ground_notice {
            return if matches!(key, "Enter" | " " | "Escape") {
                self.activate(GROUND_NOTICE_OK)
            } else {
                Action::None
            };
        }
        if key == "Escape" {
            // The lobby's notice is not in the way of Cancel (EF8).
            if self.selector.is_some() || self.help || (self.notice.is_some() && !self.lobby) {
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
        _world: &Terrain,
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
        self.controls.push((GROUND_SECTION, GROUND_SECTION_RECT));
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
        self.button(
            &mut c,
            sprites,
            OK,
            if self.lobby { "Accept" } else { "OK" },
            (387, 419, 85, 24),
        );
        self.button(&mut c, sprites, CANCEL, "Cancel", (492, 419, 85, 24));
        if let Some(message) = &self.notice {
            if self.lobby {
                lobby_notice(&mut c, &sprites["SMLFONT.PIC"], message);
            } else {
                notice(&mut c, &sprites["SMLFONT.PIC"], message);
            }
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
                // What not every player has is dimmed in the lobby's creator.
                let dim = self.gap_in(field, self.scroll + row).map(|_| GAP_TEXT);
                c.text(font, &fit(font, text, 208), 223, y + 2, dim);
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
        if self.ground_notice {
            self.controls.clear();
            let (x, y, w, h) = GROUND_NOTICE_RECT;
            c.rect((x + 4, y + 4, w, h), [16, 19, 20, 255]);
            c.rect(GROUND_NOTICE_RECT, [35, 44, 46, 255]);
            bevel(&mut c, GROUND_NOTICE_RECT, false);
            for (text, y) in [
                ("Ground targets, AAA and SAMs", 214),
                ("are not implemented yet.", 231),
            ] {
                c.centered_text(font, text, (174, y, 292, 13));
            }
            self.button(&mut c, sprites, GROUND_NOTICE_OK, "OK", (285, 255, 85, 24));
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
            matches!(id, OK | POP_OK | GROUND_NOTICE_OK),
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
    tore_world::mission::THEATERS
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
/// The lobby creator's notice (EF8): a box at the lower left, beside the
/// buttons, of up to three lines, so it does not cover the Start line that
/// the notice is about. A longer message ends in an ellipsis.
fn lobby_notice(c: &mut Canvas, font: &Sprite, text: &str) {
    const WIDTH: i32 = 340;
    const LINES: usize = 3;
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let next = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if text_width(font, &next) > WIDTH - 12 && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
            line = word.to_owned();
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > LINES {
        lines.truncate(LINES);
        let last = lines[LINES - 1].clone();
        lines[LINES - 1] = fit(font, &format!("{last} ..."), WIDTH - 12);
    }
    let height = 6 + 13 * lines.len() as i32;
    c.rect((30, 438 - height, WIDTH, height), [35, 44, 46, 255]);
    for (i, line) in lines.iter().enumerate() {
        c.text(
            font,
            line,
            36,
            438 - height + 4 + 13 * i as i32,
            Some([235, 225, 179]),
        );
    }
}
pub fn hud(
    pixels: &mut [u8],
    sprites: &BTreeMap<String, Sprite>,
    camera: &Camera,
    world: &Terrain,
) {
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
    use tore_sim::ai::launch::legacy_pairs;
    #[test]
    fn a_multiplayer_debrief_returns_to_the_menu_and_a_single_player_one_to_the_creator() {
        let mut q = setup();
        assert_eq!(q.close_debrief(), Action::Click);
        q.debrief_to_menu = true;
        assert_eq!(q.close_debrief(), Action::Back);
        // Once only: the next debrief is a single-player one again.
        assert_eq!(q.close_debrief(), Action::Click);
    }
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
    #[test]
    fn the_player_list_has_only_base_theaters_and_a_variant_is_a_developer_addition() {
        let mut q = setup();
        assert_eq!(q.options.fields[13].len(), 16);
        assert!(q.theater_codes.iter().all(|c| !c.starts_with('~')));
        // No imported layouts in this fixture, so nothing is added; an
        // unknown code falls back to the first theater.
        q.choose_theater_code("~NOPE", &BTreeMap::new());
        assert_eq!(q.theater_index(), 0);
        assert_eq!(q.options.fields[13].len(), 16);
        q.choose_theater_code("UKR", &BTreeMap::new());
        assert_eq!(q.theater_codes[q.draft.values[13]], "UKR");
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
        q.refresh_wing_catalog();
        q
    }
    /// A creator offering every selectable aircraft.
    fn full_catalog() -> QuickMission {
        let mut q = setup();
        q.aircraft_files = AircraftId::SELECTABLE
            .map(|id| id.selection_key().to_string())
            .to_vec();
        q.aircraft_names = AircraftId::SELECTABLE
            .map(|id| id.label().to_string())
            .to_vec();
        q.refresh_wing_catalog();
        q.draft.values[6] = 0;
        for field in AI_AIRCRAFT_FIELDS {
            q.draft.values[field] = 0;
        }
        q
    }
    #[test]
    fn the_ais_wing_choices_leave_out_what_it_cannot_fly_but_the_player_keeps_them() {
        let q = full_catalog();
        let kept: Vec<AircraftId> = AircraftId::SELECTABLE
            .into_iter()
            .filter(|id| !id.ai_flyable())
            .collect();
        assert_eq!(kept.len(), 6);
        for field in AI_AIRCRAFT_FIELDS {
            assert_eq!(q.values(field).len(), AircraftId::SELECTABLE.len() - 6);
            for id in &kept {
                assert!(
                    !q.values(field).contains(&id.label().to_string()),
                    "{id:?} in field {field}"
                );
            }
        }
        assert!(q.ai_choice(AircraftId::Ah64).is_none());
        assert!(q.ai_choice(AircraftId::Mig29).is_some());
        // The player's list is whole.
        assert_eq!(q.values(6).len(), AircraftId::SELECTABLE.len());
        for id in kept {
            assert!(q.values(6).contains(&id.label().to_string()));
        }
    }
    #[test]
    fn a_wing_field_names_the_aircraft_of_the_ais_list_not_the_players() {
        let mut q = full_catalog();
        for field in [9, 12, 23, 26, 29] {
            let index = q.ai_choice(AircraftId::Mig17).unwrap();
            q.apply(field, index);
            assert_eq!(q.wing_aircraft(field), Some(AircraftId::Mig17));
        }
        // The same number in the player's field is a different aircraft.
        let index = q.ai_choice(AircraftId::Mig17).unwrap();
        q.apply(6, index);
        assert_ne!(q.player(), Some(AircraftId::Mig17));
        let spec = q.mission_spec().unwrap();
        assert_eq!(spec.wings[2].aircraft, AircraftId::Mig17);
        assert_eq!(spec.wings[5].aircraft, AircraftId::Mig17);
    }
    #[test]
    fn a_player_in_an_aircraft_the_ai_cannot_fly_leads_friendly_wing_1_alone() {
        let mut q = full_catalog();
        q.apply(4, 3);
        for id in [
            AircraftId::Ah64,
            AircraftId::Mi24,
            AircraftId::Ch47,
            AircraftId::V22,
            AircraftId::Av8,
            AircraftId::Yak141,
        ] {
            q.apply(6, 0);
            q.apply(4, 3);
            let index = q
                .aircraft_files
                .iter()
                .position(|f| f == id.selection_key())
                .unwrap();
            q.apply(6, index);
            assert_eq!(q.player(), Some(id));
            assert_eq!(q.draft.values[4], 1, "{id:?}");
            assert!(q.notice.as_deref().is_some_and(|n| n.contains("just you")));
            assert!(q.unsupported().is_none());
            // The count cannot be raised again while they fly it.
            q.apply(4, 2);
            assert_eq!(q.draft.values[4], 1);
            assert_eq!(q.wing_launches(None).unwrap()[0].count(), 0);
        }
        // A draft that slipped through is refused with the reason.
        q.draft.values[4] = 2;
        assert!(q.unsupported().unwrap().contains("cannot fly the"));
        // The lobby's wing 1 is for people, so it keeps its size.
        q.lobby = true;
        assert!(q.unsupported().is_none());
        q.apply(4, 3);
        assert_eq!(q.draft.values[4], 3);
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
        q.cancel();
        for field in [30, 31, 32, GROUND_SECTION] {
            // Both pointer buttons show the notice without editing the draft.
            q.hover = Some(field);
            q.down();
            assert!(matches!(q.up(), Action::Click));
            assert!(q.ground_notice);
            assert!(q.selector.is_none());
            assert!(matches!(q.activate(OK), Action::None));
            assert!(matches!(right_click(&mut q, 15), Action::None));
            q.key("Escape", false);
            assert!(matches!(right_click(&mut q, field), Action::Click));
            assert!(q.ground_notice);
            q.key("Enter", false);
            assert!(!q.ground_notice);
            assert_eq!(q.draft.values, before);
        }
        for field in 30..=32 {
            q.focus = field;
            for shift in [false, true] {
                q.key("Enter", shift);
                assert!(q.ground_notice);
                assert!(q.selector.is_none());
                q.activate(GROUND_NOTICE_OK);
                assert_eq!(q.draft.values, before);
            }
            q.open(field); // Direct selector previews must not bypass the guard.
            assert!(q.ground_notice && q.selector.is_none());
            q.cancel();
        }
        // The sentence background, away from its value boxes, is also clickable.
        q.controls = vec![(GROUND_SECTION, GROUND_SECTION_RECT)];
        q.pointer(Some((340., 298.)));
        q.down();
        q.up();
        assert!(q.ground_notice);
        q.key(" ", false);
        q.activate(15);
        assert_ne!(q.draft.values[15], before[15]);
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
    fn the_lobbys_creator_locks_start_to_airborne_and_accepts_only_what_a_host_takes() {
        let mut q = setup();
        q.airport_names[0] = vec!["First Field".into()];
        q.airport_objects[0] = vec![0x40000000];
        q.theater_codes = vec!["UKR".into(), "~UKR1".into()];
        // Single player keeps its own Start and its OK.
        q.activate(33);
        assert!(q.ground_start() && !q.lobby);
        // The lobby takes a ground draft back to Airborne, and says why.
        let saved = q.save();
        q.enter_lobby();
        assert!(q.lobby && !q.ground_start());
        assert_eq!(q.notice.as_deref(), Some(AIRBORNE_NOTICE));
        // Touching Start (left or right click) changes nothing and says why.
        q.notice = None;
        assert_eq!(q.activate(33), Action::Click);
        assert!(!q.ground_start());
        assert_eq!(q.notice.as_deref(), Some(AIRBORNE_NOTICE));
        q.notice = None;
        q.hover = Some(34);
        q.right(true);
        q.right(false);
        assert!(!q.ground_start());
        assert_eq!(q.notice.as_deref(), Some(AIRBORNE_NOTICE));
        // The mission it makes starts airborne, and a developer layout is
        // refused before Accept, with the ground targets.
        let spec = q.lobby_spec().expect("a mission");
        assert!(matches!(spec.start, Start::Airborne { .. }));
        q.apply(13, 1);
        assert!(q.lobby_problem().unwrap().contains("developer theater"));
        assert!(q.lobby_spec().is_err());
        q.apply(13, 0);
        q.draft.values[30] = 1;
        assert!(q.lobby_problem().unwrap().contains("Ground targets"));
        q.draft.values[30] = 0;
        // OK is Accept: it still answers Mission, which the lobby takes.
        assert_eq!(q.activate(OK), Action::Mission);
        // Esc leaves with the notice up, and Cancel puts the draft back.
        assert_eq!(q.key("Escape", false), Action::Back);
        q.apply(4, 3);
        q.restore(saved);
        assert!(q.ground_start());
        assert_ne!(q.draft.values[4], 3);
        q.leave_lobby();
        assert!(!q.lobby && q.notice.is_none());
    }
    /// The words the host would give for the Rafale and for a theater.
    fn some_gaps(q: &mut QuickMission) {
        let theater = q.theater_codes[1].clone();
        q.set_gaps(|kind, key| match (kind, key) {
            (ItemKind::Aircraft, "RAFALE.PT") => {
                Some("Not everyone can fly the Rafale: Hawk's game has no Rafale.".into())
            }
            (ItemKind::Theater, code) if code == theater => Some(format!(
                "Not everyone has {code}: Hawk's game has no {code}."
            )),
            _ => None,
        });
    }
    #[test]
    fn the_lobbys_creator_refuses_a_choice_in_a_gap_and_keeps_the_old_one() {
        let mut q = setup();
        // Single player knows no gaps: the same list and clicks choose it.
        some_gaps(&mut q);
        assert!(q.gap_in(6, 1).is_none());
        q.open(6);
        q.cursor = 1;
        q.activate(POP_OK);
        assert_eq!(q.draft.values[6], 1);
        assert!(q.notice.is_none());
        q.apply(6, 0);

        q.enter_lobby();
        some_gaps(&mut q);
        // Rafale is the second aircraft, in a gap, for every aircraft field
        // and for the theater at its own index.
        for field in [6, 9, 12, 23, 26, 29] {
            assert!(q.gap_in(field, 1).is_some(), "{field}");
            assert!(q.gap_in(field, 0).is_none() && q.gap_in(field, 2).is_none());
        }
        assert!(q.gap_in(13, 1).is_some() && q.gap_in(13, 0).is_none());
        assert!(q.gap_in(15, 1).is_none(), "other fields have no gaps");
        // Choosing it in the list leaves the choice as it was, closes the
        // list and says who lacks it.
        q.open(6);
        q.cursor = 1;
        q.activate(POP_OK);
        assert_eq!(q.draft.values[6], 0);
        assert_eq!(q.aircraft_selection, 0);
        assert!(q.selector.is_none());
        assert_eq!(
            q.notice.as_deref(),
            Some("Not everyone can fly the Rafale: Hawk's game has no Rafale.")
        );
        // What everyone has is chosen as before, and the notice clears.
        q.open(6);
        q.cursor = 2;
        q.activate(POP_OK);
        assert_eq!(q.draft.values[6], 2);
        assert!(q.notice.is_none());
        // The shifted click that steps to the next value, and the right
        // click that steps back, refuse the same way.
        q.apply(9, 0);
        q.shift = true;
        q.focus = 9;
        q.activate(9);
        assert_eq!(q.draft.values[9], 0);
        assert!(q.notice.as_deref().unwrap().contains("Rafale"));
        q.notice = None;
        q.apply(9, 2);
        right_click(&mut q, 9);
        assert_eq!(q.draft.values[9], 2);
        assert!(q.notice.as_deref().unwrap().contains("Rafale"));
        // A theater in a gap is refused in its list too.
        q.notice = None;
        q.open(13);
        q.cursor = 1;
        q.activate(POP_OK);
        assert_eq!(q.draft.values[13], 0);
        assert!(q.notice.as_deref().unwrap().starts_with("Not everyone has"));
        // Leaving the lobby's creator forgets them.
        q.leave_lobby();
        assert!(q.gaps.is_empty() && q.gap_in(6, 1).is_none());
    }
    #[test]
    fn accept_checks_the_whole_mission_for_gaps_and_ignores_a_wing_nobody_flies() {
        let mut q = setup();
        q.enter_lobby();
        some_gaps(&mut q);
        assert!(q.lobby_problem().is_none());
        // The player's own aircraft.
        q.draft.values[6] = 1;
        assert!(q.lobby_problem().unwrap().contains("Rafale"));
        assert!(q.lobby_spec().is_err());
        q.draft.values[6] = 0;
        // A wing that flies, and a wing with no aircraft in it (count 0),
        // which the host does not count either.
        q.draft.values[9] = 1;
        assert_eq!(q.draft.values[7], 0);
        assert!(q.lobby_problem().is_none());
        q.draft.values[23] = 1;
        assert!(q.draft.values[21] > 0);
        assert!(q.lobby_problem().unwrap().contains("Rafale"));
        q.draft.values[23] = 0;
        // The theater.
        q.draft.values[13] = 1;
        assert!(q.lobby_problem().unwrap().starts_with("Not everyone has"));
        q.draft.values[13] = 0;
        assert!(q.lobby_spec().is_ok());
        // OK is Accept and reaches the lobby only with a clean draft; the
        // refusal is the host's own line in the notice.
        q.draft.values[6] = 1;
        assert_eq!(q.activate(OK), Action::Mission);
        assert!(q.lobby_spec().is_err());
        // Single player's creator never asks.
        q.leave_lobby();
        assert!(q.lobby_problem().is_none());
        assert!(q.lobby_spec().is_ok());
    }
    #[test]
    fn the_creators_gap_words_come_from_the_clients_refusals_for_what_it_lists() {
        let mut q = setup();
        q.enter_lobby();
        let mut asked = Vec::new();
        let asked_cell = std::cell::RefCell::new(&mut asked);
        q.set_gaps(|kind, key| {
            asked_cell.borrow_mut().push((kind, key.to_owned()));
            None
        });
        // Every aircraft and every base theater, nothing else.
        assert_eq!(
            asked
                .iter()
                .filter(|(kind, _)| *kind == ItemKind::Aircraft)
                .count(),
            q.aircraft_files.len()
        );
        assert_eq!(
            asked
                .iter()
                .filter(|(kind, _)| *kind == ItemKind::Theater)
                .count(),
            q.theater_codes.len()
        );
        assert!(asked.iter().all(|(kind, _)| *kind != ItemKind::Weapon));
        assert!(q.gaps.is_empty());
    }
    #[test]
    fn a_short_strip_is_no_ground_start() {
        let mut q = setup();
        q.airport_names[0] = vec!["Long Field".into()];
        q.airport_objects[0] = vec![0x4000_0000];
        q.short_strips[0] = vec![(0x4000_0003, "Goose Green".into())];
        // The short strip is not in the list the creator offers.
        assert_eq!(q.values(34).len(), 1);
        let error = q.choose_ground_runway(0x4000_0003).unwrap_err();
        assert!(
            error.contains("Goose Green") && error.contains("short strip"),
            "{error}"
        );
        assert!(!q.ground_start());
        q.choose_ground_runway(0x4000_0000).unwrap();
        assert_eq!(q.ground_runway(), Some(0x4000_0000));
        // Something that is neither keeps the old message.
        let error = q.choose_ground_runway(0x4000_0999).unwrap_err();
        assert!(error.contains("No imported runway matches"), "{error}");
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
    fn separation_lists_ten_nautical_choices_and_never_panics() {
        let mut q = setup();
        let labels = q.values(17).to_vec();
        assert_eq!(labels.len(), 10);
        assert_eq!(
            &labels[6..],
            ["100 miles", "150 miles", "200 miles", "300 miles"]
        );
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
        for expected in ["100 miles", "150 miles", "200 miles", "300 miles"] {
            q.activate(17);
            assert_eq!(q.value(17), expected);
        }
        q.activate(17);
        assert_eq!(q.draft.values[17], 0);
        for expected in ["300 miles", "200 miles", "150 miles", "100 miles"] {
            right_click(&mut q, 17);
            assert_eq!(q.value(17), expected);
        }
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
