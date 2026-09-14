//! Extensible 3D pass. World data/camera are independent of wgpu; UI composites afterward.
use crate::terrain::{Camera, World};
use wgpu::util::DeviceExt;
fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
pub struct SimRenderer {
    pipeline: wgpu::RenderPipeline,
    aircraft: Option<(wgpu::BindGroup, wgpu::Buffer, u32)>,
    bind: wgpu::BindGroup,
    sky_pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    vertices: wgpu::Buffer,
    count: u32,
    spare_depth: Option<([u32; 2], wgpu::TextureView)>,
    depth: wgpu::TextureView,
    size: [u32; 2],
}
impl SimRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        world: &World,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Simulation terrain"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terrain.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Simulation terrain"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 36,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3],
                }],
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
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sky_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sky layout"),
            bind_group_layouts: &[&pipeline.get_bind_group_layout(0)],
            push_constant_ranges: &[],
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Retail sky preview"),
            layout: Some(&sky_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("sky_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("sky_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera and atmosphere"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Original Ukraine terrain tiles"),
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: (world.texture_pixels.len() / (256 * 256 * 4) + 1) as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[world.texture_pixels.as_slice(), world.sky_pixels.as_slice()].concat(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1024),
                rows_per_image: Some(256),
            },
            wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: (world.texture_pixels.len() / (256 * 256 * 4) + 1) as u32,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Simulation scene bindings"),
            layout: &pipeline.get_bind_group_layout(0),
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
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Retail T2 terrain mesh"),
            contents: &bytes(&world.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Self {
            pipeline,
            aircraft: None,
            sky_pipeline,
            bind,
            uniform,
            vertices,
            count: (world.vertices.len() / 9) as u32,
            spare_depth: None,
            depth: Self::depth(device, width, height),
            size: [width, height],
        }
    }
    pub fn aircraft(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hornet: &crate::aircraft::Hornet,
        vertices: &[f32],
    ) {
        if self.aircraft.is_none() {
            let pic = &hornet.atlas;
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Retail Hornet atlas"),
                size: wgpu::Extent3d {
                    width: pic.width as u32,
                    height: pic.height as u32,
                    depth_or_array_layers: 2,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let mut pixels = pic.rgba(&hornet.palette);
            for (i, p) in pixels.chunks_exact_mut(4).enumerate() {
                if pic.pixels[i] == 255 {
                    p[3] = 0;
                }
            }
            pixels.extend_from_within(..);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pic.width as u32 * 4),
                    rows_per_image: Some(pic.height as u32),
                },
                wgpu::Extent3d {
                    width: pic.width as u32,
                    height: pic.height as u32,
                    depth_or_array_layers: 2,
                },
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Aircraft textures"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Aircraft pose"),
                size: 2 * 1024 * 1024,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.aircraft = Some((bind, buffer, 0));
        }
        if let Some((_, buffer, count)) = &mut self.aircraft {
            assert!(vertices.len() * 4 <= 2 * 1024 * 1024);
            if !vertices.is_empty() {
                queue.write_buffer(buffer, 0, &bytes(vertices));
            }
            *count = (vertices.len() / 9) as u32;
        }
    }
    pub fn hide_aircraft(&mut self) {
        if let Some((_, _, count)) = &mut self.aircraft {
            *count = 0;
        }
    }
    fn depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Simulation depth"),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: [u32; 2],
        camera: &Camera,
        world: &World,
    ) {
        if self.size != size {
            let next = match self.spare_depth.take() {
                Some((old_size, depth)) if old_size == size => depth,
                _ => Self::depth(device, size[0], size[1]),
            };
            self.spare_depth = Some((self.size, std::mem::replace(&mut self.depth, next)));
            self.size = size;
        }
        let sky = world.palette[235];
        let mut uniform = camera.uniform(
            size[0] as f32 / (size[1] as f32 * camera.view_fraction),
            0.000004,
            sky,
        );
        uniform[7] = (world.texture_pixels.len() / (256 * 256 * 4)) as f32;
        queue.write_buffer(&self.uniform, 0, &bytes(&uniform));
        let linear = |v: u8| ((v as f64 / 255.0 + 0.055) / 1.055).powf(2.4);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Simulation world"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: linear(sky[0]),
                        g: linear(sky[1]),
                        b: linear(sky[2]),
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_viewport(
            0.,
            0.,
            size[0] as f32,
            size[1] as f32 * camera.view_fraction,
            0.,
            1.,
        );
        pass.set_pipeline(&self.sky_pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..3, 0..1);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..self.count, 0..1);
        if let Some((bind, vertices, count)) = &self.aircraft {
            pass.set_bind_group(0, bind, &[]);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.draw(0..*count, 0..1);
        }
    }
}
