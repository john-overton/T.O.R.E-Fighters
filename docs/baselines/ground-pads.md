# Ground-pad foundations validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, measured on Linux on 2026-10-21. This validates a
fitted host rule, not original-game parity. The before binary is worktree
base `9ee008069361571ecedb66d1bcefbebdb26ad7cf`. The after binary includes
this change. Both read the task's isolated imported profile. No retail
executable was run. The rule lives in
[grounded objectives](../spec/surface-defenses.md#grounded-objective-assemblies).

The UKR `QUFACT` seed-1 reactor pad reproduced the report: the concrete was
broken into separate visible pieces even on flat terrain. Coplanar drawing
was one cause. The all-theater survey also found real terrain crossings,
which a drawing bias alone cannot fix. Airports already had a flat contact
surface and a hidden ground recess. Ordinary objects had only origin support
or a bounded vertex lift. The correction adds shared terrain foundations
before placement and follows actual bottom-face triangles for hidden
substrates. It also keeps fixed SAM sites and airfield-adjacent foundations
on the correct support plane.

`--surface-dump --ground-pads` built 329 worlds: all 75 live static layouts,
every template offered by the theater tables at its authored and seed-1
relocated position, and Ukraine's three extra templates. It checked 49,934
qualifying placements across 72 types. Each footprint was sampled on a
17 by 17 grid against both terrain and the simulation's surface height.
Every error was 0.0000 ft at the printed precision, below the 0.05 ft limit.
Counts include base-layout objects repeated in each objective world.

| Theater | Placements checked |
| --- | ---: |
| BAL | 3,541 |
| CUB | 2,226 |
| EGY | 3,338 |
| LFA | 521 |
| FRA | 7,645 |
| GRE | 2,348 |
| IRA | 2,497 |
| KURILE | 848 |
| TVIET | 5,333 |
| SPA | 1,599 |
| APA | 1,711 |
| PGU | 1,950 |
| NSK | 1,689 |
| WTA | 2,529 |
| UKR | 8,264 |
| VLA | 3,895 |

The full placement list is the battery's `surface-ground-pads.log`; the
sorted 72-type inventory is `survey-types.txt` in scratch. Covered types
include factory and reactor floors, concrete lots, city and town blocks,
hangars, bunkers, HAWK and SA-3 site objects, and the fixed SA-2 NT assembly.
Movable units do not create terrain foundations. Map-tied bridge and road
grades, ship waterlines and runway support keep their existing rules.
The companion `surface-grounding-sites` source-placement audit passed;
its bridge/road exceptions are not a new proof of every bridge approach.
Compiled campaign missions have no runtime and were not instantiated.

The worst captured cases use the same camera settings before and after.
Footprint intrusion measures terrain above the old base plane, including
interior samples that the old vertex-only rule did not measure.

| Case | Before measurement | Capture prefix |
| --- | --- | --- |
| UKR `QUFACT`, REACTB | Coplanar breakup at the seed-1 site | `ukr-pad-day`, `ukr-pad-dusk-1906`, `ukr-sensor-day`, `ukr-sensor-dusk-1906` |
| `~UKRF`, CITY2 ordinal 50 | 174.84 ft footprint intrusion; 167.34 ft at shape vertices | `ukrf-city2` |
| EGY, CTYBKC ordinal 38 | 98.75 ft footprint intrusion | `egy-city` |
| `~FRA0`, CTYBKC ordinal 94 | 91.09 ft footprint intrusion | `fra-city` |
| LFA `QLFSTOR`, LOT ordinal 4, authored site | 48.79 ft footprint intrusion | `lfa/still-00-t0` through `still-02-t0` |
| KURILE, CROPA ordinal 30 | 22.50 ft footprint intrusion | `kurile-crop` |
| APA `QAPSAM`, SA2A ordinal 1 | Old lift 13.75 ft, downhill gap 27.58 ft | `apa-sa2` |
| KURILE, HANGRB ordinal 42 | Foundation/runway disagreement 34.70 ft during the initial expanded-rule audit, corrected by airport support priority | `kurile-hangar` |

Images are in scratch `before/` and `after-support/`. Each scene has
oblique, low and overhead views. UKR also has noon and 19:06 dusk captures
and a 55-degree downward view resembling a sensor camera. These use the
normal scene renderer, not an AC-130 instrument overlay. The Falklands
comparison reuses a recording of the authored site at tick zero, which also
checks that the old recording still loads. Every GPU run used
`tools/agent-run.sh`. The host GPU is an NVIDIA GeForce RTX 4070.

The seven focused battery scenarios passed: `surface-ground-pads`,
`surface-grounding`, `surface-grounding-sites`, `surface-resolve-all`,
`surface-objective-destroy`, `surface-objective-protect` and
`surface-runway-stays`. The legacy airport path was separately checked on
KURILE and NSK: 34 worlds, 1,041 qualifying placements, zero support errors.

Protocol 63 and its refreshed wire golden cover the changed terrain contact
and object placement. Replay bytes and readers are unchanged. Old recordings
load against corrected terrain, so old simulation checksums near altered
sites are not promised to match. The default 1,200-tick headless flight output
is byte-identical before and after, SHA-256
`64832b8f0e9baafe6ececb4940e962c311f5a56e58a8d7ff305c4ed8fab510a1`.
Synthetic simulation golden expectations were not changed; they construct
terrain without these imported foundations. Recorded Apple-silicon values
are not compared on Linux.

Final validation passed: 6,976 workspace tests, 103 ignored, Cargo exit
status zero; workspace build and all-targets Clippy with warnings denied;
Rust format and documentation checks; 490 Python tests with four skips;
source and both binary asset checks; and the display smoke test. Synthetic
regressions cover interior ridges, overlapping and rotated foundations,
source-cell edges, distant float coordinates, airport-plane priority and
authored datum preservation, fixed/mobile eligibility, triangular substrates,
texture coordinates and cache invalidation.

The geometry rule remains fitted. Rectangular footprints can level bare
ground beside irregular or rotated objects, and the 512 ft transition is a fitted embankment, not original-game grading. Original slope treatment is unknown. Windows and macOS
runtime/GPU checks were not run. Detailed check logs, image links and the
handoff report are in the task scratch directory:
`/home/john/Development/T.O.R.E-Fighters/.local/next-round/bugfix3/ground-pads/`.
