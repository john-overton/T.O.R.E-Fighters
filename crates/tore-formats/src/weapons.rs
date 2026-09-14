//! Checked, owned FA weapon/sensor configuration. No native callbacks execute.
//! Raw timing and flag fields deliberately retain source names/units until reviewed.
use crate::{
    Result,
    aircraft::{Brf, Equipment, Token},
    invalid,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zone {
    pub heading: i16,
    pub pitch: i16,
    pub minimum_range: i32,
    pub maximum_range: i32,
    pub minimum_altitude: i32,
    pub maximum_altitude: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seeker {
    pub flags: [u8; 2],
    pub signature: u8,
    pub look_down: u8,
    pub doppler_above: u8,
    pub doppler_below: u8,
    pub doppler_minimum_range: u8,
    pub all_aspect: u8,
    pub zones: [Zone; 2],
    pub chaff_flare_chance: u8,
    pub deception_chance: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Movement {
    pub minimum_speed: i16,
    pub corner_speed: i16,
    pub maximum_speed: i16,
    pub acceleration: i32,
    pub deceleration: i32,
    pub initial_speed: i16,
    pub final_speed: i16,
    pub launch_retard: u8,
    pub ignite_t: u16,
    pub fuel_t: u16,
    pub remove_t: u16,
    pub powered_turn_rate: i16,
    pub unpowered_turn_rate: i16,
    pub performance_at_0: u8,
    pub performance_at_20: u8,
    pub cruise: [u8; 4],
    pub jink: [i16; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Burst {
    pub projectiles_in_pod: i16,
    pub actual_rounds_per_game: u8,
    pub game_rounds_in_burst: u8,
    pub game_rounds_in_carpet_burst: u8,
    pub game_burst_t: u8,
    pub reload_t: u8,
    pub startup_shots: u8,
    pub random_fire_percent: i16,
    pub offset_fire_percent: i16,
    pub offset_fire_heading: i16,
    pub offset_fire_pitch: i16,
    pub sine_pattern: [u8; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guidance {
    pub track_t: u8,
    pub track_max_g_raw: u8,
    pub target_sun_chance: u8,
    pub max_aon: u8,
    pub chances: [u8; 4],
    /// Source order: taa, climb, G, air, speed, speedMin, predictable, bigPlane, gMiss.
    pub hit_modifiers: [u8; 9],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Damage {
    pub by_class: [i16; 5],
    pub fuze_arm_t: u16,
    pub fuze_radius: i16,
    pub side_hit_fuze_failure: u8,
    pub collateral_radius: i16,
    pub collateral_percent: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effects {
    pub object_explosion: u8,
    pub land_explosion: u8,
    pub water_explosion: u8,
    pub crater_size: u8,
    /// Type, frequency, exist time, start size, end size, in native raw units.
    pub smoke: [u8; 5],
    pub max_sound_distance: i16,
    pub frequency_adjustment: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Weapon {
    pub source: String,
    pub name: String,
    pub shape: Option<String>,
    pub fire_sound: Option<String>,
    pub native_callback: String,
    pub flags: u32,
    pub object_flags: u32,
    pub weight: i32,
    pub movement: Movement,
    pub burst: Burst,
    pub seeker: Seeker,
    pub guidance: Guidance,
    pub damage: Damage,
    pub effects: Effects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Countermeasures {
    pub weight: u16,
    pub flags: u8,
    pub mode_flags: u16,
    /// Loaded, chance, heading, pitch; heading/pitch retain raw byte interpretation.
    pub chaff: [u8; 4],
    pub flare: [u8; 4],
    pub radar_deception_chance: u8,
    pub radar_signature_add: u16,
    pub radar_noise_range: [u8; 2],
    pub infrared_deception_chance: u8,
    pub infrared_signature_add: u16,
    pub infrared_lose_lock_time: u8,
}
impl Countermeasures {
    pub fn parse(name: &str, bytes: &[u8]) -> Result<Self> {
        let name = name.to_ascii_uppercase();
        if !name.ends_with(".ECM") {
            return Err(invalid("expected ECM equipment"));
        }
        let e = Equipment::parse(&name, bytes)?;
        let f = Fields(&e.fields);
        if f.byte("structType")? != 9 {
            return Err(invalid("unreviewed ECM type"));
        }
        Ok(Self {
            weight: f.word("weight")? as u16,
            flags: f.byte("flags")?,
            mode_flags: f.word("flags[1]")? as u16,
            chaff: [
                f.byte("chaffLoaded")?,
                f.byte("chaffChance")?,
                f.byte("chaffH")?,
                f.byte("chaffP")?,
            ],
            flare: [
                f.byte("flaresLoaded")?,
                f.byte("flareChance")?,
                f.byte("flareH")?,
                f.byte("flareP")?,
            ],
            radar_deception_chance: f.byte("rdChance")?,
            radar_signature_add: f.word("rdSigAdd")? as u16,
            radar_noise_range: [f.byte("rNoiseMinDist")?, f.byte("rNoiseMaxDist")?],
            infrared_deception_chance: f.byte("irdChance")?,
            infrared_signature_add: f.word("irdSigAdd")? as u16,
            infrared_lose_lock_time: f.byte("irdLoseLockTime")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tank {
    pub empty_weight: u16,
    pub fuel_weight: i32,
    pub flags: u8,
}
impl Tank {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let b = Brf::parse(bytes)?;
        let t = b.block("")?;
        if t.len() != 5
            || t[0].kind != "byte"
            || t[0].number()? != 8
            || t[1].kind != "ptr"
            || t[2].kind != "word"
            || t[3].kind != "byte"
            || t[4].kind != "dword"
        {
            return Err(invalid("unreviewed GAS layout"));
        }
        if b.strings(&t[1].value)?.len() != 3 {
            return Err(invalid("invalid GAS identity"));
        }
        let fuel_weight = t[4].number()?;
        if fuel_weight < 0 {
            return Err(invalid("negative tank fuel mass"));
        }
        Ok(Self {
            empty_weight: t[2].number()? as u16,
            flags: t[3].number()? as u8,
            fuel_weight,
        })
    }
}

struct Fields<'a>(&'a BTreeMap<String, Token>);
impl Fields<'_> {
    fn number(&self, name: &str) -> Result<i32> {
        self.0
            .get(name)
            .ok_or_else(|| invalid(&format!("missing required equipment field {name}")))?
            .number()
    }
    fn byte(&self, name: &str) -> Result<u8> {
        u8::try_from(self.number(name)?)
            .map_err(|_| invalid(&format!("equipment byte out of range: {name}")))
    }
    fn word(&self, name: &str) -> Result<i16> {
        i16::try_from(self.number(name)?)
            .map_err(|_| invalid(&format!("equipment word out of range: {name}")))
    }
    fn pointer(&self, brf: &Brf, name: &str) -> Result<Option<String>> {
        let t = self
            .0
            .get(name)
            .ok_or_else(|| invalid("missing equipment pointer"))?;
        if t.kind == "dword" && t.number()? == 0 {
            return Ok(None);
        }
        if t.kind != "ptr" {
            return Err(invalid("invalid equipment pointer kind"));
        }
        let strings = brf.strings(&t.value)?;
        if strings.len() != 1 {
            return Err(invalid("equipment pointer must name one resource"));
        }
        Ok(Some(strings[0].to_ascii_uppercase()))
    }
}

impl Seeker {
    fn read(f: &Fields<'_>, projectile: bool) -> Result<Self> {
        let zone = |i| -> Result<Zone> {
            let get = |n: &str| f.number(&format!("zone{i}.{n}"));
            let z = Zone {
                heading: f.word(&format!("zone{i}.h"))?,
                pitch: f.word(&format!("zone{i}.p"))?,
                minimum_range: get("minRange")?,
                maximum_range: get("maxRange")?,
                minimum_altitude: get("minAlt")?,
                maximum_altitude: get("maxAlt")?,
            };
            // Negative headings occur in retail rear-facing sensors. Preserve
            // them; the rear-facing coordinate producer needs separate review.
            if z.minimum_range < 0
                || z.minimum_range > z.maximum_range
                || z.minimum_altitude > z.maximum_altitude
            {
                return Err(invalid("invalid equipment acquisition/launch zone"));
            }
            Ok(z)
        };
        let signature = f.byte("sig")?;
        if signature > 4 {
            return Err(invalid("unreviewed signature selector"));
        }
        Ok(Self {
            flags: if projectile {
                [f.byte("flags[1]")?, f.byte("flags[2]")?]
            } else {
                [f.byte("flags")?, f.byte("flags[1]")?]
            },
            signature,
            look_down: f.byte("lookDown")?,
            doppler_above: f.byte("dopplerSpeedAbove")?,
            doppler_below: f.byte("dopplerSpeedBelow")?,
            doppler_minimum_range: f.byte("dopplerMinRange")?,
            all_aspect: f.byte("allAspect")?,
            zones: [zone(0)?, zone(1)?],
            chaff_flare_chance: f.byte("chaffFlareChance")?,
            deception_chance: f.byte("deceptionChance")?,
        })
    }
    pub fn parse(name: &str, data: &[u8]) -> Result<Self> {
        let name = name.to_ascii_uppercase();
        if !name.ends_with(".SEE") {
            return Err(invalid("expected SEE sensor"));
        }
        let e = Equipment::parse(&name, data)?;
        if Fields(&e.fields).byte("structType")? != 10 {
            return Err(invalid("unreviewed SEE type"));
        }
        Seeker::read(&Fields(&e.fields), false)
    }
}

impl Weapon {
    pub fn parse(name: &str, data: &[u8]) -> Result<Self> {
        let source = name.to_ascii_uppercase();
        if !source.ends_with(".JT") {
            return Err(invalid("expected JT projectile"));
        }
        let e = Equipment::parse(&source, data)?;
        let b = Brf::parse(data)?;
        let (f, o) = (Fields(&e.fields), Fields(&e.object));
        if o.number("structType")? != 7
            || o.number("typeSize")? != 315
            || f.number("structType")? != 10
        {
            return Err(invalid("unreviewed FA projectile layout"));
        }
        let movement = Movement {
            minimum_speed: o.word("_minSpeed")?,
            corner_speed: o.word("_cornerSpeed")?,
            maximum_speed: o.word("_maxSpeed")?,
            acceleration: o.number("_acc")?,
            deceleration: o.number("_dacc")?,
            initial_speed: f.word("initialSpeed")?,
            final_speed: f.word("finalSpeed")?,
            launch_retard: f.byte("launchRetard")?,
            ignite_t: f.word("igniteT")? as u16,
            fuel_t: f.word("fuelT")? as u16,
            remove_t: f.word("removeT")? as u16,
            powered_turn_rate: f.word("poweredTurnRate")?,
            unpowered_turn_rate: f.word("unpoweredTurnRate")?,
            performance_at_0: f.byte("performanceAt0")?,
            performance_at_20: f.byte("performanceAt20")?,
            cruise: [
                f.byte("cruise1Dist")?,
                f.byte("cruise1Alt")?,
                f.byte("cruise2Dist")?,
                f.byte("cruise2Alt")?,
            ],
            jink: [f.word("jinkSize")?, f.word("jinkT")?, f.word("totalJinkT")?],
        };
        if movement.minimum_speed < 0
            || movement.maximum_speed < movement.minimum_speed
            || movement.acceleration < 0
            || movement.deceleration < 0
        {
            return Err(invalid("invalid projectile speed/acceleration bounds"));
        }
        let burst = Burst {
            projectiles_in_pod: f.word("projsInPod")?,
            actual_rounds_per_game: f.byte("actualRoundsPerGame")?,
            game_rounds_in_burst: f.byte("gameRoundsInBurst")?,
            game_rounds_in_carpet_burst: f.byte("gameRoundsInCarpetBurst")?,
            game_burst_t: f.byte("gameBurstT")?,
            reload_t: f.byte("reloadT")?,
            startup_shots: f.byte("startupShots")?,
            random_fire_percent: f.word("randomFirePercent")?,
            offset_fire_percent: f.word("offsetFirePercent")?,
            offset_fire_heading: f.word("offsetFireH")?,
            offset_fire_pitch: f.word("offsetFireP")?,
            sine_pattern: [
                f.byte("hSines")?,
                f.byte("hSineDegrees")?,
                f.byte("vSines")?,
                f.byte("vSineDegrees")?,
            ],
        };
        let guidance = Guidance {
            track_t: f.byte("trackT")?,
            track_max_g_raw: f.byte("trackMaxG")?,
            target_sun_chance: f.byte("targetSunChance")?,
            max_aon: f.byte("maxAON")?,
            chances: [
                f.byte("chances[0]")?,
                f.byte("chances[1]")?,
                f.byte("chances[2]")?,
                f.byte("chances[3]")?,
            ],
            hit_modifiers: [
                f.byte("taaHitChange")?,
                f.byte("climbHitChange")?,
                f.byte("gHitChange")?,
                f.byte("airHitChange")?,
                f.byte("speedHitChange")?,
                f.byte("speedHitMin")?,
                f.byte("predictableHitChange")?,
                f.byte("bigPlaneChange")?,
                f.byte("gMiss")?,
            ],
        };
        let damage = Damage {
            by_class: [
                o.word("damage[0]")?,
                o.word("damage[1]")?,
                o.word("damage[2]")?,
                o.word("damage[3]")?,
                o.word("damage[4]")?,
            ],
            fuze_arm_t: f.word("fuzeArmT")? as u16,
            fuze_radius: f.word("fuzeRadius")?,
            side_hit_fuze_failure: f.byte("sideHitFuzeFailure")?,
            collateral_radius: f.word("collateralDamageRadius")?,
            collateral_percent: f.word("collateralDamagePercent")?,
        };
        let effects = Effects {
            object_explosion: o.byte("expType")?,
            land_explosion: f.byte("expTypeForLand")?,
            water_explosion: f.byte("expTypeForWater")?,
            crater_size: o.byte("craterSize")?,
            smoke: [
                f.byte("smokeType")?,
                f.byte("smokeFreq")?,
                f.byte("smokeExistTime")?,
                f.byte("smokeStartSize")?,
                f.byte("smokeEndSize")?,
            ],
            max_sound_distance: f.word("maxSndDist")?,
            frequency_adjustment: f.word("freqAdj")?,
        };
        let native_callback =
            o.0.get("utilProc")
                .ok_or_else(|| invalid("missing projectile callback"))?
                .value
                .clone();
        if native_callback != "_PROJProc" {
            return Err(invalid("unreviewed projectile callback"));
        }
        Ok(Self {
            source,
            name: e.name.clone(),
            shape: o.pointer(&b, "shape")?,
            fire_sound: f.pointer(&b, "fireSound")?,
            native_callback,
            flags: f.number("flags")? as u32,
            object_flags: o.number("flags")? as u32,
            weight: o.number("weight")?,
            movement,
            burst,
            seeker: Seeker::read(&f, true)?,
            guidance,
            damage,
            effects,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rear_sensor_and_countermeasure_words_preserve_source_bits() {
        let make = |schema: &[(&str, &str)], sensor: bool| {
            let mut s = "[brent's_relocatable_format]\n".to_string();
            for &(kind, name) in schema {
                let v = match name {
                    "structType" if sensor => "10",
                    "structType" => "9",
                    "si_names" => "si_names",
                    "zone0.h" => "-100",
                    "flags[1]" if !sensor => "$ffff",
                    _ => "0",
                };
                s.push_str(&format!("{kind} {v}\n"));
            }
            s.push_str(":si_names\nstring \"Synthetic\"\nstring \"Synthetic equipment\"\nstring \"TEST\"\nend\n");
            s
        };
        let sensor = make(crate::aircraft::schema::SENSOR, true);
        assert_eq!(
            Seeker::parse("TEST.SEE", sensor.as_bytes()).unwrap().zones[0].heading,
            -100
        );
        assert!(
            Seeker::parse(
                "TEST.SEE",
                sensor.replacen("byte 10", "byte 9", 1).as_bytes()
            )
            .is_err()
        );
        let ecm = make(crate::aircraft::schema::ECM, false);
        assert_eq!(
            Countermeasures::parse("TEST.ECM", ecm.as_bytes())
                .unwrap()
                .mode_flags,
            65535
        );
    }
    #[test]
    fn tank_identity_layout_and_fuel_are_checked() {
        let s = b"[brent's_relocatable_format]\nbyte 8\nptr names\nword 200\nbyte 1\ndword 300\n:names\nstring \"Test\"\nstring \"Test tank\"\nstring \"TEST.GAS\"\nend\n";
        let t = Tank::parse(s).unwrap();
        assert_eq!((t.empty_weight, t.fuel_weight), (200, 300));
        let bad = String::from_utf8_lossy(s).replace("dword 300", "dword -1");
        assert!(Tank::parse(bad.as_bytes()).is_err());
        assert!(Tank::parse(&s[..s.len() - 5]).is_err());
    }
    fn fixture() -> String {
        let mut s = "[brent's_relocatable_format]\n".to_string();
        for (layout, fields) in [
            ("object", crate::aircraft::schema::OBJECT),
            ("projectile", crate::aircraft::schema::PROJECTILE),
        ] {
            for &(kind, name) in fields {
                let v = match (layout, name) {
                    ("object", "structType") => "7",
                    ("object", "typeSize") => "315",
                    ("projectile", "structType") => "10",
                    (_, "utilProc") => "_PROJProc",
                    (_, "si_names") | (_, "ot_names") => "names",
                    (_, "_maxSpeed") => "500",
                    _ => "0",
                };
                let kind = if kind == "ptr" && v == "0" {
                    "dword"
                } else {
                    kind
                };
                s.push_str(&format!("{kind} {v}\n"));
            }
        }
        // Equipment's reviewed name-block convention remains si_names.
        s = s.replace("ptr names", "ptr si_names");
        s.push_str(":si_names\nstring \"Synthetic\"\nstring \"Synthetic weapon\"\nstring \"TEST.JT\"\nend\n");
        s
    }
    #[test]
    fn checked_profile_retains_fields_without_defaulting_missing_values() {
        let s = fixture();
        let w = Weapon::parse("test.jt", s.as_bytes()).unwrap();
        assert_eq!(w.movement.maximum_speed, 500);
        assert_eq!(w.shape, None);
        assert!(Weapon::parse("TEST.JT", s.replace("word 315", "word 314").as_bytes()).is_err());
        assert!(
            Weapon::parse(
                "TEST.JT",
                s.replace("symbol _PROJProc", "symbol _Unknown").as_bytes()
            )
            .is_err()
        );
        assert!(Weapon::parse("TEST.JT", s.replacen("byte 0\n", "", 1).as_bytes()).is_err());
        assert!(Fields(&BTreeMap::new()).number("initialSpeed").is_err());
    }
}
