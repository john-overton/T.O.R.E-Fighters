# Loading preparation measurement

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

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

## Theater comparison, 2026-10-16 review

Same probe, after the worker handoff, on a private copy of the imported
profile. Debug build, Linux, `nice -n 10`, six Cargo jobs. F18.PT in TVIET,
UKR, EGY and IRA. `TORE_LOADING_THEATER` selects the theater (default UKR).
The stage lines are the probe's own timing prints. No media bytes are included.

| Theater | Terrain | City mask | City place | Mission | Aircraft art | Scenery | Total | Heartbeat gap |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| TVIET | 146.643 ms | 73.011 ms | 20078.575 ms | 20388.546 ms | 9.748 ms | 287.765 ms | 20693.741 ms | 5.786 ms, 4093 polls |
| UKR | 40.983 ms | 47.683 ms | 2262.280 ms | 2416.605 ms | 10.791 ms | 160.638 ms | 2593.588 ms | 5.072 ms, 513 polls |
| EGY | 58.986 ms | 50.200 ms | 831.159 ms | 972.717 ms | 10.661 ms | 168.014 ms | 1159.053 ms | 5.074 ms, 230 polls |
| IRA | 90.884 ms | 48.825 ms | 1591.595 ms | 1793.170 ms | 10.410 ms | 209.601 ms | 2019.253 ms | 5.329 ms, 399 polls |

Graphics on this headless probe was about 1.4 to 1.7 ms. It is not a GPU
measurement. The mission column is the whole world build, which includes
terrain and the city fill.

TVIET's caller never sat inside the 20 second city placement. The largest
heartbeat gap on this run was 5.786 ms. An earlier run the same evening, while
the machine was busier, reached 36.775 ms across 4095 polls, still on a 20.8
second load. The whole mission build, including the city fill, runs on the
worker.

TVIET's city fill, measured again with the district counters: the theater
palette loaded (not the Ukraine fallback). 85 retail city clusters were
collected. The mask then kept 323 classified city tiles, 0 synthetic village
tiles, and 0 districts. Placement took 19849.346 ms, thinning 2,610,756
candidate lots to 463,439 buildings. That placement is 19.8 seconds of a 20.5
second load. The village raster described for a theater with no city pictures
did not run, because the mask found city tiles and discarded the clusters.
The progress weights above are still the original UKR milliseconds, so a TVIET
load spends almost all of its time inside the mission stage, whose estimate
approaches about 78 percent and waits there until placement finishes.

A second TVIET build in the same process took 542.671 ms. The layout cache
reported the generation as 0 ms (463,439 instances already built) and the
city fill as 77.416 ms, which is the per-mission height and hide pass. The
cache is the process-wide layout map in `tore_world::city`. It does not
survive a new process, so each launch of the game pays the cold placement.
Caching that layout on disk, per theater and import generation, would make a
cold start look like the second build. It is not a small change: a stale file
must not survive a placement-code change, the content manifest must still read
the same resource names, and every machine in a multiplayer game must build
the same city. Left as a proposal.

Reproduce one theater, and a second build in the same process:

```sh
TORE_LOADING_PROFILE=/path/to/private/profile TORE_LOADING_THEATER=TVIET \
  TORE_LOADING_REPEAT=1 CARGO_BUILD_JOBS=6 nice -n 10 \
  cargo test --locked -p tore-app real_mission_worker_services_headless_heartbeat \
  -- --ignored --nocapture
```

`TORE_LOADING_THEATER` defaults to UKR. The original UKR command, without
that variable, is unchanged.

For the requested display verification, measure event-loop service gaps,
first-frame uploads, graphics preparation and recording initialization for
Quick Mission, restart, host, join and Replay Watch. The headless heartbeat
cannot establish compositor responsiveness or GPU driver behavior.
