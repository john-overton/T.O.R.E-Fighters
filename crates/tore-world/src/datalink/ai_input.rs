//! What the world hands the AI each tick.
//!
//! Slice G2 puts the humans' locked targets to work: `AiWings::set_link`
//! reads `DataLink::human_engagements` before each step and the AI's
//! engagement table counts them (B41's wing attackers). The tracks and member
//! state of [`AiInput`] stay a skeleton until G3b (the linked-track pursuit)
//! and G4 (the member state for AI leads).

use super::{DataLink, Engagement, FlightId, FlightPicture, MemberStatus, Track, flight_key};

/// One flight's published picture, as the AI would be handed it.
#[derive(Clone, Debug, PartialEq)]
pub struct FlightFeed {
    pub flight: FlightId,
    /// The combat tick it was published.
    pub published: u64,
    pub tracks: Vec<Track>,
    pub status: Vec<MemberStatus>,
}

impl From<&FlightPicture> for FlightFeed {
    fn from(picture: &FlightPicture) -> Self {
        Self {
            flight: picture.flight,
            published: picture.tick,
            tracks: picture.tracks.clone(),
            status: picture.status.clone(),
        }
    }
}

/// The AI's input from the picture.
#[derive(Clone, Debug, PartialEq)]
pub struct AiInput {
    /// The combat tick of the observation.
    pub tick: u64,
    /// What the humans attack: a human's locked target, which the AI cannot
    /// see for itself. In plane id order.
    pub human_engagements: Vec<Engagement>,
    /// Each flight's published picture, friendly flights first.
    pub flights: Vec<FlightFeed>,
}

impl DataLink {
    /// What the humans attack: each living human's locked target, in plane id
    /// order. The AI cannot see a human's sensors for itself.
    pub fn human_engagements(&self) -> Vec<Engagement> {
        self.engagements()
            .into_iter()
            .filter(|e| self.member(e.plane).is_some_and(|m| m.human && m.alive))
            .collect()
    }

    /// What the AI receives of the picture now.
    pub fn ai_input(&self) -> AiInput {
        let mut flights: Vec<FlightFeed> = self.pictures().iter().map(FlightFeed::from).collect();
        flights.sort_by_key(|feed| flight_key(feed.flight));
        AiInput {
            tick: self.tick(),
            human_engagements: self.human_engagements(),
            flights,
        }
    }
}
