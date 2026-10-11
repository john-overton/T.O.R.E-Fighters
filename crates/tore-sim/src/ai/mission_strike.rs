//! The striker's step inside the mission (docs/spec/ai-strike.md, slice
//! ST-T): the strike state ([`super::super::strike::Strike`]) is made the
//! first time a striker with a ground picture is stepped with the host's
//! inputs, and is stepped every mission step before the controller. Its
//! answer goes to the controller ([`Controller::set_strike`]), which flies
//! it below a B47 defence and above everything else, and releases the store
//! it names. An actor without a ground picture never reaches any of this, so
//! air-to-air behaviour is unchanged.
use super::super::awareness::ground::RecentWarnings;
use super::super::controller::OwnState;
use super::super::strike::{self, Context, Inputs, Order, Strike, Takeover};
use super::super::weapon_service::StationId;
use super::{AiActor, AiMission};
use crate::ai::engagement::StrikeRole;
use crate::models::FlightModel;

impl AiActor {
    /// The strike state, while the actor strikes.
    pub fn strike(&self) -> Option<&Strike> {
        self.strike.as_deref()
    }

    /// The host's inputs for the next step (stores, cockpit solution). The
    /// strike state is made here for a striker that has none yet.
    pub fn set_strike_inputs(&mut self, inputs: Inputs) {
        if !self.strikes() {
            return;
        }
        self.strike
            .get_or_insert_with(|| Box::new(Strike::new(self.seed)))
            .set_inputs(inputs);
    }

    /// Forces the pass and the store (the headless driver's coverage runs).
    pub fn force_strike(&mut self, pass: Option<strike::Pass>, station: Option<StationId>) {
        if !self.strikes() {
            return;
        }
        self.strike
            .get_or_insert_with(|| Box::new(Strike::new(self.seed)))
            .force(pass, station);
    }

    /// Continues the attack a human left on this aircraft (slice ST-K): the
    /// strike starts as a takeover of `takeover`. The host gives the actor
    /// its ground order first; returns false, changing nothing, for an actor
    /// that does not strike.
    pub fn take_over_strike(&mut self, takeover: Takeover) -> bool {
        if !self.strikes() {
            return false;
        }
        self.strike = Some(Box::new(Strike::taken_over(self.seed, takeover)));
        true
    }

    /// The actor has a ground picture with work to do (the Strike role or a
    /// ground order) and flies.
    pub fn strikes(&self) -> bool {
        self.ground.as_ref().is_some_and(|g| g.active()) && self.alive() && !self.dummy
    }

    /// The striker is done and wants to go home.
    pub(super) fn strike_home(&self) -> bool {
        self.strike.as_ref().is_some_and(|s| s.home())
    }

    /// The target the striker is attacking, for its wingmates' allowance.
    fn strike_target(&self) -> Option<u32> {
        self.strike
            .as_ref()
            .filter(|s| !s.home())
            .and_then(|s| s.target())
    }

    /// The inputs the host left for this step, taken whatever the step
    /// does, so none outlive it.
    pub(super) fn take_strike_inputs(&mut self) -> Option<Inputs> {
        self.strike.as_mut().and_then(|s| s.take_inputs())
    }

    /// One strike step. `None` when the actor does not strike, or the host
    /// gave it nothing to strike with this step.
    pub(super) fn strike_step(
        &mut self,
        tick: u64,
        own: &OwnState,
        inputs: Option<Inputs>,
        mates: &[u32],
    ) -> Option<Order> {
        if !self.strikes() {
            if self.strike.is_some() && self.ground.is_none() {
                self.strike = None;
            }
            return None;
        }
        let inputs = inputs?;
        let picture = self.ground.as_deref()?;
        let warnings = self.recent_warnings(tick, picture);
        let recovering = self.damage_return.is_some()
            || self.landing_order.is_some()
            || self.bugged_out
            || self.controller.mission_complete()
            || matches!(
                self.controller.fuel_state(),
                Some(
                    super::super::route::FuelState::Bingo
                        | super::super::route::FuelState::Critical
                        | super::super::route::FuelState::OutOfFuel
                )
            );
        let context = Context {
            tick,
            aircraft: self.id(),
            side: self.identity.side,
            own,
            velocity: self.flight.velocity,
            picture,
            inputs: &inputs,
            experience: self.experience().level,
            warnings,
            defending: self.controller.defending(),
            recovering,
            mates,
            ceiling_ft: self.ceiling_ft(),
        };
        let strike = self.strike.as_mut()?;
        let order = strike.step(&context);
        // A takeover with nothing left to attack hands the aircraft back to
        // its ordinary role rather than sending it home (slice ST-K).
        if strike.takeover_done() && !recovering {
            self.strike = None;
            self.set_strike_order(None);
            self.set_strike_role(super::super::engagement::StrikeRole::Cap);
            return None;
        }
        Some(order)
    }

    /// The warnings of the last 10 s from the actor's own missile warnings:
    /// a launch at it counts as air-to-air while a hostile fighter is in its
    /// picture, else as surface-to-air (fitted, agent: the warning carries
    /// no launcher).
    fn recent_warnings(
        &self,
        tick: u64,
        picture: &super::super::awareness::ground::GroundPicture,
    ) -> RecentWarnings {
        let warned = self.missile_threats.records().any(|r| {
            r.targeting_receiver
                && !r.stale
                && tick.saturating_sub(r.observed_tick) < strike::WARNING_TICKS
        });
        let fighters = !picture.threats().fighters.is_empty();
        RecentWarnings {
            air_to_air: warned && fighters,
            surface_to_air: warned && !fighters,
        }
    }

    /// The top of the aircraft's 1 G envelope, feet above sea level
    /// (infinite when the record gives none).
    fn ceiling_ft(&self) -> f64 {
        self.flight
            .model()
            .configuration()
            .aerodynamics
            .envelopes
            .iter()
            .find(|e| e.g == 1)
            .map(|e| e.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max))
            .filter(|top| top.is_finite() && *top > 0.)
            .unwrap_or(f64::INFINITY)
    }
}

impl AiMission {
    /// The strike targets of the other members of actor `index`'s wing.
    pub(super) fn strike_mates(&self, index: usize) -> Vec<u32> {
        let actor = &self.actors[index];
        if !actor.strikes() {
            return Vec::new();
        }
        let (side, wing) = (actor.identity.side, actor.identity.wing);
        self.actors
            .iter()
            .enumerate()
            .filter(|(at, a)| *at != index && a.identity.side == side && a.identity.wing == wing)
            .filter_map(|(_, a)| a.strike_target())
            .collect()
    }

    /// Ends the work of every striker that is done (no target or store
    /// left). A wingman that only had a flight lead's order ends the order
    /// and rejoins its formation; a Strike-role actor goes home with its
    /// wing.
    pub(super) fn strikers_home(&mut self) {
        if !self.actors.iter().any(AiActor::strike_home) {
            return;
        }
        for actor in self
            .actors
            .iter_mut()
            .filter(|a| a.strike_home() && a.strike_role() == StrikeRole::Cap)
        {
            actor.clear_ground_orders();
            actor.strike = None;
        }
        if self.actors.iter().any(AiActor::strike_home) {
            self.send_home(AiActor::strike_home);
        }
    }
}
