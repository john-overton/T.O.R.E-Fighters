//! Imported pilot/seat/chute poses. Never executes the SH module.
use std::collections::BTreeMap;
use tore_formats::{Pic, shape::Shape};
use tore_sim::ejection::Phase;

pub struct Art {
    pub atlas: Pic,
    poses: Vec<Shape>,
    regions: BTreeMap<String, (usize, usize)>,
}
impl Art {
    pub fn load(data: &BTreeMap<String, Vec<u8>>) -> crate::AppResult<Self> {
        let get = |n: &str| {
            data.get(n)
                .ok_or_else(|| format!("Missing {n}; reimport retail media"))
        };
        let code = get("EJECT.SH")?;
        let (bytes, _) = tore_formats::module::code(code)?;
        if bytes.len() != 14600
            || bytes.get(0x35..0x3e) != Some(&[0xf0, 0, 0x83, 0x0d, 0xfc, 0x48, 0, 0, 2])
        {
            return Err("Unreviewed EJECT.SH layout".into());
        }
        let poses = (34..=38)
            .map(|phase| {
                Shape::with_export_state(code, &BTreeMap::from([(0x48f6, phase), (0x48f0, 0)]))
            })
            .collect::<tore_formats::Result<Vec<_>>>()?;
        let mut atlas = Pic {
            width: 0,
            height: 0,
            pixels: vec![],
            mask: vec![],
            palette: vec![],
            glyphs: vec![],
        };
        let mut regions = BTreeMap::new();
        let mut pics = Vec::new();
        for name in ["_EJECTA.PIC", "_EJECTB.PIC", "_EJECTC.PIC", "_EJECTD.PIC"] {
            let pic = Pic::parse(get(name)?)?;
            if !pic.palette.is_empty() || pic.width > 1024 || pic.height > 1024 {
                return Err("Unreviewed ejection texture".into());
            }
            atlas.width = atlas.width.max(pic.width);
            regions.insert(name.to_string(), (atlas.height, pic.height));
            atlas.height += pic.height;
            pics.push(pic);
        }
        atlas.pixels.resize(atlas.width * atlas.height, 255);
        atlas.mask.resize(atlas.pixels.len(), false);
        let mut row = 0;
        for pic in pics {
            for y in 0..pic.height {
                let at = (row + y) * atlas.width;
                atlas.pixels[at..at + pic.width]
                    .copy_from_slice(&pic.pixels[y * pic.width..(y + 1) * pic.width]);
                atlas.mask[at..at + pic.width]
                    .copy_from_slice(&pic.mask[y * pic.width..(y + 1) * pic.width]);
            }
            row += pic.height;
        }
        for face in poses.iter().flat_map(|p| &p.faces) {
            if !face.uv.is_empty() && !regions.contains_key(&face.texture) {
                return Err("Unknown ejection texture reference".into());
            }
        }
        Ok(Self {
            atlas,
            poses,
            regions,
        })
    }
    /// Synthetic poses over a blank atlas with the named texture rows, for
    /// drawing tests without retail media.
    #[cfg(test)]
    pub(crate) fn synthetic(poses: Vec<Shape>, textures: &[(&str, usize, usize)]) -> Self {
        let mut regions = BTreeMap::new();
        let (mut width, mut height) = (0, 0);
        for (name, texture_width, texture_height) in textures {
            regions.insert((*name).to_string(), (height, *texture_height));
            height += texture_height;
            width = width.max(*texture_width);
        }
        Self {
            atlas: Pic {
                width,
                height,
                pixels: vec![7; width * height],
                mask: vec![true; width * height],
                palette: Vec::new(),
                glyphs: Vec::new(),
            },
            poses,
            regions,
        }
    }
    /// Pilot, seat and parachute vertices for each (position, heading, phase).
    pub fn vertices_for(
        &self,
        pilots: impl IntoIterator<Item = ([f64; 3], f64, Phase)>,
        palette: &[[u8; 3]; 256],
        camera: [f64; 3],
    ) -> Vec<f32> {
        let mut out = Vec::new();
        for (position, heading, phase) in pilots {
            let pose = match phase {
                Phase::Seat => 0,
                Phase::Freefall | Phase::Impact => 1,
                Phase::Inflating => 2,
                Phase::Parachute | Phase::Landed => 4,
            };
            let (sin, cos) = (heading as f32).sin_cos();
            for line in &self.poses[pose].lines {
                let points = line.positions.map(|[x, z, y]| {
                    [
                        position[0] + f64::from(x * cos + z * sin) / 3.,
                        position[1] + f64::from(y) / 3.,
                        position[2] + f64::from(-x * sin + z * cos) / 3.,
                    ]
                });
                let direction = std::array::from_fn(|i| points[1][i] - points[0][i]);
                let view = std::array::from_fn(|i| camera[i] - points[0][i]);
                let cross = tore_sim::attitude::cross(direction, view);
                let length = tore_sim::attitude::dot(cross, cross).sqrt();
                if length < 1e-9 {
                    continue;
                }
                let offset = cross.map(|v| v * 0.025 / length);
                let color = palette[line.color as usize].map(|v| f32::from(v) / 255.);
                for (end, sign) in [(0, -1.), (1, -1.), (1, 1.), (0, -1.), (1, 1.), (0, 1.)] {
                    out.extend_from_slice(&[
                        (points[end][0] + sign * offset[0]) as f32,
                        (points[end][1] + sign * offset[1]) as f32,
                        (points[end][2] + sign * offset[2]) as f32,
                        0.,
                        0.,
                        -1.,
                        color[0],
                        color[1],
                        color[2],
                        line.color as f32 + 256. * line.fog as u8 as f32,
                    ]);
                }
            }
            for face in &self.poses[pose].faces {
                for i in 1..face.positions.len() - 1 {
                    for j in [0, i, i + 1] {
                        let [x, z, y] = face.positions[j].map(|v| v / 3.);
                        let uv = if face.uv.is_empty() {
                            [0.; 2]
                        } else {
                            let (row, height) = self.regions[&face.texture];
                            [
                                (face.uv[j][0] + 0.5) / self.atlas.width as f32,
                                (row as f32 + height as f32 - 0.5 - face.uv[j][1])
                                    / self.atlas.height as f32,
                            ]
                        };
                        let color = palette[face.colors[j] as usize].map(|v| f32::from(v) / 255.);
                        out.extend_from_slice(&[
                            position[0] as f32 + x * cos + z * sin,
                            position[1] as f32 + y,
                            position[2] as f32 - x * sin + z * cos,
                            uv[0],
                            uv[1],
                            if face.uv.is_empty() { -1. } else { -2. },
                            color[0],
                            color[1],
                            color[2],
                            face.colors[j] as f32 + 256. * face.fog as u8 as f32,
                        ]);
                    }
                }
            }
        }
        out
    }
}
