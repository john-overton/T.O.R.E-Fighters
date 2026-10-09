//! The coders of the render snapshot and its picture types
//! (docs/formats/checkpoint.md, stage H slice H3b).
//!
//! Combat keeps the last two snapshots. They are presentation, but the
//! checkpoint codes them because a standby that becomes the host draws from
//! them at once, and because an aircraft whose AI stopped flying keeps the
//! devices last drawn for it, which the next picture reads back from the
//! snapshot. Every field of every type is coded; nothing here is skipped.
//!
//! A `Vec` codes each item against the baseline's item at the same index, so
//! when the current snapshot is coded against the previous one (the render
//! history does that) each aircraft is coded against itself one tick earlier
//! and an unchanged value costs one bit.

use super::{
    AircraftPose, Damage, DebrisPose, Draw, EffectPose, Engine, MarkPose, PilotPose,
    ProjectilePose, RenderSnapshot,
};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};
use tore_sim::combat::live::EffectKind;

tore_sim::checkpoint_struct!(RenderSnapshot {
    tick,
    player,
    targets,
    projectiles,
    effects,
    marks,
    debris,
    pilots,
    models,
});

// Which airframe draws a pose: nothing, a model's batch or the player's.
// Written by hand so a new variant fails to compile.
impl Checkpoint for Draw {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::Hidden => s.writer().write_varint(0),
            Self::Model(id) => {
                s.writer().write_varint(1);
                id.save(s, None)?;
            }
            Self::Ownship => s.writer().write_varint(2),
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        match l.reader().read_varint()? {
            0 => Ok(Self::Hidden),
            1 => Ok(Self::Model(Checkpoint::load(l, None)?)),
            2 => Ok(Self::Ownship),
            other => invalid(format!("a draw mode has no variant {other}")),
        }
    }
}

tore_sim::checkpoint_struct!(AircraftPose {
    id,
    aircraft,
    draw,
    position,
    attitude,
    velocity,
    devices,
    engine,
    damage,
    airborne,
    wreck,
    crashed,
});

tore_sim::checkpoint_struct!(Engine {
    lit,
    afterburner,
    rates,
    rotor,
    rotor_turns,
    rotor_tilt,
    flame,
});

tore_sim::checkpoint_struct!(Damage {
    hp,
    initial_hp,
    sections,
    structural,
});

tore_sim::checkpoint_struct!(ProjectilePose {
    id,
    owner,
    weapon,
    shape,
    gun,
    tracer,
    position,
    previous,
    direction,
    target,
    incoming,
    speed_f8,
});

// `EffectKind` belongs to `tore-sim`, so its coder cannot be written here
// (the trait is `tore-sim`'s too): the kind is coded by its number through
// these two, each an exhaustive `match`, so a new kind fails to compile.
fn effect_kind_number(kind: EffectKind) -> u64 {
    match kind {
        EffectKind::Flare => 0,
        EffectKind::Chaff => 1,
        EffectKind::Launch => 2,
        EffectKind::Hit => 3,
        EffectKind::Destroyed => 4,
        EffectKind::Ground => 5,
        EffectKind::DebrisImpact => 6,
    }
}

fn effect_kind(number: u64) -> Result<EffectKind, CheckpointError> {
    Ok(match number {
        0 => EffectKind::Flare,
        1 => EffectKind::Chaff,
        2 => EffectKind::Launch,
        3 => EffectKind::Hit,
        4 => EffectKind::Destroyed,
        5 => EffectKind::Ground,
        6 => EffectKind::DebrisImpact,
        other => return invalid(format!("an effect kind has no variant {other}")),
    })
}

impl Checkpoint for EffectPose {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        let EffectPose {
            kind,
            position,
            ticks,
            blast,
        } = self;
        s.writer().write_varint(effect_kind_number(*kind));
        position.save(s, base.map(|b| &b.position))?;
        ticks.save(s, base.map(|b| &b.ticks))?;
        blast.save(s, base.map(|b| &b.blast))
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let kind = effect_kind(l.reader().read_varint()?)?;
        Ok(EffectPose {
            kind,
            position: Checkpoint::load(l, base.map(|b| &b.position))?,
            ticks: Checkpoint::load(l, base.map(|b| &b.ticks))?,
            blast: Checkpoint::load(l, base.map(|b| &b.blast))?,
        })
    }
}

tore_sim::checkpoint_struct!(MarkPose {
    kind,
    position,
    age,
    strength,
});

tore_sim::checkpoint_struct!(DebrisPose {
    owner,
    draw,
    position,
    attitude,
    variant,
});

tore_sim::checkpoint_struct!(PilotPose {
    owner,
    position,
    heading,
    phase,
    crew,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::DEVICES;
    use tore_formats::aircraft::AircraftId;
    use tore_sim::checkpoint::{Coded, Models, from_bytes, round_trip, to_bytes};
    use tore_sim::combat::blast::MarkKind;
    use tore_sim::combat::live::{DAMAGE_SECTIONS, DamageSection};
    use tore_sim::{ejection, wreck};

    fn pose(id: u32, t: f64) -> AircraftPose {
        AircraftPose {
            id,
            aircraft: (!id.is_multiple_of(3)).then_some(AircraftId::Rafale),
            draw: match id % 3 {
                0 => Draw::Hidden,
                1 => Draw::Model(AircraftId::F14),
                _ => Draw::Ownship,
            },
            position: [1000. + t, 9000. - t * 0.5, -250. * f64::from(id)],
            attitude: [0.5, -0.1 * t, 1.25],
            velocity: [10., -3., 480. + t],
            devices: id
                .is_multiple_of(2)
                .then(|| std::array::from_fn::<_, DEVICES, _>(|i| i as f64 / 7.)),
            engine: Engine {
                lit: true,
                afterburner: id % 2 == 1,
                rates: [0.1, -0.2, f64::MIN_POSITIVE],
                rotor: 0.93,
                rotor_turns: 512.25 + t,
                rotor_tilt: [[0.04, -0.01], [-0.02, 0.03 * t]],
                flame: id == 4,
            },
            damage: Damage {
                hp: 37 - id as i32,
                initial_hp: 113,
                sections: std::array::from_fn::<_, DAMAGE_SECTIONS, _>(|i| i as i32 * 3 - 2),
                structural: (id == 2).then_some(DamageSection::LeftWing),
            },
            airborne: id != 5,
            wreck: (id == 4).then_some(wreck::Phase::Falling),
            crashed: id == 4,
        }
    }

    /// A snapshot with every list filled and every enum varied, the tick as
    /// given.
    fn snapshot(tick: u64) -> RenderSnapshot {
        let t = tick as f64;
        RenderSnapshot {
            tick,
            player: pose(0, t),
            targets: (1..=6).map(|id| pose(id, t)).collect(),
            projectiles: (0..3)
                .map(|n| ProjectilePose {
                    id: n,
                    owner: n % 2,
                    weapon: format!("AIM{n}.JT"),
                    shape: (n == 1).then(|| "SYNMSL.SH".to_string()),
                    gun: n == 0,
                    tracer: n == 0,
                    position: [t, 2. * t, 3. * t],
                    previous: [t - 1., 2. * t - 1., 3. * t - 1.],
                    direction: [0., 0.6, 0.8],
                    target: (n == 2).then_some(4),
                    incoming: n == 2,
                    speed_f8: -(n as i32) * 256_000,
                })
                .collect(),
            effects: [
                EffectKind::Flare,
                EffectKind::Chaff,
                EffectKind::Launch,
                EffectKind::Hit,
                EffectKind::Destroyed,
                EffectKind::Ground,
                EffectKind::DebrisImpact,
            ]
            .into_iter()
            .enumerate()
            .map(|(n, kind)| EffectPose {
                kind,
                position: [n as f64, t, -t],
                ticks: 60 - n as u16,
                blast: (n % 2 == 0).then_some(30 + n as u8),
            })
            .collect(),
            marks: vec![
                MarkPose {
                    kind: MarkKind::Crater(18),
                    position: [5., 6., 7.],
                    age: tick,
                    strength: 1.,
                },
                MarkPose {
                    kind: MarkKind::Fire,
                    position: [8., 9., 10.],
                    age: tick + 3,
                    strength: 0.123_456_79,
                },
            ],
            debris: vec![
                DebrisPose {
                    owner: 2,
                    draw: Draw::Model(AircraftId::Su27),
                    position: [1., 2., 3.],
                    attitude: [0.1, 0.2, 0.3],
                    variant: Some(1),
                },
                DebrisPose {
                    owner: 0,
                    draw: Draw::Ownship,
                    position: [4., 5., 6.],
                    attitude: [0.4, 0.5, 0.6],
                    variant: None,
                },
            ],
            pilots: [
                ejection::Phase::Seat,
                ejection::Phase::Freefall,
                ejection::Phase::Inflating,
                ejection::Phase::Parachute,
                ejection::Phase::Landed,
                ejection::Phase::Impact,
            ]
            .into_iter()
            .enumerate()
            .map(|(n, phase)| PilotPose {
                owner: n as u32,
                position: [n as f64, 100. - n as f64, t],
                heading: 0.25 * n as f64,
                phase,
                crew: n % 2 == 1,
            })
            .collect(),
            models: vec![AircraftId::F18, AircraftId::Mig29, AircraftId::F22n],
        }
    }

    #[test]
    fn a_snapshot_with_every_list_filled_round_trips_equal() {
        let models = Models::default();
        for snapshot in [RenderSnapshot::default(), snapshot(1), snapshot(123_456)] {
            let copy = round_trip(&snapshot, &models).unwrap();
            assert_eq!(copy, snapshot);
        }
    }

    #[test]
    fn a_snapshot_coded_against_the_one_before_is_smaller_and_reads_back_equal() {
        let models = Models::default();
        let (previous, current) = (snapshot(1000), snapshot(1001));
        let plain = to_bytes(&current, &models).unwrap();
        let mut s = Saver::with_models(models.clone());
        current.save(&mut s, Some(&previous)).unwrap();
        let body = s.finish_section();
        println!(
            "a snapshot of {} targets: {} bytes alone, {} against the one before",
            current.targets.len(),
            plain.body.len(),
            body.len()
        );
        assert!(body.len() < plain.body.len());
        let mut l = Loader::new(&body, &[], &models);
        let copy = RenderSnapshot::load(&mut l, Some(&previous)).unwrap();
        l.finish().unwrap();
        assert_eq!(copy, current);
    }

    #[test]
    fn an_unknown_draw_mode_or_effect_kind_is_refused() {
        let models = Models::default();
        let mut s = Saver::with_models(models.clone());
        s.writer().write_varint(3);
        let coded = Coded {
            body: s.finish_section(),
            records: Vec::new(),
        };
        assert!(from_bytes::<Draw>(&coded, &models).is_err());
        assert!(effect_kind(7).is_err());
        for n in 0..7 {
            assert_eq!(effect_kind_number(effect_kind(n).unwrap()), n);
        }
    }

    #[test]
    fn damaged_snapshot_bytes_never_panic() {
        let models = Models::default();
        let coded = to_bytes(&snapshot(777), &models).unwrap();
        for cut in 0..coded.body.len() {
            let _ = from_bytes::<RenderSnapshot>(
                &Coded {
                    body: coded.body[..cut].to_vec(),
                    records: Vec::new(),
                },
                &models,
            );
        }
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        for _ in 0..3000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut body = coded.body.clone();
            let at = (seed as usize >> 8) % body.len();
            body[at] ^= 1 << (seed & 7);
            let _ = from_bytes::<RenderSnapshot>(
                &Coded {
                    body,
                    records: Vec::new(),
                },
                &models,
            );
        }
    }
}
