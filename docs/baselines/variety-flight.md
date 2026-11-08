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
