//! Handing an aircraft between the AI and a human, in flight. See
//! docs/ARCHITECTURE.md, "Handoff between the AI and a human".
//!
//! A handoff is a mission command ([`super::MissionCommand::Take`] and
//! `GiveBack`), applied at the start of a tick before any seat's commands, and
//! the two calls here for a host or a test that hands an aircraft over between
//! ticks. Both check everything first and change nothing when they refuse.
//!
//! Single player never hands off, so none of this runs there.

use super::{Cockpit, World};
use crate::{
    WorldResult,
    ai_wings::{self, ActorInsert},
    aircraft_type::AircraftType,
    airfield_radio::AirfieldRadio,
    combat, comms,
    crew_voice::CrewVoice,
    seats::{Pilot, PlaneId, SeatId},
};
use std::sync::Arc;
use tore_sim::combat::live::{self, AiPose, AiStores, Ownship};

/// What building a human-flown aircraft needs that the AI's records do not
/// hold: the aircraft's type and its combat configuration.
struct Kit {
    kind: Arc<AircraftType>,
    config: live::Configuration,
}

impl World {
    /// Whether `seat` can take AI-flown `plane` now. Only a living aircraft
    /// whose pilot is aboard can change hands, and a seat flies one plane at a
    /// time (a seat that does not exist yet joins by taking one).
    pub fn can_take(&self, seat: SeatId, plane: PlaneId) -> WorldResult<()> {
        self.take_kit(seat, plane).map(|_| ())
    }

    /// The checks of [`Self::can_take`], keeping what they found.
    fn take_kit(&self, seat: SeatId, plane: PlaneId) -> WorldResult<Kit> {
        let entry = self
            .roster
            .plane(plane)
            .ok_or_else(|| format!("plane {} is not in the mission", plane.0))?;
        if entry.pilot != Pilot::Ai {
            return Err(format!("plane {} is not flown by the AI", plane.0).into());
        }
        if self
            .roster
            .seat(seat)
            .is_some_and(|seat| seat.plane.is_some())
        {
            return Err(format!("seat {} already flies a plane", seat.0).into());
        }
        let wings = self.ai_wings.as_ref().ok_or("no AI flies this mission")?;
        let actor = wings
            .mission()
            .actor(plane.0)
            .ok_or_else(|| format!("the AI does not fly plane {}", plane.0))?;
        let flight = actor.flight();
        let pilot = &flight.systems.pilot;
        if !actor.alive()
            || flight.crashed
            || flight.escape.is_some()
            || pilot.dead
            || pilot.ejected
        {
            return Err(format!("plane {} is not flying with its pilot aboard", plane.0).into());
        }
        let row = self
            .combat
            .state
            .targets
            .iter()
            .find(|target| target.id == plane.0)
            .ok_or_else(|| format!("plane {} has no combat row", plane.0))?;
        if row.hp <= 0 {
            return Err(format!("plane {} is destroyed", plane.0).into());
        }
        if self.combat.state.ownship(plane.0).is_some() {
            return Err(format!("plane {} already has an ownship", plane.0).into());
        }
        let aircraft = wings
            .slot(plane.0)
            .ok_or_else(|| format!("the AI has no slot for plane {}", plane.0))?
            .aircraft;
        let kind = self
            .combat
            .dummy_types()
            .iter()
            .find(|kind| kind.profile.id == aircraft)
            .cloned()
            .ok_or_else(|| format!("the mission holds no aircraft type for plane {}", plane.0))?;
        // The AI's own record of the aircraft, or the mission's for its type,
        // or a flown one's.
        let config = wings
            .configuration(plane.0)
            .or_else(|| {
                self.combat
                    .dummy_configurations()
                    .iter()
                    .find(|config| config.aircraft == aircraft)
            })
            .or_else(|| {
                self.combat
                    .state
                    .ownships()
                    .iter()
                    .map(Ownship::configuration)
                    .find(|config| config.aircraft == aircraft)
            })
            .cloned()
            .ok_or_else(|| format!("the mission holds no configuration for plane {}", plane.0))?;
        Ok(Kit { kind, config })
    }

    /// A human takes the AI-flown `plane` from `seat`. The aircraft leaves the
    /// AI and its combat row becomes an ownship (same fractions of hit points
    /// under the player's rule, same damage, stores, countermeasures, sensors
    /// and warnings), and its flight state moves to a new cockpit unchanged.
    /// The weapon follows a flight start's rule (the gun if it carries
    /// something, else the first loaded station Guns only allows, else NAV),
    /// the AI's current target is designated if
    /// the aircraft's sensors hold it (agent proposal), and the autopilot is
    /// off. The seat joins the roster if it is new, with its own radio
    /// channel; the cockpit's crew voice, airfield radio and result tracker
    /// are built for the aircraft's type. If the aircraft leads its wing, the
    /// human now leads it, as the AI's leader rules read it from the human
    /// entries of the next step.
    pub fn take_plane(&mut self, seat: SeatId, plane: PlaneId) -> WorldResult<()> {
        let Kit { kind, config } = self.take_kit(seat, plane)?;
        let id = plane.0;
        // Everything that can fail without changing the mission comes first.
        let service = tore_sim::airport::Service::new(&self.terrain.airport_scene)
            .map_err(std::io::Error::other)?;
        let turbulence_rng = tore_formats::flight_model::clock_rng::NativeRng::seeded(1)?;
        let home = self
            .cockpits
            .first()
            .and_then(|cockpit| cockpit.result.home_base());
        let wings = self.ai_wings.as_mut().ok_or("no AI flies this mission")?;
        let target = wings
            .mission()
            .actor(id)
            .and_then(tore_sim::ai::mission::AiActor::current_target);
        let removed = wings
            .remove_actor(id)
            .ok_or_else(|| format!("the AI does not fly plane {id}"))?;
        let index = self
            .combat
            .state
            .targets
            .iter()
            .position(|target| target.id == id)
            .ok_or_else(|| format!("plane {id} has no combat row"))?;
        let built = Ownship::from_ai(
            &self.combat.state.targets[index],
            AiStores {
                stations: &removed.parts.stations,
                dispensers: &removed.parts.dispensers,
                equipment: removed.parts.equipment,
                sensors: removed.parts.sensors.clone(),
                warnings: removed.parts.warnings.clone(),
            },
            config,
            // The AI carries its external stores from the start.
            true,
        );
        let mut ownship = match built {
            Ok(ownship) => ownship,
            Err(error) => {
                // Put the aircraft back exactly as it was.
                wings.insert_actor(ActorInsert::from_removed(removed))?;
                return Err(error.into());
            }
        };
        // The start-up rule of a flight start: the gun if it carries
        // something, else the first loaded station Guns only allows, else NAV.
        combat::apply_startup_weapon_state(
            &mut ownship,
            self.combat.state.cheats.guns_only,
            self.combat.state.cheats.unlimited_ammo,
        );
        let airport_nav_mode = !ownship.armed;
        // Belly wear the AI had not yet turned into a hit point stays owed.
        ownship.set_scrape_carry(wings.take_scrape_carry(id));
        let row = self.combat.state.targets.remove(index);
        if let Err(error) = self
            .combat
            .add_ownship(ownship, kind.contrail_offsets.clone())
        {
            self.combat.state.targets.insert(index, row);
            wings.insert_actor(ActorInsert::from_removed(removed))?;
            return Err(error);
        }
        // From here nothing fails.
        let mut flight = removed.parts.flight;
        flight.autopilot.disengage();
        flight.cheats = self.combat.state.cheats;
        let crew = comms::crew(&kind.profile);
        self.roster
            .take_plane(seat, plane, crew)
            .map_err(std::io::Error::other)?;
        self.comms
            .set_seats(self.roster.seats().iter().map(|seat| seat.id));
        let launcher = combat::launcher(&flight);
        let cockpit = Cockpit {
            plane,
            previous_flight: flight.clone(),
            flight,
            turbulence: Default::default(),
            turbulence_rng,
            airport_service: service,
            airport_nav_mode,
            airfield_radio: AirfieldRadio::for_seat(seat, id),
            crew_voice: CrewVoice::new(&kind.profile).for_seat(seat, id),
            result: ai_wings::outcome::Tracker::new(home),
            overspeed_message_at: None,
            edge_message_at: None,
        };
        let at = self.cockpits.partition_point(|c| c.plane < plane);
        self.cockpits.insert(at, cockpit);
        self.refresh_friendlies();
        if let Some(target) = target {
            // Held only if the aircraft's sensors hold the contact. Not a
            // command of the seat's, so no tape or note records it.
            self.combat
                .state
                .command(id, live::Command::DesignateTarget(target), launcher);
        }
        Ok(())
    }

    /// Whether `seat` can give its plane back to the AI now. Only a living
    /// aircraft whose pilot is aboard can change hands, and the presented
    /// plane, the first ownship, stays with a human: the tick needs one.
    pub fn can_give_back(&self, seat: SeatId) -> WorldResult<()> {
        self.give_back_index(seat).map(|_| ())
    }

    fn give_back_index(&self, seat: SeatId) -> WorldResult<usize> {
        let plane = self
            .roster
            .seat(seat)
            .ok_or_else(|| format!("seat {} is not in the mission", seat.0))?
            .plane
            .ok_or_else(|| format!("seat {} flies no plane", seat.0))?;
        let index = self
            .cockpits
            .iter()
            .position(|cockpit| cockpit.plane == plane)
            .ok_or_else(|| format!("plane {} has no cockpit", plane.0))?;
        let ownship = self
            .combat
            .state
            .ownship(plane.0)
            .ok_or_else(|| format!("plane {} has no ownship", plane.0))?;
        if plane.0 == self.combat.own_id() {
            return Err(format!(
                "plane {} is the first human-flown plane and stays with its human",
                plane.0
            )
            .into());
        }
        if self.ai_wings.is_none() {
            return Err("no AI flies this mission".into());
        }
        let flight = &self.cockpits[index].flight;
        let pilot = &flight.systems.pilot;
        if flight.crashed
            || flight.escape.is_some()
            || pilot.dead
            || pilot.ejected
            || ownship.hp <= 0
        {
            return Err(format!("plane {} is not flying with its pilot aboard", plane.0).into());
        }
        Ok(index)
    }

    /// The human in `seat` gives its plane back: leaving, dropping or being
    /// kicked. The ownship becomes a combat row with the same fractions of hit
    /// points under the AI's rule; its stores, dispensers, sensors and
    /// warnings go to a new AI actor built as at mission start (same slot and
    /// seed rule, fresh awareness, neutral, the wing's skill, no orders), in
    /// id order. A wingman rejoins its leader and a leader follows the flight
    /// plan. The flight state stays as it is. The cockpit goes and the seat
    /// waits with no plane.
    pub fn give_back_plane(&mut self, seat: SeatId) -> WorldResult<()> {
        let index = self.give_back_index(seat)?;
        let plane = self.cockpits[index].plane;
        let id = plane.0;
        let slot = self
            .roster
            .plane(plane)
            .ok_or_else(|| format!("plane {id} is not in the mission"))?
            .slot;
        let wings = self.ai_wings.as_mut().ok_or("no AI flies this mission")?;
        let experience = wings
            .experience_for(id, slot.wing.side, slot.wing.index)
            .ok_or_else(|| format!("the AI has no skill to give plane {id}"))?;
        let ownship = self
            .combat
            .state
            .ownship(id)
            .ok_or_else(|| format!("plane {id} has no ownship"))?
            .clone();
        let flight = &self.cockpits[index].flight;
        let carry = ownship.scrape_carry();
        let handback = ownship.into_ai(AiPose {
            launcher: combat::launcher(flight),
            on_ground: flight.research.as_ref().is_some_and(|r| r.on_ground),
        });
        let mut stations = ai_wings::station_specs(&handback.configuration, false);
        for (spec, store) in stations.iter_mut().zip(&handback.stations) {
            spec.store = *store;
        }
        let mut ai_flight = flight.clone();
        // Cheats are the human's settings; an AI aircraft carries none.
        ai_flight.cheats = Default::default();
        // The one step that can fail comes before any change.
        wings.insert_actor(ActorInsert {
            id,
            side: slot.wing.side,
            wing: slot.wing.index,
            member: slot.member,
            aircraft: handback.configuration.aircraft,
            experience,
            flight: ai_flight,
            sensors: Some(handback.sensors),
            stations,
            dispensers: handback.dispensers,
            warnings: Some(handback.warnings),
            equipment: handback.equipment,
            config: Some(handback.configuration),
        })?;
        // From here nothing fails.
        if let Some(wings) = self.ai_wings.as_mut() {
            wings.set_scrape_carry(id, carry);
        }
        self.cockpits.remove(index);
        self.combat.remove_ownship(id);
        let row = handback.row;
        let at = self
            .combat
            .state
            .targets
            .partition_point(|target| target.id < row.id);
        self.combat.state.targets.insert(at, row);
        self.roster.release_plane(seat);
        self.refresh_friendlies();
        Ok(())
    }
}
