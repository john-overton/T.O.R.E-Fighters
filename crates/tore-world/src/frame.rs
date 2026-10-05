//! The flight frame: plain data the flight screen draws each rendered frame,
//! for the plane of one seat. D5a of the multiplayer plan; see
//! docs/ARCHITECTURE.md, "The flight screen draws a frame".
//!
//! Single player fills a frame from its [`World`]; a networked client will fill
//! one from its prediction, the interpolated snapshots, the effects it
//! regenerates and the cues the host sends. The screen, the views and the
//! sounds read the frame and never a `World`, and never assume the presented
//! plane is plane 0. Nothing here draws.

use crate::{
    combat::launcher as combat_launcher,
    readout::CockpitReadout,
    seats::{PlaneId, SeatId},
    snapshot::RenderSnapshot,
    world::{Cockpit, Cue, World},
};
use std::{borrow::Cow, cell::OnceCell};
use tore_sim::{
    combat::{
        countermeasures::Devices,
        live::{Configuration, Launcher},
        smoke::Smoke,
    },
    flight,
};

/// The readout of a frame, built on first use. A host or a client that has
/// the readout already makes it with [`ReadoutSlot::ready`]; `World::flight_frame`
/// makes one that builds it from the world when a display first reads it, and
/// [`World::flight_frame_sharing`] one that keeps what it builds in a cell
/// the caller owns, so the frames of one tick build it once between them.
/// It reads as a [`CockpitReadout`].
pub struct ReadoutSlot<'a> {
    shared: Option<&'a OnceCell<CockpitReadout>>,
    own: OnceCell<CockpitReadout>,
    make: Box<dyn Fn() -> CockpitReadout + 'a>,
}
impl<'a> ReadoutSlot<'a> {
    /// A slot holding `readout`.
    pub fn ready(readout: CockpitReadout) -> Self {
        Self {
            shared: None,
            own: OnceCell::from(readout),
            make: Box::new(|| unreachable!("a ready slot is full")),
        }
    }
    fn lazy(
        shared: Option<&'a OnceCell<CockpitReadout>>,
        make: impl Fn() -> CockpitReadout + 'a,
    ) -> Self {
        Self {
            shared,
            own: OnceCell::new(),
            make: Box::new(make),
        }
    }
}
impl std::ops::Deref for ReadoutSlot<'_> {
    type Target = CockpitReadout;
    fn deref(&self) -> &CockpitReadout {
        self.shared
            .unwrap_or(&self.own)
            .get_or_init(|| (self.make)())
    }
}

/// What the flight screen draws this frame, for the plane `plane` of `seat`.
pub struct FlightFrame<'a> {
    /// The seat whose screen this is.
    pub seat: SeatId,
    /// The plane the seat flies: the presented plane. The picture's player is
    /// this plane, and every reading of "the player's aircraft" means it.
    pub plane: PlaneId,
    /// The plane's flight as the last tick left it.
    pub flight: &'a flight::State,
    /// The plane's flight at the start of that tick, the one before `flight`.
    pub previous: &'a flight::State,
    /// The flight to draw: `flight` and `previous` blended to this frame's
    /// instant, or `flight` itself for a tick's frame and a frozen one.
    pub presented: Cow<'a, flight::State>,
    /// The presented mission picture for the seat: the last two snapshots
    /// blended to this frame's instant, or the newest one for a tick's frame.
    /// Its player is the seat's plane and every other aircraft is a target.
    pub picture: &'a RenderSnapshot,
    /// The smoke puffs of hits, wrecks and motors, then the contrails.
    pub smoke: [&'a Smoke; 2],
    /// Chaff cartridges and flares in the air.
    pub devices: &'a Devices,
    /// What the plane carries and how its weapons and sensors are made: the
    /// loadout, which changes only when the stores are loaded. Read the
    /// stores left in `readout`.
    pub config: &'a Configuration,
    /// What the seat's displays and cockpit sounds show that only the host
    /// knows, for the flight `presented` is (a tick's frame and a frozen
    /// flight show the flight as the last tick left it).
    ///
    /// It is built when first read, so a frame that never reads it costs
    /// nothing; read it through the slot like the readout itself.
    pub readout: ReadoutSlot<'a>,
    /// The cues of the tick this frame presents, every seat's; read them with
    /// [`FlightFrame::cues`]. Empty for a frame drawn between ticks.
    pub tick_cues: &'a [Cue],
}

impl Cue {
    /// The seat the cue is for, or `None` for one about the mission, which
    /// every presenter shows.
    pub fn seat(&self) -> Option<SeatId> {
        match self {
            Cue::Message { seat, .. }
            | Cue::Feedback { seat, .. }
            | Cue::Tower { seat, .. }
            | Cue::WeaponCycled { seat }
            | Cue::Radio { seat, .. }
            | Cue::OrderVoice { seat, .. } => Some(*seat),
            Cue::Flown | Cue::CombatStepped | Cue::WingEjection { .. } | Cue::Picture => None,
        }
    }

    /// Whether `seat`'s screen shows the cue: another seat's output is that
    /// seat's to present.
    pub fn is_for(&self, seat: SeatId) -> bool {
        self.seat().is_none_or(|owner| owner == seat)
    }
}

impl<'a> FlightFrame<'a> {
    /// The tick's cues this seat shows: its own and the mission-wide ones,
    /// with their place in the tick's list.
    pub fn cues(&self) -> impl Iterator<Item = (usize, &'a Cue)> + use<'a, '_> {
        let seat = self.seat;
        self.tick_cues
            .iter()
            .enumerate()
            .filter(move |(_, cue)| cue.is_for(seat))
    }

    /// The flight to draw this frame.
    pub fn presented(&self) -> &flight::State {
        &self.presented
    }
}

impl World {
    /// The plane the mission's render history is built for: the first
    /// cockpit's. It is the picture the app presents; a host serving other
    /// seats builds theirs with [`combat::Combat::snapshot`] for their planes.
    /// A presenter always has a cockpit; this panics without one (an open
    /// mission before anyone takes a plane), which [`Self::picture_plane_if_any`]
    /// answers instead.
    pub fn picture_plane(&self) -> PlaneId {
        self.picture_plane_if_any()
            .expect("a presented mission has a cockpit")
    }

    /// [`Self::picture_plane`], or `None` when no human flies: then the
    /// render history keeps nothing, as a host with nobody to draw for needs
    /// none (agent decision, D3c).
    pub fn picture_plane_if_any(&self) -> Option<PlaneId> {
        self.cockpits.first().map(|cockpit| cockpit.plane)
    }

    /// Ends the tick's picture for [`World::picture_plane`]: the current
    /// snapshot becomes the previous one. When the picture plane changed
    /// (the first cockpit came or went by a handoff) the history starts over
    /// from it, and with no cockpit it is emptied.
    pub(crate) fn advance_picture(&mut self) {
        let Some(cockpit) = self.cockpits.first() else {
            self.combat.clear_render();
            return;
        };
        let wings = self.ai_wings.as_ref();
        if self.combat.render_plane() == Some(cockpit.plane.0) {
            self.combat
                .advance_render(cockpit.plane.0, &cockpit.flight, wings);
        } else {
            self.combat
                .restart_render(cockpit.plane.0, &cockpit.flight, wings);
        }
    }

    /// Retakes the current snapshot for [`World::picture_plane`] after a
    /// command changed the scene between ticks; nothing with no cockpit.
    pub(crate) fn refresh_picture(&mut self) {
        let Some(cockpit) = self.cockpits.first() else {
            return;
        };
        let wings = self.ai_wings.as_ref();
        if self.combat.render_plane() == Some(cockpit.plane.0) {
            self.combat
                .refresh_render(cockpit.plane.0, &cockpit.flight, wings);
        } else {
            self.combat
                .restart_render(cockpit.plane.0, &cockpit.flight, wings);
        }
    }

    /// The cockpit readout of `seat`'s plane for `launcher`, the plane's
    /// position, attitude, speed and devices as the caller presents them, or
    /// `None` when the seat flies no plane. Single player builds it every
    /// rendered frame from the interpolated flight; a host builds it at each
    /// snapshot from the tick's flight.
    pub fn cockpit_readout(&self, seat: SeatId, launcher: Launcher) -> Option<CockpitReadout> {
        let cockpit = &self.cockpits[self.cockpit_of(seat)?];
        self.readout_of(cockpit, launcher)
    }

    /// The readout of `cockpit`'s plane: combat's, with the plane's share of
    /// the flight data link ([`crate::datalink::DataLink::readout`]).
    fn readout_of(&self, cockpit: &Cockpit, launcher: Launcher) -> Option<CockpitReadout> {
        let mut readout = self.combat.cockpit_readout(
            cockpit.plane.0,
            launcher,
            self.ai_wings.as_ref(),
            Some(cockpit),
        )?;
        readout.link = self.datalink.readout(cockpit.plane.0);
        Some(readout)
    }

    /// The flight frame of `seat`, or `None` when it flies no plane.
    ///
    /// `presented` is the plane's flight blended to this frame's instant, and
    /// `picture` the mission picture blended the same way; the app owns both
    /// as it owns the tick fraction. `None` for `presented` draws the flight
    /// as the last tick left it, as a frozen flight and a tick's frame do.
    /// `cues` are the cues of the tick being presented, or none.
    pub fn flight_frame<'a>(
        &'a self,
        seat: SeatId,
        presented: Option<flight::State>,
        picture: &'a RenderSnapshot,
        cues: &'a [Cue],
    ) -> Option<FlightFrame<'a>> {
        self.frame(seat, presented, picture, cues, None)
    }

    /// [`World::flight_frame`] for the flight as the last tick left it, whose
    /// readout is kept in `shared` once built: every frame made with the same
    /// cell between two steps of the world builds it once. Give each tick's
    /// frames a cell of their own.
    pub fn flight_frame_sharing<'a>(
        &'a self,
        seat: SeatId,
        picture: &'a RenderSnapshot,
        cues: &'a [Cue],
        shared: &'a OnceCell<CockpitReadout>,
    ) -> Option<FlightFrame<'a>> {
        self.frame(seat, None, picture, cues, Some(shared))
    }

    fn frame<'a>(
        &'a self,
        seat: SeatId,
        presented: Option<flight::State>,
        picture: &'a RenderSnapshot,
        cues: &'a [Cue],
        shared: Option<&'a OnceCell<CockpitReadout>>,
    ) -> Option<FlightFrame<'a>> {
        let cockpit = &self.cockpits[self.cockpit_of(seat)?];
        let config = self.combat.state.ownship(cockpit.plane.0)?.configuration();
        let launcher = combat_launcher(presented.as_ref().unwrap_or(&cockpit.flight));
        let presented = presented.map_or(Cow::Borrowed(&cockpit.flight), Cow::Owned);
        Some(FlightFrame {
            seat,
            plane: cockpit.plane,
            flight: &cockpit.flight,
            previous: &cockpit.previous_flight,
            presented,
            picture,
            smoke: [&self.combat.state.smoke, &self.combat.contrails],
            devices: &self.combat.state.devices,
            config,
            readout: ReadoutSlot::lazy(shared, move || {
                self.readout_of(cockpit, launcher)
                    .expect("the seat's plane has an ownship")
            }),
            tick_cues: cues,
        })
    }

    /// The flight of `seat`'s plane blended to `alpha` of the way from the
    /// start of the last tick to its end, as the screen presents it between
    /// ticks.
    pub fn presented_flight(&self, seat: SeatId, alpha: f64) -> Option<flight::State> {
        let cockpit = &self.cockpits[self.cockpit_of(seat)?];
        Some(cockpit.flight.presented(&cockpit.previous_flight, alpha))
    }
}
