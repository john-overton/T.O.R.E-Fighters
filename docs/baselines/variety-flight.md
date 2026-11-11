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
