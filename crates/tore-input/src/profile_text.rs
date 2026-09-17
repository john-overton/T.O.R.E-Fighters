//! Canonical profile output shared by the in-game editor and its persistence path.
use crate::*;
pub fn action_name(action: &Action) -> String {
    match action {
        Action::Axis(a) => match a {
            Axis::Pitch => "pitch",
            Axis::Roll => "roll",
            Axis::Yaw => "yaw",
            Axis::Throttle => "throttle",
            Axis::ThrottleRate => "throttle-rate",
            Axis::LookX => "look-x",
            Axis::LookY => "look-y",
        }
        .into(),
        Action::Pilot(PilotCommand::Toggle(s)) => match s {
            Switch::Gear => "gear",
            Switch::Flaps => "flaps",
            Switch::Airbrake => "airbrake",
            Switch::Hook => "hook",
            Switch::Bay => "bay",
            Switch::Engine => "engine",
            Switch::Burner => "burner",
            Switch::Radar => "radar",
            Switch::Jammer => "jammer",
        }
        .into(),
        Action::Pilot(PilotCommand::Throttle(v)) => format!("throttle={v}"),
        Action::Ui(s) => s.clone(),
        _ => "unsupported".into(),
    }
}
pub fn mode_name(mode: Mode) -> String {
    match mode {
        Mode::Axis => "axis".into(),
        Mode::Unit => "unit".into(),
        Mode::Hold(s) => if s < 0. { "negative" } else { "positive" }.into(),
        Mode::Trigger(s) => if s < 0. {
            "trigger-negative"
        } else {
            "trigger-positive"
        }
        .into(),
        Mode::HoldState => "hold".into(),
        Mode::Press => "press".into(),
        Mode::Release => "release".into(),
        Mode::Switch => "switch".into(),
        Mode::Follow => "follow".into(),
        Mode::Position(n) => format!("position={n}"),
        Mode::Delta => "delta".into(),
    }
}
impl Profile {
    pub fn to_text(&self) -> Result<String, String> {
        let mut text = format!(
            "tore-input 1\nrumble {}\n",
            if self.rumble { "on" } else { "off" }
        );
        text.push_str(&format!(
            "gamepad-defaults {}\n",
            if self.gamepad_defaults { "on" } else { "off" }
        ));
        for (alias, id) in &self.aliases {
            text.push_str(&format!("alias {alias} {id}\n"));
        }
        for b in &self.bindings {
            let c = b.calibration;
            text.push_str(&format!(
                "bind {} {} {} {} {} {} {} {} {} {} {}\n",
                b.device,
                b.control,
                action_name(&b.action),
                mode_name(b.mode),
                c.min,
                c.center,
                c.max,
                c.deadzone,
                c.curve,
                c.scale,
                b.priority
            ));
        }
        Self::parse(&text)?;
        Ok(text)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_preserves_aliases_calibration_and_behaviors() {
        let p=Profile::parse("tore-input 1\nrumble on\nalias pad device\nbind pad x roll axis -0.9 0.1 0.8 0.12 1.7 -1 30\nbind pad t throttle unit\nbind pad b gear follow\nbind pad hat instrument-next position=-1\nbind keyboard Ctrl-g gear press\nbind keyboard Shift-o bay press\n").unwrap();
        let text = p.to_text().unwrap();
        assert_eq!(Profile::parse(&text).unwrap().to_text().unwrap(), text);
        assert!(text.contains("0.12 1.7 -1 30"));
    }
}
