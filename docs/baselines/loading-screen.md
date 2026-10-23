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
measurement. A loadout stage of **fitted** 25 ms was added on 2026-10-17 so
aircraft selection can sit on the same bar as the mission build. The measured
mission, aircraft and scenery weights are unchanged. Percentage is labeled
Estimated: elapsed time advances within the current stage asymptotically, and
completion of a stage advances to its next measured boundary. This does not
claim that elapsed time is completed work. The moving activity strip that used
to sit under the bar was removed the same day, at John's request.

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
the same city. Left as a proposal; built on 2026-10-19 (below).

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

## City layout cache, 2026-10-19

Optimization slice S12, branch `opt-s12` from `optimization` 1e7055d9.
Release build, Linux, Ryzen 9 7900X, `nice -n 10`, one run each, a private
copy of the imported profile, John at the machine (not an isolated
measurement). `tore-app --city-layout-digest` builds each theater's mission
terrain once in a fresh process (the in-process layout memo never answers)
and prints the full layout digest and the wall time of the whole terrain build,
which includes the terrain, the light map, the kit and the per-mission height
and hide pass as well as the placement.

| Theater | Before, ms | Fresh after, ms | First load (writes the cache), ms | Cached, ms | Cache file, MB |
| --- | ---: | ---: | ---: | ---: | ---: |
| APA | 56.0 | 42.6 | 45.2 | 32.9 | 0.13 |
| BAL | 850.7 | 288.7 | 265.2 | 152.2 | 3.35 |
| CUB | 257.9 | 126.1 | 118.7 | 78.2 | 0.94 |
| EGY | 615.9 | 177.9 | 166.5 | 74.6 | 2.75 |
| FRA | 2764.0 | 485.1 | 480.7 | 134.4 | 7.46 |
| GRE | 334.6 | 127.5 | 127.0 | 75.7 | 1.30 |
| IRA | 1177.5 | 335.3 | 310.4 | 108.9 | 5.64 |
| KURILE | 17.6 | 19.1 | 20.2 | 18.9 | 0.00 |
| LFA | 21.2 | 19.3 | 23.7 | 17.5 | 0.03 |
| NSK | 444.8 | 161.3 | 157.4 | 79.8 | 1.94 |
| PGU | 831.6 | 238.2 | 242.9 | 79.9 | 4.06 |
| SPA | 548.2 | 167.8 | 178.7 | 67.6 | 2.64 |
| TVIET | 10883.9 | 1326.3 | 1341.6 | 149.3 | 6.96 |
| UKR | 1544.6 | 331.7 | 342.8 | 94.4 | 6.58 |
| VLA | 388.2 | 127.6 | 122.4 | 68.1 | 1.61 |
| WTA | 482.2 | 166.3 | 183.0 | 90.8 | 2.05 |

- Before is the unchanged generator. Fresh after has the exact rework of the
  placement (`TORE_CITY_CACHE=0`): each lot's sort key drawn once instead of
  once per comparison, the random stream's stem hashed once, each city's
  building menu built once per kind of lot, the lot tests reading a per-tile
  flag patch with summed street counts instead of 225 map lookups, and the
  keep-outs found through a coarse index. Cached reads the placement back from
  the profile's `city-cache/` (47 MB for all sixteen theaters).
- Every run printed the same full digest for every theater as the unchanged
  generator: every building's piece, position, heading, core flag and city,
  every kit piece with its footprint and height, every city, the candidate
  count and the resources read; the per-building ground heights and hidden
  set matched too.
- The placement stage alone on TVIET: 10,754 ms before, 1,272 ms fresh after,
  13.7 ms from the cache (the light map, 64 to 87 ms, is built every load).
- Dedicated server, `tore-server --check` on a TVIET mission: city fill
  1,678 ms with the cache off, 173 ms reading the game's cache, with
  identical output, content manifest digest included (517 resources).
