//! Bounded combat-service replay: the tape file's writer and reader. Records
//! explicit launcher/environment inputs, not a native replay or a replacement
//! for the separate pilot-input tape. The names it records are `combat_tape`.
use crate::{
    AppResult,
    combat_tape::{Entry, airport_command, command},
    terrain::Terrain,
};
use std::{
    collections::BTreeMap,
    io::{BufRead, Write},
    path::Path,
};
use tore_sim::{
    attitude::{Basis, dot},
    combat::live::{Configuration, Launcher, State},
    sensors::{Channel, Controls, RANGE_LADDER_NMI},
};

/// Version 4 defines the spec missile rules and records world velocity and bay
/// permission. Mode, heat and emitter changes are explicit commands. Versions
/// 2/3 retain compatibility rules and their original control defaults.
const VERSION: u32 = 6;

pub struct Recorder {
    out: std::io::BufWriter<std::fs::File>,
    count: usize,
    error: Option<std::io::Error>,
}
fn fingerprint(data: &BTreeMap<String, Vec<u8>>) -> u64 {
    // Includes terrain and all imported dependencies. Stable FNV-1a, identity
    // guard only (not a cryptographic integrity guarantee).
    let mut h = 0xcbf29ce484222325u64;
    for (name, bytes) in data {
        for b in name.bytes().chain([0]).chain(bytes.iter().copied()) {
            h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
    }
    h
}
impl Recorder {
    pub fn new(
        path: &Path,
        data: &BTreeMap<String, Vec<u8>>,
        config: &Configuration,
        theater: &str,
    ) -> AppResult<Self> {
        let mut out = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?,
        );
        writeln!(
            out,
            "tore-combat {VERSION} {:?} {} {:016x}",
            config.aircraft,
            theater,
            fingerprint(data)
        )?;
        Ok(Self {
            out,
            count: 0,
            error: None,
        })
    }
    pub fn record(&mut self, action: &str, l: Launcher) {
        if self.error.is_some() {
            return;
        }
        self.count += 1;
        if self.count > 432000 {
            self.error = Some(std::io::Error::other("combat tape exceeds 432000 records"));
            return;
        }
        let result = (|| {
            write!(self.out, "{action}")?;
            for v in l
                .position
                .into_iter()
                .chain(l.basis.right)
                .chain(l.basis.up)
                .chain(l.basis.forward)
                .chain([l.speed_fps])
            {
                write!(self.out, " {v}")?;
            }
            writeln!(
                self.out,
                " {} {} {} {} {} {} {} {} {} {} {}",
                u8::from(l.radar),
                u8::from(l.alive),
                u8::from(l.jammer),
                u8::from(l.controls.channel == Channel::Infrared),
                l.controls.range_index,
                u8::from(l.controls.history),
                l.velocity[0],
                l.velocity[1],
                l.velocity[2],
                u8::from(l.bay_ready),
                u8::from(l.radar_power)
            )
        })();
        if let Err(e) = result {
            self.error = Some(e);
        }
    }
    /// Writes collected records, oldest first.
    pub fn write_all(&mut self, entries: impl IntoIterator<Item = Entry>) {
        for entry in entries {
            self.record(&entry.action, entry.launcher);
        }
    }
    pub fn flush(&mut self) -> AppResult<()> {
        if let Some(e) = self.error.take() {
            return Err(e.into());
        }
        self.out.flush()?;
        Ok(())
    }
}
/// Records carry the sensor controls from version 3 onward, so the field count
/// follows the header rather than being accepted either way.
fn fields_for(version: u32) -> usize {
    if version < 3 {
        17
    } else if version < 4 {
        20
    } else if version < 5 {
        24
    } else {
        25
    }
}
fn parse(line: &str, version: u32) -> AppResult<(&str, Launcher)> {
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() != fields_for(version) {
        return Err("invalid combat record fields".into());
    }
    let mut values = [0.; 13];
    for (out, text) in values.iter_mut().zip(&fields[1..14]) {
        *out = text.parse::<f64>()?;
        if !out.is_finite() || out.abs() > 1e9 {
            return Err("combat tape number outside bounds".into());
        }
    }
    let boolean = |s| match s {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err("invalid combat tape boolean"),
    };
    let basis = Basis {
        right: values[3..6].try_into()?,
        up: values[6..9].try_into()?,
        forward: values[9..12].try_into()?,
    };
    if [basis.right, basis.up, basis.forward]
        .iter()
        .any(|v| (dot(*v, *v) - 1.).abs() > 1e-6)
        || dot(basis.right, basis.up).abs() > 1e-6
        || dot(basis.right, basis.forward).abs() > 1e-6
        || dot(basis.up, basis.forward).abs() > 1e-6
        || !(0. ..=100000.).contains(&values[12])
    {
        return Err("invalid combat launcher basis/speed".into());
    }
    let controls = if version >= 3 {
        let range: usize = fields[18].parse()?;
        if range >= RANGE_LADDER_NMI.len() {
            return Err("combat tape scope range outside bounds".into());
        }
        Controls {
            channel: if boolean(fields[17])? {
                Channel::Infrared
            } else {
                Channel::Radar
            },
            range_index: range,
            history: boolean(fields[19])?,
        }
    } else {
        Controls::default()
    };
    Ok((
        fields[0],
        Launcher {
            position: values[..3].try_into()?,
            basis,
            bay_ready: version < 4 || boolean(fields[23])?,
            speed_fps: values[12],
            velocity: if version >= 4 {
                let mut velocity = [0.; 3];
                for (v, field) in velocity.iter_mut().zip(&fields[20..23]) {
                    *v = field.parse::<f64>()?;
                    if !v.is_finite() || v.abs() > 100000. {
                        return Err("invalid combat velocity".into());
                    }
                }
                velocity
            } else {
                basis.forward.map(|v| v * values[12])
            },
            radar_power: version < 5 || boolean(fields[24])?,
            radar: boolean(fields[14])?,
            jammer: boolean(fields[16])?,
            alive: boolean(fields[15])?,
            controls,
        },
    ))
}
pub fn replay(
    path: &Path,
    data: &BTreeMap<String, Vec<u8>>,
    config: Configuration,
    theater: &str,
    world: &Terrain,
) -> AppResult<State> {
    replay_reader(
        std::io::BufReader::new(std::fs::File::open(path)?),
        config.clone(),
        &format!(
            "tore-combat {VERSION} {:?} {} {:016x}",
            config.aircraft,
            theater,
            fingerprint(data)
        ),
        |x, z| f64::from(world.height(x as f32, z as f32)),
        Some(&world.airport_scene),
        world.wind(),
    )
}
/// Replay a tape recorded without an airport scene, as the headless combat
/// smoke records: the range fixture has no airfields, so a replay that added the
/// theater's airport targets and ILS service would not match the live state.
pub fn replay_without_airports(
    path: &Path,
    data: &BTreeMap<String, Vec<u8>>,
    config: Configuration,
    theater: &str,
    world: &Terrain,
) -> AppResult<State> {
    replay_reader(
        std::io::BufReader::new(std::fs::File::open(path)?),
        config.clone(),
        &format!(
            "tore-combat {VERSION} {:?} {} {:016x}",
            config.aircraft,
            theater,
            fingerprint(data)
        ),
        |x, z| f64::from(world.height(x as f32, z as f32)),
        None,
        world.wind(),
    )
}
fn replay_reader(
    mut reader: impl BufRead,
    config: Configuration,
    header: &str,
    ground: impl Fn(f64, f64) -> f64,
    airport_scene: Option<&tore_sim::airport::Scene>,
    wind: [f64; 3],
) -> AppResult<State> {
    use std::io::Read;
    let mut s = State::new(config, true)?;
    let mut buffer = Vec::new();
    let mut initialized = false;
    let mut version = VERSION;
    let mut ticks = 0;
    let mut bytes = 0;
    let mut airport_service = airport_scene
        .map(tore_sim::airport::Service::new)
        .transpose()
        .map_err(std::io::Error::other)?;
    let mut airport_nav = false;
    let mut airport_gear = false;
    let mut airport_supported = false;
    let register_airports = |state: &mut State, enabled: bool| -> AppResult<()> {
        if let Some(scene) = airport_scene.filter(|_| enabled) {
            for object in &scene.objects {
                state.add_ground_target(
                    object.id,
                    object.bounds,
                    object.hit_points,
                    object.category,
                )?;
                if let Some(target) = state
                    .targets
                    .iter_mut()
                    .find(|target| target.id == object.id)
                {
                    target.signature.radar = object.radar_signature;
                    target.signature.infrared = object.infrared_signature;
                }
            }
        }
        Ok(())
    };
    let mut last_aircraft = None;
    for count in 0..=432001 {
        buffer.clear();
        let n = reader.by_ref().take(4097).read_until(b'\n', &mut buffer)?;
        if n == 0 {
            if !initialized || ticks == 0 {
                return Err("empty combat replay".into());
            }
            println!(
                "combat replay: ticks={ticks} shots={} hits={} kills={} ammo={:?} player-hp={} damage={} subsystem={:?} visual-failed={} radar-failed={} ecm-failed={}",
                s.shots,
                s.hits,
                s.kills,
                s.ammo,
                s.player_hp,
                s.player_damage,
                s.last_subsystem,
                s.visual_failed,
                s.radar_failed,
                s.ecm_failed
            );
            if let (Some(service), Some(scene), Some(aircraft)) =
                (&airport_service, airport_scene, last_aircraft)
            {
                println!(
                    "airport replay: selected={:?} clearance={:?} reply={:?} guidance={:?}",
                    service.selected(),
                    service.clearance(),
                    service.last_reply(),
                    service.guidance(scene, aircraft)
                );
            }
            return Ok(s);
        }
        bytes += n;
        if n > 4096 || count > 432000 || bytes > 256 * 1024 * 1024 {
            return Err("combat tape exceeds bounds".into());
        }
        let line = std::str::from_utf8(&buffer)?.trim();
        if count == 0 {
            // Existing version-2 tapes keep replaying; only the version token
            // differs, and their records carry the default sensor controls.
            version = (2..=VERSION)
                .find(|v| {
                    line == header.replacen(
                        &format!("tore-combat {VERSION}"),
                        &format!("tore-combat {v}"),
                        1,
                    )
                })
                .ok_or("combat tape version/aircraft/theater/assets mismatch")?;
            if version < 6 {
                airport_service = None;
            }
            continue;
        }
        let (action, launcher) = parse(line, version)?;
        if !initialized && !matches!(action, "reset" | "reset-scene") {
            return Err("combat tape must start with reset".into());
        }
        match action {
            "reset" | "reset-scene" => {
                if action == "reset-scene" && version < 6 {
                    return Err("scene reset requires tape version6".into());
                }
                s = State::new(s.configuration().clone(), true)?;
                register_airports(&mut s, version >= 6)?;
                if let (Some(service), Some(scene)) = (&mut airport_service, airport_scene) {
                    service.reset(scene).map_err(std::io::Error::other)?;
                }
                airport_nav = false;
                airport_gear = false;
                airport_supported = false;
                s.weapon_rules = if version < 4 {
                    tore_sim::combat::missiles::Rules::Compatibility
                } else {
                    tore_sim::combat::missiles::Rules::Spec
                };
                if action == "reset" {
                    s.range_target(launcher);
                }
                initialized = true;
            }
            "release" => s.release(),
            "fire" | "tick" => {
                // The host sets the mission wind before every step; a tape does
                // not record it, so the replay is given the same wind.
                s.smoke.wind = wind;
                s.devices.wind = wind;
                let events = s.step(action == "fire", launcher, &ground);
                if let (Some(service), Some(scene)) = (&mut airport_service, airport_scene) {
                    let _ = events;
                    service.synchronize_health(
                        s.targets
                            .iter()
                            .filter(|target| {
                                target.role == tore_sim::combat::missiles::TargetRole::Surface
                            })
                            .map(|target| (target.id, target.hp)),
                    );
                    let aircraft = tore_sim::airport::Aircraft {
                        position: launcher.position,
                        forward: launcher.basis.forward,
                        nav_mode: airport_nav,
                        gear_down: airport_gear,
                        supported: airport_supported,
                        alive: launcher.alive,
                        speed_fps: launcher.speed_fps,
                        ground_clearance_ft: 0.,
                    };
                    service.step(scene, aircraft);
                    let _ = service.guidance(scene, aircraft);
                }
                ticks += 1;
            }
            "airport-nav:0" => airport_nav = false,
            "airport-nav:1" => airport_nav = true,
            _ if action.starts_with("airport-state:") => {
                let values: Vec<_> = action[14..].split(':').collect();
                if values.len() != 3 || values.iter().any(|v| !matches!(*v, "0" | "1")) {
                    return Err("invalid airport state record".into());
                }
                airport_nav = values[0] == "1";
                airport_gear = values[1] == "1";
                airport_supported = values[2] == "1";
            }
            _ if airport_command(action).is_some() => {
                let service = airport_service
                    .as_mut()
                    .ok_or("airport command without scene")?;
                let scene = airport_scene.unwrap();
                service.command(
                    scene,
                    tore_sim::airport::Aircraft {
                        position: launcher.position,
                        forward: launcher.basis.forward,
                        nav_mode: airport_nav,
                        gear_down: airport_gear,
                        supported: airport_supported,
                        alive: launcher.alive,
                        speed_fps: launcher.speed_fps,
                        ground_clearance_ft: 0.,
                    },
                    airport_command(action).unwrap(),
                );
            }
            _ => s.command(
                command(action).ok_or("unknown combat tape action")?,
                launcher,
            ),
        }
        last_aircraft = Some(tore_sim::airport::Aircraft {
            position: launcher.position,
            forward: launcher.basis.forward,
            nav_mode: airport_nav,
            gear_down: airport_gear,
            supported: airport_supported,
            alive: launcher.alive,
            speed_fps: launcher.speed_fps,
            ground_clearance_ft: 0.,
        });
    }
    Err("combat tape record bound".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat_tape::command_name;
    use tore_sim::combat::live::Command;
    fn launcher(x: f64) -> Launcher {
        Launcher {
            position: [x, 5000., 0.],
            basis: Basis::new(0.1, 0., 0.),
            speed_fps: 700.,
            velocity: [0., 0., 700.],
            bay_ready: true,
            radar_power: true,
            radar: true,
            jammer: false,
            alive: true,
            controls: Controls::default(),
        }
    }
    /// Records that combat collects and the app writes are the bytes a
    /// recorder writes when given the same records directly.
    #[test]
    fn collected_records_write_the_same_bytes() {
        let dir = std::env::temp_dir().join(format!("tore-tape-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = crate::ai_wings::tests::combat_fixture(false)
            .configuration()
            .clone();
        let data = BTreeMap::new();
        let names = ["tick", "fire", "release", "airport-state:1:0:1", "reset"];
        let mut direct = Recorder::new(&dir.join("direct"), &data, &config, "UKR").unwrap();
        let mut drained = Recorder::new(&dir.join("drained"), &data, &config, "UKR").unwrap();
        let mut entries = Vec::new();
        for (i, name) in names.into_iter().enumerate() {
            direct.record(name, launcher(i as f64));
            entries.push(Entry {
                action: name.into(),
                launcher: launcher(i as f64),
            });
        }
        drained.write_all(entries);
        direct.flush().unwrap();
        drained.flush().unwrap();
        assert_eq!(
            std::fs::read(dir.join("direct")).unwrap(),
            std::fs::read(dir.join("drained")).unwrap()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn version_five_preserves_power_separately_from_transmission() {
        let line = "tick 0 1000 0 1 0 0 0 1 0 0 0 1 600 0 1 0 1 1 0 40 60 600 1";
        assert!(parse(&format!("{line} 1"), 5).unwrap().1.radar_power);
        assert!(!parse(&format!("{line} 0"), 5).unwrap().1.radar_power);
        assert!(!parse(&format!("{line} 1"), 5).unwrap().1.radar);
        assert!(parse(&format!("{line} 2"), 5).is_err());
        assert!(parse(line, 4).unwrap().1.radar_power);
    }
    #[test]
    fn version_four_preserves_velocity_bay_mode_and_heat_commands() {
        let line = "tick 0 1000 0 1 0 0 0 1 0 0 0 1 600 1 1 0 0 1 0 40 60 600 0";
        let (_, launcher) = parse(line, 4).unwrap();
        assert_eq!(launcher.velocity, [40., 60., 600.]);
        assert!(!launcher.bay_ready);
        assert!(parse(&line.replace("40 60 600", "NaN 60 600"), 4).is_err());
        for c in [
            Command::NextSelection,
            Command::PreviousSelection,
            Command::SelectNav,
            Command::ToggleSeekerMode,
            Command::CompatibilityWeapons,
            Command::TargetHeat(4),
            Command::ToggleTargetRadar,
        ] {
            assert_eq!(command(&command_name(c)), Some(c));
        }
        assert_eq!(command("target-heat:5"), None);
    }
    #[test]
    fn bounded_records_reject_nonfinite_axes_and_invalid_basis() {
        assert!(parse("tick 0 1000 0 1 0 0 0 1 0 0 0 1 300 1 1 0", 2).is_ok());
        assert!(parse("tick 0 1000 0 1 0 0 0 1 0 0 0 1 300 1 1 0 1 3 1", 3).is_ok());
        assert!(parse("tick 0 1000 0 1 0 0 0 1 0 0 0 1 300 1 1 0 1 9 1", 3).is_err());
        assert!(parse("tick 0 1000 0 1 0 0 0 1 0 0 0 1 300 1 1 0 2 3 1", 3).is_err());
        // A record must match the version its header declared.
        assert!(parse("tick 0 1000 0 1 0 0 0 1 0 0 0 1 300 1 1 0 1 3 1", 2).is_err());
        assert!(parse("tick 0 1000 0 1 0 0 0 1 0 0 0 1 300 1 1 0", 3).is_err());
        assert!(parse("tick NaN 1000 0 1 0 0 0 1 0 0 0 1 300 1 1 0", 2).is_err());
        assert!(parse("tick 0 1000 0 0 0 0 0 1 0 0 0 1 300 1 1 0", 2).is_err());
        assert!(parse("tick 0 1000 0 1 0 0 0 1 0 0 0 1 300 2 1 0", 2).is_err());
        assert!(parse("tick 0 1000 0 1 0 0 0 1 0 0 0 1 300 1 1 2", 2).is_err());
    }
    #[test]
    fn version_five_and_six_records_have_the_same_field_count() {
        assert_eq!(fields_for(5), 25);
        assert_eq!(fields_for(6), 25);
    }
}
