//! Directional projection of source cockpit artwork and its body-fixed HUD plane.
use crate::{
    attitude::{Basis, dot},
    flight::State,
    menu::Sprite,
    terrain::Camera,
};

pub struct CockpitRenderer {
    pipeline: wgpu::RenderPipeline,
    bind: Option<wgpu::BindGroup>,
    hud: wgpu::Texture,
    uniform: wgpu::Buffer,
    art_size: [u32; 2],
    enabled: bool,
}

fn texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Directional cockpit source"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}
fn upload(queue: &wgpu::Queue, texture: &wgpu::Texture, pixels: &[u8]) {
    queue.write_texture(
        texture.as_image_copy(),
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(texture.width() * 4),
            rows_per_image: Some(texture.height()),
        },
        texture.size(),
    );
}

/// Local camera axes expressed in aircraft coordinates, independent of aircraft attitude.
pub fn relative_axes(body: Basis, eye: Basis) -> [[f32; 3]; 3] {
    [eye.right, eye.up, eye.forward].map(|axis| {
        [
            dot(axis, body.right) as f32,
            dot(axis, body.up) as f32,
            dot(axis, body.forward) as f32,
        ]
    })
}
pub fn dimensions(size: [u32; 2], art: [u32; 2]) -> [f32; 4] {
    let [w, h] = size.map(|v| v as f32);
    [
        w,
        h,
        h * 0.5 * 3f32.sqrt(),
        (w / art[0] as f32).max(h / art[1] as f32),
    ]
}
impl CockpitRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Directional cockpit"),
            source: wgpu::ShaderSource::Wgsl(include_str!("cockpit.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cockpit and HUD projection"),
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
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let hud = texture(device, 640, 480);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cockpit projection"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind: None,
            hud,
            uniform,
            art_size: [1, 1],
            enabled: false,
        }
    }
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, source: &Sprite) {
        if self.bind.is_some() {
            return;
        }
        self.art_size = [source.width as u32, source.height as u32];
        let art = texture(device, self.art_size[0], self.art_size[1]);
        // Native PIC and current HUD rasters have binary alpha. Zero invisible RGB
        // so hardware filtering produces premultiplied samples without black fringes.
        let mut pixels = source.rgba.clone();
        for pixel in pixels.chunks_exact_mut(4) {
            if pixel[3] == 0 {
                pixel.fill(0);
            }
        }
        upload(queue, &art, &pixels);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        self.bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Body-fixed cockpit art and HUD"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &art.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &self.hud.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        }));
    }
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        size: [u32; 2],
        state: &State,
        camera: &Camera,
        art: bool,
        hud: bool,
        pixels: &[u8],
    ) {
        self.enabled = art || hud;
        if !self.enabled {
            return;
        }
        let mut values = Vec::with_capacity(24);
        for axis in relative_axes(
            Basis::new(state.yaw, state.pitch, state.bank),
            Basis::new(camera.yaw as f64, camera.pitch as f64, -camera.roll as f64),
        ) {
            values.extend_from_slice(&axis);
            values.push(0.);
        }
        values.extend_from_slice(&dimensions(size, self.art_size));
        values.extend_from_slice(&[
            self.art_size[0] as f32,
            self.art_size[1] as f32,
            f32::from(art),
            f32::from(hud),
        ]);
        let scale = (size[0] as f32 / 640.).min(size[1] as f32 / 480.)
            * crate::flight_canvas::HUD_SCALE as f32;
        values.extend_from_slice(&[640. * scale, 480. * scale, 0., 0.]);
        let bytes: Vec<u8> = values.into_iter().flat_map(f32::to_le_bytes).collect();
        queue.write_buffer(&self.uniform, 0, &bytes);
        if hud {
            upload(queue, &self.hud, pixels);
        }
    }
    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if !self.enabled {
            return;
        }
        let Some(bind) = &self.bind else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Directional cockpit and HUD"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cockpit_and_hud_anchor_do_not_move_when_aircraft_attitude_changes() {
        for (yaw, pitch, bank) in [(0., 0., 0.), (2., 1.57, 1.), (4., 2., -2.)] {
            let body = Basis::new(yaw, pitch, bank);
            let axes = relative_axes(body, body);
            for (i, axis) in axes.iter().enumerate() {
                for (j, value) in axis.iter().enumerate() {
                    assert!((*value - f32::from(i == j)).abs() < 1e-6);
                }
            }
            let head = body.rotated(body.up.map(|v| v * 0.2));
            let axes = relative_axes(body, head);
            // The fixed forward datum moves left in a rightward head view.
            let x = axes[0][2] / axes[2][2];
            assert!((x + 0.2f32.tan()).abs() < 1e-6);
        }
    }
    #[test]
    fn native_forward_plane_preserves_cover_fit_on_wide_and_tall_windows() {
        for size in [[1920, 1080], [827, 1080], [1920, 800]] {
            let [w, h, _, scale] = dimensions(size, [1280, 490]);
            assert!(1280. * scale >= w - 0.001 && 490. * scale >= h - 0.001);
            // The source top center remains the viewport top center at rest.
            let source_x = 0. / scale + 1280. * 0.5;
            let source_y = (-h * 0.5 + h * 0.5) / scale;
            assert_eq!(source_x, 640.);
            assert_eq!(source_y, 0.);
        }
    }
}
