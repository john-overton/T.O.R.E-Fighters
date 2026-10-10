//! Redrawn airports: an experiment (AP1, 2026-10-10) that replaces one retail
//! airfield shape with an airport drawn at real-world size from a small data
//! file we own, textured with retail airport art. Off by default; the app
//! turns it on with `TORE_REDRAWN_AIRPORTS=1` through
//! [`super::Overrides::redrawn_airports`].
//!
//! A plan (`crates/tore-world/airports/*.toml`) gives runways, taxiways,
//! aprons, the AI's taxi, takeoff, landing and parking points and the
//! buildings, in feet in the **runway frame**: `x` to the right of the
//! primary runway's centreline, `z` along it from its near threshold. The
//! frame's origin is the retail runway's own near threshold (STRIP anchor
//! 0x11), so the runway's position, heading, length and ILS do not move.
//! The plan carries no retail coordinates, only the runway length it
//! expects, which is checked against the retail shape.
//!
//! What a redrawn airport replaces, for its one STRIP placement: the drawn
//! mesh ([`Built::patches`], drawn by the app), the landable box and contact
//! box ([`Built::surface`], the whole airfield as before, grass included),
//! the AI anchors ([`Built::anchors`]) and the positions of the airport's
//! buildings, plus extra buildings and vehicles. The runway's approach
//! line, length and elevation stay the retail ones.
mod data;

use super::{Placements, runway_length_ft};
use crate::WorldResult;
use data::{Document, Table, Value};
use tore_formats::mission::{Placement, SourceKey};
use tore_sim::{ai::airfield::AirfieldAnchors, airport::OrientedBox};

/// The plans the build ships, one file per airport.
const BUILTIN: [(&str, &str); 1] = [(
    "airports/ukr-kiev.toml",
    include_str!("../../airports/ukr-kiev.toml"),
)];

/// Ordinal of the first building a plan adds to its layout: its object id is
/// `0x4000_0000` plus this plus its index, inside the layout id range and
/// far above any retail layout's object count.
pub const ADDED_ORDINAL_BASE: u32 = 0x00F0_0000;

/// Runway markings along the runway from each threshold, feet (`fitted`,
/// agent, from the usual real layout): threshold stripes for 150 ft, the
/// designation numbers 60 ft tall starting 40 ft after them, then the
/// touchdown zone texture.
const THRESHOLD_FT: f64 = 150.;
const NUMBERS_FROM_FT: f64 = 190.;
const NUMBER_HEIGHT_FT: f64 = 60.;
const NUMBER_WIDTH_FT: f64 = 20.;
const NUMBER_GAP_FT: f64 = 10.;
const TOUCHDOWN_FROM_FT: f64 = 300.;

/// The plans to apply: the built-in ones when `enabled`, else none.
pub fn plans(enabled: bool) -> WorldResult<Vec<Plan>> {
    if !enabled {
        return Ok(Vec::new());
    }
    BUILTIN
        .iter()
        .map(|(name, text)| Plan::parse(text).map_err(|e| format!("{name}: {e}").into()))
        .collect()
}

/// One texture region of a retail PIC and the feet one copy of it covers.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub pic: String,
    /// Texel rectangle `[x, y, width, height]`, rows counted from the top.
    pub rect: [f64; 4],
    /// Feet one copy covers along the texture's columns (`u`) and rows
    /// (`v`). A runway material's `u` of 0 spans the runway's width.
    pub tile_ft: [f64; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunwaySpec {
    pub name: String,
    /// Centreline, feet right of the primary centreline.
    pub x: f64,
    /// Near threshold, feet along from the frame origin.
    pub threshold: f64,
    pub length: f64,
    pub width: f64,
    /// Paved run before each threshold, feet.
    pub pad: f64,
    /// Touchdown zone texture length after each threshold's numbers, feet.
    pub touchdown: f64,
    /// Designators at the near and far thresholds, for example "36", "18".
    pub numbers: Option<[String; 2]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaxiwaySpec {
    pub name: String,
    pub width: f64,
    pub from: [f64; 2],
    pub to: [f64; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApronSpec {
    pub name: String,
    pub min: [f64; 2],
    pub max: [f64; 2],
}

/// The AI's airfield points (`AirfieldAnchors`), in the runway frame.
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorSpec {
    pub taxi_out: [[f64; 2]; 4],
    pub takeoff: [f64; 2],
    pub landing: [f64; 2],
    pub taxi_in: [[f64; 2]; 4],
    pub parking: [[f64; 2]; 9],
    /// Parked heading against the runway heading, degrees.
    pub parking_heading: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuildingSpec {
    pub object_type: String,
    pub at: [f64; 2],
    /// Heading against the runway heading, whole degrees.
    pub heading: i32,
    /// The retail placement (its `alias`) this building moves; `None` adds one.
    pub replaces: Option<i32>,
}

/// A parsed airport plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// Layout code without `.MM`, for example `UKR`.
    pub layout: String,
    /// The STRIP placement's name in that layout.
    pub strip: String,
    /// The retail runway length the plan was drawn for; a mismatch refuses it.
    pub runway_length_ft: f64,
    /// Grass kept landable around the pavement, feet.
    pub grass_margin_ft: f64,
    pub materials: Vec<Material>,
    pub runways: Vec<RunwaySpec>,
    pub taxiways: Vec<TaxiwaySpec>,
    pub aprons: Vec<ApronSpec>,
    pub anchors: AnchorSpec,
    pub buildings: Vec<BuildingSpec>,
}

fn field<'t>(table: &'t Table, key: &str, what: &str) -> Result<&'t Value, String> {
    table
        .get(key)
        .ok_or_else(|| format!("{what}: missing {key}"))
}
fn number(table: &Table, key: &str, what: &str) -> Result<f64, String> {
    field(table, key, what)?
        .number()
        .ok_or_else(|| format!("{what}: {key} must be a number"))
}
fn number_or(table: &Table, key: &str, what: &str, default: f64) -> Result<f64, String> {
    if table.contains_key(key) {
        number(table, key, what)
    } else {
        Ok(default)
    }
}
fn text(table: &Table, key: &str, what: &str) -> Result<String, String> {
    Ok(field(table, key, what)?
        .text()
        .ok_or_else(|| format!("{what}: {key} must be a string"))?
        .to_owned())
}
fn point(table: &Table, key: &str, what: &str) -> Result<[f64; 2], String> {
    field(table, key, what)?
        .numbers::<2>()
        .ok_or_else(|| format!("{what}: {key} must be [x, z]"))
}
fn points<const N: usize>(table: &Table, key: &str, what: &str) -> Result<[[f64; 2]; N], String> {
    let items = field(table, key, what)?
        .array()
        .filter(|items| items.len() == N)
        .ok_or_else(|| format!("{what}: {key} must hold {N} points"))?;
    let mut out = [[0.; 2]; N];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item
            .numbers::<2>()
            .ok_or_else(|| format!("{what}: {key} must hold [x, z] points"))?;
    }
    Ok(out)
}

impl Plan {
    pub fn parse(text_in: &str) -> Result<Self, String> {
        let document = Document::parse(text_in)?;
        let root = &document.root;
        let empty = Vec::new();
        let array = |name: &str| document.arrays.get(name).unwrap_or(&empty);
        let materials = array("material")
            .iter()
            .map(|t| {
                let name = text(t, "name", "material")?;
                let rect = field(t, "rect", &name)?
                    .numbers::<4>()
                    .ok_or_else(|| format!("{name}: rect must be [x, y, w, h]"))?;
                let tile_ft = point(t, "tile_ft", &name)?;
                if rect[2] < 1. || rect[3] < 1. || tile_ft[1] <= 0. || tile_ft[0] < 0. {
                    return Err(format!("{name}: empty rect or tile"));
                }
                Ok(Material {
                    pic: text(t, "pic", &name)?,
                    name,
                    rect,
                    tile_ft,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let runways = array("runway")
            .iter()
            .map(|t| {
                let name = text(t, "name", "runway")?;
                let numbers = match t.get("numbers") {
                    None => None,
                    Some(value) => {
                        let items = value
                            .array()
                            .filter(|items| items.len() == 2)
                            .ok_or_else(|| format!("{name}: numbers must be two strings"))?;
                        let text = |i: usize| {
                            items[i]
                                .text()
                                .filter(|s| s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty())
                                .map(str::to_owned)
                                .ok_or_else(|| format!("{name}: numbers must be digits"))
                        };
                        Some([text(0)?, text(1)?])
                    }
                };
                let spec = RunwaySpec {
                    x: number(t, "x", &name)?,
                    threshold: number(t, "threshold", &name)?,
                    length: number(t, "length", &name)?,
                    width: number(t, "width", &name)?,
                    pad: number_or(t, "pad", &name, 0.)?,
                    touchdown: number_or(t, "touchdown", &name, 1_500.)?,
                    numbers,
                    name,
                };
                if spec.length <= 2. * (TOUCHDOWN_FROM_FT + spec.touchdown)
                    || spec.width < 2. * (NUMBER_WIDTH_FT + NUMBER_GAP_FT)
                    || spec.pad < 0.
                {
                    return Err(format!("{}: runway too short or narrow", spec.name));
                }
                Ok(spec)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let taxiways = array("taxiway")
            .iter()
            .map(|t| {
                let name = text(t, "name", "taxiway")?;
                let spec = TaxiwaySpec {
                    width: number(t, "width", &name)?,
                    from: point(t, "from", &name)?,
                    to: point(t, "to", &name)?,
                    name,
                };
                let straight = spec.from[0] == spec.to[0] || spec.from[1] == spec.to[1];
                if !straight || spec.from == spec.to || spec.width <= 0. {
                    return Err(format!(
                        "{}: a taxiway runs along x or z between two points",
                        spec.name
                    ));
                }
                Ok(spec)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let aprons = array("apron")
            .iter()
            .map(|t| {
                let name = text(t, "name", "apron")?;
                let spec = ApronSpec {
                    min: point(t, "min", &name)?,
                    max: point(t, "max", &name)?,
                    name,
                };
                if spec.min[0] >= spec.max[0] || spec.min[1] >= spec.max[1] {
                    return Err(format!("{}: min must be below max", spec.name));
                }
                Ok(spec)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let a = document.tables.get("anchors").ok_or("missing [anchors]")?;
        let anchors = AnchorSpec {
            taxi_out: points(a, "taxi_out", "anchors")?,
            takeoff: point(a, "takeoff", "anchors")?,
            landing: point(a, "landing", "anchors")?,
            taxi_in: points(a, "taxi_in", "anchors")?,
            parking: points(a, "parking", "anchors")?,
            parking_heading: number(a, "parking_heading", "anchors")?,
        };
        let buildings = array("building")
            .iter()
            .map(|t| {
                let object_type = text(t, "type", "building")?;
                let heading = number(t, "heading", &object_type)?;
                if heading.fract() != 0. || heading.abs() > 360. {
                    return Err(format!("{object_type}: heading must be whole degrees"));
                }
                let replaces =
                    match t.get("replaces") {
                        None => None,
                        Some(value) => {
                            Some(value.number().filter(|v| v.fract() == 0.).ok_or_else(|| {
                                format!("{object_type}: replaces must be an alias")
                            })? as i32)
                        }
                    };
                Ok(BuildingSpec {
                    at: point(t, "at", &object_type)?,
                    heading: heading as i32,
                    replaces,
                    object_type,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let plan = Self {
            layout: text(root, "layout", "plan")?,
            strip: text(root, "strip", "plan")?,
            runway_length_ft: number(root, "runway_length_ft", "plan")?,
            grass_margin_ft: number_or(root, "grass_margin_ft", "plan", 0.)?,
            materials,
            runways,
            taxiways,
            aprons,
            anchors,
            buildings,
        };
        let primary = plan.runways.first().ok_or("a plan needs a runway")?;
        if primary.x != 0. || primary.threshold != 0. || primary.length != plan.runway_length_ft {
            return Err(
                "the first runway is the retail one: x 0, threshold 0, length runway_length_ft"
                    .into(),
            );
        }
        for name in [
            "runway_plain",
            "runway_threshold",
            "runway_touchdown",
            "runway_centreline",
            "taxiway",
            "apron",
        ] {
            plan.material(name)
                .ok_or_else(|| format!("missing material {name}"))?;
        }
        Ok(plan)
    }

    fn material(&self, name: &str) -> Option<usize> {
        self.materials.iter().position(|m| m.name == name)
    }

    /// The paved patches of the airport, in the runway frame: runways
    /// first, then taxiways and aprons with what earlier elements cover cut
    /// away, so no two patches overlap.
    pub fn patches(&self) -> Vec<Patch> {
        let mut out = Vec::new();
        let mut covered: Vec<Rect> = Vec::new();
        for runway in &self.runways {
            self.runway_patches(runway, &mut out);
            covered.push(runway_rect(runway));
        }
        let taxiway = self.material("taxiway").expect("validated");
        for spec in &self.taxiways {
            let half = spec.width * 0.5;
            let along_z = spec.from[0] == spec.to[0];
            let (lo, hi) = if along_z {
                (spec.from[1].min(spec.to[1]), spec.from[1].max(spec.to[1]))
            } else {
                (spec.from[0].min(spec.to[0]), spec.from[0].max(spec.to[0]))
            };
            let (rect, u, v, origin) = if along_z {
                let x = spec.from[0];
                (
                    Rect::new([x - half, lo - half], [x + half, hi + half]),
                    [0., 1.],
                    [1., 0.],
                    [x - half, lo - half],
                )
            } else {
                let z = spec.from[1];
                (
                    Rect::new([lo - half, z - half], [hi + half, z + half]),
                    [1., 0.],
                    [0., 1.],
                    [lo - half, z - half],
                )
            };
            let cell = self.materials[taxiway].tile_ft;
            for piece in subtract_all(rect, &covered) {
                out.push(Patch::new(piece, taxiway, origin, u, v, cell));
            }
            covered.push(rect);
        }
        let apron = self.material("apron").expect("validated");
        for spec in &self.aprons {
            let rect = Rect::new(spec.min, spec.max);
            let cell = self.materials[apron].tile_ft;
            for piece in subtract_all(rect, &covered) {
                out.push(Patch::new(
                    piece,
                    apron,
                    [spec.min[0], spec.max[1]],
                    [1., 0.],
                    [0., -1.],
                    cell,
                ));
            }
            covered.push(rect);
        }
        out
    }

    fn runway_patches(&self, runway: &RunwaySpec, out: &mut Vec<Patch>) {
        let material = |name: &str| self.material(name).expect("validated");
        let (plain, threshold, touchdown, centreline) = (
            material("runway_plain"),
            material("runway_threshold"),
            material("runway_touchdown"),
            material("runway_centreline"),
        );
        let half = runway.width * 0.5;
        let (x0, x1) = (runway.x - half, runway.x + half);
        let near = runway.threshold;
        let far = runway.threshold + runway.length;
        let cell = |m: usize| {
            let tile = self.materials[m].tile_ft;
            [if tile[0] > 0. { tile[0] } else { runway.width }, tile[1]]
        };
        // Near end: texture columns run to the pilot's right (+x) and rows
        // back toward the threshold (-z), so numbers read upright on approach.
        // The far end is the same turned half round.
        let band = |out: &mut Vec<Patch>, m: usize, z0: f64, z1: f64, far_end: bool| {
            if z1 - z0 < 1e-6 {
                return;
            }
            let (origin, u, v) = if far_end {
                ([x1, far], [-1., 0.], [0., 1.])
            } else {
                ([x0, near], [1., 0.], [0., -1.])
            };
            out.push(Patch::new(
                Rect::new([x0, z0], [x1, z1]),
                m,
                origin,
                u,
                v,
                cell(m),
            ));
        };
        let td_end = TOUCHDOWN_FROM_FT + runway.touchdown;
        band(out, plain, near - runway.pad, near, false);
        band(out, threshold, near, near + THRESHOLD_FT, false);
        band(
            out,
            plain,
            near + THRESHOLD_FT,
            near + NUMBERS_FROM_FT,
            false,
        );
        self.numbers(runway, out, near + NUMBERS_FROM_FT, false);
        band(
            out,
            plain,
            near + NUMBERS_FROM_FT + NUMBER_HEIGHT_FT,
            near + TOUCHDOWN_FROM_FT,
            false,
        );
        band(
            out,
            touchdown,
            near + TOUCHDOWN_FROM_FT,
            near + td_end,
            false,
        );
        band(out, centreline, near + td_end, far - td_end, false);
        band(out, touchdown, far - td_end, far - TOUCHDOWN_FROM_FT, true);
        band(
            out,
            plain,
            far - TOUCHDOWN_FROM_FT,
            far - NUMBERS_FROM_FT - NUMBER_HEIGHT_FT,
            true,
        );
        self.numbers(runway, out, far - NUMBERS_FROM_FT - NUMBER_HEIGHT_FT, true);
        band(out, plain, far - NUMBERS_FROM_FT, far - THRESHOLD_FT, true);
        band(out, threshold, far - THRESHOLD_FT, far, true);
        band(out, plain, far, far + runway.pad, true);
    }

    /// The designation band from `z0`: the two digits centred, plain runway
    /// around them, or all plain when a digit has no material.
    fn numbers(&self, runway: &RunwaySpec, out: &mut Vec<Patch>, z0: f64, far_end: bool) {
        let plain = self.material("runway_plain").expect("validated");
        let half = runway.width * 0.5;
        let (x0, x1) = (runway.x - half, runway.x + half);
        let z1 = z0 + NUMBER_HEIGHT_FT;
        let plain_cell = {
            let tile = self.materials[plain].tile_ft;
            [if tile[0] > 0. { tile[0] } else { runway.width }, tile[1]]
        };
        let (origin, u, v) = if far_end {
            ([x1, runway.threshold + runway.length], [-1., 0.], [0., 1.])
        } else {
            ([x0, runway.threshold], [1., 0.], [0., -1.])
        };
        let designator = runway.numbers.as_ref().map(|n| &n[usize::from(far_end)]);
        let digits: Option<Vec<usize>> = designator.and_then(|text| {
            text.chars()
                .map(|c| self.material(&format!("digit_{c}")))
                .collect()
        });
        let Some(digits) = digits.filter(|d| !d.is_empty()) else {
            out.push(Patch::new(
                Rect::new([x0, z0], [x1, z1]),
                plain,
                origin,
                u,
                v,
                plain_cell,
            ));
            return;
        };
        let count = digits.len() as f64;
        let span = count * NUMBER_WIDTH_FT + (count - 1.) * NUMBER_GAP_FT;
        let mut covered = Vec::new();
        for (i, material) in digits.iter().enumerate() {
            // Reading order is the pilot's left to right: +x near, -x far.
            let offset = -span * 0.5 + i as f64 * (NUMBER_WIDTH_FT + NUMBER_GAP_FT);
            let (a, b) = if far_end {
                (runway.x - offset - NUMBER_WIDTH_FT, runway.x - offset)
            } else {
                (runway.x + offset, runway.x + offset + NUMBER_WIDTH_FT)
            };
            let rect = Rect::new([a, z0], [b, z1]);
            let digit_origin = if far_end { [b, z0] } else { [a, z1] };
            out.push(Patch::new(
                rect,
                *material,
                digit_origin,
                u,
                v,
                [NUMBER_WIDTH_FT, NUMBER_HEIGHT_FT],
            ));
            covered.push(rect);
        }
        for piece in subtract_all(Rect::new([x0, z0], [x1, z1]), &covered) {
            out.push(Patch::new(piece, plain, origin, u, v, plain_cell));
        }
    }

    /// The pavement's extent in the runway frame, grass margin included: the
    /// landable box.
    pub fn landable(&self, patches: &[Patch]) -> Rect {
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for patch in patches {
            for axis in 0..2 {
                min[axis] = min[axis].min(patch.rect.min[axis]);
                max[axis] = max[axis].max(patch.rect.max[axis]);
            }
        }
        let m = self.grass_margin_ft;
        Rect::new([min[0] - m, min[1] - m], [max[0] + m, max[1] + m])
    }
}

fn runway_rect(runway: &RunwaySpec) -> Rect {
    let half = runway.width * 0.5;
    Rect::new(
        [runway.x - half, runway.threshold - runway.pad],
        [
            runway.x + half,
            runway.threshold + runway.length + runway.pad,
        ],
    )
}

/// An axis-aligned rectangle in the runway frame, feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: [f64; 2],
    pub max: [f64; 2],
}

impl Rect {
    pub fn new(min: [f64; 2], max: [f64; 2]) -> Self {
        Self { min, max }
    }
    pub fn area(&self) -> f64 {
        (self.max[0] - self.min[0]) * (self.max[1] - self.min[1])
    }
    pub fn contains(&self, p: [f64; 2]) -> bool {
        (0..2).all(|i| p[i] >= self.min[i] - 1e-6 && p[i] <= self.max[i] + 1e-6)
    }
    /// This rectangle without `other`: up to four pieces.
    fn minus(self, other: Rect) -> Vec<Rect> {
        let overlap = (0..2).all(|i| other.min[i] < self.max[i] && other.max[i] > self.min[i]);
        if !overlap {
            return vec![self];
        }
        let mut out = Vec::new();
        let mut keep = |r: Rect| {
            if r.max[0] - r.min[0] > 1e-6 && r.max[1] - r.min[1] > 1e-6 {
                out.push(r);
            }
        };
        let x0 = other.min[0].max(self.min[0]);
        let x1 = other.max[0].min(self.max[0]);
        keep(Rect::new(self.min, [x0, self.max[1]]));
        keep(Rect::new([x1, self.min[1]], self.max));
        keep(Rect::new([x0, self.min[1]], [x1, other.min[1]]));
        keep(Rect::new([x0, other.max[1]], [x1, self.max[1]]));
        out
    }
}

fn subtract_all(rect: Rect, covered: &[Rect]) -> Vec<Rect> {
    let mut pieces = vec![rect];
    for other in covered {
        pieces = pieces.into_iter().flat_map(|p| p.minus(*other)).collect();
    }
    pieces
}

/// One paved rectangle and how its material repeats over it: copies of the
/// material's texels laid on a grid of `cell` feet from `origin`, texture
/// columns along `u` and rows along `v` (unit axes of the runway frame).
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub rect: Rect,
    pub material: usize,
    pub origin: [f64; 2],
    pub u: [f64; 2],
    pub v: [f64; 2],
    pub cell: [f64; 2],
}

/// One grid cell of a patch: its corners in the runway frame and the
/// fraction of the material's texels at each, in the same order.
pub type Cell = ([[f64; 2]; 4], [[f64; 2]; 4]);

impl Patch {
    fn new(
        rect: Rect,
        material: usize,
        origin: [f64; 2],
        u: [f64; 2],
        v: [f64; 2],
        cell: [f64; 2],
    ) -> Self {
        Self {
            rect,
            material,
            origin,
            u,
            v,
            cell,
        }
    }

    /// The patch cut at its grid lines.
    pub fn cells(&self) -> Vec<Cell> {
        let to_grid = |p: [f64; 2]| {
            let d = [p[0] - self.origin[0], p[1] - self.origin[1]];
            [
                (d[0] * self.u[0] + d[1] * self.u[1]) / self.cell[0],
                (d[0] * self.v[0] + d[1] * self.v[1]) / self.cell[1],
            ]
        };
        let from_grid = |g: [f64; 2]| {
            let a = g[0] * self.cell[0];
            let b = g[1] * self.cell[1];
            [
                self.origin[0] + self.u[0] * a + self.v[0] * b,
                self.origin[1] + self.u[1] * a + self.v[1] * b,
            ]
        };
        let corners = [
            to_grid(self.rect.min),
            to_grid(self.rect.max),
            to_grid([self.rect.min[0], self.rect.max[1]]),
        ];
        let range = |axis: usize| {
            let lo = corners
                .iter()
                .map(|c| c[axis])
                .fold(f64::INFINITY, f64::min);
            let hi = corners
                .iter()
                .map(|c| c[axis])
                .fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        };
        let (u0, u1) = range(0);
        let (v0, v1) = range(1);
        let mut out = Vec::new();
        // A whisker under the grid line keeps an exact multiple in one cell.
        let first = |lo: f64| (lo + 1e-9).floor() as i64;
        for i in first(u0)..(u1 - 1e-9).ceil() as i64 {
            for j in first(v0)..(v1 - 1e-9).ceil() as i64 {
                let (a0, a1) = ((i as f64).max(u0), ((i + 1) as f64).min(u1));
                let (b0, b1) = ((j as f64).max(v0), ((j + 1) as f64).min(v1));
                if a1 - a0 < 1e-9 || b1 - b0 < 1e-9 {
                    continue;
                }
                let grid = [[a0, b0], [a1, b0], [a1, b1], [a0, b1]];
                let positions = grid.map(from_grid);
                let fractions = grid.map(|g| [g[0] - i as f64, g[1] - j as f64]);
                out.push((positions, fractions));
            }
        }
        out
    }
}

/// The world placement of a plan's runway frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: [f64; 3],
    pub right: [f64; 3],
    pub forward: [f64; 3],
}

impl Frame {
    pub fn world(&self, p: [f64; 2]) -> [f64; 3] {
        std::array::from_fn(|axis| {
            self.origin[axis] + self.right[axis] * p[0] + self.forward[axis] * p[1]
        })
    }
}

/// A plan applied to its theater: what the scene, the AI and the scenery
/// read in place of the retail airfield.
#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    pub layout: String,
    pub strip: String,
    /// The STRIP placement's object id: the runway it redraws.
    pub strip_id: u32,
    pub frame: Frame,
    pub materials: Vec<Material>,
    pub patches: Vec<Patch>,
    /// The landable and contact box: the whole redrawn airfield.
    pub surface: OrientedBox,
    pub anchors: AirfieldAnchors,
    /// Layout placements moved: index in the layout and the new placement.
    moved: Vec<(usize, Placement)>,
    /// Placements added after the layout's own.
    added: Vec<Placement>,
}

impl Built {
    /// The redrawn airport that replaces runway object `id`, if any.
    pub fn for_runway(built: &[Built], id: u32) -> Option<&Built> {
        built.iter().find(|b| b.strip_id == id)
    }
}

fn name_matches(placement: &Placement, name: &str) -> bool {
    placement
        .name
        .as_deref()
        .is_some_and(|n| n.trim_matches(|c: char| c == '\u{1}' || c.is_whitespace()) == name)
}

/// Applies every plan for layout `code` to `sources`: moves and adds the
/// buildings and returns what the scene needs. `height` is the terrain's
/// ground height at a world `x, z`.
pub(super) fn apply(
    resources: &dyn crate::resources::ResourceSource,
    code: &str,
    sources: &mut Placements,
    plans: &[Plan],
    height: impl Fn(f64, f64) -> f64,
) -> WorldResult<Vec<Built>> {
    let mut out = Vec::new();
    let mut added_count = 0u32;
    for plan in plans.iter().filter(|p| p.layout.eq_ignore_ascii_case(code)) {
        let index = sources
            .layout
            .placements
            .iter()
            .position(|p| {
                name_matches(p, &plan.strip)
                    && sources
                        .definitions
                        .get(&p.object_type)
                        .is_some_and(|d| d.callbacks.iter().any(|c| c == "_STRIPProc"))
            })
            .ok_or_else(|| format!("redrawn airport: {code} has no strip {}", plan.strip))?;
        let placement = sources.layout.placements[index].clone();
        let strip_id = crate::surface::UnitId::layout(placement.key.ordinal)
            .ok_or("airport object ID overflow")?
            .0;
        let ground = height(
            f64::from(placement.position[0]),
            f64::from(placement.position[2]),
        );
        let stance = sources
            .stance(&placement, ground)
            .ok_or_else(|| format!("redrawn airport: {} has no runway shape", plan.strip))?;
        let anchor = *sources
            .runway_anchors
            .get(&placement.object_type)
            .ok_or_else(|| format!("redrawn airport: {} has no takeoff anchor", plan.strip))?;
        let length = runway_length_ft(stance.min[2], stance.max[2], Some(anchor[2]));
        if (length - plan.runway_length_ft).abs() > 1. {
            return Err(format!(
                "redrawn airport: {} runway is {length:.0} ft, the plan expects {:.0}",
                plan.strip, plan.runway_length_ft
            )
            .into());
        }
        let basis = stance.basis;
        let frame = Frame {
            origin: std::array::from_fn(|axis| {
                stance.support_origin[axis]
                    + basis.right[axis] * anchor[0]
                    + basis.forward[axis] * anchor[2]
            }),
            right: basis.right,
            forward: basis.forward,
        };
        let patches = plan.patches();
        let landable = plan.landable(&patches);
        let centre = frame.world([
            (landable.min[0] + landable.max[0]) * 0.5,
            (landable.min[1] + landable.max[1]) * 0.5,
        ]);
        let surface = OrientedBox {
            center: [centre[0], stance.support_origin[1], centre[2]],
            half: [
                (landable.max[0] - landable.min[0]) * 0.5,
                1.,
                (landable.max[1] - landable.min[1]) * 0.5,
            ],
            heading: stance.heading,
            pitch: stance.pitch,
            bank: stance.bank,
        };
        let a = &plan.anchors;
        let anchors = AirfieldAnchors {
            taxi_out: a.taxi_out.map(|p| frame.world(p)),
            takeoff_spot: frame.world(a.takeoff),
            takeoff_heading: stance.heading,
            landing_point: frame.world(a.landing),
            landing_heading: stance.heading,
            taxi_in: a.taxi_in.map(|p| frame.world(p)),
            parking: a.parking.map(|p| frame.world(p)),
            parking_heading: (stance.heading + a.parking_heading.to_radians())
                .rem_euclid(std::f64::consts::TAU),
        };
        for p in super::anchor_points(&anchors) {
            if !surface.contains_horizontal(p[0], p[2]) {
                return Err(format!(
                    "redrawn airport: {} anchor off its landable box",
                    plan.strip
                )
                .into());
            }
        }
        let mut moved = Vec::new();
        let mut added = Vec::new();
        // An added building takes the look of the airport's buildings: the
        // first moved one's side and flags, else the strip's side.
        let template = plan
            .buildings
            .iter()
            .filter_map(|b| b.replaces)
            .find_map(|alias| {
                sources
                    .layout
                    .placements
                    .iter()
                    .find(|p| p.alias == Some(alias))
                    .cloned()
            });
        for building in &plan.buildings {
            let at = frame.world(building.at);
            let position = [at[0].round() as i32, 0, at[2].round() as i32];
            let angle = (placement.angles[0] + building.heading + 180).rem_euclid(360) - 180;
            match building.replaces {
                Some(alias) => {
                    let found = sources
                        .layout
                        .placements
                        .iter()
                        .position(|p| p.alias == Some(alias))
                        .ok_or_else(|| {
                            format!("redrawn airport: no placement with alias {alias}")
                        })?;
                    let mut moved_placement = sources.layout.placements[found].clone();
                    if !moved_placement
                        .object_type
                        .eq_ignore_ascii_case(&building.object_type)
                    {
                        return Err(format!(
                            "redrawn airport: alias {alias} is a {}, not a {}",
                            moved_placement.object_type, building.object_type
                        )
                        .into());
                    }
                    moved_placement.position = position;
                    moved_placement.angles = [angle, 0, 0];
                    moved.push((found, moved_placement));
                }
                None => {
                    let base = template.as_ref().unwrap_or(&placement);
                    added.push(Placement {
                        key: SourceKey {
                            layout: placement.key.layout.clone(),
                            ordinal: ADDED_ORDINAL_BASE + added_count,
                        },
                        section: base.section.clone(),
                        object_type: building.object_type.clone(),
                        position,
                        angles: [angle, 0, 0],
                        source_nationality: base.source_nationality,
                        nationality2: base.nationality2,
                        nationality3: base.nationality3,
                        nationality: base.nationality,
                        flags: template.as_ref().and_then(|t| t.flags),
                        speed: Some(0),
                        name: None,
                        alias: None,
                        unknown: Vec::new(),
                    });
                    added_count += 1;
                }
            }
        }
        let built = Built {
            layout: plan.layout.clone(),
            strip: plan.strip.clone(),
            strip_id,
            frame,
            materials: plan.materials.clone(),
            patches,
            surface,
            anchors,
            moved,
            added,
        };
        reapply(resources, std::slice::from_ref(&built), sources)?;
        out.push(built);
    }
    Ok(out)
}

/// Makes `sources` (a fresh load of the same layout) match the built
/// airports' building moves and additions, as the terrain's scene has them.
pub(super) fn reapply(
    resources: &dyn crate::resources::ResourceSource,
    built: &[Built],
    sources: &mut Placements,
) -> WorldResult<()> {
    for airport in built {
        for (index, placement) in &airport.moved {
            sources.layout.placements[*index] = placement.clone();
        }
        let layout = sources.layout.resource.clone();
        for placement in &airport.added {
            sources.add_type(resources, &layout, &placement.object_type)?;
            sources.layout.placements.push(placement.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
