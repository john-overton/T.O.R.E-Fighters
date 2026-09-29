//! Original explosion, fire and crater artwork as textured sprites. The
//! simulation owns every effect's type, place and life; the sheets, frame
//! layouts and sizes are in docs/spec/explosions.md.
use crate::camera::Camera;
use crate::snapshot::{EffectPose, MarkPose};
use std::collections::BTreeMap;
use tore_formats::Pic;
use tore_sim::combat::{
    blast::{self, MarkKind},
    live::EffectKind,
};

/// Explosions, fires and craters drawn at once.
const MAX_INSTANCES: usize = 4096;
/// Center relative to the eye, extent, mode, art cell, layer, opacity and
/// emissive flag.
const INSTANCE_BYTES: usize = 13 * 4;
/// Every sheet is 256 wide; the tallest, GRNDMED3, is 386 rows.
const SHEET_WIDTH: usize = 256;
const SHEET_HEIGHT: usize = 386;

/// One animation sheet's frame layout as its shape describes it: the
/// frame size, the first frame's corner, the gap between frames, columns
/// and frame count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Layout {
    width: u16,
    height: u16,
    x: u16,
    y: u16,
    gap_x: u16,
    gap_y: u16,
    columns: u16,
    frames: u16,
}
const fn layout(size: [u16; 8]) -> Layout {
    Layout {
        width: size[0],
        height: size[1],
        x: size[2],
        y: size[3],
        gap_x: size[4],
        gap_y: size[5],
        columns: size[6],
        frames: size[7],
    }
}
impl Layout {
    /// Pixel rectangle of one frame: corner, then size.
    fn cell(self, frame: u16) -> [f32; 4] {
        let frame = frame.min(self.frames - 1);
        let (column, row) = (frame % self.columns, frame / self.columns);
        [
            f32::from(self.x + column * (self.width + self.gap_x)),
            f32::from(self.y + row * (self.height + self.gap_y)),
            f32::from(self.width),
            f32::from(self.height),
        ]
    }
}

/// EXP.SH's sheets in its own order, then FIRE.SH's and CRATER.SH's.
const SHEETS: [(&str, Layout); 25] = [
    ("AIRSML.PIC", layout([78, 50, 1, 0, 2, 2, 3, 12])),
    ("AIRMED.PIC", layout([78, 64, 1, 0, 2, 2, 3, 12])),
    ("AIRMED2.PIC", layout([78, 64, 1, 0, 2, 2, 3, 12])),
    ("AIRMED3.PIC", layout([68, 58, 1, 0, 2, 2, 3, 15])),
    ("AIRLRG.PIC", layout([78, 56, 1, 0, 2, 2, 3, 12])),
    ("GRNDSML.PIC", layout([78, 64, 1, 0, 2, 2, 3, 12])),
    ("GRNDMED.PIC", layout([78, 64, 1, 0, 2, 2, 3, 12])),
    ("GRNDMED3.PIC", layout([67, 75, 1, 0, 2, 2, 3, 15])),
    ("GRNDLRG.PIC", layout([62, 60, 1, 0, 2, 2, 3, 15])),
    ("GRNDLRG2.PIC", layout([78, 64, 1, 0, 2, 2, 3, 12])),
    ("WATSML.PIC", layout([35, 59, 1, 0, 2, 2, 3, 6])),
    ("WATLRG.PIC", layout([78, 64, 1, 0, 2, 2, 3, 12])),
    ("FLAKA.PIC", layout([56, 47, 8, 8, 1, 1, 4, 28])),
    ("FLAKB.PIC", layout([76, 62, 4, 0, 1, 1, 3, 12])),
    ("FLAKC.PIC", layout([76, 62, 4, 0, 1, 1, 3, 12])),
    ("EMPEX.PIC", layout([78, 64, 1, 0, 2, 2, 3, 12])),
    ("AIRLRGAG.PIC", layout([76, 62, 4, 0, 1, 1, 3, 12])),
    ("AIRLRGC.PIC", layout([76, 62, 4, 0, 1, 1, 3, 12])),
    ("AIRLRGD.PIC", layout([76, 62, 4, 0, 1, 1, 3, 12])),
    ("AIRSMLA.PIC", layout([76, 62, 4, 0, 1, 1, 3, 12])),
    ("AIRSMLB2.PIC", layout([76, 62, 1, 0, 1, 1, 3, 12])),
    ("GRDLRGA.PIC", layout([76, 62, 4, 0, 1, 1, 3, 12])),
    ("DIRTEXP.PIC", layout([76, 62, 4, 0, 1, 1, 3, 12])),
    ("FIREA.PIC", layout([76, 62, 4, 0, 1, 1, 3, 15])),
    ("CRATERS.PIC", layout([78, 66, 1, 0, 2, 0, 3, 3])),
];
const FIRE_SHEET: usize = 23;
const CRATER_SHEET: usize = 24;
const DEBRIS_SHEET: usize = 21;

/// The sheet EXP.SH draws for each explosion type, 15 to 38.
const EXPLOSION_SHEETS: [usize; 24] = [
    5, 22, 10, 0, 19, 20, 6, 6, 7, 1, 2, 3, 12, 13, 14, 4, 16, 17, 18, 11, 8, 9, 21, 15,
];

fn explosion_sheet(kind: u8) -> Option<usize> {
    EXPLOSION_SHEETS
        .get(usize::from(kind.checked_sub(blast::FIRST)?))
        .copied()
}

/// The imported sheets, loaded once. A sheet the cache lacks draws nothing.
pub struct Art {
    layers: Vec<u8>,
    present: [bool; SHEETS.len()],
}
impl Art {
    pub fn load(data: &BTreeMap<String, Vec<u8>>) -> Self {
        let mut layers = vec![255; SHEETS.len() * SHEET_WIDTH * SHEET_HEIGHT];
        let mut present = [false; SHEETS.len()];
        for (index, (name, _)) in SHEETS.iter().enumerate() {
            let Some(pic) = data.get(*name).and_then(|bytes| Pic::parse(bytes).ok()) else {
                log::warn!("Explosion art {name} unavailable; that effect is not drawn");
                continue;
            };
            if pic.width != SHEET_WIDTH || pic.height > SHEET_HEIGHT || !pic.palette.is_empty() {
                log::warn!("Explosion art {name} has an unreviewed layout");
                continue;
            }
            let layer = &mut layers[index * SHEET_WIDTH * SHEET_HEIGHT..];
            for (i, (&pixel, &visible)) in pic.pixels.iter().zip(&pic.mask).enumerate() {
                layer[i] = if visible { pixel } else { 255 };
            }
            present[index] = true;
        }
        Self { layers, present }
    }
    /// No sheets: nothing is drawn.
    #[cfg(test)]
    pub fn empty() -> Self {
        Self {
            layers: vec![255; SHEETS.len() * SHEET_WIDTH * SHEET_HEIGHT],
            present: [false; SHEETS.len()],
        }
    }
    #[cfg(test)]
    fn synthetic() -> Self {
        Self {
            layers: vec![7; SHEETS.len() * SHEET_WIDTH * SHEET_HEIGHT],
            present: [true; SHEETS.len()],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Faces the camera, centered on its point.
    Billboard = 0,
    /// Faces the camera with its base on its point.
    Standing = 1,
    /// Lies flat on the ground.
    Flat = 2,
}

/// One sprite to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Sprite {
    position: [f64; 3],
    /// Half width, then half height (or full height when standing).
    extent: [f64; 2],
    mode: Mode,
    cell: [f32; 4],
    layer: usize,
    opacity: f32,
    emissive: bool,
}
impl Sprite {
    fn radius(&self) -> f64 {
        self.extent[0].max(self.extent[1]) * 2.
    }
}

/// Legacy recordings stored no explosion type: their kind's family, over
/// its old 45- or 240-tick life.
fn legacy(kind: EffectKind) -> Option<(u8, u16)> {
    match kind {
        EffectKind::Hit => Some((18, 45)),
        EffectKind::Ground => Some((15, 45)),
        EffectKind::Destroyed => Some((blast::AIRCRAFT, 240)),
        _ => None,
    }
}

/// Every explosion, crater and fire in one picture, craters first.
fn sprites(art: &Art, effects: &[EffectPose], marks: &[MarkPose]) -> Vec<Sprite> {
    let mut out = Vec::new();
    for mark in marks {
        let sprite = match mark.kind {
            MarkKind::Crater(size) => {
                let (_, layout) = SHEETS[CRATER_SHEET];
                let half = blast::crater_half_width(size);
                Sprite {
                    position: mark.position,
                    extent: [half, half],
                    mode: Mode::Flat,
                    cell: layout.cell(u16::from(blast::crater_style(mark.position))),
                    layer: CRATER_SHEET,
                    opacity: 1.,
                    emissive: false,
                }
            }
            MarkKind::Fire => {
                let (_, layout) = SHEETS[FIRE_SHEET];
                // FIRE.SH loops its 15 frames once a second.
                let frame = (mark.age % 120) * u64::from(layout.frames) / 120;
                let width = f64::from(blast::FIRE_SIZE);
                Sprite {
                    position: mark.position,
                    extent: [
                        width / 2.,
                        width * f64::from(layout.height) / f64::from(layout.width),
                    ],
                    mode: Mode::Standing,
                    cell: layout.cell(frame as u16),
                    layer: FIRE_SHEET,
                    opacity: mark.strength,
                    emissive: true,
                }
            }
        };
        if art.present[sprite.layer] {
            out.push(sprite);
        }
    }
    for effect in effects {
        let (kind, duration, width, sheet) = if effect.kind == EffectKind::DebrisImpact {
            (0, 45, 15., DEBRIS_SHEET)
        } else if let Some(kind) = effect.blast {
            let Some(row) = blast::explosion(kind) else {
                continue;
            };
            (
                kind,
                u16::from(row.seconds) * 120,
                f64::from(blast::rolled_size(kind, effect.position)),
                explosion_sheet(kind).unwrap_or(0),
            )
        } else if let Some((kind, duration)) = legacy(effect.kind) {
            (
                kind,
                duration,
                f64::from(blast::rolled_size(kind, effect.position)),
                explosion_sheet(kind).unwrap_or(0),
            )
        } else {
            continue;
        };
        if !art.present[sheet] || width <= 0. {
            continue;
        }
        let (_, layout) = SHEETS[sheet];
        let elapsed = duration.saturating_sub(effect.ticks);
        let frame = u32::from(elapsed) * u32::from(layout.frames) / u32::from(duration.max(1));
        let aspect = f64::from(layout.height) / f64::from(layout.width);
        let standing = blast::explosion(kind).is_some_and(|row| row.surface)
            || effect.kind == EffectKind::DebrisImpact;
        out.push(Sprite {
            position: effect.position,
            extent: if standing {
                [width / 2., width * aspect]
            } else {
                [width / 2., width * aspect / 2.]
            },
            mode: if standing {
                Mode::Standing
            } else {
                Mode::Billboard
            },
            cell: layout.cell(frame as u16),
            layer: sheet,
            opacity: 1.,
            emissive: true,
        });
    }
    out
}

pub struct EffectRenderer {
    pipeline: wgpu::RenderPipeline,
    bind: Option<wgpu::BindGroup>,
    buffer: wgpu::Buffer,
    sprites: Vec<Sprite>,
    count: u32,
}
impl EffectRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
        samples: u32,
    ) -> Self {
        Self {
            pipeline: Self::pipeline(device, format, shader, samples),
            bind: None,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Bounded explosion sprites"),
                size: (MAX_INSTANCES * INSTANCE_BYTES) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            sprites: Vec::new(),
            count: 0,
        }
    }
    /// Rebuild for a new anti-aliasing sample count; the next `prepare`
    /// recreates the bindings.
    pub fn set_samples(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
        samples: u32,
    ) {
        self.pipeline = Self::pipeline(device, format, shader, samples);
        self.bind = None;
    }
    fn pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
        samples: u32,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Original explosion, fire and crater sprites"),
            layout: None,
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("effect_vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: INSTANCE_BYTES as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3,
                        1 => Float32x2,
                        2 => Float32,
                        3 => Float32x4,
                        4 => Float32,
                        5 => Float32,
                        6 => Float32
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("effect_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Greater,
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
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniform: &wgpu::Buffer,
        weather: (&wgpu::Texture, &wgpu::TextureView),
        art: &Art,
        effects: &[EffectPose],
        marks: &[MarkPose],
    ) {
        if self.bind.is_none() {
            let size = wgpu::Extent3d {
                width: SHEET_WIDTH as u32,
                height: SHEET_HEIGHT as u32,
                depth_or_array_layers: SHEETS.len() as u32,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Runtime indexed explosion, fire and crater sheets"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                texture.as_image_copy(),
                &art.layers,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SHEET_WIDTH as u32),
                    rows_per_image: Some(SHEET_HEIGHT as u32),
                },
                size,
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let palette = weather.0.create_view(&Default::default());
            let material_storage = crate::sim_renderer::tile_layout(device, false);
            self.bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Explosion sheets and camera"),
                layout: &self.pipeline.get_bind_group_layout(0),
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
                        resource: wgpu::BindingResource::TextureView(&palette),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(weather.1),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: material_storage.as_entire_binding(),
                    },
                ],
            }));
        }
        self.sprites = sprites(art, effects, marks);
    }
    /// Sorts what the camera can see: craters first, then everything else
    /// far to near.
    pub fn update(&mut self, queue: &wgpu::Queue, camera: &Camera) {
        let eye = camera.position;
        let mut visible: Vec<_> = self
            .sprites
            .iter()
            .filter_map(|s| {
                let offset: [f64; 3] = std::array::from_fn(|i| s.position[i] - eye[i]);
                let distance: f64 = offset.iter().map(|v| v * v).sum::<f64>().sqrt();
                (distance - s.radius() < 2_200_000.).then_some((s.mode != Mode::Flat, distance, s))
            })
            .collect();
        visible.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
        let mut instances = Vec::with_capacity(visible.len().min(MAX_INSTANCES) * INSTANCE_BYTES);
        for (_, _, s) in visible.into_iter().take(MAX_INSTANCES) {
            // Relative to the eye in full precision, far from the origin too.
            let offset: [f32; 3] = std::array::from_fn(|i| (s.position[i] - eye[i]) as f32);
            let values = offset.into_iter().chain([
                s.extent[0] as f32,
                s.extent[1] as f32,
                s.mode as u8 as f32,
                s.cell[0],
                s.cell[1],
                s.cell[2],
                s.cell[3],
                s.layer as f32,
                s.opacity,
                if s.emissive { 1. } else { 0. },
            ]);
            for value in values {
                instances.extend(value.to_le_bytes());
            }
        }
        self.count = (instances.len() / INSTANCE_BYTES) as u32;
        if !instances.is_empty() {
            queue.write_buffer(&self.buffer, 0, &instances);
        }
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.count == 0 {
            return;
        }
        let Some(bind) = &self.bind else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(kind: EffectKind, blast: Option<u8>, ticks: u16) -> EffectPose {
        EffectPose {
            kind,
            position: [100., 0., 200.],
            ticks,
            blast,
        }
    }

    #[test]
    fn each_explosion_type_draws_the_sheet_exp_sh_names() {
        let name = |kind| SHEETS[explosion_sheet(kind).unwrap()].0;
        assert_eq!(name(15), "GRNDSML.PIC");
        assert_eq!(name(16), "DIRTEXP.PIC");
        assert_eq!(name(17), "WATSML.PIC");
        assert_eq!(name(18), "AIRSML.PIC");
        assert_eq!(name(22), "GRNDMED.PIC");
        assert_eq!(name(27), "FLAKA.PIC");
        assert_eq!(name(30), "AIRLRG.PIC");
        assert_eq!(name(34), "WATLRG.PIC");
        assert_eq!(name(35), "GRNDLRG.PIC");
        assert_eq!(name(37), "GRDLRGA.PIC");
        assert_eq!(name(38), "EMPEX.PIC");
        assert!(explosion_sheet(14).is_none() && explosion_sheet(39).is_none());
        // Every frame of every sheet lies inside the 256-wide texture.
        for (name, layout) in SHEETS {
            let last = layout.cell(layout.frames - 1);
            assert!(last[0] + last[2] <= 256., "{name}");
            assert!(last[1] + last[3] <= SHEET_HEIGHT as f32, "{name}");
        }
        assert_eq!(SHEETS[4].1.cell(4), [81., 58., 78., 56.]);
    }

    #[test]
    fn explosions_animate_over_their_life_and_sit_or_float_by_type() {
        let art = Art::synthetic();
        // Type 30: one second, twelve frames, floating in the air.
        let start = sprites(&art, &[effect(EffectKind::Destroyed, Some(30), 120)], &[]);
        let end = sprites(&art, &[effect(EffectKind::Destroyed, Some(30), 1)], &[]);
        assert_eq!(start[0].cell, SHEETS[4].1.cell(0));
        assert_eq!(end[0].cell, SHEETS[4].1.cell(11));
        assert_eq!(start[0].mode, Mode::Billboard);
        let width = f64::from(blast::rolled_size(30, [100., 0., 200.]));
        assert_eq!(start[0].extent[0], width / 2.);
        // Type 35 stands on the ground for two seconds.
        let ground = sprites(&art, &[effect(EffectKind::Ground, Some(35), 120)], &[]);
        assert_eq!(ground[0].mode, Mode::Standing);
        assert_eq!(ground[0].cell, SHEETS[8].1.cell(7));
        // Recordings without types draw their family; launches draw nothing.
        let old = sprites(
            &art,
            &[
                effect(EffectKind::Destroyed, None, 240),
                effect(EffectKind::Launch, None, 45),
            ],
            &[],
        );
        assert_eq!(old.len(), 1);
        assert_eq!(old[0].layer, 4);
        let debris = sprites(&art, &[effect(EffectKind::DebrisImpact, None, 45)], &[]);
        assert_eq!((debris[0].layer, debris[0].extent[0]), (DEBRIS_SHEET, 7.5));
    }

    #[test]
    fn craters_lie_flat_first_and_fires_loop_and_fade() {
        let art = Art::synthetic();
        let mark = |kind, age, strength| MarkPose {
            kind,
            position: [0., 10., 0.],
            age,
            strength,
        };
        let drawn = sprites(
            &art,
            &[effect(EffectKind::Destroyed, Some(30), 60)],
            &[
                mark(MarkKind::Crater(18), 0, 1.),
                mark(MarkKind::Fire, 60, 0.25),
            ],
        );
        assert_eq!(drawn[0].mode, Mode::Flat);
        assert_eq!(drawn[0].extent, [288., 288.]);
        assert_eq!(drawn[0].cell[1..], [0., 78., 66.]);
        assert_eq!(drawn[1].mode, Mode::Standing);
        assert_eq!(drawn[1].cell, SHEETS[FIRE_SHEET].1.cell(7));
        assert_eq!(drawn[1].opacity, 0.25);
        assert!(!drawn[0].emissive && drawn[1].emissive && drawn[2].emissive);
        let mut missing = Art::synthetic();
        missing.present[CRATER_SHEET] = false;
        assert_eq!(
            sprites(&missing, &[], &[mark(MarkKind::Crater(3), 0, 1.)]).len(),
            0
        );
    }
}
