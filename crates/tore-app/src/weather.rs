//! Bounded weather diagnostics over imported source modules. Read-only: this
//! prints what the reviewed readers resolve and never alters cached resources.
use crate::AppResult;
use std::collections::BTreeMap;
use tore_formats::weather::Module;
use tore_sim::environment::{Configuration, Environment, VISIBILITY};

/// Parses every imported `.LAY` module and exercises one full simulated day.
pub fn validate_sources(
    resources: &BTreeMap<String, Vec<u8>>,
    environment: &tore_formats::theater::Environment,
) -> AppResult<()> {
    let mut modules = 0;
    for (name, bytes) in resources.iter().filter(|(n, _)| n.ends_with(".LAY")) {
        let module = Module::parse(bytes).map_err(|e| format!("{name}: {e}"))?;
        let spans: Vec<String> = module
            .layers
            .iter()
            .map(|l| {
                format!(
                    "{}-{}s/{}-{}ft/flags={:#x}/effects={:?}/fog={}..{}ft {}..{}/see={}ft/shade={:?}/decks={:?}/callback={:?}/tint={:?}:{}",
                    l.start_seconds,
                    l.end_seconds,
                    l.low_feet,
                    l.high_feet,
                    l.flags,
                    l.effects,
                    f64::from(l.fog_near) * tore_formats::weather::DISTANCE_FEET,
                    f64::from(l.fog_far) * tore_formats::weather::DISTANCE_FEET,
                    l.fog_near_density,
                    l.fog_far_density,
                    f64::from(l.see_distance) * tore_formats::weather::DISTANCE_FEET,
                    l.shade,
                    l.decks,
                    l.callback,
                    l.tint,
                    l.tint_scalar
                )
            })
            .collect();
        println!(
            "{name}: {} records [{}]; light maps {}/{}",
            module.layers.len(),
            spans.join(" "),
            module.lighting.shade.len(),
            module.lighting.highlight.len()
        );
        // Every record must expand to a full palette without a bounds failure.
        for index in 0..module.layers.len() {
            module.palette(index).map_err(|e| format!("{name}: {e}"))?;
        }
        modules += 1;
    }
    if modules == 0 {
        return Err("no weather modules were imported".into());
    }

    for name in ["F18.SH", "RAF.SH", "CLOUD1.SH"] {
        if let Some(bytes) = resources.get(name) {
            let shape = tore_formats::shape::Shape::parse(bytes)?;
            let mut modes = [0usize; 3];
            for face in &shape.faces {
                modes[face.fog as usize] += 1;
            }
            let lit = shape
                .faces
                .iter()
                .filter(|f| f.subtype & 0x20 != 0 && f.normal.is_some())
                .count();
            println!(
                "{name}: fog-enabled/disabled/conditional faces {modes:?}, {lit} normal-lit faces"
            );
        }
    }

    for id in [
        tore_formats::aircraft::AircraftId::F18,
        tore_formats::aircraft::AircraftId::Rafale,
    ] {
        if let Some(bytes) = resources.get(id.hud()) {
            let hud = tore_formats::hud::Hud::parse(bytes)?;
            println!("{}: primary palette index {}", id.hud(), hud.primary_color);
        }
    }
    for name in ["SUN.SH", "MOON.SH", "STARS.SH", "CLOUD1.SH", "CLOUDS.SH"] {
        let bytes = resources
            .get(name)
            .ok_or_else(|| format!("weather dependency {name} missing"))?;
        if name == "CLOUD1.SH" {
            let shape = tore_formats::shape::Shape::parse(bytes)?;
            println!("{name}: {} source faces", shape.faces.len());
        } else {
            let shape = tore_formats::weather::shape::WeatherShape::parse(bytes)?;
            println!(
                "{name}: {} reviewed primitives, exponent {}, projected-point consumer {}",
                shape.primitives.len(),
                shape.scale_exponent,
                shape.publishes_point
            );
        }
    }

    for name in ["_MOON.PIC", "_CLOUD1.PIC"] {
        let pic = tore_formats::Pic::parse(resources.get(name).ok_or("weather texture missing")?)?;
        println!(
            "{name}: {}x{}, {} opaque pixels, index range {:?}",
            pic.width,
            pic.height,
            pic.mask.iter().filter(|v| **v).count(),
            pic.pixels
                .iter()
                .zip(&pic.mask)
                .filter(|(_, m)| **m)
                .map(|(p, _)| *p)
                .fold((255, 0), |(lo, hi), v| (lo.min(v), hi.max(v)))
        );
    }
    let clouds = tore_formats::weather::clouds::Layout::decode(
        resources
            .get("TORE_CLOUDS_V1")
            .ok_or("cloud layout missing")?,
    )?;
    let centers = tore_sim::clouds::centers(&clouds, [0.; 3], 10000);
    if centers.len() != clouds.patches.len() * (1usize << (2 * clouds.subdivisions)) {
        return Err("cloud repeat count mismatch".into());
    }
    println!(
        "Cloud layout: {} source descriptors, {} instances, base period {} feet; mission altitude {:?}",
        clouds.patches.len(),
        centers.len(),
        clouds.period_f8 / 256,
        environment.clouds
    );
    let flare = tore_formats::weather::flare::Layout::decode(
        resources
            .get("TORE_FLARE_V1")
            .ok_or("flare layout missing")?,
    )?;
    println!(
        "Lens flare: {} source circle descriptors; fills 265/266",
        flare.circles.len()
    );
    let layer = &environment.layer;
    let bytes = resources
        .get(layer)
        .ok_or_else(|| format!("mission layer {layer} is not imported"))?;
    let [hour, minute] = environment.time.unwrap_or([12, 0]);
    let parameter = environment.layer_parameter.unwrap_or(0);
    let configuration = Configuration::new(
        Module::parse(bytes)?,
        hour,
        minute,
        parameter,
        environment.wind,
    )?;
    let records = configuration.layers().len();
    let mut state = Environment::new(configuration);

    // One simulated day at the host rate, sampling without advancing state.
    let mut uncovered = 0;
    let mut hazing = 0;
    for _ in 0..120 * 60 * 60 * 24 {
        state.step();
        match state.sample(0.) {
            Some(ground) if ground.night_hazing() => hazing += 1,
            Some(_) => {}
            None => uncovered += 1,
        }
    }
    if state.seconds_of_day() != (hour * 60 + minute) * 60 {
        return Err("weather clock did not return to its launch time after one day".into());
    }
    if uncovered != 0 {
        return Err(format!("{layer}: {uncovered} ticks had no active weather record").into());
    }
    let visibility = state.effect(VISIBILITY, 0., 40_000.)?;
    println!(
        "{layer}: {records} records, launch {hour:02}:{minute:02}, parameter {parameter}, \
         {hazing} of {} ticks night-hazing, visibility effect {visibility}",
        120 * 60 * 60 * 24
    );

    // Selection depends only on time of day, so one launch per minute resolves
    // the whole cycle without running the clock. Interpolated instants are those
    // whose horizon color matches no stored record.
    let stored = state.configuration().layers().to_vec();
    let mut interpolated = 0;
    let mut previous = None;
    for minute in 0..24 * 60 {
        let probe = Configuration::new(
            Module::parse(bytes)?,
            minute / 60,
            minute % 60,
            parameter,
            environment.wind,
        )?;
        let Some(sample) = Environment::new(probe).sample(0.) else {
            return Err(format!("{layer}: {minute:02} has no active record").into());
        };
        let blended = !stored.iter().any(|s| s.shade == sample.shade);
        interpolated += usize::from(blended);
        let changed = previous.as_ref() != Some(&sample.shade);
        if changed || blended {
            println!(
                "  {:02}:{:02} flags={:#05x}{}{} haze={:?} fog={}..{}/256 clear-to={:.0}nm visibility={}",
                minute / 60,
                minute % 60,
                sample.flags,
                if sample.night_hazing() {
                    " night"
                } else {
                    "      "
                },
                if blended { " blend" } else { "      " },
                sample.shade,
                sample.fog_near_density,
                sample.fog_far_density,
                f64::from(sample.see_distance) * tore_formats::weather::DISTANCE_FEET / 6076.,
                sample.effect(VISIBILITY)?
            );
        }
        previous = Some(sample.shade);
    }
    let has_time_overlap = stored.windows(2).any(|pair| {
        pair[0].low_feet == pair[1].low_feet
            && pair[0].end_seconds > pair[1].start_seconds
            && pair[0].start_seconds != pair[1].start_seconds
    });
    if has_time_overlap && interpolated == 0 {
        return Err("no minute of the day fell inside a recovered transition window".into());
    }
    println!("  {interpolated} of 1440 minutes are inside a recovered transition");
    // Every recovered condition must resolve a module for this theater and
    // band correctly with altitude.
    for (index, choice) in tore_sim::environment::CONDITIONS.iter().enumerate() {
        let name = tore_sim::environment::layer_resource(index, &environment.map)?;
        let bytes = resources
            .get(&name)
            .ok_or_else(|| format!("condition {index} needs {name}, which is not imported"))?;
        let probe = Environment::new(Configuration::new(
            Module::parse(bytes)?,
            choice.seconds_of_day / 3600,
            choice.seconds_of_day / 60 % 60,
            index as i32,
            environment.wind,
        )?);
        let mut bands = Vec::new();
        for altitude in [500., 6_000., 30_000.] {
            let layer = probe
                .sample(altitude)
                .ok_or_else(|| format!("{name} covers no record at {altitude} ft"))?;
            let (density, _) = layer.visibility(20_000.);
            bands.push(format!("{:.0}ft {}/256", altitude, density));
        }
        println!(
            "  condition {index}: {name} at {:02}:{:02}, haze at 20,000 ft: {}{}",
            choice.seconds_of_day / 3600,
            choice.seconds_of_day / 60 % 60,
            bands.join(", "),
            if choice.scattered_clouds {
                "; may carry a scattered deck"
            } else {
                ""
            }
        );
    }
    println!(
        "Weather sources validated: {modules} modules; clock, record selection and palette \
         expansion accepted. Celestial/cloud readers and placement accepted; matched retail rendering acceptance remains open."
    );
    Ok(())
}
