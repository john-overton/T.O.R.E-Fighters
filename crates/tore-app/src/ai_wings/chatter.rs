//! What the AI aircraft have to say on the radio, surfaced as events for
//! [`crate::radio_calls`], which decides who hears them and how they are
//! worded. Behaviour: docs/spec/radio-chatter.md. This module only watches
//! the mission; it never changes an AI decision.
use super::*;
use crate::comms::Elevation;
use crate::radio_calls::Release;
use tore_sim::ai::{route::FuelState, wing::WingControl};

/// Most events kept between drains, so a host that never drains stays bounded.
const MAX_EVENTS: usize = 64;
/// A reporting aircraft's contact cooldown, 15 seconds (spec-derived).
const CONTACT_TICKS: u64 = 15 * 120;
/// Accepting an attack order blocks contact reports for 20 seconds
/// (spec-derived).
const ENGAGE_BLOCK_TICKS: u64 = 20 * 120;
/// Group members counted with a contact: within 10,000 ft of it, heading
/// within 45 degrees (spec-derived).
const GROUP_RANGE_FT: f64 = 10_000.;
const GROUP_HEADING_DEG: f64 = 45.;
/// `fitted`: the type is named within 8 statute miles. The original scales
/// this by a visibility percentage whose inputs are unknown; TORE uses 100%.
const IDENTIFY_FT: f64 = 8. * 5280.;
/// `fitted`: " high" or " low" when the contact is at least 2,000 ft above or
/// below the listener. The original threshold is unknown.
const HIGH_LOW_FT: f64 = 2000.;
const FEET_PER_NAUTICAL_MILE: f64 = 6076.12;

/// One radio event from an AI aircraft. `speaker` is its actor id.
#[derive(Clone, Debug, PartialEq)]
pub enum Chatter {
    /// A weapon release.
    Release {
        speaker: u32,
        release: Release,
    },
    /// "SAM launch" or "AAM launch": an accepted opposite-side warning.
    LaunchWarning {
        speaker: u32,
        by_aircraft: bool,
    },
    /// The aircraft was destroyed.
    Death {
        speaker: u32,
        ejection_seat: bool,
    },
    /// The first wingman accepted an attack order.
    Engage {
        speaker: u32,
        aircraft: bool,
    },
    /// The first wingman accepted "protect me".
    Showtime {
        speaker: u32,
    },
    Contact {
        speaker: u32,
        contact: Contact,
    },
    Fuel {
        speaker: u32,
        level: FuelLevel,
    },
}

/// A new aircraft contact, measured from the player, who is the only
/// listener TORE voices.
#[derive(Clone, Debug, PartialEq)]
pub struct Contact {
    /// The target plus its flight members flying with it.
    pub count: u32,
    /// The target's short type name, when close enough to identify.
    pub named: Option<String>,
    pub hour: u32,
    pub elevation: Elevation,
    /// Rounded nautical miles.
    pub miles: u32,
    /// The reporting wingman is under medium or tight formation control.
    pub advise: bool,
}

/// Fuel levels in the order they are announced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FuelLevel {
    Joker = 1,
    Bingo,
    Fumes,
    Out,
}
impl FuelLevel {
    /// TORE's B48 fuel state as the radio level: caution is joker, critical
    /// is fumes (spec-derived; the margins match the spec's table).
    fn of(state: FuelState) -> Option<Self> {
        match state {
            FuelState::Caution => Some(Self::Joker),
            FuelState::Bingo => Some(Self::Bingo),
            FuelState::Critical => Some(Self::Fumes),
            FuelState::OutOfFuel => Some(Self::Out),
            FuelState::Ok | FuelState::NoManagement => None,
        }
    }
}

/// A radio speaker other than the player, for labels and listeners.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub id: u32,
    pub enemy: bool,
    /// Flight number from 0; the player's flight is 0.
    pub flight: u8,
    /// Position in the flight from 0; the player is position 0 of flight 0.
    pub position: u8,
    pub alive: bool,
}

/// What the chatter watch remembers between ticks.
#[derive(Default)]
pub(super) struct Watch {
    alive: BTreeMap<u32, bool>,
    targets: BTreeMap<u32, Option<u32>>,
    /// Per aircraft: the tick its next contact report is allowed, and the
    /// last target it reported.
    contacts: BTreeMap<u32, (u64, Option<u32>)>,
    fuel: BTreeMap<u32, FuelLevel>,
    /// By PT resource name.
    names: BTreeMap<&'static str, String>,
    seats: BTreeMap<&'static str, bool>,
}
impl Watch {
    /// Remember a type's short name and ejection seat (PLANE flags 0x10).
    pub(super) fn learn(&mut self, id: AircraftId, aircraft: &Aircraft) {
        let flags = aircraft
            .fields
            .get("flags")
            .and_then(|t| t.number().ok())
            .unwrap_or(0);
        self.names.insert(id.pt(), aircraft.name.clone());
        self.seats.insert(id.pt(), flags & 0x10 != 0);
    }
}

impl AiWings {
    pub(super) fn chat(&mut self, event: Chatter) {
        if self.chatter.len() == MAX_EVENTS {
            self.chatter.remove(0);
        }
        self.chatter.push(event);
    }

    /// An accepted attack order: the reply, and the 20 second contact block
    /// with the ordered target recorded as already reported.
    pub(super) fn engaged(&mut self, speaker: u32, target: Option<u32>) {
        let aircraft = target.is_none_or(|t| t == PLAYER_ID || self.slot(t).is_some());
        self.chat(Chatter::Engage { speaker, aircraft });
        let until = self.mission.tick() + ENGAGE_BLOCK_TICKS;
        let entry = self.watch.contacts.entry(speaker).or_insert((0, None));
        entry.0 = until;
        if target.is_some() {
            entry.1 = target;
        }
    }

    /// Radio labels and listeners for every AI aircraft. `fitted` flight
    /// numbering: the player's flight is the first (Red), then the other
    /// populated friendly wings, then the enemy wings, each in setup order.
    pub fn radio_members(&self) -> Vec<Member> {
        let mut flights: Vec<(bool, u8)> = vec![(false, 1)];
        for slot in &self.slots {
            let key = (slot.side.is_enemy(), slot.wing_number);
            if !flights.contains(&key) {
                flights.push(key);
            }
        }
        flights.sort_by_key(|(enemy, wing)| (*enemy, *wing));
        self.slots
            .iter()
            .map(|slot| {
                let key = (slot.side.is_enemy(), slot.wing_number);
                Member {
                    id: slot.id,
                    enemy: key.0,
                    flight: flights.iter().position(|f| *f == key).unwrap_or(0) as u8,
                    position: slot.member_number.saturating_sub(1),
                    alive: self.mission.actor(slot.id).is_some_and(AiActor::alive),
                }
            })
            .collect()
    }

    /// Read this tick's mission output and actor states into radio events.
    pub(super) fn observe_chatter(
        &mut self,
        output: &tore_sim::ai::mission::MissionOutput,
        player: &flight::State,
    ) {
        let tick = self.mission.tick();
        let mut events = Vec::new();
        for launch in &output.launches {
            if let Some(weapon) = self.weapons.get(&(launch.actor, launch.station.0)) {
                events.push(Chatter::Release {
                    speaker: launch.actor,
                    release: Release::of(weapon, Some(launch.target)),
                });
            }
        }
        for (speaker, launcher) in &output.launch_calls {
            events.push(Chatter::LaunchWarning {
                speaker: *speaker,
                by_aircraft: *launcher == PLAYER_ID || self.slot(*launcher).is_some(),
            });
        }
        for slot in &self.slots {
            let Some(actor) = self.mission.actor(slot.id) else {
                continue;
            };
            let alive = actor.alive();
            if self.watch.alive.insert(slot.id, alive) == Some(true) && !alive {
                events.push(Chatter::Death {
                    speaker: slot.id,
                    ejection_seat: self.watch.seats.get(slot.aircraft.pt()).copied() != Some(false),
                });
            }
            if !alive || actor.is_dummy() {
                continue;
            }
            if let Some(level) = actor.controller().fuel_state().and_then(FuelLevel::of)
                && self
                    .watch
                    .fuel
                    .get(&slot.id)
                    .is_none_or(|done| level > *done)
            {
                self.watch.fuel.insert(slot.id, level);
                events.push(Chatter::Fuel {
                    speaker: slot.id,
                    level,
                });
            }
            let target = actor.controller().target();
            let previous = self.watch.targets.insert(slot.id, target).flatten();
            let Some(target) = target.filter(|t| Some(*t) != previous) else {
                continue;
            };
            // Only the first two aircraft of a flight report.
            if actor.identity().member > 1 {
                continue;
            }
            let (next, last) = self
                .watch
                .contacts
                .get(&slot.id)
                .copied()
                .unwrap_or((0, None));
            if tick < next || last == Some(target) {
                continue;
            }
            let control = actor.controller().wing_settings().0;
            let advise = actor.identity().member != 0
                && control.is_some_and(|c| c as u8 >= WingControl::Medium as u8);
            let Some(contact) = self.contact(target, player, advise) else {
                continue;
            };
            self.watch
                .contacts
                .insert(slot.id, (tick + CONTACT_TICKS, Some(target)));
            events.push(Chatter::Contact {
                speaker: slot.id,
                contact,
            });
        }
        for event in events {
            self.chat(event);
        }
    }

    /// The contact report for `target`, measured from the player. `None` when
    /// the target is not a living airborne aircraft.
    fn contact(&self, target: u32, player: &flight::State, advise: bool) -> Option<Contact> {
        let flight_of = |f: &flight::State| (f.position, f.yaw.to_degrees());
        let (position, heading, wing, aircraft) = if target == PLAYER_ID {
            if player.crashed {
                return None;
            }
            let (p, h) = flight_of(player);
            (p, h, Some((launch::Side::Friendly, 1)), None)
        } else {
            let slot = self.slot(target)?;
            let actor = self.mission.actor(target).filter(|a| a.alive())?;
            let (p, h) = flight_of(actor.flight());
            (
                p,
                h,
                Some((slot.side, slot.wing_number)),
                Some(slot.aircraft),
            )
        };
        let heading_close = |other: f64| ((other - heading + 180.).rem_euclid(360.) - 180.).abs();
        let together = |p: Vector, h: f64| {
            missiles::length(missiles::sub(p, position)) <= GROUP_RANGE_FT
                && heading_close(h) <= GROUP_HEADING_DEG
        };
        let mut count = 1;
        if let Some((side, wing)) = wing {
            for slot in self
                .slots
                .iter()
                .filter(|s| s.id != target && s.side == side && s.wing_number == wing)
            {
                if let Some(actor) = self.mission.actor(slot.id).filter(|a| a.alive()) {
                    let (p, h) = flight_of(actor.flight());
                    count += u32::from(together(p, h));
                }
            }
            if target != PLAYER_ID && side == launch::Side::Friendly && wing == 1 {
                let (p, h) = flight_of(player);
                count += u32::from(!player.crashed && together(p, h));
            }
        }
        let delta = missiles::sub(position, player.position);
        let distance = missiles::length(delta);
        let bearing = delta[0].atan2(delta[2]).to_degrees() - player.yaw.to_degrees();
        Some(Contact {
            count,
            named: aircraft
                .filter(|_| distance <= IDENTIFY_FT)
                .and_then(|id| self.watch.names.get(id.pt()).cloned()),
            hour: crate::comms::clock_hour(bearing),
            elevation: if delta[1] >= HIGH_LOW_FT {
                Elevation::High
            } else if delta[1] <= -HIGH_LOW_FT {
                Elevation::Low
            } else {
                Elevation::Level
            },
            miles: (distance / FEET_PER_NAUTICAL_MILE).round() as u32,
            advise,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    fn wings() -> AiWings {
        let mut selections = payload(None);
        selections[0].wing.index = 0;
        AiWings::build_with(&selections, &spawned(), 0, |_| Ok((aircraft(), None))).unwrap()
    }

    #[test]
    fn members_are_labelled_by_flight_with_the_player_first() {
        let wings = wings();
        let members = wings.radio_members();
        // Player's flight: Red two and three; the enemy pair is the next flight.
        let summary: Vec<_> = members
            .iter()
            .map(|m| (m.id, m.enemy, m.flight, m.position))
            .collect();
        assert_eq!(
            summary,
            [
                (1, false, 0, 1),
                (2, false, 0, 2),
                (3, true, 1, 0),
                (4, true, 1, 1)
            ]
        );
    }

    #[test]
    fn engage_and_protect_replies_become_events_and_block_contacts() {
        use tore_sim::ai::wing::PlayerOrder;
        let mut wings = wings();
        let report = wings
            .command(PlayerOrder::EngageMyTarget, Some(3), None)
            .unwrap();
        assert_eq!(report.radio, ["^ATTACK"], "the reply is delayed, not here");
        assert_eq!(
            wings.chatter,
            [Chatter::Engage {
                speaker: 1,
                aircraft: true
            }]
        );
        assert_eq!(wings.watch.contacts[&1], (ENGAGE_BLOCK_TICKS, Some(3)));
        wings.chatter.clear();
        wings.command(PlayerOrder::ProtectMe, None, None).unwrap();
        assert_eq!(wings.chatter, [Chatter::Showtime { speaker: 1 }]);
        wings.chatter.clear();
        wings
            .command(PlayerOrder::AttackOnContact, None, None)
            .unwrap();
        assert!(matches!(
            wings.chatter[..],
            [Chatter::Engage {
                speaker: 1,
                aircraft: true
            }]
        ));
    }

    #[test]
    fn contacts_count_the_group_and_measure_from_the_player() {
        let wings = wings();
        let mut player = crate::flight::State::new(&aircraft(), [0., 20000., 0.]).unwrap();
        player.yaw = 0.;
        // Enemy 3 is 40,000 ft ahead with its wingman 1,500 ft away, same heading.
        let c = wings.contact(3, &player, true).unwrap();
        assert_eq!(c.count, 2);
        assert_eq!(c.hour, 12);
        assert_eq!(c.elevation, Elevation::Level);
        assert_eq!(c.miles, 7);
        // 40,000 ft is inside the 42,240 ft identification range.
        assert_eq!(c.named, Some(aircraft().name));
        assert!(c.advise);
        let far = crate::flight::State::new(&aircraft(), [0., 30000., -10000.]).unwrap();
        let c = wings.contact(3, &far, false).unwrap();
        assert_eq!((c.named, c.elevation, c.miles), (None, Elevation::Low, 8));
    }

    #[test]
    fn a_fight_produces_contact_release_and_death_events() {
        use tore_sim::ai::wing::{TargetOrder, WingRequest};
        let mut wings = wings();
        let mut targets = spawned();
        wings
            .mission
            .order(1, WingRequest::TargetAssignment(TargetOrder::FreeSelection))
            .unwrap()
            .unwrap();
        for actor in wings.mission.actors_mut() {
            actor.set_stations(Vec::new());
        }
        let actor = wings.mission.actor_mut(1).unwrap();
        actor.set_stations(simple_stations(1, 20, AI_STORE_SPEED));
        for (station, guided) in [(0, true), (1, false)] {
            let weapon = combat_fixture(guided).configuration().stations[0]
                .weapon
                .clone();
            wings.weapons.insert((1, station), weapon);
        }
        let fixed: Vec<_> = [1, 2, 3, 4]
            .map(|id| {
                let mut flight = wings.mission.actor(id).unwrap().flight().clone();
                flight.position = [
                    if id % 2 == 0 { 50000.0 } else { 0.0 },
                    20000.0,
                    if id >= 3 { 2000.0 } else { 0.0 },
                ];
                (id, flight)
            })
            .into();
        let player = crate::flight::State::new(&aircraft(), [0., 20000., -5000.]).unwrap();
        for _ in 0..3000 {
            for (id, flight) in &fixed {
                *wings.mission.actor_mut(*id).unwrap().flight_mut() = flight.clone();
            }
            let output = wings
                .advance(player_object(player.position), &mut targets, &flat)
                .unwrap();
            wings.observe_chatter(&output, &player);
        }
        let from_red_two = |f: fn(&Chatter) -> bool| {
            wings.chatter.iter().any(|c| {
                f(c) && matches!(
                    c,
                    Chatter::Release { speaker: 1, .. } | Chatter::Contact { speaker: 1, .. }
                )
            })
        };
        assert!(from_red_two(|c| matches!(c, Chatter::Contact { .. })));
        assert!(from_red_two(|c| matches!(c, Chatter::Release { .. })));
        let contacts = wings
            .chatter
            .iter()
            .filter(|c| matches!(c, Chatter::Contact { speaker: 1, .. }))
            .count();
        assert_eq!(contacts, 1, "never the same target twice in a row");
        wings.chatter.clear();
        targets[2].hp = 0;
        let output = wings
            .advance(player_object(player.position), &mut targets, &flat)
            .unwrap();
        wings.observe_chatter(&output, &player);
        // The synthetic profile has no ejection seat flag (PLANE flags 0x10).
        assert_eq!(
            wings.chatter,
            [Chatter::Death {
                speaker: 3,
                ejection_seat: false
            }]
        );
    }

    #[test]
    fn events_are_bounded_when_nobody_drains_them() {
        let mut wings = wings();
        for _ in 0..200 {
            wings.chat(Chatter::Showtime { speaker: 1 });
        }
        assert_eq!(wings.chatter.len(), MAX_EVENTS);
    }
}
