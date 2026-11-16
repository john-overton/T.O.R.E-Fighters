//! Bounded BRF data recovery and aircraft dependency selection. Never executes modules.
use crate::{Archive, Result, invalid};
use std::collections::{BTreeMap, BTreeSet};
#[path = "aircraft_corrections.rs"]
pub mod corrections;
#[path = "aircraft_scale.rs"]
pub mod scale;
#[path = "aircraft_schema.rs"]
pub(crate) mod schema;
#[path = "aircraft_stations.rs"]
pub mod stations;
#[derive(Clone, Debug)]
pub struct Token {
    pub kind: String,
    pub value: String,
    pub scaled: bool,
}
impl Token {
    pub fn number(&self) -> Result<i32> {
        let bits = match self.kind.as_str() {
            "byte" => 8,
            "word" => 16,
            "dword" => 32,
            _ => return Err(invalid("expected numeric BRF field")),
        };
        let raw = if let Some(s) = self.value.strip_prefix('$') {
            i64::from_str_radix(s, 16)
        } else {
            self.value.parse::<i64>()
        }
        .map_err(|_| invalid("invalid BRF number"))?;
        if raw < i32::MIN as i64 || raw > u32::MAX as i64 {
            return Err(invalid("BRF number exceeds 32 bits"));
        }
        Ok(match bits {
            8 => raw as u8 as i32,
            16 => raw as u16 as i16 as i32,
            _ => raw as u32 as i32,
        })
    }
}
#[derive(Debug)]
pub struct Brf {
    pub blocks: BTreeMap<String, Vec<Token>>,
}
impl Brf {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 1024 * 1024 {
            return Err(invalid("BRF exceeds limit"));
        }
        let text = std::str::from_utf8(bytes).map_err(|_| invalid("non-UTF8 BRF"))?;
        if !text.starts_with("[brent's_relocatable_format]") {
            return Err(invalid("not BRF data"));
        }
        let mut blocks = BTreeMap::from([(String::new(), Vec::new())]);
        let mut label = String::new();
        let mut ended = false;
        for line in text.lines().skip(1) {
            let line = line.split(';').next().unwrap().trim();
            if line.is_empty() {
                continue;
            }
            if ended {
                return Err(invalid("BRF data after end"));
            }
            if line == "end" {
                ended = true;
                continue;
            }
            if let Some(s) = line.strip_prefix(':') {
                if s.is_empty()
                    || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    || blocks.contains_key(s)
                {
                    return Err(invalid("invalid/duplicate BRF label"));
                }
                label = s.into();
                blocks.insert(label.clone(), vec![]);
                continue;
            }
            let (kind, value) = line
                .split_once(char::is_whitespace)
                .ok_or_else(|| invalid("invalid BRF statement"))?;
            let mut value = value.trim();
            let scaled = value.starts_with('^');
            if scaled {
                value = &value[1..];
            }
            if kind == "string" {
                value = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .ok_or_else(|| invalid("invalid BRF string"))?;
            } else if value.is_empty() || value.contains(char::is_whitespace) {
                return Err(invalid("invalid BRF operand"));
            }
            let t = Token {
                kind: kind.into(),
                value: value.into(),
                scaled,
            };
            match kind {
                "byte" | "word" | "dword" => {
                    t.number()?;
                }
                "ptr" | "symbol" | "string" => {}
                _ => return Err(invalid("unknown BRF statement")),
            }
            blocks.get_mut(&label).unwrap().push(t);
        }
        if !ended {
            return Err(invalid("unterminated BRF"));
        }
        for t in blocks.values().flatten().filter(|t| t.kind == "ptr") {
            if !blocks.contains_key(&t.value) {
                return Err(invalid("unresolved BRF pointer"));
            }
        }
        Ok(Self { blocks })
    }
    pub fn block(&self, name: &str) -> Result<&[Token]> {
        self.blocks
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| invalid("missing BRF block"))
    }
    pub fn strings(&self, name: &str) -> Result<Vec<String>> {
        self.block(name)?
            .iter()
            .map(|t| {
                if t.kind == "string" {
                    Ok(t.value.clone())
                } else {
                    Err(invalid("expected BRF string block"))
                }
            })
            .collect()
    }
}
pub(crate) fn fields(tokens: &[Token], layout: &[(&str, &str)]) -> Result<BTreeMap<String, Token>> {
    if tokens.len() != layout.len() {
        return Err(invalid("BRF schema length mismatch"));
    }
    tokens
        .iter()
        .zip(layout)
        .map(|(t, (kind, name))| {
            if t.kind != *kind && !(*kind == "ptr" && t.kind == "dword" && t.number()? == 0) {
                return Err(invalid(&format!("BRF kind mismatch for {name}")));
            }
            Ok((name.to_string(), t.clone()))
        })
        .collect()
}
#[derive(Clone, Debug, PartialEq)]
pub struct Envelope {
    pub g: i32,
    pub points: Vec<[f64; 2]>,
}
impl Envelope {
    pub fn speeds(&self, alt: f64) -> Option<(f64, f64)> {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        let mut hit = |v: f64| {
            low = low.min(v);
            high = high.max(v);
        };
        let count = self.points.len();
        for i in 0..count {
            let a = self.points[i];
            let b = self.points[if i + 1 == count { 0 } else { i + 1 }];
            if (a[1] - alt).abs() < 1e-9 {
                hit(a[0]);
            }
            if (a[1] < alt && b[1] > alt) || (a[1] > alt && b[1] < alt) {
                hit(a[0] + (b[0] - a[0]) * (alt - a[1]) / (b[1] - a[1]));
            }
        }
        (low <= high).then_some((low, high))
    }
}
#[derive(Clone, Debug)]
pub struct Hardpoint {
    pub location: u8,
    pub flags: i32,
    pub position: [i32; 3],
    pub store: Option<String>,
    pub count: i32,
    pub weight_class: i32,
    /// `slewH`, `slewP`, `slewLimitH`, `slewLimitP` of the record: where the
    /// mount points at rest and the half-arc either side, 182 units a degree
    /// ([`crate::surface_unit::ANGLE_UNITS_PER_DEGREE`]). A forward gun reads
    /// zero heading; the B-52's tail gun 32,760 (180 degrees), 5,460 (30).
    /// Rows the game adds read zero.
    pub slew: [i32; 4],
}
/// Reviewed retail identities and explicit opinionated runtime variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AircraftId {
    F18,
    Rafale,
    F14,
    A4E,
    X31,
    Mig29,
    Su27,
    Mig21,
    Su25,
    Mig23,
    Su35,
    F22,
    F22n,
    Faxx,
    C130,
    Ac130,
    E3,
    Il76,
    E2,
    Av8,
    Yak141,
    V22,
    Ah64,
    Mi24,
    Ch47,
    Mig17,
    F4B,
    F4J,
    F4E,
    F4G,
    A7,
    F15,
    F16C,
    F104,
    A10,
    B747,
    A310,
    Mig25,
    F16A,
    Mig31,
    M2000,
    F15E,
    Gripen,
    F5E,
    Tornado,
    /// The Tornado IDS (Clean): the same record, shape and flight model as
    /// [`Self::Tornado`] (the Combat version), with no jammer, chaff or flares
    /// and no outer wing pods (John, 2026-11-12, TOR-13). A runtime variant of
    /// its own identity, never an alias.
    TornadoClean,
    Mf1,
    /// Bomber batch A (BMB-A1, 2026-11-15): appended after the Mirage F1 in
    /// batch order, each its own retail record and never an alias.
    B52,
    B2,
    F117,
    /// Bomber batch B (BMB-B1, 2026-11-15): appended after the F-117 in batch
    /// order, each its own retail record and never an alias.
    B1b,
    Tu160,
    F111,
    /// Bomber batch C (BMB-C1, 2026-11-15): appended after the F-111F in batch
    /// order, each its own retail record and never an alias.
    Tu26,
    Tu95,
    Su34,
}
/// An aircraft's own names for its targeting sight (the target camera's
/// title, the Load Ordnance sensor line, the replay's Sensor View) and for
/// its infrared channel (the radar page's IR label) and, for an aircraft whose
/// designator can be a loadable pod, that pod's name on the Load Ordnance
/// page. See [`AircraftId::sensor_names`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SensorNames {
    pub sight: &'static str,
    pub infrared: &'static str,
    /// The name of the designator pod on the Load Ordnance page (the F-111F's
    /// Pave Tack on its bay row). The F-117 has no such pod: its turret is a
    /// fixed row, and the field only repeats the sight name.
    pub pod: &'static str,
}

impl AircraftId {
    /// Retail donor for an opinionated runtime variant.
    pub fn source(self) -> Self {
        match self {
            Self::Faxx => Self::F22n,
            Self::TornadoClean => Self::Tornado,
            other => other,
        }
    }
    pub fn selection_key(self) -> &'static str {
        match self {
            Self::Faxx => "faxx",
            Self::TornadoClean => "tornadoc",
            other => other.pt(),
        }
    }
    pub const SELECTABLE: [Self; 56] = [
        Self::F18,
        Self::Rafale,
        Self::F14,
        Self::A4E,
        Self::X31,
        Self::Mig29,
        Self::Su27,
        Self::Mig21,
        Self::Su25,
        Self::Mig23,
        Self::Su35,
        Self::F22,
        Self::F22n,
        Self::Faxx,
        Self::C130,
        Self::Ac130,
        Self::E3,
        Self::Il76,
        Self::E2,
        Self::Av8,
        Self::Yak141,
        Self::V22,
        Self::Ah64,
        Self::Mi24,
        Self::Ch47,
        Self::Mig17,
        Self::F4B,
        Self::F4J,
        Self::F4E,
        Self::F4G,
        Self::A7,
        Self::F15,
        Self::F16C,
        Self::F104,
        Self::A10,
        Self::B747,
        Self::A310,
        Self::Mig25,
        Self::F16A,
        Self::Mig31,
        Self::M2000,
        Self::F15E,
        Self::Gripen,
        Self::F5E,
        Self::Tornado,
        Self::TornadoClean,
        Self::Mf1,
        Self::B52,
        Self::B2,
        Self::F117,
        Self::B1b,
        Self::Tu160,
        Self::F111,
        Self::Tu26,
        Self::Tu95,
        Self::Su34,
    ];
    /// Whether the AI can fly this aircraft in a mission (`opinionated`, agent
    /// decision 2026-10-08, revised 2026-10-12). Every selectable aircraft is
    /// offered to the AI. The AH-64 and the Mi-24 came in with slice ST-H (John,
    /// 2026-10-11); the CH-47 (unarmed, an AI wing choice only), the V-22, the
    /// AV-8 and the Yak-141 with slice H3 (John, 2026-10-12), once the AI could
    /// hover, climb and descend a rotorcraft (slice H1). The transport has no
    /// weapons and never strikes; a ground start with AI wingmen of a vertical
    /// lifter is slice H2's.
    pub fn ai_flyable(self) -> bool {
        true
    }
    /// A helicopter, a tiltrotor or a vectored-thrust jet: an aircraft that
    /// hovers and lifts off vertically, as the airfield sequences and the
    /// Quick Mission creator's ground-start check read it.
    pub fn is_powered_lift(self) -> bool {
        matches!(
            self,
            Self::Av8 | Self::Yak141 | Self::V22 | Self::Ah64 | Self::Mi24 | Self::Ch47
        )
    }
    /// The aircraft is a helicopter (the AH-64, Mi-24 or CH-47), as the
    /// strike class and the helicopter defence read it. The V-22 is a
    /// tiltrotor and not one.
    pub fn is_helicopter(self) -> bool {
        matches!(self, Self::Ah64 | Self::Mi24 | Self::Ch47)
    }
    /// Retail import identities. Runtime variants reuse these dependencies.
    pub const ALL: [Self; 54] = [
        Self::F18,
        Self::Rafale,
        Self::F14,
        Self::A4E,
        Self::X31,
        Self::Mig29,
        Self::Su27,
        Self::Mig21,
        Self::Su25,
        Self::Mig23,
        Self::Su35,
        Self::F22,
        Self::F22n,
        Self::C130,
        Self::Ac130,
        Self::E3,
        Self::Il76,
        Self::E2,
        Self::Av8,
        Self::Yak141,
        Self::V22,
        Self::Ah64,
        Self::Mi24,
        Self::Ch47,
        Self::Mig17,
        Self::F4B,
        Self::F4J,
        Self::F4E,
        Self::F4G,
        Self::A7,
        Self::F15,
        Self::F16C,
        Self::F104,
        Self::A10,
        Self::B747,
        Self::A310,
        Self::Mig25,
        Self::F16A,
        Self::Mig31,
        Self::M2000,
        Self::F15E,
        Self::Gripen,
        Self::F5E,
        Self::Tornado,
        Self::Mf1,
        Self::B52,
        Self::B2,
        Self::F117,
        Self::B1b,
        Self::Tu160,
        Self::F111,
        Self::Tu26,
        Self::Tu95,
        Self::Su34,
    ];
    pub fn parse(name: &str) -> Result<Self> {
        match name.to_ascii_lowercase().as_str() {
            "f18" | "f18.pt" => Ok(Self::F18),
            "rafale" | "rafale.pt" => Ok(Self::Rafale),
            "f14" | "f14.pt" => Ok(Self::F14),
            "a4e" | "a4e.pt" => Ok(Self::A4E),
            "x31" | "f31.pt" => Ok(Self::X31),
            "mig29" | "mig29.pt" => Ok(Self::Mig29),
            "su27" | "su27.pt" => Ok(Self::Su27),
            "mig21" | "mig21.pt" => Ok(Self::Mig21),
            "su25" | "su25.pt" => Ok(Self::Su25),
            "mig23" | "mig23.pt" => Ok(Self::Mig23),
            "su35" | "su35.pt" => Ok(Self::Su35),
            "faxx" | "fa-xx" | "f/a-xx" => Ok(Self::Faxx),
            "f22" | "f22.pt" => Ok(Self::F22),
            "f22n" | "f22n.pt" => Ok(Self::F22n),
            "c130" | "c130.pt" => Ok(Self::C130),
            "ac130" | "ac130.pt" => Ok(Self::Ac130),
            "e3" | "e3.pt" => Ok(Self::E3),
            "il76" | "il76.pt" => Ok(Self::Il76),
            "e2" | "e2.pt" => Ok(Self::E2),
            "av8" | "av8.pt" => Ok(Self::Av8),
            "yak141" | "yak141.pt" => Ok(Self::Yak141),
            "v22" | "v22.pt" => Ok(Self::V22),
            "ah64" | "ah64.pt" => Ok(Self::Ah64),
            "mi24" | "mi24.pt" => Ok(Self::Mi24),
            "ch47" | "ch47.pt" => Ok(Self::Ch47),
            "mig17" | "mig17f" | "mig17f.pt" => Ok(Self::Mig17),
            "f4b" | "f4b.pt" => Ok(Self::F4B),
            "f4j" | "f4j.pt" => Ok(Self::F4J),
            "f4e" | "f4e.pt" => Ok(Self::F4E),
            "f4g" | "f4" | "f4.pt" => Ok(Self::F4G),
            "a7" | "a7.pt" => Ok(Self::A7),
            "f15" | "f15.pt" => Ok(Self::F15),
            "f16c" | "f16c.pt" => Ok(Self::F16C),
            "f104" | "f104.pt" => Ok(Self::F104),
            "a10" | "a10.pt" => Ok(Self::A10),
            "b747" | "b747.pt" => Ok(Self::B747),
            "a310" | "a310.pt" => Ok(Self::A310),
            "mig25" | "m25.pt" => Ok(Self::Mig25),
            "f16a" | "f16.pt" => Ok(Self::F16A),
            "mig31" | "mig31.pt" => Ok(Self::Mig31),
            "m2000" | "m2000.pt" => Ok(Self::M2000),
            "f15e" | "f15e.pt" => Ok(Self::F15E),
            "gripen" | "gripen.pt" => Ok(Self::Gripen),
            "f5e" | "f5e.pt" => Ok(Self::F5E),
            "tornado" | "tor" | "tor.pt" | "tornado-combat" => Ok(Self::Tornado),
            "tornadoc" | "tornado-clean" | "tornado_clean" => Ok(Self::TornadoClean),
            "mf1" | "mf1.pt" => Ok(Self::Mf1),
            "b52" | "b52.pt" => Ok(Self::B52),
            "b2" | "b2.pt" => Ok(Self::B2),
            "f117" | "f117.pt" => Ok(Self::F117),
            "b1b" | "b1.pt" => Ok(Self::B1b),
            "tu160" | "tu160.pt" => Ok(Self::Tu160),
            "f111" | "f111.pt" => Ok(Self::F111),
            "tu26" | "tu26.pt" => Ok(Self::Tu26),
            "tu95" | "tu95.pt" => Ok(Self::Tu95),
            "su34" | "su34.pt" => Ok(Self::Su34),
            _ => Err(invalid(
                "supported aircraft: f18, rafale, f14, a4e, x31, mig29, su27, mig21, su25, mig23, su35, f22, f22n, faxx, c130, ac130, e3, il76, e2, av8, yak141, v22, ah64, mi24, ch47, mig17, f4b, f4j, f4e, f4g, a7, f15, f16c, f16a, f104, a10, b747, a310, mig25, mig31, m2000, f15e, gripen, f5e, tornado, tornadoc (the Tornado IDS Clean), mf1, b52, b2, f117, b1b, tu160, f111, tu26, tu95, su34",
            )),
        }
    }
    pub fn pt(self) -> &'static str {
        match self {
            Self::F18 => "F18.PT",
            Self::Rafale => "RAFALE.PT",
            Self::F14 => "F14.PT",
            Self::A4E => "A4E.PT",
            Self::X31 => "F31.PT",
            Self::Mig29 => "MIG29.PT",
            Self::Su27 => "SU27.PT",
            Self::Mig21 => "MIG21.PT",
            Self::Su25 => "SU25.PT",
            Self::Mig23 => "MIG23.PT",
            Self::Su35 => "SU35.PT",
            Self::F22 => "F22.PT",
            Self::F22n | Self::Faxx => "F22N.PT",
            Self::C130 => "C130.PT",
            Self::Ac130 => "AC130.PT",
            Self::E3 => "E3.PT",
            Self::Il76 => "IL76.PT",
            Self::E2 => "E2.PT",
            Self::Av8 => "AV8.PT",
            Self::Yak141 => "YAK141.PT",
            Self::V22 => "V22.PT",
            Self::Ah64 => "AH64.PT",
            Self::Mi24 => "MI24.PT",
            Self::Ch47 => "CH47.PT",
            Self::Mig17 => "MIG17F.PT",
            Self::F4B => "F4B.PT",
            Self::F4J => "F4J.PT",
            Self::F4E => "F4E.PT",
            Self::F4G => "F4.PT",
            Self::A7 => "A7.PT",
            Self::F15 => "F15.PT",
            Self::F16C => "F16C.PT",
            Self::F104 => "F104.PT",
            Self::A10 => "A10.PT",
            Self::B747 => "B747.PT",
            Self::A310 => "A310.PT",
            Self::Mig25 => "M25.PT",
            Self::F16A => "F16.PT",
            Self::Mig31 => "MIG31.PT",
            Self::M2000 => "M2000.PT",
            Self::F15E => "F15E.PT",
            Self::Gripen => "GRIPEN.PT",
            Self::F5E => "F5E.PT",
            Self::Tornado | Self::TornadoClean => "TOR.PT",
            Self::Mf1 => "MF1.PT",
            Self::B52 => "B52.PT",
            Self::B2 => "B2.PT",
            Self::F117 => "F117.PT",
            Self::B1b => "B1.PT",
            Self::Tu160 => "TU160.PT",
            Self::F111 => "F111.PT",
            Self::Tu26 => "TU26.PT",
            Self::Tu95 => "TU95.PT",
            Self::Su34 => "SU34.PT",
        }
    }
    pub fn hud(self) -> &'static str {
        match self {
            Self::F18 => "F18.HUD",
            Self::Rafale => "RAFALE.HUD",
            Self::F14 => "F14.HUD",
            Self::A4E => "F4.HUD",
            Self::X31 => "F31.HUD",
            Self::Mig29 => "SU33CC.HUD",
            Self::Su27 => "AV8.HUD",
            Self::Mig21 => "MIG21.HUD",
            Self::Su25 => "SU33CC.HUD",
            Self::Mig23 => "SU33CC.HUD",
            Self::Su35 => "SU35.HUD",
            Self::F22 => "F22.HUD",
            Self::F22n | Self::Faxx => "F22N.HUD",
            Self::C130 => "AC130.HUD",
            Self::Ac130 => "AC130.HUD",
            Self::E3 => "AC130.HUD",
            Self::Il76 => "AC130.HUD",
            Self::E2 => "AC130.HUD",
            Self::Av8 => "AV8.HUD",
            Self::Yak141 => "YAK141.HUD",
            Self::V22 => "AC130.HUD",
            Self::Ah64 => "AC130.HUD",
            Self::Mi24 => "SU33.HUD",
            Self::Ch47 => "AC130.HUD",
            Self::Mig17 => "MIG17.HUD",
            Self::F4B => "F4.HUD",
            Self::F4J => "F4.HUD",
            Self::F4E => "F4.HUD",
            Self::F4G => "F4.HUD",
            Self::A7 => "A7.HUD",
            Self::F15 => "AV8.HUD",
            Self::F16C => "F16C.HUD",
            Self::F104 => "F104_C.HUD",
            Self::A10 => "F104_C.HUD",
            Self::B747 => "AC130.HUD",
            Self::A310 => "AC130.HUD",
            Self::Mig25 => "SU33_C.HUD",
            Self::F16A => "F16C.HUD",
            Self::Mig31 => "YAK141.HUD",
            // M2000.PT names rafale.HUD; the game follows the record, so it
            // flies the Rafale HUD and cockpit by name (John, 2026-11-09).
            Self::M2000 => "RAFALE.HUD",
            // The batch-5 aircraft follow their records by name (B5-1). The
            // F-15E record names av8.HUD (the F-15C's too); the Gripen has its
            // own gripen.HUD; F5E.PT names f29.HUD, TOR.PT e2000.HUD and
            // MF1.PT rafale.HUD.
            // F-15E: decided, not interim (opinionated, John, 2026-11-10, F15E-7).
            // The F-5E's f29.HUD binding is decided: it follows the record and
            // flies the X-29's panel (John, 2026-11-10, opinionated; F5E-7).
            Self::F15E => "AV8.HUD",
            // Gripen: decided, not interim (opinionated, John, 2026-11-10,
            // GRI-7): its own HUD and ~GRI*.PIC cockpit family.
            Self::Gripen => "GRIPEN.HUD",
            Self::F5E => "F29.HUD",
            // Tornado: decided, follows the record and flies the Eurofighter
            // 2000's panel (opinionated, John, 2026-11-10; TOR-7).
            Self::Tornado | Self::TornadoClean => "E2000.HUD",
            // Mirage F1: decided, not interim (opinionated, John, 2026-11-10,
            // decision 25, MF1-7); the same binding as the Mirage 2000.
            Self::Mf1 => "RAFALE.HUD",
            // Bomber batch A (BMB-A1, 2026-11-15; John accepted, decisions 14,
            // 17, 28). The B-52 follows its record (the AC-130 module with a
            // BAY annunciator). B2.PT's hudName is a null pointer, so the
            // B-2 takes the retail B2.HUD by file name (the Su-35 and X-31
            // precedent), and so does the F-117 with F117.HUD.
            Self::B52 => "BMBROH.HUD",
            Self::B2 => "B2.HUD",
            Self::F117 => "F117.HUD",
            // Bomber batch B (BMB-B1, 2026-11-15; coordinator first pass,
            // BOMBERS-DECISIONS-B): each follows its record. B1.PT names
            // bmbrcc.HUD (the B-2's module, three bytes changed), TU160.PT
            // b2.HUD and F111.PT ac130.HUD.
            Self::B1b => "BMBRCC.HUD",
            Self::Tu160 => "B2.HUD",
            Self::F111 => "AC130.HUD",
            // Bomber batch C (BMB-C1, 2026-11-15; coordinator first pass,
            // BOMBERS-DECISIONS-C): each follows its record. TU26.PT and
            // TU95.PT name bmbroh.HUD (the B-52's, the AC-130 module with
            // seven bytes changed) and SU34.PT su33cc.HUD (the MiG-29's,
            // Su-25's and MiG-23's).
            Self::Tu26 | Self::Tu95 => "BMBROH.HUD",
            Self::Su34 => "SU33CC.HUD",
        }
    }
    pub fn stem(self) -> &'static str {
        match self {
            Self::F18 => "F18",
            Self::Rafale => "RAF",
            Self::F14 => "F14",
            Self::A4E => "A4",
            Self::X31 => "F31",
            Self::Mig29 => "MIG29",
            Self::Su27 => "SU27",
            Self::Mig21 => "MIG21",
            Self::Su25 => "SU25",
            Self::Mig23 => "MIG23",
            Self::Su35 => "SU35",
            Self::F22 => "F22",
            Self::F22n | Self::Faxx => "F22N",
            Self::C130 => "C130",
            Self::Ac130 => "AC130",
            Self::E3 => "AWACS",
            Self::Il76 => "IL76",
            Self::E2 => "E2C",
            Self::Av8 => "AV8",
            Self::Yak141 => "Y141",
            Self::V22 => "V22",
            Self::Ah64 => "APA",
            Self::Mi24 => "HIND",
            Self::Ch47 => "CH47",
            Self::Mig17 => "M17",
            Self::F4B => "F4J",
            Self::F4J => "F4J",
            Self::F4E => "F4E",
            Self::F4G => "F4",
            Self::A7 => "A7",
            Self::F15 => "F15",
            Self::F16C => "F16",
            Self::F104 => "F104",
            Self::A10 => "A10",
            Self::B747 => "B747",
            Self::A310 => "A310",
            Self::Mig25 => "MIG25",
            Self::F16A => "F16",
            Self::Mig31 => "MIG31",
            // M2000.PT names m20.SH, _M20.PIC and M20_A..D.SH.
            Self::M2000 => "M20",
            // F15E.PT names f15e.SH, gripen: gri.SH, F5E.PT f5e.SH, TOR.PT
            // tor.SH and MF1.PT mf1.SH, each with its own _A..D damage looks.
            Self::F15E => "F15E",
            Self::Gripen => "GRI",
            Self::F5E => "F5E",
            Self::Tornado | Self::TornadoClean => "TOR",
            Self::Mf1 => "MF1",
            // B52.PT names b52.SH, B2.PT b2.SH and F117.PT f117.SH, each with
            // its own _A..D damage looks (BMB-A1).
            Self::B52 => "B52",
            Self::B2 => "B2",
            Self::F117 => "F117",
            // B1.PT names b1.SH, TU160.PT tu160.SH and F111.PT f111.SH, each
            // with its own _A..D damage looks (BMB-B1).
            Self::B1b => "B1",
            Self::Tu160 => "TU160",
            Self::F111 => "F111",
            // TU26.PT names tu26.SH, TU95.PT tu95.SH and SU34.PT su34.SH, each
            // with its own _A..D damage looks (BMB-C1).
            Self::Tu26 => "TU26",
            Self::Tu95 => "TU95",
            Self::Su34 => "SU34",
        }
    }
    pub fn cockpit_stem(self) -> &'static str {
        match self {
            Self::A4E => "F4",
            Self::Mig29 => "SU33",
            Self::Su27 => "AV8",
            Self::Mig21 => "M21",
            Self::Su25 => "SU33",
            Self::Mig23 => "SU33",
            Self::Su35 => "SU35",
            Self::F22 | Self::F22n | Self::Faxx => "F22",

            Self::C130 => "AC130",
            Self::Ac130 => "AC130",
            Self::E3 => "AC130",
            Self::Il76 => "AC130",
            Self::E2 => "AC130",
            Self::Av8 => "AV8",
            Self::Yak141 => "Y141",
            Self::V22 => "AC130",
            Self::Ah64 => "AC130",
            Self::Mi24 => "SU33",
            Self::Ch47 => "AC130",
            Self::Mig17 => "M17",
            Self::F4B => "F4",
            Self::F4J => "F4",
            Self::F4E => "F4",
            Self::F4G => "F4",
            Self::A7 => "A7",
            Self::F15 => "AV8",
            Self::F16C => "F16",
            Self::F104 => "F104",
            Self::A10 => "F104",
            Self::B747 => "AC130",
            Self::A310 => "AC130",
            Self::Mig25 => "SU33",
            Self::F16A => "F16",
            Self::Mig31 => "Y141",
            Self::M2000 => "RAF",
            // Cockpit families follow each record's HUD name (interim, agent
            // decision 2026-11-10, pending John, batch decision round): the
            // F-15E flies the AV8 family as the F-15C does, the Gripen its own
            // ~GRI*.PIC, the F-5E the X-29's ~F29*.PIC, the Tornado the
            // Eurofighter 2000's ~EF2*.PIC and the Mirage F1 the Rafale's.
            // F-15E: decided as above (John, 2026-11-10); the same family as the F-15C.
            Self::F15E => "AV8",
            // Gripen: decided as above (John, 2026-11-10, GRI-7).
            Self::Gripen => "GRI",
            // F-5E: decided, follows the record (John, 2026-11-10,
            // opinionated; F5E-7), so no longer interim.
            Self::F5E => "F29",
            // Tornado: decided as above (John, 2026-11-10, TOR-7), no longer
            // interim; the first aircraft on the ~EF2*.PIC family.
            Self::Tornado | Self::TornadoClean => "EF2",
            // Mirage F1: decided as above (John, 2026-11-10); the Rafale family.
            Self::Mf1 => "RAF",
            // The B-52 has no cockpit art of its own and flies the AC-130
            // panel family its BMBROH.HUD names (decision 14); the B-2 and the
            // F-117 have their own ~B2*.PIC and ~F117*.PIC (BMB-A1).
            Self::B52 => "AC130",
            Self::B2 => "B2",
            Self::F117 => "F117",
            // No B-1B, Tu-160 or F-111 cockpit art exists: the first two fly
            // the B-2's, the F-111F the AC-130's (the families their HUD
            // records name; BMB-B1).
            Self::B1b | Self::Tu160 => "B2",
            Self::F111 => "AC130",
            // No Tu-26, Tu-95 or Su-34 cockpit art exists: the two Tupolevs
            // fly the AC-130's (the family their bmbroh.HUD names, as the
            // B-52), the Su-34 the Su-33's (su33cc.HUD; BMB-C1).
            Self::Tu26 | Self::Tu95 => "AC130",
            Self::Su34 => "SU33",
            _ => self.stem(),
        }
    }
    /// The instrument window frame picture for this cockpit family. Every
    /// flyable identity's HUD names exactly this picture; the app checks it.
    pub fn instrument_panel(self) -> String {
        format!("~{}_P.PIC", self.cockpit_stem())
    }
    pub fn cockpit(self) -> &'static str {
        match self {
            Self::F18 => "~F18H.PIC",
            Self::Rafale => "~RAFH.PIC",
            Self::F14 => "~F14H.PIC",
            Self::A4E => "~F4H.PIC",
            Self::X31 => "~F31H.PIC",
            Self::Mig29 => "~SU33H.PIC",
            Self::Su27 => "~AV8H.PIC",
            Self::Mig21 => "~M21H.PIC",
            Self::Su25 => "~SU33H.PIC",
            Self::Mig23 => "~SU33H.PIC",
            Self::Su35 => "~SU35H.PIC",
            Self::F22 | Self::F22n | Self::Faxx => "~F22H.PIC",
            Self::C130 => "~AC130H.PIC",
            Self::Ac130 => "~AC130H.PIC",
            Self::E3 => "~AC130H.PIC",
            Self::Il76 => "~AC130H.PIC",
            Self::E2 => "~AC130H.PIC",
            Self::Av8 => "~AV8H.PIC",
            Self::Yak141 => "~Y141H.PIC",
            Self::V22 => "~AC130H.PIC",
            Self::Ah64 => "~AC130H.PIC",
            Self::Mi24 => "~SU33H.PIC",
            Self::Ch47 => "~AC130H.PIC",
            Self::Mig17 => "~M17H.PIC",
            Self::F4B => "~F4H.PIC",
            Self::F4J => "~F4H.PIC",
            Self::F4E => "~F4H.PIC",
            Self::F4G => "~F4H.PIC",
            Self::A7 => "~A7H.PIC",
            Self::F15 => "~AV8H.PIC",
            Self::F16C => "~F16H.PIC",
            Self::F104 => "~F104H.PIC",
            Self::A10 => "~F104H.PIC",
            Self::B747 => "~AC130H.PIC",
            Self::A310 => "~AC130H.PIC",
            Self::Mig25 => "~SU33H.PIC",
            Self::F16A => "~F16H.PIC",
            Self::Mig31 => "~Y141H.PIC",
            Self::M2000 => "~RAFH.PIC",
            Self::F15E => "~AV8H.PIC",
            Self::Gripen => "~GRIH.PIC",
            Self::F5E => "~F29H.PIC",
            Self::Tornado | Self::TornadoClean => "~EF2H.PIC",
            Self::Mf1 => "~RAFH.PIC",
            Self::B52 => "~AC130H.PIC",
            Self::B2 => "~B2H.PIC",
            Self::F117 => "~F117H.PIC",
            Self::B1b | Self::Tu160 => "~B2H.PIC",
            Self::F111 => "~AC130H.PIC",
            Self::Tu26 | Self::Tu95 => "~AC130H.PIC",
            Self::Su34 => "~SU33H.PIC",
        }
    }
    /// Compact identity for lobby columns and server seat logs.
    pub fn short_label(self) -> &'static str {
        if self == Self::Rafale {
            return "Rafale C";
        }
        // The two Tornado versions share the PT's short name, so each says
        // which it is (TOR-13).
        match self {
            Self::Tornado => return "Tornado Combat",
            Self::TornadoClean => return "Tornado Clean",
            _ => {}
        }
        self.reviewed_names()
            .map(|(short, _)| short)
            .unwrap_or_else(|| {
                self.label()
                    .split_whitespace()
                    .next()
                    .unwrap_or(self.label())
            })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::F18 => "F/A-18D Hornet",
            Self::Rafale => "Rafale C",
            Self::F14 => "F-14D Tomcat",
            Self::A4E => "A-4E Skyhawk",
            Self::X31 => "X-31 EFM",
            Self::Mig29 => "MiG-29 Fulcrum-C",
            Self::Su27 => "Su-27 Flanker-B",
            Self::Mig21 => "MiG-21 Fishbed",
            Self::Su25 => "Su-25 Frogfoot-A",
            Self::Mig23 => "MiG-23 Flogger-B",
            Self::Su35 => "Su-35",
            Self::F22 => "F-22A Raptor",
            Self::F22n => "F-22N Raptor",
            Self::Faxx => "F/A-XX",
            Self::C130 => "C-130 Hercules",
            Self::Ac130 => "AC-130U Spectre",
            Self::E3 => "E-3 AWACS Sentry (AIR)",
            Self::Il76 => "IL-76 Mainstay (AIR)",
            Self::E2 => "E-2C Hawkeye (AIR)",
            Self::Av8 => "Av-8B Harrier II",
            Self::Yak141 => "Yak-141 Freestyle-A",
            Self::V22 => "V-22 Osprey",
            Self::Ah64 => "AH-64 Apache",
            Self::Mi24 => "Mi-24 Hind-D",
            Self::Ch47 => "CH-47 Chinook",
            Self::Mig17 => "MiG-17F Fresco",
            Self::F4B => "F-  4B Phantom II",
            Self::F4J => "F-  4J Phantom II",
            Self::F4E => "F-  4E (Desert) Phantom",
            Self::F4G => "F-  4G Wild Weasel Phantom",
            Self::A7 => "A- 7E Corsair II",
            Self::F15 => "F- 15C Eagle",
            Self::F16C => "F- 16C Falcon",
            Self::F104 => "F-104N Starfighter",
            Self::A10 => "A-10 Thunderbolt",
            Self::B747 => "Boeing 747",
            Self::A310 => "Airbus 310",
            Self::Mig25 => "MiG-25 Foxbat",
            Self::F16A => "F- 16A Falcon",
            Self::Mig31 => "MiG-31 Foxhound",
            Self::M2000 => "Mirage 2000",
            Self::F15E => "F- 15E Strike Eagle",
            Self::Gripen => "Saab Gripen",
            Self::F5E => "F-  5E Tiger",
            Self::Tornado => "Tornado IDS (Combat)",
            Self::TornadoClean => "Tornado IDS (Clean)",
            Self::Mf1 => "Mirage F1",
            Self::B52 => "B-52 Stratofortress",
            // Retail's own spelling, stray space and all (decision 15).
            Self::B2 => "B- 2 Spirit",
            Self::F117 => "F-117A Nighthawk",
            // Retail's own spellings (stray space in the B-1B's).
            Self::B1b => "B- 1B Lancer",
            Self::Tu160 => "Tu-160 Blackjack-A",
            Self::F111 => "F-111F Aardvark",
            // Retail's own spellings (stray space in the Tu-26's and Tu-95's).
            Self::Tu26 => "Tu- 26 Backfire-C",
            Self::Tu95 => "Tu- 95 Bear-H",
            Self::Su34 => "Su-34 Platypus",
        }
    }
    /// Exact reviewed short/long retail names, absent for the original loose fixtures.
    pub fn reviewed_names(self) -> Option<(&'static str, &'static str)> {
        match self.source() {
            Self::F14 => Some(("F-14", "F- 14D Tomcat")),
            Self::A4E => Some(("A-4E", "A- 4E Skyhawk")),
            Self::X31 => Some(("X-31", "X-31 EFM")),
            Self::Mig29 => Some(("MiG-29", "MiG-29 Fulcrum-C")),
            Self::Su27 => Some(("Su-27", "Su-27 Flanker-B")),
            Self::Mig21 => Some(("MiG-21", "MiG-21 Fishbed")),
            Self::Su25 => Some(("Su-25", "Su-25 Frogfoot-A")),
            Self::Mig23 => Some(("MiG-23", "MiG-23 Flogger-B")),
            Self::Su35 => Some(("Su-35", "Su-35")),
            Self::F22 => Some(("F-22", "F- 22A Raptor")),
            Self::F22n => Some(("F-22", "F- 22N Raptor")),

            Self::C130 => Some(("C-130", "C-130 Hercules")),
            Self::Ac130 => Some(("AC-130U", "AC-130U Spectre")),
            Self::E3 => Some(("E-3", "E-3 AWACS Sentry (AIR)")),
            Self::Il76 => Some(("IL-76", "IL-76 Mainstay (AIR)")),
            Self::E2 => Some(("E-2C", "E-2C Hawkeye (AIR)")),
            Self::Av8 => Some(("Av-8", "Av-8B Harrier II")),
            Self::Yak141 => Some(("Yak-141", "Yak-141 Freestyle-A")),
            Self::V22 => Some(("V-22", "V-22 Osprey")),
            Self::Ah64 => Some(("AH-64", "AH-64 Apache")),
            Self::Mi24 => Some(("Mi-24", "Mi-24 Hind-D")),
            Self::Ch47 => Some(("CH-47", "CH-47 Chinook")),
            Self::Mig17 => Some(("MiG-17", "MiG-17F Fresco")),
            Self::F4B => Some(("F-4B", "F-  4B Phantom II")),
            Self::F4J => Some(("F-4J", "F-  4J Phantom II")),
            Self::F4E => Some(("F-4", "F-  4E (Desert) Phantom")),
            Self::F4G => Some(("F-4G", "F-  4G Wild Weasel Phantom")),
            Self::A7 => Some(("A-7", "A- 7E Corsair II")),
            Self::F15 => Some(("F-15", "F- 15C Eagle")),
            Self::F16C => Some(("F-16C", "F- 16C Falcon")),
            Self::F104 => Some(("F-104", "F-104N Starfighter")),
            Self::A10 => Some(("A-10", "A-10 Thunderbolt")),
            Self::B747 => Some(("B747", "Boeing 747")),
            Self::A310 => Some(("A310", "Airbus 310")),
            Self::Mig25 => Some(("MiG-25", "MiG-25 Foxbat")),
            Self::F16A => Some(("F-16", "F- 16A Falcon")),
            Self::Mig31 => Some(("MiG-31", "MiG-31 Foxhound")),
            Self::M2000 => Some(("M2000", "Mirage 2000")),
            Self::F15E => Some(("F-15E", "F- 15E Strike Eagle")),
            Self::Gripen => Some(("Gripen", "Saab Gripen")),
            Self::F5E => Some(("F-5", "F-  5E Tiger")),
            Self::Tornado => Some(("Tornado", "Tornado IDS")),
            Self::Mf1 => Some(("MF1", "Mirage F1")),
            Self::B52 => Some(("B-52", "B-52 Stratofortress")),
            Self::B2 => Some(("B-2", "B- 2 Spirit")),
            Self::F117 => Some(("F-117", "F-117A Nighthawk")),
            Self::B1b => Some(("B-1B", "B- 1B Lancer")),
            Self::Tu160 => Some(("Tu-160", "Tu-160 Blackjack-A")),
            Self::F111 => Some(("F-111F", "F-111F Aardvark")),
            Self::Tu26 => Some(("Tu-26", "Tu- 26 Backfire-C")),
            Self::Tu95 => Some(("Tu-95", "Tu- 95 Bear-H")),
            Self::Su34 => Some(("Su-34", "Su-34 Platypus")),
            _ => None,
        }
    }
    /// Installed source radar, absent when the PT has no radar channel.
    pub fn radar(self) -> Option<&'static str> {
        match self {
            Self::F18 => Some("F18R.SEE"),
            Self::Rafale => Some("F18R.SEE"),
            Self::F14 => Some("F14R.SEE"),
            Self::A4E => Some("F4BR.SEE"),
            Self::X31 => Some("F18R.SEE"),
            Self::Mig29 => Some("MIG29R.SEE"),
            Self::Su27 => Some("SU27R.SEE"),
            Self::Mig21 => Some("MIG21R.SEE"),
            Self::Su25 => Some("SU24R.SEE"),
            Self::Mig23 => Some("MIG27R.SEE"),
            Self::Su35 => Some("SU27R.SEE"),
            Self::F22 => Some("F22R.SEE"),
            Self::F22n => Some("F22R.SEE"),
            Self::Faxx => Some("F22R.SEE"),
            Self::C130 => None,
            Self::Ac130 => Some("AC130R.SEE"),
            Self::E3 => Some("E3R.SEE"),
            Self::Il76 => Some("E3R.SEE"),
            Self::E2 => Some("E2R.SEE"),
            Self::Av8 => Some("AV8R.SEE"),
            Self::Yak141 => Some("YAK141R.SEE"),
            Self::V22 => None,
            Self::Ah64 => Some("F18R.SEE"),
            Self::Mi24 => None,
            Self::Ch47 => None,
            Self::Mig17 => None,
            Self::F4B => Some("F4BR.SEE"),
            Self::F4J => Some("F4JR.SEE"),
            Self::F4E => Some("F18R.SEE"),
            Self::F4G => Some("F18R.SEE"),
            Self::A7 => Some("A7R.SEE"),
            Self::F15 => Some("F15R.SEE"),
            Self::F16C => Some("F18R.SEE"),
            Self::F104 => Some("F104R.SEE"),
            Self::A10 => Some("A10R.SEE"),
            Self::B747 => None,
            Self::A310 => None,
            Self::Mig25 => Some("MIG27R.SEE"),
            Self::F16A => Some("F18R.SEE"),
            Self::Mig31 => Some("MIG31R.SEE"),
            Self::M2000 => Some("F18R.SEE"),
            Self::F15E => Some("F15R.SEE"),
            Self::Gripen => Some("F18R.SEE"),
            Self::F5E => Some("F104R.SEE"),
            Self::Tornado | Self::TornadoClean => Some("F18R.SEE"),
            Self::Mf1 => Some("F18R.SEE"),
            Self::B52 => Some("B52R.SEE"),
            Self::B2 => Some("F18R.SEE"),
            // F117.PT has no radar row (VIS340, Pave Spike, F18.ECM, AV8IR).
            Self::F117 => None,
            Self::B1b => Some("F18R.SEE"),
            Self::Tu160 => Some("TU160R.SEE"),
            Self::F111 => Some("A6R.SEE"),
            Self::Tu26 => Some("TU26R.SEE"),
            Self::Tu95 => Some("TU95R.SEE"),
            Self::Su34 => Some("SU27R.SEE"),
        }
    }
    /// The F-117's own names for the two sensors its record shares with every
    /// other aircraft (F117-7, 2026-11-15; John, decision 26, opinionated):
    /// the `PAVESPK.SEE` row is its IRADS turret (the laser designator the
    /// laser bombs need) and the `AV8IR.SEE` row is its FLIR. Text only: the
    /// records, the sight and the channel behave as the shared ones. `None`
    /// for every other aircraft, which keep the generic labels.
    ///
    /// The F-111F (F111-7, 2026-11-16; coordinator first pass, F111 item 6)
    /// shows its laser and FLIR pair as the PAVE TACK pod, the way the F-117
    /// shows IRADS: the `PAVESPK.SEE` laser (its fixed row and the pod on its
    /// bay row, a second record of the same laser) is the Pave Tack's laser
    /// and `AV8IR.SEE` is its FLIR. Text only again.
    pub fn sensor_names(self) -> Option<SensorNames> {
        match self {
            Self::F117 => Some(SensorNames {
                sight: "IRADS",
                infrared: "FLIR",
                pod: "IRADS",
            }),
            Self::F111 => Some(SensorNames {
                sight: "PAVE TACK",
                infrared: "FLIR",
                pod: "Pave Tack",
            }),
            _ => None,
        }
    }
    /// Primary installed gun. Multi-gun identities expose every type in `guns`.
    pub fn gun(self) -> Option<&'static str> {
        self.guns().first().copied()
    }
    /// Gun pods this aircraft can carry in place of a store, beyond the guns
    /// it is built with (the F-4B's centreline SUU-16, 2026-10-16): the weapon
    /// records of [`Self::guns`]'s kind that a loadout may put on the
    /// [`stations::gun_pod_row`].
    pub fn gun_pods(self) -> &'static [&'static str] {
        match self {
            Self::F4B => &["SUU16.JT"],
            _ => &[],
        }
    }
    /// Whether `source` is a gun on this aircraft: one it is built with
    /// ([`Self::guns`], the F-4J's default SUU-16 pod among them) or a pod it
    /// can carry ([`Self::gun_pods`]). Guns only keeps exactly these.
    pub fn mounts_gun(self, source: &str) -> bool {
        self.guns().contains(&source) || self.gun_pods().contains(&source)
    }
    /// Source gun types. An empty slice means the aircraft is unarmed with guns.
    pub fn guns(self) -> &'static [&'static str] {
        match self {
            Self::F18 => &["M61.JT"],
            Self::Rafale => &["DEFA.JT"],
            Self::F14 => &["M61.JT"],
            Self::A4E => &["MK12.JT"],
            Self::X31 => &["M61.JT"],
            Self::Mig29 => &["GSH301.JT"],
            Self::Su27 => &["GSH301.JT"],
            Self::Mig21 => &["GSH23.JT"],
            Self::Su25 => &["GSH301.JT"],
            Self::Mig23 => &["GSH6_30.JT"],
            Self::Su35 => &["GSH301.JT"],
            Self::F22 => &["M61.JT"],
            Self::F22n => &["M61.JT"],
            Self::Faxx => &["M61.JT"],
            Self::C130 => &[],
            Self::Ac130 => &["C_25.JT", "C_40.JT", "C_105.JT"],
            Self::E3 => &[],
            Self::Il76 => &[],
            Self::E2 => &[],
            Self::Av8 => &["GAU12.JT"],
            Self::Yak141 => &["GSH30.JT"],
            Self::V22 => &["T30_1.JT"],
            Self::Ah64 => &["M61.JT"],
            Self::Mi24 => &["T12_4.JT"],
            Self::Ch47 => &[],
            Self::Mig17 => &["GSH30.JT", "GSH23.JT"],
            Self::F4B => &[],
            Self::F4J => &["SUU16.JT"],
            Self::F4E => &["M61.JT"],
            Self::F4G => &["M61.JT"],
            Self::A7 => &["M61.JT"],
            Self::F15 => &["M61.JT"],
            Self::F16C => &["M61.JT"],
            Self::F104 => &["M61.JT"],
            Self::A10 => &["GAU8.JT"],
            Self::B747 => &[],
            Self::A310 => &[],
            Self::Mig25 => &[],
            Self::F16A => &["M61.JT"],
            Self::Mig31 => &["GSH6_30.JT"],
            Self::M2000 => &["DEFA.JT"],
            Self::F15E => &["M61.JT"],
            // BK27.JT is not a live weapon yet (the ordnance slices own it).
            // Update (GRI-6, 2026-11-10): listing it here is what makes it the
            // Gripen's working internal gun (`is_gun`, Tab, tracers, sound, the
            // live-fire probe); it still stays out of SUPPORTED_WEAPONS.
            Self::Gripen => &["BK27.JT"],
            Self::F5E => &["M61.JT"],
            Self::Tornado | Self::TornadoClean => &["BK27.JT"],
            Self::Mf1 => &["DEFA.JT"],
            // The B-52H's M61 tail gun (decision 13); the B-2 and F-117 carry
            // no gun.
            Self::B52 => &["M61.JT"],
            Self::B2 | Self::F117 => &[],
            // The B-1B, Tu-160 and F-111F records carry no gun row.
            Self::B1b | Self::Tu160 | Self::F111 => &[],
            // Bomber batch C (BMB-C1): the Tu-26 and Tu-95 tail turrets fire
            // the 23 mm `T23_1.JT` (row 3, and rows 5 and 6 on the Tu-95 until
            // BMB-C2 clears them); the Su-34 keeps the Su-27's GSh-30-1.
            Self::Tu26 | Self::Tu95 => &["T23_1.JT"],
            Self::Su34 => &["GSH301.JT"],
        }
    }
    /// Reviewed optional left, centre and right cockpit overlays.
    pub fn cockpit_overlays(self) -> [Option<String>; 3] {
        let available = match self.cockpit_stem() {
            // The B-2 and F-117 cockpit art has no side or centre overlay
            // pictures (`~B2_LH.PIC` and `~F117_LH.PIC` do not exist; the
            // review sweeps found only the four panel pictures; BMB-A1).
            "AC130" | "M17" | "F16" | "F31" | "M21" | "F22" | "B2" | "F117" => [false; 3],
            "SU33" | "Y141" | "F104" => [true, false, true],
            _ => [true; 3],
        };
        std::array::from_fn(|i| {
            available[i].then(|| format!("~{}_{}H.PIC", self.cockpit_stem(), ["L", "C", "R"][i]))
        })
    }
    /// Whether this reviewed source identity supplies an inert PTS companion.
    pub fn has_pts(self) -> bool {
        !matches!(
            self.source(),
            Self::Mig29
                | Self::Su27
                | Self::Su25
                | Self::Mig23
                | Self::C130
                | Self::E3
                | Self::Il76
                | Self::E2
                | Self::V22
                | Self::Ah64
                | Self::Mi24
                | Self::Ch47
                | Self::F4E
                | Self::F4G
                | Self::F15
                | Self::A10
                | Self::B747
                | Self::A310
                | Self::Mig25
                | Self::F16A
                | Self::Mig31
                | Self::M2000
                | Self::F15E
                | Self::F5E
                | Self::Tornado
                | Self::Mf1
                | Self::B52
                | Self::B1b
                | Self::Tu160
                | Self::F111
                | Self::Tu26
                | Self::Tu95
                | Self::Su34
        )
    }
}
#[derive(Debug)]
pub struct Aircraft {
    pub id: AircraftId,
    pub name: String,
    pub shape: String,
    pub fields: BTreeMap<String, Token>,
    pub object: BTreeMap<String, Token>,
    pub hardpoints: Vec<Hardpoint>,
    pub envelopes: Vec<Envelope>,
    pub sounds: BTreeMap<String, String>,
}
impl Aircraft {
    /// This record as runtime identity `id`, which must be the identity it was
    /// parsed for or one of its variants ([`AircraftId::source`]). The F/A-XX
    /// is its donor's record unchanged; the Tornado IDS (Clean) loses its
    /// jammer row ([`stations::variant`], TOR-13). Parsing a record never
    /// yields a variant, so every reader that flies or loads an identity takes
    /// it through here (or [`Aircraft::parse_as`]).
    pub fn into_identity(mut self, id: AircraftId) -> Result<Self> {
        if self.id != id && self.id != id.source() {
            return Err(invalid("aircraft record is not this identity's"));
        }
        self.id = id;
        stations::variant(id, &mut self.hardpoints);
        Ok(self)
    }
    /// [`Aircraft::parse`] of the record `data` for identity `id`
    /// ([`Aircraft::into_identity`]).
    pub fn parse_as(data: &[u8], id: AircraftId) -> Result<Self> {
        Self::parse(data)?.into_identity(id)
    }
    pub fn parse(data: &[u8]) -> Result<Self> {
        let b = Brf::parse(data)?;
        let root = b.block("")?;
        let o = schema::OBJECT.len();
        let n = schema::NPC.len();
        if root.len() != o + n + schema::PLANE.len() {
            return Err(invalid("unsupported aircraft BRF layout"));
        }
        let mut object = fields(&root[..o], schema::OBJECT)?;
        let npc = fields(&root[o..o + n], schema::NPC)?;
        let mut plane = fields(&root[o + n..], schema::PLANE)?;
        let names = b.strings("ot_names")?;
        if names.len() != 3 {
            return Err(invalid("invalid aircraft identity"));
        }
        let id = AircraftId::parse(&names[2])?;
        // Plainly copied records get published figures (slice HF3).
        corrections::apply(id, &mut object, &mut plane);
        let size = match id {
            AircraftId::F14 | AircraftId::F22 | AircraftId::F22n | AircraftId::Mig29 => 636,
            AircraftId::A4E | AircraftId::Mig21 => 612,
            AircraftId::C130 => 468,
            AircraftId::Ac130 => 612,
            AircraftId::E3 => 516,
            AircraftId::Il76 => 516,
            AircraftId::E2 => 516,
            AircraftId::Av8 => 660,
            AircraftId::Yak141 => 588,
            AircraftId::V22 => 540,
            AircraftId::Ah64 => 636,
            AircraftId::Mi24 => 516,
            AircraftId::Ch47 => 468,
            AircraftId::Mig17 => 540,
            AircraftId::F4B => 612,
            AircraftId::F4J => 636,
            AircraftId::F4E => 636,
            AircraftId::F4G => 636,
            AircraftId::A7 => 612,
            AircraftId::F15 => 588,
            AircraftId::F16C => 660,
            AircraftId::F104 => 660,
            AircraftId::A10 => 660,
            AircraftId::B747 => 468,
            AircraftId::A310 => 468,
            AircraftId::Mig25 => 588,
            AircraftId::F16A => 660,
            AircraftId::Mig31 => 588,
            AircraftId::M2000 => 660,
            // Record sizes read in the batch-5 source reviews (2026-11-10).
            AircraftId::F15E | AircraftId::Gripen | AircraftId::Tornado => 660,
            AircraftId::F5E => 612,
            AircraftId::Mf1 => 636,
            // Bomber batch A source reviews (2026-11-15): B52.PT 612 (the
            // AC-130's), B2.PT 660, F117.PT 588 (the MiG-31's).
            AircraftId::B52 => 612,
            AircraftId::B2 => 660,
            AircraftId::F117 => 588,
            // Bomber batch B source reviews (2026-11-15): B1.PT 636,
            // TU160.PT 588, F111.PT 660.
            AircraftId::B1b => 636,
            AircraftId::Tu160 => 588,
            AircraftId::F111 => 660,
            // Bomber batch C source reviews (2026-11-15): TU26.PT, TU95.PT and
            // SU34.PT are all 660.
            AircraftId::Tu26 | AircraftId::Tu95 | AircraftId::Su34 => 660,
            _ => 660,
        };
        if object["structType"].number()? != 5 || object["typeSize"].number()? != size {
            return Err(invalid(
                "unexpected FA aircraft type size for selected identity",
            ));
        }
        if !names[2].eq_ignore_ascii_case(id.pt()) {
            return Err(invalid("aircraft identity must name its PT resource"));
        }
        let expected_names = id.reviewed_names();
        if expected_names.is_some_and(|(short, long)| names[0] != short || names[1] != long) {
            return Err(invalid("unreviewed FA aircraft variant"));
        }
        let resolve = |t: &Token| -> Result<Option<String>> {
            if t.kind == "ptr" {
                let s = b.strings(&t.value)?;
                if s.len() != 1 {
                    return Err(invalid("expected single resource reference"));
                }
                Ok(Some(s[0].to_ascii_uppercase()))
            } else {
                Ok(None)
            }
        };
        let h = b.block("hards")?;
        let count = npc["numHards"].number()?;
        if !(1..=64).contains(&count) || h.len() != count as usize * 12 {
            return Err(invalid("invalid hardpoint count"));
        }
        let mut hardpoints = Vec::new();
        for row in h.chunks_exact(12) {
            let f = fields(row, schema::HARDPOINT)?;
            hardpoints.push(Hardpoint {
                location: u8::try_from(f["name"].number()?)
                    .map_err(|_| invalid("hardpoint location exceeds byte"))?,
                flags: f["flags"].number()?,
                position: [
                    f["pos.x"].number()?,
                    f["pos.y"].number()?,
                    f["pos.z"].number()?,
                ],
                store: resolve(&f["defaultTypeName"])?,
                count: f["maxItems"].number()?,
                weight_class: f["maxWeight"].number()?,
                slew: [
                    f["slewH"].number()?,
                    f["slewP"].number()?,
                    f["slewLimitH"].number()?,
                    f["slewLimitP"].number()?,
                ],
            });
        }
        stations::append(id, &mut hardpoints);
        stations::retire(id, &mut hardpoints);
        stations::side_rail(id, &mut hardpoints);
        stations::adjust_defaults(id, &mut hardpoints);
        let (min, max) = (plane["envMin"].number()?, plane["envMax"].number()?);
        let env = b.block("env")?;
        if min > 0 || max < 1 || min < -20 || max > 30 || env.len() != (max - min + 1) as usize * 44
        {
            return Err(invalid("invalid G envelope range"));
        }
        let mut envelopes = Vec::new();
        for (i, row) in env.chunks_exact(44).enumerate() {
            fields(row, schema::ENVELOPE)?;
            let g = row[0].number()?;
            let count = row[1].number()?;
            if g != min + i as i32 || !(3..=20).contains(&count) {
                return Err(invalid("invalid envelope row"));
            }
            let mut points = Vec::new();
            for j in 0..count as usize {
                let s = row[4 + j * 2].number()?;
                let a = row[5 + j * 2].number()?;
                if s < 0 || !(0..=200000).contains(&a) {
                    return Err(invalid("invalid flight envelope point"));
                }
                points.push([s as f64, a as f64]);
            }
            envelopes.push(Envelope { g, points });
        }
        // The Mirage 2000's +9 G limit (M2K-11): one range for every reader.
        corrections::raise_g_limit(id, &mut plane, &mut envelopes);
        // The batch aircraft's G limits and the F-5E's ceiling (B5-2).
        corrections::fit_envelopes(id, &object, &mut plane, &mut envelopes);
        let mut sounds = BTreeMap::new();
        for name in [
            "loopSound",
            "secondSound",
            "engineOnSound",
            "engineOffSound",
        ] {
            if let Some(s) = resolve(&object[name])? {
                sounds.insert(name.into(), s);
            }
        }
        for (map, key) in [
            (&object, "weight"),
            (&plane, "internalFuel"),
            (&plane, "thrust"),
            (&plane, "maxTakeoffWeight"),
        ] {
            if map[key].number()? <= 0 {
                return Err(invalid("nonpositive aircraft mass/thrust"));
            }
        }
        // The empty aircraft must leave room under the maximum takeoff weight.
        // A full tank may pass it (the B-52H's real 312,197 lb of fuel does,
        // BMB-A2): the loadout caps the fuel it starts with
        // (`Loadout::new`) and refuses a takeoff over the maximum.
        if object["weight"].number()? as i64 >= plane["maxTakeoffWeight"].number()? as i64 {
            return Err(invalid("aircraft empty weight reaches MTOW"));
        }
        Ok(Self {
            id,
            name: names[0].clone(),
            shape: resolve(&object["shape"])?.ok_or_else(|| invalid("missing aircraft shape"))?,
            fields: plane,
            object,
            hardpoints,
            envelopes,
            sounds,
        })
    }
    pub fn number(&self, key: &str) -> f64 {
        self.fields
            .get(key)
            .or_else(|| self.object.get(key))
            .and_then(|t| t.number().ok())
            .unwrap_or(0) as f64
    }
}
/// Literal candidate discovery for bounded data modules; presence must be checked in archive catalog.
pub fn references(bytes: &[u8]) -> BTreeSet<String> {
    bytes
        .split(|b| !b.is_ascii_alphanumeric() && !b"_~^&.$-".contains(b))
        .filter(|s| s.len() > 2 && s.len() <= 32)
        .filter_map(|s| std::str::from_utf8(s).ok())
        .map(str::to_ascii_uppercase)
        .collect()
}
/// Same dependency closure for CLI and app, independent of Python/reference checkout.
pub fn dependencies(
    archives: &[&Archive],
    aircraft: &[AircraftId],
    weapons: bool,
) -> Result<BTreeSet<String>> {
    Ok(dependency_report(archives, aircraft, weapons)?.resources)
}

/// FA GRAPHICInit (0x442c00) and shared effect audio references.
/// Filenames only: all bytes are read from the user's media at runtime.
pub const COMBAT_RESOURCES: &[&str] = &[
    "&BOMB.11K", // Store/tank jettison, independent of the loaded weapons.
    "CRATER.SH",
    "SMOKE.SH",
    "FIRE.SH",
    "EXP.SH",
    "DEBRIS.SH",
    "CHAFF.SH",
    "FLARE.SH",
    "SPD.SH",
    "MPD.SH",
    "LPD.SH",
    "&EXPL3.5K",
    "&EXPL7.5K",
    "&EXPL9.5K",
    "&EXPL10.5K",
    "&EXPL12.5K",
    "&SPLASH3.11K",
    // The explosion table's recordings (docs/spec/explosions.md).
    "&MEDEXP1.5K",
    "&MEDEXP2.5K",
    "&AIREXP1.11K",
    "&AIREXP2.11K",
    "&AIREXP3.11K",
    "&AIREXP4.11K",
    "&AIREXP5.11K",
    "&BIGEXP1.5K",
    "&BIGEXP2.5K",
    "&WTREXP1.5K",
    "&WTREXP2.5K",
    "&BULLTS1.5K",
    "&BULLTS2.8K",
    "&BULLTS3.5K",
    "&BULLTS4.5K",
    "&EMPEXP.11K",
    // Explosion, crater and fire art (EXP.SH, CRATER.SH, FIRE.SH).
    "AIRSML.PIC",
    "AIRMED.PIC",
    "AIRMED2.PIC",
    "AIRMED3.PIC",
    "AIRLRG.PIC",
    "GRNDSML.PIC",
    "GRNDMED.PIC",
    "GRNDMED3.PIC",
    "GRNDLRG.PIC",
    "GRNDLRG2.PIC",
    "WATSML.PIC",
    "WATLRG.PIC",
    "FLAKA.PIC",
    "FLAKB.PIC",
    "FLAKC.PIC",
    "EMPEX.PIC",
    "AIRLRGAG.PIC",
    "AIRLRGC.PIC",
    "AIRLRGD.PIC",
    "AIRSMLA.PIC",
    "AIRSMLB2.PIC",
    "GRDLRGA.PIC",
    "DIRTEXP.PIC",
    "CRATERS.PIC",
    "FIREA.PIC",
    "&FIRE.5K",
    "&CHAFF.5K",
    "&FLARE.5K",
    "&IRTRY.5K",
    "&IRLOCK.5K",
    "&IR1.11K",
    "&AIRPASS.11K",
    "&MPASS.5K",
    "&SNCBOOM.11K",
    "&RDRTRY.5K",
    "&RDRLOCK.5K",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyEdge {
    pub source: String,
    pub target: String,
    pub kind: &'static str,
    pub available: bool,
}

#[derive(Debug, Default)]
pub struct DependencyReport {
    pub resources: BTreeSet<String>,
    pub edges: BTreeSet<DependencyEdge>,
    /// Resource -> archive indices in caller order; last supplies dependency reads.
    pub providers: BTreeMap<String, Vec<usize>>,
}

/// A report of discoverable edges, not proof of complete native dependencies.
pub fn dependency_report(
    archives: &[&Archive],
    aircraft: &[AircraftId],
    weapons: bool,
) -> Result<DependencyReport> {
    let catalog: BTreeSet<String> = archives
        .iter()
        .flat_map(|a| a.entries.keys().cloned())
        .collect();
    let mut selected = BTreeSet::new();
    for &id in aircraft {
        for n in [
            id.pt(),
            id.hud(),
            &format!("{}.SH", id.stem()),
            "PALETTE.PAL",
            id.cockpit(),
            &id.instrument_panel(),
            "WIN11.FNT",
            "HUD11.FNT",
            "FMENUD.MNU",
        ] {
            if !catalog.contains(n) {
                return Err(invalid(&format!("aircraft import missing {n}")));
            }
            selected.insert(n.into());
        }
        // These reviewed FA profiles have no compiled PTS companion. Runtime
        // flight and editable loadouts use PT data, never the inert PTS code.
        if id.has_pts() {
            let pts = id.pt().replace(".PT", ".PTS");
            if !catalog.contains(&pts) {
                return Err(invalid(&format!("aircraft import missing {pts}")));
            }
            selected.insert(pts);
        }
        for n in &catalog {
            if n.starts_with("&GEAR")
                || n.starts_with("&FLAP")
                || n.starts_with("&STALL")
                || n == "&HOOK.5K"
                || n == "&WIND.11K"
                || n == "&SQUEAL.5K"
                || n.starts_with(&format!("~{}", id.cockpit_stem()))
                || n.starts_with(&format!("{}_", id.stem()))
                || n.starts_with(&format!("_{}", id.stem()))
                || (n.starts_with("WIN") || n.starts_with("HUD")) && n.ends_with(".FNT")
            {
                selected.insert(n.clone());
            }
        }
    }
    if weapons {
        if !catalog.contains("PALETTE.PAL") {
            return Err(invalid("combat profile missing PALETTE.PAL"));
        }
        selected.extend(
            catalog
                .iter()
                .filter(|n| {
                    [".JT", ".SEE", ".ECM", ".GAS"]
                        .iter()
                        .any(|ext| n.ends_with(ext))
                })
                .cloned(),
        );
        selected.insert("PALETTE.PAL".into());
    }
    if weapons || !aircraft.is_empty() {
        for &name in COMBAT_RESOURCES {
            if !catalog.contains(name) {
                return Err(invalid(&format!("FA combat profile missing {name}")));
            }
            selected.insert(name.into());
        }
    }
    let mut edges = BTreeSet::new();
    // These reviewed editable rows lack an appropriate default-JT reference.
    // The host retains a real compatible selection at zero count; it must be
    // readable even in aircraft-only profiles. This is not a retail PT pointer.
    for &id in aircraft {
        let seed = match id {
            AircraftId::Mig17 => Some((3, "MK82.JT")),
            AircraftId::Mig23 => Some((5, "AIM9M.JT")),
            _ => None,
        };
        if let Some((hardpoint, name)) = seed {
            if !catalog.contains(name) {
                return Err(invalid(&format!(
                    "{} -> host retained selection at hardpoint {hardpoint} -> missing dependency {name}",
                    id.pt()
                )));
            }
            selected.insert(name.into());
            edges.insert(DependencyEdge {
                source: id.pt().into(),
                target: name.into(),
                kind: "host-retained-station-seed",
                available: true,
            });
        }
    }
    for name in &selected {
        edges.insert(DependencyEdge {
            source: if COMBAT_RESOURCES.contains(&name.as_str()) {
                "@FA-combat-effects"
            } else {
                "@selected-profile"
            }
            .into(),
            target: name.clone(),
            kind: "profile-root",
            available: true,
        });
    }
    let mut pending: Vec<_> = selected.iter().cloned().collect();
    while let Some(name) = pending.pop() {
        if ![".PT", ".PTS", ".JT", ".SEE", ".ECM", ".GAS", ".SH", ".HUD"]
            .iter()
            .any(|e| name.ends_with(e))
        {
            continue;
        }
        let a = archives
            .iter()
            .rev()
            .find(|a| a.entries.contains_key(&name))
            .ok_or_else(|| invalid("missing dependency"))?;
        let bytes = a.read(&name)?;
        // Reviewed GAS records carry names and mass, not an icon pointer.
        // Their original menu art uses the same generated filename as the UI.
        if let Some(stem) = name.strip_suffix(".GAS") {
            let icon = format!("${stem}.PIC");
            let available = catalog.contains(&icon);
            edges.insert(DependencyEdge {
                source: name.clone(),
                target: icon.clone(),
                kind: "generated-tank-thumbnail",
                available,
            });
            if available && selected.insert(icon.clone()) {
                pending.push(icon);
            }
        }
        let refs = references(&bytes);
        // BRF strings and symbols are typed edges. Never execute a symbol.
        if bytes.starts_with(b"[brent's_relocatable_format]") {
            let brf = Brf::parse(&bytes)?;
            for t in brf
                .blocks
                .values()
                .flatten()
                .filter(|t| t.kind == "string" || t.kind == "symbol")
            {
                let n = t.value.to_ascii_uppercase();
                if t.kind == "symbol" {
                    edges.insert(DependencyEdge {
                        source: name.clone(),
                        target: t.value.clone(),
                        kind: "native-symbol-unimplemented",
                        available: false,
                    });
                    continue;
                }
                if [".JT", ".SH", ".SEE", ".ECM", ".GAS", ".11K", ".5K", ".PIC"]
                    .iter()
                    .any(|e| n.ends_with(e))
                {
                    if !catalog.contains(&n) {
                        return Err(invalid(&format!(
                            "{name} -> BRF string -> missing dependency {n}"
                        )));
                    }
                    edges.insert(DependencyEdge {
                        source: name.clone(),
                        target: n,
                        kind: "brf-string",
                        available: true,
                    });
                }
            }
        }
        for r in refs {
            // Explicit module resource names are required even when absent from the catalog.
            if [".SH", ".PIC", ".11K", ".5K"]
                .iter()
                .any(|e| r.ends_with(e))
                && !catalog.contains(&r)
            {
                // PTS is an unreviewed compiled module, not a BRF loadout.
                // Its literal icon names can be absent in the retail catalog.
                if name.ends_with(".PTS") {
                    edges.insert(DependencyEdge {
                        source: name.clone(),
                        target: r,
                        kind: "unresolved-module-candidate",
                        available: false,
                    });
                    continue;
                }
                return Err(invalid(&format!(
                    "{name} -> literal resource -> missing dependency {r}"
                )));
            }
            for candidate in [r.clone(), format!("{r}.PIC")] {
                if catalog.contains(&candidate) {
                    edges.insert(DependencyEdge {
                        source: name.clone(),
                        target: candidate.clone(),
                        kind: "literal-candidate",
                        available: true,
                    });
                    if selected.insert(candidate.clone()) {
                        pending.push(candidate);
                    }
                }
            }
            if r.starts_with('~')
                && !r.contains('.')
                && !catalog.contains(&r)
                && !catalog.contains(&format!("{r}.PIC"))
            {
                edges.insert(DependencyEdge {
                    source: name.clone(),
                    target: r,
                    kind: "unresolved-art-candidate",
                    available: false,
                });
            }
        }
        if selected.len() > 4096 || edges.len() > 32768 {
            return Err(invalid("aircraft dependency limit exceeded"));
        }
    }
    let providers = selected
        .iter()
        .map(|name| {
            (
                name.clone(),
                archives
                    .iter()
                    .enumerate()
                    .filter_map(|(i, a)| a.entries.contains_key(name).then_some(i))
                    .collect(),
            )
        })
        .collect();
    Ok(DependencyReport {
        resources: selected,
        edges,
        providers,
    })
}
#[cfg(test)]
mod tests {
    /// `Envelope::speeds` with the modulo it used to step round the polygon.
    fn speeds_by_modulo(envelope: &super::Envelope, alt: f64) -> Option<(f64, f64)> {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        let mut hit = |v: f64| {
            low = low.min(v);
            high = high.max(v);
        };
        for i in 0..envelope.points.len() {
            let a = envelope.points[i];
            let b = envelope.points[(i + 1) % envelope.points.len()];
            if (a[1] - alt).abs() < 1e-9 {
                hit(a[0]);
            }
            if (a[1] < alt && b[1] > alt) || (a[1] > alt && b[1] < alt) {
                hit(a[0] + (b[0] - a[0]) * (alt - a[1]) / (b[1] - a[1]));
            }
        }
        (low <= high).then_some((low, high))
    }
    #[test]
    fn envelope_speeds_are_what_the_modulo_walk_gave() {
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1_u64 << 53) as f64
        };
        for size in 0..9 {
            for _ in 0..200 {
                let envelope = super::Envelope {
                    g: 1,
                    points: (0..size)
                        .map(|_| [next() * 900., (next() * 60.).floor() * 1000.])
                        .collect(),
                };
                for alt in [0., 500., 1000., 5000., 17_300., 59_999., 60_000., 70_000.]
                    .into_iter()
                    .chain((0..4).map(|_| next() * 60_000.))
                {
                    let (new, old) = (envelope.speeds(alt), speeds_by_modulo(&envelope, alt));
                    assert_eq!(
                        new.map(|(a, b)| (a.to_bits(), b.to_bits())),
                        old.map(|(a, b)| (a.to_bits(), b.to_bits()))
                    );
                }
            }
        }
    }
    #[test]
    fn every_selectable_aircraft_is_offered_to_the_ai() {
        use super::AircraftId::*;
        let kept: Vec<_> = super::AircraftId::SELECTABLE
            .into_iter()
            .filter(|id| !id.ai_flyable())
            .collect();
        assert!(kept.is_empty(), "{kept:?}");
        // The four that slice H3 added, with the two helicopters before them.
        for id in [Ch47, V22, Av8, Yak141, Ah64, Mi24, F18, Faxx, Mig17] {
            assert!(id.ai_flyable(), "{id:?}");
        }
    }
    #[test]
    fn the_powered_lift_aircraft_are_the_six_that_hover() {
        use super::AircraftId::*;
        let hovering: Vec<_> = super::AircraftId::SELECTABLE
            .into_iter()
            .filter(|id| id.is_powered_lift())
            .collect();
        assert_eq!(hovering, [Av8, Yak141, V22, Ah64, Mi24, Ch47]);
    }
    #[test]
    fn concept_selection_is_distinct_from_its_retail_dependency() {
        use super::AircraftId;
        let concept = AircraftId::parse("faxx").unwrap();
        assert_eq!(concept, AircraftId::Faxx);
        assert_eq!(concept.source(), AircraftId::F22n);
        assert_ne!(concept.selection_key(), AircraftId::F22n.selection_key());
        assert_ne!(
            AircraftId::F22n.selection_key(),
            AircraftId::F22.selection_key()
        );
        assert_eq!(concept.pt(), AircraftId::F22n.pt());
        assert_ne!(concept.pt(), AircraftId::F22.pt());
        assert_eq!(concept.cockpit(), AircraftId::F22.cockpit());
        assert!(AircraftId::SELECTABLE.contains(&concept));
        assert!(!AircraftId::ALL.contains(&concept));
        for id in AircraftId::SELECTABLE {
            assert_eq!(AircraftId::parse(id.selection_key()).unwrap(), id);
        }
    }

    /// John, 2026-11-12 (TOR-13): two Tornado IDS versions, each its own
    /// identity on the one TOR.PT record, never aliases of each other.
    #[test]
    fn the_two_tornado_versions_are_distinct_identities_on_one_record() {
        use super::AircraftId::{Tornado, TornadoClean};
        assert_eq!(TornadoClean.source(), Tornado);
        assert_eq!(Tornado.source(), Tornado);
        assert_eq!(TornadoClean.pt(), Tornado.pt());
        assert_ne!(TornadoClean.selection_key(), Tornado.selection_key());
        assert_eq!(TornadoClean.selection_key(), "tornadoc");
        assert_eq!(Tornado.label(), "Tornado IDS (Combat)");
        assert_eq!(TornadoClean.label(), "Tornado IDS (Clean)");
        assert_ne!(TornadoClean.short_label(), Tornado.short_label());
        assert_eq!(AircraftId::parse("tornadoc").unwrap(), TornadoClean);
        assert_eq!(AircraftId::parse("tornado").unwrap(), Tornado);
        assert_eq!(AircraftId::parse("TOR.PT").unwrap(), Tornado);
        assert!(AircraftId::SELECTABLE.contains(&TornadoClean));
        assert!(!AircraftId::ALL.contains(&TornadoClean));
        // Every identity-keyed table the Tornado has, its variant has too.
        for (clean, combat) in [
            (TornadoClean.hud(), Tornado.hud()),
            (TornadoClean.cockpit(), Tornado.cockpit()),
            (TornadoClean.stem(), Tornado.stem()),
            (TornadoClean.cockpit_stem(), Tornado.cockpit_stem()),
        ] {
            assert_eq!(clean, combat);
        }
        assert_eq!(TornadoClean.radar(), Tornado.radar());
        assert_eq!(TornadoClean.guns(), Tornado.guns());
        assert_eq!(TornadoClean.has_pts(), Tornado.has_pts());
        assert_eq!(TornadoClean.reviewed_names(), Tornado.reviewed_names());
    }

    #[test]
    fn variety_capabilities_preserve_missing_and_multiple_source_guns() {
        use super::AircraftId;
        for id in [
            AircraftId::C130,
            AircraftId::E3,
            AircraftId::Il76,
            AircraftId::E2,
            AircraftId::Ch47,
            AircraftId::F4B,
            AircraftId::B747,
            AircraftId::A310,
            AircraftId::Mig25,
        ] {
            assert_eq!(id.gun(), None);
            assert!(id.guns().is_empty());
        }
        for id in [
            AircraftId::C130,
            AircraftId::V22,
            AircraftId::Mi24,
            AircraftId::Ch47,
            AircraftId::Mig17,
            AircraftId::B747,
            AircraftId::A310,
        ] {
            assert_eq!(id.radar(), None);
        }
        assert_eq!(
            AircraftId::Ac130.guns(),
            &["C_25.JT", "C_40.JT", "C_105.JT"]
        );
        assert_eq!(AircraftId::Mig17.guns(), &["GSH30.JT", "GSH23.JT"]);
        assert_eq!(AircraftId::V22.gun(), Some("T30_1.JT"));
        assert_eq!(AircraftId::F4G.gun(), Some("M61.JT"));
        assert_eq!(AircraftId::F4J.gun(), Some("SUU16.JT"));
        // The B is built with no gun but carries the SUU-16 on its centreline.
        assert_eq!(AircraftId::F4B.gun(), None);
        assert!(AircraftId::F4B.mounts_gun("SUU16.JT"));
        assert!(AircraftId::F4J.mounts_gun("SUU16.JT"));
        assert!(AircraftId::F4E.mounts_gun("M61.JT"));
        assert!(!AircraftId::F4E.mounts_gun("SUU16.JT"));
        assert!(!AircraftId::F4B.mounts_gun("M61.JT"));
        assert_eq!(AircraftId::Ah64.radar(), Some("F18R.SEE"));
        assert_eq!(AircraftId::parse("f4.pt").unwrap(), AircraftId::F4G);
        assert_ne!(AircraftId::F4B.pt(), AircraftId::F4J.pt());
        assert!(AircraftId::parse("mig15").is_err());
        assert!(AircraftId::parse("f4u").is_err());
        assert_eq!(AircraftId::C130.cockpit_overlays(), [None, None, None]);
        assert_eq!(AircraftId::F16C.cockpit_overlays(), [None, None, None]);
        assert_eq!(AircraftId::F16A.cockpit_overlays(), [None, None, None]);
        assert_eq!(AircraftId::Yak141.cockpit_overlays()[1], None);
        assert!(AircraftId::Yak141.cockpit_overlays()[0].is_some());
    }

    use super::*;
    #[test]
    fn brf_bounds_and_relocations() {
        let b=Brf::parse(b"[brent's_relocatable_format]\nword $ffff8000\ndword ^60000\nptr name\n:name\nstring \"X.PT\"\nend").unwrap();
        assert_eq!(b.blocks[""][0].number().unwrap(), -32768);
        assert!(b.blocks[""][1].scaled);
        assert!(Brf::parse(b"[brent's_relocatable_format]\nptr missing\nend").is_err());
        assert!(Brf::parse(b"[brent's_relocatable_format]\nword 2").is_err());
    }
    #[test]
    fn envelope_intersects_edges_not_nearest_vertices() {
        let e = Envelope {
            g: 1,
            points: vec![[100., 0.], [200., 10000.], [500., 10000.], [600., 0.]],
        };
        assert_eq!(e.speeds(5000.), Some((150., 550.)));
        assert_eq!(e.speeds(10000.), Some((200., 500.)));
        assert_eq!(e.speeds(11000.), None);
    }
}

#[derive(Debug)]
pub struct Equipment {
    pub name: String,
    pub fields: BTreeMap<String, Token>,
    pub object: BTreeMap<String, Token>,
}
impl Equipment {
    pub fn parse(name: &str, data: &[u8]) -> Result<Self> {
        let b = Brf::parse(data)?;
        let t = b.block("")?;
        let (f, object) = if name.ends_with(".JT") {
            if t.len() != schema::OBJECT.len() + schema::PROJECTILE.len() {
                return Err(invalid("unsupported JT schema"));
            }
            (
                fields(&t[schema::OBJECT.len()..], schema::PROJECTILE)?,
                fields(&t[..schema::OBJECT.len()], schema::OBJECT)?,
            )
        } else if name.ends_with(".SEE") {
            (fields(t, schema::SENSOR)?, BTreeMap::new())
        } else if name.ends_with(".ECM") {
            (fields(t, schema::ECM)?, BTreeMap::new())
        } else {
            return Err(invalid("unsupported equipment schema"));
        };
        let names = b.strings("si_names")?;
        if names.len() != 3 {
            return Err(invalid("invalid store name block"));
        }
        Ok(Self {
            name: names[0].clone(),
            fields: f,
            object,
        })
    }
    pub fn number(&self, key: &str) -> f64 {
        self.fields
            .get(key)
            .or_else(|| self.object.get(key))
            .and_then(|t| t.number().ok())
            .unwrap_or(0) as f64
    }
}
#[cfg(test)]
mod profile_tests {
    use super::*;
    fn fixture() -> String {
        let row = |g| (g, vec![[100, 0], [200, 10_000], [300, 0]]);
        fixture_with_rows(0, 1, &[row(0), row(1)])
    }
    /// The fixture with its own G range and rows (speed, altitude points).
    fn fixture_with_rows(min: i32, max: i32, rows: &[(i32, Vec<[i32; 2]>)]) -> String {
        fixture_with_fields(min, max, rows, &[])
    }
    /// [`fixture_with_rows`] with named record fields set (by field name).
    fn fixture_with_fields(
        min: i32,
        max: i32,
        rows: &[(i32, Vec<[i32; 2]>)],
        fields: &[(&str, i32)],
    ) -> String {
        let mut text = String::from("[brent's_relocatable_format]\n");
        for (block, layout) in [
            ("o", schema::OBJECT),
            ("n", schema::NPC),
            ("p", schema::PLANE),
        ] {
            for (kind, name) in layout {
                let v = match (block, *name) {
                    ("o", "structType") => 5,
                    ("o", "typeSize") => 660,
                    ("o", "weight") => 1000,
                    ("n", "numHards") => 1,
                    ("p", "envMin") => min,
                    ("p", "envMax") => max,
                    ("p", "internalFuel") => 100,
                    // The retail Mirage 2000 / Rafale-template engine numbers.
                    ("p", "engines") => 2,
                    ("p", "thrust") => 17687,
                    ("p", "aftThrust") => 32000,
                    ("p", "maxTakeoffWeight") => 2000,
                    _ => 0,
                };
                let v = fields
                    .iter()
                    .find(|(field, _)| field == name)
                    .map_or(v, |(_, value)| *value);
                match *kind {
                    "ptr" => {
                        if ["ot_names", "shape", "hards", "env"].contains(name) {
                            text += &format!("ptr {name}\n");
                        } else {
                            text += "dword 0\n";
                        }
                    }
                    "symbol" => text += "symbol _PLANEProc\n",
                    _ => text += &format!("{kind} {v}\n"),
                };
            }
        }
        text += ":hards\n";
        for (kind, _) in schema::HARDPOINT {
            if *kind == "ptr" {
                text += "dword 0\n";
            } else {
                text += &format!("{kind} 0\n");
            }
        }
        text += ":env\n";
        for (g, points) in rows {
            for (i, (kind, _)) in schema::ENVELOPE.iter().enumerate() {
                let v = match i {
                    0 => *g,
                    1 => points.len() as i32,
                    4.. if (i - 4) / 2 < points.len() => points[(i - 4) / 2][(i - 4) % 2],
                    _ => 0,
                };
                text += &format!("{kind} {v}\n");
            }
        }
        text += ":ot_names\nstring \"Synthetic\"\nstring \"Synthetic plane\"\nstring \"F18.PT\"\n:shape\nstring \"TEST.SH\"\nend\n";
        text
    }
    #[test]
    fn rafale_identity_keeps_its_own_profile_and_rejects_other_variants() {
        let text = fixture().replace("F18.PT", "RAFALE.PT");
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::Rafale);
        assert_eq!(a.number("weight"), 1000.);
        for unsupported in ["RAFALEF.PT", "RAFALEE.PT", "F18C.PT", "RAFALE"] {
            assert!(Aircraft::parse(text.replace("RAFALE.PT", unsupported).as_bytes()).is_err());
        }
    }
    #[test]
    fn additional_identities_require_their_own_size_and_variant() {
        for (id, short, long, size) in [
            (AircraftId::F14, "F-14", "F- 14D Tomcat", 636),
            (AircraftId::A4E, "A-4E", "A- 4E Skyhawk", 612),
            (AircraftId::X31, "X-31", "X-31 EFM", 660),
            (AircraftId::Mig29, "MiG-29", "MiG-29 Fulcrum-C", 636),
            (AircraftId::Su27, "Su-27", "Su-27 Flanker-B", 660),
            (AircraftId::Mig21, "MiG-21", "MiG-21 Fishbed", 612),
            (AircraftId::Su25, "Su-25", "Su-25 Frogfoot-A", 660),
            (AircraftId::Mig23, "MiG-23", "MiG-23 Flogger-B", 660),
            (AircraftId::Su35, "Su-35", "Su-35", 660),
            (AircraftId::F22, "F-22", "F- 22A Raptor", 636),
            (AircraftId::F22n, "F-22", "F- 22N Raptor", 636),
            (AircraftId::C130, "C-130", "C-130 Hercules", 468),
            (AircraftId::Ac130, "AC-130U", "AC-130U Spectre", 612),
            (AircraftId::E3, "E-3", "E-3 AWACS Sentry (AIR)", 516),
            (AircraftId::Il76, "IL-76", "IL-76 Mainstay (AIR)", 516),
            (AircraftId::E2, "E-2C", "E-2C Hawkeye (AIR)", 516),
            (AircraftId::Av8, "Av-8", "Av-8B Harrier II", 660),
            (AircraftId::Yak141, "Yak-141", "Yak-141 Freestyle-A", 588),
            (AircraftId::V22, "V-22", "V-22 Osprey", 540),
            (AircraftId::Ah64, "AH-64", "AH-64 Apache", 636),
            (AircraftId::Mi24, "Mi-24", "Mi-24 Hind-D", 516),
            (AircraftId::Ch47, "CH-47", "CH-47 Chinook", 468),
            (AircraftId::Mig17, "MiG-17", "MiG-17F Fresco", 540),
            (AircraftId::F4B, "F-4B", "F-  4B Phantom II", 612),
            (AircraftId::F4J, "F-4J", "F-  4J Phantom II", 636),
            (AircraftId::F4E, "F-4", "F-  4E (Desert) Phantom", 636),
            (AircraftId::F4G, "F-4G", "F-  4G Wild Weasel Phantom", 636),
            (AircraftId::A7, "A-7", "A- 7E Corsair II", 612),
            (AircraftId::F15, "F-15", "F- 15C Eagle", 588),
            (AircraftId::F16C, "F-16C", "F- 16C Falcon", 660),
            (AircraftId::F104, "F-104", "F-104N Starfighter", 660),
            (AircraftId::A10, "A-10", "A-10 Thunderbolt", 660),
            (AircraftId::B747, "B747", "Boeing 747", 468),
            (AircraftId::A310, "A310", "Airbus 310", 468),
            (AircraftId::Mig25, "MiG-25", "MiG-25 Foxbat", 588),
            (AircraftId::F16A, "F-16", "F- 16A Falcon", 660),
            (AircraftId::Mig31, "MiG-31", "MiG-31 Foxhound", 588),
            (AircraftId::M2000, "M2000", "Mirage 2000", 660),
            (AircraftId::F15E, "F-15E", "F- 15E Strike Eagle", 660),
            (AircraftId::Gripen, "Gripen", "Saab Gripen", 660),
            (AircraftId::F5E, "F-5", "F-  5E Tiger", 612),
            (AircraftId::Tornado, "Tornado", "Tornado IDS", 660),
            (AircraftId::Mf1, "MF1", "Mirage F1", 636),
            (AircraftId::B52, "B-52", "B-52 Stratofortress", 612),
            (AircraftId::B2, "B-2", "B- 2 Spirit", 660),
            (AircraftId::F117, "F-117", "F-117A Nighthawk", 588),
            (AircraftId::B1b, "B-1B", "B- 1B Lancer", 636),
            (AircraftId::Tu160, "Tu-160", "Tu-160 Blackjack-A", 588),
            (AircraftId::F111, "F-111F", "F-111F Aardvark", 660),
            (AircraftId::Tu26, "Tu-26", "Tu- 26 Backfire-C", 660),
            (AircraftId::Tu95, "Tu-95", "Tu- 95 Bear-H", 660),
            (AircraftId::Su34, "Su-34", "Su-34 Platypus", 660),
        ] {
            let text = fixture()
                .replace("F18.PT", id.pt())
                .replace("\"Synthetic\"", &format!("\"{short}\""))
                .replace("Synthetic plane", long)
                .replacen("word 660", &format!("word {size}"), 1);
            assert_eq!(Aircraft::parse(text.as_bytes()).unwrap().id, id);
            assert!(Aircraft::parse(text.replace(long, "Unreviewed variant").as_bytes()).is_err());
            assert!(
                Aircraft::parse(
                    text.replacen(&format!("word {size}"), "word 659", 1)
                        .as_bytes()
                )
                .is_err()
            );
        }
        assert!(AircraftId::parse("F31E.PT").is_err());
        assert!(AircraftId::parse("F14B.PT").is_err());
    }
    #[test]
    fn the_batch_five_aircraft_parse_by_key_and_keep_their_neighbours_apart() {
        // B5-1 (2026-11-10): checkpoint codes 41 to 45, in batch order.
        for (key, id) in [
            ("f15e", AircraftId::F15E),
            ("F15E.PT", AircraftId::F15E),
            ("gripen", AircraftId::Gripen),
            ("GRIPEN.PT", AircraftId::Gripen),
            ("f5e", AircraftId::F5E),
            ("F5E.PT", AircraftId::F5E),
            ("tornado", AircraftId::Tornado),
            ("tor", AircraftId::Tornado),
            ("TOR.PT", AircraftId::Tornado),
            ("mf1", AircraftId::Mf1),
            ("MF1.PT", AircraftId::Mf1),
        ] {
            assert_eq!(AircraftId::parse(key).unwrap(), id, "{key}");
        }
        // Separate records, never aliases.
        assert_eq!(AircraftId::parse("f15").unwrap(), AircraftId::F15);
        for key in ["F5EE.PT", "F5EV.PT", "SU24.PT", "MR3.PT", "RAFALE.PT"] {
            assert!(
                AircraftId::parse(key).is_err()
                    || AircraftId::parse(key).unwrap() == AircraftId::Rafale,
                "{key}"
            );
        }
        // Update (BMB-A1, 2026-11-15): the bomber batch appends three more.
        // Update (BMB-B1, 2026-11-15): batch B appends three more.
        // Update (BMB-C1, 2026-11-15): batch C appends three more.
        assert_eq!(AircraftId::ALL.len(), 54);
        assert_eq!(AircraftId::SELECTABLE.len(), 56);
        assert_eq!(
            AircraftId::ALL[40..45],
            [
                AircraftId::F15E,
                AircraftId::Gripen,
                AircraftId::F5E,
                AircraftId::Tornado,
                AircraftId::Mf1,
            ]
        );
    }
    #[test]
    fn the_bomber_batch_a_aircraft_parse_by_key_and_follow_the_batch_five_ones() {
        // BMB-A1 (2026-11-15): checkpoint codes 47 to 49, in batch order.
        for (key, id) in [
            ("b52", AircraftId::B52),
            ("B52.PT", AircraftId::B52),
            ("b2", AircraftId::B2),
            ("B2.PT", AircraftId::B2),
            ("f117", AircraftId::F117),
            ("F117.PT", AircraftId::F117),
        ] {
            assert_eq!(AircraftId::parse(key).unwrap(), id, "{key}");
            assert_eq!(id.source(), id);
            assert!(
                id.pt()
                    .eq_ignore_ascii_case(&format!("{}.PT", key.trim_end_matches(".PT")))
            );
        }
        // Separate records: no neighbour key reaches them.
        // (BMB-B1: B1.PT and B1B are batch B's own keys now.)
        for key in ["B52G.PT", "F117A", "F22.PT"] {
            assert!(
                !matches!(
                    AircraftId::parse(key),
                    Ok(AircraftId::B52 | AircraftId::B2 | AircraftId::F117)
                ),
                "{key}"
            );
        }
        assert_eq!(
            AircraftId::ALL[45..48],
            [AircraftId::B52, AircraftId::B2, AircraftId::F117]
        );
        assert_eq!(
            AircraftId::SELECTABLE[47..50],
            [AircraftId::B52, AircraftId::B2, AircraftId::F117]
        );
        // The bindings John accepted (decisions 14, 15, 17, 28).
        assert_eq!(AircraftId::B52.hud(), "BMBROH.HUD");
        assert_eq!(AircraftId::B52.cockpit_stem(), "AC130");
        assert_eq!(AircraftId::B2.hud(), "B2.HUD");
        assert_eq!(AircraftId::B2.cockpit_stem(), "B2");
        assert_eq!(AircraftId::B2.cockpit(), "~B2H.PIC");
        assert_eq!(AircraftId::B2.label(), "B- 2 Spirit");
        assert_eq!(AircraftId::F117.hud(), "F117.HUD");
        assert_eq!(AircraftId::F117.cockpit(), "~F117H.PIC");
        assert_eq!(AircraftId::B52.gun(), Some("M61.JT"));
        assert_eq!(AircraftId::B2.gun(), None);
        assert_eq!(AircraftId::F117.radar(), None);
        // No overlay pictures exist for the three cockpit families.
        for id in [AircraftId::B52, AircraftId::B2, AircraftId::F117] {
            assert_eq!(id.cockpit_overlays(), [None, None, None], "{id:?}");
        }
        assert!(!AircraftId::B52.has_pts());
        assert!(AircraftId::B2.has_pts() && AircraftId::F117.has_pts());
    }
    /// F117-7 and F111-7: only the F-117 (IRADS turret and FLIR, decision 26)
    /// and the F-111F (Pave Tack and its FLIR) have their own sensor names;
    /// every other aircraft, the B-2 included, keeps the generic labels.
    #[test]
    fn only_the_f117_and_the_f111_have_their_own_sensor_names() {
        for id in AircraftId::ALL {
            let names = id.sensor_names();
            match id {
                AircraftId::F117 => {
                    let names = names.unwrap();
                    assert_eq!((names.sight, names.infrared), ("IRADS", "FLIR"));
                }
                AircraftId::F111 => {
                    let names = names.unwrap();
                    assert_eq!(
                        (names.sight, names.infrared, names.pod),
                        ("PAVE TACK", "FLIR", "Pave Tack")
                    );
                }
                _ => assert_eq!(names, None, "{id:?}"),
            }
        }
    }
    #[test]
    fn the_bomber_batch_b_aircraft_parse_by_key_and_follow_their_records() {
        // BMB-B1 (2026-11-15): checkpoint codes 50 to 52, in batch order.
        for (key, id, pt) in [
            ("b1b", AircraftId::B1b, "B1.PT"),
            ("B1.PT", AircraftId::B1b, "B1.PT"),
            ("tu160", AircraftId::Tu160, "TU160.PT"),
            ("TU160.PT", AircraftId::Tu160, "TU160.PT"),
            ("f111", AircraftId::F111, "F111.PT"),
            ("F111.PT", AircraftId::F111, "F111.PT"),
        ] {
            assert_eq!(AircraftId::parse(key).unwrap(), id, "{key}");
            assert_eq!(id.source(), id);
            assert_eq!(id.pt(), pt);
        }
        // Separate records: no neighbour key reaches them.
        for key in ["B1", "B1A", "B1.PTS", "Tu-160", "F111A", "F22.PT"] {
            assert!(
                !matches!(
                    AircraftId::parse(key),
                    Ok(AircraftId::B1b | AircraftId::Tu160 | AircraftId::F111)
                ),
                "{key}"
            );
        }
        assert_eq!(
            AircraftId::ALL[48..51],
            [AircraftId::B1b, AircraftId::Tu160, AircraftId::F111]
        );
        assert_eq!(
            AircraftId::SELECTABLE[50..53],
            [AircraftId::B1b, AircraftId::Tu160, AircraftId::F111]
        );
        // The bindings the coordinator's first pass fixed (BOMBERS-DECISIONS-B).
        assert_eq!(AircraftId::B1b.hud(), "BMBRCC.HUD");
        assert_eq!(AircraftId::B1b.stem(), "B1");
        assert_eq!(AircraftId::B1b.cockpit_stem(), "B2");
        assert_eq!(AircraftId::B1b.cockpit(), "~B2H.PIC");
        assert_eq!(AircraftId::B1b.label(), "B- 1B Lancer");
        assert_eq!(AircraftId::B1b.radar(), Some("F18R.SEE"));
        assert_eq!(AircraftId::Tu160.hud(), "B2.HUD");
        assert_eq!(AircraftId::Tu160.stem(), "TU160");
        assert_eq!(AircraftId::Tu160.cockpit_stem(), "B2");
        assert_eq!(AircraftId::Tu160.cockpit(), "~B2H.PIC");
        assert_eq!(AircraftId::Tu160.radar(), Some("TU160R.SEE"));
        assert_eq!(AircraftId::F111.hud(), "AC130.HUD");
        assert_eq!(AircraftId::F111.stem(), "F111");
        assert_eq!(AircraftId::F111.cockpit_stem(), "AC130");
        assert_eq!(AircraftId::F111.cockpit(), "~AC130H.PIC");
        assert_eq!(AircraftId::F111.radar(), Some("A6R.SEE"));
        for id in [AircraftId::B1b, AircraftId::Tu160, AircraftId::F111] {
            // No gun row, no overlay pictures, no PTS companion.
            assert_eq!(id.gun(), None, "{id:?}");
            assert_eq!(id.cockpit_overlays(), [None, None, None], "{id:?}");
            assert!(!id.has_pts(), "{id:?}");
        }
    }
    #[test]
    fn the_bomber_batch_c_aircraft_parse_by_key_and_follow_their_records() {
        // BMB-C1 (2026-11-15): checkpoint codes 53 to 55, in batch order.
        for (key, id, pt) in [
            ("tu26", AircraftId::Tu26, "TU26.PT"),
            ("TU26.PT", AircraftId::Tu26, "TU26.PT"),
            ("tu95", AircraftId::Tu95, "TU95.PT"),
            ("TU95.PT", AircraftId::Tu95, "TU95.PT"),
            ("su34", AircraftId::Su34, "SU34.PT"),
            ("SU34.PT", AircraftId::Su34, "SU34.PT"),
        ] {
            assert_eq!(AircraftId::parse(key).unwrap(), id, "{key}");
            assert_eq!(id.source(), id);
            assert_eq!(id.pt(), pt);
        }
        // Separate records: no neighbour key reaches them.
        for key in [
            "TU26A.PT", "Tu-26", "TU22", "TU95A", "TU95.PTS", "Tu-95", "SU34M", "SU33.PT",
            "SU35B.PT",
        ] {
            assert!(
                !matches!(
                    AircraftId::parse(key),
                    Ok(AircraftId::Tu26 | AircraftId::Tu95 | AircraftId::Su34)
                ),
                "{key}"
            );
        }
        assert_eq!(
            AircraftId::ALL[51..],
            [AircraftId::Tu26, AircraftId::Tu95, AircraftId::Su34]
        );
        assert_eq!(
            AircraftId::SELECTABLE[53..],
            [AircraftId::Tu26, AircraftId::Tu95, AircraftId::Su34]
        );
        // The bindings the coordinator's first pass fixed (BOMBERS-DECISIONS-C).
        for id in [AircraftId::Tu26, AircraftId::Tu95] {
            assert_eq!(id.hud(), "BMBROH.HUD", "{id:?}");
            assert_eq!(id.cockpit_stem(), "AC130", "{id:?}");
            assert_eq!(id.cockpit(), "~AC130H.PIC", "{id:?}");
            assert_eq!(id.gun(), Some("T23_1.JT"), "{id:?}");
            assert_eq!(id.cockpit_overlays(), [None, None, None], "{id:?}");
        }
        assert_eq!(AircraftId::Tu26.stem(), "TU26");
        assert_eq!(AircraftId::Tu26.label(), "Tu- 26 Backfire-C");
        assert_eq!(AircraftId::Tu26.radar(), Some("TU26R.SEE"));
        assert_eq!(AircraftId::Tu95.stem(), "TU95");
        assert_eq!(AircraftId::Tu95.label(), "Tu- 95 Bear-H");
        assert_eq!(AircraftId::Tu95.radar(), Some("TU95R.SEE"));
        assert_eq!(AircraftId::Su34.hud(), "SU33CC.HUD");
        assert_eq!(AircraftId::Su34.stem(), "SU34");
        assert_eq!(AircraftId::Su34.cockpit_stem(), "SU33");
        assert_eq!(AircraftId::Su34.cockpit(), "~SU33H.PIC");
        assert_eq!(AircraftId::Su34.instrument_panel(), "~SU33_P.PIC");
        assert_eq!(AircraftId::Su34.label(), "Su-34 Platypus");
        assert_eq!(AircraftId::Su34.radar(), Some("SU27R.SEE"));
        assert_eq!(AircraftId::Su34.gun(), Some("GSH301.JT"));
        // The Su-33 cockpit family has left and right overlays and no centre.
        assert_eq!(
            AircraftId::Su34.cockpit_overlays(),
            [
                Some("~SU33_LH.PIC".to_string()),
                None,
                Some("~SU33_RH.PIC".to_string())
            ]
        );
        for id in [AircraftId::Tu26, AircraftId::Tu95, AircraftId::Su34] {
            assert!(!id.has_pts(), "{id:?}");
        }
    }
    #[test]
    fn the_mirage_2000_reads_one_engine_and_one_m53_p2s_thrust_everyone_else_retail() {
        // Opinionated (John, 2026-11-09): the override applies where the
        // record is read, so every consumer sees 1 engine, 14,455 / 21,380
        // lbf. The same retail numbers on the Rafale record stay retail.
        let text = fixture()
            .replace("F18.PT", "M2000.PT")
            .replace("\"Synthetic\"", "\"M2000\"")
            .replace("Synthetic plane", "Mirage 2000");
        let m = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(m.id, AircraftId::M2000);
        assert_eq!(m.number("engines"), 1.);
        assert_eq!(m.number("thrust"), 14_455.);
        assert_eq!(m.number("aftThrust"), 21_380.);
        let r = Aircraft::parse(fixture().replace("F18.PT", "RAFALE.PT").as_bytes()).unwrap();
        assert_eq!(r.number("engines"), 2.);
        assert_eq!(r.number("thrust"), 17_687.);
        assert_eq!(r.number("aftThrust"), 32_000.);
    }
    /// A Mirage-shaped record: G rows -4 to +8 with the retail +8 polygon
    /// (or `plus_eight`), a 1 G row with a fast edge well past the +8 row's,
    /// and plain triangles elsewhere.
    fn mirage_rows(plus_eight: &[[i32; 2]]) -> Vec<(i32, Vec<[i32; 2]>)> {
        (-4..=8)
            .map(|g| match g {
                1 => (
                    g,
                    vec![
                        [200, 0],
                        [870, 59_000],
                        [2_100, 59_000],
                        [1_500, 18_000],
                        [1_170, 0],
                    ],
                ),
                8 => (g, plus_eight.to_vec()),
                _ => (g, vec![[100, 0], [200, 10_000], [300, 0]]),
            })
            .collect()
    }
    const RETAIL_PLUS_8: [[i32; 2]; 7] = [
        [570, 0],
        [675, 11_000],
        [920, 20_000],
        [1_235, 20_000],
        [1_330, 18_000],
        [1_205, 9_000],
        [1_100, 0],
    ];
    fn mirage_text(plus_eight: &[[i32; 2]]) -> String {
        fixture_with_rows(-4, 8, &mirage_rows(plus_eight))
    }
    fn mirage_2000(text: &str) -> String {
        text.replace("F18.PT", "M2000.PT")
            .replace("\"Synthetic\"", "\"M2000\"")
            .replace("Synthetic plane", "Mirage 2000")
    }
    #[test]
    fn the_mirage_2000_reads_g_rows_minus_4_to_plus_9_with_a_fitted_plus_9_row() {
        // Opinionated (John, 2026-11-10) +9 G; the row is fitted from the
        // Mirage's own +8 row: slow side x sqrt(9/8), fast side and altitudes
        // as the +8 row's.
        let text = mirage_text(&RETAIL_PLUS_8);
        let m = Aircraft::parse(mirage_2000(&text).as_bytes()).unwrap();
        assert_eq!(m.id, AircraftId::M2000);
        assert_eq!(m.number("envMin"), -4.);
        assert_eq!(m.number("envMax"), 9.);
        let gs: Vec<i32> = m.envelopes.iter().map(|e| e.g).collect();
        assert_eq!(gs, (-4..=9).collect::<Vec<_>>());
        let plus_nine = m.envelopes.last().unwrap();
        assert_eq!(
            plus_nine.points,
            vec![
                [605., 0.],
                [716., 11_000.],
                [976., 20_000.],
                [1_235., 20_000.],
                [1_330., 18_000.],
                [1_205., 9_000.],
                [1_100., 0.],
            ]
        );
        // Every other row is the record's own.
        let plain = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(plain.id, AircraftId::F18);
        assert_eq!(plain.number("envMax"), 8.);
        assert_eq!(m.envelopes[..13], plain.envelopes[..]);
    }
    #[test]
    fn only_the_mirage_2000_gets_the_plus_9_row_and_an_edited_record_is_read_as_it_is() {
        let text = mirage_text(&RETAIL_PLUS_8);
        // The same retail rows on other identities stay at +8.
        for other in [
            text.replace("F18.PT", "RAFALE.PT"),
            text.replace("F18.PT", "F16.PT")
                .replace("\"Synthetic\"", "\"F-16\"")
                .replace("Synthetic plane", "F- 16A Falcon"),
        ] {
            let a = Aircraft::parse(other.as_bytes()).unwrap();
            assert_eq!(a.number("envMax"), 8., "{:?}", a.id);
            assert_eq!(a.envelopes.len(), 13);
            assert_eq!(a.envelopes.last().unwrap().g, 8);
        }
        // One +8 point off the retail polygon: left alone.
        let mut edited = RETAIL_PLUS_8;
        edited[1][0] = 676;
        let m = Aircraft::parse(mirage_2000(&mirage_text(&edited)).as_bytes()).unwrap();
        assert_eq!(m.number("envMax"), 8.);
        assert_eq!(m.envelopes.len(), 13);
        assert_eq!(m.envelopes.last().unwrap().points[1][0], 676.);
        // A record that already reaches +9 keeps its own rows.
        let mut rows = mirage_rows(&RETAIL_PLUS_8);
        rows.push((9, vec![[700, 0], [800, 9_000], [900, 0]]));
        let nine = fixture_with_rows(-4, 9, &rows);
        let m = Aircraft::parse(mirage_2000(&nine).as_bytes()).unwrap();
        assert_eq!(m.envelopes.len(), 14);
        assert_eq!(m.envelopes.last().unwrap().points[0], [700., 0.]);
    }
    /// A batch-aircraft record: the fixture dressed as `file` with retail
    /// numbers for the fields B5-2 corrects.
    fn batch_text(
        file: &str,
        short: &str,
        long: &str,
        size: i32,
        range: (i32, i32),
        rows: &[(i32, Vec<[i32; 2]>)],
        fields: &[(&str, i32)],
    ) -> String {
        let mut all = vec![("typeSize", size)];
        all.extend_from_slice(fields);
        fixture_with_fields(range.0, range.1, rows, &all)
            .replace("F18.PT", file)
            .replace("\"Synthetic\"", &format!("\"{short}\""))
            .replace("Synthetic plane", long)
    }
    fn plain_rows(min: i32, max: i32) -> Vec<(i32, Vec<[i32; 2]>)> {
        (min..=max)
            .map(|g| (g, vec![[100, 0], [200, 10_000], [900, 10_000], [300, 0]]))
            .collect()
    }
    #[test]
    fn the_tornado_reads_real_thrust_weights_and_plus_7_and_plus_8_rows_everyone_else_retail() {
        // Opinionated (John, 2026-11-10): two RB199 Mk 103, real weights; B5-4
        // (John, 2026-11-11): +8 G (rows +7 and +8 fitted) and 50,000 ft.
        let mut rows = plain_rows(-3, 6);
        rows[9].1 = vec![
            [575, 0],
            [667, 7_000],
            [880, 14_000],
            [1_120, 14_000],
            [1_140, 0],
        ];
        let retail = [
            ("weight", 41_885),
            ("maxAlt", 60_000),
            ("thrust", 35_270),
            ("aftThrust", 50_700),
            ("maxTakeoffWeight", 90_390),
            ("internalFuel", 11_221),
            ("fuelConsumption", 6),
            ("aftFuelConsumption", 38),
        ];
        let text = batch_text(
            "TOR.PT",
            "Tornado",
            "Tornado IDS",
            660,
            (-3, 6),
            &rows,
            &retail,
        );
        let t = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(t.id, AircraftId::Tornado);
        assert_eq!(t.number("engines"), 2.);
        assert_eq!(t.number("thrust"), 17_300.);
        assert_eq!(t.number("aftThrust"), 32_150.);
        assert_eq!(t.number("weight"), 31_065.);
        assert_eq!(t.number("maxTakeoffWeight"), 61_620.);
        assert_eq!((t.number("envMin"), t.number("envMax")), (-3., 8.));
        assert_eq!(t.number("maxAlt"), 50_000.);
        // B5-3: the RB199 flows, 3 dry and 22 reheat lb/s (the Su-24's 6 / 38).
        assert_eq!(
            (t.number("fuelConsumption"), t.number("aftFuelConsumption")),
            (3., 22.)
        );
        let gs: Vec<i32> = t.envelopes.iter().map(|e| e.g).collect();
        assert_eq!(gs, (-3..=8).collect::<Vec<_>>());
        // The same numbers on the MiG-23 record stay retail.
        let m = Aircraft::parse(
            batch_text(
                "MIG23.PT",
                "MiG-23",
                "MiG-23 Flogger-B",
                660,
                (-3, 6),
                &rows,
                &retail,
            )
            .as_bytes(),
        )
        .unwrap();
        assert_eq!((m.number("thrust"), m.number("envMax")), (35_270., 6.));
        assert_eq!(m.number("aftFuelConsumption"), 38.);
        assert_eq!(m.number("maxAlt"), 60_000.);
        // Every Tornado row below +7 is the record's own.
        assert_eq!(t.envelopes[..10], m.envelopes[..]);
        // An edited Tornado (one thrust off) is read as it is, rows and all.
        let mut edited = retail;
        edited[2].1 = 35_271;
        let e = Aircraft::parse(
            batch_text(
                "TOR.PT",
                "Tornado",
                "Tornado IDS",
                660,
                (-3, 6),
                &rows,
                &edited,
            )
            .as_bytes(),
        )
        .unwrap();
        assert_eq!((e.number("thrust"), e.number("weight")), (35_271., 41_885.));
    }
    #[test]
    fn the_b52_b2_and_f117_read_their_real_figures_only_while_retail() {
        // BMB-A2 (John, 2026-11-15, decisions 5, 16, 23 and 26).
        // B-52H: thrust 136,000, empty 185,000, fuel 312,197 (over the 488,000
        // maximum with the empty weight, as on the real jet), flow 20 lb/s.
        let b52 = [
            ("engines", 8),
            ("weight", 235_220),
            ("thrust", 120_000),
            ("maxTakeoffWeight", 488_000),
            ("internalFuel", 199_000),
            ("fuelConsumption", 18),
        ];
        let text = batch_text(
            "B52.PT",
            "B-52",
            "B-52 Stratofortress",
            612,
            (-2, 3),
            &plain_rows(-2, 3),
            &b52,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::B52);
        assert_eq!(a.number("thrust"), 136_000.);
        assert_eq!(a.number("weight"), 185_000.);
        assert_eq!(a.number("internalFuel"), 312_197.);
        assert_eq!(a.number("fuelConsumption"), 20.);
        assert_eq!(a.number("maxTakeoffWeight"), 488_000.);
        assert!(a.number("weight") + a.number("internalFuel") > a.number("maxTakeoffWeight"));
        // One changed number keeps the whole record retail.
        let edited = b52.map(|(k, v)| if k == "thrust" { (k, 121_000) } else { (k, v) });
        let text = batch_text(
            "B52.PT",
            "B-52",
            "B-52 Stratofortress",
            612,
            (-2, 3),
            &plain_rows(-2, 3),
            &edited,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(
            (a.number("thrust"), a.number("weight")),
            (121_000., 235_220.)
        );
        // B-2A: thrust 69,200, empty 158,000, fuel 167,000, maximum 376,000;
        // the fuel flow 6 lb/s becomes 7 (B2-6).
        // B2-8 adds the real 50,000 ft ceiling (both top edges up 1,000 ft)
        // and the fitted airbrake drag 64 (the B-1B's 256 in the record).
        let b2 = [
            ("engines", 4),
            ("weight", 100_000),
            ("thrust", 114_000),
            ("aftThrust", 0),
            ("maxTakeoffWeight", 400_000),
            ("internalFuel", 180_000),
            ("fuelConsumption", 6),
            ("maxAlt", 49_000),
            ("airBrakesDrag", 256),
            ("sigs[2]", 15),
            ("sigs[3]", 1),
        ];
        let mut b2_rows = plain_rows(-1, 3);
        b2_rows[1].1 = vec![
            [170, 0],
            [200, 15_500],
            [290, 34_250],
            [420, 46_625],
            [550, 52_000],
            [720, 51_500],
            [855, 45_750],
            [895, 36_000],
            [790, 0],
        ];
        b2_rows[2].1 = vec![
            [220, 0],
            [250, 15_000],
            [350, 33_500],
            [460, 44_750],
            [560, 49_000],
            [700, 48_000],
            [800, 43_000],
            [840, 36_000],
            [750, 0],
        ];
        let text = batch_text("B2.PT", "B-2", "B- 2 Spirit", 660, (-1, 3), &b2_rows, &b2);
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::B2);
        assert_eq!(
            (a.number("thrust"), a.number("weight")),
            (69_200., 158_000.)
        );
        assert_eq!(
            (a.number("internalFuel"), a.number("maxTakeoffWeight")),
            (167_000., 376_000.)
        );
        assert_eq!(a.number("fuelConsumption"), 7.);
        assert_eq!(
            (a.number("maxAlt"), a.number("airBrakesDrag")),
            (50_000., 64.)
        );
        // The stealth signature (infrared 15, radar 1) is not touched.
        assert_eq!((a.number("sigs[2]"), a.number("sigs[3]")), (15., 1.));
        let env = |g: i32| a.envelopes.iter().find(|e| e.g == g).unwrap();
        assert_eq!(env(1).points[4], [560., 50_000.]);
        assert_eq!(env(0).points[4], [550., 53_000.]);
        // Nothing else in the polygons moved: the 36,000 ft top speed (840
        // ft/s, Mach 0.87) and the sea level edge (750 ft/s) are the record's.
        assert_eq!(env(1).points[7], [840., 36_000.]);
        assert_eq!(env(1).points[8], [750., 0.]);
        assert_eq!(env(1).points[5], [700., 48_000.]);
        // An edited B-2 (the ceiling already changed) keeps every retail value.
        let mut edited = b2;
        edited[7].1 = 48_000;
        let e = Aircraft::parse(
            batch_text(
                "B2.PT",
                "B-2",
                "B- 2 Spirit",
                660,
                (-1, 3),
                &b2_rows,
                &edited,
            )
            .as_bytes(),
        )
        .unwrap();
        assert_eq!(
            (e.number("maxAlt"), e.number("airBrakesDrag")),
            (48_000., 256.)
        );
        // F-117A: maximum 52,500 lb, ceiling 45,000 ft with both top edges
        // moved, the 1 G fast edge at 36,000 ft trimmed to Mach 0.92 (891
        // ft/s), sea level kept, the other rows held to the 1 G edge.
        let mut rows = plain_rows(-3, 6);
        let row = |p: &[[i32; 2]]| p.to_vec();
        rows[3].1 = row(&[
            [250, 0],
            [265, 16_000],
            [327, 29_500],
            [418, 38_750],
            [570, 44_000],
            [715, 44_000],
            [897, 42_000],
            [1_090, 36_000],
            [1_060, 0],
        ]);
        rows[4].1 = row(&[
            [285, 0],
            [305, 17_000],
            [367, 30_500],
            [458, 37_750],
            [570, 42_000],
            [715, 42_000],
            [847, 40_000],
            [1_020, 36_000],
            [1_000, 0],
        ]);
        rows[2].1 = row(&[
            [315, 0],
            [335, 17_000],
            [392, 25_500],
            [481, 30_750],
            [570, 34_000],
            [710, 35_000],
            [960, 34_000],
            [980, 0],
        ]);
        let f117 = [
            ("engines", 2),
            ("weight", 29_500),
            ("maxAlt", 42_000),
            ("thrust", 21_600),
            ("maxTakeoffWeight", 54_000),
            ("internalFuel", 18_500),
            ("fuelConsumption", 2),
            ("airBrakesDrag", 256),
        ];
        let text = batch_text(
            "F117.PT",
            "F-117",
            "F-117A Nighthawk",
            588,
            (-3, 6),
            &rows,
            &f117,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::F117);
        assert_eq!(
            (a.number("maxTakeoffWeight"), a.number("maxAlt")),
            (52_500., 45_000.)
        );
        assert_eq!(a.number("thrust"), 21_600.);
        // F117-8: the real jet has no speed brake, so the record's 256 reads 0.
        assert_eq!(a.number("airBrakesDrag"), 0.);
        // F117-9: the fuel flow fitted to the published range, 2 to 3 lb/s.
        assert_eq!(a.number("fuelConsumption"), 3.);
        let env = |g: i32| a.envelopes.iter().find(|e| e.g == g).unwrap();
        assert_eq!(
            env(1).points,
            [
                [285., 0.],
                [305., 17_000.],
                [367., 30_500.],
                [458., 37_750.],
                [570., 45_000.],
                [715., 45_000.],
                [847., 40_000.],
                [891., 36_000.],
                [1_000., 0.]
            ]
        );
        assert_eq!(
            env(0).points[4..],
            [
                [570., 47_000.],
                [715., 47_000.],
                [897., 42_000.],
                [961., 36_000.],
                [1_060., 0.]
            ]
        );
        // The -1 G row's fast side is held to the 1 G edge at 34,000 ft
        // (891 + 109 x 2,000 / 36,000 = 897).
        assert_eq!(env(-1).points[6], [897., 34_000.]);
        assert_eq!(env(-1).points[7], [980., 0.]);
    }
    #[test]
    fn the_b1b_tu160_and_f111_read_their_real_figures_only_while_retail() {
        // BMB-B2 (coordinator first pass, 2026-11-15).
        // B-1B: maximum 477,000, fuel 265,274, thrust 69,560 / 123,120, the
        // high-altitude fast edge at Mach 1.25 (1,210 ft/s above 36,089 ft).
        let mut rows = plain_rows(-2, 5);
        let row = |p: &[[i32; 2]]| p.to_vec();
        rows[2].1 = row(&[
            [170, 0],
            [200, 15_500],
            [290, 34_250],
            [420, 46_625],
            [610, 58_000],
            [917, 57_875],
            [1_235, 52_750],
            [1_465, 42_375],
            [1_475, 36_000],
            [1_180, 0],
        ]);
        let one = row(&[
            [220, 0],
            [250, 15_000],
            [350, 33_500],
            [460, 44_750],
            [630, 55_000],
            [900, 55_000],
            [1_200, 50_000],
            [1_400, 41_500],
            [1_410, 36_000],
            [1_120, 0],
        ]);
        rows[1].1 = one.clone();
        rows[3].1 = one;
        let b1b = [
            ("engines", 4),
            ("weight", 192_000),
            ("thrust", 58_400),
            ("aftThrust", 140_000),
            ("maxTakeoffWeight", 447_000),
            ("internalFuel", 105_000),
            ("fuelConsumption", 6),
            ("aftFuelConsumption", 16),
        ];
        let text = batch_text("B1.PT", "B-1B", "B- 1B Lancer", 636, (-2, 5), &rows, &b1b);
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::B1b);
        assert_eq!(
            (a.number("thrust"), a.number("aftThrust")),
            (69_560., 123_120.)
        );
        assert_eq!(
            (a.number("internalFuel"), a.number("maxTakeoffWeight")),
            (265_274., 477_000.)
        );
        // Weight, ceiling and the flows stay.
        assert_eq!(a.number("weight"), 192_000.);
        assert_eq!(a.number("fuelConsumption"), 6.);
        assert!(a.number("weight") + a.number("internalFuel") < a.number("maxTakeoffWeight"));
        let env = |g: i32| a.envelopes.iter().find(|e| e.g == g).unwrap();
        assert_eq!(
            env(1).points[6..],
            [
                [1_200., 50_000.],
                [1_210., 41_500.],
                [1_210., 36_000.],
                [1_120., 0.]
            ]
        );
        // The 0 G polygon keeps its 65 ft/s step over the 1 G edge.
        assert_eq!(
            env(0).points[6..],
            [
                [1_235., 52_750.],
                [1_275., 42_375.],
                [1_275., 36_000.],
                [1_180., 0.]
            ]
        );
        // The -1 G row repeated the 1 G row and is held to the new edge.
        assert_eq!(env(-1).points[7..9], [[1_210., 41_500.], [1_210., 36_000.]]);
        // One changed number keeps the whole record retail.
        let edited = b1b.map(|(k, v)| if k == "thrust" { (k, 58_500) } else { (k, v) });
        let text = batch_text(
            "B1.PT",
            "B-1B",
            "B- 1B Lancer",
            636,
            (-2, 5),
            &rows,
            &edited,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(
            (a.number("thrust"), a.number("maxTakeoffWeight")),
            (58_500., 447_000.)
        );
        assert_eq!(
            a.envelopes.iter().find(|e| e.g == 1).unwrap().points[8],
            [1_410., 36_000.]
        );
        // Tu-160: empty 242,500, fuel 326,285, thrust 123,600 / 220,400; the
        // maximum, engines and ceiling stay.
        let tu160 = [
            ("engines", 4),
            ("weight", 202_260),
            ("maxAlt", 50_000),
            ("thrust", 158_092),
            ("aftThrust", 232_398),
            ("maxTakeoffWeight", 606_260),
            ("internalFuel", 368_000),
        ];
        let text = batch_text(
            "TU160.PT",
            "Tu-160",
            "Tu-160 Blackjack-A",
            588,
            (-2, 5),
            &plain_rows(-2, 5),
            &tu160,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::Tu160);
        assert_eq!(
            (a.number("weight"), a.number("internalFuel")),
            (242_500., 326_285.)
        );
        assert_eq!(
            (a.number("thrust"), a.number("aftThrust")),
            (123_600., 220_400.)
        );
        assert_eq!(
            (a.number("maxTakeoffWeight"), a.number("maxAlt")),
            (606_260., 50_000.)
        );
        // F-111F: fuel 32,663 lb (the record's 5,025 is gallons), thrust
        // 29,120 / 50,200 and the flows refitted to 5 and 35 lb/s.
        let f111 = [
            ("engines", 2),
            ("weight", 47_481),
            ("thrust", 50_200),
            ("aftThrust", 80_000),
            ("maxTakeoffWeight", 100_000),
            ("internalFuel", 5_025),
            ("fuelConsumption", 3),
            ("aftFuelConsumption", 6),
        ];
        let text = batch_text(
            "F111.PT",
            "F-111F",
            "F-111F Aardvark",
            660,
            (-2, 5),
            &plain_rows(-2, 5),
            &f111,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::F111);
        assert_eq!(a.number("internalFuel"), 32_663.);
        assert_eq!(
            (a.number("thrust"), a.number("aftThrust")),
            (29_120., 50_200.)
        );
        assert_eq!(
            (a.number("fuelConsumption"), a.number("aftFuelConsumption")),
            (5., 35.)
        );
        assert_eq!(a.number("weight"), 47_481.);
        assert!(a.number("weight") + a.number("internalFuel") < a.number("maxTakeoffWeight"));
        // A record whose fuel is already in pounds is left as it is.
        let edited = f111.map(|(k, v)| {
            if k == "internalFuel" {
                (k, 32_663)
            } else {
                (k, v)
            }
        });
        let text = batch_text(
            "F111.PT",
            "F-111F",
            "F-111F Aardvark",
            660,
            (-2, 5),
            &plain_rows(-2, 5),
            &edited,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(
            (a.number("thrust"), a.number("fuelConsumption")),
            (50_200., 3.)
        );
    }
    #[test]
    fn the_tu26_tu95_and_su34_read_their_real_figures_only_while_retail() {
        // BMB-C2 (coordinator first pass, 2026-11-15).
        let row = |p: &[[i32; 2]]| p.to_vec();
        // Tu-26: empty 127,868, thrust 66,000 / 110,000, ceiling 43,635 ft
        // (every polygon cut there, the 0 G one 5,500 ft higher) and the
        // 1,080 kt (1,823 ft/s) high-altitude edge.
        let mut rows = plain_rows(-2, 4);
        rows[2].1 = row(&[
            [180, 0],
            [225, 18_000],
            [313, 39_000],
            [521, 54_000],
            [870, 65_000],
            [1_337, 65_500],
            [1_840, 52_750],
            [2_084, 36_000],
            [1_040, 0],
        ]);
        rows[3].1 = row(&[
            [245, 0],
            [309, 19_500],
            [423, 38_000],
            [591, 50_000],
            [870, 60_000],
            [1_302, 60_000],
            [1_712, 50_000],
            [1_933, 36_000],
            [1_010, 0],
        ]);
        let tu26 = [
            ("engines", 2),
            ("weight", 110_230),
            ("maxAlt", 60_000),
            ("thrust", 59_986),
            ("aftThrust", 88_180),
            ("maxTakeoffWeight", 273_370),
            ("internalFuel", 110_230),
            ("fuelConsumption", 10),
            ("aftFuelConsumption", 66),
        ];
        let text = batch_text(
            "TU26.PT",
            "Tu-26",
            "Tu- 26 Backfire-C",
            660,
            (-2, 4),
            &rows,
            &tu26,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::Tu26);
        assert_eq!(
            (a.number("weight"), a.number("maxAlt")),
            (127_868., 43_635.)
        );
        assert_eq!(
            (a.number("thrust"), a.number("aftThrust")),
            (66_000., 110_000.)
        );
        // Kept: the maximum, the fuel and its flows.
        assert_eq!(
            (a.number("maxTakeoffWeight"), a.number("internalFuel")),
            (273_370., 110_230.)
        );
        assert_eq!(
            (a.number("fuelConsumption"), a.number("aftFuelConsumption")),
            (10., 66.)
        );
        assert!(a.number("weight") + a.number("internalFuel") < a.number("maxTakeoffWeight"));
        let env = |a: &Aircraft, g: i32| a.envelopes.iter().find(|e| e.g == g).unwrap().clone();
        assert_eq!(
            env(&a, 1).points,
            [
                [245., 0.],
                [309., 19_500.],
                [423., 38_000.],
                [502., 43_635.],
                [1_812., 43_635.],
                [1_823., 36_000.],
                [1_010., 0.]
            ]
        );
        assert_eq!(
            env(&a, 0).points,
            [
                [180., 0.],
                [225., 18_000.],
                [313., 39_000.],
                [454., 49_135.],
                [1_893., 49_135.],
                [1_974., 36_000.],
                [1_040., 0.]
            ]
        );
        // One changed number keeps the whole record retail, polygons too.
        let edited = tu26.map(|(k, v)| if k == "thrust" { (k, 60_000) } else { (k, v) });
        let text = batch_text(
            "TU26.PT",
            "Tu-26",
            "Tu- 26 Backfire-C",
            660,
            (-2, 4),
            &rows,
            &edited,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(
            (a.number("weight"), a.number("maxAlt")),
            (110_230., 60_000.)
        );
        assert_eq!(env(&a, 1).points[7], [1_933., 36_000.]);
        // Tu-95: thrust 129,000, empty 198,415, fuel 148,150, maximum 414,470.
        let tu95 = [
            ("engines", 4),
            ("weight", 264_550),
            ("thrust", 355_080),
            ("aftThrust", 0),
            ("maxTakeoffWeight", 407_850),
            ("internalFuel", 100_000),
            ("fuelConsumption", 4),
        ];
        let text = batch_text(
            "TU95.PT",
            "Tu-95",
            "Tu- 95 Bear-H",
            660,
            (-2, 5),
            &plain_rows(-2, 5),
            &tu95,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::Tu95);
        assert_eq!(
            (a.number("weight"), a.number("internalFuel")),
            (198_415., 148_150.)
        );
        assert_eq!(
            (a.number("thrust"), a.number("maxTakeoffWeight")),
            (129_000., 414_470.)
        );
        assert_eq!(a.number("fuelConsumption"), 4.);
        assert!(a.number("weight") + a.number("internalFuel") < a.number("maxTakeoffWeight"));
        let edited = tu95.map(|(k, v)| if k == "weight" { (k, 264_551) } else { (k, v) });
        let text = batch_text(
            "TU95.PT",
            "Tu-95",
            "Tu- 95 Bear-H",
            660,
            (-2, 5),
            &plain_rows(-2, 5),
            &edited,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(
            (a.number("thrust"), a.number("maxTakeoffWeight")),
            (355_080., 407_850.)
        );
        // Su-34: empty 49,604, fuel 26,676, dry thrust 33,800 and the Mach
        // 1.8 edge (1,742 ft/s above 36,000 ft).
        let mut rows = plain_rows(-2, 4);
        rows[2].1 = row(&[
            [120, 0],
            [160, 13_000],
            [225, 27_000],
            [313, 39_000],
            [501, 50_000],
            [870, 58_000],
            [1_725, 58_000],
            [2_417, 52_000],
            [2_600, 36_000],
            [1_835, 18_000],
            [1_280, 0],
        ]);
        rows[3].1 = row(&[
            [180, 0],
            [225, 13_000],
            [307, 26_000],
            [433, 39_000],
            [601, 48_000],
            [870, 55_000],
            [1_704, 55_000],
            [2_308, 49_500],
            [2_473, 36_000],
            [1_731, 18_000],
            [1_250, 0],
        ]);
        let su34 = [
            ("engines", 2),
            ("weight", 36_000),
            ("maxAlt", 55_000),
            ("thrust", 40_755),
            ("aftThrust", 59_510),
            ("maxTakeoffWeight", 97_000),
            ("internalFuel", 20_000),
        ];
        let text = batch_text(
            "SU34.PT",
            "Su-34",
            "Su-34 Platypus",
            660,
            (-2, 4),
            &rows,
            &su34,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::Su34);
        assert_eq!(
            (a.number("weight"), a.number("internalFuel")),
            (49_604., 26_676.)
        );
        assert_eq!(
            (a.number("thrust"), a.number("aftThrust")),
            (33_800., 59_510.)
        );
        assert_eq!(
            (a.number("maxTakeoffWeight"), a.number("maxAlt")),
            (97_000., 55_000.)
        );
        assert!(a.number("weight") + a.number("internalFuel") < a.number("maxTakeoffWeight"));
        assert_eq!(
            env(&a, 1).points[6..],
            [
                [1_704., 55_000.],
                [1_742., 49_500.],
                [1_742., 36_000.],
                [1_731., 18_000.],
                [1_250., 0.]
            ]
        );
        // The 0 G polygon keeps its 127 ft/s step over the 1 G edge.
        assert_eq!(
            env(&a, 0).points[6..],
            [
                [1_725., 58_000.],
                [1_869., 52_000.],
                [1_869., 36_000.],
                [1_835., 18_000.],
                [1_280., 0.]
            ]
        );
        let edited = su34.map(|(k, v)| if k == "thrust" { (k, 40_756) } else { (k, v) });
        let text = batch_text(
            "SU34.PT",
            "Su-34",
            "Su-34 Platypus",
            660,
            (-2, 4),
            &rows,
            &edited,
        );
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.number("weight"), 36_000.);
        assert_eq!(env(&a, 1).points[8], [2_473., 36_000.]);
    }
    #[test]
    fn the_mirage_f1_reads_one_atar_9k_50_and_the_f5e_its_real_fuel_g_and_ceiling() {
        // Mirage F1: 11,020 / 15,873 lbf, retail weights.
        let mf1 = [
            ("engines", 1),
            ("weight", 15_542),
            ("thrust", 15_000),
            ("aftThrust", 30_000),
            ("maxTakeoffWeight", 30_203),
            ("internalFuel", 6_120),
            ("fuelConsumption", 3),
            ("aftFuelConsumption", 23),
        ];
        let mut rows = plain_rows(-3, 7);
        rows[10].1 = vec![
            [520, 0],
            [645, 11_500],
            [900, 20_000],
            [1_075, 20_000],
            [1_240, 17_000],
            [1_230, 0],
        ];
        let text = batch_text("MF1.PT", "MF1", "Mirage F1", 636, (-3, 7), &rows, &mf1);
        let a = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(a.id, AircraftId::Mf1);
        assert_eq!((a.number("engines"), a.number("thrust")), (1., 11_020.));
        assert_eq!(a.number("aftThrust"), 15_873.);
        assert_eq!(a.number("weight"), 15_542.);
        assert_eq!(a.number("maxTakeoffWeight"), 30_203.);
        // B5-4 (John, 2026-11-11): the Atar's reheat flow, 9 lb/s (the dry 3
        // stays); +8 and -4 rows, the -4 row the +8 polygon.
        assert_eq!(
            (a.number("fuelConsumption"), a.number("aftFuelConsumption")),
            (3., 9.)
        );
        assert_eq!((a.number("envMin"), a.number("envMax")), (-4., 8.));
        let gs: Vec<i32> = a.envelopes.iter().map(|e| e.g).collect();
        assert_eq!(gs, (-4..=8).collect::<Vec<_>>());
        assert_eq!(a.envelopes[0].points, a.envelopes[12].points);
        assert_eq!(a.envelopes[12].points[0], [556., 0.]);
        // F-5E: fuel 4,400 lb, +8 G (B5-4), 51,800 ft.
        let mut rows = plain_rows(-4, 8);
        let minus_four = vec![[570, 0], [610, 4_000], [730, 3_000], [760, 0]];
        rows[0].1 = minus_four.clone();
        rows[12].1 = minus_four;
        rows[4].1 = vec![
            [200, 0],
            [255, 21_000],
            [317, 30_500],
            [418, 39_750],
            [570, 44_000],
            [995, 44_000],
            [1_607, 41_000],
            [1_940, 36_000],
            [1_545, 18_000],
            [1_270, 0],
        ];
        rows[5].1 = vec![
            [240, 0],
            [295, 21_000],
            [357, 30_500],
            [458, 38_750],
            [580, 42_000],
            [965, 42_000],
            [1_607, 39_000],
            [1_840, 36_000],
            [1_475, 18_000],
            [1_220, 0],
        ];
        let f5e = [
            ("weight", 9_558),
            ("maxAlt", 42_000),
            ("thrust", 7_000),
            ("aftThrust", 10_000),
            ("maxTakeoffWeight", 24_664),
            ("internalFuel", 6_240),
            ("fuelConsumption", 1),
            ("aftFuelConsumption", 8),
        ];
        let text = batch_text("F5E.PT", "F-5", "F-  5E Tiger", 612, (-4, 8), &rows, &f5e);
        let f = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(f.id, AircraftId::F5E);
        assert_eq!(f.number("internalFuel"), 4_400.);
        assert_eq!(f.number("maxAlt"), 51_800.);
        assert_eq!(
            (f.number("thrust"), f.number("aftThrust")),
            (7_000., 10_000.)
        );
        // B5-4 (John, 2026-11-11): J85 flows 2 and 6 lb/s; the -4 sliver is
        // retired (-3) and the top row is a fitted +8.
        assert_eq!(
            (f.number("fuelConsumption"), f.number("aftFuelConsumption")),
            (2., 6.)
        );
        assert_eq!((f.number("envMin"), f.number("envMax")), (-3., 8.));
        assert_eq!(f.envelopes.len(), 12);
        let one = f.envelopes.iter().find(|e| e.g == 1).unwrap();
        assert_eq!(one.points[4], [580., 51_800.]);
        // B5-5 (John, 2026-11-11): the 1 G fast side is trimmed by 0.92, so
        // the top-edge plateau ends at 888 ft/s (965 x 0.92) and the fastest
        // point is 1,693 ft/s (1,840 x 0.92, Mach 1.64 at 36,000 ft).
        assert_eq!(one.points[5], [888., 51_800.]);
        assert_eq!(one.points[7], [1_693., 36_000.]);
        // The desert F-5E has its own record and keeps the retail ceiling.
        // One value off the copy (the fuel) leaves the whole ceiling and fuel
        // as they were, but the repeated +8 row is still retired.
        let mut edited = f5e;
        edited[5].1 = 6_241;
        let text = batch_text(
            "F5E.PT",
            "F-5",
            "F-  5E Tiger",
            612,
            (-4, 8),
            &rows,
            &edited,
        );
        let e = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(
            (e.number("internalFuel"), e.number("maxAlt")),
            (6_241., 42_000.)
        );
        let top = |e: &Aircraft| {
            e.envelopes
                .iter()
                .find(|r| r.g == 1)
                .unwrap()
                .points
                .iter()
                .map(|p| p[1])
                .fold(f64::MIN, f64::max)
        };
        assert_eq!(top(&e), 42_000.);
        assert_eq!(top(&f), 51_800.);
    }
    #[test]
    fn the_f15e_reads_real_flows_a_60_000_ft_ceiling_and_a_plus_10_row_the_f15c_stays_retail() {
        // Opinionated (John, 2026-11-11, B5-4). The F-15E's record shares its
        // flight words with the F-15C's, which is its own, retail, record.
        let mut rows = plain_rows(-4, 9);
        rows[0].1 = vec![[910, 0], [920, 1_000], [940, 0], [950, 0]];
        rows[4].1 = vec![
            [140, 0],
            [531, 74_000],
            [870, 105_000],
            [1_574, 105_000],
            [1_440, 0],
        ];
        rows[5].1 = vec![
            [200, 0],
            [563, 65_000],
            [870, 100_000],
            [1_554, 100_000],
            [1_360, 0],
        ];
        rows[13].1 = vec![
            [585, 0],
            [687, 5_500],
            [940, 10_000],
            [1_020, 9_000],
            [1_180, 0],
        ];
        let retail = [
            ("maxAlt", 100_000),
            ("fuelConsumption", 4),
            ("aftFuelConsumption", 24),
            ("thrust", 32_558),
            ("aftThrust", 47_860),
        ];
        let text = batch_text(
            "F15E.PT",
            "F-15E",
            "F- 15E Strike Eagle",
            660,
            (-4, 9),
            &rows,
            &retail,
        );
        let e = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(e.id, AircraftId::F15E);
        assert_eq!(
            (e.number("fuelConsumption"), e.number("aftFuelConsumption")),
            (7., 26.)
        );
        assert_eq!(e.number("maxAlt"), 60_000.);
        // Thrust is John's earlier decision and stays.
        assert_eq!(
            (e.number("thrust"), e.number("aftThrust")),
            (32_558., 47_860.)
        );
        assert_eq!((e.number("envMin"), e.number("envMax")), (-3., 10.));
        let gs: Vec<i32> = e.envelopes.iter().map(|r| r.g).collect();
        assert_eq!(gs, (-3..=10).collect::<Vec<_>>());
        assert_eq!(e.envelopes.last().unwrap().points[0], [617., 0.]);
        let one = e.envelopes.iter().find(|r| r.g == 1).unwrap();
        assert_eq!(
            one.points,
            vec![[200., 0.], [535., 60_000.], [1_476., 60_000.], [1_360., 0.]]
        );
        // The F-15C record is another identity: the same numbers on it stay
        // retail (no aliasing of F15.PT to F15E.PT).
        let token = |v: &str| Token {
            kind: "dword".into(),
            value: v.into(),
            scaled: false,
        };
        let mut object = BTreeMap::from([("maxAlt".to_string(), token("100000"))]);
        let mut plane = BTreeMap::new();
        assert!(!corrections::apply(
            AircraftId::F15,
            &mut object,
            &mut plane
        ));
        assert_eq!(object["maxAlt"].value, "100000");
        // One flow off the retail value leaves the whole record as it was.
        let mut edited = retail;
        edited[1].1 = 5;
        let t = batch_text(
            "F15E.PT",
            "F-15E",
            "F- 15E Strike Eagle",
            660,
            (-4, 9),
            &rows,
            &edited,
        );
        let x = Aircraft::parse(t.as_bytes()).unwrap();
        assert_eq!(
            (x.number("fuelConsumption"), x.number("maxAlt")),
            (5., 100_000.)
        );
    }
    #[test]
    fn the_gripen_reads_rm12_flows_a_50_000_ft_ceiling_and_no_minus_4_row() {
        // Opinionated (John, 2026-11-11, B5-4): the +9 limit stays.
        let mut rows = plain_rows(-4, 9);
        let plus_eight = vec![[520, 0], [615, 11_000], [920, 21_000], [1_010, 0]];
        rows[0].1 = plus_eight.clone();
        rows[12].1 = plus_eight;
        rows[4].1 = vec![[160, 0], [870, 63_000], [1_734, 63_000], [1_280, 0]];
        rows[5].1 = vec![[200, 0], [870, 60_000], [1_684, 60_000], [1_230, 0]];
        let retail = [
            ("maxAlt", 60_000),
            ("fuelConsumption", 2),
            ("aftFuelConsumption", 14),
            ("thrust", 12_140),
            ("aftThrust", 18_100),
            ("internalFuel", 5_000),
            ("weight", 14_599),
            ("maxTakeoffWeight", 27_560),
        ];
        let text = batch_text(
            "GRIPEN.PT",
            "Gripen",
            "Saab Gripen",
            660,
            (-4, 9),
            &rows,
            &retail,
        );
        let g = Aircraft::parse(text.as_bytes()).unwrap();
        assert_eq!(g.id, AircraftId::Gripen);
        assert_eq!(
            (g.number("fuelConsumption"), g.number("aftFuelConsumption")),
            (3., 9.)
        );
        assert_eq!(g.number("maxAlt"), 50_000.);
        assert_eq!((g.number("envMin"), g.number("envMax")), (-3., 9.));
        assert_eq!(g.envelopes.len(), 13);
        assert_eq!(g.envelopes[0].g, -3);
        let one = g.envelopes.iter().find(|r| r.g == 1).unwrap();
        assert_eq!(one.points[1], [758., 50_000.]);
        let zero = g.envelopes.iter().find(|r| r.g == 0).unwrap();
        assert_eq!(zero.points[1], [757., 53_000.]);
        // Thrust and fuel are the real aircraft's and stay.
        assert_eq!(
            (g.number("thrust"), g.number("internalFuel")),
            (12_140., 5_000.)
        );
        // One value off the retail flows (a changed thrust is not a guard) or
        // a stale ceiling field: the flows and ceiling stay retail.
        let mut edited = retail;
        edited[2].1 = 15;
        let t = batch_text(
            "GRIPEN.PT",
            "Gripen",
            "Saab Gripen",
            660,
            (-4, 9),
            &rows,
            &edited,
        );
        let x = Aircraft::parse(t.as_bytes()).unwrap();
        assert_eq!(
            (x.number("aftFuelConsumption"), x.number("maxAlt")),
            (15., 60_000.)
        );
    }
    #[test]
    fn typed_profile_rejects_misalignment_and_wrong_identity() {
        let t = fixture();
        let a = Aircraft::parse(t.as_bytes()).unwrap();
        assert_eq!(a.number("weight"), 1000.);
        assert_eq!(a.envelopes.len(), 2);
        assert!(Aircraft::parse(t.replacen("word 660", "dword 660", 1).as_bytes()).is_err());
        assert!(Aircraft::parse(t.replace("F18.PT", "OTHER.PT").as_bytes()).is_err());
        assert!(Aircraft::parse(t.replace("F18.PT", "RAFALE.PT").as_bytes()).is_ok());
        assert!(Aircraft::parse(t.replace("F18.PT", "RAFALEE.PT").as_bytes()).is_err());
        assert!(Aircraft::parse(t.replacen("dword 100", "dword 3000", 1).as_bytes()).is_err());
    }
}

#[cfg(test)]
mod dependency_tests {
    use super::*;
    fn archive(mut resources: BTreeMap<String, Vec<u8>>, omit: Option<&str>) -> Archive {
        if let Some(name) = omit {
            resources.remove(name);
        }
        let count = resources.len();
        let mut data = vec![0; 7 + (count + 1) * 18];
        data[..5].copy_from_slice(b"EALIB");
        data[5..7].copy_from_slice(&(count as u16).to_le_bytes());
        for (i, (name, bytes)) in resources.iter().enumerate() {
            let at = 7 + i * 18;
            data[at..at + name.len()].copy_from_slice(name.as_bytes());
            let offset = data.len() as u32;
            data[at + 14..at + 18].copy_from_slice(&offset.to_le_bytes());
            data.extend(bytes);
        }
        let end = data.len() as u32;
        let at = 7 + count * 18 + 14;
        data[at..at + 4].copy_from_slice(&end.to_le_bytes());
        Archive::parse(data).unwrap()
    }
    #[test]
    fn reviewed_missing_pts_profiles_still_require_their_own_aircraft_and_hud() {
        for id in [
            AircraftId::Mig29,
            AircraftId::Su27,
            AircraftId::Su25,
            AircraftId::Mig23,
        ] {
            let mut r = resources();
            r.remove(&id.pt().replace(".PT", ".PTS"));
            let a = archive(r.clone(), None);
            assert!(dependency_report(&[&a], &[id], false).is_ok());
            for required in [id.pt(), id.hud()] {
                let a = archive(r.clone(), Some(required));
                assert!(dependency_report(&[&a], &[id], false).is_err());
            }
        }
    }
    fn resources() -> BTreeMap<String, Vec<u8>> {
        let mut resources = BTreeMap::new();
        for &name in COMBAT_RESOURCES {
            resources.insert(name.into(), vec![0]);
        }
        for name in ["AIM9M.JT", "MK82.JT"] {
            resources.insert(name.into(), vec![0]);
        }
        for name in ["PALETTE.PAL", "WIN11.FNT", "HUD11.FNT", "FMENUD.MNU"] {
            resources.insert(name.into(), vec![0]);
        }
        for id in AircraftId::ALL {
            let sound = format!("&{}.11K", id.stem());
            resources.insert(
                id.pt().into(),
                format!("[brent's_relocatable_format]\nstring \"{sound}\"\nend\n").into_bytes(),
            );
            resources.insert(id.hud().into(), vec![0]);
            resources.insert(id.pt().replace(".PT", ".PTS"), vec![0]);
            resources.insert(id.cockpit().into(), vec![0]);
            resources.insert(id.instrument_panel(), vec![0]);
            resources.insert(
                format!("{}.SH", id.stem()),
                format!("_{}.PIC", id.stem()).into_bytes(),
            );
            resources.insert(format!("_{}.PIC", id.stem()), vec![0]);
            resources.insert(sound, vec![0]);
        }
        resources
    }
    #[test]
    fn selected_profile_follows_its_own_texture_and_audio_and_union_keeps_both() {
        let a = archive(resources(), None);
        let rafale = dependencies(&[&a], &[AircraftId::Rafale], false).unwrap();
        for name in [
            "RAFALE.PT",
            "RAFALE.HUD",
            "RAF.SH",
            "~RAFH.PIC",
            "_RAF.PIC",
            "&RAF.11K",
        ] {
            assert!(rafale.contains(name), "{name}");
        }
        for name in ["F18.PT", "F18.SH", "~F18H.PIC", "_F18.PIC", "&F18.11K"] {
            assert!(!rafale.contains(name), "{name}");
        }
        let both = dependencies(&[&a], &AircraftId::ALL, false).unwrap();
        assert!(both.is_superset(&rafale));
        assert!(both.contains("F18.PT"));
        assert!(both.contains("&F18.11K"));
    }
    #[test]
    fn each_cockpit_family_requires_its_own_instrument_window_frame() {
        let a = archive(resources(), None);
        let rafale = dependencies(&[&a], &[AircraftId::Rafale], false).unwrap();
        assert!(rafale.contains("~RAF_P.PIC"));
        assert!(!rafale.contains("~F18_P.PIC"));
        assert_eq!(AircraftId::A4E.instrument_panel(), "~F4_P.PIC");
        assert_eq!(AircraftId::Faxx.instrument_panel(), "~F22_P.PIC");
        let missing = archive(resources(), Some("~RAF_P.PIC"));
        assert!(
            dependencies(&[&missing], &[AircraftId::Rafale], false)
                .unwrap_err()
                .to_string()
                .contains("~RAF_P.PIC")
        );
        assert!(dependencies(&[&missing], &[AircraftId::F18], false).is_ok());
    }
    #[test]
    fn missing_selected_shape_does_not_fall_back_to_other_aircraft() {
        let a = archive(resources(), Some("RAF.SH"));
        assert!(dependencies(&[&a], &[AircraftId::Rafale], false).is_err());
        assert!(dependencies(&[&a], &[AircraftId::F18], false).is_ok());
    }

    #[test]
    fn combat_roots_follow_textures_and_missing_art_fails() {
        let mut r = resources();
        // A texture the shape names that is not itself a combat root.
        r.insert("FIRE.SH".into(), b"FIREB.PIC\0".to_vec());
        let missing = archive(r.clone(), None);
        assert!(
            dependency_report(&[&missing], &[], true)
                .unwrap_err()
                .to_string()
                .contains("FIREB.PIC")
        );
        r.insert("FIREB.PIC".into(), vec![0]);
        let a = archive(r, None);
        let report = dependency_report(&[&a], &[], true).unwrap();
        assert!(report.resources.contains("FIREB.PIC"));
        assert!(
            report
                .edges
                .iter()
                .any(|e| e.source == "FIRE.SH" && e.target == "FIREB.PIC")
        );
    }

    #[test]
    fn cycles_and_override_dependencies_are_explicit() {
        let mut r = resources();
        r.insert(
            "A.JT".into(),
            b"[brent's_relocatable_format]\nstring \"B.JT\"\nsymbol _PROJProc\nend".to_vec(),
        );
        r.insert(
            "B.JT".into(),
            b"[brent's_relocatable_format]\nstring \"A.JT\"\nend".to_vec(),
        );
        let first = archive(r, None);
        let second = archive(
            BTreeMap::from([
                ("FIRE.SH".into(), b"ALT.PIC\0".to_vec()),
                ("ALT.PIC".into(), vec![0]),
            ]),
            None,
        );
        let report = dependency_report(&[&first, &second], &[], true).unwrap();
        assert_eq!(report.providers["FIRE.SH"], vec![0, 1]);
        assert!(report.resources.contains("ALT.PIC"));
        assert!(
            report
                .edges
                .iter()
                .any(|e| e.kind == "native-symbol-unimplemented" && !e.available)
        );
    }
    #[test]
    fn optional_station_seeds_are_explicit_required_host_edges() {
        for (id, seed) in [
            (AircraftId::Mig17, "MK82.JT"),
            (AircraftId::Mig23, "AIM9M.JT"),
        ] {
            let archive_with = archive(resources(), None);
            let report = dependency_report(&[&archive_with], &[id], false).unwrap();
            assert!(report.resources.contains(seed));
            assert!(report.edges.iter().any(|edge| edge.source == id.pt()
                && edge.target == seed
                && edge.kind == "host-retained-station-seed"
                && edge.available));
            let without = archive(resources(), Some(seed));
            let error = dependency_report(&[&without], &[id], false)
                .unwrap_err()
                .to_string();
            assert!(error.contains("host retained selection") && error.contains(seed));
            let unrelated = dependency_report(&[&archive_with], &[AircraftId::F18], false).unwrap();
            assert!(!unrelated.resources.contains(seed));
            assert!(dependency_report(&[&without], &[AircraftId::F18], false).is_ok());
        }
    }

    #[test]
    fn tank_thumbnails_follow_selected_gas_across_archives_without_guessing_shapes() {
        let mut data = resources();
        data.insert(
            "F18.PT".into(),
            b"[brent's_relocatable_format]\nstring \"TEST.GAS\"\nend\n".to_vec(),
        );
        data.insert("TEST.GAS".into(), b"[brent's_relocatable_format]\nbyte 8\nptr names\nword 100\nbyte 1\ndword 500\n:names\nstring \"Test\"\nstring \"Test tank\"\nstring \"TEST.GAS\"\nend\n".to_vec());
        data.insert("OTHER.GAS".into(), b"[brent's_relocatable_format]\nbyte 8\nptr names\nword 200\nbyte 1\ndword 600\n:names\nstring \"Other\"\nstring \"Other tank\"\nstring \"OTHER.GAS\"\nend\n".to_vec());
        data.insert("TEST.SH".into(), vec![0]);
        let definitions = archive(data, None);
        let pictures = archive(
            BTreeMap::from([
                ("$TEST.PIC".into(), vec![1]),
                ("$OTHER.PIC".into(), vec![2]),
            ]),
            None,
        );
        let report =
            dependency_report(&[&pictures, &definitions], &[AircraftId::F18], false).unwrap();
        assert!(report.resources.contains("TEST.GAS"));
        assert!(report.resources.contains("$TEST.PIC"));
        assert_eq!(report.providers["$TEST.PIC"], vec![0]);
        assert!(report.edges.iter().any(|edge| edge.source == "TEST.GAS"
            && edge.target == "$TEST.PIC"
            && edge.kind == "generated-tank-thumbnail"
            && edge.available));
        assert!(!report.resources.contains("$OTHER.PIC"));
        assert!(!report.resources.contains("TEST.SH"));
        let all = dependency_report(&[&pictures, &definitions], &[], true).unwrap();
        assert!(all.resources.contains("$TEST.PIC") && all.resources.contains("$OTHER.PIC"));
    }
    #[test]
    fn absent_tank_thumbnails_are_explicit_optional_edges() {
        let mut data = resources();
        data.insert("TEST.GAS".into(), b"[brent's_relocatable_format]\nbyte 8\nptr names\nword 100\nbyte 1\ndword 500\n:names\nstring \"Test\"\nstring \"Test tank\"\nstring \"TEST.GAS\"\nend\n".to_vec());
        let definitions = archive(data, None);
        let report = dependency_report(&[&definitions], &[], true).unwrap();
        assert!(report.resources.contains("TEST.GAS"));
        assert!(!report.resources.contains("$TEST.PIC"));
        assert!(report.edges.iter().any(|edge| edge.source == "TEST.GAS"
            && edge.target == "$TEST.PIC"
            && edge.kind == "generated-tank-thumbnail"
            && !edge.available));
    }

    #[test]
    fn compiled_pts_candidates_remain_explicit_without_inventing_missing_icons() {
        let mut r = resources();
        r.insert("F18.PTS".into(), b"MISSING.PIC\0".to_vec());
        let a = archive(r, None);
        let report = dependency_report(&[&a], &[AircraftId::F18], false).unwrap();
        assert!(report.resources.contains("F18.PTS"));
        assert!(report.edges.iter().any(|e| e.target == "MISSING.PIC"
            && e.kind == "unresolved-module-candidate"
            && !e.available));
    }
}
