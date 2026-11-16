//! Runtime masks from reviewed flat source fills; fitted rear-view optics.
use crate::{attitude::Basis, camera::Camera, flight::State, menu::Sprite};
use tore_formats::aircraft::AircraftId;
pub const SIZE: [u32; 2] = [768, 384];

pub struct Masks {
    pub pixels: Vec<u8>,
    pub rects: [[f32; 4]; 3],
}
/// Seed pixels of the mirror regions in a cockpit art family's 1280x490
/// picture, centre, left and right (`None` where the art has no such mirror).
/// Authored identification hints from inspected art (fitted; 2026-10-12 for
/// the families added by slice BC), not native camera records. Families that
/// share a cockpit picture share a row: the picture's own name decides, so
/// the F-4 variants and the A-4E use the F-4 picture, the Su-27, AV-8B and
/// F-15 the AV-8 picture, the MiG-29, Su-25, MiG-23 and Mi-24 the SU-33 one,
/// the F-104 and A-10 the F-104 one. Pictures without a flat mirror fill
/// (F-16, F-22, X-31, MiG-17, MiG-21 and the transport/helicopter AC-130
/// picture) have none.
pub(crate) fn seeds(id: AircraftId) -> Option<[Option<[usize; 2]>; 3]> {
    let s = |x, y| Some([x, y]);
    Some(match id.cockpit_stem() {
        "F18" => [s(640, 40), s(110, 380), s(1170, 380)],
        "F14" => [s(640, 40), s(140, 250), s(1140, 250)],
        "F4" => [s(640, 40), s(200, 300), s(1080, 300)],
        "RAF" => [s(640, 40), s(190, 360), s(1090, 360)],
        // The Gripen's own ~GRIH.PIC (1279x490, GRI-7, 2026-11-10): one flat
        // blue-grey fill (117, 138, 154) for the three mirror glasses.
        "GRI" => [s(640, 44), s(245, 304), s(1040, 304)],
        "AV8" => [s(640, 40), s(204, 257), s(1064, 257)],
        "SU35" => [s(640, 40), s(203, 297), s(1075, 297)],
        "A7" => [s(641, 35), s(120, 304), s(1161, 304)],
        "F104" => [None, s(231, 232), s(1049, 232)],
        "SU33" => [None, s(176, 195), s(1103, 195)],
        // Tornado (TOR-7, 2026-11-11): the Eurofighter picture ~EF2H has three
        // separate flat fills of one blue-grey (105, 113, 134): the top
        // centre panel (x 479 to 797) and the two lower canopy rails
        // (x 125 to 277 and 999 to 1151). Each floods well under the cap.
        "EF2" => [s(640, 50), s(200, 330), s(1090, 330)],
        "Y141" => [None, s(375, 91), s(902, 91)],
        // F-5E (F5E-7, 2026-11-10): the X-29 picture ~F29H has three flat
        // light-blue mirror fills (top centre and the two lower canopy rails).
        "F29" => [s(640, 70), s(120, 420), s(1160, 420)],
        _ => return None,
    })
}
/// Four-connected exact-color flood; bounds prevent unrelated artwork leaks.
pub fn masks(source: &Sprite, id: AircraftId) -> Option<Masks> {
    // Every reviewed picture is 1280x490 except the Gripen's `~GRIH.PIC`,
    // which is 1279x490 (GRI-7).
    if !matches!((source.width, source.height), (1280 | 1279, 490)) {
        return None;
    }
    flood(source, seeds(id)?)
}
fn flood(source: &Sprite, seeds: [Option<[usize; 2]>; 3]) -> Option<Masks> {
    let (w, h) = (source.width, source.height);
    if w == 0 || h == 0 || w.checked_mul(h)?.checked_mul(4)? != source.rgba.len() {
        return None;
    }
    let mut result = Masks {
        pixels: vec![0; w * h],
        rects: [[0.; 4]; 3],
    };
    for (index, seed) in seeds.into_iter().enumerate() {
        let Some([sx, sy]) = seed else {
            continue;
        };
        if sx >= w || sy >= h {
            return None;
        }
        let seed = sy * w + sx;
        let color = &source.rgba[seed * 4..seed * 4 + 4];
        if color[3] != 255 || result.pixels[seed] != 0 {
            return None;
        }
        let mut todo = vec![seed];
        let (mut x0, mut y0, mut x1, mut y1, mut count) = (w, h, 0, 0, 0);
        while let Some(at) = todo.pop() {
            if result.pixels[at] != 0 || &source.rgba[at * 4..at * 4 + 4] != color {
                continue;
            }
            result.pixels[at] = index as u8 + 1;
            count += 1;
            if count > w * h / 8 {
                return None;
            }
            let (x, y) = (at % w, at / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
            if x > 0 {
                todo.push(at - 1);
            }
            if x + 1 < w {
                todo.push(at + 1);
            }
            if y > 0 {
                todo.push(at - w);
            }
            if y + 1 < h {
                todo.push(at + w);
            }
        }
        if count < 4 {
            return None;
        }
        result.rects[index] = [x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32];
    }
    Some(result)
}
pub(crate) fn rear_basis(body: Basis) -> Basis {
    Basis {
        right: body.right.map(|v| -v),
        up: body.up,
        forward: body.forward.map(|v| -v),
    }
}
/// The seat every aircraft without an entry of its own uses: 7 ft above and
/// 10 ft ahead of the model origin (fitted).
const DEFAULT_EYE: [f64; 2] = [7., 10.];
/// Fitted pilot's eye (feet above, feet ahead of the model origin) for an
/// aircraft whose drawn cockpit is not near the shared seat. Only entries
/// listed here differ; every other aircraft keeps [`DEFAULT_EYE`].
///
/// B-2 (B2-7, 2026-11-15, fitted from the shape): the glasshouse of `B2.SH`
/// starts 20 ft ahead of the origin (windscreen faces from y 62 to 80 shape
/// units at 1/3 ft a unit, 21 units high at the back falling to 14), under a
/// roof 22 units (7.3 ft) high at y 60, and the spine behind it peaks at 24
/// units (8 ft) at y 44. The seat is 20 ft ahead and 10 ft up: at the
/// windscreen, 2.7 ft above the roof, so the back view clears the spine, not
/// the shared seat 10 to 17 ft behind the glass and a foot below the spine
/// skin. The record's own `viewOffset` (0, 30, 60), which no host code
/// reads, gives the same figure read at the shape's 1/3 ft a unit (30 up, 60
/// ahead); that reading of its units is unproven. A seat inside the skin
/// (6 ft up) draws the inside of the spine and was rejected.
///
/// F-117 (F117-7, 2026-11-15, fitted from the scaled shape: `F117.SH` after
/// the per-axis real-size factors, 1/3 ft a unit times 0.7647, 1.0463 and
/// 0.8869): the faceted canopy runs from 10.5 to 18.8 ft ahead of the origin,
/// its crown on the centre line 4.6 ft up at 14.6 ft ahead (4.4 ft at 12 ft,
/// 3.6 ft at 17.4 ft), on a back that rises evenly from the tail (0 ft at 26
/// ft aft of the origin, 3.9 ft at 9 ft ahead). The seat is the canopy's
/// middle, 14 ft ahead, and 6 ft up, 1.4 ft over the crown, so the back view
/// looks over the canopy and down the spine to the ruddervators, not out of
/// the inside of the glass. The shared seat (10 ft ahead, 3 ft above the back
/// at that point) is 4 ft too far aft of the cockpit.
///
/// B-52 (B52-7, 2026-11-15, fitted from the shape: `B52.SH` at 2/3 ft a
/// unit): the fuselage top is flat at 7.3 ft (11 units) from 31 ft aft of the
/// origin to 58.7 ft ahead of it, where the nose steps down. The flight deck
/// is the nose forward of that step: its glass faces stand at 58.7 ft ahead
/// (a 10.7 ft wide window, 3.3 to 6 ft up) and its roof is flat at 3.3 ft
/// (5 units) to 66.7 ft. The shared seat (10 ft ahead, 7 ft up) is 49 ft aft
/// of the deck and 0.3 ft under the fuselage skin, so F2 Back looked at the
/// inside of the roof. The seat is the middle of the deck, 63 ft ahead, and 9
/// ft up: 1.7 ft over the fuselage crown behind it, so the back view clears
/// the spine and looks down the length of the aircraft to the fin and the
/// tail cone.
///
/// B-1B (B1B-7, 2026-11-16, fitted from the shape: `B1.SH` at 4/3 ft a
/// unit): the fuselage crown rises along the centre line from 8 ft (z 6
/// units) at 38.7 ft ahead of the origin to 10.7 ft (z 8) at 64 ft, where the
/// flight deck begins: the nose roof drops to 6.7 ft (z 5) and a canopy
/// ridge 2.7 ft wide runs on from the crown (10.7 ft at 64 ft, 9.3 ft at 68
/// ft) and slopes down to the roof at 72 ft; the nose cone ends 86.7 ft
/// ahead. The shared seat is 54 ft aft of the deck, inside the fuselage, so
/// F2 Back looked at its inner walls. A seat inside the ridge (8.7 ft up)
/// looked at the ridge's own back wall 2.7 ft behind it (the skin draws from
/// inside), so the seat is the deck's middle in plan, 66 ft ahead, and 11.3
/// ft up: 0.6 ft over the 10.7 ft crown behind it, so the back view clears
/// the spine and looks down the length of the jet to the fins.
///
/// Tu-160 (T160-7, 2026-11-16, fitted from the shape: `TU160.SH` at 8/3 ft a
/// unit, the surface on the centre line): the fuselage crown is flat at 10.7 ft
/// (4 units) from 16 ft aft of the origin to 72 ft ahead of it; the glass is the
/// four faces from 72 to 80 ft, falling to 5.3 ft, and the nose cone runs on
/// down to the tip at 101 ft. The record's own eye (`viewOffset` 0, 35, 60:
/// 11.7 ft up, 20 ft ahead) is the Tu-26's copy, 52 ft aft of the glass and
/// inside the skin. The seat is the flight deck behind the windscreen, 71 ft
/// ahead, and 12 ft up, 1.3 ft over the crown behind it, so the back view
/// clears the fuselage and looks down its length to the fin and the tail.
///
/// F-111F (F111-7, 2026-11-16, fitted from the scaled shape: `F111.SH` after
/// the per-axis real-size factors, 2/3 ft a unit times 0.859, 1.0 and 0.827):
/// the cockpit hump stands on the centre line from 8.7 ft ahead of the origin
/// (2.8 ft up) rising to a crown 3.3 ft up from 23.3 to 27.3 ft ahead, then
/// the windscreen falls to 2.8 ft at 28 ft and the canopy ends at 30.7 ft
/// (1.1 ft up) before the nose cone. The shared seat (10 ft ahead, 7 ft up)
/// is 15 ft aft of the crown and 4 ft over the spine, outside the cockpit. The
/// seat is the crown's middle, 25 ft ahead, and 5 ft up: 1.7 ft over the crown,
/// as the B-52's, so the back view clears the hump and looks down the spine to
/// the fin and the tailerons, not out of the inside of the glass.
///
/// Tu-26 (T26-7, 2026-11-16, fitted from the shape: `TU26.SH` at 4/3 ft a
/// unit): the fuselage top is flat at 5.33 ft (z 4) from 16 ft to 61.3 ft
/// ahead of the origin, where the windscreen faces (y 46 to 49 units) fall
/// to 2.67 ft at 65.3 ft and 1.33 ft at the nose cone's start at 68 ft. The
/// glasshouse (side glass and roof from y 34 to 49 units, 45.3 to 65.3 ft
/// ahead, 5.3 ft wide) stands no higher than the fuselage behind it, so the
/// shared seat (10 ft ahead, 7 ft up) is 46 ft aft of the windscreen and 1.7
/// ft over the top skin, out of its own cockpit. The seat is the glasshouse's
/// middle, 56 ft ahead, 7 ft up: 1.7 ft over the 5.33 ft crown (the B-52's
/// margin), so the back view clears the spine and looks down the length of
/// the aircraft to the fin and the nozzles.
///
/// Tu-95 (T95-7, 2026-11-16, fitted from the shape: `TU95.SH` at 2/3 ft a
/// unit): the fuselage top is flat at 3.1 ft (4.7 units) from 45 ft aft of the
/// origin to 48 ft ahead of it. The flight deck is the nose forward of that:
/// a roof 3.3 ft up (5 units) from 47.3 to 51.3 ft ahead, then the windscreen
/// falling to 1.3 ft (2 units) at 58.7 ft and the nose cone ending 64 ft
/// ahead. The shared seat (10 ft ahead, 7 ft up) is 37 ft aft of the deck and
/// 3.9 ft over the crown, so F2 Back already cleared the spine but looked out
/// from the middle of the wing, not the cockpit. The seat is the middle of
/// the deck, 53 ft ahead, and 5 ft up: 1.7 ft over the roof (the B-52's
/// margin), so the back view looks over the crown and down the length of the
/// aircraft to the fin, past the four propeller discs.
pub(crate) fn eye_offset(id: Option<AircraftId>) -> [f64; 2] {
    match id {
        Some(AircraftId::B2) => [10., 20.],
        Some(AircraftId::F117) => [6., 14.],
        Some(AircraftId::B52) => [9., 63.],
        Some(AircraftId::B1b) => [11.3, 66.],
        Some(AircraftId::Tu160) => [12., 71.],
        Some(AircraftId::F111) => [5., 25.],
        Some(AircraftId::Tu26) => [7., 56.],
        Some(AircraftId::Tu95) => [5., 53.],
        _ => DEFAULT_EYE,
    }
}
/// The pilot's eye of aircraft `id` (the shared seat when the type is not
/// known), in the world.
pub(crate) fn pilot_eye_of(id: Option<AircraftId>, position: [f64; 3], body: Basis) -> [f64; 3] {
    let [up, forward] = eye_offset(id);
    std::array::from_fn(|i| position[i] + body.up[i] * up + body.forward[i] * forward)
}

/// How much wider the rear camera's field of view is than a 1x view's 60
/// degrees vertical: 66 degrees vertical, about 105 across the 2:1 rear
/// picture (98 before). Opinionated, John 2026-10-19 ("maybe 10% wider").
pub const WIDENING: f64 = 1.1;
/// The rear camera's zoom: a 1x view's half-angle tangent over the widened
/// one's.
pub fn rear_zoom() -> f32 {
    let half = std::f64::consts::FRAC_PI_6;
    (half.tan() / (half * WIDENING).tan()) as f32
}
/// The one rear camera every mirror of a cockpit shows a crop of.
pub fn camera(state: &State) -> Camera {
    let body = Basis::new(state.yaw, state.pitch, state.bank);
    let [yaw, pitch, bank] = rear_basis(body).angles();
    let mut c = Camera::new();
    c.weather_slot = 1;
    // The rear feed stays on the shared seat for every aircraft: only cockpit
    // families with fitted mirror seeds draw it, and the B-2's has none.
    c.position = pilot_eye_of(None, state.position, body);
    c.yaw = yaw as f32;
    c.pitch = pitch as f32;
    c.roll = -bank as f32;
    c.zoom = rear_zoom();
    c
}
/// Where mirror `id` (1 centre, 2 left, 3 right) of source rectangle
/// `rect` samples the rear picture at `uv` within the rectangle: an
/// aspect-preserving, horizontally reflected crop of the one picture. The
/// CPU twin of `cockpit.wgsl`'s mirror sampling, for tests.
#[cfg(test)]
pub fn crop(id: usize, rect: [f32; 4], uv: [f32; 2]) -> [f32; 2] {
    let width = (rect[2] / rect[3] / 2.).min(1.);
    let height = (2. * rect[3] / rect[2]).min(1.);
    let center = [0.5, 0.8, 0.2][id - 1];
    let start = (center - width * 0.5).clamp(0., 1. - width);
    [start + (1. - uv[0]) * width, 0.5 + (uv[1] - 0.5) * height]
}
/// Which visible flight frames draw the rear picture: every other one, the
/// mirrors showing the last picture in between (John, 2026-10-19). The first
/// visible frame after hidden ones always draws, so a mirror never shows an
/// old picture.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cadence {
    frame: u64,
    drawn: Option<u64>,
}
impl Cadence {
    /// Called once per flight frame; whether this frame draws the mirrors.
    pub fn due(&mut self, visible: bool) -> bool {
        self.frame += 1;
        if !visible {
            return false;
        }
        let due = self.drawn != Some(self.frame - 1);
        if due {
            self.drawn = Some(self.frame);
        }
        due
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mirrors_draw_every_other_visible_frame_and_never_start_stale() {
        let mut c = Cadence::default();
        let drawn: Vec<bool> = (0..6).map(|_| c.due(true)).collect();
        assert_eq!(drawn, [true, false, true, false, true, false]);
        // Hidden frames draw nothing; the first visible one after them draws.
        assert!(!c.due(false));
        assert!(!c.due(false));
        assert!(c.due(true));
        assert!(!c.due(true));
        // A single hidden frame between: the next visible frame draws even
        // though the frame before the gap did not.
        assert!(c.due(true));
        assert!(!c.due(false));
        assert!(c.due(true));
        // A fresh cadence (new aircraft or scene) draws at once.
        let mut c = Cadence::default();
        assert!(c.due(true));
        // Half the visible frames over a long run.
        let mut c = Cadence::default();
        assert_eq!((0..1000).filter(|_| c.due(true)).count(), 500);
    }
    #[test]
    fn rear_camera_is_ten_percent_wider_than_a_1x_view() {
        let zoom = rear_zoom();
        // Camera::uniform projects with focal = half height * sqrt(3) * zoom.
        let half = (1. / (3f64.sqrt() * f64::from(zoom))).atan();
        assert!((2. * half.to_degrees() - 66.).abs() < 1e-3, "{zoom}");
        let across = (2. * (2. * half.tan()).atan()).to_degrees();
        assert!((across - 104.86).abs() < 0.05, "{across}");
        assert!(zoom < 1.);
    }
    #[test]
    fn every_mirror_is_a_crop_of_the_one_rear_picture() {
        // Each mirror's corners land inside the 2:1 picture, keep the
        // mirror's own aspect on it, and the side mirrors look to their own
        // side of it (the picture is reflected: the left mirror shows the
        // right of the rear picture's centre, which is the aircraft's left).
        for (id, rect) in [
            (1, [500., 20., 280., 50.]),
            (2, [60., 340., 110., 70.]),
            (3, [1110., 340., 110., 70.]),
        ] {
            let corners = [[0., 0.], [1., 0.], [0., 1.], [1., 1.]].map(|uv| crop(id, rect, uv));
            for [u, v] in corners {
                assert!(
                    (0. ..=1.).contains(&u) && (0. ..=1.).contains(&v),
                    "{id} {u} {v}"
                );
            }
            let w = (corners[0][0] - corners[1][0]).abs() * SIZE[0] as f32;
            let h = (corners[2][1] - corners[0][1]).abs() * SIZE[1] as f32;
            assert!((w / h - rect[2] / rect[3]).abs() < 1e-3, "{id}");
        }
        let centre = |id, rect| crop(id, rect, [0.5, 0.5])[0];
        assert!(centre(2, [60., 340., 110., 70.]) > 0.5);
        assert!(centre(3, [1110., 340., 110., 70.]) < 0.5);
        assert_eq!(centre(1, [500., 20., 280., 50.]), 0.5);
        // The shader samples the same crops from a single rear texture.
        let shader = include_str!("cockpit.wgsl");
        assert!(shader.contains("array<f32,3>(.5,.8,.2)"));
        assert_eq!(shader.matches("textureSampleLevel(rear,").count(), 1);
    }
    #[test]
    fn rear_camera_tracks_body_through_bank_and_vertical_attitudes() {
        for (yaw, pitch, bank) in [
            (0., 0., 0.),
            (1., 0.5, 1.),
            (2., std::f64::consts::FRAC_PI_2, 2.),
            (3., 2., -1.),
        ] {
            let body = Basis::new(yaw, pitch, bank);
            let [y, p, b] = rear_basis(body).angles();
            let rear = Basis::new(y, p, b);
            assert!(crate::attitude::dot(rear.forward, body.forward) < -0.999999);
            assert!(crate::attitude::dot(rear.up, body.up) > 0.999999);
        }
    }
    #[test]
    fn flood_preserves_outlines_and_rejects_leaks_and_overlaps() {
        let mut s = Sprite {
            width: 30,
            height: 12,
            rgba: vec![0; 30 * 12 * 4],
            glyphs: vec![],
        };
        for x0 in [1, 11, 21] {
            for y in 2..5 {
                for x in x0..x0 + 4 {
                    s.rgba[(y * 30 + x) * 4..(y * 30 + x) * 4 + 4]
                        .copy_from_slice(&[10, 20, 30, 255]);
                }
            }
        }
        let all = |a: [usize; 2], b: [usize; 2], c: [usize; 2]| [Some(a), Some(b), Some(c)];
        let m = flood(&s, all([2, 3], [12, 3], [22, 3])).unwrap();
        assert_eq!(m.rects[0], [1., 2., 4., 3.]);
        assert_eq!(m.pixels.iter().filter(|v| **v != 0).count(), 36);
        assert_eq!(m.pixels[0], 0);
        assert!(flood(&s, all([2, 3], [2, 3], [22, 3])).is_none());
        assert!(flood(&s, all([0, 0], [12, 3], [22, 3])).is_none());
        // A side-only picture leaves the centre slot empty and keeps the
        // left and right slots' own numbers.
        let sides = flood(&s, [None, Some([12, 3]), Some([22, 3])]).unwrap();
        assert_eq!(sides.rects[0], [0.; 4]);
        assert_eq!(sides.rects[1], [11., 2., 4., 3.]);
        assert_eq!(sides.rects[2], [21., 2., 4., 3.]);
        assert_eq!(sides.pixels[3 * 30 + 12], 2);
        assert_eq!(sides.pixels[3 * 30 + 22], 3);
        assert_eq!(sides.pixels[3 * 30 + 2], 0);
        s.rgba.fill(255);
        assert!(flood(&s, all([2, 3], [12, 3], [22, 3])).is_none());
    }
    #[test]
    fn mirror_seeds_follow_the_cockpit_picture_family() {
        use AircraftId::*;
        let family = |ids: &[AircraftId]| {
            let first = seeds(ids[0]);
            assert!(first.is_some(), "{:?}", ids[0]);
            for id in ids {
                assert_eq!(seeds(*id), first, "{id:?}");
            }
        };
        family(&[A4E, F4B, F4J, F4E, F4G]);
        family(&[Su27, Av8, F15]);
        family(&[Mig29, Su25, Mig23, Mi24]);
        family(&[F104, A10]);
        for id in [F18, F14, Rafale, Su35, A7, Yak141, Gripen, F5E, Tornado] {
            assert!(seeds(id).is_some(), "{id:?}");
        }
        // The F-16, F-22, X-31, MiG-17, MiG-21 and AC-130 pictures have no
        // flat mirror fill.
        for id in [
            F16C, F16A, F22, F22n, Faxx, X31, Mig17, Mig21, C130, Ac130, E3, Il76, E2, V22, Ah64,
            Ch47, B747, A310,
        ] {
            assert!(seeds(id).is_none(), "{id:?}");
        }
        // Every table: seeds are inside the 1280x490 picture and distinct, and
        // a picture with side mirrors names both sides.
        for id in AircraftId::ALL {
            let Some(table) = seeds(id) else { continue };
            assert_eq!(table[1].is_some(), table[2].is_some(), "{id:?}");
            let mut points: Vec<_> = table.iter().flatten().collect();
            assert!(points.iter().all(|p| p[0] < 1280 && p[1] < 490));
            points.sort();
            points.dedup();
            assert_eq!(points.len(), table.iter().flatten().count());
        }
    }
    /// GRI-7: the Gripen's own 1279x490 picture takes its three flat mirror
    /// fills; a picture of any other width does not.
    #[test]
    fn the_gripen_picture_is_1279_wide_and_floods_three_mirrors() {
        let (w, h) = (1279, 490);
        let mut s = Sprite {
            width: w,
            height: h,
            rgba: vec![0; w * h * 4],
            glyphs: vec![],
        };
        let fills = [(600, 30, 80, 30), (200, 280, 90, 60), (990, 280, 90, 60)];
        for (x0, y0, fw, fh) in fills {
            for y in y0..y0 + fh {
                for x in x0..x0 + fw {
                    s.rgba[(y * w + x) * 4..(y * w + x) * 4 + 4]
                        .copy_from_slice(&[117, 138, 154, 255]);
                }
            }
        }
        let m = masks(&s, AircraftId::Gripen).expect("three mirrors");
        assert_eq!(m.rects[0], [600., 30., 80., 30.]);
        assert_eq!(m.rects[1], [200., 280., 90., 60.]);
        assert_eq!(m.rects[2], [990., 280., 90., 60.]);
        let wrong = Sprite {
            width: 1278,
            height: h,
            rgba: vec![0; 1278 * h * 4],
            glyphs: vec![],
        };
        assert!(masks(&wrong, AircraftId::Gripen).is_none());
    }

    /// TOR-7: the Tornado's Eurofighter picture takes three flat mirror fills
    /// and the flood stays inside each one.
    #[test]
    fn the_tornado_picture_floods_three_eurofighter_mirrors() {
        let (w, h) = (1280, 490);
        let mut s = Sprite {
            width: w,
            height: h,
            rgba: vec![0; w * h * 4],
            glyphs: vec![],
        };
        let fills = [
            (479, 11, 318, 77),
            (125, 183, 152, 261),
            (999, 183, 152, 261),
        ];
        for (x0, y0, fw, fh) in fills {
            for y in y0..y0 + fh {
                for x in x0..x0 + fw {
                    s.rgba[(y * w + x) * 4..(y * w + x) * 4 + 4]
                        .copy_from_slice(&[105, 113, 134, 255]);
                }
            }
        }
        let m = masks(&s, AircraftId::Tornado).expect("three mirrors");
        assert_eq!(m.rects[0], [479., 11., 318., 77.]);
        assert_eq!(m.rects[1], [125., 183., 152., 261.]);
        assert_eq!(m.rects[2], [999., 183., 152., 261.]);
    }

    /// Retail-reading (skips without an install): the three seeds land on the
    /// three mirror fills of the real `~EF2H.PIC`, and the left and right
    /// mirrors match the sizes of the `~EF2_LH` and `~EF2_RH` overlays.
    #[test]
    fn the_tornado_seeds_hit_the_imported_eurofighter_mirrors() {
        let root = std::env::var_os("TORE_GAME_DIR").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../gameassets/fighters-anthology")
            },
            std::path::PathBuf::from,
        );
        let Ok(media) = tore_import::MediaSource::detect(&root) else {
            eprintln!("skipped: no retail install at {}", root.display());
            return;
        };
        let Ok(library) = media.archive("FA_1.LIB") else {
            eprintln!("skipped: no retail FA_1.LIB");
            return;
        };
        let pic = tore_formats::Pic::parse(&library.read("~EF2H.PIC").unwrap()).unwrap();
        let source = Sprite {
            width: pic.width,
            height: pic.height,
            rgba: pic.rgba(&[[0; 3]; 256]),
            glyphs: vec![],
        };
        let m = masks(&source, AircraftId::Tornado).expect("three mirrors");
        assert_eq!(m.rects[0], [479., 11., 319., 78.]);
        assert_eq!(m.rects[1], [125., 183., 153., 262.]);
        assert_eq!(m.rects[2], [999., 183., 153., 262.]);
        for (name, rect) in [("~EF2_LH.PIC", m.rects[1]), ("~EF2_RH.PIC", m.rects[2])] {
            let overlay = tore_formats::Pic::parse(&library.read(name).unwrap()).unwrap();
            assert_eq!(
                [overlay.width as f32, overlay.height as f32],
                [rect[2], rect[3]]
            );
        }
    }

    /// B2-7, F117-7, B52-7, B1B-7, T160-7, F111-7, T26-7 and T95-7: only the B-2,
    /// the F-117, the B-52, the B-1B, the Tu-160, the F-111F, the Tu-26 and the Tu-95
    /// have a seat of their own; every other aircraft, and an unknown type, keeps
    /// the shared one (7 ft up, 10 ft ahead).
    #[test]
    fn only_the_bombers_with_a_drawn_flight_deck_have_their_own_pilot_eye() {
        for id in AircraftId::ALL {
            let want = match id {
                AircraftId::B2 => [10., 20.],
                AircraftId::F117 => [6., 14.],
                AircraftId::B52 => [9., 63.],
                AircraftId::B1b => [11.3, 66.],
                AircraftId::Tu160 => [12., 71.],
                AircraftId::F111 => [5., 25.],
                AircraftId::Tu26 => [7., 56.],
                AircraftId::Tu95 => [5., 53.],
                _ => [7., 10.],
            };
            assert_eq!(eye_offset(Some(id)), want, "{id:?}");
        }
        assert_eq!(eye_offset(None), [7., 10.]);
        // Level, heading north (yaw 0): the seat is `up` above and `forward`
        // ahead of the origin.
        let level = Basis::new(0., 0., 0.);
        let at = |id| pilot_eye_of(id, [100., 200., 300.], level);
        let shared = at(None);
        assert_eq!(at(Some(AircraftId::F18)), shared);
        let b2 = at(Some(AircraftId::B2));
        let up = level.up;
        let forward = level.forward;
        for i in 0..3 {
            let moved = (b2[i] - shared[i]) - (up[i] * 3. + forward[i] * 10.);
            assert!(moved.abs() < 1e-9, "axis {i}");
        }
        // The F-117: 1 ft lower and 4 ft further forward than the shared seat.
        let f117 = at(Some(AircraftId::F117));
        for i in 0..3 {
            let moved = (f117[i] - shared[i]) - (-up[i] + forward[i] * 4.);
            assert!(moved.abs() < 1e-9, "axis {i}");
        }
        // The B-52: 2 ft higher and 53 ft further forward (the drawn flight
        // deck), clear of the 7.3 ft fuselage crown behind it.
        let b52 = at(Some(AircraftId::B52));
        for i in 0..3 {
            let moved = (b52[i] - shared[i]) - (up[i] * 2. + forward[i] * 53.);
            assert!(moved.abs() < 1e-9, "axis {i}");
        }
        // The B-1B: 4.3 ft higher and 56 ft further forward (over the drawn
        // flight deck, whose crown peaks 10.7 ft up).
        let b1b = at(Some(AircraftId::B1b));
        for i in 0..3 {
            let moved = (b1b[i] - shared[i]) - (up[i] * 4.3 + forward[i] * 56.);
            assert!(moved.abs() < 1e-9, "axis {i}");
        }
        // The Tu-160: 5 ft higher and 61 ft further forward (the drawn flight
        // deck behind the windscreen).
        let tu160 = at(Some(AircraftId::Tu160));
        for i in 0..3 {
            let moved = (tu160[i] - shared[i]) - (up[i] * 5. + forward[i] * 61.);
            assert!(moved.abs() < 1e-9, "axis {i}");
        }
        // The F-111F: 2 ft lower and 15 ft further forward (the drawn
        // cockpit), 1.7 ft over its 3.3 ft crown.
        let f111 = at(Some(AircraftId::F111));
        for i in 0..3 {
            let moved = (f111[i] - shared[i]) - (-up[i] * 2. + forward[i] * 15.);
            assert!(moved.abs() < 1e-9, "axis {i}");
        }
        // The Tu-26: the shared height and 46 ft further forward (the drawn
        // glasshouse's middle), 1.7 ft over the 5.33 ft crown behind it.
        let tu26 = at(Some(AircraftId::Tu26));
        for i in 0..3 {
            let moved = (tu26[i] - shared[i]) - forward[i] * 46.;
            assert!(moved.abs() < 1e-9, "axis {i}");
        }
        // The Tu-95 (T95-7): 2 ft lower and 43 ft further forward (the drawn
        // flight deck), 1.7 ft over its 3.3 ft roof.
        let tu95 = at(Some(AircraftId::Tu95));
        for i in 0..3 {
            let moved = (tu95[i] - shared[i]) - (-up[i] * 2. + forward[i] * 43.);
            assert!(moved.abs() < 1e-9, "axis {i}");
        }
    }
}
