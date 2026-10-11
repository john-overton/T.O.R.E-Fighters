//! The plan files: what one runway shape type (or one pair of tiles) is
//! redrawn as. See docs/formats/redrawn-airports.md for the keys.
use super::data::{Document, Table, Value};
use super::geometry::Point;

/// Which texture axis runs along a runway or taxiway: the columns (`U`) or
/// the rows (`V`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Along {
    U,
    V,
}

/// Markings painted into a runtime copy of a material's texels, in the
/// retail palette (the art lives in the app's `redrawn_pavement.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    /// White runway edge stripes along both sides.
    Edges,
    /// A white centreline dash over the first half of each copy.
    Centre,
    /// White threshold bars across the copy.
    Threshold,
    /// A yellow taxiway centreline and yellow edge lines.
    Taxiway,
    /// The yellow centreline alone: a taxiway across a junction's mouth.
    TaxiwayCentre,
    /// A runway number board without its board: the board's texels become
    /// the runway's own plain asphalt.
    Digit,
}

/// One texture region of a retail PIC and the feet one copy of it covers.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub pic: String,
    /// Texel rectangle `[x, y, width, height]`, rows counted from the top.
    pub rect: [f64; 4],
    /// Feet one copy covers along the texture's columns (`u`) and rows
    /// (`v`). On a runway or taxiway material the across-the-element side
    /// may be 0: it spans the element's width.
    pub tile_ft: [f64; 2],
    pub along: Along,
    pub paint: Vec<Paint>,
    /// Other rectangles of the same size; each copy picks one, mirrored or
    /// not, from a fixed hash of where it lies.
    pub variants: Vec<[f64; 4]>,
}

/// A runway's designators.
#[derive(Clone, Debug, PartialEq)]
pub enum Numbers {
    /// From its world heading: a tenth of the approach heading, rounded,
    /// without a leading zero (36 for north).
    Auto,
    None,
    Fixed([String; 2]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunwaySpec {
    pub name: String,
    /// Near threshold.
    pub from: Point,
    /// Heading against the frame's runway heading, degrees clockwise.
    pub heading: f64,
    pub length: f64,
    pub width: f64,
    /// Paved run before each threshold, feet.
    pub pad: f64,
    /// Touchdown zone texture length after each threshold's numbers, feet.
    pub touchdown: f64,
    pub numbers: Numbers,
    /// Whether each end (near, far) has threshold markings; a runway that
    /// runs on into the next tile leaves that end plain.
    pub marked: [bool; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaxiwaySpec {
    pub name: String,
    pub width: f64,
    /// Centreline points; each leg is straight, at any angle.
    pub points: Vec<Point>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApronSpec {
    pub name: String,
    pub min: Point,
    pub max: Point,
}

/// A building line: the edge buildings back onto, and the way out from the
/// apron (buildings stand on that side of it).
#[derive(Clone, Debug, PartialEq)]
pub struct LineSpec {
    pub from: Point,
    pub to: Point,
    pub out: Point,
}

/// The AI's airfield points (`AirfieldAnchors`), in the runway frame.
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorSpec {
    pub taxi_out: [Point; 4],
    pub takeoff: Point,
    pub landing: Point,
    pub taxi_in: [Point; 4],
    pub parking: [Point; 9],
    /// Parked heading against the runway heading, degrees.
    pub parking_heading: f64,
}

/// Buildings added by rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Extras {
    /// At most this many extra hangars along the building lines.
    pub hangars: u32,
    /// A fuel truck by every third parking slot and a truck on each line.
    pub vehicles: bool,
}

/// The two tile types a pair plan joins.
#[derive(Clone, Debug, PartialEq)]
pub struct PairSpec {
    pub base: String,
    pub tile: String,
    /// Runway ends of the two tiles that run on into the other tile and
    /// lose their threshold markings, as `RUNWAY:near` or `RUNWAY:far`.
    pub unmark: Vec<(String, bool)>,
}

/// A parsed plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The STRIP type it redraws (`STRIP.OT`), or the pair's base type.
    pub applies_to: String,
    pub pair: Option<PairSpec>,
    /// The retail runway length of the type, checked against the shape.
    pub runway_length_ft: Option<f64>,
    /// Whether the first runway lies on the retail ILS line (origin,
    /// heading 0, the retail length). Second tiles' lines are not runways.
    pub ils_runway: bool,
    pub grass_margin_ft: f64,
    pub materials: Vec<Material>,
    pub runways: Vec<RunwaySpec>,
    pub taxiways: Vec<TaxiwaySpec>,
    pub aprons: Vec<ApronSpec>,
    pub lines: Vec<LineSpec>,
    pub anchors: Option<AnchorSpec>,
    pub extras: Extras,
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
fn bool_or(table: &Table, key: &str, what: &str, default: bool) -> Result<bool, String> {
    match table.get(key) {
        None => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(format!("{what}: {key} must be true or false")),
    }
}
fn point(table: &Table, key: &str, what: &str) -> Result<Point, String> {
    field(table, key, what)?
        .numbers::<2>()
        .ok_or_else(|| format!("{what}: {key} must be [x, z]"))
}
fn point_list(table: &Table, key: &str, what: &str) -> Result<Vec<Point>, String> {
    field(table, key, what)?
        .array()
        .ok_or_else(|| format!("{what}: {key} must be a list of points"))?
        .iter()
        .map(|item| {
            item.numbers::<2>()
                .ok_or_else(|| format!("{what}: {key} must hold [x, z] points"))
        })
        .collect()
}
fn points<const N: usize>(table: &Table, key: &str, what: &str) -> Result<[Point; N], String> {
    point_list(table, key, what)?
        .try_into()
        .map_err(|_| format!("{what}: {key} must hold {N} points"))
}
fn rects(table: &Table, key: &str, what: &str) -> Result<Vec<[f64; 4]>, String> {
    match table.get(key) {
        None => Ok(Vec::new()),
        Some(value) => value
            .array()
            .ok_or_else(|| format!("{what}: {key} must be a list"))?
            .iter()
            .map(|item| {
                item.numbers::<4>()
                    .ok_or_else(|| format!("{what}: {key} must hold [x, y, w, h]"))
            })
            .collect(),
    }
}

fn material(t: &Table) -> Result<Material, String> {
    let name = text(t, "name", "material")?;
    let rect = field(t, "rect", &name)?
        .numbers::<4>()
        .ok_or_else(|| format!("{name}: rect must be [x, y, w, h]"))?;
    let tile_ft = point(t, "tile_ft", &name)?;
    if rect[2] < 2. || rect[3] < 2. || tile_ft[0] < 0. || tile_ft[1] < 0. {
        return Err(format!("{name}: empty rect or tile"));
    }
    let along = match t.get("along").and_then(Value::text) {
        None | Some("v") => Along::V,
        Some("u") => Along::U,
        Some(other) => return Err(format!("{name}: along must be u or v, not {other}")),
    };
    let paint = match t.get("paint") {
        None => Vec::new(),
        Some(value) => value
            .array()
            .ok_or_else(|| format!("{name}: paint must be a list"))?
            .iter()
            .map(|item| match item.text() {
                Some("edges") => Ok(Paint::Edges),
                Some("centre") => Ok(Paint::Centre),
                Some("threshold") => Ok(Paint::Threshold),
                Some("taxiway") => Ok(Paint::Taxiway),
                Some("digit") => Ok(Paint::Digit),
                _ => Err(format!("{name}: unknown paint {item:?}")),
            })
            .collect::<Result<_, _>>()?,
    };
    let variants = rects(t, "variants", &name)?;
    if variants.iter().any(|v| v[2] != rect[2] || v[3] != rect[3]) {
        return Err(format!("{name}: variants must be the size of rect"));
    }
    Ok(Material {
        pic: text(t, "pic", &name)?,
        name,
        rect,
        tile_ft,
        along,
        paint,
        variants,
    })
}

/// Parses the shared default materials file.
pub fn default_materials(text_in: &str) -> Result<Vec<Material>, String> {
    let document = Document::parse(text_in)?;
    document
        .arrays
        .get("material")
        .map(|tables| tables.iter().map(material).collect())
        .unwrap_or_else(|| Ok(Vec::new()))
}

impl Plan {
    /// Parses a plan; `defaults` fill in every material it does not name.
    pub fn parse(text_in: &str, defaults: &[Material]) -> Result<Self, String> {
        let document = Document::parse(text_in)?;
        let root = &document.root;
        let empty = Vec::new();
        let array = |name: &str| document.arrays.get(name).unwrap_or(&empty);
        let mut materials = array("material")
            .iter()
            .map(material)
            .collect::<Result<Vec<_>, String>>()?;
        for default in defaults {
            if !materials.iter().any(|m| m.name == default.name) {
                materials.push(default.clone());
            }
        }
        let runways = array("runway")
            .iter()
            .map(|t| {
                let name = text(t, "name", "runway")?;
                let numbers =
                    match t.get("numbers") {
                        None => Numbers::Auto,
                        Some(Value::Bool(false)) => Numbers::None,
                        Some(value) => {
                            let items = value.array().filter(|items| items.len() == 2).ok_or_else(
                                || format!("{name}: numbers must be two strings or false"),
                            )?;
                            let digits = |i: usize| {
                                items[i]
                                    .text()
                                    .filter(|s| {
                                        !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
                                    })
                                    .map(str::to_owned)
                                    .ok_or_else(|| format!("{name}: numbers must be digits"))
                            };
                            Numbers::Fixed([digits(0)?, digits(1)?])
                        }
                    };
                let spec = RunwaySpec {
                    from: point(t, "from", &name)?,
                    heading: number_or(t, "heading", &name, 0.)?,
                    length: number(t, "length", &name)?,
                    width: number(t, "width", &name)?,
                    pad: number_or(t, "pad", &name, 0.)?,
                    touchdown: number_or(t, "touchdown", &name, 1_500.)?,
                    numbers,
                    marked: [
                        bool_or(t, "marked_near", &name, true)?,
                        bool_or(t, "marked_far", &name, true)?,
                    ],
                    name,
                };
                if spec.length < 600. || spec.width < 50. || spec.pad < 0. || spec.touchdown < 0. {
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
                    points: point_list(t, "points", &name)?,
                    name,
                };
                if spec.points.len() < 2
                    || spec.width <= 0.
                    || spec.points.windows(2).any(|w| w[0] == w[1])
                {
                    return Err(format!(
                        "{}: a taxiway needs two distinct points",
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
        let lines = array("line")
            .iter()
            .map(|t| {
                let spec = LineSpec {
                    from: point(t, "from", "line")?,
                    to: point(t, "to", "line")?,
                    out: point(t, "out", "line")?,
                };
                let d = [spec.to[0] - spec.from[0], spec.to[1] - spec.from[1]];
                let out_len = spec.out[0].hypot(spec.out[1]);
                if d[0].hypot(d[1]) < 100. || (out_len - 1.).abs() > 1e-6 {
                    return Err("line: from and to 100 ft apart and out a unit vector".into());
                }
                Ok(spec)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let anchors = match document.tables.get("anchors") {
            None => None,
            Some(a) => Some(AnchorSpec {
                taxi_out: points(a, "taxi_out", "anchors")?,
                takeoff: point(a, "takeoff", "anchors")?,
                landing: point(a, "landing", "anchors")?,
                taxi_in: points(a, "taxi_in", "anchors")?,
                parking: points(a, "parking", "anchors")?,
                parking_heading: number(a, "parking_heading", "anchors")?,
            }),
        };
        let extras = match document.tables.get("extras") {
            None => Extras {
                hangars: 0,
                vehicles: false,
            },
            Some(t) => Extras {
                hangars: number_or(t, "hangars", "extras", 0.)?.max(0.) as u32,
                vehicles: bool_or(t, "vehicles", "extras", false)?,
            },
        };
        let pair = match document.tables.get("pair") {
            None => None,
            Some(t) => Some(PairSpec {
                base: text(t, "base", "pair")?,
                tile: text(t, "tile", "pair")?,
                unmark: match t.get("unmark") {
                    None => Vec::new(),
                    Some(value) => value
                        .array()
                        .ok_or("pair: unmark must be a list")?
                        .iter()
                        .map(|item| match item.text().and_then(|s| s.split_once(':')) {
                            Some((name, "near")) => Ok((name.to_owned(), false)),
                            Some((name, "far")) => Ok((name.to_owned(), true)),
                            _ => Err(format!("pair: bad unmark {item:?}")),
                        })
                        .collect::<Result<_, String>>()?,
                },
            }),
        };
        let plan = Self {
            applies_to: match &pair {
                Some(pair) => pair.base.clone(),
                None => text(root, "applies_to", "plan")?,
            },
            runway_length_ft: if pair.is_some() {
                None
            } else {
                Some(number(root, "runway_length_ft", "plan")?)
            },
            pair,
            ils_runway: bool_or(root, "ils_runway", "plan", true)?,
            grass_margin_ft: number_or(root, "grass_margin_ft", "plan", 0.)?,
            materials,
            runways,
            taxiways,
            aprons,
            lines,
            anchors,
            extras,
        };
        if plan.pair.is_none() {
            let length = plan.runway_length_ft.expect("single plan");
            if plan.ils_runway {
                let first = plan.runways.first().ok_or("a plan needs a runway")?;
                if first.from != [0., 0.] || first.heading != 0. || first.length != length {
                    return Err(
                        "the first runway is the retail one: from [0, 0], heading 0, \
                         length runway_length_ft"
                            .into(),
                    );
                }
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
            for name in [
                "runway_plain",
                "runway_threshold",
                "runway_touchdown",
                "runway_centreline",
            ] {
                if plan.materials[plan.material(name).expect("checked")].along != Along::V {
                    return Err(format!("{name}: runway texture rows run along the runway"));
                }
            }
        }
        Ok(plan)
    }

    pub fn material(&self, name: &str) -> Option<usize> {
        self.materials.iter().position(|m| m.name == name)
    }
}
