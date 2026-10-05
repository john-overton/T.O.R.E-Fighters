//! Bounded BRF data recovery and aircraft dependency selection. Never executes modules.
use crate::{Archive, Result, invalid};
use std::collections::{BTreeMap, BTreeSet};
#[path = "aircraft_schema.rs"]
pub(crate) mod schema;
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
        for i in 0..self.points.len() {
            let a = self.points[i];
            let b = self.points[(i + 1) % self.points.len()];
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
}
impl AircraftId {
    /// Retail donor for an opinionated runtime variant.
    pub fn source(self) -> Self {
        if self == Self::Faxx { Self::F22n } else { self }
    }
    pub fn selection_key(self) -> &'static str {
        if self == Self::Faxx {
            "faxx"
        } else {
            self.pt()
        }
    }
    pub const SELECTABLE: [Self; 37] = [
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
    ];
    /// Retail import identities. Runtime variants reuse these dependencies.
    pub const ALL: [Self; 36] = [
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
            _ => Err(invalid(
                "supported aircraft: f18, rafale, f14, a4e, x31, mig29, su27, mig21, su25, mig23, su35, f22, f22n, faxx, c130, ac130, e3, il76, e2, av8, yak141, v22, ah64, mi24, ch47, mig17, f4b, f4j, f4e, f4g, a7, f15, f16c, f104, a10, b747, a310",
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
        }
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
        }
    }
    /// Primary installed gun. Multi-gun identities expose every type in `guns`.
    pub fn gun(self) -> Option<&'static str> {
        self.guns().first().copied()
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
        }
    }
    /// Reviewed optional left, centre and right cockpit overlays.
    pub fn cockpit_overlays(self) -> [Option<String>; 3] {
        let available = match self.cockpit_stem() {
            "AC130" | "M17" | "F16" | "F31" | "M21" | "F22" => [false; 3],
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
    pub fn parse(data: &[u8]) -> Result<Self> {
        let b = Brf::parse(data)?;
        let root = b.block("")?;
        let o = schema::OBJECT.len();
        let n = schema::NPC.len();
        if root.len() != o + n + schema::PLANE.len() {
            return Err(invalid("unsupported aircraft BRF layout"));
        }
        let object = fields(&root[..o], schema::OBJECT)?;
        let npc = fields(&root[o..o + n], schema::NPC)?;
        let plane = fields(&root[o + n..], schema::PLANE)?;
        let names = b.strings("ot_names")?;
        if names.len() != 3 {
            return Err(invalid("invalid aircraft identity"));
        }
        let id = AircraftId::parse(&names[2])?;
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
            });
        }
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
        if object["weight"].number()? as i64 + plane["internalFuel"].number()? as i64
            > plane["maxTakeoffWeight"].number()? as i64
        {
            return Err(invalid("aircraft empty plus fuel exceeds MTOW"));
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
        assert_eq!(AircraftId::Ah64.radar(), Some("F18R.SEE"));
        assert_eq!(AircraftId::parse("f4.pt").unwrap(), AircraftId::F4G);
        assert_ne!(AircraftId::F4B.pt(), AircraftId::F4J.pt());
        assert!(AircraftId::parse("mig15").is_err());
        assert!(AircraftId::parse("f4u").is_err());
        assert_eq!(AircraftId::C130.cockpit_overlays(), [None, None, None]);
        assert_eq!(AircraftId::F16C.cockpit_overlays(), [None, None, None]);
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
                    ("p", "envMin") => 0,
                    ("p", "envMax") => 1,
                    ("p", "internalFuel") => 100,
                    ("p", "thrust") => 200,
                    ("p", "maxTakeoffWeight") => 2000,
                    _ => 0,
                };
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
        for g in 0..2 {
            for (i, (kind, _)) in schema::ENVELOPE.iter().enumerate() {
                let v = match i {
                    0 => g,
                    1 => 3,
                    4 => 100,
                    6 => 200,
                    7 => 10000,
                    8 => 300,
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
