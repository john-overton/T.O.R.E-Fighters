//! Static texture upload plus a vertex-buffer-free grass draw.
use super::{Map, PAGE, VERTICES, candidates, root_lift, side};
use crate::{graphics::VegDensity, terrain::Terrain};

pub struct Blades {
    layout: wgpu::BindGroupLayout,
    bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    area: [f32; 4],
    pages_x: u32,
    count: u32,
}
impl Blades {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        samples: u32,
        material: &wgpu::BindGroupLayout,
        lighting: &wgpu::BindGroupLayout,
        map: &Map,
    ) -> Self {
        let texture_entry = |binding, dimension, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: dimension,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blade grass resources"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(64),
                    },
                    count: None,
                },
                texture_entry(
                    1,
                    wgpu::TextureViewDimension::D2Array,
                    wgpu::TextureSampleType::Uint,
                ),
                texture_entry(
                    2,
                    wgpu::TextureViewDimension::D2,
                    wgpu::TextureSampleType::Float { filterable: false },
                ),
            ],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Blade grass uniforms"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let upload = |label, size: wgpu::Extent3d, format, bytes: &[u8], stride| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
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
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size.width * stride),
                    rows_per_image: Some(size.height),
                },
                size,
            );
            texture
        };
        let empty_mask = vec![0; PAGE * PAGE * 4];
        let mask = if map.mask.is_empty() {
            &empty_mask
        } else {
            &map.mask
        };
        let habitat = upload(
            "Grass habitat, cached 128 ft classifications",
            wgpu::Extent3d {
                width: PAGE as u32,
                height: PAGE as u32,
                depth_or_array_layers: (mask.len() / (PAGE * PAGE * 4)) as u32,
            },
            wgpu::TextureFormat::Rgba8Uint,
            mask,
            4,
        )
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let ground = upload(
            "Grass terrain vertices and picture lookup",
            wgpu::Extent3d {
                width: map.terrain_size[0],
                height: map.terrain_size[1],
                depth_or_array_layers: 1,
            },
            wgpu::TextureFormat::Rgba32Float,
            &crate::vegetation_renderer::bytes_of_f32(&map.terrain),
            16,
        )
        .create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blade grass"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&habitat),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&ground),
                },
            ],
        });
        let pipeline = pipeline(device, shader, format, samples, material, lighting, &layout);
        Self {
            layout,
            bind,
            uniform,
            pipeline,
            area: map.area,
            pages_x: map.pages_x,
            count: 0,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn rebuild(
        &mut self,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        samples: u32,
        material: &wgpu::BindGroupLayout,
        lighting: &wgpu::BindGroupLayout,
    ) {
        self.pipeline = pipeline(
            device,
            shader,
            format,
            samples,
            material,
            lighting,
            &self.layout,
        );
    }
    /// Fixed-size stack uniform. No placement, chunk enumeration, packing or
    /// blade buffer upload is allowed here, including while the camera moves.
    pub fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        world: &Terrain,
        density: VegDensity,
        radius: f32,
    ) {
        let wind = world.wind();
        let values = [
            self.area[0],
            self.area[1],
            self.area[2],
            self.area[3],
            side(density) as f32,
            radius,
            self.pages_x as f32,
            root_lift(),
            (world.weather.ticks() as f64 / 120.) as f32,
            wind[0] as f32,
            wind[2] as f32,
            0.,
            0.,
            0.,
            0.,
            0.,
        ];
        let mut bytes = [0; 64];
        for (i, v) in values.into_iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        queue.write_buffer(&self.uniform, 0, &bytes);
        self.count = if self.pages_x > 0 && radius > 0. {
            candidates(density)
        } else {
            0
        };
    }
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(2, &self.bind, &[]);
        pass.draw(0..VERTICES, 0..self.count);
    }
    pub fn count(&self) -> u64 {
        u64::from(self.count)
    }
}
#[allow(clippy::too_many_arguments)]
fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    samples: u32,
    material: &wgpu::BindGroupLayout,
    lighting: &wgpu::BindGroupLayout,
    resources: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Procedural blades"),
        bind_group_layouts: &[material, lighting, resources],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Opaque grass blades"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("blade_vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("blade_fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::GreaterEqual,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        multiview: None,
        cache: None,
    })
}
