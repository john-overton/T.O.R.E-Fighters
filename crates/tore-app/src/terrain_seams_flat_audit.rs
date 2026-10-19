//! CPU material checks and fixed GPU contrast probes for flat-tile enrichment.
use super::*;
use crate::terrain_seams::flat::measure_rgb;

pub fn run(out: &Path, code: &str, world: &Terrain, scene: &Scenery) -> AppResult<()> {
    let atlas = scene.vegetation.as_ref().ok_or("no atlas")?;
    let map = &atlas.seams;
    let flat = &map.flat;
    let packed = map.pack();
    let palette = scene.palette.map(|p| p.map(crate::terrain_seams::linear));
    let mut pages = std::fs::File::create(out.join(format!("{code}-flat-pages.tsv")))?;
    writeln!(
        pages,
        "page\tflat\treference\tgreen_fraction\tsd256\tsd512\tsd1024\tsd2048\tnoise_scale\tsmall_share"
    )?;
    for (p, s) in flat.pages.iter().enumerate() {
        let target = flat.targets[p];
        let model = if target == 0 {
            *s
        } else {
            flat.pages[target as usize - 1]
        };
        writeln!(
            pages,
            "{p}\t{}\t{}\t{:.4}\t{:.8}\t{:.8}\t{:.8}\t{:.8}\t{}\t{}",
            target != 0,
            target as i32 - 1,
            s[10],
            s[3],
            s[4],
            s[5],
            s[6],
            model[11],
            model[12]
        )?;
    }
    let count = flat.cells.iter().filter(|&&v| v != 0).count();
    if count == 0 {
        return Err(format!("{code}: no flat placements found").into());
    }
    // First representative of each flat source page, plus the reviewed UKR23
    // placement. Selection never uses the resulting contrast measurement.
    let mut representatives = BTreeMap::new();
    for (tile, &slot) in flat.cells.iter().enumerate() {
        if slot != 0 {
            representatives
                .entry(map.material(tile)[0] - 1)
                .or_insert(tile);
        }
    }
    if code == "UKR" && flat.cells[map.cols] != 0 {
        representatives.insert(map.material(map.cols)[0] - 1, map.cols);
    }
    let mut report = std::fs::File::create(out.join(format!("{code}-flat-contrast.csv")))?;
    writeln!(
        report,
        "tile,page,reference,modifiable_fraction,before_sd256,after_sd256,target_sd256,before_sd512,after_sd512,target_sd512,before_sd1024,after_sd1024,target_sd1024,before_sd2048,after_sd2048,target_sd2048,blocks256,blocks512,blocks1024,blocks2048"
    )?;
    let mut changed = 0;
    let mut protected = 0;
    let mut pattern_hashes = std::collections::BTreeSet::new();
    for (&page, &tile) in &representatives {
        let features = crate::terrain_seams::flat::feature_mask(
            &scene.texture_indices[page as usize * 65536..(page as usize + 1) * 65536],
            atlas,
        );
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut valid = Vec::new();
        let mut hash = 14695981039346656037_u64;
        let origin = [
            (tile % map.cols) as f32 * TILE,
            (tile / map.cols) as f32 * TILE,
        ];
        for y in 0..256 {
            for x in 0..256 {
                let p = [
                    origin[0] + x as f32 * 128. + 64.,
                    origin[1] + y as f32 * 128. + 64.,
                ];
                let source = packed
                    .sample(map, p, &scene.texture_indices, &palette)
                    .unwrap_or([0.; 3]);
                let response = map.baked.flat_response(map, p);
                let result = map.baked.flat_surface(map, p, source);
                let (u, v) = crate::vegetation::rotate(x, y, map.material(tile)[1] as u8);
                let allowed = !features[v * 256 + u];
                if !allowed {
                    protected += 1;
                    if result != source {
                        return Err(format!("{code}: painted feature changed at {p:?}").into());
                    }
                }
                let active = response[..3].iter().any(|v| v.abs() > 1e-7);
                changed += usize::from(active);
                before.push(source);
                after.push(result);
                valid.push(active && x > 2 && x < 253 && y > 2 && y < 253);
                for value in response {
                    hash ^= u64::from(value.to_bits());
                    hash = hash.wrapping_mul(1099511628211);
                }
            }
        }
        pattern_hashes.insert(hash);
        let a = measure_rgb(&before, &valid);
        let b = measure_rgb(&after, &valid);
        let target = flat.targets[page as usize] - 1;
        let t = flat.pages[target as usize];
        write!(report, "{tile},{page},{target},{:.5}", b[10])?;
        for k in 3..7 {
            write!(report, ",{:.8},{:.8},{:.8}", a[k], b[k], t[k])?;
        }
        let blocks: Vec<_> = crate::terrain_seams::flat::SCALES
            .into_iter()
            .map(|step| {
                (0..256)
                    .step_by(step)
                    .map(|y| {
                        (0..256)
                            .step_by(step)
                            .filter(|&x| {
                                (y..y + step).all(|v| (x..x + step).all(|u| valid[v * 256 + u]))
                            })
                            .count()
                    })
                    .sum::<usize>()
            })
            .collect();
        for count in &blocks {
            write!(report, ",{count}")?;
        }
        writeln!(report)?;
        if blocks[1] >= 64 && !(0.75..=1.25).contains(&(b[4] / t[4].max(1e-6))) {
            return Err(format!("{code}: page {page} misses the 25% contrast target").into());
        }
        if b[10] > 0.5 && b[4] <= a[4] * 1.25 {
            return Err(
                format!("{code}: flat page {page} did not gain low-frequency contrast").into(),
            );
        }
        let target_rgb: Vec<_> = scene.texture_indices
            [target as usize * 65536..(target as usize + 1) * 65536]
            .iter()
            .map(|&i| palette[usize::from(i)])
            .collect();
        let mut image = vec![255; 768 * 256 * 4];
        for y in 0..256 {
            for x in 0..256 {
                for (side, pixels) in [&before, &after, &target_rgb].into_iter().enumerate() {
                    let rgb =
                        super::srgb(pixels[y * 256 + x]).map(|v| v.round().clamp(0., 255.) as u8);
                    let at = (y * 768 + x + side * 256) * 4;
                    image[at..at + 3].copy_from_slice(&rgb);
                }
            }
        }
        std::fs::write(
            out.join(format!("{code}-flat-page-{page}.png")),
            crate::replay::png::encode_rgba(768, 256, &image)?,
        )?;
        let mask: Vec<_> = valid
            .iter()
            .flat_map(|&v| [if v { 255 } else { 0 }; 3].into_iter().chain([255]))
            .collect();
        std::fs::write(
            out.join(format!("{code}-flat-page-{page}-mask.png")),
            crate::replay::png::encode_rgba(256, 256, &mask)?,
        )?;
    }
    // Check separate placements of the same source page, away from edge fades.
    let first = flat.cells.iter().position(|&v| v != 0).unwrap();
    let page = map.material(first)[0];
    if let Some(second) =
        (first + 1..flat.cells.len()).find(|&i| flat.cells[i] != 0 && map.material(i)[0] == page)
    {
        let samples = |tile: usize| {
            (0..64)
                .map(|i| {
                    map.baked.flat_response(
                        map,
                        [
                            (tile % map.cols) as f32 * TILE + 4096. + (i % 8) as f32 * 3072.,
                            (tile / map.cols) as f32 * TILE + 4096. + (i / 8) as f32 * 3072.,
                        ],
                    )
                })
                .collect::<Vec<_>>()
        };
        if samples(first) == samples(second)
            && samples(first)
                .iter()
                .any(|r| r[..3].iter().any(|v| v.abs() > 1e-7))
        {
            return Err("flat pattern repeats at tile spacing".into());
        }
    }
    let text = format!(
        "{code}: {count} enriched placements, {} flat source pages checked, {changed} changed natural samples, {protected} protected samples unchanged, {} distinct representative patterns. PASS\n",
        representatives.len(),
        pattern_hashes.len()
    );
    std::fs::write(out.join(format!("{code}-flat-check.txt")), &text)?;
    print!("{text}");

    let selected = if code == "UKR" && flat.cells[map.cols] != 0 {
        map.cols
    } else {
        first
    };
    let source_page = map.material(selected)[0] as usize - 1;
    let target = flat.targets[source_page];
    let reference = (0..flat.cells.len())
        .filter(|&t| map.material(t)[0] == target && flat.cells[t] == 0)
        .min_by_key(|&t| {
            (t % map.cols).abs_diff(selected % map.cols)
                + (t / map.cols).abs_diff(selected / map.cols)
        })
        .ok_or("no mottled reference placement")?;
    let mut cameras = std::fs::File::create(out.join(format!("{code}-flat-cameras.txt")))?;
    let mut checks = std::fs::File::create(out.join(format!("{code}-flat-camera-check.tsv")))?;
    for (label, tile) in [("flat", selected), ("mottled", reference)] {
        let p = [
            (tile % map.cols) as f64 * f64::from(TILE) + 16384.,
            (tile / map.cols) as f64 * f64::from(TILE) + 16384.,
        ];
        let target = [
            p[0],
            f64::from(world.height(p[0] as f32, p[1] as f32)),
            p[1],
        ];
        let mut camera = crate::camera::Camera::new();
        camera.position = [p[0], target[1] + 30000., p[1]];
        camera.yaw = 0.;
        camera.pitch = -std::f32::consts::FRAC_PI_2;
        super::write_camera(
            &mut cameras,
            &mut checks,
            &format!("{code}-{label}-material"),
            code,
            &camera,
            target,
            world,
        )?;
    }
    Ok(())
}
