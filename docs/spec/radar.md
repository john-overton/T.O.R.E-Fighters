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
The [component guide](../radar.md) separates our implementation choices from
these findings. Unknowns below did not block implementing the specified
portions.

## Aircraft capabilities

Use the radar installed in the aircraft PT hardpoints. The F-22N (F22N.PT,
added 2026-09-22) carries the same sensors, ECM and stores as the F-22A rows
below. The following twelve
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
physical infrared output. The component guide uses 100 as its reference.

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
provide distinct per-aircraft inputs for the authored detection model.

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
remake's earlier M button cycling of RWS/TWS/A-G labels was never established
retail behaviour, and it is gone: M now selects an available sensor channel,
which is an authored control described in the [component guide](../radar.md).
Exact retail M/Y handlers, mouse selection and track-history durations remain
unknown. The older USNF manual describes sensor switching and history, but is a
lead, not authority for FA-specific controls or numbers.

## Target selection keys

John's recollection of retail, given on 2026-09-23. Retail comparison is
unavailable.

- **T** cycles through current radar contacts only. **Shift-T** cycles
  backwards.
- **Enter** selects an aircraft the pilot can see, but only one that is also a
  current radar or infrared contact.
- When the radar or infrared scope loses the contact, the target drops and the
  track is lost completely, however it was selected, including by clicking it
  on the scope. Seeing the aircraft does not keep it, and the HUD does not
  remember a dropped target. Only the [Easy targeting cheat](cheats.md) keeps
  it: the selection then stays set while the aircraft is off the scope, without
  radar support.

Clicking a contact on the scope still selects it at once, including a
search-only RWS contact, as John requested on 2026-09-16; it now drops with the
contact.

**Proposed**, as agent decisions awaiting John's review:

| Rule | Proposed |
| --- | --- |
| T order | nearest first; the order is taken again at each press |
| Skipped by T and Enter | friendly aircraft and wrecks |
| T with the infrared channel selected | selects nothing; infrared contacts are for Enter |
| Enter's view | the forward view at 1x zoom around the nose: 60 degrees tall and 4:3 wide |
| Enter's choice | the eligible aircraft nearest the nose |
| Gamepad | South (A) is T; Enter and Shift-T have no default button |

These replace an agent decision under which a selection survived radar loss
while the aircraft stayed inside the roughly 10-nmi visual envelope, and the
fitted HUD memory of a target after sensor loss.

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
track for these sensors needs its own consumer review. HARM capability belongs
to the selected weapon/equipment, not a fabricated infrared/radar capability on
every aircraft.

All twelve aircraft also carry a visual sensor, on source signature 0. Two
records are installed, and both rate 10 nmi search and 5 nmi track. Their
half-angles differ, and the record names match the full azimuth coverage.

| Visual record | Search / track nmi | Azimuth x elevation half-angles | Aircraft |
| --- | --- | --- | --- |
| VIS340.SEE | 10 / 5 | 170 x 140 degrees | F/A-18D, Rafale C, F-14D, A-4E, X-31, MiG-29, Su-27, Su-35, F-22A |
| VIS240.SEE | 10 / 5 | 120 x 90 degrees | MiG-21, Su-25, MiG-23 |

Search and track angles are identical within each record; only the distance
differs. These are the FA record values, not a claim about a pilot's real
field of view.

## Installed ECM records

Eight distinct ECM records are installed across the twelve aircraft. Two of their
fields bear on the radar picture: the mode flag word, whose bit 0x10 is the
reviewed radar-deception mode and bit 0x100 the infrared one, and the radar
deception chance.

| ECM record | Mode flags | Radar deception chance | Aircraft |
| --- | ---: | ---: | --- |
| F4.ECM | 0 | 0 | A-4E |
| MIG21.ECM | 0 | 0 | MiG-21, MiG-23 |
| F14.ECM | 0x1F0 | 30 | F-14D |
| MIG29.ECM | 0x1F0 | 30 | MiG-29 |
| SU24.ECM | 0x1F0 | 30 | Su-25 |
| F18.ECM | 0x1F0 | 30 | F/A-18D, Rafale C, X-31 |
| SU27.ECM | 0x1F0 | 30 | Su-27, Su-35 |
| F22.ECM | 0x1F0 | 50 | F-22A |

F4.ECM and MIG21.ECM carry neither the radar-deception mode flag nor a nonzero
chance, so the A-4E, MiG-21 and MiG-23 have no radar-deception capability in
their own records. The remaining six all share mode flags 0x1F0; only F-22A's
chance differs. The chance is a source probability field; the remake repurposes
it as a jammer strength input, which is an agent decision recorded in the
[component guide](../radar.md). The other ECM fields, chaff and flare counts,
signature additions and infrared terms, are documented with the
[weapons systems evidence](../baselines/weapons-systems.md).

## USNF history and infrared reference

The locally retained USNF manual describes Y, or the scope Y button, enabling a
trail of dots showing prior contact movement. It separately describes a heading
tail on a TWS contact; that is not the history trail. No trail count, sample
interval, persistence or fade schedule is specified in the inspected text.

The same manual describes passive IR selection through I or the on-screen M
button, with R returning to active radar. IR uses heat signatures and has shorter
coverage and weather limitations. This is USNF evidence, not proof of FA's exact
bindings, timings or aircraft equipment. The FA equipment table above remains
authoritative for installed devices. [Source location and identity](../formats/radar.md#usnf-history-and-ir-functional-reference).

John's requested authored scope includes histories, persistent click selection,
detectable destroyed aircraft, single-target tracking and IR air-to-air. Those
rules and their timings live in the [component guide](../radar.md). The retail
RWS acquisition restriction above remains a research fact, not a reason to reject
his requested click selection. RWS selection does not supply a fire-control lock.
The target view keeps gamified IFF; realistic IFF and A2G are outside this pass.

## Weapon coupling and remaining evidence

Reviewed PROJLock evidence distinguishes launch radar requirements from continued
illumination under weapon flag 0x200. A designation is not by itself permission
to fire, and active radar missiles must not all be treated as semi-active.
[Manual weapons](../baselines/manual-weapons.md) documents the current evidence.

The USNF manual describes active radar weapons as receiving a target at launch
and permitting the aircraft to break lock after firing. It describes IR weapons
as needing their own seeker lock at launch, without aircraft radar support, and
semi-active weapons as depending on continued launcher lock. This is a manual
functional reference, not proof of FA activation distances or every weapon's
classification. The requested plan preserves those per-weapon distinctions:
one aircraft track can support sequential launches at different targets when the
weapons guide independently. Each missile retains its own launch target.

Missing retail facts: exact mouse hit rules, history cadence, lock/loss timers,
maintained support after changing display mode or designation, active-seeker
activation, and supplemental radar. Each one has an authored replacement in the
[component guide](../radar.md), labelled there; none of them is recovered
behaviour. Detailed ground-target filtering is deferred with A2G;
IFF remains the existing gamified target view. Next research should
inspect only the handlers needed for the next player interaction, then add its
observable rules here. Do not hold basic radar profiles or shared contact state
behind complete byte-level closure. No AI work is included.

## Selected-contact bars and movement line

**Spec-derived, requested by John on 2026-09-21:** a selected current contact has
one vertical bar on each side, following the supplied radar image and the
[manual evidence](#air-and-surface-contact-symbols). Every moving current contact
in TWS has a line pointing in its observed horizontal direction of travel
relative to own-aircraft heading, without requiring selection or weapon lock.
RWS has no direction line. Up means
travel along own heading, right means travel to own right. Use observed velocity,
not aircraft nose direction or relative closing velocity. Stationary and stale
contacts have no movement line. History visibility does not affect this line.
Selection bars appear immediately, independently of weapon-track acquisition;
all current air contacts are filled, whether acquired or not. The track-status
text reports acquisition, and unselected hover corners remain distinct.

**Fitted, agent decision:** on the 160 by 156 instrument raster, each bar is
2 pixels wide and 7 high, centred vertically on the contact. Their left edges
are at contact x minus 7 and x plus 6. The direction line is 1 pixel wide and
9 pixels long from the contact centre, with rounded endpoints, in the contact's
colour. Its length indicates direction only, not speed. Exact retail dimensions
remain unknown; inspecting the FA contact drawing
handler is the next research step if those details are needed.

## Air and surface contact symbols

**Spec-derived:** the FA manual, chapter 4, printed page 97, describes aircraft
as small squares and illustrates filled aircraft symbols. Large surface targets
are single-pixel dots, not hollow squares. It describes TWS motion flags relative
to own heading and two vertical captain's bars around the selected target.
[Manual identity and inspection](../formats/radar.md#fa-manual-contact-symbols).
This establishes the documented presentation, not an executed-retail comparison.

**Fitted:** current air contacts use a solid 5 by 5 raster-pixel square regardless
of selection or acquisition. Stale contacts retain their dim cross. Direction
lines follow the TWS rule above. IR direction-line behavior is not established
by this passage and is left disabled as an agent decision.
Surface-symbol implementation remains deferred with A2G. The inspected passage
specifies large surface targets only; other ground-symbol distinctions remain
unknown and require inspecting the ground-radar examples before implementation.
