//! Live sensor state: current observations, one persistent selected target, at
//! most one fire-control track across radar and infrared, bounded contact
//! history and the received interference used by both detection and the scope.
//! Timings are the authored values in docs/radar.md at the fixed 120 Hz cadence.
use super::detection::{
    Sighting, angular_coupling, clutter_exposure, effective_range_ft, infrared_range_ft,
    interference_quotient, jammer_factor, look_down_factor, notch_factor, notch_speed_fps,
    received,
};
use super::profile::{
    DEFAULT_RANGE_INDEX, JammerProfile, RANGE_LADDER_NMI, RadarProfile, SensorProfiles,
};
use super::signature::{Configuration, SignatureProfile};
use crate::attitude::{Basis, Vector, dot};

/// Weapon-track acquisition, 0.5 seconds of consecutive valid steps.
pub const ACQUISITION_STEPS: u32 = 60;
/// Unselected coasting plot lifetime after an observation is lost.
pub const STALE_STEPS: u32 = 120;
/// Presentation fade for received noise, 0.25 seconds. Authoritative support
/// changes never wait for it.
pub const NOISE_FADE_STEPS: u32 = 30;
/// History sample interval, capacity per target per channel and maximum age.
pub const HISTORY_INTERVAL_STEPS: u64 = 60;
pub const HISTORY_SAMPLES: usize = 8;
pub const HISTORY_AGE_STEPS: u64 = 480;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Radar,
    Infrared,
    /// Always available alongside the selected scope channel. It is not part of
    /// the scope channel cycle and never supplies radar weapon support.
    Visual,
}
impl Channel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Radar => "RADAR",
            Self::Infrared => "IR",
            Self::Visual => "VISUAL",
        }
    }
}

/// Radar search/track selection is automatic from the display range. Infrared
/// is its own passive channel and never reports RWS or TWS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Rws,
    Tws,
    Infrared,
}
impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rws => "RWS",
            Self::Tws => "TWS",
            Self::Infrared => "IR",
        }
    }
    /// Only TWS and infrared can hold a fire-control track.
    pub fn tracks(self) -> bool {
        self != Self::Rws
    }
}

/// Player sensor controls. They never reveal a hidden target or change a
/// sensor's physical coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controls {
    pub channel: Channel,
    pub range_index: usize,
    pub history: bool,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            channel: Channel::Radar,
            range_index: DEFAULT_RANGE_INDEX,
            history: false,
        }
    }
}
impl Controls {
    pub fn range_nmi(&self) -> f64 {
        RANGE_LADDER_NMI[self.range_index.min(RANGE_LADDER_NMI.len() - 1)]
    }
}

/// Ownship sensor state for one step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observer {
    pub position: Vector,
    pub basis: Basis,
    /// Radar emitting: powered, selected and not otherwise inhibited.
    pub radar_powered: bool,
    pub radar_failed: bool,
    pub infrared_failed: bool,
    pub visual_failed: bool,
}

/// One physical object the sensors may observe. Combat viability, physical
/// existence and sensor observation are deliberately separate states.
#[derive(Clone, Debug, PartialEq)]
pub struct Observable {
    pub id: u32,
    pub position: Vector,
    /// Ground-relative velocity, used for the notch projection.
    pub velocity: Vector,
    pub basis: Basis,
    pub configuration: Configuration,
    pub signature: SignatureProfile,
    pub jammer: Option<JammerProfile>,
    pub jammer_active: bool,
    /// Own radar transmission, supplied by the host emitter registry.
    pub radar_emitting: bool,
    /// A physical airborne object. Hit points reaching zero does not clear it.
    pub airborne: bool,
    pub destroyed: bool,
}

/// Terrain services supplied by the host, so this component never samples a
/// world of its own.
pub struct Environment<'a> {
    pub ground: &'a dyn Fn(f64, f64) -> f64,
    pub obscured: &'a dyn Fn(Vector, Vector) -> bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub id: u32,
    pub channel: Channel,
    pub bearing_rad: f64,
    pub elevation_rad: f64,
    pub distance_ft: f64,
    pub position: Vector,
    pub velocity: Vector,
    /// Inside the tracking volume and its effective distance. Tracking still
    /// requires the selected target and a tracking mode.
    pub track_eligible: bool,
    pub destroyed: bool,
}

/// The last observation of a lost contact, drawn as a visibly stale plot. It
/// carries no predicted motion and can never be selected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plot {
    pub id: u32,
    pub channel: Channel,
    pub bearing_rad: f64,
    pub elevation_rad: f64,
    pub distance_ft: f64,
    pub position: Vector,
    pub age: u32,
}

/// Received noise from one emitter. The renderer receives bearing and
/// intensity, never the emitter's true range or identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strobe {
    /// Stable identity of the emitting object, for presentation continuity.
    pub id: u32,
    pub bearing_rad: f64,
    pub elevation_rad: f64,
    pub received: f64,
    pub half_width_rad: f64,
    pub sidelobe_floor: f64,
    line_of_sight: Vector,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub tick: u64,
    pub position: Vector,
}

#[derive(Clone, Debug, PartialEq)]
struct Trail {
    channel: Channel,
    id: u32,
    samples: Vec<Sample>,
}

/// Why a specific target is or is not supported for a radar weapon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    Tracked,
    Acquiring,
    SearchOnly,
    TrackCoverage,
    NotSelected,
    NoObservation,
    RadarOff,
    RadarFailed,
    Unavailable,
}
impl Support {
    pub fn label(self) -> &'static str {
        match self {
            Self::Tracked => "TRACK",
            Self::Acquiring => "ACQUIRING",
            Self::SearchOnly => "RWS SEARCH ONLY",
            Self::TrackCoverage => "BEYOND TRACK COVERAGE",
            Self::NotSelected => "NOT SELECTED",
            Self::NoObservation => "NO RADAR CONTACT",
            Self::RadarOff => "RADAR OFF",
            Self::RadarFailed => "RADAR FAILED",
            Self::Unavailable => "NO RADAR",
        }
    }
    pub fn supported(self) -> bool {
        self == Self::Tracked
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    SelectionCleared(u32),
    TrackAcquired(u32),
    TrackReleased(u32),
    ContactLost(u32),
}

/// A presentation-only map observation. Identification requires a visual return.
#[derive(Clone, Debug, PartialEq)]
pub struct MapContact {
    pub contact: Contact,
    pub identified: bool,
    pub airborne: bool,
}

/// The forward view at 1x zoom: 60 degrees tall, 4:3 wide. Enter picks
/// only aircraft inside it.
const FORWARD_VIEW_HALF_HEIGHT: f64 = std::f64::consts::FRAC_PI_6;
const FORWARD_VIEW_HALF_WIDTH: f64 = 0.656_053; // atan(4/3 * tan 30 degrees)

#[derive(Clone, Debug, PartialEq)]
pub struct Sensors {
    pub profiles: SensorProfiles,
    pub controls: Controls,
    selected: Option<u32>,
    acquired: Option<u32>,
    acquisition: u32,
    active: Option<Channel>,
    mode: Option<Mode>,
    /// Equipment state from the last step, so a failed radar stays
    /// distinguishable from one the player switched off.
    radar_powered: bool,
    radar_failed: bool,
    infrared_failed: bool,
    visual_failed: bool,
    contacts: Vec<Contact>,
    visual: Vec<Contact>,
    map_contacts: Vec<MapContact>,
    strobes: Vec<Strobe>,
    fading: Vec<(Strobe, u32)>,
    plots: Vec<Plot>,
    history: Vec<Trail>,
    tick: u64,
}
impl Sensors {
    pub fn new(profiles: SensorProfiles) -> Self {
        Self {
            profiles,
            controls: Controls::default(),
            selected: None,
            acquired: None,
            acquisition: 0,
            active: None,
            mode: None,
            radar_powered: false,
            radar_failed: false,
            infrared_failed: false,
            visual_failed: false,
            contacts: Vec::new(),
            visual: Vec::new(),
            map_contacts: Vec::new(),
            strobes: Vec::new(),
            fading: Vec::new(),
            plots: Vec::new(),
            history: Vec::new(),
            tick: 0,
        }
    }
    pub fn map_contacts(&self) -> &[MapContact] {
        &self.map_contacts
    }
    pub fn contacts(&self) -> &[Contact] {
        &self.contacts
    }
    /// Visually observed contacts, collected independently of the selected
    /// scope channel. They are never fused with radar or infrared returns.
    pub fn visual(&self) -> &[Contact] {
        &self.visual
    }
    /// Live received emitters, used for detection.
    pub fn strobes(&self) -> &[Strobe] {
        &self.strobes
    }
    /// Received emitters for presentation, fading out over a quarter second
    /// after an emission stops. Detection never reads this.
    pub fn display_strobes(&self) -> Vec<Strobe> {
        self.fading
            .iter()
            .map(|(strobe, age)| Strobe {
                received: strobe.received
                    * (1. - f64::from(*age) / f64::from(NOISE_FADE_STEPS)).max(0.),
                ..*strobe
            })
            .collect()
    }
    pub fn plots(&self) -> &[Plot] {
        &self.plots
    }
    pub fn selected(&self) -> Option<u32> {
        self.selected
    }
    pub fn acquired(&self) -> Option<u32> {
        self.acquired
    }
    pub fn mode(&self) -> Option<Mode> {
        self.mode
    }
    pub fn tick(&self) -> u64 {
        self.tick
    }
    pub fn contact(&self, id: u32) -> Option<&Contact> {
        self.contacts.iter().find(|c| c.id == id)
    }
    /// Any current observation of this identity, on the active scope channel
    /// or visually. Selection uses this; radar weapon support does not.
    pub fn observation(&self, id: u32) -> Option<&Contact> {
        self.contact(id)
            .or_else(|| self.visual.iter().find(|c| c.id == id))
    }
    /// Recorded observations for one target on the currently active channel.
    /// Samples from different channels stay separate even for the same object.
    pub fn trail(&self, id: u32) -> &[Sample] {
        self.active
            .and_then(|channel| {
                self.history
                    .iter()
                    .find(|t| t.id == id && t.channel == channel)
            })
            .map_or(&[][..], |t| &t.samples)
    }
    /// Which channel this aircraft can actually use for the requested control.
    pub fn available(&self, channel: Channel) -> bool {
        match channel {
            Channel::Radar => self.profiles.radar.is_some(),
            Channel::Infrared => self.profiles.infrared.is_some(),
            Channel::Visual => self.profiles.visual.is_some(),
        }
    }
    fn resolved_channel(&self) -> Option<Channel> {
        if self.controls.channel != Channel::Visual && self.available(self.controls.channel) {
            Some(self.controls.channel)
        } else if self.available(Channel::Radar) {
            Some(Channel::Radar)
        } else {
            None
        }
    }
    fn resolved_mode(&self, channel: Channel) -> Option<Mode> {
        self.mode_for(channel, &self.controls)
    }
    /// The mode a channel would report for these controls. Presentation uses
    /// it so a label never lags the player's own control by a step.
    pub fn mode_for(&self, channel: Channel, controls: &Controls) -> Option<Mode> {
        match channel {
            Channel::Infrared | Channel::Visual => Some(Mode::Infrared),
            Channel::Radar => self.profiles.radar.as_ref().map(|r| {
                if controls.range_nmi() <= r.track.maximum_nmi() {
                    Mode::Tws
                } else {
                    Mode::Rws
                }
            }),
        }
    }
    /// Whether the requested channel is installed and its equipment usable,
    /// from the equipment state of the last step.
    pub fn operating(&self, channel: Channel) -> bool {
        self.available(channel)
            && match channel {
                Channel::Radar => self.radar_powered && !self.radar_failed,
                Channel::Infrared => !self.infrared_failed,
                Channel::Visual => !self.visual_failed,
            }
    }
    /// Immediate persistent selection of a current radar or infrared contact's
    /// stable identity. Re-selecting the same target does not restart acquisition.
    pub fn designate(&mut self, id: u32) -> bool {
        if self.selected == Some(id) {
            return true;
        }
        if self.contact(id).is_none() {
            return false;
        }
        self.selected = Some(id);
        self.acquired = None;
        self.acquisition = 0;
        true
    }
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.acquired = None;
        self.acquisition = 0;
    }
    /// T and Shift-T: step through current radar contacts, nearest first,
    /// skipping wrecks and any identity `skip` rejects (friendly aircraft).
    /// Search-only RWS contacts stay selectable, as John requested.
    pub fn cycle(&mut self, forward: bool, skip: impl Fn(u32) -> bool) -> bool {
        let mut ranked: Vec<(f64, u32)> = self
            .contacts
            .iter()
            .filter(|c| c.channel == Channel::Radar && !c.destroyed && !skip(c.id))
            .map(|c| (c.distance_ft, c.id))
            .collect();
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let ids: Vec<u32> = ranked.into_iter().map(|(_, id)| id).collect();
        if ids.is_empty() {
            return false;
        }
        let next = match self
            .selected
            .and_then(|id| ids.iter().position(|v| *v == id))
        {
            Some(index) if forward => ids[(index + 1) % ids.len()],
            Some(index) => ids[(index + ids.len() - 1) % ids.len()],
            None if forward => ids[0],
            None => ids[ids.len() - 1],
        };
        self.designate(next)
    }
    /// Enter: the aircraft the pilot can see that is also a current radar or
    /// infrared contact, nearest the nose, inside the forward view.
    pub fn select_visual(
        &mut self,
        position: Vector,
        basis: Basis,
        skip: impl Fn(u32) -> bool,
    ) -> bool {
        let best = self
            .visual
            .iter()
            .filter(|c| !c.destroyed && !skip(c.id) && self.contact(c.id).is_some())
            .filter_map(|c| {
                let d = std::array::from_fn(|i| c.position[i] - position[i]);
                let z = dot(d, basis.forward);
                (z > 0.
                    && (dot(d, basis.right) / z).abs() <= FORWARD_VIEW_HALF_WIDTH.tan()
                    && (dot(d, basis.up) / z).abs() <= FORWARD_VIEW_HALF_HEIGHT.tan())
                .then(|| ((z / dot(d, d).sqrt()).clamp(-1., 1.).acos(), c.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        best.is_some_and(|(_, id)| self.designate(id))
    }
    /// Whether current radar data supports a weapon aimed at this specific
    /// target. Infrared selection never confers radar illumination.
    pub fn support(&self, id: u32) -> Support {
        if self.profiles.radar.is_none() {
            return Support::Unavailable;
        }
        if self.radar_failed {
            return Support::RadarFailed;
        }
        if !self.radar_powered || self.active != Some(Channel::Radar) {
            return Support::RadarOff;
        }
        if self.acquired == Some(id) {
            return Support::Tracked;
        }
        if self.selected != Some(id) {
            return Support::NotSelected;
        }
        let Some(contact) = self.contact(id) else {
            return Support::NoObservation;
        };
        if self.mode == Some(Mode::Rws) {
            return Support::SearchOnly;
        }
        if !contact.track_eligible {
            return Support::TrackCoverage;
        }
        Support::Acquiring
    }
    pub fn supports(&self, id: u32) -> bool {
        self.support(id).supported()
    }
    /// Tracking state on the channel currently in use, for the scope readout.
    /// Only the radar answer is a weapon permission; see `support`.
    pub fn track_status(&self, id: u32) -> Support {
        match self.active {
            Some(Channel::Radar) | None => self.support(id),
            Some(_) => {
                if self.acquired == Some(id) {
                    Support::Tracked
                } else if self.selected != Some(id) {
                    Support::NotSelected
                } else {
                    match self.contact(id) {
                        None => Support::NoObservation,
                        Some(c) if c.track_eligible => Support::Acquiring,
                        Some(_) => Support::TrackCoverage,
                    }
                }
            }
        }
    }
    /// One fixed 120 Hz step. Pausing means not calling this method.
    pub fn step(
        &mut self,
        observer: &Observer,
        targets: &[Observable],
        environment: &Environment<'_>,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        self.radar_powered = observer.radar_powered;
        self.radar_failed = observer.radar_failed;
        self.infrared_failed = observer.infrared_failed;
        self.visual_failed = observer.visual_failed;
        let channel = self.resolved_channel().filter(|c| match c {
            Channel::Radar => observer.radar_powered && !observer.radar_failed,
            Channel::Infrared => !observer.infrared_failed,
            // The scope channel never resolves to the visual sensor.
            Channel::Visual => false,
        });
        let mode = channel.and_then(|c| self.resolved_mode(c));
        // Changing channel or leaving TWS releases the old fire-control track
        // immediately and restarts acquisition. A still observed selection is
        // preserved by the ordinary observation test below.
        if channel != self.active || mode != self.mode {
            if let Some(id) = self.acquired.take() {
                events.push(Event::TrackReleased(id));
            }
            self.acquisition = 0;
            // Coasting plots belong to the channel that observed them, so a
            // channel change retires them rather than replotting them here.
            if channel != self.active {
                self.plots.clear();
            }
        }
        self.active = channel;
        self.mode = mode;
        let previous = std::mem::take(&mut self.contacts);
        self.strobes.clear();
        if let (Some(Channel::Radar), Some(radar)) = (channel, self.profiles.radar.as_ref()) {
            self.strobes = strobes(observer, targets, radar, environment);
        }
        // Presentation keeps a briefly fading copy of each emitter. Losing the
        // radar hides its noise at once rather than fading it out.
        for live in &self.strobes {
            match self.fading.iter_mut().find(|(s, _)| s.id == live.id) {
                Some(entry) => *entry = (*live, 0),
                None => self.fading.push((*live, 0)),
            }
        }
        let live: Vec<u32> = self.strobes.iter().map(|s| s.id).collect();
        let emitting = channel == Some(Channel::Radar);
        self.fading.retain_mut(|(strobe, age)| {
            if live.contains(&strobe.id) {
                return true;
            }
            *age += 1;
            emitting && *age <= NOISE_FADE_STEPS
        });
        let mut sorted: Vec<&Observable> = targets.iter().collect();
        sorted.sort_by_key(|t| t.id);
        self.visual.clear();
        self.map_contacts.clear();
        let visual = !observer.visual_failed;
        for target in &sorted {
            let sensed =
                channel.and_then(|channel| self.observe(observer, target, channel, environment));
            let seen = visual
                .then(|| self.observe(observer, target, Channel::Visual, environment))
                .flatten();
            if let Some(contact) = seen.as_ref().or(sensed.as_ref()) {
                self.map_contacts.push(MapContact {
                    contact: *contact,
                    identified: seen.is_some(),
                    airborne: target.airborne,
                });
            }
            // Surface returns are map-only and never enter targeting or the scope.
            if target.airborne {
                self.contacts.extend(sensed);
                self.visual.extend(seen);
            }
        }
        // Selection survives only while the radar or infrared scope still
        // holds the contact; seeing it no longer keeps it (John, 2026-09-23).
        if let Some(id) = self.selected
            && self.contact(id).is_none()
        {
            self.selected = None;
            self.acquisition = 0;
            if let Some(acquired) = self.acquired.take() {
                events.push(Event::TrackReleased(acquired));
            }
            events.push(Event::SelectionCleared(id));
        }
        let eligible = self.selected.is_some_and(|id| {
            self.mode.is_some_and(Mode::tracks)
                && self.contact(id).is_some_and(|c| c.track_eligible)
        });
        if eligible {
            self.acquisition = self.acquisition.saturating_add(1);
            if self.acquisition >= ACQUISITION_STEPS && self.acquired != self.selected {
                self.acquired = self.selected;
                events.push(Event::TrackAcquired(self.selected.expect("selected")));
            }
        } else {
            if let Some(id) = self.acquired.take() {
                events.push(Event::TrackReleased(id));
            }
            self.acquisition = 0;
        }
        for lost in previous
            .iter()
            .filter(|c| !self.contacts.iter().any(|current| current.id == c.id))
        {
            events.push(Event::ContactLost(lost.id));
            self.plots.retain(|p| p.id != lost.id);
            self.plots.push(Plot {
                id: lost.id,
                channel: lost.channel,
                bearing_rad: lost.bearing_rad,
                elevation_rad: lost.elevation_rad,
                distance_ft: lost.distance_ft,
                position: lost.position,
                age: 0,
            });
        }
        for plot in &mut self.plots {
            plot.age += 1;
        }
        self.plots
            .retain(|p| p.age <= STALE_STEPS && !self.contacts.iter().any(|c| c.id == p.id));
        self.record_history(channel);
        self.tick += 1;
        events
    }
    fn record_history(&mut self, channel: Option<Channel>) {
        // History is collected whether or not it is displayed, so enabling it
        // shows the recent observations immediately.
        if let Some(channel) = channel
            && self.tick.is_multiple_of(HISTORY_INTERVAL_STEPS)
        {
            let tick = self.tick;
            for contact in &self.contacts {
                let trail = match self
                    .history
                    .iter_mut()
                    .position(|t| t.id == contact.id && t.channel == channel)
                {
                    Some(index) => &mut self.history[index],
                    None => {
                        self.history.push(Trail {
                            channel,
                            id: contact.id,
                            samples: Vec::new(),
                        });
                        self.history.last_mut().expect("pushed trail")
                    }
                };
                trail.samples.push(Sample {
                    tick,
                    position: contact.position,
                });
                if trail.samples.len() > HISTORY_SAMPLES {
                    trail.samples.remove(0);
                }
            }
        }
        let tick = self.tick;
        for trail in &mut self.history {
            trail
                .samples
                .retain(|s| tick.saturating_sub(s.tick) <= HISTORY_AGE_STEPS);
        }
        self.history.retain(|t| !t.samples.is_empty());
    }
    fn observe(
        &self,
        observer: &Observer,
        target: &Observable,
        channel: Channel,
        environment: &Environment<'_>,
    ) -> Option<Contact> {
        let sighting = Sighting::new(observer.position, &observer.basis, target.position);
        if !sighting.distance_ft.is_finite()
            || (environment.obscured)(observer.position, target.position)
        {
            return None;
        }
        let (search, track) = match channel {
            Channel::Radar => {
                let radar = self.profiles.radar.as_ref()?;
                let to_observer: Vector =
                    std::array::from_fn(|i| observer.position[i] - target.position[i]);
                let effective = target.signature.effective_radar(
                    &target.basis,
                    to_observer,
                    target.configuration,
                );
                if effective <= 0. {
                    return None;
                }
                let relative = effective / 100.;
                let height = target.position[1]
                    - (environment.ground)(target.position[0], target.position[2]);
                let clutter = clutter_exposure(&sighting, height);
                let look_down = look_down_factor(radar.look_down, clutter);
                let notch = notch_factor(
                    &radar.notch,
                    clutter,
                    notch_speed_fps(&sighting, target.velocity),
                );
                let coupled: f64 = self
                    .strobes
                    .iter()
                    .map(|s| {
                        s.received
                            * angular_coupling(
                                &radar.resistance,
                                dot(s.line_of_sight, sighting.line_of_sight)
                                    .clamp(-1., 1.)
                                    .acos(),
                            )
                    })
                    .sum();
                let jammer = jammer_factor(interference_quotient(
                    &radar.resistance,
                    coupled,
                    sighting.distance_nmi(),
                    relative,
                ));
                (
                    (
                        &radar.search,
                        effective_range_ft(
                            radar.search.maximum_ft,
                            relative,
                            look_down,
                            notch,
                            jammer,
                        ),
                    ),
                    (
                        &radar.track,
                        effective_range_ft(
                            radar.track.maximum_ft,
                            relative,
                            look_down,
                            notch,
                            jammer,
                        ),
                    ),
                )
            }
            Channel::Infrared => {
                let infrared = self.profiles.infrared.as_ref()?;
                let signature = target.signature.infrared;
                (
                    (
                        &infrared.search,
                        infrared_range_ft(infrared.search.maximum_ft, signature),
                    ),
                    (
                        &infrared.track,
                        infrared_range_ft(infrared.track.maximum_ft, signature),
                    ),
                )
            }
            // The visual channel keeps the existing geometric contract: no
            // signature scaling, no interference and no weapon support.
            Channel::Visual => {
                let visual = self.profiles.visual.as_ref()?;
                (
                    (&visual.search, visual.search.maximum_ft),
                    (&visual.track, visual.track.maximum_ft),
                )
            }
        };
        let admits = |(volume, effective): (&super::profile::Volume, f64)| {
            volume.admits_geometry(
                sighting.azimuth_rad,
                sighting.elevation_rad,
                sighting.distance_ft,
                sighting.relative_altitude_ft,
            ) && sighting.distance_ft <= effective
        };
        let in_track = admits(track);
        // A retained fire-control track may stay current inside its own
        // tracking envelope, which can reach past the search envelope. It can
        // never acquire an unseen target there.
        let retained = self.selected == Some(target.id) && self.acquired == Some(target.id);
        if !(admits(search) || (retained && in_track)) {
            return None;
        }
        Some(Contact {
            id: target.id,
            channel,
            bearing_rad: sighting.azimuth_rad,
            elevation_rad: sighting.elevation_rad,
            distance_ft: sighting.distance_ft,
            position: target.position,
            velocity: target.velocity,
            track_eligible: in_track,
            destroyed: target.destroyed,
        })
    }
}

/// Received emitters at the radar. A jammer beyond the selected display range
/// still interferes; nothing here requires the emitter to be a known target.
fn strobes(
    observer: &Observer,
    targets: &[Observable],
    radar: &RadarProfile,
    environment: &Environment<'_>,
) -> Vec<Strobe> {
    let mut sorted: Vec<&Observable> = targets.iter().collect();
    sorted.sort_by_key(|t| t.id);
    let mut strobes = Vec::new();
    for target in sorted {
        let Some(jammer) = target.jammer.as_ref().filter(|_| target.jammer_active) else {
            continue;
        };
        if !target.airborne || (environment.obscured)(observer.position, target.position) {
            continue;
        }
        let sighting = Sighting::new(observer.position, &observer.basis, target.position);
        // Receiver coverage gates the noise; the range bounds do not, because
        // passive noise carries no range information.
        if sighting.azimuth_rad.abs() > radar.search.azimuth_rad
            || sighting.elevation_rad.abs() > radar.search.elevation_rad
        {
            continue;
        }
        let received = received(jammer, radar, sighting.distance_nmi());
        if received <= 0. {
            continue;
        }
        strobes.push(Strobe {
            id: target.id,
            bearing_rad: sighting.azimuth_rad,
            elevation_rad: sighting.elevation_rad,
            received,
            half_width_rad: radar.resistance.coupling_rad,
            sidelobe_floor: radar.resistance.sidelobe_floor,
            line_of_sight: sighting.line_of_sight,
        });
    }
    strobes
}
