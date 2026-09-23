use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn show_error(title: &str, message: &str) -> Result<(), String> {
    // Optional desktop utility, invoked directly with arguments, never a shell.
    // No --wait: acknowledgement must never keep a crashed game alive.
    let mut child = Command::new("notify-send")
        .args([
            "--app-name=T.O.R.E-Fighters",
            "--urgency=critical",
            "--expire-time=10000",
            "--",
            title,
            message,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("desktop notification unavailable: {error}"))?;
    await_delivery(&mut child, Duration::from_secs(2))
}

fn await_delivery(child: &mut std::process::Child, limit: Duration) -> Result<(), String> {
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!("desktop notification: {status}"))
                };
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match result {
                    Err(error) => format!("desktop notification: {error}"),
                    _ => "desktop notification timed out".into(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hanging_delivery_is_killed_and_reaped() {
        let mut child = Command::new("sleep").arg("60").spawn().unwrap();
        let start = Instant::now();
        assert!(
            await_delivery(&mut child, Duration::from_millis(20))
                .unwrap_err()
                .contains("timed out")
        );
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn failed_delivery_is_reported() {
        let mut child = Command::new("false").spawn().unwrap();
        assert!(await_delivery(&mut child, Duration::from_secs(1)).is_err());
    }
}
