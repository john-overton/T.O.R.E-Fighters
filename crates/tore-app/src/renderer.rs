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
pub struct Renderer {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    sim: crate::sim_renderer::SimRenderer,
}
impl Renderer {
    pub fn set_world(&mut self, world: &crate::terrain::World) {
        self.sim = crate::sim_renderer::SimRenderer::new(
            &self.device,
            &self.queue,
            self.config.format,
            world,
            self.config.width,
            self.config.height,
        );
    }

    pub async fn new(window: Arc<Window>, world: &crate::terrain::World) -> AppResult<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;
        let info = adapter.get_info();
        println!(
            "Renderer: {} ({:?}, {:?})",
            info.name, info.backend, info.device_type
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;
        let size = window.inner_size();
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface has no configuration")?;
        surface.configure(&device, &config);
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
            mag_filter: wgpu::FilterMode::Linear,
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
            config.width,
            config.height,
        );
        Ok(Self {
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
    pub fn capture_sim(
        &mut self,
        path: &std::path::Path,
        camera: &crate::terrain::Camera,
        world: &crate::terrain::World,
    ) -> AppResult<()> {
        use std::io::Write;
        let (width, height) = (960u32, 720u32);
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
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })?;
        rx.recv()??;
        let data = buffer.slice(..).get_mapped_range();
        let mut file = std::fs::File::create(path)?;
        write!(file, "P6\n{width} {height}\n255\n")?;
        let bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        for row in data.chunks_exact(stride as usize) {
            for pixel in row[..width as usize * 4].chunks_exact(4) {
                file.write_all(&if bgra {
                    [pixel[2], pixel[1], pixel[0]]
                } else {
                    [pixel[0], pixel[1], pixel[2]]
                })?;
            }
        }
        drop(data);
        buffer.unmap();
        println!("Terrain capture: {}", path.display());
        Ok(())
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
    ) -> AppResult<bool> {
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
                bytes_per_row: Some(WIDTH as u32 * 4),
                rows_per_image: Some(HEIGHT as u32),
            },
            wgpu::Extent3d {
                width: WIDTH as u32,
                height: HEIGHT as u32,
                depth_or_array_layers: 1,
            },
        );
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
            let v = self.viewport();
            pass.set_viewport(v.x, v.y, v.width, v.height, 0.0, 1.0);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        frame.present();
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
