//! The combat tape's vocabulary: the stable names of the commands and airport
//! requests it records, and the record `Combat` collects while a tape is being
//! written. The file writer and the reader are the app's.
use tore_sim::combat::live::{Command, Launcher};

pub fn airport_command_name(command: tore_sim::airport::Command) -> String {
    use tore_sim::airport::Command;
    match command {
        Command::SelectAirport(id) => format!("airport-select:{id}"),
        Command::RequestLanding => "airport-request".into(),
        Command::RepeatReply => "airport-repeat".into(),
        Command::CancelApproach => "airport-cancel".into(),
    }
}

pub fn airport_command(text: &str) -> Option<tore_sim::airport::Command> {
    use tore_sim::airport::Command;
    if let Some(id) = text.strip_prefix("airport-select:") {
        return id.parse().ok().map(Command::SelectAirport);
    }
    match text {
        "airport-request" => Some(Command::RequestLanding),
        "airport-repeat" => Some(Command::RepeatReply),
        "airport-cancel" => Some(Command::CancelApproach),
        _ => None,
    }
}

/// One record of the tape: the action's name and the launcher it saw.
/// `Combat` collects them; the app writes them to the tape file.
pub struct Entry {
    pub action: String,
    pub launcher: Launcher,
}
pub fn command_name(c: Command) -> String {
    // A designation carries its stable target identity, never a screen
    // coordinate, so a replay selects the same object.
    if let Command::TargetDistance(value) = c {
        return format!("target-distance:{value}");
    }
    if let Command::TargetHeat(value) = c {
        return format!("target-heat:{value}");
    }
    if let Command::DesignateTarget(id) = c {
        return format!("designate-id:{id}");
    }
    match c {
        Command::NextWeapon => "next",
        Command::NextGunGroup => "gun-group-next",
        Command::ToggleGunGroup => "gun-group-toggle",
        Command::NextSelection => "selection-next",
        Command::PreviousSelection => "selection-previous",
        Command::SelectNav => "selection-nav",
        Command::AdvanceFromEmpty => "selection-dry",
        Command::ToggleSeekerMode => "seeker-mode",
        Command::CompatibilityWeapons => "compatibility-weapons",
        Command::ToggleTargetRadar => "target-radar",
        Command::TargetHeat(_) | Command::TargetDistance(_) => unreachable!("handled above"),
        Command::ClearRange => "empty-range",
        Command::Designate => "designate",
        Command::DesignatePrevious => "designate-previous",
        Command::DesignateVisual => "designate-visual",
        Command::ClearDesignation => "clear",
        Command::ToggleArm => "arm",
        Command::Jettison => "jettison",
        Command::ReplaceTarget => "target",
        Command::CycleClass => "class",
        Command::FailStation => "fail",
        Command::DamagePlayer => "damage",
        Command::Incoming => "incoming",
        Command::ToggleTargetJammer => "target-jammer",
        Command::ReleaseChaff => "chaff",
        Command::ReleaseFlare => "flare",
        Command::DesignateTarget(_) => unreachable!("handled above"),
    }
    .into()
}
pub fn command(s: &str) -> Option<Command> {
    if let Some(value) = s.strip_prefix("target-distance:") {
        return value
            .parse::<u32>()
            .ok()
            .filter(|v| (1..=1_000_000).contains(v))
            .map(Command::TargetDistance);
    }
    if let Some(value) = s.strip_prefix("target-heat:") {
        return value
            .parse::<u8>()
            .ok()
            .filter(|v| *v <= 4)
            .map(Command::TargetHeat);
    }
    if let Some(id) = s.strip_prefix("designate-id:") {
        return id.parse().ok().map(Command::DesignateTarget);
    }
    [
        Command::NextWeapon,
        Command::NextGunGroup,
        Command::ToggleGunGroup,
        Command::NextSelection,
        Command::PreviousSelection,
        Command::SelectNav,
        Command::AdvanceFromEmpty,
        Command::ToggleSeekerMode,
        Command::CompatibilityWeapons,
        Command::ToggleTargetRadar,
        Command::ClearRange,
        Command::Designate,
        Command::DesignatePrevious,
        Command::DesignateVisual,
        Command::ClearDesignation,
        Command::ToggleArm,
        Command::Jettison,
        Command::ReplaceTarget,
        Command::CycleClass,
        Command::FailStation,
        Command::DamagePlayer,
        Command::Incoming,
        Command::ToggleTargetJammer,
        Command::ReleaseChaff,
        Command::ReleaseFlare,
    ]
    .into_iter()
    .find(|c| command_name(*c) == s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_dry_station_hand_on_round_trips_through_a_tape() {
        use tore_sim::combat::live::Command;
        assert_eq!(command_name(Command::AdvanceFromEmpty), "selection-dry");
        assert_eq!(command("selection-dry"), Some(Command::AdvanceFromEmpty));
    }
    #[test]
    fn airport_commands_have_stable_bounded_names() {
        use tore_sim::airport::Command;
        for command in [
            Command::SelectAirport(17),
            Command::RequestLanding,
            Command::RepeatReply,
            Command::CancelApproach,
        ] {
            assert_eq!(
                airport_command(&airport_command_name(command)),
                Some(command)
            );
        }
        assert_eq!(airport_command("airport-select:not-a-number"), None);
    }
    #[test]
    fn countermeasure_commands_have_stable_names() {
        for (command, name) in [
            (Command::NextGunGroup, "gun-group-next"),
            (Command::ToggleGunGroup, "gun-group-toggle"),
            (Command::ReleaseChaff, "chaff"),
            (Command::ReleaseFlare, "flare"),
        ] {
            assert_eq!(command_name(command), name);
            assert_eq!(super::command(name), Some(command));
        }
    }
    #[test]
    fn designations_carry_the_target_identity() {
        assert_eq!(command("designate-id:7"), Some(Command::DesignateTarget(7)));
        assert_eq!(command("designate-id:x"), None);
        assert_eq!(command_name(Command::DesignateTarget(7)), "designate-id:7");
    }
}
