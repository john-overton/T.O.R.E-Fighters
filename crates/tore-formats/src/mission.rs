//! Bounded inert reader for object placements in M/MM text resources.
use crate::{Archive, Result, invalid};
use std::collections::BTreeSet;

pub const MAX_PLACEMENTS: usize = 16_384;
const MAX_FIELDS: usize = 128;
const MAX_LINE: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceKey {
    pub layout: String,
    pub ordinal: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub key: SourceKey,
    pub section: Option<String>,
    pub object_type: String,
    pub position: [i32; 3],
    pub angles: [i32; 3],
    pub source_nationality: Option<i32>,
    /// True for the newer already-numbered nationality2 field.
    pub nationality2: bool,
    pub nationality: Option<i32>,
    pub flags: Option<i32>,
    pub speed: Option<i32>,
    pub name: Option<String>,
    pub alias: Option<i32>,
    /// Uninterpreted fields are retained in source order. Imported callbacks stay inert.
    pub unknown: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SideTables {
    pub sides: Vec<i32>,
    pub sides2: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub resource: String,
    pub map: Option<String>,
    pub sides: SideTables,
    pub placements: Vec<Placement>,
}

pub fn airport_section(section: Option<&str>) -> bool {
    section.is_some_and(|name| {
        let name = name.to_ascii_lowercase();
        name.contains("airport") || name.contains("airfield") || name.contains("airbase")
    })
}

fn integer(value: &str, context: &str) -> Result<i32> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('$') {
        return u32::from_str_radix(hex, 16)
            .map(|bits| bits as i32)
            .map_err(|_| invalid(&format!("{context}: invalid 32-bit hexadecimal integer")));
    }
    value
        .parse::<i32>()
        .map_err(|_| invalid(&format!("{context}: integer outside i32")))
}

fn vector(value: &str, context: &str) -> Result<[i32; 3]> {
    let values = value
        .split_whitespace()
        .map(|v| integer(v, context))
        .collect::<Result<Vec<_>>>()?;
    values
        .try_into()
        .map_err(|_| invalid(&format!("{context}: expected three integers")))
}

fn resource(value: &str, context: &str) -> Result<String> {
    let value = value.to_ascii_uppercase();
    if value.is_empty()
        || value.len() > 64
        || value
            .bytes()
            .any(|b| !b.is_ascii_alphanumeric() && !matches!(b, b'.' | b'_' | b'~' | b'$'))
    {
        return Err(invalid(&format!("{context}: invalid resource name")));
    }
    Ok(value)
}

fn display_name(value: &str, context: &str) -> Result<String> {
    let value = value.trim().trim_matches('\u{1}');
    if value.is_empty() || value.len() > 255 || value.chars().any(char::is_control) {
        return Err(invalid(&format!("{context}: invalid object name")));
    }
    Ok(value.to_owned())
}

impl Layout {
    pub fn parse(resource_name: &str, bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(invalid("mission layout exceeds limit"));
        }
        // Retail layouts are ASCII control text, but some display names contain
        // legacy code-page bytes. Lossy conversion affects display metadata only.
        let text = String::from_utf8_lossy(bytes);
        if !text.starts_with("textFormat") {
            return Err(invalid("expected textFormat mission"));
        }
        let layout = resource(resource_name, resource_name)?;
        let mut placements = Vec::new();
        let mut map = None;
        let mut sides = SideTables::default();
        let mut section = None;
        let mut record: Option<(usize, Vec<(String, String)>)> = None;
        let mut side_table: Option<&str> = None;
        for (line_index, raw) in text.lines().enumerate() {
            let line_no = line_index + 1;
            if raw.len() > MAX_LINE {
                return Err(invalid(&format!("{layout}:{line_no}: line exceeds limit")));
            }
            let trimmed = raw.trim_end_matches('\0').trim();
            if trimmed.starts_with(';') {
                let label = trimmed.trim_start_matches(';').trim_matches('-').trim();
                if !label.is_empty() {
                    section = Some(label.chars().take(255).collect());
                }
                continue;
            }
            if trimmed.is_empty() {
                continue;
            }
            let indented = raw.starts_with(char::is_whitespace);
            if let Some((start, fields)) = record.as_mut() {
                if trimmed == "." {
                    let ordinal = u32::try_from(placements.len())
                        .map_err(|_| invalid("placement ordinal overflow"))?;
                    placements.push(parse_record(
                        &layout,
                        ordinal,
                        *start,
                        section.clone(),
                        fields,
                    )?);
                    record = None;
                    if placements.len() > MAX_PLACEMENTS {
                        return Err(invalid("too many object placements"));
                    }
                    continue;
                }
                if !indented {
                    return Err(invalid(&format!(
                        "{layout}:{line_no}: unterminated object record"
                    )));
                }
                let (key, value) = trimmed.split_once(char::is_whitespace).ok_or_else(|| {
                    invalid(&format!("{layout}:{line_no}: object field missing value"))
                })?;
                if fields.iter().any(|(seen, _)| seen == key) {
                    return Err(invalid(&format!(
                        "{layout}:{line_no}: duplicate object field {key}"
                    )));
                }
                if fields.len() == MAX_FIELDS {
                    return Err(invalid(&format!(
                        "{layout}:{line_no}: too many object fields"
                    )));
                }
                fields.push((key.to_owned(), value.trim().to_owned()));
                continue;
            }
            if let Some(table) = side_table {
                if indented {
                    let value = integer(trimmed, &format!("{layout}:{line_no}"))?;
                    let target = if table == "sides" {
                        &mut sides.sides
                    } else {
                        &mut sides.sides2
                    };
                    if target.len() == 256 {
                        return Err(invalid(&format!(
                            "{layout}:{line_no}: side table exceeds limit"
                        )));
                    }
                    target.push(value);
                    continue;
                }
                side_table = None;
            }
            match trimmed {
                "obj" => record = Some((line_no, Vec::new())),
                "sides" => side_table = Some("sides"),
                "sides2" => side_table = Some("sides2"),
                _ if trimmed.starts_with("map ") => {
                    map = Some(resource(
                        trimmed[4..].trim(),
                        &format!("{layout}:{line_no}"),
                    )?)
                }
                _ => {}
            }
        }
        if let Some((start, _)) = record {
            return Err(invalid(&format!(
                "{layout}:{start}: unterminated object record"
            )));
        }
        for placement in &mut placements {
            placement.nationality = placement.source_nationality.map(|value| {
                if placement.nationality2 {
                    i32::from(value as u8)
                } else {
                    mission_nationality(map.as_deref(), value)
                }
            });
        }
        Ok(Self {
            resource: layout,
            map,
            sides,
            placements,
        })
    }
}

fn parse_record(
    layout: &str,
    ordinal: u32,
    line: usize,
    section: Option<String>,
    fields: &[(String, String)],
) -> Result<Placement> {
    let context = format!("{layout}:{line}");
    let find = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    let object_type = resource(
        find("type").ok_or_else(|| invalid(&format!("{context}: missing object type")))?,
        &context,
    )?;
    let position = vector(
        find("pos").ok_or_else(|| invalid(&format!("{context}: missing object position")))?,
        &context,
    )?;
    let angles = find("angle").map_or(Ok([0; 3]), |v| vector(v, &context))?;
    let optional_integer = |name| find(name).map(|v| integer(v, &context)).transpose();
    let known = [
        "type",
        "pos",
        "angle",
        "nationality",
        "nationality2",
        "flags",
        "speed",
        "name",
        "alias",
    ];
    if find("nationality").is_some() && find("nationality2").is_some() {
        return Err(invalid(&format!(
            "{context}: conflicting nationality fields"
        )));
    }
    Ok(Placement {
        key: SourceKey {
            layout: layout.to_owned(),
            ordinal,
        },
        section,
        object_type,
        position,
        angles,
        source_nationality: optional_integer("nationality2")?.or(optional_integer("nationality")?),
        nationality2: find("nationality2").is_some(),
        nationality: None,
        flags: optional_integer("flags")?,
        speed: optional_integer("speed")?,
        name: find("name")
            .map(|v| display_name(v, &context))
            .transpose()?,
        alias: optional_integer("alias")?,
        unknown: fields
            .iter()
            .filter(|(key, _)| !known.contains(&key.as_str()))
            .cloned()
            .collect(),
    })
}

/// Resolve explicit placement -> OT -> main SH -> PIC dependencies. The walk is
/// bounded and cycle-safe; no callback or mission statement is executed.
pub fn scene_dependencies(archives: &[&Archive], layouts: &[String]) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let read = |name: &str| -> Result<Vec<u8>> {
        for archive in archives {
            if archive.entries.contains_key(&name.to_ascii_uppercase()) {
                return archive.read(name);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("missing airport scene resource {name}"),
        ))
    };
    let mut definitions = std::collections::BTreeMap::<String, bool>::new();
    for layout in layouts {
        let bytes = read(layout)?;
        out.insert(layout.to_ascii_uppercase());
        let environment = crate::theater::Environment::parse(&bytes)?;
        let base = crate::theater::base_theater(&environment.map)
            .ok_or_else(|| invalid("unreviewed scenery map reference"))?;
        out.insert(format!("{base}.T2"));
        for placement in environment.textures.values() {
            let name = placement.resource_name(base);
            if out.insert(name.clone()) {
                read(&name)?;
            }
        }
        for placement in Layout::parse(layout, &bytes)?.placements {
            let associated = airport_section(placement.section.as_deref());
            definitions
                .entry(placement.object_type)
                .and_modify(|old| *old |= associated)
                .or_insert(associated);
        }
    }
    if definitions.len() > 2048 {
        return Err(invalid("scene definition count exceeds bound"));
    }
    for (definition_name, associated) in definitions {
        let bytes = read(&definition_name).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("{definition_name}: required by a scene placement: {error}"),
            )
        })?;
        let definition = crate::static_object::Definition::parse(&bytes).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("{definition_name}: static definition: {error}"),
            )
        })?;
        out.insert(definition_name.clone());
        let _ = associated;
        let Some(main_shape) = definition.main_shape else {
            continue;
        };
        out.insert(main_shape.clone());
        let shape_bytes = read(&main_shape).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("{main_shape}: referred by {definition_name}: {error}"),
            )
        })?;
        let shape = match crate::shape::Shape::scenery(&shape_bytes) {
            Ok(shape) => shape,
            Err(_) => continue,
        };
        for texture in shape
            .faces
            .into_iter()
            .map(|face| face.texture.to_ascii_uppercase())
        {
            if !texture.is_empty() {
                read(&texture).map_err(|error| {
                    std::io::Error::new(
                        error.kind(),
                        format!("{texture}: referred by {main_shape}: {error}"),
                    )
                })?;
                out.insert(texture);
            }
        }
    }
    Ok(out)
}

/// Reviewed map-dependent mission conversion. This resolves source nationality,
/// but never infers controller, side, permission, or autonomous behavior.
pub fn mission_nationality(map: Option<&str>, value: i32) -> i32 {
    let source = value as u8;
    let high = source & 0x80;
    let mut low = source & 0x7f;
    if low >= 8 {
        low = low.wrapping_add(1);
    }
    if map
        .map(|name| crate::theater::base_theater(name).unwrap_or(name))
        .and_then(|name| name.bytes().next())
        .is_some_and(|first| matches!(first.to_ascii_uppercase(), b'T' | b'U' | b'K'))
    {
        low = match low {
            5 => 23,
            6 => 24,
            13 => 22,
            14 => 20,
            15 => 21,
            other => other,
        };
    }
    i32::from(high | low)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variant_nationality_uses_the_same_theater_conversion() {
        for value in [5, 6, 12, 13, 14, 137] {
            assert_eq!(
                mission_nationality(Some("~UKR1.T2"), value),
                mission_nationality(Some("UKR.T2"), value)
            );
            assert_eq!(
                mission_nationality(Some("$FRA0.T2"), value),
                mission_nationality(Some("FRA.T2"), value)
            );
        }
    }
    const SAMPLE: &str = "textFormat\nmap ukr.T2\nsides\n\t0\n\t128\n;--- Kiev Airport\nobj\n\ttype HANGR.OT\n\tpos 10 0 -20\n\tangle -90 1 2\n\tnationality 137\n\tflags $4003\n\tname \u{1}Hangar One\u{1}\n\talias -10114\n\tfuture inert callback\n\t.\n";
    #[test]
    fn preserves_identity_signed_values_and_unknowns() {
        let l = Layout::parse("ukr.mm", SAMPLE.as_bytes()).unwrap();
        let p = &l.placements[0];
        assert_eq!(
            p.key,
            SourceKey {
                layout: "UKR.MM".into(),
                ordinal: 0
            }
        );
        assert_eq!(p.angles, [-90, 1, 2]);
        assert_eq!(p.alias, Some(-10114));
        assert_eq!(p.source_nationality, Some(137));
        assert_eq!(p.nationality, Some(138));
        assert_eq!(p.section.as_deref(), Some("Kiev Airport"));
        assert_eq!(p.unknown, vec![("future".into(), "inert callback".into())]);
    }
    #[test]
    fn hexadecimal_flags_retain_all_32_bits() {
        let source = SAMPLE.replace("flags $4003", "flags $FFFFFFFF");
        assert_eq!(
            Layout::parse("UKR.MM", source.as_bytes())
                .unwrap()
                .placements[0]
                .flags,
            Some(-1)
        );
        assert!(integer("$100000000", "test").is_err());
    }
    #[test]
    fn modern_nationality_is_not_legacy_remapped() {
        let text = SAMPLE.replace("nationality 137", "nationality2 137");
        let l = Layout::parse("UKR.MM", text.as_bytes()).unwrap();
        assert_eq!(l.placements[0].nationality, Some(137));
        assert!(l.placements[0].nationality2);
        assert_eq!(mission_nationality(Some("UKR.T2"), 255), 128);
        assert!(
            Layout::parse(
                "UKR.MM",
                SAMPLE
                    .replace("nationality 137", "nationality 137\n\tnationality2 137")
                    .as_bytes()
            )
            .is_err()
        );
    }
    #[test]
    fn rejects_duplicate_and_truncated_records() {
        assert!(
            Layout::parse(
                "X.MM",
                SAMPLE
                    .replace("\tpos 10 0 -20", "\tpos 1 2 3\n\tpos 4 5 6")
                    .as_bytes()
            )
            .is_err()
        );
        assert!(Layout::parse("X.MM", SAMPLE.trim_end_matches("\t.\n").as_bytes()).is_err());
    }
}
