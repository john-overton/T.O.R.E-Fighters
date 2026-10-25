# Bridge placement and asset audit

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-25, branch `bridges-audit` from `cf36587a`.
The [behavior specification](../spec/bridges.md) owns the fitted rules and
remaining unknowns. This pass uses the task's freshly imported profile.
The import identifies FA.EXE 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
No retail executable was run. No retail comparison was available.

## Placement census

The imported pack contains 75 layouts across sixteen theaters. The baseline
`--bridge-audit` counted 624 placed bridge pieces, including repeated pieces
in alternate layouts. It found 511 with road or ramp samples more than 1 ft
below terrain or sea level. Twelve bridge types are present. This is a count
of placement records, not 624 distinct physical crossings.

| Theater | Layouts checked | Bridge pieces | Buried before |
| --- | ---: | ---: | ---: |
| APA | 2 | 0 | 0 |
| BAL | 10 | 135 | 135 |
| CUB | 2 | 0 | 0 |
| EGY | 11 | 0 | 0 |
| FRA | 12 | 72 | 48 |
| GRE | 2 | 0 | 0 |
| IRA | 2 | 0 | 0 |
| KURILE | 2 | 0 | 0 |
| LFA | 2 | 0 | 0 |
| NSK | 2 | 0 | 0 |
| PGU | 2 | 0 | 0 |
| SPA | 2 | 0 | 0 |
| TVIET | 2 | 132 | 128 |
| UKR | 10 | 285 | 200 |
| VLA | 10 | 0 | 0 |
| WTA | 2 | 0 | 0 |

| Type | Layout placements | Buried before |
| --- | ---: | ---: |
| BR1END.OT | 8 | 8 |
| BR1MID.OT | 4 | 0 |
| BR2END.OT | 12 | 12 |
| BR2MID.OT | 6 | 6 |
| BR3END.OT | 68 | 68 |
| BR3MID.OT | 34 | 34 |
| BRD1.OT | 31 | 31 |
| BRD2.OT | 45 | 45 |
| BRD3.OT | 44 | 44 |
| BRD4.OT | 15 | 15 |
| BRDEND.OT | 238 | 238 |
| BRDMID.OT | 119 | 10 |

After bottom grounding, connected-piece support and shared terrain grading,
all 624 layout placements clear the audit's burial and support limits.
At the measured lowest geometry, 781 final rows touch their support plane
and 81 joined BR3MID rows retain an 8 ft source-part gap. END ramp toes can
also sit 8 ft above their tower feet. These are explicit fitted limits,
not a claim of perfectly flush road joints.

The four offered bridge-bearing templates are also exercised at heavy defenses,
seed 1: QBBRD, QTBRDG, QTTRUCK and QUBRI. Including their theater scenery,
the final command checks 862 placements in 79 cases. It reports zero buried
decks, zero unknown decks and zero support gaps above 30 ft.

The whole-source origin rule was the first cause: the lowest geometry lies
below the origin in all twelve types. The second was independent placement
heights on long crossings. Review also found that the suspension span's
flat colour-6 shadow was included in solid bounds. Bridge bounds now exclude
that decal, preventing a false hit in the air below the deck. Raising every piece to the highest local support
left two Ukraine assemblies suspended over their low banks. The shared
corridor avoids that. Source angles, scales and horizontal locations remain. Contact sheets of
146 distinct type/position/heading combinations were reviewed. The bridges
follow the painted approach roads and cross the depicted rivers; no heading
correction was indicated. This is visual inspection, not a pixel-level
survey of artwork alignment.

The table's overlap column contains conservative solid-box candidates,
including intentional END/MID joins and ROAD4 approaches in France. It is
not a polygon intersection verdict. Painted roads and rivers have no decoded
vector alignment contract. Source headings are preserved; exact artwork
alignment remains a visual review, not a claimed numeric pass. No separate
riverbed depth is supplied by the imported terrain.

## Asset census

This census reads the imported pack directly. The older `--surface-dump`
supplements its records from retail archives, which could otherwise conceal
an import gap. Every shape and every intact/broken texture below is present
and readable in the fresh pack. All types use category `0x0100`, weapon damage
class index 2, explosion 35 and crater 0.

| Type | Shape | HP | Intact / broken faces | Damage branch | Textures |
| --- | --- | ---: | ---: | --- | --- |
| BR1END.OT | BR1END.SH | 500 | 42 / 42 | false | _BR1END.PIC |
| BR1MID.OT | BR1MID.SH | 500 | 18 / 27 | true | _BR1MID.PIC, _BR1MIDD.PIC |
| BR2END.OT | BR2END.SH | 500 | 30 / 30 | false | _BR2END.PIC |
| BR2MID.OT | BR2MID.SH | 200 | 17 / 32 | true | _BR2MID.PIC, _BR2MIDD.PIC |
| BR3END.OT | BR3END.SH | 500 | 36 / 36 | false | _BR3END.PIC |
| BR3MID.OT | BR3MID.SH | 50 | 18 / 19 | true | _BR3MID.PIC, _BR3MIDD.PIC |
| BRD1.OT | BRD1.SH | 400 | 151 / 116 | true | _BRD1.PIC, _BRD1D.PIC |
| BRD2.OT | BRD2.SH | 500 | 171 / 145 | true | _BRD2.PIC, _BRD2D.PIC |
| BRD3.OT | BRD3.SH | 300 | 89 / 73 | true | _BRD3.PIC, _BRD3D.PIC |
| BRD4.OT | BRD4.SH | 600 | 198 / 181 | true | _BRD4.PIC, _BRD4D.PIC |
| BRDEND.OT | BRDEND.SH | 100 | 28 / 28 | false | _BRDEND.PIC |
| BRDMID.OT | BRDMID.SH | 100 | 31 / 12 | true | _BRDMID.PIC, _BRDMIDD.PIC |

No bridge asset needed importing or wiring. Eight span types already select
their internal broken branch at zero HP. Four end types have no such branch
and retain their intact geometry. Partial health uses the intact copy. There
is no additional bridge ruin, crater or named DEBRIS.SH reference to activate.
The generic DEBRIS.SH exists in retail, but its presence alone does not prove
that bridge destruction uses it. No falling animation, convoy rerouting or
river blockage behavior is claimed. The specification records those unknowns.

Explosion 35 uses GRNDLRG.PIC, varying to 36/GRNDLRG2.PIC or 37/GRDLRGA.PIC.
All three last two seconds, with nominal sizes 400, 400 and 380 before the
existing size variation. Their sounds use the existing big-explosion pool.
The existing effect renderer adds SMOKE.PIC shockwave dust. Bridges have no
persistent wreck fire or smoke. Weapon impacts have their own effects and marks.

## Evidence and validation

Local evidence is in
`/home/john/Development/T.O.R.E-Fighters/.local/next-round/0.2.2-rc1/bridges-audit/`.
`before/placements.tsv` and `after/placements.tsv` contain the per-placement
tables. `assets.tsv` in each directory contains the complete type inventory.
Captures in `before/` and `after-final/` are named `LAYOUT-ID-intact.png` and `LAYOUT-ID-destroyed.png`.
The final capture run also records `damaged` at half health for each type.
There are 1,248 before images, eight before-template assembly images and
1,752 after images. Every span image changes on destruction; every END
image remains identical. All 28 half-health views, covering all twelve types, are identical to
their intact counterparts. Live and replay geometry matched throughout.
`before-templates/` holds the original mission-template assembly views.
The scratch `report.md` lists checks, exact image paths and integration notes.
No pictures, geometry exports, source bytes or other retail derivatives are
committed. The reusable commands are in [development](../DEVELOPMENT.md).

The new tests cover connected support, independent crossings, authored height,
sea-level clamping, interior ridge samples, terrain/contact agreement and
intact/partial/destroyed/reversed-replay drawing. `bridge-audit` is the fast
battery subset. `a2g-bridge-destruction` fires a real AGM-65G at QUBRI and
compares recorded render snapshots. An initial approach along the bridge hit
and destroyed an intervening END piece; a broadside approach destroyed the
intended MID piece. This was expected obstruction, not a damage-state fix.

Final Linux validation passed: formatting; workspace clippy with warnings
as errors; workspace build; 7,160 Rust tests (106 ignored); 500 Python tests
(5 skipped); repository and app/extract binary asset checks; documentation
check; and the wrapped GPU smoke test on an NVIDIA RTX 4070. Final build,
test, audit and capture commands recorded zero exit codes.

Nine selected battery scenarios passed: `bridge-audit`,
`a2g-bridge-destruction`, `surface-grounding`, `surface-grounding-sites`,
`surface-ground-pads`, `surface-resolve-all`, `a2g-bomb-motion`,
`a2g-maverick` and `net-surface-loopback`. The bridge strike destroyed its
100 HP span and matched 1,267 recorded render snapshots with zero missing
or differing frames. Its solid box has centre height 568 ft and vertical
half-size 24 ft; the shadow-inflated box had centre height 296 ft and
vertical half-size 296 ft after the initial footing correction.

The corrected scene also changes the old northbound visual-designation
probe in `a2g-maverick`: Enter now chooses the restored bridge behind the
column (`0x40000085`) and the shot strikes an intervening bridge end
(`0x40000091`). The test approaches at 90 degrees instead, retaining its
original unobstructed column-shot purpose. It selects `0x50000001` and
reduces it from 200 HP to 20. This is a test setup change; no missile
implementation was modified. The original setup passed on the saved
baseline, so this difference was traced rather than dismissed as flaky.

The multiplayer HP snapshot, session recording and app recording paths already
carry bridge damage. Replay seeking derives the same destroyed-ID set from HP.
No wire, replay or checkpoint fields changed. Shared terrain, target positions
and reconstructed old-recording geometry do change. Version decisions belong
to the integration lead; no version was bumped on this branch.
