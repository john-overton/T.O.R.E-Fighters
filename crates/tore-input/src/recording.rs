//! Bounded pilot-input tape. Replay requires the same initial state, model and environment.
use crate::{Action, FlightAxis, PilotCommand, PilotInput};
use std::io::{self, BufRead, Write};
pub const HEADER: &str = "tore-pilot 2";
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid pilot input tape")
}
fn switch_name(s: crate::Switch) -> &'static str {
    use crate::Switch::*;
    match s {
        Gear => "gear",
        Flaps => "flaps",
        Airbrake => "airbrake",
        Hook => "hook",
        Bay => "bay",
        Engine => "engine",
        Burner => "burner",
        Radar => "radar",
        Jammer => "jammer",
        Autopilot => "autopilot",
        WaypointAutopilot => "waypoint-autopilot",
    }
}
pub fn write_frame(mut out: impl Write, tick: u64, input: &PilotInput) -> io::Result<()> {
    if tick == 0 || tick > 432000 || !valid(input) {
        return Err(invalid());
    }
    write!(
        out,
        "{tick} {} {} {} {} {}",
        input.pitch,
        input.roll,
        input.yaw,
        input.throttle_rate,
        input
            .throttle
            .map(|n| n.to_string())
            .unwrap_or_else(|| "-".into())
    )?;
    for (axis, rate, position) in [
        (
            FlightAxis::VectorPitch,
            input.vector_pitch_rate,
            input.vector_pitch,
        ),
        (
            FlightAxis::VectorYaw,
            input.vector_yaw_rate,
            input.vector_yaw,
        ),
        (
            FlightAxis::Conversion,
            input.conversion_rate,
            input.conversion,
        ),
        (
            FlightAxis::Collective,
            input.collective_rate,
            input.collective,
        ),
    ] {
        write!(
            out,
            " lift:{}:{rate}:{}",
            crate::profile_text::flight_axis_name(axis),
            position
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".into())
        )?;
    }
    for command in &input.commands {
        let s = match command {
            PilotCommand::NeutralVector => "neutral-vector".into(),
            PilotCommand::SetAxis(axis, value) => format!(
                "axis-set:{}:{value}",
                crate::profile_text::flight_axis_name(*axis)
            ),
            PilotCommand::AdjustAxis(axis, value) => format!(
                "axis-adjust:{}:{value}",
                crate::profile_text::flight_axis_name(*axis)
            ),
            PilotCommand::Eject => "eject".into(),
            PilotCommand::Toggle(s) => format!("toggle:{}", switch_name(*s)),
            PilotCommand::Set(s, on) => format!("set:{}:{}", switch_name(*s), u8::from(*on)),
            PilotCommand::Throttle(v) => format!("throttle:{v}"),
            PilotCommand::AdjustThrottle(v) => format!("adjust:{v}"),
        };
        write!(out, " {s}")?;
    }
    writeln!(out)
}
fn valid(i: &PilotInput) -> bool {
    [
        i.pitch,
        i.roll,
        i.yaw,
        i.throttle_rate,
        i.vector_pitch_rate,
        i.vector_yaw_rate,
        i.conversion_rate,
        i.collective_rate,
    ]
    .iter()
    .all(|v| v.is_finite() && (-1. ..=1.).contains(v))
        && i.throttle
            .is_none_or(|v| v.is_finite() && (0. ..=1.).contains(&v))
        && [i.vector_pitch, i.conversion, i.collective]
            .iter()
            .all(|v| v.is_none_or(|v| v.is_finite() && (0. ..=1.).contains(&v)))
        && i.vector_yaw
            .is_none_or(|v| v.is_finite() && (-1. ..=1.).contains(&v))
        && i.commands.len() <= 256
        && i.commands.iter().all(|c| match c {
            PilotCommand::SetAxis(axis, v) => {
                v.is_finite()
                    && ((if *axis == FlightAxis::VectorYaw {
                        -1.
                    } else {
                        0.
                    })..=1.)
                        .contains(v)
            }
            PilotCommand::AdjustAxis(_, v) => v.is_finite() && (-1. ..=1.).contains(v),
            PilotCommand::Throttle(v) => v.is_finite() && (0. ..=1.).contains(v),
            PilotCommand::AdjustThrottle(v) => v.is_finite() && (-1. ..=1.).contains(v),
            _ => true,
        })
}
pub fn read(mut input: impl BufRead) -> io::Result<Vec<PilotInput>> {
    let mut frames = vec![];
    let mut bytes = 0usize;
    let mut buffer = Vec::new();
    let mut header = false;
    loop {
        buffer.clear();
        // Take limits a malicious unterminated line before allocating it in full.
        use std::io::Read;
        let n = input.by_ref().take(32769).read_until(b'\n', &mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n;
        if n > 32768 || bytes > 64 * 1024 * 1024 {
            return Err(invalid());
        }
        let line = std::str::from_utf8(&buffer).map_err(|_| invalid())?.trim();
        if !header {
            if line != HEADER && line != "tore-pilot 1" {
                return Err(invalid());
            }
            header = true;
            continue;
        }
        if frames.len() >= 432000 {
            return Err(invalid());
        }
        let w: Vec<_> = line.split_whitespace().collect();
        if w.len() < 6 || w.len() > 266 || w[0].parse::<usize>().ok() != Some(frames.len() + 1) {
            return Err(invalid());
        }
        let number = |s: &str| s.parse::<f64>().map_err(|_| invalid());
        let mut frame = PilotInput {
            pitch: number(w[1])?,
            roll: number(w[2])?,
            yaw: number(w[3])?,
            throttle_rate: number(w[4])?,
            throttle: if w[5] == "-" {
                None
            } else {
                Some(number(w[5])?)
            },
            ..Default::default()
        };
        let mut lift_seen = std::collections::BTreeSet::new();
        for text in &w[6..] {
            let p: Vec<_> = text.split(':').collect();
            if let ["lift", name, rate, position] = p.as_slice() {
                if !lift_seen.insert(*name) {
                    return Err(invalid());
                }
                let pair = match *name {
                    "vector-pitch" => (&mut frame.vector_pitch_rate, &mut frame.vector_pitch),
                    "vector-yaw" => (&mut frame.vector_yaw_rate, &mut frame.vector_yaw),
                    "conversion" => (&mut frame.conversion_rate, &mut frame.conversion),
                    "collective" => (&mut frame.collective_rate, &mut frame.collective),
                    _ => return Err(invalid()),
                };
                *pair.0 = number(rate)?;
                *pair.1 = if *position == "-" {
                    None
                } else {
                    Some(number(position)?)
                };
                continue;
            }
            let command = match p.as_slice() {
                ["neutral-vector"] => PilotCommand::NeutralVector,
                [kind @ ("axis-set" | "axis-adjust"), name, value] => {
                    let token = if *kind == "axis-set" {
                        format!("{name}={value}")
                    } else {
                        format!("{name}-step={value}")
                    };
                    match Action::parse(&token).map_err(|_| invalid())? {
                        Action::Pilot(
                            c @ (PilotCommand::SetAxis(_, _) | PilotCommand::AdjustAxis(_, _)),
                        ) => c,
                        _ => return Err(invalid()),
                    }
                }
                ["eject"] => PilotCommand::Eject,
                ["throttle", v] => PilotCommand::Throttle(number(v)?),
                ["adjust", v] => PilotCommand::AdjustThrottle(number(v)?),
                ["toggle", s] => match Action::parse(s).map_err(|_| invalid())? {
                    Action::Pilot(PilotCommand::Toggle(s)) => PilotCommand::Toggle(s),
                    _ => return Err(invalid()),
                },
                ["set", s, v] if matches!(*v, "0" | "1") => {
                    match Action::parse(s).map_err(|_| invalid())? {
                        Action::Pilot(PilotCommand::Toggle(s)) => PilotCommand::Set(s, *v == "1"),
                        _ => return Err(invalid()),
                    }
                }
                _ => return Err(invalid()),
            };
            frame.commands.push(command);
        }
        if !valid(&frame) {
            return Err(invalid());
        }
        frames.push(frame);
    }
    if !header {
        return Err(invalid());
    }
    Ok(frames)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_retains_fractional_axes_and_command_order() {
        let frame = PilotInput {
            pitch: 0.314159,
            roll: -0.271828,
            throttle: Some(0.4),
            commands: vec![
                PilotCommand::Toggle(crate::Switch::Autopilot),
                PilotCommand::Set(crate::Switch::WaypointAutopilot, true),
                PilotCommand::Toggle(crate::Switch::Gear),
                PilotCommand::Set(crate::Switch::Gear, true),
                PilotCommand::Throttle(0.8),
                PilotCommand::Eject,
                PilotCommand::Toggle(crate::Switch::Bay),
                PilotCommand::Set(crate::Switch::Bay, true),
            ],
            ..Default::default()
        };
        let mut bytes = format!("{HEADER}\n").into_bytes();
        write_frame(&mut bytes, 1, &frame).unwrap();
        assert_eq!(read(bytes.as_slice()).unwrap(), vec![frame]);
    }
    #[test]
    fn lift_tape_preserves_demands_and_reads_legacy_frames() {
        let frame = PilotInput {
            vector_pitch_rate: -0.25,
            vector_yaw: Some(-0.75),
            conversion: Some(0.),
            collective: Some(1.),
            collective_rate: 0.3,
            commands: vec![
                PilotCommand::SetAxis(FlightAxis::Collective, 0.6),
                PilotCommand::AdjustAxis(FlightAxis::Conversion, -0.1),
                PilotCommand::NeutralVector,
            ],
            ..Default::default()
        };
        let mut bytes = format!("{HEADER}\n").into_bytes();
        write_frame(&mut bytes, 1, &frame).unwrap();
        assert_eq!(read(bytes.as_slice()).unwrap(), vec![frame]);
        assert_eq!(
            read(b"tore-pilot 1\n1 0 0 0 0 -\n".as_slice()).unwrap(),
            vec![PilotInput::default()]
        );
        for line in [
            "lift:collective:0:-0.1",
            "lift:vector-yaw:NaN:0",
            "axis-set:collective:2",
            "lift:conversion:0:0 lift:conversion:0:1",
        ] {
            assert!(read(format!("{HEADER}\n1 0 0 0 0 - {line}\n").as_bytes()).is_err());
        }
    }
    #[test]
    fn rejects_nonfinite_invalid_ticks_and_overlong_lines() {
        for s in [
            "",
            "tore-pilot 3\n",
            "tore-pilot 1\n2 0 0 0 0 -\n",
            "tore-pilot 1\n1 NaN 0 0 0 -\n",
            "tore-pilot 1\n1 0 0 0 0 - set:gear:2\n",
        ] {
            assert!(read(s.as_bytes()).is_err());
        }
        assert!(read(format!("{HEADER}\n{}", "x".repeat(32769)).as_bytes()).is_err());
    }
}
