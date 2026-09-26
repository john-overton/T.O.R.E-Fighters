//! Replay sound: the voices, tones and effects of a recording, played back
//! at normal speed through the same plain-data audio calls live flight
//! makes. Opinionated addition requested by John on 2026-09-26; how it is
//! scheduled is an agent design (2026-09-26). See docs/REPLAYS.md#sound.
//!
//! The viewer calls [`ReplaySound::frame`] once per drawn frame with the
//! stretch of recording the frame played. At exactly 1x forwards every
//! recorded cue in that stretch plays once, in order. A paused clock
//! freezes what is playing and playing on resumes it. Any other speed,
//! reverse, dragging the timeline, or a jump of the playhead between frames
//! cancels queued speech and silences tones and loops. The steady sounds
//! (the seeker tone, the stall warning and the watched aircraft's engine)
//! are read from the recording at the playhead, so they come back right
//! after a jump. Scheduling is plain data ([`Cue`]), tested without a sound
//! device and written to the session log with `TORE_REPLAY_SOUND_LOG=1`.
use crate::audio::{self, Audio, EngineLoops, EngineSounds};
use crate::replay::clock::{self, Clock, Direction};
use crate::replay::convert;
use crate::replay::playback::Playback;
use crate::terrain::Camera;
use std::sync::Arc;
use tore_formats::aircraft::{Aircraft, AircraftId};
use tore_formats::flight_model::departure::DepartureMode;
use tore_replay::{Event, Recording, TimedEvent, control, vocab};
use tore_sim::acoustics::{Emission, Kind, Listener, Source};
use tore_sim::attitude::Basis;
use tore_sim::combat::live::SeekerTone;
use tore_sim::combat::missiles::seeker::{Seeker, Status};

/// The most ticks one frame plays: a second of recording. The viewer
/// advances at most a quarter of a second a frame, so this only guards
/// against a runaway caller.
const MAX_TICKS: u64 = 120;
/// The recording keeps which seeker tone played, not how strong it was, so
/// the replay sounds it at the live strength for this seeker quality and
/// estimated hit chance (fitted, agent decision 2026-09-26).
const TONE_QUALITY: f64 = 0.5;
const TONE_PERCENT: u8 = 50;
/// The trigger the recorder gives a tower reply to the player's request.
const PLAYER_REQUEST: &str = "player request";
/// The route the recorder gives a recording played straight into the
/// cockpit.
const DIRECT: &str = "direct";

/// What one viewer frame played.
pub struct Moment<'a> {
    /// The playhead before this frame advanced it; the clock holds where it
    /// is now, the direction, the speed and whether it is paused.
    pub from: f64,
    pub clock: &'a Clock,
    /// The timeline is being dragged.
    pub scrubbing: bool,
    /// The camera the frame is drawn from: the listener.
    pub camera: &'a Camera,
    /// The flight view, or `None` for the drone.
    pub view: Option<u8>,
    /// The aircraft the camera follows, whose engine is heard.
    pub selected: u32,
}

/// One call to the sound device, as plain data.
#[derive(Clone, Debug)]
pub enum Cue {
    /// Cancel queued speech and silence tones, loops and effects:
    /// [`Audio::restart_flight`].
    Reset,
    /// Freeze or resume everything playing: [`Audio::pause_flight`].
    Pause(bool),
    /// A radio or crew line: [`Audio::speech`].
    Speech(Vec<String>),
    /// A tower line: [`Audio::airport_speech`].
    Tower(Vec<String>),
    /// The tower's reply to the player's request, which replaces queued
    /// tower speech: [`Audio::airport_radio`], or
    /// [`Audio::cancel_airport_radio`] for a reply without a recording.
    TowerReply(Option<String>),
    /// The player's aircraft was lost, which stops queued tower speech as
    /// in flight: [`Audio::cancel_airport_radio`].
    TowerCancelled,
    /// A recording played straight into the cockpit, such as the player's
    /// death scream: [`Audio::direct_voice`].
    Direct(String),
    /// The player's wing order voice, which cuts off queued wing speech:
    /// [`Audio::radio`].
    Order(Vec<String>),
    /// One of the player's ejection cues: [`Audio::ejection_cue`].
    Ejection(String),
    /// A friendly wingman ejected: [`Audio::wingman_ejected`].
    WingmanEjected,
    /// A gear, flap, hook or brake sound, or the watched engine starting
    /// or stopping: [`Audio::effect`].
    Effect(String),
    /// The seeker tone changed: [`Audio::seeker`].
    Seeker(Option<SeekerTone>),
    /// One simulation tick of traveling sound: [`Audio::spatial_tick`].
    Tick(Box<SpatialTick>),
    /// The steady loops, every frame: [`Audio::replay_loops`].
    Loops {
        engine: Option<EngineLoops>,
        stall: Option<&'static str>,
    },
}

/// One tick of traveling sound, heard from the camera.
#[derive(Clone, Debug)]
pub struct SpatialTick {
    pub listener: Listener,
    /// Aircraft and missiles that can pass the camera.
    pub sources: Vec<Source>,
    /// Impacts and explosions released this tick.
    pub emissions: Vec<Emission>,
    /// The player's weapon release sounds this tick.
    pub releases: Vec<String>,
    /// Where the player's aircraft is, where its releases sound from.
    pub player: [f64; 3],
}

impl Cue {
    /// Hands the cue to the sound device.
    pub fn play(&self, audio: &Audio) {
        match self {
            Self::Reset => audio.restart_flight(),
            Self::Pause(paused) => audio.pause_flight(*paused),
            Self::Speech(stems) => audio.speech(stems),
            Self::Tower(stems) => audio.airport_speech(stems),
            Self::TowerReply(Some(stem)) => audio.airport_radio(&[stem.as_str()]),
            Self::TowerReply(None) | Self::TowerCancelled => audio.cancel_airport_radio(),
            Self::Direct(stem) => audio.direct_voice(stem),
            Self::Order(stems) => {
                audio.radio(&stems.iter().map(String::as_str).collect::<Vec<_>>(), true);
            }
            Self::Ejection(sound) => audio.ejection_cue(sound),
            Self::WingmanEjected => audio.wingman_ejected(),
            Self::Effect(sound) => audio.effect(sound),
            Self::Seeker(tone) => audio.seeker(*tone),
            Self::Tick(tick) => audio.spatial_tick(
                tick.listener,
                &tick.sources,
                &tick.emissions,
                &tick.releases.iter().map(String::as_str).collect::<Vec<_>>(),
                tick.player,
            ),
            Self::Loops { engine, stall } => audio.replay_loops(engine.as_ref(), *stall),
        }
    }

    /// One line for the session log and the tests.
    pub fn describe(&self) -> String {
        match self {
            Self::Reset => "reset: speech cancelled, tones and loops off".into(),
            Self::Pause(true) => "paused".into(),
            Self::Pause(false) => "resumed".into(),
            Self::Speech(stems) => format!("radio {}", stems.join(" ")),
            Self::Tower(stems) => format!("tower {}", stems.join(" ")),
            Self::TowerReply(Some(stem)) => format!("tower reply {stem}"),
            Self::TowerReply(None) => "tower reply without a recording".into(),
            Self::TowerCancelled => "player lost: tower speech cancelled".into(),
            Self::Direct(stem) => format!("direct voice {stem}"),
            Self::Order(stems) => format!("order voice {}", stems.join(" ")).trim().into(),
            Self::Ejection(sound) => format!("ejection {sound}"),
            Self::WingmanEjected => "wingman ejected".into(),
            Self::Effect(sound) => format!("effect {sound}"),
            Self::Seeker(None) => "seeker tone off".into(),
            Self::Seeker(Some(tone)) => format!(
                "seeker tone {}{}{} at {:.3}",
                if tone.radar { "radar" } else { "infrared" },
                if tone.ground { " surface" } else { "" },
                if tone.locked { " lock" } else { " search" },
                tone.strength
            ),
            Self::Tick(tick) => {
                let effects: Vec<String> = tick
                    .emissions
                    .iter()
                    .map(|e| format!("{:?}", e.kind).to_lowercase())
                    .collect();
                format!(
                    "traveling sound: effects [{}], releases [{}], {} sources",
                    effects.join(" "),
                    tick.releases.join(" "),
                    tick.sources.len()
                )
            }
            Self::Loops { engine, stall } => {
                let engine = engine.as_ref().map_or("no engine".to_owned(), |e| {
                    format!(
                        "{} engine {}{}",
                        e.aircraft.label(),
                        if e.running { "running" } else { "stopped" },
                        if e.running && e.afterburner {
                            ", afterburner"
                        } else {
                            ""
                        }
                    )
                });
                format!("loops: {engine}; stall {}", stall.unwrap_or("off"))
            }
        }
    }
}

/// Silences everything a replay started: queued speech, tones, loops and
/// effects. For entering and leaving the viewer.
pub fn stop(audio: &Audio) {
    audio.restart_flight();
    audio.flight(None);
}

/// The listener for a frame drawn from `camera`: its position and right
/// hand, built as live flight builds it from the main view, and outside
/// unless it sits in the player's cockpit. `cut` changes whenever the
/// camera cuts, so a cut is never heard as something flying past.
pub fn listener(camera: &Camera, cut: u8) -> Listener {
    let basis = Basis::new(
        f64::from(camera.yaw),
        f64::from(camera.pitch),
        -f64::from(camera.roll),
    );
    Listener {
        position: camera.position.map(f64::from),
        right: basis.right,
        view: cut,
        external: camera.hidden_target != Some(0),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Nothing plays: at first and after a reset.
    Silent,
    /// Playing at 1x forwards.
    Playing,
    /// Frozen by a paused clock, ready to carry on.
    Paused,
}

/// The replay viewer's sound. See the module documentation.
pub struct ReplaySound {
    recording: Arc<Recording>,
    /// A reader of its own, so sound never disturbs the picture's decoded
    /// chunks.
    playback: Playback,
    /// Engine recordings of each loaded aircraft type.
    engines: Vec<(AircraftId, EngineSounds)>,
    /// The seeker tone and the stall warning from each recorded change on.
    tones: Vec<(u64, Option<SeekerTone>)>,
    stalls: Vec<(u64, Option<&'static str>)>,
    state: State,
    /// Where the playhead stood after the last frame.
    last: Option<f64>,
    /// The seeker tone last sent, `None` until one is.
    tone: Option<Option<SeekerTone>>,
    /// The watched aircraft and whether its engine ran, last frame.
    engine: Option<(u32, bool)>,
    /// The last frame's view and aircraft, and a count of camera cuts.
    camera: Option<(Option<u8>, u32)>,
    cuts: u8,
    /// The player's aircraft is lost.
    player_down: bool,
    /// `TORE_REPLAY_SOUND_LOG=1`: every cue goes to the session log.
    log: bool,
    /// The loops last logged, so the log shows only their changes.
    logged: Option<String>,
}

impl ReplaySound {
    /// Sound for `recording`, with the engine recordings of every aircraft
    /// type the viewer loaded.
    pub fn new<'a>(
        recording: Arc<Recording>,
        profiles: impl IntoIterator<Item = &'a Aircraft>,
    ) -> Self {
        let engines = profiles
            .into_iter()
            .map(|profile| (profile.id, EngineSounds::of(profile)))
            .collect();
        Self::with_engines(recording, engines)
    }

    fn with_engines(recording: Arc<Recording>, engines: Vec<(AircraftId, EngineSounds)>) -> Self {
        let (tones, stalls) = timelines(recording.events());
        Self {
            playback: Playback::new(Arc::clone(&recording)),
            recording,
            engines,
            tones,
            stalls,
            state: State::Silent,
            last: None,
            tone: None,
            engine: None,
            camera: None,
            cuts: 0,
            player_down: false,
            log: std::env::var("TORE_REPLAY_SOUND_LOG").is_ok_and(|v| v == "1"),
            logged: None,
        }
    }

    /// Schedules one frame's sound and plays it on `audio`, if there is a
    /// sound device.
    pub fn frame(&mut self, audio: Option<&Audio>, moment: &Moment<'_>) {
        let cues = self.schedule(moment);
        if self.log {
            self.write_log(&cues);
        }
        if let Some(audio) = audio {
            for (_, cue) in &cues {
                cue.play(audio);
            }
        }
    }

    /// What the sound device should do for one frame, in order, each with
    /// the tick it belongs to.
    pub fn schedule(&mut self, moment: &Moment<'_>) -> Vec<(u64, Cue)> {
        let mut cues = Vec::new();
        let clock = moment.clock;
        let (from, to) = (moment.from, clock.position());
        let here = clock.tick();
        // Anything but this frame's advance moved the playhead: a jump, a
        // step, a marker or a click on the timeline.
        if self.last.is_some_and(|last| last != from) {
            self.reset(here, &mut cues);
        }
        self.last = Some(to);
        let camera = (moment.view, moment.selected);
        if self.camera.is_some_and(|last| last != camera) {
            self.cuts = self.cuts.wrapping_add(1);
        }
        self.camera = Some(camera);
        let normal =
            clock.direction() == Direction::Forward && clock.speed() == 1. && !moment.scrubbing;
        if normal && (to > from || !clock.paused()) {
            // A fresh start hears the tick under a whole playhead; carrying
            // on starts after the ticks already heard.
            let first = match self.state {
                State::Silent => from.ceil(),
                State::Paused => {
                    cues.push((here, Cue::Pause(false)));
                    from.floor() + 1.
                }
                State::Playing => from.floor() + 1.,
            };
            self.state = State::Playing;
            let listener = listener(moment.camera, self.cuts);
            self.play(first, to.floor(), listener, moment.selected, &mut cues);
            // Playback stops by itself at the end.
            if clock.paused() {
                cues.push((here, Cue::Pause(true)));
                self.state = State::Paused;
            }
        } else if clock.paused() && to == from && !moment.scrubbing {
            if self.state == State::Playing {
                cues.push((here, Cue::Pause(true)));
                self.state = State::Paused;
            }
        } else {
            self.reset(here, &mut cues);
        }
        cues
    }

    fn reset(&mut self, tick: u64, cues: &mut Vec<(u64, Cue)>) {
        if self.state != State::Silent {
            cues.push((tick, Cue::Reset));
        }
        self.state = State::Silent;
        self.tone = None;
        self.engine = None;
        self.player_down = false;
    }

    /// Plays the recorded ticks `first` to `last`, then sets the steady
    /// sounds for `last`.
    fn play(
        &mut self,
        first: f64,
        last: f64,
        listener: Listener,
        selected: u32,
        cues: &mut Vec<(u64, Cue)>,
    ) {
        let recording = Arc::clone(&self.recording);
        let (Some(start), Some(end)) = (recording.first_tick(), recording.last_tick()) else {
            return;
        };
        let last = (last.max(0.) as u64).clamp(start, end);
        let first = (first.max(0.) as u64)
            .max(start)
            .max(last.saturating_sub(MAX_TICKS - 1));
        if first <= last {
            let events = recording.events_between(first, last);
            let mut at = 0;
            for tick in first..=last {
                let begin = at;
                while events.get(at).is_some_and(|e| e.tick == tick) {
                    at += 1;
                }
                self.tick(tick, &events[begin..at], listener, cues);
            }
        }
        self.steady(last, selected, cues);
    }

    /// One recorded tick: its one-shot cues in recorded order, then its
    /// traveling sound.
    fn tick(
        &mut self,
        tick: u64,
        events: &[TimedEvent],
        listener: Listener,
        cues: &mut Vec<(u64, Cue)>,
    ) {
        // Inside a gap the last frame before it holds.
        let frame = self.playback.frame(tick);
        let frame = frame.as_ref().map(|(frames, at)| &frames[*at]);
        let down = frame
            .and_then(|frame| frame.aircraft.iter().find(|a| a.id == 0))
            .is_some_and(|p| p.flags.crashed || p.flags.ejected || !p.flags.alive);
        if down && !self.player_down {
            cues.push((tick, Cue::TowerCancelled));
        }
        self.player_down = down;
        let mut emissions = Vec::new();
        let mut releases = Vec::new();
        for TimedEvent { event, .. } in events {
            if let Some(cue) = cue(event) {
                cues.push((tick, cue));
            }
            match event.kind.as_str() {
                vocab::kind::AUDIO_EFFECT => emissions.extend(emission(event)),
                vocab::kind::AUDIO_RELEASE if event.subject == Some(0) => {
                    releases.extend(event.string(vocab::field::SOUND).map(str::to_owned));
                }
                _ => {}
            }
        }
        let Some(frame) = frame else {
            return;
        };
        let snapshot = convert::snapshot(
            frame,
            &[],
            &self.playback.presentation,
            &self.playback.identities,
        );
        cues.push((
            tick,
            Cue::Tick(Box::new(SpatialTick {
                listener,
                sources: audio::snapshot_sources(&snapshot),
                emissions,
                releases,
                player: snapshot.player.position,
            })),
        ));
    }

    /// The steady sounds at `tick`: the seeker tone when it changed, then
    /// the loops, and the watched engine's start or stop sound when it
    /// lit or stopped while watched.
    fn steady(&mut self, tick: u64, selected: u32, cues: &mut Vec<(u64, Cue)>) {
        let tone = state_at(&self.tones, tick).flatten();
        if self.tone != Some(tone) {
            cues.push((tick, Cue::Seeker(tone)));
            self.tone = Some(tone);
        }
        let stall = state_at(&self.stalls, tick).flatten();
        let watched = self.playback.aircraft(tick, selected);
        let mut engine = None;
        if let Some(state) = &watched
            && let Some(&aircraft) = self.playback.identities.aircraft.get(&state.id)
            && let Some((_, sounds)) = self.engines.iter().find(|(id, _)| *id == aircraft)
        {
            let flags = state.flags;
            let aboard = !flags.ejected && !flags.crashed;
            if aboard && self.engine == Some((state.id, !flags.engine_on)) {
                let sound = if flags.engine_on {
                    &sounds.start
                } else {
                    &sounds.stop
                };
                if let Some(sound) = sound {
                    cues.push((tick, Cue::Effect(sound.clone())));
                }
            }
            engine = Some(EngineLoops {
                aircraft,
                sounds: sounds.clone(),
                running: flags.engine_on && aboard,
                throttle: state.controls[control::THROTTLE].clamp(0., 1.),
                afterburner: flags.afterburner,
            });
        }
        self.engine = watched.map(|state| (state.id, state.flags.engine_on));
        cues.push((tick, Cue::Loops { engine, stall }));
    }

    /// The session log: every cue with its tick, leaving out quiet ticks of
    /// traveling sound and loops that did not change.
    fn write_log(&mut self, cues: &[(u64, Cue)]) {
        for (tick, cue) in cues {
            let text = cue.describe();
            match cue {
                Cue::Tick(t) if t.emissions.is_empty() && t.releases.is_empty() => continue,
                Cue::Loops { .. } if self.logged.as_ref() == Some(&text) => continue,
                Cue::Loops { .. } => self.logged = Some(text.clone()),
                _ => {}
            }
            log::info!(
                "Replay sound: tick {tick} ({}): {text}",
                clock::timestamp(*tick as f64)
            );
        }
    }
}

/// The one-shot cue a recorded event plays, if any. Only lines the player
/// heard and the radio delivered speak, only the player's own orders have a
/// voice, and only the player's cockpit makes device sounds.
fn cue(event: &Event) -> Option<Cue> {
    use vocab::{field, kind};
    let stems = || -> Vec<String> {
        event
            .string(field::STEMS)
            .map(|s| s.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default()
    };
    let sound = || event.string(field::SOUND).map(str::to_owned);
    match event.kind.as_str() {
        kind::COMMS_RADIO | kind::COMMS_CREW | kind::COMMS_TOWER if !heard(event) => None,
        kind::COMMS_TOWER if event.string(field::TRIGGER) == Some(PLAYER_REQUEST) => {
            Some(Cue::TowerReply(stems().into_iter().next()))
        }
        kind::COMMS_TOWER => Some(stems()).filter(|s| !s.is_empty()).map(Cue::Tower),
        kind::COMMS_RADIO if event.string(field::ROUTE) == Some(DIRECT) => {
            stems().into_iter().next().map(Cue::Direct)
        }
        kind::COMMS_RADIO | kind::COMMS_CREW => {
            Some(stems()).filter(|s| !s.is_empty()).map(Cue::Speech)
        }
        // As in flight, even an order without a voice cuts off queued wing
        // speech; an order the wing refused says nothing.
        kind::COMMS_ORDER
            if event.subject == Some(0)
                && event.string(field::OUTCOME) != Some(vocab::outcome::REJECTED) =>
        {
            Some(Cue::Order(stems()))
        }
        kind::AUDIO_EJECTION => match event.subject {
            Some(0) => sound().map(Cue::Ejection),
            Some(_) => Some(Cue::WingmanEjected),
            None => None,
        },
        kind::AUDIO_DEVICE if event.subject == Some(0) => sound().map(Cue::Effect),
        _ => None,
    }
}

/// A line the player heard and the radio delivered. Entries for lines that
/// were queued, held back or not heard stay silent.
fn heard(event: &Event) -> bool {
    event.flag(vocab::field::HEARD) != Some(false)
        && event
            .string(vocab::field::OUTCOME)
            .is_none_or(|outcome| outcome == vocab::outcome::DELIVERED)
}

/// A recorded impact or explosion as the traveling-sound model takes it.
fn emission(event: &Event) -> Option<Emission> {
    use vocab::field;
    let name = event.string(field::SOUND)?;
    // Named as the recorder names them.
    let kind = [
        Kind::Impact,
        Kind::Explosion,
        Kind::AircraftPass,
        Kind::MissilePass,
        Kind::SonicBoom,
    ]
    .into_iter()
    .find(|kind| format!("{kind:?}").to_lowercase() == name)?;
    Some(Emission {
        kind,
        position: [
            event.num(field::X_FT)?,
            event.num(field::Y_FT)?,
            event.num(field::Z_FT)?,
        ],
        arrived: false,
        own: false,
    })
}

/// The state set by the last change at or before `tick`, if any.
fn state_at<T: Copy>(changes: &[(u64, T)], tick: u64) -> Option<T> {
    let at = changes.partition_point(|(t, _)| *t <= tick);
    at.checked_sub(1).map(|i| changes[i].1)
}

/// The player's seeker tone and stall warning over the recording: each
/// recorded change and the state from then on.
#[allow(clippy::type_complexity)]
fn timelines(
    events: &[TimedEvent],
) -> (
    Vec<(u64, Option<SeekerTone>)>,
    Vec<(u64, Option<&'static str>)>,
) {
    use vocab::{field, kind};
    let (mut tones, mut stalls) = (Vec::new(), Vec::new());
    let mut surface = false;
    for TimedEvent { tick, event } in events {
        if event.subject != Some(0) {
            continue;
        }
        let on = event.flag(field::ON) == Some(true);
        match event.kind.as_str() {
            kind::AUDIO_TONE => {
                let tone = if on {
                    seeker_tone(event.string(field::TONE).unwrap_or_default(), &mut surface)
                } else {
                    surface = false;
                    None
                };
                tones.push((*tick, tone));
            }
            kind::AUDIO_STALL_WARNING => {
                let sound = event.string(field::SOUND).and_then(stall_sound);
                stalls.push((*tick, sound.filter(|_| on)));
            }
            _ => {}
        }
    }
    (tones, stalls)
}

/// The tone a recorded tone name stands for, as the recorder names them.
/// Its strength is the live rule's at [`TONE_QUALITY`] or
/// [`TONE_PERCENT`]. An infrared lock does not say whether its weapon aims
/// at the surface, so it keeps the surface search's tone before it
/// (fitted, agent decision 2026-09-26).
fn seeker_tone(name: &str, surface: &mut bool) -> Option<SeekerTone> {
    let (radar, locked, ground) = match name {
        "radar lock" => (true, true, false),
        "radar search" => (true, false, false),
        "infrared lock" => (false, true, *surface),
        "ground" => (false, false, true),
        "infrared search" => (false, false, false),
        _ => return None,
    };
    *surface = ground;
    let strength = if radar {
        Seeker {
            status: if locked {
                Status::Locked
            } else {
                Status::Acquiring
            },
            quality: TONE_QUALITY,
            ..Seeker::default()
        }
        .tone()
    } else {
        SeekerTone::ir_strength(TONE_PERCENT, locked)
    };
    Some(SeekerTone {
        strength,
        ground,
        radar,
        locked,
    })
}

/// The stall warning recording a recorded sound names.
fn stall_sound(name: &str) -> Option<&'static str> {
    [DepartureMode::Warning, DepartureMode::Stalled]
        .into_iter()
        .filter_map(|mode| audio::stall_cue(Some(mode)))
        .find(|sound| *sound == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::tests::TempDir;
    use std::collections::BTreeMap;
    use tore_replay::{
        AircraftFlags, AircraftInfo, AircraftState, Frame, ProjectileState, Side, Value,
        WeaponClass, WeaponInfo, WriterOptions,
    };
    use tore_sim::acoustics::SourceId;

    const LAST: u64 = 1_200;
    /// The player's engine stops, and later the player's aircraft is lost.
    const FLAMEOUT: u64 = 900;
    const CRASH: u64 = 1_100;
    /// The player's missile flies from the first tick until the second.
    const MISSILE: (u64, u64) = (200, 300);
    /// A frame shorter than a tick, so every tick gets a frame of its own
    /// and every playhead is exact in binary.
    const FINE: f64 = 1. / 128.;

    fn position(id: u32, tick: u64) -> [f64; 3] {
        let t = tick as f64 / 120.;
        [
            f64::from(id) * 2_000.,
            10_000.,
            800. * t + f64::from(id) * 500.,
        ]
    }

    fn aircraft(id: u32, tick: u64) -> AircraftState {
        let player = id == 0;
        AircraftState {
            id,
            position: position(id, tick),
            velocity: [0., 0., 800.],
            flags: AircraftFlags {
                engine_on: !(player && tick >= FLAMEOUT),
                afterburner: player && tick < 100,
                airborne: true,
                crashed: player && tick >= CRASH,
                alive: !(player && tick >= CRASH),
                animated: true,
                ..Default::default()
            },
            controls: [0., 0., 0., 0.6],
            hp: 100,
            max_hp: 100,
            ..Default::default()
        }
    }

    /// Every kind of recorded cue, plus journal entries and orders that
    /// must stay silent.
    fn events(tick: u64) -> Vec<Event> {
        use vocab::{field, kind, outcome};
        let line = |kind: &str, stems: &str, route: &str| {
            Event::new(kind)
                .with(field::STEMS, stems)
                .with(field::ROUTE, route)
        };
        let heard = |event: Event| {
            event
                .with(field::HEARD, true)
                .with(field::OUTCOME, outcome::DELIVERED)
        };
        let tone = |name: &str, on: bool| {
            Event::new(kind::AUDIO_TONE)
                .with_subject(0)
                .with(field::TONE, name)
                .with(field::ON, on)
        };
        let stall = |sound: &str, on: bool| {
            Event::new(kind::AUDIO_STALL_WARNING)
                .with_subject(0)
                .with(field::ON, on)
                .with(field::SOUND, sound)
        };
        match tick {
            5 => vec![
                heard(line(kind::COMMS_RADIO, "^FOX2 ^RED2", "radio").with_subject(2)),
                line(kind::COMMS_RADIO, "^CONTACT", "radio")
                    .with(field::HEARD, false)
                    .with(field::OUTCOME, "unheard"),
            ],
            6 => vec![
                line(kind::COMMS_RADIO, "^SPLASH", "radio")
                    .with(field::HEARD, true)
                    .with(field::OUTCOME, outcome::QUEUED),
            ],
            20 => vec![heard(line(kind::COMMS_CREW, "^CHECK6", "radio"))],
            30 => vec![heard(line(kind::COMMS_TOWER, "^RWYFREE", "tower"))],
            40 => vec![
                heard(line(kind::COMMS_TOWER, "^CLRLAND", "tower"))
                    .with(field::TRIGGER, PLAYER_REQUEST),
            ],
            41 => vec![
                heard(line(kind::COMMS_TOWER, "", "tower")).with(field::TRIGGER, PLAYER_REQUEST),
            ],
            50 => vec![heard(line(kind::COMMS_RADIO, "^SCREAM", DIRECT))],
            60 => vec![
                Event::new(kind::COMMS_ORDER)
                    .with_subject(0)
                    .with(field::STEMS, "^ENGAGE ^MYTGT"),
            ],
            61 => vec![
                Event::new(kind::COMMS_ORDER)
                    .with_subject(0)
                    .with(field::STEMS, "")
                    .with(field::OUTCOME, outcome::REJECTED),
            ],
            62 => vec![
                Event::new(kind::COMMS_ORDER)
                    .with_subject(1)
                    .with(field::STEMS, "^ATTACK"),
            ],
            70 => vec![Event::new(kind::COMMS_HUD).with_text("Radar on")],
            100 => vec![tone("infrared search", true)],
            130 => vec![tone("infrared lock", true)],
            160 => vec![tone("infrared lock", false)],
            170 => vec![tone("ground", true)],
            180 => vec![tone("infrared lock", true)],
            190 => vec![tone("infrared lock", false)],
            200 => vec![
                Event::new(kind::AUDIO_RELEASE)
                    .with_subject(0)
                    .with(field::SOUND, "&MSLFIRE.5K")
                    .with(field::WEAPON, Value::Id(1)),
            ],
            300 => vec![
                Event::new(kind::AUDIO_EFFECT)
                    .with(field::SOUND, "explosion")
                    .with(field::X_FT, 1_000.)
                    .with(field::Y_FT, 9_000.)
                    .with(field::Z_FT, 3_000.),
            ],
            400 => vec![stall("&STALLWR.5K", true)],
            420 => vec![stall("&STALL.5K", true)],
            460 => vec![stall("&STALL.5K", false)],
            500 => vec![
                Event::new(kind::AUDIO_DEVICE)
                    .with_subject(0)
                    .with(field::DEVICE, "gear")
                    .with(field::SOUND, "&GEARDWN.5K"),
            ],
            600 => vec![
                Event::new(kind::AUDIO_EJECTION)
                    .with_subject(2)
                    .with(field::SOUND, "^PUNCH.5K"),
            ],
            1_000 => vec![
                Event::new(kind::AUDIO_EJECTION)
                    .with_subject(0)
                    .with(field::SOUND, "^EJECTX3.5K"),
            ],
            _ => Vec::new(),
        }
    }

    fn frame(tick: u64) -> Frame {
        let missile = (MISSILE.0..MISSILE.1).contains(&tick).then(|| {
            let [x, y, z] = position(0, tick);
            ProjectileState {
                id: 7,
                owner: 0,
                weapon: 1,
                position: [x, y, z + 1_000.],
                previous: [x, y, z + 990.],
                direction: [0., 0., 1.],
                speed: 1_200.,
                ..Default::default()
            }
        });
        Frame {
            tick,
            aircraft: (0..3).map(|id| aircraft(id, tick)).collect(),
            projectiles: missile.into_iter().collect(),
            events: events(tick),
            ..Default::default()
        }
    }

    /// Ten seconds of synthetic recording: the player and a wingman in
    /// F/A-18Ds, an enemy MiG-29, and the events above.
    fn recording(dir: &TempDir, name: &str) -> Arc<Recording> {
        let path = dir.path().join(format!("{name}.tore-replay"));
        let mut writer = tore_replay::Writer::create_with(
            &path,
            &tore_replay::Header::default(),
            WriterOptions {
                chunk_ticks: 120,
                sync_ticks: 3_600,
            },
        )
        .unwrap();
        for (id, pt, side) in [
            (0, "F18.PT", Side::Friendly),
            (1, "MIG29.PT", Side::Enemy),
            (2, "F18.PT", Side::Friendly),
        ] {
            writer
                .register_aircraft(&AircraftInfo {
                    id,
                    pt: pt.into(),
                    side,
                    ..Default::default()
                })
                .unwrap();
        }
        writer
            .register_weapon(&WeaponInfo {
                id: 1,
                source: "AIM9M.JT".into(),
                shape: None,
                name: "AIM-9M".into(),
                class: WeaponClass::Missile,
            })
            .unwrap();
        for tick in 0..=LAST {
            writer.push(&frame(tick)).unwrap();
        }
        let path = writer.finish(&tore_replay::Footer::default()).unwrap();
        Arc::new(Recording::open(path).unwrap())
    }

    fn engines() -> Vec<(AircraftId, EngineSounds)> {
        let sounds = |stem: &str| EngineSounds {
            engine: Some(format!("&{stem}LOOP.11K")),
            burner: Some(format!("&{stem}AB.11K")),
            start: Some(format!("&{stem}ON.5K")),
            stop: Some(format!("&{stem}OFF.5K")),
        };
        vec![
            (AircraftId::F18, sounds("F18")),
            (AircraftId::Mig29, sounds("MIG")),
        ]
    }

    /// The viewer's side of the call: the clock, the camera and a frame.
    struct Host {
        clock: Clock,
        sound: ReplaySound,
        camera: Camera,
        view: Option<u8>,
        selected: u32,
        scrubbing: bool,
    }

    impl Host {
        fn new(recording: &Arc<Recording>) -> Self {
            Self {
                clock: Clock::new(0, LAST),
                sound: ReplaySound::with_engines(Arc::clone(recording), engines()),
                camera: Camera::new(),
                view: Some(1),
                selected: 0,
                scrubbing: false,
            }
        }

        /// One frame `seconds` long, advanced as the viewer advances it,
        /// played on `audio` when given.
        fn step(&mut self, seconds: f64, audio: bool) -> Vec<(u64, Cue)> {
            let from = self.clock.position();
            if !self.scrubbing {
                self.clock.advance(seconds);
            }
            let moment = Moment {
                from,
                clock: &self.clock,
                scrubbing: self.scrubbing,
                camera: &self.camera,
                view: self.view,
                selected: self.selected,
            };
            if audio {
                self.sound.frame(None, &moment);
                Vec::new()
            } else {
                self.sound.schedule(&moment)
            }
        }

        fn frame(&mut self, seconds: f64) -> Vec<(u64, Cue)> {
            self.step(seconds, false)
        }

        /// Frames of `seconds` until the playhead reaches `tick` or stops.
        fn play_to(&mut self, tick: u64, seconds: f64) -> Vec<(u64, Cue)> {
            let mut cues = self.frame(seconds);
            while self.clock.position() < tick as f64 && !self.clock.paused() {
                cues.extend(self.frame(seconds));
            }
            cues
        }
    }

    /// The cues a listener notices once: all but the per-tick traveling
    /// sound and the per-frame loops.
    fn heard(cues: &[(u64, Cue)]) -> Vec<(u64, String)> {
        cues.iter()
            .filter(|(_, cue)| !matches!(cue, Cue::Tick(_) | Cue::Loops { .. }))
            .map(|(tick, cue)| (*tick, cue.describe()))
            .collect()
    }

    fn spatial(cues: &[(u64, Cue)]) -> Vec<(u64, &SpatialTick)> {
        cues.iter()
            .filter_map(|(tick, cue)| match cue {
                Cue::Tick(t) => Some((*tick, &**t)),
                _ => None,
            })
            .collect()
    }

    #[allow(clippy::type_complexity)]
    fn loops(cues: &[(u64, Cue)]) -> Vec<(u64, Option<EngineLoops>, Option<&'static str>)> {
        cues.iter()
            .filter_map(|(tick, cue)| match cue {
                Cue::Loops { engine, stall } => Some((*tick, engine.clone(), *stall)),
                _ => None,
            })
            .collect()
    }

    /// The seeker cue for a tone, at the loudness the replay assumes.
    fn seeker(radar: bool, ground: bool, locked: bool) -> String {
        let strength = if radar {
            Seeker {
                status: if locked {
                    Status::Locked
                } else {
                    Status::Acquiring
                },
                quality: TONE_QUALITY,
                ..Seeker::default()
            }
            .tone()
        } else {
            SeekerTone::ir_strength(TONE_PERCENT, locked)
        };
        Cue::Seeker(Some(SeekerTone {
            strength,
            ground,
            radar,
            locked,
        }))
        .describe()
    }

    #[test]
    fn every_heard_cue_plays_once_in_order_at_normal_speed() {
        let dir = TempDir::new("sound-order");
        let recording = recording(&dir, "order");
        let mut host = Host::new(&recording);
        let cues = host.play_to(LAST, FINE);
        assert!(host.clock.paused(), "playback stops at the end");
        let expected: Vec<(u64, String)> = vec![
            (0, "seeker tone off".to_owned()),
            (5, "radio ^FOX2 ^RED2".into()),
            (20, "radio ^CHECK6".into()),
            (30, "tower ^RWYFREE".into()),
            (40, "tower reply ^CLRLAND".into()),
            (41, "tower reply without a recording".into()),
            (50, "direct voice ^SCREAM".into()),
            (60, "order voice ^ENGAGE ^MYTGT".into()),
            (100, seeker(false, false, false)),
            (130, seeker(false, false, true)),
            (160, "seeker tone off".into()),
            (170, seeker(false, true, false)),
            (180, seeker(false, true, true)),
            (190, "seeker tone off".into()),
            (500, "effect &GEARDWN.5K".into()),
            (600, "wingman ejected".into()),
            (FLAMEOUT, "effect &F18OFF.5K".into()),
            (1_000, "ejection ^EJECTX3.5K".into()),
            (CRASH, "player lost: tower speech cancelled".into()),
            (LAST, "paused".into()),
        ];
        assert_eq!(heard(&cues), expected);
        // Every tick's traveling sound once, in order.
        let ticks: Vec<u64> = spatial(&cues).iter().map(|(tick, _)| *tick).collect();
        assert_eq!(ticks, (0..=LAST).collect::<Vec<_>>());
        // Paused at the end, nothing more plays.
        for _ in 0..10 {
            assert!(host.frame(FINE).is_empty());
        }
    }

    #[test]
    fn frames_of_any_length_play_each_tick_once() {
        let dir = TempDir::new("sound-frames");
        let recording = recording(&dir, "frames");
        let reference = Host::new(&recording).play_to(LAST, FINE);
        // The seeker is sampled once a frame, so a long frame can pass over
        // a short tone, as in flight; everything else plays exactly once.
        let once = |cues: &[(u64, Cue)]| -> Vec<String> {
            heard(cues)
                .into_iter()
                .map(|(_, text)| text)
                .filter(|text| !text.starts_with("seeker"))
                .collect()
        };
        for lengths in [
            &[1. / 60.][..],
            &[0.001, 0.02, 0.0137, 0.25, 0.004],
            &[0.25],
        ] {
            let mut host = Host::new(&recording);
            let mut cues = Vec::new();
            for seconds in lengths.iter().cycle() {
                cues.extend(host.frame(*seconds));
                if host.clock.paused() {
                    break;
                }
            }
            assert_eq!(once(&cues), once(&reference), "{lengths:?}");
            let ticks: Vec<u64> = spatial(&cues).iter().map(|(tick, _)| *tick).collect();
            assert_eq!(ticks, (0..=LAST).collect::<Vec<_>>(), "{lengths:?}");
            // A recorded cue keeps its own tick, whatever the frame.
            let gear = heard(&cues)
                .into_iter()
                .find(|(_, text)| text == "effect &GEARDWN.5K");
            assert_eq!(gear.map(|(tick, _)| tick), Some(500), "{lengths:?}");
        }
    }

    #[test]
    fn other_speeds_reverse_and_scrubbing_are_silent() {
        let dir = TempDir::new("sound-speeds");
        let recording = recording(&dir, "speeds");
        for speed in [2., 16., 0.75, 0.5, 0.125, -1., -2.] {
            let mut host = Host::new(&recording);
            if speed < 0. {
                host.clock.end();
            }
            assert!(host.clock.set_speed(speed));
            for _ in 0..200 {
                assert!(host.frame(1. / 60.).is_empty(), "{speed}x");
            }
        }
        // Leaving 1x, reversing or dragging the timeline cancels what plays,
        // once, and stays silent.
        let leaves: [fn(&mut Host); 4] = [
            |h| h.clock.faster(),
            |h| h.clock.slower(),
            |h| h.clock.reverse(),
            |h| {
                h.scrubbing = true;
                h.clock.seek(300.);
            },
        ];
        for leave in leaves {
            let mut host = Host::new(&recording);
            host.play_to(65, FINE);
            leave(&mut host);
            let cues = host.frame(1. / 60.);
            assert!(matches!(cues[..], [(_, Cue::Reset)]), "{cues:?}");
            for _ in 0..30 {
                if host.scrubbing {
                    host.clock.seek(host.clock.position() + 3.);
                }
                assert!(host.frame(1. / 60.).is_empty());
            }
        }
        // Back at 1x, sound starts afresh from the playhead.
        let mut host = Host::new(&recording);
        host.clock.set_speed(2.);
        host.play_to(95, FINE);
        host.clock.slower();
        let cues = host.play_to(135, FINE);
        let texts: Vec<String> = heard(&cues).into_iter().map(|(_, t)| t).collect();
        assert_eq!(
            texts,
            [
                "seeker tone off".to_owned(),
                seeker(false, false, false),
                seeker(false, false, true)
            ]
        );
    }

    #[test]
    fn a_jump_cancels_speech_and_starts_afresh_at_the_playhead() {
        let dir = TempDir::new("sound-seek");
        let recording = recording(&dir, "seek");
        let mut host = Host::new(&recording);
        host.play_to(70, FINE);
        // Back to tick 50: the direct voice and the order play again.
        host.clock.seek(50.);
        let cues = host.play_to(65, FINE);
        assert!(matches!(cues.first(), Some((_, Cue::Reset))));
        // A frame's recorded cues come first, then its steady sounds.
        assert_eq!(
            heard(&cues[1..]),
            [
                (50, "direct voice ^SCREAM".to_owned()),
                (50, "seeker tone off".into()),
                (60, "order voice ^ENGAGE ^MYTGT".into()),
            ]
        );
        assert_eq!(spatial(&cues).first().map(|(tick, _)| *tick), Some(50));
        // Forward into the stall: nothing in between plays, and the stall
        // warning sounding there comes back at once.
        host.clock.seek(450.);
        let cues = host.frame(FINE);
        assert!(matches!(cues.first(), Some((_, Cue::Reset))));
        assert_eq!(heard(&cues[1..]), [(450, "seeker tone off".to_owned())]);
        let ticks: Vec<u64> = spatial(&cues).iter().map(|(tick, _)| *tick).collect();
        assert_eq!(ticks, [450]);
        assert_eq!(loops(&cues)[0].2, Some("&STALL.5K"));
        // Paused, a step is a jump too; playing on starts at the new tick.
        host.clock.pause();
        assert!(matches!(host.frame(FINE)[..], [(_, Cue::Pause(true))]));
        host.clock.step(50);
        assert!(matches!(host.frame(FINE)[..], [(_, Cue::Reset)]));
        assert!(host.frame(FINE).is_empty());
        host.clock.play();
        let cues = host.frame(FINE);
        assert_eq!(
            heard(&cues),
            [
                (500, "effect &GEARDWN.5K".to_owned()),
                (500, "seeker tone off".into())
            ]
        );
    }

    #[test]
    fn pausing_freezes_and_playing_on_resumes_without_repeats() {
        let dir = TempDir::new("sound-pause");
        let recording = recording(&dir, "pause");
        let mut host = Host::new(&recording);
        host.play_to(55, FINE);
        let played = host.clock.position().floor() as u64;
        host.clock.pause();
        let cues = host.frame(FINE);
        assert!(matches!(cues[..], [(_, Cue::Pause(true))]));
        for _ in 0..20 {
            assert!(host.frame(FINE).is_empty());
        }
        // Turning round and pausing again before a frame moved the
        // playhead is not a jump.
        host.clock.reverse();
        host.clock.pause();
        assert!(host.frame(FINE).is_empty());
        host.clock.forward();
        let cues = host.play_to(65, FINE);
        assert!(matches!(cues.first(), Some((_, Cue::Pause(false)))));
        assert!(!cues.iter().any(|(_, cue)| matches!(cue, Cue::Reset)));
        assert_eq!(
            spatial(&cues).first().map(|(tick, _)| *tick),
            Some(played + 1)
        );
        assert_eq!(
            heard(&cues[1..]),
            [(60, "order voice ^ENGAGE ^MYTGT".to_owned())]
        );
    }

    #[test]
    fn the_listener_is_the_viewer_camera() {
        let mut camera = Camera::new();
        camera.position = [100., 2_000., -300.];
        [camera.yaw, camera.pitch, camera.roll] = [0.7, -0.2, 0.3];
        let heard = listener(&camera, 4);
        assert_eq!(heard.position, [100., 2_000., -300.]);
        let basis = Basis::new(f64::from(0.7f32), f64::from(-0.2f32), -f64::from(0.3f32));
        assert_eq!(heard.right, basis.right);
        assert_eq!(heard.view, 4);
        assert!(heard.external);
        // Only a camera in the player's own cockpit is inside.
        camera.hidden_target = Some(0);
        assert!(!listener(&camera, 0).external);
        camera.hidden_target = Some(3);
        assert!(listener(&camera, 0).external);
        // Looking north, an explosion to the east is on the right.
        let mut north = Camera::new();
        [north.yaw, north.pitch, north.roll] = [0.; 3];
        north.position = [0., 1_000., 0.];
        let mix =
            tore_sim::acoustics::mix(Kind::Explosion, [800., 1_000., 0.], listener(&north, 0));
        assert!(mix.pan > 0.9, "{}", mix.pan);
        // Every tick is heard from the frame's camera, and a cut to another
        // view, aircraft or the drone is a new view for the pass detector.
        let dir = TempDir::new("sound-listener");
        let recording = recording(&dir, "listener");
        let mut host = Host::new(&recording);
        host.camera = camera;
        let views = |cues: &[(u64, Cue)]| -> Vec<u8> {
            let mut views: Vec<u8> = spatial(cues).iter().map(|(_, t)| t.listener.view).collect();
            views.dedup();
            views
        };
        let cues = host.play_to(10, FINE);
        assert!(
            spatial(&cues)
                .iter()
                .all(|(_, t)| t.listener.position == [100., 2_000., -300.] && t.listener.external)
        );
        let mut seen = views(&cues);
        assert_eq!(seen.len(), 1);
        let cuts: [fn(&mut Host); 3] =
            [|h| h.view = Some(0), |h| h.selected = 1, |h| h.view = None];
        for cut in cuts {
            cut(&mut host);
            let now = views(&host.frame(FINE * 2.));
            assert_eq!(now.len(), 1);
            assert!(!seen.contains(&now[0]));
            seen.extend(&now);
            assert_eq!(views(&host.frame(FINE * 2.)), now, "no cut, same view");
        }
    }

    #[test]
    fn traveling_sound_comes_from_the_recorded_tick() {
        let dir = TempDir::new("sound-spatial");
        let recording = recording(&dir, "spatial");
        let cues = Host::new(&recording).play_to(LAST, FINE);
        let ticks: BTreeMap<u64, &SpatialTick> = spatial(&cues).into_iter().collect();
        let release = ticks[&MISSILE.0];
        assert_eq!(release.releases, ["&MSLFIRE.5K"]);
        // Positions come back within the recording's 1/32 ft.
        let near =
            |a: [f64; 3], b: [f64; 3], within: f64| (0..3).all(|i| (a[i] - b[i]).abs() <= within);
        assert!(near(release.player, position(0, MISSILE.0), 1. / 32.));
        let ids = |t: &SpatialTick| t.sources.iter().map(|s| s.id).collect::<Vec<_>>();
        assert_eq!(
            ids(release),
            [
                SourceId::Aircraft(0),
                SourceId::Aircraft(1),
                SourceId::Aircraft(2),
                SourceId::Missile(7)
            ]
        );
        assert!(near(
            release.sources[3].velocity,
            [0., 0., 1_200.],
            120. / 32.
        ));
        assert_eq!(ids(ticks[&MISSILE.1]).len(), 3, "the missile is gone");
        let explosion = &ticks[&300].emissions;
        assert_eq!(explosion.len(), 1);
        assert_eq!(explosion[0].kind, Kind::Explosion);
        assert_eq!(explosion[0].position, [1_000., 9_000., 3_000.]);
        assert!(!explosion[0].arrived);
        assert_eq!(
            ticks.values().filter(|t| !t.emissions.is_empty()).count(),
            1
        );
    }

    #[test]
    fn the_watched_engine_and_the_stall_warning_are_steady_sounds() {
        let dir = TempDir::new("sound-steady");
        let recording = recording(&dir, "steady");
        let cues = Host::new(&recording).play_to(LAST, FINE);
        let steady = loops(&cues);
        let at = |tick: u64| steady.iter().find(|(t, ..)| *t == tick).unwrap().clone();
        let (_, engine, stall) = at(50);
        let engine = engine.unwrap();
        assert_eq!(engine.aircraft, AircraftId::F18);
        assert_eq!(engine.sounds.engine.as_deref(), Some("&F18LOOP.11K"));
        assert!(engine.running && engine.afterburner);
        assert!((engine.throttle - 0.6).abs() < 1e-3);
        assert_eq!(stall, None);
        assert_eq!(at(410).2, Some("&STALLWR.5K"));
        assert_eq!(at(440).2, Some("&STALL.5K"));
        assert_eq!(at(460).2, None);
        assert!(!at(FLAMEOUT).1.unwrap().running);
        // Watching the MiG swaps the loops without a start or stop sound;
        // an aircraft missing from the recording has no engine.
        let mut host = Host::new(&recording);
        host.play_to(20, FINE);
        host.selected = 1;
        let cues = host.frame(FINE);
        assert!(heard(&cues).is_empty());
        let engine = loops(&cues)[0].1.clone().unwrap();
        assert_eq!(engine.aircraft, AircraftId::Mig29);
        host.selected = 99;
        assert!(loops(&host.frame(FINE))[0].1.is_none());
        // The MiG's engine never stops, so watching it through the
        // player's flameout makes no sound.
        host.selected = 1;
        let cues = host.play_to(LAST, 1. / 60.);
        assert!(!heard(&cues).iter().any(|(_, t)| t.contains("OFF")));
    }

    #[test]
    fn recorded_tone_names_become_seeker_tones() {
        let mut surface = false;
        let mut tone = |name| seeker_tone(name, &mut surface).unwrap();
        let lock = tone("radar lock");
        assert!(lock.radar && lock.locked && !lock.ground);
        assert!((lock.strength - 0.7).abs() < 1e-9);
        assert!((tone("radar search").strength - 0.425).abs() < 1e-9);
        assert!((tone("infrared search").strength - 0.2875).abs() < 1e-9);
        assert!(!tone("infrared lock").ground);
        assert!(tone("ground").ground);
        let lock = tone("infrared lock");
        assert!(lock.ground && lock.locked && !lock.radar);
        assert!((lock.strength - 0.575).abs() < 1e-9);
        assert!(seeker_tone("hum", &mut false).is_none());
        assert_eq!(stall_sound("&STALLWR.5K"), Some("&STALLWR.5K"));
        assert_eq!(stall_sound("&STALL.5K"), Some("&STALL.5K"));
        assert_eq!(stall_sound("&BEEP.5K"), None);
    }

    /// Prints what a real recording schedules when played through at 1x
    /// in frames of a sixtieth of a second, for checking by eye without a
    /// display: `TORE_REPLAY_SOUND_FILE=FILE cargo test --locked -p tore-app
    /// replay::sound -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads the recording named by TORE_REPLAY_SOUND_FILE"]
    fn schedule_a_recording() {
        let Ok(path) = std::env::var("TORE_REPLAY_SOUND_FILE") else {
            return;
        };
        let recording = Arc::new(Recording::open(&path).unwrap());
        let (first, last) = (
            recording.first_tick().unwrap(),
            recording.last_tick().unwrap(),
        );
        let mut host = Host::new(&recording);
        host.clock = Clock::new(first, last);
        host.sound = ReplaySound::new(Arc::clone(&recording), std::iter::empty());
        let mut total = 0;
        while !host.clock.paused() {
            for (tick, cue) in host.frame(1. / 60.) {
                total += 1;
                let quiet = match &cue {
                    Cue::Tick(t) => t.emissions.is_empty() && t.releases.is_empty(),
                    Cue::Loops { .. } => true,
                    _ => false,
                };
                if !quiet {
                    println!(
                        "{tick:>7} {} {}",
                        clock::timestamp(tick as f64),
                        cue.describe()
                    );
                }
            }
        }
        println!("{total} cues for ticks {first} to {last}");
    }

    #[test]
    fn nothing_breaks_without_a_sound_device() {
        let dir = TempDir::new("sound-device");
        let recording = recording(&dir, "device");
        let mut host = Host::new(&recording);
        host.sound.log = true;
        for step in 0..400 {
            match step {
                100 => host.clock.seek(600.),
                150 => host.clock.reverse(),
                200 => host.clock.play(),
                250 => host.clock.pause(),
                300 => host.clock.play(),
                _ => {}
            }
            host.step(1. / 60., true);
        }
        let silent = ReplaySound::new(Arc::clone(&recording), std::iter::empty());
        assert!(silent.engines.is_empty());
    }
}
