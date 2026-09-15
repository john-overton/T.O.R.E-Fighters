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
                    "{}-{}s/{}-{}ft/flags={:#x}/effects={:?}/fog={}..{}ft {}..{}/see={}ft/shade={:?}/decks={:?}",
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
                    l.decks
                )
            })
            .collect();
        println!(
            "{name}: {} records [{}]",
            module.layers.len(),
            spans.join(" ")
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

    for name in ["F18.SH", "RAF.SH"] {
        if let Some(bytes) = resources.get(name) {
            let (code, _) = tore_formats::module::code(bytes)?;
            println!("PROBE {name} head: {:02x?}", &code[..0x40.min(code.len())]);
        }
    }
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
    if records > 1 && interpolated == 0 {
        return Err("no minute of the day fell inside a recovered transition window".into());
    }
    println!("  {interpolated} of 1440 minutes are inside a recovered transition");
    println!(
        "Weather sources validated: {modules} modules; clock, record selection and palette \
         expansion accepted. Celestial and cloud rendering remain open."
    );
    Ok(())
}
