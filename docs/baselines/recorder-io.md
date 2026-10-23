# Dedicated recorder disk isolation validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation on Linux x86-64, 2026-10-23. The synchronous
reference uses `ab532b17d` (0.2.1rc-1) plus worst-tick reporting. The first
asynchronous build is the implementation committed as `d786cf48`; the
7,200-frame measurement build adds the lead-requested budget to that commit.
All three measured binaries use protocol 72 and replay format 4. The buffer
change itself needs no protocol or replay-format revision. The imported profile identifies Fighters
Anthology 1.02F. This is a host-feature test, not a retail comparison.

The [behavior spec](../spec/replay-feedback.md) is authoritative for the
buffer, crash exposure, sync cadence and saturation policy.

## Local disk-load comparison

Each run starts a dedicated co-op server on an ephemeral localhost UDP port,
with recording on, broadcasting and telemetry off. The authored Ukraine
mission has 24 AI aircraft: four each of F/A-18D, F-14, Rafale C, MiG-29,
Su-27 and MiG-21, starting at 20,000 ft and 20 nm separation. No human client
is connected. The same controller waits 300 wall-clock seconds, then sends
`status` and `quit`. Startup and final flush are included in elapsed time.

A parallel writer repeatedly overwrites a bounded 256 MiB scratch file using
1 MiB writes, then calls fsync. It wrote 32,366 MiB in the synchronous run,
29,184 MiB with the small buffer, and 52,403 MiB with the 60-second buffer.
The machine also ran development builds and tests. These are real contention
samples on a shared machine, not a controlled disk-throughput benchmark.

| Measurement | Synchronous reference | 120-frame budget | 7,200-frame budget |
| --- | ---: | ---: | ---: |
| UDP port | 51333 | 57920 | 36795 |
| Elapsed including shutdown | 308.3 s | 301.6 s | 306.2 s |
| Overload messages | 44 | 1 | 0 |
| Worst measured tick | 7,291.1 ms | 7.8 ms | 19.6 ms |
| Final simulation tick | 30,009 | 34,284 | 34,483 |
| Saved frames | 30,009 | 240 | 34,483 |
| Recording state | Complete | Incomplete | Complete |
| Exit status | 0 | 0 | 0 |

The small budget stopped recording at the second chunk under disk pressure.
Its low tick cost therefore did not demonstrate sustained recording. With
7,200 frames, the recording survives the same load procedure for the whole
mission, keeps every tick from 1 through 34,483, and finishes normally with
288 chunks, 7,159,219 replay bytes and its companion log. No recorder failure
or overload was logged. Neither sync cadence nor chunk size changed.

## Memory per queued frame

A separate, temporary instrumented binary sampled the actual owned `Tick`
values handed to the writer in the same 24-aircraft mission. It counted the
inline value plus every vector's allocated capacity and every owned string's
capacity, including nested identities, event fields and tree nodes. It counted
unencoded frames before the worker applies format bounds. The payload sample
is therefore different from the much smaller serialized replay-file size.

Over 17,520 sampled frames on Linux x86-64:

| Quantity | Measured payload |
| --- | ---: |
| Mean per frame | 18,618.2 bytes, 18.2 KiB |
| Largest single frame | 143,540 bytes, 140.2 KiB |
| Largest rolling 7,200-frame window | 154,135,078 bytes, 147.0 MiB |
| Mean per frame in that busiest window | 21,407.6 bytes, 20.9 KiB |
| Inline `Tick` value, included above | 392 bytes |
| 7,200 message values, separate from payload | 115,200 bytes, 112.5 KiB |

Thus a full minute of this workload can retain about 147 MiB of frame payload,
plus channel synchronization storage, allocator overhead and the ordinary
world/writer state. This measures the payload of a possible full queue, not
process RSS or a universal maximum. More aircraft, projectiles, effects or
events can use more memory. The bound is on frame count, not bytes.

The probe emitted per-second samples and tracked the largest rolling window;
it was used only for memory accounting, not performance figures. Its temporary
source hook was removed and the production server rebuilt before validation.
The shipped binary contains no memory-probe code.

## Validation and evidence

Worker tests simulate five seconds at 120 Hz with no writer progress, in
addition to the chunk awaiting sync. Every frame and a feedback flag survive;
the completed flagged recording survives a retention limit of zero. The test
uses a gate instead of a wall-clock sleep. At 7,200 outstanding frames, the
next handoff still returns without waiting, accepted frames drain to a partial
and cleanup joins. Budget replenishment, frames already received but not yet
flushed, errors and shutdown remain covered.

The byte-equality test compares 7,205 frames and companion-log output against
the original synchronous write sequence, across both sync boundaries and the
final partial chunk. The underlying writer checks unchanged 120-frame chunks
and 3,600-frame syncs. Host tests cover rotation, Stop, migration step-down,
destruction and worst-tick interval reset. The headless battery checks restart
and quit, and the multiplayer feedback scenarios check both enabled and
disabled feedback, retention, exports and playback.

The 2,400-tick, 15-versus-15 AI probe keeps checksum `cbd959d1f838e9db` at the
default observation rate and `bcdea9dc8f961940` with `--ai-observe-hz 120`.
No simulation or AI behavior changed. Power failure was not injected.
Windows and macOS runtime execution were not tested.

Evidence is local under
`/home/john/Development/T.O.R.E-Fighters/.local/next-round/recorder-io/`:
`measure.py`, the saved binaries, `before/`, `after/` and `budget60/` contain
exact configs, stdout, disk-load totals, summaries, recordings and reader
reports. `budget60-memory/` holds the instrumented memory run;
`follow-up/memory_probe.rs` and `follow-up/memory-summary.json` describe the
accounting. The session `report.md` records final checks and commit identities.
Retail-derived recordings and imported media remain uncommitted.
