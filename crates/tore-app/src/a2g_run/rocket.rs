//! The `rocket` attack: a shallow dive with a rocket pod along the line that
//! puts the HUD's rocket pipper on the target, the trigger pressed (and held
//! `--hold` seconds) once the pipper says IN RNG (docs/spec/air-to-ground.md,
//! "Rockets and guns").
//!
//! It prints the sight every half second and at the press, and for every
//! rocket where it burst against the target and against the pipper of the
//! tick it was fired on, the check that the pipper is where a rocket lands.
use super::ccip::{Pass, Tracker, delivery, sight_line};
use super::{Options, Scene};
use crate::AppResult;
use tore_sim::combat::ballistics;

/// How near the target, along the pass, the pipper must be to fire.
const ON_TARGET_FT: f64 = 40.;

pub(super) fn run(scene: &mut Scene, o: &Options) -> AppResult<()> {
    let weapon = scene.station().weapon.clone();
    if !ballistics::is_pod(&weapon) {
        return Err(format!("{} is not a rocket pod", weapon.source).into());
    }
    let pass = Pass::new(scene, o)?;
    let hold = o.hold.max(1. / 120.);
    let mut tracker = Tracker::default();
    let mut pressed: Option<f64> = None;
    let mut next_line = 0.;
    for tick in 0..(o.seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        pass.fly(scene, t);
        scene.select();
        let sight = delivery(scene);
        if let Some(d) = &sight
            && t >= next_line
            && pressed.is_none()
        {
            println!("{}", sight_line(scene, d));
            next_line = t + 0.5;
        }
        // Holding the dive line keeps the pipper on the target; fire once it
        // is in range (IN RNG) with the pipper on the target, led.
        if pressed.is_none()
            && sight.as_ref().and_then(|d| d.ccip).is_some_and(|c| {
                c.in_range() && pass.past(scene, c.impact, c.seconds).abs() <= ON_TARGET_FT
            })
        {
            let d = sight.as_ref().expect("a sight");
            println!(
                "{}",
                sight_line(scene, d).replace("a2g-run: sight", "a2g-run: release")
            );
            pressed = Some(t);
        }
        let trigger = pressed.is_some_and(|at| t < at + hold);
        scene.step(trigger)?;
        // The pipper this tick's step worked out is the one a rocket fired on
        // this tick flies to.
        let pipper = delivery(scene).and_then(|d| d.ccip).map(|c| c.impact);
        tracker.update(scene, pipper);
        if pressed.is_some_and(|at| t > at + hold + 0.5) && !tracker.flying() {
            break;
        }
    }
    println!(
        "a2g-run: released {} rounds-end {}",
        tracker.released(),
        scene.rounds()
    );
    if pressed.is_none() {
        println!("a2g-run: no release");
    }
    Ok(())
}
