//! Original CLOUD1 geometry with source repeat layout and mission altitude.
use crate::{AppResult, terrain::Camera};
use std::collections::BTreeMap;
use tore_formats::{Pic, shape::Shape, weather::clouds::Layout};
pub struct Clouds {
    pub layout: Layout,
    pub altitude: i32,
    shape: Shape,
    scale: f32,
    texture: usize,
    radius_feet: i32,
    detail: u8,
    exponent: u16,
}
impl Clouds {
    pub fn load(
        resources: &BTreeMap<String, Vec<u8>>,
        images: &mut Vec<u8>,
        offset: usize,
        altitude: i32,
    ) -> AppResult<Self> {
        let read = |n: &str| {
            resources
                .get(n)
                .ok_or_else(|| format!("Missing {n}; re-import media"))
        };
        let layout = Layout::decode(read("TORE_CLOUDS_V1")?)?;
        let data = read("CLOUD1.SH")?;
        let shape = Shape::parse(data)?;
        let (code, _) = tore_formats::module::code(data)?;
        let exponent = u16::from_le_bytes(code[6..8].try_into().unwrap());
        let radius = i16::from_le_bytes(code[4..6].try_into().unwrap());
        if !(8..=20).contains(&exponent)
            || radius < 0
            || shape.faces.len() != 2
            || shape.faces.iter().any(|f| {
                f.texture != "_CLOUD1.PIC"
                    || f.positions.len() != 4
                    || f.uv.len() != 4
                    || f.positions.iter().any(|p| p[2] != 0.)
            })
        {
            return Err("unsupported cloud sheet geometry".into());
        }
        let radius_feet = i32::from(radius) << (exponent - 8);
        if radius_feet > 1_000_000 {
            return Err("unsupported cloud bounds".into());
        }
        let detail = match std::env::var("TORE_CLOUD_DETAIL").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("2") => 2,
            Ok("0") => 0,
            Ok("1") => 1,
            _ => return Err("TORE_CLOUD_DETAIL must be 0, 1 or 2".into()),
        };
        let pic = Pic::parse(read("_CLOUD1.PIC")?)?;
        if pic.width > 256 || pic.height > 256 || !pic.palette.is_empty() {
            return Err("unsupported cloud texture".into());
        }
        let texture = offset + images.len() / 65536;
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
        Ok(Self {
            layout,
            altitude,
            shape,
            scale: 2_f32.powi(i32::from(exponent) - 8),
            texture,
            radius_feet,
            detail,
            exponent,
        })
    }
    pub fn vertices(&self, camera: &Camera) -> Vec<f32> {
        // Host radians to native binary-angle words; exact native matrix rounding
        // is not implied. Normalize before converting so long-running turns wrap.
        let angle = |v: f32| {
            (v.rem_euclid(std::f32::consts::TAU) * 65536. / std::f32::consts::TAU).round() as i32
                as i16
        };
        let mut centers = tore_sim::clouds::centers_for_view(
            &self.layout,
            camera.position.map(f64::from),
            self.altitude,
            tore_sim::clouds::View {
                heading: angle(camera.yaw),
                pitch: angle(camera.pitch),
                detail: self.detail,
                radius_feet: self.radius_feet,
            },
        );
        centers.retain(|(center, _)| {
            tore_sim::clouds::within_shape_range(
                camera.position.map(f64::from),
                *center,
                self.exponent,
            )
        });
        let distance = |p: [f64; 3]| {
            p.iter()
                .zip(camera.position)
                .map(|(a, b)| (a - f64::from(b)).powi(2))
                .sum::<f64>()
        };
        centers.sort_by(|a, b| distance(b.0).total_cmp(&distance(a.0)));
        let mut out = Vec::with_capacity(centers.len() * 60);
        // Two coincident source faces have opposite normals and reversed UVs.
        // Select the viewer-facing side instead of double-blending both faces.
        let face = &self.shape.faces[usize::from(camera.position[1] >= self.altitude as f32)];
        for (center, yaw) in centers {
            for i in [0, 1, 2, 0, 2, 3] {
                let p = face.positions[i];
                let p =
                    crate::celestial::rotate([p[0] * self.scale, 0., p[1] * self.scale], [yaw, 0]);
                out.extend((0..3).map(|k| center[k] as f32 + p[k]));
                out.extend([
                    face.uv[i][0] / 256.,
                    face.uv[i][1] / 256.,
                    self.texture as f32,
                    0.,
                    0.,
                    0.,
                    0.,
                ]);
            }
        }
        out
    }
}
