# Multiplayer follow-up validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-02, Linux x86_64. This pass follows the
[original flight/replay fixes](multiplayer-flight-replay-fixes.md) on
`mp/flight-replay-fixes`. The worktree is retained for review, not merged.

## Findings and changes

| Item | Root cause | Change and limits |
| --- | --- | --- |
| NSW HUD percentage | Steering had an angle query but no available-authority readout. | Sim supplies the fraction independently of pedals. HUD shows whole percent under BRAKE with wheels supporting the aircraft. HOOK and MSL move down to preserve spacing. |
| Nose gear braces turning | Steering rotated every face in the nose-gear group. | Reviewed wheel/strut face lists restrict steering. Separate braces and doors retain their pose; full gear retraction still works. Single-panel source wheel/strut art moves together. |
| Missiles circling destroyed aircraft | Seekers could retain a dead airborne body, while physical collision rejected zero health. Lost seekers also kept turning toward a passed intercept. | Guns and missiles hit present wrecks. No repeat kill, system damage or pilot injury. An unobserved passed intercept stops steering, with same-target reacquisition and finite lifetime retained. Dynamics remain in sim. |
| Human wreck collision/replay | Launcher health also represented physical presence. | World derives presence from the flight wreck lifecycle. Combat tape v7 records presence separately; v2 through v6 remain readable. |
| F-14 swept wings | Wing rig used rear inner roots; wing surfaces lay below the fuselage deck. Vapor attachment duplicated the old pivots. | Front-root pivots and a fitted 17-inch lift clear the deck. Mesh and vapor tips share the corrected placement. Sweep timing and flight behavior are unchanged. |
| Drone input configuration | Replay input bypassed the profile system and used literal keys. | Catalog entries, separate replay desktop assignments, device bindings and normal held/release handling. Playback pause still permits camera movement; Escape menu, focus loss and disconnect clear holds. Other replay shortcuts remain fixed. |
| Replay view popup | Right-click always called object picking, including on the camera button. | The camera button opens explicit view choices. Scene clicks retain object menus. Camera choices are excluded from dynamic object-list refresh. |
| Tacview geography | Guessed centers and a physical distance conversion ignored the source maps' geographic compression. | All 16 centers and axis scales are calibrated from explicit public airport references. Native coordinates retain game distances. [Measured errors and uncertainties](theater-georeference.md) remain significant and are not a claim of exact GPS terrain. |

## Checks

The complete workspace test run and build passed. Clippy with warnings denied,
formatting, Python tests, documentation headers, source asset guard and both
binary asset guards passed. The controls master list was regenerated from the
catalog. The synthetic ACMI golden changed in geographic coordinates, geographic
yaw, reference time and calibration comments; native U/V/Heading remain intact.
The combat render hash changed only in modeled geometry, reflecting the F-14 rig.

The [Apple Silicon job](https://github.com/john-overton/T.O.R.E-Fighters/actions/runs/37029327276/job/110911884281)
measured three changed combat fingerprints: guns/damage `8ae03ea93fe341e8`,
guided missiles `75637b24dee28693`, and player countermeasures `1bab4b4093d319c9`.
These scenarios include destroyed bodies and missile loss/reacquisition. Their
physical hit, observation, projectile and effect fields change under the requested
rules. Updated only those saved values; the two-run determinism and required-event
assertions remain. The other 1,012 sim tests passed in that job.

New synthetic checks cover steering authority at 0, 10, 17.5, 25 and 40 mph;
HUD text spacing; stationary braces under steering; F-14 front-root invariance,
height, UVs and vapor attachment; guns/missiles against ownship and target wrecks;
body removal, no duplicate kills and no stale-point orbit; fresh reacquisition;
combat tape compatibility; drone profile persistence, chord releases, focus and
controller loss; movement while paused; direct camera selection at 4:3, wide and
tall aspect ratios; and GPS spacing independent of native distance coordinates.

Nineteen targeted battery scenarios passed:

- The new `flight-wreckcontact-f18`, including the imported gun and AIM-120.
- Fourteen aircraft combat-evidence scenarios, each writing and replaying tapes.
- Replay drone/object views, F/A-18D and F-14 model views, and replay menu pages.

Private results are under `.local/flight-replay-followup/battery/`, runs
`20261002-103111-followup` and `20261002-103359-views`. Logs, calibration inputs
and captures remain in `.local/flight-replay-followup/`; no retail art was committed.

GPU smoke passed through `tools/agent-run.sh`. Reviewed captures show NSW beneath
BRAKE, F/A-18 nosewheel steering with stationary braces, and F-14 extended, fully
swept and side views. The F-14 full-sweep capture uses the existing overspeed pose
probe for one tick; it does not test an overspeed crash. The ground capture guard
now permits the existing nosewheel probe with a ground start.

Physical controller input and Tacview's own map display were not manually tested.
Windows and macOS runtime presentation remain CI/manual checks. Geographic
calibration residuals and the fitted wing-height adjustment are the main review
limitations.
