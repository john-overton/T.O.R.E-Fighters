//! Authoritative dedicated-server recordings. Uses the game's pose conversion
//! and the ordinary replay writer, without renderer or sound dependencies.
pub mod bounds;
pub mod convert;
pub mod identity;
use crate::{BuildId, HostLog};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tore_replay::{self as replay, Frame, Writer};
use tore_world::{seats::PlaneId, world::World};

#[derive(Clone, Debug)]
pub struct Options {
    pub folder: PathBuf,
    pub kept: usize,
}

pub struct Recorder {
    writer: Writer,
    pending: Option<Frame>,
    options: Options,
    log: BufWriter<File>,
    registered: BTreeSet<u32>,
    weapons: BTreeMap<String, u32>,
    effects: convert::EffectWatch,
    surface_hp: BTreeMap<u32, i32>,
    stock: BTreeMap<(u32, u16), (u32, Option<u32>)>,
    stores: crate::client::convert::stores::StoresWatch,
    feedback_count: u64,
    marks: u64,
    city: Option<(u64, Vec<u32>)>,
}
impl Recorder {
    pub fn start(options: Options, world: &World, build: &BuildId) -> Result<Self, String> {
        fs::create_dir_all(&options.folder).map_err(|e| e.to_string())?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let stem = format!("server-{:020}-{:09}", now.as_secs(), now.subsec_nanos());
        let path = options.folder.join(format!("{stem}.tore-replay"));
        let snapshot = picture(world);
        let header = replay::Header {
            format_version: replay::FORMAT_VERSION,
            game_version: build.version.clone(),
            game_commit: build.commit.clone(),
            recorded_at: utc_text(UNIX_EPOCH + now),
            mission: replay::MissionKind::Other("dedicated-server".into()),
            world: identity::of(&world.terrain),
            extra: {
                let mut extra = convert::Presentation::of(&snapshot)
                    .with_player(convert::NO_PLAYER)
                    .extras();
                extra.push((
                    replay::model::FUEL_KEY.into(),
                    replay::model::FUEL_WITH_EXTERNAL.into(),
                ));
                extra
            },
        };
        let mut writer = Writer::create(path, &header).map_err(|e| e.to_string())?;
        for info in crate::client::convert::surface_infos(world) {
            writer
                .register_surface_unit(&info)
                .map_err(|e| e.to_string())?;
        }
        let log = BufWriter::new(
            File::create(options.folder.join(format!("{stem}.log"))).map_err(|e| e.to_string())?,
        );
        Ok(Self {
            writer,
            pending: None,
            options,
            log,
            registered: BTreeSet::new(),
            weapons: BTreeMap::new(),
            effects: Default::default(),
            surface_hp: BTreeMap::new(),
            stock: BTreeMap::new(),
            stores: Default::default(),
            feedback_count: 0,
            marks: 0,
            city: None,
        })
    }
    pub fn combat_events(&mut self, events: &[tore_sim::combat::live::Event]) {
        if let Some(frame) = &mut self.pending {
            frame.events.extend(
                events
                    .iter()
                    .map(|e| replay::Event::new("server.combat").with_text(format!("{e:?}"))),
            );
        }
    }
    pub fn note(&mut self, entry: &HostLog) {
        if let Some(frame) = &mut self.pending {
            if let HostLog::Feedback(f) = entry {
                frame.events.push(f.event());
                self.feedback_count += 1;
            } else {
                frame
                    .events
                    .push(replay::Event::new("server.log").with_text(format!("{entry:?}")));
            }
        }
    }
    fn flush(&mut self) -> Result<(), String> {
        if let Some(mut frame) = self.pending.take() {
            bounds::fit(&mut frame);
            for event in &frame.events {
                writeln!(
                    self.log,
                    "tick {}: {} {}",
                    frame.tick,
                    event.kind,
                    replay::feedback::Feedback::from_event(event)
                        .map_or_else(|| event.text.clone(), |f| f.text)
                        .replace(['\n', '\r'], " ")
                )
                .map_err(|e| e.to_string())?;
            }
            self.writer.push(&frame).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn tick(
        &mut self,
        world: &World,
        inputs: &[tore_world::seats::SeatInput],
    ) -> Result<(), String> {
        self.flush()?;
        let s = picture(world);
        let mut frame = Frame {
            tick: s.tick,
            ..Default::default()
        };
        for pose in std::iter::once(&s.player)
            .chain(&s.targets)
            .filter(|p| p.aircraft.is_some())
        {
            // The open world's player sentinel is not an aircraft in its roster.
            let Some(plane) = world.roster.plane(PlaneId(pose.id)) else {
                continue;
            };
            if self.registered.insert(pose.id) {
                let id = pose.aircraft.expect("filtered");
                self.writer
                    .register_aircraft(&replay::AircraftInfo {
                        id: pose.id,
                        pt: id.selection_key().into(),
                        name: id.label().into(),
                        label: world
                            .roster
                            .plane_callsign(plane.id)
                            .map_or_else(|| format!("Plane {}", plane.id.0), str::to_owned),
                        human: world.roster.seat_of(plane.id).is_some(),
                        side: match plane.slot.wing.side {
                            tore_sim::ai::launch::Side::Friendly => replay::Side::Friendly,
                            tore_sim::ai::launch::Side::Enemy => replay::Side::Enemy,
                        },
                        wing: u16::from(plane.slot.wing.index),
                        member: u16::from(plane.slot.member),
                        ..Default::default()
                    })
                    .map_err(|e| e.to_string())?;
            }
            let actor = world
                .ai_wings
                .as_ref()
                .and_then(|w| w.mission().actor(pose.id));
            let flight = world
                .cockpits
                .iter()
                .find(|c| c.plane == plane.id)
                .map(|c| &c.flight)
                .or_else(|| actor.map(|a| a.flight()));
            let mut data =
                flight.map_or_else(convert::FlightData::default, |f| convert::FlightData {
                    airspeed: f.speed,
                    g: f.g,
                    fuel_lb: f.fuel + f.systems.external_lbs(),
                    controls: actor.map_or([0., 0., 0., f.throttle], |a| {
                        let p = a.last_input();
                        [p.pitch, p.roll, p.yaw, f.throttle]
                    }),
                    on_ground: f
                        .supported_at(world.terrain.surface(f.position[0], f.position[2]).height),
                    alive: !f.crashed
                        && pose.damage.hp > 0
                        && !f.systems.pilot.dead
                        && f.escape.is_none(),
                    ejected: f.escape.is_some(),
                    wreck_gone: f.wreck_gone(),
                });
            if let Some(input) = inputs
                .iter()
                .find(|i| world.roster.seat(i.seat).and_then(|s| s.plane) == Some(plane.id))
            {
                data.controls = [
                    input.pilot.pitch,
                    input.pilot.roll,
                    input.pilot.yaw,
                    data.controls[3],
                ];
            }
            frame.aircraft.push(convert::aircraft_state(pose, &data));
            frame
                .events
                .extend(self.stores.events(pose.id, &pose.stores));
        }
        for p in &s.projectiles {
            let shot = world
                .combat
                .state
                .projectiles
                .iter()
                .find(|shot| shot.id == p.id);
            let weapon = shot.and_then(|shot| {
                world
                    .aircraft_configuration(p.owner)
                    .map(|c| shot.weapon(c))
            });
            let id = if let Some(id) = self.weapons.get(&p.weapon) {
                *id
            } else {
                let id = self.weapons.len() as u32;
                let info = weapon.map_or_else(
                    || replay::WeaponInfo {
                        id,
                        source: p.weapon.clone(),
                        shape: p.shape.clone(),
                        name: p.weapon.clone(),
                        class: if p.gun {
                            replay::WeaponClass::Gun
                        } else {
                            replay::WeaponClass::Other
                        },
                    },
                    |w| convert::weapon_info(id, w),
                );
                self.writer
                    .register_weapon(&info)
                    .map_err(|e| e.to_string())?;
                self.weapons.insert(p.weapon.clone(), id);
                id
            };
            frame.projectiles.push(convert::projectile_state(
                p,
                id,
                shot.map_or(0, |p| p.age),
                shot.and_then(|p| p.guidance.as_ref()).map(convert::seeker),
            ));
        }
        for p in s.targets.iter().filter(|p| p.aircraft.is_none()) {
            if self.surface_hp.insert(p.id, p.damage.hp) != Some(p.damage.hp) {
                frame.surface_hp.push((p.id, p.damage.hp));
            }
        }
        frame.surface = convert::surface_states(&s.surface);
        for unit in &world.combat.surface.units {
            for (index, mount) in unit.mounts.iter().enumerate() {
                let key = (unit.id.0, index as u16);
                let value = (mount.loaded, mount.reserve);
                if self.stock.insert(key, value) != Some(value) {
                    frame.surface_stock.push(replay::SurfaceStock {
                        unit: key.0,
                        mount: key.1,
                        loaded: value.0,
                        reserve: value.1,
                    });
                }
            }
        }
        frame.debris = convert::debris_states(&s.debris);
        frame.debris_pieces = convert::debris_pieces(&s.debris);
        frame.escapees = s.pilots.iter().map(convert::escapee_state).collect();
        frame.new_effects = self.effects.started(&s.effects);
        for mark in &world.combat.state.marks {
            if mark.serial >= self.marks {
                frame.new_effects.push(convert::mark_spawn(mark));
                self.marks = mark.serial + 1;
            }
        }
        new_puffs(&world.combat, &mut frame.new_puffs);
        if let Some(fill) = &world.terrain.city_fill {
            city_change(fill, &mut self.city, &mut frame);
        }
        frame.checksum = frame
            .tick
            .is_multiple_of(120)
            .then(|| replay::state_checksum(&frame.aircraft));
        self.pending = Some(frame);
        Ok(())
    }
    pub fn finish(mut self) -> Result<PathBuf, String> {
        self.flush()?;
        self.log.flush().map_err(|e| e.to_string())?;
        let footer = replay::Footer {
            end_tick: self.writer.last_tick().unwrap_or(0),
            result: vec![("feedback".into(), self.feedback_count.to_string())],
        };
        let path = self.writer.finish(&footer).map_err(|e| e.to_string())?;
        retain(&self.options.folder, self.options.kept)?;
        Ok(path)
    }
}

/// The cached cockpit picture, or the observer's full world when nobody is
/// seated. The latter also needs art and effects that the wire does not send.
fn picture(world: &World) -> std::borrow::Cow<'_, tore_world::snapshot::RenderSnapshot> {
    if !world.cockpits.is_empty() {
        return std::borrow::Cow::Borrowed(world.combat.render_snapshot());
    }
    let mut picture = crate::wire::from_world::observer_picture(world);
    picture.models = world
        .combat
        .dummy_types()
        .iter()
        .map(|a| a.profile.id)
        .collect();
    picture.effects = world
        .combat
        .state
        .effects
        .iter()
        .map(|e| tore_world::snapshot::EffectPose {
            kind: e.kind,
            position: e.position,
            ticks: e.ticks,
            blast: e.blast,
            scale: e.scale,
        })
        .collect();
    let stores = world.station_stores();
    for pose in &mut picture.targets {
        if let Some(config) = world.aircraft_configuration(pose.id)
            && let Some(loads) = stores.get(&pose.id)
        {
            pose.stores = tore_world::snapshot::store_poses_of(config, loads);
        }
        if let Some(actor) = world
            .ai_wings
            .as_ref()
            .and_then(|w| w.mission().actor(pose.id))
        {
            let f = actor.flight();
            pose.engine.lit = f.engine && f.fuel > 0.;
            pose.engine.afterburner = f.afterburner_active();
            pose.engine.rates = f.auxiliary_rates;
        }
    }
    std::borrow::Cow::Owned(picture)
}

/// Only complete, readable recordings bearing our prefix are eligible. Flags
/// are read from the file, never inferred from its name. Logs rotate together.
pub fn retain(folder: &Path, kept: usize) -> Result<(), String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(folder).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|e| e == "tore-replay")
            && path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("server-"))
            && let Ok(r) = replay::Recording::open(&path)
            && r.complete()
            && r.problems().is_empty()
            && !r.flagged()
        {
            files.push(path);
        }
    }
    files.sort();
    let remove = files.len().saturating_sub(kept);
    for path in files.into_iter().take(remove) {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
        match fs::remove_file(path.with_extension("log")) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
fn new_puffs(combat: &tore_world::combat::Combat, out: &mut Vec<replay::PuffSpawn>) {
    use tore_sim::combat::smoke::Kind;
    out.clear();
    for (layer, smoke) in [
        (replay::LAYER_SMOKE, &combat.state.smoke),
        (replay::LAYER_CONTRAILS, &combat.contrails),
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
        // Release order, oldest first.
        out[start..].reverse();
    }
    out.truncate(tore_replay::limits::MAX_PUFFS_PER_TICK);
}

pub fn utc(time: SystemTime) -> [i64; 6] {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    [year, month, day, rest / 3_600, rest / 60 % 60, rest % 60]
}

pub fn utc_text(time: SystemTime) -> String {
    let [y, mo, d, h, mi, s] = utc(time);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

pub fn city_change(
    fill: &tore_world::city::CityFill,
    last: &mut Option<(u64, Vec<u32>)>,
    frame: &mut Frame,
) {
    let version = fill.version();
    if last.as_ref().is_some_and(|(seen, _)| *seen == version) {
        return;
    }
    let now = fill.flattened();
    let before = last.take().map(|(_, ids)| ids).unwrap_or_default();
    let fell: Vec<u32> =
        if now.len() < before.len() || before.iter().any(|id| now.binary_search(id).is_err()) {
            frame.city_stood = true;
            now.clone()
        } else {
            now.iter()
                .copied()
                .filter(|id| before.binary_search(id).is_err())
                .collect()
        };
    // Named by where each stands, so a viewer's rebuilt fill finds it
    // whatever its index there.
    frame.city_fell = fell
        .iter()
        .filter_map(|&n| fill.layout.instances.get(n as usize))
        .map(|i| [i.x, i.z])
        .collect();
    *last = Some((version, now));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feedback_retention_keeps_flags_and_newest_unflagged_and_deletes_logs() {
        let folder =
            std::env::temp_dir().join(format!("tore-retain-feedback-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        for i in 0..6 {
            let path = folder.join(format!("server-{i:020}.tore-replay"));
            let mut writer = Writer::create(&path, &replay::Header::default()).unwrap();
            let events = if i == 1 || i == 4 {
                vec![
                    replay::feedback::Feedback {
                        text: "flag".into(),
                        ..Default::default()
                    }
                    .event(),
                ]
            } else {
                vec![]
            };
            writer
                .push(&Frame {
                    tick: 0,
                    events,
                    ..Default::default()
                })
                .unwrap();
            writer.finish(&replay::Footer::default()).unwrap();
            fs::write(path.with_extension("log"), "log").unwrap();
        }
        let corrupt = folder.join("server-000-broken.tore-replay");
        fs::write(&corrupt, b"broken").unwrap();
        retain(&folder, 2).unwrap();
        for i in 0..6 {
            let path = folder.join(format!("server-{i:020}.tore-replay"));
            assert_eq!(path.exists(), [1, 3, 4, 5].contains(&i));
            assert_eq!(path.with_extension("log").exists(), path.exists());
        }
        retain(&folder, 0).unwrap();
        assert!(corrupt.exists());
        assert!(
            folder
                .join("server-00000000000000000001.tore-replay")
                .exists()
        );
        fs::remove_dir_all(folder).unwrap();
    }
}
