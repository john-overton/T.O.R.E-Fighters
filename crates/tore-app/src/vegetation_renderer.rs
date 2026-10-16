//! Instanced vegetation. The CPU places a ring of objects around the camera
//! from [`crate::vegetation::Atlas`]; a compute pass culls them the way the
//! city fill does. Full meshes inside the detail distance, crossed cards
//! beyond. Not collidable.

use crate::vegetation::{self, Atlas, Inst};
use crate::{camera::Camera, terrain::Terrain};
use std::sync::Arc;
use wgpu::util::DeviceExt;

const SLOTS: usize = 10;
const MAX_PER: u32 = 12_288;
const DRAWN_STRIDE: u64 = 64;
const INST_STRIDE: u64 = 48;

pub struct Renderer {
    atlas: Arc<Atlas>,
    vertices: wgpu::Buffer,
    ranges: Vec<std::ops::Range<u32>>,
    instances: wgpu::Buffer,
    drawn: wgpu::Buffer,
    counts: wgpu::Buffer,
    args: wgpu::Buffer,
    params: wgpu::Buffer,
    bind: wgpu::BindGroup,
    cull: wgpu::ComputePipeline,
    finish: wgpu::ComputePipeline,
    pipeline: wgpu::RenderPipeline,
    cache: Option<Cache>,
    count: u32,
    prepared: bool,
    suppressed: bool,
    draw_nm: f32,
    detail_nm: f32,
    density: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Cache {
    cell: [i32; 2],
    band: u8,
    draw: u32,
    detail: u32,
    density: u32,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        samples: u32,
        material: &wgpu::BindGroupLayout,
        lighting: &wgpu::BindGroupLayout,
        atlas: Arc<Atlas>,
    ) -> Self {
        let (vertices, ranges) = meshes();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vegetation meshes"),
            contents: &bytes_of_f32(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vegetation instances"),
            size: vegetation::MAX_INSTANCES as u64 * INST_STRIDE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let drawn = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vegetation drawn"),
            size: SLOTS as u64 * u64::from(MAX_PER) * DRAWN_STRIDE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let counts = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vegetation counts"),
            size: SLOTS as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut args = vec![0u32; SLOTS * 4];
        for (slot, range) in ranges.iter().enumerate() {
            args[slot * 4] = range.end - range.start;
            args[slot * 4 + 2] = range.start;
        }
        let args = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vegetation draws"),
            contents: &bytes_of_u32(&args),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vegetation cull params"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Vegetation cull layout"),
            entries: &[
                storage(0, true),
                uniform(1),
                storage(2, false),
                storage(3, false),
                storage(4, false),
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Vegetation cull"),
            layout: &layout,
            entries: &[
                entry(0, &instances),
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params.as_entire_binding(),
                },
                entry(2, &counts),
                entry(3, &drawn),
                entry(4, &args),
            ],
        });
        let shader_cull = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Vegetation cull"),
            source: wgpu::ShaderSource::Wgsl(include_str!("vegetation_cull.wgsl").into()),
        });
        let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Vegetation cull pipelines"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let cull = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Vegetation cull"),
            layout: Some(&compute_layout),
            module: &shader_cull,
            entry_point: Some("cull"),
            compilation_options: Default::default(),
            cache: None,
        });
        let finish = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Vegetation cull finish"),
            layout: Some(&compute_layout),
            module: &shader_cull,
            entry_point: Some("finish"),
            compilation_options: Default::default(),
            cache: None,
        });
        let pipeline = render_pipeline(device, shader, format, samples, material, lighting);
        Self {
            atlas,
            vertices,
            ranges,
            instances,
            drawn,
            counts,
            args,
            params,
            bind,
            cull,
            finish,
            pipeline,
            cache: None,
            count: 0,
            prepared: false,
            suppressed: false,
            draw_nm: 0.,
            detail_nm: 0.,
            density: 1.,
        }
    }

    pub fn set_options(&mut self, options: &crate::graphics::Options) {
        let draw = options.vegetation_distance_nm;
        let detail = options.vegetation_detail_nm.min(draw);
        let density = (options.vegetation_density.scale() * 100.) as u32;
        if self
            .cache
            .as_ref()
            .is_some_and(|c| c.draw == draw && c.detail == detail && c.density == density)
        {
            self.draw_nm = draw as f32;
            self.detail_nm = detail as f32;
            self.density = options.vegetation_density.scale();
            return;
        }
        self.draw_nm = draw as f32;
        self.detail_nm = detail as f32;
        self.density = options.vegetation_density.scale();
        self.cache = None;
    }

    /// The before picture: objects are not drawn. Ground detail is the
    /// graphics option's own switch.
    pub fn set_suppressed(&mut self, suppressed: bool) {
        self.suppressed = suppressed;
    }

    pub fn placed(&self) -> u32 {
        self.count
    }

    pub fn rebuild(
        &mut self,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        samples: u32,
        material: &wgpu::BindGroupLayout,
        lighting: &wgpu::BindGroupLayout,
    ) {
        self.pipeline = render_pipeline(device, shader, format, samples, material, lighting);
    }

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
        if self.suppressed || self.draw_nm <= 0. {
            return;
        }
        let band = if camera.position[1] > 11_000. {
            2
        } else if camera.position[1] > 5_000. {
            1
        } else {
            0
        };
        let cell = [
            (camera.position[0] / 2_048.).floor() as i32,
            (camera.position[2] / 2_048.).floor() as i32,
        ];
        let key = Cache {
            cell,
            band,
            draw: self.draw_nm as u32,
            detail: self.detail_nm as u32,
            density: (self.density * 100.) as u32,
        };
        if self.cache.as_ref() != Some(&key) {
            let list = self.atlas.instances(
                world,
                [camera.position[0], camera.position[2]],
                camera.position[1],
                self.draw_nm,
                self.density,
            );
            self.count = list.len() as u32;
            if !list.is_empty() {
                queue.write_buffer(&self.instances, 0, &pack(&list));
            }
            self.cache = Some(key);
        }
        if self.count == 0 {
            return;
        }
        queue.write_buffer(&self.counts, 0, &[0u8; SLOTS * 4]);
        let (sy, cy) = camera.yaw.sin_cos();
        let (sp, cp) = camera.pitch.sin_cos();
        let (sr, cr) = camera.roll.sin_cos();
        let right = [cy * cr - sy * sp * sr, cp * sr, -sy * cr - cy * sp * sr];
        let up = [-cy * sr - sy * sp * cr, cp * cr, sy * sr - cy * sp * cr];
        let forward = [sy * cp, sp, cy * cp];
        let height = (size[1] as f32 * camera.view_fraction).max(1.);
        let aspect = size[0] as f32 / height;
        let f = 1.732_050_8 * camera.zoom.max(0.01);
        let eye = [
            (camera.position[0] - origin[0]) as f32,
            (camera.position[1] - origin[1]) as f32,
            (camera.position[2] - origin[2]) as f32,
        ];
        let draw = self.draw_nm * vegetation::NM as f32;
        let detail = self.detail_nm * vegetation::NM as f32;
        let mut params = Vec::with_capacity(32);
        params.extend([origin[0] as f32, origin[1] as f32, origin[2] as f32, 0.]);
        params.extend([eye[0], eye[1], eye[2], 0.]);
        params.extend([forward[0], forward[1], forward[2], 0.]);
        params.extend([right[0], right[1], right[2], 0.]);
        params.extend([up[0], up[1], up[2], 0.]);
        params.extend([detail, draw, draw * 0.15, 1.25]);
        params.extend([
            aspect / f,
            1. / f,
            height * f * 0.5,
            camera.position[1] as f32,
        ]);
        params.extend_from_slice(&u32s(&[
            self.count,
            SLOTS as u32,
            MAX_PER,
            (SLOTS - 1) as u32,
        ]));
        queue.write_buffer(&self.params, 0, &bytes_of_f32(&params));
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Vegetation cull"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.cull);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.dispatch_workgroups(self.count.div_ceil(256), 1, 1);
        pass.set_pipeline(&self.finish);
        pass.dispatch_workgroups(1, 1, 1);
        drop(pass);
        self.prepared = true;
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, bind: &'a wgpu::BindGroup) {
        if !self.prepared {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        for slot in 0..SLOTS {
            if self.ranges[slot].is_empty() {
                continue;
            }
            let start = slot as u64 * u64::from(MAX_PER) * DRAWN_STRIDE;
            pass.set_vertex_buffer(1, self.drawn.slice(start..));
            pass.draw_indirect(&self.args, slot as u64 * 16);
        }
    }
}

fn render_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    samples: u32,
    material: &wgpu::BindGroupLayout,
    lighting: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let constants = [("FILL_SAMPLES", f64::from(samples))];
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Vegetation layout"),
        bind_group_layouts: &[material, lighting],
        push_constant_ranges: &[],
    });
    let buffers = [
        wgpu::VertexBufferLayout {
            array_stride: 24,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        },
        wgpu::VertexBufferLayout {
            array_stride: DRAWN_STRIDE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![
                5 => Float32x4,
                6 => Float32x4,
                7 => Float32x4,
                8 => Float32x4
            ],
        },
    ];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Vegetation"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("veg_vertex"),
            compilation_options: Default::default(),
            buffers: &buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("veg_fragment"),
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
}

fn storage(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
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

fn uniform(binding: u32) -> wgpu::BindGroupLayoutEntry {
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

fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn pack(list: &[Inst]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(list.len() * INST_STRIDE as usize);
    for i in list {
        let kind = i.kind as f32 + if i.trunk > 0. { 16. } else { 0. };
        for v in [i.x, i.y, i.z, i.yaw, i.scale_xz, i.scale_y, i.radius, kind] {
            bytes.extend(v.to_le_bytes());
        }
        for v in [i.color[0], i.color[1], i.color[2], i.hash] {
            bytes.extend(v.to_le_bytes());
        }
    }
    bytes
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

/// Mesh slots: conifer, broadleaf, palm, bush, grass, rock, dead, dune,
/// hedge, impostor. Local y is 0 at the ground and 1 at the top. Horizontal
/// size is about 1, except the hedge (long and thin) and the dune (long).
fn meshes() -> (Vec<f32>, Vec<std::ops::Range<u32>>) {
    let mut v = Vec::new();
    let mut ranges = Vec::new();
    let push = |v: &mut Vec<f32>, extra: Vec<f32>| {
        let start = (v.len() / 6) as u32;
        v.extend(extra);
        start..(v.len() / 6) as u32
    };
    ranges.push(push(&mut v, conifer()));
    ranges.push(push(&mut v, broadleaf()));
    ranges.push(push(&mut v, palm()));
    ranges.push(push(&mut v, bush()));
    ranges.push(push(&mut v, grass()));
    ranges.push(push(&mut v, rock()));
    ranges.push(push(&mut v, dead()));
    ranges.push(push(&mut v, dune()));
    ranges.push(push(&mut v, hedge()));
    ranges.push(push(&mut v, impostor()));
    (v, ranges)
}

fn tri(out: &mut Vec<f32>, a: [f32; 3], b: [f32; 3], c: [f32; 3]) {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let mut n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-5);
    n = [n[0] / l, n[1] / l, n[2] / l];
    for p in [a, b, c] {
        out.extend([p[0], p[1], p[2], n[0], n[1], n[2]]);
    }
}

fn cone(out: &mut Vec<f32>, y0: f32, y1: f32, radius: f32, sides: usize) {
    for i in 0..sides {
        let a0 = i as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let p0 = [a0.cos() * radius, y0, a0.sin() * radius];
        let p1 = [a1.cos() * radius, y0, a1.sin() * radius];
        tri(out, [0., y1, 0.], p0, p1);
    }
}

fn prism(out: &mut Vec<f32>, hx: f32, y0: f32, y1: f32, hz: f32) {
    let p = |x: f32, y: f32, z: f32| [x, y, z];
    let a = p(-hx, y0, -hz);
    let b = p(hx, y0, -hz);
    let c = p(hx, y0, hz);
    let d = p(-hx, y0, hz);
    let e = p(-hx, y1, -hz);
    let f = p(hx, y1, -hz);
    let g = p(hx, y1, hz);
    let h = p(-hx, y1, hz);
    tri(out, a, b, f);
    tri(out, a, f, e);
    tri(out, b, c, g);
    tri(out, b, g, f);
    tri(out, c, d, h);
    tri(out, c, h, g);
    tri(out, d, a, e);
    tri(out, d, e, h);
    tri(out, e, f, g);
    tri(out, e, g, h);
}

fn conifer() -> Vec<f32> {
    let mut o = Vec::new();
    prism(&mut o, 0.08, 0., 0.28, 0.08);
    cone(&mut o, 0.22, 0.62, 0.85, 7);
    cone(&mut o, 0.48, 1.0, 0.55, 7);
    o
}

fn broadleaf() -> Vec<f32> {
    let mut o = Vec::new();
    prism(&mut o, 0.1, 0., 0.42, 0.1);
    // A faceted ball sitting on the trunk.
    let y = 0.68;
    let r = 0.95;
    for i in 0..6 {
        let a0 = i as f32 / 6. * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / 6. * std::f32::consts::TAU;
        let p = |a: f32, yy: f32, rad: f32| [a.cos() * rad, yy, a.sin() * rad];
        tri(&mut o, p(a0, y, r), p(a1, y, r), [0., y + 0.42, 0.]);
        tri(&mut o, p(a1, y, r), p(a0, y, r), [0., y - 0.38, 0.]);
    }
    o
}

fn palm() -> Vec<f32> {
    let mut o = Vec::new();
    prism(&mut o, 0.06, 0., 0.72, 0.06);
    for i in 0..6 {
        let a = i as f32 / 6. * std::f32::consts::TAU;
        let tip = [a.cos() * 1.15, 0.62, a.sin() * 1.15];
        let side = [a.sin() * 0.12, 0.78, -a.cos() * 0.12];
        tri(
            &mut o,
            [0., 0.78, 0.],
            [tip[0] + side[0], tip[1], tip[2] + side[2]],
            [tip[0] - side[0], tip[1] + 0.08, tip[2] - side[2]],
        );
    }
    o
}

fn bush() -> Vec<f32> {
    let mut o = Vec::new();
    for i in 0..5 {
        let a0 = i as f32 / 5. * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / 5. * std::f32::consts::TAU;
        let p = |a: f32| [a.cos(), 0.45, a.sin()];
        tri(&mut o, p(a0), p(a1), [0., 1., 0.]);
        tri(&mut o, p(a1), p(a0), [0., 0.05, 0.]);
    }
    o
}

fn grass() -> Vec<f32> {
    let mut o = Vec::new();
    for (x, z) in [(0.7, 0.15), (-0.2, 0.75), (-0.55, -0.45)] {
        tri(
            &mut o,
            [x, 0., z],
            [x + 0.18, 1., z + 0.05],
            [x - 0.05, 0., z + 0.2],
        );
    }
    o
}

fn rock() -> Vec<f32> {
    let mut o = Vec::new();
    prism(&mut o, 0.7, 0., 0.55, 0.5);
    prism(&mut o, 0.35, 0.4, 1.0, 0.28);
    o
}

fn dead() -> Vec<f32> {
    let mut o = Vec::new();
    prism(&mut o, 0.07, 0., 1.0, 0.07);
    prism(&mut o, 0.45, 0.62, 0.7, 0.04);
    prism(&mut o, 0.04, 0.55, 0.85, 0.32);
    o
}

fn dune() -> Vec<f32> {
    let mut o = Vec::new();
    let sides = 8;
    for i in 0..sides {
        let a0 = i as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let p = |a: f32| [a.cos(), 0., a.sin() * 0.38];
        tri(&mut o, p(a0), p(a1), [0.15, 1., 0.]);
    }
    o
}

fn hedge() -> Vec<f32> {
    let mut o = Vec::new();
    prism(&mut o, 1.0, 0., 1.0, 0.045);
    o
}

fn impostor() -> Vec<f32> {
    let mut o = Vec::new();
    tri(&mut o, [-0.7, 0., 0.], [0.7, 0., 0.], [0., 1., 0.]);
    tri(&mut o, [0., 0., -0.7], [0., 0., 0.7], [0., 1., 0.]);
    o
}
