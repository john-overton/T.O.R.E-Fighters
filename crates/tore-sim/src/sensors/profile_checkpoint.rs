//! The coders of the sensor profiles, copied records (docs/formats/checkpoint.md,
//! "Shared records"): an aircraft's radar, infrared, visual and jammer
//! channels. The profiles never change after the import, but a handoff copies
//! them into state the fresh world does not hold, so they are coded by value.
//! `SensorProfiles` is coded in full wherever it appears; its callers
//! (`live::Configuration` and `Sensors`) code it as a shared record so the
//! copies cost one coding.

use super::{
    Generation, InfraredProfile, JammerProfile, Notch, Preset, RadarProfile, Resistance,
    SensorProfiles, Volume,
};

crate::checkpoint_struct!(Volume {
    azimuth_rad,
    elevation_rad,
    minimum_ft,
    maximum_ft,
    minimum_relative_ft,
    maximum_relative_ft,
});

crate::checkpoint_enum!(Preset {
    Basic = 0,
    Transitional = 1,
    Advanced = 2,
});

crate::checkpoint_enum!(Generation {
    Early = 0,
    Transitional = 1,
    LateColdWar = 2,
});

crate::checkpoint_struct!(Notch {
    enabled,
    half_width_fps,
    centre_factor,
});

crate::checkpoint_struct!(Resistance {
    burn_through_nmi,
    coupling_rad,
    sidelobe_floor,
});

crate::checkpoint_struct!(RadarProfile {
    record,
    search,
    track,
    look_down,
    preset,
    notch,
    resistance,
    band,
    source_flags,
    source_doppler,
});

// `InfraredProfile` and `VisualProfile` are aliases of `PassiveProfile`, so
// this one coder serves both.
crate::checkpoint_struct!(InfraredProfile {
    record,
    search,
    track,
});

crate::checkpoint_struct!(JammerProfile {
    record,
    generation,
    strength,
    band,
    radio_frequency,
});

crate::checkpoint_struct!(SensorProfiles {
    aircraft,
    radar,
    infrared,
    visual,
    jammer,
    signature,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{Models, round_trip, to_bytes};
    use crate::sensors::signature::SignatureProfile;

    fn volume(maximum_nmi: f64) -> Volume {
        Volume {
            azimuth_rad: 60f64.to_radians(),
            elevation_rad: 45f64.to_radians(),
            minimum_ft: 100.,
            maximum_ft: maximum_nmi * crate::sensors::FEET_PER_NAUTICAL_MILE,
            minimum_relative_ft: f64::NEG_INFINITY,
            maximum_relative_ft: f64::INFINITY,
        }
    }

    fn profiles() -> SensorProfiles {
        let preset = Preset::Transitional;
        SensorProfiles {
            aircraft: tore_formats::aircraft::AircraftId::Mig29,
            radar: Some(RadarProfile {
                record: "TESTR.SEE".into(),
                search: volume(90.),
                track: volume(50.),
                look_down: 35.5,
                preset,
                notch: preset.notch(),
                resistance: preset.resistance(),
                band: 2,
                source_flags: [9, 200],
                source_doppler: [5, 6, 7],
            }),
            infrared: Some(InfraredProfile {
                record: "TESTI.SEE".into(),
                search: volume(20.),
                track: volume(10.),
            }),
            visual: Some(InfraredProfile {
                record: "TESTV.SEE".into(),
                search: volume(6.),
                track: volume(6.),
            }),
            jammer: Some(JammerProfile {
                record: "TEST.ECM".into(),
                generation: Generation::Transitional,
                strength: 0.3,
                band: 1,
                radio_frequency: true,
            }),
            signature: SignatureProfile {
                radar: 12.5,
                infrared: 80.,
                ..SignatureProfile::default()
            },
        }
    }

    #[test]
    fn a_profile_set_round_trips_equal_with_its_infinities() {
        let models = Models::default();
        let original = profiles();
        // The fixture's volumes are unbounded above and below in altitude.
        assert!(
            original
                .radar
                .as_ref()
                .is_some_and(|r| r.search.maximum_relative_ft.is_infinite())
        );
        assert_eq!(round_trip(&original, &models).unwrap(), original);
        let bare = SensorProfiles {
            radar: None,
            infrared: None,
            visual: None,
            jammer: None,
            signature: SignatureProfile::default(),
            ..original
        };
        assert_eq!(round_trip(&bare, &models).unwrap(), bare);
    }

    #[test]
    fn every_preset_and_generation_codes_and_a_changed_field_changes_the_bytes() {
        let models = Models::default();
        for preset in [Preset::Basic, Preset::Transitional, Preset::Advanced] {
            assert_eq!(round_trip(&preset, &models).unwrap(), preset);
        }
        for generation in [
            Generation::Early,
            Generation::Transitional,
            Generation::LateColdWar,
        ] {
            assert_eq!(round_trip(&generation, &models).unwrap(), generation);
        }
        let base = profiles();
        let coded = to_bytes(&base, &models).unwrap();
        let mut changed = base.clone();
        changed.jammer.as_mut().unwrap().radio_frequency = false;
        assert_ne!(to_bytes(&changed, &models).unwrap(), coded);
        let mut changed = base.clone();
        changed.radar.as_mut().unwrap().source_doppler[2] += 1;
        assert_ne!(to_bytes(&changed, &models).unwrap(), coded);
        let mut changed = base;
        changed.signature.reference.signature = -0.;
        assert_ne!(to_bytes(&changed, &models).unwrap(), coded);
    }
}
