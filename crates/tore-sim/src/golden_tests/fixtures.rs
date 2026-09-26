//! Private synthetic fixtures for the golden tests. They copy the shape of the
//! crate's shared test fixtures on purpose: editing another test's fixture
//! must never move a recorded fingerprint. No retail data is involved.

use std::collections::BTreeMap;

use tore_formats::aircraft::{Aircraft, AircraftId, Envelope, Token};

use crate::research::Surface;
use crate::sensors;

fn token(kind: &str, value: i64) -> Token {
    Token {
        kind: kind.into(),
        value: value.to_string(),
        scaled: false,
    }
}

/// The synthetic record the flight tests use ("Synthetic", "TEST.SH"), which
/// test builds fly with the F/A-18D model.
pub(super) fn synthetic_aircraft() -> Aircraft {
    let mut fields: BTreeMap<String, Token> = [
        ("weight", 10000),
        ("internalFuel", 1000),
        ("thrust", 8000),
        ("aftThrust", 12000),
        ("fuelConsumption", 2),
        ("aftFuelConsumption", 10),
        ("maxTakeoffWeight", 15000),
        ("loadedElevator", 40),
        ("loadedDrag", 0),
        ("_gpullDrag", 0),
        ("flapsLift", 51),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), token("dword", value)))
    .collect();
    for (key, value) in [("gearDrag", 23), ("flapsDrag", 70), ("airBrakesDrag", 256)] {
        fields.insert(key.into(), token("word", value));
    }
    for prefix in ["_brv.x", "puffRot.x", "puffRot.y", "puffRot.z"] {
        for (suffix, value) in [("min", -90), ("max", 90), ("acc", 200), ("dacc", 400)] {
            fields.insert(format!("{prefix}.{suffix}"), token("word", value));
        }
    }
    for key in [
        "turbulencePercent",
        "rudderDrag",
        "bayDrag",
        "wheelBrakesDrag",
        "stallWarningDelay",
        "stallDelay",
        "stallSeverity",
        "stallPitchDown",
        "spinEntry",
        "spinYawLow",
        "spinYawHigh",
        "spinAOALow",
        "spinAOAHigh",
        "spinBankLow",
        "spinBankHigh",
        "flags",
    ] {
        fields.insert(key.into(), token("word", 0));
    }
    for axis in ["x", "y", "z"] {
        for suffix in ["min", "max", "acc", "dacc"] {
            let value = if suffix == "min" { -100 } else { 100 };
            fields.insert(format!("_bv.{axis}.{suffix}"), token("word", value));
        }
    }
    for (key, value) in [
        ("_brv.x.max", 100),
        ("crashSpeedForward", 330),
        ("crashSpeedSide", 50),
        ("crashSpeedVertical", 30),
        ("crashPitch", 25),
        ("crashRoll", 10),
        ("spinExit", -2),
    ] {
        fields.insert(key.into(), token("word", value));
    }
    Aircraft {
        id: AircraftId::F18,
        name: "Synthetic".into(),
        shape: "TEST.SH".into(),
        fields,
        object: BTreeMap::new(),
        hardpoints: vec![],
        sounds: BTreeMap::new(),
        envelopes: (-2..=6)
            .map(|g| Envelope {
                g,
                points: vec![[200., 0.], [250., 50000.], [1300., 50000.], [1800., 0.]],
            })
            .collect(),
    }
}

/// The synthetic record under one aircraft's exact identity, with distinct
/// synthetic capabilities: afterburner thrust and envelope speeds vary by
/// aircraft, the A-4E and Su-25 carry no afterburner, and every one has an
/// ejection seat and a spin rate. Never presented as measured retail data.
pub(super) fn aircraft(id: AircraftId) -> Aircraft {
    let index = AircraftId::SELECTABLE
        .iter()
        .position(|candidate| *candidate == id)
        .expect("selectable aircraft");
    let mut a = synthetic_aircraft();
    a.id = id;
    a.name = match id {
        AircraftId::F18 => "F/A-18D",
        AircraftId::Rafale => "RAFALE",
        AircraftId::F14 => "F-14",
        AircraftId::A4E => "A-4E",
        AircraftId::X31 => "X-31",
        AircraftId::Mig29 => "MiG-29",
        AircraftId::Su27 => "Su-27",
        AircraftId::Mig21 => "MiG-21",
        AircraftId::Su25 => "Su-25",
        AircraftId::Mig23 => "MiG-23",
        AircraftId::Su35 => "Su-35",
        AircraftId::F22 | AircraftId::F22n | AircraftId::Faxx => "F-22",
    }
    .into();
    a.shape = format!("{}.SH", id.stem());
    let afterburner = if matches!(id, AircraftId::A4E | AircraftId::Su25) {
        0
    } else {
        400 + index as i64 * 10
    };
    a.fields
        .insert("aftThrust".into(), token("dword", afterburner));
    a.fields.insert("flags".into(), token("word", 0x10));
    a.fields.insert("spinYawLow".into(), token("word", 40));
    a.fields
        .insert("spinYawHigh".into(), token("word", 120 + index as i64 * 5));
    for envelope in &mut a.envelopes {
        for point in &mut envelope.points {
            point[0] *= 0.85 + index as f64 * 0.025;
        }
    }
    a
}

/// Every synthetic configuration the flight goldens fly: the shared synthetic
/// record, then each selectable aircraft identity.
pub(super) fn configurations() -> Vec<(String, Aircraft)> {
    std::iter::once(("Synthetic".to_string(), synthetic_aircraft()))
        .chain(
            AircraftId::SELECTABLE
                .into_iter()
                .map(|id| (id.label().to_string(), aircraft(id))),
        )
        .collect()
}

/// The synthetic record with the extra fields the restricted native adapter
/// joins, copied from the native adapter's own synthetic fixture.
pub(super) fn native_aircraft() -> Aircraft {
    let mut a = synthetic_aircraft();
    for (key, value) in [
        ("vtLimitDown", 0),
        ("structure[0]", 100),
        ("structure[1]", 100),
        ("envMin", -2),
        ("envMax", 6),
        ("maxAlt", 60_000 * 256),
        ("_minSpeed", 100),
        ("coefDrag", 256),
        ("loadedGpullDrag", 0),
        ("loadedAileron", 0),
        ("flapsLift", 0),
        ("gpullAOA", 0),
        ("lowAOASpeed", 100),
        ("lowAOAPitch", 5),
        ("rudderSlip", 4),
        ("rudderBank", 5),
    ] {
        a.fields.insert(key.into(), token("dword", value));
    }
    for axis in [
        "_brv.x",
        "_brv.y",
        "_brv.z",
        "rudderYaw",
        "puffRot.x",
        "puffRot.y",
        "puffRot.z",
    ] {
        for (suffix, value) in [("min", -100), ("max", 100), ("acc", 100), ("dacc", 100)] {
            a.fields
                .insert(format!("{axis}.{suffix}"), token("dword", value));
        }
    }
    a
}

/// Synthetic sine and arctangent tables for the native adapter.
pub(super) fn native_tables() -> std::sync::Arc<crate::native::Tables> {
    let sine: Vec<u8> = (0..321)
        .flat_map(|i| {
            ((f64::from(i) * std::f64::consts::TAU / 256.)
                .sin()
                .mul_add(32767., 0.)
                .round() as i16)
                .to_le_bytes()
        })
        .collect();
    let atan: Vec<u8> = (0..514)
        .flat_map(|i| {
            (((f64::from(i) / 512.).atan() / std::f64::consts::TAU * 65536.).round() as u16)
                .to_le_bytes()
        })
        .collect();
    std::sync::Arc::new(crate::native::Tables::parse(&sine, &atan).expect("synthetic tables"))
}

/// Rolling terrain: a flat plain at sea level with one ridge running along
/// the X axis near Z = 60,000 ft and one isolated peak. Piecewise linear, so
/// the terrain itself never depends on a maths library.
pub(super) fn terrain(x: f64, z: f64) -> f64 {
    let ridge = (4_000. - (z - 60_000.).abs() * 0.4).max(0.);
    let peak = (6_000. - ((x + 20_000.).abs() + (z - 20_000.).abs()) * 0.5).max(0.);
    ridge.max(peak)
}

/// One 200 ft wide, 8,000 ft long runway along the Z axis at the origin, on
/// sea-level ground.
pub(super) fn runway_surface(x: f64, z: f64) -> Surface {
    if x.abs() <= 100. && z.abs() <= 4_000. {
        Surface::runway(0.)
    } else {
        Surface::terrain(0.)
    }
}

/// Synthetic 20 nm radar and 5 nm visual channels for AI actors.
pub(super) fn sensor_profiles(aircraft: AircraftId) -> sensors::SensorProfiles {
    let volume = |nmi: f64| sensors::Volume {
        azimuth_rad: 60_f64.to_radians(),
        elevation_rad: 60_f64.to_radians(),
        minimum_ft: 0.,
        maximum_ft: nmi * sensors::FEET_PER_NAUTICAL_MILE,
        minimum_relative_ft: f64::NEG_INFINITY,
        maximum_relative_ft: f64::INFINITY,
    };
    let preset = sensors::Preset::Advanced;
    sensors::SensorProfiles {
        aircraft,
        radar: Some(sensors::RadarProfile {
            record: "GOLDEN.SEE".into(),
            search: volume(20.),
            track: volume(20.),
            look_down: 0.,
            preset,
            notch: preset.notch(),
            resistance: preset.resistance(),
            band: 0,
            source_flags: [0; 2],
            source_doppler: [0; 3],
        }),
        infrared: None,
        visual: Some(sensors::profile::VisualProfile {
            record: "GOLDEN.VIS".into(),
            search: volume(5.),
            track: volume(3.),
        }),
        jammer: None,
        signature: sensors::SignatureProfile::default(),
    }
}
