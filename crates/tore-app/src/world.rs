//! The mission core: every piece of mutable mission state in one place, so a
//! single type can advance the whole mission one fixed 120 Hz tick at a time.
//! Stage A of the multiplayer plan; see docs/ARCHITECTURE.md, "Mission core
//! and seats".

use crate::{ai_wings, airfield_radio, combat, comms, crew_voice, flight, radio_calls, terrain};

/// The whole mission. The app drives it and presents it.
pub struct World {
    /// Terrain queries, the airport scene and the weather clock. Until the
    /// crate move splits it, it also holds the scenery the renderer draws.
    pub terrain: terrain::Terrain,
    /// The player's flight state.
    pub flight: flight::State,
    /// The player's flight state at the start of the last tick.
    pub previous_flight: flight::State,
    pub combat: combat::Combat,
    pub ai_wings: Option<ai_wings::AiWings>,
    pub airport_service: tore_sim::airport::Service,
    pub airport_nav_mode: bool,
    pub turbulence: tore_sim::turbulence::Turbulence,
    pub turbulence_rng: tore_formats::flight_model::clock_rng::NativeRng,
    /// Radio and crew voice delivery; see docs/spec/radio-chatter.md.
    pub comms: comms::Comms,
    pub airfield_radio: airfield_radio::AirfieldRadio,
    /// Weapon, hit, kill and wing radio calls; see radio_calls.rs.
    pub radio: radio_calls::Radio,
    /// Imported phrase text for composing radio lines.
    pub phrases: comms::Phrases,
    /// The player's crew voice; see docs/spec/cockpit-voice.md.
    pub crew_voice: crew_voice::CrewVoice,
}
