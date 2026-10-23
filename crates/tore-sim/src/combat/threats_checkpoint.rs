//! The coders of a missile-warning service's tracked threats
//! (docs/formats/checkpoint.md). The service holds its receiver's id and one
//! stored record per missile it is tracking; `Receiver` and `MissileSnapshot`
//! are the caller's per-tick input and are not state.
//!
//! Every field is coded. `Stored::current_this_tick` is cleared at the start
//! of each `observe_with_visual` before anything reads it, so it is scratch,
//! but it costs one bit per tracked missile and a restored service then
//! compares equal as well.

use super::{EvidenceSource, GuidanceClass, Stored, ThreatRecord, ThreatService, VisualSample};

crate::checkpoint_enum!(EvidenceSource {
    ElectronicSupported = 0,
    ElectronicActive = 1,
    OwnLaunch = 2,
    Visual = 3,
});

crate::checkpoint_enum!(GuidanceClass {
    Radar = 0,
    Infrared = 1,
    Passive = 2,
});

crate::checkpoint_struct!(ThreatRecord {
    missile_id,
    source,
    observed_tick,
    bearing_deg,
    position,
    velocity,
    guidance_class,
    targeting_receiver,
    was_targeting_receiver,
    stale,
    radar_bearing_deg,
});

crate::checkpoint_struct!(VisualSample { tick, position });

crate::checkpoint_struct!(Stored {
    record,
    current_this_tick,
    last_report_tick,
    visual_sample,
    radar_source_position,
    lost_since_tick,
    incoming_tick,
});

crate::checkpoint_struct!(ThreatService {
    receiver_id,
    contacts,
});

#[cfg(test)]
mod tests {
    use super::super::{MissileSnapshot, Receiver};
    use super::*;
    use crate::ai::Experience;
    use crate::checkpoint::{Models, from_bytes, round_trip, to_bytes};
    use crate::combat::missiles::Guidance;

    fn receiver(tick: u64) -> Receiver {
        Receiver {
            id: 7,
            position: [0., 0., tick as f64],
            velocity: [0., 0., 1.],
            heading_deg: 10.,
            pitch_deg: 0.,
            skill: Experience::Ace,
            rwr_operating: tick % 400 < 300,
            visual_operating: true,
            visibility_limit_ft: tick.is_multiple_of(7).then_some(40_000.),
        }
    }

    /// Four missiles of every evidence kind: supported (a blinking radar
    /// warning), active, an infrared round seen by eye and the receiver's own
    /// launch; each one dies or goes dark at its own tick.
    fn missiles(tick: u64) -> Vec<MissileSnapshot> {
        let base = |id: u32, guidance: Guidance| MissileSnapshot {
            id,
            owner: 99,
            position: [100. * f64::from(id), 0., 12_000. - 8. * tick as f64],
            velocity: [0., 0., -1_000.],
            guidance,
            target: Some(7),
            radar_active: false,
            radar_acquired: false,
            supported: false,
            supporting_radar_position: None,
            alive: true,
        };
        let mut supported = base(1, Guidance::Supported);
        supported.supported = tick < 450;
        supported.supporting_radar_position = Some([1_000., 5_000., 2_000.]);
        let mut active = base(2, Guidance::Active);
        active.radar_active = tick < 600;
        active.radar_acquired = tick > 100;
        let infrared = base(3, Guidance::Infrared);
        let mut own = base(4, Guidance::Infrared);
        own.owner = 7;
        own.alive = tick < 800;
        let mut all = vec![supported, active, infrared, own];
        if tick > 200 {
            all[2].alive = tick < 700;
        }
        all
    }

    fn observe(service: &mut ThreatService, tick: u64) {
        service.observe(tick, receiver(tick), &missiles(tick), |_, _| {
            !tick.is_multiple_of(11)
        });
    }

    fn bytes(service: &ThreatService) -> crate::checkpoint::Coded {
        to_bytes(service, &Models::default()).unwrap()
    }

    #[test]
    fn a_threat_service_round_trips_at_300_600_and_900_and_steps_on_identically() {
        let models = Models::default();
        let mut service = ThreatService::new(7);
        let mut kinds = Vec::new();
        let mut stale = false;
        let mut tick = 0;
        for target in [300, 600, 900] {
            while tick < target {
                observe(&mut service, tick);
                tick += 1;
                for record in service.records() {
                    kinds.push(record.source);
                    stale |= record.stale;
                }
            }
            let copy = round_trip(&service, &models).unwrap();
            assert_eq!(copy.receiver_id, service.receiver_id);
            assert!(copy.records().eq(service.records()), "at tick {target}");

            let mut twin = copy;
            let mut original = service.clone();
            for step in 0..600 {
                observe(&mut original, target + step);
                observe(&mut twin, target + step);
                assert!(
                    twin.records().eq(original.records()),
                    "{step} steps after tick {target}"
                );
                assert_eq!(bytes(&twin), bytes(&original));
            }
        }
        // The fixture holds electronic, own-launch and visual evidence, and a
        // record that has gone stale.
        for kind in [
            EvidenceSource::ElectronicSupported,
            EvidenceSource::ElectronicActive,
            EvidenceSource::OwnLaunch,
            EvidenceSource::Visual,
        ] {
            assert!(kinds.contains(&kind), "{kind:?} was never recorded");
        }
        assert!(stale, "no record ever went stale");
    }

    #[test]
    fn damaged_threat_bytes_are_refused_without_a_panic() {
        let mut service = ThreatService::new(7);
        for tick in 0..120 {
            observe(&mut service, tick);
        }
        let coded = bytes(&service);
        for cut in 0..coded.body.len() {
            let mut damaged = coded.clone();
            damaged.body.truncate(cut);
            assert!(
                from_bytes::<ThreatService>(&damaged, &Models::default()).is_err(),
                "cut {cut}"
            );
        }
        let mut hostile = coded.clone();
        for byte in &mut hostile.body {
            *byte ^= 0x5a;
        }
        let _ = from_bytes::<ThreatService>(&hostile, &Models::default());
    }
}
