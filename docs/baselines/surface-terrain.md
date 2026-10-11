# Surface missile terrain validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation, 2026-10-11, slice CL-T on `cl-terrain` from
`07e89569`. The rules are in [surface lock and launch](../spec/air-to-ground.md#terrain-at-lock-and-launch)
and [cruise terrain clearance](../spec/missiles.md#cruise-terrain-clearance).
This is validation of the requested opinionated behaviour and fitted numbers,
not a claim of retail parity. Retail comparison is unavailable.

## Method

Linux x86_64, the pinned Rust toolchain, locked debug builds. The before binary
was built from `07e89569` before the implementation. Every direct run used
`.local/tmp-cl-terrain/profile`, a private copy of the imported `surface-data`
profile, with a private log directory. No shared profile was run directly.
The missile keys, designation and trigger go through the real player path.

All three measured cases use F/A-18D (`F18.PT`), 350 kt, 6 nmi initial range,
surface seed 1, target `0x50000000`, and designation through the A-G page.
The armor cases run for at most 90 s and stop after impact. The masked case
runs for 10 s. Altitudes below are MSL; the command's `--altitude` is relative
to the ground under the target, not under the launcher.

| Missile and setup | Before | After |
| --- | --- | --- |
| AGM-65A, `VLA QVARMOR`, heading 30, `--altitude 900` (1,668 ft MSL). Route peak 1,583 ft | Killed the 150 hp target; last position 21 ft away, end 27.958 s; highest logged altitude 1,759 ft | Killed it; 23 ft, end 28.508 s; highest logged altitude 1,869 ft |
| AGM-65A, `VLA QVARMOR`, heading 60, `--altitude 1100` (1,868 ft MSL). Route peak 1,747 ft | Killed the 150 hp target; 22 ft, end 27.958 s; highest logged altitude 1,866 ft | Killed it; 18 ft, end 28.333 s; highest logged altitude 1,952 ft |
| AGM-65G, `VLA QVSUP`, heading 180, `--altitude 300` (1,042 ft MSL). Terrain rises 10 ft above the sight line near the target | Locked and launched at 3.258 s; missile still flying at the 10 s stop | No launch; TERRAIN MASK; target retains 50/50 hp |

The armor target stands at approximately `(256415, 776, 1011569)` ft; the
supply target at `(378057, 751, 1236152)` ft. Both armor shots were already
successful before. They demonstrate the added clearance climb and return to
the terminal profile, not a newly saved hit. The route's terrain peaks are
measured every 128 ft; missile altitudes are the runner's once-a-second logs,
not exact continuous maxima. The last-position miss distance is the existing
runner measurement, not the impact contact point.

The masked case is a real failure of the former eight-probe visibility test:
the intervening rise lay between probes. A denser prelaunch check denies the
lock and release. The A-G page can still supply its designation; the fresh
weapon gate then reports TERRAIN MASK separately from the energy envelope.

Reproduce the second case with:

```sh
TORE_DATA_DIR=.local/tmp-cl-terrain/profile \
TORE_LOG_DIR=.local/tmp-cl-terrain/logs \
  target/debug/tore-app --a2g-run VLA QVARMOR --aircraft f18 \
  --load AGM65A --attack maverick --from 6 --altitude 1100 \
  --heading 60 --speed 350 --unit 0x50000000 --seconds 90
```

Change the load, template, altitude, heading and duration as in the table for
the other cases. Logs are in `.local/tmp-cl-terrain/before-vla-*.log` and
`after-vla-*.log`. An exploratory Greece radar-site shot hit, but its route was
flat and is not used as ridge evidence. Quick Mission relocation is why the
measurements use the placed targets rather than template coordinates.

## Regression coverage

- All ten profiled surface missile records reject a masked lock and release.
  A fresh terrain check also rejects a stale ready solution, while the range
  estimate and IN RNG answer stay unchanged.
- Buddy-laser spots require the shooter's sight line; laser bombs and powered
  laser missiles read MASK when blocked. AT-2's masked release remains
  unguided. A late first laser acquisition needs visibility; an established
  laser lock retains its existing loss rules.
- A synthetic 2,200 ft ridge stops the previous TORE cruise path. The new path
  clears it by at least 256 ft at the crest, resumes the profile and reaches
  its target. This fixture isolates in-flight motion from the launch visibility
  gate. Fly-out prediction agrees within one 120 Hz tick and a repeated
  flight gives the same result. Flat sea profiles, non-cruise weapons and
  terminal impact are covered separately.
- `a2g-terrain-mask` reproduces the real masked launch. `a2g-terrain-cruise`
  requires the measured eastern approach to climb above 1,900 ft and kill
  within 40 ft. Its validator rejects the before log and accepts the after log.

## Checks

- Format and strict workspace/all-target Clippy with `--locked`: passed.
- Rust tests: sim 1,679 passed, world 832 passed, session 514 unit tests plus
  11 integration tests passed. App 1,692 passed in the full run; five socket
  timing tests failed under concurrent machine load and all five passed in
  isolated Cargo reruns, without code changes to those tests. These were the
  window-stall, listed-join, two keepalive and returning-host cases. The final
  weapon-HUD subset passed all 28 tests, including MASK beside IN RNG.
- Locked workspace binary build: passed. Python tools: 344 passed. Source,
  `tore-app` and `tore-extract` asset checks: passed.
- Affected battery (`--changed 07e89569 --budget 2400 --jobs 6
  --with-windows no`): 97/97 passed, 986 s wall time. Explicit `a2g-*`:
  24/24 passed, 126 s, including both new terrain scenarios. Each scenario
  copied the shared `surface-data` source profile. Results are under
  `.local/tmp-cl-terrain/battery/20261011-152119-clt-changed/` and
  `20261011-153900-clt-a2g/`.
- Single-player guard against `sf-8b1fdebf`: SAME 52, DIFFERENT 0, MISSING 0,
  repeated on the final HUD binary. Final artifacts are in
  `.local/tmp-cl-terrain/guard-final/` (243 s wall time).

No protocol, checkpoint fields, goldens or missile sag steering were changed.
The wrapped rendering smoke test passed with audio disabled. No manual flight
was run. Terrain avoidance remains fitted and
limited by sampling, ignition, turn authority and energy, as the spec states.
