# Performance validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The [threading plan](../ARCHITECTURE.md#performance-and-threads) defines what
must stay unchanged. Before and after use the same release compiler, imported
profile content, resolution, mission input and measurement script. Build before
measuring; do not run another build, test suite or game alongside a timing run.
The harness refuses a live game on Linux and never stops somebody else's process.

## Repeatable workloads

`tools/performance.py` runs already-built executables and retains each raw log
and a `results.json`. Use a fresh output directory per pass. `--compare` names
the before results and refuses a different workload. Three repeats are the
default. Worker count is chosen with `--workers`, including `0` for inline work.

```sh
python3 tools/performance.py --suite headless --binary target/release/tore-app \
  --profile .local/my-perf-data --output .local/perf-before/headless
python3 tools/performance.py --suite frames --binary target/release/tore-app \
  --profile .local/my-perf-data --output .local/perf-before/frames
```

Headless cases include a 2 against 2 and a 15 against 15 fight, opening and
sustained fixed tick counts, and a scripted attack. Their process elapsed time
includes loading. Correctness comes from the separate canonical single-player
harness and exact serial/parallel tests, not from a speed ratio.

Frame cases run through `tools/agent-run.sh` at 1920 by 1080, with audio on/off
and 1x/2x/4x/8x. These workloads explicitly enable normal mission recording
(`TORE_RECORD_MISSIONS=1`), including its CPU and writer costs. Scripted input selects and designates a weapon before changing
speed. A case fails if no designated estimate was observed or the requested
speed/tick workload was not reached. Lock evidence resets when speed changes,
so a designation only during the initial 1x setup cannot qualify an 8x case. `--ticks` chooses fixed simulated work;
`--scales 8 --ticks 36000` supplies a sustained accelerated-flight pass.

`TORE_PERF_TICKS=120..1000000` ends the existing opt-in frame measurement after
a fixed tick workload, with at most the final frame's tick overshoot. These
runs retain the opening frames; the first frame has no preceding interval.
Both diagnostic modes ignore saved display/sound preferences and disable
automatic recording unless explicitly enabled. Ordinary `TORE_PERF_FRAMES`
runs still exclude 30 warm-up frames. Reports include
frame p99, achieved simulation rate and evidence of a designated weapon
estimate. Captures must separately match at fixed simulation inputs on the same
GPU backend; live performance frames advance at different wall-clock rates.

Bare `--capture-flight` and `--capture-terrain` hold the prepared state while
the window and GPU initialize. Before this fix, startup time advanced the
flight or weather, making even repeated captures from one binary differ.
`render-capture-flight-deterministic` and
`render-capture-terrain-deterministic` in the battery each compare three
captures. Keep the same capture behavior on both sides of a comparison.

Synthetic worker, AI, geometry and complete instrument-canvas timing probes
are ignored Rust tests. Run them explicitly in release mode, one at a time on
an otherwise quiet machine. They compare inline work with 1, 2, 4 and 8 workers;
their times are evidence, not pass/fail deadlines. For example:

```sh
cargo test --release --locked -p tore-workers --test dispatch_probe -- --ignored --nocapture --test-threads=1
cargo test --release --locked -p tore-app instrument_canvas_workers_wall_time -- --ignored --nocapture --test-threads=1
```

## Host elapsed time

Build the ignored host benchmark with `cargo test --release --locked -p
tore-session --test host_players --no-run`. Pass its executable under
`target/release/deps/` as `--host-benchmark` with `--suite host`; the harness
runs 0, 15 and 30 humans for 60 simulated seconds by default.

The benchmark times the complete host call with `Instant`, including worker
completion. Its separate Linux host-thread CPU figure excludes workers and
cannot establish a threading speed-up. The simulated bots run outside the
measured host calls. Deadline and frame contention must additionally be checked
with a real hosting game; a fast-forwarded host-only test is not that check.

## Evidence and limits

Retain compiler/build identity and raw runs locally. Report one feature-level
baseline in `docs/baselines/`, including repeated spreads and p95/p99, failures,
unrun platforms and any deferred work. Use the same cases after each relevant
slice. Portable synthetic worker/geometry tests need no retail media; live
frame tests do. Do not compare capture hashes across unrelated GPU backends.

For an actual hosted flight, `TORE_PERF_HOST=1` prints each second's completed
host tick mean and maximum elapsed cost and cumulative overload count. Combined
with `TORE_PERF_TICKS`, which counts client ticks for networked flights, this
checks the main screen, audio and hosting thread sharing the pool. It changes
only diagnostics. Keep the local bot load and mission file identical across
builds; individual tick cost is a wall-time budget, not processor utilization.
