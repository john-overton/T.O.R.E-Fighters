//! Small PCM mixer: original samples, linear resampling, no external synth.
use crate::{AppResult, menu::Action};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

struct Clip {
    samples: Vec<f32>,
    rate: f64,
}
struct Voice {
    clip: Arc<Clip>,
    position: f64,
}
struct Mixer {
    music: Option<Voice>,
    voices: Vec<Voice>,
    music_on: bool,
    effects_on: bool,
}
pub struct Audio {
    _stream: cpal::Stream,
    mixer: Arc<Mutex<Mixer>>,
    clips: BTreeMap<String, Arc<Clip>>,
}
impl Voice {
    fn next(&mut self, rate: f64, looping: bool) -> f32 {
        if self.position >= self.clip.samples.len() as f64 {
            if looping {
                self.position %= self.clip.samples.len() as f64;
            } else {
                return 0.0;
            }
        }
        let at = self.position as usize;
        let next = if at + 1 < self.clip.samples.len() {
            at + 1
        } else if looping {
            0
        } else {
            at
        };
        let t = (self.position - at as f64) as f32;
        let value = self.clip.samples[at] * (1.0 - t) + self.clip.samples[next] * t;
        self.position += self.clip.rate / rate;
        value
    }
}
impl Audio {
    pub fn new(sounds: &BTreeMap<String, Vec<u8>>) -> AppResult<Self> {
        let clips = sounds
            .iter()
            .map(|(name, bytes)| {
                (
                    name.clone(),
                    Arc::new(Clip {
                        samples: bytes.iter().map(|b| (*b as f32 - 128.0) / 128.0).collect(),
                        rate: if name.ends_with(".5K") {
                            5512.0
                        } else {
                            11025.0
                        },
                    }),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let music = clips.get("AIR003.11K").map(|c| Voice {
            clip: c.clone(),
            position: 0.0,
        });
        let mixer = Arc::new(Mutex::new(Mixer {
            music,
            voices: Vec::with_capacity(8),
            music_on: true,
            effects_on: true,
        }));
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no default audio output device")?;
        let supported = device.default_output_config()?;
        let config = supported.config();
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(&device, &config, mixer.clone())?,
            cpal::SampleFormat::I16 => build::<i16>(&device, &config, mixer.clone())?,
            cpal::SampleFormat::U16 => build::<u16>(&device, &config, mixer.clone())?,
            _ => return Err("unsupported audio device sample format".into()),
        };
        stream.play()?;
        println!(
            "Audio: {} ({} Hz, {} channels)",
            device.name()?,
            config.sample_rate.0,
            config.channels
        );
        Ok(Self {
            _stream: stream,
            mixer,
            clips,
        })
    }
    pub fn action(&self, action: Action) {
        let Ok(mut mixer) = self.mixer.lock() else {
            return;
        };
        let name = match action {
            Action::Music(enabled) => {
                mixer.music_on = enabled;
                Some("&TOGGLE1.5K")
            }
            Action::Effects(enabled) => {
                mixer.effects_on = enabled;
                if !enabled {
                    mixer.voices.clear();
                }
                Some("&TOGGLE1.5K")
            }
            Action::Hover => Some("&CLICK.11K"),
            Action::Click => Some("&BUTTON.11K"),
            _ => None,
        };
        if mixer.effects_on
            && mixer.voices.len() < 8
            && let Some(clip) = name.and_then(|n| self.clips.get(n))
        {
            mixer.voices.push(Voice {
                clip: clip.clone(),
                position: 0.0,
            });
        }
    }
}
fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mixer: Arc<Mutex<Mixer>>,
) -> AppResult<cpal::Stream> {
    let (channels, rate) = (config.channels as usize, config.sample_rate.0 as f64);
    Ok(device.build_output_stream(
        config,
        move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
            let Ok(mut mixer) = mixer.try_lock() else {
                output.fill(T::from_sample(0.0));
                return;
            };
            for frame in output.chunks_mut(channels) {
                let mut value = 0.0;
                if mixer.music_on
                    && let Some(music) = &mut mixer.music
                {
                    value += music.next(rate, true) * 0.16;
                }
                for voice in &mut mixer.voices {
                    value += voice.next(rate, false) * 0.4;
                }
                frame.fill(T::from_sample(value.clamp(-1.0, 1.0)));
            }
            mixer
                .voices
                .retain(|v| v.position < v.clip.samples.len() as f64);
        },
        |error| eprintln!("Audio stream error: {error}"),
        None,
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_resampling_and_loop_boundaries() {
        let clip = Arc::new(Clip {
            samples: vec![0.0, 1.0],
            rate: 2.0,
        });
        let mut v = Voice {
            clip,
            position: 0.0,
        };
        assert_eq!(
            [v.next(4.0, false), v.next(4.0, false), v.next(4.0, false)],
            [0.0, 0.5, 1.0]
        );
        v.position = 2.0;
        assert_eq!(v.next(4.0, false), 0.0);
        assert_eq!(v.next(4.0, true), 0.0);
    }
}
