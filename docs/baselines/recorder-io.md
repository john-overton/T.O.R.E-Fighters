# Dedicated recorder disk isolation validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation on Linux, 2026-10-23. Both binaries use the
`recorder-io` worktree based on `ab532b17d` (0.2.1rc-1). The before binary
adds only worst-tick reporting. The after binary also moves recording to the
worker described in [the behavior spec](../spec/replay-feedback.md).
Protocol 72 and replay format 4 are unchanged. The imported profile identifies
Fighters Anthology 1.02F. This is a host-feature test, not retail comparison.

## Local disk-load comparison

Each run starts a dedicated co-op server on an ephemeral localhost UDP port,
with recording on, broadcasting and telemetry off. The authored Ukraine
mission has 24 AI aircraft: four each of F/A-18D, F-14, Rafale C, MiG-29,
Su-27 and MiG-21, starting at 20,000 ft and 20 nm separation. The controller
waits 300 wall-clock seconds, then sends `status` and `quit`. Startup and final
flush are included in elapsed time. No human client is connected.

A parallel writer repeatedly overwrites a bounded 256 MiB scratch file using
1 MiB writes, then calls fsync. It wrote 32,366 MiB before and 29,184 MiB after.
The machine also ran development builds and tests. These are real contention
samples on a shared machine, not a controlled disk benchmark.

| Measurement | Before | After |
| --- | ---: | ---: |
| UDP port | 51333 | 57920 |
| Elapsed including shutdown | 308.3 s | 301.6 s |
| Overload messages | 44 | 1 |
| Ticks discarded by overload handling | 4,456 | 48 |
| Worst measured tick | 7,291.1 ms | 7.8 ms |
| Final simulation tick | 30,009 | 34,284 |
| Exit status | 0 | 0 |

Before: the recording finishes normally with 30,009 frames. After: disk
pressure exhausts the shared unwritten-frame budget at tick 241; the server
warns, disables this mission's recorder, and continues flying. Its recovered
partial contains all 240 accepted frames and its companion log. The after
run's one overload appears before the first status line. Since the measured
maximum is only 7.8 ms, the log does not establish a tick-body stall as its
cause. Startup, scheduling and other run-loop work are outside that maximum.

This demonstrates that saturated replay I/O no longer forces flight to wait.
It does **not** demonstrate continuous recording on an overloaded disk. A
100-second follow-up without the deliberate load also stopped recording at
360 frames while other builds and tests were running, with zero overloads
and a 20.4 ms worst tick. The conservative budget can stop a recording on a
short chunk-write spike; that operational tradeoff is explicit in the spec.

A second 100-second follow-up, after compilation ended and while the replay
battery ran, finishes its recording normally with zero overloads and a 5.8 ms
worst tick. It crosses both the 3,600- and 7,200-frame sync boundaries. The
normal path therefore keeps recording and flushes its final chunk on quit;
the saturated path is tested separately above.

## Validation and evidence

Synthetic worker tests cover ordered output and byte equality with the old
synchronous write sequence over 7,205 frames, including both sync boundaries
and the final partial chunk. Companion logs are byte-identical too. A gated
writer proves that saturation returns without waiting, accepted frames drain
to a partial, and cleanup joins. Tests also cover the unwritten budget after
the receiver has emptied the channel, replenishment after chunk completion,
I/O errors, panic cleanup, retention, mission restart, Stop, migration step
down and host destruction. The underlying writer checks 120-frame chunks and
3,600-frame periodic syncs. Status tests cover interval maxima, reset, the
periodic line and console output after existing listing fields.

The 2,400-tick, 15-versus-15 AI probe keeps checksum `cbd959d1f838e9db` at the
default observation rate and `bcdea9dc8f961940` with `--ai-observe-hz 120`.
No simulation or AI behavior changed. Power failure was not injected.
Windows and macOS runtime execution were not tested.

Evidence is local under
`/home/john/Development/T.O.R.E-Fighters/.local/next-round/recorder-io/`:
`measure.py`, the two saved binaries, `before/` and `after/` contain exact
configs, stdout, disk-load totals, summaries, recordings and reader reports.
`healthy/` and `healthy2/` contain the follow-ups. The session `report.md` records the final
check results, battery paths and commit identities. Retail-derived recordings
and imported media remain uncommitted.
