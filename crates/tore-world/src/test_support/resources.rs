//! A synthetic import: the named resources a Quick Mission in one theater with
//! F/A-18D aircraft on both sides is built from. Every value is generated
//! here; no retail data is copied. The layouts come from the format readers'
//! own field tables (`aircraft_schema.rs`), included so the two cannot drift.
use std::collections::BTreeMap;

#[path = "../../../tore-formats/src/aircraft_schema.rs"]
#[allow(dead_code)]
mod schema;

/// The theater the synthetic import holds.
pub const THEATER: &str = "UKR";

/// One line per field of `layout`, in order. `value` gives a field's number;
/// the named pointers point at a block of the same name, other pointers are
/// null, and symbols are `symbol`.
fn fields(
    layout: &[(&str, &str)],
    pointers: &[&str],
    symbol: &str,
    value: &dyn Fn(&str) -> i64,
) -> String {
    let mut text = String::new();
    for &(kind, name) in layout {
        match kind {
            "ptr" if pointers.contains(&name) => text += &format!("ptr {name}\n"),
            "ptr" => text += "dword 0\n",
            "symbol" => text += &format!("symbol {symbol}\n"),
            _ => text += &format!("{kind} {}\n", value(name)),
        }
    }
    text
}

fn names(short: &str, long: &str, resource: &str) -> String {
    format!(":si_names\nstring \"{short}\"\nstring \"{long}\"\nstring \"{resource}\"\nend\n")
}

const HEADER: &str = "[brent's_relocatable_format]\n";

/// A sensor record: a seeker's signature channel (3 radar, 2 infrared, 0
/// visual) with generous zones.
fn sensor(resource: &str, signature: i64) -> Vec<u8> {
    let value = |name: &str| match name {
        "structType" => 10,
        "weight" => 100,
        "sig" => signature,
        "zone0.h" | "zone1.h" => 4000,
        "zone0.p" | "zone1.p" => 4000,
        "zone0.maxRange" | "zone1.maxRange" => 60_000,
        "zone0.minAlt" | "zone1.minAlt" => -2_000_000_000,
        "zone0.maxAlt" | "zone1.maxAlt" => 2_000_000_000,
        "lookDown" => 50,
        _ => 0,
    };
    let mut text = String::from(HEADER);
    text += &fields(schema::SENSOR, &["si_names"], "", &value);
    text += &names("Sensor", "Synthetic sensor", resource);
    text.into_bytes()
}

fn countermeasures(resource: &str) -> Vec<u8> {
    let value = |name: &str| match name {
        "structType" => 9,
        "weight" => 50,
        "flags[1]" => 0x10,
        "chaffLoaded" | "flaresLoaded" => 30,
        "chaffChance" | "flareChance" => 50,
        "rdChance" => 30,
        _ => 0,
    };
    let mut text = String::from(HEADER);
    text += &fields(schema::ECM, &["si_names"], "", &value);
    text += &names("ECM", "Synthetic ECM", resource);
    text.into_bytes()
}

/// A weapon record: a gun or a short-range missile.
fn weapon(resource: &str, gun: bool) -> Vec<u8> {
    weapon_with(resource, gun, 0)
}

/// A missile other than the aircraft's own that its missile station takes
/// (its flags say it may be loaded where it is not the default), for
/// loadout tests: a resource the standard load never reads.
pub fn loadable_missile(resource: &str) -> Vec<u8> {
    weapon_with(resource, false, 2)
}

fn weapon_with(resource: &str, gun: bool, extra_flags: i64) -> Vec<u8> {
    let value = |name: &str| match name {
        "structType" => {
            // The object block's and the projectile block's first fields are
            // both called structType: 7 and 10.
            0
        }
        "weight" => 20,
        "flags" => (if gun { 0x844 } else { 0x240 }) | extra_flags,
        "sig" => {
            if gun {
                0
            } else {
                2
            }
        }
        "_minSpeed" => 10,
        "_cornerSpeed" => 1000,
        "_maxSpeed" => 2000,
        "_acc" => 100,
        "_dacc" => 2,
        "initialSpeed" => 1000,
        "finalSpeed" => 500,
        "launchRetard" => 100,
        "fuelT" => 10,
        "removeT" => 20,
        "poweredTurnRate" | "unpoweredTurnRate" => 10_000,
        "performanceAt0" | "performanceAt20" => 100,
        "actualRoundsPerGame" => 2,
        "gameRoundsInBurst" | "gameRoundsInCarpetBurst" | "gameBurstT" | "projsInPod" => 1,
        "trackT" | "trackMaxG" => 1,
        "chances[0]" | "chances[1]" | "chances[2]" | "chances[3]" => 100,
        "zone0.h" | "zone1.h" | "zone0.p" | "zone1.p" => 12_000,
        "zone0.maxRange" | "zone1.maxRange" => 10_000,
        "zone0.minAlt" | "zone1.minAlt" => -2_000_000_000,
        "zone0.maxAlt" | "zone1.maxAlt" => 2_000_000_000,
        "damage[0]" | "damage[1]" | "damage[2]" | "damage[3]" | "damage[4]" => 10,
        _ => 0,
    };
    let mut text = String::from(HEADER);
    // The object block, then the projectile block. Their structTypes are 7
    // and 10 and the size of the object is 315.
    let object = |name: &str| match name {
        "structType" => 7,
        "typeSize" => 315,
        other => value(other),
    };
    let projectile = |name: &str| match name {
        "structType" => 10,
        other => value(other),
    };
    let mut block = fields(schema::OBJECT, &[], "_PROJProc", &object);
    // The object block has no name pointer to keep: the names belong to the
    // projectile block.
    text += &block;
    block = fields(schema::PROJECTILE, &["si_names"], "", &projectile);
    text += &block;
    text += &names("SYN", "Synthetic weapon", resource);
    text.into_bytes()
}

/// One hardpoint of an aircraft: where its flags, capacity and default store
/// are.
struct Hardpoint {
    flags: i64,
    store: &'static str,
    count: i64,
}

/// The aircraft record of the F/A-18D: a gun, a radar, a visual sensor, a
/// jammer and a missile, with the flight fields of `test_support::profile`.
fn aircraft(hardpoints: &[Hardpoint]) -> Vec<u8> {
    let profile = crate::test_support::profile();
    let value = |name: &str| -> i64 {
        if let Some(token) = profile.fields.get(name) {
            return token.number().map_or(0, i64::from);
        }
        match name {
            "structType" => 5,
            "typeSize" => 660,
            "weight" => 10_000,
            "hitPoints" => 100,
            "numHards" => hardpoints.len() as i64,
            "envMin" => -2,
            "envMax" => 6,
            "maxTakeoffWeight" => 15_000,
            n if n.starts_with("systemDamage[") => 0x11,
            _ => 0,
        }
    };
    let mut text = String::from(HEADER);
    let object = |name: &str| match name {
        "structType" | "typeSize" | "weight" | "hitPoints" => value(name),
        _ => value(name),
    };
    text += &fields(
        schema::OBJECT,
        &["ot_names", "shape"],
        "_PLANEProc",
        &object,
    );
    text += &fields(schema::NPC, &["hards"], "_PLANEProc", &value);
    text += &fields(schema::PLANE, &["hards", "env"], "_PLANEProc", &value);
    text += ":hards\n";
    for (index, point) in hardpoints.iter().enumerate() {
        let value = |name: &str| match name {
            "flags" => point.flags,
            "maxItems" => point.count,
            "name" => index as i64,
            _ => 0,
        };
        let pointer = format!("hp{index}");
        for &(kind, name) in schema::HARDPOINT {
            match kind {
                "ptr" => text += &format!("ptr {pointer}\n"),
                _ => text += &format!("{kind} {}\n", value(name)),
            }
        }
    }
    text += ":env\n";
    for (row, envelope) in profile.envelopes.iter().enumerate() {
        let _ = row;
        let value = |name: &str| -> i64 {
            match name {
                "gload" => i64::from(envelope.g),
                "count" => envelope.points.len() as i64,
                _ => {
                    let pick = |prefix: &str, axis: usize| -> i64 {
                        let index: usize = name[prefix.len() + 1..name.len() - 1]
                            .parse()
                            .unwrap_or(usize::MAX);
                        envelope
                            .points
                            .get(index)
                            .map_or(0, |point| point[axis] as i64)
                    };
                    if name.starts_with("speed[") {
                        pick("speed", 0)
                    } else if name.starts_with("alt[") {
                        pick("alt", 1)
                    } else {
                        0
                    }
                }
            }
        };
        text += &fields(schema::ENVELOPE, &[], "", &value);
    }
    for (index, point) in hardpoints.iter().enumerate() {
        text += &format!(":hp{index}\nstring \"{}\"\n", point.store);
    }
    text += ":ot_names\nstring \"F/A-18D\"\nstring \"Synthetic fighter\"\nstring \"F18.PT\"\n";
    text += ":shape\nstring \"F18.SH\"\nend\n";
    text.into_bytes()
}

/// A shape module: one triangle of the given extent, in the container the
/// shape reader takes (`MZ` header, `PL` signature, one `CODE` section).
fn shape(extent: i16) -> Vec<u8> {
    let mut code = vec![0x82, 0, 3, 0, 0, 0];
    for vertex in [[0, 0, 0], [extent, 0, 0], [0, extent, extent]] {
        for word in vertex {
            code.extend(i16::to_le_bytes(word));
        }
    }
    // A plain polygon of the three vertices, then the end of the shape.
    code.extend([0xfc, 0, 0, 0, 0, 3, 0, 1, 2, 0]);
    let mut module = vec![0; 256 + code.len()];
    module[..2].copy_from_slice(b"MZ");
    module[60..64].copy_from_slice(&64u32.to_le_bytes());
    module[64..68].copy_from_slice(b"PL\0\0");
    module[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
    module[70..72].copy_from_slice(&1u16.to_le_bytes());
    module[84..86].copy_from_slice(&32u16.to_le_bytes());
    module[120..124].copy_from_slice(b"CODE");
    for (offset, value) in [(8, code.len()), (12, 4096), (16, code.len()), (20, 256)] {
        module[120 + offset..124 + offset].copy_from_slice(&(value as u32).to_le_bytes());
    }
    module[256..].copy_from_slice(&code);
    module
}

/// A terrain grid of `tiles` by `tiles` tiles of 32 cells, every cell sea
/// level.
fn grid(tiles: usize) -> Vec<u8> {
    let cells_per_tile = 32usize;
    let cols = tiles * cells_per_tile;
    let fine = 149usize;
    let coarse = fine + cols * cols * 3;
    let mut data = vec![0u8; coarse + tiles * tiles * 3];
    data[..4].copy_from_slice(b"BIT2");
    data[4..4 + 9].copy_from_slice(b"Synthetic");
    data[84..84 + 6].copy_from_slice(b"UKR.T2");
    let mut put = |at: usize, value: usize| {
        data[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
    };
    put(0x79, cells_per_tile);
    put(0x7d, tiles);
    put(0x81, tiles);
    put(0x85, coarse);
    put(0x89, cols);
    put(0x8d, cols);
    put(0x91, fine);
    // Cells are three bytes: colour, class and elevation; land at 200 feet
    // above sea level everywhere.
    for cell in data[fine..coarse].chunks_exact_mut(3) {
        cell.copy_from_slice(&[100, 2, 1]);
    }
    data
}

/// The resources of the synthetic import, ready to build a mission from.
pub fn resources() -> BTreeMap<String, Vec<u8>> {
    let mut resources = BTreeMap::new();
    let mut add = |name: &str, bytes: Vec<u8>| {
        resources.insert(name.to_owned(), bytes);
    };
    add(
        "UKR.MM",
        b"textFormat\nmap UKR.T2\nlayer CLEAR.LAY 0\ntime 12 0\n".to_vec(),
    );
    add("UKR.T2", grid(4));
    for choice in tore_sim::environment::CONDITIONS {
        add(
            &format!("{}.LAY", choice.layer),
            tore_formats::weather::synthetic_module(24),
        );
    }
    // The aircraft's shape, and the two halves its damaged look breaks into.
    add("F18.SH", shape(100));
    for (name, extent) in [("A", 60), ("B", 30), ("C", 60), ("D", 30)] {
        add(&format!("F18_{name}.SH"), shape(extent));
    }
    add("M61.JT", weapon("M61.JT", true));
    add("AIM9M.JT", weapon("AIM9M.JT", false));
    add("F18R.SEE", sensor("F18R.SEE", 3));
    add("F18V.SEE", sensor("F18V.SEE", 0));
    add("F18.ECM", countermeasures("F18.ECM"));
    add(
        "F18.PT",
        aircraft(&[
            Hardpoint {
                flags: 8,
                store: "M61.JT",
                count: 500,
            },
            Hardpoint {
                flags: 8,
                store: "F18R.SEE",
                count: 1,
            },
            Hardpoint {
                flags: 8,
                store: "F18V.SEE",
                count: 1,
            },
            Hardpoint {
                flags: 8,
                store: "F18.ECM",
                count: 1,
            },
            Hardpoint {
                flags: 0x80,
                store: "AIM9M.JT",
                count: 2,
            },
        ]),
    );
    resources
}
