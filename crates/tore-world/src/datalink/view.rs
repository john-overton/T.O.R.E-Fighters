//! What one plane receives of the picture: its seat's share, which rides in
//! the cockpit readout (slice G6) and the wire (slice G7).
//!
//! Worked out from the picture on demand and never stored per plane. Every
//! aircraft of a side is linked with every other (John, 2026-10-05), so a
//! plane receives its flight's engagements, the locks of every other living
//! plane of its side, the tracks of its flight and of its side's other
//! flights, and its flightmates' state. Whether the plane has a radar is only
//! reported, for the displays to read: it never changes what the plane
//! receives.

use super::{
    DataLink, Engagement, FlightId, Lock, Member, MemberStatus, SEAT_TRACKS, Track, flight_key,
    squared_feet,
};
use std::collections::BTreeMap;

/// Where a track in a plane's view came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackSource {
    /// The plane's own sensors.
    Own,
    /// A flightmate's.
    Flight,
    /// A plane of another flight of the side, over the battle net.
    Network,
}

/// A track a plane receives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTrack {
    pub track: Track,
    pub source: TrackSource,
}

/// One plane's share of the picture.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkView {
    pub plane: u32,
    /// The plane's aircraft has a radar: its displays show the link's cues
    /// only if it does.
    pub radar: bool,
    /// What every member of the plane's flight attacks.
    pub engagements: Vec<Engagement>,
    /// The locks of the other living planes of the plane's side: the plane
    /// holding each, in plane id order.
    pub locks: Vec<(u32, Lock)>,
    /// Up to [`SEAT_TRACKS`] tracks, nearest the plane first.
    pub tracks: Vec<ViewTrack>,
    /// The state of the plane's flightmates other than itself.
    pub mates: Vec<MemberStatus>,
}

impl LinkView {
    fn empty(plane: u32) -> Self {
        Self {
            plane,
            radar: false,
            engagements: Vec::new(),
            locks: Vec::new(),
            tracks: Vec::new(),
            mates: Vec::new(),
        }
    }
}

impl DataLink {
    /// What `plane` receives of the picture. Empty for a plane the picture
    /// does not know.
    pub fn view(&self, plane: u32) -> LinkView {
        let Some(me) = self.member(plane).copied() else {
            return LinkView::empty(plane);
        };
        let mut view = LinkView::empty(plane);
        view.radar = me.radar;
        view.engagements = self
            .members
            .iter()
            .filter(|m| m.alive && m.flight == me.flight)
            .filter_map(|m| {
                self.engaged(m.plane).map(|target| Engagement {
                    plane: m.plane,
                    target,
                })
            })
            .collect();
        let allied = |other: &Member| other.flight.side == me.flight.side;
        view.locks = self
            .members
            .iter()
            .filter(|m| m.alive && m.plane != plane && allied(m))
            .filter_map(|m| self.lock(m.plane).map(|lock| (m.plane, lock)))
            .collect();
        view.mates = self
            .picture(me.flight)
            .map(|picture| {
                picture
                    .status
                    .iter()
                    .filter(|status| status.plane != plane)
                    .filter(|status| {
                        self.member(status.plane)
                            .is_some_and(|m| m.alive && m.flight == me.flight)
                    })
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        view.tracks = self.view_tracks(&me);
        view
    }

    fn view_tracks(&self, me: &Member) -> Vec<ViewTrack> {
        let mut best: BTreeMap<u32, ViewTrack> = BTreeMap::new();
        // The plane's own flight first, then the side's other flights in
        // flight order, so a tie in freshness keeps the nearer source.
        let mut pictures: Vec<_> = self
            .pictures
            .iter()
            .filter(|p| p.flight == me.flight || p.flight.side == me.flight.side)
            .collect();
        pictures.sort_by_key(|p| (p.flight != me.flight, flight_key(p.flight)));
        for picture in pictures {
            for track in &picture.tracks {
                let Some(reporter) = self.member(track.reporter) else {
                    continue;
                };
                if !reporter.alive || reporter.flight.side != me.flight.side {
                    continue;
                }
                let source = if track.reporter == me.plane {
                    TrackSource::Own
                } else if picture.flight == me.flight {
                    TrackSource::Flight
                } else {
                    TrackSource::Network
                };
                match best.get(&track.target) {
                    Some(old) if old.track.observed >= track.observed => {}
                    _ => {
                        best.insert(
                            track.target,
                            ViewTrack {
                                track: *track,
                                source,
                            },
                        );
                    }
                }
            }
        }
        let mut tracks: Vec<ViewTrack> = best.into_values().collect();
        tracks.sort_by(|a, b| {
            squared_feet(a.track.position, me.position)
                .total_cmp(&squared_feet(b.track.position, me.position))
                .then(a.track.target.cmp(&b.track.target))
        });
        tracks.truncate(SEAT_TRACKS);
        tracks
    }

    /// The flights of the mission, friendly first.
    pub fn flights(&self) -> Vec<FlightId> {
        let mut flights: Vec<FlightId> = Vec::new();
        for member in &self.members {
            if !flights.contains(&member.flight) {
                flights.push(member.flight);
            }
        }
        flights.sort_by_key(|flight| flight_key(*flight));
        flights
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datalink::{Damage, FlightPicture, Fuel, Weapons};
    use tore_sim::{
        ai::launch::{Side, WingId},
        sensors::Channel,
    };

    const RED: WingId = WingId {
        side: Side::Friendly,
        index: 0,
    };
    const BLUE: WingId = WingId {
        side: Side::Friendly,
        index: 1,
    };
    const BANDIT: WingId = WingId {
        side: Side::Enemy,
        index: 0,
    };

    fn member(plane: u32, flight: WingId, radar: bool, x: f64) -> Member {
        Member {
            plane,
            flight,
            member: plane as u8,
            aircraft: None,
            radar,
            human: false,
            alive: true,
            position: [x, 0., 0.],
        }
    }

    fn track(reporter: u32, target: u32, x: f64, observed: u64) -> Track {
        Track {
            reporter,
            target,
            position: [x, 0., 0.],
            velocity: [0.; 3],
            channel: Channel::Radar,
            observed,
        }
    }

    fn status(plane: u32) -> MemberStatus {
        MemberStatus {
            plane,
            fuel: Fuel::Normal,
            weapons: Weapons::Missiles,
            damage: Damage::None,
        }
    }

    /// Red: planes 0, 1 and 2, the last with no radar. Blue: plane 3. The
    /// bandits: plane 4. Planes 0, 1, 3 and 4 hold tracks and locks.
    fn link() -> DataLink {
        let mut link = DataLink {
            members: vec![
                member(0, RED, true, 0.),
                member(1, RED, true, 100.),
                member(2, RED, false, 200.),
                member(3, BLUE, true, 300.),
                member(4, BANDIT, true, 400.),
            ],
            ..DataLink::default()
        };
        let picture = |flight, tracks, ids: &[u32]| FlightPicture {
            flight,
            tick: 30,
            tracks,
            status: ids.iter().map(|id| status(*id)).collect(),
        };
        link.pictures = vec![
            picture(
                RED,
                vec![track(0, 10, 1000., 30), track(1, 11, 2000., 30)],
                &[0, 1, 2],
            ),
            picture(BLUE, vec![track(3, 12, 3000., 30)], &[3]),
            picture(BANDIT, vec![track(4, 13, 4000., 30)], &[4]),
        ];
        for (plane, target) in [(1, 10), (3, 12), (4, 20)] {
            link.locks.insert(plane, Lock { target, since: 5 });
        }
        for (plane, target) in [(0, 10), (2, 10), (3, 12), (4, 20)] {
            link.engaged.insert(plane, target);
        }
        link
    }

    fn targets(view: &LinkView) -> Vec<u32> {
        view.tracks.iter().map(|t| t.track.target).collect()
    }

    fn planes_of(view: &LinkView) -> Vec<u32> {
        view.locks.iter().map(|(plane, _)| *plane).collect()
    }

    #[test]
    fn a_plane_receives_its_flights_engagements_only() {
        let view = link().view(1);
        let engaged: Vec<(u32, u32)> = view
            .engagements
            .iter()
            .map(|e| (e.plane, e.target))
            .collect();
        // Plane 1 is locked on 10 but engaged on nothing; 3 and 4 are other
        // flights.
        assert_eq!(engaged, [(0, 10), (2, 10)]);
    }

    #[test]
    fn every_plane_of_the_side_is_linked_in_the_flight_and_over_the_net() {
        let view = link().view(0);
        assert!(view.radar);
        // The flightmate's lock and the other flight's, not the bandit's.
        assert_eq!(planes_of(&view), [1, 3]);
        assert_eq!(targets(&view), [10, 11, 12]);
        assert_eq!(view.tracks[0].source, TrackSource::Own);
        assert_eq!(view.tracks[1].source, TrackSource::Flight);
        assert_eq!(view.tracks[2].source, TrackSource::Network);
        assert!(!targets(&view).contains(&13), "an enemy's track");
        // Flightmates' state only, the radar-less one included.
        assert_eq!(
            view.mates.iter().map(|m| m.plane).collect::<Vec<_>>(),
            [1, 2]
        );
    }

    #[test]
    fn a_plane_with_no_radar_is_linked_all_the_same() {
        let link = link();
        let view = link.view(2);
        assert!(!view.radar, "the flag is reported for the displays");
        assert_eq!(planes_of(&view), [1, 3]);
        assert_eq!(targets(&view), [10, 11, 12]);
        assert_eq!(view.mates.len(), 2);
        // Its flightmates receive its state like any other plane's.
        assert!(link.view(1).mates.iter().any(|m| m.plane == 2));
    }

    #[test]
    fn the_other_side_shares_nothing_with_this_one() {
        let view = link().view(4);
        assert_eq!(targets(&view), [13]);
        assert!(view.locks.is_empty() && view.mates.is_empty());
    }

    #[test]
    fn a_plane_the_picture_does_not_know_receives_nothing() {
        let view = link().view(99);
        assert_eq!(view, LinkView::empty(99));
    }

    #[test]
    fn a_dead_member_shares_nothing() {
        let mut link = link();
        link.members[1].alive = false;
        let view = link.view(0);
        assert!(view.locks.iter().all(|(plane, _)| *plane != 1));
        assert!(!targets(&view).contains(&11));
        assert!(view.mates.iter().all(|m| m.plane != 1));
    }

    #[test]
    fn the_freshest_report_of_a_target_wins() {
        let mut link = link();
        // The second member saw target 10 later than the first did.
        link.pictures[0].tracks.push(track(1, 10, 1500., 31));
        let view = link.view(0);
        let ten = view.tracks.iter().find(|t| t.track.target == 10).unwrap();
        assert_eq!((ten.track.reporter, ten.track.observed), (1, 31));
        assert_eq!(ten.source, TrackSource::Flight);
    }

    #[test]
    fn a_view_lists_the_nearest_tracks_up_to_the_cap() {
        let mut link = link();
        link.pictures[1].tracks.clear();
        link.pictures[0].tracks = (0..40)
            .map(|n| track(0, 100 + n, 10_000. - 100. * f64::from(n), 30))
            .collect();
        let view = link.view(0);
        assert_eq!(view.tracks.len(), SEAT_TRACKS);
        let nearest = &view.tracks[0];
        assert_eq!(
            nearest.track.target, 139,
            "x = 6,100, nearest the plane at 0"
        );
        assert!(
            view.tracks
                .windows(2)
                .all(|pair| pair[0].track.position[0] <= pair[1].track.position[0])
        );
    }
}
