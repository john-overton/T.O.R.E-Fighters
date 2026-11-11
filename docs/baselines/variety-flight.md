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
