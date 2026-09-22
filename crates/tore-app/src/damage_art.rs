//! Original alternate damage bodies with a shared, lossless texture atlas.
use crate::AppResult;
use std::collections::BTreeMap;
use tore_formats::{
    Pic,
    aircraft::AircraftId,
    shape::{Face, Shape},
};

pub struct DamageArt {
    extents: [f32; 3],
    pub bodies: [Shape; 2],
    pub fragments: [Shape; 2],
    /// Texture name -> source width, source height, atlas row offset.
    pub regions: BTreeMap<String, [usize; 3]>,
}
impl DamageArt {
    pub fn load(
        id: AircraftId,
        data: &BTreeMap<String, Vec<u8>>,
        atlas: &mut Pic,
    ) -> AppResult<Self> {
        let get = |name: &str| {
            data.get(name)
                .ok_or_else(|| format!("missing damage resource {name}; re-import media"))
        };
        let intact = Shape::parse(get(&format!("{}.SH", id.stem()))?)?;
        let mut extents = [1f32; 3];
        for p in intact.faces.iter().flat_map(|f| &f.positions) {
            for i in 0..3 {
                extents[i] = extents[i].max(p[i].abs());
            }
        }
        let mut bodies = [
            Shape::parse(get(&format!("{}_A.SH", id.stem()))?)?,
            Shape::parse(get(&format!("{}_C.SH", id.stem()))?)?,
        ];
        let fragments = [
            Shape::parse(get(&format!("{}_B.SH", id.stem()))?)?,
            Shape::parse(get(&format!("{}_D.SH", id.stem()))?)?,
        ];
        if id == AircraftId::Faxx {
            for (body, fins) in bodies
                .iter_mut()
                .zip([&[0x3366, 0x3389][..], &[0x2ac0, 0x2ae3, 0x2c8b, 0x2cae][..]])
            {
                if body
                    .faces
                    .iter()
                    .filter(|f| fins.contains(&f.address))
                    .count()
                    != fins.len()
                {
                    return Err("unreviewed F/A-XX donor damage fins".into());
                }
                body.faces.retain(|f| !fins.contains(&f.address));
            }
        }
        let mut fragments = fragments;
        if id == AircraftId::Faxx {
            for body in bodies.iter_mut().chain(fragments.iter_mut()) {
                crate::additional_animation::concept_colors(&mut body.faces);
            }
        }
        let mut regions = BTreeMap::from([(
            format!("_{}.PIC", id.stem()),
            [atlas.width, atlas.height, 0],
        )]);
        let mut textures = Vec::new();
        for body in bodies.iter().chain(&fragments) {
            if body.faces.is_empty() {
                return Err("empty damaged aircraft body".into());
            }
            for face in &body.faces {
                if !face.texture.is_empty() && !regions.contains_key(&face.texture) {
                    let pic = Pic::parse(get(&face.texture)?)?;
                    if !pic.palette.is_empty() {
                        return Err("unreviewed damage atlas private palette".into());
                    }
                    regions.insert(face.texture.clone(), [pic.width, pic.height, 0]);
                    textures.push((face.texture.clone(), pic));
                }
            }
        }
        // Shared original dark damage patch, fitted onto the struck surface.
        if !regions.contains_key("_F18_A.PIC") {
            let pic = Pic::parse(get("_F18_A.PIC")?)?;
            if !pic.palette.is_empty() || pic.width != 256 || pic.height != 418 {
                return Err("unreviewed shared damage patch atlas".into());
            }
            regions.insert("_F18_A.PIC".into(), [pic.width, pic.height, 0]);
            textures.push(("_F18_A.PIC".into(), pic));
        }
        let width = textures
            .iter()
            .fold(atlas.width, |w, (_, p)| w.max(p.width));
        let height = atlas.height + textures.iter().map(|(_, p)| p.height).sum::<usize>();
        if width > 8192 || height > 8192 {
            return Err("damage atlas exceeds supported dimensions".into());
        }
        let mut pixels = vec![255; width * height];
        let mut mask = vec![false; width * height];
        let mut offset = 0;
        for (name, pic) in std::iter::once((format!("_{}.PIC", id.stem()), &*atlas))
            .chain(textures.iter().map(|(n, p)| (n.clone(), p)))
        {
            regions.get_mut(&name).unwrap()[2] = offset;
            for y in 0..pic.height {
                let dst = (y + offset) * width;
                pixels[dst..dst + pic.width]
                    .copy_from_slice(&pic.pixels[y * pic.width..(y + 1) * pic.width]);
                mask[dst..dst + pic.width]
                    .copy_from_slice(&pic.mask[y * pic.width..(y + 1) * pic.width]);
            }
            offset += pic.height;
        }
        atlas.width = width;
        atlas.height = height;
        atlas.pixels = pixels;
        atlas.mask = mask;
        Ok(Self {
            extents,
            bodies,
            fragments,
            regions,
        })
    }
    pub fn variant(id: AircraftId, section: Option<usize>) -> Option<usize> {
        tore_sim::combat::debris::damage_variant(id, section?)
    }

    /// Reviewed whole-part loss is reserved for a destroyed airframe.
    /// Surviving aircraft currently retain intact geometry and textures.
    pub fn body_variant(id: AircraftId, section: Option<usize>, damage: f64) -> Option<usize> {
        (damage >= 1.).then(|| Self::variant(id, section)).flatten()
    }
    /// Persistent fitted surface marks and progressive loss of wing/fin area.
    /// Work in source coordinates; animation has already placed this face.
    pub fn surfaces(&self, face: &Face, amounts: &[f64; 6], scale: f32) -> Vec<Face> {
        surfaces(face, amounts, scale, self.extents)
    }
}

fn surfaces(face: &Face, amounts: &[f64; 6], scale: f32, extents: [f32; 3]) -> Vec<Face> {
    let center: [f32; 3] = std::array::from_fn(|i| {
        face.positions.iter().map(|p| p[i]).sum::<f32>() / face.positions.len() as f32
    });
    let [right, forward, up] = center.map(|v| v * scale / 28.);
    let section = if forward > 0.28 && up > 0.18 && right.abs() < 0.28 {
        1
    } else if forward > 0.45 {
        0
    } else if center[1] < -extents[1] * 0.25 || (center[1] < 0. && center[2] > extents[2] * 0.35) {
        5
    } else if center[0] < -extents[0] * 0.30 {
        3
    } else if center[0] > extents[0] * 0.30 {
        4
    } else {
        2
    };
    let amount = amounts[section];
    let mut body = face.clone();
    // Cutting the polygons preserves interpolated source UVs and the opposite
    // wing. The slanted cut is a fitted tear, not a recovered breakup model.
    if amount >= 0.35 {
        let retained = if amount >= 0.75 { 0.52 } else { 0.82 };
        let cut = |p: [f32; 3]| match section {
            3 => extents[0] * retained + p[0] + p[1] * 0.08,
            4 => extents[0] * retained - p[0] + p[1] * 0.08,
            5 => extents[2] * retained - p[2] + p[0].abs() * 0.08,
            _ => 1.,
        };
        body = clip(&body, cut);
        if body.positions.len() < 3 {
            return Vec::new();
        }
    }
    if amount < 0.04 {
        return vec![body];
    }
    let density = if amount >= 0.35 {
        4
    } else if amount >= 0.15 {
        2
    } else {
        1
    };
    // Low-polygon aircraft may represent a whole wing with only two faces.
    // Always mark substantial panels so address sampling cannot hide all
    // damage on that section (notably the Su-27's flat-colored wings).
    let area = (1..face.positions.len() - 1)
        .map(|i| {
            let a: [f32; 3] = std::array::from_fn(|k| face.positions[i][k] - face.positions[0][k]);
            let b: [f32; 3] =
                std::array::from_fn(|k| face.positions[i + 1][k] - face.positions[0][k]);
            let cross = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            cross.iter().map(|v| v * v).sum::<f32>().sqrt() * 0.5 * scale * scale
        })
        .sum::<f32>();
    if area < 8. && face.address.wrapping_mul(2654435761) % 5 >= density {
        return vec![body];
    }
    let mut patch = body.clone();
    let center: [f32; 3] = std::array::from_fn(|i| {
        body.positions.iter().map(|p| p[i]).sum::<f32>() / body.positions.len() as f32
    });
    let normal = face.normal.unwrap_or([0., 1., 0.]);
    let normal = [normal[0], normal[2], normal[1]];
    let length = normal.iter().map(|v| v * v).sum::<f32>().sqrt().max(1.);
    let size = if amount >= 0.35 {
        0.58
    } else if amount >= 0.15 {
        0.40
    } else {
        0.22
    };
    for p in &mut patch.positions {
        for i in 0..3 {
            p[i] = center[i] + (p[i] - center[i]) * size + normal[i] / length * 0.04 / scale;
        }
    }
    // Source V is bottom-up. The crop is x=141..193, y=180..236 from top.
    // Rotate successive polygons' UVs for varied persistent damage patches.
    let uv = [[141., 182.], [193., 182.], [193., 238.], [141., 238.]];
    let turn = face.address % 4;
    patch.uv = (0..patch.positions.len())
        .map(|i| uv[(i + turn) % 4])
        .collect();
    patch.texture = "_F18_A.PIC".into();
    patch.subtype = 0x4c;
    patch.address = usize::MAX;
    vec![body, patch]
}

fn clip(face: &Face, distance: impl Fn([f32; 3]) -> f32) -> Face {
    let mut result = face.clone();
    result.positions.clear();
    result.colors.clear();
    result.uv.clear();
    for i in 0..face.positions.len() {
        let j = (i + 1) % face.positions.len();
        let a = face.positions[i];
        let b = face.positions[j];
        let da = distance(a);
        let db = distance(b);
        if da >= 0. {
            result.positions.push(a);
            result.colors.push(face.colors[i]);
            if !face.uv.is_empty() {
                result.uv.push(face.uv[i]);
            }
        }
        if (da < 0. && db > 0.) || (da > 0. && db < 0.) {
            let t = da / (da - db);
            result
                .positions
                .push(std::array::from_fn(|k| a[k] + t * (b[k] - a[k])));
            result.colors.push(face.colors[i]);
            if !face.uv.is_empty() {
                result.uv.push(std::array::from_fn(|k| {
                    face.uv[i][k] + t * (face.uv[j][k] - face.uv[i][k])
                }));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wing() -> Face {
        Face {
            positions: vec![
                [20., 0., 4.],
                [54., 0., 4.],
                [54., -12., 4.],
                [20., -12., 4.],
            ],
            colors: vec![1; 4],
            uv: vec![[0., 0.], [10., 0.], [10., 10.], [0., 10.]],
            texture: "synthetic".into(),
            fog: Default::default(),
            subtype: 0x4c,
            normal: Some([0., 1., 0.]),
            address: 5,
        }
    }
    #[test]
    fn missing_nose_body_requires_actual_destruction() {
        for damage in [0., 0.75, 0.92, 0.99, 0.99999] {
            assert_eq!(
                DamageArt::body_variant(AircraftId::F18, Some(0), damage),
                None
            );
        }
        assert_eq!(
            DamageArt::body_variant(AircraftId::F18, Some(0), 1.),
            Some(0)
        );
    }
    #[test]
    fn regional_surface_damage_keeps_opposite_wing_and_grows() {
        let face = wing();
        let mut damage = [0.; 6];
        damage[3] = 0.9;
        assert_eq!(
            surfaces(&face, &damage, 1. / 3., [55., 102., 30.])[0].positions,
            face.positions
        );
        damage[4] = 0.1;
        let light = surfaces(&face, &damage, 1. / 3., [55., 102., 30.]);
        assert_eq!(light.len(), 2);
        assert_eq!(light[0].positions, face.positions);
        assert_eq!(light[1].texture, "_F18_A.PIC");
        damage[4] = 0.8;
        let torn = surfaces(&face, &damage, 1. / 3., [55., 102., 30.]);
        assert!(torn[0].positions.iter().all(|p| p[0] <= 55. * 0.52 + 0.001));
        assert_eq!(torn[0].positions.len(), torn[0].uv.len());
    }
    #[test]
    fn light_marks_cover_flat_colored_aircraft_surfaces() {
        let mut face = wing();
        face.address = 0x1b7d; // A large panel rejected by the one-in-five sampler.
        face.uv.clear();
        face.texture.clear();
        let mut damage = [0.; 6];
        damage[4] = 0.1;
        let marked = surfaces(&face, &damage, 1. / 3., [55., 102., 30.]);
        assert_eq!(marked.len(), 2);
        assert_eq!(marked[0].positions, face.positions);
        assert!(marked[0].uv.is_empty());
        assert_eq!(marked[1].positions.len(), marked[1].uv.len());
        assert_eq!(marked[1].texture, "_F18_A.PIC");
    }

    #[test]
    fn original_breakup_never_substitutes_the_wrong_side() {
        assert_eq!(DamageArt::variant(AircraftId::F18, Some(3)), Some(1));
        assert_eq!(DamageArt::variant(AircraftId::F18, Some(4)), None);
        assert_eq!(DamageArt::variant(AircraftId::F18, Some(5)), None);
        assert_eq!(DamageArt::variant(AircraftId::Rafale, Some(5)), Some(1));
    }

    #[test]
    fn tail_damage_shortens_fin_without_cutting_nose_or_wing() {
        let mut fin = wing();
        fin.positions = vec![
            [8., -10., 4.],
            [8., -20., 30.],
            [8., -30., 30.],
            [8., -30., 4.],
        ];
        let mut damage = [0.; 6];
        damage[5] = 0.8;
        let output = surfaces(&fin, &damage, 1. / 3., [55., 102., 30.]);
        assert!(output[0].positions.iter().all(|p| p[2] <= 16.25));
        assert_eq!(output[0].positions.len(), output[0].uv.len());
        let wing = wing();
        assert_eq!(
            surfaces(&wing, &damage, 1. / 3., [55., 102., 30.])[0].positions,
            wing.positions
        );
        let mut nose = wing.clone();
        nose.positions.iter_mut().for_each(|p| {
            p[0] = 0.;
            p[1] += 80.;
        });
        assert_eq!(
            surfaces(&nose, &damage, 1. / 3., [55., 102., 30.])[0].positions,
            nose.positions
        );
    }
}
