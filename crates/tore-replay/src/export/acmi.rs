//! Tacview ACMI 2.2 plain text export (`.txt.acmi`).
//!
//! The game world is flat and has no latitude or longitude, so each theater
//! gets a fixed real-world anchor for its map centre. Positions use Tacview's
//! flat-world transform `T=Lon|Lat|Alt|Roll|Pitch|Yaw|U|V|Heading`: U and V
//! are the game's own east and north coordinates in metres, and Lon/Lat are
//! offsets from the anchor (Tacview adds `ReferenceLongitude` and
//! `ReferenceLatitude`) on a local flat-earth approximation. Angles are
//! degrees. Roll uses Tacview's sign, positive when rolling to the right,
//! which is the game's bank sign (right wing down positive); pitch is
//! positive nose up and yaw clockwise from north in both. Unchanged
//! components and properties are omitted after an object's first line.

use super::text::{Names, describe};
use crate::error::{Result, invalid};
use crate::model::{
    AircraftState, EffectKind, Event, Side, TICKS_PER_SECOND, TreeSample, WeaponClass, device,
};
use crate::reader::Recording;
use crate::vocab::{channel, field, kind, node, outcome};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;

/// A theater's real-world anchor. Provenance: `fitted`. The region comes
/// from the retail theater names (recorded in docs/formats/quick-mission.md
/// and docs/baselines/ukraine-viewer.md); where the map sits inside that
/// region is an agent estimate (2026-09-26), and the game's maps are not
/// claimed to match real terrain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TheaterAnchor {
    /// Base theater code.
    pub code: &'static str,
    /// Region for people.
    pub region: &'static str,
    /// Map centre latitude, degrees north.
    pub latitude: f64,
    /// Map centre longitude, degrees east.
    pub longitude: f64,
    /// Terrain grid size in samples (east, north), 8,192 feet apart, used
    /// to find the map centre when the header has no map size.
    pub grid: [u32; 2],
}

const fn anchor(
    code: &'static str,
    region: &'static str,
    latitude: f64,
    longitude: f64,
    grid: [u32; 2],
) -> TheaterAnchor {
    TheaterAnchor {
        code,
        region,
        latitude,
        longitude,
        grid,
    }
}

/// The 16 base theaters.
pub const THEATER_ANCHORS: [TheaterAnchor; 16] = [
    anchor("APA", "Panama", 9.0, -79.6, [256, 256]),
    anchor("BAL", "The Baltics", 57.0, 24.0, [256, 256]),
    anchor("CUB", "Cuba", 22.0, -79.5, [256, 256]),
    anchor("EGY", "Egypt", 30.0, 32.5, [208, 200]),
    anchor("FRA", "France", 46.5, 2.5, [208, 200]),
    anchor("GRE", "Greece", 38.5, 23.5, [256, 256]),
    anchor("IRA", "Iraq", 33.0, 44.0, [256, 256]),
    anchor("KURILE", "Kuril Islands", 44.5, 146.5, [256, 256]),
    anchor("LFA", "Falkland Islands", -51.7, -59.5, [256, 256]),
    anchor("NSK", "North and South Korea", 38.0, 127.5, [256, 256]),
    anchor("PGU", "Persian Gulf", 27.0, 51.5, [256, 256]),
    anchor("SPA", "Pakistan", 30.0, 71.5, [256, 256]),
    anchor("TVIET", "North Vietnam", 21.0, 105.8, [200, 200]),
    anchor("UKR", "Ukraine", 45.3, 34.0, [208, 200]),
    anchor("VLA", "Vladivostok", 43.1, 132.0, [208, 200]),
    anchor("WTA", "Taiwan", 24.0, 120.5, [256, 256]),
];

/// Used for any theater code outside the table: open ocean at 0 N 0 E, so
/// nothing suggests a real place. Provenance: `unknown`.
pub const UNKNOWN_ANCHOR: [f64; 2] = [0., 0.];

/// Feet between terrain grid samples.
const CELL_FT: f64 = 8_192.;
const M_PER_FT: f64 = 0.3048;
const KG_PER_LB: f64 = 0.453_592_37;
const EARTH_RADIUS_M: f64 = 6_371_000.;

/// The anchor for a theater code. Variant layout names (`~UKR3`, `UKR.MM`)
/// resolve to their base theater.
pub fn theater_anchor(code: &str) -> Option<&'static TheaterAnchor> {
    let code = code.trim().to_ascii_uppercase();
    let code = code.trim_end_matches(".MM").trim_end_matches(".T2");
    let code = code.trim_start_matches(['~', '$']);
    THEATER_ANCHORS.iter().find(|a| a.code == code).or_else(|| {
        THEATER_ANCHORS
            .iter()
            .filter(|a| code.starts_with(a.code))
            .max_by_key(|a| a.code.len())
    })
}

/// ACMI options.
#[derive(Clone, Debug, PartialEq)]
pub struct AcmiOptions {
    /// Position samples per second, up to 120. Default 10.
    pub sample_hz: f64,
    /// `[latitude, longitude]` for the map centre, replacing the table.
    pub anchor: Option<[f64; 2]>,
    /// Include gun rounds. Default false: they are many and small.
    pub guns: bool,
    /// Include debug events (AI decisions, orders, comms reasons, flight
    /// model effect changes). Tacview shows them with `/Debug:on`. Default true.
    pub debug_events: bool,
}

impl Default for AcmiOptions {
    fn default() -> Self {
        Self {
            sample_hz: 10.,
            anchor: None,
            guns: false,
            debug_events: true,
        }
    }
}

/// What was written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcmiStats {
    pub lines: u64,
    pub objects: u64,
    pub events: u64,
}

/// Object id namespaces: ACMI ids are hex, and never 0.
const AIRCRAFT: u64 = 1;
const PROJECTILE: u64 = 2;
const DECOY: u64 = 3;
const PARACHUTE: u64 = 4;

fn object_id(kind: u64, value: u64) -> u64 {
    kind << 40 | value
}

/// Text for a property or event: commas escaped as Tacview requires,
/// line breaks flattened, and separators that could end the value replaced.
fn text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            ',' => out.push_str("\\,"),
            '\n' | '\r' => out.push(' '),
            '\\' | '|' => out.push('/'),
            c => out.push(c),
        }
    }
    out
}

fn fixed(v: f64, decimals: usize) -> String {
    super::text::num(v, decimals)
}

/// Days since 1970-01-01 for a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The UTC reference time: the recording date at the mission's local time
/// of day, shifted by the anchor longitude (15 degrees per hour) so
/// Tacview's sun sits roughly where the game's does.
pub(crate) fn reference_time(recorded_at: &str, time_of_day_s: f64, longitude: f64) -> String {
    let date = recorded_at.get(..10).and_then(|d| {
        let mut parts = d.split('-');
        let y = parts.next()?.parse::<i64>().ok()?;
        let m = parts.next()?.parse::<i64>().ok()?;
        let d = parts.next()?.parse::<i64>().ok()?;
        ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
    });
    let (y, m, d) = date.unwrap_or((2000, 1, 1));
    let local = if time_of_day_s.is_finite() {
        time_of_day_s
    } else {
        43_200.
    };
    let utc = (local - longitude / 15. * 3_600.).round() as i64;
    let days = days_from_civil(y, m, d) + utc.div_euclid(86_400);
    let seconds = utc.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        seconds / 3_600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn side_colors(side: Side) -> (&'static str, &'static str) {
    match side {
        Side::Friendly => ("Allies", "Blue"),
        Side::Enemy => ("Enemies", "Red"),
        Side::Neutral => ("Neutrals", "Green"),
        Side::Unknown => ("Unknown", "Yellow"),
    }
}

/// Where the map centre is and how feet become degrees there.
struct Projection {
    centre: [f64; 2],
    lat_per_ft: f64,
    lon_per_ft: f64,
}

impl Projection {
    fn new(centre: [f64; 2], latitude: f64) -> Self {
        let lat_per_ft = (M_PER_FT / EARTH_RADIUS_M).to_degrees();
        let cos = latitude.to_radians().cos().abs().max(1e-6);
        Self {
            centre,
            lat_per_ft,
            lon_per_ft: lat_per_ft / cos,
        }
    }

    /// Longitude, latitude and altitude components (offsets from the anchor).
    fn place(&self, p: [f64; 3]) -> [String; 3] {
        [
            fixed((p[0] - self.centre[0]) * self.lon_per_ft, 7),
            fixed((p[2] - self.centre[1]) * self.lat_per_ft, 7),
            fixed(p[1] * M_PER_FT, 2),
        ]
    }

    /// The nine-component flat-world transform.
    fn transform(&self, p: [f64; 3], attitude: [f64; 3]) -> Vec<String> {
        let [lon, lat, alt] = self.place(p);
        let yaw = attitude[0].to_degrees().rem_euclid(360.);
        vec![
            lon,
            lat,
            alt,
            fixed(attitude[2].to_degrees(), 2),
            fixed(attitude[1].to_degrees(), 2),
            fixed(yaw, 2),
            fixed(p[0] * M_PER_FT, 2),
            fixed(p[2] * M_PER_FT, 2),
            fixed(yaw, 2),
        ]
    }

    /// The five-component flat-world transform for simple objects.
    fn simple(&self, p: [f64; 3]) -> Vec<String> {
        let [lon, lat, alt] = self.place(p);
        vec![
            lon,
            lat,
            alt,
            fixed(p[0] * M_PER_FT, 2),
            fixed(p[2] * M_PER_FT, 2),
        ]
    }
}

#[derive(Default)]
struct Written {
    transform: Vec<String>,
    properties: BTreeMap<&'static str, String>,
}

struct Acmi<'a, W: Write> {
    out: W,
    stats: AcmiStats,
    time: Option<u64>,
    objects: HashMap<u64, Written>,
    names: Names<'a>,
}

impl<W: Write> Acmi<'_, W> {
    fn line(&mut self, line: &str) -> Result<()> {
        self.out.write_all(line.as_bytes())?;
        self.out.write_all(b"\n")?;
        self.stats.lines += 1;
        Ok(())
    }

    fn at(&mut self, tick: u64) -> Result<()> {
        if self.time != Some(tick) {
            self.time = Some(tick);
            let seconds = tick as f64 / TICKS_PER_SECOND as f64;
            self.line(&format!("#{}", fixed(seconds, 3)))?;
        }
        Ok(())
    }

    /// Writes an object's line with only what changed since its last line.
    fn object(
        &mut self,
        tick: u64,
        id: u64,
        transform: Vec<String>,
        properties: Vec<(&'static str, String)>,
    ) -> Result<()> {
        let fresh = !self.objects.contains_key(&id);
        if fresh {
            self.stats.objects += 1;
        }
        let written = self.objects.entry(id).or_default();
        let mut parts = Vec::new();
        let changed: Vec<String> = transform
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if written.transform.get(i) == Some(c) {
                    String::new()
                } else {
                    c.clone()
                }
            })
            .collect();
        if changed.iter().any(|c| !c.is_empty()) {
            parts.push(format!("T={}", changed.join("|")));
        }
        written.transform = transform;
        for (key, value) in properties {
            if written.properties.get(key) != Some(&value) {
                parts.push(format!("{key}={}", text(&value)));
                written.properties.insert(key, value);
            }
        }
        if parts.is_empty() {
            return Ok(());
        }
        self.at(tick)?;
        self.line(&format!("{id:x},{}", parts.join(",")))
    }

    fn remove(&mut self, tick: u64, id: u64) -> Result<()> {
        if self.objects.remove(&id).is_some() {
            self.at(tick)?;
            self.line(&format!("-{id:x}"))?;
        }
        Ok(())
    }

    fn event(&mut self, tick: u64, kind_name: &str, objects: &[u64], message: &str) -> Result<()> {
        self.at(tick)?;
        let mut parts = vec![kind_name.to_owned()];
        parts.extend(objects.iter().map(|id| format!("{id:x}")));
        parts.push(text(message));
        self.stats.events += 1;
        self.line(&format!("0,Event={}", parts.join("|")))
    }
}

/// Numbers from the latest telemetry tree, converted to Tacview's units.
fn telemetry(tree: Option<&TreeSample>) -> Vec<(&'static str, String)> {
    let Some(tree) = tree else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let value = |label: &str| {
        tree.node(label)
            .and_then(|n| n.value.as_f64().map(|v| (v, n.unit.as_str())))
            .filter(|(v, _)| v.is_finite())
    };
    if let Some((mach, _)) = value(node::MACH) {
        out.push(("Mach", fixed(mach, 3)));
    }
    if let Some((aoa, _)) = value(node::AOA) {
        out.push(("AOA", fixed(aoa, 1)));
    }
    if let Some((aos, _)) = value(node::SIDESLIP) {
        out.push(("AOS", fixed(aos, 1)));
    }
    if let Some((agl, unit_name)) = value(node::AGL) {
        let metres = if unit_name == "m" {
            agl
        } else {
            agl * M_PER_FT
        };
        out.push(("AGL", fixed(metres, 1)));
    }
    out
}

fn aircraft_properties(
    s: &AircraftState,
    tree: Option<&TreeSample>,
) -> Vec<(&'static str, String)> {
    let mut props = vec![
        ("TAS", fixed(s.airspeed * M_PER_FT, 1)),
        ("HDG", fixed(s.attitude[0].to_degrees().rem_euclid(360.), 1)),
        ("Throttle", fixed(s.controls[3], 2)),
        ("Afterburner", u8::from(s.flags.afterburner).to_string()),
        ("LandingGear", fixed(s.devices[device::GEAR], 2)),
        ("Flaps", fixed(s.devices[device::FLAPS], 2)),
        ("AirBrakes", fixed(s.devices[device::BRAKE], 2)),
        ("Tailhook", fixed(s.devices[device::HOOK], 2)),
        ("FuelWeight", fixed(s.fuel_lb * KG_PER_LB, 0)),
        ("VerticalGForce", fixed(s.g, 2)),
    ];
    props.extend(telemetry(tree));
    props.retain(|(_, v)| !matches!(v.as_str(), "NaN" | "infinity" | "-infinity"));
    props
}

fn debug_event(kind_name: &str) -> bool {
    kind_name.starts_with("ai.")
        || matches!(
            kind_name,
            kind::COMMS_ORDER
                | kind::COMMS_REQUEST
                | kind::COMMS_REPORT
                | kind::COMMS_DELIVERY
                | kind::FLIGHT_EFFECT
                | kind::FLIGHT_DEPARTURE
                | kind::FLIGHT_STALL
                | kind::FLIGHT_SPIN
                | kind::FLIGHT_G_LIMIT
                | kind::FLIGHT_STRUCTURAL_FAILURE
        )
}

/// Writes the Tacview file.
pub fn write_acmi(
    recording: &Recording,
    options: &AcmiOptions,
    out: impl Write,
) -> Result<AcmiStats> {
    if !(options.sample_hz.is_finite() && options.sample_hz > 0. && options.sample_hz <= 120.) {
        return Err(invalid(format!(
            "samples per second must be above 0 and at most 120, not {}",
            options.sample_hz
        )));
    }
    let header = recording.header();
    let world = &header.world;
    let table = theater_anchor(&world.theater).or_else(|| theater_anchor(&world.layout));
    let [latitude, longitude] = options
        .anchor
        .or(table.map(|a| [a.latitude, a.longitude]))
        .unwrap_or(UNKNOWN_ANCHOR);
    let centre = match (world.extent_ft, table) {
        (Some([east, north]), _) if east.is_finite() && north.is_finite() => {
            [east / 2., north / 2.]
        }
        (_, Some(a)) => [
            f64::from(a.grid[0] - 1) * CELL_FT / 2.,
            f64::from(a.grid[1] - 1) * CELL_FT / 2.,
        ],
        _ => [0., 0.],
    };
    let projection = Projection::new(centre, latitude);
    let mut acmi = Acmi {
        out,
        stats: AcmiStats::default(),
        time: None,
        objects: HashMap::new(),
        names: Names::new(recording),
    };
    acmi.line("FileType=text/acmi/tacview")?;
    acmi.line("FileVersion=2.2")?;
    acmi.line(&format!(
        "0,ReferenceTime={}",
        reference_time(&header.recorded_at, world.time_of_day_s, longitude)
    ))?;
    if header.recorded_at.len() >= 20 && header.recorded_at.ends_with('Z') {
        acmi.line(&format!("0,RecordingTime={}", text(&header.recorded_at)))?;
    }
    acmi.line(&format!("0,ReferenceLongitude={}", fixed(longitude, 7)))?;
    acmi.line(&format!("0,ReferenceLatitude={}", fixed(latitude, 7)))?;
    let game = if header.game_version.is_empty() {
        "T.O.R.E-Fighters".to_owned()
    } else {
        format!("T.O.R.E-Fighters {}", header.game_version)
    };
    acmi.line(&format!("0,DataSource={}", text(&game)))?;
    acmi.line(&format!(
        "0,DataRecorder={}",
        text(&format!("tore-replay format {}", header.format_version))
    ))?;
    let place = if world.theater_name.is_empty() {
        world.theater.clone()
    } else {
        world.theater_name.clone()
    };
    acmi.line(&format!(
        "0,Title={}",
        text(&format!("{} in {place}", header.mission.title()))
    ))?;
    let where_from = match (options.anchor, table) {
        (Some(_), _) => "an anchor chosen for this export".to_owned(),
        (None, Some(a)) => format!("a fitted anchor near {}", a.region),
        (None, None) => "an unknown theater, placed at 0 N 0 E".to_owned(),
    };
    acmi.line(&format!(
        "0,Comments={}",
        text(&format!(
            "Game map centred on {where_from}. Tacview draws real-world terrain, which will not match the game's maps."
        ))
    ))?;

    let first = recording.first_tick().unwrap_or(0);
    let step = (TICKS_PER_SECOND as f64 / options.sample_hz)
        .round()
        .max(1.) as u64;
    let mut latest_telemetry: HashMap<u32, TreeSample> = HashMap::new();
    let mut decoys: Vec<(u64, u64)> = Vec::new();
    let mut next_decoy = 1u64;
    let mut seen_aircraft: HashSet<u32> = HashSet::new();
    for frame in recording.frames(0, u64::MAX) {
        let frame = frame?;
        let tick = frame.tick;
        let sample = (tick - first).is_multiple_of(step);
        for tree in &frame.trees {
            if tree.channel == channel::FLIGHT_TELEMETRY {
                latest_telemetry.insert(tree.subject, tree.clone());
            }
        }
        let mut present = HashSet::new();
        for s in &frame.aircraft {
            let id = object_id(AIRCRAFT, u64::from(s.id));
            if s.flags.wreck_gone {
                continue;
            }
            present.insert(id);
            let fresh = !acmi.objects.contains_key(&id);
            if !(fresh || sample) {
                continue;
            }
            let mut props = Vec::new();
            if fresh {
                seen_aircraft.insert(s.id);
                let info = recording.aircraft_info(s.id);
                props.push(("Type", "Air+FixedWing".to_owned()));
                let name = info
                    .map(|i| {
                        if i.name.is_empty() {
                            i.pt.clone()
                        } else {
                            i.name.clone()
                        }
                    })
                    .unwrap_or_else(|| format!("Aircraft {}", s.id));
                props.push(("Name", name));
                if let Some(info) = info {
                    if !info.label.is_empty() {
                        props.push(("Pilot", info.label.clone()));
                        props.push(("CallSign", info.label.clone()));
                    }
                    if info.wing > 0 {
                        let side = match info.side {
                            Side::Friendly => "Friendly",
                            Side::Enemy => "Enemy",
                            Side::Neutral => "Neutral",
                            Side::Unknown => "Unknown",
                        };
                        props.push(("Group", format!("{side} wing {}", info.wing)));
                    }
                    let (coalition, color) = side_colors(info.side);
                    props.push(("Coalition", coalition.to_owned()));
                    props.push(("Color", color.to_owned()));
                }
            }
            props.extend(aircraft_properties(s, latest_telemetry.get(&s.id)));
            acmi.object(
                tick,
                id,
                projection.transform(s.position, s.attitude),
                props,
            )?;
        }
        for p in &frame.projectiles {
            let weapon = recording.weapon_info(p.weapon);
            let class = weapon.map_or(WeaponClass::Other, |w| w.class);
            if class == WeaponClass::Gun && !options.guns {
                continue;
            }
            let id = object_id(PROJECTILE, u64::from(p.id));
            present.insert(id);
            let fresh = !acmi.objects.contains_key(&id);
            if !(fresh || sample) {
                continue;
            }
            let mut props = Vec::new();
            if fresh {
                let kind_tags = match class {
                    WeaponClass::Missile => "Weapon+Missile",
                    WeaponClass::Bomb => "Weapon+Bomb",
                    WeaponClass::Rocket => "Weapon+Rocket",
                    WeaponClass::Gun => "Projectile+Bullet",
                    WeaponClass::Other => "Weapon",
                };
                props.push(("Type", kind_tags.to_owned()));
                props.push((
                    "Name",
                    weapon.map_or_else(|| format!("Weapon {}", p.weapon), |w| w.name.clone()),
                ));
                if seen_aircraft.contains(&p.owner) {
                    props.push((
                        "Parent",
                        format!("{:x}", object_id(AIRCRAFT, u64::from(p.owner))),
                    ));
                }
                if let Some(info) = recording.aircraft_info(p.owner) {
                    let (coalition, color) = side_colors(info.side);
                    props.push(("Coalition", coalition.to_owned()));
                    props.push(("Color", color.to_owned()));
                }
            }
            let d = p.direction;
            let attitude = [d[0].atan2(d[2]), d[1].atan2(d[0].hypot(d[2])), 0.];
            if attitude.iter().chain(&p.position).any(|v| !v.is_finite()) {
                continue;
            }
            props.push(("TAS", fixed(p.speed * M_PER_FT, 1)));
            acmi.object(tick, id, projection.transform(p.position, attitude), props)?;
        }
        let mut occurrence: HashMap<u32, u64> = HashMap::new();
        for e in &frame.escapees {
            let n = occurrence.entry(e.owner).or_insert(0);
            let id = object_id(PARACHUTE, u64::from(e.owner) << 8 | (*n & 0xff));
            *n += 1;
            present.insert(id);
            let fresh = !acmi.objects.contains_key(&id);
            if !(fresh || sample) || e.position.iter().any(|v| !v.is_finite()) {
                continue;
            }
            let mut props = Vec::new();
            if fresh {
                props.push(("Type", "Ground+Light+Human+Air+Parachutist".to_owned()));
                props.push(("Name", format!("Pilot of {}", acmi.names.who(e.owner))));
                if seen_aircraft.contains(&e.owner) {
                    props.push((
                        "Parent",
                        format!("{:x}", object_id(AIRCRAFT, u64::from(e.owner))),
                    ));
                }
            }
            acmi.object(
                tick,
                id,
                projection.transform(e.position, [e.heading, 0., 0.]),
                props,
            )?;
        }
        for effect in &frame.new_effects {
            let tags = match effect.kind {
                EffectKind::Flare => "Misc+Decoy+Flare",
                EffectKind::Chaff => "Misc+Decoy+Chaff",
                _ => continue,
            };
            if effect.position.iter().any(|v| !v.is_finite()) {
                continue;
            }
            let id = object_id(DECOY, next_decoy);
            next_decoy += 1;
            let name = if effect.kind == EffectKind::Flare {
                "Flare"
            } else {
                "Chaff"
            };
            acmi.object(
                tick,
                id,
                projection.simple(effect.position),
                vec![("Type", tags.to_owned()), ("Name", name.to_owned())],
            )?;
            decoys.push((tick + u64::from(effect.duration_ticks.max(1)), id));
        }
        let gone: Vec<u64> = acmi
            .objects
            .keys()
            .filter(|id| *id >> 40 != DECOY && !present.contains(id))
            .copied()
            .collect();
        let mut gone = gone;
        gone.sort_unstable();
        for id in gone {
            acmi.remove(tick, id)?;
        }
        let expired: Vec<u64> = decoys
            .iter()
            .filter(|(until, _)| *until <= tick)
            .map(|(_, id)| *id)
            .collect();
        decoys.retain(|(until, _)| *until > tick);
        for id in expired {
            acmi.remove(tick, id)?;
        }
        for e in &frame.events {
            write_event(&mut acmi, tick, e, options, &seen_aircraft)?;
        }
    }
    let Acmi { mut out, stats, .. } = acmi;
    out.flush()?;
    Ok(stats)
}

fn write_event<W: Write>(
    acmi: &mut Acmi<W>,
    tick: u64,
    e: &Event,
    options: &AcmiOptions,
    seen: &HashSet<u32>,
) -> Result<()> {
    let aircraft = |id: Option<u32>| {
        id.filter(|id| seen.contains(id))
            .map(|id| object_id(AIRCRAFT, u64::from(id)))
    };
    let subject = aircraft(e.subject);
    let line = describe(e, &acmi.names);
    match e.kind.as_str() {
        kind::COMBAT_DESTROYED => match subject {
            Some(id) => acmi.event(tick, "Destroyed", &[id], &line),
            None => acmi.event(tick, "Message", &[], &line),
        },
        // A line the player heard, once: its delivery, not its queuing or
        // a later cut-off.
        kind::COMMS_RADIO | kind::COMMS_CREW | kind::COMMS_TOWER
            if e.flag(field::HEARD) == Some(true)
                && e.string(field::OUTCOME)
                    .is_none_or(|o| o == outcome::DELIVERED) =>
        {
            let speaker = e
                .string(field::SPEAKER)
                .map(str::to_owned)
                .or_else(|| e.subject.map(|s| acmi.names.who(s)))
                .unwrap_or_default();
            let message = if e.text.is_empty() {
                super::text::comms_text(e, &acmi.names)
            } else {
                format!("{speaker}: {}", e.text)
            };
            acmi.event(
                tick,
                "Message",
                &subject.into_iter().collect::<Vec<_>>(),
                &message,
            )
        }
        kind::PLAYER_BOOKMARK => {
            let note = if e.text.is_empty() {
                "Bookmark"
            } else {
                &e.text
            };
            acmi.event(tick, "Bookmark", &[], note)
        }
        kind::AIRCRAFT_TOOK_OFF => match subject {
            Some(id) => acmi.event(tick, "TakenOff", &[id], &line),
            None => Ok(()),
        },
        kind::AIRCRAFT_LANDED => match subject {
            Some(id) => acmi.event(tick, "Landed", &[id], &line),
            None => Ok(()),
        },
        k if options.debug_events
            && (debug_event(k)
                || (k.starts_with("comms.")
                    && (e.get(field::REASON).is_some() || e.get(field::OUTCOME).is_some()))) =>
        {
            acmi.event(
                tick,
                "Debug",
                &subject.into_iter().collect::<Vec<_>>(),
                &line,
            )
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_base_theater_has_an_anchor_and_variants_resolve() {
        let codes = [
            "APA", "BAL", "CUB", "EGY", "FRA", "GRE", "IRA", "KURILE", "LFA", "NSK", "PGU", "SPA",
            "TVIET", "UKR", "VLA", "WTA",
        ];
        for code in codes {
            let a = theater_anchor(code).unwrap();
            assert_eq!(a.code, code);
            assert!((-90. ..=90.).contains(&a.latitude) && (-180. ..=180.).contains(&a.longitude));
        }
        assert_eq!(theater_anchor("~UKR3.MM").unwrap().code, "UKR");
        assert_eq!(theater_anchor("ukr").unwrap().code, "UKR");
        assert_eq!(theater_anchor("KURILE.MM").unwrap().code, "KURILE");
        assert!(theater_anchor("XYZ").is_none());
    }

    #[test]
    fn reference_time_follows_the_sun_across_midnight() {
        assert_eq!(
            reference_time("2026-09-26T15:40:00Z", 43_200., 34.),
            "2026-09-26T09:44:00Z"
        );
        // Early morning far east rolls back to the previous day.
        assert_eq!(
            reference_time("2026-03-01T00:00:00Z", 3_600., 146.5),
            "2026-02-28T15:14:00Z"
        );
        // Late evening far west rolls on to the next day.
        assert_eq!(
            reference_time("2026-12-31T23:00:00Z", 82_800., -79.5),
            "2027-01-01T04:18:00Z"
        );
        assert_eq!(reference_time("garbage", 0., 0.), "2000-01-01T00:00:00Z");
        for days in [-1_000_000, -1, 0, 1, 20_000, 1_000_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    #[test]
    fn text_escapes_commas_and_flattens_lines() {
        assert_eq!(text("a,b\nc|d\\e"), "a\\,b c/d/e");
    }
}
