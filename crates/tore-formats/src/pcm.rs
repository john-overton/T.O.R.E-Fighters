//! Bounded unsigned PCM8 mono clips and lossless WAV wrapping; no synthesis.
use crate::{Result, invalid, slice, u16_at, u32_at};
pub struct Pcm<'a> {
    pub samples: &'a [u8],
    pub rate: u32,
}
impl<'a> Pcm<'a> {
    pub fn parse(name: &str, bytes: &'a [u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
            return Err(invalid("invalid PCM size"));
        }
        let clip = if bytes.starts_with(b"RIFF") {
            if slice(bytes, 8, 4)? != b"WAVE" || u32_at(bytes, 4)? + 8 != bytes.len() {
                return Err(invalid("invalid WAV length/type"));
            }
            let mut at = 12;
            let mut format = None;
            let mut samples = None;
            while at < bytes.len() {
                let tag = slice(bytes, at, 4)?;
                let len = u32_at(bytes, at + 4)?;
                let data = slice(bytes, at + 8, len)?;
                if tag == b"fmt " {
                    if format.is_some()
                        || len < 16
                        || u16_at(data, 0)? != 1
                        || u16_at(data, 2)? != 1
                        || u16_at(data, 12)? != 1
                        || u16_at(data, 14)? != 8
                        || u32_at(data, 8)? != u32_at(data, 4)?
                    {
                        return Err(invalid("unsupported WAV format; expected PCM8 mono"));
                    }
                    format = Some(u32_at(data, 4)? as u32);
                } else if tag == b"data" && samples.replace(data).is_some() {
                    return Err(invalid("duplicate WAV data"));
                }
                at += 8 + len;
                if at < bytes.len() {
                    at += len % 2;
                }
            }
            Self {
                samples: samples.ok_or_else(|| invalid("missing WAV samples"))?,
                rate: format.ok_or_else(|| invalid("missing WAV format"))?,
            }
        } else {
            let name = name.to_ascii_uppercase();
            let rate = if name.ends_with(".11K") {
                11025
            } else if name.ends_with(".5K") {
                5512
            } else {
                return Err(invalid("unknown PCM sample rate"));
            };
            Self {
                samples: bytes,
                rate,
            }
        };
        if clip.samples.is_empty() || !(4000..=96000).contains(&clip.rate) {
            return Err(invalid("invalid PCM rate/samples"));
        }
        Ok(clip)
    }
    pub fn wav(&self) -> Vec<u8> {
        let len = self.samples.len() as u32;
        let mut out = Vec::with_capacity(44 + len as usize + (len % 2) as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + len + len % 2).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        out.extend_from_slice(&self.rate.to_le_bytes());
        out.extend_from_slice(&self.rate.to_le_bytes());
        out.extend_from_slice(b"\x01\0\x08\0data");
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(self.samples);
        if !len.is_multiple_of(2) {
            out.push(0);
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wav_roundtrip_preserves_odd_samples_and_overrides_suffix() {
        let pcm = Pcm::parse("test.11k", &[0, 128, 255]).unwrap();
        let wav = pcm.wav();
        let decoded = Pcm::parse("test.5k", &wav).unwrap();
        assert_eq!(decoded.rate, 11025);
        assert_eq!(decoded.samples, pcm.samples);
        for n in 0..wav.len() {
            assert!(Pcm::parse("test.11k", &wav[..n]).is_err() || n < 4);
        }
        assert!(Pcm::parse("bad.11k", &[]).is_err());
    }
}
