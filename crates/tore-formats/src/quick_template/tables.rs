//! Recorded facts from `FA.EXE` for the Quick Mission ground target: the
//! template names by theater, the unit lists behind the placeholders, the
//! nationality to equipment-group word table, the defense roll percentages and
//! the night and stealth rule. They are names and small integers, kept in code
//! the way `enemy_nationality` is; no executable bytes are embedded.
//!
//! The ignored import test `retail_tables_match_the_executable` reads every
//! value back out of a user-owned `FA.EXE` (build 1.02F, addresses below) and
//! fails on any difference. The disc 1.0 build holds the same tables at other
//! addresses that no one has located, so that test skips it.
//! Evidence and reading rules: docs/formats/quick-templates.md.
use super::Placeholder;

/// Equipment groups, indexed 0 through 4 by [`group_of`].
pub const GROUPS: usize = 5;

/// Theater codes in creator source order (creator field 13). Index into
/// [`TEMPLATES`], [`ENEMY_NATIONALITY`] and the creator's target lists.
pub const THEATERS: [&str; 16] = [
    "BAL", "CUB", "EGY", "LFA", "FRA", "GRE", "IRA", "KURILE", "TVIET", "SPA", "APA", "PGU", "NSK",
    "WTA", "UKR", "VLA",
];

/// Template stems per theater in menu order. Entry 0 of each list is the
/// "nothing" template. The resource is `~{stem}.M`. 124 templates.
pub const TEMPLATES: [&[&str]; 16] = [
    // BAL
    &[
        "QBNOTH", "QBFLT", "QBAIR", "QBBRD", "QBXING", "QBACOL", "QBFAIR", "QBSPPY", "QBSHAR",
    ],
    // CUB
    &[
        "QCNOTH", "QCFAIR", "QCSCUD", "QCSUB", "QCLST", "QCCARG", "QCCMHQ",
    ],
    // EGY
    &[
        "QENOTH", "QESFLT", "QESAIR", "QELAIR", "QECMHQ", "QERDRI", "QEARMOR", "QECDEF",
    ],
    // LFA
    &[
        "QLFNOTH", "QLFCARG", "QLFPATR", "QLFSAM", "QLFFAIR", "QLFSTOR", "QLFCMHQ",
    ],
    // FRA
    &[
        "QFNOTH", "QFFLT", "QFSAIR", "QFLAIR", "QFSUP", "QFRDRI", "QFCMHQ", "QFFACT",
    ],
    // GRE
    &[
        "QGRNOTH", "QGRSAIR", "QGRPATR", "QGRRDR", "QGRCARG", "QGRSTOR",
    ],
    // IRA
    &[
        "QIRNOTH", "QIRRDR", "QIRFAIR", "QIRPOW", "QIRCCC", "QIRARM", "QIRSCUD", "QIRCWP",
        "QIRRETR",
    ],
    // KURILE
    &[
        "QKNOTH", "QKSFLT", "QKLFLT", "QKSCFT", "QKSUB", "QKPLNGR", "QKSILO", "QKARMOR",
    ],
    // TVIET
    &[
        "QTNOTH", "QTBARG", "QTCARGO", "QTBRDG", "QTBUNK", "QTCOMM", "QTSTRG", "QTTRUCK", "QTAAA",
        "QTSAM",
    ],
    // SPA
    &[
        "QSPNOTH", "QSPFAIR", "QSPSAM", "QSPASA", "QSPFRU", "QSPSUP", "QSPCMHQ",
    ],
    // APA
    &[
        "QAPNOTH", "QAPFAIR", "QAPBLK", "QAPPATR", "QAPHELO", "QAPSAM", "QAPCMHQ",
    ],
    // PGU
    &[
        "QPGNOTH", "QPGPATR", "QPGFAIR", "QPGSAM", "QPGSRUN", "QPGRDR", "QPGWSHP",
    ],
    // NSK
    &[
        "QNSNOTH", "QNSFAIR", "QNSARM", "QNSFOA", "QNSBORD", "QNSCOL", "QNSSUP",
    ],
    // WTA
    &[
        "QWTNOTH", "QWTFAIR", "QWTPATR", "QWTHYDO", "QWTWARS", "QWTCARG", "QWTLAND",
    ],
    // UKR
    &[
        "QUNOTH", "QUSFLT", "QULFLT", "QUCITY", "QUFACT", "QUSTRIP", "QUCOL", "QUNUKE", "QUBRI",
    ],
    // VLA
    &[
        "QVNOTH", "QVSFLT", "QVSAIR", "QVLAIR", "QVCMHQ", "QVARMOR", "QVRDRI", "QVSUP",
    ],
];

/// Templates in the archive that no menu entry offers (survey 4.1): a copy of
/// `QUFACT`, a Ukraine bunker, a Ukraine radar site and two for theaters that
/// do not exist. They still parse.
pub const UNREFERENCED: [&str; 5] = ["QFACT", "QUBUNK", "QURADAR", "QMANOTH", "QOSNOTH"];

/// The order of the template-name pointer table at [`addresses::TEMPLATE_POINTERS`], as
/// indexes into [`THEATERS`]: Ukraine, Kuril, Vietnam, Cuba, Persian Gulf,
/// Falklands, Panama, South Korea, Pakistan, Taiwan, Iraq, Greece, Egypt,
/// Vladivostok, France, Baltics.
pub const POINTER_TABLE_ORDER: [usize; 16] = [14, 7, 8, 1, 11, 3, 10, 12, 9, 13, 6, 5, 2, 15, 4, 0];

/// Enemy nationality (creator list index) the creator picks for each theater
/// (docs/formats/quick-mission.md).
pub const ENEMY_NATIONALITY: [usize; 16] =
    [10, 33, 14, 57, 3, 41, 23, 10, 20, 37, 34, 24, 9, 2, 10, 2];

/// Equipment group per creator nationality index, from the word table
/// `0x4f1e58`.
pub const NATIONALITY_GROUP: [u8; 60] = [
    0, 0, 2, 1, 0, 0, 4, 4, 0, 2, 2, 0, 3, 4, 3, 2, 2, 0, 0, 2, 2, 4, 2, 2, 2, 4, 4, 4, 4, 4, 4, 3,
    1, 2, 2, 0, 0, 2, 2, 4, 4, 4, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 0, 0, 3, 4, 3,
];

/// The equipment group a creator nationality draws from, `None` past the list.
pub fn group_of(nationality: usize) -> Option<usize> {
    NATIONALITY_GROUP.get(nationality).map(|g| usize::from(*g))
}

/// Chance in percent that a `<sam>` or `<aaa>` slot is manned at defense level
/// 0 (none), 1 (light), 2 (moderate) and 3 (heavy). Both tables hold the same
/// values (`0x4f3148` for SAM and `0x4f31b8` for AAA).
pub const DEFENSE_PERCENT: [u32; 4] = [0, 25, 60, 100];

/// Percent for a defense level, `None` past heavy.
pub fn defense_percent(level: usize) -> Option<u32> {
    DEFENSE_PERCENT.get(level).copied()
}

/// Creator conditions index (field 15) that means night.
pub const NIGHT_CONDITION: usize = 6;
/// A friendly wing flying one of these at night turns every manned `<aaa>` into
/// [`NIGHT_AAA`] with skill [`NIGHT_AAA_SKILL`] (list `0x4f31b0`, stealth flag
/// `0x4f1c84`). Neither aircraft is imported yet, so the rule is dormant.
pub const NIGHT_STEALTH_AIRCRAFT: [&str; 2] = ["F117.PT", "B2.PT"];
pub const NIGHT_AAA: &[&str] = &["ZSU23.NT"];
/// Novice.
pub const NIGHT_AAA_SKILL: i32 = 0;

/// One placeholder's unit lists, `groups[group]`.
#[derive(Clone, Copy, Debug)]
pub struct PlaceholderLists {
    pub placeholder: Placeholder,
    pub groups: [&'static [&'static str]; GROUPS],
}

/// The eleven list blocks. In the executable each block stores its five lists
/// as groups 4, 0, 1, 2, 3 (`0x4f2e68` onward); the code selects a list by
/// group, so only the group index matters here.
pub const LISTS: [PlaceholderLists; 11] = [
    PlaceholderLists {
        placeholder: Placeholder::Cruiser,
        groups: [
            &["IOWA.NT", "TICON.NT"],
            &["TYPE69.NT"],
            &["KIROV.NT", "SOVR.NT", "JIANC.NT"],
            &["JIANE.NT", "KNOX.NT"],
            &["JIANE.NT", "KNOX.NT"],
        ],
    },
    PlaceholderLists {
        placeholder: Placeholder::Cargo,
        groups: [
            &["CARGO.NT", "SACRAM.NT"],
            &["CARGO.NT"],
            &["CARGO.NT", "OLEKMA.NT"],
            &["CARGO.NT"],
            &["CARGO.NT"],
        ],
    },
    PlaceholderLists {
        placeholder: Placeholder::Carrier,
        groups: [
            &["NIMZ.NT", "WASP.NT"],
            &["CLEM.NT"],
            &["KIEV.NT"],
            &["KIEV.NT"],
            &["NIMZ.NT"],
        ],
    },
    PlaceholderLists {
        placeholder: Placeholder::Destroyer,
        groups: [
            &["TICON.NT"],
            &["TYPE69.NT"],
            &["KRIVAK.NT", "JIANC.NT"],
            &["JIANE.NT", "KNOX.NT"],
            &["JIANE.NT", "KNOX.NT"],
        ],
    },
    PlaceholderLists {
        placeholder: Placeholder::Small,
        groups: [
            &["SL100.NT", "LCAC.NT", "SESHDW.NT"],
            &["SL100.NT"],
            &["PMORN.NT", "SARAN.NT"],
            &["SARAN.NT"],
            &["SL100.NT"],
        ],
    },
    PlaceholderLists {
        placeholder: Placeholder::Hovercraft,
        groups: [
            &["SL100.NT", "LCAC.NT", "SESHDW.NT"],
            &["SL100.NT"],
            &["PMORN.NT", "SARAN.NT"],
            &["SARAN.NT"],
            &["SL100.NT"],
        ],
    },
    PlaceholderLists {
        placeholder: Placeholder::Vehicle,
        groups: [
            &["TRUCK.NT", "TANKER.NT"],
            &["TRUCK.NT", "TANKER.NT", "SRDR1.NT", "SRDR2.NT"],
            &["TRUCK.NT", "TANKER.NT", "LTRACK.NT", "SFLUSH.NT"],
            &["TRUCK.NT", "TANKER.NT", "LTRACK.NT", "SFLUSH.NT"],
            &["TRUCK.NT", "TANKER.NT", "LTRACK.NT"],
        ],
    },
    PlaceholderLists {
        placeholder: Placeholder::Tank,
        groups: [
            &["M1.NT", "M2.NT"],
            &["T80.NT", "T90.NT"],
            &["T72.NT", "T80.NT", "T90.NT"],
            &["T72.NT", "M1.NT", "M2.NT"],
            &["M1.NT", "M2.NT"],
        ],
    },
    PlaceholderLists {
        placeholder: Placeholder::Afv,
        groups: [
            &["M113.NT", "HUMVEE.NT"],
            &["M113.NT", "HUMVEE.NT"],
            &["BMP2.NT", "BTR80.NT"],
            &["M113.NT", "BMP2.NT", "BTR80.NT"],
            &["M113.NT", "HUMVEE.NT"],
        ],
    },
    PlaceholderLists {
        placeholder: Placeholder::Sam,
        groups: [
            &["FIM92.NT", "ROLAND.NT", "CHAP.NT"],
            &["MIS.NT", "ASA5.NT"],
            &[
                "SA6.NT", "SA7.NT", "SA9.NT", "SA13.NT", "SA14.NT", "SA15.NT", "2S6.NT",
            ],
            &[
                "M113.NT", "CHAP.NT", "SA6.NT", "SA9.NT", "SA14.NT", "ASA5.NT",
            ],
            &["M113.NT", "CHAP.NT", "ASA5.NT", "SA7.NT", "SA13.NT"],
        ],
    },
    PlaceholderLists {
        placeholder: Placeholder::Aaa,
        groups: [
            &["M163.NT", "M113.NT"],
            &["M113.NT", "ZSU23.NT"],
            &["ZSU23.NT", "ZSU57.NT"],
            &["M113.NT", "ZSU23.NT", "ZSU57.NT", "ZIF31.NT"],
            &["ZSU23.NT", "ZIF31.NT", "M113.NT"],
        ],
    },
];

/// The unit list a placeholder draws from for an equipment group. `None` for
/// `<nothing>` and for a group past 4.
pub fn equipment(placeholder: Placeholder, group: usize) -> Option<&'static [&'static str]> {
    LISTS
        .iter()
        .find(|lists| lists.placeholder == placeholder)
        .and_then(|lists| lists.groups.get(group).copied())
}

/// The template resource for a theater (source index) and menu entry.
pub fn resource(theater: usize, entry: usize) -> Option<String> {
    TEMPLATES
        .get(theater)
        .and_then(|list| list.get(entry))
        .map(|stem| format!("~{stem}.M"))
}

/// FA.EXE 1.02F addresses the cross-check reads (hash e31560c2...). Virtual
/// addresses in `.data`.
pub mod addresses {
    /// Where each block starts (the group 4 list), in [`super::LISTS`] order.
    pub const BLOCKS: [usize; 11] = [
        0x4f2e68, 0x4f2eb0, 0x4f2ee8, 0x4f2f18, 0x4f2f58, 0x4f2f90, 0x4f2fc8, 0x4f3030, 0x4f3080,
        0x4f30d0, 0x4f3158,
    ];
    pub const NIGHT_AAA_LIST: usize = 0x4f31b0;
    pub const SAM_PERCENT: usize = 0x4f3148;
    pub const AAA_PERCENT: usize = 0x4f31b8;
    pub const NATIONALITY_GROUP: usize = 0x4f1e58;
    pub const TEMPLATE_POINTERS: usize = 0x4f3278;
    /// The pointer table runs to here (exclusive); zero words separate some
    /// theaters.
    pub const TEMPLATE_POINTERS_END: usize = 0x4f348c;
    /// The placeholder strings the generator compares a `type` against.
    pub const PLACEHOLDER_TOKENS: [(super::Placeholder, usize); 12] = [
        (super::Placeholder::Aaa, 0x4f3914),
        (super::Placeholder::Nothing, 0x4f391c),
        (super::Placeholder::Sam, 0x4f3928),
        (super::Placeholder::Afv, 0x4f3930),
        (super::Placeholder::Tank, 0x4f3938),
        (super::Placeholder::Vehicle, 0x4f3940),
        (super::Placeholder::Hovercraft, 0x4f394c),
        (super::Placeholder::Small, 0x4f395c),
        (super::Placeholder::Destroyer, 0x4f3964),
        (super::Placeholder::Carrier, 0x4f3970),
        (super::Placeholder::Cargo, 0x4f397c),
        (super::Placeholder::Cruiser, 0x4f3984),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_lists_match_the_survey_counts() {
        let counts: Vec<_> = TEMPLATES.iter().map(|list| list.len()).collect();
        assert_eq!(counts, [9, 7, 8, 7, 8, 6, 9, 8, 10, 7, 7, 7, 7, 7, 9, 8]);
        assert_eq!(counts.iter().sum::<usize>(), 124);
        for (theater, list) in TEMPLATES.iter().enumerate() {
            // Entry 0 is the "nothing" template of that theater.
            assert!(list[0].ends_with("NOTH"), "{}", THEATERS[theater]);
            assert!(list.iter().all(|stem| stem.starts_with('Q')));
        }
        let mut seen = std::collections::BTreeSet::new();
        for stem in TEMPLATES.iter().flat_map(|list| list.iter()) {
            assert!(seen.insert(*stem), "duplicate {stem}");
        }
        for stem in UNREFERENCED {
            assert!(!seen.contains(stem));
        }
        assert_eq!(resource(14, 6).as_deref(), Some("~QUCOL.M"));
        assert_eq!(resource(14, 99), None);
        assert_eq!(resource(16, 0), None);
    }

    #[test]
    fn pointer_table_order_is_a_permutation() {
        let mut order = POINTER_TABLE_ORDER;
        order.sort_unstable();
        assert_eq!(order, core::array::from_fn(|i| i));
    }

    #[test]
    fn groups_follow_the_default_enemy_of_each_theater() {
        // Egypt and the Falklands group 3, France group 1, Greece group 4, the
        // other twelve group 2 (survey 4.2).
        let groups: Vec<_> = ENEMY_NATIONALITY
            .iter()
            .map(|n| group_of(*n).unwrap())
            .collect();
        for (theater, group) in THEATERS.iter().zip(&groups) {
            let want = match *theater {
                "EGY" | "LFA" => 3,
                "FRA" => 1,
                "GRE" => 4,
                _ => 2,
            };
            assert_eq!(*group, want, "{theater}");
        }
        assert_eq!(group_of(0), Some(0)); // American
        assert_eq!(group_of(59), Some(3)); // Serbian
        assert_eq!(group_of(60), None);
        assert!(NATIONALITY_GROUP.iter().all(|g| usize::from(*g) < GROUPS));
        // Sudanese is the other member of the French group.
        assert_eq!(group_of(32), Some(1));
    }

    #[test]
    fn every_list_is_a_nonempty_nt_set_and_m113_stays_a_pick() {
        for lists in LISTS {
            for list in lists.groups {
                assert!(!list.is_empty() && list.iter().all(|n| n.ends_with(".NT")));
                let mut sorted = list.to_vec();
                sorted.sort_unstable();
                sorted.dedup();
                assert_eq!(sorted.len(), list.len(), "{:?}", lists.placeholder);
            }
        }
        for group in [1, 3, 4] {
            assert!(
                equipment(Placeholder::Aaa, group)
                    .unwrap()
                    .contains(&"M113.NT")
            );
        }
        assert!(equipment(Placeholder::Sam, 3).unwrap().contains(&"M113.NT"));
        assert!(equipment(Placeholder::Sam, 4).unwrap().contains(&"M113.NT"));
        assert!(!equipment(Placeholder::Sam, 2).unwrap().contains(&"M113.NT"));
        assert_eq!(equipment(Placeholder::Nothing, 0), None);
        assert_eq!(equipment(Placeholder::Tank, 5), None);
        assert_eq!(equipment(Placeholder::Carrier, 1), Some(&["CLEM.NT"][..]));
        // Every placeholder but <nothing> has exactly one block.
        for p in Placeholder::ALL {
            let blocks = LISTS.iter().filter(|l| l.placeholder == p).count();
            assert_eq!(blocks, usize::from(p != Placeholder::Nothing), "{p:?}");
        }
    }

    #[test]
    fn defense_and_night_rule_data() {
        assert_eq!(DEFENSE_PERCENT, [0, 25, 60, 100]);
        assert_eq!(defense_percent(3), Some(100));
        assert_eq!(defense_percent(4), None);
        assert_eq!(NIGHT_CONDITION, 6);
        assert_eq!(NIGHT_STEALTH_AIRCRAFT, ["F117.PT", "B2.PT"]);
        assert_eq!(NIGHT_AAA, &["ZSU23.NT"]);
        assert_eq!(NIGHT_AAA_SKILL, 0);
        assert_eq!(addresses::BLOCKS.len(), LISTS.len());
    }
}
