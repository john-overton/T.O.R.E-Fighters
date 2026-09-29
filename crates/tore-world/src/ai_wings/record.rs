//! Read-only views of the bridge for the mission recorder and the debug
//! panels. Nothing here changes a decision: the accessors only read, and the
//! decoy rolls are a write-only copy of what [`AiWings::step`] decided.
//! Opinionated addition requested by John on 2026-09-26; see
//! docs/REPLAYS.md.

use super::AiWings;
use tore_sim::ai::threat::SeekerClass;

/// One missile's roll against one released chaff bundle or flare.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecoyRoll {
    pub projectile: u32,
    /// The aircraft that released the decoy.
    pub releaser: u32,
    /// Chaff (radar) or flare (infrared).
    pub class: SeekerClass,
    /// The missile's decoy susceptibility, percent.
    pub susceptibility: u8,
    /// The device's effectiveness, percent.
    pub effectiveness: u8,
    /// The draw, with its threshold (susceptibility x effectiveness / 100).
    pub draw: Option<tore_sim::ai::Draw>,
    /// The missile followed the decoy.
    pub decoyed: bool,
}

impl AiWings {
    /// The decoy rolls of the latest step, in the order they were made.
    pub fn decoy_rolls(&self) -> &[DecoyRoll] {
        &self.decoy_rolls
    }

    /// The decoy generator's draws of the latest step.
    #[allow(dead_code)] // Read by the debug panels.
    pub fn decoy_draws(&self) -> &tore_sim::ai::DrawLog {
        self.device_random.log()
    }

    /// The weapon an AI aircraft carries on a station, by its display name.
    pub fn station_weapon(&self, actor: u32, station: u8) -> Option<&str> {
        self.weapons
            .get(&(actor, station))
            .map(|weapon| weapon.name.as_str())
    }
}
