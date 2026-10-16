//! One actually fired 105 mm shell beside imported truck, APC and tank shapes.
use super::*;
use tore_formats::{shape::Shape, static_object::Definition, surface_unit::SurfaceUnit};
use tore_sim::airport::OrientedBox;
use tore_world::{grounding, resources::ResourceSource};

pub(super) fn run(config: &Configuration, resources: &dyn ResourceSource) -> AppResult<()> {
    let mut report = Report::default();
    let mut fired = state(config)?;
    fired.targets.clear();
    let station = group(&fired).stations[2].ok_or("missing 105 mm")?;
    group_mut(&mut fired).included = [false, false, true];
    fired.own_mut().selected = station;
    let l = launcher([0., 3000., 0.], 0., 0., 300.);
    for _ in 0..180 {
        tick(&mut fired, l, false);
    }
    for _ in 0..120 {
        tick(&mut fired, l, true);
        if !fired.projectiles.is_empty() {
            break;
        }
    }
    report.check(
        !fired.projectiles.is_empty(),
        "105 mm left the AC-130 barrel",
    );
    let mut p = fired.projectiles.first().ok_or("no 105 shell")?.clone();
    p.weapon = Some(fired.weapon(&p).clone());
    report.check(
        p.weapon.as_ref().unwrap().source == "C_105.JT",
        "only the 105 mm is selected",
    );
    // Isolate a single physical shell, avoiding another round's splash in
    // the measured health change. The launch itself uses the normal trigger.
    fired.projectiles = vec![p];
    fired.own_mut().armed = false;
    let mut calibration = fired.clone();
    let mut impacts = Impacts::default();
    for _ in 0..12 * 120 {
        tick(&mut calibration, l, false);
        impacts.collect(&calibration);
    }
    let impact = *impacts
        .points
        .first()
        .ok_or("shell did not reach terrain")?;
    let w = fired.projectiles[0].weapon.as_ref().unwrap();
    report.check(
        tore_sim::combat::blast_scale::of(w) == tore_sim::combat::blast_scale::HOWITZER,
        "105 mm retains HOWITZER blast scale",
    );
    println!(
        "splash-105: effects {:?}, crater {:?}",
        w.effects,
        tore_sim::combat::blast_scale::crater_of(w)
    );
    println!(
        "splash-105: ground explosion {:?}",
        tore_sim::combat::blast::explosion(w.effects.land_explosion)
    );
    let mut ids = Vec::new();
    for (i, (name, direction, distance)) in [
        ("TRUCK.NT", [0., 0., 1.], 25.),
        ("M113.NT", [0., 0., -1.], 25.),
        ("M1.NT", [-1., 0., 0.], 25.),
        ("TRUCK.NT", [0., 0., 1.], 110.),
    ]
    .into_iter()
    .enumerate()
    {
        let bytes = resources.get(name).ok_or("missing mixed-group type")?;
        let unit = SurfaceUnit::parse(bytes)?;
        let definition = Definition::parse(bytes)?;
        let shape_bytes = resources
            .get(unit.shape.as_deref().ok_or("missing shape name")?)
            .ok_or("missing mixed-group shape")?;
        let shape = Shape::scenery(shape_bytes)?;
        let scale = tore_world::terrain::placed_shape_scale(&definition, shape_bytes)?;
        let footing = grounding::UnitFooting::of(&shape, scale);
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for point in footing.points {
            for axis in 0..3 {
                low[axis] = low[axis].min(point[axis]);
                high[axis] = high[axis].max(point[axis]);
            }
        }
        let half = std::array::from_fn(|axis| (high[axis] - low[axis]) / 2.);
        let mut center = [impact[0], (high[1] + low[1]) / 2., impact[2]];
        for axis in [0, 2] {
            center[axis] += direction[axis] * (distance + half[axis]);
        }
        let id = 0x5000_0100 + i as u32;
        let bounds = OrientedBox {
            center,
            half,
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        };
        fired.add_ground_target(id, bounds, unit.hit_points, unit.class, live::Side(2))?;
        println!(
            "splash-105: {name} id={id} category={:#06x} class={} hp={} near_ft={distance}",
            unit.class,
            live::damage_class(unit.class),
            unit.hit_points
        );
        ids.push((id, name, unit.hit_points));
    }
    let mut before = fired.clone();
    before.projectiles[0]
        .weapon
        .as_mut()
        .unwrap()
        .damage
        .collateral_radius = 0;
    let mut after = fired;
    for _ in 0..12 * 120 {
        tick(&mut before, l, false);
        tick(&mut after, l, false);
    }
    for (i, (id, name, initial)) in ids.into_iter().enumerate() {
        let loss = |s: &live::State| initial - s.targets.iter().find(|t| t.id == id).unwrap().hp;
        let (old, new) = (loss(&before), loss(&after));
        println!("splash-105: {name} id={id} damage before={old} after={new}");
        report.check(
            old == 0 && new == if i == 0 { 39.min(initial) } else { 0 },
            format!("{name} mixed-group splash before={old} after={new}"),
        );
    }
    report.done("splash-105")
}
