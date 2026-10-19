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
use tore_sim::airport::Allegiance;
use tore_sim::combat::live::{DEFAULT_OWNSHIP_SIDE, NO_SIDE, Side};
use tore_sim::sensors::FEET_PER_NAUTICAL_MILE as NMI;
use tore_world::readout::CockpitReadout;
use tore_world::surface::{Unit, UnitId};

/// The plot: shared with the planning map (458 by 390, leaving room for the
/// two scroll bars), so both maps scroll and zoom the same way.
const AREA: (i32, i32, i32, i32) = plan_view::PLOT;
/// Colour of the words beside a symbol pinned at the map edge.
const OFF_MAP: [u8; 4] = [255, 170, 130, 255];
/// Where the in-flight map's zoom rocker stands in its side rail.
const FLIGHT_ROCKER: (i32, i32) = (582, 372);
const INK: [u8; 4] = [232, 232, 218, 255];
const UNKNOWN: [u8; 4] = [255, 223, 92, 255];

#[derive(Clone, Copy)]
enum Category {
    Aircraft,
    Airfields,
    Buildings,
    Surface,
    Emitters,
    /// The mission's markup (CL-MAP): the starts, the approximate objective
    /// area and the route.
    Mission,
}
impl Category {
    const ALL: [Self; 6] = [
        Self::Aircraft,
        Self::Airfields,
        Self::Buildings,
        Self::Surface,
        Self::Emitters,
        Self::Mission,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Aircraft => "AIRCRAFT",
            Self::Airfields => "AIRFIELDS",
            Self::Buildings => "BUILDINGS",
            Self::Surface => "SURFACE",
            Self::Emitters => "EMITTERS",
            Self::Mission => "MISSION",
        }
    }
    fn rect(self) -> (i32, i32, i32, i32) {
        (504, 76 + self as i32 * 44, 120, 30)
    }
}
struct Filters {
    visible: [bool; 6],
    pressed: Option<usize>,
}
impl Default for Filters {
    fn default() -> Self {
        Self {
            visible: [true, true, false, true, true, true],
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

/// The tile of an airfield for a viewer on side `own`. The scene records an
/// airport from Blue's point of view, so a Redfor viewer sees the two swapped;
/// a neutral field, or one whose side is unknown, claims neither.
fn airfield_tile(allegiance: Allegiance, own: Side) -> Tile {
    match allegiance.seen_by(own != DEFAULT_OWNSHIP_SIDE) {
        Allegiance::Friendly => Tile::Friendly,
        Allegiance::Hostile => Tile::Hostile,
        Allegiance::Neutral | Allegiance::Unknown => Tile::Neutral,
    }
}

/// The in-flight map. Its view is the planning map's ([`PlanView`]): the
/// same zoom steps, scroll bars and rocker, and the same rule that no black
/// ever shows, but it follows the player until the player scrolls away.
#[derive(Default)]
pub struct Map {
    pub open: bool,
    filters: Filters,
    /// Made on the first draw, when the theater's size is known.
    view: Option<PlanView>,
    /// Keys pressed before the first draw (the previews), kept for it.
    pending: Vec<String>,
    /// The theater picture as last drawn, while the view holds still.
    ground: Option<Ground>,
}
impl Map {
    /// A click of the left button at `point`: the scroll bars and the zoom
    /// rocker take it first, then the layer buttons.
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) {
        if let Some(view) = &mut self.view {
            view.pointer(point);
            if (down && view.down()) || (!down && view.up()) {
                return;
            }
        }
        self.filters.pointer(point, down);
    }
    /// The pointer moved: drags a held scroll thumb.
    pub fn moved(&mut self, point: Option<(f64, f64)>) {
        if let Some(view) = &mut self.view {
            view.pointer(point);
        }
    }
    pub fn cancel_press(&mut self) {
        self.filters.pressed = None;
        if let Some(view) = &mut self.view {
            view.up();
        }
    }

    pub fn key(&mut self, key: &str) {
        match &mut self.view {
            Some(view) => view.key(key),
            None if self.pending.len() < 64 => self.pending.push(key.to_owned()),
            None => {}
        }
    }
    /// The view, made for `world`'s theater the first time it is wanted.
    fn view_for(&mut self, world: &Terrain) -> &mut PlanView {
        let pending = &mut self.pending;
        self.view.get_or_insert_with(|| {
            let theater = &world.theater;
            let mut view = PlanView::flight(
                [
                    theater.cols as f64 * f64::from(CELL_FEET),
                    theater.rows as f64 * f64::from(CELL_FEET),
                ],
                FLIGHT_ROCKER,
            );
            for key in pending.drain(..) {
                view.key(&key);
            }
            view
        })
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
        .text(font, "CLICK TO TOGGLE", 504, 336);
    }
    /// Draws the map. `markup` is the mission's own (CL-MAP): the starts, the
    /// approximate objective area and the route, built from the mission data
    /// as the creator's map is; `own` is the viewer's side, whose start the
    /// route leaves.
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
        markup: Option<&Plan>,
    ) {
        Canvas(pixels).rect((0, 0, 640, 480), [72, 72, 72, 255]);
        let view = self.view_for(world);
        view.follow([state.position[0], state.position[2]]);
        view.advance(std::time::Instant::now());
        let projection = view.projection();
        let scale_nmi = view.scale_nmi();
        let icons = sprites.get("MCICONS.PIC");
        let mut ground = self.ground.take();
        draw_ground(
            pixels,
            world,
            &projection,
            sprites.get(&world.theater.map),
            |color| scenery.palette[usize::from(color)],
            &mut ground,
        );
        self.ground = ground;
        let mut labels = Vec::new();
        let mut markers = Vec::new();
        let player = projection.locate(state.position);
        if let Some(((x, y), _)) = player {
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
        // The mission's markup: the objective area and the route under
        // everything, then the starts.
        let mut start_fields: &[u32] = &[];
        if self.filters.shows(Category::Mission)
            && let Some(plan) = markup
        {
            let viewer_side = if own == DEFAULT_OWNSHIP_SIDE {
                PlanSide::Blue
            } else {
                PlanSide::Red
            };
            draw_objective(
                pixels,
                font,
                &projection,
                plan,
                Some(viewer_side),
                &mut markers,
                &mut labels,
            );
            let placed = place_starts(
                pixels,
                &projection,
                plan,
                viewer_side,
                &mut markers,
                &mut labels,
            );
            draw_placed(pixels, icons, font, &placed, &mut labels);
            start_fields = &plan.start_fields;
        }
        // Navigation landmarks are known independently of aircraft sensors.
        for runway in world
            .airport_scene
            .runways
            .iter()
            .filter(|_| self.filters.shows(Category::Airfields))
            // A start that names its airfield stands for it.
            .filter(|runway| !start_fields.contains(&runway.object))
        {
            if let Some((point, _)) = projection.locate(runway.surface.center) {
                let (x, y) = place_marker(pixels, point, &mut markers, &mut labels);
                let airport = world
                    .airport_scene
                    .airports
                    .iter()
                    .find(|a| a.id == runway.airport);
                let tile = airport.map_or(Tile::Neutral, |a| airfield_tile(a.allegiance, own));
                symbol(pixels, icons, x, y, Some(15), tile);
                let name = airport.map_or(runway.name.as_str(), |a| a.name.as_str());
                // A STOL runway says so (slice CL-AP, John 2026-10-11).
                let name = tore_sim::stol::label(name, tore_sim::stol::runway(runway));
                label(pixels, font, &name, (x, y + 12), INK, &mut labels);
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
            let Some((point, off_map)) = projection.locate(contact.position) else {
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
            if off_map {
                off_map_label(pixels, font, (x, y), &mut labels);
            }
        }
        // Passive noise has no measured location. Show its direction at ownship.
        if let Some(((x, y), off_map)) = player {
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
            if off_map {
                off_map_label(pixels, font, (x, y), &mut labels);
            }
        }
        self.draw_filters(pixels, font, sprites);
        if let Some(view) = &self.view {
            view.draw_bars(pixels);
            view.draw_rocker(pixels, font, sprites, (500, 30, 128, 408));
        }
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
        p.text(font, &format!("{scale_nmi} NAUTICAL MILES"), 174, 9);
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
/// Everything the theater picture under the plot is made from.
#[derive(Default, PartialEq)]
struct GroundKey {
    center: [u64; 2],
    feet_per_pixel: u64,
    area: (i32, i32, i32, i32),
    bounds: [u64; 2],
    /// The theater: its names, size, and where its cells are held.
    theater: (String, String, usize, usize, usize, usize),
    /// The imported picture, when there is one: its size and where it is held.
    background: Option<(usize, usize, usize)>,
    palette: Vec<[u8; 3]>,
}

/// The theater picture as last drawn: the same while its key is.
struct Ground {
    key: GroundKey,
    /// The plot's rows, one after another.
    picture: Vec<u8>,
}

/// The theater under the plot: the imported map picture when there is one,
/// else each terrain cell's colour (`palette` for the land, the sea blue).
/// `kept` holds the last picture, which is copied back when nothing it is
/// made from changed.
fn draw_ground(
    pixels: &mut [u8],
    world: &Terrain,
    projection: &Projection,
    background: Option<&Sprite>,
    palette: impl Fn(u8) -> [u8; 3],
    kept: &mut Option<Ground>,
) {
    let area = projection.area;
    let theater = &world.theater;
    let key = GroundKey {
        center: projection.center.map(f64::to_bits),
        feet_per_pixel: projection.feet_per_pixel.to_bits(),
        area,
        bounds: projection.bounds.map(f64::to_bits),
        theater: (
            theater.name.clone(),
            theater.map.clone(),
            theater.cols,
            theater.rows,
            theater.cells.as_ptr() as usize,
            theater.coarse.as_ptr() as usize,
        ),
        background: background.map(|map| (map.width, map.height, map.rgba.as_ptr() as usize)),
        palette: (0..=255).map(&palette).collect(),
    };
    let span = area.0.max(0) as usize * 4..(area.0 + area.2).max(0) as usize * 4;
    let rows = area.1.max(0) as usize..(area.1 + area.3).max(0) as usize;
    let mut ground = kept.take().unwrap_or_else(|| Ground {
        key: GroundKey::default(),
        picture: Vec::new(),
    });
    if ground.key == key && ground.picture.len() == rows.len() * span.len() {
        for (row, picture) in rows.zip(ground.picture.chunks_exact(span.len())) {
            pixels[row * 640 * 4 + span.start..row * 640 * 4 + span.end].copy_from_slice(picture);
        }
        *kept = Some(ground);
        return;
    }
    for y in area.1..area.1 + area.3 {
        for x in area.0..area.0 + area.2 {
            let [mut east, mut north] = projection.world(x, y);
            // A pixel row or column that starts exactly on the theater's far
            // edge is still the theater's last cell, not the void beyond it
            // (a view held against the north edge starts there).
            let (width, height) = (
                world.theater.cols as f64 * f64::from(CELL_FEET),
                world.theater.rows as f64 * f64::from(CELL_FEET),
            );
            if (width..width + projection.feet_per_pixel).contains(&east) {
                east = width - 1e-3;
            }
            if (height..height + projection.feet_per_pixel).contains(&north) {
                north = height - 1e-3;
            }
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
    ground.picture.clear();
    for row in rows {
        ground
            .picture
            .extend_from_slice(&pixels[row * 640 * 4 + span.start..row * 640 * 4 + span.end]);
    }
    ground.key = key;
    *kept = Some(ground);
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

/// Colour of the objective area's ring and its words.
const OBJECTIVE: [u8; 4] = [255, 223, 92, 255];
/// The dark edge that keeps the objective area visible on light ground.
const OBJECTIVE_EDGE: [u8; 4] = [24, 26, 28, 255];

mod plan_view;
pub use plan_view::PlanView;

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
    /// The plot the projection draws into.
    area: (i32, i32, i32, i32),
    /// The theater's size, east and north feet: beyond it is off the map.
    bounds: [f64; 2],
}
impl Projection {
    /// True for a position outside the theater's map area.
    fn off_map(&self, position: [f64; 3]) -> bool {
        position[0] < 0.
            || position[2] < 0.
            || position[0] > self.bounds[0]
            || position[2] > self.bounds[1]
    }
    /// The position itself, or for one off the map the nearest point on the
    /// map's edge: the direction it lies in.
    fn pinned(&self, position: [f64; 3]) -> [f64; 3] {
        [
            position[0].clamp(0., self.bounds[0]),
            position[1],
            position[2].clamp(0., self.bounds[1]),
        ]
    }
    /// Where a position is drawn, and whether it is off the map. A position
    /// off the map is pinned at the map's edge, which shows only while that
    /// edge is in view; a symbol that would spill over the frame is held just
    /// inside it.
    fn locate(&self, position: [f64; 3]) -> Option<((i32, i32), bool)> {
        let off = self.off_map(position);
        let (x, y) = self.unclipped(self.pinned(position))?;
        let area = self.area;
        let (left, top) = (f64::from(area.0), f64::from(area.1));
        if x < left || y < top || x > left + f64::from(area.2) || y > top + f64::from(area.3) {
            return None;
        }
        // Held inside the margin the symbols and labels need.
        let (x, y) = (
            (x as i32).clamp(area.0 + 12, area.0 + area.2 - 13),
            (y as i32).clamp(area.1 + 12, area.1 + area.3 - 25),
        );
        Some(((x, y), off))
    }
    fn inside(&self, x: i32, y: i32) -> bool {
        let area = self.area;
        (area.0 + 12..area.0 + area.2 - 12).contains(&x)
            && (area.1 + 12..area.1 + area.3 - 12).contains(&y)
    }
    /// Where a position falls on the screen, inside the plot or not.
    fn unclipped(&self, position: [f64; 3]) -> Option<(f64, f64)> {
        position.iter().all(|p| p.is_finite()).then(|| {
            (
                f64::from(self.area.0 + self.area.2 / 2)
                    + (position[0] - self.center[0]) / self.feet_per_pixel,
                f64::from(self.area.1 + self.area.3 / 2)
                    - (position[2] - self.center[1]) / self.feet_per_pixel,
            )
        })
    }
    fn world(&self, x: i32, y: i32) -> [f64; 2] {
        [
            self.center[0]
                + (x as f64 - f64::from(self.area.0 + self.area.2 / 2)) * self.feet_per_pixel,
            self.center[1]
                - (y as f64 - f64::from(self.area.1 + self.area.3 / 2)) * self.feet_per_pixel,
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
    place_marker_in(AREA, pixels, point, occupied, labels)
}
fn place_marker_in(
    area: (i32, i32, i32, i32),
    pixels: &mut [u8],
    point: (i32, i32),
    occupied: &mut Vec<(i32, i32)>,
    labels: &mut Vec<(i32, i32, i32, i32)>,
) -> (i32, i32) {
    let free = |(x, y): (i32, i32)| {
        x >= area.0 + 12
            && x < area.0 + area.2 - 12
            && y >= area.1 + 12
            && y < area.1 + area.3 - 24
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
            clip: area,
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
    at: (i32, i32),
    color: [u8; 4],
    occupied: &mut Vec<(i32, i32, i32, i32)>,
) {
    label_in(AREA, pixels, font, text, at, color, occupied);
}
fn label_in(
    area: (i32, i32, i32, i32),
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
    let x = (x - w / 2).clamp(area.0 + 2, (area.0 + area.2 - w - 2).max(area.0 + 2));
    let fits = |y: i32| {
        y > area.1
            && y + h < area.1 + area.3
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
        clip: area,
        color,
    }
    .text(font, &text, x, y);
}

/// "OFF MAP" beside a symbol pinned at the map's edge: the thing it stands
/// for lies beyond the theater, in the direction the symbol sits.
fn off_map_label(
    pixels: &mut [u8],
    font: &Font,
    (x, y): (i32, i32),
    labels: &mut Vec<(i32, i32, i32, i32)>,
) {
    label_in(
        AREA,
        pixels,
        font,
        "OFF MAP",
        (x, y - 11 - font.height as i32),
        OFF_MAP,
        labels,
    );
}

/// The part of the segment `a` to `b` inside `area`, if any.
fn clip_segment(
    area: (i32, i32, i32, i32),
    a: (f64, f64),
    b: (f64, f64),
) -> Option<((f64, f64), (f64, f64))> {
    let (left, top) = (f64::from(area.0), f64::from(area.1));
    let (right, bottom) = (left + f64::from(area.2), top + f64::from(area.3));
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0_f64, 1_f64);
    for (p, q) in [
        (-dx, a.0 - left),
        (dx, right - a.0),
        (-dy, a.1 - top),
        (dy, bottom - a.1),
    ] {
        if p == 0. {
            if q < 0. {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0. {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 < t1).then_some((
        (a.0 + dx * t0, a.1 + dy * t0),
        (a.0 + dx * t1, a.1 + dy * t1),
    ))
}

/// The route: a thin line from `side`'s start to the objective area's edge,
/// in the map's line style (one pixel, the map's ink over a dark edge so it
/// shows on any ground). The creator's mission map drew no lines before, so
/// this is the one line both maps draw. A route off the map runs to the edge
/// the start or the area is pinned at.
fn draw_route(pixels: &mut [u8], projection: &Projection, plan: &Plan, side: PlanSide) {
    let (Some((center, radius)), Some(start)) = (
        plan.objective,
        plan.markers.iter().find(|marker| marker.side == side),
    ) else {
        return;
    };
    let (Some(from), Some(to)) = (
        projection.unclipped(projection.pinned(start.position)),
        projection.unclipped(projection.pinned([center[0], 0., center[1]])),
    ) else {
        return;
    };
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let distance = dx.hypot(dy);
    let r = (radius / projection.feet_per_pixel).max(7.);
    // Clear of the start's symbol at one end and the area's ring at the other.
    if distance <= r + 14. {
        return;
    }
    let begin = (from.0 + dx / distance * 12., from.1 + dy / distance * 12.);
    let end = (
        to.0 - dx / distance * (r + 2.),
        to.1 - dy / distance * (r + 2.),
    );
    for (color, shift) in [(OBJECTIVE_EDGE, 1.), (INK, 0.)] {
        if let Some((a, b)) = clip_segment(AREA, (begin.0, begin.1 + shift), (end.0, end.1 + shift))
        {
            Paint {
                pixels: &mut *pixels,
                clip: AREA,
                color,
            }
            .line(a, b);
        }
    }
}

/// The objective area, with its words, and `route` from that side's start:
/// the mission's markup that goes under every symbol. Shared by the creator's
/// map and the in-flight map, so the two draw the same circle the same way.
/// The area is an approximation of where the targets are, never the targets.
fn draw_objective(
    pixels: &mut [u8],
    font: &Font,
    projection: &Projection,
    plan: &Plan,
    route: Option<PlanSide>,
    markers: &mut Vec<(i32, i32)>,
    labels: &mut Vec<(i32, i32, i32, i32)>,
) {
    // The yellow hatching edged in dark and a yellow ring between two dark
    // ones show on sea, forest and desert alike, and never less than 7 pixels
    // in radius so the area shows at the widest zoom.
    let mut words = None;
    if let Some((center, radius)) = plan.objective {
        let at_world = [center[0], 0., center[1]];
        let off_map = projection.off_map(at_world);
        let at = if off_map {
            projection
                .locate(at_world)
                .map(|((x, y), _)| (f64::from(x), f64::from(y)))
        } else {
            projection.unclipped(at_world)
        };
        if let Some(at) = at {
            let r = (radius / projection.feet_per_pixel).max(7.);
            objective_mark(pixels, at, r, AREA);
            let point = (at.0 as i32, at.1 as i32);
            if projection.inside(point.0, point.1) {
                // Nothing is placed on top of a small area.
                if r < 20. {
                    markers.push(point);
                    labels.push((point.0 - 12, point.1 - 10, point.0 + 12, point.1 + 10));
                }
                words = Some((point.0, point.1 + r.min(20.) as i32 + 3, off_map, point));
            }
        }
    }
    if let Some(side) = route {
        draw_route(pixels, projection, plan, side);
    }
    if let Some((x, y, off_map, point)) = words {
        label_in(
            AREA,
            pixels,
            font,
            "OBJECTIVE AREA",
            (x, y),
            OBJECTIVE,
            labels,
        );
        if off_map {
            off_map_label(pixels, font, point, labels);
        }
    }
}

/// A symbol and its words, placed and waiting to be drawn.
struct Placed<'a> {
    at: (i32, i32),
    icon: usize,
    tile: Tile,
    words: &'a str,
    off_map: bool,
}

/// The tile of a start marker for a viewer on side `viewer`: the viewer's
/// own side's starts are friendly, the other side's hostile, as for airfields
/// and surface units.
fn start_tile(marker: PlanSide, viewer: PlanSide) -> Tile {
    if marker == viewer {
        Tile::Friendly
    } else {
        Tile::Hostile
    }
}

/// Places the plan's starts in the order listed, moving a crowded one aside
/// with a leader as the in-game map does. `viewer` is the side whose view the
/// map shows.
fn place_starts<'a>(
    pixels: &mut [u8],
    projection: &Projection,
    plan: &'a Plan,
    viewer: PlanSide,
    markers: &mut Vec<(i32, i32)>,
    labels: &mut Vec<(i32, i32, i32, i32)>,
) -> Vec<Placed<'a>> {
    let mut placed = Vec::new();
    for marker in &plan.markers {
        let Some((point, off_map)) = projection.locate(marker.position) else {
            continue;
        };
        let at = place_marker_in(AREA, pixels, point, markers, labels);
        let icon = (marker.heading.rem_euclid(std::f64::consts::TAU) / std::f64::consts::FRAC_PI_4)
            .round() as usize
            % 8;
        let tile = start_tile(marker.side, viewer);
        placed.push(Placed {
            at,
            icon,
            tile,
            words: &marker.label,
            off_map,
        });
    }
    placed
}

/// Draws placed symbols, then their words, so no word is covered.
fn draw_placed(
    pixels: &mut [u8],
    icons: Option<&Sprite>,
    font: &Font,
    placed: &[Placed],
    labels: &mut Vec<(i32, i32, i32, i32)>,
) {
    for item in placed {
        symbol(
            pixels,
            icons,
            item.at.0,
            item.at.1,
            Some(item.icon),
            item.tile,
        );
    }
    for item in placed {
        let (x, y) = item.at;
        label_in(AREA, pixels, font, item.words, (x, y + 12), INK, labels);
        if item.off_map {
            off_map_label(pixels, font, item.at, labels);
        }
    }
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
    fn an_airfield_is_on_the_tile_of_its_side_against_the_viewers() {
        let (blue, red) = (DEFAULT_OWNSHIP_SIDE, Side(2));
        // The scene records Blue's point of view: a Blue pilot sees a Blue
        // field as a friend and a Redfor one as a foe.
        assert_eq!(airfield_tile(Allegiance::Friendly, blue), Tile::Friendly);
        assert_eq!(airfield_tile(Allegiance::Hostile, blue), Tile::Hostile);
        // A Redfor pilot sees the two swapped.
        assert_eq!(airfield_tile(Allegiance::Friendly, red), Tile::Hostile);
        assert_eq!(airfield_tile(Allegiance::Hostile, red), Tile::Friendly);
        // A neutral field, or an unknown one, claims neither side for either.
        for own in [blue, red] {
            assert_eq!(airfield_tile(Allegiance::Neutral, own), Tile::Neutral);
            assert_eq!(airfield_tile(Allegiance::Unknown, own), Tile::Neutral);
        }
    }
    #[test]
    fn a_start_is_friendly_to_its_own_side_and_hostile_to_the_other() {
        use PlanSide::{Blue, Red};
        assert_eq!(start_tile(Blue, Blue), Tile::Friendly);
        assert_eq!(start_tile(Red, Blue), Tile::Hostile);
        // A Redfor viewer: RED START is friendly, BLUE START hostile.
        assert_eq!(start_tile(Red, Red), Tile::Friendly);
        assert_eq!(start_tile(Blue, Red), Tile::Hostile);
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
    fn projection(center: [f64; 2], width_nmi: f64) -> Projection {
        Projection {
            center,
            feet_per_pixel: width_nmi * NMI / f64::from(AREA.2),
            area: AREA,
            bounds: [300. * NMI, 300. * NMI],
        }
    }
    #[test]
    fn a_kept_theater_picture_is_the_one_drawn_afresh_while_nothing_it_is_made_from_changes() {
        let world = tore_world::test_support::terrain();
        let sprite = Sprite {
            width: 2,
            height: 2,
            rgba: vec![
                200, 10, 10, 255, 10, 200, 10, 255, 10, 10, 200, 255, 90, 90, 90, 255,
            ],
            glyphs: vec![],
        };
        let flat = |color: u8| [color, 100u8.wrapping_add(color), 50];
        let mut kept = None;
        let mut hits_possible = 0;
        // (centre, width in nautical miles, palette tweak, imported picture)
        let steps = [
            ([30_000., 30_000.], 100., 0, false),
            ([30_000., 30_000.], 100., 0, false),
            ([30_000., 30_000.], 100., 0, false),
            ([30_000., 30_001.], 100., 0, false),
            ([30_000., 30_001.], 100., 0, false),
            ([30_000., 30_001.], 100., 7, false),
            ([30_000., 30_001.], 100., 7, false),
            ([30_000., 30_001.], 50., 7, false),
            ([30_000., 30_001.], 50., 7, true),
            ([30_000., 30_001.], 50., 7, true),
            ([-4_000., 90_000.], 50., 7, true),
            ([-4_000., 90_000.], 50., 7, false),
        ];
        for (n, (center, width, tweak, imported)) in steps.into_iter().enumerate() {
            let projection = projection(center, width);
            let background = imported.then_some(&sprite);
            let palette = |color: u8| flat(color.wrapping_add(tweak));
            let mut expected = vec![72u8; 640 * 480 * 4];
            draw_ground(
                &mut expected,
                &world,
                &projection,
                background,
                palette,
                &mut None,
            );
            let mut actual = vec![72u8; 640 * 480 * 4];
            if kept.is_some() {
                hits_possible += 1;
            }
            draw_ground(
                &mut actual,
                &world,
                &projection,
                background,
                palette,
                &mut kept,
            );
            assert!(actual == expected, "step {n}");
        }
        assert!(hits_possible > 5);
    }
    #[test]
    fn north_up_projection_scale_and_edges() {
        let p = projection([1000., 2000.], 100.);
        let at = |position| p.locate(position).map(|(point, _)| point);
        // The plot's centre is (241, 225): 12 + 458 / 2 and 30 + 390 / 2.
        assert_eq!(at([1000., 0., 2000.]), Some((241, 225)));
        let quarter = (25. * NMI / p.feet_per_pixel) as i32;
        assert_eq!(
            at([1000. + 25. * NMI, 0., 2000.]),
            Some((241 + quarter, 225))
        );
        // North is up, and a pixel row is truncated, not rounded.
        let north = (225. - 25. * NMI / p.feet_per_pixel) as i32;
        assert_eq!(at([1000., 0., 2000. + 25. * NMI]), Some((241, north)));
        assert_eq!(at([1000. + 100. * NMI, 0., 2000.]), None);
        assert_eq!(at([f64::NAN, 0., 0.]), None);
        assert_eq!(p.world(241, 225), p.center);
    }
    #[test]
    fn a_position_beyond_the_theater_is_pinned_at_the_edge_it_lies_past() {
        // A view held against the theater's east edge: the plot's right side
        // is the edge itself.
        let p = projection([300. * NMI - 50. * NMI, 150. * NMI], 100.);
        let east = [325. * NMI, 0., 150. * NMI];
        assert!(p.off_map(east));
        assert!(!p.off_map([299. * NMI, 0., 150. * NMI]));
        let ((x, y), off) = p.locate(east).expect("the edge is in view");
        assert!(off);
        // Held just inside the plot's right margin, level with where it lies.
        assert_eq!(x, AREA.0 + AREA.2 - 13);
        assert_eq!(y, 225);
        // North-east of the theater's corner: pinned to the corner.
        let corner = [320. * NMI, 0., 340. * NMI];
        let p = projection([280. * NMI, 280. * NMI], 100.);
        let (point, off) = p.locate(corner).expect("the corner is in view");
        assert!(off);
        let step = (20. * NMI / p.feet_per_pixel) as i32;
        assert_eq!(point.0, 241 + step);
        assert_eq!(point.1, (225. - 20. * NMI / p.feet_per_pixel) as i32);
        // Out of sight while the edge is not in view.
        let far = projection([100. * NMI, 150. * NMI], 100.);
        assert_eq!(far.locate(east), None);
        // A position in the theater is not off the map.
        assert_eq!(
            far.locate([100. * NMI, 0., 150. * NMI]).map(|l| l.1),
            Some(false)
        );
    }
    #[test]
    fn the_map_keys_zoom_and_scroll_but_home_goes_back_to_following() {
        let mut view = PlanView::flight([300. * NMI, 300. * NMI], FLIGHT_ROCKER);
        // It opens 100 nm across and follows the player.
        assert_eq!(view.scale_nmi(), 25.);
        view.follow([150. * NMI, 150. * NMI]);
        assert_eq!(view.projection().center, [150. * NMI, 150. * NMI]);
        // The pilot scrolls: it stays where it is while the player moves on.
        view.key("ArrowUp");
        let held = view.projection().center;
        view.follow([200. * NMI, 200. * NMI]);
        assert_eq!(view.projection().center, held);
        view.key("Home");
        view.follow([200. * NMI, 200. * NMI]);
        assert_eq!(view.projection().center, [200. * NMI, 200. * NMI]);
        // Zoom keeps following and stops at the whole theater and at 25 nm.
        for _ in 0..20 {
            view.key("-");
        }
        assert_eq!(
            view.projection().feet_per_pixel * f64::from(AREA.2) / NMI,
            300.
        );
        for _ in 0..20 {
            view.key("+");
        }
        assert_eq!(view.scale_nmi(), 6.);
    }
    #[test]
    fn a_segment_is_clipped_to_the_plot() {
        let inside = clip_segment(AREA, (100., 100.), (200., 150.));
        assert_eq!(inside, Some(((100., 100.), (200., 150.))));
        // Wholly outside: nothing.
        assert_eq!(clip_segment(AREA, (-50., -50.), (-10., 20.)), None);
        // Crossing the right edge: cut at it.
        let (a, b) = clip_segment(AREA, (400., 100.), (700., 100.)).unwrap();
        assert_eq!((a, b), ((400., 100.), (f64::from(AREA.0 + AREA.2), 100.)));
    }
    fn plan(start: [f64; 3], area: [f64; 2]) -> Plan {
        Plan {
            markers: vec![PlanMarker {
                position: start,
                heading: 0.,
                side: PlanSide::Blue,
                label: "BLUE START".into(),
            }],
            objective: Some((area, 5. * NMI)),
            ..Plan::default()
        }
    }
    fn inked(pixels: &[u8]) -> usize {
        pixels
            .chunks_exact(4)
            .filter(|p| p[..3] == INK[..3])
            .count()
    }
    #[test]
    fn the_route_runs_from_the_start_to_the_area_and_not_when_the_start_is_inside_it() {
        let p = projection([150. * NMI, 150. * NMI], 100.);
        let draw = |plan: &Plan, side| {
            let mut pixels = vec![0_u8; 640 * 480 * 4];
            draw_route(&mut pixels, &p, plan, side);
            pixels
        };
        let far = plan([120. * NMI, 0., 140. * NMI], [160. * NMI, 155. * NMI]);
        assert!(inked(&draw(&far, PlanSide::Blue)) > 100);
        // The viewer's own side's start: Red has none in this plan.
        assert_eq!(inked(&draw(&far, PlanSide::Red)), 0);
        // A start within the area: no line.
        let inside = plan([150. * NMI, 0., 150. * NMI], [152. * NMI, 151. * NMI]);
        assert_eq!(inked(&draw(&inside, PlanSide::Blue)), 0);
        // No area, no route.
        let mut none = far.clone();
        none.objective = None;
        assert_eq!(inked(&draw(&none, PlanSide::Blue)), 0);
    }
    #[test]
    fn an_area_beyond_the_theater_is_pinned_and_says_so() {
        // The area's centre is 20 nm east of the edge; the view is held
        // against that edge.
        let p = projection([250. * NMI, 150. * NMI], 100.);
        let (plan, mut markers, mut labels) = (
            plan([200. * NMI, 0., 150. * NMI], [320. * NMI, 150. * NMI]),
            Vec::<(i32, i32)>::new(),
            Vec::<(i32, i32, i32, i32)>::new(),
        );
        assert!(p.off_map([320. * NMI, 0., 150. * NMI]));
        let mut pixels = vec![0_u8; 640 * 480 * 4];
        let font = Font {
            height: 8,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 6,
                    pixels: vec![],
                })
                .collect(),
        };
        draw_objective(
            &mut pixels,
            &font,
            &p,
            &plan,
            None,
            &mut markers,
            &mut labels,
        );
        // Drawn at the plot's right margin (the nearest edge point), with its
        // words and the off-map words (two labels).
        assert_eq!(labels.len(), 2);
        let at = |x: i32, y: i32| pixels[(y as usize * 640 + x as usize) * 4..][..3].to_vec();
        assert!(
            (AREA.0 + AREA.2 - 30..AREA.0 + AREA.2)
                .flat_map(|x| (200..250).map(move |y| (x, y)))
                .any(|(x, y)| at(x, y) == OBJECTIVE[..3]),
            "the area is drawn at the right edge"
        );
    }
}
