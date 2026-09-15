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
                    "{}-{}s/{}-{}ft/flags={:#x}/effects={:?}",
                    l.start_seconds, l.end_seconds, l.low_feet, l.high_feet, l.flags, l.effects
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

    let layer = &environment.layer;
    let bytes = resources
        .get(layer)
        .ok_or_else(|| format!("mission layer {layer} is not imported"))?;
    let [hour, minute] = environment.time.unwrap_or([12, 0]);
    let parameter = environment.layer_parameter.unwrap_or(0);
    let configuration = Configuration::new(Module::parse(bytes)?, hour, minute, parameter)?;
    let records = configuration.layers().len();
    let mut state = Environment::new(configuration);

    // One simulated day at the host rate, sampling without advancing state.
    let mut uncovered = 0;
    let mut transitions = 0;
    let mut previous = state.sample(0.).map(|s| s.layer);
    let mut hazing = 0;
    for _ in 0..120 * 60 * 60 * 24 {
        state.step();
        let current = state.sample(0.).map(|s| s.layer);
        if current != previous {
            transitions += 1;
            previous = current;
        }
        if current.is_none() {
            uncovered += 1;
        }
        if state.sample(0.).is_some_and(|s| s.night_hazing()) {
            hazing += 1;
        }
    }
    if state.seconds_of_day() != (hour * 60 + minute) * 60 {
        return Err("weather clock did not return to its launch time after one day".into());
    }
    let visibility = state.effect(VISIBILITY, 0., 40_000.)?;
    println!(
        "{layer}: {records} records, launch {hour:02}:{minute:02}, parameter {parameter}, \
         {transitions} ground-layer transitions per day, {uncovered} uncovered ticks, \
         {hazing} night-hazing ticks, visibility effect {visibility}"
    );
    println!(
        "Weather sources validated: {modules} modules; clock, record selection and palette \
         expansion accepted. Altitude interpolation, celestial and cloud rendering remain open."
    );
    Ok(())
}
