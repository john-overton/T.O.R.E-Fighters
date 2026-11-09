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

## MiG-31 source review (M3, 2026-11-08)

Research mode, slice M31-0. This reviews `MIG31.PT` and its dependency closure
before `AircraftId::Mig31` exists. `MIG31.PT` is the 39th retail identity, to be
appended in M31-1 (checkpoint code 39, the 40th `SELECTABLE` entry). The
[fixed-wing record audit](aircraft.md#fixed-wing-record-audit) predates this
identity; this section is its MiG-31 row. No original module was executed. PT,
SEE, ECM, JT and GAS are read as bounded BRF data with the shared field order in
`aircraft_schema.rs`; SH and HUD are inspected as data. The one exception to
"data only" is the static disassembly of `FA.EXE` (read with LLVM `objdump`,
never run) behind the HUD finding below. Retail media stays local and no
extracted bytes are committed. Slice done by Claude Sonnet (sonnet-medium).

Two questions are open for John and are **not decided here**: the missing
`y141.HUD` ([HUD](#the-missing-hud-for-john)) and the unreviewed `MIG31R.SEE`
and `MIG31.ECM` ([sensors](#the-unreviewed-radar-and-jammer-for-john)). Both
were decided by John on 2026-11-08 and implemented in M31-7; the review and its
result are recorded in that sensors section.

### Build and extraction

`FA_1.LIB` SHA-256 is
`657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` and
`FA_2.LIB` is
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`, the same
pair as the MiG-25 and F-16A reviews. `FA.EXE` is 1,319,424 bytes with SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`, the build
the [instrument bezel spec](../spec/instrument-bezel.md) uses. The closure was
extracted locally to `.local/m3-imports/scratch/mig31` with
`cargo run --locked -p tore-extract -- --source gameassets/fighters-anthology
--out .local/m3-imports/scratch/mig31` and `--include` globs for `MIG31*`,
`Y141*`, `VIS240.SEE`, `GSH6_30.JT`, `AA9.JT`, `AA8.JT`, `F250.GAS` and `*.HUD`.
The `SETUP.ESA` installer copy of `FA_2.LIB` was extracted too. Its PT, SH, SEE,
ECM, JT and GAS bytes are identical to the root copy; only the HUD files differ
by build (`YAK141.HUD` differs in 4 bytes, `SU33_C.HUD` carries its own hash).
No `MIG31.PTS` exists in any archive (an explicit exception, as for the MiG-25).

### Identity and presentation

| Field | Decoded value |
| --- | --- |
| PT | `MIG31.PT`, 1,680 stored and 9,723 decoded bytes, SHA-256 `fea8d9cf5c645ee9bf800d24cf10f3bf143c171369c7b4c110a622e1b2325320` |
| Displayed names | `MiG-31`, `MiG-31 Foxhound`, `MIG31.PT` |
| Schema | OBJECT + NPC + PLANE, 219 tokens; `structType` 5; `typeSize` 588; `instanceSize` 575 |
| Object class | `0x8000` (fighter); the PLANE no-lift bit is clear |
| Year / hit points | 1983 / 167; `expType` 30, `craterSize` 0 |
| Shape | `MIG31.SH`, exponent 8 (scale 1), so 1 shape unit is 1/3 ft in the aircraft renderer |
| Shadow | `MIG31_S.SH`, 10 faces, no texture |
| HUD | `y141.HUD` in the PT, **absent from every archive** (see below) |
| Cockpit | none of its own: no `~MIG31*` picture exists in any archive |
| Damage looks | `MIG31_A.SH` 229 faces, `MIG31_B.SH` 38, `MIG31_C.SH` 240, `MIG31_D.SH` 29 |
| PTS | absent, an explicit exception |
| Checkpoint code | 39 reserved for M31-1 (F-16A is 38), not yet in `checkpoint_shared.rs` |
| CLI key | `mig31` and `mig31.pt` reserved for M31-1 |
| AI script | `f.BI`, present (6,081 stored bytes) |
| Radio callout | `^ACMIG31.5K` exists (4,097 stored bytes). The PT does not name it; the original builds `^AC` plus the PT stem ([radio](radio.md)) |
| Encyclopedia | `MIG31.INF` (Jane's-style text) and `MIG31_0/1/C/E.PIC` in `disc2/FA_3.LIB`; reference media, not flight data |

All fixed names in the plan are confirmed in the media except the HUD:
`MIG31.PT`, `MIG31.SH`, `VIS240.SEE`, `MIG31R.SEE`, `MIG31.ECM`, `GSH6_30.JT`,
`AA9.JT`, `AA8.JT`. The displayed names are exactly "MiG-31" and "MiG-31
Foxhound". `typeSize` is 588, not the parser's default 660: M31-1 must add
`Mig31 => 588`. The PLANE flags are `0x15`: seat `0x10`, multi-crew `0x04` and
the base bit `0x01` (no reviewed meaning) set; hook `0x02`, no-lift `0x08` and
carrier `0x40` clear. So the MiG-31 has no hook, and the multi-crew bit is set
(the real aircraft has a pilot and a weapons operator). OBJECT `flags` is
`0x2bf3`, as the MiG-25.

### Resources

| Resource | Role | Decoded bytes | SHA-256 |
| --- | --- | ---: | --- |
| `MIG31.SH` | Shape | 25,088 | `6dc4ed9cb6c99675cf7076dd38c90146e2e6300acd3c11bc565a5269f8a46a89` |
| `MIG31_S.SH` | Shadow shape | 4,608 | `063b9f28070af4acd9953c716c93264ad0c9adc843a678e64ef2a60befb289d2` |
| `MIG31_A.SH` | Damage body A | 16,896 | `374e24798d954b0cda67a54269b68495dc480d4f4150c9fadc3815ae126631ce` |
| `MIG31_B.SH` | Damage piece B | 4,608 | `6aff8fdab918c6fbfffb53e203c1f94423787fc53352efe2b0cad75057a384f0` |
| `MIG31_C.SH` | Damage body C | 16,896 | `4199aa5ed4ab3fc02de433937bab64790cdd69212579abe71a32c1e41ef4f710` |
| `MIG31_D.SH` | Damage piece D | 4,608 | `9ab111e2920423ea579c2cf82adc5a5ffca6afa74e2907322ac04a6913116c53` |
| `MIG31R.SEE` | Radar, **unreviewed** | 494 | `e7aa12bd307d1b24b0b8fd9594cdadece542e27b6343656ae0eff5cb3a0ca669` |
| `MIG31.ECM` | Jammer, **unreviewed**, chaff 60 and flare 60 | 354 | `874f7b364ce6068341d6a17e10c4b73b37efcf86ec0d528a62f93e9c0296e56e` |
| `VIS240.SEE` | Visual sensor, signature 0 | 471 | `a3224581abef75e0fe4e6dbb37d2d47d5ab4704bb1043e09dd6aa2e5a4510ee1` |
| `GSH6_30.JT` | Internal gun | 2,717 | `4fc94198cbd0e40de6fc314dc73502dc662d106ffa9d14291939f4f779120cf4` |
| `AA9.JT` | Default A2A store | 2,755 | `38adf9f9cfc00aee15ed729553c98597c7a92ccfb6323b12a6c26a8016192f4c` |
| `AA8.JT` | Default A2A store | 2,751 | `c656407564e307f9eba70b853ef7cf226ea4a91a4d3cabae88062e6128cf7008` |
| `F250.GAS` | Not named by this PT (see hardpoints) | 205 | `974e49de6eb22237bf7a1bd24b3ed9308300bbb95270a9c0fe7104892514f0de` |

The shape and damage looks name `_MIG31.PIC` (9,657 stored bytes),
`_MIG31_A.PIC` (14,967), `_MIG31_B.PIC` (7,969), `_MIG31_C.PIC` (17,162) and
`_MIG31_D.PIC` (6,158), all in `FA_2.LIB`. The four sounds are present in
`FA_2.LIB`: `&JET1N.11K` (loop, 29,861 stored bytes), `&JET1A.11K` (second sound,
23,585), `&powerup.5k` (34,262) and `&powerdn.5k` (23,177). The MiG-25 used a
different loop pair (`&jet19.11k`). `doDoppler` is 1, `maxSndDist` 15,000, the
Doppler pitch limits 320 and 160 and the speed window 20 to 1,600, as the
MiG-25.

### The record against the MiG-25 template

The MiG-31 record is built on the same template as `M25.PT`: 178 of 219 fields
are identical (turn rates, stall and spin rows, drag words, crash speeds, damage
table, structure limits 2,560 and 5,120). The 41 that differ are the identity
and resource names, the year, the weights, thrust, fuel and flow, the envelope
rows, the hit points and structure, the signatures, the view offset, the
sounds, the PLANE flags, the rudder yaw rate, `_gpullDrag` (58 against 23),
`throttleAcc` (30 against 50), the loaded control words (36 against 38) and the
debris positions. All five mass and thrust fields differ from the MiG-25's, and
no other PT in `fa-aircraft.csv` shares even one of them, so this is not a copied
record under the [record rule](../spec/aircraft-records.md#rule). Selected
values:

| Field | Value |
| --- | --- |
| Empty weight / internal fuel / maximum takeoff | 48,115 / 30,865 / 91,725 lb |
| Military / afterburner thrust (both engines) | 33,510 / 48,500 lbf |
| Fuel flow, military / afterburner (both engines) | 6 / 36 lb/s |
| Altitude | `minAlt` 300 ft, `maxAlt` 68,000 ft |
| G envelope | `envMin` -2, `envMax` +4 (the MiG-25 is +5) |
| Hit points / structure | 167 / 1,375 and 2,742, warn 2,560, limit 5,120 |
| Signatures | 130, 130, 100, 100, 0; `maxVisDist` 321 |
| Rudder yaw | min -1, max 1, acc 1, dacc 3 (the MiG-25: 3, 3, 3, 9) |
| View offset | x 0, y 20, z 60 source units (0 / 6.7 / 20 ft); the MiG-25 is 0 / 11 / 71 |
| Debris | `dmgDebrisPos` -60, 0, 0; `dstDebrisPos` 0, 20, -90 (source units) |
| Crash | forward 330, side 51, vertical 95; pitch 25, roll 10 |

### G envelope rows

Points are speed in ft/s at altitude in ft. Seven consecutive rows, `envMin` -2
to `envMax` +4.

| G | Points |
| ---: | --- |
| -2 | 460@0, 565@16,500, 792@29,750, 1,020@35,000, 1,517@34,750, 1,680@26,000, 1,495@16,000, 1,180@0 |
| -1 | 310@0, 510@36,500, 700@57,250, 970@61,000, 1,435@60,500, 1,890@56,250, 2,265@48,125, 2,440@36,000, 1,925@24,000, 1,310@0 |
| 0 | 190@0, 260@19,500, 330@38,250, 515@66,125, 970@71,062, 1,490@70,500, 2,055@66,250, 2,592@54,625, 2,796@40,812, 2,806@36,000, 2,163@24,000, 1,450@0 |
| 1 | 240@0, 395@38,000, 577@64,000, 960@68,000, 1,465@68,000, 2,012@64,000, 2,526@53,500, 2,733@40,750, 2,743@36,000, 2,056@24,000, 1,370@0 |
| 2 | 310@0, 510@36,500, 700@57,250, 970@61,000, 1,435@60,500, 1,890@56,250, 2,265@48,125, 2,440@36,000, 1,925@24,000, 1,310@0 |
| 3 | 390@0, 505@20,500, 672@35,750, 950@47,000, 1,490@47,000, 1,845@42,000, 2,010@32,000, 1,790@24,500, 1,250@0 |
| 4 | 460@0, 565@16,500, 792@29,750, 1,020@35,000, 1,517@34,750, 1,680@26,000, 1,495@16,000, 1,180@0 |

The -2 and +4 rows are the same polygon, and the -1 and +2 rows are the same
polygon. This is source data, not a decoding overlap. The 1 G row peaks at
2,743 ft/s (1,625 kt) at 36,000 ft and 1,370 ft/s (812 kt) at sea level; the 0 G
row has a point at 71,062 ft, above `maxAlt`, and the host caps altitude at the
record field.

### Published audit

The comparison column is the game's own encyclopedia text (`MIG31.INF`, retail
media, quoted as numbers only) and, for thrust, public figures: Wikipedia's
Soloviev D-30 page and MILAVIA's MiG-31 specifications (both accessed
2026-11-08) give the Aviadvigatel D-30F6 as about 93 kN (20,900 lbf) dry and
151.9 kN (34,170 lbf) with afterburner per engine. These are real-aircraft
figures and do not establish original game behaviour.

| Field | Record | Published | Difference | Verdict |
| --- | ---: | ---: | ---: | --- |
| Empty weight, lb | 48,115 | 48,115 (21,825 kg, encyclopedia) | 0.0% | Agrees exactly |
| Internal fuel, lb | 30,865 | 36,045 (16,350 kg, encyclopedia) | -14.4% | Left as retail, listed for John |
| Maximum takeoff, lb | 91,725 | 90,390 (41,000 kg, encyclopedia) | +1.5% | Agrees |
| Military thrust, lbf | 33,510 | 41,800 (2 x 20,900) | -19.8% | Left, within a third |
| Afterburner thrust, lbf | 48,500 | 68,340 (2 x 34,170) | -29.0% | Left, within a third (near the edge) |
| Ceiling, ft | 68,000 | 67,600 (20,600 m, encyclopedia) | +0.6% | Agrees |
| 1 G top speed, kt | 1,625 | 1,620 (3,000 km/h at 17,500 m, encyclopedia) | +0.3% | Agrees |
| Sea-level top speed, kt | 812 | 810 (encyclopedia) | +0.2% | Agrees |
| G limit | +4 / -2 | +5 supersonic (encyclopedia) | -1 G positive | Keep retail (M31-8) |
| Year | 1983 | Delivery began by early 1983 (encyclopedia) | 0 | No change |

Agent audit decision, 2026-11-08: leave the values as retail and list them for
John, the same rule as the MiG-25. The afterburner thrust is 29.0% under the
real engines, close to the one-third line; M31-8 should watch the afterburner
climb and acceleration against the envelope rows. Fuel flow of 6 and 36 lb/s
totals gives, as plain arithmetic, about 85.7 minutes of military power or 14.3
minutes of full afterburner from the internal tank. These are host reading
numbers, not a claim about real endurance; the flow is for M31-8.

The record's own budget: empty plus internal fuel is 78,980 lb, leaving 12,745
lb below its 91,725 lb maximum. Four `AA9.JT` at 1,080 lb and two `AA8.JT` at 145
lb are 4,610 lb of source stores. The default quantities are a host rule for
M31-6.

Shape size, from the runtime state path at scale 1: x -81 to 81, y -68 to 154
and z -21 to 38 shape units, so 54.0 ft across, 74.0 ft long and 19.7 ft high.
The length (74.4 ft real) and height (20.2 ft real) agree; the **span is 22%
wide of the real 13.46 m (44.2 ft)**. The outer wing pieces reach x +/-81 (the
two outer wing quads at the tip) and the 6138 and 613e panels start at x +/-17
to +/-45. This is a model fact for M31-2 to record, not a defect to fix
here.

### Hardpoints

All six source rows, in original order. Positions are source units; feet divide
by three; x right, y up, z forward ([aircraft
ordnance](aircraft-ordnance.md#hardpoint-positions)). Location byte: 0
Centerline, 1 Fuselage, 2 Internal Gun, 3 Internal Bay, 4 Wing, 5 Wingtip.

| Row | Location | Flags | Position x/y/z | Feet right / up / fwd | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `VIS240.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `MIG31R.SEE` | Fixed radar |
| 2 | Fuselage | `0x0008` | 0 / 0 / -50 | 0 / 0 / -16.7 | 1 | 0 | `MIG31.ECM` | Fixed jammer |
| 3 | Internal Gun | `0x0008` | 5 / -5 / -5 | 1.7 / -1.7 / -1.7 | 260 | 0 | `GSH6_30.JT` | Internal gun, 260 rounds |
| 4 | Wing | `0x07e5` | 25 / -10 / 0 | 8.3 / -3.3 / 0 | 4 | 0 | `AA9.JT` | A2A row, fuselage-side wing row |
| 5 | Wingtip | `0x0085` | 50 / -10 / 0 | 16.7 / -3.3 / 0 | 2 | 8 | `AA8.JT` | A2A row, outer wing |

Differences from the MiG-25 table that M31-5 and M31-6 must handle:

- There is **no tank row and no centreline row**. `F250.GAS` is not named by
  this PT; the MiG-31 has no external tank in the record. (The encyclopedia text
  mentions provision for two underwing tanks; the record does not.)
- The gun is a fixed internal row (Internal Gun location, 260 rounds at
  1.7 ft starboard of the reference point), unlike the MiG-25's none. The
  encyclopedia says the real gun is a starboard fairing on the lower fuselage.
  It is `GSH6_30.JT` (the MiG-23's gun record), not `GSH6_23.JT`; the in-game
  text calls the real gun a GSh-6-23, which this record's name does not follow.
  The record decides the weapon, not the text.
- The AA-9 row is a Wing row with weight class 0, so its capacity is its
  `maxItems` of 4 (the weight formula applies only to a nonzero class). The
  AA-8 row has class 8 and `maxItems` 2. There is **no AA-6 row** even though the
  encyclopedia lists two AA-6 on the inner pylons; adding one is outside this
  slice (the record decides the roster of stores).
- The Wing row sits 8.3 ft out and the Wingtip row 16.7 ft out, but the model's
  wing reaches 27 ft. The Wingtip row is therefore mid-wing on this shape. The
  standard pylon rules and clipping stop-and-report rule apply in M31-5.
- The jammer row has a z of -50 (-16.7 ft), the only non-origin fixed row. It
  has no known meaning (nothing draws an ECM); it is recorded, not used.
- `AA9.JT` is a semi-active radar missile (AA-9, 41.15 nmi; see
  [aircraft ordnance](aircraft-ordnance.md#aa-6-aa-9-pl-7-and-pl-10-jt-4-2026-11-06)).
  This is its **first imported default carrier**; it is live on compatible
  stations since JT-4. `AA8.JT` is live and is the MiG-29's default.

### Shape, damage and animation handoff

`MIG31.SH` has 214 runtime faces (216 on the export path), 0 lines and 0
billboards, one texture atlas. The shape has six state words, listed below with
the counts from `Shape::with_state` (the export branch adds two faces to every
count). The streamer definition has a zero pivot, hinge scale 40 and two
identical points. Damage bodies A (229 faces) and C (240) and pieces B (38) and D
(29) follow the existing aircraft damage contract; there is no separate hulk
file. The shadow is 10 faces with no texture. The parked path knows this
aircraft already: `parked_aircraft.rs` lists `MIG31.PT` with gear word `0x612c`,
and the parked inspector reports contact offset -30, exactly the gear-down low
point of -30 shape units (-10.0 ft). The MiG-25's two differed by one unit; here
they agree.

| Word | Faces | Low point | Geometry (shape units) | Reading | Status |
| --- | ---: | ---: | --- | --- | --- |
| `0x6120` | +12 | -9 | Twelve plates behind the tail at x -14 to 14, y -129 to -68, z -9 to 5 | Afterburner flame, aft of the nozzles | Confirmed; mapped to exhaust in M31-4 |
| `0x6126` | +2 | +8 | Raised quads at x +/-5, y 32 to 50, z 8 to 24 | Dorsal airbrake by slot order | Confirmed as the source's brake word; mapped to brake in M31-4. The source geometry is the raised dorsal panel; the encyclopedia's under-intake text is not used for the rig |
| `0x612c` | +12 | -30 | Wheel and strut groups at x +/-17, y 7 to 95, z -30 to -10 | Landing gear down | Confirmed; the parked gear word, mapped to rigid gear in M31-4 |
| `0x6138` | -2 | -21 | Removes the left trailing-edge pair at x -17 to -45, y -5 to -23, z 2 to 4 | Left flap pair | Confirmed; the M31-3 flap word |
| `0x613e` | -2 | -21 | Removes the mirrored right pair | Right flap pair | Confirmed; the M31-3 flap word |
| `0x6144` | 0 net | -21 | Swaps four fin faces (two a side) at x +/-13 to +/-17, y -55 to -42, z 8 to 31 for four whose lower edge sits 4 units outboard on both sides | A rudder-sense deflection of the twin fins (the same sign on both sides) | Confirmed; the M31-3 rudder word |

The first five words follow the shared base order (flame, brake, gear, flap,
flap) used by the MiG-25, F-18, Rafale and F-104 families, with the sixth word
after them. No hook word exists because the PLANE hook flag is clear. The state-path and export-path counts differ by two faces in every row, as on
the MiG-25. The two export-only faces are quads with indexed textures
(`@indexed:3` and `@indexed:4`, the decal slots) on the upper wing surface, one
at x -58 to -37 and one at x 54 to 75, z about 0 to 3: wing markings. Decal slots
are unimplemented in the host and must never be filled with skin (see
[texture state and decals](objects-and-shapes.md#texture-state-and-decals));
M31-2 handles them.

### Sensors and stores

`VIS240.SEE` is the reviewed signature-0 visual sensor. `MIG31R.SEE` and
`MIG31.ECM` are decoded in full in [the sensors section
below](#the-unreviewed-radar-and-jammer-for-john). `GSH6_30.JT` is the live
MiG-23 gun. `AA9.JT` and `AA8.JT` are live stores. The default quantities,
pairing on the Wing and Wingtip rows, and the 260-round gun count are M31-5 and
M31-6 questions.

### The missing HUD (for John)

**Fact.** `MIG31.PT` names `y141.HUD` (`hudName`, OBJECT offset 162). No
`Y141.HUD` exists in any archive of the installed media (`FA_1.LIB`,
`FA_2.LIB`, `disc2/FA_3.LIB`, the `SETUP.ESA` copies, `FA_4B.LIB`, `FA_4D.LIB`);
an extraction with `--include '*.HUD'` finds 46 HUD files, all in `FA_2.LIB`
(and its `SETUP.ESA` copy), and `y141` is not among them. `Y141.SH`, its damage looks and the `~Y141*` cockpit
pictures exist, because they belong to the Yak-141. `YAK141.PT` itself has a
**null** `hudName`, and the host maps it to `YAK141.HUD` by convention. So
`y141` in the MiG-31's PT reads as a Yak-141 stem left in the record, but that is
an inference from naming; naming does not establish behaviour.

**What the original does (static reading of `FA.EXE`, not run, build above).**
The routine that copies the HUD root to `0x521360` (`0x406193`, the one the
[bezel spec](../spec/instrument-bezel.md) cites) first builds a file name, and
then loads it with the resource-manager wrapper at `0x4a6ae0`:

1. If the PT's `hudName` pointer (`0x50d30a`, which is the PT root plus 162, with
   the names pointer at `0x50d26d` being root plus 5) is non-null, the string is
   copied as the name (`0x406126` to `0x406152`).
2. If it is null, the name is the third `ot_names` string (the PT name), with its
   extension replaced by the `.HUD` string at `0x4ebe7c` (`0x406154` to
   `0x40616f`, using the extension-replacing copy at `0x4a6870`). This is the
   "same-stem HUD" rule the bezel spec already records for null pointers.
3. It calls `0x4a6ae0(name, 0)` (`0x40617b`). With flags 0, the finder
   (`0x4a6b30`) returns null **silently** when the resource is not found: the
   missing-resource error branch only runs when bit `0x80` of the high flag byte
   is set (`0x4a6bcc`), and otherwise it returns 0 (`0x4a6cae`).
4. If the result is null, it calls `0x4a6ae0` again with the string at `0x4ebe74`,
   which is **`f18.HUD`**, and flags `0x8000` (`0x406184` to `0x40618e`). With
   that flag a miss reports "RMFindAndLoad: can't load %s" through the fatal
   error routine `0x44a120`.
5. The 0x2b2-byte root of whichever module loaded is copied to `0x521360`.

So for `y141.HUD` missing, the original appears to fall back to **`F18.HUD`**,
with no error. That module names `~f18`, `~f18h`, `~f18s`, `~f18_l`, `~f18_c`,
`~f18_r`, the overlays and the panel `~f18_p`, so the cockpit pictures and the
bezel would be the F/A-18's. Not established, and recorded as unknowns: what the
finder at `0x4ad3c0` searches and whether it is case sensitive; whether the
retail game lets the player fly the MiG-31 at all (the encyclopedia and the PT
exist; no flight check was possible); and what the caller does after loading
with the root fields (the F/A-18 panel name sits at root +0x275).

**Evidenced options for John** (none chosen; each is a presentation choice, to be
recorded as opinionated with John's date, as the Su-35, F-14 and A-4E choices
are):

| Option | Evidence for it | Cost or catch |
| --- | --- | --- |
| `F18.HUD` | The original's static fallback when the named HUD is missing (above). `F18.HUD` is present (4,608 bytes). The F-18 cockpit family and glass aperture are reviewed. | The player sees an F/A-18 cockpit and bezel in a Soviet interceptor. Only the static fallback evidences it; the load-and-use path was not run |
| `YAK141.HUD` | The PT's `y141` stem; the only HUD with that stem; the Yak-141 cockpit family (`~Y141H.PIC`, `~Y141_P.PIC` and the overlays) is complete and reviewed. | Shows another aircraft's cockpit, and the stem is only a naming match. The two builds of `YAK141.HUD` differ by 4 bytes. The glass aperture and the overlay set are the Yak's |
| `SU33CC.HUD` (or `SU33_C.HUD`, byte-identical in a build) | What the MiG-29, Su-25 and MiG-23 use (`SU33CC.HUD`) and the MiG-25 (`SU33_C.HUD`); the MiG-31 record is built on the MiG-25 template (178 of 219 fields equal). Reviewed glass (`SU33` maps to the MiG-29 aperture), side overlays and bezel; no new art | No evidence that the original used it. It also repeats `~SU33_W.PIC`'s missing optional picture, as the other four do |
| Another Soviet HUD (`AV8.HUD` as the Su-27, `SU35.HUD`, `MIG21.HUD`, `MIG17.HUD`) | Same family of bezel art; all present and reviewed | No evidence either; the Su-27 uses `AV8.HUD`, a Harrier module, so a shared cockpit need not match the aircraft |

Whatever is chosen, two things hold. The viewpoint is the record's own
(`viewOffset` 0 / 20 / 60), so any borrowed cockpit art must be checked for eye
point and glass in M31-7. And the host must return the exact PT name only when
that file exists; a substitute is an explicit, labelled presentation mapping, not
an alias of the record. M31-7 cannot start the cockpit until John picks. M31-1
can import the aircraft with the choice recorded in `aircraft.rs` once made; if
John wants `F18.HUD`, no new resource work is needed beyond naming it.

### The unreviewed radar and jammer (for John)

Both records are loaded by EQ-1 as unreviewed profiles and never fly
(`sensors/profile.rs`; [radar](../radar.md#profile-live-state-and-presentation)).
The rest of this section decodes them. Nothing here enables either.

**`MIG31R.SEE`** (494 bytes, SHA-256 in the resource table), decoded with the
shared sensor schema:

| Field | Value |
| --- | --- |
| Type / names | 10 / "Flash Dance", "Flash Dance", `MIG31R.SEE` (NATO's name for the Zaslon radar) |
| Signature | 3 (radar channel); `flags` 0, `flags[1]` 1 |
| Look-down | 30; Doppler above, below and minimum range 0; all-aspect 0 |
| Search volume | half-angles 10,920 / 8,190 source units, which is 60.0 degrees azimuth and 45.0 degrees elevation; range 0 to 753,424 ft (124.0 nmi); altitude unbounded |
| Track volume | the same angles; range 0 to 607,600 ft (100.0 nmi); altitude unbounded |
| Chaff/flare chance, deception chance | 100, 100 |

It is byte-for-byte `F15R.SEE` apart from the name strings and the search range
(753,424 ft against `F15R.SEE`'s 911,400 ft; the track range, the angles and
the look-down 30 are equal). The nearest decoded siblings:

| Record | Look-down | `flags[1]` | Search / track, nmi | Preset in code |
| --- | ---: | ---: | --- | --- |
| `MIG31R.SEE` | 30 | 1 | 124.0 / 100.0 | none (unreviewed) |
| `F15R.SEE` | 30 | 1 | 150.0 / 100.0 | Advanced |
| `F14R.SEE` | 30 | 1 | 190.0 / 150.0 | Transitional |
| `SU27R.SEE` | 0 | 1 | 150.0 / 115.0 | Advanced |
| `MIG29R.SEE` | 0 | 1 | 62.0 / 43.0 | Advanced |
| `F18R.SEE` | 0 | 1 | 90.0 / 50.0 | Advanced |
| `MIG27R.SEE` | 50 | 0 | 55.0 / 40.0 | Basic |

The in-game encyclopedia gives the real radar as a 108 nm search and 65 nm track,
unlike the record's 124 and 100; the retail numbers are game numbers, and the
authored detection model uses the record's.

**`MIG31.ECM`** (354 bytes): type 9; weight 0; `flags` 0; mode flags `0x1f0`
(the `0x10` bit is set, so a reviewed jammer would emit radio-frequency noise);
chaff loaded 60, chance 35, heading 95, pitch 24; flares loaded 60, chance 35,
heading 159, pitch 31; radar deception chance 30 with signature add 100;
noise ranges 0 and 0; infrared deception chance 40 with signature add 100; lose
lock time 0. It is byte-for-byte `SU24.ECM` **and** `SU27.ECM` apart from the name
string. In code `SU24.ECM` is `Transitional` and `SU27.ECM` is `LateColdWar`, so
the record alone does not say which generation it is. `MIG29.ECM` has the same
fields with chaff and flares of 30 and is `Transitional`. The Jaguar and the
Tornado also carry `MIG31.ECM` (non-roster; they are not imported).

**What reviewing them would need** (research and a John decision, none done):

1. **Radar.** John picks a `Preset` (Basic, Transitional or Advanced) for
   `MIG31R.SEE`, the same decision EQ-1 took for each reviewed radar by
   installed record. The evidence points at Advanced (`F15R.SEE` is the same
   data) or Transitional (`F14R.SEE` shares the look-down 30); it does not decide
   it. The change is one match arm in `Preset::for_record`, the new row in the
   [preset table](../radar.md) and its provenance line, then the sensor tests
   (`sensors/profile.rs`, `sensor_equipment_tests.rs`) and `--sensor-summary`.
2. **Jammer.** John picks a `Generation` (Early, Transitional or LateColdWar) for
   `MIG31.ECM`: one match arm in `Generation::for_record`, plus the same docs and
   test lines. It would also apply to the Jaguar and Tornado if they are ever
   ported. The chaff and flare counts (60 and 60) are read from the record and
   do not depend on the generation: `ecm.chaff[0]` feeds the live countermeasure
   devices directly (`combat/live.rs`), so M31-6 can use them either way.
3. **Effect on the loadout.** `AA9.JT` is a semi-active radar missile whose
   guidance needs the aircraft's radar lock throughout flight
   ([missiles](../spec/missiles.md)). With no reviewed radar the MiG-31 has no
   flying radar channel, so its radar missiles have no support source and it has
   no radar scope. Whether the live code refuses to fire, or fires an unsupported
   missile, was not run here; M31-7 must check. `AA8.JT` (infrared), the gun and
   `VIS240.SEE` do not depend on the radar.
4. **Fly without** keeps both profiles unreviewed (the current behaviour). The
   aircraft would then show no radar and no radio-frequency jamming, the AA-9
   would be carried without guidance, and the audit would list the aircraft as
   "radar unreviewed". That is a valid state, but the MiG-31 is an interceptor
   whose main weapon is the radar missile, so it leaves the identity close to a
   static model.
5. **Docs.** `docs/radar.md` (preset and jammer tables and the plan list),
   `docs/research/m3-asset-inventory.md` rows 565 and 619, and the sensor row of
   `docs/aircraft-import.md`.

Neither record is enabled by this slice, and no new preset is proposed without
John.

M31-7 then reviewed both by John's decision of 2026-11-08 (below), so
`MIG31R.SEE` and `MIG31.ECM` now fly.

**Reviewed (M31-7, 2026-11-08).** John chose, and M31-7 implemented, one match
arm each in `crates/tore-sim/src/sensors/profile.rs`:

| Record | Review | Basis (opinionated, John 2026-11-08) |
| --- | --- | --- |
| `MIG31R.SEE` | `Preset::Advanced` | The `F15R.SEE` class of the record it copies; its own decoded 124 nmi search range is kept, not replaced |
| `MIG31.ECM` | `Generation::LateColdWar` | The generation of the byte-identical `SU27.ECM`; the other sibling, `SU24.ECM`, is Transitional and does not decide it |

The MiG-31 now reads, from `--sensor-summary`: `radar MIG31R.SEE search 124 nmi
60x60 deg track 100 nmi 45x45 deg look-down 30 ADVANCED; ... jammer MIG31.ECM
LATE strength 0.30`. The radar gives the semi-active `AA9.JT` its required
launcher illumination, so the AA-9 guides (checked in M31-7; it killed the
range fixture in a 3,600-tick live-fire probe). The chaff and flare counts (60 and 60) come from the
record and never depended on the review.

### M31-8 flight review (2026-11-08)

The MiG-31 flies the hybrid adapter's own `variety` configuration, built from
its `MIG31.PT` (`MIG31.SH`) record, and the model reads the record's own values:
empty 48,115 lb, internal fuel 30,865 lb, maximum takeoff 91,725 lb, 33,510 lbf
military and 48,500 lbf afterburner thrust, environment rows -2 to +4 G and a
68,000 ft ceiling. No envelope, G or flow fit was added and no record value was
changed.

- The 1 G row's fast edge decodes to 2,743 ft/s at 36,000 ft, 1,625 kt, against
  the published 3,000 km/h (1,620 kt) at 17,500 m, plus 0.3 percent. The
  sea-level 1 G edge is 1,370 ft/s, 812 kt, against the published 810 kt, plus
  0.2 percent. Both are inside the decoded envelope, so `fitted_envelopes` leaves
  the record untouched: this is the source's top speed, not a fit.
- The shared loaded-drag rule makes full internal fuel settle at 0.915 or 0.916
  of the edge at 1,000, 10,000, 20,000, 30,000 and 36,000 ft, inside the
  fighters' 87 to 96 percent band.
- The G limits are the source's +4/-2 rows, so the loaded 4 G row gives 3.25 G
  at the default gross weight (36 percent loaded elevator, 0.735 loading). The
  positive limit is one G under the real aircraft's published +5 supersonic
  limit; it stays retail (M31-0, no record change).
- The record's fuel flows are 6 lb/s at military power and 36 lb/s at full
  afterburner. That is, as plain arithmetic, about 85.7 minutes of military power
  or 14.3 minutes of afterburner from the 30,865 lb tank, far longer than the
  real D-30F6's specific fuel consumption allows; the record rule keeps the flows
  with the tank, which is left as retail, so the flow is a gap for John, not a
  correction.
- The afterburner thrust is 29 percent under the real engines (M31-0). It still
  fills the envelope: the full-afterburner climb probe tops 77,009 ft (1.132 of
  the 68,000 ft ceiling) before the tank runs dry, and the 300-second sprint
  reaches 842.3 kt at 5,000 ft, 0.940 of the 1 G edge there, so the reduced
  thrust does not leave the aircraft short of its decoded top speed in level
  flight.
- The seven ground probes take off, land and stop as the shared rules expect:
  liftoff tick 1,949 at 165.8 kt over 2,309 ft, approach 166.2 kt, touchdown
  167.9 kt at 12.0 ft/s sink and a 100 score; gear-up and off-runway crash by
  design and the nosewheel does not crash.

The liftoff range is the heavy-interceptor plausibility band (160 to 190 kt) and
the approach range is fitted from the published 280 km/h landing speed
(airbase.ru, read 2026-11-08), a plus or minus 5 percent window; both are agent
decisions, and the measured 165.8 kt and 166.2 kt are recorded as regression
values
([variety flight baseline](../baselines/variety-flight.md#mig-31-validation-2026-11-08)).

The hybrid researched default is the validated path. The legacy `--legacy-flight`
adapter and the restricted `--native-flight-tables` path stay separate and are
not changed.

### Unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| The HUD | `y141.HUD` is missing; the cockpit cannot start | Resolved in M31-7: John chose `YAK141.HUD` with the Y141 cockpit and `~Y141H.PIC` (opinionated, 2026-11-08); it draws |
| Radar preset and jammer generation | Both unreviewed, so no flying radar | Resolved in M31-7: `MIG31R.SEE` Advanced and `MIG31.ECM` Late-Cold-War (opinionated, John, 2026-11-08) |
| What `0x4ad3c0` searches | Whether the fallback to `F18.HUD` also covers other mounts or case | Read the finder, if John wants the original's behaviour as the choice |
| Whether the MiG-31 is player-flyable in retail | It is not on the reviewed player roster, so the fallback may be unreachable | Roster and menu research; not needed to proceed |
| State-word roles | `6120`, `6126`, `6138`, `613e`, `6144` are inferred from slot order and geometry | M31-3 confirmed the flap and rudder words; M31-4 confirmed the gear, brake and plume words against the branch geometry |
| Airbrake position | The geometry is dorsal; the encyclopedia puts the brakes under the intake trunks | Resolved in M31-4: the source's brake word is the raised dorsal panel; the encyclopedia text is not used for the rig |
| Export-only faces | Two indexed-texture wing quads exist only on the export path (decal slots) | M31-2: leave unfilled, per the decal rule |
| Wing span 22% wide | The shape is wider than the real span | Record in M31-2; no change planned |
| No tank row | The record has no `F250.GAS` row | Keep; M31-5 and M31-6 follow the record |
| No AA-6 row | The encyclopedia shows AA-6 on the inner pylons | Keep; follow the record |
| Fuel (-14.4%), thrust (-19.8% and -29.0%) and flow | Range, climb and endurance | Resolved in M31-8: kept as retail and listed for John; the flows stay with the tank |
| Default quantities, pairing, 260-round gun count | Stores load | M31-5 and M31-6 |
| Debris scale and hulk identity | A and C are both damaged bodies | M31-2 |
| Shadow shape | 10 faces, no texture | M31-2 |
| `~SU33_W.PIC` and other `~xx_w` names | Only if a Soviet HUD is chosen | Already resolved in M25-7 (never requested) |

## Mirage 2000 source review (2026-11-09)

Research mode, slice M2K-0. This reviews `M2000.PT` and its dependency closure
before `AircraftId::M2000` exists. `M2000.PT` is the 40th retail identity, to be
appended in M2K-1 (checkpoint code 40, the 41st `SELECTABLE` entry). The Mirage
2000 is a parked target today (`parked_aircraft.rs` lists `M2000.PT` with gear
word `0x589c`); that path stays as it is. `M2000E.PT` (desert) is a separate
record with its own shape `m20e.SH` and is out of scope: no alias and no shared
identity. No original module was executed. PT, SEE, ECM, JT and GAS are read as
bounded BRF data with the shared field order in `aircraft_schema.rs`; SH and HUD
are inspected as data. Slice done by Claude Sonnet (sonnet-medium). Nothing here
changes code or behaviour.

Three questions are open for John and are **not decided here**: the HUD and
cockpit ([below](#mirage-hud-and-cockpit-for-john)), the engine count
([below](#mirage-engine-count-for-john)) and the delta control layout
([below](#mirage-delta-control-layout-for-john)).

### Mirage build and extraction

`FA_1.LIB` and `FA_2.LIB` are the same pair as the MiG-31 review (SHA-256
`657254c5...` and `fb8b3021...`, see the [MiG-31 build
section](#build-and-extraction)). The closure was extracted locally to
`.local/m3-imports/scratch/m2000` with `cargo run --locked -p tore-extract --
--source gameassets/fighters-anthology --out .local/m3-imports/scratch/m2000
--include 'M2000*' --include 'M20*' --include 'RAFALE*' ...` and a name sweep
(`*M20*`, `*MIR*`, `*2000*`, `~RAF*`) to `.../names`. The `SETUP.ESA` installer
copy of `FA_2.LIB` was extracted too. Its `M2000.PT`, `M20*.SH`, `VIS340.SEE`,
`F18R.SEE`, `F18.ECM`, `DEFA.JT`, `F150.GAS`, `AS15.JT`, `R530.JT`, `R550.JT`
and `F.BI` bytes are identical to the root copy; only `RAFALE.HUD` (4 bytes)
and the `_M20*.PIC` skins differ by build. The usual five unrelated archive
errors appear (`LHX*.LIB` compression flags and `_SETUP.LIB`).

### Mirage identity and record

| Field | Decoded value |
| --- | --- |
| PT | `M2000.PT`, 14,316 bytes, SHA-256 `939c96b72012eeb090c73f4d371644b3649033095a5506f24a42689822c1bea9` |
| Displayed names | `M2000`, `Mirage 2000`, `M2000.PT` |
| Schema | OBJECT + NPC + PLANE, 219 statements; `structType` 5; `typeSize` 660; `instanceSize` 626 (the parser default, so M2K-1 adds no size entry) |
| Object class / year / hit points | `0x8000` (fighter) / 1984 / 100; `expType` 30 |
| PLANE flags | `0x11`: seat `0x10` and the base bit `0x01`; hook `0x02`, no-lift `0x08`, carrier `0x40` and multi-crew `0x04` all clear |
| Shape | `m20.SH` (25,088 bytes), shadow `m20_s.SH` (4 faces) |
| Damage looks | `M20_A.SH` 169 faces, `M20_B.SH` 24, `M20_C.SH` 182, `M20_D.SH` 34 |
| Skins | `_M20.PIC` (59,084 bytes), `_M20_A.PIC` 68,184, `_M20_B.PIC` 31,264, `_M20_C.PIC` 59,344, `_M20_D.PIC` 41,404 (all in `FA_2.LIB`) |
| Sounds | `&JET1N.11K` loop, `&JET1A.11K` second, `&powerup.5k`, `&powerdn.5k` (present) |
| HUD | `rafale.HUD` (resolves to the existing `RAFALE.HUD`) |
| AI script | `f.BI` (12,800 bytes, present; the Rafale's too) |
| Sensors / jammer / gun | `VIS340.SEE`, `F18R.SEE`, `F18.ECM`, `DEFA.JT` (all reviewed and live) |
| PTS | none: no `M2000.PTS` exists (the Rafale has `RAFALE.PTS`) |
| Radio callout | `^ACM2000.5K` exists (6,929 bytes) |
| Encyclopedia | `M2000.INF` and `M2000_0/1/2/3/E.PIC` in `disc2/FA_3.LIB`; reference media, not flight data |
| Checkpoint code / CLI key | 40 and `m2000`/`m2000.pt`, reserved for M2K-1 |

The decoded record equals the catalog row in
[`fa-aircraft.csv`](fa-aircraft.csv) field for field (year 1984, class
`0x8000`, 2 engines, 17,100 / 6,346 / 37,480 lb, 17,687 / 32,000 lbf, 1,262 kt
top, 59,000 ft, G -4/+8, hit points 100, 9 hardpoints, the default-store list
and `rafale.HUD`). Resource hashes of the files M2K-1 onward depend on:

| Resource | Bytes | SHA-256 |
| --- | ---: | --- |
| `M2000.PT` | 14,316 | `939c96b72012eeb090c73f4d371644b3649033095a5506f24a42689822c1bea9` |
| `M20.SH` | 25,088 | `78e2a93143c7bb5aeb80729a3c56613589e923aaf9080ec9012efdeb32b0f65f` |
| `M20_S.SH` | 4,608 | `bab9ef07b37be6236168fbbac1b53a9b0643b25af3f5f0e04a8f3f406e97e656` |
| `M20_A.SH` | 16,896 | `b9b91f55c8bb2ca8b148c045b46013780a70d6d9437b50285b6f92ba23ca7829` |
| `M20_B.SH` | 4,608 | `fcd58e3cb8188e9b799bc3fb113039c7c94f4ee784a86bdf6b61e28ba7a6b842` |
| `M20_C.SH` | 16,896 | `26f38ee4be850c22809ba89184e07b5cd82faad602138becad9ae1f5e2d758ca` |
| `M20_D.SH` | 4,608 | `bb0740e2cc9f612777fd0dc1d4a9b78ea2b5b88d2b30e3cccca885731ba1e83e` |
| `RAFALE.HUD` | 4,608 | `b555f9febf6041297393c8d92e9e70343b1c5c9439316b1c0c1ec0721fe39228` |
| `_M20.PIC` | 59,084 | `0f298bf02366c417865a33a0dc1e857c7a56f910ee75efb2a52e1ec600b3c1b2` |

### The record against the Rafale record

The Mirage 2000 record is the `RAFALE.PT` template with its own mass, thrust,
fuel and envelope: **27 of 219 statements differ** (plus the resource names, the
G envelope table and four default-store names), and 192 are identical. Both
`typeSize`/`instanceSize` words, the PLANE flags, hit points (100), empty weight
(17,100 lb), afterburner thrust (32,000 lbf), engines (2), the gun, sensors,
jammer, AI script, loop sound and every station slot are shared. Differences:

| Field | `M2000.PT` | `RAFALE.PT` |
| --- | ---: | ---: |
| `year` | 1984 | 2005 |
| `hudName` | `rafale.HUD` | null (TORE maps `RAFALE.HUD` by convention) |
| Military thrust, lbf | 17,687 | 24,000 |
| Internal fuel / max takeoff, lb | 6,346 / 37,480 | 9,900 / 40,100 |
| `maxAlt`, ft | 59,000 | 65,000 |
| `envMax` | +8 | +9 |
| Fuel flow (`fuelConsumption`) | 2 | 3 |
| Structure `[0]` / `[1]` | 1,161 / 2,129 | 1,278 / 1,946 |
| Signatures 0..3 | 91, 91, 100, 100 | 95, 95, 70, 50 |
| `maxVisDist` / view offset z | 157 / 60 | 172 / 80 |
| Debris (`dmg` x, `dst` x/y/z) | -40; 20, 10, 0 | -60; 0, 20, -40 |
| `_gpullDrag` / `wheelBrakesDrag` | 40 / 102 | 30 / 40 |
| `spinEntry` / `turbulencePercent` | 0 / 122 | 1 / 130 |
| Yaw `acc` / `dacc` | 8 / 8 | 9 / 9 |
| OBJECT `flags` | `0x2bf3` | `0x806bf3` |
| Second sound | `&JET1A.11K` | `&JET6A.11K` |
| Default stores 4..7 | `F150.GAS`, `AS15.JT`, `R530.JT`, `R550.JT` | `AAS38.SEE`, `AGM65G.JT`, `MICA.JT`, `R530.JT` |

Because 17,100 lb and 32,000 lbf match the Rafale's, the empty weight and
afterburner thrust are inherited template values. This is a retail fact, not a
defect to fix; the [record rule](../spec/aircraft-records.md#rule) is met by the
27 differing statements and the Mirage's own shape, sounds, stations and
envelope. The published audit follows. The comparison column is the game's own
encyclopedia text (`M2000.INF`, retail media, numbers only) and Jane's-style
public figures (cited below); they do not establish original game behaviour.

| Field | Record | Published | Difference | Verdict |
| --- | ---: | ---: | ---: | --- |
| Empty weight, lb | 17,100 | 16,534 (7,500 kg, encyclopedia) | +3.4% | Agrees |
| Maximum takeoff, lb | 37,480 | 37,480 (17,000 kg, encyclopedia) | 0.0% | Agrees exactly |
| Military thrust, lbf (all engines) | 17,687 | 14,462 (one M53-P2, 64.3 kN) | +22.3% | Left as retail |
| Afterburner thrust, lbf | 32,000 | 21,385 (95.1 kN) | +49.6% | Left as retail, **over a third** |
| Ceiling, ft | 59,000 | 54,003 (16,460 m service, encyclopedia) | +9.3% | Left |
| Sea-level top speed, kt | 693 | 688 (encyclopedia) | +0.7% | Agrees |
| Internal fuel, lb | 6,346 | about 7,000 (3,978 L, at an assumed 0.8 kg/L) | about -9% | Left, listed |
| Span / length | 22.0 ft / 38.3 ft (shape) | 29.9 ft (9.13 m) / 47.1 ft (14.36 m) | -26% / -19% | Model fact, see shape |

The encyclopedia and MILAVIA / aerospaceweb agree on one SNECMA M53-P2 at 64.3
kN dry and 95.1 kN with afterburner, 9.13 m span and 14.36 m length (MILAVIA
Mirage 2000 specifications, aerospaceweb.org Mirage 2000 data, both accessed
2026-11-09). The afterburner thrust is 49.6% over a single real engine, which
is well over the one-third line the MiG-25 and MiG-31 audits used. Agent audit
decision, 2026-11-09: leave every value as retail and list them for John, the
same rule as the MiG-25 and MiG-31; M2K-8 watches the afterburner climb and
acceleration against the envelope rows. The thrust rows are one more reason the
engine count below is a John decision: the record's thrust is the total for
"2 engines", but the real aircraft has one engine with lower thrust.

### Mirage resources

| Resource | Role | Note |
| --- | --- | --- |
| `M20.SH` | Shape | 206 runtime faces (208 export), 0 lines, 0 billboards, six state words |
| `M20_S.SH` | Shadow | 4 faces |
| `M20_A.SH` / `M20_C.SH` | Damage bodies | 169 / 182 faces |
| `M20_B.SH` / `M20_D.SH` | Damage pieces | 24 / 34 faces |
| `VIS340.SEE`, `F18R.SEE`, `F18.ECM`, `DEFA.JT` | Sensors, jammer, gun | 471, 481, 366, 2,689 bytes; already reviewed |
| `F150.GAS`, `AS15.JT`, `R530.JT`, `R550.JT` | Default stores | 204, 2,748, 2,758, 2,760 bytes; all live in TORE today |

`M20E.SH` and the `M20E_*` looks belong to `M2000E.PT` and are not part of this
identity. The shape streamer has pivot 0 and hinge scale 0 (no streamer).

### Mirage HUD and cockpit (for John)

**Fact.** `M2000.PT` names `rafale.HUD`. The file resolves (case-insensitive) to
the existing `RAFALE.HUD` in `FA_2.LIB`, the same file TORE's Rafale flies. That
HUD carries the panel names `~raf`, `~rafh`, `~rafs`, `~raf_l`, `~raf_c`,
`~raf_r`, `~raf_lh`, `~raf_ch`, `~raf_rh`, `~raf_ls`, `~raf_cs`, `~raf_rs` (the
Rafale cockpit family `~RAF*.PIC` in `FA_1.LIB`, 13 pictures). The Rafale
record itself has a **null** `hudName`, and TORE maps it to `RAFALE.HUD` by
convention (stem `RAF`, cockpit `~RAFH.PIC`). **No Mirage 2000 cockpit art
exists anywhere in the media**: the sweeps `~M20*`, `~M2000*`, `*M20*`, `*MIR*`
and `*2000*` over every archive find only the `M20*` shapes and skins, the
`^ACM2000.5K` callout, the `M2000_*` encyclopedia pictures, the unrelated
`E2000.*` Eurofighter files and one stray `IFMIRA.PIC` (256 x 256, 64 colours,
named by no resource of this aircraft; not examined further). So the retail
Mirage 2000 flies the Rafale cockpit.

Options (none chosen):

1. **Follow the record: `RAFALE.HUD`, cockpit stem `RAF`, `~RAFH.PIC`.** What
   retail does; costs nothing and needs no new art. The identity stays
   `M2000.PT`; sharing a panel is not aliasing (the HUD and cockpit are the
   panel, not the record). The player sees a Rafale cockpit in a Mirage.
2. **Substitute another reviewed HUD family** (for example `F18.HUD` or
   another existing panel). Opinionated, no retail evidence, extra wiring in
   `hud_aperture.rs` and the cockpit stem table.
3. **Author a Mirage panel.** Not in the media; would be invented art, so only
   as a later, separate decision.

Agent note: option 1 is the evidenced one. Each option needs M2K-7 to add one
exhaustive stem arm.

### Mirage engine count (for John)

**Fact.** The record says `engines` = 2 (PLANE field, byte), the same as the
Rafale record and the catalog row. The real Mirage 2000C has **one** SNECMA
M53-P2, and the game's own encyclopedia text says so ("One SNECMA M53-P2
turbofan"). The shape agrees with one engine: a single six-sided nozzle ring
at x 0, y -44 (face `0x3794`), a single centreline flame group (word `0x5890`,
x -3 to 4); there is no second outlet. So the retail
record disagrees with the retail shape and text.

**What the field drives in TORE** (code read, no run):

- **Thrust:** nothing. The flight model reads `thrust` and `aftThrust` as
  totals; the count neither multiplies nor divides them.
- **Wreck and crash:** `tore-sim` `wreck::Power::symmetric` splits the thrust
  over the count and applies a yaw torque from the uneven split (none with one
  engine).
- **Engine damage:** `aircraft_systems::Engine::new(count)`; the two engine-out
  damage codes (9 and 10) each shut one engine and remove `1/count` of the
  power. With 2, each code halves the power; with 1, the first code kills it
  and the second hits the same engine.
- **Contrails and exhaust:** `contrail_offsets` makes one trail per engine; with
  no reviewed nozzle faces for the Mirage, 2 puts the two trails at +/-15% of
  the half span (about 1.65 ft either side of the centreline) while the one
  real nozzle is at x 0. `engine_material::outlet_count` has no Mirage arm yet.
- **AI:** `ai_wings` builds its `Systems::new(config.engines, ...)` from the
  record, and `ordnance.rs` checks the AI default load against the same count.
- **Sound:** nothing; the loop and second sound are named by the PT, not by
  the count.

Options (none chosen):

1. **Keep 2, the retail record.** Matches the catalog, the Rafale precedent and
   the audit rule that record facts stay retail. Visible cost: two contrails
   about 3.3 ft apart from a single nozzle, and half-power engine-out damage.
2. **Treat it as 1 for TORE only** (an opinionated override, John or agent).
   One centreline trail, no wreck yaw, engine-out kills power. Costs a new
   per-identity override: the live path reads the record field directly and I
   found no existing override for a real record (the 1-engine table in
   `aircraft.rs` is the synthetic test path).
3. **Keep the field at 2 for damage and wreck but draw one trail** (visual
   only). Smallest change; leaves a half-power engine-out on a one-engine
   aircraft.

### Mirage delta control layout (for John)

**Fact.** The shape has six state words, listed below with the counts from
`Shape::with_state` (the export path adds two faces to every count, 208 total).
Gear word `0x589c` matches the parked path; the parked inspector reports
contact offset -13 against a gear-down low point of -12 (one unit apart, as the
MiG-25). Shape size at scale 1 (1 unit is 1/3 ft): x -33 to 33, y -44 to 71, z
-6 to 22 (gear down -12), so 22.0 ft across, 38.3 ft long, 9.3 ft high. The
Rafale shape is true size (36 ft by 50 ft) but this one is **26% narrow, 19%
short and about 45% low** against the real aircraft. This is a model fact for
M2K-2 to record, not a defect to fix here.

| Word | Faces | Low point | Geometry (shape units, x right, y forward, z up) | Reading | Status |
| --- | ---: | ---: | --- | --- | --- |
| `0x5890` | +8 | -4 | Four flat plates behind the nozzle, y -44 to -80, crossed planes (x 0 and z 0) | Afterburner flame | Confirmed by slot order and position |
| `0x5896` | +2 | +5 | One quad at x +/-1, from y 17, z 10 down to y 24, z 5 (two windings) | Dorsal airbrake: a single 2-unit-wide centreline plate | Confirmed as the source's brake word; geometry unusual (see below) |
| `0x589c` | +6 | -12 | Three flat plates: mains at x -7 and x 7 (y -18 to -12, z -12 to -3), nose at x 0 (y 32 to 37, z -12 to -5) | Landing gear down | Confirmed; the parked gear word |
| `0x58a8` | -2 | none | Removes the left inboard trailing-edge strip (x -18 to -3, y -32 to -29) | Left flap word | Slot order only |
| `0x58ae` | -2 | none | Removes the right inboard strip (x 3 to 17) | Right flap word | Slot order only |
| `0x58b4` | 0 net | +4 | Swaps the rudder quad for a copy offset 1 to 2 units in x | Rudder deflection (positive direction only) | Confirmed by geometry |

The slot order (flame, brake, gear, flap, flap, sixth) is the shared base order
of the MiG-25, MiG-31, F-18 and Rafale families, with the rudder as the sixth
word as on the MiG-31. No hook word exists (the hook flag is clear). **There is
no slat word, no canopy word and no separate slat or canopy geometry**, and no
nozzle word: the source has none to rig, and I recommend none be invented.

**State map for the rig** (elevons, rudder, airbrake, gear, burner, slats,
canopy):

| Moving part | Source panel | State word | Rig note |
| --- | --- | --- | --- |
| Elevon, inboard, each side | 1-ft-chord trailing strip, 2 quads (both layers) | `58a8`/`58ae` remove it, nothing replaces it | The rig keeps the strip drawn and turns it |
| Elevon, outboard, each side | Same strip further out, 4 triangles (2 per layer) | none | Same hinge as inboard, same angle |
| Rudder | 2-face quad on the single fin | `58b4` | Hinge on the fin trailing edge |
| Airbrake | 2-face quad on the spine | `5896` | Appears only when open |
| Gear | 3 plates (6 faces), no doors, no wheels | `589c` | Rigid plates, hinge at top edge |
| Burner | 4 plates (8 faces) | `5890` | Additive plume, no hinge |
| Slats, canopy | none | none | Not in the source |

**Rule 11 face table (from the `state_neutral` source dump).** The wing trailing
edge is the straight line y = -29 from the root (x +/-3) to the tip (x +/-33).
Each elevon is a thin wedge: its hinge vertices sit at y = -29 on the top layer
(z = -2) and the bottom layer (z = -4), and its trailing edge at y = -32, z =
-3. The mid axis y = -29, z = -3 therefore runs through the whole span and is
the single straight line each wedge can turn about; the hinge vertices are
1 unit above or below it.

| Surface | Face | Vertices | Moved | On hinge line (y = -29) | Fixed |
| --- | --- | ---: | ---: | ---: | ---: |
| Right inboard (x 3 to 17) | `3969` | 4 | 2 | 2 | 0 |
| Right inboard | `394a` | 4 | 2 | 2 | 0 |
| Right outboard (x 17 to 33) | `2176` | 3 | 2 | 1 | 0 |
| Right outboard | `2391` | 3 | 1 | 2 | 0 |
| Right outboard | `2190` | 3 | 2 | 1 | 0 |
| Right outboard | `2233` | 3 | 1 | 2 | 0 |
| Left inboard (x -18 to -3) | `3a31` | 4 | 2 | 2 | 0 |
| Left inboard | `3a50` | 4 | 2 | 2 | 0 |
| Left outboard (x -33 to -18) | `1e87` | 3 | 2 | 1 | 0 |
| Left outboard | `23c3` | 3 | 1 | 2 | 0 |
| Left outboard | `1ea1` | 3 | 2 | 1 | 0 |
| Left outboard | `1f1b` | 3 | 1 | 2 | 0 |
| Rudder | `3bc6` | 4 | 2 | 2 | 0 |
| Rudder | `3be5` | 4 | 2 | 2 | 0 |
| Gear (each of 6 faces) | `380b` ... `3900` | 4 | 2 | 2 | 0 |
| Airbrake (each of 2 faces) | `3c73`, `3c92` | 4 | 2 | 2 | 0 |

Verdicts:

- **Every elevon is a whole panel in the source.** No elevon is split across a
  diagonal or a threshold, and no face has a fixed vertex off the hinge line. The
  inboard strip is two quads (top and bottom layer); the outboard strip is the
  quad on each layer cut into two triangles (the top layer on the diagonal from
  the inboard hinge corner to the outboard trailing corner, the bottom layer on
  the other diagonal). Every neighbouring wing face shares only hinge-line
  vertices with the strips, apart from the inboard/outboard seam at x = 17 (left
  -18), where both strips share their trailing vertex.
- **No diagonal or partial surface was found**, so no probe sheet is needed.
  The things for John or M2K-3 to know (none decided): (a) the chord is only 3
  units (1 ft), so even a 25 degree turn moves the trailing edge about 1.3 units
  (0.4 ft); the elevons will read as thin; (b) the hinge vertices are 1 unit off
  the mid axis, so a turn about the mid axis opens or closes a gap of up to
  `sin(angle)` units against the wing faces (0.17 units at 10 degrees), the
  same wedge issue as the other thin trailing-edge strips; (c) the inboard
  strip is 14 units on the right and 15 on the left (the seam is x 17 against
  x -18), so the two sides are not exact mirrors; (d) inboard and outboard share
  the seam vertex, so they must take the same angle (rule 12 says both take
  pitch and roll) or the seam shows; (e) the flap words delete the inboard
  strip with no replacement, so the rig must not hide it.
- **Rudder:** one hinged quad (two windings). Two vertices lie on the leading
  edge from (0, -35, 4) to (0, -40, 19) shared with the fixed fin faces `2682`
  and `275b`; the other two move. The trailing vertex (0, -42, 19) is also
  shared with the fixed triangles `2b64` and `2b7c` above it, which would
  leave a one-unit crack as the retail swap itself does. The retail swap
  corresponds to roughly 40 degrees, rounded to whole units, so it is not an
  exact rotation.
- **Airbrake:** the source panel is a single plate 2 units wide, not a pair of
  side panels; the real aircraft has paired airbrakes. It is recorded as the
  source's brake, hinged on its aft edge (y 24, z 5).
- **Gear:** three flat plates, mains 6 units fore-aft, no doors or wheels.
  The wheelbase is 50 units (16.7 ft) and the track 14 units (4.7 ft).

The whole-surface gate (John, 2026-11-07 and 2026-11-08) is met by every
elevon, the rudder, the brake and the gear. The delta has no horizontal tail,
so M2K-3 will drive each elevon with a pitch term plus a differential roll term
per rule 12 and the face table above.

### Mirage hardpoints

All nine source rows, in original order. Positions are source units; feet divide
by three; x right, y up, z forward ([aircraft
ordnance](aircraft-ordnance.md#hardpoint-positions)). Location byte: 0
Centerline, 1 Fuselage, 2 Internal Gun, 3 Internal Bay, 4 Wing, 5 Wingtip.

| Row | Location | Flags | Position x/y/z | Feet right / up / fwd | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `VIS340.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `F18R.SEE` | Fixed radar |
| 2 | Fuselage | `0x0008` | 0 / 0 / -50 | 0 / 0 / -16.7 | 1 | 0 | `F18.ECM` | Fixed jammer |
| 3 | Internal Gun | `0x000c` | 3 / -5 / 45 | 1 / -1.7 / 15 | 250 | 0 | `DEFA.JT` | Internal gun, 250 rounds |
| 4 | Centerline | `0x0605` | 0 / -15 / 0 | 0 / -5 / 0 | 1 | 13 | `F150.GAS` | 150 gal tank |
| 5 | Fuselage | `0x07e5` | 4 / -20 / 0 | 1.3 / -6.7 / 0 | 4 | 46 | `AS15.JT` | A2G, belly pair |
| 6 | Wing | `0x07e5` | 11 / -20 / -10 | 3.7 / -6.7 / -3.3 | 2 | 20 | `R530.JT` | A2A, inner wing pair |
| 7 | Wing | `0x07e5` | 20 / -20 / -20 | 6.7 / -6.7 / -6.7 | 2 | 20 | `R550.JT` | A2A, outer wing pair |
| 8 | Wingtip | `0x0085` | 33 / -4 / -29 | 11 / -1.3 / -9.7 | 2 | 4 | `R550.JT` | A2A, wingtip pair |

Default stores in the catalog (`F150.GAS`, `AS15.JT`, `R530.JT`, `R550.JT` x2)
count one default per row; the per-row quantities are a host rule for M2K-6.
The rows are the Rafale's rows with a different pylon layout (the Rafale has
`AAS38.SEE`, `AGM65G.JT`, `MICA.JT`, `R530.JT`, `R550.JT`, its wingtip row at
x 59 and its fuselage row at x 10). Things M2K-5 must handle:

- The Wingtip row (x 33, z -29) sits **on the shape's wing tip trailing
  corner**: the shape tip is x 33, y -29, so this row matches the geometry.
- The Centerline tank (row 4) hangs 5 ft below the centreline between two
  Fuselage rows at x +/-4 (+/-1.3 ft, 2.7 ft apart, flag `0x4` set so they
  mirror). A 150 gal tank and two AS-15 on the standard pylon rules are likely
  to overlap; this is the delta analogue of the MiG-31 fin clipping and the
  expected stop-and-report case with frames.
- The Wing rows at x 11 and 20 and the Wingtip at 33 lie within the 33-unit
  semi-span; M2K-5 measures them against the wing under the standard pylon rules.
- The gun is a fixed internal row (250 rounds, 1 ft starboard, 15 ft forward).
- There is no sensor or designator pod row: never hang one (John, 2026-11-07).

### Mirage roster note

`M2000.PT` appends last: checkpoint code 40 (`Mig31` is 39), `ALL` becomes 40
and `SELECTABLE` the 41st entry. With codes 0 to 40 in use, **23 of the 6-bit
(64) wire identity slots remain**; the identity-protocol decision stays
John's. `M2000E.PT` is not added.

### Mirage unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| HUD and cockpit choice | M2K-7 cannot start without it | John, three options above |
| Engine count 2 or 1 | Wreck, damage, contrails, AI | John, three options above |
| Elevon thinness and the hinge gap | Visual size of a 1 ft chord | M2K-3 measures with the 25-pose grid |
| What the original does with `58a8`/`58ae` | They delete the inboard strip | Unknown; the rig keeps the strip drawn |
| Airbrake as one plate | Real aircraft has paired brakes | M2K-4 rigs the source panel only |
| Shape 26% narrow, 19% short | Scale of stores and pylons | M2K-2 records; no change planned |
| Afterburner thrust 49.6% over real | Climb and acceleration | M2K-8 against the envelope rows |
| Centerline tank under two belly rows | Clipping | M2K-5 stop-and-report with frames |
| Per-row default quantities | Stores load | M2K-6 |
| Radar and jammer | `F18R.SEE` and `F18.ECM` | Already reviewed; no choice needed |
