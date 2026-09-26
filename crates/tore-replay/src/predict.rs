//! Per-entity records: an exact key record, or a change record predicted from
//! the values the reader holds.
//!
//! Every quantized number is stored as whole steps from the entity's key
//! value (the exact value in its last key record), so both sides compute the
//! same integers and rounding never accumulates: the reader's value is always
//! within half a step of the truth. Smooth quantities use second-order
//! prediction (the last step repeats), controls and devices first-order (no
//! change). A change record holds a bit mask of the groups whose residuals
//! are not all zero, then those residuals.

use crate::codec::{In, put_iv, put_opt_id, put_pair, put_triple, put_uv, put_xf64};
use crate::error::{Result, corrupt};
use crate::model::{
    AircraftFlags, AircraftState, DEVICE_COUNT, DebrisState, EscapeeState, ProjectileState,
    SECTION_COUNT, Seeker, TICKS_PER_SECOND, device,
};
use std::array::from_fn;
use std::f64::consts::{PI, TAU};

/// Quantization steps. The largest error of a quantized value is half its
/// step; key records (every chunk's first frame, and an entity's first
/// frame) are exact.
pub mod precision {
    use std::f64::consts::TAU;
    /// Positions of aircraft, projectiles, debris, ejected pilots, effects
    /// and puffs, in feet.
    pub const POSITION_FT: f64 = 1. / 32.;
    /// Attitude, headings and projectile direction, in radians: 2^-20 of a
    /// turn, about 0.00034 degrees.
    pub const ANGLE_RAD: f64 = TAU / 1_048_576.;
    /// Aircraft velocity, feet per second.
    pub const VELOCITY_FPS: f64 = 1. / 64.;
    /// Airspeed, the speed device and projectile speed, feet per second.
    pub const SPEED_FPS: f64 = 1. / 64.;
    /// Load factor, G.
    pub const G: f64 = 1. / 1024.;
    /// Devices that run from 0 to 1, and engine heat: one byte of range.
    pub const UNIT: f64 = 1. / 255.;
    /// Devices that run from -1 to 1: a signed byte of range.
    pub const SIGNED: f64 = 1. / 127.;
    /// Pilot controls.
    pub const CONTROL: f64 = 1. / 1024.;
    /// Fuel, pounds.
    pub const FUEL_LB: f64 = 1. / 16.;
}

use precision::{
    ANGLE_RAD as ANGLE, CONTROL, FUEL_LB as FUEL, G, POSITION_FT as POS, SIGNED,
    SPEED_FPS as SPEED, UNIT, VELOCITY_FPS as VEL,
};

/// Beyond these magnitudes a value is stored exactly in a key record.
const POSITION_LIMIT: f64 = 1e9;
const VELOCITY_LIMIT: f64 = 1e6;
const SCALAR_LIMIT: f64 = 1e8;
const ANGLE_LIMIT: f64 = 1e6;
/// A projectile direction this far from unit length is stored exactly.
const UNIT_TOLERANCE: f64 = 1e-6;

fn within(v: f64, limit: f64) -> bool {
    v.is_finite() && v.abs() <= limit
}

/// Nearest whole number of steps; NaN becomes 0 and infinities saturate.
fn units(x: f64) -> i64 {
    let r = x.round();
    if r.is_nan() { 0 } else { r as i64 }
}

/// Angle difference folded into -pi..pi.
pub(crate) fn wrap_pi(a: f64) -> f64 {
    (a + PI).rem_euclid(TAU) - PI
}

/// One quantized number: the exact key value, whole steps from it, and the
/// last change in steps (for second-order prediction).
#[derive(Clone, Copy, Debug, Default)]
struct Chan {
    base: f64,
    u: i64,
    d: i64,
}

impl Chan {
    fn key(base: f64) -> Self {
        Self { base, u: 0, d: 0 }
    }

    fn seeded(base: f64, d: i64) -> Self {
        Self { base, u: 0, d }
    }

    fn p1(&self) -> i64 {
        self.u
    }

    fn p2(&self) -> i64 {
        self.u.wrapping_add(self.d)
    }

    fn set(&mut self, u: i64) {
        self.d = u.wrapping_sub(self.u);
        self.u = u;
    }

    /// The reader's value. Unchanged since the key record means exact.
    fn value(&self, step: f64) -> f64 {
        if self.u == 0 {
            self.base
        } else {
            self.base + self.u as f64 * step
        }
    }

    fn bounded(&self, step: f64, lo: f64, hi: f64) -> f64 {
        if self.u == 0 {
            self.base
        } else {
            (self.base + self.u as f64 * step).clamp(lo, hi)
        }
    }

    /// Yaw-like angles read back in 0..2pi.
    fn yaw(&self) -> f64 {
        let v = self.value(ANGLE);
        if self.u == 0 || (0. ..TAU).contains(&v) {
            v
        } else {
            v.rem_euclid(TAU)
        }
    }

    /// Pitch and bank read back in -pi..pi.
    fn signed_angle(&self) -> f64 {
        let v = self.value(ANGLE);
        if self.u == 0 || (-PI..=PI).contains(&v) {
            v
        } else {
            wrap_pi(v)
        }
    }

    /// Writer: quantizes `v` and returns its residual from `pred`.
    fn code(&mut self, v: f64, step: f64, pred: i64) -> i64 {
        let target = units((v - self.base) / step);
        self.set(target);
        target.wrapping_sub(pred)
    }

    /// Writer, for angles: the residual is the wrapped difference, so a turn
    /// through north costs a small step, not a whole turn.
    fn code_angle(&mut self, v: f64, pred: i64) -> i64 {
        let predicted = self.base + pred as f64 * ANGLE;
        let r = units(wrap_pi(v - predicted) / ANGLE);
        self.set(pred.wrapping_add(r));
        r
    }

    /// Reader.
    fn apply(&mut self, pred: i64, r: i64) {
        self.set(pred.wrapping_add(r));
    }
}

/// Position steps a key record's velocity predicts for the next tick.
fn seed(velocity: f64) -> i64 {
    units(velocity / TICKS_PER_SECOND as f64 / POS)
}

#[derive(Clone, Copy)]
enum Slot {
    Unit,
    Signed,
    Speed,
}

const SLOTS: [Slot; DEVICE_COUNT] = [
    Slot::Unit,
    Slot::Unit,
    Slot::Unit,
    Slot::Unit,
    Slot::Unit,
    Slot::Unit,
    Slot::Signed,
    Slot::Signed,
    Slot::Signed,
    Slot::Speed,
    Slot::Unit,
];

impl Slot {
    fn step(self) -> f64 {
        match self {
            Self::Unit => UNIT,
            Self::Signed => SIGNED,
            Self::Speed => SPEED,
        }
    }

    fn encodable(self, v: f64) -> bool {
        match self {
            Self::Unit | Self::Signed => v.is_finite(),
            Self::Speed => within(v, SCALAR_LIMIT),
        }
    }

    /// Render inputs are stored within their range.
    fn limit(self, v: f64) -> f64 {
        match self {
            Self::Unit => v.clamp(0., 1.),
            Self::Signed => v.clamp(-1., 1.),
            Self::Speed => v,
        }
    }

    fn out(self, c: &Chan) -> f64 {
        match self {
            Self::Unit => c.bounded(UNIT, 0., 1.),
            Self::Signed => c.bounded(SIGNED, -1., 1.),
            Self::Speed => c.value(SPEED),
        }
    }
}

/// Device mask bit `i` carries slot `DEVICE_ORDER[i]`: the busiest slots
/// first, so a typical mask fits in one byte.
const DEVICE_ORDER: [usize; DEVICE_COUNT] = [
    device::ELEVATOR,
    device::AILERON,
    device::RUDDER,
    device::SPEED,
    device::THROTTLE,
    device::EXHAUST,
    device::BRAKE,
    device::GEAR,
    device::FLAPS,
    device::HOOK,
    device::BAY,
];

const A_POS: u64 = 1;
const A_ATT: u64 = 1 << 1;
const A_VEL: u64 = 1 << 2;
const A_AIRSPEED: u64 = 1 << 3;
const A_G: u64 = 1 << 4;
const A_DEVICES: u64 = 1 << 5;
const A_CONTROLS: u64 = 1 << 6;
const A_FUEL: u64 = 1 << 7;
const A_HEAT: u64 = 1 << 8;
const A_FLAGS: u64 = 1 << 9;
const A_WRECK: u64 = 1 << 10;
const A_DAMAGE: u64 = 1 << 11;
const A_KEY: u64 = 1 << 12;
const A_ALL: u64 = A_KEY - 1;

fn aircraft_encodable(s: &AircraftState) -> bool {
    s.position.iter().all(|v| within(*v, POSITION_LIMIT))
        && s.attitude.iter().all(|v| within(*v, ANGLE_LIMIT))
        && s.velocity.iter().all(|v| within(*v, VELOCITY_LIMIT))
        && within(s.airspeed, SCALAR_LIMIT)
        && within(s.g, SCALAR_LIMIT)
        && within(s.fuel_lb, SCALAR_LIMIT)
        && s.heat.is_finite()
        && s.devices
            .iter()
            .zip(SLOTS)
            .all(|(v, slot)| slot.encodable(*v))
        && s.controls.iter().all(|v| within(*v, SCALAR_LIMIT))
}

/// What the reader knows about one aircraft after its latest record.
#[derive(Clone, Debug)]
pub(crate) struct AircraftPred {
    pos: [Chan; 3],
    att: [Chan; 3],
    vel: [Chan; 3],
    airspeed: Chan,
    g: Chan,
    devices: [Chan; DEVICE_COUNT],
    heat: Chan,
    fuel: Chan,
    controls: [Chan; 4],
    flags: u16,
    wreck: u8,
    hp: i32,
    max_hp: i32,
    sections: [i32; SECTION_COUNT],
    structural: Option<u8>,
    predictable: bool,
}

impl AircraftPred {
    fn key(s: &AircraftState) -> Self {
        Self {
            pos: from_fn(|i| Chan::seeded(s.position[i], seed(s.velocity[i]))),
            att: s.attitude.map(Chan::key),
            vel: s.velocity.map(Chan::key),
            airspeed: Chan::key(s.airspeed),
            g: Chan::key(s.g),
            devices: s.devices.map(Chan::key),
            heat: Chan::key(s.heat),
            fuel: Chan::key(s.fuel_lb),
            controls: s.controls.map(Chan::key),
            flags: s.flags.bits(),
            wreck: s.wreck_phase,
            hp: s.hp,
            max_hp: s.max_hp,
            sections: s.sections,
            structural: s.structural_section,
            predictable: aircraft_encodable(s),
        }
    }

    pub fn state(&self, id: u32) -> AircraftState {
        AircraftState {
            id,
            position: from_fn(|i| self.pos[i].value(POS)),
            attitude: [
                self.att[0].yaw(),
                self.att[1].signed_angle(),
                self.att[2].signed_angle(),
            ],
            velocity: from_fn(|i| self.vel[i].value(VEL)),
            airspeed: self.airspeed.value(SPEED),
            g: self.g.value(G),
            devices: from_fn(|slot| SLOTS[slot].out(&self.devices[slot])),
            heat: self.heat.bounded(UNIT, 0., 1.),
            flags: AircraftFlags::from_bits(self.flags),
            wreck_phase: self.wreck,
            fuel_lb: self.fuel.value(FUEL),
            controls: from_fn(|i| self.controls[i].value(CONTROL)),
            hp: self.hp,
            max_hp: self.max_hp,
            sections: self.sections,
            structural_section: self.structural,
        }
    }
}

fn put_opt_u8(buf: &mut Vec<u8>, v: Option<u8>) {
    put_uv(buf, v.map_or(0, |v| u64::from(v) + 1));
}

fn get_opt_u8(input: &mut In) -> Result<Option<u8>> {
    match input.uv()? {
        0 => Ok(None),
        n => u8::try_from(n - 1)
            .map(Some)
            .map_err(|_| corrupt("a section number is out of range")),
    }
}

fn get_i32(input: &mut In) -> Result<i32> {
    i32::try_from(input.iv()?).map_err(|_| corrupt("a damage value is out of range"))
}

fn add_i32(old: i32, delta: i64) -> Result<i32> {
    i32::try_from(i64::from(old).wrapping_add(delta))
        .map_err(|_| corrupt("a damage value is out of range"))
}

fn put_aircraft_key(buf: &mut Vec<u8>, s: &AircraftState) {
    let floats = s
        .position
        .iter()
        .chain(&s.attitude)
        .chain(&s.velocity)
        .chain([&s.airspeed, &s.g])
        .chain(&s.devices)
        .chain([&s.heat, &s.fuel_lb])
        .chain(&s.controls);
    for v in floats {
        put_xf64(buf, *v);
    }
    put_uv(buf, u64::from(s.flags.bits()));
    buf.push(s.wreck_phase);
    put_iv(buf, i64::from(s.hp));
    put_iv(buf, i64::from(s.max_hp));
    for v in s.sections {
        put_iv(buf, i64::from(v));
    }
    put_opt_u8(buf, s.structural_section);
}

fn get_aircraft_key(input: &mut In) -> Result<AircraftState> {
    let mut f = || input.xf64();
    let position = [f()?, f()?, f()?];
    let attitude = [f()?, f()?, f()?];
    let velocity = [f()?, f()?, f()?];
    let airspeed = f()?;
    let g = f()?;
    let mut devices = [0.; DEVICE_COUNT];
    for v in &mut devices {
        *v = f()?;
    }
    let heat = f()?;
    let fuel_lb = f()?;
    let controls = [f()?, f()?, f()?, f()?];
    let flags = AircraftFlags::from_bits(
        u16::try_from(input.uv()?).map_err(|_| corrupt("aircraft flags are out of range"))?,
    );
    let wreck_phase = input.u8()?;
    let hp = get_i32(input)?;
    let max_hp = get_i32(input)?;
    let mut sections = [0; SECTION_COUNT];
    for v in &mut sections {
        *v = get_i32(input)?;
    }
    let structural_section = get_opt_u8(input)?;
    Ok(AircraftState {
        id: 0,
        position,
        attitude,
        velocity,
        airspeed,
        g,
        devices,
        heat,
        flags,
        wreck_phase,
        fuel_lb,
        controls,
        hp,
        max_hp,
        sections,
        structural_section,
    })
}

/// Writes one aircraft record and returns the state the reader will hold.
pub(crate) fn put_aircraft(
    buf: &mut Vec<u8>,
    old: Option<AircraftPred>,
    s: &AircraftState,
) -> AircraftPred {
    let Some(mut p) = old.filter(|p| p.predictable && aircraft_encodable(s)) else {
        put_uv(buf, A_KEY);
        put_aircraft_key(buf, s);
        return AircraftPred::key(s);
    };
    let pos: [i64; 3] = from_fn(|i| {
        let pred = p.pos[i].p2();
        p.pos[i].code(s.position[i], POS, pred)
    });
    let att: [i64; 3] = from_fn(|i| {
        let pred = p.att[i].p2();
        p.att[i].code_angle(s.attitude[i], pred)
    });
    let vel: [i64; 3] = from_fn(|i| {
        let pred = p.vel[i].p2();
        p.vel[i].code(s.velocity[i], VEL, pred)
    });
    let airspeed_before = p.airspeed.u;
    let pred = p.airspeed.p2();
    let airspeed = p.airspeed.code(s.airspeed, SPEED, pred);
    let airspeed_step = p.airspeed.u.wrapping_sub(airspeed_before);
    let pred = p.g.p1();
    let g = p.g.code(s.g, G, pred);
    let mut devices = [0i64; DEVICE_COUNT];
    for (slot, residual) in devices.iter_mut().enumerate() {
        let c = &mut p.devices[slot];
        // The speed device usually follows airspeed step for step.
        let pred = if slot == device::SPEED {
            c.u.wrapping_add(airspeed_step)
        } else {
            c.p1()
        };
        let kind = SLOTS[slot];
        *residual = c.code(kind.limit(s.devices[slot]), kind.step(), pred);
    }
    let pred = p.heat.p1();
    let heat = p.heat.code(s.heat.clamp(0., 1.), UNIT, pred);
    let pred = p.fuel.p1();
    let fuel = p.fuel.code(s.fuel_lb, FUEL, pred);
    let controls: [i64; 4] = from_fn(|i| {
        let pred = p.controls[i].p1();
        p.controls[i].code(s.controls[i], CONTROL, pred)
    });
    let flags = s.flags.bits();
    let mut damage = 0u64;
    let mut damage_values: Vec<i64> = Vec::new();
    for (bit, (new, old)) in [s.hp, s.max_hp]
        .iter()
        .chain(&s.sections)
        .zip([p.hp, p.max_hp].iter().chain(&p.sections))
        .enumerate()
    {
        if new != old {
            damage |= 1 << bit;
            damage_values.push(i64::from(*new) - i64::from(*old));
        }
    }
    let structural_changed = s.structural_section != p.structural;
    if structural_changed {
        damage |= 1 << (2 + SECTION_COUNT);
    }

    let mut mask = 0;
    let mut set = |bit: u64, on: bool| {
        if on {
            mask |= bit;
        }
    };
    set(A_POS, pos != [0; 3]);
    set(A_ATT, att != [0; 3]);
    set(A_VEL, vel != [0; 3]);
    set(A_AIRSPEED, airspeed != 0);
    set(A_G, g != 0);
    set(A_DEVICES, devices.iter().any(|r| *r != 0));
    set(A_CONTROLS, controls.iter().any(|r| *r != 0));
    set(A_FUEL, fuel != 0);
    set(A_HEAT, heat != 0);
    set(A_FLAGS, flags != p.flags);
    set(A_WRECK, s.wreck_phase != p.wreck);
    set(A_DAMAGE, damage != 0);
    put_uv(buf, mask);
    if mask & A_POS != 0 {
        put_triple(buf, pos);
    }
    if mask & A_ATT != 0 {
        put_triple(buf, att);
    }
    if mask & A_VEL != 0 {
        put_triple(buf, vel);
    }
    if mask & A_AIRSPEED != 0 {
        put_iv(buf, airspeed);
    }
    if mask & A_G != 0 {
        put_iv(buf, g);
    }
    if mask & A_DEVICES != 0 {
        let bits = DEVICE_ORDER
            .iter()
            .enumerate()
            .filter(|(_, slot)| devices[**slot] != 0)
            .fold(0u64, |bits, (i, _)| bits | 1 << i);
        put_uv(buf, bits);
        for slot in DEVICE_ORDER {
            if devices[slot] != 0 {
                put_iv(buf, devices[slot]);
            }
        }
    }
    if mask & A_CONTROLS != 0 {
        let bits = (0..4)
            .filter(|i| controls[*i] != 0)
            .fold(0u8, |bits, i| bits | 1 << i);
        buf.push(bits);
        for r in controls.iter().filter(|r| **r != 0) {
            put_iv(buf, *r);
        }
    }
    if mask & A_FUEL != 0 {
        put_iv(buf, fuel);
    }
    if mask & A_HEAT != 0 {
        put_iv(buf, heat);
    }
    if mask & A_FLAGS != 0 {
        put_uv(buf, u64::from(flags));
    }
    if mask & A_WRECK != 0 {
        buf.push(s.wreck_phase);
    }
    if mask & A_DAMAGE != 0 {
        put_uv(buf, damage);
        for v in damage_values {
            put_iv(buf, v);
        }
        if structural_changed {
            put_opt_u8(buf, s.structural_section);
        }
    }
    p.flags = flags;
    p.wreck = s.wreck_phase;
    p.hp = s.hp;
    p.max_hp = s.max_hp;
    p.sections = s.sections;
    p.structural = s.structural_section;
    p
}

/// Reads one aircraft record on top of the state from the previous tick.
pub(crate) fn get_aircraft(input: &mut In, old: Option<AircraftPred>) -> Result<AircraftPred> {
    let mask = input.uv()?;
    if mask & A_KEY != 0 {
        if mask != A_KEY {
            return Err(corrupt("an aircraft key record carries change bits"));
        }
        return Ok(AircraftPred::key(&get_aircraft_key(input)?));
    }
    if mask & !A_ALL != 0 {
        return Err(corrupt("an aircraft record has unknown bits"));
    }
    let mut p = old.ok_or_else(|| corrupt("a change record for an aircraft with no key record"))?;
    let has = |bit: u64| mask & bit != 0;
    let pos = if has(A_POS) { input.triple()? } else { [0; 3] };
    let att = if has(A_ATT) { input.triple()? } else { [0; 3] };
    let vel = if has(A_VEL) { input.triple()? } else { [0; 3] };
    let airspeed = if has(A_AIRSPEED) { input.iv()? } else { 0 };
    let g = if has(A_G) { input.iv()? } else { 0 };
    let mut devices = [0i64; DEVICE_COUNT];
    if has(A_DEVICES) {
        let bits = input.uv()?;
        if bits >> DEVICE_COUNT != 0 || bits == 0 {
            return Err(corrupt("a device mask is invalid"));
        }
        for (i, slot) in DEVICE_ORDER.iter().enumerate() {
            if bits & (1 << i) != 0 {
                devices[*slot] = input.iv()?;
            }
        }
    }
    let mut controls = [0i64; 4];
    if has(A_CONTROLS) {
        let bits = input.u8()?;
        if bits >> 4 != 0 || bits == 0 {
            return Err(corrupt("a control mask is invalid"));
        }
        for (i, r) in controls.iter_mut().enumerate() {
            if bits & (1 << i) != 0 {
                *r = input.iv()?;
            }
        }
    }
    let fuel = if has(A_FUEL) { input.iv()? } else { 0 };
    let heat = if has(A_HEAT) { input.iv()? } else { 0 };
    for i in 0..3 {
        let pred = p.pos[i].p2();
        p.pos[i].apply(pred, pos[i]);
        let pred = p.att[i].p2();
        p.att[i].apply(pred, att[i]);
        let pred = p.vel[i].p2();
        p.vel[i].apply(pred, vel[i]);
    }
    let airspeed_before = p.airspeed.u;
    let pred = p.airspeed.p2();
    p.airspeed.apply(pred, airspeed);
    let airspeed_step = p.airspeed.u.wrapping_sub(airspeed_before);
    let pred = p.g.p1();
    p.g.apply(pred, g);
    for (slot, r) in devices.iter().enumerate() {
        let c = &mut p.devices[slot];
        let pred = if slot == device::SPEED {
            c.u.wrapping_add(airspeed_step)
        } else {
            c.p1()
        };
        c.apply(pred, *r);
    }
    let pred = p.heat.p1();
    p.heat.apply(pred, heat);
    let pred = p.fuel.p1();
    p.fuel.apply(pred, fuel);
    for (c, r) in p.controls.iter_mut().zip(controls) {
        let pred = c.p1();
        c.apply(pred, r);
    }
    if has(A_FLAGS) {
        p.flags =
            u16::try_from(input.uv()?).map_err(|_| corrupt("aircraft flags are out of range"))?;
    }
    if has(A_WRECK) {
        p.wreck = input.u8()?;
    }
    if has(A_DAMAGE) {
        let bits = input.uv()?;
        if bits >> (3 + SECTION_COUNT) != 0 || bits == 0 {
            return Err(corrupt("a damage mask is invalid"));
        }
        let on = |bit: usize| bits & (1 << bit) != 0;
        if on(0) {
            p.hp = add_i32(p.hp, input.iv()?)?;
        }
        if on(1) {
            p.max_hp = add_i32(p.max_hp, input.iv()?)?;
        }
        for i in 0..SECTION_COUNT {
            if on(2 + i) {
                p.sections[i] = add_i32(p.sections[i], input.iv()?)?;
            }
        }
        if on(2 + SECTION_COUNT) {
            p.structural = get_opt_u8(input)?;
        }
    }
    Ok(p)
}

const P_POS: u64 = 1;
const P_PREV: u64 = 1 << 1;
const P_DIR: u64 = 1 << 2;
const P_SPEED: u64 = 1 << 3;
const P_AGE: u64 = 1 << 4;
const P_FLAGS: u64 = 1 << 5;
const P_IDS: u64 = 1 << 6;
const P_SEEKER: u64 = 1 << 7;
const P_KEY: u64 = 1 << 8;
const P_ALL: u64 = P_KEY - 1;

fn direction_angles(d: [f64; 3]) -> (f64, f64) {
    (d[0].atan2(d[2]), d[1].atan2(d[0].hypot(d[2])))
}

fn projectile_encodable(s: &ProjectileState) -> bool {
    let [x, y, z] = s.direction;
    s.position
        .iter()
        .chain(&s.previous)
        .all(|v| within(*v, POSITION_LIMIT))
        && s.direction.iter().all(|v| v.is_finite())
        && ((x * x + y * y + z * z).sqrt() - 1.).abs() <= UNIT_TOLERANCE
        && within(s.speed, SCALAR_LIMIT)
}

/// What the reader knows about one projectile after its latest record.
#[derive(Clone, Debug)]
pub(crate) struct ProjectilePred {
    owner: u32,
    weapon: u32,
    target: Option<u32>,
    pos: [Chan; 3],
    prev: [Chan; 3],
    azimuth: Chan,
    elevation: Chan,
    direction: [f64; 3],
    speed: Chan,
    tracer: bool,
    incoming: bool,
    age: u32,
    seeker: Option<Seeker>,
    predictable: bool,
}

impl ProjectilePred {
    fn key(s: &ProjectileState) -> Self {
        let (azimuth, elevation) = direction_angles(s.direction);
        Self {
            owner: s.owner,
            weapon: s.weapon,
            target: s.target,
            pos: from_fn(|i| Chan::seeded(s.position[i], seed(s.direction[i] * s.speed))),
            prev: s.previous.map(Chan::key),
            azimuth: Chan::key(azimuth),
            elevation: Chan::key(elevation),
            direction: s.direction,
            speed: Chan::key(s.speed),
            tracer: s.tracer,
            incoming: s.incoming,
            age: s.age,
            seeker: s.seeker,
            predictable: projectile_encodable(s),
        }
    }

    pub fn state(&self, id: u32) -> ProjectileState {
        let direction = if self.azimuth.u == 0 && self.elevation.u == 0 {
            self.direction
        } else {
            let (sa, ca) = self.azimuth.value(ANGLE).sin_cos();
            let (se, ce) = self.elevation.value(ANGLE).sin_cos();
            [sa * ce, se, ca * ce]
        };
        ProjectileState {
            id,
            owner: self.owner,
            weapon: self.weapon,
            target: self.target,
            position: from_fn(|i| self.pos[i].value(POS)),
            previous: from_fn(|i| self.prev[i].value(POS)),
            direction,
            speed: self.speed.value(SPEED),
            tracer: self.tracer,
            incoming: self.incoming,
            age: self.age,
            seeker: self.seeker,
        }
    }

    fn flag_byte(tracer: bool, incoming: bool, seeker: bool) -> u8 {
        u8::from(tracer) | u8::from(incoming) << 1 | u8::from(seeker) << 2
    }
}

fn put_seeker(buf: &mut Vec<u8>, s: &Seeker) {
    buf.push(u8::from(s.acquired));
    buf.push(s.status);
    buf.extend_from_slice(&s.quality.to_bits().to_le_bytes());
    put_opt_id(buf, s.target);
}

fn get_seeker(input: &mut In) -> Result<Seeker> {
    let acquired = match input.u8()? {
        0 => false,
        1 => true,
        _ => return Err(corrupt("a seeker flag is invalid")),
    };
    Ok(Seeker {
        acquired,
        status: input.u8()?,
        quality: f32::from_bits(input.u32()?),
        target: input.opt_id()?,
    })
}

fn same_seeker(a: &Seeker, b: &Seeker) -> bool {
    a.acquired == b.acquired
        && a.status == b.status
        && a.quality.to_bits() == b.quality.to_bits()
        && a.target == b.target
}

fn put_projectile_key(buf: &mut Vec<u8>, s: &ProjectileState) {
    put_uv(buf, u64::from(s.owner));
    put_uv(buf, u64::from(s.weapon));
    put_opt_id(buf, s.target);
    for v in s
        .position
        .iter()
        .chain(&s.previous)
        .chain(&s.direction)
        .chain([&s.speed])
    {
        put_xf64(buf, *v);
    }
    buf.push(ProjectilePred::flag_byte(
        s.tracer,
        s.incoming,
        s.seeker.is_some(),
    ));
    put_uv(buf, u64::from(s.age));
    if let Some(seeker) = &s.seeker {
        put_seeker(buf, seeker);
    }
}

fn get_projectile_key(input: &mut In) -> Result<ProjectileState> {
    let owner = input.u32v()?;
    let weapon = input.u32v()?;
    let target = input.opt_id()?;
    let mut f = || input.xf64();
    let position = [f()?, f()?, f()?];
    let previous = [f()?, f()?, f()?];
    let direction = [f()?, f()?, f()?];
    let speed = f()?;
    let flags = input.u8()?;
    if flags >> 3 != 0 {
        return Err(corrupt("projectile flags are invalid"));
    }
    let age = input.u32v()?;
    let seeker = if flags & 4 != 0 {
        Some(get_seeker(input)?)
    } else {
        None
    };
    Ok(ProjectileState {
        id: 0,
        owner,
        weapon,
        target,
        position,
        previous,
        direction,
        speed,
        tracer: flags & 1 != 0,
        incoming: flags & 2 != 0,
        age,
        seeker,
    })
}

/// Writes one projectile record and returns the state the reader will hold.
pub(crate) fn put_projectile(
    buf: &mut Vec<u8>,
    old: Option<ProjectilePred>,
    s: &ProjectileState,
) -> ProjectilePred {
    let Some(mut p) = old.filter(|p| p.predictable && projectile_encodable(s)) else {
        put_uv(buf, P_KEY);
        put_projectile_key(buf, s);
        return ProjectilePred::key(s);
    };
    let earlier: [f64; 3] = from_fn(|i| p.pos[i].value(POS));
    let pos: [i64; 3] = from_fn(|i| {
        let pred = p.pos[i].p2();
        p.pos[i].code(s.position[i], POS, pred)
    });
    // The previous position is usually exactly the position one tick ago.
    let prev: [i64; 3] = from_fn(|i| {
        let pred = units((earlier[i] - p.prev[i].base) / POS);
        p.prev[i].code(s.previous[i], POS, pred)
    });
    let (azimuth, elevation) = direction_angles(s.direction);
    let pred = p.azimuth.p2();
    let dir_a = p.azimuth.code_angle(azimuth, pred);
    let pred = p.elevation.p2();
    let dir_e = p.elevation.code_angle(elevation, pred);
    let pred = p.speed.p2();
    let speed = p.speed.code(s.speed, SPEED, pred);
    let age = i64::from(s.age) - (i64::from(p.age) + 1);
    let flags = ProjectilePred::flag_byte(s.tracer, s.incoming, s.seeker.is_some());
    let old_flags = ProjectilePred::flag_byte(p.tracer, p.incoming, p.seeker.is_some());
    let ids = (s.owner, s.weapon, s.target) != (p.owner, p.weapon, p.target);
    let seeker = match (&s.seeker, &p.seeker) {
        (Some(new), Some(old)) => !same_seeker(new, old),
        (Some(_), None) => true,
        _ => false,
    };
    let mut mask = 0;
    let mut set = |bit: u64, on: bool| {
        if on {
            mask |= bit;
        }
    };
    set(P_POS, pos != [0; 3]);
    set(P_PREV, prev != [0; 3]);
    set(P_DIR, [dir_a, dir_e] != [0; 2]);
    set(P_SPEED, speed != 0);
    set(P_AGE, age != 0);
    set(P_FLAGS, flags != old_flags);
    set(P_IDS, ids);
    set(P_SEEKER, seeker);
    put_uv(buf, mask);
    if mask & P_POS != 0 {
        put_triple(buf, pos);
    }
    if mask & P_PREV != 0 {
        put_triple(buf, prev);
    }
    if mask & P_DIR != 0 {
        put_pair(buf, [dir_a, dir_e]);
    }
    if mask & P_SPEED != 0 {
        put_iv(buf, speed);
    }
    if mask & P_AGE != 0 {
        put_iv(buf, age);
    }
    if mask & P_FLAGS != 0 {
        buf.push(flags);
    }
    if ids {
        put_uv(buf, u64::from(s.owner));
        put_uv(buf, u64::from(s.weapon));
        put_opt_id(buf, s.target);
    }
    if let (true, Some(new)) = (seeker, &s.seeker) {
        put_seeker(buf, new);
    }
    p.owner = s.owner;
    p.weapon = s.weapon;
    p.target = s.target;
    p.tracer = s.tracer;
    p.incoming = s.incoming;
    p.age = s.age;
    p.seeker = s.seeker;
    p
}

/// Reads one projectile record on top of the state from the previous tick.
pub(crate) fn get_projectile(
    input: &mut In,
    old: Option<ProjectilePred>,
) -> Result<ProjectilePred> {
    let mask = input.uv()?;
    if mask & P_KEY != 0 {
        if mask != P_KEY {
            return Err(corrupt("a projectile key record carries change bits"));
        }
        return Ok(ProjectilePred::key(&get_projectile_key(input)?));
    }
    if mask & !P_ALL != 0 {
        return Err(corrupt("a projectile record has unknown bits"));
    }
    let mut p =
        old.ok_or_else(|| corrupt("a change record for a projectile with no key record"))?;
    let has = |bit: u64| mask & bit != 0;
    let pos = if has(P_POS) { input.triple()? } else { [0; 3] };
    let prev = if has(P_PREV) { input.triple()? } else { [0; 3] };
    let [dir_a, dir_e] = if has(P_DIR) { input.pair()? } else { [0; 2] };
    let speed = if has(P_SPEED) { input.iv()? } else { 0 };
    let age = if has(P_AGE) { input.iv()? } else { 0 };
    let earlier: [f64; 3] = from_fn(|i| p.pos[i].value(POS));
    for i in 0..3 {
        let pred = p.pos[i].p2();
        p.pos[i].apply(pred, pos[i]);
        let pred = units((earlier[i] - p.prev[i].base) / POS);
        p.prev[i].apply(pred, prev[i]);
    }
    let pred = p.azimuth.p2();
    p.azimuth.apply(pred, dir_a);
    let pred = p.elevation.p2();
    p.elevation.apply(pred, dir_e);
    let pred = p.speed.p2();
    p.speed.apply(pred, speed);
    p.age = u32::try_from(i64::from(p.age) + 1 + age)
        .map_err(|_| corrupt("a projectile age is out of range"))?;
    if has(P_FLAGS) {
        let flags = input.u8()?;
        if flags >> 3 != 0 {
            return Err(corrupt("projectile flags are invalid"));
        }
        p.tracer = flags & 1 != 0;
        p.incoming = flags & 2 != 0;
        if flags & 4 == 0 {
            p.seeker = None;
        } else if p.seeker.is_none() && !has(P_SEEKER) {
            return Err(corrupt("a seeker appeared without its values"));
        }
    }
    if has(P_IDS) {
        p.owner = input.u32v()?;
        p.weapon = input.u32v()?;
        p.target = input.opt_id()?;
    }
    if has(P_SEEKER) {
        p.seeker = Some(get_seeker(input)?);
    }
    Ok(p)
}

const D_POS: u64 = 1;
const D_ATT: u64 = 1 << 1;
const D_KEY: u64 = 1 << 6;

fn debris_encodable(s: &DebrisState) -> bool {
    s.position.iter().all(|v| within(*v, POSITION_LIMIT))
        && s.attitude.iter().all(|v| within(*v, ANGLE_LIMIT))
}

#[derive(Clone, Debug)]
pub(crate) struct DebrisPred {
    pos: [Chan; 3],
    att: [Chan; 3],
    predictable: bool,
}

impl DebrisPred {
    fn key(s: &DebrisState) -> Self {
        Self {
            pos: s.position.map(Chan::key),
            att: s.attitude.map(Chan::key),
            predictable: debris_encodable(s),
        }
    }

    pub fn state(&self, owner: u32, index: u32) -> DebrisState {
        DebrisState {
            owner,
            index,
            position: from_fn(|i| self.pos[i].value(POS)),
            attitude: [
                self.att[0].yaw(),
                self.att[1].signed_angle(),
                self.att[2].signed_angle(),
            ],
        }
    }
}

pub(crate) fn put_debris(
    buf: &mut Vec<u8>,
    old: Option<DebrisPred>,
    s: &DebrisState,
) -> DebrisPred {
    let Some(mut p) = old.filter(|p| p.predictable && debris_encodable(s)) else {
        put_uv(buf, D_KEY);
        for v in s.position.iter().chain(&s.attitude) {
            put_xf64(buf, *v);
        }
        return DebrisPred::key(s);
    };
    let pos: [i64; 3] = from_fn(|i| {
        let pred = p.pos[i].p2();
        p.pos[i].code(s.position[i], POS, pred)
    });
    let att: [i64; 3] = from_fn(|i| {
        let pred = p.att[i].p2();
        p.att[i].code_angle(s.attitude[i], pred)
    });
    let mask = if pos != [0; 3] { D_POS } else { 0 } | if att != [0; 3] { D_ATT } else { 0 };
    put_uv(buf, mask);
    if mask & D_POS != 0 {
        put_triple(buf, pos);
    }
    if mask & D_ATT != 0 {
        put_triple(buf, att);
    }
    p
}

pub(crate) fn get_debris(input: &mut In, old: Option<DebrisPred>) -> Result<DebrisPred> {
    let mask = input.uv()?;
    if mask == D_KEY {
        let mut f = || input.xf64();
        let position = [f()?, f()?, f()?];
        let attitude = [f()?, f()?, f()?];
        return Ok(DebrisPred::key(&DebrisState {
            owner: 0,
            index: 0,
            position,
            attitude,
        }));
    }
    if mask & !(D_POS | D_ATT) != 0 {
        return Err(corrupt("a debris record has unknown bits"));
    }
    let mut p = old.ok_or_else(|| corrupt("a change record for debris with no key record"))?;
    let pos = if mask & D_POS != 0 {
        input.triple()?
    } else {
        [0; 3]
    };
    let att = if mask & D_ATT != 0 {
        input.triple()?
    } else {
        [0; 3]
    };
    for i in 0..3 {
        let pred = p.pos[i].p2();
        p.pos[i].apply(pred, pos[i]);
        let pred = p.att[i].p2();
        p.att[i].apply(pred, att[i]);
    }
    Ok(p)
}

const E_POS: u64 = 1;
const E_HEADING: u64 = 1 << 1;
const E_PHASE: u64 = 1 << 2;
const E_KEY: u64 = 1 << 6;

fn escapee_encodable(s: &EscapeeState) -> bool {
    s.position.iter().all(|v| within(*v, POSITION_LIMIT)) && within(s.heading, ANGLE_LIMIT)
}

#[derive(Clone, Debug)]
pub(crate) struct EscapeePred {
    pos: [Chan; 3],
    heading: Chan,
    phase: u8,
    predictable: bool,
}

impl EscapeePred {
    fn key(s: &EscapeeState) -> Self {
        Self {
            pos: s.position.map(Chan::key),
            heading: Chan::key(s.heading),
            phase: s.phase,
            predictable: escapee_encodable(s),
        }
    }

    pub fn state(&self, owner: u32) -> EscapeeState {
        EscapeeState {
            owner,
            position: from_fn(|i| self.pos[i].value(POS)),
            heading: self.heading.yaw(),
            phase: self.phase,
        }
    }
}

pub(crate) fn put_escapee(
    buf: &mut Vec<u8>,
    old: Option<EscapeePred>,
    s: &EscapeeState,
) -> EscapeePred {
    let Some(mut p) = old.filter(|p| p.predictable && escapee_encodable(s)) else {
        put_uv(buf, E_KEY);
        for v in s.position.iter().chain([&s.heading]) {
            put_xf64(buf, *v);
        }
        buf.push(s.phase);
        return EscapeePred::key(s);
    };
    let pos: [i64; 3] = from_fn(|i| {
        let pred = p.pos[i].p2();
        p.pos[i].code(s.position[i], POS, pred)
    });
    let pred = p.heading.p2();
    let heading = p.heading.code_angle(s.heading, pred);
    let mask = if pos != [0; 3] { E_POS } else { 0 }
        | if heading != 0 { E_HEADING } else { 0 }
        | if s.phase != p.phase { E_PHASE } else { 0 };
    put_uv(buf, mask);
    if mask & E_POS != 0 {
        put_triple(buf, pos);
    }
    if mask & E_HEADING != 0 {
        put_iv(buf, heading);
    }
    if mask & E_PHASE != 0 {
        buf.push(s.phase);
    }
    p.phase = s.phase;
    p
}

pub(crate) fn get_escapee(input: &mut In, old: Option<EscapeePred>) -> Result<EscapeePred> {
    let mask = input.uv()?;
    if mask == E_KEY {
        let mut f = || input.xf64();
        let position = [f()?, f()?, f()?];
        let heading = f()?;
        let phase = input.u8()?;
        return Ok(EscapeePred::key(&EscapeeState {
            owner: 0,
            position,
            heading,
            phase,
        }));
    }
    if mask & !(E_POS | E_HEADING | E_PHASE) != 0 {
        return Err(corrupt("an ejected pilot record has unknown bits"));
    }
    let mut p = old.ok_or_else(|| corrupt("a change record for a pilot with no key record"))?;
    let pos = if mask & E_POS != 0 {
        input.triple()?
    } else {
        [0; 3]
    };
    let heading = if mask & E_HEADING != 0 {
        input.iv()?
    } else {
        0
    };
    for (c, r) in p.pos.iter_mut().zip(pos) {
        let pred = c.p2();
        c.apply(pred, r);
    }
    let pred = p.heading.p2();
    p.heading.apply(pred, heading);
    if mask & E_PHASE != 0 {
        p.phase = input.u8()?;
    }
    Ok(p)
}
