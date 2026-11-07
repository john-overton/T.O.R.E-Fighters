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
| `~SU33_W.PIC` | The HUD names it but no archive entry exists; the existing shared cockpit may treat it as optional | Check the MiG-29, Su-25 and MiG-23 cockpit resolution in M25-7 and record the absence if it is optional |
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
