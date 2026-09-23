//! Media-free diagnostic entry points and unattended error-dialog policy.
//! Opinionated host behavior: docs/spec/startup-diagnostics.md.
use crate::{AppResult, canvas_present::CanvasPresenter, diagnostics};
use std::{
    ffi::OsString,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::{Window, WindowId},
};

pub(crate) fn interactive() -> bool {
    if std::env::var_os("TORE_NO_ERROR_DIALOG").is_some_and(|v| v == "1") {
        return false;
    }
    if [
        "TORE_ENVIRONMENT_PROBE",
        "TORE_AIRPORT_PROBE",
        "TORE_PERF_FRAMES",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some())
    {
        return false;
    }
    interactive_arguments(std::env::args_os().skip(1))
}

fn interactive_arguments(args: impl Iterator<Item = OsString>) -> bool {
    !args.filter_map(|arg| arg.into_string().ok()).any(|arg| {
        (arg.starts_with("--diagnostics-self-test") && arg != "--diagnostics-self-test=dialog")
            || matches!(
                arg.as_str(),
                "--help"
                    | "-h"
                    | "--version"
                    | "-V"
                    | "--import-only"
                    | "--headless-flight"
                    | "--snapshot"
                    | "--panel-snapshot"
                    | "--capture-terrain"
                    | "--capture-flight"
                    | "--smoke-test"
                    | "--replay-input"
                    | "--replay-combat"
                    | "--combat-smoke"
                    | "--combat-probe-ticks"
                    | "--missile-acceptance"
                    | "--ai-probe-ticks"
                    | "--ai-roster-probe-ticks"
                    | "--sensor-summary"
                    | "--validate-creator"
                    | "--validate-weather"
                    | "--validate-maps"
                    | "--native-flight-report"
                    | "--list-inputs"
                    | "--monitor-inputs"
                    | "--test-rumble"
                    | "--write-input-profile"
                    | "--airport-probe"
            )
    })
}

pub(crate) fn self_test() -> Option<AppResult<()>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mode = args.iter().filter_map(|s| s.to_str()).find_map(|s| {
        if s == "--diagnostics-self-test" {
            Some("success")
        } else {
            s.strip_prefix("--diagnostics-self-test=")
        }
    })?;
    Some(if args.len() != 1 {
        Err("--diagnostics-self-test must be used alone".into())
    } else {
        run_self_test(mode)
    })
}

fn run_self_test(mode: &str) -> AppResult<()> {
    diagnostics::stage("diagnostics self-test");
    log::info!("Deliberate diagnostics self-test: {mode}");
    match mode {
        "success" => {
            diagnostics::stage_done();
            println!("Diagnostics self-test: PASS; log={:?}", diagnostics::log_path());
            Ok(())
        }
        "error" | "dialog" => Err(std::io::Error::other(
            "Deliberate diagnostics self-test error; no game data was loaded",
        ).into()),
        "panic" => panic!("Deliberate diagnostics self-test panic"),
        "worker-panic" => {
            let worker = std::thread::Builder::new().name("diagnostics-test-worker".into())
                .spawn(|| panic!("Deliberate diagnostics self-test worker panic"))?;
            if worker.join().is_err() {
                return Err("Deliberate diagnostics self-test worker panic was recorded".into());
            }
            Err("diagnostics worker unexpectedly returned normally".into())
        }
        "graphics" => graphics_self_test(),
        _ => Err("--diagnostics-self-test expects success, error, panic, worker-panic, graphics or dialog".into()),
    }
}

fn graphics_self_test() -> AppResult<()> {
    diagnostics::stage("diagnostic event loop creation");
    let mut event_loop = EventLoop::new()?;
    diagnostics::stage_done();
    let mut app = GraphicsCheck {
        presenter: None,
        error: None,
        presented: false,
        deadline: Instant::now() + Duration::from_secs(15),
    };
    event_loop.run_app_on_demand(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    if !app.presented {
        return Err("diagnostic graphics window closed before a frame was presented".into());
    }
    log::info!("Diagnostics graphics self-test: PASS");
    Ok(())
}

struct GraphicsCheck {
    presenter: Option<CanvasPresenter>,
    error: Option<Box<dyn std::error::Error>>,
    presented: bool,
    deadline: Instant,
}

impl ApplicationHandler for GraphicsCheck {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.presenter.is_some() {
            return;
        }
        let result = (|| -> AppResult<_> {
            diagnostics::stage("diagnostic window creation");
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("T.O.R.E-Fighters graphics diagnostic")
                        .with_inner_size(LogicalSize::new(640., 480.)),
                )?,
            );
            diagnostics::stage_done();
            pollster::block_on(CanvasPresenter::new(window))
        })();
        match result {
            Ok(presenter) => {
                presenter.window.request_redraw();
                self.presenter = Some(presenter);
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                let Some(presenter) = &mut self.presenter else {
                    return;
                };
                let mut pixels = vec![0; 640 * 480 * 4];
                for (i, pixel) in pixels.chunks_exact_mut(4).enumerate() {
                    pixel.copy_from_slice(&[32, (i % 640 / 3) as u8, (i / 640 / 2) as u8, 255]);
                }
                match presenter.present(&pixels) {
                    Ok(true) => {
                        self.presented = true;
                        event_loop.exit();
                    }
                    Ok(false) => presenter.window.request_redraw(),
                    Err(error) => {
                        self.error = Some(error);
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if Instant::now() >= self.deadline {
            self.error = Some("diagnostic graphics presentation timed out after 15 seconds".into());
            event_loop.exit();
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unattended_modes_never_request_dialogs_even_with_malformed_arguments() {
        for mode in [
            "--headless-flight",
            "--snapshot",
            "--import-only",
            "--capture-flight",
            "--smoke-test",
            "--version",
            "--help",
            "--list-inputs",
            "--diagnostics-self-test=panic",
            "--diagnostics-self-test=graphics",
        ] {
            assert!(
                !interactive_arguments([mode, "invalid"].into_iter().map(OsString::from)),
                "{mode}"
            );
        }
        assert!(interactive_arguments(std::iter::empty()));
        assert!(interactive_arguments(
            [OsString::from("--diagnostics-self-test=dialog")].into_iter()
        ));
        assert!(interactive_arguments(
            [OsString::from("--windowed")].into_iter()
        ));
    }
}

/// Track first presentation without emitting a record for every redraw retry.
#[derive(Default)]
pub(crate) enum FirstFrame {
    #[default]
    Pending,
    Waiting,
    Presented,
}
impl FirstFrame {
    pub(crate) fn begin(&mut self, stage: &'static str) {
        if matches!(self, Self::Pending) {
            diagnostics::stage(stage);
            *self = Self::Waiting;
        }
    }
    pub(crate) fn complete(&mut self) {
        if matches!(self, Self::Waiting) {
            diagnostics::stage_done();
            log::info!("First frame presented successfully");
            *self = Self::Presented;
        }
    }
}
