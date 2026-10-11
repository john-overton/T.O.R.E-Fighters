# Air missile sag validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, CL-A2A, Linux x86_64, 2026-10-11, based on `07e89569`.
[Behaviour and
provenance](../spec/missiles.md#drop-launch-sag-and-cruise-profile).

## Method

`--missile-sag --no-audio` reads the user's imported records. It follows a
continuously known stationary target over flat ground at 120 Hz, measuring swept
closest approach until the first pass, ground contact or expiry. Aircraft launch
at target height with 600 ft/s forward velocity. Sea Sparrow launches stationary
from a 20 ft rail aimed at the target. This tests motion independently of launch
permission, seeker loss, target evasion, fuzes and damage. Miss is distance to
the target point, not distance to an aircraft's collision volume.

The probe also calls the production fly-out predictor. Its 25 ft acceptance
sphere and lack of future terrain differ from the measurement's first-pass
closest approach. Coasting sag ticks establish that each selected long-range
shot actually exercises the correction.

Synthetic live-guidance tests use the same target heights, 4 nmi separation, 600
ft/s launch speed, a two-second motor, no thrust gain or deceleration, and 600
ft/s final speed, with powered and unpowered turn rates of 1,000 record units.
The three A2A profiles and two SAM profiles (SA-6 and Sea Sparrow) receive a
continuous support observation. Their results compare the existing velocity-only
steering with path holding in the same test; a separate launch-selection test
checks that all sagging records opt in, including unnamed synthetic weapons.
Fixtures contain no retail bytes. The prediction reference retains its full
flight loop and adopts the corrected path vector so optimization equivalence
remains tested.

All direct runs use `.local/tmp-cl-a2a/profile`, copied from `surface-data`,
with private log paths. Raw evidence is local under `.local/tmp-cl-a2a/`. No
retail comparison or windowed playtest is part of this pass.

The imported SAM survey covered all 13 supported-radar and six infrared
profiles. Sea Sparrow is the only one with flag 0x4 (`flags=0x12345`). SA-6 has
`flags=0x12341`, so it remains a non-sagging control in launch-selection tests.
Synthetic SA-6 motion deliberately sets the sag flag to check that the
correction depends on the record, not the name.

## Imported-record results

The selected rows all spend time coasting with nonzero sink. The initial sweep
at 2 to 30 nmi did not coast for AIM-7 or AIM-120, so the final 128-row sweep
extends to 120 nmi. These distances isolate fitted motion and do not override
seeker range, support or launch restrictions in normal combat.

| Missile | Target height, ft AGL | Horizontal range, nmi | Miss before, ft | Miss after, ft | Before outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| AIM-7 | 300 | 40 | 30,869.621 | <0.001 | ground |
| AIM-7 | 10,000 | 40 | 2.074 | <0.001 | passed |
| AIM-9M | 300 | 12 | 2.036 | <0.001 | passed |
| AIM-9M | 10,000 | 12 | 1.524 | <0.001 | passed |
| AIM-120 | 300 | 50 | 29,627.681 | <0.001 | ground |
| AIM-120 | 10,000 | 50 | 1.394 | <0.001 | passed |
| Sea Sparrow | 300 | 12 | 23,626.012 | <0.001 | ground |
| Sea Sparrow | 10,000 | 12 | 1.743 | <0.001 | passed |

After correction all eight pass the target point and have a predicted intercept.
The battery requires a miss under 1 ft, a completed first pass, a predicted
intercept and at least one coasting sag tick. Its check rejects all eight old
rows and accepts all eight corrected rows. The three old low-altitude ground
impacts illustrate the predictor's existing terrain limitation: it predicted
reach even when the actual path hit the ground first. No gameplay constants were
tuned.

The before executable was built from the unchanged simulation at `07e89569` with
the measurement probe added. The extended sweep used that same simulation
library and probe module through a local headless runner reading the private
pack. The corrected sweep ran through `tore-app --missile-sag --no-audio`. Raw
tables: `.local/tmp-cl-a2a/before.csv` and `after.csv`.

## Single-player guard

The guard against `sf-8b1fdebf`, using the private profile and `tapes-surface`,
reports **SAME 39 / DIFFERENT 13 / MISSING 0**. It exits 1 for these intentional
differences. The six cases below each account for two differences, normalized
text and the mission recording. Shot numbers are those in the recording's
`summary.txt`; closest approaches here are rounded to whole feet by that
exporter, unlike the isolated motion measurements above.

| Case | Missiles and measured difference | Outcome |
| --- | --- | --- |
| `c-ai-dead-leader` | AIM-120 #1, AA-11 #2 and AA-12 #3/#4 still approach to 132, 145, 155 and 138 ft at summary precision. Small impact-direction changes propagate to aircraft positions, including a 1 ft rounded marshal X coordinate, and checksums. | All four hit; damage, event ticks and final result unchanged. |
| `probe-attack-once` | AIM-120 #10 approaches to 139 ft instead of 138. AIM-120 #8 and AIM-9M #9/#11 retain their displayed closest approaches. Player final X rounds 0.1 ft differently. Gun shots precede these missiles and are unchanged. | Hits, spoofed shot, damage, event ticks and final result unchanged. |
| `probe-attack-order` | Four AIM-120 and three AIM-9M shots retain all displayed closest approaches. Recorded trajectories and the final checksum change; closest-approach differences are below the summary precision. | All outcome and damage events, including their ticks, unchanged. |
| `probe-legacy` | AIM-120 #2/#3/#4: 153/152/145 ft becomes 155/154/147 ft. #3's peak speed rounds from 2,559 to 2,558 kt. #1 stays 136 ft. | Three hits and one spoofed shot; damage, event ticks and final result unchanged. This is the legacy flight adapter with the spec weapons adapter, not a changed flight default. |
| `probe-mission-hold` | AIM-9M #4: 1,285 to 1,183 ft, peak speed 2,473 to 2,472 kt. Flare #52 spoofs it at tick 4892 instead of 4891, the same roll and outcome. AIM-120 #1/#2 and AIM-9M #3 retain displayed misses. Enemy 1-1's final position shifts by about 3 ft. | Same one hit, three spoofed shots, damage and final result. |
| `probe-quick` | AIM-120 #2: 135 to 137 ft, peak speed 2,559 to 2,558 kt. The other three AIM-120 and two AIM-9M shots retain displayed misses. | Same hits, damage, event ticks and final result. |

The existing launch motion stores the ejection sink until the first powered
motion step clears it. Enabling path holding also includes that stored sink in
the preceding steering calculation, so small differences can appear before
burnout. This pass preserves that timing and every motion constant. Altered hit
directions can change the later aircraft motion without changing damage.

All other guard recordings, flight-only runs, combat tapes and smoke evidence
are identical. The remaining difference is the roster text, detailed below.
Local evidence is `guard.log`, `guard/` and `guard-analysis/` under
`.local/tmp-cl-a2a/`. The exported event lists were compared for hit,
destruction, shot outcome and final-result changes; only the one-tick spoof
timing above moves among those fields.

 ### Roster difference

The thirteenth item, `text/roster-7200.txt`, changes exactly three rows. A local
copy of the roster runner, restricted to those rows and with read-only shot and
peak logging, reproduces both sets of guard numbers exactly. The before build
uses `missiles.rs` from `07e89569`; the after build uses this slice. Both read
the same private pack. The diagnostic source and logs are local under
`roster-diagnostic/`, `roster-before.log` and `roster-after.log`.

| Row | Peak roll before / after, deg/s | Peak turn before / after, deg/s | Missile explanation |
| --- | ---: | ---: | --- |
| Su-27 Ace | 199.476 / 196.329 | 89.687 / 245.722 | AA-11 then three AA-12 launches. The third shot, an AA-12, hits aircraft 1 at loop tick 5307 instead of 5306, still doing 145 damage. Both peak pairs occur on that already-dead aircraft's wreck, at tick 5741 before and 5473 after. Four shots still resolve as hits, for 84, 138, 145 and 162 damage. Final hp stays player 0, aircraft 1 zero, aircraft 2 145. |
| MiG-21 Experienced | 45.054 / 45.287 | 6.732 / 6.732 | Three AA-2 launches. The second hits at loop tick 6768 instead of 6769; the first still hits at 2039. Damage remains 105 and 92, with the third shot still flying at the end. The changed roll peak belongs to living aircraft 1, at tick 6584 instead of 6889. Final hp stays player 0 and both AI aircraft 86. |
| MiG-21 Ace | 45.053 / 45.287 | 6.732 / 6.732 | The same AA-2 launch and outcome changes as Experienced; final health is identical. |

The diagnostic samples distance to the original aim each tick, rather than the
recording exporter's swept/impact report. Su-27's first AA-11 and AA-12
distances change by less than 0.01 ft. Its third shot approaches the player aim
to 1,413.308 / 1,386.527 ft before hitting the other aircraft; its fourth
approaches to 153.450 / 155.362 ft. MiG-21's first AA-2 stays about 140.375 ft;
the second changes from 129.107 / 128.972 ft (Experienced / Ace) to 136.283 /
136.146 ft. These shots still hit. Launch counts, dropped counts and all 144
roster PASS results are unchanged. The A/B run isolates missile path holding as
the cause; no AI control or decision rule was edited.

## Automated checks

The requested crate command passed: **4,725 passed, 70 ignored, zero failures**,
including binary/example and documentation tests. Library/app totals are
`tore-app` 1,697 passed / 31 ignored, `tore-session` 514 / 26, `tore-sim` 1,671
/ 4 and `tore-world` 832 / 3. The extra 11 passes and six ignores are
binary/example and documentation tests.

`TORE_GOLDEN_VERBOSE=1 cargo test --locked -p tore-sim -p tore-world fingerprint
-- --nocapture` passed before and after. All 440 golden output lines, including
28 scenario totals, are identical on Linux. These fixtures do not exercise the
retail sagging missile paths. No golden constants or files were changed; macOS
validation remains for CI. The simulation's self-repeat checks also pass.

Strict workspace/all-target Clippy with `--locked`, the locked workspace binary
build, 344 Python tests, the source and both binary asset checks, and the
documentation check passed. Battery selection now includes surface and A2G
families for shared missile-motion changes, and includes `flight-missile-sag` in
the combat family. The focused windowless list has 57 scenarios, including the
available missile-acceptance runs, all `ai-fight-*` cases, missile tactic
regressions, replay missile recording, laser missiles, SA-6 engagement, low
SA-2, SA-2 battery support and chaff/flare interaction. Windowed and opt-in slow
acceptance runs are outside this pass.

An initial changed run used an isolated battery output directory. That also
removed the timing history used by scenario selection, which selected the
exhaustive `ai-probe-matrix`. Its other 190 scenarios passed; the matrix was
intentionally terminated while still progressing. This interrupted run is
supplementary evidence, not the acceptance run. Its log is
`.local/tmp-cl-a2a/battery-initial.log`, with results under
`.local/tmp-cl-a2a/battery/20261011-145215-cl-a2a-changed/`. Acceptance uses the
requested command's normal output directory and timing history, and separate
explicit missile scenarios.

The exact requested changed-scenario command, with normal timing history,
passed **191/191** at six jobs and no windows. Results:
`.local/battery/20261011-151431/summary.md`; retained console log:
`.local/tmp-cl-a2a/battery-changed.log`. Its 72 selected Python checks also passed.

The explicit focused run passed **57/57**, at six jobs on scenario profile
copies. Results:
`.local/tmp-cl-a2a/battery/20261011-153059-cl-a2a-missiles/summary.md`; console log:
`.local/tmp-cl-a2a/battery-missiles.log`. Final formatting, asset and documentation
checks pass. No gameplay failure was observed in either completed battery run.
