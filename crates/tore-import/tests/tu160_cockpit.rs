//! T160-6 and T160-7: the Tu-160 keeps the record's sensor rows (the radar and
//! the jammer are its own, `SU37L.SEE` is a fixed laser row with no pod), flies
//! the HUD its record names (`b2.HUD`, the B-2's stem and art), and sits its
//! pilot at the drawn flight deck. This reads `TU160.PT`, `b2.HUD`, the `~B2*`
//! pictures and `TU160.SH` from the user's install and pins what the slice's
//! notes quote: the six rows, the HUD and art bindings, the art size the glass
//! polygon is keyed to, the record's copied eye, and the shape's crown and
//! windscreen the fitted seat sits behind and over. It skips quietly without a
//! retail install and reads data only; no retail bytes are committed.
//! Evidence: docs/formats/aircraft-variety.md, "Tu-160 ordnance, sensors, HUD
//! and cockpit (T160-6/7, 2026-11-16)".
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
    hud::Hud,
    shape::Shape,
};
use tore_import::MediaSource;

const FEET_PER_UNIT: f64 = 8. / 3.;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn the_tu160_keeps_its_sensor_rows_and_flies_the_b2_hud_and_art() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let (Ok(fa1), Ok(fa2)) = (media.archive("FA_1.LIB"), media.archive("FA_2.LIB")) else {
        eprintln!("skipped: no retail FA_1.LIB and FA_2.LIB");
        return;
    };
    let read = |name: &str| {
        [&fa1, &fa2]
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap_or_else(|| panic!("{name} is in neither archive"))
            .read(name)
            .unwrap()
    };
    let id = AircraftId::Tu160;
    let record = Aircraft::parse(&read(id.pt())).unwrap();
    // The sensors: a visual row, the radar, the jammer and the laser. All four
    // are fixed rows (no station, no pod body); only the jammer's carries an
    // offset (z -50 record units, a position the host never draws).
    let sensors: Vec<_> = record
        .hardpoints
        .iter()
        .filter_map(|h| h.store.clone())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect();
    assert_eq!(
        sensors,
        ["VIS300.SEE", "TU160R.SEE", "TU160.ECM", "SU37L.SEE"]
    );
    for row in &record.hardpoints[..4] {
        let want = if row.store.as_deref() == Some("TU160.ECM") {
            [0, 0, -50]
        } else {
            [0, 0, 0]
        };
        assert_eq!(row.position, want, "{:?}", row.store);
    }
    assert_eq!(id.radar(), Some("TU160R.SEE"));
    // The record's own HUD module and the B-2's stem and art.
    assert_eq!(id.hud(), "B2.HUD");
    assert_eq!(id.cockpit_stem(), "B2");
    assert_eq!(id.cockpit(), "~B2H.PIC");
    assert_eq!(id.instrument_panel(), "~B2_P.PIC");
    assert!(id.cockpit_overlays().iter().all(Option::is_none));
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~B2_P.PIC"));
    // No Tu-160 art exists: nothing in either archive is named for it.
    for library in [&fa1, &fa2] {
        for name in library.entries.keys() {
            let upper = name.to_ascii_uppercase();
            assert!(
                !(upper.starts_with("~TU16") || upper.starts_with("~TU-160")),
                "{name}"
            );
        }
    }
    let art = tore_formats::Pic::parse(&read(id.cockpit())).unwrap();
    assert_eq!((art.width, art.height), (1394, 508));
    // The record's eye is the Tu-26's copy (0, 35, 60): 20 ft ahead at the
    // Tu-160's scale, 52 ft behind the drawn glass. Pinned so a later change
    // to the record shows.
    let eye: Vec<_> = ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
        .iter()
        .map(|key| record.object[*key].value.clone())
        .collect();
    assert_eq!(eye, ["0", "35", "60"]);
}

/// The surface on the centre line (faces within 0.6 unit of it): the fuselage
/// crown is flat at 4 units (10.7 ft) from 17 to 27 units ahead (45 to 72 ft)
/// and falls from 27 to the nose tip at 38 (101 ft), the glass between 72 and
/// 80 ft. The fitted seat (`mirrors::eye_offset`, 12 ft up and 71 ft ahead) is
/// 1.3 ft over the crown behind the glass.
#[test]
fn the_fitted_seat_sits_over_the_crown_behind_the_windscreen() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let Ok(fa2) = media.archive("FA_2.LIB") else {
        eprintln!("skipped: no retail FA_2.LIB");
        return;
    };
    let shape = Shape::parse(&fa2.read("TU160.SH").unwrap()).unwrap();
    // Highest point of the centre-line skin at `y` units ahead: the faces'
    // edges crossing that plane near the centre line.
    let crown = |y: f64| -> Option<f64> {
        let mut top: Option<f64> = None;
        for face in &shape.faces {
            let p = &face.positions;
            for i in 0..p.len() {
                let (a, b) = (p[i].map(f64::from), p[(i + 1) % p.len()].map(f64::from));
                if (a[1] - y) * (b[1] - y) <= 0. && a[1] != b[1] {
                    let t = (y - a[1]) / (b[1] - a[1]);
                    let (x, z) = (a[0] + t * (b[0] - a[0]), a[2] + t * (b[2] - a[2]));
                    if x.abs() <= 0.6 {
                        top = Some(top.map_or(z, |t: f64| t.max(z)));
                    }
                }
            }
        }
        top
    };
    let feet = |units: f64| units * FEET_PER_UNIT;
    // Flat at 4 units from 17 to 27.
    for y in [17., 20., 23., 26., 27.] {
        assert_eq!(crown(y), Some(4.), "crown at {y}");
    }
    // The windscreen and nose cone fall away ahead of 27.
    assert_eq!(crown(30.), Some(2.));
    assert!(crown(34.).unwrap() < 2.);
    // The seat: 71 ft ahead, 12 ft up.
    let (seat_up, seat_ahead) = (12., 71.);
    assert!(seat_ahead < feet(27.), "the seat is behind the glass");
    assert!(seat_ahead > feet(23.), "the seat is on the flight deck");
    let over = seat_up - feet(crown(seat_ahead / FEET_PER_UNIT).unwrap());
    assert!((over - 1.33).abs() < 0.01, "{over} ft over the crown");
}
