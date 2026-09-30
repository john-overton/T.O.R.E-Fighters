//! One step for a human-flown plane: the parts of the mission tick that act
//! on one cockpit's flight, as functions that read only what they are given.
//! See docs/ARCHITECTURE.md, "One step for a human's plane".
//!
//! [`World::step`](super::World::step) calls them for every cockpit, in the
//! tick's order: [`fly`] for each plane, then the weather clock, then
//! [`after_weather`]; combat's step calls [`take_system_hits`] and
//! [`take_combat`] with the plane's [`OwnshipTerms`]; then the tick's events
//! go through [`take_event`]. A network client calls the same functions for
//! its one plane without a `World`, `Combat` or `AiWings`, through
//! [`OwnPlane::step`], which runs them in that order.
//!
//! Everything a step reads that is not the plane's own state is an argument:
//! the terrain, the ground objects still standing, the weather clock's reading
//! at the tick, the plane's ownship terms and its combat configuration, and the
//! tick's combat events.
//!
//! [`ExactState`] is what the step reads and writes of the plane itself,
//! coded bit for bit against an optional baseline, with the own state hash a
//! snapshot carries.

use crate::{WorldResult, terrain::Terrain};
use tore_codec::{BitReader, BitWriter, CodecError};
use tore_formats::flight_model::clock_rng::NativeRng;
use tore_sim::combat::live::{self, DAMAGE_SECTIONS, DamageSection, Event};
use tore_sim::flight::exact::{Exact, ExactError};
use tore_sim::models::FlightModel;
use tore_sim::{flight, turbulence::Turbulence};

/// The weather clock's reading at a tick: what turbulence reads of it. The
/// reading follows from the host tick and the mission's start time, so a
/// client that does not step a weather clock can still supply it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeatherReading {
    /// Native ticks since launch, 256 a second.
    pub ticks: i64,
    /// The time of day, seconds.
    pub seconds_of_day: i32,
}

impl WeatherReading {
    /// The reading of a weather clock as it stands.
    pub fn of(weather: &tore_sim::environment::Environment) -> Self {
        Self {
            ticks: weather.ticks(),
            seconds_of_day: weather.seconds_of_day(),
        }
    }
}

/// Everything combat's write-back reads from a plane's ownship that can change
/// during the mission: its subsystem hit counts and failures, the weight of its
/// stores, whether a release holds the bay open, and its damage. What the
/// loadout fixes (the damage capacity, external tanks, stations and the
/// hardpoints of each sensor) comes from the ownship's configuration beside it.
/// Plain data, so a host can send it and a client can keep it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnshipTerms {
    /// How often each of the 45 subsystems has been hit. The flight takes one
    /// systems hit for every count above its own.
    pub subsystem_counts: [u8; 45],
    pub radar_failed: bool,
    pub visual_failed: bool,
    pub infrared_failed: bool,
    pub ecm_failed: bool,
    /// The stores' weight, pounds, external fuel included.
    pub payload_lbs: f64,
    /// A release is waiting on the bay doors or holding them open.
    pub bay_demand: bool,
    pub hp: i32,
    /// The structural section that broke up, if one did.
    pub damage_section: Option<DamageSection>,
    /// Damage taken in each section, whole points.
    pub damage_amounts: [i32; DAMAGE_SECTIONS],
}

impl OwnshipTerms {
    /// The terms of `own` at combat tick `tick`.
    pub fn of(own: &live::Ownship, tick: u64) -> Self {
        Self {
            subsystem_counts: own.subsystem_counts,
            radar_failed: own.radar_failed,
            visual_failed: own.visual_failed,
            infrared_failed: own.infrared_failed,
            ecm_failed: own.ecm_failed,
            payload_lbs: own.payload_lbs(),
            bay_demand: own.bay_demand(tick),
            hp: own.hp,
            damage_section: own.damage_section(),
            damage_amounts: own.damage_amounts(),
        }
    }
}

/// The plane flies one tick: `previous` keeps where the tick started it, the
/// flight steps over the terrain's surface, and it crashes into, or with no
/// crashes rebounds from, any of the `standing` ground objects it passed
/// through. The native research adapter has no building contact.
pub fn fly(
    previous: &mut flight::State,
    flight: &mut flight::State,
    pilot: &tore_input::PilotInput,
    terrain: &Terrain,
    standing: impl IntoIterator<Item = u32>,
) {
    previous.clone_from(flight);
    flight.step_surface(pilot, |x, z| terrain.surface(x, z));
    if flight.native.is_none()
        && terrain
            .solid_contact(previous.position, flight.position, standing)
            .is_some()
    {
        if flight.cheats.no_crashes {
            flight.rebound(previous.position);
        } else {
            flight.crashed = true;
        }
    }
}

/// What the part of the step after the weather clock tells the plane's seat.
#[derive(Clone, Copy, Debug, Default)]
pub struct Warnings {
    /// The turn-back or OVERSPEED lines, in that order, when due.
    pub messages: [Option<&'static str>; 2],
    /// Turbulence shook the aircraft.
    pub turbulence: Option<tore_input::FeedbackEvent>,
}

/// The part of the step after the weather clock: turbulence from the plane's
/// own state and random stream (unless the plane's cheats turn it off), then
/// the world edge and OVERSPEED with their message clocks.
pub fn after_weather(
    flight: &mut flight::State,
    turbulence: &mut Turbulence,
    turbulence_rng: &mut NativeRng,
    edge_message_at: &mut Option<f64>,
    overspeed_message_at: &mut Option<f64>,
    terrain: &Terrain,
    weather: WeatherReading,
) -> Warnings {
    let enabled = !flight.cheats.no_turbulence;
    let turbulence = step_turbulence(
        turbulence,
        turbulence_rng,
        flight,
        terrain,
        weather,
        enabled,
    );
    let messages = edge_and_overspeed(flight, edge_message_at, overspeed_message_at, terrain);
    Warnings {
        messages,
        turbulence,
    }
}

/// One tick of physical turbulence, applied to attitude and height only.
/// Velocity is untouched: the recovered routine is an angular and vertical
/// perturbation, not a three-dimensional wind field.
pub fn step_turbulence(
    turbulence: &mut Turbulence,
    rng: &mut NativeRng,
    flight: &mut flight::State,
    terrain: &Terrain,
    weather: WeatherReading,
    enabled: bool,
) -> Option<tore_input::FeedbackEvent> {
    // The joined native service explicitly selects the source disabled branch.
    if flight.native.is_some() {
        return None;
    }
    let ground = f64::from(terrain.height(flight.position[0] as f32, flight.position[2] as f32));
    let agl = flight.position[1] - ground;
    let conditions = tore_sim::turbulence::Conditions {
        agl_feet: agl,
        on_ground: flight.crashed || flight.research.as_ref().is_some_and(|r| r.on_ground),
        speed_fps: flight.speed,
        seconds_of_day: weather.seconds_of_day,
        percent: flight.model().configuration().turbulence_percent,
        daytime_ground: terrain.turbulence_reduced_surface(flight.position[0], flight.position[2]),
        enabled,
        nearby_strength: 0,
    };
    let d = turbulence.step(weather.ticks, 2, conditions, rng).ok()?;
    if d == tore_sim::turbulence::Disturbance::default() {
        return None;
    }
    flight.apply_turbulence(d);
    d.shake.then(|| tore_input::FeedbackEvent::Turbulence {
        intensity: d.severity(),
    })
}

/// Beyond the map: a turn-back warning every ten seconds from 100 nautical
/// miles out, and the aircraft is lost at 105. Past the top speed: a short
/// cockpit message, repeated every four seconds. Both requested by John,
/// 2026-09-29; see docs/spec/world-edge.md and docs/spec/overspeed.md. Each
/// human-flown plane has its own warnings and its own clocks. Returns the
/// messages due, the edge's first.
pub fn edge_and_overspeed(
    flight: &mut flight::State,
    edge_message_at: &mut Option<f64>,
    overspeed_message_at: &mut Option<f64>,
    terrain: &Terrain,
) -> [Option<&'static str>; 2] {
    let mut messages = [None; 2];
    if !flight.crashed {
        let [x, _, z] = flight.position;
        let out_nm = terrain.edge_distance_nm(x, z);
        if out_nm >= crate::terrain::EDGE_DESTROY_NM {
            flight
                .systems
                .destroy(tore_sim::aircraft_systems::LossCause::OutOfBounds);
            flight.crashed = true;
        } else if out_nm >= crate::terrain::EDGE_WARNING_NM {
            let now = flight.ticks as f64 * flight::DT;
            if edge_message_at.is_none_or(|at| now - at >= 10. || now < at) {
                *edge_message_at = Some(now);
                messages[0] = Some("You have left the theater: turn back now");
            }
        } else {
            *edge_message_at = None;
        }
    }
    if !flight.crashed
        && flight
            .overspeed_ratio()
            .is_some_and(|r| r >= flight::OVERSPEED_SHAKE_FULL)
    {
        let now = flight.ticks as f64 * flight::DT;
        if overspeed_message_at.is_none_or(|at| now - at >= 4. || now < at) {
            *overspeed_message_at = Some(now);
            messages[1] = Some("OVERSPEED");
        }
    }
    messages
}

/// Combat's first write-back, once combat has stepped: the flight takes one
/// systems hit for every subsystem hit its ownship counts beyond its own, with
/// the notice for a damaged hardpoint, and crashes when its systems are fatal.
pub fn take_system_hits(
    flight: &mut flight::State,
    terms: &OwnshipTerms,
    config: &live::Configuration,
) {
    for index in 0..45 {
        while flight.systems.counts[index] < terms.subsystem_counts[index] {
            flight.systems.hit(index, flight.throttle);
            if let Some(hardpoint) = index.checked_sub(36) {
                if config.external_fuel_lbs[hardpoint] > 0. {
                    flight.systems.fuel.external[hardpoint] = 0.;
                    flight.systems.notify(format!(
                        "External fuel tank {} damaged: fuel lost",
                        hardpoint + 1
                    ));
                } else if let Some(Some(slot)) = config.hardpoint_slots.get(hardpoint) {
                    flight.systems.notify(format!(
                        "{} station damaged",
                        config.stations[*slot].weapon.hud_name
                    ));
                } else {
                    flight.systems.notify(
                        if terms.radar_failed && hardpoint == config.radar_hardpoint {
                            "Radar failed"
                        } else if terms.visual_failed && hardpoint == config.visual_hardpoint {
                            "Visual sensor failed"
                        } else if terms.infrared_failed
                            && Some(hardpoint) == config.infrared_hardpoint
                        {
                            "Infrared sensor failed"
                        } else if Some(hardpoint) == config.rwr_hardpoint {
                            "RWR failed"
                        } else if hardpoint == config.ecm_hardpoint {
                            "Countermeasure equipment damaged"
                        } else {
                            "Hardpoint equipment damaged"
                        },
                    );
                }
            }
        }
    }
    if flight.systems.fatal() {
        flight.crashed = true;
    }
}

/// Combat's second write-back: the payload less the external fuel burned, the
/// bay held open for a release, radar and jammer forced off by failures, the
/// damage figures with the impact report on a damage event, and the crash and
/// the pilot's death at no hit points or on a cockpit hit. `events` are the
/// tick's combat events; only those about `plane` count.
pub fn take_combat(
    flight: &mut flight::State,
    plane: u32,
    terms: &OwnshipTerms,
    config: &live::Configuration,
    events: &[Event],
) -> WorldResult<()> {
    flight.set_payload((terms.payload_lbs - flight.systems.used_external_lbs()).max(0.))?;
    // The bays stay shut until a release asks for them.
    flight.bay_auto_open = flight.bay_available() && terms.bay_demand;
    if terms.radar_failed {
        flight.radar = false;
    }
    if terms.ecm_failed {
        flight.jammer = false;
    }
    flight.damage_fraction =
        (1. - f64::from(terms.hp) / f64::from(config.damage_capacity)).clamp(0., 1.);
    if events
        .iter()
        .any(|e| matches!(e, Event::OwnshipDamaged { aircraft, .. } if *aircraft == plane))
    {
        flight
            .systems
            .report_impact(flight.ticks, flight.damage_fraction);
    }
    flight.damage_variant = terms.damage_section.map(|section| section as usize);
    flight.damage_regions = live::LocalizedDamage {
        amounts: terms.damage_amounts,
        ..Default::default()
    }
    .fractions(config.damage_capacity);
    if terms.hp == 0 {
        flight.crashed = true;
        if matches!(
            terms.damage_section,
            Some(DamageSection::Nose | DamageSection::Cockpit)
        ) {
            flight
                .systems
                .kill_pilot("Pilot killed: nose or cockpit lost");
        }
    }
    if events.contains(&Event::PilotKilled { aircraft: plane }) {
        flight.systems.kill_pilot("Pilot killed by cockpit hit");
    }
    Ok(())
}

/// A combat event of the tick that acts on the plane after combat: a missile
/// blast's jolt, and the ownship's destruction. Events about other planes
/// change nothing.
pub fn take_event(flight: &mut flight::State, plane: u32, event: &Event) {
    match event {
        Event::Jolt(jolt) if jolt.target == plane => flight.jolt_from(jolt.from, jolt.strength),
        Event::OwnshipDestroyed { aircraft } if *aircraft == plane => flight.crashed = true,
        _ => {}
    }
}

/// A human-flown plane's own state as the shared step moves it: what a
/// [`Cockpit`](super::Cockpit) keeps of it, with no `World` around it.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnPlane {
    pub plane: u32,
    pub flight: flight::State,
    /// The flight state at the start of the last tick.
    pub previous_flight: flight::State,
    pub turbulence: Turbulence,
    pub turbulence_rng: NativeRng,
    pub edge_message_at: Option<f64>,
    pub overspeed_message_at: Option<f64>,
}

/// What one tick of [`OwnPlane::step`] reads besides the plane and the
/// mission's fixed data.
pub struct PlaneTick<'a> {
    /// The seat's sensor controls, which the tick's command phase gives the
    /// flight before it steps.
    pub sensors: tore_sim::sensors::Controls,
    pub pilot: &'a tore_input::PilotInput,
    /// The ground objects still standing.
    pub standing: &'a [u32],
    /// The weather clock's reading after it stepped this tick.
    pub weather: WeatherReading,
    /// The plane's ownship terms as combat left them this tick, or `None`
    /// when combat keeps no ownship for it.
    pub terms: Option<&'a OwnshipTerms>,
    /// The tick's combat events; those about other planes are ignored.
    pub events: &'a [Event],
}

impl OwnPlane {
    /// One tick for the plane alone, in the order [`World::step`](super::World::step)
    /// runs it: the seat's sensor controls, fly, turbulence and warnings,
    /// combat's two write-backs, then the tick's events. The command phase's
    /// rarer flight writes, a mission's new cheats and the payload after a
    /// range command, are not part of it. `config` is the plane's ownship configuration. The
    /// systems messages stay queued on the flight for the caller to drain.
    pub fn step(
        &mut self,
        tick: &PlaneTick<'_>,
        terrain: &Terrain,
        config: &live::Configuration,
    ) -> WorldResult<Warnings> {
        self.flight.sensors = tick.sensors;
        fly(
            &mut self.previous_flight,
            &mut self.flight,
            tick.pilot,
            terrain,
            tick.standing.iter().copied(),
        );
        let warnings = after_weather(
            &mut self.flight,
            &mut self.turbulence,
            &mut self.turbulence_rng,
            &mut self.edge_message_at,
            &mut self.overspeed_message_at,
            terrain,
            tick.weather,
        );
        if let Some(terms) = tick.terms {
            take_system_hits(&mut self.flight, terms, config);
            take_combat(&mut self.flight, self.plane, terms, config, tick.events)?;
        }
        for event in tick.events {
            take_event(&mut self.flight, self.plane, event);
        }
        Ok(warnings)
    }
}

impl OwnPlane {
    /// A copy of what `cockpit` keeps of its plane's step.
    pub fn of(cockpit: &super::Cockpit) -> Self {
        Self {
            plane: cockpit.plane.0,
            flight: cockpit.flight.clone(),
            previous_flight: cockpit.previous_flight.clone(),
            turbulence: cockpit.turbulence,
            turbulence_rng: cockpit.turbulence_rng.clone(),
            edge_message_at: cockpit.edge_message_at,
            overspeed_message_at: cockpit.overspeed_message_at,
        }
    }
}

impl Exact for OwnshipTerms {
    fn write(&self, w: &mut BitWriter, base: Option<&Self>) -> Result<(), CodecError> {
        let OwnshipTerms {
            subsystem_counts,
            radar_failed,
            visual_failed,
            infrared_failed,
            ecm_failed,
            payload_lbs,
            bay_demand,
            hp,
            damage_section,
            damage_amounts,
        } = self;
        subsystem_counts.write(w, base.map(|b| &b.subsystem_counts))?;
        radar_failed.write(w, base.map(|b| &b.radar_failed))?;
        visual_failed.write(w, base.map(|b| &b.visual_failed))?;
        infrared_failed.write(w, base.map(|b| &b.infrared_failed))?;
        ecm_failed.write(w, base.map(|b| &b.ecm_failed))?;
        payload_lbs.write(w, base.map(|b| &b.payload_lbs))?;
        bay_demand.write(w, base.map(|b| &b.bay_demand))?;
        hp.write(w, base.map(|b| &b.hp))?;
        damage_section.write(w, base.map(|b| &b.damage_section))?;
        damage_amounts.write(w, base.map(|b| &b.damage_amounts))
    }
    fn read(r: &mut BitReader<'_>, base: Option<&Self>) -> Result<Self, CodecError> {
        Ok(OwnshipTerms {
            subsystem_counts: Exact::read(r, base.map(|b| &b.subsystem_counts))?,
            radar_failed: Exact::read(r, base.map(|b| &b.radar_failed))?,
            visual_failed: Exact::read(r, base.map(|b| &b.visual_failed))?,
            infrared_failed: Exact::read(r, base.map(|b| &b.infrared_failed))?,
            ecm_failed: Exact::read(r, base.map(|b| &b.ecm_failed))?,
            payload_lbs: Exact::read(r, base.map(|b| &b.payload_lbs))?,
            bay_demand: Exact::read(r, base.map(|b| &b.bay_demand))?,
            hp: Exact::read(r, base.map(|b| &b.hp))?,
            damage_section: Exact::read(r, base.map(|b| &b.damage_section))?,
            damage_amounts: Exact::read(r, base.map(|b| &b.damage_amounts))?,
        })
    }
}

/// A human-flown plane's **exact state**: everything its step reads and
/// writes that is not the mission's fixed data, coded bit for bit so a copy
/// decoded from it steps exactly as the original (docs/formats/net-protocol.md,
/// "The own aircraft"). The flight at the start of the tick is not part of it:
/// the next step overwrites it before reading it.
#[derive(Clone, Debug, PartialEq)]
pub struct ExactState {
    pub flight: flight::State,
    pub turbulence: Turbulence,
    pub turbulence_rng: NativeRng,
    pub edge_message_at: Option<f64>,
    pub overspeed_message_at: Option<f64>,
    /// The ownship terms the plane last took from combat, or `None` when
    /// combat keeps no ownship for it.
    pub terms: Option<OwnshipTerms>,
}

impl ExactState {
    /// The exact state of `plane` with the `terms` it last took.
    pub fn of(plane: &OwnPlane, terms: Option<&OwnshipTerms>) -> Self {
        Self {
            flight: plane.flight.clone(),
            turbulence: plane.turbulence,
            turbulence_rng: plane.turbulence_rng.clone(),
            edge_message_at: plane.edge_message_at,
            overspeed_message_at: plane.overspeed_message_at,
            terms: terms.copied(),
        }
    }

    /// The plane `plane` restored from this state; the flight at the start
    /// of the tick is the flight itself until the next step.
    pub fn into_own_plane(self, plane: u32) -> (OwnPlane, Option<OwnshipTerms>) {
        let ExactState {
            flight,
            turbulence,
            turbulence_rng,
            edge_message_at,
            overspeed_message_at,
            terms,
        } = self;
        let own = OwnPlane {
            plane,
            previous_flight: flight.clone(),
            flight,
            turbulence,
            turbulence_rng,
            edge_message_at,
            overspeed_message_at,
        };
        (own, terms)
    }

    /// Codes the state against `base`, an earlier exact state of the same
    /// plane the reader also has, or against zero. Refuses the native
    /// research adapter.
    pub fn write(&self, w: &mut BitWriter, base: Option<&Self>) -> Result<(), ExactError> {
        let ExactState {
            flight,
            turbulence,
            turbulence_rng,
            edge_message_at,
            overspeed_message_at,
            terms,
        } = self;
        flight.write_exact(w, base.map(|b| &b.flight))?;
        turbulence.write(w, base.map(|b| &b.turbulence))?;
        turbulence_rng.write(w, base.map(|b| &b.turbulence_rng))?;
        edge_message_at.write(w, base.map(|b| &b.edge_message_at))?;
        overspeed_message_at.write(w, base.map(|b| &b.overspeed_message_at))?;
        terms.write(w, base.map(|b| &b.terms))?;
        Ok(())
    }

    /// Reads a state [`Self::write`] wrote against the same `base`. `model`
    /// is the plane's aircraft type as the import builds it.
    pub fn read(
        r: &mut BitReader<'_>,
        base: Option<&Self>,
        model: &tore_sim::models::AircraftModel,
    ) -> Result<Self, ExactError> {
        Ok(ExactState {
            flight: flight::State::read_exact(r, base.map(|b| &b.flight), model)?,
            turbulence: Exact::read(r, base.map(|b| &b.turbulence))?,
            turbulence_rng: Exact::read(r, base.map(|b| &b.turbulence_rng))?,
            edge_message_at: Exact::read(r, base.map(|b| &b.edge_message_at))?,
            overspeed_message_at: Exact::read(r, base.map(|b| &b.overspeed_message_at))?,
            terms: Exact::read(r, base.map(|b| &b.terms))?,
        })
    }

    /// The state's bytes against `base`, padded to a whole byte.
    pub fn encode(&self, base: Option<&Self>) -> Result<Vec<u8>, ExactError> {
        let mut w = BitWriter::with_capacity(512);
        self.write(&mut w, base)?;
        Ok(w.finish())
    }

    /// The state in `bytes`, which [`Self::encode`] wrote against `base`;
    /// anything but zero padding after it is an error.
    pub fn decode(
        bytes: &[u8],
        base: Option<&Self>,
        model: &tore_sim::models::AircraftModel,
    ) -> Result<Self, ExactError> {
        let mut r = BitReader::new(bytes);
        let state = Self::read(&mut r, base, model)?;
        if !r.only_zero_padding_left() {
            return Err(CodecError::NonCanonical.into());
        }
        Ok(state)
    }

    /// FNV-1a 64 of the state coded with no baseline: the own state hash a
    /// snapshot's header carries.
    pub fn hash(&self) -> Result<u64, ExactError> {
        Ok(tore_codec::fnv1a64(&self.encode(None)?))
    }
}
