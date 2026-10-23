//! The `cluster` attack: the `release` pass with a cluster bomb, then what
//! the burst did to the ground (docs/spec/ordnance-presentation.md, "Cluster
//! bombs"; plan decision 13).
//!
//! After the burst it prints the record's numbers, then one line for every
//! surface target with its distance from the burst, the hit points it lost
//! and whether the splash reaches it, and the presentation-only bomblet
//! flashes the renderer derives from the burst. Splash is damage in the
//! simulation; the flashes are not, and the lines show both.
use super::{Options, Scene, release};
use crate::{AppResult, effect_renderer::cluster as cluster_flash};
use tore_sim::{
    attitude::Vector,
    combat::{live::EffectKind, missiles::TargetRole},
};

/// Targets farther than this from the burst are not listed.
const NEAR_FT: f64 = 12_000.;

pub(super) fn run(scene: &mut Scene, o: &Options) -> AppResult<()> {
    let weapon = scene.station().weapon.clone();
    let before: Vec<(u32, i32)> = surface_targets(scene).map(|t| (t.id, t.hp)).collect();
    println!(
        "a2g-run: record {} flags {:#x} land-explosion {} object-explosion {} water-explosion {} crater {} splash-radius {} splash-percent {} fuze {} damage {}",
        weapon.source,
        weapon.flags,
        weapon.effects.land_explosion,
        weapon.effects.object_explosion,
        weapon.effects.water_explosion,
        weapon.effects.crater_size,
        weapon.damage.collateral_radius,
        weapon.damage.collateral_percent,
        weapon.damage.fuze_radius,
        weapon.damage.by_class.map(|d| d.to_string()).join(";"),
    );
    release::run(scene, o)?;
    let burst = scene
        .world
        .combat
        .state
        .effects
        .iter()
        .rev()
        .find(|e| matches!(e.kind, EffectKind::Ground | EffectKind::Hit))
        .cloned();
    let Some(burst) = burst else {
        println!("a2g-run: cluster no burst");
        return Ok(());
    };
    println!(
        "a2g-run: cluster burst at {:.1} {:.1} {:.1} blast {:?} ticks {}",
        burst.position[0], burst.position[1], burst.position[2], burst.blast, burst.ticks
    );
    let radius = f64::from(weapon.damage.collateral_radius);
    let mut hurt = 0;
    let mut reached = 0;
    let mut killed = 0;
    for target in surface_targets(scene) {
        // The scene holds the whole theater's ground objects; only the ones
        // near the target can be in a cluster bomb's splash.
        let distance = horizontal(target.position, burst.position);
        if distance > NEAR_FT {
            continue;
        }
        let was = before
            .iter()
            .find(|(id, _)| *id == target.id)
            .map_or(target.hp, |(_, hp)| *hp);
        let lost = was - target.hp;
        // A target's surface is its radius nearer than its centre.
        let inside = distance - target.radius < radius;
        println!(
            "a2g-run: cluster target {:#010x} distance {:.0} radius {:.0} lost {} hp {}/{} inside {}",
            target.id,
            distance,
            target.radius,
            lost,
            target.hp,
            target.initial_hp,
            if inside { "yes" } else { "no" },
        );
        if inside {
            reached += 1;
        }
        if lost > 0 {
            hurt += 1;
        }
        if target.hp == 0 && was > 0 {
            killed += 1;
        }
    }
    println!(
        "a2g-run: cluster splash radius {radius:.0} reached {reached} hurt {hurt} killed {killed}"
    );
    let pose = crate::snapshot::EffectPose {
        kind: burst.kind,
        position: burst.position,
        ticks: burst.ticks,
        blast: burst.blast,
        scale: tore_sim::combat::blast_scale::FULL,
    };
    let flashes = cluster_flash::derive(&pose);
    let again = cluster_flash::derive(&pose);
    println!(
        "a2g-run: cluster flashes {} deterministic {} first-at-tick {} last-at-tick {} farthest {:.0} spread {:.0} splash {:.0}",
        flashes.len(),
        if flashes == again { "yes" } else { "no" },
        flashes.iter().map(|f| f.start).min().unwrap_or(0),
        flashes.iter().map(|f| f.start).max().unwrap_or(0),
        flashes
            .iter()
            .map(|f| horizontal(f.position, burst.position))
            .fold(0., f64::max),
        cluster_flash::SPREAD_FT,
        radius,
    );
    for (n, f) in flashes.iter().enumerate() {
        println!(
            "a2g-run: cluster flash n={n} at {:.1} {:.1} {:.1} start {} width {:.0}",
            f.position[0], f.position[1], f.position[2], f.start, f.width
        );
    }
    Ok(())
}

fn surface_targets(scene: &Scene) -> impl Iterator<Item = &tore_sim::combat::live::Target> {
    scene
        .world
        .combat
        .state
        .targets
        .iter()
        .filter(|t| t.role == TargetRole::Surface)
}

fn horizontal(a: Vector, b: Vector) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}
