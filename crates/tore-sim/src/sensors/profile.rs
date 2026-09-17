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
}
impl Preset {
    /// Assignment by installed radar record, from docs/radar.md. An unreviewed
    /// record fails the import rather than borrowing another aircraft's radar.
    pub fn for_record(record: &str) -> Option<Self> {
        match record.to_ascii_uppercase().as_str() {
            "F4BR.SEE" | "MIG21R.SEE" | "MIG27R.SEE" => Some(Self::Basic),
            "F14R.SEE" => Some(Self::Transitional),
            "F18R.SEE" | "MIG29R.SEE" | "SU24R.SEE" | "SU27R.SEE" | "F22R.SEE" => {
                Some(Self::Advanced)
            }
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Basic => "BASIC",
            Self::Transitional => "TRANSITIONAL",
            Self::Advanced => "ADVANCED",
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
}
impl Generation {
    /// Explicit reviewable assignment for every ECM record installed on the
    /// twelve imported aircraft, from docs/radar.md. Never derived from the
    /// associated radar preset or the airframe year.
    pub fn for_record(record: &str) -> Option<Self> {
        match record.to_ascii_uppercase().as_str() {
            "F4.ECM" | "MIG21.ECM" => Some(Self::Early),
            "F14.ECM" | "MIG29.ECM" | "SU24.ECM" => Some(Self::Transitional),
            "F18.ECM" | "SU27.ECM" | "F22.ECM" => Some(Self::LateColdWar),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Early => "EARLY",
            Self::Transitional => "TRANSITIONAL",
            Self::LateColdWar => "LATE",
        }
    }
    /// Agent-proposed effectiveness of this jammer generation against a radar
    /// preset. No matchup guarantees immunity or a lost lock.
    pub fn matchup(self, radar: Preset) -> f64 {
        match (self, radar) {
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
        let preset = Preset::for_record(record).ok_or_else(|| {
            invalid("unreviewed radar record: add an explicit preset assignment first")
        })?;
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
        let generation = Generation::for_record(record).ok_or_else(|| {
            invalid("unreviewed ECM record: add an explicit generation assignment first")
        })?;
        Ok(Self {
            record: record.to_ascii_uppercase(),
            generation,
            strength: (f64::from(ecm.radar_deception_chance) / 100.).clamp(0., 1.),
            band: 0,
            radio_frequency: ecm.mode_flags & 0x10 != 0,
        })
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
    pub signature: SignatureProfile,
}
impl SensorProfiles {
    /// Resolve sensors by the parsed record channel, never by aircraft name.
    /// A missing optional channel is valid; an invalid record is an error.
    pub fn from_source(
        a: &Aircraft,
        mut read: impl FnMut(&str) -> Result<Vec<u8>>,
    ) -> Result<Self> {
        let mut radar = None;
        let mut infrared = None;
        let mut visual = None;
        for name in a
            .hardpoints
            .iter()
            .filter_map(|h| h.store.as_deref())
            .filter(|n| n.to_ascii_uppercase().ends_with(".SEE"))
        {
            let seeker = Seeker::parse(name, &read(name)?)?;
            match seeker.signature {
                3 if radar.is_none() => radar = Some(RadarProfile::from_seeker(name, &seeker)?),
                2 if infrared.is_none() => {
                    infrared = Some(InfraredProfile::from_seeker(name, &seeker, 2)?)
                }
                0 if visual.is_none() => {
                    visual = Some(VisualProfile::from_seeker(name, &seeker, 0)?)
                }
                // Laser designators keep their own channel and are not used here.
                _ => {}
            }
        }
        let jammer = match a
            .hardpoints
            .iter()
            .filter_map(|h| h.store.as_deref())
            .find(|n| n.to_ascii_uppercase().ends_with(".ECM"))
        {
            Some(name) => Some(JammerProfile::from_countermeasures(
                name,
                &Countermeasures::parse(name, &read(name)?)?,
            )?),
            None => None,
        };
        Ok(Self {
            aircraft: a.id,
            radar,
            infrared,
            visual,
            jammer,
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
        let radar = self.radar.as_ref().map_or("none".into(), |r| {
            format!(
                "{} search {} track {} look-down {:.0} {}",
                r.record,
                volume(&r.search),
                volume(&r.track),
                r.look_down,
                r.preset.label()
            )
        });
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
        let jammer = self.jammer.as_ref().map_or("none".into(), |j| {
            format!(
                "{} {} strength {:.2}{}",
                j.record,
                j.generation.label(),
                j.strength,
                if j.radio_frequency {
                    ""
                } else {
                    " (no RF mode)"
                }
            )
        });
        format!(
            "{:?}: radar {radar}; infrared {infrared}; visual {visual}; jammer {jammer}; signature radar {:.0} infrared {:.0}",
            self.aircraft, self.signature.radar, self.signature.infrared
        )
    }
}
