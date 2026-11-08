# Aircraft variety source registration

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation source review, 2026-10-05. This review registers 23 additional
exact retail PT identities. The [behavior plan](../spec/aircraft-variety.md)
owns the requested player behavior. Registration and extraction establish
reviewed data inputs; they do not establish retail gameplay parity.

## Source build and bounded review

The local installed FA media supplied FA_1.LIB and FA_2.LIB. Their SHA-256
identities are recorded below. PT, SEE, ECM and JT are read as bounded BRF data.
HUD, SH and PTS modules are inspected as data; no module is executed. All 23 PT
roots have the existing 219-token OBJECT + NPC + PLANE layout, with identity-
specific type sizes. Strict PT names, displayed names and type sizes are guarded
by the reader. Exact identities remain distinct, including F4.PT = F-4G.

| Archive | SHA-256 |
| --- | --- |
| `FA_1.LIB` | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| `FA_2.LIB` | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |

The extraction report is local-only at
`.local/aircraft-variety/extraction-report.json`. It records the archive provider,
entry offsets, resource hashes and dependency edges. The 23-aircraft union
extracted 598 resources, including 119 from FA_1 and 479 from FA_2, with zero
errors. The unimplemented native symbols and unresolved art candidates remain
explicit in the report. This is the selected reviewed dependency closure, not a
claim of complete native module coverage.

## Reviewed identity and presentation roots

A missing PTS is an explicit reviewed exception. Existing PTS companions remain
inert source evidence. Every shape family below supplies its own A/B/C/D damage
pieces. HUD and cockpit roots follow PT pointers when present. AC130, AV8 and
YAK141 have null PT HUD pointers: selecting their own same-name HUD and its own
art is a **fitted host selection**, an agent decision, not recovered retail
selection behavior. No other aircraft is substituted.

| PT | Displayed retail name | Type size | Shape stem | HUD | Cockpit family | PTS |
| --- | --- | ---: | --- | --- | --- | --- |
| `C130.PT` | C-130 Hercules | 468 | `C130` | `AC130.HUD` | `AC130` | Absent |
| `AC130.PT` | AC-130U Spectre | 612 | `AC130` | `AC130.HUD` | `AC130` | Present |
| `E3.PT` | E-3 AWACS Sentry (AIR) | 516 | `AWACS` | `AC130.HUD` | `AC130` | Absent |
| `IL76.PT` | IL-76 Mainstay (AIR) | 516 | `IL76` | `AC130.HUD` | `AC130` | Absent |
| `E2.PT` | E-2C Hawkeye (AIR) | 516 | `E2C` | `AC130.HUD` | `AC130` | Absent |
| `AV8.PT` | Av-8B Harrier II | 660 | `AV8` | `AV8.HUD` | `AV8` | Present |
| `YAK141.PT` | Yak-141 Freestyle-A | 588 | `Y141` | `YAK141.HUD` | `Y141` | Present |
| `V22.PT` | V-22 Osprey | 540 | `V22` | `AC130.HUD` | `AC130` | Absent |
| `AH64.PT` | AH-64 Apache | 636 | `APA` | `AC130.HUD` | `AC130` | Absent |
| `MI24.PT` | Mi-24 Hind-D | 516 | `HIND` | `SU33.HUD` | `SU33` | Absent |
| `CH47.PT` | CH-47 Chinook | 468 | `CH47` | `AC130.HUD` | `AC130` | Absent |
| `MIG17F.PT` | MiG-17F Fresco | 540 | `M17` | `MIG17.HUD` | `M17` | Present |
| `F4B.PT` | F-  4B Phantom II | 612 | `F4J` | `F4.HUD` | `F4` | Present |
| `F4J.PT` | F-  4J Phantom II | 636 | `F4J` | `F4.HUD` | `F4` | Present |
| `F4E.PT` | F-  4E (Desert) Phantom | 636 | `F4E` | `F4.HUD` | `F4` | Absent |
| `F4.PT` | F-  4G Wild Weasel Phantom | 636 | `F4` | `F4.HUD` | `F4` | Absent |
| `A7.PT` | A- 7E Corsair II | 612 | `A7` | `A7.HUD` | `A7` | Present |
| `F15.PT` | F- 15C Eagle | 588 | `F15` | `AV8.HUD` | `AV8` | Absent |
| `F16C.PT` | F- 16C Falcon | 660 | `F16` | `F16C.HUD` | `F16` | Present |
| `F104.PT` | F-104N Starfighter | 660 | `F104` | `F104_C.HUD` | `F104` | Present |
| `A10.PT` | A-10 Thunderbolt | 660 | `A10` | `F104_C.HUD` | `F104` | Absent |
| `B747.PT` | Boeing 747 | 468 | `B747` | `AC130.HUD` | `AC130` | Absent |
| `A310.PT` | Airbus 310 | 468 | `A310` | `AC130.HUD` | `AC130` | Absent |

AC130, M17 and F16 cockpit families have no left/centre/right overlays. SU33,
Y141 and F104 provide left/right overlays but no centre overlay. AV8, F4 and A7
provide all three. The shared identity API exposes these absences explicitly.
Every registered family names its own `~<family>_P.PIC` instrument panel.

## Installed systems and authored sensor assignments

No radar is installed on C130, V22, MI24, CH47, MIG17F, B747 or A310. No gun is
installed on C130, E3, IL76, E2, CH47, F4B, B747 or A310. The runtime identity API
returns optional primary gun/radar resources and a complete gun-type slice.
AC130 exposes C_25, C_40 and C_105 together; MIG17F exposes GSH30 and GSH23.

Source choices are preserved even where they differ from real-aircraft
expectations: V22 installs T30_1.JT with KA50 laser/ECM records; AH64 installs
F18R.SEE, F18.ECM and M61.JT; F4.PT installs M61.JT; IL76 installs E3R.SEE.
F4J's gun is its source SUU16.JT pod; the F4B can carry that record on its
centreline row as an option, though it is built with no gun (the F4B's
`guns()` stays empty, `gun_pods()` names the pod). The F4 variants retain their own fuel,
stores, sensors and exterior references.

The AC130 PT's zero-based hardpoints 4, 5 and 6 retain the following raw
gun-mount fields. Coordinates are source right/up/forward; the shared mount
reader divides them by three to obtain host feet. Angular words use the
existing 182-source-units-per-degree conversion. Interpreting arc fields as
tracking half-widths is a fitted consumer choice, rather than a proven original
animation or gun-control contract.

| Hardpoint | Default gun | Position x/y/z | Heading | Pitch | arcH | arcP | maxItems | Flags |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| 4 | C_25.JT | -6 / -24 / 51 | -16380 | 0 | 10920 | 10920 | 3000 | 0x1808 |
| 5 | C_40.JT | -12 / -24 / -6 | -16380 | 0 | 8190 | 8190 | 1000 | 0x1808 |
| 6 | C_105.JT | -16 / -26 / -48 | -16380 | 0 | 4550 | 8190 | 500 | 0x1808 |

The gun JT records provide these source burst fields. All three include the
existing automatic-repeat flag 0x800. Their separate OBJECT weight is 1 lb and
PROJECTILE weight is 0 lb; internal mounts are not added as external equipment.
Each has source launch-zone minimum 0 ft and maximum 13000 ft, initial speed
2933 ft/s and removeT 40, which the shared host conversion treats as 10 seconds.

| JT | Flags | projsInPod | actualRoundsPerGame | gameRoundsInBurst | gameBurstT | reloadT |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| C_25 | 0x348c4 | 1 | 2 | 4 | 1 | 4 |
| C_40 | 0x708c4 | 1 | 2 | 4 | 1 | 4 |
| C_105 | 0x608c4 | 1 | 2 | 4 | 4 | 4 |

F4J's SUU16.JT source fields are projsInPod 1200, actualRoundsPerGame 2,
gameRoundsInBurst 4, gameBurstT 1, reloadT 4, flags 0x348c6, OBJECT weight
1702 lb and PROJECTILE weight 0 lb. The PT default installs one pod at original
hardpoint 3, with source flags 0x000d and position 0 / -20 / 10. The fixed-slot
bit remains part of loading compatibility. Treating this named pod as separate
external hardware is a fitted host interpretation; source flags stay preserved.
The host interprets 1200 as actual rounds and derives 600 logical ammunition
units per installed pod. That interpretation and retaining the full 1702 lb
hardware mass when ammunition is exhausted are **fitted agent choices**,
2026-10-05. The source does not separately identify empty shell and ammunition
mass. The loadout's installed pod count stays separate from runtime ammunition.
Only this reviewed source-installed SUU16 station receives the conversion;
unrelated rocket pods keep their existing semantics.

The common sensor component reads all source range/angle/look-down numbers.
The following added assignments are **opinionated agent tuning**, 2026-10-05.
The existing [radar component rules](../radar.md#look-down-and-equipment-generations)
own the preset constants. These labels do not claim historical equipment eras.

| Added record | Assignment |
| --- | --- |
| F104R | Basic radar |
| F4JR, AC130R, AV8R, YAK141R, A7R, A10R | Transitional radar |
| F15R, E3R, E2R | Advanced radar |
| AV8, YAK141, KA50, A7, F104 ECM | Early jammer label; source RF capability may be absent |
| AC130, B52, A10 ECM | Transitional jammer label |
| F15 ECM | Late-Cold-War jammer label |

Airborne-radar SEE records retain their source 200 nmi search and 200 nmi track
coverage and broad angles. Contact sharing is not implemented by this source
registration. Source-limited equipment and unsupported channels stay distinct.

## Resource hashes and reproducible checks

All following hashes refer to the extracted PT bytes from FA_2.LIB.

| PT | SHA-256 |
| --- | --- |
| `C130.PT` | `27fe75263d603e17ba1298fee592538feed0beb77fb66d8586c9350d2bc02c0e` |
| `AC130.PT` | `65a4a441ce7061b185d6f94e91c1ce9693c5f0c64639ba0840ca22a02f4cdaeb` |
| `E3.PT` | `414c9d927a8d77e2be7688b0e660ccf3f72ab27ae40a8fbee4cc384e557df1c8` |
| `IL76.PT` | `ec92c31731d163125dc19b679c58ce1d92bf43792f36c6749308fef0d700dd4a` |
| `E2.PT` | `0b4f6e8a9db555a1d0fe9539b0d5a9bd7ddce6040927bdba9834b4d28f07e468` |
| `AV8.PT` | `26a45342ee469bffcd4aa1122ebaf7e4da4ef1173a5f71a498bfa7eeb8cc67c1` |
| `YAK141.PT` | `1727dc7a667a34327b8181742111577503101911d220afce26d69ee551959b9a` |
| `V22.PT` | `522a3011269dd5e8889a1dd696c04907e3be385dc7a33d9f663efca1874f92c0` |
| `AH64.PT` | `489ff9bd3e923469f4e99832232d4d5ce47dad3fe93f170a2ada3b02fba3ab99` |
| `MI24.PT` | `09d54a36026f3b9815a72ebc8aea528991dd4791f3df6d572e1f2de3fd976f45` |
| `CH47.PT` | `e4f5272525a302e82824b837a13566a80d47bb807146514b8af8b3b80f2c6daa` |
| `MIG17F.PT` | `a1e880bc87c69ce032dfcd0c401f555699220770ffa01527452fb5489ca261e6` |
| `F4B.PT` | `40ee72698c9e2966bad34466038e3692c351f5ec4cd2c0c572012a043e6567d9` |
| `F4J.PT` | `3ebedc917d157403e11b99695f0e6d2845c5643ff388b5c93020648102f6bf13` |
| `F4E.PT` | `3c1ca544abd442fa20ceb82b16e0dfc4afc26ad5075a16bbfddda89321d8164c` |
| `F4.PT` | `bbd6812a392a222c46273c1dfc1de0235437dca6444b8d4546ee31e54360dd38` |
| `A7.PT` | `fc1ca768747ef0d9a36bf3cca3dd8a1f2db51d2221445a6a0db9e466129eb133` |
| `F15.PT` | `d9f1a2387adeeb126d90704b6213ef99b0fd38502a92a160302c33d946b36cd2` |
| `F16C.PT` | `0da2b96cc4402eabecc343e44ba37ce992603b0696002ff1f49d5bd1ccc3d28a` |
| `F104.PT` | `3934b56ea2e1da9ea24112d642e148825784e159bcb289fd18bea3c52d58b45e` |
| `A10.PT` | `061d3c10b84f141b0a27af50f63adf41fc59fc181d487532670773ae13379397` |
| `B747.PT` | `39a818883fd92f9157aa8d3aa631e53fed57ff8a043d6c2819cd7c61c55a2e38` |
| `A310.PT` | `cf32c785a277081e9720a67b0fb40c74ccf2c7e0de21f17ee9cd2ded6fe01df7` |

Run `python3 tools/extract_assets.py --source <user-media> --out
.local/aircraft-variety --exclude-archive swpatch.lib --exclude-archive 'disc*/*'`
with one `--aircraft` argument for each registered identity. The Python wrapper
adds the source archive and extracted resource SHA-256 values. Extracted art,
audio, modules and generated source derivatives remain ignored and local.

Validation: `cargo test --locked -p tore-formats aircraft` passes 14 tests,
including synthetic identity/name/type-size rejection and absent/multiple gun
capabilities. A local standalone reader probe parsed all 23 extracted PT records
and every installed JT/SEE/ECM definition, preserving the exact identity, shape,
envelopes and station counts. All 23 configurations and source-default loadouts
validated, followed by 120 combat ticks and weapon-view reads per identity.
The unarmed-aircraft commands/ticks test and named default-missile profile test
also passed. The [missile contract](../spec/missiles.md#variety-default-weapon-extension)
owns the added supported-radar/IR mappings and the fitted unguided AT2 gap.
Retail gameplay comparison is unavailable. Visual,
audio, full flight and shared systems acceptance are separate work.

## MiG-25 source review (M3, 2026-11-07)

Research mode, slice M25-0. This reviews `M25.PT` and its selected dependency
closure before `AircraftId::Mig25` exists. The [fixed-wing record
audit](aircraft.md#fixed-wing-record-audit) predates this identity; this section
is its MiG-25 row. No original module was executed. PT, SEE, ECM, JT and GAS are
read as bounded BRF data with the shared field order in
`aircraft_schema.rs`; SH and HUD are inspected as data. Retail media stays local
and no extracted bytes are committed.

### Build and extraction

`FA_1.LIB` SHA-256 is
`657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` and
`FA_2.LIB` is
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`, the same
pair as the 2026-10-05 variety review. The selected closure was extracted
locally to `.local/m3-imports/scratch/mig25` with:

```sh
cargo run --locked -p tore-extract -- --source gameassets/fighters-anthology \
  --out .local/m3-imports/scratch/mig25 --include 'M25.PT' --include 'MIG25*' \
  --include 'SU33_C.HUD' --include 'MIG27R.SEE' --include 'MIG21.ECM' \
  --include 'VIS240.SEE' --include 'F250.GAS' --include 'AA6.JT' --include 'AA2.JT'
```

The extraction wrote the 14 resources to `FA_2.LIB/` and the same 14 to
`disc1/SETUP.ESA/FA_2.LIB/`, because the installer copy is a second archive
boundary. The PT, SH, SEE, ECM, JT and GAS bytes are identical in both; the HUD
differs by build (below). Hashes here are the root `FA_2.LIB` copies, matching
the 2026-10-05 review's archive identity. No `M25.PTS` companion exists in any
archive.

### Identity and presentation

| Field | Decoded value |
| --- | --- |
| PT | `M25.PT`, 1,686 stored and 10,289 decoded bytes, SHA-256 `63b968033f7cfb410277e7e9b45edf3e57a7639932ee80c1ebd2a3e99eba826a` |
| Displayed names | `MiG-25`, `MiG-25 Foxbat`, `M25.PT` |
| Schema | OBJECT + NPC + PLANE, 219 tokens; `structType` 5; `typeSize` 588; `instanceSize` 575 |
| Object class | `0x8000` (fighter); the PLANE no-lift bit is clear |
| Year / hit points | 1964 / 130; `expType` 30, `craterSize` 0 |
| Shape | `MIG25.SH`, exponent 8, so scale `2^(8-8) = 1` and 1 shape unit is 1/3 ft in the aircraft renderer |
| Shadow | `MIG25_S.SH`, 16 faces, no texture |
| HUD | `SU33_C.HUD`, byte-identical to `SU33CC.HUD` in the same build |
| Cockpit | stem `SU33`; forward `~SU33H.PIC`; panel `~SU33_P.PIC` |
| Damage looks | `MIG25_A.SH` 308 faces, `MIG25_B.SH` 60, `MIG25_C.SH` 278, `MIG25_D.SH` 136 |
| PTS | absent, an explicit exception |
| Checkpoint code | 37 reserved for M25-1, not yet in `checkpoint_shared.rs` |
| CLI key | `mig25` and `m25.pt` reserved for M25-1, not yet in the CLI |
| AI script | `f.BI`, present |

The media-backed identity matches the plan's fixed names. `AircraftId::Mig25`
is not present yet, so `Aircraft::parse` cannot read the record until M25-1 adds
`Mig25 => 588` to the size table and the reviewed names. The parser's default
`_ => 660` would reject this record.

### Resources

All PT-named resources exist in `FA_2.LIB` or `FA_1.LIB`.

| Resource | Role | Decoded bytes | SHA-256 |
| --- | --- | ---: | --- |
| `MIG25.SH` | Shape | 29,184 | `d76f8f7384305426b58543334b629aff757bedddfe2a78e6be6ccdd2393814bb` |
| `MIG25_S.SH` | Shadow shape | 4,608 | `08bb496add40ffc89f79823bb87504f6bfad4bc57d4aa9e86569f47208d785fe` |
| `MIG25_A.SH` | Damage body A | 25,088 | `29479ec7e179f67099b7500c7dd8d62a5f366d30df6c2c24c231103f5861c945` |
| `MIG25_B.SH` | Damage piece B | 8,704 | `7f51676be47af4bd6f22819e32bc1a311554b9c79dd8fe9ca5fd28cd59f47f6b` |
| `MIG25_C.SH` | Damage body C | 20,992 | `b5a55cfec9a888a64e61456af014e69d4f605c44c73a9c1ebe7ea3f7212b5a31` |
| `MIG25_D.SH` | Damage piece D | 12,800 | `1a18c67126b9872222f0af4736d8797f0324a29baa48f3a5057929c13584b51b` |
| `SU33_C.HUD` | HUD and cockpit pointers | 4,608 | `d22b4cd527bd4aaa23eb690fb1a75a7e92e797367f2eded97c3a89f56a3751ef` |
| `MIG27R.SEE` | Radar, BASIC | 490 | `33846cc54be0774b243838cba435c844d079525f970f53e89ce66ba9e104475f` |
| `MIG21.ECM` | Jammer, EARLY, chaff 20 and flare 20 | 346 | `2d194e3b843aed2bee616278208667bbb68ec5eed06457d081f3f8580943cc3e` |
| `VIS240.SEE` | Visual sensor, signature 0 | 471 | `a3224581abef75e0fe4e6dbb37d2d47d5ab4704bb1043e09dd6aa2e5a4510ee1` |
| `AA6.JT` | Default A2A store | 2,756 | `83f67aa812bcac9080186193a84a4812a045458ce46c848d76f516f9e8cd2a90` |
| `AA2.JT` | Default A2A store | 2,756 | `94e415550095ee4323c3d70277e13d825bc49639540d1160f83e1629e73f06cc` |
| `F250.GAS` | Centreline tank, 198 lb empty and 1,650 lb fuel | 205 | `974e49de6eb22237bf7a1bd24b3ed9308300bbb95270a9c0fe7104892514f0de` |

The shape's faces name `_MIG25.PIC`; the damage looks name `_MIG25_A.PIC` to
`_MIG25_D.PIC`. All five atlases are present in `FA_2.LIB` (19,953, 19,620,
17,111, 16,878 and 6,205 stored bytes). The cockpit supplies `~SU33H.PIC`
(87,084 bytes), `~SU33_P.PIC` (910), the left and right overlays
`~SU33_LH.PIC`, `~SU33_RH.PIC`, `~SU33_L.PIC`, `~SU33_R.PIC`, `~SU33_LS.PIC`
and `~SU33_RS.PIC`, plus `~SU33.PIC` (22,283) and `~SU33S.PIC` (139,164), all
in `FA_1.LIB`.

The HUD's own strings name `~su33`, `~su33h`, `~su33s`, `~su33_p`, `~su33_w`,
`~su33_l`, `~su33_r`, `~su33_lh`, `~su33_rh`, `~su33_ls`, `~su33_rs`, `hudsym`,
`GEAR`, `FLAP`, `BRAKE` and `HOOK`. Every referenced picture except
`~SU33_W.PIC` exists; that name has no archive entry. The existing MiG-29,
Su-25 and MiG-23 imports use the same shared cockpit, so the missing name is
recorded as an optional or unresolved reference, not as a blocker. `HUDSYM00`,
`HUDSYM01`, `HUDSYM11`, `WIN11`, `HUD11`, `WIN01` and `WIN00` fonts exist.

`SU33_C.HUD` is not an alias for `SU33CC.HUD`; it is the exact PT name and the
host must return `SU33_C.HUD`. The two files are byte-identical inside one
build. The root `FA_2.LIB` copy is 483 stored bytes with SHA-256 `d22b4cd5...`;
the `disc1/SETUP.ESA:FA_2.LIB` copy is 485 stored bytes with SHA-256
`ce00eb5a...`. Both builds also carry an identical `SU33CC.HUD` of the same
size. The import report keeps whichever build the user supplied.

The `HOOK` label is shared cockpit art, not a device. PLANE flag `0x02` is
clear, so the MiG-25 has no hook. There is no gun record: `guns()` stays empty
and the cockpit must show no gun reticle or gun mode.

### Mass, thrust and published audit

Published columns are MiG-25P/PD reference figures, not retail behaviour. The
specification box on Wikipedia and MILAVIA gives empty 20,000 kg, gross 36,720
kg, ceiling 20,700 m, 3,000 km/h and two R-15B-300 engines at 73.5 kN dry and
100.1 kN with afterburner (both accessed 2026-11-07). GlobalSecurity gives the
internal fuel as 14,200 kg. topwar.ru gives a 41,000 kg maximum takeoff weight;
the sources do not agree on that field.

| Field | Record | Published | Difference | Verdict |
| --- | ---: | ---: | ---: | --- |
| Empty weight, lb | 48,221 | 44,092 (20,000 kg) | +9.4% | Left as retail, listed for John |
| Internal fuel, lb | 37,320 | 31,306 (14,200 kg) | +19.2% | Left as retail, listed for John |
| Maximum takeoff, lb | 90,830 | 80,954 (36,720 kg) to 90,390 (41,000 kg) | +12.2% to +0.5% | Left as retail, listed for John |
| Military thrust, lbf | 38,800 | 33,047 (2 x 73.5 kN) | +17.4% | Left, within a third |
| Afterburner thrust, lbf | 49,382 | 45,008 (2 x 100.1 kN) | +9.7% | Left, within a third |
| Ceiling, ft | 68,000 | 67,913 (20,700 m) | +0.1% | Agrees |
| 1 G top speed, kt | 1,623 | 1,620 (3,000 km/h) | +0.2% | Agrees |
| G limits | +5 / -2 | +4.5 safety, about 11 structural | +0.5 G positive | Keep retail (M25-8) |
| Military flow, lb/s | 2 | about 12 at rated thrust | 6 times low | Retail value, review in M25-8 |
| Afterburner flow, lb/s | 12 | about 34 at rated thrust | 2.8 times low | Retail value, review in M25-8 |
| Year | 1964 | First flight 1964; PD from 1978 | Family year | No change |

The record is not a copied template. No other PT in `fa-aircraft.csv` shares
three or more of its five mass and thrust fields. The nearest, `MIG31.PT`,
differs in all five. The maximum takeoff weight is 440 lb above `SU24.PT`'s
90,390, but the empty weight, fuel and thrust are not the Su-24's. The
[record rule](../spec/aircraft-records.md#rule) therefore leaves all of these
values as retail and lists them for John rather than correcting them (agent
audit decision, 2026-11-07).

The record's own budget is coherent. Empty plus internal fuel is 85,541 lb,
leaving 5,289 lb below its 90,830 lb maximum. Two `AA6.JT` at 1,010 lb each,
two `AA2.JT` at 165 lb each and one full `F250.GAS` at 198 plus 1,650 lb total
4,198 lb of source stores, leaving about 1,100 lb. The live loaded mass may use
the missile data table rather than these OBJECT weights. The actual default
quantity is a host rule for M25-6.

`maxAlt` is 68,000 ft and `minAlt` 300 ft. The 0 G envelope row carries a point
at 70,000 ft, above `maxAlt`; the host caps altitude at the record field, so
this is not a change.

### G envelope rows

Points are speed in ft/s at altitude in ft. The eight rows are consecutive from
`envMin` -2 to `envMax` +5.

| G | Points |
| ---: | --- |
| -2 | 325@0, 421@19,500, 575@33,750, 720@38,750, 1,425@38,000, 1,722@33,000, 1,910@24,000, 1,010@0 |
| -1 | 210@0, 235@13,000, 287@27,000, 363@41,000, 501@56,000, 710@68,000, 1,451@68,000, 2,412@54,000, 2,740@36,000, 1,100@0 |
| 0 | 150@0, 170@13,000, 205@27,000, 283@41,000, 451@57,000, 690@70,000, 1,465@70,000, 2,470@55,000, 2,810@36,000, 1,150@0 |
| 1 | 210@0, 235@13,000, 287@27,000, 363@41,000, 501@56,000, 710@68,000, 1,451@68,000, 2,412@54,000, 2,740@36,000, 1,100@0 |
| 2 | 260@0, 310@14,750, 440@34,500, 610@46,750, 760@52,000, 1,417@52,000, 1,992@43,000, 2,337@31,000, 1,050@0 |
| 3 | 325@0, 421@19,500, 575@33,750, 720@38,750, 1,425@38,000, 1,722@33,000, 1,910@24,000, 1,010@0 |
| 4 | 385@0, 542@15,875, 720@23,750, 1,175@23,375, 1,490@17,000, 970@0 |
| 5 | 465@0, 582@8,375, 720@12,750, 965@12,875, 1,150@9,000, 950@0 |

The -2 and +3 rows are the same polygon, and the -1 and +1 rows are the same
polygon. This is source data, not a decoding overlap.

### Engines, fuel and sound

| Field | Value |
| --- | --- |
| Engines | 2 |
| Empty weight | 48,221 lb |
| Internal fuel | 37,320 lb |
| Maximum takeoff | 90,830 lb |
| Military thrust | 38,800 lbf total |
| Afterburner thrust | 49,382 lbf total |
| Military fuel flow | 2 lb/s total |
| Afterburner fuel flow | 12 lb/s total |
| Tank fuel | 1,650 lb, plus 198 lb empty mass |

At the record's flows the internal tank lasts about 5.2 hours at military
power or 51.8 minutes at full afterburner; with the tank, 54.1 minutes at
afterburner. These are host reading arithmetic, not a claim about real
endurance. The real engine's specific fuel consumption is far higher (1.12 and
2.45 lb/(h lbf)), which is a flight-model review item for M25-8, not a record
correction here.

The PT names four audio resources, all present in `FA_2.LIB`:

| Field | Name | Archive entry |
| --- | --- | --- |
| `loopSound` | `&jet19.11k` | `&JET19.11K`, 21,055 stored bytes |
| `secondSound` | `&jet19.11k` | `&JET19.11K` |
| `engineOnSound` | `&powerup.5k` | `&POWERUP.5K`, 34,262 stored bytes |
| `engineOffSound` | `&powerdn.5k` | `&POWERDN.5K`, 23,177 stored bytes |

`doDoppler` is 1, `maxSndDist` 15,000, the plus and minus Doppler pitch limits
are 320 and 160, and the Doppler speed window is 20 to 1,600. The `f.BI` AI
script exists (6,081 stored bytes).

### Hardpoints

All six source rows, in original order. Positions are source units and divide
by three for host feet; x is right, y is up and z is forward of the aircraft's
reference point, the hardpoint convention in
[aircraft ordnance](aircraft-ordnance.md#hardpoint-positions), not the shape
mesh convention. The location byte maps 0 to Centerline, 1 to Fuselage, 4 to
Wing and 5 to Wingtip. `maxWeight` is the weight class used by
`min(maxItems, maxWeight * 100 / store weight)`, not a pound value.

| Row | Location | Flags | Position x/y/z | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x0008` | 0 / 0 / 0 | 1 | 0 | `VIS240.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x0008` | 0 / 0 / 0 | 1 | 0 | `MIG27R.SEE` | Fixed radar |
| 2 | Fuselage | `0x0008` | 0 / 0 / 0 | 1 | 0 | `MIG21.ECM` | Fixed jammer and countermeasures |
| 3 | Wing | `0x00c5` | 38 / -11 / -15 | 2 | 40 | `AA6.JT` | A2A row, inner wing |
| 4 | Wing | `0x00c5` | 51 / -11 / -21 | 2 | 40 | `AA2.JT` | A2A row, outer wing |
| 5 | Centerline | `0x0601` | 0 / -25 / -4 | 1 | 20 | `F250.GAS` | External tank |

The three fixed rows carry bit 8, which the original weapon loader skips for
non-projectile equipment. They are not editable ordnance. The two A2A rows and
the tank row are the only carrier rows. In host feet the inner A2A row
is at 12.67 / -3.67 / -5.0, the outer at 17.0 / -3.67 / -7.0 and the tank at
0 / -8.33 / -1.33. `maxItems` is a quantity or capacity field, not a count of
physical pylons; the one source row per store type represents the wing pair.
M25-5 applies the pylon rules and checks the 20 ft `ACRID.SH` fin clearance
against the neighbouring row and the gear.

### Flags and devices

- OBJECT `flags` is `0x2bf3`, uninterpreted here.
- PLANE `flags` is `0x11`: seat bit `0x10` is set; hook `0x02`, multi-crew
  `0x04`, carrier `0x40` and the no-lift bit `0x08` are clear. Bit `0x01` has
  no reviewed meaning.
- NPC `flags` is 0; `ctName` is `f.BI`, `numHards` 6, `searchFrequencyT` 20,
  `unreadyAttackT` 32, `attackT` 20, `retargetT` 32767 and `zoneDist` 0.
- `internalFuel` 37,320, `fuelConsumption` 2, `aftFuelConsumption` 12,
  `engines` 2. `thrust` 38,800 and `aftThrust` 49,382.
- `structure[0]` 1,091, `structure[1]` 2,732, `structureWarnLimit` 2,560 and
  `structureLimit` 5,120. `maxTakeoffWeight` 90,830.
- `crashSpeedForward` 330, `crashSpeedSide` 51 and `crashSpeedVertical` 95;
  `crashPitch` 25 and `crashRoll` 10.
- The shape has five state words. Counts and low points are from the runtime
  `Shape::with_state` path (neutral 320 faces, low -16); the export branch
  (`with_export_state`) has two extra wingtip faces and 322 neutral faces.

| Word | Faces | Low point | Geometry | Reading | Status |
| --- | ---: | ---: | --- | --- | --- |
| `0x7600` | +8 | -8 | Two vertical quads at x +/-10 and two horizontal quads at z 0, all from y -129 to -70, aft of the neutral shape's -91 bound | Afterburner flame: two crossed double-sided plates, one per engine, rooted at the nozzle exit y=-70 | Confirmed; mapped to exhaust in M25-4 |
| `0x7606` | +2 | +9 | Raised quads at x +/-5, y 3 to -13, z 9 to 23; one has a texture and one has an empty UV list | Dorsal airbrake, hinged on the spine front edge (0,3,9) | Confirmed; mapped to brake in M25-4 |
| `0x760c` | +12 | -28 | Three wheel and strut groups at x +/-18 to +/-8, y -19 to -7, z -28 to -10, plus two centreline nose-wheel quads at y 45 to 54 | Landing gear down | Confirmed; this is the parked gear word |
| `0x7618` | -2 | -16 | Removes one left trailing-edge pair at x -39 to -18, y -25 to -39, z 4 to 8 | Left flap pair, the M25-3 surface | Confirmed; mapped to flaps in M25-4 |
| `0x761e` | -2 | -16 | Removes the mirrored right pair | Right flap pair, the M25-3 surface | Confirmed; mapped to flaps in M25-4 |

The word order is the shared base order flame, brake, gear, flap, flap: the
flame, brake and gear words sit at base, base+6 and base+0xc, exactly as the
F-18, Rafale and F-104 families, and the two trailing words are the left and
right flap pairs the M25-3 rig already turns. No hook word exists because the
PLANE flag 0x02 is clear. M25-4 maps `0x7600` to the exhaust, `0x7606` to the
airbrake and `0x7618`/`0x761e` to the flaps, and records the gear as the parked
word it always was.

The gear-down low point is -28 shape units, 9.33 ft below the origin. The F2
contact offset is -27 units, 9.0 ft, the value the parked table already uses.
The two-unit difference is the parked-aircraft acceptance tolerance. The
deployed-gear low point is the conservative clearance for `models/variety.rs`.

The export branch adds two wingtip quads (`0x3e19` and `0x4420`) at x +/-24 to
+/-50, y -1 to -22, z 3 to 7, with indexed textures. They are not present in
the runtime state path. They may be missile rails, wingtip lights or
export-only geometry; M25-2 and M25-5 must identify them before they are drawn
or used for store placement.

### Shape, damage and animation handoff

`MIG25.SH` has 320 runtime faces, 0 lines and 0 billboards, and uses one
texture atlas. Its bounding box is x -72 to 72, y -91 to 143 and z -16 to 38
shape units, so about 48 ft span, 78 ft length and 18 ft height. This agrees
with the published 14.01 m span and 23.82 m length within the source's
coarse units. The streamer definition has a zero pivot and hinge scale with
two identical points at `[18432, 512, -12032]`; its use is not reviewed.

The four damage shapes are A 308 faces, B 60, C 278 and D 136. A and C are the
damaged bodies and B and D are the debris pieces under the existing aircraft
damage contract. There is no separate hulk file. M25-2 must confirm which body
is the destroyed hulk and that the pieces come from the correct source.

Animation handoff: `0x760c` is the gear word and is already used by the parked
path. The other four words are not used there. M25-4 identified them from the
shared base order and their geometry: `0x7600` is the afterburner flame,
`0x7606` the dorsal airbrake, and `0x7618`/`0x761e` the left and right flaps
that M25-3 turns. M25-3 owns the stabilators, ailerons, flaps and twin rudders
on the neutral shape; M25-4 owns the gear, brake and burner.

### Sensors and stores

`MIG27R.SEE` is assigned `Preset::Basic` and `MIG21.ECM` is assigned
`Generation::Early` in `tore-sim::sensors::profile`. No new preset is needed.
`VIS240.SEE` is the signature-0 visual sensor every imported aircraft carries.
`MIG21.ECM` decodes as chaff loaded 20 with chance 35, heading 95 and pitch 24,
and flares loaded 20 with chance 35, heading 159 and pitch 31; both deception
chances and signature additions are zero. M25-6 owns the countermeasure counts
and the default load.

`AA6.JT` and `AA2.JT` are the two default A2A records; `F250.GAS` is the
F-14's reviewed 250 gallon tank, with the same SHA-256 as the F-14 import. The
F250 tank is a centreline row with `maxItems` 1.

### Unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| PLANE flag `0x01` | The base flag is set but has no reviewed meaning; it must not be mistaken for a device | Trace the PLANE flag consumers in the reviewed executable and compare with the F-22's `0x91` base; M25-1 can proceed without it |
| `0x7600` geometry | It may be an afterburner nozzle or a tail extension a player sees | Resolved in M25-4: the two crossed double-sided plates per engine are the afterburner flame, rooted at the y=-70 nozzle exit and mapped to the exhaust fraction |
| `0x7606` geometry | It may be the canopy, one face of which is transparent | Resolved in M25-4: the word sits at the base+6 brake slot and the raised dorsal panel hinges on the spine front edge, so it is the airbrake, mapped to the brake fraction |
| `0x7618` and `0x761e` geometry | They remove a left and right trailing-edge pair, which could be a control surface or a damage look | Resolved in M25-4: the removed pairs are exactly the M25-3 left and right flaps, so the words are the per-side flap words and map to the flaps |
| Export wingtip faces | Two wingtip quads appear only on the export path and could be rails, lights or export-only art | Identify in M25-2 and check against store placement in M25-5 |
| `~SU33_W.PIC` | The HUD names it but no archive entry exists; the existing shared cockpit may treat it as optional | Resolved in M25-7: the host reads only the panel name at HUD root +0x275 (`~su33_p`) and never the `~xx_w` field at +0x282, so the missing entry is an optional reference with no code path, and the MiG-29, Su-25 and MiG-23 already ship without it. The original's use of +0x282 stays unknown (`docs/spec/instrument-bezel.md`) |
| Default A2A quantity | `maxItems` is capacity, not a proven default load | Review the source-default normalization in M25-6 |
| MTOW and internal fuel above published figures | They are within a third but affect range, takeoff and loading | List for John now; validate flight and takeoff in M25-8. No record change in M25-0 |
| Fuel flow far below the real engine's SFC | It sets endurance and the fuel gauge | Review in M25-8 with the envelope probe; no record change in M25-0 |
| Destroyed hulk identity | A and C are both damaged bodies and there is no separate hulk file | Confirm in M25-2 which body draws as the hulk and that B and D are the pieces |
| Two HUD builds | The root and `SETUP.ESA` copies differ by two bytes in size, though each is internally identical to its `SU33CC.HUD` | Record the supplied build's hash in the import report; never alias the two names |
| Shadow shape | `MIG25_S.SH` reads but its faces and placement are not reviewed | Include in the M25-2 look review |
| Streamer definition | The two points and zero pivot are unreviewed | Check the contrail or effect origin in M25-2 or M25-7 |

### M25-1 handoff

The record is ready for identity registration, with no media disagreement.
M25-1 should append `Mig25` after `A310` (`ALL` 37, `SELECTABLE` 38), use
`typeSize` 588, return `M25.PT`, `MIG25.SH`, `SU33_C.HUD`, `~SU33H.PIC`,
`~SU33_P.PIC`, `MIG27R.SEE`, `MIG21.ECM` and `VIS240.SEE`, keep `guns()` empty
and `has_pts()` false, and use checkpoint code 37. The deployed gear low point
is -28 shape units (-9.33 ft); the F2 contact offset is -27 (-9.0 ft). The
debris scale is 1/3, the exponent-8 host factor that `combat/debris.rs` uses
for every aircraft without a special case. The live permitted list is
`AA6.JT` and `AA2.JT`. The strike class is Fighter. No other record is
substituted for any of these names.

### M25-7 cockpit and HUD confirmation (2026-11-07)

Confirmation work; no code changed. Full frames and command output are in the
local slice record, and the acceptance baseline joins M25-10.

- `SU33_C.HUD` loads. `Airframe::load` requires the HUD panel field to equal the
  identity's instrument panel, and the MiG-25's is `~SU33_P.PIC`. The panel
  colours are title, button and press index 1 and the primary ink index is 40,
  the same as `SU33CC.HUD` in the same build (`docs/spec/instrument-bezel.md`
  now lists the mig25 bezel row).
- Cockpit stem `SU33`: forward `~SU33H.PIC`, bezel `~SU33_P.PIC`, left and
  right overlays `~SU33_LH.PIC` and `~SU33_RH.PIC` and no centre overlay. The
  glass aperture is the reviewed `SU33` polygon (`hud_aperture.rs` maps the
  `SU33` stem to the MiG-29 group) and the mirrors are the reviewed `SU33H`
  left and right masks at (176,195) and (1103,195)
  ([mirrors baseline](../baselines/mirrors.md#2026-11-07-mig-25-cockpit-confirmation)).
- The forward HUD draws the airspeed, altitude, heading, pitch ladder and
  flight-path markers and no gun reticle, because `guns()` is empty and no gun
  group is set.
- Every instrument page draws: RCS (0), Envelope (1), Front View (2), Other
  View (3), Target Cam (4), RWR (5), Nav Info (6), Systems (7, internal fuel
  plus the external tank), Weapons (8, the default stores and the chaff and
  flare counts, no gun row) and Radar (9). The MiG-25 has no targeting sight or
  gunsight, so Target Cam shows the shared plain target window; absent systems
  stay absent.
- Sensors are unchanged from M25-0: `MIG27R.SEE` BASIC radar, `MIG21.ECM` EARLY
  jammer with chaff 20 and flare 20, and `VIS240.SEE`. No unreviewed sensor is
  enabled. The `--sensor-summary` line matches the MiG-23's on the shared
  records.

### M25-8 flight review (2026-11-07)

The flight model reads the record's own values, so no envelope, G or flow fit
was needed. The 1 G row's fast edge decodes to 2,740 ft/s at 36,000 ft, 1,623
kt, against the published 3,000 km/h (1,620 kt) at 13,000 m, so the record's
envelope stays as decoded. The G limits stay +5/-2. The default load's 89,827
lb gross is 1,003 lb below the record's 90,830 lb maximum. The fuel flows are
2 and 12 lb/s, 6 and 2.8 times below the real R-15B-300's specific fuel
consumption (1.12 and 2.45 lb/(h lbf)); the record rule keeps them with the
tank, which is left as retail, so the real engine's endurance is a gap for
John, not a correction. The flight validation measured liftoff 153.9 kt over
2,161 ft, approach 165.6 kt and touchdown 164.0 kt, inside the published 300
to 350 km/h and 270 to 290 km/h ranges' plus or minus 10 percent window
([variety flight baseline](../baselines/variety-flight.md#mig-25-validation-2026-11-07)).

## F-16A source review (M3, 2026-11-08)

Research mode, slice F16A-0. This reviews `F16.PT` and its selected dependency
closure before `AircraftId::F16a` exists. `F16.PT` is the 38th retail identity,
to be appended in F16A-1. The [fixed-wing record
audit](aircraft.md#fixed-wing-record-audit) predates this identity; this section
is its F-16A row and the field-by-field diff against `F16C.PT`. No original
module was executed. PT, SEE, ECM, JT and GAS are read as bounded BRF data with
the shared field order in `aircraft_schema.rs`; SH and HUD are inspected as
data. Retail media stays local and no extracted bytes are committed.

### Build and extraction

`FA_1.LIB` SHA-256 is
`657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` and
`FA_2.LIB` is
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`, the same
pair as the 2026-10-05 and MiG-25 reviews. The selected closure was extracted
locally to `.local/m3-imports/scratch/f16a` with `tore-extract` and the patterns
`F16.PT F16C.PT F16E.PT F16F.PT F16* F16C.HUD F18R.SEE F18.ECM VIS340.SEE
PAVESPK.SEE M61.JT F150.GAS AGM65G.JT AIM120.JT AIM9M.JT ~F16* _F16* F16_*
F16*.11K F16*.CB8 &JET1N.11K &JET1A.11K &POWERUP.5K &POWERDN.5K`. The installer
copy under `disc1/SETUP.ESA/FA_2.LIB` holds the same resources; the PT, SH, SEE,
ECM, JT and GAS bytes are identical, and `F16C.HUD` differs between the two
builds by four header bytes only (below). No `F16.PTS` companion exists in any
archive, unlike `F16C.PTS`.

### Headline: the plan's record-diff assumption is false

The plan says F16A "differs from `F16C.PT` in its record (mass, thrust, envelope,
fuel, year, stations)". The decode shows that is only half true. `F16.PT` and
`F16C.PT` are **identical in every mass, thrust, fuel, engine and envelope
field**, so F16A does not have its own flight record: it flies the F-16C record.
They differ only in the identity names, the OBJECT flags, the year, the cockpit
view offset and the station block. The repo already knew this from the other
direction: the fixed-wing audit records that "F16C.PT mass is F16.PT's (F-16A)".
This is the one plan assumption that is false; F16A-1 can still proceed, but
F16A-6 must say "the F-16C configuration" rather than "its own configuration",
and the plan's parenthetical should be corrected to "(year, object flags, view
offset, stations, and the missing Pave Spike row)".

### Identity and presentation

| Field | Decoded value |
| --- | --- |
| PT | `F16.PT`, 14,732 stored bytes, SHA-256 `46018763b5e823df1d29c37c78d1cb9fdb7e7075b5235e23001bcc001b55b010` |
| Displayed names | `F-16`, `F- 16A Falcon`, `F16.PT` |
| Schema | OBJECT + NPC + PLANE, 219 tokens; `structType` 5; `typeSize` 660; `instanceSize` 626 |
| Object class | `0x8000` (fighter); the PLANE no-lift bit is clear |
| Year / hit points | 1979 / 92; `expType` 30, `craterSize` 0 |
| Shape | `F16.SH`, exponent 8, so scale `2^(8-8) = 1` and 1 shape unit is 1/3 ft in the aircraft renderer |
| Shadow | `F16_S.SH`, 12 faces, no texture |
| HUD | `F16C.HUD`, identical content to the F-16C's, naming panel `~f16_p` |
| Cockpit | stem `F16`; forward `~F16H.PIC`; panel `~F16_P.PIC`; no left, centre or right overlay |
| Damage looks | `F16_A.SH` 352 faces, `F16_B.SH` 224, `F16_C.SH` 410, `F16_D.SH` 73 |
| PTS | absent, an explicit exception (F16C has `F16C.PTS`) |
| Checkpoint code | 38 reserved for F16A-1, not yet in `checkpoint_shared.rs` |
| CLI key | `f16a` (agent choice) reserved for F16A-1, not yet in the CLI |
| AI script | `f.BI`, present |

The media-backed identity matches the plan's fixed names: the exact PT is
`F16.PT`, the shape is `F16.SH`, the HUD is `F16C.HUD`, and the cockpit stem is
the F-16C's `F16`. `AircraftId::F16a` is not present yet. The parser's default
size is 660 and this record is 660, so `Aircraft::parse` would accept the size on
the default arm; F16A-1 should still list it explicitly beside `F16C`.

### Field-by-field diff against `F16C.PT`

Two decodes of the raw records agree in every field except these. There is no
difference anywhere in the NPC block and no difference anywhere in the PLANE
block (weight, thrust, fuel, engine count, all drag, stall, spin, rudder and
crash coefficients, and every envelope row).

| Field | `F16.PT` (F-16A) | `F16C.PT` (F-16C) | Note |
| --- | --- | --- | --- |
| `ot_names` | `F-16`, `F- 16A Falcon`, `F16.PT` | `F-16C`, `F- 16C Falcon`, `F16C.PT` | identity |
| OBJECT `flags` | `0x2bf3` | `0x806bf3` | F16C adds bits `0x800000` and `0x4000`; uninterpreted |
| `year` | 1979 | 1984 | display and roster year |
| `viewOffset.z` | 110 | 60 | cockpit eye forward offset |
| `hards` | nine rows, no designator (below) | nine rows with a Pave Spike row (below) | stations, masks and capacities |

Everything else matches word for word: `typeSize` 660, `instanceSize` 626,
`shape` `f16.SH`, `shadowShape` `f16_s.SH`, `hitPoints` 92, `sigs` 88 / 88 / 100
/ 100 / 0, `weight` 14,567 lb, `minAlt` 300 and `maxAlt` 60,000, `envMin` -4 and
`envMax` +9, `engines` 1, `thrust` 17,687 lbf and `aftThrust` 32,000 lbf,
`fuelConsumption` 2 and `aftFuelConsumption` 16, `internalFuel` 6,972 lb,
`maxTakeoffWeight` 33,000 lb, `structure[0]` 1,342 and `structure[1]` 1,934,
`crashSpeedForward` 330, `crashSpeedSide` 51 and `crashSpeedVertical` 95,
`crashPitch` 25 and `crashRoll` 10, the NPC block (`ctName` `f.BI`, `numHards`
9, `searchFrequencyT` 12, `unreadyAttackT` 32, `attackT` 20, `retargetT` 32767,
`zoneDist` 0), the four sounds and `hudName` `f16c.HUD`. `dmgDebrisPos` is
`[0, 0, 30]` and `dstDebrisPos` `[30, 0, 0]`, the same as F16C.

### Hardpoints

All nine source rows, in original order. Positions are source units and divide
by three for host feet; x is right, y is up and z is forward of the reference
point, the hardpoint convention in
[aircraft ordnance](aircraft-ordnance.md#hardpoint-positions), not the shape
mesh convention. The location byte maps 0 to Centerline, 1 to Fuselage, 2 to the
internal gun, 3 to an internal bay, 4 to Wing and 5 to Wingtip. `maxWeight` is
the weight class used by `min(maxItems, maxWeight * 100 / store weight)`, not a
pound value.

| Row | Location | Flags | Position x/y/z | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x0008` | 0 / 0 / 0 | 1 | 0 | `VIS340.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x0008` | 0 / 0 / 0 | 1 | 0 | `F18R.SEE` | Fixed radar |
| 2 | Fuselage | `0x0008` | 0 / 0 / -50 | 1 | 0 | `F18.ECM` | Fixed jammer and countermeasures |
| 3 | Internal gun | `0x0008` | -11 / 3 / 31 | 500 | 0 | `M61.JT` | Built-in gun |
| 4 | Centerline | `0x0605` | 0 / -20 / 0 | 1 | 20 | `F150.GAS` | External tank |
| 5 | Wing | `0x07e5` | 21 / -10 / -4 | 2 | 20 | `AGM65G.JT` | Inner wing |
| 6 | Wing | `0x07e5` | 32 / -10 / -7 | 2 | 20 | `AIM120.JT` | Mid wing |
| 7 | Wing | `0x0085` | 43 / -10 / -11 | 2 | 4 | `AIM9M.JT` | Outer wing |
| 8 | Wingtip | `0x0085` | 54 / 2 / -14 | 2 | 4 | `AIM9M.JT` | Wingtip |

In host feet the four weapon carriers are at 7.0 / -3.33 / -1.33, 10.67 / -3.33
/ -2.33, 14.33 / -3.33 / -3.67 and 18.0 / 0.67 / -4.67, and the tank at 0 /
-6.67 / 0. Rows 0 to 3 carry bit 8, the fixed-equipment bit the original weapon
loader skips, so they are not editable ordnance: rows 0 to 2 are the fixed
sensors and row 3 is the built-in gun. The five editable rows are the tank and
the four wing and wingtip carriers.

The station block is the main record difference from the F16C. The F16C has a
fourth fixed row (a Pave Spike designator `PAVESPK.SEE` at z +50) that F16A does
not, and its rows 6, 7 and 8 are three carriers; F16A has four. The positions
and capacities differ too: F16A's AGM-65G row is inboard at x 21 with `maxItems`
2 and weight class 20 where the F16C's AGM-65G row is at x 32 with `maxItems` 4
and class 27; F16A's AIM-120 row is at x 32 with mask `0x07e5` and class 20 where
the F16C's AIM-120 row is at x 43 with mask `0x0185` and class 13; and F16A adds
an outer-wing AIM-9M row at x 43 that the F16C does not have. The plan's "no Pave
Spike" and "two AIM-9M" hold: F16A has no designator row and two AIM-9M rows
(outer wing and wingtip). F16A-4 owns the compatibility masks, the even pairs,
the source-default quantity and the clipping check.

### Envelope rows

Points are speed in ft/s at altitude in ft. The fourteen rows are consecutive
from `envMin` -4 to `envMax` +9, and they are byte-for-byte the F16C's, because
the whole PLANE block is shared.

| G | Points |
| ---: | --- |
| -4 | 540@0, 660@8000, 870@15000, 950@14500, 1030@13000, 1040@0 |
| -3 | 465@0, 627@15000, 830@27000, 990@27000, 1120@24500, 1170@20000, 1100@0 |
| -2 | 335@0, 537@21000, 820@42000, 1105@42000, 1297@37500, 1380@28000, 1230@14000, 1170@0 |
| -1 | 180@0, 295@19500, 437@35250, 608@47125, 820@55000, 1262@54250, 1585@48500, 1740@35000, 1410@17500, 1270@0 |
| 0 | 90@0, 120@13000, 175@27000, 263@39000, 471@54000, 820@65000, 1337@65500, 1850@54750, 2084@36000, 1697@18000, 1450@0 |
| 1 | 120@0, 165@13000, 237@26000, 363@39000, 551@52000, 800@60000, 1312@60000, 1712@52000, 1933@36000, 1556@18000, 1340@0 |
| 2 | 180@0, 295@19500, 437@35250, 608@47125, 820@55000, 1262@54250, 1585@48500, 1740@35000, 1410@17500, 1270@0 |
| 3 | 255@0, 391@18500, 587@39000, 820@50000, 1245@49000, 1468@43500, 1552@32000, 1306@16000, 1210@0 |
| 4 | 335@0, 537@21000, 820@42000, 1105@42000, 1297@37500, 1380@28000, 1230@14000, 1170@0 |
| 5 | 407@0, 603@18500, 820@34000, 1005@34000, 1197@30000, 1250@24000, 1130@0 |
| 6 | 465@0, 627@15000, 830@27000, 990@27000, 1120@24500, 1170@20000, 1100@0 |
| 7 | 510@0, 635@11500, 850@21000, 955@21000, 1072@19000, 1100@17000, 1070@0 |
| 8 | 540@0, 660@8000, 870@15000, 950@14500, 1030@13000, 1040@0 |
| 9 | 585@0, 677@5500, 890@10000, 990@9000, 1010@0 |

The rows pair exactly: -4 with +8, -3 with +6, -2 with +4 and -1 with +2 are the
same polygon. This is source data, not a decoding overlap. The 1 G row's fast
edge is 1,933 ft/s at 36,000 ft, 1,145 kt, the value in the fixed-wing audit for
F16C.

### Engines, fuel and sound

| Field | Value |
| --- | --- |
| Engines | 1 |
| Empty weight | 14,567 lb |
| Internal fuel | 6,972 lb |
| Maximum takeoff | 33,000 lb |
| Military thrust | 17,687 lbf |
| Afterburner thrust | 32,000 lbf |
| Military fuel flow | 2 lb/s |
| Afterburner fuel flow | 16 lb/s |

At the record's flows the internal tank lasts about 58 minutes at military power
or 7.3 minutes at full afterburner. These are host reading arithmetic, not a
claim about real endurance. The PT names four audio resources, all present in
`FA_2.LIB`: `loopSound` `&JET1N.11K`, `secondSound` `&JET1A.11K`, `engineOnSound`
`&POWERUP.5K` and `engineOffSound` `&POWERDN.5K`. `doDoppler` is 1, `maxSndDist`
15,000, the Doppler pitch limits are 320 and 160, and the Doppler speed window is
20 to 1,600. The `f.BI` AI script is present.

### Resources

All PT-named resources exist in `FA_2.LIB` except the cockpit art, which is in
`FA_1.LIB`. Hashes are the root `FA_2.LIB` copies unless stated.

| Resource | Role | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `F16.SH` | Shape | 37,376 | `7d570f1f705cfaa6b1288653e47e4c502d7606109044f306972bd6c63161f315` |
| `F16_S.SH` | Shadow shape | 4,608 | `ad87c11f0897eb9bc1e451f539795843d08e501fc69c3fd8c0c195aa3f050e17` |
| `F16_A.SH` | Damage body A | 25,088 | `0bf3a39736fe6a89dd53170a0fcc8b30298a812f534b72befb32ddf1779c9108` |
| `F16_B.SH` | Damage piece B | 16,896 | `8b03394f3bb7408c34eb0354a50453c9ee1af2ee95df0a8845af48f002688a1c` |
| `F16_C.SH` | Damage body C | 29,184 | `237a61d3d8417dc1bdccb270aed6614970d4b70e73783ebfeea3a88b990f89c2` |
| `F16_D.SH` | Damage piece D | 8,704 | `2b0be00c5f6c76e87f76867f0152fa67d4b40c7933bb9947ea3480856c3363f2` |
| `F16C.HUD` | HUD and cockpit pointers | 4,608 | `f18d6b5cb8c16116cc012429fb2efb074a1bb1c25b07b1718238b00379564cd5` |
| `F18R.SEE` | Radar | 481 | `b00861010ec23c39a082cd2b313a4a5223daf8ff6e7769f95353f77bbae01682` |
| `F18.ECM` | Jammer, chaff and flare counts | 366 | `0fb851d57d7f88f51a017d9663101b52c681c6a6d5ac55823588f25a942d31a9` |
| `VIS340.SEE` | Visual sensor | 471 | `445c35d1cc778513b0d38b73d36e10b47b7a4f210ffef87f54a53b17e6cd9efe` |
| `M61.JT` | Built-in gun | 2,703 | `0b79689a728911d1097569f6e6612050e414ba9703c9f131a51fe1ce9a7eb33d` |
| `F150.GAS` | Centreline tank | 204 | `ca9054921aa93eb890801a5a1f64deb55c8bcc6d03854ff03f1b42833a9f4c95` |
| `AGM65G.JT` | Default store | 2,777 | `b94ee0dae96dd2b7fc70a21e430444a1d72b565eec9611ecb749965ece2e5cca` |
| `AIM120.JT` | Default store | 2,778 | `702d559376e544b49d2d118e1a8911227b0abe6560ff84340a7fd592c709d7a8` |
| `AIM9M.JT` | Default store | 2,775 | `fdac769d40e374ed691a2758c80f4deddf44ecb136d8973374ea053ca03723cf` |
| `~F16H.PIC` | Forward cockpit (`FA_1.LIB`) | 124,274 | `5162b8a467e54d546f3a066f34befecdaabd82a154d3482e88521a055386d851` |
| `~F16_P.PIC` | Instrument panel (`FA_1.LIB`) | 6,864 | `223cf78dfab87fb5f9d1eaf3e4ba055525d7b8f546663b2cc146e07fa358b841` |
| `~F16.PIC` | Cockpit base (`FA_1.LIB`) | 23,832 | `52d195301a936092af8cbabf8e6ab02d629f94c362ea4faed74793b6365de49a` |
| `~F16S.PIC` | Cockpit surround (`FA_1.LIB`) | 190,852 | `b045ab827c47beef571afc990356bc3e5bb25682dc8fdb4f9a94035bc84f0768` |

The shape's faces name `_F16.PIC`, present in `FA_2.LIB` (111,864 bytes,
`d7822eded62964011b552a80aca1c8fbfe1faf34a0323c865fc572b4248d031b`), and the
damage looks name `_F16_A.PIC` to `_F16_D.PIC`, all present (`cc181f7e...`,
`09eaae19...`, `bfb2d8bd...`, `4b3602e2...`). `PAVESPK.SEE` exists in the media
(505 bytes) and is named only by `F16C.PT`. The two `F16C.HUD` builds have
identical parsed content but differ by four bytes near the module header
(`f18d6b5c...` in the root `FA_2.LIB`, `324012a1...` under `SETUP.ESA`); the
import report keeps whichever build the user supplied, and neither is aliased to
the other.

### Sensors, HUD and cockpit reuse

`F16C.HUD` is the exact PT name and the host must return `F16C.HUD`. Its panel
field at root +0x275 is `~f16_p`, so `panel_resource()` returns `~F16_P.PIC`,
which is the identity's `instrument_panel()` for the `F16` cockpit stem and is
present in `FA_1.LIB`. The title, button and press colour indices are 1, 0 and 0
and the primary ink index is 40. The HUD also names `~f16`, `~f16h` and `~f16s`,
and a `~raf_w` field at +0x282 that no reviewed code path reads; the host reads
only the panel field at +0x275.

The cockpit stem is `F16`, the same as the F-16C: forward `~F16H.PIC`, panel
`~F16_P.PIC`, and no left, centre or right overlay (`cockpit_overlays()` returns
three `None`s for the `F16` stem). The glass aperture and mirrors therefore come
from the reviewed F-16C rig. No new art or sensor is authored for F16A.

`F18R.SEE` is an assigned radar preset and `F18.ECM` a jammer generation in
`tore-sim::sensors::profile`, both already reviewed on the F-16C, so no new
preset is needed and no unreviewed record flies. `VIS340.SEE` is the signature-0
visual sensor. `F18.ECM` carries the chaff and flare counts F16A-4 reads.

### Reuse confirmation

- Shape, shadow, damage looks, cockpit art, HUD, radar and jammer are the same
  resources the approved F-16C names, reached through the same stems, so the
  existing F-16C rig, cockpit and sensors apply.
- The six state words are identical to the F-16C's: `0x8ad0` (+6 faces,
  afterburner flame), `0x8ad6` (+8, airbrake), `0x8adc` (+25, gear, raising the
  low point from -13 to -21 units, 7.0 ft), `0x8ae8` and `0x8aee` (0 new, the two
  flap words) and `0x8af4` (+2, rudder). PLANE flag `0x02` is clear, so there is
  no hook word. The reviewed F-16C whole-surface rig (`f16_animation.rs`) already
  drives these words on `F16.SH`, so F16A inherits the reviewed control motion,
  with no new fit.
- The default stores are the F-16C's live set. `M61.JT`, `F150.GAS`,
  `AGM65G.JT`, `AIM120.JT` and `AIM9M.JT` all already fly, and the
  `combat/live.rs` permitted list for `F16C` is `AGM65G.JT`, `AIM120.JT`,
  `AIM9M.JT` and `M61.JT`, which covers every F16A store. F16A-1 must add the
  same permitted list for the new identity, or the default load fails with
  "unreviewed live-fire station".
- The shape draws 356 neutral faces, 0 lines and 0 billboards, bounding box x
  -51 to 51, y -59 to 88 and z -13 to 39 shape units, about 34 ft span, 49 ft
  length and 17.3 ft height. The length matches the published 15.0 m exactly in
  the source's coarse units; the span and height run about 7 percent high against
  the published 31 to 33 ft and 16.5 ft.

### Published audit

Published columns are F-16A reference figures, not retail behaviour. Sources:
Aeropedia's F-16A page and Aero Web's F-16A page give empty 14,567 lb (6,607 kg),
maximum loaded weight with external stores 33,000 lb, and a 2,074 km/h (1,120 kt)
top speed at 36,000 ft; Aerospaceweb gives the F-16A internal fuel as 6,970 lb
and the empty weight as 15,585 lb; the F100-PW-200 is 14,870 lbf dry and 23,830
lbf with afterburner; the published maximum takeoff weight is commonly 37,500 lb
(FAS and the US Navy fact file). All sources accessed 2026-11-08.

| Field | Record | Published | Difference | Verdict |
| --- | ---: | ---: | ---: | --- |
| Empty weight, lb | 14,567 | 14,567 (early F-16A) to 15,585 | exact to 7% low | Agrees with the early F-16A |
| Internal fuel, lb | 6,972 | 6,970 | +0.03% | Agrees |
| Maximum takeoff, lb | 33,000 | 33,000 loaded to 37,500 maximum takeoff | exact to 12% low | A published loaded weight; the higher figure is the maximum takeoff |
| Military thrust, lbf | 17,687 | 14,870 (F100-PW-200) | +19% | Left as retail, listed for John |
| Afterburner thrust, lbf | 32,000 | 23,830 (F100-PW-200) | +34% | Left as retail, listed for John |
| 1 G top speed, kt | 1,145 | 1,120 to 1,174 | within 2% | Agrees |
| Ceiling, ft | 60,000 | 50,000 to 60,000 | at the top of the range | Agrees |
| G limits | +9 / -4 | +9 design | matches positive | Keep retail |

The thrust pair is the fleet template shared word for word with `F16C.PT` (and
`F18.PT`, `M2000.PT`, `F29.PT`, `CMCHE.PT`), about a third above the F-16A's
F100-PW-200; under the [record rule](../spec/aircraft-records.md#rule) it is
"only roughly off" and stays retail and listed for John, exactly as the audit
already lists it for the F-16C. The mass and fuel, by contrast, are an exact
early F-16A fit, so nothing here is a copy far from the real aircraft. No value
is corrected in F16A-0.

### Unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| OBJECT flag `0x2bf3` and the F16C's extra `0x800000` / `0x4000` | Uninterpreted; must not be mistaken for equipment | Trace the OBJECT flag consumers in the reviewed executable; F16A-1 can proceed without it |
| `viewOffset.z` 110 against the F16C's 60 | It sets the cockpit eye position a player sees | Resolved (F16A-5): decoded into the schema but no host code reads it. The cockpit camera uses the model origin and a shared fitted pilot eye (`mirrors::pilot_eye`), so the 110/60 difference is not player-visible and no frame differs |
| Source-default A2A quantity | `maxItems` is capacity, not a proven default load, and F16A has four carrier rows where the F16C has three | Review the source-default normalization in F16A-4 |
| The tank and carrier masks (`0x0605`, `0x07e5`, `0x07e5`, `0x0085`, `0x0085`) | They set which stores fit each row and differ from the F16C's | Decode and pin in F16A-4 with `--validate-ordnance` |
| High thrust against the F100-PW-200 | It sets acceleration and climb | List for John now; validate flight in F16A-6. No record change |
| Destroyed hulk identity | A and C are both damaged bodies and there is no separate hulk file | Confirm in F16A-2 which body draws as the hulk and that B and D are the pieces |
| `F16C_0.PIC` to `F16C_8.PIC`, `F16C_P.PIC` in `disc2/FA_3.LIB` | A second F-16C cockpit art set not in the identity closure | Resolved (F16A-5): the import closure selects `~F16*` from `FA_1.LIB`, which are `~F16.PIC`, `~F16H.PIC`, `~F16S.PIC` and `~F16_P.PIC`; no code path requests the `F16C_*.PIC` set |
| Streamer definition | The two points and zero pivot are unreviewed | Check the contrail or effect origin in F16A-2 or F16A-5 |

### F16A-1 hand-off

The record is ready for identity registration, with one plan correction: F16A
and F16C share mass, thrust, fuel and envelope, so F16A-6 must not expect an
F16C-different flight model.

F16A-1 should append `F16a` after `Mig25` (`ALL` 38, `SELECTABLE` 39), use
`typeSize` 660, return `F16.PT`, `F16.SH`, `F16C.HUD`, `~F16H.PIC`,
`~F16_P.PIC`, `F18R.SEE` and `VIS340.SEE`, keep `guns()` as `M61.JT`, and use
checkpoint code 38. It must set `has_pts()` false for `F16a`, because `F16.PTS`
does not exist and the importer would otherwise demand it. The reviewed names
are `F-16` and `F- 16A Falcon`, so `Aircraft::parse` accepts the record with no
name change. The deployed gear low point is -21 shape units (7.0 ft) and the F2
contact offset is -21; the debris scale is 1/3, the exponent-8 host factor
`combat/debris.rs` uses for every aircraft without a special case. The live
permitted list is `AGM65G.JT`, `AIM120.JT`, `AIM9M.JT` and `M61.JT`, and the
strike class is Fighter. The `variety` identity is `("F-16", "F16.SH")` with
clearance 7.0 ft, and it shares the F-16C layout spec (`31488`, 356 faces, the
six words) and the `SpecificRig::F16` rig. No other record is substituted for
any of these names, and `F16C` stays its own identity.

### F16A-5 cockpit and HUD confirmation

Slice F16A-5 confirmed the imported F-16A on the shared `F16` cockpit and the
shared `F16C.HUD`. `Airframe::load` rejects any aircraft whose HUD panel field
is not its instrument panel, so the clean loads and captures below are the
proof. No new art, sensor or preset was authored, and nothing unreviewed was
switched on.

- The HUD is `F16C.HUD`, byte-identical in parsed content in both archive
  builds (`FA_2.LIB` and `disc1/SETUP.ESA/FA_2.LIB`). Decoded root fields:
  panel `~f16_p` (+0x275), primary ink 40 (+0x72), title 1 (+0x2a8), button 0
  (+0x2a9), press 0 (+0x2aa). `panel_resource()` is `~F16_P.PIC`, present in
  `FA_1.LIB`, and `instrument_panel()` for the `F16` stem.
- The cockpit stem is `F16`: forward `~F16H.PIC`, panel `~F16_P.PIC`, base
  `~F16.PIC`, surround `~F16S.PIC`, and no left, centre or right overlay
  (`cockpit_overlays()` returns three `None`s). `--cockpit-sheet OUT_DIR f16a`
  reported `stem F16 cockpit ~F16H.PIC art 1280x490 NO MIRRORS FITTED`, and the
  F-16A render and art-mask tile are pixel-identical to the F-16C's.
- The glass aperture is the reviewed `F16C`/`F16A` polygon in `hud_aperture.rs`.
  The F-16 picture has no flat mirror fill, so the captures printed `Cockpit
  mirrors: 0 reviewed regions, 768x384 rear feed, every other visible frame`.
- All ten instrument pages draw: RCS, Envelope (5000 FT, 539 KTS), Front View
  and Other View (CAMERA LOADING), Target Cam (NO TARGET), RWR (scale 10), Nav
  Info (NO WAYPOINTS, 1 MISSION), Systems (THR 70%, TEMP 0%, OIL 100%, HYD
  100%, FUEL 6972 LBS, + EXT 990 LBS), Weapons (NO WEAPONS standalone; the live
  capture lists the default load) and Radar (NO SENSOR DATA standalone; the live
  capture shows TWS at 10 miles). The fuel reads the record's internal 6,972 lb
  plus the 990 lb `F150.GAS`, and the live Weapons page lists `313 M61`,
  `2 AGM-65`, `2 AIM-120`, `4 AIM-9M`, `30 CHAFF`, `30 FLARE`.
- The sensors are the reviewed F-16C presets, reached by the same record names:
  radar `F18R.SEE`, jammer `F18.ECM`, visual `VIS340.SEE`. `--sensor-summary`
  prints an F16A line byte-for-byte equal to the F16C's bar the identity.
- Absent systems stay absent: no gun reticle beyond the M61 gun row, no
  targeting sight of its own (Target Cam is the shared plain window), and no
  Pave Spike designator row (the record has none). The two `disc2/FA_3.LIB`
  `F16C_*.PIC` cockpit art sets are unused by every code path.

### F16A-6 flight review (2026-11-08)

Slice F16A-6 flies the F-16A's own record on the hybrid researched default and
found the flight numbers equal the F-16C's, field for field, exactly as F16A-0
predicted. The configuration is built from `F16.PT`, not shared with the
`F16C` identity: `AircraftModel::for_aircraft` reads the record the importer
loaded for `AircraftId::F16a`, and the `variety` identity is `("F-16",
"F16.SH")`. Nothing is aliased and no record value was corrected.

- The record and the model agree: empty 14,567 lb, internal fuel 6,972 lb,
  maximum takeoff 33,000 lb, 17,687 lbf military and 32,000 lbf afterburner
  thrust, 14 G rows from -4 to +9 and a 60,000 ft ceiling, byte-for-byte the
  `F16C.PT` values F16A-0 decoded.
- `tore-sim`'s `envelope_probe` over `F16.PT` and `F16C.PT` in one run printed
  identical envelope, level-speed and pull lines: the same 1 G edge (794 kt at
  sea level, 1,145 kt at 36,000 ft), the same settled level speeds (749 / 809 /
  885 / 1,001 / 1,040 / 716 kt) and the same pull G at every band.
- The battery's air maneuvers (`level`, `pull`, `loop`, `sprint`, `climb`,
  `dive`, `overspeed`, `rudder`, `devices`, `stall-recover`, `spin-recover`)
  are identical for the two identities apart from the `Aircraft:` name line.
- The seven ground maneuvers differ only in the default ordnance load, which
  F16A-4 set on the station record: the F-16A carries 3,894 lb (25,433 lb
  gross) against the F-16C's 4,854 lb (26,393 lb gross). With `--loadout none`
  the takeoff, gear-airborne, land, gear-up, hard, off-runway and nosewheel
  probes are identical apart from the name line, so the difference is stores,
  not a flight field.

The 1 G edge at 36,000 ft, 1,145 kt, sits 2 percent over the published
2,074 km/h (1,120 kt) at 36,000 ft and inside the published 1,147 to 1,176 kt
band at 40,000 ft, the same figure the F-16C flies. No `fitted_envelopes` entry
is added for F16A. The hybrid researched default is the validated path; the
legacy `--legacy-flight` adapter and the restricted `--native-flight-tables`
path stay distinct and unchanged.
