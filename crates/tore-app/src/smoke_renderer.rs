//! Camera-facing original smoke artwork. Simulation owns emission and lifetime.
use crate::{menu::Sprite, terrain::Camera};
use tore_sim::combat::smoke::{Kind, MAX_PUFFS, Puff, Smoke};
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
                array_stride: 32, step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32x2,3=>Float32],
            }] },
            fragment: Some(wgpu::FragmentState {module:shader,entry_point:Some("smoke_fragment"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState{format,blend:Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})]}),
            primitive:wgpu::PrimitiveState { cull_mode:None,..Default::default() },
            depth_stencil:Some(wgpu::DepthStencilState {format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:false,depth_compare:wgpu::CompareFunction::Less,stencil:Default::default(),bias:Default::default()}),
            multisample:Default::default(),multiview:None,cache:None,
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bounded smoke puffs"),
            size: (MAX_PUFFS * 6 * 32) as u64,
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
        art: &Sprite,
        smoke: [&Smoke; 2],
    ) {
        if self.bind.is_none() {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Runtime SMOKE.PIC"),
                size: wgpu::Extent3d {
                    width: art.width as u32,
                    height: art.height as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let mut rgba = art.rgba.clone();
            for pixel in rgba.chunks_exact_mut(4) {
                if pixel[3] == 0 {
                    pixel[..3].fill(0);
                }
            }
            queue.write_texture(
                texture.as_image_copy(),
                &rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(art.width as u32 * 4),
                    rows_per_image: Some(art.height as u32),
                },
                texture.size(),
            );
            let view = texture.create_view(&Default::default());
            self.bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Smoke texture and camera"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                ],
            }));
        }
        self.puffs.clear();
        self.puffs.extend(
            smoke
                .into_iter()
                .flat_map(|s| s.puffs.iter().rev())
                .take(MAX_PUFFS)
                .cloned(),
        );
    }
    pub fn update(&mut self, queue: &wgpu::Queue, camera: &Camera) {
        let distance = |p: &Puff| {
            (0..3)
                .map(|i| (p.position[i] - f64::from(camera.position[i])).powi(2))
                .sum::<f64>()
        };
        self.puffs
            .sort_by(|a, b| distance(b).total_cmp(&distance(a)));
        let mut vertices = Vec::<f32>::with_capacity(self.puffs.len() * 48);
        for p in &self.puffs {
            let start = match p.kind {
                Kind::Aircraft => 0.,
                Kind::Missile | Kind::Contrail => 94.,
            };
            let radius = p.radius() as f32;
            for [x, y] in [[0., 0.], [1., 0.], [1., 1.], [0., 0.], [1., 1.], [0., 1.]] {
                vertices.extend(p.position.map(|v| v as f32));
                vertices.extend([
                    (x * 2. - 1.) * radius,
                    (1. - y * 2.) * radius,
                    (start + 0.5 + x * 42.) / 256.,
                    (0.5 + y * 42.) / 43.,
                    p.opacity(),
                ]);
            }
        }
        self.count = (vertices.len() / 8) as u32;
        if !vertices.is_empty() {
            queue.write_buffer(
                &self.buffer,
                0,
                &vertices
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>(),
            );
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
        pass.draw(0..self.count, 0..1);
    }
}
