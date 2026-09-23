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
    /// Latched by a fault until the situation selector reads it.
    failed: bool,
    /// Latched when the script passes a marked boundary (F9).
    boundary: bool,
    /// Set by the selector while a different score is wanted: stop at the
    /// next marked boundary rather than start the old score's next phrase.
    hold: bool,
    /// The phrase that follows a held boundary, played if the hold is released.
    held: Option<u8>,
    /// Last fault reported, so a retried missing score is printed once.
    reported: Option<(Scene, Fault)>,
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
                        log::warn!("Music {name} unavailable: {error}");
                        return None;
                    }
                };
                let mut resolved = vec![None; 249];
                for track in &score.tracks {
                    let filename = score.filename(*track);
                    resolved[*track as usize] = clips.get(&filename).cloned();
                    if resolved[*track as usize].is_none() {
                        log::warn!("Music {name}: missing {filename}; no substitute");
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
                log::warn!("Music playlist: missing {name}; no substitute");
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
            failed: false,
            boundary: false,
            hold: false,
            held: None,
            reported: None,
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
    /// Start a flight score from its beginning, even if it is the current one.
    pub fn start(&mut self, index: usize) {
        self.scene = Scene::Score(index);
        self.restart();
    }
    /// Flight silence until the selector starts a score.
    pub fn stop(&mut self) {
        self.start(0);
        self.stopped = true;
    }
    /// A fault not already reported for this scene, for a diagnostic line.
    pub fn new_fault(&mut self) -> Option<Fault> {
        let fault = self.fault.take()?;
        (self.reported != Some((self.scene, fault))).then(|| {
            self.reported = Some((self.scene, fault));
            fault
        })
    }
    /// Consume what happened since the previous call, for the flight selector.
    pub fn playback(&mut self) -> super::situation::Playback {
        super::situation::Playback {
            playing: !self.stopped,
            boundary: std::mem::take(&mut self.boundary),
            failed: std::mem::take(&mut self.failed),
        }
    }
    pub fn set_hold(&mut self, hold: bool) {
        self.hold = hold;
    }
    pub fn restart(&mut self) {
        self.cursor = Cursor::default();
        self.voice = None;
        self.stopped = false;
        self.fault = None;
        self.failed = false;
        self.boundary = false;
        self.hold = false;
        self.held = None;
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
                    self.fail(Fault::MissingScore);
                    return None;
                };
                let track = match self.held.take() {
                    Some(track) => Ok(Some(track)),
                    None => prepared
                        .score
                        .next(&mut self.cursor, |limit| draw(&mut self.rng, limit)),
                };
                match track {
                    Ok(Some(track)) => {
                        if std::mem::take(&mut self.cursor.host_flag) {
                            self.boundary = true;
                            if self.hold {
                                // Silence at the boundary until the selector
                                // switches or releases the hold.
                                self.held = Some(track);
                                return None;
                            }
                        }
                        prepared.clips[track as usize].clone()
                    }
                    Ok(None) => {
                        self.stopped = true;
                        return None;
                    }
                    Err(_) => {
                        self.fail(Fault::InstructionBudget);
                        return None;
                    }
                }
            }
        };
        if clip.is_none() {
            self.fail(Fault::MissingTrack);
        }
        clip
    }
    fn fail(&mut self, fault: Fault) {
        self.fault = Some(fault);
        self.failed = true;
        self.stopped = true;
    }
    pub fn next(&mut self, rate: f64) -> f32 {
        if self.stopped || (self.hold && self.held.is_some()) {
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
    fn clips() -> BTreeMap<String, Arc<Clip>> {
        let clip = |value| {
            Arc::new(Clip {
                samples: vec![value, value],
                rate: 4.,
            })
        };
        BTreeMap::from([
            ("T001.11K".into(), clip(192)),
            ("T002.11K".into(), clip(64)),
        ])
    }
    fn prepare(code: &[u8]) -> Prepared {
        let clips = clips();
        Prepared {
            score: Score::from_code(code).unwrap(),
            clips: (0..249)
                .map(|i| clips.get(&format!("T{i:03}.11K")).cloned())
                .collect(),
        }
    }
    fn prepared(code: &[u8]) -> Music {
        let mut player = Music::new(&clips(), &BTreeMap::new(), 1);
        player.scores[0] = Some(prepare(code));
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

    fn mixer(music: Music) -> super::super::Mixer {
        super::super::Mixer {
            spatial: Default::default(),
            seeker: Default::default(),
            seeker_voice: None,
            seeker_cue: None,
            seeker_volume: 0.15,
            music,
            situation: Default::default(),
            engine: None,
            engine_aircraft: None,
            burner: None,
            stall: None,
            stall_cue: None,
            flight_on: true,
            flight_paused: false,
            ejection_warning: false,
            engine_gain: 0.,
            burner_gain: 0.,
            voices: Vec::new(),
            ui_voices: Vec::new(),
            radio: std::collections::VecDeque::new(),
            music_on: true,
            effects_on: false,
        }
    }
    #[test]
    fn mixer_pause_mute_and_effects_are_independent() {
        let mut mixer = mixer(prepared(&[255, b'T', 0, 1, 2, 252]));
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

    #[test]
    fn boundary_latches_and_a_hold_waits_there_without_the_next_phrase() {
        let mut p = prepared(&[255, b'T', 0, 1, 249, 2, 252]);
        assert_eq!((p.next(4.), p.next(4.)), (0.5, 0.5));
        assert!(!p.playback().boundary);
        p.set_hold(true);
        for _ in 0..4 {
            assert_eq!(p.next(4.), 0., "silent at the marked boundary");
        }
        let playback = p.playback();
        assert!(playback.boundary && playback.playing && !playback.failed);
        assert!(!p.playback().boundary, "consumed once");
        p.set_hold(false);
        assert_eq!((p.next(4.), p.next(4.)), (-0.5, -0.5));
        assert_eq!(p.next(4.), 0.);
        assert_eq!(p.playback(), super::super::situation::Playback::default());
        // Without a hold the boundary is only reported.
        let mut p = prepared(&[255, b'T', 0, 1, 249, 2, 252]);
        let samples: Vec<_> = (0..4).map(|_| p.next(4.)).collect();
        assert_eq!(samples, [0.5, 0.5, -0.5, -0.5]);
        assert!(p.playback().boundary);
    }

    #[test]
    fn faults_latch_once_and_are_reported_once_per_scene() {
        let mut p = prepared(&[255, b'T', 0, 3, 252]);
        assert_eq!(p.next(4.), 0.);
        let playback = p.playback();
        assert!(playback.failed && !playback.playing);
        assert!(!p.playback().failed);
        assert_eq!(p.new_fault(), Some(Fault::MissingTrack));
        p.start(0);
        p.next(4.);
        assert_eq!(p.new_fault(), None, "a retry does not print again");
        p.stop();
        assert!(!p.playback().playing);
    }

    #[test]
    fn mixer_situation_cuts_up_and_waits_down_for_a_boundary() {
        use super::super::situation::Inputs;
        // NORMAL loops phrase 1 and AIR loops phrase 2, each with a marked
        // boundary after every phrase.
        let mut music = prepared(&[255, b'T', 0, 1, 249, 254, 3, 0, 0, 0]);
        music.scores[1] = Some(prepare(&[255, b'T', 0, 2, 249, 254, 3, 0, 0, 0]));
        let mut m = mixer(music);
        m.music_on = true;
        m.music.stop();
        let calm = Inputs::default();
        let fight = Inputs {
            air_target: true,
            ..calm
        };
        m.situation(&calm, 0.);
        assert_eq!(m.sample(4.), 0.08, "NORMAL");
        m.situation(&fight, 1.);
        assert_eq!(m.sample(4.), -0.08, "AIR cuts in at once");
        m.situation(&calm, 2.);
        assert_eq!(m.sample(4.), -0.08, "the AIR phrase plays out");
        assert_eq!(m.sample(4.), 0., "held at the boundary");
        m.situation(&calm, 2.01);
        assert_eq!(m.sample(4.), 0.08, "NORMAL after the boundary");
        // Music off: nothing is chosen, and turning it on starts afresh.
        m.music_on = false;
        m.situation(&fight, 3.1);
        m.music_on = true;
        m.situation(&calm, 3.2);
        assert_eq!(m.sample(4.), 0.08);
    }
}
