//! `--vegetation-preview OUT`: offscreen before/after pictures of the
//! exploratory vegetation. No window. 2560 by 1440, four samples.

use crate::{AppResult, camera::Camera, scenery::Scenery, terrain::Terrain};
use std::{path::Path, time::Instant};

const WIDTH: u32 = 2560;
const HEIGHT: u32 = 1440;

struct Shot {
    name: &'static str,
    spot: &'static str,
    height: f64,
    pitch: f64,
}

pub fn run() -> AppResult<()> {
    let out = std::env::args()
        .nth(2)
        .ok_or("--vegetation-preview needs an output directory")?;
    let out = Path::new(&out);
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    // Day is condition 0 at noon. Dusk is condition 4 (sunset) at 18:30,
    // so the palette is the game's dusk rather than noon colours.
    let jobs = [
        ("UKR", 0usize, [12i32, 0], "day"),
        ("EGY", 0, [12, 0], "day"),
        ("CUB", 0, [12, 0], "woods"),
        ("UKR", 4, [18, 30], "dusk"),
    ];
    for (code, condition, time, light) in jobs {
        println!("vegetation preview: building {code} {light}");
        let world = Terrain::for_mission(resources, code, Some(condition), &overrides(time)?)?;
        let scenery = Scenery::build(resources, &world)?;
        let atlas = scenery
            .vegetation
            .as_ref()
            .ok_or("vegetation atlas missing (TORE_VEGETATION=0)")?;
        let found = crate::vegetation::spots(atlas, &world);
        for spot in &found {
            println!("  spot {} at {:.0}, {:.0}", spot.label, spot.x, spot.z);
        }
        let mut gpu = pollster::block_on(View::new(&scenery))?;
        for shot in shots(light) {
            let Some(spot) = found.iter().find(|s| s.label == shot.spot) else {
                println!("  skip {} (no {} spot)", shot.name, shot.spot);
                continue;
            };
            let camera = look_at(
                &world,
                spot.x,
                spot.z,
                shot.height,
                shot.pitch,
                spot.heading,
            );
            gpu.set_shown(false);
            let off_ms = gpu.time(&camera, &world, &scenery)?;
            let before = gpu.pixels(&camera, &world, &scenery)?;
            gpu.set_shown(true);
            let on_ms = gpu.time(&camera, &world, &scenery)?;
            let after = gpu.pixels(&camera, &world, &scenery)?;
            let placed = gpu
                .sim
                .vegetation
                .as_ref()
                .map(crate::vegetation_renderer::Renderer::placed)
                .unwrap_or(0);
            let stem = format!("{code}-{light}-{}-{}ft", shot.name, shot.height as u32);
            write_png(out, &format!("{stem}-before.png"), &before)?;
            write_png(out, &format!("{stem}-after.png"), &after)?;
            println!("  {stem}: before {off_ms:.2} ms, after {on_ms:.2} ms, placed {placed}");
        }
    }
    Ok(())
}

fn shots(light: &str) -> Vec<Shot> {
    if light == "dusk" {
        return vec![Shot {
            name: "open",
            spot: "open",
            height: 500.,
            pitch: 12.,
        }];
    }
    if light == "woods" {
        return vec![Shot {
            name: "woods",
            spot: "edge",
            height: 80.,
            pitch: 5.,
        }];
    }
    vec![
        Shot {
            name: "open",
            spot: "open",
            height: 500.,
            pitch: 12.,
        },
        Shot {
            name: "open",
            spot: "open",
            height: 2_000.,
            pitch: 18.,
        },
        Shot {
            name: "open",
            spot: "open",
            height: 10_000.,
            pitch: 40.,
        },
        Shot {
            name: "river",
            spot: "river",
            height: 200.,
            pitch: 6.,
        },
        Shot {
            name: "edge",
            spot: "edge",
            height: 80.,
            pitch: 4.,
        },
    ]
}

fn overrides(time: [i32; 2]) -> AppResult<crate::terrain::Overrides> {
    Ok(crate::terrain::Overrides {
        time: Some(time),
        wind: None,
        cloud_altitude: Some(0),
        redrawn_airports: crate::scenery::redrawn_airports()?,
        city_fill: crate::scenery::city_fill()?,
        city_fill_cap: crate::scenery::city_fill_cap()?,
    })
}

fn look_at(world: &Terrain, x: f64, z: f64, height: f64, pitch_deg: f64, heading: f64) -> Camera {
    let pitch = pitch_deg.to_radians();
    let yaw = heading.to_radians();
    let ground = f64::from(world.height(x as f32, z as f32));
    let back = height / pitch.tan().abs().max(1e-3);
    let mut camera = Camera::new();
    camera.yaw = yaw as f32;
    camera.pitch = -pitch as f32;
    camera.position = [x - back * yaw.sin(), ground + height, z - back * yaw.cos()];
    camera
}

fn write_png(dir: &Path, name: &str, rgba: &[u8]) -> AppResult<()> {
    let bytes = crate::replay::png::encode_rgba(WIDTH, HEIGHT, rgba)?;
    std::fs::write(dir.join(name), bytes)?;
    Ok(())
}

struct View {
    device: wgpu::Device,
    queue: wgpu::Queue,
    sim: crate::sim_renderer::SimRenderer,
    texture: wgpu::Texture,
    buffer: wgpu::Buffer,
}

impl View {
    async fn new(scenery: &Scenery) -> AppResult<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;
        println!("vegetation preview adapter: {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let sim = crate::sim_renderer::SimRenderer::new(
            &device,
            &queue,
            format,
            scenery,
            crate::graphics::Options::default(),
            4,
        );
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Vegetation preview"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vegetation preview readback"),
            size: u64::from(WIDTH) * u64::from(HEIGHT) * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Ok(Self {
            device,
            queue,
            sim,
            texture,
            buffer,
        })
    }

    fn set_shown(&mut self, on: bool) {
        let mut options = crate::graphics::Options::default();
        if !on {
            options.ground_texture = crate::graphics::GroundTexture::Off;
        }
        self.sim.set_graphics(&self.device, options, 4);
        if let Some(vegetation) = &mut self.sim.vegetation {
            vegetation.set_suppressed(!on);
        }
    }

    fn draw(&mut self, camera: &Camera, world: &Terrain, scenery: &Scenery) {
        let view = self.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.sim.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            [WIDTH, HEIGHT],
            camera,
            world,
            scenery,
        );
        self.queue.submit([encoder.finish()]);
    }

    fn wait(&self) -> AppResult<()> {
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })?;
        Ok(())
    }

    /// Mean of four frames after a warmup, from submit through the wait.
    /// The readback is not included.
    fn time(&mut self, camera: &Camera, world: &Terrain, scenery: &Scenery) -> AppResult<f64> {
        self.draw(camera, world, scenery);
        self.wait()?;
        let start = Instant::now();
        const FRAMES: u32 = 4;
        for _ in 0..FRAMES {
            self.draw(camera, world, scenery);
        }
        self.wait()?;
        Ok(start.elapsed().as_secs_f64() * 1_000. / f64::from(FRAMES))
    }

    fn pixels(
        &mut self,
        camera: &Camera,
        world: &Terrain,
        scenery: &Scenery,
    ) -> AppResult<Vec<u8>> {
        let view = self.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.sim.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            [WIDTH, HEIGHT],
            camera,
            world,
            scenery,
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH * 4),
                    rows_per_image: Some(HEIGHT),
                },
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.wait()?;
        rx.recv()??;
        let mapped = self.buffer.slice(..).get_mapped_range();
        let pixels = mapped.to_vec();
        drop(mapped);
        self.buffer.unmap();
        Ok(pixels)
    }
}
