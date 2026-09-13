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
    engine: Option<Voice>,
    burner: Option<Voice>,
    flight_on: bool,
    flight_paused: bool,
    engine_gain: f32,
    burner_gain: f32,
    voices: Vec<Voice>,
    music_on: bool,
    effects_on: bool,
}
pub struct Audio {
    _stream: cpal::Stream,
    mixer: Arc<Mutex<Mixer>>,
    clips: BTreeMap<String, Arc<Clip>>,
}
fn cue(action: Action) -> Option<&'static str> {
    match action {
        Action::Theater(_)
        | Action::Click
        | Action::QuickMission
        | Action::FreeFlight
        | Action::Back => Some("&BUTTON.11K"),
        Action::Music(_) | Action::Effects(_) => Some("&TOGGLE1.5K"),
        // Mouse hover and keyboard focus changes never play a sound.
        _ => None,
    }
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
            engine: None,
            burner: None,
            flight_on: false,
            flight_paused: false,
            engine_gain: 0.,
            burner_gain: 0.,
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
    pub fn control(&self, key: &str, state: &crate::flight::State) {
        let name = match key {
            "g" => {
                if state.gear_down {
                    "&GEARDWN.5K"
                } else {
                    "&GEARUP.5K"
                }
            }
            "f" => {
                if state.flaps_down {
                    "&FLAPOPN.5K"
                } else {
                    "&FLAPCLS.5K"
                }
            }
            "h" => "&HOOK.5K",
            _ => return,
        };
        if let Ok(mut m) = self.mixer.lock()
            && m.effects_on
            && m.voices.len() < 8
            && let Some(clip) = self.clips.get(name)
        {
            m.voices.push(Voice {
                clip: clip.clone(),
                position: 0.,
            });
        }
    }
    pub fn pause_flight(&self, paused: bool) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.flight_paused = paused;
        }
    }
    pub fn flight(
        &self,
        state: Option<(&tore_formats::aircraft::Aircraft, &crate::flight::State)>,
    ) {
        if let Ok(mut m) = self.mixer.lock() {
            m.flight_on = state.is_some();
            if let Some((a, s)) = state {
                if m.engine.is_none() {
                    m.engine = a
                        .sounds
                        .get("loopSound")
                        .and_then(|n| self.clips.get(n))
                        .map(|clip| Voice {
                            clip: clip.clone(),
                            position: 0.,
                        });
                    m.burner = a
                        .sounds
                        .get("secondSound")
                        .and_then(|n| self.clips.get(n))
                        .map(|clip| Voice {
                            clip: clip.clone(),
                            position: 0.,
                        });
                }
                if (m.engine_gain > 0.) != s.engine
                    && m.effects_on
                    && m.voices.len() < 8
                    && let Some(clip) = a
                        .sounds
                        .get(if s.engine {
                            "engineOnSound"
                        } else {
                            "engineOffSound"
                        })
                        .and_then(|n| self.clips.get(n))
                {
                    m.voices.push(Voice {
                        clip: clip.clone(),
                        position: 0.,
                    });
                }
                m.engine_gain = if s.engine {
                    0.08 + 0.15 * s.throttle as f32
                } else {
                    0.
                };
                m.burner_gain = if s.engine && s.burner && s.throttle > 0.95 {
                    0.15
                } else {
                    0.
                };
            }
        }
    }
    pub fn action(&self, action: Action) {
        let Ok(mut mixer) = self.mixer.lock() else {
            return;
        };
        match action {
            Action::Music(enabled) => {
                mixer.music_on = enabled;
            }
            Action::Effects(enabled) => {
                mixer.effects_on = enabled;
                if !enabled {
                    mixer.voices.clear();
                }
            }
            _ => {}
        }
        let name = cue(action);
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
                    && !mixer.flight_on
                    && let Some(music) = &mut mixer.music
                {
                    value += music.next(rate, true) * 0.16;
                }
                if mixer.flight_on && !mixer.flight_paused && mixer.effects_on {
                    let (eg, bg) = (mixer.engine_gain, mixer.burner_gain);
                    if let Some(v) = &mut mixer.engine {
                        value += v.next(rate, true) * eg;
                    }
                    if let Some(v) = &mut mixer.burner {
                        value += v.next(rate, true) * bg;
                    }
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
    fn hover_is_silent_and_activation_has_one_cue() {
        assert_eq!(cue(Action::Hover), None);
        assert_eq!(cue(Action::None), None);
        assert_eq!(cue(Action::Click), Some("&BUTTON.11K"));
    }
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
