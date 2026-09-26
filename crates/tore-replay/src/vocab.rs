//! The shared vocabulary of events, fields, channels, units and outcomes.
//! The recorder writes these names and the exports read them, so both sides
//! agree without a format change. A producer may add fields beyond those
//! listed; readers ignore what they do not know.
//!
//! Conventions: `subject` is the actor (shooter, speaker, sender, the aircraft
//! that changed), `object` the other party (target, killer, recipient).
//! Distances are feet unless the field name says otherwise; `_kt` is knots,
//! `_nm` nautical miles, `_deg` degrees, `_s` seconds. Comms entries share one
//! outcome vocabulary ([`outcome`]).

/// Event kinds.
pub mod kind {
    /// A weapon left its launcher. Subject: shooter. Object: intended target.
    /// Fields: `projectile` (Id), `weapon` (Id of a WeaponInfo), `class`
    /// (Text), `mode` (Text: seeker or fire mode), `range_ft`, `aspect_deg`,
    /// `off_boresight_deg`, `closure_kt`, `shooter_alt_ft`, `target_alt_ft`,
    /// `shooter_speed_kt`, `target_speed_kt` (Num).
    pub const WEAPON_LAUNCH: &str = "weapon.launch";
    /// A guided weapon's own seeker started searching. Subject: shooter.
    /// Object: target. Fields: `projectile`, `weapon`, `range_ft`.
    pub const WEAPON_SEEKER_ACTIVE: &str = "weapon.seeker_active";
    /// An active radar missile locked on by itself. Subject: shooter.
    /// Object: target. Fields: `projectile`, `range_ft`.
    pub const WEAPON_PITBULL: &str = "weapon.pitbull";
    /// A guided weapon lost its target. Subject: shooter. Object: target.
    /// Fields: `projectile`, `reason` (Text), `range_ft`.
    pub const WEAPON_TRACK_LOST: &str = "weapon.track_lost";
    /// A guided weapon followed a decoy. Subject: shooter. Object: the
    /// aircraft that released the decoy. Fields: `projectile`, `decoy`
    /// (Text: chaff or flare), `reason`.
    pub const WEAPON_DECOYED: &str = "weapon.decoyed";
    /// How a shot ended. Subject: shooter. Object: intended target. Fields:
    /// `projectile`, `result` (Text from [`super::outcome`]: hit, missed,
    /// spoofed, jammed), `damage` (Int), `hp_after` (Int), `reason` (Text),
    /// `miss_ft` (Num).
    pub const WEAPON_OUTCOME: &str = "weapon.outcome";
    /// Damage landed. Subject: attacker. Object: the aircraft or surface
    /// object hit. Fields: `projectile`, `weapon`, `damage` (Int), `hp_after`
    /// (Int), `section` (Int).
    pub const COMBAT_HIT: &str = "combat.hit";
    /// Something was destroyed. Subject: what was destroyed. Object: killer.
    /// Fields: `weapon` (Id), `projectile` (Id), `reason` (Text).
    pub const COMBAT_DESTROYED: &str = "combat.destroyed";
    /// A warhead burst in the air near its target. Subject: shooter. Object:
    /// target. Fields: `projectile`, `miss_ft`, `damage`.
    pub const COMBAT_AIRBURST: &str = "combat.airburst";
    /// A weapon hit the ground. Subject: shooter. Fields: `projectile`,
    /// `weapon`.
    pub const COMBAT_GROUND_IMPACT: &str = "combat.ground_impact";
    /// An aircraft hit the ground. Subject: the aircraft. Fields: `speed_kt`,
    /// `reason`.
    pub const AIRCRAFT_CRASHED: &str = "aircraft.crashed";
    /// A pilot ejected. Subject: the aircraft. Fields: `reason`.
    pub const AIRCRAFT_EJECTED: &str = "aircraft.ejected";
    /// A pilot died. Subject: the aircraft. Object: killer, if any. Fields:
    /// `reason`.
    pub const AIRCRAFT_PILOT_KILLED: &str = "aircraft.pilot_killed";
    /// Wheels left the ground. Subject: the aircraft. Fields: `airport` (Id).
    pub const AIRCRAFT_TOOK_OFF: &str = "aircraft.took_off";
    /// Wheels touched down and stayed. Subject: the aircraft. Fields:
    /// `airport` (Id), `grade` (Text), `sink_fps` (Num).
    pub const AIRCRAFT_LANDED: &str = "aircraft.landed";
    /// An engine stopped. Subject: the aircraft. Fields: `reason`.
    pub const AIRCRAFT_FLAMEOUT: &str = "aircraft.flameout";
    /// Fuel ran out. Subject: the aircraft.
    pub const AIRCRAFT_FUEL_OUT: &str = "aircraft.fuel_out";
    /// The flight model changed departure mode. Subject: the aircraft.
    /// Fields: `from`, `to` (Text), `reason`.
    pub const FLIGHT_DEPARTURE: &str = "flight.departure";
    /// A stall began or ended. Subject: the aircraft. Fields: `on` (Bool),
    /// `aoa_deg`, `speed_kt`.
    pub const FLIGHT_STALL: &str = "flight.stall";
    /// A spin began or ended. Subject: the aircraft. Fields: `on` (Bool),
    /// `direction` (Text: left or right).
    pub const FLIGHT_SPIN: &str = "flight.spin";
    /// The pilot asked for more G than the limit allowed. Subject: the
    /// aircraft. Fields: `asked`, `limit` (Num, G), `reason`.
    pub const FLIGHT_G_LIMIT: &str = "flight.g_limit";
    /// The structure failed. Subject: the aircraft. Fields: `section` (Int),
    /// `g` (Num).
    pub const FLIGHT_STRUCTURAL_FAILURE: &str = "flight.structural_failure";
    /// A flight-model effect started or stopped applying. Subject: the
    /// aircraft. Fields: `effect` (Text), `on` (Bool), `reason` (Text: the
    /// because line). Tacview shows these as debug events.
    pub const FLIGHT_EFFECT: &str = "flight.effect";
    /// AI activity changed. Subject: the aircraft. Fields: `from`, `to`
    /// (Text), `reason`.
    pub const AI_ACTIVITY: &str = "ai.activity";
    /// AI target changed. Subject: the aircraft. Object: the new target.
    /// Fields: `from`, `to` (Id), `priority` (Text), `score` (Num),
    /// `reason`.
    pub const AI_TARGET: &str = "ai.target";
    /// AI weapon service phase changed. Subject: the aircraft. Object:
    /// target. Fields: `from`, `to` (Text), `station` (Int), `reason`.
    pub const AI_WEAPON_PHASE: &str = "ai.weapon_phase";
    /// AI started or changed a defensive reaction. Subject: the aircraft.
    /// Object: the threat's shooter. Fields: `threat` (Id of the projectile),
    /// `reaction` (Text), `range_ft`, `reason`.
    pub const AI_DEFENSE: &str = "ai.defense";
    /// AI fell back to a simpler behaviour. Subject: the aircraft. Fields:
    /// `from`, `to`, `reason`.
    pub const AI_FALLBACK: &str = "ai.fallback";
    /// AI airfield phase changed (taxi, takeoff, landing). Subject: the
    /// aircraft. Fields: `from`, `to`, `airport` (Id), `reason`.
    pub const AI_AIRFIELD_PHASE: &str = "ai.airfield_phase";
    /// AI decided to eject, or checked and did not. Subject: the aircraft.
    /// Fields: `decision` (Text), `reason`.
    pub const AI_EJECTION: &str = "ai.ejection";
    /// An order between aircraft. Subject: sender. Fields: `message` (Int,
    /// unique per recording), `recipients` (Ids), `order` (Text), `trigger`,
    /// `reason`, and for a single recipient `outcome`. Each recipient's
    /// answer is its own [`COMMS_DELIVERY`].
    pub const COMMS_ORDER: &str = "comms.order";
    /// A request, for example a wingman asking for help. Same fields as an
    /// order.
    pub const COMMS_REQUEST: &str = "comms.request";
    /// A silent data report, for example an attack passed to escorts. Same
    /// fields as an order, plus `about` (Id) and `kept_s` (Num).
    pub const COMMS_REPORT: &str = "comms.report";
    /// One recipient's outcome for an order, request or report. Subject:
    /// recipient. Object: sender. Fields: `message` (Int), `outcome`,
    /// `reason`, `wait_s`.
    pub const COMMS_DELIVERY: &str = "comms.delivery";
    /// A radio call. Subject: speaker. Fields: `speaker` (Text label),
    /// `stems` (Text: the recordings, space separated), `route` (Text),
    /// `trigger` (Text), `heard` (Bool: the player could hear it), `outcome`,
    /// `reason`, `wait_s`. Text: the words.
    pub const COMMS_RADIO: &str = "comms.radio";
    /// A crew remark in the player's cockpit. Same fields as radio.
    pub const COMMS_CREW: &str = "comms.crew";
    /// A tower reply. Same fields as radio, plus `airport` (Id).
    pub const COMMS_TOWER: &str = "comms.tower";
    /// A cockpit HUD message. Fields: `outcome`, `reason`. Text: the message.
    pub const COMMS_HUD: &str = "comms.hud";
    /// A seeker or lock tone changed. Subject: the aircraft. Fields: `tone`
    /// (Text), `on` (Bool), `reason`.
    pub const AUDIO_TONE: &str = "audio.tone";
    /// The stall warning started or stopped. Subject: the aircraft. Fields:
    /// `on` (Bool).
    pub const AUDIO_STALL_WARNING: &str = "audio.stall_warning";
    /// The music situation changed. Fields: `from`, `to` (Text), `reason`.
    pub const AUDIO_MUSIC: &str = "audio.music";
    /// A sound effect played. Subject: the source, if any. Fields: `sound`
    /// (Text), `x_ft`, `y_ft`, `z_ft`, `volume`.
    pub const AUDIO_EFFECT: &str = "audio.effect";
    /// A weapon release sound played. Subject: the aircraft. Fields: `sound`,
    /// `weapon` (Id).
    pub const AUDIO_RELEASE: &str = "audio.release";
    /// An ejection cue played. Subject: the aircraft. Fields: `sound`.
    pub const AUDIO_EJECTION: &str = "audio.ejection";
    /// A device sound played (gear, flaps, brake, hook, bay). Subject: the
    /// aircraft. Fields: `device` (Text), `sound`.
    pub const AUDIO_DEVICE: &str = "audio.device";
    /// A player command reached the game. Subject: the player's aircraft.
    /// Fields: `command` (Text). Text: what it did.
    pub const PLAYER_COMMAND: &str = "player.command";
    /// The player marked a moment (Ctrl+B). Text: the note, if any.
    pub const PLAYER_BOOKMARK: &str = "player.bookmark";
    /// The game paused.
    pub const SYSTEM_PAUSE: &str = "system.pause";
    /// The game resumed.
    pub const SYSTEM_RESUME: &str = "system.resume";
    /// Time compression changed. Fields: `scale` (Num).
    pub const SYSTEM_TIME_SCALE: &str = "system.time_scale";
    /// A cheat was switched. Fields: `cheat` (Text), `on` (Bool).
    pub const SYSTEM_CHEAT: &str = "system.cheat";
    /// The mission restarted.
    pub const SYSTEM_RESTART: &str = "system.restart";
    /// The mission ended. Fields: `reason` (Text).
    pub const SYSTEM_END: &str = "system.end";
    /// The recorder fell behind and skipped ticks. Fields: `from`, `to`
    /// (Int ticks, the missing range).
    pub const SYSTEM_GAP: &str = "system.gap";
    /// A free-form note, for example what a headless probe does not run.
    pub const SYSTEM_NOTE: &str = "system.note";

    /// Every kind above, in the order listed.
    pub const ALL: &[&str] = &[
        WEAPON_LAUNCH,
        WEAPON_SEEKER_ACTIVE,
        WEAPON_PITBULL,
        WEAPON_TRACK_LOST,
        WEAPON_DECOYED,
        WEAPON_OUTCOME,
        COMBAT_HIT,
        COMBAT_DESTROYED,
        COMBAT_AIRBURST,
        COMBAT_GROUND_IMPACT,
        AIRCRAFT_CRASHED,
        AIRCRAFT_EJECTED,
        AIRCRAFT_PILOT_KILLED,
        AIRCRAFT_TOOK_OFF,
        AIRCRAFT_LANDED,
        AIRCRAFT_FLAMEOUT,
        AIRCRAFT_FUEL_OUT,
        FLIGHT_DEPARTURE,
        FLIGHT_STALL,
        FLIGHT_SPIN,
        FLIGHT_G_LIMIT,
        FLIGHT_STRUCTURAL_FAILURE,
        FLIGHT_EFFECT,
        AI_ACTIVITY,
        AI_TARGET,
        AI_WEAPON_PHASE,
        AI_DEFENSE,
        AI_FALLBACK,
        AI_AIRFIELD_PHASE,
        AI_EJECTION,
        COMMS_ORDER,
        COMMS_REQUEST,
        COMMS_REPORT,
        COMMS_DELIVERY,
        COMMS_RADIO,
        COMMS_CREW,
        COMMS_TOWER,
        COMMS_HUD,
        AUDIO_TONE,
        AUDIO_STALL_WARNING,
        AUDIO_MUSIC,
        AUDIO_EFFECT,
        AUDIO_RELEASE,
        AUDIO_EJECTION,
        AUDIO_DEVICE,
        PLAYER_COMMAND,
        PLAYER_BOOKMARK,
        SYSTEM_PAUSE,
        SYSTEM_RESUME,
        SYSTEM_TIME_SCALE,
        SYSTEM_CHEAT,
        SYSTEM_RESTART,
        SYSTEM_END,
        SYSTEM_GAP,
        SYSTEM_NOTE,
    ];
}

/// Event field names.
pub mod field {
    pub const PROJECTILE: &str = "projectile";
    pub const WEAPON: &str = "weapon";
    pub const CLASS: &str = "class";
    pub const MODE: &str = "mode";
    pub const RANGE_FT: &str = "range_ft";
    pub const ASPECT_DEG: &str = "aspect_deg";
    pub const OFF_BORESIGHT_DEG: &str = "off_boresight_deg";
    pub const CLOSURE_KT: &str = "closure_kt";
    pub const SHOOTER_ALT_FT: &str = "shooter_alt_ft";
    pub const TARGET_ALT_FT: &str = "target_alt_ft";
    pub const SHOOTER_SPEED_KT: &str = "shooter_speed_kt";
    pub const TARGET_SPEED_KT: &str = "target_speed_kt";
    pub const RESULT: &str = "result";
    pub const DAMAGE: &str = "damage";
    pub const HP_AFTER: &str = "hp_after";
    pub const MISS_FT: &str = "miss_ft";
    pub const SECTION: &str = "section";
    pub const DECOY: &str = "decoy";
    pub const REASON: &str = "reason";
    pub const FROM: &str = "from";
    pub const TO: &str = "to";
    pub const ON: &str = "on";
    pub const DIRECTION: &str = "direction";
    pub const ASKED: &str = "asked";
    pub const LIMIT: &str = "limit";
    pub const G: &str = "g";
    pub const AOA_DEG: &str = "aoa_deg";
    pub const SPEED_KT: &str = "speed_kt";
    pub const SINK_FPS: &str = "sink_fps";
    pub const GRADE: &str = "grade";
    pub const AIRPORT: &str = "airport";
    pub const EFFECT: &str = "effect";
    pub const PRIORITY: &str = "priority";
    pub const SCORE: &str = "score";
    pub const STATION: &str = "station";
    pub const THREAT: &str = "threat";
    pub const REACTION: &str = "reaction";
    pub const DECISION: &str = "decision";
    pub const MESSAGE: &str = "message";
    pub const RECIPIENTS: &str = "recipients";
    pub const ORDER: &str = "order";
    pub const ABOUT: &str = "about";
    pub const KEPT_S: &str = "kept_s";
    pub const SPEAKER: &str = "speaker";
    pub const STEMS: &str = "stems";
    pub const ROUTE: &str = "route";
    pub const TRIGGER: &str = "trigger";
    pub const HEARD: &str = "heard";
    pub const OUTCOME: &str = "outcome";
    pub const WAIT_S: &str = "wait_s";
    pub const TONE: &str = "tone";
    pub const SOUND: &str = "sound";
    pub const DEVICE: &str = "device";
    pub const X_FT: &str = "x_ft";
    pub const Y_FT: &str = "y_ft";
    pub const Z_FT: &str = "z_ft";
    pub const VOLUME: &str = "volume";
    pub const COMMAND: &str = "command";
    pub const SCALE: &str = "scale";
    pub const CHEAT: &str = "cheat";
}

/// Display tree channels.
pub mod channel {
    /// An AI aircraft's thinking: mission, activity, target and why,
    /// weapon, defense, motion, steering, controls, fuel, recent changes.
    pub const AI_THOUGHT: &str = "ai.thought";
    /// Flight-model telemetry: air data, load, power, drag, and the effects
    /// applied this tick with their because lines.
    pub const FLIGHT_TELEMETRY: &str = "flight.telemetry";
    /// A guided weapon's seeker and steering.
    pub const WEAPON_GUIDANCE: &str = "weapon.guidance";
}

/// Well-known node labels that exports look for in display trees. Values
/// are numbers in the unit named on the node.
pub mod node {
    /// Height above the ground under the aircraft (`ft`). The summary's
    /// lowest height and Tacview's AGL come from here.
    pub const AGL: &str = "AGL";
    /// Mach number (no unit).
    pub const MACH: &str = "Mach";
    /// Angle of attack (`deg`).
    pub const AOA: &str = "AoA";
    /// Sideslip angle (`deg`).
    pub const SIDESLIP: &str = "Sideslip";
    /// True airspeed (`kt`).
    pub const TAS: &str = "TAS";
    /// Load factor (`g`).
    pub const LOAD: &str = "G";
    /// Structural G limit (`g`).
    pub const G_LIMIT: &str = "G limit";
    /// AI activity (Text).
    pub const ACTIVITY: &str = "Activity";
    /// AI target (Id).
    pub const TARGET: &str = "Target";
}

/// Units for tree nodes and documentation.
pub mod unit {
    pub const FT: &str = "ft";
    pub const FT_S: &str = "ft/s";
    pub const KT: &str = "kt";
    pub const NM: &str = "nm";
    pub const DEG: &str = "deg";
    pub const DEG_S: &str = "deg/s";
    pub const G: &str = "g";
    pub const LB: &str = "lb";
    pub const LB_FT2: &str = "lb/ft2";
    pub const S: &str = "s";
    pub const PERCENT: &str = "%";
    pub const RATIO: &str = "x";
}

/// Outcomes shared by weapon results and comms entries.
pub mod outcome {
    pub const HIT: &str = "hit";
    pub const MISSED: &str = "missed";
    pub const SPOOFED: &str = "spoofed";
    pub const JAMMED: &str = "jammed";
    pub const QUEUED: &str = "queued";
    pub const DELIVERED: &str = "delivered";
    pub const DROPPED: &str = "dropped";
    pub const CANCELLED: &str = "cancelled";
    pub const SUPPRESSED: &str = "suppressed";
    pub const REPLACED: &str = "replaced";
    pub const EXPIRED: &str = "expired";
    pub const APPLIED: &str = "applied";
    pub const REJECTED: &str = "rejected";
}

#[cfg(test)]
mod tests {
    #[test]
    fn kinds_are_unique_dotted_names() {
        let mut seen = std::collections::HashSet::new();
        for kind in super::kind::ALL {
            assert!(seen.insert(*kind), "duplicate kind {kind}");
            let (family, name) = kind.split_once('.').expect("family.name");
            assert!(!family.is_empty() && !name.is_empty());
            assert!(
                kind.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_')
            );
        }
    }
}
