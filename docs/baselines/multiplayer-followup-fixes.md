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
`mp/flight-replay-fixes`. John had it merged into `multiplayer` on 2026-10-02
(fast-forward to `bbf3f613`).

## Findings and changes

| Item | Root cause | Change and limits |
| --- | --- | --- |
| NSW HUD percentage | Steering had an angle query but no available-authority readout. | Sim supplies the fraction independently of pedals. HUD shows nonzero whole percent under BRAKE with wheels supporting the aircraft and hides the inactive readout. HOOK and MSL move down to preserve spacing. |
| Nose gear braces turning | Steering rotated every face in the nose-gear group. | Reviewed wheel/strut face lists restrict steering. Separate braces and doors retain their pose; full gear retraction still works. Single-panel source wheel/strut art moves together. |
| Missiles circling destroyed aircraft | Seekers could retain a dead airborne body, while physical collision rejected zero health. Lost seekers also kept turning toward a passed intercept. | Guns and missiles hit present wrecks. No repeat kill, system damage or pilot injury. An unobserved passed intercept stops steering, with same-target reacquisition and finite lifetime retained. Dynamics remain in sim. |
| Human wreck collision/replay | Launcher health also represented physical presence. | World derives presence from the flight wreck lifecycle. Combat tape v7 records presence separately; v2 through v6 remain readable. |
| F-14 swept wings | Wing rig used rear inner roots. The left wing was one source unit inward, the right tail forward root was one unit outward, and the initial lift floated the wing above its outer attachment. Vapor attachment duplicated the old pivots. | Matching front-root pivots, corrected wing/tail offsets and a fitted two-inch outer-root clearance seat the wings beneath the fixed glove. Mesh and vapor tips share the corrected placement. Sweep timing and flight behavior are unchanged. |
| F-14 unequal exhausts and body holes | Source left outlet corners collapse differently from the right, the right horizontal plume spans three model units while the left spans two, and six three-edge panel boundaries remain open. John reports these defects in retail too; retail was not independently run. | Mirror the better formed right outlet/collar onto the left, fit horizontal plume width to the outlet, and close the six seams from neighboring positions/UVs. This is an opinionated source-art correction in the app, leaving the reader and source files intact. |
| F-14 hook spike | Its source branch contains two untextured collapsed triangles; the earlier width fit only made a thin solid spike. | Using John's suggested F-22N reference, the agent reuses its loaded textured two-sided blade. Uniformly fit it between the existing F-14 hinge and tip, retaining texture cutout, original UVs and F-14 retraction. F-22N is unchanged. This is an opinionated visual substitution with fitted placement. |
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
HUD text spacing and inactive-label suppression; stationary braces under steering; F-14 front-root invariance and wing/tail symmetry,
height, UVs and vapor attachment; guns/missiles against ownship and target wrecks;
body removal, no duplicate kills and no stale-point orbit; fresh reacquisition;
combat tape compatibility; drone profile persistence, chord releases, focus and
controller loss; movement while paused; direct camera selection at 4:3, wide and
tall aspect ratios; and GPS spacing independent of native distance coordinates.

Thirty targeted battery scenarios passed:

- The new `flight-wreckcontact-f18`, including the imported gun and AIM-120.
- Fourteen aircraft combat-evidence scenarios, each writing and replaying tapes.
- Replay drone/object views, F/A-18D and F-14 model views, and replay menu pages.
- Three render scenarios for F-14 top/side attachment alignment and the inactive steering HUD.
- Four F-14 geometry captures: exhaust top, rear belly, nose belly and deployed devices.
  Synthetic tests check reflected winding/materials, seam UVs/outward normals and plume width.
- Four hook captures: both sides deployed, half retracted and stowed. A synthetic
  check verifies donor proportions, fitted endpoints, UV/material retention,
  opposite normals, rigid retraction and hiding at zero extension.

Private results are under `.local/flight-replay-followup/battery/`, runs
`20261002-103111-followup`, `20261002-103359-views`,
`20261002-114453-alignment`, `20261002-122630-mesh-repair`, and
`20261002-124424-hook`. Logs, calibration inputs
and captures remain in `.local/flight-replay-followup/`; no retail art was committed.

GPU smoke passed through `tools/agent-run.sh`. Reviewed captures show NSW beneath
BRAKE, its absence during the takeoff roll above the steering cutoff, F/A-18 nosewheel steering with stationary braces, and F-14 extended, fully
swept and side views. The F-14 full-sweep capture uses the existing overspeed pose
probe for one tick; it does not test an overspeed crash. The ground capture guard
now permits the existing nosewheel probe with a ground start. Geometry captures
show matching exhausts and closed marked seams with devices stowed and deployed.
This repairs the reviewed defects, not every irregularity in the low-resolution
source model. Intentional intakes and cockpit/device openings remain. The hook
captures show its original F-22N striped shank and hooked end from both sides,
with the cutout background retained and the stowed blade hidden. All thirteen
retail airframes loaded successfully through the updated shared atlas path.

Physical controller input and Tacview's own map display were not manually tested.
Windows and macOS runtime presentation remain CI/manual checks. Geographic
calibration residuals and the fitted wing-height adjustment are the main review
limitations.
