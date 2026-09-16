//! Source lens-flare circles over a completed world view, before cockpit/UI.
//! Filtered GPU colors are resolved to their nearest live palette entry only
//! within a flare circle; outside it the world image passes through unchanged.
use crate::terrain::{Camera, World};

pub struct LensFlare {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    palette: wgpu::Texture,
    maps: wgpu::Texture,
    format: wgpu::TextureFormat,
    backing: Option<(wgpu::Texture, wgpu::BindGroup)>,
}
fn texture(
    device: &wgpu::Device,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Lens flare image"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}
/// Source 0x4b4990 gates and offsets, normalized to the host drawable size.
/// Native x/y shifts and whole-pixel circle rounding are projection adaptations.
pub fn circles(world: &World, camera: &Camera, size: [u32; 2]) -> Vec<[f32; 4]> {
    let Some(celestial) = &world.celestial else {
        return vec![];
    };
    if !world.glare_enabled() {
        return vec![];
    }
    let Some(layer) = world.weather.sample(camera.position[1] as f64) else {
        return vec![];
    };
    let sun = if world.smooth_weather {
        let Some(sun) =
            crate::celestial::continuous_sun_direction(&layer, world.weather.seconds_of_day())
        else {
            return vec![];
        };
        if crate::celestial::glare_strength(world, f64::from(camera.position[1]), sun) <= 0. {
            return vec![];
        }
        sun
    } else {
        let Some(angles) =
            tore_sim::environment::sun_angles(&layer, world.weather.seconds_of_day())
        else {
            return vec![];
        };
        if angles[1] < 182 {
            return vec![];
        }
        crate::celestial::rotate([0., 0., 1.], angles)
    };
    let basis = camera.uniform(1., [0.; 4], [0; 3]);
    let dot = |i: usize| (0..3).map(|j| sun[j] * basis[i + j]).sum::<f32>();
    let z = dot(12);
    if z <= 0. {
        return vec![];
    }
    let focal = size[1] as f32 * camera.view_fraction * 0.5 * 1.7320508 * camera.zoom;
    let offset = [dot(4) * focal / z, -dot(8) * focal / z];
    let [w, h] = [size[0] as f32, size[1] as f32 * camera.view_fraction];
    if offset[0].abs() > w * 9. / 16.
        || offset[1].abs() > h * 9. / 16.
        || offset[0].abs() * 320. / w + offset[1].abs() * 240. / h < 10.
    {
        return vec![];
    }
    celestial
        .flare
        .circles
        .iter()
        .map(|c| {
            [
                w * 0.5 + offset[0] * f32::from(c.offset_percent) / 100.,
                h * 0.5 + offset[1] * f32::from(c.offset_percent) / 100.,
                f32::from(c.radius) * w / 320.,
                f32::from(c.fill - 265),
            ]
        })
        .collect()
}
impl LensFlare {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Original lens-flare fills"),
            source: wgpu::ShaderSource::Wgsl(include_str!("lens_flare.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Source lens-flare composition"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Lens flare circles"),
                size: 272,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            palette: texture(
                device,
                [256, 1],
                wgpu::TextureFormat::Rgba8UnormSrgb,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            ),
            maps: texture(
                device,
                [256, 1],
                wgpu::TextureFormat::Rg8Uint,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            ),
            format,
            backing: None,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &World,
        camera: &Camera,
        size: [u32; 2],
        palette: &[[u8; 3]; 256],
    ) -> Option<wgpu::TextureView> {
        let circles = circles(world, camera, size);
        if circles.is_empty() {
            return None;
        }
        if self
            .backing
            .as_ref()
            .is_none_or(|(t, _)| t.width() != size[0] || t.height() != size[1])
        {
            let source = texture(
                device,
                size,
                self.format,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Lens flare inputs"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &source.create_view(&Default::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &self.palette.create_view(&Default::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &self.maps.create_view(&Default::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.uniform.as_entire_binding(),
                    },
                ],
            });
            self.backing = Some((source, bind));
        }
        let strength = if world.smooth_weather {
            world
                .weather
                .sample(f64::from(camera.position[1]))
                .and_then(|layer| {
                    crate::celestial::continuous_sun_direction(
                        &layer,
                        world.weather.seconds_of_day(),
                    )
                })
                .map_or(0., |sun| {
                    crate::celestial::glare_strength(world, f64::from(camera.position[1]), sun)
                })
        } else {
            1.
        };
        let mut values = vec![circles.len() as f32, strength, 0., 0.];
        values.extend(circles.into_iter().flatten());
        values.resize(68, 0.);
        let bytes: Vec<u8> = values.into_iter().flat_map(f32::to_le_bytes).collect();
        queue.write_buffer(&self.uniform, 0, &bytes);
        let pixels: Vec<u8> = palette
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect();
        let maps = world.weather.configuration().flare_fills();
        let maps: Vec<u8> = (0..256).flat_map(|i| [maps[0][i], maps[1][i]]).collect();
        for (t, b, pitch) in [(&self.palette, pixels, 1024), (&self.maps, maps, 512)] {
            queue.write_texture(
                t.as_image_copy(),
                &b,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pitch),
                    rows_per_image: Some(1),
                },
                t.size(),
            );
        }
        Some(
            self.backing
                .as_ref()
                .unwrap()
                .0
                .create_view(&Default::default()),
        )
    }
    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Lens flare after world"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.backing.as_ref().unwrap().1, &[]);
        pass.draw(0..3, 0..1);
    }
}
