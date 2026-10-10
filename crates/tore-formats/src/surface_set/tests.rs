//! Synthetic libraries only: no retail bytes. The retail cross-check is
//! `tore-import`'s ignored `surface_data` test.
use super::*;
use crate::{aircraft::schema, surface_unit::tests::fixture};
use std::collections::BTreeMap;

type Pack = BTreeMap<String, Vec<u8>>;

/// An OT or PT text: the OBJECT prefix with a shape, and extra strings that
/// only the reference scan sees.
fn object_text(resource: &str, shape: Option<&str>, extras: &[&str]) -> Vec<u8> {
    let mut root = String::from("[brent's_relocatable_format]\n");
    for (kind, name) in schema::OBJECT {
        let value = match *name {
            "ot_names" => "ot_names",
            "shape" if shape.is_some() => "shape",
            "hitPoints" => "100",
            "utilProc" => "_OBJProc",
            _ => "0",
        };
        let output_kind = if *kind == "ptr" && value == "0" {
            "dword"
        } else {
            kind
        };
        root.push_str(&format!("{output_kind} {value}\n"));
    }
    root.push_str(&format!(
        ":ot_names\nstring \"Thing\"\nstring \"Thing\"\nstring \"{resource}\"\n"
    ));
    if let Some(shape) = shape {
        root.push_str(&format!(":shape\nstring \"{shape}\"\n"));
    }
    for (i, extra) in extras.iter().enumerate() {
        root.push_str(&format!(":extra{i}\nstring \"{extra}\"\n"));
    }
    root.push_str("end\n");
    root.into_bytes()
}

fn shape_bytes(textures: &[&str]) -> Vec<u8> {
    let mut bytes = vec![0u8; 16];
    for texture in textures {
        bytes.extend_from_slice(texture.as_bytes());
        bytes.push(0);
    }
    bytes
}

fn unit(name: &str, shape: &str, stores: &[&str], extra: &[(&str, &str)]) -> Vec<u8> {
    let mounts: Vec<_> = stores
        .iter()
        .map(|store| (0, 0, 0, 0, Some(*store), 1))
        .collect();
    fixture(["Test", "Test", name], Some(shape), extra, &[], &mounts).into_bytes()
}

fn template(types: &[&str]) -> Vec<u8> {
    let mut text = String::new();
    for kind in types {
        text.push_str(&format!(
            "obj\r\n\ttype {kind}\r\n\tpos 1 0 2\r\n\tangle 0 0 0\r\n\tnationality3 151\r\n\tflags $80\r\n\tspeed 0\r\n\talias -1\r\n\t.\r\n"
        ));
    }
    text.into_bytes()
}

/// A library with every table template (empty but `~QUCOL.M`), a stub unit for
/// every name in the equipment lists, and the records the tests add.
fn library() -> Pack {
    let mut pack = Pack::new();
    for stem in tables::TEMPLATES
        .iter()
        .flat_map(|list| list.iter())
        .chain(tables::UNREFERENCED.iter())
    {
        pack.insert(format!("~{stem}.M"), template(&[]));
    }
    let mut listed: BTreeSet<&str> = tables::NIGHT_AAA.iter().copied().collect();
    for lists in tables::LISTS.iter() {
        for group in lists.groups {
            listed.extend(group.iter().copied());
        }
    }
    for name in listed {
        let stem = name.trim_end_matches(".NT");
        pack.insert(name.to_owned(), unit(name, &format!("{stem}.SH"), &[], &[]));
        pack.insert(
            format!("{stem}.SH"),
            shape_bytes(&[&format!("_{stem}.PIC")]),
        );
        pack.insert(format!("_{stem}.PIC"), vec![1]);
    }
    pack.insert(
        DESTROYED_VEHICLE_OBJECT.to_owned(),
        object_text(DESTROYED_VEHICLE_OBJECT, Some("DEST.SH"), &[]),
    );
    pack.insert("DEST.SH".into(), shape_bytes(&["_DEST.PIC"]));
    pack.insert("_DEST.PIC".into(), vec![1]);
    pack
}

#[test]
fn a_complete_library_has_nothing_missing_and_every_template() {
    let pack = library();
    let selection = select(&pack).unwrap();
    assert!(selection.missing.is_empty(), "{:?}", selection.missing);
    assert!(selection.unread.is_empty(), "{:?}", selection.unread);
    assert_eq!(selection.templates.len(), 129);
    assert!(selection.templates.contains("~QUCOL.M"));
    assert!(selection.resources.contains("DEST.OT"));
    assert!(selection.resources.contains("_DEST.PIC"));
    // Every equipment list name is kept with its shape and picture.
    for lists in tables::LISTS.iter() {
        for group in lists.groups {
            for name in group.iter() {
                let stem = name.trim_end_matches(".NT");
                for kept in [
                    name.to_string(),
                    format!("{stem}.SH"),
                    format!("_{stem}.PIC"),
                ] {
                    assert!(selection.resources.contains(&kept), "{kept}");
                }
            }
        }
    }
}

#[test]
fn a_missing_template_unit_or_shape_is_reported_with_who_needs_it() {
    let mut pack = library();
    pack.remove("~QTSAM.M");
    pack.remove("TRUCK.SH");
    pack.insert(
        "~QUCOL.M".into(),
        template(&["TRUCK.NT", "GONE.NT", "NOEXT", "STRIPE.XX"]),
    );
    pack.insert(
        "TRUCK.NT".into(),
        unit("TRUCK.NT", "TRUCK.SH", &["TRUCK.JT"], &[]),
    );
    let selection = select(&pack).unwrap();
    let missing: Vec<(&str, &str)> = selection
        .missing
        .iter()
        .map(|m| (m.name.as_str(), m.needed_by.as_str()))
        .collect();
    assert!(missing.contains(&("~QTSAM.M", "the executable's template table")));
    assert!(missing.contains(&("GONE.NT", "~QUCOL.M")));
    assert!(missing.contains(&("TRUCK.SH", "TRUCK.NT")));
    assert!(missing.contains(&("TRUCK.JT", "TRUCK.NT")));
    assert!(missing.iter().any(|(n, _)| *n == "NOEXT"));
    assert!(
        missing
            .iter()
            .any(|(n, w)| *n == "STRIPE.XX" && w.contains("not a surface record"))
    );
}

#[test]
fn a_bare_type_name_resolves_to_the_record_the_library_has() {
    let mut pack = library();
    pack.insert("~QUCOL.M".into(), template(&["bnk5", "mig29"]));
    pack.insert(
        "BNK5.OT".into(),
        object_text("BNK5.OT", Some("BNK5.SH"), &[]),
    );
    pack.insert("BNK5.SH".into(), shape_bytes(&["_BNK5.PIC"]));
    pack.insert("_BNK5.PIC".into(), vec![1]);
    pack.insert(
        "MIG29.PT".into(),
        object_text("MIG29.PT", Some("MIG29.SH"), &[]),
    );
    pack.insert("MIG29.SH".into(), shape_bytes(&[]));
    let selection = select(&pack).unwrap();
    assert!(selection.missing.is_empty(), "{:?}", selection.missing);
    assert!(selection.types[&Family::Object].contains("BNK5.OT"));
    assert!(selection.types[&Family::Aircraft].contains("MIG29.PT"));
}

#[test]
fn a_units_weapons_sensor_script_sounds_and_looks_follow_it() {
    let mut pack = library();
    pack.insert("~QUCOL.M".into(), template(&["KRIVAK.NT", "GCI.NT"]));
    pack.insert(
        "KRIVAK.NT".into(),
        unit(
            "KRIVAK.NT",
            "KRIV.SH",
            &["SAN4.JT"],
            &[("obj_class", "$2000")],
        ),
    );
    pack.insert("KRIV.SH".into(), shape_bytes(&["_KRIV.PIC"]));
    pack.insert("KRIV_A.SH".into(), shape_bytes(&["_KRIV_A.PIC"]));
    pack.insert("_KRIV.PIC".into(), vec![1]);
    pack.insert("_KRIV_A.PIC".into(), vec![1]);
    pack.insert("SAN4.JT".into(), b"SAN4M.SH &SAN4.11K".to_vec());
    pack.insert("SAN4M.SH".into(), shape_bytes(&["_SAN4M.PIC"]));
    pack.insert("_SAN4M.PIC".into(), vec![1]);
    pack.insert("&SAN4.11K".into(), vec![1]);
    pack.insert(
        "GCI.NT".into(),
        unit("GCI.NT", "KING.SH", &["GCIR.SEE"], &[("obj_class", "$100")]),
    );
    pack.insert("KING.SH".into(), shape_bytes(&["_KING.PIC"]));
    pack.insert("_KING.PIC".into(), vec![1]);
    pack.insert("GCIR.SEE".into(), b"GCIR".to_vec());
    let selection = select(&pack).unwrap();
    assert!(selection.missing.is_empty(), "{:?}", selection.missing);
    for kept in [
        "KRIV.SH",
        "KRIV_A.SH",
        "_KRIV.PIC",
        "_KRIV_A.PIC",
        "SAN4.JT",
        "SAN4M.SH",
        "_SAN4M.PIC",
        "&SAN4.11K",
        "GCIR.SEE",
        "KING.SH",
        "_KING.PIC",
    ] {
        assert!(selection.resources.contains(kept), "{kept}");
    }
}

#[test]
fn a_carrier_keeps_its_tower_and_far_shape_and_an_object_its_damaged_variant() {
    let mut pack = library();
    pack.insert("~QUCOL.M".into(), template(&["NIMZ.NT", "BNK5.OT"]));
    pack.insert(
        "NIMZ.NT".into(),
        unit(
            "NIMZ.NT",
            "NIMZ.SH",
            &[],
            &[("obj_class", "$2000"), ("utilProc", "_CARRIERProc")],
        ),
    );
    for shape in ["NIMZ", "NIMZ_A", "NIMZT", "XNIMZ"] {
        pack.insert(
            format!("{shape}.SH"),
            shape_bytes(&[&format!("_{shape}.PIC")]),
        );
        pack.insert(format!("_{shape}.PIC"), vec![1]);
    }
    pack.insert("_NIMZT_A.PIC".into(), vec![1]);
    pack.get_mut("NIMZT.SH").unwrap().extend(b"_NIMZT_A.PIC\0");
    pack.insert(
        "~NIMZT.OT".into(),
        object_text("~NIMZT.OT", Some("NIMZT.SH"), &[]),
    );
    pack.insert(
        "BNK5.OT".into(),
        object_text("BNK5.OT", Some("BNK5.SH"), &[]),
    );
    pack.insert(
        "~BNK5.OT".into(),
        object_text("~BNK5.OT", Some("DBK5.SH"), &[]),
    );
    for shape in ["BNK5", "DBK5"] {
        pack.insert(
            format!("{shape}.SH"),
            shape_bytes(&[&format!("_{shape}.PIC")]),
        );
        pack.insert(format!("_{shape}.PIC"), vec![1]);
    }
    let selection = select(&pack).unwrap();
    assert!(selection.missing.is_empty(), "{:?}", selection.missing);
    for kept in [
        "NIMZ.SH",
        "NIMZ_A.SH",
        "_NIMZ_A.PIC",
        "NIMZT.SH",
        "~NIMZT.OT",
        "_NIMZT_A.PIC",
        "XNIMZ.SH",
        "_XNIMZ.PIC",
        "~BNK5.OT",
        "DBK5.SH",
        "_DBK5.PIC",
    ] {
        assert!(selection.resources.contains(kept), "{kept}");
    }
}

#[test]
fn a_parked_aircraft_keeps_its_looks_and_nothing_of_its_flight() {
    let mut pack = library();
    pack.insert("~QUCOL.M".into(), template(&["SU35.PT"]));
    pack.insert(
        "SU35.PT".into(),
        object_text(
            "SU35.PT",
            Some("SU35.SH"),
            &[
                "AA11B.JT",
                "SU27.ECM",
                "VIS340.SEE",
                "&JET1A.11K",
                "F.BI",
                "SU33.HUD",
                "SU35_S.SH",
            ],
        ),
    );
    for shape in ["SU35", "SU35_S", "SU35_A", "SU35_B", "SU35_C", "SU35_D"] {
        pack.insert(
            format!("{shape}.SH"),
            shape_bytes(&[&format!("_{shape}.PIC")]),
        );
        pack.insert(format!("_{shape}.PIC"), vec![1]);
    }
    for flight in [
        "AA11B.JT",
        "SU27.ECM",
        "VIS340.SEE",
        "&JET1A.11K",
        "F.BI",
        "SU33.HUD",
    ] {
        pack.insert(flight.into(), vec![1]);
    }
    let selection = select(&pack).unwrap();
    for kept in [
        "SU35.PT",
        "SU35.SH",
        "SU35_S.SH",
        "SU35_A.SH",
        "SU35_D.SH",
        "_SU35.PIC",
        "_SU35_C.PIC",
    ] {
        assert!(selection.resources.contains(kept), "{kept}");
    }
    for left_out in [
        "AA11B.JT",
        "SU27.ECM",
        "VIS340.SEE",
        "&JET1A.11K",
        "F.BI",
        "SU33.HUD",
    ] {
        assert!(!selection.resources.contains(left_out), "{left_out}");
    }
    assert!(selection.types[&Family::Aircraft].contains("SU35.PT"));
}

#[test]
fn every_unit_record_is_kept_whether_a_template_names_it_or_not() {
    let mut pack = library();
    pack.insert(
        "EJECT.NT".into(),
        unit("EJECT.NT", "EJECT.SH", &[], &[("obj_class", "$40")]),
    );
    pack.insert("EJECT.SH".into(), shape_bytes(&["_EJECT.PIC"]));
    pack.insert("_EJECT.PIC".into(), vec![1]);
    let selection = select(&pack).unwrap();
    assert!(selection.types[&Family::Unit].contains("EJECT.NT"));
    assert!(selection.resources.contains("_EJECT.PIC"));
}

#[test]
fn a_template_the_table_does_not_list_is_kept_and_a_bad_record_is_noted() {
    let mut pack = library();
    pack.insert("~QZZZZ.M".into(), template(&["T72.NT"]));
    pack.insert("T72.NT".into(), unit("T72.NT", "T72.SH", &[], &[]));
    pack.insert("BROKEN.NT".into(), b"not a record".to_vec());
    let selection = select(&pack).unwrap();
    assert!(selection.templates.contains("~QZZZZ.M"));
    assert_eq!(selection.templates.len(), 130);
    assert!(selection.unread.contains_key("BROKEN.NT"));
    assert!(selection.resources.contains("BROKEN.NT"));
}

#[test]
fn selecting_over_the_selection_is_stable() {
    // The pack is the selection, so asking the pack again finds the same set
    // and nothing missing: the completeness check relies on it.
    let pack = library();
    let first = select(&pack).unwrap();
    let kept: Pack = pack
        .into_iter()
        .filter(|(name, _)| first.resources.contains(name))
        .collect();
    let second = select(&kept).unwrap();
    assert_eq!(first.resources, second.resources);
    assert!(second.missing.is_empty());
}

#[test]
fn shape_textures_names_nothing_for_bytes_that_are_not_a_shape() {
    assert!(shape_textures(b"not a shape").is_empty());
}
