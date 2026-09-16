//! User-supplied engine artwork. Runtime bytes, separate from retail palettes.
use std::{io::Read, path::PathBuf};
use tore_formats::aircraft::AircraftId;

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
impl Image {
    fn parse(bytes: &[u8]) -> crate::AppResult<Self> {
        if bytes.len() < 16 || &bytes[..8] != b"TORErgba" {
            return Err("invalid engine material header".into());
        }
        let width = u32::from_le_bytes(bytes[8..12].try_into()?);
        let height = u32::from_le_bytes(bytes[12..16].try_into()?);
        if !(1..=2048).contains(&width)
            || !(1..=2048).contains(&height)
            || bytes.len() != 16 + width as usize * height as usize * 4
        {
            return Err("invalid engine material dimensions or byte count".into());
        }
        Ok(Self {
            width,
            height,
            pixels: bytes[16..].to_vec(),
        })
    }
    pub fn load() -> crate::AppResult<Option<Self>> {
        let mut roots = Vec::new();
        if let Some(path) = std::env::var_os("TORE_ASSET_DIR") {
            roots.push(PathBuf::from(path));
        } else {
            if let Some(parent) = std::env::current_exe()?.parent() {
                roots.push(parent.join("assets"));
            }
            roots.push(std::env::current_dir()?.join("assets"));
            roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
        }
        for root in roots {
            let path = root.join("aircraft/engine-texture.rgba");
            match std::fs::File::open(&path) {
                Ok(file) => {
                    let mut bytes = Vec::new();
                    file.take(16 + 2048 * 2048 * 4 + 1)
                        .read_to_end(&mut bytes)?;
                    return Self::parse(&bytes).map(Some);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        eprintln!("Engine material not installed; retaining original nozzle presentation");
        Ok(None)
    }
    pub fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("User engine material"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4),
                rows_per_image: Some(self.height),
            },
            size,
        );
        texture.create_view(&Default::default())
    }
}

pub fn nozzle(id: AircraftId, address: usize) -> bool {
    match id {
        AircraftId::F18 => {
            crate::aircraft_animation::part(address) == crate::aircraft_animation::Part::Nozzle
        }
        AircraftId::Rafale => {
            crate::rafale_animation::part(address) == crate::rafale_animation::Part::Nozzle
        }
        AircraftId::F14 => matches!(address, 0x48a6 | 0x48d5 | 0x48fc | 0x491b),
        AircraftId::X31 => address == 0x29f7,
        AircraftId::A4E => false,
    }
}
pub fn heat(s: &crate::flight::State) -> f32 {
    if !s.engine || s.fuel <= 0. {
        0.
    } else if s.afterburner_active() {
        1.
    } else {
        s.throttle.clamp(0., 1.) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_image_and_power() {
        let mut bytes = b"TORErgba".to_vec();
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend([255, 0, 255, 255]);
        assert_eq!(Image::parse(&bytes).unwrap().pixels, [255, 0, 255, 255]);
        assert!(Image::parse(&bytes[..19]).is_err());
        bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Image::parse(&bytes).is_err());
        let mut s =
            crate::flight::State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        for power in [0., 0.25, 0.5, 1.] {
            s.throttle = power;
            assert_eq!(heat(&s), power as f32);
        }
        s.throttle = 0.96;
        s.burner = true;
        assert_eq!(heat(&s), 1.);
        s.burner = false;
        assert_eq!(heat(&s), 0.96);
        s.engine = false;
        assert_eq!(heat(&s), 0.);
        s.engine = true;
        s.fuel = 0.;
        assert_eq!(heat(&s), 0.);
        assert!(!nozzle(AircraftId::A4E, 0x29f7));
    }
}
