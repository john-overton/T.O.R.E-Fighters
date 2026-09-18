//! Shared, opinionated surface lighting and geometric shadows.
//! Player-visible rules: docs/spec/surface-lighting.md.
use crate::terrain::{Camera, World};
use wgpu::util::DeviceExt;

pub const MAP_SIZE: u32 = 2048;
const EXTENTS: [f32; 3] = [256., 8192., 131072.];
const DEPTH: f32 = 262144.;

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.into_iter().zip(b).map(|(x, y)| x * y).sum()
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
#[cfg(test)]
fn unit(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt();
    v.map(|x| x / length)
}

// Each projection is three rows plus scale/bias parameters. Coordinates are
// relative to the view origin to retain precision in million-foot theaters.
fn projection(origin: [f32; 3], sun: [f32; 3], extent: f32) -> [f32; 16] {
    // Continuous orthonormal basis on the sun's upper hemisphere. Switching
    // reference axes at an elevation threshold rotates the entire shadow grid.
    let sign = if sun[1] >= 0. { 1. } else { -1. };
    let a = -1. / (sign + sun[1]);
    let right = [
        1. + sign * sun[0] * sun[0] * a,
        -sign * sun[0],
        sign * sun[0] * sun[2] * a,
    ];
    let up = cross(sun, right);
    let texel = 2. * extent / MAP_SIZE as f32;
    let snap = |axis| {
        let p = dot(origin, axis);
        (p - (p / texel).round() * texel) / extent
    };
    [
        right[0] / extent,
        right[1] / extent,
        right[2] / extent,
        snap(right),
        up[0] / extent,
        up[1] / extent,
        up[2] / extent,
        snap(up),
        -sun[0] / (2. * DEPTH),
        -sun[1] / (2. * DEPTH),
        -sun[2] / (2. * DEPTH),
        0.5,
        texel,
        extent,
        DEPTH,
        0.,
    ]
}

// Visible circular-segment area and centroid, matching the rendered solid disc.
fn visible_sun(sun: [f32; 3], radius: f32) -> ([f32; 3], f32) {
    let elevation = sun[1].clamp(-1., 1.).asin();
    let x = (elevation / radius).clamp(-1., 1.);
    if x <= -1. {
        return (sun, 0.);
    }
    if x >= 1. {
        return (sun, 1.);
    }
    let root = (1. - x * x).sqrt();
    let area = (-x).acos() + x * root;
    if area <= 1e-6 {
        return (sun, 0.);
    }
    let center = (elevation + (2. / 3.) * radius * root.powi(3) / area).max(0.);
    let horizontal = sun[0].hypot(sun[2]).max(1e-6);
    (
        [
            sun[0] / horizontal * center.cos(),
            center.sin(),
            sun[2] / horizontal * center.cos(),
        ],
        area / std::f32::consts::PI,
    )
}

/// Area-weighted normals shared across terrain triangle and material boundaries.
/// The separate stream leaves geometry and shadow-caster positions untouched.
pub(crate) fn terrain_normals(vertices: &[f32]) -> Vec<f32> {
    use std::collections::HashMap;
    let key = |v: &[f32]| [v[0], v[1], v[2]].map(|x| if x == 0. { 0 } else { x.to_bits() });
    let mut sums = HashMap::<[u32; 3], [f64; 3]>::new();
    for triangle in vertices.chunks_exact(30) {
        let a: [f64; 3] =
            std::array::from_fn(|i| f64::from(triangle[10 + i]) - f64::from(triangle[i]));
        let b: [f64; 3] =
            std::array::from_fn(|i| f64::from(triangle[20 + i]) - f64::from(triangle[i]));
        let mut normal = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        if normal[1] < 0. {
            normal = normal.map(|v| -v);
        }
        for vertex in triangle.chunks_exact(10) {
            let sum = sums.entry(key(vertex)).or_default();
            for i in 0..3 {
                sum[i] += normal[i];
            }
        }
    }
    vertices
        .chunks_exact(10)
        .flat_map(|v| {
            let n = sums.get(&key(v)).copied().unwrap_or([0., 1., 0.]);
            let length = n.iter().map(|v| v * v).sum::<f64>().sqrt();
            if length > 0. {
                n.map(|v| (v / length) as f32)
            } else {
                [0., 1., 0.]
            }
        })
        .collect()
}

pub struct SurfaceLighting {
    pub layout: wgpu::BindGroupLayout,
    pub bind: wgpu::BindGroup,
    caster_bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    views: Vec<wgpu::TextureView>,
    pipeline: wgpu::RenderPipeline,
    terrain_pipeline: wgpu::RenderPipeline,
}

impl SurfaceLighting {
    pub fn new(
        device: &wgpu::Device,
        material: &wgpu::BindGroupLayout,
        shader: &wgpu::ShaderModule,
    ) -> Self {
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Surface light and shadow projections"),
            contents: &[0; 256],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("World geometric shadow maps"),
            size: wgpu::Extent3d {
                width: MAP_SIZE,
                height: MAP_SIZE,
                depth_or_array_layers: 3,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let views = (0..3)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let all = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Filtered shadow comparison"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(256),
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shared surface lighting"),
            entries: &[
                uniform_entry,
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("World shadow receivers"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&all),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        // Casters must not bind the sampled depth texture while writing it.
        let caster_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow projection only"),
            entries: &[uniform_entry],
        });
        let caster_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow caster projection"),
            layout: &caster_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow casting"),
            bind_group_layouts: &[material, &caster_layout],
            push_constant_ranges: &[],
        });
        let make = |entry, terrain| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("shadow_vertex"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 40,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[],
                }),
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState { constant: if terrain { 2 } else { 1 }, slope_scale: if terrain { 3. } else { 1. }, clamp: 0. },
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        Self {
            layout,
            bind,
            caster_bind,
            uniform,
            views,
            pipeline: make("shadow_fragment", false),
            terrain_pipeline: make("terrain_shadow_fragment", true),
        }
    }

    pub fn prepare(&self, queue: &wgpu::Queue, camera: &Camera, world: &World) -> bool {
        let layer = world.weather.sample(f64::from(camera.position[1]));
        let sun = layer
            .as_ref()
            .and_then(|l| crate::celestial::visual_sun_direction(l, &world.weather))
            .unwrap_or([0., -1., 0.]);
        let moon = layer
            .as_ref()
            .map(|l| crate::celestial::rotate([0., 0., 1.], [l.moon_azimuth, l.moon_elevation]))
            .unwrap_or([0., 1., 0.]);
        let radius = world
            .celestial
            .as_ref()
            .and_then(|c| c.solid_sun_radius())
            .unwrap_or(0.5_f32.to_radians());
        let (light, strength) = visible_sun(sun, radius);
        let active = world.smooth_weather && strength > 0.;
        let mut values = Vec::with_capacity(64);
        for extent in EXTENTS {
            values.extend(projection(camera.position, light, extent));
        }
        values.extend([
            camera.position[0],
            camera.position[1],
            camera.position[2],
            f32::from(active),
        ]);
        values.extend([
            light[0],
            light[1],
            light[2],
            f32::from(world.smooth_weather),
        ]);
        values.extend([moon[0], moon[1], moon[2], 0.]);
        values.extend([strength, sun[1], radius.tan(), 0.]);
        let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        queue.write_buffer(&self.uniform, 0, &bytes);
        active
    }

    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        terrain: (&wgpu::BindGroup, &wgpu::Buffer, u32),
        objects: &[(&wgpu::BindGroup, &wgpu::Buffer, u32)],
    ) {
        for (cascade, view) in self.views.iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("World geometry shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.terrain_pipeline);
            pass.set_bind_group(1, &self.caster_bind, &[]);
            pass.set_bind_group(0, terrain.0, &[]);
            pass.set_vertex_buffer(0, terrain.1.slice(..));
            pass.draw(0..terrain.2, cascade as u32..cascade as u32 + 1);
            pass.set_pipeline(&self.pipeline);
            for (bind, buffer, count) in objects {
                pass.set_bind_group(0, *bind, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, cascade as u32..cascade as u32 + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terrain_normals_join_shared_edges_across_materials() {
        let positions = [
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 0., 1.],
            [1., 0., 0.],
            [1., 1., 1.],
            [0., 0., 1.],
        ];
        let mut vertices = Vec::new();
        for (i, p) in positions.into_iter().enumerate() {
            vertices.extend([
                p[0] + 1_000_000.,
                p[1],
                p[2] + 600_000.,
                0.,
                0.,
                (i / 3) as f32,
                0.,
                0.,
                0.,
                100.,
            ]);
        }
        let normals = terrain_normals(&vertices);
        assert_eq!(&normals[3..6], &normals[9..12]);
        assert_eq!(&normals[6..9], &normals[15..18]);
        for normal in normals.chunks_exact(3) {
            assert!((normal.iter().map(|x| x * x).sum::<f32>() - 1.).abs() < 1e-6);
            assert!(normal[1] > 0.);
        }
        assert!(normals[3] < 0. && normals[4] < 1.);
    }

    #[test]
    fn shadow_basis_does_not_switch_axes_near_overhead_sun() {
        let direction = |y: f32| [(1. - y * y).sqrt(), y, 0.];
        let a = projection([0.; 3], direction(0.94999), 256.);
        let b = projection([0.; 3], direction(0.95001), 256.);
        for i in [0, 1, 2, 4, 5, 6] {
            assert!((a[i] - b[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn disc_fraction_preserves_one_and_thirty_percent_shadow_strength() {
        let radius = 0.5_f32.to_radians();
        for (height, expected) in [(-0.93433, 0.01), (-0.31969, 0.30), (0., 0.50), (1., 1.)] {
            let elevation = height * radius;
            let (_, fraction) = visible_sun([elevation.cos(), elevation.sin(), 0.], radius);
            assert!((fraction - expected).abs() < 0.00002);
        }
    }

    #[test]
    fn visible_disc_keeps_sunset_shadows_until_the_last_limb_sets() {
        let radius = 0.5_f32.to_radians();
        for (degrees, expected) in [(1., 1.), (0., 0.5), (-1., 0.)] {
            let angle = degrees * std::f32::consts::PI / 180.;
            let (direction, strength) = visible_sun([angle.cos(), angle.sin(), 0.], radius);
            assert!((strength - expected).abs() < 1e-5);
            if strength > 0. {
                assert!(direction[1] > 0.);
            }
        }
        let mut previous = 1.;
        for step in -100..=100 {
            let angle = -(step as f32) / 100. * radius;
            let (direction, strength) = visible_sun([angle.cos(), angle.sin(), 0.], radius);
            assert!(strength <= previous + 1e-5);
            assert!(previous - strength < 0.02);
            assert!(direction.iter().all(|v| v.is_finite()));
            if strength > 0. {
                assert!(direction[1] >= 0.);
            }
            previous = strength;
        }
    }

    #[test]
    fn shadow_projection_follows_light_and_stays_stable_in_large_worlds() {
        for sun in [[0., 1., 0.], unit([1., 0.1, 0.]), unit([-1., 1., 1.])] {
            for extent in EXTENTS {
                let origin = [1_000_000., 5000., 600_000.];
                let m = projection(origin, sun, extent);
                let project = |v| {
                    [
                        dot(m[0..3].try_into().unwrap(), v) + m[3],
                        dot(m[4..7].try_into().unwrap(), v) + m[7],
                        dot(m[8..11].try_into().unwrap(), v) + m[11],
                    ]
                };
                let center = project([0.; 3]);
                let toward = project(sun.map(|v| v * 100.));
                assert!((center[0] - toward[0]).abs() < 1e-5);
                assert!((center[1] - toward[1]).abs() < 1e-5);
                assert!(toward[2] < center[2], "the sunward occluder must win depth");
                assert!(center[0].abs() <= 1. / MAP_SIZE as f32 + 1e-5);
                assert!(center[1].abs() <= 1. / MAP_SIZE as f32 + 1e-5);
                assert!(m.iter().all(|v| v.is_finite()));
            }
        }
    }
}
