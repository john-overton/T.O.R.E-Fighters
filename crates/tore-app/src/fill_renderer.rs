//! City fill buildings, CF0 prototype (`tore_world::city`), drawn in three
//! tiers out to 30 nm (John, 2026-10-10): full detail (the kit piece with its
//! textures) inside 20 nm, a blend band from 20 to 25 nm where each building
//! turns from its far box to full detail at its own fixed distance, and the
//! far box (its footprint in its dominant colour, untextured, a cheap
//! shader) from 25 to 30 nm. Buildings sink into the ground over the last
//! nautical mile, and anything smaller than one pixel on screen is not drawn.
//!
//! The per-view work is on the GPU: a compute pass culls every building
//! against the view and writes the full detail lists per piece and the far
//! box list, and indirect draws read them (one per piece and pass, plus one
//! for every far box). The CPU writes one small uniform per view. Knocked-down
//! buildings draw as a low stub (0.08 of their height, in the cull shader).
//! Fill buildings cast no shadow yet.

use crate::{AppResult, camera::Camera};
use std::{collections::BTreeMap, ops::Range, sync::Arc};
use tore_world::city::CityFill;

const NM: f64 = 6_076.115_49;

/// The tier distances, nautical miles, and the smallest drawn size, pixels.
/// `TORE_CITY_FILL_TIERS=FULL,BLEND,DRAW,MIN_PX[,DETAIL_PX]` changes them
/// (a measuring aid).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tiers {
    /// Full detail inside this.
    pub full_nm: f64,
    /// The blend band ends here; far boxes beyond.
    pub blend_nm: f64,
    /// Nothing beyond this.
    pub draw_nm: f64,
    /// Buildings smaller than this on screen are not drawn (fitted).
    pub min_px: f64,
    /// Buildings smaller than this on screen draw as far boxes even inside
    /// the full detail range (0: distance alone decides).
    pub detail_px: f64,
}

impl Default for Tiers {
    fn default() -> Self {
        Self {
            full_nm: 20.,
            blend_nm: 25.,
            draw_nm: 30.,
            min_px: 1.,
            detail_px: 0.,
        }
    }
}

impl Tiers {
    pub fn from_env() -> Self {
        let mut tiers = Self::default();
        if let Ok(text) = std::env::var("TORE_CITY_FILL_TIERS") {
            let v: Vec<f64> = text.split(',').filter_map(|s| s.parse().ok()).collect();
            if v.len() >= 4 {
                tiers = Self {
                    full_nm: v[0],
                    blend_nm: v[1],
                    draw_nm: v[2],
                    min_px: v[3],
                    detail_px: v.get(4).copied().unwrap_or(0.),
                };
            }
        }
        tiers
    }
}

/// The length over which a building sinks into the ground before the draw
/// distance (fitted).
pub const SINK_FT: f64 = NM;
/// Views that may draw in one submission, each with its own uniform.
const SLOTS: usize = 8;

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

/// The fill's art for one scenery: the kit's vertices, the piece table and
/// every building's fixed record for the cull pass.
pub struct FillArt {
    pub fill: Arc<CityFill>,
    /// Kit vertices, ten floats each, base-centred feet (x right, y up, z
    /// forward), then the unit far box.
    pub vertices: Vec<f32>,
    /// Each piece's vertex ranges: untextured triangles, then textured ones.
    pub ranges: Vec<[Range<u32>; 2]>,
    /// The unit far box's vertex range.
    pub box_range: Range<u32>,
    /// Per piece: its first slot in the full detail list, colour, half
    /// footprint, height and radius (the cull shader's `Piece`).
    piece_table: Vec<u32>,
    /// Per building: world base, piece, cosine, sine, hash and flags (the
    /// cull shader's `Inst`), eight words each.
    records: Vec<u32>,
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
        let one = tore_world::surface::layout::trig::ONE as f64;
        // The full detail list holds, per piece, room for every building of
        // that piece.
        let mut per_piece = vec![0u32; layout.kit.pieces.len()];
        for i in &layout.instances {
            per_piece[usize::from(i.piece)] += 1;
        }
        let mut piece_table = Vec::with_capacity(per_piece.len() * 8);
        let mut offset = 0u32;
        for (n, p) in layout.kit.pieces.iter().enumerate() {
            let radius = ((p.half[0] * p.half[0] + p.half[1] * p.half[1]) as f64).sqrt() as f32;
            piece_table.extend([
                offset,
                colors[n].to_bits(),
                (p.half[0] as f32).to_bits(),
                (p.half[1] as f32).to_bits(),
                (p.height as f32).to_bits(),
                radius.to_bits(),
                0,
                0,
            ]);
            offset += per_piece[n];
        }
        let mut records = Vec::with_capacity(layout.instances.len() * 8);
        for (n, i) in layout.instances.iter().enumerate() {
            let heading = i32::from(i.heading);
            let cos = (tore_world::surface::layout::trig::cos(heading) as f64 / one) as f32;
            let sin = (tore_world::surface::layout::trig::sin(heading) as f64 / one) as f32;
            // A fixed per-building number in [0, 1): where in the blend band
            // it turns to full detail.
            let mut h = (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            h ^= h >> 29;
            let hash = (h >> 40) as f32 / (1u64 << 24) as f32;
            records.extend([
                (i.x as f32).to_bits(),
                fill.base[n].to_bits(),
                (i.z as f32).to_bits(),
                u32::from(i.piece),
                cos.to_bits(),
                sin.to_bits(),
                hash.to_bits(),
                u32::from(fill.hidden[n]),
            ]);
        }
        Ok(Self {
            fill: Arc::clone(fill),
            vertices,
            ranges,
            box_range,
            piece_table,
            records,
        })
    }

    /// Kit vertex bytes on the GPU.
    pub fn vertex_bytes(&self) -> u64 {
        self.vertices.len() as u64 * 4
    }

    /// Buildings in the art.
    pub fn len(&self) -> usize {
        self.records.len() / 8
    }
}

/// What a view drew, read back from the GPU on request (a measuring aid).
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Buildings drawn at full detail (inside the blend band's far edge).
    pub full: usize,
    /// Buildings drawn as far boxes.
    pub boxes: usize,
}

pub struct FillRenderer {
    art: Arc<FillArt>,
    tiers: Tiers,
    vertices: wgpu::Buffer,
    records: wgpu::Buffer,
    counts: wgpu::Buffer,
    full: wgpu::Buffer,
    boxes: wgpu::Buffer,
    args: wgpu::Buffer,
    readback: wgpu::Buffer,
    params: Vec<wgpu::Buffer>,
    binds: Vec<wgpu::BindGroup>,
    slot: usize,
    /// The bind group of the last prepared view, and whether it drew.
    prepared: bool,
    flattened_version: u64,
    cull: wgpu::ComputePipeline,
    finish: wgpu::ComputePipeline,
    solid: wgpu::RenderPipeline,
    decal: wgpu::RenderPipeline,
    far: wgpu::RenderPipeline,
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
            array_stride: 32,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![5=>Float32x4,6=>Float32x4],
        },
    ];
    let far = [
        vertex,
        wgpu::VertexBufferLayout {
            array_stride: 48,
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

/// Words in the cull uniform.
const PARAM_WORDS: usize = 32;

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
        let storage = |label: &str, contents: &[u8], usage: wgpu::BufferUsages| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let words = |w: &[u32]| w.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>();
        let n = art.len().max(1) as u64;
        let pieces = art.ranges.len();
        let vertices = storage(
            "City fill kit vertices",
            &crate::sim_renderer::bytes(&art.vertices),
            wgpu::BufferUsages::VERTEX,
        );
        let records = storage(
            "City fill buildings",
            &words(&art.records),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let piece_table = storage(
            "City fill pieces",
            &words(&art.piece_table),
            wgpu::BufferUsages::STORAGE,
        );
        let counts = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("City fill counts"),
            size: (pieces as u64 + 1) * 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let full = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("City fill full detail list"),
            size: n * 32,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let boxes = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("City fill far box list"),
            size: n * 48,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        // Indirect draws: each piece's solid and textured ranges, then the
        // far box; instance counts start at zero.
        let mut args = Vec::with_capacity((2 * pieces + 1) * 4);
        // First instance stays 0 (a non-zero one needs a device feature):
        // each piece's draws bind the full detail list at that piece's slot.
        for range in &art.ranges {
            for r in range {
                args.extend([r.end - r.start, 0, r.start, 0]);
            }
        }
        args.extend([
            art.box_range.end - art.box_range.start,
            0,
            art.box_range.start,
            0,
        ]);
        let args = storage(
            "City fill indirect draws",
            &words(&args),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT,
        );
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("City fill count readback"),
            size: (pieces as u64 + 1) * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("City fill cull"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fill_cull.wgsl").into()),
        });
        let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let read = wgpu::BufferBindingType::Storage { read_only: true };
        let write = wgpu::BufferBindingType::Storage { read_only: false };
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("City fill cull bindings"),
            entries: &[
                entry(0, read),
                entry(1, read),
                entry(2, wgpu::BufferBindingType::Uniform),
                entry(3, write),
                entry(4, write),
                entry(5, write),
                entry(6, write),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("City fill cull layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
        let compute = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let cull = compute("cull");
        let finish = compute("finish");
        let params: Vec<wgpu::Buffer> = (0..SLOTS)
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("City fill cull view"),
                    size: (PARAM_WORDS * 4) as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let binds = params
            .iter()
            .map(|p| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("City fill cull bindings"),
                    layout: &bind_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: records.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: piece_table.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: p.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: counts.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: full.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: boxes.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: args.as_entire_binding(),
                        },
                    ],
                })
            })
            .collect();
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
            tiers: Tiers::from_env(),
            vertices,
            records,
            counts,
            full,
            boxes,
            args,
            readback,
            params,
            binds,
            slot: 0,
            prepared: false,
            flattened_version: 0,
            cull,
            finish,
            solid,
            decal,
            far,
        }
    }

    /// Rebuilds the render pipelines for a new sample count.
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

    /// GPU bytes: kit, building records, the two lists, counts and draws.
    pub fn gpu_bytes(&self) -> u64 {
        self.vertices.size()
            + self.records.size()
            + self.full.size()
            + self.boxes.size()
            + self.counts.size()
            + self.args.size()
    }

    /// Records this view's cull: clears the counts, culls every building and
    /// sets the indirect draws. `size` is the world pass's pixel size.
    pub fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        camera: &Camera,
        origin: [f64; 3],
        size: [u32; 2],
    ) {
        let fill = &self.art.fill;
        // Knocked-down buildings: refresh the records' flags.
        let version = fill.version();
        if version != self.flattened_version {
            self.flattened_version = version;
            let mut records = self.art.records.clone();
            for n in 0..self.art.len() {
                records[n * 8 + 7] =
                    u32::from(fill.hidden[n]) | (u32::from(fill.is_flattened(n)) << 1);
            }
            let bytes: Vec<u8> = records.iter().flat_map(|v| v.to_le_bytes()).collect();
            queue.write_buffer(&self.records, 0, &bytes);
        }
        let (sy, cy) = camera.yaw.sin_cos();
        let (sp, cp) = camera.pitch.sin_cos();
        let (sr, cr) = camera.roll.sin_cos();
        let right = [cy * cr - sy * sp * sr, cp * sr, -sy * cr - cy * sp * sr];
        let up = [-cy * sr - sy * sp * cr, cp * cr, sy * sr - cy * sp * cr];
        let forward = [sy * cp, sp, cy * cp];
        let height = (size[1] as f32 * camera.view_fraction).max(1.);
        let aspect = size[0] as f32 / height;
        let f = 1.732_050_8 * camera.zoom.max(0.01);
        let zoom = f64::from(camera.zoom).clamp(1., 4.);
        let eye: [f32; 3] = std::array::from_fn(|i| (camera.position[i] - origin[i]) as f32);
        let t = self.tiers;
        let mut p = [0f32; PARAM_WORDS];
        p[0..3].copy_from_slice(&origin.map(|v| v as f32));
        p[4..7].copy_from_slice(&eye);
        p[8..11].copy_from_slice(&forward);
        p[12..15].copy_from_slice(&right);
        p[16..19].copy_from_slice(&up);
        p[20] = (t.full_nm * NM * zoom) as f32;
        p[21] = (t.blend_nm * NM * zoom) as f32;
        p[22] = (t.draw_nm * NM * zoom) as f32;
        p[23] = SINK_FT as f32;
        p[24] = aspect / f * 1.05;
        p[25] = 1. / f * 1.05;
        p[26] = f * height * 0.5;
        p[27] = t.min_px as f32;
        let mut bytes: Vec<u8> = p.iter().flat_map(|v| v.to_le_bytes()).collect();
        bytes[28 * 4..29 * 4].copy_from_slice(&(self.art.len() as u32).to_le_bytes());
        bytes[29 * 4..30 * 4].copy_from_slice(&(self.art.ranges.len() as u32).to_le_bytes());
        bytes[30 * 4..31 * 4].copy_from_slice(&(t.detail_px as f32).to_le_bytes());
        self.slot = (self.slot + 1) % SLOTS;
        queue.write_buffer(&self.params[self.slot], 0, &bytes);
        encoder.clear_buffer(&self.counts, 0, None);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("City fill cull"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &self.binds[self.slot], &[]);
        pass.set_pipeline(&self.cull);
        pass.dispatch_workgroups((self.art.len() as u32).div_ceil(256).max(1), 1, 1);
        pass.set_pipeline(&self.finish);
        pass.dispatch_workgroups((self.art.ranges.len() as u32 + 1).div_ceil(64), 1, 1);
        drop(pass);
        self.prepared = true;
    }

    /// Draws the prepared view: far boxes, then each piece's solid art and
    /// texture details, all indirect.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, bind: &'a wgpu::BindGroup) {
        if !self.prepared {
            return;
        }
        pass.set_bind_group(0, bind, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        let pieces = self.art.ranges.len() as u64;
        pass.set_pipeline(&self.far);
        pass.set_vertex_buffer(1, self.boxes.slice(..));
        pass.draw_indirect(&self.args, 2 * pieces * 16);
        for (kind, pipeline) in [&self.solid, &self.decal].into_iter().enumerate() {
            pass.set_pipeline(pipeline);
            for (p, range) in self.art.ranges.iter().enumerate() {
                let first = u64::from(self.art.piece_table[p * 8]) * 32;
                if !range[kind].is_empty() && first < self.full.size() {
                    pass.set_vertex_buffer(1, self.full.slice(first..));
                    pass.draw_indirect(&self.args, (2 * p as u64 + kind as u64) * 16);
                }
            }
        }
    }

    /// Reads back what the last view drew (waits for the GPU; a measuring
    /// aid).
    pub fn stats(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> AppResult<Stats> {
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&self.counts, 0, &self.readback, 0, self.counts.size());
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
        device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(60)),
        })?;
        rx.recv()??;
        let counts: Vec<u32> = self
            .readback
            .slice(..)
            .get_mapped_range()
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        self.readback.unmap();
        let (boxes, full) = counts.split_last().map_or((0, &[][..]), |(b, f)| (*b, f));
        Ok(Stats {
            full: full.iter().map(|&c| c as usize).sum(),
            boxes: boxes as usize,
        })
    }
}
