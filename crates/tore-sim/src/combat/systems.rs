//! Reviewed scalar contracts; caller RNG, difficulty and full damage dispatch remain explicit.
use tore_formats::weapons::Countermeasures;

/// HARDFindJammer 0x452ed8 and PROJHitChance 0x4c34a4..0x4c34d4.
pub fn deception_chance(ecm: Countermeasures, signature: u8, powered: bool) -> u8 {
    if !powered {
        return 0;
    }
    match signature {
        2 if ecm.mode_flags & 0x100 != 0 => ecm.infrared_deception_chance,
        3 if ecm.mode_flags & 0x10 != 0 => ecm.radar_deception_chance,
        _ => 0,
    }
}
pub fn hit_chance(base: i32, deception: u8) -> i32 {
    (i64::from(base) * (100 - i64::from(deception)) / 100) as i32
}
/// DAMAGEDoHit 0x40f9b0..0x40f9e8, with explicit bounded random draw.
pub fn damage_amount(base: u16, percent: u16, roll40: u8) -> i32 {
    (i64::from(base) * i64::from(percent) / 100 * (80 + i64::from(roll40.min(39))) / 100) as i32
}
/// Normal damage branch 0x40fd0a..0x40fd71. Forced hits use a different branch.
pub fn subsystem_chance(total: i32, structure: i32, hit: i32) -> i32 {
    if structure <= 0 || total < structure / 3 {
        return 0;
    }
    ((i64::from(total) * 50 / i64::from(structure)).min(70) + i64::from(hit / 4)).min(90) as i32
}
/// 0x410810..0x4108a9; caller resolves difficulty and special afterburner availability.
pub fn eligible(
    byte: u8,
    count: u8,
    total: i32,
    structure: i32,
    difficulty: u32,
    afterburner_available: bool,
    index: usize,
) -> bool {
    byte & 15 != 0
        && count < (byte >> 4 & 3)
        && !(byte & 0x80 != 0
            && ((difficulty & 2 != 0 && total < structure)
                || (difficulty & 4 != 0 && i64::from(total) < i64::from(structure) * 101 / 100)))
        && (index != 8 || afterburner_available)
}
/// Ten weighted attempts then at most 45 fallback entries, excluding forced-only
/// indices 31..33 on fallback. Rolls are caller-owned, not native scheduler parity.
pub fn select(
    table: &[u8; 45],
    counts: &[u8; 45],
    total: i32,
    structure: i32,
    mut roll: impl FnMut(u16) -> u16,
) -> Option<usize> {
    let mut last = 0;
    for _ in 0..10 {
        let draw = roll(200).min(199);
        let mut cumulative = 0;
        let selected = table.iter().position(|v| {
            cumulative += u16::from(v & 15);
            draw < cumulative
        });
        let Some(index) = selected else {
            continue;
        }; // bounded rejection of malformed/short tables
        last = index;
        if eligible(
            table[index],
            counts[index],
            total,
            structure,
            0,
            true,
            index,
        ) {
            return Some(index);
        }
    }
    for offset in 1..=45 {
        let i = (last + offset) % 45;
        if !(31..=33).contains(&i) && eligible(table[i], counts[i], total, structure, 0, true, i) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_integer_damage_and_selection_boundaries() {
        assert_eq!(damage_amount(25, 100, 0), 20);
        assert_eq!(damage_amount(25, 100, 39), 29);
        assert_eq!(damage_amount(7, 50, 39), 3);
        assert_eq!(subsystem_chance(32, 100, 20), 0);
        assert_eq!(subsystem_chance(33, 100, 20), 21);
        assert_eq!(subsystem_chance(200, 100, 100), 90);
        let mut table = [0; 45];
        table[36] = 0x12;
        table[37] = 0x11;
        let mut counts = [0; 45];
        assert_eq!(select(&table, &counts, 40, 100, |_| 0), Some(36));
        counts[36] = 1;
        assert_eq!(select(&table, &counts, 40, 100, |_| 0), Some(37));
        counts[37] = 1;
        assert_eq!(select(&table, &counts, 40, 100, |_| 0), None);
        assert!(!eligible(0x91, 0, 99, 100, 2, true, 0));
        assert!(eligible(0x91, 0, 100, 100, 2, true, 0));
        assert!(!eligible(0x11, 0, 100, 100, 0, false, 8));
    }
    #[test]
    fn deception_is_signature_specific_and_integer_scaled() {
        let mut e = Countermeasures {
            weight: 0,
            flags: 0,
            mode_flags: 0x10,
            chaff: [0; 4],
            flare: [0; 4],
            radar_deception_chance: 30,
            radar_signature_add: 0,
            radar_noise_range: [0; 2],
            infrared_deception_chance: 60,
            infrared_signature_add: 0,
            infrared_lose_lock_time: 0,
        };
        assert_eq!(deception_chance(e, 3, true), 30);
        assert_eq!(deception_chance(e, 2, true), 0);
        assert_eq!(deception_chance(e, 3, false), 0);
        e.mode_flags |= 0x100;
        assert_eq!(deception_chance(e, 2, true), 60);
        assert_eq!(hit_chance(73, 30), 51);
        assert_eq!(hit_chance(100, 100), 0);
    }
}
