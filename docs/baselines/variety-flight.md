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

## F-15E validation, 2026-11-10

Implementation and measurement mode, slice F15E-8. Nothing in the flight model,
the record or a shared constant changed. The `F15E.PT` record was read from the
ignored local FA_2.LIB extraction under `.local/m3-imports/scratch/f15e` and the
app profile is `.local/m3-aircraft/profile-f15e`. The F-15E flies its own record
on the default hybrid `--researched-flight` path (John kept the retail copy of
the F-15C's numbers, decision 1 of 2026-11-10, so no thrust or weight
correction applies). Its flight words are the F-15C's: of the 17 statements that
differ from `F15.PT` (see [aircraft variety](../formats/aircraft-variety.md))
none is an envelope row, a weight, a thrust or a fuel flow, and the sim reads the
record only, so **every F-15E number below equals the F-15C's** on the same load
(`diff` of the clean `performance_probe` logs: identical). The two aircraft
differ only by the load they start with. Everything below is measured, not
fitted; the only fitted values are the two battery windows at the end. All knots
are true airspeed. The logs are local: `.local/m3-aircraft/checks/F15E-8/`.

The record: empty 28,600 lb, internal fuel 13,662 lb, maximum 68,000 lb, military
32,558 lbf and afterburner 47,860 lbf (both engines together), 1 G top speed
1,434 kt at 36,000 ft (806 kt at sea level), ceiling 100,000 ft, G rows -4 to +9,
fuel flows 4 and 24 lb/s. The clean aircraft with full internal fuel weighs
42,262 lb. The default load (a 2,300 lb centreline tank, 12 Mk 82 on the belly
pair, four AIM-120, 450 rounds) carries 9,940 lb, so the default gross is 52,202
lb (Load Ordnance page; the F-15C's default 2,152 lb gives 44,414 lb). The probe
runs "default load" as `PROBE_PAYLOAD=9940`, with the tank's fuel counted as dead
weight, so burn is unchanged.

**Harness.** `variety_flight` was not rerun (the `flight-variety-f15e` scenario
covers it). `PROBE_POINTS=1 PROBE_LEVEL=1 PROBE_PULLS=1 envelope_probe` gave the
level and pull rows; `performance_probe` (clean, `PROBE_PAYLOAD=9940`, and
`PROBE_ONLY=turn PROBE_ALTS=1000 PROBE_FUEL=... PROBE_PAYLOAD=...` for the fuel
table) gave the rest; `--headless-flight` with `--aircraft f15e` the maneuvers.
The pilots are scripted, so turn and climb figures are a repeatable harness
reading, not a best-pilot figure.

### Top speed and ceiling

| Altitude | 1 G edge (record), kt | Level, AB, clean, kt | Ratio | Level, AB, default load, kt | Level, military, clean, kt |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1,000 ft | 819 (806 at sea level) | 750 (Mach 1.14) | 0.916 | 710 (Mach 1.08) | 619 (Mach 0.94) |
| 10,000 ft | 934 | 856 | 0.916 | | |
| 15,000 ft | | 915 (Mach 1.46) | | 865 (Mach 1.38) | 754 (Mach 1.20) |
| 20,000 ft | 1,081 | 990 | 0.916 | | |
| 30,000 ft | 1,301 | 1,192 (Mach 2.02) | 0.916 | 1,128 (Mach 1.91) | 983 (Mach 1.67) |
| 36,000 ft | 1,434 | 1,313 (Mach 2.29) | 0.916 | 1,243 (Mach 2.17) | 1,083 (Mach 1.89) |
| 40,000 ft | 1,427 | 1,307 | 0.916 | | |
| 50,000 ft | 1,408 | 1,290 | 0.916 | | |
| 60,000 ft | 1,368 | 1,253 | 0.916 | | |
| 80,000 ft | 1,237 | 1,133 | 0.916 | | |
| 95,000 ft | 1,040 | 953 | 0.916 | | |
| 100,000 ft | 921 | 807, not held | 0.876 | | |

The loaded-drag rule (`loadedDrag=40`) settles full internal fuel at 0.916 of
the edge at every altitude, a little lower than the Mirage (0.948), inside the
fighters' 87 to 96 percent band. The maximum is Mach 2.29 against the record's
Mach 2.5 edge. The `climb` maneuver tops out at 108,921 ft (1.089 of the record
ceiling) on a dry tank. The 1 G polygon runs to 100,000 ft, so level flight is
held up to about 95,000 ft, far above the real service ceiling (below).

### Acceleration and climb (afterburner unless stated, full internal fuel)

| Level acceleration | Clean, s | Default load, s | Peak specific excess power, clean / default |
| --- | --- | --- | --- |
| 1,000 ft, AB, 363 to 460 / 530 / 590 / 660 kt | 7 / 13 / 19 / 30 | 9 / 17 / 26 / 46 | 588 / 446 ft/s |
| 1,000 ft, military, 363 to 460 / 530 / 590 kt | 13 / 27 / 52 | 17 / 39 / not reached | 319 / 238 ft/s |
| 15,000 ft, AB, 345 to 500 / 690 / 810 kt | 12 / 31 / 53 | 15 / 42 / 79 | 573 / 436 ft/s |
| 36,000 ft, AB, 316 to 520 / 800 / 1,030 / 1,150 kt | 18 / 48 / 83 / 114 | 23 / 61 / 110 / 163 | 594 / 453 ft/s |
| 36,000 ft, military, 316 to 520 / 800 / 980 kt | 28 / 80 / 148 | 35 / 106 / 228 | 332 / 253 ft/s |

| Afterburner climb from sea level | Clean, Mach 0.8 | Clean, Mach 0.9 | Default load, Mach 0.8 | Default load, Mach 0.9 |
| --- | --- | --- | --- | --- |
| 10,000 ft | 36 s | 44 s | 43 s | 55 s |
| 36,000 ft | 84 s | 94 s | 104 s | 118 s |
| 50,000 ft | 109 s | 119 s | 138 s | 151 s |
| 59,000 ft | 129 s | 137 s | 165 s | 177 s |
| Best sustained rate | 593 ft/s (35,574 ft/min) at 36,900 ft | 604 ft/s (36,245 ft/min) at 39,800 ft | zoom reading only | zoom reading only |

The default load's "best rate" rows print 623 to 634 ft/s at 87,000 ft and 850
kt: that is a late zoom reading, not a sustained climb, so only the times above
are quoted for the loaded aircraft. The clean best sustained climb is 36,000 ft
per minute and the peak specific excess power 588 to 594 ft/s (35,300 to 35,600
ft/min).

### Turns and G limits

| Case (afterburner, full internal fuel) | Clean (42,262 lb) | Default load (52,202 lb) |
| --- | --- | --- |
| Full aft stick, 3 s, 1,000 ft, 330 kt | 6.69 G, 19.1 deg/s | 5.94 G, 16.4 deg/s |
| Full aft stick, 1,000 ft, 461 kt (Mach 0.7) | 7.57 G, 17.1 deg/s | 6.73 G, 14.9 deg/s |
| Full aft stick, 1,000 ft, 593 kt | 7.57 G, 13.0 deg/s | 6.73 G, 11.4 deg/s |
| Full aft stick, 15,000 ft, 564 kt | 5.87 G, 9.8 deg/s | 5.22 G, 8.5 deg/s |
| Full aft stick, 30,000 ft, 530 kt | 4.18 G, 6.6 deg/s | 3.71 G, 5.6 deg/s |
| Full forward stick, 461 kt, 1,000 ft | -2.51 G | -2.23 G |
| Sustained level turn, 1,000 ft, 461 kt | 5.74 G, 12.8 deg/s (bank 80 deg, speed settled at 485 kt) | 4.80 G, 11.1 deg/s |
| Sustained level turn, 1,000 ft, 593 kt | 4.14 G, 7.4 deg/s | 3.24 G, 5.8 deg/s |
| Sustained level turn, 15,000 ft, 439 kt (Mach 0.7) | 5.74 G, 13.1 deg/s | 4.80 G, 11.2 deg/s |
| Sustained level turn, 15,000 ft, 564 kt | 4.82 G, 8.7 deg/s | 4.14 G, 7.8 deg/s |
| Sustained level turn, 36,000 ft, 631 kt | 4.17 G, 6.4 deg/s | 3.64 G, 5.7 deg/s |

**Full aft stick at 461 kt, 1,000 ft, by fuel (the Mirage M2K-11 table).** The
record's +9 G row is in the source, so unlike the Mirage no correction was
needed; the shared loaded-elevator rule (`loadedElev=43`) scales the limit with
gross weight, so the limit climbs as fuel burns.

| Internal fuel, lb | 13,662 (full) | 6,831 (half) | 3,416 (quarter) | 300 (near empty) |
| --- | ---: | ---: | ---: | ---: |
| Clean gross, lb | 42,262 | 35,431 | 32,016 | 28,900 |
| Peak G, clean | 7.57 | 8.28 | 8.69 | 9.10 |
| Turn rate, clean, deg/s | 17.1 | 18.9 | 19.8 | 20.8 |
| Full forward stick, clean | -2.51 | -2.74 | -2.88 | -3.02 |
| Peak G, default stores | 6.73 | 7.29 | 7.60 | 7.91 |
| Sustained turn, clean | 5.74 G, 12.8 deg/s | 7.14 G, 16.3 deg/s | 7.16 G, 15.0 deg/s | 7.17 G, 13.9 deg/s |

So a full-fuel clean F-15E peaks at 7.57 G against the record's +9, and reaches
+9 only at near-empty tanks (9.10 G at 300 lb). The push stops at -2.5 to -3.0 G
against the record's -4 (and the Jane's text's design -3). The sustained turn
comes from a scripted pilot that raises the bank in 2 degree steps until speed
or altitude is lost, so it is a lower bound; with light fuel the pilot settles
near the 82 degree bank limit and the speed drifts up (521 to 563 kt), which is
why the sustained rate falls while the G stays at 7.2.

### Fuel and endurance

The record's flows are 4 lb/s at military power and 24 lb/s at afterburner (14,400
and 86,400 lb/h), which is 0.442 and 1.805 lb/lbf/h against the record's 32,558 and
47,860 lbf. The 13,662 lb tank lasts 56.9 minutes at military power and 9.5
minutes at afterburner (burn measured: 4.00 and 24.00 lb/s over 300 s). A level
cruise at 65 percent of the top speed at 20,000 ft (702 kt, throttle trimmed) burns
2.96 lb/s, which is 77.0 minutes or 901 nautical miles clean, and 68.9 minutes or
806 nautical miles with the default stores. Military power gives 754 nautical miles
clean and afterburner 153 nautical miles. The `climb` maneuver burns the tank dry
and the 300 s `sprint` burns 7,200 lb (24 lb/s).

### Takeoff, approach and landing

Ground probes at `--theater UKR --ground-start 1` (the first Ukrainian airfield,
a 5,532 ft runway), default load (gross 52,202 lb, `loading=0.825`, stall scale
1.351, 120.1 kt stall and 148.5 kt minimum level flapped speed after the weight
scaling):

| Probe | Result |
| --- | --- |
| takeoff, default load | liftoff tick 1,085 at 149.1 kt after 1,155 ft (352 m), rotation 135.1 kt |
| takeoff, `--loadout none` (42,262 lb) | liftoff 127.3 kt after 673 ft (205 m), rotation 121.0 kt |
| takeoff, F-15C default (44,414 lb) | liftoff 131.8 kt after 759 ft, rotation 124.2 kt |
| takeoff, `--retail-stall-speeds`, default load | liftoff 113.1 kt after 656 ft (imported polygon speeds, no weight scaling) |
| land, 65 percent fuel (8,880 lb), default load | approach 156.1 kt, touchdown 155.0 kt at 4.7 ft/s sink, 2,247 ft rollout, stopped, score 100 |
| land, 65 percent fuel, `--loadout none` | approach 140.4 kt, touchdown 137.7 kt, 1,808 ft rollout |
| land, 65 percent fuel, F-15C default | approach 144.0 kt, touchdown 141.4 kt, 1,899 ft rollout |
| land (full fuel, default load) | touchdown 156.9 kt at 7.5 ft/s, stopped, score 100 |
| land-hard | touchdown 166.2 kt at 33.8 ft/s, stopped, score 100 |
| land-gear-up, land-off-runway | crashed for the gear and for the not-landable ground, by design |
| nosewheel | no crash, 11.2 kt maximum |

Eleven air maneuvers all ended with `non_finite=0`: `level` 7,200 ticks, `pull`
7,200 (7.57 G, loop completed), `loop` completed at 2,565 ticks (7.62 G), `sprint`
36,000 (831 kt), `climb` 115,554 ticks (108,921 ft, 1,059 kt), `dive` and
`overspeed` ending in the overspeed loss, `rudder`, `devices` (no violations),
`stall-recover` (recovered, no loss) and `spin-recover` (recovered).

### Against the real aircraft

Retail text (Jane's All the World's Aircraft, `F15E.INF` and `F15.INF` on the
retail media, read 2026-11-10; numbers only) and public pages read the same day:
Wikipedia, "McDonnell Douglas F-15E Strike Eagle" (early aircraft F100-PW-220 at
23,770 lbf afterburner, later -229 at 29,160 lbf) and "Pratt & Whitney F100"
(-220: 14,590 lbf military, 0.73 lb/lbf/h; -229: 17,800 lbf and 29,160 lbf, 0.76
and 1.94 lb/lbf/h); milavia.net, "Specifications F-15 Eagle" (F-15E: more than
2,655 km/h = 1,650 mph at altitude, 50,000 ft/min at sea level; F-15C: Mach 2.5
plus, service ceiling more than 60,000 ft); migflug.com, "McDonnell Douglas F-15
Eagle" (service ceiling 65,000 ft, climb more than 50,000 ft/min, sea level
about Mach 1.2 = 1,450 km/h; a stripped record aircraft reached 30,000 m in
207.8 s); a Key.aero forum post (an early F-15C at 41,000 lb with a centreline
tank and eight missiles sustaining about 5.2 G at Mach 0.8, 15,000 ft; unverified,
a forum figure). No primary source gave a sustained turn rate, a take-off speed
or an F-15E landing speed.

| Quantity | Measured (clean / default load) | Retail record | Real (cited) |
| --- | --- | --- | --- |
| Top speed at altitude | 1,313 kt (Mach 2.29) at 36,000 ft / 1,243 kt (Mach 2.17) | 1,434 kt at 36,000 ft | Mach 2.5 (`F15E.INF`), 1,650 mph (1,434 kt) |
| Top speed at sea level | 750 kt at 1,000 ft (Mach 1.14) / 710 kt | 806 kt (819 at 1,000 ft) | about Mach 1.2 (migflug), 800 kt CAS (`F15.INF`) |
| Ceiling | level flight to about 95,000 ft, climb peak 108,921 ft | 100,000 ft | 60,000 ft service (`F15.INF`, milavia), 65,000 ft (migflug) |
| Climb | 35,300 to 36,200 ft/min best; 36,000 ft in 84 s / 104 s (Mach 0.8 schedule) | not in the record | more than 50,000 ft/min (milavia, migflug) |
| Specific excess power | 588 to 594 ft/s / 436 to 453 ft/s (AB) | | about 833 ft/s if the 50,000 ft/min figure is a sea-level rate at light weight |
| G limits | +7.57 / -2.51 at full fuel, +9.10 / -3.02 near empty (461 kt, 1,000 ft); default load +6.73 | +9 / -4 | design +9 / -3 (`F15.INF`) |
| Sustained turn, 461 kt, 1,000 ft | 5.74 G, 12.8 deg/s / 4.80 G, 11.1 deg/s (lower bound) | | about 5.2 G at Mach 0.8, 15,000 ft, 41,000 lb loaded (forum, unverified); model 5.3 G interpolated at 42,262 lb clean |
| Fuel, military / AB | 56.9 / 9.5 min; 0.442 / 1.805 lb/lbf/h | 4 / 24 lb/s | -220 about 0.73 lb/lbf/h (35 min at 32,558 lbf); -229 AB 1.94 |
| Range | 901 nmi cruise clean (extrapolated) | | 2,400 nmi ferry, combat radius 685 nmi (`F15E.INF`), not like for like |
| Liftoff | 149.1 kt, 1,155 ft (default) / 127.3 kt, 673 ft (clean) | | take-off run 274 m = 899 ft (`F15.INF`, F-15C, weight not stated) |
| Approach / landing | 156.1 kt (default, 65% fuel) / 140.4 kt (clean); rollout 2,247 / 1,808 ft | | approach 125 kt CAS, landing run 1,067 m = 3,500 ft (`F15.INF`) |

### Findings (nothing changed)

1. **Top speeds need no fit.** The record's Mach 2.5 edge equals the real
   figure; the model reaches 0.916 of it (Mach 2.29) through the shared
   loaded-drag rule, and 750 kt (Mach 1.14) at 1,000 ft against about Mach 1.2
   at sea level.
2. **The ceiling is the record's 100,000 ft, not the real 60,000 ft.** The model
   holds level flight at Mach 2.1 at 60,000 ft and to about 95,000 ft, and the
   `climb` maneuver peaks at 108,921 ft. The F-15C does the same (identical
   record). It is a retail copy kept by John's decision 1; the envelope's 0 G
   row reaches 105,000 ft.
3. **Climb is about 71 percent of the published 50,000 ft/min clean and about 53
   percent with the default load.** The cause is the shared drag law again (drag
   equals the reference thrust at the top speed and grows with speed squared,
   no ram thrust gain). The shortfall is smaller than the Mirage's (about 45
   percent) because the F-15E's thrust to weight is 1.13 clean, so more of the
   thrust is left over; thrust is not the lever. Options: accept; a fleet-level
   drag shape or ram factor (not an F-15E fit, not done).
4. **G reaches +9 only when nearly empty.** The record has the +9 G row, but the
   loaded-elevator rule scales it with weight: 7.57 G at full fuel, 8.28 at half,
   9.10 at 300 lb (clean); the default load peaks at 6.73 to 7.91 G. The push is
   -2.5 to -3.0 G against the record's -4. A fleet rule, as for the Mirage
   before M2K-11; John may want the same +9 treatment (an opinionated change he
   has not asked for here).
5. **Fuel flows are the F-15C record's.** Military flow is 39 percent under the
   real F100 (0.442 against 0.73 lb/lbf/h), so the endurance at military power
   is 57 minutes against about 35; afterburner is within 7 percent of the -229
   figure. Kept retail.
6. **Default load is heavy for the airframe's numbers.** The default 12 Mk 82,
   tank and four AIM-120 add 9,940 lb, 52,202 lb gross against a 68,000 lb record
   maximum (81,000 lb real): liftoff 149.1 kt, approach 156.1 kt, climb to 36,000
   ft in 104 s. The retail 68,000 lb maximum and the F-15C's 129 hit points stay
   as the record has them (decision 1).
7. **Takeoff and landing are plausible.** Clean liftoff after 205 m and clean
   rollout 551 m against Jane's 274 m and 1,067 m (for the F-15C, weights not
   stated); the model's approach (140 to 144 kt clean) is 12 to 15 percent above
   Jane's 125 kt CAS, in line with the weight-scaled stall rule the other jets
   show.

### Battery windows (fitted, agent decisions 2026-11-10)

`flight-liftoff-f15e`, `flight-liftoff-retail-f15e` and `flight-approach-f15e`
join the battery with `INTERNAL_FUEL_LB["f15e"] = 13662`, a liftoff window of 130
to 150 kt (fleet plausibility band; no primary figure), an approach window of 125
to 145 kt (Jane's 125 kt CAS up to the F-15C's measured 144 kt) and the recorded
default-load values 149.1 and 156.1 kt as the plus or minus 6 percent regression
windows. The F-15E's default load measures above the approach window (156.1 kt, 7.7
percent over 145), which the shared 10 percent allowance and the recorded value
accept; it is a regression guard, not a claim of the real approach speed.

Provenance: every flight figure is a measurement of the existing hybrid model on
retail record values (spec-derived); the liftoff and approach windows are
fitted (agent decisions, 2026-11-10); the real-aircraft figures are cited public
pages and the Jane's text on the retail media, for plausibility only and not
evidence of original behaviour. Nothing was attributed to John beyond decision 1.

## Saab Gripen validation, 2026-11-10

Validation mode, slice GRI-8. The Gripen flies the hybrid `--researched-flight`
default on its own record (`GRIPEN.PT`, shape `gri.SH`, `variety` identity from
GRI-1 and B5-1); `--legacy-flight` and `--native-flight-tables` stay separate. The
record is a genuine JAS 39A (GRI-0): empty 14,599 lb, internal fuel 5,000 lb,
maximum takeoff 27,560 lb, one engine at 12,140 lbf military and 18,100 lbf
afterburner, G rows -4 to +9, 1 G top speed 1,145 kt at 36,000 ft (739 kt at
1,000 ft, 729 kt at the 5 ft airport), ceiling 60,000 ft, fuel flows 2 and 14
lb/s. No record correction applies, none was made, and **nothing was fitted**:
John kept the 60,000 ft ceiling and the -4 limit (decision 10, 2026-11-10). The
only fitted values are the two battery windows at the end. The default load
(`AGM65G.JT` x2, `AIM120.JT` x4, `AIM9M.JT` x2, the `F150.GAS` centreline tank
and 570 BK 27 rounds) weighs 23,809 lb, so the carried stores are 4,210 lb
including 990 lb in the tank.

**Harness.** As for the Mirage: `variety_flight` printed `aircraft=Gripen
conventional=pass deterministic=pass landing=pass ticks=1200`; `envelope_probe`
(`PROBE_POINTS=1 PROBE_LEVEL=1 PROBE_PULLS=1`) and `performance_probe`
(`PROBE_ALTS=1000,15000,30000,36000,50000`, then `PROBE_PAYLOAD=4210` for the
default load) on the local `GRIPEN.PT`; `target/debug/tore-app --aircraft gripen
--headless-flight N --maneuver ...` with the profile
`.local/m3-aircraft/profile-gripen`. Pilots are scripted, so the turn and climb
figures are a repeatable harness reading, not a best-pilot figure. All knots are
true airspeed. Logs are local: `.local/m3-aircraft/checks/GRI-8/`.

### Top speed and ceiling

| Altitude | 1 G edge (record), kt | Level, AB, clean, kt | Ratio |
| --- | ---: | ---: | ---: |
| 1,000 ft | 739 | 704 (Mach 1.07) | 0.952 |
| 10,000 ft | 831 | 791 | 0.952 |
| 20,000 ft | 939 | 894 | 0.952 |
| 30,000 ft | 1,068 | 1,017 (Mach 1.72) | 0.952 |
| 36,000 ft | 1,145 | 1,090 (Mach 1.90) | 0.952 |
| 40,000 ft | 1,143 | 1,088 | 0.952 |
| 50,000 ft | 1,118 | 1,065 (Mach 1.86) | 0.952 |
| 55,000 ft | 1,058 | 1,007 | 0.952 |
| 60,000 ft | 998 | 933 (Mach 1.62) | 0.935 |

Level flight is held at every altitude to the 60,000 ft ceiling. The dry
(military power) level speed at full fuel is 576 kt at 1,000 ft and 893 kt
(Mach 1.56) at 36,000 ft. With the default load the level speed is 678 kt at
1,000 ft and 809 kt at 15,000 ft. The `climb` maneuver zooms to 68,470 ft (1.141
of the ceiling, the Mirage 1.092 and the MiG-31 1.132) with the tank burned dry;
the scripted afterburner climb reaches 66,385 ft (Mach 0.8 schedule). The model
ceiling is a record value kept by decision.

### Acceleration and climb (afterburner unless stated, full internal fuel, 19,599 lb)

| Level acceleration | Time, s | Peak specific excess power |
| --- | --- | --- |
| 1,000 ft, AB, 363 to 460 / 590 / 660 kt | 9 / 27 / 47 | 445 ft/s (26,700 ft/min) |
| 1,000 ft, military, 363 to 460 / 530 kt | 18 / 42 | 230 ft/s |
| 15,000 ft, AB, 345 to 500 / 690 / 810 kt | 15 / 43 / 93 | 426 ft/s |
| 36,000 ft, AB, 316 to 520 / 800 / 1,030 / 1,090 kt | 23 / 67 / 154 / 394 | 402 ft/s |
| 36,000 ft, military, 316 to 520 / 800 kt | 38 / 137 | 220 ft/s |

| Afterburner climb from sea level | Speed schedule Mach 0.8 | Mach 0.9 |
| --- | --- | --- |
| 10,000 ft | 43 s | 56 s |
| 36,000 ft | 107 s | 123 s |
| 50,000 ft | 144 s | 158 s |
| 59,000 ft | 212 s | 187 s |
| Best sustained rate | 427 ft/s (25,600 ft/min) at 35,400 ft | 432 ft/s (25,900 ft/min) at 37,200 ft |

With the default load the peak specific excess power falls to 336 to 349 ft/s
(20,200 to 20,900 ft/min). The Gripen's clean thrust to weight is 0.92 (18,100
lbf over 19,599 lb), the Mirage's 0.91, and the two give the same acceleration
shape (445 and 418 ft/s). The F-16A record, with the F100's retail thrust and
more thrust per pound, gives 789 ft/s and 655 ft/s sustained (39,300 ft/min).

### Turns and G limits

| Case (afterburner, full internal fuel, clean) | Result |
| --- | --- |
| Full aft stick, 3 s, 1,000 ft, 330 kt | 7.06 G, 22.5 deg/s |
| Full aft stick, 1,000 ft, 461 kt (Mach 0.7) | 8.00 G, 18.6 deg/s |
| Full aft stick, 1,000 ft, 593 kt | 8.01 G, 14.0 deg/s |
| Full aft stick, 15,000 ft, 564 kt | 7.98 G, 14.0 deg/s |
| Full aft stick, 30,000 ft, 530 kt | 5.31 G, 9.3 deg/s |
| Full aft stick, 36,000 ft, 517 kt | 4.43 G, 7.4 deg/s |
| Full forward stick, 461 kt and up at 1,000 ft | -3.55 G |
| Same pulls with the default load | 7.26 G (461 kt), -3.22 G |
| Sustained level turn, 1,000 ft, 461 kt | 4.80 G, 10.4 deg/s (bank 78 deg, speed rose to 497 kt) |
| Sustained level turn, 1,000 ft, 593 kt | 3.24 G, 5.7 deg/s |
| Sustained level turn, 15,000 ft, 564 kt | 4.14 G, 7.5 deg/s |
| Sustained level turn, 30,000 ft, 530 kt | 4.81 G, 9.7 deg/s |
| Sustained level turn, 36,000 ft, 631 kt | 4.15 G, 6.9 deg/s |

The sustained turns come from a scripted pilot that raises the bank in 2 degree
steps, so they are lower bounds. At 1,000 ft and Mach 1.1 there is no sustained or
instantaneous turn (above the 739 kt edge); at 50,000 ft the pull is altitude
limited (0.4 to 3.5 deg/s).

**Full-aft-stick pull by fuel**, 461 kt (Mach 0.7), 1,000 ft, clean, afterburner,
3 s (`PROBE_ONLY=turn PROBE_ALTS=1000 PROBE_FUEL=...`), against the F-16A and the
Mirage 2000 (the Mirage column is the M2K-11 +9 G result, reproduced):

| Fuel | Gripen G (deg/s) | F-16A G (deg/s) | Mirage 2000 G (deg/s) |
| --- | ---: | ---: | ---: |
| Full (5,000 / 6,972 / 6,346 lb) | 8.00 (18.6) | 7.67 (16.8) | 7.96 (19.1) |
| Half (2,500 / 3,486 / 3,173 lb) | 8.52 (20.0) | 8.35 (18.3) | 8.52 (20.7) |
| Quarter (1,250 / 1,743 / 1,587 lb) | 8.80 (20.7) | 8.74 (19.1) | 8.83 (21.6) |
| Near empty (300 lb) | 9.03 (21.3) | 9.09 (19.8) | 9.09 (22.3) |
| Full forward stick, same four | -3.55 / -3.78 / -3.90 / -4.01 | -3.39 / -3.70 / -3.87 / -4.02 | -3.52 / -3.76 / -3.90 / -4.02 |

All three aircraft carry a +9 row (the Gripen and the F-16A from their records,
the Mirage from M2K-11's fitted row), and the shared loaded-elevator rule scales
the limit with the fuel and stores carried, so the pull rises from 8.0 G at full
fuel to 9.0 G nearly dry, a hair past the +9 row as for the Mirage's 9.09. At full
tanks the Gripen gives the highest figure of the three (8.00, against 7.67 and
7.96), because it is the lightest aircraft and carries the least fuel; the three
converge as the tanks empty. The -4 limit is reached on the push nearly dry (-4.01); the real limit is -3
(decision 10 keeps -4).

### Fuel and endurance

The record's flows are 2 lb/s at military power and 14 lb/s at afterburner, 7,200
and 50,400 lb/h, or 0.593 and 2.785 lb/lbf/h against the engine's thrust. The 5,000
lb tank lasts 41.7 minutes at military power and 6.0 minutes at afterburner
(burn measured: 2.00 and 14.00 lb/s over 300 s). A level cruise at 65 percent of
the top speed (610 kt at 20,000 ft, throttle trimmed) burns 1.39 lb/s, which is
59.8 minutes or 608 nautical miles; with the default load 55.5 minutes and 563
nautical miles. The `sprint` maneuver burns 4,200 lb in 300 s at afterburner and
the `climb` maneuver burns the tank dry.

### Takeoff, approach and landing

Ground probes at `--theater UKR --ground-start 1` with the default load (gross
23,809 lb; `loading=0.631`, flapped stall 113.5 kt after the weight scaling):

| Probe | Result |
| --- | --- |
| takeoff | liftoff tick 1,164 at 132.1 kt after 1,097 ft (334 m), rotation 123.4 kt |
| takeoff, `--retail-stall-speeds` | liftoff 105.7 kt after 695 ft (imported polygon speeds, no weight scaling) |
| takeoff-gear-airborne | same liftoff, gear raised after the wheels are clear (gear 0.55 at the end) |
| land, 65 percent fuel (3,250 lb) | approach 147.5 kt, touchdown 142.0 kt at 3.3 ft/s sink, stopped 1,932 ft (589 m) after touchdown with 1,462 ft of the 5,532 ft runway left, score 100 |
| land, full fuel | touchdown 143.5 kt, 2.9 ft/s, stopped 1,969 ft (600 m) after touchdown, 1,616 ft left, score 100 |
| land-hard | touchdown 157.1 kt at 33.7 ft/s, stopped, score 100 |
| land-gear-up, land-off-runway | crashed for the gear and for the not-landable ground, by design |
| nosewheel | no crash, 11.2 kt maximum |

The record has no arrestor hook (`hook_available=false`), like the real aircraft.

Eleven air maneuvers all ended with `non_finite=0`: `level` 7,200 ticks (502 kt,
65 percent of the 5,000 ft edge), `pull` 7,200 (8.00 G, loop completed), `loop`
completed at 2,011 ticks (8.03 G), `sprint` 36,000 (771.7 kt, 0.989 of the edge),
`climb` as above, `dive` and `overspeed` ending in the overspeed loss (1.154 and
1.100 of the edge), `rudder`, `devices` (violations 0), `stall-recover` (alert at
tick 1,131, recovered at 1,713, 0 ft lost) and `spin-recover` (entered at tick 3,
recovered at 1,048 after 0.12 revolutions of recovery and 1.23 in all, 1,949 ft
lost). Logs under `.local/m3-aircraft/checks/GRI-8/probes/`.

### Against the real aircraft

Public figures read 2026-11-10: Wikipedia, "Saab JAS 39 Gripen", JAS 39C/D
specifications (empty 6,800 kg = 14,991 lb, maximum takeoff 14,000 kg = 30,865 lb,
internal fuel 3,000 L = about 2,340 kg, Mach 2 at 15,240 m = 50,000 ft, Mach 1.2
at sea level, service ceiling 15,240 m = 50,000 ft, g limits +9/-3, thrust to
weight 0.97, combat range 800 km, ferry range 3,200 km, takeoff distance 400 m,
landing distance 500 m); Wikipedia "Volvo RM12" (54 kN = 12,100 lbf dry, 80.5 kN =
18,100 lbf with afterburner; specific fuel consumption 0.844 lb/lbf/h dry, 1.79
afterburner); Migflug, "Fighter Jet Takeoff and Landing: The Real Numbers"
(Saab's three takeoff and landing distance sets: 400 m and 500 m in the 2016
*Gripen C In Brief* sheet, 800 m by 16 m road strips in 2019, 500 and 600 m for
the Gripen E; the Swedish Air Force's touchdown speed of just under 300 km/h, about
162 kt; Saab deliberately dropped the thrust reverser, brake chute and arrestor
hook). The JAS 39A weights are those of the GRI-0 review (MILAVIA and others).
No climb rate, sustained or instantaneous turn rate, approach speed or liftoff
speed was found for the C/D in a primary source. None of these pages is a flight
manual.

| Quantity | Measured (clean unless stated) | Retail record | Real (cited) |
| --- | --- | --- | --- |
| Top speed at altitude | 1,090 kt at 36,000 ft (Mach 1.90); 1,065 kt (Mach 1.86) at 50,000 ft | 1,145 kt at 36,000 ft (Mach 2.0), 1,118 kt at 50,000 ft | Mach 2 at 50,000 ft (about 1,147 kn) |
| Top speed at sea level | 704 kt at 1,000 ft (Mach 1.07) | 739 kt at 1,000 ft, 729 kt at 5 ft (Mach 1.10) | Mach 1.2 (about 794 kn) Wikipedia; Mach 1.15 (about 760 kn) other sources |
| Service ceiling | 66,385 ft zoom peak (68,470 ft on the `climb` maneuver); level flight held to 60,000 ft | 60,000 ft | 50,000 ft |
| Rate of climb, sea level | 445 ft/s peak excess power (26,700 ft/min); 427 to 432 ft/s sustained (25,600 to 25,900 ft/min) | | not found for the C/D |
| Thrust | 12,140 dry, 18,100 AB, 1 engine | same | 12,100 / 18,100, 1 engine |
| Thrust to weight | 0.92 clean full fuel, 0.76 default load | | 0.97 (loading not stated) |
| G limits | +8.0 (+9.0 nearly dry) / -3.55 clean; +7.3 / -3.2 default load | +9 / -4 | +9 / -3 |
| Instantaneous turn | 22.5 deg/s at 330 kt (7.1 G); 18.6 deg/s at 461 kt (8.0 G) | | not found |
| Sustained turn (scripted, lower bound) | 4.8 G, 10.4 deg/s at 461 kt, 1,000 ft | | not found |
| Liftoff | 132.1 kt, 1,097 ft (334 m) | | takeoff distance 400 m (500 m, other sets) |
| Approach / touchdown | 147.5 kt approach, 142.0 kt touchdown, 589 m rollout | | touchdown just under 162 kt; landing distance 500 to 600 m |
| Arrestor hook | none | none | none |
| Empty weight | 14,599 lb | 14,599 lb | JAS 39A 14,599 lb (39C/D 14,991 lb) |
| Internal fuel | 5,000 lb | 5,000 lb | about 5,160 lb (3,000 L; retail 3 percent low) |
| Dry fuel flow | 7,200 lb/h (0.593 per lbf) | same | 0.844 per lbf, about 10,250 lb/h |
| Afterburner fuel flow | 50,400 lb/h (2.785 per lbf) | same | 1.79 per lbf, about 32,400 lb/h |
| Range, clean 5,000 lb | 608 nm cruise at 20,000 ft | | combat range 800 km (432 nm, a radius with weapons), ferry 3,200 km with tanks: not comparable |

Findings (no value was changed):

- **Top speed at altitude needs no fit.** The record's 1 G row peaks at 1,145 kt
  at 36,000 ft, which is Mach 2.0, the published figure; at the published
  50,000 ft the record edge is Mach 1.95 (1,118 kt) and the model flies Mach 1.86.
  The fleet loaded-drag rule puts the level speed 4.8 percent under the edge.
- **Sea-level top speed is under the published Mach 1.15 to 1.2.** The record edge
  is Mach 1.10 (729 kt at the airport, 739 kt at 1,000 ft) and the model flies Mach
  1.07 (704 kt). The shortfall against the 794 kn of Wikipedia is 7 percent at the
  edge and 11 percent in the model. The rows are the retail JAS 39A rows and John
  asked for no speed fit; it can be done with `fitted_envelopes` if he wants it.
- **The 60,000 ft ceiling is 10,000 ft over the real one, by decision.** The model
  holds level flight at 60,000 ft (Mach 1.62), 933 kt, and zooms to 66,000 to
  68,000 ft; real 50,000 ft. The F-16A record, whose ceiling is its own, zooms to
  66,000 ft as well.
- **Climb and the shared drag law.** No published sea-level climb rate was found
  for the JAS 39C/D (the Wikipedia page gives one only for the Gripen E/F), so the
  Mirage's 45 percent factor cannot be computed. The shape is the Mirage's:
  peak excess power 445 ft/s at a thrust to weight of 0.92 (the Mirage 418 ft/s at
  0.91); 460 kt takes 9 s and 590 kt takes 27 s from 363 kt at 1,000 ft; the
  best sustained climb is about 430 ft/s. The F-16A record, with more thrust per
  pound, climbs at 655 ft/s. The drag law (drag equals the reference thrust at the
  top speed and grows with the square of speed) and the lack of ram thrust apply
  to the Gripen exactly as to the Mirage, and the Gripen's sustained turn is
  governed by it too (see the next finding).
- **Sustained turn is two thirds of the F-16A's.** 4.8 G and 10.4 deg/s at 461 kt
  and 1,000 ft, against the F-16A's 7.15 G and 15.4 deg/s and the Mirage's 4.13 G
  and 9.6 deg/s; at 15,000 ft and 564 kt 4.14 G (7.5 deg/s) against 5.76 G (10.6
  deg/s) and 3.63 G (6.8 deg/s). Thrust to weight and the same drag law explain
  it. No published figure was found, so the gap to the real aircraft is not
  known.
- **G limit.** The Gripen's own +9 row gives 8.00 G at full fuel and 7.26 G with
  the default load at 461 kt and 1,000 ft, reaching 9.03 G nearly dry. The rule
  that scales the limit with fuel and stores is the fleet's. No G fit.
- **Fuel flows are the record's, not the RM12's.** The dry flow is 30 percent under
  the published specific fuel consumption (0.593 against 0.844 lb/lbf/h) and the
  afterburner flow 56 percent over (2.785 against 1.79). At the published values
  the 5,000 lb tank would last about 29 minutes dry (the record gives 42) and 9.3
  minutes at afterburner (the record gives 6.0). They stay retail, as for the
  Mirage and the MiG-25.
- **Record values are real.** Weights, tank (3 percent under the real litres),
  thrust and engine count match the JAS 39A or the RM12, so unlike the Mirage no
  weight error shows.
- **Takeoff and landing agree with Saab's short-field figures.** The model lifts
  off after 334 m (Saab: 400 m minimum, 500 m in another set) and stops 589 to 600
  m after touchdown (Saab 500 to 600 m). Its 142 to 143.5 kt touchdown is under the
  Swedish Air Force's just under 162 kt. The approach speed is the shared rule's
  147.5 kt at both fuel loads (1.05 times the loaded minimum speed is the floor).
  The record has no hook, which is correct for this aircraft.

The liftoff range (125 to 150 kt) and the approach range (140 to 160 kt) in
`tools/battery_scenarios/flight.py` are fitted plausibility windows, agent
decisions of 2026-11-10. The liftoff band is the Mirage's: it brackets the
record's own 118.5 kt sea-level 1 G minimum speed, and no primary liftoff speed
exists in the sources read. The approach band brackets the measured 147.5 kt
approach and the Swedish Air Force's roughly 162 kt touchdown figure. The measured
132.1 kt and 147.5 kt are recorded as regression values (`KNOWN_LIFTOFF`,
`KNOWN_APPROACH`); `INTERNAL_FUEL_LB` holds 5,000 lb.

The battery: `python3 tools/battery.py --scenario '*-gripen*' --lane flight
--profile .local/m3-aircraft/profile-gripen --windows 1` passed 17 of 17 (run
`20261110-215803`), including `flight-liftoff-gripen`,
`flight-liftoff-retail-gripen`, `flight-approach-gripen` and
`flight-variety-gripen`.

Not run: `--native-flight-tables` (tables not present locally), a windowed free
flight (the slice asks for no frames), a primary-source takeoff or landing run.

## Northrop F-5E validation, 2026-11-10

Validation mode, slice F5E-8. The F-5E Tiger flies the hybrid `--researched-flight`
default on its own record (`F5E.PT`, shape `f5e.SH`, `variety` identity from B5-1);
`--legacy-flight` and `--native-flight-tables` stay separate. The record is the
retail `F5E.PT` with John's B5-2 corrections (decisions of 2026-11-10, "follow the
RL F-5E"): empty 9,558 lb, internal fuel 4,400 lb (retail 6,240 lb), maximum
takeoff 24,664 lb, two J85-GE-21 at 7,000 lbf military and 10,000 lbf afterburner
in all, G rows -4 to +7 (the retail +8 row, a repeat of the -4 row, is retired), a
51,800 ft ceiling (retail 42,000 ft; the maximum altitude and the top edges of the
0 G and 1 G polygons moved by 9,800 ft), fuel flows 1 and 8 lb/s. **Nothing was
fitted**: no value, envelope or constant changed. The only fitted values are the
two battery windows at the end. The default load (the M61 with 560 rounds, two
`AIM9B.JT` and the 990 lb `F150.GAS` centreline tank) weighs 15,365 lb, carried
stores 1,407 lb including the tank.

**Harness.** As for the Mirage and the Gripen: `envelope_probe`
(`PROBE_POINTS=1 PROBE_LEVEL=1 PROBE_PULLS=1`) and `performance_probe`
(`PROBE_ALTS=1000,15000,30000,36000,50000`, then `PROBE_PAYLOAD=1407` for the
default load) on the local `F5E.PT`; `target/debug/tore-app --aircraft f5e
--headless-flight N --maneuver ...` with the profile
`.local/m3-aircraft/profile-f5e`. Pilots are scripted, so turn and climb figures
are a repeatable harness reading, not a best-pilot figure. All knots are true
airspeed. Logs are local: `.local/m3-aircraft/checks/F5E-8/`.

### Top speed and ceiling

| Altitude | 1 G edge (record), kt | Level, AB, clean, kt | Ratio |
| --- | ---: | ---: | ---: |
| 1,000 ft | 731 | 685 (Mach 1.04) | 0.937 |
| 10,000 ft | 807 | 756 | 0.937 |
| 20,000 ft | 898 | 841 | 0.937 |
| 30,000 ft | 1,018 | 954 (Mach 1.62) | 0.937 |
| 36,000 ft | 1,090 | 1,021 (Mach 1.78) | 0.937 |
| 40,000 ft | 922 | 864 | 0.937 |
| 50,000 ft | 625 | 586 (Mach 1.02) | 0.937 |
| 51,800 ft | 572 | 535 | 0.937 |

Level flight is held at every altitude to the 51,800 ft ceiling. The dry (military
power) level speed at full fuel is 573 kt (Mach 0.87) at 1,000 ft and 855 kt (Mach
1.49) at 36,000 ft; with the default load the afterburner speed is 672 kt at 1,000
ft and 1,002 kt at 36,000 ft. The `climb` maneuver zooms to 52,214 ft (1.008 of the
corrected ceiling) after 208 s, and the scripted afterburner climb reaches 52,247 ft
(Mach 0.8 schedule). The 1 G polygon's fast edge falls steeply above 39,000 ft
(952 kt at 39,000 ft, 572 kt at 51,800 ft, about 29 kt per 1,000 ft), because the
B5-2 correction moved only the polygon's top edge; speeds between 40,000 ft and the
ceiling are therefore low (586 kt, Mach 1.02, at 50,000 ft).

### Acceleration and climb (afterburner unless stated, full internal fuel, 13,958 lb)

| Level acceleration | Time, s | Peak specific excess power |
| --- | --- | --- |
| 1,000 ft, AB, 363 to 460 / 590 / 660 kt | 12 / 37 / 72 | 332 ft/s (19,900 ft/min) |
| 1,000 ft, military, 363 to 460 / 530 kt | 22 / 54 | 185 ft/s |
| 15,000 ft, AB, 345 to 500 / 690 / 750 kt | 20 / 62 / 94 | 311 ft/s |
| 36,000 ft, AB, 316 to 520 / 800 / 980 kt | 31 / 92 / 203 | 291 ft/s |
| 36,000 ft, military, 316 to 520 / 800 kt | 48 / 197 | 170 ft/s |

(The 50,000 ft rows of the probe start 315 kt below the envelope's slow edge, so
the aircraft is not in level flight there and they are not used.)

| Afterburner climb from sea level | Speed schedule Mach 0.8 | Mach 0.9 |
| --- | --- | --- |
| 10,000 ft | 52 s | 70 s |
| 36,000 ft | 138 s | 159 s |
| 50,000 ft | 199 s | 221 s |
| Best sustained rate | 314 ft/s (18,800 ft/min) at 29,100 ft | 316 ft/s (19,000 ft/min) at 34,800 ft |

With the default load the peak excess power falls to 258 to 295 ft/s and the best
sustained rate to 280 ft/s on the Mach 0.8 schedule; the scripted Mach 0.9 climb
does not hold its schedule with the load (it reached only 221 ft), a harness
limit, not read as a flight finding. The F-5E's clean thrust to weight is 0.72
(10,000 lbf over 13,958 lb), against the Gripen's 0.92, the Mirage 2000's 0.91
(peak excess power 445 and 418 ft/s) and the F-16A's much higher figure (789 ft/s).

### Turns and G limits

| Case (afterburner, full internal fuel, clean) | Result |
| --- | --- |
| Full aft stick, 3 s, 1,000 ft, 330 kt | 5.95 G, 17.8 deg/s |
| Full aft stick, 1,000 ft, 461 kt (Mach 0.7) | 5.98 G, 12.8 deg/s |
| Full aft stick, 1,000 ft, 593 kt | 2.56 G, 2.9 deg/s |
| Full aft stick, 15,000 ft, 439 kt | 4.26 G, 8.5 deg/s |
| Full aft stick, 30,000 ft, 413 kt | 1.70 G, 1.8 deg/s |
| Full forward stick, 330 / 461 kt at 1,000 ft | -3.32 / -2.55 G |
| `pull` maneuver (450 kt, 5,000 ft) | 5.97 G |
| Same pulls with the default load | see the fuel table |
| Sustained level turn, 1,000 ft, 461 kt | 4.13 G, 9.6 deg/s (bank 76 deg) |
| Sustained level turn, 1,000 ft, 593 kt | 2.47 G, 4.2 deg/s |
| Sustained level turn, 15,000 ft, 439 kt | 4.13 G, 9.9 deg/s |
| Sustained level turn, 15,000 ft, 564 kt | 3.25 G, 6.0 deg/s |
| Sustained level turn, 30,000 and 36,000 ft | 1.6 G, 2 deg/s (scripted bank 52 deg) |
| Sustained level turn, default load, 1,000 ft, 461 kt | 3.63 G, 8.2 deg/s |

The sustained turns come from a scripted pilot that raises the bank in 2 degree
steps, so they are lower bounds. At 1,000 ft and Mach 1.1 there is no sustained or
instantaneous turn (above the 731 kt edge); at 50,000 ft there is no sustained turn
above 20 degrees of bank. The record's rows explain the high-speed numbers: the +3 G
row ends at 28,875 ft, the +4 G row at 22,000 ft, and at sea level the +3 G row
reaches only 622 kt and the +5 G row 533 kt (times 0.937 for the loaded drag rule),
so the F-5E pulls 2.6 G at 593 kt and 1,000 ft. This is the retail record's shape,
not a fit.

**Full-aft-stick pull by fuel**, 461 kt (Mach 0.7), 1,000 ft, clean, afterburner,
3 s (`PROBE_ONLY=turn PROBE_ALTS=1000 PROBE_FUEL=...`), against the F-16A, the
Mirage 2000 (the M2K-11 table) and the Gripen (GRI-8), all re-run on this tree:

| Fuel | F-5E G (deg/s) | Gripen G (deg/s) | F-16A G (deg/s) | Mirage 2000 G (deg/s) |
| --- | ---: | ---: | ---: | ---: |
| Full (4,400 / 5,000 / 6,972 / 6,346 lb) | 5.98 (12.8) | 8.00 (18.6) | 7.67 (16.8) | 7.96 (19.1) |
| Half (2,200 / 2,500 / 3,486 / 3,173 lb) | 6.49 (14.2) | 8.52 (20.0) | 8.35 (18.3) | 8.52 (20.7) |
| Quarter (1,100 / 1,250 / 1,743 / 1,587 lb) | 6.78 (14.9) | 8.80 (20.7) | 8.74 (19.1) | 8.83 (21.6) |
| Near empty (300 lb) | 7.00 (15.5) | 9.03 (21.3) | 9.09 (19.8) | 9.09 (22.3) |
| Full forward stick, same four | -2.55 / -2.77 / -2.89 / -2.99 | -3.55 / -3.78 / -3.90 / -4.01 | -3.39 / -3.70 / -3.87 / -4.02 | -3.52 / -3.76 / -3.90 / -4.02 |

At 330 kt the same four fuel states give 5.95 / 6.45 / 6.73 / 6.95 G. The shared
loaded-elevator rule scales the limit with the fuel and stores carried, so the pull
rises as the tank empties, as for the others, but the F-5E's top row is +7: the
near-empty pull stops at 7.00 G, exactly the row and not a hair past it as the +9
aircraft do (9.03 to 9.09). There is no +8 row left; the pull never exceeds +7
(7.00 G at 461 kt, 6.95 G at 330 kt). The real limit is +7.33 G, 0.33 G above the
whole-G top row (John's B5-2 decision). At full tanks the F-5E gives 5.98 G, 25
percent under the Gripen: the rule scales with the fuel carried, and the F-5E's
4,400 lb tank is 46 percent of its empty weight (the Gripen's 5,000 lb is 34 percent),
so the same rule takes 15 percent off the +7 row against 11 percent off the Gripen's +9.
The push is -2.55 G at 461 kt full and -2.99 near empty against a -4 row, because
the -4 row's slow edge is 570 kt at sea level.

### Fuel and endurance

The record's flows are 1 lb/s at military power and 8 lb/s at afterburner, 3,600
and 28,800 lb/h, or 0.514 and 2.880 lb/lbf/h against the engines' thrust. The 4,400
lb tank lasts 73.3 minutes at military power and 9.2 minutes at afterburner (burn
measured: 1.00 and 8.00 lb/s over 300 s). A level cruise at 65 percent of the top
speed (583 kt at 1,000 ft, throttle trimmed) burns 0.69 lb/s, which is 106 minutes
or 1,032 nautical miles (extrapolated from 300 s); the `sprint` maneuver burns
2,400 lb in 300 s at afterburner.

### Takeoff, approach and landing

Ground probes at `--theater UKR --ground-start 1` with the default load (gross
15,365 lb; `loading=0.608`, flapped stall 135.2 kt after the weight scaling):

| Probe | Result |
| --- | --- |
| takeoff | liftoff tick 1,605 at 153.4 kt after 1,763 ft (537 m), rotation 141.1 kt |
| takeoff, `--retail-stall-speeds` | liftoff 123.0 kt after 1,114 ft (imported polygon speeds, no weight scaling) |
| takeoff-gear-airborne | same liftoff, gear raised after the wheels are clear |
| land, 65 percent fuel (2,860 lb) | approach 166.2 kt, touchdown 159.8 kt at 3.3 ft/s sink, stopped 2,305 ft (703 m) after touchdown with 1,074 ft of the 5,532 ft runway left, score 100 |
| land, full fuel | touchdown 163.5 kt, 5.1 ft/s, stopped 2,401 ft (732 m) after touchdown, 1,399 ft left, score 100 |
| land-hard | touchdown 174.2 kt at 33.8 ft/s, stopped, score 100 |
| land-gear-up, land-off-runway | crashed for the gear and for the not-landable ground, by design |
| nosewheel | no crash, 11.2 kt maximum |

The record has no arrestor hook (`hook_available=false`).

Eleven air maneuvers: ten ended with `non_finite=0` and no loss (`level` 7,200
ticks at 494 kt, 65 percent of the 5,000 ft edge; `pull` 5.97 G; `loop` completed
at 2,999 ticks; `sprint` 36,000 ticks, 739.8 kt, 0.967 of the edge; `rudder`;
`devices` violations 0; `stall-recover` alert at tick 1,352, recovered at 1,889,
0 ft lost; `spin-recover` entered at tick 3, recovered at 1,199 after 0.65
revolutions of recovery and 1.99 in all, 2,104 ft lost), `dive` and `overspeed`
ended in the overspeed loss by design (1.146 and 1.100 of the edge). The eleventh,
**`climb`, ended in an overspeed loss as well** (`crashed=true`, `non_finite=0`):
the scripted pilot zooms to 52,214 ft at 208 s and then, in afterburner with its
speed hold, runs through 849 kt at 43,000 ft where the corrected polygon's fast
edge has fallen to 832 kt (1.020 of it) for 840 ticks. The Gripen's and the
Mirage's climb ended without a loss because their edges do not fall so fast; the
F-5E's steep fast edge is the same B5-2 top-edge shift described above. Not
changed (it is a harness script, not a flight rule), and reported for John. Logs
under `.local/m3-aircraft/checks/F5E-8/probes/`.

### Against the real aircraft

Public figures read 2026-11-10: Wikipedia, "Northrop F-5", F-5E specifications
(maximum speed Mach 1.63, 1,739 km/h = 939 kn, at 36,000 ft; service ceiling 51,800
ft; rate of climb 34,500 ft/min = 175 m/s; empty 9,583 lb, clean gross 15,745 lb,
maximum takeoff 24,675 lb; internal fuel 677 US gal = 2,560 L; thrust to weight 0.4
at maximum takeoff weight; range 481 nmi clean; two J85-GE-21 at 3,500 lbf dry and
5,000 lbf afterburner each); MILAVIA (maximum level speed Mach 1.64 at 36,000 ft,
ceiling 51,800 ft); FAS "F-5" (ceiling 51,800 ft, 9,683 lb empty, 15,745 lb gross,
24,676 lb maximum takeoff); Migflug, "Northrop F-5" (take-off ground run 2,000 ft
with two Sidewinders at 15,745 lb, 2,900 ft over a 50 ft obstacle; landing roll from
50 ft 2,500 ft with the brake chute, 3,700 ft without; stall speed with flaps and
gear down at 50 percent internal fuel 230 km/h = 124 kn; climb 34,500 ft/min;
ceiling 51,800 ft); Wikipedia, "General Electric J85" (J85-GE-21: 3,600 lbf dry,
5,000 lbf afterburner; specific fuel consumption 1.24 lb/lbf/h dry, 2.13
afterburning). **No sea-level top speed was found** in any source read (the
brief's "about 760 kt" is not supported by them), and no G limit, sustained or
instantaneous turn rate, or primary-source approach speed. None of these pages is
a flight manual; the test weight of the climb figure is not stated.

| Quantity | Measured (clean unless stated) | Retail record (corrected) | Real (cited) |
| --- | --- | --- | --- |
| Top speed at 36,000 ft | 1,021 kt (Mach 1.78) | 1,090 kt (Mach 1.90) | Mach 1.63 (939 kn), Wikipedia; Mach 1.64, MILAVIA |
| Top speed at sea level | 685 kt at 1,000 ft (Mach 1.04) | 731 kt at 1,000 ft, 723 kt at 5 ft | not found |
| Service ceiling | 52,214 ft zoom peak; level flight held to 51,800 ft | 51,800 ft (retail 42,000 ft) | 51,800 ft |
| Rate of climb, sea level | 332 ft/s peak excess power (19,900 ft/min); 314 to 316 ft/s sustained (18,800 to 19,000 ft/min) | | 34,500 ft/min (575 ft/s), test weight not stated |
| Thrust | 7,000 dry, 10,000 AB, 2 engines | same | 7,000 to 7,200 dry, 10,000 AB, 2 engines |
| Thrust to weight | 0.72 clean full fuel, 0.65 default load, 0.40 at maximum takeoff | | 0.4 at maximum takeoff weight |
| G limits | +6.0 (+7.0 nearly dry) / -2.55 clean at 461 kt | +7 / -4 | +7.33 (John's note) |
| Instantaneous turn | 17.8 deg/s at 330 kt (5.95 G); 12.8 deg/s at 461 kt (5.98 G) | | not found |
| Sustained turn (scripted, lower bound) | 4.13 G, 9.6 deg/s at 461 kt, 1,000 ft | | not found |
| Liftoff | 153.4 kt, 1,763 ft (537 m), 15,365 lb | | ground run about 2,000 ft (610 m) at 15,745 lb with two Sidewinders |
| Approach / touchdown | 166.2 kt approach, 159.8 kt touchdown, 703 m rollout | | stall 124 kt flaps and gear down, 50% fuel (1.3 times it is 161 kt); landing roll 2,500 ft (chute) to 3,700 ft from 50 ft |
| Empty weight | 9,558 lb | 9,558 lb | 9,583 lb (FAS 9,683) |
| Internal fuel | 4,400 lb | 4,400 lb | 677 US gal, about 4,500 lb (3 percent under real) |
| Dry fuel flow | 3,600 lb/h (0.514 per lbf) | same | 1.24 per lbf, about 8,700 lb/h |
| Afterburner fuel flow | 28,800 lb/h (2.880 per lbf) | same | 2.13 per lbf, about 21,300 lb/h |

Findings (no value was changed):

- **Top speed at altitude is above the real one.** The record's 1 G row peaks at
  Mach 1.90 at 36,000 ft, 17 percent over the published Mach 1.63; the model flies
  Mach 1.78, 9 percent over. The shape (about the F-4 family's) is retail and the B5-2
  corrections did not touch it. A per-aircraft speed fit through `fitted_envelopes`
  is possible; John asked for none.
- **The corrected ceiling is reachable and the +7 top row behaves.** Level flight is
  held to 51,800 ft (535 kt) and the zoom peak is 1.008 of it. The pull reaches 7.00
  G only when the tank is nearly empty and never passes it; there is no +8 row.
- **Climb and the shared drag law.** The model's peak specific excess power is 332
  ft/s (19,900 ft/min) and its sustained climb 314 to 316 ft/s, 55 to 58 percent of
  the published 34,500 ft/min (575 ft/s; the Mirage showed 45 percent, whose test
  weight is also unstated). The thrust and the weights are the real ones; the cause
  is the drag law (drag equals the reference thrust at the top speed and grows with
  the square of speed, no ram thrust gain). By hand with that law, matching 575 ft/s
  at the same speed would take about 1.5 times the engines' 10,000 lbf (an estimate,
  not run). It is reported, not changed; a per-aircraft drag shape or ram factor is
  a fleet-level change.
- **Sustained turn.** 4.13 G and 9.6 deg/s at 461 kt and 1,000 ft, the Mirage 2000's
  4.13 G and 9.6 deg/s (similar thrust to weight and bank step), against the
  Gripen's 4.8 G and the F-16A's 7.15 G; it falls to 2.5 G at 593 kt and to 1.6 G
  at 30,000 ft and above. The same drag law governs it, and so does the record's G
  rows (above). No published figure was found.
- **High-speed G is limited by the retail record, not by a fit.** The F-5E record
  keeps a template-like envelope: the +3 G fast edge at sea level is 622 kt and the
  +3 G row ends at 28,875 ft, so a pull at 593 kt gives 2.6 G and a pull at 30,000 ft
  1.7 G. The real aircraft holds its structural limit over most of the speed range.
  John may want these rows reshaped; not done.
- **Fuel flows are the record's, not the J85s'.** The dry flow is 59 percent under
  the published specific fuel consumption (0.514 against 1.24 lb/lbf/h) and the
  afterburner flow 35 percent over (2.88 against 2.13). At the published values the
  4,400 lb tank would last about 30 minutes dry (the record gives 73) and 12.4
  minutes at afterburner (the record gives 9.2). They stay retail, as for the Mirage
  and the Gripen.
- **Record values are real.** Empty weight (25 lb under), thrust and engine count
  match the real aircraft, and the maximum takeoff weight gives the published 0.4
  thrust to weight; the tank is 3 percent under the real 677 US gal.
- **Takeoff and landing agree with the published figures.** The model lifts off after
  537 m (real ground run about 610 m at a heavier 15,745 lb) at 153.4 kt, and the
  approach is 166.2 kt against 161 kt from 1.3 times the published 124 kt stall
  speed. It stops 703 to 732 m after touchdown, shorter than the published 762 to
  1,128 m (those start at 50 ft and include the flare). The imported polygon speeds
  alone (`--retail-stall-speeds`) lift off at 123 kt, under the 124 kt published
  stall speed with flaps and gear down, so the weight scaling matters here. The
  record has no hook; not compared with the real aircraft.
- **The `climb` maneuver ends in an overspeed loss** after the zoom because of the
  steep fast edge above 39,000 ft (above).

The liftoff range (135 to 165 kt) and the approach range (155 to 175 kt) in
`tools/battery_scenarios/flight.py` are fitted plausibility windows, agent
decisions of 2026-11-10, taken as 1.1 to 1.33 and 1.25 to 1.4 times the one
published figure (the 124 kt stall speed). The measured 153.4 kt and 166.2 kt are
recorded as regression values (`KNOWN_LIFTOFF`, `KNOWN_APPROACH`);
`INTERNAL_FUEL_LB` holds 4,400 lb.

The battery: `python3 tools/battery.py --scenario '*-f5e*' --lane flight --profile
.local/m3-aircraft/profile-f5e --windows 1` passed 13 of 13 (run `20261110-222926`),
including `flight-liftoff-f5e`, `flight-liftoff-retail-f5e`, `flight-approach-f5e`
and `flight-variety-f5e`.

Provenance: record values and G rows spec-derived (retail); the fuel, ceiling and
+8 row corrections opinionated (John, 2026-11-10, B5-2); the measurements agent
findings of 2026-11-10; the liftoff and approach battery ranges fitted agent
decisions of 2026-11-10; the real figures are public pages (Wikipedia, MILAVIA, FAS,
Migflug), for comparison only, which disagree among themselves and are not flight
manuals. Nothing here is attributed to John beyond the B5-2 corrections.

Not run: `--native-flight-tables` (tables not present locally), a windowed free
flight (the slice asks for no frames), a primary-source takeoff or landing run, a
better-pilot sustained turn.

## Batch 5 flight maxima and fuel flows (B5-4, 2026-11-11)

Implementation and measurement mode, slice B5-4. **Opinionated (John,
2026-11-11)**: the flight-model maxima and fuel flows of the five batch aircraft
follow real figures that fit TORE. The rules, the cited figures and the rounding
are in [aircraft records](../spec/aircraft-records.md#batch-5-flight-maxima-and-fuel-flows-b5-4-2026-11-11);
the review per aircraft is in [the variety document](../formats/aircraft-variety.md#batch-5-flight-maxima-and-fuel-flows-b5-4-2026-11-11).
The earlier sections of this page are the measurements before the change and are
kept as written. Probes: `performance_probe` (`PROBE_ONLY=turn PROBE_ALTS=1000,15000,30000`,
`PROBE_FUEL` full, half, quarter and 300 lb; `PROBE_ONLY=fuel`) and `envelope_probe`
(`PROBE_POINTS PROBE_LEVEL PROBE_PULLS`), on the local extracted records (clean,
afterburner, full aft stick for three seconds at Mach 0.7, 461 kt, 1,000 ft; push
is full forward stick for two seconds). Logs are local, `before/` and `after/`
under `.local/m3-aircraft/checks/B5-4/`. The Tornado and Mirage F1 numbers are
clean only: their default loads are being changed on other branches.

**Pull and push at 461 kt and 1,000 ft, clean, by fuel (full, half, quarter, 300
lb).**

| Aircraft | Row | Fuel, lb | Positive G before | Positive G after | Push before | Push after | Real |
| --- | --- | --- | --- | --- | --- | --- | --- |
| F-15E | +9 to +10, -4 to -3 | 13,662 / 6,831 / 3,416 / 300 | 7.57 / 8.28 / 8.69 / 9.10 | 8.41 / 9.19 / 9.65 / 10.10 | -2.51 / -2.74 / -2.88 / -3.02 | same | +9 / -3 |
| Gripen | +9 kept, -4 to -3 | 5,000 / 2,500 / 1,250 / 300 | 8.00 / 8.52 / 8.80 / 9.03 | same | -3.55 / -3.78 / -3.90 / -4.01 | -2.66 / -2.83 / -2.92 / -3.00 | +9 / -3 |
| F-5E | +7 to +8, -4 to -3 | 4,400 / 2,200 / 1,100 / 300 | 5.98 / 6.49 / 6.78 / 7.00 | 6.83 / 7.41 / 7.74 / 8.00 | -2.55 / -2.77 / -2.89 / -2.99 | same | +7.33 / -3.0 |
| Tornado | +7 to +8, -3 kept | 11,221 / 5,610 / 2,805 / 300 | 6.17 / 6.59 / 6.82 / 7.04 | 7.05 / 7.52 / 7.79 / 8.04 | -2.63 / -2.81 / -2.91 / -3.01 | same | +7.5 / -3 |
| Mirage F1 | +7 to +8, -3 to -4 | 6,120 / 3,060 / 1,530 / 300 | 6.14 / 6.59 / 6.84 / 7.06 | 7.01 / 7.53 / 7.82 / 8.07 | -2.61 / -2.80 / -2.91 / -3.00 | -3.48 / -3.74 / -3.88 / -4.01 | +7.5 / -3.6 |

At half fuel the new positive G is within 0.1 G of the real limit for the F-5E
(+0.08), the Tornado (+0.02), the Mirage F1 (+0.03) and 0.19 G over for the F-15E;
the Gripen's +9 stays 0.48 G under (a +10 row would be 0.47 G over). Near empty
every aircraft passes the real limit (the F-15E reaches 10.1 G, the F-5E 8.0 G, the
Tornado 8.0 G, the Mirage F1 8.1 G): the loading divisor is the shared rule, and the
half-fuel test is John's Mirage 2000 precedent. At 15,000 and 30,000 ft the pulls
are the altitude-limited ones of the lower rows and did not change.

**Ceilings and level top speed (`envelope_probe`, afterburner, level flight held
to within 200 ft).** Below each new ceiling the envelope edges are the record's
own, so the level speeds are unchanged: F-15E 750 kt at 1,000 ft, 1,313 kt at
36,000 ft, 1,290 kt at 50,000 ft; Gripen 704, 1,090 and (50,000 ft, the new
ceiling) 1,041 kt; Tornado 793, 1,321 and (50,000 ft) 1,135 kt; Mirage F1 762 and
1,271 kt; F-5E 685 and 1,021 kt. The level-flight ceiling is now the polygon's: F-15E
60,000 ft (1,229 kt there, it was held to 95,000 ft before), Gripen 50,000 ft (it
was 60,000 ft), Tornado 50,000 ft (it was 60,000 ft).

**Fuel (`PROBE_ONLY=fuel`, full tank, military and afterburner at 20,000 ft; the
probe's burn equals the record flow).**

| Aircraft | Flows before, lb/s | Flows after, lb/s | Implied lb/(lbf h) before | after | Cited | Full tank, military, min before / after | Afterburner, min before / after |
| --- | --- | --- | --- | --- | --- | --- | --- |
| F-15E | 4 / 24 | 7 / 26 | 0.442 / 1.805 | 0.774 / 1.956 | 0.73 / 1.94 | 56.9 / 32.5 | 9.5 / 8.8 |
| Gripen | 2 / 14 | 3 / 9 | 0.593 / 2.785 | 0.890 / 1.790 | 0.844 / 1.79 | 41.7 / 27.8 | 6.0 / 9.3 |
| F-5E | 1 / 8 | 2 / 6 | 0.514 / 2.880 | 1.029 / 2.160 | 1.24 / 2.13 | 73.3 / 36.7 | 9.2 / 12.2 |
| Tornado (B5-3) | 3 / 22 | 3 / 22 | 0.624 / 2.463 | same | 0.598 / 2.5 | 62.3 / 62.3 | 8.5 / 8.5 |
| Mirage F1 | 3 / 23 | 3 / 9 | 0.980 / 5.216 | 0.980 / 2.041 | 0.981 / 1.991 | 34.0 / 34.0 | 4.4 / 11.3 |

The whole-pound unit leaves the F-5E's dry flow 17 percent under the cited figure
(2 against 2.41 lb/s; 3 would be 24 percent over), the Gripen's dry flow 5 percent
over and the F-15E's dry flow 6 percent over.

**Not changed, and why (findings for John).**

- **Top speeds.** F-15E: 750 kt (Mach 1.14) at 1,000 ft against a real 800 kt
  (Mach 1.2), 6 percent under; 1,313 kt (Mach 2.29) at 36,000 ft against Mach 2.5,
  8 percent under. Gripen: 704 kt (Mach 1.07) at sea level against Mach 1.2 (794
  kt), 11 percent under; 1,090 kt (Mach 1.90) at 36,000 ft and 1,041 kt (Mach 1.82) at
  50,000 ft against Mach 2 (Wikipedia gives it at 50,000 ft), 5 to 9 percent
  under. F-5E: 1,021 kt (Mach 1.78) at 36,000 ft against Mach 1.63 (939 kn), 9
  percent over (the record itself is Mach 1.90, 16 percent over). Tornado, Mirage
  F1: within 5 percent. In every case the record's envelope is the aircraft's own,
  not another's, and the measured level speed is 92 to 97 percent of the record's
  top edge through the shared drag law (`reference_thrust` against drag at the top
  edge), so a fix needs either a per-record polygon fit or a change to the shared
  law. Neither was made. The F-5E would need a guarded fast-edge scale of about
  0.92 (and its +8 and ceiling rows follow the same polygon).
- **Gripen +9.** Half fuel pulls 8.52 G, 0.48 G under; kept.
- **Mirage F1 ceiling,** Tornado top speeds, Tornado -3: inside the thresholds.

Checks: workspace tests 7,614 passed, 0 failed; the flight lane battery for the
F-15E, Gripen and F-5E (`--windows 1`, profile `.local/m3-aircraft/profile`) passed
44 of 44, so no liftoff or approach window moved (the weights, stall edges and the
envelope below the ceilings are unchanged); `--headless-flight 1200` for all five
ran with 0 non-finite samples. Not run: a zoom-climb ceiling measurement after the
cut, the AI, mission, replay and net batteries.

## F-5E top-speed trim (B5-5, 2026-11-11)

Implementation and measurement mode, slice B5-5. **Opinionated (John,
2026-11-11: "Trim F-5E only"); factor fitted.** The rule and the numbers a player
notices are in [aircraft records](../spec/aircraft-records.md#f-5e-top-speed-trim-b5-5-2026-11-11);
how top speed arises is in [the variety document](../formats/aircraft-variety.md#f-5e-top-speed-trim-b5-5-2026-11-11).
The earlier sections of this page are the measurements before the change and are
kept as written. Probes (local record, clean, afterburner, full fuel 4,400 lb,
`performance_probe` and `envelope_probe`, logs under
`.local/m3-aircraft/checks/B5-5/before/` and `after/`, driver `run_probes.sh`):

| Measurement | Before (HEAD 52c0178b) | After |
| --- | --- | --- |
| Envelope top edge at 0 / 1,000 / 10,000 / 20,000 / 30,000 / 36,000 / 40,000 / 50,000 / 51,800 ft, kt | 723 / 731 / 807 / 898 / 1,018 / 1,090 / 922 / 625 / 572 | 665 / 673 / 742 / 826 / 937 / 1,003 / 848 / 575 / 526 |
| Level top speed, same altitudes, kt | 677 / 685 / 756 / 841 / 954 / 1,021 / 864 / 586 / 535 | 623 / 630 / 695 / 774 / 878 / 940 / 795 / 539 / 493 |
| Level speed over envelope edge | 0.937 | 0.937 |
| Mach at 36,000 ft | 1.78 | 1.64 (real 1.63 to 1.64) |
| Pull at 461 kt, 1,000 ft, fuel 4,400 / 2,200 / 1,100 / 300 lb | 6.83 / 7.41 / 7.74 / 8.00 G | 6.84 / 7.42 / 7.75 / 8.01 G |
| Push at 461 kt, same fuels | -2.55 / -2.77 / -2.89 / -2.99 G | same |
| Instantaneous turn 330 kt / 461 kt, 1,000 ft | 5.95 G 17.8 deg/s / 6.83 G 15.4 deg/s | 5.96 G 17.8 deg/s / 6.84 G 15.5 deg/s |
| Peak Ps, 1,000 / 15,000 / 36,000 ft | 332 / 311 / 291 ft/s | 303 / 286 / 268 ft/s |
| Acceleration 363 to 460 / 530 / 590 kt, 1,000 ft | 12 / 23 / 37 s | 13 / 27 / 50 s |
| Best sustained climb (Mach 0.8 schedule); 36,000 ft in | 314 ft/s at 29,127 ft; 138 s | 286 ft/s at 31,711 ft; 156 s |
| Sustained turn, 1,000 ft, 461 kt / 593 kt | 4.13 G / 2.47 G | 3.63 G / 1.79 G |
| Military and afterburner level speed on the fuel probe | 675 / 803 kt | 624 / 742 kt |
| Fuel endurance (military 36.7 min, afterburner 12.2 min) | same | same |

The probe's Mach 0.9 sea-level climb schedule (593 kt) can no longer be held after
the trim (best rate 36 ft/s, highest 218 ft) because sea-level top speed is now
623 kt; it is a probe schedule, not a game rule. The Mach 0.9 sea-level
instantaneous peak rose from 2.56 to 2.92 G only because the aircraft bleeds speed
faster and passes into the +4 row sooner (the row table is unchanged there).

`tore-app --aircraft f5e --headless-flight` (profile `.local/m3-aircraft/profile`,
through `tools/agent-run.sh`): `--headless-flight 1200` ran with 0 non-finite
samples (fuel 4,400 lb); the F5E-8 air maneuvers (`level`, `pull`, `loop`, `sprint`,
`climb`, `dive`, `overspeed`, `rudder`, `devices`, `stall-recover`, `spin-recover`)
all ran with `non_finite=0`; `dive`, `overspeed` and `climb` still end in an
overspeed loss as in the F-5E validation above (`speed_over_envelope_top` 1.15,
1.10 and 1.02 against the new edge; climb zoom to 52,289 ft, 1.009 of the ceiling).
`sprint` 36,000 ticks ends at 675 kt (739.8 kt before B5-4 and this slice).

Not run: `--native-flight-tables`, windowed runs, a second aircraft's probes (the
Gripen and F-15E are unchanged by construction: the fit applies only to `F5E.PT`).

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

Update (MF1-5b, 2026-11-11): John chose the cap (two tanks). The default load is now
the gun, two `F250.GAS` and four R550 (`loadout:` external fuel 3,300 lb, carried
4,481 lb, gross 26,143 lb, 87 percent of the 30,203 lb maximum). Re-run through the
battery's scripted runs (`flight-liftoff-mf1`, `flight-approach-mf1`, profile
`profile-mf1`): liftoff 121.0 kt after 1,144 ft (349 m; was 135.4 kt, 504 m),
rotation 108.5 kt; approach 127.6 kt (was 145.6 kt), touchdown 118.7 kt, stop 447 m
after touchdown. Both pass the unchanged 125 to 150 kt bands (the liftoff 121.0 kt
is 3 percent under the band start, inside the battery's 10 percent slack); the
recorded values in `tools/battery_scenarios/flight.py` were refitted to 121.0 and
127.6 (plus or minus 6 percent, agent decision). The pull, turn, top-speed,
climb and endurance rows in the tables of this section were measured with the
four-tank load and were not re-run; the clean columns are unchanged.

Update (MF1-10, 2026-11-11): the clean and default-load figures were re-measured
with the same probes after B5-4's +8 and -4 rows and 9 lb/s afterburner flow and
with the two-tank load (`PROBE_PAYLOAD=4481`), and the tables are in the
[Mirage F1 acceptance record](aircraft-mf1.md#flight-envelope). Clean, what moved:
the pull by fuel 7.01 / 7.53 / 7.82 / 8.07 G (was 6.14 / 6.59 / 6.84 / 7.06), the
push -3.48 to -4.01 G (was -2.61 to -3.00), the instantaneous turn at 461 kt 7.01 G
and 16.2 deg/s (was 6.14 G, 13.4) and the afterburner endurance 11.3 min (was 4.4);
top speeds, climb, acceleration, sustained turn and cruise are unchanged. Default
load (26,143 lb): top speed 734 kt at 1,000 ft and 1,225 kt at 36,000 ft, peak Ps
298 ft/s, acceleration 363 to 460 / 590 kt in 13 / 37 s, pull 6.37 / 6.80 / 7.03 /
7.23 G, push -3.16 to -3.59 G, sustained 461 kt turn 3.24 G at 7.4 deg/s, cruise
46.2 min, liftoff 121.5 kt after 1,158 ft, approach 127.6 kt. The default-load rows
above (four tanks) stay as the morning's record. Logs
`.local/m3-aircraft/checks/MF1-10/`.

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

## Panavia Tornado IDS validation, 2026-11-11

Validation mode, slice TOR-8. The Tornado IDS flies the hybrid `--researched-flight`
default on its own record (`TOR.PT`, shape `tor.SH`, `variety` identity from B5-1);
`--legacy-flight` and `--native-flight-tables` stay separate. The record is the
retail `TOR.PT` with John's B5-2 and B5-3 corrections: two RB199 Mk 103 at 17,300 lbf
military and 32,150 lbf afterburner in all, empty weight 31,065 lb, maximum takeoff
61,620 lb, a fitted +7 G row (real +7.5), fuel flows 3 and 22 lb/s from the
RB199's specific fuel consumption. The internal fuel (11,221 lb) and the 60,000 ft
ceiling are the record's own and stay. **Nothing was fitted**: no value, envelope
or constant changed. The only fitted values are the two battery windows at the end.
The wing sweeps with speed (TOR-3) but only draws the wing and moves the swivelling
pylons (TOR-5); no flight law reads it (see "The swing wing" below).

**Clean and default load.** The stations question (TOR-5) is open with John, so
every figure is given clean (empty weight plus full internal fuel, 42,286 lb) and
with the current default load. The default load carries 14,696 lb: two
`F250.GAS` tanks (3,300 lb of fuel in all, which feeds first), four `PAVEWAY.JT`
laser bombs, two `MK82P3.JT` and the 27 mm gun with its 360 rounds, so the default gross is 56,982 lb
(`loading=0.834`, 4,638 lb under the 61,620 lb maximum). The probe runs "default
load" as `PROBE_PAYLOAD=14696` with the tank fuel counted as dead weight, so burn is
unchanged; the headless ground starts (`--ground-start`) always carry the default
load. The shared loaded-drag and loaded-elevator rules scale drag, stall speed and
the pull limit with the weight carried; stores have no drag of their own. Numbers
that depend on John's TOR-5 answer are listed at the end of the section.

**Harness.** `envelope_probe` (`PROBE_POINTS=1 PROBE_LEVEL=1 PROBE_PULLS=1`, then
`PROBE_ALTS` up to 62,000 ft for the ceiling) and `performance_probe`
(`PROBE_ALTS=1000,15000,30000,36000,50000`, then `PROBE_PAYLOAD=14696`) on the
local `TOR.PT`; `target/debug/tore-app --aircraft tornado --headless-flight N
--maneuver ...` with the profile `.local/m3-aircraft/profile-tornado`;
`variety_flight` (conventional, deterministic, landing: pass). Pilots are scripted,
so turn and climb figures are a repeatable harness reading, not a best-pilot
figure. All knots are true airspeed. Logs are local:
`.local/m3-aircraft/checks/TOR-8/`.

### Top speed and ceiling

| Altitude | 1 G edge (record), kt | Level, AB, clean, kt | Ratio |
| --- | ---: | ---: | ---: |
| 1,000 ft | 821 | 793 (Mach 1.20) | 0.966 |
| 10,000 ft | 962 | 929 | 0.966 |
| 20,000 ft | 1,118 | 1,080 | 0.965 |
| 30,000 ft | 1,275 | 1,231 (Mach 2.09) | 0.965 |
| 36,000 ft | 1,369 | 1,321 (Mach 2.30) | 0.965 |
| 40,000 ft | 1,363 | 1,316 | 0.965 |
| 45,000 ft | 1,282 | 1,238 | 0.965 |
| 50,000 ft | 1,198 | 1,156 (Mach 2.02) | 0.965 |
| 55,000 ft | 1,042 | 1,006 | 0.965 |
| 58,000 ft | 944 | 912 | 0.965 |
| 60,000 ft | 879 | 837 | 0.952 |

Level flight is held at every altitude to the record's 60,000 ft ceiling. The
`climb` maneuver zooms to 64,966 ft (1.083 of the ceiling) and the probe's scripted
climb to 62,108 ft. The dry (military power) level speed at full fuel is 582 kt
(Mach 0.88) at 1,000 ft, 903 kt (Mach 1.53) at 30,000 ft and 969 kt (Mach 1.69) at
36,000 ft. With the default load the afterburner speed is 760 kt (Mach 1.15) at
1,000 ft, 963 kt (Mach 1.54) at 15,000 ft and 1,266 kt (Mach 2.21) at 36,000 ft,
dry 558 kt (Mach 0.85) at 1,000 ft and 928 kt (Mach 1.62) at 36,000 ft.

### Acceleration and climb (afterburner unless stated, full internal fuel)

| Level acceleration | Clean (42,286 lb): time, s | Default load (56,982 lb): time, s |
| --- | --- | --- |
| 1,000 ft, AB, 363 to 460 / 590 / 730 kt | 10 / 26 / 61 | 14 / 38 / 103 |
| 1,000 ft, military, 363 to 460 / 530 kt | 26 / 61 | 39 / 104 |
| 15,000 ft, AB, 345 to 500 / 690 / 880 kt | 17 / 42 / 86 | 23 / 59 / 131 |
| 36,000 ft, AB, 316 to 520 / 800 / 1,210 kt | 27 / 70 / 201 | 37 / 98 / 323 |
| Peak specific excess power, AB | 398 to 414 ft/s (sea level 406, 24,400 ft/min) | 282 to 292 ft/s (sea level 285) |
| Peak specific excess power, military | 154 to 162 ft/s | 107 to 161 ft/s |

(The 50,000 ft rows of the probe start 315 kt below the envelope's slow edge, so the
aircraft is not in level flight there and they are not used.)

| Afterburner climb from sea level, 450 kt start | Clean, Mach 0.8 | Clean, Mach 0.9 | Default load, Mach 0.8 | Default load, Mach 0.9 |
| --- | --- | --- | --- | --- |
| 10,000 ft | 42 s | 52 s | 53 s | 68 s |
| 30,000 ft | 91 s | 101 s | 120 s | 134 s |
| 36,000 ft | 106 s | 115 s | 142 s | 154 s |
| 50,000 ft | 147 s | 153 s | 207 s | 211 s |
| Best sustained rate | 423 ft/s (25,400 ft/min) at 25,600 ft | 433 ft/s (26,000 ft/min) at 29,700 ft | about 283 ft/s (17,000 ft/min), from the 5,000 to 20,000 ft marks | about 273 ft/s (16,400 ft/min), same marks |

The probe's "best rate" for the default load (448 and 455 ft/s at 54,700 to 55,000
ft and 860 kt) is a zoom, not a steady climb, and is not used. Thrust to weight is
0.76 clean with full fuel (32,150 lbf over 42,286 lb), 0.56 at the default gross
and 0.52 at the 61,620 lb maximum, against the Mirage 2000's 0.91, the Gripen's 0.92,
the F-5E's 0.72 and the F-15E's 1.13.

### Turns and G limits

| Case (afterburner, full internal fuel) | Clean (42,286 lb) | Default load (56,982 lb) |
| --- | --- | --- |
| Full aft stick, 3 s, 1,000 ft, 330 kt | 4.39 G, 11.9 deg/s | 3.77 G, 9.7 deg/s |
| Full aft stick, 1,000 ft, 461 kt (Mach 0.7) | 6.17 G, 13.1 deg/s | 5.29 G, 10.8 deg/s |
| Full aft stick, 1,000 ft, 593 kt | 6.17 G, 10.0 deg/s | 5.29 G, 8.3 deg/s |
| Full aft stick, 15,000 ft, 564 kt | 4.39 G, 6.7 deg/s | 3.77 G, 5.5 deg/s |
| Full aft stick, 30,000 ft, 530 kt | 3.51 G, 5.2 deg/s | 3.01 G, 4.2 deg/s |
| Full forward stick, 461 kt, 1,000 ft | -2.63 G | -2.26 G |
| `pull` maneuver (450 kt, 5,000 ft) | 6.17 G | not run |
| Sustained level turn, 1,000 ft, 461 kt | 4.82 G, 10.2 deg/s (bank 78 deg, settled 508 kt) | 4.14 G, 9.4 deg/s |
| Sustained level turn, 1,000 ft, 593 kt | 4.15 G, 7.5 deg/s | 2.93 G, 5.0 deg/s |
| Sustained level turn, 15,000 ft, 439 kt | 4.14 G, 8.2 deg/s | 3.63 G, 7.8 deg/s |
| Sustained level turn, 15,000 ft, 564 kt | 4.15 G, 7.2 deg/s | 3.64 G, 6.7 deg/s |
| Sustained level turn, 36,000 ft, 631 kt | 2.47 G, 3.2 deg/s | 2.14 G, 2.8 deg/s |

The sustained turns come from a scripted pilot that raises the bank in 2 degree
steps, so they are lower bounds. At 1,000 ft and Mach 1.1 the clean sustained turn is
2.28 G (3.1 deg/s); at 50,000 ft there is no sustained turn at Mach 0.7 above 20
degrees of bank. The record's rows explain the shape: the +6 G row (a repeat of the
-3 G row) and the fitted +7 row end at 14,000 ft, the +5 G row at 24,000 ft, so the
pull falls to 4.4 G at 15,000 ft; and at 330 kt and 1,000 ft the slow edge of the +6
G row (341 kt) limits the pull to 4.39 G, so the corner speed of the record is
about 340 to 370 kt at sea level. This is the record's shape, not a fit.

**Full-aft-stick pull by fuel**, 461 kt (Mach 0.7), 1,000 ft, clean, afterburner,
3 s (`PROBE_ONLY=turn PROBE_ALTS=1000 PROBE_FUEL=...`), against the F-15E, the F-5E,
the Gripen and the Mirage F1, all re-run on this tree:

| Fuel | Tornado G (deg/s) | F-15E G (deg/s) | F-5E G (deg/s) | Gripen G (deg/s) | Mirage F1 G (deg/s) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Full (11,221 / 13,662 / 4,400 / 5,000 / 6,120 lb) | 6.17 (13.1) | 7.57 (17.1) | 5.98 (12.8) | 8.00 (18.6) | 6.14 (13.4) |
| Half (5,610 / 6,831 / 2,200 / 2,500 / 3,060 lb) | 6.59 (14.2) | 8.28 (18.9) | 6.49 (14.2) | 8.52 (20.0) | 6.59 (14.6) |
| Quarter (2,805 / 3,416 / 1,100 / 1,250 / 1,530 lb) | 6.82 (14.7) | 8.69 (19.8) | 6.78 (14.9) | 8.80 (20.7) | 6.84 (15.3) |
| Near empty (300 lb) | 7.04 (15.3) | 9.10 (20.8) | 7.00 (15.5) | 9.03 (21.3) | 7.06 (15.9) |
| Top G row | +7 (fitted) | +9 | +7 | +9 | +7 (retail) |
| Full forward stick, same four | -2.63 / -2.81 / -2.91 / -3.01 | -2.51 / -2.74 / -2.88 / -3.02 | -2.55 / -2.77 / -2.89 / -2.99 | -3.55 / -3.78 / -3.90 / -4.01 | -2.61 / -2.80 / -2.91 / -3.00 |
| Tornado, default load (56,982 lb gross at full fuel) | 5.29 / 5.59 / 5.76 / 5.91 | | | | |

The Mirage F1 (empty 15,542 lb, one Atar at 11,020 / 15,873 lbf after B5-2) keeps
its retail rows up to +7. The Tornado's full-fuel pull is 12 percent under its +7 row: the 11,221 lb tank is 36
percent of the 31,065 lb empty weight, so the shared loaded-elevator rule takes
off more than from the Gripen (11 percent, tank 34 percent of empty) and less than
from the F-5E (15 percent, 46 percent). Near empty it gives 7.04 G, a hair above
the +7 row, as the +9 aircraft pass theirs (9.03 to 9.10); the F-5E, whose row is
hard against the limit, stops at exactly 7.00. The real limit is +7.5 G, 0.5 G above
the whole-G top row (John's B5-2 decision); at no fuel state does the model reach
it. At 330 kt the four fuel states give 4.39 / 4.69 / 4.86 / 5.01 G (-1.75 to
-2.97 G on the push), held down by the slow edge of the +5 to +7 rows. The default load also rises as
fuel burns: 5.29 G full to 5.91 G near empty at 461 kt. A payload sweep at full fuel
at 461 kt and 1,000 ft: 0 lb 6.17 G (sustained 4.82 G), 5,000 lb 5.84 G (4.81 G),
10,000 lb 5.54 G (4.14 G), 14,696 lb 5.29 G (4.14 G).

### Fuel and endurance

The record's flows are 3 lb/s at military power and 22 lb/s at afterburner (10,800
and 79,200 lb/h), or 0.624 and 2.463 lb/lbf/h against the engines' thrust (the
RB199's published 0.598 dry and 2.5 reheat, rounded to whole lb/s). The 11,221 lb
tank lasts 62.3 minutes at military power and 8.5 minutes at afterburner (burn
measured: 3.00 and 22.00 lb/s over 300 s); with the default load's 3,300 lb of
external fuel (feeds first) the same flows give 80.7 and 11.0 minutes. A level cruise
at 65 percent of the top speed (726 kt clean at 1,000 ft, throttle trimmed) burns
2.54 lb/s, which is 73.7 minutes or 891 nautical miles (extrapolated from 300 s);
with the default load 723 kt, 2.76 lb/s, 67.9 minutes, 818 nautical miles from the
internal tank. The `sprint` maneuver burns 6,600 lb in 300 s at afterburner (869.5
kt at 5,000 ft). Maneuver `climb` ran the tank to zero (124,540 ticks) and ended
without a crash.

### Takeoff, approach and landing

Ground probes at `--theater UKR --ground-start 1` with the default load (gross
56,982 lb; `loading=0.834`, flapped stall 120.4 kt after the weight scaling,
minimum level speed with flaps 152.5 kt):

| Probe | Result |
| --- | --- |
| takeoff | liftoff tick 1,813 at 150.6 kt after 1,945 ft (593 m), rotation 130.8 kt, 100 ft at tick 2,643 (196.6 kt) |
| takeoff, `--retail-stall-speeds` | liftoff 113.4 kt after 1,083 ft (imported polygon speeds, no weight scaling) |
| takeoff-gear-airborne | same liftoff, gear raised after the wheels are clear |
| land, 65 percent fuel (7,293 lb internal, tanks 3,212 lb) | approach 160.1 kt (the 1.05 times 152.5 kt floor), touchdown 158.7 kt at 7.6 ft/s sink, stopped 2,482 ft (757 m) after touchdown, 1,621 ft of the 5,532 ft runway left, score 100 |
| land, full fuel | approach 160.1 kt, touchdown 159.4 kt at 9.7 ft/s, stopped 2,494 ft (760 m) after touchdown, 1,720 ft left, score 100 |
| land-hard | touchdown 166.4 kt at 34.0 ft/s, stopped, score 100 |
| land-gear-up, land-off-runway | crashed for the gear and for the not-landable ground, by design |
| nosewheel | no crash, 11.2 kt maximum |

The record has no arrestor hook (`hook_available=false`; the devices probe reports
violations 0 and 3.00 s for the gear, flaps and brakes). The landings carry the full
default load (bombs and tanks), because the ground starts cannot drop it; a clean
approach was not measured.

Eleven air maneuvers: nine ended with `non_finite=0` and no loss (`level` 7,200
ticks at 574.6 kt, 65 percent of the 5,000 ft edge; `pull` 6.17 G, loop completed;
`loop` completed at 3,342 ticks; `sprint` 36,000 ticks, 869.5 kt, 0.984 of the edge;
`rudder`; `devices` violations 0; `stall-recover` alert at tick 1,405, recovered at
1,916, 0 ft lost; `spin-recover` entered at tick 3, recovered at 1,249 after 0.58
revolutions of recovery and 2.13 in all, 2,227 ft lost; `climb` zoomed to 64,966 ft
at 438 s and glided on after the fuel ran out, `crashed=false`), `dive` and
`overspeed` ended in the overspeed loss by design (1.198 and 1.100 of the edge).
Unlike the F-5E, the Tornado's `climb` does not end in an overspeed loss: its fast
edge falls slowly above the ceiling. Logs under
`.local/m3-aircraft/checks/TOR-8/probes/`.

### The swing wing (finding, nothing changed)

Real wing sweep changes a lot (general aerodynamics, not a cited Tornado figure):
spread (25 degrees) it gives the Tornado its low landing and take-off speeds and
its lift for manoeuvring; swept (67 degrees) it
cuts wave drag for the Mach 2.2 and the low-level dash it was built for, and trades
away lift (a swept wing has a lower lift slope and a lower usable lift
coefficient). TORE's model has **no sweep term**: stall speed, the G rows, the drag
law and the thrust are one set of numbers per aircraft (the retail record's
polygons, scaled by the shared weight rules), and nothing reads the sweep angle
that TOR-3 and TOR-5 draw (spread to 250 kt, 67 degrees at 600 kt, flaps hold the
wing out; `swing::wing_turn`). The drawn sweep at the probe's points is 25 degrees
at takeoff and approach (flaps out), 34 degrees at 330 kt, 50 degrees at 461 kt,
66 degrees at 593 kt and 67 degrees above 600 kt. Because the drawn sweep is a pure
function of speed, any real sweep effect would be a function of speed only; the
Tornado's own envelope rows (its slow and fast edges per G) already describe the
aircraft over that speed range as the retail data authors it (a "blended" wing),
so a sweep term would add nothing the polygons do not hold, apart from the
pilot-selected sweep (a real pilot chooses 25, 45 or 67 degrees; TORE's
speed-scheduled sweep takes that choice away). Where it would show: the 330 kt
pull (4.39 G, the record's slow edge) would be higher for a pilot who sweeps 45
degrees and lower for one who sweeps to 67; a spread-wing low-speed envelope with a
lower slow edge (and the dash at 67 degrees with a lower drag) would need new
polygons per sweep or a sweep multiplier on the slow edge and the drag, both fitted
per-aircraft changes, not made here. Finding for John: whether to leave the sweep
visual only (the current choice, John's decision 19 asks for the rig and the
pylons only) or fit a sweep multiplier.

Update (TOR-11, 2026-11-12): the wing now starts to sweep at 304 kt (350 mph, John), so
the drawn sweep at the probe's speeds is 25 degrees below 304 kt, 28.7 degrees at 330
kt, 47.3 degrees at 461 kt, 66.0 degrees at 593 kt and 67 degrees from 600 kt. The
flight probes themselves are unchanged (nothing reads the sweep).

### Against the real aircraft

Public figures read 2026-11-11 (Tornado IDS unless noted): Wikipedia, "Panavia
Tornado", Tornado GR4 specifications (maximum speed Mach 2.2, 2,400 km/h = 1,300 kn
at 30,000 ft, 1,482 km/h = 800 kn IAS near sea level; service ceiling 50,000 ft;
30,000 ft in less than 2 minutes from brake release; wing area 286 sq ft; thrust to
weight 0.77; combat radius over 850 nmi hi-lo-hi, over 500 nmi lo-lo-lo; wing sweep
25 to 67 degrees); aerospaceweb.org, "Tornado IDS data" (Mach 2.2, 1,450 mph at 36,090
ft; Mach 1.2, 915 mph at sea level; ceiling 50,000 ft; empty 31,065 lb, normal
take-off 45,000 lb, maximum take-off 60,000 lb; internal fuel 14,000 lb); MILAVIA,
"Specifications Tornado" (more than Mach 2.2, 2,337 km/h at 11,000 m; ceiling more
than 50,000 ft; Mk 103 8,650 lbf dry and 16,075 lbf reheat; empty 31,065 lb, maximum
61,620 lb); Migflug, "Panavia Tornado" (Mach 2.2 at 9,000 m with the intake ramps
working, which were inhibited on most airframes, capping them near Mach 1.9; Mach
1.3 at sea level as the Bundeswehr's figure; take-off run 900 m at typical strike
weight, wings at 25 degrees; landing run 370 m with thrust reversers; G limit
+7.5 / -3); Dinger's Aviation Pages (30,000 ft in less than two minutes). Other
pages read give a rate of climb of 76.7 m/s (15,100 ft/min) for the GR4; **no
page read gives 30,000 ft/min**, nor any approach, take-off or stall speed, nor a
sustained or instantaneous turn rate (one search returned only game forums).
None of these pages is a flight manual; the Mk 103 thrust differs between them
(8,650 lbf dry in MILAVIA, 9,850 in Migflug, 9,220 for the Bundeswehr), and the
B5-2 figures are kept.

| Quantity | Measured clean | Measured, default load | Retail record (corrected) | Real (cited) |
| --- | --- | --- | --- | --- |
| Top speed at altitude | 1,321 kt (Mach 2.30) at 36,000 ft | 1,266 kt (Mach 2.21) | 1,369 kt (Mach 2.38) at 36,000 ft | Mach 2.2 (1,262 kt at 36,090 ft), ramps working; about Mach 1.9 on most airframes |
| Top speed at sea level | 793 kt (Mach 1.20) at 1,000 ft | 760 kt (Mach 1.15) | 806 kt at sea level, 821 kt at 1,000 ft | Mach 1.2 (795 kt, 915 mph); Mach 1.3 (Bundeswehr); 800 kn IAS (Wikipedia GR4) |
| Service ceiling | 60,000 ft held in level flight; zoom 62,108 to 64,966 ft | not run | 60,000 ft | 50,000 ft (more than) |
| Climb | 30,000 ft in 91 to 101 s; 25,400 to 26,000 ft/min sustained; 406 ft/s peak excess power (24,400 ft/min) | 30,000 ft in 120 to 134 s; about 17,000 ft/min | | 30,000 ft in less than 120 s from brake release (not from 450 kt); 15,100 ft/min (76.7 m/s) quoted for the GR4 |
| Acceleration | 363 to 590 kt in 26 s, 1,000 ft | 38 s | | not found |
| Thrust | 17,300 dry, 32,150 AB, 2 engines | same | same | 17,300 / 32,150 (MILAVIA Mk 103); 19,700 / 34,500 (Migflug) |
| Thrust to weight | 0.76 | 0.56 (0.52 at maximum) | | 0.77 (Wikipedia GR4, weight not stated) |
| G limits | +6.2 (+7.0 nearly dry) / -2.6 at 461 kt | 5.3 (5.9 nearly dry) | +7 / -3 | +7.5 / -3 |
| Instantaneous turn | 13.1 deg/s at 461 kt (6.17 G); 11.9 deg/s at 330 kt (4.39 G) | 10.8 / 9.7 deg/s | | not found |
| Sustained turn (scripted, lower bound) | 4.82 G, 10.2 deg/s at 461 kt, 1,000 ft | 4.14 G, 9.4 deg/s | | not found |
| Liftoff | not run (ground starts carry the default load) | 150.6 kt, 1,945 ft (593 m), 56,982 lb | | take-off run 900 m (2,950 ft) at typical strike weight |
| Approach / touchdown | not run | 160.1 kt approach, 158.7 kt touchdown, 757 m rollout (no reversers modelled) | | landing run 370 m (1,215 ft) with thrust reversers; no approach speed found |
| Empty weight | 31,065 lb | | 31,065 lb | 31,065 lb equipped (30,620 lb basic) |
| Maximum take-off | | 56,982 lb gross | 61,620 lb | 61,620 lb (MILAVIA), 60,000 lb (aerospaceweb) |
| Internal fuel | 11,221 lb | + 3,300 lb external | 11,221 lb | about 14,000 lb (aerospaceweb), 10,280 lb (the game's encyclopedia) |
| Dry fuel flow | 10,800 lb/h (0.624 per lbf) | | same | 0.598 per lbf |
| Afterburner fuel flow | 79,200 lb/h (2.463 per lbf) | | same | 2.5 per lbf |

Findings (no value was changed):

- **Top speeds need no fit and are close to the cited figures.** Sea level agrees
  with Mach 1.2 (793 kt against 795). The altitude figure is 4.7 percent over the
  published Mach 2.2 clean (Mach 2.30) and 0.3 percent over it with the default
  load, which also matches the cited note that most real airframes were held to
  Mach 1.9 by inhibited intake ramps.
- **The ceiling is the record's 60,000 ft, 20 percent over the real 50,000 ft
  service ceiling.** Level flight holds to 60,000 ft (837 kt) and the zoom peaks
  at 62,108 to 64,966 ft. B5-2 corrected thrust, weights and G, not the ceiling.
  Option for John: lower `maxAlt` and the top vertices of the 0 G and 1 G polygons
  to about 50,000 to 55,000 ft the way B5-2 moved the F-5E's.
- **Climb is on or above the published figure.** 30,000 ft in 91 to 101 s clean
  from a 450 kt sea-level start (120 to 134 s with the default load) against "less
  than two minutes" from brake release, so about even with the default load and
  quicker clean. The 25,400 to 26,000 ft/min sustained rate is 85 to 87 percent of
  the 30,000 ft/min figure in the brief, which no source read gives; the published
  one found is 15,100 ft/min. The shared drag law that left the Mirage at 45
  percent and the F-5E at 55 to 58 percent cannot be read across: those were
  compared with 34,500 to 50,000 ft/min figures, and the Tornado's published figures
  are much weaker (15,100 ft/min, 30,000 ft in under two minutes), so the model is
  1.7 times the sea-level figure found.
- **G follows the rule.** The +7 row is never passed by more than 0.04 G; the
  pull is 6.17 G at full fuel, 7.04 G nearly dry (real +7.5), and 5.29 to 5.91 G
  with the default load. The F-15E, Gripen and F-5E move the same way with the fuel.
  The -3 G row gives -2.63 to -3.01 G.
- **High-speed and low-speed G are the retail rows.** The +5 G row ends at
  24,000 ft and the +6 G row (a repeat of the -3 G row) and the fitted +7 row at
  14,000 ft, so the pull is 4.4 G at 15,000 ft and 3.5 G at 30,000 ft; the slow
  edge puts the corner speed at 340 to 370 kt (the pull at 330 kt is 4.39 G).
  John may want these rows reshaped; not done.
- **No thrust reverser in flight.** The rollout (757 m) is twice the published
  370 m landing run, which assumes thrust reversers; the record, the flight model
  and the shape have no reverser (TOR-0). The model's liftoff (593 m) is shorter
  than the published 900 m take-off run, which is for a typical strike weight with
  the wings at 25 degrees and full flap and slat. A fitted reverser would be a
  decision for John.
- **The default load lowers every figure.** 56,982 lb is 35 percent over the clean
  weight and 7 percent under the maximum; it costs 33 kt at sea level, 55 kt at
  36,000 ft, 4 s on the 363 to 460 kt acceleration and about 0.9 G of pull.
- **Fuel flows are close to the published TSFC.** The record's whole-pound flows
  give 0.624 and 2.463 lb/lbf/h against 0.598 and 2.5. Endurance is 62 minutes dry
  and 8.5 at afterburner from the internal tank. The internal fuel (11,221 lb) is
  20 percent under aerospaceweb's 14,000 lb and 9 percent over the encyclopedia's
  10,280 lb; sources disagree, the record stands.
- **No published sustained or instantaneous turn rate, stall speed, take-off or
  approach speed was found**, so the Tornado's turns, liftoff and approach are
  reported, not compared.
- **Sweep.** See "The swing wing" above: no sweep term.

**Which numbers depend on John's stations answer (TOR-5).** Everything under
"default load" does: the carried weight (14,696 lb; the two tanks, four laser
bombs, two Mk 82 and the gun), the gross (56,982 lb), the liftoff speed and ground
run, the approach, touchdown and rollout (the landings carry the whole load), the
default-load climb, acceleration, top speeds and turns, the pull by fuel with
stores, and the endurance with external fuel. A lighter load moves them along the
payload sweep above (for example 5,000 lb: 5.84 G pull and 4.81 G sustained at 461
kt). The clean figures, the record envelope, the ceiling, the G by fuel table, the
fuel flows and the sweep finding do not depend on it.

### Battery windows (fitted, agent decisions 2026-11-11)

`flight-liftoff-tornado`, `flight-liftoff-retail-tornado` and
`flight-approach-tornado` join the battery with `INTERNAL_FUEL_LB["tornado"] =
11221`, a liftoff window of 135 to 165 kt and an approach window of 140 to 175 kt
(a fleet plausibility band, the width of the F-5E's: no primary figure was found for
the IDS), and the recorded default-load values 150.6 and 160.1 kt as the plus or
minus 6 percent regression windows. `flight-variety-tornado` and
`flight-animation-tornado` were already in the battery.

The battery: `python3 tools/battery.py --scenario '*-tornado*' --lane flight
--profile .local/m3-aircraft/profile-tornado --windows 1` passed 6 of 6 (run
`20261111-002325`), including `flight-liftoff-tornado`,
`flight-liftoff-retail-tornado`, `flight-approach-tornado`, `flight-variety-tornado`,
`flight-animation-tornado` and `render-tornado-skin`.

Provenance: record values and G rows spec-derived (retail); thrust, weights, the +7
row and the fuel flows opinionated (John, 2026-11-10, B5-2 and B5-3); the
measurements agent findings of 2026-11-11; the liftoff and approach battery ranges
fitted agent decisions of 2026-11-11; the real figures are public pages (Wikipedia,
aerospaceweb.org, MILAVIA, Migflug, Dinger's Aviation Pages), for comparison only,
which disagree among themselves and are not flight manuals. Nothing here is
attributed to John beyond the B5-2 and B5-3 corrections.

Not run: `--native-flight-tables` (tables not present locally), a windowed free
flight (the slice asks for no frames), a clean-load takeoff or landing, a
primary-source takeoff or landing run, a better-pilot sustained turn, the default
load's ceiling and sustained rate.

Update (TOR-10, 2026-11-11): the clean and default-load figures were re-measured
with the same probes after B5-4's +8 row and 50,000 ft ceiling and with the
lighter default load (one centreline tank, two AIM-9Ms: 13,228 lb carried,
`PROBE_PAYLOAD=13228`, 55,514 lb against 56,982), and the tables are in the
[Tornado acceptance record](aircraft-tornado.md#flight-envelope). Clean, what
moved: the pull by fuel 7.05 / 7.52 / 7.79 / 8.04 G (was 6.17 / 6.59 / 6.82 /
7.04) and the instantaneous turn at 461 kt 7.05 G and 15.7 deg/s (was 6.17 G,
13.1); top speeds, climb, acceleration, sustained turn, endurance and push are
unchanged. The ceiling: level flight now stops at 50,000 ft (1,135 kt there, was
1,156 on the record's polygon), a level start at 60,000 ft cannot hold altitude,
and the zoom climb peaks at 55,514 to 57,910 ft (was 62,032 to 62,108); TOR-8's
level flight to 60,000 ft at 837 kt no longer exists. Default load: top speed 763
kt at 1,000 ft and 1,272 kt at 36,000 ft (was 760 / 1,266), peak Ps 294 ft/s (was
285), 30,000 ft in 117 to 131 s (was 120 to 134), acceleration 363 to 590 kt in
36 s (was 38), pull 6.13 / 6.49 / 6.68 / 6.87 G (was 5.29 / 5.59 / 5.76 / 5.91),
sustained 461 kt turn 4.14 G at 9.3 deg/s (was 9.4), instantaneous turn 6.13 G and
13.2 deg/s (was 5.29 G, 10.8), liftoff 147.2 kt after 1,805 ft (was 150.6 kt after
1,945 ft), approach 156.2 kt, touchdown 155.3 kt, rollout 725 m (was 160.1 kt and
757 m). The default-load rows above (two tanks) stay as the morning's record. Logs
`.local/m3-aircraft/checks/TOR-10/`.

## Boeing B-52H validation, 2026-11-15

B52-8. Measured with `crates/tore-sim/examples/performance_probe.rs` (new options
`PROBE_CLIMB=bomber`, `PROBE_SPEEDS_KT`, `PROBE_HOLD_SPEED`, `PROBE_CRUISE=ALT:KT`) on
the corrected record, at 497,197 lb (full tank), 488,000 lb and 400,000 lb, and with
`tore-app --headless-flight`. The script is
`.local/m3-aircraft/checks/B52-8/run_probes.sh before|after`; logs are beside it. The
before and after table with its sources is in
[the B-52 flight review](../formats/aircraft-variety.md#b-52-flight-review-b52-8-2026-11-15).

- Sustained level turn, speed held, bank at which the aircraft still holds altitude
  (before to after): at 400,000 lb, 1,000 ft 250 to 350 kt 60 to 60 degrees (2.0 G);
  15,000 ft 250 kt 34 to 60 degrees; 300 to 350 kt 60 to 60; 400 kt 44 to 44; 439 kt (the
  top-speed edge) 42 to 42; 36,000 ft 400 kt 60 to 60 and 440 to 480 kt 42 to 42. At
  497,197 lb, 1,000 ft 250 kt 28 to 54 degrees; 15,000 ft 250 kt none above 8 to 24; the
  rest within 2 degrees.
- Level acceleration at 488,000 lb, military power, peak specific excess power: 17 to 62
  ft/s at sea level, 49 to 89 at 15,000 ft, 97 to 61 at 36,000 ft (the last falls because
  the lapse height is lower; it settles at 515 kt both times).
- `--headless-flight 1200`: 289.4 kt level at 5,000 ft (was 274.1), no crash, no
  non-finite value, fuel 312,197 to 312,057 lb.
- Takeoff (`--maneuver takeoff`, ground start, `--flight-fuel` for the lighter weights):
  488,000 lb 7,674 ft at 158.6 kt on UKR airport 1 (5,532 ft runway) and 7,324 ft at 158.7
  kt on GRE airport 1 (12,000 ft); 416,600 lb 4,843 ft at 147.8 kt; 301,600 lb 2,081 ft at
  125.4 kt. Landing (`--maneuver land`, GRE 1): 270,000 lb approach 140.3 kt, touchdown
  131.6 kt, rollout 4,131 ft; 404,000 lb approach 166.2 kt (the cap), touchdown 153.7 kt,
  rollout 5,056 ft.

## Tupolev Tu-95 Bear-H validation, 2026-11-16

T95-8. Measured with `crates/tore-sim/examples/performance_probe.rs` (the B-52H's `PROBE_CLIMB=bomber`,
`PROBE_SPEEDS_KT`, `PROBE_HOLD_SPEED`, `PROBE_CRUISE=ALT:KT`) on the corrected record at 346,565 lb (full tank),
388,894 lb (the default load of 16 AS-16) and 300,000 lb, and with `tore-app --headless-flight`. The scripts are
`.local/m3-aircraft/checks/T95-8/run_probes.sh before|after` and `run_ground.sh before|after`; logs are beside
them. The before and after table with its sources is in
[the Tu-95 flight review](../formats/aircraft-variety.md#tu-95-bear-h-flight-review-t95-8-2026-11-16).

- Level top speed, military power, full tank / 300,000 lb (before to after): sea level 295 / 311 to 333 / 340 kt,
  15,000 ft 332 / 349 to 413 / 422, 25,000 ft 357 / 375 to 467 / 476, 36,000 ft 384 / 404 to 432 / 441.
- Climb, schedule 300 KCAS then Mach 0.77, military power: best sustained rate 1,530 / 1,560 / 1,499 to 2,175 /
  1,805 / 2,713 ft/min (full, 388,894 lb, 300,000 lb); 5,000 ft in 167 / 196 / 138 s (was 751 s, none, 362 s);
  highest altitude 9,611 / 130 / 15,653 to 40,395 / 39,616 / 40,476 ft.
- Fuel: 400 kt at 25,000 ft throttle trimmed burns 2.29 lb/s (was full throttle, 3.99 lb/s, 359 kt held), so
  7,174 nautical miles from the 148,150 lb tank at constant weight; 380 kt at 30,000 ft 2.19 lb/s, 7,134.
- Sustained level turn, speed held, full tank (before to after): 1,000 ft 200 kt 64 to 64 degrees, 250 kt 66 to
  66, 300 kt 30 to 66 (2.49 G); 15,000 ft 250 kt 66 to 64, 300 kt 58 to 66, 350 and 400 kt none to 64 and 52;
  25,000 ft 300 kt 62 to 66, 350 kt 40 to 64, 400 and 450 kt none to 52 (1.63 G) and 50.
- `--headless-flight 1200`: 265.0 kt level at 5,000 ft (was 244.7), no crash, no non-finite value, fuel 148,150
  to 148,122 lb. `--maneuver roll`: 23.6 deg/s. `--maneuver gcurve`: the full-stick limit never falls below
  1.67 G after reaching 2.5 G.
- Takeoff (`--maneuver takeoff`, ground start): 388,894 lb 7,417 ft at 137.1 kt on GRE airport 1 (12,000 ft) and
  7,762 ft on UKR airport 1 (was 3,535 and 3,697 ft at 137.0 kt); 342,329 lb 4,650 ft at 128.7 kt (was 2,498 ft at
  128.6). Landing (`--maneuver land`, GRE 1): 337,041 lb approach 158.1 kt, touchdown 135.0 kt, rollout 4,429 ft
  (was 131.9 kt, 1,373 ft); 292,744 lb approach 147.4 kt, touchdown 127.1 kt, rollout 4,136 ft (was 124.3 kt,
  1,248 ft).
- AI heavy-class scenarios: `ai-heavy-takeoff-tu95-gre-a1` and `ai-heavy-land-tu95-ukr-a2` pass (touchdown 121 kt,
  parked at 603 s with 3.7 s off the pavement in the taxi-in).

