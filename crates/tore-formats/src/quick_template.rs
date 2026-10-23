//! Bounded reader for the Quick Mission ground-target templates, the 129
//! `~Q<theater><target>.M` resources. A template is an object list in the
//! mission text grammar with three differences from a theater layout: it has no
//! `textFormat` header, an object's `type` may be a placeholder such as
//! `<sam>`, and it may carry `quickpos` and `waypoint2` route blocks. This
//! reader keeps the theater reader's strict grammar separate
//! ([`crate::mission::Layout`]). Nothing is rolled or resolved here: the lead
//! slices read placeholders, owners and flags raw. Grammar, census and the
//! FA.EXE facts: docs/formats/quick-templates.md.
use crate::{
    Result, invalid,
    mission::{NationalityField, display_name, integer, mission_nationality, resource, vector},
};

pub mod tables;

#[cfg(test)]
mod import_tests;

pub const MAX_BYTES: usize = 4 * 1024 * 1024;
/// The largest retail template has 127 objects.
pub const MAX_OBJECTS: usize = 512;
/// Manual p. 209: "up to ten waypoints".
pub const MAX_WAYPOINTS: usize = 10;
const MAX_LINE: usize = 4096;
const MAX_FIELDS: usize = 128;
/// Mission `flags` bit 0x80: a destroy target the debrief counts.
pub const TARGET_FLAG: i32 = 0x80;
/// Mission `flags` bit 0x20: a protect objective.
pub const PROTECT_FLAG: i32 = 0x20;
/// `w_flags` of the route's first waypoint.
pub const WAYPOINT_START: i32 = 1;
/// `w_flags` of a waypoint to drive or sail to.
pub const WAYPOINT_LEG: i32 = 4;
/// `w_flags` of the closing waypoint, written with position 0 0 0.
pub const WAYPOINT_END: i32 = 2;

/// The `type` placeholders the executable's generator knows (strings at
/// `0x4f3914..0x4f3984`). It compares them case sensitively in lower case; the
/// reader folds case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Placeholder {
    Sam,
    Aaa,
    Tank,
    Afv,
    Vehicle,
    Small,
    Hovercraft,
    Destroyer,
    Cruiser,
    Carrier,
    Cargo,
    /// What a failed defense roll writes.
    Nothing,
}

impl Placeholder {
    pub const ALL: [Self; 12] = [
        Self::Sam,
        Self::Aaa,
        Self::Tank,
        Self::Afv,
        Self::Vehicle,
        Self::Small,
        Self::Hovercraft,
        Self::Destroyer,
        Self::Cruiser,
        Self::Carrier,
        Self::Cargo,
        Self::Nothing,
    ];
    /// The retail spelling without the angle brackets.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sam => "sam",
            Self::Aaa => "aaa",
            Self::Tank => "tank",
            Self::Afv => "afv",
            Self::Vehicle => "vehicle",
            Self::Small => "small",
            Self::Hovercraft => "hovercraft",
            Self::Destroyer => "destroyer",
            Self::Cruiser => "cruiser",
            Self::Carrier => "carrier",
            Self::Cargo => "cargo",
            Self::Nothing => "nothing",
        }
    }
    /// Accepts `<sam>`, `<SAM>` and `<Sam>`.
    pub fn parse(token: &str) -> Option<Self> {
        let inner = token.strip_prefix('<')?.strip_suffix('>')?;
        Self::ALL
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(inner))
    }
    /// True for the two placeholders a defense strength setting gates.
    pub fn is_defense(self) -> bool {
        matches!(self, Self::Sam | Self::Aaa)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    /// An archive resource, upper case: `T72.NT`, `BNK6.OT`, `SU35.PT`.
    Named(String),
    Placeholder(Placeholder),
}

impl ObjectKind {
    /// The resource extension (`NT`, `OT`, `PT`) of a named object.
    pub fn extension(&self) -> Option<&str> {
        match self {
            Self::Named(name) => name.rsplit_once('.').map(|(_, ext)| ext),
            Self::Placeholder(_) => None,
        }
    }
}

/// The nationality field exactly as the template wrote it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    pub field: NationalityField,
    pub source: i32,
}

impl Owner {
    /// Bit 0x80 of the byte: Redfor, the side the retail "them" flag marks.
    pub fn redfor(&self) -> bool {
        self.source as u8 & 0x80 != 0
    }
    /// The creator nationality index with the side bit. `nationality2` and
    /// `nationality3` are already numbered; the legacy field goes through the
    /// theater's remap, so it needs the map name.
    pub fn index(&self, map: Option<&str>) -> i32 {
        match self.field {
            NationalityField::Legacy => mission_nationality(map, self.source),
            NationalityField::Two | NationalityField::Three => i32::from(self.source as u8),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waypoint {
    pub index: u32,
    /// [`WAYPOINT_START`], [`WAYPOINT_LEG`] or [`WAYPOINT_END`].
    pub flags: i32,
    pub goal: i32,
    pub next: i32,
    /// The two leading numbers of `w_pos2` (0 0 on the start, 1 0 on a leg).
    pub head: [i32; 2],
    /// `w_pos2` x, y, z. The closing waypoint reads 0 0 0.
    pub position: [i32; 3],
    /// `w_speed` in feet per second (decision 12.2 of the surface-AI plan).
    pub speed: i32,
    /// `w_wng`, the aircraft wing format (`1 0 2048 0` on legs).
    pub wing: [i32; 4],
    pub react: [u32; 3],
    pub search_dist: i32,
    /// `w_preferredTargetId` or the `...Id2` spelling.
    pub preferred_target: i32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    /// `w_for`: the alias of the object that follows this route.
    pub alias: i32,
    pub waypoints: Vec<Waypoint>,
}

impl Route {
    /// The start waypoint, when the route has one.
    pub fn start(&self) -> Option<&Waypoint> {
        self.waypoints
            .iter()
            .find(|w| w.flags & WAYPOINT_START != 0)
    }
    /// The waypoints to drive or sail to, in order.
    pub fn legs(&self) -> impl Iterator<Item = &Waypoint> {
        self.waypoints
            .iter()
            .filter(|w| w.flags & WAYPOINT_LEG != 0)
    }
    /// Length of the path start to last leg, in feet, flat.
    pub fn length_feet(&self) -> f64 {
        let mut last = self.start().map(|w| w.position);
        let mut total = 0.0;
        for leg in self.legs() {
            if let Some(from) = last {
                let dx = f64::from(leg.position[0] - from[0]);
                let dz = f64::from(leg.position[2] - from[2]);
                total += dx.hypot(dz);
            }
            last = Some(leg.position);
        }
        total
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateObject {
    /// Position in the file, counting every object. The id basis for fixed
    /// network ids: it does not change when a roll removes an object.
    pub ordinal: u32,
    pub kind: ObjectKind,
    pub position: [i32; 3],
    pub angles: [i32; 3],
    pub owner: Owner,
    /// Mission `flags`: `$13` and `$413` are common, 0x80 a target.
    pub flags: i32,
    /// Always 0 in the retail templates; movement comes from the route.
    pub speed: i32,
    pub alias: i32,
    pub skill: Option<i32>,
    pub react: Option<[u32; 3]>,
    pub search_dist: Option<i32>,
    /// Seconds, 60 to 5,400 on 159 retail objects; the consumer is not traced.
    pub start_time: Option<i32>,
    pub route: Option<Route>,
    /// Fields this reader does not interpret, in source order.
    pub unknown: Vec<(String, String)>,
}

impl TemplateObject {
    /// Flag 0x80: a destroy target.
    pub fn is_target(&self) -> bool {
        self.flags & TARGET_FLAG != 0
    }
    /// Flag 0x20: a protect objective.
    pub fn is_protected(&self) -> bool {
        self.flags & PROTECT_FLAG != 0
    }
    pub fn placeholder(&self) -> Option<Placeholder> {
        match self.kind {
            ObjectKind::Placeholder(p) => Some(p),
            ObjectKind::Named(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    /// `QUCOL` for `~QUCOL.M`: the name a `ground-target` line carries.
    pub stem: String,
    /// `quickpos`/`quickPos`: the engagement anchor accumulator (18 templates,
    /// all "nothing" templates).
    pub quickpos: Option<[i32; 3]>,
    pub objects: Vec<TemplateObject>,
}

impl Template {
    /// `resource_name` is the archive name, `~QUCOL.M` (any case).
    pub fn parse(resource_name: &str, bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_BYTES {
            return Err(invalid("template exceeds limit"));
        }
        let stem = stem_of(resource_name)?;
        // The retail files end in a Ctrl-Z, or a NUL after trailing blank lines.
        let end = bytes
            .iter()
            .position(|b| matches!(b, 0 | 0x1a))
            .unwrap_or(bytes.len());
        let text = crate::text::decode_cp437(&bytes[..end]);
        let mut quickpos = None;
        let mut objects: Vec<ObjectBuilder> = Vec::new();
        let mut routes: Vec<(i32, Route)> = Vec::new();
        let mut block = Block::None;
        for (line_index, raw) in text.lines().enumerate() {
            let line_no = line_index + 1;
            let context = format!("{stem}:{line_no}");
            if raw.len() > MAX_LINE {
                return Err(invalid(&format!("{context}: line exceeds limit")));
            }
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }
            match &mut block {
                Block::None => {
                    let (keyword, rest) = split_key(trimmed);
                    if raw.starts_with(char::is_whitespace) {
                        return Err(invalid(&format!(
                            "{context}: indented text outside a block"
                        )));
                    }
                    match keyword.to_ascii_lowercase().as_str() {
                        "textformat" => {}
                        "obj" if rest.is_empty() => {
                            if objects.len() == MAX_OBJECTS {
                                return Err(invalid(&format!("{context}: too many objects")));
                            }
                            block = Block::Object(line_no, Vec::new());
                        }
                        "waypoint2" => {
                            let count = integer(rest, &context)?;
                            if !(1..=MAX_WAYPOINTS as i32).contains(&count) {
                                return Err(invalid(&format!(
                                    "{context}: waypoint count outside 1 to {MAX_WAYPOINTS}"
                                )));
                            }
                            block = Block::Route(RouteBuilder::new(line_no, count as usize));
                        }
                        "quickpos" => {
                            if quickpos.replace(vector(rest, &context)?).is_some() {
                                return Err(invalid(&format!("{context}: duplicate quickpos")));
                            }
                        }
                        _ => {
                            return Err(invalid(&format!(
                                "{context}: unsupported template statement"
                            )));
                        }
                    }
                }
                Block::Object(start, fields) => {
                    if trimmed == "." {
                        let ordinal = objects.len() as u32;
                        objects.push(ObjectBuilder::build(&stem, ordinal, *start, fields)?);
                        block = Block::None;
                        continue;
                    }
                    if !raw.starts_with(char::is_whitespace) {
                        return Err(invalid(&format!("{context}: unterminated object record")));
                    }
                    let (key, value) = split_key(trimmed);
                    if fields.iter().any(|(seen, _)| seen == key) {
                        return Err(invalid(&format!("{context}: duplicate object field {key}")));
                    }
                    if fields.len() == MAX_FIELDS {
                        return Err(invalid(&format!("{context}: too many object fields")));
                    }
                    fields.push((key.to_owned(), value.to_owned()));
                }
                Block::Route(route) => {
                    if trimmed == "." {
                        let (owner, route) =
                            std::mem::replace(route, RouteBuilder::new(0, 0)).finish(&stem)?;
                        routes.push((owner, route));
                        block = Block::None;
                        continue;
                    }
                    if !raw.starts_with(char::is_whitespace) {
                        return Err(invalid(&format!("{context}: unterminated route block")));
                    }
                    let (key, value) = split_key(trimmed);
                    route.field(key, value, &context)?;
                }
            }
        }
        match block {
            Block::None => {}
            Block::Object(start, _) => {
                return Err(invalid(&format!(
                    "{stem}:{start}: unterminated object record"
                )));
            }
            Block::Route(route) => {
                return Err(invalid(&format!(
                    "{stem}:{}: unterminated route block",
                    route.line
                )));
            }
        }
        let mut objects: Vec<TemplateObject> = objects.into_iter().map(|b| b.0).collect();
        for (alias, route) in routes {
            let mut matching = objects.iter_mut().filter(|object| object.alias == alias);
            let target = matching
                .next()
                .ok_or_else(|| invalid(&format!("{stem}: route for unknown alias {alias}")))?;
            if matching.next().is_some() {
                return Err(invalid(&format!(
                    "{stem}: route alias {alias} is ambiguous"
                )));
            }
            if target.route.replace(route).is_some() {
                return Err(invalid(&format!("{stem}: two routes for alias {alias}")));
            }
        }
        Ok(Self {
            stem,
            quickpos,
            objects,
        })
    }

    /// The archive name, `~QUCOL.M`.
    pub fn resource(&self) -> String {
        format!("~{}.M", self.stem)
    }
    /// Objects flagged 0x80, placeholders included (a failed roll removes one).
    pub fn targets(&self) -> impl Iterator<Item = &TemplateObject> {
        self.objects.iter().filter(|o| o.is_target())
    }
    /// How many objects use a placeholder.
    pub fn placeholder_count(&self, placeholder: Placeholder) -> usize {
        self.objects
            .iter()
            .filter(|o| o.placeholder() == Some(placeholder))
            .count()
    }
    /// Objects that follow a route.
    pub fn routed(&self) -> impl Iterator<Item = &TemplateObject> {
        self.objects.iter().filter(|o| o.route.is_some())
    }
}

/// `~QUCOL.M` to `QUCOL`.
fn stem_of(resource_name: &str) -> Result<String> {
    let name = resource(resource_name, resource_name)?;
    let stem = name
        .strip_prefix('~')
        .and_then(|rest| rest.strip_suffix(".M"))
        .filter(|stem| stem.starts_with('Q') && stem.len() > 1)
        .ok_or_else(|| invalid("template name must look like ~Q<theater><target>.M"))?;
    Ok(stem.to_owned())
}

fn split_key(line: &str) -> (&str, &str) {
    line.split_once(char::is_whitespace)
        .map_or((line, ""), |(key, rest)| (key, rest.trim()))
}

enum Block {
    None,
    Object(usize, Vec<(String, String)>),
    Route(RouteBuilder),
}

struct ObjectBuilder(TemplateObject);

impl ObjectBuilder {
    fn build(stem: &str, ordinal: u32, line: usize, fields: &[(String, String)]) -> Result<Self> {
        let context = format!("{stem}:{line}");
        let find = |name: &str| {
            fields
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        let number = |name: &str| find(name).map(|v| integer(v, &context)).transpose();
        let triple = |name: &str| -> Result<Option<[u32; 3]>> {
            find(name)
                .map(|v| {
                    let values = v
                        .split_whitespace()
                        .map(|x| integer(x, &context).map(|n| n as u32))
                        .collect::<Result<Vec<_>>>()?;
                    values
                        .try_into()
                        .map_err(|_| invalid(&format!("{context}: expected three integers")))
                })
                .transpose()
        };
        let ty = find("type").ok_or_else(|| invalid(&format!("{context}: missing object type")))?;
        let kind = if ty.starts_with('<') {
            ObjectKind::Placeholder(
                Placeholder::parse(ty)
                    .ok_or_else(|| invalid(&format!("{context}: unknown placeholder {ty}")))?,
            )
        } else {
            ObjectKind::Named(resource(ty, &context)?)
        };
        let position = vector(
            find("pos").ok_or_else(|| invalid(&format!("{context}: missing object position")))?,
            &context,
        )?;
        let angles = find("angle").map_or(Ok([0; 3]), |v| vector(v, &context))?;
        let mut owners = [
            ("nationality", NationalityField::Legacy),
            ("nationality2", NationalityField::Two),
            ("nationality3", NationalityField::Three),
        ]
        .into_iter()
        .filter_map(|(name, field)| find(name).map(|v| (field, v)));
        let (field, value) = owners
            .next()
            .ok_or_else(|| invalid(&format!("{context}: object has no nationality")))?;
        if owners.next().is_some() {
            return Err(invalid(&format!(
                "{context}: conflicting nationality fields"
            )));
        }
        let owner = Owner {
            field,
            source: integer(value, &context)?,
        };
        let known = [
            "type",
            "pos",
            "angle",
            "nationality",
            "nationality2",
            "nationality3",
            "flags",
            "speed",
            "alias",
            "skill",
            "react",
            "searchDist",
            "startTime",
        ];
        Ok(Self(TemplateObject {
            ordinal,
            kind,
            position,
            angles,
            owner,
            flags: number("flags")?.unwrap_or(0),
            speed: number("speed")?.unwrap_or(0),
            alias: number("alias")?.unwrap_or(0),
            skill: number("skill")?,
            react: triple("react")?,
            search_dist: number("searchDist")?,
            start_time: number("startTime")?,
            route: None,
            unknown: fields
                .iter()
                .filter(|(key, _)| !known.contains(&key.as_str()))
                .cloned()
                .collect(),
        }))
    }
}

struct RouteBuilder {
    line: usize,
    count: usize,
    alias: Option<i32>,
    waypoints: Vec<Waypoint>,
}

impl RouteBuilder {
    fn new(line: usize, count: usize) -> Self {
        Self {
            line,
            count,
            alias: None,
            waypoints: Vec::new(),
        }
    }
    fn field(&mut self, key: &str, value: &str, context: &str) -> Result<()> {
        let ints = |n: usize| -> Result<Vec<i32>> {
            let values = value
                .split_whitespace()
                .map(|x| integer(x, context))
                .collect::<Result<Vec<_>>>()?;
            if values.len() == n {
                Ok(values)
            } else {
                Err(invalid(&format!("{context}: expected {n} integers")))
            }
        };
        let one = || integer(value, context);
        if key == "w_for" {
            if self.alias.replace(one()?).is_some() {
                return Err(invalid(&format!("{context}: duplicate w_for")));
            }
            return Ok(());
        }
        if key == "w_index" {
            if self.waypoints.len() == self.count {
                return Err(invalid(&format!("{context}: more waypoints than declared")));
            }
            let index = one()?;
            if index != self.waypoints.len() as i32 {
                return Err(invalid(&format!("{context}: waypoint index out of order")));
            }
            self.waypoints.push(Waypoint {
                index: index as u32,
                flags: 0,
                goal: 0,
                next: 0,
                head: [0; 2],
                position: [0; 3],
                speed: 0,
                wing: [0; 4],
                react: [0; 3],
                search_dist: 0,
                preferred_target: 0,
                name: String::new(),
            });
            return Ok(());
        }
        let waypoint = self
            .waypoints
            .last_mut()
            .ok_or_else(|| invalid(&format!("{context}: waypoint field before w_index")))?;
        match key {
            "w_flags" => waypoint.flags = one()?,
            "w_goal" => waypoint.goal = one()?,
            "w_next" => waypoint.next = one()?,
            "w_pos2" => {
                let v = ints(5)?;
                waypoint.head = [v[0], v[1]];
                waypoint.position = [v[2], v[3], v[4]];
            }
            "w_speed" => {
                waypoint.speed = one()?;
                if waypoint.speed < 0 {
                    return Err(invalid(&format!("{context}: negative waypoint speed")));
                }
            }
            "w_wng" => waypoint.wing = ints(4)?.try_into().unwrap(),
            "w_react" => {
                let v = ints(3)?;
                waypoint.react = [v[0] as u32, v[1] as u32, v[2] as u32];
            }
            "w_searchDist" => waypoint.search_dist = one()?,
            "w_preferredTargetId" | "w_preferredTargetId2" => waypoint.preferred_target = one()?,
            "w_name" => {
                // Retail writes a name between two Ctrl-A marks, or nothing.
                waypoint.name = if value.trim_matches('\u{1}').is_empty() {
                    String::new()
                } else {
                    display_name(value, context)?
                };
            }
            _ => {
                return Err(invalid(&format!(
                    "{context}: unsupported waypoint field {key}"
                )));
            }
        }
        Ok(())
    }
    fn finish(self, stem: &str) -> Result<(i32, Route)> {
        let context = format!("{stem}:{}", self.line);
        if self.waypoints.len() != self.count {
            return Err(invalid(&format!(
                "{context}: route declares {} waypoints and holds {}",
                self.count,
                self.waypoints.len()
            )));
        }
        let alias = self
            .alias
            .ok_or_else(|| invalid(&format!("{context}: route has no w_for")))?;
        // The owner is resolved by alias after every object is read.
        Ok((
            alias,
            Route {
                alias,
                waypoints: self.waypoints,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAY: &str = "waypoint2 3\r\n\tw_index 0\r\n\tw_flags 1\r\n\tw_goal 0\r\n\tw_next 0\r\n\tw_pos2 0   0   1000 0 2000\r\n\tw_speed 0\r\n\tw_wng 0 0 0 0\r\n\tw_react 0 0 0\r\n\tw_searchDist 0\r\n\tw_preferredTargetId 0\r\n\tw_name \u{1}\u{1}\r\n\r\n\tw_index 1\r\n\tw_flags $4\r\n\tw_goal 1\r\n\tw_next 0\r\n\tw_pos2 1   0   4000 0 6000\r\n\tw_speed 50\r\n\tw_wng 1 0 2048 0\r\n\tw_react $c000 $0 $0\r\n\tw_searchDist 25\r\n\tw_preferredTargetId2 $0\r\n\tw_name \r\n\r\n\tw_index 2\r\n\tw_flags 2\r\n\tw_goal 0\r\n\tw_next 0\r\n\tw_pos2 0   0   0 0 0\r\n\tw_speed 0\r\n\tw_wng 0 0 0 0\r\n\tw_react 0 0 0\r\n\tw_searchDist 0\r\n\tw_preferredTargetId 0\r\n\tw_name \u{1}\u{1}\r\n\r\n  w_for -3\r\n\t.\r\n";

    fn object(extra: &str) -> String {
        format!(
            "obj\r\n\ttype T72.NT\r\n\tpos 10 0 20\r\n\tangle -90 0 0\r\n\tnationality2 138\r\n\tflags $93\r\n\tspeed 0\r\n\talias -3\r\n{extra}\t.\r\n"
        )
    }

    #[test]
    fn placeholders_fold_case_and_reject_strangers() {
        assert_eq!(Placeholder::parse("<sam>"), Some(Placeholder::Sam));
        assert_eq!(Placeholder::parse("<SAM>"), Some(Placeholder::Sam));
        assert_eq!(Placeholder::parse("<Cargo>"), Some(Placeholder::Cargo));
        assert_eq!(Placeholder::parse("<nothing>"), Some(Placeholder::Nothing));
        assert_eq!(Placeholder::parse("<helicopter>"), None);
        assert_eq!(Placeholder::parse("sam"), None);
        assert_eq!(Placeholder::parse("<sam"), None);
        for p in Placeholder::ALL {
            assert_eq!(Placeholder::parse(&format!("<{}>", p.name())), Some(p));
        }
        assert!(Placeholder::Sam.is_defense() && !Placeholder::Tank.is_defense());
    }

    #[test]
    fn reads_objects_with_hex_and_decimal_words() {
        let text = format!(
            "{}{}",
            "obj\r\n\ttype <SAM>\r\n\tpos 999570 0 789172\r\n\tangle 135 0 0\r\n\tnationality3 151\r\n\tflags $413\r\n\tspeed 0\r\n\talias -1\r\n\tskill 3\r\n\treact $c000 $3fff $0\r\n\tsearchDist 25\r\n\tstartTime 1800\r\n\t.\r\n",
            "obj\r\n\ttype su35.PT\r\n\tpos 1 0 2\r\n\tangle 0 0 0\r\n\tnationality 0\r\n\tflags 128\r\n\tspeed 0\r\n\talias -2\r\n\treact 49152 0 0\r\n\tfuture thing\r\n\t.\r\n"
        );
        let t = Template::parse("~qucol.m", text.as_bytes()).unwrap();
        assert_eq!(t.stem, "QUCOL");
        assert_eq!(t.resource(), "~QUCOL.M");
        assert_eq!(t.objects.len(), 2);
        let a = &t.objects[0];
        assert_eq!(a.kind, ObjectKind::Placeholder(Placeholder::Sam));
        assert_eq!(a.position, [999570, 0, 789172]);
        assert_eq!(a.flags, 0x413);
        assert!(!a.is_target() && !a.is_protected());
        assert_eq!(
            (a.skill, a.search_dist, a.start_time),
            (Some(3), Some(25), Some(1800))
        );
        assert_eq!(a.react, Some([0xc000, 0x3fff, 0]));
        assert_eq!(a.owner.field, NationalityField::Three);
        assert!(a.owner.redfor());
        assert_eq!(a.owner.index(Some("UKR.T2")), 151);
        let b = &t.objects[1];
        assert_eq!(b.kind, ObjectKind::Named("SU35.PT".into()));
        assert_eq!(b.kind.extension(), Some("PT"));
        assert!(b.is_target());
        assert_eq!(b.react, Some([49152, 0, 0]));
        assert_eq!(b.skill, None);
        assert_eq!(b.unknown, vec![("future".into(), "thing".into())]);
        assert_eq!(b.ordinal, 1);
        assert_eq!(t.targets().count(), 1);
        assert_eq!(t.placeholder_count(Placeholder::Sam), 1);
        // The legacy field gets the theater remap; Redfor comes from bit 0x80.
        assert_eq!(b.owner.field, NationalityField::Legacy);
        assert!(!b.owner.redfor());
        assert_eq!(b.owner.index(None), 0);
    }

    #[test]
    fn type_names_may_lack_an_extension() {
        let text = object("").replace("T72.NT", "t72");
        let t = Template::parse("~QXX.M", text.as_bytes()).unwrap();
        assert_eq!(t.objects[0].kind, ObjectKind::Named("T72".into()));
        assert_eq!(t.objects[0].kind.extension(), None);
        for bad in ["<unknown>", "T72 NT", "../x", ""] {
            let text = object("").replace("T72.NT", bad);
            assert!(
                Template::parse("~QXX.M", text.as_bytes()).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn quickpos_in_both_spellings_and_empty_templates() {
        for spelling in ["quickpos", "quickPos", "QUICKPOS"] {
            let text = format!("\r\n{spelling} 983444 0 352711\r\n\r\n");
            let t = Template::parse("~QENOTH.M", text.as_bytes()).unwrap();
            assert_eq!(t.quickpos, Some([983444, 0, 352711]));
            assert!(t.objects.is_empty());
        }
        let both = "quickpos 1 2 3\r\nquickPos 4 5 6\r\n";
        assert!(Template::parse("~QENOTH.M", both.as_bytes()).is_err());
        assert!(Template::parse("~QENOTH.M", b"quickpos 1 2\r\n").is_err());
        assert!(
            Template::parse("~QENOTH.M", b"")
                .unwrap()
                .objects
                .is_empty()
        );
    }

    #[test]
    fn route_binds_by_alias_and_keeps_speed_in_feet_per_second() {
        let text = format!("{}{}", object(""), WAY);
        let t = Template::parse("~QUCOL.M", text.as_bytes()).unwrap();
        let route = t.objects[0]
            .route
            .as_ref()
            .expect("route bound by w_for -3");
        assert_eq!(route.alias, -3);
        assert_eq!(route.waypoints.len(), 3);
        assert_eq!(route.start().unwrap().position, [1000, 0, 2000]);
        let legs: Vec<_> = route.legs().collect();
        assert_eq!(legs.len(), 1);
        assert_eq!((legs[0].speed, legs[0].position), (50, [4000, 0, 6000]));
        assert_eq!(legs[0].wing, [1, 0, 2048, 0]);
        assert_eq!(legs[0].react, [0xc000, 0, 0]);
        assert_eq!(legs[0].search_dist, 25);
        assert_eq!(legs[0].head, [1, 0]);
        assert_eq!(route.waypoints[2].flags, WAYPOINT_END);
        assert_eq!(route.waypoints[2].position, [0, 0, 0]);
        assert_eq!(route.waypoints[0].name, "");
        // A 3000 by 4000 foot leg.
        assert!((route.length_feet() - 5000.0).abs() < 1e-9);
        assert_eq!(t.routed().count(), 1);
    }

    #[test]
    fn retail_file_endings_are_accepted() {
        // Windows line ends, a Ctrl-Z after blank lines, or a NUL.
        let base = format!("{}{}", object(""), WAY);
        for tail in [
            "\r\n \r\n\u{1a}",
            "\r\n\u{0}",
            "\r\n\r\n\u{1a}junk after the marker\r\nobj",
        ] {
            let text = format!("{base}{tail}");
            let t = Template::parse("~QUCOL.M", text.as_bytes()).unwrap();
            assert_eq!(t.objects.len(), 1, "{tail:?}");
        }
    }

    #[test]
    fn owners_decode_all_three_fields_and_conflicts_fail() {
        for (name, field) in [
            ("nationality", NationalityField::Legacy),
            ("nationality2", NationalityField::Two),
            ("nationality3", NationalityField::Three),
        ] {
            let text = object("").replace("nationality2 138", &format!("{name} 138"));
            let t = Template::parse("~QXX.M", text.as_bytes()).unwrap();
            assert_eq!(t.objects[0].owner.field, field);
            assert!(t.objects[0].owner.redfor());
        }
        let both = object("\tnationality3 138\r\n");
        assert!(Template::parse("~QXX.M", both.as_bytes()).is_err());
        let none = object("").replace("\tnationality2 138\r\n", "");
        assert!(Template::parse("~QXX.M", none.as_bytes()).is_err());
        // Legacy 137 on a Ukraine map is Redfor Russian (9 + 1 = 10).
        let legacy = object("").replace("nationality2 138", "nationality 137");
        let t = Template::parse("~QUXX.M", legacy.as_bytes()).unwrap();
        assert_eq!(t.objects[0].owner.index(Some("UKR.T2")), 128 | 10);
    }

    #[test]
    fn malformed_templates_fail_closed() {
        let good = format!("{}{}", object(""), WAY);
        assert!(Template::parse("~QUCOL.M", good.as_bytes()).is_ok());
        let cases: Vec<(&str, String)> = vec![
            ("name", String::new()),
            (
                "duplicate field",
                good.replace("\tflags $93\r\n", "\tflags 1\r\n\tflags 2\r\n"),
            ),
            ("missing type", good.replace("\ttype T72.NT\r\n", "")),
            ("missing pos", good.replace("\tpos 10 0 20\r\n", "")),
            ("unterminated obj", good.replacen("\t.\r\n", "", 1)),
            ("unknown statement", format!("hello\r\n{good}")),
            ("indented stray", format!("  stray\r\n{good}")),
            ("route unknown alias", good.replace("w_for -3", "w_for -9")),
            ("route no w_for", good.replace("  w_for -3\r\n", "")),
            ("count mismatch", good.replace("waypoint2 3", "waypoint2 4")),
            ("zero count", good.replace("waypoint2 3", "waypoint2 0")),
            (
                "eleven waypoints",
                good.replace("waypoint2 3", "waypoint2 11"),
            ),
            ("index out of order", good.replace("w_index 1", "w_index 2")),
            (
                "bad pos2",
                good.replace("w_pos2 0   0   1000 0 2000", "w_pos2 0 0 1000"),
            ),
            (
                "unknown waypoint field",
                good.replace("w_goal 0", "w_surprise 0"),
            ),
            ("negative speed", good.replace("w_speed 50", "w_speed -1")),
            ("field before index", good.replace("\tw_index 0\r\n", "")),
            ("two routes", format!("{good}{WAY}")),
            ("bad number", good.replace("alias -3", "alias three")),
            (
                "long line",
                format!("quickpos 1 2 3 {}\r\n", " ".repeat(5000)),
            ),
        ];
        for (name, text) in cases {
            assert!(
                Template::parse(
                    if name == "name" { "UCOL.M" } else { "~QUCOL.M" },
                    text.as_bytes()
                )
                .is_err(),
                "{name} should fail"
            );
        }
        for bad in ["QUCOL.M", "~UCOL.M", "~QUCOL.MM", "~Q.M", "~QUCOL"] {
            assert!(Template::parse(bad, b"").is_err(), "{bad}");
        }
    }

    #[test]
    fn size_and_count_bounds() {
        assert!(Template::parse("~QXX.M", &vec![b' '; MAX_BYTES + 1]).is_err());
        let one = object("");
        let many = one.repeat(MAX_OBJECTS);
        assert_eq!(
            Template::parse("~QXX.M", many.as_bytes())
                .unwrap()
                .objects
                .len(),
            MAX_OBJECTS
        );
        let too_many = one.repeat(MAX_OBJECTS + 1);
        assert!(Template::parse("~QXX.M", too_many.as_bytes()).is_err());
        // Ten waypoints are legal, eleven are not (checked above).
        let mut route = String::from("waypoint2 10\r\n");
        for i in 0..10 {
            route.push_str(&format!(
                "\tw_index {i}\r\n\tw_flags 4\r\n\tw_pos2 1 0 {i} 0 {i}\r\n\tw_speed 10\r\n"
            ));
        }
        route.push_str("\tw_for -3\r\n\t.\r\n");
        let text = format!("{}{route}", object(""));
        let t = Template::parse("~QXX.M", text.as_bytes()).unwrap();
        assert_eq!(t.objects[0].route.as_ref().unwrap().waypoints.len(), 10);
    }

    #[test]
    fn ambiguous_aliases_for_routes_fail() {
        let two = format!("{}{}{}", object(""), object(""), WAY);
        assert!(Template::parse("~QXX.M", two.as_bytes()).is_err());
        // Shared aliases are fine when no route needs them.
        let fine = format!("{}{}", object(""), object(""));
        assert_eq!(
            Template::parse("~QXX.M", fine.as_bytes())
                .unwrap()
                .objects
                .len(),
            2
        );
    }

    #[test]
    fn hostile_input_never_panics() {
        let good = format!("{}{}", object(""), WAY);
        for cut in (0..good.len()).step_by(5) {
            let _ = Template::parse("~QXX.M", &good.as_bytes()[..cut]);
        }
        let lines: Vec<_> = good.split("\r\n").collect();
        for skip in 0..lines.len() {
            let mutated: Vec<_> = lines
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != skip)
                .map(|(_, l)| *l)
                .collect();
            let _ = Template::parse("~QXX.M", mutated.join("\r\n").as_bytes());
        }
        let mut noise = good.into_bytes();
        for at in (0..noise.len()).step_by(11) {
            noise[at] = 0xff;
            let _ = Template::parse("~QXX.M", &noise);
        }
    }
}
