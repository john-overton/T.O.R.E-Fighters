//! The bombing class of an aircraft, read from its imported record.
//!
//! Spec: `docs/spec/ai-strike.md`, "Bombing class". One function of the
//! record, so a newly imported aircraft needs no code (John, 2026-10-11:
//! "maybe we go by weight so we don't have to keep programming it in").
//! The test runs in this order, first match wins:
//!
//! 1. [`StrikeClass::Helicopter`]: the record's "no lift" flag (PLANE flags
//!    bit 8, every rotary-wing type, the tiltrotor and the blimp; the flight
//!    model's `LiftKind::Helicopter` and `Tiltrotor` are exactly the imported
//!    ones that carry it).
//! 2. [`StrikeClass::Gunship`]: the record gets the linked-gun gunsight
//!    ([`crate::combat::gunship::has_linked_guns`]).
//! 3. [`StrikeClass::Heavy`]: maximum takeoff weight [`HEAVY_WEIGHT_LB`] or
//!    more, or a positive G limit of [`HEAVY_G_LIMIT`] or less.
//! 4. [`StrikeClass::Attack`]: top speed [`ATTACK_TOP_SPEED_KT`] or less.
//! 5. [`StrikeClass::Fighter`]: everything else.
//!
//! The thresholds are opinionated (John, 2026-10-11); the order and the
//! top-speed test opinionated (agent). The escape hatch is [`OVERRIDES`]: a
//! data table keyed by PT resource name, empty for every current aircraft,
//! for the day the rule is wrong for an aircraft we care about.

use crate::combat::gunship;
use tore_formats::aircraft::Aircraft;

/// Maximum takeoff weight at which an aircraft is a Heavy, pounds.
pub const HEAVY_WEIGHT_LB: f64 = 150_000.;
/// Highest positive G limit of a Heavy.
pub const HEAVY_G_LIMIT: i32 = 3;
/// Highest top speed of an Attack aircraft, knots.
pub const ATTACK_TOP_SPEED_KT: f64 = 700.;
/// PLANE flag bit 8: the record's "no lift" mark.
const NO_LIFT_FLAG: i32 = 8;
/// Feet per second in a knot (the converter the aircraft catalog uses).
const FPS_PER_KNOT: f64 = 1.6878098571;

/// How an aircraft bombs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StrikeClass {
    /// Pop-up and stand-off fire; never dives.
    Helicopter,
    /// Orbits its target (the AC-130).
    Gunship,
    /// Level release from altitude only; no dive, pop-up, rockets or strafe.
    Heavy,
    /// Subsonic attack jets: the fighter passes plus strafe and rockets.
    Attack,
    /// The four retail fighter passes and the level run.
    #[default]
    Fighter,
}
crate::checkpoint_enum!(StrikeClass {
    Helicopter = 0,
    Gunship = 1,
    Heavy = 2,
    Attack = 3,
    Fighter = 4,
});
impl StrikeClass {
    pub const ALL: [Self; 5] = [
        Self::Helicopter,
        Self::Gunship,
        Self::Heavy,
        Self::Attack,
        Self::Fighter,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Helicopter => "Helicopter",
            Self::Gunship => "Gunship",
            Self::Heavy => "Heavy",
            Self::Attack => "Attack",
            Self::Fighter => "Fighter",
        }
    }
    /// The class of `record`: the rule, then an override when the table
    /// names the aircraft's PT resource.
    pub fn of(record: &Record) -> Self {
        Self::of_with(record, OVERRIDES)
    }
    /// [`Self::of`] with an explicit override table, for tests.
    pub fn of_with(record: &Record, overrides: &[(&str, Self)]) -> Self {
        overrides
            .iter()
            .find(|(resource, _)| resource.eq_ignore_ascii_case(&record.resource))
            .map_or_else(|| record.rule(), |(_, class)| *class)
    }
    /// The class of an imported aircraft record.
    pub fn of_aircraft(a: &Aircraft) -> Self {
        Self::of(&Record::from_aircraft(a))
    }
}

/// Per-aircraft overrides, by PT resource name (`"F117.PT"`), for an aircraft
/// the rule gets wrong. Empty for every current aircraft. Opinionated (agent):
/// an escape hatch, not a place to list aircraft. The F-117 (retail flies it
/// at 5,000 ft level) is the likely first.
pub const OVERRIDES: &[(&str, StrikeClass)] = &[];

/// The five facts the rule reads. Built from an imported record
/// ([`Record::from_aircraft`]) or, in tests and tools, from the aircraft
/// catalog's columns.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// PT resource name, upper case, for [`OVERRIDES`].
    pub resource: String,
    /// The "no lift" flag: rotary wing, tiltrotor, lighter than air.
    pub no_lift: bool,
    /// The record gets the AC-130's linked-gun gunsight.
    pub gunship: bool,
    /// Maximum takeoff weight, pounds.
    pub max_takeoff_lb: f64,
    /// Highest G row of the record's envelope.
    pub g_limit_max: i32,
    /// Fastest point of the 1 G envelope, knots, to the whole knot (the
    /// catalog's figure).
    pub top_speed_kt: f64,
}
impl Record {
    pub fn from_aircraft(a: &Aircraft) -> Self {
        let number = |key: &str| a.number(key);
        let top_fps = a
            .envelopes
            .iter()
            .find(|e| e.g == 1)
            .into_iter()
            .flat_map(|e| e.points.iter().map(|p| p[0]))
            .fold(0., f64::max);
        Self {
            resource: a.id.pt().to_ascii_uppercase(),
            no_lift: number("flags") as i32 & NO_LIFT_FLAG != 0,
            gunship: gunship::has_linked_guns(a.id),
            max_takeoff_lb: number("maxTakeoffWeight"),
            g_limit_max: number("envMax") as i32,
            top_speed_kt: (top_fps / FPS_PER_KNOT).round(),
        }
    }
    /// The rule alone, without the override table.
    pub fn rule(&self) -> StrikeClass {
        if self.no_lift {
            StrikeClass::Helicopter
        } else if self.gunship {
            StrikeClass::Gunship
        } else if self.max_takeoff_lb >= HEAVY_WEIGHT_LB || self.g_limit_max <= HEAVY_G_LIMIT {
            StrikeClass::Heavy
        } else if self.top_speed_kt <= ATTACK_TOP_SPEED_KT {
            StrikeClass::Attack
        } else {
            StrikeClass::Fighter
        }
    }
}

/// The player's default bomb mode when a bomb is selected: a Heavy selects
/// LVL whatever its attitude (retail heavies have no CCIP); every other
/// class keeps the situational rule ([`crate::combat::delivery::DeliveryMode::chosen`]).
/// Both modes stay available to the player on every aircraft. Opinionated
/// (John, 2026-10-11).
pub fn default_delivery(
    class: StrikeClass,
    ground_target: bool,
    flight_path: f64,
) -> crate::combat::delivery::DeliveryMode {
    use crate::combat::delivery::DeliveryMode;
    if class == StrikeClass::Heavy {
        DeliveryMode::Level
    } else {
        DeliveryMode::chosen(ground_target, flight_path)
    }
}

#[cfg(test)]
mod tests {
    use super::StrikeClass::{self, *};
    use super::*;
    use std::collections::BTreeMap;
    use tore_formats::aircraft::{AircraftId, Envelope, Token};

    /// The committed aircraft catalog: one row per retail record, with the
    /// columns the rule reads and the AI script the record binds.
    const CATALOG: &str = include_str!("../../../../docs/formats/fa-aircraft.csv");

    struct Row {
        resource: String,
        script: String,
        record: Record,
    }

    fn catalog() -> Vec<Row> {
        let mut lines = CATALOG.lines();
        let header: Vec<&str> = lines.next().expect("a header").split(',').collect();
        let column = |name: &str| header.iter().position(|h| *h == name).expect(name);
        let (resource, no_lift, weight, g, speed, script) = (
            column("resource"),
            column("no_lift"),
            column("max_takeoff_weight_lb"),
            column("g_limit_max"),
            column("top_speed_kt"),
            column("ai_script"),
        );
        lines
            .map(|line| {
                let f: Vec<&str> = line.split(',').collect();
                Row {
                    resource: f[resource].to_string(),
                    script: f[script].to_string(),
                    record: Record {
                        resource: f[resource].to_string(),
                        no_lift: f[no_lift] == "yes",
                        gunship: AircraftId::SELECTABLE
                            .into_iter()
                            .find(|id| id.pt() == f[resource])
                            .is_some_and(gunship::has_linked_guns),
                        max_takeoff_lb: f[weight].parse().unwrap(),
                        g_limit_max: f[g].parse().unwrap(),
                        top_speed_kt: f[speed].parse().unwrap(),
                    },
                }
            })
            .collect()
    }

    fn row(rows: &[Row], pt: &str) -> StrikeClass {
        let r = rows
            .iter()
            .find(|r| r.resource == pt)
            .unwrap_or_else(|| panic!("{pt} in the catalog"));
        StrikeClass::of_with(&r.record, &[])
    }

    /// The imported roster of section 4.13 of the plan and "The current
    /// import" of the spec, one line per identity.
    #[test]
    fn the_imported_roster_has_the_documented_classes() {
        let rows = catalog();
        let expected: &[(AircraftId, StrikeClass)] = &[
            (AircraftId::F18, Fighter),
            (AircraftId::Rafale, Fighter),
            (AircraftId::F14, Fighter),
            (AircraftId::A4E, Attack),
            (AircraftId::X31, Fighter),
            (AircraftId::Mig29, Fighter),
            (AircraftId::Su27, Fighter),
            (AircraftId::Mig21, Fighter),
            (AircraftId::Su25, Attack),
            (AircraftId::Mig23, Fighter),
            (AircraftId::Su35, Fighter),
            (AircraftId::F22, Fighter),
            (AircraftId::F22n, Fighter),
            // The F/A-XX borrows the F-22N record.
            (AircraftId::Faxx, Fighter),
            (AircraftId::C130, Heavy),
            (AircraftId::Ac130, Gunship),
            (AircraftId::E3, Heavy),
            (AircraftId::Il76, Heavy),
            (AircraftId::E2, Heavy),
            (AircraftId::Av8, Attack),
            (AircraftId::Yak141, Fighter),
            (AircraftId::V22, Helicopter),
            (AircraftId::Ah64, Helicopter),
            (AircraftId::Mi24, Helicopter),
            (AircraftId::Ch47, Helicopter),
            (AircraftId::Mig17, Attack),
            (AircraftId::F4B, Fighter),
            (AircraftId::F4J, Fighter),
            (AircraftId::F4E, Fighter),
            (AircraftId::F4G, Fighter),
            (AircraftId::A7, Attack),
            (AircraftId::F15, Fighter),
            (AircraftId::F16C, Fighter),
            (AircraftId::F104, Fighter),
            (AircraftId::A10, Attack),
            (AircraftId::B747, Heavy),
            (AircraftId::A310, Heavy),
        ];
        assert_eq!(expected.len(), AircraftId::SELECTABLE.len());
        for (id, class) in expected {
            assert_eq!(row(&rows, id.pt()), *class, "{id:?}");
        }
        // Table entries that share a record or are not imported.
        assert_eq!(row(&rows, "TU95.PT"), Heavy);
        assert_eq!(row(&rows, "B52.PT"), Heavy);
        assert_eq!(row(&rows, "B1.PT"), Heavy);
        assert_eq!(row(&rows, "F117.PT"), Attack);
        assert_eq!(row(&rows, "SU24.PT"), Fighter);
        assert_eq!(row(&rows, "F111.PT"), Fighter);
    }

    /// The census: over the 145 retail records the rule agrees with the AI
    /// script each record binds for 142, and the three that differ are the
    /// ones the spec lists.
    #[test]
    fn the_rule_agrees_with_retails_script_binding_for_142_of_145() {
        let rows = catalog();
        assert_eq!(rows.len(), 145);
        let agrees = |script: &str, class: StrikeClass| match script {
            // The fighter script, and the two novelty fighter scripts.
            "f.BI" | "moth.BI" => matches!(class, Fighter | Attack),
            // The F-117's own level-pass script: an attack jet by the rule.
            "f117.BI" => matches!(class, Fighter | Attack),
            "b.BI" | "large.BI" | "liner.BI" => class == Heavy,
            "h.BI" => class == Helicopter,
            "ac130.BI" => class == Gunship,
            other => panic!("unmapped script {other}"),
        };
        let mut differ: Vec<&str> = rows
            .iter()
            .filter(|r| !agrees(&r.script, r.record.rule()))
            .map(|r| r.resource.as_str())
            .collect();
        differ.sort_unstable();
        assert_eq!(differ, ["AURORA.PT", "B1.PT", "BLIMP.PT"]);
        // The class counts of the spec: the fighter script splits 79 and 19
        // (the B-1B is the 99th record on it, a Heavy).
        let count = |script: &str, class: StrikeClass| {
            rows.iter()
                .filter(|r| r.script == script && r.record.rule() == class)
                .count()
        };
        assert_eq!(count("f.BI", Fighter), 79);
        assert_eq!(count("f.BI", Attack), 19);
        assert_eq!(count("f.BI", Heavy), 1);
        // The three differences, by their numbers.
        assert_eq!(row(&rows, "B1.PT"), Heavy);
        assert_eq!(row(&rows, "AURORA.PT"), Fighter);
        assert_eq!(row(&rows, "BLIMP.PT"), Helicopter);
    }

    fn base() -> Record {
        Record {
            resource: "TEST.PT".into(),
            no_lift: false,
            gunship: false,
            max_takeoff_lb: 50_000.,
            g_limit_max: 9,
            top_speed_kt: 1_000.,
        }
    }

    #[test]
    fn the_thresholds_and_the_order_are_as_specified() {
        let r = base();
        assert_eq!(r.rule(), Fighter);
        // Attack at 700 kt or less.
        assert_eq!(
            Record {
                top_speed_kt: 700.,
                ..base()
            }
            .rule(),
            Attack
        );
        assert_eq!(
            Record {
                top_speed_kt: 701.,
                ..base()
            }
            .rule(),
            Fighter
        );
        // Heavy at 150,000 lb or more, or a G limit of 3 or less.
        assert_eq!(
            Record {
                max_takeoff_lb: 149_999.,
                ..base()
            }
            .rule(),
            Fighter
        );
        assert_eq!(
            Record {
                max_takeoff_lb: 150_000.,
                ..base()
            }
            .rule(),
            Heavy
        );
        assert_eq!(
            Record {
                g_limit_max: 4,
                ..base()
            }
            .rule(),
            Fighter
        );
        assert_eq!(
            Record {
                g_limit_max: 3,
                ..base()
            }
            .rule(),
            Heavy
        );
        // Heavy beats Attack (a slow, heavy bomber); lift and gunship beat
        // Heavy.
        let slow = Record {
            top_speed_kt: 400.,
            max_takeoff_lb: 400_000.,
            ..base()
        };
        assert_eq!(slow.rule(), Heavy);
        assert_eq!(
            Record {
                gunship: true,
                ..slow.clone()
            }
            .rule(),
            Gunship
        );
        assert_eq!(
            Record {
                no_lift: true,
                gunship: true,
                ..slow
            }
            .rule(),
            Helicopter
        );
    }

    #[test]
    fn a_new_import_needs_no_code_and_an_override_is_the_escape_hatch() {
        // An aircraft the table has never seen: classed by its numbers.
        let new = Record {
            resource: "NEWBOMBR.PT".into(),
            max_takeoff_lb: 300_000.,
            ..base()
        };
        assert_eq!(StrikeClass::of(&new), Heavy);
        // An override names a resource and wins over the rule, whatever the
        // case; other aircraft are untouched.
        let table = [("newbombr.pt", Attack)];
        assert_eq!(StrikeClass::of_with(&new, &table), Attack);
        assert_eq!(StrikeClass::of_with(&base(), &table), Fighter);
        // Shipped overrides touch no retail aircraft: every record's class is
        // the rule's.
        for r in catalog() {
            assert_eq!(
                StrikeClass::of(&r.record),
                r.record.rule(),
                "{}",
                r.resource
            );
        }
    }

    fn token(kind: &str, value: i64) -> Token {
        Token {
            kind: kind.into(),
            value: value.to_string(),
            scaled: false,
        }
    }

    /// A synthetic record, read through [`Record::from_aircraft`]: the
    /// weight, the G row, the 1 G envelope's fastest point (ft/s, the 3
    /// rows beyond it do not count) and the flag word.
    fn aircraft(id: AircraftId, weight: i64, g_max: i64, fps: f64, flags: i64) -> Aircraft {
        let fields: BTreeMap<String, Token> = [
            ("maxTakeoffWeight", token("dword", weight)),
            ("envMax", token("word", g_max)),
            ("flags", token("word", flags)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        Aircraft {
            id,
            name: "Synthetic".into(),
            shape: "TEST.SH".into(),
            fields,
            object: BTreeMap::new(),
            hardpoints: vec![],
            sounds: BTreeMap::new(),
            envelopes: [0, 1, 4]
                .into_iter()
                .map(|g| Envelope {
                    g,
                    // Only the 1 G row is the top speed; the other rows are
                    // faster on purpose.
                    points: vec![[100., 0.], [if g == 1 { fps } else { fps * 2. }, 30_000.]],
                })
                .collect(),
        }
    }

    #[test]
    fn the_record_is_read_from_the_imported_aircraft() {
        // 700 kt is 1,181.5 ft/s: the 1 G row decides, to the whole knot.
        let a = aircraft(AircraftId::A10, 50_000, 6, 700. * 1.6878098571 + 0.4, 0);
        let r = Record::from_aircraft(&a);
        assert_eq!(r.resource, "A10.PT");
        assert_eq!(
            (
                r.max_takeoff_lb,
                r.g_limit_max,
                r.top_speed_kt,
                r.no_lift,
                r.gunship
            ),
            (50_000., 6, 700., false, false)
        );
        assert_eq!(StrikeClass::of_aircraft(&a), Attack);
        assert_eq!(
            StrikeClass::of_aircraft(&aircraft(AircraftId::B747, 800_000, 4, 830., 0)),
            Heavy
        );
        // Flag bit 8 is the "no lift" mark; the other flag bits are not.
        let heli = aircraft(AircraftId::Ah64, 23_810, 4, 220., 0x18);
        assert!(Record::from_aircraft(&heli).no_lift);
        assert_eq!(StrikeClass::of_aircraft(&heli), Helicopter);
        assert!(!Record::from_aircraft(&aircraft(AircraftId::F18, 49_224, 9, 1700., 0x17)).no_lift);
        // The AC-130 is a gunship before its weight is looked at.
        assert_eq!(
            StrikeClass::of_aircraft(&aircraft(AircraftId::Ac130, 155_000, 3, 570., 0)),
            Gunship
        );
    }

    /// Run with `TORE_DATA_DIR` naming a data folder with an import: every
    /// aircraft record in the pack gives the class the catalog's numbers
    /// give, so the record reader and the committed catalog agree.
    #[test]
    #[ignore = "needs an imported data profile (TORE_DATA_DIR)"]
    fn real_data_records_class_as_the_catalog_does() {
        use std::io::Read;
        let directory = std::env::var_os("TORE_DATA_DIR").expect("TORE_DATA_DIR names an import");
        let mut packs: Vec<_> = std::fs::read_dir(&directory)
            .expect("the data folder")
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.extension().is_some_and(|ext| ext == "pack")
                    && path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("menu-"))
            })
            .collect();
        packs.sort();
        let mut bytes = Vec::new();
        std::fs::File::open(packs.pop().expect("an import pack"))
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .expect("the pack reads");
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let rows = catalog();
        let (count, mut at) = (word(12), 16);
        let mut checked = 0;
        for _ in 0..count {
            let len = u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
            let name = String::from_utf8(bytes[at + 2..at + 2 + len].to_vec()).unwrap();
            let size = word(at + 2 + len);
            let start = at + 6 + len;
            if name.ends_with(".PT")
                && let Ok(a) = Aircraft::parse(&bytes[start..start + size])
            {
                let r = Record::from_aircraft(&a);
                let catalog = rows.iter().find(|row| row.resource == r.resource).unwrap();
                assert_eq!(
                    r.rule(),
                    StrikeClass::of_with(&catalog.record, &[]),
                    "{name}"
                );
                assert_eq!(
                    (r.no_lift, r.max_takeoff_lb, r.g_limit_max, r.top_speed_kt),
                    (
                        catalog.record.no_lift,
                        catalog.record.max_takeoff_lb,
                        catalog.record.g_limit_max,
                        catalog.record.top_speed_kt
                    ),
                    "{name}"
                );
                checked += 1;
            }
            at = start + size;
        }
        assert!(checked >= 30, "only {checked} aircraft records found");
    }
}
