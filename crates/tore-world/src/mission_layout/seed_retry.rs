//! Bounded host policy for layouts that cannot place every resolved vehicle.
use crate::mission::MissionSpec;

/// Agent-chosen bound for the lead's 2026-10-26 retry policy: sixteen layouts,
/// including the first.
pub const ATTEMPTS: usize = 16;

/// Try consecutive seeds after a vehicle-placement refusal. Preserve the caller's
/// spec on failure, and install only the seed whose complete build succeeded.
/// Clients never call this: they build the seed the host sends.
pub fn build<T>(
    spec: &mut MissionSpec,
    mut build: impl FnMut(&MissionSpec) -> Result<T, String>,
) -> Result<(T, usize), String> {
    let initial = spec.surface_seed;
    let mut candidate = spec.clone();
    for attempt in 1..=ATTEMPTS {
        match build(&candidate) {
            Ok(value) => {
                *spec = candidate;
                return Ok((value, attempt));
            }
            Err(error) => {
                if !candidate.lays_out_surface()
                    || !error.starts_with("Vehicle placement unavailable:")
                {
                    return Err(error);
                }
                if attempt == ATTEMPTS {
                    return Err(format!(
                        "{error} No valid placement in {ATTEMPTS} attempts starting at surface seed {initial}."
                    ));
                }
                candidate.surface_seed = candidate.surface_seed.wrapping_add(1).max(1);
            }
        }
    }
    unreachable!("the last attempt returns")
}

pub fn notice(initial: u32, selected: u32, attempts: usize) -> String {
    format!(
        "Surface placement seed {selected} selected on attempt {attempts} of {ATTEMPTS} (initial seed {initial})."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::aircraft::AircraftId;

    fn spec(seed: u32) -> MissionSpec {
        let mut spec = MissionSpec::new("UKR", AircraftId::F18);
        spec.ground_target = Some("QUCITY".into());
        spec.surface_seed = seed;
        spec
    }
    fn refused<T>() -> Result<T, String> {
        Err("Vehicle placement unavailable: synthetic footprint".into())
    }

    #[test]
    fn retries_in_fixed_order_and_installs_the_successful_seed_only() {
        for initial in [0, 7, u32::MAX] {
            let mut spec = spec(initial);
            let mut seen = Vec::new();
            let (chosen, attempts) = build(&mut spec, |candidate| {
                seen.push(candidate.surface_seed);
                if seen.len() < 3 {
                    refused()
                } else {
                    Ok(candidate.surface_seed)
                }
            })
            .unwrap();
            let next = initial.wrapping_add(1).max(1);
            assert_eq!(seen, [initial, next, next.wrapping_add(1).max(1)]);
            assert_eq!((spec.surface_seed, attempts), (chosen, 3));
        }
    }

    #[test]
    fn exhaustion_is_bounded_and_leaves_the_original_mission_intact() {
        let mut spec = spec(22);
        let original = spec.clone();
        let mut seen = Vec::new();
        let error = build::<()>(&mut spec, |candidate| {
            seen.push(candidate.surface_seed);
            refused()
        })
        .unwrap_err();
        assert_eq!(seen, (22..38).collect::<Vec<_>>());
        assert_eq!(spec, original);
        assert!(error.contains("16 attempts starting at surface seed 22"));
    }

    #[test]
    fn valid_seeds_and_unrelated_errors_do_not_retry() {
        let mut spec = spec(9);
        let mut calls = 0;
        assert_eq!(
            build(&mut spec, |_| {
                calls += 1;
                Ok(())
            }),
            Ok(((), 1))
        );
        assert_eq!(calls, 1);
        for error in [
            "Separation 300 nm does not fit this setup.",
            "Missing aircraft",
        ] {
            calls = 0;
            assert_eq!(
                build::<()>(&mut spec, |_| {
                    calls += 1;
                    Err(error.into())
                }),
                Err(error.into())
            );
            assert_eq!(calls, 1);
            assert_eq!(spec.surface_seed, 9);
        }
    }
}
