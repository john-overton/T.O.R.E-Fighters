//! Bounded BRF data recovery and aircraft dependency selection. Never executes modules.
use crate::{Archive, Result, invalid};
use std::collections::{BTreeMap, BTreeSet};
#[path = "aircraft_schema.rs"]
mod schema;
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
fn fields(tokens: &[Token], layout: &[(&str, &str)]) -> Result<BTreeMap<String, Token>> {
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
#[derive(Debug)]
pub struct Hardpoint {
    pub flags: i32,
    pub position: [i32; 3],
    pub store: Option<String>,
    pub count: i32,
    pub weight_class: i32,
}
/// Reviewed retail identities. Other variants require their own profile review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AircraftId {
    F18,
    Rafale,
}
impl AircraftId {
    pub const ALL: [Self; 2] = [Self::F18, Self::Rafale];
    pub fn parse(name: &str) -> Result<Self> {
        match name.to_ascii_lowercase().as_str() {
            "f18" | "f18.pt" => Ok(Self::F18),
            "rafale" | "rafale.pt" => Ok(Self::Rafale),
            _ => Err(invalid("supported aircraft: f18, rafale")),
        }
    }
    pub fn pt(self) -> &'static str {
        match self {
            Self::F18 => "F18.PT",
            Self::Rafale => "RAFALE.PT",
        }
    }
    pub fn hud(self) -> &'static str {
        match self {
            Self::F18 => "F18.HUD",
            Self::Rafale => "RAFALE.HUD",
        }
    }
    pub fn stem(self) -> &'static str {
        match self {
            Self::F18 => "F18",
            Self::Rafale => "RAF",
        }
    }
    pub fn cockpit(self) -> &'static str {
        match self {
            Self::F18 => "~F18H.PIC",
            Self::Rafale => "~RAFH.PIC",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::F18 => "F/A-18D Hornet",
            Self::Rafale => "Rafale C",
        }
    }
    pub fn gun(self) -> &'static str {
        match self {
            Self::F18 => "M61.JT",
            Self::Rafale => "DEFA.JT",
        }
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
        if object["structType"].number()? != 5 || object["typeSize"].number()? != 660 {
            return Err(invalid("expected reviewed FA aircraft layout (660)"));
        }
        let names = b.strings("ot_names")?;
        if names.len() != 3 {
            return Err(invalid("invalid aircraft identity"));
        }
        let id = AircraftId::parse(&names[2])?;
        if !names[2].eq_ignore_ascii_case(id.pt()) {
            return Err(invalid("aircraft identity must name its PT resource"));
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
pub const INSTRUMENT_ART: &[&str] = &[
    "EDGETL.PIC",
    "EDGETR.PIC",
    "EDGEBL.PIC",
    "EDGEBR.PIC",
    "EDGELR.PIC",
    "EDGETB.PIC",
    "PANEL.PIC",
    "PANELFNT.PIC",
    "PANELFND.PIC",
    "PANLFNT2.PIC",
];
/// Same dependency closure for CLI and app, independent of Python/reference checkout.
pub fn dependencies(
    archives: &[&Archive],
    aircraft: &[AircraftId],
    weapons: bool,
) -> Result<BTreeSet<String>> {
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
            "WIN11.FNT",
            "HUD11.FNT",
            "FMENUD.MNU",
            "PANEL.PIC",
        ] {
            if !catalog.contains(n) {
                return Err(invalid(&format!("aircraft import missing {n}")));
            }
            selected.insert(n.into());
        }
        for n in &catalog {
            if n.starts_with("&GEAR")
                || n.starts_with("&FLAP")
                || n.starts_with("&STALL")
                || n == "&HOOK.5K"
                || n == "&WIND.11K"
                || n == "&SQUEAL.5K"
                || n.starts_with(&format!("~{}", id.stem()))
                || n.starts_with(&format!("{}_", id.stem()))
                || n.starts_with(&format!("_{}", id.stem()))
                || INSTRUMENT_ART.contains(&n.as_str())
                || (n.starts_with("WIN") || n.starts_with("HUD")) && n.ends_with(".FNT")
            {
                selected.insert(n.clone());
            }
        }
    }
    if weapons {
        selected.extend(catalog.iter().filter(|n| n.ends_with(".JT")).cloned());
        selected.insert("PALETTE.PAL".into());
    }
    let mut pending: Vec<_> = selected.iter().cloned().collect();
    while let Some(name) = pending.pop() {
        if ![".PT", ".JT", ".SEE", ".ECM", ".GAS", ".SH", ".HUD"]
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
        let refs = references(&bytes);
        // BRF names represent actual files; report missing dependent data instead of silently dropping it.
        if bytes.starts_with(b"[brent's_relocatable_format]") {
            let brf = Brf::parse(&bytes)?;
            for t in brf.blocks.values().flatten().filter(|t| t.kind == "string") {
                let n = t.value.to_ascii_uppercase();
                if [".JT", ".SH", ".SEE", ".ECM", ".GAS", ".11K", ".5K"]
                    .iter()
                    .any(|e| n.ends_with(e))
                    && !catalog.contains(&n)
                {
                    return Err(invalid(&format!("{name} missing dependency {n}")));
                }
            }
        }
        for r in refs {
            for candidate in [r.clone(), format!("{r}.PIC")] {
                if catalog.contains(&candidate) && selected.insert(candidate.clone()) {
                    pending.push(candidate);
                }
            }
        }
        if selected.len() > 4096 {
            return Err(invalid("aircraft dependency limit exceeded"));
        }
    }
    Ok(selected)
}
#[cfg(test)]
mod tests {
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
    fn resources() -> BTreeMap<String, Vec<u8>> {
        let mut resources = BTreeMap::new();
        for name in [
            "PALETTE.PAL",
            "WIN11.FNT",
            "HUD11.FNT",
            "FMENUD.MNU",
            "PANEL.PIC",
        ] {
            resources.insert(name.into(), vec![0]);
        }
        for id in AircraftId::ALL {
            let sound = format!("&{}.11K", id.stem());
            resources.insert(
                id.pt().into(),
                format!("[brent's_relocatable_format]\nstring \"{sound}\"\nend\n").into_bytes(),
            );
            resources.insert(id.hud().into(), vec![0]);
            resources.insert(id.cockpit().into(), vec![0]);
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
    fn missing_selected_shape_does_not_fall_back_to_other_aircraft() {
        let a = archive(resources(), Some("RAF.SH"));
        assert!(dependencies(&[&a], &[AircraftId::Rafale], false).is_err());
        assert!(dependencies(&[&a], &[AircraftId::F18], false).is_ok());
    }
}
