//! Disposable, bounded local presentation cache. Never imported game state.
use super::canopy::Map;
use crate::{
    terrain::Terrain,
    vegetation_shapes::{Crown, Library, Look, Mesh},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::RwLock,
};
use tore_codec::{Fnv1a64, fnv1a64};

static PROFILE: RwLock<Option<(PathBuf, Vec<u8>)>> = RwLock::new(None);
const MAX_BYTES: usize = 256 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"TOREVEG7";

pub fn configure(directory: &Path) {
    // The successful import loader has already pruned older good generations.
    // Include names and sizes, plus actual decoded resources below, so fallback
    // to a previous valid generation cannot reuse another import's data.
    let mut generations: Vec<_> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("menu-") && name.ends_with(".pack")
        })
        .map(|e| (e.file_name(), e.metadata().map_or(0, |m| m.len())))
        .collect();
    generations.sort();
    *PROFILE.write().unwrap_or_else(|e| e.into_inner()) = Some((
        directory.join("vegetation-cache"),
        format!("{generations:?}").into_bytes(),
    ));
}

const MAX_OVERRIDE: u64 = 16 * 1024 * 1024;

fn override_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_OVERRIDE + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_OVERRIDE {
        return Err(std::io::Error::other("top PNG exceeds 16 MiB"));
    }
    Ok(bytes)
}

fn override_directory(cache_directory: &Path) -> PathBuf {
    cache_directory.with_file_name("vegetation-overrides")
}

fn hash_overrides(hash: &mut Fnv1a64, cache_directory: &Path) {
    let mut paths: Vec<_> = std::fs::read_dir(override_directory(cache_directory))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with("-top.png"))
        })
        .collect();
    paths.sort();
    for path in paths {
        hash.update(path.file_name().unwrap().to_string_lossy().as_bytes());
        match override_bytes(&path) {
            Ok(bytes) => hash.update(&bytes),
            Err(error) => hash.update(error.to_string().as_bytes()),
        }
    }
}

pub(crate) fn top_override(name: &str, palette: &[[u8; 3]; 256]) -> Option<(Vec<u8>, f32)> {
    let directory = PROFILE.read().ok()?.as_ref()?.0.clone();
    let path = override_directory(&directory).join(format!("{name}-top.png"));
    let bytes = match override_bytes(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            eprintln!("vegetation override {}: {e}", path.display());
            return None;
        }
    };
    match crate::vegetation_shapes::override_cell(&bytes, palette) {
        Ok(cell) => Some(cell),
        Err(e) => {
            eprintln!("vegetation override {}: {e}", path.display());
            None
        }
    }
}

pub struct Cache {
    path: PathBuf,
    key: u64,
}
impl Cache {
    pub fn for_scene(
        resources: &BTreeMap<String, Vec<u8>>,
        world: &Terrain,
        palette: &[[u8; 3]; 256],
        first: usize,
        mode: &str,
    ) -> Option<Self> {
        if std::env::var("TORE_VEG_CACHE").as_deref() == Ok("0") {
            return None;
        }
        let profile = PROFILE.read().ok()?.clone()?;
        Some(Self::with_profile(
            profile,
            resources,
            world,
            palette,
            first,
            &format!(
                "{mode}:seams={}",
                crate::terrain_seams::enabled().unwrap_or(false)
            ),
            env!("TORE_VEGETATION_VERSION"),
        ))
    }
    fn with_profile(
        profile: (PathBuf, Vec<u8>),
        resources: &BTreeMap<String, Vec<u8>>,
        world: &Terrain,
        palette: &[[u8; 3]; 256],
        first: usize,
        mode: &str,
        version: &str,
    ) -> Self {
        let mut hash = Fnv1a64::new();
        for bytes in [
            MAGIC.as_slice(),
            version.as_bytes(),
            &profile.1,
            world.layout.as_bytes(),
            mode.as_bytes(),
        ] {
            hash.update(bytes);
        }
        hash_overrides(&mut hash, &profile.0);
        hash.update(&(first as u64).to_le_bytes());
        for rgb in palette {
            hash.update(rgb);
        }
        for source in [resources, &super::art()] {
            for (name, bytes) in source {
                hash.update(name.as_bytes());
                hash.update(&(bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            }
        }
        // Effective terrain and exclusion geometry, including runtime airport
        // choices and city-fill switches, are inputs too.
        hash.update(
            format!("{:?}{:?}", world.theater.cells, world.airport_scene.runways).as_bytes(),
        );
        // Cached ground habitat and seam heights must follow mission-specific
        // foundations, including when only the objective layout changes.
        for (&(x, z), triangles) in &world.foundations.cells {
            hash.update(&(x as u32).to_le_bytes());
            hash.update(&(z as u32).to_le_bytes());
            hash.update(&(triangles.len() as u32).to_le_bytes());
            for point in triangles.iter().flatten() {
                for coordinate in point {
                    hash.update(&coordinate.to_bits().to_le_bytes());
                }
            }
        }
        hash.update(format!("{:?}", world.neutral_airfield_ground).as_bytes());
        if let Some(fill) = &world.city_fill {
            hash.update(format!("{:?}", fill.layout.map).as_bytes());
        }
        for object in world.airport_scene.objects.iter().filter(|o| o.runway) {
            hash.update(format!("{:?}", object.bounds).as_bytes());
        }
        let key = hash.finish();
        Self {
            path: profile.0.join(format!(
                "{}-{key:016x}.bin",
                world.layout.trim_end_matches(".MM")
            )),
            key,
        }
    }
    fn atlas_file(&self) -> Self {
        Self {
            path: self.path.with_extension("atlas"),
            key: self.key,
        }
    }
    pub fn read_atlas(&self) -> Option<super::Atlas> {
        decode_atlas(&self.atlas_file().read_payload()?)
    }
    pub fn write_atlas(&self, atlas: &super::Atlas) {
        self.atlas_file().write_data(encode_atlas(atlas));
    }
    pub fn read_seams(&self) -> Option<crate::terrain_seams::Map> {
        let bytes = Self {
            path: self.path.with_extension("seams"),
            key: self.key,
        }
        .read_payload()?;
        let mut r = Read(&bytes);
        let cols = r.count(1024)?;
        let rows = r.count(1024)?;
        let mean_error = r.floats::<1>()?[0];
        let bytes = r.blob(MAX_BYTES)?.to_vec();
        let materials = r.blob(1024 * 1024 * 8)?.to_vec();
        let page_count = r.count(4096)?;
        let pages = (0..page_count)
            .map(|_| r.floats::<15>())
            .collect::<Option<Vec<_>>>()?;
        let targets = (0..page_count)
            .map(|_| r.count(4096).map(|v| v as u32))
            .collect::<Option<Vec<_>>>()?;
        let cell_count = r.count(1024 * 1024)?;
        let cells = (0..cell_count)
            .map(|_| r.count(1024 * 1024).map(|v| v as u32))
            .collect::<Option<Vec<_>>>()?;
        let boundaries = r.blob(1024 * 1024)?.to_vec();
        let size = [
            r.count(1024)? as u32,
            r.count(1024)? as u32,
            r.count(256)? as u32,
        ];
        let records = r.blob(MAX_BYTES)?.to_vec();
        let baked_pixels = r.blob(MAX_BYTES)?.to_vec();
        if !r.0.is_empty()
            || bytes.len() != cols * rows * crate::terrain_seams::STRIDE
            || materials.len() != cols * rows * 8
            || cells.len() != boundaries.len()
            || (!cells.is_empty() && cells.len() != cols * rows)
            || baked_pixels.len() != size.iter().map(|&v| v as usize).product::<usize>() * 8
        {
            return None;
        }
        Some(crate::terrain_seams::Map {
            cols,
            rows,
            bytes,
            mean_error,
            materials,
            flat: crate::terrain_seams::flat::Flat {
                pages,
                targets,
                cells,
                boundaries,
            },
            baked: crate::terrain_seams::baked::Baked {
                size,
                records,
                pixels: baked_pixels,
            },
        })
    }
    pub fn write_seams(&self, map: &crate::terrain_seams::Map) {
        let mut data = Vec::new();
        put(&mut data, map.cols);
        put(&mut data, map.rows);
        floats(&mut data, &[map.mean_error]);
        blob(&mut data, &map.bytes);
        blob(&mut data, &map.materials);
        put(&mut data, map.flat.pages.len());
        for p in &map.flat.pages {
            floats(&mut data, p);
        }
        for &v in &map.flat.targets {
            put(&mut data, v as usize);
        }
        put(&mut data, map.flat.cells.len());
        for &v in &map.flat.cells {
            put(&mut data, v as usize);
        }
        blob(&mut data, &map.flat.boundaries);
        for v in map.baked.size {
            put(&mut data, v as usize);
        }
        blob(&mut data, &map.baked.records);
        blob(&mut data, &map.baked.pixels);
        Self {
            path: self.path.with_extension("seams"),
            key: self.key,
        }
        .write_data(data);
    }
    pub fn read_grass(&self) -> Option<super::grass::Map> {
        let bytes = Self {
            path: self.path.with_extension("grass"),
            key: self.key,
        }
        .read_payload()?;
        decode_grass(&bytes)
    }
    pub fn write_grass(&self, map: &super::grass::Map) {
        Self {
            path: self.path.with_extension("grass"),
            key: self.key,
        }
        .write_data(encode_grass(map));
    }
    pub fn read(&self) -> Option<(Library, Map, Vec<u8>)> {
        let bytes = self.read_payload()?;
        decode(&bytes)
    }
    fn read_payload(&self) -> Option<Vec<u8>> {
        use std::io::Read;
        let file = std::fs::File::open(&self.path).ok()?;
        if file.metadata().ok()?.len() > MAX_BYTES as u64 {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() < 24 || bytes.len() > MAX_BYTES || &bytes[..8] != MAGIC {
            return None;
        }
        if u64::from_le_bytes(bytes[8..16].try_into().ok()?) != self.key
            || u64::from_le_bytes(bytes[16..24].try_into().ok()?) != fnv1a64(&bytes[24..])
        {
            return None;
        }
        Some(bytes.split_off(24))
    }
    pub fn write(&self, library: &Library, map: Map, pages: &[u8]) {
        self.write_data(encode(library, map, pages));
    }
    fn write_data(&self, payload: Vec<u8>) {
        if payload.len() + 24 > MAX_BYTES {
            return;
        }
        let result = (|| -> std::io::Result<()> {
            use std::io::Write;
            static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            std::fs::create_dir_all(self.path.parent().unwrap())?;
            let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let temp = self
                .path
                .with_extension(format!("{}-{serial}.tmp", std::process::id()));
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            let result = (|| {
                file.write_all(MAGIC)?;
                file.write_all(&self.key.to_le_bytes())?;
                file.write_all(&fnv1a64(&payload).to_le_bytes())?;
                file.write_all(&payload)?;
                drop(file);
                // Rename never exposes a partially written cache to a reader.
                std::fs::rename(&temp, &self.path)
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(&temp);
            }
            result
        })();
        if let Err(error) = result {
            log::debug!("vegetation cache write: {error}");
        }
    }
}

fn put(out: &mut Vec<u8>, n: usize) {
    out.extend((n as u32).to_le_bytes());
}
fn floats(out: &mut Vec<u8>, values: &[f32]) {
    for n in values {
        out.extend(n.to_le_bytes());
    }
}
fn blob(out: &mut Vec<u8>, bytes: &[u8]) {
    put(out, bytes.len());
    out.extend(bytes);
}
fn encode_grass(map: &super::grass::Map) -> Vec<u8> {
    let mut out = Vec::new();
    floats(&mut out, &map.area);
    put(&mut out, map.pages_x as usize);
    blob(&mut out, &map.mask);
    for n in map.terrain_size {
        put(&mut out, n as usize);
    }
    floats(&mut out, &map.terrain);
    out
}
fn decode_grass(bytes: &[u8]) -> Option<super::grass::Map> {
    let mut r = Read(bytes);
    let area = r.floats::<4>()?;
    if area[2..]
        .iter()
        .any(|v| *v < 0. || *v > 4096. || v.fract() != 0.)
    {
        return None;
    }
    let pages_x = r.count(16)? as u32;
    if pages_x != (area[2] as u32).div_ceil(256) {
        return None;
    }
    let mask = r.blob(64 * 1024 * 1024)?.to_vec();
    if mask.len() != pages_x as usize * (area[3] as usize).div_ceil(256) * 256 * 256 * 4 {
        return None;
    }
    let terrain_size = [r.count(4096)? as u32, r.count(4096)? as u32];
    if terrain_size.iter().any(|n| *n < 2) {
        return None;
    }
    let len = terrain_size[0] as usize * terrain_size[1] as usize * 4;
    if r.0.len() != len * 4 {
        return None;
    }
    let mut terrain = Vec::with_capacity(len);
    for _ in 0..len {
        terrain.push(r.floats::<1>()?[0]);
    }
    Some(super::grass::Map {
        area,
        pages_x,
        mask,
        terrain_size,
        terrain,
    })
}
fn encode(lib: &Library, map: Map, pages: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    floats(&mut out, &map.area);
    put(&mut out, map.first);
    put(&mut out, map.pages_x);
    blob(&mut out, pages);
    put(&mut out, lib.vertices.len());
    floats(&mut out, &lib.vertices);
    put(&mut out, lib.meshes.len());
    for m in &lib.meshes {
        put(&mut out, m.range.start as usize);
        put(&mut out, m.range.end as usize);
        floats(&mut out, &[m.radius, m.height_scale]);
        put(&mut out, usize::from(m.look == Look::Solid));
        put(&mut out, usize::from(m.recolour.is_some()));
        floats(&mut out, &m.recolour.unwrap_or_default());
        blob(&mut out, m.source.as_bytes());
        put(&mut out, m.sprite_layer);
        put(&mut out, m.sprite_tile.map_or(4, usize::from));
        floats(&mut out, &m.top);
        floats(&mut out, &m.mean_color);
        put(&mut out, m.crowns.len());
        for c in &m.crowns {
            floats(&mut out, &c.center);
            floats(&mut out, &c.half);
        }
    }
    put(&mut out, lib.variants.len());
    for v in &lib.variants {
        put(&mut out, v.len());
        for &i in v {
            put(&mut out, usize::from(i));
        }
    }
    out
}
struct Read<'a>(&'a [u8]);
impl<'a> Read<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let v = self.0.get(..n)?;
        self.0 = &self.0[n..];
        Some(v)
    }
    fn int(&mut self) -> Option<usize> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?) as usize)
    }
    fn count(&mut self, limit: usize) -> Option<usize> {
        let n = self.int()?;
        (n <= limit).then_some(n)
    }
    fn blob(&mut self, limit: usize) -> Option<&'a [u8]> {
        let n = self.count(limit)?;
        self.take(n)
    }
    fn floats<const N: usize>(&mut self) -> Option<[f32; N]> {
        let mut values = [0.; N];
        for v in &mut values {
            *v = f32::from_le_bytes(self.take(4)?.try_into().ok()?);
            if !v.is_finite() {
                return None;
            }
        }
        Some(values)
    }
}
fn decode(bytes: &[u8]) -> Option<(Library, Map, Vec<u8>)> {
    let mut r = Read(bytes);
    let map = Map {
        area: r.floats()?,
        first: r.count(4096)?,
        pages_x: r.count(256)?,
    };
    let pages = r.blob(MAX_BYTES)?.to_vec();
    if !pages.len().is_multiple_of(65536) {
        return None;
    }
    let mut vertices = Vec::new();
    for _ in 0..r.count(4_000_000)? {
        vertices.push(r.floats::<1>()?[0]);
    }
    let mut meshes = Vec::new();
    for _ in 0..r.count(512)? {
        let range = r.int()? as u32..r.int()? as u32;
        if range.start > range.end {
            return None;
        }
        let [radius, height_scale] = r.floats()?;
        let look = if r.count(1)? == 1 {
            Look::Solid
        } else {
            Look::Foliage
        };
        let recolour = r.count(1)?;
        let rgb = r.floats()?;
        let source = String::from_utf8(r.blob(512)?.to_vec()).ok()?;
        let sprite_layer = r.count(4096)?;
        let tile = r.count(4)?;
        let top = r.floats()?;
        let mean_color = r.floats()?;
        let mut crowns = Vec::new();
        for _ in 0..r.count(128)? {
            crowns.push(Crown {
                center: r.floats()?,
                half: r.floats()?,
            });
        }
        meshes.push(Mesh {
            range,
            radius,
            height_scale,
            look,
            recolour: (recolour == 1).then_some(rgb),
            source,
            sprite_layer,
            sprite_tile: (tile < 4).then_some(tile as u8),
            top,
            mean_color,
            crowns,
        });
    }
    if r.count(super::Species::ALL.len())? != super::Species::ALL.len() {
        return None;
    }
    let mut variants = Vec::new();
    for _ in super::Species::ALL {
        let mut v = Vec::new();
        for _ in 0..r.count(512)? {
            let i = r.int()?;
            if i >= meshes.len() {
                return None;
            }
            v.push(i as u16);
        }
        if v.is_empty() {
            return None;
        }
        variants.push(v);
    }
    if !r.0.is_empty() {
        return None;
    }
    Some((
        Library {
            vertices,
            meshes,
            variants,
        },
        map,
        pages,
    ))
}

fn encode_atlas(atlas: &super::Atlas) -> Vec<u8> {
    let mut out = Vec::new();
    blob(&mut out, atlas.region.code.as_bytes());
    put(&mut out, atlas.tone.dark as usize);
    put(&mut out, atlas.tone.light as usize);
    floats(&mut out, &[atlas.tone.yellow, atlas.median_height]);
    for rgb in atlas.palette {
        out.extend(rgb);
    }
    put(&mut out, atlas.cols);
    blob(&mut out, &atlas.cells);
    put(&mut out, atlas.pictures.len());
    for picture in &atlas.pictures {
        blob(&mut out, &picture.indices);
    }
    put(&mut out, atlas.placements.len());
    for (&(x, z), &(picture, rotation)) in &atlas.placements {
        out.extend(x.to_le_bytes());
        out.extend(z.to_le_bytes());
        put(&mut out, picture as usize);
        put(&mut out, rotation as usize);
    }
    for (x, z, cols, rows, bytes) in [
        (
            atlas.water.x0,
            atlas.water.z0,
            atlas.water.cols,
            atlas.water.rows,
            &atlas.water.dist,
        ),
        (
            atlas.coast.x0,
            atlas.coast.z0,
            atlas.coast.cols,
            atlas.coast.rows,
            &atlas.coast.blocked,
        ),
    ] {
        out.extend(x.to_le_bytes());
        out.extend(z.to_le_bytes());
        put(&mut out, cols);
        put(&mut out, rows);
        blob(&mut out, bytes);
    }
    out
}
fn decode_atlas(bytes: &[u8]) -> Option<super::Atlas> {
    use super::{Atlas, Picture, Tone, WaterMap};
    let mut r = Read(bytes);
    let code = std::str::from_utf8(r.blob(16)?).ok()?;
    let region = super::REGIONS.into_iter().find(|v| v.code == code)?;
    let tone = Tone {
        dark: r.count(255)? as i32,
        light: r.count(255)? as i32,
        yellow: r.floats::<1>()?[0],
    };
    let median_height = r.floats::<1>()?[0];
    let mut palette = [[0; 3]; 256];
    for rgb in &mut palette {
        rgb.copy_from_slice(r.take(3)?);
    }
    let cols = r.count(65536)?;
    let cells = r.blob(MAX_BYTES)?.to_vec();
    if cols == 0 || !cells.len().is_multiple_of(cols) {
        return None;
    }
    let mut pictures = Vec::new();
    for _ in 0..r.count(65536)? {
        let indices = r.blob(65536)?.to_vec();
        if indices.len() != 65536 {
            return None;
        }
        pictures.push(Picture { indices });
    }
    let mut placements = BTreeMap::new();
    for _ in 0..r.count(1_000_000)? {
        let x = r.int()? as i32;
        let z = r.int()? as i32;
        let picture = r.int()?;
        if picture >= pictures.len() {
            return None;
        }
        placements.insert((x, z), (picture as u16, r.count(3)? as u8));
    }
    let mut grid = |bits: bool| -> Option<_> {
        let x = i64::from_le_bytes(r.take(8)?.try_into().ok()?);
        let z = i64::from_le_bytes(r.take(8)?.try_into().ok()?);
        let cols = r.count(65536)?;
        let rows = r.count(65536)?;
        let count = cols.checked_mul(rows)?;
        let data = r.blob(MAX_BYTES)?.to_vec();
        if data.len() != if bits { count.div_ceil(8) } else { count } {
            return None;
        }
        Some((x, z, cols, rows, data))
    };
    let (x0, z0, cols_w, rows, dist) = grid(false)?;
    let water = WaterMap {
        x0,
        z0,
        cols: cols_w,
        rows,
        dist,
    };
    let (x0, z0, cols_c, rows, blocked) = grid(true)?;
    let coast = super::coast::Coast {
        x0,
        z0,
        cols: cols_c,
        rows,
        blocked,
    };
    if !r.0.is_empty() {
        return None;
    }
    Some(Atlas {
        region,
        tone,
        palette,
        pictures,
        placements,
        water,
        coast,
        seams: Default::default(),
        median_height,
        cells,
        cols,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_ground_heights_change_with_the_missions_foundations() {
        let mut world = tore_world::test_support::terrain();
        let key = |world: &Terrain| {
            Cache::with_profile(
                (
                    std::env::temp_dir().join("tore-foundation-cache-test"),
                    Vec::new(),
                ),
                &BTreeMap::new(),
                world,
                &[[0; 3]; 256],
                0,
                "test",
                "test",
            )
            .key
        };
        let before = key(&world);
        world.foundations = tore_world::grounding::pads::Mesh::build(
            vec![tore_world::grounding::pads::Pad {
                min: [1000., 1000.],
                max: [2000., 2000.],
                height: 0.,
            }],
            2,
            2,
            |x, z| world.raw_height(x, z),
        );
        assert_ne!(before, key(&world));
    }

    #[test]
    fn override_content_changes_and_removal_change_cache_key() {
        let root = std::env::temp_dir().join(format!("tore-top-overrides-{}", std::process::id()));
        let cache = root.join("vegetation-cache");
        let directory = override_directory(&cache);
        std::fs::create_dir_all(&directory).unwrap();
        let fingerprint = || {
            let mut hash = Fnv1a64::new();
            hash_overrides(&mut hash, &cache);
            hash.finish()
        };
        let empty = fingerprint();
        let path = directory.join("SYNTHETIC-crown-01-top.png");
        std::fs::write(&path, b"first").unwrap();
        let first = fingerprint();
        std::fs::write(&path, b"other").unwrap();
        let other = fingerprint();
        assert_ne!(first, other, "same-length edits invalidate the cache");
        assert_ne!(empty, first);
        std::fs::remove_file(path).unwrap();
        assert_eq!(empty, fingerprint());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn grass_cache_roundtrip_is_bounded_and_rejects_truncation() {
        let atlas = super::super::Atlas::synthetic(vec![12; 65536], 0);
        let world = tore_world::test_support::terrain();
        let map = super::super::grass::Map::build(&atlas, &world, &BTreeMap::new(), None);
        let bytes = encode_grass(&map);
        assert_eq!(decode_grass(&bytes), Some(map));
        for len in [0, 15, 20, bytes.len() - 1] {
            assert!(decode_grass(&bytes[..len]).is_none());
        }
        let mut bad = bytes.clone();
        bad[8..12].copy_from_slice(&f32::INFINITY.to_le_bytes());
        assert!(decode_grass(&bad).is_none());
        bad = bytes;
        bad[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_grass(&bad).is_none());
    }
    #[test]
    fn canopy_and_cards_cache_hit_miss_invalidation_and_corruption() {
        let root =
            std::env::temp_dir().join(format!("tore-vegetation-cache-{}", std::process::id()));
        let mut world = tore_world::test_support::terrain();
        let mut resources = BTreeMap::from([("SYNTHETIC".into(), vec![1, 2, 3])]);
        let palette = [[80; 3]; 256];
        let make = |world: &Terrain,
                    resources: &BTreeMap<String, Vec<u8>>,
                    generation: &[u8],
                    version: &str| {
            Cache::with_profile(
                (root.clone(), generation.to_vec()),
                resources,
                world,
                &palette,
                0,
                "mixed",
                version,
            )
        };
        let cache = make(&world, &resources, b"generation-1", "code-data-1");
        let _ = std::fs::remove_file(&cache.path);
        assert!(cache.read().is_none());
        let mut pages = Vec::new();
        let mut library = Library::build(
            &BTreeMap::new(),
            &BTreeMap::new(),
            &palette,
            &mut pages,
            |p| p.len() / 65536,
        );
        let first = pages.len() / 65536;
        library.add_pack(
            "pack",
            super::super::region("UKR"),
            &palette,
            &mut pages,
            first,
        );
        let map = Map {
            area: [0., 0., 128., 128.],
            first: pages.len() / 65536,
            pages_x: 1,
        };
        pages.extend(vec![17; 65536]);
        cache.write(&library, map, &pages);
        let seams = crate::terrain_seams::Map {
            cols: 2,
            rows: 2,
            bytes: vec![0; 4 * crate::terrain_seams::STRIDE],
            mean_error: 0.001,
            materials: vec![0; 32],
            ..Default::default()
        };
        cache.write_seams(&seams);
        assert_eq!(cache.read_seams(), Some(seams));

        let mut atlas = super::super::Atlas::synthetic(vec![10; 65536], 0);
        atlas.region = super::super::region("UKR");
        atlas.cols = 2;
        atlas.cells = vec![10; 4];
        cache.write_atlas(&atlas);
        let restored = cache.read_atlas().expect("atlas hit");
        assert_eq!(encode_atlas(&atlas), encode_atlas(&restored));
        let (actual, actual_map, actual_pages) = cache.read().expect("cache hit");
        assert_eq!(
            encode(&actual, actual_map, &actual_pages),
            encode(&library, map, &pages)
        );
        for changed in [
            make(&world, &resources, b"generation-2", "code-data-1"),
            make(&world, &resources, b"generation-1", "code-data-2"),
        ] {
            assert_ne!(changed.key, cache.key);
            assert!(changed.read().is_none());
        }
        resources.get_mut("SYNTHETIC").unwrap()[0] = 9;
        assert_ne!(
            make(&world, &resources, b"generation-1", "code-data-1").key,
            cache.key
        );
        let previous = make(&world, &resources, b"generation-1", "code-data-1").key;
        world.neutral_airfield_ground.insert((4, 4));
        assert_ne!(
            make(&world, &resources, b"generation-1", "code-data-1").key,
            previous
        );
        world.layout = "TVIET.MM".into();
        assert_ne!(
            make(&world, &resources, b"generation-1", "code-data-1").key,
            cache.key
        );
        let mut bytes = std::fs::read(&cache.path).unwrap();
        bytes[31] ^= 1;
        std::fs::write(&cache.path, bytes).unwrap();
        assert!(cache.read().is_none());
        std::fs::remove_file(&cache.path).unwrap();
        std::fs::remove_file(&cache.atlas_file().path).unwrap();
        std::fs::remove_file(cache.path.with_extension("seams")).unwrap();
        let _ = std::fs::remove_dir(root);
    }
}
