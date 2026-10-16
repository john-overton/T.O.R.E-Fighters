//! Opinionated extra stations requested by John, 2026-10-15 and 2026-10-16.
//! Guarded additions only; source rows and their indices are preserved.
//! Contract and fitted coordinates: docs/spec/stores-on-pylons.md.
use super::{AircraftId, Hardpoint};

pub const F15_CENTER: usize = 6;
pub const F15_WING_TANKS: usize = 7;
pub const F16_WET: usize = 9;
pub const F16_TIP: usize = 8;
pub const F22_WET: usize = 8;
pub const MIG29_CENTER: usize = 8;
pub const RAFALE_CENTER: usize = 9;

/// These names have authored GAS records, generated in memory at pack load.
/// Fuel uses the game's 6.6 lb/US gal convention; shell weights are fitted by
/// scaling the 350 gallon class (248 lb), rounded to whole pounds.
pub const TANKS: [(&str, u16, i32, u16); 5] = [
    ("F610.GAS", 610, 4026, 432),
    ("F370.GAS", 370, 2442, 262),
    ("F600.GAS", 600, 3960, 425),
    ("M396.GAS", 396, 2614, 281),
    ("R528.GAS", 528, 3485, 374),
];

/// Canonical authored bytes, also used by the old combat-tape media guard.
pub fn resource(name: &str) -> Option<Vec<u8>> {
    let (_, gallons, fuel, shell) = TANKS.iter().find(|t| t.0 == name)?;
    Some(format!("[brent's_relocatable_format]\nbyte 8\nptr si_names\nword {shell}\nbyte 1\ndword {fuel}\n:si_names\nstring \"{gallons} gallon tank\"\nstring \"{gallons} Gallon External Fuel Tank\"\nstring \"{name}\"\nend\n").into_bytes())
}

/// Adds each authored tank only to a pack holding its carrier, as the
/// derived bombs need their donor: a pack without a supported carrier stays
/// exactly as imported.
pub fn extend_resources(resources: &mut std::collections::BTreeMap<String, Vec<u8>>) {
    for ((name, ..), carrier) in
        TANKS
            .iter()
            .zip(["F15.PT", "F16C.PT", "F22.PT", "MIG29.PT", "RAFALE.PT"])
    {
        if resources.contains_key(carrier) {
            resources
                .entry((*name).into())
                .or_insert_with(|| resource(name).expect("known authored tank"));
        }
    }
}

/// Index in the existing nine fuel banks. F-16 and Rafale row 9 use bank 0; their
/// fixed visual sensors cannot fuel. Configuration rejects bank collisions.
/// No serialized array or previous aircraft's fuel bank changes.
pub fn fuel_bank(hardpoint: usize) -> Option<usize> {
    match hardpoint {
        0..=8 => Some(hardpoint),
        F16_WET => Some(0),
        _ => None,
    }
}

/// Restrict the new tank identities to the requested real carriers. Existing
/// GAS types retain the game's compatibility rules, including Cheat loading.
pub fn tank_allowed(id: AircraftId, row: usize, name: &str) -> bool {
    match name {
        "F610.GAS" => id == AircraftId::F15 && matches!(row, F15_CENTER | F15_WING_TANKS),
        "F370.GAS" => id == AircraftId::F16C && row == F16_WET,
        "F600.GAS" => id == AircraftId::F22 && row == F22_WET,
        "M396.GAS" => id == AircraftId::Mig29 && row == MIG29_CENTER,
        "R528.GAS" => id == AircraftId::Rafale && row == RAFALE_CENTER,
        _ => true,
    }
}

/// Weapons an existing source row takes in normal loading beyond its own mask,
/// on aircraft whose guarded rows were appended (so the reviewed source
/// signature matched). The F-16C's wingtip launchers (stations 1 and 9,
/// LAU-129) carry the AIM-120 as well as the AIM-9 (John, 2026-10-16). The
/// source row stays as imported: its mask (0x85, infrared only) and weight
/// class (4, sized for two AIM-9s) cannot admit a 345 lb radar missile, and
/// raising either would also change the Archer and other infrared counts.
/// The count is the row's own, one a tip. Cheat loading already allows it.
pub fn extra_weapon(id: AircraftId, rows: usize, row: usize, source: &str) -> bool {
    id == AircraftId::F16C && rows > F16_WET && row == F16_TIP && source == "AIM120.JT"
}

type Signature = (u8, i32, [i32; 3], &'static str, i32, i32);

fn signature(id: AircraftId) -> &'static [Signature] {
    // Check every source row, not just the identity. Edited records keep
    // their own station definitions; repeated application is a no-op.
    match id {
        AircraftId::F15 => &[
            (1, 8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 8, [0, 0, 0], "F15R.SEE", 1, 0),
            (1, 8, [0, 0, -50], "F15.ECM", 1, 0),
            (1, 8, [5, -5, -5], "M61.JT", 940, 0),
            (1, 0x7e5, [25, -10, 0], "AIM120.JT", 4, 60),
            (4, 0x7e5, [50, -20, 0], "AIM9M.JT", 4, 14),
        ],
        AircraftId::F16C => &[
            (1, 8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 8, [0, 0, 0], "F18R.SEE", 1, 0),
            (1, 8, [0, 0, -50], "F18.ECM", 1, 0),
            (1, 8, [0, 0, 50], "PAVESPK.SEE", 1, 0),
            (2, 8, [-11, 3, 31], "M61.JT", 500, 0),
            (0, 0x601, [0, -20, 0], "F150.GAS", 1, 26),
            (4, 0x7e5, [32, -10, -7], "AGM65G.JT", 4, 27),
            (4, 0x185, [43, -10, -11], "AIM120.JT", 2, 13),
            (5, 0x85, [54, 2, -14], "AIM9M.JT", 2, 4),
        ],
        AircraftId::F22 => &[
            (1, 0x8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "F22R.SEE", 1, 0),
            (1, 0x8, [0, 0, -50], "F22.ECM", 1, 0),
            (1, 0x8, [11, 3, 50], "M61.JT", 750, 0),
            (3, 0x8, [0, 0, 0], "AV8IR.SEE", 1, 0),
            (3, 0x5e0, [-10, -25, 0], "AIM120.JT", 2, 40),
            (3, 0x5e0, [10, -25, 0], "AGM65G.JT", 2, 40),
            (3, 0x5e4, [27, -11, 0], "AIM9X.JT", 4, 8),
        ],
        AircraftId::Mig29 => &[
            (1, 0x8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "MIG29I.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "MIG29R.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "MIG29.ECM", 1, 0),
            (2, 0x8, [-5, -5, 60], "GSH301.JT", 150, 0),
            (4, 0x7e5, [20, -10, -5], "AA8.JT", 2, 20),
            (4, 0x7e5, [25, -10, -5], "AA8.JT", 2, 20),
            (4, 0x7e5, [50, -10, -5], "AA8.JT", 2, 20),
        ],
        AircraftId::Rafale => &[
            (1, 0x8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "F18R.SEE", 1, 0),
            (1, 0x8, [0, 0, -50], "F18.ECM", 1, 0),
            (2, 0x8, [-4, -5, 59], "DEFA.JT", 250, 0),
            (0, 0x601, [0, -20, 0], "AAS38.SEE", 1, 22),
            (1, 0x5e5, [10, -20, 0], "AGM65G.JT", 4, 46),
            (4, 0x5e5, [20, -20, 8], "MICA.JT", 2, 30),
            (4, 0x5e5, [36, -20, -18], "R530.JT", 2, 14),
            (5, 0x85, [59, -3, -24], "R550.JT", 2, 4),
        ],
        _ => &[],
    }
}

pub fn append(id: AircraftId, rows: &mut Vec<Hardpoint>) {
    let signature = signature(id);
    if signature.is_empty() {
        return;
    }
    if rows.len() != signature.len()
        || !rows.iter().zip(signature).all(|(h, s)| {
            (
                h.location,
                h.flags,
                h.position,
                h.store.as_deref(),
                h.count,
                h.weight_class,
            ) == (s.0, s.1, s.2, Some(s.3), s.4, s.5)
        })
    {
        return;
    }
    let row = |location, flags, position, count, weight_class| Hardpoint {
        location,
        flags,
        position,
        count,
        weight_class,
        store: None,
    };
    match id {
        AircraftId::F15 => {
            rows.push(row(0, 0x3c1, [0, -24, -9], 3, 45));
            rows.push(row(4, 0x205, [50, -20, -16], 2, 90));
        }
        AircraftId::F16C => rows.push(row(4, 0x3e5, [21, -18, -3], 2, 60)),
        AircraftId::F22 => rows.push(row(4, 0x2a5, [36, -18, -18], 4, 90)),
        AircraftId::Mig29 => rows.push(row(0, 0x201, [0, -24, -6], 1, 30)),
        AircraftId::Rafale => rows.push(row(0, 0x201, [0, -24, -12], 1, 40)),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additions_are_empty_guarded_and_idempotent() {
        for (id, total) in [
            (AircraftId::F15, 8),
            (AircraftId::F16C, 10),
            (AircraftId::F22, 9),
            (AircraftId::Mig29, 9),
            (AircraftId::Rafale, 10),
        ] {
            let original: Vec<_> = signature(id)
                .iter()
                .map(|s| Hardpoint {
                    location: s.0,
                    flags: s.1,
                    position: s.2,
                    store: Some(s.3.into()),
                    count: s.4,
                    weight_class: s.5,
                })
                .collect();
            let mut rows = original.clone();
            append(id, &mut rows);
            assert_eq!(rows.len(), total);
            assert!(rows[original.len()..].iter().all(|h| h.store.is_none()));
            append(id, &mut rows);
            assert_eq!(rows.len(), total);
            for index in 0..original.len() {
                for field in 0..6 {
                    let mut edited = original.clone();
                    match field {
                        0 => edited[index].position[0] += 1,
                        1 => edited[index].location ^= 1,
                        2 => edited[index].flags ^= 1,
                        3 => edited[index].count += 1,
                        4 => edited[index].weight_class += 1,
                        _ => edited[index].store = None,
                    }
                    append(id, &mut edited);
                    assert_eq!(edited.len(), original.len());
                }
            }
        }
    }
    #[test]
    fn only_the_appended_f16c_tip_row_takes_the_amraam() {
        assert!(extra_weapon(AircraftId::F16C, 10, F16_TIP, "AIM120.JT"));
        assert!(!extra_weapon(AircraftId::F16C, 9, F16_TIP, "AIM120.JT"));
        assert!(!extra_weapon(AircraftId::F16C, 10, 7, "AIM120.JT"));
        assert!(!extra_weapon(AircraftId::F16C, 10, F16_TIP, "AIM7.JT"));
        assert!(!extra_weapon(AircraftId::F18, 10, F16_TIP, "AIM120.JT"));
        // The row is the reviewed source tip, which the signature guards.
        let tip = signature(AircraftId::F16C)[F16_TIP];
        assert_eq!(
            (tip.0, tip.1, tip.3, tip.4, tip.5),
            (5, 0x85, "AIM9M.JT", 2, 4)
        );
    }

    #[test]
    fn each_tank_requires_its_own_carrier() {
        for (index, carrier) in ["F15.PT", "F16C.PT", "F22.PT", "MIG29.PT", "RAFALE.PT"]
            .iter()
            .enumerate()
        {
            let mut resources = std::collections::BTreeMap::from([((*carrier).into(), Vec::new())]);
            extend_resources(&mut resources);
            assert_eq!(resources.len(), 2);
            assert!(resources.contains_key(TANKS[index].0));
        }
        for id in [AircraftId::F22n, AircraftId::A10] {
            let mut resources = std::collections::BTreeMap::from([(id.pt().into(), Vec::new())]);
            extend_resources(&mut resources);
            assert_eq!(resources.len(), 1);
            let mut rows = Vec::new();
            append(id, &mut rows);
            assert!(rows.is_empty());
            assert!(!tank_allowed(id, 8, "F600.GAS"));
        }
    }

    #[test]
    fn authored_tanks_parse_and_explicit_records_win() {
        let mut data = std::collections::BTreeMap::new();
        extend_resources(&mut data);
        assert!(data.is_empty(), "no carrier, no authored tank");
        data.insert("F15.PT".into(), Vec::new());
        data.insert("F16C.PT".into(), Vec::new());
        for name in ["F22.PT", "MIG29.PT", "RAFALE.PT"] {
            data.insert(name.into(), Vec::new());
        }
        extend_resources(&mut data);
        for (name, _, fuel, shell) in TANKS {
            let t = crate::weapons::Tank::parse(&data[name]).unwrap();
            assert_eq!((t.fuel_weight, t.empty_weight), (fuel, shell));
        }
        data.insert("F610.GAS".into(), b"edited".to_vec());
        extend_resources(&mut data);
        assert_eq!(data["F610.GAS"], b"edited");
        assert_eq!(fuel_bank(F16_WET), Some(0));
        assert_eq!(fuel_bank(10), None);
        assert!(!tank_allowed(AircraftId::F18, 7, "F610.GAS"));
    }
}
