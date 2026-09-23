//! Original PCM, local avionics and physically delayed spatial effects.
use crate::{AppResult, menu::Action};
pub mod music;
mod seeker;
pub mod situation;
mod spatial;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::{BTreeMap, VecDeque},
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RadioSource {
    Wing,
    Airport,
    Ejection,
}
/// Queued recordings for composed calls. A contact report alone can be a
/// dozen recordings; wing orders keep their own limit of 16.
const SPEECH_QUEUE: usize = 48;
struct RadioVoice {
    source: RadioSource,
    voice: Voice,
}
struct Mixer {
    spatial: spatial::Scene,
    seeker: seeker::Tone,
    seeker_voice: Option<Voice>,
    seeker_cue: Option<&'static str>,
    seeker_volume: f64,
    music: music::Music,
    situation: situation::Selector,
    engine: Option<Voice>,
    engine_aircraft: Option<tore_formats::aircraft::AircraftId>,
    burner: Option<Voice>,
    stall: Option<Voice>,
    stall_cue: Option<&'static str>,
    flight_on: bool,
    flight_paused: bool,
    ejection_warning: bool,
    engine_gain: f32,
    burner_gain: f32,
    voices: Vec<Voice>,
    ui_voices: Vec<Voice>,
    radio: VecDeque<RadioVoice>,
    music_on: bool,
    effects_on: bool,
}
pub struct Audio {
    _stream: cpal::Stream,
    mixer: Arc<Mutex<Mixer>>,
    clips: BTreeMap<String, Arc<Clip>>,
    radio_phrases: BTreeMap<String, String>,
}
/// Presentation-only observations. No aircraft or missile control is changed.
pub fn spatial_sources(
    combat: &tore_sim::combat::live::State,
    player: &crate::flight::State,
) -> Vec<tore_sim::acoustics::Source> {
    use tore_sim::acoustics::{Source, SourceId};
    let mut sources = vec![Source {
        id: SourceId::Aircraft(0),
        position: player.position,
        velocity: player.velocity,
    }];
    sources.extend(
        combat
            .targets
            .iter()
            .filter(|t| t.airborne)
            .map(|t| Source {
                id: SourceId::Aircraft(t.id),
                position: t.position,
                velocity: t.velocity,
            }),
    );
    sources.extend(
        combat
            .projectiles
            .iter()
            .filter(|p| {
                tore_sim::combat::missiles::Profile::for_weapon(p.weapon(combat.configuration()))
                    .is_some()
            })
            .map(|p| Source {
                id: SourceId::Missile(p.id),
                position: p.position,
                velocity: std::array::from_fn(|i| (p.position[i] - p.previous[i]) * 120.),
            }),
    );
    sources
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
        Action::RockerUp => Some("&ROCKUP.11K"),
        Action::RockerDown => Some("&ROCKDN.11K"),
        Action::OrdnanceWeapon => Some("&ARMWPN.5K"),
        Action::OrdnanceAmmunition => Some("&ARMBLLT.5K"),
        Action::OrdnanceFuel => Some("&ARMDRIP.11K"),
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
        resources: &BTreeMap<String, Vec<u8>>,
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
            .unwrap_or(0.30);
        if !seeker_volume.is_finite() || !(0. ..=1.).contains(&seeker_volume) {
            return Err("TORE_SEEKER_VOLUME requires 0..1".into());
        }
        let mixer = Arc::new(Mutex::new(Mixer {
            spatial: spatial::Scene::default(),
            seeker: seeker::Tone::default(),
            seeker_voice: None,
            seeker_cue: None,
            seeker_volume,
            music,
            situation: situation::Selector::default(),
            engine: None,
            engine_aircraft: None,
            burner: None,
            stall: None,
            stall_cue: None,
            flight_on: false,
            flight_paused: false,
            ejection_warning: false,
            engine_gain: 0.,
            burner_gain: 0.,
            voices: Vec::with_capacity(8),
            ui_voices: Vec::with_capacity(8),
            radio: VecDeque::new(),
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
        log::info!(
            "Audio: {} ({} Hz, {} channels)",
            device.name()?,
            config.sample_rate.0,
            config.channels
        );
        let radio_phrases: BTreeMap<_, _> = tore_formats::radio::STEMS
            .iter()
            .filter_map(|(stem, _)| {
                let bytes = resources.get(&format!("TORE_RADIO_{stem}"))?;
                if bytes.is_empty()
                    || bytes.len() > 127
                    || !bytes.iter().all(|b| (32..127).contains(b))
                {
                    return None;
                }
                Some((stem.to_string(), String::from_utf8(bytes.clone()).ok()?))
            })
            .collect();
        let missing_ejection: Vec<_> = [
            "^EJECTX3.5K",
            "^EJECTNG.5K",
            "^PUNCH.5K",
            "&EJECT.5K",
            "&CHUTE.5K",
        ]
        .into_iter()
        .filter(|name| !clips.contains_key(*name))
        .collect();
        if !missing_ejection.is_empty() {
            log::warn!(
                "Optional ejection audio unavailable: {}. Reimport retail media; pilot simulation remains available.",
                missing_ejection.join(", ")
            );
        }
        let missing = missing_airport_audio(&clips, &radio_phrases);
        if !missing.is_empty() {
            log::warn!(
                "Optional airport radio unavailable for {}. Reimport retail media to refresh the cache; tower text remains available.",
                missing.join(", ")
            );
        }
        Ok(Self {
            _stream: stream,
            mixer,
            clips,
            radio_phrases,
        })
    }
    /// A new order supersedes pending radio, never flight execution.
    pub fn radio(&self, stems: &[&str], interrupt: bool) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.enqueue_radio(
                &self.clips,
                &self.radio_phrases,
                stems,
                RadioSource::Wing,
                interrupt,
            );
        }
    }

    /// Transition-owned cockpit/seat audio. Speaker assignment is fitted, see the spec.
    pub fn ejection(
        &self,
        before: &crate::flight::State,
        after: &crate::flight::State,
        danger: bool,
    ) {
        use tore_sim::ejection::Phase;
        if let Ok(mut mixer) = self.mixer.lock() {
            if mixer.flight_paused {
                return;
            }
            if danger
                && !mixer.ejection_warning
                && after.escape.is_none()
                && !after.systems.pilot.dead
            {
                mixer.escape_voice(&self.clips, "^EJECTX3.5K", true);
            }
            mixer.ejection_warning = danger;
            if before.escape.is_none() && after.escape.is_some() {
                mixer.escape_voice(&self.clips, "^EJECTNG.5K", true);
                mixer.escape_effect(&self.clips, "&EJECT.5K");
            }
            if after
                .escape
                .as_ref()
                .is_some_and(|p| p.phase == Phase::Inflating)
                && before
                    .escape
                    .as_ref()
                    .is_none_or(|p| p.phase != Phase::Inflating)
            {
                mixer.escape_effect(&self.clips, "&CHUTE.5K");
            }
        }
    }
    /// One delivered radio or crew line from [`crate::comms`]. Recordings play
    /// in order after anything already queued; missing recordings are skipped.
    /// A line that would not fit the queue is dropped whole, never truncated.
    pub fn speech(&self, stems: &[String]) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.enqueue_speech(&self.clips, stems);
        }
    }
    /// A recording played directly rather than over the radio, such as the
    /// player's death scream.
    pub fn direct_voice(&self, stem: &str) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.escape_effect(&self.clips, &format!("{stem}.5K"));
        }
    }
    pub fn wingman_ejected(&self) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.escape_voice(&self.clips, "^PUNCH.5K", false);
        }
    }
    /// Airport speech supersedes stale airport speech without cancelling wing radio.
    pub fn airport_radio(&self, stems: &[&str]) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.enqueue_radio(
                &self.clips,
                &self.radio_phrases,
                stems,
                RadioSource::Airport,
                true,
            );
        }
    }

    pub fn cancel_airport_radio(&self) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.cancel_radio(RadioSource::Airport);
        }
    }

    pub fn seeker(&self, state: Option<tore_sim::combat::live::SeekerTone>) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.set_seeker(&self.clips, state);
        }
    }
    /// Exactly one simulation tick, independent of rendering and the device clock.
    pub fn spatial_tick(
        &self,
        listener: tore_sim::acoustics::Listener,
        sources: &[tore_sim::acoustics::Source],
        emissions: &[tore_sim::acoustics::Emission],
        releases: &[&str],
        player_position: [f64; 3],
    ) {
        if let Ok(mut mixer) = self.mixer.lock() {
            if mixer.flight_paused {
                return;
            }
            let enabled = mixer.effects_on;
            if enabled {
                for name in releases {
                    if let Some(clip) = self.clips.get(*name) {
                        if listener.external {
                            mixer
                                .spatial
                                .weapon(clip.clone(), player_position, listener);
                        } else if mixer.voices.len() < 8 {
                            mixer.voices.push(Voice {
                                clip: clip.clone(),
                                position: 0.,
                            });
                        }
                    }
                }
            }
            mixer
                .spatial
                .tick(&self.clips, listener, sources, emissions, enabled);
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
            // Silent until the first fixed step chooses the situation score.
            m.situation.new_flight();
            m.music.stop();
            m.spatial.clear();
            m.seeker = seeker::Tone::default();
            m.seeker_voice = None;
            m.seeker_cue = None;
            m.stall = None;
            m.stall_cue = None;
            m.engine = None;
            m.burner = None;
            m.engine_gain = 0.;
            m.burner_gain = 0.;
            m.voices.clear();
            m.radio.clear();
            m.flight_paused = false;
            m.ejection_warning = false;
        }
    }
    /// One fixed flight step of situation music, in game seconds since the
    /// flight started. Called only while the simulation advances, so a paused
    /// flight chooses nothing. Audio never feeds back into the simulation.
    pub fn situation(&self, inputs: &situation::Inputs, now: f64) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.situation(inputs, now);
        }
    }
    /// Ctrl+V: toggles the Valkyries score for the session and stops the
    /// current score. Returns the new state.
    pub fn toggle_valkyries(&self) -> Option<bool> {
        let mut m = self.mixer.lock().ok()?;
        let on = m.situation.toggle_valkyries();
        m.music.stop();
        Some(on)
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
                m.spatial.clear();
                m.seeker = seeker::Tone::default();
                m.seeker_voice = None;
                m.seeker_cue = None;
                m.stall = None;
                m.stall_cue = None;
                m.engine = None;
                m.burner = None;
                m.engine_gain = 0.;
                m.burner_gain = 0.;
                m.voices.clear();
                m.radio.clear();
            }
            m.flight_on = state.is_some();
            if let Some(fault) = m.music.new_fault() {
                eprintln!("Music stopped: {fault:?}; see import-report.txt for missing resources");
            }
            if let Some((a, s, ground)) = state {
                if m.engine_aircraft != Some(a.id) {
                    m.stall = None;
                    m.stall_cue = None;
                    m.engine = None;
                    m.burner = None;
                    m.engine_gain = 0.;
                    m.burner_gain = 0.;
                    m.voices.clear();
                    m.radio.clear();
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
                if s.escape.is_none()
                    && (m.engine_gain > 0.) != s.engine
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
                m.engine_gain = if s.engine && s.escape.is_none() {
                    0.08 + 0.15 * s.throttle as f32
                } else {
                    0.
                };
                m.burner_gain = if s.afterburner_active() && s.escape.is_none() {
                    0.15
                } else {
                    0.
                };
            }
        }
    }
    /// Restore user preferences without synthesizing a menu click at startup.
    pub fn preferences(&self, music: bool, effects: bool) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.music_on = music;
            mixer.effects_on = effects;
            if !effects {
                mixer.spatial.clear();
                mixer.voices.clear();
                mixer.radio.clear();
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
                    mixer.spatial.clear();
                    mixer.ui_voices.clear();
                    mixer.voices.clear();
                    mixer.radio.clear();
                }
            }
            _ => {}
        }
        let name = cue(action);
        if let Some(clip) = name.and_then(|n| self.clips.get(n)) {
            mixer.play_ui(clip, action == Action::OrdnanceFuel);
        }
    }
}
impl Mixer {
    fn set_seeker(
        &mut self,
        clips: &BTreeMap<String, Arc<Clip>>,
        state: Option<tore_sim::combat::live::SeekerTone>,
    ) {
        self.seeker.target =
            state.map_or(0., |cue| cue.strength.clamp(0., 1.)) * self.seeker_volume;
        self.seeker.ground = state.is_some_and(|cue| cue.ground);
        self.seeker.radar = state.is_some_and(|cue| cue.radar);
        self.seeker.locked = state.is_some_and(|cue| cue.locked);
        if let Some(state) = state {
            let cue = match (state.radar || state.ground, state.locked) {
                (false, _) => "&IR1.11K",
                (true, false) => "&RDRTRY.5K",
                (true, true) => "&RDRLOCK.5K",
            };
            if self.seeker_cue != Some(cue) {
                self.seeker_cue = Some(cue);
                self.seeker_voice = clips.get(cue).map(|clip| Voice {
                    clip: clip.clone(),
                    position: 0.,
                });
            }
        }
    }
}
impl Mixer {
    fn situation(&mut self, inputs: &situation::Inputs, now: f64) {
        if !self.music_on {
            self.situation.silence();
            self.music.set_hold(false);
            return;
        }
        if self.situation.due(now) {
            let playback = self.music.playback();
            if let Some(rank) = self.situation.update(now, inputs, playback) {
                self.music.start(rank.score());
            }
        }
        self.music.set_hold(self.situation.waiting(inputs));
    }
    fn play_ui(&mut self, clip: &Arc<Clip>, reuse_active: bool) {
        if self.effects_on
            && self.ui_voices.len() < 8
            && !(reuse_active
                && self
                    .ui_voices
                    .iter()
                    .any(|voice| Arc::ptr_eq(&voice.clip, clip) && !voice.finished()))
        {
            self.ui_voices.push(Voice {
                clip: clip.clone(),
                position: 0.,
            });
        }
    }
}
fn missing_airport_audio(
    clips: &BTreeMap<String, Arc<Clip>>,
    phrases: &BTreeMap<String, String>,
) -> Vec<&'static str> {
    [
        tore_formats::radio::AIRPORT_CLEAR_TO_LAND,
        tore_formats::radio::AIRPORT_WELCOME_HOME,
    ]
    .into_iter()
    .filter(|stem| !phrases.contains_key(*stem) || !clips.contains_key(&format!("{stem}.5K")))
    .collect()
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
    fn escape_voice(&mut self, clips: &BTreeMap<String, Arc<Clip>>, name: &str, urgent: bool) {
        if !self.effects_on || self.flight_paused {
            return;
        }
        if urgent {
            self.cancel_radio(RadioSource::Ejection);
        }
        if self.radio.len() < 16
            && let Some(clip) = clips.get(name)
        {
            let voice = RadioVoice {
                source: RadioSource::Ejection,
                voice: Voice {
                    clip: clip.clone(),
                    position: 0.,
                },
            };
            if urgent {
                self.radio.push_front(voice);
            } else {
                self.radio.push_back(voice);
            }
        }
    }
    fn escape_effect(&mut self, clips: &BTreeMap<String, Arc<Clip>>, name: &str) {
        if self.effects_on
            && !self.flight_paused
            && self.voices.len() < 8
            && let Some(clip) = clips.get(name)
        {
            self.voices.push(Voice {
                clip: clip.clone(),
                position: 0.,
            });
        }
    }
    fn enqueue_radio(
        &mut self,
        clips: &BTreeMap<String, Arc<Clip>>,
        phrases: &BTreeMap<String, String>,
        stems: &[&str],
        source: RadioSource,
        interrupt: bool,
    ) {
        if interrupt {
            self.cancel_radio(source);
        }
        if !self.effects_on || self.flight_paused {
            return;
        }
        for stem in stems {
            if self.radio.len() >= 16 {
                break;
            }
            if phrases.contains_key(*stem)
                && let Some(clip) = clips.get(&format!("{stem}.5K"))
            {
                self.radio.push_back(RadioVoice {
                    source,
                    voice: Voice {
                        clip: clip.clone(),
                        position: 0.,
                    },
                });
            }
        }
    }
    fn enqueue_speech(&mut self, clips: &BTreeMap<String, Arc<Clip>>, stems: &[String]) {
        if !self.effects_on || self.flight_paused {
            return;
        }
        let voices: Vec<_> = stems
            .iter()
            .filter_map(|stem| clips.get(&format!("{stem}.5K")))
            .map(|clip| RadioVoice {
                source: RadioSource::Wing,
                voice: Voice {
                    clip: clip.clone(),
                    position: 0.,
                },
            })
            .collect();
        if self.radio.len() + voices.len() <= SPEECH_QUEUE {
            self.radio.extend(voices);
        }
    }
    fn cancel_radio(&mut self, source: RadioSource) {
        self.radio.retain(|voice| voice.source != source);
    }
    fn frame(&mut self, rate: f64) -> [f32; 2] {
        let local = self.sample(rate);
        let spatial = if self.effects_on && self.flight_on && !self.flight_paused {
            self.spatial.sample(rate)
        } else {
            [0.; 2]
        };
        spatial.map(|v| (local + v).clamp(-1., 1.))
    }
    fn sample(&mut self, rate: f64) -> f32 {
        let mut value = self.seeker.sample(
            rate,
            self.flight_on && !self.flight_paused && self.effects_on,
        );
        if let Some(voice) = &mut self.seeker_voice {
            value = if self.flight_on && !self.flight_paused && self.effects_on {
                voice.next(rate, true) * self.seeker.gain as f32
            } else {
                0.
            };
        }
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
            if let Some(voice) = self.radio.front_mut() {
                value += voice.voice.next(rate, false) * 0.4;
                if voice.voice.finished() {
                    self.radio.pop_front();
                }
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
                let stereo = mixer.frame(rate);
                frame.fill(T::from_sample((stereo[0] + stereo[1]) * 0.5));
                if frame.len() >= 2 {
                    frame[0] = T::from_sample(stereo[0]);
                    frame[1] = T::from_sample(stereo[1]);
                }
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
    fn spatial_mixer_pause_preserves_pcm_without_catch_up() {
        use tore_sim::acoustics::{Emission, Kind, Listener};
        let clips = BTreeMap::from([(
            "&EXPL12.5K".into(),
            Arc::new(Clip {
                samples: vec![192; 8000],
                rate: 8000.,
            }),
        )]);
        let mut paused = test_mixer();
        let mut reference = test_mixer();
        for m in [&mut paused, &mut reference] {
            m.stall = None;
            m.spatial.tick(
                &clips,
                Listener {
                    position: [0.; 3],
                    right: [1., 0., 0.],
                    view: 0,
                    external: false,
                },
                &[],
                &[Emission {
                    kind: Kind::Explosion,
                    position: [1., 0., 0.],
                    arrived: false,
                }],
                true,
            );
        }
        for _ in 0..80 {
            assert_eq!(paused.frame(8000.), reference.frame(8000.));
        }
        paused.flight_paused = true;
        for _ in 0..8000 {
            assert_eq!(paused.frame(8000.), [0.; 2]);
        }
        paused.flight_paused = false;
        for _ in 0..80 {
            assert_eq!(paused.frame(8000.), reference.frame(8000.));
        }
    }
    #[test]
    fn ir_recording_keeps_playhead_on_lock_and_volume_tracks_hud_percent() {
        use tore_sim::combat::live::SeekerTone;
        let clips = ["&IR1.11K", "&RDRTRY.5K", "&RDRLOCK.5K"]
            .into_iter()
            .map(|name| {
                (
                    name.to_string(),
                    Arc::new(Clip {
                        samples: vec![192; 8000],
                        rate: 8000.,
                    }),
                )
            })
            .collect();
        let mut m = test_mixer();
        m.stall = None;
        for (percent, expected) in [(0, 0.075), (50, 0.2875), (100, 0.5)] {
            let state = SeekerTone {
                strength: SeekerTone::ir_strength(percent, false),
                ground: false,
                radar: false,
                locked: false,
            };
            assert!((state.strength - expected).abs() < 1e-9);
            m.set_seeker(&clips, Some(state));
            assert_eq!(m.seeker_cue, Some("&IR1.11K"));
            for _ in 0..800 {
                m.sample(8000.);
            }
            assert!((m.seeker.gain - expected * 0.30).abs() < 1e-9);
            let position = m.seeker_voice.as_ref().unwrap().position;
            m.set_seeker(
                &clips,
                Some(SeekerTone {
                    strength: SeekerTone::ir_strength(percent, true),
                    locked: true,
                    ..state
                }),
            );
            assert_eq!(m.seeker_voice.as_ref().unwrap().position, position);
            for _ in 0..800 {
                m.sample(8000.);
            }
            assert!((m.seeker.gain - expected * 2. * 0.30).abs() < 1e-9);
        }
        m.set_seeker(
            &clips,
            Some(SeekerTone {
                strength: 1.,
                ground: true,
                radar: false,
                locked: true,
            }),
        );
        assert_eq!(m.seeker_cue, Some("&RDRLOCK.5K"));
        m.set_seeker(&clips, None);
        for _ in 0..800 {
            m.sample(8000.);
        }
        assert_eq!(m.seeker.gain, 0.);
    }
    #[test]
    fn ejection_audio_is_optional_serial_and_pause_aware() {
        let mut mixer = test_mixer();
        mixer.effects_on = true;
        mixer.flight_on = true;
        let clip = Arc::new(Clip {
            samples: vec![150; 80],
            rate: 8000.,
        });
        let clips = BTreeMap::from([("voice".into(), clip.clone()), ("seat".into(), clip)]);
        mixer.escape_voice(&clips, "missing", true);
        assert!(mixer.radio.is_empty());
        mixer.escape_voice(&clips, "voice", false);
        mixer.escape_voice(&clips, "voice", true);
        assert_eq!(
            mixer.radio.len(),
            1,
            "urgent cockpit speech replaces stale ejection speech"
        );
        mixer.escape_effect(&clips, "seat");
        assert_eq!(mixer.voices.len(), 1);
        mixer.flight_paused = true;
        mixer.escape_voice(&clips, "voice", false);
        mixer.escape_effect(&clips, "seat");
        assert_eq!(mixer.radio.len(), 1);
        assert_eq!(mixer.voices.len(), 1);
        let before = mixer.radio.front().unwrap().voice.position;
        mixer.sample(8000.);
        assert_eq!(mixer.radio.front().unwrap().voice.position, before);
    }
    fn test_mixer() -> Mixer {
        Mixer {
            spatial: spatial::Scene::default(),
            seeker: seeker::Tone::default(),
            seeker_voice: None,
            seeker_cue: None,
            seeker_volume: 0.30,
            music: music::Music::new(&BTreeMap::new(), &BTreeMap::new(), 1),
            situation: situation::Selector::default(),
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
            ejection_warning: false,
            engine_gain: 0.,
            burner_gain: 0.,
            voices: Vec::new(),
            ui_voices: Vec::new(),
            radio: VecDeque::new(),
            music_on: false,
            effects_on: true,
        }
    }

    #[test]
    fn stall_cues_and_pause_mute_use_a_dedicated_loop() {
        use tore_formats::flight_model::departure::DepartureMode::*;
        assert_eq!(stall_cue(Some(Warning)), Some("&STALLWR.5K"));
        assert_eq!(stall_cue(Some(ExtendedWarning)), Some("&STALLWR.5K"));
        assert_eq!(stall_cue(Some(Stalled)), Some("&STALL.5K"));
        assert_eq!(stall_cue(Some(Spinning)), Some("&STALL.5K"));
        assert_eq!(stall_cue(Some(Normal)), None);
        let mut m = test_mixer();
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
        // Recorded seeker PCM replaces the oscillator, loops and freezes on pause.
        m.seeker.target = 0.5;
        m.seeker_voice = Some(Voice {
            clip: Arc::new(Clip {
                samples: vec![192; 4],
                rate: 4.,
            }),
            position: 0.,
        });
        for _ in 0..12 {
            assert!((m.sample(4.) - 0.25).abs() < 1e-6);
        }
        m.flight_paused = true;
        let position = m.seeker_voice.as_ref().unwrap().position;
        assert_eq!(m.sample(4.), 0.);
        assert_eq!(m.seeker_voice.as_ref().unwrap().position, position);
        m.flight_paused = false;
        m.effects_on = false;
        assert_eq!(m.sample(4.), 0.);
        m.effects_on = true;
        m.seeker.target = 0.;
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
                AircraftId::F22 | AircraftId::F22n | AircraftId::Faxx => "F-22",
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
    fn ordnance_cues_use_imported_samples_and_fuel_does_not_stack() {
        assert_eq!(cue(Action::OrdnanceWeapon), Some("&ARMWPN.5K"));
        assert_eq!(cue(Action::OrdnanceAmmunition), Some("&ARMBLLT.5K"));
        assert_eq!(cue(Action::OrdnanceFuel), Some("&ARMDRIP.11K"));
        let clip = Arc::new(Clip {
            samples: vec![160; 4],
            rate: 5512.,
        });
        let mut mixer = test_mixer();
        mixer.play_ui(&clip, true);
        mixer.ui_voices[0].position = 2.;
        mixer.play_ui(&clip, true);
        assert_eq!(mixer.ui_voices.len(), 1);
        assert_eq!(mixer.ui_voices[0].position, 2.);
        mixer.ui_voices[0].position = 4.;
        mixer.play_ui(&clip, true);
        assert_eq!(mixer.ui_voices.len(), 2);
        mixer.effects_on = false;
        mixer.ui_voices.clear();
        mixer.play_ui(&clip, false);
        assert!(mixer.ui_voices.is_empty());
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
    #[test]
    fn speech_skips_missing_recordings_and_drops_whole_lines_when_full() {
        let mut m = test_mixer();
        let clips = BTreeMap::from([(
            "^CLOCK02.5K".into(),
            Arc::new(Clip {
                samples: vec![192; 2],
                rate: 4.,
            }),
        )]);
        let line = |n: usize| vec!["^CLOCK02".to_string(); n];
        m.enqueue_speech(&clips, &["^YOUR".into(), "^CLOCK02".into()]);
        assert_eq!(m.radio.len(), 1, "text-only stems play nothing");
        m.enqueue_speech(&clips, &line(SPEECH_QUEUE));
        assert_eq!(
            m.radio.len(),
            1,
            "a line that does not fit is not truncated"
        );
        m.enqueue_speech(&clips, &line(SPEECH_QUEUE - 1));
        assert_eq!(m.radio.len(), SPEECH_QUEUE);
        m.radio.clear();
        m.flight_paused = true;
        m.enqueue_speech(&clips, &line(1));
        assert!(
            m.radio.is_empty(),
            "pause drops new speech like other radio"
        );
    }
    #[test]
    fn radio_is_serial_pauses_and_interrupts_without_effects_overlap() {
        let mut m = test_mixer();
        m.stall = None;
        let clips = BTreeMap::from([
            (
                "^FIRST.5K".into(),
                Arc::new(Clip {
                    samples: vec![192; 2],
                    rate: 4.,
                }),
            ),
            (
                "^SECOND.5K".into(),
                Arc::new(Clip {
                    samples: vec![64; 2],
                    rate: 4.,
                }),
            ),
        ]);
        let phrases = BTreeMap::from([
            ("^FIRST".into(), "First".into()),
            ("^SECOND".into(), "Second".into()),
            ("^MISSING".into(), "Missing".into()),
        ]);
        m.enqueue_radio(
            &clips,
            &phrases,
            &["^FIRST", "^MISSING", "^SECOND"],
            RadioSource::Wing,
            true,
        );
        assert_eq!(m.radio.len(), 2);
        assert!((m.sample(4.) - 0.2).abs() < 1e-6);
        m.flight_paused = true;
        assert_eq!(m.sample(4.), 0.);
        assert_eq!(m.radio.front().unwrap().voice.position, 1.);
        m.flight_paused = false;
        assert!((m.sample(4.) - 0.2).abs() < 1e-6);
        assert!((m.sample(4.) + 0.2).abs() < 1e-6);
        m.enqueue_radio(&clips, &phrases, &["^FIRST"], RadioSource::Wing, true);
        assert_eq!(m.radio.len(), 1);
        assert!((m.sample(4.) - 0.2).abs() < 1e-6);
        m.enqueue_radio(
            &clips,
            &BTreeMap::new(),
            &["^FIRST"],
            RadioSource::Wing,
            true,
        );
        assert!(m.radio.is_empty());
        m.enqueue_radio(&clips, &phrases, &["^FIRST"; 20], RadioSource::Wing, true);
        assert_eq!(m.radio.len(), 16);
        m.effects_on = false;
        m.enqueue_radio(&clips, &phrases, &["^SECOND"], RadioSource::Wing, true);
        assert!(m.radio.is_empty());
    }

    #[test]
    fn airport_radio_replaces_only_airport_speech() {
        let mut m = test_mixer();
        m.stall = None;
        let clip = Arc::new(Clip {
            samples: vec![192; 2],
            rate: 4.,
        });
        let clips = BTreeMap::from([
            ("^WING.5K".into(), clip.clone()),
            ("^TOWER.5K".into(), clip),
        ]);
        let phrases = BTreeMap::from([
            ("^WING".into(), "Wing".into()),
            ("^TOWER".into(), "Tower".into()),
        ]);
        m.enqueue_radio(&clips, &phrases, &["^WING"], RadioSource::Wing, false);
        m.enqueue_radio(&clips, &phrases, &["^TOWER"], RadioSource::Airport, true);
        m.enqueue_radio(&clips, &phrases, &["^TOWER"], RadioSource::Airport, true);
        assert_eq!(m.radio.len(), 2);
        assert_eq!(m.radio.front().unwrap().source, RadioSource::Wing);
        m.cancel_radio(RadioSource::Airport);
        assert_eq!(m.radio.len(), 1);
        assert_eq!(m.radio.front().unwrap().source, RadioSource::Wing);
    }

    #[test]
    fn airport_playback_is_serial_and_cancellation_keeps_wing_progress() {
        let mut m = test_mixer();
        m.stall = None;
        let clips = BTreeMap::from([
            (
                "^WING.5K".into(),
                Arc::new(Clip {
                    samples: vec![192; 2],
                    rate: 4.,
                }),
            ),
            (
                "^TOWER.5K".into(),
                Arc::new(Clip {
                    samples: vec![64; 2],
                    rate: 4.,
                }),
            ),
        ]);
        let phrases = BTreeMap::from([
            ("^WING".into(), "Wing".into()),
            ("^TOWER".into(), "Tower".into()),
        ]);
        m.enqueue_radio(&clips, &phrases, &["^WING"], RadioSource::Wing, false);
        assert!((m.sample(4.) - 0.2).abs() < 1e-6);
        m.enqueue_radio(&clips, &phrases, &["^TOWER"], RadioSource::Airport, true);
        assert_eq!(m.radio.front().unwrap().voice.position, 1.);
        assert!((m.sample(4.) - 0.2).abs() < 1e-6);
        m.flight_paused = true;
        assert_eq!(m.sample(4.), 0.);
        assert_eq!(m.radio.front().unwrap().voice.position, 0.);
        m.flight_paused = false;
        assert!((m.sample(4.) + 0.2).abs() < 1e-6);
        m.cancel_radio(RadioSource::Airport);
        assert_eq!(m.sample(4.), 0.);
        assert!(m.radio.is_empty());
        m.effects_on = false;
        m.enqueue_radio(&clips, &phrases, &["^TOWER"], RadioSource::Airport, true);
        assert!(m.radio.is_empty());
    }
    #[test]
    fn old_cache_reports_optional_airport_audio_as_missing() {
        assert_eq!(
            missing_airport_audio(&BTreeMap::new(), &BTreeMap::new()),
            vec!["^CLRLAND", "^WELHOME"]
        );
    }
}
