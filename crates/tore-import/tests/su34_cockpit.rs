//! SU34-7: the Su-34 flies the Su-33 family's HUD and cockpit art by its record
//! (`su33cc.HUD`, stem `SU33`, `~SU33H.PIC`; coordinator first pass, Su-34
//! item 7, opinionated) and keeps the five sensor rows of its record, none of
//! them with a pod body. This reads `SU34.PT`, `SU33CC.HUD`, the `~SU33*`
//! pictures and `SU34.SH` from the user's install and pins the bindings, the
//! sensor rows and the drawn canopy the fitted pilot eye (`mirrors::eye_offset`)
//! stands over. It skips quietly without a retail install and reads data only;
//! no retail bytes are committed. Evidence: docs/formats/aircraft-variety.md,
//! "Su-34 ordnance, sensors, HUD and cockpit (SU34-6/7, 2026-11-16)".
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
    hud::Hud,
    shape::Shape,
};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

/// The Su-34's pilot eye in shape units (1/3 ft): 7 ft up, 17 ft ahead.
const EYE_UP: f32 = 21.;
const EYE_FORWARD: f32 = 51.;

#[test]
fn the_su34_flies_the_su33_family_with_the_flanker_sensor_rows_and_no_pod_bodies() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let (Ok(fa1), Ok(fa2)) = (media.archive("FA_1.LIB"), media.archive("FA_2.LIB")) else {
        eprintln!("skipped: no retail FA_1.LIB and FA_2.LIB");
        return;
    };
    let has = |name: &str| {
        [&fa1, &fa2]
            .iter()
            .any(|library| library.entries.contains_key(name))
    };
    let read = |name: &str| {
        [&fa1, &fa2]
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap_or_else(|| panic!("{name} is in neither archive"))
            .read(name)
            .unwrap()
    };
    let id = AircraftId::Su34;
    let record = Aircraft::parse(&read(id.pt())).unwrap();
    let sensors = record
        .hardpoints
        .iter()
        .filter_map(|h| h.store.clone())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect::<Vec<_>>();
    // Five fixed rows at the origin (B7: no pod body): the visual set, the
    // radar, the IRST, the jammer and the laser.
    assert_eq!(
        sensors,
        [
            "VIS300.SEE",
            "SU27R.SEE",
            "SU27I.SEE",
            "SU27.ECM",
            "SU37L.SEE"
        ]
    );
    assert_eq!(id.radar(), Some("SU27R.SEE"));
    // The record's own HUD module and the Su-33 family's art.
    assert_eq!(id.hud(), "SU33CC.HUD");
    assert_eq!(id.cockpit_stem(), "SU33");
    assert_eq!(id.stem(), "SU34");
    assert_eq!(id.cockpit(), "~SU33H.PIC");
    assert_eq!(id.instrument_panel(), "~SU33_P.PIC");
    // No Su-34 picture of its own exists (and so no `ICO2SU34.PIC`: the list
    // screen draws no per-aircraft picture).
    for name in ["~SU34.PIC", "~SU34H.PIC", "~SU34_P.PIC", "ICO2SU34.PIC"] {
        assert!(!has(name), "{name} appeared");
    }
    // `su33cc.HUD` references exactly the instrument window frame the identity
    // names.
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~SU33_P.PIC"));
    assert!(has(id.cockpit()) && has(&id.instrument_panel()));
    // The art TORE draws is the 1280 x 490 the glass aperture is keyed to.
    let art = tore_formats::Pic::parse(&read(id.cockpit())).unwrap();
    assert_eq!((art.width, art.height), (1280, 490));
    // The record's own eye offset is a record field no host code reads.
    let eye: Vec<_> = ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
        .iter()
        .map(|key| record.object[*key].value.clone())
        .collect();
    assert_eq!(eye, ["2", "20", "80"]);
}

/// The fitted eye (7 ft up, 17 ft ahead) is the middle of the drawn canopy and
/// clears every face of the skin there: the crown on the centre line is 17
/// units (5.7 ft) up at y 43, the windscreen apex 15 units at y 58, and no
/// face between them or along the spine behind reaches the eye's 21 units.
#[test]
fn the_su34_pilot_eye_stands_over_the_drawn_canopy() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let Ok(fa2) = media.archive("FA_2.LIB") else {
        eprintln!("skipped: no retail FA_2.LIB");
        return;
    };
    let shape = Shape::parse(&fa2.read("SU34.SH").unwrap()).unwrap();
    let points = shape.faces.iter().flat_map(|f| f.positions.iter());
    // The canopy: the highest point on the centre line is the crown.
    let (crown_y, crown_z) =
        points
            .clone()
            .filter(|p| p[0].abs() <= 1.)
            .fold((0., f32::NEG_INFINITY), |best, p| {
                if p[2] > best.1 && p[1] > 0. {
                    (p[1], p[2])
                } else {
                    best
                }
            });
    assert_eq!((crown_y, crown_z), (43., 17.));
    // The windscreen apex: the front edge of the glass, 15 units up.
    assert!(
        points.clone().any(|p| p[1] == 58. && p[2] == 15.),
        "no windscreen apex at y 58"
    );
    // The eye is between the crown and the apex, over the crown, and above
    // every skin point from the tail to 10 ft past the apex.
    assert!((crown_y..=58.).contains(&EYE_FORWARD));
    assert!(EYE_UP > crown_z);
    let top = points
        .filter(|p| (-100.0..=88.).contains(&p[1]) && p[0].abs() <= 12.)
        .map(|p| p[2])
        .fold(f32::NEG_INFINITY, f32::max);
    eprintln!("highest skin point within 12 units of the centre line: {top}");
    // The fins stand taller (z 35), 23 units out: outside the 12 unit strip.
    assert!(EYE_UP > top, "the eye is inside the skin ({top})");
}
