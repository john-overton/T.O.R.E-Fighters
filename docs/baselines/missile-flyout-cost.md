# Missile fly-out cost validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation, 2026-10-20. Comparison against `95472692`, using
its saved release binary and the final Part 1 source on `v2-tti`.
The [exact shortcuts](../ARCHITECTURE.md#the-firing-estimate-and-its-exact-shortcuts)
change prediction cost, not the [missile model](../spec/missiles.md).

## Method and limits

Linux, Ryzen 9 7900X, pinned Rust 1.91.1, release builds. Builds and tests used
`nice -n 10` and `CARGO_BUILD_JOBS=4`, one heavy job in this worktree at a time.
The import was the task's copied profile. No game windows were opened.
Other worktrees were running builds and validation on the shared host, so these
are observed CPU costs, not isolated benchmark results or windowed frame rates.

Both workloads used `--separation 5 --probe-attack 600:10 --no-audio`:
`--ai-probe-ticks 2400 --probe-fight 15:15` and
`--ai-probe-ticks 21600 --probe-fight 2:2`. CPU time is the complete process's
user plus system time, measured with `resource.getrusage(RUSAGE_CHILDREN)`.
Three before/after pairs ran alternately, at the default 60 Hz observation rate
and default worker setting. Startup and import work are included.

Samply used `record --save-only --unstable-presymbolicate`. Shares below count
main-thread stacks containing `missiles::intercept_with_terrain`, excluding the
unrelated autopilot intercept. The supplied earlier 15v15 base profile is the
before sample for that case; both 2v2 profiles were captured in this pass.
One profile per build/case is insufficient to resolve small percentage changes.

## Measured costs

| Workload | Before CPU seconds, three runs | After CPU seconds, three runs | Mean before / after | Fly-out samples before / after |
| --- | --- | --- | --- | --- |
| 15v15, 2,400 ticks | 2.682, 2.424, 2.749 | 2.478, 2.563, 2.496 | 2.618 / 2.512 | 381/2,937 (12.97%) / 314/2,472 (12.70%) |
| 2v2, 21,600 ticks | 2.988, 2.972, 2.958 | 3.049, 2.653, 2.711 | 2.973 / 2.804 | 503/2,809 (17.91%) / 437/2,551 (17.13%) |

The before/after spreads overlap. The maximum-range search remains the largest
fly-out caller: 9.75% of the after 15v15 samples and 11.76% of the after 2v2
samples. Launch solutions account for 2.14% and 4.39%; the firing band accounts
for 0.81% and 0.98%. No windowed performance improvement was measured here.

Temporary per-thread counters measured the launch-query cache separately, then
were removed before the final release build, timing runs and commit:

| Workload | Plain probe hits / queries | Recorded probe hits / queries |
| --- | --- | --- |
| 15v15, 2,400 ticks | 0/991 (0%) | 991/2,040 (48.58%) |
| 2v2, 21,600 ticks | 0/1,736 (0%) | 2,327/4,232 (54.99%) |

The plain-probe timing benefit comes from the reach bound. The recorded probes
exercise repeated readers. Interpolated frame launchers remain separate inputs;
no frame-level sharing or windowed cache rate is claimed.

The fixed range/band regression flies 831,341 ticks against 2,187,517 in the
full-flight reference, with identical answers. Its work limit is now 1,000,000
ticks. The powered-flight regression compares receding and stationary targets
against the full-flight reference. Cache tests cover exact keys, signed zero,
last-bit changes, replacement and repeated reads.

## Unchanged results

| Probe | Default workers and `TORE_WORKERS=0` |
| --- | --- |
| 15v15, 2,400 ticks, default 60 Hz observation | `6d7f0d87dc66bd00` |
| 15v15, 2,400 ticks, `--ai-observe-hz 120` | `416d3b1e5e8e4f05` |
| 2v2, 21,600 ticks, default 60 Hz observation | `1b3ef475ed3772a4` |

The 15v15 `--record-mission` comparison has 3,325,196 identical bytes after the
header: 21 data chunks, 2,401 frames, footer, index and trailer. Only the header's
`recorded_at` value differs; its checksum changes accordingly. Payload SHA-256:
`a28bbbdde6d4e7520a20128c0df4cc871f4250c2c9526fe05d434ff4b69b020c`.

Validation passed: sim/world tests (2,069 and 1,007), workspace tests (6,914
passed, 102 intentionally ignored), wire golden, host picture and worker tests,
workspace build, Clippy with warnings denied, formatting, 481 Python tooling
tests (four skipped), source and executable asset scans, and documentation
checks. The separately run real-data host picture test compared 55,998 packets
and 48,454,665 bytes over 60 simulated seconds with 15 bots, all identical.
The inherited cache test's unreachable crossing fixture was corrected to a
reachable receding fixture; Clippy's nested-type warning was fixed with a type
alias. Neither fix changes gameplay. No Windows/macOS run, retail comparison,
rendering smoke test or new missile-acceptance pass was performed in this slice.

## Local evidence

Raw artifacts are local only under
`.local/next-round/visual-fixes2/tti/` in the main checkout:
`runs/cpu.json`, `runs/probe-progress.log`, `runs/replay-comparison.log`,
`runs/checks-final.json`, `runs/cache-*.log`, `runs/flown-ticks.log`, the
`compare-*.tore-replay` recordings, and `prof/profile-*.json.gz` with their
symbol sidecars, top-40 summaries and caller splits. The original 15v15 profile
is `prof/base-15-2400.json.gz`. `runs/benchmark-host-load.txt` records the shared
host activity at the start of the timing batch.
