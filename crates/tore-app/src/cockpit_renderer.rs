//! Aircraft-forward cockpit and HUD with flat translation and directional fading.
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
    palette: wgpu::Texture,
    uniform: wgpu::Buffer,
    art_size: [u32; 2],
    enabled: bool,
    pub mirror_target: wgpu::Texture,
    mirror_rects: [[f32; 4]; 3],
    pub mirrors_visible: bool,
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
/// Fitted screen-space presentation. Bottom anchoring avoids a floating image edge;
/// the forward datum projects opposite head-look without clamping to the screen.
pub fn layout(size: [u32; 2], art: [u32; 2], yaw: f32, pitch: f32, zoom: f32) -> [f32; 4] {
    let [w, h] = size.map(|v| v as f32);
    let scale = ((w / art[0] as f32).max(h / art[1] as f32) * zoom).max(w / art[0] as f32);
    let opacity = ((65. - yaw.to_degrees().abs()) / 20.)
        .min((55. - pitch.to_degrees().abs()) / 20.)
        .clamp(0., 1.);
    // Project the aircraft-forward datum with the world camera focal length,
    // but translate the whole raster rather than tilting its rectangular plane.
    // Invisible rear/up views need no projection across the tangent singularity.
    if opacity == 0. {
        return [0., 0., scale, 0.];
    }
    let focal = h * 0.5 * 3f32.sqrt() * zoom;
    [
        -focal * yaw.tan() / pitch.cos(),
        focal * pitch.tan(),
        scale,
        opacity,
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
            size: 112,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mirror_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Live rear mirrors"),
            size: wgpu::Extent3d {
                width: crate::mirrors::SIZE[0],
                height: crate::mirrors::SIZE[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        Self {
            mirror_target,
            mirror_rects: [[0.; 4]; 3],
            mirrors_visible: false,
            pipeline,
            bind: None,
            hud,
            palette: texture(device, 256, 1),
            uniform,
            art_size: [1, 1],
            enabled: false,
        }
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &Sprite,
        indexed: &tore_formats::Pic,
        id: tore_formats::aircraft::AircraftId,
    ) {
        // Called when an aircraft is prepared, including selection changes.
        // A previous binding belongs to the previous aircraft's cockpit.
        self.art_size = [source.width as u32, source.height as u32];
        let art = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Indexed original cockpit"),
            size: wgpu::Extent3d {
                width: self.art_size[0],
                height: self.art_size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let pixels: Vec<u8> = indexed
            .pixels
            .iter()
            .zip(&indexed.mask)
            .flat_map(|(&index, &visible)| [index, u8::from(visible)])
            .collect();
        queue.write_texture(
            art.as_image_copy(),
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.art_size[0] * 2),
                rows_per_image: Some(self.art_size[1]),
            },
            art.size(),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let masks = if std::env::var("TORE_MIRRORS").as_deref() == Ok("0") {
            None
        } else {
            crate::mirrors::masks(source, id)
        };
        self.mirror_rects = masks.as_ref().map_or([[0.; 4]; 3], |m| m.rects);
        println!(
            "Cockpit mirrors: {} reviewed regions, {}x{} rear feed, every visible frame",
            self.mirror_rects
                .iter()
                .filter(|r| r[2] > 0. && r[3] > 0.)
                .count(),
            crate::mirrors::SIZE[0],
            crate::mirrors::SIZE[1]
        );
        let mask = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Source mirror silhouettes"),
            size: art.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mask_pixels = masks.map_or_else(|| vec![0; source.width * source.height], |m| m.pixels);
        queue.write_texture(
            mask.as_image_copy(),
            &mask_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.art_size[0]),
                rows_per_image: Some(self.art_size[1]),
            },
            mask.size(),
        );
        self.bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Body-fixed cockpit art and HUD"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        &self.palette.create_view(&Default::default()),
                    ),
                },
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
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &mask.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        &self.mirror_target.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        }));
    }
    /// Upload the palette shared with the CPU HUD raster.
    pub fn weather(&self, queue: &wgpu::Queue, colors: &[[u8; 3]; 256]) {
        let pixels: Vec<u8> = colors
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect();
        upload(queue, &self.palette, &pixels);
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
        self.mirrors_visible = false;
        if !self.enabled {
            return;
        }
        let axes = relative_axes(
            Basis::new(state.yaw, state.pitch, state.bank),
            Basis::new(camera.yaw as f64, camera.pitch as f64, -camera.roll as f64),
        );
        let yaw = axes[2][0].atan2(axes[2][2]);
        let pitch = axes[2][1].clamp(-1., 1.).asin();
        let placement = layout(size, self.art_size, yaw, pitch, camera.zoom);
        self.mirrors_visible = art
            && placement[3] > 0.
            && self.mirror_rects.iter().any(|r| {
                let x = (size[0] as f32 - self.art_size[0] as f32 * placement[2]) * 0.5
                    + placement[0]
                    + r[0] * placement[2];
                let base = size[1] as f32 - self.art_size[1] as f32 * placement[2];
                let y = base.max(base * 0.5) + placement[1] + r[1] * placement[2];
                r[2] > 0.
                    && x < size[0] as f32
                    && x + r[2] * placement[2] > 0.
                    && y < size[1] as f32
                    && y + r[3] * placement[2] > 0.
            });
        let mut values = Vec::with_capacity(28);
        values.extend_from_slice(&placement);
        values.extend_from_slice(&[size[0] as f32, size[1] as f32, camera.zoom, 0.]);
        values.extend_from_slice(&[
            self.art_size[0] as f32,
            self.art_size[1] as f32,
            f32::from(art),
            f32::from(hud),
        ]);
        let scale = (size[0] as f32 / 640.).min(size[1] as f32 / 480.)
            * crate::flight_canvas::HUD_SCALE as f32
            * camera.zoom;
        values.extend_from_slice(&[640. * scale, 480. * scale, 0., 0.]);
        for rect in self.mirror_rects {
            values.extend_from_slice(&rect);
        }
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
    fn forward_datum_moves_opposite_head_look_without_edge_clamping() {
        for size in [[1920, 1080], [827, 1080], [1920, 800]] {
            for zoom in [0.5, 1., 2., 4.] {
                let center = layout(size, [1280, 490], 0., 0., zoom);
                let right = layout(size, [1280, 490], 30f32.to_radians(), 0., zoom);
                let left = layout(size, [1280, 490], -30f32.to_radians(), 0., zoom);
                // At 30 degrees, the forward datum is half a viewport height
                // to the opposite side, scaled by zoom, even beyond art margins.
                assert!((right[0] + size[1] as f32 * 0.5 * zoom).abs() < 0.001);
                assert_eq!(left[0], -right[0]);
                assert_eq!(right[1], 0.);
                assert_eq!(right[2], center[2]);
                let farther = layout(size, [1280, 490], 40f32.to_radians(), 0., zoom);
                assert!(farther[0] < right[0]);
                let up = layout(size, [1280, 490], 0., 20f32.to_radians(), zoom);
                assert!(up[1] > 0.);
                assert_eq!(up[0], 0.);
                for (yaw, pitch) in [
                    (std::f32::consts::PI, 0.),
                    (0., std::f32::consts::FRAC_PI_2),
                ] {
                    let hidden = layout(size, [1280, 490], yaw, pitch, zoom);
                    assert_eq!(hidden[3], 0.);
                    assert!(hidden.iter().all(|v| v.is_finite()));
                }
            }
        }
    }
}
