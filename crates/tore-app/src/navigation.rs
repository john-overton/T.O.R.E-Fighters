//! Player NAV destination selection. See docs/spec/weapon-navigation-selection.md.
use tore_sim::airport::{Allegiance, Scene, Service};

#[derive(Clone, Debug)]
pub struct Destination {
    pub id: u32,
    pub name: String,
    pub position: [f64; 3],
}
impl Destination {
    pub fn distance(&self, position: [f64; 3]) -> f64 {
        (self.position[0] - position[0]).hypot(self.position[2] - position[2])
    }
    pub fn bearing(&self, position: [f64; 3]) -> u32 {
        ((self.position[0] - position[0])
            .atan2(self.position[2] - position[2])
            .to_degrees()
            .rem_euclid(360.)
            .round() as u32)
            % 360
    }
}
#[derive(Default)]
pub struct Navigation {
    pub airports_mode: bool,
    pub waypoints: Vec<Destination>,
    pub airports: Vec<Destination>,
    selected: [Option<u32>; 2],
    pub pending: Vec<usize>,
}
impl Navigation {
    pub fn entries(&self) -> &[Destination] {
        if self.airports_mode {
            &self.airports
        } else {
            &self.waypoints
        }
    }
    pub fn index(&self) -> Option<usize> {
        let selected = self.selected[usize::from(self.airports_mode)];
        self.entries()
            .iter()
            .position(|entry| Some(entry.id) == selected)
    }
    pub fn refresh(&mut self, scene: &Scene, service: &Service, position: [f64; 3]) {
        self.airports = scene
            .airports
            .iter()
            .filter(|airport| {
                airport.allegiance == Allegiance::Friendly
                    || (airport.allegiance == Allegiance::Neutral && airport.neutral_permission)
            })
            .filter_map(|airport| {
                let runway = airport
                    .runway_objects
                    .iter()
                    .filter(|id| service.usable(**id))
                    .filter_map(|id| scene.runway(*id))
                    .min_by(|a, b| {
                        let distance = |p: [f64; 3]| (p[0] - position[0]).hypot(p[2] - position[2]);
                        distance(a.approach_center)
                            .total_cmp(&distance(b.approach_center))
                            .then(a.object.cmp(&b.object))
                    })?;
                Some(Destination {
                    id: airport.id,
                    name: airport.name.clone(),
                    position: runway.approach_center,
                })
            })
            .collect();
        self.airports.sort_by(|a, b| {
            a.distance(position)
                .total_cmp(&b.distance(position))
                .then(a.id.cmp(&b.id))
        });
        for (mode, entries) in [&self.waypoints, &self.airports].into_iter().enumerate() {
            if !entries
                .iter()
                .any(|entry| Some(entry.id) == self.selected[mode])
            {
                self.selected[mode] = entries.first().map(|entry| entry.id);
            }
        }
    }
    pub fn control(&mut self, button: usize) -> Option<u32> {
        if button == 2 {
            self.airports_mode = !self.airports_mode;
        } else if let Some(index) = self.index() {
            let count = self.entries().len();
            let next = (index + if button == 0 { count - 1 } else { 1 }) % count;
            self.selected[usize::from(self.airports_mode)] = Some(self.entries()[next].id);
        }
        self.airports_mode.then(|| self.selected[1]).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::airport::{Airport, OrientedBox, Runway, SourceKey, StaticObject};

    fn scene() -> Scene {
        let mut scene = Scene::default();
        for (id, distance, allegiance, permission) in [
            (1, 3000., Allegiance::Friendly, false),
            (2, 1000., Allegiance::Neutral, true),
            (3, 100., Allegiance::Hostile, true),
            (4, 200., Allegiance::Unknown, true),
            (5, 300., Allegiance::Neutral, false),
            (6, 50., Allegiance::Friendly, false),
            (7, 1000., Allegiance::Friendly, false),
        ] {
            let bounds = OrientedBox {
                center: [0., 0., distance],
                half: [50., 1., 500.],
                heading: 0.,
                pitch: 0.,
                bank: 0.,
            };
            scene.objects.push(StaticObject {
                id,
                source: SourceKey {
                    layout: "synthetic".into(),
                    ordinal: id,
                },
                name: format!("Airport {id}"),
                object_type: "runway".into(),
                bounds,
                hit_points: 100,
                category: 0,
                radar_signature: 0.,
                infrared_signature: 0.,
                runway: true,
            });
            scene.runways.push(Runway {
                object: id,
                airport: id,
                name: format!("Runway {id}"),
                surface: bounds,
                approach_center: bounds.center,
                elevation_ft: 0.,
                heading: 0.,
                length_ft: 1000.,
            });
            scene.airports.push(Airport {
                id,
                name: format!("Airport {id}"),
                runway_objects: vec![id],
                allegiance,
                neutral_permission: permission,
            });
        }
        scene
    }

    #[test]
    fn airport_eligibility_distance_ties_wrap_and_selection_identity() {
        let scene = scene();
        let mut service = Service::new(&scene).unwrap();
        service.damage(6, 100);
        let mut nav = Navigation::default();
        nav.refresh(&scene, &service, [0.; 3]);
        assert_eq!(
            nav.airports.iter().map(|e| e.id).collect::<Vec<_>>(),
            [2, 7, 1]
        );
        assert_eq!(nav.index(), None);
        assert_eq!(nav.control(2), Some(2));
        assert_eq!(nav.control(0), Some(1));
        assert_eq!(nav.control(1), Some(2));
        nav.refresh(&scene, &service, [0., 0., 2900.]);
        assert_eq!(nav.entries()[nav.index().unwrap()].id, 2);
        service.damage(2, 100);
        nav.refresh(&scene, &service, [0., 0., 2900.]);
        assert_eq!(nav.entries()[nav.index().unwrap()].id, 1);
        assert_eq!(nav.control(2), None);
        assert!(nav.entries().is_empty());
    }

    #[test]
    fn mission_order_and_empty_controls() {
        let mut nav = Navigation::default();
        assert_eq!(nav.control(0), None);
        assert_eq!(nav.control(1), None);
        nav.waypoints = vec![
            Destination {
                id: 10,
                name: "FIRST".into(),
                position: [6076.12, 0., 0.],
            },
            Destination {
                id: 4,
                name: "SECOND".into(),
                position: [0., 0., 6076.12],
            },
        ];
        let scene = Scene::default();
        let service = Service::new(&scene).unwrap();
        nav.refresh(&scene, &service, [0.; 3]);
        assert_eq!(nav.index(), Some(0));
        nav.control(0);
        assert_eq!(nav.index(), Some(1));
        nav.control(1);
        assert_eq!(nav.index(), Some(0));
        assert_eq!(nav.entries()[0].bearing([0.; 3]), 90);
        assert_eq!(nav.entries()[0].distance([0.; 3]), 6076.12);
    }
}
