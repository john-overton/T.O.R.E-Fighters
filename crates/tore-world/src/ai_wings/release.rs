//! The AI's releases of ground stores (slice ST-C; docs/spec/ai-strike.md,
//! "One cockpit, two pilots"): a bomb (a pack whole), a rocket from a pod or
//! a guided air-to-ground weapon leaves through combat's shared release, the
//! one a human's trigger takes ([`live::State::release_strike`]), from the
//! strike cockpit the AI flies the aircraft with. So an AI bomb has a human's
//! pack offsets and scatter, an AI rocket leaves along the nose, and an AI
//! laser bomb, JDAM or AT-2 guides as a human's does. Every other store
//! (missiles with a seeker profile, and anything an aircraft without a
//! combat configuration fires) keeps the AI's own release
//! ([`AiWings::realise`]).
use super::*;
use tore_sim::combat::live::{StrikeRelease, StrikeTarget};

impl AiWings {
    /// One AI launch that is not a gun burst, turned into rounds. Returns
    /// how many it made; the ledger learns each one's target.
    pub(super) fn release(
        &mut self,
        event: &LaunchEvent,
        state: &mut live::State,
        weapon: &tore_formats::weapons::Weapon,
        humans: &[(u32, Vector)],
        ground: &impl Fn(f64, f64) -> f64,
    ) -> u32 {
        let station = usize::from(event.station.0);
        let shared = live::is_strike_store(weapon)
            && self.mission.actor(event.actor).is_some()
            && self
                .configs
                .get(&event.actor)
                .is_some_and(|config| station < config.stations.len());
        let emitted = if shared {
            self.release_strike(event, state, station, humans, ground)
        } else if self.striker_seeker(event, weapon) {
            // A striker's seeker missile at its ground target (ST-T).
            self.release_seeker(event, state, weapon, station, humans)
        } else {
            self.realise(event, &mut state.projectiles, weapon, station, humans, None)
        };
        aim_latest(state, emitted, event.target);
        emitted
    }

    /// The rounds AI `actor`'s `station` holds now, after the weapon
    /// service's debit; an unlimited store answers with the next projectile
    /// number, so its releases still alternate sides.
    fn rounds_after(&self, actor: u32, station: usize) -> u32 {
        let rounds = self.mission.actor(actor).and_then(|actor| {
            actor
                .stations()
                .iter()
                .find(|spec| usize::from(spec.station.0) == station)
                .map(|spec| spec.rounds())
        });
        match rounds {
            Some(tore_sim::ai::weapon_service::Rounds::Finite(rounds)) => rounds,
            _ => self.next_projectile_id & 1,
        }
    }

    /// Where the store AI `actor`'s next release from `station` lets go
    /// hangs now, before the weapon service's debit: what its bomb or
    /// rocket sight predicts from, so the release gate judges the pylon the
    /// store will leave (B3 follow-up). The station's own position for an
    /// aircraft or station with no configuration.
    pub(super) fn next_release_mount(&self, actor: u32, station: usize) -> Option<Vector> {
        let config = self.configs.get(&actor)?;
        let s = config.stations.get(station)?;
        let debit = u32::from(s.weapon.burst.actual_rounds_per_game);
        let rounds = self.mission.actor(actor).and_then(|actor| {
            actor
                .stations()
                .iter()
                .find(|spec| usize::from(spec.station.0) == station)
                .map(|spec| spec.rounds())
        });
        Some(match rounds {
            Some(tore_sim::ai::weapon_service::Rounds::Finite(rounds)) => {
                s.rack.next_release_mount(s.mount, rounds, debit)
            }
            // An unlimited store: the release's projectile number picks the
            // side, as [`Self::rounds_after`] does.
            _ => s.rack.release_mount(
                s.mount,
                tore_sim::combat::hardpoints::units(self.next_projectile_id & 1, debit),
            ),
        })
    }

    /// Where AI `actor`'s next store from `station` leaves, feet right, up
    /// and forward of the aircraft: its side of a paired station (B3,
    /// [`tore_sim::combat::hardpoints::Rack::release_mount`]). `None` for an
    /// aircraft with no combat configuration.
    pub(super) fn release_mount(&self, actor: u32, station: usize) -> Option<Vector> {
        let config = self.configs.get(&actor)?;
        let s = config.stations.get(station)?;
        let units = tore_sim::combat::hardpoints::units(
            self.rounds_after(actor, station),
            u32::from(s.weapon.burst.actual_rounds_per_game),
        );
        Some(s.rack.release_mount(s.mount, units))
    }

    /// A ground store's release through the shared release, from the
    /// actor's strike cockpit: the station becomes its selected and armed
    /// store and the event's target its target (a ground point the AI chose
    /// stays). A release the cockpit's readiness holds back makes nothing
    /// and is counted as dropped.
    fn release_strike(
        &mut self,
        event: &LaunchEvent,
        state: &mut live::State,
        station: usize,
        humans: &[(u32, Vector)],
        ground: &impl Fn(f64, f64) -> f64,
    ) -> u32 {
        let (Some(actor), Some(config)) = (
            self.mission.actor(event.actor),
            self.configs.get(&event.actor),
        ) else {
            self.dropped_launches += event.projectiles;
            return 0;
        };
        let launcher = crate::combat::launcher(actor.flight());
        let side = state
            .targets
            .iter()
            .find(|t| t.id == event.actor)
            .map_or(live::NO_SIDE, |t| t.side);
        let Some(cockpit) = state.ai_cockpit_mut(event.actor, side, config) else {
            self.dropped_launches += event.projectiles;
            return 0;
        };
        if !matches!(cockpit.target, Some(StrikeTarget::Point(_))) {
            cockpit.target = Some(StrikeTarget::Object(event.target));
        }
        let incoming = humans
            .iter()
            .any(|(id, _)| *id == event.target)
            .then_some(event.target);
        let rounds_after = self.rounds_after(event.actor, station);
        let mut emitted = 0;
        for _ in 0..event.projectiles {
            let next = &mut self.next_projectile_id;
            let released = state.release_strike(
                StrikeRelease {
                    aircraft: event.actor,
                    config,
                    station,
                    launcher,
                    incoming,
                    rounds_after,
                },
                || {
                    let id = *next;
                    *next = next.wrapping_add(1);
                    id
                },
                ground,
            );
            match released {
                Ok(ids) => {
                    for id in ids {
                        self.ai_shots.insert(id, event.actor);
                        self.realised_launches += 1;
                        emitted += 1;
                    }
                }
                Err(_) => self.dropped_launches += 1,
            }
        }
        emitted
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::build;
    use super::*;
    use crate::test_support::combat_fixture;
    use tore_sim::ai::weapon_service::{RequestId, StationId};
    use tore_sim::combat::ballistics::ReleaseState;

    /// A Mk 82 six-pack: bomb flag, 0 to 1,466 ft/s, the whole burst a call.
    fn mk82_pack(mut w: tore_formats::weapons::Weapon) -> tore_formats::weapons::Weapon {
        w.source = "MK82.JT".into();
        w.flags = 0x22012;
        w.movement.minimum_speed = 0;
        w.movement.maximum_speed = 1466;
        w.movement.initial_speed = 0;
        w.movement.launch_retard = 100;
        w.burst.actual_rounds_per_game = 1;
        w.burst.game_rounds_in_burst = 6;
        w.burst.projectiles_in_pod = 1;
        w.seeker.signature = 0;
        w
    }

    fn launch(actor: u32, target: u32) -> LaunchEvent {
        LaunchEvent {
            actor,
            station: StationId(0),
            target,
            request_id: RequestId(1),
            projectiles: 1,
        }
    }

    /// An AI bomb leaves through the shared release: the pack whole, each
    /// bomb at its pack offset with a bomb path, from the strike cockpit the
    /// AI now flies with. A missile keeps the AI's own release and makes no
    /// cockpit.
    #[test]
    fn an_ai_bomb_pack_goes_through_the_shared_release_and_a_missile_does_not() {
        let (mut wings, _) = build(None);
        let mut state = combat_fixture(false);
        state.projectiles.clear();
        let id = wings.mission().actors()[1].id();
        let mut config = state.own().configuration().clone();
        let mut missile = config.stations[0].weapon.clone();
        missile.source = "AIM120.JT".into();
        missile.seeker.signature = 3;
        config.stations[0].weapon = mk82_pack(config.stations[0].weapon.clone());
        config.stations[0].mount = [3., -2., 1.];
        wings.configs.insert(id, config.clone());
        let ground = |_: f64, _: f64| 0.;

        assert_eq!(
            wings.release(&launch(id, 3), &mut state, &missile, &[], &ground),
            1
        );
        assert!(state.projectiles[0].guidance.is_some());
        assert!(state.bomb_motion(state.projectiles[0].id).is_none());
        assert!(state.strike_cockpit(id).is_none());
        state.projectiles.clear();

        const TANK: u32 = 0x5000_0001;
        let weapon = config.stations[0].weapon.clone();
        let emitted = wings.release(&launch(id, TANK), &mut state, &weapon, &[], &ground);
        assert_eq!(emitted, 6, "a pack leaves whole");
        let launcher = crate::combat::launcher(wings.mission().actor(id).unwrap().flight());
        for (round, p) in state.projectiles.iter().enumerate() {
            assert_eq!((p.owner, p.weapon.as_ref()), (id, Some(&weapon)));
            assert!(state.bomb_motion(p.id).is_some(), "a bomb path");
            assert_eq!(
                p.position,
                ReleaseState::from_station(&launcher, &config.stations[0], round).position
            );
            assert_eq!(wings.ai_shots.get(&p.id), Some(&id));
        }
        let cockpit = state.strike_cockpit(id).expect("the AI's strike cockpit");
        assert_eq!(
            (cockpit.selected, cockpit.armed, cockpit.target),
            (0, true, Some(StrikeTarget::Object(TANK)))
        );
    }
}
