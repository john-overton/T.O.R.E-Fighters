//! Cached vegetation impostors and the unchanged grass layer. Source shapes
//! are rasterized once into indexed atlas pages. GPU culling compacts all
//! visible species into one instance stream and one indirect draw. Not collidable.

use crate::vegetation::{self, Inst, Patch, Placement, Species};
use crate::vegetation_shapes::{FLOATS, Library, Look};
use crate::{camera::Camera, terrain::Terrain};
use std::sync::Arc;
use wgpu::util::DeviceExt;

const INST_STRIDE: u64 = 64;
const DRAWN_STRIDE: u64 = 80;
const PATCH_STRIDE: u64 = 64;
/// Grass distance rings: outer edge as a share of the grass radius, and
/// the share of a patch's full tufts drawn in it.
const RINGS: [(f32, f32); 4] = [(0.14, 1.), (0.32, 0.5), (0.6, 0.25), (1.0, 0.125)];

pub struct Renderer {
    placement: Placement,
    library: Arc<Library>,
    vertices: wgpu::Buffer,
    instances: wgpu::Buffer,
    drawn: wgpu::Buffer,
    shadow_drawn: wgpu::Buffer,
    counts: wgpu::Buffer,
    args: wgpu::Buffer,
    params: wgpu::Buffer,
    meshes: wgpu::Buffer,
    patches: wgpu::Buffer,
    bind: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
    /// Objects the instance buffers hold: the density's budget.
    capacity: usize,
    cull: wgpu::ComputePipeline,
    finish: wgpu::ComputePipeline,
    pipeline: wgpu::RenderPipeline,
    grass_pipeline: wgpu::RenderPipeline,
    /// Per mesh: first slot and slot count in the instance list.
    slots: Vec<(u32, u32)>,
    count: u32,
    uploaded: u64,
    since_upload: u32,
    /// Per ring: patch range and tufts per patch.
    rings: Vec<(std::ops::Range<u32>, u32)>,
    prepared: bool,
    suppressed: bool,
    settle: bool,
    draw_nm: f32,
    detail_nm: f32,
    density: f32,
    grass_radius: f32,
    tufts: u32,
}

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        samples: u32,
        material: &wgpu::BindGroupLayout,
        lighting: &crate::surface_lighting::SurfaceLighting,
        atlas: Arc<vegetation::Atlas>,
        library: Arc<Library>,
    ) -> Self {
        let mesh_count = library.meshes.len();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vegetation meshes"),
            contents: &bytes_of_f32(&library.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let buffer = |label, size: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let capacity = object_budget(crate::graphics::VegDensity::High);
        let counts = buffer(
            "Vegetation counts",
            (mesh_count as u64 * 2).max(1) * 4,
            storage,
        );
        let mut args = vec![0u32; mesh_count * 8];
        for (m, mesh) in library.meshes.iter().enumerate() {
            for list in 0..2 {
                let at = (list * mesh_count + m) * 4;
                args[at] = mesh.range.end - mesh.range.start;
                args[at + 2] = mesh.range.start;
            }
        }
        let args = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vegetation draws"),
            contents: &bytes_of_u32(&args),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST,
        });
        let params = buffer(
            "Vegetation cull params",
            128,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let meshes = buffer(
            "Vegetation mesh table",
            (mesh_count as u64).max(1) * 32,
            storage,
        );
        let patches = buffer(
            "Grass patches",
            vegetation::MAX_PATCHES as u64 * PATCH_STRIDE,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Vegetation cull layout"),
            entries: &[
                storage_entry(0, true),
                uniform_entry(1),
                storage_entry(2, false),
                storage_entry(3, false),
                storage_entry(4, false),
                storage_entry(5, false),
                storage_entry(6, true),
            ],
        });
        let (instances, drawn, shadow_drawn) = object_buffers(device, capacity);
        let bind = cull_bind(
            device,
            &layout,
            [
                &instances,
                &params,
                &counts,
                &drawn,
                &shadow_drawn,
                &args,
                &meshes,
            ],
        );
        let cull_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Vegetation cull"),
            source: wgpu::ShaderSource::Wgsl(include_str!("vegetation_cull.wgsl").into()),
        });
        let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Vegetation cull pipelines"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let compute = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Vegetation cull"),
                layout: Some(&compute_layout),
                module: &cull_shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let cull = compute("cull");
        let finish = compute("finish");
        let (pipeline, grass_pipeline) =
            render_pipelines(device, shader, format, samples, material, &lighting.layout);
        Self {
            placement: Placement::new(atlas),
            library,
            vertices,
            instances,
            drawn,
            shadow_drawn,
            counts,
            args,
            params,
            meshes,
            patches,
            bind,
            layout,
            capacity,
            cull,
            finish,
            pipeline,
            grass_pipeline,
            slots: vec![(0, 0); mesh_count],
            count: 0,
            uploaded: u64::MAX,
            since_upload: 0,
            rings: Vec::new(),
            prepared: false,
            suppressed: false,
            settle: false,
            draw_nm: 0.,
            detail_nm: 0.,
            density: 1.,
            grass_radius: 0.,
            tufts: 0,
        }
    }

    pub fn set_options(&mut self, device: &wgpu::Device, options: &crate::graphics::Options) {
        self.draw_nm = options.vegetation_distance_nm as f32;
        self.detail_nm = (options.vegetation_detail_nm as f32).min(self.draw_nm);
        let density = options.vegetation_density;
        self.density = density.scale();
        self.placement.set_density(self.density);
        let capacity = object_budget(density);
        if capacity != self.capacity {
            self.capacity = capacity;
            (self.instances, self.drawn, self.shadow_drawn) = object_buffers(device, capacity);
            self.bind = cull_bind(
                device,
                &self.layout,
                [
                    &self.instances,
                    &self.params,
                    &self.counts,
                    &self.drawn,
                    &self.shadow_drawn,
                    &self.args,
                    &self.meshes,
                ],
            );
            self.count = 0;
            self.uploaded = u64::MAX;
        }
        self.placement.set_budget(capacity);
        use crate::graphics::VegDensity;
        (self.grass_radius, self.tufts) = match density {
            VegDensity::Low => (700., 48),
            VegDensity::Medium => (1_100., 96),
            VegDensity::High => (1_500., 160),
            VegDensity::Ultra => (2_200., 256),
        };
        self.grass_radius = self
            .grass_radius
            .min(self.detail_nm * vegetation::NM as f32 * 0.5);
    }

    /// The before picture: objects and grass are not drawn. Ground detail
    /// is the graphics option's own switch.
    pub fn set_suppressed(&mut self, suppressed: bool) {
        self.suppressed = suppressed;
    }

    /// Build every chunk the view needs before drawing (previews and
    /// captures), instead of a few per frame.
    pub fn set_settle(&mut self, settle: bool) {
        self.settle = settle;
    }

    /// Objects placed around the camera.
    pub fn placed(&self) -> u32 {
        self.count
    }

    /// Feet to the farthest chunk of objects within the instance budget.
    pub fn kept_reach(&self, eye: [f64; 2]) -> f32 {
        self.placement.kept_reach(eye)
    }

    /// How far full density reaches, as a share of the setting's distance.
    pub fn squeeze(&self) -> f32 {
        self.placement.squeezed(1.)
    }

    /// Grass tufts drawn last frame, before the shader drops any.
    pub fn tufts_drawn(&self) -> u64 {
        self.rings
            .iter()
            .map(|(r, n)| u64::from(r.end - r.start) * u64::from(*n))
            .sum()
    }

    pub fn rebuild(
        &mut self,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        samples: u32,
        material: &wgpu::BindGroupLayout,
        lighting: &crate::surface_lighting::SurfaceLighting,
    ) {
        (self.pipeline, self.grass_pipeline) =
            render_pipelines(device, shader, format, samples, material, &lighting.layout);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        camera: &Camera,
        world: &Terrain,
        origin: [f64; 3],
        size: [u32; 2],
    ) {
        self.prepared = false;
        self.rings.clear();
        if self.suppressed || self.draw_nm <= 0. || camera.position[1] > 16_000. {
            return;
        }
        let eye2 = [camera.position[0], camera.position[2]];
        let full = self.detail_nm * vegetation::NM as f32 * 0.4;
        let draw = self.draw_nm * vegetation::NM as f32;
        let budget = if self.settle { None } else { Some(3) };
        let done = self.placement.update(world, eye2, full, draw, budget);
        // Full density reaches less far where the view would hold more
        // objects than the budget.
        let full = self.placement.squeezed(full);
        self.since_upload += 1;
        if self.placement.generation != self.uploaded && (done || self.since_upload >= 20) {
            self.upload(queue, eye2);
        }
        let (fx, rx, ux) = basis(camera);
        let height = (size[1] as f32 * camera.view_fraction).max(1.);
        let aspect = size[0] as f32 / height;
        let f = 1.732_050_8 * camera.zoom.max(0.01);
        let eye = [
            (camera.position[0] - origin[0]) as f32,
            (camera.position[1] - origin[1]) as f32,
            (camera.position[2] - origin[2]) as f32,
        ];
        self.prepare_grass(
            queue,
            world,
            camera,
            origin,
            eye,
            [fx, rx, ux],
            [aspect / f, 1. / f],
        );
        if self.count == 0 {
            self.prepared = !self.rings.is_empty();
            return;
        }
        let mesh_count = self.library.meshes.len();
        queue.write_buffer(&self.counts, 0, &vec![0u8; mesh_count * 8]);
        let mut params = Vec::with_capacity(32);
        params.extend([origin[0] as f32, origin[1] as f32, origin[2] as f32, 0.]);
        params.extend([eye[0], eye[1], eye[2], 0.]);
        params.extend([fx[0], fx[1], fx[2], 0.]);
        params.extend([rx[0], rx[1], rx[2], 0.]);
        params.extend([ux[0], ux[1], ux[2], 0.]);
        params.extend([
            aspect / f,
            1. / f,
            height * f * 0.5,
            camera.position[1] as f32,
        ]);
        params.extend([full, draw, 0., 1.5]);
        params.extend(u32s(&[self.count, mesh_count as u32, 0, 0]));
        queue.write_buffer(&self.params, 0, &bytes_of_f32(&params));
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Vegetation cull"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.cull);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.dispatch_workgroups(self.count.div_ceil(256), 1, 1);
        pass.set_pipeline(&self.finish);
        pass.dispatch_workgroups((mesh_count as u32).div_ceil(64), 1, 1);
        drop(pass);
        self.prepared = true;
    }

    /// Sorts the placed objects by mesh and uploads them with each mesh's
    /// slot range.
    fn upload(&mut self, queue: &wgpu::Queue, eye: [f64; 2]) {
        let list = self.placement.objects(eye);
        let lib = &self.library;
        let mesh_count = lib.meshes.len();
        let mesh_of: Vec<u16> = list
            .iter()
            .map(|i| lib.mesh_for(i.species, i.variant))
            .collect();
        let mut per = vec![0u32; mesh_count];
        for &m in &mesh_of {
            per[usize::from(m)] += 1;
        }
        let mut start = 0u32;
        for (m, n) in per.iter().enumerate() {
            self.slots[m] = (start, *n);
            start += n;
        }
        let mut fill: Vec<u32> = self.slots.iter().map(|s| s.0).collect();
        let mut bytes = vec![0u8; list.len() * INST_STRIDE as usize];
        for (i, &m) in list.iter().zip(&mesh_of) {
            let slot = fill[usize::from(m)] as usize;
            fill[usize::from(m)] += 1;
            let mesh = &lib.meshes[usize::from(m)];
            let record = pack(i, m, mesh.radius);
            bytes[slot * INST_STRIDE as usize..][..INST_STRIDE as usize]
                .copy_from_slice(&bytes_of_f32(&record));
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.instances, 0, &bytes);
        }
        let mut table = Vec::with_capacity(mesh_count * 8);
        for mesh in &lib.meshes {
            let look = match (mesh.look, mesh.recolour.is_some()) {
                (Look::Foliage, _) => 0.,
                (Look::Solid, false) => 1.,
                (Look::Solid, true) => 3.,
            };
            let mean = mesh.recolour.unwrap_or([1.; 3]);
            table.extend([
                f32::from_bits(0),
                look,
                mesh.radius,
                f32::from_bits(list.len() as u32),
            ]);
            table.extend([mean[0], mean[1], mean[2], mesh.sprite_layer as f32]);
        }
        if !table.is_empty() {
            queue.write_buffer(&self.meshes, 0, &bytes_of_f32(&table));
        }
        self.count = list.len() as u32;
        self.uploaded = self.placement.generation;
        self.since_upload = 0;
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_grass(
        &mut self,
        queue: &wgpu::Queue,
        world: &Terrain,
        camera: &Camera,
        origin: [f64; 3],
        eye: [f32; 3],
        basis: [[f32; 3]; 3],
        tan: [f32; 2],
    ) {
        let agl = camera.position[1] as f32
            - world.height(camera.position[0] as f32, camera.position[2] as f32);
        if self.tufts == 0 || self.grass_radius <= 0. || agl > 2_500. {
            return;
        }
        let budget = if self.settle { None } else { Some(2) };
        let radius = self.grass_radius;
        let (patches, _) = self.placement.grass(
            world,
            [camera.position[0], camera.position[2]],
            radius,
            budget,
        );
        let [fx, rx, ux] = basis;
        let mut rings: Vec<Vec<&Patch>> = vec![Vec::new(); RINGS.len()];
        for p in &patches {
            let rel = [
                p.pos[0] - origin[0] as f32 - eye[0],
                p.pos[1] - origin[1] as f32 - eye[1],
                p.pos[2] - origin[2] as f32 - eye[2],
            ];
            let dist = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt();
            let r = 34.;
            let z = dot(rel, fx);
            if z < -r || dot(rel, rx).abs() > z * tan[0] + r || dot(rel, ux).abs() > z * tan[1] + r
            {
                continue;
            }
            let share = dist / radius;
            if share >= 1. {
                continue;
            }
            let ring = RINGS
                .iter()
                .position(|(edge, _)| share < *edge)
                .unwrap_or(RINGS.len() - 1);
            rings[ring].push(p);
        }
        let mut bytes = Vec::new();
        let mut start = 0u32;
        for (ring, list) in rings.iter().enumerate() {
            let tufts = ((self.tufts as f32 * RINGS[ring].1).round() as u32).max(4);
            for p in list {
                let record = [
                    p.pos[0] - origin[0] as f32,
                    p.pos[1] - origin[1] as f32,
                    p.pos[2] - origin[2] as f32,
                    p.density,
                    p.grad[0],
                    p.grad[1],
                    p.kind,
                    p.height,
                    p.color[0],
                    p.color[1],
                    p.color[2],
                    p.hash,
                    tufts as f32,
                    self.tufts as f32,
                    radius,
                    0.,
                ];
                bytes.extend(bytes_of_f32(&record));
            }
            let end = start + list.len() as u32;
            if end > start {
                self.rings.push((start..end, tufts));
            }
            start = end;
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.patches, 0, &bytes);
        }
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, bind: &'a wgpu::BindGroup) {
        if !self.prepared {
            return;
        }
        pass.set_bind_group(0, bind, &[]);
        if self.count > 0 {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_vertex_buffer(1, self.drawn.slice(..));
            pass.draw_indirect(&self.args, 0);
        }
        if !self.rings.is_empty() {
            pass.set_pipeline(&self.grass_pipeline);
            pass.set_vertex_buffer(0, self.patches.slice(..));
            for (range, tufts) in &self.rings {
                pass.draw(0..tufts * 6, range.clone());
            }
        }
    }
}

/// Objects kept around the camera at most, per density.
fn object_budget(density: crate::graphics::VegDensity) -> usize {
    use crate::graphics::VegDensity;
    match density {
        VegDensity::Low => 65_536,
        VegDensity::Medium => 131_072,
        VegDensity::High => 262_144,
        VegDensity::Ultra => 524_288,
    }
}

/// The instance list and the two lists the cull pass writes.
fn object_buffers(
    device: &wgpu::Device,
    capacity: usize,
) -> (wgpu::Buffer, wgpu::Buffer, wgpu::Buffer) {
    let buffer = |label, stride: u64, usage| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: capacity as u64 * stride,
            usage,
            mapped_at_creation: false,
        })
    };
    let listed = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX;
    (
        buffer(
            "Vegetation instances",
            INST_STRIDE,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        ),
        buffer("Vegetation drawn", DRAWN_STRIDE, listed),
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Unused vegetation caster binding"),
            size: DRAWN_STRIDE,
            usage: listed,
            mapped_at_creation: false,
        }),
    )
}

fn cull_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffers: [&wgpu::Buffer; 7],
) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry<'_>> = buffers
        .iter()
        .enumerate()
        .map(|(binding, buffer)| wgpu::BindGroupEntry {
            binding: binding as u32,
            resource: buffer.as_entire_binding(),
        })
        .collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Vegetation cull"),
        layout,
        entries: &entries,
    })
}

fn basis(camera: &Camera) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let (sy, cy) = camera.yaw.sin_cos();
    let (sp, cp) = camera.pitch.sin_cos();
    let (sr, cr) = camera.roll.sin_cos();
    let right = [cy * cr - sy * sp * sr, cp * sr, -sy * cr - cy * sp * sr];
    let up = [-cy * sr - sy * sp * cr, cp * cr, sy * sr - cy * sp * cr];
    let forward = [sy * cp, sp, cy * cp];
    (forward, right, up)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// One instance record: position and yaw; height, width, ground slope;
/// tint and rank; mesh, distance reach, radius in feet, hash.
fn pack(i: &Inst, mesh: u16, radius: f32) -> [f32; 16] {
    let reach = Species::reach(i.species);
    [
        i.pos[0],
        i.pos[1],
        i.pos[2],
        i.yaw,
        i.height,
        i.width,
        i.grad[0],
        i.grad[1],
        i.tint[0],
        i.tint[1],
        i.tint[2],
        i.rank,
        f32::from(mesh),
        reach,
        radius * i.height * i.width,
        f32::from(i.variant) / 255.,
    ]
}

fn render_pipelines(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    samples: u32,
    material: &wgpu::BindGroupLayout,
    lighting: &wgpu::BindGroupLayout,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
    let constants = [("FILL_SAMPLES", f64::from(samples))];
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Vegetation layout"),
        bind_group_layouts: &[material, lighting],
        push_constant_ranges: &[],
    });
    let object_buffers = [
        wgpu::VertexBufferLayout {
            array_stride: (FLOATS * 4) as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32, 3 => Float32x3, 4 => Float32],
        },
        wgpu::VertexBufferLayout {
            array_stride: DRAWN_STRIDE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![
                5 => Float32x4,
                6 => Float32x4,
                7 => Float32x4,
                8 => Float32x4,
                9 => Float32x4
            ],
        },
    ];
    let grass_buffers = [wgpu::VertexBufferLayout {
        array_stride: PATCH_STRIDE,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
            8 => Float32x4
        ],
    }];
    let make = |label, vertex: &str, fragment: &str, buffers: &[wgpu::VertexBufferLayout]| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some(vertex),
                compilation_options: Default::default(),
                buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fragment),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &constants,
                    ..Default::default()
                },
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
    };
    (
        make("Vegetation", "veg_vertex", "veg_fragment", &object_buffers),
        make("Grass", "grass_vertex", "grass_fragment", &grass_buffers),
    )
}

/// The object shadow casters, one pipeline per cascade (the cascade is the
/// vertex entry, since the instance index is the object).
fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn u32s(v: &[u32]) -> [f32; 4] {
    std::array::from_fn(|i| f32::from_bits(v[i]))
}

fn bytes_of_f32(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for n in v {
        out.extend(n.to_le_bytes());
    }
    out
}

fn bytes_of_u32(v: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for n in v {
        out.extend(n.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    /// John saw the ground detail vanish with the afterburner lit: a
    /// dynamic light (flare, afterburner, fire, gun flash) swapped in an
    /// undetailed albedo. Both the shaded colour and the flare-lit albedo
    /// must carry the same detail tint and normal.
    #[test]
    fn a_dynamic_light_never_drops_the_ground_detail() {
        let shader = include_str!("terrain.wgsl");
        let start = shader
            .find("@fragment fn terrain_fragment")
            .expect("terrain fragment");
        let end = start + shader[start..].find("\n}\n").expect("end of function");
        let body = &shader[start..end];
        let calls: Vec<&str> = body
            .match_indices("surface_color(")
            .map(|(i, _)| {
                let rest = &body[i..];
                &rest[..rest.find(';').expect("statement end")]
            })
            .collect();
        assert_eq!(calls.len(), 2, "textured and untextured ground");
        for call in calls {
            assert_eq!(call.matches("detail.tint").count(), 2, "{call}");
            assert!(call.contains("detail.normal"), "{call}");
        }
    }
}
