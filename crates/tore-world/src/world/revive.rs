//! Revival: a player whose plane is lost flies again in a new aircraft of the
//! same type, just outside the battle. Stage F phase 2; see
//! docs/ARCHITECTURE.md, "Death, revival and lives".
//!
//! Slice F2-0 added the types the host, the wire and the mission core share:
//! where a revived plane appears ([`Spawn`]) and what it may carry
//! ([`RevivalWeapons`]). Slice F2-V builds the rest here:
//!
//! - **Abandon** ([`World::abandon_plane`], the mission command
//!   [`super::MissionCommand::Abandon`]): a seat leaves its lost plane, whose
//!   pilot becomes [`Pilot::Lost`]. Its cockpit steps on with neutral controls
//!   (the wreck falling, the escape) and its cues reach [`NOBODY`].
//! - **Revive** ([`World::revive_plane`], [`super::MissionCommand::Revive`]):
//!   abandons the seat's lost plane, adds a new plane of the same aircraft to
//!   its wing ([`World::add_plane`], which a client's copy of the mission
//!   repeats from the Spawned message) and seats the seat in it by the
//!   handoff ([`World::take_plane`]), so every rule of the handoff holds.
//! - **Room**: at most [`MAX_PLANES`] planes at once. A revival that would
//!   pass it first retires the oldest abandoned wreck that has rested on the
//!   ground for [`RETIRE_AFTER_TICKS`] ([`World::retire_plane`]).
//! - **The revival point** ([`point`]) and **the weapons rule**
//!   ([`RevivalWeapons::cut`]), which the host reads to choose a [`Spawn`].
//!
//! The bookkeeping ([`Book`]) is mutable mission state a checkpoint must
//! carry; single player never abandons a plane, so its book stays empty.

use super::World;
use crate::{
    WorldResult,
    ai_wings::{self, ENEMY_SIDE, FRIENDLY_SIDE},
    combat,
    mission::{LoadoutSpec, StationLoad},
    seats::{Pilot, Plane, PlaneId, SeatId, Slot},
};
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::{
    ai::launch::Side,
    attitude::Basis,
    combat::{ledger::ShotKind, live},
    ejection::Phase,
    flight,
    sensors::FEET_PER_NAUTICAL_MILE,
};

/// The seat a lost plane's cues are addressed to: no seat has it (a host
/// gives out seats 0 to 254), so what a wreck does reaches nobody.
pub const NOBODY: SeatId = SeatId(u8::MAX);

/// The most planes a mission holds at once: the wire's aircraft in a
/// snapshot (docs/ARCHITECTURE.md, "Death, revival and lives").
pub const MAX_PLANES: usize = 64;

/// How long an abandoned wreck must have rested on the ground before a
/// revival may retire it: 30 seconds of ticks.
pub const RETIRE_AFTER_TICKS: u64 = 30 * 120;

/// Aircraft within this many nautical miles of an aircraft of the other side
/// are in the battle, whose centre the revival point is measured from
/// (fitted; retail says only "just outside the battle zone").
pub const BATTLE_RANGE_NM: f64 = 20.;

/// Where and how a revived plane appears, which the host chooses and every
/// client's copy of the mission repeats: the host's
/// [`super::MissionCommand::Revive`] and the wire's Spawned message carry it.
#[derive(Clone, Debug, PartialEq)]
pub struct Spawn {
    /// The world position, feet.
    pub position: [f64; 3],
    /// The heading, radians.
    pub heading_rad: f64,
    /// The airspeed, feet a second.
    pub speed_fps: f64,
    /// The stores, already cut by the revival weapons rule, with full fuel.
    pub loadout: LoadoutSpec,
}

/// The King's `revive-weapons` setting: what a revived aircraft carries of
/// the loadout its player chose (or the standard load). The wire codes it in
/// the setting's value, in this order (docs/formats/net-protocol.md,
/// "Settings by number").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RevivalWeapons {
    /// The loadout whole.
    #[default]
    Missiles,
    /// Every air-to-air missile station emptied; air-to-ground missiles,
    /// bombs and the gun kept.
    NoMissiles,
    /// The gun alone.
    Guns,
    /// The gun alone, with half its rounds, rounded up.
    HalfGuns,
}

impl RevivalWeapons {
    /// Every rule, in the setting's value order.
    pub const ALL: [RevivalWeapons; 4] = [
        RevivalWeapons::Missiles,
        RevivalWeapons::NoMissiles,
        RevivalWeapons::Guns,
        RevivalWeapons::HalfGuns,
    ];

    /// The rule for the setting's value, if it names one.
    pub fn from_value(value: u32) -> Option<Self> {
        Self::ALL.get(usize::try_from(value).ok()?).copied()
    }

    /// The setting's value for the rule.
    pub fn value(self) -> u32 {
        match self {
            RevivalWeapons::Missiles => 0,
            RevivalWeapons::NoMissiles => 1,
            RevivalWeapons::Guns => 2,
            RevivalWeapons::HalfGuns => 3,
        }
    }

    /// What a station carrying `quantity` of `weapon` keeps under the rule.
    /// An air-to-air missile is what the retail debrief counts as one
    /// (`ShotKind::AirToAir`: guided, flag 1, against aircraft, flag
    /// 0x10000); the gun is the aircraft's gun record (agent decision,
    /// F2-V).
    pub fn cut(self, weapon: &Weapon, quantity: u16) -> u16 {
        let gun = live::is_gun(weapon);
        let air_to_air = ShotKind::of(weapon) == ShotKind::AirToAir;
        match self {
            RevivalWeapons::Missiles => quantity,
            RevivalWeapons::NoMissiles if air_to_air => 0,
            RevivalWeapons::NoMissiles => quantity,
            RevivalWeapons::Guns | RevivalWeapons::HalfGuns if !gun => 0,
            RevivalWeapons::Guns => quantity,
            RevivalWeapons::HalfGuns => quantity.div_ceil(2),
        }
    }
}

/// A plane a revival adds to the mission: the host's world makes it by
/// [`World::revive_plane`], a client's copy by [`World::add_plane`] from the
/// Spawned message.
#[derive(Clone, Debug, PartialEq)]
pub struct NewPlane {
    pub plane: PlaneId,
    /// Its wing, and the next free member number in it.
    pub slot: Slot,
    pub aircraft: AircraftId,
    pub spawn: Spawn,
}

/// A plane abandoned to the mission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LostPlane {
    pub plane: PlaneId,
    /// The tick it was abandoned at.
    pub abandoned: u64,
    /// The tick from which its wreck has rested on the ground (crashed and
    /// no longer falling), its pilot's escape over; `None` while it still
    /// flies or falls.
    pub resting_since: Option<u64>,
}

/// The revival's bookkeeping in the mission core: the abandoned planes in
/// the order they were abandoned, the retired ones, and the planes the
/// mission did not start with. Mutable mission state, coded in its own
/// checkpoint section (`revive_checkpoint.rs`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    lost: Vec<LostPlane>,
    retired: Vec<Plane>,
    /// Every plane [`World::add_plane`] added, in order.
    added: Vec<PlaneId>,
}

impl Book {
    /// The abandoned planes still in the mission, oldest first.
    pub fn lost(&self) -> &[LostPlane] {
        &self.lost
    }

    /// The planes retired to make room, oldest first, as the roster held
    /// them (their pilot [`Pilot::Lost`]). They keep their ledger entries
    /// and their results row.
    pub fn retired(&self) -> &[Plane] {
        &self.retired
    }

    /// The planes the mission did not start with (revivals' new planes), in
    /// the order they were added, retired ones included.
    pub fn added(&self) -> &[PlaneId] {
        &self.added
    }

    /// The planes the mission started with, as the roster `planes` and the
    /// retired ones hold them, in id order: what a fresh build of the
    /// mission has, for the checkpoint's mission identity.
    pub fn first_planes<'a>(&'a self, planes: &'a [Plane]) -> Vec<&'a Plane> {
        let mut first: Vec<&Plane> = planes
            .iter()
            .chain(&self.retired)
            .filter(|plane| !self.added.contains(&plane.id))
            .collect();
        first.sort_by_key(|plane| plane.id);
        first
    }
}

/// The horizontal distance between two points, feet.
fn horizontal(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

/// The revival point (fitted; docs/ARCHITECTURE.md, "Death, revival and
/// lives"). The battle's centre is the mean position of the living
/// `aircraft` (side and position) that have an aircraft of the other side
/// within [`BATTLE_RANGE_NM`], or of every living aircraft if none has, or
/// `start` with none at all. The new plane is placed on the bearing from
/// that centre towards `start` (the mean start position of its side's
/// wings), `distance_ft` from the centre, at `altitude_ft`, heading for the
/// centre. Returns the position and the heading. Distances are horizontal
/// (agent decision, F2-V); a start on the centre itself places the plane
/// due south of it, heading north (+z).
pub fn point(
    aircraft: &[(Side, [f64; 3])],
    start: [f64; 3],
    distance_ft: f64,
    altitude_ft: f64,
) -> ([f64; 3], f64) {
    let range = BATTLE_RANGE_NM * FEET_PER_NAUTICAL_MILE;
    let fighting: Vec<[f64; 3]> = aircraft
        .iter()
        .filter(|(side, at)| {
            aircraft
                .iter()
                .any(|(other, there)| other != side && horizontal(*at, *there) <= range)
        })
        .map(|(_, at)| *at)
        .collect();
    let pool: Vec<[f64; 3]> = if fighting.is_empty() {
        aircraft.iter().map(|(_, at)| *at).collect()
    } else {
        fighting
    };
    let centre = if pool.is_empty() {
        start
    } else {
        let n = pool.len() as f64;
        std::array::from_fn(|i| pool.iter().map(|p| p[i]).sum::<f64>() / n)
    };
    let (dx, dz) = (start[0] - centre[0], start[2] - centre[2]);
    let length = dx.hypot(dz);
    let (ux, uz) = if length < 1. {
        (0., -1.)
    } else {
        (dx / length, dz / length)
    };
    let position = [
        centre[0] + ux * distance_ft,
        altitude_ft,
        centre[2] + uz * distance_ft,
    ];
    // Basis forward is (sin yaw, 0, cos yaw): head back along the bearing.
    let heading = (-ux).atan2(-uz);
    (position, heading)
}

/// A side of the AI's launch as combat's rows carry it.
fn row_side(side: Side) -> live::Side {
    match side {
        Side::Friendly => FRIENDLY_SIDE,
        Side::Enemy => ENEMY_SIDE,
    }
}

/// What [`World::revival_check`] found a revival will do.
struct Plan {
    new: NewPlane,
    /// The wreck retired first to make room.
    retire: Option<PlaneId>,
}

impl World {
    /// Whether the plane in cockpit `index` is lost: crashed, destroyed, or
    /// its pilot dead, ejected or escaping. The handoff's own test: such a
    /// plane cannot go back to the AI.
    pub(crate) fn cockpit_lost(&self, index: usize) -> bool {
        let cockpit = &self.cockpits[index];
        let flight = &cockpit.flight;
        let pilot = &flight.systems.pilot;
        flight.crashed
            || flight.escape.is_some()
            || pilot.dead
            || pilot.ejected
            || self
                .combat
                .state
                .ownship(cockpit.plane.0)
                .is_none_or(|own| own.hp <= 0)
    }

    /// Whether a human's `plane` is lost ([`Self::cockpit_lost`]). An
    /// AI-flown plane, or one not in the mission, is not.
    pub fn plane_lost(&self, plane: PlaneId) -> bool {
        self.cockpits
            .iter()
            .position(|cockpit| cockpit.plane == plane)
            .is_some_and(|index| self.cockpit_lost(index))
    }

    /// The cockpit of the lost plane `seat` flies.
    fn lost_cockpit_of(&self, seat: SeatId) -> WorldResult<usize> {
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
        if !self.cockpit_lost(index) {
            return Err(format!("plane {} is not lost", plane.0).into());
        }
        Ok(index)
    }

    /// Whether `seat` can abandon its plane now: it flies one, and it is lost.
    pub fn can_abandon(&self, seat: SeatId) -> WorldResult<()> {
        self.lost_cockpit_of(seat).map(|_| ())
    }

    /// The seat leaves its lost plane to the mission: nobody flies it from
    /// now on ([`Pilot::Lost`]), its trigger is let go, its cockpit steps on
    /// with neutral controls, and the seat waits with no plane. Refuses a
    /// plane that is not lost, changing nothing.
    pub fn abandon_plane(&mut self, seat: SeatId) -> WorldResult<PlaneId> {
        let index = self.lost_cockpit_of(seat)?;
        let plane = self.cockpits[index].plane;
        self.combat.cancel_for(plane.0);
        self.roster
            .abandon_plane(seat)
            .ok_or_else(|| format!("seat {} flies no plane", seat.0))?;
        self.revival.lost.push(LostPlane {
            plane,
            abandoned: self.tick(),
            resting_since: None,
        });
        self.refresh_friendlies();
        Ok(plane)
    }

    /// The next plane id: one past every plane the mission has had, retired
    /// ones included, and past any combat row's id.
    fn next_plane_id(&self) -> WorldResult<PlaneId> {
        let highest = self
            .roster
            .planes()
            .iter()
            .chain(&self.revival.retired)
            .map(|plane| plane.id.0)
            .max();
        let mut id = highest.map_or(Some(0), |id| id.checked_add(1));
        while let Some(candidate) = id {
            if !self
                .combat
                .state
                .targets
                .iter()
                .any(|target| target.id == candidate)
                && self.combat.state.ownship(candidate).is_none()
            {
                return Ok(PlaneId(candidate));
            }
            id = candidate.checked_add(1);
        }
        Err("the mission has no plane id left".into())
    }

    /// The next free member number of `wing`: one past every member any
    /// plane of the wing has had.
    fn next_member(&self, wing: tore_sim::ai::launch::WingId) -> WorldResult<u8> {
        let highest = self
            .roster
            .planes()
            .iter()
            .chain(&self.revival.retired)
            .filter(|plane| plane.slot.wing == wing)
            .map(|plane| plane.slot.member)
            .max();
        match highest.map_or(Some(0), |member| member.checked_add(1)) {
            Some(member) if member < u8::MAX => Ok(member),
            _ => Err("the wing has no member number left".into()),
        }
    }

    /// The aircraft `plane` is.
    fn aircraft_of(&self, plane: PlaneId) -> Option<AircraftId> {
        self.combat
            .state
            .ownship(plane.0)
            .map(|own| own.configuration().aircraft)
            .or_else(|| {
                self.ai_wings
                    .as_ref()
                    .and_then(|wings| wings.slot(plane.0))
                    .map(|slot| slot.aircraft)
            })
    }

    /// The configuration an aircraft's standard load is built from.
    fn standard_configuration(&self, aircraft: AircraftId) -> WorldResult<&live::Configuration> {
        self.combat
            .dummy_configurations()
            .iter()
            .find(|config| config.aircraft == aircraft)
            .ok_or_else(|| {
                format!("the mission holds no configuration for {}", aircraft.pt()).into()
            })
    }

    /// The weapon record named `name` that some aircraft of the mission
    /// carries: a standard load's, an AI aircraft's or a human's.
    fn weapon_record(&self, name: &str) -> Option<&Weapon> {
        let standard = self.combat.dummy_configurations().iter();
        let ai = self.ai_wings.iter().flat_map(|wings| {
            wings
                .slots()
                .iter()
                .filter_map(|slot| wings.configuration(slot.id))
        });
        let human = self
            .combat
            .state
            .ownships()
            .iter()
            .map(live::Ownship::configuration);
        standard
            .chain(ai)
            .chain(human)
            .flat_map(|config| &config.stations)
            .map(|station| &station.weapon)
            .find(|weapon| weapon.source == name)
    }

    /// `load` put onto `aircraft`'s standard configuration, as
    /// [`LoadoutSpec::apply`] puts it, with each weapon the standard load
    /// does not carry taken from the records the mission already holds
    /// (every loadout a revival names was flown in this mission, or is the
    /// standard one). Returns the configuration and each station's quantity.
    pub fn loadout_configuration(
        &self,
        aircraft: AircraftId,
        load: &LoadoutSpec,
    ) -> WorldResult<(live::Configuration, Vec<u16>)> {
        let mut config = self.standard_configuration(aircraft)?.clone();
        if load.stations.len() != config.stations.len() {
            return Err(format!(
                "the loadout has {} stations but {} has {}",
                load.stations.len(),
                aircraft.pt(),
                config.stations.len()
            )
            .into());
        }
        for (station, wanted) in config.stations.iter_mut().zip(&load.stations) {
            if station.weapon.source != wanted.weapon {
                station.weapon = self
                    .weapon_record(&wanted.weapon)
                    .ok_or_else(|| format!("the mission holds no record of {}", wanted.weapon))?
                    .clone();
            }
            station.count = wanted.count;
        }
        let quantities = load.stations.iter().map(|s| s.quantity).collect();
        Ok((config, quantities))
    }

    /// `aircraft`'s internal fuel capacity, pounds.
    fn full_fuel(&self, aircraft: AircraftId) -> WorldResult<f64> {
        let kind = self
            .combat
            .dummy_types()
            .iter()
            .find(|kind| kind.profile.id == aircraft)
            .ok_or_else(|| format!("the mission holds no aircraft type {}", aircraft.pt()))?;
        Ok(kind.profile.fields["internalFuel"].number()? as f64)
    }

    /// The loadout a revived `aircraft` carries: `chosen` (the player's
    /// lobby loadout for that aircraft) or else its standard load, with full
    /// fuel, cut by `weapons`.
    pub fn revival_loadout(
        &self,
        aircraft: AircraftId,
        chosen: Option<&LoadoutSpec>,
        weapons: RevivalWeapons,
    ) -> WorldResult<LoadoutSpec> {
        let standard = || -> WorldResult<LoadoutSpec> {
            let config = self.standard_configuration(aircraft)?;
            Ok(LoadoutSpec {
                fuel_lbs: 0.,
                cheat: false,
                stations: config
                    .stations
                    .iter()
                    .map(|station| StationLoad {
                        weapon: station.weapon.source.clone(),
                        count: station.count,
                        quantity: station.count,
                    })
                    .collect(),
            })
        };
        let mut load = match chosen {
            Some(chosen) => chosen.clone(),
            None => standard()?,
        };
        let (config, _) = self.loadout_configuration(aircraft, &load)?;
        for (station, load) in config.stations.iter().zip(&mut load.stations) {
            load.quantity = weapons.cut(&station.weapon, load.quantity);
        }
        load.fuel_lbs = self.full_fuel(aircraft)?;
        Ok(load)
    }

    /// Every living aircraft's side and position, the humans' and the AI's:
    /// what [`point`] measures the battle from.
    pub fn living_aircraft(&self) -> Vec<(Side, [f64; 3])> {
        let mut living = Vec::new();
        for plane in self.roster.planes() {
            if plane.pilot == Pilot::Lost {
                continue;
            }
            let side = plane.slot.wing.side;
            match self.cockpits.iter().position(|c| c.plane == plane.id) {
                Some(index) => {
                    if !self.cockpit_lost(index) {
                        living.push((side, self.cockpits[index].flight.position));
                    }
                }
                None => {
                    let alive = self
                        .ai_wings
                        .as_ref()
                        .and_then(|wings| wings.mission().actor(plane.id.0))
                        .is_some_and(|actor| actor.alive());
                    let row = self
                        .combat
                        .state
                        .targets
                        .iter()
                        .find(|target| target.id == plane.id.0 && target.hp > 0);
                    if let (true, Some(row)) = (alive, row) {
                        living.push((side, row.position));
                    }
                }
            }
        }
        living
    }

    /// The mean position of `side`'s aircraft now, from their combat rows
    /// and cockpits: what a host records at the mission's first tick as
    /// where each side's wings started.
    pub fn side_mean(&self, side: Side) -> Option<[f64; 3]> {
        let positions: Vec<[f64; 3]> = self
            .roster
            .planes()
            .iter()
            .filter(|plane| plane.slot.wing.side == side)
            .filter_map(|plane| {
                self.cockpits
                    .iter()
                    .find(|c| c.plane == plane.id)
                    .map(|c| c.flight.position)
                    .or_else(|| {
                        self.combat
                            .state
                            .targets
                            .iter()
                            .find(|t| t.id == plane.id.0)
                            .map(|t| t.position)
                    })
            })
            .collect();
        (!positions.is_empty()).then(|| {
            let n = positions.len() as f64;
            std::array::from_fn(|i| positions.iter().map(|p| p[i]).sum::<f64>() / n)
        })
    }

    /// Where a revival of `seat` appears now and what it carries: the
    /// [`point`] from `start` (its side's mean start) at `distance_ft`, at the
    /// mission's airborne start altitude (raised clear of the ground as the
    /// mission's airborne spawns are), at its aircraft's airborne start
    /// speed, with [`Self::revival_loadout`].
    pub fn revival_spawn(
        &self,
        seat: SeatId,
        start: [f64; 3],
        distance_ft: f64,
        chosen: Option<&LoadoutSpec>,
        weapons: RevivalWeapons,
    ) -> WorldResult<Spawn> {
        let index = self.lost_cockpit_of(seat)?;
        let plane = self.cockpits[index].plane;
        let aircraft = self
            .aircraft_of(plane)
            .ok_or_else(|| format!("plane {} has no aircraft", plane.0))?;
        let altitude = self.setup.mission.map_or(10_000., |(altitude, _)| altitude);
        let (mut position, heading_rad) =
            point(&self.living_aircraft(), start, distance_ft, altitude);
        let floor = combat::SPAWN_MIN_MSL_FT.max(
            f64::from(self.terrain.height(position[0] as f32, position[2] as f32))
                + combat::SPAWN_MIN_AGL_FT,
        );
        position[1] = position[1].max(floor);
        let kind = self
            .combat
            .dummy_types()
            .iter()
            .find(|kind| kind.profile.id == aircraft)
            .ok_or_else(|| format!("the mission holds no aircraft type {}", aircraft.pt()))?;
        let speed_fps = flight::State::new(&kind.profile, position)?.speed;
        Ok(Spawn {
            position,
            heading_rad,
            speed_fps,
            loadout: self.revival_loadout(aircraft, chosen, weapons)?,
        })
    }

    /// The oldest abandoned wreck that has rested on the ground for
    /// [`RETIRE_AFTER_TICKS`], which a revival may retire.
    pub fn retirable(&self) -> Option<PlaneId> {
        let now = self.tick();
        self.revival
            .lost
            .iter()
            .find(|lost| {
                lost.resting_since
                    .is_some_and(|since| now.saturating_sub(since) >= RETIRE_AFTER_TICKS)
            })
            .map(|lost| lost.plane)
    }

    /// Whether one more plane fits the mission now, retiring a wreck if one
    /// may go.
    pub fn room_for_one(&self) -> bool {
        self.roster.planes().len() < MAX_PLANES || self.retirable().is_some()
    }

    /// Everything a revival of `seat` at `spawn` checks before it changes
    /// anything.
    fn revival_check(&self, seat: SeatId, spawn: &Spawn) -> WorldResult<Plan> {
        let index = self.lost_cockpit_of(seat)?;
        let old = self.cockpits[index].plane;
        let wing = self
            .roster
            .plane(old)
            .ok_or_else(|| format!("plane {} is not in the mission", old.0))?
            .slot
            .wing;
        let aircraft = self
            .aircraft_of(old)
            .ok_or_else(|| format!("plane {} has no aircraft", old.0))?;
        let retire = if self.roster.planes().len() < MAX_PLANES {
            None
        } else {
            Some(
                self.retirable()
                    .ok_or("no room for another aircraft: no wreck may be retired yet")?,
            )
        };
        let new = NewPlane {
            plane: self.next_plane_id()?,
            slot: Slot {
                wing,
                member: self.next_member(wing)?,
            },
            aircraft,
            spawn: spawn.clone(),
        };
        self.add_check(&new)?;
        Ok(Plan { new, retire })
    }

    /// Whether `seat` can be revived at `spawn` now: it flies a lost plane,
    /// the mission has room (or a wreck to retire), and the new plane can be
    /// built.
    pub fn can_revive(&self, seat: SeatId, spawn: &Spawn) -> WorldResult<()> {
        self.revival_check(seat, spawn).map(|_| ())
    }

    /// What [`Self::revive_plane`] would add for `seat` at `spawn`: the
    /// new plane's id, slot and aircraft, for the host's Spawned message.
    pub fn revival_plane(&self, seat: SeatId, spawn: &Spawn) -> WorldResult<NewPlane> {
        self.revival_check(seat, spawn).map(|plan| plan.new)
    }

    /// Revives `seat`: retires a wreck first if the mission is full,
    /// abandons the seat's lost plane, adds a new plane of the same aircraft
    /// in its wing at `spawn` ([`Self::add_plane`]) and seats the seat in
    /// it by the handoff. Everything is checked first; a refusal changes
    /// nothing. Returns the new plane.
    pub fn revive_plane(&mut self, seat: SeatId, spawn: &Spawn) -> WorldResult<PlaneId> {
        let Plan { new, retire } = self.revival_check(seat, spawn)?;
        if let Some(wreck) = retire {
            self.retire_plane(wreck)?;
        }
        self.abandon_plane(seat)?;
        self.add_plane(&new)?;
        self.take_plane(seat, new.plane)?;
        Ok(new.plane)
    }

    /// The checks of [`Self::add_plane`].
    fn add_check(
        &self,
        new: &NewPlane,
    ) -> WorldResult<(live::Configuration, Vec<u16>, flight::State)> {
        if self.roster.plane(new.plane).is_some()
            || self.revival.retired.iter().any(|p| p.id == new.plane)
            || self
                .combat
                .state
                .targets
                .iter()
                .any(|target| target.id == new.plane.0)
            || self.combat.state.ownship(new.plane.0).is_some()
        {
            return Err(format!("plane {} is in the mission already", new.plane.0).into());
        }
        if self.ai_wings.is_none() {
            return Err("no AI flies this mission".into());
        }
        let kind = self
            .combat
            .dummy_types()
            .iter()
            .find(|kind| kind.profile.id == new.aircraft)
            .ok_or_else(|| format!("the mission holds no aircraft type {}", new.aircraft.pt()))?;
        let (config, quantities) = self.loadout_configuration(new.aircraft, &new.spawn.loadout)?;
        let spawn = &new.spawn;
        if !spawn.position.iter().all(|v| v.is_finite())
            || !spawn.heading_rad.is_finite()
            || !spawn.speed_fps.is_finite()
            || !spawn.loadout.fuel_lbs.is_finite()
        {
            return Err("a revival's place, heading, speed and fuel must be finite".into());
        }
        let mut state = flight::State::new(&kind.profile, spawn.position)?;
        state.yaw = spawn.heading_rad;
        state.pitch = 0.;
        state.bank = 0.;
        state.speed = spawn.speed_fps;
        state.velocity = Basis::new(spawn.heading_rad, 0., 0.)
            .forward
            .map(|v| v * spawn.speed_fps);
        Ok((config, quantities, state))
    }

    /// Adds a plane the mission did not start with: the AI flies it, built
    /// as the mission builds an AI aircraft with a lobby loadout (the
    /// spawn's stores and fuel), at the spawn's place, heading and speed,
    /// with its combat row and its roster entry. A client's copy of the
    /// mission adds each Spawned plane this way; the host's world does it
    /// inside [`Self::revive_plane`]. Refuses an id the mission has had,
    /// changing nothing.
    pub fn add_plane(&mut self, new: &NewPlane) -> WorldResult<()> {
        let (config, quantities, flight) = self.add_check(new)?;
        let id = new.plane.0;
        let side = new.slot.wing.side;
        let guns_only = self.setup.ai.as_ref().is_some_and(|ai| ai.guns_only);
        // The combat row, in id order, placed and moving as the flight is.
        let basis = Basis::new(new.spawn.heading_rad, 0., 0.);
        self.combat
            .state
            .add_dummy(&config, new.spawn.position, basis, row_side(side));
        let mut row = self
            .combat
            .state
            .targets
            .pop()
            .ok_or("combat did not add the row")?;
        row.id = id;
        row.velocity = flight.velocity;
        let at = self
            .combat
            .state
            .targets
            .partition_point(|target| target.id < id);
        self.combat.state.targets.insert(at, row);
        let wings = self.ai_wings.as_mut().ok_or("no AI flies this mission")?;
        if let Err(error) = wings.insert_new(ai_wings::NewAircraft {
            id,
            side,
            wing: new.slot.wing.index,
            member: new.slot.member,
            config,
            quantities,
            fuel_lbs: new.spawn.loadout.fuel_lbs,
            flight,
            guns_only,
        }) {
            self.combat.state.targets.remove(at);
            return Err(error);
        }
        self.roster
            .add_plane(Plane {
                id: new.plane,
                slot: new.slot,
                pilot: Pilot::Ai,
            })
            .map_err(std::io::Error::other)?;
        self.revival.added.push(new.plane);
        self.refresh_friendlies();
        Ok(())
    }

    /// Retires an abandoned wreck to make room: it leaves combat, the
    /// snapshots and the roster (its cockpit goes), and keeps its ledger
    /// entries; [`Book::retired`] keeps its roster entry for the results.
    pub fn retire_plane(&mut self, plane: PlaneId) -> WorldResult<()> {
        if self.roster.plane(plane).map(|p| p.pilot) != Some(Pilot::Lost) {
            return Err(format!("plane {} is not an abandoned wreck", plane.0).into());
        }
        let entry = self
            .roster
            .remove_plane(plane)
            .ok_or_else(|| format!("plane {} is not an abandoned wreck", plane.0))?;
        self.cockpits.retain(|cockpit| cockpit.plane != plane);
        self.combat.remove_ownship(plane.0);
        self.combat
            .state
            .targets
            .retain(|target| target.id != plane.0);
        self.revival.lost.retain(|lost| lost.plane != plane);
        self.revival.retired.push(entry);
        self.refresh_friendlies();
        Ok(())
    }

    /// The `ai-slot` respawn rule's weapons cut: what each station of the AI
    /// aircraft `plane` carries, cut by `weapons`, before a human takes it.
    /// Missiles keep everything.
    pub fn cut_ai_stores(&mut self, plane: PlaneId, weapons: RevivalWeapons) -> WorldResult<()> {
        if weapons == RevivalWeapons::Missiles {
            return Ok(());
        }
        self.ai_wings
            .as_mut()
            .ok_or("no AI flies this mission")?
            .cut_stores(plane.0, |weapon, rounds| {
                u32::from(weapons.cut(weapon, rounds.min(u32::from(u16::MAX)) as u16))
            })
    }

    /// Each tick, after combat and the AI: when each abandoned wreck came to
    /// rest (crashed and no longer falling, its pilot's escape over).
    pub(super) fn track_wrecks(&mut self) {
        if self.revival.lost.is_empty() {
            return;
        }
        let tick = self.tick();
        for lost in &mut self.revival.lost {
            let resting = self
                .cockpits
                .iter()
                .find(|cockpit| cockpit.plane == lost.plane)
                .is_none_or(|cockpit| {
                    let flight = &cockpit.flight;
                    flight.crashed
                        && flight
                            .wreck
                            .as_ref()
                            .is_none_or(|wreck| wreck.phase != tore_sim::wreck::Phase::Falling)
                        && flight.escape.as_ref().is_none_or(|escape| {
                            matches!(escape.phase, Phase::Landed | Phase::Impact)
                        })
                });
            if !resting {
                lost.resting_since = None;
            } else if lost.resting_since.is_none() {
                lost.resting_since = Some(tick);
            }
        }
    }
}

// Exact checkpoints (docs/formats/checkpoint.md): the revival section.
#[path = "revive_checkpoint.rs"]
mod checkpoint;

// The world tests of abandoning, reviving and retiring (slice F2-V).
#[cfg(test)]
#[path = "revive_tests.rs"]
mod revive_tests;

#[cfg(test)]
mod tests {
    use super::RevivalWeapons;

    #[test]
    fn revival_weapons_values_round_trip() {
        for rule in RevivalWeapons::ALL {
            assert_eq!(RevivalWeapons::from_value(rule.value()), Some(rule));
        }
        assert_eq!(RevivalWeapons::from_value(4), None);
    }
}
