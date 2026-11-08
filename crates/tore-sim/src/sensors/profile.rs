//! Imported sensor capability profiles: radar, infrared air-to-air and the
//! self-protection jammer, with the aircraft signature inputs they compare
//! against. Nominal volumes, look-down coefficients and signatures come from the
//! player's own PT/SEE/ECM records. The preset, generation and band groupings
//! are agent tuning choices recorded in docs/radar.md, not recovered retail
//! classifications, and are assigned per record so no aircraft-specific radar
//! code exists anywhere else.
use super::{invalid, signature::SignatureProfile};
use tore_formats::{
    Result,
    aircraft::{Aircraft, AircraftId},
    weapons::{Countermeasures, Seeker, Zone},
};

/// One nautical mile in the source records and in this simulation.
pub const FEET_PER_NAUTICAL_MILE: f64 = 6076.;
/// Source angle units in a full turn, shared with the flight model.
pub const SOURCE_TURN: f64 = 65520.;
/// Recovered FA scope ladder. Index 1 is the recovered radar-reset default.
pub const RANGE_LADDER_NMI: [f64; 6] = [5., 10., 25., 50., 100., 150.];
/// Recovered radar reset selection, the 10-mile setting.
pub const DEFAULT_RANGE_INDEX: usize = 1;

/// An independent acquisition or tracking volume. Search and tracking stay
/// separate, including sensors whose tracking distance exceeds their search
/// distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    pub azimuth_rad: f64,
    pub elevation_rad: f64,
    pub minimum_ft: f64,
    pub maximum_ft: f64,
    pub minimum_relative_ft: f64,
    pub maximum_relative_ft: f64,
}
impl Volume {
    /// Source zones carry binary angles and feet. Rear-facing records store a
    /// negative heading; the magnitude is the half-angle either way.
    pub fn from_zone(z: &Zone) -> Result<Self> {
        let angle = |v: i16| f64::from(v).abs() * std::f64::consts::TAU / SOURCE_TURN;
        let bound = |v: i32, sentinel: i32, infinite: f64| {
            if v == sentinel {
                infinite
            } else {
                f64::from(v)
            }
        };
        let volume = Self {
            azimuth_rad: angle(z.heading),
            elevation_rad: angle(z.pitch),
            minimum_ft: f64::from(z.minimum_range),
            maximum_ft: f64::from(z.maximum_range),
            minimum_relative_ft: bound(z.minimum_altitude, i32::MIN, f64::NEG_INFINITY),
            maximum_relative_ft: bound(z.maximum_altitude, i32::MAX, f64::INFINITY),
        };
        volume.validate()?;
        Ok(volume)
    }
    fn validate(&self) -> Result<()> {
        if !self.azimuth_rad.is_finite()
            || !self.elevation_rad.is_finite()
            || self.azimuth_rad <= 0.
            || self.elevation_rad <= 0.
            || !self.minimum_ft.is_finite()
            || !self.maximum_ft.is_finite()
            || self.minimum_ft < 0.
            || self.maximum_ft <= self.minimum_ft
            || self.minimum_relative_ft > self.maximum_relative_ft
        {
            return Err(invalid("unusable sensor volume"));
        }
        Ok(())
    }
    pub fn maximum_nmi(&self) -> f64 {
        self.maximum_ft / FEET_PER_NAUTICAL_MILE
    }
    pub fn minimum_nmi(&self) -> f64 {
        self.minimum_ft / FEET_PER_NAUTICAL_MILE
    }
    /// Geometric admission only. The signature-scaled effective distance is a
    /// separate test so a contact can fail on interference alone.
    pub fn admits_geometry(
        &self,
        azimuth_rad: f64,
        elevation_rad: f64,
        distance_ft: f64,
        relative_altitude_ft: f64,
    ) -> bool {
        azimuth_rad.abs() <= self.azimuth_rad
            && elevation_rad.abs() <= self.elevation_rad
            && distance_ft >= self.minimum_ft
            && relative_altitude_ft >= self.minimum_relative_ft
            && relative_altitude_ft <= self.maximum_relative_ft
    }
}

/// Agent gameplay groupings for notch and jamming resistance. They are not
/// recovered historical classifications and never follow from an airframe year.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Basic,
    Transitional,
    Advanced,
    /// A decoded radar record with no reviewed preset assignment yet (EQ-1,
    /// 2026-11-06). Its profile is loaded so an aircraft batch can review and
    /// switch it on, but it is never enabled in flight.
    Unreviewed,
}
impl Preset {
    /// Assignment by installed radar record, from docs/radar.md. A record with
    /// no assignment decodes as [`Preset::Unreviewed`] rather than borrowing
    /// another aircraft's radar: the profile loads, but nothing enables it.
    pub fn for_record(record: &str) -> Option<Self> {
        match record.to_ascii_uppercase().as_str() {
            "F4BR.SEE" | "MIG21R.SEE" | "MIG27R.SEE" | "F104R.SEE" => Some(Self::Basic),
            "F14R.SEE" | "F4JR.SEE" | "AC130R.SEE" | "AV8R.SEE" | "YAK141R.SEE" | "A7R.SEE"
            | "A10R.SEE" => Some(Self::Transitional),
            "F18R.SEE" | "MIG29R.SEE" | "SU24R.SEE" | "SU27R.SEE" | "F22R.SEE" | "F15R.SEE"
            | "E3R.SEE" | "E2R.SEE" | "MIG31R.SEE" => Some(Self::Advanced),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Basic => "BASIC",
            Self::Transitional => "TRANSITIONAL",
            Self::Advanced => "ADVANCED",
            Self::Unreviewed => "UNREVIEWED",
        }
    }
    pub fn notch(self) -> Notch {
        match self {
            // Basic sets are punished by ground clutter, not also by a notch.
            Self::Basic => Notch {
                enabled: false,
                half_width_fps: 0.,
                centre_factor: 1.,
            },
            Self::Transitional => Notch {
                enabled: true,
                half_width_fps: 100.,
                centre_factor: 0.20,
            },
            Self::Advanced => Notch {
                enabled: true,
                half_width_fps: 60.,
                centre_factor: 0.45,
            },
            // Inert: an unreviewed preset is never in a flying radar channel.
            Self::Unreviewed => Notch {
                enabled: false,
                half_width_fps: 0.,
                centre_factor: 1.,
            },
        }
    }
    pub fn resistance(self) -> Resistance {
        let degrees = |d: f64| d.to_radians();
        match self {
            Self::Basic => Resistance {
                burn_through_nmi: 5.,
                coupling_rad: degrees(3.),
                sidelobe_floor: 0.05,
            },
            Self::Transitional => Resistance {
                burn_through_nmi: 8.,
                coupling_rad: degrees(2.),
                sidelobe_floor: 0.02,
            },
            Self::Advanced => Resistance {
                burn_through_nmi: 10.,
                coupling_rad: degrees(1.),
                sidelobe_floor: 0.005,
            },
            // Inert: an unreviewed preset is never in a flying radar channel.
            Self::Unreviewed => Resistance {
                burn_through_nmi: 0.,
                coupling_rad: std::f64::consts::PI,
                sidelobe_floor: 1.,
            },
        }
    }
}

/// Zero-Doppler retail records do not supply these widths. Notching here is a
/// requested departure from the recovered data behaviour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Notch {
    pub enabled: bool,
    pub half_width_fps: f64,
    pub centre_factor: f64,
}

/// Receiver side of the authored jamming model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resistance {
    pub burn_through_nmi: f64,
    pub coupling_rad: f64,
    pub sidelobe_floor: f64,
}

/// Jammer technology generation, an agent design label rather than a claim
/// about any real system or a transition year.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Generation {
    Early,
    Transitional,
    LateColdWar,
    /// A decoded ECM record with no reviewed generation assignment yet (EQ-1,
    /// 2026-11-06). Its profile is loaded so an aircraft batch can review and
    /// switch it on, but it is never enabled in flight.
    Unreviewed,
}
impl Generation {
    /// Explicit reviewable assignment for every ECM record installed on the
    /// reviewed imported aircraft, from docs/radar.md. Never derived from the
    /// associated radar preset or the airframe year. A record with no
    /// assignment decodes as [`Generation::Unreviewed`]: the profile loads, but
    /// nothing enables it.
    pub fn for_record(record: &str) -> Option<Self> {
        match record.to_ascii_uppercase().as_str() {
            "F4.ECM" | "MIG21.ECM" | "AV8.ECM" | "YAK141.ECM" | "KA50.ECM" | "A7.ECM"
            | "F104.ECM" => Some(Self::Early),
            "F14.ECM" | "MIG29.ECM" | "SU24.ECM" | "AC130.ECM" | "B52.ECM" | "A10.ECM" => {
                Some(Self::Transitional)
            }
            "F18.ECM" | "SU27.ECM" | "F22.ECM" | "F15.ECM" | "MIG31.ECM" => Some(Self::LateColdWar),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Early => "EARLY",
            Self::Transitional => "TRANSITIONAL",
            Self::LateColdWar => "LATE",
            Self::Unreviewed => "UNREVIEWED",
        }
    }
    /// Agent-proposed effectiveness of this jammer generation against a radar
    /// preset. No matchup guarantees immunity or a lost lock. An unreviewed
    /// generation has no effect here; it is never in a flying channel.
    pub fn matchup(self, radar: Preset) -> f64 {
        match (self, radar) {
            (Self::Unreviewed, _) | (_, Preset::Unreviewed) => 0.,
            (Self::Early, Preset::Basic) => 1.00,
            (Self::Early, Preset::Transitional) => 0.60,
            (Self::Early, Preset::Advanced) => 0.30,
            (Self::Transitional, Preset::Basic) => 1.30,
            (Self::Transitional, Preset::Transitional) => 1.00,
            (Self::Transitional, Preset::Advanced) => 0.65,
            (Self::LateColdWar, Preset::Basic) => 1.60,
            (Self::LateColdWar, Preset::Transitional) => 1.25,
            (Self::LateColdWar, Preset::Advanced) => 1.00,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RadarProfile {
    pub record: String,
    pub search: Volume,
    pub track: Volume,
    /// Source SEE +10, 0..100. Zero skips the normal look-down penalty.
    pub look_down: f64,
    pub preset: Preset,
    pub notch: Notch,
    pub resistance: Resistance,
    /// Receiver band group. Every reviewed fighter radar shares group 0, which
    /// is why the current jammer pairs are compatible. It is a device-pair
    /// approximation, not a recovered frequency coverage claim.
    pub band: u8,
    /// Preserved source fields whose gameplay meaning is unresolved.
    pub source_flags: [u8; 2],
    pub source_doppler: [u8; 3],
}
impl RadarProfile {
    pub fn from_seeker(record: &str, seeker: &Seeker) -> Result<Self> {
        if seeker.signature != 3 {
            return Err(invalid("sensor record is not a radar channel"));
        }
        let preset = Preset::for_record(record).unwrap_or(Preset::Unreviewed);
        Ok(Self {
            record: record.to_ascii_uppercase(),
            search: Volume::from_zone(&seeker.zones[0])?,
            track: Volume::from_zone(&seeker.zones[1])?,
            look_down: f64::from(seeker.look_down).clamp(0., 100.),
            preset,
            notch: preset.notch(),
            resistance: preset.resistance(),
            band: 0,
            source_flags: seeker.flags,
            source_doppler: [
                seeker.doppler_above,
                seeker.doppler_below,
                seeker.doppler_minimum_range,
            ],
        })
    }
    /// Whether this radar's gameplay preset has been reviewed. A decoded
    /// record with no preset assignment is loaded (EQ-1, 2026-11-06) but never
    /// enabled in flight, so it does not join the flying radar channel.
    pub fn reviewed(&self) -> bool {
        self.preset != Preset::Unreviewed
    }
}

/// A passive channel with no emission, no look-down coefficient and no notch:
/// the installed infrared sensor and the visual sensor share this shape.
#[derive(Clone, Debug, PartialEq)]
pub struct PassiveProfile {
    pub record: String,
    pub search: Volume,
    pub track: Volume,
}
impl PassiveProfile {
    pub fn from_seeker(record: &str, seeker: &Seeker, signature: u8) -> Result<Self> {
        if seeker.signature != signature {
            return Err(invalid("sensor record does not match the expected channel"));
        }
        Ok(Self {
            record: record.to_ascii_uppercase(),
            search: Volume::from_zone(&seeker.zones[0])?,
            track: Volume::from_zone(&seeker.zones[1])?,
        })
    }
}
/// Source signature 2, the installed infrared air-to-air sensor.
pub type InfraredProfile = PassiveProfile;
/// Source signature 0, the visual sensor every imported aircraft carries.
pub type VisualProfile = PassiveProfile;

#[derive(Clone, Debug, PartialEq)]
pub struct JammerProfile {
    pub record: String,
    pub generation: Generation,
    /// Source radar deception chance repurposed as an authored strength input,
    /// not its original probability meaning.
    pub strength: f64,
    pub band: u8,
    /// Only records carrying the radar deception mode emit RF noise here.
    pub radio_frequency: bool,
}
impl JammerProfile {
    pub fn from_countermeasures(record: &str, ecm: &Countermeasures) -> Result<Self> {
        let generation = Generation::for_record(record).unwrap_or(Generation::Unreviewed);
        Ok(Self {
            record: record.to_ascii_uppercase(),
            generation,
            strength: (f64::from(ecm.radar_deception_chance) / 100.).clamp(0., 1.),
            band: 0,
            radio_frequency: ecm.mode_flags & 0x10 != 0,
        })
    }
    /// Whether this jammer's generation has been reviewed. A decoded record
    /// with no generation assignment is loaded (EQ-1, 2026-11-06) but never
    /// enabled in flight, so it does not join the flying jammer channel.
    pub fn reviewed(&self) -> bool {
        self.generation != Generation::Unreviewed
    }
    /// Device-pair band compatibility K, 0 or 1 in this first component.
    pub fn compatible(&self, radar: &RadarProfile) -> f64 {
        if self.radio_frequency && self.band == radar.band {
            1.
        } else {
            0.
        }
    }
}

/// Everything one aircraft contributes to the shared sensor component.
#[derive(Clone, Debug, PartialEq)]
pub struct SensorProfiles {
    pub aircraft: AircraftId,
    pub radar: Option<RadarProfile>,
    pub infrared: Option<InfraredProfile>,
    pub visual: Option<VisualProfile>,
    pub jammer: Option<JammerProfile>,
    /// A decoded radar whose preset is not reviewed yet (EQ-1, 2026-11-06).
    /// The profile is loaded so an aircraft batch can review and switch it on,
    /// but it is never placed in [`Self::radar`], so nothing flies with it.
    pub unreviewed_radar: Option<RadarProfile>,
    /// The ECM counterpart of [`Self::unreviewed_radar`].
    pub unreviewed_jammer: Option<JammerProfile>,
    pub signature: SignatureProfile,
}
impl SensorProfiles {
    /// Resolve sensors by the parsed record channel, never by aircraft name.
    /// A missing optional channel is valid; an invalid record is an error. A
    /// radar or ECM record whose tuning is unreviewed still decodes and loads,
    /// into the `unreviewed_*` fields, and never enables a flying channel
    /// (EQ-1, 2026-11-06).
    pub fn from_source(
        a: &Aircraft,
        mut read: impl FnMut(&str) -> Result<Vec<u8>>,
    ) -> Result<Self> {
        let mut radar = None;
        let mut infrared = None;
        let mut visual = None;
        let mut unreviewed_radar = None;
        for name in a
            .hardpoints
            .iter()
            .filter_map(|h| h.store.as_deref())
            .filter(|n| n.to_ascii_uppercase().ends_with(".SEE"))
        {
            let seeker = Seeker::parse(name, &read(name)?)?;
            match seeker.signature {
                3 if radar.is_none() && unreviewed_radar.is_none() => {
                    let profile = RadarProfile::from_seeker(name, &seeker)?;
                    if profile.reviewed() {
                        radar = Some(profile);
                    } else {
                        unreviewed_radar = Some(profile);
                    }
                }
                2 if infrared.is_none() => {
                    infrared = Some(InfraredProfile::from_seeker(name, &seeker, 2)?)
                }
                0 if visual.is_none() => {
                    visual = Some(VisualProfile::from_seeker(name, &seeker, 0)?)
                }
                // Laser designators and the FLIR as a targeting sight are read
                // by `combat::sight::fixed_sight`.
                _ => {}
            }
        }
        let (jammer, unreviewed_jammer) = match a
            .hardpoints
            .iter()
            .filter_map(|h| h.store.as_deref())
            .find(|n| n.to_ascii_uppercase().ends_with(".ECM"))
        {
            Some(name) => {
                let profile = JammerProfile::from_countermeasures(
                    name,
                    &Countermeasures::parse(name, &read(name)?)?,
                )?;
                if profile.reviewed() {
                    (Some(profile), None)
                } else {
                    (None, Some(profile))
                }
            }
            None => (None, None),
        };
        Ok(Self {
            aircraft: a.id,
            radar,
            infrared,
            visual,
            jammer,
            unreviewed_radar,
            unreviewed_jammer,
            signature: SignatureProfile::from_source(a)?,
        })
    }
    /// The reviewable per-aircraft capability summary. Porting an aircraft
    /// means reviewing this output, not writing another radar controller.
    pub fn summary(&self) -> String {
        let volume = |v: &Volume| {
            let minimum = if v.minimum_ft > 0. {
                format!("{:.0} to ", v.minimum_nmi())
            } else {
                String::new()
            };
            format!(
                "{minimum}{:.0} nmi {:.0}x{:.0} deg",
                v.maximum_nmi(),
                v.azimuth_rad.to_degrees(),
                v.elevation_rad.to_degrees()
            )
        };
        let radar = match (&self.radar, &self.unreviewed_radar) {
            (Some(r), _) => format!(
                "{} search {} track {} look-down {:.0} {}",
                r.record,
                volume(&r.search),
                volume(&r.track),
                r.look_down,
                r.preset.label()
            ),
            (None, Some(r)) => format!("{} UNREVIEWED (not enabled)", r.record),
            (None, None) => "none".into(),
        };
        let passive = |p: &Option<PassiveProfile>| {
            p.as_ref().map_or("none".into(), |i| {
                format!(
                    "{} search {} track {}",
                    i.record,
                    volume(&i.search),
                    volume(&i.track)
                )
            })
        };
        let infrared = passive(&self.infrared);
        let visual = passive(&self.visual);
        let jammer = match (&self.jammer, &self.unreviewed_jammer) {
            (Some(j), _) => format!(
                "{} {} strength {:.2}{}",
                j.record,
                j.generation.label(),
                j.strength,
                if j.radio_frequency {
                    ""
                } else {
                    " (no RF mode)"
                }
            ),
            (None, Some(j)) => format!("{} UNREVIEWED (not enabled)", j.record),
            (None, None) => "none".into(),
        };
        format!(
            "{:?}: radar {radar}; infrared {infrared}; visual {visual}; jammer {jammer}; signature radar {:.0} infrared {:.0}",
            self.aircraft, self.signature.radar, self.signature.infrared
        )
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "profile_checkpoint.rs"]
mod checkpoint;

#[cfg(test)]
mod tests {
    //! EQ-1 (2026-11-06): every decoded SEE or ECM record builds a profile,
    //! and a record whose tuning is unreviewed loads without enabling a flying
    //! channel. The records are the roster ones from docs/radar.md plus the
    //! unported aircraft's, `docs/research/m3-asset-inventory.md`.
    use super::*;
    use tore_formats::aircraft::{Aircraft, Hardpoint, Token};
    use tore_formats::weapons::{Countermeasures, Seeker, Zone};

    /// The `SENSOR` review field order (crates/tore-formats/src/aircraft_schema.rs).
    const SENSOR: &[(&str, &str)] = &[
        ("byte", "structType"),
        ("ptr", "si_names"),
        ("word", "weight"),
        ("byte", "flags"),
        ("byte", "sig"),
        ("byte", "flags[1]"),
        ("byte", "lookDown"),
        ("byte", "dopplerSpeedAbove"),
        ("byte", "dopplerSpeedBelow"),
        ("byte", "dopplerMinRange"),
        ("byte", "allAspect"),
        ("word", "zone0.h"),
        ("word", "zone0.p"),
        ("dword", "zone0.minRange"),
        ("dword", "zone0.maxRange"),
        ("dword", "zone0.minAlt"),
        ("dword", "zone0.maxAlt"),
        ("word", "zone1.h"),
        ("word", "zone1.p"),
        ("dword", "zone1.minRange"),
        ("dword", "zone1.maxRange"),
        ("dword", "zone1.minAlt"),
        ("dword", "zone1.maxAlt"),
        ("byte", "chaffFlareChance"),
        ("byte", "deceptionChance"),
    ];

    /// The `ECM` review field order.
    const ECM: &[(&str, &str)] = &[
        ("byte", "structType"),
        ("ptr", "si_names"),
        ("word", "weight"),
        ("byte", "flags"),
        ("word", "flags[1]"),
        ("byte", "chaffLoaded"),
        ("byte", "chaffChance"),
        ("byte", "chaffH"),
        ("byte", "chaffP"),
        ("byte", "flaresLoaded"),
        ("byte", "flareChance"),
        ("byte", "flareH"),
        ("byte", "flareP"),
        ("byte", "rdChance"),
        ("word", "rdSigAdd"),
        ("byte", "rNoiseMaxDist"),
        ("byte", "rNoiseMinDist"),
        ("byte", "irdChance"),
        ("word", "irdSigAdd"),
        ("byte", "irdLoseLockTime"),
    ];

    fn seeker(signature: u8) -> Seeker {
        let zone = Zone {
            heading: 4000,
            pitch: 4000,
            minimum_range: 0,
            maximum_range: 60_000,
            minimum_altitude: i32::MIN,
            maximum_altitude: i32::MAX,
        };
        Seeker {
            flags: [0; 2],
            signature,
            look_down: 50,
            doppler_above: 0,
            doppler_below: 0,
            doppler_minimum_range: 0,
            all_aspect: 0,
            zones: [zone; 2],
            chaff_flare_chance: 0,
            deception_chance: 0,
        }
    }

    /// A synthetic SEE record in the reader's own field order.
    fn sensor_bytes(record: &str, signature: i64) -> Vec<u8> {
        let mut text = String::from("[brent's_relocatable_format]\n");
        for (kind, name) in SENSOR {
            if *name == "si_names" {
                text.push_str("ptr si_names\n");
                continue;
            }
            let value: i64 = match *name {
                "structType" => 10,
                "weight" => 100,
                "sig" => signature,
                "lookDown" => 50,
                "zone0.h" | "zone1.h" => 4000,
                "zone0.p" | "zone1.p" => 4000,
                "zone0.maxRange" | "zone1.maxRange" => 60_000,
                "zone0.minAlt" | "zone1.minAlt" => -2_000_000_000,
                "zone0.maxAlt" | "zone1.maxAlt" => 2_000_000_000,
                _ => 0,
            };
            text.push_str(&format!("{kind} {value}\n"));
        }
        text.push_str(&format!(
            ":si_names\nstring \"Synthetic\"\nstring \"Synthetic sensor\"\nstring \"{record}\"\nend\n"
        ));
        text.into_bytes()
    }

    /// A synthetic ECM record in the reader's own field order.
    fn ecm_bytes(record: &str) -> Vec<u8> {
        let mut text = String::from("[brent's_relocatable_format]\n");
        for (kind, name) in ECM {
            if *name == "si_names" {
                text.push_str("ptr si_names\n");
                continue;
            }
            let value: i64 = match *name {
                "structType" => 9,
                "weight" => 50,
                "flags[1]" => 0x10,
                "rdChance" => 30,
                _ => 0,
            };
            text.push_str(&format!("{kind} {value}\n"));
        }
        text.push_str(&format!(
            ":si_names\nstring \"Synthetic\"\nstring \"Synthetic ECM\"\nstring \"{record}\"\nend\n"
        ));
        text.into_bytes()
    }

    fn aircraft_with(stores: &[&str]) -> Aircraft {
        let mut a = crate::flight::integration_tests::profile();
        for (key, value) in [("sigs[3]", "100"), ("sigs[2]", "100")] {
            a.object.insert(
                key.into(),
                Token {
                    kind: "word".into(),
                    value: value.into(),
                    scaled: false,
                },
            );
        }
        a.hardpoints = stores
            .iter()
            .map(|store| Hardpoint {
                location: 0,
                flags: 0,
                position: [0; 3],
                store: Some((*store).into()),
                count: 1,
                weight_class: 0,
            })
            .collect();
        a
    }

    #[test]
    fn an_unreviewed_radar_or_ecm_still_decodes_into_a_profile() {
        // A record with no reviewed assignment decodes, with an inert tuning.
        let radar = RadarProfile::from_seeker("A6R.SEE", &seeker(3)).unwrap();
        assert_eq!(radar.preset, Preset::Unreviewed);
        assert!(!radar.reviewed());
        assert_eq!(radar.search.maximum_ft, 60_000.);
        assert_eq!(radar.track.maximum_ft, 60_000.);
        assert!(!radar.notch.enabled);

        let ecm = JammerProfile::from_countermeasures("A6.ECM", &Countermeasures::NONE).unwrap();
        assert_eq!(ecm.generation, Generation::Unreviewed);
        assert!(!ecm.reviewed());

        // A reviewed record keeps its assignment, and the wrong channel fails.
        let reviewed = RadarProfile::from_seeker("F18R.SEE", &seeker(3)).unwrap();
        assert_eq!(reviewed.preset, Preset::Advanced);
        assert!(reviewed.reviewed());
        let reviewed = JammerProfile::from_countermeasures("F18.ECM", &Countermeasures::NONE);
        assert_eq!(reviewed.unwrap().generation, Generation::LateColdWar);
        assert!(RadarProfile::from_seeker("AAS38.SEE", &seeker(2)).is_err());
        // An unreviewed generation has no jamming effect if it is ever read.
        assert_eq!(Generation::Unreviewed.matchup(Preset::Basic), 0.);
        assert_eq!(Generation::Early.matchup(Preset::Unreviewed), 0.);
    }

    #[test]
    fn the_mig31_records_are_reviewed_by_johns_2026_11_08_decision() {
        // M31-7 (John, 2026-11-08): MIG31R.SEE is reviewed as Advanced, the
        // F-15 class of the F15R.SEE record it copies, keeping its own record
        // search range; MIG31.ECM is reviewed as Late-Cold-War, as the
        // byte-identical SU27.ECM. The range itself is the retail record's,
        // read by `Volume::from_zone`, and is confirmed by `--sensor-summary`.
        let radar = RadarProfile::from_seeker("MIG31R.SEE", &seeker(3)).unwrap();
        assert_eq!(radar.preset, Preset::Advanced);
        assert!(radar.reviewed());
        assert_eq!(radar.notch, Preset::Advanced.notch());
        assert_eq!(radar.resistance, Preset::Advanced.resistance());
        // The decoded record's own search maximum is preserved, not replaced by
        // the preset's reference.
        assert_eq!(radar.search.maximum_ft, 60_000.);

        let jammer =
            JammerProfile::from_countermeasures("MIG31.ECM", &Countermeasures::NONE).unwrap();
        assert_eq!(jammer.generation, Generation::LateColdWar);
        assert!(jammer.reviewed());
        // MIG31.ECM is byte-equal to SU27.ECM, so both review the same way.
        let su27 = JammerProfile::from_countermeasures("SU27.ECM", &Countermeasures::NONE).unwrap();
        assert_eq!(jammer.generation, su27.generation);
    }

    #[test]
    fn an_unreviewed_record_loads_without_enabling_a_flying_channel() {
        let a = aircraft_with(&["A6R.SEE", "A6.ECM"]);
        let profiles = SensorProfiles::from_source(&a, |name| {
            Ok(if name.ends_with(".SEE") {
                sensor_bytes(name, 3)
            } else {
                ecm_bytes(name)
            })
        })
        .unwrap();
        // The flying channels stay absent, so the radar and jammer never fly.
        assert!(profiles.radar.is_none());
        assert!(profiles.jammer.is_none());
        // The decoded profiles are loaded and named, with no reviewed tuning.
        assert!(
            profiles
                .unreviewed_radar
                .as_ref()
                .is_some_and(|r| !r.reviewed() && r.record == "A6R.SEE")
        );
        assert!(
            profiles
                .unreviewed_jammer
                .as_ref()
                .is_some_and(|j| !j.reviewed() && j.record == "A6.ECM")
        );
        let summary = profiles.summary();
        assert!(
            summary.contains("A6R.SEE UNREVIEWED (not enabled)"),
            "{summary}"
        );
        assert!(
            summary.contains("A6.ECM UNREVIEWED (not enabled)"),
            "{summary}"
        );
    }

    #[test]
    fn a_reviewed_record_still_enables_its_flying_channel() {
        let a = aircraft_with(&["F18R.SEE", "F18.ECM"]);
        let profiles = SensorProfiles::from_source(&a, |name| {
            Ok(if name.ends_with(".SEE") {
                sensor_bytes(name, 3)
            } else {
                ecm_bytes(name)
            })
        })
        .unwrap();
        assert_eq!(
            profiles.radar.as_ref().map(|r| r.preset),
            Some(Preset::Advanced)
        );
        assert_eq!(
            profiles.jammer.as_ref().map(|j| j.generation),
            Some(Generation::LateColdWar)
        );
        assert!(profiles.unreviewed_radar.is_none());
        assert!(profiles.unreviewed_jammer.is_none());
        assert!(profiles.summary().contains("F18R.SEE"));
    }
}
