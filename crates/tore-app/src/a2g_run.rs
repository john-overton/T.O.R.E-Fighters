//! `--a2g-run THEATER STEM --aircraft ID --load REC --attack KIND [--from NM]
//! [--altitude FT] [--speed KT] [--dive DEG] [--heading DEG] [--seconds S]
//! [--target N] [--station N] [--count N] [--hold S] [--repeat N] [--gap S]
//! [--defenses AAA SAM] [--surface-seed N]`: a development aid and the
//! battery's driver for air-to-ground weapons (plan section 11.2). No window
//! opens; the whole mission tick runs as in the game.
//!
//! The player's aircraft flies a scripted straight pass at a ground target of
//! the Quick Mission `STEM` describes, invulnerable, and attacks it through
//! the real player path: the weapon keys select the station and the trigger
//! releases, never a projectile placed by hand. Each line starts `a2g-run:`.
//!
//! `--load REC` names the store. When the aircraft carries it the run uses
//! that station; otherwise it is fitted to `--station N` (default: the first
//! station carrying a bomb or a rocket pod), with `--count N` stores, for the
//! run only, as a Cheat loading would, so records the catalog does not offer
//! yet can be flown. `--target N` picks the Nth ground target (default 0).
//!
//! Attack kinds, one submodule each (each weapon slice adds its own):
//!
//! - `release` ([`release`]): a level pass (`--dive 0`) or a dive along
//!   `--dive DEG`, released when the bomb sight's mean impact reaches the
//!   target (bombs), or one second in (rockets). `--hold S` keeps the trigger
//!   down that long, `--repeat N` presses N times `--gap S` apart.
//! - `laser`, `jdam`, `at2` ([`guided`]): the guided weapons, released on
//!   the HUD's release range cue, with `--laser-off S`, `--buddy ID`,
//!   `--turn-away S` and `--condition C` of their own.
use crate::{AppResult, surface_dump};
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::{
    attitude::Vector,
    combat::{
        ballistics,
        live::{Launcher, Ownship, Station},
    },
};
use tore_world::{
    mission::{Defense, MissionSpec},
    seats::{SeatCommand, SeatId, SeatInput},
    world::{Seating, TickOutput, World},
};

mod guided;
mod release;

const SEAT: SeatId = SeatId(0);
pub(crate) const FEET_PER_NM: f64 = 6_076.;
pub(crate) const FPS_PER_KNOT: f64 = 1.687_8;

pub(crate) struct Options {
    theater: String,
    stem: String,
    aircraft: AircraftId,
    load: Option<String>,
    attack: String,
    pub from_nm: f64,
    pub altitude: f64,
    pub speed_kt: f64,
    pub dive_deg: f64,
    pub heading_deg: f64,
    pub seconds: f64,
    target: usize,
    station: Option<usize>,
    count: Option<u16>,
    pub hold: f64,
    pub repeat: usize,
    pub gap: f64,
    defenses: (usize, usize),
    seed: u32,
}

const USAGE: &str = "--a2g-run THEATER STEM --aircraft ID --load REC --attack KIND [--from NM] [--altitude FT] [--speed KT] [--dive DEG] [--heading DEG] [--seconds S] [--target N] [--station N] [--count N] [--hold S] [--repeat N] [--gap S] [--defenses AAA SAM] [--surface-seed N]";

fn options() -> AppResult<Options> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut o = Options {
        theater: String::new(),
        stem: String::new(),
        aircraft: AircraftId::A4E,
        load: None,
        attack: String::new(),
        from_nm: 4.,
        altitude: 5_000.,
        speed_kt: 450.,
        dive_deg: 0.,
        heading_deg: 0.,
        seconds: 90.,
        target: 0,
        station: None,
        count: None,
        hold: 0.,
        repeat: 1,
        gap: 0.5,
        defenses: (0, 0),
        seed: 1,
    };
    let mut positional = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut next = || -> AppResult<String> {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value").into())
        };
        match arg.as_str() {
            "--aircraft" => o.aircraft = AircraftId::parse(&next()?)?,
            "--load" => o.load = Some(record_name(&next()?)),
            "--attack" => o.attack = next()?,
            "--from" => o.from_nm = next()?.parse()?,
            "--altitude" => o.altitude = next()?.parse()?,
            "--speed" => o.speed_kt = next()?.parse()?,
            "--dive" => o.dive_deg = next()?.parse()?,
            "--heading" => o.heading_deg = next()?.parse()?,
            "--seconds" => o.seconds = next()?.parse()?,
            "--target" => o.target = next()?.parse()?,
            "--station" => o.station = Some(next()?.parse()?),
            "--count" => o.count = Some(next()?.parse()?),
            "--hold" => o.hold = next()?.parse()?,
            "--repeat" => o.repeat = next()?.parse()?,
            "--gap" => o.gap = next()?.parse()?,
            "--defenses" => o.defenses = (next()?.parse()?, next()?.parse()?),
            "--surface-seed" => o.seed = next()?.parse()?,
            // The guided kinds' own options, read by `guided`.
            other if guided::OPTIONS.contains(&other) => {
                next()?;
            }
            other if other.starts_with("--") => return Err(format!("unknown {other}").into()),
            other => positional.push(other.to_owned()),
        }
    }
    let [theater, stem] = positional.as_slice() else {
        return Err(USAGE.into());
    };
    if o.attack.is_empty() {
        return Err(USAGE.into());
    }
    o.theater = theater.to_ascii_uppercase();
    o.stem = stem.trim_start_matches('~').to_ascii_uppercase();
    Ok(o)
}

/// `mk82` or `MK82.JT` as the record's resource name.
fn record_name(name: &str) -> String {
    let upper = name.to_ascii_uppercase();
    if upper.ends_with(".JT") {
        upper
    } else {
        format!("{upper}.JT")
    }
}

pub fn run() -> AppResult<()> {
    let o = options()?;
    let mut resources = crate::reel::load_assets()?.theater_resources;
    surface_dump::with_retail_surface(&mut resources)?;
    if guided::KINDS.contains(&o.attack.as_str()) {
        return guided::run(&o, &resources);
    }
    let mut scene = Scene::new(&o, &resources)?;
    match o.attack.as_str() {
        "release" => release::run(&mut scene, &o)?,
        other => return Err(format!("unknown --attack {other}; kinds: release").into()),
    }
    scene.finish();
    println!("a2g-run: done");
    Ok(())
}

/// The world, the player's attack station and the ground target.
pub(crate) struct Scene {
    pub world: World,
    out: TickOutput,
    commands: Vec<SeatCommand>,
    pub station: usize,
    pub target: u32,
    pub owner: u32,
}

impl Scene {
    fn new(
        o: &Options,
        resources: &std::collections::BTreeMap<String, Vec<u8>>,
    ) -> AppResult<Self> {
        let mut spec = MissionSpec::new(&o.theater, o.aircraft);
        spec.ground_target = Some(o.stem.clone());
        spec.aaa = Defense::from_level(o.defenses.0).ok_or("defense levels are 0 to 3")?;
        spec.sam = Defense::from_level(o.defenses.1).ok_or("defense levels are 0 to 3")?;
        spec.surface_seed = o.seed;
        spec.cheats.damage = tore_sim::cheats::Damage::Invulnerable;
        spec.validate()?;
        let mut world = World::new(&spec, resources, Seating::SinglePlayer)?;
        if let Some(why) = &world.terrain.surface.unresolved {
            return Err(format!("the ground target stands nowhere: {why}").into());
        }
        let owner = world.combat.own_id();
        let targets = tore_world::ai_wings::outcome::ground_targets(
            &world.terrain.surface,
            &world.combat.state,
        );
        let target = *targets
            .get(o.target)
            .ok_or_else(|| format!("{} has {} ground targets", o.stem, targets.len()))?;
        let station = fit_store(&mut world, owner, o, resources)?;
        let scene = Self {
            world,
            out: TickOutput::default(),
            commands: Vec::new(),
            station,
            target,
            owner,
        };
        let row = scene.target_row().ok_or("the target has no row")?;
        let own = scene.world.combat.state.own();
        let s = &own.configuration().stations[station];
        println!(
            "a2g-run: setup aircraft {:?} station {station} {} count {} rounds {} target {target:#010x} at {:.0} {:.0} {:.0} hp {}",
            o.aircraft,
            s.weapon.source,
            s.count,
            own.rounds(station),
            row.position[0],
            row.position[1],
            row.position[2],
            row.hp
        );
        Ok(scene)
    }

    pub fn target_row(&self) -> Option<&tore_sim::combat::live::Target> {
        self.world
            .combat
            .state
            .targets
            .iter()
            .find(|t| t.id == self.target)
    }

    pub fn station(&self) -> &Station {
        &self.world.combat.state.own().configuration().stations[self.station]
    }

    pub fn rounds(&self) -> u16 {
        self.world.combat.state.own().rounds(self.station)
    }

    pub fn ground(&self, x: f64, z: f64) -> f64 {
        f64::from(self.world.terrain.height(x as f32, z as f32))
    }

    /// The launcher combat takes from the cockpit now.
    pub fn launcher(&self) -> Launcher {
        tore_world::combat::launcher(&self.world.cockpits[0].flight)
    }

    pub fn seconds(&self) -> f64 {
        self.world.combat.state.tick() as f64 / 120.
    }

    /// Puts the aircraft at `position`, its nose and velocity along
    /// `heading` and `pitch` (radians), at `speed` ft/s.
    pub fn fly(&mut self, position: Vector, heading: f64, pitch: f64, speed: f64) {
        let flight = &mut self.world.cockpits[0].flight;
        let forward = [
            heading.sin() * pitch.cos(),
            pitch.sin(),
            heading.cos() * pitch.cos(),
        ];
        flight.position = position;
        flight.velocity = forward.map(|v| v * speed);
        flight.speed = speed;
        flight.yaw = heading;
        flight.pitch = pitch;
        flight.bank = 0.;
        flight.vertical_speed = forward[1] * speed;
        flight.roll_rate = 0.;
        flight.pitch_rate = 0.;
    }

    /// One mission tick with the trigger as given and the queued commands.
    pub fn step(&mut self, trigger: bool) -> AppResult<()> {
        let input = SeatInput {
            seat: SEAT,
            tick: self.world.tick(),
            trigger,
            commands: std::mem::take(&mut self.commands),
            ..SeatInput::default()
        };
        self.world
            .step(std::slice::from_ref(&input), &mut self.out)?;
        Ok(())
    }

    /// Whether the attack station is selected and armed.
    pub fn selected(&self) -> bool {
        let own = self.world.combat.state.own();
        own.armed && own.selected == self.station
    }

    /// Queues the weapon key for the next step until the station is chosen.
    pub fn select(&mut self) {
        if !self.selected() && self.commands.is_empty() {
            self.commands
                .push(SeatCommand::CycleWeapon { forward: true });
        }
    }

    fn finish(&self) {
        let state = &self.world.combat.state;
        for kill in state
            .ledger
            .kills()
            .iter()
            .filter(|k| k.owner == self.owner)
        {
            println!(
                "a2g-run: kill {:#010x} class {:#06x}",
                kill.victim, kill.category
            );
        }
        if let Some(row) = self.target_row() {
            println!(
                "a2g-run: target {:#010x} hp {}/{}",
                row.id, row.hp, row.initial_hp
            );
        }
    }
}

/// The station the run attacks with, the store fitted to it when the
/// aircraft does not carry it (an agent decision for this development
/// driver: records the catalog does not list yet are flown as a Cheat
/// loading would carry them).
fn fit_store(
    world: &mut World,
    owner: u32,
    o: &Options,
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
) -> AppResult<usize> {
    let state = &mut world.combat.state;
    let own = state.ownship(owner).ok_or("no ownship")?;
    let mut config = own.configuration().clone();
    let carried = o.load.as_ref().and_then(|name| {
        config
            .stations
            .iter()
            .position(|s| s.weapon.source.eq_ignore_ascii_case(name))
    });
    let station = match (carried, o.station) {
        (_, Some(station)) => station,
        (Some(station), None) => station,
        (None, None) => config
            .stations
            .iter()
            .position(|s| ballistics::is_bomb(&s.weapon) || ballistics::is_pod(&s.weapon))
            .ok_or("the aircraft has no bomb or rocket station; give --station N")?,
    };
    if station >= config.stations.len() {
        return Err(format!("the aircraft has {} stations", config.stations.len()).into());
    }
    let fitted = match &o.load {
        Some(name)
            if !config.stations[station]
                .weapon
                .source
                .eq_ignore_ascii_case(name) =>
        {
            let bytes = resources
                .get(name)
                .ok_or_else(|| format!("missing {name}"))?;
            let mut weapon = Weapon::parse(name, bytes)?;
            tore_sim::combat::gunship::apply_tore_record(&mut weapon);
            config.stations[station].weapon = weapon;
            true
        }
        _ => false,
    };
    if let Some(count) = o.count {
        config.stations[station].count = count;
    }
    if fitted || o.count.is_some() {
        let (side, friendlies) = (own.side, own.friendlies.clone());
        state.remove_ownship(owner);
        let mut own = Ownship::new(owner, side, config, true)?;
        own.friendlies = friendlies;
        state.add_ownship(own)?;
        println!("a2g-run: fitted station {station}");
    }
    Ok(station)
}
