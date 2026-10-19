//! CPU evidence from imported bytes and the renderer's actual indexed pages.
//! Output belongs in local scratch: images are derivatives of user-owned art.
use crate::{
    AppResult,
    scenery::Scenery,
    terrain::{Overrides, Terrain},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    path::Path,
};
use tore_formats::{
    Archive, Pic,
    theater::{Environment, base_theater},
};

pub fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let [out, source] = args.as_slice() else {
        return Err("--terrain-audit OUT RETAIL_INSTALL_DIR".into());
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let resources = crate::reel::load_assets()?.theater_resources;
    let archives = [
        Archive::open(Path::new(source).join("FA_1.LIB"))?,
        Archive::open(Path::new(source).join("FA_2.LIB"))?,
    ];
    let mut identity = String::from("resource\tarchive\tsha256\timport_equal\n");
    let mut checked = BTreeSet::new();
    let mut summary = String::from(
        "layout\tmap\tlayer\ttmap_lines\tunique\tduplicates\trotations\ttdic\tshade_sets\n",
    );
    for (name, bytes) in &resources {
        if !name.ends_with(".MM") || !matches!(base_theater(name), Some("UKR" | "FRA" | "BAL")) {
            continue;
        }
        let base = base_theater(name).unwrap();
        let env = Environment::parse(bytes)?;
        let text = tore_formats::text::decode_cp437(bytes);
        let mut names = BTreeSet::from([
            name.clone(),
            format!("{base}.T2"),
            env.layer.clone(),
            "LAND.PIC".into(),
        ]);
        names.extend(env.textures.values().map(|p| p.resource_name(base)));
        names.extend(
            resources
                .keys()
                .filter(|n| {
                    n.strip_prefix(base)
                        .and_then(|s| s.strip_suffix(".PIC"))
                        .is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
                })
                .cloned(),
        );
        let mut keywords = BTreeMap::<&str, usize>::new();
        for line in text.lines().filter(|l| !l.starts_with(char::is_whitespace)) {
            if let Some(key) = line.split_whitespace().next() {
                *keywords.entry(key).or_default() += 1;
            }
        }
        std::fs::write(
            out.join(format!("{name}-keywords.txt")),
            format!("{keywords:#?}"),
        )?;

        // Weather conditions and original auxiliary remaps are imported whole.
        names.extend(
            resources
                .keys()
                .filter(|n| n.ends_with(".LAY") || n.ends_with(".PAL"))
                .cloned(),
        );
        if resources.contains_key(&env.map) {
            names.insert(env.map.clone());
        }
        for resource in names {
            if !checked.insert(resource.clone()) {
                continue;
            }
            let (a, archive) = archives
                .iter()
                .enumerate()
                .find(|(_, a)| a.entries.contains_key(&resource))
                .ok_or("resource missing in retail archives")?;
            let retail = archive.read(&resource)?;
            let equal = resources.get(&resource) == Some(&retail);
            writeln!(
                identity,
                "{resource}\tFA_{}.LIB\t{}\t{equal}",
                a + 1,
                tore_formats::executable::sha256(&retail)
            )?;
            if !equal {
                return Err(format!("import differs from retail: {resource}").into());
            }
        }
        let world = Terrain::for_mission(
            &resources,
            name.trim_end_matches(".MM"),
            None,
            &Overrides {
                city_fill: false,
                ..Overrides::default()
            },
        )?;
        let scene = Scenery::build_for(&resources, &world, false)?;
        let grid_name = if resources.contains_key(&env.map) {
            env.map.clone()
        } else {
            format!("{base}.T2")
        };
        let source_grid = tore_formats::theater::Theater::parse(&resources[&grid_name])?;
        if source_grid.cells != world.theater.cells {
            return Err(format!("{name}: runtime T2 differs from source").into());
        }
        check_mesh(&world, &scene)?;
        let lines = text.lines().filter(|l| l.starts_with("tmap ")).count();
        let rotations: BTreeSet<_> = env.textures.values().map(|p| p.rotation).collect();
        let dictionaries = check_dictionaries(&text, &resources, base)?;
        writeln!(
            summary,
            "{name}\t{}\t{}\t{lines}\t{}\t{}\t{rotations:?}\t{dictionaries}\t{}",
            env.map,
            env.layer,
            env.textures.len(),
            lines - env.textures.len(),
            world.weather.configuration().shades().len()
        )?;
        dump(out, name, &world, &scene, &resources)?;
        println!(
            "terrain audit: PASS {name} source bytes, {dictionaries} coverage dictionaries, {} placements",
            env.textures.len()
        );
    }
    std::fs::write(out.join("resources.tsv"), identity)?;
    std::fs::write(out.join("layouts.tsv"), summary)?;
    let mut builds = String::new();
    for name in ["FA.EXE", "FA_1.LIB", "FA_2.LIB"] {
        writeln!(
            builds,
            "{}  {name}",
            tore_formats::executable::sha256(&std::fs::read(Path::new(source).join(name))?)
        )?;
    }
    std::fs::write(out.join("retail.sha256"), builds)?;
    Ok(())
}

/// Check the actual upload mesh, including triangles clipped round airports.
/// A centroid is unambiguous even when vertices sit on shared tile boundaries.
fn check_mesh(world: &Terrain, scene: &Scenery) -> AppResult<()> {
    for tri in scene.vertices.chunks_exact(30) {
        let x = (tri[0] + tri[10] + tri[20]) / 3.;
        let z = (tri[2] + tri[12] + tri[22]) / 3.;
        let key = (
            ((x / 32768.).floor() as i32) * 4,
            ((z / 32768.).floor() as i32) * 4,
        );
        let layer = scene
            .ground_layers
            .get(&key)
            .copied()
            .or(scene.land_texture);
        if tri[5] != layer.map_or(-1., |l| l as f32) {
            return Err("terrain mesh texture page differs from placement".into());
        }
        let (u, v) = ((x / 32768.).fract(), (z / 32768.).fract());
        let expected = match world
            .environment
            .textures
            .get(&key)
            .map_or(0, |p| p.rotation)
        {
            1 => [1. - v, 1. - u],
            2 => [1. - u, v],
            3 => [v, u],
            _ => [u, 1. - v],
        };
        for k in 0..2 {
            let actual = (tri[3 + k] + tri[13 + k] + tri[23 + k]) / 3.;
            if (actual - expected[k]).abs() > 0.0001 {
                return Err("terrain mesh rotation/flip differs from placement".into());
            }
        }
    }
    Ok(())
}

fn check_dictionaries(
    text: &str,
    resources: &BTreeMap<String, Vec<u8>>,
    base: &str,
) -> AppResult<usize> {
    let words: Vec<_> = text.split_whitespace().collect();
    let mut number = 0;
    for (at, word) in words.iter().enumerate() {
        if *word != "tdic" {
            continue;
        }
        let pic = Pic::parse(
            resources
                .get(&format!("{base}{number}.PIC"))
                .ok_or("missing dictionary picture")?,
        )?;
        let expected: Vec<usize> = words
            .get(at + 1..at + 34)
            .ok_or("short tdic")?
            .iter()
            .map(|v| v.parse())
            .collect::<Result<_, _>>()?;
        if expected[0] != pic.width {
            return Err("dictionary dimensions differ".into());
        }
        for z in 0..4 {
            for x in 0..4 {
                let mut land = true;
                let mut water = true;
                for v in z * pic.height / 4..(z + 1) * pic.height / 4 {
                    for u in x * pic.width / 4..(x + 1) * pic.width / 4 {
                        let p = (pic.height - 1 - v) * pic.width + u;
                        let wet = pic.pixels[p] == 255 || !pic.mask[p];
                        land &= !wet;
                        water &= wet;
                    }
                }
                if expected[1 + z * 4 + x] != usize::from(land)
                    || expected[17 + z * 4 + x] != usize::from(water)
                {
                    return Err(format!("{base}{number} tdic differs at {x},{z}").into());
                }
            }
        }
        number += 1;
    }
    Ok(number)
}

fn dump(
    out: &Path,
    name: &str,
    world: &Terrain,
    scene: &Scenery,
    resources: &BTreeMap<String, Vec<u8>>,
) -> AppResult<()> {
    let folder = out.join(name.trim_end_matches(".MM"));
    std::fs::create_dir_all(&folder)?;
    let base = base_theater(name).unwrap();
    let mut names = BTreeMap::new();
    for (key, &layer) in &scene.ground_layers {
        let p = &world.environment.textures[key];
        names.insert(
            layer,
            format!(
                "{}{}",
                if world.neutral_airfield_ground.contains(key) {
                    "neutral:"
                } else {
                    ""
                },
                p.resource_name(base)
            ),
        );
    }
    if let Some(layer) = scene.land_texture {
        names.insert(layer, "LAND.PIC".into());
    }
    let mut source_stats = String::from(
        "texture\twidth\theight\tembedded_palette_entries\tmean_linear_rgb\tmean_palette_pair\tlinear_fit_error\n",
    );
    for (texture, bytes) in resources.iter().filter(|(n, _)| {
        n.strip_prefix(base)
            .and_then(|s| s.strip_suffix(".PIC"))
            .is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            || *n == "LAND.PIC"
    }) {
        let pic = Pic::parse(bytes)?;
        let (mean, pair, error) = crate::terrain_seams::mean(&pic.pixels, &scene.palette);
        writeln!(
            source_stats,
            "{texture}\t{}\t{}\t{}\t{mean:?}\t{pair:?}\t{error:.7}",
            pic.width,
            pic.height,
            pic.palette.len()
        )?;
    }
    std::fs::write(folder.join("source-textures.tsv"), source_stats)?;
    let mut stats = String::from(
        "page\ttexture\twidth\theight\tpalette\twater_texels\tmean_r\tmean_g\tmean_b\n",
    );
    let mut means = BTreeMap::new();
    for (layer, texture) in &names {
        let pic = Pic::parse(&resources[texture.trim_start_matches("neutral:")])?;
        let pixels = &scene.texture_indices[layer * 65536..(layer + 1) * 65536];
        let mut mean = [0.; 3];
        let mut count = 0;
        for &index in pixels.iter().filter(|i| **i != 255) {
            for (v, c) in mean.iter_mut().zip(scene.palette[usize::from(index)]) {
                *v += f64::from(c);
            }
            count += 1;
        }
        mean = mean.map(|v| v / f64::from(count.max(1)));
        writeln!(
            stats,
            "{layer}\t{texture}\t{}\t{}\t{} (embedded={})\t{}\t{:.4}\t{:.4}\t{:.4}",
            pic.width,
            pic.height,
            world.environment.layer,
            pic.palette.len(),
            65536 - count,
            mean[0],
            mean[1],
            mean[2]
        )?;
        means.insert(*layer, mean);
    }
    let t = &world.theater;
    let mut cells = String::from(
        "col\trow\ttile_col\ttile_row\tindex\ttexture\tpage\trotation\tpalette\tT2_color\tT2_class\televation_ft\n",
    );
    let width = (t.cols - 1) * 8;
    let height = (t.rows - 1) * 8;
    let mut image = vec![0; width * height * 4];
    let mut mean_image = image.clone();
    for z in 0..t.rows - 1 {
        for x in 0..t.cols - 1 {
            let key = ((x & !3) as i32, (z & !3) as i32);
            let placement = world.environment.textures.get(&key);
            let cell = t.cell(x, z);
            let layer = scene
                .ground_layers
                .get(&key)
                .copied()
                .or(scene.land_texture);
            let rotation = placement.map_or(0, |p| p.rotation);
            writeln!(
                cells,
                "{x}\t{z}\t{}\t{}\t{}\t{}\t{}\t{rotation}\t{}\t{}\t{}\t{}",
                key.0,
                key.1,
                placement.map_or_else(|| "fallback".into(), |p| p.texture.to_string()),
                layer.map_or("none", |l| names[&l].as_str()),
                layer.map_or(-1, |v| v as i32),
                world.environment.layer,
                cell.color,
                cell.class,
                u32::from(cell.elevation) * 256
            )?;
            for dz in 0..8 {
                for dx in 0..8 {
                    let u = (x % 4) * 64 + dx * 8 + 4;
                    let v = (z % 4) * 64 + dz * 8 + 4;
                    let (u, v) = match rotation {
                        1 => (255 - v, 255 - u),
                        2 => (255 - u, v),
                        3 => (v, u),
                        _ => (u, 255 - v),
                    };
                    let index = if placement.is_none() && cell.color == 255 {
                        255
                    } else {
                        layer.map_or(cell.color, |l| {
                            scene.texture_indices[l * 65536 + v * 256 + u]
                        })
                    };
                    let rgb = if index == 255 {
                        [40, 60, 80]
                    } else {
                        scene.palette[usize::from(index)]
                    };
                    let at = ((height - 1 - (z * 8 + dz)) * width + x * 8 + dx) * 4;
                    image[at..at + 3].copy_from_slice(&rgb);
                    image[at + 3] = 255;
                    let avg = if index == 255 {
                        rgb
                    } else {
                        layer.map_or(rgb, |l| means[&l].map(|v| v.round() as u8))
                    };
                    mean_image[at..at + 3].copy_from_slice(&avg);
                    mean_image[at + 3] = 255;
                }
            }
        }
    }
    std::fs::write(folder.join("cells.tsv"), cells)?;
    std::fs::write(folder.join("textures.tsv"), stats)?;
    for (file, pixels) in [("ground-unlit.png", image), ("tile-means.png", mean_image)] {
        std::fs::write(
            folder.join(file),
            crate::replay::png::encode_rgba(width as u32, height as u32, &pixels)?,
        )?;
    }
    Ok(())
}
