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

    /// Gives the actor a lase duty on ground object `target` (slice ST-L;
    /// the wing order "Lase my target" calls this): it flies an offset turn
    /// of radius max(3 nm, 1.2 turn radii) about the target, lasing it,
    /// until [`LaseUntil`] says stop, the target dies, the actor defends or
    /// [`Self::stop_lase`]. An actor with no ground picture is given one by
    /// ordering it the target; the order stands until the duty ends. Returns
    /// false for an actor that cannot fly it (dead, a dummy).
    pub fn lase_for(&mut self, target: u32, until: strike::LaseUntil) -> bool {
        if self.dummy || !self.alive() {
            return false;
        }
        if !self.strikes() {
            self.set_strike_order(Some(target));
        }
        if !self.strikes() {
            return false;
        }
        self.strike
            .get_or_insert_with(|| Box::new(Strike::new(self.seed)))
            .lase_for(target, until);
        true
    }

    /// Ends a lase duty (the order changed). Returns whether one ended.
    pub fn stop_lase(&mut self) -> bool {
        let Some(target) = self.strike.as_ref().and_then(|s| s.lasing()) else {
            return false;
        };
        let ended = self.strike.as_mut().is_some_and(|s| s.stop_lase());
        if ended
            && self.strike_role() == crate::ai::engagement::StrikeRole::Cap
            && self.strike_order() == Some(target)
        {
            self.set_strike_order(None);
        }
        ended
    }

    /// Records whether the AI turned its cockpit's laser on by hand
    /// ([`Strike::set_laser_set`]).
    pub fn set_laser_set(&mut self, set: bool) {
        if let Some(strike) = self.strike.as_mut() {
            strike.set_laser_set(set);
        }
    }

    /// The no-laze decision's radio call has been made (slice ST-G); true if
    /// there was a call still to make.
    pub fn told_no_laze(&mut self) -> bool {
        self.strike.as_mut().is_some_and(|s| s.told_no_laze())
    }

    /// The object the actor is lasing, if it is.
    pub fn lasing(&self) -> Option<u32> {
        self.strike.as_ref().and_then(|s| s.lasing())
    }

    /// The actor has a ground picture with work to do (the Strike role or a
    /// ground order) and flies.
    pub fn strikes(&self) -> bool {
        self.ground.as_ref().is_some_and(|g| g.active()) && self.alive() && !self.dummy
    }

    /// The striker is done and wants to go home. A no-laze decision not yet
    /// said on the radio or recorded keeps the strike one step longer, so
    /// the host sees it before the strike is dropped (slice ST-G).
    pub(super) fn strike_home(&self) -> bool {
        self.strike
            .as_ref()
            .is_some_and(|s| s.home() && s.no_laze().is_none_or(|call| call.told))
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
        wing: strike::WingFacts,
    ) -> Option<Order> {
        if !self.strikes() {
            if self.strike.is_some() && self.ground.is_none() {
                self.strike = None;
            }
            return None;
        }
        let inputs = inputs?;
        // Weapons held (the hold preset or a hold fire order): no attack
        // passes; the rest of the controller flies (formation, CAP), and the
        // strike starts again from its ingress when the hold lifts (slice
        // ST-T2, opinionated agent).
        if self.controller.weapons_held()
            || self.assignment.stance == super::super::engagement::Stance::WeaponsHold
        {
            if let Some(strike) = self.strike.as_mut() {
                strike.stand_down(tick);
            }
            return None;
        }
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
            wing,
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
        // A flight lead's lase order ends with its duty (the target died, a
        // threat made it defend, the designator is gone): the order onto the
        // target that the duty gave is cleared, so it does not go on to bomb
        // it, and a CAP wingman with nothing else rejoins (ST-INT).
        if let Some(target) = self.lase
            && strike.lasing() != Some(target)
        {
            self.lase = None;
            if self.strike_order() == Some(target) {
                self.set_strike_order(None);
            }
            if !self.strikes() {
                self.strike = None;
                return None;
            }
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

    /// What the other members of actor `index`'s wing are doing about
    /// lasing (slice ST-L): whether a designator and laser carriers fly with
    /// it, and the target a carrier has chosen.
    pub(super) fn strike_wing(&self, index: usize) -> strike::WingFacts {
        let actor = &self.actors[index];
        let mut facts = strike::WingFacts::default();
        if !actor.strikes() {
            return facts;
        }
        let (side, wing) = (actor.identity.side, actor.identity.wing);
        for mate in self
            .actors
            .iter()
            .enumerate()
            .filter(|(at, a)| *at != index && a.identity.side == side && a.identity.wing == wing)
            .filter(|(_, a)| a.strikes())
            .filter_map(|(_, a)| a.strike.as_deref())
        {
            facts.designator |= mate.is_designator();
            facts.carriers |= mate.is_carrier();
            facts.escort = facts.escort.or(mate.needs_spot());
        }
        facts
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
