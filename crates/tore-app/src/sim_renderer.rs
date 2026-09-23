//! Extensible 3D pass. World data/camera are independent of wgpu; UI composites afterward.
use crate::terrain::{Camera, World};
use wgpu::util::DeviceExt;
fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
/// The shared `Scene` uniform, ending with the graphics `quality` and
/// `viewport` vectors.
const UNIFORM_BYTES: u64 = 1392;
type AircraftBatch = (wgpu::BindGroup, wgpu::Buffer, u32);
pub struct SimRenderer {
    lighting: crate::surface_lighting::SurfaceLighting,
    aircraft_visible: bool,
    lens_flare: crate::lens_flare::LensFlare,
    smoke: crate::smoke_renderer::SmokeRenderer,
    battle: Option<(wgpu::Buffer, u32)>,
    airports: Option<(wgpu::Buffer, u32)>,
    dummies: Vec<(tore_formats::aircraft::AircraftId, AircraftBatch)>,
    vapor: Option<(wgpu::Buffer, u32)>,
    p: Pipelines,
    shader: wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    material_layout: wgpu::BindGroupLayout,
    vapor_bind: wgpu::BindGroup,
    palette: wgpu::Texture,
    weather_tiles: wgpu::TextureView,
    canopy_visible: bool,
    aircraft: Option<(wgpu::BindGroup, wgpu::Buffer, u32)>,
    bind: wgpu::BindGroup,
    celestial_vertices: wgpu::Buffer,
    cloud_vertices: wgpu::Buffer,
    uniform: wgpu::Buffer,
    vertices: wgpu::Buffer,
    terrain_normals: wgpu::Buffer,
    count: u32,
    targets: Vec<Targets>,
    resample: wgpu::RenderPipeline,
    options: crate::graphics::Options,
    samples: u32,
}
/// World-pass attachments for one output size. The main view, mirrors and
/// instrument previews alternate sizes within a frame, so a few are cached.
struct Targets {
    output: [u32; 2],
    size: [u32; 2],
    samples: u32,
    depth: wgpu::TextureView,
    /// Multisampled color, resolved at the end of the world pass.
    color: Option<wgpu::TextureView>,
    /// The render-scale image and its resample bindings, when the world is
    /// drawn at a different size than the output.
    scaled: Option<(wgpu::TextureView, wgpu::BindGroup)>,
}
/// Every pipeline that draws into the world pass. They are rebuilt together
/// when the anti-aliasing sample count changes.
struct Pipelines {
    vapor_pipeline: wgpu::RenderPipeline,
    tracer_pipeline: wgpu::RenderPipeline,
    pipeline: wgpu::RenderPipeline,
    airport_pipeline: wgpu::RenderPipeline,
    airport_decal_pipeline: wgpu::RenderPipeline,
    terrain_pipeline: wgpu::RenderPipeline,
    canopy_depth_pipeline: wgpu::RenderPipeline,
    canopy_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    celestial_pipeline: wgpu::RenderPipeline,
    cloud_pipeline: wgpu::RenderPipeline,
}
impl Pipelines {
    fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        samples: u32,
        material_layout: &wgpu::BindGroupLayout,
        lighting_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let multisample = wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        };
        let surface_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shared world surface layout"),
            bind_group_layouts: &[material_layout, lighting_layout],
            push_constant_ranges: &[],
        });
        let mut surface_descriptor = wgpu::RenderPipelineDescriptor {
            label: Some("Simulation terrain"),
            layout: Some(&surface_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 40,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample,
            multiview: None,
            cache: None,
        };
        let pipeline = device.create_render_pipeline(&surface_descriptor);
        surface_descriptor.label = Some("Static airport depth-biased surfaces");
        surface_descriptor.fragment.as_mut().unwrap().entry_point = Some("airport_solid_fragment");
        surface_descriptor.depth_stencil.as_mut().unwrap().bias = wgpu::DepthBiasState {
            constant: -4,
            slope_scale: -1.0,
            clamp: 0.0,
        };
        let airport_pipeline = device.create_render_pipeline(&surface_descriptor);
        surface_descriptor.label = Some("Static airport coplanar texture details");
        surface_descriptor.fragment.as_mut().unwrap().entry_point = Some("airport_fragment");
        surface_descriptor
            .depth_stencil
            .as_mut()
            .unwrap()
            .bias
            .constant = -8;
        let airport_decal_pipeline = device.create_render_pipeline(&surface_descriptor);
        surface_descriptor.depth_stencil.as_mut().unwrap().bias = Default::default();
        surface_descriptor.fragment.as_mut().unwrap().entry_point = Some("fragment");
        let sky_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sky layout"),
            bind_group_layouts: &[material_layout, lighting_layout],
            push_constant_ranges: &[],
        });
        surface_descriptor.label = Some("Shoreline terrain");
        surface_descriptor.layout = Some(&sky_layout);
        surface_descriptor.fragment.as_mut().unwrap().entry_point = Some("terrain_fragment");
        let surface_buffers = surface_descriptor.vertex.buffers;
        let terrain_buffers = [
            surface_buffers[0].clone(),
            wgpu::VertexBufferLayout {
                array_stride: 12,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![5=>Float32x3],
            },
        ];
        surface_descriptor.vertex.entry_point = Some("terrain_vertex");
        surface_descriptor.vertex.buffers = &terrain_buffers;
        let terrain_pipeline = device.create_render_pipeline(&surface_descriptor);
        surface_descriptor.vertex.entry_point = Some("vertex");
        surface_descriptor.vertex.buffers = surface_buffers;
        // Share the aircraft material bindings. First select the nearest glass
        // without changing color, then blend that surface once over opaque art.
        surface_descriptor.label = Some("Canopy nearest depth");
        surface_descriptor.fragment.as_mut().unwrap().entry_point = Some("canopy_fragment");
        let depth_target = [Some(wgpu::ColorTargetState {
            format,
            blend: None,
            write_mask: wgpu::ColorWrites::empty(),
        })];
        surface_descriptor.fragment.as_mut().unwrap().targets = &depth_target;
        let canopy_depth_pipeline = device.create_render_pipeline(&surface_descriptor);
        surface_descriptor.label = Some("Canopy transparency");
        let glass_target = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        surface_descriptor.fragment.as_mut().unwrap().targets = &glass_target;
        let depth = surface_descriptor.depth_stencil.as_mut().unwrap();
        depth.depth_write_enabled = false;
        depth.depth_compare = wgpu::CompareFunction::Equal;
        let canopy_pipeline = device.create_render_pipeline(&surface_descriptor);
        surface_descriptor.label = Some("Additive luminous gun tracers");
        surface_descriptor.fragment.as_mut().unwrap().entry_point = Some("tracer_fragment");
        let tracer_target = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        surface_descriptor.fragment.as_mut().unwrap().targets = &tracer_target;
        surface_descriptor
            .depth_stencil
            .as_mut()
            .unwrap()
            .depth_compare = wgpu::CompareFunction::Less;
        let tracer_pipeline = device.create_render_pipeline(&surface_descriptor);
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Retail sky preview"),
            layout: Some(&sky_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("sky_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
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
            multisample,
            multiview: None,
            cache: None,
        });
        let celestial_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Original celestial primitives"), layout: Some(&sky_layout),
            vertex: wgpu::VertexState { module:shader,entry_point:Some("celestial_vertex"),compilation_options:Default::default(),
                buffers:&[wgpu::VertexBufferLayout {array_stride:40,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32]}]},
            fragment:Some(wgpu::FragmentState {module:shader,entry_point:Some("celestial_fragment"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState {format,blend:Some(wgpu::BlendState::ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})]}),
            primitive:Default::default(),depth_stencil:Some(wgpu::DepthStencilState {format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:false,depth_compare:wgpu::CompareFunction::Always,stencil:Default::default(),bias:Default::default()}),multisample,multiview:None,cache:None,
        });
        let cloud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("Original cloud sheets"),layout:Some(&sky_layout),
            vertex:wgpu::VertexState {module:shader,entry_point:Some("vertex"),compilation_options:Default::default(),buffers:&[wgpu::VertexBufferLayout {array_stride:40,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32]}]},
            fragment:Some(wgpu::FragmentState {module:shader,entry_point:Some("cloud_fragment"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState {format,blend:None,write_mask:wgpu::ColorWrites::ALL})]}),
            primitive:wgpu::PrimitiveState {cull_mode:None,..Default::default()},depth_stencil:Some(wgpu::DepthStencilState {format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:true,depth_compare:wgpu::CompareFunction::Less,stencil:Default::default(),bias:Default::default()}),multisample,multiview:None,cache:None,
        });
        // Wing vapor is five one-pixel line segments per side, exactly as
        // `_DrawStreamer@12` draws them, so it needs its own blended pipeline.
        let vapor_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Wing vapor"),
            layout: None,
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vapor_vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 28,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("vapor_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample,
            multiview: None,
            cache: None,
        });
        Self {
            vapor_pipeline,
            tracer_pipeline,
            pipeline,
            airport_pipeline,
            airport_decal_pipeline,
            terrain_pipeline,
            canopy_depth_pipeline,
            canopy_pipeline,
            sky_pipeline,
            celestial_pipeline,
            cloud_pipeline,
        }
    }
}
impl SimRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        world: &World,
        options: crate::graphics::Options,
        samples: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Simulation terrain"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}",
                    include_str!("surface_lighting.wgsl"),
                    include_str!("terrain.wgsl")
                )
                .into(),
            ),
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("World surface material"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let lighting =
            crate::surface_lighting::SurfaceLighting::new(device, &material_layout, &shader);
        let pipelines = Pipelines::new(
            device,
            &shader,
            format,
            samples,
            &material_layout,
            &lighting.layout,
        );
        let celestial_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Celestial vertices"),
            size: 256 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cloud_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cloud vertices"),
            size: 256 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera and atmosphere"),
            size: UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Original Ukraine terrain tiles"),
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: ((world.texture_indices.len() + world.sky_indices.len())
                    / (256 * 256)) as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
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
            &[
                world.texture_indices.as_slice(),
                world.sky_indices.as_slice(),
            ]
            .concat(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(256),
            },
            wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: ((world.texture_indices.len() + world.sky_indices.len())
                    / (256 * 256)) as u32,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let palette = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Live weather palette"),
            size: wgpu::Extent3d {
                width: 256,
                height: 11,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let palette_view = palette.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Simulation scene bindings"),
            layout: &pipelines.pipeline.get_bind_group_layout(0),
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
                    resource: wgpu::BindingResource::TextureView(&palette_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&palette_view),
                },
            ],
        });
        // Vapor reads this view's camera and palette, with its own derived layout.
        let vapor_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Wing vapor bindings"),
            layout: &pipelines.vapor_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&palette_view),
                },
            ],
        });
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Retail T2 terrain mesh"),
            contents: &bytes(&world.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let terrain_normals = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shared terrain lighting normals"),
            contents: &bytes(&crate::surface_lighting::terrain_normals(&world.vertices)),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Self {
            terrain_normals,
            lighting,
            aircraft_visible: true,
            lens_flare: crate::lens_flare::LensFlare::new(device, format),
            smoke: crate::smoke_renderer::SmokeRenderer::new(device, format, &shader, samples),
            battle: None,
            airports: None,
            vapor: None,
            p: pipelines,
            shader,
            format,
            material_layout,
            vapor_bind,
            palette,
            weather_tiles: view,
            canopy_visible: false,
            aircraft: None,
            dummies: Vec::new(),
            celestial_vertices,
            cloud_vertices,
            bind,
            uniform,
            vertices,
            count: (world.vertices.len() / 10) as u32,
            targets: Vec::new(),
            resample: Self::resample_pipeline(device, format),
            options,
            samples,
        }
    }
    pub fn smoke(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        art: &tore_formats::Pic,
        smoke: [&tore_sim::combat::smoke::Smoke; 2],
    ) {
        self.smoke.prepare(
            device,
            queue,
            &self.uniform,
            (&self.palette, &self.weather_tiles),
            art,
            smoke,
        );
    }
    pub fn combat(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vertices: &[f32]) {
        if self.battle.is_none() {
            self.battle = Some((
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Bounded combat geometry"),
                    size: 8 * 1024 * 1024,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                0,
            ));
        }
        if let Some((buffer, count)) = &mut self.battle {
            let length = vertices.len().min((8 * 1024 * 1024 / 4 / 30) * 30);
            if length > 0 {
                queue.write_buffer(buffer, 0, &bytes(&vertices[..length]));
            }
            *count = (length / 10) as u32;
        }
    }
    pub fn airports(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vertices: &[f32]) {
        if self.airports.is_none() {
            self.airports = Some((
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Imported static airport geometry"),
                    size: 32 * 1024 * 1024,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                0,
            ));
        }
        if let Some((buffer, count)) = &mut self.airports {
            assert!(
                vertices.len() * 4 <= 32 * 1024 * 1024,
                "validated static scene exceeds GPU budget"
            );
            let length = vertices.len();
            if length > 0 {
                queue.write_buffer(buffer, 0, &bytes(&vertices[..length]));
            }
            *count = (length / 10) as u32;
        }
    }
    /// Seven floats per vertex: position then premultiplied-free RGBA.
    pub fn vapor(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vertices: &[f32]) {
        if self.vapor.is_none() {
            self.vapor = Some((
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Wing vapor"),
                    size: 64 * 1024,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                0,
            ));
        }
        if let Some((buffer, count)) = &mut self.vapor {
            let length = vertices.len().min((64 * 1024 / 4 / 14) * 14);
            if length > 0 {
                queue.write_buffer(buffer, 0, &bytes(&vertices[..length]));
            }
            *count = (length / 7) as u32;
        }
    }

    pub fn dummies(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        geometry: Vec<(&crate::aircraft::Airframe, Vec<f32>)>,
    ) {
        let ownship = self.aircraft.take();
        let visible = self.aircraft_visible;
        let canopy = self.canopy_visible;
        let mut old = std::mem::take(&mut self.dummies);
        for (model, vertices) in geometry {
            self.aircraft = old
                .iter()
                .position(|(id, _)| *id == model.profile.id)
                .map(|i| old.swap_remove(i).1);
            self.aircraft(device, queue, model, &vertices);
            self.dummies
                .push((model.profile.id, self.aircraft.take().unwrap()));
        }
        self.aircraft = ownship;
        self.aircraft_visible = visible;
        self.canopy_visible = canopy;
    }
    pub fn clear_aircraft(&mut self) {
        self.aircraft = None;
        self.canopy_visible = false;
    }
    pub fn aircraft(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hornet: &crate::aircraft::Airframe,
        vertices: &[f32],
    ) {
        self.aircraft_visible = true;
        if self.aircraft.is_none() {
            let pic = &hornet.atlas;
            // The aircraft atlas takes the same index-plus-palette path as the
            // terrain, with its own airframe palette rather than the weather one.
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Retail aircraft atlas"),
                size: wgpu::Extent3d {
                    width: pic.width as u32,
                    height: pic.height as u32,
                    depth_or_array_layers: 2,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            // write_texture needs a 256-byte row pitch; pad each source row.
            let pitch = pic.width.div_ceil(256) * 256;
            let mut indices = Vec::with_capacity(pitch * pic.height * 2);
            for row in 0..pic.height {
                for column in 0..pitch {
                    let at = row * pic.width + column;
                    indices.push(match pic.pixels.get(at) {
                        Some(index) if column < pic.width && pic.mask[at] => *index,
                        _ => 255,
                    });
                }
            }
            indices.extend_from_within(..);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &indices,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pitch as u32),
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
            // Both reviewed exterior atlases have no embedded palette. Native
            // Remap and the palette worker share the world's indexed palette.
            assert!(pic.palette.is_empty(), "unreviewed aircraft atlas palette");
            let palette_view = self.palette.create_view(&Default::default());
            let engine_view = hornet
                .engine_material
                .as_ref()
                .map(|image| image.upload(device, queue));
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Aircraft textures"),
                layout: &self.p.pipeline.get_bind_group_layout(0),
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
                        resource: wgpu::BindingResource::TextureView(&palette_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.weather_tiles),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(
                            engine_view.as_ref().unwrap_or(&palette_view),
                        ),
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
            if vertices.len() as u64 * 4 > buffer.size() {
                *buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Aircraft formation poses"),
                    size: (vertices.len() as u64 * 4).next_power_of_two(),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            if !vertices.is_empty() {
                queue.write_buffer(buffer, 0, &bytes(vertices));
            }
            *count = (vertices.len() / 10) as u32;
            self.canopy_visible = vertices.chunks_exact(10).any(|v| v[5] == -5.);
        }
    }
    pub fn update_aircraft_vertices(&mut self, queue: &wgpu::Queue, vertices: &[f32]) {
        self.aircraft_visible = true;
        assert!(vertices.len() * 4 <= 2 * 1024 * 1024);
        if let Some((_, buffer, count)) = &mut self.aircraft {
            if !vertices.is_empty() {
                queue.write_buffer(buffer, 0, &bytes(vertices));
            }
            *count = (vertices.len() / 10) as u32;
            self.canopy_visible = vertices.chunks_exact(10).any(|v| v[5] == -5.);
        }
    }
    pub fn hide_aircraft(&mut self) {
        self.canopy_visible = false;
        self.aircraft_visible = false;
    }
    /// Apply new graphics choices. A different sample count rebuilds every
    /// world pipeline; the caller clamps it to what the adapter supports.
    pub fn set_graphics(
        &mut self,
        device: &wgpu::Device,
        options: crate::graphics::Options,
        samples: u32,
    ) {
        self.options = options;
        if samples == self.samples {
            return;
        }
        self.samples = samples;
        self.p = Pipelines::new(
            device,
            &self.shader,
            self.format,
            samples,
            &self.material_layout,
            &self.lighting.layout,
        );
        self.vapor_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Wing vapor bindings"),
            layout: &self.p.vapor_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &self.palette.create_view(&Default::default()),
                    ),
                },
            ],
        });
        self.smoke
            .set_samples(device, self.format, &self.shader, samples);
        self.targets.clear();
    }
    fn resample_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Render scale resample"),
            source: wgpu::ShaderSource::Wgsl(include_str!("resample.wgsl").into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render scale resample"),
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
        })
    }
    /// The cached attachments for an output size, created on first use.
    fn targets(&mut self, device: &wgpu::Device, output: [u32; 2]) -> usize {
        let scale = self.options.scale();
        let size = output.map(|v| ((v as f32 * scale).round() as u32).clamp(1, 8192));
        if let Some(i) = self
            .targets
            .iter()
            .position(|t| t.output == output && t.size == size && t.samples == self.samples)
        {
            return i;
        }
        let texture = |label, samples, format, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let scaled = (size != output).then(|| {
            let view = texture(
                "Render scale image",
                1,
                self.format,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Render scale resample"),
                layout: &self.resample.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            (view, bind)
        });
        let targets = Targets {
            output,
            size,
            samples: self.samples,
            depth: texture(
                "Simulation depth",
                self.samples,
                wgpu::TextureFormat::Depth32Float,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
            color: (self.samples > 1).then(|| {
                texture(
                    "Multisampled world",
                    self.samples,
                    self.format,
                    wgpu::TextureUsages::RENDER_ATTACHMENT,
                )
            }),
            scaled,
        };
        // Main view, mirrors and up to three instrument previews.
        if self.targets.len() >= 5 {
            self.targets.remove(0);
        }
        self.targets.push(targets);
        self.targets.len() - 1
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
        // The world renders at the render-scale size; the lens flare and the
        // resample work at the output size.
        let output = size;
        let slot = self.targets(device, output);
        let size = self.targets[slot].size;
        let weather = world.sample_view(f64::from(camera.position[1]), camera.weather_slot);
        // The recovered haze color the visibility ramp blends toward.
        let sky = weather.haze;
        let mut uniform = camera.uniform(
            size[0] as f32 / (size[1] as f32 * camera.view_fraction),
            weather.fog,
            sky,
        );
        uniform[7] = (world.texture_indices.len() / (256 * 256)) as f32;
        uniform[15] = weather.fog_palette.len() as f32;
        uniform.extend(weather.decks.into_iter().flatten());
        let mut celestial_count = 0;
        if let Some(celestial) = &world.celestial {
            let data = celestial.vertices(world, camera, size[1]);
            assert!(data.len() * 4 <= 256 * 1024);
            celestial_count = (data.len() / 10) as u32;
            if !data.is_empty() {
                queue.write_buffer(&self.celestial_vertices, 0, &bytes(&data));
            }
            uniform[27] = celestial.sun_remap as f32;
            if let Some(layer) = world.weather.sample(camera.position[1] as f64) {
                let shade = world.weather.configuration().shade_remap(layer.shade);
                uniform[31] = celestial.shade_rows[&shade.color] as f32;
            }
            uniform.extend(celestial.sun_uniform(world, camera.position[1]));
        } else {
            uniform.extend([0.; 36]);
        }
        // Band rows refer to the imported remap atlas; absent assets disable them.
        uniform.extend([0.; 4]);
        if let Some(celestial) = &world.celestial {
            let bands = if world.smooth_weather {
                weather.visual_bands.as_slice()
            } else {
                world.weather.active()
            };
            uniform[68] = bands.len() as f32;
            for layer in bands {
                let shade = world.weather.configuration().shade_remap(layer.shade);
                uniform.extend([
                    layer.low_feet as f32,
                    layer.high_feet as f32,
                    celestial.shade_rows[&shade.color] as f32,
                    shade.levels.len() as f32,
                    layer.fog_near as f32,
                    layer.fog_near_density as f32,
                    layer.fog_far as f32,
                    layer.fog_far_density as f32,
                ]);
            }
        }
        uniform.resize(328, 0.);
        if let Some(layer) = world.weather.sample(camera.position[1] as f64) {
            let horizon =
                tore_sim::environment::horizon::Horizon::new(&layer, camera.position[1] as f64);
            uniform[70] = horizon.lower_extent as f32;
            uniform[71] = (u16::from(horizon.flags())
                | ((layer.flags & 0x40) >> 4)
                | (u16::from(world.smooth_weather) << 3)) as f32;
            let roll = (camera.roll.rem_euclid(std::f32::consts::TAU) * 65536.
                / std::f32::consts::TAU)
                .round() as i32 as i16;
            uniform[19] =
                2. * f32::from(tore_sim::environment::horizon::lower_solid_offset(
                    size, roll,
                )) / 32767.;
        }
        let ocean = world.weather.sample(camera.position[1] as f64);
        let ocean_decks = ocean.as_ref().map_or([false; 2], |layer| {
            std::array::from_fn(|i| layer.decks[i].name.starts_with("OCEAN"))
        });
        // Approximate angular width of a pixel, shared by the surface filtering.
        let pixel_angle = 2. / (1.732_050_8 * camera.zoom * size[1].max(1) as f32);
        uniform.extend(
            world
                .ocean_motion
                .uniform(world.weather.ticks(), ocean_decks, pixel_angle),
        );
        let dense = weather
            .visual_bands
            .iter()
            .find(|band| band.fog_far_density >= 256 && band.fog_far * 256 <= 8000);
        let mut reflection = match (&world.clouds, dense) {
            (Some(clouds), Some(band)) if world.smooth_weather => [
                clouds.reflection_texture() as f32,
                (band.low_feet as f32 + 250.).max(500.),
                world
                    .ocean_motion
                    .uniform(world.weather.ticks(), [true, false], pixel_angle)[1],
                0.,
            ],
            _ => [-1., 0., 0., 0.],
        };
        reflection[2] =
            world
                .ocean_motion
                .uniform(world.weather.ticks(), [true, false], pixel_angle)[1];
        reflection[3] = world.ocean_motion.environment_reflection;
        uniform.extend(reflection);
        uniform.extend([
            camera.near_clip,
            f32::from(camera.weather_slot == 4),
            0.,
            0.,
        ]);
        // Graphics options: spotting aid strength, terrain filtering, sun
        // glint; then the world image size in pixels, samples and scale.
        uniform.extend([
            self.options.spotting_aid.strength(),
            f32::from(self.options.terrain_filtering),
            f32::from(self.options.sun_glint),
            0.,
            size[0] as f32,
            size[1] as f32 * camera.view_fraction,
            self.samples as f32,
            self.options.scale(),
        ]);
        debug_assert_eq!(uniform.len() * 4, UNIFORM_BYTES as usize);
        queue.write_buffer(&self.uniform, 0, &bytes(&uniform));
        self.smoke.update(
            queue,
            camera,
            size[0] as f32 / (size[1] as f32 * camera.view_fraction),
        );
        let mut entries = Vec::with_capacity(11 * 1024);
        for row in std::iter::once(&weather.palette).chain(weather.fog_palette.iter()) {
            for rgb in row {
                entries.extend([rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        entries.resize(11 * 1024, 0);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.palette,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &entries,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1024),
                rows_per_image: Some(11),
            },
            wgpu::Extent3d {
                width: 256,
                height: 11,
                depth_or_array_layers: 1,
            },
        );
        let cloud_data = world
            .clouds
            .as_ref()
            .map_or_else(Vec::new, |clouds| clouds.vertices(camera));
        assert!(cloud_data.len() * 4 <= 256 * 1024);
        let cloud_count = (cloud_data.len() / 10) as u32;
        if cloud_count > 0 {
            queue.write_buffer(&self.cloud_vertices, 0, &bytes(&cloud_data));
        }
        let linear = |v: u8| ((v as f64 / 255.0 + 0.055) / 1.055).powf(2.4);
        let flare_target =
            self.lens_flare
                .prepare(device, queue, world, camera, output, &weather.palette);
        if self.lighting.prepare(queue, camera, world) {
            let mut objects = Vec::new();
            if let Some((buffer, count)) = &self.airports {
                objects.push((&self.bind, buffer, *count));
            }
            if let Some((bind, buffer, count)) = &self.aircraft {
                objects.push((bind, buffer, *count));
                if let Some((buffer, count)) = &self.battle {
                    objects.push((bind, buffer, *count));
                }
            }
            for (_, (bind, buffer, count)) in &self.dummies {
                objects.push((bind, buffer, *count));
            }
            self.lighting
                .draw(encoder, (&self.bind, &self.vertices, self.count), &objects);
        }
        let destination = flare_target.as_ref().unwrap_or(target);
        let targets = &self.targets[slot];
        // Multisampled color resolves into the render-scale image when there
        // is one, otherwise straight into the destination.
        let direct = targets
            .scaled
            .as_ref()
            .map_or(destination, |(view, _)| view);
        let (view, resolve_target) = match &targets.color {
            Some(color) => (color, Some(direct)),
            None => (direct, None),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Simulation world"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: linear(sky[0]),
                        g: linear(sky[1]),
                        b: linear(sky[2]),
                        a: if camera.weather_slot == 4 { 0. } else { 1. },
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
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
        pass.set_pipeline(&self.p.sky_pipeline);
        pass.set_bind_group(1, &self.lighting.bind, &[]);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..3, 0..1);
        if celestial_count > 0 {
            pass.set_pipeline(&self.p.celestial_pipeline);
            pass.set_vertex_buffer(0, self.celestial_vertices.slice(..));
            pass.draw(0..celestial_count, 0..1);
        }
        pass.set_pipeline(&self.p.terrain_pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.terrain_normals.slice(..));
        pass.draw(0..self.count, 0..1);
        if let Some((buffer, count)) = &self.airports {
            pass.set_pipeline(&self.p.airport_pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
            pass.set_pipeline(&self.p.airport_decal_pipeline);
            pass.draw(0..*count, 0..1);
        }
        pass.set_pipeline(&self.p.pipeline);
        if let Some((bind, vertices, count)) = &self.aircraft {
            pass.set_bind_group(0, bind, &[]);
            pass.set_vertex_buffer(0, vertices.slice(..));
            if self.aircraft_visible {
                pass.draw(0..*count, 0..1);
            }
            if let Some((buffer, count)) = &self.battle {
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, 0..1);
            }
        }
        for (_, (bind, vertices, count)) in &self.dummies {
            pass.set_pipeline(&self.p.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.draw(0..*count, 0..1);
            pass.set_pipeline(&self.p.canopy_depth_pipeline);
            pass.draw(0..*count, 0..1);
            pass.set_pipeline(&self.p.canopy_pipeline);
            pass.draw(0..*count, 0..1);
        }
        if self.canopy_visible
            && let Some((bind, vertices, count)) = &self.aircraft
        {
            pass.set_bind_group(0, bind, &[]);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.set_pipeline(&self.p.canopy_depth_pipeline);
            pass.draw(0..*count, 0..1);
            pass.set_pipeline(&self.p.canopy_pipeline);
            pass.draw(0..*count, 0..1);
        }
        if cloud_count > 0 {
            pass.set_pipeline(&self.p.cloud_pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, self.cloud_vertices.slice(..));
            pass.draw(0..cloud_count, 0..1);
        }
        // Blended and depth-tested but not depth-writing, so trails read behind
        // terrain and aircraft without occluding each other.
        if let Some((buffer, count)) = &self.vapor
            && *count > 0
        {
            pass.set_pipeline(&self.p.vapor_pipeline);
            pass.set_bind_group(0, &self.vapor_bind, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
        if let Some((buffer, count)) = &self.battle {
            pass.set_pipeline(&self.p.tracer_pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
        self.smoke.draw(&mut pass);
        drop(pass);
        if let Some((_, bind)) = &targets.scaled {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render scale resample"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: destination,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.resample);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
        if flare_target.is_some() {
            self.lens_flare.draw(encoder, target);
        }
    }
}

#[cfg(test)]
mod lighting_tests {
    use super::*;

    fn plane(y: f32, radius: f32) -> Vec<f32> {
        [
            (-1., -1.),
            (-1., 1.),
            (1., -1.),
            (1., -1.),
            (-1., 1.),
            (1., 1.),
        ]
        .into_iter()
        .flat_map(|(x, z)| [x * radius, y, z * radius, 0., 0., -1., 0.6, 0.6, 0.6, -1.])
        .collect()
    }

    // Runs the production shader, shadow maps, surface pipelines and readback.
    // Synthetic geometry only; explicitly invoked on a GPU-capable host.
    #[test]
    #[ignore = "requires a GPU adapter"]
    fn gpu_geometry_shadows_cross_terrain_and_objects() {
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
            let render = |minutes,
                          terrain_caster: bool,
                          caster: bool,
                          smooth: bool,
                          tilt: bool,
                          material: f32,
                          view_x: f32,
                          water: bool,
                          caster_height: f32,
                          elapsed_steps: usize| {
                let mut world = crate::terrain::tests::world();
                let mut module = tore_formats::weather::Module::parse(
                    &tore_formats::weather::synthetic_module(1),
                )
                .unwrap();
                for layer in &mut module.layers {
                    layer.start_seconds = 0;
                    layer.end_seconds = 86399;
                    layer.sunrise_seconds = 6 * 3600;
                    layer.sunset_seconds = 18 * 3600;
                    layer.sun_azimuth_morning = 16384;
                    layer.sun_azimuth_evening = -16384;
                    layer.fog_near_density = 0;
                    layer.fog_far_density = 0;
                }
                world.weather = tore_sim::environment::Environment::new(
                    tore_sim::environment::Configuration::new(
                        module,
                        minutes / 60,
                        minutes % 60,
                        0,
                        None,
                    )
                    .unwrap(),
                );
                for _ in 0..elapsed_steps {
                    world.weather.step();
                }
                world.smooth_weather = smooth;
                world.no_sun_whiteout = true; // Shadows must work with sunglare disabled.
                world.texture_indices = vec![255; 65536];
                world.sky_indices = vec![100; 65536];
                world.vertices = if terrain_caster {
                    plane(caster_height, 10.)
                } else {
                    plane(0., 120.)
                };
                if terrain_caster && !caster {
                    world.vertices.clear();
                }
                if tilt {
                    for vertex in world.vertices.chunks_exact_mut(10) {
                        let x = vertex[0];
                        vertex[0] = x * 0.5;
                        vertex[1] = -x * 3_f32.sqrt() * 0.5;
                    }
                }
                // A zero-sized vertex buffer is not portable; supply a degenerate triangle.
                if world.vertices.is_empty() {
                    world.vertices = vec![0.; 30];
                }
                let mut renderer = SimRenderer::new(
                    &device,
                    &queue,
                    wgpu::TextureFormat::Rgba8Unorm,
                    &world,
                    crate::graphics::Options::original(),
                    1,
                );
                if water {
                    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("Production water reflection probe"),
                        source: wgpu::ShaderSource::Wgsl(format!("{}\n{}\n{}", include_str!("surface_lighting.wgsl"), include_str!("terrain.wgsl"),
                            "@fragment fn reflection_probe(in:VertexOut)->@location(0) vec4<f32>{return vec4<f32>(water_sun(vec3<f32>(0.05),normalize(vec3<f32>(1.0,-1.0,0.0)),scene.eye.xyz+in.direction,1000.0),1.0);}").into()),
                    });
                    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: None,
                        bind_group_layouts: &[
                            &renderer.p.pipeline.get_bind_group_layout(0),
                            &renderer.lighting.layout,
                        ],
                        push_constant_ranges: &[],
                    });
                    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("Water receiver probe"), layout: Some(&layout),
                        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vertex"), compilation_options: Default::default(), buffers: &[wgpu::VertexBufferLayout { array_stride: 40, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32] }] },
                        fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("reflection_probe"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
                        primitive: Default::default(), depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias: Default::default() }), multisample: Default::default(), multiview: None, cache: None,
                    });
                    renderer.p.terrain_pipeline = pipeline.clone();
                    renderer.p.pipeline = pipeline;
                }
                let mut object = if terrain_caster {
                    plane(0., 120.)
                } else if caster {
                    plane(caster_height, 10.)
                } else {
                    vec![]
                };
                if !terrain_caster {
                    for vertex in object.chunks_exact_mut(10) {
                        // A vertical blocker exercises grazing sunlight at sunrise.
                        if minutes == 6 * 60 {
                            vertex[1] = vertex[0];
                            vertex[0] = 30.;
                        }
                        vertex[5] = material;
                    }
                }
                if !object.is_empty() {
                    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Synthetic opaque object"),
                        contents: &bytes(&object),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    renderer.aircraft =
                        Some((renderer.bind.clone(), buffer, (object.len() / 10) as u32));
                }
                if !terrain_caster {
                    renderer.hide_aircraft();
                }
                let mut camera = Camera::new();
                camera.position = [0., 200., 0.];
                camera.pitch = -std::f32::consts::FRAC_PI_2;
                camera.yaw = 0.;
                if view_x != 0. {
                    camera.position[0] = view_x;
                    camera.yaw = -view_x.signum() * std::f32::consts::FRAC_PI_2;
                    camera.pitch = -200_f32.atan2(view_x.abs());
                }
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Synthetic lighting readback"),
                    size: wgpu::Extent3d {
                        width: 256,
                        height: 256,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 256 * 256 * 4,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut encoder = device.create_command_encoder(&Default::default());
                renderer.draw(
                    &device,
                    &queue,
                    &mut encoder,
                    &texture.create_view(&Default::default()),
                    [256, 256],
                    &camera,
                    &world,
                );
                if water {
                    // Fixed clear-white source and coarse footprint isolate the actual
                    // water_sun consumer from imported art and procedural ripple detail.
                    let axis = 0.5_f32.sqrt();
                    queue.write_buffer(
                        &renderer.uniform,
                        128,
                        &bytes(&[axis, axis, 0., 1., 0.1, 100., 1., 0.]),
                    );
                    queue.write_buffer(&renderer.uniform, 1324, &bytes(&[1.]));
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &renderer.palette,
                            mip_level: 0,
                            origin: wgpu::Origin3d { x: 100, y: 0, z: 0 },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &[255; 4],
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4),
                            rows_per_image: Some(1),
                        },
                        wgpu::Extent3d {
                            width: 1,
                            height: 1,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(1024),
                            rows_per_image: Some(256),
                        },
                    },
                    wgpu::Extent3d {
                        width: 256,
                        height: 256,
                        depth_or_array_layers: 1,
                    },
                );
                queue.submit([encoder.finish()]);
                let (tx, rx) = std::sync::mpsc::channel();
                buffer
                    .slice(..)
                    .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
                device
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: None,
                    })
                    .unwrap();
                rx.recv().unwrap().unwrap();
                let pixels = buffer.slice(..).get_mapped_range().to_vec();
                buffer.unmap();
                pixels
            };
            let mut previous: Option<Vec<u8>> = None;
            for frame in 0..11 {
                let image = render(
                    6 * 60 + 1,
                    false,
                    false,
                    true,
                    true,
                    -1.,
                    frame as f32 * 0.02,
                    false,
                    30.,
                    frame * 60,
                );
                if let Some(prior) = previous {
                    for y in 112..144 {
                        for x in 112..144 {
                            for c in 0..3 {
                                let at = (y * 256 + x) * 4 + c;
                                assert!(
                                    (i32::from(image[at]) - i32::from(prior[at])).abs() <= 2,
                                    "sloped receiver must not flicker across five seconds, frame={frame} at={x},{y}"
                                );
                            }
                        }
                    }
                }
                previous = Some(image);
            }
            let near = render(12 * 60, false, true, true, false, -1., 0., false, 2., 0);
            let far = render(12 * 60, false, true, true, false, -1., 0., false, 500., 0);
            let clear = render(12 * 60, false, false, true, false, -1., 0., false, 2., 0);
            let dark = near[(128 * 256 + 128) * 4];
            let soft_pixels = |pixels: &[u8]| {
                (90..166)
                    .filter(|x| {
                        let at = (128 * 256 + x) * 4;
                        pixels[at] > dark + 4 && pixels[at] + 4 < clear[at]
                    })
                    .count()
            };
            assert!(
                soft_pixels(&far) >= soft_pixels(&near) + 4,
                "distant caster must have a wider penumbra: near={} far={}",
                soft_pixels(&near),
                soft_pixels(&far)
            );
            for terrain_caster in [false, true] {
                for (hour, x) in [(9, 95), (15, 160)] {
                    let clear = render(
                        hour * 60,
                        terrain_caster,
                        false,
                        true,
                        false,
                        -1.,
                        0.,
                        false,
                        30.,
                        0,
                    );
                    let shadow = render(
                        hour * 60,
                        terrain_caster,
                        true,
                        true,
                        false,
                        -1.,
                        0.,
                        false,
                        30.,
                        0,
                    );
                    let at = (128 * 256 + x) * 4;
                    assert!(
                        u16::from(shadow[at]) * 2 < u16::from(clear[at]),
                        "caster terrain={terrain_caster}, hour={hour}: shadow={} clear={}",
                        shadow[at],
                        clear[at]
                    );
                    let away = (128 * 256 + (255 - x)) * 4;
                    assert!(
                        (i32::from(shadow[away]) - i32::from(clear[away])).abs() <= 2,
                        "opposite side must stay lit"
                    );
                    let stepped_clear = render(
                        hour * 60,
                        terrain_caster,
                        false,
                        false,
                        false,
                        -1.,
                        0.,
                        false,
                        30.,
                        0,
                    );
                    let stepped_shadow = render(
                        hour * 60,
                        terrain_caster,
                        true,
                        false,
                        false,
                        -1.,
                        0.,
                        false,
                        30.,
                        0,
                    );
                    assert_eq!(
                        stepped_clear[at], stepped_shadow[at],
                        "compatibility has no geometric shadow"
                    );
                }
            }
            for terrain_caster in [false, true] {
                let unblocked = render(
                    9 * 60,
                    terrain_caster,
                    false,
                    true,
                    false,
                    -1.,
                    0.,
                    true,
                    30.,
                    0,
                );
                let blocked = render(
                    9 * 60,
                    terrain_caster,
                    true,
                    true,
                    false,
                    -1.,
                    0.,
                    true,
                    30.,
                    0,
                );
                let at = (128 * 256 + 95) * 4;
                assert!(unblocked[at] > 80, "clear water must have a visible glint");
                assert!(
                    (i32::from(blocked[at]) - 13).abs() <= 1,
                    "blocked glint must return the 0.05 water base, got {}",
                    blocked[at]
                );
                let away = (128 * 256 + 160) * 4;
                assert_eq!(
                    unblocked[away], blocked[away],
                    "unblocked water must keep its reflection"
                );
            }
            let clear = render(9 * 60, false, false, true, false, -1., 0., false, 30., 0);
            for material in [-2., -5., -6., -7., -8.] {
                let transparent = render(
                    9 * 60,
                    false,
                    true,
                    true,
                    false,
                    material,
                    0.,
                    false,
                    30.,
                    0,
                );
                let at = (128 * 256 + 95) * 4;
                assert_eq!(
                    transparent[at], clear[at],
                    "cutouts, glass and emissive effects must not cast a solid shadow: {material}"
                );
            }
            let at = (128 * 256 + 128) * 4;
            let horizon_clear = render(6 * 60, false, false, true, true, -1., 0., false, 30., 0);
            let horizon_shadow = render(6 * 60, false, true, true, true, -1., 0., false, 30., 0);
            assert!(
                horizon_shadow[at] + 15 < horizon_clear[at],
                "the half-visible sun must still cast a panel shadow: {} vs {}",
                horizon_shadow[at],
                horizon_clear[at]
            );
            let toward_highlight =
                render(9 * 60, true, false, true, false, -1., -200., false, 30., 0);
            let away_highlight = render(9 * 60, true, false, true, false, -1., 200., false, 30., 0);
            assert!(
                toward_highlight[at] > away_highlight[at] + 12,
                "painted panels must respond to view direction: {} vs {}",
                toward_highlight[at],
                away_highlight[at]
            );
            let terrain_a = render(9 * 60, false, false, true, false, -1., -200., false, 30., 0);
            let terrain_b = render(9 * 60, false, false, true, false, -1., 200., false, 30., 0);
            assert!(
                (i32::from(terrain_a[at]) - i32::from(terrain_b[at])).abs() <= 2,
                "terrain must not acquire a painted-panel highlight"
            );
            let morning = render(7 * 60, false, false, true, true, -1., 0., false, 30., 0);
            let shaded_land = render(17 * 60, false, false, true, true, -1., 0., false, 30., 0);
            let noon_land = render(12 * 60, false, false, true, true, -1., 0., false, 30., 0);
            let night_land = render(22 * 60, false, false, true, true, -1., 0., false, 30., 0);
            let pixel = (128 * 256 + 128) * 4;
            assert!(
                shaded_land[pixel] <= 15,
                "low-sun back slope must darken from its preceding 20-level fill, got {}",
                shaded_land[pixel]
            );
            // Reference values from the unchanged response on this synthetic grey slope.
            for (image, expected) in [(&morning, 90), (&noon_land, 56), (&night_land, 24)] {
                assert!(
                    (i32::from(image[pixel]) - expected).abs() <= 2,
                    "preserve exposed/day/night land brightness: got {} expected {expected}",
                    image[pixel]
                );
            }
            let before = render(7 * 60 + 1, false, false, true, true, -1., 0., false, 30., 0);
            let at = (128 * 256 + 128) * 4;
            assert!(
                morning[at] > morning[at + 2] + 10,
                "sun-facing grey surface must receive warm morning light: {:?}",
                &morning[at..at + 3]
            );
            for channel in 0..3 {
                assert!(
                    (i32::from(morning[at + channel]) - i32::from(before[at + channel])).abs() <= 2,
                    "a minute of sun movement must not step brightness"
                );
            }
        });
    }
}
