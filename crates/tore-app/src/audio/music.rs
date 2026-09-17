//! PCM phrase player. No device, renderer, file access or synthesis in scheduling.
use super::{Clip, Voice};
use std::{collections::BTreeMap, sync::Arc};
use tore_formats::music::{self, Cursor, Score};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Main,
    Brief,
    Score(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    MissingTrack,
    MissingScore,
    InstructionBudget,
}
struct Prepared {
    score: Score,
    clips: Vec<Option<Arc<Clip>>>,
}
pub struct Music {
    main: Vec<Option<Arc<Clip>>>,
    brief: Vec<Option<Arc<Clip>>>,
    scores: Vec<Option<Prepared>>,
    scene: Scene,
    cursor: Cursor,
    playlist_index: usize,
    rng: u32,
    voice: Option<Voice>,
    stopped: bool,
    pub fault: Option<Fault>,
}
fn draw(rng: &mut u32, limit: u32) -> u32 {
    // Authored audio-only RNG, independent of authoritative flight state.
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    *rng % limit
}
impl Music {
    pub(super) fn new(
        clips: &BTreeMap<String, Arc<Clip>>,
        scripts: &BTreeMap<String, Vec<u8>>,
        seed: u32,
    ) -> Self {
        let playlist = |names: &[&str]| names.iter().map(|n| clips.get(*n).cloned()).collect();
        let scores = music::SCORES
            .iter()
            .map(|name| {
                let bytes = scripts.get(*name)?;
                let score = match Score::parse(bytes) {
                    Ok(score) => score,
                    Err(error) => {
                        eprintln!("Music {name} unavailable: {error}");
                        return None;
                    }
                };
                let mut resolved = vec![None; 249];
                for track in &score.tracks {
                    let filename = score.filename(*track);
                    resolved[*track as usize] = clips.get(&filename).cloned();
                    if resolved[*track as usize].is_none() {
                        eprintln!("Music {name}: missing {filename}; no substitute");
                    }
                }
                Some(Prepared {
                    score,
                    clips: resolved,
                })
            })
            .collect();
        for name in music::MAIN.iter().chain(music::BRIEF) {
            if !clips.contains_key(*name) {
                eprintln!("Music playlist: missing {name}; no substitute");
            }
        }
        let mut result = Self {
            main: playlist(music::MAIN),
            brief: playlist(music::BRIEF),
            scores,
            scene: Scene::Main,
            cursor: Cursor::default(),
            playlist_index: 0,
            rng: seed.max(1),
            voice: None,
            stopped: false,
            fault: None,
        };
        result.restart();
        result
    }
    pub fn scene(&mut self, scene: Scene) {
        if self.scene != scene {
            self.scene = scene;
            self.restart();
        }
    }
    pub fn restart(&mut self) {
        self.cursor = Cursor::default();
        self.voice = None;
        self.stopped = false;
        self.fault = None;
        self.playlist_index = match self.scene {
            Scene::Main => draw(&mut self.rng, self.main.len() as u32) as usize,
            Scene::Brief => draw(&mut self.rng, self.brief.len() as u32) as usize,
            Scene::Score(_) => 0,
        };
    }
    fn phrase(&mut self) -> Option<Arc<Clip>> {
        let clip = match self.scene {
            Scene::Main | Scene::Brief => {
                let playlist = if self.scene == Scene::Main {
                    &self.main
                } else {
                    &self.brief
                };
                let clip = playlist[self.playlist_index].clone();
                self.playlist_index = (self.playlist_index + 1) % playlist.len();
                clip
            }
            Scene::Score(index) => {
                let Some(Some(prepared)) = self.scores.get(index) else {
                    self.fault = Some(Fault::MissingScore);
                    self.stopped = true;
                    return None;
                };
                match prepared
                    .score
                    .next(&mut self.cursor, |limit| draw(&mut self.rng, limit))
                {
                    Ok(Some(track)) => prepared.clips[track as usize].clone(),
                    Ok(None) => {
                        self.stopped = true;
                        return None;
                    }
                    Err(_) => {
                        self.fault = Some(Fault::InstructionBudget);
                        self.stopped = true;
                        return None;
                    }
                }
            }
        };
        if clip.is_none() {
            self.fault = Some(Fault::MissingTrack);
            self.stopped = true;
        }
        clip
    }
    pub fn next(&mut self, rate: f64) -> f32 {
        if self.stopped {
            return 0.;
        }
        if self.voice.as_ref().is_none_or(|v| v.finished()) {
            let overshoot = self.voice.as_ref().map_or(0., |v| {
                (v.position - v.clip.samples.len() as f64) / v.clip.rate
            });
            let Some(clip) = self.phrase() else {
                return 0.;
            };
            self.voice = Some(Voice {
                position: overshoot * clip.rate,
                clip,
            });
        }
        self.voice.as_mut().unwrap().next(rate, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prepared(code: &[u8]) -> Music {
        let mut clips = BTreeMap::new();
        clips.insert(
            "T001.11K".into(),
            Arc::new(Clip {
                samples: vec![192, 192],
                rate: 4.,
            }),
        );
        clips.insert(
            "T002.11K".into(),
            Arc::new(Clip {
                samples: vec![64, 64],
                rate: 4.,
            }),
        );
        let mut player = Music::new(&clips, &BTreeMap::new(), 1);
        player.scores[0] = Some(Prepared {
            score: Score::from_code(code).unwrap(),
            clips: (0..249)
                .map(|i| clips.get(&format!("T{i:03}.11K")).cloned())
                .collect(),
        });
        player.scene(Scene::Score(0));
        player
    }
    #[test]
    fn successive_phrases_stop_and_restart_without_repeating_first() {
        let mut p = prepared(&[255, b'T', 0, 1, 2, 252]);
        assert_eq!(
            (0..6).map(|_| p.next(4.)).collect::<Vec<_>>(),
            vec![0.5, 0.5, -0.5, -0.5, 0., 0.]
        );
        assert_eq!(p.fault, None);
        p.restart();
        assert_eq!(p.next(4.), 0.5);
    }
    #[test]
    fn missing_phrase_and_busy_loop_stop_explicitly() {
        let mut p = prepared(&[255, b'T', 0, 3, 252]);
        assert_eq!(p.next(4.), 0.);
        assert_eq!(p.fault, Some(Fault::MissingTrack));
        let mut p = prepared(&[255, b'T', 0, 254, 0, 0, 0, 0]);
        assert_eq!(p.next(4.), 0.);
        assert_eq!(p.fault, Some(Fault::InstructionBudget));
    }

    #[test]
    fn mixer_pause_mute_and_effects_are_independent() {
        let mut mixer = super::super::Mixer {
            seeker: Default::default(),
            seeker_voice: None,
            seeker_cue: None,
            seeker_volume: 0.15,
            music: prepared(&[255, b'T', 0, 1, 2, 252]),
            engine: None,
            engine_aircraft: None,
            burner: None,
            stall: None,
            stall_cue: None,
            flight_on: true,
            flight_paused: false,
            engine_gain: 0.,
            burner_gain: 0.,
            voices: Vec::new(),
            ui_voices: Vec::new(),
            music_on: true,
            effects_on: false,
        };
        assert_eq!(mixer.sample(4.), 0.08);
        mixer.flight_paused = true;
        for _ in 0..20 {
            assert_eq!(mixer.sample(4.), 0.);
        }
        mixer.flight_paused = false;
        mixer.music_on = false;
        for _ in 0..20 {
            assert_eq!(mixer.sample(4.), 0.);
        }
        mixer.music_on = true;
        assert_eq!(mixer.sample(4.), 0.08);
        assert_eq!(mixer.sample(4.), -0.08);
        mixer.music.scene(Scene::Main);
        // Missing menu media is explicit, not a fallback to a flight phrase.
        assert_eq!(mixer.sample(4.), 0.);
        assert_eq!(mixer.music.fault, Some(Fault::MissingTrack));
        mixer.music.scene(Scene::Score(0));
        assert_eq!(mixer.sample(4.), 0.08);
        mixer.flight_paused = true;
        mixer.effects_on = true;
        let clip = Arc::new(Clip {
            samples: vec![192, 192],
            rate: 4.,
        });
        mixer.voices.push(Voice {
            clip: clip.clone(),
            position: 0.,
        });
        mixer.ui_voices.push(Voice { clip, position: 0. });
        assert_eq!(mixer.sample(4.), 0.2);
        assert_eq!(mixer.voices[0].position, 0.);
        assert_eq!(mixer.ui_voices[0].position, 1.);
    }
}
