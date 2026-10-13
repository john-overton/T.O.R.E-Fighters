//! How humid each theater's air is, for the look of a large explosion
//! (slice B12; docs/spec/explosions.md, "Climate"). A humid theater's blasts
//! throw a white condensation cloud; a dry one's kick up wide tan dust that
//! hangs in the air. Presentation only: nothing in the simulation, the wire
//! or a recording reads it, and it is fixed by the mission's theater and
//! weather, so every machine and every replay of a mission agree.
//!
//! The numbers are fitted (opinionated, John, 2026-10-12: one fixed value per
//! theater, a best estimate; Egypt, Iraq and the Persian Gulf arid, Vietnam,
//! Cuba and Panama humid, the rest the agent's estimate from each region's
//! climate; fog or rain in the mission's weather pushes toward humid). The
//! original game has no such setting.

/// Humidity of a theater by its base code, 0 (desert) to 1 (jungle). A code
/// the table does not name is temperate, [`TEMPERATE`].
pub const THEATERS: [(&str, f64); 16] = [
    ("EGY", 0.10),
    ("IRA", 0.15),
    ("PGU", 0.20),
    ("SPA", 0.30),
    ("GRE", 0.40),
    ("UKR", 0.50),
    ("FRA", 0.60),
    ("LFA", 0.60),
    ("VLA", 0.60),
    ("BAL", 0.65),
    ("NSK", 0.65),
    ("KURILE", 0.80),
    ("WTA", 0.85),
    ("CUB", 0.85),
    ("TVIET", 0.90),
    ("APA", 0.95),
];

/// The humidity of a theater the table does not name.
pub const TEMPERATE: f64 = 0.5;
/// How much fog or rain in the mission's weather adds, and an overcast deck.
pub const FOG_PUSH: f64 = 0.35;
pub const OVERCAST_PUSH: f64 = 0.15;

/// The table's humidity for the theater `code` (a base code or a layout name
/// such as `UKR.MM` or `~QUCOL`'s theater).
pub fn theater_humidity(code: &str) -> f64 {
    let base = tore_formats::theater::base_theater(code).unwrap_or(code);
    THEATERS
        .iter()
        .find(|(name, _)| *name == base)
        .map_or(TEMPERATE, |(_, humidity)| *humidity)
}

/// The humidity of a mission in theater `code` flown under the weather
/// module `layer` (`FOG1`, `CLOUD1`, `DAY2` or a theater variant of one):
/// fog or rain adds [`FOG_PUSH`], an overcast deck [`OVERCAST_PUSH`], at most
/// fully humid.
pub fn humidity(code: &str, layer: &str) -> f64 {
    let layer = layer.to_ascii_uppercase();
    let push = if layer.contains("FOG") || layer.contains("RAIN") {
        FOG_PUSH
    } else if layer.contains("CLOUD") {
        OVERCAST_PUSH
    } else {
        0.
    };
    (theater_humidity(code) + push).min(1.)
}

impl crate::terrain::Terrain {
    /// This mission's humidity: its theater's, pushed by its weather
    /// ([`humidity`]).
    pub fn humidity(&self) -> f64 {
        let layer = self
            .condition
            .and_then(|index| tore_sim::environment::CONDITIONS.get(index))
            .map_or(self.environment.layer.as_str(), |choice| choice.layer);
        humidity(&self.layout, layer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserts_are_dry_jungles_humid_and_every_theater_is_listed() {
        for code in ["EGY", "IRA", "PGU"] {
            assert!(theater_humidity(code) <= 0.2, "{code}");
        }
        for code in ["TVIET", "CUB", "APA"] {
            assert!(theater_humidity(code) >= 0.85, "{code}");
        }
        // The table covers the sixteen base theaters, each once.
        for (code, _) in tore_formats::theater::THEATERS {
            assert_eq!(
                THEATERS.iter().filter(|(name, _)| name == code).count(),
                1,
                "{code}"
            );
        }
        assert!(THEATERS.iter().all(|(_, h)| (0. ..=1.).contains(h)));
        // Layout names resolve to their base theater.
        assert_eq!(theater_humidity("TVIET.MM"), theater_humidity("TVIET"));
        assert_eq!(theater_humidity("NOWHERE"), TEMPERATE);
    }

    #[test]
    fn fog_and_overcast_push_toward_humid_and_never_past_it() {
        let clear = humidity("UKR", "DAY2");
        assert_eq!(clear, theater_humidity("UKR"));
        assert!(humidity("UKR", "CLOUD1") > clear);
        assert!(humidity("UKR", "FOG1") > humidity("UKR", "CLOUD1"));
        assert!(humidity("EGY", "efog1.lay") > humidity("EGY", "EDAY2.LAY"));
        assert_eq!(humidity("APA", "FOG1"), 1.);
    }
}
