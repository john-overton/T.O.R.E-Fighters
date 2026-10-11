//! The `level` attack: the level release "T" flown through the player's own
//! controls (docs/spec/air-to-ground.md, "The level release T (LVL)").
//!
//! Holding at the start of the pass, the aircraft turns its radar to the
//! air-to-ground page (Ctrl+R) and designates the target from the page's list
//! (a scope click), the listed contact nearest the target when the column's
//! contacts merge. Then it selects the bomb (the weapon key), which starts in
//! LVL with a ground target ahead in level flight, and flies the pass with
//! the trigger held throughout: pickle and hold. Combat releases at the cue;
//! the driver never presses at a moment of its own. It prints the solution
//! every second, the release with its errors, OVERFLY once passed, and where
//! the bomb burst against the target and the record's splash radius.
use super::ccip::{Pass, Tracker, delivery, sight_line};
use super::{Options, Scene};
use crate::AppResult;
use tore_sim::combat::{
    delivery::{DeliveryMode, Warning},
    live::Command,
};
use tore_world::seats::SeatCommand;

/// Seconds the designation may take before the run gives up.
const DESIGNATE_SECONDS: f64 = 15.;

/// Turns the A-G page on and designates the target (or the listed contact
/// nearest it), holding the aircraft at the start of `pass`. Returns the
/// designated id.
pub(super) fn designate(scene: &mut Scene, pass: &Pass) -> AppResult<u32> {
    scene.queue(SeatCommand::Combat(Command::ToggleAirGround));
    let aim = pass.aim;
    for tick in 0..(DESIGNATE_SECONDS * 120.) as u64 {
        pass.fly(scene, 0.);
        let own = scene.world.combat.state.own();
        if let Some(id) = own.designated() {
            println!(
                "a2g-run: designated {id:#010x} {} after {:.2} s",
                scene.world.combat.ground_name(id).unwrap_or("unnamed"),
                tick as f64 / 120.
            );
            return Ok(id);
        }
        let nearest = own
            .sensors
            .ground()
            .iter()
            .min_by(|a, b| {
                let d = |p: [f64; 3]| (p[0] - aim[0]).hypot(p[2] - aim[2]);
                d(a.contact.position).total_cmp(&d(b.contact.position))
            })
            .map(|g| g.contact.id);
        if let Some(id) = nearest {
            scene.queue(SeatCommand::Combat(Command::DesignateTarget(id)));
        }
        scene.step(false)?;
    }
    Err("no ground contact to designate".into())
}

pub(super) fn run(scene: &mut Scene, o: &Options) -> AppResult<()> {
    let pass = Pass::new(scene, o)?;
    let id = designate(scene, &pass)?;
    if id != scene.target {
        scene.target = id;
        println!("a2g-run: aiming at the designated {id:#010x}");
    }
    // The pass runs over the designated object.
    let pass = Pass::new(scene, o)?;
    let w = &scene.station().weapon;
    let radius = f64::from(w.damage.collateral_radius.max(w.damage.fuze_radius).max(0));
    let mut tracker = Tracker::default();
    let mut released_at: Option<f64> = None;
    let mut overfly = false;
    let mut mode_said = false;
    let mut next_line = 0.;
    for tick in 0..(o.seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        pass.fly(scene, t);
        scene.select();
        let selected = scene.selected();
        if selected && !mode_said {
            let mode = scene.world.combat.state.own().delivery_mode;
            println!("a2g-run: mode {} on selection", mode.label());
            if mode != DeliveryMode::Level {
                scene.queue(SeatCommand::Combat(Command::ToggleSeekerMode));
                println!("a2g-run: ctrl-m to LVL");
            }
            mode_said = true;
        }
        if let Some(d) = delivery(scene) {
            if let Some(l) = d.level
                && t >= next_line
            {
                println!(
                    "a2g-run: level t={:.3} along {:.1} cross {:.1} rel {:.2} steer {:.3} marker {:.3} far {} slant {:.0}",
                    scene.seconds(),
                    l.along_ft,
                    l.cross_ft,
                    l.release_seconds,
                    l.steering_rad.to_degrees(),
                    l.marker_fraction(),
                    if l.far() { "yes" } else { "no" },
                    l.slant_ft
                );
                next_line = t + 1.;
            }
            if released_at.is_some() && !overfly && d.warnings.contains(&Warning::Overfly) {
                println!(
                    "a2g-run: overfly t={:.3} {}",
                    scene.seconds(),
                    sight_line(scene, &d)
                );
                overfly = true;
            }
        }
        let before = tracker.released();
        // Pickle and hold: the trigger is down from the selection on.
        scene.step(selected)?;
        tracker.update(scene, None);
        if tracker.released() > before {
            let d = delivery(scene).ok_or("no sight on the release tick")?;
            let l = d.level.ok_or("no level solution on the release tick")?;
            println!(
                "a2g-run: level-release t={:.3} along {:.1} cross {:.1} rel {:.3} radius {:.0} rounds {} count {}",
                scene.seconds(),
                l.along_ft,
                l.cross_ft,
                l.release_seconds,
                radius,
                scene.rounds(),
                tracker.released() - before
            );
            released_at.get_or_insert(t);
        }
        if released_at.is_some_and(|at| t > at + 3.) && overfly && !tracker.flying() {
            break;
        }
    }
    println!(
        "a2g-run: trigger held released {} radius {radius:.0} rounds-end {}",
        tracker.released(),
        scene.rounds()
    );
    if released_at.is_none() {
        println!("a2g-run: no release");
    }
    Ok(())
}
