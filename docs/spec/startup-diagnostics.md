# Startup diagnostics

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. **Opinionated** host behavior, requested by John on
2026-09-23: diagnostics built into the executable, visible startup errors on
Windows, and persistent logs on Windows, macOS and Linux. Storage limits,
file names, diagnostic switches and platform interfaces below are agent
decisions. These are not claims about Fighters Anthology behavior.

## Every launch

Logging starts before argument parsing, preferences, media, controllers, audio,
the event loop and graphics. The log records UTC Unix timestamps, elapsed time,
severity, build version, commit identity, target, OS/architecture, executable
location, working directory and resolved data location. It does not dump the
environment, media contents or frame-by-frame simulation state.

Startup stages record a start and successful completion with elapsed time.
They cover settings, asset loading/import, aircraft, audio, terrain, menus,
input, window creation, graphics adapter/device and first presentation. A
failure identifies the unfinished stage and the available error cause chain.
Recoverable failures, including unavailable audio, remain recoverable warnings.
No graphics or flight adapter selection changes as a side effect of logging.

## Files and retention

Default log directories:

| Platform | Directory |
| --- | --- |
| Windows | `%LOCALAPPDATA%\T.O.R.E-Fighters\logs` |
| macOS | `~/Library/Logs/T.O.R.E-Fighters` |
| Linux | `$XDG_STATE_HOME/T.O.R.E-Fighters/logs`, default `~/.local/state/T.O.R.E-Fighters/logs` |

`TORE_LOG_DIR` overrides the directory. An isolated `TORE_DATA_DIR` without a
log override stores logs in that profile's `logs` child. Unwritable or
unavailable storage falls back to a T.O.R.E directory under the OS temporary
directory, then stderr. The actual destination or inability to save is reported
in the failure message. Logs never require writing into the install directory.

Each process owns a uniquely named, stable session file. A separate empty
`.lock` sidecar holds its lifetime lock, so the log remains readable while the
game is running on Windows as well as Unix systems. Retain five recent closed session logs; simultaneous processes must not
overwrite each other's files. A session
keeps at most 5 MiB of routine records, then records that further routine output
was suppressed. Fatal reports have a separate 256 KiB file and remain available
after the routine limit. Retain five closed fatal files independently. Routine
records are capped at 16 KiB; each fatal reason/backtrace is capped at 32 KiB.
Retention probes sidecar locks so killed sessions can be reclaimed without
removing another live process's output. Unlocked sidecars are removed after
both their session log and fatal report have been removed. Maintenance lock contention waits at
most 20 ms before falling back or skipping cleanup. Records are bounded and
repeated backend warnings
must not produce unlimited disk use. Startup milestones and fatal reports are
written promptly without relying on normal process shutdown.

The most recent fatal summary is also written to `last-error.txt` in the data
directory for compatibility. Successful starts, help and version queries do
not delete a previous error. Each report includes its timestamp and session
log path to distinguish old failures. Logger failures do not stop gameplay.

## Errors and panics

Returned fatal errors and Rust panics are recorded with build identity, active
stage and available causes. Panic reports include thread, source location and
a best-effort backtrace without requiring `RUST_BACKTRACE`. Panic logging must
not wait for its own held mutex or recursively panic. Worker panics are logged;
normal worker failure handling determines whether the application must stop.
The app does not attempt to resume gameplay after a main-thread panic.

Interactive Windows failures use a native error dialog independent of winit,
wgpu and imported assets. macOS uses an AppKit alert on the main thread. Linux
uses stderr and persistent files, plus optional `notify-send` with a
10-second display hint and a 2-second delivery-process timeout. A message names the failed stage, reason and actual report location.
Headless, capture, validation and diagnostic modes never block on a dialog.
`TORE_NO_ERROR_DIALOG=1` suppresses dialogs for unattended launches.

Windows fatal reports also attempt an Application event with source
`T.O.R.E-Fighters`, event ID 1000, and the build, reason and report path. Event summaries put
paths first and limit the reason to 4 KiB; full available backtraces stay local. The MSI
registers the source and the executable's embedded message resource. The game
never asks for elevation to log. Event logging failure leaves file reporting
and the dialog available.

## Diagnostic and release checks

`--diagnostics-self-test` runs a media-free success check and exits zero.
`--diagnostics-self-test=error`, `=panic` and `=worker-panic` deliberately
exercise reporting and exit nonzero without dialogs. `=graphics` presents a
synthetic first-run canvas and exits; it needs a display/GPU but no retail data.
`=dialog` deliberately shows the native error UI for manual acceptance.
Diagnostic failures are plainly labelled as deliberate tests.

Release builds carry version, commit and target metadata and retain matching
debug symbols as separate build artifacts. Package checks launch staged
executables with an isolated profile, validate deliberate failure reports and
check runtime library dependencies. Installed Windows shortcut/direct launches,
macOS Finder launches and Linux desktop launches remain explicit acceptance
checks; build success does not substitute for them.

Missing libraries, OS security blocking, access violations, process kills and
other failures before or outside Rust reporting may leave only the last stage,
or no application log at all. OS crash/event reports and installation evidence
are needed in those cases. No automatic upload or crash-dump collection is
enabled by this feature.
