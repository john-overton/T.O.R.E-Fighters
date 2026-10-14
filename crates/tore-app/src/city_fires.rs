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
//! message. A replay records the set by where each building stands
//! (nr-fx3) and the tick it fell on, and shows the fires and their columns
//! from that tick (nr-fx4, [`Falls`]).
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
//!   age ([`crate::surface_fx::column`], the wreck smoke's own shape; a
//!   function of the age alone, so a replay seek shows the column at the
//!   height it had, nr-fx4) at
//!   half the wreck column's puff density ([`STEP_TICKS`]), about
//!   [`PUFFS_PER_COLUMN`] puffs a column, so the smoke budget this adds is
//!   at most `MAX_COLUMNS * PUFFS_PER_COLUMN` (about 1,200) puffs, a
//!   separate budget from the simulation's 8,192 and the renderer's own cap.
//!   Farther fires show their flame only.
//!
//! - **Past the sprites, glow points.** The nearest [`MAX_FAR_GLOWS`] of the
//!   rest are drawn as night glow points only ([`crate::night_glow`], nr-fx3),
//!   so a burning city shows to the limit of the night scene.
//!
//! Agent decisions (2026-10-13): the caps and ranges above, the fire width
//! (a building fire's rule: 0.6 of the smaller plan side, 24 to 90 ft, as
//! `surface_fx::object_fire`), and a column density of half the wrecks'.
use crate::gun_flash::Puff;
use crate::surface_fx::{column, wreck};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// The burning buildings past the fire sprites' reach or budget, the
    /// nearest [`MAX_FAR_GLOWS`]: drawn as night glow points only
    /// (nr-fx3), so a burning city shows from afar.
    pub glows: Vec<crate::night_glow::Glow>,
    /// The tallest column's top above its building's ground, feet (the
    /// centre of its highest puff).
    pub tallest: f64,
}

/// What a frame drew of the burning city, in numbers (nr-fx4): printed by
/// `--replay-stills` so a seek and a play-through can be compared as text.
/// `digest` covers every puff's place, size and opacity.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Report {
    /// Buildings burning, whether drawn or not.
    pub burning: usize,
    /// Fire sprites drawn.
    pub fires: usize,
    /// Smoke puffs drawn.
    pub puffs: usize,
    /// The tallest column's top above its ground, feet.
    pub tallest_ft: f64,
    pub digest: u64,
}

impl Frame {
    /// This frame in numbers, for `burning` buildings on fire.
    pub fn report(&self, burning: usize) -> Report {
        let mut digest = 0xcbf2_9ce4_8422_2325_u64;
        for p in &self.puffs {
            for v in [
                p.position[0],
                p.position[1],
                p.position[2],
                p.radius,
                f64::from(p.opacity),
            ] {
                digest = (digest ^ v.to_bits()).wrapping_mul(0x0100_0000_01b3);
            }
        }
        Report {
            burning,
            fires: self.fires.len(),
            puffs: self.puffs.len(),
            tallest_ft: self.tallest,
            digest,
        }
    }
}

/// Burning buildings drawn as glow points only, at most, the nearest past
/// the fire sprites (agent decision, nr-fx3).
pub const MAX_FAR_GLOWS: usize = 2_048;

/// When each flattened building fell, in the clock the columns age on
/// (nr-fx4): a replay's simulation ticks. With it a column's age is
/// `tick - fell`, whether the playhead played to the tick, jumped there or
/// went back; without it (live flight) a building's fire starts when the
/// tracker first sees it flat.
#[derive(Debug)]
pub struct Falls {
    id: u64,
    fell: HashMap<u32, f64>,
}

impl Falls {
    /// `fell` maps a fill building's index to the tick it fell on.
    pub fn new(fell: HashMap<u32, f64>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            fell,
        }
    }

    /// The tick building `n` fell on.
    pub fn get(&self, n: u32) -> Option<f64> {
        self.fell.get(&n).copied()
    }
}

/// A replay's clock for the burning city: the playhead in ticks and when
/// each building fell ([`Falls`]).
#[derive(Clone, Debug)]
pub struct Clock {
    pub tick: f64,
    pub falls: Arc<Falls>,
}

/// The burning buildings of one city fill, and the clock their animation
/// runs on.
pub struct CityFires {
    /// The fill (by address), its version and the [`Falls`] the burning set
    /// was last built from.
    seen: Option<(usize, u64, Option<u64>)>,
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
            seen: None,
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
        self.observe_with(fill, now, None);
    }

    /// As [`Self::observe`], with a replay's fall ticks (nr-fx4): every
    /// building burns from the tick it fell on, in the same clock as `now`,
    /// so the burning set is a function of the fill and the falls alone. A
    /// building the falls do not name (a recording of an older build, a
    /// place the rebuilt fill moved) starts warm, with its column up.
    pub fn observe_with(&mut self, fill: &CityFill, now: f64, falls: Option<&Falls>) {
        let key = (
            fill as *const CityFill as usize,
            fill.version(),
            falls.map(|f| f.id),
        );
        if self.seen == Some(key) {
            return;
        }
        // The first look, another fill, or a change of clock: whatever
        // burns was already burning.
        let first = self
            .seen
            .is_none_or(|(at, _, was)| at != key.0 || was.is_some() != key.2.is_some());
        self.seen = Some(key);
        let old = std::mem::take(&mut self.blazes);
        self.blazes = fill
            .flattened()
            .into_iter()
            .map(|n| {
                if falls.is_none()
                    && !first
                    && let Ok(at) = old.binary_search_by_key(&n, |b| b.building)
                {
                    return old[at];
                }
                let index = n as usize;
                let instance = &fill.layout.instances[index];
                let piece = &fill.layout.kit.pieces[usize::from(instance.piece)];
                let warm = now - WARM_TICKS;
                Blaze {
                    building: n,
                    position: [
                        f64::from(instance.x),
                        f64::from(fill.base[index]),
                        f64::from(instance.z),
                    ],
                    width: width_of(piece.half),
                    born: match falls {
                        Some(falls) => falls.get(n).unwrap_or(warm),
                        None if first => warm,
                        None => now,
                    },
                }
            })
            .collect();
    }

    /// What to draw for a camera at `eye` at presentation tick `now`: the
    /// nearest fires and the nearest columns' smoke.
    pub fn frame(&self, eye: Vector, now: f64) -> Frame {
        let mut near: Vec<(f64, usize)> = Vec::new();
        let mut far: Vec<(f64, usize)> = Vec::new();
        for (at, b) in self.blazes.iter().enumerate() {
            let d = (b.position[0] - eye[0])
                .hypot(b.position[2] - eye[2])
                .hypot(b.position[1] - eye[1]);
            if d <= b.width * REACH_WIDTHS {
                near.push((d, at));
            } else {
                far.push((d, at));
            }
        }
        if near.len() > MAX_SPRITES {
            near.select_nth_unstable_by(MAX_SPRITES - 1, |a, b| a.0.total_cmp(&b.0));
            far.extend(near.drain(MAX_SPRITES..));
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        if far.len() > MAX_FAR_GLOWS {
            far.select_nth_unstable_by(MAX_FAR_GLOWS - 1, |a, b| a.0.total_cmp(&b.0));
            far.truncate(MAX_FAR_GLOWS);
        }
        let mut frame = Frame {
            glows: far
                .iter()
                .map(|&(_, at)| {
                    let b = &self.blazes[at];
                    crate::night_glow::Glow {
                        // The middle of the fire's art (FIRE.SH is about
                        // as tall as it is wide).
                        position: [b.position[0], b.position[1] + b.width * 0.4, b.position[2]],
                        width: b.width,
                        strength: crate::night_glow::fire_light(b.width),
                        heat: 0.,
                    }
                })
                .collect(),
            ..Frame::default()
        };
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
                let from = frame.puffs.len();
                column(
                    u64::from(b.building) ^ 0x5eed,
                    b.position,
                    column_scale(b.width),
                    now - b.born,
                    STEP_TICKS,
                    1.,
                    &mut frame.puffs,
                );
                for p in &frame.puffs[from..] {
                    frame.tallest = frame.tallest.max(p.position[1] - b.position[1]);
                }
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
        // Past the sprites the nearest of the rest glow (nr-fx3).
        assert_eq!(frame.glows.len(), MAX_FAR_GLOWS);
        // Farther than a fire's reach no sprite or smoke is drawn, only the
        // glow points.
        let far = fires.frame([0., 500., 2_000_000.], 100_000.);
        assert!(far.fires.is_empty() && far.puffs.is_empty());
        assert_eq!(far.glows.len(), MAX_FAR_GLOWS);
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

    /// nr-fx4: in a replay a column's height depends on the playhead and the
    /// tick each building fell on alone, not on how the playhead got there:
    /// playing through, jumping ahead, jumping back and a fresh viewer all
    /// draw the same smoke.
    #[test]
    fn a_replay_seek_shows_the_columns_as_tall_as_playing_through() {
        let fill = fill(10);
        let eye = [0., 100., 0.];
        // Buildings 3 and 4 fell at tick 1,000, building 7 at tick 9,000.
        let early = |ids: &[(u32, f64)]| Falls::new(ids.iter().copied().collect());
        let at_5000 = early(&[(3, 1_000.), (4, 1_000.)]);
        let at_12000 = early(&[(3, 1_000.), (4, 1_000.), (7, 9_000.)]);
        let draw = |fires: &mut CityFires, tick: f64, set: &[u32], falls: &Falls| {
            fill.set_flattened(set);
            fires.observe_with(&fill, tick, Some(falls));
            fires.frame(eye, tick)
        };
        // Played through: a frame every 400 ticks, the third building falling
        // on the way.
        let mut played = CityFires::new();
        let mut through = None;
        for tick in (0..=12_000).step_by(400) {
            let (set, falls): (&[u32], &Falls) = match tick {
                0..=999 => (&[], &at_5000),
                1_000..=8_999 => (&[3, 4], &at_5000),
                _ => (&[3, 4, 7], &at_12000),
            };
            let tick = tick as f64;
            through = Some(draw(&mut played, tick, set, falls));
        }
        let through = through.expect("frames drawn");
        // A viewer that opens at tick 12,000, one that jumps there from
        // tick 5,000 and one that jumps back from the end and forward again
        // all agree.
        let mut fresh = CityFires::new();
        let fresh = draw(&mut fresh, 12_000., &[3, 4, 7], &at_12000);
        let mut jumped = CityFires::new();
        draw(&mut jumped, 5_000., &[3, 4], &at_5000);
        let jumped = draw(&mut jumped, 12_000., &[3, 4, 7], &at_12000);
        let mut back = CityFires::new();
        draw(&mut back, 12_000., &[3, 4, 7], &at_12000);
        draw(&mut back, 2_000., &[3, 4], &at_5000);
        let back = draw(&mut back, 12_000., &[3, 4, 7], &at_12000);
        assert_eq!(through, fresh);
        assert_eq!(through, jumped);
        assert_eq!(through, back);
        // Buildings 3 and 4 have stood 11,000 ticks: a full column of 37
        // puffs (their ages, 56 ticks and every 96 after, stop below
        // 3,600). Building 7 has stood 3,000: puffs at ages 3,000 down to 24.
        assert_eq!(through.puffs.len(), 2 * 37 + 32);
        // Going back before the third fell takes its column away at once,
        // and the others are as tall as they were.
        let mut before = CityFires::new();
        let before = draw(&mut before, 5_000., &[3, 4], &at_5000);
        assert_eq!(before.fires.len(), 2);
        assert_eq!(before.puffs.len(), 2 * 37);
    }

    /// nr-fx3: the recorder writes each knocked-down building by where it
    /// stands, only what changed, and a stand-again after a restart.
    #[test]
    fn the_recorder_writes_knocked_down_buildings_by_where_they_stand() {
        use crate::replay::recorder::city_change;
        let fill = fill(300);
        let mut last = None;
        let mut frame = tore_replay::Frame::default();
        city_change(&fill, &mut last, &mut frame);
        assert!(frame.city_fell.is_empty() && !frame.city_stood);
        fill.knock_down([5, 101]);
        let mut frame = tore_replay::Frame::default();
        city_change(&fill, &mut last, &mut frame);
        assert_eq!(frame.city_fell, [[500, 0], [100, 100]]);
        // Unchanged: nothing.
        let mut frame = tore_replay::Frame::default();
        city_change(&fill, &mut last, &mut frame);
        assert!(frame.city_fell.is_empty());
        fill.knock_down([7]);
        let mut frame = tore_replay::Frame::default();
        city_change(&fill, &mut last, &mut frame);
        assert_eq!(
            (frame.city_fell.as_slice(), frame.city_stood),
            (&[[700, 0]][..], false)
        );
        // A restart: the city stands again, then one falls.
        fill.reset();
        fill.knock_down([9]);
        let mut frame = tore_replay::Frame::default();
        city_change(&fill, &mut last, &mut frame);
        assert_eq!(
            (frame.city_fell.as_slice(), frame.city_stood),
            (&[[900, 0]][..], true)
        );
    }

    /// nr-fx3: a replay rebuilds the fill and finds each recorded building
    /// by where it stands, even when the rebuilt fill numbers its buildings
    /// differently (another build, other import resources), and its fires
    /// burn there; playing back or seeking earlier takes them away again.
    #[test]
    fn a_replay_finds_its_knocked_down_buildings_by_place_and_they_burn() {
        use crate::replay::viewer::city_index;
        let flight = fill(300);
        let mut last = None;
        let mut track = tore_replay::CityTrack::default();
        for (tick, ids) in [(100, vec![5_u32, 101]), (200, vec![250])] {
            flight.knock_down(ids);
            let mut frame = tore_replay::Frame::default();
            crate::replay::recorder::city_change(&flight, &mut last, &mut frame);
            track
                .changes
                .push((tick, frame.city_stood, frame.city_fell));
        }
        // The same fill: the same buildings, at any tick.
        let same = fill(300);
        let index = city_index(&same, &track.all());
        let at = |tick| -> Vec<u32> {
            let mut ids: Vec<u32> = track
                .flattened_at(tick)
                .iter()
                .filter_map(|p| index.get(p).copied())
                .collect();
            ids.sort_unstable();
            ids
        };
        assert!(at(99).is_empty());
        assert_eq!(at(150), [5, 101]);
        assert_eq!(at(250), [5, 101, 250]);
        // A different fill: the same places hold other indices.
        let moved = {
            let base = fill(400);
            let layout = std::sync::Arc::new(tore_world::city::Layout {
                instances: base.layout.instances[100..].to_vec(),
                layout: base.layout.layout.clone(),
                map: base.layout.map.clone(),
                kit: base.layout.kit.clone(),
                cities: Vec::new(),
                candidates: 300,
                inputs: Vec::new(),
            });
            CityFill::assemble(layout, vec![5.; 300], vec![false; 300])
        };
        let index = city_index(&moved, &track.all());
        let mut ids: Vec<u32> = track
            .flattened_at(250)
            .iter()
            .filter_map(|p| index.get(p).copied())
            .collect();
        ids.sort_unstable();
        // [500, 0] is not in the moved fill (it starts at index 100 of the
        // original); [100, 100] is its index 1 and [5000, 200] its 150.
        assert_eq!(ids, [1, 150]);
        moved.set_flattened(&ids);
        let mut fires = CityFires::new();
        fires.observe(&moved, 0.);
        let burning: Vec<_> = fires
            .blazes()
            .iter()
            .map(|b| (b.position[0], b.position[2]))
            .collect();
        assert_eq!(burning, [(100., 100.), (5_000., 200.)]);
        // Seeking back before the strike: the city stands whole again.
        moved.set_flattened(&[]);
        fires.observe(&moved, 10.);
        assert!(fires.blazes().is_empty());
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
