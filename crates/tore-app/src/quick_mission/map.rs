//! The creator's Map (slice CL-R, John 2026-10-11): "a map button that
//! shows a screen like the in-game map and shows the bluefor and redfor
//! starting locations", with the objective as an approximate area, "so the
//! player knows roughly where to go without giving the targets away".
//!
//! The positions are the flight's own: the map builds the mission the
//! creator would fly ([`World::build`]) and reads where it put the player,
//! each AI wing and the ground target. It draws them with
//! [`crate::flight_map::PlanView`], the in-game map's look.
use super::QuickMission;
use crate::flight_map::{Plan, PlanMarker, PlanSide, PlanView};
use crate::menu::Sprite;
use std::collections::BTreeMap;
use tore_formats::font::Font;
use tore_sim::ai::launch::Side;
use tore_sim::sensors::FEET_PER_NAUTICAL_MILE as NMI;
use tore_world::ai_wings::AiFlightModel;
use tore_world::surface::layout::Variation;
use tore_world::terrain::Terrain;
use tore_world::world::{Hooks, Seating, World};

/// The objective area's radius, nm. `fitted` (agent decision, slice CL-R,
/// 2026-10-11), for John's opinionated rule of the same day: the objective
/// shows as an area, not the targets' positions.
pub const OBJECTIVE_RADIUS_NM: f64 = 5.;
/// The most the area's centre is moved off the targets' centroid, nm, so the
/// targets are inside the area but not at its centre. `fitted`, as above.
pub const OBJECTIVE_OFFSET_NM: f64 = 2.5;
/// A wing that starts within this many nm of the side's start is not drawn
/// again. `fitted` (agent decision, slice CL-R, 2026-10-11).
const SAME_PLACE_NM: f64 = 3.;
/// Said on the lobby's map, where the host draws the target's layout when the
/// flight starts.
pub const LOBBY_MAP_NOTE: &str = "The host places the target when the flight starts. The area is around the target's retail spot, and the starts may move with it.";

/// The open map: the mission's terrain, what it shows, its view and font.
pub struct Planning {
    terrain: Terrain,
    plan: Plan,
    view: PlanView,
    font: Font,
    title: String,
    close: (i32, i32, i32, i32),
    hover: bool,
    pressed: bool,
}

impl Planning {
    pub fn pointer(&mut self, point: Option<(f64, f64)>) {
        let (x, y, w, h) = self.close;
        self.hover = point.is_some_and(|(px, py)| {
            px >= f64::from(x)
                && py >= f64::from(y)
                && px < f64::from(x + w)
                && py < f64::from(y + h)
        });
    }
    pub fn down(&mut self) {
        self.pressed = self.hover;
    }
    /// The Close button was clicked: pressed and released on it.
    pub fn up(&mut self) -> bool {
        std::mem::take(&mut self.pressed) && self.hover
    }
    pub fn key(&mut self, key: &str) {
        self.view.key(key);
    }
    pub fn render(&mut self, pixels: &mut [u8], sprites: &BTreeMap<String, Sprite>) {
        self.close = self.view.draw(
            pixels,
            &self.terrain,
            &self.font,
            sprites,
            &self.plan,
            &self.title,
            self.pressed && self.hover,
        );
    }
    /// What the map shows, for tests and the snapshot's log.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
}

/// A deterministic 64-bit mix (SplitMix64's finaliser).
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The objective area for targets whose centroid is `centroid` (east and
/// north feet): a circle of [`OBJECTIVE_RADIUS_NM`] whose centre is moved up
/// to [`OBJECTIVE_OFFSET_NM`] in a direction and by a distance drawn from
/// the theater, the target's stem and the surface seed, so the same mission
/// always shows the same circle. Returns the centre and the radius in feet.
pub fn objective_area(centroid: [f64; 2], theater: &str, stem: &str, seed: u32) -> ([f64; 2], f64) {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in theater.bytes().chain([0]).chain(stem.bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    let draw = mix(hash ^ u64::from(seed));
    let angle = (draw & 0xffff_ffff) as f64 / 4_294_967_296. * std::f64::consts::TAU;
    // At least a fifth of the offset, so the targets are never at the centre.
    let distance = (0.2 + 0.8 * ((draw >> 32) as f64 / 4_294_967_296.)) * OBJECTIVE_OFFSET_NM * NMI;
    (
        [
            centroid[0] + angle.sin() * distance,
            centroid[1] + angle.cos() * distance,
        ],
        OBJECTIVE_RADIUS_NM * NMI,
    )
}

/// The airfield a ground start stands on: the nearest runway's, with its
/// runway objects.
fn airfield_at(terrain: &Terrain, position: [f64; 3]) -> Option<(String, Vec<u32>)> {
    let scene = &terrain.airport_scene;
    let runway = scene.runways.iter().min_by(|a, b| {
        let d = |r: &tore_sim::airport::Runway| {
            (r.surface.center[0] - position[0]).hypot(r.surface.center[2] - position[2])
        };
        d(a).total_cmp(&d(b))
    })?;
    let airport = scene.airports.iter().find(|a| a.id == runway.airport)?;
    Some((airport.name.clone(), airport.runway_objects.clone()))
}

fn distance_nm(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2]) / NMI
}

impl QuickMission {
    /// Opens the map on the mission the draft describes, or says in the
    /// notice why it cannot.
    pub(super) fn open_map(&mut self) {
        match self.build_map() {
            Ok(planning) => {
                self.cancel();
                self.map = Some(Box::new(planning));
            }
            Err(words) => self.notice = Some(words),
        }
    }

    /// Builds the mission the creator would fly and reads the starts and
    /// the objective from it.
    fn build_map(&mut self) -> Result<Planning, String> {
        let resources = self
            .resources
            .clone()
            .ok_or("The map needs the imported game data.")?;
        if let Some(problem) = self.unsupported() {
            return Err(problem);
        }
        let mut spec = self.mission_spec()?;
        spec.ai_flight_model = AiFlightModel::Standard;
        // The target's layout follows the seed. Single player draws the
        // flight's seed now, as OK would, and OK keeps it, so the flight
        // flies the layout the map shows. The lobby's host draws its own
        // when the flight starts, so the lobby shows the retail spot.
        let lobby = self.lobby || self.view.is_some();
        let mut variation = None;
        if spec.ground_target.is_some() {
            if lobby {
                spec.surface_seed = 0;
                variation = Some(Variation::OFF);
            } else {
                if !self.seed_pending {
                    self.surface_seed = super::fresh_seed();
                    self.seed_pending = true;
                }
                spec.surface_seed = self.surface_seed;
            }
        }
        let built = World::build(
            &spec,
            &*resources,
            Seating::SinglePlayer,
            &mut Hooks {
                ground_variation: variation,
                ..Hooks::default()
            },
        )
        .map_err(|error| format!("The map could not be built: {error}"))?;
        let world = built.world;
        let font = resources
            .get("WIN11.FNT")
            .and_then(|bytes| Font::parse(bytes).ok())
            .ok_or("The map needs WIN11.FNT; re-import the game.")?;
        let plan = plan_of(&world, &spec, lobby);
        let view = PlanView::new(&plan, &world.terrain);
        let title = world.terrain.theater.name.clone();
        Ok(Planning {
            terrain: world.terrain,
            plan,
            view,
            font,
            title,
            close: (0, 0, 0, 0),
            hover: false,
            pressed: false,
        })
    }
}

/// What the map shows of a built mission.
fn plan_of(world: &World, spec: &tore_world::mission::MissionSpec, lobby: bool) -> Plan {
    let mut plan = Plan::default();
    let terrain = &world.terrain;
    let ground_label = |side: &str, position: [f64; 3], plan: &mut Plan| -> String {
        match airfield_at(terrain, position) {
            Some((name, runways)) => {
                plan.start_fields.extend(runways);
                format!("{side} START: {}", name.to_ascii_uppercase())
            }
            None => format!("{side} START"),
        }
    };
    // Blue: the player's own start, which leads friendly wing 1.
    let mut blue_words = String::from("BLUE: AIRBORNE");
    if let Some(cockpit) = world.cockpits.first() {
        let flight = &cockpit.flight;
        let label = if spec.start.on_ground() {
            blue_words = "BLUE: ON THE GROUND".into();
            ground_label("BLUE", flight.position, &mut plan)
        } else {
            blue_words = format!("BLUE: AIRBORNE AT {} FT", spec.start.altitude_ft());
            "BLUE START".into()
        };
        plan.markers.push(PlanMarker {
            position: flight.position,
            heading: flight.yaw,
            side: PlanSide::Blue,
            label,
        });
    }
    // The AI wings' leads, by side and wing.
    let mut leads: Vec<(Side, u8, [f64; 3], f64)> = Vec::new();
    if let Some(ai) = &world.ai_wings {
        for slot in ai.slots() {
            if leads
                .iter()
                .any(|(side, wing, ..)| *side == slot.side && *wing == slot.wing_number)
            {
                continue;
            }
            if let Some(actor) = ai.mission().actor(slot.id) {
                let flight = actor.flight();
                leads.push((slot.side, slot.wing_number, flight.position, flight.yaw));
            }
        }
    }
    leads.sort_by_key(|(side, wing, ..)| (*side == Side::Enemy, *wing));
    let blue_start = plan.markers.first().map(|m| m.position);
    for (_, wing, position, heading) in leads.iter().filter(|(side, ..)| *side == Side::Friendly) {
        if *wing == 1 || blue_start.is_some_and(|at| distance_nm(at, *position) < SAME_PLACE_NM) {
            continue;
        }
        plan.markers.push(PlanMarker {
            position: *position,
            heading: *heading,
            side: PlanSide::Blue,
            label: format!("BLUE WING {wing}"),
        });
    }
    // Red: Enemy Wing 1's start (parked at its field for an enemy ground
    // start), else the first enemy wing's; the other wings where they differ.
    let mut red_words = String::from("RED: NONE");
    let mut red_start: Option<[f64; 3]> = None;
    for (_, wing, position, heading) in leads.iter().filter(|(side, ..)| *side == Side::Enemy) {
        let label = match red_start {
            None => {
                red_start = Some(*position);
                if *wing == 1 && spec.enemy_start.on_ground() {
                    red_words = "RED: ON THE GROUND".into();
                    ground_label("RED", *position, &mut plan)
                } else {
                    red_words = "RED: AIRBORNE".into();
                    "RED START".into()
                }
            }
            Some(at) if distance_nm(at, *position) < SAME_PLACE_NM => continue,
            Some(_) => format!("RED WING {wing}"),
        };
        plan.markers.push(PlanMarker {
            position: *position,
            heading: *heading,
            side: PlanSide::Red,
            label,
        });
    }
    // The objective as an area around the targets, never the targets.
    let surface = &terrain.surface;
    if let Some(stem) = &spec.ground_target {
        let centroid = surface
            .starts
            .as_ref()
            .map(|starts| [f64::from(starts.target[0]), f64::from(starts.target[1])])
            .or_else(|| {
                let points: Vec<[f64; 2]> = surface
                    .units
                    .iter()
                    .filter(|u| u.is_target())
                    .map(|u| u.position)
                    .chain(
                        surface
                            .parked
                            .iter()
                            .filter(|p| p.target)
                            .map(|p| p.position),
                    )
                    .map(|p| [f64::from(p[0]), f64::from(p[2])])
                    .collect();
                (!points.is_empty()).then(|| {
                    let n = points.len() as f64;
                    [
                        points.iter().map(|p| p[0]).sum::<f64>() / n,
                        points.iter().map(|p| p[1]).sum::<f64>() / n,
                    ]
                })
            });
        match (&surface.unresolved, centroid) {
            (None, Some(centroid)) => {
                plan.objective = Some(objective_area(
                    centroid,
                    &spec.theater,
                    stem,
                    spec.surface_seed,
                ));
            }
            _ => {
                plan.note = Some(
                    "The ground target is not in this import, so the map has no objective area."
                        .into(),
                );
            }
        }
    }
    if lobby && spec.ground_target.is_some() && plan.note.is_none() {
        plan.note = Some(LOBBY_MAP_NOTE.into());
    }
    plan.summary = format!("{blue_words}   {red_words}");
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_objective_area_holds_the_targets_off_its_centre_and_is_the_same_each_time() {
        for (theater, stem, seed) in [
            ("UKR", "QUSFLT", 0),
            ("EGY", "QEARMOR", 7),
            ("UKR", "QUSFLT", 99),
        ] {
            let centroid = [1_000_000., 2_000_000.];
            let (center, radius) = objective_area(centroid, theater, stem, seed);
            assert_eq!(
                objective_area(centroid, theater, stem, seed),
                (center, radius)
            );
            assert_eq!(radius, 5. * NMI);
            let off = (center[0] - centroid[0]).hypot(center[1] - centroid[1]) / NMI;
            assert!((0.5..=2.5).contains(&off), "{theater} {stem} {seed}: {off}");
        }
        // Another seed or target moves it.
        let a = objective_area([0.; 2], "UKR", "QUSFLT", 1).0;
        let b = objective_area([0.; 2], "UKR", "QUSFLT", 2).0;
        let c = objective_area([0.; 2], "UKR", "QULFLT", 1).0;
        assert!(a != b && a != c);
    }
}
