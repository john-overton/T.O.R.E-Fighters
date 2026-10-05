//! The coders of the cockpits section: every human-flown plane's `Cockpit`
//! (its flight through `tore_sim::checkpoint::save_flight`, turbulence and
//! its stream, airport service, NAV mode, message clocks, tower radio, crew
//! voice and result tracker), rebuilt as a list, since handoffs add and
//! remove cockpits.
//!
//! Stage H slice H2 (world shell). A restore replaces the world's cockpits
//! with the ones in the bytes, in plane order, so a world built fresh with no
//! cockpits (an open mission) gets the cockpits the checkpoint holds.

use super::{Cockpit, World};
use crate::seats::PlaneId;
use tore_sim::checkpoint::{
    Checkpoint, CheckpointError, Loader, Saver, invalid, load_flight, save_flight,
};

/// Writes every cockpit, in plane order.
pub(crate) fn save_cockpits(world: &World, s: &mut Saver) -> Result<(), CheckpointError> {
    s.count(world.cockpits.len());
    for cockpit in &world.cockpits {
        // A flight state is coded with its aircraft's identity, which the
        // cockpit does not hold: the plane's ownship does.
        let Some(own) = world.combat.state.ownship(cockpit.plane.0) else {
            return invalid(format!(
                "the cockpit of plane {} has no ownship in combat",
                cockpit.plane.0
            ));
        };
        save_cockpit(cockpit, own.configuration().aircraft, s)?;
    }
    Ok(())
}

/// Replaces the world's cockpits with the ones [`save_cockpits`] wrote.
pub(crate) fn restore_cockpits(
    world: &mut World,
    l: &mut Loader<'_>,
) -> Result<(), CheckpointError> {
    let count = l.count()?;
    let mut cockpits: Vec<Cockpit> = Vec::with_capacity(count);
    for _ in 0..count {
        let cockpit = load_cockpit(l)?;
        // The world keeps its cockpits in plane order, one to a plane, and
        // finds them by binary search.
        if cockpits
            .last()
            .is_some_and(|last| last.plane >= cockpit.plane)
        {
            return invalid("the cockpits are not in plane order");
        }
        cockpits.push(cockpit);
    }
    world.cockpits = cockpits;
    Ok(())
}

fn save_cockpit(
    cockpit: &Cockpit,
    aircraft: tore_formats::aircraft::AircraftId,
    s: &mut Saver,
) -> Result<(), CheckpointError> {
    let Cockpit {
        plane,
        flight,
        // Scratch: `plane::fly` begins every step with
        // `previous.clone_from(flight)`, before anything reads it, so a
        // restored cockpit starts it equal to `flight` (the wire's decoded
        // own plane does the same). Only the picture between two ticks reads
        // it, to blend; the first frame after a restore shows no blend.
        previous_flight: _,
        turbulence,
        turbulence_rng,
        airport_service,
        airport_nav_mode,
        airfield_radio,
        crew_voice,
        result,
        overspeed_message_at,
        edge_message_at,
    } = cockpit;
    plane.save(s, None)?;
    save_flight(s, flight, aircraft)?;
    turbulence.save(s, None)?;
    turbulence_rng.save(s, None)?;
    airport_service.save(s, None)?;
    airport_nav_mode.save(s, None)?;
    airfield_radio.save(s, None)?;
    crew_voice.save(s, None)?;
    result.save(s, None)?;
    overspeed_message_at.save(s, None)?;
    edge_message_at.save(s, None)
}

fn load_cockpit(l: &mut Loader<'_>) -> Result<Cockpit, CheckpointError> {
    let plane = PlaneId::load(l, None)?;
    let (_, flight) = load_flight(l)?;
    Ok(Cockpit {
        plane,
        previous_flight: flight.clone(),
        flight,
        turbulence: Checkpoint::load(l, None)?,
        turbulence_rng: Checkpoint::load(l, None)?,
        airport_service: Checkpoint::load(l, None)?,
        airport_nav_mode: Checkpoint::load(l, None)?,
        airfield_radio: Checkpoint::load(l, None)?,
        crew_voice: Checkpoint::load(l, None)?,
        result: Checkpoint::load(l, None)?,
        overspeed_message_at: Checkpoint::load(l, None)?,
        edge_message_at: Checkpoint::load(l, None)?,
    })
}
