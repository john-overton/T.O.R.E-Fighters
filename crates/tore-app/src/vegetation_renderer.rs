//! Cached vegetation impostors and the unchanged grass layer. Source shapes
//! are rasterized once into indexed atlas pages. GPU culling compacts all
//! visible species into one instance stream and one indirect draw. Not collidable.

use crate::vegetation::{self, Inst, Patch, Placement, Species};
use crate::vegetation_shapes::{FLOATS, Library, Look};
use crate::{camera::Camera, terrain::Terrain};
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[path = "vegetation_stream.rs"]
pub(crate) mod stream;

#[path = "vegetation_popin.rs"]
pub(crate) mod popin;

const INST_STRIDE: u64 = 64;
const DRAWN_STRIDE: u64 = 96;
const PATCH_STRIDE: u64 = 64;
/// Grass distance rings: outer edge as a share of the grass radius, and
/// the share of a patch's full tufts drawn in it.
const RINGS: [(f32, f32); 4] = [(0.14, 1.), (0.32, 0.5), (0.6, 0.25), (1.0, 0.125)];

pub struct Renderer {
    atlas: Arc<vegetation::Atlas>,
    worker: Option<stream::Worker>,
    epoch: u64,
    ready: Option<stream::Ready>,
    incoming: Option<stream::Ready>,
    chunks: wgpu::Buffer,
    pages: stream::Slots,
    page_count: u32,
    depth_pipeline: wgpu::RenderPipeline,
    library: Arc<Library>,
    vertices: wgpu::Buffer,
    instances: wgpu::Buffer,
    drawn: wgpu::Buffer,
    shadow_drawn: wgpu::Buffer,
    counts: wgpu::Buffer,
    args: wgpu::Buffer,
    params: wgpu::Buffer,
    meshes: wgpu::Buffer,
    mesh_dirty: bool,
    patches: wgpu::Buffer,
    bind: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
    /// Objects the instance buffers hold: the density's budget.
    capacity: usize,
    cull: wgpu::ComputePipeline,
    finish: wgpu::ComputePipeline,
    pipeline: wgpu::RenderPipeline,
    grass_pipeline: wgpu::RenderPipeline,
    count: u32,
    /// Per ring: patch range and tufts per patch.
    rings: Vec<(std::ops::Range<u32>, u32)>,
    prepared: bool,
    suppressed: bool,
    settle: bool,
    draw_nm: f32,
    visible_reach: f32,
    last_prepare: std::time::Instant,
    last_eye: Option<[f64; 2]>,
    detail_nm: f32,
    density: f32,
    grass_radius: f32,
    tufts: u32,
    cards: crate::graphics::VegCards,
    /// Object chunks, grass chunks and CPU build time on the last `prepare`.
    last_cache: CacheFrame,
}

/// Cache work done while preparing one frame. Zeros when vegetation is hidden.
#[derive(Clone, Copy, Debug, Default)]
pub struct CacheFrame {
    pub object_chunks: u64,
    pub grass_chunks: u64,
    pub build_ms: f64,
    pub pending: bool,
    pub submit_ms: f64,
    pub packed_chunks: u64,
    pub uploaded_bytes: usize,
    pub upload_chunks: usize,
    pub upload_bytes: usize,
    pub worker_pending: bool,
    pub range_pending: bool,
    pub available: f32,
    pub visible: f32,
}

impl CacheFrame {
    pub const CSV_HEADER: &str = "frame,object_builds,grass_builds,packed_chunks,uploaded_bytes,upload_chunks,upload_bytes,worker_pending,range_pending,available_ft,visible_ft,pending\n";

    pub fn write_csv(&self, out: &mut impl std::io::Write, frame: usize) -> std::io::Result<()> {
        writeln!(
            out,
            "{frame},{},{},{},{},{},{},{},{},{:.3},{:.3},{}",
            self.object_chunks,
            self.grass_chunks,
            self.packed_chunks,
            self.uploaded_bytes,
            self.upload_chunks,
            self.upload_bytes,
            self.worker_pending,
            self.range_pending,
            self.available,
            self.visible,
            self.pending
        )
    }
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
            (mesh_count as u64).max(1) * 48,
            storage,
        );
        let patches = buffer(
            "Grass patches",
            vegetation::MAX_PATCHES as u64 * PATCH_STRIDE,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        let chunks = buffer(
            "Vegetation chunk pages",
            (stream::SPARE_PAGES + 524_288 / stream::PAGE) as u64 * 8,
            storage,
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
                storage_entry(7, true),
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
                &chunks,
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
        let (pipeline, grass_pipeline, depth_pipeline) =
            render_pipelines(device, shader, format, samples, material, &lighting.layout);
        Self {
            atlas,
            worker: None,
            epoch: 0,
            ready: None,
            incoming: None,
            chunks,
            pages: stream::Slots::new(capacity),
            page_count: 0,
            depth_pipeline,
            library,
            vertices,
            instances,
            drawn,
            shadow_drawn,
            counts,
            args,
            params,
            meshes,
            mesh_dirty: true,
            patches,
            bind,
            layout,
            capacity,
            cull,
            finish,
            pipeline,
            grass_pipeline,
            count: 0,
            rings: Vec::new(),
            prepared: false,
            suppressed: false,
            settle: false,
            draw_nm: 0.,
            visible_reach: 0.,
            last_prepare: std::time::Instant::now(),
            last_eye: None,
            detail_nm: 0.,
            density: 1.,
            grass_radius: 0.,
            tufts: 0,
            cards: Default::default(),
            last_cache: CacheFrame::default(),
        }
    }

    pub fn set_options(&mut self, device: &wgpu::Device, options: &crate::graphics::Options) {
        self.mesh_dirty = true;
        self.cards = options.vegetation_cards.effective();
        self.draw_nm = options.vegetation_distance_nm as f32;
        self.detail_nm = (options.vegetation_detail_nm as f32).min(self.draw_nm);
        let density = options.vegetation_density;
        if self.density != density.scale() {
            self.reset_cache();
        }
        self.density = density.scale();
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
                    &self.chunks,
                ],
            );
            self.count = 0;
            self.pages = stream::Slots::new(capacity);
            self.page_count = 0;
        }
        (self.grass_radius, self.tufts) = density.grass();
        self.grass_radius = self
            .grass_radius
            .min(self.detail_nm * vegetation::NM as f32 * 0.5);
    }

    /// The before picture: objects and grass are not drawn. Ground detail
    /// is the graphics option's own switch.
    pub fn suppressed(&self) -> bool {
        self.suppressed
    }

    pub fn set_suppressed(&mut self, suppressed: bool) {
        self.suppressed = suppressed;
    }

    /// Build every chunk the view needs before drawing (previews and
    /// captures), instead of a few per frame.
    pub fn set_settle(&mut self, settle: bool) {
        self.settle = settle;
    }

    /// Drops the patch cache so the next frames fill it at the current budget.
    pub fn reset_cache(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.ready = None;
        self.incoming = None;
        self.pages = stream::Slots::new(self.capacity);
        self.page_count = 0;
        self.count = 0;
        self.rings.clear();
        self.prepared = false;
        self.last_cache = CacheFrame::default();
    }

    /// Chunks built while preparing the last frame, and the CPU time that took.
    pub fn cache_frame(&self) -> CacheFrame {
        self.last_cache
    }

    // PASS6_POPIN_OBSERVER_BEGIN
    /// Observe the uploaded population without GPU readback or frustum-entry
    /// false positives. Called only by the explicit low-pass benchmark.
    pub fn near_population(&self, camera: &Camera, _size: [u32; 2]) -> popin::Population {
        let full = self.detail_nm * vegetation::NM as f32;
        let mut out = popin::Population::new();
        if let Some(ready) = &self.ready {
            for (_, bytes) in &ready.chunks {
                for bytes in bytes.chunks_exact(INST_STRIDE as usize) {
                    let words: [f32; 16] = std::array::from_fn(|i| {
                        f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
                    });
                    let distance = (words[0] - camera.position[0] as f32)
                        .hypot(words[2] - camera.position[2] as f32);
                    if distance > full || distance >= self.visible_reach {
                        continue;
                    }
                    let fade = 1.;
                    out.insert(
                        [
                            words[0].to_bits(),
                            words[1].to_bits(),
                            words[2].to_bits(),
                            words[12].to_bits(),
                        ],
                        fade,
                    );
                }
            }
        }
        out
    }
    pub fn detail_reach(&self) -> f32 {
        self.detail_nm * vegetation::NM as f32
    }
    // PASS6_POPIN_OBSERVER_END

    /// Objects placed around the camera.
    pub fn placed(&self) -> u32 {
        self.count
    }

    /// Feet to the farthest chunk of objects within the instance budget.
    pub fn kept_reach(&self, _eye: [f64; 2]) -> f32 {
        self.ready.as_ref().map_or(0., |r| r.reach)
    }

    /// How far full density reaches, as a share of the setting's distance.
    pub fn draw_reach(&self) -> f32 {
        self.visible_reach
    }

    pub fn squeeze(&self) -> f32 {
        self.ready.as_ref().map_or(1., |r| r.squeeze)
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
        (self.pipeline, self.grass_pipeline, self.depth_pipeline) =
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
        let started = std::time::Instant::now();
        self.prepared = false;
        self.rings.clear();
        if self.suppressed || self.draw_nm <= 0. {
            self.last_cache = CacheFrame::default();
            return;
        }
        self.last_cache = CacheFrame::default();
        let eye2 = [camera.position[0], camera.position[2]];
        let dt = self.last_prepare.elapsed().as_secs_f64().max(0.001);
        let ahead = self.last_eye.map_or([0.; 2], |last| {
            let delta = [eye2[0] - last[0], eye2[1] - last[1]];
            let distance = delta[0].hypot(delta[1]);
            // Five seconds at 480 kt is about 4,050 ft, within the guard ring.
            let scale = (5. / dt).min(4096. / distance.max(1.));
            let ahead = delta.map(|v| (v * scale / 512.).round() * 512.);
            let scale = 4096. / ahead[0].hypot(ahead[1]).max(4096.);
            ahead.map(|v| v * scale)
        });
        self.last_eye = Some(eye2);
        let full = self.detail_nm * vegetation::NM as f32;
        let draw = self.draw_nm * vegetation::NM as f32;
        let request = stream::Request {
            epoch: self.epoch,
            eye: eye2,
            ahead,
            full,
            draw,
            density: self.density,
            capacity: self.capacity,
            grass_radius: self.grass_radius,
            settle: self.settle,
        };
        if self.settle {
            self.pages.upload(usize::MAX, |offset, bytes| {
                queue.write_buffer(&self.instances, offset, bytes)
            });
            if self.incoming.is_some() {
                self.ready = self.incoming.take();
            }
        }
        let worker = self.worker.get_or_insert_with(|| {
            stream::Worker::new(self.atlas.clone(), self.library.clone(), world)
        });
        // Normal frames never wait. Captures explicitly request a settled view.
        if self.settle && worker.busy {
            let _ = worker.poll(true);
        }
        if self.settle {
            worker.request(request);
        }
        if let Some(ready) = worker.poll(self.settle)
            && ready.epoch == self.epoch
        {
            self.last_cache = ready.cache;
            self.pages.sync(&ready.chunks);
            self.incoming = Some(ready);
        }
        self.pages.upload(
            if self.settle {
                usize::MAX
            } else {
                stream::UPLOAD_BYTES
            },
            |offset, bytes| {
                queue.write_buffer(&self.instances, offset, bytes);
                self.last_cache.uploaded_bytes += bytes.len();
            },
        );
        if !self.pages.pending() && (self.incoming.is_some() || self.settle) {
            if self.incoming.is_some() {
                self.ready = self.incoming.take();
            }
            let (table, count) = self.pages.table();
            self.count = count;
            self.page_count = (table.len() / 2) as u32;
            if !table.is_empty() {
                queue.write_buffer(&self.chunks, 0, &bytes_of_u32(&table));
            }
        }
        if !self.pages.pending() && !self.settle {
            worker.request(request);
        }
        self.last_cache.worker_pending = worker.busy || self.ready.as_ref().is_none_or(|r| !r.done);
        (self.last_cache.upload_chunks, self.last_cache.upload_bytes) = self.pages.remaining();
        self.last_cache.pending = self.pages.pending() || self.last_cache.worker_pending;
        // Uploaded coverage defines a circle, never individual patch visibility.
        // The outer fade stays wholly inside that circle, including during a
        // teleport or a worker delay. A new circle expands continuously.
        let available = self
            .ready
            .as_ref()
            .map_or(0., |r| {
                (r.available - (r.eye[0] - eye2[0]).hypot(r.eye[1] - eye2[1]) as f32).max(0.)
            })
            .min(draw);
        let dt = self.last_prepare.elapsed().as_secs_f32().min(0.1);
        self.last_prepare = std::time::Instant::now();
        let (reach, range_pending) =
            advance_visible_range(self.visible_reach, available, draw, dt, self.settle);
        let reach = if available >= full {
            reach.max(full)
        } else {
            0.
        };
        self.visible_reach = reach;
        self.last_cache.available = available;
        self.last_cache.visible = reach;
        self.last_cache.range_pending = range_pending;
        self.last_cache.pending |= range_pending;
        let draw = self.visible_reach;
        if self.mesh_dirty {
            self.upload_meshes(queue);
            queue.write_buffer(&self.args, 0, &self.cards.vertices().to_le_bytes());
            self.mesh_dirty = false;
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
            self.last_cache.submit_ms = started.elapsed().as_secs_f64() * 1000.;
            return;
        }
        let mesh_count = self.library.meshes.len();
        encoder.clear_buffer(&self.counts, 0, None);
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
        params.extend(u32s(&[self.count, mesh_count as u32, self.page_count, 0]));
        queue.write_buffer(&self.params, 0, &bytes_of_f32(&params));
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Vegetation cull"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.cull);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.dispatch_workgroups(self.page_count, 1, 1);
        pass.set_pipeline(&self.finish);
        pass.dispatch_workgroups((mesh_count as u32).div_ceil(64), 1, 1);
        drop(pass);
        self.prepared = true;
        self.last_cache.submit_ms = started.elapsed().as_secs_f64() * 1000.;
    }

    fn upload_meshes(&self, queue: &wgpu::Queue) {
        let lib = &self.library;
        let mesh_count = lib.meshes.len();
        let mut table = Vec::with_capacity(mesh_count * 12);
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
                f32::from_bits(self.capacity as u32),
            ]);
            // Negative page codes carry a generated atlas cell without enlarging
            // the instance or mesh record. Retail still selects four views.
            let page = mesh.sprite_tile.map_or(mesh.sprite_layer as f32, |tile| {
                -((mesh.sprite_layer * 4 + usize::from(tile) + 1) as f32)
            });
            table.extend([mean[0], mean[1], mean[2], page]);
            table.extend(mesh.top);
        }
        if !table.is_empty() {
            queue.write_buffer(&self.meshes, 0, &bytes_of_f32(&table));
        }
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
        let radius = self.grass_radius;
        let Some(ready) = &self.ready else {
            return;
        };
        let patches = &ready.grass;
        let [fx, rx, ux] = basis;
        let mut rings: Vec<Vec<&Patch>> = vec![Vec::new(); RINGS.len()];
        for p in patches {
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
            pass.set_pipeline(&self.depth_pipeline);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_vertex_buffer(1, self.drawn.slice(..));
            pass.draw_indirect(&self.args, 0);
            pass.set_pipeline(&self.pipeline);
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

/// Readiness includes the fade circle reaching the requested radius. Otherwise
/// a fast worker could end benchmark warmup while the visible forest still grows.
fn advance_visible_range(
    previous: f32,
    available: f32,
    requested: f32,
    dt: f32,
    settle: bool,
) -> (f32, bool) {
    let target = available.min(requested);
    let reach = if settle {
        target
    } else {
        (previous + dt.min(0.1) * 6000.).min(target)
    };
    (reach, reach + 1. < requested)
}

/// Objects kept around the camera at most, per density.
fn object_budget(density: crate::graphics::VegDensity) -> usize {
    density.object_budget()
}

/// The instance list and the two lists the cull pass writes.
fn object_buffers(
    device: &wgpu::Device,
    capacity: usize,
) -> (wgpu::Buffer, wgpu::Buffer, wgpu::Buffer) {
    let buffer = |label, count: usize, stride: u64, usage| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: count as u64 * stride,
            usage,
            mapped_at_creation: false,
        })
    };
    let listed = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX;
    (
        buffer(
            "Vegetation instances",
            (2 * capacity.div_ceil(stream::PAGE) + stream::SPARE_PAGES) * stream::PAGE,
            INST_STRIDE,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        ),
        buffer("Vegetation drawn", capacity, DRAWN_STRIDE, listed),
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
    buffers: [&wgpu::Buffer; 8],
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
) -> (
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
) {
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
                9 => Float32x4,
                10 => Float32x4
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
    let make = |label,
                vertex: &str,
                fragment: &str,
                buffers: &[wgpu::VertexBufferLayout],
                depth: bool,
                writes: bool| {
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
                    write_mask: if depth {
                        wgpu::ColorWrites::empty()
                    } else {
                        wgpu::ColorWrites::ALL
                    },
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: writes,
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
        make(
            "Vegetation",
            "veg_vertex",
            "veg_fragment",
            &object_buffers,
            false,
            false,
        ),
        make(
            "Grass",
            "grass_vertex",
            "grass_fragment",
            &grass_buffers,
            false,
            true,
        ),
        make(
            "Vegetation depth",
            "veg_vertex",
            "veg_depth",
            &object_buffers,
            true,
            true,
        ),
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
    #[test]
    fn warmup_waits_for_both_uploaded_range_and_visible_range() {
        let (mut reach, pending) = super::advance_visible_range(0., 24000., 24000., 0.016, false);
        assert!(pending);
        assert!(reach < 100.);
        for _ in 0..400 {
            reach = super::advance_visible_range(reach, 24000., 24000., 0.016, false).0;
        }
        assert_eq!(
            super::advance_visible_range(reach, 24000., 24000., 0.016, false),
            (24000., false)
        );
        assert_eq!(
            super::advance_visible_range(24000., 8000., 24000., 0.016, false),
            (8000., true)
        );
        assert_eq!(
            super::advance_visible_range(0., 30000., 24000., 0., true),
            (24000., false)
        );
    }

    #[test]
    fn card_stream_layout_matches_the_compute_output() {
        let module =
            wgpu::naga::front::wgsl::parse_str(include_str!("vegetation_cull.wgsl")).unwrap();
        for (name, bytes) in [
            ("Inst", super::INST_STRIDE as u32),
            ("Drawn", super::DRAWN_STRIDE as u32),
            ("Mesh", 48),
        ] {
            let ty = module
                .types
                .iter()
                .find(|(_, ty)| ty.name.as_deref() == Some(name))
                .unwrap()
                .1;
            let wgpu::naga::TypeInner::Struct { span, .. } = ty.inner else {
                panic!("expected struct")
            };
            assert_eq!(span, bytes, "{name}");
        }
    }

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
