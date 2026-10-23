//! Headless motion probe with a continuously known, stationary intercept.
//! Retail movement records, flat ground, no seeker or damage random draws.
use crate::AppResult;
use tore_formats::weapons::Weapon;
use tore_sim::{
    attitude::{dot, unit},
    combat::missiles::{self, Motion, Profile, length, sub},
};

pub fn run(resources: &std::collections::BTreeMap<String, Vec<u8>>) -> AppResult<()> {
    println!("sag,weapon,height_ft,range_nm,miss_ft,end,predicted,sag_ticks");
    for name in ["AIM7.JT", "AIM9M.JT", "AIM120.JT", "SEA_SPAR.JT"] {
        let w = Weapon::parse(
            name,
            resources
                .get(name)
                .ok_or_else(|| format!("missing {name}"))?,
        )?;
        eprintln!(
            "sag record {name}: flags={:#x}, movement={:?}",
            w.flags, w.movement
        );
        let lifetime = Profile::for_weapon(&w)
            .ok_or("missing missile profile")?
            .guidance_ticks;
        for height in [300., 10_000.] {
            for range in [
                2., 4., 6., 8., 10., 12., 14., 16., 20., 30., 40., 50., 60., 80., 100., 120.,
            ] {
                let target = [0., height, range * 6076.];
                let start = [0., if name == "SEA_SPAR.JT" { 20. } else { height }, 0.];
                let mut forward = unit(sub(target, start));
                let speed = if name == "SEA_SPAR.JT" { 0. } else { 600. };
                let mut motion = Motion::launch(&w, forward.map(|v| v * speed), start[1]);
                let prediction = missiles::intercept(
                    &w.movement,
                    motion,
                    start,
                    forward,
                    target,
                    [0.; 3],
                    0,
                    lifetime,
                )
                .is_some();
                let mut position = start;
                let mut miss = length(sub(target, start));
                let mut end = "expiry";
                let mut sag_ticks = 0;
                for age in 0..lifetime.min(u64::from(w.movement.remove_t) * 30) {
                    let desired = missiles::commanded_heading(
                        forward,
                        motion.path(),
                        unit(sub(motion.aim(position, target), position)),
                    );
                    let next = missiles::steer(&w.movement, age, forward, desired);
                    motion.turn(forward, next);
                    forward = next;
                    let delta = motion.step(&w.movement, age, forward);
                    sag_ticks += usize::from(
                        motion.sink > 0.
                            && missiles::phase(&w.movement, age)
                                == tore_sim::combat::EnginePhase::Coast,
                    );
                    let previous = position;
                    position = std::array::from_fn(|i| position[i] + delta[i]);
                    // End at flat ground, measuring the swept segment up to impact.
                    if position[1] <= 0. {
                        let fraction = previous[1] / (previous[1] - position[1]);
                        position = std::array::from_fn(|i| previous[i] + delta[i] * fraction);
                        end = "ground";
                    }
                    let segment = sub(position, previous);
                    let relative = sub(target, previous);
                    let fraction =
                        (dot(relative, segment) / dot(segment, segment).max(1e-12)).clamp(0., 1.);
                    miss = miss.min(length(std::array::from_fn(|i| {
                        relative[i] - fraction * segment[i]
                    })));
                    if end == "ground" {
                        break;
                    }
                    if dot(sub(target, position), delta) < 0. {
                        end = "passed";
                        break;
                    }
                }
                println!("sag,{name},{height},{range},{miss:.3},{end},{prediction},{sag_ticks}");
            }
        }
    }
    Ok(())
}
