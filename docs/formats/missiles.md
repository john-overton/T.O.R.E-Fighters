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
release without one and retains normal release gates; a bay opens on the trigger.

The [manual-supported behavior](../spec/missiles.md#manual-supported-behavior)
is separate evidence from this build's static records. Its presentation and
weapon-family prose must not silently override per-record signatures, timers or
support flags. Exact HUD geometry, sample mapping and any conflicting family
classification require a focused resource/consumer review during implementation.

## Release, sag and cruise profile

Research mode, 2026-09-28, static review of the objdump listing of FA.EXE
(sha256 prefix e31560c2, 1,319,424 bytes). The current JT is copied to `_cpt`
0x50d268, so fields read at absolute addresses: flags 0x50d30e, igniteT
0x50d367, fuelT 0x50d369, cruise bytes 0x50d373..0x50d376. Positions are 1/256
ft; the projectile clock 0x5528c8 counts quarter seconds. Behaviour is in
[the spec](../spec/missiles.md#drop-launch-sag-and-cruise-profile).

| Evidence | Finding |
| --- | --- |
| Motor state 0x4c1170 | Before igniteT: unlit. Before fuelT: powered. Then burnt out. fuelT counts from launch, so burn is fuelT − igniteT. |
| Turn rate 0x478101..0x478139 | Zero while unlit, poweredTurnRate while powered, unpoweredTurnRate after burnout. |
| Sag 0x4c14cf..0x4c1569 (`FallState` in `combat.rs`) | Flag 0x4 and motor not powered: a sink subtracted from altitude apart from the flight path, growing 32 ft/s² to a 0x5000 fixed8 (80 ft/s) cap. Powered resets it to zero. The 32 ft/s² assumes fixed8-second service ticks; that unit is inferred. |
| Ejection `_PROJAdd` 0x4c0e08..0x4c0e27 | Flag 0x8 starts the sink at 0x2000 fixed8, a 32 ft/s downward kick. All 0x…4f and 0x…6f air-launched records carry it; SEA_SPAR (0x45) and SSN9 (0x65) have 0x4 without 0x8. |
| Cruise caller 0x4c147f..0x4c148f | Runs only while the projectile has a target. Flag 0x20 selects 0x4c1660; otherwise 0x4c1630 homes on the target directly. |
| Cruise 0x4c1660 | Horizontal distance only (target copied with the missile's own altitude, 0x4c66cc, `shr 16`: 256-ft steps). Below cruise2Dist: direct. From cruise2Dist to cruise1Dist: cruise2Alt. At or beyond cruise1Dist: cruise1Alt. The aim point is directly over the target at target altitude + Alt × 256 ft, issued every service through 0x463a20 (flags 8, mode 2). |
| Speed, flag 0x40 (0x4c1494..) | Unlit holds speed, powered heads for maximum speed, burnt out heads for finalSpeed. |
| Motor smoke 0x4c123f..0x4c12c9 | Starts at ignition and lasts fuelT − igniteT. |
| Removal 0x4c1215..0x4c122c | At removeT or above 100,000 ft. |

Cruise bytes in the retail catalog are nonzero exactly where flag 0x20 is set:
AIM54C, AAML and PL10 [78,20,78,20]; AGM84A/E [78,4,20,12]; AM39 [78,4,20,1];
AGM65A/G, AS7, AS14, AS15, AS16, AS30, AT2, AT12 [29,4,29,4]; SSN9 [59,8,10,1].
igniteT is 8 (2 s) for AIM54C, AA9, AIM7, PL10, AGM84A/E and AM39; 4 (1 s) for
AA6, AIM7E, AGM65A/G and the AS set; 12 (3 s) for AT12; zero for AAML, AIM9 and
AIM120. launchRetard is 100 in every record.

Unresolved: how movement steers to the 0x463a20 point (points at it, or climbs
to its height); whether an unlit missile loses speed to drag; the service tick
time unit behind 32 ft/s²; the random launch-distance percentage that flags
0x1000 and 0x2000 set at 0x4c0e48..0x4c0eb5; any launch position offset. Next
step: trace 0x463a20's mode-2 consumer in projectile movement.

## Laser and designator

Research mode, 2026-10-10, slice R2 of the air-to-ground round; same build,
listing and units as the [A-G page](radar.md#a-g-page). Nothing was executed.
While a projectile is the current object, `_curThing` gives its launcher +0xe2
at 0x50cf62, its target +0xe4 at 0x50cf64 and its state +0xde at 0x50cf5e; its
JT is copied to `_curThingType` 0x50d268, so flags read at 0x50d30e (JT +0xa6),
the seeker starts at JT +0xac and the seeker signature reads at 0x50d31c. JT
values below come from `tore_formats::weapons::Weapon` over the retail records.

### Records

| Records | Data |
| --- | --- |
| GBU10, GBU10A, PAVEWAY, PAVEWA3, GBU28 | Signature 1, flags 0x2a543: guided 0x1, 0x100, designator 0x400, speed command 0x40; no gravity 0x4 and no bomb flag 0x10. Final speed 278 ft/s, deceleration 14, no motor, `launchRetard` 90, turn rate 6370 powered and unpowered. Zone 0 90 degrees (16380) to 50,000 ft; zone 1 45 degrees (8190), 0 ft (GBU10, GBU10A) or 50 ft to 30,000 ft |
| AS14, AS30, AT12 | Signature 1, flags 0x2a56f: the same guidance bits on a powered, cruising missile |
| PAVESPK, PAVEKNF (pods) | Signature 1, zone 0 all round (32767 by 32767) to 60,760 ft |
| SU24L, SU37L, KA50L, AV8L (built in) | Signature 1, zone 0 60 degrees (10920) to 60,760 ft |

### The lock rule

`PROJLock` 0x4c2f20 takes the JT, the launcher, the target, the hardpoint and a
launch flag. For these records it requires, in order:

| Step | Evidence | Rule |
| --- | --- | --- |
| 1 | 0x4c3014..0x4c3024 | The target is active |
| 2 | 0x4c3027..0x4c3054 | Any of 0x100, 0x200, 0x400: the launcher is active (alive) |
| 3 | 0x4c30e7..0x4c3113 | 0x400: with the launcher made current, `HARDBestSeeker` 0x452e60 with signature 1 and weapons excluded must find a sensor whose count is nonzero (0x452df0). That is, the weapon's own launching aircraft carries a working laser designator |
| 4 | 0x4c3121..0x4c3155 | `PROJInFOV` 0x4c2860 with the weapon's own seeker, zone 1, about the mount direction at launch or the weapon's nose in flight: heading and pitch each within 45 degrees (0x4c2ad1..0x4c2aeb). At launch also the zone-1 range and altitude, with the range scaled by the laser signature; in flight angles only |

The designator is never checked for range, field of view or line of sight: the
only signature-1 queries in the executable are step 3 and a generic search
(`PROJSelectTarget` 0x4c41a7, signature 0xff), and nothing reads a designator's
zones. There is no laser spot object and no laser on or off state. A laser
weapon homes on its target object and needs its own launcher, alive, to carry
a designator. Another aircraft's designator never helps: **retail has no buddy
lasing** (plan decision 6). Buddy lasing, and a display that shows a spot is
someone else's, are therefore opinionated additions (John, 2026-10-10).

### Release, flight and loss

| Evidence | Finding |
| --- | --- |
| Release, `PROJFire` 0x4c21f4..0x4c2213 | Runs the lock with launch = 1; on failure the weapon leaves with no target. A laser weapon released by an aircraft without a designator, at a target outside its 45 degree zone-1 cone, or beyond its scaled range, flies unguided |
| Flight, 0x4c12ce..0x4c135c | While it has a target the projectile reruns the lock (launcher, target, no launch checks) every 2 quarter seconds, 0.5 s |
| Loss, 0x4c1469..0x4c1470 and 0x4c1720 | On failure `PROJSetTarget` 0x4c0870 clears the target and `CreateMove` 0x463a20 holds the current heading, pitch and bank. Neither the flight loop nor the retarget routine (which skips a weapon without a target, 0x4c3e2e) gives it a new one, so the loss is permanent: the launcher dies, loses its designator, the target dies, or the target leaves the 45 degree cone |
| Damage message | "LASER SEEKER DAMAGED" (0x4ec53c) sits in the damage message table at 0x4ecb5c beside "IR SEEKER DAMAGED" and "RADAR DAMAGED" |

### Target switch

`PROJRetargetMissiles` 0x4c3dd0 is called from `NPCSetStateTarget` 0x4736d6
only for the player (`_curId` equals `_playerId`), only when the new target is
nonzero and differs from the old one (0x4736b7..0x4736d2). It walks every live
projectile owned by this computer (0x4c3de8..0x4c3e9f) and acts on one that is
guided (0x4c3e1c), launched by the player (0x4c3e25), has a target other than
the new one (0x4c3e2e..0x4c3e3f), has not begun its terminal hit roll (state
bits 0x18 clear, 0x4c3e41; bit 0x8 is set when the roll starts, 0x4c3277), and
has signature 1, or signature 3 with 0x200 (0x4c3e4a..0x4c3e5f). It reruns the
lock on the new target: success retargets the weapon (0x4c3e7c..0x4c3e7e),
failure clears its target and flies it straight (0x4c3e85..0x4c3e8c).

So switching designation sends the player's laser weapons in flight to the new
target if it lies inside their 45 degree cone, and blinds them otherwise.
Clearing the designation (;) does not call the routine, so the weapons keep
guiding on the old target. AI launchers never retarget. The manual's "switch
targets and the bomb will fall blindly" (M:7988 to 7992) matches only the
failing case.

### Weather and night

| Evidence | Finding |
| --- | --- |
| Signature, `COSig` 0x478249..0x47824c | A laser seeker uses the target's `sigs[1]` (type +0x41; 100 on retail ground units) |
| Weather, 0x478390..0x4783a7 and `WRWeatherEffects` 0x4b4720..0x4b4784 | The signature is scaled by the minimum, over the weather layers spanning the launcher's and the target's altitudes, of the layer byte at +0x14e + signature. The selector is the seeker signature: 0 visual, 1 laser, 2 IR, 3 radar, 4 anti-radiation. Retail selector-1 values are 50 in `CLOUD1` inside the deck and in `FOG1` low, 100 elsewhere ([weather selectors](weather.md#effect-selectors)) |
| Effect | Range is compared after scaling by 100 / signature, so a GBU-10's 30,000 ft release limit becomes 15,000 ft when the line from launcher to target spans cloud or fog. The test runs at release only; in flight the lock skips range and signature, so weather never breaks a lock already made |
| Night, 0x47840d..0x478480 | For visual and laser seekers, a target whose type flags carry 0x1000000 has its signature divided by 1 within 1,500 ft rising to 5 at 4,500 ft and beyond, before 07:00 (0x6270 s) or after 19:05 (0x10c5c s). No retail NT record carries the flag |

### Unknowns

| Unknown | Next step | Fallback |
| --- | --- | --- |
| Whether "LASER SEEKER DAMAGED" zeroes the count step 3 tests | Trace the damage writer that indexes the table at 0x4ecb34 | The designator works while its aircraft lives |
| How a guided bomb moves after losing its target | 0x4c1720 holds attitude; whether movement then adds a fall to a record without 0x4 or 0x10 is R1's bomb motion (`PROJMoveProc` 0x4c11b0) | R1's fitted bomb fallback, so the bomb falls ballistically |
| Which objects carry type flag 0x1000000 | Scan the OT and PT records' type flags | No night rule |

## Optical

Same build and listing. "Optical" is the retail label for every guided
signature-0 record.

| Evidence | Finding |
| --- | --- |
| Label, 0x41a5b4..0x41a62a, table `sigName` 0x4ee800 | Load Ordnance shows "unguided" without flag 0x1, "SARH" for signature 3 with 0x200, otherwise `sigName[signature]`: 0 "optical", 1 "laser", 2 "IR", 3 "active rdr", 4 "passive rdr". GBU29, GBU29P, GBU30 and AT2 are the guided signature-0 records, so all four read "optical" |
| JDAM records | GBU29, GBU29P, GBU30 flags 0x2a043: the laser bombs' 0x2a543 without 0x100 and 0x400. Same movement and zones as the laser bombs (zone 1 50 to 30,000 ft, 45 degrees) |
| AT2 | Flags 0x2216f: guided, 0x100, cruise 0x20 with bytes [29, 4, 29, 4], gravity 0x4, ejection 0x8, speed command 0x40. Zone 1 500 to 18,000 ft, 45 degrees |
| Lock | The [lock rule](#the-lock-rule) without step 3. A JDAM carries none of 0x100, 0x200, 0x400, so only target alive and the zone-1 test apply: fire and forget. AT-2 also needs its launcher alive (0x100) for the whole flight |
| Steering, 0x4c147f..0x4c148f, 0x4c1630, 0x4c1660 | The projectile steers at its target object, not a stored point: 0x4c1630 issues a `CreateMove` toward the target id every service; AT-2's flag 0x20 selects the cruise profile 0x4c1660 instead. A moving target is followed |
| Visibility, `COSig` 0x478249..0x4783f8 | Signature 0 is the target's visual `sigs[0]` with weather selector 0 (visibility: `DAY2` night 25, dawn and dusk 75, below cloud 75, inside cloud 10, low fog 10). Below 75 the percent is raised toward 75 near the target: 75 within 200 ft, blending back over 200 to 1,500 ft (0x4783ad..0x4783f8). At release only, so a JDAM's 30,000 ft limit becomes 7,500 ft at night and 3,000 ft in cloud or fog |
| Target switch | Signature 0 is not retargeted (0x4c3e4a..0x4c3e5f): a JDAM or AT-2 keeps its release target |
| No target | Released without a target the weapon has no guidance. JDAMs lack the bomb flag 0x10, so the unguided-bomb release check (`PROJMakeBombEq` 0x4c3eb0 impact within max(range / 8, 1,000 ft) of the target, 0x4c3163..0x4c31cb, see R1's bomb sight) does not apply to them |

Retail answer to plan decision 4: there is no GPS or coordinate guidance. The
JDAM records are optical, fire-and-forget bombs that home on the object
designated at release, follow it if it moves, need no designator, and have
their release range cut by darkness, cloud and fog. A bomb that flies to a
stored ground point in all weather is an opinionated design.

| Unknown | Next step | Fallback |
| --- | --- | --- |
| What a player sees when firing an optical weapon with no target (refused or released unguided) | Trace the player's fire path ahead of `PROJFire` and the HUD readiness for signature 0 | Released unguided, as `PROJFire` allows |

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
