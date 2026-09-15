//! Reviewed fog palette helpers in the six-bit source domain. These are not the
//! complete ordered native palette pipeline, and exclude unreviewed non-weather effects.
use crate::{Result, invalid};

/// FA 0x4b3f28..0x4b3f74, restricted to the reviewed 0..255 tint domain.
/// Caller owns smoothing state and supplies the separately produced reduction.
/// One invocation is one palette pass, not an inferred elapsed-time duration.
pub fn smooth_tint(current: u8, scalar: i16, reduction: i16, increment: u8) -> Result<u8> {
    let target = (i32::from(scalar) - i32::from(reduction)).max(0);
    if target > 255 {
        return Err(invalid("weather tint target outside reviewed domain"));
    }
    let current = i32::from(current);
    let increment = i32::from(increment);
    Ok(if current < target {
        (current + increment).min(target)
    } else {
        (current - increment).max(target)
    } as u8)
}

/// FA 0x4b4000..0x4b403c and 0x4c8f10: tint indices 64..254, then
/// 47..60 with strength capped at 92. Apply before six-to-eight-bit expansion.
/// Strength 256's distinct wrapping branch is deliberately outside this API.
pub fn apply_tint(palette: &mut [[u8; 3]; 256], tint: [u8; 3], strength: u8) -> Result<()> {
    if palette.iter().flatten().chain(tint.iter()).any(|v| *v > 63) {
        return Err(invalid("weather tint requires six-bit source colors"));
    }
    tint_colors(&mut palette[64..255], tint, strength);
    tint_colors(&mut palette[47..61], tint, strength.min(92));
    Ok(())
}

/// FA 0x4b3fd5 / 0x4c8e6c: whiten entries 0..254 before weather tint.
pub fn apply_sun_whitening(palette: &mut [[u8; 3]; 256], strength: u8) -> Result<()> {
    if palette.iter().flatten().any(|v| *v > 63) {
        return Err(invalid("sun whitening requires six-bit colors"));
    }
    for channel in palette[..255].iter_mut().flatten() {
        *channel += (((63 - u16::from(*channel)) * u16::from(strength)) >> 8) as u8;
    }
    Ok(())
}

/// FA 0x4b3f74: HUD brightness changes palette entry 40 before sun whitening.
/// Positive values blend toward 63; negative values multiply toward zero.
pub fn apply_hud_brightness(palette: &mut [[u8; 3]; 256], brightness: i16) -> Result<()> {
    if !(-256..=256).contains(&brightness) || palette[40].iter().any(|c| *c > 63) {
        return Err(invalid("HUD brightness outside reviewed domain"));
    }
    for channel in &mut palette[40] {
        let c = i32::from(*channel);
        *channel = if brightness >= 0 {
            c + (((63 - c) * i32::from(brightness)) >> 8)
        } else {
            (c * (256 + i32::from(brightness))) >> 8
        } as u8;
    }
    Ok(())
}

fn tint_colors(colors: &mut [[u8; 3]], tint: [u8; 3], strength: u8) {
    // Native shifts strength right one, signed-multiplies the byte difference,
    // doubles the product and subtracts its high byte. Negative differences
    // round down; odd strengths lose their low bit.
    let even_strength = i32::from(strength & !1);
    for color in colors {
        for (channel, target) in color.iter_mut().zip(tint) {
            let difference = i32::from(*channel) - i32::from(target);
            *channel = (i32::from(*channel) - ((difference * even_strength) >> 8)) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_ranges_rounding_and_special_cap_match_source() {
        let mut palette = [[10, 30, 50]; 256];
        apply_tint(&mut palette, [30, 20, 0], 235).unwrap();
        for index in [0, 46, 61, 63, 255] {
            assert_eq!(palette[index], [10, 30, 50]);
        }
        for index in [64, 192, 224, 254] {
            assert_eq!(palette[index], [29, 21, 5]);
        }
        for index in [47, 60] {
            assert_eq!(palette[index], [18, 27, 33]);
        }
        let before = palette;
        apply_tint(&mut palette, [63; 3], 1).unwrap();
        assert_eq!(palette, before);
        assert!(apply_tint(&mut palette, [64; 3], 10).is_err());
        assert_eq!(palette, before, "reject before mutation");
    }

    #[test]
    fn sunlight_whitens_before_fog_without_changing_transparency() {
        let mut p = [[0, 32, 63]; 256];
        apply_sun_whitening(&mut p, 255).unwrap();
        assert_eq!(p[0], [62, 62, 63]);
        assert_eq!(p[254], [62, 62, 63]);
        assert_eq!(p[255], [0, 32, 63]);
        apply_tint(&mut p, [0; 3], 254).unwrap();
        assert_eq!(p[64], [1; 3]);
    }
    #[test]
    fn hud_brightness_bounds_order_and_tint_exclusion() {
        let source = [[12, 32, 60]; 256];
        for (brightness, expected) in [
            (-256, [0; 3]),
            (-16, [11, 30, 56]),
            (0, [12, 32, 60]),
            (16, [15, 33, 60]),
            (256, [63; 3]),
        ] {
            let mut p = source;
            apply_hud_brightness(&mut p, brightness).unwrap();
            assert_eq!(p[40], expected);
            assert_eq!(p[39], source[39]);
            assert_eq!(p[41], source[41]);
            apply_tint(&mut p, [0; 3], 254).unwrap();
            assert_eq!(p[40], expected);
        }
        let mut p = source;
        apply_hud_brightness(&mut p, -256).unwrap();
        apply_sun_whitening(&mut p, 128).unwrap();
        assert_eq!(p[40], [31; 3], "sunlight follows HUD dimming");
        let before = p;
        assert!(apply_hud_brightness(&mut p, 257).is_err());
        assert_eq!(p, before);
    }
    #[test]
    fn smoothing_clamps_overshoot_and_subtracts_reduction() {
        assert_eq!(smooth_tint(0, 225, 0, 16).unwrap(), 16);
        assert_eq!(smooth_tint(224, 225, 0, 16).unwrap(), 225);
        assert_eq!(smooth_tint(235, 217, 0, 16).unwrap(), 219);
        assert_eq!(smooth_tint(219, 217, 0, 16).unwrap(), 217);
        assert_eq!(smooth_tint(225, 225, 100, 16).unwrap(), 209);
        assert_eq!(smooth_tint(10, 20, 30, 16).unwrap(), 0);
        assert!(smooth_tint(0, i16::MAX, i16::MIN, 16).is_err());
    }
}
