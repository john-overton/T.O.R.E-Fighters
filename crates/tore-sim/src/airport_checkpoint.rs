//! The coders of the player's tower service: the health it mirrors, the
//! destroyed objects, the selected airport, the clearance and the last reply.
//!
//! Stage H slice H2 (world shell). A cockpit's `Service` is coded whole and
//! restored as a new value: it was built from the airport scene, but its
//! health map and the rest are state, and the scene is not at hand to a
//! `Loader`.

use super::{Clearance, DeclineReason, Reply, Service};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

crate::checkpoint_enum!(DeclineReason {
    NoAirport = 0,
    NoRunway = 1,
    Hostile = 2,
    NeutralPermission = 3,
    UnknownAllegiance = 4,
    RunwayDisabled = 5,
});

crate::checkpoint_struct!(Clearance {
    airport,
    runway,
    end
});

impl Checkpoint for Reply {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        // Every variant, no catch-all, so a new one fails to compile here.
        match self {
            Reply::Selected { airport } => {
                s.writer().write_varint(0);
                airport.save(s, None)
            }
            Reply::Cleared {
                airport,
                runway,
                end,
            } => {
                s.writer().write_varint(1);
                airport.save(s, None)?;
                runway.save(s, None)?;
                end.save(s, None)
            }
            Reply::Landed { airport, runway } => {
                s.writer().write_varint(2);
                airport.save(s, None)?;
                runway.save(s, None)
            }
            Reply::Declined { airport, reason } => {
                s.writer().write_varint(3);
                airport.save(s, None)?;
                reason.save(s, None)
            }
            Reply::Repeated(inner) => {
                s.writer().write_varint(4);
                inner.save(s, None)
            }
            Reply::Cancelled { airport } => {
                s.writer().write_varint(5);
                airport.save(s, None)
            }
        }
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        load_reply(l, 0)
    }
}

/// How many `Repeated` wrappers a coded reply may nest. The service wraps a
/// reply once; the limit only keeps damaged bytes from recursing deeply.
const MAX_REPEATS: u32 = 4;

fn load_reply(l: &mut Loader<'_>, depth: u32) -> Result<Reply, CheckpointError> {
    match l.reader().read_varint()? {
        0 => Ok(Reply::Selected {
            airport: Checkpoint::load(l, None)?,
        }),
        1 => Ok(Reply::Cleared {
            airport: Checkpoint::load(l, None)?,
            runway: Checkpoint::load(l, None)?,
            end: Checkpoint::load(l, None)?,
        }),
        2 => Ok(Reply::Landed {
            airport: Checkpoint::load(l, None)?,
            runway: Checkpoint::load(l, None)?,
        }),
        3 => Ok(Reply::Declined {
            airport: Checkpoint::load(l, None)?,
            reason: Checkpoint::load(l, None)?,
        }),
        4 if depth < MAX_REPEATS => Ok(Reply::Repeated(Box::new(load_reply(l, depth + 1)?))),
        4 => invalid("an airport reply repeats too deeply"),
        5 => Ok(Reply::Cancelled {
            airport: Checkpoint::load(l, None)?,
        }),
        other => invalid(format!("an airport reply has no variant {other}")),
    }
}

crate::checkpoint_struct!(Service {
    health,
    destroyed,
    selected,
    clearance,
    last_reply,
    landing_ticks,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::airport::{
        Aircraft, Airport, Allegiance, ApproachEnd, Command, Event, LANDING_TICKS, OrientedBox,
        Runway, Scene, SourceKey, StaticObject,
    };
    use crate::checkpoint::{Models, from_bytes, round_trip, to_bytes};

    fn object(id: u32, runway: bool, bounds: OrientedBox) -> StaticObject {
        StaticObject {
            id,
            source: SourceKey {
                layout: "X.MM".into(),
                ordinal: id,
            },
            name: format!("Object {id}"),
            object_type: "STRIP.OT".into(),
            bounds,
            hit_points: 100,
            category: 0x100,
            radar_signature: 100.,
            infrared_signature: 100.,
            runway,
        }
    }

    /// One long runway and one other object, at one friendly airport.
    fn scene() -> Scene {
        let bounds = OrientedBox {
            center: [0., 100., 0.],
            half: [100., 10., 5000.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        };
        let hangar = OrientedBox {
            center: [900., 100., 0.],
            half: [20., 10., 20.],
            ..bounds
        };
        Scene {
            objects: vec![object(1000, true, bounds), object(2000, false, hangar)],
            runways: vec![Runway {
                object: 1000,
                airport: 7,
                name: "09/27".into(),
                surface: bounds,
                approach_center: bounds.center,
                elevation_ft: 100.,
                heading: 0.,
                length_ft: 10000.,
            }],
            airports: vec![Airport {
                id: 7,
                name: "Field".into(),
                runway_objects: vec![1000],
                allegiance: Allegiance::Friendly,
                neutral_permission: false,
            }],
        }
    }

    fn plane(z: f64, alt: f64, supported: bool) -> Aircraft {
        Aircraft {
            position: [0., alt, z],
            forward: [0., 0., 1.],
            nav_mode: true,
            gear_down: true,
            supported,
            alive: true,
            speed_fps: 20.,
            ground_clearance_ft: 0.,
            redfor: false,
        }
    }

    fn assert_round_trips(service: &Service, what: &str) -> Service {
        let models = Models::default();
        let copy = round_trip(service, &models).unwrap();
        assert_eq!(&copy, service, "{what}");
        copy
    }

    /// A conversation with the tower, stopped part way through the landing
    /// count, restores and finishes the landing with the same events.
    #[test]
    fn a_service_mid_landing_restores_and_finishes_the_landing_identically() {
        let scene = scene();
        let mut service = Service::new(&scene).unwrap();
        let air = plane(-10_000., 1000., false);
        let ground = plane(0., 100., true);
        assert_round_trips(&service, "fresh");
        service.command(&scene, air, Command::SelectAirport(7));
        assert_round_trips(&service, "selected");
        service.command(&scene, air, Command::RequestLanding);
        assert_eq!(service.clearance(), Some((7, 1000, ApproachEnd::Near)));
        service.damage(2000, 40);
        assert_round_trips(&service, "cleared, a hangar hurt");
        for _ in 0..100 {
            service.step(&scene, ground);
        }
        assert_eq!(service.landing_ticks, 100);
        let mut copy = assert_round_trips(&service, "mid landing");
        let mut landed = false;
        for tick in 0..u32::from(LANDING_TICKS) {
            let (a, b) = (service.step(&scene, ground), copy.step(&scene, ground));
            assert_eq!(a, b, "tick {tick}");
            landed |= a.iter().any(|e| matches!(e, Event::LandingComplete { .. }));
            assert_eq!(service, copy, "tick {tick}");
        }
        assert!(landed, "the landing completed");
        assert_round_trips(&service, "landed");
    }

    /// Destroyed objects, a clearance that went with the runway and every
    /// kind of reply code back.
    #[test]
    fn destroyed_objects_and_every_reply_round_trip() {
        let scene = scene();
        let mut service = Service::new(&scene).unwrap();
        let air = plane(-10_000., 1000., false);
        service.command(&scene, air, Command::SelectAirport(7));
        service.command(&scene, air, Command::RequestLanding);
        service.damage(2000, 1000);
        assert!(service.destroyed.contains(&2000));
        assert_round_trips(&service, "an object destroyed");
        service.damage(1000, 1000);
        assert_eq!(service.clearance(), None);
        assert_round_trips(&service, "the runway gone with the clearance");

        let replies = [
            Reply::Selected { airport: 7 },
            Reply::Cleared {
                airport: 7,
                runway: 1000,
                end: ApproachEnd::Far,
            },
            Reply::Landed {
                airport: 7,
                runway: 1000,
            },
            Reply::Declined {
                airport: None,
                reason: DeclineReason::NeutralPermission,
            },
            Reply::Declined {
                airport: Some(7),
                reason: DeclineReason::RunwayDisabled,
            },
            Reply::Cancelled { airport: Some(7) },
            Reply::Cancelled { airport: None },
            Reply::Repeated(Box::new(Reply::Landed {
                airport: 1,
                runway: 2,
            })),
        ];
        for reply in replies {
            service.last_reply = Some(reply.clone());
            assert_round_trips(&service, &format!("{reply:?}"));
        }
        for reason in [
            DeclineReason::NoAirport,
            DeclineReason::NoRunway,
            DeclineReason::Hostile,
            DeclineReason::NeutralPermission,
            DeclineReason::UnknownAllegiance,
            DeclineReason::RunwayDisabled,
        ] {
            service.last_reply = Some(Reply::Declined {
                airport: Some(3),
                reason,
            });
            assert_round_trips(&service, &format!("{reason:?}"));
        }
    }

    #[test]
    fn damaged_service_bytes_are_refused_without_a_panic() {
        let scene = scene();
        let mut service = Service::new(&scene).unwrap();
        let air = plane(-10_000., 1000., false);
        service.command(&scene, air, Command::SelectAirport(7));
        service.command(&scene, air, Command::RequestLanding);
        service.damage(2000, 1000);
        service.last_reply = Some(Reply::Repeated(Box::new(Reply::Selected { airport: 7 })));
        let models = Models::default();
        let coded = to_bytes(&service, &models).unwrap();
        for cut in 0..coded.body.len() {
            let mut shorter = coded.clone();
            shorter.body.truncate(cut);
            let _ = from_bytes::<Service>(&shorter, &models);
        }
        for bit in 0..coded.body.len() * 8 {
            let mut flipped = coded.clone();
            flipped.body[bit / 8] ^= 1 << (bit % 8);
            let _ = from_bytes::<Service>(&flipped, &models);
        }
        // Replies nested without end are refused, not followed.
        let mut deep = tore_codec::BitWriter::new();
        for _ in 0..64 {
            deep.write_varint(4);
        }
        let bytes = deep.finish();
        let mut l = Loader::new(&bytes, &[], &models);
        assert!(Reply::load(&mut l, None).is_err());
    }
}
