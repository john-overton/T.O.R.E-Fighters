//! Exact conversion between an AI-flown aircraft and a human-flown one, for the
//! handoff (docs/ARCHITECTURE.md, "Handoff between the AI and a human").
//!
//! An AI-flown aircraft keeps its hit points, damage and faults in a combat
//! [`Target`] row and its stores, dispensers, sensors and warnings in the AI
//! actor. A human-flown aircraft keeps all of them in an [`Ownship`]. Both are
//! built from the same [`Configuration`], so nothing is lost either way:
//!
//! - **Hit points** keep their fraction of the aircraft's capacity: the row's
//!   `initial_hp` under the AI rule, the configuration's `damage_capacity`
//!   (twice the hit points, the player's rule) for an ownship. A living
//!   aircraft keeps at least one point, so it never dies in the conversion.
//! - **Damage** (the total taken, which decides the system faults still to
//!   come), the hit-section amounts and the fault counts move with the same
//!   scale. The breakup variant and section are copied.
//! - **Stores**: each station's rounds are the AI store's (an unlimited store
//!   is the station's full count). A station the AI has out of action is one
//!   the ownship has failed, and the other way round.
//! - **Countermeasures**: flares and chaff are the AI's two dispensers.
//! - **Sensors and missile warnings** move over as they are. The AI's
//!   equipment failures are the ownship's failure flags; a jammer the row lost
//!   is `ecm_failed`.
//!
//! What the human set in the cockpit (the weapon and its modes, the sight,
//! the laser, the designation and the A-G page) is not the AI's to fly with:
//! [`Ownship::kept`] puts it aside as a [`Kept`] when the human gives the
//! aircraft back, combat holds it while the AI flies ([`super::State::keep`];
//! the sight and the laser keep working for the weapons the human left
//! guiding), and [`Ownship::restore`] puts it back when a human takes the
//! aircraft again (slice FX-J; docs/ARCHITECTURE.md, "What a returning
//! player keeps"). Its ground-attack part is the aircraft's
//! [`StrikeCockpit`], the same record a human and an AI striker fly with
//! (slice ST-C).

use super::cockpit::{StrikeCockpit, StrikeTarget};
use super::{
    AIRCRAFT_RADIUS_FT, Configuration, Launcher, LocalizedDamage, Ownship, Side, SystemFaults,
    Target,
};
use crate::ai::{
    mission::{EquipmentFaults, StationSpec},
    threat::{DispenserStore, SeekerClass},
    weapon_service::{Rounds, StoreState},
};
use crate::combat::{gunship, sight::SightInput};
use crate::combat::{missiles::TargetRole, missiles::seeker::Heat, threats::ThreatService};
use crate::sensors::Picks;
use crate::sensors::{self, Sensors};
use tore_formats::Result;

/// The rounds bit that marks a station out of action, as
/// `Ownship::damage_systems` sets it.
const FAILED: u16 = 0x8000;
const ROUNDS: u16 = 0x7fff;

/// What the AI keeps of an aircraft apart from its combat row, given up so a
/// human can fly it.
pub struct AiStores<'a> {
    /// Its weapon stations, indexed by configuration station.
    pub stations: &'a [StationSpec],
    /// Its dispensers.
    pub dispensers: &'a [DispenserStore],
    /// Its failed avionics.
    pub equipment: EquipmentFaults,
    /// Its sensors, `None` if it had none.
    pub sensors: Option<Sensors>,
    /// Its missile warnings.
    pub warnings: ThreatService,
}

/// What an aircraft gives back to the AI when its human leaves: everything
/// [`Ownship::into_ai`] turns the ownship into.
pub struct AiHandback {
    /// The aircraft's combat row, on its pose, with its hit points, damage and
    /// faults under the AI rule.
    pub row: Target,
    /// Each configuration station's store, in station order.
    pub stations: Vec<StoreState>,
    /// The two dispensers in the AI's order: flares, then chaff.
    pub dispensers: Vec<DispenserStore>,
    /// Failed avionics.
    pub equipment: EquipmentFaults,
    /// The aircraft's sensors.
    pub sensors: Sensors,
    /// Its missile warnings.
    pub warnings: ThreatService,
    /// The configuration the aircraft was built from.
    pub configuration: Configuration,
}

/// What a human left set in the cockpit of an aircraft it gave back to the
/// AI, kept by combat while the AI flies it and put back when a human takes
/// it again ([`Ownship::kept`], [`Ownship::restore`]). Stores, damage and
/// sensors go to the AI and come back by the conversion above; this is the
/// rest: the strike cockpit (the weapon, its modes, the sight, the laser,
/// the target) and the human's own extras. It reads as its cockpit
/// (`kept.laser` is `kept.cockpit.laser`).
#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    /// The aircraft's side, for the spot its laser keeps while the AI flies.
    pub side: Side,
    /// The strike cockpit as the human left it: the selected station and
    /// armed state, the seeker and bomb sight modes (Ctrl+M's choice
    /// included), the targeting sight (its look, what it tracks or pins, its
    /// zoom) and the laser (the hand setting, Ctrl+L, included), with the
    /// designated object as its target. The sight and the laser are stepped
    /// while the AI flies, so a track follows its object and a laser lases
    /// for the weapons the human left guiding.
    pub cockpit: StrikeCockpit,
    /// The stations that held something this mission: an emptied or
    /// jettisoned pod keeps or loses its row as the human left it.
    pub ever_loaded: Vec<bool>,
    /// The AC-130's gunsight and gun group, as left.
    pub gunship: Option<gunship::State>,
    /// The designation and the A-G flag.
    pub picks: Picks,
    /// Easy targeting's HUD memory and the views' sight hold.
    pub hud_selection: Option<u32>,
    pub sight_hold: Option<u32>,
    /// Rounds fired, hits and kills.
    pub shots: u32,
    pub hits: u32,
    pub kills: u32,
    /// The radar switch, which the AI flies with on (the flight state holds
    /// it; the host fills it in).
    pub radar: bool,
    /// The airport NAV selected (the cockpit's tower service holds it; the
    /// host fills it in).
    pub airport: Option<u32>,
}

impl Ownship {
    /// What the human set in this cockpit, to keep while the AI flies the
    /// aircraft. The sight and gunsight let go of the slew controls (nobody
    /// holds them now) and drop a key press that was waiting. The flight's
    /// radar switch and NAV airport are the host's to fill in.
    pub fn kept(&self) -> Kept {
        let neutral = |input: SightInput| SightInput {
            deflection: [0; 2],
            ..input
        };
        Kept {
            side: self.side,
            cockpit: StrikeCockpit {
                target: self.designated().map(StrikeTarget::Object),
                sight: self.cockpit.sight.map(|mut sight| {
                    sight.input = neutral(sight.input);
                    sight.request = None;
                    sight.notice = None;
                    sight
                }),
                ..self.cockpit.clone()
            },
            ever_loaded: self.ever_loaded.clone(),
            gunship: self.gunship.clone().map(|mut group| {
                group.input = neutral(group.input);
                group.request = None;
                group.notice = None;
                group
            }),
            picks: self.sensors.picks(),
            hud_selection: self.hud_selection,
            sight_hold: self.sight_hold,
            shots: self.shots,
            hits: self.hits,
            kills: self.kills,
            radar: true,
            airport: None,
        }
    }

    /// Puts back what a human left set in this cockpit, on an ownship just
    /// built from the AI's aircraft ([`Self::from_ai`]). Each part is checked
    /// against the aircraft as it is now: a selected station that ran dry
    /// while the AI flew advances as an emptied one does, Guns only moves a
    /// missile selection to the gun, and a sight or gunsight comes back only
    /// on the aircraft that has the same one. The radar switch and the NAV
    /// airport are the host's to put back.
    pub fn restore(&mut self, kept: &Kept, guns_only: bool, unlimited_ammo: bool) {
        if kept.ever_loaded.len() == self.ever_loaded.len() {
            self.ever_loaded.clone_from(&kept.ever_loaded);
        }
        let left = &kept.cockpit;
        self.cockpit.armed = left.armed && !self.config.stations.is_empty();
        if left.selected < self.config.stations.len() {
            self.cockpit.selected = left.selected;
        }
        self.cockpit.launch_mode = left.launch_mode;
        self.cockpit.delivery_mode = left.delivery_mode;
        if let (Some(group), Some(left)) = (&mut self.gunship, &kept.gunship)
            && group.stations == left.stations
        {
            *group = left.clone();
        }
        if let (Some(sight), Some(left)) = (&mut self.cockpit.sight, left.sight)
            && sight.sensor == left.sensor
        {
            *sight = left;
        }
        if self.config.sight_sensor().is_some_and(|s| s.laser) {
            self.cockpit.laser = left.laser;
        }
        self.sensors.restore_picks(kept.picks);
        self.sync_target();
        self.hud_selection = kept.hud_selection;
        self.sight_hold = kept.sight_hold;
        self.shots = kept.shots;
        self.hits = kept.hits;
        self.kills = kept.kills;
        self.mounted_key = None;
        self.enforce_guns_only(guns_only);
        self.advance_from_empty(guns_only, unlimited_ammo);
    }
}

impl super::State {
    /// Keeps what a human left set in the cockpit of `aircraft`, which the
    /// AI flies from now on, until a human takes it again. Combat steps its
    /// sight and laser meanwhile, while the aircraft's row stands.
    pub fn keep(&mut self, aircraft: u32, kept: Kept) {
        self.kept.insert(aircraft, kept);
    }
    /// What a human left set in the cockpit of `aircraft`, while the AI
    /// flies it.
    pub fn kept(&self, aircraft: u32) -> Option<&Kept> {
        self.kept.get(&aircraft)
    }
    /// Every aircraft the AI flies with a human's cockpit kept, in aircraft
    /// order, for the replay to follow their lasers and designations.
    pub fn kept_cockpits(&self) -> impl Iterator<Item = (u32, &Kept)> {
        self.kept.iter().map(|(aircraft, kept)| (*aircraft, kept))
    }
    /// Takes what a human left set in `aircraft`'s cockpit, for the human
    /// who takes it now.
    pub fn take_kept(&mut self, aircraft: u32) -> Option<Kept> {
        self.kept.remove(&aircraft)
    }
}

/// Where the aircraft is, for the row it becomes.
#[derive(Clone, Copy)]
pub struct AiPose {
    pub launcher: Launcher,
    /// Parked or rolling on a runway, which radar cannot see.
    pub on_ground: bool,
}

/// `value` scaled from a capacity of `from` to one of `to`, rounded to the
/// nearest whole point.
fn scale(value: i32, from: i32, to: i32) -> i32 {
    let (value, from, to) = (
        i64::from(value.max(0)),
        i64::from(from.max(1)),
        i64::from(to),
    );
    ((value * to * 2 + from) / (2 * from)).clamp(0, i64::from(i32::MAX)) as i32
}

/// Hit points scaled the same way, but a living aircraft never rounds to zero.
fn scale_hp(hp: i32, from: i32, to: i32) -> i32 {
    let scaled = scale(hp, from, to);
    if hp > 0 { scaled.max(1).min(to) } else { 0 }
}

fn scaled_damage(damage: &LocalizedDamage, from: i32, to: i32) -> LocalizedDamage {
    let mut scaled = damage.clone();
    scaled.amounts = damage.amounts.map(|amount| scale(amount, from, to));
    scaled
}

impl Ownship {
    /// The human-flown aircraft that the AI-flown one in `row` becomes, with
    /// the same fraction of its hit points under the player's rule, the same
    /// damage and faults, its stores, countermeasures, sensors and warnings.
    /// `external` says whether the aircraft carries external stores, which
    /// decides its payload weight. The gun is not selected here: that is the
    /// cockpit's start, the host's to give.
    pub fn from_ai(
        row: &Target,
        stores: AiStores<'_>,
        config: Configuration,
        external: bool,
    ) -> Result<Self> {
        let capacity = config.damage_capacity;
        let mut own = Self::new(row.id, row.side, config, external)?;
        own.hp = scale_hp(row.hp, row.initial_hp, capacity);
        own.damage = scale(row.faults.damage, row.initial_hp, capacity);
        own.subsystem_counts = row.faults.counts;
        own.localized_damage = scaled_damage(&row.localized_damage, row.initial_hp, capacity);
        own.fragment_released = row.fragment_released;
        let default_ammo = own.ammo.clone();
        for (index, ammo) in own.ammo.iter_mut().enumerate() {
            let Some(spec) = stores
                .stations
                .iter()
                .find(|spec| usize::from(spec.station.0) == index)
            else {
                continue;
            };
            let rounds = match spec.store.rounds {
                Rounds::Finite(n) => n.min(u32::from(ROUNDS)) as u16,
                Rounds::Unlimited => default_ammo[index].min(ROUNDS),
            };
            *ammo = rounds;
            // A station out of action stays so; an empty one has nothing to
            // mark, as a fault on an empty station does not.
            if spec.store.inhibited && rounds > 0 {
                *ammo |= FAILED;
            }
        }
        let count = |class| {
            stores
                .dispensers
                .iter()
                .filter(|d| d.class == class)
                .map(|d| d.count)
                .sum::<u32>()
                .min(u32::from(u8::MAX)) as u8
        };
        own.flares = count(SeekerClass::Infrared);
        own.chaff = count(SeekerClass::Radar);
        own.radar_failed = stores.equipment.radar;
        own.infrared_failed = stores.equipment.infrared;
        own.visual_failed = stores.equipment.visual;
        own.rwr_failed = stores.equipment.rwr;
        // A jammer the row lost is the ownship's failed ECM suite.
        own.ecm_failed = own.config.sensors.jammer.is_some() && row.jammer.is_none();
        if let Some(sensors) = stores.sensors {
            own.sensors = sensors;
        }
        own.missile_threats = stores.warnings;
        Ok(own)
    }

    /// The reverse: the AI-flown aircraft this ownship becomes when its human
    /// gives it back, with the same fractions under the AI's rule. The row is
    /// placed at `pose`; the host puts its stores and dispensers into the new
    /// actor.
    pub fn into_ai(mut self, pose: AiPose) -> AiHandback {
        for pod in &mut self.config.gun_pods {
            if !self.ever_loaded.get(pod.station).copied().unwrap_or(false) {
                pod.quantity = 0;
            }
        }
        self.config
            .refresh_tanks()
            .expect("validated handback equipment");
        let config = &self.config;
        let capacity = config.damage_capacity;
        let initial = config.hit_points;
        let mut faults = SystemFaults::new(config);
        faults.counts = self.subsystem_counts;
        faults.damage = scale(self.damage, capacity, initial);
        let launcher = pose.launcher;
        let row = Target {
            aircraft: Some(config.aircraft),
            role: TargetRole::Aircraft,
            heat: Heat::Engine {
                on: true,
                throttle: 0.7,
                afterburner: false,
            },
            radar_emitting: launcher.radar,
            id: self.aircraft,
            position: launcher.position,
            velocity: launcher.velocity,
            basis: launcher.basis,
            configuration: sensors::Configuration::CLEAN,
            signature: config.sensors.signature,
            jammer: if self.ecm_failed {
                None
            } else {
                config.sensors.jammer.clone()
            },
            jammer_active: false,
            airborne: launcher.alive,
            on_ground: pose.on_ground,
            wreck: None,
            wreck_power: config.wreck_power,
            radius: AIRCRAFT_RADIUS_FT,
            hp: scale_hp(self.hp, capacity, initial),
            initial_hp: initial,
            fragment_offsets: config.fragment_offsets,
            fragment_released: self.fragment_released,
            localized_damage: scaled_damage(&self.localized_damage, capacity, initial),
            faults,
            category: config.target_category,
            side: self.side,
        };
        let stations = self
            .ammo
            .iter()
            .map(|&ammo| StoreState {
                inhibited: ammo & FAILED != 0,
                rounds: Rounds::Finite(u32::from(ammo & ROUNDS)),
            })
            .collect();
        let dispensers = vec![
            DispenserStore {
                class: SeekerClass::Infrared,
                count: u32::from(self.flares),
            },
            DispenserStore {
                class: SeekerClass::Radar,
                count: u32::from(self.chaff),
            },
        ];
        AiHandback {
            row,
            stations,
            dispensers,
            equipment: EquipmentFaults {
                radar: self.radar_failed,
                infrared: self.infrared_failed,
                visual: self.visual_failed,
                rwr: self.rwr_failed,
            },
            sensors: self.sensors,
            warnings: self.missile_threats,
            configuration: self.config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ai::{ScalarSpeed, mission::simple_stations},
        attitude::Basis,
        combat::live::tests::fixture,
    };

    fn config() -> Configuration {
        fixture(true).own().configuration().clone()
    }

    fn launcher() -> Launcher {
        Launcher {
            position: [10., 5000., -20.],
            basis: Basis::new(0.3, 0.1, -0.2),
            speed_fps: 400.,
            velocity: [0., 0., 400.],
            bay_ready: true,
            radar_power: true,
            radar: false,
            jammer: false,
            alive: true,
            body_present: true,
            controls: sensors::Controls::default(),
        }
    }

    /// A damaged AI aircraft of `config`: a row at `hp` of 100, faults, hit
    /// sections, and a store per station.
    fn ai_aircraft(config: &Configuration, hp: i32) -> (Target, Vec<StationSpec>) {
        let mut row = Target {
            aircraft: Some(config.aircraft),
            role: TargetRole::Aircraft,
            heat: Heat::Unknown,
            radar_emitting: false,
            id: 4,
            position: [0.; 3],
            velocity: [0.; 3],
            basis: Basis::new(0., 0., 0.),
            configuration: sensors::Configuration::CLEAN,
            signature: config.sensors.signature,
            jammer: config.sensors.jammer.clone(),
            jammer_active: false,
            airborne: true,
            on_ground: false,
            wreck: None,
            wreck_power: config.wreck_power,
            radius: AIRCRAFT_RADIUS_FT,
            hp,
            initial_hp: 100,
            fragment_offsets: config.fragment_offsets,
            fragment_released: false,
            localized_damage: LocalizedDamage::default(),
            faults: SystemFaults::new(config),
            category: config.target_category,
            side: super::super::Side(2),
        };
        row.localized_damage.amounts = [3, 0, 40, 0, 7, 0];
        row.faults.damage = 100 - hp;
        row.faults.counts[5] = 2;
        row.faults.counts[36] = 1;
        let mut stations = Vec::new();
        for (index, station) in config.stations.iter().enumerate() {
            let mut spec =
                simple_stations(u32::from(station.count), 0, ScalarSpeed(2000.)).remove(0);
            spec.station = crate::ai::weapon_service::StationId(index as u8);
            stations.push(spec);
        }
        (row, stations)
    }

    fn stores(stations: &[StationSpec]) -> AiStores<'_> {
        static DISPENSERS: [DispenserStore; 2] = [
            DispenserStore {
                class: SeekerClass::Infrared,
                count: 17,
            },
            DispenserStore {
                class: SeekerClass::Radar,
                count: 29,
            },
        ];
        AiStores {
            stations,
            dispensers: &DISPENSERS,
            equipment: EquipmentFaults::default(),
            sensors: None,
            warnings: ThreatService::new(4),
        }
    }

    #[test]
    fn an_ai_row_becomes_an_ownship_with_the_same_fractions() {
        let config = config();
        let (row, mut stations) = ai_aircraft(&config, 61);
        stations[0].store.rounds = Rounds::Finite(7);
        let own = Ownship::from_ai(&row, stores(&stations), config.clone(), true).unwrap();
        assert_eq!((own.aircraft, own.side), (4, row.side));
        // 61 of 100 becomes 61 of the capacity of 30, within a point.
        let expected = 0.61 * f64::from(config.damage_capacity);
        assert!((f64::from(own.hp) - expected).abs() <= 1., "{}", own.hp);
        assert_eq!(own.rounds(0), 7);
        assert_eq!((own.flares, own.chaff), (17, 29));
        assert_eq!(own.subsystem_counts[5], 2);
        assert_eq!(own.subsystem_counts[36], 1);
        // Section fractions are the row's.
        let ours = own.damage_regions();
        let theirs = row.localized_damage.fractions(row.initial_hp);
        for (a, b) in ours.iter().zip(theirs) {
            assert!((a - b).abs() < 0.04, "{ours:?} {theirs:?}");
        }
    }

    #[test]
    fn a_round_trip_keeps_rounds_countermeasures_and_hit_point_fractions() {
        let config = config();
        for hp in [100, 61, 37, 12, 1] {
            let (row, mut stations) = ai_aircraft(&config, hp);
            stations[0].store.rounds = Rounds::Finite(9);
            let own = Ownship::from_ai(&row, stores(&stations), config.clone(), true).unwrap();
            let back = own.into_ai(AiPose {
                launcher: launcher(),
                on_ground: false,
            });
            assert_eq!(back.stations.len(), config.stations.len());
            assert_eq!(back.stations[0].rounds, Rounds::Finite(9));
            assert_eq!(
                back.dispensers,
                vec![
                    DispenserStore {
                        class: SeekerClass::Infrared,
                        count: 17
                    },
                    DispenserStore {
                        class: SeekerClass::Radar,
                        count: 29
                    },
                ]
            );
            // The fraction of the hit points: within one point of the row's
            // own capacity, never zero for a living aircraft.
            assert!(back.row.hp >= 1);
            let fraction = f64::from(back.row.hp) / f64::from(back.row.initial_hp);
            assert!(
                (fraction - f64::from(hp) / 100.).abs() < 0.06,
                "{hp}: {fraction}"
            );
            assert_eq!(back.row.faults.counts[5], 2);
            assert_eq!(back.row.faults.counts[36], 1);
            assert_eq!(back.row.id, 4);
            assert_eq!(back.row.position, launcher().position);
        }
    }

    /// An ownship, handed to the AI and taken back, keeps its rounds, chaff and
    /// flares exactly and its hit points within one point, whatever the
    /// capacities of the two rules.
    #[test]
    fn an_ownship_given_back_and_taken_again_keeps_its_stores_and_hit_points() {
        let config = config();
        for hp in 1..=config.damage_capacity {
            let mut own = Ownship::new(4, super::super::Side(2), config.clone(), true).unwrap();
            own.hp = hp;
            own.damage = config.damage_capacity - hp;
            own.ammo[0] = 6 | FAILED;
            own.chaff = 31;
            own.flares = 12;
            own.subsystem_counts[7] = 3;
            let back = own.into_ai(AiPose {
                launcher: launcher(),
                on_ground: false,
            });
            let (_, mut specs) = ai_aircraft(&config, 50);
            for (spec, store) in specs.iter_mut().zip(&back.stations) {
                spec.store = *store;
            }
            let again = Ownship::from_ai(
                &back.row,
                AiStores {
                    stations: &specs,
                    dispensers: &back.dispensers,
                    equipment: back.equipment,
                    sensors: Some(back.sensors),
                    warnings: back.warnings,
                },
                back.configuration,
                true,
            )
            .unwrap();
            assert_eq!(again.ammo[0], 6 | FAILED);
            assert_eq!((again.chaff, again.flares), (31, 12));
            assert_eq!(again.subsystem_counts[7], 3);
            assert!((again.hp - hp).abs() <= 1, "{hp} came back as {}", again.hp);
            assert!(again.hp >= 1);
        }
    }

    #[test]
    fn a_failed_station_stays_failed() {
        let config = config();
        let (row, mut stations) = ai_aircraft(&config, 80);
        stations[0].store.inhibited = true;
        let own = Ownship::from_ai(&row, stores(&stations), config, true).unwrap();
        assert_eq!(own.ammo[0] & FAILED, FAILED);
        assert_eq!(own.rounds(0), 11);
        let back = own.into_ai(AiPose {
            launcher: launcher(),
            on_ground: false,
        });
        assert!(back.stations[0].inhibited);
        assert_eq!(back.stations[0].rounds, Rounds::Finite(11));
    }

    #[test]
    fn failed_avionics_and_a_lost_jammer_carry_over() {
        let config = config();
        let (mut row, stations) = ai_aircraft(&config, 80);
        row.jammer = None;
        let mut ai = stores(&stations);
        ai.equipment = EquipmentFaults {
            radar: true,
            infrared: false,
            visual: true,
            rwr: true,
        };
        let own = Ownship::from_ai(&row, ai, config, true).unwrap();
        assert!(own.radar_failed && own.visual_failed && own.rwr_failed);
        assert!(!own.infrared_failed);
        let back = own.into_ai(AiPose {
            launcher: launcher(),
            on_ground: true,
        });
        assert_eq!(
            back.equipment,
            EquipmentFaults {
                radar: true,
                infrared: false,
                visual: true,
                rwr: true
            }
        );
        assert!(back.row.on_ground);
    }

    #[test]
    fn a_living_aircraft_never_rounds_to_zero_and_a_dead_one_stays_dead() {
        let config = config();
        let (mut row, stations) = ai_aircraft(&config, 1);
        row.initial_hp = 1000;
        let own = Ownship::from_ai(&row, stores(&stations), config.clone(), true).unwrap();
        assert_eq!(own.hp, 1);
        assert_eq!(scale_hp(0, 100, 30), 0);
        assert_eq!(scale_hp(100, 100, 30), 30);
    }

    /// A human who gives its plane back while its rounds are in the air: the
    /// rounds fly on with their own copy of the weapon, and combat steps on
    /// with no ownship at all, as an open mission does (D3c).
    #[test]
    fn given_back_rounds_fly_on_and_combat_steps_with_no_ownship() {
        let mut state = fixture(false);
        let gun = tore_formats::aircraft::AircraftId::F18.gun().unwrap();
        let mut config = state.own().configuration().clone();
        config.stations[0].weapon.source = gun.into();
        state.own_mut().config.stations[0].weapon.source = gun.into();
        state
            .add_ownship(Ownship::new(5, state.own().side, config.clone(), true).unwrap())
            .unwrap();
        let input = |held| super::super::OwnshipInput {
            aircraft: 5,
            held,
            launcher: launcher(),
        };
        for _ in 0..20 {
            state.step(&[input(true)], |_, _| 0.);
        }
        assert!(
            state
                .projectiles
                .iter()
                .any(|p| p.owner == 5 && p.weapon.is_none())
        );
        state.remove_ownship(5).unwrap();
        let rounds: Vec<_> = state.projectiles.iter().filter(|p| p.owner == 5).collect();
        assert!(!rounds.is_empty());
        assert!(
            rounds
                .iter()
                .all(|p| p.weapon.as_ref() == Some(&config.stations[p.station].weapon))
        );
        // The last ownship goes too, and the rounds fly on to the end.
        state.remove_ownship(0).unwrap();
        assert!(state.ownships().is_empty());
        for _ in 0..600 {
            state.step(&[], |_, _| 0.);
        }
        assert!(state.projectiles.iter().all(|p| p.owner != 5));
    }

    /// An open mission's aircraft rows count from 0: plane 0 is a row.
    #[test]
    fn an_open_mission_numbers_its_rows_from_zero() {
        let mut state = super::super::State::open_mission();
        assert!(state.ownships().is_empty());
        let config = config();
        for _ in 0..2 {
            state.add_dummy(
                &config,
                [0., 5000., 0.],
                Basis::new(0., 0., 0.),
                super::super::Side(1),
            );
        }
        let ids: Vec<u32> = state.targets.iter().map(|t| t.id).collect();
        assert_eq!(ids, [0, 1]);
    }
}
