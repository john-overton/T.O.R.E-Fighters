//! Human target selection from the same readout and projection as the displays.
//! The host validates each selected identity; remote data-link tracks are not
//! candidates. See docs/spec/radar.md, "Target selection keys".
use crate::{camera::Camera, instruments};
use tore_sim::sensors::{Channel, Contact, Controls, Mode};
use tore_world::readout::SensorReadout;

fn cycle(ids: &[u32], selected: Option<u32>, forward: bool) -> Option<u32> {
    if ids.is_empty() {
        return None;
    }
    let index = selected.and_then(|id| ids.iter().position(|&candidate| candidate == id));
    Some(
        ids[match index {
            Some(i) if forward => (i + 1) % ids.len(),
            Some(i) => (i + ids.len() - 1) % ids.len(),
            None if forward => 0,
            None => ids.len() - 1,
        }],
    )
}

pub fn radar(sensors: &SensorReadout, controls: Controls, forward: bool) -> Option<u32> {
    if controls.channel != Channel::Radar
        || !sensors.operating(Channel::Radar)
        || sensors.mode_for(Channel::Radar, &controls) != Some(Mode::Tws)
    {
        return None;
    }
    let mut contacts: Vec<_> = sensors
        .contacts
        .iter()
        .filter(|c| {
            c.channel == Channel::Radar
                && instruments::project(c.bearing_rad, c.distance_ft, controls.range_nmi())
                    .is_some()
        })
        .collect();
    contacts.sort_by(|a, b| {
        a.distance_ft
            .total_cmp(&b.distance_ft)
            .then(a.id.cmp(&b.id))
    });
    cycle(
        &contacts.iter().map(|c| c.id).collect::<Vec<_>>(),
        sensors.selected,
        forward,
    )
}

/// Enter cycles left to right in the current camera. Apostrophe chooses the
/// centre in the forward camera supplied by the caller. Visibility (including
/// terrain obstruction) belongs to the ownship's visual readout, not the camera.
pub fn visual(
    contacts: &[Contact],
    selected: Option<u32>,
    camera: &Camera,
    size: [u32; 2],
    centre: bool,
) -> Option<u32> {
    let mut points: Vec<_> = contacts
        .iter()
        .filter_map(|c| camera.project(size, c.position).map(|p| (c.id, p)))
        .collect();
    if centre {
        let distance = |p: [f64; 2]| {
            (p[0] - f64::from(size[0]) / 2.).powi(2) + (p[1] - f64::from(size[1]) / 2.).powi(2)
        };
        points.sort_by(|a, b| distance(a.1).total_cmp(&distance(b.1)).then(a.0.cmp(&b.0)));
        points.first().map(|p| p.0)
    } else {
        points.sort_by(|a, b| a.1[0].total_cmp(&b.1[0]).then(a.0.cmp(&b.0)));
        cycle(
            &points.iter().map(|p| p.0).collect::<Vec<_>>(),
            selected,
            true,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::sensors::FEET_PER_NAUTICAL_MILE as NM;

    fn contact(id: u32, nm: f64, degrees: f64) -> Contact {
        let angle = degrees.to_radians();
        Contact {
            id,
            channel: Channel::Radar,
            bearing_rad: angle,
            elevation_rad: 0.,
            distance_ft: nm * NM,
            position: [nm * NM * angle.sin(), 0., nm * NM * angle.cos()],
            velocity: [0.; 3],
            track_eligible: true,
            destroyed: false,
        }
    }

    fn readout() -> tore_world::readout::CockpitReadout {
        let (combat, flight) = tore_world::combat::fixtures::loaded([1, 1]);
        combat
            .cockpit_readout(
                combat.own_id(),
                crate::combat::launcher(&flight),
                None,
                None,
            )
            .unwrap()
    }

    #[test]
    fn radar_cycles_displayed_own_contacts_including_friendlies_and_excludes_remote_tracks() {
        let mut r = readout();
        let controls = Controls::default(); // 10 nm
        r.sensors.operating[0] = true;
        r.sensors.radar_track_nmi = Some(50.);
        // 2 is a friendly: selection has no allegiance filter. A link-only
        // aircraft is absent from this ownship list, just as on the wire.
        r.sensors.contacts = vec![
            contact(3, 8., 0.),
            contact(2, 3., 0.),
            contact(1, 3., 0.),
            contact(4, 11., 0.),
            contact(5, 2., 100.),
        ];
        let mut order = Vec::new();
        for _ in 0..4 {
            let id = radar(&r.sensors, controls, true).unwrap();
            r.sensors.selected = Some(id);
            order.push(id);
        }
        assert_eq!(order, [1, 2, 3, 1]);
        assert_eq!(radar(&r.sensors, controls, false), Some(3));
        r.link.radar = true;
        r.link.tracks.push(tore_world::readout::LinkTrack {
            target: 99,
            position: [0., 0., NM],
            velocity: [0.; 3],
            source: tore_world::datalink::TrackSource::Flight,
        });
        r.sensors.selected = Some(99); // a shared-only track is never cycled
        assert_eq!(radar(&r.sensors, controls, true), Some(1));
        r.sensors.contacts.reverse();
        assert_eq!(radar(&r.sensors, controls, true), Some(1));
        r.sensors.radar_track_nmi = Some(5.); // RWS still draws search contacts
        assert_eq!(radar(&r.sensors, controls, true), None);
        r.sensors.radar_track_nmi = Some(50.);
        r.sensors.air_ground = true;
        assert_eq!(radar(&r.sensors, controls, true), None);
        r.sensors.air_ground = false;
        assert_eq!(
            radar(
                &r.sensors,
                Controls {
                    channel: Channel::Infrared,
                    ..controls
                },
                true
            ),
            None
        );
        r.sensors.operating[0] = false;
        assert_eq!(radar(&r.sensors, controls, true), None);
        r.sensors.operating[0] = true;
        r.sensors.contacts.clear();
        assert_eq!(radar(&r.sensors, controls, true), None);
    }

    #[test]
    fn enter_cycles_screen_order_and_apostrophe_picks_centre_not_nearest_distance() {
        let camera = Camera {
            position: [0.; 3],
            yaw: 0.,
            pitch: 0.,
            ..Camera::new()
        };
        let contacts = [
            contact(1, 3., 20.),
            contact(2, 4., -20.),
            contact(3, 5., 1.),
            contact(4, 1., 70.),
            contact(5, 1., 180.),
        ];
        let size = [800, 600];
        assert_eq!(visual(&contacts, None, &camera, size, false), Some(2));
        assert_eq!(visual(&contacts, Some(2), &camera, size, false), Some(3));
        assert_eq!(visual(&contacts, Some(3), &camera, size, false), Some(1));
        assert_eq!(visual(&contacts, Some(1), &camera, size, false), Some(2));
        assert_eq!(visual(&contacts, None, &camera, size, true), Some(3));
        let zoomed = Camera { zoom: 4., ..camera };
        assert_eq!(visual(&contacts, None, &zoomed, size, false), Some(3));
        assert_eq!(visual(&[], None, &zoomed, size, false), None);
        let rear = Camera {
            yaw: std::f32::consts::PI,
            zoom: 1.,
            ..zoomed
        };
        assert_eq!(visual(&contacts, None, &rear, size, false), Some(5));
    }
}
