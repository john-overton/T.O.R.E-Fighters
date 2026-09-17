//! Bounded combat-service replay. Records explicit launcher/environment inputs,
//! not a native replay or a replacement for the separate pilot-input tape.
use crate::{AppResult, terrain::World};
use std::{
    collections::BTreeMap,
    io::{BufRead, Write},
    path::Path,
};
use tore_sim::{
    attitude::{Basis, dot},
    combat::live::{Command, Configuration, Launcher, State},
    sensors::{Channel, Controls, RANGE_LADDER_NMI},
};

/// Version 4 defines the spec missile rules and records world velocity and bay
/// permission. Mode, heat and emitter changes are explicit commands. Versions
/// 2/3 retain compatibility rules and their original control defaults.
const VERSION: u32 = 4;

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
                " {} {} {} {} {} {} {} {} {} {}",
                u8::from(l.radar),
                u8::from(l.alive),
                u8::from(l.jammer),
                u8::from(l.controls.channel == Channel::Infrared),
                l.controls.range_index,
                u8::from(l.controls.history),
                l.velocity[0],
                l.velocity[1],
                l.velocity[2],
                u8::from(l.bay_ready)
            )
        })();
        if let Err(e) = result {
            self.error = Some(e);
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
pub fn command_name(c: Command) -> String {
    // A designation carries its stable target identity, never a screen
    // coordinate, so a replay selects the same object.
    if let Command::TargetHeat(value) = c {
        return format!("target-heat:{value}");
    }
    if let Command::DesignateTarget(id) = c {
        return format!("designate-id:{id}");
    }
    match c {
        Command::NextWeapon => "next",
        Command::ToggleSeekerMode => "seeker-mode",
        Command::CompatibilityWeapons => "compatibility-weapons",
        Command::ToggleTargetRadar => "target-radar",
        Command::TargetHeat(_) => unreachable!("handled above"),
        Command::Designate => "designate",
        Command::ClearDesignation => "clear",
        Command::ToggleArm => "arm",
        Command::Jettison => "jettison",
        Command::ReplaceTarget => "target",
        Command::CycleClass => "class",
        Command::FailStation => "fail",
        Command::DamagePlayer => "damage",
        Command::Incoming => "incoming",
        Command::ToggleTargetJammer => "target-jammer",
        Command::DesignateTarget(_) => unreachable!("handled above"),
    }
    .into()
}
pub fn command(s: &str) -> Option<Command> {
    if let Some(value) = s.strip_prefix("target-heat:") {
        return value
            .parse::<u8>()
            .ok()
            .filter(|v| *v <= 4)
            .map(Command::TargetHeat);
    }
    if let Some(id) = s.strip_prefix("designate-id:") {
        return id.parse().ok().map(Command::DesignateTarget);
    }
    [
        Command::NextWeapon,
        Command::ToggleSeekerMode,
        Command::CompatibilityWeapons,
        Command::ToggleTargetRadar,
        Command::Designate,
        Command::ClearDesignation,
        Command::ToggleArm,
        Command::Jettison,
        Command::ReplaceTarget,
        Command::CycleClass,
        Command::FailStation,
        Command::DamagePlayer,
        Command::Incoming,
        Command::ToggleTargetJammer,
    ]
    .into_iter()
    .find(|c| command_name(*c) == s)
}
/// Records carry the sensor controls from version 3 onward, so the field count
/// follows the header rather than being accepted either way.
fn fields_for(version: u32) -> usize {
    if version < 3 {
        17
    } else if version < 4 {
        20
    } else {
        24
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
    world: &World,
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
    )
}
fn replay_reader(
    mut reader: impl BufRead,
    config: Configuration,
    header: &str,
    ground: impl Fn(f64, f64) -> f64,
) -> AppResult<State> {
    use std::io::Read;
    let mut s = State::new(config, true)?;
    let mut buffer = Vec::new();
    let mut initialized = false;
    let mut version = VERSION;
    let mut ticks = 0;
    let mut bytes = 0;
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
            continue;
        }
        let (action, launcher) = parse(line, version)?;
        if !initialized && action != "reset" {
            return Err("combat tape must start with reset".into());
        }
        match action {
            "reset" => {
                s = State::new(s.configuration().clone(), true)?;
                s.weapon_rules = if version < 4 {
                    tore_sim::combat::missiles::Rules::Compatibility
                } else {
                    tore_sim::combat::missiles::Rules::Spec
                };
                s.range_target(launcher);
                initialized = true;
            }
            "release" => s.release(),
            "fire" | "tick" => {
                s.step(action == "fire", launcher, &ground);
                ticks += 1;
            }
            _ => s.command(
                command(action).ok_or("unknown combat tape action")?,
                launcher,
            ),
        }
    }
    Err("combat tape record bound".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_four_preserves_velocity_bay_mode_and_heat_commands() {
        let line = "tick 0 1000 0 1 0 0 0 1 0 0 0 1 600 1 1 0 0 1 0 40 60 600 0";
        let (_, launcher) = parse(line, 4).unwrap();
        assert_eq!(launcher.velocity, [40., 60., 600.]);
        assert!(!launcher.bay_ready);
        assert!(parse(&line.replace("40 60 600", "NaN 60 600"), 4).is_err());
        for c in [
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
        assert_eq!(command("designate-id:7"), Some(Command::DesignateTarget(7)));
        assert_eq!(command("designate-id:x"), None);
        assert_eq!(command_name(Command::DesignateTarget(7)), "designate-id:7");
        assert!(parse("tick NaN 1000 0 1 0 0 0 1 0 0 0 1 300 1 1 0", 2).is_err());
        assert!(parse("tick 0 1000 0 0 0 0 0 1 0 0 0 1 300 1 1 0", 2).is_err());
        assert!(parse("tick 0 1000 0 1 0 0 0 1 0 0 0 1 300 2 1 0", 2).is_err());
        assert!(parse("tick 0 1000 0 1 0 0 0 1 0 0 0 1 300 1 1 2", 2).is_err());
    }
}
