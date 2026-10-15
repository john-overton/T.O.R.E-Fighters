# Missile classification and Fox calls

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-15, nr-fox2 from `eeaeee96`, protocol 32 pending the lead's bump.
[Behavior](../spec/missiles.md#shared-classification).

## Inventory evidence

A bounded `tore-formats` probe read all 135 JT definitions in the installed
`FA_2.LIB` (SHA-256
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`). The installed FA_1, FA_4B, FA_4D and swpatch archives contain no JT
records. The surface-data import pack contains 135 JT definitions, byte-equal
to the existing retail extraction. The repo's aircraft additions, multiplayer
and AI loadouts resolve these same definitions; the supported-store allowlist
originally introduced no additional A2A identity. The current load-time correction
adds three virtual R-27 variants, with rules in the [spec](../spec/missiles.md#corrected-identities-and-r-27-stores). The 22 air-launched A2A candidates have
motor fuel, the guided flag and the air-target flag. SAMs, ASROC and the Moth
Balls special object were inspected and excluded from this A2A census.

All A2A seeker flags are `[0, 0]` except PL10's `[0, 4]`. There is no separate
active/semi-active enum in the JT: the reviewed support distinction is weapon
flag `0x200`. Raw signatures and weapon flags for every candidate are tested
in `weapons::tests::every_inventoried_air_to_air_missile_has_the_right_seeker_and_fox_call`.
This is a data census, not a retail gameplay comparison.

AIM-7 and AIM-7E, AIM-9B/M/X, AIM-54C, AIM-120, AA2, AA6, AA8, AA9,
AA10, AA11/B, AA12, AAML, AEMP1, MICA, PL7/10 and R530/550 are the complete
set. No separate R-27R/T/ET/ER, R-40T, MICA-IR, Python, Sky Flash, AIM-4,
AIM-26 or beam-rider definition was found. Their absence does not establish
that an unnamed original loadout represents such a variant.

## Current A2A table

Read through the corrected loader on nr-fox2. Guidance still comes from the
single `Weapon::seeker_kind` classifier. Aspect roles and fitted numbers are
specified in [missiles](../spec/missiles.md#infrared-aspect-and-acquisition).
Catalog-only entries have no newly enabled flight profile.

| Resource | HUD | Identity | Guidance / Fox | IR aspect | Availability / origin |
| --- | --- | --- | --- | --- | --- |
| AA10.JT | AA-10A | R-27R (AA-10A Alamo-A) | SARH / one | N/A | live, retail |
| AA11.JT | AA-11 | R-73 (AA-11 Archer) | IR / two | all-aspect | live, retail |
| AA11B.JT | AA-11B | AA-11B (game IR variant) | IR / two | all-aspect | live, retail |
| AA12.JT | AA-12 | R-77 (AA-12 Adder) | active radar / thr | N/A | live, retail |
| AA2.JT | AA-2A | R-3S (AA-2A Atoll) | IR / two | rear-only | live, retail |
| AA6.JT | AA-6A | R-40R family (AA-6A Acrid) | SARH / one | N/A | catalog only, retail |
| AA8.JT | AA-8 | R-60M (AA-8 Aphid) | IR / two | all-aspect | live, retail |
| AA9.JT | AA-9 | R-33 (AA-9 Amos) | SARH / one | N/A | catalog only, retail |
| AAML.JT | AAM-L | AAM-L | active radar / thr | N/A | live, retail |
| AEMP1.JT | AEMP-1 | AEMP-1 | active radar / thr | N/A | catalog only, retail |
| AIM120.JT | AIM-120 | AIM-120 | active radar / thr | N/A | live, retail |
| AIM54C.JT | AIM-54C | AIM-54C Phoenix | active radar / thr | N/A | live, retail |
| AIM7.JT | AIM-7 | AIM-7 | SARH / one | N/A | live, retail |
| AIM7E.JT | AIM-7E | AIM-7E | SARH / one | N/A | live, retail |
| AIM9B.JT | AIM-9B | AIM-9B | IR / two | rear-only | live, retail |
| AIM9M.JT | AIM-9M | AIM-9M | IR / two | all-aspect | live, retail |
| AIM9X.JT | AIM-9X | AIM-9X | IR / two | all-aspect | live, retail |
| MICA.JT | MICA-EM | MICA EM | active radar / thr | N/A | live, retail |
| PL10.JT | PL10-R | PL-10 (1997 radar concept) | SARH / one | N/A | catalog only, retail |
| PL7.JT | PL-7 | PL-7 | IR / two | all-aspect | catalog only, retail |
| R27ER.JT | AA-10C | R-27ER (AA-10C Alamo-C) | SARH / one | N/A | live, derived |
| R27ET.JT | AA-10D | R-27ET (AA-10D Alamo-D) | IR / two | all-aspect | live, derived |
| R27T.JT | AA-10B | R-27T (AA-10B Alamo-B) | IR / two | all-aspect | live, derived |
| R530.JT | S530D | Super 530D | SARH / one | N/A | live, retail |
| R550.JT | R-550 | R-550 Magic 2 | IR / two | all-aspect | live, retail |

## Speech evidence

The installed executable is 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
The existing `radio::phrases` reader validates every pointer pair, including:

| Stem | 1.02F pair address | Text | Decoded FA_2.LIB resource bytes |
| --- | --- | --- | ---: |
| `^FOXONE` | `0x4ff248` | Fox one | 3448 |
| `^FOXTWO` | `0x4ff250` | Fox two | 3432 |
| `^FOXTHR` | `0x4ff258` | Fox three | 4000 |

All three `#FOX*` alternate crew recordings also exist (6261, 6848, 6743
bytes respectively). The stem table needs no correction. This checks bounded
resource decoding and mapping, not an audible listening assessment. The 1.0
shift remains covered by the existing synthetic build-layout tests; no 1.0
retail executable was newly inspected in this pass.

## Real-world guidance references and limits

These references establish real-world categories, not retail behavior or our
fitted activation distances. The [USAF AMRAAM fact sheet](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104576/aim-120-amraam/)
identifies active terminal radar. [NAVAIR's Phoenix fact sheet](https://www.navair.navy.mil/node/12701)
describes semi-active, inertial/update and active phases. [US Army Sparrow entry](https://odin.t2com.army.mil/WEG/Asset/da4fc2fb0fc134f8cf1cc9f5e408038c)
identifies semi-active radar. [USAF Sidewinder history](https://www.aftc.af.mil/About-Us/History/On-This-Day-in-Test-History/Article-Display-Test-History/Article/2326942/september-26-2000-testing-the-sidewinder-coupled-with-joint-helmet-mounted-cuei/)
warns against family-only classification: AIM-9C used radar, unlike the IR
B/M/X records present here.

[Rosoboronexport RVV-AE](https://roe.ru/pdfs/pdf_2015.pdf) confirms active radar
with inertial updates. [R-27ET1](https://roe.ru/pdfs/pdf_4714.pdf) confirms
infrared. The former AA-10T HUD string conflicted with its radar fields;
AA10.JT now names the SARH R-27R, with separate T/ET stores. [MBDA MICA](https://www.mbda-systems.com/products/force-protection/vl-mica-family)
distinguishes active RF and passive IR seekers; the game's one MICA is radar.
[US Army R-73E](https://odin.t2com.army.mil/WEG/Asset/b772e6f643bfe928cd06ea994bc24031)
and [PL-7](https://odin.t2com.army.mil/WEG/Asset/PL-7_Chinese_Air-to-Air_Infrared_Homing_Missile)
confirm IR. No verified real-world identity is assigned to AAM-L or AEMP-1.
The old PL-10 name is ambiguous and must not be silently remapped to a modern
IR weapon. New R-27 envelopes are fitted, not measurements recovered from these references.

## Validation

The slice report records commands, results, before/after recordings and guard
differences. Synthetic classification tests cover every inventory row; radio
tests cover all 25 rows, renamed HUD labels and delivered targetless calls. The existing
support-loss test now explicitly exercises AIM-7 as well as AIM-120. The new
`replay-rec-fox-three` battery scenario ties an AIM-120 release to a delivered
`^FOXTHR` event in the exported recording. Replay sound uses the recorded stem,
and multiplayer transmits it through the existing name table and speech event.
The new catalog and checkpoint semantics require a protocol bump by the lead.
The slice leaves the number at 32. No retail bytes added.

## nr-fox2 focused evidence

Base eeaeee96 source rejects close launches, excludes launch-origin-close
contacts and hits, and holds targetless missile calls. Its head-on IR score is
strictly below 0.25 at nonzero range for a normal reference fighter, so it
cannot acquire. Boresight tone also incorrectly requires a predicted intercept.
These source checks are the before evidence, not a new retail run.

The three new recorded scenarios pass: empty-sky AIM-120 at tick 61 records
Fox three; the 0.2 nm AIM-120 shot acquires at 1,162.49 feet on tick 90 and
hits on tick 206; head-on AIM-9M search tone starts tick 1, lock tick 31,
release tick 61 at 17,455.54 feet and 179.40 degrees aspect, and hit tick 576.
All three run twice identically and pass render-state verification without
rendering or a GPU. The complete checks and guard differences are in the
slice's local report, with artifacts under `.local/tmp-nr-fox2`.
