//! Shared CPU resource sizing for device creation and imported-theater audits.
//! No GPU or retail fixture is needed to exercise packing and boundary cases.
use crate::{scenery::Scenery, vegetation_renderer::stream};

#[derive(Clone, Copy, Debug)]
pub struct Pages {
    pub size: wgpu::Extent3d,
    pub across: u32,
}
impl Pages {
    /// Pack 256-square logical pages into square atlas layers. Prefer 1024,
    /// growing only if the actual device's array-layer limit requires it.
    pub fn new(count: usize, limits: &wgpu::Limits) -> Result<Self, String> {
        let count = u32::try_from(count.max(1)).map_err(|_| "too many vegetation pages")?;
        let max = limits.max_texture_dimension_2d / 256;
        let mut across = max.min(4);
        if across == 0 || limits.max_texture_array_layers == 0 {
            return Err("device cannot hold one 256-square vegetation page".into());
        }
        while count.div_ceil(across * across) > limits.max_texture_array_layers {
            if across == max {
                return Err("vegetation pages exceed device texture capacity".into());
            }
            across = (across * 2).min(max);
        }
        Ok(Self {
            size: wgpu::Extent3d {
                width: across * 256,
                height: across * 256,
                depth_or_array_layers: count.div_ceil(across * across),
            },
            across,
        })
    }

    /// CPU upload layout. The shader derives the same page address from the
    /// physical texture dimensions, so device-specific layouts need no cache.
    pub fn origin(self, page: u32) -> wgpu::Origin3d {
        let n = self.across;
        wgpu::Origin3d {
            x: page % n * 256,
            y: page / n % n * 256,
            z: page / (n * n),
        }
    }
}

pub fn texture(label: &str, size: wgpu::Extent3d, limits: &wgpu::Limits) -> Result<(), String> {
    if size.width == 0
        || size.height == 0
        || size.depth_or_array_layers == 0
        || size.width > limits.max_texture_dimension_2d
        || size.height > limits.max_texture_dimension_2d
        || size.depth_or_array_layers > limits.max_texture_array_layers
    {
        return Err(format!("{label}: {size:?} exceeds device texture limits"));
    }
    Ok(())
}

pub fn buffer(
    label: &str,
    size: u64,
    usage: wgpu::BufferUsages,
    limits: &wgpu::Limits,
) -> Result<(), String> {
    let binding = if usage.contains(wgpu::BufferUsages::STORAGE) {
        u64::from(limits.max_storage_buffer_binding_size)
    } else if usage.contains(wgpu::BufferUsages::UNIFORM) {
        u64::from(limits.max_uniform_buffer_binding_size)
    } else {
        limits.max_buffer_size
    };
    if size == 0 || size > binding.min(limits.max_buffer_size) {
        return Err(format!(
            "{label}: {size} bytes exceeds device buffer/binding limits"
        ));
    }
    Ok(())
}

pub fn instance_bytes(capacity: usize) -> u64 {
    ((2 * capacity.div_ceil(stream::PAGE) + stream::SPARE_PAGES) * stream::PAGE) as u64 * 64
}
pub const CHUNK_BYTES: u64 = (stream::SPARE_PAGES + 524_288 / stream::PAGE) as u64 * 8;

/// Largest buffers across every density setting, including the legacy tufts.
/// The shared art texture contains canopy, sprite atlases and all card pages.
pub fn validate(scene: &Scenery, limits: &wgpu::Limits) -> Result<(), String> {
    let pages = (scene.texture_indices.len() + scene.sky_indices.len()) / 65536;
    texture(
        "World art including vegetation",
        wgpu::Extent3d {
            width: 1024,
            height: 1024,
            depth_or_array_layers: pages.div_ceil(16).max(1) as u32,
        },
        limits,
    )?;
    let Some(lib) = scene.vegetation_meshes.as_ref() else {
        return Ok(());
    };
    if let Some(atlas) = &scene.vegetation {
        let size = atlas.seams.gpu_size();
        texture(
            "Terrain seam records",
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            limits,
        )?;
        let b = &atlas.seams.baked;
        texture(
            "Cached seam tints",
            wgpu::Extent3d {
                width: b.size[0].max(1),
                height: b.size[1].max(1),
                depth_or_array_layers: b.size[2].max(1),
            },
            limits,
        )?;
    }
    let grass = &scene.grass;
    Pages::new(grass.mask.len() / (256 * 256 * 4), limits)?;
    texture(
        "Grass terrain lookup",
        wgpu::Extent3d {
            width: grass.terrain_size[0].max(1),
            height: grass.terrain_size[1].max(1),
            depth_or_array_layers: 1,
        },
        limits,
    )?;
    use wgpu::BufferUsages as U;
    for (label, size, usage) in [
        ("Card vertices", (lib.vertices.len() * 4) as u64, U::VERTEX),
        (
            "Cull counts",
            (lib.meshes.len() as u64 * 2).max(1) * 4,
            U::STORAGE,
        ),
        ("Indirect draws", lib.meshes.len() as u64 * 32, U::STORAGE),
        (
            "Mesh table",
            (lib.meshes.len() as u64).max(1) * 48,
            U::STORAGE,
        ),
        ("Chunk table", CHUNK_BYTES, U::STORAGE),
        (
            "Legacy grass patches",
            crate::vegetation::MAX_PATCHES as u64 * 64,
            U::VERTEX,
        ),
        ("Unused caster binding", 96, U::STORAGE),
        ("Cull uniforms", 128, U::UNIFORM),
        ("Grass uniforms", 64, U::UNIFORM),
    ] {
        buffer(label, size, usage, limits)?;
    }
    for density in crate::graphics::VegDensity::ALL {
        let capacity = density.object_budget();
        buffer(
            "Instance slots",
            instance_bytes(capacity),
            U::STORAGE,
            limits,
        )?;
        buffer(
            "Visible instances",
            capacity as u64 * 96,
            U::STORAGE,
            limits,
        )?;
    }
    let max_groups = (CHUNK_BYTES / 8).max((lib.meshes.len() as u64).div_ceil(64));
    if max_groups > u64::from(limits.max_compute_workgroups_per_dimension)
        || limits.max_compute_invocations_per_workgroup < 256
        || limits.max_compute_workgroup_size_x < 256
    {
        return Err("vegetation cull exceeds device compute limits".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pgu_and_larger_habitat_arrays_fit_portable_limits_without_aliasing() {
        for limits in [
            wgpu::Limits::default(),
            wgpu::Limits::downlevel_defaults(),
            wgpu::Limits {
                max_texture_dimension_2d: 512,
                max_texture_array_layers: 2,
                ..wgpu::Limits::downlevel_defaults()
            },
        ] {
            let capacity =
                (limits.max_texture_dimension_2d / 256).pow(2) * limits.max_texture_array_layers;
            for count in [
                0,
                1,
                8,
                256,
                272,
                4097,
                capacity as usize,
                capacity as usize + 1,
            ] {
                let layout = Pages::new(count, &limits);
                if count > capacity as usize {
                    assert!(layout.is_err());
                    continue;
                }
                let layout = layout.unwrap();
                texture("habitat", layout.size, &limits).unwrap();
                let mut addresses = std::collections::BTreeSet::new();
                for page in 0..count as u32 {
                    let p = layout.origin(page);
                    assert!(
                        p.x + 256 <= layout.size.width
                            && p.y + 256 <= layout.size.height
                            && p.z < layout.size.depth_or_array_layers
                    );
                    assert!(addresses.insert((p.x, p.y, p.z)));
                }
            }
        }
        assert_eq!(
            Pages::new(272, &wgpu::Limits::downlevel_defaults())
                .unwrap()
                .size
                .depth_or_array_layers,
            17
        );
    }

    #[test]
    fn vegetation_ultra_buffers_fit_downlevel_binding_limits() {
        let limits = wgpu::Limits::downlevel_defaults();
        let n = crate::graphics::VegDensity::Ultra.object_budget();
        assert_eq!(instance_bytes(n), 117_440_512);
        buffer(
            "slots",
            instance_bytes(n),
            wgpu::BufferUsages::STORAGE,
            &limits,
        )
        .unwrap();
        buffer("drawn", n as u64 * 96, wgpu::BufferUsages::STORAGE, &limits).unwrap();
        assert!(buffer("too big", 134_217_729, wgpu::BufferUsages::STORAGE, &limits).is_err());
    }

    #[test]
    #[ignore = "requires TORE_DATA_DIR (a private full import) and TORE_VEG_LIMITS_OUT (output directory)"]
    fn every_imported_theater_and_variant_loads_and_fits_gpu_limits() {
        let Some(out) = std::env::var_os("TORE_VEG_LIMITS_OUT") else {
            eprintln!(
                "skipped: set TORE_VEG_LIMITS_OUT to a private output directory \
                 (with TORE_DATA_DIR at a full import) to run this audit"
            );
            return;
        };
        let assets = crate::reel::load_assets().unwrap();
        crate::vegetation_preview::limits_check(
            &assets.theater_resources,
            std::path::Path::new(&out),
        )
        .unwrap();
    }
}
