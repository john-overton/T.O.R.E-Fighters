# Missile record interpretation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-17. These are bounded record findings and unresolved
consumer meanings. [Proposed gameplay and inventory matrix](../spec/missiles.md),
[measured inventory pass](../baselines/missiles.md),
[existing weapon source notes](weapons.md).

## What the fields establish

`Weapon::parse` reads a signature selector, weapon flags, two zones, movement
parameters and a separate guidance block from each JT. The source record names
are identifiers, not evidence for a real-world variant or guidance mode.

| Data | Current live use | Limit of the evidence |
| --- | --- | --- |
| `sig` | Nonzero enables target guidance; 3 invokes radar support checks | 2 is the proposed IR group, 4 the emitter group; neither proves every weapon's full seeker behavior |
| `flags & 0x200` | Signature-3 weapons require launcher support during flight | Absence is not proof of an onboard active seeker or a pitbull transition |
| `zone1` | Launch range, relative altitude and angle permission | Launch envelope is not a kinematic reach guarantee |
| `zone0` | Seeker acquisition envelope, used for sensor and target search | Corrected 2026-09-17: in flight the weapon re-runs the lock routine with range checking off, so only `zone1` angular limits gate tracking; see the [AI source map](ai.md#seeker-signature-and-store-selection-follow-through). Geometry alone does not establish signature strength, acquisition delay, ECM rejection or emitter compatibility |
| `igniteT`, `fuelT`, `removeT` | Ignition age, motor cutoff age, cleanup age | Live host maps one timer unit to 0.25 seconds; retail scheduling equivalence is unestablished |
| `trackT` | Parsed as `Guidance::track_t`, unused by live combat | AI preparation-delay use is traced in [AI B42](../spec/ai.md#b42-weapon-preparation-search-cadence-and-firing); not evidence of battery life |

Reviewed motor consumer treats `fuelT` as a launch-relative cutoff. Burn duration
is the difference from ignition, not an additional duration after it. Existing
source evidence and build hashes are in the
[combat component baseline](../baselines/combat-components.md#native-arithmetic-evidence).
The compatibility adapter advances simulation at 120 Hz, obtains timer age from `tick/30`
and movement service increments from a separate 256-units-per-second accumulator.
These are distinct clocks; do not divide motor timer values by 120 or 256.

Raw `trackT` values among the 63 candidates are 4, 12, 20 and 40. AGM88 has 4;
AGM45 has 20, despite both being signature-4 records. This alone says nothing
about either weapon's usable guidance duration. Next research: inspect reads of
that exact field in FA PROJLockUpdate, PROJLock and their callers, using the
hash-gated static artifacts. Stop when the player-visible purpose and units can
be stated. No retail execution is required.

Angles retain source units, normally 182 per degree, with special wide-angle
values such as 0x7fff. Next implementation must review these cases and separate
horizontal and vertical gates. The current single-cone simplification does not
establish their behavior. Source geometry takes priority over invented era values.

## Compatibility and current launch adapters

`combat::launch_speed` scales the launcher scalar by source `launchRetard`, takes
the maximum with source `initialSpeed`, and clamps to source speed limits.
`commanded_speed` approaches an altitude-adjusted absolute speed while powered,
then source final speed in coast. Compatibility points the projectile along the aircraft basis. The current
launcher bridge carries both scalar speed and world velocity; compatibility
uses the scalar while the spec profile inherits the full vector.

The [new velocity/intercept specification](../spec/missiles.md#launch-velocity-and-intercept-estimates)
changes that behavior deliberately. Its additive boost budget reinterprets
source numbers as fitted inputs and must not be attributed to the reviewed
consumer. Compatibility retains the current rule. CUED readiness requires designation. BORESIGHT permits independent-seeker
release without one, opens an available bay and retains normal release gates.

The [manual-supported behavior](../spec/missiles.md#manual-supported-behavior)
is separate evidence from this build's static records. Its presentation and
weapon-family prose must not silently override per-record signatures, timers or
support flags. Exact HUD geometry, sample mapping and any conflicting family
classification require a focused resource/consumer review during implementation.

## Exceptions that must survive classification

- AA10 labels itself AA-10T but has signature 3 and flag 0x200. AA9 also has
  flag 0x200. The draft uses the supported-radar category from these fields,
  without replacing either with guidance inferred from its real-world name.
- AGM65A, AGM65G and AGM84E have signature 2. Their proposed IR grouping is an
  authored mapping of the game data, not a claim about real-world TV/IR variants.
- MICA has signature 3. No separate MICA-IR record appears in this catalog.
- AS14, AS30 and AT12 have signature 1. Keep them on hold as designator-channel
  candidates, rather than calling them IR or passive radar emitters.
- AT2 has signature 0. Guidance is unresolved; do not classify it as an IR weapon
  merely because it is a missile-shaped powered record.
- ASROC's special role, and SA19/SAN11's radar-control requirements, need consumer
  review. Their draft rows are on hold. In particular SA19 lacks 0x200 while
  carrying 0x100, so absence of the illumination flag is insufficient to promote
  it to an active seeker.
- SA14 has fuel/remove 88/80; SA6 has 92/80. Cleanup precedes motor cutoff under
  the existing host timing. Do not normalize those records silently.

For other starred rows, classification remains provisional. Next research is
weapon-specific review of support, target eligibility and seeker consumers, not
an attempt to copy their original control flow. Emitter weapons additionally
need explicit radar-versus-jammer eligibility. Reuse original record identities;
no name aliases, era substitutions or extra ordnance variants are implied.
