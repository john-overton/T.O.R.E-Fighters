# Performance and threads validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-03. The completed opening-flight, headless and
host measurements show substantial gains, with unchanged results in the
matched single-player harness and all 96 fixed captures. The heavy opening
fight reaches approximately 6.8 to 7.1x when asked for 8x; the three-minute
workload reaches 7.87x overall. Real hosted flights maintain 1x with zero
reported overloads. Worker-count and serial-fallback acceptance are complete.

The [architecture](../ARCHITECTURE.md#performance-and-threads) describes the
implementation and its agent decisions. The [testing guide](../testing/performance.md)
describes reproduction. This is a comparison with the previous Rust build,
not evidence of retail-game parity.

## Machine, builds and method

AMD Ryzen 9 7900X, 12 cores and 24 logical CPUs, 64 GB RAM, NVIDIA RTX 4070,
driver 610.57.04, Linux 7.2.5-3-omarchy x86_64. Both sides used Rust 1.91.1
release builds with the locked dependencies. Windowed cases used the same
GPU/backend settings at 1920 by 1080, through `tools/agent-run.sh`.

The before gameplay revision is `c1d1e736`, including PF1, in the frozen
`T.O.R.E-Fighters-pf-baseline-20261003` checkout with measurement-only changes.
Measurements used the implementation working tree on `performance`, based on
that revision, before committing it. `final-build-identity.json` and
`final-reviewed.patch` identify the timed implementation; its application
SHA-256 starts `2db7beb1ea67d3e8`. The final capture-corrected executable starts
`0e7262d8bb6932ed`. Each `results.json` preserves the full binary hash. The earlier headless
before build has hash prefix `276a10173c8c4952`; frame/host metadata uses the
later measurement-only before build, `4d7e877bd38f443f`.

The isolated import was `.local/mpb-data-perf-implementation`. Standalone
frame workloads enabled normal mission recording on both sides, including its
writer cost. The initial frame runs inherited this setting from the fresh
profile; the harness now requests it explicitly. Headless probes do not record
unless requested. No saved display or sound preferences were present, and the
harness now ignores them explicitly. No builds, other tests or other games
ran alongside the timing cases.

The completed timing tables use three repeats from `tools/performance.py`, with eight
workers selected for the implementation. A time written `median [min, max]`
is the spread of the three run summaries, not a confidence interval. A p99
column is the median of each run's 99th-percentile time. Raw results also
retain p50, p95, maxima and individual p99 values.

For frame and headless cases, light means a 2 against 2 mission and heavy
means 15 against 15, at 5 nautical miles separation. Frame cases request
2,400 simulation ticks and retain the
last frame's bounded overshoot. Input selects and designates AIM120.JT before
changing speed. Every recorded frame case produced a designated estimate at
the requested speed. Statistics for accelerated cases exclude the initial
1x setup. These are opening-fight measurements, not a sustained 8x guarantee.

The capture-only startup-clock correction described below followed these
timing matrices. A three-repeat heavy-8x recheck on the final executable
reached 7.01x without audio and 6.80x with audio. Mean frame intervals were
153.93 [148.94, 155.15] ms and 177.71 [172.55, 183.33] ms respectively. These later
results are slower than the earlier after matrix, especially with audio, but
retain the large gain over the original 4.65x/4.58x rates. Both passes are
retained; the result is approximately 6.8 to 7.1x, not a full-8x promise.

## Headless elapsed time

These are whole-process seconds, including loading, not just simulation time.
The 2,400- and 21,600-tick cases cover 20 and 180 simulated seconds. Both use
the same scripted attack and no audio.

| Mission | Ticks | Before seconds | After seconds | Before / after |
| --- | --- | --- | --- | --- |
| light | 2,400 | 1.192 [1.191, 1.204] | 0.994 [0.988, 0.997] | 1.20x |
| light | 21,600 | 2.573 [2.564, 2.598] | 2.039 [2.034, 2.039] | 1.26x |
| heavy | 2,400 | 4.007 [3.994, 4.128] | 2.423 [2.421, 2.425] | 1.65x |
| heavy | 21,600 | 11.702 [11.653, 11.716] | 8.234 [8.226, 8.312] | 1.42x |

The heavy opening process is 1.65 times faster; the longer heavy run is
1.42 times faster. These ratios include the serial fixes as well as workers.

## Opening-flight frame intervals

Times are milliseconds. Achieved speed is simulated time divided by elapsed
wall time. Shorter frames also change how many overdue ticks the next frame
must execute, so frame-interval ratios are not direct simulation speedups.
The implementation retains every owed simulation tick.

| Mission | Asked | Audio | Achieved before to after | Before mean ms [range] | After mean ms [range] | p99 ms before to after |
| --- | --- | --- | --- | --- | --- | --- |
| light | 1x | off | 1.00x to 1.00x | 9.34 [9.31, 9.35] | 9.12 [9.05, 9.12] | 25.07 to 24.61 |
| light | 1x | on | 1.00x to 1.00x | 9.39 [9.36, 9.40] | 9.01 [8.96, 9.08] | 25.48 to 24.79 |
| light | 2x | off | 2.00x to 2.00x | 9.89 [9.80, 9.93] | 9.37 [9.37, 9.45] | 27.31 to 25.91 |
| light | 2x | on | 2.00x to 2.00x | 9.80 [9.79, 9.81] | 9.45 [9.39, 9.45] | 26.26 to 25.53 |
| light | 4x | off | 4.00x to 4.00x | 11.87 [11.81, 11.99] | 10.19 [10.10, 10.21] | 30.94 to 26.79 |
| light | 4x | on | 4.00x to 4.00x | 12.01 [11.99, 12.20] | 10.17 [10.17, 10.29] | 31.02 to 25.98 |
| light | 8x | off | 8.00x to 8.00x | 41.10 [39.10, 41.13] | 14.10 [13.81, 14.44] | 54.48 to 28.77 |
| light | 8x | on | 8.00x to 8.00x | 45.65 [44.67, 46.45] | 14.83 [14.22, 14.92] | 59.78 to 28.81 |
| heavy | 1x | off | 1.00x to 1.00x | 15.37 [15.19, 15.59] | 10.11 [10.10, 10.12] | 35.92 to 23.73 |
| heavy | 1x | on | 1.00x to 1.00x | 15.40 [15.37, 15.56] | 10.16 [10.12, 10.16] | 35.73 to 24.15 |
| heavy | 2x | off | 2.00x to 2.00x | 27.66 [27.40, 27.98] | 11.04 [11.03, 11.11] | 65.40 to 28.33 |
| heavy | 2x | on | 2.00x to 2.00x | 28.47 [27.91, 29.55] | 11.19 [11.14, 11.23] | 68.03 to 28.89 |
| heavy | 4x | off | 3.81x to 4.00x | 105.23 [103.11, 125.71] | 17.52 [17.39, 17.72] | 314.72 to 50.30 |
| heavy | 4x | on | 3.77x to 4.00x | 110.71 [106.55, 110.83] | 18.17 [18.05, 18.22] | 320.82 to 50.42 |
| heavy | 8x | off | 4.65x to 7.09x | 379.99 [378.87, 383.97] | 148.55 [148.47, 152.00] | 579.50 to 378.71 |
| heavy | 8x | on | 4.58x to 7.00x | 386.26 [386.10, 402.75] | 163.04 [154.43, 163.62] | 590.84 to 350.50 |

Light missions retain their requested rates through 8x. Heavy 4x now reaches
4.00x, with median frames of 17.52 ms without audio and 18.17 ms with audio.
Light 1x changes are small; the larger improvements are in heavy and accelerated
cases, where they clearly exceed the repeated-run spread.

Heavy 8x improves from 4.65x to 7.09x without audio, and from 4.58x to 7.00x
with audio. The after runs span 7.05x to 7.10x and 6.86x to 7.01x respectively.
It still misses the requested 960 ticks per wall second in this opening fight,
and p99 frame times remain hundreds of milliseconds. The result is an
improvement, not a claim that every heavy mission sustains 8x.

## Sustained accelerated flight

The matched final executables ran 21,600 ticks, three simulated minutes, at
8x requested speed with audio off/on and three repeats each. This fixture
keeps the player invulnerable, disables crashes and supplies unlimited fuel;
ammunition remains finite. Combat and target availability evolve during the
run. It verifies an initial AIM120 designation at the requested speed without
imposing a constant lock. Recording remains enabled.

| Audio | Achieved speed before to after | Before mean frame ms [range] | After mean frame ms [range] | p99 ms before to after |
| --- | --- | --- | --- | --- |
| off | 7.37x to 7.87x | 16.92 [16.27, 17.09] | 14.01 [13.90, 14.23] | 73.94 to 47.97 |
| on | 7.36x to 7.87x | 17.06 [16.64, 17.40] | 14.31 [13.57, 14.39] | 76.73 to 51.02 |

All runs reached their requested tick count without paused frames. The final
build reaches 7.87x overall in both audio modes, including the costly opening.
Its complete raw reports retain the long-frame maxima as well as percentiles;
the median does not imply that the opening stalls disappeared.

## Host elapsed time

The 30-aircraft mission uses 10 nautical miles separation and runs for 60
simulated seconds after seating on the seeded, perfect simulated network.
Bots run outside the timed host calls. Times cover
the complete host operation, including worker waits. The Linux calling-thread
CPU diagnostic excludes workers and is not the measure used here.

| Human seats | Before mean ms/tick [range] | After mean ms/tick [range] | Before / after | Ticking-call p99 ms before to after |
| --- | --- | --- | --- | --- |
| 0 | 3.063 [3.061, 3.072] | 0.936 [0.936, 0.940] | 3.27x | 5.184 to 1.704 |
| 15 | 5.675 [5.668, 5.677] | 2.220 [2.212, 2.240] | 2.56x | 7.744 to 4.619 |
| 30 | 6.558 [6.512, 6.594] | 2.281 [2.262, 2.283] | 2.88x | 6.863 to 2.533 |

Both the mean cost and p99 improve in all three cases. The real hosted-flight
comparison below separately measures a process that also renders and plays audio.

## Real hosting game and shared workers

A windowed host rendered at 1920 by 1080 with audio, mirrors and instrument
camera panels. Local bot clients occupied the remaining seats for totals of
1, 15 and 30 humans in a 30-aircraft mission, 10 nautical miles apart. Each
run completed 2,400 client ticks; each load was repeated three times on both
builds. The fixture uses invulnerability, no crashes, unlimited fuel/ammunition
and ignored midair collisions to retain the load. Recording was requested,
but ongoing network-client recording was not established by this pass.

Host means below average the one-second elapsed-cost reports after the
requested player count was seated and tick 240 had passed. Frame statistics
cover the completed client workload. Raw logs retain every status interval.

| Humans | Before host mean ms [range] | After host mean ms [range] | Frame mean ms before to after | Frame p99 ms before to after | Overloads before / after |
| --- | --- | --- | --- | --- | --- |
| 1 | 5.595 [5.555, 5.702] | 2.214 [2.189, 2.293] | 11.21 to 9.08 | 18.63 to 12.28 | 0 / 0 |
| 15 | 6.233 [6.218, 6.465] | 2.628 [2.620, 2.630] | 10.85 to 8.25 | 18.62 to 10.64 | 0 / 0 |
| 30 | 7.052 [6.946, 7.150] | 2.937 [2.897, 2.954] | 11.42 to 9.39 | 19.07 to 11.53 | 0 / 0 |

All eighteen runs maintained approximately 1x simulation rate, completed their
AIM120 designation check and ended cleanly. No host overload was reported.
Individual ticks still exceed the 8.33 ms tick budget: the final run maxima
span 22.77 to 25.89 ms. Zero overloads refers to the existing backlog counter,
not a guarantee that every individual tick met its deadline. Frame mean
spread was largest with one or thirty humans; the complete repeats are kept.

## Worker counts and serial fallback

The final release executable ran the imported heavy 2,400-tick headless attack
three times at each worker setting. All fifteen standard-output files are
byte-identical. Times include process startup and loading.

| Workers | Seconds, median [min, max] |
| --- | --- |
| 0 | 3.614 [3.599, 3.620] |
| 1 | 3.924 [3.871, 3.924] |
| 2 | 2.983 [2.979, 3.027] |
| 4 | 2.571 [2.564, 2.581] |
| 8 | 2.496 [2.493, 2.525] |

One explicitly requested worker pays dispatch/copying costs without parallel
throughput and is slower than inline execution here. Automatic sizing uses
serial execution when fewer than two workers remain after its headroom rule.
The 4- and 8-worker results support retaining the bounded eight-worker default
on this machine. The separate debug quick guard with `TORE_WORKERS=0` reports
**SAME 52, DIFFERENT 0, MISSING 0** against the canonical reference.

## Unchanged behavior and capture correction

The canonical `mp-67aa2b18` and fresh before recording
`perf-before-20261003` both use the **default 249-case** single-player harness.
The matched after recording is `perf-after-matched-20261003`. Comparison with
each reference reports **SAME 316, DIFFERENT 0, MISSING 0**, including the
harness timing tolerance. Compiler, host and run-mode metadata match.

A separate `--full` run completed the additional 1,008-case aircraft matrix.
Its shared behavior outputs matched, but comparison with the default canonical
reference correctly rejected the different mode metadata and 1,009 extra
matrix outputs without reference counterparts. It is extra coverage, not a
matched full baseline. The rejection is retained in
`final-check-canonical-compare.log`; the successful matched comparisons are
`matched-canonical-compare.log` and `matched-before-compare.log`.

Initial GPU comparisons changed 46 of 96 images. Repeating captures from the
same original binary reproduced differences: window/GPU startup time advanced
an already-prepared flight or its weather. Those comparisons were unsuitable
for deciding whether the rendering changes preserved pixels.

The same capture-only elapsed-time hold was applied to original and current
builds, and capture fixtures disable controllers. Ordinary gameplay and a
smoke test without capture retain their normal clock. With identical prepared
state, all **63 standard plus 33 extra captures match byte for byte**. There
are no missing or extra images. They cover terrain/weather, cockpit pages and
views, airports, aircraft formations, weapons/effects, damage, ejection and
replay views/seeks on this host.

Two battery regressions each repeat a flight or terrain capture three times.
Both pass with the fix; both fail on the preserved original without the fix.
This validates the capture correction independently of the threaded rendering
changes. Initial and corrected comparison manifests remain local.

## Checks and implementation guards

All nine required repository checks passed after the capture correction:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo build --workspace --locked`
- `python3 -m unittest discover -s tools -p 'test_*.py'`
- `python3 tools/check_assets.py`
- `python3 tools/check_assets.py target/debug/tore-app`
- `python3 tools/check_assets.py target/debug/tore-extract`
- `python3 tools/check_docs.py`

The wrapped GPU smoke test also passed. The original validation run before
review cleanup recorded 2,890 Rust tests passed and 42 ignored; the Python run
passed 230 tests. Ignored
timing and long-running probes are separate from that ordinary test count.
The final headless battery passed **196/196**, followed by the two new capture
regressions above. Renderer startup warned that the Vulkan validation layer
was unavailable, so this is not a claim of validation-layer coverage.

Explicit executors cover 0, 1, 2, 4 and 8 workers and repeatable shuffled
schedules without changing process environment. Guards compare independent
pre-extraction references, floating-point bits, complete actor/ownship state,
ordered outputs and journals, exact packet bytes and bookkeeping, vertices
and contact ranges, and canvas pixels/cache state. They cover single-worker
nesting, simultaneous callers, panic completion, inactive inputs, firing at
projectile capacity, failed sends, plane changes, reconnection, replay seeking,
duplicate instrument pages and resize. The unused join and mutable-map APIs
were removed after review; nesting, concurrent callers and panic cleanup are
now exercised through the collection operations the game uses. No golden was
changed to accept a threading difference.

## Profiling decisions and rejected work

Two caches spanning an entire AI observation job were tried and removed.
Their lookup overhead made smaller worker configurations slower. In one
30-actor, two-worker synthetic comparison, the reference median was 1,109.72
microseconds, versus 1,363.93 for one cache and 1,190.72 for the other. The
retained change prepares target visibility separately and preserves awareness
metadata; it does not retain either broad cache. The narrower sensor-local
visibility reuse remains, now keyed by every ordered endpoint bit. Changed
segments call the original terrain query. Synthetic tests check changed and
reversed segments, adjacent floating-point values and signed zero.

A four-case user-space sampling pass, original/current with audio off/on,
identified missile range estimation, ordered AI work, camera-scene building
and changed-panel scaling as residual costs. It led to the bounded scene reuse,
prepared visibility and dirty-page worker changes included in the timed
implementation. Those intermediate samples are diagnostic evidence, not the
final timing matrix above.

The scratch Samply 0.13.1 build excluded kernel sampling and used 256 KiB
rings with 32,000-byte captured stacks under unchanged OS permissions and
memory-lock limits. None of the four runs reported lost events. Per-address
LLVM symbolication supplied inline call paths. Its exact source patch,
commands, binary hashes and raw profiles are retained locally.

The matched frame matrix and sampling did not reproduce PF1's severe
audio-dependent slowdown. In the sampled current heavy-8x interval, spatial
audio processing was about 0.5% of main-thread sampled CPU. No playback rule
was changed. These CPU samples do not establish mixer wait times or callback
silence counts, and do not rule out an audio issue in another workload.

## Limits and remaining review

Local implementation and Linux validation are complete. Review cleanup adds
endpoint-keyed visibility reuse and removes unused pool operations. The timing
tables above retain their measured build identities. Heavy opening fights still miss 8x,
and frame and host tick tails remain visible in the measurements.

Windows and macOS runtime/performance checks and remote CI were not run in
this Linux pass. The macOS scheduling boundary was reviewed, not measured on
this machine. Retail comparison remains unavailable. John has not yet flown
or approved a merge of this result.

## Evidence locations

Raw evidence is local under `.local/mp-notes/perf/`:

- `before-{headless,frames,host}/results.json` and
  `final-{headless,frames,host}/results.json`: repeated timings and raw logs.
- `before-build.json`, `final-build-identity.json`, `final-reviewed.patch`:
  revision, compiler, binary identity and reviewed implementation.
- `final-source-identity.json`, `final-tracked.patch`: final binary and changed
  source/document hashes, including the capture correction.
- `matched-default-guard.log`, `matched-canonical-compare.log`,
  `matched-before-compare.log`: matched default-mode behavior comparisons.
- `final-check-full-guard.log`, `final-check-canonical-compare.log`: expanded
  matrix coverage and its deliberately unmatched comparison metadata.
- `startup-timed-capture-comparison.json`, `fixed-capture-comparison.json`,
  `fixed-{before,after}-{captures,extra-captures}/`: capture diagnosis and
  corrected exact comparisons.
- `fixed-check-*.log`, `fixed-smoke.log`, `final-check-battery.log`,
  `capture-regression.log`, `capture-regression-original.log`,
  `capture-python-tests.log`: checks and regression evidence.
- `before-sustained/`, `after-sustained/`, `before-hosted/`, `after-hosted/`,
  `final-worker-counts/`, `post-capture-frames/`, `serial-guard-driver.log`:
  extended load, pool-size and final-executable checks. Local reproduction
  scripts are `sustained-run.py`, `hosted-run.py` and `worker-count-run.py`.
- `reference-visibility-timing.log`, `after-visibility-timing.log`,
  `direct-visibility-timing.log`: rejected cache measurements.

Sampling evidence is in
`.local/tmp-perf-implementation/samply.RUEICF/`, including `findings.txt`,
`focused-summary.txt`, `callsite-summary.txt` and `samply-user-only.patch`.
Imported media, captures, recordings and profiles remain outside Git.
