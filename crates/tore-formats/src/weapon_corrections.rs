//! Load-time store identities and fitted missile/bomb variants. No retail bytes.
//! See docs/spec/missiles.md for donor, scaling and provenance rules.
use super::Weapon;

/// Opinionated smaller guided stores, requested 2026-10-15. Ratios and
/// dimensions are fitted game classes, not retail or aerodynamic measurements.
pub const SMALL_BOMBS: [(&str, &str, i32, i16, &str); 3] = [
    ("GBU12.JT", "PAVEWAY.JT", 500, 4, "GB12.SH"),
    ("GBU16.JT", "PAVEWAY.JT", 1000, 2, "GB16.SH"),
    ("GBU38.JT", "GBU29.JT", 500, 4, "GB38.SH"),
];

pub fn donor(source: &str) -> Option<&'static str> {
    if R27_DERIVED.contains(&source) {
        return Some("AA10.JT");
    }
    SMALL_BOMBS.iter().find(|v| v.0 == source).map(|v| v.1)
}

/// Source resource for a virtual record or its private, resized shape alias.
pub fn resource_donor(
    source: &str,
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
) -> Option<String> {
    if let Some(name) = donor(source) {
        return Some(name.into());
    }
    let variant = SMALL_BOMBS.iter().find(|v| v.4 == source)?;
    Weapon::parse(variant.1, resources.get(variant.1)?)
        .ok()?
        .shape
}

pub const R27_DERIVED: [&str; 3] = ["R27ER.JT", "R27T.JT", "R27ET.JT"];

/// Full names are separate from the compact HUD labels. Unknown subvariants
/// are described honestly instead of assigning a modern missile's identity.
pub fn identity(source: &str) -> Option<(&'static str, &'static str)> {
    Some(match source {
        "GBU12.JT" => ("GBU-12", "GBU-12 Paveway II (500 lb)"),
        "GBU16.JT" => ("GBU-16", "GBU-16 Paveway II (1000 lb)"),
        "GBU38.JT" => ("GBU-38", "GBU-38 JDAM (500 lb)"),
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
    // Opinionated, John 2026-10-15: double shell damage, not blast geometry.
    if matches!(w.source.as_str(), "C_40.JT" | "C_105.JT") {
        for amount in &mut w.damage.by_class {
            *amount = amount.saturating_mul(2);
        }
    }
    // Opinionated, John 2026-10-15: roughly 30 m fragmentation reach.
    // Percentage is agent-fitted; armor filtering is splash-only below.
    if w.source == "C_105.JT" {
        w.damage.collateral_radius = 100;
        w.damage.collateral_percent = 35;
    }
    if let Some((short, long)) = identity(&w.source) {
        w.hud_name = short.into();
        w.name = long.into();
    }
    if let Some((_, _, weight, divisor, shape)) = SMALL_BOMBS.iter().find(|v| v.0 == w.source) {
        w.weight = *weight;
        w.shape = Some((*shape).into());
        w.damage.by_class = w.damage.by_class.map(|damage| damage / divisor);
        w.damage.collateral_radius =
            (f64::from(w.damage.collateral_radius) / f64::from(*divisor).cbrt()).round() as i16;
        return;
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
    crate::aircraft::stations::extend_resources(resources);
    for (name, donor, _, _, shape) in SMALL_BOMBS {
        if let Some(bytes) = resources.get(donor).cloned() {
            if let Ok(weapon) = Weapon::parse(donor, &bytes)
                && let Some(source) = weapon.shape
                && let Some(model) = resources.get(&source).cloned()
            {
                resources.entry(shape.into()).or_insert(model);
            }
            resources.entry(name.into()).or_insert(bytes);
        }
    }
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

/// Splash-only class shares, leaving direct-hit damage intact. Class order:
/// aircraft/fallback, ships, structures, armor, soft surface targets.
/// Opinionated fragmentation rule; docs/spec/ac130-linked-guns.md.
pub fn collateral_damage(w: &Weapon) -> [i16; 5] {
    const FRAGMENTATION: [i32; 5] = [100, 0, 100, 0, 100];
    if w.source != "C_105.JT" {
        return w.damage.by_class;
    }
    std::array::from_fn(|class| {
        (i32::from(w.damage.by_class[class]) * FRAGMENTATION[class] / 100) as i16
    })
}
