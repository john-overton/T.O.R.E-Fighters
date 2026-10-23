//! Bounded reader for the OBJECT block of an aircraft record (PT), the part a
//! parked aircraft needs: its names, shapes, class word, hit points, debris
//! positions, explosion and crater. Nothing from the flight model, the NPC
//! block or the hardpoints is read, so any PT parses the same way, the
//! aircraft the game flies and the 28 types the Quick Mission templates only
//! park alike. There is no identity whitelist and no aliasing: the record
//! names itself (`MIG21F.PT` is not `MIG21.PT`).
//!
//! Behaviour: docs/spec/surface-defenses.md, "Parked aircraft". Layout: the
//! OBJECT block is the shared prefix of every PT, NT and OT
//! (docs/formats/surface-units.md).
use crate::{
    Result,
    aircraft::{Brf, fields, schema},
    invalid,
    shape::Shape,
    surface_unit::{optional_resource, single_block},
};
use std::collections::{BTreeMap, BTreeSet};

/// `structType` of an aircraft record.
const PT_STRUCT_TYPE: i32 = 5;

/// One aircraft type as a parked target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParkedType {
    /// Archive resource, as the record names itself: `MIG21F.PT`.
    pub resource: String,
    /// The record's first identity string, the short name: `MiG-21F`.
    pub short_name: String,
    /// The second identity string, the long name.
    pub name: String,
    /// The main shape, `M21F.SH`.
    pub shape: String,
    pub shadow_shape: Option<String>,
    /// `obj_class`: 0x8000 fighter, 0x4000 bomber (the debrief kill rows).
    pub class: u16,
    /// OBJECT `flags`.
    pub object_flags: u32,
    pub hit_points: i32,
    /// `sigs[0..5]`; `sigs[3]` is radar, `sigs[2]` infrared.
    pub signatures: [i32; 5],
    /// `dmgDebrisPos`: where pieces leave a damaged aircraft, shape units
    /// (right, up, forward as the record writes x, y, z).
    pub debris_damaged: [i32; 3],
    /// `dstDebrisPos`: where pieces leave a destroyed one.
    pub debris_destroyed: [i32; 3],
    /// `expType` and `craterSize`.
    pub explosion: u8,
    pub crater: u8,
}

impl ParkedType {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let brf = Brf::parse(bytes)?;
        let root = brf.block("")?;
        let prefix = root
            .get(..schema::OBJECT.len())
            .ok_or_else(|| invalid("aircraft record shorter than its OBJECT block"))?;
        let object = fields(prefix, schema::OBJECT)?;
        let number = |key: &str| -> Result<i32> {
            let field = &object[key];
            if field.scaled {
                return Err(invalid("scaled aircraft OBJECT statistic unsupported"));
            }
            field.number()
        };
        if number("structType")? != PT_STRUCT_TYPE {
            return Err(invalid("aircraft record structType must be 5"));
        }
        let names = single_block(&brf, &object["ot_names"], 3, "identity")?;
        let resource = names[2].to_ascii_uppercase();
        if !resource.ends_with(".PT") {
            return Err(invalid(
                "aircraft record identity must name its PT resource",
            ));
        }
        let shape = optional_resource(&brf, &object["shape"])?
            .ok_or_else(|| invalid("aircraft record names no main shape"))?;
        let shadow_shape = optional_resource(&brf, &object["shadowShape"])?;
        let mut signatures = [0; 5];
        for (i, slot) in signatures.iter_mut().enumerate() {
            *slot = number(&format!("sigs[{i}]"))?;
        }
        let hit_points = number("hitPoints")?;
        if hit_points <= 0 || signatures.iter().any(|v| *v < 0) {
            return Err(invalid("aircraft record needs positive hit points"));
        }
        let vector = |prefix: &str| -> Result<[i32; 3]> {
            Ok([
                number(&format!("{prefix}.x"))?,
                number(&format!("{prefix}.y"))?,
                number(&format!("{prefix}.z"))?,
            ])
        };
        let byte = |key: &str| -> Result<u8> {
            u8::try_from(number(key)?).map_err(|_| invalid("aircraft OBJECT byte out of range"))
        };
        Ok(Self {
            resource,
            short_name: names[0].clone(),
            name: names[1].clone(),
            shape,
            shadow_shape,
            class: number("obj_class")? as u16,
            object_flags: number("flags")? as u32,
            hit_points,
            signatures,
            debris_damaged: vector("dmgDebrisPos")?,
            debris_destroyed: vector("dstDebrisPos")?,
            explosion: byte("expType")?,
            crater: byte("craterSize")?,
        })
    }

    /// The radar signature, `sigs[3]`.
    pub fn radar_signature(&self) -> i32 {
        self.signatures[3]
    }
    /// The infrared signature, `sigs[2]`.
    pub fn infrared_signature(&self) -> i32 {
        self.signatures[2]
    }
}

/// The shape's landing gear: the state word that draws it, and the shape
/// drawn with the gear down.
pub struct Gear {
    /// `None` when the shape always shows its gear (skids, fixed wheels) or
    /// has no gear branch.
    pub word: Option<usize>,
    /// The shape with the gear word set to 1 and every other word 0.
    pub down: Shape,
}

/// Finds a shape's gear. An aircraft shape draws its devices (afterburner
/// flame, airbrake, landing gear, hook, flaps) as branches the instance's
/// state words switch on; with every word 0 the gear is up. The gear is the
/// word whose branch, switched on alone, adds the faces that reach lowest
/// (the wheels), at least as low as the rest of the shape; a tie goes to the
/// branch that adds more faces, then to the lower word. A rule over the
/// shape's own geometry with no list of types (fitted); across the 38 types
/// the Quick Mission templates park it picks the word whose lowest point
/// matches the shape's recorded ground offset within two units
/// (docs/formats/objects-and-shapes.md, "Parked aircraft gear").
pub fn gear(shape_bytes: &[u8]) -> Result<Gear> {
    let neutral = Shape::with_state(shape_bytes, &BTreeMap::new())?;
    let lowest = |faces: &mut dyn Iterator<Item = &crate::shape::Face>| {
        faces
            .flat_map(|face| face.positions.iter().map(|p| p[2]))
            .fold(f32::INFINITY, f32::min)
    };
    let floor = lowest(&mut neutral.faces.iter());
    let known: BTreeSet<usize> = neutral.faces.iter().map(|face| face.address).collect();
    // (lowest point, faces added, word, shape)
    let mut best: Option<(f32, usize, usize, Shape)> = None;
    for &word in &neutral.state_words {
        let shape = Shape::with_state(shape_bytes, &BTreeMap::from([(word, 1)]))?;
        let added: Vec<_> = shape
            .faces
            .iter()
            .filter(|face| !known.contains(&face.address))
            .collect();
        if added.is_empty() {
            continue;
        }
        let low = lowest(&mut added.iter().copied());
        if low > floor {
            continue;
        }
        let better = best.as_ref().is_none_or(|(b_low, b_added, _, _)| {
            low < *b_low || (low == *b_low && added.len() > *b_added)
        });
        if better {
            best = Some((low, added.len(), word, shape));
        }
    }
    Ok(match best {
        Some((_, _, word, down)) => Gear {
            word: Some(word),
            down,
        },
        None => Gear {
            word: None,
            down: neutral,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic PT: the OBJECT block with the given values, then a few
    /// PLANE-like fields the reader must not look at.
    fn record(resource: &str, values: &[(&str, &str)]) -> Vec<u8> {
        let mut root = String::from("[brent's_relocatable_format]\n");
        for (kind, name) in schema::OBJECT {
            let given = values.iter().find(|(key, _)| key == name).map(|v| v.1);
            let (kind, value) = match (*name, given) {
                ("ot_names", _) => ("ptr", "ot_names"),
                ("shape", _) => ("ptr", "shape"),
                ("shadowShape", Some(v)) => ("ptr", v),
                (_, Some(v)) => (*kind, v),
                (_, None) if *kind == "symbol" => ("symbol", "_PLANEProc"),
                (_, None) if *kind == "ptr" => ("dword", "0"),
                (_, None) => (*kind, "0"),
            };
            root.push_str(&format!("{kind} {value}\n"));
        }
        // Flight-model words after the OBJECT block, garbage on purpose.
        root.push_str("dword $ffffffff\nword 77\n");
        root.push_str(&format!(
            ":ot_names\nstring \"MiG-21F\"\nstring \"Fishbed C\"\nstring \"{resource}\"\n:shape\nstring \"m21f.SH\"\n"
        ));
        if values.iter().any(|(k, _)| *k == "shadowShape") {
            root.push_str(":shadow\nstring \"m21f_s.SH\"\n");
        }
        root.push_str("end\n");
        root.into_bytes()
    }

    fn base() -> Vec<(&'static str, &'static str)> {
        vec![
            ("structType", "5"),
            ("obj_class", "$8000"),
            ("hitPoints", "190"),
            ("sigs[2]", "40"),
            ("sigs[3]", "55"),
            ("dmgDebrisPos.x", "-3"),
            ("dmgDebrisPos.y", "2"),
            ("dmgDebrisPos.z", "-30"),
            ("dstDebrisPos.y", "4"),
            ("expType", "30"),
            ("craterSize", "9"),
        ]
    }

    #[test]
    fn reads_the_object_block_and_nothing_of_the_flight_model() {
        let mut values = base();
        values.push(("shadowShape", "shadow"));
        let parked = ParkedType::parse(&record("MIG21F.PT", &values)).unwrap();
        assert_eq!(parked.resource, "MIG21F.PT");
        assert_eq!(parked.short_name, "MiG-21F");
        assert_eq!(parked.name, "Fishbed C");
        assert_eq!(parked.shape, "M21F.SH");
        assert_eq!(parked.shadow_shape.as_deref(), Some("M21F_S.SH"));
        assert_eq!(parked.class, 0x8000);
        assert_eq!(parked.hit_points, 190);
        assert_eq!(parked.infrared_signature(), 40);
        assert_eq!(parked.radar_signature(), 55);
        assert_eq!(parked.debris_damaged, [-3, 2, -30]);
        assert_eq!(parked.debris_destroyed, [0, 4, 0]);
        assert_eq!((parked.explosion, parked.crater), (30, 9));
    }

    #[test]
    fn the_record_names_itself_with_no_whitelist_and_no_aliasing() {
        // A type nobody flies reads, under its own name: an unknown PT is not
        // refused and not turned into a type the game knows.
        for name in ["MIG21F.PT", "RAFALEF.PT", "ZZTOP.PT"] {
            assert_eq!(
                ParkedType::parse(&record(name, &base())).unwrap().resource,
                name
            );
        }
    }

    #[test]
    fn refuses_what_is_not_an_aircraft_record() {
        let mut nt = base();
        nt[0] = ("structType", "3");
        assert!(ParkedType::parse(&record("MIG21F.PT", &nt)).is_err());
        assert!(ParkedType::parse(&record("SA6.NT", &base())).is_err());
        let mut dead = base();
        dead[2] = ("hitPoints", "0");
        assert!(ParkedType::parse(&record("MIG21F.PT", &dead)).is_err());
        assert!(ParkedType::parse(b"not a record").is_err());
    }
}

/// Over a user-owned retail install (`TORE_GAME_DIR`, or the
/// `gameassets/fighters-anthology` link):
///
/// ```text
/// cargo test -p tore-formats parked_aircraft::import_tests -- --ignored --nocapture
/// ```
#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::{
        Archive,
        quick_template::{ObjectKind, Template},
        shape::{contact_offset, object_scale},
    };
    use std::path::PathBuf;

    /// Every aircraft the templates park, with the gear word the rule finds
    /// (checked by eye on the parked-aircraft sheet).
    const GEAR: [(&str, Option<usize>); 39] = [
        ("A37.PT", Some(0x6380)),
        ("AH1.PT", None),
        ("C130.PT", Some(0x3a30)),
        ("F16E.PT", Some(0x8d8c)),
        ("F4E.PT", Some(0x5dfc)),
        ("F5EE.PT", Some(0x639c)),
        ("F5EV.PT", Some(0x614c)),
        ("J7E.PT", Some(0x4ce6)),
        ("KA50.PT", Some(0x7350)),
        ("M2000.PT", Some(0x589c)),
        ("M2000E.PT", Some(0x592c)),
        ("M25.PT", Some(0x760c)),
        ("M5.PT", Some(0x67cc)),
        ("MF1.PT", Some(0x5adc)),
        ("MI17.PT", None),
        ("MI24.PT", Some(0x7196)),
        ("MIG17F.PT", Some(0x605c)),
        ("MIG21.PT", Some(0x4a56)),
        ("MIG21F.PT", Some(0x5d0c)),
        ("MIG23.PT", Some(0x6ae6)),
        ("MIG27.PT", Some(0x390c)),
        ("MIG29.PT", Some(0x824c)),
        ("MIG29M.PT", Some(0x7f1c)),
        ("MIG29V.PT", Some(0x38b6)),
        ("MIG31.PT", Some(0x612c)),
        ("MR3.PT", Some(0x604c)),
        ("MR3E.PT", Some(0x593c)),
        ("Q5.PT", Some(0x63fc)),
        ("RAFALE.PT", Some(0x5b62)),
        ("RAFALEF.PT", Some(0x6092)),
        ("SFR.PT", Some(0x5e50)),
        ("SPE.PT", Some(0x68cc)),
        ("SU24.PT", Some(0x765c)),
        ("SU25.PT", Some(0x8396)),
        ("SU27V.PT", Some(0x41dc)),
        ("SU34.PT", Some(0x67cc)),
        ("SU35.PT", Some(0x79cc)),
        ("SU7.PT", Some(0x629c)),
        ("YAK141.PT", Some(0x6270)),
    ];

    fn game_dir() -> PathBuf {
        std::env::var_os("TORE_GAME_DIR").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../gameassets/fighters-anthology")
            },
            PathBuf::from,
        )
    }

    #[test]
    #[ignore = "needs a retail install (TORE_GAME_DIR)"]
    fn every_parked_type_reads_and_stands_on_its_gear() {
        let lib = Archive::open(game_dir().join("FA_2.LIB")).expect("FA_2.LIB");
        let mut named = BTreeSet::new();
        for name in lib
            .entries
            .keys()
            .filter(|n| n.starts_with("~Q") && n.ends_with(".M"))
        {
            let Ok(template) = Template::parse(name, &lib.read(name).unwrap()) else {
                continue;
            };
            for object in &template.objects {
                if let ObjectKind::Named(written) = &object.kind {
                    let upper = written.to_ascii_uppercase();
                    let pt = if upper.contains('.') {
                        upper
                    } else {
                        format!("{upper}.PT")
                    };
                    if pt.ends_with(".PT") && lib.entries.contains_key(&pt) {
                        named.insert(pt);
                    }
                }
            }
        }
        let expected: BTreeSet<String> = GEAR.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(named, expected);
        for (name, word) in GEAR {
            let parked = ParkedType::parse(&lib.read(name).unwrap()).unwrap();
            assert_eq!(parked.resource, name);
            assert!(matches!(parked.class, 0x8000 | 0x4000), "{name}");
            let bytes = lib.read(&parked.shape).unwrap();
            let gear = gear(&bytes).unwrap();
            assert_eq!(gear.word, word, "{name}");
            // The gear's lowest point is the shape's ground offset (F2,
            // scenery feet) within two shape units.
            let scale = object_scale(&bytes).unwrap() as f32;
            let low = gear
                .down
                .faces
                .iter()
                .flat_map(|f| &f.positions)
                .map(|p| p[2])
                .fold(f32::INFINITY, f32::min);
            let contact = f32::from(contact_offset(&bytes).unwrap().unwrap());
            assert!(
                (low * scale - contact).abs() <= 2. * scale,
                "{name}: low {low} x {scale} against {contact}"
            );
        }
    }
}
