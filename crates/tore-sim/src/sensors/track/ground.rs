//! Ground contacts: the A-G page (Ctrl+R), the ground objects of the IR page
//! and the HARM page's emitters (docs/spec/air-to-ground.md, "The
//! air-to-ground radar"; evidence in docs/formats/radar.md, "A-G page" and
//! "HARM page").
//!
//! One list serves the page in use. It is rebuilt when the page changes and
//! then every 6 seconds (every second on the HARM page); between rebuilds the
//! listed contacts follow their objects every quarter second. Retail data
//! decides what is listed: the class word, the type's listing flag 0x20 and
//! visibility distance, airport sequences for aircraft, emission for the HARM
//! page, the page's range box, the sensor's volume, its range scaled by the
//! object's signature and the weather, and a clear terrain line. Nothing here
//! compares a speed: the manual's 90 kt split is not in the code.
//!
//! What TORE adds over the retail page (labelled in the spec): T skips
//! friendlies, a scope click and Enter designate from the list, and a ground
//! designation is kept while the list holds it, the pilot sees it, the
//! targeting sight's track holds it or Easy targeting keeps it.
use super::{Basis, Channel, Contact, Environment, Event, Mode, Observer, Sensors, Vector, dot};
use crate::sensors::detection::Sighting;
use crate::sensors::profile::{FEET_PER_NAUTICAL_MILE, Volume};

/// Object class words (docs/formats/debrief.md, "Outcome").
pub mod class {
    pub const AIRCRAFT: u16 = 0xc000;
    pub const SHIP: u16 = 0x2000;
    pub const SAM: u16 = 0x1000;
    pub const AAA: u16 = 0x0800;
    /// SAM or AAA: drawn as the X.
    pub const DEFENSE: u16 = SAM | AAA;
}

/// The weather effect selectors, which equal the seeker signature
/// (docs/formats/radar.md, "A-G page").
pub mod selector {
    pub const VISUAL: usize = 0;
    pub const LASER: usize = 1;
    pub const INFRARED: usize = 2;
    pub const RADAR: usize = 3;
    pub const ANTI_RADIATION: usize = 4;
}

/// Entries the page list holds (retail: 50 of 0x8c bytes).
pub const MAX_GROUND_CONTACTS: usize = 50;
/// The A-G and IR pages rebuild their list every 6 seconds.
pub const GROUND_REBUILD_STEPS: u64 = 720;
/// The HARM page rebuilds every second.
pub const HARM_REBUILD_STEPS: u64 = 120;
/// Listed contacts follow their objects every quarter second.
pub const GROUND_REFRESH_STEPS: u64 = 30;
/// The range boxes' caps: 0x42c, 0x2c8 and 0x76 units of 256 ft (45.0, 30.0
/// and 4.97 nmi).
pub const AG_CAP_FT: f64 = 1068. * 256.;
pub const HARM_CAP_FT: f64 = 712. * 256.;
pub const PAGE_CAP_FT: f64 = 118. * 256.;
/// The listing flag in a type's OBJECT `flags`: the host sets
/// [`GroundObject::listed`] from it.
pub const LISTED_FLAG: u32 = 0x20;
/// The A-G page lists a ground object only when its `maxVisDist` is this or
/// more (`maxVisDist` x 256 >= 19,000 ft).
pub const MIN_VISIBILITY: u16 = 75;
/// `maxVisDist` units, feet.
pub const VISIBILITY_UNIT_FT: f64 = 256.;
/// On the A-G page a ground object within its range / 20 of a listed one
/// merges into it.
pub const MERGE_DIVISOR: f64 = 20.;
/// The HARM page drops aircraft below this sea-level altitude (0xbd00).
pub const AIR_FLOOR_FT: f64 = 189.;
/// The look-down penalty applies to targets within this height of the
/// terrain, in full on the ground.
pub const LOOK_DOWN_HEIGHT_FT: f64 = 5000.;

/// How a page draws a contact (one window-font character).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundSymbol {
    /// A 2 by 2 dot: ground units, structures and ships.
    Dot,
    /// A 5 by 5 X: SAM and AAA units.
    Cross,
    /// A 5 by 5 filled square: aircraft.
    Square,
}

/// One listed contact of a ground page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundContact {
    /// Where it is, as the page sees it. The channel is the page's: radar
    /// for A-G, infrared or HARM.
    pub contact: Contact,
    /// Its class word.
    pub class: u16,
    /// An aircraft (listed only in an airport sequence on A-G, or emitting
    /// on the HARM page).
    pub aircraft: bool,
    /// The pilot can see it: inside the visual sensor's volume with a clear
    /// line. Enter picks among these.
    pub visible: bool,
}
impl GroundContact {
    pub fn id(&self) -> u32 {
        self.contact.id
    }
    pub fn symbol(&self) -> GroundSymbol {
        if self.aircraft {
            GroundSymbol::Square
        } else if self.class & class::DEFENSE != 0 {
            GroundSymbol::Cross
        } else {
            GroundSymbol::Dot
        }
    }
    /// The designation bars stand 14 pixels apart around an aircraft or a SAM
    /// or AAA unit, 10 around anything else (window font 0x04 and 0x05).
    pub fn wide_bars(&self) -> bool {
        self.symbol() != GroundSymbol::Dot
    }
}

/// One object a ground page may list, as the host describes it each step.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundObject {
    pub id: u32,
    pub position: Vector,
    pub velocity: Vector,
    /// The class word.
    pub class: u16,
    /// The type carries the listing flag 0x20.
    pub listed: bool,
    /// `maxVisDist`, raw (256 ft units).
    pub max_visible: u16,
    /// Radar (`sigs[3]`) and infrared (`sigs[2]`) signatures, percent.
    pub radar: f64,
    pub infrared: f64,
    /// Its radar is on, or its jammer: an anti-radiation seeker receives it.
    pub emitting: bool,
    /// An aircraft, flying or parked.
    pub aircraft: bool,
    /// Physically airborne: a flying aircraft's row. The air channels see it.
    pub airborne: bool,
    /// An aircraft in an airfield's takeoff or landing sequence.
    pub airfield: bool,
    pub destroyed: bool,
}

/// One weather layer's sensor effects over an altitude span.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeatherBand {
    pub low_ft: f64,
    pub high_ft: f64,
    /// The five effect selectors, percent.
    pub effects: [u8; 5],
}

/// The active weather layers' sensor effects, from the host's weather.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    pub bands: Vec<WeatherBand>,
}
impl Weather {
    /// The effect across an altitude span, percent: the least of the layers
    /// the span meets, at most 100, and 100 with none (`_WRWeatherEffects`).
    pub fn percent(&self, selector: usize, a_ft: f64, b_ft: f64) -> f64 {
        let (low, high) = (a_ft.min(b_ft), a_ft.max(b_ft));
        let value = self
            .bands
            .iter()
            .filter(|band| band.high_ft >= low && band.low_ft <= high)
            .filter_map(|band| band.effects.get(selector).copied())
            .fold(100, u8::min);
        f64::from(value.min(100))
    }
}

impl Sensors {
    /// The page list, nearest first.
    pub fn ground(&self) -> &[GroundContact] {
        &self.ground
    }
    pub fn ground_contact(&self, id: u32) -> Option<&GroundContact> {
        self.ground.iter().find(|g| g.contact.id == id)
    }
    /// The A-G flag. The page shows while the radar is the scope channel.
    pub fn air_ground(&self) -> bool {
        self.air_ground
    }
    /// Ctrl+R: A-G on or off. Turning it on needs a working radar ("No working
    /// radar found" otherwise). It never touches the designated target.
    /// Returns whether it is on afterwards.
    pub fn toggle_air_ground(&mut self) -> bool {
        if self.air_ground {
            self.air_ground = false;
        } else if self.profiles.radar.is_some() && !self.radar_failed {
            self.air_ground = true;
        }
        self.air_ground
    }
    /// The HARM page's seeker volume.
    pub fn harm_seeker(&self) -> Option<&Volume> {
        self.harm_seeker.as_ref()
    }
    pub fn set_harm_seeker(&mut self, seeker: Option<Volume>) {
        self.harm_seeker = seeker;
    }
    /// The page in use, if it lists ground objects.
    pub fn ground_page(&self) -> Option<Mode> {
        match (self.active, self.mode) {
            (Some(Channel::Radar), Some(Mode::AirGround)) => Some(Mode::AirGround),
            (Some(Channel::Infrared), _) => Some(Mode::Infrared),
            (Some(Channel::Harm), _) => Some(Mode::Harm),
            _ => None,
        }
    }
    /// The targeting sight's track designates `id`, whatever the channels see,
    /// for as long as the track holds it.
    pub fn hold(&mut self, id: u32) {
        if self.selected != Some(id) {
            self.selected = Some(id);
            self.acquired = None;
            self.acquisition = 0;
        }
        self.held = Some(id);
        self.ground_retained = None;
    }
    /// The selection the sight's track holds.
    pub fn held(&self) -> Option<u32> {
        self.held
    }
    /// Ctrl+X under Easy targeting: designate `id` directly. Easy targeting
    /// keeps it while the object stands.
    pub fn select_ground(&mut self, id: u32) {
        if self.selected != Some(id) {
            self.selected = Some(id);
            self.acquired = None;
            self.acquisition = 0;
        }
        self.held = None;
        self.ground_retained = Some(id);
    }

    /// One fixed step of the ground pages, after [`Sensors::step`] with the
    /// same observer: rebuild or refresh the list, then keep or drop a ground
    /// selection. `objects` are every object a page may list, in any order.
    pub fn step_ground(
        &mut self,
        observer: &Observer,
        objects: &[GroundObject],
        environment: &Environment<'_>,
        weather: &Weather,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        // `step` advanced the tick; this step is the one before it.
        let tick = self.tick.saturating_sub(1);
        let page = self.ground_page();
        let interval = if page == Some(Mode::Harm) {
            HARM_REBUILD_STEPS
        } else {
            GROUND_REBUILD_STEPS
        };
        if page != self.ground_page || tick.saturating_sub(self.ground_built) >= interval {
            self.ground_page = page;
            self.ground_built = tick;
            self.rebuild_ground(page, observer, objects, environment, weather);
        } else if tick
            .saturating_sub(self.ground_built)
            .is_multiple_of(GROUND_REFRESH_STEPS)
        {
            self.refresh_ground(observer, objects, environment);
        }
        self.ground_retained = None;
        if let Some(id) = self.selected
            && self.contact(id).is_none()
        {
            let object = objects.iter().find(|o| o.id == id);
            let retained = self.held == Some(id)
                || self.ground_contact(id).is_some()
                || object.is_some_and(|o| {
                    if o.airborne {
                        // The air rules decided it in `step`.
                        true
                    } else {
                        (self.keep_selection && !o.destroyed) || self.sees(observer, o, environment)
                    }
                });
            if retained {
                self.ground_retained = Some(id);
            } else {
                self.selected = None;
                self.held = None;
                self.acquisition = 0;
                if let Some(acquired) = self.acquired.take() {
                    events.push(Event::TrackReleased(acquired));
                }
                events.push(Event::SelectionCleared(id));
            }
        }
        if self.held.is_some() && self.held != self.selected {
            self.held = None;
        }
        events
    }

    /// Whether the pilot sees `object`: inside the visual volume and range,
    /// with a clear terrain line.
    fn sees(
        &self,
        observer: &Observer,
        object: &GroundObject,
        environment: &Environment<'_>,
    ) -> bool {
        let Some(visual) = self
            .profiles
            .visual
            .as_ref()
            .filter(|_| !observer.visual_failed)
        else {
            return false;
        };
        let s = Sighting::new(observer.position, &observer.basis, object.position);
        s.distance_ft.is_finite()
            && visual.search.admits_geometry(
                s.azimuth_rad,
                s.elevation_rad,
                s.distance_ft,
                s.relative_altitude_ft,
            )
            && s.distance_ft <= visual.search.maximum_ft
            && !(environment.obscured)(observer.position, object.position)
    }

    fn rebuild_ground(
        &mut self,
        page: Option<Mode>,
        observer: &Observer,
        objects: &[GroundObject],
        environment: &Environment<'_>,
        weather: &Weather,
    ) {
        self.ground.clear();
        let Some(page) = page else {
            return;
        };
        let (volume, cap, channel) = match page {
            Mode::AirGround => match self.profiles.radar.as_ref() {
                Some(radar) => (radar.search, AG_CAP_FT, Channel::Radar),
                None => return,
            },
            Mode::Infrared => match self.profiles.infrared.as_ref() {
                Some(infrared) => (infrared.search, PAGE_CAP_FT, Channel::Infrared),
                None => return,
            },
            Mode::Harm => match self.harm_seeker {
                Some(seeker) => (seeker, HARM_CAP_FT, Channel::Harm),
                None => return,
            },
            Mode::Rws | Mode::Tws => return,
        };
        let look_down = self.profiles.radar.as_ref().map_or(0., |r| r.look_down);
        let range_ft = self.controls.range_nmi() * FEET_PER_NAUTICAL_MILE;
        // Box 1: twice the selected range, at most the sensor's search range;
        // box 2 caps it for the page.
        let box_one = (2. * range_ft).min(volume.maximum_ft);
        let box_two = box_one.min(cap);
        let mut candidates: Vec<(f64, u32, &GroundObject, Sighting)> = Vec::new();
        for object in objects.iter().filter(|o| !o.destroyed) {
            let Some(half) = eligible(page, object, box_one, box_two) else {
                continue;
            };
            let dx = object.position[0] - observer.position[0];
            let dz = object.position[2] - observer.position[2];
            if dx.abs() > half || dz.abs() > half {
                continue;
            }
            let s = Sighting::new(observer.position, &observer.basis, object.position);
            if !s.distance_ft.is_finite()
                || !volume.admits_geometry(
                    s.azimuth_rad,
                    s.elevation_rad,
                    s.distance_ft,
                    s.relative_altitude_ft,
                )
            {
                continue;
            }
            if page == Mode::AirGround
                && !object.aircraft
                && object.class & class::SHIP == 0
                && (object.max_visible < MIN_VISIBILITY
                    || s.distance_ft > 3. * f64::from(object.max_visible) * VISIBILITY_UNIT_FT)
            {
                continue;
            }
            // Range scaled by the effective signature: distance x 100 /
            // signature within the search range.
            let span = |selector: usize| {
                weather.percent(selector, observer.position[1], object.position[1]) / 100.
            };
            let signature = match page {
                Mode::AirGround => {
                    let height = object.position[1]
                        - (environment.ground)(object.position[0], object.position[2]);
                    let mut penalty = if height < LOOK_DOWN_HEIGHT_FT {
                        (LOOK_DOWN_HEIGHT_FT - height.max(0.)) / LOOK_DOWN_HEIGHT_FT * look_down
                    } else {
                        0.
                    };
                    if object.class & class::SHIP != 0 {
                        penalty /= 3.;
                    }
                    object.radar * span(selector::RADAR) - penalty
                }
                Mode::Infrared => object.infrared * span(selector::INFRARED),
                _ => 100. * span(selector::ANTI_RADIATION),
            };
            if signature <= 0. || s.distance_ft * 100. / signature > volume.maximum_ft {
                continue;
            }
            candidates.push((s.distance_ft, object.id, object, s));
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (distance, _, object, s) in candidates {
            if self.ground.len() >= MAX_GROUND_CONTACTS {
                break;
            }
            if page == Mode::AirGround
                && self.ground.iter().any(|listed| {
                    listed.aircraft == object.aircraft
                        && length(sub(listed.contact.position, object.position))
                            <= distance / MERGE_DIVISOR
                })
            {
                continue;
            }
            if (environment.obscured)(observer.position, object.position) {
                continue;
            }
            self.ground.push(GroundContact {
                contact: Contact {
                    id: object.id,
                    channel,
                    bearing_rad: s.azimuth_rad,
                    elevation_rad: s.elevation_rad,
                    distance_ft: s.distance_ft,
                    position: object.position,
                    velocity: object.velocity,
                    track_eligible: false,
                    destroyed: object.destroyed,
                },
                class: object.class,
                aircraft: object.aircraft,
                visible: self.sees(observer, object, environment),
            });
        }
    }

    /// Listed contacts follow their objects; one whose object is gone leaves.
    fn refresh_ground(
        &mut self,
        observer: &Observer,
        objects: &[GroundObject],
        environment: &Environment<'_>,
    ) {
        let mut list = std::mem::take(&mut self.ground);
        list.retain_mut(|listed| {
            let Some(object) = objects.iter().find(|o| o.id == listed.contact.id) else {
                return false;
            };
            let s = Sighting::new(observer.position, &observer.basis, object.position);
            listed.contact.bearing_rad = s.azimuth_rad;
            listed.contact.elevation_rad = s.elevation_rad;
            listed.contact.distance_ft = s.distance_ft;
            listed.contact.position = object.position;
            listed.contact.velocity = object.velocity;
            listed.contact.destroyed = object.destroyed;
            listed.visible = self.sees(observer, object, environment);
            true
        });
        self.ground = list;
    }

    /// T and Shift+T on a ground page (retail order): T picks the nearest
    /// contact beyond the current one, wrapping to the nearest; Shift+T the
    /// farthest nearer than the current one, wrapping to the farthest. Only
    /// contacts on the scope (within the selected range) and alive count;
    /// friendlies are skipped (TORE's air rule).
    pub(super) fn cycle_ground(&mut self, forward: bool, skip: impl Fn(u32) -> bool) -> bool {
        let range_ft = self.controls.range_nmi() * FEET_PER_NAUTICAL_MILE;
        let ranked: Vec<(f64, u32)> = self
            .ground
            .iter()
            .filter(|g| !g.contact.destroyed && g.contact.distance_ft <= range_ft && !skip(g.id()))
            .map(|g| (g.contact.distance_ft, g.id()))
            .collect();
        let current = self
            .selected
            .and_then(|id| self.ground_contact(id))
            .map(|g| (g.contact.distance_ft, g.id()));
        let order = |a: &(f64, u32), b: &(f64, u32)| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1));
        let pick = if forward {
            current
                .and_then(|c| {
                    ranked
                        .iter()
                        .filter(|r| order(r, &c).is_gt())
                        .min_by(|a, b| order(a, b))
                })
                .or_else(|| ranked.iter().min_by(|a, b| order(a, b)))
        } else {
            current
                .and_then(|c| {
                    ranked
                        .iter()
                        .filter(|r| order(r, &c).is_lt())
                        .max_by(|a, b| order(a, b))
                })
                .or_else(|| ranked.iter().max_by(|a, b| order(a, b)))
        };
        pick.is_some_and(|&(_, id)| self.designate(id))
    }

    /// Enter and ' with no aircraft to pick: the visible ground contacts of
    /// the page inside the forward view. The first press takes the one nearest
    /// the centre; pressing again steps to the next one to the right, wrapping
    /// to the leftmost (Enter's retail left-to-right cycle; one action serves
    /// both keys, opinionated, agent).
    pub(super) fn select_visual_ground(
        &mut self,
        position: Vector,
        basis: Basis,
        skip: &impl Fn(u32) -> bool,
    ) -> bool {
        let mut seen: Vec<(f64, f64, u32)> = self
            .ground
            .iter()
            .filter(|g| g.visible && !g.contact.destroyed && !skip(g.id()))
            .filter_map(|g| {
                let d = sub(g.contact.position, position);
                let z = dot(d, basis.forward);
                let x = dot(d, basis.right) / z;
                let y = dot(d, basis.up) / z;
                (z > 0.
                    && x.abs() <= super::FORWARD_VIEW_HALF_WIDTH.tan()
                    && y.abs() <= super::FORWARD_VIEW_HALF_HEIGHT.tan())
                .then(|| (x, (z / length(d)).clamp(-1., 1.).acos(), g.id()))
            })
            .collect();
        if seen.is_empty() {
            return false;
        }
        seen.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
        let next = match self
            .selected
            .and_then(|id| seen.iter().position(|s| s.2 == id))
        {
            Some(index) => seen[(index + 1) % seen.len()].2,
            None => {
                seen.iter()
                    .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.cmp(&b.2)))
                    .expect("not empty")
                    .2
            }
        };
        self.designate(next)
    }
}

/// The box a page lists `object` in (half side, feet), or `None` when the
/// page never lists it.
fn eligible(page: Mode, object: &GroundObject, box_one: f64, box_two: f64) -> Option<f64> {
    match page {
        Mode::AirGround => {
            if object.aircraft {
                // Aircraft only while attached to an airport.
                object.airfield.then_some(box_two)
            } else {
                (object.listed || object.class & class::SHIP != 0).then_some(box_two)
            }
        }
        // Flying aircraft are the air channel's; ground objects, parked
        // aircraft and ships are listed.
        Mode::Infrared => (!object.airborne && object.listed).then_some(box_two),
        Mode::Harm => {
            if !object.emitting || object.class & class::SHIP != 0 {
                None
            } else if object.airborne {
                (object.position[1] >= AIR_FLOOR_FT && !object.airfield).then_some(box_one)
            } else {
                object.listed.then_some(box_two)
            }
        }
        Mode::Rws | Mode::Tws => None,
    }
}

fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}
fn length(v: Vector) -> f64 {
    dot(v, v).sqrt()
}
