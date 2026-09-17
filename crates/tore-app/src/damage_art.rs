//! Original alternate damage bodies with a shared, lossless texture atlas.
use crate::AppResult;
use std::collections::BTreeMap;
use tore_formats::{Pic, aircraft::AircraftId, shape::Shape};

pub struct DamageArt {
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
        let bodies = [
            Shape::parse(get(&format!("{}_A.SH", id.stem()))?)?,
            Shape::parse(get(&format!("{}_C.SH", id.stem()))?)?,
        ];
        let fragments = [
            Shape::parse(get(&format!("{}_B.SH", id.stem()))?)?,
            Shape::parse(get(&format!("{}_D.SH", id.stem()))?)?,
        ];
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
            bodies,
            fragments,
            regions,
        })
    }
    pub fn variant(id: AircraftId, fraction: f64) -> Option<usize> {
        (fraction >= 0.5).then(|| tore_sim::combat::debris::variant(id))
    }
}
