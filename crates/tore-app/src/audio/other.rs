//! Shared live, network and replay policy for other shooters' weapon reports.
//! Fitted agent choices, documented in docs/spec/sound.md.
use super::{Launch, OTHER_LAUNCH_VOICES, nearest_launches, one_sound_per_release};
use std::collections::BTreeMap;
use tore_sim::acoustics::{self, Listener, Mix};

pub const GUN_BURST_TICKS: u64 = 30;
pub const GUN_VOICES: usize = 4;
pub const LEVEL: f32 = 0.65;

pub fn range(launch: Launch<'_>) -> i16 {
    if launch.gun {
        if launch.max_distance_ft > 0 {
            launch.max_distance_ft.min(6000)
        } else {
            4500
        }
    } else {
        launch.max_distance_ft
    }
}

pub fn mix(launch: Launch<'_>, listener: Listener) -> Mix {
    let mut mix = acoustics::launch_mix(
        launch.shooter.expect("other shooter").position,
        listener,
        range(launch),
    );
    mix.gain *= LEVEL;
    mix
}

/// Advances at 120 Hz, independent of rendering or the audio device.
/// A gap of 30 ticks permits another burst, even if more rounds were fired.
#[derive(Default)]
pub struct Fire {
    tick: u64,
    guns: BTreeMap<(u32, String), u64>,
}
impl Fire {
    pub fn step<'a>(&mut self, launches: &[Launch<'a>], listener: Listener) -> Vec<Launch<'a>> {
        self.tick += 1;
        self.guns.retain(|_, until| *until > self.tick);
        let sounds = one_sound_per_release(launches);
        let missiles: Vec<_> = sounds.iter().copied().filter(|l| !l.gun).collect();
        let mut heard = nearest_launches(&missiles, listener, OTHER_LAUNCH_VOICES);
        let guns: Vec<_> = sounds
            .iter()
            .copied()
            .filter(|l| l.gun)
            .map(|mut l| {
                l.max_distance_ft = range(l);
                l
            })
            .collect();
        for gun in nearest_launches(&guns, listener, usize::MAX) {
            let key = (gun.shooter.unwrap().id, gun.name.to_owned());
            if self.guns.len() < GUN_VOICES && !self.guns.contains_key(&key) {
                self.guns.insert(key, self.tick + GUN_BURST_TICKS);
                heard.push(gun);
            }
        }
        heard
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn listener(external: bool) -> Listener {
        Listener {
            position: [0.; 3],
            right: [1., 0., 0.],
            external,
            view: 0,
            own: Some(0),
        }
    }
    #[test]
    fn distance_canopy_and_other_level() {
        let shot = |distance, gun| Launch::by(1, [distance, 0., 0.], "shot", 6000).with_gun(gun);
        assert_eq!(mix(shot(100., false), listener(true)).gain, LEVEL * 0.4);
        assert!((mix(shot(3050., false), listener(true)).gain - LEVEL * 0.4 / 2.).abs() < 1e-6);
        assert_eq!(mix(shot(6000., true), listener(true)).gain, 0.);
        assert!((mix(shot(100., true), listener(false)).gain - LEVEL * 0.4 * 0.4).abs() < 1e-6);
        assert_eq!(range(Launch::by(1, [0.; 3], "gun", 0).with_gun(true)), 4500);
        assert_eq!(range(shot(100., false)), 6000);
    }
    #[test]
    fn rounds_become_bursts_with_a_persistent_cap_without_starving_missiles() {
        let guns: Vec<_> = (1..10)
            .map(|id| Launch::by(id, [id as f64 * 100., 0., 0.], "gun", 6000).with_gun(true))
            .collect();
        let mut fire = Fire::default();
        let heard = fire.step(&guns, listener(true));
        assert_eq!(
            heard
                .iter()
                .map(|g| g.shooter.unwrap().id)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        let missile = Launch::by(20, [2000., 0., 0.], "missile", 7000);
        for _ in 1..GUN_BURST_TICKS {
            let mut shots = guns.clone();
            shots.push(missile);
            assert_eq!(fire.step(&shots, listener(true)), [missile]);
        }
        assert_eq!(fire.step(&guns, listener(true)).len(), GUN_VOICES);
        assert!(fire.step(&[], listener(true)).is_empty());
    }
}
