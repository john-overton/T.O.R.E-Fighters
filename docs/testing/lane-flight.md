# Flight lane

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The flight lane flies the player's aircraft from ground start to touchdown in all fourteen
flyable identities (`f18`, `rafale`, `f14`, `a4e`, `x31`, `mig29`, `su27`, `mig21`, `su25`,
`mig23`, `su35`, `f22`, `f22n`, `faxx`), and checks the weapons, countermeasures, systems and
instruments that go with it. It was built in the 2026-09-28 overnight bug battery. The scenario
file is [`tools/battery_scenarios/flight.py`](../../tools/battery_scenarios/flight.py); how the
runner works is in the [testing overview](README.md). The `render-*` capture scenarios live in
[`tools/battery_scenarios/render.py`](../../tools/battery_scenarios/render.py).

`flight-missile-energy` runs 240 headless imported shots through the Energy
model twice, with four workers and with one. It checks identical rows, real
hits and misses, and an upper bound of one roll per two-second hammered
episode. Eight historical Spec AIM9B shots also cover raw susceptibility above
100, which that preserved percentage-only API must clamp before evaluating.
These are regressions for repeated rolls and comparison compatibility, not
fixed Monte Carlo hit-rate targets. The broad calibration/study results are in the
[missile energy baseline](../baselines/missile-energy.md).

The `flight-missile-defeat` scenario repeats 32 long AIM120/AA8 shots
with four and one workers, requires an observed post-burn defeat and rejects
any defeated shot that scores. Two additional stationary-target AIM120 shots
use the `defeat-edge` range band, just beyond the shared kernel's last live
intercept, and repeat with one and four workers. At least one inert body must
pass within the target's fuze/contact radius without damage. The CSV columns
`defeat_s`, `closest_inert_ft`, `inert_radius_ft` and `inert_damage` distinguish
kinetic defeat from ordinary control-time expiry and expose that near pass.
Synthetic Rust tests cover
exact before/at/after contact ordering, direct/proximity/ground damage,
powered cruise, mid-countdown checkpointing, countdown cache buckets with a
defeat deadline, and same-tick warning/AI defense removal with the body retained.

The offline missile_energy_ranges example accepts a local JT directory.
`--verify` checks the shared motor-shape and powered cruise-arrival guards,
primary and secondary reference bands, class floors, decoded
minimum margins, family and radar-role ordering, long-low escape and local
perturbations. Named residual exceptions have explicit bounds; unlisted stores
use the ordinary tolerance. `--fit-class` exports fixed-budget fuel/drag
candidates for the high head and low tail conditions. `--fit-ground` fits ground
references. `--fit-motors` and `--fit-staged` also explore bounded per-store
motor durations and thrust, never control time. `tools/missile_energy_fit.py`
selects families and radar roles jointly; synthetic Python tests cover selection,
bounds and parsing. These are fitted game coefficients, not engineering data.
`--diagnostics` opts into G demand/capacity/deficit and closing-speed math,
which normal motion and prediction skip, and reports defeat
at the edge, 0.1% beyond and 10% beyond. The near-edge miss reports whether
defeat, control expiry, terrain contact or another event stopped the flight;
a continued hit is reported as a sampled-boundary limitation. --measure-pairs is historical comparison only.
Timer-fitting tools require explicit --legacy-fit-pair/--legacy-fit-sparrow
names and print a warning. No search edits source or runs during gameplay.

`--motor-shapes` reports initial thrust/weight, dry-mass load, declared and
actual impulse coefficient, burn/control ratio, sustain-entry thrust/weight,
standardized time to Mach 1, powered-hit limit and reference/edge
arrival speed and powered state. A missed reference is explicitly marked as
a miss, so its final speed is not an arrival measurement. `--refine-motor`
and `--measure-motor` accept SOURCE FUEL DRAG BOOST_S SUSTAIN_S COEFFICIENT
BOOST_SHARE for a bounded refinement or measurement of one candidate.
Both reapply the guard. The complete offline search retries an empty envelope
with factor-1.1 probes after the ordinary factor-1.5 sweep, to find narrow
beam-intercept intervals. Nonempty envelopes keep their original search;
this is still sampling, not proof that every success island was found.
Runtime advice separately keeps each sampled region and advises the one for the present range, as specified in the
[range contract](../spec/missile-energy.md#estimates-and-acceptance). Thresholds and class floors have one home in
[the data document](../formats/missile-energy-data.md#per-store-motors-and-class-control-budgets).
Run the synthetic guard and exception tests with
`cargo test --locked -p tore-sim --example missile_energy_ranges -- --test-threads=4`.

The `flight-countermeasures-cadence` scenario runs an isolated real-AI Ace
AIM9M encounter at short range, head-on, co-altitude and seed 0. It requires
at least two pairs, exactly 30 ticks within each pair, at least 240 ticks
between pair starts and no more than six releases for the single threat.
All-skill reaction, reserve and encounter transitions have synthetic tests.

The offline `ordnance_envelopes` example accepts `JT_DIR OUT_DIR [SOURCES]`.
Its default survey covers eleven altitude cases, five aspects, shooter speeds
200/300 m/s against 250 m/s, and a 150 m/s target subset at three altitudes.
It exports physical and live-seeker intervals, with every sampled transition
refined to 0.5 m. A finite scan can still miss a narrower island or hole.
Single-case modes use `SOURCE point|cell|trace|advice LAUNCH_FT TARGET_FT
ASPECT_DEG SHOOTER_MPS TARGET_MPS RANGE_M`; `cell` takes a scan ratio instead
of range and `trace` additionally takes `physical|seeker`. Range is horizontal;
the runtime `advice` result reports slant-range edges and fixed-queue work.
The trace emits Mach, G demand/capacity, motor/drag force and seeker state.
`--override key=value,...` is diagnostic only: fuel, share, boost_s, sustain_s,
coefficient, drag, lift, g, induced, navigation, control and defeat are supported.
Counterfactual control/defeat overrides must never be mistaken for shipping
policy or an approved calibration. No mode edits game data.

The synthetic `behavior_hash_requires_a_model_revision_for_rule_changes` test
pairs the model identity with a quantized trajectory/contact trace. A changed
trace requires investigating the behavior, incrementing the relevant semantic
revision, and reviewing the new pair. An injected PN perturbation verifies that
behavior changes independently of the declared identity. This finite set of
traces is a tripwire, not exhaustive proof for every branch or platform.

## Variety aircraft checks

`flight-variety-*` checks each of the 30 variety identities through real
application asset loading and 1,200 hybrid flight ticks, with finite motion, no
crash and bounded fuel/energy output. It is separate from the original roster's fighter
maneuver and AI acceptance cases. Source-backed powered-lift checks use
`tore-sim`'s `variety_flight` example; synthetic tests cover hover limits,
conversion, control release and exact-state continuation. See
[the variety flight evidence](../baselines/variety-flight.md).

`tore-sim`'s `performance_probe` example is the matching development harness for
performance numbers: level acceleration at military and afterburner power, an
afterburner climb on a speed schedule, sustained and instantaneous turns and fuel
burn for any extracted PT (`PROBE_PAYLOAD`, `PROBE_FUEL`, `PROBE_ALTS`,
`PROBE_ONLY`). Its pilots are scripted, so it records the model, not a best
pilot. The Mirage 2000 results are in the
[variety flight baseline](../baselines/variety-flight.md#mirage-2000-validation-2026-11-10).
Update (F5E-8, 2026-11-10): the Northrop F-5E results, with the full-stick pull by
fuel against the Gripen, the F-16A and the Mirage 2000, are in the
[variety flight baseline](../baselines/variety-flight.md#northrop-f-5e-validation-2026-11-10);
`f5e` joins the liftoff, approach and internal-fuel tables of `flight.py`.

The F-15E results (clean and default load, a full-aft-stick pull table by fuel,
liftoff, approach and fuel) are in the
[variety flight baseline](../baselines/variety-flight.md#f-15e-validation-2026-11-10);
`flight-liftoff-f15e`, `flight-liftoff-retail-f15e` and `flight-approach-f15e`
join the liftoff and approach scenarios (F15E-8, 2026-11-10).

Update (GRI-8, 2026-11-10): the Saab Gripen results, with the full-stick pull by
fuel against the F-16A and the Mirage 2000, are in the
[variety flight baseline](../baselines/variety-flight.md#saab-gripen-validation-2026-11-10);
`gripen` joins the liftoff, approach and internal-fuel tables of `flight.py`.

Update (B52-8, 2026-11-15): `b52` joins `VARIETY_AIRCRAFT`, `LIFTOFF_AIRCRAFT` (liftoff,
retail-stall and approach scenarios) and `HEAVY_AIRCRAFT` (`flight-variety-gcurve-b52`) in
`flight.py`, with fitted windows (liftoff 150 to 175 kt, approach 150 to 170 kt), a
9,000 tick takeoff limit (`TAKEOFF_TICKS_MAX`) and a 12,000 ft runway (`THEATER_FOR`,
Greece airport 1). `--flight-fuel` takes up to 400,000 lb and the `land` probe applies it
before it works out the approach speed. Results: the
[variety flight baseline](../baselines/variety-flight.md#boeing-b-52h-validation-2026-11-15).

Update (T160-8, 2026-11-16): `tu160` joins `VARIETY_AIRCRAFT`, `LIFTOFF_AIRCRAFT` (liftoff, retail-stall and
approach scenarios) and `HEAVY_AIRCRAFT` (`flight-variety-gcurve-tu160`, floor 1.15 G like the B-52's) in
`flight.py`, with fitted windows (liftoff 155 to 185 kt, recorded 167.8; approach 130 to 160 kt, recorded
143.6), a 6,000 tick takeoff limit and Athens (`THEATER_FOR`, 12,000 ft). `ai.py` adds
`ai-heavy-takeoff-tu160-gre-a1` and `ai-heavy-land-tu160-ukr-a2` (see
[lane-ai](lane-ai.md)). `ai-strike-tu160-stick` now starts 10 nm out, not 12: with the corner speed the stall
fit lowered, the AI's pass from 12 nm opens the bay a second late and exits without bombing, while 8, 10, 14 and
16 nm all drop the six (a timing edge of the AI's bay lead, T160-9's). Results: the
[variety flight baseline](../baselines/variety-flight.md#tupolev-tu-160-validation-2026-11-16).

Update (T95-8, 2026-11-16): `tu95` joins `VARIETY_AIRCRAFT`, `LIFTOFF_AIRCRAFT` (liftoff, retail-stall and
approach scenarios) and `HEAVY_AIRCRAFT` (`flight-variety-gcurve-tu95`) in `flight.py`, with fitted windows (liftoff
135 to 165 kt, recorded 137.1; approach 145 to 170 kt, recorded 158.1), a 10,000 tick takeoff limit and 18,000 tick
run (`TAKEOFF_TICKS_MAX`, `TAKEOFF_RUN_TICKS`) and Greece airport 1 (`THEATER_FOR`). `ai.py` adds
`ai-heavy-takeoff-tu95-gre-a1` and `ai-heavy-land-tu95-ukr-a2` (90,000 ticks; the landing tolerated off-pavement ticks like
the B-52's until T95-9, 2026-11-16, which closed its taxi-in: it now demands the pavement). Results: the
[variety flight baseline](../baselines/variety-flight.md#tupolev-tu-95-bear-h-validation-2026-11-16).

Update (MF1-8, 2026-11-11): the Mirage F1 results, clean and with the default load,
with the full-stick pull by fuel against the Mirage 2000, the Gripen, the F-5E and
the F-16A, are in the
[variety flight baseline](../baselines/variety-flight.md#mirage-f1-validation-2026-11-11);
`mf1` joins the liftoff, approach and internal-fuel tables of `flight.py` (the
windows depend on the open default-load decision).

Update (TOR-8, 2026-11-11): the Panavia Tornado IDS results, clean and with the
default load, with the full-stick pull by fuel against the F-15E, the F-5E, the
Gripen and the Mirage F1, are in the
[variety flight baseline](../baselines/variety-flight.md#panavia-tornado-ids-validation-2026-11-11);
`tornado` joins the liftoff, approach and internal-fuel tables of `flight.py`
(fitted windows 135 to 165 kt and 140 to 175 kt). Update (TOR-13, 2026-11-12):
`tornadoc` (the Tornado IDS (Clean), the Tornado without its pods and jammer)
joins the same tables and the variety, liftoff and approach scenarios with the
earlier Tornado's recorded figures; `flight-countermeasures-tornado-boz`,
`flight-countermeasures-tornado-boz-sweep67` and
`flight-countermeasures-tornadoc-none` (`--countermeasure-preview 2`) check
that the Combat version's chaff and flares are at the BOZ-107's aft face
(the `Countermeasure outlet:` and `Countermeasure origin:` lines) at the
default speed and at 650 kt, and that the Clean version carries and releases
none.

Update (B2-8, 2026-11-15): the B-2 (`b2`) joins the variety, liftoff, retail-stall and
approach scenarios of `flight.py` (fitted windows 140 to 165 kt liftoff and 140 to 170 kt
approach, recorded 158.1 and 166.2 kt at the 373,000 lb default load; the approach
scenario flies 100,000 lb of fuel, the `--flight-fuel` cap, because 65 percent of the
B-2's 167,000 lb tank is over it). `performance_probe` gained `PROBE_CLIMB=KEAS,MACH` (a
bomber's climb on a constant equivalent airspeed, its pitch held by a speed integrator),
`PROBE_ONLY=brake` (idle deceleration with and without the airbrake) and
`PROBE_TURN_MACH`. Evidence: the
[variety flight review](../formats/aircraft-variety.md#b-2-flight-review-b2-8-2026-11-15).

Update (F117-8, 2026-11-15): the F-117A (`f117`) joins the liftoff, retail-stall and approach
scenarios of `flight.py` (it was already in `VARIETY_AIRCRAFT`): fitted windows 155 to 185 kt
liftoff and 150 to 180 kt approach, recorded 177.2 and 166.2 kt at the 52,000 lb default
load, an internal fuel of 18,500 lb (the approach flies 65 percent, 12,025 lb) and the
ordinary takeoff allowance. Evidence: the
[F-117 flight review](../formats/aircraft-variety.md#f-117-flight-review-f117-8-2026-11-15).

Update (B1B-8, 2026-11-16): the B-1B (`b1b`) joins the variety, liftoff, retail-stall and approach scenarios of
`flight.py` (fitted windows 155 to 185 kt liftoff and 145 to 170 kt approach, recorded 168.5 and 166.2 kt at the 477,000 lb
default load; an internal fuel of 265,274 lb, the approach flying 65 percent of it, 172,428 lb; a 7,000 tick takeoff
allowance; Athens, GRE 1, because Simferopol's 5,532 ft is shorter than the 5,472 to 5,671 ft run). The AI lane gains
`ai-heavy-takeoff-b1b-gre-a1`, `ai-heavy-land-b1b-ukr-a2` and `ai-merged-airfield-b1b-wing2`
([lane-ai](lane-ai.md)). Evidence: the
[B-1B flight review](../formats/aircraft-variety.md#b-1b-lancer-flight-review-b1b-8-2026-11-16).

Update (F111-8, 2026-11-16): the F-111F (`f111`) joins the liftoff, retail-stall and approach
scenarios of `flight.py` (it was already in `VARIETY_AIRCRAFT`): fitted windows 150 to 190 kt
liftoff and 140 to 175 kt approach, recorded 171.8 kt after 2,944 ft at the 99,999 lb default
load and 166.2 kt (the model's cap) for the approach at 65 percent fuel (21,231 lb), an internal fuel of 32,663 lb and the
ordinary takeoff allowance. Two AI scenarios join the BMB-AI set, `ai-heavy-takeoff-f111-gre-a1`
(a wing of two at Athens at the default load: taxi, takeoff, climb-out to `Airborne`, flying on) and
`ai-heavy-land-f111-ukr-a2` (sent home with 16,000 lb of fuel and the stores: lands at 175 kt or
slower at Simferopol, rolls out, parks on the pavement); the F-111F's flaps-up minimum is above
190.5 kt at those weights, so the BMB-AI flaps law and climb-out wait apply and pass unchanged.
The wing's sweep rate (3 degrees a second) is covered by Rust tests (`swing`, `live`,
`ai_wings`, `f111_animation`, and `live_checkpoint` for a restore mid-sweep stepping on identically), not by a scenario: no check line shows the sweep. Evidence: the
[F-111F flight review](../formats/aircraft-variety.md#f-111f-flight-review-f111-8-2026-11-16).

`flight-variety-gcurve-*` sweeps the full-stick G limit of the seven transports
and airliners from 100 to 750 kt at 5,000 ft (`--maneuver gcurve`). Once the
limit has passed 1.4 G it must not fall below 1.3 G, through the top speed and
into overspeed. Before the fast-side hold
([flight model](../FLIGHT-MODEL.md#envelope-limits-and-loading)) it fell to
about 1 G well below the top speed.

## Air-to-ground weapons

`a2g-*` (scenario files
[`tools/battery_scenarios/a2g_bombs.py`](../../tools/battery_scenarios/a2g_bombs.py),
[`a2g_sights.py`](../../tools/battery_scenarios/a2g_sights.py),
[`a2g_guided.py`](../../tools/battery_scenarios/a2g_guided.py) and
[`a2g_missiles.py`](../../tools/battery_scenarios/a2g_missiles.py)) runs
`--a2g-run THEATER STEM --aircraft ID --load REC --attack KIND`, which flies a scripted
straight pass at a ground target of a Quick Mission, invulnerable, and attacks through the
real player path: the weapon keys select the station and the trigger releases. A store the
aircraft does not carry is fitted to a bomb, pod or air-to-ground missile station for the run
(`--count N`). The missile kinds also select the page and designate through the player's keys
(`--designate ag|ir|harm|visual|none`, `--unit ID`), and `--render OUT.png` draws the HUD at
the release. Each
weapon slice adds its attack kind as a submodule of `crates/tore-app/src/a2g_run/`.

| Scenario | What it flies | What must hold |
| --- | --- | --- |
| `a2g-bomb-motion` | Over `~QUCOL`: an A-4E Mk 82 from a level pass at 5,000 ft and 450 kt, an F-4B Mk 82 from a 30 degree dive released 3,000 ft above the target, an A-4E Mk 82 AIR 7-pack and an A-4E Snakeye, each released when the bomb sight's mean impact reaches the target | The solution obeys the bomb equation (fall and throw within 0.01 s and 2 ft); every bomb bursts within 2 ft of its own path and within the 1.5 degree scatter's reach of the mean point, its scatter within 1.5 degrees; the 7-pack is seven bombs in one tick for one round at the retail stagger and 66 percent of the speed; the Snakeye falls as long as the slick bomb and throws 66 percent as far ([bomb motion](../spec/air-to-ground.md#bomb-motion)) |
| `a2g-ccip-dive` | `--attack ccip` over the TVIET truck park: an F/A-18D's Mk 82 in a 30 degree dive from 2,000 ft, released when the HUD's CCIP pipper covers the truck; then a longer, shallower pass from 3 nmi | The truck is destroyed; every logged sight follows the retail arc rule (full inside 0.4 of the 20,000 ft range, one step fewer each fourteenth of the rest, the bare ring from the maximum, on retail's approximate distance), its glyph is 0x20 plus the arc, IN RNG inside the range, and the arc never shrinks as the range closes ([CCIP](../spec/air-to-ground.md#ccip-the-dive-pipper)) |
| `a2g-ccip-terrain` | `--attack ccip` toward the Vladivostok SAM site of surface seed 4 (`--unit 0x50000009`, undefended), which stands on a 768 ft plateau while the ground 24,000 ft short of it is lower: an A-4E's Mk 82 in a 3 degree dive from 15,000 and from 18,000 ft (slice ST-G, realistic CCIP) | At the release the pipper lies within 20 ft of the target on the plateau (`target-gap`), and in at least one run retail's pipper (`retail-gap`, the ground under the aircraft for the whole fall) is more than 50 ft worse (100 and 145 ft seen); the bomb comes down on the plateau |
| `a2g-level-t` | `--attack level` over `~QUCOL`: an F-4B designates the column from the A-G page, selects its Mk 82 (LVL on selection) and holds the trigger through a level pass at 5,000 ft and 450 kt | Exactly one bomb, released within three ticks of ground travel past the cue with the cross-track miss inside the radius; the release time falls and the marker rides down before it; OVERFLY after; the burst inside the record's splash radius of the target plus the 1.5 degree scatter's reach (about 595 ft; the bomb is one random draw) ([the level T](../spec/air-to-ground.md#the-level-release-t-lvl)) |
| `a2g-rocket-ccip` | `--attack rocket`: an A-4E's LAU-61s in a 10 degree dive on a truck, fired once the rocket pipper says IN RNG, held a second | Rockets hit and destroy the truck; a rocket that bursts on the ground does so within a foot of the pipper of the tick it was fired on ([rockets](../spec/air-to-ground.md#rockets-and-guns)) |
| `a2g-warnings` | `--attack ccip` with an A-4E's Mk 82 from a 30 degree dive at 300 ft, a level pass at 400 ft and one at 1,500 ft | LOW and FRAG, FRAG only, and neither; the LOW bomb lands a dud (a puff, nothing destroyed), the others burst ([warnings](../spec/air-to-ground.md#warnings)) |
| `a2g-rocket-pod` | An A-4E's two LAU-61 pods tapped three times, held for a second and held until empty; two LAU-10 pods held for two seconds | 38 rockets in the pods, one a press; four or five in a second held, a quarter second apart on the game's quarter-second clock; all 38 when held on; the LAU-10 pods hold 8 and fire one however long the trigger is held ([rockets](../spec/air-to-ground.md#rockets)) |
| `a2g-rocket-smoke` | An A-4E's LAU-61 pod held for a second over `~QUCOL` | The `motors` line: rockets whose motors lit, and missile smoke puffs in the air while they fly (bug B6: rockets have no seeker and smoked nothing before; [smoke](../spec/damage-smoke.md)) |
| `a2g-gun-key` | `--a2g-run --attack strafe`: an A-4E with an Mk 82 selected, Tab held for two seconds, the trigger never pressed | the gun fires a burst and its ammunition falls round for round, about one round in three a tracer, the selection stays on the bomb, the bomb stays on its pylon ([the gun key](../spec/air-to-ground.md#the-gun-key)) |
| `a2g-cluster` | Over `~QUCOL` (scenario file [`a2g_cluster.py`](../../tools/battery_scenarios/a2g_cluster.py), attack kind `cluster`): an A-4E CBU-87 and Mk 20 from a level pass and an F-4B RBK-500 from a 30 degree dive | The burst is the type 18 family with no crater; a vehicle inside the record's splash (2,200, 1,600 and 2,500 ft) loses the record's damage times 70 percent, falling in a line to nothing at the edge, and one outside loses none; the 28 bomblet flashes the renderer derives are the same on every derivation, inside 1,200 ft of the burst and start within its first half second ([cluster flashes](../spec/ordnance-presentation.md#cluster-bomb-flashes)); the fitted dispense draws two dispenser halves and bomblet bodies over the same burst, repeatable from the burst's place and inside the pattern ([cluster bomb dispense](../spec/ordnance-presentation.md#cluster-bomb-dispense)) |

`a2g-catalog` (menus lane, [`a2g_catalog.py`](../../tools/battery_scenarios/a2g_catalog.py))
runs `--validate-ordnance`, whose audit covers the air-to-ground records on the Load
Ordnance page and the Cheat-only novelty bombs: the normal and Cheat catalogs are checked
against the station rules on all 45 aircraft, and Boulder and Moose must have no capacity,
be refused and fail validation without Cheat loading ([novelty
rule](../spec/ordnance-presentation.md#novelty-bombs-are-cheat-only)).
| `a2g-laser-self` | `--attack laser` over `~QUCOL`: an F-16C designates a T-80 from the A-G page (T), its Pave Spike lases automatically with the GBU-10 selected, and the bomb is released from 15,000 ft at 480 kt on the HUD's IN RNG cue; again with Ctrl+L turning the laser off 20 s after the release | The bomb leaves at 90 percent of the speed, guiding on aircraft 0's spot from inside its reach, its time to impact counts down, and it destroys the tank; with the laser off it goes from guiding to blind, falls and misses, and the tank is unhurt ([the laser](../spec/air-to-ground.md#the-laser)) |
| `a2g-laser-buddy` | Two seats (Open seating): an Su-25 two miles behind designates with Ctrl+X (Easy targeting) and lases by hand; an F/A-18D, which has no designator, drops a GBU-10 from 12,000 ft | The release guides on the Su-25's spot (buddy yes, own laser off), lands within 40 ft of the tank and destroys it ([buddy lasing](../spec/air-to-ground.md#buddy-lasing)) |
| `a2g-jdam-cloud` | `--attack jdam --condition cloudy`: an F-16C's GBU-30 from 12,000 ft at a standing object of the TVIET truck park, then at a `~QUCOL` tank that drives on at 50 ft/s | The first lands on the stored point and destroys the object; the second lands within 40 ft of the stored point and at least 500 ft from the tank, which is unhurt ([guided bomb release range indicator](../spec/air-to-ground.md#guided-bomb-release-range-indicator)) |
| `a2g-bar` | `--attack jdam --release bar` then `--attack laser --release bar`: an F-16C's GBU-30 from 12,000 ft at 450 kt at a standing object of the truck park, then its GBU-10 on its own Pave Spike from 8,000 ft at 400 kt at a `~QUCOL` tank; the run turns onto the designated object and holds the trigger in LVL | One release each, within 60 ft of the T's bar (along track) and within 10 degrees of the stem; the bomb guides to the end, lands within 60 ft of the target and destroys it, its time to impact counts down ([the level T for guided bombs](../spec/air-to-ground.md#guided-bomb-release-range-indicator)) |
| `a2g-laser-missile` | `--attack laser`: an Su-25's AS-14 from 3,000 ft, on its built-in designator's spot | Released guiding on its own spot; ends guiding within 60 ft and destroys the tank ([weapon classes](../spec/air-to-ground.md#weapon-classes)) |
| `a2g-at2` | `--attack at2`: a Mi-24 300 ft up, 2 nmi out at 60 kt, launches an AT-2 at a `~QUCOL` tank (Easy targeting's Ctrl+X: it has no ground page); again turning 90 degrees away 2 s after the launch | The AT-2 is command guided and hits (the T-80 takes 120 of its 200 hit points); turning away makes it blind for good, it never guides again and misses ([weapon classes](../spec/air-to-ground.md#weapon-classes)) |
| `a2g-missile-burnout` | An A-10's AGM-65G from 6 nmi at 3,000 ft over `~QUCOL` | The `motors` line: one motor lit, smoked and burnt out while the missile still flew, which ends its smoke and its drawn flame (bug B7; [smoke](../spec/damage-smoke.md)) |
| `a2g-terrain-cruise` | F/A-18D AGM-65A at `VLA QVARMOR`, target `0x50000000`, 6 nmi, 1,100 ft above target ground, heading 60, 350 kt | Logged altitude rises above 1,900 ft over the ridge, then the missile resumes its profile and kills the tank within 40 ft. Without terrain clearance the logged maximum was 1,866 ft ([cruise clearance](../spec/missiles.md#cruise-terrain-clearance)) |
| `a2g-terrain-mask` | F/A-18D AGM-65G at `VLA QVSUP`, target `0x50000000`, 6 nmi, 300 ft above target ground, heading 180, 350 kt, 10 s | Terrain rises above the measured sight line near the target; TERRAIN MASK, no launch. This caught a ridge skipped by the old eight-probe visibility check ([terrain gates](../spec/air-to-ground.md#terrain-at-lock-and-launch)) |
| `a2g-maverick` | Over `~QUCOL`: an A-10's AGM-65G on a tank designated on the A-G page (Ctrl+R, T), the same with nothing designated (BORE), an AV-8B's AGM-65G on a tank designated on its FLIR (M, T), an A-10's AGM-65A on the tank Enter picks, and an AGM-65A from 6 nmi on the A-G page | CUED with the seeker locked on the designated tank, launched at it and killed; in BORE the candidate and the missile's target are the strongest return in the 5 degree bore circle, locked at release, killed; the FLIR run kills, both AGM-65A runs hit and the 6 nmi shot ends within 40 ft of the tank ([missiles](../spec/air-to-ground.md#missiles-and-the-harm-boresight-cue)) |
| `a2g-slam-shrike-as15` | SLAM on the column, Shrike on the Straight Flush, AS-15 on the A-G track | Each selected ground target is destroyed |
| `a2g-laser-as30l` | Su-25 AS-30L (AS30.JT) on its built-in designator | Guides on its own spot and destroys the tank |
| `a2g-harm-bore` | Over `~QVRDRI` (seed 4, heavy defenses): an F-4G's AGM-88 with nothing designated, at an SA-6 battery's Straight Flush at ace and at novice experience | The HUD cue lists several emitters with the radar as the BORE candidate; the HARM leaves in BORE at it; the ace radar shuts down within 10 nm and the HARM keeps homing, locked, and kills it; the novice radar stays on and dies |
| `a2g-harm-page` | The same F-4G on the HARM page (M), T to the Straight Flush | The designation turns the cue to CUED; the HARM leaves in CUED at the radar and kills it |
| `a2g-antiship` | Over `~QUSFLT`: an F/A-18D's AGM-84A and a Rafale's AM-39 at the Krivak designated on the A-G page | Launched midcourse at the Krivak, the seeker goes active on it and it is hit |
| `a2g-antiship-b52` (B52-6, 2026-11-15) | Over `~QUSFLT`: two AGM-84E from the B-52's underwing pylons (`--station 1 --count 2`) at the Krivak designated on the A-G page through the B-52's radar | Designated, launched at the Krivak, the ship is hit |
| `a2g-antiship-tu26` (T26-6, 2026-11-16) | Over `~QUSFLT`: two AS-16 from the Tu-26's outer glove row (`--station 3`) and two from its belly bay (`--station 4`) at the Krivak designated on the A-G page through the Tu-26's radar | Each designated, launched at the Krivak, the seeker goes active, the ship is hit |
| `flight-b52-tail-gun` (B52-6, 2026-11-15) | `--aircraft f22 --probe-enemy-aircraft b52 --probe-enemy-skill ace --probe-geometry rear --separation 1 --probe-guns --ai-probe-ticks 3000`: a scripted F-22 closes on two AI B-52s from behind | The debrief's `enemy_gun=hits/shots` shows the B-52's slewed M61 firing astern and reaching the F-22 (shots above zero, hits above zero) |
| `flight-tu26-tail-gun` (T26-6, 2026-11-16) | The same probe against an AI Tu-26 (`--probe-enemy-aircraft tu26`) | The Tu-26's slewed `T23_1.JT` fires astern at the F-22 (shots above zero, hits under 35 percent of them: 4 of 253) |
| `a2g-sarh` | Over `~QUCOL`: a MiG-23's AS-7 on a tank designated on the A-G page, then the same with the A-G page turned off two seconds after launch | On the track it kills its tank, never unguided; with the page off it loses its target for good (status Lost, unguided) and the tank is not hit |
| `a2g-sarh-uncued` | A MiG-23 AS-7 released at 0.5 seconds with no A-G track; the page opens at 3 seconds and designation follows | Initial targetless Search without permanent disablement, then Locked on later illumination; `--attack sarh-uncued` uses ordinary player controls |

## AI strike driver

Class-order probes accept `--order-class vehicles|tanks|structures|aaa|sams|ships`
and `--order-wingmen 1..9`. Order probes can use `--record PATH` without a GPU.
The `SELECTION` line identifies the actual ground target, side, class and weapon;
`WINGMAN` totals report each member's launches, hits and damage when the count is
explicit. These probes exercise the ordinary seat order path, including host
processing of client orders.

`--a2g-run THEATER STEM --pilot ai` (scenario file
[`tools/battery_scenarios/ai_strike.py`](../../tools/battery_scenarios/ai_strike.py), source
`crates/tore-app/src/a2g_run/ai.rs`) is the headless driver for the AI strike round (plan
slice ST-D). The player-side `--attack` kinds above fly the aircraft through the seat's
controls. This one lets the AI fly it, from the same mission step as the game, and only
watches. It exists so every behaviour slice of the round can test from the first day. Since
slice ST-T the driver orders the AI onto the target (an ordered ground target, which outranks any
briefing) and the AI flies its strike tactics ([AI strike](../spec/ai-strike.md)).

The mission is an open one (every plane on the AI), one friendly wing, no enemy. The striker
starts `--from NM` short of the target at `--altitude` ft and `--speed` kt on `--heading`
(and `--dive`), loaded as the player-side kinds load it (`--aircraft`, `--load`, `--station`,
`--count`). The target is the Nth ground target of the stem (`--target N`, or `--unit ID`),
with `--defenses AAA SAM` and `--surface-seed N` as before.

| Option | What it does |
| --- | --- |
| `--skill 0..3` | The striker's wing skill, Novice to Ace (Average by default) |
| `--profile straight\|popup\|fasthigh\|dive\|level\|orbit\|standoff\|laydown\|strafe` | **Forces** a pass, for coverage (`orbit` is the AC-130's, `standoff` a helicopter's stand-off attack, `laydown` a drag bomb's low level pass, `strafe` a gun run). The real AI chooses its pass from the perceived threat and its energy (John, 2026-10-11), not at random; the flag lets a test fly each pass once |
| `--handback S` | A human-flown run handed to the AI: a seat flies the striker on the scripted pass, designates the target on the A-G page (or Easy targeting's Ctrl+X for an aircraft with no ground page), selects the store, and gives the aircraft back `S` seconds into the pass. `--mode ccip\|lvl` sets the bomb sight in the human leg, `--trigger` holds the trigger there (pickle and hold) so a bomb leaves before the handoff, `--laser` turns the laser on by hand (Ctrl+L), and `--take-back S` has the seat take the aircraft again `S` seconds after the handoff and ends the run there. For the takeover tests |
| `--choose-store` | The AI chooses its own store instead of the fitted one (slice ST-L): a laser store only with a spot assured |
| `--buddy ID` | A second seat flies aircraft ID as a buddy (slice ST-L): a scripted pass 1 nm ahead of the striker, 3,000 ft right and 1,000 ft higher, which designates the nearest ground object with Ctrl+X and lases it by hand (Ctrl+L); the striker is ordered onto what it designated and chooses its own store. A striker with no designator drops on the buddy's spot (`spot_owner` on the RELEASE line). `--buddy` with `--buddy-ai` on the player-side `--attack laser` run is the other way round: the AI flies the buddy, is given the wing order's call and lases for the seat's bomb |
| `--order KIND` | Wing orders: a seat leads a two-ship wing on the pass, designates the target, and gives the wingman (the striker) the order `--order-at S` seconds in (5 by default). Kinds: `engage-my-target`, `engage-from-formation`, `attack-on-contact`, `protect-me`, `disengage`, `bug-out`, `land-selected`, `sort`, and the strike round's `engage-ir-laser` (Alt+F), `engage-class` (Alt+W) and `lase-my-target`. The order is the seat's own command: the world reads the ground target from the seat's designation and its targeting sight, and the wingman takes it or refuses. The driver does not give the wingman its target in an order run, the order does |
| `--quick-wing` | A Quick Mission's own strike wing: friendly wing 2 is two of `--aircraft`, nobody flies or orders it, and the mission's default role (bomber-class aircraft strike when a ground target is set) sends it. `ROLE` reports the role it was given |
| `--render OUT.png --render-at S` | One offscreen picture of the striker and its target `S` seconds into an AI run, the chase camera on the target line (slice ST-F) |
| `--offset FT` | The striker starts this far right of the pass line (negative: left), for the first-pass convergence runs |
| `--sead-at S` | A stand-in for a SEAD flight (slice H4): every hostile SAM and gun site but the target is destroyed `S` seconds into the run (`SEAD t= killed=N`), so a helicopter holding off can be seen to resume |
| `--sead-site ID` | With `--sead-at` or `--sead-after-shot`: the stand-in destroys only this site (repeatable, the id as the SITE lines name it), so a site the helicopter has not met yet stays in place, for laying out an ambush after a resume |
| `--sead-after-shot S` | The SEAD stand-in goes in by the shot, not the clock (2026-10-29): `S` seconds after the first round one of the `--sead-site` sites (any site when none is named) launches at the striker, they are destroyed (`SEAD t= killed=N`). For an ambusher whose second round, 10 s after its first, would hit as well. Not with `--sead-at` |
| `--place-site ID,BEARING,RANGE_FT` | Before the first tick the hostile SAM or gun site `ID` (as the SITE lines name it) moves to that compass bearing and range from the target: its surface unit, its arms, its combat row and hit box (the drawn scene keeps it where it was). Prints `PLACED`. Repeatable: each named site moves, in the order given. For laying out the threats a helicopter meets (slice HF1) |
| `--site-sight ID,FEET` | The hostile site `ID` is listed by an eye only from `FEET` of it (at least 256), in place of its type's 19,968 ft (`maxVisDist` 78 units of 256 ft). Prints `SIGHTLIMIT`. A site dug in and hidden well: the helicopter does not know it until it is near, and the site fires as always. Repeatable. Written for the layout of an ambush by a site the helicopter did not know (2026-10-29; why terrain cannot do it: [AI strike](../spec/ai-strike.md#helicopter-threat-awareness)); since the game hides shoulder-fired teams itself the scenarios no longer need it, and it stays as a tool to try a spotting distance or hide a vehicle site |
| `--terrain-map RADIUS_FT,STEP_FT` | Before the first tick prints the ground height on a grid round the target, north row first (`TERRAINMAP` and `TERRAIN` lines, whole feet; a step of at least 100 ft) |
| `--terrain-nodes RADIUS_FT` | Before the first tick prints the ground height at every node of the imported terrain lattice (8,192 ft apart; the ground between nodes is interpolated from them) within that radius of the target: a `TERRAINNODES` header with the first node's numbers and the grid size, then one `NODE` line per row from the south. For working out sight lines exactly |
| `--follow-egress` | After the last impact the run goes on, within `--seconds`, until the striker egresses, and settles from the EGRESS line, so its way out is seen (slice HF1) |
| `--follow-threat` | After the last impact the run goes on, within `--seconds`, while a helicopter keeps out of a ring it means to come back from (its latest THREAT is `threat_leave`, or it is holding off), so the resume is seen (slice H2); after a resume it goes on until the next release (slice HF1) |
| `--mission-altitude FT` | The Quick Mission altitude setting (5,000, 10,000, 20,000 or 40,000 ft), the airborne height a heavy bombs from |
| `--station N` one past the last | With `--load`, adds an internal bay there: a heavy with no ground station (the C-130) can carry bombs for the level run |
| `--render-scene DIR` | Draws one frame offscreen at 1080p (no window), from a chase camera off the striker's right wing, 0.4 s after its first release, as `STEM-PROFILE-SKILL.png`. A companion aircraft (friendly wing 2) sits on a second seat, far from the pass, to present the picture from |

Every line starts `ai-strike:` and is `key=value` words (the battery reads them with
`ai_strike.parse_runs`):

| Line | Meaning |
| --- | --- |
| `setup` | Aircraft, store, station, target (its id and `name`) and where it stands, hit points, skill, forced profile, handback, order, mode |
| `PROFILE name=P requested=R` | The first line echoes the request (`note=forced` or `note=chosen`); a later line names each pass the AI flies |
| `start` | The striker at tick 0: range to the target, height above the ground, speed |
| `DESIGNATE`, `HANDBACK`, `ORDER`, `ORDER-REPLY`, `ORDER-STATE` | The human leg: the target designated, what was set when the aircraft was handed back (designation, selected station, bomb sight mode, laser hand setting `hand-on`, `hand-off` or `auto`), the order given and the world's reply (given, refused or failed, with its words), and what the wingman holds after it (`strike_order`, the size of its `class_order`, `lase_order`) |
| `ROLE role=strike\|cap` | `--quick-wing`: the role the mission gave the wing |
| `TAKEOVER` | After a handback: the attack the AI continues (target, kept station or `ai`, bomb sight mode), or `none` |
| `KEPT` | While the AI flies a handed-back aircraft: the kept cockpit's mode, laser hand setting, whether it lases, and the selected station, each time one changes |
| `TAKEBACK` | A `--take-back`: what the human finds (designation, station, mode, laser hand setting, rounds left on the store) |
| `ACTIVITY name=...` | What the AI says it is doing, each time that changes |
| `PHASE name=...` | The striker's strike phase (Ingress, Attack, Lase, Exit, TurnBack, Wait, Defend, Home, HoldOff), each time it changes |
| `TARGET target=T phase=P` | The striker's strike target, each time it changes, with the phase it changed in: an early retarget (a target killed before the pass released, John, 2026-10-28) changes it inside `Attack` |
| `COMMIT open=yes\|no release_s=S held_defence=yes\|no held_at=T\|none` | The committed-to-release window (John, 2026-10-28) opened or closed: the estimated seconds to the release (`none` once the pass has ended or released, or while the cue is not closing on the aim) and, on closing, whether a missile or gunfire defence was pending while it was open (one the window held back) and when it was first seen (`held_at`, the run's seconds) |
| `THREAT decision=D target=T ring=R words="..."` | A helicopter's threat decision (slice H4): `threat_hold_off`, `threat_resume`, `threat_retarget`, `threat_home` or `threat_leave`, the target it concerns and the ring that blocked it (a defence's id, `0x0` for an ambush); the replay records each as an `ai.strike` event. Followed by a `RINGS` line |
| `RINGS n=N rings=id:radius:from_target,...` | The threat rings a helicopter knows (slice H4), with each `THREAT` line and each `PHASE` change of a helicopter: the defence's id, the ring's radius and its distance from the target, feet |
| `HOLDOFF` | Every ten seconds of a hold-off (slice H4): seconds held, distance from the hold point, height above the ground and range to the target |
| `NOLAZE decision=manual or rtb` | A striker with laser stores and no laser support decided (slice ST-G): `reason` (`no_laze_manual` or `no_laze_rtb`), the store, station and target, and the `words` on the radio. Once |
| `SITE` | Once, after `setup`: each standing hostile SAM or gun site of the mission (id, name, class `SAM` or `AAA`, hit points) and its horizontal distance from the target (`target_ft`) and from the start (`start_ft`), for reading a defended run (slice ST-H) |
| `BREAK` | While a helicopter flies its defence (slice ST-H), once a second: its heading and track, the threat's compass `bearing` (or `none`), the break's `offset` off that bearing (60, 90 or 120 degrees), the seconds of the next ten that terrain hides it from the site (`masked_s`) and that it could not climb clear of the ground (`blocked_s`), the flight path asked for (`pitch`), its height above the ground, its speed in knots and bank |
| `SHOT site=ID range_ft=R known=yes\|no` | A ground site launched a round (not a gun's) at the striker, `R` feet from it; `known` says whether the striker knew the site at that moment (the site's ring was in its ground picture, and not only because this launch gave a hidden shoulder-fired team away; `none` without a picture). An ambush in the sense of the threat rule is a shot from a site it did not know |
| `SEEN site=ID range_ft=R` | A hostile SAM or gun site entered the striker's ground picture for the first time (the first time its ring is known), `R` feet from it |
| `SIGHT site=ID range_ft=R clear=yes\|no` | The terrain line between the striker and a hostile site within 30,000 ft opened or closed (the line the eye uses: eight probes from the striker to the site), for reading why a site was or was not known when it fired |
| `PICTURE contacts=N` | The number of ground contacts the striker's picture holds, each time it changes (the 50 nearest the eye lists) |
| `SIGHTLIMIT id=ID sight_ft=F was_ft=W` | `--site-sight` set the site's listing distance |
| `HURT` | The striker lost hit points: the points left and its first, and the system faults the hits have caused (index and count) |
| `HOLD` | Every 5 s of a lase hold: range from the target, height above it, speed, bank |
| `LASER plane=P state=S mask=M` | The laser of the striker (and of the buddy), each time its state or mask changes (Off, Ready, Lasing, Masked: Range, Gimbal or Terrain) |
| `RELEASE` | A store left the striker: time, store, station, height, speed, range to the target, and the pass's number, its drawn aim offset from the target (`aim_ft`) and whether the draw lies inside the release circle (`inside=yes`) or in the ring R to 2R (`inside=no`), the lead laid on a moving target (`lead_ft`, 0 on a standing one) and the dive angle (`dive_deg`).; for a guided weapon, its mode and the owner of the spot it follows (`guided`, `spot_owner`). A gun burst is one `RELEASE` (its first round) with `burst=B rounds_left=N`, `aim_ft` being the burst's start offset (its rounds walk onto the target over the skill's time, slice ST-G) |
| `IMPACT miss_ft=F` | A released round ended; its horizontal distance from the target (the object the striker released at, where it stands when the round ends) in feet (the nearest crater or burst within 60 ft of its last place, else that place). For a gun burst, once its rounds have ended: the distance from the target to the middle of the rounds that reached the ground, with `rounds`, `ground`, `near` (rounds that ended within 50 ft of the target where it stood as each ended), `hits` (rounds that struck it, that is entered its volume) and `damage` (the hit points they took) |
| `BURST` | An AC-130 burst began (slice ST-F; gun rounds are not RELEASE lines): its number at the target, its walked-in aim offset (`aim_ft`) and whether the draw lies inside the circle, the orbit's horizontal radius, height above the target, slant range and bank, the speed and the gun rounds fired so far |
| `RENDER path=P` | `--render-scene` wrote its frame |
| `KILL` | A ground object the striker destroyed, and whether it was the target |
| `EGRESS` | After a release and once the rounds have landed, the striker is a nautical mile past its closest approach and opening |
| `LOST` | The striker was destroyed, crashed, ejected or left the AI (`crashed=yes` when its flight model says it hit the ground, which a shot-down aircraft's wreck also does) |
| `summary` | Counts of the above, the final activity, the target's hit points, the store's rounds left, the gun rounds and bursts (`gun_rounds`, `bursts`), and the seconds a laser weapon of the run flew guiding (`fall_s`) and how many of them the spot it followed was not lasing (`blind_s`), the lowest height above the ground it flew (`min_agl_ft`) and the seconds a helicopter flew its break (`break_s`); then `no release` if there was none, and `done` |

The run ends 20 s after the striker is lost or its last round has landed. The hooks the strike
slices fill are one `hooks` module in `ai.rs`: ST-T hands the AI its target (and the driver's
store) and forces the `--profile` pass and reports it, ST-K reads what a takeover kept, ST-O
adds order kinds.

| Scenario | What it flies | What must hold |
| --- | --- | --- |
| `ai-strike-driver` | An A-4E's Mk 82 on `~QUCOL` with the AI flying, at skills 2, 0 and 3, the first two with a forced profile | The report is consistent with itself (the summary counts the lines, a release comes before its impact and its egress), the setup and PROFILE lines echo the request. Passes before and after the strike behaviour |
| `ai-strike-handback` | A seat designates the column, selects the Mk 82 in LVL and holds the trigger; the bomb falls and is reported; the aircraft is handed to the AI at 30 s. A second run is handed back at 6 s with nothing released | HANDBACK shows LVL and the bomb's station, the human's release comes before the handoff and ends within 1,500 ft, the AI reports an activity after it |
| `ai-strike-takeover` | An F-16C's GBU-10 pass at the column from 8,000 ft: the seat designates, selects the bomb in LVL with the laser on by hand and hands back 3 s in, then takes the aircraft back 60 s after | TAKEOVER names the human's target, station and LVL; the AI flies a level run and releases; the bomb ends within 500 ft; every KEPT line keeps LVL, the hand setting and the station, and lases until the bomb ends; TAKEBACK finds LVL, the laser on by hand, the station and one bomb fewer |
| `ai-strike-order` | A seat leads a two-ship wing, designates the column, orders Engage my target | One ORDER line with a designation, and the world's reply (taken since slice ST-O) |
| `ai-wing-ground-order` | A seat leads a two-ship wing and orders the wingman onto the ground: Alt+E and Alt+R on the Ukraine column (an A-4E's Mk 82), Alt+F with the lead's Pave Spike on the factory `~QUFACT` (an F-16C's GBU-30), Alt+W with a SAM of the defended column designated (`--defenses 3 3`; the wingman's Mk 82 may be shot down, so it holds the class order but need not release), and Alt+E refused by an F-14 wingman with only air-to-air stores | Each takes or refuses as it should: the first four are "1 applied" and leave the wingman holding the object (a strike order) or its class (Alt+W), and the wingman releases (not required for the class order, which flies into the defences); the last is refused ("no usable surface store"), holds nothing and releases nothing |
| `ai-wing-lase` | The lead of two F-16Cs designates the factory and orders Lase my target, 60 s | "1 applied", the wingman holds the lase order (and the duty's strike order on the same object), enters the lase hold within 2 s (PHASE Lase), its laser comes up Lasing and is still lasing at the end, at least two HOLD lines between 2 nm and the laser's reach, no release |
| `qmc-strike-wing` | A Quick Mission with the factory `~QUFACT` as the ground target and a second friendly wing (not the column: its guns shoot the wing down on the way in): two A-4E (bomber class), then two F-16C (fighters) | The A-4E wing is given the Strike role by the mission's default and releases on the target, and its bomb ends within the skill's release limit of the target the wing struck (the wing picks its own, so the miss is measured to the object the striker released at, not to the driver's; before nr-cue it read 1,830 ft off the driver's target); the F-16C wing keeps CAP and releases nothing |
| `ai-strike-bombs` | An A-4E's Mk 82 on the Ukraine column `~QUCOL`: straight, pop-up, fast-high, dive and level forced once at skill 2, and skills 0, 1 and 3 on the dive | Each run names its pass and releases with its drawn aim inside the skill's release circle (or in the ring R to 2R when the draw says so), ends the bomb within the circle plus the release gate plus 400 ft of scatter (707, 617, 526 and 443 ft for skills 0 to 3; the ring's edge plus them, 986, 813, 641 and 476 ft, for a ring draw, see [AI strike](../spec/ai-strike.md#skill-and-release-precision)), shows an EGRESS and is not lost. Not the TVIET truck park: its radar-laid guns (26,200 and 44,250 ft reach) shoot an A-4E down at 5,000 ft within 25 s whatever pass it flies |
| `ai-strike-rockets` | An A-4E's LAU-61 on `~QUCOL`, straight runs at skills 1 and 2 (an Ace rarely meets its 10 m circle on a short straight run) | A rocket burst with the aim inside the circle, each rocket within the circle plus the gate plus 400 ft, an EGRESS, not lost |
| `ai-strike-maverick`, `ai-strike-jdam`, `ai-sead-harm` | An A-10's AGM-65G on `~QUCOL` (fast-high), an F-16C's GBU-30 on the factory `~QUFACT` (level; a JDAM flies to the point stored at release, so a moving column would be missed), an F-4G's AGM-88 on the `~QVRDRI` radar (fast-high) | The named pass, a release and an impact, the target hurt, the striker not lost |
| `ai-strike-heli` | On `~QUCOL` from 4 nm at 1,500 ft and 120 kt: a Mi-24's AT-2 (its own choice, the pop-up), an AH-64's LAU-61 at skills 2 and 3, followed out with `--follow-egress` for up to 300 s (slice HF1) | The named pass, a release and an impact; the AT-2 hurts the tank; the rockets' drawn aim lies in the circle (or the ring) and each rocket ends within the circle plus the gate plus 400 ft; not lost. The AH-64 runs must EGRESS (they turn away from the column and mask low, about 47 s after the release) and live through the 20 s after it; before slice HF1 the run ended 20 s after the impact and the AH-64, flying on over the column, was shot down 2 s later. The stand-off attack is no longer forced on a quiet column (slice ST-H): the AI chooses it under SAMs itself, in `ai-strike-heli-defended` |
| `ai-strike-heli-defended` | A Mi-24's AT-2 against defences (slices ST-H and H4, John's threat rule of 2026-10-12), 4 nm out at 1,500 ft unless named: heavy AAA on the column (skill 1); light AAA at the factory `~QUFACT` (skill 1) with its ZSU-23 moved 4,500 ft north of the factory (`--place-site 0x5000000c,0,4500`, 2026-10-29); light SAMs of seed 19 (a SAM that out-ranges the AT-2); light SAMs of seed 4 (an SA-14 team on the target, up to 400 s); the same with `--sead-at 60`; two ambushes on the factory `~QUFACT` (`--defenses 0 1`, up to 400 s with `--follow-threat`; slice HF1, a new layout on 2026-10-29): a lone SA-7 team (surface seed 7, `0x5000000f`, skill 2) and a lone SA-14 team (seed 8, `0x5000000e`, skill 3), each moved to bearing 190 and 7,000 ft from the factory (`--place-site`), hidden by the game's rule for shoulder-fired teams (since 2026-10-29 the helicopter spots one only within 3,500 ft at skill 2 and 4,000 ft at skill 3, or once it launches; before, the runs used `--site-sight 3000`) and destroyed 4 s after its first launch (`--sead-site`, `--sead-after-shot 4`); from 4 nm at 200 ft on seed 1 (the layout where the AI would choose the stand-off); light SAMs of seed 16 (`--defenses 0 1 --surface-seed 16`, from `ai-strike-heli` since 2026-10-29: it starts 12,100 ft from an SA-14 team, inside its ring) | Heavy AAA: an `AHEAD` line, a release and an impact, the tank hurt, no THREAT decision, and the summary's `reach_margin_ft` 0 or more (never inside the reach of a gun it knew). Since the [move-ahead ambush](../spec/ai-strike.md#move-ahead-ambush) of 2026-10-29 it plans on where the tank will be: from 1 s it sets up west of the road, the column turns south-east at about 62 s and spoils that plan, and it plans again and fires at 86 s from 13,900 ft; the AT-2 takes 120 of the T-90's 200 hit points and the exit turn comes about 960 ft into a ZSU-23 ring's 2,000 ft margin. Before, each stand-off planned on the tank's place was spoiled within seconds as the column drove on among the standing guns (every gun of `--defenses` stands), and at 113 s it held off without firing. Factory: the stand-off and a kill (it sets out on the pop-up and gives it up for the stand-off once it sees the ZSU-23 near the factory, 20 s in; at the gun's own 2,650 ft no firing point leaves room for the advance, hence the move). Seeds 19 and 1 (seed 1 names no pass since 0.2.2: it holds off from the start): THREAT `threat_hold_off` then `threat_home`, no release. Seed 4: THREAT `threat_hold_off`, `threat_leave`, `threat_resume`, `threat_hold_off`, `threat_leave` and `threat_home` in that order, an `AHEAD` line, each leave after a SHOT with `known=no`, and no release. Until 2026-10-29 it held off the SA-14 it knew and went home; since the game hides shoulder-fired teams, the team on the target launches at 29 s (`known=no`), the helicopter holds off its ring and breaks the round, the column drives out from under the ring and at 198 s a move-ahead shot stands clear of it, and the seed's other hidden SA-14 team launches at 204 s (`known=no`); it holds off again and goes home at 385 s. Seed 16: a SHOT with `known=no` (20 s in), THREAT `threat_leave`, at least 2 BREAK lines and no release; until 2026-10-29 it knew the SA-14 from the start and flew a stand-off from outside its ring that hurt the tank, but the game now hides the team, whose launch gives it away (the second round, 10 s later, is `known=yes`), and two hits send it home. SEAD: `threat_hold_off`, the SEAD line, `threat_resume`, a release and the tank hurt. Ambush runs: a SHOT with `known=no` before the `threat_leave` (since 2026-10-29 `threat_leave` is the answer to a shot from a site it did not know, and a leave after only `known=yes` or `none` shots fails in any run), then THREAT `threat_leave` (ring 0, the ambush) and `threat_resume` in that order, at least 2 BREAK lines, a release and the factory killed by the shot after the resume. The SA-7 launches at 32 s from 10,900 ft (the SA-14 at 20 s from 13,400 ft), the helicopter breaks, takes the first round (78 of 103 hit points) and flies on; the second round, 10 s after the first, would hit too and send it home, which the stand-in prevents. The resume comes at about 119 s (101 s) and the kill at about 226 s (223 s). The layout does not hang on the skill, the start or the seed; the site can move across a wide area and the spotting distance anywhere up to 11,000 ft (the spec, "The ambush layout of 2026-10-29", has the margins). Every BREAK at 60, 90 or 120 degrees off the threat, 75 knots or more, no steeper than 10 degrees, never blocked by the ground; the helicopter never lost and never lower than 150 ft above the ground. The switch to another target after the wait is covered by the unit tests: no driver layout has a second target outside every ring |
| `qmc-strike-heli` | A Quick Mission with the factory as the ground target and a second friendly wing of two AI helicopters, a Mi-24 pair then an AH-64 pair (slice ST-H) | ROLE strike from the bomber-class default, a start 1,500 ft over the ground, a release (the AT-2, then a rocket burst). The factory's defences may bring them down, so a loss is allowed |
| `ai-strike-ac130` | An AC-130 from 3 nm at 6,000 ft on `~QUCOL` for 240 s at skills 0, 2 and 3 (the TVIET truck park's radar-laid guns shoot it down on the way in) | PROFILE orbit; BURST lines whose walked-in aim lies inside the circle (the ring for a ring draw); after five bursts the slant range inside 8,000 to 13,000 ft and the bank at most 30 degrees; gun rounds fired; the tank killed; not lost |
| `ai-strike-laser-self` | An F-16C's Pave Spike and a GBU-10 on `~QUCOL` from 15,000 ft at 480 kt, and an Su-25's built-in designator and a GBU-10 from 8,000 ft at 400 kt, the AI flying both (slice ST-L) | The F-16C goes from the pass to the hold at the release (a PHASE Lase line within a second of it), flies an offset turn (a bank over 10 degrees) inside the laser's 10 nm reach, holds the spot on the tank for the whole 50 s fall (no LASER line leaves Lasing, `blind_s` at most 1) and exits only after the bomb has landed, within 150 ft of the tank, which is destroyed; the laser is not asked to be off afterwards (the second GBU-10 is still armed, so the automatic rule keeps it on). The Su-25's cone holds its nose on the tank and may leave inside 2,000 ft, the bomb following the spot's last point; it also destroys the tank |
| `ai-strike-laser-buddy` | Three runs (slice ST-L). A player's F/A-18D drops a GBU-10 with an AI F-16C as the buddy, given the wing order's call; an AI F-14 with a GBU-10 and no designator, with a player's F-16C lasing as the buddy and the store left to the AI; the same F-14 with nobody lasing | The AI lases, the sight at the release says IN RNG on a buddy's spot with no laser of its own, the bomb guides on aircraft 1's spot and destroys the tank, and the target page would name the lasing pilot `Friendly 2-1`. The F-14 waits at the choice, chooses the bomb once the player's spot is up, never lases or holds, drops on that spot and destroys the tank, the spot never lost. With nobody lasing it says "No laze support, dropping manual" (one NOLAZE line) and drops the bomb by hand as an unguided bomb (slice ST-G), released `Waiting` with no spot owner and within the circle limits of the target |
| `ai-strike-no-laze` | An AI F-14 with two GBU-10 and nobody lasing, at skills 0, 1 and 2, then with two AS-14 laser missiles (slice ST-G) | The bombs: one NOLAZE line `decision=manual reason=no_laze_manual` saying "No laze support, dropping manual", a bomb pass, a RELEASE `Waiting` with no spot owner and a skill aim and lead, an impact within the circle limits plus 150 ft, no lase hold. The missiles: `decision=rtb reason=no_laze_rtb` saying "No laze support, RTB", no pass, no release, PHASE Home |
| `ai-strike-commit-aaa` | The committed-to-release window (John, 2026-10-28; [AI strike](../spec/ai-strike.md), "Committed to the release, 5 s"). An A-4E's forced straight Mk 82 pass on `~QUCOL` from the default 4 nm, 5,000 ft and 450 kt, with `--defenses 2 0 --surface-seed 1`, at skills 2, 0 and 3 for 35 s: the window opens at about t=23.2, a gun's rounds reach the striker at about t=26.6 and the bomb leaves at about t=27.8. The control flies the skill 2 pass for 30 s with the column's nearest ZSU-23 moved 14,000 ft south under the run line (`--place-site 0x5000000a,180,14000`): it fires at about t=21.7, before the window would open | Held runs: a COMMIT line opens with `release_s` 0 to 5; the closing line says `held_defence=yes` with `held_at` inside the window; no PHASE Defend between the opening and the release; a RELEASE inside the window; PHASE Defend within 0.5 s after it. Control: the first PHASE Defend comes in Attack, with no window opened before it and nothing released, so the run breaks as it did before the change. The runs are deterministic (the same lines run after run); surface seeds 3 and 4 put the gun's first rounds just after the release, which is why the seed is fixed |
| `ai-strike-defended` | An F/A-18D's Mk 82 on `~QUCOL` with `--defenses 2 2`, its pass chosen by threat (a pop-up, SAMs cover the column) | Releases inside its circle and defends (an ACTIVITY Defending line). The column's guns can bring it down at the pop-up's low altitude, so a loss is allowed |
| `ai-strike-dive-steep` | An A-4E's Mk 82 dive on `~QUCOL` at every skill (slice ST-T2) | Released in a dive of at least 45 degrees (the inverted roll-in), above the 3,000 ft floor, with a lead on the moving column, within the circle limits |
| `ai-strike-strafe` | An A-10's GAU-8 gun run on the column's lighter vehicle (`--unit 0x50000003`, 150 hit points) at every skill (slice ST-G: the rounds walk onto the target) | At least three bursts, each burst's start offset inside the circle (or the ring), the burst's middle within the circle limits with 150 ft of gun spread in place of the bomb scatter, released under 4,000 ft, and the vehicle hit and hurt (the IMPACT lines' `hits` and `damage` above 0, its hit points down) at every skill. No egress required (the run ends near the target); a loss is allowed |
| `ai-strike-rockets-popup`, `ai-strike-rockets-dive` | An A-4E's LAU-61 pop-up and dive at skill 2 on `~QUCOL` | Rockets released, within the circle limits with 250 ft of rocket spread |
| `ai-strike-snakeye` | An A-4E's Mk 82 Snakeye laydown at skills 1 and 2 | Released under 1,000 ft with a lead on the column, within the circle limits |
| `ai-strike-heavy-level` | A C-130 with an added bay of eight Mk 82 on `~QUFACT`, `--mission-altitude 10000`, starting at 7,000 ft 8 nm out | Released at 9,500 ft or more (the floor is the mission height, not the 5,000 ft base), no lead on the standing factory |
| `ai-strike-f117` (slice F117-9, 2026-11-15) | An F-117A on a GBU-27 from the left bay station against the Ukraine armored column (`--aircraft f117 --load PAVEWA3 --station 0`), then as a Quick Mission wingman on the factory (`--quick-wing`) | PROFILE level only (no dive, no pop-up), released at 4,500 to 5,500 ft, the bomb guiding on the F-117's own IRADS spot, 150 ft or less from the tank and the tank destroyed (the first run), the wingman given the Strike role (the second), EGRESS, not lost |
| `ai-strike-heavy-level` update (B2-9, 2026-11-15) | The same run | Its eight bombs now leave as a stick (a quarter second apart), not one a pass |
| `ai-strike-b2-stick` (B2-9) | An AI B-2 (Heavy) on `~QUCOL` at 20,000 ft, 12 nm out, 32 Mk 82 in the left bay | Eight releases 0.25 s apart from the grid's first eight places (the new `ai-strike: ORIGIN` lines: right, up and forward of the aircraft), all inside the left opening, no two from one place, release above the 15,000 ft threat floor, an impact, egress, not lost |
| `ai-strike-b2-climb` (B2-9) | The same B-2 starting at 5,000 ft 30 nm out | Releases at 9,000 ft or more, never below 4,000 ft above the ground, an impact, egress, not lost |
| `ai-strike-b2-paveway` (B2-9) | The B-2's Paveway III row | The release leaves the left bay's first Paveway place (-7.28, -1.79, 6.67), kills the tank, egress |
| `ai-strike-b52-stick` (B52-9, 2026-11-16) | An AI B-52 (Heavy) on `~QUCOL` at 20,000 ft, 12 nm out, nine Mk 82 in the forward bay row (`--station 2 --count 9`) | The doors open before the first bomb (a `BAY` line), eight releases 0.25 s apart each with `bay=1.00`, from the row's first eight places (4 abreast, outermost first, bottom tier first: right -1.95, 1.95, -0.65, 0.65 ft, up -5.29 then -3.98, forward 13.87), the doors read shut 1.5 to 6 s after the last, an impact, egress, not lost |
| `ai-strike-b52-pylon` (B52-9) | The same B-52 with six Mk 82 on the wing pylons (`--station 1 --count 6`) | Six releases 0.25 s apart from the two beams, left and right alternating (right -15.96, 15.96, -18.70, 18.70, -17.33, 17.33 ft, up 2.4, forward 10.33), every `bay=0.00` and no `BAY` line, an impact, egress, not lost |
| `ai-strike-b52-quick-wing` (B52-9) | A Quick Mission with the Ukraine factory as the ground target and a B-52 wing (`--quick-wing --station 2 --count 9`) | `ROLE strike`, the same bay stick of eight with the doors open for it at the 5,000 ft heavy floor, egress, not lost |
| `ai-strike-b52-antiship` (B52-9) | An AI B-52 with two AGM-84E on the pylons (`--station 1 --count 2`), 15 nm from the Ukraine surface fleet's Krivak at 20,000 ft, 60 s | One Harpoon leaves each wing (right -17.33 and 17.33 ft, up 2.3, forward 10.33), the doors never move, both hit the Krivak and it is destroyed. The run stops at 60 s: after the kill the AI searches on and is lost at about 137 s within 8,000 ft of the fleet (cause not traced) |
| `a2g-ripple-taps` (BMB-AI2) | The human trigger on the B-52's forward bay row (`--a2g-run --attack release`, 8 one-tick presses of a Mk 82 row at 0.2, 0.1 and 0.05 s apart, faster than the doors open) | Eight releases each (4, 1 and 1 before): a press made while the doors open is held and fires when they are open |

| `ai-strike-b1b-mid` (B1B-9, 2026-11-16) | An AI B-1B (Heavy) on `~QUCOL` at 20,000 ft, 12 nm out, 20 Mk 82 in the intermediate bay (`--station 1 --count 20`) | The doors open before the first bomb (a `BAY` line), eight releases 0.25 s apart each with `bay=1.00`, from that bay's first eight places (4 abreast, outermost first, bottom tier first: right -1.95, 1.95, -0.65, 0.65 ft, up -0.62 then 0.68, forward 19.97), every release from station 1, the doors read shut 1.9 s after the last, an impact, egress, not lost, the level profile |
| `ai-strike-b1b-aft` (B1B-9) | The same B-1B with 20 Mk 82 in the aft bay (`--station 2 --count 20`) | The same stick of eight from the aft bay's grid (forward -13.03 ft), station 2, doors open for it and shut after, an impact, egress, not lost |
| `ai-strike-b1b-quick-wing` (B1B-9) | A Quick Mission with the Ukraine factory as the ground target and a B-1B wing (`--quick-wing --count 20`) | `ROLE strike`, the forward bay's stick of eight with the doors open for it at 5,079 ft (the 5,000 ft heavy floor), egress, not lost, the level profile (the Heavy class the rule gives a 477,000 lb record) |
| `ai-strike-tu160-stick-12nm` (T160-9, 2026-11-16) | The 12 nm start T160-8 moved to 10 nm: an AI Tu-160 on `~QUCOL` at 20,000 ft, 12 nm out, six Mk 82 in the forward bay row | The same checks as `ai-strike-tu160-stick`: six releases from the grid's places with the doors open, impacts, egress. The doors were never the cause of the lost pass: the column's corner and the heavy cue pointing (gain 0.4, damping 3 s) were |
| `ai-strike-tu160-stick-12nm-factory` (T160-9) | The same 12 nm pass over the standing Ukraine factory (`QUFACT`) | The same checks: doors open 0.5 s before the first release, six releases, egress |
| `ai-strike-tu160-aft` (T160-9) | Six Mk 82 in the aft bay row (`--station 1 --count 6`), 10 nm | Six releases from the aft row's places (right -1.95, 1.95, -0.65, 0.65, then -1.95, 1.95; up 0.72; forward -5.60 then -13.15), doors open, impacts, egress |
| `ai-strike-tu160-quick-wing` (T160-9) | A Quick Mission with the Ukraine factory as the target and a Tu-160 wing (`--quick-wing`), 400 s | `ROLE strike`, the forward row's six Mk 82 on the third attack (324 s) at the 5,000 ft heavy floor, doors open, egress, not lost |
| `ai-strike-tu160-as16-tank` (T160-9) | An AI Tu-160 with its AS-16 (`--load AS16`), 10 nm from the column's T-90 at 20,000 ft | One AS-16 from the forward row's first place (right -1.59, forward 38.61), released at 20,000 ft or more of slant range, impact, T-90 killed. Before T160-9 the AI never fired it |
| `ai-strike-tu160-antiship` (T160-9) | The same AS-16 against the Krivak of the Ukraine surface fleet (`QUSFLT --unit`), 15 nm, 115 s | One AS-16 leaves the forward row at 27,500 ft, hits the Krivak (47 ft, 150 of 500 left); the run stops before the fleet shoots the Tu-160 down at 119 s |
| `ai-strike-f111-laser` (F111-9, 2026-11-16) | An AI F-111F (Attack class) on `~QUCOL` at 5,000 ft with the GBU-28 of the inner wing pylon (`--station 0`) | Level pass, the F-111F lases (its Pave Tack and fixed laser), one release from the held wing's cell (-8.41, -0.50, 2.22 ft: the human's own, `a2g-release-origin-f111`), `guided=Guiding` on its own spot, the tank killed, `bay=0.00` and no `BAY` line, egress, not lost |
| `ai-strike-f111-pylon` (F111-9) | Three runs, a Mk 82 from rows 6, 7 and 8 (`--station 1`, `2`, `3`, `--count 2`) | One release a pass from (-11.71, -0.35, 1.11), (-15.28, -0.35, -0.08) and (-18.30, -0.35, -1.09), the doors never move, an impact, egress, not lost |
| `ai-strike-f111-bay` (F111-9) | Two Mk 82 in the bay (`--station 4`; the Pave Tack gives way) | The doors open before the release (`BAY` open, 0.95 or more), one release from the shared grid's first place (-0.65, -2.59, 16.54), the doors read shut 1.5 to 6 s after, an impact, egress, not lost |
| `ai-strike-f111-maverick` (F111-9) | The default load's four AGM-65G on the outer pylon pair (`--station 2 --count 4`) | One AGM-65G from (-16.13, -0.40, -0.08) on the fast high pass, the tank killed, the doors never move, not lost |
| `ai-strike-f111-antiship` (F111-9) | Two AGM-84E on the outer pair (`--station 2 --count 2`), 15 nm from the Ukraine surface fleet's Krivak at 20,000 ft, 60 s | One Harpoon a wing (right -15.28 and 15.28 ft, forward -0.08), the doors never move, both hit and the Krivak is destroyed |
| `ai-strike-f111-quick-wing` (F111-9) | A Quick Mission with the Ukraine column as the ground target and an F-111F wing (`--quick-wing`) | `ROLE strike`, the six retarded Mk 82 of station 1 in the laydown, six impacts, the target killed, egress, not lost. The factory is not used: the wingman is lost to its own bombs over a rise on the way (docs/formats/aircraft-variety.md, F111-9) |
| `ai-strike-tu26-pylon` (T26-9, 2026-11-16) | An AI Tu-26 on `~QUCOL` at 20,000 ft, 12 nm out, six Mk 82 on the inner glove row (`--station 2 --count 6`) | Six releases 0.25 s apart from the two gloves alternating from the left (right -11.33, 11.33, -12.02, 12.02, -10.64, 10.64 ft, up -4.69 to -4.12, forward 4.33), the doors never move, six impacts, egress, not lost |
| `ai-strike-tu26-quick-wing` (T26-9) | A Quick Mission with the Ukraine factory as the ground target and a Tu-26 wing (`--quick-wing --station 4 --count 8`) | `ROLE strike`, the bay stick of eight (`ai-strike-tu26-stick`'s cells) at 4,949 ft with the belly doors open before and shut after, the factory killed, egress, not lost |
| `ai-strike-tu26-antiship` (T26-9) | An AI Tu-26 with two AGM-84E on the inner glove row (`--station 2 --count 2`), 15 nm from the Ukraine surface fleet's Krivak at 20,000 ft, 60 s | One Harpoon leaves each glove (right -11.33 and 11.33 ft, up -4.2, forward 4.33), the doors never move, both hit and the Krivak is destroyed (29 s). The Tu-26's own AS-16 is not flown by the AI at a ship (the T160-9 dependency, [AI strike spec](../spec/ai-strike.md)) |
| `ai-strike-tu95-stick` (T95-9, 2026-11-16) | An AI Tu-95 (Heavy) on `~QUCOL` at 20,000 ft, 12 nm out, six Mk 82 in the belly bay (`--station 1 --count 6`) | The doors open before the first bomb (a `BAY` line), six releases 0.25 s apart each with `bay=1.00`, from the row's six places (right -1.95, 1.95, -0.65, 0.65 ft at 16.19 ft forward, then -1.95 and 1.95 ft at 8.64 ft; up -2.98), station 1, the doors read shut 1.5 to 6 s after the last, an impact, egress, not lost |
| `ai-strike-tu95-quick-wing` (T95-9) | A Quick Mission with the Ukraine factory as the ground target and a Tu-95 wing (`--quick-wing --station 1 --count 6`) | `ROLE strike`, the same bay stick of six with the doors open for it at the 5,000 ft heavy floor, the factory killed, egress, not lost |
| `ai-strike-tu95-antiship`, `ai-strike-tu95-antiship-beams` (T95-9, rewritten in round 7, 2026-11-16) | An AI Tu-95 with two AS-16 on the bay row (`--station 1`) or the wing beams (`--station 2`), 15 nm from the Ukraine surface fleet's Krivak at 20,000 ft, 135 s | The radar missile is selected (`SELECTION kind=RadarAgm`, the station asked for), one AS-16 leaves the row's cell from about 27,500 ft at about 108 s (the bay doors open for it on the belly bay, no door moving on the beams), hits the Krivak (150 of 500 left) and the Tu-95 is not lost. T95-9 pinned `no release` in 90 s; T160-9's shared fire range lifted that |
| `ai-strike-tu26-as16` (round 7, 2026-11-16) | An AI Tu-26 with only the AS-16 loaded (`--load AS16`, its intake pair, station 1), 15 nm from the Krivak at 20,000 ft, 125 s | One AS-16 leaves the intake row's first cell (-6.33, -4.91, 9.67 ft) from 27,480 ft at 102 s with no door moving, hits the Krivak at 118.6 s (150 of 500 left), the Tu-26 is not lost |
| `ai-strike-su34-bombs` (SU34-9, 2026-11-16) | An AI Su-34 (Fighter, one bomb a pass) on `~QUCOL` with a Mk 82 from row 6 (`--station 1`) then row 7 (`--station 2`), two runs | The level pass at 4,500 to 5,500 ft, one release from the named station leaving the cell a human's first press leaves from ((-4.50, -3.46, 6.15) and (-18.00, -1.78, -3.40) ft), no bay door moved, an impact inside the skill 1 limit, egress, not lost |
| `ai-strike-su34-default` (SU34-9) | An AI Su-34 on its default load picks its own store (`--station 1 --choose-store`) | A laser store (the AS-14, `guided=Guiding` on its own spot), landed within 150 ft of the tank, the tank hurt, egress, not lost |
| `ai-strike-su34-quick-wing` (SU34-9) | A Quick Mission with the Ukraine factory as the ground target and a Su-34 wing (`--station 1 --quick-wing`) | `ROLE strike` from the record's bomber class word, an AS-14 from station 1 guiding on the wing's own spot, landed within 150 ft, something destroyed, egress, not lost |
| `ai-strike-su34-antiship` (SU34-9) | An AI Su-34 with two AGM-84E on its wing row (`--station 2 --count 2`), 15 nm from the Ukraine surface fleet's Krivak, 40 s | One Harpoon leaves each wing ((-18.00, -1.89, -3.40) and the mirror), both hit and the Krivak is destroyed, no bay door moves, not lost inside the window. The jet is shot down at about 52 s if the run goes on (the lone driver-flown wing flies on in formation: open) |
| `a2g-release-origin-b2` (B2-6) | The human trigger on the B-2's four bay rows (`--a2g-run --attack release`, 12 presses a Mk 82 row, 4 a Paveway row) | Each release leaves where it hangs: outermost first, alternating sides, front row before back, bottom tier first |
| `ai-strike-tu160-stick` (T160-6, 2026-11-16) | An AI Tu-160 (Heavy) on `~QUCOL` at 20,000 ft, 12 nm out, the forward bay's six Mk 82 (`--aircraft tu160 --load MK82`) | Six releases from the grid's own places (four abreast at 45.73 then 38.18 ft ahead, -1.95 ft up), doors open for the release, an impact, egress, not lost; the cells equal the human's in `a2g-release-origin-tu160` |
| `a2g-release-origin-tu160` (T160-6, 2026-11-16) | The human trigger on the Tu-160's bays (`--a2g-run --attack release`, 6 AS-16 a bay, 6 Mk 82 of the forward bay) | Each release leaves where it hangs: AS-16 three abreast (-1.59, +1.59, 0), forward row then back (38.61 and 22.66 ft forward, aft bay -12.73 and -28.68); Mk 82 four abreast |
| `a2g-antiship-tu160` (T160-6, 2026-11-16) | Over `~QUSFLT`: an AS-16 from the forward bay (`--station 0 --count 6 --from 6`) at the Krivak designated on the A-G page through the Tu-160's radar | Launched CUED at the Krivak, midcourse then Pitbull, the ship hit (150 of 500 left) |
| `flight-countermeasures-tu160` (T160-6, 2026-11-16) | `--live-fire --aircraft tu160 --jammer-on`, five chaff and three flares | The record's 99 chaff and 99 flares, 94 and 96 left |
| `a2g-release-origin-tu26` (T26-6, 2026-11-16) | The human trigger on the Tu-26's AS-16 rows (intake pair, both glove pairs, the bay, `--a2g-run --attack release`) and the bay loaded with eight Mk 82 | Each release leaves where it hangs: left then right off the rails and pylons; the bay's AS-16 three abreast then the second tier, its Mk 82 four abreast in two rows (the AI's cells, `ai-strike-tu26-stick`) |
| `ai-strike-tu26-stick` (T26-6) | An AI Tu-26 (Heavy) on `~QUCOL` at 20,000 ft, 12 nm out, eight Mk 82 loaded into the bay (`--station 4`) | Eight releases 0.25 s apart from the grid's own eight places, inside the bay opening, the doors open at every release, an impact, egress, not lost |
| `a2g-laser-tu26` (T26-6) | A GBU-16 from the Tu-26's inner glove pair (`--station 2`) at 15,000 ft on the spot of its built-in `SU37L.SEE` designator | Lased, released guiding on its own spot, the tank destroyed |
| `flight-livefire-tu26-slot1` to `slot5`, `flight-jettison-tu26-slot2` to `slot4`, `flight-countermeasures-tu26` (T26-6) | `--live-fire --aircraft tu26`, `STATIONS["tu26"] = [2000, 2, 2, 2, 0]`; slots 2 to 4 are in `REFUSED_SLOTS` (a surface missile against the practice aircraft) | The gun fires astern, the AS-16 slots fire nothing and keep their rounds, the three AS-16 rows jettison, the bay slot stays empty; 99 chaff and 99 flares, five and three released |
| `a2g-laser-b2` (B2-6) | A GBU-27 from the left bay at 20,000 ft on the B-2's own laser spot | Guides to within a second of the ground, kills the tank |
| `flight-livefire-b2-slot1` to `slot4`, `flight-countermeasures-b2` (B2-6) | `--live-fire --aircraft b2`, `STATIONS["b2"] = [32, 32, 0, 0]` | One Mk 82 a slot (the empty Paveway slots fall to the sister bay), 30 chaff and 30 flares |
| `ai-strike-offset` | Level and dive Mk 82 passes at skill 2 starting 1,000 and 3,000 ft off the attack line (`--offset`) | The first release on pass 1 (the sideways law converges on the first pass) |
| `ai-strike-lead-dive`, `ai-strike-lead-level` | John's comparison: the same A-4E Mk 82 dive (level) on the moving column `~QUCOL` and the standing factory `~QUFACT` at every skill | A lead on the moving target and none on the standing one, each within the circle limits; the miss and kill of each run read side by side |

The scenarios' parsing and checks have unit tests in `tools/test_battery_ai_strike.py`, and the
battery selection family `ai-strike` (`ai-strike-*`, `ai-sead-*`, `ai-wing-*`, `qmc-strike-*`) is chosen by changes to
`ai.rs`, `guided.rs`, the scenario file, the AI sources (`tore-sim/src/ai`, `tore-world/src/ai_wings`)
and the laser and sight step (`combat/laser.rs`, `combat/live/sighting.rs`).

## Rotor probe

`--rotor-probe` (slice H1) flies each imported powered-lift PT through scripted
requests to the AI's [rotorcraft and jet-lift control](../spec/ai.md#rotorcraft-and-jet-lift-control)
over flat ground at sea level, clean, at the Damper level, and prints one `ROTOR`
line per flight. The flights are `tore_sim::ai::rotor_probe`'s, the same ones the
unit tests in `flight/powered/ai_lift_tests.rs` assert on the synthetic PT fixtures.
The battery scenario `flight-powered-ai-probe` checks the lines against the unit
tests' bands.

| Line | Flight | Band |
| --- | --- | --- |
| `hold calm`, `hold wind15` | A hover held over a point at 200 ft for 60 s after 30 s to settle, calm and in a 15 kt wind across the heading | within 10 ft and 10 ft of height |
| `move500` | The point moved 500 ft diagonally | overshoot under 10 ft, arrives within 10 ft |
| `climb500` | A vertical climb from 20 to 500 ft at 25 ft/s | fastest 23 to 27 ft/s, top under 515 ft, ends within 10 ft |
| `forward <kt> <fpm>` | 40, 80 and 120 kt climbing and descending 1,000 ft/min | climb within 150 ft/min, speed within 5 kt, heading within 3 degrees |
| `descent_turn` | Four seconds banked left onto a slot, then a right turn onto heading 80 slowing from 130 to 60 kt while asked 12 degrees down (about 42 ft/s), 40 s from 3,000 ft (a wingman's formation chase, slice CH47) | the helicopters never fly the collective under 0.1 and the nose stays within 30 degrees of the track (the V-22, with its power lever, is not held to it) |
| `turn200` | 180 degrees at 200 ft asking 160 kt, more than these aircraft make | loses under 50 ft, turns round within 5 degrees in 60 s |
| `slow120` | From 120 kt at 1,000 ft to a hover over a point 3,000 ft ahead | loses under 50 ft, ends within 10 ft hovering |
| `convert` (V-22) | From a hover to airplane mode asking 220 kt, then a hover 15,000 ft ahead | nacelles down and over 200 kt, back over 80 degrees within 10 ft, within 300 ft of height, no corridor override |
| `jet hover` (AV-8, Yak-141) | A hover at 200 ft, then a vertical landing at a 5 ft/s sink | within 10 ft, touchdown under 8 ft/s within 15 ft of the point |

```sh
TORE_DATA_DIR=<a copy of your profile> cargo run --locked -p tore-app -- --rotor-probe --no-audio
```

## Powered-lift scenarios (VTOL overhaul)

`flight-powered-*` flies the six powered-lift aircraft (AV-8, Yak-141, V-22, AH-64,
Mi-24, CH-47) by short scripted pilot-input tapes, from
[`tools/battery_scenarios/_powered.py`](../../tools/battery_scenarios/_powered.py). The
app's `--replay-input TAPE --maneuver hover` puts an aircraft at rest in the air in its own
hover trim (hands off it holds, so a tape starts from a fixed point), and the tapes
(`tore-pilot 3`, written at run time) fly the rest. Each is open loop, so the checks are
bands read from the result line (`speed_kt`, `altitude_ft`, `crashed`, `heading_deg`,
`bank_deg`, `non_finite`); the numbers are in the
[overhaul baseline](../baselines/vtol-overhaul.md) and the physics in the
[spec](../spec/powered-lift-flight.md).

| Scenario | What it flies | What must hold |
| --- | --- | --- |
| `flight-powered-hover-<aircraft>` (all six) | 15 seconds hands off in the trimmed hover (the Yak-141 with 6,000 lb of fuel: full tanks leave it no hover at 5,000 ft) | Speed under 3 kt and height within 30 ft; a rotorcraft also burns 0.1 to 0.9 lb/s (slice HF2: by the power the rotor draws, not the record's 1 to 3 lb/s) |
| `flight-powered-hover-hold-<aircraft>` (AH-64, Mi-24, CH-47, V-22) | A hard stick pulse from the hover, hover hold engaged at 3.5 seconds, and the same pulse without the hold | The held run ends under 3 kt and within 50 ft of its height; the unheld run is drifting at over 15 kt, so the pulse is real. The jets refuse hover hold by design (unit test A2) |
| `flight-powered-transition-<aircraft>` (all six) | Hover to forward flight: forward cyclic trim taps at the Attitude level (helicopters), the conversion keys at 90 percent lever (V-22; 85 before slice HF3 gave the V-22 its real weight), the manual's three nozzle steps and the forward preset (jets) | A speed over each aircraft's minimum, a height inside its band, no crash |
| `flight-powered-corridor-v22` | Airplane-mode start at 175 kt, the nacelles asked toward helicopter mode for 20 seconds | The aircraft keeps its speed and height and never goes over the overspeed limit: the corridor protection moved the nacelles, the pilot did not |
| `flight-powered-autorotation-ah64` | Engine cut at the start speed, lever down, Attitude level | A steady descent of 1,500 to 3,500 ft in 50 seconds at 90 to 160 kt, no crash |
| `flight-powered-vrs-ah64` | A sinking hover, then full collective, with and without Easy flight physics | Without the cheat it sinks at least 100 ft further than with it: the vortex ring state shows |
| `flight-powered-easy-<aircraft>` (all six) | At stability Off, a 30 percent collective step from the hover (helicopters) or a roll pulse (jets), with and without Easy flight physics | The single-rotor helicopters turn the nose over 15 degrees without the cheat and under 5 with it; the jets bank over 15 degrees without it and under 12 with it; the CH-47 and V-22 have no torque and do not turn either way |
| `flight-powered-touchdown-offfield` | Six headless touchdowns from 60 ft over flat open ground with `--maneuver settle:SINK`, which holds the sink rate on the collective: AH-64 at 5, 15 and 25 ft/s, Mi-24 at 5, V-22 at 5 and 15 | The 5 ft/s landings sit undamaged; the 15 and 25 ft/s ones crash (off an airfield every touchdown over the limits is a crash) ([touchdown rule](../spec/touchdown.md)). The jets (a hands-off hover drifts past 10 kt) and fixed-wing off-airfield crashes are in the unit tests and the `land-off-runway` landing probes |

`mp-seat-ah64` (a human taking a rotorcraft seat mid-flight), which the design named, is
not a scenario: the exact-state continuation it relies on is covered by unit tests
(`powered_state_restoration_and_input_replay_continue_exactly` and the per-type restore
tests) and by the wire's round trips. No windowed scenario was added: the HUD cluster is
covered by `powered_hud` unit tests and `--hud-snapshot`, and the moving parts by the
`flight-animation-*` scenarios.

```sh
TORE_BATTERY_PROFILE="$PWD/.local/dev-profile" python3 tools/battery.py --scenario 'flight-variety-*' --profile .local/dev-profile --jobs 4
```

The profile must contain a current runtime import. Rendering cases use the
required agent window wrapper; their captures remain local.

## What it means for the game

Nothing in this lane crashed, produced a NaN, put an aircraft under the ground, let ammunition go
negative or over capacity, or left an aircraft stuck on a runway. Every aircraft takes off from
every airport, lands on every long runway, recovers from a stall, recovers from a spin unless its
data forbids spinning, and burns and runs out of fuel sensibly. The one real breakage was in the
test tools: the headless combat smoke probe had failed for thirteen of fourteen aircraft since
0.1.0 because it still expected rules the game has deliberately moved past. The probe is fixed
and now passes for all fourteen. Several behaviours are not defined anywhere in the specs and are
listed under "Needs a decision" below, with the evidence.

## Running it

```sh
cargo build --locked -p tore-app
python3 tools/battery.py --lane flight --jobs 6 --windows 2 --tag flight
python3 tools/battery.py --scenario 'flight-land-*' --jobs 6              # one family
python3 tools/battery.py --scenario 'flight-livefire-f18-*' --windows 2   # windowed
```

The whole lane is 3,285 scenarios, 396 of them windowed (through `tools/agent-run.sh`).
The headless ones take about a second each; the missile acceptance runs take up to five minutes
each and the windowed ones five to eight seconds. See "Runtime" at the end for the measured time.

## What is covered

| Family | Scenarios | What it checks |
| --- | --- | --- |
| `takeoff-*` | 772 | Ground start and takeoff for all 14 aircraft from the first and middle airport of every base theater, the Hornet from every airport of every base theater and the first airport of every imported variant: reaches 100 ft, no crash, sane liftoff speed and time, a default load no heavier than the aircraft's maximum takeoff weight, and external tanks that feed before the internal fuel. |
| `land-*` | 676 | The scripted approach and landing (see below): the Hornet at every airport of every base theater, every other aircraft at the first airport of every base theater, the Hornet at the first airport of every variant, all 14 with gear up (must crash for the gear), off the runway (must crash for not landing on a runway) and with no flare, and all 14 in five different winds. On runways of 5,500 ft or more: touches down, stops, no crash, no unsafe touchdown, does not leave the runway surface. |
| `level`, `pull`, `loop`, `roll`, `stall`, `spin`, `bank-left`, `bank-right` | 336 | Every manoeuvre in every aircraft on the default, `--legacy-flight` and `--researched-flight` adapters: no non-finite value, fuel never rises, no energy from nothing, load, speed, altitude and rates within limits, no veil without G. |
| `spinrecover-*`, `stallrecover-*` | 28 | The manual's spin and stall recovery procedures recover every aircraft that can enter a spin or stall (X-31 and the F-22 family cannot spin: their data says so). |
| `climb-*`, `sprint-*` | 28 | Climb ceiling behavior and full-afterburner level flight settling near the top speed. Time above the limit, rather than a peak-speed multiplier, determines overspeed failure. |
| `overspeed-*` | 42 | Every aircraft is held above its limit and lost at a 6 to 10 second boundary with cause overspeed; Invulnerable survives; an unattended dive ends in a loss ([overspeed](../spec/overspeed.md)). |
| `lateral-*` | 4 | Unpowered rudder turns the path and banks; low-speed nosewheel steering reaches 90 degrees for F/A-18D and Rafale C. |
| `target-camera-*` | 6 | Close and long-range target window, F7 and F8 captures for visual inspection. |
| `belly-*` | 28 | The gear key at 80 knots on the roll is refused by the ground sensor (one message, gear still down); the same key once airborne is a normal retraction ([gear on the ground](../spec/gear-on-the-ground.md)). The names keep "belly" from the slide they replaced. |
| `fault*` | 73 | Every system fault 0..44 on three aircraft in a pull, all 45 at once, and one after another, in every aircraft. |
| `combatsmoke-*`, `combatevidence-*`, `missileacceptance-*` | 40 | The headless combat smoke (default slots, five damage classes, jettison, radar power, incoming missiles, jammer), the same smoke with per-slot combat tapes written and replayed to the identical state, and the missile reach probes (the F-14 and Su-35 tables are left to the slow set, below). |
| `ac130-pin-orbit`, `ac130-fire-no-target`, `ac130-track-out-of-arc` | 3 | Headless AC-130 gunsight probes (`--gunsight-probe NAME --aircraft ac130`, `crates/tore-app/src/gunsight_probe.rs`) on flat ground with the imported guns, about half a second each. Orbit at 4,000 feet, pin the ground at its centre and fire for ten seconds (impacts within 150 feet of the pin, no barrel faster than 30 degrees a second, guns READY, never NO LINE OF FIRE; `--gunsight-dump PATH` writes the per-tick gun train as CSV). Fire with nothing held in the default view (all three guns fire, impacts around the aim point). Track a ground object past the wing line (CANNOT BEAR, barrels at the arc limit, camera still on the object, trigger still fires). |
| `ac130-orbit-pin`, `ac130-orbit-slew`, `ac130-orbit-track`, `ac130-orbit-close` | 4 | The AC-130's sensor console (slice nr-orbit; `--a2g-run UKR QUCOL --aircraft ac130 --load C_25.JT --attack orbit`, `crates/tore-app/src/a2g_run/orbit.rs`), about 50 s each. From 3 nm at 4,000 ft the console opens on a pin (also slewed) or the tracked lead tank, and nothing touches the stick or throttle: it enters the orbit; after entry never inside the minimum radius or below the minimum height, every gun and the camera bear at least 90% of the time, bank at most 31 degrees, bank rate at most 8 and pitch rate at most 4 degrees a second, no stall; the slewed pin and the moving tank move the point and the orbit follows; Ctrl+9 again at 120 s hands the aircraft back. |
| `ac130-console-search`, `ac130-console-pin-release` | 2 | The console's search and keys (slice nr-cona; the same run with `--orbit-sight free --console-keys T:KEY,...`, `crates/tore-app/src/a2g_run/orbit/console.rs`), about 25 to 30 s each. The console opens searching with nothing pinned; the keys go in as seat commands (`pin`, `heading+N`, `altitude+N`, `speed=P`, `speed+N`). Search: a 30 degree right turn and 2,000 ft climb, then a 90 degree left turn and a descent; each search settles within 1 degree of its heading and stays within 3 (no turn back), and at the end holds altitude within 200 ft, speed within 8 kt and wings within 3 degrees. Pin and release: P at 15 s joins and enters the orbit (90% bearing after entry), P at 240 s rolls out into the search, which settles the same way. Bank rate at most 8 and pitch rate at most 4 degrees a second, no stall. |
| `livefire-*`, `cheat-unlimited-ammo-*`, `cheat-damage-*`, `countermeasures-*` | about 90 | Windowed: fire every weapon slot of every aircraft (ammunition never negative or over capacity, drops by exactly what was fired, other stations untouched, surface weapons refuse the practice aircraft), Unlimited ammo, the three Damage modes, and chaff and flare counts against capacity. |
| `cheat-*` | 70 | Extra G reaches about 9 G, No redout or blackout, No spins, No crashes and Unlimited fuel on every aircraft. |
| `devices-*` | 14 | Gear, flaps, airbrake and hook stay within 0..1, never move against their command and take the aircraft's own deployment time; an aircraft with no hook does not lower one. |
| `eject-*`, `ejectionpose-*` | 70 | The seat and parachute at 5,000 ft and 250 ft in every aircraft; windowed frames of the three ejection poses. |
| `autopilot-*`, `waypoint-*`, `fuelout-*` | 42 | Heading and altitude hold from a 25 degree bank, waypoint steering, and running out of fuel. |
| `climbout-*` | 30 | The scripted leader's takeoff, gear and flaps up (the A-4E and Su-25 hold the flaps a moment for speed), and cruise, for every aircraft alone and for a wing of five from every base theater. |
| `edge-*`, `terrain-*` | 160 | Flying out over each of the four map edges at 20,000 ft for 200 s (still flying), `edge-lost-*` flying on until the aircraft is lost 105 nm past the map with cause out of bounds ([world edge](../spec/world-edge.md)), and a spin or roll at 90 ft over every theater. |
| `weather*-*`, `hour*-*`, `groundstart-*`, `damage-*`, `bay-*` | about 250 | Windowed frames of every weather condition in every base theater, every hour of the day in three theaters, every aircraft at a ground start, and the damage and F-22 bay fixtures, each checked for a blank or flat frame. |
| `loadout-*` | 56 | Round two: every aircraft with `--loadout none` and `--loadout guns` takes off and lands on a long runway like any other and carries less than its default load. |
| `jettison-*` | 38 | Round two, windowed: jettisoning every station of every aircraft empties exactly that station (an internal station, for example the Su-35 slot 5, refuses and still fires), lightens the load, keeps the flight model's carried weight in step (less fuel already burned) and touches no other station. |
| `fight-*`, `attack-*` | 56 | Round two: the player passive, then attacking through its own controls, in a 5 v 5 against ace or average AI from ahead and behind: the debrief's fate, damage, hit points, crash flag and hit and shot counts agree, and the AI invariants hold. |
| `environment-*` | 96 | The wind, air data and turbulence probe in every base theater at 100 and 5,000 ft in three winds. |
| `panel-*`, `panelfault*` | 175 | Every instrument page of every aircraft, and the Systems page under panel faults 1..35. |
| `window-seeker-*` | 8 | The seeker window: the target camera's place in the small and the large layout before a seeker launch, after it (pop-up at the top right, or left of the top-right window), with free slots, and after the landing ([spec](../spec/target-window.md#seeker-window)). |

Also run by hand and found clean: bad or out-of-range command lines (about twenty, all refused
with a message and no panic), extreme winds up to 200 ft/s (the scripted takeoff pilot crashes in the strongest headwinds only because it holds its rotation), starting at 90,000 ft, and every
theater and weather condition through `--validate-weather` and `--validate-maps`.

## The scripted pilots and probes

The headless flight probe (`--headless-flight`) gained the tools this lane needs, all described
in [DEVELOPMENT.md](../DEVELOPMENT.md#headless-flight-checks): an `extremes:` line that a script
reads for impossible states (non-finite values, fuel that rises, energy from nothing, speed
against the envelope, height above the terrain, the G veil), `--flight-trace`, `--flight-fault`,
`--flight-cheat`, `--flight-fuel`, `--flight-start`, and scripted pilots for spin recovery, stall
recovery, an approach and landing (with gear-up, hard and off-runway variants), autopilot,
waypoint, devices, climb, sprint and ejection. They are `fitted` test harnesses (agent decision,
2026-09-28) and are not game behaviour. The scripted landing floats about 1,500 ft past its aim
point, so it is only expected to stop on a runway of about 5,500 ft or more; a tailwind landing
is allowed a 400 ft longer roll.

## Bugs found

| Symptom | Cause | Fix |
| --- | --- | --- |
| `--combat-smoke` failed for 13 of 14 aircraft (only the MiG-29 passed), since 0.1.0. | Not the game: the probe expected rules that have since changed on purpose. Radar power off now allows an unguided radar-missile release (feature matrix, "Uncued launch with the onboard seeker enabled"); surface weapons refuse the practice aircraft (`WrongTarget`); a fixture flying at the player can win the race against a slow gun and collide with it; the A-4E's guns need far more than forty hits to destroy the player fixture. | The probe now checks those rules and names the slot, weapon and reason when it fails (commit 503b580). All fourteen pass and are in the battery. |
| A combat tape replayed with `--replay-combat` drifted its smoke differently from the live run (`TORE_COMBAT_EVIDENCE` smoke: "serialized live-fire replay diverged"). | The host sets the mission wind on the smoke and countermeasures before every step, but a tape does not record it, so the replay drifted them with no wind. The smoke also compared a live state without airfields to a replay that added them, and kept stepping a crashed flight. | The replay is given the theater's wind (`combat_tape.rs`); the smoke replays without airfields and starts its manual-command tape from a fresh flight. All fourteen aircraft roundtrip their tapes and are in the battery as `combatevidence-*` (commit "Give combat tape replays the mission wind and fix the smoke's tape roundtrip"). |
| A full-power climb carried aircraft far past their own ceiling (X-31 to 66,700 ft against 41,000) and they flew on there; the F-22 could hold level flight 6,000 ft above its ceiling. | The manual (p. 90) says the 1 G ceiling is where the air is too thin to lift the weight, but above the polygon the model kept the full 1 G. | Lift above the ceiling now falls with air density (`flight.rs`, fitted, in FLIGHT-MODEL.md); a zoom climb carries an aircraft at most 23 percent past it and it sinks back. Unit test `above_its_ceiling_an_aircraft_cannot_hold_level_flight`. |
| A loaded aircraft at full afterburner in level flight (autopilot altitude hold) sank into the ground near its top speed: F-22, MiG-21, X-31. | Loading divides the G limit, and on the outermost band of the envelope, where only the 1 G row holds, the divided limit is below 1 G. | Inside the envelope the limit never falls below 1 G, in the flight model and in the AI's own G limit (fitted, FLIGHT-MODEL.md). Unit test `a_loaded_aircraft_holds_level_flight_on_the_outer_band_of_its_envelope`. The sprint scenarios now hold altitude. |
| The RCS instrument window drew "NO EXPOSURE DATA" over the 270 bearing label. | Message placed at the left edge of the window. | Moved below the crosshair. The menus lane fixed the same line the same way; that version is the one merged. |

No other game defect was found in takeoff, landing, the manoeuvres, spin and stall recovery,
faults, ejection, devices, autopilot, fuel, weapons accounting, countermeasures, cheats or the
terrain checks. Three problems belong to other lanes and were handed on:

- **AI wingman flies into rising terrain behind a fast leader.** Still present after round two:
  `tore-app --theater UKR --aircraft f22 --ground-start 3 --ai-probe-ticks 9000 --maneuver takeoff --no-audio`
  ends with `HAZARD off landable surface: Friendly 1-2 agl=14 kt=809` at t=8544 and
  `wingman[Dead ...]`. It happens with any wingman type (`--probe-friendly-aircraft f18` too) but
  only with the F-22 as the scripted leader, which cruises fast. The wingman is in formation at
  full throttle, drifts 2,000 ft below the leader (which holds its MSL altitude), reaches about
  800 kt and hits terrain that rises 586 to 1,726 ft over the last 14,000 ft. It is not the
  envelope: the numbers did not change with the round-two flight-model fixes. The AI terrain floor
  (B44, `ai/steering.rs::terrain_pitch_floor`) looks only 1,000 ft ahead, under a second at 800 kt,
  so it cannot start a climb in time, and the formation code lets the wingman sink below its slot.
  Not changed: it is AI behaviour.
- **The debrief credited kills with no recorded hit (fixed 2026-10-09).** With
  `--aircraft faxx --probe-enemy-aircraft su27 --probe-enemy-skill average --ai-probe-ticks 20000 --probe-fight 5:5 --separation 10 --probe-attack 100:8`
  the debrief showed `kills=[1,...] a2a=0/2` while the probe saw one player hit (the Su-35
  showed `a2a=1/4 kills=[2]`). The cause: a flare or chaff spoofed the missile, the ledger closed
  it as Spoofed, and the missile flew on and struck the aircraft anyway. The damage, the kill and
  the last-attacker credit were applied, but the hit could not be tallied because the missile was
  already resolved. The ledger now remembers spoofed missiles and counts a late strike as the
  missile's hit (see [debrief kills](../spec/debrief.md#kills)). Before the fix the same
  mismatch showed in 33 of 186 AI-lane and combat scenarios (every default `ai-fight-NvN`, the
  MiG-29 vs F-14 skill pairs and 10 fuzz seeds) and the two lane fights above; the scenario check
  no longer exempts the Su-27 and Su-35, and `tools/battery_scenarios/_debrief.py` holds every
  AI probe debrief line to it, for the player and the wingman.
- **AI aircraft collide.** In the F-14 rear-geometry fight two AI enemies touch (56 ft apart)
  and both are lost; the AI invariant counts it. The fight scenarios do not count mid-air
  collisions between AI aircraft.

## Stall and liftoff speeds against the imported data

John doubted the F-22, Su-27 and Su-25 numbers on 2026-09-29: at 80 knots they were
inside their lift envelope, and he expected 130 to 150 knots to fly. This was checked
with the headless takeoff (`envelope:` and `liftoff:` lines, sea-level airports, calm
air, the default loadout, full flaps and afterburner, back stick 0.35 held from the
start). The model is doing what the imported data says. Nothing was changed.

**How the model gets its stall speed** (`flight.rs`, hybrid adapter). It takes the left
edge of the aircraft's imported 1 G speed and altitude polygon at the current altitude
(the game's "stall speed limit", manual p. 90). Full flaps lower it 25 percent (the
reviewed flap effect, [takeoff rules](../spec/takeoff-ground-contact.md)). Lift is
`(speed / stall)^2` of one G up to the stall speed. Between the stall speed and the next
G row's left edge the G limit ramps from 1 G up to that row, and loading divides it
(`1 + loading * loadedElevator / 100`, for example 1.35 for the F-22), so a loaded
aircraft gets a full 1 G only above the stall speed. That is the only weight term: the
polygon itself is fixed, and it is not scaled by the square root of the weight. Thrust
contributes its vertical share while the nose is up. There is no wing area or CLmax in
the model: the imported polygon stands for them.

**The imported data.** The sea-level vertex of each 1 G polygon is round in feet per
second: F/A-18D 200 (118.5 kt), Su-27 180 (106.7), Su-25 130 (77.0), F-22 120 (71.1). The
F-22's edge is 120 ft/s, low against the F/A-18D's 200; the game data gives the F-22 the low
figure, and F-22N and F/A-XX share it. The manual has no stall, takeoff or approach figure
for the F-22, Su-27 or Su-25 (its real-aircraft pages give approach speeds only for a few
other types; the 80 to 90 knot "stall speed" on pp. 70 to 71 is the STOVL vector-nozzle
procedure), so the polygons are the only per-aircraft data. The imported landing limit is
195.5 kt for every aircraft (a touchdown limit, not an approach speed).

| Aircraft | Gross lb | 1 G edge (kt) | With flaps (kt) | Min 1 G with flaps and load (kt) | Rotation (kt) | Liftoff (kt) | Liftoff run (ft) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| F/A-18D | 41,838 | 118.5 | 88.9 | 110.1 | 103.1 | 111.9 | 780 |
| Rafale C | 32,092 | 118.5 | 88.9 | 107.6 | 105.3 | 111.6 | 588 |
| F-14D | 64,511 | 100.7 | 75.5 | 88.5 | 85.7 | 91.3 | 607 |
| A-4E | 22,026 | 106.7 | 80.0 | 91.2 | 90.4 | 94.1 | 847 |
| X-31 | 28,620 | 130.4 | 97.8 | 115.0 | 114.2 | 120.5 | 597 |
| MiG-29 | 31,895 | 118.5 | 88.9 | 106.2 | 105.4 | 111.0 | 506 |
| Su-27 | 57,920 | 106.7 | 80.0 | 91.4 | 92.6 | 96.9 | 429 |
| MiG-21 | 18,018 | 118.5 | 88.9 | 97.2 | 96.2 | 100.9 | 662 |
| Su-25 | 69,875 | 77.0 | 57.8 | 70.8 | 68.4 | 73.4 | 513 |
| MiG-23 | 39,413 | 106.7 | 80.0 | 97.4 | 91.4 | 99.4 | 748 |
| Su-35 | 69,054 | 106.7 | 80.0 | 93.1 | 91.9 | 97.1 | 523 |
| F-22, F-22N, F/A-XX | 57,800 | 71.1 | 53.3 | 70.0 | 73.2 | 77.6 | 207 |

Sea level, so calibrated and true airspeed and ground speed are the same here. The
"Min 1 G" column is where the loaded G limit first reaches 1 G, and liftoff follows it
closely. The default loadout is the takeoff weight in the table; the stall speed does not
change with weight, only the loading term does.

**Against John's reference figures** (unsourced real-world numbers, a plausibility range
only): liftoff 135 to 150 kt for the Su-27, 130 to 145 kt for the Su-25 and 130 to 150 kt
for the F-22; approach 120 to 135, 125 to 140 and 135 to 145 kt. The model lifts them off at
97, 73 and 78 kt, 30 to 45 percent lower. The other types sit low the same way (the F/A-18D lifts
off at 112 kt and its 1 G edge is 118.5 kt, against a real approach of about 135 kt), so the
cause is the game's own polygons, whose left edges are below real stall speeds, and not a
fault in one aircraft's handling. The scripted landing's
approach speed is 1.3 times the 1 G edge (F-22 92 kt, Su-25 100, Su-27 139), a probe choice
and not an aircraft figure.

John decided the same day to have the model scale the stall speed with weight,
as a deliberate departure from the polygon data (next section). The
`flight-takeoff-*` scenarios check that liftoff follows the model's own
minimum speed for 1 G.

## Weight-scaled stall speed (2026-09-29)

`opinionated`, requested by John; rules in the
[takeoff spec](../spec/takeoff-ground-contact.md#weight-scaled-stall-speed). The polygon's
slow edges apply at a reference weight and grow with the square root of the weight over
it. The first version used the empty weight for every aircraft. It could not bring every
aircraft within 10 percent of John's figures: no single fraction of the empty weight (0.6
to 1.6 tried) does, because a fraction low enough to help the F-22 family and Su-25 puts the
F/A-18D, Rafale and A-4E 20 to 40 percent too high, and a high one does the reverse. Since
2026-09-30 (`fitted`, an agent decision, John's decisions 1 and 2 of the aircraft pass) each
aircraft has its own fraction of its empty weight, keyed by the aircraft's identity
(`STALL_REFERENCE_FRACTIONS`, `AircraftModel::stall_reference_fraction`, the table in the
[takeoff spec](../spec/takeoff-ground-contact.md#reference-fraction-per-aircraft-fitted)).
A lower fraction means faster liftoff and approach. The X-31, with no figure from John,
keeps 1.00.

Liftoff at the default loadout (calm, sea level, UKR airport 1, full flaps, afterburner,
back stick 0.35), with the imported speeds (`--retail-stall-speeds`), with one reference for
every aircraft (the empty weight, before 2026-09-30) and now, against John's figures where
he gave them:

| Aircraft | Gross lb | Scale now | Liftoff, imported (kt / ft) | Liftoff, one reference (kt / ft) | Liftoff now (kt / ft) | John's liftoff |
| --- | ---: | ---: | ---: | ---: | ---: | :---: |
| F/A-18D | 41,838 | 1.17 | 112 / 780 | 147 / 1,383 | 129 / 1,050 | - |
| Rafale C | 32,092 | 1.17 | 112 / 588 | 149 / 1,056 | 128 / 781 | - |
| F-14D | 64,511 | 1.38 | 91 / 607 | 114 / 948 | 123 / 1,107 | - |
| A-4E | 22,026 | 1.30 | 94 / 847 | 132 / 1,716 | 121 / 1,427 | - |
| X-31 | 28,620 | 1.33 | 121 / 597 | 156 / 1,001 | 156 / 1,001 | - |
| MiG-29 | 31,895 | 1.21 | 111 / 506 | 143 / 851 | 132 / 719 | - |
| Su-27 | 57,920 | 1.37 | 97 / 429 | 120 / 659 | 128 / 757 | 135 to 150 (5 percent low) |
| MiG-21 | 18,018 | 1.39 | 101 / 662 | 118 / 911 | 136 / 1,224 | - |
| Su-25 | 69,875 | 1.74 | 73 / 513 | 93 / 836 | 123 / 1,506 | 130 to 145 (5 percent low) |
| MiG-23 | 39,413 | 1.46 | 99 / 748 | 126 / 1,219 | 142 / 1,552 | - |
| Su-35 | 69,054 | 1.34 | 97 / 523 | 124 / 855 | 127 / 897 | - |
| F-22, F-22N, F/A-XX | 57,800 | 2.14 | 78 / 207 | 103 / 359 | 128 / 559 | 130 to 150 (2 percent low) |

Approach speed of the scripted landing (flaps down, 65 percent internal fuel, stores as
loaded; 1.3 times the flapped, scaled stall speed and never under 1.05 times the loaded
minimum for 1 G, capped at 166 kt, 0.85 times the landing limit) with the imported speeds
(full fuel, 1.3 times the polygon edge), with one reference and now, against the figures
John gave:

| Aircraft | Imported (kt) | One reference (kt) | Now (kt) | Figure (kt) | Off by now |
| --- | ---: | ---: | ---: | :---: | ---: |
| F/A-18D | 154 | 156 | 135 | 135 | in range |
| Rafale C | 154 | 158 | 135 | 130 to 140 | in range |
| F-14D | 131 | 125 | 135 | 130 to 140 | in range |
| A-4E | 139 | 148 | 136 | 130 to 140 | in range |
| X-31 | 166 | 166 | 166 (cap) | (research aircraft) | - |
| MiG-29 | 154 | 154 | 140 | 135 to 145 | in range |
| Su-27 | 139 | 132 | 142 | 120 to 135 | +5 percent |
| MiG-21 | 154 | 137 | 160 | 160 to 170 | in range |
| Su-25 | 100 | 97 | 131 | 125 to 140 | in range |
| MiG-23 | 139 | 134 | 152 | 150 to 165 | in range |
| Su-35 | 139 | 136 | 139 | 135 to 145 | in range |
| F-22, F-22N, F/A-XX | 92 | 102 | 148 | 135 to 145 | +2 percent |

Eight of the eleven figures are inside John's range and the other three are within 5
percent of it, against 2 of 11 with one reference. The F/A-18D's 135.1 kt against a single
figure of 135 counts as in. The Su-27 is the one aircraft the model cannot fit both ways:
John's liftoff (135 to 150 kt) and approach (120 to 135 kt) ranges put its approach at or
below its liftoff, and the scripted approach is about 1.1 times the liftoff at every
fraction, so 0.86 takes the middle (John's decision 2). The nine aircraft with no liftoff
figure are fitted to their approach only, and their liftoff comes out 4 to 15 percent under
it. The battery's windows for these figures are `KNOWN_LIFTOFF` and `KNOWN_APPROACH` in
`tools/battery_scenarios/flight.py`, re-recorded on 2026-09-30 (plus or minus 6 percent
around the recorded value, on top of John's range plus or minus 10 percent).

Each fraction was chosen at least 0.02 clear of the liftoff jumps that appear when the
model's G rows cross at a larger scale (the X-31 between 0.94 and 0.95, the MiG-21 between
0.68 and 0.69 and the Su-25 between 0.52 and 0.53, measured by sweeping every fraction 0.06
either side; the Su-25's 0.55 is the closest). Since the Su-25's record correction
(2026-10-13) its fraction is 0.51 on the corrected empty weight and its jump sits near
0.49; `KNOWN_LIFTOFF` is 120.2 kt and `PULL_G_RETAIL` 5.41 G for it.

**Combat-speed G (correction, 2026-09-29).** The first version scaled every row of the
polygon, which counted the weight twice (the edges, then the loaded-elevator divisor) and left a
fuelled F/A-18D pulling 5.1 G in the held pull at 450 kt and 5,000 ft (7.64 with the imported
speeds; the replay lane's scripted pull peaked at 3.8 G). Now the scale applies to the 0 G, 1 G and
2 G rows, fades linearly to nothing at the 4 G row and leaves the rows from 4 G up as imported
(`row_scale`, `STALL_SCALE_FADE_G`). The stall, liftoff and approach speeds are unchanged (they
come from the 1 G edge and the ramp to the 2 G edge), and above roughly twice the stall speed the G limit is the
imported one; below it the weight still costs G. The divisor stays: removing it would put the
F-22's liftoff back near 80 knots (97 kt for 1 G instead of 74).

Held pull at 450 kt and 5,000 ft (default loadout, `--maneuver pull`, max G reached), and the
G limit at the top row's own speed (corner speed) at 5,000 ft with the same weight
(`--maneuver gcurve`), the first (double counted) version, now, and with `--retail-stall-speeds`:

| Aircraft | Pull before | Pull now | Pull retail | Corner G before | Corner G now | Corner G retail |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| F/A-18D | 5.10 | 7.64 | 7.64 | 5.02 | 7.53 | 7.53 |
| Rafale C | 6.57 | 7.39 | 7.39 | 4.87 | 7.31 | 7.31 |
| F-14D | 6.18 | 6.18 | 6.18 | 4.40 | 6.15 | 6.15 |
| A-4E | 6.10 | 6.13 | 6.13 | 4.29 | 6.01 | 6.01 |
| X-31 | 5.64 | 7.25 | 7.25 | 4.82 | 7.22 | 7.22 |
| MiG-29 | 4.78 | 7.15 | 7.15 | 4.71 | 7.06 | 7.06 |
| Su-27 | 5.25 | 7.54 | 7.54 | 5.18 | 7.77 | 7.77 |
| MiG-21 | 6.16 | 6.47 | 6.47 | n/a | n/a | n/a |
| Su-25 | 4.99 | 4.99 | 4.99 | 4.11 | 4.93 | 4.93 |
| MiG-23 | 5.92 | 6.77 | 6.77 | 4.21 | 6.73 | 6.73 |
| Su-35 | 5.22 | 6.96 | 6.96 | n/a | n/a | n/a |
| F-22 family | 6.14 | 6.91 | 6.91 | 4.56 | 6.84 | 6.84 |

(The MiG-21 and Su-35 have no top row at 5,000 ft, so they have no corner figure there.) The
pull now equals the imported value for every aircraft and the corner G is within 2 percent
of it (the F/A-18D's small difference is the divisor at the moment of measuring); measured again
with the per-aircraft fractions of 2026-09-30, every held pull and every corner G is the same.
The G limit against speed at gross weight
(default loadout, UKR airport 1, calm, 5,000 ft, full back stick), imported speeds / now:

| Speed (kt) | F/A-18D | F-22 | Su-27 |
| ---: | :---: | :---: | :---: |
| 100 | 0.84 / 0.84 | 1.03 / 0.76 | 0.86 / 0.86 |
| 150 | 1.32 / 1.16 | 2.28 / 0.76 | 1.57 / 0.86 |
| 200 | 1.67 / 1.67 | 3.04 / 2.66 | 1.73 / 1.57 |
| 250 | 2.51 / 2.51 | 4.56 / 4.56 | 3.45 / 3.45 |
| 300 | 3.35 / 3.35 | 6.08 / 6.08 | 4.32 / 4.32 |
| 350 | 5.02 / 5.02 | 6.84 / 6.84 | 5.18 / 5.18 |
| 400 | 6.70 / 6.70 | 6.84 / 6.84 | 6.91 / 6.91 |
| 450 and up | 7.53 / 7.53 | 6.84 / 6.84 | 7.77 / 7.77 |

So the weight now costs G only at the slow speeds where the low G rows lie (up to about 200 to
250 kt), where a heavy aircraft really is lift-limited, and nothing at combat speed. With the
per-aircraft fractions the clean aircraft (no flaps, 5,000 ft, default load) first pulls 1 G at
150 kt (F/A-18D, Rafale, A-4E, Su-25), 175 kt (F-14D, MiG-29, Su-27, MiG-23, Su-35) or 200 kt
(X-31, MiG-21, F-22 family), in 25 kt steps, against 100 to 150 kt with the imported speeds: the
F-22 cannot hold level flight clean below about 200 kt, the direct cost of lifting off at 128 kt
(the flaps lower this by a quarter).

**Short strips.** The roll to liftoff at full afterburner is longer for every aircraft, most
for the heavy loadouts and for the ones whose fraction is under 1. From the Santa Fe start
(`--theater APA --ground-start 3`, a strip of about 1,074 ft, 1,020 ft of paved run from a solo
start slot; San Carlos is the same length) the liftoff runs are, with the imported speeds and now:
F/A-18D 823 to 1,098 ft, Rafale C 620 to 816, F-14D 650 to 1,164, A-4E 908 to 1,507, X-31 626 to
1,040, MiG-29 532 to 751, Su-27 454 to 790, MiG-21 702 to 1,278, Su-25 560 to 1,588, MiG-23 796 to
1,622, Su-35 555 to 940 and the F-22 family 222 to 584. So the F/A-18D, F-14D, A-4E, X-31, MiG-21,
Su-25 and MiG-23 no longer lift off within the paved strip (they roll on past its end); the
Rafale, MiG-29, Su-27, Su-35 and F-22 family still do. The AI takeoffs on those strips were already
known failures (lane-ai item 11). Since 2026-09-30 a Quick Mission never starts on such a strip
(lane-ai item 11); these developer `--headless-flight --ground-start` runs, which measure the
flight model and not a mission, still start there. Landing rollouts shorten slightly (the flare
from 60 ft touches down nearer the threshold), so the 5,532 ft UKR runway still works for all twelve.

**Other users of the stall speed.** The flight model, the stall warning and departure
behaviour, the autopilot, the ejection G check, the flight envelope window (the drawn slow
edges follow the weight), the scripted landing probe and the crew callouts all
read the scaled speeds. The HUD landing-speed brackets are not implemented in the game, so
there is nothing to update. The AI reads the same scaled speeds through the model's
configuration without any change to `tore-sim/src/ai`. The AI follow-up this rule needed is done: the AI lane retuned its speed
limits during the bug bash (`4e47944`, `0057684`: the minimum never below the 1 G level-flight
speed, the maximum never above the fastest row that leaves 1.2 G, a final lift margin of 1.15 G;
the twelve scenarios that failed first are listed in the
[battery report](../baselines/battery-2026-09-28.md), section 2). With the per-aircraft stall
references of the aircraft pass (2026-09-30) the whole AI lane passes. AI takeoffs are
unaffected (the roll only lasts longer).

## Needs a decision

None of these is defined in the specs, the manual text or the feature matrix, so none was changed.

1. **Overspeed** (decided). John requested the current five-second safe period,
   seeded 25 percent rolls at 6 to 9 seconds, and guaranteed loss at 10 seconds
   on 2026-10-01. The fixture holds 1.1 times the current limit to isolate timing;
   it is not a natural acceleration test. [Behavior](../spec/overspeed.md).

2. **The autopilot's altitude hold outside the envelope.** Beyond the top speed the aircraft cannot
   hold 1 G and sinks; the autopilot spec says "no guaranteed hold outside the flight envelope
   is implied", so this is as written, and inside the envelope the hold now works (see "Bugs
   found"). Overspeed now has a rule (item 1); the hold outside the envelope is unchanged.
3. **The map edge** (decided). John asked on 2026-09-29 for a warning at 100 nautical miles past the map
   and a loss at 105, AI aircraft lost without a kill ([world edge](../spec/world-edge.md),
   `opinionated`, distances are agent decisions). The `flight-edge-*` scenarios fly 200 seconds out
   and must stay flying; `flight-edge-lost-*` fly on until the loss.
4. **A ground-start airport is a flat square of 5,000 to 8,000 ft a side and all of it counts as
   runway.** The footprint (`footprint_half_ft` in the `landing_start:` line, for example 2,604
   by 3,000 ft either side of the centre at UKR airport 1) is the airport shape's bounds. A
   touchdown 1,300 ft to the side of the paved runway is graded as a normal landing, a takeoff
   roll can run past the runway's end on the flat square, and rolling off the paved strip is never
   penalised. The airport spec calls this "fitted contact" and says terrain outside is unchanged,
   so it is by design; whether a narrower landing area is wanted is a decision.
5. **The small airstrips (about 1,000 ft) accept any aircraft.** Changed (John, 2026-09-30): a Quick Mission ground start is no longer offered on a runway under 2,000 ft (the 22 strips), so no aircraft or wing starts there. Round two had searched the specs, the airport spec and the manual for a minimum runway length and found none, so the rule is John's, not retail's ([short strips](../spec/airports.md#short-strips)).
6. **Combat tapes do not record everything the host does.** A replayed tape shows the player alive
   after a flight that crashed, because the host turns a crashed flight into a dead player and the
   tape does not record that. Only the developer replay uses tapes; the mission recordings of
   [REPLAYS.md](../REPLAYS.md) are separate.
7. **Carrier hook and arresting gear.** The hook lowers on the five aircraft that have one (F/A-18D,
   F-14D, A-4E, F-22N and the F/A-XX concept) and does nothing else; carriers are listed as
   remaining work in the feature matrix.

## Needs a human eye or ear

- The `weather*` frames: the cloudy and foggy conditions are a solid white at the 5,000 ft
  start, with no ground visible at all. Check that being inside the cloud layer is what the
  retail game shows.
- The `hour*` frames of the day: hour 07 has a dawn glow, hours 08 to 19 are full day and night
  starts at 20; check the sunset transition looks right around 19 to 20.
- The `damage-*`, `ejectionpose-*`, `groundstart-*` and `bay-*` frames: the checks only reject
  blank pictures. The F-22 bay frames are taken from a distance where the doors are hard to see.
- Captures made with `--flight-devices` show "RADAR OFF" on the radar window because the capture
  path skips the combat service; live flight shows the scope. Cosmetic, in a diagnostic mode only.
- Aural checks (stall horn, RWR tones, gun and missile sounds) belong to the replay and sound work.

## The slow set and the missile acceptance tables

`--missile-sag` is a headless motion regression using imported AIM-7, AIM-9M,
AIM-120 and Sea Sparrow records, a continuously known stationary target, and
flat ground.
It reports the swept closest approach before the first pass, ground contact or
expiry, alongside the terrain-free fly-out prediction and coasting sag ticks.
Target heights are 300 and 10,000 ft AGL. Aircraft launches inherit 600 ft/s at
target height; the SAM starts stationary on a 20 ft rail aimed at the target.
This isolates motion, not seeker acquisition, launch permission or damage odds.
The `flight-missile-sag` battery scenario checks the coasting long-range rows.
See the [sag validation](../baselines/missile-sag.md) for before/after measurements.

`--missile-acceptance` prints one row per weapon, mode, launch motion, target motion and range
(hit, expiry or an inhibited shot). Every aircraft was run to completion in an optimized build
(`cargo build --release -p tore-app`, each run a few minutes; the F-14's long-range AIM-54 rows
take far longer) and every row was checked: a hit has a positive time, a sane average speed
(100 to 5,500 ft/s) and a travel that matches the target's motion (about the range for a stationary
target, less for an approaching one, more for a receding one); an inhibited shot never flies; the
only outcomes are hit, expiry and inhibit. No impossible row was found in any table. Patterns
worth knowing: every infrared missile has a block of inhibited rows (the seeker is still
acquiring when a cued shot is asked for) and eight boresight expiries (a blind shot at an
approaching target from a climbing or slipping launcher); the F-22A has ten more AIM-120
boresight expiries against a crossing target at 75 and 100 percent range than the F-22N and the
F/A-XX, which fits the F-22A's bay doors adding a second to the launch.

The slow set is off by default (the default battery leaves out the F-14 and Su-35 tables, which
take about 40 minutes each in a debug build):

```sh
CARGO_TARGET_DIR=target-release cargo build --release --locked -p tore-app
TORE_BATTERY_SLOW=1 python3 tools/battery.py --scenario 'flight-slow-*' \
    --bin target-release/release/tore-app --jobs 4
```

## Not covered

RWR reaction to AI radars and missiles, and system faults appearing on the panels during an AI
fight, have no headless or scripted path (the AI probe does not feed the player's RWR or panel
faults, and `--dummy-aircraft` cannot be combined with `--live-fire`), so they were not tested in
round two; the windowed `cheat-damage-*` scenarios cover system faults from incoming missiles and
the `--combat-command` sequences cover designation. Radar and infrared channel switching and
target cycling with several targets in view are exercised only by the probe-attack leader
(`attack-*`), not by a scripted key sequence.

## Render captures

`render-night-light-slots` is a CPU-only exception: it runs
`--night-light-slots` without a window or GPU. The synthetic regression checks
240 frames with eight sustained fires and up to 64 remote muzzle/impact
flashes, through both light budgets and the camera-relative upload. Run it
with `--scenario render-night-light-slots --no-unit-tests --with-windows no`.
The emission and point-light rules are in [night glow](../spec/explosions.md#night-glow).

`python3 tools/battery.py --scenario 'render-*' --windows 1` runs windowed captures
(through the wrapper, one window at a time) that reproduce drawing faults.
`render-ah64-stub-wing-pylons` and `render-ah64-stub-wing-at12` use the CPU stores
audit to check that both Apache hanger rows meet the low stub wing rather than the rotor disk. They
check actual rendered geometry and need an imported profile, with no window.

The two `render-capture-*-deterministic` cases repeat a bare flight or terrain
capture three times and compare complete PPM bytes. They fix launch time,
disable controllers and keep focus-related pause presentation stable.
The art inspection cases below check for blank frames; look at their PPMs:

| Scenario | What it shows | What to look for |
| --- | --- | --- |
| `render-f14-roots-top`, `render-f14-roots-side` | Fully swept F-14, top and side | Wing/tail attachments are symmetric, and the wing sits close to the outer root under the fixed glove. |
| `render-f14-exhaust-top` | Top view with afterburners | Exhaust collars and plume roots have equal widths and engine centerlines. |
| `render-f14-rear-belly` | Rear underside with afterburners | Aft belly and tail fairing seams are closed; no sky shows through. |
| `render-f14-nose-belly`, `render-f14-devices-belly` | Forward underside with gear/brakes stowed and deployed | Nose side and under-cockpit seams are closed; intakes and device movement remain visible. |
| `render-f14-hook-*` | Deployed hook from both sides, half retracted and stowed | The F-22N striped shank and hooked end keep their cutout outline. Retraction stays attached and rigid; zero extension is hidden. |
| `render-nosewheel-hud-inactive` | F/A-18 takeoff roll past the steering cutoff | NSW authority disappears from the HUD. |
| `render-mig21-upper-skin` | The MiG-21 in free flight, flight view 2, look `0,-12`, `TORE_WEATHER_TIME=11:00` | The upper wing is plain camouflage: no grey underside shape shows through it. |
| `render-mig25-skin` | The MiG-25 in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | No underside shape shows through the top and the double-sided wing panels do not z-fight; the two omitted E0 wing-marking decal slots leave no blank or skin-filled patch. |
| `render-f16a-skin` | The F-16A in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | The shared `F16.SH` draws whole: no underside shape shows through the top and the double-sided wing panels do not z-fight; the five omitted E0 decal slots leave no blank or skin-filled patch. |
| `render-mig31-skin` | The MiG-31 in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | No underside shape shows through the top and the double-sided wing panels do not z-fight; the two omitted E0 wing-marking decal slots leave no blank or skin-filled patch. |
| `render-m2000-skin` | The Mirage 2000 in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | The shape draws at its real size (per-axis scale) with no underside showing through the top and no z-fighting on the double-sided panels; the omitted E0 decal slots leave no blank or skin-filled patch. |
| `render-f15e-skin` | The F-15E in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | The shape draws at its own size with no underside showing through the top and no z-fighting on the double-sided panels; the omitted E0 decal slots leave no blank or skin-filled patch. |
| `render-gripen-skin` | The Saab Gripen in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | The shape draws at its own true size with no underside showing through the top and no z-fighting on the double-sided panels; the five omitted E0 decal slots leave no blank or skin-filled patch. |
| `render-f5e-skin` | The F-5E in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | The F-5E draws its own `F5E.SH` and `_F5E.PIC` with no underside showing through the top and no z-fighting on the double-sided panels; the two omitted E0 wing decal slots leave no blank or skin-filled patch. |
| `render-mf1-skin` | The Mirage F1 in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | The shape draws at its real size (per-axis scale) with no underside showing through the top and no z-fighting on the double-sided panels; the two omitted E0 wing-marking decal slots leave no blank or skin-filled patch. |
| `render-tornado-skin` | The Tornado IDS in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | The shape draws at its header scale (true size) with no underside showing through the top and no z-fighting on the double-sided panels; the two omitted E0 fin decal slots leave no blank or skin-filled patch. |
| `render-b52-skin` | The B-52H in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 0.5` (the 185 ft span needs the widest view) | The shape draws at its header scale (true size) with no underside showing through the top and no z-fighting on the 12 double-sided panels; the two omitted E0 wing decal slots leave no blank or skin-filled patch. |
| `render-tu95-skin` | The Tu-95 in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 0.5` (the 165 ft span needs the widest view) | The shape draws at its header scale (true size) with no underside showing through the top and no z-fighting on the 12 double-sided panels; the two omitted E0 wing decal slots leave no blank or skin-filled patch; the four propeller discs draw as one blur image each. |
| `render-b2-skin` | The B-2 in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 1.4` | The shape draws at its header scale (1/3 ft a unit, true span) with no underside showing through the top and no z-fighting on the double-sided panels; the one omitted E0 decal slot (`2a73`, beside the cockpit) leaves no blank or skin-filled patch. |
| `render-f117-skin` | The F-117A in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | The shape draws at its real size (per-axis scale) with no underside showing through the top and no z-fighting on the double-sided panels; the five omitted E0 decal slots leave no blank or skin-filled patch. |
| `render-b1b-skin` | The B-1B in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 0.5` (the 137 ft span needs the widest view), `--loadout none` (the default load is refused at start, B1B-6/B1B-8) | The shape draws at its header scale (4/3 ft a unit, true size) with the wing spread and no underside showing through the top and no z-fighting on the double-sided panels; the two omitted E0 glove decal slots leave no blank or skin-filled patch. |
| `render-tu160-skin` | The Tu-160 in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 0.5` (the 181 ft span needs the widest view) | The shape draws at its header scale (8/3 ft a unit, true size) with the wing spread at 20 degrees, no underside showing through the top and no z-fighting on the double-sided panels; the two omitted E0 decal slots (on the fixed glove) leave no blank or skin-filled patch. |
| `render-f111-skin` | The F-111F in free flight with no stores, flight view 2, wing spread (`--flight-speed 200`), gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 2` | The shape draws at its real size (per-axis shape scale 0.859, 1.0, 0.827 on the header scale) with no underside showing through the top and no z-fighting on the 19 double-sided panels; the shape has no E0 decal faces, and the fixed glove plates lie inside the spread wing and show nothing. The default load is not used: with the full tank it is over the maximum take-off weight and free flight refuses to start. |
| `render-tu26-skin` | The Tu-26 in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 1` | The shape draws at its header scale (4/3 ft a unit, 101 ft spread span) with no underside showing through the top and no z-fighting on the double-sided panels; the omitted E0 decal slots leave no blank or skin-filled patch. |
| `render-su34-skin` | The Su-34 in free flight, flight view 2, gear down, look `0,60` (top) and `0,-60` (under), `TORE_WEATHER_TIME=12:00`, `--flight-zoom 1.4` | The shape draws at its header scale (1/3 ft a unit, 1:1) with no underside showing through the top and no z-fighting on the 41 double-sided panels; the two omitted E0 wing decal slots (`3786`, `48d7`) leave no blank or skin-filled patch. |
| `render-runway-paint-calm` | The F/A-18 on a UKR ground start (airport 1), takeoff maneuver, 1,300 probe ticks, flight view 1, `TORE_WEATHER_TIME=07:15`, `TORE_WIND=0,0` | The runway paint and joints are whole, with no flicker or z-fighting. |

The synthetic GPU checks run separately with
`cargo test --locked -p tore-app gpu_airport_ -- --ignored`. They test that
pavement never hides an aircraft above it and that paint over pavement holds at
distant and moving cameras.

## Headless animation regressions

`flight-animation-*` scenarios run the actual drawing-path geometry probe for
individually reviewed aircraft, without opening a window. Combined rotor, flap/roll and
nozzle sweeps are required where their aircraft contracts specify them. They require reviewed
attachment scope and passing per-control gates, including signed motion,
root/skin coherence, wheel rigidity/separation, intermediate topology and hook
closing direction. They write user-owned OBJ and CPU contact-sheet evidence
under the ignored battery output directory. Apache and Chinook scenarios also
check the loaded fixed-gear state and ignored retraction commands before the
mesh sweep. Multi-axis artifacts are required separately, including both V-22
conversion/rotor and flaperon sweeps and all 125 Rafale control combinations.
F-14 isolated controls use the existing unswept 400-knot threshold; separate
coupled sweep/flap/roll and pitch/roll grids cover its operational interactions.
Its hook also samples positive near-stow before hiding. A-4E adds 202 samples
each for gear, hook and brake, plus coupled flap/roll and pitch/roll checks.
The original stowed A-4E hook stays visible at zero. Exact selection keys
keep F/A-XX separate from the F22N source donor, and the F-16A from the F-16C,
in reports and scenario matching. `flight-animation-f16a` expects the `F16.PT`
key and runs the shared F-16 witness set on its own identity. MiG-31 adds a
202-value dense gear sweep, a 25-combination flap/roll grid, and brake and
exhaust endpoints; the gear, brake and plume each carry their own rigid, hinge
and direction checks. The Mirage 2000 adds the same 202-value dense gear sweep (grounded at gear 1,
mains inward, nose rearward), a seven-value airbrake sweep that checks the
closed plate on the spine, and the 25-combination pitch/roll grid; its
`flight-variety-m2000` scenario flies 1,200 headless ticks.
The F-5E (F5E-3) adds the 25-combination pitch/roll grid (`flight-animation-f5e`): its stabilators must follow pitch only and its ailerons roll only, each staying put under every other control; its gear, brake and plume follow in F5E-4 (next paragraph).
The F-5E (F5E-4, 2026-11-10) adds the same 202-value dense gear sweep (grounded at gear 1, mains inward, nose forward) and a seven-value brake sweep that checks the closed plate on the dorsal ridge; `flight-animation-f5e` now also expects the 203-row `dense-gear.csv`, and `flight-variety-f5e` flies 1,200 headless ticks.

The F-15E (F15E-4, 2026-11-10) adds the same 202-value dense gear sweep (grounded at gear 1, legs and nose forward, doors outward) and a seven-value brake sweep that checks the closed piece on the dorsal ridge; its control-surface rows stay `unreviewed` until F15E-3, and `flight-variety-f15e` flies 1,200 headless ticks.
F15E-3 (2026-11-10) makes elevator, aileron and flaps `required` (whole-panel stabilators with roll, ailerons about their slanted hinges, flaps), rudder `not-applicable` (no rudder motion), and adds the 25-combination pitch/roll grid and 21-value elevator, aileron and flap sweeps to the `flight-animation-f15e` sweep row.

The Gripen (GRI-3, GRI-4) adds a 202-value dense gear sweep, a seven-value airbrake sweep, its gear, brake and exhaust witnesses, and the 25-combination pitch/roll grid over both canards (pitch together, roll against each other) and both elevon wedges, with the rudder, elevator, aileron and flap witnesses; `flight-animation-gripen` is registered and `flight-variety-gripen` flies 1,200 headless ticks.

The Mirage F1 (MF1-3) adds the 25-combination pitch/roll grid (`flight-animation-mf1`): its tailplane must follow pitch only and its aileron panels roll only, each staying put under every other control including the flap lever (the probe records the flap control as not applicable for this aircraft); its gear, airbrake and plume are still the interim rig.
Update (MF1-4, 2026-11-10): the Mirage F1 adds the same 202-value dense gear sweep (grounded at gear 1, mains forward and inward into the fuselage side, nose rearward), a seven-value airbrake sweep with the closed plate on the ridge line, the exhaust sweep, and is registered in the variety flight scenarios (`flight-variety-mf1` flies 1,200 headless ticks); `flight-animation-mf1` now expects the 203-line dense gear file beside the 26-line grid.

The Tornado (TOR-4, 2026-11-10) adds the same 202-value dense gear sweep (grounded at 7.33 ft at gear 1, three rigid pieces folding forward, near-stow checked against the neutral skin), a seven-value brake sweep (two plates, closed flat at the hinge height) and a seven-value exhaust sweep (two plumes from the exit plane), beside TOR-3's 25-pose grids at 25 and 67 degrees and its 35-pose sweep/flap grid; `flight-variety-tornado` flies 1,200 headless ticks.

The F-117 (F117-3 and F117-4, 2026-11-15) registers `flight-animation-f117`: its two ruddervator assemblies follow pitch (collective) and yaw (differential) in the 25-combination pitch/yaw grid, while aileron, flaps, brake and exhaust must move nothing (the shape has no elevon, flap, airbrake or plume faces, so the probe records them as not applicable); the 202-value dense gear sweep (grounded at gear 1, three legs and three doors, stow checked against the neutral skin) and the 8-value interim bay sweep (`bay-sweep.csv`) must pass every row, and `flight-variety-f117` flies 1,200 headless ticks.

The F-111F (F111-3, 2026-11-15) registers `flight-animation-f111`: the tailerons follow pitch and roll in two 25-combination grids (the wing spread at 200 knots and fully swept at 650), the wing panels turn whole about the pivot by the speed schedule (a 40-pose sweep against speed and flaps, the measured tip turn equal to the schedule within 0.01 degree, flaps holding the wing spread), the root seam, swept span and taileron clearance are written at nine sweep angles (`sweep-seam-span-taileron.csv`, 19 lines, the swept panel never entering the taileron), and the 202-value dense gear sweep, the 8-value dorsal brake sweep and the 7-value plume sweep must pass every row; the hook is not applicable (the record has none) and the bay is unreviewed until F111-5. `flight-variety-f111` flies 1,200 headless ticks.

Update (F117-5, 2026-11-15): the bay sweep now checks the fitted bays (six belly skins whole at 0, no lining; open, 20 lining faces, a vertex on each of the four hinge lines, door free edges `leaf * sin(90 degrees * value)` below the belly), still 8 values. New scenarios: `flight-bay-fire-f117-row4` and `-row5` (windowed, the F-22 one-press release on the F-117's two bays: doors open before the GBU-27 leaves, closed after), `bay-f117-0`, `-0.5` and `-1` (the `--flight-bay` frames, as for the F-22), and `render-f117-bay-clearance` (headless: the default GBU-27s clear the doors, both ends and the trapeze rail by at least 0.05 ft with the doors open).

The Tu-160 (T160-3, 2026-11-15) registers `flight-animation-tu160`: the 25-pose pitch and roll grids at the spread and the fully swept wing (the whole tailplane quads turn about the lateral spindle, pitch plus differential roll), the 36-pose sweep and flap grid (the tip turn of each wing panel against the 20, 35 and 65 degree detents and their smoothstep transitions, flaps holding it spread), the 202-value dense gear sweep (three rigid legs, grounded at gear 1, the stowed legs inside the neutral skin), the swept span and the root seam files (`sweep-span.csv`, `sweep-seam.csv`), and single-sample checks of the rudder strip, the two flap strips with their end caps, the keel plates and the four plumes; every row must pass.

The Su-34 (SU34-3, 2026-11-15) registers `flight-animation-su34`: its two canards (pitch only), two tailerons (a quad and a root triangle a side as one panel, pitch plus differential roll), two outer ailerons, two flap morphs, airbrake, gear and plume must pass every row of the 25-pose pitch/roll grid, the 25-pose flap/roll grid, the dense elevator, aileron and flap sweeps (21 values each), the 7-value brake sweep and the 202-value gear sweep, while the rudder must move nothing (the fins have no rudder face; the probe records it as not applicable).

Run one repaired aircraft before expanding the audit:

```sh
python3 tools/battery.py --scenario flight-animation-a7 --profile .local/dev-profile
```

Expand the roster in `tools/battery_scenarios/animation.py` only after independent
source and pose-sheet review. This regression family does not replace that
review, validate GPU appearance, or establish retail animation parity. Current
coverage and remaining profiles are in the
[aircraft animation audit](../baselines/aircraft-animations.md).

## Runtime

The full lane, 3,089 scenarios with `--jobs 6 --windows 2`, took 1,804 seconds (30 minutes) on the
24-thread dev machine with other agents running (load average about 24). Round two's run had one
failure, the jettison check of the Su-35's internal slot 5, which was the check's mistake and is
fixed; that family passes 38 of 38, and the rest passed. The missile acceptance
runs (about five minutes each in a debug build) and the windowed frames set the length.

The `render-night-effect-lights` scenario renders fires of 24, 90 and 160 feet
beside a burning flare and bomb fireballs before and after their initial flash.
It checks capture completion; inspect the PNGs for lighting appearance.
`tore-app` tests assert fire size, linear fade, fixed source height and sustained
fireball light lifetime. See [capture commands](../DEVELOPMENT.md#explosion-inspection).


### Theater ground colours

`render-terrain-colors` is a headless scenario, with no window or GPU. It
compares the live terrain loader's ground palette with the family read from
each retail base MM at all six weather choices and three altitudes. It prints
mean land-cell RGB for each comparison and fails on any palette mismatch.
[Diagnostic and optional offscreen captures](../DEVELOPMENT.md#ground-colour-regression).

## Human air target selection

`flight-sensor-target-select` flies a four-against-four Quick Mission through
T, Shift+T, radar off, Enter and Apostrophe. `net-window-target-select` repeats
those inputs on a private server with a bot, including opening and closing
chat. Each saves before/after HUD and radar captures and checks the next key's
authoritative selection to verify host acceptance and visual retention.
The multiplayer fixture starts enemies 20 nmi out, uses their existing dummy
flight mode and waits for host tick 6,000 before visual selection. Captures can
block the client long enough to reset its network baseline, so the script
waits 0.5 seconds after each shot for fresh contact lists. This avoids treating
snapshot recovery as a target-selection regression.

Run both with `python3 tools/battery.py --scenario '*target-select' --jobs 4
--windows 1 --profile <private-import> --keep-data`. The battery wraps windows
with `tools/agent-run.sh`. See the
[target-selection specification](../spec/radar.md#target-selection-keys).

Update (B2-5, 2026-11-15): the B-2's bay sweep (`bay-sweep.csv`, 8 values) joins `flight-animation-b2`
(the fitted doors: the belly polygon cut open and no lining at 0, 10 lining faces, a vertex on each
of the four hinge lines, free edges `leaf * sin(90 degrees * value)` below the door plane). New scenarios:
`flight-bay-fire-b2-row5` and `-row6` (the one-press release through the B-2's two Mk 82 bays),
`flight-bay-ripple-b2` (16 presses: the bays alternate, each empties its bottom tier first, front to
back; `TORE_STORES_PREVIEW_PRESSES`), `bay-b2-0`, `-0.5` and `-1` (the `--flight-bay` frames) and
`render-b2-bay-clearance` (headless: the default 64 Mk 82 clear the doors, ends, sloped roof and top skin by
at least 0.05 ft with the doors open, and no two touch).

Update (B2-5, 2026-11-15): the B-2's default load is the two Mk 82 bays alone (64 bombs, 32,000 lb), so
`flight-liftoff-b2` records 155.2 kt after 7,232 ft (was 158.1 kt) and `flight-approach-b2` 167.5 kt (was 166.2 kt);
the windows (140 to 165 kt and 140 to 170 kt) are unchanged.

Update (B1B-5, 2026-11-16): the B-1B's three fitted bays. `flight-animation-b1b`'s bay row now checks the door
leaves (belly polygons `3617` and `362e` whole and no lining at 0; 15 lining faces, a vertex on each bay's four
hinge corners and the lowest edge `3 units * sin(90 degrees * value)` below the belly plane; the old `7b16`
plates never drawn). `STATIONS["b1b"] = [4, 4, 4]` adds `flight-livefire-b1b-slot1` to `slot3` and
`flight-jettison-b1b-slot2` and `slot3` (one retarded Mk 82 seven-pack a slot). New scenarios:
`flight-bay-fire-b1b-row5`, `-row6` and `-row7` (the one-press release through each bay: the doors open to 0.95
before the pack leaves, then close), `flight-bay-ripple-b1b` (20 Mk 82 in the forward bay, 20 presses: one bomb a
press from the bottom tier up, outermost first alternating sides; the row's own release points),
`bay-b1b-0`, `-0.5` and `-1` (the `--flight-bay` frames), `render-b1b-clearance` (headless: 60 Mk 82, 20 a bay, clear
the door leaves, ends, roof and floor and each other by at least 0.05 ft with the doors open) and
`render-b1b-clearance-default` (the default 12 seven-packs, centred in the bays). `render-b1b-skin` keeps
`--loadout none` (its skin check needs no stores; the default load now starts since the start-fuel fix, 6f0277d8).

Update (B1B-6, 2026-11-16): new scenarios for the B-1B's ordnance. `a2g-release-origin-b1b` (six `--a2g-run --attack release` runs: each of the three bays' plain Mk 82 rows, 12
presses, the first nine places from the grid's cells, 4 abreast, outermost first alternating sides, 1.3 ft a tier; each bay's default seven-pack row, four packs of seven bombs from the
pack's own place and the retail pack stagger), `ai-strike-b1b-stick` (an AI B-1B, `--load MK82 --count 20` at 20,000 ft: a stick of eight 0.25 s apart from the same cells, the doors open at
every release) and `ai-strike-b1b-pack` (the default load: four packs of seven, the first bomb of each from the row's own place), `flight-countermeasures-b1b` (the retail `F18.ECM`: 30 chaff
and 30 flares, five and three released) and `flight-jettison-b1b-slot1` (the forward bay, which the generator's slot-2 start leaves out). No guided store is admitted on the bay rows, so
there is no `a2g-laser-b1b`.

Update (T160-5, 2026-11-16): the Tu-160's two fitted bays join the bay scenarios. New:
`flight-bay-fire-tu160-row4` and `-row5` (the one-press release of an AS-16 through each bay: the doors
open before the missile leaves and close after it), `flight-bay-tu160-0`, `-0.5` and `-1` (the
`--flight-bay` frames, from `bay-{ac}-{bay}`), `render-tu160-clearance` (headless: the default 12 AS-16
clear the open bays' walls, door leaves, ends, roof, floor and rail by at least 0.05 ft; the nearest-store
figure is informational), `flight-livefire-tu160-slot1` and `-slot2` and `flight-jettison-tu160-slot2`
(`STATIONS["tu160"] = [6, 6]`; the bay stations cannot be jettisoned: the scenario still fires them.
Corrected in T160-6, 2026-11-16: the AS-16 fired at the practice aircraft only while the Tu-160 had no
radar; with `TU160R.SEE` enabled both slots are refused slots, as the Su-25's). The probe's `bay` control is `Required` for the Tu-160
(`flight-animation-tu160`: no lining and nothing hanging below a bay's belly plane at 0; 20 lining faces,
a vertex on each hinge line and the free edge `3.8 ft * sin(90 degrees * value)` below the plane from a
sixth of the travel on, every leaf vertex on a hinge line at 1). The glove plates change the
`render-tu160-skin` top view only at the swept root.

Update (F111-5, 2026-11-16): the F-111F's stations. `f111` joins `STATIONS` (`flight-livefire-f111-slot1` to
`slot7`: the four wing rows, the bay and the two AIM-9 rails; the bay and rails start empty, and slot 3, the
AGM-65G row, is a refused surface-missile slot). New scenarios:

| Scenario | Does | Pass when |
| --- | --- | --- |
| `flight-bay-fire-f111-row9` | `--stores-preview` actual one-press release of the bay's two Mk 82 (`9=MK82.JT:2`, 20,000 lb of fuel) | the doors open to 0.95 before the bomb leaves and close after, `Stores firing: f111 row=9 PASS` |
| `flight-bay-fire-f111-pylon-row7` | the same from the fixed outer pylon (`7=MK82P.JT:2`) | a bomb leaves row 7 with the bay doors shut, `Stores firing: f111 row=7 PASS` |
| `render-f111-clearance` | headless `TORE_STORES_PREVIEW_BAYCLEAR` of the two Mk 82 in the open bay | at least 0.05 ft to the door planes, ends and roof, above the closed belly plane, 0.05 ft between the bombs |
| `a2g-release-origin-f111` | `--a2g-run` releases of the outer pylon's Mk 82 at 200 and 650 knots and of the default GBU-28 on the inner pylon | each store leaves from where it hangs: 15.46 ft out and 1.31 ft ahead at 200 knots, 15.28 ft out and 0.08 ft aft at 650 (the wing held to 26 degrees by the outer store); the GBU-28 8.30 / 2.38 and 8.41 / 2.22 |

Update (F111-6, 2026-11-16): three scenarios for the F-111F's sensors and guided stores.

| Scenario | Does | Pass when |
| --- | --- | --- |
| `a2g-laser-f111` | `--attack laser --aircraft f111 --load GBU28 --station 0` from 15,000 ft at 480 kt: the Pave Tack lases a tank, a GBU-28 leaves the inner wing pylon | Released guiding on aircraft 0's spot with the laser lasing, ends guiding within 40 ft, destroys the tank |
| `a2g-maverick-f111` | `--attack maverick --load AGM65G --station 2 --count 4` (the outer pylon's four AGM-65G, through the A-G radar channel) | The column's lead truck is killed (`hp 0/200`) |
| `flight-countermeasures-f111` | `--live-fire --aircraft f111`, five chaff and three flare commands | 30 chaff and 30 flares from the `A6.ECM` row, carried counts fall by the releases (the shared countermeasure check) |

Update (T26-5, 2026-11-16): the Tu-26's bay sweep (`bay` control of `flight-animation-tu26`) now checks the
fitted bay (17 belly faces whole at 0 with nothing under the keel line and no lining; open, 5 lining
faces, a vertex on each of the two hinge lines at the outer panels' plane, the lowest leaf point
`-3 - 2.2125 sin(a) - cos(a)` shape units). New scenarios: `flight-bay-fire-tu26-row5` (windowed, the
one-press release of eight Mk 82 through the Tu-26's bay: doors open before the bomb leaves, closed
after), `flight-bay-fire-tu26-pylon-row7` (a Mk 82 from the fixed glove with the doors shut),
`bay-tu26-0`, `-0.5` and `-1` (the `--flight-bay` frames) and `render-tu26-clearance` (headless: the four
Kh-15 clear the open bay's doors, walls, roof and floor by 0.05 ft, the six default Kh-15 clear the swing
panel at every sweep and the gear).

Update (T26-8, 2026-11-16): `tu26` joins `VARIETY_AIRCRAFT` and `LIFTOFF_AIRCRAFT` in `flight.py`
(`flight-variety-tu26`, `flight-liftoff-tu26`, `flight-liftoff-retail-tu26`, `flight-approach-tu26`) with
fitted windows (liftoff 150 to 205 kt, recorded 167.3; approach 150 to 175 kt, recorded 166.2; takeoff
run 3,500 to 5,500 ft and landing roll 3,500 to 5,000 ft through `LIFTOFF_RUN_FT` and `ROLLOUT_FT`), a
6,000 tick takeoff limit (`TAKEOFF_TICKS_MAX`), Athens (`THEATER_FOR`) and 60 percent fuel (66,138 lb,
`APPROACH_FUEL_SHARE`; at 65 percent the scripted flare touches down at 12.1 ft/s) for the approach. `ai-heavy-takeoff-tu26-gre-a1` joins the [AI lane](lane-ai.md); the landing twin was
not added (the AI Tu-26 lands short at Simferopol, see the review). Results: the
[variety flight baseline](../baselines/variety-flight.md#tupolev-tu-26-backfire-c-validation-2026-11-16).

Update (T95-5, 2026-11-16): the Tu-95's bay, pylons and tail turret join the flight lane.
`flight-bay-fire-tu95-row7` (the one-press release through the fitted bay with six Mk 82: the doors open
to 0.95 before the bomb leaves and close after it; the default AS-16 needs a target the stores preview does
not give it), `flight-bay-fire-tu95-pylon-row8` (an AS-16 leaves the underwing beam with the doors shut, ten
loaded), `flight-bay-tu95-0`, `-0.5` and `-1` (the `--flight-bay` frames) and `render-tu95-clearance`
(headless, `TORE_STORES_PREVIEW_T95CLEAR=1`: the six default AS-16 clear the open bay's doors, walls, roof,
floor, rails and ends by 0.05 ft or more, their fins touching each other being the one accepted exception;
ten pylon AS-16 clear each other, the inboard propeller disc, the nacelle, the fuselage and the gear).
`STATIONS["tu95"]` is `[2000, 6, 0]` (the tail turret, the six bay AS-16 and the empty pylon row), so
`flight-livefire-tu95-slot1` to `-slot3` and `flight-jettison-tu95-slot2` run too. `flight-animation-tu95`
now requires the bay control (the four belly triangles whole at 0 with no lining; open, the 20 lining
faces and the leaves hanging on a quarter turn). The menus expected line reads 5,365 weapon placements.

Update (T95-6/7, 2026-11-16): the Tu-95's ordnance, countermeasures and tail turret join the flight lane.
`STATIONS["tu95"]` is now `[2000, 6, 10]` (the pylon row starts loaded with ten AS-16), so
`flight-livefire-tu95-slot1` to `-slot3` and `flight-jettison-tu95-slot2` and `-slot3` run; slots 2 and 3 (the
bay's and the beams' AS-16, surface missiles) are in `REFUSED_SLOTS` (the Weapons window reads `TARGET TYPE`,
`NO LOCK`, nothing fires). New: `a2g-sarh-as16-tu95` (an AS-16 from the bay and one from the beam each designated
on the A-G page through the Bear's radar, launched at the lead truck of `~QUCOL` and killing it; the bay's leaves
at least half a second after the beam's, the door wait), `a2g-release-origin-tu95` (the human's presses leave from
the bay grid's cells and the beams' rows, each within 0.05 ft of where it hangs; the unit test
`an_ai_tu95_leaves_the_bay_and_the_beams_from_the_humans_places` pins the AI to the same cells),
`flight-tu95-tail-gun` (the B-52's scenario with an AI Tu-95: 5 of 180 rounds hit the F-22, the same 35 percent
ceiling) and `flight-countermeasures-tu95` (99 chaff and 99 flares, five and three released). The `menus` and
`a2g-catalog` expected lines read 22 recovered weapon stations (from 23).

Update (SU34-5, 2026-11-16): `su34` joins `STATIONS` (`[180, 4, 4, 2]`), so the slot and
jettison scenarios cover its gun, fuselage, wing and wingtip rows. New scenarios:
`flight-bay-fire-su34-pylon-row6`, `-row7` and `-row8` (windowed stores-preview fire: a Mk 82 or
GBU-12 leaves the intake, wing or tip station; nothing opens, the Su-34 has no bay), and
`render-su34-clearance` and `render-su34-clearance-mk82` (headless: the default and a Mk 82 load
clip the airframe by at most 0.75 ft, no two stores touch, and every store stays at least 1 ft
from the gear-down plates).

Update (SU34-6, 2026-11-16): new scenarios `a2g-laser-su34` (the Su-34's built-in laser: an AS-14 from
row 6 guides to the tank and destroys it, an AT-12 from row 7 guides and hurts it),
`a2g-release-origin-su34` (AS-14 front pair then rear pair, AT-12 outer then middle pylon, AA-11 tip
rails, LAU-61 rocket pods on the outer pylon, each from the cell it hangs at, left then right),
`ai-strike-su34-origin` (an AI Su-34's AS-14, AT-12 and LAU-61 leave from the human's cells, guide and
egress) and `flight-countermeasures-su34` (the retail `SU27.ECM` row, five chaff and three flare
releases). `menus-ordnance-availability` expects 5,361 weapon placements (the four rocket pods back on
row 7).

Update (SU34-8, 2026-11-16): `su34` joins `flight-variety`, `flight-liftoff`, `flight-liftoff-retail` and
`flight-approach` (liftoff window 110 to 140 kt, recorded 121.5 kt; approach window 115 to 145 kt, recorded
126.8 kt at 17,339 lb of fuel, 65 percent; the ordinary takeoff tick allowance; Simferopol), and
`ai-heavy-takeoff-su34-gre-a1` and `ai-heavy-land-su34-ukr-a2` join the AI lane (the heavy-class airfield
laws do not apply to the Su-34, whose flaps-up minimum is 119.8 kt; the scenarios pin that it takes off,
lands at 125 kt and parks cleanly). The windows are fitted agent decisions on no published speed.
