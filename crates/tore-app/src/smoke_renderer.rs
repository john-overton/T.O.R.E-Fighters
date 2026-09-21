//! Camera-facing original smoke artwork. Simulation owns emission and lifetime.
use crate::terrain::Camera;
use tore_formats::Pic;
use tore_sim::combat::smoke::{Kind, MAX_CONTRAIL_PUFFS, MAX_PUFFS, Puff, Smoke};
const MAX_INSTANCES: usize = MAX_PUFFS + MAX_CONTRAIL_PUFFS;
const INSTANCE_BYTES: usize = 6 * 4;

struct Frustum {
    view: Vec<f32>,
    horizontal: f64,
    vertical: f64,
    near: f64,
}
impl Frustum {
    fn new(camera: &Camera, aspect: f32) -> Self {
        let vertical = 1. / (3_f64.sqrt() * f64::from(camera.zoom));
        Self {
            view: camera.uniform(aspect, [0.; 4], [0; 3]),
            horizontal: vertical * f64::from(aspect),
            vertical,
            near: f64::from(camera.near_clip),
        }
    }
    fn distance(&self, puff: &Puff) -> Option<f64> {
        let offset: [f64; 3] = std::array::from_fn(|i| puff.position[i] - f64::from(self.view[i]));
        let axis = |start: usize| {
            (0..3)
                .map(|i| offset[i] * f64::from(self.view[start + i]))
                .sum::<f64>()
        };
        let depth = axis(12);
        let radius = puff.radius();
        (depth >= self.near
            && depth <= 2200000.
            && axis(4).abs() <= depth * self.horizontal + radius
            && axis(8).abs() <= depth * self.vertical + radius)
            .then(|| offset.iter().map(|v| v * v).sum())
    }
}
pub struct SmokeRenderer {
    pipeline: wgpu::RenderPipeline,
    bind: Option<wgpu::BindGroup>,
    buffer: wgpu::Buffer,
    puffs: Vec<Puff>,
    count: u32,
}
impl SmokeRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
    ) -> Self {
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Original smoke billboards"), layout: None,
            vertex: wgpu::VertexState { module: shader, entry_point: Some("smoke_vertex"), compilation_options: Default::default(), buffers: &[wgpu::VertexBufferLayout {
                array_stride: INSTANCE_BYTES as u64, step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32,2=>Float32,3=>Float32],
            }] },
            fragment: Some(wgpu::FragmentState {module:shader,entry_point:Some("smoke_fragment"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState{format,blend:Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})]}),
            primitive:wgpu::PrimitiveState { cull_mode:None,..Default::default() },
            depth_stencil:Some(wgpu::DepthStencilState {format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:false,depth_compare:wgpu::CompareFunction::Less,stencil:Default::default(),bias:Default::default()}),
            multisample:Default::default(),multiview:None,cache:None,
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bounded smoke puffs"),
            size: (MAX_INSTANCES * INSTANCE_BYTES) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind: None,
            buffer,
            puffs: Vec::new(),
            count: 0,
        }
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniform: &wgpu::Buffer,
        weather: (&wgpu::Texture, &wgpu::TextureView),
        art: &Pic,
        smoke: [&Smoke; 2],
    ) {
        if self.bind.is_none() {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Runtime indexed SMOKE.PIC"),
                size: wgpu::Extent3d {
                    width: art.width as u32,
                    height: art.height as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let indices: Vec<_> = art
                .pixels
                .iter()
                .zip(&art.mask)
                .map(|(&index, &visible)| if visible { index } else { 255 })
                .collect();
            queue.write_texture(
                texture.as_image_copy(),
                &indices,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(art.width as u32),
                    rows_per_image: Some(art.height as u32),
                },
                texture.size(),
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let palette = weather.0.create_view(&Default::default());
            self.bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Smoke texture and camera"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&palette),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(weather.1),
                    },
                ],
            }));
        }
        self.puffs.clear();
        self.puffs.extend(
            smoke
                .into_iter()
                .flat_map(|s| s.puffs.iter().rev())
                .take(MAX_INSTANCES)
                .cloned(),
        );
    }
    pub fn update(&mut self, queue: &wgpu::Queue, camera: &Camera, aspect: f32) {
        let frustum = Frustum::new(camera, aspect);
        // Retain the whole history for other views and for later camera turns.
        // Only the visible instances are sorted and uploaded for this draw.
        let mut visible: Vec<_> = self
            .puffs
            .iter()
            .filter_map(|p| frustum.distance(p).map(|distance| (distance, p)))
            .collect();
        visible.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut instances = Vec::with_capacity(visible.len() * INSTANCE_BYTES);
        for (_, p) in visible {
            let start = match p.kind {
                Kind::Aircraft => 0.,
                Kind::Missile | Kind::Contrail => 94.,
            };
            for value in p.position.map(|v| v as f32).into_iter().chain([
                p.radius() as f32,
                start,
                p.opacity(),
            ]) {
                instances.extend(value.to_le_bytes());
            }
        }
        self.count = (instances.len() / INSTANCE_BYTES) as u32;
        if !instances.is_empty() {
            queue.write_buffer(&self.buffer, 0, &instances);
        }
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.count == 0 {
            return;
        }
        let Some(bind) = &self.bind else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::util::DeviceExt;

    #[test]
    fn frustum_keeps_overlapping_edges_and_rejects_offscreen_puffs() {
        let mut camera = Camera::new();
        camera.position = [0.; 3];
        camera.yaw = 0.;
        camera.pitch = 0.;
        let puff = |position| Puff {
            position,
            kind: Kind::Contrail,
            age: 480,
        };
        let view = Frustum::new(&camera, 1.);
        assert!(view.distance(&puff([0., 0., 100.])).is_some());
        assert!(view.distance(&puff([70., 0., 100.])).is_some());
        assert!(view.distance(&puff([73., 0., 100.])).is_none());
        assert!(view.distance(&puff([0., 73., 100.])).is_none());
        assert!(view.distance(&puff([0., 0., -100.])).is_none());
        camera.yaw = std::f32::consts::PI;
        let rear = Frustum::new(&camera, 1.);
        assert!(rear.distance(&puff([0., 0., -100.])).is_some());
        assert!(rear.distance(&puff([0., 0., 100.])).is_none());
        assert_eq!(MAX_INSTANCES, 80192);
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn gpu_all_smoke_families_follow_live_palette_without_lighting_cutouts() {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .unwrap();
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .unwrap();
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Production smoke lighting test"),
                source: wgpu::ShaderSource::Wgsl(
                    format!(
                        "{}\n{}",
                        include_str!("surface_lighting.wgsl"),
                        include_str!("terrain.wgsl")
                    )
                    .into(),
                ),
            });
            let texture = |format, width, height, usage| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
            };
            let palette = texture(
                wgpu::TextureFormat::Rgba8Unorm,
                256,
                1,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            );
            let remaps = texture(
                wgpu::TextureFormat::R8Uint,
                256,
                1,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            );
            let remap_view = remaps.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let mut camera = Camera::new();
            camera.position = [0.; 3];
            camera.yaw = 0.;
            camera.pitch = 0.;
            let mut values = camera.uniform(1., [0., 1000000., 0., 0.], [0; 3]);
            values.resize(340, 0.);
            values[336] = 1.;
            let raw: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
            let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &raw,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let mut art = Pic {
                width: 256,
                height: 43,
                pixels: vec![255; 256 * 43],
                mask: vec![true; 256 * 43],
                palette: vec![],
                glyphs: vec![],
            };
            for start in [0, 94] {
                for y in 8..35 {
                    for x in 8..35 {
                        art.pixels[y * 256 + start + x] = 100;
                    }
                }
            }
            let output = texture(
                wgpu::TextureFormat::Rgba8Unorm,
                64,
                64,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            );
            let output_view = output.create_view(&Default::default());
            let depth = texture(
                wgpu::TextureFormat::Depth32Float,
                64,
                64,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
            let depth_view = depth.create_view(&Default::default());
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 64 * 64 * 4,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            for kind in [Kind::Aircraft, Kind::Missile, Kind::Contrail] {
                let mut smoke = Smoke::default();
                for tick in 1..=12 {
                    if kind == Kind::Contrail {
                        smoke.step([]);
                        smoke.contrails([(1, [0., 0., 30. + f64::from(tick) * 0.001])]);
                    } else {
                        smoke.step([([0., 0., 30.], kind)]);
                    }
                }
                assert_eq!(smoke.puffs.len(), 1);
                if kind == Kind::Contrail {
                    smoke.puffs[0].age = 10800;
                }
                let alpha = smoke.puffs[0].opacity();
                let mut renderer =
                    SmokeRenderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, &shader);
                renderer.prepare(
                    &device,
                    &queue,
                    &uniform,
                    (&palette, &remap_view),
                    &art,
                    [&smoke, &Smoke::default()],
                );
                renderer.update(&queue, &camera, 1.);
                // The same bound particle texture must respond to subsequent
                // weather palette writes, including returning to daylight.
                for rgb in [
                    [220u8, 220, 220],
                    [80, 40, 25],
                    [12, 15, 30],
                    [220, 220, 220],
                ] {
                    let colors: Vec<_> = (0..256)
                        .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
                        .collect();
                    queue.write_texture(
                        palette.as_image_copy(),
                        &colors,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(1024),
                            rows_per_image: Some(1),
                        },
                        palette.size(),
                    );
                    let mut encoder = device.create_command_encoder(&Default::default());
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &output_view,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(
                                wgpu::RenderPassDepthStencilAttachment {
                                    view: &depth_view,
                                    depth_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(1.),
                                        store: wgpu::StoreOp::Store,
                                    }),
                                    stencil_ops: None,
                                },
                            ),
                            ..Default::default()
                        });
                        renderer.draw(&mut pass);
                    }
                    encoder.copy_texture_to_buffer(
                        output.as_image_copy(),
                        wgpu::TexelCopyBufferInfo {
                            buffer: &readback,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(256),
                                rows_per_image: Some(64),
                            },
                        },
                        output.size(),
                    );
                    queue.submit([encoder.finish()]);
                    let (tx, rx) = std::sync::mpsc::channel();
                    readback
                        .slice(..)
                        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
                    device
                        .poll(wgpu::PollType::Wait {
                            submission_index: None,
                            timeout: Some(std::time::Duration::from_secs(30)),
                        })
                        .unwrap();
                    rx.recv().unwrap().unwrap();
                    let pixels = readback.slice(..).get_mapped_range();
                    let center = &pixels[(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 4];
                    for i in 0..3 {
                        let c = f32::from(rgb[i]) / 255.;
                        let linear = if c <= 0.04045 {
                            c / 12.92
                        } else {
                            ((c + 0.055) / 1.055).powf(2.4)
                        };
                        assert!(
                            (f32::from(center[i]) - linear * alpha * 255.).abs() <= 2.,
                            "{kind:?} {rgb:?}: {center:?}"
                        );
                    }
                    assert!((f32::from(center[3]) - alpha * 255.).abs() <= 1.);
                    assert_eq!(&pixels[..4], &[0; 4]);
                    let edge = (32. - smoke.puffs[0].radius() * 3_f64.sqrt() / 30. * 32. * 0.85)
                        .floor() as usize;
                    let offset = (edge * 64 + edge) * 4;
                    assert_eq!(
                        &pixels[offset..offset + 4],
                        &[0; 4],
                        "lighting must not fill transparent sprite corners"
                    );
                    drop(pixels);
                    readback.unmap();
                }
            }
        });
    }
}
