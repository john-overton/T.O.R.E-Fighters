# Aircraft radar behaviour

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-16. This partial specification describes the inspected FA
build, not real-world radar performance. No retail execution or complete parity
claim. [Source findings](../formats/radar.md), [validation](../baselines/radar.md).
The [component proposal](../radar.md) separates our implementation choices from
these findings. Unknowns below do not block implementing the specified portions.

## Aircraft capabilities

Use the radar installed in the aircraft PT hardpoints. The following twelve
bindings were checked directly against FA records. Do not substitute avionics
from a real-world aircraft of the same name, or impose a capability by year.
One nautical mile is 6,076 feet in these records.

| Aircraft | Radar record | Nominal search nmi | Nominal track nmi | Look-down coefficient | Highest TWS scope setting nmi |
| --- | --- | ---: | ---: | ---: | ---: |
| F/A-18D, F18.PT | F18R, APG-65 | 90 | 50 | 0 | 50 |
| Rafale C, RAFALE.PT | F18R, APG-65 | 90 | 50 | 0 | 50 |
| F-14D, F14.PT | F14R, AWG-9 | 190 | 150 | 30 | 150 |
| A-4E, A4E.PT | F4BR, APQ-72 | 50 | 25 | 50 | 25 |
| X-31 EFM, F31.PT | F18R, APG-65 | 90 | 50 | 0 | 50 |
| MiG-29 Fulcrum-C, MIG29.PT | MIG29R, Slot Back | 62 | 43 | 0 | 25 |
| Su-27 Flanker-B, SU27.PT | SU27R | 150 | 115 | 0 | 100 |
| MiG-21 Fishbed, MIG21.PT | MIG21R, Jay Bird | 55 | 40 | 50 | 25 |
| Su-25 Frogfoot-A, SU25.PT | SU24R | 50 | 40 | 0 | 25 |
| MiG-23 Flogger-B, MIG23.PT | MIG27R, High Lark | 55 | 40 | 50 | 25 |
| Su-35, SU35.PT | SU27R | 150 | 115 | 0 | 100 |
| F-22A, F22.PT | F22R | 150 | 150 | 0 | 150 |

Names are source labels, not an assertion that these aircraft had those real
radars. FA leaves several radar names blank; their filenames identify them here.
The last column is derived from the confirmed range ladder and mode rule below,
not an extra equipment value or a continuous tracking-distance limit.

All nine distinct radar records have horizontal and vertical search half-angles
of 60 degrees and track half-angles of 45 degrees. Minimum ranges are zero;
altitude bounds use unlimited sentinels. Nominal maxima are modified by target
signature and look-down. A scope setting does not extend a sensor's coverage.

## Target signatures

The target aircraft's PT carries relative signatures separately from the sensor
installed on the observing aircraft. Radar uses sigs[3]; IR uses sigs[2]. These
are game scale values, not established square metres of radar cross section or
physical infrared output. The component proposal uses 100 as its reference.

| Target aircraft | Radar signature | IR signature |
| --- | ---: | ---: |
| F/A-18D | 100 | 100 |
| Rafale C | 50 | 70 |
| F-14D | 100 | 100 |
| A-4E | 120 | 100 |
| X-31 | 80 | 90 |
| MiG-29 | 100 | 100 |
| Su-27 | 100 | 100 |
| MiG-21 | 100 | 100 |
| Su-25 | 100 | 100 |
| MiG-23 | 100 | 100 |
| Su-35 | 100 | 100 |
| F-22A | 10 | 60 |

These are FA base-record values. They do not establish the complete original
aspect, configuration, power-setting or weather modifiers. They nevertheless
provide distinct per-aircraft inputs for the proposed authored detection model.

## Scope range and targeting modes

The normal FA range ladder is **5, 10, 25, 50, 100, 150 nautical miles**. Range
controls stop at either end. Radar reset selects index 1, the 10-mile setting.
The range label, contact projection and picking must agree on the selected scale.
The F-14's 190-mile sensor rating does not add a 190-mile display setting.

With own-aircraft radar selected, the scope uses TWS when the selected display
range is at or below the installed sensor's nominal tracking range. It uses RWS
above that threshold. Equality selects TWS. This is automatic; the inspected
selector has no separate era test or TWS-capability flag test.

Examples: F/A-18D changes from TWS at 50 to RWS at 100. A-4E changes from TWS at
25 to RWS at 50. MiG-29 is already RWS at the 50-mile setting despite its
43-mile nominal tracking range. F-14D and F-22A stay TWS throughout the ladder.
Do not replace this with historical restrictions invented for older aircraft.

The normal target-cycle path refuses to acquire a radar target in RWS. A
supplemental-radar path is an exception whose operational conditions remain
unknown. Existing designation retention and missile support when changing to
RWS require follow-up; do not infer them from the acquisition rule.

The FA scope label table contains RWS, TWS, IR, HARM and A-G. This establishes
available display channels, not that every aircraft has every channel. The
current remake's M button cycling RWS/TWS/A-G is not established retail behaviour.
Exact M/Y handlers, mouse selection and track-history durations remain unknown.
The older USNF manual describes sensor switching and history, but is a lead,
not authority for FA-specific controls or numbers.

## Look-down and detection

The normal look-down branch applies only when the sensor is above the target
and the installed coefficient is nonzero. It takes the larger of two penalties:
one increases with downward sight angle, the other with proximity of the target
to terrain below it. The terrain-relative scale is 5,000 feet. Penalties are
bounded to 0..100 and reduce effective target signature. That signature affects
the effective distance tested against sensor range.

The source supports the following normal-branch interpretation, before special
flags and with helper units as documented in the source notes: at 22.5 degrees
downward and at least 5,000 feet target height above terrain, coefficient 50
produces a 25-point signature penalty; coefficient 30 produces 15. At 2,500 feet
above terrain, the height terms are also 25 and 15 respectively. Level or higher
targets skip this look-down branch. Coefficient zero skips its normal penalty.
These are static-analysis examples, not observed retail measurements.

This means the A-4E, MiG-21 and MiG-23 have a stronger normal look-down penalty
than the F-14, while the other inspected sets have none from this parameter.
It does not imply all-aspect perfect detection: target signature, terrain,
equipment state and other conditions still matter. Never interpret coefficient
50 as an unconditional 50-percent range reduction.

Special caller flags, a reduced-penalty target category, the complete signature
function and the low-signature cutoff remain unresolved. The standard component
can carry these as explicit policy boundaries without copying original internals.

The executable contains a Doppler branch. All nine inspected radar profiles
have zero Doppler parameters and do not set that branch's enable bit. Do not add
notching to these profiles based on real-world pulse-Doppler expectations alone.

## Other installed sensor channels

These are separate devices in the checked PT hardpoints, not radar upgrades.
The ranges below are nominal search/track, before their own modifiers.

| Aircraft | Additional sensor | Channel | Nominal nmi |
| --- | --- | --- | --- |
| Rafale C | AAS38 | Infrared | 15 / 15 |
| X-31, F-22A | AV8IR | Infrared | 15 / 15 |
| MiG-29 | MIG29I | Infrared | 9 / 10 |
| Su-27, Su-35 | SU27I | Infrared | 9 / 10 |
| Su-25 | SU24L | Laser/designator | 10 / 10 |

The 9/10 values are present in FA; preserve them rather than silently clamping
track to search or replacing the device. Initial acquisition versus retained
track for these sensors needs its own consumer review. All twelve aircraft also
carry visual sensors. HARM capability belongs to the selected weapon/equipment,
not a fabricated infrared/radar capability on every aircraft.

## Weapon coupling and remaining evidence

Reviewed PROJLock evidence distinguishes launch radar requirements from continued
illumination under weapon flag 0x200. A designation is not by itself permission
to fire, and active radar missiles must not all be treated as semi-active.
[Manual weapons](../baselines/manual-weapons.md) documents the current evidence.

Missing facts: exact mouse hit rules, history cadence, lock/loss timers, maintained
support after changing display mode or designation, active-seeker activation,
supplemental radar, IFF and detailed ground-target filtering. Next research should
inspect only the handlers needed for the next player interaction, then add its
observable rules here. Do not hold basic radar profiles or shared contact state
behind complete byte-level closure. No AI work is included.
