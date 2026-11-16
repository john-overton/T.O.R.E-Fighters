//! Swing wings as the simulation sees them: the Tornado's (TOR-5, 2026-11-10)
//! and, as data, the other swing-wing aircraft's (SWING-1, 2026-11-15).
//!
//! The wing's sweep is a pure function of the aircraft's speed and flaps (and,
//! for some aircraft, its gear), so every peer works out the same turn. The
//! drawn rig (`tore-app` `tornado_animation`) turns the wing panels by it, and
//! the stations on the swinging wing ride the same turn here: a store on a
//! swivelling wing pylon hangs from the point the swept wing carries it to, and
//! leaves from there. The pylon swivels back by the wing's own turn (John,
//! 2026-11-10, decision 19: the wing pylons swivel to stay aligned with the
//! airflow as the wing sweeps), so the store itself never changes heading.
//!
//! Provenance: the schedule, the pivot and the choice of the swivelling row are
//! fitted or opinionated (agent decisions 2026-11-10, John's decision 19 for
//! the rotating pylons); the real sweep range 25 to 67 degrees is cited from
//! `TOR.INF` (Jane's). Update (TOR-11, 2026-11-12): the wing starts to sweep at
//! 350 mph, 304 knots (opinionated, John 2026-11-12; was 250 knots, fitted).
//! Update (TOR-12, 2026-11-12): the wing is placed 1.54 ft in, 0.1167 ft up and
//! 0.20 ft forward of the retail panel, mirrored left to right, with new pivots
//! ([`PIVOT_R`], [`WING_IN_FT`]), so the stations that ride on it move with it.
//! Update (TOR-14, 2026-11-13): every swivelling pylon is centred on the wing
//! panel's mid-chord at its span ([`mid_chord`]) and has one fixed length
//! ([`PYLON_LENGTH_FT`]) and depth, whatever the sweep.
//! Update (SWING-1, 2026-11-15): everything above that was a module constant
//! is a field of the aircraft's [`SwingSpec`] ([`spec`]); the Tornado's is
//! [`TORNADO`], with every value unchanged, and the module constants and the
//! Tornado-flavoured free functions ([`wing_turn`], [`swept`], [`pivot_feet`],
//! [`mid_chord`] and the rest) read it. The B-1B, Tu-160, F-111F and Tu-26
//! have specs ([`B1B`], [`TU160`], [`F111F`], [`TU26`]) but no
//! [`AircraftId`] yet, so [`spec`] does not return them: wiring one is one
//! match arm there (batch B's identity slices).
use tore_formats::aircraft::AircraftId;

/// Shape units to feet at the Tornado's header scale 2 (2/3 ft a unit).
pub const FEET_PER_UNIT: f64 = 2. / 3.;
/// Feet per second to knots.
pub const FPS_PER_KNOT: f64 = 1.68781;

/// How the commanded sweep follows the airspeed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SweepLaw {
    /// Spread up to `from_knots`, fully swept at `to_knots`, linear between.
    Linear { from_knots: f64, to_knots: f64 },
    /// Detents: `(knots, degrees)` points, ascending in both, joined by
    /// smoothstep curves. The sweep holds the first point's degrees below its
    /// speed and the last point's above, and holds between two points of the
    /// same degrees (the Tu-160's three positions: 20, 35 and 65).
    Detents(&'static [(f64, f64)]),
}

impl SweepLaw {
    /// The speed (knots) at which the wing starts to sweep from spread.
    pub const fn from_knots(&self) -> f64 {
        match self {
            Self::Linear { from_knots, .. } => *from_knots,
            Self::Detents(points) => points[0].0,
        }
    }
    /// The speed (knots) at which the wing is fully swept.
    pub const fn to_knots(&self) -> f64 {
        match self {
            Self::Linear { to_knots, .. } => *to_knots,
            Self::Detents(points) => points[points.len() - 1].0,
        }
    }
}

/// How a swing wing is placed against the retail shape (TOR-12, John
/// 2026-11-12, from his Blender review of the Tornado's wing): the whole right
/// panel moves `in_ft` toward the fuselage, `up_ft` up and `forward_ft`
/// forward, and its root points (the glove vertices) move `root_shift_ft`
/// inboard along the wing's span axis, so the root stays inside the fuselage
/// through the sweep. The left panel is an exact mirror. All zero for an
/// aircraft whose wing stays where the shape draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub in_ft: f64,
    pub up_ft: f64,
    pub forward_ft: f64,
    pub root_shift_ft: f64,
}

impl Placement {
    pub const NONE: Self = Self {
        in_ft: 0.,
        up_ft: 0.,
        forward_ft: 0.,
        root_shift_ft: 0.,
    };
}

/// The placed right wing panel's leading and trailing edges in the spread
/// pose, each as its inboard and outboard corner `[right, forward]` in feet
/// (TOR-14, 2026-11-13). The left panel mirrors them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WingOutline {
    pub leading: [[f64; 2]; 2],
    pub trailing: [[f64; 2]; 2],
}

impl WingOutline {
    /// Where along an edge (two `[right, forward]` corners) the span station
    /// `right` feet out is: its forward coordinate.
    const fn edge_forward(edge: [[f64; 2]; 2], right: f64) -> f64 {
        let share = (right - edge[0][0]) / (edge[1][0] - edge[0][0]);
        edge[0][1] + share * (edge[1][1] - edge[0][1])
    }
    /// The forward coordinate (feet, aircraft frame, wing spread) of the
    /// middle of the placed panel's chord at the span station `right` feet
    /// out of the reference point: halfway between the leading edge and the
    /// trailing edge (the flap strip's trailing edge, flaps up) along the
    /// streamwise line at that span. The chord is the panel's own local
    /// section, so a pylon centred there stays on the wing's middle through
    /// the sweep (TOR-14, 2026-11-13, John: "all pylons need to be centered on
    /// the wing"; the definition of mid-chord is the agent's). Right-hand
    /// wing; the left mirrors it.
    pub const fn mid_chord(&self, right: f64) -> f64 {
        (Self::edge_forward(self.leading, right) + Self::edge_forward(self.trailing, right)) / 2.
    }
}

/// One aircraft's swing wing: every number the sweep, the wing's turn about
/// its pivot and the stations on the wing need (SWING-1, 2026-11-15). The
/// sweep angle is the leading edge's from the lateral axis, as the real
/// aircraft's ranges are quoted; the drawn panel's turn is measured from the
/// pose the shape is drawn in ([`Self::rest_sweep`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwingSpec {
    /// Short name, for messages and probes.
    pub name: &'static str,
    /// Real minimum (spread) and maximum (fully swept) sweep, degrees.
    pub sweep_min: f64,
    pub sweep_max: f64,
    /// The sweep the shape is drawn at, degrees: the rig's zero turn. The
    /// spread minimum for every aircraft but the Tu-26, whose mesh sits at
    /// 18.4 degrees against a real 20 (so its spread wing is already turned
    /// 1.6 degrees from the drawn pose).
    pub rest_sweep: f64,
    /// Feet per shape unit (the header scale) and the per-axis factors
    /// (right, forward, up) the aircraft's size fit multiplies it by; `1.`
    /// when the shape is drawn at its header scale.
    pub feet_per_unit: f64,
    pub axis_scale: [f64; 3],
    /// The right wing's pivot, shape units `[right, forward, up]`, vertical
    /// axis; the left pivot is [`Self::pivot_left`] (the mirror, `-right`).
    pub pivot_right: [f64; 3],
    pub pivot_left: [f64; 3],
    /// Speed to commanded sweep.
    pub law: SweepLaw,
    /// Whether the landing gear, as well as the flaps, holds the wing spread
    /// (the Tu-26: "spread to 300 kt and whenever flaps or gear are out").
    pub gear_holds_spread: bool,
    /// How fast the real wing sweeps, degrees a second, when a source gives
    /// it. None has: the drawn sweep is a pure function of speed, not a state
    /// that moves at a rate.
    pub sweep_rate_dps: Option<f64>,
    /// Where the wing panel is placed against the retail shape.
    pub placement: Placement,
    /// Source hardpoints (rows) that are swivelling wing pylons carried by
    /// the swinging wing.
    pub swivel_rows: &'static [usize],
    /// Source hardpoints on the wing that do not swivel, and the sweep limit
    /// (degrees, leading edge) while any of them carries a store: the
    /// F-111F's outer pylons hold the wing at 26 degrees or less.
    pub fixed_store_rows: &'static [usize],
    pub max_sweep_with_fixed_stores: Option<f64>,
    /// The placed panel's edges, for the mid-chord (aircraft with swivelling
    /// pylons).
    pub outline: Option<WingOutline>,
    /// How long every swivelling wing pylon is drawn, feet.
    pub pylon_length_ft: Option<f64>,
}

impl SwingSpec {
    /// Turn (radians) from the drawn pose to the spread minimum sweep: zero
    /// when the shape is drawn at it.
    pub fn min_turn(&self) -> f64 {
        (self.sweep_min - self.rest_sweep).to_radians()
    }
    /// Turn (radians) from the drawn pose to the fully swept maximum.
    pub fn max_turn(&self) -> f64 {
        (self.sweep_max - self.rest_sweep).to_radians()
    }
    /// The wing's leading-edge sweep for a turn from the drawn pose, degrees.
    pub fn sweep_degrees(&self, turn: f64) -> f64 {
        self.rest_sweep + turn.to_degrees()
    }
    /// How far the flaps (and, where it holds, the gear) hold the wing
    /// spread, 0 (free) to 1 (fully spread).
    pub fn hold(&self, flaps: f64, gear: f64) -> f64 {
        let held = flaps.clamp(0., 1.);
        if self.gear_holds_spread {
            held.max(gear.clamp(0., 1.))
        } else {
            held
        }
    }
    /// The share of the full sweep range (0 spread, 1 fully swept) the speed
    /// schedule commands at `knots`, before the flaps hold it.
    pub fn command_share(&self, knots: f64) -> f64 {
        match self.law {
            SweepLaw::Linear {
                from_knots,
                to_knots,
            } => ((knots - from_knots) / (to_knots - from_knots)).clamp(0., 1.),
            SweepLaw::Detents(points) => ((detent_degrees(points, knots) - self.sweep_min)
                / (self.sweep_max - self.sweep_min))
                .clamp(0., 1.),
        }
    }
    /// How far the wing panel has turned from its drawn pose, radians (zero
    /// at the drawn pose, [`Self::max_turn`] fully swept), for an aircraft
    /// flying at `speed_fps` with its flaps `flaps` and gear `gear` out (0 to
    /// 1): the schedule's sweep, held spread by the flaps (and the gear
    /// where [`Self::gear_holds_spread`]).
    pub fn turn(&self, speed_fps: f64, flaps: f64, gear: f64) -> f64 {
        let knots = speed_fps / FPS_PER_KNOT;
        self.min_turn()
            + self.command_share(knots)
                * (self.sweep_max - self.sweep_min).to_radians()
                * (1. - self.hold(flaps, gear))
    }
    /// [`Self::turn`] held to [`Self::max_sweep_with_fixed_stores`] while
    /// `fixed_stores` (a store on one of [`Self::fixed_store_rows`]) is
    /// carried.
    pub fn turn_carrying(&self, speed_fps: f64, flaps: f64, gear: f64, fixed_stores: bool) -> f64 {
        let turn = self.turn(speed_fps, flaps, gear);
        match self.max_sweep_with_fixed_stores {
            Some(limit) if fixed_stores => turn.min((limit - self.rest_sweep).to_radians()),
            _ => turn,
        }
    }
    /// The speed in feet per second at which the schedule has swept the wing
    /// to `degrees` (flaps up): the inverse of [`Self::turn`], for captures
    /// and measurements that pose the wing. On a flat part of a detent law
    /// it is the speed the sweep first reaches that angle.
    pub fn speed_for_sweep(&self, degrees: f64) -> f64 {
        let share = ((degrees - self.sweep_min) / (self.sweep_max - self.sweep_min)).clamp(0., 1.);
        let knots = match self.law {
            SweepLaw::Linear {
                from_knots,
                to_knots,
            } => from_knots + share * (to_knots - from_knots),
            SweepLaw::Detents(points) => detent_knots(
                points,
                self.sweep_min + share * (self.sweep_max - self.sweep_min),
            ),
        };
        knots * FPS_PER_KNOT
    }
    /// The degrees a knot of speed adds to the sweep on a linear schedule.
    pub fn degrees_per_knot(&self) -> Option<f64> {
        match self.law {
            SweepLaw::Linear {
                from_knots,
                to_knots,
            } => Some((self.sweep_max - self.sweep_min) / (to_knots - from_knots)),
            SweepLaw::Detents(_) => None,
        }
    }
    /// The pivot of one wing, feet right, up and forward (the station frame).
    pub fn pivot_feet(&self, right: bool) -> [f64; 3] {
        let p = if right {
            self.pivot_right
        } else {
            self.pivot_left
        };
        let scale = self.feet_per_unit;
        [
            p[0] * scale * self.axis_scale[0],
            p[2] * scale * self.axis_scale[2],
            p[1] * scale * self.axis_scale[1],
        ]
    }
    /// Where the wing carries a point of its own panel, feet right, up and
    /// forward of the aircraft: `point` is where the point is with the wing
    /// at the drawn pose, `turn` the wing's turn ([`Self::turn`]), the tip
    /// moving aft. The height does not change. `right` names the wing (a
    /// left-hand point is carried by the left wing).
    pub fn swept(&self, right: bool, point: [f64; 3], turn: f64) -> [f64; 3] {
        let pivot = self.pivot_feet(right);
        // Tip aft: clockwise seen from above on the right wing (negative about +z).
        let angle = if right { -turn } else { turn };
        let (dx, dz) = (point[0] - pivot[0], point[2] - pivot[2]);
        let (s, c) = angle.sin_cos();
        [
            pivot[0] + dx * c - dz * s,
            point[1],
            pivot[2] + dx * s + dz * c,
        ]
    }
    /// [`Self::swept`] undone: where a point at `point` now was at the drawn
    /// pose.
    pub fn unswept(&self, right: bool, point: [f64; 3], turn: f64) -> [f64; 3] {
        self.swept(right, point, -turn)
    }
    /// Whether source hardpoint `hardpoint` is a swivelling wing pylon.
    pub fn swivels(&self, hardpoint: usize) -> bool {
        self.swivel_rows.contains(&hardpoint)
    }
    /// Whether any station swivels.
    pub fn has_swivel(&self) -> bool {
        !self.swivel_rows.is_empty()
    }
    /// The fixed length of the swivelling pylon of source hardpoint
    /// `hardpoint`, when it has one.
    pub fn pylon_length(&self, hardpoint: usize) -> Option<f64> {
        self.swivels(hardpoint)
            .then_some(self.pylon_length_ft)
            .flatten()
    }
    /// The forward coordinate of the middle of the placed panel's chord at
    /// the span station `right` feet out ([`WingOutline::mid_chord`]); `None`
    /// for an aircraft with no outline.
    pub fn mid_chord(&self, right: f64) -> Option<f64> {
        self.outline.map(|outline| outline.mid_chord(right))
    }
}

/// The sweep degrees a detent law commands at `knots`.
fn detent_degrees(points: &[(f64, f64)], knots: f64) -> f64 {
    let first = points[0];
    let last = points[points.len() - 1];
    if knots <= first.0 {
        return first.1;
    }
    if knots >= last.0 {
        return last.1;
    }
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if knots <= b.0 {
            let t = ((knots - a.0) / (b.0 - a.0)).clamp(0., 1.);
            return a.1 + t * t * (3. - 2. * t) * (b.1 - a.1);
        }
    }
    last.1
}

/// The lowest speed, knots, at which a detent law commands `degrees`.
fn detent_knots(points: &[(f64, f64)], degrees: f64) -> f64 {
    let first = points[0];
    if degrees <= first.1 {
        return first.0;
    }
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if b.1 > a.1 && degrees <= b.1 {
            let want = (degrees - a.1) / (b.1 - a.1);
            // Smoothstep rises monotonically on 0 to 1: bisect it.
            let (mut low, mut high) = (0_f64, 1_f64);
            for _ in 0..80 {
                let t = (low + high) / 2.;
                if t * t * (3. - 2. * t) < want {
                    low = t;
                } else {
                    high = t;
                }
            }
            return a.0 + (low + high) / 2. * (b.0 - a.0);
        }
    }
    points[points.len() - 1].0
}

/// The Tornado's swing wing. Real minimum and maximum sweep 25 and 67 degrees
/// (`TOR.INF`, Jane's); the shape's spread pose is the minimum.
///
/// Speed schedule, knots: the wing stays spread up to the first speed and is
/// fully swept at the second. Flaps hold the wing out. The end points are
/// fitted (agent decision 2026-11-10, after the MiG-23's rig); the start is
/// John's (2026-11-12: "I don't think for the tornado the wings should start
/// sweeping back until it's about 350 mph", opinionated): 350 mph is 304
/// knots, where it was a fitted 250. The full sweep stays 600 knots, so the
/// sweep now runs 304 to 600 knots, linear (about 0.14 degrees a knot, from
/// 0.12).
///
/// The wing sweep pivots, shape units (right, forward, up), vertical axis.
/// Update (TOR-12, John 2026-11-12, from his Blender review of the wing): the
/// right pivot is (4.3333, -0.9007, 0.31) ft right, forward and up, that is
/// (6.5, -1.351025, 0.465) shape units, and the left pivot mirrors it. The old
/// pivots (9.5, -1.5) right and (-10.5, -1.5) left were the centre of the
/// smallest circle round the fixed glove corners the wing panel shared (fitted,
/// TOR-3); the glove fillets are gone, so that reason went with them. (The
/// pivots are the `f32` values of those numbers, widened.)
///
/// How the retail right wing panel is placed in the spread pose (TOR-12, John
/// 2026-11-12, from his Blender review): the whole panel moves 1.54 ft in
/// (toward the fuselage), 0.1167 ft up and 0.20 ft forward, and its root points
/// (the five vertices on the glove, x 9 and 10 units) move 2.3483 ft inboard
/// along the wing's span axis, so the root stays inside the fuselage through
/// the sweep. The left panel is an exact mirror of the right one: the retail
/// left panel sat one unit further out, and the placement drops that
/// asymmetry. Opinionated (John's measured values, not a source's).
///
/// The outline is the panel's outline corners (retail's shape units, 2/3 ft
/// each: leading edge (9, 7) to (31, -2), trailing edge, the flap strip's outer
/// edge, (10, -4) to (30, -9)) with John's placement applied, exactly as
/// `tornado_animation::wing_point` places them; a test there pins the two
/// against each other (TOR-14, 2026-11-13).
///
/// How long every swivelling wing pylon is drawn, feet, whatever the sweep or
/// the store (TOR-14, 2026-11-13; fitted, agent decision): 3.3 ft, the length
/// the Mk 82 pack's pylon had (0.45 of the 7.3 ft bomb), which is a little over
/// half the 6.4 ft panel chord at the inner pylon and 60 percent of the 5.46 ft
/// chord at the outer pods, so the pylon stays inside the panel's outline with
/// the wing spread, and its aft end stays within 0.4 ft of the flap strip's
/// hinge line (the strip droops onto at most 0.17 ft of it with the flaps
/// down). It was a share of the store's length (0.45 of it, at least twice its
/// depth) trimmed to wherever the wing's surface was under it, which was 5.9 ft
/// for the 13 ft pod and grew with the spread wing.
///
/// Which rows swivel (opinionated, John 2026-11-10, decision 19). The Tornado
/// IDS's inner wing pair (row 7) swivels; the centreline tank pair (row 5) and
/// the shoulder pylons (row 6) are on the fuselage and stay fixed. Row 8, the
/// outer wing pair, was retired. Update (TOR-6b, 2026-11-11): row 8 is now the
/// AIM-9 side rail on the row 7 pylon, which swivels with it. Update (TOR-13,
/// 2026-11-12): rows 9 and 10 are the Tornado IDS (Combat)'s outer wing pylons
/// with the BOZ-107 and the Sky Shadow
/// ([`tore_formats::aircraft::stations::authored_pods`]); they are not source
/// rows, only station indices past them, and they swivel with the wing like
/// rows 7 and 8. Both Tornado versions share rows 7 to 10 ([`AircraftId::source`]).
pub const TORNADO: SwingSpec = SwingSpec {
    name: "Tornado",
    sweep_min: 25.,
    sweep_max: 67.,
    rest_sweep: 25.,
    feet_per_unit: FEET_PER_UNIT,
    axis_scale: [1., 1., 1.],
    pivot_right: [6.5_f32 as f64, -1.351_025_f32 as f64, 0.465_f32 as f64],
    pivot_left: [-6.5_f32 as f64, -1.351_025_f32 as f64, 0.465_f32 as f64],
    law: SweepLaw::Linear {
        from_knots: 304.,
        to_knots: 600.,
    },
    gear_holds_spread: false,
    sweep_rate_dps: None,
    placement: Placement {
        in_ft: 1.54,
        up_ft: 0.175 * FEET_PER_UNIT,
        forward_ft: 0.2,
        root_shift_ft: 2.3483,
    },
    swivel_rows: &[7, 8, 9, 10],
    fixed_store_rows: &[],
    max_sweep_with_fixed_stores: None,
    outline: Some(WingOutline {
        leading: [
            [9. * FEET_PER_UNIT - 2.3483 - 1.54, 7. * FEET_PER_UNIT + 0.2],
            [31. * FEET_PER_UNIT - 1.54, -2. * FEET_PER_UNIT + 0.2],
        ],
        trailing: [
            [
                10. * FEET_PER_UNIT - 2.3483 - 1.54,
                -4. * FEET_PER_UNIT + 0.2,
            ],
            [30. * FEET_PER_UNIT - 1.54, -9. * FEET_PER_UNIT + 0.2],
        ],
    }),
    pylon_length_ft: Some(3.3),
};

/// The B-1B Lancer's swing wing (SWING-1, from the coordinator's decision B2,
/// 2026-11-15). Sweep 15 to 67.5 degrees (`B1.INF`: "15 degrees of
/// leading-edge sweep when fully forward and 67 degrees 30 minutes when fully
/// swept", cited), a whole-panel turn of 52.5 degrees about the shared vertex
/// line (10, 11) of the shape (measured: 13.33 ft out, 14.67 ft forward at the
/// header's 4/3 ft a unit; no fold, no seam). The speed schedule is fitted, the
/// real one being the crew's: spread up to 280 knots (a flaps-out approach and
/// the take-off roll stay at 15 degrees) and fully swept by 520 knots (the real
/// low-level penetration speed, over 521 knots), linear; flaps hold the wing
/// spread. No wing stations, so nothing swivels. The shape is drawn at the
/// spread minimum (its leading edge measures 12.7 degrees; the real 15 is what
/// the turn counts from).
pub const B1B: SwingSpec = SwingSpec {
    name: "B-1B",
    sweep_min: 15.,
    sweep_max: 67.5,
    rest_sweep: 15.,
    feet_per_unit: 4. / 3.,
    axis_scale: [1., 1., 1.],
    pivot_right: [10., 11., 0.],
    pivot_left: [-10., 11., 0.],
    law: SweepLaw::Linear {
        from_knots: 280.,
        to_knots: 520.,
    },
    gear_holds_spread: false,
    sweep_rate_dps: None,
    placement: Placement::NONE,
    swivel_rows: &[],
    fixed_store_rows: &[],
    max_sweep_with_fixed_stores: None,
    outline: None,
    pylon_length_ft: None,
};

/// The Tu-160 Blackjack's swing wing (SWING-1, from decision T160-0 item 3,
/// 2026-11-15). The real wing is selected by the crew to three positions, 20,
/// 35 and 65 degrees (`TU160.INF`, cited); TORE has no manual input, so a speed
/// law visits the three detents (opinionated): 20 degrees up to 250 knots, 35
/// from 320 to 520 knots, 65 from 600 knots up, with smooth (smoothstep)
/// transitions between them. Flaps hold the wing at 20. The pivot is fitted to
/// the three published spans (12.8 ft out and level with the reference point:
/// shape (4.8, 0) at the header's 8/3 ft a unit), which puts it inside the
/// nacelle, so the turning root corners go into the fixed airframe (the panel
/// root seam is accepted). No wing stations.
pub const TU160: SwingSpec = SwingSpec {
    name: "Tu-160",
    sweep_min: 20.,
    sweep_max: 65.,
    rest_sweep: 20.,
    feet_per_unit: 8. / 3.,
    axis_scale: [1., 1., 1.],
    pivot_right: [4.8, 0., 0.],
    pivot_left: [-4.8, 0., 0.],
    law: SweepLaw::Detents(&[(250., 20.), (320., 35.), (520., 35.), (600., 65.)]),
    gear_holds_spread: false,
    sweep_rate_dps: None,
    placement: Placement::NONE,
    swivel_rows: &[],
    fixed_store_rows: &[],
    max_sweep_with_fixed_stores: None,
    outline: None,
    pylon_length_ft: None,
};

/// The F-111F Aardvark's swing wing (SWING-1, from decision F111-0 items 2 to
/// 4, 2026-11-15). Sweep 16 to 72.5 degrees (`F111.INF`, cited), a whole-panel
/// turn about the fitted pivot, the smallest circle round the panel's root
/// corners: shape (13, 2.5). The shape is drawn at the real 16 degrees (its
/// leading edge measures 14) at the header's 2/3 ft a unit with the size fit
/// per axis, x 0.859, y 1.0, z 0.827 ([`SwingSpec::axis_scale`]), so the pivot
/// is 7.4 ft out and 1.7 ft forward. Schedule (fitted; the real wing is the
/// crew's, by lever): spread up to 250 knots, fully swept by 650 knots (about
/// Mach 1.0 at sea level), linear, flaps hold it spread (option b of the
/// review, recommended). Inner pylons, rows 5 and 6, swivel (Jane's, TOR-14)
/// on the mid-chord of the panel (leading edge shape (13, 10) to (53, 0),
/// trailing edge at y -5); the outer pylons, rows 7 and 8, are fixed, and
/// while they carry stores the sweep is held to 26 degrees or less (fitted,
/// matching the real limit). The pylon length is the Tornado's (3.3 ft, which
/// the review judged to fit). No placement offsets: the root seam treatment
/// (4.7 ft at full sweep: a root shift or a glove plate) is the rig slice's
/// call.
pub const F111F: SwingSpec = SwingSpec {
    name: "F-111F",
    sweep_min: 16.,
    sweep_max: 72.5,
    rest_sweep: 16.,
    feet_per_unit: FEET_PER_UNIT,
    axis_scale: [0.859, 1., 0.827],
    pivot_right: [13., 2.5, 0.],
    pivot_left: [-13., 2.5, 0.],
    law: SweepLaw::Linear {
        from_knots: 250.,
        to_knots: 650.,
    },
    gear_holds_spread: false,
    sweep_rate_dps: None,
    placement: Placement::NONE,
    swivel_rows: &[5, 6],
    fixed_store_rows: &[7, 8],
    max_sweep_with_fixed_stores: Some(26.),
    outline: Some(WingOutline {
        leading: [
            [13. * FEET_PER_UNIT * 0.859, 10. * FEET_PER_UNIT],
            [53. * FEET_PER_UNIT * 0.859, 0.],
        ],
        trailing: [
            [13. * FEET_PER_UNIT * 0.859, -5. * FEET_PER_UNIT],
            [53. * FEET_PER_UNIT * 0.859, -5. * FEET_PER_UNIT],
        ],
    }),
    pylon_length_ft: Some(3.3),
};

/// The Tu-26 Backfire-C's swing wing (SWING-1, from the batch C decision 4,
/// 2026-11-15). Sweep 20 to 65 degrees (`TU26.INF`: "from 20 degrees to 65
/// degrees sweepback", cited). The mesh is drawn at 18.4 degrees, so the
/// drawn pose is [`SwingSpec::rest_sweep`] and the spread wing is already
/// turned 1.6 degrees from it; 46.6 degrees is the full turn. Pivot: the
/// measured glove-corner vertex (14, 3) of the shape, 18.67 ft out and 4 ft
/// forward at the header's 4/3 ft a unit. Schedule (fitted): spread to 300
/// knots and whenever the flaps or the gear are out, fully swept by 560 knots,
/// linear. No fence, no glove plate, no wing stations (the glove rows are fixed).
pub const TU26: SwingSpec = SwingSpec {
    name: "Tu-26",
    sweep_min: 20.,
    sweep_max: 65.,
    rest_sweep: 18.4,
    feet_per_unit: 4. / 3.,
    axis_scale: [1., 1., 1.],
    pivot_right: [14., 3., 0.],
    pivot_left: [-14., 3., 0.],
    law: SweepLaw::Linear {
        from_knots: 300.,
        to_knots: 560.,
    },
    gear_holds_spread: true,
    sweep_rate_dps: None,
    placement: Placement::NONE,
    swivel_rows: &[],
    fixed_store_rows: &[],
    max_sweep_with_fixed_stores: None,
    outline: None,
    pylon_length_ft: None,
};

/// Every swing-wing spec there is, the aircraft not yet wired into [`spec`]
/// included, for probes and tests.
pub const ALL: [&SwingSpec; 5] = [&TORNADO, &B1B, &TU160, &F111F, &TU26];

/// The swing wing of aircraft `id`, `None` when it has none. Wiring a new
/// swing-wing aircraft (batch B's identity slices) is one arm here; the
/// specs are already written.
pub fn spec(id: AircraftId) -> Option<&'static SwingSpec> {
    match id.source() {
        AircraftId::Tornado => Some(&TORNADO),
        AircraftId::B1b => Some(&B1B),
        AircraftId::Tu160 => Some(&TU160),
        // AircraftId::F111F => Some(&F111F),
        // AircraftId::Tu26 => Some(&TU26),
        _ => None,
    }
}

/// How far aircraft `id`'s wing has turned from its drawn pose, radians, at
/// `speed_fps` with its flaps and gear out (0 to 1): 0 for an aircraft with no
/// swing wing.
pub fn wing_turn_of(id: AircraftId, speed_fps: f64, flaps: f64, gear: f64) -> f64 {
    spec(id).map_or(0., |spec| spec.turn(speed_fps, flaps, gear))
}

// The Tornado's constants, read from its spec (SWING-1).

/// Real minimum and maximum Tornado wing sweep, degrees (`TOR.INF`, Jane's);
/// the shape's spread pose is the minimum.
pub const SWEEP_MIN: f64 = TORNADO.sweep_min;
pub const SWEEP_MAX: f64 = TORNADO.sweep_max;
/// The Tornado's speed schedule, knots: spread up to the first, fully swept
/// at the second ([`TORNADO`]).
pub const SWEEP_FROM_KNOTS: f64 = TORNADO.law.from_knots();
pub const SWEEP_TO_KNOTS: f64 = TORNADO.law.to_knots();
/// The Tornado's wing sweep pivots, shape units (right, forward, up), vertical
/// axis (TOR-12, John 2026-11-12).
pub const PIVOT_R: [f32; 3] = [
    TORNADO.pivot_right[0] as f32,
    TORNADO.pivot_right[1] as f32,
    TORNADO.pivot_right[2] as f32,
];
pub const PIVOT_L: [f32; 3] = [
    TORNADO.pivot_left[0] as f32,
    TORNADO.pivot_left[1] as f32,
    TORNADO.pivot_left[2] as f32,
];

/// The Tornado's wing placement (TOR-12, John 2026-11-12): in, up, forward and
/// the root points' shift ([`Placement`]).
pub const WING_IN_FT: f64 = TORNADO.placement.in_ft;
pub const WING_UP_FT: f64 = TORNADO.placement.up_ft;
pub const WING_FORWARD_FT: f64 = TORNADO.placement.forward_ft;
pub const ROOT_SHIFT_FT: f64 = TORNADO.placement.root_shift_ft;

/// The Tornado's placed right wing panel's leading and trailing edges in the
/// spread pose (TOR-14, 2026-11-13), each as its inboard and outboard corner
/// [right, forward] in feet ([`WingOutline`]).
pub const WING_LEADING: [[f64; 2]; 2] = match TORNADO.outline {
    Some(outline) => outline.leading,
    None => panic!("the Tornado has an outline"),
};
pub const WING_TRAILING: [[f64; 2]; 2] = match TORNADO.outline {
    Some(outline) => outline.trailing,
    None => panic!("the Tornado has an outline"),
};

/// The Tornado's mid-chord ([`WingOutline::mid_chord`]): the forward
/// coordinate (feet, aircraft frame, wing spread) of the middle of the placed
/// panel's chord at the span station `right` feet out of the reference point.
/// Right-hand wing; the left mirrors it.
pub const fn mid_chord(right: f64) -> f64 {
    WingOutline {
        leading: WING_LEADING,
        trailing: WING_TRAILING,
    }
    .mid_chord(right)
}

/// How long every swivelling Tornado wing pylon is drawn, feet
/// ([`TORNADO`]'s `pylon_length_ft`).
pub const PYLON_LENGTH_FT: f64 = match TORNADO.pylon_length_ft {
    Some(length) => length,
    None => panic!("the Tornado has a pylon length"),
};

/// The fixed length of the swivelling pylon of source hardpoint `hardpoint`
/// of aircraft `id`, when it has one ([`swivels`]).
pub fn pylon_length(id: AircraftId, hardpoint: usize) -> Option<f64> {
    spec(id)?.pylon_length(hardpoint)
}

/// How far the Tornado's wing panel has turned from its spread pose, radians
/// (0 spread, `SWEEP_MAX - SWEEP_MIN` degrees fully swept), for an aircraft
/// flying at `speed_fps` with its flaps `flaps` out (0 to 1). For another
/// aircraft use [`wing_turn_of`] or its [`SwingSpec::turn`].
pub fn wing_turn(speed_fps: f64, flaps: f64) -> f64 {
    TORNADO.turn(speed_fps, flaps, 0.)
}

/// The speed in feet per second at which the Tornado's wing has turned
/// `degrees` of sweep (25 spread to 67), flaps up: the inverse of
/// [`wing_turn`], for captures and measurements that pose the wing.
pub fn speed_for_sweep(degrees: f64) -> f64 {
    TORNADO.speed_for_sweep(degrees)
}

/// The pivot of one Tornado wing, feet right, up and forward (the station
/// frame).
pub fn pivot_feet(right: bool) -> [f64; 3] {
    TORNADO.pivot_feet(right)
}

/// Where the Tornado's wing carries a point of its own panel, feet right, up
/// and forward of the aircraft: `point` is where the point is with the wing
/// spread, `turn` the wing's turn ([`wing_turn`]), the tip moving aft. The
/// height does not change. `right` names the wing (a left-hand point is
/// carried by the left wing).
pub fn swept(right: bool, point: [f64; 3], turn: f64) -> [f64; 3] {
    TORNADO.swept(right, point, turn)
}

/// [`swept`] undone: where a point at `point` now was with the wing spread.
pub fn unswept(right: bool, point: [f64; 3], turn: f64) -> [f64; 3] {
    TORNADO.unswept(right, point, turn)
}

/// The pylon's yaw against the wing panel, radians, positive to the
/// aircraft's right: the wing's own turn undone, so a store on a swivelling
/// pylon keeps its heading along the fuselage at every sweep.
pub fn pylon_swivel(right: bool, turn: f64) -> f64 {
    -turn * if right { 1. } else { -1. }
}

/// Whether source hardpoint `hardpoint` of aircraft `id` is a swivelling wing
/// pylon, carried by the swinging wing ([`SwingSpec::swivel_rows`]).
pub fn swivels(id: AircraftId, hardpoint: usize) -> bool {
    spec(id).is_some_and(|spec| spec.swivels(hardpoint))
}

/// Whether `id` has any swivelling station.
pub fn has_swivel(id: AircraftId) -> bool {
    spec(id).is_some_and(SwingSpec::has_swivel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schedule_spreads_below_304_knots_and_sweeps_fully_at_600() {
        assert_eq!(wing_turn(200. * FPS_PER_KNOT, 0.), 0.);
        // John, 2026-11-12: 350 mph is 304 knots, and the wing is still
        // spread there; the old 250 knots start is gone.
        assert_eq!(wing_turn(SWEEP_FROM_KNOTS * FPS_PER_KNOT, 0.), 0.);
        assert_eq!(wing_turn(250. * FPS_PER_KNOT, 0.), 0.);
        assert_eq!(wing_turn(300. * FPS_PER_KNOT, 0.), 0.);
        assert!((SWEEP_FROM_KNOTS - 350. * 0.868_976).abs() < 0.5);
        assert!(wing_turn(305. * FPS_PER_KNOT, 0.) > 0.);
        let full = (SWEEP_MAX - SWEEP_MIN).to_radians();
        assert!((wing_turn(600. * FPS_PER_KNOT, 0.) - full).abs() < 1e-12);
        assert!((wing_turn(900. * FPS_PER_KNOT, 0.) - full).abs() < 1e-12);
        assert!((wing_turn(452. * FPS_PER_KNOT, 0.) - full / 2.).abs() < 1e-12);
        // Flaps hold the wing out.
        assert_eq!(wing_turn(900. * FPS_PER_KNOT, 1.), 0.);
    }

    #[test]
    fn the_pivots_are_johns_and_mirror_each_other() {
        // John, 2026-11-12 (TOR-12): right pivot (4.3333, -0.9007, 0.31) ft.
        let r = pivot_feet(true);
        assert!((r[0] - 4.333_333).abs() < 1e-5);
        assert!((r[2] + 0.900_683).abs() < 1e-5);
        assert!((r[1] - 0.31).abs() < 1e-5);
        let l = pivot_feet(false);
        assert_eq!((l[0], l[1], l[2]), (-r[0], r[1], r[2]));
        assert!((WING_UP_FT - 0.116_666_7).abs() < 1e-6);
    }

    #[test]
    fn the_mid_chord_is_halfway_between_the_placed_panels_edges() {
        // TOR-14: the placed right panel's leading edge runs from (2.1117,
        // 4.8667) to (19.1267, -1.1333) ft and its trailing edge from (2.7784,
        // -2.4667) to (18.46, -5.8), read off the drawn panel's corners.
        assert!((WING_LEADING[0][0] - 2.1117).abs() < 1e-4);
        assert!((WING_LEADING[0][1] - 4.8667).abs() < 1e-4);
        assert!((WING_LEADING[1][0] - 19.1267).abs() < 1e-4);
        assert!((WING_LEADING[1][1] + 1.1333).abs() < 1e-4);
        assert!((WING_TRAILING[0][0] - 2.7784).abs() < 1e-4);
        assert!((WING_TRAILING[0][1] + 2.4667).abs() < 1e-4);
        assert!((WING_TRAILING[1][0] - 18.46).abs() < 1e-4);
        assert!((WING_TRAILING[1][1] + 5.8).abs() < 1e-4);
        // The inner pylon's span (7.96 ft out) and the outer pods' (14.46).
        assert!(
            (mid_chord(7.96) + 0.3815).abs() < 1e-3,
            "{}",
            mid_chord(7.96)
        );
        assert!(
            (mid_chord(14.46) + 2.2187).abs() < 1e-3,
            "{}",
            mid_chord(14.46)
        );
        // Aft with the span, as the panel's chord runs.
        assert!(mid_chord(14.46) < mid_chord(7.96));
    }

    #[test]
    fn the_inverse_schedule_poses_the_wing() {
        for degrees in [25., 35., 46., 60., 67.] {
            let turn = wing_turn(speed_for_sweep(degrees), 0.);
            assert!((turn.to_degrees() + SWEEP_MIN - degrees).abs() < 1e-9);
        }
    }

    #[test]
    fn a_swept_point_turns_about_the_pivot_and_comes_back() {
        for right in [true, false] {
            let side = if right { 1. } else { -1. };
            let point = [side * 10., -8., 3.];
            for degrees in [25., 67.] {
                let turn = (degrees - SWEEP_MIN).to_radians();
                let there = swept(right, point, turn);
                let pivot = pivot_feet(right);
                let radius = |p: [f64; 3]| (p[0] - pivot[0]).hypot(p[2] - pivot[2]);
                assert!((radius(there) - radius(point)).abs() < 1e-12);
                assert_eq!(there[1], point[1]);
                let back = unswept(right, there, turn);
                for i in 0..3 {
                    assert!((back[i] - point[i]).abs() < 1e-12);
                }
                // A point outboard of the pivot moves aft as the wing sweeps.
                if turn > 0. {
                    assert!(there[2] < point[2] + 1e-12 || point[2] < pivot[2]);
                }
            }
        }
    }

    #[test]
    fn only_the_tornados_wing_pylons_and_the_outer_pod_pylons_swivel() {
        let rows: Vec<usize> = (0..12)
            .filter(|row| swivels(AircraftId::Tornado, *row))
            .collect();
        // Row 7, the inner wing pair, row 8, the AIM-9 side rails on its
        // pylons (TOR-6b, 2026-11-11), and stations 9 and 10, the Combat
        // version's outer pod pylons (TOR-13, 2026-11-12). The Clean version
        // swivels the same rows (it has no pod on 9 and 10).
        assert_eq!(rows, [7, 8, 9, 10]);
        assert!(has_swivel(AircraftId::Tornado));
        for row in 0..12 {
            assert_eq!(
                swivels(AircraftId::TornadoClean, row),
                swivels(AircraftId::Tornado, row)
            );
        }
        assert!(has_swivel(AircraftId::TornadoClean));
        for id in [AircraftId::F15E, AircraftId::Mig23, AircraftId::F14] {
            assert!(!has_swivel(id));
            assert!((0..12).all(|row| !swivels(id, row)));
        }
    }

    #[test]
    fn the_pylon_swivels_back_by_the_wings_turn() {
        assert_eq!(pylon_swivel(true, 0.5), -0.5);
        assert_eq!(pylon_swivel(false, 0.5), 0.5);
    }

    // SWING-1 (2026-11-15): the spec and the aircraft that are not wired yet.

    fn knots(k: f64) -> f64 {
        k * FPS_PER_KNOT
    }
    fn degrees(spec: &SwingSpec, k: f64, flaps: f64, gear: f64) -> f64 {
        spec.sweep_degrees(spec.turn(knots(k), flaps, gear))
    }

    #[test]
    fn the_tornados_spec_is_the_modules_constants_and_resolves_for_both_versions() {
        assert_eq!(spec(AircraftId::Tornado), Some(&TORNADO));
        assert_eq!(spec(AircraftId::TornadoClean), Some(&TORNADO));
        assert_eq!(spec(AircraftId::F15E), None);
        assert_eq!(spec(AircraftId::Mig23), None);
        assert_eq!((SWEEP_MIN, SWEEP_MAX), (25., 67.));
        assert_eq!((SWEEP_FROM_KNOTS, SWEEP_TO_KNOTS), (304., 600.));
        assert_eq!(PIVOT_R, [6.5, -1.351_025, 0.465]);
        assert_eq!(PIVOT_L, [-6.5, -1.351_025, 0.465]);
        assert_eq!(WING_IN_FT, 1.54);
        assert_eq!(WING_FORWARD_FT, 0.2);
        assert_eq!(ROOT_SHIFT_FT, 2.3483);
        assert_eq!(PYLON_LENGTH_FT, 3.3);
        assert_eq!(TORNADO.rest_sweep, TORNADO.sweep_min);
        assert_eq!(TORNADO.min_turn(), 0.);
        assert_eq!(TORNADO.max_turn(), (SWEEP_MAX - SWEEP_MIN).to_radians());
        // Gear never holds the Tornado's wing.
        assert_eq!(
            TORNADO.turn(knots(700.), 0., 1.),
            wing_turn(knots(700.), 0.)
        );
        assert_eq!(
            wing_turn_of(AircraftId::Tornado, knots(452.), 0.3, 1.),
            wing_turn(knots(452.), 0.3)
        );
        assert_eq!(wing_turn_of(AircraftId::F15E, knots(700.), 0., 0.), 0.);
        assert!((TORNADO.degrees_per_knot().unwrap() - 0.141_9).abs() < 1e-3);
        assert_eq!(TORNADO.sweep_rate_dps, None);
        assert_eq!(TORNADO.placement.in_ft, 1.54);
    }

    #[test]
    fn every_spec_is_consistent_and_its_schedule_runs_from_spread_to_swept() {
        for spec in ALL {
            assert!(spec.sweep_min < spec.sweep_max, "{}", spec.name);
            assert!(spec.rest_sweep <= spec.sweep_min, "{}", spec.name);
            assert!(spec.law.from_knots() < spec.law.to_knots(), "{}", spec.name);
            // The left pivot mirrors the right.
            assert_eq!(spec.pivot_left[0], -spec.pivot_right[0], "{}", spec.name);
            assert_eq!(spec.pivot_left[1..], spec.pivot_right[1..], "{}", spec.name);
            let (r, l) = (spec.pivot_feet(true), spec.pivot_feet(false));
            assert_eq!((l[0], l[1], l[2]), (-r[0], r[1], r[2]), "{}", spec.name);
            // Spread slow, swept fast, monotone between, held out by flaps.
            let mut last = f64::NEG_INFINITY;
            for k in (0..=1000).step_by(5) {
                let turn = spec.turn(knots(f64::from(k)), 0., 0.);
                assert!(turn >= last - 1e-12, "{} at {k}", spec.name);
                assert!(turn >= spec.min_turn() - 1e-12 && turn <= spec.max_turn() + 1e-12);
                last = turn;
            }
            assert_eq!(
                spec.turn(knots(10.), 0., 0.),
                spec.min_turn(),
                "{}",
                spec.name
            );
            assert!(
                (spec.turn(knots(1000.), 0., 0.) - spec.max_turn()).abs() < 1e-12,
                "{}",
                spec.name
            );
            assert_eq!(
                spec.turn(knots(1000.), 1., 0.),
                spec.min_turn(),
                "{}",
                spec.name
            );
            // The inverse poses the wing.
            for degrees in [
                spec.sweep_min,
                spec.sweep_min + 10.,
                spec.sweep_max - 7.,
                spec.sweep_max,
            ] {
                let turn = spec.turn(spec.speed_for_sweep(degrees), 0., 0.);
                assert!(
                    (spec.sweep_degrees(turn) - degrees).abs() < 1e-6,
                    "{} {degrees}",
                    spec.name
                );
            }
            // A swept point comes back.
            for right in [true, false] {
                let p = [if right { 20. } else { -20. }, 1., -9.];
                let back = spec.unswept(right, spec.swept(right, p, 0.7), 0.7);
                assert!((0..3).all(|i| (back[i] - p[i]).abs() < 1e-12));
            }
        }
    }

    #[test]
    fn the_b1bs_wing_spreads_to_280_knots_and_sweeps_fully_by_520() {
        let b = &B1B;
        assert_eq!((b.sweep_min, b.sweep_max, b.rest_sweep), (15., 67.5, 15.));
        assert!((b.max_turn().to_degrees() - 52.5).abs() < 1e-12);
        // Pivot (10, 11) shape units at 4/3 ft a unit: 13.33 ft out, 14.67 forward.
        let p = b.pivot_feet(true);
        assert!((p[0] - 40. / 3.).abs() < 1e-12);
        assert!((p[2] - 44. / 3.).abs() < 1e-12);
        assert_eq!(p[1], 0.);
        assert_eq!(b.turn(knots(280.), 0., 0.), 0.);
        assert_eq!(b.turn(knots(100.), 0., 0.), 0.);
        assert!(b.turn(knots(281.), 0., 0.) > 0.);
        assert!((degrees(b, 400., 0., 0.) - 41.25).abs() < 1e-9);
        assert!((degrees(b, 520., 0., 0.) - 67.5).abs() < 1e-9);
        assert!((degrees(b, 700., 0., 0.) - 67.5).abs() < 1e-9);
        // Flaps hold it spread, half flaps half way; the gear does not.
        assert_eq!(b.turn(knots(700.), 1., 0.), 0.);
        assert!((degrees(b, 700., 0.5, 0.) - 41.25).abs() < 1e-9);
        assert!((degrees(b, 700., 0., 1.) - 67.5).abs() < 1e-9);
        assert!(!b.has_swivel());
        assert_eq!(b.pylon_length(5), None);
        assert_eq!(b.mid_chord(10.), None);
    }

    #[test]
    fn the_tu160s_wing_visits_20_35_and_65_degrees_and_blends_between() {
        let t = &TU160;
        assert_eq!((t.sweep_min, t.sweep_max, t.rest_sweep), (20., 65., 20.));
        let p = t.pivot_feet(true);
        assert!((p[0] - 12.8).abs() < 1e-12, "{p:?}");
        assert_eq!((p[1], p[2]), (0., 0.));
        // 20 degrees to 250 knots, 35 from 320 to 520, 65 from 600.
        for k in [0., 100., 250.] {
            assert!((degrees(t, k, 0., 0.) - 20.).abs() < 1e-9, "{k}");
        }
        for k in [320., 400., 520.] {
            assert!((degrees(t, k, 0., 0.) - 35.).abs() < 1e-9, "{k}");
        }
        for k in [600., 800.] {
            assert!((degrees(t, k, 0., 0.) - 65.).abs() < 1e-9, "{k}");
        }
        // Smooth: half way up each ramp is half way up the step, and the
        // sweep stays between the detents and rises through them.
        assert!((degrees(t, 285., 0., 0.) - 27.5).abs() < 1e-9);
        assert!((degrees(t, 560., 0., 0.) - 50.).abs() < 1e-9);
        let mut last = 20.;
        for k in 250..=600 {
            let d = degrees(t, f64::from(k), 0., 0.);
            assert!(d >= last - 1e-12 && (20. ..=65.).contains(&d));
            last = d;
        }
        // The ramps leave their detents with no slope (smoothstep).
        assert!(degrees(t, 251., 0., 0.) - 20. < 0.01);
        assert!(35. - degrees(t, 319., 0., 0.) < 0.01);
        // Flaps hold it at 20; the gear does not.
        assert_eq!(t.turn(knots(700.), 1., 0.), 0.);
        assert!((degrees(t, 700., 0., 1.) - 65.).abs() < 1e-9);
        // The inverse finds the speed a sweep first appears at.
        assert!((t.speed_for_sweep(20.) / FPS_PER_KNOT - 250.).abs() < 1e-9);
        assert!((t.speed_for_sweep(35.) / FPS_PER_KNOT - 320.).abs() < 1e-6);
        assert!((t.speed_for_sweep(27.5) / FPS_PER_KNOT - 285.).abs() < 1e-6);
        assert!((t.speed_for_sweep(50.) / FPS_PER_KNOT - 560.).abs() < 1e-6);
        assert!((t.speed_for_sweep(65.) / FPS_PER_KNOT - 600.).abs() < 1e-6);
        assert_eq!(t.degrees_per_knot(), None);
        assert!(!t.has_swivel());
    }

    #[test]
    fn the_tu26s_mesh_rests_at_18_4_so_the_spread_wing_is_already_turned() {
        let t = &TU26;
        assert_eq!((t.sweep_min, t.sweep_max, t.rest_sweep), (20., 65., 18.4));
        assert!((t.min_turn().to_degrees() - 1.6).abs() < 1e-12);
        assert!((t.max_turn().to_degrees() - 46.6).abs() < 1e-12);
        let p = t.pivot_feet(true);
        assert!((p[0] - 56. / 3.).abs() < 1e-12);
        assert!((p[2] - 4.).abs() < 1e-12);
        // Spread (20 degrees, a 1.6 degree turn from the mesh) to 300 knots.
        assert!((t.turn(knots(100.), 0., 0.).to_degrees() - 1.6).abs() < 1e-12);
        assert!((t.turn(knots(300.), 0., 0.).to_degrees() - 1.6).abs() < 1e-12);
        assert!((degrees(t, 300., 0., 0.) - 20.).abs() < 1e-9);
        assert!((degrees(t, 430., 0., 0.) - 42.5).abs() < 1e-9);
        assert!((degrees(t, 560., 0., 0.) - 65.).abs() < 1e-9);
        assert!((degrees(t, 900., 0., 0.) - 65.).abs() < 1e-9);
        // Flaps or gear hold it spread.
        assert!((degrees(t, 900., 1., 0.) - 20.).abs() < 1e-9);
        assert!((degrees(t, 900., 0., 1.) - 20.).abs() < 1e-9);
        assert!((degrees(t, 900., 0., 0.5) - 42.5).abs() < 1e-9);
        assert!((degrees(t, 900., 0.25, 0.5) - 42.5).abs() < 1e-9);
        assert!(!t.has_swivel());
    }

    #[test]
    fn the_f111fs_wing_spreads_to_250_knots_and_swivels_the_inner_rows() {
        let f = &F111F;
        assert_eq!((f.sweep_min, f.sweep_max, f.rest_sweep), (16., 72.5, 16.));
        assert!((f.max_turn().to_degrees() - 56.5).abs() < 1e-12);
        // Pivot shape (13, 2.5) at 2/3 ft a unit, x scaled 0.859.
        let p = f.pivot_feet(true);
        assert!((p[0] - 13. * 2. / 3. * 0.859).abs() < 1e-12);
        assert!((p[2] - 2.5 * 2. / 3.).abs() < 1e-12);
        assert_eq!(f.turn(knots(250.), 0., 0.), 0.);
        assert!((degrees(f, 450., 0., 0.) - 44.25).abs() < 1e-9);
        assert!((degrees(f, 650., 0., 0.) - 72.5).abs() < 1e-9);
        assert_eq!(f.turn(knots(700.), 1., 0.), 0.);
        let rows: Vec<usize> = (0..12).filter(|row| f.swivels(*row)).collect();
        assert_eq!(rows, [5, 6]);
        assert_eq!(f.fixed_store_rows, [7, 8]);
        assert_eq!(f.pylon_length(5), Some(3.3));
        assert_eq!(f.pylon_length(7), None);
        // While the outer rows carry stores the sweep stops at 26 degrees.
        assert_eq!(f.max_sweep_with_fixed_stores, Some(26.));
        let fast = knots(900.);
        assert!((f.sweep_degrees(f.turn_carrying(fast, 0., 0., true)) - 26.).abs() < 1e-9);
        assert!((f.sweep_degrees(f.turn_carrying(fast, 0., 0., false)) - 72.5).abs() < 1e-9);
        // Below the limit the stores change nothing.
        let slow = knots(300.);
        assert_eq!(f.turn_carrying(slow, 0., 0., true), f.turn(slow, 0., 0.));
        // An aircraft with no limit is never held.
        assert_eq!(
            B1B.turn_carrying(fast, 0., 0., true),
            B1B.turn(fast, 0., 0.)
        );
        // The mid-chord reads the panel's own edges: leading (13, 10) to
        // (53, 0), trailing at y -5, shape units.
        let u = 2. / 3.;
        let x13 = 13. * u * 0.859;
        let x53 = 53. * u * 0.859;
        assert!((f.mid_chord(x13).unwrap() - (10. - 5.) / 2. * u).abs() < 1e-9);
        assert!((f.mid_chord(x53).unwrap() - (0. - 5.) / 2. * u).abs() < 1e-9);
        assert!(f.mid_chord(x53).unwrap() < f.mid_chord(x13).unwrap());
    }

    #[test]
    fn the_tornados_spec_still_swivels_and_the_other_specs_only_where_they_say() {
        assert!(TORNADO.has_swivel());
        assert!(TORNADO.swivels(7) && TORNADO.swivels(10) && !TORNADO.swivels(6));
        assert_eq!(TORNADO.pylon_length(9), Some(PYLON_LENGTH_FT));
        assert_eq!(TORNADO.mid_chord(7.96), Some(mid_chord(7.96)));
        assert_eq!(TORNADO.max_sweep_with_fixed_stores, None);
        for spec in [&B1B, &TU160, &TU26] {
            assert!(!spec.has_swivel() && spec.outline.is_none());
            assert_eq!(spec.placement, Placement::NONE);
        }
        // Update (coordinator, 2026-11-15): the B-1B and Tu-160 are wired
        // with their identities (BMB-B1); the F-111F and Tu-26 are not yet.
        assert!(AircraftId::ALL.iter().all(|id| match spec(*id) {
            None => true,
            Some(s) => {
                (id.source() == AircraftId::Tornado && s.name == TORNADO.name)
                    || (*id == AircraftId::B1b && s.name == B1B.name)
                    || (*id == AircraftId::Tu160 && s.name == TU160.name)
            }
        }));
    }
}
