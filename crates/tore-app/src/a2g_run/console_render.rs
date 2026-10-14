//! `--console-scene DIR --console-at S[,S...]` for the AI strike driver
//! (slice nr-fltv): offscreen frames of the AC-130 sensor console
//! ([`crate::sensor_console`]) from the striking gunship's own sensor dome,
//! S seconds into the run, looking at the target. Each moment is drawn in
//! several forms: white hot and black hot tracking the target, the free
//! sensor with no target, the wide field, the sensor near and at its
//! gimbal limit, and the TV and night vision pictures. Day or night comes
//! from `--condition`.
//!
//! The picture is the world as the game draws it, thermally. The AI flies
//! the gunship without the player's gunsight, so the symbology is built here
//! from the same geometry the gunsight uses (each gun's arcs and reach from
//! the sensor's line to the target, the rounds from the gunship's own
//! stations); a development aid, not a check of the gunsight's numbers.
use super::{Scene, ai_render::Loaded};
use crate::{
    AppResult, gunsight_view,
    sensor_console::{GunLine, Mode, Pipper, Plan, Radar, SensorMode, Symbology, TargetLine},
};
use std::path::Path;
use tore_sim::{
    attitude::Vector,
    combat::{gunship, live::Readiness, sight},
};
use tore_world::seats::PlaneId;

/// One form of the console: name, look, zoom, picture mode, tracking.
type Form<'a> = (&'a str, [f64; 2], u8, SensorMode, bool);

/// The guns' reach in this harness: the shipped guns' 13,000 ft.
const REACH_FT: f64 = 13_000.;

/// Draws every form of the console for this moment into `dir`.
pub(super) fn render(
    scene: &mut Scene,
    observer: PlaneId,
    striker: PlaneId,
    target: u32,
    seconds: f64,
    dir: &Path,
) -> AppResult<()> {
    let flight = scene
        .world
        .ai_wings
        .as_ref()
        .and_then(|w| w.mission().actor(striker.0))
        .ok_or("the striker is not AI-flown")?
        .flight()
        .clone();
    let row = scene
        .world
        .combat
        .state
        .targets
        .iter()
        .find(|t| t.id == target)
        .ok_or("the target has no row")?
        .clone();
    let name = scene
        .world
        .combat
        .ground_name(target)
        .unwrap_or("UNKNOWN")
        .to_string();
    // The gunship's guns and their rounds: the AI's own stations.
    let (rounds, fitted) = {
        let wings = scene.world.ai_wings.as_ref().ok_or("no AI wings")?;
        let config = wings
            .configuration(striker.0)
            .ok_or("the striker has no configuration")?;
        let actor = wings
            .mission()
            .actor(striker.0)
            .ok_or("the striker is not AI-flown")?;
        let station = |slot: usize| {
            config
                .stations
                .iter()
                .position(|s| s.weapon.source.eq_ignore_ascii_case(gunship::GUNS[slot]))
        };
        let rounds: [Option<u32>; 3] = std::array::from_fn(|slot| {
            let index = station(slot)?;
            actor
                .stations()
                .iter()
                .find(|s| usize::from(s.station.0) == index)
                .map(|s| match s.store.rounds {
                    tore_sim::ai::weapon_service::Rounds::Finite(n) => n,
                    tore_sim::ai::weapon_service::Rounds::Unlimited => 9999,
                })
        });
        let fitted: [bool; 3] = std::array::from_fn(|slot| station(slot).is_some());
        (rounds, fitted)
    };
    let mut loaded = Loaded::new(&mut scene.world, observer)?;
    // The gunship's own airframe is not in its sensor's picture.
    loaded.snapshot.targets.retain(|t| t.id != striker.0);
    let launcher = crate::combat::launcher(&flight);
    let eye = gunship::eye_position(launcher);
    let offset: Vector = std::array::from_fn(|i| row.position[i] - eye[i]);
    let on_target = sight::body_angles(launcher, offset);
    let ground = |x: f64, z: f64| f64::from(scene.world.terrain.height(x as f32, z as f32));
    let agl_ft = flight.position[1] - ground(flight.position[0], flight.position[2]);
    let base = Symbology {
        sensor: SensorMode::WhiteHot,
        mode: Mode::Track,
        zoom: 3,
        fov_deg: gunship::field_of_view(3).to_degrees(),
        heading_deg: flight.yaw.to_degrees().rem_euclid(360.),
        agl_ft,
        airspeed_kt: flight.speed / 1.68781,
        look: on_target,
        los_deg: 0.,
        slant_ft: None,
        target: Some(TargetLine {
            name: tore_formats::text::upper(&name),
            range_ft: distance(row.position, flight.position),
            clock: crate::target_window::Readout::new(
                &tore_world::readout::TargetRow::of(&row),
                &flight,
                name.clone(),
            )
            .bearing,
            bearing_deg: (row.position[0] - flight.position[0])
                .atan2(row.position[2] - flight.position[2])
                .to_degrees()
                .rem_euclid(360.),
        }),
        guns: std::array::from_fn(|slot| GunLine {
            label: gunship::NAMES[slot],
            fitted: fitted[slot],
            linked: fitted[slot],
            candidate: slot == 2,
            rounds: rounds[slot],
            status: Readiness::Ready,
        }),
        lead: Some(2),
        trains: [on_target; 3],
        gimbal_limit: false,
        pipper: None,
        others: Vec::new(),
        mark: Some((row.position, true)),
        notice: None,
        orbit: None,
        designated: Some(row.position),
        // The AI flies without the player's pages: no A-G list here.
        radar: Radar {
            on: false,
            range_nm: 10.,
            dots: Vec::new(),
        },
        plan: Plan::new(launcher, agl_ft, on_target, [on_target; 3], None, Some(2)),
        blink: true,
    };
    // The forms: name, look, zoom, polarity, tracking, static level.
    let near = [on_target[0], -4_f64.to_radians()];
    let top = [on_target[0], gunship::GIMBAL_TOP];
    let forms: [Form; 8] = [
        ("whot", on_target, 3, SensorMode::WhiteHot, true),
        ("bhot", on_target, 3, SensorMode::BlackHot, true),
        ("tv", on_target, 3, SensorMode::Tv, true),
        ("nvg", on_target, 3, SensorMode::NightVision, true),
        ("free-wfov", on_target, 1, SensorMode::WhiteHot, false),
        ("narrow-e4", on_target, 5, SensorMode::WhiteHot, true),
        ("near-limit", near, 2, SensorMode::WhiteHot, false),
        ("at-limit", top, 2, SensorMode::WhiteHot, false),
    ];
    let stem = format!("console-t{seconds:05.1}s");
    for (label, look, zoom, mode, tracking) in forms {
        let mut sym = base.clone();
        sym.sensor = mode;
        sym.look = look;
        sym.zoom = zoom;
        sym.fov_deg = gunship::field_of_view(zoom).to_degrees();
        let line = sight::direction(launcher, look[0], look[1]);
        sym.los_deg = line[0].atan2(line[2]).to_degrees().rem_euclid(360.);
        sym.gimbal_limit = look[1] >= gunship::GIMBAL_TOP - 0.5_f64.to_radians();
        // The aim: the target when tracking, else where the line meets the
        // ground (the sky beyond reach when it does not).
        let aim = if tracking {
            Some(row.position)
        } else {
            sight::sight_ground(eye, line, &ground)
        };
        sym.slant_ft = aim.map(|a| distance(a, eye));
        if !tracking {
            sym.mode = Mode::Manual;
            sym.target = None;
            sym.mark = None;
        }
        // Each gun: inside its arcs and reach, or not.
        for slot in 0..3 {
            let (half_h, half_e) = (gunship::HEADING_ARC[slot], gunship::ELEVATION_ARC[slot]);
            let neutral = -std::f64::consts::FRAC_PI_2;
            let dh = sight::wrap(look[0] - neutral);
            let inside = dh.abs() <= half_h && look[1].abs() <= half_e;
            let reach = sym.slant_ft.is_some_and(|r| r <= REACH_FT);
            sym.guns[slot].status = match (inside, reach) {
                (false, _) => Readiness::GunArc,
                (true, false) => Readiness::MaximumRange,
                (true, true) => Readiness::Ready,
            };
            sym.trains[slot] = [
                neutral + dh.clamp(-half_h, half_h),
                look[1].clamp(-half_e, half_e),
            ];
        }
        sym.plan = Plan::new(launcher, agl_ft, look, sym.trains, aim, Some(2));
        sym.pipper = aim.map(|point| {
            let status = sym.guns[2].status;
            Pipper {
                point,
                range_arc: tore_sim::combat::gunsight::range_arc_fraction(
                    sym.slant_ft.unwrap_or(f64::INFINITY),
                    REACH_FT,
                ),
                dashed: status != Readiness::Ready,
                dot: status != Readiness::MaximumRange,
            }
        });
        let view = gunsight_view::free_view(&launcher, look, zoom);
        let mut camera = gunsight_view::camera(&view);
        camera.weather_slot = 0;
        camera.thermal = mode.thermal();
        camera.sensor = true;
        camera.near_clip = 1.;
        loaded.upload(&scene.world, &camera)?;
        let light = crate::sensor_console::night(&scene.world.terrain, camera.position[1]);
        let overlay =
            crate::sensor_console::draw(&sym, &camera, [crate::reel::WIDTH, crate::reel::HEIGHT]);
        let frame = crate::sensor_console::crt_frame(seconds, 0., mode, light);
        let pixels = loaded.gpu.console_pixels(
            &camera,
            &scene.world.terrain,
            &loaded.scenery,
            &frame,
            &overlay,
        )?;
        let path = dir.join(format!("{stem}-{label}.png"));
        super::ai_render::write_png(&path, &pixels)?;
        println!(
            "ai-strike: CONSOLE path={} look={:.1},{:.1} slant={:.0} status={}",
            path.display(),
            look[0].to_degrees(),
            look[1].to_degrees(),
            sym.slant_ft.unwrap_or(-1.),
            sym.status()
        );
    }
    Ok(())
}

fn distance(a: Vector, b: Vector) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

/// The console of the run's own seat (the player's path: the `orbit`
/// attack, the console opened with its seat command), `seconds` into the
/// run: the gunsight page, target, orbit status and A-G radar exactly as the
/// game builds them, in each picture mode, at the console radar's three
/// ranges, and with the sensor slewed off the designated target so its box
/// shows (`box-visible`, and off the picture, `box-edge`; a picture check:
/// the console's slew is nr-cona's).
/// `range` is the seat's radar range setting; `tag` names the moment.
pub(super) fn render_seat(
    scene: &mut Scene,
    seconds: f64,
    dir: &Path,
    range: usize,
    tag: &str,
) -> AppResult<()> {
    let observer = scene.world.cockpits[0].plane;
    let mut loaded = Loaded::new(&mut scene.world, observer)?;
    let world = &scene.world;
    let snapshot = world.combat.render_snapshot().clone();
    let frame = world
        .flight_frame(super::SEAT, None, &snapshot, &[])
        .ok_or("the seat flies no plane")?;
    let flight = frame.flight;
    let readout = &*frame.readout;
    let gunsight = readout
        .gunsight
        .as_ref()
        .ok_or("the seat's plane has no gunsight")?;
    let launcher = crate::combat::launcher(flight);
    let mut page = crate::instruments::gunsight::Page::new(readout, frame.config, flight)
        .ok_or("no gunsight page")?;
    let tracked = gunsight
        .sight
        .tracked()
        .and(readout.targets.display.as_ref());
    let view = gunsight_view::free_view(&launcher, gunsight.look, gunsight.zoom);
    page.present(
        gunsight.look,
        gunsight.zoom,
        view,
        tracked.map(|t| t.position),
    );
    let target = tracked.map(|row| {
        let name = world
            .combat
            .ground_name(row.id)
            .map_or_else(|| "TARGET".to_string(), str::to_string);
        crate::target_window::Readout::new(row, flight, name)
    });
    let designated = readout.targets.display.as_ref().map(|row| row.position);
    let ground = f64::from(
        world
            .terrain
            .height(flight.position[0] as f32, flight.position[2] as f32),
    );
    let air = world.terrain.air_data(flight).ok();
    let size = [crate::reel::WIDTH, crate::reel::HEIGHT];
    let shown = crate::sensor_console::radar_range_index(range);
    // Name, picture mode, the console's radar range, the sensor slewed off
    // the target (fields of view of heading: a third keeps the target in
    // the picture, two put it off the edge).
    let forms: [(&str, SensorMode, usize, f64); 9] = [
        ("whot", SensorMode::WhiteHot, shown, 0.),
        ("bhot", SensorMode::BlackHot, shown, 0.),
        ("tv", SensorMode::Tv, shown, 0.),
        ("nvg", SensorMode::NightVision, shown, 0.),
        ("radar-5nm", SensorMode::WhiteHot, 0, 0.),
        ("radar-10nm", SensorMode::WhiteHot, 1, 0.),
        ("radar-25nm", SensorMode::WhiteHot, 2, 0.),
        ("box-visible", SensorMode::WhiteHot, shown, 0.33),
        ("box-edge", SensorMode::WhiteHot, shown, 2.),
    ];
    for (label, mode, radar, slew) in forms {
        let field = tore_sim::combat::gunship::field_of_view(gunsight.zoom);
        let look = [gunsight.look[0] + slew * field, gunsight.look[1]];
        let view = gunsight_view::free_view(&launcher, look, gunsight.zoom);
        let mut camera = gunsight_view::camera(&view);
        camera.weather_slot = 0;
        camera.thermal = mode.thermal();
        camera.sensor = true;
        camera.near_clip = 1.;
        loaded.upload(world, &camera)?;
        let light = crate::sensor_console::night(&world.terrain, camera.position[1]);
        let mut sym = Symbology::live(
            &page,
            target.as_ref(),
            readout,
            frame.config,
            flight,
            air.as_ref(),
            ground,
            mode,
            seconds,
            designated,
            radar,
        );
        if slew != 0. {
            // The sensor turned away by hand: free, no track mark.
            sym.mode = Mode::Manual;
            sym.mark = None;
            sym.look = look;
        }
        let overlay = crate::sensor_console::draw(&sym, &camera, size);
        let crt = crate::sensor_console::crt_frame(seconds, 0., mode, light);
        let pixels =
            loaded
                .gpu
                .console_pixels(&camera, &world.terrain, &loaded.scenery, &crt, &overlay)?;
        let path = dir.join(format!("seat-{tag}t{seconds:05.1}s-{label}.png"));
        super::ai_render::write_png(&path, &pixels)?;
        println!(
            "orbit-run: CONSOLE path={} light={light:.4} gain={:.2} zoom={} dots={} radar={} designated={:?} status={}",
            path.display(),
            crt.gain,
            gunsight.zoom,
            sym.radar.dots.len(),
            sym.radar.range_nm,
            readout.targets.designated,
            sym.status()
        );
    }
    Ok(())
}
