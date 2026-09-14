# Linux setup — 2026-09-14

Base source revision: `c7347b5b751f314114176eeb3380cf84f79b05c2`, plus the
renderer shutdown fix recorded here. Host: Omarchy 4.0.2, x86_64, Ryzen 9 7900X,
61 GiB reported RAM, Wayland desktop. Python 3.14.7 and native C, ALSA,
XKB, Wayland and X11 build prerequisites were already available. Installed
rustup-managed Rust 1.91.1 with rustfmt and Clippy; the installer configured the
user's shell startup files. Cargo.lock and dependencies were unchanged.

Cloned `https://github.com/john-overton/USNF-ATF.git` into ignored `USNF-ATF/`,
at `2d818054ff51db9f3353d0548dbd0e469b275a1a`. Copied the user's MacBook
checkout's `gameassets/` into ignored local `gameassets/` with `rsync -a --partial`,
including Fighters Anthology media and reference photos: 3,150 regular files,
1,344,517,843 bytes. A subsequent `rsync -acni --stats` checksum comparison
reported no differences or transfers. The four top-level FA LIB archives also
matched source SHA-256 sums. Source media was not modified.

`cargo run --locked -p tore-app -- --import gameassets/fighters-anthology --import-only`
created the menu/theater/aircraft cache under the standard Linux
`~/.local/share/T.O.R.E-Fighters/` application data directory.

## Shutdown correction

The initial menu smoke test presented a frame but exited with SIGSEGV (139).
The core's only remaining thread passed through `wgpu_hal::gles::egl::terminate_display`,
NVIDIA EGL/Wayland cleanup, and `wl_proxy_marshal_flags` in libwayland-client.
This occurred even though the selected rendering adapter was AMD Vulkan.
There was ample memory and no OOM event in the checked kernel log.

The app previously retained its renderer after `EventLoop::run_app` consumed
and dropped the event loop. It now releases the renderer in `ApplicationHandler::exiting`,
while the display connection remains alive. Renderer fields also keep the
window alive until all GPU resources have dropped. These changes address the
observed lifetime ordering; driver-internal frames were not fully symbolized,
so this is not an independently established NVIDIA driver defect.

## Validation

All checks below passed after the cleanup fix:

| Check | Evidence |
| --- | --- |
| Formatting | `cargo fmt --all -- --check` |
| Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Rust tests | `cargo test --workspace --locked`: 110 passed |
| Build | `cargo build --workspace --locked` |
| Python tests | `python3 -m unittest discover -s tools -p 'test_*.py'`: 9 passed |
| Asset guards | Source tree and both debug executables passed |
| Main menu | `cargo run --locked -p tore-app -- --smoke-test`: presented, exit 0 |
| Creator | Same command with `--quick-mission`: presented, exit 0 |
| Viewer | Same command with `--viewer`: presented, exit 0 |
| Flight | Same command with `--free-flight`: presented, exit 0 |
| Headless flight | `--headless-flight 1200 --maneuver pull`: 1,200 ticks, `crashed=false` |
| Local-only paths | `git check-ignore` confirms media, reference checkout, research and build output are ignored |

The initial graphical checks selected
`AMD Ryzen 9 7900X 12-Core Processor (RADV RAPHAEL_MENDOCINO)`
(Vulkan, IntegratedGpu), with AutoVsync and one requested queued frame.
The installed RTX 4070 was not selected for rendering. Local logs, archive
hashes and transfer verification are in ignored `.local/setup-linux/`.

Smoke tests establish frame presentation and successful shutdown, not sustained
performance, visual parity, manual controls or audio acceptance. No composition
or flight response changed. Windows and macOS were not retested during this
Linux setup. Remaining menu, environment, and native flight parity work stays
open in [progress](../progress.md).

## Visible-window follow-up

The user reported a blank window after setup. A compositor screenshot confirmed
that normal startup with the low-power AMD Vulkan adapter displayed no menu,
despite successful submission and no reported surface error. Restricting Vulkan
to the installed NVIDIA driver made the original menu visible. The renderer now
requests `PowerPreference::HighPerformance` with its existing surface-compatibility
requirement, selecting the RTX 4070 on this host without environment overrides
or machine-specific paths. Integrated-only systems remain eligible.

A second compositor screenshot of the normal `cargo run --locked -p tore-app`
launch confirmed the menu remains visible on the RTX 4070. Captures are local:
`.local/setup-linux/blank-window.png`, `nvidia-window.png`, and
`default-gpu-window.png`. This establishes an adapter-selection workaround on
this mixed-GPU desktop, not the underlying driver/compositor failure mechanism.
The earlier submission-only smoke evidence did not establish visible output.

After changing the preference, formatting, warnings-denied Clippy, all 110 Rust
tests, the locked build, nine Python tests and source/binary asset guards passed
again. Menu, creator, viewer and flight smoke tests all selected the RTX 4070
and exited successfully; logs use the `nvidia-*-smoke.log` prefix. This follow-up
visually checked the normal menu, not every theater or instrument panel, and
does not establish a performance comparison between the GPUs.
