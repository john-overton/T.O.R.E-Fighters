//! Shared renderer-independent simulation.
pub use tore_sim::flight::*;

#[cfg(test)]
mod animation_tests {
    use super::*;
    use std::collections::BTreeMap;
    use tore_formats::aircraft::{Aircraft, Envelope, Token};
    fn base_profile() -> Aircraft {
        let fields = [
            ("weight", 10000),
            ("internalFuel", 1000),
            ("thrust", 8000),
            ("aftThrust", 12000),
            ("fuelConsumption", 2),
            ("aftFuelConsumption", 10),
            ("maxTakeoffWeight", 15000),
            ("gearDrag", 23),
            ("flapsDrag", 70),
            ("airBrakesDrag", 256),
            ("loadedElevator", 40),
            ("loadedDrag", 0),
            ("_gpullDrag", 0),
        ]
        .into_iter()
        .map(|(k, v)| {
            (
                k.into(),
                Token {
                    kind: "dword".into(),
                    value: v.to_string(),
                    scaled: false,
                },
            )
        })
        .collect();
        Aircraft {
            name: "F/A-18D".into(),
            shape: "F18.SH".into(),
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
    fn profile() -> Aircraft {
        let mut a = base_profile();
        for key in [
            "rudderDrag",
            "bayDrag",
            "wheelBrakesDrag",
            "stallWarningDelay",
            "stallDelay",
            "stallSeverity",
            "stallPitchDown",
            "spinEntry",
            "spinExit",
            "spinYawLow",
            "spinYawHigh",
            "spinAOALow",
            "spinAOAHigh",
            "spinBankLow",
            "spinBankHigh",
            "crashSpeedForward",
            "crashSpeedSide",
            "crashSpeedVertical",
            "crashPitch",
            "crashRoll",
            "flags",
        ] {
            a.fields.insert(
                key.into(),
                Token {
                    kind: "word".into(),
                    value: "0".into(),
                    scaled: false,
                },
            );
        }
        for key in ["gearDrag", "flapsDrag", "airBrakesDrag"] {
            a.fields.get_mut(key).unwrap().kind = "word".into();
        }
        for axis in ["x", "y", "z"] {
            for suffix in ["min", "max", "acc", "dacc"] {
                a.fields.insert(
                    format!("_bv.{axis}.{suffix}"),
                    Token {
                        kind: "word".into(),
                        value: if suffix == "min" { "-100" } else { "100" }.into(),
                        scaled: false,
                    },
                );
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
            a.fields.insert(
                key.into(),
                Token {
                    kind: "word".into(),
                    value: value.to_string(),
                    scaled: false,
                },
            );
        }
        a
    }
    #[test]
    fn animated_source_faces_preserve_deployed_endpoints_and_uvs() {
        use crate::aircraft_animation::animate;
        use tore_formats::shape::Face;
        let mut s = State::new(&profile(), [0., 5000., 0.]).unwrap();
        s.gear = 1.;
        s.brake = 1.;
        s.hook = 1.;
        s.exhaust = 1.;
        for address in [0x5059, 0x4a03, 0x4bfa, 0x4d33, 0x4f64, 0x5310] {
            let f = Face {
                address,
                positions: vec![[1., 2., 3.], [5., 2., 3.], [1., 6., 3.]],
                colors: vec![20; 3],
                uv: vec![[0., 0.], [1., 0.], [0., 1.]],
                texture: "SYNTHETIC".into(),
                subtype: 0x4c,
                normal: Some([0., 1., 0.]),
            };
            let open = animate(&f, &s).unwrap();
            assert_eq!(open.positions, f.positions);
            assert_eq!(open.uv, f.uv);
            let mut half = s.clone();
            half.gear = 0.5;
            half.brake = 0.5;
            half.hook = 0.5;
            half.exhaust = 0.5;
            let middle = animate(&f, &half).unwrap();
            assert_ne!(middle.positions, f.positions);
            assert_eq!(middle.uv, f.uv);
            let mut closed = s.clone();
            closed.gear = 0.;
            closed.brake = 0.;
            closed.hook = 0.;
            closed.exhaust = 0.;
            assert!(animate(&f, &closed).is_none());
        }
    }
    #[test]
    fn rudder_split_preserves_texture_attributes_and_fixed_fin() {
        use tore_formats::shape::Face;
        let f = Face {
            address: 0x5467,
            positions: vec![
                [8., -40., 4.],
                [8., -20., 4.],
                [18., -30., 24.],
                [18., -45., 24.],
            ],
            colors: vec![20; 4],
            uv: vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]],
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([1., 0., 0.]),
        };
        let mut s = State::new(&profile(), [0., 5000., 0.]).unwrap();
        s.rudder = 0.5;
        let positive = crate::aircraft_animation::rudder_faces(&f, &s);
        s.rudder = -0.5;
        let negative = crate::aircraft_animation::rudder_faces(&f, &s);
        assert_eq!(positive.len(), 2);
        assert_eq!(positive[0].positions, negative[0].positions);
        assert_ne!(positive[1].positions, negative[1].positions);
        for face in positive {
            assert_eq!(face.positions.len(), face.uv.len());
            assert_eq!(face.positions.len(), face.colors.len());
            assert!(face.uv.iter().flatten().all(|v| (0. ..=1.).contains(v)));
        }
    }
}
