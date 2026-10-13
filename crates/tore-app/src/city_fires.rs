//! Fires on the city fill's knocked-down buildings (nr-fx2, John
//! 2026-10-13: "city fill buildings burn too, no cap"). A background city
//! building a ground burst flattened ([`tore_world::city::CityFill::flatten`])
//! burns for the rest of the mission, with no limit on how many.
//!
//! Presentation only: nothing here reaches the simulation, the wire, a
//! checkpoint or a recording. The set of burning buildings *is* the fill's
//! flattened set, which the host already tells every player
//! ([`tore_world::city::CityFill::knock_down`]) and the world checkpoint
//! carries, so every machine shows the same fires without a single new
//! message. (A replay does not record the fill's flattened set, so a replay
//! shows its city whole and unburnt, as before.)
//!
//! What keeps it cheap with thousands burning:
//!
//! - **Nothing is kept per frame but the nearest.** The burning set is
//!   rebuilt only when the fill's version changes. Each frame the nearest
//!   [`MAX_SPRITES`] fires within their reach ([`REACH_WIDTHS`] times their
//!   width) become fire sprites, in the same instanced pass as the
//!   explosions ([`crate::effect_renderer`]), and the nearest
//!   [`MAX_COLUMNS`] within [`COLUMN_RANGE_FT`] get a smoke column.
//! - **Smoke is analytic, not simulated.** A column is computed from its
//!   age ([`crate::surface_fx::column`], the wreck smoke's own shape) at
//!   half the wreck column's puff density ([`STEP_TICKS`]), about
//!   [`PUFFS_PER_COLUMN`] puffs a column, so the smoke budget this adds is
//!   at most `MAX_COLUMNS * PUFFS_PER_COLUMN` (about 1,200) puffs, a
//!   separate budget from the simulation's 8,192 and the renderer's own cap.
//!   Farther fires show their flame only.
//!
//! Agent decisions (2026-10-13): the caps and ranges above, the fire width
//! (a building fire's rule: 0.6 of the smaller plan side, 24 to 90 ft, as
//! `surface_fx::object_fire`), and a column density of half the wrecks'.
use crate::gun_flash::Puff;
use crate::surface_fx::{column, wreck};
use std::time::Instant;
use tore_sim::attitude::Vector;
use tore_world::city::CityFill;

/// Fire sprites drawn at most, the nearest.
pub const MAX_SPRITES: usize = 1_024;
/// A fire is drawn out to this many of its widths (a 30 ft fire to 60,000 ft,
/// where it is a fraction of a pixel).
pub const REACH_WIDTHS: f64 = 2_000.;
/// Smoke columns drawn at most, the nearest.
pub const MAX_COLUMNS: usize = 32;
/// Columns are drawn within this range, feet (8 nautical miles).
pub const COLUMN_RANGE_FT: f64 = 8. * 6_076.12;
/// Ticks between a column's puffs: half the density of a wreck's column.
pub const STEP_TICKS: u64 = 2 * wreck::STEP_TICKS;
/// Puffs in a full column.
pub const PUFFS_PER_COLUMN: u64 = wreck::PUFF_TICKS / STEP_TICKS + 1;
/// A fire's width as a share of the building's smaller plan side, and its
/// limits in feet (the building fire's rule, `surface_fx::object_fire`).
const WIDTH_SHARE: f64 = 0.6;
const WIDTH_FT: [f64; 2] = [24., 90.];
/// A building already burning when first seen (a checkpoint, a join) starts
/// with its column up.
const WARM_TICKS: f64 = wreck::PUFF_TICKS as f64;

/// One burning building.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blaze {
    /// The fill building's index, which seeds its look.
    pub building: u32,
    /// Where its fire stands: its base centre on the ground.
    pub position: Vector,
    /// How wide its fire is drawn, feet.
    pub width: f64,
    /// The presentation tick it was first seen burning (negative: warm).
    pub born: f64,
}

/// One fire sprite to draw: where, how wide, and its animation's offset in
/// ticks, so neighbours do not flicker in step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CityFire {
    pub position: Vector,
    pub width: f64,
    pub phase: u64,
}

/// What a frame draws of the burning city.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub fires: Vec<CityFire>,
    pub puffs: Vec<Puff>,
}

/// The burning buildings of one city fill, and the clock their animation
/// runs on.
pub struct CityFires {
    version: Option<u64>,
    blazes: Vec<Blaze>,
    epoch: Instant,
}

impl Default for CityFires {
    fn default() -> Self {
        Self::new()
    }
}

/// A fire's width, feet, for a building whose footprint is `half` (right,
/// forward) feet from its centre.
pub fn width_of(half: [i64; 2]) -> f64 {
    let small = 2. * half[0].min(half[1]) as f64;
    (small * WIDTH_SHARE).clamp(WIDTH_FT[0], WIDTH_FT[1])
}

/// How much a column of a fire `width` feet wide is scaled: a wreck column's
/// rule by hit points inverted (a 30 ft fire is the base size), 0.6 to 2.4.
fn column_scale(width: f64) -> f64 {
    (width / 30.).sqrt().clamp(0.6, 2.4)
}

impl CityFires {
    pub fn new() -> Self {
        Self {
            version: None,
            blazes: Vec::new(),
            epoch: Instant::now(),
        }
    }

    /// Presentation ticks since this tracker started.
    pub fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64() * 120.
    }

    /// The buildings burning now.
    pub fn blazes(&self) -> &[Blaze] {
        &self.blazes
    }

    /// Follows `fill`: when its version changed, the burning set becomes
    /// its flattened buildings, a newly flattened one starting at `now`.
    pub fn observe(&mut self, fill: &CityFill, now: f64) {
        let version = fill.version();
        if self.version == Some(version) {
            return;
        }
        let first = self.version.is_none();
        self.version = Some(version);
        let old = std::mem::take(&mut self.blazes);
        self.blazes = fill
            .flattened()
            .into_iter()
            .map(|n| {
                if let Ok(at) = old.binary_search_by_key(&n, |b| b.building) {
                    return old[at];
                }
                let index = n as usize;
                let instance = &fill.layout.instances[index];
                let piece = &fill.layout.kit.pieces[usize::from(instance.piece)];
                Blaze {
                    building: n,
                    position: [
                        f64::from(instance.x),
                        f64::from(fill.base[index]),
                        f64::from(instance.z),
                    ],
                    width: width_of(piece.half),
                    born: if first { now - WARM_TICKS } else { now },
                }
            })
            .collect();
    }

    /// What to draw for a camera at `eye` at presentation tick `now`: the
    /// nearest fires and the nearest columns' smoke.
    pub fn frame(&self, eye: Vector, now: f64) -> Frame {
        let mut near: Vec<(f64, usize)> = self
            .blazes
            .iter()
            .enumerate()
            .filter_map(|(at, b)| {
                let d = (b.position[0] - eye[0])
                    .hypot(b.position[2] - eye[2])
                    .hypot(b.position[1] - eye[1]);
                (d <= b.width * REACH_WIDTHS).then_some((d, at))
            })
            .collect();
        if near.len() > MAX_SPRITES {
            near.select_nth_unstable_by(MAX_SPRITES - 1, |a, b| a.0.total_cmp(&b.0));
            near.truncate(MAX_SPRITES);
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut frame = Frame::default();
        let mut columns = 0;
        for &(d, at) in &near {
            let b = &self.blazes[at];
            frame.fires.push(CityFire {
                position: b.position,
                width: b.width,
                phase: u64::from(b.building).wrapping_mul(0x9e37_79b9) % 120,
            });
            if columns < MAX_COLUMNS && d <= COLUMN_RANGE_FT {
                columns += 1;
                column(
                    u64::from(b.building) ^ 0x5eed,
                    b.position,
                    column_scale(b.width),
                    now - b.born,
                    STEP_TICKS,
                    1.,
                    &mut frame.puffs,
                );
            }
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tore_world::city::{
        Instance, Layout,
        fill::City,
        kit::{Class, Kit, Piece},
        mask::LightMap,
    };

    /// A fill of `count` identical 40 by 60 ft buildings on a 100 ft grid.
    pub(crate) fn fill(count: usize) -> CityFill {
        let piece = Piece {
            source: "TEST".into(),
            shape: "TEST.SH".into(),
            faces: Vec::new(),
            origin: [0.; 3],
            scale: 1.,
            half: [20, 30],
            height: 40,
            class: Class::House,
            cut: false,
        };
        let instances: Vec<Instance> = (0..count)
            .map(|n| Instance {
                piece: 0,
                x: (n % 100) as i32 * 100,
                z: (n / 100) as i32 * 100,
                heading: 0,
                core: false,
                city: 0,
            })
            .collect();
        let layout = Arc::new(Layout {
            layout: "TEST".into(),
            map: LightMap::default(),
            kit: Kit {
                pieces: vec![piece],
            },
            cities: Vec::<City>::new(),
            instances,
            candidates: count,
            inputs: Vec::new(),
        });
        CityFill::assemble(layout, vec![5.; count], vec![false; count])
    }

    #[test]
    fn a_flattened_building_burns_where_it_stood_with_a_building_fires_width() {
        let fill = fill(10);
        let mut fires = CityFires::new();
        fires.observe(&fill, 0.);
        assert!(fires.blazes().is_empty());
        fill.knock_down([3, 4]);
        fires.observe(&fill, 500.);
        let at: Vec<_> = fires.blazes().iter().map(|b| b.position).collect();
        assert_eq!(at, [[300., 5., 0.], [400., 5., 0.]]);
        // 0.6 of the smaller side (40 ft) is 24 ft.
        assert!(fires.blazes().iter().all(|b| b.width == 24.));
        // Newly flattened: starts at once; first seen at the start: warm.
        assert!(fires.blazes().iter().all(|b| b.born == 500.));
        let again = CityFires::new();
        let mut again = again;
        again.observe(&fill, 500.);
        assert!(again.blazes().iter().all(|b| b.born == 500. - WARM_TICKS));
    }

    #[test]
    fn buildings_already_burning_keep_their_start_and_a_reset_puts_them_out() {
        let fill = fill(10);
        let mut fires = CityFires::new();
        fill.knock_down([1]);
        fires.observe(&fill, 0.);
        fill.knock_down([2]);
        fires.observe(&fill, 700.);
        let born: Vec<f64> = fires.blazes().iter().map(|b| b.born).collect();
        assert_eq!(born, [-WARM_TICKS, 700.]);
        fill.reset();
        fires.observe(&fill, 900.);
        assert!(fires.blazes().is_empty());
    }

    #[test]
    fn only_the_nearest_fires_and_columns_are_drawn_however_many_burn() {
        let fill = fill(5_000);
        fill.knock_down(0..5_000);
        let mut fires = CityFires::new();
        fires.observe(&fill, 0.);
        assert_eq!(fires.blazes().len(), 5_000);
        let frame = fires.frame([0., 500., 0.], 100_000.);
        assert_eq!(frame.fires.len(), MAX_SPRITES);
        assert!(frame.puffs.len() <= MAX_COLUMNS * PUFFS_PER_COLUMN as usize);
        assert!(frame.puffs.len() > 10 * PUFFS_PER_COLUMN as usize);
        // The nearest come first.
        let first = frame.fires[0].position;
        assert_eq!((first[0], first[2]), (0., 0.));
        // Farther than a fire's reach nothing is drawn.
        let far = fires.frame([0., 500., 2_000_000.], 100_000.);
        assert!(far.fires.is_empty() && far.puffs.is_empty());
    }

    #[test]
    fn a_young_column_is_shorter_and_a_warm_one_stands() {
        let fill = fill(2);
        let mut fires = CityFires::new();
        fires.observe(&fill, 0.);
        fill.knock_down([0]);
        fires.observe(&fill, 1_000.);
        let eye = [0., 100., 0.];
        let young = fires.frame(eye, 1_000. + 480.);
        let grown = fires.frame(eye, 1_000. + 20_000.);
        assert_eq!(young.puffs.len(), 480 / STEP_TICKS as usize + 1);
        assert_eq!(grown.puffs.len() as u64, PUFFS_PER_COLUMN);
    }

    #[test]
    #[ignore = "a measurement: run with --release --nocapture"]
    fn stress_frame_cost_of_burning_cities() {
        for count in [500, 5_000, 50_000] {
            let fill = fill(count);
            fill.knock_down(0..count as u32);
            let mut fires = CityFires::new();
            fires.observe(&fill, 0.);
            let eye = [2_500., 800., 2_500.];
            let start = Instant::now();
            let frames = 200;
            let mut drawn = 0;
            for f in 0..frames {
                let frame = fires.frame(eye, 50_000. + f as f64);
                drawn += frame.fires.len() + frame.puffs.len();
            }
            let per = start.elapsed().as_secs_f64() * 1e3 / frames as f64;
            println!(
                "city fires: {count} burning, {:.3} ms a frame on the CPU, {} sprites and puffs drawn",
                per,
                drawn / frames
            );
        }
    }
}
