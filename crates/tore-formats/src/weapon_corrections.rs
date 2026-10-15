//! Load-time store identities and fitted R-27 variants. No retail bytes live here.
//! See docs/spec/missiles.md, "Corrected identities and R-27 stores".
use super::Weapon;

pub const R27_DERIVED: [&str; 3] = ["R27ER.JT", "R27T.JT", "R27ET.JT"];

/// Full names are separate from the compact HUD labels. Unknown subvariants
/// are described honestly instead of assigning a modern missile's identity.
pub fn identity(source: &str) -> Option<(&'static str, &'static str)> {
    Some(match source {
        "AA10.JT" => ("AA-10A", "R-27R (AA-10A Alamo-A)"),
        "R27ER.JT" => ("AA-10C", "R-27ER (AA-10C Alamo-C)"),
        "R27T.JT" => ("AA-10B", "R-27T (AA-10B Alamo-B)"),
        "R27ET.JT" => ("AA-10D", "R-27ET (AA-10D Alamo-D)"),
        "AA11.JT" => ("AA-11", "R-73 (AA-11 Archer)"),
        "AA11B.JT" => ("AA-11B", "AA-11B (game IR variant)"),
        "AA2.JT" => ("AA-2A", "R-3S (AA-2A Atoll)"),
        "AA6.JT" => ("AA-6A", "R-40R family (AA-6A Acrid)"),
        "AA8.JT" => ("AA-8", "R-60M (AA-8 Aphid)"),
        "AA9.JT" => ("AA-9", "R-33 (AA-9 Amos)"),
        "AA12.JT" => ("AA-12", "R-77 (AA-12 Adder)"),
        "PL10.JT" => ("PL10-R", "PL-10 (1997 radar concept)"),
        "R530.JT" => ("S530D", "Super 530D"),
        "R550.JT" => ("R-550", "R-550 Magic 2"),
        "MICA.JT" => ("MICA-EM", "MICA EM"),
        "AIM54C.JT" => ("AIM-54C", "AIM-54C Phoenix"),
        _ => return None,
    })
}

pub(super) fn apply(w: &mut Weapon) {
    if let Some((short, long)) = identity(&w.source) {
        w.hud_name = short.into();
        w.name = long.into();
    }
    if !R27_DERIVED.contains(&w.source.as_str()) {
        return;
    }
    // Explicit derived identities always use AA10 as their donor. These are
    // fitted gameplay envelopes, not published maximum-range guarantees.
    let extended = matches!(w.source.as_str(), "R27ER.JT" | "R27ET.JT");
    let infrared = matches!(w.source.as_str(), "R27T.JT" | "R27ET.JT");
    if extended {
        w.weight = 772;
        w.movement.acceleration = w.movement.acceleration.saturating_mul(5) / 4;
        w.movement.fuel_t = w.movement.fuel_t.saturating_mul(3) / 2;
        w.movement.remove_t = w.movement.remove_t.saturating_mul(3) / 2;
        for zone in &mut w.seeker.zones {
            zone.maximum_range = zone.maximum_range.saturating_mul(3) / 2;
        }
    }
    if infrared {
        w.flags &= !0x300;
        w.seeker.signature = 2;
        w.seeker.all_aspect = 1;
        // The larger motor does not make the heat seeker see farther.
        w.seeker.zones[0].maximum_range = 60_000;
        w.seeker.zones[1].maximum_range = if extended { 150_000 } else { 100_000 };
        w.weight = if extended { 756 } else { 541 };
    }
}

/// Add virtual donor records in memory when loading a user's pack. Parsing the
/// virtual name applies its variant above. Existing explicit records win.
pub fn extend_resources(resources: &mut std::collections::BTreeMap<String, Vec<u8>>) {
    if let Some(donor) = resources.get("AA10.JT").cloned() {
        for name in R27_DERIVED {
            resources
                .entry(name.into())
                .or_insert_with(|| donor.clone());
        }
    }
}

/// Reviewed retail aspect categories plus the authored R-27 IR derivatives.
/// AA8's all-aspect record is presented as the R-60M role. PL7 remains catalog-only.
pub fn infrared_aspect(source: &str) -> Option<bool> {
    match source {
        "AIM9B.JT" | "AA2.JT" => Some(false),
        "AIM9M.JT" | "AIM9X.JT" | "AA11.JT" | "AA11B.JT" | "AA8.JT" | "R550.JT" | "PL7.JT"
        | "R27T.JT" | "R27ET.JT" => Some(true),
        _ => None,
    }
}
