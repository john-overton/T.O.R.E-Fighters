# Loading preparation measurement

Implementation mode, 2026-10-16, nr-loading based on multiplayer ed6b534b,
protocol 51. Debug build, Linux, `nice -n 10`, four Cargo jobs. The local probe
uses a private copy of the imported surface-data profile, UKR and F18.PT.
No media bytes or screenshots of retail content are included here.

| Work | Elapsed |
| --- | ---: |
| World and mission aircraft build | 2442.242 ms |
| Combat presentation art | 10.605 ms |
| Scenery and texture preparation | 163.636 ms |
| Total worker and completion polling | 2623.090 ms |
| Largest caller heartbeat gap, 519 polls | 5.067 ms |

These are the same CPU calls that previously ran in the window callback,
measured after moving them to the worker. They are not a before/after window
benchmark. The heartbeat polls completion every 5 ms, with a 250 ms failure
threshold. There was no display, GPU, socket binding or retail comparison.

CPU progress weights round the first three timings to milliseconds. The GPU
reservation is **fitted**, 500 ms, an agent decision pending a display-host
measurement. Percentage is labeled Estimated: elapsed time advances within the
current stage asymptotically, and completion of a stage advances to its next
measured boundary. This does not claim that elapsed time is completed work.

The in-memory join test held its mission worker for seven seconds of transport
time while the caller continued servicing the link, then reached flight. The later flight-loadout build also passes a seven-second virtual hold. Its
regression test caught and now covers validation against accepted baselines
waiting in the deferred event queue. Broader results live in the slice report.

Reproduce CPU timing with a private profile copy:

```sh
TORE_LOADING_PROFILE=/path/to/private/profile CARGO_BUILD_JOBS=4 nice -n 10 \
  cargo test --locked -p tore-app real_mission_worker_services_headless_heartbeat \
  -- --ignored --nocapture
```

For the requested display verification, measure event-loop service gaps,
first-frame uploads, graphics preparation and recording initialization for
Quick Mission, restart, host, join and Replay Watch. The headless heartbeat
cannot establish compositor responsiveness or GPU driver behavior.
