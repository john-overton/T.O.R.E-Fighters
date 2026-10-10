//! City fill buildings, CF0 prototype (`tore_world::city`): one shared
//! vertex batch per kit piece, drawn instanced. Each view picks the
//! buildings near enough to draw from 8,192 ft chunks, culls whole chunks
//! behind the camera, and sinks small buildings into the ground before their
//! size-ordered draw distance, so houses go first and towers last (10 nm).
//! Past a nearer, size-ordered distance a building draws as its far level of
//! detail: its footprint box in its dominant colour, every one of them in a
//! single instanced draw with no texture pass. Knocked-down buildings draw
//! as a low stub. Fill buildings cast no shadow in this prototype.

use crate::{AppResult, camera::Camera};
use std::{collections::BTreeMap, ops::Range, sync::Arc};
use tore_world::city::{CityFill, kit::Class};

/// Feet per culling chunk side.
pub const CHUNK_FT: f64 = 8_192.;
/// The farthest any fill building draws: 10 nm (John, 2026-10-10).
pub const DRAW_FT: f64 = 10. * 6_076.115_49;
/// The last share of a building's draw distance over which it sinks into the
/// ground instead of popping out (fitted).
pub const SINK_SHARE: f64 = 0.15;
/// A flattened building's height, as a share of its standing height (fitted).
pub const FLAT_SHARE: f32 = 0.08;
/// Views that may draw in one submission, each with its own instance range.
const SLOTS: u64 = 8;
/// Floats per instance: base relative to the render origin, then cosine,
/// sine and height share of the heading.
const INSTANCE_FLOATS: usize = 8;
/// Floats per far box instance: base relative to the render origin and the
/// colour index, cosine, sine and height in feet, then the half footprint.
const BOX_FLOATS: usize = 12;

/// The draw distance of a piece class, before zoom (fitted, size-ordered).
pub fn draw_distance(class: Class) -> f64 {
    let nm = match class {
        Class::House => 4.,
        Class::Industrial | Class::LowRise => 6.,
        Class::Apartment => 8.,
        Class::Tower => 10.,
    };
    nm * 6_076.115_49
}

/// Where a piece class changes to its far box, before zoom (fitted,
/// size-ordered).
pub fn lod_distance(class: Class) -> f64 {
    let nm = match class {
        Class::House => 0.75,
        Class::Industrial | Class::LowRise => 1.25,
        Class::Apartment => 2.,
        Class::Tower => 3.,
    };
    nm * 6_076.115_49
}

/// The unit far box: four walls and a roof, x and z from -1 to 1, y from 0
/// to 1, untextured.
fn unit_box() -> Vec<f32> {
    let corner = |x: f32, y: f32, z: f32| [x, y, z, 0., 0., -1., 0., 0., 0., 0.];
    let quads = [
        [[-1., 0., -1.], [1., 0., -1.], [1., 1., -1.], [-1., 1., -1.]],
        [[1., 0., -1.], [1., 0., 1.], [1., 1., 1.], [1., 1., -1.]],
        [[1., 0., 1.], [-1., 0., 1.], [-1., 1., 1.], [1., 1., 1.]],
        [[-1., 0., 1.], [-1., 0., -1.], [-1., 1., -1.], [-1., 1., 1.]],
        [[-1., 1., -1.], [1., 1., -1.], [1., 1., 1.], [-1., 1., 1.]],
    ];
    let mut out = Vec::new();
    for q in quads {
        for i in [0, 1, 2, 0, 2, 3] {
            out.extend(corner(q[i][0], q[i][1], q[i][2]));
        }
    }
    out
}

/// A chunk's buildings that share one draw distance: the distance and their
/// indices.
type DistanceGroup = (f64, Vec<u32>);

/// The fill's art for one scenery: the kit's vertices and each building's
/// fixed placement, chunked.
pub struct FillArt {
    pub fill: Arc<CityFill>,
    /// Kit vertices, ten floats each, base-centred feet (x right, y up, z
    /// forward).
    pub vertices: Vec<f32>,
    /// Each piece's vertex ranges: untextured triangles, then textured ones.
    pub ranges: Vec<[Range<u32>; 2]>,
    /// The unit far box's vertex range, after the pieces.
    pub box_range: Range<u32>,
    /// Each piece's draw distance and far box distance.
    distances: Vec<(f64, f64)>,
    /// Each piece's far box colour: its dominant palette index by area.
    colors: Vec<f32>,
    /// Each building's world base, cosine and sine.
    places: Vec<([f64; 3], [f32; 2])>,
    /// Per chunk, its buildings grouped by draw distance, farthest first.
    chunks: BTreeMap<(i64, i64), Vec<DistanceGroup>>,
}

impl FillArt {
    /// Builds the kit vertices, appending its textures as static art pages
    /// after `first_layer` layers (the scenery's static art rule).
    pub fn build(
        resources: &BTreeMap<String, Vec<u8>>,
        fill: &Arc<CityFill>,
        pages: &mut Vec<u8>,
        first_layer: impl Fn(&Vec<u8>) -> usize,
    ) -> AppResult<Self> {
        let layout = &fill.layout;
        let mut shapes = BTreeMap::<String, tore_formats::shape::Shape>::new();
        let mut images = BTreeMap::<String, crate::static_art::Image>::new();
        let mut vertices = Vec::new();
        let mut ranges = Vec::new();
        let mut colors = Vec::new();
        let mut modes = BTreeMap::<String, u8>::new();
        for piece in &layout.kit.pieces {
            let mut weight = [0f64; 256];
            if !shapes.contains_key(&piece.shape) {
                let bytes = resources
                    .get(&piece.shape)
                    .ok_or_else(|| format!("city fill: missing {}", piece.shape))?;
                shapes.insert(
                    piece.shape.clone(),
                    tore_formats::shape::Shape::scenery(bytes)?,
                );
            }
            let shape = &shapes[&piece.shape];
            let mut solid = Vec::new();
            let mut textured = Vec::new();
            for &f in &piece.faces {
                let face = &shape.faces[f as usize];
                if face.positions.len() < 3 {
                    continue;
                }
                let image = if face.texture.is_empty() || face.uv.is_empty() {
                    None
                } else {
                    let name = face.texture.to_ascii_uppercase();
                    if !images.contains_key(&name) {
                        let pic = tore_formats::Pic::parse(
                            resources
                                .get(&name)
                                .ok_or_else(|| format!("city fill: missing texture {name}"))?,
                        )?;
                        let mut counts = [0usize; 256];
                        for (p, m) in pic.pixels.iter().zip(&pic.mask) {
                            if *m && *p != 255 {
                                counts[usize::from(*p)] += 1;
                            }
                        }
                        let mode = (0..256).max_by_key(|&i| (counts[i], i)).unwrap_or(0);
                        modes.insert(name.clone(), mode as u8);
                        let first = first_layer(pages);
                        let image = crate::static_art::Image::append(&pic, pages, first)?;
                        images.insert(name.clone(), image);
                    }
                    images.get(&name)
                };
                // The face's share of the far box colour: its area in shape
                // units, for its colour or its texture's commonest index.
                let a = face.positions[0];
                let mut area = 0f64;
                for w in face.positions[1..].windows(2) {
                    let (b, c) = (w[0], w[1]);
                    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]].map(f64::from);
                    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]].map(f64::from);
                    let n = [
                        u[1] * v[2] - u[2] * v[1],
                        u[2] * v[0] - u[0] * v[2],
                        u[0] * v[1] - u[1] * v[0],
                    ];
                    area += (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() * 0.5;
                }
                let index = match image {
                    Some(_) => modes[&face.texture.to_ascii_uppercase()],
                    None => face.colors[0],
                };
                weight[usize::from(index)] += area;
                for triangle in 1..face.positions.len() - 1 {
                    let mut points = [[0f32; 10]; 3];
                    for (slot, vertex_index) in [0, triangle, triangle + 1].into_iter().enumerate()
                    {
                        let p = face.positions[vertex_index];
                        let local = |axis: usize| {
                            ((f64::from(p[axis]) - piece.origin[axis]) * piece.scale) as f32
                        };
                        let uv = face.uv.get(vertex_index).copied().unwrap_or([0.0; 2]);
                        let uv = image.map_or([0., 0.], |image| {
                            [uv[0] + 0.5, image.height as f32 - 0.5 - uv[1]]
                        });
                        points[slot] = [
                            local(0),
                            local(2),
                            local(1),
                            uv[0],
                            uv[1],
                            -1.0,
                            0.0,
                            0.0,
                            0.0,
                            f32::from(face.colors[vertex_index]) + f32::from(face.fog as u8) * 256.,
                        ];
                    }
                    match image {
                        Some(image) => image.triangle(points, &mut textured, usize::MAX)?,
                        None => solid.extend(points.into_iter().flatten()),
                    }
                }
            }
            let start = (vertices.len() / 10) as u32;
            let middle = start + (solid.len() / 10) as u32;
            let end = middle + (textured.len() / 10) as u32;
            vertices.extend(solid);
            vertices.extend(textured);
            ranges.push([start..middle, middle..end]);
            let dominant = (0..256)
                .max_by(|&a, &b| weight[a].total_cmp(&weight[b]).then(a.cmp(&b)))
                .unwrap_or(0);
            colors.push(dominant as f32);
        }
        let box_start = (vertices.len() / 10) as u32;
        vertices.extend(unit_box());
        let box_range = box_start..(vertices.len() / 10) as u32;
        let distances: Vec<(f64, f64)> = layout
            .kit
            .pieces
            .iter()
            .map(|p| (draw_distance(p.class), lod_distance(p.class)))
            .collect();
        let one = tore_world::surface::layout::trig::ONE as f64;
        let mut places = Vec::with_capacity(layout.instances.len());
        let mut chunks = BTreeMap::<(i64, i64), BTreeMap<u64, Vec<u32>>>::new();
        for (n, i) in layout.instances.iter().enumerate() {
            let heading = i32::from(i.heading);
            places.push((
                [f64::from(i.x), f64::from(fill.base[n]), f64::from(i.z)],
                [
                    (tore_world::surface::layout::trig::cos(heading) as f64 / one) as f32,
                    (tore_world::surface::layout::trig::sin(heading) as f64 / one) as f32,
                ],
            ));
            let key = (
                (f64::from(i.x) / CHUNK_FT).floor() as i64,
                (f64::from(i.z) / CHUNK_FT).floor() as i64,
            );
            let limit = distances[usize::from(i.piece)].0;
            chunks
                .entry(key)
                .or_default()
                .entry(u64::MAX - limit.to_bits())
                .or_default()
                .push(n as u32);
        }
        let chunks = chunks
            .into_iter()
            .map(|(key, groups)| {
                let groups = groups
                    .into_iter()
                    .map(|(bits, members)| (f64::from_bits(u64::MAX - bits), members))
                    .collect();
                (key, groups)
            })
            .collect();
        Ok(Self {
            fill: Arc::clone(fill),
            vertices,
            ranges,
            box_range,
            distances,
            colors,
            places,
            chunks,
        })
    }

    /// Kit vertex bytes on the GPU.
    pub fn vertex_bytes(&self) -> u64 {
        self.vertices.len() as u64 * 4
    }
}

/// What the last prepared view drew.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Buildings drawn, near and far.
    pub instances: usize,
    /// ... of which as far boxes.
    pub boxes: usize,
    pub draws: usize,
    /// Instance ring bytes on the GPU.
    pub ring_bytes: u64,
    /// CPU time picking and writing the view's buildings, microseconds.
    pub prepare_us: u64,
}

pub struct FillRenderer {
    art: Arc<FillArt>,
    vertices: wgpu::Buffer,
    /// Near and far instance rings, each `SLOTS` slots of its capacity.
    instances: Ring,
    boxes: Ring,
    slot: u64,
    /// Per piece in view: its two vertex ranges and its instances.
    draws: Vec<([Range<u32>; 2], Range<u32>)>,
    /// This view's far boxes, in the box ring.
    box_draw: Range<u32>,
    solid: wgpu::RenderPipeline,
    decal: wgpu::RenderPipeline,
    far: wgpu::RenderPipeline,
    /// Per-piece scratch for the counting sort.
    counts: Vec<u32>,
    picked: Vec<(u16, [f32; INSTANCE_FLOATS])>,
    data: Vec<f32>,
    box_data: Vec<f32>,
    pub stats: Stats,
}

/// A growable instance ring: `SLOTS` slots of `capacity` instances.
struct Ring {
    buffer: wgpu::Buffer,
    capacity: u64,
    stride: u64,
    label: &'static str,
}

impl Ring {
    fn new(device: &wgpu::Device, label: &'static str, stride: u64, capacity: u64) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: SLOTS * capacity * stride,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
            stride,
            label,
        }
    }
    /// Writes `data` (`count` instances) into `slot`, growing the ring when
    /// it does not fit; returns the first instance index.
    fn write(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        slot: u64,
        count: u64,
        data: &[f32],
    ) -> u32 {
        if count > self.capacity {
            *self = Self::new(device, self.label, self.stride, count.next_power_of_two());
        }
        let first = slot * self.capacity;
        queue.write_buffer(
            &self.buffer,
            first * self.stride,
            &crate::sim_renderer::bytes(data),
        );
        first as u32
    }
    fn bytes(&self) -> u64 {
        SLOTS * self.capacity * self.stride
    }
}

fn pipelines(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    samples: u32,
    material_layout: &wgpu::BindGroupLayout,
    lighting_layout: &wgpu::BindGroupLayout,
) -> [wgpu::RenderPipeline; 3] {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("City fill layout"),
        bind_group_layouts: &[material_layout, lighting_layout],
        push_constant_ranges: &[],
    });
    let vertex = wgpu::VertexBufferLayout {
        array_stride: 40,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3,4=>Float32],
    };
    let near = [
        vertex.clone(),
        wgpu::VertexBufferLayout {
            array_stride: (INSTANCE_FLOATS * 4) as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![5=>Float32x4,6=>Float32x4],
        },
    ];
    let far = [
        vertex,
        wgpu::VertexBufferLayout {
            array_stride: (BOX_FLOATS * 4) as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![5=>Float32x4,6=>Float32x4,7=>Float32x4],
        },
    ];
    let make = |label: &str,
                entry: &str,
                buffers: &[wgpu::VertexBufferLayout<'_>],
                fragment: &str,
                bias: i32| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fragment),
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
            // The static airport passes' depth rules: solid art, then
            // coplanar texture details one reversed-depth ULP in front. Each
            // pass draws only its own triangles, and the solid passes have no
            // discard, so they keep the early depth test.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: bias,
                    ..Default::default()
                },
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            multiview: None,
            cache: None,
        })
    };
    [
        make(
            "City fill surfaces",
            "fill_vertex",
            &near,
            "fill_solid_fragment",
            0,
        ),
        make(
            "City fill texture details",
            "fill_vertex",
            &near,
            "airport_fragment",
            1,
        ),
        make(
            "City fill far boxes",
            "fill_box_vertex",
            &far,
            "fill_far_fragment",
            0,
        ),
    ]
}

impl FillRenderer {
    pub fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        samples: u32,
        material_layout: &wgpu::BindGroupLayout,
        lighting_layout: &wgpu::BindGroupLayout,
        art: &Arc<FillArt>,
    ) -> Self {
        use wgpu::util::DeviceExt;
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("City fill kit vertices"),
            contents: &crate::sim_renderer::bytes(&art.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let instances = Ring::new(
            device,
            "City fill instances",
            (INSTANCE_FLOATS * 4) as u64,
            4_096,
        );
        let boxes = Ring::new(
            device,
            "City fill far boxes",
            (BOX_FLOATS * 4) as u64,
            4_096,
        );
        let [solid, decal, far] = pipelines(
            device,
            shader,
            format,
            samples,
            material_layout,
            lighting_layout,
        );
        Self {
            art: Arc::clone(art),
            vertices,
            instances,
            boxes,
            slot: 0,
            draws: Vec::new(),
            box_draw: 0..0,
            solid,
            decal,
            far,
            counts: vec![0; art.ranges.len()],
            picked: Vec::new(),
            data: Vec::new(),
            box_data: Vec::new(),
            stats: Stats::default(),
        }
    }

    /// Rebuilds the pipelines for a new sample count.
    pub fn rebuild(
        &mut self,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        samples: u32,
        material_layout: &wgpu::BindGroupLayout,
        lighting_layout: &wgpu::BindGroupLayout,
    ) {
        [self.solid, self.decal, self.far] = pipelines(
            device,
            shader,
            format,
            samples,
            material_layout,
            lighting_layout,
        );
    }

    /// Picks this view's buildings and writes them into the next slot.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        origin: [f64; 3],
    ) {
        let started = std::time::Instant::now();
        self.draws.clear();
        self.picked.clear();
        self.box_data.clear();
        self.box_draw = 0..0;
        let art = &self.art;
        let fill = &art.fill;
        let eye = camera.position;
        let (sy, cy) = f64::from(camera.yaw).sin_cos();
        let (sp, cp) = f64::from(camera.pitch).sin_cos();
        let forward = [sy * cp, sp, cy * cp];
        let zoom = f64::from(camera.zoom).clamp(1., 4.);
        let reach = DRAW_FT * zoom;
        let chunk_radius = CHUNK_FT * std::f64::consts::FRAC_1_SQRT_2 + 400.;
        let lo = (
            ((eye[0] - reach) / CHUNK_FT).floor() as i64,
            ((eye[2] - reach) / CHUNK_FT).floor() as i64,
        );
        let hi = (
            ((eye[0] + reach) / CHUNK_FT).floor() as i64,
            ((eye[2] + reach) / CHUNK_FT).floor() as i64,
        );
        for (&(cx, cz), groups) in art.chunks.range((lo.0, lo.1)..=(hi.0, hi.1)) {
            if cz < lo.1 || cz > hi.1 {
                continue;
            }
            let first = groups[0].1[0] as usize;
            let center = [
                (cx as f64 + 0.5) * CHUNK_FT,
                fill.base[first] as f64,
                (cz as f64 + 0.5) * CHUNK_FT,
            ];
            let d: [f64; 3] = std::array::from_fn(|i| center[i] - eye[i]);
            let nearest = d[0].hypot(d[2]) - chunk_radius;
            if nearest > reach {
                continue;
            }
            if d[0] * forward[0] + d[1] * forward[1] + d[2] * forward[2] < -chunk_radius {
                continue;
            }
            for (limit, members) in groups {
                // Groups run farthest first: the rest are out of reach too.
                if nearest >= limit * zoom {
                    break;
                }
                for &n in members {
                    let n = n as usize;
                    if fill.hidden[n] {
                        continue;
                    }
                    let i = &fill.layout.instances[n];
                    let (place, turn) = art.places[n];
                    let dx = place[0] - eye[0];
                    let dz = place[2] - eye[2];
                    let dy = place[1] - eye[1];
                    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
                    let (limit, lod) = art.distances[usize::from(i.piece)];
                    let (limit, lod) = (limit * zoom, lod * zoom);
                    if distance >= limit {
                        continue;
                    }
                    let sink = ((limit - distance) / (limit * SINK_SHARE)).min(1.) as f32;
                    let height = if fill.is_flattened(n) { FLAT_SHARE } else { 1. } * sink;
                    let local = [
                        (place[0] - origin[0]) as f32,
                        (place[1] - origin[1]) as f32,
                        (place[2] - origin[2]) as f32,
                    ];
                    if distance < lod {
                        self.picked.push((
                            i.piece,
                            [
                                local[0], local[1], local[2], 0., turn[0], turn[1], height, 0.,
                            ],
                        ));
                    } else {
                        let piece = &fill.layout.kit.pieces[usize::from(i.piece)];
                        self.box_data.extend([
                            local[0],
                            local[1],
                            local[2],
                            art.colors[usize::from(i.piece)],
                            turn[0],
                            turn[1],
                            height * piece.height as f32,
                            0.,
                            piece.half[0] as f32,
                            piece.half[1] as f32,
                            0.,
                            0.,
                        ]);
                    }
                }
            }
        }
        self.counts.iter_mut().for_each(|c| *c = 0);
        for (piece, _) in &self.picked {
            self.counts[usize::from(*piece)] += 1;
        }
        let mut starts = Vec::with_capacity(self.counts.len());
        let mut total = 0u32;
        for &c in &self.counts {
            starts.push(total);
            total += c;
        }
        self.data.clear();
        self.data.resize(total as usize * INSTANCE_FLOATS, 0.);
        let mut cursor = starts.clone();
        for (piece, values) in &self.picked {
            let at = cursor[usize::from(*piece)] as usize * INSTANCE_FLOATS;
            self.data[at..at + INSTANCE_FLOATS].copy_from_slice(values);
            cursor[usize::from(*piece)] += 1;
        }
        let boxes = (self.box_data.len() / BOX_FLOATS) as u32;
        self.slot = (self.slot + 1) % SLOTS;
        if total > 0 {
            let first =
                self.instances
                    .write(device, queue, self.slot, u64::from(total), &self.data);
            for (piece, &count) in self.counts.iter().enumerate() {
                let range = art.ranges[piece].clone();
                if count == 0 || range.iter().all(Range::is_empty) {
                    continue;
                }
                let start = first + starts[piece];
                self.draws.push((range, start..start + count));
            }
        }
        if boxes > 0 {
            let first =
                self.boxes
                    .write(device, queue, self.slot, u64::from(boxes), &self.box_data);
            self.box_draw = first..first + boxes;
        }
        self.stats = Stats {
            instances: (total + boxes) as usize,
            boxes: boxes as usize,
            draws: self.draws.len() * 2 + usize::from(boxes > 0),
            ring_bytes: self.instances.bytes() + self.boxes.bytes(),
            prepare_us: started.elapsed().as_micros() as u64,
        };
    }

    /// Draws the prepared view's buildings: solid art, then texture details.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, bind: &'a wgpu::BindGroup) {
        if self.draws.is_empty() && self.box_draw.is_empty() {
            return;
        }
        pass.set_bind_group(0, bind, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        if !self.box_draw.is_empty() {
            pass.set_pipeline(&self.far);
            pass.set_vertex_buffer(1, self.boxes.buffer.slice(..));
            pass.draw(self.art.box_range.clone(), self.box_draw.clone());
        }
        pass.set_vertex_buffer(1, self.instances.buffer.slice(..));
        for (kind, pipeline) in [&self.solid, &self.decal].into_iter().enumerate() {
            pass.set_pipeline(pipeline);
            for (vertices, instances) in &self.draws {
                if !vertices[kind].is_empty() {
                    pass.draw(vertices[kind].clone(), instances.clone());
                }
            }
        }
    }
}
