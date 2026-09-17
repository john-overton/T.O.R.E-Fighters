//! Scope and exposure presentation inputs. The simulation decides what is
//! observable; this module only reprojects shared observations for drawing and
//! picking. It can never make a hidden target selectable, and it carries
//! bearing and intensity for received noise rather than an emitter's range.
use crate::flight;
use tore_sim::{
    attitude::{Basis, Vector, dot},
    combat::live,
    sensors::{self, passive},
};

/// One plotted return. A stale plot is the final observation of a lost
/// contact: visibly old, never selectable.
#[derive(Clone, Debug, PartialEq)]
pub struct Contact {
    pub id: u32,
    pub bearing_rad: f64,
    pub distance_ft: f64,
    pub heading_rad: f64,
    pub track_eligible: bool,
    pub destroyed: bool,
    pub selected: bool,
    pub acquired: bool,
    pub stale: bool,
    /// Past observations reprojected through the current scope transform,
    /// oldest first. Never connected across a missing sample.
    pub trail: Vec<(f64, f64)>,
}

/// Received interference in one direction. Passive noise supplies no range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strobe {
    pub bearing_rad: f64,
    pub density: f64,
    pub half_width_rad: f64,
    pub sidelobe: f64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Scope {
    /// Simulation tick, used only for the deterministic noise texture phase.
    pub tick: u64,
    pub channel: &'static str,
    /// The passive infrared channel is selected, so the radar is not emitting.
    pub infrared: bool,
    pub mode: Option<&'static str>,
    pub range_nmi: f64,
    /// The selected channel is installed, powered and undamaged.
    pub operating: bool,
    pub unavailable: Option<&'static str>,
    pub history: bool,
    pub contacts: Vec<Contact>,
    pub strobes: Vec<Strobe>,
    pub selected: Option<u32>,
    /// Weapon support for the selected target, or None when nothing is selected.
    pub status: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    pub bearing_rad: f64,
    pub distance_nmi: Option<f64>,
    pub symbol: passive::Symbol,
}

/// Passive exposure instrument inputs. The contour is a reference-radar
/// estimate of directional vulnerability, not a detection boundary.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Rcs {
    pub contour: Vec<(f64, f64)>,
    pub emitters: Vec<Emitter>,
    pub scale_nmi: f64,
    pub signature: f64,
}

/// Agent-proposed noise density, clamped for readability.
fn density(received: f64) -> f64 {
    let value = 0.35 * received / (1. + received);
    if value.is_finite() {
        value.clamp(0., 0.35)
    } else {
        0.
    }
}

fn relative(basis: &Basis, from: Vector, to: Vector) -> (f64, f64) {
    let delta: Vector = std::array::from_fn(|i| to[i] - from[i]);
    (
        dot(delta, basis.right).atan2(dot(delta, basis.forward)),
        dot(delta, delta).sqrt(),
    )
}

pub fn scope(state: &live::State, s: &flight::State) -> Scope {
    let sensors = &state.sensors;
    let basis = Basis::new(s.yaw, s.pitch, s.bank);
    // Labels and the plotted scale follow the player's own controls, so they
    // never lag a step behind the switch that was just pressed. Contacts and
    // weapon support stay with the simulation that produced them.
    let controls = s.sensors;
    let channel = controls.channel;
    let selected = sensors.selected();
    let acquired = sensors.acquired();
    let plot = |c: &sensors::Contact, stale: bool, trail: Vec<(f64, f64)>| Contact {
        id: c.id,
        bearing_rad: c.bearing_rad,
        distance_ft: c.distance_ft,
        heading_rad: c.velocity[0].atan2(c.velocity[2]) - s.yaw,
        track_eligible: c.track_eligible,
        destroyed: c.destroyed,
        selected: !stale && selected == Some(c.id),
        acquired: !stale && acquired == Some(c.id),
        stale,
        trail,
    };
    let mut contacts: Vec<Contact> = sensors
        .contacts()
        .iter()
        .map(|c| {
            let trail = sensors
                .trail(c.id)
                .iter()
                .map(|sample| {
                    let (bearing, distance) = relative(&basis, s.position, sample.position);
                    (bearing, distance)
                })
                .collect();
            plot(c, false, trail)
        })
        .collect();
    contacts.extend(
        sensors
            .plots()
            .iter()
            .filter(|p| p.channel == channel)
            .map(|p| Contact {
                id: p.id,
                bearing_rad: p.bearing_rad,
                distance_ft: p.distance_ft,
                heading_rad: 0.,
                track_eligible: false,
                destroyed: false,
                selected: false,
                acquired: false,
                stale: true,
                trail: vec![],
            }),
    );
    Scope {
        tick: sensors.tick(),
        channel: channel.label(),
        infrared: channel == sensors::Channel::Infrared,
        mode: sensors.mode_for(channel, &controls).map(|m| m.label()),
        range_nmi: controls.range_nmi(),
        operating: sensors.operating(channel),
        unavailable: if sensors.available(channel) {
            None
        } else {
            Some("NOT INSTALLED")
        },
        history: controls.history,
        contacts,
        strobes: sensors
            .display_strobes()
            .iter()
            .map(|strobe| Strobe {
                bearing_rad: strobe.bearing_rad,
                density: density(strobe.received),
                half_width_rad: strobe.half_width_rad,
                sidelobe: strobe.sidelobe_floor,
            })
            .collect(),
        selected,
        status: selected.map(|id| sensors.track_status(id).label()),
    }
}

pub fn rcs(state: &live::State, s: &flight::State, scale_nmi: f64) -> Rcs {
    let signature = state.configuration().sensors.signature;
    let basis = Basis::new(s.yaw, s.pitch, s.bank);
    let configuration = sensors::Configuration {
        gear: s.gear,
        flaps: s.flaps,
        bay: s.bay,
    };
    Rcs {
        contour: signature.exposure_contour(&basis, configuration, 5.),
        emitters: state
            .emitters
            .iter()
            .map(|e| Emitter {
                bearing_rad: e.bearing_rad,
                distance_nmi: e.distance_nmi,
                symbol: e.symbol,
            })
            .collect(),
        scale_nmi,
        // Nose-on exposure against the reference observer, using the same
        // level heading direction as the contour's first sample.
        signature: {
            let heading = basis.angles()[0];
            signature.effective_radar(&basis, [heading.sin(), 0., heading.cos()], configuration)
        },
    }
}

/// Pick the nearest drawn contact to an instrument raster point. Drawing and
/// picking share one projection, so a click selects what the player sees.
/// Stale plots and history dots are never selectable.
pub fn pick(
    contacts: &[Contact],
    project: impl Fn(&Contact) -> Option<(f64, f64)>,
    point: (f64, f64),
    tolerance: f64,
) -> Option<u32> {
    let mut best: Option<(f64, u32)> = None;
    for contact in contacts.iter().filter(|c| !c.stale) {
        let Some((x, y)) = project(contact) else {
            continue;
        };
        let distance = (x - point.0).hypot(y - point.1);
        if distance <= tolerance
            // Equal-distance ties resolve by stable target identity.
            && best.is_none_or(|(d, id)| distance < d || (distance == d && contact.id < id))
        {
            best = Some((distance, contact.id));
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn contact(id: u32, bearing: f64, stale: bool) -> Contact {
        Contact {
            id,
            bearing_rad: bearing,
            distance_ft: 10_000.,
            heading_rad: 0.,
            track_eligible: true,
            destroyed: false,
            selected: false,
            acquired: false,
            stale,
            trail: vec![],
        }
    }
    #[test]
    fn picking_uses_the_drawn_projection_and_stable_tie_breaking() {
        let contacts = [
            contact(2, 0., false),
            contact(1, 0., false),
            contact(3, 1., false),
        ];
        let project = |c: &Contact| Some((c.bearing_rad * 50., 0.));
        assert_eq!(pick(&contacts, project, (0., 0.), 7.), Some(1));
        assert_eq!(pick(&contacts, project, (50., 0.), 7.), Some(3));
        assert_eq!(pick(&contacts, project, (25., 0.), 7.), None);
        assert_eq!(pick(&[], project, (0., 0.), 7.), None);
    }
    #[test]
    fn stale_plots_and_hidden_contacts_are_never_picked() {
        let contacts = [contact(1, 0., true), contact(2, 1., false)];
        let project = |c: &Contact| (!c.stale).then_some((c.bearing_rad * 50., 0.));
        assert_eq!(pick(&contacts, project, (0., 0.), 7.), None);
        assert_eq!(pick(&contacts, project, (50., 0.), 7.), Some(2));
    }
    #[test]
    fn noise_density_is_bounded_and_rises_with_received_interference() {
        assert_eq!(density(0.), 0.);
        assert!((density(1.) - 0.175).abs() < 1e-9);
        assert!(density(1000.) <= 0.35 && density(1000.) > 0.34);
        assert_eq!(density(f64::NAN), 0.);
    }
}
