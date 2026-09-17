//! Small PCM mixer: original samples, linear resampling and a fitted seeker cue.
use crate::{AppResult, menu::Action};
pub mod music;
mod seeker;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

struct Clip {
    samples: Vec<u8>,
    rate: f64,
}
struct Voice {
    clip: Arc<Clip>,
    position: f64,
}
struct Mixer {
    seeker: seeker::Tone,
    seeker_volume: f64,
    music: music::Music,
    engine: Option<Voice>,
    engine_aircraft: Option<tore_formats::aircraft::AircraftId>,
    burner: Option<Voice>,
    stall: Option<Voice>,
    stall_cue: Option<&'static str>,
    flight_on: bool,
    flight_paused: bool,
    engine_gain: f32,
    burner_gain: f32,
    voices: Vec<Voice>,
    ui_voices: Vec<Voice>,
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
        | Action::Aircraft(_)
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
    fn finished(&self) -> bool {
        self.position >= self.clip.samples.len() as f64
    }
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
        let value = ((self.clip.samples[at] as f32 - 128.) * (1.0 - t)
            + (self.clip.samples[next] as f32 - 128.) * t)
            / 128.;
        self.position += self.clip.rate / rate;
        value
    }
}
impl Audio {
    pub fn new(
        sounds: BTreeMap<String, Vec<u8>>,
        scripts: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<Self> {
        let clips = sounds
            .into_iter()
            .map(|(name, bytes)| {
                let pcm = tore_formats::pcm::Pcm::parse(&name, &bytes)?;
                let rate = pcm.rate as f64;
                let samples = if pcm.samples.len() == bytes.len() {
                    bytes
                } else {
                    pcm.samples.to_vec()
                };
                Ok((name, Arc::new(Clip { samples, rate })))
            })
            .collect::<tore_formats::Result<BTreeMap<_, _>>>()?;
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let music = music::Music::new(&clips, scripts, seed);
        let seeker_volume = std::env::var("TORE_SEEKER_VOLUME")
            .ok()
            .map(|v| v.parse::<f64>())
            .transpose()?
            .unwrap_or(0.15);
        if !seeker_volume.is_finite() || !(0. ..=1.).contains(&seeker_volume) {
            return Err("TORE_SEEKER_VOLUME requires 0..1".into());
        }
        let mixer = Arc::new(Mutex::new(Mixer {
            seeker: seeker::Tone::default(),
            seeker_volume,
            music,
            engine: None,
            engine_aircraft: None,
            burner: None,
            stall: None,
            stall_cue: None,
            flight_on: false,
            flight_paused: false,
            engine_gain: 0.,
            burner_gain: 0.,
            voices: Vec::with_capacity(8),
            ui_voices: Vec::with_capacity(8),
            // Stay silent until the app has restored the user's saved preferences.
            music_on: false,
            effects_on: false,
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
    pub fn seeker(&self, state: Option<(f64, bool)>) {
        if let Ok(mut m) = self.mixer.lock() {
            m.seeker.target = state.map_or(0., |(gain, _)| gain.clamp(0., 1.)) * m.seeker_volume;
            m.seeker.ground = state.is_some_and(|(_, ground)| ground);
        }
    }
    pub fn combat(&self, names: &[&str]) {
        if let Ok(mut mixer) = self.mixer.lock()
            && mixer.effects_on
            && !mixer.flight_paused
        {
            for name in names {
                if mixer.voices.len() < 8
                    && let Some(clip) = self.clips.get(*name)
                {
                    mixer.voices.push(Voice {
                        clip: clip.clone(),
                        position: 0.,
                    });
                }
            }
        }
    }
    pub fn controls(&self, before: &crate::flight::State, after: &crate::flight::State) {
        if let Ok(mut m) = self.mixer.lock()
            && m.effects_on
            && !m.flight_paused
        {
            for name in actuator_cues(before, after).into_iter().flatten() {
                if m.voices.len() < 8
                    && let Some(clip) = self.clips.get(name)
                {
                    m.voices.push(Voice {
                        clip: clip.clone(),
                        position: 0.,
                    });
                }
            }
        }
    }
    pub fn scene(&self, scene: music::Scene) {
        if let Ok(mut m) = self.mixer.lock() {
            m.music.scene(scene);
        }
    }
    pub fn restart_flight(&self) {
        if let Ok(mut m) = self.mixer.lock() {
            m.music.scene(music::Scene::Score(0));
            m.music.restart();
            m.stall = None;
            m.stall_cue = None;
            m.engine = None;
            m.burner = None;
            m.engine_gain = 0.;
            m.burner_gain = 0.;
            m.voices.clear();
            m.flight_paused = false;
        }
    }
    pub fn pause_flight(&self, paused: bool) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.flight_paused = paused;
        }
    }
    pub fn flight(
        &self,
        state: Option<(
            &tore_formats::aircraft::Aircraft,
            &crate::flight::State,
            f64,
        )>,
    ) {
        if let Ok(mut m) = self.mixer.lock() {
            if state.is_none() && m.flight_on {
                m.seeker = seeker::Tone::default();
                m.stall = None;
                m.stall_cue = None;
                m.engine = None;
                m.burner = None;
                m.engine_gain = 0.;
                m.burner_gain = 0.;
                m.voices.clear();
            }
            m.flight_on = state.is_some();
            if let Some(fault) = m.music.fault.take() {
                eprintln!("Music stopped: {fault:?}; see import-report.txt for missing resources");
            }
            if let Some((a, s, ground)) = state {
                m.music.scene(music::Scene::Score(0));
                if m.engine_aircraft != Some(a.id) {
                    m.stall = None;
                    m.stall_cue = None;
                    m.engine = None;
                    m.burner = None;
                    m.engine_gain = 0.;
                    m.burner_gain = 0.;
                    m.voices.clear();
                    m.engine_aircraft = Some(a.id);
                }
                let alert = stall_cue(s.stall_alert(ground));
                if m.stall_cue != alert {
                    m.stall = alert
                        .and_then(|name| self.clips.get(name))
                        .map(|clip| Voice {
                            clip: clip.clone(),
                            position: 0.,
                        });
                    m.stall_cue = alert;
                }
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
                m.burner_gain = if s.afterburner_active() { 0.15 } else { 0. };
            }
        }
    }
    /// Restore user preferences without synthesizing a menu click at startup.
    pub fn preferences(&self, music: bool, effects: bool) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.music_on = music;
            mixer.effects_on = effects;
            if !effects {
                mixer.voices.clear();
                mixer.ui_voices.clear();
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
                    mixer.ui_voices.clear();
                    mixer.voices.clear();
                }
            }
            _ => {}
        }
        let name = cue(action);
        if mixer.effects_on
            && mixer.ui_voices.len() < 8
            && let Some(clip) = name.and_then(|n| self.clips.get(n))
        {
            mixer.ui_voices.push(Voice {
                clip: clip.clone(),
                position: 0.0,
            });
        }
    }
}
fn actuator_cues(
    before: &crate::flight::State,
    after: &crate::flight::State,
) -> [Option<&'static str>; 4] {
    [
        (before.gear_down != after.gear_down).then_some(if after.gear_down {
            "&GEARDWN.5K"
        } else {
            "&GEARUP.5K"
        }),
        (before.flaps_down != after.flaps_down).then_some(if after.flaps_down {
            "&FLAPOPN.5K"
        } else {
            "&FLAPCLS.5K"
        }),
        (before.hook_down != after.hook_down).then_some("&HOOK.5K"),
        (before.brake_out != after.brake_out).then_some(brake_cue(
            after.brake_out,
            after.research.as_ref().is_some_and(|r| r.on_ground),
        )),
    ]
}
fn brake_cue(deployed: bool, on_ground: bool) -> &'static str {
    if !deployed {
        "&FLAPCLS.5K"
    } else if on_ground {
        "&SQUEAL.5K"
    } else {
        "&FLAPOPN.5K"
    }
}
fn stall_cue(
    mode: Option<tore_formats::flight_model::departure::DepartureMode>,
) -> Option<&'static str> {
    use tore_formats::flight_model::departure::DepartureMode::*;
    match mode {
        Some(Warning | ExtendedWarning) => Some("&STALLWR.5K"),
        Some(Stalled | Spinning) => Some("&STALL.5K"),
        _ => None,
    }
}
impl Mixer {
    fn sample(&mut self, rate: f64) -> f32 {
        let mut value = self.seeker.sample(
            rate,
            self.flight_on && !self.flight_paused && self.effects_on,
        );
        if self.music_on && !(self.flight_on && self.flight_paused) {
            value += self.music.next(rate) * 0.16;
        }
        if self.flight_on && !self.flight_paused && self.effects_on {
            if let Some(v) = &mut self.stall {
                value += v.next(rate, true) * 0.4;
            }
            if let Some(v) = &mut self.engine {
                value += v.next(rate, true) * self.engine_gain;
            }
            if let Some(v) = &mut self.burner {
                value += v.next(rate, true) * self.burner_gain;
            }
        }
        if self.effects_on && !(self.flight_on && self.flight_paused) {
            for voice in &mut self.voices {
                value += voice.next(rate, false) * 0.4;
            }
        }
        if self.effects_on {
            for voice in &mut self.ui_voices {
                value += voice.next(rate, false) * 0.4;
            }
        }
        value.clamp(-1., 1.)
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
                frame.fill(T::from_sample(mixer.sample(rate)));
            }
            mixer.voices.retain(|v| !v.finished());
            mixer.ui_voices.retain(|v| !v.finished());
        },
        |error| eprintln!("Audio stream error: {error}"),
        None,
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stall_cues_and_pause_mute_use_a_dedicated_loop() {
        use tore_formats::flight_model::departure::DepartureMode::*;
        assert_eq!(stall_cue(Some(Warning)), Some("&STALLWR.5K"));
        assert_eq!(stall_cue(Some(ExtendedWarning)), Some("&STALLWR.5K"));
        assert_eq!(stall_cue(Some(Stalled)), Some("&STALL.5K"));
        assert_eq!(stall_cue(Some(Spinning)), Some("&STALL.5K"));
        assert_eq!(stall_cue(Some(Normal)), None);
        let mut m = Mixer {
            seeker: seeker::Tone::default(),
            seeker_volume: 0.15,
            music: music::Music::new(&BTreeMap::new(), &BTreeMap::new(), 1),
            engine: None,
            engine_aircraft: None,
            burner: None,
            stall: Some(Voice {
                clip: Arc::new(Clip {
                    samples: vec![192; 4],
                    rate: 4.,
                }),
                position: 0.,
            }),
            stall_cue: Some("&STALL.5K"),
            flight_on: true,
            flight_paused: false,
            engine_gain: 0.,
            burner_gain: 0.,
            voices: Vec::new(),
            ui_voices: Vec::new(),
            music_on: false,
            effects_on: true,
        };
        for _ in 0..12 {
            assert!((m.sample(4.) - 0.2).abs() < 1e-6);
        }
        m.flight_paused = true;
        let position = m.stall.as_ref().unwrap().position;
        assert_eq!(m.sample(4.), 0.);
        assert_eq!(m.stall.as_ref().unwrap().position, position);
        m.flight_paused = false;
        m.effects_on = false;
        assert_eq!(m.sample(4.), 0.);
        m.effects_on = true;
        m.stall = None;
        assert_eq!(m.sample(4.), 0.);
    }
    #[test]
    fn both_aircraft_emit_brake_cues_only_on_actual_state_changes() {
        use tore_formats::aircraft::AircraftId;
        use tore_input::{PilotCommand, PilotInput, Switch};
        for id in AircraftId::ALL {
            let mut profile = crate::flight::animation_tests::profile();
            profile.id = id;
            profile.name = match id {
                AircraftId::F18 => "F/A-18D",
                AircraftId::Rafale => "RAFALE",
                AircraftId::F14 => "F-14",
                AircraftId::A4E => "A-4E",
                AircraftId::X31 => "X-31",
                AircraftId::Mig29 => "MiG-29",
                AircraftId::Su27 => "Su-27",
                AircraftId::Mig21 => "MiG-21",
                AircraftId::Su25 => "Su-25",
                AircraftId::Mig23 => "MiG-23",
                AircraftId::Su35 => "Su-35",
                AircraftId::F22 => "F-22",
            }
            .into();
            profile.shape = format!("{}.SH", id.stem());
            let mut state = crate::flight::State::new(&profile, [0., 10000., 0.]).unwrap();
            let before = state.clone();
            let mut input = PilotInput::default();
            input
                .commands
                .push(PilotCommand::Set(Switch::Airbrake, true));
            state.step(&input, |_, _| 0.);
            assert_eq!(
                actuator_cues(&before, &state),
                [None, None, None, Some("&FLAPOPN.5K")]
            );
            let before = state.clone();
            state.step(&input, |_, _| 0.);
            assert_eq!(actuator_cues(&before, &state), [None; 4]);
            input.commands = vec![PilotCommand::Set(Switch::Burner, true)];
            let before = state.clone();
            state.step(&input, |_, _| 0.);
            assert_eq!(actuator_cues(&before, &state), [None; 4]);
            input.commands = vec![PilotCommand::Set(Switch::Airbrake, false)];
            let before = state.clone();
            state.step(&input, |_, _| 0.);
            assert_eq!(
                actuator_cues(&before, &state),
                [None, None, None, Some("&FLAPCLS.5K")]
            );
        }
        assert_eq!(brake_cue(true, true), "&SQUEAL.5K");
    }
    #[test]
    fn hover_is_silent_and_activation_has_one_cue() {
        assert_eq!(cue(Action::Hover), None);
        assert_eq!(cue(Action::None), None);
        assert_eq!(cue(Action::Click), Some("&BUTTON.11K"));
        assert_eq!(cue(Action::Aircraft(1)), Some("&BUTTON.11K"));
    }
    #[test]
    fn pcm_resampling_and_loop_boundaries() {
        let clip = Arc::new(Clip {
            samples: vec![128, 192],
            rate: 2.0,
        });
        let mut v = Voice {
            clip,
            position: 0.0,
        };
        assert_eq!(
            [v.next(4.0, false), v.next(4.0, false), v.next(4.0, false)],
            [0.0, 0.25, 0.5]
        );
        v.position = 2.0;
        assert_eq!(v.next(4.0, false), 0.0);
        assert_eq!(v.next(4.0, true), 0.0);
    }
}
