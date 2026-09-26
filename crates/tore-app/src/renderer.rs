use crate::{
    AppResult,
    menu::{HEIGHT, WIDTH},
};
use std::sync::Arc;
use winit::window::Window;

#[derive(Clone, Copy)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Viewport {
    pub fn new(width: u32, height: u32) -> Self {
        let scale = (width as f32 / WIDTH as f32).min(height as f32 / HEIGHT as f32);
        let (w, h) = (WIDTH as f32 * scale, HEIGHT as f32 * scale);
        Self {
            x: (width as f32 - w) / 2.0,
            y: (height as f32 - h) / 2.0,
            width: w,
            height: h,
        }
    }
    pub fn point(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        if self.width <= 0.0
            || self.height <= 0.0
            || x < self.x as f64
            || y < self.y as f64
            || x >= (self.x + self.width) as f64
            || y >= (self.y + self.height) as f64
        {
            return None;
        }
        Some((
            (x - self.x as f64) * WIDTH as f64 / self.width as f64,
            (y - self.y as f64) * HEIGHT as f64 / self.height as f64,
        ))
    }
}
struct Readback {
    buffer: wgpu::Buffer,
    receiver: std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    width: u32,
    height: u32,
    stride: u32,
    bgra: bool,
}
impl Readback {
    fn pixels(self) -> Vec<u8> {
        let data = self.buffer.slice(..).get_mapped_range();
        let mut pixels = Vec::with_capacity((self.width * self.height * 4) as usize);
        for row in data.chunks_exact(self.stride as usize) {
            for pixel in row[..self.width as usize * 4].chunks_exact(4) {
                pixels.extend_from_slice(&if self.bgra {
                    [pixel[2], pixel[1], pixel[0], pixel[3]]
                } else {
                    [pixel[0], pixel[1], pixel[2], pixel[3]]
                });
            }
        }
        drop(data);
        self.buffer.unmap();
        pixels
    }
}
pub struct Renderer {
    previews: std::collections::BTreeMap<u8, Readback>,
    first_frame: crate::startup::FirstFrame,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    sim: crate::sim_renderer::SimRenderer,
    cockpit: crate::cockpit_renderer::CockpitRenderer,
    mirror_camera: crate::terrain::Camera,
    mirror_vertices: Vec<f32>,
    pub mirror_frames: u64,
    mirrors_enabled: bool,
    graphics: crate::graphics::Options,
    /// MSAA sample counts the adapter supports for the surface and depth.
    sample_counts: Vec<u32>,
    // Fields drop in declaration order; keep the window alive through GPU cleanup.
    pub window: Arc<Window>,
}
impl Renderer {
    pub fn prepare_aircraft(&mut self, hornet: &crate::aircraft::Airframe) {
        self.previews.clear();
        self.mirror_vertices.clear();
        self.sim.clear_aircraft();
        self.sim.aircraft(&self.device, &self.queue, hornet, &[]);
        self.cockpit.prepare(
            &self.device,
            &self.queue,
            &hornet.sprites[hornet.profile.id.cockpit()],
            &hornet.cockpit_pic,
            hornet.profile.id,
        );
    }
    #[allow(clippy::too_many_arguments)]
    pub fn cockpit(
        &mut self,
        state: &crate::flight::State,
        camera: &crate::terrain::Camera,
        art: bool,
        hud: bool,
        pixels: &[u8],
        colors: &[[u8; 3]; 256],
    ) {
        self.cockpit.weather(&self.queue, colors);
        self.cockpit.update(
            &self.queue,
            self.flight_size(),
            state,
            camera,
            art,
            hud,
            pixels,
        );
    }
    pub fn aircraft(
        &mut self,
        hornet: &crate::aircraft::Airframe,
        state: &crate::flight::State,
        visible: bool,
        camera: &crate::terrain::Camera,
        world: &crate::terrain::World,
    ) {
        self.mirror_camera = crate::mirrors::camera(state);
        if !visible {
            self.mirror_vertices = hornet.vertices(state, &self.mirror_camera, world);
        }
        self.sim.aircraft(
            &self.device,
            &self.queue,
            hornet,
            &hornet.vertices(state, camera, world),
        );
        if !visible {
            self.sim.hide_aircraft();
        }
    }
    pub fn dummies(
        &mut self,
        geometry: Vec<(
            &crate::aircraft::Airframe,
            Vec<f32>,
            Vec<crate::sim_renderer::Contact>,
        )>,
    ) {
        self.sim.dummies(&self.device, &self.queue, geometry);
    }
    pub fn combat(&mut self, geometry: &crate::sim_renderer::CombatGeometry) {
        self.sim.combat(&self.device, &self.queue, geometry);
    }
    pub fn escapees(&mut self, art: &crate::ejection_art::Art, vertices: &[f32]) {
        self.sim.escapees(&self.device, &self.queue, art, vertices);
    }
    pub fn airports(&mut self, vertices: &[f32], lines: &[f32]) {
        self.sim.airports(&self.device, &self.queue, vertices);
        self.sim.airport_lines(&self.device, &self.queue, lines);
    }
    /// Smoke puffs, flare smoke and the released chaff and flares.
    pub fn smoke(
        &mut self,
        art: &tore_formats::Pic,
        smoke: [&tore_sim::combat::smoke::Smoke; 2],
        devices: &tore_sim::combat::countermeasures::Devices,
    ) {
        self.sim
            .smoke(&self.device, &self.queue, art, smoke, devices);
    }
    pub fn vapor(&mut self, vertices: &[f32]) {
        self.sim.vapor(&self.device, &self.queue, vertices);
    }
    pub fn set_world(&mut self, world: &crate::terrain::World) {
        self.sim = crate::sim_renderer::SimRenderer::new(
            &self.device,
            &self.queue,
            self.config.format,
            world,
            self.graphics,
            self.samples(self.graphics.anti_aliasing),
        );
    }
    pub fn graphics(&self) -> crate::graphics::Options {
        self.graphics
    }
    /// Apply graphics choices at once; an anti-aliasing change rebuilds the
    /// world pipelines.
    pub fn set_graphics(&mut self, options: crate::graphics::Options) {
        self.graphics = options;
        let samples = self.samples(options.anti_aliasing);
        self.sim.set_graphics(&self.device, options, samples);
    }
    /// Whether this adapter can draw the given anti-aliasing level exactly.
    pub fn supports(&self, level: crate::graphics::AntiAliasing) -> bool {
        self.sample_counts.contains(&level.samples())
    }
    /// The highest supported sample count not above the requested one.
    fn samples(&self, level: crate::graphics::AntiAliasing) -> u32 {
        self.sample_counts
            .iter()
            .copied()
            .filter(|&n| n <= level.samples())
            .max()
            .unwrap_or(1)
    }

    pub async fn new(
        window: Arc<Window>,
        world: &crate::terrain::World,
        graphics: crate::graphics::Options,
    ) -> AppResult<Self> {
        crate::diagnostics::stage("game graphics instance and surface");
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone())?;
        crate::diagnostics::stage_done();
        crate::diagnostics::stage("game graphics adapter selection");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;
        crate::diagnostics::stage_done();
        log::info!("game adapter: {:?}", adapter.get_info());
        crate::diagnostics::stage("game graphics device creation");
        let info = adapter.get_info();
        log::info!(
            "Renderer: {} ({:?}, {:?})",
            info.name,
            info.backend,
            info.device_type
        );
        // Sample counts beyond 1 and 4 are adapter specific; opt in when
        // offered so 2x and 8x anti-aliasing can be used.
        let specific =
            adapter.features() & wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: specific,
                ..Default::default()
            })
            .await?;
        crate::diagnostics::stage_done();
        device.set_device_lost_callback(|reason, message| {
            if reason != wgpu::DeviceLostReason::Destroyed {
                log::error!("game graphics device lost: {reason:?}: {message}");
            }
        });
        crate::diagnostics::stage("game graphics resources and pipelines");
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface has no configuration")?;
        let modes = surface.get_capabilities(&adapter).present_modes;
        config.present_mode = if modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else if modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        config.desired_maximum_frame_latency = 1;
        log::info!(
            "Presentation: {:?}, maximum queued frames: {}",
            config.present_mode,
            config.desired_maximum_frame_latency
        );
        surface.configure(&device, &config);
        let sample_counts: Vec<u32> = [1, 2, 4, 8]
            .into_iter()
            .filter(|&n| {
                if specific.is_empty() {
                    return matches!(n, 1 | 4);
                }
                [config.format, wgpu::TextureFormat::Depth32Float]
                    .iter()
                    .all(|&f| {
                        adapter
                            .get_texture_format_features(f)
                            .flags
                            .sample_count_supported(n)
                    })
            })
            .collect();
        let samples = sample_counts
            .iter()
            .copied()
            .filter(|&n| n <= graphics.anti_aliasing.samples())
            .max()
            .unwrap_or(1);
        log::info!("Anti-aliasing: {samples}x (supported {sample_counts:?})");
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Retail menu canvas"),
            size: wgpu::Extent3d {
                width: WIDTH as u32,
                height: HEIGHT as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Menu compositor"),
            source: wgpu::ShaderSource::Wgsl(include_str!("menu.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Menu"),
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
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Menu image"),
            layout: &pipeline.get_bind_group_layout(0),
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
        let sim = crate::sim_renderer::SimRenderer::new(
            &device,
            &queue,
            config.format,
            world,
            graphics,
            samples,
        );
        let cockpit = crate::cockpit_renderer::CockpitRenderer::new(&device, config.format);
        crate::diagnostics::stage_done();
        Ok(Self {
            first_frame: Default::default(),
            mirror_camera: crate::terrain::Camera::new(),
            mirror_vertices: Vec::new(),
            mirror_frames: 0,
            mirrors_enabled: std::env::var("TORE_MIRRORS").as_deref() != Ok("0"),
            graphics,
            sample_counts,
            cockpit,
            previews: Default::default(),
            sim,
            window,
            surface,
            device,
            queue,
            config,
            texture,
            bind_group,
            pipeline,
        })
    }
    /// Submit before the primary pass writes the shared camera/vertex buffers.
    /// GPU-only render-to-texture, every visible frame, with no rate timer/readback.
    fn render_mirrors(&mut self, world: &crate::terrain::World) {
        if !self.mirrors_enabled || !self.cockpit.mirrors_visible {
            return;
        }
        self.sim
            .update_aircraft_vertices(&self.queue, &self.mirror_vertices);
        let view = self.cockpit.mirror_target.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.sim.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            crate::mirrors::SIZE,
            &self.mirror_camera,
            world,
        );
        self.queue.submit([encoder.finish()]);
        self.sim.hide_aircraft();
        self.mirror_frames += 1;
    }
    pub fn capture_sim(
        &mut self,
        path: &std::path::Path,
        camera: &crate::terrain::Camera,
        world: &crate::terrain::World,
        overlay: bool,
    ) -> AppResult<()> {
        use std::io::Write;
        let [width, height] = if overlay {
            self.flight_size()
        } else {
            [960, 720]
        };
        let pixels = self.scene_pixels(camera, world, width, height, overlay)?;
        let mut file = std::fs::File::create(path)?;
        write!(file, "P6\n{width} {height}\n255\n")?;
        for p in pixels.chunks_exact(4) {
            file.write_all(&p[..3])?;
        }
        println!("Scene capture: {}", path.display());
        Ok(())
    }
    pub fn scene_pixels(
        &mut self,
        camera: &crate::terrain::Camera,
        world: &crate::terrain::World,
        width: u32,
        height: u32,
        overlay: bool,
    ) -> AppResult<Vec<u8>> {
        let pending = self.submit_readback(camera, world, width, height, overlay)?;
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })?;
        pending.receiver.recv()??;
        Ok(pending.pixels())
    }
    /// Live panels consume a previous completed frame; never wait for GPU completion.
    pub fn poll_previews(&mut self) -> AppResult<Vec<(u8, Vec<u8>)>> {
        self.device.poll(wgpu::PollType::Poll)?;
        let mut ready = Vec::new();
        for (&page, pending) in &self.previews {
            match pending.receiver.try_recv() {
                Ok(result) => {
                    result?;
                    ready.push(page);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(ready
            .into_iter()
            .map(|page| (page, self.previews.remove(&page).unwrap().pixels()))
            .collect())
    }
    pub fn request_preview(
        &mut self,
        page: u8,
        camera: &crate::terrain::Camera,
        world: &crate::terrain::World,
    ) -> AppResult<bool> {
        if !matches!(page, 2..=4) {
            return Err("invalid camera instrument".into());
        }
        if !self.previews.contains_key(&page) {
            let pending = self.submit_readback(camera, world, 138, 114, false)?;
            self.previews.insert(page, pending);
            return Ok(true);
        }
        Ok(false)
    }
    fn submit_readback(
        &mut self,
        camera: &crate::terrain::Camera,
        world: &crate::terrain::World,
        width: u32,
        height: u32,
        overlay: bool,
    ) -> AppResult<Readback> {
        if width == 0 || height == 0 || width > 1920 || height > 1080 {
            return Err("capture dimensions outside bounds".into());
        }

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Terrain validation capture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        if overlay {
            self.render_mirrors(world);
        }
        let view = texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.sim.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            [width, height],
            camera,
            world,
        );
        if overlay {
            self.cockpit.draw(&mut encoder, &view);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Flight UI capture"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        let stride = (width * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Terrain capture readback"),
            size: (stride * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
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
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        Ok(Readback {
            buffer,
            receiver: rx,
            width,
            height,
            stride,
            bgra: matches!(
                self.config.format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            ),
        })
    }
    pub fn flight_size(&self) -> [u32; 2] {
        let s = self.window.inner_size();
        let scale = (1920. / s.width.max(1) as f64)
            .min(1080. / s.height.max(1) as f64)
            .min(1.);
        [
            (s.width as f64 * scale).round().max(1.) as u32,
            (s.height as f64 * scale).round().max(1.) as u32,
        ]
    }
    fn canvas_texture(&mut self, size: [u32; 2]) {
        if self.texture.width() == size[0] && self.texture.height() == size[1] {
            return;
        }
        self.texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Responsive UI"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = self.texture.create_view(&Default::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        self.bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Responsive UI"),
            layout: &self.pipeline.get_bind_group_layout(0),
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
    }
    pub fn viewport(&self) -> Viewport {
        let s = self.window.inner_size();
        Viewport::new(s.width, s.height)
    }
    pub fn resize(&mut self) {
        let s = self.window.inner_size();
        if s.width > 0 && s.height > 0 {
            self.config.width = s.width;
            self.config.height = s.height;
            self.surface.configure(&self.device, &self.config);
            self.window.request_redraw();
        }
    }
    pub fn draw(
        &mut self,
        pixels: &[u8],
        scene: Option<(&crate::terrain::Camera, &crate::terrain::World)>,
        flight_size: Option<[u32; 2]>,
    ) -> AppResult<bool> {
        self.first_frame.begin("game first frame presentation");
        let s = self.window.inner_size();
        if s.width == 0 || s.height == 0 {
            return Ok(false);
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.resize();
                return Ok(false);
            }
            Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other) => {
                self.window.request_redraw();
                return Ok(false);
            }
            Err(error) => return Err(error.into()),
        };
        let size = flight_size.unwrap_or([WIDTH as u32, HEIGHT as u32]);
        self.canvas_texture(size);
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size[0] * 4),
                rows_per_image: Some(size[1]),
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
        );
        if flight_size.is_some()
            && let Some((_, world)) = scene
        {
            self.render_mirrors(world);
        }
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        if let Some((camera, world)) = scene {
            self.sim.draw(
                &self.device,
                &self.queue,
                &mut encoder,
                &view,
                [s.width, s.height],
                camera,
                world,
            );
        }
        if flight_size.is_some() {
            self.cockpit.draw(&mut encoder, &view);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main menu"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if scene.is_some() {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            if flight_size.is_none() {
                let v = self.viewport();
                pass.set_viewport(v.x, v.y, v.width, v.height, 0., 1.);
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        // Wayland frame callbacks throttle redraw requests to compositor refresh.
        // Only request that pacing when the surface must fall back to FIFO.
        if self.config.present_mode == wgpu::PresentMode::Fifo {
            self.window.pre_present_notify();
        }
        frame.present();
        self.first_frame.complete();
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn letterbox_coordinates_match_rendered_canvas() {
        let v = Viewport::new(1600, 900);
        assert_eq!(v.point(200.0, 0.0), Some((0.0, 0.0)));
        assert_eq!(v.point(800.0, 450.0), Some((320.0, 240.0)));
        assert_eq!(v.point(199.0, 200.0), None);
        assert_eq!(v.point(1400.0, 200.0), None);
        assert_eq!(Viewport::new(0, 0).point(0.0, 0.0), None);
    }
}
