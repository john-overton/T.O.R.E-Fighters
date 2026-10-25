//! Authored physical metadata, not extracted retail bytes.
//! Field evidence: docs/formats/missile-energy-data.md. No flight code here.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reference {
    pub source: &'static str,
    pub mass_kg: f64,
    pub diameter_m: f64,
    pub range_km: f64,
    /// Source IDs in the data document, separately for dimensions and range.
    pub dimensions_evidence: &'static str,
    pub imaging: bool,
    pub range_evidence: &'static str,
}

/// Reviewed anchors. An absent row is unresolved, not permission to substitute
/// a different missile variant. Physics calibration is a separate fitted layer.
pub const REFERENCES: &[Reference] = &[
    Reference {
        imaging: false,
        source: "AIM120.JT",
        mass_kg: 157.85,
        diameter_m: 0.1778,
        range_km: 32.18688,
        dimensions_evidence: "NAVAIR-AMRAAM",
        range_evidence: "USAF-AMRAAM-LOWER-BOUND",
    },
    Reference {
        imaging: false,
        source: "AIM9M.JT",
        mass_kg: 86.183,
        diameter_m: 0.127,
        range_km: 18.,
        dimensions_evidence: "USAF-SIDEWINDER",
        range_evidence: "SIDEWINDER-OPEN-ESTIMATE",
    },
    Reference {
        imaging: true,
        source: "AIM9X.JT",
        mass_kg: 85.275,
        diameter_m: 0.127,
        range_km: 18.,
        dimensions_evidence: "NAVAIR-SIDEWINDER",
        range_evidence: "SHARED-MOTOR-FITTED",
    },
    Reference {
        imaging: false,
        source: "AIM54C.JT",
        mass_kg: 463.,
        diameter_m: 0.380,
        range_km: 150.,
        dimensions_evidence: "NAVAIR-PHOENIX-C",
        range_evidence: "NAVAIR-PHOENIX-C",
    },
    Reference {
        imaging: false,
        source: "AA10.JT",
        mass_kg: 253.,
        diameter_m: 0.23,
        range_km: 60.,
        dimensions_evidence: "ROE2005-R27",
        range_evidence: "ROE2005-R27-EXPORT-PROXY",
    },
    Reference {
        imaging: false,
        source: "R27ER.JT",
        mass_kg: 350.,
        diameter_m: 0.26,
        range_km: 62.5,
        dimensions_evidence: "ROE2005-R27",
        range_evidence: "ROE2005-R27-EXPORT-PROXY",
    },
    Reference {
        imaging: false,
        source: "R27T.JT",
        mass_kg: 245.,
        diameter_m: 0.23,
        range_km: 65.,
        dimensions_evidence: "ROE2005-R27",
        range_evidence: "ROE2005-R27-EXPORT-PROXY",
    },
    Reference {
        imaging: false,
        source: "R27ET.JT",
        mass_kg: 343.,
        diameter_m: 0.26,
        range_km: 80.,
        dimensions_evidence: "ROE2005-R27",
        range_evidence: "ROE2005-R27-EXPORT-PROXY",
    },
    Reference {
        imaging: false,
        source: "AA11.JT",
        mass_kg: 105.,
        diameter_m: 0.17,
        range_km: 30.,
        dimensions_evidence: "ROE2005-R73",
        range_evidence: "ROE2005-R73-EXPORT-PROXY",
    },
    Reference {
        imaging: false,
        source: "AA12.JT",
        mass_kg: 175.,
        diameter_m: 0.20,
        range_km: 50.,
        dimensions_evidence: "ROE2005-RVVAE",
        range_evidence: "ROE2005-RVVAE-FIGHTER-PROXY",
    },
    Reference {
        imaging: false,
        source: "AA9.JT",
        mass_kg: 490.,
        diameter_m: 0.38,
        range_km: 120.,
        dimensions_evidence: "ROE2005-R33",
        range_evidence: "ROE2005-R33-EXPORT-PROXY",
    },
    Reference {
        imaging: true,
        source: "AGM65G.JT",
        mass_kg: 303.907,
        diameter_m: 0.3048,
        range_km: 32.18688,
        dimensions_evidence: "USAF-MAVERICK-G",
        range_evidence: "MAVERICK-OPEN-ESTIMATE",
    },
    Reference {
        imaging: false,
        source: "AGM88.JT",
        mass_kg: 362.874,
        diameter_m: 0.254,
        range_km: 48.28032,
        dimensions_evidence: "USAF-HARM",
        range_evidence: "USAF-HARM-LOWER-BOUND",
    },
    Reference {
        imaging: false,
        source: "AM39.JT",
        mass_kg: 670.,
        diameter_m: 0.35,
        range_km: 70.,
        dimensions_evidence: "MBDA-AM39",
        range_evidence: "MBDA-AM39",
    },
];

pub fn reference(source: &str) -> Option<&'static Reference> {
    REFERENCES.iter().find(|row| row.source == source)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn references_are_positive_unique_and_identified() {
        for (i, row) in REFERENCES.iter().enumerate() {
            assert!(row.mass_kg > 0. && row.diameter_m > 0. && row.range_km > 0.);
            assert!(!row.dimensions_evidence.is_empty() && !row.range_evidence.is_empty());
            assert!(
                !REFERENCES[..i]
                    .iter()
                    .any(|prior| prior.source == row.source)
            );
        }
        assert!(reference("AIM120D.JT").is_none());
    }
}
