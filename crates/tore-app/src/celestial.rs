//! Original weather primitives, projected independently of camera translation.
use crate::{
    AppResult,
    terrain::{Camera, World},
};
use std::collections::BTreeMap;
use tore_formats::{
    Pic,
    weather::shape::{Primitive, WeatherShape},
};

pub struct Celestial {
    pub sun: WeatherShape,
    pub flare: tore_formats::weather::flare::Layout,
    pub sun_effects: bool,
    pub moon: WeatherShape,
    pub stars: WeatherShape,
    pub moon_texture: usize,
    pub moon_uv: [f32; 4],
    pub sun_remap: usize,
    pub shade_rows: BTreeMap<[u8; 3], usize>,
}
impl Celestial {
    pub fn load(
        resources: &BTreeMap<String, Vec<u8>>,
        images: &mut Vec<u8>,
        offset: usize,
        sun_fill: &[u8; 256],
        shades: &[tore_formats::weather::ShadeRemap],
    ) -> AppResult<Self> {
        let read = |n: &str| {
            resources
                .get(n)
                .ok_or_else(|| format!("Missing {n}; re-import media"))
        };
        let flare = tore_formats::weather::flare::Layout::decode(read("TORE_FLARE_V1")?)?;
        let sun = WeatherShape::parse(read("SUN.SH")?)?;
        let moon = WeatherShape::parse(read("MOON.SH")?)?;
        let stars = WeatherShape::parse(read("STARS.SH")?)?;
        // This renderer accepts the reviewed concentric source sun and one moon.
        if sun.primitives.len() > 8
            || sun.primitives.iter().any(|p| {
                !matches!(
                    p,
                    Primitive::Circle { fill: 267, .. } | Primitive::Circle { fill: 0..=255, .. }
                )
            })
        {
            return Err("unsupported sun primitive/material".into());
        }
        if sun.primitives.iter().any(|p| matches!(p,Primitive::Circle{center,..} if center[0]!=0. || center[1]!=0. || center[2]<=0.))
            || stars.primitives.len()>1024
            || stars.primitives.iter().any(|p| !matches!(p,Primitive::Point{fill:0..=255,..})) {
            return Err("unsupported celestial geometry".into());
        }
        let [
            Primitive::Billboard {
                texture, uv: None, ..
            },
        ] = moon.primitives.as_slice()
        else {
            return Err("unsupported moon shape".into());
        };
        let pic = Pic::parse(read(texture)?)?;
        if pic.width < 3
            || pic.height < 3
            || pic.width > 256
            || pic.height > 256
            || !pic.palette.is_empty()
        {
            return Err("unsupported moon texture".into());
        }
        let moon_texture = offset + images.len() / 65536;
        let mut pixels = vec![255; 65536];
        for y in 0..pic.height {
            for x in 0..pic.width {
                let i = y * pic.width + x;
                if pic.mask[i] {
                    pixels[y * 256 + x] = pic.pixels[i];
                }
            }
        }
        images.extend(pixels);
        let moon_uv = [
            1. / 256.,
            1. / 256.,
            (pic.width - 2) as f32 / 256.,
            (pic.height - 2) as f32 / 256.,
        ];
        let sun_remap = offset + images.len() / 65536;
        let mut pixels = vec![0; 65536];
        pixels[..256].copy_from_slice(sun_fill);
        let mut shade_rows = BTreeMap::new();
        let mut row = 1;
        for shade in shades {
            if shade_rows.contains_key(&shade.color) {
                continue;
            }
            if row + shade.levels.len() > 256 {
                return Err("too many sky shade levels".into());
            }
            shade_rows.insert(shade.color, row);
            for level in &shade.levels {
                pixels[row * 256..(row + 1) * 256].copy_from_slice(level);
                row += 1;
            }
        }
        images.extend(pixels);
        Ok(Self {
            sun,
            flare,
            sun_effects: match std::env::var("TORE_SUN_GLARE").as_deref() {
                Ok("0") => false,
                Ok("1") | Err(_) => true,
                _ => return Err("TORE_SUN_GLARE needs 0 or 1".into()),
            },
            moon,
            stars,
            moon_texture,
            moon_uv,
            sun_remap,
            shade_rows,
        })
    }
    pub fn sun_uniform(&self, world: &World, altitude: f32) -> Vec<f32> {
        let angle = world
            .weather
            .sample(altitude as f64)
            .and_then(|l| tore_sim::environment::sun_angles(&l, world.weather.seconds_of_day()));
        let mut out = vec![0.; 36];
        if let Some(angle) = angle {
            let direction = rotate([0., 0., 1.], angle);
            out[..3].copy_from_slice(&direction);
            out[3] = self.sun.primitives.len() as f32;
            for (i, p) in self.sun.primitives.iter().enumerate() {
                if let Primitive::Circle {
                    center,
                    diameter,
                    fill,
                } = p
                {
                    // Native circle radius is half the projected diameter.
                    out[4 + i * 4] = *diameter as f32 / (2. * center[2]);
                    out[5 + i * 4] = *fill as f32;
                }
            }
        }
        out
    }
    pub fn vertices(&self, world: &World, camera: &Camera, height: u32) -> Vec<f32> {
        let Some(layer) = world.weather.sample(camera.position[1] as f64) else {
            return Vec::new();
        };
        if layer.flags & 16 == 0 {
            return Vec::new();
        }
        let mut vertices = Vec::new();
        let u = camera.uniform(1., [0.; 4], [0; 3]);
        let right: [f32; 3] = u[4..7].try_into().unwrap();
        let up: [f32; 3] = u[8..11].try_into().unwrap();
        for p in &self.stars.primitives {
            if let Primitive::Point { center, fill } = p {
                let distance = center.iter().map(|x| x * x).sum::<f32>().sqrt();
                let half =
                    distance / (height as f32 * camera.view_fraction * 1.7320508 * camera.zoom);
                quad(
                    &mut vertices,
                    *center,
                    right.map(|v| v * half),
                    up.map(|v| v * half),
                    -1.,
                    *fill as f32,
                    [0., 0., 1., 1.],
                );
            }
        }
        let angle = [layer.moon_azimuth, layer.moon_elevation];

        for p in &self.moon.primitives {
            if let Primitive::Billboard { center, size, .. } = p {
                let center = rotate(*center, angle);
                // The lunar texture basis belongs to the sky, not aircraft bank.
                // Mixing a rolled camera right with a world vertical shears it.
                let horizontal = rotate([1., 0., 0.], angle);
                let vertical = rotate([0., 1., 0.], angle);
                quad(
                    &mut vertices,
                    center,
                    horizontal.map(|v| v * size[0] as f32 * 0.5),
                    vertical.map(|v| v * size[1] as f32 * 0.5),
                    self.moon_texture as f32,
                    0.,
                    self.moon_uv,
                );
            }
        }
        vertices
    }
}
pub fn rotate(p: [f32; 3], angle: [i16; 2]) -> [f32; 3] {
    let [az, el] = angle.map(|v| v as f32 * std::f32::consts::TAU / 65536.);
    let (s, c) = el.sin_cos();
    let y = p[1] * c + p[2] * s;
    let z = p[2] * c - p[1] * s;
    let (s, c) = az.sin_cos();
    [p[0] * c + z * s, y, z * c - p[0] * s]
}
#[allow(clippy::too_many_arguments)]
fn quad(
    out: &mut Vec<f32>,
    center: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    texture: f32,
    index: f32,
    uv: [f32; 4],
) {
    for [x, y] in [
        [-1., -1.],
        [1., -1.],
        [1., 1.],
        [-1., -1.],
        [1., 1.],
        [-1., 1.],
    ] {
        out.extend((0..3).map(|i| center[i] + x * right[i] + y * up[i]));
        out.extend([
            if x < 0. { uv[0] } else { uv[2] },
            if y < 0. { uv[3] } else { uv[1] },
            texture,
            0.,
            0.,
            0.,
            index,
        ]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lunar_geometry_is_independent_of_camera_bank_and_translation() {
        let mut world = crate::terrain::tests::world();
        let mut module =
            tore_formats::weather::Module::parse(&tore_formats::weather::synthetic_module(1))
                .unwrap();
        let layer = &mut module.layers[0];
        layer.flags = 16;
        layer.moon_azimuth = 8192;
        layer.moon_elevation = 4096;
        world.weather = tore_sim::environment::Environment::new(
            tore_sim::environment::Configuration::new(module, 0, 0, 0, None).unwrap(),
        );
        let empty = WeatherShape {
            primitives: vec![],
            scale_exponent: 8,
            publishes_point: false,
        };
        let celestial = Celestial {
            sun: empty.clone(),
            stars: empty.clone(),
            moon: WeatherShape {
                primitives: vec![Primitive::Billboard {
                    center: [0., 0., 160.],
                    size: [4, 4],
                    texture: "SYNTH.PIC".into(),
                    uv: None,
                }],
                ..empty
            },
            moon_texture: 0,
            moon_uv: [0., 0., 1., 1.],
            sun_remap: 0,
            shade_rows: BTreeMap::new(),
            flare: tore_formats::weather::flare::Layout { circles: vec![] },
            sun_effects: true,
        };
        let mut camera = Camera::new();
        camera.position[1] = 0.;
        let expected = celestial.vertices(&world, &camera, 720);
        assert_eq!(expected.len(), 60);
        for roll in [-2., -1., 0., 1., 2.] {
            camera.roll = roll;
            camera.position[0] += 100.;
            camera.position[2] -= 300.;
            assert_eq!(celestial.vertices(&world, &camera, 720), expected);
        }
        let edge = |a: usize, b: usize| -> Vec<f32> {
            (0..3)
                .map(|k| expected[a * 10 + k] - expected[b * 10 + k])
                .collect()
        };
        let right = edge(1, 0);
        let up = edge(2, 1);
        assert!((right.iter().map(|x| x * x).sum::<f32>() - 16.).abs() < 0.001);
        assert!((right.iter().zip(up).map(|(a, b)| a * b).sum::<f32>()).abs() < 0.001);
    }
}
