//! The formation flight trace file (`TORE_FORMATION_TRACE`, see
//! docs/DEVELOPMENT.md, "Formation flight traces").
//!
//! The mission only collects rows ([`FormationBatch`]); this writer, which
//! lives in the app, reads the variable, opens the file and writes them. Nothing
//! here reaches back into the simulation.
use crate::ai_wings::{AiWings, FormationBatch, FormationRow};
use std::io::{BufWriter, Write};

const HEADER: &str = "tick,actor,phase,phase_seconds,slot_distance_ft,closure_fps,altitude_error_ft,predicted_separation_ft,yielding_to,x,y,z,speed_fps,bank_deg,g,pitch_input,roll_input,yaw_input,throttle,burner,aim_x,aim_y,aim_z";

/// An open trace file with its header written.
pub struct FormationTrace<W: Write = std::fs::File> {
    log: BufWriter<W>,
}

impl FormationTrace {
    /// Open the file named by `TORE_FORMATION_TRACE`, appending, and write the
    /// header. `None` when the variable is unset. The parent directory must
    /// exist; a file that cannot be opened is an error.
    pub fn from_env() -> std::io::Result<Option<Self>> {
        let Some(path) = std::env::var_os("TORE_FORMATION_TRACE") else {
            return Ok(None);
        };
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Self::new(file).map(Some)
    }
}

impl<W: Write> FormationTrace<W> {
    fn new(sink: W) -> std::io::Result<Self> {
        let mut log = BufWriter::new(sink);
        writeln!(log, "{HEADER}")?;
        Ok(Self { log })
    }

    /// Write a drained batch: its rows in order, then a flush when it says so.
    fn write(&mut self, batch: &FormationBatch) -> std::io::Result<()> {
        for row in &batch.rows {
            writeln!(self.log, "{}", line(row))?;
        }
        if batch.flush {
            self.log.flush()?;
        }
        Ok(())
    }
}

/// Start tracing a mission that was just built: open the file if the variable
/// is set and switch the wings' collection on. A file that cannot be opened is
/// an error, so the flight does not start. Call this after the previous
/// mission's trace (if any) has been dropped, so its last rows land first.
pub fn start(wings: &mut AiWings) -> std::io::Result<Option<FormationTrace>> {
    let trace = FormationTrace::from_env()?;
    wings.set_formation_trace(trace.is_some());
    Ok(trace)
}

/// Move what the wings collected this tick into the file. A write failure is
/// reported to stderr and switches tracing off without stopping the flight.
pub fn drain(trace: &mut Option<FormationTrace>, wings: &mut AiWings) {
    let batch = wings.take_formation_trace();
    let Some(open) = trace.as_mut() else {
        return;
    };
    if let Err(error) = open.write(&batch) {
        eprintln!("Formation trace disabled after write failure: {error}");
        *trace = None;
        wings.set_formation_trace(false);
    }
}

fn line(row: &FormationRow) -> String {
    let trace = &row.trace;
    format!(
        "{},{},{:?},{:.3},{:.2},{:.2},{:.2},{:.2},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.3},{:.4},{:.4},{:.4},{:.4},{},{:.2},{:.2},{:.2}",
        row.tick,
        row.actor,
        trace.phase,
        trace.phase_seconds,
        trace.slot_distance_ft,
        trace.closure_fps,
        trace.altitude_error_ft,
        trace.minimum_predicted_separation_ft,
        trace.yielding_to.map_or(String::new(), |id| id.to_string()),
        row.position[0],
        row.position[1],
        row.position[2],
        row.speed,
        row.bank.to_degrees(),
        row.g,
        row.pitch_input,
        row.roll_input,
        row.yaw_input,
        row.throttle,
        row.afterburner,
        trace.aim[0],
        trace.aim[1],
        trace.aim[2]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::ai::formation::{Phase, Trace};

    fn row(tick: u64, yielding_to: Option<u32>) -> FormationRow {
        FormationRow {
            tick,
            actor: 2,
            trace: Trace {
                phase: Phase::default(),
                phase_seconds: 1.23456,
                slot_distance_ft: 6.5849,
                closure_fps: -0.04,
                altitude_error_ft: 0.111,
                minimum_predicted_separation_ft: 729.125,
                yielding_to,
                aim: [1071010.171, 5000.114, 591614.829],
                planned_velocity: None,
            },
            position: [1070360.271, 5000., 589432.104],
            speed: 759.2,
            bank: -0.0075,
            g: 1.0013,
            pitch_input: 0.00011,
            roll_input: -0.15634,
            yaw_input: 0.,
            throttle: 0.81434,
            afterburner: false,
        }
    }

    #[test]
    fn a_row_is_one_csv_line_matching_the_header() {
        let text = line(&row(12, None));
        assert_eq!(
            text,
            format!(
                "12,2,{:?},1.235,6.58,-0.04,0.11,729.12,,1070360.27,5000.00,589432.10,759.20,-0.43,1.001,0.0001,-0.1563,0.0000,0.8143,false,1071010.17,5000.11,591614.83",
                Phase::default()
            )
        );
        assert_eq!(text.split(',').count(), HEADER.split(',').count());
        assert!(line(&row(24, Some(7))).contains(",7,1070360.27,"));
    }

    #[test]
    fn a_batch_writes_its_rows_and_flushes_only_when_told() {
        let mut trace = FormationTrace::new(Vec::new()).unwrap();
        trace
            .write(&FormationBatch {
                rows: vec![row(12, None), row(12, Some(1))],
                flush: false,
            })
            .unwrap();
        assert!(trace.log.get_ref().is_empty(), "rows wait in the buffer");
        trace
            .write(&FormationBatch {
                rows: vec![row(120, None)],
                flush: true,
            })
            .unwrap();
        let text = String::from_utf8(trace.log.get_ref().clone()).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], HEADER);
        assert!(lines[1].starts_with("12,2,"));
        assert!(lines[3].starts_with("120,2,"));
    }
}
