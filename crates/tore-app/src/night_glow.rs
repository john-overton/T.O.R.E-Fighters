//! The night glow of explosions and fires (nr-fx3, opinionated, John
//! 2026-10-13; docs/spec/explosions.md, "Night glow"). Presentation only,
//! derived from the picture's effects, marks and burning city alone, so a
//! single-player flight, a networked client and a replay show the same.
//!
//! Two parts:
//!
//! - **Glow points.** Every explosion, fireball and fire is also drawn as an
//!   additive point of light in the world pass, tested against the world's
//!   depth (a hill hides it), with a minimum size on screen and a floor on
//!   its brightness, so it stays visible out to the limit of the night scene
//!   instead of shrinking below a pixel and fading out by 5 to 10 miles.
//!   Its brightness and its smallest size grow with the size of the fire or
//!   blast, which for a weapon is its yield ([`blast_scale`]). Close in, where
//!   the fire's own art is several pixels wide, the point turns into a soft
//!   halo around it. Haze dims it as a light seen far through the night
//!   ([`GLOW_HAZE_SHARE`]), and a cloud between hides it. It shows at night
//!   and dusk only.
//! - **Fire light.** A burning fire lights the ground around it, as a flare
//!   does, with a strength that grows with its width; the light's reach
//!   grows with its strength (`surface_lighting.wgsl`), so it stays a
//!   physical light. Only the nearest [`MAX_FIRE_LIGHTS`] within
//!   [`FIRE_LIGHT_FT`] of the camera light the ground: the glow point, not
//!   the ground light, is what is seen from far off.
//!
//! The numbers are agent decisions tuned on renders (2026-10-13), labelled
//! fitted in the spec. `TORE_NIGHT_GLOW=0` turns both parts off, to compare
//! and to measure their cost.
use crate::countermeasure_renderer::{FLARE_STRENGTH, FlareLight};
use tore_sim::{attitude::Vector, combat::blast_scale};

/// Glow points drawn at most, the nearest.
pub const MAX_GLOWS: usize = 4_096;
/// Bytes of one glow instance: offset from the eye, width, strength, heat.
pub const INSTANCE_BYTES: usize = 6 * 4;

/// A fire's light, in flares, is `FIRE_LIGHT_AT_30_FT` for a 30 ft fire
/// (a vehicle's), growing as the width to the power `FIRE_LIGHT_POWER`, within
/// `FIRE_LIGHT_RANGE`: a 24 ft city building fire 0.36 flares, a 90 ft one
/// 2.6, a 160 ft fuel or ammunition fire 6.2.
pub const FIRE_LIGHT_AT_30_FT: f64 = 0.5;
pub const FIRE_LIGHT_POWER: f64 = 1.5;
pub const FIRE_LIGHT_RANGE: [f64; 2] = [0.25, 8.];
/// The fire's light stands this share of its width above the ground.
pub const FIRE_LIGHT_RISE: f64 = 0.35;
/// Fires that light the ground at once, the nearest, within this many feet
/// of the camera (3 nautical miles).
pub const MAX_FIRE_LIGHTS: usize = 8;
pub const FIRE_LIGHT_FT: f64 = 3. * 6_076.12;
/// A fireball glows on after its flash at this share of the flash's peak,
/// fading with the square of the time left of its art.
pub const EMBER_SHARE: f64 = 0.3;

// Mirrors of the constants in night_glow.wgsl.
/// The point is at least this many pixels in radius, more for a bigger
/// light (`0.8 + 0.45 * sqrt(flares)` times it), at most `GLOW_MAX_PX`.
#[cfg_attr(not(test), allow(dead_code))]
pub const GLOW_MIN_PX: f64 = 1.5;
#[cfg_attr(not(test), allow(dead_code))]
pub const GLOW_MAX_PX: f64 = 4.5;
/// Its brightness: `GLOW_BASE * sqrt(flares)` within `GLOW_LEVEL`.
#[cfg_attr(not(test), allow(dead_code))]
pub const GLOW_BASE: f64 = 0.8;
#[cfg_attr(not(test), allow(dead_code))]
pub const GLOW_LEVEL: [f64; 2] = [0.4, 2.4];
/// The point takes over from the art as the art shrinks from
/// `GLOW_SPRITE_PX[1]` to `GLOW_SPRITE_PX[0]` pixels across; nearer it is a
/// halo `GLOW_HALO` times the art's width at `GLOW_NEAR` of its brightness.
#[cfg_attr(not(test), allow(dead_code))]
pub const GLOW_SPRITE_PX: [f64; 2] = [3., 16.];
#[cfg_attr(not(test), allow(dead_code))]
pub const GLOW_HALO: f64 = 0.8;
#[cfg_attr(not(test), allow(dead_code))]
pub const GLOW_NEAR: f64 = 0.12;
/// Haze dims a glow as it dims the ground at this share of the distance,
/// counting only the haze ramp: on a clear night (the ramp ending at about
/// 3.5 nm) a glow fades out by about 35 nm; in fog, far nearer.
#[allow(dead_code)]
pub const GLOW_HAZE_SHARE: f64 = 0.1;

/// Whether the glow and fire light are on (`TORE_NIGHT_GLOW=0` turns them
/// off).
pub fn enabled() -> bool {
    std::env::var("TORE_NIGHT_GLOW").as_deref() != Ok("0")
}

/// A fire `width` feet wide: its light in flares.
pub fn fire_light(width: f64) -> f64 {
    (FIRE_LIGHT_AT_30_FT * (width.max(0.) / 30.).powf(FIRE_LIGHT_POWER))
        .clamp(FIRE_LIGHT_RANGE[0], FIRE_LIGHT_RANGE[1])
}

/// An explosion's glow in flares, `elapsed` ticks into its art's `duration`:
/// its flash ([`crate::impact_fx::light`], by the weapon's yield) and the
/// fireball's ember after it. `blast` is the explosion type when the effect
/// detonates (not a dud's puff), `width` the drawn width.
pub fn explosion_glow(
    position: Vector,
    blast: Option<u8>,
    scale: u8,
    width: f64,
    elapsed: u16,
    duration: u16,
) -> f64 {
    let factor = blast_scale::factor(scale);
    let flash = blast
        .and_then(|kind| crate::impact_fx::light(position, kind, scale, f64::from(elapsed)))
        .map_or(0., |(_, strength)| strength / FLARE_STRENGTH);
    let left = 1. - f64::from(elapsed) / f64::from(duration.max(1));
    let peak = crate::impact_fx::light_of(width, factor).0 / FLARE_STRENGTH;
    flash + EMBER_SHARE * peak * left.clamp(0., 1.).powi(2)
}

/// One glow point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    /// The middle of the fire or blast.
    pub position: Vector,
    /// The width of its art, feet.
    pub width: f64,
    /// How much light it gives, in flares.
    pub strength: f64,
    /// 1 a white-hot flash, 0 a fire's orange.
    pub heat: f32,
}

/// The nearest [`MAX_GLOWS`] of `glows` to `eye`, as instances relative to
/// the eye in full precision.
pub fn instances(glows: &[Glow], eye: Vector) -> Vec<u8> {
    let mut near: Vec<(f64, &Glow)> = glows
        .iter()
        .filter(|g| g.strength > 0.)
        .map(|g| (distance(g.position, eye), g))
        .collect();
    if near.len() > MAX_GLOWS {
        near.select_nth_unstable_by(MAX_GLOWS - 1, |a, b| a.0.total_cmp(&b.0));
        near.truncate(MAX_GLOWS);
    }
    let mut out = Vec::with_capacity(near.len() * INSTANCE_BYTES);
    for (_, g) in near {
        let offset: [f32; 3] = std::array::from_fn(|i| (g.position[i] - eye[i]) as f32);
        for value in offset
            .into_iter()
            .chain([g.width as f32, g.strength as f32, g.heat])
        {
            out.extend(value.to_le_bytes());
        }
    }
    out
}

fn distance(a: Vector, b: Vector) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

/// The ground light of the nearest fires: `fires` are each fire's base and
/// width; at most [`MAX_FIRE_LIGHTS`], within [`FIRE_LIGHT_FT`] of `eye`.
pub fn fire_lights(fires: &[(Vector, f64)], eye: Vector) -> Vec<FlareLight> {
    let mut near: Vec<(f64, FlareLight)> = fires
        .iter()
        .filter_map(|&(base, width)| {
            let position = [base[0], base[1] + width * FIRE_LIGHT_RISE, base[2]];
            let d = distance(position, eye);
            (d <= FIRE_LIGHT_FT).then_some((
                d,
                FlareLight {
                    position,
                    strength: fire_light(width) * FLARE_STRENGTH,
                },
            ))
        })
        .collect();
    if near.len() > MAX_FIRE_LIGHTS {
        near.select_nth_unstable_by(MAX_FIRE_LIGHTS - 1, |a, b| a.0.total_cmp(&b.0));
        near.truncate(MAX_FIRE_LIGHTS);
    }
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    near.into_iter().map(|(_, light)| light).collect()
}

/// What the shader draws of a glow point of `width` feet and `strength`
/// flares at view depth `z` feet, for a picture `height` pixels tall at
/// `zoom`, at full night and before haze: its radius in pixels and its
/// brightness. A mirror of `glow_vertex` in night_glow.wgsl, for tests.
#[cfg_attr(not(test), allow(dead_code))]
pub fn on_screen(width: f64, strength: f64, z: f64, height: f64, zoom: f64) -> (f64, f64) {
    let pixel = 2. * z / (3_f64.sqrt() * zoom * height.max(1.));
    let across = width / pixel;
    let t = ((across - GLOW_SPRITE_PX[0]) / (GLOW_SPRITE_PX[1] - GLOW_SPRITE_PX[0])).clamp(0., 1.);
    let far = 1. - t * t * (3. - 2. * t);
    let s = strength.sqrt();
    let min_px = (GLOW_MIN_PX * (0.8 + 0.45 * s)).clamp(GLOW_MIN_PX, GLOW_MAX_PX);
    let radius = (width * GLOW_HALO / pixel).max(min_px);
    let level =
        (GLOW_NEAR + (1. - GLOW_NEAR) * far) * (GLOW_BASE * s).clamp(GLOW_LEVEL[0], GLOW_LEVEL[1]);
    (radius, level)
}

/// The glow points' pipeline: additive points in the world pass, tested
/// against the world depth without writing it, like the airfield lights.
pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    /// `surface` holds the world material and shared lighting groups.
    pub fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        samples: u32,
        surface: &wgpu::PipelineLayout,
    ) -> Self {
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Night glow of explosions and fires"),
            layout: Some(surface),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("glow_vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: INSTANCE_BYTES as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32,2=>Float32,3=>Float32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("glow_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(additive),
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
        });
        Self { pipeline }
    }
}

/// This frame's glow points on the GPU.
pub struct Points {
    buffer: wgpu::Buffer,
    count: u32,
}

impl Points {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Bounded night glow points"),
                size: (MAX_GLOWS * INSTANCE_BYTES) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            count: 0,
        }
    }

    /// Uploads this frame's glow points as seen from `eye`.
    pub fn update(&mut self, queue: &wgpu::Queue, glows: &[Glow], eye: Vector) {
        let bytes = instances(glows, eye);
        self.count = (bytes.len() / INSTANCE_BYTES) as u32;
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, &bytes);
        }
    }

    /// Inside the world pass; the caller has bound the world material
    /// (group 0) and shared lighting (group 1).
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, pipeline: &Pipeline) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NM: f64 = 6_076.12;

    #[test]
    fn a_bigger_fire_glows_and_lights_more() {
        // City building fires 24 to 90 ft, fuel and ammunition to 160 ft.
        let widths = [24., 30., 60., 90., 160.];
        let lights: Vec<f64> = widths.iter().map(|w| fire_light(*w)).collect();
        assert!(lights.windows(2).all(|w| w[0] < w[1]), "{lights:?}");
        assert!((fire_light(30.) - FIRE_LIGHT_AT_30_FT).abs() < 1e-12);
        assert!((fire_light(160.) - 6.16).abs() < 0.01);
        // And the light reaches farther (surface_lighting.wgsl: the reach
        // grows with the square root of the strength past one flare).
        let reach = |flares: f64| 1_500. * flares.sqrt().max(1.);
        assert!(reach(fire_light(160.)) > 2. * reach(fire_light(30.)));
    }

    #[test]
    fn a_bombs_glow_grows_with_its_yield() {
        // 250, 500 and 2,000 lb bombs on the same type 35 land explosion.
        let at = [0., 0., 0.];
        let glow = |scale: u8, elapsed: u16| {
            let width = 150. * blast_scale::factor(scale);
            explosion_glow(at, Some(35), scale, width, elapsed, 240)
        };
        for elapsed in [0, 10, 60, 180] {
            let row = [glow(79, elapsed), glow(100, elapsed), glow(159, elapsed)];
            assert!(row[0] < row[1] && row[1] < row[2], "{elapsed}: {row:?}");
        }
        // The flash then the fireball's ember, gone when its art ends.
        assert!(glow(159, 0) > 3. * glow(159, 120));
        assert!(glow(159, 120) > 0.);
        assert_eq!(glow(159, 240), 0.);
        // A dud's inert puff gives no flash, only what its art shows.
        assert!(explosion_glow(at, None, 0, 75., 0, 240) < glow(79, 0));
        // On screen: at least as large at every range, and once both are
        // points of light (their art a few pixels or less), brighter.
        for nm in [0.3, 2., 5., 10., 20., 30.] {
            let small = on_screen(150. * 0.79, glow(79, 2), nm * NM, 1080., 1.);
            let big = on_screen(150. * 1.59, glow(159, 2), nm * NM, 1080., 1.);
            assert!(big.0 >= small.0, "{nm} nm");
            if nm >= 10. {
                assert!(big.1 > small.1, "{nm} nm");
            }
        }
    }

    #[test]
    fn a_glow_keeps_a_size_and_brightness_out_to_the_night_scenes_limit() {
        // A 30 ft vehicle fire and a 2,000 lb bomb's flash, near and far.
        for (width, flares) in [(30., fire_light(30.)), (238., 12.)] {
            let mut last = 0.;
            for nm in [2., 5., 10., 20., 40.] {
                let (radius, level) = on_screen(width, flares, nm * NM, 1080., 1.);
                assert!(radius >= GLOW_MIN_PX, "{width} ft at {nm} nm: {radius} px");
                // Never dimmer for being farther: the point takes over as
                // the art shrinks, at full strength from 10 nm.
                assert!(level >= last - 1e-9, "{width} ft at {nm} nm");
                if nm >= 10. {
                    assert!(level >= GLOW_LEVEL[0], "{width} ft at {nm} nm: {level}");
                }
                last = level;
            }
            // Close in, a soft halo around the art.
            let (radius, level) = on_screen(width, flares, 300., 1080., 1.);
            assert!(
                radius > 30. && level < 0.25 * GLOW_LEVEL[1],
                "{radius} {level}"
            );
        }
    }

    #[test]
    fn ground_light_is_culled_by_distance_but_glow_points_are_not() {
        let fires: Vec<(Vector, f64)> = (0..40)
            .map(|k| ([f64::from(k) * 2_000., 0., 0.], 60.))
            .collect();
        let lights = fire_lights(&fires, [0., 500., 0.]);
        assert_eq!(lights.len(), MAX_FIRE_LIGHTS);
        assert!(lights.iter().all(|l| l.position[0] <= FIRE_LIGHT_FT));
        assert_eq!(lights[0].position, [0., 21., 0.]);
        assert_eq!(lights[0].strength, fire_light(60.) * FLARE_STRENGTH);
        // From 20 nm no fire lights the ground, but every glow is drawn.
        assert!(fire_lights(&fires, [0., 500., 20. * NM]).is_empty());
        let glows: Vec<Glow> = fires
            .iter()
            .map(|&(position, width)| Glow {
                position,
                width,
                strength: fire_light(width),
                heat: 0.,
            })
            .collect();
        let bytes = instances(&glows, [0., 500., 20. * NM]);
        assert_eq!(bytes.len(), 40 * INSTANCE_BYTES);
    }

    #[test]
    fn only_the_nearest_glows_are_drawn() {
        let glows: Vec<Glow> = (0..MAX_GLOWS + 500)
            .map(|k| Glow {
                position: [k as f64 * 10., 0., 0.],
                width: 30.,
                strength: 1.,
                heat: 0.,
            })
            .collect();
        let bytes = instances(&glows, [0.; 3]);
        assert_eq!(bytes.len(), MAX_GLOWS * INSTANCE_BYTES);
        let far = |b: &[u8]| f32::from_le_bytes(b[0..4].try_into().unwrap());
        assert!(
            bytes
                .chunks(INSTANCE_BYTES)
                .all(|b| far(b) < (MAX_GLOWS * 10) as f32)
        );
    }
}
