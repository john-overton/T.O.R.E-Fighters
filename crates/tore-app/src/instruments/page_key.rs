//! Page dependency keys. Each page's key holds what the page draws, at the
//! precision it draws it: the text as formatted, symbols as plotted pixel
//! positions, blink and noise phases, colours, button presses and the live
//! palette. Two equal keys mean equal rasters (for the same airframe font
//! and frame, which the flight canvas checks itself), so the canvas keeps the
//! page's pixels instead of drawing it again. Debug builds draw it anyway and
//! compare; see `flight_canvas::prepare_panel`.
//!
//! Every branch here follows the matching branch of [`Instruments::page`];
//! a change to what a page draws needs the same change here.
use super::*;
use std::fmt::Write as _;

/// What a page drew from; see the module notes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageKey(Vec<u8>);

/// Writes a key's parts. Every part has a fixed size or a length, so two
/// different sequences of parts never give the same bytes.
#[derive(Default)]
pub(super) struct Key(Vec<u8>);
impl Key {
    pub(super) fn tag(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    pub(super) fn flag(&mut self, v: bool) -> &mut Self {
        self.tag(u8::from(v))
    }
    pub(super) fn u64(&mut self, v: u64) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub(super) fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    /// The exact value: distinct bits, distinct keys (so 0 and -0 differ).
    pub(super) fn f64(&mut self, v: f64) -> &mut Self {
        self.u64(v.to_bits())
    }
    pub(super) fn point(&mut self, (x, y): (i32, i32)) -> &mut Self {
        self.i32(x).i32(y)
    }
    pub(super) fn colour(&mut self, c: [u8; 4]) -> &mut Self {
        self.0.extend_from_slice(&c);
        self
    }
    pub(super) fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.u64(v.len() as u64);
        self.0.extend_from_slice(v);
        self
    }
    pub(super) fn text(&mut self, v: &str) -> &mut Self {
        self.bytes(v.as_bytes())
    }
    /// A value through its `Debug` text, which writes every field and each
    /// float exactly (shortest round trip, -0 as -0).
    pub(super) fn debug(&mut self, v: &impl std::fmt::Debug) -> &mut Self {
        let mut text = String::new();
        let _ = write!(text, "{v:?}");
        self.text(&text)
    }
}

fn plot(k: &mut Key, plot: RwrPlot) {
    match plot {
        RwrPlot::Ranged(x, y) => k.tag(0).point((x, y)),
        RwrPlot::BearingOnly { rim, inner } => k.tag(1).point(rim).point(inner),
        RwrPlot::Clipped { rim, inner } => k.tag(2).point(rim).point(inner),
    };
}

/// The link marks `draw_link_marks` draws for one contact.
fn link_marks(k: &mut Key, marks: Option<&scope::LinkMarks>, tick: u64) {
    let Some(marks) = marks else {
        k.tag(0);
        return;
    };
    k.tag(1).bytes(&marks.locked_by).bytes(&marks.assigned_to);
    match marks.assigned {
        None => k.tag(0),
        Some(true) => k.tag(1),
        Some(false) => k.tag(2).flag(scope::link_blink_on(tick)),
    };
}

impl Instruments {
    /// The key of page `id` drawn now ([`Instruments::page`] with the
    /// flying airframe), or `None` for a page that always draws.
    pub fn page_key(&self, id: u8, s: &State) -> Option<PageKey> {
        let mut k = Key::default();
        // The window: frame and labels through the palette, the press square.
        k.tag(id);
        for colour in &self.palette {
            k.0.extend_from_slice(colour);
        }
        let slot = self.pages.iter().position(|p| *p == id);
        for b in 0..BUTTON_X.len() {
            k.flag(slot.is_some_and(|slot| self.pressed == Some((slot, b))));
        }
        if id == 6 {
            k.flag(self.navigation.airports_mode);
        }
        let blank = (id == 9 && s.systems.has(32))
            || (id == 5
                && (s.systems.counts[32] > 1
                    || self
                        .combat
                        .as_ref()
                        .is_some_and(|c| c.rwr_failed || !c.rwr.operating)));
        k.flag(blank);
        if blank {
            return Some(PageKey(k.0));
        }
        if id == 6 {
            k.flag(s.systems.has(33));
            if s.systems.has(33) {
                return Some(PageKey(k.0));
            }
        }
        match id {
            0 => self.rcs_key(&mut k),
            7 => systems_key(&mut k, s),
            5 => self.rwr_key(&mut k, s),
            9 => self.radar_key(&mut k),
            8 => self.weapons_key(&mut k),
            6 => self.nav_key(&mut k, s),
            1 => envelope::key(
                &mut k,
                s.retail_envelopes(),
                s,
                self.envelope_mode,
                self.combat.as_ref(),
            ),
            4 => self.target_key(&mut k),
            2 | 3 => self.camera_key(&mut k, id, s),
            _ => {}
        }
        Some(PageKey(k.0))
    }

    fn rcs_key(&self, k: &mut Key) {
        let Some(c) = &self.combat else {
            k.tag(0);
            return;
        };
        k.tag(1);
        let (cx, cy, radius) = (69i32, 55i32, 48.);
        let scale = c.rcs.scale_nmi.max(1e-6);
        let point = |bearing: f64, nmi: f64| {
            let ratio = (nmi / scale).min(1.) * radius;
            (
                cx + (bearing.sin() * ratio).round() as i32,
                cy - (bearing.cos() * ratio).round() as i32,
            )
        };
        k.u64(c.rcs.contour.len() as u64);
        let mut over_range = false;
        for &(bearing, nmi) in &c.rcs.contour {
            over_range |= nmi > scale;
            k.point(point(bearing, nmi));
        }
        k.u64(c.rcs.emitters.len() as u64);
        for e in &c.rcs.emitters {
            k.point(match e.distance_nmi {
                Some(nmi) => point(e.bearing_rad, nmi),
                None => point(e.bearing_rad, scale),
            });
            k.tag(match (e.symbol, e.distance_nmi) {
                (tore_sim::sensors::passive::Symbol::Ground, _) => 0,
                (_, Some(_)) => 1,
                _ => 2,
            });
        }
        k.text(&format!("{:.0}", c.rcs.scale_nmi))
            .flag(over_range)
            .text(&format!("SIG {:.0}", c.rcs.signature));
    }

    fn rwr_key(&self, k: &mut Key, s: &State) {
        let radius = 48.;
        let scale = self.rwr_scale_nmi();
        k.text(&format!("{scale:.0}"));
        if let Some(combat) = &self.combat {
            k.tag(1);
            let rwr = &combat.rwr;
            let blink_on = rwr_blink_on(rwr.tick);
            for emitter in &rwr.emitters {
                if emitter.state == scope::EmitterState::Tracking && !blink_on {
                    continue;
                }
                k.tag(2)
                    .flag(emitter.state == scope::EmitterState::Detected)
                    .tag(emitter.kind as u8);
                plot(
                    k,
                    rwr_plot(emitter.bearing_rad, emitter.distance_nmi, scale, radius),
                );
            }
            for missile in &rwr.missiles {
                if missile.known_targeting_receiver && !missile.stale && !blink_on {
                    continue;
                }
                k.tag(3).flag(missile.stale);
                plot(
                    k,
                    rwr_plot(missile.bearing_rad, missile.distance_nmi, scale, radius),
                );
            }
            for indicator in [rwr.radar_indicator, rwr.infrared_indicator] {
                k.tag(match indicator {
                    scope::Indicator::Off => 4,
                    scope::Indicator::Detected => 5,
                    scope::Indicator::Tracking => 6,
                    scope::Indicator::Incoming if blink_on => 7,
                    scope::Indicator::Incoming => 4,
                });
            }
        } else {
            k.tag(0);
        }
        k.flag(s.jammer && s.engine);
    }

    fn radar_key(&self, k: &mut Key) {
        match &self.combat {
            None => {
                k.tag(0);
            }
            Some(c) if !c.scope.operating => {
                let reason = match (c.scope.infrared, c.scope.unavailable) {
                    (_, Some(text)) => text,
                    (true, _) => "IR UNAVAILABLE",
                    _ => "RADAR OFF",
                };
                k.tag(1).text(reason);
            }
            Some(c) => {
                k.tag(2);
                let scope = &c.scope;
                let range = scope.range_nmi;
                k.u64(scope.strobes.len() as u64);
                for strobe in &scope.strobes {
                    // The band as the noise loop takes it.
                    k.f64(((strobe.bearing_rad.to_degrees() / 2.).round() * 2.).to_radians())
                        .f64(strobe.half_width_rad.max(2f64.to_radians()))
                        .f64(strobe.density)
                        .f64(strobe.density * strobe.sidelobe);
                }
                if !scope.strobes.is_empty() {
                    k.u64(scope.tick / 12);
                }
                for contact in &scope.contacts {
                    if self.history && !contact.stale {
                        for (age, (bearing, distance)) in contact.trail.iter().rev().enumerate() {
                            if let Some((x, y)) = project(*bearing, *distance, range) {
                                k.tag(3).point((x as i32, y as i32)).u64(age as u64);
                            }
                        }
                    }
                    let Some((x, y)) = project(contact.bearing_rad, contact.distance_ft, range)
                    else {
                        continue;
                    };
                    let (x, y) = (x as i32, y as i32);
                    if contact.stale {
                        k.tag(4).point((x, y));
                        continue;
                    }
                    k.tag(5)
                        .point((x, y))
                        .flag(contact.selected)
                        .tag(match contact.symbol {
                            tore_sim::sensors::GroundSymbol::Square => 0,
                            tore_sim::sensors::GroundSymbol::Dot => 1,
                            tore_sim::sensors::GroundSymbol::Cross => 2,
                        });
                    link_marks(k, scope.marks_on(contact.id), scope.tick);
                    match contact.heading_rad {
                        Some(heading) if scope.mode == Some("TWS") => {
                            k.tag(1).point((
                                x + (heading.sin() * 9.).round() as i32,
                                y - (heading.cos() * 9.).round() as i32,
                            ));
                        }
                        _ => {
                            k.tag(0);
                        }
                    }
                    k.tag(if contact.selected && contact.wide_bars {
                        1
                    } else if contact.selected {
                        2
                    } else if self.hovered == Some(contact.id) {
                        3
                    } else {
                        0
                    });
                }
                for track in &scope.remote {
                    let Some((x, y)) = project(track.bearing_rad, track.distance_ft, range) else {
                        continue;
                    };
                    k.tag(6).point((x as i32, y as i32));
                    link_marks(k, scope.marks_on(track.id), scope.tick);
                }
                k.tag(7)
                    .text(scope.mode.unwrap_or(scope.channel))
                    .text(&format!("{range:.0}"))
                    .flag(scope.history);
                match scope.status {
                    Some(status) => k.tag(1).text(status),
                    None => k.tag(0),
                };
            }
        }
        match self.crosshair {
            Some(at) => k.tag(1).point(at),
            None => k.tag(0),
        };
    }

    fn weapons_key(&self, k: &mut Key) {
        let Some(c) = &self.combat else {
            k.tag(0);
            return;
        };
        k.tag(1);
        let page = self.weapon_page % c.weapons.len().div_ceil(6).max(1);
        for (name, count, selected) in c.weapons.iter().skip(page * 6).take(6) {
            k.tag(2).text(name).u64(u64::from(*count)).flag(*selected);
        }
        match &c.gun_group {
            Some(group) => k.tag(3).text(group),
            None => k.tag(4),
        };
        k.u64(u64::from(c.chaff)).u64(u64::from(c.flares));
    }

    fn nav_key(&self, k: &mut Key, s: &State) {
        let nav = &self.navigation;
        let Some(selected) = nav.index() else {
            k.tag(0);
            return;
        };
        k.tag(1);
        let page = selected / 3 * 3;
        for (index, entry) in nav.entries().iter().enumerate().skip(page).take(3) {
            k.tag(2)
                .u64(index as u64)
                .flag(index == selected)
                .text(&entry.name)
                .text(&format!("{:03}", entry.bearing(s.position)))
                .text(&format!(", {:.1} NM", entry.distance(s.position) / 6076.12));
        }
        let speed = s.velocity[0].hypot(s.velocity[2]);
        let eta = if speed >= 1. {
            let seconds = (nav.entries()[selected].distance(s.position) / speed).round() as u64;
            format!("ETA {}:{:02}", seconds / 60, seconds % 60)
        } else {
            "ETA --:--".into()
        };
        k.tag(3).text(&eta);
    }

    fn target_key(&self, k: &mut Key) {
        let gunsight = self.combat.as_ref().and_then(|c| c.gunsight.as_ref());
        let targeting = self.combat.as_ref().and_then(|c| c.targeting.as_ref());
        let shown = self
            .combat
            .as_ref()
            .and_then(|c| c.target.as_ref().map(|target| (target, &c.target_link)));
        let any = shown.is_some() || gunsight.is_some() || targeting.is_some();
        k.flag(any);
        if any {
            match self.cameras.get(&4) {
                Some(pixels) if self.camera_target == shown.map(|(target, _)| target.id) => {
                    k.tag(1).bytes(pixels).f64(self.sight_bloom);
                }
                _ => {
                    k.tag(0);
                }
            }
        }
        match (gunsight, targeting) {
            (Some(page), _) => k.tag(1).debug(page),
            (None, Some(page)) => k.tag(2).debug(page),
            (None, None) => k.tag(3),
        };
        match shown {
            Some((target, link)) => {
                k.tag(1)
                    .u64(u64::from(target.id))
                    .text(&target.name)
                    .f64(target.damage)
                    .text(&target.bearing)
                    .text(&target.metric)
                    .debug(&target.objective)
                    .text(&target.activity)
                    .text(target.goal)
                    .flag(target.player_goal)
                    .debug(&target.skill)
                    .debug(link);
            }
            None => {
                k.tag(0);
            }
        }
    }

    fn camera_key(&self, k: &mut Key, id: u8, s: &State) {
        let Some(pixels) = self.cameras.get(&id) else {
            k.tag(0);
            return;
        };
        k.tag(1).bytes(pixels).colour([
            self.hud_color[0],
            self.hud_color[1],
            self.hud_color[2],
            255,
        ]);
        match &self.front_shown {
            Some(symbology) if id == 2 && !s.systems.has(31) => k.tag(1).debug(symbology),
            _ => k.tag(0),
        };
        match self.aim_shown.get(&id) {
            Some(layout) => k.tag(1).debug(layout),
            None => k.tag(0),
        };
    }
}

fn systems_key(k: &mut Key, s: &State) {
    let green = [72, 172, 55, 255];
    let amber = [235, 187, 60, 255];
    let red = [235, 70, 50, 255];
    for (value, colour) in [
        (s.throttle * 100., green),
        (
            s.systems.engine.temperature,
            if s.systems.engine.temperature >= 75. {
                red
            } else if s.systems.engine.temperature > 25. {
                amber
            } else {
                green
            },
        ),
        (
            s.systems.oil_pressure() * 100.,
            if s.systems.oil_pressure() <= 0.25 {
                red
            } else if s.systems.oil_pressure() < 0.75 {
                amber
            } else {
                green
            },
        ),
        (
            s.systems.fluids.hydraulic * 100.,
            if s.systems.fluids.hydraulic <= 0.25 {
                red
            } else if s.systems.fluids.hydraulic < 0.75 {
                amber
            } else {
                green
            },
        ),
    ] {
        k.text(&format!("{value:.0}%")).colour(colour);
    }
    k.text(&format!("{:.0} LBS", s.fuel))
        .text(&format!("{:.0} LBS)", s.systems.external_lbs()));
}
