//! The once-per-second state checksum. The app computes it from the live,
//! exact aircraft states and stores it in [`crate::Frame::checksum`]; the
//! comparison export uses it to find the first second two recordings differ.

use crate::codec::{FNV_OFFSET, fnv1a};
use crate::model::AircraftState;

/// FNV-1a 64 over every aircraft's exact state, in id order so list order
/// does not matter: id, position, attitude, velocity, airspeed, G, devices,
/// heat, fuel, controls, auxiliary rates (as IEEE bits), then flags, wreck
/// phase, hit points, regional damage and the structural section.
pub fn state_checksum(aircraft: &[AircraftState]) -> u64 {
    let mut sorted: Vec<&AircraftState> = aircraft.iter().collect();
    sorted.sort_by_key(|a| a.id);
    let mut hash = FNV_OFFSET;
    for a in sorted {
        hash = fnv1a(hash, &a.id.to_le_bytes());
        let floats = a
            .position
            .iter()
            .chain(&a.attitude)
            .chain(&a.velocity)
            .chain([&a.airspeed, &a.g])
            .chain(&a.devices)
            .chain([&a.heat, &a.fuel_lb])
            .chain(&a.controls)
            .chain(&a.auxiliary_rates);
        for v in floats {
            hash = fnv1a(hash, &v.to_bits().to_le_bytes());
        }
        hash = fnv1a(hash, &a.flags.bits().to_le_bytes());
        hash = fnv1a(hash, &[a.wreck_phase]);
        for v in [a.hp, a.max_hp].iter().chain(&a.sections) {
            hash = fnv1a(hash, &v.to_le_bytes());
        }
        hash = fnv1a(hash, &a.structural_section.map_or([0, 0], |s| [1, s]));
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_ignores_order_but_not_one_bit_of_state() {
        let a = AircraftState {
            id: 0,
            position: [1., 2., 3.],
            ..AircraftState::default()
        };
        let b = AircraftState {
            id: 7,
            hp: 100,
            ..AircraftState::default()
        };
        let one = state_checksum(&[a.clone(), b.clone()]);
        assert_eq!(one, state_checksum(&[b.clone(), a.clone()]));
        let mut moved = a.clone();
        moved.position[0] = f64::from_bits(1f64.to_bits() + 1);
        assert_ne!(one, state_checksum(&[moved, b.clone()]));
        let mut damaged = b;
        damaged.structural_section = Some(0);
        assert_ne!(one, state_checksum(&[a, damaged]));
    }
}
