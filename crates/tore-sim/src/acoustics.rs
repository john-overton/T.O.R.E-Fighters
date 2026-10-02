//! Renderer/device-independent, fitted acoustics. Rules and units: docs/audio.md.
use std::collections::BTreeMap;

pub type Vector = [f64; 3];
const DT: f64 = 1. / 120.;
const MAX_WAVES: usize = 256;

/// The original's level 255 on the mixer's scale.
pub const FULL_LEVEL: f64 = 0.4;
/// The original refuses any sound request farther away than this.
pub const REFUSED_BEYOND_FT: f64 = 20000.;
/// Heard from inside a cockpit, a sound from outside the aircraft plays at
/// this share of its level with its treble above this cutoff removed
/// (opinionated, John 2026-09-28).
pub const COCKPIT_GAIN: f32 = 0.4;
pub const COCKPIT_CUTOFF_HZ: f32 = 2000.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Recordings made before explosion types: a generic hit.
    Impact,
    /// Recordings made before explosion types: a generic kill.
    Explosion,
    /// One original explosion type (docs/spec/explosions.md).
    Blast(u8),
    AircraftPass,
    MissilePass,
    SonicBoom,
    Chaff,
    Flare,
}
impl Kind {
    /// The name a recording stores for this sound.
    pub fn name(self) -> String {
        match self {
            Self::Blast(kind) => format!("blast{kind}"),
            other => format!("{other:?}").to_lowercase(),
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        if let Some(kind) = name.strip_prefix("blast") {
            return Some(Self::Blast(kind.parse().ok()?));
        }
        [
            Self::Impact,
            Self::Explosion,
            Self::AircraftPass,
            Self::MissilePass,
            Self::SonicBoom,
            Self::Chaff,
            Self::Flare,
        ]
        .into_iter()
        .find(|kind| kind.name() == name)
    }
    /// Reference distance, maximum distance and peak gain. An explosion
    /// type's pair is its full-level and silent distances.
    pub fn parameters(self) -> (f64, f64, f64) {
        match self {
            Self::Blast(kind) => crate::combat::blast::explosion(kind)
                .map_or((1000., REFUSED_BEYOND_FT, FULL_LEVEL), |e| {
                    (f64::from(e.full_ft), f64::from(e.silent_ft), FULL_LEVEL)
                }),
            Self::Impact => (80., 8000., 0.4),
            Self::Explosion => (400., 40000., 0.65),
            Self::AircraftPass => (200., 2000., 0.5),
            Self::MissilePass => (80., 1500., 0.5),
            Self::SonicBoom => (500., 13000., 0.8),
            // The original's full-level and silent distances, at its level 200
            // against a weapon release's 255 (docs/spec/countermeasures.md).
            Self::Chaff | Self::Flare => (100., 4000., 0.4 * 200. / 255.),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Listener {
    pub position: Vector,
    pub right: Vector,
    /// Main camera mode. A change is a camera cut, not listener motion.
    pub view: u8,
    pub external: bool,
    /// The aircraft the listener flies, whose source among the ones given to
    /// [`Passes::step`] is its own: its sonic boom is heard from an outside
    /// view only. External fly-by cameras can be passed by it. `None` for a listener that flies
    /// nothing.
    pub own: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    pub gain: f32,
    /// -1 is left, +1 is right; constant-power stereo.
    pub pan: f32,
    pub cutoff: f32,
}

pub fn speed_of_sound(altitude: f64) -> f64 {
    f64::from(tore_formats::flight_model::sound_speed(
        (altitude.clamp(0., 36000.) * 256.) as i32,
    ))
}

pub fn mix(kind: Kind, position: Vector, listener: Listener) -> Mix {
    let relative = sub(position, listener.position);
    let distance = length(relative);
    let (reference, maximum, peak) = kind.parameters();
    let gain = if let Kind::Blast(_) = kind {
        // The original's straight-line fade from full to silent.
        if distance > REFUSED_BEYOND_FT {
            0.
        } else {
            peak * ((maximum - distance) / (maximum - reference)).clamp(0., 1.)
        }
    } else {
        let fade = ((maximum - distance) / (0.2 * maximum)).clamp(0., 1.);
        peak * (reference / distance.max(reference)) * fade * fade * (3. - 2. * fade)
    };
    outside(
        Mix {
            gain: gain as f32,
            pan: (dot(relative, listener.right) / distance.max(1.)).clamp(-1., 1.) as f32,
            cutoff: (12000. / (1. + distance / 6000.)).clamp(250., 12000.) as f32,
        },
        listener,
    )
}

/// A sound from outside the aircraft as the listener hears it: through
/// the canopy from a cockpit, unchanged in an external view.
pub fn outside(mix: Mix, listener: Listener) -> Mix {
    if listener.external {
        mix
    } else {
        Mix {
            gain: mix.gain * COCKPIT_GAIN,
            cutoff: mix.cutoff.min(COCKPIT_CUTOFF_HZ),
            ..mix
        }
    }
}

/// A looping source such as an engine or a fire: inverse distance beyond
/// `reference`, fading to nothing at `maximum`, with the same stereo and
/// treble rules as one-shot sounds.
pub fn loop_mix(
    position: Vector,
    listener: Listener,
    reference: f64,
    maximum: f64,
    peak: f64,
) -> Mix {
    let relative = sub(position, listener.position);
    let distance = length(relative);
    let fade = ((maximum - distance) / (0.2 * maximum)).clamp(0., 1.);
    outside(
        Mix {
            gain: (peak * (reference / distance.max(reference)) * fade * fade * (3. - 2. * fade))
                as f32,
            pan: (dot(relative, listener.right) / distance.max(1.)).clamp(-1., 1.) as f32,
            cutoff: (12000. / (1. + distance / 6000.)).clamp(250., 12000.) as f32,
        },
        listener,
    )
}

/// The Doppler pitch ratio of a source moving at `velocity` heard by a
/// listener moving at `listener_velocity`, at the local speed of sound,
/// held within `limits` (lowest, highest).
pub fn doppler(
    position: Vector,
    velocity: Vector,
    listener: Vector,
    listener_velocity: Vector,
    limits: (f64, f64),
) -> f64 {
    let toward = sub(listener, position);
    let distance = length(toward);
    if distance < 1. {
        return 1.;
    }
    let axis = scale(toward, 1. / distance);
    let c = speed_of_sound((position[1] + listener[1]) * 0.5);
    let source = dot(velocity, axis).min(0.95 * c);
    let observer = dot(listener_velocity, axis);
    ((c - observer) / (c - source)).clamp(limits.0, limits.1)
}

/// A release heard inside the releasing aircraft's own cockpit: the cue's
/// peak level, centered and unfiltered, with no distance or travel.
pub fn cockpit(kind: Kind) -> Mix {
    Mix {
        gain: kind.parameters().2 as f32,
        pan: 0.,
        cutoff: 12000.,
    }
}

struct Wave<T> {
    payload: T,
    kind: Kind,
    position: Vector,
    radius: f64,
    speed: f64,
}

pub struct Arrival<T> {
    pub payload: T,
    pub mix: Mix,
    pub kind: Kind,
    pub position: Vector,
}

/// Time advances only when the caller advances one simulation tick.
pub struct Field<T> {
    waves: Vec<Wave<T>>,
}
impl<T> Default for Field<T> {
    fn default() -> Self {
        Self { waves: Vec::new() }
    }
}
impl<T> Field<T> {
    pub fn clear(&mut self) {
        self.waves.clear();
    }
    pub fn emit(&mut self, payload: T, kind: Kind, position: Vector, listener: Listener) {
        if !position.iter().all(|v| v.is_finite()) {
            return;
        }
        if self.waves.len() == MAX_WAVES {
            self.waves.remove(0);
        }
        self.waves.push(Wave {
            payload,
            kind,
            position,
            radius: 0.,
            speed: speed_of_sound((position[1] + listener.position[1]) * 0.5),
        });
    }
    pub fn step(&mut self, listener: Listener) -> Vec<Arrival<T>> {
        let mut arrivals = Vec::new();
        // Stable order matters when the bounded device mixer must choose voices.
        let mut index = 0;
        while index < self.waves.len() {
            let wave = &mut self.waves[index];
            wave.radius += wave.speed * DT;
            let distance = length(sub(wave.position, listener.position));
            if distance <= wave.radius {
                let wave = self.waves.remove(index);
                let mix = mix(wave.kind, wave.position, listener);
                if mix.gain > 0. {
                    arrivals.push(Arrival {
                        payload: wave.payload,
                        mix,
                        kind: wave.kind,
                        position: wave.position,
                    });
                }
            } else if wave.radius > wave.kind.parameters().1 {
                self.waves.remove(index);
            } else {
                index += 1;
            }
        }
        arrivals
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceId {
    Aircraft(u32),
    Missile(u32),
}
#[derive(Clone, Copy, Debug)]
pub struct Source {
    pub id: SourceId,
    pub position: Vector,
    pub velocity: Vector,
}
#[derive(Clone, Copy, Debug)]
pub struct Emission {
    pub kind: Kind,
    pub position: Vector,
    /// A Mach cone crossing is already the shock's arrival at the observer.
    pub arrived: bool,
    /// Released by the player's own aircraft, so its cockpit hears it as
    /// [`cockpit`] rather than through the air.
    pub own: bool,
}
struct Track {
    relative: Vector,
    position: Vector,
    cone: Option<f64>,
    pass_armed: bool,
    boom_armed: bool,
}
#[derive(Default)]
pub struct Passes {
    tracks: BTreeMap<SourceId, Track>,
    view: Option<u8>,
    own_armed: Option<bool>,
}
impl Passes {
    pub fn step(&mut self, listener: Listener, sources: &[Source]) -> Vec<Emission> {
        if self.view != Some(listener.view) {
            self.tracks.clear();
            self.view = Some(listener.view);
        }
        let mut events = Vec::new();
        self.tracks
            .retain(|id, _| sources.iter().any(|s| s.id == *id));
        for source in sources {
            let speed = length(source.velocity);
            let mach = speed / speed_of_sound(source.position[1]);
            if matches!(source.id, SourceId::Aircraft(id) if listener.own == Some(id)) {
                if let Some(armed) = self.own_armed {
                    if armed && mach >= 1. {
                        if listener.external {
                            events.push(Emission {
                                kind: Kind::SonicBoom,
                                position: source.position,
                                arrived: false,
                                own: false,
                            });
                        }
                        self.own_armed = Some(false);
                    } else if mach < 0.98 {
                        self.own_armed = Some(true);
                    }
                } else {
                    self.own_armed = Some(mach < 1.);
                }
                // Outside cameras can be passed by their own aircraft too.
                // The cockpit still excludes its own pass and trailing shock.
                if !listener.external {
                    continue;
                }
            }
            let kind = match source.id {
                SourceId::Aircraft(_) => Kind::AircraftPass,
                SourceId::Missile(_) => Kind::MissilePass,
            };
            let maximum = kind.parameters().1;
            let relative = sub(source.position, listener.position);
            let distance = length(relative);
            let cone = if matches!(source.id, SourceId::Aircraft(_)) && mach >= 1. {
                let along = dot(relative, source.velocity) / speed;
                let lateral = (dot(relative, relative) - along * along).max(0.).sqrt();
                Some(along - lateral * (mach * mach - 1.).sqrt())
            } else {
                None
            };
            if let Some(track) = self.tracks.get_mut(&source.id) {
                if distance > maximum * 1.5 {
                    track.pass_armed = true;
                }
                let movement = sub(relative, track.relative);
                let moved2 = dot(movement, movement);
                let minimum_speed = if kind == Kind::AircraftPass {
                    100.
                } else {
                    200.
                };
                if track.pass_armed && moved2 > (minimum_speed * DT).powi(2) {
                    let fraction = -dot(track.relative, movement) / moved2;
                    if (0. ..1.).contains(&fraction) {
                        let closest = add(track.relative, scale(movement, fraction));
                        if length(closest) <= maximum {
                            track.pass_armed = false;
                            if kind != Kind::AircraftPass || mach < 1. {
                                events.push(Emission {
                                    kind,
                                    position: add(
                                        track.position,
                                        scale(sub(source.position, track.position), fraction),
                                    ),
                                    arrived: false,
                                    own: false,
                                });
                            }
                        }
                    }
                }
                if let (Some(before), Some(now)) = (track.cone, cone) {
                    if now < -100. {
                        track.boom_armed = true;
                    }
                    if track.boom_armed
                        && before < 0.
                        && now >= 0.
                        && distance < Kind::SonicBoom.parameters().1
                    {
                        events.push(Emission {
                            kind: Kind::SonicBoom,
                            position: source.position,
                            arrived: true,
                            own: false,
                        });
                        track.boom_armed = false;
                    }
                }
                track.relative = relative;
                track.position = source.position;
                track.cone = cone;
            } else {
                self.tracks.insert(
                    source.id,
                    Track {
                        relative,
                        position: source.position,
                        cone,
                        pass_armed: true,
                        boom_armed: cone.is_none_or(|v| v < 0.),
                    },
                );
            }
        }
        events
    }
}
fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}
fn add(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] + b[i])
}
fn scale(a: Vector, b: f64) -> Vector {
    a.map(|v| v * b)
}
fn dot(a: Vector, b: Vector) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn length(a: Vector) -> f64 {
    dot(a, a).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn listener() -> Listener {
        Listener {
            position: [0.; 3],
            right: [1., 0., 0.],
            view: 0,
            external: false,
            own: Some(0),
        }
    }
    #[test]
    fn ten_second_explosion_survives_visual_lifetime_and_arrives_once() {
        let l = listener();
        let mut field = Field::default();
        field.emit(7, Kind::Explosion, [11150., 0., 0.], l);
        for _ in 0..1199 {
            assert!(field.step(l).is_empty());
        }
        let mut result = field.step(l);
        // Floating-point accumulation can place reception one tick later.
        result.extend(field.step(l));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].payload, 7);
        assert_eq!(result[0].mix.pan, 1.);
        assert!(field.step(l).is_empty());
    }
    #[test]
    fn moving_listener_intercepts_wave_and_queue_is_bounded_and_resettable() {
        let mut l = listener();
        let mut field = Field::default();
        field.emit(1, Kind::Explosion, [11150., 0., 0.], l);
        let mut received = 0;
        for tick in 1..=601 {
            l.position[0] = tick as f64 * 1115. / 120.;
            received += field.step(l).len();
        }
        assert_eq!(received, 1);
        for i in 0..1000 {
            field.emit(i, Kind::Explosion, [30000., 0., 0.], l);
        }
        assert_eq!(field.waves.len(), 256);
        field.clear();
        assert!(field.step(l).is_empty());
    }
    #[test]
    fn distance_pressure_fade_and_atmospheric_speed() {
        let l = Listener {
            external: true,
            ..listener()
        };
        assert_eq!(speed_of_sound(0.), 1115.);
        assert_eq!(speed_of_sound(50000.), 967.);
        let a = mix(Kind::Explosion, [1000., 0., 0.], l);
        let b = mix(Kind::Explosion, [8000., 0., 0.], l);
        assert!((a.gain / b.gain - 8.).abs() < 1e-5);
        assert!(b.cutoff < a.cutoff);
        assert_eq!(mix(Kind::Explosion, [40000., 0., 0.], l).gain, 0.);
        assert_eq!(mix(Kind::Explosion, [-1000., 0., 0.], l).pan, -1.);
    }
    #[test]
    fn releases_are_full_within_100_ft_silent_at_4000_ft_and_centered_in_own_cockpit() {
        let l = Listener {
            external: true,
            ..listener()
        };
        // Level 200 of the original's 255, relative to a weapon release.
        let peak = 0.4 * 200. / 255.;
        for kind in [Kind::Chaff, Kind::Flare] {
            assert_eq!(kind.parameters(), (100., 4000., peak));
            assert_eq!(mix(kind, [100., 0., 0.], l).gain, peak as f32);
            assert!(mix(kind, [101., 0., 0.], l).gain < mix(kind, [100., 0., 0.], l).gain);
            assert!(mix(kind, [3999., 0., 0.], l).gain > 0.);
            assert_eq!(mix(kind, [4000., 0., 0.], l).gain, 0.);
            let own = cockpit(kind);
            assert_eq!((own.gain, own.pan, own.cutoff), (peak as f32, 0., 12000.));
        }
        assert!(peak < Kind::Impact.parameters().2);
    }
    #[test]
    fn explosion_types_fade_in_a_straight_line_and_are_refused_past_20000_ft() {
        let l = Listener {
            external: true,
            ..listener()
        };
        // Type 30: full within 2,000 ft, silent at 25,000.
        let at = |x: f64| mix(Kind::Blast(30), [x, 0., 0.], l).gain;
        assert_eq!(at(10.), FULL_LEVEL as f32);
        assert_eq!(at(2000.), FULL_LEVEL as f32);
        let half = (2000. + 25000.) / 2.;
        assert!((at(half) - FULL_LEVEL as f32 / 2.).abs() < 1e-6);
        assert!(at(19999.) > 0.08);
        assert_eq!(at(20001.), 0.);
        // A gun strike on dirt: full within 100 ft, silent at 10,000.
        let dirt = mix(Kind::Blast(15), [5050., 0., 0.], l).gain;
        assert!((dirt - FULL_LEVEL as f32 / 2.).abs() < 1e-6);
        assert_eq!(Kind::Blast(30).parameters(), (2000., 25000., FULL_LEVEL));
    }
    #[test]
    fn cockpit_hears_outside_sounds_at_40_percent_without_treble() {
        let outside_view = Listener {
            external: true,
            ..listener()
        };
        let cockpit_view = listener();
        for kind in [Kind::Blast(35), Kind::AircraftPass, Kind::Flare] {
            let out = mix(kind, [300., 0., 0.], outside_view);
            let inside = mix(kind, [300., 0., 0.], cockpit_view);
            assert!((inside.gain - out.gain * COCKPIT_GAIN).abs() < 1e-6);
            assert_eq!(inside.cutoff, COCKPIT_CUTOFF_HZ);
            assert_eq!(inside.pan, out.pan);
        }
        // The player's own release is not outside the cockpit.
        assert_eq!(cockpit(Kind::Chaff).gain, Kind::Chaff.parameters().2 as f32);
        let engine = loop_mix([400., 0., 0.], outside_view, 200., 15000., 0.2);
        assert!((engine.gain - 0.1).abs() < 1e-6);
        let muffled = loop_mix([400., 0., 0.], cockpit_view, 200., 15000., 0.2);
        assert!((muffled.gain - 0.04).abs() < 1e-6);
    }
    #[test]
    fn recorded_names_round_trip_and_doppler_rises_toward_and_falls_away() {
        for kind in [Kind::Impact, Kind::Explosion, Kind::Blast(30), Kind::Flare] {
            assert_eq!(Kind::from_name(&kind.name()), Some(kind));
        }
        assert_eq!(Kind::Blast(17).name(), "blast17");
        assert_eq!(Kind::Impact.name(), "impact");
        assert_eq!(Kind::from_name("blast"), None);
        let limits = (0.5, 2.);
        let toward = doppler([-1000., 0., 0.], [500., 0., 0.], [0.; 3], [0.; 3], limits);
        let away = doppler([1000., 0., 0.], [500., 0., 0.], [0.; 3], [0.; 3], limits);
        assert!((toward - 1115. / 615.).abs() < 1e-9);
        assert!((away - 1115. / 1615.).abs() < 1e-9);
        assert_eq!(
            doppler([-1000., 0., 0.], [5000., 0., 0.], [0.; 3], [0.; 3], limits),
            2.
        );
        assert_eq!(
            doppler([0.; 3], [500., 0., 0.], [0.; 3], [0.; 3], limits),
            1.
        );
    }
    #[test]
    fn swept_pass_once_with_no_spawn_formation_or_camera_cut_noise() {
        let mut passes = Passes::default();
        let mut l = listener();
        let mut s = Source {
            id: SourceId::Missile(1),
            position: [-10., 100., 0.],
            velocity: [2400., 0., 0.],
        };
        assert!(passes.step(l, &[s]).is_empty());
        s.position[0] = 10.;
        let events = passes.step(l, &[s]);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, Kind::MissilePass);
        assert_eq!(events[0].position, [0., 100., 0.]);
        for _ in 0..10 {
            assert!(passes.step(l, &[s]).is_empty());
        }
        l.view = 1;
        l.position = [100., 0., 0.];
        assert!(passes.step(l, &[s]).is_empty());
        for _ in 0..10 {
            l.position[0] += 20.;
            s.position[0] += 20.;
            assert!(passes.step(l, &[s]).is_empty());
        }
    }
    #[test]
    fn mach_two_boom_at_cone_arrival_not_closest_approach() {
        let l = listener();
        let mut passes = Passes::default();
        let mut s = Source {
            id: SourceId::Aircraft(1),
            position: [-2000., 1000., 0.],
            velocity: [2230., 0., 0.],
        };
        // At source altitude the existing atmosphere slightly lowers sound speed.
        let boundary = 1000. * ((2230. / speed_of_sound(1000.)).powi(2) - 1.).sqrt();
        assert!(passes.step(l, &[s]).is_empty());
        s.position[0] = 0.;
        assert!(passes.step(l, &[s]).is_empty());
        s.position[0] = boundary - 1.;
        assert!(passes.step(l, &[s]).is_empty());
        s.position[0] = boundary + 1.;
        let e = passes.step(l, &[s]);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].kind, Kind::SonicBoom);
        assert!(e[0].arrived);
        assert!(passes.step(l, &[s]).is_empty());
    }
    #[test]
    fn sonic_passes_include_mach_one_and_the_flyby_aircraft() {
        for id in [0, 17] {
            for mach in [1., 1.01, 2.] {
                let mut passes = Passes::default();
                let mut l = listener();
                l.external = true;
                l.view = 10;
                let speed = speed_of_sound(0.) * mach;
                let mut s = Source {
                    id: SourceId::Aircraft(id),
                    position: [-4000., 0., 300.],
                    velocity: [speed, 0., 0.],
                };
                let mut events = Vec::new();
                for _ in 0..1200 {
                    events.extend(passes.step(l, &[s]));
                    s.position[0] += speed * DT;
                }
                assert_eq!(events.len(), 1, "id={id}, mach={mach}");
                assert_eq!(events[0].kind, Kind::SonicBoom);
                assert!(events[0].arrived);
                // Reselecting F9 ahead of the aircraft gives another actual pass.
                l.view += 1;
                s.position[0] = -4000.;
                assert!(passes.step(l, &[s]).is_empty());
            }
        }
    }

    #[test]
    fn own_boom_external_only_rearms_and_does_not_fire_on_view_switch() {
        let mut p = Passes::default();
        let mut l = listener();
        let mut s = Source {
            id: SourceId::Aircraft(0),
            position: [0.; 3],
            velocity: [1000., 0., 0.],
        };
        assert!(p.step(l, &[s]).is_empty());
        s.velocity[0] = 1200.;
        assert!(p.step(l, &[s]).is_empty());
        l.view = 1;
        l.external = true;
        assert!(p.step(l, &[s]).is_empty());
        s.velocity[0] = 1000.;
        assert!(p.step(l, &[s]).is_empty());
        s.velocity[0] = 1200.;
        assert_eq!(p.step(l, &[s]).len(), 1);
        assert!(p.step(l, &[s]).is_empty());
    }
}
