//! Cluster bomb bomblet flashes (docs/spec/ordnance-presentation.md,
//! "Cluster bombs"; A2G plan decision 13, opinionated, agent).
//!
//! The retail game draws a cluster burst as one small flash, and in a
//! one-machine session about ten bursts in eleven as nothing at all (the
//! 0x800000 flag, docs/formats/weapons.md). The remake keeps the wide splash
//! damage and draws a scatter of small ground flashes over it, so a cluster
//! bomb looks like one. Presentation only: the flashes are derived here from
//! the burst's effect alone, so nothing in the simulation, a checkpoint, the
//! wire or a replay changes, and every client draws the same pattern.
use crate::snapshot::EffectPose;
use tore_sim::combat::{blast, live::EffectKind};

/// Flashes in one burst (fitted).
pub const BOMBLETS: usize = 28;
/// How far from the burst a flash can stand, in feet (fitted). The smallest
/// cluster splash is 1,500 ft (RBK-250), so every flash lies inside the
/// splash of every cluster record.
pub const SPREAD_FT: f64 = 1_200.;
const _: () = assert!(SPREAD_FT < 1_500.);
/// How long one flash plays, in 120 Hz ticks (fitted). Flashes start within
/// the first half of the burst's second, so the last one ends with it.
pub const FLASH_TICKS: u16 = 60;
/// The small explosion each flash draws: type 18 (`AIRSML`), the burst a
/// cluster record names, or its variants 19 and 20 (`AIRSMLA`, `AIRSMLB2`),
/// standing on the ground. The orange flashes read as bomblets; the ground
/// sheets (types 15 and 16) draw dust.
pub const FLASH_TYPES: [u8; 3] = [18, 19, 20];
/// A flash is this share of its explosion type's rolled width (fitted).
pub const FLASH_SCALE: f64 = 1.5;
/// The burst's own life, in 120 Hz ticks. Every type in the small-air family
/// the burst can be (18 to 20, 24 to 26, 28, 29) plays one second
/// ([`blast::explosion`]), so the fitted dispense belongs to the same second.
pub const LIFE_TICKS: u16 = 120;
/// Ticks the fitted dispenser takes to open and shed its two halves, from the
/// burst (fitted).
pub const DISPENSER_TICKS: u16 = 16;
/// How high the fitted dispenser stands above the burst when it opens, feet
/// (fitted): inside the 1,200 ft flash pattern and above the type 18 fuze.
pub const DISPENSE_FT: f64 = 140.;
/// The fitted dispenser casing drawn splitting open, length and diameter in
/// feet (fitted).
pub const DISPENSER_FT: [f64; 2] = [6.5, 1.4];
/// The fitted bomblet drawn falling, length and diameter in feet (fitted).
pub const BOMBLET_FT: [f64; 2] = [1.1, 0.4];
/// How long the fitted bomblet is drawn falling before its flash, in ticks
/// (fitted): a bomblet whose flash starts sooner falls for its own shorter
/// time instead.
pub const BOMBLET_TICKS: u16 = 30;
/// How far the dispenser's two halves are shed sideways as it opens, feet
/// (fitted).
pub const SHED_FT: f64 = 9.;

/// One bomblet flash.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flash {
    pub position: [f64; 3],
    /// Ticks after the burst that the flash starts, 0 to [`FLASH_TICKS`].
    pub start: u16,
    /// The small ground explosion it draws, one of [`FLASH_TYPES`].
    pub kind: u8,
    /// Drawn width in feet.
    pub width: f64,
}

/// One body of the fitted dispense presentation at one instant: a dispenser
/// half opening above the burst, or a bomblet falling to its flash. The
/// renderer draws it as a small solid body
/// (docs/spec/ordnance-presentation.md, "Cluster bomb dispense").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub position: [f64; 3],
    /// Unit direction the body points, nose first: it falls nose down.
    pub direction: [f64; 3],
    /// Body length and diameter, feet.
    pub body: [f64; 2],
    /// Which fitted body it is.
    pub part: Part,
}

/// Which fitted body a [`Piece`] is: the dispenser casing or a falling
/// bomblet. The renderer draws the casing lighter than a bomblet (fitted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Dispenser,
    Bomblet,
}

/// The explosion types a cluster burst can show: type 18, which every
/// cluster record names for land and water, and the variants the original
/// rolls for it ([`blast::vary`]).
fn small_air(kind: u8) -> bool {
    matches!(kind, 18..=20 | 24..=26 | 28 | 29)
}

/// Whether `effect` is a cluster bomb going off on the ground or the sea.
/// The five cluster records are the only weapons whose land explosion is
/// type 18 (guns name 15, bombs and missiles 21 and 35), and only an impact
/// on the ground makes a `Ground` effect, so the burst identifies itself.
/// A cluster bomb that strikes an object directly makes a `Hit`, which a
/// gun burst on an aircraft also makes; it draws no flashes.
pub fn is_burst(effect: &EffectPose) -> bool {
    effect.kind == EffectKind::Ground && effect.blast.is_some_and(small_air)
}

/// The flashes of `effect`: none unless it is a cluster burst, otherwise
/// [`BOMBLETS`] flashes scattered evenly over a disc of [`SPREAD_FT`] around
/// the burst, each at a repeatable place, time and size taken from where the
/// burst is.
pub fn derive(effect: &EffectPose) -> Vec<Flash> {
    if !is_burst(effect) {
        return Vec::new();
    }
    let at = effect.position;
    (0..BOMBLETS as u64)
        .map(|n| {
            let salt = 100 + n * 4;
            let angle = f64::from(blast::pick(at, salt, 3600)) / 10.;
            // The square root spreads the flashes evenly over the disc.
            let share = (f64::from(blast::pick(at, salt + 1, 10_000)) + 0.5) / 10_000.;
            let reach = SPREAD_FT * share.sqrt();
            let position = [
                at[0] + reach * angle.to_radians().sin(),
                at[1],
                at[2] + reach * angle.to_radians().cos(),
            ];
            let kind =
                FLASH_TYPES[usize::from(blast::pick(at, salt + 2, FLASH_TYPES.len() as u16))];
            Flash {
                position,
                start: blast::pick(at, salt + 3, FLASH_TICKS + 1),
                kind,
                width: FLASH_SCALE * f64::from(blast::rolled_size(kind, position)),
            }
        })
        .collect()
}

/// A unit direction, or straight down where the vector is too short to name
/// one.
fn down(v: [f64; 3]) -> [f64; 3] {
    let n = v[0].hypot(v[1]).hypot(v[2]);
    if n < 1e-9 {
        [0., -1., 0.]
    } else {
        [v[0] / n, v[1] / n, v[2] / n]
    }
}

/// The fitted dispenser halves and falling bomblets of `effect` right now.
/// No geometry unless it is a cluster burst. The retail cluster records
/// and shapes carry no dispenser part, submunition shape, count or timing
/// (docs/formats/weapons.md, "Cluster flag 0x800000"), so this whole
/// presentation is fitted; it is derived from the burst's own place alone, so
/// the same burst always shows the same bodies on every client. Presentation
/// only: no damage, simulation, checkpoint, wire or replay state changes.
pub fn pieces(effect: &EffectPose) -> Vec<Piece> {
    if !is_burst(effect) {
        return Vec::new();
    }
    let at = effect.position;
    let elapsed = LIFE_TICKS.saturating_sub(effect.ticks);
    let mut out = Vec::new();
    // The dispenser stands above the burst and splits: its two halves are
    // shed evenly, falling and tumbling apart. Its orientation is repeatable
    // from the burst's place, like the pattern.
    let yaw = f64::from(blast::pick(at, 900, 3600)).to_radians();
    let right = [yaw.sin(), 0., yaw.cos()];
    if elapsed < DISPENSER_TICKS {
        let u = f64::from(elapsed) / f64::from(DISPENSER_TICKS);
        for side in [-1., 1.] {
            let shed = side * SHED_FT * u;
            out.push(Piece {
                position: [
                    at[0] + right[0] * shed,
                    at[1] + DISPENSE_FT * (1. - u * u),
                    at[2] + right[2] * shed,
                ],
                direction: down([right[0] * shed, -SHED_FT, right[2] * shed]),
                body: DISPENSER_FT,
                part: Part::Dispenser,
            });
        }
    }
    // Each bomblet falls from the dispenser to the exact place its flash
    // will play, arriving as the flash starts. It is drawn nose down: the
    // fan is wide and the drop short, so the true velocity would read
    // sideways (fitted).
    for flash in derive(effect) {
        let flight = BOMBLET_TICKS.min(flash.start);
        let start = flash.start - flight;
        if flash.start == 0 || elapsed < start || elapsed >= flash.start {
            continue;
        }
        let u = f64::from(elapsed - start) / f64::from(flight);
        out.push(Piece {
            position: [
                at[0] + (flash.position[0] - at[0]) * u,
                at[1] + DISPENSE_FT * (1. - u * u),
                at[2] + (flash.position[2] - at[2]) * u,
            ],
            direction: [0., -1., 0.],
            body: BOMBLET_FT,
            part: Part::Bomblet,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(kind: EffectKind, blast: Option<u8>) -> EffectPose {
        EffectPose {
            kind,
            position: [1_000_000., 40., 790_000.],
            ticks: 120,
            blast,
            scale: tore_sim::combat::blast_scale::FULL,
        }
    }

    #[test]
    fn only_a_ground_impact_of_the_small_air_family_is_a_cluster_burst() {
        for kind in [18, 19, 20, 24, 25, 26, 28, 29] {
            assert!(is_burst(&effect(EffectKind::Ground, Some(kind))), "{kind}");
        }
        // Guns and rockets on land, bombs, missiles, flak and water.
        for kind in [15, 16, 17, 21, 22, 23, 27, 30, 34, 35, 36, 37, 38] {
            assert!(!is_burst(&effect(EffectKind::Ground, Some(kind))), "{kind}");
        }
        // A gun hit on an aircraft is a type 18 too, and is not one.
        for other in [EffectKind::Hit, EffectKind::Destroyed, EffectKind::Flak] {
            assert!(!is_burst(&effect(other, Some(18))), "{other:?}");
        }
        assert!(!is_burst(&effect(EffectKind::Ground, None)));
        assert!(derive(&effect(EffectKind::Hit, Some(18))).is_empty());
    }

    #[test]
    fn the_flashes_are_repeatable_and_belong_to_the_burst() {
        let burst = effect(EffectKind::Ground, Some(20));
        let flashes = derive(&burst);
        assert_eq!(flashes.len(), BOMBLETS);
        assert_eq!(flashes, derive(&burst), "the same burst, the same pattern");
        // The pattern does not depend on the life left or the variety rolled.
        let later = EffectPose {
            ticks: 30,
            blast: Some(28),
            ..burst
        };
        assert_eq!(flashes, derive(&later));
        // Another burst has another pattern.
        let elsewhere = EffectPose {
            position: [1_000_100., 40., 790_000.],
            ..burst
        };
        assert_ne!(flashes[0].position, derive(&elsewhere)[0].position);
    }

    #[test]
    fn the_flashes_stay_inside_every_cluster_splash_and_end_with_the_burst() {
        // Different places, so different patterns, all inside the bounds.
        for east in 0..40 {
            let burst = EffectPose {
                position: [900_000. + f64::from(east) * 137.5, 12., 700_000.],
                ..effect(EffectKind::Ground, Some(18))
            };
            for flash in derive(&burst) {
                let gap = (flash.position[0] - burst.position[0])
                    .hypot(flash.position[2] - burst.position[2]);
                assert!(gap <= SPREAD_FT, "{gap}");
                assert_eq!(flash.position[1], burst.position[1]);
                assert!(flash.start + FLASH_TICKS <= 120, "ends with the burst");
                assert!(FLASH_TYPES.contains(&flash.kind));
                assert!((38. ..=120.).contains(&flash.width), "{}", flash.width);
            }
        }
    }

    #[test]
    fn the_flashes_spread_over_the_disc_and_over_the_second() {
        let burst = effect(EffectKind::Ground, Some(18));
        let flashes = derive(&burst);
        let reach = |f: &Flash| {
            (f.position[0] - burst.position[0]).hypot(f.position[2] - burst.position[2])
        };
        assert!(flashes.iter().any(|f| reach(f) < SPREAD_FT / 2.));
        assert!(flashes.iter().any(|f| reach(f) > SPREAD_FT * 0.8));
        assert!(flashes.iter().any(|f| f.start < 15));
        assert!(flashes.iter().any(|f| f.start > 45));
        for kind in FLASH_TYPES {
            assert!(flashes.iter().any(|f| f.kind == kind), "{kind}");
        }
    }

    /// The effect `elapsed` ticks into its life (the burst's ticks count
    /// down).
    fn at(effect: EffectPose, elapsed: u16) -> EffectPose {
        EffectPose {
            ticks: LIFE_TICKS - elapsed,
            ..effect
        }
    }

    #[test]
    fn only_a_cluster_burst_shows_a_dispenser_and_bomblets() {
        let burst = effect(EffectKind::Ground, Some(18));
        let halves = pieces(&burst)
            .into_iter()
            .filter(|p| p.part == Part::Dispenser)
            .count();
        assert_eq!(halves, 2, "the dispenser opens as two halves");
        // A gun burst on an aircraft, a ground hit and a plain ground
        // explosion show none.
        assert!(pieces(&effect(EffectKind::Hit, Some(18))).is_empty());
        assert!(pieces(&effect(EffectKind::Destroyed, Some(18))).is_empty());
        assert!(pieces(&effect(EffectKind::Ground, Some(15))).is_empty());
        assert!(pieces(&effect(EffectKind::Ground, None)).is_empty());
    }

    #[test]
    fn the_dispenser_stands_above_the_burst_and_is_gone_after_a_beat() {
        let burst = effect(EffectKind::Ground, Some(18));
        // When it opens, the casing is whole at the dispense height.
        let closed: Vec<Piece> = pieces(&burst)
            .into_iter()
            .filter(|p| p.part == Part::Dispenser)
            .collect();
        for half in &closed {
            assert!((half.position[1] - (burst.position[1] + DISPENSE_FT)).abs() < 1e-9);
            assert_eq!(half.body, DISPENSER_FT);
            assert!(half.direction[1] < 0., "the casing falls");
        }
        // A beat later its two halves have split apart and dropped.
        let opening: Vec<Piece> = pieces(&at(burst, DISPENSER_TICKS - 1))
            .into_iter()
            .filter(|p| p.part == Part::Dispenser)
            .collect();
        assert_eq!(opening.len(), 2);
        assert_ne!(opening[0].position, opening[1].position, "the halves split");
        for half in &opening {
            assert!(half.position[1] < burst.position[1] + DISPENSE_FT);
        }
        // By the end of the opening the casing is shed and no longer drawn.
        for elapsed in DISPENSER_TICKS..LIFE_TICKS {
            assert!(
                pieces(&at(burst, elapsed))
                    .iter()
                    .all(|p| p.part != Part::Dispenser),
                "{elapsed}"
            );
        }
    }

    #[test]
    fn the_bomblets_fall_from_the_dispenser_inside_the_pattern() {
        let burst = effect(EffectKind::Ground, Some(20));
        // The pattern is repeatable, so the same burst draws the same bodies.
        assert_eq!(pieces(&burst), pieces(&burst));
        let mut seen = 0;
        for elapsed in 0..LIFE_TICKS {
            let pose = at(burst, elapsed);
            for piece in pieces(&pose)
                .into_iter()
                .filter(|p| p.part == Part::Bomblet)
            {
                seen += 1;
                assert_eq!(piece.body, BOMBLET_FT);
                assert!(piece.direction[1] < 0., "a bomblet falls");
                // It hangs between the dispense height and the ground.
                assert!(piece.position[1] <= burst.position[1] + DISPENSE_FT + 1e-9);
                assert!(piece.position[1] >= burst.position[1] - 1e-9);
                let gap = (piece.position[0] - burst.position[0])
                    .hypot(piece.position[2] - burst.position[2]);
                assert!(gap <= SPREAD_FT + 1e-9, "reach {gap}");
            }
        }
        assert!(seen > 0, "the bomblets are drawn in flight");
    }

    #[test]
    fn a_bomblet_lands_on_the_place_of_its_flash() {
        let burst = effect(EffectKind::Ground, Some(18));
        // A flash that gives the bomblet its full flight: by the last tick
        // before it plays, the bomblet is at the flash's own place.
        for flash in derive(&burst)
            .into_iter()
            .filter(|f| f.start >= BOMBLET_TICKS)
        {
            let near = pieces(&at(burst, flash.start - 1))
                .into_iter()
                .filter(|p| p.part == Part::Bomblet)
                .filter(|p| {
                    (p.position[0] - flash.position[0]).hypot(p.position[2] - flash.position[2])
                        < DISPENSE_FT
                })
                .count();
            assert!(near >= 1, "no bomblet reaches the flash at {}", flash.start);
        }
    }

    /// Checks the retail numbers the module depends on against an import:
    /// run by hand with `TORE_DATA_DIR` set.
    #[test]
    #[ignore]
    fn the_cluster_records_are_the_only_ones_that_name_type_18_for_land() {
        let assets = crate::reel::load_assets().expect("an import");
        let mut clusters = Vec::new();
        for (name, bytes) in assets
            .theater_resources
            .iter()
            .filter(|(n, _)| n.ends_with(".JT"))
        {
            let Ok(w) = tore_formats::weapons::Weapon::parse(name, bytes) else {
                continue;
            };
            let land = w.effects.land_explosion;
            assert_eq!(
                small_air(land),
                w.flags & 0x80_0000 != 0,
                "{name}: land {land} flags {:#x}",
                w.flags
            );
            if small_air(land) {
                clusters.push(name.clone());
            }
        }
        clusters.sort();
        assert_eq!(
            clusters,
            ["CBU87.JT", "CBU89.JT", "MK20.JT", "RBK250.JT", "RBK500.JT"]
        );
    }
}
