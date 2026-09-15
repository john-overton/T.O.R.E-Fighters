//! Extensible 3D pass. World data/camera are independent of wgpu; UI composites afterward.
use crate::terrain::{Camera, World};
use wgpu::util::DeviceExt;
fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
pub struct SimRenderer {
    battle: Option<(wgpu::Buffer, u32)>,
    vapor: Option<(wgpu::Buffer, u32)>,
    vapor_pipeline: wgpu::RenderPipeline,
    vapor_bind: wgpu::BindGroup,
    palette: wgpu::Texture,
    weather_tiles: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    aircraft: Option<(wgpu::BindGroup, wgpu::Buffer, u32)>,
    bind: wgpu::BindGroup,
    sky_pipeline: wgpu::RenderPipeline,
    celestial_pipeline: wgpu::RenderPipeline,
    celestial_vertices: wgpu::Buffer,
    cloud_pipeline: wgpu::RenderPipeline,
    cloud_vertices: wgpu::Buffer,
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
                    array_stride: 40,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32],
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
        let celestial_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Original celestial primitives"), layout: Some(&sky_layout),
            vertex: wgpu::VertexState { module:&shader,entry_point:Some("celestial_vertex"),compilation_options:Default::default(),
                buffers:&[wgpu::VertexBufferLayout {array_stride:40,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32]}]},
            fragment:Some(wgpu::FragmentState {module:&shader,entry_point:Some("celestial_fragment"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState {format,blend:Some(wgpu::BlendState::ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})]}),
            primitive:Default::default(),depth_stencil:Some(wgpu::DepthStencilState {format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:false,depth_compare:wgpu::CompareFunction::Always,stencil:Default::default(),bias:Default::default()}),multisample:Default::default(),multiview:None,cache:None,
        });
        let celestial_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Celestial vertices"),
            size: 256 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cloud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("Original cloud sheets"),layout:Some(&sky_layout),
            vertex:wgpu::VertexState {module:&shader,entry_point:Some("vertex"),compilation_options:Default::default(),buffers:&[wgpu::VertexBufferLayout {array_stride:40,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32]}]},
            fragment:Some(wgpu::FragmentState {module:&shader,entry_point:Some("cloud_fragment"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState {format,blend:None,write_mask:wgpu::ColorWrites::ALL})]}),
            primitive:wgpu::PrimitiveState {cull_mode:None,..Default::default()},depth_stencil:Some(wgpu::DepthStencilState {format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:true,depth_compare:wgpu::CompareFunction::Less,stencil:Default::default(),bias:Default::default()}),multisample:Default::default(),multiview:None,cache:None,
        });
        let cloud_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cloud vertices"),
            size: 256 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Wing vapor is five one-pixel line segments per side, exactly as
        // `_DrawStreamer@12` draws them, so it needs its own blended pipeline.
        let vapor_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Wing vapor"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vapor_vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 28,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
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
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera and atmosphere"),
            size: 1312,
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
                    resource: wgpu::BindingResource::TextureView(&palette_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        // The vapor pipeline only reads the camera uniform, so its derived
        // layout differs from the terrain pipeline's and needs its own group.
        let vapor_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Wing vapor bindings"),
            layout: &vapor_pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Retail T2 terrain mesh"),
            contents: &bytes(&world.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Self {
            battle: None,
            vapor: None,
            vapor_pipeline,
            vapor_bind,
            palette,
            weather_tiles: view,
            pipeline,
            aircraft: None,
            sky_pipeline,
            celestial_pipeline,
            celestial_vertices,
            cloud_pipeline,
            cloud_vertices,
            bind,
            uniform,
            vertices,
            count: (world.vertices.len() / 10) as u32,
            spare_depth: None,
            depth: Self::depth(device, width, height),
            size: [width, height],
        }
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

    pub fn clear_aircraft(&mut self) {
        self.aircraft = None;
    }
    pub fn aircraft(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hornet: &crate::aircraft::Airframe,
        vertices: &[f32],
    ) {
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
                        resource: wgpu::BindingResource::TextureView(&palette_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.weather_tiles),
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
            *count = (vertices.len() / 10) as u32;
        }
    }
    pub fn update_aircraft_vertices(&mut self, queue: &wgpu::Queue, vertices: &[f32]) {
        assert!(vertices.len() * 4 <= 2 * 1024 * 1024);
        if let Some((_, buffer, count)) = &mut self.aircraft {
            if !vertices.is_empty() {
                queue.write_buffer(buffer, 0, &bytes(vertices));
            }
            *count = (vertices.len() / 10) as u32;
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
        // The recovered haze color the visibility ramp blends toward.
        let sky = world.haze;
        let mut uniform = camera.uniform(
            size[0] as f32 / (size[1] as f32 * camera.view_fraction),
            world.fog,
            sky,
        );
        uniform[7] = (world.texture_indices.len() / (256 * 256)) as f32;
        uniform[15] = world.fog_palette.len() as f32;
        uniform.extend(world.decks.into_iter().flatten());
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
                uniform[19] = world
                    .palette
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, rgb)| {
                        rgb.iter()
                            .zip(world.haze)
                            .map(|(a, b)| (*a as i32 - b as i32).abs())
                            .sum::<i32>()
                    })
                    .unwrap()
                    .0 as f32;
            }
            uniform.extend(celestial.sun_uniform(world, camera.position[1]));
        } else {
            uniform.extend([0.; 36]);
        }
        // Band rows refer to the imported remap atlas; absent assets disable them.
        uniform.extend([0.; 4]);
        if let Some(celestial) = &world.celestial {
            uniform[68] = world.weather.active().len() as f32;
            for layer in world.weather.active() {
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
            uniform[71] = horizon.flags() as f32;
        }
        queue.write_buffer(&self.uniform, 0, &bytes(&uniform));
        let mut entries = Vec::with_capacity(11 * 1024);
        for row in std::iter::once(&world.palette).chain(world.fog_palette.iter()) {
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
        if celestial_count > 0 {
            pass.set_pipeline(&self.celestial_pipeline);
            pass.set_vertex_buffer(0, self.celestial_vertices.slice(..));
            pass.draw(0..celestial_count, 0..1);
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..self.count, 0..1);
        if let Some((bind, vertices, count)) = &self.aircraft {
            pass.set_bind_group(0, bind, &[]);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.draw(0..*count, 0..1);
            if let Some((buffer, count)) = &self.battle {
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, 0..1);
            }
        }
        if cloud_count > 0 {
            pass.set_pipeline(&self.cloud_pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, self.cloud_vertices.slice(..));
            pass.draw(0..cloud_count, 0..1);
        }
        // Blended and depth-tested but not depth-writing, so trails read behind
        // terrain and aircraft without occluding each other.
        if let Some((buffer, count)) = &self.vapor
            && *count > 0
        {
            pass.set_pipeline(&self.vapor_pipeline);
            pass.set_bind_group(0, &self.vapor_bind, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
    }
}
