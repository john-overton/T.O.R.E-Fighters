//! Opinionated map presentation. Knowledge comes from simulation observations.
use crate::{
    flight,
    hud::Paint,
    menu::{Canvas, Sprite},
    scenery::Scenery,
    terrain::Terrain,
};
use tore_formats::surface_unit::class;
use tore_formats::text::GlyphCodes;
use tore_formats::{font::Font, theater::CELL_FEET};
use tore_sim::combat::live::{NO_SIDE, Side};
use tore_sim::sensors::FEET_PER_NAUTICAL_MILE as NMI;
use tore_world::readout::CockpitReadout;
use tore_world::surface::{Unit, UnitId};

const AREA: (i32, i32, i32, i32) = (12, 30, 476, 408);
const INK: [u8; 4] = [232, 232, 218, 255];
const UNKNOWN: [u8; 4] = [255, 223, 92, 255];

#[derive(Clone, Copy)]
enum Category {
    Aircraft,
    Airfields,
    Buildings,
    Surface,
    Emitters,
}
impl Category {
    const ALL: [Self; 5] = [
        Self::Aircraft,
        Self::Airfields,
        Self::Buildings,
        Self::Surface,
        Self::Emitters,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Aircraft => "AIRCRAFT",
            Self::Airfields => "AIRFIELDS",
            Self::Buildings => "BUILDINGS",
            Self::Surface => "SURFACE",
            Self::Emitters => "EMITTERS",
        }
    }
    fn rect(self) -> (i32, i32, i32, i32) {
        (504, 76 + self as i32 * 44, 120, 30)
    }
}
struct Filters {
    visible: [bool; 5],
    pressed: Option<usize>,
}
impl Default for Filters {
    fn default() -> Self {
        Self {
            visible: [true, true, false, true, true],
            pressed: None,
        }
    }
}
impl Filters {
    fn shows(&self, category: Category) -> bool {
        self.visible[category as usize]
    }
    fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) {
        let hit = point.and_then(|(x, y)| {
            Category::ALL.iter().position(|category| {
                let (a, b, w, h) = category.rect();
                x >= a as f64 && x < (a + w) as f64 && y >= b as f64 && y < (b + h) as f64
            })
        });
        if down {
            self.pressed = hit;
        } else if let Some(index) = self.pressed.take().filter(|index| Some(*index) == hit) {
            self.visible[index] = !self.visible[index];
        }
    }
}

// Authored structural grouping. Do not use damage classes: they mix buildings,
// defenses and scenery. Unlisted resources remain in the Surface category.
fn is_building(resource: &str) -> bool {
    matches!(
        resource,
        "APTM.OT"
            | "APTLA.OT"
            | "APTLB.OT"
            | "APTLC.OT"
            | "APTB1.OT"
            | "APTOLD.OT"
            | "BARKSA.OT"
            | "BARKSB.OT"
            | "BLDG1.OT"
            | "BLDG2.OT"
            | "BLDG3.OT"
            | "BNK1.OT"
            | "BNK2.OT"
            | "BNK3.OT"
            | "BNK4.OT"
            | "BNK5.OT"
            | "BNK6.OT"
            | "BNK7.OT"
            | "BNK8.OT"
            | "BNK9.OT"
            | "~BNK5.OT"
            | "~BNK6.OT"
            | "~BNK8.OT"
            | "BUNKER.OT"
            | "SHELT.OT"
            | "BR1END.OT"
            | "BR1MID.OT"
            | "BR2END.OT"
            | "BR2MID.OT"
            | "BR3END.OT"
            | "BR3MID.OT"
            | "BRD1.OT"
            | "BRD2.OT"
            | "BRD3.OT"
            | "BRD4.OT"
            | "BRDEND.OT"
            | "BRDMID.OT"
            | "CASTLE.OT"
            | "CGRP1.OT"
            | "CGRP2.OT"
            | "CGRP3.OT"
            | "CMHQ1.OT"
            | "CMHQ2.OT"
            | "COLTWR.OT"
            | "~COLTWR.OT"
            | "COMM.OT"
            | "COMM1.OT"
            | "COMM2.OT"
            | "CTOWR.OT"
            | "CTWR1.OT"
            | "CTWR2.OT"
            | "TOWER.OT"
            | "DKHOS1.OT"
            | "DKHOS2.OT"
            | "DOCK1.OT"
            | "FACT1.OT"
            | "FACTD.OT"
            | "FCTYA.OT"
            | "FCTYB.OT"
            | "FUEL.OT"
            | "HANGR.OT"
            | "HANGRB.OT"
            | "AV8TNT.OT"
            | "HILT.OT"
            | "IND1.OT"
            | "IND2.OT"
            | "NUCCT.OT"
            | "NUCOB.OT"
            | "NUCRD.OT"
            | "OILW.OT"
            | "REACTB.OT"
            | "REACTR.OT"
            | "REDOM.OT"
            | "RELAY.OT"
            | "SLUM01.OT"
            | "SLUM02.OT"
            | "STORE.OT"
            | "HGRP1.OT"
            | "HGRP2.OT"
            | "HGRP3.OT"
            | "HOOCH.OT"
            | "HOUS.OT"
            | "HOUSB.OT"
            | "CITY1.OT"
            | "CITY2.OT"
            | "CITY3.OT"
            | "RES1.OT"
            | "RES2.OT"
            | "SGRP1.OT"
            | "SGRP2.OT"
            | "CTYBKA.OT"
            | "CTYBKB.OT"
            | "CTYBKC.OT"
            | "CTYBKD.OT"
            | "CTYBKE.OT"
            | "CTYBKF.OT"
            | "CTYBKG.OT"
            | "TWNBKA.OT"
            | "TWNBKB.OT"
            | "TWNBKC.OT"
            | "TWNBKD.OT"
            | "TWNBKE.OT"
            | "TWNBKF.OT"
    )
}

/// MCICONS.PIC's ground symbols, by column: the AAA gun, the missile
/// launcher, the tank, the truck, the radar building, the ship and the
/// generic cube. Which symbol means which is read from the sheet (agent
/// decision, docs/spec/flight-map.md).
mod icon {
    pub const AAA: usize = 8;
    pub const SAM: usize = 9;
    pub const TANK: usize = 10;
    pub const TRUCK: usize = 11;
    pub const RADAR: usize = 12;
    pub const SHIP: usize = 13;
    pub const OTHER: usize = 14;
}

/// The side a symbol's tile shows: the sheet's blue row is Bluefor, its red
/// row Redfor, and a grey tile asserts neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tile {
    Neutral,
    Friendly,
    Hostile,
}

/// What a visually identified surface unit is drawn as: its symbol by its
/// class, and the tile of its side against the viewer's `own` side.
fn unit_look(unit: &Unit, own: Side) -> (usize, Tile) {
    let symbol = match unit.class {
        class::AAA => icon::AAA,
        class::SAM => icon::SAM,
        class::TANK => icon::TANK,
        class::VEHICLE if unit.supply_truck => icon::TRUCK,
        class::VEHICLE | class::STRUCTURE => icon::RADAR,
        class::SHIP => icon::SHIP,
        _ => icon::OTHER,
    };
    let tile = if unit.side == NO_SIDE {
        Tile::Neutral
    } else if unit.side == own {
        Tile::Friendly
    } else {
        Tile::Hostile
    };
    (symbol, tile)
}

#[derive(Default)]
pub struct Map {
    pub open: bool,
    filters: Filters,
    zoom: i32,
    offset: [f64; 2],
    anchor: Option<[f64; 2]>,
    last_player: [f64; 2],
}
impl Map {
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) {
        self.filters.pointer(point, down);
    }
    pub fn cancel_press(&mut self) {
        self.filters.pressed = None;
    }

    fn width_nmi(&self) -> f64 {
        100. * 2f64.powi(self.zoom)
    }
    pub fn key(&mut self, key: &str) {
        let step = self.width_nmi() * NMI * 0.25;
        if key.starts_with("Arrow") && self.anchor.is_none() {
            self.anchor = Some(self.last_player);
        }
        match key {
            "+" | "=" => self.zoom = (self.zoom - 1).max(-2),
            "-" | "_" => self.zoom = (self.zoom + 1).min(3),
            "ArrowLeft" => self.offset[0] -= step,
            "ArrowRight" => self.offset[0] += step,
            "ArrowUp" => self.offset[1] += step,
            "ArrowDown" => self.offset[1] -= step,
            "Home" => {
                self.offset = [0.; 2];
                self.anchor = None;
            }
            _ => {}
        }
    }
    fn projection(&self, state: &flight::State) -> Projection {
        let anchor = self
            .anchor
            .unwrap_or([state.position[0], state.position[2]]);
        Projection {
            center: [anchor[0] + self.offset[0], anchor[1] + self.offset[1]],
            feet_per_pixel: self.width_nmi() * NMI / f64::from(AREA.2),
        }
    }
    fn draw_filters(
        &self,
        pixels: &mut [u8],
        font: &Font,
        sprites: &std::collections::BTreeMap<String, Sprite>,
    ) {
        Canvas(pixels).rect((500, 30, 128, 408), [42, 45, 47, 255]);
        Paint {
            pixels,
            clip: (500, 30, 128, 408),
            color: INK,
        }
        .text(font, "SHOW OBJECTS", 507, 45);
        for category in Category::ALL {
            let (x, y, w, h) = category.rect();
            let enabled = self.filters.shows(category);
            let pressed = self.filters.pressed == Some(category as usize);
            if [
                "ACTION0L.PIC",
                "ACTION0M.PIC",
                "ACTION0R.PIC",
                "FONTACT.PIC",
            ]
            .iter()
            .all(|key| sprites.contains_key(*key))
            {
                Canvas(pixels).button_style(
                    sprites,
                    "",
                    (x, y, w),
                    if pressed { 0.7 } else { 1. },
                    "ACTION0",
                );
            } else {
                Canvas(pixels).rect((x, y, w, h), [66, 70, 73, 255]);
            }
            Canvas(pixels).rect((x + 5, y + 6, 12, 12), [20, 25, 27, 255]);
            let mut p = Paint {
                pixels,
                clip: (x, y, w, h),
                color: if enabled { UNKNOWN } else { INK },
            };
            if enabled {
                p.line(
                    ((x + 7) as f64, (y + 11) as f64),
                    ((x + 10) as f64, (y + 15) as f64),
                );
                p.line(
                    ((x + 10) as f64, (y + 15) as f64),
                    ((x + 15) as f64, (y + 8) as f64),
                );
            }
            p.text(font, category.label(), x + 22, y + 7);
        }
        Paint {
            pixels,
            clip: (500, 30, 128, 408),
            color: INK,
        }
        .text(font, "CLICK TO TOGGLE", 504, 310);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        pixels: &mut [u8],
        world: &Terrain,
        scenery: &Scenery,
        state: &flight::State,
        readout: &CockpitReadout,
        own: Side,
        font: &Font,
        sprites: &std::collections::BTreeMap<String, Sprite>,
    ) {
        Canvas(pixels).rect((0, 0, 640, 480), [72, 72, 72, 255]);
        self.last_player = [state.position[0], state.position[2]];
        let projection = self.projection(state);
        let icons = sprites.get("MCICONS.PIC");
        draw_ground(
            pixels,
            world,
            &projection,
            sprites.get(&world.theater.map),
            |color| scenery.palette[usize::from(color)],
        );
        let mut labels = Vec::new();
        let mut markers = Vec::new();
        if let Some((x, y)) = projection.point(state.position) {
            markers.push((x, y));
            let w = "PLAYER"
                .glyph_codes()
                .map(|c| font.glyphs[c as usize].advance as i32)
                .sum::<i32>();
            labels.push((
                x - w / 2 - 2,
                y + 11,
                x + w / 2 + 2,
                y + 13 + font.height as i32,
            ));
        }
        // Navigation landmarks are known independently of aircraft sensors.
        for runway in world
            .airport_scene
            .runways
            .iter()
            .filter(|_| self.filters.shows(Category::Airfields))
        {
            if let Some(point) = projection.point(runway.surface.center) {
                let (x, y) = place_marker(pixels, point, &mut markers, &mut labels);
                symbol(pixels, icons, x, y, Some(15), Tile::Neutral);
                let name = world
                    .airport_scene
                    .airports
                    .iter()
                    .find(|a| a.id == runway.airport)
                    .map_or(runway.name.as_str(), |a| a.name.as_str());
                label(pixels, font, name, (x, y + 12), INK, &mut labels);
            }
        }
        let contacts = &readout.map;
        for observed in contacts
            .iter()
            .filter(|c| c.airborne)
            .chain(contacts.iter().filter(|c| !c.airborne))
        {
            let contact = &observed.contact;
            if world
                .airport_scene
                .runways
                .iter()
                .any(|r| r.object == contact.id)
            {
                continue;
            }
            let category = if observed.airborne {
                Category::Aircraft
            } else if world
                .airport_scene
                .objects
                .iter()
                .any(|o| o.id == contact.id && is_building(&o.object_type))
            {
                Category::Buildings
            } else {
                Category::Surface
            };
            if !self.filters.shows(category) {
                continue;
            }
            let Some(point) = projection.point(contact.position) else {
                continue;
            };
            let (x, y) = place_marker(pixels, point, &mut markers, &mut labels);
            let identified = observed.identified;
            let name = if identified {
                observed
                    .aircraft
                    .map(|a| a.label())
                    .or_else(|| {
                        world
                            .airport_scene
                            .objects
                            .iter()
                            .find(|o| o.id == contact.id)
                            .map(|o| o.name.as_str())
                    })
                    .unwrap_or(if observed.airborne {
                        "AIRCRAFT"
                    } else {
                        "SURFACE OBJECT"
                    })
            } else if observed.airborne {
                "UNKNOWN AIRCRAFT"
            } else {
                "UNKNOWN SURFACE"
            };
            // A visually identified surface unit shows what it is, on the
            // tile of its side; anything else keeps the generic symbol.
            let unit = (!observed.airborne && identified)
                .then(|| world.surface.unit(UnitId(contact.id)))
                .flatten();
            let (icon, tile) = match unit {
                Some(unit) => {
                    let (symbol, tile) = unit_look(unit, own);
                    (Some(symbol), tile)
                }
                None => (
                    identified.then_some(if observed.airborne { 0 } else { icon::OTHER }),
                    Tile::Neutral,
                ),
            };
            symbol(pixels, icons, x, y, icon, tile);
            label(
                pixels,
                font,
                if contact.destroyed { "DESTROYED" } else { name },
                (x, y + 12),
                if identified { INK } else { UNKNOWN },
                &mut labels,
            );
        }
        // Passive noise has no measured location. Show its direction at ownship.
        if let Some((x, y)) = projection.point(state.position) {
            for emitter in readout
                .rwr
                .emitters
                .iter()
                .filter(|e| e.distance_nmi.is_none() && self.filters.shows(Category::Emitters))
            {
                let angle = state.yaw + emitter.bearing_rad;
                let end = (
                    f64::from(x) + angle.sin() * 45.,
                    f64::from(y) - angle.cos() * 45.,
                );
                if projection.inside(end.0 as i32, end.1 as i32) {
                    Paint {
                        clip: AREA,
                        pixels,
                        color: UNKNOWN,
                    }
                    .line((x as f64, y as f64), end);
                    label(
                        pixels,
                        font,
                        "UNKNOWN EMITTER",
                        (end.0 as i32, end.1 as i32),
                        UNKNOWN,
                        &mut labels,
                    );
                }
            }
            // The eight original aircraft headings are clockwise from north.
            let heading = (state.yaw.rem_euclid(std::f64::consts::TAU)
                / std::f64::consts::FRAC_PI_4)
                .round() as usize
                % 8;
            symbol(pixels, icons, x, y, Some(heading), Tile::Friendly);
            label(
                pixels,
                font,
                "PLAYER",
                (x, y + 12),
                UNKNOWN,
                &mut Vec::new(),
            );
        }
        self.draw_filters(pixels, font, sprites);
        // Redraw the frame so text and ray endpoints never escape the plot.
        let mut c = Canvas(pixels);
        c.rect((0, 0, 640, 30), [72, 72, 72, 255]);
        c.rect((0, 438, 640, 42), [72, 72, 72, 255]);
        let mut p = Paint {
            pixels,
            clip: (0, 0, 640, 480),
            color: INK,
        };
        p.line((12., 15.), (12. + AREA.2 as f64 / 4., 15.));
        for i in 0..=4 {
            let x = 12. + i as f64 * AREA.2 as f64 / 16.;
            p.line((x, 12.), (x, 18.));
        }
        p.text(
            font,
            &format!("{} NAUTICAL MILES", self.width_nmi() / 4.),
            174,
            9,
        );
        p.text(
            font,
            &format!(
                "HEADING: {:03.0} DEG",
                state.yaw.to_degrees().rem_euclid(360.)
            ),
            449,
            9,
        );
        p.text(
            font,
            &format!("SPEED: {:.0} KTS", state.speed * 3600. / NMI),
            12,
            443,
        );
        p.text(
            font,
            &format!("ALTITUDE: {:.0} FT", state.position[1]),
            436,
            443,
        );
        p.text(
            font,
            "SHIFT-M / ESC: CLOSE   +/-: ZOOM   ARROWS: PAN   HOME: FOLLOW",
            12,
            463,
        );
    }
}
/// The theater under the plot: the imported map picture when there is one,
/// else each terrain cell's colour (`palette` for the land, the sea blue).
fn draw_ground(
    pixels: &mut [u8],
    world: &Terrain,
    projection: &Projection,
    background: Option<&Sprite>,
    palette: impl Fn(u8) -> [u8; 3],
) {
    for y in AREA.1..AREA.1 + AREA.3 {
        for x in AREA.0..AREA.0 + AREA.2 {
            let [east, north] = projection.world(x, y);
            let col = (east / f64::from(CELL_FEET)).floor() as i32;
            let row = (north / f64::from(CELL_FEET)).floor() as i32;
            let rgb = match world.theater.lookup(col, row, 1) {
                Some(cell) => background.map_or_else(
                    || {
                        if cell.color == 255 {
                            [65, 119, 137]
                        } else {
                            palette(cell.color)
                        }
                    },
                    |map| {
                        let u = (east / (world.theater.cols as f64 * f64::from(CELL_FEET))
                            * map.width as f64)
                            .clamp(0., (map.width - 1) as f64)
                            as usize;
                        let v = ((1. - north / (world.theater.rows as f64 * f64::from(CELL_FEET)))
                            * map.height as f64)
                            .clamp(0., (map.height - 1) as f64)
                            as usize;
                        let at = (v * map.width + u) * 4;
                        map.rgba[at..at + 3].try_into().unwrap()
                    },
                ),
                None => [18; 3],
            };
            let at = (y as usize * 640 + x as usize) * 4;
            pixels[at..at + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
}

/// Which side a planning map marker belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanSide {
    Blue,
    Red,
}

/// One start on the planning map: where a wing begins and which way it
/// points, with the words beside it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanMarker {
    pub position: [f64; 3],
    /// Radians clockwise from north.
    pub heading: f64,
    pub side: PlanSide,
    pub label: String,
}

/// What the Quick Mission creator's map shows before a flight (slice CL-R,
/// John 2026-10-11: "a map button that shows a screen like the in-game map
/// and shows the bluefor and redfor starting locations"): the starts, the
/// approximate objective and a note.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub markers: Vec<PlanMarker>,
    /// The objective area: its centre (east, north feet) and radius in feet.
    pub objective: Option<([f64; 2], f64)>,
    /// Runway objects whose airfield a start's label names already, so the
    /// field is not drawn again beneath it.
    pub start_fields: Vec<u32>,
    /// The line below the plot.
    pub summary: String,
    /// Words in the side panel under the legend, when the map has to say
    /// something about what it shows.
    pub note: Option<String>,
}

/// The planning map's Close button.
pub const PLAN_CLOSE: (i32, i32, i32) = (518, 404, 95);
/// Colour of the objective area's ring and its words.
const OBJECTIVE: [u8; 4] = [255, 223, 92, 255];
/// The dark edge that keeps the objective area visible on light ground.
const OBJECTIVE_EDGE: [u8; 4] = [24, 26, 28, 255];

/// The creator's map view: the plan's own zoom and centre, then the
/// player's zoom and pan from there, as the in-game map's keys do.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanView {
    center: [f64; 2],
    fit: i32,
    zoom: i32,
    offset: [f64; 2],
}

impl PlanView {
    /// A view that holds every start and the objective area, at the
    /// narrowest of the in-game map's widths (25 to 800 nm) that does.
    pub fn new(plan: &Plan, world: &Terrain) -> Self {
        let mut points: Vec<[f64; 2]> = plan
            .markers
            .iter()
            .map(|m| [m.position[0], m.position[2]])
            .collect();
        if let Some((center, radius)) = plan.objective {
            points.push([center[0] - radius, center[1] - radius]);
            points.push([center[0] + radius, center[1] + radius]);
        }
        let (center, fit) = if points.is_empty() {
            let theater = &world.theater;
            (
                [
                    theater.cols as f64 * f64::from(CELL_FEET) / 2.,
                    theater.rows as f64 * f64::from(CELL_FEET) / 2.,
                ],
                3,
            )
        } else {
            let min = |i: usize| points.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min);
            let max = |i: usize| {
                points
                    .iter()
                    .map(|p| p[i])
                    .fold(f64::NEG_INFINITY, f64::max)
            };
            let span =
                (max(0) - min(0)).max((max(1) - min(1)) * f64::from(AREA.2) / f64::from(AREA.3));
            // A margin of a fifth of the plot for the symbols and labels.
            let fit = (0..=3)
                .find(|zoom| 100. * 2f64.powi(*zoom) * NMI * 0.8 >= span)
                .unwrap_or(3);
            ([(min(0) + max(0)) / 2., (min(1) + max(1)) / 2.], fit)
        };
        Self {
            center,
            fit,
            zoom: fit,
            offset: [0.; 2],
        }
    }
    fn width_nmi(&self) -> f64 {
        100. * 2f64.powi(self.zoom)
    }
    /// The in-game map's keys: + and - zoom, the arrows pan, Home goes back
    /// to the plan's own view.
    pub fn key(&mut self, key: &str) {
        let step = self.width_nmi() * NMI * 0.25;
        match key {
            "+" | "=" => self.zoom = (self.zoom - 1).max(-2),
            "-" | "_" => self.zoom = (self.zoom + 1).min(3),
            "ArrowLeft" => self.offset[0] -= step,
            "ArrowRight" => self.offset[0] += step,
            "ArrowUp" => self.offset[1] += step,
            "ArrowDown" => self.offset[1] -= step,
            "Home" => {
                self.offset = [0.; 2];
                self.zoom = self.fit;
            }
            _ => {}
        }
    }
    fn projection(&self) -> Projection {
        Projection {
            center: [
                self.center[0] + self.offset[0],
                self.center[1] + self.offset[1],
            ],
            feet_per_pixel: self.width_nmi() * NMI / f64::from(AREA.2),
        }
    }
    /// Draws the plan over `world` in the in-game map's look, with
    /// `sprites` the creator's (the theater picture, MCICONS.PIC and the
    /// button pieces). Returns the Close button's rectangle.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        pixels: &mut [u8],
        world: &Terrain,
        font: &Font,
        sprites: &std::collections::BTreeMap<String, Sprite>,
        plan: &Plan,
        title: &str,
        close_pressed: bool,
    ) -> (i32, i32, i32, i32) {
        Canvas(pixels).rect((0, 0, 640, 480), [72, 72, 72, 255]);
        let projection = self.projection();
        let icons = sprites.get("MCICONS.PIC");
        // Without the theater picture the cells are drawn in a plain land
        // grey (the creator has no scenery palette).
        draw_ground(
            pixels,
            world,
            &projection,
            sprites.get(&world.theater.map),
            |_| [118, 114, 98],
        );
        let mut labels = Vec::new();
        let mut markers = Vec::new();
        // The objective area first, under everything else: yellow hatching
        // edged in dark and a yellow ring between two dark ones, so it shows
        // on sea, forest and desert alike, and never less than 7 pixels in
        // radius so it shows at the widest zoom.
        let mut objective_label = None;
        if let Some((center, radius)) = plan.objective
            && let Some(at) = projection.unclipped([center[0], 0., center[1]])
        {
            let r = (radius / projection.feet_per_pixel).max(7.);
            objective_mark(pixels, at, r, AREA);
            let point = (at.0 as i32, at.1 as i32);
            if projection.inside(point.0, point.1) {
                // Nothing is placed on top of a small area.
                if r < 20. {
                    markers.push(point);
                    labels.push((point.0 - 12, point.1 - 10, point.0 + 12, point.1 + 10));
                }
                objective_label = Some((point.0, point.1 + r.min(20.) as i32 + 3));
            }
        }
        if let Some(at) = objective_label {
            label(pixels, font, "OBJECTIVE AREA", at, OBJECTIVE, &mut labels);
        }
        // Where each symbol goes: the starts first, then the airfields
        // around them. The leaders are drawn as they are placed, so every
        // symbol and label is drawn over them.
        let mut placed: Vec<((i32, i32), usize, Tile, &str)> = Vec::new();
        for marker in &plan.markers {
            let Some(point) = projection.point(marker.position) else {
                continue;
            };
            let at = place_marker(pixels, point, &mut markers, &mut labels);
            let heading = (marker.heading.rem_euclid(std::f64::consts::TAU)
                / std::f64::consts::FRAC_PI_4)
                .round() as usize
                % 8;
            let tile = match marker.side {
                PlanSide::Blue => Tile::Friendly,
                PlanSide::Red => Tile::Hostile,
            };
            placed.push((at, heading, tile, &marker.label));
        }
        let scene = &world.airport_scene;
        for airport in &scene.airports {
            if airport
                .runway_objects
                .iter()
                .any(|object| plan.start_fields.contains(object))
            {
                continue;
            }
            let runways: Vec<[f64; 3]> = scene
                .runways
                .iter()
                .filter(|r| r.airport == airport.id)
                .map(|r| r.surface.center)
                .collect();
            if runways.is_empty() {
                continue;
            }
            let center: [f64; 3] = std::array::from_fn(|i| {
                runways.iter().map(|p| p[i]).sum::<f64>() / runways.len() as f64
            });
            let Some(point) = projection.point(center) else {
                continue;
            };
            let at = place_marker(pixels, point, &mut markers, &mut labels);
            let tile = match airport.allegiance {
                tore_sim::airport::Allegiance::Friendly => Tile::Friendly,
                tore_sim::airport::Allegiance::Hostile => Tile::Hostile,
                _ => Tile::Neutral,
            };
            placed.push((at, 15, tile, &airport.name));
        }
        for ((x, y), icon, tile, _) in &placed {
            symbol(pixels, icons, *x, *y, Some(*icon), *tile);
        }
        for ((x, y), _, _, words) in &placed {
            label(pixels, font, words, (*x, y + 12), INK, &mut labels);
        }
        self.draw_legend(pixels, font, icons, plan);
        // The frame, as the in-game map redraws it.
        let mut c = Canvas(pixels);
        c.rect((0, 0, 640, 30), [72, 72, 72, 255]);
        c.rect((0, 438, 640, 42), [72, 72, 72, 255]);
        let mut p = Paint {
            pixels,
            clip: (0, 0, 640, 480),
            color: INK,
        };
        p.line((12., 15.), (12. + AREA.2 as f64 / 4., 15.));
        for i in 0..=4 {
            let x = 12. + i as f64 * AREA.2 as f64 / 16.;
            p.line((x, 12.), (x, 18.));
        }
        p.text(
            font,
            &format!("{} NAUTICAL MILES", self.width_nmi() / 4.),
            174,
            9,
        );
        let title = title.to_ascii_uppercase();
        p.text(font, &title, 628 - advance(font, &title), 9);
        p.text(font, &plan.summary, 12, 443);
        p.text(
            font,
            "ESC / ENTER: CLOSE   +/-: ZOOM   ARROWS: PAN   HOME: RESET",
            12,
            463,
        );
        let (x, y, w) = PLAN_CLOSE;
        if ["ACTION0L.PIC", "ACTION0M.PIC", "ACTION0R.PIC", "QUICKFONT"]
            .iter()
            .all(|key| sprites.contains_key(*key))
        {
            Canvas(pixels).action_button(sprites, "Close", (x, y, w), false, close_pressed)
        } else {
            Canvas(pixels).rect((x, y, w, 24), [66, 70, 73, 255]);
            Paint {
                pixels,
                clip: (x, y, w, 24),
                color: INK,
            }
            .text(font, "CLOSE", x + (w - advance(font, "CLOSE")) / 2, y + 7);
            (x, y, w, 24)
        }
    }
    /// The side panel: what each mark means, and the plan's note.
    fn draw_legend(&self, pixels: &mut [u8], font: &Font, icons: Option<&Sprite>, plan: &Plan) {
        Canvas(pixels).rect((500, 30, 128, 408), [42, 45, 47, 255]);
        let mut p = Paint {
            pixels,
            clip: (500, 30, 128, 408),
            color: INK,
        };
        p.text(font, "MISSION MAP", 507, 45);
        let rows: [(&str, Option<usize>, Tile); 4] = [
            ("BLUE START", Some(2), Tile::Friendly),
            ("RED START", Some(6), Tile::Hostile),
            ("AIRFIELD", Some(15), Tile::Neutral),
            ("OBJECTIVE AREA", None, Tile::Neutral),
        ];
        for (i, (words, icon, tile)) in rows.into_iter().enumerate() {
            let y = 82 + i as i32 * 30;
            match icon {
                Some(icon) => symbol(pixels, icons, 517, y, Some(icon), tile),
                None => objective_mark(pixels, (517., f64::from(y)), 8., (500, 30, 128, 408)),
            }
            Paint {
                pixels,
                clip: (500, 30, 128, 408),
                color: if icon.is_none() { OBJECTIVE } else { INK },
            }
            .text(font, words, 533, y - 4);
        }
        let mut p = Paint {
            pixels,
            clip: (500, 30, 128, 408),
            color: INK,
        };
        p.text(font, "AIRFIELD TILES:", 507, 206);
        p.text(font, "BLUE, RED OR GREY", 507, 206 + font.height as i32 + 3);
        if let Some(note) = &plan.note {
            let mut y = 250;
            let mut line = String::new();
            for word in note.split_whitespace() {
                let next = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if advance(font, &next) > 114 && !line.is_empty() {
                    p.text(font, &line, 507, y);
                    y += font.height as i32 + 3;
                    line = word.to_owned();
                } else {
                    line = next;
                }
            }
            p.text(font, &line, 507, y);
        }
    }
}

/// The objective area's look: yellow hatching edged in dark inside a yellow
/// ring between two dark ones, so it shows on sea, forest and desert alike.
fn objective_mark(pixels: &mut [u8], at: (f64, f64), r: f64, clip: (i32, i32, i32, i32)) {
    let (x0, y0) = ((at.0 - r).floor() as i32, (at.1 - r).floor() as i32);
    for y in y0.max(clip.1)..((at.1 + r).ceil() as i32).min(clip.1 + clip.3) {
        for x in x0.max(clip.0)..((at.0 + r).ceil() as i32).min(clip.0 + clip.2) {
            if (f64::from(x) - at.0).hypot(f64::from(y) - at.1) > r {
                continue;
            }
            let color = match (x - y).rem_euclid(6) {
                0 => OBJECTIVE,
                1 => OBJECTIVE_EDGE,
                _ => continue,
            };
            let i = (y as usize * 640 + x as usize) * 4;
            pixels[i..i + 4].copy_from_slice(&color);
        }
    }
    let steps = (r * 8.).ceil().max(64.) as u32;
    for (ring, color) in [
        (r + 1., OBJECTIVE_EDGE),
        (r - 1., OBJECTIVE_EDGE),
        (r, OBJECTIVE),
    ] {
        let mut p = Paint {
            pixels: &mut *pixels,
            clip,
            color,
        };
        for i in 0..steps {
            let a = std::f64::consts::TAU * f64::from(i) / f64::from(steps);
            let b = std::f64::consts::TAU * f64::from(i + 1) / f64::from(steps);
            p.line(
                (at.0 + ring * a.sin(), at.1 - ring * a.cos()),
                (at.0 + ring * b.sin(), at.1 - ring * b.cos()),
            );
        }
    }
}

/// The width of `text` in `font`.
fn advance(font: &Font, text: &str) -> i32 {
    text.glyph_codes()
        .map(|c| font.glyphs[c as usize].advance as i32)
        .sum()
}

struct Projection {
    center: [f64; 2],
    feet_per_pixel: f64,
}
impl Projection {
    fn inside(&self, x: i32, y: i32) -> bool {
        (AREA.0 + 12..AREA.0 + AREA.2 - 12).contains(&x)
            && (AREA.1 + 12..AREA.1 + AREA.3 - 12).contains(&y)
    }
    fn point(&self, position: [f64; 3]) -> Option<(i32, i32)> {
        if !position.iter().all(|p| p.is_finite()) {
            return None;
        }
        let x =
            f64::from(AREA.0 + AREA.2 / 2) + (position[0] - self.center[0]) / self.feet_per_pixel;
        let y = 234. - (position[2] - self.center[1]) / self.feet_per_pixel;
        self.inside(x as i32, y as i32)
            .then_some((x as i32, y as i32))
    }
    /// Where a position falls on the screen, inside the plot or not.
    fn unclipped(&self, position: [f64; 3]) -> Option<(f64, f64)> {
        position.iter().all(|p| p.is_finite()).then(|| {
            (
                f64::from(AREA.0 + AREA.2 / 2)
                    + (position[0] - self.center[0]) / self.feet_per_pixel,
                234. - (position[2] - self.center[1]) / self.feet_per_pixel,
            )
        })
    }
    fn world(&self, x: i32, y: i32) -> [f64; 2] {
        [
            self.center[0] + (x as f64 - f64::from(AREA.0 + AREA.2 / 2)) * self.feet_per_pixel,
            self.center[1] - (y as f64 - 234.) * self.feet_per_pixel,
        ]
    }
}
// Keep neighboring symbols readable, with a leader to the actual observation.
fn place_marker(
    pixels: &mut [u8],
    point: (i32, i32),
    occupied: &mut Vec<(i32, i32)>,
    labels: &mut Vec<(i32, i32, i32, i32)>,
) -> (i32, i32) {
    let free = |(x, y): (i32, i32)| {
        x >= AREA.0 + 12
            && x < AREA.0 + AREA.2 - 12
            && y >= AREA.1 + 12
            && y < AREA.1 + AREA.3 - 24
            && !labels
                .iter()
                .any(|&(a, b, c, d)| x - 12 < c && x + 12 > a && y - 10 < d && y + 10 > b)
            && occupied
                .iter()
                .all(|&(a, b)| (x - a).abs() >= 26 || (y - b).abs() >= 34)
    };
    let mut placed = point;
    'search: for radius in [0, 34, 68, 102] {
        for (dx, dy) in [
            (1, 0),
            (0, -1),
            (-1, 0),
            (0, 1),
            (1, -1),
            (-1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let candidate = (point.0 + dx * radius, point.1 + dy * radius);
            if free(candidate) {
                placed = candidate;
                break 'search;
            }
        }
    }
    if placed != point {
        let mut p = Paint {
            pixels,
            clip: AREA,
            color: INK,
        };
        p.line(
            (point.0 as f64, point.1 as f64),
            (placed.0 as f64, placed.1 as f64),
        );
        p.rect(point.0 - 1, point.1 - 1, 3, 3);
    }
    occupied.push(placed);
    labels.push((placed.0 - 12, placed.1 - 10, placed.0 + 12, placed.1 + 10));
    placed
}
fn symbol(
    pixels: &mut [u8],
    icons: Option<&Sprite>,
    x: i32,
    y: i32,
    index: Option<usize>,
    tile: Tile,
) {
    if let (Some(sheet), Some(index)) = (icons, index)
        && sheet.width >= (index + 1) * 24
        && sheet.height >= 20
    {
        // The sheet's second row is the same symbols on the red tile; without
        // it a hostile symbol is drawn grey rather than in the wrong colour.
        let tile = if tile == Tile::Hostile && sheet.height < 40 {
            Tile::Neutral
        } else {
            tile
        };
        let row = if tile == Tile::Hostile { 20 } else { 0 };
        for dy in 0..20 {
            for dx in 0..23 {
                let src = ((dy + row) * sheet.width + index * 24 + dx) * 4;
                let mut color: [u8; 4] = sheet.rgba[src..src + 4].try_into().unwrap();
                // Neutral tiles do not assert friend/foe knowledge.
                if tile == Tile::Neutral && color[2] > color[0] {
                    color = [48, 53, 60, 255];
                }
                Canvas(pixels).rect((x - 11 + dx as i32, y - 10 + dy as i32, 1, 1), color);
            }
        }
    } else {
        Canvas(pixels).rect((x - 9, y - 9, 19, 19), [38, 42, 46, 255]);
        let mut p = Paint {
            clip: AREA,
            pixels,
            color: UNKNOWN,
        };
        for (a, b) in [
            ((x, y - 8), (x + 8, y)),
            ((x + 8, y), (x, y + 8)),
            ((x, y + 8), (x - 8, y)),
            ((x - 8, y), (x, y - 8)),
        ] {
            p.line((a.0 as f64, a.1 as f64), (b.0 as f64, b.1 as f64));
        }
        if index.is_none() {
            p.line(
                ((x - 2) as f64, (y - 3) as f64),
                ((x + 2) as f64, (y - 3) as f64),
            );
            p.line(((x + 2) as f64, (y - 3) as f64), (x as f64, y as f64));
            p.rect(x, y + 3, 1, 1);
        } else {
            p.line((x as f64, (y - 5) as f64), (x as f64, (y + 5) as f64));
        }
    }
}
fn label(
    pixels: &mut [u8],
    font: &Font,
    text: &str,
    (x, y): (i32, i32),
    color: [u8; 4],
    occupied: &mut Vec<(i32, i32, i32, i32)>,
) {
    let text: String = text.chars().take(28).collect();
    let w = text
        .glyph_codes()
        .map(|c| font.glyphs[c as usize].advance as i32)
        .sum::<i32>();
    let h = font.height as i32;
    let x = (x - w / 2).clamp(AREA.0 + 2, (AREA.0 + AREA.2 - w - 2).max(AREA.0 + 2));
    let fits = |y: i32| {
        y > AREA.1
            && y + h < AREA.1 + AREA.3
            && !occupied
                .iter()
                .any(|&(a, b, c, d)| x - 2 < c && x + w + 2 > a && y - 1 < d && y + h + 1 > b)
    };
    let Some(y) = [y, y - h - 24].into_iter().find(|y| fits(*y)) else {
        return;
    };
    occupied.push((x - 2, y - 1, x + w + 2, y + h + 1));
    Canvas(pixels).rect((x - 2, y - 1, w + 4, h + 2), [30, 36, 40, 255]);
    Paint {
        pixels,
        clip: AREA,
        color,
    }
    .text(font, &text, x, y);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buildings_default_off_and_defenses_are_not_buildings() {
        let filters = Filters::default();
        assert!(!filters.shows(Category::Buildings));
        for category in [
            Category::Aircraft,
            Category::Airfields,
            Category::Surface,
            Category::Emitters,
        ] {
            assert!(filters.shows(category));
        }
        for name in ["HANGR.OT", "CTYBKA.OT", "FUEL.OT", "TOWER.OT", "BUNKER.OT"] {
            assert!(is_building(name));
        }
        for name in [
            "SA3SITE.OT",
            "HAWKSITE.OT",
            "KING.OT",
            "PRDR1.OT",
            "STRIP3.OT",
            "UNKNOWN.OT",
        ] {
            assert!(!is_building(name));
        }
    }
    fn unit(class: u16, side: Side, truck: bool) -> Unit {
        use tore_world::surface::{DestroyedLook, Origin, UnitKind};
        Unit {
            id: UnitId(0x5000_0001),
            origin: Origin::Added,
            resource: "X.NT".into(),
            kind: UnitKind::Active,
            class,
            name: "X".into(),
            nationality: None,
            side,
            position: [0; 3],
            angles: [0; 3],
            flags: 0,
            skill: 1,
            react: None,
            search_dist: None,
            start_time: None,
            route: None,
            hit_points: 100,
            look: DestroyedLook::Vanish,
            explosion: None,
            crater: None,
            supply_truck: truck,
            in_scene: true,
        }
    }
    #[test]
    fn a_known_surface_unit_shows_its_kind_on_the_tile_of_its_side() {
        let (blue, red) = (Side(1), Side(2));
        for (class, truck, symbol) in [
            (class::AAA, false, icon::AAA),
            (class::SAM, false, icon::SAM),
            (class::TANK, false, icon::TANK),
            (class::VEHICLE, true, icon::TRUCK),
            (class::VEHICLE, false, icon::RADAR),
            (class::STRUCTURE, false, icon::RADAR),
            (class::SHIP, false, icon::SHIP),
            (0x4000, false, icon::OTHER),
        ] {
            assert_eq!(
                unit_look(&unit(class, red, truck), blue),
                (symbol, Tile::Hostile),
                "{class:#x}"
            );
        }
        // A friendly unit is on the blue tile, an ownerless one on a grey
        // tile that claims nothing, and the viewer's side decides which is
        // which: a Redfor pilot sees Redfor units as friends.
        assert_eq!(
            unit_look(&unit(class::SAM, blue, false), blue).1,
            Tile::Friendly
        );
        assert_eq!(
            unit_look(&unit(class::SAM, NO_SIDE, false), blue).1,
            Tile::Neutral
        );
        assert_eq!(
            unit_look(&unit(class::SAM, red, false), red).1,
            Tile::Friendly
        );
        assert_eq!(
            unit_look(&unit(class::SAM, blue, false), red).1,
            Tile::Hostile
        );
    }
    #[test]
    fn symbols_take_the_blue_row_for_friends_the_red_row_for_foes_and_grey_for_neither() {
        // A two-row sheet: blue tiles on top, red tiles below.
        let (width, height) = (24 * 17, 40);
        let mut rgba = vec![0_u8; width * height * 4];
        for (i, pixel) in rgba.chunks_exact_mut(4).enumerate() {
            let blue_row = i / width < 20;
            pixel.copy_from_slice(&if blue_row {
                [20, 40, 200, 255]
            } else {
                [200, 20, 20, 255]
            });
        }
        let sheet = Sprite {
            width,
            height,
            rgba,
            glyphs: vec![],
        };
        let at = |tile| {
            let mut pixels = vec![0_u8; 640 * 480 * 4];
            symbol(&mut pixels, Some(&sheet), 100, 100, Some(9), tile);
            let i = (100 * 640 + 100) * 4;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };
        assert_eq!(at(Tile::Friendly), [20, 40, 200]);
        assert_eq!(at(Tile::Hostile), [200, 20, 20]);
        assert_eq!(at(Tile::Neutral), [48, 53, 60]);
        // A sheet with no red row never shows a foe on a friendly tile.
        let single = Sprite {
            height: 20,
            rgba: sheet.rgba[..width * 20 * 4].to_vec(),
            ..sheet
        };
        let mut pixels = vec![0_u8; 640 * 480 * 4];
        symbol(&mut pixels, Some(&single), 100, 100, Some(9), Tile::Hostile);
        let i = (100 * 640 + 100) * 4;
        assert_eq!(pixels[i..i + 3], [48, 53, 60]);
    }
    #[test]
    fn category_click_requires_matching_press_release_and_cancel_discards_press() {
        let mut map = Map::default();
        let buildings = Some((510., 170.));
        map.pointer(buildings, false);
        assert!(!map.filters.shows(Category::Buildings));
        map.pointer(buildings, true);
        map.pointer(Some((510., 82.)), false);
        assert!(!map.filters.shows(Category::Buildings));
        assert!(map.filters.shows(Category::Aircraft));
        map.pointer(buildings, true);
        map.cancel_press();
        map.pointer(buildings, false);
        assert!(!map.filters.shows(Category::Buildings));
        map.pointer(buildings, true);
        map.pointer(buildings, false);
        assert!(map.filters.shows(Category::Buildings));
        map.open = false;
        map.open = true;
        assert!(map.filters.shows(Category::Buildings));
        map.pointer(Some((490., 170.)), true);
        map.pointer(buildings, false);
        assert!(map.filters.shows(Category::Buildings));
        map.pointer(buildings, true);
        map.pointer(buildings, false);
        assert!(!map.filters.shows(Category::Buildings));
    }
    #[test]
    fn north_up_projection_scale_and_edges() {
        let p = Projection {
            center: [1000., 2000.],
            feet_per_pixel: 100. * NMI / 476.,
        };
        assert_eq!(p.point([1000., 0., 2000.]), Some((250, 234)));
        assert_eq!(p.point([1000. + 25. * NMI, 0., 2000.]), Some((369, 234)));
        assert_eq!(p.point([1000., 0., 2000. + 25. * NMI]), Some((250, 115)));
        assert_eq!(p.point([1000. + 100. * NMI, 0., 2000.]), None);
        assert_eq!(p.point([f64::NAN, 0., 0.]), None);
        assert_eq!(p.world(250, 234), p.center);
    }
    #[test]
    fn zoom_is_bounded_and_home_resets_pan() {
        let mut m = Map::default();
        for _ in 0..20 {
            m.key("+");
        }
        assert_eq!(m.width_nmi(), 25.);
        m.key("ArrowUp");
        assert_eq!(m.offset, [0., 6.25 * NMI]);
        m.key("Home");
        assert_eq!(m.offset, [0.; 2]);
        for _ in 0..20 {
            m.key("-");
        }
        assert_eq!(m.width_nmi(), 800.);
    }
}
