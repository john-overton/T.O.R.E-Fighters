//! A paired retail field becomes one runway, one route and one parking queue.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    resources: &dyn crate::resources::ResourceSource,
    sources: &mut Placements,
    base: &StripPlacement,
    tile: &StripPlacement,
    zones: &[buildings::Strip],
    parked: &mut [crate::surface::ParkedAircraft],
    taken: &mut std::collections::BTreeSet<usize>,
    added_count: &mut u32,
    height: &impl Fn(f64, f64) -> f64,
    terrain: Option<&super::super::Terrain>,
) -> WorldResult<Built> {
    let defaults = plan::default_materials(DEFAULTS)?;
    let plan = Plan::parse(include_str!("../../../airports/merged.toml"), &defaults)?;
    let heading = base.stance.heading;
    let mut frame = base.frame;
    // Retail pairs use cardinal headings. Exact axes keep site rounding
    // independent of a platform's tiny sin(pi) residual.
    for axis in frame.right.iter_mut().chain(frame.forward.iter_mut()) {
        if (*axis - axis.round()).abs() < 1e-12 {
            *axis = axis.round();
        }
    }
    // Centre the compact replacement between the two source placement origins.
    for axis in [0, 2] {
        frame.origin[axis] = (base.stance.support_origin[axis] + tile.stance.support_origin[axis])
            * 0.5
            - frame.forward[axis] * 6000.;
    }
    let parts = [(&plan, [0., 0.])];
    let (materials, patches) = layout::compose(&parts, None, f64::from(base.angle));
    let (min, max) =
        geometry::bounds(patches.iter().map(|p| &p.poly)).ok_or("merged field has no pavement")?;
    let wet_min = min.map(|v| v - plan.grass_margin_ft);
    let wet_max = max.map(|v| v + plan.grass_margin_ft);
    if let Some(terrain) = terrain {
        let mut types = super::super::type_infos(resources, sources);
        let ground = super::super::surface_ground(&terrain.theater, sources, &mut types);
        let own = [base.index, tile.index];
        let obstacles: Vec<_> = ground
            .objects
            .iter()
            .filter(|o| {
                let placement = sources.layout.placements.iter().find(|p| {
                    crate::surface::UnitId::layout(p.key.ordinal).is_some_and(|id| id.0 == o.id)
                });
                let movable = placement.is_some_and(|p| {
                    let upper = p.object_type.to_ascii_uppercase();
                    upper.ends_with(".OT")
                        && !["CITY", "CTYBK", "TWNBK"]
                            .iter()
                            .any(|prefix| upper.starts_with(prefix))
                        && sources.definitions.get(&p.object_type).is_some_and(|d| {
                            super::super::PlacedSize::of(d) != super::super::PlacedSize::MapTied
                        })
                        && sources.shapes.contains_key(&p.object_type)
                });
                !movable || !buildings::belongs(o.at[0] as f64, o.at[1] as f64, &own, zones)
            })
            .collect();
        let base_code = tore_formats::theater::base_theater(&terrain.layout).unwrap_or("");
        let texel = crate::city::mask::ground_texel(resources, &terrain.environment, base_code);
        let mut candidates = Vec::new();
        for x in -16i64..=16 {
            for z in -16i64..=16 {
                candidates.push((x * x + z * z, x * 2048, z * 2048));
            }
        }
        candidates.sort_unstable();
        let original = frame.origin;
        let mut found = false;
        for (_, dx, dz) in candidates {
            frame.origin[0] = (original[0] + dx as f64).round();
            frame.origin[2] = (original[2] + dz as f64).round();
            let mut elevation = None;
            let clear = (-4..=4).all(|x| {
                (0..=26).all(|z| {
                    let p = frame.world([f64::from(x) * 500., f64::from(z) * 500. - 500.]);
                    let at = [p[0].round() as i64, p[2].round() as i64];
                    let h = height(p[0], p[2]);
                    let same = elevation.get_or_insert(h);
                    ground.inside(at, 1)
                        && ground.level(at, 0)
                        && !ground.water(at)
                        && texel(at[0], at[1]) != Some(255)
                        && (h - *same).abs() < 0.01
                        && !obstacles.iter().any(|o| o.within(at, 350))
                        && !ground
                            .runways
                            .iter()
                            .filter(|r| r.id != base.id && r.id != tile.id)
                            .any(|r| r.within(at, 350))
                })
            });
            if clear
                && dry_site(&frame, wet_min, wet_max, &|x, z| {
                    let at = [x as i64, z as i64];
                    ground.water(at) || texel(at[0], at[1]) == Some(255)
                })
            {
                found = true;
                break;
            }
        }
        if !found {
            return Err(format!(
                "no clear flat merged site for {} in {}",
                base.placement.name.as_deref().unwrap_or("airport"),
                terrain.layout
            )
            .into());
        }
    }
    // Use the shared pavement footing at the replacement site. Buildings
    // subsequently use stance_on_terrain at their own relocated positions.
    let mut placement = base.placement.clone();
    placement.position[0] = frame.origin[0].round() as i32;
    placement.position[2] = frame.origin[2].round() as i32;
    frame.origin[1] = sources
        .stance_on_terrain(&placement, height)
        .ok_or("merged airfield has no pavement footing")?
        .support_origin[1];
    let anchors = plan_anchors(&plan, [0., 0.], &frame, heading);

    let center = frame.world([(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5]);
    let surface = OrientedBox {
        center,
        half: [
            (max[0] - min[0]) * 0.5 + plan.grass_margin_ft,
            1.,
            (max[1] - min[1]) * 0.5 + plan.grass_margin_ft,
        ],
        heading,
        pitch: 0.,
        bank: 0.,
    };
    let own = [base.index, tile.index];
    let layout_count = sources.layout.placements.len();
    sources
        .layout
        .placements
        .extend(sources.surface.iter().map(|(_, p)| p.clone()));
    let mut local_taken = taken.clone();
    let mut relaid = buildings::relay(
        resources,
        sources,
        &frame,
        &own,
        zones,
        &plan.lines,
        &plan.extras,
        anchors.map(|a| (a.parking, a.parking_heading)),
        &base.placement,
        &mut local_taken,
        added_count,
    );
    sources.layout.placements.truncate(layout_count);
    taken.extend(local_taken.into_iter().filter(|i| *i < layout_count));
    let moved_surface = relaid
        .moved
        .iter()
        .filter(|(i, _)| *i >= layout_count)
        .map(|(i, p)| (sources.surface[*i - layout_count].0, p.clone()))
        .collect();
    relaid.moved.retain(|(i, _)| *i < layout_count);
    let aprons: Vec<_> = plan.aprons.iter().map(|a| (a.min, a.max)).collect();
    let slots: Vec<_> = anchors
        .iter()
        .flat_map(|a| a.parking.map(|p| frame.local(p)))
        .collect();
    let spots = buildings::apron_spots(&aprons, &slots);
    let snapped = buildings::snap_parked(parked, &frame, &own, zones, &spots, &|p| {
        patches
            .iter()
            .any(|patch| geometry::contains(&patch.poly, p))
    });
    let lights = lights::lights(&parts, None, &patches, &materials, &frame, height);
    Ok(Built {
        plan: format!("MERGED {}+{}", base.object_type, tile.object_type),
        strip_id: base.id,
        merged_from: Some(tile.id),
        replaced_centres: [base, tile]
            .into_iter()
            .flat_map(|s| {
                [-1., 0., 1.].into_iter().flat_map(move |x| {
                    [-1., 0., 1.].into_iter().map(move |z| {
                        std::array::from_fn(|a| {
                            s.stance.center[a]
                                + s.stance.basis.right[a] * s.stance.half[0] * x
                                + s.stance.basis.forward[a] * s.stance.half[2] * z
                        })
                    })
                })
            })
            .collect(),
        frame,
        materials,
        patches,
        surface,
        anchors,
        lights,
        runways: plan
            .runways
            .iter()
            .map(|r| DrawnRunway {
                name: r.name.clone(),
                from: r.from,
                heading: r.heading,
                length: r.length,
                width: r.width,
                pad: r.pad,
            })
            .collect(),
        moved: relaid.moved,
        moved_surface,
        added: relaid.added,
        snapped,
        runway_line: RunwayLine::main(&plan, &frame, heading, [0., 0.]),
        continues: None,
    })
}

/// Check the entire landable field, including grass between paved pieces.
/// The coarse site grid can miss a single 128 ft source-water texel.
fn dry_site(frame: &Frame, min: Point, max: Point, wet: &impl Fn(f64, f64) -> bool) -> bool {
    let step = audit::WATER_STEP_FT;
    let nx = ((max[0] - min[0]) / step).ceil() as usize;
    let nz = ((max[1] - min[1]) / step).ceil() as usize;
    (0..=nx).all(|x| {
        (0..=nz).all(|z| {
            let p = frame.world([
                (min[0] + x as f64 * step).min(max[0]),
                (min[1] + z as f64 * step).min(max[1]),
            ]);
            !wet(p[0], p[2])
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_narrow_water_strip_between_coarse_samples_rejects_the_site() {
        let frame = Frame {
            origin: [0.; 3],
            right: [1., 0., 0.],
            forward: [0., 0., 1.],
        };
        let water = |_x: f64, z: f64| (100. ..=228.).contains(&z);
        assert!((0..=24).all(|z| !water(0., f64::from(z) * 500.)));
        assert!(!dry_site(&frame, [-100., 0.], [100., 12000.], &water));
        assert!(dry_site(&frame, [-100., 0.], [100., 12000.], &|_, _| false));
    }
}
