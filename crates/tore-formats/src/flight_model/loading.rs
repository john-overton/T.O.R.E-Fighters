//! FA loaded-weight and drag/thrust selectors. Equipment resolution remains upstream.
use super::div32;
use crate::{Result, invalid};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadedWeight {
    pub weight: i32,
    pub ordinary_percent: i16,
    pub flagged_percent: i16,
}
/// FA 0x4516f2..0x451814 after resolving each hardpoint's mass.
/// Ordinary includes internal fuel; flagged is the native hardpoint flag-1 bucket.
/// These are not invented aerodynamic inside/outside classifications.
pub fn loaded_weight(
    empty: i32,
    maximum: i32,
    internal_fuel_f8: i32,
    ordinary_stores: i32,
    flagged_stores: i32,
    empty_weight_override: bool,
) -> Result<LoadedWeight> {
    if empty_weight_override {
        return Ok(LoadedWeight {
            weight: empty,
            ordinary_percent: 0,
            flagged_percent: 0,
        });
    }
    let ordinary = (internal_fuel_f8 >> 8).wrapping_add(ordinary_stores);
    let total = empty.wrapping_add(flagged_stores).wrapping_add(ordinary);
    if total > maximum {
        return Ok(LoadedWeight {
            weight: maximum,
            ordinary_percent: 50,
            flagged_percent: 50,
        });
    }
    let span = maximum.wrapping_sub(empty);
    if span <= 0 {
        return Err(invalid("invalid native weight range"));
    }
    Ok(LoadedWeight {
        weight: total,
        ordinary_percent: div32(ordinary.wrapping_mul(100), span)? as i16,
        flagged_percent: div32(flagged_stores.wrapping_mul(100), span)? as i16,
    })
}
/// FA 0x47a690 / 0x4784a0: shared clean/pull coefficient loading arithmetic.
/// `damage_addition` applies only when caller's native damage-state gate is set.
pub fn loaded_drag(
    base: i16,
    load_coefficient: i16,
    ordinary: i16,
    flagged: i16,
    damage_addition: i16,
) -> i32 {
    let load = (ordinary as i32 + flagged as i32).wrapping_mul(load_coefficient as i32) / 100;
    (base as i32).wrapping_mul(
        100i32
            .wrapping_add(load)
            .wrapping_add(damage_addition as i32),
    ) / 100
}
/// FA 0x478190: AB falls back to military if its PT value is zero.
/// Halving gate is supplied from native difficulty/player flags, not engine count.
pub fn selected_thrust(military: i32, afterburner: i32, ab: bool, halve: bool) -> i32 {
    let value = if ab && afterburner != 0 {
        afterburner
    } else {
        military
    };
    if halve { value >> 1 } else { value }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weight_buckets_overload_override_and_bad_range() {
        assert_eq!(
            loaded_weight(10000, 20000, 2500 * 256, 1000, 1500, false).unwrap(),
            LoadedWeight {
                weight: 15000,
                ordinary_percent: 35,
                flagged_percent: 15
            }
        );
        assert_eq!(
            loaded_weight(10000, 20000, 20000 * 256, 0, 0, false).unwrap(),
            LoadedWeight {
                weight: 20000,
                ordinary_percent: 50,
                flagged_percent: 50
            }
        );
        assert_eq!(
            loaded_weight(10000, 10000, 0, 0, 0, true).unwrap().weight,
            10000
        );
        assert!(loaded_weight(10000, 10000, 0, 0, 0, false).is_err());
    }
    #[test]
    fn load_damage_scaling_and_thrust_selection() {
        assert_eq!(loaded_drag(100, 50, 35, 15, 10), 135);
        assert_eq!(selected_thrust(100, 0, true, false), 100);
        assert_eq!(selected_thrust(100, 201, true, true), 100);
        assert_eq!(selected_thrust(100, 201, false, false), 100);
    }
}

/// Resolved equipment data; runtime addresses never enter the update kernel.
#[derive(Clone, Copy, Debug)]
pub enum EquipmentMass {
    Other(u16),
    Weapon { unit: i32, divisor: i16 },
    Tank { empty: u16, fuel_f8: i32 },
}
/// FA 0x451724..0x451784. Count bit 15 is not quantity; 0x7fff is a sentinel.
pub fn equipment_mass(
    count: u16,
    equipment: Option<EquipmentMass>,
    divide_weapon: bool,
) -> Result<i32> {
    let count = (count & 0x7fff) as i32;
    if count == 0 || count == 0x7fff {
        return Ok(0);
    }
    Ok(match equipment {
        None => 0,
        Some(EquipmentMass::Other(unit)) => (unit as i32).wrapping_mul(count),
        Some(EquipmentMass::Tank { empty, fuel_f8 }) => (empty as i32)
            .wrapping_mul(count)
            .wrapping_add(fuel_f8 >> 8),
        Some(EquipmentMass::Weapon { unit, divisor }) => {
            let mass = unit.wrapping_mul(count);
            if divide_weapon {
                div32(mass, divisor as i32)?
            } else {
                mass
            }
        }
    })
}
#[derive(Clone, Copy, Debug)]
pub enum ControlCondition {
    /// Native flag 0x80: byte damage percentages and absolute roll-lock deadline.
    Damage {
        pitch: u8,
        roll: u8,
        roll_locked: bool,
    },
    /// Raw cp+0x0e and cpt+0x49 values; semantic names remain unverified.
    Other { current: i16, nominal: i16 },
}
/// FA 0x477ed0..0x478088. Three four-word axes, with loaded pitch min/max.
/// Acceleration/deceleration words and yaw are copied without these reductions.
pub fn loaded_controls(
    mut axes: [[i16; 4]; 3],
    pitch_limits: [i16; 2],
    condition: ControlCondition,
    load_percent: i32,
    load_coefficient: i16,
) -> Result<[[i16; 4]; 3]> {
    axes[1][..2].copy_from_slice(&pitch_limits);
    let scale = |v: i16, p: i32| (v as i32).wrapping_mul(p).wrapping_div(100) as i16;
    match condition {
        ControlCondition::Damage {
            pitch,
            roll,
            roll_locked,
        } => {
            for v in &mut axes[1][..2] {
                *v = scale(*v, 100 - pitch as i32);
            }
            for v in &mut axes[0][..2] {
                *v = if roll_locked {
                    0
                } else {
                    scale(*v, 100 - roll as i32)
                };
            }
        }
        ControlCondition::Other { current, nominal } => {
            let half = nominal as i32 / 2;
            if current as i32 <= half {
                let percent = div32(current as i32 * 100, half)?.clamp(50, 75);
                for axis in &mut axes[..2] {
                    for v in &mut axis[..2] {
                        *v = scale(*v, percent);
                    }
                }
            }
        }
    }
    let percent = 100i32.wrapping_sub(load_percent.wrapping_mul(load_coefficient as i32) / 100);
    for v in &mut axes[0][..2] {
        *v = scale(*v, percent);
    }
    Ok(axes)
}
#[cfg(test)]
mod equipment_tests {
    use super::*;
    #[test]
    fn resolved_mass_and_authority_order() {
        assert_eq!(
            equipment_mass(0xffff, Some(EquipmentMass::Other(20)), false).unwrap(),
            0
        );
        assert_eq!(
            equipment_mass(
                0x8002,
                Some(EquipmentMass::Tank {
                    empty: 100,
                    fuel_f8: 51 * 256
                }),
                false
            )
            .unwrap(),
            251
        );
        assert!(
            equipment_mass(
                2,
                Some(EquipmentMass::Weapon {
                    unit: 10,
                    divisor: 0
                }),
                true
            )
            .is_err()
        );
        let a = loaded_controls(
            [[-101, 101, 7, 8]; 3],
            [-50, 100],
            ControlCondition::Damage {
                pitch: 25,
                roll: 10,
                roll_locked: false,
            },
            50,
            50,
        )
        .unwrap();
        assert_eq!(a, [[-67, 67, 7, 8], [-37, 75, 7, 8], [-101, 101, 7, 8]]);
        assert!(
            loaded_controls(
                [[0; 4]; 3],
                [0; 2],
                ControlCondition::Other {
                    current: 0,
                    nominal: 0
                },
                0,
                0
            )
            .is_err()
        );
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GLoadInput {
    pub range: [i16; 2],
    pub altitude_f8: i32,
    pub speed_f8: i32,
    pub flaps: bool,
    pub structure: [i16; 2],
    pub load_percent: i32,
    pub elevator_coefficient: i16,
    pub player: bool,
    pub low_skill: bool,
    pub extra_g_flag: bool,
}
/// FA 0x452167..0x452482 after the external refresh/weight-update gate.
/// Native row scan, adjacent-row interpolation, loading and player/skill flags.
pub fn loaded_g_limits(envelopes: &[crate::aircraft::Envelope], i: GLoadInput) -> Result<[i16; 2]> {
    if i.range[0] > i.range[1] || (i.range[1] as i32 - i.range[0] as i32) > 255 {
        return Err(invalid("invalid diagnostic G envelope range"));
    }
    let limits = |g: i32| -> Result<super::Limits> {
        super::envelope_limits(
            envelopes
                .iter()
                .find(|e| e.g == g)
                .ok_or_else(|| invalid("missing loaded G row"))?,
            i.altitude_f8,
            i.flaps,
            i.structure,
        )
    };
    let mut bounds = [0i16; 2];
    for g in i.range[0] as i32..=i.range[1] as i32 {
        if super::envelope_class(&limits(g)?, i.speed_f8) == 0 {
            bounds[0] = bounds[0].min((g << 8) as i16);
            bounds[1] = bounds[1].max((g << 8) as i16);
        }
    }
    let speed = i.speed_f8 >> 8;
    for (j, bound) in bounds.iter_mut().enumerate() {
        let g = (*bound >> 8) as i32;
        if g == i.range[j] as i32 {
            continue;
        }
        let current = limits(g)?;
        let next = limits(g + if j == 0 { -1 } else { 1 })?;
        let pair = if next.minimum >= speed {
            Some((current.minimum, next.minimum))
        } else if next.maximum <= speed {
            Some((current.maximum, next.maximum))
        } else {
            None
        };
        if let Some((a, b)) = pair.filter(|(a, b)| a != b) {
            let delta = if j == 0 {
                a.wrapping_sub(speed)
            } else {
                speed.wrapping_sub(a)
            };
            *bound = bound.wrapping_add(div32(delta.wrapping_shl(8), b.wrapping_sub(a))? as i16);
        }
    }
    let percent =
        100i32.wrapping_sub(i.load_percent.wrapping_mul(i.elevator_coefficient as i32) / 100);
    for b in &mut bounds {
        *b = (((*b as i32).wrapping_mul(percent)) / 100) as i16;
    }
    if !i.player && i.low_skill {
        bounds[0] = bounds[0].wrapping_add(256).min(-512);
        bounds[1] = bounds[1].wrapping_sub(256).max(512);
    }
    if i.player && i.extra_g_flag {
        bounds[0] = bounds[0].wrapping_sub(256).max(i.range[0].wrapping_shl(8));
        bounds[1] = bounds[1].wrapping_add(256).min(i.range[1].wrapping_shl(8));
    }
    bounds[0] = bounds[0].min(0);
    bounds[1] = bounds[1].max(512);
    Ok(bounds)
}

#[cfg(test)]
mod g_limit_tests {
    use super::*;
    #[test]
    fn adjacent_rows_loading_player_flags_and_missing_data() {
        let rows: Vec<_> = (-4i32..=4)
            .map(|g| crate::aircraft::Envelope {
                g,
                points: vec![
                    [100. + g.abs() as f64 * 50., 0.],
                    [100. + g.abs() as f64 * 50., 10000.],
                    [1000., 10000.],
                    [1000., 0.],
                ],
            })
            .collect();
        let i = GLoadInput {
            range: [-4, 4],
            altitude_f8: 1000 * 256,
            speed_f8: 275 * 256,
            flaps: false,
            structure: [2000, 2000],
            load_percent: 0,
            elevator_coefficient: 20,
            player: true,
            low_skill: false,
            extra_g_flag: false,
        };
        assert_eq!(loaded_g_limits(&rows, i).unwrap(), [-896, 896]);
        assert_eq!(
            loaded_g_limits(
                &rows,
                GLoadInput {
                    load_percent: 50,
                    ..i
                }
            )
            .unwrap(),
            [-806, 806]
        );
        assert_eq!(
            loaded_g_limits(
                &rows,
                GLoadInput {
                    extra_g_flag: true,
                    ..i
                }
            )
            .unwrap(),
            [-1024, 1024]
        );
        assert_eq!(
            loaded_g_limits(
                &rows,
                GLoadInput {
                    player: false,
                    low_skill: true,
                    ..i
                }
            )
            .unwrap(),
            [-640, 640]
        );
        assert!(loaded_g_limits(&rows[1..], i).is_err());
    }
}
