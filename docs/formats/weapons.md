# Aircraft weapons: FA research

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature, see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


Research date: 2026-09-14. This schedules aircraft armament research and subsequent
importer/simulation work. It does not mark combat or vanilla parity complete.
[Measured extraction and code evidence](../baselines/weapons-research.md).

## Scope and fidelity contract

Recover the supplied Fighters Anthology game's internal guns, gun/rocket pods,
guided missiles, ballistic/retarded/guided bombs, cluster/special ordnance, tanks,
targeting equipment and countermeasures. Inventory the whole JT library so
aircraft compatibility research cannot silently lose alternatives. Ground/ship
weapons remain identified separately; importing their definitions does not
schedule ground/ship AI. Preserve unusual vanilla weapons and statistics even
when they differ from real-world specifications.

Missiles, bombs, rockets and bullets need movement models, not just graphics.
Acquisition, launch permission, guidance, motor/coast phases, fuzes, impacts,
damage and countermeasures must form one reviewed lifecycle. A missile following
an authored pursuit curve does not establish vanilla performance. Nor does a
real-world gun rate substituted for FA's representative-projectile accounting.

The target is unchanged vanilla gameplay performance: launch conditions, cadence,
ammunition, trajectories, range, guidance, sensors, damage and stores effects.
Preserve native arithmetic/order when it affects those outcomes. Keep rendering
independent of authoritative 120 Hz state. Verify native service-clock conversion;
never equate a render frame or Rust tick with one native update. Do not add
realism corrections, rebalance weapons or adopt the reference engine's fallbacks.

Current aircraft flight remains legacy/hybrid. Faithful projectile components
coupled to those adapters cannot establish whole-engagement parity until aircraft,
contact and native scheduling gates also pass. See [native flight](native-flight.md).

## Source hierarchy

1. User-owned FA archives and this exact FA.EXE/FA.SMS build, with hashes and
   bounded offsets. Source records establish values, not consumer semantics.
2. Static FA consumer/caller analysis, with unresolved branches and external
   state recorded. SMS names locate routines; names alone prove no behavior.
3. Matched original-game observations for end-to-end acceptance. The importer,
   app and tests never execute imported native code or drawing modules.
4. Ignored USNF-ATF format notes/readers as research leads. Their other-title
   addresses, hypotheses and custom engine are not FA parity evidence.

No original C/C++ source tree was identified in the supplied media/reference
checkout during this pass. Original code here means statically inspected retail
machine code and compiled asset modules. The reference gun exporter contains
authored ballistics/mounts alongside USNF-derived native fields; neither can be
copied wholesale as the FA specification.

## Initial exporter audit (before implementation)

| Area | Current result/code | Completion needed |
| --- | --- | --- |
| Whole weapons catalog | Shared resolver roots every JT with `--weapons`; 135 named JT analyses succeed | Classify all native branches; library membership is not aircraft compatibility |
| Aircraft bindings | 145 PTs contain 70 unique literal JT references; all 70 exported | PTS alternatives, compatibility masks, station pairing, racks/pods, availability and mass rules |
| App/CLI selection | App imports F18/Rafale with `weapons=false`; wrapper accepts one aircraft, Rust CLI repeated flags | Shared armament profile/cache validation and deliberate wrapper union support |
| Discovery | Token scan follows PT/JT/SEE/ECM/GAS/SH/HUD; exact names or `.PIC` suffix | Typed edges, aliases, generated names, executable tables and reason chains |
| Missing files | BRF JT/SH/SEE/ECM/GAS/11K/5K references fail when absent | Equivalent required PIC/nonliteral checks and reported optional/unknown edges |
| Weapon graphics | 75 SH, 61 PIC preserved | Native scales, mounts, LOD branches, animation and actual weapon rendering |
| Shared effects | Six graphics-initializer SH roots demonstrably absent | Add reviewed shared roots and follow their textures/audio/effect tables |
| Native code | JT preserves inert `_PROJProc` symbol, not its implementation | Separate hash-gated static weapon research and bounded Rust translations |
| Equipment | Named JT/SEE/ECM; GAS raw/BRF preservation | Checked typed configuration, resolved pointers and verified units |
| Provenance | Archive boundaries, hashes, bytes and named raw/scaled fields | Dependency edges, overrides, unresolved mappings and separate extraction/decode/runtime/parity statuses |

`--include` filters after closure and can remove required files. Mark these
exports filtered/incomplete. CLI dependency lookup takes the last matching archive
in input order, while extraction preserves matches from every archive. App import
reads FA_1/FA_2. Record duplicate/build selection before widening profiles; do not
silently prefer a patch/disc variant or imply every variant was reviewed.

### Confirmed gaps and FA checks

`_GRAPHICInit` (0x442c00) requests CRATER.SH, SMOKE.SH, FIRE.SH, DEBRIS.SH,
CHAFF.SH and FLARE.SH. All exist and none is in the measured weapons export.
Their module strings name CRATERS.PIC, SMOKE.PIC, FIREA.PIC and FLARE.PIC;
these also exist and are missing. The already-selected FIRE.PIC is different art.
Executable sound-string candidates missing from selection include &EXPL12.5K,
&SPLASH3.11K, &FIRE.5K, &CHAFF.5K and &FLARE.5K. Finish table/caller tracing
before mapping sounds to effect indices. Similar names alone are insufficient.

FA `_PROJSpeed` (0x4c1120 through return at 0x4c1163) takes launcher speed
shifted right eight, multiplies by unsigned JT launchRetard (+0x115), divides
by 100, takes the maximum with signed initialSpeed (+0xfb), then clamps to
signed _minSpeed/+0x67 and _maxSpeed/+0x6b. These offsets agree with the local
315-byte packed JT schema. It is scalar selection, not vector addition of
aircraft velocity. This is static arithmetic review, not differential execution.

M61 and DEFA both contain initial/final speed 2933/1466, actualRoundsPerGame=2,
gameBurstT=1 and removeT=40. Hornet capacity is 570; Rafale C is 250. FA
`_PROJFire` reads ammunition debit at +0xf0 before calling `_HARDUnload`.
The older USNF note uses +0xec. Trace FA player dispatch, burst/reload scheduling
and unlimited-ammo gates before assigning rounds/second or seconds to those
raw timing fields. Do not transplant USNF offsets or cadence claims.

## Implementation plan and status

The ordered W0–W5 implementation plan and its dated status log moved to
[the frozen weapons plan](../research/weapons-plan.md) on 2026-09-16. This file
keeps the recovered facts. Current sequencing is in
[the parity plan](../parity-plan.md); measured results are in
[weapons systems](../baselines/weapons-systems.md) and
[combat components](../baselines/combat-components.md).

## FA entry points for the next pass

SMS locations below are research starting points, not fully reviewed routines
or a complete call graph.

| Contract | Entry points |
| --- | --- |
| Loading/mount | HARDCanLoad 0x452980; HARDLoad 0x452c20; HARDPos 0x4532a0; HARDPodHack 0x453710; HARDStoreWeight 0x452940 |
| Fire/service | PROJAdd 0x4c0a90; PROJFire 0x4c2170; PROJServiceWeapon 0x4c4700; PROJProc 0x4c1f50 |
| Movement | PROJSpeed 0x4c1120; PROJEngineState 0x4c1170; PROJMoveProc 0x4c11b0; PROJBombPos 0x4c4050 |
| Seeker | PROJInFOV 0x4c2860; PROJLock 0x4c2f20; PROJLockUpdate 0x4c0960; PROJRadarIsOn 0x4c2eb0; PROJSelectTarget 0x4c4100 |
| Equipment/CM | HARDBestSeeker 0x452e60; HARDFindJammer 0x452ea0; HARDFindECMForObj 0x452f10; PROJLaunchDevice 0x4c39a0 |
| Damage | PROJHitChance 0x4c3380; PROJHit 0x4c20c0; PROJDamageProc 0x4c1870; PROJSendCollateralDamages 0x4c5670; DAMAGEDoHit 0x40f970 |
| Graphics/audio | GRAPHICInit 0x442c00; GRAPHICAddExp 0x4432d0; GRAPHICAddSmoke 0x443e80; GRAPHICAddDebris 0x4441d0; PROJFireSound 0x4c26f0 |

Do not classify the inventory symbol `_explode` as combat explosion code merely
from its name.

## Development live-fire adapter (subsequent implementation)

The user's subsequent live-fire request explicitly permits documented
approximations. `tore-sim::combat::live` is that connected development adapter;
it does not promote the diagnostic components to native-parity status.

Identity resolution follows `AircraftId::ALL`, the F18 and Rafale C model modules,
reviewed cockpit/exterior assets and each PT's actual hardpoint records. Only
F18.PT (F/A-18D) and RAFALE.PT (Rafale C) are supported. Live configuration resolves
PT weapon counts/mounts and JT movement/damage/sensor/effect fields once. The
Hornet has M61/570, AIM120/2, two AGM65G/4 groups and AIM9M/2; Rafale C has
DEFA/250, AGM65G/4, MICA/2, R530/2 and R550/2. These are **PT defaults**, not a
recovered mission-specific loadout preset or a new compatibility claim.

Source launch speed, motor states, axial acceleration/deceleration, altitude
performance, expiry, fall and ammo-debit arithmetic are reused. The connected
adapter advances at 120 Hz with an authored remainder conversion to 256-unit
service time and four-unit-per-second deadlines. Representative burst count
and per-projectile ammunition debit come from JT; burst grouping and input
service ordering remain approximate. No real-world RPM replaces FA data.

The explicit range's targets are scripted instances of the selected ported
aircraft, with source HP (116 Hornet; 100 Rafale). They do not return fire.
Designation refers to real target IDs; dead targets cannot lock or receive
another destruction. SEE range/FOV plus radar emission gate radar contacts and
radar-guided launch; JT launch/track zones gate guided weapons. The current
cone test, direct pursuit capped by source turn-rate fields, all-radar-weapons
illumination requirement and irreversible loss of track are approximations.
Native lead/PN, active/semi-active distinctions, signature strength, ground/air
eligibility, aspect/Doppler, terrain masking, sun, ECM and difficulty/RNG remain
open. AGM65G can engage the same range aircraft surrogate; this is not native
AGM65 target-class acceptance.

Swept relative-motion sphere intersection prevents round/target tunneling;
eight terrain samples plus bisection find the earliest sampled ground crossing.
This is not native polygon collision. Fuzes use source arm time/radius; damage
subtracts the source aircraft-class entry from source target HP. Native hit
probability, subsystem damage, immunity, collateral, debris and water effects
remain open. Ordinary free flight stays externally clean; explicit live range
loads the PT weapon counts and auxiliary external equipment mass. Tank fuel is
carried mass only, with no transfer/jettison. Released stores reduce payload
through the existing aircraft-owned model; rack pairing, weapon-specific drag
and carried-store rendering are not recovered.

See [live-fire validation](../baselines/live-fire.md) for controls, screenshots,
end-to-end results and presentation approximations. This is a working test range,
not a completed W3–W5 vanilla acceptance gate.

## Manual weapons integration follow-up

[Manual acceptance](../baselines/manual-weapons.md) supersedes the live adapter's
previous all-radar illumination rule, class-0-only fixture path, absent carried
geometry and combat-recording gap. The source category switch at 0x411470 is
translated; both PT categories are 0x8000 -> damage index 0. Five-class fixtures,
bounded nominal/applied hit history, failed-station high-bit semantics, shared
visual/radar acquisition, sampled terrain masking and manual jettison are wired.
R530 retains launcher radar during tracking; AIM120/MICA do not. Native activation,
lead/PN and whole-tick native parity remain unaccepted. The systems continuation
below supersedes this checkpoint’s open ECM/automatic-selection work with partial
translations; native RNG and complete subsystem effects remain open.
At this checkpoint static extraction emitted 22 reviewed regions, including category, amount and
station-failure spans; a reviewed span is not a complete native translation.
All 135 catalog JT files remain preserved; live acceptance covers the ten default
PT stations of the two ported identities, not alternative compatible loadouts.

## Player systems continuation

The static pass now includes 28 reviewed regions, including player capacity,
weighted subsystem selection, eligibility, ECM lookup/probability and equipment
damage. Runtime resolves both PT systemDamage tables and ECM once at startup.
[Exact translated contracts, integration and open native gates](../baselines/weapons-systems.md).
The 119 located symbol spans are not 119 fully translated routines.
