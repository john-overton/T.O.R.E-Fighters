# Missile classification and Fox calls

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-15, nr-fox from `06c68743`, protocol 32.
[Behavior](../spec/missiles.md#shared-classification).

## Inventory evidence

A bounded `tore-formats` probe read all 135 JT definitions in the installed
`FA_2.LIB` (SHA-256
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`). The installed FA_1, FA_4B, FA_4D and swpatch archives contain no JT
records. The surface-data import pack contains 135 JT definitions, byte-equal
to the existing retail extraction. The repo's aircraft additions, multiplayer
and AI loadouts resolve these same definitions; the supported-store allowlist
introduces no additional A2A identity. The 22 air-launched A2A candidates have
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
infrared, so the AA-10T HUD string conflicts with its radar fields and remains
an unresolved variant question. [MBDA MICA](https://www.mbda-systems.com/products/force-protection/vl-mica-family)
distinguishes active RF and passive IR seekers; the game's one MICA is radar.
[US Army R-73E](https://odin.t2com.army.mil/WEG/Asset/b772e6f643bfe928cd06ea994bc24031)
and [PL-7](https://odin.t2com.army.mil/WEG/Asset/PL-7_Chinese_Air-to-Air_Infrared_Homing_Missile)
confirm IR. No verified real-world identity is assigned to AAM-L or AEMP-1.
The old PL-10 name is ambiguous and must not be silently remapped to a modern
IR weapon. No performance constants were imported from these references.

## Validation

The slice report records commands, results, before/after recordings and guard
differences. Synthetic classification tests cover every inventory row; radio
tests cover renamed HUD labels and delivered active calls. The existing
support-loss test now explicitly exercises AIM-7 as well as AIM-120. The new
`replay-rec-fox-three` battery scenario ties an AIM-120 release to a delivered
`^FOXTHR` event in the exported recording. Replay sound uses the recorded stem,
and multiplayer transmits it through the existing name table and speech event.
No wire layout or protocol number changes, no retail bytes added.
