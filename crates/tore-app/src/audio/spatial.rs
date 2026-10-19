//! Original recordings driven by the device-independent acoustic model.
use super::{Clip, Voice};
use std::{collections::BTreeMap, sync::Arc};
use tore_sim::acoustics::{self, Arrival, Emission, Field, Kind, Listener, Mix, Passes, Source};

struct SpatialVoice {
    voice: Voice,
    mix: Mix,
    kind: Kind,
    /// A weapon launch: its record's `maxSndDist`, which sets how it fades
    /// with distance in place of `kind`'s curve.
    launch_ft: Option<i16>,
    other: bool,
    gun: bool,
    position: [f64; 3],
    /// Heard inside the releasing aircraft's own cockpit: its mix never
    /// follows distance or direction.
    cockpit: bool,
    filtered: f32,
    stereo_gain: [f32; 2],
    target_gain: [f32; 2],
    filter_rate: f64,
    filter_alpha: f32,
    gain_smoothing: f32,
}

/// A steady sound heard from a place: an engine or a fire. Gain, stereo
/// and treble follow its mix smoothly, and its pitch its Doppler ratio.
pub(super) struct Placed {
    gain: [f32; 2],
    target: [f32; 2],
    cutoff: f32,
    filtered: f32,
    pitch: f64,
    target_pitch: f64,
    smoothing: Smoothing,
}

/// The smoothing coefficients of a [`Placed`] sound at one sample rate: they
/// depend on the rate (and the treble filter's on the cutoff) alone, so they
/// are worked out when either changes instead of at every sample, which is
/// up to three `exp` per sample per voice. Each is the very expression the
/// per-sample code evaluated, so the sound is the same bit for bit.
#[derive(Clone, Copy)]
struct Smoothing {
    rate: f64,
    /// Gains glide over about 50 ms.
    glide: f32,
    /// Pitch glides over 100 ms.
    pitch: f64,
    /// The treble filter for the cutoff whose bits are held, at `rate`.
    filter: Option<(u32, f32)>,
}
impl Smoothing {
    fn at(rate: f64) -> Self {
        Self {
            rate,
            glide: 1. - (-1. / (0.05 * rate)).exp() as f32,
            pitch: 1. - (-1. / (0.1 * rate)).exp(),
            filter: None,
        }
    }
    /// The one-pole filter coefficient for `cutoff` hertz.
    fn alpha(&mut self, cutoff: f32) -> f32 {
        let bits = cutoff.to_bits();
        match self.filter {
            Some((held, alpha)) if held == bits => alpha,
            _ => {
                let alpha =
                    1. - (-std::f64::consts::TAU * f64::from(cutoff) / self.rate).exp() as f32;
                self.filter = Some((bits, alpha));
                alpha
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    /// Tests only: sound is worked out the way it was before the
    /// coefficients were cached, to hold the cached sound to it.
    pub(super) static REFERENCE_MATH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl Placed {
    /// Heard from inside the aircraft: centered, full and unfiltered.
    pub fn inside() -> Self {
        Self {
            gain: [1.; 2],
            target: [1.; 2],
            cutoff: 12000.,
            filtered: 0.,
            pitch: 1.,
            target_pitch: 1.,
            smoothing: Smoothing::at(f64::NAN),
        }
    }
    pub fn aim(&mut self, mix: Option<Mix>, pitch: f64, stereo: Stereo) {
        match mix {
            Some(mix) => {
                self.target = stereo_gain(mix, stereo);
                self.cutoff = mix.cutoff;
            }
            None => {
                self.target = [1.; 2];
                self.cutoff = 12000.;
            }
        }
        self.target_pitch = pitch;
    }
    /// The playback rate divisor for the pitch now.
    pub fn pitch(&self) -> f64 {
        self.pitch
    }
    pub fn silent(&self) -> bool {
        self.gain.iter().chain(&self.target).all(|g| g.abs() < 1e-4)
    }
    /// One sample placed in stereo. Gains glide over about 50 ms, so a view
    /// change crossfades instead of stepping, and pitch over 100 ms.
    pub fn apply(&mut self, sample: f32, rate: f64) -> [f32; 2] {
        #[cfg(test)]
        if REFERENCE_MATH.get() {
            return self.apply_reference(sample, rate);
        }
        if self.smoothing.rate != rate {
            self.smoothing = Smoothing::at(rate);
        }
        let glide = self.smoothing.glide;
        for (gain, target) in self.gain.iter_mut().zip(self.target) {
            *gain += glide * (target - *gain);
        }
        self.pitch += self.smoothing.pitch * (self.target_pitch - self.pitch);
        let value = if self.cutoff >= 12000. {
            sample
        } else {
            let alpha = self.smoothing.alpha(self.cutoff);
            self.filtered += alpha * (sample - self.filtered);
            self.filtered
        };
        [value * self.gain[0], value * self.gain[1]]
    }
    /// [`Self::apply`] as it was, every coefficient worked out at every
    /// sample (tests only).
    #[cfg(test)]
    fn apply_reference(&mut self, sample: f32, rate: f64) -> [f32; 2] {
        let glide = 1. - (-1. / (0.05 * rate)).exp() as f32;
        for (gain, target) in self.gain.iter_mut().zip(self.target) {
            *gain += glide * (target - *gain);
        }
        self.pitch += (1. - (-1. / (0.1 * rate)).exp()) * (self.target_pitch - self.pitch);
        let value = if self.cutoff >= 12000. {
            sample
        } else {
            let alpha = 1. - (-std::f64::consts::TAU * f64::from(self.cutoff) / rate).exp() as f32;
            self.filtered += alpha * (sample - self.filtered);
            self.filtered
        };
        [value * self.gain[0], value * self.gain[1]]
    }
}

struct LoopVoice {
    voice: Voice,
    placed: Placed,
    gain: f32,
    heard: bool,
}

/// Loops played at once, loudest first.
const MAX_LOOPS: usize = 10;

/// The Stereo Separation slider, 0 to 100, and Swap Left/Right Channels,
/// applied to every positioned sound (docs/spec/sound-prefs.md#stereo).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stereo {
    pub separation: u8,
    pub swap: bool,
}
impl Default for Stereo {
    /// Separation 50 leaves every direction as heard.
    fn default() -> Self {
        Self {
            separation: 50,
            swap: false,
        }
    }
}
impl Stereo {
    /// A sound's pan, -1 left to 1 right, after separation and swap. The
    /// pan's angle from ahead (behind mirrors to the front) is widened by
    /// `(s - 50) / 16` of itself above 50 and narrowed by `(s - 50) / 50`
    /// below it, up to hard left or right; 0 is mono.
    pub fn pan(self, pan: f32) -> f32 {
        let angle = pan.clamp(-1., 1.).asin().to_degrees();
        let s = f32::from(self.separation) - 50.;
        let factor = if s > 0. { s / 16. } else { s / 50. };
        let pan = (angle + factor * angle).clamp(-90., 90.).to_radians().sin();
        if self.swap { -pan } else { pan }
    }
}

#[derive(Default)]
pub(super) struct Scene {
    /// Set by the mixer from the Sound/Music Prefs.
    pub stereo: Stereo,
    field: Field<Arc<Clip>>,
    passes: Passes,
    voices: Vec<SpatialVoice>,
    loops: BTreeMap<(u8, u64), LoopVoice>,
    other_fire: super::other::Fire,
}
impl Scene {
    pub fn clear(&mut self) {
        *self = Self {
            stereo: self.stereo,
            ..Self::default()
        };
    }
    /// This tick's fires and engines, heard from `listener` moving at
    /// `listener_velocity`. The loudest play; the rest, and any that ended,
    /// fade out.
    pub fn loops(
        &mut self,
        clips: &BTreeMap<String, Arc<Clip>>,
        listener: Listener,
        listener_velocity: [f64; 3],
        sources: &[super::LoopSource],
        engine: f32,
        enabled: bool,
    ) {
        for voice in self.loops.values_mut() {
            voice.heard = false;
        }
        if enabled {
            let mut heard: Vec<_> = sources
                .iter()
                .map(|s| {
                    // Other aircraft's engines follow the Engine level;
                    // fires do not.
                    let gain = if matches!(s.key.0, 1 | 2) {
                        s.gain * engine
                    } else {
                        s.gain
                    };
                    let mix = acoustics::loop_mix(
                        s.position,
                        listener,
                        s.reference,
                        s.maximum,
                        f64::from(gain),
                    );
                    (mix, s)
                })
                .filter(|(mix, _)| mix.gain > 0.)
                .collect();
            heard.sort_by(|a, b| b.0.gain.total_cmp(&a.0.gain));
            for (mix, source) in heard.into_iter().take(MAX_LOOPS) {
                let Some(clip) = clips.get(&source.clip) else {
                    continue;
                };
                let pitch = acoustics::doppler(
                    source.position,
                    source.velocity,
                    listener.position,
                    listener_velocity,
                    super::DOPPLER_LIMITS,
                ) * source.pitch;
                let voice = self.loops.entry(source.key).or_insert_with(|| LoopVoice {
                    voice: Voice {
                        clip: clip.clone(),
                        // Start each loop somewhere different so two fires
                        // or two jets never sound in step.
                        position: (source.key.1 % 997) as f64 / 997. * clip.samples.len() as f64,
                    },
                    placed: {
                        let mut placed = Placed::inside();
                        placed.gain = [0.; 2];
                        placed.pitch = pitch;
                        placed
                    },
                    gain: 0.,
                    heard: true,
                });
                voice.placed.aim(Some(mix), pitch, self.stereo);
                voice.gain = mix.gain;
                voice.heard = true;
            }
        }
        for voice in self.loops.values_mut().filter(|v| !v.heard) {
            voice.placed.aim(
                Some(Mix {
                    gain: 0.,
                    pan: 0.,
                    cutoff: voice.placed.cutoff,
                }),
                voice.placed.target_pitch,
                self.stereo,
            );
        }
        self.loops.retain(|_, v| v.heard || !v.placed.silent());
    }
    pub fn tick(
        &mut self,
        clips: &BTreeMap<String, Arc<Clip>>,
        listener: Listener,
        sources: &[Source],
        emissions: &[Emission],
        enabled: bool,
    ) {
        let passes = self.passes.step(listener, sources);
        if !enabled {
            self.field.clear();
            self.other_fire = super::other::Fire::default();
            self.voices.clear();
            return;
        }
        for voice in self.voices.iter_mut().filter(|v| !v.cockpit) {
            let mut mix = match voice.launch_ft {
                Some(feet) => acoustics::launch_mix(voice.position, listener, feet),
                None => acoustics::mix(voice.kind, voice.position, listener),
            };
            if voice.other {
                mix.gain *= super::other::LEVEL;
            }
            if mix != voice.mix {
                voice.mix = mix;
                voice.filter_rate = 0.;
            }
            voice.target_gain = stereo_gain(mix, self.stereo);
        }
        for event in emissions.iter().chain(&passes) {
            let name = match event.kind {
                Kind::Blast(kind) => {
                    match tore_sim::combat::blast::recording(kind, event.position) {
                        Some(name) => name,
                        None => continue,
                    }
                }
                Kind::Impact => "&EXPL3.5K",
                Kind::Explosion => "&EXPL12.5K",
                Kind::AircraftPass => "&AIRPASS.11K",
                Kind::MissilePass => "&MPASS.5K",
                Kind::SonicBoom => "&SNCBOOM.11K",
                Kind::Chaff => "&CHAFF.5K",
                Kind::Flare => "&FLARE.5K",
            };
            if let Some(clip) = clips.get(name) {
                let cockpit = event.own && !listener.external;
                if event.arrived || cockpit {
                    self.play(
                        Arrival {
                            payload: clip.clone(),
                            mix: if cockpit {
                                acoustics::cockpit(event.kind)
                            } else {
                                acoustics::mix(event.kind, event.position, listener)
                            },
                            kind: event.kind,
                            position: event.position,
                        },
                        cockpit,
                    );
                } else {
                    self.field
                        .emit(clip.clone(), event.kind, event.position, listener);
                }
            }
        }
        for arrival in self.field.step(listener) {
            self.play(arrival, false);
        }
    }
    /// A weapon launch heard from outside the aircraft: it starts at once (the
    /// original asks for the sound as the weapon leaves), full within 100 ft
    /// of `position` and silent at the record's `max_sound_distance` feet.
    pub fn weapon(
        &mut self,
        clip: Arc<Clip>,
        max_sound_distance: i16,
        position: [f64; 3],
        listener: Listener,
    ) {
        let mix = acoustics::launch_mix(position, listener, max_sound_distance);
        self.play_with(
            Arrival {
                payload: clip,
                mix,
                kind: Kind::Impact,
                position,
            },
            false,
            Some(max_sound_distance),
        );
    }
    /// The same bounded burst selector for live flight, clients and replays.
    pub fn other_weapons(
        &mut self,
        clips: &BTreeMap<String, Arc<Clip>>,
        listener: Listener,
        launches: &[super::Launch<'_>],
    ) {
        for report in self.other_fire.step(launches, listener) {
            let launch = report.launch();
            let Some(clip) = clips.get(launch.name) else {
                continue;
            };
            self.voices.retain(|v| !v.voice.finished());
            if launch.gun
                && self.voices.iter().filter(|v| v.gun).count() >= super::other::GUN_VOICES
            {
                let quietest = self
                    .voices
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.gun)
                    .min_by(|(_, a), (_, b)| a.mix.gain.total_cmp(&b.mix.gain))
                    .map(|(i, _)| i)
                    .unwrap();
                self.voices.remove(quietest);
            }
            let mix = super::other::mix(launch, listener);
            let added = self.play_with(
                Arrival {
                    payload: clip.clone(),
                    mix,
                    kind: Kind::Impact,
                    position: launch.shooter.unwrap().position,
                },
                false,
                Some(super::other::range(launch)),
            );
            if added && let Some(voice) = self.voices.last_mut() {
                voice.other = true;
                voice.gun = launch.gun;
            }
        }
    }
    fn play(&mut self, arrival: Arrival<Arc<Clip>>, cockpit: bool) {
        self.play_with(arrival, cockpit, None);
    }
    fn play_with(
        &mut self,
        arrival: Arrival<Arc<Clip>>,
        cockpit: bool,
        launch_ft: Option<i16>,
    ) -> bool {
        self.voices.retain(|v| !v.voice.finished());
        if arrival.mix.gain <= 0. {
            return false;
        }
        if self.voices.len() == 16 {
            let (index, quietest) = self
                .voices
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.mix.gain.total_cmp(&b.mix.gain))
                .unwrap();
            if quietest.mix.gain >= arrival.mix.gain {
                return false;
            }
            self.voices.remove(index);
        }
        self.voices.push(SpatialVoice {
            voice: Voice {
                clip: arrival.payload,
                position: 0.,
            },
            mix: arrival.mix,
            kind: arrival.kind,
            launch_ft,
            other: false,
            gun: false,
            position: arrival.position,
            cockpit,
            filtered: 0.,
            stereo_gain: stereo_gain(arrival.mix, self.stereo),
            target_gain: stereo_gain(arrival.mix, self.stereo),
            filter_rate: 0.,
            filter_alpha: 0.,
            gain_smoothing: 0.,
        });
        true
    }
    #[cfg(test)]
    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }
    pub fn sample(&mut self, rate: f64) -> [f32; 2] {
        let mut out = [0.; 2];
        for v in &mut self.voices {
            if v.voice.finished() {
                continue;
            }
            let time = v.voice.position / v.voice.clip.rate;
            let remaining =
                (v.voice.clip.samples.len() as f64 - v.voice.position) / v.voice.clip.rate;
            let remaining = if v.gun {
                remaining.min(super::other::GUN_BURST_TICKS as f64 / 120. - time)
            } else {
                remaining
            };
            if remaining <= 0. {
                v.voice.position = v.voice.clip.samples.len() as f64;
                continue;
            }
            let envelope = (time / 0.005).min(1.) * (remaining / 0.03).min(1.);
            let sample = v.voice.next(rate, false);
            if v.filter_rate != rate {
                v.filter_rate = rate;
                v.filter_alpha =
                    1. - (-std::f64::consts::TAU * f64::from(v.mix.cutoff) / rate).exp() as f32;
                v.gain_smoothing = 1. - (-1. / (0.02 * rate)).exp() as f32;
            }
            for (gain, target) in v.stereo_gain.iter_mut().zip(v.target_gain) {
                *gain += v.gain_smoothing * (target - *gain);
            }
            v.filtered += v.filter_alpha * (sample - v.filtered);
            let value = v.filtered * envelope as f32;
            out[0] += value * v.stereo_gain[0];
            out[1] += value * v.stereo_gain[1];
        }
        for v in self.loops.values_mut() {
            let sample = v.voice.next(rate / v.placed.pitch(), true);
            let [left, right] = v.placed.apply(sample, rate);
            out[0] += left;
            out[1] += right;
        }
        out
    }
}
fn stereo_gain(mix: Mix, stereo: Stereo) -> [f32; 2] {
    let pan = stereo.pan(mix.pan);
    [
        mix.gain * ((1. - pan) * 0.5).sqrt(),
        mix.gain * ((1. + pan) * 0.5).sqrt(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    const COCKPIT: f32 = acoustics::COCKPIT_GAIN;
    fn listener() -> Listener {
        Listener {
            position: [0.; 3],
            right: [1., 0., 0.],
            view: 0,
            external: false,
            own: Some(0),
        }
    }
    fn clips() -> BTreeMap<String, Arc<Clip>> {
        BTreeMap::from([(
            "&EXPL12.5K".into(),
            Arc::new(Clip {
                samples: vec![192; 8000],
                rate: 8000.,
            }),
        )])
    }
    #[test]
    fn muting_discards_pending_weapon_reports() {
        let clips = clips();
        let mut scene = Scene::default();
        let shot = super::super::Launch::by(1, [3000., 0., 0.], "&EXPL12.5K", 6000);
        scene.other_weapons(&clips, listener(), &[shot]);
        assert_eq!(scene.voice_count(), 0);
        scene.tick(&clips, listener(), &[], &[], false);
        for _ in 0..400 {
            scene.other_weapons(&clips, listener(), &[]);
        }
        assert_eq!(scene.voice_count(), 0);
    }

    #[test]
    fn other_guns_have_four_voices_and_a_quarter_second_envelope() {
        let clips = BTreeMap::from([(
            "gun".into(),
            Arc::new(Clip {
                samples: vec![192; 8000],
                rate: 8000.,
            }),
        )]);
        let guns: Vec<_> = (1..10)
            .map(|id| super::super::Launch::by(id, [100., 0., 0.], "gun", 6000).with_gun(true))
            .collect();
        let mut scene = Scene::default();
        scene.other_weapons(&clips, listener(), &guns);
        for _ in 0..11 {
            scene.other_weapons(&clips, listener(), &[]);
        }
        assert_eq!(scene.voice_count(), 4);
        let mut energy = 0.;
        for _ in 0..2000 {
            energy += scene.sample(8000.).iter().map(|v| v.abs()).sum::<f32>();
        }
        assert!(energy > 1.);
        assert_eq!(scene.sample(8000.), [0.; 2]);
        scene.clear();
        scene.other_weapons(&clips, listener(), &guns);
        for _ in 0..11 {
            scene.other_weapons(&clips, listener(), &[]);
        }
        assert_eq!(scene.voice_count(), 4, "reset releases the burst slots");
    }

    #[test]
    fn supersonic_pass_reaches_the_mixer_for_flyby_and_other_aircraft() {
        let mut clips = clips();
        clips.insert("&SNCBOOM.11K".into(), clips["&EXPL12.5K"].clone());
        for own in [true, false] {
            let mut scene = Scene::default();
            let mut l = listener();
            l.external = own;
            l.view = if own { 10 } else { 0 };
            let mut source = Source {
                id: acoustics::SourceId::Aircraft(if own { 0 } else { 9 }),
                position: [-4000., 0., 300.],
                velocity: [2230., 0., 0.],
            };
            let mut audible = false;
            for _ in 0..600 {
                scene.tick(&clips, l, &[source], &[], true);
                for _ in 0..67 {
                    audible |= scene.sample(8000.).iter().any(|v| v.abs() > 0.001);
                }
                source.position[0] += 2230. / 120.;
            }
            assert!(audible, "own={own}: the boom never reached PCM output");
        }
    }

    #[test]
    fn fires_loop_nearby_through_the_canopy_and_fade_out_when_gone() {
        let clips = BTreeMap::from([(
            "&FIRE.5K".to_string(),
            Arc::new(Clip {
                samples: vec![192; 8000],
                rate: 8000.,
            }),
        )]);
        let fire = |x: f64| super::super::LoopSource {
            key: (0, 1),
            clip: "&FIRE.5K".into(),
            position: [x, 0., 0.],
            velocity: [0.; 3],
            gain: 0.3,
            reference: 100.,
            maximum: 2000.,
            pitch: 1.,
        };
        let level = |external, x| {
            let mut scene = Scene::default();
            let l = Listener {
                external,
                ..listener()
            };
            scene.loops(&clips, l, [0.; 3], &[fire(x)], 1., true);
            let mut out = [0.; 2];
            for _ in 0..4000 {
                out = scene.sample(8000.);
            }
            (scene, out[1])
        };
        let (_, near) = level(true, 50.);
        let (_, inside) = level(false, 50.);
        let (_, far) = level(true, 1000.);
        let (beyond, silent) = level(true, 2500.);
        assert!(near > 0.05, "{near}");
        assert!((inside / near - COCKPIT).abs() < 0.02, "{inside} {near}");
        assert!(far > 0. && far < near / 5.);
        assert_eq!(silent, 0.);
        assert!(beyond.loops.is_empty());
        // A fire that goes out fades, then its loop is dropped.
        let (mut scene, _) = level(true, 50.);
        scene.loops(&clips, listener(), [0.; 3], &[], 1., true);
        for _ in 0..8000 {
            scene.sample(8000.);
        }
        scene.loops(&clips, listener(), [0.; 3], &[], 1., true);
        assert!(scene.loops.is_empty());
    }

    /// The Engine level scales other aircraft's engines (kinds 1 and 2) and
    /// not fires, as it did when the mixer scaled a copy of the sources.
    #[test]
    fn the_engine_level_scales_engines_and_not_fires() {
        let clips = BTreeMap::from([(
            "LOOP".to_string(),
            Arc::new(Clip {
                samples: (0..400).map(|i| (128 + (i % 40) * 3) as u8).collect(),
                rate: 8000.,
            }),
        )]);
        let source = |kind: u8, x: f64| super::super::LoopSource {
            key: (kind, u64::from(kind) * 10),
            clip: "LOOP".into(),
            position: [x, 0., 100.],
            velocity: [10., 0., 0.],
            gain: 0.6,
            reference: 100.,
            maximum: 4000.,
            pitch: 1.,
        };
        let sources = [
            source(0, 300.),
            source(1, 700.),
            source(2, 1_100.),
            source(3, 1_500.),
        ];
        let engine = 0.37f32;
        let scaled: Vec<_> = sources
            .iter()
            .map(|s| super::super::LoopSource {
                gain: if matches!(s.key.0, 1 | 2) {
                    s.gain * engine
                } else {
                    s.gain
                },
                ..s.clone()
            })
            .collect();
        let (mut by_level, mut by_copy) = (Scene::default(), Scene::default());
        let l = Listener {
            external: true,
            ..listener()
        };
        by_level.loops(&clips, l, [0.; 3], &sources, engine, true);
        by_copy.loops(&clips, l, [0.; 3], &scaled, 1., true);
        for _ in 0..3000 {
            assert_eq!(by_level.sample(8000.), by_copy.sample(8000.));
        }
    }

    #[test]
    fn arrivals_are_stereo_faded_and_mute_drops_pending_and_active() {
        let clips = clips();
        let mut scene = Scene::default();
        let l = listener();
        scene.tick(
            &clips,
            l,
            &[],
            &[Emission {
                kind: Kind::Explosion,
                position: [1115., 0., 0.],
                arrived: false,
                own: false,
            }],
            true,
        );
        assert_eq!(scene.sample(8000.), [0.; 2]);
        for _ in 0..120 {
            scene.tick(&clips, l, &[], &[], true);
        }
        assert_eq!(scene.voices.len(), 1);
        assert_eq!(scene.sample(8000.), [0.; 2]); // attack starts at silence
        let mut out = [0.; 2];
        for _ in 0..80 {
            out = scene.sample(8000.);
        }
        assert_eq!(out[0], 0.);
        assert!(out[1] > 0.);
        scene.tick(&clips, l, &[], &[], false);
        assert_eq!(scene.sample(8000.), [0.; 2]);
        scene.tick(
            &clips,
            l,
            &[],
            &[Emission {
                kind: Kind::Explosion,
                position: [11150., 0., 0.],
                arrived: false,
                own: false,
            }],
            true,
        );
        scene.clear();
        for _ in 0..1201 {
            scene.tick(&clips, l, &[], &[], true);
        }
        assert!(scene.voices.is_empty());
    }
    #[test]
    fn moving_listener_fades_and_repans_an_already_playing_explosion() {
        let clips = clips();
        let mut scene = Scene::default();
        let mut l = listener();
        scene.play(
            Arrival {
                payload: clips["&EXPL12.5K"].clone(),
                kind: Kind::Explosion,
                position: [400., 0., 0.],
                mix: acoustics::mix(Kind::Explosion, [400., 0., 0.], l),
            },
            false,
        );
        for _ in 0..800 {
            scene.sample(8000.);
        }
        let near = scene.sample(8000.);
        assert_eq!(near[0], 0.);
        l.position = [2000., 0., 0.];
        scene.tick(&clips, l, &[], &[], true);
        for _ in 0..1600 {
            scene.sample(8000.);
        }
        let far = scene.sample(8000.);
        assert!(far[0] > 0. && far[0] < near[1] * 0.3);
        assert!(far[1].abs() < 0.0001);
    }
    #[test]
    fn simultaneous_positions_remain_distinct_and_loud_voice_wins_overload() {
        let clips = clips();
        let mut scene = Scene::default();
        for i in 0..20 {
            scene.play(
                Arrival {
                    payload: clips["&EXPL12.5K"].clone(),
                    kind: Kind::Explosion,
                    position: [0.; 3],
                    mix: Mix {
                        gain: i as f32 / 20.,
                        pan: 0.,
                        cutoff: 1000.,
                    },
                },
                false,
            );
        }
        assert_eq!(scene.voices.len(), 16);
        assert!(scene.voices.iter().all(|v| v.mix.gain >= 0.2));
        scene.clear();
        for x in [-10., 10.] {
            scene.play(
                Arrival {
                    payload: clips["&EXPL12.5K"].clone(),
                    kind: Kind::Explosion,
                    position: [x, 0., 0.],
                    mix: acoustics::mix(Kind::Explosion, [x, 0., 0.], listener()),
                },
                false,
            );
        }
        assert_eq!(scene.voices.len(), 2);
        assert_ne!(scene.voices[0].mix.pan, scene.voices[1].mix.pan);
    }
    #[test]
    fn a_launch_seen_from_outside_starts_at_once_and_fades_to_its_records_distance() {
        let clip = Arc::new(Clip {
            samples: vec![255; 8000],
            rate: 8000.,
        });
        let mut outside = listener();
        outside.external = true;
        let mut scene = Scene::default();
        // Fired 150 ft from the camera: at once (no travel time), nearly
        // the full level of the original's 255.
        scene.weapon(clip.clone(), 7000, [150., 0., 0.], outside);
        assert_eq!(scene.voices.len(), 1);
        let near = scene.voices[0].mix.gain;
        assert!(
            near > 0.39 && near <= acoustics::FULL_LEVEL as f32,
            "{near}"
        );
        assert_eq!(scene.voices[0].mix.pan, 1.);
        // As the camera falls behind, the same voice fades in a straight line
        // and is silent at the record's 7,000 ft.
        let mut far = outside;
        far.position = [-3_450., 0., 0.];
        scene.tick(&BTreeMap::new(), far, &[], &[], true);
        let half = scene.voices[0].mix.gain;
        assert!(
            (half - acoustics::FULL_LEVEL as f32 / 2.).abs() < 0.01,
            "{half}"
        );
        far.position = [-6_900., 0., 0.];
        scene.tick(&BTreeMap::new(), far, &[], &[], true);
        assert_eq!(scene.voices[0].mix.gain, 0.);
        // Released past the record's distance it is not heard at all.
        let mut scene = Scene::default();
        scene.weapon(clip, 6000, [6_500., 0., 0.], outside);
        assert!(scene.voices.is_empty());
    }
    #[test]
    fn own_release_is_centered_in_cockpit_travels_outside_and_far_releases_are_silent() {
        let clips = BTreeMap::from(["&CHAFF.5K", "&FLARE.5K"].map(|name| {
            (
                name.to_string(),
                Arc::new(Clip {
                    samples: vec![192; 8000],
                    rate: 8000.,
                }),
            )
        }));
        let release = |kind, position, own| Emission {
            kind,
            position,
            arrived: false,
            own,
        };
        // In the player's cockpit: at once, centered, at the cue's peak level,
        // and never re-mixed as the aircraft flies on.
        let mut scene = Scene::default();
        let mut l = listener();
        scene.tick(
            &clips,
            l,
            &[],
            &[release(Kind::Chaff, [30., 0., 0.], true)],
            true,
        );
        assert_eq!(scene.voices.len(), 1);
        assert!(Arc::ptr_eq(
            &scene.voices[0].voice.clip,
            &clips["&CHAFF.5K"]
        ));
        assert_eq!(scene.voices[0].mix, acoustics::cockpit(Kind::Chaff));
        l.position = [3000., 0., 0.];
        scene.tick(&clips, l, &[], &[], true);
        assert_eq!(scene.voices[0].mix, acoustics::cockpit(Kind::Chaff));
        let mut out = [0.; 2];
        for _ in 0..80 {
            out = scene.sample(8000.);
        }
        assert!(out[0] > 0.);
        assert_eq!(out[0], out[1]);
        // Outside, the player's release travels from where it was released.
        let mut scene = Scene::default();
        let mut l = listener();
        l.external = true;
        scene.tick(
            &clips,
            l,
            &[],
            &[release(Kind::Flare, [1115., 0., 0.], true)],
            true,
        );
        assert!(scene.voices.is_empty());
        for _ in 0..120 {
            scene.tick(&clips, l, &[], &[], true);
        }
        assert_eq!(scene.voices.len(), 1);
        assert!(Arc::ptr_eq(
            &scene.voices[0].voice.clip,
            &clips["&FLARE.5K"]
        ));
        assert_eq!(scene.voices[0].mix.pan, 1.);
        // Another aircraft's release 4,000 ft away is never heard.
        let mut scene = Scene::default();
        let l = listener();
        scene.tick(
            &clips,
            l,
            &[],
            &[release(Kind::Chaff, [4000., 0., 0.], false)],
            true,
        );
        for _ in 0..600 {
            scene.tick(&clips, l, &[], &[], true);
        }
        assert!(scene.voices.is_empty());
    }
}
