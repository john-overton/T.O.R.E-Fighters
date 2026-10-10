//! The smoke, contrails and gun rounds of a converted network flight
//! (docs/ARCHITECTURE.md, "Converting a capture into a replay"; slice E2).
//!
//! The host sends none of them. A live client makes them again from what it
//! draws ([`crate::regen::Effects`] for smoke and contrails, [`Guns`] for the
//! rounds of the host's gun bursts), and this does the same over the frames
//! of a conversion, tick by tick, so the replay carries them the way a
//! single-player recording does: puffs in the frame's spawns, rounds as
//! projectiles of the gun's weapon. The viewer then rebuilds them from the
//! recording like any other.
//!
//! Nothing here reads a clock or an unordered map: what comes out is a pure
//! function of the frames it is given, so converting twice gives the same
//! bytes.
use super::convert::{self, Identities, Presentation};
use crate::net::guns::{Around, Guns};
use crate::regen::{Effects, Motor, Surroundings};
use crate::snapshot::RenderSnapshot;
use std::cell::RefCell;
use std::collections::BTreeMap;
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_replay as replay;
use tore_replay::vocab::{field, kind};
use tore_session::client::convert::Regenerate;
use tore_session::wire::events::{ReceivedEvent, WireEvent};
use tore_sim::{
    attitude::Vector,
    combat::{live, smoke::Kind},
};
use tore_world::world::World;

/// What finds an aircraft type's engine outlets: feet right, up and forward
/// of the aircraft's centre.
pub type Outlets<'a> = Box<dyn FnMut(AircraftId) -> Vec<Vector> + 'a>;

/// The outlets of the drawn model the game loads for each aircraft type, from
/// the import (none when the model does not load: no contrails for it).
pub fn model_outlets(resources: &BTreeMap<String, Vec<u8>>) -> Outlets<'_> {
    Box::new(
        move |aircraft| match crate::aircraft::Airframe::load(resources, aircraft) {
            Ok(model) => model.kind.contrail_offsets.clone(),
            Err(error) => {
                log::warn!(
                    "Network replay: no contrails for {}: its model did not load ({error})",
                    aircraft.label()
                );
                Vec::new()
            }
        },
    )
}

/// What was added to a flight's frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Puffs of smoke released: hits, wrecks, motors and burning crash sites.
    pub smoke: u64,
    pub contrails: u64,
    /// Gun rounds made, one for each round whatever its length of flight.
    pub rounds: u64,
}

/// Smoke, contrails and gun rounds for the frames of one flight.
pub struct NetEffects<'a> {
    world: &'a World,
    resources: &'a BTreeMap<String, Vec<u8>>,
    presentation: Presentation,
    identities: Identities,
    effects: Effects,
    guns: Guns,
    /// Every crash-site fire the host's events started: they burn for good
    /// and send up a column of smoke.
    fires: Vec<(u64, replay::EffectSpawn)>,
    /// Each aircraft type's engine outlets, loaded the first time one is
    /// high enough to leave a contrail.
    outlets: BTreeMap<&'static str, Vec<Vector>>,
    load_outlets: Outlets<'a>,
    /// Each weapon record's motor, parsed once.
    motors: RefCell<BTreeMap<String, Option<Motor>>>,
    /// The ids of the guns' weapons the flight's rounds name, by record name,
    /// and the next id free for one.
    guns_registered: BTreeMap<String, u32>,
    next_weapon: u32,
    tally: Tally,
    /// The newest round number seen: they only go up.
    newest_round: u32,
}

impl<'a> NetEffects<'a> {
    /// For a flight of `world` (the mission as the client built it), with
    /// the replay's `header`, its aircraft and the weapons the conversion
    /// registered. `resources` is the import the capture was made with and
    /// `outlets` finds an aircraft type's engine outlets.
    pub fn new(
        world: &'a World,
        resources: &'a BTreeMap<String, Vec<u8>>,
        outlets: Outlets<'a>,
        header: &replay::Header,
        aircraft: &[replay::AircraftInfo],
        weapons: &[replay::WeaponInfo],
    ) -> Self {
        let identities = Identities {
            aircraft: aircraft
                .iter()
                .filter_map(|info| Some((info.id, convert::identity(&info.pt)?)))
                .collect(),
            weapons: weapons.iter().map(|w| (w.id, w.clone())).collect(),
            ..Identities::default()
        };
        Self {
            world,
            resources,
            presentation: Presentation::from_header(header),
            guns_registered: weapons.iter().map(|w| (w.source.clone(), w.id)).collect(),
            next_weapon: weapons.iter().map(|w| w.id + 1).max().unwrap_or(0),
            identities,
            effects: Effects::default(),
            guns: Guns::default(),
            fires: Vec::new(),
            tally: Tally::default(),
            newest_round: 0,
            outlets: BTreeMap::new(),
            load_outlets: outlets,
            motors: RefCell::default(),
        }
    }

    /// What has been added so far.
    pub fn tally(&self) -> Tally {
        self.tally
    }

    /// Loads the engine outlets of every aircraft type in `picture` that is
    /// high enough to leave a contrail and has none loaded yet.
    fn load_outlets(&mut self, picture: &RenderSnapshot) {
        let sortie = self.world.combat.contrail_sortie();
        let poses = std::iter::once(&picture.player).chain(&picture.targets);
        for pose in poses {
            let Some(aircraft) = pose.aircraft else {
                continue;
            };
            if self.outlets.contains_key(aircraft.selection_key())
                || !pose.airborne
                || pose.crashed
                || pose.damage.hp <= 0
                || !pose.engine.lit
                || pose.position[1] < tore_sim::combat::smoke::contrail_altitude_ft(sortie, pose.id)
            {
                continue;
            }
            let offsets = (self.load_outlets)(aircraft);
            self.outlets.insert(aircraft.selection_key(), offsets);
        }
    }

    /// The id the replay gives the gun weapon named `source`, with its
    /// identity when it is new.
    fn gun_weapon(&mut self, source: &str) -> (u32, Option<replay::WeaponInfo>) {
        if let Some(id) = self.guns_registered.get(source) {
            return (*id, None);
        }
        let id = self.next_weapon;
        self.next_weapon += 1;
        self.guns_registered.insert(source.to_owned(), id);
        let info = self
            .world
            .combat
            .dummy_configurations()
            .iter()
            .flat_map(|config| &config.stations)
            .map(|station| &station.weapon)
            .find(|weapon| weapon.source == source)
            .map(|weapon| convert::weapon_info(id, weapon))
            .unwrap_or_else(|| replay::WeaponInfo {
                id,
                source: source.to_owned(),
                shape: None,
                name: source.to_owned(),
                class: replay::WeaponClass::Gun,
            });
        (id, Some(info))
    }
}

/// The gun bursts a frame holds, as the client session was given them.
fn bursts(frame: &replay::Frame) -> Vec<ReceivedEvent> {
    frame
        .events
        .iter()
        .filter(|event| event.kind == kind::WEAPON_GUN_BURST)
        .filter_map(|event| {
            let station = u8::try_from(event.get(field::STATION)?.as_i64()?).ok()?;
            Some(ReceivedEvent {
                number: 0,
                tick: u32::try_from(frame.tick).ok()?,
                event: WireEvent::GunBurst {
                    shooter: event.subject?,
                    station,
                    length: event
                        .get("length_ticks")
                        .and_then(replay::Value::as_i64)
                        .and_then(|n| u32::try_from(n).ok()),
                },
            })
        })
        .collect()
}

/// The puffs released this tick: the newest of each layer, which have not
/// aged yet, oldest first (as the single-player recorder lists them).
fn new_puffs(effects: &Effects) -> Vec<replay::PuffSpawn> {
    let mut out = Vec::new();
    for (layer, smoke) in [
        (replay::LAYER_SMOKE, &effects.smoke),
        (replay::LAYER_CONTRAILS, &effects.contrails),
    ] {
        let start = out.len();
        for puff in smoke.puffs.iter().rev().take_while(|p| p.age == 0) {
            out.push(replay::PuffSpawn {
                layer,
                kind: match puff.kind {
                    Kind::Missile => replay::PuffKind::Missile,
                    Kind::Aircraft => replay::PuffKind::Aircraft,
                    Kind::Contrail => replay::PuffKind::Contrail,
                    Kind::Burning => replay::PuffKind::Burning,
                },
                position: puff.position,
            });
        }
        out[start..].reverse();
    }
    out.truncate(replay::limits::MAX_PUFFS_PER_TICK);
    out
}

impl Regenerate for NetEffects<'_> {
    fn frame(&mut self, frame: &mut replay::Frame) -> Vec<replay::WeaponInfo> {
        let tick = frame.tick;
        let world = self.world;
        self.fires.extend(
            frame
                .new_effects
                .iter()
                .filter(|effect| effect.kind == replay::EffectKind::Fire)
                .map(|effect| (tick, effect.clone())),
        );
        let mut picture = convert::snapshot(frame, &[], &self.presentation, &self.identities);
        picture.marks = convert::marks_at(&self.fires, tick);

        // Smoke and contrails, by the rules combat steps them with.
        self.load_outlets(&picture);
        {
            let outlets = &self.outlets;
            let motors = &self.motors;
            let resources = self.resources;
            let outlets_of = |id: AircraftId| -> &[Vector] {
                outlets
                    .get(id.selection_key())
                    .map_or(&[][..], Vec::as_slice)
            };
            let motor_of = |name: &str| -> Option<Motor> {
                motors
                    .borrow_mut()
                    .entry(name.to_owned())
                    .or_insert_with(|| {
                        let bytes = resources.get(name)?;
                        Motor::of(&Weapon::parse(name, bytes).ok()?)
                    })
                    .to_owned()
            };
            self.effects.step(
                &picture,
                &Surroundings {
                    terrain: &world.terrain,
                    outlets: &outlets_of,
                    motor: &motor_of,
                    sortie: world.combat.contrail_sortie(),
                },
                &[],
            );
        }
        frame.new_puffs = new_puffs(&self.effects);
        for puff in &frame.new_puffs {
            if puff.layer == replay::LAYER_CONTRAILS {
                self.tally.contrails += 1;
            } else {
                self.tally.smoke += 1;
            }
        }

        // Gun rounds, from the host's bursts. Every aircraft's burst makes
        // rounds, the player's too: a replay has no trigger of its own.
        let ground = |x: f64, z: f64| f64::from(world.terrain.height(x as f32, z as f32));
        let stations = |id: AircraftId| -> &[live::Station] {
            world
                .combat
                .dummy_configurations()
                .iter()
                .find(|config| config.aircraft == id)
                .map_or(&[][..], |config| config.stations.as_slice())
        };
        if frame
            .aircraft
            .iter()
            .any(|a| a.id == self.presentation.player)
        {
            picture.targets.push(picture.player.clone());
        }
        self.guns.step_recorded(
            &bursts(frame),
            tick,
            &Around {
                ground: &ground,
                stations: &stations,
            },
            &picture,
        );
        let mut registered = Vec::new();
        let rounds: Vec<_> = self
            .guns
            .rounds(tick)
            .map(|(id, shooter, age, round)| {
                (
                    id,
                    shooter,
                    age,
                    round.weapon.clone(),
                    round.tracer,
                    round.position,
                    round.previous,
                    round.direction,
                    round.speed_f8,
                )
            })
            .collect();
        for (id, shooter, age, weapon, tracer, position, previous, direction, speed_f8) in rounds {
            if frame.projectiles.len() >= replay::limits::MAX_PROJECTILES {
                break;
            }
            if id > self.newest_round {
                self.newest_round = id;
                self.tally.rounds += 1;
            }
            let (weapon, new) = self.gun_weapon(&weapon);
            registered.extend(new);
            frame.projectiles.push(replay::ProjectileState {
                id,
                owner: shooter,
                weapon,
                target: None,
                position,
                previous,
                direction,
                speed: f64::from(speed_f8) / 256.,
                tracer,
                incoming: false,
                age: u32::try_from(age).unwrap_or(u32::MAX),
                seeker: None,
            });
        }
        registered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::{identity, playback::Playback, tests::TempDir};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use tore_session::client::convert as session;
    use tore_session::fixture::{Fight, bot_fight, bot_fight_at};

    /// Where the synthetic engines exhaust.
    const OUTLETS: [Vector; 2] = [[-3., 0., -20.], [3., 0., -20.]];

    /// `fight`'s capture converted into `out` with the smoke, contrails and
    /// gun rounds, the contrails from `outlets` (the synthetic import has no
    /// drawn model to read them from).
    fn convert(fight: &Fight, out: &Path, outlets: Outlets<'_>) -> PathBuf {
        let conversion = session::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
        let flight = conversion.flights().remove(0);
        let world = conversion.mission().expect("the mission");
        let header = conversion.header(
            &flight,
            identity::of(&world.terrain),
            "test",
            "test",
            "2026-10-05T15:40:00Z",
        );
        let mut effects = NetEffects::new(
            world,
            &fight.resources,
            outlets,
            &header,
            &conversion.roster(&flight),
            &conversion.weapons(&flight),
        );
        conversion
            .write_with(&flight, &header, out, &mut effects)
            .unwrap()
            .path
    }

    fn fixed_outlets() -> Outlets<'static> {
        Box::new(|_| OUTLETS.to_vec())
    }

    /// How many puffs of each layer and kind a recording spawned.
    fn puffs(recording: &tore_replay::Recording) -> BTreeMap<(u8, u8), usize> {
        let mut counts = BTreeMap::new();
        for (_, puff) in recording.spawns(0, u64::MAX).unwrap().puffs {
            *counts.entry((puff.layer, puff.kind.code())).or_insert(0) += 1;
        }
        counts
    }

    #[test]
    fn a_high_flight_converts_with_contrails_from_its_engines() {
        let fight = bot_fight_at(12, 40_000);
        let dir = TempDir::new("net-effects-contrails");
        let path = convert(&fight, &dir.path().join("a.tore-replay"), fixed_outlets());
        let recording = tore_replay::Recording::open(&path).unwrap();
        assert!(
            recording.problems().is_empty(),
            "{:?}",
            recording.problems()
        );
        let counts = puffs(&recording);
        // Four aircraft, two outlets each, above their onset altitude for
        // the whole flight: contrails and nothing else (no damage, no
        // missiles, no fires). Combat leaves ten puffs a second per outlet;
        // the client's flight is a few seconds shorter than the host's.
        assert_eq!(counts.len(), 1, "{counts:?}");
        let contrails = counts[&(replay::LAYER_CONTRAILS, replay::PuffKind::Contrail.code())];
        let seconds = recording.last_tick().unwrap() - recording.first_tick().unwrap();
        let seconds = seconds as f64 / 120.;
        let per_second = contrails as f64 / seconds;
        assert!(
            (60. ..=85.).contains(&per_second),
            "{contrails} contrail puffs in {seconds} s is {per_second} a second"
        );
        // The viewer's own playback rebuilds them from their release ticks.
        let mut playback = Playback::new(Arc::new(recording));
        let last = playback.recording().last_tick().unwrap();
        let live = playback.smoke(last)[1].puffs.len();
        assert!(live > 100, "{live} contrail puffs alive at the end");
        assert!(playback.smoke(last)[0].puffs.is_empty());
    }

    #[test]
    fn a_low_flight_has_no_contrails_and_the_import_that_cannot_load_a_model_has_none_either() {
        let fight = bot_fight(10);
        let dir = TempDir::new("net-effects-low");
        let low = convert(&fight, &dir.path().join("a.tore-replay"), fixed_outlets());
        assert!(puffs(&tore_replay::Recording::open(&low).unwrap()).is_empty());
        // The high flight with the game's own loader, which the synthetic
        // import cannot satisfy: the conversion still works.
        let high = bot_fight_at(6, 40_000);
        let path = convert(
            &high,
            &dir.path().join("b.tore-replay"),
            model_outlets(&high.resources),
        );
        assert!(puffs(&tore_replay::Recording::open(&path).unwrap()).is_empty());
    }

    #[test]
    fn the_hosts_gun_bursts_become_rounds_in_flight_with_stable_numbers() {
        let fight = bot_fight(12);
        let dir = TempDir::new("net-effects-guns");
        let path = convert(&fight, &dir.path().join("a.tore-replay"), fixed_outlets());
        let recording = tore_replay::Recording::open(&path).unwrap();
        let gun = recording
            .weapons()
            .find(|w| w.class == replay::WeaponClass::Gun)
            .expect("the gun's weapon is registered")
            .clone();
        let cannon = Weapon::parse("M61.JT", &fight.resources["M61.JT"]).unwrap();
        // The rounds the closed bursts add up to, by the gun's own cadence.
        let mut bursts = 0;
        let mut expected = 0;
        let mut lives: BTreeMap<u32, (u64, u64, u32, bool)> = BTreeMap::new();
        for frame in recording.frames(0, u64::MAX) {
            let frame = frame.unwrap();
            for event in &frame.events {
                let length = event.get("length_ticks").and_then(replay::Value::as_i64);
                if event.kind == kind::WEAPON_GUN_BURST
                    && let Some(length) = length
                {
                    bursts += 1;
                    let last = frame.tick + length as u64 - 1;
                    expected += (0u64..)
                        .take_while(|n| {
                            tore_sim::combat::gun_round::release_tick(&cannon, frame.tick, *n)
                                <= last
                        })
                        .count();
                }
            }
            for round in &frame.projectiles {
                assert_eq!(round.weapon, gun.id);
                assert!(round.id >= 0xE000_0000, "{}", round.id);
                assert!(round.speed > 1_000., "a cannon shell: {}", round.speed);
                let life = lives.entry(round.id).or_insert((
                    frame.tick,
                    frame.tick,
                    round.owner,
                    round.tracer,
                ));
                // Alive on every tick between its first and last, one shooter.
                assert!(frame.tick <= life.1 + 1, "{} skipped a tick", round.id);
                assert_eq!(life.2, round.owner);
                assert_eq!(life.3, round.tracer);
                life.1 = frame.tick;
                assert_eq!(u64::from(round.age), frame.tick - life.0);
            }
        }
        let tracers = lives.values().filter(|life| life.3).count();
        assert!(bursts >= 2, "the fight has closed bursts: {bursts}");
        assert_eq!(lives.len(), expected, "{bursts} bursts");
        // Every third round is a tracer.
        let third = lives.len() / 3;
        assert!(
            tracers.abs_diff(third) <= bursts + 1,
            "{tracers} of {}",
            lives.len()
        );
        // The viewer draws them as gun rounds, from the burst's shooter.
        let mut playback = Playback::new(Arc::new(recording));
        let at = lives.values().map(|l| l.0 + 3).next().unwrap();
        let picture = playback.picture(at, 1.);
        let drawn: Vec<_> = picture.projectiles.iter().filter(|p| p.gun).collect();
        assert!(!drawn.is_empty() && drawn.iter().all(|p| p.weapon == gun.source));
    }

    #[test]
    fn converting_twice_gives_the_same_bytes_and_a_conversion_without_effects_has_none() {
        let fight = bot_fight_at(8, 40_000);
        let dir = TempDir::new("net-effects-twice");
        let a = convert(&fight, &dir.path().join("a.tore-replay"), fixed_outlets());
        let b = convert(&fight, &dir.path().join("b.tore-replay"), fixed_outlets());
        let (a, b) = (std::fs::read(a).unwrap(), std::fs::read(b).unwrap());
        assert!(a == b, "the same capture gave different bytes");
        // The conversion without the game's additions (the session's own
        // `write`) is smaller and holds no puffs.
        let conversion = session::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
        let flight = conversion.flights().remove(0);
        let header = conversion.header(
            &flight,
            identity::of(&conversion.mission().unwrap().terrain),
            "test",
            "test",
            "2026-10-05T15:40:00Z",
        );
        let plain = dir.path().join("plain.tore-replay");
        conversion.write(&flight, &header, &plain).unwrap();
        let plain = tore_replay::Recording::open(&plain).unwrap();
        assert!(puffs(&plain).is_empty());
    }

    /// A frame of the synthetic fight's world to put effects on: the
    /// player (id 0) and one other aircraft.
    fn bare_frame(tick: u64, others: &[replay::AircraftState]) -> replay::Frame {
        replay::Frame {
            tick,
            aircraft: others.to_vec(),
            ..replay::Frame::default()
        }
    }

    fn aircraft(id: u32, y: f64, hp: i32) -> replay::AircraftState {
        replay::AircraftState {
            id,
            position: [id as f64 * 500., y, 0.],
            velocity: [0., 0., 600.],
            flags: replay::AircraftFlags {
                engine_on: true,
                airborne: true,
                alive: hp > 0,
                ..replay::AircraftFlags::default()
            },
            hp,
            max_hp: 100,
            ..replay::AircraftState::default()
        }
    }

    #[test]
    fn a_hurt_aircraft_a_burning_wreck_and_a_missile_in_its_burn_leave_smoke() {
        let fight = bot_fight(4);
        let conversion = session::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
        let flight = conversion.flights().remove(0);
        let world = conversion.mission().unwrap();
        let header = conversion.header(&flight, identity::of(&world.terrain), "test", "test", "");
        let aircraft_info = |id: u32| replay::AircraftInfo {
            id,
            pt: "F18.PT".into(),
            ..replay::AircraftInfo::default()
        };
        let weapons = [replay::WeaponInfo {
            id: 0,
            source: "AIM9M.JT".into(),
            shape: None,
            name: "AIM-9M".into(),
            class: replay::WeaponClass::Missile,
        }];
        let mut effects = NetEffects::new(
            world,
            &fight.resources,
            Box::new(|_| Vec::new()),
            &header,
            &[aircraft_info(0), aircraft_info(1), aircraft_info(2)],
            &weapons,
        );
        let missile = replay::ProjectileState {
            id: 40,
            owner: 1,
            weapon: 0,
            position: [100., 5_000., 100.],
            direction: [0., 0., 1.],
            speed: 900.,
            ..replay::ProjectileState::default()
        };
        let mut counts: BTreeMap<(u8, u8), usize> = BTreeMap::new();
        for tick in 0..400 {
            // Aircraft 1 is hurt below half its hit points, aircraft 2 is
            // whole, and a crash-site fire starts on tick 5.
            let mut frame = bare_frame(tick, &[aircraft(0, 8_000., 100), aircraft(1, 6_000., 40)]);
            frame.aircraft.push(aircraft(2, 7_000., 100));
            frame.projectiles.push(missile.clone());
            if tick == 5 {
                frame.new_effects.push(replay::EffectSpawn {
                    kind: replay::EffectKind::Fire,
                    position: [900., 20., 900.],
                    duration_ticks: u32::MAX,
                });
            }
            assert!(effects.frame(&mut frame).is_empty());
            for puff in &frame.new_puffs {
                *counts.entry((puff.layer, puff.kind.code())).or_insert(0) += 1;
            }
        }
        let count = |kind: replay::PuffKind| counts.get(&(replay::LAYER_SMOKE, kind.code()));
        assert!(
            count(replay::PuffKind::Aircraft).is_some_and(|n| *n > 20),
            "{counts:?}"
        );
        assert!(
            count(replay::PuffKind::Burning).is_some_and(|n| *n > 20),
            "{counts:?}"
        );
        assert!(
            count(replay::PuffKind::Missile).is_some_and(|n| *n > 20),
            "{counts:?}"
        );
        // Nothing is high enough for a contrail.
        assert!(
            !counts
                .keys()
                .any(|(layer, _)| *layer == replay::LAYER_CONTRAILS)
        );
    }

    #[test]
    fn a_burst_names_its_gun_once_and_an_unknown_gun_is_still_a_gun() {
        let fight = bot_fight(4);
        let conversion = session::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
        let flight = conversion.flights().remove(0);
        let world = conversion.mission().unwrap();
        let header = conversion.header(&flight, identity::of(&world.terrain), "test", "test", "");
        let mut effects = NetEffects::new(
            world,
            &fight.resources,
            Box::new(|_| Vec::new()),
            &header,
            &conversion.roster(&flight),
            &[],
        );
        let (id, new) = effects.gun_weapon("M61.JT");
        assert_eq!(id, 0);
        let new = new.expect("a new weapon is returned once");
        assert_eq!(new.class, replay::WeaponClass::Gun);
        assert_eq!(effects.gun_weapon("M61.JT"), (0, None));
        let (other, unknown) = effects.gun_weapon("NOGUN.JT");
        assert_eq!(other, 1);
        assert_eq!(unknown.unwrap().class, replay::WeaponClass::Gun);
    }
}
