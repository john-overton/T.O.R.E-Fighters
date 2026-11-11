# Variety flight validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-05. The
[variety flight contract](../spec/variety-flight.md) owns the fitted rules and
numbers. Source build identities and dependency evidence live in the
[source registration](../formats/aircraft-variety.md#source-build-and-bounded-review).
The validation used Linux x86-64, Rust 1.91.1, repository base
`7d25975b8bde296442ecb873d256cc51864a2a4a` plus the uncommitted variety
implementation. No original executable or retail trajectory comparison was run.

## Source-backed acceptance

`cargo run --locked -p tore-sim --example variety_flight -- PT...` read the
23 exact PT files from the ignored local FA_2.LIB catalog. The command takes
any number of extracted PT paths and never embeds them. All seventeen new
conventional aircraft passed ten seconds of fixed-step flight at full power,
finite motion, fuel consumption and exact equality with an independently
stepped clone, then a gentle 2 ft/s touchdown at each revised gear clearance:
C130, AC130, E3, IL76, E2, MIG17F, F4B, F4J, F4E, F4,
A7, F15, F16C, F104, A10, B747 and A310.

AV8, YAK141, V22, AH64, MI24 and CH47 all passed ten seconds of level hover
at their own configured mass and calculated collective/power, within the
five-foot height bound. Each then passed a dry-runway vertical landing at
2 ft/s descent, takeoff after raising lift, stick-driven translation and yaw.
AV8, YAK141 and V22 additionally completed four seconds of held conversion
controls, reached forward actuator position, gained horizontal speed and
remained airborne with finite state. Each powered configuration also refused
restricted native activation through its source configuration check.
These checks establish working fitted behavior with reviewed data, not retail
parity. The logged results remain local in `.local/variety-flight-source-powered.log`.

## Synthetic behavior and recording checks

The powered flight tests cover all six aircraft: level hover within 0.01 feet
for ten seconds, climb/descent from a ten-percent lift change, engine shutdown,
cyclic/yaw response, gradual actuator travel, neutral vectoring, capability
gates, gentle landing/departure, payload and power limits, and a 44-second
forward-flight conversion that recovers vertical velocity. Exact full-state
serialization restores each aircraft and continues identical inputs for
1,200 more ticks. Conventional configuration tests cover all seventeen,
independent cloned tuning and source values, exact identities and separate
F-4 thrust/fuel configurations. Legacy flight ignores powered-lift demands.

All 46 `tore-replay` tests passed, including new actual-actuator and gun-pose key/delta
round trips and a synthetic format-1 file containing an original eleven-slot
key and gear delta. Format 2 stores the four additional actual positions with
the existing device precision; the format-1 reader supplies neutral extension
values. Signed gun heading/elevation and discrete group membership also round
trip. Invalid group masks are rejected in both keys and deltas. Export goldens
were refreshed for the deliberate format/device change.
The five `tore-world` snapshot tests passed with actual actuator interpolation
and the unchanged discrete-throttle rule. A further combat snapshot test passed
for combat-owned gun poses, linked membership and draw-only State clones.
The application frame-rebuild test passed with all twenty-two device slots.

The full simulation flight checkpoint passed 1,049 tests with one existing
ignored test. The frozen golden roster and the legacy AI defense fixture retain
their original fourteen identities; the new human flight families have their
own acceptance. No new autonomous behavior is covered or claimed. All 32
battery-selection tests passed with the variety scenario family registered.

Review fixes passed focused checks for final-mass/altitude hover initialization,
power-limited overload, V22 forward neutral conversion and an empty-loadout
HUD before its readout arrives. Revised source geometry clearances passed
landing probes for all 23 aircraft. V22 and CH47 GPU smoke captures confirmed
original rotor image-phase selection and the CH47 front mast placement, using
`tools/agent-run.sh` for workspace isolation. The images remain local in
`.local/variety-captures/*-material-fixed.png`.

Focused clippy for `tore-app`, `tore-world`, `tore-sim`, `tore-input` and `tore-replay`, all targets with
warnings denied, passed. The documentation check passed. Full workspace,
application rendering and broader acceptance are recorded by the parent
implementation pass; they are not implied by these focused results.

## Top speed pass, 2026-10-08

Implementation mode. Linux x86-64, branch `envelope-fixes` from
`import-variety` `f6b979a3`. The probe is
`cargo run --release --locked -p tore-sim --example envelope_probe -- PT...`
over the 23 local FA_2.LIB PT files. `PROBE_LEVEL=1` holds altitude and wings
level at full power (afterburner where fitted, unlimited fuel at full internal
fuel weight) until the speed settles; helicopters hold full forward stick with
collective holding altitude. `PROBE_PULLS=1` pulls full aft stick for three
seconds from level flight; `PROBE_AI=1` asks the AI control adapter to level
out of a dive. Knots are true airspeed. The fitted contract and sources are in
[top speeds](../spec/variety-flight.md#top-speeds).

### Pull near top speed

Before the fast-side hold, full aft stick at 1,000 ft gave the C-130 2.45 G at
70 percent of its top speed, 1.63 G at 85 percent and 1.00 G (0 deg/s pitch
rate) at 90 and 95 percent; the E-3 2.23 G to 90 percent and 1.49 G at
95 percent (and 1.00 G past 444 kt). The AI adapter flew a C-130 from 5,000 ft
in a 20-degree dive at 320 kt (95 percent) into the ground, and an E-3 at
440 kt from 12,000 ft and 20,000 ft past its top speed to an overspeed loss.
After it, the C-130 holds 1.63 G from 85 percent through top speed and the AI
recovers all of those cases. The F-16C and F/A-18D limits are unchanged
up to 90 percent of top speed at 1,000, 10,000 and 20,000 ft; the F-16C's last
band (above 752 kt at sea level) now gives 1.68 G instead of 1.00 G.
`flight-variety-gcurve-*` (seven transports and airliners) fail on the
`import-variety` binary (limit 0.94 to 1.00 G from about 450 kt) and pass
after.

### Level top speed, full power, full internal fuel

Decoded: the 1 G row's right edge. Simulated: the probe's settled speed.
Columns give sea level or 1,000 ft / 10,000 / 20,000 / 30,000 ft. "After"
includes the follow-up John asked for the same day: the E3 capped at the
707-300B VMO/MMO and the 0.96 level-speed fraction for the seven transports
and airliners ("heavy drag"). Each of those seven settles at 95.7 to 95.9
percent of its fitted edge at every altitude it holds; before the follow-up
they settled at 99.7 to 99.9 percent, inside the overspeed shake. The F-15,
F-16C and other fighters are unchanged, and the golden fingerprints are
identical before and after on Linux.

| Aircraft | Decoded top, kt (ceiling, ft) | Simulated before | Simulated after | Published | Result |
| --- | --- | --- | --- | --- | --- |
| C130 | 338 / 336 / 334 / 326 (34,000) | 337 / 335 / 333 / 326 | 324 / 322 / 320 / 313 | C-130H 320 at 20,000 ft | Edge left (4 percent); heavy drag |
| AC130 | 338 / 336 / 334 / 326 (34,000) | 337 / 335 / 333 / 325 | 249 / 248 / 247 / 241 | AC-130U 261 at sea level | Fitted; heavy drag |
| E3 | 462 / 462 / 462 / 438 (30,000) | 462 / 462 / 462 / - | 365 / 418 / 443 / - | 461 maximum; 707-300B VMO 375 KIAS at sea level | Fitted (707-300B analogue); heavy drag |
| IL76 | 462 at all (51,000) | 462 at all | 443 at all | 459 at 11,000 m | Edge left; heavy drag |
| E2 | 314 / 318 / 322 / 312 (31,000) | 314 / 318 / 322 / 312 | 301 / 305 / 309 / 299 | 325 to 350 | Edge left; heavy drag |
| AV8 | 581 / 576 / 571 / 567 (50,125) | 519 / 515 / 511 / 507 | same | 585 | Left (loaded drag) |
| YAK141 | 675 / 758 / 840 / 922 (50,000) | over its edge, 721 at 1,000 ft in a descent | 659 / 731 / 810 / 889 | 675 at sea level, 971 at 11,000 m | Model fix |
| V22 | 130 (7,000) | 127 at 1,000 ft | 270 / 261 / 252 / - (25,000) | 275 at sea level | Fitted |
| AH64 | 130 (7,000) | 37 at 1,000 ft | 154 at 1,000 ft, 146 at 5,000 | 158 | Fitted and model fix |
| MI24 | 178 (7,000) | 39 at 1,000 ft | 168 at 1,000 ft, 136 at 5,000 | 173 to 181 | Model fix |
| CH47 | 178 (7,000) | 39 at 1,000 ft | 164 at 1,000 ft, 139 at 5,000 | 170 | Model fix |
| MIG17 | 622 / 615 / 607 / 600 (55,000) | 561 / 555 / 548 / 541 | same | 618 at 3,000 m | Left (loaded drag) |
| F4B, F4J | 723 / 861 / 1005 / 1169 (42,000) | 695 / 813 / 948 / 1104 | same | 734 to 760 at sea level | Left |
| F4E | as F4B | 697 / 815 / 951 / 1107 | same | 786 at sea level, 1,280 at 40,000 ft | Left, 8 percent |
| F4G | as F4B | 702 / 820 / 957 / 1114 | same | about 1,243 at altitude | Left |
| A7 | 604 / 593 / 581 / 570 (42,000) | 555 / 545 / 535 / 524 | same | 600 at sea level | Left |
| F15 | 806 / 934 / 1081 / 1301 (100,000) | 750 / 856 / 990 / 1192 | same | about 782 at sea level, 1,434 at altitude | Left |
| F16C | 794 / 865 / 947 / 1071 (60,000) | 749 / 809 / 885 / 1001 | same | 795 at sea level, 1,147 to 1,176 at 40,000 ft | Left |
| F104 | 646 / 815 / 985 / 1154 (58,000) | 616 / 758 / 915 / 1073 | same | 996 to 1,154 at altitude | Left |
| A10 | 385 / 418 / 450 / - (22,812) | 347 / 373 / 402 / - | same | 381 at sea level | Left (loaded drag) |
| B747 | 492 at all (50,000) | 491 at all | 364 / 412 / 472 / 472 | VMO 375 KCAS, MMO 0.92 | Fitted; heavy drag |
| A310 | 456 / 468 / 479 / 491 (46,875) | 457 / 467 / 479 / 490 | 350 / 396 / 456 / 471 | VMO 360 KIAS, MMO 0.84 | Fitted; heavy drag |

The E3 at 30,000 ft, its 1 G ceiling, cannot hold level flight in the probe
before or after: no row above 1 G reaches 25,500 ft or more, so it has no pull
margin there and sinks into overspeed once disturbed. It holds level to
29,000 ft. Every other aircraft held its altitude and stayed under its fitted
top speed in level flight.

## MiG-25 validation, 2026-11-07

Implementation mode. The `M25.PT` record was read from the ignored local
FA_2.LIB catalog and the app profile is `.local/m3-aircraft/profile`. The
model is the default hybrid `--researched-flight` path; `--legacy-flight` keeps
the previous compatibility model and `--native-flight-tables` stays a separate
restricted path. The 1 G top speed is kept as decoded (no `fitted_envelopes`
entry), the G limits stay +5/-2, and the flight model uses the record's own
configuration.

`cargo run --locked -p tore-sim --example variety_flight -- .../M25.PT` printed
`aircraft=MiG-25 conventional=pass deterministic=pass landing=pass ticks=1200`.

`PROBE_LEVEL=1 PROBE_PULLS=1 PROBE_ALTS=1000,10000,20000,30000,36000
cargo run --locked -p tore-sim --example envelope_probe -- .../M25.PT` printed
the record's 1 G row (2,740 ft/s at 36,000 ft, 1,623 kt) and the following
level speeds. The full-stick pulls are at 0.50, 0.70, 0.85, 0.90 and 0.95 of
the 1 G edge, three seconds from level flight.

| Level top speed, full internal fuel | 1 G edge, kt | Simulated, kt | Ratio |
| --- | ---: | ---: | ---: |
| 1,000 ft | 679 | 611 | 0.901 |
| 10,000 ft | 922 | 830 | 0.901 |
| 20,000 ft | 1,192 | 1,073 | 0.901 |
| 30,000 ft | 1,461 | 1,316 | 0.901 |
| 36,000 ft | 1,623 | 1,462 | 0.900 |

| Full-stick pull | 1,000 ft | 10,000 ft | 20,000 ft |
| --- | --- | --- | --- |
| 0.50 of the edge | 3.89 G, 9.8 deg/s, limit 3.86 | 3.88 G, 7.0 deg/s, limit 3.86 | 3.10 G, 3.9 deg/s, limit 3.09 |
| 0.70 | 3.90 G, 6.9 deg/s, limit 3.86 | 3.88 G, 5.0 deg/s, limit 3.86 | 2.32 G, 1.7 deg/s, limit 2.32 |
| 0.85 | 3.90 G, 5.6 deg/s, limit 3.09 | 2.33 G, 1.9 deg/s, limit 2.32 | 2.32 G, 1.4 deg/s, limit 2.32 |
| 0.90 | 2.34 G, 2.4 deg/s, limit 2.32 | 1.55 G, 0.7 deg/s, limit 1.55 | 1.55 G, 0.6 deg/s, limit 1.55 |
| 0.95 | 1.56 G, 0.9 deg/s, limit 1.55 | 1.55 G, 0.7 deg/s, limit 1.55 | 1.55 G, 0.5 deg/s, limit 1.55 |

The peak G exceeds the limit printed at the start speed in the first two
bands because the pull decelerates the aircraft and the envelope's fast side
gives the higher row more pull. The G limits are the source's +5/-2; the
loaded 5 G row gives 3.86 G at the default gross weight.

The record's fuel flows are 2 lb/s at military power and 12 lb/s at full
afterburner. The 36,000-tick sprint run burned 3,600 lb in 300 s, which is
12 lb/s, and the 7,200-tick level run burned 84 lb in 60 s, 1.4 lb/s. The
flows are 6 and 2.8 times below the real R-15B-300's specific fuel
consumption (1.12 and 2.45 lb/(h lbf)); the record rule keeps them with the
tank, which is left as retail, so the tank's endurance is far longer than the
real aircraft's. The M25-0 audit lists the flows for John; no record value
changed.

Headless probes, all `--aircraft mig25 --no-audio`:

| Probe | Ticks | Max speed, kt | Max G | Fraction of the edge | Crashed |
| --- | ---: | ---: | ---: | ---: | --- |
| level | 7,200 | 517.4 | 1.01 | 0.654 | no |
| pull | 7,200 | 511.3 | 3.89 | 0.650 | no |
| loop | 4,635 | 597.1 | 3.91 | 0.824 | no |
| sprint | 36,000 | 714.6 | 1.00 | 0.908 | no |
| climb | 77,387 | 1,078.1 | 3.89 | 1.042 | overspeed loss |
| dive | 6,440 | 1,047.7 | 1.01 | 1.295 | overspeed loss |
| overspeed | 1,200 | 1,310.7 | 1.00 | 1.100 | overspeed loss |
| rudder | 360 | 511.3 | 1.01 | 0.650 | no |
| devices | 3,600 | 511.7 | 1.10 | 0.650 | no |
| stall-recover | 3,245 | 223.6 | 1.21 | 0.211 | no |
| spin-recover | 1,533 | 275.8 | 0.97 | 0.278 | no |

The stall recovery raised the alert at tick 2,524 and recovered at 3,004 with
0 ft of altitude lost. The spin recovery entered at tick 3, recovered at 1,292
after 0.66 revolutions, losing 2,328 ft. The overspeed probe is the expected
dive: it reached 1,310.7 kt, 1.10 times the 20,000 ft edge, and the overspeed
rule ended it.

Ground probes at `--theater UKR --ground-start 1`:

| Probe | Ticks | Result |
| --- | ---: | --- |
| takeoff | 2,776 | 100 ft reached; liftoff tick 1,952 at 153.9 kt over 2,161 ft, rotation 136.1 kt |
| takeoff-gear-airborne | 2,773 | 100 ft reached; same liftoff; gear raised after the wheels are clear |
| land | 12,458 | touchdown at 164.0 kt, 8.5 ft/s sink, 1,689 ft of runway left, stopped, score 100 |
| land-gear-up | 10,494 | crashed for the gear; no touchdown |
| land-hard | 12,197 | touchdown at 172.4 kt, 33.9 ft/s sink, stopped, 2,295 ft left |
| land-off-runway | 10,235 | crashed for the not-landable ground |
| nosewheel | 9,000 | no crash, 11.2 kt maximum |

The battery: `python3 tools/battery.py --scenario '*-mig25*' --lane flight
--profile .local/m3-aircraft/profile` passed 9 of 9 scenarios. It covers the
animation, the two live-fire slots, the slot-2 jettison, the two liftoff runs,
the approach, the variety flight and the skin render.

The liftoff and approach targets are fitted plausibility ranges from the
MiG-25PD manual quoted in the AviaSimHD MSFS manual: V1 300 km/h and V2 350
km/h at maximum takeoff weight (160 to 190 kt) and a 270 to 290 km/h landing
speed (146 to 157 kt). The model lifts off at 153.9 kt and approaches at
165.6 kt, both inside the ranges' plus or minus 10 percent window. The targets
are recorded as a regression check, not a retail figure.

Not run: `--native-flight-tables` needs the extracted sine and atan tables and
the path is not present locally; it remains a separate restricted research
path. The windowed scenarios ran through `tools/agent-run.sh` inside the
battery; no frame was captured for this slice because the plan asks for none.

## F-16A validation, 2026-11-08

Implementation mode, slice F16A-6. The `F16.PT` and `F16C.PT` records were read
from the ignored local FA_2.LIB catalog and the app profile is
`.local/m3-aircraft/profile`. The F-16A flies its own record on the default
hybrid `--researched-flight` path; `--legacy-flight` keeps the previous
compatibility model and `--native-flight-tables` stays a separate restricted
path. The record is identical to `F16C.PT` in every flight field, so the F-16A
has no `fitted_envelopes` entry and its numbers match the F-16C's.

`cargo run --locked -p tore-sim --example variety_flight -- F16.PT F16C.PT`
printed `aircraft=F-16 conventional=pass deterministic=pass landing=pass
ticks=1200` and the same line for `F-16C`.

`PROBE_LEVEL=1 PROBE_PULLS=1 PROBE_ALTS=1000,10000,20000,30000,40000,60000
cargo run --locked -p tore-sim --example envelope_probe -- F16.PT F16C.PT`
printed identical envelope, level-speed and pull lines for the two records:
`empty=14567 fuel=6972 mil=17687 ab=32000 loadedDrag=30 loadedElev=40
pullDrag=33 ceiling=60000`, the same 14 G rows and the same 1 G edge. Every
line below the `==` header is byte-identical between the two, so the table is
one aircraft's, shown once.

| Level top speed, full internal fuel | 1 G edge, kt | Simulated, kt | Ratio |
| --- | ---: | ---: | ---: |
| 1,000 ft | 801 | 749 | 0.935 |
| 10,000 ft | 865 | 809 | 0.935 |
| 20,000 ft | 947 | 885 | 0.935 |
| 30,000 ft | 1,071 | 1,001 | 0.935 |
| 40,000 ft | 1,113 | 1,040 | 0.935 |
| 60,000 ft | 777 | 716 | 0.921 |

The sea-level 1 G edge is 794 kt, 0.1 percent under the published 795 kt. The
1 G edge at 36,000 ft is 1,145 kt against the published 1,120 kt at 36,000 ft
(Aeropedia, Aero Web; the F-16A-0 published audit) and inside the 1,147 to
1,176 kt band at 40,000 ft: within 2 percent, so no fit is added.

| Full-stick pull | 1,000 ft | 10,000 ft | 20,000 ft |
| --- | --- | --- | --- |
| 0.50 of the edge | 7.66 G, limit 7.55 | 6.80 G, limit 6.71 | 5.09 G, limit 5.04 |
| 0.70 | 7.68 G, limit 7.55 | 6.81 G, limit 6.71 | 5.09 G, limit 5.04 |
| 0.85 | 3.41 G, limit 3.36 | 2.55 G, limit 2.52 | 2.54 G, limit 2.52 |
| 0.90 | 1.70 G, limit 1.68 | 1.70 G, limit 1.68 | 1.69 G, limit 1.68 |
| 0.95 | 1.70 G, limit 1.68 | 1.70 G, limit 1.68 | 1.69 G, limit 1.68 |

The peak G exceeds the limit printed at the start speed in the first two bands
because the pull decelerates the aircraft and the envelope's fast side gives
the higher row more pull, the same effect the MiG-25 shows.

Headless probes, all `--aircraft f16a --no-audio`, run against `--aircraft
f16c` on the same commands. Every air maneuver is identical for the two
identities apart from the `Aircraft:` name line, so the table is one
aircraft's.

| Probe | Ticks | Max speed, kt | Max G | Fraction of the edge | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| level | 7,200 | 539.1 | 1.02 | 0.650 | no crash |
| pull | 7,200 | 539.1 | 7.67 | 0.650 | no crash |
| loop | 2,533 | 560.1 | 7.74 | 0.678 | completed, no crash |
| sprint | 36,000 | 810.1 | 1.01 | 0.976 | no crash |
| climb | 115,370 | 839.5 | 7.63 | 1.099 | topped 69,706 ft (1.162 of ceiling), no crash |
| dive | 9,000 | 993.7 | 1.01 | 1.120 | overspeed loss |
| overspeed | 1,200 | 1,041.3 | 1.01 | 1.100 | overspeed loss |
| rudder | 360 | 539.1 | 1.01 | 0.650 | no crash |
| devices | 3,600 | 539.1 | 1.17 | 0.650 | no crash |
| stall-recover | 2,428 | 200.8 | 1.68 | 0.223 | recovered, no crash |
| spin-recover | 959 | 201.2 | 0.99 | 0.225 | recovered, no crash |

The stall recovery raised the alert at tick 1,465 and recovered at 2,187 with
0 ft of altitude lost and a minimum 102.3 kt. The spin recovery entered at
tick 3, recovered at 718 after 2.11 total revolutions (0.39 during recovery),
losing 902 ft. The overspeed and dive runs both end in the overspeed loss, the
intended end of those maneuvers, at 1.10 and 1.12 times the 20,000 ft edge.

Ground probes at `--theater UKR --ground-start 1` with the default load. The
seven lines differ from the F-16C's by the default ordnance weight only: the
F-16A carries 3,894 lb (25,433 lb gross) against 4,854 lb (26,393 lb gross).
With `--loadout none` the seven probes are byte-identical apart from the
`Aircraft:` line, so the difference is the F16A-4 station record, not a flight
field.

| Probe | Ticks | Result |
| --- | ---: | --- |
| takeoff | 982 | 100 ft reached; liftoff tick 500 at 96.2 kt over 343 ft, rotation 89.8 kt |
| takeoff-gear-airborne | 981 | 100 ft reached; same liftoff; gear raised after the wheels are clear |
| land | 18,150 | touchdown at 97.7 kt, 3.1 ft/s sink, stopped tick 17,909, 2,832 ft left, score 100 |
| land-gear-up | 14,534 | crashed for the gear; no touchdown |
| land-hard | 17,520 | touchdown at 109.7 kt, 33.3 ft/s sink, stopped, 3,741 ft left |
| land-off-runway | 16,449 | crashed for the not-landable ground |
| nosewheel | 9,000 | no crash |

The battery: `python3 tools/battery.py --scenario '*-f16a*' --lane flight
--profile .local/m3-aircraft/profile` passed 15 of 15 scenarios, run
`20261108-092857`: `flight-animation-f16a`, the five live-fire slots, the two
liftoff runs, the approach, `flight-variety-f16a`, the four jettison slots and
`render-f16a-skin`. The variety, liftoff, liftoff-retail and approach scenarios
are new in this slice; the F-16A has no per-aircraft speed target because its
record equals the F-16C's, which has none.

**Paths stay distinct.** The default prints `flight_model=hybrid`; the same
`--maneuver pull` under `--legacy-flight` prints `flight_model=legacy` and a
different flight (final 271.2 kt against the hybrid's 260.7 kt at 7,200 ticks,
energy rate 0.097 against 0.147). The `stall-recover` maneuver is a larger
difference: legacy recovered in 3,062 ticks with a 22.9 deg/s peak pitch rate,
hybrid in 2,428 ticks with 7.9 deg/s. `--native-flight-tables` needs the
extracted `sine-q15.bin` and `atan-pa.bin`, which are not present locally, so it
was not run; it stays a separate restricted research path.

Not run: `--native-flight-tables` (tables not present locally); no windowed
frame capture, because the plan asks for none and the render scenario ran
through the battery's own `tools/agent-run.sh` wrapper. No other AGENTS.md
check is part of this slice; F16A-8 runs the full list. No network, replay or
golden fixture changed.

## MiG-31 validation, 2026-11-08

Implementation mode, slice M31-8. The `MIG31.PT` record was read from the
ignored local FA_2.LIB catalog under `.local/m3-imports/scratch/mig31` and the
app profile is `.local/m3-aircraft/profile`. The MiG-31 flies its own record on
the default hybrid `--researched-flight` path; `--legacy-flight` keeps the
previous compatibility model and `--native-flight-tables` stays a separate
restricted path. The record's 1 G top speed is kept as decoded (no
`fitted_envelopes` entry), the G limits stay +4/-2 and no flight value is
changed. The default load is the record's own four AA-9s and two AA-8s.

`cargo run --locked -p tore-sim --example variety_flight --
.../MIG31.PT` printed `aircraft=MiG-31 conventional=pass deterministic=pass
landing=pass ticks=1200`. `PROBE_LEVEL=1 PROBE_PULLS=1
PROBE_ALTS=1000,10000,20000,30000,36000 cargo run --locked -p tore-sim --example
envelope_probe -- .../MIG31.PT` printed
`empty=48115 fuel=30865 mil=33510 ab=48500 loadedDrag=30 loadedElev=36
pullDrag=58 ceiling=68000`, the record's own 1 G row of 1,625 kt at 36,000 ft,
and the level speeds and full-stick pulls below.

| Level top speed, full internal fuel | 1 G edge, kt | Simulated, kt | Ratio |
| --- | ---: | ---: | ---: |
| 1,000 ft | 829 | 759 | 0.916 |
| 10,000 ft | 981 | 898 | 0.915 |
| 20,000 ft | 1,150 | 1,053 | 0.915 |
| 30,000 ft | 1,422 | 1,301 | 0.915 |
| 36,000 ft | 1,625 | 1,488 | 0.915 |

| Full-stick pull | 1,000 ft | 10,000 ft | 20,000 ft |
| --- | --- | --- | --- |
| 0.50 of the edge | 3.30 G, 6.3 deg/s, limit 3.25 | 3.29 G, 5.2 deg/s, limit 3.25 | 3.28 G, 4.4 deg/s, limit 3.25 |
| 0.70 | 3.29 G, 4.4 deg/s, limit 3.25 | 3.27 G, 3.7 deg/s, limit 3.25 | 3.27 G, 3.1 deg/s, limit 3.25 |
| 0.85 | 3.28 G, 3.6 deg/s, limit 3.25 | 2.45 G, 1.9 deg/s, limit 2.44 | 2.45 G, 1.6 deg/s, limit 2.44 |
| 0.90 | 2.46 G, 2.2 deg/s, limit 2.44 | 1.63 G, 0.8 deg/s, limit 1.62 | 1.63 G, 0.7 deg/s, limit 1.62 |
| 0.95 | 1.64 G, 0.9 deg/s, limit 1.62 | 1.63 G, 0.7 deg/s, limit 1.62 | 1.63 G, 0.6 deg/s, limit 1.62 |

The peak G exceeds the limit printed at the start speed in the first three bands
because the pull decelerates the aircraft and the envelope's fast side gives the
higher row more pull, the same effect the MiG-25 and F-16A show. The G limits
are the source's +4/-2; the loaded 4 G row gives 3.25 G at the default gross
weight.

The record's fuel flows are 6 lb/s at military power and 36 lb/s at full
afterburner. The climb burned the 30,865 lb tank dry over 167,963 ticks
(`fuel_end: internal_lb=0.0`) and the 36,000-tick sprint burned 10,800 lb in
300 s, 36 lb/s. They are far above what the real D-30F6's specific fuel
consumption allows, but the record rule keeps them with the tank, which is left
as retail; the M31-0 audit lists them for John and no record value changed.

Headless probes, all `--aircraft mig31 --no-audio`:

| Probe | Ticks | Max speed, kt | Max G | Fraction of the edge | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| level | 7,200 | 582.6 | 1.01 | 0.650 | no crash |
| pull | 7,200 | 582.6 | 3.31 | 0.650 | no crash |
| loop | 5,419 | 583.1 | 3.32 | 0.650 | completed, no crash |
| sprint | 36,000 | 842.3 | 1.00 | 0.940 | no crash |
| climb | 167,963 | 889.2 | 3.29 | 1.171 | topped 77,009 ft (1.132 of ceiling) on a dry tank, no crash |
| dive | 4,610 | 1,089.3 | 1.01 | 1.204 | overspeed loss |
| overspeed | 1,200 | 1,265.4 | 1.00 | 1.100 | overspeed loss |
| rudder | 360 | 582.6 | 1.01 | 0.650 | no crash |
| devices | 3,600 | 582.6 | 1.10 | 0.650 | no crash |
| stall-recover | 3,486 | 263.4 | 1.22 | 0.247 | recovered, no crash |
| spin-recover | 1,664 | 321.6 | 0.97 | 0.318 | recovered, no crash |

The stall recovery raised the alert at tick 2,765 and recovered at 3,245 with
0 ft of altitude lost and a minimum 231.2 kt. The spin recovery entered at tick
3, recovered at 1,423 after 0.73 revolutions during recovery (1.89 total),
losing 3,146 ft. The dive and overspeed runs both end in the overspeed loss, the
intended end of those maneuvers, at 1.20 and 1.10 times the 20,000 ft edge.

Ground probes at `--theater UKR --ground-start 1` with the default load:

| Probe | Ticks | Result |
| --- | ---: | --- |
| takeoff | 2,781 | 100 ft reached; liftoff tick 1,949 at 165.8 kt over 2,309 ft, rotation 146.3 kt |
| takeoff-gear-airborne | 2,778 | 100 ft reached; same liftoff; gear raised after the wheels are clear |
| land | 12,365 | touchdown at 167.9 kt, 12.0 ft/s sink, stopped tick 12,124, 1,709 ft of runway left, score 100 |
| land-gear-up | 10,356 | crashed for the gear; no touchdown |
| land-hard | 12,163 | touchdown at 174.6 kt, 34.1 ft/s sink, stopped, 2,182 ft left |
| land-off-runway | 10,061 | crashed for the not-landable ground |
| nosewheel | 9,000 | no crash, 11.2 kt maximum |

The battery: `python3 tools/battery.py --scenario '*-mig31*' --lane flight
--profile .local/m3-aircraft/profile` passed 11 of 11 scenarios, run
`20261108-160050`: `flight-animation-mig31`, `flight-liftoff-mig31`,
`flight-liftoff-retail-mig31`, `flight-approach-mig31`, `flight-variety-mig31`,
the three live-fire slots, the two jettison slots and `render-mig31-skin`. The
liftoff, liftoff-retail, approach and variety scenarios are new in this slice.

The liftoff range is the heavy-interceptor plausibility band (160 to 190 kt) and
the approach range is fitted from the published 280 km/h landing speed
(airbase.ru, MiG-31, read 2026-11-08), a plus or minus 5 percent window (265 to
296 km/h). The model lifts off at 165.8 kt and approaches at 166.2 kt, both
inside their ranges' windows, and the values are recorded as regression checks,
not retail figures.

**Paths stay distinct.** The default prints `flight_model=hybrid`; the same
`--maneuver pull` under `--legacy-flight` prints `flight_model=legacy` and a
different flight (final 140.6 kt against the hybrid's 246.6 kt at 7,200 ticks).
The `stall-recover` maneuver is a larger difference: the hybrid raised the alert
at tick 2,765 and recovered at 3,245, while legacy never raised one (0 alerts,
332 ft lost). `--native-flight-tables` needs the extracted `sine-q15.bin` and
`atan-pa.bin`, which are not present locally, so it was not run; it stays a
separate restricted research path.

Not run: `--native-flight-tables` (tables not present locally); no windowed
frame capture, because the plan asks for none and the render scenario ran
through the battery's own `tools/agent-run.sh` wrapper. No other AGENTS.md check
is part of this slice; M31-10 runs the full list. No network, replay or golden
fixture changed.

## Mirage 2000 validation, 2026-11-10

Implementation mode, slice M2K-8. The `M2000.PT` record was read from the
ignored local FA_2.LIB extraction under `.local/m3-imports/scratch/m2000` and
the app profile is `.local/m3-aircraft/profile`. The Mirage 2000 flies its own
record on the default hybrid `--researched-flight` path (its `variety` identity
is `("Mirage 2000", "M20.SH")`, M2K-1), with the TORE override of decision 2
(John, 2026-11-09): one engine, 14,455 lbf military and 21,380 lbf afterburner
thrust, applied where the record is read
([aircraft record corrections](../spec/aircraft-records.md)). `--legacy-flight`
and `--native-flight-tables` stay separate. The record's 1 G top speed, its
G rows and its ceiling are kept as decoded (no `fitted_envelopes` entry), and no
flight constant, thrust or weight was changed. Everything below is measured, not
fitted; the only fitted values are the two battery ranges at the end.

The record is the Rafale template (M2K-0): empty 17,100 lb, internal fuel
6,346 lb, maximum 37,480 lb, 1 G top speed 1,262 kt at 36,000 ft (693 kt at sea
level), ceiling 59,000 ft, G rows -4 to +8, military and afterburner fuel flows
2 and 16 lb/s. The default load weighs 27,007 lb (Load Ordnance page), so the
carried stores are 3,561 lb including 990 lb in the centerline tank.

**Harness.** `cargo run --locked -p tore-sim --example variety_flight --
.../M2000.PT` printed `aircraft=M2000 conventional=pass deterministic=pass
landing=pass ticks=1200`. `PROBE_LEVEL=1 PROBE_PULLS=1 PROBE_ALTS=...
envelope_probe` is the level top speed and full-stick pull probe used for every
aircraft above. The new `performance_probe` example (`tore-sim`, development
harness, no retail fixtures) adds what the earlier slices did not need: level
acceleration at military and afterburner power, an afterburner climb on a speed
schedule, sustained and instantaneous turns and fuel burn. `PROBE_PAYLOAD=3561`
adds the default stores, `PROBE_FUEL=pounds` sets the tank, `PROBE_ALTS` and
`PROBE_ONLY` (`accel`, `climb`, `turn`, `fuel`) pick the cases. Its pilots are
scripted (altitude hold, a fed-forward load factor in turns, a pitch law that
holds a true airspeed schedule in climbs), so its turn and climb figures are a
repeatable harness reading, not a best-pilot figure. All knots are true airspeed.
The logs are local: `.local/m3-aircraft/checks/M2K-8/`.

### Top speed and ceiling

| Altitude | 1 G edge (record), kt | Level, AB, clean, kt | Ratio | Level, AB, default load, kt | Level, military, clean, kt |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1,000 ft | 706 | 670 | 0.948 | 652 | 551 |
| 10,000 ft | 825 | 782 | 0.948 | | |
| 15,000 ft | | 845 (Mach 1.35) | | 822 | 695 |
| 20,000 ft | 967 | 917 | 0.948 | | |
| 30,000 ft | 1,151 | 1,092 (Mach 1.85) | 0.948 | 1,062 | 898 |
| 36,000 ft | 1,262 | 1,197 (Mach 2.09) | 0.948 | 1,164 | 984 (Mach 1.71) |
| 40,000 ft | 1,250 | 1,185 | 0.948 | | |
| 50,000 ft | 1,175 | 1,114 | 0.948 | | |

The sea-level edge at the airport (6 ft) is 693 kt. The loaded-drag rule
(`loadedDrag=30`) settles full internal fuel at 0.948 of the edge at every
altitude, inside the fighters' 87 to 96 percent band. The `climb` maneuver tops
out at 64,422 ft (1.092 of the 59,000 ft record ceiling) on a dry tank with a
peak speed of 878 kt, in the same range as the MiG-31 (1.132).

### Acceleration and climb (afterburner unless stated, full internal fuel)

Military power is the 14,455 lbf rating at full throttle with the burner switch
off; afterburner is 21,380 lbf. The drag law of the hybrid model reaches the
reference (afterburner) thrust at the top speed, so a thrust cut lowers
acceleration and climb in proportion and leaves the top speeds alone.

| Level acceleration | Time, s | Peak specific excess power |
| --- | --- | --- |
| 1,000 ft, AB, 363 to 460 / 590 / 660 kt | 10 / 31 / 72 | 418 ft/s (25,100 ft/min) |
| 1,000 ft, military, 363 to 460 / 530 kt | 20 / 57 | 213 ft/s |
| 15,000 ft, AB, 345 to 500 / 690 / 810 kt | 15 / 44 / 91 | 423 ft/s |
| 36,000 ft, AB, 316 to 520 / 800 / 1,030 / 1,150 kt | 23 / 62 / 119 / 195 | 435 ft/s |
| 36,000 ft, military, 316 to 520 / 800 / 980 kt | 36 / 113 / 383 | 240 ft/s |

| Afterburner climb from sea level | Speed schedule Mach 0.8 | Mach 0.9 |
| --- | --- | --- |
| 10,000 ft | 46 s | 63 s |
| 36,000 ft | 110 s | 129 s |
| 50,000 ft | 145 s | 163 s |
| 59,000 ft | 237 s | 192 s |
| Best sustained rate | 443 ft/s (26,600 ft/min) at 36,100 ft | 454 ft/s (27,200 ft/min) at 36,600 ft |

With the default 3,561 lb of stores the peak specific excess power falls to
349 to 366 ft/s (20,900 to 22,000 ft/min) and the Mach 0.8 climb takes 127 s to
36,000 ft. For the comparison with retail thrust the same harness ran the
Rafale record (`RAFALE.PT`: 32,000 lbf afterburner, 17,687 lbf military, two
engines) with the tank set to the Mirage's 6,346 lb, so the weight matches:
peak specific excess power 699 ft/s at 1,000 ft, best climb 609 to 615 ft/s,
36,000 ft in 81 to 93 s. The Mirage at the real M53-P2 thrust therefore has
about 60 percent of that acceleration and 74 percent of that climb rate.

### Turns and G limits

| Case (afterburner, full internal fuel) | Result |
| --- | --- |
| Full aft stick, 3 s, 1,000 ft, 330 kt | 6.17 G, 19.8 deg/s |
| Full aft stick, 1,000 ft, 461 kt (Mach 0.7) | 7.08 G, 16.2 deg/s |
| Full aft stick, 1,000 ft, 593 kt | 7.08 G, 12.3 deg/s |
| Full aft stick, 15,000 ft, 564 kt | 7.05 G, 12.8 deg/s |
| Full aft stick, 30,000 ft, 530 kt | 5.27 G, 9.3 deg/s |
| Full aft stick, 36,000 ft, 517 kt | 4.39 G, 7.4 deg/s |
| Full forward stick, 461 kt and up at 1,000 ft, 564 kt and up at 15,000 ft | -3.5 G |
| Same pulls with the default stores | 6.60 G (461 kt), -3.28 G |
| Sustained level turn, 1,000 ft, 461 kt | 4.13 G, 9.6 deg/s (bank 76 deg, speed held) |
| Sustained level turn, 15,000 ft, 564 kt | 3.63 G, 6.8 deg/s |
| Sustained level turn, 36,000 ft, 631 kt | 3.64 G, 6.1 deg/s |

The record's rows are -4 and +8 G. The loaded-elevator rule (40 percent
loaded elevator, the shared fighter rule) scales them to the gross weight, so
the pull probe peaks at 7.05 to 7.08 G on a clean aircraft and the full-stick
push at -3.5 G, as for the Rafale (7.39 G at 450 kt in the combat-G battery).
The sustained turns come from a scripted pilot that raises the bank in 2 degree
steps until speed or altitude is lost, so they are lower bounds. At 1,000 ft and
Mach 1.1 there is no sustained or instantaneous turn because that speed is
above the 706 kt edge.

### Fuel and endurance

The record's flows are 2 lb/s at military power and 16 lb/s at afterburner,
which is 7,200 and 57,600 lb/h, or 0.498 and 2.694 lb/lbf/h against the
override's thrust. The 6,346 lb tank lasts 52.9 minutes at military power and
6.6 minutes at afterburner (burn measured: 2.00 and 16.00 lb/s over 300 s). A
level cruise at 65 percent of the top speed at 20,000 ft (628 kt, throttle
trimmed) burns 1.39 lb/s, which is 75.8 minutes or 794 nautical miles; with the
default stores 71.8 minutes and 751 nautical miles. The `climb` maneuver burns
the tank dry in 998 s and the 300 s sprint burns 4,800 lb (16 lb/s).

### Takeoff, approach and landing

Ground probes at `--theater UKR --ground-start 1` with the default load (gross
27,007 lb plus fuel; `loading=0.579`, flapped stall 111.7 kt after the weight
scaling):

| Probe | Result |
| --- | --- |
| takeoff | liftoff tick 1,103 at 130.3 kt after 1,026 ft (313 m), rotation 121.6 kt; 100 ft reached at 180 kt |
| takeoff, `--retail-stall-speeds` | liftoff 105.8 kt after 670 ft (imported polygon speeds, no weight scaling) |
| takeoff-gear-airborne | same liftoff, gear raised after the wheels are clear (gear 0.53 at the end) |
| land, 65 percent fuel (4,125 lb) | approach 145.2 kt, touchdown 132.6 kt at 3.7 ft/s sink, stopped with 1,646 ft of the 5,532 ft runway left, score 100 |
| land, full fuel | touchdown 133.2 kt, 3.1 ft/s, 1,772 ft left, score 100 |
| land-hard | touchdown 151.4 kt at 33.6 ft/s, stopped, score 100 |
| land-gear-up, land-off-runway | crashed for the gear and for the not-landable ground, by design |
| nosewheel | no crash, 11.2 kt maximum |

The record has no arrestor hook (`hook_available=false`), so the hook slot of
the devices maneuver reports none.

Eleven air maneuvers all ended with `non_finite=0`: `level` 7,200 ticks (488.5
kt at 65 percent of the 5,000 ft edge), `pull` 7,200 (7.07 G, loop completed),
`loop` completed at 2,173 ticks (7.10 G), `sprint` 36,000 (747.2 kt, 0.984 of
the 5,000 ft edge), `climb` as above, `dive` and `overspeed` ending in the
overspeed loss (1.175 and 1.10 of the edge), `rudder`, `devices` (violations 0),
`stall-recover` (alert at tick 1,183, recovered at 1,717, 0 ft lost) and
`spin-recover` (entered at tick 3, recovered at 1,121 after 0.56 revolutions,
1,811 ft lost). The same `--maneuver pull` under `--legacy-flight` prints
`flight_model=legacy` and a different flight (219.7 kt against 162.7 kt at 7,200
ticks), so the two paths stay distinct.

### Against the real aircraft

Public figures for the Mirage 2000C, read 2026-11-10: Wikipedia, "Dassault
Mirage 2000" specifications (2000C/RDI: empty 7,500 kg = 16,535 lb, gross
13,800 kg = 30,424 lb, internal fuel 3,950 L, M53-P2 64.3 kN = 14,500 lbf dry and
95.1 kN = 21,400 lbf afterburner, Mach 2.2 = 1,261 kn at altitude, 1,110 km/h =
600 kn at sea level, ceiling 17,060 m = 55,970 ft, climb 285 m/s = 56,100 ft/min,
thrust to weight 0.7); MILAVIA ("Dassault Mirage 2000 Specifications": ceiling
54,000 ft, Mach 2.2 at 11,000 m); GlobalSecurity ("Mirage 2000 Specifications":
1,350 km/h = 729 kn at sea level; load factor limits +9 and -3.2 G, "Design and
Technical Features"); Dassault and airforce-technology.com (approach speed 259
km/h = 140 kt); HandWiki "Snecma M53" (specific fuel consumption 0.90 kg/daN/h
dry, 2.10 at afterburner). The sources disagree on the sea-level speed, the
ceiling and the G limits, and none of them is a flight manual.

| Quantity | Measured (clean unless stated) | Retail record | Real (cited) |
| --- | --- | --- | --- |
| Top speed at altitude | 1,197 kt at 36,000 ft (Mach 2.09) | 1,262 kt at 36,000 ft | 1,261 kn (Mach 2.2) |
| Top speed at sea level | 670 kt at 1,000 ft | 693 kt (706 at 1,000 ft) | 600 kn (most sources) to 729 kn (GlobalSecurity); pilot reports about Mach 1.1 |
| Service ceiling | 64,422 ft zoom peak | 59,000 ft | 54,000 to 55,970 ft |
| Rate of climb, sea level | 418 ft/s peak excess power (25,100 ft/min); 454 ft/s sustained (27,200 ft/min) | 17,687 / 32,000 lbf Rafale thrust would give 699 / 615 ft/s | 285 m/s = 933 ft/s (56,100 ft/min), test weight not stated |
| Thrust | 14,455 dry, 21,380 AB, 1 engine | 17,687 / 32,000, 2 engines | 14,500 / 21,400, 1 engine |
| Thrust to weight | 0.91 clean full fuel, 0.79 default load | 1.36 | 0.7 at 30,424 lb gross |
| G limits | +7.1 / -3.5 clean, +6.6 / -3.3 default load | +8 / -4 | +9 / -3.2 |
| Instantaneous turn | 16 to 20 deg/s at SL | | not found |
| Liftoff | 130.3 kt, 1,026 ft | | not found in a primary source |
| Approach | 145.2 kt at 65 percent fuel | | 140 kt (259 km/h) |
| Empty weight | 17,100 lb | 17,100 lb | 16,535 lb (+3.4 percent) |
| Internal fuel | 6,346 lb | 6,346 lb | about 6,970 lb (3,950 L at 0.8 kg/L; retail is 9 percent low) |
| Dry fuel flow | 7,200 lb/h (0.50 per lbf) | same | 0.90 per lbf, about 13,000 lb/h |
| Afterburner fuel flow | 57,600 lb/h (2.69 per lbf) | same | 2.10 per lbf, about 44,900 lb/h |
| Range, clean 6,346 lb | 794 nm cruise at 20,000 ft | | 840 nm (Wikipedia) |

Findings (no value was changed):

- The record's top speeds already match the real aircraft at altitude (1,262
  against 1,261 kn) and sit inside the published spread at sea level, so no
  envelope fit is needed, as for the MiG-25 and MiG-31. The fleet-wide loaded
  drag puts the level speed 5 percent under the edge.
- The empty weight is 565 lb over the real one and the tank 624 lb under it, so
  a clean full-fuel aircraft is within 60 lb of the real 23,500 lb. As fuel burns
  off the Mirage is up to 565 lb (3.4 percent) heavy, and a full tank holds 9
  percent less fuel than the real one.
- The acceleration and climb gap to the published 56,100 ft/min is not a thrust
  to weight problem: the thrust is the real thrust, the weights agree to 60 lb at
  full fuel, and at the real 30,424 lb gross the model has the same 0.70. The
  published climb is also for an unstated, probably lighter, weight. It is the drag law (drag equals the reference thrust at the top
  speed and grows with the square of speed, so at 460 kt and sea level the
  excess is about 60 percent of the thrust) and the lack of ram thrust gain with
  speed, both shared by the fleet. At the shared drag law the published 933 ft/s
  would need about 2.2 times the thrust, so raising thrust is not the lever, and
  it would also break decision 2.
- The record's fuel flows are not the M53-P2's: the dry flow is 45 percent
  under, the afterburner flow 28 percent over. At the real specific fuel
  consumption the same 6,346 lb tank would last about 29 minutes dry (the record
  gives 53) and 8.5 minutes at afterburner (the record gives 6.6). They stay retail (the record rule), as the MiG-25 and MiG-31 flows did.
- The model G limit (+7.1 clean) is 21 percent under the real +9 and the push
  limit -3.5 is 0.3 G beyond the real -3.2. Both come from the record's -4 and
  +8 rows and the shared loaded-elevator rule.

The liftoff range (125 to 150 kt) and the approach range (133 to 147 kt) in
`tools/battery_scenarios/flight.py` are fitted plausibility windows, agent
decisions of 2026-11-10. The approach range is the published 140 kt plus or
minus 5 percent. No primary takeoff figure was found, so the liftoff band brackets
the record's own 118.5 kt sea-level 1 G minimum and the 120 to 140 kt rotation
speeds of the FlightGear Mirage 2000 guide (a simulator manual, not a primary
source), and the measured 130.3 kt and 145.2 kt are recorded as regression
values (`KNOWN_LIFTOFF`, `KNOWN_APPROACH`).

The battery: `python3 tools/battery.py --scenario '*-m2000*' --lane flight
--profile .local/m3-aircraft/profile --windows 1` passed 16 of 16, run
`20261110-121406`: `flight-animation-m2000`, `flight-liftoff-m2000`,
`flight-liftoff-retail-m2000`, `flight-approach-m2000`, `flight-variety-m2000`,
the four live-fire and three jettison slots, the three `a2g-*-m2000`
scenarios and `render-m2000-skin`. The liftoff, liftoff-retail and approach
scenarios are new in this slice.

Not run: `--native-flight-tables` (tables not present locally, as for the MiG-31);
no windowed frame capture (the slice asks for none).

## Mirage 2000 positive G limit +9, 2026-11-10

Implementation and measurement mode, slice M2K-11. **Opinionated (John,
2026-11-10)**: the Mirage 2000's positive G limit is the real +9 instead of the
record's +8; the +9 polygon is **fitted** from the Mirage's own +8 row (slow-side
speeds times sqrt(9/8), fast side and altitudes from the +8 row, held to the 1 G
top speed), listed in [aircraft records](../spec/aircraft-records.md#mirage-2000-positive-g-limit-9-2026-11-10).
Nothing else changed: thrust, weights, drag and the -4 limit are as in the
validation above. The lines of that section are the pre-change measurement.

Probe: `performance_probe` on the local `M2000.PT`, clean, afterburner,
`PROBE_ONLY=turn PROBE_ALTS=1000`, `PROBE_FUEL` as shown; full aft stick for
three seconds at Mach 0.7 (461 kt) and 1,000 ft. Logs are local:
`.local/m3-aircraft/checks/M2K-11/`.

| Fuel, lb | Peak G before (+8 row) | Peak G now (+9 row) |
| ---: | ---: | ---: |
| 6,346 (full) | 7.08 | 7.96 |
| 3,173 | 7.57 | 8.52 |
| 1,587 | 7.85 | 8.83 |
| 300 | 8.09 | 9.09 |

Other points, full fuel: 15,000 ft at 564 kt 7.05 to 7.94 G; with the default
stores (3,561 lb) 6.60 to 7.42 G at 461 kt; 461 kt turn rate 16.2 to 19.1 deg/s.
Unchanged to the figure: the 330 kt pull (6.17 G), the 30,000
and 36,000 ft pulls (5.27 and 4.39 G, altitude limited), the push limits
(-3.5 G clean, -3.28 loaded) and every sustained turn (4.13 G at 461 kt and
1,000 ft, 3.63 G at 564 kt and 15,000 ft). At 300 lb the pull reaches 9.09 G, a
hair past the +9 row, the same overshoot the +8 row showed (8.09). The earlier
finding that the model G limit (+7.1 clean) was 21 percent under the real +9 is
now smaller: 7.96 G at full fuel (12 percent under), 8.5 at half fuel and 9.1
nearly dry, because the loaded-elevator rule still scales the limit down by the
fuel and stores carried.

Battery: `flight-variety-m2000`, `flight-liftoff-m2000`,
`flight-liftoff-retail-m2000`, `flight-approach-m2000`, `flight-animation-m2000`,
`ai-m2000-quick-mission-2v2`, `replay-rec-m2000-2v2` and the three
`a2g-*-m2000` scenarios passed (profile `.local/m3-aircraft/profile`). No combat-G
battery scenario pins the Mirage (`PULL_G_RETAIL` has no entry).

## Mirage F1 validation, 2026-11-11

Validation mode, slice MF1-8. The Mirage F1 flies the hybrid `--researched-flight`
default on its own record (`MF1.PT`, shape `mf1.SH`, `variety` identity from B5-1);
`--legacy-flight` and `--native-flight-tables` stay separate. The record is the
retail `MF1.PT` with John's B5-2 thrust correction (decision of 2026-11-10): one
Atar 9K-50 at 11,020 lbf military and 15,873 lbf afterburner (retail 15,000 and
30,000). Weights and fuel stay retail (empty 15,542 lb, internal fuel 6,120 lb,
maximum takeoff 30,203 lb, all copied from the Mirage III record), as do the
Mirage F1's own G rows -3 to +7, the 66,125 ft ceiling and the 1,345 kt top row.
**Nothing was fitted**: no value, envelope or constant changed. The only fitted
values are the two battery windows at the end.

**Which numbers depend on the open stations question.** The record's default load
(MF1-5, stopped on clipping, John's call) is the DEFA gun with 250 rounds, four
belly `F250.GAS` tanks and two `R550.JT`, with the centreline empty (B5-2). It
weighs 29,839 lb (`loadout:` line: 6,600 lb of tank fuel plus 1,577 lb of other
stores on top of the 21,662 lb clean aircraft), 99 percent of the 30,203 lb
maximum, so it changes everything that depends on weight. Every table below is
therefore given twice, **clean** (`--loadout none` on the ground, no payload in
the probes, 21,662 lb) and **default load** (the ground runs as they stand;
`PROBE_PAYLOAD=8177`, the whole carried 8,177 lb as dead mass, which also counts
the tank fuel as mass that does not burn). The clean figures do not depend on the
stations answer. The default-load figures, the battery's liftoff and approach
windows and their recorded values, and the endurance with tanks all do: if John
caps the belly row at two tanks or drops it, those numbers and the windows move
(an in-between load, never the clean figures, unless the default is cleared).

**Harness.** As for the Mirage 2000, the Gripen and the F-5E: `envelope_probe`
(`PROBE_POINTS=1 PROBE_LEVEL=1 PROBE_PULLS=1`) and `performance_probe`
(`PROBE_ALTS=1000,15000,30000,36000,50000,60000`; `PROBE_PAYLOAD=8177` for the
default load; `PROBE_ONLY=turn PROBE_ALTS=1000 PROBE_FUEL=...` for the pull by fuel)
on the local `MF1.PT`; `variety_flight` printed `aircraft=MF1 conventional=pass
deterministic=pass landing=pass ticks=1200`; `target/debug/tore-app --aircraft mf1
--headless-flight N --maneuver ...` with the profile
`.local/m3-aircraft/profile-mf1`. Pilots are scripted, so turn and climb figures are
a repeatable harness reading, not a best-pilot figure. All knots are true airspeed.
Logs are local: `.local/m3-aircraft/checks/MF1-8/`.

### Top speed and ceiling

| Altitude | 1 G edge (record), kt | Level, AB, clean, kt | Ratio | Level, AB, default load, kt | Level, military, clean, kt |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1,000 ft | 806 | 762 (Mach 1.16) | 0.945 | 714 (Mach 1.08) | 635 (Mach 0.96) |
| 10,000 ft | 866 | 818 | 0.945 | | |
| 15,000 ft | | 850 (Mach 1.36) | | 795 (Mach 1.27) | 708 (Mach 1.13) |
| 20,000 ft | 966 | 913 | 0.945 | | |
| 30,000 ft | 1,203 | 1,137 (Mach 1.93) | 0.945 | 1,064 (Mach 1.81) | 947 (Mach 1.61) |
| 36,000 ft | 1,345 | 1,271 (Mach 2.22) | 0.945 | 1,190 (Mach 2.07) | 1,059 (Mach 1.85) |
| 40,000 ft | 1,314 | 1,242 | 0.945 | | |
| 50,000 ft | 1,237 | 1,170 | 0.945 | | |
| 60,000 ft | 1,046 | 989 | 0.945 | | |
| 66,000 ft | 881 | 832 | 0.945 | | |

Level flight is held at every altitude to the 66,125 ft ceiling (`held=true` up
to 66,000 ft). The shared loaded-drag rule settles full internal fuel at 0.945 of
the edge, inside the fighters' 87 to 96 percent band. The sea-level edge at the
airport (6 ft) is 800 kt. The scripted `climb` maneuver tops out at 63,914 ft
(0.967 of the ceiling, 771.5 kt) and then the tank runs dry (the afterburner
burns 23 lb/s, so 6,120 lb lasts 266 s); the performance probe's zoom climbs
reach 69,360 ft (Mach 0.8 schedule) and 68,725 ft (Mach 0.9).

### Acceleration and climb (afterburner unless stated, full internal fuel)

Clean weight 21,662 lb. Thrust to weight is 0.73 with the afterburner (15,873
lbf), 0.51 at military power, and 0.53 with the 29,839 lb default load.

| Level acceleration | Time, s | Peak specific excess power |
| --- | --- | --- |
| 1,000 ft, AB, 363 to 460 / 590 / 660 / 730 kt | 11 / 29 / 45 / 78 | 376 ft/s (22,600 ft/min) |
| 1,000 ft, military, 363 to 460 / 530 / 590 kt | 18 / 37 / 67 | 215 ft/s |
| 15,000 ft, AB, 345 to 500 / 690 / 810 kt | 19 / 53 / 109 | 338 ft/s |
| 36,000 ft, AB, 316 to 520 / 800 / 1,030 / 1,150 kt | 28 / 75 / 133 / 190 | 369 ft/s |
| 36,000 ft, military, 316 to 520 / 800 / 980 kt | 43 / 124 / 241 | 213 ft/s |

| Afterburner climb from sea level | Speed schedule Mach 0.8 | Mach 0.9 |
| --- | --- | --- |
| 10,000 ft | 46 s | 58 s |
| 36,000 ft | 121 s | 135 s |
| 50,000 ft | 166 s | 177 s |
| 59,000 ft | 228 s | 212 s |
| Best sustained rate | 360 ft/s (21,600 ft/min) at 32,200 ft | 375 ft/s (22,500 ft/min) at 36,300 ft |

With the default load the peak specific excess power falls to 228 to 252 ft/s
(15,100 ft/min at 1,000 ft) and the Mach 0.8 climb takes 61 s to 10,000 ft and
170 s to 36,000 ft. The load's climb readings above 50,000 ft (a "best sustained
rate" of 728 ft/s at 61,400 ft, a Mach 0.9 schedule that never leaves the runway
height) are a harness limit with a payload of that size, not a flight finding, and
are not used. The Mirage F1's clean thrust to weight (0.73) is below the Mirage
2000's 0.91 (peak excess power 418 ft/s) and the Gripen's 0.92 (445 ft/s), about
the F-5E's 0.72 (332 ft/s), and far below the F-16A's (789 ft/s); its 376 ft/s sits
between the F-5E and the Mirage 2000, as the thrust to weight does.

### Turns and G limits

| Case (afterburner, full internal fuel) | Clean | Default load |
| --- | --- | --- |
| Full aft stick, 3 s, 1,000 ft, 330 kt | 6.12 G, 18.1 deg/s | 5.17 G, 14.7 deg/s |
| Full aft stick, 1,000 ft, 461 kt (Mach 0.7) | 6.14 G, 13.4 deg/s | 5.18 G, 10.8 deg/s |
| Full aft stick, 1,000 ft, 593 kt | 6.13 G, 10.2 deg/s | 5.17 G, 8.2 deg/s |
| Full aft stick, 1,000 ft, 725 kt (Mach 1.1) | 6.12 G, 8.3 deg/s | 5.17 G, 6.7 deg/s |
| Full aft stick, 15,000 ft, 439 / 564 kt | 5.25 G, 11.5 deg/s / 6.12 G, 10.8 deg/s | 4.43 G / 5.17 G |
| Full aft stick, 30,000 ft, 413 / 530 kt | 3.49 G, 6.8 deg/s / 4.36 G, 7.2 deg/s | 2.95 G / 3.68 G |
| Full aft stick, 36,000 ft, 517 kt | 3.48 G, 5.4 deg/s | 2.95 G, 4.2 deg/s |
| Full forward stick, 461 kt at 1,000 ft | -2.61 G | -2.20 G |
| Sustained level turn, 1,000 ft, 461 kt | 3.63 G, 8.1 deg/s (bank 74 deg) | 2.93 G, 6.6 deg/s |
| Sustained level turn, 1,000 ft, 593 kt | 2.68 G, 4.5 deg/s | 2.00 G, 3.2 deg/s |
| Sustained level turn, 15,000 ft, 439 kt | 3.63 G, 8.7 deg/s | 2.67 G, 6.0 deg/s |
| Sustained level turn, 15,000 ft, 564 kt | 2.93 G, 5.3 deg/s | 2.29 G, 4.0 deg/s |
| Sustained level turn, 30,000 ft, 530 kt | 3.24 G, 6.3 deg/s | 2.46 G, 4.5 deg/s |
| Sustained level turn, 36,000 ft, 517 kt | 3.24 G, 6.6 deg/s | 2.46 G, 4.6 deg/s |

The record's top row is +7 (rows -3 to +7). The loaded-elevator rule scales the
limit to the gross weight, so the clean aircraft pulls 6.1 G at sea level and the
default load (99 percent of the maximum weight) 5.2 G. The G rows keep their
strength at speed: 6.1 G holds from 330 kt to Mach 1.1 at 1,000 ft and to 564 kt
at 15,000 ft (the +7 row ends at 729 kt at sea level), unlike the F-5E's rows. The
sustained turns come from a scripted pilot that raises the bank in 2 degree steps
until speed or altitude is lost, so they are lower bounds, and their steps
are coarse (the same bank gives the same G at nearby fuel loads). The B5-2 note
stands: the 461 kt sustained turn fell from 5.76 G to 3.63 G (12.7 to 8.1 deg/s)
with the real thrust. At 1,000 ft and Mach 1.1 (725 kt, just under the 806 kt edge) the clean aircraft
sustains only 1.56 G (1.8 deg/s), and the default load none.

**Full-aft-stick pull by fuel**, 461 kt (Mach 0.7), 1,000 ft, afterburner, 3 s
(`PROBE_ONLY=turn PROBE_ALTS=1000 PROBE_FUEL=...`), against the F-5E (the F5E-8
table), the Gripen (GRI-8), the F-16A and the Mirage 2000 (the M2K-11 table), all
re-run on this tree:

| Fuel | Mirage F1 clean G (deg/s) | Mirage F1 default load G (deg/s) | Mirage 2000 G (deg/s) | Gripen G (deg/s) | F-5E G (deg/s) | F-16A G (deg/s) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Full (6,120 / 6,346 / 5,000 / 4,400 / 6,972 lb) | 6.14 (13.4) | 5.18 (10.8) | 7.96 (19.1) | 8.00 (18.6) | 5.98 (12.8) | 7.67 (16.8) |
| Half (3,060 / 3,173 / 2,500 / 2,200 / 3,486 lb) | 6.59 (14.6) | 5.50 (11.7) | 8.52 (20.7) | 8.52 (20.0) | 6.49 (14.2) | 8.35 (18.3) |
| Quarter (1,530 / 1,587 / 1,250 / 1,100 / 1,743 lb) | 6.84 (15.3) | 5.68 (12.2) | 8.83 (21.6) | 8.80 (20.7) | 6.78 (14.9) | 8.74 (19.1) |
| Near empty (300 lb) | 7.06 (15.9) | 5.83 (12.6) | 9.09 (22.3) | 9.03 (21.3) | 7.00 (15.5) | 9.09 (19.8) |
| Full forward stick, same four | -2.61 / -2.80 / -2.91 / -3.00 | -2.20 / -2.34 / -2.41 / -2.48 | -3.52 / -3.76 / -3.90 / -4.02 | -3.55 / -3.78 / -3.90 / -4.01 | -2.55 / -2.77 / -2.89 / -2.99 | -3.39 / -3.70 / -3.87 / -4.02 |

(The default-load column keeps the 8,177 lb payload as dead mass at all four
fuel states; with real tanks the external fuel would burn off first.) The pull
rises as the tank empties for every aircraft, the shared loaded-elevator rule
scaling the limit with the fuel and stores carried. The Mirage F1's clean full-fuel
pull (6.14 G) is the F-5E's (5.98 G) plus a little, and 23 percent under the Mirage
2000 and the Gripen, whose rows are +9; as a fraction of its own top row it is 88
percent (the Mirage 2000's 88, the Gripen's 89, the F-5E's 85 and the F-16A's
85 percent). Near empty the pull reaches 7.06 G, 0.06 G past the +7 row, the same
small overshoot the +9 aircraft show (9.03 to 9.09). The real limit is +7.5 G
(MIGFlug), 0.5 G above the top row and 1.4 G above the clean full-fuel pull. The
push is -2.61 G (full) to -3.00 G (near empty) against the -3 row, because the -3
row's slow edge is 475 kt at sea level.

### Fuel and endurance

The record's flows are 3 lb/s at military power and 23 lb/s at afterburner,
10,800 and 82,800 lb/h, or 0.98 and 5.22 lb/lbf/h against the corrected thrust
(11,020 and 15,873 lbf). The 6,120 lb tank lasts 34.0 minutes at military power
and 4.4 minutes at afterburner (burn measured: 3.00 and 23.00 lb/s over 300 s).
A level cruise at 65 percent of the top speed (627 kt at 1,000 ft, throttle
trimmed) burns 2.05 lb/s, which is 49.8 minutes or 520 nautical miles clean
(extrapolated from 300 s); with the default load's mass 2.34 lb/s, 43.5 minutes
and 455 nautical miles on the internal tank alone (the 6,600 lb of tank fuel is
extra range the probe does not count). The `sprint` maneuver burns the whole tank
(6,120 lb in 266 s at afterburner).

### Takeoff, approach and landing

Ground probes at `--theater UKR --ground-start 1`. **Default load**: gross 29,839
lb (maximum 30,203 lb), `loading=0.920`, flapped stall 104.7 kt after the weight
scaling. **Clean** (`--loadout none`): gross 21,662 lb, `loading=0.394`, flapped
stall 89.2 kt.

| Probe | Default load | Clean |
| --- | --- | --- |
| takeoff | liftoff tick 1,721 at 135.4 kt after 1,655 ft (504 m), rotation 116.9 kt | tick 937 at 103.9 kt after 691 ft (211 m), rotation 97.7 kt |
| takeoff, `--retail-stall-speeds` | liftoff 100.4 kt after 895 ft (imported polygon speeds, no weight scaling) | 89.5 kt after 512 ft |
| land, 65 percent fuel (3,978 lb) | approach 145.6 kt, touchdown 135.9 kt at 3.5 ft/s sink, stopped 1,917 ft (584 m) after touchdown with 1,738 ft of the 5,532 ft runway left, score 100 | approach 115.9 kt, touchdown 107.7 kt at 2.7 ft/s, stopped 1,233 ft (376 m) after touchdown, 2,382 ft left |
| land, full internal fuel | touchdown 137.8 kt, 5.2 ft/s, stopped 1,963 ft (598 m) after touchdown, 1,883 ft left | touchdown 109.5 kt, 2.8 ft/s, stopped 1,259 ft (384 m), 2,519 ft left |
| land-hard | touchdown 149.6 kt at 33.5 ft/s, stopped, score 100 | not run |
| land-gear-up, land-off-runway | crashed for the gear and for the not-landable ground, by design | not run |
| nosewheel | no crash, 11.2 kt maximum | not run |

The takeoff-gear-airborne probe ran with the same liftoff. The record has no
arrestor hook (`hook_available=false`). The scripted approach speed does not
change with the `--flight-fuel` value (145.6 kt at both fuel states), as for the
other aircraft; it is flown at 1.3 times the weight-scaled stall speed.

Eleven air maneuvers: nine ended with `non_finite=0` and no loss (`level` 7,200
ticks at 541.8 kt, 65 percent of the 5,000 ft edge; `pull` 6.13 G with the loop
completed; `loop` completed at 2,630 ticks, 6.20 G; `sprint` 36,000 ticks, 828.5
kt, 0.995 of the edge, tank empty at the end; `climb` as above, `crashed=false`,
fuel empty; `rudder`; `devices` violations 0; `stall-recover` alert at tick
1,539, recovered at 2,037, 0 ft lost; `spin-recover` entered at tick 3, recovered
at 1,132 after 0.58 revolutions of recovery and 2.12 in all, 1,827 ft lost);
`dive` and `overspeed` ended in the overspeed loss by design (1.145 and 1.100 of
the edge). Logs under `.local/m3-aircraft/checks/MF1-8/probes/` and `ground/`.

### Against the real aircraft

Real figures read 2026-11-11. The first column of cited facts is the game's own
encyclopedia (`MF1.INF`, Jane's All The World's Aircraft text, F1-C) which is
reference media, not original behaviour: empty 16,314 lb (7,400 kg), clean takeoff
24,030 lb, maximum 35,715 lb, 4,300 L internal fuel, Mach 2.2 at altitude (data
line 1,266 kt), Mach 1.2 (800 kt, 920 mph EAS) at low altitude, maximum climb at sea
level with afterburning 12,780 m/min (41,930 ft/min), service ceiling 65,600 ft,
stabilised supersonic ceiling 52,500 ft, takeoff run 600 m, landing run 670 m. Web
sources: MIGFlug, "Dassault Mirage F1" (+7.5 / -3.6 G, rate of climb 243 m/s = 47,800
ft/min, approach about 125 kt, thrust to weight 0.66 at clean takeoff weight, 49.0
kN = 11,020 lbf dry and 70.6 kN afterburner, internal fuel about 4,300 L = 7,385 lb);
MILAVIA ("Mirage F1 specifications": Mach 2.2 at 12,000 m, ceiling 65,615 ft, 70.21
kN); Wikipedia, "Dassault Mirage F1" (rate of climb 243 m/s, thrust 15,873 lbf
afterburner); GlobalSpec's Atar datasheet and Wikipedia's thrust specific fuel
consumption table (Atar 9K-50: about 0.96 to 0.98 lb/lbf/h dry, 1.96 to 1.99
afterburning). No primary flight manual was found; the sources agree on everything
except the sea-level climb, where the encyclopedia and MIGFlug differ by 14 percent.

| Quantity | Measured clean | Measured default load | Retail record | Real (cited) |
| --- | --- | --- | --- | --- |
| Top speed at 36,000 ft | 1,271 kt (Mach 2.22) | 1,190 kt (Mach 2.07) | 1,345 kt (Mach 2.35) | Mach 2.2, 1,266 kt data line (Jane's text, MILAVIA, MIGFlug) |
| Top speed at sea level | 762 kt at 1,000 ft (Mach 1.16) | 714 kt (Mach 1.08) | 800 kt (806 at 1,000 ft) | 800 kt, Mach 1.2 (Jane's text); 913 mph = 793 kt (MIGFlug) |
| Service ceiling | level flight held at 66,000 ft; scripted climb 63,914 ft; zoom 68,700 to 69,400 ft | | 66,125 ft | 65,600 ft (Jane's text), 65,615 ft (MILAVIA) |
| Rate of climb, sea level | 376 ft/s peak excess power (22,600 ft/min); 360 to 375 ft/s sustained (21,600 to 22,500 ft/min) | 252 ft/s (15,100 ft/min) | 15,000 / 30,000 lbf thrust would give more | 41,930 ft/min (Jane's text); 47,800 ft/min (MIGFlug, Wikipedia); test weight not stated |
| Thrust | 11,020 dry, 15,873 AB, 1 engine | same | 15,000 / 30,000 | 11,020 / 15,873, 1 engine |
| Thrust to weight | 0.73 clean full fuel | 0.53 | | 0.66 at 24,030 lb (MIGFlug) |
| G limits | +6.1 (+7.1 nearly dry) / -2.6 clean at 461 kt | +5.2 / -2.2 | +7 / -3 | +7.5 / -3.6 (MIGFlug) |
| Instantaneous turn | 18.1 deg/s at 330 kt (6.12 G); 13.4 deg/s at 461 kt | 14.7 / 10.8 deg/s | | not found |
| Sustained turn (scripted, lower bound) | 3.63 G, 8.1 deg/s at 461 kt, 1,000 ft | 2.93 G, 6.6 deg/s | | not found |
| Liftoff / takeoff run | 103.9 kt, 691 ft (211 m) at 21,662 lb | 135.4 kt, 1,655 ft (504 m) at 29,839 lb | | takeoff run 600 m (Jane's text; weight not stated) |
| Approach / landing run | 115.9 kt approach, 107.7 kt touchdown, 376 m rollout | 145.6 kt approach, 135.9 kt touchdown, 584 m rollout | | about 125 kt approach (MIGFlug); landing run 670 m (Jane's text; weight and brake parachute not stated) |
| Empty weight | 15,542 lb | same | 15,542 lb | 16,314 lb (retail 4.7 percent under) |
| Internal fuel | 6,120 lb | same | 6,120 lb | 4,300 L, about 7,385 lb (retail 17 percent under) |
| Clean takeoff weight | 21,662 lb | | | 24,030 lb (model 10 percent under) |
| Maximum takeoff | 30,203 lb | | 30,203 lb | 35,715 lb (retail 15 percent under) |
| Dry fuel flow | 10,800 lb/h (0.98 per lbf) | same | same | 0.96 to 0.98 per lbf, 10,600 to 10,800 lb/h |
| Afterburner fuel flow | 82,800 lb/h (5.22 per lbf) | same | same | 1.96 to 1.99 per lbf, about 31,100 to 31,600 lb/h (8.6 to 8.8 lb/s) |
| Endurance, internal tank | 34.0 min military, 4.4 min AB; cruise 49.8 min, 520 nm | 43.5 min cruise | | combat air patrol 2 h 15 min with two missiles and a 1,200 L centreline tank (MIGFlug) |

Findings (no value was changed):

- **Top speeds are the real ones.** The model flies Mach 2.22 at 36,000 ft against
  the published Mach 2.2 (1,271 kt against the encyclopedia's 1,266 kt data line,
  0.4 percent over) and 762 kt at 1,000 ft against about 800 kt (4.8 percent
  under, 806 kt on the record's own edge). No `fitted_envelopes` entry is
  needed. With the default load the speeds fall by 6 to 7 percent (1,190 kt,
  714 kt).
- **The ceiling matches.** Level flight is held to the record's 66,125 ft against
  the published 65,600 ft (0.8 percent), and the scripted climb reaches 0.967 of
  it before the tank runs dry.
- **Climb is about half the published figure, and the cause is the shared drag
  law, not the thrust.** The model's sea-level peak specific excess power is 376
  ft/s (22,600 ft/min) and its best sustained climb 360 to 375 ft/s (21,600 to
  22,500 ft/min): 54 percent of the encyclopedia's 41,930 ft/min and 47 percent of
  the 47,800 ft/min of MIGFlug and Wikipedia. The thrust is the real 15,873 lbf,
  the clean thrust to weight (0.73) is above the published 0.66, and the weights
  are within 10 percent. The cause is the one found for the Mirage 2000 (45
  percent) and the F-5E (55 to 58 percent): drag equals the reference thrust at the
  top speed and grows with the square of speed, and no ram thrust gain is
  modelled. Reported, not changed (a per-aircraft drag shape or ram factor is a
  fleet-level change).
- **The sustained turn fell with the real thrust and stays low.** 3.63 G (8.1
  deg/s) at 461 kt and 1,000 ft clean, 2.93 G with the default load, 2.68 G at 593
  kt. Against the others at the same point: the Mirage 2000 and F-5E 4.13 G, the
  Gripen 4.80 G, the F-16A 7.15 G. It rises with lower fuel (4.14 G at half and
  quarter, 4.81 G near empty) because the scripted bank steps are coarse. No
  published figure was found.
- **The instantaneous G holds up at speed but sits under the real limit.** The F1's
  own +7 row gives 6.1 G clean at 461 kt (88 percent of the row) rising to 7.06 G
  nearly dry, against the real +7.5 G; the 0.06 G overshoot past the row is the
  same hair the +9 aircraft show. Whole-G rows leave no +7.5 row, so a correction
  as for the Tornado would be a +8 row fitted from the +7 row (not done; John's
  call, and `plus_nine_row`/`fitted_row` would do it). The push is -2.61 to -3.00
  G against the real -3.6.
- **The afterburner fuel flow is now far above the real engine's.** The record's
  flows were authored for the retail 30,000 lbf: 23 lb/s is 2.76 lb/lbf/h there,
  but 5.22 lb/lbf/h against the real 15,873 lbf, 2.6 times the published
  specific fuel consumption (1.96 to 1.99; 8.6 to 8.8 lb/s). The tank lasts 4.4
  minutes at afterburner where the real engine's consumption would give about 11.8.
  The dry flow (3 lb/s, 0.98 lb/lbf/h) is exact to the published 0.96 to 0.98. As
  with the Tornado (B5-3, flows set from the real specific fuel consumption), a
  guarded override of `aftFuelConsumption` 23 to 9 lb/s would be a small, sourced
  correction; it is John's call and was not made.
- **Weights and fuel are the retail Mirage III copies** (decision 24): empty 4.7
  percent under, fuel 17 percent under (6,120 against 7,385 lb), maximum 15
  percent under, so the clean aircraft weighs 10 percent less than the real
  24,030 lb. Kept as John decided.
- **Takeoff and landing are quicker than the Jane's text for the clean aircraft and
  close for the default load.** Clean the model lifts off after 211 m and rolls out
  in 376 to 384 m (published 600 m and 670 m, weights unstated, brake parachute not
  stated); with the default load it needs 504 m and 584 to 598 m, which brackets
  the published figures from below. The approach speed is 115.9 kt clean and 145.6
  kt with the load, either side of the published 125 kt (the load flies at 1.3 times
  a stall speed scaled to 29,839 lb, near the maximum). The imported polygon speeds
  alone (`--retail-stall-speeds`) lift off at 100.4 and 89.5 kt.
- **The `climb` maneuver runs the tank dry** at 113,602 ticks (the scripted climb
  and cruise out of fuel), with no crash and no loss, unlike the F-5E's overspeed
  loss after the zoom.

The liftoff range (125 to 150 kt) and the approach range (125 to 150 kt) in
`tools/battery_scenarios/flight.py` are fitted plausibility windows, agent
decisions of 2026-11-11. The approach range starts at the published 125 kt and
allows 20 percent above, because the scripted approach flies the default load; the
liftoff band is the Mirage 2000's, no primary figure having been found. The
measured 135.4 kt and 145.6 kt are the regression values (`KNOWN_LIFTOFF`,
`KNOWN_APPROACH`). They depend on John's stations decision: a lighter default load
lowers both speeds (clean 103.9 and 115.9 kt) and the windows then need re-fitting.

The battery: `python3 tools/battery.py --scenario '*-mf1*' --lane flight --profile
.local/m3-aircraft/profile-mf1 --windows 1 --jobs 2` passed 6 of 6, run
`20261111-000852`: `flight-animation-mf1`, `flight-liftoff-mf1`,
`flight-liftoff-retail-mf1`, `flight-approach-mf1`, `flight-variety-mf1` and
`render-mf1-skin`. The liftoff, liftoff-retail and approach scenarios are new in
this slice. The combat-G table has no entry for the Mirage F1 (`PULL_G_RETAIL`).

Not run: `--native-flight-tables` (tables not present locally), the live-fire and
jettison battery slots (MF1-6), the AI, Quick Mission and multiplayer scenarios
(MF1-9), and a pull or turn probe with the tank fuel burning off (the probe takes
stores as dead mass).
