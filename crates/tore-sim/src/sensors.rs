//! The shared aircraft sensor component: one implementation of radar, infrared
//! air-to-air and passive exposure serving every imported aircraft. Aircraft
//! differ only by imported profile parameters, never by separate radar code.
//!
//! Recovered equipment values live in the profiles; the detection, notch,
//! jamming, selection and history rules are authored gameplay recorded in
//! docs/radar.md. Presentation receives observations and interference; it never
//! decides whether a target is detectable.
pub mod detection;
pub mod passive;
pub mod profile;
pub mod signature;
pub mod track;

pub use profile::{
    DEFAULT_RANGE_INDEX, FEET_PER_NAUTICAL_MILE, Generation, InfraredProfile, JammerProfile,
    Preset, RANGE_LADDER_NMI, RadarProfile, SensorProfiles, Volume,
};
pub use signature::{Configuration, SignatureProfile};
pub use track::{
    Channel, Contact, Controls, Environment, Event, Mode, Observable, Observer, Plot, Sample,
    Sensors, Strobe, Support,
};

fn invalid(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod acceptance;
