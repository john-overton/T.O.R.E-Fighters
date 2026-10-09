//! A networked flight's capture converted into a replay: when the flight
//! ends, in the background, and by hand with `--convert-capture CAPTURE
//! [OUT]` (docs/ARCHITECTURE.md, "Converting a capture into a replay";
//! docs/REPLAYS.md, "Network flights"). The conversion itself is
//! `tore_session::client::convert`; this module names the files, builds the
//! header's world from the mission and says what happened.
use super::{
    identity, library,
    net_effects::{self, NetEffects, Tally},
};
use crate::AppResult;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tore_session::client::convert::{self, FlightInfo, Written};

/// The import's resources, as the client session reads them.
pub type Resources = Arc<BTreeMap<String, Vec<u8>>>;

/// What converting one capture did.
#[derive(Debug)]
pub struct Converted {
    /// A replay for each flight the capture holds, in flight order.
    pub written: Vec<Written>,
    /// What the game added to each replay: the smoke, contrails and gun
    /// rounds the host does not send.
    pub effects: Vec<Tally>,
    /// The capture ended inside a record, or without an end, at this point.
    pub cut: Option<convert::Cut>,
}

/// The start of the capture a name like `2026-10-05_1540_NET_HOST.tore-capture`
/// holds, as the time of the replay's name and header.
fn started(capture: &Path) -> Option<SystemTime> {
    let name = capture.file_name()?.to_str()?;
    let (seconds, _) = library::order_key_for(name, crate::net::files::CAPTURE_EXTENSION)?;
    UNIX_EPOCH.checked_add(Duration::from_secs(u64::try_from(seconds).ok()?))
}

/// Where the replay of flight number `n` (from 0) goes: `out` for the first
/// and `out-2`, `out-3` and so on for the next, or, with no `out`, a name in
/// the capture's own folder like any recording's (the date and time of the
/// capture, the map and the aircraft; a clash adds `-2`).
fn destination(
    capture: &Path,
    out: Option<&Path>,
    n: usize,
    when: SystemTime,
    map: &str,
    aircraft: &str,
) -> PathBuf {
    if let Some(out) = out {
        if n == 0 {
            return out.to_path_buf();
        }
        let stem = out
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        let name = match out.extension() {
            Some(extension) => format!("{stem}-{}.{}", n + 1, extension.to_string_lossy()),
            None => format!("{stem}-{}", n + 1),
        };
        return out.with_file_name(name);
    }
    let folder = capture
        .parent()
        .map_or_else(PathBuf::new, Path::to_path_buf);
    let stem = format!(
        "{}_{}_{}",
        library::stamp(when),
        library::name_part(map),
        library::name_part(aircraft)
    );
    for k in 1..10_000 {
        let name = if k == 1 {
            format!("{stem}.tore-replay")
        } else {
            format!("{stem}-{k}.tore-replay")
        };
        let path = folder.join(name);
        if !path.exists() && !tore_replay::partial_path(&path).exists() {
            return path;
        }
    }
    folder.join(format!("{stem}-10000.tore-replay"))
}

/// Converts `capture` with the import `resources`. With `out` the first
/// flight's replay is exactly that file, which must not exist.
pub fn convert(
    capture: &Path,
    out: Option<&Path>,
    resources: Resources,
) -> Result<Converted, String> {
    let bytes = std::fs::read(capture)
        .map_err(|error| format!("{}: cannot read it: {error}", capture.display()))?;
    let conversion = convert::observe(&bytes, Arc::clone(&resources))
        .map_err(|error| format!("{}: {error}", capture.display()))?;
    let flights = conversion.flights();
    if flights.is_empty() {
        return Err(format!(
            "{}: {}",
            capture.display(),
            conversion.no_flight_reason()
        ));
    }
    let world = conversion
        .mission()
        .map(|world| identity::of(&world.terrain))
        .ok_or("the capture's mission did not build")?;
    // A name's time is the capture's; a capture with another name gets the
    // time of its last change in the name and no time in the header.
    let named = started(capture);
    let when = named
        .or_else(|| std::fs::metadata(capture).and_then(|m| m.modified()).ok())
        .unwrap_or(UNIX_EPOCH);
    let recorded_at = named.map_or_else(String::new, library::utc_text);
    let mut written = Vec::new();
    let mut tallies = Vec::new();
    for (n, flight) in flights.iter().enumerate() {
        let layout = world.layout.trim_end_matches(".MM").to_owned();
        let aircraft = flight
            .aircraft
            .map_or("X", |a| a.selection_key().trim_end_matches(".PT"));
        let path = destination(capture, out, n, when, &layout, aircraft);
        let header = conversion.header(
            flight,
            world.clone(),
            crate::version::version(),
            crate::version::commit(),
            &recorded_at,
        );
        let (done, tally) = write_flight(&conversion, flight, &header, &path, &resources)?;
        written.push(done);
        tallies.push(tally);
    }
    Ok(Converted {
        written,
        effects: tallies,
        cut: conversion.cut,
    })
}

fn write_flight(
    conversion: &convert::Conversion,
    flight: &FlightInfo,
    header: &tore_replay::Header,
    path: &Path,
    resources: &Resources,
) -> Result<(Written, Tally), String> {
    // The smoke, contrails and gun rounds the host does not send, made again
    // as a live client makes them (slice E2).
    let mut effects = conversion.mission().map(|world| {
        NetEffects::new(
            world,
            resources,
            net_effects::model_outlets(resources),
            header,
            &conversion.roster(flight),
            &conversion.weapons(flight),
        )
    });
    let written = match effects.as_mut() {
        Some(effects) => conversion.write_with(flight, header, path, effects),
        None => conversion.write(flight, header, path),
    };
    written
        .map(|done| (done, effects.map(|e| e.tally()).unwrap_or_default()))
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// What a conversion says in words, a line each.
pub fn report(capture: &Path, converted: &Converted) -> Vec<String> {
    let mut lines = Vec::new();
    for (n, done) in converted.written.iter().enumerate() {
        lines.push(format!(
            "Replay: {} ({} frames, {:.1} s, {} aircraft)",
            done.path.display(),
            done.frames,
            done.seconds,
            done.aircraft
        ));
        if let Some(tally) = converted.effects.get(n) {
            lines.push(format!(
                "Made again: {} smoke puffs, {} contrail puffs, {} gun rounds",
                tally.smoke, tally.contrails, tally.rounds
            ));
        }
    }
    if let Some(cut) = converted.cut {
        lines.push(format!(
            "{} is cut short: converted up to its last whole record, byte {} of {} ({:.1} s of the client's time).",
            capture.display(),
            cut.at_byte,
            cut.of_bytes,
            cut.seconds
        ));
    }
    lines
}

/// Converts `capture` on a thread of its own and logs what came of it. The
/// game starts it when a networked flight has ended and its capture is
/// closed; nothing waits for it.
pub fn spawn(capture: PathBuf, resources: Resources) {
    let spawned = std::thread::Builder::new()
        .name("net-convert".into())
        .spawn(move || match convert(&capture, None, resources) {
            Ok(converted) => {
                for line in report(&capture, &converted) {
                    log::info!("Network replay: {line}");
                }
            }
            Err(error) => log::warn!("Network replay not made: {error}"),
        });
    if let Err(error) = spawned {
        log::warn!("Network replay not made: cannot start its thread: {error}");
    }
}

/// `--convert-capture CAPTURE [OUT]`: converts with the import in the data
/// folder and prints where the replays went.
pub fn command(capture: &Path, out: Option<&Path>) -> AppResult<()> {
    let directory = crate::assets::data_directory()?;
    let loaded = tore_import::load_with(&directory, &|resources| {
        tore_import::check_markers(resources)
    })
    .map_err(|error| {
        format!(
            "There is no usable import in {} ({error}). Import Fighters Anthology with the game first.",
            directory.display()
        )
    })?;
    let converted = convert(capture, out, Arc::new(loaded.resources))?;
    for line in report(capture, &converted) {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::convert::{self, Presentation};
    use crate::replay::playback::Playback;
    use crate::replay::tests::TempDir;
    use tore_session::fixture::{Fight, bot_fight, bot_fight_as};

    /// A synthetic fight's capture in a folder, named as the game names it.
    fn capture_in(dir: &TempDir, fight: &Fight) -> PathBuf {
        let path = dir.path().join("2026-10-05_1540_NET_HOST.tore-capture");
        std::fs::write(&path, &fight.capture).unwrap();
        path
    }

    #[test]
    fn a_networked_flight_converts_and_plays_in_the_viewers_playback() {
        let fight = bot_fight(12);
        let dir = TempDir::new("net-convert");
        let capture = capture_in(&dir, &fight);
        let converted = convert(&capture, None, Arc::clone(&fight.resources)).unwrap();
        assert!(converted.cut.is_none());
        assert_eq!(converted.written.len(), 1);
        let written = &converted.written[0];
        // Named like any recording, beside the capture: the capture's date
        // and time, the map and the player's aircraft.
        let name = written
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        assert!(name.starts_with("2026-10-05_1540_"), "{name}");
        assert!(name.ends_with("_F18.tore-replay"), "{name}");
        assert!(library::is_recording_name(&name), "{name}");
        // The Replays screen lists it like any recording.
        let listed = library::Library::new(dir.path()).list();
        let _ = listed;

        let recording = Arc::new(tore_replay::Recording::open(&written.path).unwrap());
        assert!(recording.complete() && recording.problems().is_empty());
        let header = recording.header();
        assert_eq!(header.recorded_at, "2026-10-05T15:40:00Z");
        assert_eq!(header.extra("net.callsign"), Some("Alpha"));
        assert_eq!(header.mission.title(), "Network flight");
        let presentation = Presentation::from_header(header);
        assert_eq!(presentation.player, 0);
        assert!(presentation.slots >= 3 && presentation.models.len() == 1);

        // The viewer's own playback draws every tick: the player is plane 0
        // with the player's aircraft, and the other three are drawn.
        let first = recording.first_tick().unwrap();
        let last = recording.last_tick().unwrap();
        let mut playback = Playback::new(Arc::clone(&recording));
        for tick in [first, first + 200, (first + last) / 2, last] {
            let picture = playback.picture(tick, 1.);
            assert_eq!(picture.player.id, 0, "tick {tick}");
            assert!(picture.player.aircraft.is_some());
            assert_eq!(picture.targets.len(), 3, "tick {tick}");
            assert!(
                picture
                    .targets
                    .iter()
                    .all(|t| t.aircraft.is_some() && t.position[1] > 1000.),
                "tick {tick}"
            );
        }
        // The exports read it like any other recording.
        let dir_out = dir.path().join("log");
        std::fs::create_dir_all(&dir_out).unwrap();
        crate::replay::cli::log(
            &written.path,
            &crate::replay::cli::LogOptions {
                out: Some(dir_out.clone()),
                ..Default::default()
            },
        )
        .expect("the debug log writes");
        let summary = std::fs::read_to_string(dir_out.join("summary.txt")).unwrap();
        assert!(
            summary.contains("Alpha") || summary.contains("You"),
            "{summary}"
        );
        let acmi =
            crate::replay::cli::acmi(&written.path, None, None, false).expect("Tacview writes");
        let text = std::fs::read_to_string(acmi).unwrap();
        assert!(
            text.starts_with("\u{feff}FileType=text/acmi/tacview")
                || text.contains("FileType=text/acmi")
        );
    }

    /// The viewer follows the plane the seat flew (John, 2026-10-05): a
    /// player on plane 1 is plane 1 in the replay, in its picture, its
    /// tracks and its panels' roster, and plane 0 is another aircraft.
    #[test]
    fn a_player_on_plane_one_is_plane_one_in_the_replay_and_its_playback() {
        let fight = bot_fight_as(12, true);
        assert_eq!(fight.plane, 1);
        let dir = TempDir::new("net-convert-seat");
        let capture = capture_in(&dir, &fight);
        let converted = convert(&capture, None, Arc::clone(&fight.resources)).unwrap();
        let recording = Arc::new(tore_replay::Recording::open(&converted.written[0].path).unwrap());
        assert!(recording.complete() && recording.problems().is_empty());
        let header = recording.header();
        assert_eq!(header.extra(convert::PLAYER_KEY), Some("1"));
        assert_eq!(header.extra("net.player_plane"), Some("1"));
        let presentation = Presentation::from_header(header);
        assert_eq!(presentation.player, 1);
        assert_eq!(recording.aircraft_info(1).unwrap().label, "You");
        assert_ne!(recording.aircraft_info(0).unwrap().label, "You");

        let first = recording.first_tick().unwrap();
        let last = recording.last_tick().unwrap();
        let mut playback = Playback::new(Arc::clone(&recording));
        for tick in [first, (first + last) / 2, last] {
            let picture = playback.picture(tick, 1.);
            assert_eq!(picture.player.id, 1, "tick {tick}");
            assert!(picture.player.aircraft.is_some());
            assert_eq!(picture.targets.len(), 3, "tick {tick}");
            assert!(picture.targets.iter().any(|t| t.id == 0), "tick {tick}");
        }
        // The tracks' player view, which drives the weather, is plane 1's.
        let tracks = crate::replay::tracks::Tracks::scan(&recording);
        let at = (first + last) / 2;
        let mine = playback.aircraft(at, 1).unwrap().position;
        let view = tracks.view(at).unwrap().position;
        assert!((0..3).all(|i| (mine[i] - view[i]).abs() < 1e-9));
        // The Replays screen's details name the seat's aircraft.
        let details = crate::replay::screen::Details::read(&converted.written[0].path).unwrap();
        assert_eq!(details.player.as_deref(), Some("F/A-18D Hornet"));
    }

    #[test]
    fn the_replay_goes_where_out_says_and_never_over_a_file() {
        let fight = bot_fight(8);
        let dir = TempDir::new("net-convert-out");
        let capture = capture_in(&dir, &fight);
        let out = dir.path().join("mine.tore-replay");
        let converted = convert(&capture, Some(&out), Arc::clone(&fight.resources)).unwrap();
        assert_eq!(converted.written[0].path, out);
        assert!(out.exists());
        let again = convert(&capture, Some(&out), Arc::clone(&fight.resources)).unwrap_err();
        assert!(again.contains("already exists"), "{again}");
        // With no `--out` a second conversion is a second replay.
        let first = convert(&capture, None, Arc::clone(&fight.resources)).unwrap();
        let second = convert(&capture, None, Arc::clone(&fight.resources)).unwrap();
        assert_ne!(first.written[0].path, second.written[0].path);
        assert!(
            second.written[0]
                .path
                .to_string_lossy()
                .ends_with("-2.tore-replay")
        );
        let missing = convert(
            &dir.path().join("none.tore-capture"),
            None,
            Arc::clone(&fight.resources),
        );
        assert!(missing.unwrap_err().contains("cannot read it"));
        let bad = dir.path().join("2026-10-05_1541_NET_HOST.tore-capture");
        std::fs::write(&bad, b"not a capture at all").unwrap();
        let error = convert(&bad, None, Arc::clone(&fight.resources)).unwrap_err();
        assert!(error.contains("not a capture"), "{error}");
    }

    #[test]
    fn the_games_conversion_carries_gun_rounds_and_gives_the_same_bytes_twice() {
        let fight = bot_fight(12);
        let dir = TempDir::new("net-convert-twice");
        let capture = capture_in(&dir, &fight);
        let mut bytes = Vec::new();
        let mut converted = Vec::new();
        for name in ["a", "b"] {
            let out = dir.path().join(format!("{name}.tore-replay"));
            converted.push(convert(&capture, Some(&out), Arc::clone(&fight.resources)).unwrap());
            bytes.push(std::fs::read(out).unwrap());
        }
        assert!(
            bytes[0] == bytes[1],
            "the same capture gave different bytes"
        );
        assert_eq!(converted[0].effects, converted[1].effects);
        let recording = tore_replay::Recording::open(dir.path().join("a.tore-replay")).unwrap();
        let mut ids = std::collections::BTreeSet::new();
        for frame in recording.frames(0, u64::MAX) {
            ids.extend(frame.unwrap().projectiles.iter().map(|p| p.id));
        }
        assert!(ids.len() > 5, "the host's bursts are drawn: {}", ids.len());
        // The tally counts every round once and the report says so.
        assert_eq!(converted[0].effects[0].rounds, ids.len() as u64);
        let lines = report(&capture, &converted[0]);
        assert!(
            lines[1].starts_with("Made again: ")
                && lines[1].ends_with(&format!("{} gun rounds", ids.len())),
            "{lines:?}"
        );
        assert!(
            recording
                .weapons()
                .any(|w| w.class == tore_replay::WeaponClass::Gun)
        );
    }

    #[test]
    fn a_cut_capture_makes_a_replay_and_the_report_says_so() {
        let fight = bot_fight(10);
        let dir = TempDir::new("net-convert-cut");
        let cut = &fight.capture[..fight.capture.len() * 2 / 3];
        let capture = dir.path().join("2026-10-05_1542_NET_HOST.tore-capture");
        std::fs::write(&capture, cut).unwrap();
        let converted = convert(&capture, None, Arc::clone(&fight.resources)).unwrap();
        let lines = report(&capture, &converted);
        assert!(lines[0].starts_with("Replay: "), "{lines:?}");
        assert!(
            lines.iter().any(|l| l.contains("is cut short")),
            "{lines:?}"
        );
        let recording = tore_replay::Recording::open(&converted.written[0].path).unwrap();
        let footer = recording.footer().unwrap();
        assert!(footer.result.iter().any(|(k, v)| k == "end" && v == "cut"));
    }

    /// The session's mapping from a drawn pose to a replay aircraft equals the
    /// game's own recorder's, so a converted replay stores aircraft as a
    /// recorded one does.
    #[test]
    fn the_conversion_stores_an_aircraft_as_the_recorder_does() {
        use crate::snapshot::{AircraftPose, Damage, Draw, Engine};
        let pose = AircraftPose {
            id: 4,
            aircraft: Some(tore_formats::aircraft::AircraftId::F18),
            draw: Draw::Model(tore_formats::aircraft::AircraftId::F18),
            position: [10., 5000., -20.],
            attitude: [0.5, 0.1, -0.2],
            velocity: [300., 10., 50.],
            devices: Some([0.5; crate::snapshot::DEVICES]),
            engine: Engine {
                lit: true,
                afterburner: true,
                rates: [0.1, 0.2, 0.3],
                rotor: 1.043,
                flame: true,
            },
            damage: Damage {
                hp: 30,
                initial_hp: 100,
                sections: [1, 2, 3, 4, 5, 6],
                structural: Some(tore_sim::combat::live::DamageSection::Tail),
            },
            airborne: true,
            wreck: Some(tore_sim::wreck::Phase::Falling),
            crashed: false,
        };
        let data = convert::FlightData {
            airspeed: 310.,
            g: 2.5,
            fuel_lb: 1200.,
            controls: [0.1, -0.2, 0.3, 0.9],
            on_ground: false,
            alive: true,
            ejected: false,
            wreck_gone: false,
        };
        let ours = tore_session::client::convert::pose_state(
            &pose,
            &tore_session::client::convert::Flight {
                airspeed: data.airspeed,
                g: data.g,
                fuel_lb: data.fuel_lb,
                controls: data.controls,
                on_ground: data.on_ground,
                alive: data.alive,
                ejected: data.ejected,
                wreck_gone: data.wreck_gone,
            },
        );
        assert_eq!(ours, convert::aircraft_state(&pose, &data));
    }
}
