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

## Mirage 2000 identity and record import (M2K-1, 2026-11-09)

Implementation, slice M2K-1, Claude Sonnet (sonnet-medium). `AircraftId::M2000`
joins the end of `AircraftId`, `ALL` (40 retail identities) and `SELECTABLE`
(41 entries) with checkpoint code 40 and the parse keys `m2000` and `m2000.pt`.
It is a TORE addition on this branch: the parked path
`("M2000.PT", Some(0x589c))` in `parked_aircraft.rs` is unchanged, and
`M2000E.PT` and the Rafale are not aliased.

| Item | Value | Provenance |
| --- | --- | --- |
| Record | `M2000.PT`, `typeSize` 660, no PTS | spec-derived (retail record, M2K-0 review) |
| Shape stem | `M20` (`m20.SH`, `_M20.PIC`, `M20_A..D.SH`) | spec-derived |
| Display | short `M2000`, long `Mirage 2000` | spec-derived |
| Radar / gun | `F18R.SEE`, `DEFA.JT` | spec-derived; both already reviewed |
| HUD and cockpit | `RAFALE.HUD`, stem `RAF`, `~RAFH.PIC`, bound by name | opinionated (John, 2026-11-09, decision 1); no Rafale alias |
| Live-fire list | `DEFA.JT`, `R530.JT`, `R550.JT`, `AS15.JT` | spec-derived from the default stores |
| Strike class | Fighter | agent decision, 2026-11-09 |
| Interim rig | CODE 18,624 bytes, 206 faces, words `5890 5896 589c 58a8 58ae 58b4`, gear word `589c` adds 6 faces | spec-derived; other words unmapped until M2K-3/M2K-4 |
| Gear low point | 12 shape units at 1/3 ft | spec-derived (M2K-0 gear table) |
| Engines and thrust | 1 engine, 14,455 / 21,380 lbf | **opinionated (John, 2026-11-09, decision 2)**, see below |

**Engine override.** The retail record reads 2 engines with 17,687 / 32,000
lbf, which the [M2K-0 audit](#mirage-engine-count-for-john) found to be the
Rafale template's numbers (afterburner 49.6% over one real M53-P2). On John's
decision of 2026-11-09 TORE runs the Mirage 2000 with one engine and the thrust
of one SNECMA M53-P2: 64.3 kN dry = 14,455 lbf and 95.1 kN with afterburner =
21,380 lbf (MILAVIA and aerospaceweb.org Mirage 2000 data, both accessed
2026-11-09, cited in the review above). It is applied in one place, the record
corrections table (`aircraft_corrections.rs`, run by `Aircraft::parse`), so the
sim, the app, the AI and the checkpoint all see one engine; the catalog row and
the decoder keep the retail values, which stay documented as retail in the
review above. See [the player-facing note](../spec/aircraft-records.md#mirage-2000-one-engine-one-m53-p2-2026-11-09).
Empty weight (17,100 lb) and internal fuel (6,346 lb) stay retail. M2K-4 draws
one trail and one nozzle glow from the count; M2K-8 validates climb and
acceleration with the lower thrust.

The ordnance audit pins moved to 40 aircraft as a first measurement of a record
not yet reviewed station by station; see the [ordnance availability
baseline](../baselines/ordnance-availability.md#the-mirage-2000-joins-the-roster-m2k-1-2026-11-09).
Wire identity slots: codes 0 to 40 are in use, 23 of the 64 remain.

## Mirage 2000 shape, real size and skin (M2K-2, 2026-11-09)

Implementation, slice M2K-2, John's decision 3 of 2026-11-09 ("Per axis to
real"). `m20.SH` is scaled per axis to the real Mirage 2000: factors
`[1.3616, 1.2290, 1.5053]` (x right, y forward, z up), fitted from the measured
shape extents to 9.13 m span, 14.36 m length and 5.20 m height. The table, the
fit, the order with animation and the list of consumers are in [aircraft shape
scale](objects-and-shapes.md#aircraft-shape-scale-2026-11-09). The [skin
review](objects-and-shapes.md#mirage-2000-skin-review) (double-sided panels,
damage looks, the decal slots) is in the objects and shapes guide; the
`render-m2000-skin` battery scenario captures the top and under views with the
gear down. Provenance: choice opinionated
(John, 2026-11-09), factors fitted. Later slices use the scaled geometry: M2K-3
and M2K-4 scale any shape-unit hinge point their rigs name, M2K-5 measures the
stations on it.

## Mirage 2000 stations measured (M2K-5, 2026-11-10)

Slice M2K-5, Claude Sonnet (sonnet-high). The nine decoded rows were measured on
the per-axis scaled shape under the standard pylon rules; the table, the
clipping (wingtip R550 1.21 ft, belly AS-15 0.56 to 0.75 ft, centreline tank
0.09 ft), the clearances to the gear and the ground, the alternatives and the
options for John are in [stores on pylons](../spec/stores-on-pylons.md#the-mirage-2000-m2k-5-2026-11-10).
The M2K-0 expectation that the tank would clash with the two belly rows did not
hold (the tank is 1.58 ft wide, the AS-15's inboard edge clears it by 0.23 ft).
No station table was adopted and no placement count moved, so the ordnance audit
pins are unchanged. Open until John decides the two clipping cases.

John's decisions on the M2K-5 stop (2026-11-10, opinionated): the belly pairs
(row 5) take the MiG-31's 0.1 ft rail drop with the decoded positions kept
(residual AS-15 entry 0.46 to 0.65 ft), no 350 or 500 gallon tank on the belly
pairs (tank pin 201 to 199), and the wingtip R550 (row 8) moves to the nearest
real station. That station is row 7's own outboard wing pylon, so row 8 stayed
where it was and the slice stopped again with measured options; see [the
decisions](../spec/stores-on-pylons.md#the-mirage-2000-decisions-m2k-5-2026-11-10).

M2K-5, later on 2026-11-10 (opinionated, John): TORE carries no store on the
Mirage 2000's wingtip row 8, which is the Rafale template's wingtip rail (the real
aircraft has nine stations and no tip rail); the retail row above stays decoded
and documented as retail, and TORE's parsed record drops it
(`aircraft_stations::retire`). This supersedes the row 8 option left open in the
previous note; see [the decision](../spec/stores-on-pylons.md#no-mirage-2000-wingtip-station-m2k-5-2026-11-10-later-the-same-day).

## Mirage 2000 ordnance, default load and countermeasures (M2K-6, 2026-11-10)

Slice M2K-6 confirmed the Mirage 2000's ordnance as the record's own, with no
row, mask, capacity or default authored: the DEFA gun at 250 rounds (the real
2000C's two guns of 125 rounds make the same 250 together), `F150.GAS` on the
centreline, four `AS15.JT` on the belly row, two `R530.JT` and two `R550.JT` on
the wing rows, and 30 chaff and 30 flares from `F18.ECM` (the fleet's
convention, the ECM record is the capacity). Live fire, release positions,
jettison and the audit are in [aircraft
ordnance](aircraft-ordnance.md#the-mirage-2000-m2k-6-2026-11-10) and the
[ordnance availability baseline](../baselines/ordnance-availability.md#mirage-2000-ordnance-m2k-6-2026-11-10).
The earlier "per-row quantities are a host rule for M2K-6" sentence above is
answered there: the rows' own capacities (250 rounds, 1 tank, 4, 2 and 2) are the
default quantities.

## Mirage 2000 sensors, HUD and cockpit (M2K-7, 2026-11-10)

Slice M2K-7 confirmed the Mirage 2000 on the shared sensor records and the
Rafale HUD and cockpit family. No game code changed: the bindings were wired in
M2K-1 and every record was already reviewed. Added: a retail-reading test and
documentation.

**Sensors (spec-derived, retail records, already reviewed).** `--sensor-summary`
reads: `M2000: radar F18R.SEE search 90 nmi 60x60 deg track 50 nmi 45x45 deg
look-down 0 ADVANCED; infrared none; visual VIS340.SEE search 10 nmi 170x140 deg
track 5 nmi 170x140 deg; jammer F18.ECM LATE strength 0.30; signature radar 100
infrared 100`. That is the F/A-18D's line, record for record: `F18R.SEE` is
`Preset::Advanced`, `F18.ECM` is `Generation::LateColdWar`, and no
`sensors/profile.rs` arm was needed (the MiG-31 needed two because its records
were unreviewed). The infrared channel is absent because the record carries no
`.SEE` infrared row (the Rafale's `AAS38.SEE` is a Rafale-only row). The
signature numbers (radar 100, infrared 100) are the Mirage record's own fields
and differ from the Rafale record's 50 and 70; they are read, not changed.
Radar, the RWR and the jammer have no per-aircraft code: nothing under
`crates/tore-sim/src/sensors` branches on the aircraft identity. In a live-fire
run with the radar window open (`--live-fire --aircraft m2000 --weapon-slot 3`)
the RADAR window reads `TWS 10NM 1 CONTACTS TRACK`, the Super 530D locks, flies,
hits and kills, and the jammer key lights `JAM` in the RWR window.

**HUD and cockpit (opinionated, John, 2026-11-09, decision 1).** Follow the
record: `AircraftId::M2000.hud()` is `RAFALE.HUD`, the cockpit stem is `RAF`,
the forward art is `~RAFH.PIC` and the instrument window frame is `~RAF_P.PIC`
(`Hud::parse` of `RAFALE.HUD` names `~raf_p`, which `Airframe::load` requires of
every flyable identity). The Rafale record is not aliased: `M2000.PT` stays the
identity (test pinned in
`crates/tore-import/tests/m2000_cockpit.rs`, retail-reading, skips without
media). Overlays `~RAF_LH`, `~RAF_CH`, `~RAF_RH` exist. No `~M20*` cockpit
resource exists in either archive.

`--cockpit-sheet OUT_DIR M2000,RAFALE,F18` reported `m2000: stem M20 cockpit
~RAFH.PIC art 1280x490 FITTED CLR`, centre mirror x 461 y 24 w 360 h 79, left x
140 y 242 w 118 h 248, right x 1024 y 242 w 118 h 248, identical to the Rafale
line. The HUD aperture mask arm `RAF` (M2K-1, `hud_aperture.rs`) draws the HUD
behind the Rafale combiner glass; the forward capture shows the HUD inside the
glass with no clipping.

**Views (windowed, `--flight-view`, `--flight-look`).** Forward cockpit,
look left and right at 45 degrees (the cockpit art turns away with the look, the
HUD glass and canopy bow at the edge of frame), 90 degrees left and right and 60
degrees up (no cockpit art, open sky, the same as the Rafale at 90 degrees), F3 Up and F2 Back all draw as for the Rafale. F2 Back shows the
Mirage's spine, fin and wing roots from the pilot's eye. The four instrument
windows or six (small layout) keep their positions. The standalone pages 0 to 9
(RCS, Envelope, Front View, Other View, Target Cam, RWR, Nav Info, Systems,
Weapons, Radar) render with the Rafale bezel.

**Instruments and the single engine (decision 2).** The Systems window shows
`THR`, `TEMP`, `OIL`, `HYD` and `FUEL` / `(+ EXT)`; it has one engine readout
for every aircraft and never one per engine, so a one-engine Mirage on the
twin-engine Rafale panel shows no second engine needle or readout (the panel
art draws none; the live readouts are generated text). The fleet's twin-engine
aircraft share the same single readout (it reads the engine system's
temperature and the throttle). Fuel reads the Mirage's own record: `FUEL 6346
LBS` (the Rafale reads 9,900), and `(+ EXT 990 LBS)` with the `F150.GAS`
centreline tank, 989 LBS after the live-fire run's first seconds. The Weapons
window lists `250 DEFA`, `4 AS 15`, `2 S530D`, `2 R-550` and `30 CHAFF` /
`30 FLARE`. Throttle shows 70 percent at the free-flight start, as for the
other aircraft. HUD airspeed and altitude boxes read the Mirage's own state
(493 kt and 5,000 ft at the live-fire start).

**Eye point (agent decision, 2026-11-10, left unchanged).** `mirrors::pilot_eye`
is one fixed offset for every aircraft: 7 ft above and 10 ft ahead of the model
origin, used only by the F2 Back view, the rear-view mirrors and the replay's
back and cockpit views. The forward cockpit view looks from the model origin
and draws the 2D cockpit picture, so it does not depend on the eye point. There
is no per-aircraft eye mechanism: `pilot_eye(position, body)` has no aircraft
parameter and is called from the flight views, the mirror camera, the aircraft
camera and four replay viewer sites, most of which do not hold an aircraft
identity. Measured on the scaled shape (neutral dump, the topmost faces within
4 units of the centreline): the Mirage's canopy top is 7 units, 3.5 ft above
the origin, between 9.8 and 18.0 ft ahead of it, so the fixed eye sits 3.5 ft
above the canopy and at its aft end. The Rafale's unscaled canopy top is 9
units, 3.0 ft, between 8 and 16 ft ahead, so the Mirage eye stands in the same
place relative to its canopy as the Rafale's. The F2 frames match. Not changed.
If John wants a true seated eye (inside the canopy) it would be a fleet change:
one eye table keyed by aircraft, threaded through those call sites.

Evidence and frames: `.local/m3-aircraft/frames/M2K-7/`. Not run: an RWR with an
emitter or missile plotted (see below).

**RWR with a threat.** The live-fire fixture's target emits no radar and the
diagnostic incoming round plots at the centre, so no scripted windowed run puts
an emitter on the Mirage's RWR (the existing note that the RWR reaction has no
headless or scripted path stands, [lane-flight](../testing/lane-flight.md)). The
RWR window is aircraft-agnostic code (`rwr_readout`), covered by the shared unit
tests. The frames show the window with the jammer lit and with the incoming
round at the centre.

## Mirage 2000 flight review (M2K-8, 2026-11-10)

Dated section, additions only. The Mirage 2000 flies the hybrid adapter's own
`variety` configuration built from `M2000.PT` (`M20.SH`) with the one-engine
override of John's decision 2 (2026-11-09): 14,455 lbf military and 21,380 lbf
afterburner, one engine. Retail stays as recorded in the M2K-0 and M2K-1 sections
(two engines, 17,687 and 32,000 lbf). No envelope, G, weight, fuel or flow value
was changed and no flight constant was touched.

- Top speeds: the 1 G row decodes to 1,262 kt at 36,000 ft against a published
  Mach 2.2 (1,261 kn) and 693 kt at sea level against a published 600 to 729 kn,
  so `fitted_envelopes` leaves the record untouched. Full internal fuel settles
  at 0.948 of the edge (1,197 kt, Mach 2.09 at 36,000 ft; 670 kt at 1,000 ft).
- Climb and acceleration at the real thrust: peak specific excess power 418 to
  435 ft/s (25,100 to 26,100 ft/min) at afterburner, 213 to 240 ft/s at military
  power, a best sustained climb of 454 ft/s (27,200 ft/min), 36,000 ft in 110 to
  129 s. The Rafale record's 32,000 lbf at the same weight gives 699 ft/s and
  609 to 615 ft/s. The published 56,100 ft/min is out of reach by about a factor
  of two; the cause is the shared drag law (drag equals the reference thrust at
  the top speed) and the lack of ram thrust, not the thrust or the weight. For
  John, if he wants the published climb: a per-aircraft drag or ram fit is a
  fleet-level model change; raising the thrust would need about 2.2 times the
  real M53-P2 and contradicts decision 2.
- G limits: the record's -4 and +8 rows give +7.1 and -3.5 G on a clean aircraft
  (+6.6 and -3.3 with the default stores) through the shared loaded-elevator
  rule; the real limits are quoted at +9 and -3.2 G.
- Fuel: record flows 2 and 16 lb/s kept; the 6,346 lb tank gives 52.9 minutes
  dry and 6.6 minutes at afterburner, against about 29 and 8.5 minutes at the
  M53-P2's published specific fuel consumption. Empty weight 17,100 lb (real
  16,535 lb) and the tank (real about 6,970 lb) stay retail; the two errors almost
  cancel at full fuel (within 60 lb).
- Takeoff and landing: liftoff 130.3 kt after 1,026 ft, approach 145.2 kt at 65
  percent fuel (published 140 kt), touchdown 132.6 kt, stopped; the record has no
  hook.

The liftoff (125 to 150 kt) and approach (133 to 147 kt) ranges in the flight
battery are fitted windows, agent decisions of 2026-11-10; the approach range is
the published 140 kt plus or minus 5 percent. Full tables, sources and the
commands are in the
[variety flight baseline](../baselines/variety-flight.md#mirage-2000-validation-2026-11-10).
The M2K-0 unknowns row for fuel, thrust and flow is resolved by this section:
kept as retail and listed for John.

## Mirage 2000 in missions, multiplayer and replay (M2K-9, 2026-11-10)

Implementation and verification, slice M2K-9 of the Mirage 2000 port. No game
code changed: `AircraftId::M2000` was appended to every identity list in M2K-1
(checkpoint code 40, `SELECTABLE` index 40, `ALL` 40 entries), and the wire
codes an identity by its 6-bit `SELECTABLE` position, so 23 of the 64 slots
remain and no golden, replay or network fixture moved. The AI builds a wing
member from the record like any aircraft: `Systems::new(config.engines, ...)`
now sees the one engine of decision 2, and the default AI load check in
`ordnance.rs` (external fuel and engine count against the load's configuration)
passes for it, so an AI Mirage carries the record's `F150.GAS`, `AS15.JT`,
`R530.JT` and `R550.JT` load. `strike_class.rs` already lists it in the
fighter family (M2K-1), so no AI behaviour work was done.

What changed is coverage: a Quick Mission test
(`the_m2000_is_a_quick_mission_player_and_a_wing_choice`) naming `M2000.PT` as
the player's aircraft and in every AI wing field, the
`ai-m2000-quick-mission-2v2` and `replay-rec-m2000-2v2` battery scenarios, the
Mirage 2000 row in the Load Ordnance notes loop
(`menus-snap-quick-lobby-ordnance-laser-notes-m2000`: the lobby page warns about
a laser-guided store and offers the pod helper, the same as the F-16A and the
MiG-31, because its decoded wing mask admits both), and `ai-m2000-*` in the
`ai-fights` battery family.

Results (2026-11-10, throttled, profile `.local/m3-aircraft/profile`):

- `--ai-roster-probe-ticks 3600`: the four `M2000.PT` lines (Novice, Average,
  Experienced, Ace) each `ticks=3600 models=2 radar=true ir=false stores=4
  gun=true projectiles=0 dropped=0 PASS` (four source rows: the DEFA gun, the
  AS-15 row, the Super 530D row and the Magic 2 row; no wingtip row), so the AI
  flies it finite with an exact input replay at every skill.
- `ai-m2000-quick-mission-2v2` passes: every wing the Mirage 2000, one missile
  kill by the player, two kills in all, nothing dropped. The `cap`, `intercept`,
  `escort` and `hold` presets of the same fight ran headless with no warnings.
- `replay-rec-m2000-2v2` passes: the recording reads back `player.aircraft =
  M2000.PT`, `probe.enemy = M2000.PT`, `draw.models = M2000.PT`, two runs are
  identical and `--verify-render` finds nothing missing.
- `net-content-missing` and `net-window-lobby` pass with the rebuilt
  `tore-server` (the one built on 2026-11-08 had a 207 item content list
  against the Mirage build's 208, so a bot joined under a different
  compatibility code; the battery needs `cargo build --locked -p tore-server`
  after any change to the identity list).

The parked Mirage 2000 path is unchanged and checked: `--surface-parked FRA
QFLAIR` and `FRA QFSAIR` list `M2000.PT` with `gear 0x589c`, `scale 0.3333` and
`origin-up 6.02` (the scaled gear low point), and `--surface-scene` draws it
gear down on the apron at the same size as the flown aircraft (see
[parked aircraft](../spec/surface-defenses.md#parked-aircraft)).

## Wire identity widened to 9 bits (NET-ID9, 2026-11-10)

John's decision, 2026-11-10: the wire codes an aircraft identity in 9 bits
(512 identities) instead of 6 (64), which leaves room for the base game's 145
aircraft records about twice over, for mods. This replaces the "23 of the 64
slots remain" counts in the dated Mirage sections above: with codes 0 to 40 in
use, 471 of the 512 identity slots remain (the readout reserves a code for
"none", so the roster may reach 511 entries). Protocol 83; one constant,
`wire::AIRCRAFT_BITS`, codes every aircraft field, and a compile-time
assertion fails the build when `AircraftId::SELECTABLE` outgrows it. See
[the protocol](net-protocol.md) for the field list.

## F-15E source review (2026-11-10)

Research mode, slice F15E-0. This reviews `F15E.PT` and its dependency closure
before `AircraftId::F15E` exists (checkpoint code 41, reserved for B5-1). No
original module was executed. PT, SEE, ECM, JT and GAS are read as bounded BRF
data with the shared field order in `aircraft_schema.rs`; SH, HUD and INF are
inspected as data. Slice done by Claude Sonnet (sonnet-medium). Nothing here
changes code or behaviour. `F15.PT` (the F-15C, `AircraftId::F15`) and
`F15J.PT` are separate records: no alias and no shared identity.

Open for John, **not decided here**: the record template values
([below](#f-15e-record-against-the-f-15c-and-the-real-aircraft)), the HUD and
cockpit ([below](#f-15e-hud-and-cockpit-for-john)), the pod rows
([below](#f-15e-pods-for-john)), the rudders
([below](#f-15e-surfaces-for-john)) and the real-size scaling
([below](#f-15e-size-for-john)).

### F-15E build and extraction

`FA_1.LIB` and `FA_2.LIB` are the same pair as the MiG-31 review (see the
[MiG-31 build section](#build-and-extraction)). The closure was extracted to
`.local/m3-imports/scratch/f15e` with `cargo run --locked -p tore-extract --
--source gameassets/fighters-anthology --out .local/m3-imports/scratch/f15e
--include 'F15E.PT' --include 'F15.PT' --include 'f15e*' --include 'F15*'
--include '*.HUD'`, and a name sweep (`~F15*`, `*F15*`, `*F-15*`, `*STRIKE*`,
`*EAGLE*`, `*LANT*`, `~AV8*`, `AV8*`, the pod, store, sound and AI names) to
`.../names`. The `SETUP.ESA` installer copy of `FA_2.LIB` holds the same
`F15E.PT`, `F15E.SH`, `F15E_S.SH`, `F15E_A.SH` and `F15E_C.SH` bytes (compared);
the usual five unrelated archive errors appear.

### F-15E identity and record

| Field | Decoded value |
| --- | --- |
| PT | `F15E.PT`, 14,746 bytes, SHA-256 `6c92d010665d8e23da21269783752568e30571660dadcec39688ba56e1e76989` |
| Displayed names | `F-15E`, `F- 15E Strike Eagle`, `F15E.PT` |
| Schema | OBJECT + NPC + PLANE, 219 statements; `structType` 5; `typeSize` 660; `instanceSize` 626 (the F-15C's are 588 / 575; 660 is the parser's usual size for nine hardpoints, so B5-1 adds a size entry) |
| Class / year / hit points | `0x8000` (fighter) / 1990 / 129; `expType` 30 |
| PLANE flags | `0x15`: seat `0x10`, multi-crew `0x04`, base `0x01`; hook, no-lift and carrier clear (the F-15C is `0x11`) |
| Shape | `f15e.SH` (33,280 bytes), shadow `f15e_s.SH` (16 faces) |
| Damage looks | `F15E_A.SH` 311 faces, `F15E_B.SH` 138, `F15E_C.SH` 350, `F15E_D.SH` 33 |
| Skins | `_F15E.PIC` (167,764 bytes), `_F15E_A.PIC` 100,424, `_F15E_B.PIC` 52,844, `_F15E_C.PIC` 133,444, `_F15E_D.PIC` 52,064 (all in `FA_2.LIB`) |
| Sounds | `&JET1N.11K` loop, `&JET1A.11K` second, `&powerup.5k`, `&powerdn.5k` (present; the F-15C's too) |
| HUD | `av8.HUD` (resolves to the existing `AV8.HUD`) |
| AI script | `f.BI` (12,800 bytes, present) |
| Sensors / jammer / gun | `VIS340.SEE`, `F15R.SEE`, `F15.ECM`, `AV8IR.SEE`, `PAVESPK.SEE`, `M61.JT` |
| PTS | none: no `F15E.PTS` (nor `F15.PTS`) |
| Radio callout | none: `^ACF15.5K` and `^ACF15J.5K` exist, no `^ACF15E.5K` |
| Encyclopedia | `F15E.INF` only (4,567 bytes, text, `disc2/FA_3.LIB`); no `F15E_*.PIC` pictures |
| Checkpoint code / CLI key | 41 and `f15e`/`f15e.pt`, reserved for B5-1 |

The decoded record equals the catalog row in [`fa-aircraft.csv`](fa-aircraft.csv)
field for field. Hashes of the files later slices depend on (first 16 hex
digits, full values are in the scratch folder): `F15E.SH` `d67183064e083578`,
`F15E_S.SH` `c83286459c491f2c`, `F15E_A.SH` `05aea1207c78d240`, `F15E_B.SH`
`60a3ed7292268d2c`, `F15E_C.SH` `e921ecd8a47b8002`, `F15E_D.SH`
`aef081cf423ef054`, `AV8.HUD` `369e32a63ac1e7b7`, `_F15E.PIC`
`0431a9f36124787d`, `AV8IR.SEE` `d4791bcb3faf9379`, `PAVESPK.SEE`
`7fd9c6d3e909015e`, `F350.GAS` `fde47be5904b2cb6`, `F.BI`
`8719314060695f82`.

### F-15E record against the F-15C and the real aircraft

**17 of 219 statements differ from `F15.PT`**; 202 are identical. Of the 17, two
are the size words that follow from nine hardpoints against six. The rest:

| Field | `F15E.PT` | `F15.PT` |
| --- | ---: | ---: |
| `year` | 1990 | 1979 |
| PLANE `flags` | `0x15` (multi-crew) | `0x11` |
| `numHards` | 9 | 6 |
| Debris (`dmg` z, `dst` x) | 50, 60 | -90, -60 |
| Pitch rate `_brv.x` min/max, acc/dacc | -240/240, 311/621 | -225/225, 286/571 |
| `puffRot` x/y/z acc/dacc | 311/621 | 286/571 |

Everything else is the F-15C's, including all fourteen G envelope rows
(identical, row for row), empty weight, internal fuel, maximum takeoff, both
thrusts, top speed, ceiling, hit points (129), signatures, structure, drag,
stall, spin and crash words, damage tables, sounds, view offset and AI script.
The names, shape, default stores and the other changes above are the F-15E's
own, so the [record rule](../spec/aircraft-records.md#rule) is met by the shape,
stations, flag and handling rates. The numbers below are a **template-copy
list for John**, each against the game's own `F15E.INF` / `F15.INF`
(Jane's-style text, retail media, numbers only) and a cited public source. They
do not establish original game behaviour.

| Field | Record (= F-15C) | Real F-15E | Difference | Note |
| --- | ---: | ---: | ---: | --- |
| Empty weight, lb | 28,600 | 32,000 (14,515 kg, `F15E.INF`); plan figure about 31,700 | -10.6% | Copied. The F-15C's own `F15.INF` empty is 28,600 |
| Maximum takeoff, lb | 68,000 | 81,000 (36,741 kg, `F15E.INF`) | -16.0% | Copied. 68,000 is the F-15C with CFTs (`F15.INF`) |
| Internal fuel, lb | 13,662 | about 13,700 (7,643 L, 2,019 US gal at about 6.8 lb/gal) | about -0.5% | Agrees; the C's 13,455 lb is close too |
| Military thrust, lbf (both) | 32,558 | 29,200 (-220, public figure, unverified here) or 35,600 (-229, 17,800 each) | +11.5% / -8.5% | Copied |
| Afterburner thrust, lbf (both) | 47,860 | 46,900 (-220, 23,450 each, `F15E.INF`) or 58,200 (-229, 29,100 each, `F15E.INF`) | +2.0% / -17.8% | Matches the early -220 engine; short of the -229 |
| Engines | 2 | 2 | 0 | Agrees; two flame groups in the shape |
| Top speed, kt | 1,434 | Mach 2.5, 1,434 kt (`F15E.INF`) | 0 | Agrees exactly |
| Sea-level max, kt | 806 | 800 CAS (`F15.INF`) | +0.8% | Agrees |
| Ceiling `maxAlt`, ft | 100,000 | about 60,000 to 65,000 service | +54% to +67% | Envelope 0 G row reaches 105,000 ft; the host caps at the field |
| G limits | -4 / +9 | design +9 / -3 (`F15.INF`) | +9 agrees, -4 vs -3 | Agrees |
| Gun rounds | 450 | 512 (`F15E.INF`) | -12% | The C record has 940 |
| Hit points | 129 | none | n/a | Copied |

Thrust options for John (none chosen): keep retail (the early -220 figure
within 2% on afterburner); the -229 totals 35,600 / 58,200 lbf as in the plan;
or retail empty weight and maximum takeoff replaced by 32,000 / 81,000 lb. The
catalog year, 1990, fits the early -220 aircraft. Sources: Jane's text in
`F15E.INF`; public -229 figures (about 17,800 / 29,000 lbf) agree across
[migflug.com](https://migflug.com/?p=16829) and the
[Air Force fact sheet](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104499/f-15e-strike-eagle/)
(accessed 2026-11-10).

### F-15E G envelope rows

Speed in ft/s at altitude in ft. Fourteen rows, -4 to +9, identical to
`F15.PT`'s. The -3 and +6 rows are the same polygon, the -2 and +4 rows are the
same polygon and the -1 and +2 rows are the same polygon.

| G | Points |
| ---: | --- |
| -4 | 910@0, 920@1,000, 940@0, 950@0 |
| -3 | 475@0, 647@15,000, 880@29,000, 1,220@29,000, 1,380@21,000, 1,240@0 |
| -2 | 365@0, 567@21,000, 930@54,000, 1,440@54,000, 1,680@45,500, 1,770@30,000, 1,480@15,000, 1,270@0 |
| -1 | 250@0, 320@14,750, 430@29,500, 670@58,250, 900@85,000, 1,635@85,000, 2,007@71,000, 2,192@52,500, 2,257@35,000, 1,663@17,500, 1,310@0 |
| 0 | 140@0, 170@13,000, 245@31,000, 373@52,000, 531@74,000, 715@96,500, 870@105,000, 1,574@105,000, 1,980@95,250, 2,297@75,500, 2,470@54,250, 2,510@36,000, 1,865@18,000, 1,440@0 |
| 1 | 200@0, 235@13,000, 307@26,000, 563@65,000, 801@96,000, 870@100,000, 1,554@100,000, 1,897@91,500, 2,183@74,250, 2,366@53,625, 2,420@36,000, 1,750@18,000, 1,360@0 |
| 2 | same as -1 |
| 3 | 305@0, 441@18,500, 637@39,000, 930@68,000, 1,575@68,000, 1,897@56,000, 2,000@33,000, 1,575@16,500, 1,290@0 |
| 4 | same as -2 |
| 5 | 427@0, 633@18,500, 920@41,000, 1,290@41,000, 1,540@25,000, 1,260@0 |
| 6 | same as -3 |
| 7 | 510@0, 655@11,500, 900@22,000, 1,115@22,000, 1,240@17,000, 1,220@0 |
| 8 | 540@0, 675@8,000, 920@15,000, 1,090@14,000, 1,200@0 |
| 9 | 585@0, 687@5,500, 940@10,000, 1,020@9,000, 1,180@0 |

The 1 G row peaks at 2,420 ft/s (1,434 kt) at 36,000 ft, which is the record's
top speed. The +9 G row is already in the source, so unlike the Mirage 2000 no
G raise is needed; the record's -4 G floor is one more than the encyclopedia's
design -3 (an F-15C template value, noted only).

### F-15E resources

| Resource | Role | Note |
| --- | --- | --- |
| `F15E.SH` | Shape | 350 runtime faces (352 export), 0 lines, 0 billboards, five state words |
| `F15E_S.SH` | Shadow | 16 faces |
| `F15E_A.SH` / `F15E_C.SH` | Damage bodies | 311 / 350 faces |
| `F15E_B.SH` / `F15E_D.SH` | Damage pieces | 138 / 33 faces |
| `VIS340.SEE`, `F15R.SEE`, `F15.ECM`, `M61.JT`, `AIM120.JT` | Sensor, radar, jammer, gun, missile | already live (the F-15C's) |
| `F350.GAS`, `MK82.JT` | Default tank and bomb | live (F-4B/J, MiG-17, A-4 and the F-15C's derived rows) |
| `AV8IR.SEE`, `PAVESPK.SEE` | Internal FLIR, designator row | both live; see pods below |

The shape is a **new model**, not the F-15C's: no face of `f15e.SH` has the same
vertex set as any face of `f15.SH` (the two neutral dumps, 346 and 350 faces, share no face), yet
the outline is the same. The shape streamer has pivot 0 and hinge scale 0 (no
streamer).

### F-15E HUD and cockpit (for John)

**Fact.** `F15E.PT` names `av8.HUD`, the same as `F15.PT` (the F-15C record
already names it, so this is not a Strike Eagle error). TORE's F-15C flies
`AV8.HUD`, stem `AV8`, cockpit `~AV8H.PIC` (the Harrier cockpit,
`aircraft.rs`), so the retail F-15 family already flies a Harrier panel. The
HUD names the panels `~av8`, `~av8h`, `~av8s`, `~av8_l/c/r`, `~av8_lh/ch/rh`,
`~av8_ls/cs/rs`, `~av8_p`, `~av8_w`, plus GEAR, FLAP, BRAKE and HOOK symbols.
**No F-15 or Strike Eagle cockpit art exists in any archive**: the sweeps
`~F15*`, `*F15*`, `*F-15*`, `*STRIKE*`, `*EAGLE*` find only the shapes, skins,
`F150.GAS`, the encyclopedia pictures, `^ACF15*.5K` and `$F150.PIC`. The record
is a two-seater (`0x04`); TORE's crew-voice rule reads that flag and labels it
`RIO` ([cockpit voice](../spec/cockpit-voice.md#which-aircraft-have-a-crew-voice)).

Options (none chosen):

1. **Follow the record: `AV8.HUD`, stem `AV8`, `~AV8H.PIC`.** What retail does
   and what the F-15C does; no new art. The identity stays `F15E.PT`.
2. **Substitute another reviewed HUD family** (for example `F18.HUD`, whose
   aircraft is also a two-seater). Opinionated; extra wiring in `hud_aperture.rs`
   and the stem tables.
3. **Author a Strike Eagle panel.** Not in the media; only as a later decision.

Agent note: option 1 is the evidenced one; it needs one stem arm in F15E-7.

Update (F15E-7, 2026-11-10): John chose option 1 in the batch decision round ("go
with recs", opinionated); the stem arm already existed from B5-1 and needed no
change. See [F-15E sensors, HUD and cockpit](#f-15e-sensors-hud-and-cockpit-f15e-7-2026-11-10).

### F-15E pods (for John)

**Fact.** Rows 3 and 4 are `AV8IR.SEE` and `PAVESPK.SEE`, both flag `0x0008`
fixed rows at position (0, 0, 0), with no pylon, as `VIS340.SEE` and `F15R.SEE`.

- `AV8IR.SEE` has weight 0 and flags 2: an **internal FLIR** (91,140 ft zone,
  the Harrier's, X-31's and F-22's). It is not a pod and draws nothing.
- `PAVESPK.SEE` ("AN/ASQ-153 Pave Spike", weight 425 lb, flags 1) is a laser
  designator **pod**, a fixed pod row exactly like the F-16C's: its source
  position is a placeholder, and TORE's existing rule draws it under the model's
  lowest underside across its own length (see [external pods name no
  shape](aircraft-ordnance.md#external-equipment-pods-name-no-shape-eq-2-2026-11-06)).
- The real F-15E carries the **AN/AAQ-13 navigation and AN/AAQ-14 targeting
  LANTIRN pods** under the engine intakes (which side carries which differs
  between public sources; the pod weights are about 430 and 540 lb) on the
  fuselage stations. A Pave Spike is not a real F-15E store. The record's two
  rows are the game's stand-ins for the nav FLIR and the targeting/laser pod.

Other rows with the pod bit `0x400`: row 6 (centreline, `0x0601`) and row 7
(fuselage pair, `0x0505`); under TORE's current rule a Pave Spike could also be
loaded on those two stations from the Load Ordnance page.

Options (none chosen):

1. **Follow the record: draw the fixed `PAVESPK.SEE` pod as for the F-16C, and
   nothing for `AV8IR.SEE`.** The pod hangs under the belly, which is not the
   real intake position.
2. **Draw the fixed pod on the intake position** (one side of the fuselage pair,
   about the row 7 x of 6.3 ft out): matches the real LANTIRN, but is an
   authored placement over a placeholder row (B7 allows it only for stations the
   real jet has, and the intake stations are real).
3. **Do not draw the fixed pod at all** (keep the sight, hide the body).
   Opinionated.

Agent note: under B7 the intake position is the only real pod station; the belly
position is the one real aircraft do not use.

Update (F15E-7, 2026-11-10): option 3 was chosen: the sensor stays and no pod body
is drawn (see [F-15E sensors, HUD and cockpit](#f-15e-sensors-hud-and-cockpit-f15e-7-2026-11-10)).

### F-15E shape state words

Five state words, listed with the counts from `Shape::with_state` (the export
path adds two to every count, 352 total). The slot order and offsets
`0, 6, c, 18, 1e` are the shared base order of the F-15C, MiG-25 and Rafale
families (flame, brake, gear, flap, flap); there is **no rudder word** (the
Mirage's sixth), no hook word, no slat word, no canopy word and no nozzle word.

| Word | Faces | Geometry (shape units, x right, y forward, z up) | Reading | Status |
| --- | ---: | --- | --- | --- |
| `80c0` | +8 | Eight plates at x +/-8 (z -6 to 5) and x +/-4 to 13 (z -1), y -62 to -115 | Two afterburner flames | Confirmed by slot order and the pair of positions |
| `80c6` | +4 | Trapezoid y 20 to -1, z 16 to 30, x -6..5 to -3..3, and a front wedge to y 25, z 11 | Dorsal airbrake (one centreline plate, hinged at its front edge) | Confirmed as the source's brake word |
| `80cc` | +18 | Nose gear x 0, y 48 to 62, z -8 to -27; main wheels x +/-13 (y -4 to 6, z -27 to -11) with cross plates; main struts x +/-7 | Landing gear down | Confirmed; low point -27 |
| `80d8` | -2 | Removes the left inboard trailing-edge wedge, x -39 to -22, y -24 to -30 | Left flap word | Slot order only |
| `80de` | -2 | Removes the right inboard wedge, x 21 to 39 | Right flap word | Slot order only |

The F-15C shape has the same five words with the same face counts, apart from
gear (22 faces against 18).

**Rule 11 face table (from the `state_neutral` source dump).** The inboard
wedges are the same kind of 2-face strip as the Mirage elevons: top layer front
edge at z 1, bottom layer front edge at z 4, trailing edge at y -30, z 3. The
front edge y = -24 is therefore 1.5 units above or below the mid axis (y -24,
z 2.5), against the Mirage's 1.

| Surface | Faces | Vertices | Moved | On hinge line | Fixed |
| --- | --- | ---: | ---: | ---: | ---: |
| Right flap (x 21 to 39) | `5470` top, `548f` bottom | 4 each | 2 | 2 | 0 |
| Left flap (x -39 to -22) | `5557` top, `5576` bottom | 4 each | 2 | 2 | 0 |
| Right aileron (x 39 to 59) | `4b33` top, `4ac4` bottom | 4 each | 2 | 2 | 0 |
| Left aileron (x -59 to -39) | `42c5` top, `41d9` bottom | 4 each | 2 | 2 | 0 |
| Right stabilator | `4b5f`, `4b86`, `4ba7` top; `4c5d`, `4c8a` bottom | 4, 3, 4, 5, 4 | all | none (whole panel) | 0 |
| Left stabilator | `42f1`, `4318`, `4339` top; `43ef`, `441c` bottom | 4, 3, 4, 5, 4 | all | none (whole panel) | 0 |
| Right fin (x 21) | `4ae3`, `4b0a` | 4 each | see rudders | n/a | n/a |
| Left fin (x -22) | `4242`, `4269` | 4 each | see rudders | n/a | n/a |
| Airbrake (4 faces) | `568c`, `56a3`, `56ba`, `56d1` | 4 each | 2 | 2 | 0 |
| Gear (18 faces) | `50e3` ... `539b` | 4 each | 2 | 2 | 0 |

Verdicts:

- **Flaps (`80d8`/`80de`):** whole panels (two quads each), no diagonal or
  partial surface. Chord 6 units (2 ft), span 18 units (6 ft). The left strip is
  one unit shorter than the right (x -22 against 21, as the Mirage).
- **Ailerons (no state word):** the source has **no aileron word**. The outboard
  strip x 39 to 59 is a wedge of two faces per side (top quad, bottom quad) in
  the neutral shape. They are whole panels, but their front (hinge) edge is
  **slanted**: from (39, -24) to (59, -27), and its top, bottom and tip
  vertices are not collinear in 3D (z 1 and 4 at x 39, 3 at x 59). A turn about
  the best-fit axis through (39, -24, 2.5) and (59, -27, 3) leaves a gap of up to
  1.5 units at the root end. The inboard trailing corner (39, -30, 3) is shared
  with the flap, so the two must take the same angle. **Flagged as a
  slanted-hinge whole panel for John or F15E-3.** No diagonal split.
- **Stabilators (no state word):** five coplanar faces per side at z = -1
  (flat, zero anhedral; the real jet has about 15 degrees), root x 21 (-22
  left), root chord y -41 to -70, tip x 47. All five faces lie wholly in the
  panel, so the **whole-panel pitch turn about a lateral spindle** through the
  root chord midpoint (y -55.5, z -1) is rigid. The root vertices (21, -41, -1)
  and (21, -70, -1) are shared with fixed fuselage and fin faces (`4edd`,
  `4bdb`), so a turn opens a gap there up to `sin(angle)` times 15 units (the
  same root gap as other all-moving tails). Real stabilators also take roll
  (taileron, plan Q4); the shape is a legitimate carrier for that.
- **Rudders:** see below, a question for John.
- **Airbrake, gear, flame:** whole panels, as the F-15C.

The whole-surface gate (John, 2026-11-07 and 2026-11-08) is met by every flap,
aileron, stabilator, the brake and the gear. No diagonal or partial surface was
found, so **no probe sheet was needed**; only the slanted aileron hinge and the
fin question are flagged.

### F-15E surfaces (for John)

**Rudders.** Each fin is a single flat plate at x = 21 (-22), two windings, with
**no separate rudder face and no rudder state word** (the Mirage had both). The
fin is a vertical plane, not canted. A rudder would have to be authored by
splitting the fin plate (new geometry), or the whole fin turned (wrong), or left
out.

Options (none chosen):

1. **No rudder motion** (the F-15C record has none either; match retail).
2. **Split each fin quad at a hinge line (for example 30% of the chord from the
   trailing edge) into fixed fin and rudder panel** in the rig, an authored
   change over the source (fitted, John to approve since it adds geometry).
3. **Turn the whole fin as a one-piece vertical surface**: breaks the
   whole-surface rule's intent (the real fin is fixed).

**Taileron.** All-moving stabilators take roll as well as pitch on the real jet
(differential, fitted magnitudes, rule 12 style). The source has no state word
for them; the rig reads the neutral faces. Options: pitch only (match the
simple retail look), or pitch plus differential roll (the batch default,
B7).

**Flaps.** The two words delete the inboard wedge with nothing in its place; the
rig must keep it drawn (as the Mirage). **Ailerons** turn about the slanted hinge
above. **Slats, canopy and nozzles:** not in the source; recommend none.

**Conformal tanks.** No state word and no separable tank geometry was
identified; whether `f15e.SH` draws CFTs baked in is **unknown** and is
F15E-2's first frame check. Row 7 (a fuselage pair carrying 12 Mk 82 each, class
80, at 6.3 ft out and 8.7 ft below the origin) is the CFT-style carrier the
record gives.

### F-15E size (for John)

Measured on `f15e.SH` at scale 1 (1 unit is 1/3 ft), from the `state_neutral`
and gear dumps: x -68 to 68 (the wing tips, 136 units), y -73 to 116 (stabilator
trailing corner to nose probe tip, 189 units), z -13 to 43 (belly to fin top, 56
units; gear down -27 to 43, 70 units). The F-15C shape has the same extents
(x 136, y 189 from -71 to 118, z -13 to 44). Real figures from `F15E.INF`:
span 13.05 m (42 ft 9.75 in, 42.81 ft), length 19.43 m (63 ft 9 in, 63.75 ft),
height 5.63 m (18 ft 5.5 in, 18.46 ft).

| Axis | Shape | Real | Shape against real | Factor to real |
| --- | ---: | ---: | ---: | ---: |
| Span | 45.33 ft | 42.81 ft | +5.9% | 0.9444 |
| Length | 63.00 ft | 63.75 ft | -1.2% | 1.0119 |
| Height, belly to fin top (gear up) | 18.67 ft | 18.46 ft | +1.1% | 0.989 |
| Height, gear-down low point to fin top | 23.33 ft | 18.46 ft | +26.4% | 0.791 |

Verdict: **true size to within 6%**, a size match unlike the Mirage 2000's (26%
and 19% short). Only the span (the wing tips) and the
gear-down height stand out; the gear plates hang 14 units (4.7 ft) below the
belly, a longer leg than the real jet's.

Options (none chosen):

1. **Keep 1:1 for the F-15E (and the F-15C, which has the same size).** The shape
   is already close; no scale table entry.
2. **Scale the span only** by 0.944 (a one-axis entry). Narrows the wing and the
   pylon rows.
3. **Per-axis to real** x 0.944, y 1.012, z 0.989 (z against the gear-up height).
   The Mirage precedent was per-axis; here the factors are all within 6%.

Agent note: option 1 or 3; the gear-down height is a gear-plate fact (F15E-4).

### F-15E engines and thrust (for John)

The record says 2 engines and the shape has two flame groups (`80c0`, x +/-8), so
the count agrees with the real jet; no override is needed. The thrust totals are
the F-15C's (see the template table): afterburner within 2% of two early
F100-PW-220, short of two -229 by 17.8%. F15E-8 should compare the climb and
acceleration against the envelope rows and the real figures.

### F-15E hardpoints

All nine source rows, in original order. Positions are source units; feet divide
by three; x right, y up, z forward. Location byte: 0 Centerline, 1 Fuselage,
2 Internal Gun, 3 Internal Bay, 4 Wing, 5 Wingtip. Default stores here are the
decoded defaults; the per-row count is the row's `maxItems`.

| Row | Location | Flags | Position x/y/z | Feet right / up / fwd | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `VIS340.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `F15R.SEE` | Fixed radar |
| 2 | Fuselage | `0x0008` | 0 / 0 / -50 | 0 / 0 / -16.7 | 1 | 0 | `F15.ECM` | Fixed jammer |
| 3 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `AV8IR.SEE` | Fixed internal FLIR |
| 4 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `PAVESPK.SEE` | Fixed designator pod row |
| 5 | Fuselage | `0x0008` | 7 / -5 / 73 | 2.3 / -1.7 / 24.3 | 450 | 0 | `M61.JT` | Internal gun, 450 rounds (real 512) |
| 6 | Centerline | `0x0601` | 0 / -26 / 0 | 0 / -8.7 / 0 | 1 | 26 | `F350.GAS` | Tank (pod bit too) |
| 7 | Fuselage | `0x0505` | 19 / -26 / -5 | 6.3 / -8.7 / -1.7 | 12 | 80 | `MK82.JT` | CFT-style bomb pair, 12 a side (pod bit) |
| 8 | Wing | `0x01c5` | 43 / -14 / -14 | 14.3 / -4.7 / -4.7 | 4 | 14 | `AIM120.JT` | Wing pair, 4 a side |

Compared with the F-15C: the C has rows 3 (gun 940, at 5/-5/-5), 4 (AIM-120
x4 at 25/-10/0) and 5 (AIM-9M x4 at 50/-20/0), and TORE appends derived
centreline and wing-tank rows for it only when the reviewed C signature matches
(`aircraft_stations.rs`); the Strike Eagle's different rows never match, so it
gets no derived rows. The F-15E has no AIM-9 or Sparrow row, no wing tank row
and no wingtip row; the wing row (mask `0x01c5`) takes no tanks. Things F15E-5
must handle (none decided):

- The centreline tank (z -26) and the bomb rows (z -26) sit 14 units (4.7 ft)
  below the intake bottom (z -12) and 13 below the belly (z -13), and hang near
  the gear-down low point (z -27); expect ground clearance and pylon-rule
  questions. Stop and report with frames on clipping (John, 2026-11-07).
- The wing row at x 43 hangs 15 units (5 ft) below the wing surface (z 1 to 4), and 14.3 ft out of a 22.7 ft semi-span.
  The F-15C's fitted wing pylon is 11 ft out; compare.
- The bomb pair (12 a side, 6.3 ft out) and a LANTIRN-style pod share the
  intake region.
- There are no AIM-9 rails; the wing row's mask carries IR and radar bits
  (`0x1c5`), so Sidewinders load by mask.
- No sensor or designator pod may be hung on a station the real jet lacks (John,
  2026-11-07): see pods above.

### F-15E roster note

`F15E.PT` appends as batch order 1: checkpoint code 41, after `M2000` (40). The
wire identity is 9 bits since NET-ID9, so capacity is not a constraint.
`F15.PT` and `F15J.PT` keep their own identities.

### F-15E unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| Template numbers (empty, max takeoff, thrust, gun rounds) | Handling, takeoff, climb | John, options in the template table |
| HUD and cockpit | F15E-7 cannot start | John, three options |
| Fixed pod placement | Intake against belly | John, three options |
| Rudders (fin is one plate) | Rig, whole-surface gate | John, three options |
| Taileron roll | Real stabilators roll | John (batch default is yes) |
| Slanted aileron hinge | Gap at the root | F15E-3 measures with a pose grid |
| Real-size scaling | Stores, pylons | John, three options |
| CFTs baked in the shape? | Row 7 and frames | F15E-2 frame check |
| Tank and bomb clipping under the belly | Station rows | F15E-5 stop-and-report |
| Per-row default quantities | Stores load | F15E-6 |

### F-15E shape, damage looks and decals (F15E-2, 2026-11-10)

Implementation, slice F15E-2, Claude Sonnet (sonnet-high). `AircraftId::F15E`
flies `F15E.SH` at its own size: `aircraft_scale` stays `[1, 1, 1]`, because
the size question ([above](#f-15e-size-for-john)) is John's and is still open.
Nothing in this slice depends on the answer. The skin review (double-sided
panels, the two omitted decal slots, damage looks, the conformal-tank answer)
is in [the F-15E skin review](objects-and-shapes.md#f-15e-skin-review) in the
objects and shapes guide; the `render-f15e-skin` battery scenario captures the
top and under views with the gear down.

- **Shape and twins.** 350 runtime faces (352 on the export path), 17
  exact-vertex twin groups, every one a pair with opposite stored normals
  (pinned by `crates/tore-import/tests/f15e_shape.rs`, which skips without a
  retail install). No stored-normal exception is needed, as for the MiG-31 and
  the Mirage 2000.
- **Conformal fuel tanks: not in the shape.** The F-15E outline in plan and side
  view matches the F-15C's; the only extra volume is a flat shelf on the lower
  fuselage side. No tank shape exists to hang or hide, so F15E-5 draws any
  tank as a store.
- **Damage looks.** `F15E_A.SH` to `F15E_D.SH` (311, 138, 350 and 33 faces) and
  their four skins load (pinned by the same test). No whole-body variant is
  registered for the F-15E, as for the F-15C, MiG-25, MiG-31, F-16A and Mirage
  2000, so a destroyed F-15E draws the intact skin with regional cuts. Choosing
  an A to D substitution would be a decision for the damage slice; nothing here
  changes `damage_variant`.
- **Decals.** Two `E0` slots (3 and 4, the wing markings) stay omitted; the
  tail and fuselage art are textured faces and draw whole.

Provenance: spec-derived (retail shape, face counts and twins read as data);
the omit-not-fill decal rule follows the existing texture-state decision;
frames are review evidence only.

### F-15E control surfaces resolved (F15E-3, 2026-11-10)

The review's open surface items are answered by slice F15E-3 (details, fold
table and measured gaps in [control-surface
geometry](aircraft-control-surfaces.md#f-15e-control-surfaces-f15e-3-2026-11-10)).
The slanted aileron hinge is a whole-panel turn about the line from the root
pair's mid point to the tip vertex, root gap 0.10 ft at full travel; the
stabilators are whole panels with roll about a lateral spindle through the root
chord midpoint; the flap wedge turns whole and the `-1` branch's end triangle
draws while it is down; rudders: none, as John decided the same day. The
"Rudders" and "Taileron" options in "F-15E surfaces (for John)" above are
settled by his 2026-11-10 answers (no rudder motion, tailerons with roll).

### F-15E stations measured (F15E-5, 2026-11-10)

Slice F15E-5 measured every decoded store row on the drawn shape under the
standard pylon rules and **stopped on clipping** (John's rule): see [the F-15E in
stores on pylons](../spec/stores-on-pylons.md#the-f-15e-f15e-5-2026-11-10) for the
station table, every alternative, the frames and the options. In short: row 6
(centreline tank) and row 8 (wing AIM-120 pair, 14.3 ft out) are clean; row 7
(the 12 Mk 82 conformal-style pair, 6.3 ft out) hangs on two triple racks a side,
its aft inboard bombs enter the body by 0.06 ft (right) and 0.02 ft (left), they
sit 0.32 ft from the main gear and the gear leg crosses them while it retracts
(gear 0.4); several larger alternatives (GBU-10/GBU-30 about 2 ft, HARM and
Shrike 1.0 ft) clip the belly flank. The "Things F15E-5 must handle" list in
"F-15E hardpoints" above is answered: the tank and bombs hang 0.17 ft under the
shelf and flank and are 3.3 to 3.9 ft above the ground (the gear-down low point is
9 ft under the reference, so they do not hang near it once the pylon rules place
them). The Pave Spike row already draws no body under the 2026-11-07 fixed-pod
rule (John's decision 3 of 2026-11-10), checked by the retail pod guard. No row,
mask or default changed and the ordnance audit pins did not move.

### F-15E sensors, HUD and cockpit (F15E-7, 2026-11-10)

Slice F15E-7 confirmed the F-15E on the records it names. John's decision round of
2026-11-10 ("go with recs") decided the question the review left open in [F-15E
HUD and cockpit](#f-15e-hud-and-cockpit-for-john): **follow the record**, `av8.HUD`,
the same as TORE's F-15C, and the Pave Spike row draws no pod body while its
sensor stays. That settles what the batch identity slice had wired as an interim
agent decision, so the F-15E's binding is now **opinionated (John, 2026-11-10)**,
no longer interim; only the comments in `aircraft.rs` and `hud_aperture.rs`
changed. Added: a retail-reading test and one aperture test. No behaviour changed.

**Sensors (spec-derived, retail records, already reviewed).** `--sensor-summary`
reads: `F15E: radar F15R.SEE search 150 nmi 60x60 deg track 100 nmi 45x45 deg
look-down 30 ADVANCED; infrared AV8IR.SEE search 15 nmi 60x60 deg track 15 nmi
45x45 deg; visual VIS340.SEE search 10 nmi 170x140 deg track 5 nmi 170x140 deg;
jammer F15.ECM LATE strength 0.30; signature radar 100 infrared 100`. Radar and
jammer are the F-15C's line, record for record (`F15R.SEE`, `F15.ECM`); the F-15E
adds the internal FLIR as an infrared channel. Nothing under
`crates/tore-sim/src/sensors` branches on the aircraft identity, so radar modes,
the RWR and the jammer behave as for the F-15C. A live-fire run (`--live-fire
--aircraft f15e --weapon-slot 3`) shows the Radar window `TWS 10 ... TRACK`, the
HUD `ARM 3 AIM-120 92% IN RNG`, and the AIM-120 locks, flies, hits and kills
(`shots=1 hits=1 kills=1` at 1,500 ticks; at 700 ticks the round is still flying,
`hits=0 active=1`). `--sensor-channel ir` shows `IR` on the Radar window (frame).
The jammer key lights `JAM` in the RWR window.

**FLIR and laser designation (decision 3).** The aircraft's own sight takes the
first laser record among its fixed rows, else the first FLIR
(`combat::sight::fixed_sight`). The F-15E carries both `PAVESPK.SEE` and
`AV8IR.SEE`, so its targeting sight is the **Pave Spike laser** (10 nmi, the
pod's hemisphere below the aircraft, as the F-16C and A-10), and the internal
FLIR stays a sensor channel on the scope. `--a2g-run UKR QUCOL --aircraft f15e
--load GBU12 --station 2 --count 2 --attack laser --altitude 15000 --speed 480
--from 6 --seconds 120` prints `sight ... kind Laser`, designates the tank at
t 0.26 s, reads `OUT-OF-REACH` then `in-reach yes`, releases at 15.2 s with
`laser Lasing`, and the bomb impacts 2.9 ft from the target (`target-hp 100/200`).
The target-cam page for the F-15E shows the flashing `LASE` cue and the pin and
FLIR pages. **No pod body is drawn.** The row sits at (0, 0, 0) inside the
airframe, which the 2026-11-07 fixed-pod rule never draws; the retail guard in
`pod_equipment_tests.rs` (extended in F15E-5) pins that the F-15E's `fixed_pods`
list is empty, and the F15E-5 below and quarter frames show a clean belly. The
real jet's intake LANTIRN pods are not drawn either (no row decoded, none
invented).

**HUD and cockpit (opinionated, John, 2026-11-10).** `AircraftId::F15E.hud()` is
`AV8.HUD`, the cockpit stem `AV8`, the forward art `~AV8H.PIC` and the instrument
window frame `~AV8_P.PIC` (`Hud::parse` of `AV8.HUD` names it), each equal to the
F-15C's. The F-15E record is not aliased: `F15E.PT` stays the identity. Pinned in
`crates/tore-import/tests/f15e_cockpit.rs` (retail-reading, skips without media):
the five sensor rows `VIS340.SEE, F15R.SEE, F15.ECM, AV8IR.SEE, PAVESPK.SEE`, the
F-15C's first three rows, the binding, the panel, the overlays and the record's
own eye offset. `hud_aperture.rs` resolves the stem `AV8` to the Su-27's measured
glass (the Harrier cockpit art), checked by a new unit test: the F-15E mask equals
the F-15C's and the Su-27's and is not empty. `--cockpit-sheet OUT_DIR
F15E,F15,AV8` (run through `tools/agent-run.sh`) printed `f15e: stem F15E cockpit
~AV8H.PIC art 1280x490 FITTED CLR`, centre mirror x 531 y 5 w 208 h 51, left x
156 y 180 w 109 h 171, right x 1005 y 180 w 109 h 171, identical to the F-15C and
the Harrier. No F-15 or Strike Eagle cockpit art exists (`~F15*` is absent from
both archives, as the review found), so the F-15E flies a Harrier cockpit, as the
F-15C does.

**Views (windowed).** Front cockpit with the HUD inside the combiner glass and
the six small-layout windows; look left and right 45 degrees (the cockpit art
turns away with the look, the HUD and canopy bow at the frame edge); 90 degrees
left and right and 60 degrees up (no cockpit art, open sky, as for the F-15C);
`--flight-view 4` (F3 Up) and `--flight-view 3` (F2 Back). F2 Back shows the
spine, the two outboard fins and the engine nozzles from the pilot's eye, the same
picture as the F-15C's. The standalone instrument pages 0 to 9 draw with the
Harrier bezel.

**Instruments and two engines.** The Systems window shows `THR`, `TEMP`, `OIL`,
`HYD` and `FUEL` / `(+ EXT)`; one engine readout serves every aircraft, so the
two-engine F-15E shows one throttle and one temperature, as the F-15C's twin
engines do (the panel art has no per-engine needle; the readouts are generated
text, and no host code reads a second engine's state for the panel). Fuel reads
the F-15E record's own number: `FUEL 13662 LBS`, and `(+ EXT 2300 LBS)` with the
default `F350.GAS` centreline tank (the F-15C record's 13,662 as well, a copied
value listed in the review; the tank is the F-15E's own default row). The Weapons
window lists `450 M61`, `12 MK-82`, `4 AIM-120` and `30 CHAFF` / `30 FLARE`. The
HUD reads the F-15E's own state (airspeed and altitude boxes `564` and `5000` in
the lock frame, `1.0G`, `70%` throttle).

**Eye point (agent decision, 2026-11-10, left unchanged).** `mirrors::pilot_eye`
is one fixed offset for every aircraft, 7 ft above and 10 ft ahead of the model
origin, used only by F2 Back, the rear-view mirrors and the replay views; the
forward cockpit looks from the origin with 2D art. The record's own `viewOffset`
(x 0, y 20, z 60) is the F-15C's and no host code reads it (as the F-16A review
found), so no per-aircraft eye exists. Measured on the neutral shape (the faces
inside 4 units of the centreline, 1 unit = 1/3 ft): the F-15E's canopy top is 16
units, 5.3 ft above the origin, over 43 to 73 units, 14.3 to 24.3 ft ahead of it;
the F-15C's is the same 16 units over 45 to 77 units, 15.0 to 25.7 ft. The fixed
eye therefore stands 1.7 ft above the canopy top and 4.3 ft behind its forward
span on the F-15E, 5.0 ft behind on the F-15C: the same place against both
canopies, and the F2 Back frames match. Not changed. A seated eye would be a fleet
change (a table keyed by aircraft through six call sites), a question for John, as
in the Mirage 2000 review.

**RWR with a threat.** As for the Mirage 2000: no scripted path plots an emitter
on the player's RWR (the live-fire target emits no radar and the diagnostic
incoming round plots at the centre). The frames show the window with the jammer
lit and with an incoming round; the RWR code is aircraft-agnostic and unit
tested.

Evidence and frames: `.local/m3-aircraft/frames/F15E-7/` and
`.local/m3-aircraft/checks/F15E-7/`.

## Gripen source review (2026-11-10)

Research mode, slice GRI-0 (batch 5). This reviews `GRIPEN.PT` and its
dependency closure before `AircraftId::Gripen` exists. `GRIPEN.PT` is the 42nd
retail identity (checkpoint code 42 in the batch order, after `F15E`); the wire
identity is 9 bits now, so capacity is no constraint. It is not a parked target
(`parked_aircraft.rs` has no `GRIPEN.PT` entry). No original module was
executed; PT, SEE, ECM, JT and GAS are read as bounded BRF data with the shared
field order, SH and HUD are inspected as data. Slice done by Claude Sonnet
(sonnet-medium). Nothing here changes code or behaviour.

Four questions are open for John and are **not decided here**: the HUD and
cockpit ([below](#gripen-hud-and-cockpit-for-john)), the AAS-38 pod row
([below](#gripen-aas-38-pod-row-for-john)), the canards
([below](#gripen-canards-for-john)) and the record values that look copied
([below](#gripen-record-values-that-look-copied-for-john)). The surfaces pass
the whole-surface gate and real size needs no change (see
[size](#gripen-real-size)).

### Gripen build and extraction

`FA_1.LIB` and `FA_2.LIB` are the same pair as the MiG-31 review (see the [MiG-31
build section](#build-and-extraction)). The closure was extracted locally to
`.local/m3-imports/scratch/gripen` with `tore-extract --include 'GRIPEN*'
--include 'GRI*' --include '_GRI*' --include '~*' ...` (the `~*` sweep also
pulled every cockpit picture) and the `SETUP.ESA` installer copy was extracted
too. Its `GRIPEN.PT`, `GRI.SH`, `GRI_A.SH`, `GRI_S.SH`, `_GRI.PIC`, `_GRI_A.PIC`,
`GRIPEN.PTS`, `AAS38.SEE`, `BK27.JT`, `F.BI`, `VIS340.SEE`, `F18R.SEE` and
`F18.ECM` bytes are identical to the root copy; only `GRIPEN.HUD` differs by
build (as `RAFALE.HUD` does). The usual five unrelated archive errors appear.

### Gripen identity and record

| Field | Decoded value |
| --- | --- |
| PT | `GRIPEN.PT`, 14,805 bytes, SHA-256 `9cc5c3f11d10d4ce9b291164d77102d00ec16685b874d2b56d81384527251487` |
| Displayed names | `Gripen`, `Saab Gripen`, `GRIPEN.PT` |
| Schema | OBJECT + NPC + PLANE, 219 statements; `structType` 5; `typeSize` 660; `instanceSize` 626 (the parser default) |
| Object class / year / hit points | `0x8000` (fighter) / 1998 / 99; `expType` 30 |
| OBJECT flags / PLANE flags | `0x806bf3` (the Rafale and F/A-18 value) / `0x11` (seat and base bit; hook, no-lift, carrier, multi-crew clear) |
| Shape | `gri.SH` (33,280 bytes), shadow `gri_s.SH` (15 faces) |
| Damage looks | `GRI_A.SH` 409 faces, `GRI_B.SH` 50, `GRI_C.SH` 296, `GRI_D.SH` 256 |
| Skins | `_GRI.PIC` (152,424 bytes, 256 x 586), `_GRI_A.PIC` 137,084, `_GRI_B.PIC` 42,704, `_GRI_C.PIC` 80,664, `_GRI_D.PIC` 71,044 |
| Sounds | loop `&JET1N.11K`, second `&JET6A.11K` (the Rafale's), `&powerup.5k`, `&powerdn.5k`; all present |
| HUD | `gripen.HUD` (own file, 4,608 bytes) |
| AI script | `f.BI` (present) |
| Sensors / jammer / gun | `VIS340.SEE`, `F18R.SEE`, `F18.ECM`, `AAS38.SEE`, `BK27.JT` |
| PTS | `GRIPEN.PTS` (4,608 bytes; names `ICONGRI.PIC`) |
| Radio callout | none: no `^ACMGRI*` exists (the callouts are `2000`, `F1`, `IG21/23/27/29/31`, `R3`, `R3E`) |
| Encyclopedia | `GRIPEN.INF` and `GRIPEN_0..7/C/E/F/P.PIC` in `disc2/FA_3.LIB`; `GRI_D/M.11K` and `.CB8` narration in `FA_10B.LIB`; reference media |
| Checkpoint code / CLI key | 42 and `gripen`/`gripen.pt`, reserved for B5-1 |

The decoded record equals the catalog row in [`fa-aircraft.csv`](fa-aircraft.csv)
field for field. Hashes of the files later slices depend on:

| Resource | Bytes | SHA-256 |
| --- | ---: | --- |
| `GRIPEN.PT` | 14,805 | `9cc5c3f11d10d4ce9b291164d77102d00ec16685b874d2b56d81384527251487` |
| `GRI.SH` | 33,280 | `ea05b2789f2c8b342517cd155f4f2e28c52aa77f0fab9ec1bb6cee6e885d96e3` |
| `GRI_S.SH` | 4,608 | `80bfce4989c0e8318b9d1eccdfb29cca77dc5862e6544c45a4d3c6c69ced7134` |
| `GRI_A.SH` | 29,184 | `fecb39f8c46f5eee52dcc1d6b8d13c945e60c505c795b085fb9ba28b8e0bd091` |
| `GRI_B.SH` | 8,704 | `591e28812902504a7abbd541f21c6c6024fbc472b3c825967f02dbf5b135d34c` |
| `GRI_C.SH` | 20,992 | `96163db969637ac59fbe795077192ac62d40b99a9a49f029b510fe28e3363c20` |
| `GRI_D.SH` | 20,992 | `e17dbf501e1f77f2a7cf6a2ec2af0a833adc109fa8cd06efd6c9f4b0956b72d9` |
| `GRIPEN.HUD` | 4,608 | `dab77fd5cc30007240e84f3c47b664a26a64f81444579bffd03a923e9df3ec80` |
| `GRIPEN.PTS` | 4,608 | `e2b513a473c0deb2bf424d8f0bb013bf7ccafe76d01bc6e266be4fcc5e04d33c` |
| `AAS38.SEE` | 492 | `9e447523a2fda1612d17fa7d76d3beb0fb03f11115f9e855937ffd2875f5f75b` |
| `BK27.JT` | 2,719 | `0458077843fcc3c6252acfdb706f735cd7b26f80c0d36a74adbe80ab008a106f` |
| `_GRI.PIC` | 152,424 | `b1f65130b86336e1d68795802c29f2cafe050c004741301826a44309e98b2eb2` |

### Gripen record against the templates

The closest records are `RAFALE.PT` and `F18.PT` (180 of the 219 statements
equal, pointer statements excluded), then `F18C.PT` (179) and `M2000.PT` (177).
**27 statements differ from the Rafale, 27 from the F/A-18, 30 from the Mirage
2000.** 170 statements are identical in all five records: the damage and
`systemDamage` tables (all 45 entries), stall, spin, crash, `puffRot`, `_bv`,
`_brv`, rudder, throttle, `structureLimit` 5,120, `structureWarnLimit` 2,560,
the Doppler settings, the minimum altitude, the drag constants `coefDrag`,
`airBrakesDrag`, `flapsDrag`, `bayDrag`, `flapsLift` and the `loaded*` limits.
That block is the shared F/A-18 family template, not Gripen data. The Gripen's
own statements, with the other four records for comparison:

| Field | Gripen | Rafale | F/A-18 | Mirage 2000 |
| --- | ---: | ---: | ---: | ---: |
| `year` | 1998 | 2005 | 1986 | 1984 |
| Empty weight, lb | 14,599 | 17,100 | 23,050 | 17,100 |
| Max takeoff, lb | 27,560 | 40,100 | 49,224 | 37,480 |
| Internal fuel, lb | 5,000 | 9,900 | 11,220 | 6,346 |
| Engines | 1 | 2 | 2 | 2 |
| Thrust / afterburner, lbf | 12,140 / 18,100 | 24,000 / 32,000 | 17,687 / 32,000 | 17,687 / 32,000 |
| `maxAlt`, ft | 60,000 | 65,000 | 60,000 | 59,000 |
| `envMin` / `envMax` | -4 / +9 | -4 / +9 | -4 / +9 | -4 / +8 |
| Hit points | 99 | 100 | 116 | 100 |
| `structure[0]` / `[1]` | 1,229 / 1,933 | 1,278 / 1,946 | 1,193 / 1,743 | 1,161 / 2,129 |
| Signatures 0..3 | 94, 94, 90, 100 | 95, 95, 70, 50 | 104, 104, 100, 100 | 91, 91, 100, 100 |
| `maxVisDist` / view offset y, z | 168 / 30, 60 | 172 / 20, 80 | 204 / 20, 60 | 157 / 20, 60 |
| Debris (`dmg` x, z; `dst` x, y, z) | -30, -20; 0, 0, 30 | -60, 0; 0, 20, -40 | 0, 40; -40, 0, 0 | -40, 0; 20, 10, 0 |
| `turbulencePercent` | 131 | 130 | 116 | 122 |
| `spinEntry` | 1 | 1 | 0 | 0 |
| Fuel flow (`fuelConsumption` / `aftFuelConsumption`) | 2 / 14 | 3 / 16 | 2 / 16 | 2 / 16 |
| `_gpullDrag` / `wheelBrakesDrag` / `gearDrag` / `loadedDrag` | 30 / 102 / 23 / 30 | 30 / 40 / 38 / 30 | 38 / 40 / 23 / 65 | 40 / 102 / 38 / 30 |
| `negGLimit` | 0 | 0 | 2,560 | 0 |

Apart from the table the Gripen differs from the Rafale in the resource names
(shape, HUD, second sound is the same) and the nine hardpoint rows. The
14 G rows are the Gripen's own (see [G rows](#gripen-g-envelope-rows)): none
equals a Rafale, F/A-18 or Mirage polygon.

### Gripen published audit

The comparison is the game's own encyclopedia text (`GRIPEN.INF`, a Jane's
extract in the retail media, numbers only) and public figures (Wikipedia, JAS
39C/D specifications, accessed 2026-11-10; MILAVIA and other pages for the JAS
39A). They do not establish original game behaviour.

| Field | Record | Published | Difference | Verdict |
| --- | ---: | ---: | ---: | --- |
| Empty weight, lb | 14,599 | 14,600 (6,622 kg, JAS 39A, encyclopedia) | 0.0% | Agrees exactly |
| Maximum takeoff, lb | 27,560 | 27,558 (12,500 kg, JAS 39A, public); encyclopedia says about 28,660 (13,000 kg) | 0.0% / -3.8% | Agrees with the public 39A figure |
| Internal fuel, lb | 5,000 | 5,000 (2,268 kg, encyclopedia) | 0.0% | Agrees exactly |
| Engines | 1 | 1 (one RM12) | none | Agrees; shape has one nozzle |
| Military thrust, lbf | 12,140 | 12,140 (54 kN, encyclopedia) | 0.0% | Agrees exactly |
| Afterburner thrust, lbf | 18,100 | 18,100 (80.5 kN, encyclopedia) | 0.0% | Agrees exactly |
| Ceiling, ft | 60,000 | 50,000 (15,240 m service ceiling, public) | +20% | Looks copied (equals the F/A-18 field) |
| Top speed at 36,000 ft | 1,145 kt (1,933 ft/s, 1 G row) | Mach 2 (about 1,146 kt at that altitude) | 0% | Agrees |
| Positive G | +9 | +9 (encyclopedia) | none | Agrees |
| Negative G | -4 (row -4 exists) | -3 (public) | one row | Looks copied (all five records carry `envMin` -4) |
| Gun rounds | 570 | 120 (BK27, public) | +375% | Looks copied (equals the F/A-18's M61 count) |
| Store stations | 7 (centreline, 2 wing pairs, tip pair) | 7 (two wingtip, two under each wing, one centreline; encyclopedia) | none | Agrees |

The record is genuinely the JAS 39A, not a template in the mass, fuel, engine
and thrust fields.

### Gripen record values that look copied (for John)

Per standing rule B7, listed, not changed. Retail values stay retail unless
John decides. "Copied" means equal to another record where the real aircraft
disagrees:

1. **Ceiling `maxAlt` 60,000 ft.** Equals the F/A-18 value; the real service
   ceiling is 50,000 ft (15,240 m). Option: keep retail; or cap at 50,000 ft
   as a TORE override (needs the 0 G and 1 G polygon peaks, which reach 63,000
   and 60,000 ft, trimmed).
2. **Negative G -4.** All five reviewed records have `envMin` -4 and a row at
   -4; the real limit is -3 (public figure). Option: keep retail (the -4 row
   is the Gripen's own polygon); or mask the -4 row.
3. **Gun rounds 570.** The F/A-18's M61 count. Real BK27 holds 120 rounds
   (public figure). Option: keep retail; or fit 120.
4. **Template block.** The 170 shared statements listed above, plus
   `OBJECT.flags` `0x806bf3` and `viewOffset.z` 60 and `sigs[3]` 100 (F/A-18
   values), the second sound `&JET6A.11K` (the Rafale's), `gearDrag` 23 and
   `fuelConsumption` 2 (F/A-18), `spinEntry` 1 (Rafale), `wheelBrakesDrag`
   102 (Mirage). These are drag, damage, handling and sound constants; there
   is no real-Gripen figure to cite, so they are recorded as template values.
5. **Sensors.** `VIS340.SEE`, `F18R.SEE` (F/A-18 radar), `F18.ECM` are the
   family sensors; the real aircraft has the PS-05/A radar. Record fact, left.
6. **Everything else is the Gripen's own** and matches the real JAS 39A: empty
   weight, takeoff weight, fuel, engine count, both thrusts, the 14 G rows,
   year, hit points, structure and the station layout.

### Gripen real size

`gri.SH` at 1 unit = 1/3 ft, over the neutral dump (383 runtime faces; the
shape's own export adds 5 branch faces for 388):

| Axis | Units | Feet | Real (JAS 39A) | Difference |
| --- | --- | ---: | ---: | ---: |
| Span x (-41 to 41, wingtip rails) | 82 | 27.3 | 27.56 (8.40 m, encyclopedia) | -0.9% |
| Length y (-61 to 77, nozzle to nose probe) | 138 | 46.0 | 46.25 (14.10 m, JAS 39A, encyclopedia) | -0.5% |
| Height z, gear down (-16 to 29) | 45 | 15.0 | 14.76 (4.50 m, encyclopedia) | +1.6% |
| Height z, gear up (-8 to 29) | 37 | 12.3 | n/a | n/a |

**Verdict: true size within 2% on every axis. No scaling is needed.** The
plan's "length 46.3 ft (JAS 39C)" is the JAS 39A figure; the 39C is 14.9 m
(48.9 ft) in the public specification, 6% longer than this shape, but the
record and the encyclopedia are the 39A. Decision for John: none required;
the Mirage per-axis scale is not needed here.

### Gripen state words and moving parts

The shape has six state words, listed with the faces each adds or removes when
set to 1 (`Shape::with_state`, 383 neutral faces; the export path adds 5):

| Word | Faces | Low point | Geometry (shape units, x right, y forward, z up) | Reading | Status |
| --- | ---: | ---: | --- | --- | --- |
| `8080` | +4 | -4 | Two crossed flat plates behind the nozzle, y -61 to -85 | Afterburner flame | Confirmed by slot order and position |
| `8086` | +4 | -4 | One quad per side at x +/-9 to +/-13, y -38 to -43, z 1 to -4, two windings | Side airbrake, rear fuselage | Confirmed; hinge on the forward vertical edge (x +/-9, y -38) |
| `808c` | +22 | -16 | Nose plates x 0 (y 38 to 44, z -5 to -15), two cross plates; each main: strut plate x 4 to 6 (y -8 to -14), wheel plate y -11 (x 2 to 7), door plates x 9 to 15 (y 1 to -14, z -3 to -9) | Landing gear down | Confirmed; slot order |
| `8098` | -6 | none | Removes the left trailing strip, x -12 to -40, y -27 to -33 | Left flap word | Slot order only |
| `809e` | -6 | none | Removes the right trailing strip, x 12 to 40 | Right flap word | Slot order only |
| `80a4` | 0 net | +4 | Swaps the fin's rudder quad for a copy turned about y -43: trailing edge (0,-50,6)/(0,-49,23) goes to (4,-49,6)/(3,-48,23) | Rudder, positive direction only | Confirmed by geometry |

The slot order (flame, brake, gear, a skipped `8092`, flap, flap, rudder) is the
shared family order. There is no hook word (hook flag clear), no slat word, no
canopy word, no nozzle word and **no canard word: the canards are fixed
geometry in the source**, so the rig must turn them from nothing. The shape
streamer has pivot 0 and hinge scale 0.

| Moving part | Source panel | State word | Rig note |
| --- | --- | --- | --- |
| Canard, each side | One pentagonal plate (two windings) plus a root filler triangle (two windings) | none | Fixed in the source; rig turns it |
| Elevon, inboard, each side | Trailing strip, a quad on each layer | `8098`/`809e` remove it with the outboard strip | The rig keeps the strip drawn and turns it |
| Elevon, outboard, each side | Outer strip, 2 triangles per layer | same word | Same hinge, same angle |
| Rudder | 2-face quad on the single fin | `80a4` | Hinge on the fin leading edge of the rudder |
| Airbrake | One quad (two windings) per side | `8086` | Appears only when open |
| Gear | 22 faces, no wheels | `808c` | Rigid plates |
| Burner | 4 plates | `8080` | Additive plume |

### Gripen canards (for John)

**Fact.** Each canard is one pentagonal plate, two faces (top and bottom
winding), vertices (right side) (10,19,1), (22,15,2), (22,19,2), (8,37,1),
(8,30,1): root edge on the intake side at x 8 to 10 (y 19 to 37, an 18-unit
root chord, 6 ft), tip at x 22 with a 4-unit chord, trailing edge sweeping aft
as it goes out, leading edge running 18 units aft over 14 outboard, and a 4
degree dihedral (z rises 1 unit over 14). The plate is planar. A root filler
triangle (10,19,1), (9,19,1), (8,30,1) with two windings sits against the
fuselage. The left canard is the mirror with one root vertex 1 unit off (x -7
against +8 at y 37); the faces are `24ae`/`2553` and filler `25c2`/`25e0`
against `212d`/`2284` and `22f3`/`2311`.

Rule 11 face table (no vertex lies on a hinge, because there is no source
hinge; the rig supplies a lateral spindle through the root chord midpoint,
about (9, 28, 1), tilted by the 4 degree dihedral):

| Surface | Face | Vertices | Moved | On hinge | Fixed |
| --- | --- | ---: | ---: | ---: | ---: |
| Right canard | `212d`, `2284` | 5 | 5 | 0 | 0 |
| Right filler | `22f3`, `2311` | 3 | 3 | 0 | 0 |
| Left canard | `24ae`, `2553` | 5 | 5 | 0 | 0 |
| Left filler | `25c2`, `25e0` | 3 | 3 | 0 | 0 |

The only fixed faces touching a canard share vertex (8,30,1) / (-8,30,1)
(`2155`, `234a`, `273a`, `307b` and the left four): the root edge of a panel
turned about a spindle opens a small gap against the intake side (up to
`sin(angle)` times the half root chord, about 9 units from the spindle ends,
so roughly 1.5 units at 10 degrees). This is the same root gap of any
whole-panel tail. **Whole-surface gate: met.** Each canard is a whole panel
(plate plus filler), no diagonal, no partial surface. No probe sheet is needed.

Real aircraft: the foreplanes are all-moving, mainly for pitch, and angle
leading-edge-down to act as air brakes on landing (public sources; Wikipedia
article, accessed 2026-11-10). Whether they ever take differential roll is
disputed in the public sources I found (forums and secondary articles say the
elevons, inboard and outboard, carry pitch and roll and the canards move
together). The encyclopedia says only "independently movable foreplanes ...
leading-edge sweep of approximately 43o".

Options for John (none chosen):

1. **Pitch only (symmetric).** Both canards share one pitch angle. Simplest;
   matches the cited reading that the elevons carry roll.
2. **Pitch plus a landing air-brake bias** (leading edge down on the ground or
   with the gear down and brakes). Adds the real landing behaviour.
3. **Pitch plus differential roll.** Only if John wants the unconfirmed
   reading; no evidence found that the real canards do.

Agent note: option 1 or 2 is the supported reading; roll stays on the elevons.

### Gripen elevons, rudder, airbrakes and gear (rule 11 tables)

Each wing's trailing edge is the straight line y = -27 from x 12 to x 40 on the
right (-12 to -40 on the left); the trailing edge of the strip falls from y -33
at x 12 to -32 at x 26 and -31 at x 40 (chords 6, 5 and 4 units, 2.0 to 1.3
ft). The hinge vertices sit at z -2 (top layer) and z -1 (bottom layer); the
trailing edge is a knife edge at z -2, so the single hinge axis y -27, z -1.5
is within 0.5 unit of every hinge vertex.

| Surface | Face | Vertices | Moved | On hinge (y -27) | Fixed |
| --- | --- | ---: | ---: | ---: | ---: |
| Right inboard (x 12 to 26) | `5fd2`, `5ff1` | 4 | 2 | 2 | 0 |
| Right outboard (x 26 to 40) | `6010`, `602b` | 3 | 2 | 1 | 0 |
| Right outboard | `6046`, `6061` | 3 | 1 | 2 | 0 |
| Left inboard (x -12 to -25) | `6194`, `61b3` | 4 | 2 | 2 | 0 |
| Left outboard (x -25 to -40) | `61d2`, `61ed` | 3 | 2 | 1 | 0 |
| Left outboard | `6208`, `6223` | 3 | 1 | 2 | 0 |
| Rudder (neutral) | `5d92`, `5db9` | 4 | 2 | 2 (y -43) | 0 |
| Rudder (word `80a4`) | `5dff`, `5e26` | 4 | 2 | 2 (y -43) | 0 |
| Airbrake (each side) | `62ad`/`62c4`, `634a`/`6361` | 4 | 2 | 2 (y -38) | 0 |
| Gear (22 faces) | `594b` ... `5d5b` | 4 | 2 | 2 | 0 |

Verdicts:

- **Every elevon is a whole panel in the source.** No elevon is split across a
  diagonal or threshold; no face has a fixed vertex off the hinge line. The
  inboard strip is a quad on each layer; the outboard strip is the quad cut
  into two triangles per layer. Each side is thus 6 faces. **Inboard and
  outboard share the seam vertices (26,-32,-2) and (26,-27,*) on the right and
  (-25,...) on the left**, so rule 12 applies: both take pitch and roll at the
  same angle or the seam shows. The right and left seams are 1 unit apart (26
  against -25). **The flap words remove inboard and outboard together**, one
  word per side; the Mirage words removed only the inboard strip. The rig must
  not hide the strip.
- **Elevon crack at the inboard corner.** The trailing corner (12,-33,-2) is
  also a vertex of the fixed centre-body strip faces `5716`/`57a8` (left:
  `51d4`/`52dc`) at x 9 to 12, which fall away to y -36; turning the strip
  leaves a small slit there. Every other neighbour shares only hinge-line
  vertices. The strips are thin (up to 2 ft chord, about 0.7 ft trailing-edge
  travel at 20 degrees) as on the Mirage.
- **Rudder.** One hinged quad (two windings), hinge on the leading edge x 0, y
  -43, z 6 to 23. Retail swap is about 31 to 34 degrees, rounded to whole
  units, positive direction only. The trailing vertices (0,-50,6) and
  (0,-49,23) are shared with fixed faces (`3bba`, `4ba9`, `4bd7`, `4c36`,
  `4c9f`, `4d1c`, `4d85` and others) so a turned rudder leaves a one-unit crack
  against the spine and the fin cap, as on every reviewed aircraft.
- **Airbrake.** Two plates, one per side, each a quad with two windings; open
  they swing out to (+/-13, -43) from the forward edge (+/-9, -38), about 39
  degrees, matching the real paired rear-fuselage brakes. Hinge: the FORWARD
  vertical edge. Closed, the word hides them; the rig must show a closed pose
  flush with the fuselage (no source face to turn).
- **Gear.** 22 rigid plates: nose (6 faces: a plane at x 0 and two thin cross
  plates), each main strut and wheel plates (4 faces), each door wall (4 plus
  4 faces). Wheelbase from nose centre y 41 to main centre y -11 is 52 units
  (17.3 ft). No wheels or doors that fold.

The whole-surface gate (John, 2026-11-07 and 2026-11-08) is met by every
canard, elevon, the rudder, the airbrakes and the gear. **No diagonal or
partial surface was found**, so no probe sheet was made.

### Gripen engine and thrust

The record says `engines` = 1 and thrust 12,140 / 18,100 lbf, equal to the
real RM12 (54 kN dry, 80.5 kN with afterburner) and to the encyclopedia text.
The shape agrees: one octagonal nozzle at y -61 (x -2 to 3) and one flame group.
This is the first record of the batch where the count and the thrust are real
and need no TORE override. Consumers (`ai_wings`, `Engine::new`, wreck power,
contrail offsets) read the one engine; contrails need no special case.
`aftFuelConsumption` 14 is the Gripen's own value (the others 16).

### Gripen G envelope rows

Points are speed in ft/s at altitude in ft. Fourteen consecutive rows, `envMin`
-4 to `envMax` +9. The 1 G row peaks at 1,933 ft/s (1,145 kt, Mach 2.0) at 36,000 ft and
1,230 ft/s (729 kt) at sea level.

| G | Points |
| ---: | --- |
| -4 | 520@0, 615@11,000, 920@21,000, 1,230@21,000, 1,220@13,000, 1,010@0 |
| -3 | 420@0, 462@8,250, 575@16,500, 707@24,750, 890@33,000, 1,305@33,000, 1,480@26,000, 1,320@15,500, 1,090@0 |
| -2 | 330@0, 395@11,500, 520@23,000, 675@34,500, 870@46,000, 1,495@46,000, 1,650@30,000, 1,385@16,500, 1,160@0 |
| -1 | 240@0, 300@14,750, 410@29,500, 600@44,250, 870@57,000, 1,625@57,000, 1,827@47,000, 1,847@34,000, 1,488@18,000, 1,210@0 |
| 0 | 160@0, 180@13,000, 225@27,000, 313@39,000, 521@54,000, 870@63,000, 1,734@63,000, 2,027@49,250, 2,037@36,000, 1,613@18,000, 1,280@0 |
| 1 | 200@0, 235@13,000, 307@26,000, 423@40,000, 611@53,000, 870@60,000, 1,684@60,000, 1,923@48,250, 1,933@36,000, 1,541@18,000, 1,230@0 |
| 2 | same polygon as -1 |
| 3 | 280@0, 342@13,000, 465@26,000, 637@39,000, 870@52,000, 1,526@51,875, 1,712@44,750, 1,750@32,000, 1,435@17,500, 1,180@0 |
| 4 | same polygon as -2 |
| 5 | 380@0, 427@10,000, 555@20,000, 702@30,000, 870@40,000, 1,395@40,000, 1,560@28,000, 1,325@14,500, 1,130@0 |
| 6 | same polygon as -3 |
| 7 | 470@0, 530@9,500, 670@18,250, 900@26,000, 1,320@26,000, 1,245@12,500, 1,050@0 |
| 8 | same polygon as -4 |
| 9 | 580@0, 685@9,000, 950@16,000, 1,150@16,000, 1,080@6,500, 980@0 |

Rows -4 and 8, -3 and 6, -2 and 4, and -1 and 2 are the same polygon, as in
every reviewed record (source data). No row equals a Rafale, F/A-18 or Mirage
row. The 0 G row has a point at 63,000 ft, above `maxAlt` 60,000.

### Gripen HUD and cockpit (for John)

**Fact.** `GRIPEN.PT` names `gripen.HUD`, which resolves to its own
`GRIPEN.HUD` (4,608 bytes). It is the Rafale's HUD program with different panel
names (66 bytes differ): `~gri`, `~grih`, `~gris`, `~gri_l`, `~gri_c`, `~gri_r`,
`~gri_lh`, `~gri_ch`, `~gri_rh`, `~gri_ls`, `~gri_cs`, `~gri_rs`, `~gri_p` and the
Rafale's `~raf_w` window panel. **Gripen cockpit art exists**: all 13 pictures
are in `FA_1.LIB` (`~GRI.PIC` 535 x 205, `~GRIH.PIC` 1279 x 490, `~GRIS.PIC`
1601 x 613, `~GRI_L/C/R.PIC` 61 x 109 / 130 x 30 / 61 x 109, `~GRI_LH/CH/RH.PIC`,
`~GRI_LS/CS/RS.PIC`, `~GRI_P.PIC` 81 x 80), with their own dimensions, not the
Rafale's (`~RAF.PIC` 640 x 205, `~RAFH.PIC` 1280 x 490). The `ICONGRI.PIC` named
by `GRIPEN.PTS` was not found as a separate picture. The files `~AGRIPEN.M`,
`~B39.M`, `~E39.M`, `~U39.M` and `~V39.M` are named by no resource of this
aircraft and were not examined. Cockpit stem `GRI`, shape stem `GRI`.

Options (none chosen):

1. **Follow the record: `GRIPEN.HUD`, cockpit stem `GRI`, `~GRIH.PIC`.** What
   retail does, with real Gripen art; needs a new HUD family stem and the
   cockpit-stem arms (the 13 panels follow the Rafale panel structure).
2. **Bind the Rafale glass** (`RAFALE.HUD`, stem `RAF`) as the Mirage 2000 does:
   no new wiring, but wrong art when real art exists.
3. **Another reviewed HUD family.** No retail evidence.

Agent note: option 1 is the evidenced one and the first batch aircraft with
own cockpit art.

Update (GRI-7, 2026-11-10): John chose option 1 (decision 7, the batch decision
round); [the GRI-7 section](#gripen-sensors-hud-and-cockpit-gri-7-2026-11-10) wires
and measures it.

### Gripen AAS-38 pod row (for John)

**Fact.** Row 3 of the record is `AAS38.SEE`: location 1 (Fuselage), flags
`0x8` (the fixed-sensor flag), position [0, 0, 0], max items 1, max weight 0.
`AAS38.SEE` is a pod record: weight 350 lb, flag bit 1, no shape in the media
(the existing authored AAS-38 model, 6.0 x 1.08 ft, is TORE's). The Rafale
carries it on a real centreline station (flags `0x601`, [0, -20, 0]); the
Gripen's row is the **A-10 pattern**: flags `0x8` at the reference point,
inside the fuselage. Under the standing rule
([fixed pods draw only on a real station](../spec/stores-on-pylons.md#fixed-pods-draw-only-on-a-real-station-2026-11-07),
John, 2026-11-07) such a row **draws nothing**; the pod's mass and sensor
effect stay. The encyclopedia (`GRIPEN.INF`) says the FLIR pod is "carried
under starboard air intake trunk, forward of wing leading-edge": there is a
real FLIR mounting, but it is not a numbered store station in the record.

Options (none chosen):

1. **Follow the rule: no pod drawn.** Mass and sensor kept. Satisfies B7.
2. **Draw the pod under the starboard intake trunk** at a fitted spot (the
   encyclopedia names the place; the record does not), as a separate decision
   with frames.
3. **Treat it as an internal sensor with no mass.** Departs from the
   fixed-pod accounting.

Agent note: option 1 is the standing rule and requires no new decision.

Update (GRI-7, 2026-11-10): John chose option 1 (decision 8): no pod body is
drawn and the sensor stays (the AAS-38 infrared channel, below).

### Gripen hardpoints

All nine source rows in original order. Feet divide units by three; x right, y
up, z forward ([aircraft ordnance](aircraft-ordnance.md#hardpoint-positions)).
Location byte: 0 Centerline, 1 Fuselage, 2 Internal Gun, 3 Internal Bay,
4 Wing, 5 Wingtip. Mirror flag is bit `0x4`.

| Row | Location | Flags | Position x/y/z | Feet right / up / fwd | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `VIS340.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `F18R.SEE` | Fixed radar |
| 2 | Fuselage | `0x0008` | 0 / 0 / -50 | 0 / 0 / -16.7 | 1 | 0 | `F18.ECM` | Fixed jammer |
| 3 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `AAS38.SEE` | Fixed FLIR pod (placeholder, see above) |
| 4 | Internal Gun | `0x0008` | 4 / 2 / 37 | 1.3 / 0.7 / 12.3 | 570 | 0 | `BK27.JT` | Internal gun, 570 rounds |
| 5 | Fuselage | `0x07e1` | 0 / -20 / 0 | 0 / -6.7 / 0 | 1 | 40 | `F150.GAS` | Centreline tank |
| 6 | Wing | `0x07e5` | 15 / -15 / -14 | 5.0 / -5.0 / -4.7 | 2 | 40 | `AGM65G.JT` | A2G, inner wing pair |
| 7 | Wing | `0x07e5` | 30 / -15 / -21 | 10.0 / -5.0 / -7.0 | 4 | 40 | `AIM120.JT` | A2A, outer wing pair |
| 8 | Wingtip | `0x0085` | 44 / -2 / -26 | 14.7 / -0.7 / -8.7 | 2 | 4 | `AIM9M.JT` | A2A, wingtip pair |

Default stores in the catalog (`VIS340.SEE`, `F18R.SEE`, `F18.ECM`,
`AAS38.SEE`, `BK27.JT`, `F150.GAS`, `AGM65G.JT`, `AIM120.JT`, `AIM9M.JT`, one
default per row; per-row quantities are a host rule for GRI-6). Every default
is live in TORE except `BK27.JT`, which is decoded but hidden (the carriers
`ALPH`, `E2000`, `GRIPEN`, `TOR` and `VIG` are not imported yet); GRI-6 wires
the BK27 gun. The real aircraft has **seven** store stations (two wingtip, two
under each wing, one centreline; the encyclopedia) and the record has the same
seven: rows 5 to 8, mirrored. This is not a template row layout (the Rafale has
a different set); the record's tip row is the AIM-9 pair, the real aircraft's
standard fit. Things GRI-5 must handle:

- The Wingtip row (x 44, z -26) sits **3 units (1 ft) outboard of the shape's
  own tip rail** (x 41): measure it against the standard tip rule.
- The centreline tank (row 5, [0, -20, 0]) hangs 12 units (4 ft) below the
  shape's lowest belly point (z -8 at (0, 47)); a 150 gal tank on the standard
  pylon rules, with the pylon drop, is a likely close call with the nose gear
  doors and the main gear plates (z -16): measure.
- Rows 6 and 7 sit under the wing at x 15 and 30 (semi-span 41); row 7 has 4
  items per side (a host rule decides how many hang).
- The gun row is at x +4 (starboard side), while the encyclopedia puts the
  BK27 in the port side of the lower front fuselage. The sign may be the
  position convention for the gun port; GRI-6 records it.
- There is no designator pod station: the AAS-38 is the row-3 placeholder.

### Gripen roster note

`GRIPEN.PT` appends as the 42nd entry (checkpoint code 42 in the batch order
after `F15E`); `ALL` and `SELECTABLE` growth is the B5-1 slice's. The identity
is not aliased to any other record.

### Gripen unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| HUD and cockpit choice | GRI-7 cannot start without it | John, three options above |
| AAS-38 row (no pod, or under the intake) | B7 | John, three options above |
| Canard behaviour (pitch, brake bias, roll) | GRI-3 | John, three options above |
| Copied record values (ceiling, -4 G, 570 rounds) | Flight and gun | John, per value above |
| Elevon thinness, root slit | Visual | GRI-3 measures with the 25-pose grid |
| What the original does with `8098`/`809e` | They delete both strips | Unknown; the rig keeps the strips drawn |
| Closed airbrake pose | No source face for closed | GRI-4 |
| Wingtip row 1 ft outboard of the tip | Stations | GRI-5 measures |
| Gun side and 570 rounds | Gun row | GRI-5/GRI-6 |
| Radio callout absent | Chatter | None (no `^ACMGRI*`); GRI-9 notes |

## F-5E source review (2026-11-10)

Research mode, slice F5E-0. This reviews `F5E.PT` and its dependency closure
before `AircraftId::F5E` exists. `F5E.PT` is the 43rd retail identity to be
appended in the aircraft batch (checkpoint code 43, after `M2000`, the
F-15E and the Gripen; the batch identity slice B5-1 adds it). `F5EE.PT`
(desert) and `F5EV.PT` (snow) are separate records with their own shapes
(`f5ee.SH`, `f5ev.SH`) and are out of scope: no alias and no shared identity.
Both of those are parked targets today (`parked_aircraft.rs`: `F5EE.PT` gear
word `0x639c`, `F5EV.PT` `0x614c`); `F5E.PT` is not parked, and none of that
changes. No original module was executed. PT, SEE, ECM, JT and GAS are read as
bounded BRF data with the shared field order in `aircraft_schema.rs`; SH, HUD
and PIC are inspected as data. Slice done by Claude Sonnet (sonnet-medium).
Nothing here changes code or behaviour.

Open for John, **not decided here**: the HUD and cockpit
([F-5E HUD](#f-5e-hud-and-cockpit-for-john)), the control surfaces
([F-5E surfaces](#f-5e-control-surfaces-for-john)), the record
([F-5E record](#f-5e-record-against-the-real-aircraft-for-john)), the
stations and default load ([F-5E hardpoints](#f-5e-hardpoints)) and the size
([F-5E size](#f-5e-size-against-the-real-aircraft-for-john)).

### F-5E build and extraction

`FA_1.LIB` and `FA_2.LIB` are the same pair as the MiG-31 review (see the
[build section](#build-and-extraction)). The closure was extracted locally to
`.local/m3-imports/scratch/f5e` with `cargo run --locked -p tore-extract --
--source gameassets/fighters-anthology --out .local/m3-imports/scratch/f5e
--include 'F5E*' --include 'F29*' --include '~F5*' --include '*F5*' ...`. A name
sweep (`*TIGER*`, `*NORTH*`, `~F5*`, `*F5E*`, `*F-5*`, `*F20*`, `*F29*`,
`*ACF5*`, `*BARF5*`) went to `.../names`, and every `*.PT` of `FA_2.LIB` to
`.../allpt` for the template comparison. The `SETUP.ESA` installer copies are
present for all of these. The usual five unrelated archive errors appear
(`LHX*.LIB` compression flags and `_SETUP.LIB`).

### F-5E identity and record

| Field | Decoded value |
| --- | --- |
| PT | `F5E.PT`, 13,489 bytes, SHA-256 `7f0026d5059b1cc037be856d784dcbc7d9d4cca6c5f634994c1731ce105fbae2` |
| Displayed names | `F-5`, `F-  5E Tiger`, `F5E.PT` |
| Schema | OBJECT + NPC + PLANE, 219 statements; `structType` 5; **`typeSize` 612, `instanceSize` 592** (not the parser default 660/626: F5E-1 needs a `612` arm next to `A4E`, `Mig21`, `Ac130`, `F4B`, `A7` in `aircraft.rs`) |
| Object class / year / hit points | `0x8000` (fighter) / 1973 / 75; `expType` 30 |
| PLANE flags | `0x11`: seat and base bit; hook, no-lift, carrier and multi-crew clear |
| Shape | `f5e.SH` (25,088 bytes), shadow `f5e_s.SH` (12 faces) |
| Damage looks | `F5E_A.SH` 228 faces, `F5E_B.SH` 128, `F5E_C.SH` 234, `F5E_D.SH` 24 |
| Skins | `_F5E.PIC` (81,704 bytes), `_F5E_A.PIC` 81,964, `_F5E_B.PIC` 68,184, `_F5E_C.PIC` 96,524, `_F5E_D.PIC` 42,184 |
| Sounds | `&JET1N.11K` loop, `&JET1A.11K` second, `&powerup.5k`, `&powerdn.5k` |
| HUD | `f29.HUD` (resolves to the existing `F29.HUD`, the X-29's) |
| AI script | `f.BI` |
| Sensors / jammer / gun | `VIS340.SEE`, `F18R.SEE`, `F18.ECM`, `M61.JT` (all reviewed) |
| PTS | none: no `F5E.PTS` exists (`F29.PTS` does) |
| Radio callouts | `^ACF5E.5K` (2,865 bytes), `^TIGER.5K` (2,329), `^BARF5.5K` (8,865) |
| Encyclopedia | `F5E.INF` and `F5E_0/1/2/C.PIC` in `disc2/FA_3.LIB`; reference media, not flight data |
| Parked / CLI | not parked; checkpoint code 43 and key `f5e`/`f5e.pt` reserved for B5-1 |

The decoded record equals the catalog row in [`fa-aircraft.csv`](fa-aircraft.csv)
field for field. Hashes of the files later slices depend on:

| Resource | Bytes | SHA-256 |
| --- | ---: | --- |
| `F5E.PT` | 13,489 | `7f0026d5059b1cc037be856d784dcbc7d9d4cca6c5f634994c1731ce105fbae2` |
| `F5E.SH` | 25,088 | `3c7cbd3e403d95e4dd52a7669caed2999509377a75b4077b9cad0e115bd0845f` |
| `F5E_S.SH` | 4,608 | `4c73623088802890f0655229984d463a1c85023fa2476d05cabcac30507da412` |
| `F5E_A.SH` | 16,896 | `4bf3099db775b99e19e210aaa889eb4ac20e36291c4952f6145e916f42c75b1c` |
| `F5E_B.SH` | 8,704 | `660312074e9281b5c5749aa21069952abaaa3eab8ec4136bd3f9fa788613c15f` |
| `F5E_C.SH` | 20,992 | `6839fa53462ebabfe64091b0fae46f0e1c21ed657de41071b15638e7406f71b1` |
| `F5E_D.SH` | 4,608 | `36ee7a9871313c6ce063521b68993a8a1e779228a427746f23bcc881b8db41df` |
| `F29.HUD` | 4,608 | `1b6591c70dc7b166b7515f7014bd10c2a614184399a7003431bb99b9b900ef3c` |
| `_F5E.PIC` | 81,704 | `62e8ee3a779683533996d19a337c5c491109573de86fce70ad154f14f2c2f44f` |

Identity facts for the desert and snow records (for B6, no alias): against
`F5E.PT`, `F5EE.PT` and `F5EV.PT` differ in exactly 3 statements (`year`
1974, `gearDrag` 38 against 23, `OBJECT.flags` `0x8002bf3` against `0x2bf3`)
plus names and shapes; the envelope table and the station rows are identical.
`_F5EE.PIC` and all four `_F5EE_A..D.PIC` damage skins are **byte-identical**
to the F-5E's; the snow skins differ. The F-5E, desert and snow shapes have
the same 253 faces and six words at different addresses (F-5E `63c0 63c6 63cc
63d8 63de 63e4`, desert `6390 .. 63b4`, snow `6140 .. 6164`).

### F-5E record against other records (template copies)

No single record is the template. The closest by shared statements are
`F5EE`/`F5EV` (216 of 219), then `M2000` (191), `MTF1` (187) and `E2000`
(186): those overlaps are the generic fighter defaults (sounds, Doppler, crash
speeds, spin tables, AI timers). What the F-5E record has of its own, shared
with no other record in `FA_2.LIB`: year, empty weight, thrust (7,000 / 10,000),
internal fuel, maximum takeoff weight, `structure[1]` and `turbulencePercent`,
the whole 13-row G envelope table and the hardpoint block. Values that look
copied, with cited real F-5E figures (the game's own `F5E.INF`, Jane's-style,
numbers only, and Wikipedia "Northrop F-5", Specifications (F-5E Tiger II),
accessed 2026-11-10):

| Value in the record | Where else it appears | Real F-5E | Verdict |
| --- | --- | --- | --- |
| Ceiling 42,000 ft (`maxAlt`) | 13 records, all of the F-4 family and the A-6, A-7, F-105, F-117 | 51,800 ft (15,800 m) service ceiling; `F5E.INF` 15,790 m | **Copy; 19% low** |
| Sea-level 142 / 723 kt | `F4`, `F4B`, `F4E`, `F4J`, `BUC` (the F-4 family) | not published | **Copy of the F-4 family** |
| `structure[0]` 1,218 | `F4`, `F4B`, `F4E`, `F4J`, `BUC` | n/a | Copy of the F-4 family |
| Envelope rows 8, 6, 4, 2 | rows -4, -3, -2, -1 of the same record | n/a | Mirror copies inside the record (the +8 row equals the -4 row) |
| G -4 / +8 | `envMax` +8 on 17 records | about +7.33 G (commonly cited limit; not fetched this pass) | Probably template; +8 is 9% over |
| Internal fuel 6,240 lb | no other record | 677 US gal = about 4,500 lb (2,563 L, `F5E.INF`); `F5E.INF` also prints 4,400 lb and 5,324 lb | **38% over**, origin unknown |
| Radar `F18R.SEE`, jammer `F18.ECM` | 40 records carry the pair | AN/APQ-159 (20 nm, `F5E.INF`); no internal jammer | Generic fighter pair, not F-5E equipment |
| Default `AIM7.JT` on the wing row | F-4 family, F-104 | Sparrow is not on the F-5E weapon list (Wikipedia: AIM-9 wingtips; AIM-120 only on upgraded F-5S) | **Not real** |
| Gun `M61.JT`, 560 rounds | many fighters | two M39A2 20 mm, 280 rounds per gun (`F5E.INF`): 560 rounds matches exactly; M61 is the Vulcan | Count real, gun type generic |
| `f29.HUD` | `F29.PT` (the X-29) by convention | no F-5E panel | The X-29's panel, see below |
| Sounds `&JET1N.11K` / `&JET1A.11K` | many | n/a | Generic |

Real-aircraft comparison for the rest of the record:

| Field | Record | Published | Difference | Verdict |
| --- | ---: | ---: | ---: | --- |
| Engines | 2 | 2 (J85-GE-21B) | 0 | **Agrees** (shape: two nozzles at x +/-3, flame word with two lateral plates) |
| Military thrust, lbf (both) | 7,000 | 7,000 (2 x 3,500, Wikipedia; later text gives 3,600) | 0.0% | **Agrees, not a template** |
| Afterburner thrust, lbf (both) | 10,000 | 10,000 (2 x 5,000, `F5E.INF` and Wikipedia) | 0.0% | **Agrees, not a template** |
| Empty weight, lb | 9,558 | 9,583 (Wikipedia); 9,723 (`F5E.INF`) | -0.3% / -1.7% | Agrees |
| Maximum takeoff, lb | 24,664 | 24,675 (Wikipedia); 24,722 (`F5E.INF`) | -0.04% / -0.2% | Agrees |
| Top speed, kt | 1,090 | 1,092 (`F5E.INF`) | -0.2% | Agrees |
| Ceiling, ft | 42,000 | 51,800 | -19% | Copy, see above |
| Internal fuel, lb | 6,240 | about 4,500 | +38% | Origin unknown |
| Hit points | 75 | n/a | | Shared with `HWK.PT`, `STRK.PT` only |

Agent audit decision, 2026-11-10: unlike the Mirage 2000, the thrust, engine
count, empty weight and takeoff weight are the real F-5E's, so nothing here is
a John thrust or engine question. The record facts that look copied are the
ceiling, the sea-level speeds, the G ceiling, the fuel, the radar and jammer
pair and the Sparrow default; all stay retail until John decides, and F5E-8
reports the envelope against the real aircraft.

### F-5E resources

| Resource | Role | Note |
| --- | --- | --- |
| `F5E.SH` | Shape | 253 runtime faces, 0 lines, 0 billboards, six state words |
| `F5E_S.SH` | Shadow | 12 faces |
| `F5E_A.SH` / `F5E_C.SH` | Damage bodies | 228 / 234 faces |
| `F5E_B.SH` / `F5E_D.SH` | Damage pieces | 128 / 24 faces |
| `VIS340.SEE`, `F18R.SEE`, `F18.ECM`, `M61.JT` | Sensors, jammer, gun | already reviewed; live |
| `F150.GAS`, `AIM7.JT`, `AIM9B.JT` | Default stores | reviewed; live in TORE for other carriers (F-4 family, F-4B) |

The shape streamer has pivot 0 and hinge scale 0 (no streamer). The damage
looks carry no state words.

### F-5E HUD and cockpit (for John)

**Fact.** `F5E.PT` names `f29.HUD`. That resolves to the existing `F29.HUD` in
`FA_2.LIB`, the **X-29's** HUD (`F29.PT`, "X-29 FSW", has a null `hudName` and
TORE maps by convention; only `F5E`, `F5EE` and `F5EV` name `f29.HUD`
explicitly). It carries the panel names `~f29`, `~f29h`, `~f29s`, `~f29_l`,
`~f29_c`, `~f29_r`, `~f29_lh`, `~f29_ch`, `~f29_rh`, `~f29_ls`, `~f29_cs`,
`~f29_rs`, `~f29_p`, `~f29_w` and the GEAR, FLAP, BRAKE, HOOK lamps. The cockpit
family is `~F29*.PIC` in `FA_1.LIB` (13 pictures: `~F29` 640 x 205, `~F29H`
1280 x 490, `~F29S` 1635 x 613, the L/C/R panels and `~F29_P`). The game's own
encyclopedia says the X-29s were built from "two F-5E fuselages"
(`F5E.INF`), a plausible reason the X-29 cockpit was reused. **No F-5E
cockpit art exists anywhere in the media**: the sweeps `~F5*`, `*F5E*`, `*F-5*`,
`*TIGER*` and `*NORTH*` over every archive find only the `F5E*` shapes and
skins, the `^ACF5E`, `^TIGER`, `^NORTH`, `^TONORTH` and `^BARF5` callouts and
the `F5E_*` encyclopedia pictures. TORE has no X-29 aircraft, so no existing
stem table entry covers `F29`.

Options (none chosen):

1. **Follow the record: `F29.HUD`, cockpit stem `F29`, `~F29H.PIC`.** What
   retail does; no new art. Needs one exhaustive stem arm and aperture
   measurement (F5E-7), like the Mirage's Rafale panel. The player sees the
   X-29 cockpit in an F-5E. Sharing a panel is not aliasing the record.
2. **Substitute another reviewed HUD family** (for example the F-4 or F-16
   panel). Opinionated, no retail evidence, extra wiring.
3. **Author an F-5E panel.** Invented art; only as a later, separate decision.

Agent note: option 1 is the evidenced one.

Update (F5E-7, 2026-11-10): John chose option 1 in the batch decision round
("follow the record", opinionated), and the F-5E now measures and uses the family:
see [F-5E sensors, HUD and cockpit](#f-5e-sensors-hud-and-cockpit-f5e-7-2026-11-10).

### F-5E control surfaces (for John)

**Fact.** Six state words, counts from `Shape::with_state` (gear down extends
the low point). All six are in the base slot order of the Mirage and MiG-31
families. Neutral extents: x -44 to 44, y -48 to 87, z -5 to 24.

| Word | Faces | Geometry (shape units, x right, y forward, z up) | Reading |
| --- | ---: | --- | --- |
| `63c0` | +8 | Four vertical plates (x +/-3, 6 units tall) and four horizontal plates (x -6 to 5, z 0), y -49 to -79 | Afterburner flame, two lateral plumes |
| `63c6` | +2 | One quad x +/-2, front edge y 29 z 10, rear edge y 22 z 16, two windings | Speed brake: one dorsal centreline plate |
| `63cc` | +6 | Three flat plates: mains at x +14 and x -15 (y -2 to 6, z -18 to -4), nose at x 0 (y 53 to 60, z -17 to -5) | Landing gear down |
| `63d8` | -2 | Removes the left trailing-edge strip (x -7 to -31, y -4 to -11) | Left flap |
| `63de` | -2 | Removes the right strip (x 7 to 31) | Right flap |
| `63e4` | 0 net | Swaps the rudder quad for a copy 2 to 3 units to starboard | Rudder (one direction) |

There is **no slat word, no canopy word, no hook word and no nozzle word**.
The real F-5E has full-span automatic leading-edge flaps and an
upward-opening canopy (`F5E.INF`); the source draws neither and none should be
invented. The gear is three flat plates with no doors,
wheels or struts: wheelbase about 54 units (18.2 ft), main track 29 units
(9.7 ft).

**State map for the rig:**

| Moving part | Source panel | State word | Rig note |
| --- | --- | --- | --- |
| Stabilator, each side | Flat plate z -4: root x 5 (y -23 to -37), tip x 20 (y -30 to -35), 2 faces (both windings); shares no vertex with any other face | none | Whole panel about a lateral spindle |
| Aileron, each side | Strip x 31 to 43, hinge y -5, trailing y -8 to -7, 2 faces | none | Hinge on the wing trailing edge |
| Trailing-edge flap, each side | Strip x 7 to 31, hinge y -4 to -5, trailing y -11 to -8, 2 faces | `63d8`, `63de` remove it | The rig keeps the strip drawn and turns it |
| Rudder | Quad on the single fin, hinge (0,-31,4) to (0,-29,22) | `63e4` | Hinge on the fin trailing edge |
| Speed brake | Dorsal plate, 2 faces | `63c6` | Appears only when open |
| Gear | 3 plates, 6 faces | `63cc` | Rigid plates, hinge at top edge |
| Burner | 8 plates | `63c0` | Additive plume, no hinge |
| Leading-edge flaps, canopy | none | none | Not in the source |

**Rule 11 face table (from the neutral dump, `fold5.py`).** Wing hinge line
(top layer z -3, bottom layer z -4): flap from (7,-4) to (31,-5), aileron from
(31,-5) to (43,-5). Hinge vertices sit on the line; trailing vertices move.

| Surface | Face | Vertices | Moved | On hinge line | Fixed |
| --- | --- | ---: | ---: | ---: | ---: |
| Right stabilator | `3a9d` | 4 | 4 (turned whole) | 0 | 0 |
| Right stabilator | `3b8c` | 4 | 4 | 0 | 0 |
| Left stabilator | `3f2c`, `40eb` | 4 each | 4 | 0 | 0 |
| Right flap (x 7 to 31) | `44d9`, `44f8` | 4 each | 2 | 2 | 0 |
| Left flap (x -7 to -31) | `45c0`, `45df` | 4 each | 2 | 2 | 0 |
| Right aileron (x 31 to 43) | `3b45`, `3b9f` | 4 each | 2 | 2 | 0 |
| Left aileron (x -31 to -43) | `4067`, `41ce` | 4 each | 2 | 2 | 0 |
| Rudder | `4755`, `4774` (neutral); `469b`, `46ba` (word) | 4 each | 2 | 2 | 0 |
| Speed brake | `4802`, `4819` | 4 each | 2 | 2 | 0 |
| Gear (each of 6 faces) | `49cc` ... `4ac1` | 4 | 2 | 2 | 0 |

Verdicts:

- **Every moving surface is a whole panel in the source.** No diagonal or
  partial surface was found, so no probe sheet is needed, and no face has a
  fixed vertex off its hinge line. The whole-surface gate (John, 2026-11-07 and
  2026-11-08) is met by all of them.
- **Stabilators.** Each is a flat 4-vertex plate (two coincident faces, zero
  thickness) that shares no vertex with the fuselage, so turning it whole about
  a lateral spindle through the root-chord midpoint, (+/-5, -30, -4), folds
  nothing and opens no crack beyond the root gap the retail geometry already
  has (the root is a free edge at x +/-5, the fuselage side). Span 40 units =
  13.3 ft against the real tailplane span 14.1 ft (4.31 m, `F5E.INF`).
  Pitch only: the encyclopedia says tailplane incidence is "varied by hydraulic
  actuators ... for control of rudder and tailplane" and gives the F-5E ailerons
  as the roll surface; I found no source that the real F-5E tail rolls
  differentially (the roll-by-aileron arrangement is general knowledge, not
  cited here beyond that text). **Option for John:** (a) pitch only, the real
  F-5E and the evidence; (b) pitch plus a small differential roll term as the
  fleet's tailerons (opinionated, not the real aircraft).
- **Ailerons and flaps.** Thin wedge strips, chord 3 units (1 ft) at the
  aileron, 7 units (2.3 ft) at the flap root narrowing to 3. The flap hinge
  line is not parallel to the aileron's (y -4 to -5 against y -5 flat), a
  fraction of a degree. Together they take the whole trailing edge from x 7 to
  43. Both layers' hinge vertices are 1 unit apart in z, the same wedge gap as
  the Mirage (0.17 units at 10 degrees). Flap and aileron share the seam
  vertices at x +/-31, so they must not take opposite angles at the seam
  without a visible step; they are separate surfaces with different jobs
  (flap: symmetric; aileron: differential). The flap root edge (x 7) is shared
  with 1-unit closure strips (`3161`, `317d`; `23c0`, `38b2` on the left) that
  stay fixed; turning the flap opens a gap up to `sin(angle)` x 7 units there.
  The flap words delete the strip with no replacement, so the rig must not
  hide it.
- **Rudder.** One hinged quad per winding; its leading edge is shared with
  fixed fin faces (`3cd7`, `3d86`), its top trailing vertex with `3cf2`,
  `3da1`. The retail swap is a roughly 40 degree one-way offset rounded to
  units, not an exact rotation (the same as the Mirage).
- **Speed brake.** The source plate is a single 4-unit-wide dorsal plate on the
  spine, hinged on its forward edge. The real F-5E has paired brakes under the
  fuselage (general knowledge, not cited this pass). It is the source's brake;
  options are to rig exactly that, or to leave the brake unrigged.
- **Leading-edge flaps.** The real aircraft has full-span automatic
  leading-edge flaps (`F5E.INF`); the shape has no separate geometry or word.
  Option: leave unrigged, as for the Mirage's slats.
- **Wingtip rails.** The shape draws a small rail wedge at each tip (x 43 to 44,
  y -9 to 4, 8 faces a side), in the wing skin; the hardpoint row 6 sits at its
  x 43 (see below).

### F-5E hardpoints

All seven source rows, original order. Positions are source units; feet divide
by three; x right, y up, z forward. Location byte: 0 Centerline, 1 Fuselage,
2 Internal Gun, 4 Wing, 5 Wingtip. Flag `0x4` is the mirror (pair) flag.

| Row | Location | Flags | Position x/y/z | Feet right / up / fwd | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `VIS340.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `F18R.SEE` | Fixed radar |
| 2 | Fuselage | `0x0008` | 0 / 0 / -50 | 0 / 0 / -16.7 | 1 | 0 | `F18.ECM` | Fixed jammer |
| 3 | Internal Gun | `0x000c` | 4 / -5 / 41 | 1.3 / -1.7 / 13.7 | 560 | 0 | `M61.JT` | Internal gun, 560 rounds |
| 4 | Centerline | `0x0605` | 0 / -14 / 0 | 0 / -4.7 / 0 | 1 | 11 | `F150.GAS` | 150 gal tank |
| 5 | Wing | `0x07e5` | 22 / -14 / 3 | 7.3 / -4.7 / 1.0 | 4 | 40 | `AIM7.JT` | A2A and stores, wing pair |
| 6 | Wingtip | `0x0085` | 43 / -3 / -3 | 14.3 / -1.0 / -1.0 | 2 | 4 | `AIM9B.JT` | Wingtip rail pair |

**Seven rows, three weapon rows.** The record has one centreline station, one
wing pair (two stores a side) and the wingtip pair. The real F-5E has seven
external stations (Wikipedia: 7 hardpoints; the layout of two pylons a side, one
centreline and two wingtip rails is general knowledge, not cited this pass), so the source gives the centreline and
the wingtips and one of the two wing stations per side, and no second wing
pair. Row 5's x 22 (7.3 ft) is the inboard wing region; the outer pylon has no
row. **Wingtip rails are real on the F-5E**, the shape draws them, and row 6
sits on the rail (x 43), so row 6 stays under B7. The default `AIM7.JT` on the
wing pair is a Sparrow, which the real F-5E does not carry; the F-5E's own
missile is the Sidewinder, which row 6 supplies (`AIM9B.JT` x2 as default). The
gun row is the real 560 rounds (2 x 280). There is **no sensor or designator
pod row** and none may be hung (John, 2026-11-07).

Things F5E-5 must handle: the centreline tank (row 4, y -14 = 4.7 ft below the
reference) with the shape's underside at z -5; the wing pair at x 22 against a
wing of semi-span 43; the wingtip pair on the rail (the tip rule on a 1-unit
rail); the second wing pair is a John question if a real-station layout is
wanted (option: leave as one pair, or add pylons at the real outboard station
as an opinionated change, with a cited position).

Update (F5E-5, 2026-11-10): the stations are measured, see [F-5E stations measured](#f-5e-stations-measured-f5e-5-2026-11-10) and [stores on pylons](../spec/stores-on-pylons.md#the-f-5e-tiger-f5e-5-2026-11-10). John kept the one wing pair (2026-11-10); B5-2 emptied its default.

### F-5E size against the real aircraft (for John)

Measured on `F5E.SH` at 1 unit = 1/3 ft, gear down (neutral extents plus the
`63cc` plates), nose probe included:

| Axis | Shape | Real | Shape / real |
| --- | ---: | ---: | ---: |
| Span, wing tips | 86 units = 28.7 ft | 26.67 ft (8.13 m) | +7.5% |
| Span, over the rail wedges | 88 units = 29.3 ft | 26.67 ft clean; 27.99 ft with tip missiles (8.53 m) | +10.0% / +4.8% |
| Length, nose probe tip to nozzle ring | 135 units = 45.0 ft | 47.33 ft (14.45 m, `F5E.INF`); 48.19 ft (14.69 m, Wikipedia) | -4.9% / -6.6% |
| Length, fuselage without probe | 123 units = 41.0 ft | n/a | |
| Height, gear down to fin top | 42 units = 14.0 ft (z -18 to 24) | 13.35 ft (4.07 m); 13.38 ft (4.08 m, Wikipedia) | +4.9% |
| Tailplane span | 40 units = 13.3 ft | 14.1 ft (4.31 m) | -5.7% |

**Verdict: the shape is close to true size** (every axis within 10%, against
the Mirage's -26% / -19% / -45%). Options (none chosen): (a) **no scaling**,
retail size (smallest change, no per-aircraft scale entry); (b) per-axis scale
to the real aircraft as the Mirage decision did, about x 0.91 (to the 26.67 ft
clean span) or 0.95 (to the 27.99 ft span over missiles), y 1.05, z 0.95,
fitted from these extents in F5E-2; (c) a uniform 1.0 that leaves length
alone. Agent note: with wingtip missiles the span is within 5%, so (a) is the
evidenced default.

### F-5E roster note

`F5E.PT` appends as checkpoint code 43 in the batch order (F-15E 41, Gripen 42,
F-5E 43, Tornado 44, Mirage F1 45). The wire identity is 9 bits
([NET-ID9](#wire-identity-widened-to-9-bits-net-id9-2026-11-10)), so capacity is
not a constraint. `F5EE.PT` and `F5EV.PT` are not added.

### F-5E unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| HUD and cockpit choice (X-29 panel) | F5E-7 cannot start without it | John, three options above |
| Stabilator pitch only or with roll | F5E-3 rig | John, two options above |
| Ceiling, G, fuel, radar, Sparrow default | Record corrections | John; F5E-8 reports the envelope |
| Real-size scaling | F5E-2 | John, options above |
| Second wing pair and outboard pylon | F5E-5 | John, options in hardpoints |
| `typeSize` 612 | `aircraft.rs` size arm | F5E-1 |
| Speed brake as one dorsal plate | Real brakes are paired and ventral | F5E-4 rigs the source panel only |
| What the original does with `63d8`/`63de` | They delete the flap strip | Unknown; the rig keeps the strip drawn |
| Leading-edge flaps | Real, not in the source | None drawn |

### F-5E shape, skin and damage looks (F5E-2, 2026-11-10)

Implementation, slice F5E-2, mirroring the Mirage 2000's M2K-2 **without a
scale**: the size question (keep 1:1 or fit per axis) is still John's, so
`aircraft_scale.rs` keeps `[1, 1, 1]` for `F5E` and the shape draws at retail
size (the "no scaling" option above). Everything else in the slice is
independent of that answer. The skin, twin and decal review is in the
objects and shapes guide, [F-5E skin
review](objects-and-shapes.md#f-5e-skin-review-2026-11-10); the
`render-f5e-skin` battery scenario captures the top and under views with the
gear down.

| Check | Result |
| --- | --- |
| Shape | `F5E.SH` loads: 253 runtime faces (259 gear down), six state words, extents x -44 to 44, y -48 to 87, z -5 to 24 (gear down z -18 to 24) |
| Own art | Every face reads `_F5E.PIC`; the damage looks read `_F5E_A..D.PIC`. Lookup is by exact name (`F5E.SH`, `_F5E.PIC`), so the importer's `_F5E` prefix match, which also carries the desert `_F5EE*` skins (byte-identical to the F-5E's) and the snow `_F5EV*` skins, never feeds the F-5E (the desert and snow shapes are not carried at all). A retail-reading test pins it: `F5EE.SH` and `F5EV.SH` differ from `F5E.SH`, and no face names a `_F5EE` picture |
| Twin faces | 8 exact-vertex twin groups, every one a pair with opposite stored normals: three windshield and canopy plates on the centreline (`2a01`/`2a7d`, `2a34`/`2a4c`, `2aad`/`2ac5`), the two stabilators (`3a9d`/`3b8c`, `3f2c`/`40eb`), two fin plates (`3cd7`/`3d86`, `3cf2`/`3da1`) and the rudder (`4755`/`4774`). No MiG-21 normal exception is needed. F5E-3 turns each stabilator and the rudder as a twin pair |
| Decals | Two `E0` slots on the wings, both omitted (see the skin review) |
| Damage looks | `F5E_A` 228 faces, `F5E_B` 128, `F5E_C` 234, `F5E_D` 24 (each its own mesh, none a subset of the intact shape). `A` is the body with the nose cut away at y 35 (wings clipped to x +/-36, fin to z 18) and `B` is the nose piece (x +/-4, y -19 to 26); `C` is the body with the fin cut to z 13 and the nose tip cut at y 63 (wings x -35 to 36) and `D` is a flat fin plate on x 0 (y -9 to 8, z -7 to 12) |
| Damage pairing (agent decision) | `debris::damage_variant` pairs Nose and Cockpit with `A`/`B` and Tail with `C`/`D`; the wings and core keep the fitted regional tears. A destroyed F-5E hit in the nose or tail therefore draws the original broken body and sheds the original piece, like the F/A-18 and the Rafale. Frames below; revert is the three arms in `debris.rs` |

Frames (`.local/m3-aircraft/frames/F5E-2/`): the scenario pair, top, side,
three-quarter and under views, 3x crops, and damage at 0.6 and 1.0 (1.0 draws the `A` body; the `C` tail body
is `f5e-damage-1.0-tail-variant`). The shape reads as an F-5E (single fin,
low straight-edged wing, long pointed nose, twin nozzles) with no clipping, no
underside through the top and no z-fighting.

Provenance: shape and skin spec-derived (retail data read as data); the
damage pairing and the omitted decals agent decisions (2026-11-10); size
pending John (no scale).

### F-5E stations measured (F5E-5, 2026-11-10)

The default load and every alternative the record's masks offer, measured on the
1:1 shape under the standard pylon rules (full tables, clipping and options in
[stores on pylons](../spec/stores-on-pylons.md#the-f-5e-tiger-f5e-5-2026-11-10)):

| Row | Position, ft right / up / fwd | Default | Hang | Result |
| ---: | --- | --- | --- | --- |
| 4 | 0 / -4.67 / 0 | `F150.GAS` x1 | 2 inch rail, belly -1.67 | clean (0.17 ft off the belly, 2.58 ft off the ground) |
| 5 | 7.33 / -4.67 / 1.00 | empty (B5-2) | 1 ft twin-rail pylon, wing underside -1.33 | defaults clean; of 64 alternatives 8 enter the wing (0.07 to 0.23 ft), 13 cross a neighbour on the pylon, 4 touch the gear sheet (`AIM7`, `AIM7E`, `GBU16`, `GBU30`) |
| 6 | 14.33 / -1.00 / -1.00 | `AIM9B.JT` x2 | tip rule on the 0.33 ft thick tip | **0.32 ft entry, both sides** (stop, options for John) |

Rows 0 to 3 (sensor, radar, empty jammer, gun) draw nothing. No pod was added
(B7). F5E-5 is `[~]`: the default tip AIM-9B clips 0.32 ft (inside the 0.27 to
0.72 ft range of the default loads reviewed so far) and several Load Ordnance
alternatives clip, so the decision is John's. At full flap (0.40 rad) the drooped
flap crosses `R550`, `AGM45`, `GBU12` and `AA6` on the wing row (not the AIM-9B,
0.14 ft clear). Nothing touches the ground in the default load. Provenance:
measurement and comparison agent findings (2026-11-10); no Rust, pin or count
changed.

## Tornado source review (2026-11-10)

Research mode, slice TOR-0. This reviews `TOR.PT` (Tornado IDS) and its
dependency closure before `AircraftId::Tornado` exists. `TOR.PT` is the 44th
retail identity of the batch-5 plan (checkpoint code 44, appended by slice
B5-1). It is not a parked target (no `parked_aircraft.rs` entry). No original
module was executed. PT, SEE, ECM, JT and GAS are read as bounded BRF data with
the shared field order in `aircraft_schema.rs`; SH and HUD are inspected as
data. Slice done by Claude Sonnet (sonnet-medium). Nothing here changes code or
behaviour. Real-aircraft figures are cited and never establish game behaviour.

Questions open for John, **not decided here**: the record template values
([below](#tornado-the-template-copy-list-for-john)), the real-size scale
([below](#tornado-real-size-for-john)), the variable sweep
([below](#tornado-variable-sweep-for-john)), the surfaces that fail or strain the
whole-surface gate ([below](#tornado-surfaces-for-john)), the HUD and cockpit
([below](#tornado-hud-and-cockpit-for-john)), the sensor, jammer and pod rows
([below](#tornado-sensors-jammer-and-pod-for-john)) and the stations
([below](#tornado-hardpoints)).

### Tornado build and extraction

`FA_1.LIB` and `FA_2.LIB` are the same pair as the MiG-31 review (see the
[MiG-31 build section](#build-and-extraction)). The closure was extracted
locally to `.local/m3-imports/scratch/tornado` with `tore-extract --include
'TOR*' --include 'E2000.HUD' --include 'SU24.PT' ...` and name sweeps (`~TOR*`,
`*TORN*`, `_TOR*`, `^*TOR*`, `*GR1*`, `*GR4*`, `*E2000*`, `~EF2*`, `*IDS*`).
The `SETUP.ESA` installer copy of `FA_2.LIB` holds byte-identical `TOR.PT`,
`TOR*.SH`, `_TOR*.PIC`, `F.BI` and ordnance; only `E2000.HUD` differs (71 bytes,
a build difference). `swpatch.lib` (the toolkit patch archive, excluded from
base-FA research) holds a different `TOR.SH` (33,280 bytes, CODE 28,198 against
28,150, state words `7df0` to `7e14`, same 287 faces) and a larger `_TOR.PIC`;
this review reads the base `FA_2.LIB` shape only. The usual five unrelated
archive errors appear.

### Tornado identity and record

| Field | Decoded value |
| --- | --- |
| PT | `TOR.PT`, 12,201 bytes, SHA-256 `39f9c677de59267bfcb87634a944433f7d659d47c1a850e9e699c1eb2d3f99fe` |
| Displayed names | `Tornado`, `Tornado IDS`, `TOR.PT` |
| Schema | OBJECT + NPC + PLANE, 219 statements; `structType` 5; `typeSize` 660; `instanceSize` 626 (parser default) |
| Object class / year / hit points | `0x4000` (attack) / 1990 / 156; `expType` 30 |
| PLANE flags | `0x15`: base `0x01`, multi-crew `0x04` (two seats), seat `0x10`; hook, no-lift and carrier clear |
| Shape | `tor.SH` (33,280 bytes, scale exponent giving header scale 2), shadow `tor_s.SH` (14 faces) |
| Damage looks | `TOR_A.SH` 290 faces, `TOR_B.SH` 36, `TOR_C.SH` 314, `TOR_D.SH` 37 |
| Skins | `_TOR.PIC` 150,604 bytes, `_TOR_A/B/C/D.PIC` 141,764 / 42,704 / 132,664 / 91,584 |
| Sounds | `&jet09.11k` loop and second sound, `&powerup.5k`, `&powerdn.5k` (all present) |
| HUD | `e2000.HUD` (the Eurofighter 2000 HUD) |
| AI script | `f.BI` (12,800 bytes, present) |
| Sensors / jammer / gun | `VIS240.SEE`, `F18R.SEE` (APG-65), `MIG31.ECM`, `BK27.JT`; `PAVESPK.SEE` as a fixed row |
| PTS | none: no `TOR.PTS` exists (`E2000.PTS` exists) |
| Radio callout | `^ACTOR.5K` exists (4,328 bytes) |
| Encyclopedia | `TOR.INF`, `TOR_0.PIC`, `TOR_1.PIC` in `disc2/FA_3.LIB` (Jane's text, numbers cited below as reference media) |
| Checkpoint code / CLI key | 44 and `tornado`/`tor.pt`, reserved for B5-1 |

The decoded record equals the catalog row in [`fa-aircraft.csv`](fa-aircraft.csv)
field for field (1990, `0x4000`, 2 engines, 41,885 / 11,221 / 90,390 lb, 35,270
/ 50,700 lbf, 1,369 kt top, 806 kt sea-level maximum, 60,000 ft, G -3/+6, 156
hit points, 9 hardpoints, the default-store list and `e2000.HUD`). Hashes of
the files later slices depend on:

| Resource | Bytes | SHA-256 |
| --- | ---: | --- |
| `TOR.PT` | 12,201 | `39f9c677de59267bfcb87634a944433f7d659d47c1a850e9e699c1eb2d3f99fe` |
| `TOR.SH` | 33,280 | `1fc8b434696f88fa8bb28da0c7350aeb3ea9ce2e48c345f54081d1a03824afdb` |
| `TOR_S.SH` | 4,608 | `03a831378eeeb2841eda0240255deb629b50f1995341f28309afcc235aefa40d` |
| `TOR_A.SH` | 25,088 | `98bb65ae64beb19414903ff8add7b8c8a223906d2e2f67ac4d9f97dd3c004769` |
| `TOR_B.SH` | 4,608 | `e570a281aed5a322b7ad78f6376165ac80460bad8be7968f45e9eaef53d9ded1` |
| `TOR_C.SH` | 25,088 | `39b5a1f9a3360ecfc9e920aff6913dd157fefb1c640d15524e4ab3424ea287fa` |
| `TOR_D.SH` | 4,608 | `35756f4049894393eaadf53f6aebdea18dace1e85995dfb667f035ef5776a2f6` |
| `E2000.HUD` | 4,608 | `4ac570f7df6e65c7a00d372ba95ca7ccc8ddedf9361a3ee5051910e5e6738b5d` |
| `_TOR.PIC` | 150,604 | `5d6f20e9debca91487c99d0d7d1eca5d5c0a94ac69f8543071a5bd8116bc7599` |
| `MIG31.ECM` | 354 | `874f7b364ce6068341d6a17e10c4b73b37efcf86ec0d528a62f93e9c0296e56e` |
| `PAVESPK.SEE` | 505 | `7fd9c6d3e909015e8715056e5c0f13ce472cdb7fb70ac10bdcd4a54cb6e53b91` |

### Tornado record against the Su-24 record

`TOR.PT` is the **`SU24.PT` record with eight numeric statements changed**: of
219 statements, **211 are identical to the Su-24's**. The 41,885 lb empty weight
is the Su-24's (the Su-25 shares it, and also 90,390 lb, 35,270 lbf and G
-3/+6; only the Su-24 also shares the 50,700 lbf afterburner thrust and the 156
hit points). Everything beyond the eight numbers is the Tornado's own: names,
shape, HUD, ordnance, hardpoint rows and the ten G envelope rows (the sound
names are the Su-24's too).

| Field | `TOR.PT` | `SU24.PT` |
| --- | ---: | ---: |
| `year` | 1990 | 1983 |
| `maxAlt`, ft | 60,000 | 58,000 |
| `internalFuel`, lb | 11,221 | 22,664 |
| Structure `[0]` / `[1]` | 1,375 / 2,326 | 1,210 / 1,362 |
| Debris `dmg` y | 10 | 0 |
| Debris `dst` x / z | 40 / -20 | 60 / 0 |
| HUD | `e2000.HUD` | `ac130.HUD` |
| Default stores 1..8 | `F18R.SEE`, `MIG31.ECM`, `PAVESPK.SEE`, `BK27.JT`, `F250.GAS`, `PAVEWAY.JT`, `MK82P3.JT`, `AIM9M.JT` | `SU24R.SEE`, `SU24.ECM`, `SU24L.SEE`, `GSH6_23.JT`, `AS14.JT` x4 |

`MIG31.ECM` is the Su-24's jammer (`SU24.ECM`) with a different name: 17 bytes
differ, all in the record tail and its name strings (the MiG-31 review says
the same: byte-equal to `SU24.ECM` and `SU27.ECM` apart from its name).

### Tornado: the template copy list (for John)

Every value that is the Su-24's rather than a Tornado's, against cited real
Tornado IDS figures (Jane's text in `TOR.INF`, the game's own encyclopedia;
aerospaceweb.org Tornado IDS data and MILAVIA Tornado specifications, both
accessed 2026-11-10; Wikipedia Tornado GR4 specifications, accessed
2026-11-10):

| Field | Record | Real | Difference | Source / verdict |
| --- | ---: | ---: | ---: | --- |
| Empty weight, lb | 41,885 | 30,620 basic, 31,065 equipped | +34.8% (vs 31,065) | Su-24 copy |
| Max takeoff, lb | 90,390 | 61,620 (27,950 kg encyclopedia) | +46.7% | Su-24 copy |
| Military thrust, lbf (both) | 35,270 | 17,300 to 18,200 (2 x 8,650 RAF to 9,100) | +94% to +104% | Su-24 copy |
| Afterburner thrust, lbf (both) | 50,700 | 32,150 (2 x 16,075 Mk 103) | +58% | Su-24 copy |
| Hit points | 156 | none | | Su-24 copy |
| G limit | -3 / +6 | +7.5 (encyclopedia) | +25% | Su-24 (and Su-25) copy |
| Internal fuel, lb | 11,221 | 10,280 (encyclopedia); 14,000 (aerospaceweb) | +9.2% / -19.9% | Tornado's own, sources disagree |
| Ceiling, ft | 60,000 | more than 50,000 | +20% | Tornado's own (Su-24 is 58,000) |
| Top speed, kt | 1,369 | Mach 2.2 = 1,262 at 36,090 ft | +8.5% | Tornado envelope, not a copy |
| Sea-level max, kt | 806 | Mach 1.2 = 795 (915 mph) | +1.4% | Agrees |
| Year | 1990 | in service 1979 to 1980 | | Tornado's own value (not examined) |
| Engines | 2 | 2 | | Correct; the shape has two nozzles and two flame groups |

**Copied from the Su-24:** empty weight, max takeoff weight, both thrust
figures, hit points and the G limits, plus the structure-warning words, the
whole flight tuning block and every other PLANE statement (stall, spin, drag,
turn rates, fuel flow; 211 statements). Only the ten G-envelope rows,
fuel, ceiling and year are re-authored. **Not a copy:** the stations, ordnance,
HUD and the G envelope speeds. The record rule is met (the Tornado has its own
shape, stations and envelope), as with the Mirage 2000.

Options for John (none chosen, same precedent as the Mirage 2000: real
thrust, real size, +G corrections are John's call):

1. Keep every value as retail (documented, audit only).
2. Correct thrust to the real two-engine figures (about 17,300 dry / 32,150
   afterburner total, MILAVIA/Jane's Mk 103) and leave weights.
3. Also correct empty weight (31,065) and max takeoff (61,620), plus G +7.5.
   The G envelope rows reach +6 only; a +7.5 row is fitted from the +6 row as
   M2K-11 did for +9.

Agent note: the 41,885 / 90,390 weights make the Tornado fly like a Fencer. A
default load is far below 90,390 (see stations), so the max weight is never the
binding number.

### Tornado G envelope rows

Points are speed in ft/s at altitude in ft. Ten rows, `envMin` -3 to `envMax`
+6; **every row differs from the Su-24's**, so the envelope is authored for the
Tornado (it is not a copy). Row 6 equals row -3 and row 4 equals row -2 and
row 2 equals row -1 (the same mirroring as other records).

| G | Points |
| ---: | --- |
| -3 | 575@0, 667@7,000, 880@14,000, 1,120@14,000, 1,140@0 |
| -2 | 425@0, 507@12,000, 643@23,000, 870@34,000, 1,235@34,000, 1,450@26,000, 1,210@0 |
| -1 | 270@0, 320@14,750, 430@29,500, 600@44,250, 870@52,000, 1,375@52,000, 1,787@45,000, 2,007@39,500, 2,027@34,000, 1,290@0 |
| 0 | 140@0, 170@16,000, 285@35,000, 513@54,000, 820@64,000, 1,514@64,000, 2,067@53,500, 2,420@40,750, 2,430@36,000, 1,420@0 |
| 1 | 200@0, 235@13,000, 307@26,000, 433@39,000, 621@52,000, 870@60,000, 1,484@60,000, 2,007@50,500, 2,300@40,250, 2,310@36,000, 1,360@0 |
| 2 | same as -1 |
| 3 | 345@0, 441@18,500, 617@34,000, 870@44,000, 1,305@44,000, 1,577@40,000, 1,730@31,000, 1,250@0 |
| 4 | same as -2 |
| 5 | 497@0, 593@11,000, 701@16,500, 870@24,000, 1,155@24,000, 1,220@20,000, 1,180@0 |
| 6 | same as -3 |

The 1 G row tops out at 2,310 ft/s (1,369 kt) at 36,000 ft and 1,360 ft/s (806
kt) at sea level; those two numbers are the catalog's top and sea-level speeds.
Rows 0 to 1 reach 60,000 ft (the record ceiling) and the 0 G row 64,000 ft.

### Tornado resources

| Resource | Role | Note |
| --- | --- | --- |
| `TOR.SH` | Shape | 287 runtime faces (single texture `_TOR.PIC`), 0 lines, 0 billboards, six state words (`7dc0`, `7dc6`, `7dcc`, `7dd8`, `7dde`, `7de4`); streamer record present (pivot 20, 2, 16 and wing-tip points 48, 0, -34 in CE units, hinge scale 40) |
| `TOR_S.SH` | Shadow | 14 faces |
| `TOR_A.SH` / `TOR_C.SH` | Damage bodies | 290 / 314 faces |
| `TOR_B.SH` / `TOR_D.SH` | Damage pieces | 36 / 37 faces |
| `VIS240.SEE`, `F18R.SEE`, `MIG31.ECM`, `PAVESPK.SEE`, `BK27.JT` | Sensors, jammer, pod, gun | 21,840 / 546,840 range-class sensors; see below |
| `F250.GAS`, `PAVEWAY.JT`, `MK82P3.JT`, `AIM9M.JT` | Default stores | all in the TORE roster (2,000 lb, 1,500 lb, 190 lb, 1,650 lb) |

`BK27.JT` weight field is 1 (a pod-gun record) and the research inventory notes
the carriers ALPH, E2000, GRIPEN, TOR and VIG were not yet imported (JT-1). The
header scale of `tor.SH` is **2** (as `MIG23.SH`, `SU24.SH`; `F14.SH` is 4,
most aircraft 1).

### Tornado HUD and cockpit (for John)

**Fact.** `TOR.PT` names `e2000.HUD`. That file is the **Eurofighter 2000
HUD**: `E2000.PT` names it too, and its panel names are `~ef2`, `~ef2h`,
`~ef2s`, `~ef2_l/c/r`, `~ef2_lh/ch/rh`, `~ef2_ls/cs/rs`, `~ef2_p` (13 pictures
`~EF2*.PIC` in `FA_1.LIB`, `~EF2H.PIC` 54,656 bytes) plus a stray `~raf_w`. It
is the `RAFALE.HUD` template with the Rafale panel names replaced (71 of 4,608
bytes differ). **No Tornado cockpit art exists**: the sweeps `~TOR*`, `*TORN*`,
`*GR1*`, `*GR4*`, `*IDS*` find only `TOR*` shapes and skins, the `^ACTOR.5K`
callout, `TOR_0/1.PIC`, `TOR.INF`, two unrelated `~Q*STOR.M` mission files,
a flag shape `FGR1.SH` and `AR_VIDS.PIC` (unrelated). So retail flies the
Tornado with the Eurofighter panel family. TORE has no Eurofighter cockpit
today (`E2000.PT` is catalog-decoded only; no `EF2` stem arm).

Options (none chosen):

1. **Follow the record: `E2000.HUD`, cockpit stem `EF2`, `~EF2H.PIC`.** What
   retail does; needs the HUD panel and one new stem family (13 pictures
   already in `FA_1.LIB`). The player sees a Eurofighter cockpit in a Tornado,
   as the Mirage 2000 sees a Rafale one. Not an alias of `E2000.PT`.
2. **Substitute an existing reviewed HUD/cockpit family** (for example the
   Su-24-era `ac130.HUD` is the template's, `F18.HUD` or `RAFALE.HUD`).
   Opinionated, no retail evidence.
3. **Author a Tornado panel.** Not in the media; invented art, later decision.

Update (TOR-7, 2026-11-11): John chose option 1 in the batch decision round
(decision 21, "follow the record", opinionated). The Tornado now measures and
uses the family: see [Tornado sensors, HUD and cockpit
(TOR-7)](#tornado-sensors-hud-and-cockpit-tor-7-2026-11-11).

### Tornado engine count and thrust

The record says 2 engines, correct for the Tornado (two RB199). The shape
agrees: two six-or-eight-sided nozzle rings at x about +/-3.5 (faces `3eb7`
and its mirror, centre 3 to 4 units either side) and two flame groups at
x -5 to -2 and 2 to 5 (word `7dc0`, 8 plates, y -43 to -25). The two-engine
behaviour (contrails, engine-out halving) is therefore right, no engine-count
question. Thrust is the Su-24 total (above): the total is 58% over the real
afterburner and about double the dry figure. Options are the three above. The
RB199 has bucket thrust reversers (`TOR.INF`); no shape word, panel or record
field represents them, so there is nothing in the source to rig (see surfaces).

### Tornado real size (for John)

Measured on the neutral state of `tor.SH` (clean, gear up) and the gear word:
x -34 to 33, y -36 to 45, z -6 to 19; gear down low -11 (the shape's contact
offset in raw units is -11, the parked inspector prints -22 after the header
scale). The flame plates reach y -43. One unit is 1/3 ft at the aircraft
convention (`debris::scale` default). The shape header scale is **2** (like
the MiG-23, which `debris::scale` already lists at 2/3 ft per unit), so the
aircraft is authored at 2/3 ft per unit.

| Axis | Units | At 1/3 ft (current default) | At 2/3 ft (header scale 2) | Real | Verdict at 2/3 |
| --- | ---: | ---: | ---: | ---: | --- |
| Span (wing fully spread) | 67 | 22.3 ft | 44.7 ft | 45.6 ft spread / 28.2 ft swept | -2.0% vs spread |
| Length (nose to tail, no flame) | 81 | 27.0 ft | 54.0 ft | 54.8 ft | -1.5% |
| Height (fin tip z 19 to gear low -11) | 30 | 10.0 ft | 20.0 ft | 19.5 ft | +2.6% |

**Verdict.** The shape is modelled with the wing **fully spread** and is true
size when drawn at the header scale of 2/3 ft per unit (within 3% on all
three axes). If TORE draws it at the default 1/3 ft per unit it is **half
size**. That is a scale-table question for B5-1/TOR-2 rather than a per-axis
real-size fit: no per-axis factor is needed, only the entry `Tornado => 2/3` in
`tore_sim::combat::debris::scale` beside `Mig23` (and the matching
aircraft_scale default of `[1, 1, 1]`). The spread leading edge sweeps
`atan(10/21)` = 25.5 degrees on the planform (real 25 degrees minimum), a
check that this is the spread pose. Options (none chosen):

1. **Draw at 2/3 ft per unit** (the header scale; MiG-23 precedent). Real size
   within 3%. Record positions in feet (`/3`) then land on the aircraft: the
   outer wing row at x 16 is 5.3 ft out, and 8 units at 2/3 ft per unit is the
   same 5.3 ft.
2. **Draw at 1/3 ft per unit** (the default). Half-size Tornado, store rows
   half as far out; not recommended, but it is what the code does if no entry
   is added.
3. **Draw at 2/3 and add a per-axis fit** (the Mirage 2000 mechanism) only if
   John wants the residual 1.5 to 2.6% removed; the factors would be about
   x 1.02, y 1.015, z 0.975.

### Tornado variable sweep (for John)

**Fact: the shape has no sweep.** `tor.SH` has six state words and no sweep
word, no sweep branch and no swept-wing geometry. The streamer record names a
pivot (20, 2, 16) and tip points (48, 0, -34) in CE units (the F-14 and
MiG-23 shapes carry the same kind of record: F-14 pivot 32, 4, 16; MiG-23 pivot
18, 6, 8); how those relate to the mesh is **unknown**. The wing mesh is one
fixed spread pose, 13 faces per side (right: `4cf1 4d2b 4d54 4dea 4eba 4edd
4f00 4f23 4f46 4f7f 4fa8 4ff5 501c`; left: `52c3 52e6 5309 5332 535b 537e
53a1 542b 5454 5483 54a6 5549 5570`), flat plates at z 0 and 1, joined to the
fuselage glove at x 9 to 10 (left -10 to -11). The glove fillets (`4d14`,
`4d83`, `4da4`, `4dc7`, `4e13`, `4f67`) and the fuselage side (x 7, y -6 to 13)
stay fixed. There is no sweep state word to read, so any sweep is a **fitted
rig**, like the MiG-23's (`mig23_animation.rs`: explicit wing face list,
vertical axis through (+/-9, 4, 3), 0 to 40 degrees over 400 to 700 kt, flaps
hold the wings out) and the F-14's.

**Whole-panel sweep (rule 11).** The wing panel is built from separate faces
(13 per side), not one quad; each face moves whole when the panel is turned
about a vertical axis through the glove joint. Shared vertices between the
panel and the fixed fillets lie on the glove line: right (9, 1, 0), (9, 1, 1),
(10, -4, 0), (10, -1, 0), (10, -1, 1); left (-10, 1, 0), (-10, 1, 1), (-11,
-4, 0), (-11, -1, 0), (-11, -1, 1). A pivot at (9.5, 0.5, 0.5) (right, mirror
left at -10.5) puts every shared vertex within about 1.5 units of the axis;
a point exactly on the axis stays fixed. The glove vertices at (9, 1) and (10,
-1) are 2.2 units apart, so **no single pivot point sits exactly on all of
them**, and a turn of 42 degrees (25 to 67) opens or overlaps the fillets by
up to 1.5 units (1 ft at 2/3 ft per unit). The real pivot is also inboard of
the wing/glove joint: Jane's states 60 degrees on the fixed inboard portions
and 25 to 67 degrees on the movable portions ("63 degrees when 2,250 litre
drop tanks are carried", `TOR.INF`), so the joint at 9 to 10 units (6.0 to
6.7 ft out) is the movable-wing root.

| Panel (per side) | Faces | Vertices moving in a whole-panel sweep | On the pivot line | Fixed off the line |
| --- | ---: | ---: | ---: | ---: |
| Wing panel | 13 | 13 distinct vertices (5 shared with the fillets) | the 5 shared | 0 if the axis passes within 1.5 units of the shared vertices |
| Flap strip (`4ff5`, `501c` / `5549`, `5570`) | 2 | all 4 vertices each | 0 | 0, moves with the wing |

Fold check: **no face folds**, because the panel is moved whole; the cost is
the fillet seam. Options (none chosen):

1. **No sweep, fixed spread wing** (what the source has). Honest to the data;
   the Tornado always shows 25 degrees.
2. **Fitted visual sweep like the MiG-23** (additional 0 to 42 degrees over a
   speed band, flaps hold out), whole wing panel about (+/-9.5, 0.5, 0.5),
   with a seam note. Agent-fitted; John chooses the speed band and the angle
   (67 degrees real maximum; the wing to 63 degrees with 2,250 L tanks).
3. As option 2, but restricted to the rig (no change to flight tables, as the
   MiG-23 spec says "no sweep-dependent flight law"), with the wing-mounted
   store rows swivelling to stay aligned with the fuselage (real
   swivelling pylons).

The wingtip vapor streamer should follow whichever the swept tip is
(Mig23 precedent in the spec).

Update (TOR-3, 2026-11-10): the pivot suggested above, (9.5, 0.5, 0.5), is not
within 1.5 units of every shared vertex: the shared corner (10, -4) is 4.5 units
from it, so the worst seam at a 42 degree turn is 3.25 units (2.2 ft), not 1.5.
The point that minimizes the worst separation is the centre of the smallest
circle round the three distinct glove corners (9,1), (10,-1), (10,-4): (9.5,
-1.5), radius 2.55 units, which opens the seam by at most 1.83 units (1.22 ft) at
the full turn. TOR-3 ships that pivot (left (-10.5, -1.5), the left glove being a
unit further out), a speed-scheduled sweep like the MiG-23's, and the other
findings of this section as measured in
[the Tornado control rig](aircraft-control-surfaces.md#tornado-control-rig-tor-3-2026-11-10).

### Tornado surfaces (for John)

State words (neutral 287 faces; counts from `Shape::with_state`):

| Word | Faces | Low point | Geometry (raw units, x right, y forward, z up) | Reading | Status |
| --- | ---: | ---: | --- | --- | --- |
| `7dc0` | +8 | -3 | Two groups of 4 plates behind the nozzles, x -5 to -2 and 2 to 5, y -43 to -25 | Afterburner flames (twin) | Confirmed by position |
| `7dc6` | +4 | +2 | Two plates each side on the rear spine: x 2 to 5 and -2 to -6, y -10 to -7, z 2 to 4 | Airbrake, one per side | Confirmed as the brake slot |
| `7dcc` | +26 | -11 | Nose plates (3 groups of 2 faces, x -1 to 1, y 25 to 29, z -11 to -5) and two mains of 10 faces (x +/-3 to +/-7, y -2 to 3, z -11 to -4) | Landing gear down | Confirmed (slot order) |
| `7dd8` | -2 | none | Removes the left trailing strip x -31 to -11, y -9 to -1 (`5549`, `5570`) | Left flap word | Slot order |
| `7dde` | -2 | none | Removes the right trailing strip x 10 to 30, y -9 to -1 (`4ff5`, `501c`) | Right flap word | Slot order |
| `7de4` | +2 / -2 | +3 | Swaps the lower fin block (`5723`, `5750`) for a skewed copy (`5625`, `5652`): vertices move 1 to 3 units in x | Rudder (positive direction only) | Confirmed by geometry |

The slot order (flame, brake, gear, flap, flap, rudder) is the shared order of
the MiG-25/31, Mirage 2000 and Su-24 families. There is **no hook word (hook
flag clear), no slat word, no spoiler word, no canopy word, no sweep word and no
thrust-reverser word** and no separate geometry for any of them.

Moving-surface map for the rig:

| Surface | Source panel | Rig note |
| --- | --- | --- |
| Tailerons (all-moving, roll) | One 5-vertex polygon per side at z -2 (`446a`/`45e9` right, `4176`/`42f8` left; two windings each) | Only the polygon; no word. Turn whole about a lateral spindle through the root-chord midpoint (y -22.5, z -2). Roll: real tailerons are differential |
| Wing trailing strips ("flaps") | Full strip x 10 to 30 per side, 2 faces (two layers) | Source deletes them with the flap words; the rig keeps them drawn and turns them |
| Slats | none | Not in the source (the 2-unit leading strip `4dea`/`4d2b` is skin) |
| Spoilers | none | Not in the source (real: two per wing) |
| Rudder | The lower fin block, 5 vertices, 2 windings | The word skews it; the upper fin is fixed |
| Airbrakes | 2 plates per side, hinge at y -7 | Appear only when open; see face table |
| Thrust reversers | none | Not in the source |
| Gear | nose 6 faces, mains 10 faces each side | Rigid plates, hinge at top edge |

**Rule 11 face table.** Counted from the `state_neutral`/word dumps.

| Surface | Face | Vertices | Moved | On hinge/spindle | Fixed |
| --- | --- | ---: | ---: | ---: | ---: |
| Taileron right | `446a` | 5 | 5 | 0 | 0 |
| Taileron right | `45e9` | 5 | 5 | 0 | 0 |
| Taileron left | `4176`, `42f8` | 5 each | 5 | 0 | 0 |
| Flap strip right (outer x 30, inner x 10) | `4ff5`, `501c` | 4 each | 2 | 2 (the fixed neighbour edge) | 0 |
| Flap strip left | `5549`, `5570` | 4 each | 2 | 2 | 0 |
| Rudder block | `5723`, `5750` | 5 each | 4 | 1 `(0,-21,4)` | 0 |
| Airbrake plates (4) | `5889`, `58a0`, `57ec`, `5803` | 4 each | 2 | 2 (y -7) | 0 |
| Nose gear (6) | `4a97 ... 4b2a` | 4 each | 2 | 2 (top, z -5) | 0 |
| Main gear (10 per side) | `4908 ... 49ff`, `4705 ... 47fc` | 4 each | 2 (4 on the two top plates) | 2 (top z -4/-5) | 0 |

Verdicts:

- **Tailerons.** The polygon touches the fuselage at one vertex only
  ((6, -20, -2) on the right, (-6, -20, -2) on the left, shared with four fixed
  fuselage faces). No other vertex is shared, so a whole-panel turn folds
  nothing and opens only that one-vertex seam. **Strain, not failure:** the
  polygon has a forward corner at (7, -8, -2), 14.5 units ahead of the
  spindle; at 25 degrees it swings 6 units (4 ft) up or down along the
  fuselage flank (the same flank corner issue as the Mirage). Options (none
  chosen): (a) whole-panel turn about the lateral spindle (rule 11); (b) turn the
  panel about the spindle through the root-chord midpoint with a small
  fitted limit (for example 15 degrees) so the forward corner stays inside the
  fuselage flank; (c) leave the tailerons fixed. Roll: real tailerons are
  differential (plus wing spoilers), so rule 12 plus the taileron standard
  applies; pitch and roll both on the same whole panels.
- **Wing trailing strips ("flaps").** Whole-strip faces; two vertices lie on
  the shared edge with fixed neighbours (the glove-side corner pair at x 10,
  and the outer corner pair at x 30), so a turn about the hinge edge keeps the
  edges stitched. They span x 10 to 30 of the semi-span 33, not the whole
  span, and are deleted by the flap words with no replacement. Real: full
  span flaps and slats ("full-span flaps and leading-edge slats", Wikipedia)
  plus spoilers; the source has none of the slat and spoiler panels. Options:
  rig the strips only, or leave slats and spoilers un-rigged (none are in the
  source; inventing them would be fitted geometry for John).
- **Rudder.** The source swaps the lower fin block for a copy skewed 1 to 3
  units; the upper fin is fixed. The leading vertex (0, -21, 4) is shared with
  fixed faces; the other four move. The retail swap is not an exact rotation
  (as the Mirage 2000), and covers the lower block only.
- **Airbrakes.** Four plates (two per side), each hinged on its forward edge
  (y -7, z 2) with the rear edge raised to y -10, z 4. The source has paired
  brakes (unlike the Mirage 2000's single plate), matching the real
  Tornado's airbrakes. They exist only when open.
- **Gear.** Flat plates, no wheels or doors; nose gear at y 25 to 29 and mains
  at x +/-3 to +/-7, y -2 to 3. At 2/3 ft per unit: wheelbase about 17 ft,
  track about 8 ft (real gear dimensions were not found; unverified).
- **Thrust reversers, spoilers, slats** do not exist in the source and are not
  invented here. The real aircraft has bucket-type reversers (`TOR.INF`) and
  roll spoilers.

The whole-surface gate (John, 2026-11-07/08) is met by every face above except
the taileron's forward-corner strain and the missing-surface gaps; no
diagonal or partial surface is in the source, so no probe sheet is needed.

Update (TOR-4, 2026-11-10): the airbrake word's four faces are two plates, one
a side, each in both windings (`57ec`/`5803` right, `5889`/`58a0` left), not four
plates; the gear is rigged as three rigid pieces (each main leg of ten faces and
the nose of six, folding forward) rather than "rigid plates, hinge at top edge";
and the burner word's eight faces are two crossed plate pairs, one per nozzle
(nozzle octagons `3e90`, `3eb7` at y -24). See [the Tornado gear, airbrake and
burner rig](aircraft-control-surfaces.md#tornado-gear-airbrake-and-burner-rig-tor-4-2026-11-10).

### Tornado sensors, jammer and pod (for John)

Nine rows. Rows 0 to 3 are fixed internal rows (flag `0x8`); `F18R.SEE` is the
**F/A-18's APG-65** (a 546,840-range air-to-air class radar), not the Tornado's
Texas Instruments terrain-following ground-mapping radar ("European built Texas
Instruments multimode ... terrain-following ground-mapping radar",
`TOR.INF`). `MIG31.ECM` is the Su-24 jammer renamed (60 chaff / 35 flare class
rows in the shared MiG-31 review) at (0, -15, -60). `PAVESPK.SEE` (AN/ASQ-153
Pave Spike, 425 lb) is a **fixed internal row at (0, 0, 0) with flag `0x8`**,
the same arrangement as the F-16C and A-10 rows.

- **Pave Spike.** The real Tornado GR1 did not carry Pave Spike: a Buccaneer
  detachment carried the Pave Spike laser designator so GR1s could drop
  guided weapons; the TIALD pod arrived in 1991 (Wikipedia Tornado, accessed
  2026-11-10). There is no pod station in the record. Options: (1) keep the
  row as the retail fixed sensor (no pod drawn, nothing on a station,
  B7: never hang a pod on a station the real aircraft lacks); (2) drop the row
  for TORE; (3) keep it but present it as the targeting sensor.
- **Jammer.** The real jammer is the Sky Shadow pod or internal Marconi gear
  (not verified in this review); `MIG31.ECM` is a Soviet-named record with the
  Su-24's values. Options: keep as retail, or rename the label only.
- **Radar.** `F18R.SEE` gives the Tornado an air-to-air radar it does not have
  in reality; keep as retail or substitute a ground-mapping record (none
  exists in the media).

### Tornado hardpoints

All nine rows, in original order, decoded with the standard field order.
Location byte: 0 Centerline, 1 Fuselage, 2 Internal Gun, 3 Internal Bay, 4
Wing, 5 Wingtip. Feet divide the record units by three. The wing/fuselage rows
at the header scale 2/3 ft per unit land on the mesh (see size); at the default
1/3 they would sit inside a half-size model.

| Row | Location | Flags | x / y / z | Feet right / up / fwd | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `VIS240.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `F18R.SEE` | Fixed radar (APG-65) |
| 2 | Fuselage | `0x0008` | 0 / -15 / -60 | 0 / -5 / -20 | 1 | 0 | `MIG31.ECM` | Fixed jammer |
| 3 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `PAVESPK.SEE` | Fixed pod row |
| 4 | Fuselage | `0x000c` | 7 / -5 / 55 | 2.3 / -1.7 / 18.3 | 360 | 0 | `BK27.JT` | Internal gun, 360 rounds, mirrored (two guns of 180) |
| 5 | Wing | `0x0205` | 0 / -25 / 17 | 0 / -8.3 / 5.7 | 2 | 40 | `F250.GAS` | Tank pair at x 0 (centreline pylon in reality) |
| 6 | Fuselage | `0x07e5` | 6 / -25 / 14 | 2 / -8.3 / 4.7 | 4 | 80 | `PAVEWAY.JT` | Shoulder pylon pairs, 4 bombs |
| 7 | Wing | `0x0505` | 11 / -25 / 9 | 3.7 / -8.3 / 3 | 2 | 30 | `MK82P3.JT` | Inner wing pair |
| 8 | Wing | `0x0085` | 16 / -22 / 4 | 5.3 / -7.3 / 1.3 | 2 | 5 | `AIM9M.JT` | Outer wing pair |

Default stores in the catalog (`BK27.JT`, `F250.GAS`, `PAVEWAY.JT`,
`MK82P3.JT`, `AIM9M.JT`, one per row) count one default per row; per-row
quantities are a host rule for TOR-6. Weights: Paveway 2,000, Mk 82 triple
1,500, AIM-9M 190, 250 gal tank 1,650 lb each.

Against the real Tornado ("seven fuselage and wing hardpoints: one centreline
pylon, two fuselage shoulder pylons each with three ejector release units,
and under each wing one inboard and one outboard pylon", swivelling wing
pylons, `TOR.INF`; Wikipedia: 3 fuselage and 4 wing stations): row 5 is the
centreline (x 0), row 6 the shoulder pylons (2 ft out, four bombs, three
ERUs a side in reality), rows 7 and 8 the inner and outer wing pylons. The
record has no pod station; **a Tornado has no pod station in this record**, so
no pod is ever placed (B7). Open items for TOR-5: row 5 is a Wing-location row
at x 0 with the pair flag (the F-4 and A-4E cases show how TORE treats such
rows: a centreline rack); rows 7 and 8 sit 3.7 and 5.3 ft out, about on the
glove, and the real pylons swivel with the wing; the mesh has no pylons. The
real outer pylon position is not cited here; the real stations need a
dimensioned drawing before TOR-5 moves any row.

### Tornado roster note

`TOR.PT` is the 4th aircraft of batch 5 and the 44th retail identity (after
`M2000` 40, F15E 41, Gripen 42, F-5E 43), appended by B5-1. The wire identity
is 9 bits (NET-ID9), so there is no capacity constraint.

### Tornado unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| Record template (weights, thrust, G, hit points) | Fly like a Fencer | John, options above |
| Draw scale (1/3 against header 2/3 ft per unit) | Half size if no table entry | John; B5-1/TOR-2 adds the entry |
| Sweep | Not in the shape; fitted only | John, options above |
| Streamer pivot (20, 2, 16) relation to the mesh | Possible real pivot data | Unknown; fit and compare to the F-14 and MiG-23 |
| HUD and cockpit | Needs `EF2` family or substitute | John, three options |
| Taileron forward corner (7, -8, -2) | Swings 4 ft at 25 degrees | TOR-3 probe, small limit or fixed |
| No slats, spoilers, reversers in the source | Real aircraft has them | Not rigged unless John asks |
| `F18R.SEE` (APG-65) and `MIG31.ECM` on a Tornado | Wrong radar and jammer | John, options above |
| Pave Spike fixed row | Not on a Tornado | John, options above |
| Real station positions | Rows 5 to 8 vs the real pylons | Needs a cited drawing (TOR-5) |
| Gear dimensions | Wheelbase and track unverified | TOR-4 with a cited source |

## Mirage F1 source review (2026-11-10)

Research mode, slice MF1-0 (batch 5). This reviews `MF1.PT` and its dependency
closure before `AircraftId::Mf1` exists. `MF1.PT` becomes the 45th retail
identity in the batch identity slice (B5-1, checkpoint code 45). The Mirage F1
is a parked target today; `parked_aircraft.rs` lists `MF1.PT` with gear word
`0x5adc` and that path stays as it is. No original module was executed. PT,
SEE, ECM, JT and GAS are read as bounded BRF data with the shared field order
in `aircraft_schema.rs`; SH is inspected as data. Slice done by Claude Sonnet
(sonnet-medium). Nothing here changes code or behaviour.

Questions for John, **not decided here**: the record template and thrust
([below](#mirage-f1-record-template-and-thrust-for-john)), the HUD and cockpit
([below](#mirage-f1-hud-and-cockpit-for-john)), the surfaces
([below](#mirage-f1-surfaces-and-rule-11-for-john)), the centreline jammer pod
([below](#mirage-f1-pods-and-stations-for-john)) and the size
([below](#mirage-f1-real-size-for-john)).

### Mirage F1 build and extraction

`FA_1.LIB` and `FA_2.LIB` are the same pair as the MiG-31 review (SHA-256
`657254c5...` and `fb8b3021...`, see the [MiG-31 build
section](#build-and-extraction)). Only the named closure was extracted, to
`.local/m3-imports/scratch/mf1`, with `tore-extract --source
gameassets/fighters-anthology --out .local/m3-imports/scratch/mf1 --include
'MF1*' --include '_MF1*' ...` plus the SEE, ECM, JT, GAS, HUD, AI and sound
files the record names and four comparison records (`RAFALE.PT`, `M2000.PT`,
`MR3.PT`, `M5.PT`). The `SETUP.ESA` installer copy was extracted too: every
file is byte-identical to the root copy except `RAFALE.HUD` (4 bytes, build
difference, as in the Mirage 2000 review). The usual five unrelated archive
errors appear (`LHX*.LIB` compression flags and `_SETUP.LIB`).

### Mirage F1 identity and record

| Field | Decoded value |
| --- | --- |
| PT | `MF1.PT`, 12,659 bytes, SHA-256 `ea10025dfe50ebeeb543e2173fb9a033404845183294c16485a3d38fd1537249` |
| Displayed names | `MF1`, `Mirage F1`, `MF1.PT` |
| Schema | OBJECT + NPC + PLANE; `structType` 5; **`typeSize` 636**; `instanceSize` 609 |
| Object class / year / hit points | `0x8000` (fighter) / 1970 / 95; `expType` 30 |
| PLANE flags | `0x11` (seat `0x10`, base `0x01`); hook, no-lift, carrier and multi-crew clear |
| Engines | **1** (the real aircraft has one Atar 9K-50) |
| Shape | `mf1.SH` (25,088 bytes), shadow `mf1_s.SH` (15 faces) |
| Damage looks | `MF1_A.SH` 225 faces, `MF1_B.SH` 53, `MF1_C.SH` 216, `MF1_D.SH` 49 |
| Skins | `_MF1.PIC` (153,204 bytes), `_MF1_A.PIC` 140,984, `_MF1_B.PIC` 71,564, `_MF1_C.PIC` 138,904, `_MF1_D.PIC` 42,704 (all in `FA_2.LIB`) |
| Sounds | `&JET1N.11K` loop, `&JET1A.11K` second, `&powerup.5k`, `&powerdn.5k` (present) |
| HUD | `rafale.HUD` (resolves to the existing `RAFALE.HUD`) |
| AI script | `f.BI` (12,800 bytes, shared) |
| Sensors / jammer / gun | `VIS340.SEE`, `F18R.SEE`, `AV8.ECM` (row 2), `DEFA.JT`; pod `ALQ167.ECM` (row 4) |
| Radio callout | `^ACMF1.5K` exists (5,648 bytes) |
| Encyclopedia | `MF1.INF` and `MF1_0/1/2/C.PIC` in `disc2/FA_3.LIB`; reference media, numbers only |
| PTS | none: no `MF1.PTS` exists |
| Checkpoint code / CLI key | 45 and `mf1`/`mf1.pt`, reserved for B5-1 |

The decoded record equals the catalog row in [`fa-aircraft.csv`](fa-aircraft.csv)
field for field (1970, `0x8000`, 1 engine, 15,542 / 6,120 / 30,203 lb, 15,000 /
30,000 lbf, 1,345 kt top, 66,125 ft, G -3/+7, 95 hit points, 8 hardpoints, the
default-store list and `rafale.HUD`).

**B5-1 must add a type-size entry.** `AircraftId::Mf1` needs `=> 636` in the
`typeSize` match in `crates/tore-formats/src/aircraft.rs` (the F-4E, F-14 and
MiG-29 use 636); the fall-through value 660 would refuse to load. `instanceSize`
(609) is not validated.

Resource hashes of the files later slices depend on:

| Resource | Bytes | SHA-256 |
| --- | ---: | --- |
| `MF1.PT` | 12,659 | `ea10025dfe50ebeeb543e2173fb9a033404845183294c16485a3d38fd1537249` |
| `MF1.SH` | 25,088 | `a19501cfa37ad8b28315080ff97db2ec232cdb33a7cde9a109879e00367cbe41` |
| `MF1_S.SH` | 4,608 | `77deb0f5d32e6e3f5f60d4ab9f212e5ec61c8c2f03f85b148d48261f33107d53` |
| `MF1_A.SH` | 16,896 | `21ec1c3ac526646a3c7f53d546044a5d1867c29a555c001f308b6f9c3970fa5d` |
| `MF1_B.SH` | 4,608 | `fa4ab5d0d3ae5d3d2bc706dd875107dd64c8140b2783bcd3da44a8b94b140224` |
| `MF1_C.SH` | 16,896 | `dccf56cf4f8d22475cb2ccc50270d4e07f438c9a520e92e4dcf38eb488b50d54` |
| `MF1_D.SH` | 4,608 | `02364ac9c89546e251fa2f294eca23140f4af9d2b64a0a18e6be0f6f87ae0c72` |
| `_MF1.PIC` | 153,204 | `85b82d8647d44965903cdbc6d3209ec983369eb39a16c0c8c316c055c21c6eee` |
| `RAFALE.HUD` | 4,608 | `b555f9febf6041297393c8d92e9e70343b1c5c9439316b1c0c1ec0721fe39228` |
| `AV8.ECM` | 344 | `90d9dff4344c2914a2b90f240de63f021a0e5b73cb6ea894a186986a916f6ce2` |
| `ALQ167.ECM` | 376 | `cab8049aa941866fc906fcef385f19b78b721545f8b36ab7bdaa62f9563065a8` |
| `F250.GAS` | 205 | `974e49de6eb22237bf7a1bd24b3ed9308300bbb95270a9c0fe7104892514f0de` |

### Mirage F1 record template and thrust (for John)

**Fact: this record is the Mirage III record with new numbers, not the Rafale
template.** Diffed field by field (238 decoded fields and named blocks), `MF1.PT`
is identical to `MR3.PT` (Mirage III) in 198 and differs in 40, against 178
identical and 60 different for `RAFALE.PT`, and 188 identical and 50 different
for `M2000.PT`. `M5.PT` (Mirage 5) is identical in 191. Only three fields match
the Rafale but not the Mirage III (`dmgDebrisPos.z` 0, `dstDebrisPos.x` 0,
`throttleAcc` 40). The 44 system-damage bytes equal both the Mirage III and the
Rafale. Fields that match **no** other record I compared (the F1's own
numbers): year 1970, `maxVisDist` 144, `maxAlt` 66,125, G range -3/+7, structure
1,328 / 2,107, the three turn-rate acceleration words (147 / 294), turbulence
103, `spinExit` -1, thrust 15,000, afterburner thrust 30,000, afterburner fuel
flow 23 and `_gpullDrag` 43, plus all eleven G rows.

Differences from the Mirage III record (all 40): debris positions, year, view
distance, ceiling, G range, structure, turn-rate words, turbulence, spin exit,
thrust, afterburner thrust, throttle acceleration (40 against 50), afterburner
fuel flow (23 against 21), `_gpullDrag` (43 against 46), `hudName` (`rafale.HUD`
against `seaharcc.HUD`), the shape, shadow and names, the four rows of the
station block that differ and the G table.

**Copies from the Mirage III record, with the cited real Mirage F1C figure.**
The real figures are Jane's text in the game's own encyclopedia (`MF1.INF`, F1-C
variant) unless a web source is named; none establishes original behaviour.

| Field | `MF1.PT` | Copied from | Real F1C | Difference |
| --- | ---: | --- | ---: | ---: |
| Empty weight, lb | 15,542 | `MR3.PT`, `MR3E.PT`, `M5.PT` (exactly) | 16,314 (7,400 kg) | -4.7% |
| Maximum takeoff, lb | 30,203 | same three | 35,715 (16,200 kg) | -15.4% |
| Internal fuel, lb | 6,120 | `MR3.PT`, `MR3E.PT` | about 7,385 (4,300 L, [MIGFlug](https://migflug.com/afterburner/aircraft/dassault-mirage-f1/)); 7,580 at 0.8 kg/L | -17% to -19% |
| Hit points | 95 | `MR3.PT` | none | n/a |
| Signatures 87, 87, 100, 100; `sigs` | as Mirage III | `MR3.PT` | none | n/a |
| Wheel-brake drag 102, PLANE `flags`, OBJECT `flags` | as Mirage III | `MR3.PT` | none | n/a |
| Rudder yaw -3 / 3 / 3, `spinEntry` 0, view offset z 60 | as Mirage III | `MR3.PT` | none | n/a |
| Station rows 0 to 2 (`VIS340.SEE`, `F18R.SEE`, `AV8.ECM`), gun row, wingtip `R550.JT` | as Mirage III | `MR3.PT` | see stations | n/a |
| Sounds, AI script `f.BI`, crash, stall, spin and drag words | as Mirage III and Rafale | family template | none | n/a |

The F1's thrust is **not** a Mirage III copy and is the largest departure:

| Field | Record | Real F1C (one Atar 9K-50) | Difference | Verdict |
| --- | ---: | ---: | ---: | --- |
| Military thrust, lbf | 15,000 | 11,020 (49.0 kN dry, [MIGFlug](https://migflug.com/afterburner/aircraft/dassault-mirage-f1/)) | +36% | Over |
| Afterburner thrust, lbf | 30,000 | 15,873 (70.6 kN, Jane's text in `MF1.INF`; [MILAVIA](https://milavia.net/aircraft/miragef1/miragef1_specs.htm) gives 70.21 kN) | +89% | Nearly double |
| Ceiling, ft | 66,125 | 65,600 (20,000 m service ceiling) | +0.8% | Agrees |
| Top speed, kt | 1,345 | 1,266 (the encyclopedia's data line; Mach 2.2) | +6.2% | Agrees closely |
| Sea-level top speed, kt | 800 | 800 ("Mach 1.2 (800 knots)") | 0% | Agrees exactly |
| G range | -3 / +7 | +7.5 / -3.6 (as commonly published, [MIGFlug](https://migflug.com/afterburner/aircraft/dassault-mirage-f1/)) | -0.5 G on the positive side | Agrees closely |
| Engines | 1 | 1 | none | Agrees |
| Year | 1970 | prototype flew 23 December 1966 (`MF1.INF`); service 1973 | n/a | Left |

The Mirage III record next to it has the real Atar 9C thrust (13,668 lbf
afterburner), so the F1's 15,000 / 30,000 is an authored pair, not a template
inheritance. The record's military figure (15,000) is within 6% of the real
**afterburner** thrust (15,873) and the afterburner figure is almost exactly
double it; I note this and conclude nothing. The record's top speeds, ceiling
and eleven G rows are consistent with the encyclopedia: the 1 G row peaks at
2,270 ft/s (1,345 kt) at 36,000 ft and 1,350 ft/s (800 kt) at sea level, and it
reaches 66,125 ft. So the envelope looks authored for the real aircraft while
the thrust looks inflated; the flight model uses the envelope as a speed limit
and the thrust for acceleration and climb. Clean weight (empty plus internal
fuel) is 21,662 lb, so retail thrust-to-weight with afterburner is 1.38 against
about 0.73 for the real aircraft at its 24,030 lb clean weight.

**G rows** (speed in ft/s at altitude in ft; eleven rows, -3 to +7):

| G | Points |
| ---: | --- |
| -3 | 475@0, 627@14,000, 880@26,000, 1,130@26,000, 1,310@20,000, 1,250@0 |
| -2 | 365@0, 487@15,000, 673@30,500, 870@40,000, 1,225@40,000, 1,477@35,000, 1,580@27,000, 1,445@19,000, 1,290@0 |
| -1 | 240@0, 320@14,750, 430@29,500, 620@44,250, 870@59,000, 1,467@59,000, 1,882@50,000, 2,017@35,000, 1,508@18,000, 1,330@0 |
| 0 | 100@0, 140@13,000, 225@27,000, 313@39,000, 541@56,000, 870@71,000, 1,555@71,125, 2,120@57,250, 2,410@36,000, 1,615@18,000, 1,370@0 |
| 1 | 170@0, 235@13,000, 307@26,000, 433@39,000, 631@54,000, 870@66,000, 1,481@66,125, 2,033@54,250, 2,270@36,000, 1,550@18,000, 1,350@0 |
| 2 | same polygon as -1 |
| 3 | 295@0, 431@18,500, 559@31,750, 697@40,000, 870@49,000, 1,355@49,000, 1,687@44,000, 1,790@31,000, 1,475@18,500, 1,310@0 |
| 4 | same polygon as -2 |
| 5 | 427@0, 540@12,250, 693@23,500, 870@33,000, 1,115@33,000, 1,327@29,500, 1,410@23,000, 1,270@0 |
| 6 | same polygon as -3 |
| 7 | 520@0, 645@11,500, 900@20,000, 1,075@20,000, 1,240@17,000, 1,230@0 |

None of the eleven rows equals the same-G row of the Mirage III, Mirage 5,
Rafale or Mirage 2000. The 0 G row has a point at 71,125 ft above `maxAlt`; the
host caps altitude at the record field, as for the MiG-31.

Options for the template and thrust (none chosen; the Mirage 2000 precedent,
John 2026-11-09, was one-engine real thrust, labelled opinionated):

1. **Keep retail** (15,000 / 30,000 lbf). Matches the catalog and the audit
   rule that record facts stay retail. Cost: nearly double the real
   afterburner thrust, so acceleration and climb are well above the real
   aircraft inside an envelope that is otherwise right.
2. **Override to the real Atar 9K-50**: 11,020 lbf dry, 15,873 lbf with
   afterburner (the Mirage 2000 approach; `aircraft_corrections.rs`). No engine
   count change is needed, the record already says 1. MF1-8 would check climb
   and acceleration against the retained envelope rows.
3. **Override the afterburner only** (30,000 to 15,873) and keep 15,000 as
   military thrust, which is already near the real wet figure. Smallest change;
   leaves military thrust 36% over the real dry thrust.

Empty weight (-4.7%), maximum takeoff (-15.4%) and internal fuel (-17% to -19%)
are Mirage III copies below the real figures. Whether to correct them is also
John's call (the Mirage 2000 left weights at retail). Hit points and the
signature block are template values with no real counterpart.

### Mirage F1 HUD and cockpit (for John)

**Fact.** `MF1.PT` names `rafale.HUD`. It resolves (case-insensitive) to the
existing `RAFALE.HUD`, the same file TORE's Rafale and, since the Mirage 2000,
`M2000.PT` use (same hash `b555f9fe...`), with the Rafale cockpit family
(`~RAF*.PIC`, 13 pictures in `FA_1.LIB`, stem `RAF`, `~RAFH.PIC`). The Mirage
III record next to it names `seaharcc.HUD` (the Sea Harrier panel), so naming
`rafale.HUD` was a deliberate choice by the record's author. **No Mirage F1
cockpit art exists in any archive**: sweeps for `*MF1*`, `~MF*`, `*MIR*`,
`*F1.*`, `*F1_*`, `*F-1*` and `$MF1*` find only the `MF1*` shapes, skins, the
`^ACMF1.5K` callout, the `MF1_*` encyclopedia pictures, the unrelated `AF1`,
`MTF1` and `MIF1` records and the same stray `IFMIRA.PIC` the Mirage 2000
review noted.

Options (none chosen), as in the Mirage 2000 review:

1. **Follow the record: `RAFALE.HUD`, stem `RAF`, `~RAFH.PIC`.** What retail
   does and what John chose for the Mirage 2000 (2026-11-09). Bind by name, never
   alias the Rafale record. The player sees a Rafale cockpit in a Mirage F1.
2. **Substitute another reviewed HUD family**, for example the Mirage III's
   `seaharcc.HUD` panel if it is wired. Opinionated, extra stem arms.
3. **Author a Mirage F1 panel.** Invented art; a separate later decision.

Agent note: option 1 is evidenced and needs one exhaustive stem arm in MF1-7,
exactly as M2K-7 did.

Update (MF1-7, 2026-11-10): John chose option 1 in the batch decision round
(decision 25, "follow the record", opinionated); the stem arm already existed
from B5-1 and needed no change. See [Mirage F1 sensors, HUD and
cockpit](#mirage-f1-sensors-hud-and-cockpit-mf1-7-2026-11-10).

### Mirage F1 parked path

`("MF1.PT", Some(0x5adc))` is in the `GEAR` table of the retail test in
`crates/tore-formats/src/parked_aircraft.rs`. That path reads only the OBJECT
block of the record (names, shape and shadow, class, flags, hit points, the five
signatures, the two debris positions, `expType`, `craterSize`) plus the shape's
gear word. `parked_inspect` on the real media confirms the same values the
identity slice must leave alone:

| Item | Value |
| --- | --- |
| Shape / shadow | `MF1.SH` / `MF1_S.SH`, scale 1 |
| Class / hit points / `expType` / crater | `0x8000` / 95 / 30 / 0 |
| Debris | damaged (-40, 0, 0), destroyed (0, 10, -30) |
| State words | `5ad0 5ad6 5adc 5ae8 5aee 5af4` (236 faces) |
| Gear word | `0x5adc` (+6 faces, low point -17); shape low point gear up is -10 |
| Ground contact offset | -17 (the gear low point) |

B5-1 adds `AircraftId::Mf1` reading the same `MF1.PT`; because the parked reader
neither reads the flight model nor the hardpoints and has no whitelist, adding
the flown identity does not touch it. Never alias `MF1.PT` to another record
(B6). The state words follow the family order flame, brake, gear, flap, flap,
rudder (the Mirage 2000 order).

### Mirage F1 surfaces and rule 11 (for John)

**State words.** From `shape_inspect` with each word set to 1 alone (runtime
faces; the export path adds two). Axes are shape units (1 unit = 1/3 ft), x
right, y forward, z up.

| Word | Faces | Low point | Geometry | Reading | Status |
| --- | ---: | ---: | --- | --- | --- |
| `0x5ad0` | +8 | -5 | Four crossed plates behind the nozzle, y -42 to -81 | Afterburner flame | Slot order and position |
| `0x5ad6` | +2 | +5 | One quad, (2/-2/3, 19, 14) down to (+-2/3, 29, 5), two windings | Dorsal airbrake: one centreline plate | Slot order; geometry unusual |
| `0x5adc` | +6 | -17 | Three flat plates: mains at x +-3 to +-7 (y -5 to 4, z -6 to -17), nose at x 0 (y 38 to 47, z -7 to -17) | Landing gear down | Confirmed; the parked word |
| `0x5ae8` | -4 | none | Removes the whole left outboard trailing-edge panel (x -23 to -35) | Left flap-slot word | Slot order only |
| `0x5aee` | -4 | none | Removes the whole right outboard trailing-edge panel (x 23 to 35) | Right flap-slot word | Slot order only |
| `0x5af4` | 0 net | +3 | Swaps the rudder quad (x 0) for a copy deflected about 45 degrees to +x | Rudder deflection, one direction | Confirmed by geometry |

There is **no hook word** (hook flag clear), **no slat geometry**, **no spoiler
geometry**, **no inboard flap panel**, **no canopy word** and **no nozzle word**
in the source. The only trailing-edge panel on the wing is the outboard one;
the wing's inboard trailing edge (x 6 to 23) is fixed faces. I recommend none
of the missing parts be invented. The word-to-face mapping for the two panel
words is a reading from slot order and geometry (the Mirage 2000 slots 4 and 5
are the same, and there they delete the inboard strips); the original's use of
them is unknown.

**Real aircraft.** Cited facts (all web sources are secondary; the encyclopedia
text is the game's own): full-span leading-edge slats and double-slotted
trailing-edge flaps ([Wikipedia](https://en.wikipedia.org/wiki/Dassault_Mirage_F1),
[MIGFlug](https://migflug.com/afterburner/aircraft/dassault-mirage-f1/)); ailerons
with carbonfibre skin and an "All-moving, mid-mounted, swept tapered tailplane"
(Jane's text, `MF1.INF`); the roll control described in the secondary sources
found is ailerons plus two-section upper-wing spoilers, with no source
describing differential tailplane action; the airbrake position is reported
inconsistently (twin perforated brakes under the intakes in one source, on the
wing in another), and the source shape has neither, only one dorsal plate.

**The all-moving tailplane.** It is **one flat five-vertex panel per side**
(right `2d0e`/`2d36`, left `2c60`/`2c88`, two windings each), at z -1, with no
state word. It is a whole panel, so it can be rigged as one without a fold:
root chord about y -22 to -41 at x 5 to 6, tip at x 23 (right) or -22 (left)
(the two sides differ by one unit), no anhedral. The lateral spindle runs
through the root chord midpoint (y -31.5, z -1). The root vertex (5, -39, -1) is
shared with four fixed rear-fuselage faces, so a turn opens a small crack there
(accepted for a whole-panel turn, as on the MiG-31). **Does it take roll?**
The cited evidence says no: the real F1 has ailerons (Jane's text) and I found
no source for taileron roll; the batch standard ("with roll where the real tails
roll") then means pitch only. This is unconfirmed beyond those sources, so it is
a question for John.

Options for the tailplane and ailerons (none chosen):

1. **Tailplane pitch only, ailerons roll** (the evidence-led reading): the
   outboard trailing-edge panel carries roll with a differential angle; the
   tailplane turns together both sides for pitch.
2. **Tailplane pitch plus differential roll** (taileron, as the F-15 and
   Rafale-style tails): no cited source for the F1; opinionated.
3. **Ailerons only on the outboard panel and no roll from the tail**, with the
   panel also taking the flap-slot words as a droop (same as option 1 for roll;
   decide the droop in MF1-3).

Update (MF1-3, 2026-11-10): John chose option 1 (tailplane pitch only; ailerons
carry roll). MF1-3 rigged the tailplane, the two outboard panels as ailerons and
the rudder as whole panels; the flap-slot words are pinned and never set, and
the flap lever moves nothing because no source places the real F1's flaps at the
aileron position (option 3's droop is not applied). The geometry, fold table,
seam openings and magnitudes are in [Mirage F1 control
surfaces](aircraft-control-surfaces.md#mirage-f1-control-surfaces-mf1-3-2026-11-10).

Update (MF1-4, 2026-11-10): the airbrake is rigged as the source's one dorsal
plate only (hinge the forward edge, y 29, z 5), as planned; the real paired brakes
are not in the shape and are not drawn. The gear and burner follow the same
section: [Mirage F1 gear, airbrake and
plume](aircraft-control-surfaces.md#mirage-f1-gear-airbrake-and-plume-mf1-4-2026-11-10).

**Rule 11 face table** (from the `state_neutral` dump; "on hinge" means within
one unit of the hinge line, the thickness of the wedge; no face has a fixed
vertex):

| Surface | Face | Vertices | Moved | On hinge | Fixed |
| --- | --- | ---: | ---: | ---: | ---: |
| Right outboard panel (x 23 to 35) | `3ce8` (top quad) | 4 | 3 | 1 | 0 |
| Right outboard panel | `3cc9` (bottom quad) | 4 | 3 | 1 | 0 |
| Right outboard panel | `3d0f` (top leading wedge) | 3 | 1 | 2 | 0 |
| Right outboard panel | `3d30` (bottom leading wedge) | 3 | 1 | 2 | 0 |
| Left outboard panel (x -23 to -35) | `3e93`, `3e74`, `3eba`, `3edb` | 4, 4, 3, 3 | 3, 3, 1, 1 | 1, 1, 2, 2 | 0 |
| Right tailplane | `2d0e`, `2d36` | 5 each | 5 | 0 (turns about a spindle) | 0 |
| Left tailplane | `2c60`, `2c88` | 5 each | 5 | 0 (turns about a spindle) | 0 |
| Rudder | `40e1`, `4108` (deflected pair `4007`, `402e`) | 4 | 2 | 2 | 0 |
| Gear (each of 6 faces) | `3b7e` to `3c73` | 4 | 2 | 2 | 0 |
| Airbrake (each of 2 faces) | `4182`, `4199` | 4 | 2 | 2 | 0 |

Notes behind the table:

- **Outboard panel, each side.** The four faces form one tapered
  trailing-edge panel: a quad pair plus a thin leading wedge pair, all removed
  by the same word. The hinge line is the straight leading edge of the wedge,
  from (23, -6) to (35, -11) at mid thickness z about 1 to 1.5; chord 6 units
  (2 ft) at the root, 4 (1.3 ft) at the tip, so the surface is 4 ft wide at 59%
  to 90% of the semi-span, the aileron position on the real wing. It is a whole
  panel (the quad vertices (23, -8) and (23, -12) are the moving seam,
  shared with the fixed inboard wing faces `2103`, `2121`, `21ff`, `22f6`, `230e`
  and `2321`, so a turn opens a crack there; the outboard seam at x 35 meets the
  fixed wing-tip faces `216c`, `2190`, `2339`, `2384`). No diagonal or partial
  surface: no face of the panel has a fixed vertex off the hinge.
- **Rudder.** One quad (two windings) on a fin line from (0, -30, 3) to (0, -39,
  22). Two vertices lie on that hinge and are shared with the fixed fin faces
  `2f4d` and `3073`; the other two move, and the trailing top vertex (0, -44,
  22) is also shared with fixed fin-cap faces `2f70` and `3096` (a crack, as the
  retail swap itself leaves). The retail deflection is about 45 degrees, one way.
- **Gear.** Three flat plates (6 faces), no doors, no wheels, no struts.
  Wheelbase 43 units (14.3 ft), main track at the ground 14 units (4.7 ft).
- **Airbrake.** One plate 5 units wide: forward edge on the spine at (y 29, z 5),
  free edge aft and raised at (y 19, z 14). The hinge is the forward edge
  (MF1-4 to confirm against the spine faces). The source has no paired brakes.
- **Burner.** Four crossed plates, additive plume, no hinge. One nozzle ring at
  y -42 and one flame group, which agrees with the record's one engine.
- No face of any moving surface is split across a diagonal or a coordinate
  threshold, and no probe sheet is needed.

### Mirage F1 pods and stations (for John)

All eight source rows, in original order. Positions are source units; feet
divide by three; x right, y up, z forward ([aircraft
ordnance](aircraft-ordnance.md#hardpoint-positions)). Location byte: 0
Centerline, 1 Fuselage, 2 Internal Gun, 3 Internal Bay, 4 Wing, 5 Wingtip. Flag
`0x8` marks a fixed internal row.

| Row | Location | Flags | Position x/y/z | Feet right / up / fwd | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `VIS340.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x0008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `F18R.SEE` | Fixed radar |
| 2 | Fuselage | `0x0008` | 0 / 0 / -50 | 0 / 0 / -16.7 | 1 | 0 | `AV8.ECM` | Fixed internal jammer suite |
| 3 | Internal Gun | `0x000c` | 3 / -5 / 51 | 1 / -1.7 / 17 | 250 | 0 | `DEFA.JT` | Internal gun, 250 rounds |
| 4 | Centerline | `0x07e5` | 0 / -18 / 5 | 0 / -6 / 1.7 | 1 | 22 | `ALQ167.ECM` | Centreline station, jammer pod default |
| 5 | Wing | `0x07e5` | 6 / -17 / 0 | 2 / -5.7 / 0 | 4 | 46 | `F250.GAS` | Inner pair, tank default |
| 6 | Wing | `0x01c5` | 21 / -10 / -3 | 7 / -3.3 / -1 | 2 | 20 | `R550.JT` | Outer wing pair, Magic default |
| 7 | Wingtip | `0x0085` | 41 / 3 / -11 | 13.7 / 1 / -3.7 | 2 | 4 | `R550.JT` | Wingtip rail pair |

Default stores in the catalog (`VIS340.SEE`, `F18R.SEE`, `AV8.ECM`, `DEFA.JT`,
`ALQ167.ECM`, `F250.GAS`, `R550.JT` x2) count one default per row; the per-row
quantities are a host rule for MF1-6. All three weapon names are live in TORE
today (`DEFA.JT`, `R550.JT`, `F250.GAS`); `AV8.ECM` is a reviewed internal
suite; `ALQ167.ECM` is decoded but its inventory row says unreviewed and hidden.

**Real stations (Jane's text, `MF1.INF`).** Centreline pylon 2,100 kg; inner
wing pylon 1,300 kg each; outer wing pylon 550 kg each; wingtip AAM rail 150 kg
each (Magic or Sidewinder); chaff and flare dispensers, 100 kg, on each fuselage
shoulder. Auxiliary tanks: two of 1,200 L (317 US gal) on the inner wing pylons
and one of 2,200 L (581 US gal) on the underfuselage station. Armament: two 30
mm DEFA 553 cannon, 135 rounds per gun. [Aerospaceweb](https://aerospaceweb.org/aircraft/fighter/miragef1)
gives "five external hardpoints and two wingtip rails". The record has one
centreline row plus three mirrored pairs, which is seven positions: a station
count that agrees with the real five pylons and two rails. The row positions
were not checked against a cited real pylon location (none cited); the shape
decides clipping in MF1-5.

**`AV8.ECM` (row 2).** An internal, fixed, weight-0 jammer suite at the aft
reference point (z -50), the same row the Mirage III and Mirage 5 records carry.
It is not a pod and hangs from no station. TORE already reviews it for the
AV-8B (chaff and flare, jammer generation). The real F1 carries chaff and flare
dispensers on the fuselage shoulders, so an internal countermeasure suite is
consistent. Nothing to decide beyond the ordinary reuse of a reviewed record.

**`ALQ167.ECM` (row 4, B7).** The station is real (the centreline pylon). The
default **pod** is the ALQ-167, an American pod that in this roster rides the
A-6, AV-8B and Kfir (see [aircraft ordnance](aircraft-ordnance.md#external-equipment-pods-name-no-shape-eq-2-2026-11-06)).
Jane's text in `MF1.INF` lists no ECM pod among the F1's loads (only the
fuselage-shoulder chaff and flare dispensers), and the secondary sources found
name French pods (the Syrel ELINT pod on the centreline, Remora and Barem on
other Mirage types) without an authoritative F1 station assignment. So the
record puts a US jammer on a real centreline station. In TORE a fixed pod draws
as a fitted body (12.0 by 1.4 ft for the ALQ-167) under the belly when its row
sits below the reference point, as the Rafale's `AAS38.SEE` does; row 4 is at
-6 ft, so it would draw.

Options (none chosen):

1. **Follow the record:** ALQ-167 default on the centreline row, drawn as the
   fitted pod body; the real station, the record's choice of pod.
2. **Keep the station, drop the pod default:** an empty centreline row that
   takes the 250 gal tank (or nothing); never hang a sensor or designator pod
   there. Opinionated.
3. **Retire the pod row** (a `retire` entry as the Mirage 2000 used for its
   wingtip): no centreline store at all. Loses the real centreline station.

Never hang `PAVESPK.SEE` or `AAS38.SEE` on an F1 (B7; neither row exists).

Update (MF1-5, 2026-11-10): the stations were measured on the scaled shape and
the slice stopped on clipping: the default four `F250.GAS` on row 5 cross each
other and the main gear, and the wingtip Magic enters the tip by 1.48 ft. See
[Mirage F1 stations measured](#mirage-f1-stations-measured-mf1-5-2026-11-10).
The B5-2 remark that the centreline "takes two tanks" means two tank records
(150 and 250 gallon); the row holds one store.

Update (MF1-5b, 2026-11-11): John's round 3 answered both cases. Row 5 holds two
stores (one a side) at 3.24 ft out and the wingtip Magic stands 1.1 ft outboard of
the tip; both clean. See [Mirage F1 stations finished](#mirage-f1-stations-finished-mf1-5b-2026-11-11).

### Mirage F1 real size (for John)

Shape extents at scale 1 (1 unit = 1/3 ft), `MF1.SH` neutral state: x -40 to 40,
y -45 to 82, z -10 to 25 (gear down -17). The x extreme includes the wingtip
rails (wing alone x +-39); the y maximum includes a pitot-style spike from
y 68 to 82 (the radome ends at y 68).

| Axis | Shape | Real F1C | Difference |
| --- | ---: | ---: | ---: |
| Span (rails included), 80 units | 26.7 ft (wing alone 26.0 ft) | 27.6 ft (8.40 m without missiles; Jane's text and [MILAVIA](https://milavia.net/aircraft/miragef1/miragef1_specs.htm) agree) | -3.3% |
| Length, 127 units to the spike tip | 42.3 ft | 50.0 ft (15.23 m, Jane's text), 50.2 ft (15.30 m, [Aerospaceweb](https://aerospaceweb.org/aircraft/fighter/miragef1)) | -15.3% to -15.7% |
| Length to the radome, 113 units | 37.7 ft | same | -24.6% |
| Height, gear down 42 units | 14.0 ft | 14.8 ft (4.50 m) | -5.1% |
| Height, gear up 35 units | 11.7 ft | n/a | n/a |

The Mirage F1 shape is far closer to true size than the Mirage 2000 was (that
shape was 26% narrow and 19% short): span and height are within 5%, and only the
length is short, by about a sixth. Per-axis factors to the cited real figures
would be about x 1.03, y 1.18, z 1.05 (against the Mirage 2000's 1.36 / 1.23 /
1.5). A uniform 1.05 would bring span to 28.0 ft and height to 14.7 ft, and
leave the length 11% short.

Options (none chosen; the Mirage 2000 precedent, John 2026-11-09, was
per-axis real size with one per-aircraft scale):

1. **Leave the shape as authored** (scale 1). Span and height are already
   within 5%; only the length is short. No per-aircraft scale code.
2. **Uniform scale about 1.05.** One factor; span and height within 2%, length
   still 11% short.
3. **Per-axis to the real figures** (about 1.03 / 1.18 / 1.05, fitted from the
   measured extents in MF1-2). The Mirage 2000 method; stretches the nose and
   tail, so the stations, pylons and the gear wheelbase move with it.

Update (MF1-2, 2026-11-10): John chose option 3 in the batch decision round
(per-axis to the real span, length and height); the fit is
`[1.0335, 1.1804, 1.0546]`, see [Mirage F1 shape, real size and
skin](#mirage-f1-shape-real-size-and-skin-mf1-2-2026-11-10).

### Mirage F1 roster note

`MF1.PT` appends last in the batch: checkpoint code 45, `ALL` 45 and
`SELECTABLE` 46 after the batch (B5-1). Wire capacity is not a constraint (9-bit
identity, NET-ID9). `MTF1.PT`, `MIF1.PT` and `AF1.PT` are different records and
are not added; `MR3.PT` (Mirage III) is the record the F1 was built from and
stays a separate identity.

### Mirage F1 unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| Thrust 15,000 / 30,000 against real 11,020 / 15,873 | Climb, acceleration | John, three options above; MF1-8 validates |
| Weights are Mirage III copies | Takeoff load, stores | John (leave or correct) |
| HUD and cockpit choice | MF1-7 cannot start without it | John, three options above |
| Tailplane roll | Rig design | John; cited evidence says pitch only |
| What the original does with `5ae8`/`5aee` | They delete the outboard panel | Unknown; the rig keeps the panel drawn |
| Airbrake as one dorsal plate | Real brakes are paired | MF1-4 rigs the source panel only |
| ALQ-167 pod on the centreline | B7 (pod real for the type) | John, three options above |
| Real pylon x positions | Row 5 sits inside the fuselage width | MF1-5 measures on the shape; no cited real position |
| Real size | Pylon and gear scale | John, three options above |
| `typeSize` 636 | Refusal if missing | B5-1 adds the entry |
| Per-row default quantities | Stores load | MF1-6 |

## Batch 5 identities: F-15E, Gripen, F-5E, Tornado IDS, Mirage F1 (B5-1, 2026-11-10)

Implementation, batch slice B5-1, Claude Sonnet. The five source
reviews above (all 2026-11-10) are now identities. `AircraftId` gains `F15E`,
`Gripen`, `F5E`, `Tornado` and `Mf1`, appended in that order after `M2000`:
checkpoint codes 41 to 45, `ALL` 45 retail identities, `SELECTABLE` 46 entries.
**No record correction and no John decision is applied.** Every record value is
the retail value; every choice John has not made is the minimal interim that
follows the record by name, labelled **interim, agent decision 2026-11-10,
pending John (batch decision round)**. The wire identity is 9 bits since
protocol 83 (NET-ID9), so codes 0 to 45 use 46 of 512 slots and the
compile-time assertion still passes. No other record is aliased: `F15.PT` (the
F-15C), `F5EE.PT`, `F5EV.PT`, `SU24.PT`, the Mirage III and the Rafale keep their
own identities, and the parked path `("MF1.PT", Some(0x5adc))` in
`parked_aircraft.rs` is unchanged.

| | F-15E | Gripen | F-5E | Tornado IDS | Mirage F1 |
| --- | --- | --- | --- | --- | --- |
| Record | `F15E.PT` | `GRIPEN.PT` | `F5E.PT` | `TOR.PT` | `MF1.PT` |
| Parse keys | `f15e`, `f15e.pt` | `gripen`, `gripen.pt` | `f5e`, `f5e.pt` | `tornado`, `tor`, `tor.pt` | `mf1`, `mf1.pt` |
| Checkpoint code | 41 | 42 | 43 | 44 | 45 |
| `typeSize` | 660 (parser default, now an explicit arm) | 660 (explicit arm) | **612 (new arm)** | 660 (explicit arm) | **636 (new arm)** |
| Short / long name | `F-15E` / `F- 15E Strike Eagle` | `Gripen` / `Saab Gripen` | `F-5` / `F-  5E Tiger` | `Tornado` / `Tornado IDS` | `MF1` / `Mirage F1` |
| Shape stem | `F15E` | `GRI` | `F5E` | `TOR` | `MF1` |
| HUD (record) | `AV8.HUD` | `GRIPEN.HUD` | `F29.HUD` | `E2000.HUD` | `RAFALE.HUD` |
| Cockpit stem / picture | `AV8` / `~AV8H.PIC` | `GRI` / `~GRIH.PIC` | `F29` / `~F29H.PIC` | `EF2` / `~EF2H.PIC` | `RAF` / `~RAFH.PIC` |
| PTS companion | none | `GRIPEN.PTS` | none | none | none |
| Radar / gun | `F15R.SEE` / `M61.JT` | `F18R.SEE` / `BK27.JT` | `F18R.SEE` / `M61.JT` | `F18R.SEE` / `BK27.JT` | `F18R.SEE` / `DEFA.JT` |
| Live-fire default stores | `M61`, `MK82`, `AIM120` | `BK27`*, `AGM65G`, `AIM120`, `AIM9M` | `M61`, `AIM7`, `AIM9B` | `BK27`*, `PAVEWAY`, `MK82P3`, `AIM9M` | `DEFA`, `R550` |
| Strike class | Fighter | Fighter | Fighter | Fighter | Fighter |
| Interim rig: CODE bytes / runtime faces | 28,906 / 350 | 28,848 / 383 | 21,488 / 253 | 28,150 / 287 | 19,200 / 236 |
| State words | `80c0 80c6 80cc 80d8 80de` | `8080 8086 808c 8098 809e 80a4` | `63c0 63c6 63cc 63d8 63de 63e4` | `7dc0 7dc6 7dcc 7dd8 7dde 7de4` | `5ad0 5ad6 5adc 5ae8 5aee 5af4` |
| Gear word / faces added | `80cc` +18 | `808c` +22 | `63cc` +6 | `7dcc` +26 | `5adc` +6 |
| Gear low point (shape units) | 27 at 1/3 ft | 16 at 1/3 ft | 18 at 1/3 ft | 11 at 2/3 ft | 17 at 1/3 ft |
| Draw scale | 1/3 ft a unit | 1/3 ft a unit | 1/3 ft a unit | **2/3 ft a unit (header scale 2)** | 1/3 ft a unit |

\* `BK27.JT` is not a supported weapon. It is admitted only as the station's own
default (the Gripen's and the Tornado's internal gun row, flag 8); it stays out
of `SUPPORTED_WEAPONS`, the ordnance catalog and the Cheat catalog, and gets no
weapon work here. The ordnance slices (GRI-6, TOR-6) own the gun's ballistics,
sight and loading. The two guard tests that said no roster aircraft mounts the
decoded cannon records (`loadout.rs` and the retail `hidden_guns.rs` check) now
name `BK27.JT` as mounted by exactly the Gripen and the Tornado.

**Interim choices pending John (all agent decisions, 2026-11-10).**

| Choice | Interim | Why this one |
| --- | --- | --- |
| HUD and cockpit bindings | Each record's own `hudName` and cockpit family (table above) | The original game follows the record. The F-15E and Mirage F1 resolve to glass TORE already measures (`AV8` to the Su-27 mask, `RAF` to the Rafale). |
| Gripen, F-5E and Tornado HUD glass | `hud_aperture.rs` fails closed (empty mask) | Their pictures (`~GRIH`, `~F29H`, `~EF2H`) have no measured polygon. Cockpit slices (GRI-7, F5E-7, TOR-7) measure them. |
| Strike class | Fighter for all five | The rule (top speed over 700 kt, takeoff weight under 150,000 lb) gives Fighter; it is the fleet's class for the F-15C and the Su-24. |
| Tornado draw scale | 2/3 ft a unit | See below. |
| Rig and gear | Gear word only; the other words pinned, unmapped | As the Mirage 2000 interim: the shared fitted gear law until X-3 and X-4. |

Update (F15E-7, 2026-11-10): for the F-15E the HUD and cockpit binding row is
decided, no longer interim: John chose to follow the record, `av8.HUD`, in the
batch decision round, so it is opinionated (John, 2026-11-10). The other four
aircraft's bindings stay as their own slices find them. See [F-15E sensors, HUD
and cockpit](#f-15e-sensors-hud-and-cockpit-f15e-7-2026-11-10).

Update (GRI-7, 2026-11-10): the Gripen's HUD and cockpit binding is decided, not
interim (opinionated, John, 2026-11-10, decision 7), and its HUD glass is measured
(`hud_aperture.rs`, no longer an empty mask). The F-5E and Tornado rows above are
unchanged. See [the GRI-7 section](#gripen-sensors-hud-and-cockpit-gri-7-2026-11-10).

Update (F5E-7, 2026-11-10): for the F-5E the HUD and cockpit binding row is
decided, no longer interim: John chose to follow the record, `f29.HUD`, so it is
opinionated (John, 2026-11-10), and the F-5E's HUD glass row is measured (a polygon
replaces the empty mask). The Gripen's and Tornado's rows stay as their own slices
find them. See [F-5E sensors, HUD and
cockpit](#f-5e-sensors-hud-and-cockpit-f5e-7-2026-11-10).

Update (MF1-7, 2026-11-10): for the Mirage F1 the HUD and cockpit binding row is
decided, no longer interim: John chose to follow the record, `rafale.HUD`, in the
batch decision round (decision 25), so it is opinionated (John, 2026-11-10). The
other aircraft's bindings stay as their own slices find them. See [Mirage F1
sensors, HUD and cockpit](#mirage-f1-sensors-hud-and-cockpit-mf1-7-2026-11-10).

Update (TOR-7, 2026-11-11): for the Tornado the HUD and cockpit binding row is
decided, no longer interim (John followed the record: `E2000.HUD`, stem `EF2`;
opinionated, 2026-11-10), and the Tornado's HUD glass row is measured (a polygon
replaces the empty mask). The Gripen's and F-5E's rows are their own slices'. See
[Tornado sensors, HUD and cockpit](#tornado-sensors-hud-and-cockpit-tor-7-2026-11-11).

**Tornado scale finding.** Before this slice no table drew `TOR.SH` at its
header scale: `tore_sim::combat::debris::scale` returned 1/3 ft a unit for every
identity except the MiG-23, the F-14 (4/3) and the transports, so an imported
Tornado would have drawn at half size (the Tornado review measured 22.3 ft span
at 1/3 against 45.6 ft real). B5-1 adds `Tornado` beside `Mig23` in that
function (2/3 ft a unit) and gives the interim rig spec the same scale, so the
shape, the stores preview, the underside probe and the damage pieces all use
2/3; `aircraft_scale.rs` stays `[1, 1, 1]` (no per-axis fit; that is John's
call). The gear low point is 11 units at 2/3 ft, 7.33 ft. Store rows are
measured in TOR-5 on the drawn shape.

Update (TOR-2, 2026-11-10): the 2/3 draw scale is confirmed in every consumer that
matters, with tests; see [the Tornado shape section](#tornado-shape-scale-and-skin-tor-2-2026-11-10).

**Not done here, by design.** No cockpit measurement, no rig mapping beyond the
gear word, no station, pylon or ordnance review, no flight-model change, no
record correction (thrust, weights, G limits and engine counts stay retail for
John's decision round), no battery list extension. The ordnance audit pins moved
to 45 aircraft as first measurements of five records not yet reviewed station by
station; see the [ordnance availability
baseline](../baselines/ordnance-availability.md#batch-5-identities-join-the-roster-b5-1-2026-11-10).

## Gripen shape, damage looks and decals (GRI-2, 2026-11-10)

Implementation, slice GRI-2 (batch 5), Claude Sonnet. `gri.SH` draws at its own
size: the [source review](#gripen-real-size) measured -0.9% span, -0.5% length
and +1.6% height against the JAS 39A, so the Gripen keeps
`aircraft::scale::shape_axis_scale` at `[1, 1, 1]` (**no scale, no John
decision needed**) and every size consumer (rig, damage looks, stations,
debris, gear low point, parked placement) reads the unscaled shape. The [skin
review](objects-and-shapes.md#saab-gripen-skin-review) records the 383-face
runtime shape, the 19 twin groups (18 double-sided panels with opposite stored
normals plus one zero-area sliver), the four damage looks (`GRI_A.SH` to
`GRI_D.SH`, no registered variant, regional cuts on the intact skin) and the
verdict on all five `E0` decal slots (omitted, each over drawn skin, nothing
clipped or filled). The `render-gripen-skin` battery scenario captures the top
and under views with the gear down; `gripen_shape.rs` (retail-reading, skips
without media) and a `debris.rs` unit test pin the facts. Review frames:
`.local/m3-aircraft/frames/GRI-2/`. Provenance: shape and decal facts
spec-derived from the retail shape; no scale (agent decision, from the measured
sizes). Not touched here: control rigs (GRI-3), gear and brakes (GRI-4),
stations (GRI-5).

## Batch 5 record and station corrections (B5-2, 2026-11-10)

Implementation mode, slice B5-2, John's decision round of 2026-11-10 (opinionated).
The retail reading of every record in the five source reviews above is unchanged
and stays the research record; TORE now overrides the values below where the
record is read, only while the record still holds the retail numbers, and each
override has a unit test (retail still decodes as retail, the identity sees the
correction, other identities and edited records are untouched). The F-15E and the
Gripen have no correction (the F-15E's retail values and the Gripen's 570 rounds
are kept). Behaviour, numbers, sources and the whole-G representation are in
[aircraft records](../spec/aircraft-records.md#batch-5-record-corrections-tornado-ids-mirage-f1-f-5e-b5-2-2026-11-10);
the stations are in [stores on pylons](../spec/stores-on-pylons.md#batch-5-stations-tornado-outer-wing-mirage-f1-centreline-f-5e-defaults-b5-2-2026-11-10).

| Aircraft | Correction |
| --- | --- |
| Tornado IDS | thrust 17,300 / 32,150 lbf (two RB199 Mk 103), empty 31,065 lb, maximum take-off 61,620 lb, G limit +7 (fitted +7 row from the +6 row; real +7.5), outer wing row (`TOR.PT` row 8) retired |
| Mirage F1 | thrust 11,020 / 15,873 lbf (one Atar 9K-50), no default `ALQ167.ECM` on the centreline (row 4 keeps its station and mask); weights retail |
| F-5E | internal fuel 4,400 lb, `maxAlt` 51,800 ft with the 1 G and 0 G top edges moved to match, G limit +7 (the copied +8 row retired; real +7.33), `F104R.SEE` radar in place of `F18R.SEE`, no `F18.ECM` jammer, empty wing pair (no `AIM7.JT`) |

Station indices: none moves. The Tornado row is the last, so it is dropped as the
Mirage 2000's wingtip row was; the Mirage F1 and F-5E rows keep their place and
only their default store changes.

Open for John: the F-5E radar stands in with the F-104's `F104R.SEE` (alternatives
`F4BR.SEE`, `YAK141R.SEE`; the AN/APQ-159 has no record), and clearing the F-5E's
`F18.ECM` also removes its chaff and flares (`F104.ECM` would keep dispensers
without a jammer). Tornado fuel flows (6 and 38 lb/s) are still the Su-24's. The
Tornado's default load has no Sidewinders now that the outer wing pair is retired.

## Gripen stations measured (GRI-5, 2026-11-10)

Implementation and review mode, slice GRI-5 (batch 5), Claude Sonnet (sonnet-high).
This answers the "things GRI-5 must handle" list of the
[Gripen hardpoints](#gripen-hardpoints) above; the numbers, frames and options are
in [stores on pylons](../spec/stores-on-pylons.md#the-saab-gripen-gri-5-2026-11-10).
**Stopped on clipping** (John's standing rule): no station, mask or count changed
and the audit pins did not move. Measured with `tore-app --stores-preview` (audit,
catalog) and a local triangle-level check of 61 single-row loads, eight
whole-aircraft loads and the default, gear up and down.

| Question from the review | Measured |
| --- | --- |
| Wingtip row 3 units (1 ft) outboard of the shape's tip rail | The standard tip rule centres the missile on the tip's edge, 13.67 ft out (record 14.67 ft): the offset is absorbed, nothing to move. Residual entry 0.15 ft for the AIM-9M. |
| Centreline tank 4 ft under the belly, close to the nose gear doors and main gear | The tank hangs on the 2 inch rail (1.28 ft above the ground, 0.16 ft from the belly, no airframe entry). Gear down it crosses the left main strut plate by 0.25 ft; the right plate clears by 0.09 ft; the nose gear is clear. |
| Rows 6 and 7 under the wing (x 5 and 10 ft) | Row 7 is clean (gear up and down, twin Amraams 0.02 ft apart on the standard bar). Row 6 hangs at x 5.0 ft, where the open main gear door wall ends: every store on row 6 crosses it gear down (default AGM-65: 36 and 44 crossings). Gear up it is clean (0.59 to 0.64 ft from the wing). |
| Gun row at x +4 (starboard) while the real gun is port | Row 4 is 1.1 ft inside the starboard skin (fuselage section x -2.4 to 2.7 ft at that station); nothing is drawn. Left as retail for GRI-6 to decide (options in the spec). |
| AAS-38 row 3 (flags `0x8` at the origin) | A placeholder: no body is drawn and the FLIR sight stays (John, 2026-11-10, on the 2026-11-07 rule); retail test extended to the Gripen. Its 350 lb was never in the mass sum (rows flagged `0x8` are skipped, as for the A-10 and F-16C pods): unchanged, noted for GRI-6. |
| Designator pod station | None exists in the record and none was added. |

Canards (neutral) are at least 2.56 ft from every store measured; the ground is
clear in every load (lowest 0.20 ft for a GBU-10 on the centreline). Three clipping
cases wait for John (spec section): the inner wing row against the open gear doors,
the centreline tank against the left strut plate (both gear down only), and the
wingtip missiles' 0.15 ft entry into the thin tip (the last is below every reviewed
default load and listed for completeness).

Provenance: positions and masks spec-derived (retail `GRIPEN.PT`, `GRI.SH`); the
measurements and the classification agent findings, 2026-11-10; the real station count
(seven in the in-game encyclopedia, eight on public pages) plausibility only.

## Gripen ordnance (GRI-6, 2026-11-10)

Update to the B5-1 and Gripen review notes above (they are left as written).
`BK27.JT` is now proven as the Gripen's working internal gun: it was already in
`AircraftId::Gripen.guns()` and the live-fire list, which is all the shared gun
path needs, so no gameplay code changed. It stays out of `SUPPORTED_WEAPONS`
and the catalogs, and the Tornado shares the record (its own slice verifies it).
Record values: 27 mm, 570 rounds (John kept the count), 2,933 ft/s, 2 rounds a
shot, 32 rounds a second through the shared fitted cadence, damage 25 / 2 / 7 / 7 /
25, own sound `&FSTGUN3.11K`, red tracers. Against the real Mauser BK 27 (27 mm,
1,000 to 1,700 a minute, about 1,100 m/s, 120 rounds on the Gripen; Wikipedia
"Mauser BK-27", accessed 2026-11-10) TORE's cadence is about 13 percent above the
top real rate and its muzzle velocity about 18 percent below; neither is Gripen
specific and neither was changed. Default load: `F150.GAS` centreline,
`AGM65G.JT` x2 inner wing, `AIM120.JT` x4 outer wing pair, `AIM9M.JT` x2 tips,
the same for the player, AI and saved loads. Countermeasures 30 chaff and 30
flares from `F18.ECM` (fleet convention); the real BOL and BOP dispensers have
no verified official count (finding only). Live fire: gun, AIM-9M, AIM-120 and
AGM-65G each fire and kill; release origins are the GRI-5 hang points left then
right; the tank and Mk 82s jettison. Details:
[aircraft ordnance](aircraft-ordnance.md#the-saab-gripen-gri-6-2026-11-10) and
[the baseline](../baselines/ordnance-availability.md#saab-gripen-ordnance-gri-6-2026-11-10).
The guard tests keep expecting exactly the Gripen and the Tornado for
`BK27.JT`, with GRI-6 notes.

## Mirage F1 shape, real size and skin (MF1-2, 2026-11-10)

Implementation, slice MF1-2, Claude Sonnet (sonnet-high). John's decision of
2026-11-10 ("per-axis scale to the real span, length and height, as the Mirage
2000"). `mf1.SH` is scaled per axis to the real Mirage F1C: factors
`[1.0335, 1.1804, 1.0546]` (x right, y forward, z up), fitted from the measured
gear-down extents (80 by 127 by 42 units) to 8.40 m span, 15.23 m length **with
the nose probe** (the shape's own y maximum includes its probe spike, so the two
use one definition) and 4.50 m height. The table row, the fit, the definitions
and the consumers (drawn aircraft, stores preview, damage looks, station rows,
debris, gear low point 17 to 17.93 units, wingtip vapor, parked placement) are in
[Mirage F1 shape scale](objects-and-shapes.md#mirage-f1-shape-scale-mf1-2-2026-11-10)
and the [skin review](objects-and-shapes.md#mirage-f1-skin-review) (23 exact twin
groups, two omitted wing-marking decal slots). The parked path
`("MF1.PT", Some(0x5adc))` is unchanged: the two Mirage F1s on
`FRA QFLAIR` keep their identity, 95 hit points and gear word, and now stand at
`origin-up` 5.98 ft on the scaled shape. The `render-mf1-skin` battery scenario
captures the top and under views. Provenance: choice opinionated (John,
2026-11-10), factors fitted. Later slices use the scaled geometry: MF1-3 and MF1-4
scale any shape-unit hinge point their rigs name, MF1-5 measures the stations.

## Tornado shape, scale and skin (TOR-2, 2026-11-10)

Implementation, slice TOR-2, Claude Sonnet (sonnet-high). John's decision of
2026-11-10 (draw at the header scale, true size) is confirmed in code and
frames; nothing in the draw path needed to change beyond B5-1's `Tornado`
entry in `debris::scale`. The Tornado is drawn from `TOR.SH` of `FA_2.LIB`
(the toolkit patch archive's different `TOR.SH` is never imported), at 2/3 ft a
unit with the identity per-axis scale: 44.7 ft span (real 45.6), 54.0 ft long
(54.8), 20.0 ft high (19.5). Every consumer was checked: the drawn aircraft,
the stores preview and station underside, the damage looks and their section
extents, the damage bodies and pieces, breakup offsets, gear low point (7.33
ft), contrail offsets and the wingtip vapor point (22.67 ft, on the drawn wing
tip) all use 2/3; no consumer is at 1/3. The table, the skin review (22 twin
groups, 21 with opposite normals, the centreline sliver pair `255c` and `257a`
that both draw), the two omitted `E0` fin decal slots (`2ce3`, `2e3d`) and the
damage looks are in [the Tornado shape, scale and skin
review](objects-and-shapes.md#tornado-shape-scale-and-skin-review). The
`render-tornado-skin` battery scenario captures the top and under views with
the gear down. Frames: `.local/m3-aircraft/frames/TOR-2/` (local only).

The MiG-23 (the other header-scale-2 aircraft in the roster) draws at the same
scale beside it. The Su-24 is not a roster aircraft (`SU24.PT` is only the
Tornado's template record), so it cannot be drawn for comparison.

Later slices use the 2/3 scale as it stands: TOR-3 and TOR-4 work in shape
units at 2/3 ft; TOR-5 measures the stations on the drawn shape; a rig that
names a hinge in CE (third-of-a-foot) units divides by 3 as the streamer does,
and a hinge in shape units uses the shape's own units. The wing stays the fixed
spread pose (no sweep word), so the sweep rig is TOR-3's. No whole-body damage
variant is registered (open decision, as for the Mirage 2000 and the MiG-25).

## Batch 5 follow-up corrections (B5-3, 2026-11-10)

Implementation mode, slice B5-3, John's decision round 2 of 2026-11-10 (items D2,
D3 and E3, all "follow recs", opinionated). This adds to the B5-2 note above; that
note's open items for the F-5E countermeasures and the Tornado fuel flows are
answered here and the earlier text is kept as written.

| Item | Change | Where |
| --- | --- | --- |
| D2 F-5E countermeasures | Row 2 (`F5E.PT`) holds `F104.ECM` (20 chaff, 20 flares, no deception jammer) in place of the empty row B5-2 left; the real F-5E carries an AN/ALE-40 dispenser. No station index moves; only the retail rows take it. | [stores on pylons](../spec/stores-on-pylons.md#batch-5-stations-tornado-outer-wing-mirage-f1-centreline-f-5e-defaults-b5-2-2026-11-10) (Update line) |
| D3 Tornado fuel flows | 6 / 38 lb/s (the Su-24's) become 3 / 22 lb/s, two RB199 Mk 103 at 17,300 / 32,150 lbf with the cited dry 0.598 and reheat 2.5 lb/(lbf h). Full tank: 31.2 to 62.3 min military, 4.9 to 8.5 min afterburner. | [aircraft records](../spec/aircraft-records.md#batch-5-follow-up-corrections-tornado-fuel-flows-b5-3-2026-11-10) |
| E3 F-15C nose gear | The nose gear folds forward (the real F-15's nosewheel retracts forward) as one rigid piece about Y=48, Z=-8, grounded at gear down. The F-15C probe and `flight-animation-f15` pass. | [control surfaces](aircraft-control-surfaces.md#f-15c-source-additions) (Update line), [F-15C fit](../spec/variety-animation.md#f-15c-control-and-device-fit) |

No audit pin moves (the F-5E row keeps its place and the audit counts weapon rows
only): 45 aircraft, 18 recovered weapon stations, 4,652 weapon placements, 233 tank
placements, 220 Cheat-only placements, 80 pod placements. Each correction fires only
on the retail record and has a unit test.

## F-15E ordnance, default load and countermeasures (F15E-6, 2026-11-10)

Slice F15E-6 confirmed the F-15E's ordnance as the record's own, with no row, mask,
capacity or default authored: the M61 gun at 450 rounds (the real jet's 512 is a
finding only; John kept retail), `F350.GAS` on the centreline, twelve `MK82.JT` on the
belly pair, four `AIM120.JT` on the wing pair, and 30 chaff and 30 flares from
`F15.ECM` (the fleet's convention, the record is the capacity). The F15E-5 belly rail
drop and refused stores are in place in these releases. Live fire (gun, Mk 82 and
AIM-120 kill), release positions and rack order, the tank-then-bombs jettison and a
Pave Spike laser bomb that destroys its tank are in [aircraft
ordnance](aircraft-ordnance.md#the-f-15e-strike-eagle-f15e-6-2026-11-10) and the
[ordnance availability baseline](../baselines/ordnance-availability.md#f-15e-ordnance-f15e-6-2026-11-10).
No audit pin moved. Update to the F15E-5 "default load" lines above: the gear leg
crossing the aft inboard bombs in transit is still open.

## F-15E flight review (F15E-8, 2026-11-10)

Slice F15E-8 validated the F-15E headless against the record, TORE's F-15C and the
cited real aircraft; nothing in the flight model or the record changed (John kept
the retail copy of the F-15C's numbers, decision 1). The sim reads the record only,
and the F-15E's flight words equal the F-15C's, so every F-15E result equals the
F-15C's on the same load; the two differ by default load alone (9,940 against
2,152 lb). Details, tables and the battery windows are in the
[variety flight baseline](../baselines/variety-flight.md#f-15e-validation-2026-11-10)
and the [spec](../spec/variety-flight.md#f-15e-f15e-8-2026-11-10).

Summary (afterburner, full internal fuel, clean 42,262 lb): Mach 2.29 at 36,000 ft
against the record's Mach 2.5 edge (0.916 of it, the shared loaded-drag rule), 750
kt at 1,000 ft against about Mach 1.2 at sea level, level flight to about 95,000
ft and a climb peak of 108,921 ft against a published 60,000 to 65,000 ft service
ceiling (the record's 100,000 ft, a retail copy); best sustained climb 36,000
ft/min against more than 50,000 published (about 71 percent, the shared drag law);
full-aft-stick pull at 461 kt and 1,000 ft 7.57 / 8.28 / 8.69 / 9.10 G at full,
half, quarter and near-empty fuel (clean; 6.73 to 7.91 G with the default load);
liftoff 149.1 kt after 1,155 ft and approach 156.1 kt with the default load.

Findings for John, nothing changed: the ceiling, the +9 G reached only near empty
(the fleet loaded-elevator rule, as the Mirage before M2K-11), fuel flows 39 percent
under the real F100 at military power, and the default load's weight.

## F-15E in missions, multiplayer and replay (F15E-9, 2026-11-10)

Implementation and verification, slice F15E-9 of the F-15E port. No game code
changed: `AircraftId::F15E` was appended to every identity list in B5-1
(checkpoint code 41, wire index 41 of the `SELECTABLE` order), the wire codes it in the 9-bit identity field since protocol 83,
and `strike_class.rs` already lists it in the fighter family. The AI builds a
wing member from the record like any aircraft (two engines, the default AI load
check passes for its Mk 82, AIM-120 and M61 stores), so no AI behaviour work
was done.

Added coverage: a Quick Mission test
(`the_f15e_is_a_quick_mission_player_and_a_wing_choice`) naming `F15E.PT` as the
player's aircraft and in every AI wing field and refusing the F-15C in its
place, the `ai-f15e-quick-mission-2v2` and `replay-rec-f15e-2v2` battery
scenarios, the F-15E row in the Load Ordnance notes loop
(`menus-snap-quick-lobby-ordnance-laser-notes-f15e`: no laser warning, sensor
helper shown, as the F-16C, because the record has the PAVESPK.SEE row; decision
3 of the F-15E plan keeps that sensor without a drawn pod), and `ai-f15e-*` in
the `ai-fights` battery family.

AI strike finding (reported, no behaviour work). The strike class is Fighter, as
the F-15C's and the F-16C's. In a Quick Mission with a ground target set, the
mission gives a fighter the CAP role: `--a2g-run UKR QUFACT --pilot ai
--aircraft f15e --load MK82 --quick-wing --seconds 300` prints `ROLE plane=1
role=cap`, `releases=0` and `store_rounds_left=12` (the twelve Mk 82s untouched),
the same as the F-16C of `qmc-strike-wing`. The Mk 82 load is not refused on
that account: when the player orders the wingman onto the ground (`--order
engage-my-target --order-at 3` on `UKR QUCOL`), the order is taken ("1
applied"), the wingman flies a level pass, releases a Mk 82 at 5,027 ft and 401
kt, lands it 123 ft from the aim, egresses and keeps 11 rounds. So an AI F-15E
bombs when ordered or assigned the Strike role and otherwise flies as a fighter.
Whether the Strike role should default for an aircraft carrying a bomb load is
an AI behaviour question and is left to John.

Results (2026-11-10, throttled, profile `.local/m3-aircraft/profile-f15e`):

- `--ai-roster-probe-ticks 3600`: the four `F15E.PT` lines (Novice, Average,
  Experienced, Ace) each `models=2 radar=true ir=true stores=3 gun=true
  projectiles=0 dropped=0 PASS` (two engines; the `ir` is the AV8IR.SEE row).
- `ai-f15e-quick-mission-2v2` passes: every wing the F-15E, one missile kill by
  the player at 14.9 s, nothing dropped, no warnings.
- `replay-rec-f15e-2v2` passes: the recording reads back `player.aircraft =
  F15E.PT`, `probe.enemy = F15E.PT`, `draw.models = F15E.PT`, the aircraft
  listed as `F-15E (F15E.PT)`, two runs identical, `--verify-render` finds
  nothing missing or differing.
- `net-content-missing` and `net-window-lobby` pass after rebuilding
  `tore-server` **and `tore-bot`** (`cargo build --locked -p tore-session --bin
  tore-bot`): with the 2026-11-10 18:31 bot, the same server and bot counted the
  same 213 content items but derived different compatibility codes, and the bot
  was refused. Rebuild both before a net scenario after the identity list
  changes.

## Gripen sensors, HUD and cockpit (GRI-7, 2026-11-10)

Implementation and review mode, slice GRI-7 (batch 5). The Gripen flies its own
`GRIPEN.HUD` and its own 13 `~GRI*.PIC` cockpit pictures (John, 2026-11-10,
decision 7, opinionated), the F/A-18's reviewed radar and jammer records, and the
AAS-38 infrared channel. B5-1 had bound the names by record as an interim; this
slice makes the cockpit family work end to end and relabels the binding as decided.
Gameplay code changed in two small places (the mirror table and the HUD glass,
both for the Gripen's 1279-wide picture); the sensor records needed none.

**Sensors (spec-derived, retail records, already reviewed).** `--sensor-summary`
reads: `Gripen: radar F18R.SEE search 90 nmi 60x60 deg track 50 nmi 45x45 deg
look-down 0 ADVANCED; infrared AAS38.SEE search 15 nmi 60x60 deg track 15 nmi 45x45
deg; visual VIS340.SEE search 10 nmi 170x140 deg track 5 nmi 170x140 deg; jammer
F18.ECM LATE strength 0.30; signature radar 100 infrared 90`. The record's four
fixed sensor rows are `VIS340.SEE`, `F18R.SEE`, `F18.ECM` and `AAS38.SEE` (pinned
by `crates/tore-import/tests/gripen_cockpit.rs`, retail-reading, skips without
media). The infrared line equals the Rafale's (the same `AAS38.SEE` record); the
signature numbers are the Gripen record's own. No `sensors/profile.rs` arm was
needed and nothing under `crates/tore-sim/src/sensors` branches on the aircraft.
Live fire with the radar window open (`--live-fire --aircraft gripen --weapon-slot
3`): the Radar window reads `TWS 10NM 1 CONTACTS TRACK`, the HUD shows `ARM 3
AIM-120`, `92% IN RNG`, the AIM-120 locks (`T2 HP 99 LOCK`) and at 3,600 ticks
`shots=1 hits=1 kills=1`; the AIM-9M (slot 4) hits and kills (`shots=1 hits=1
kills=1`, 700 ticks); the gun (slot 1) fires 187 rounds with 59 hits and 1 kill in
700 ticks. With `--sensor-channel ir` the Radar window reads `IR IR 10NM 1 CONTACTS`
(the AAS-38 channel; the radar-guided AIM-120 has no lock on it, as for the F-15E
and Rafale). With `--jammer-on --combat-command incoming` the RWR lights `JAM`. The
air-to-ground Maverick is designated, CUED, launched and kills the column's lead
truck (the existing `a2g-maverick-gripen` scenario, passed again in this slice).
Because `fixed_sight` takes any infrared or laser `.SEE` row, the AAS-38 is the
Gripen's FLIR sight (no laser, so the sight is the infrared one, as the Rafale's);
the target window FLIR pages draw (frames). No pod body is drawn: row 3 is flags
`0x8` at the reference point (decision 8).

**HUD and cockpit wiring (opinionated, John, 2026-11-10).** `AircraftId::Gripen`:
`hud()` `GRIPEN.HUD`, `cockpit_stem()` `GRI`, `cockpit()` `~GRIH.PIC`,
`instrument_panel()` `~GRI_P.PIC` (81 x 80). `Hud::parse` of `GRIPEN.HUD` names
`~gri_p`, which `Airframe::load` requires. All 13 pictures exist and are pinned in
the test with sizes: `~GRI` 535 x 205, `~GRIH` **1279** x 490, `~GRIS` 1601 x 613,
`~GRI_L/C/R` 61 x 109, 130 x 30 and 61 x 109, the `_LH/_CH/_RH` and `_LS/_CS/_RS`
overlays and the panel. The loader reads every pixel size from the picture, so the
1279 width needed no renderer change; the only code that assumed 1280 were the
mirror flood (`mirrors::masks`) and the HUD glass (`hud_aperture::mask`), which
accept the Gripen's 1279x490 now (any other width still fails closed).

**HUD aperture (fitted, measured, GRI-7).** The glass is the transparent area
inside the teal combiner outline and between its two slanted posts, down to the
top of the base bar. The polygon (art pixels, 1279x490) is `(632,118) (583,124)
(531,136) (501,148) (477,178) (466,190) (466,262) (469,300) (510,383) (770,383)
(811,300) (814,262) (814,190) (803,178) (779,148) (749,136) (697,124) (648,118)`:
the arc over the top (the outline's inner edge at x = 640 is y = 118), the
vertical sides at x 466 and 814, the posts sloping in, and the bar at y 383. It was
measured from the transparent run per row in the imported `~GRIH.PIC` (left and
right edges per ten rows, 467 to 813 on the straight section, 510 to 770 at y 380).
Opaque art (the teal outline, posts) is excluded by the existing alpha test. It is
pinned by `hud_aperture::tests::the_gripen_glass_is_measured_for_its_1279_picture`
(inside and outside points, left and right symmetric, an opaque pixel stays out, a
1280x490 picture fails closed). The front frame shows the HUD inside the glass.
This is a fitted aperture (the fleet's convention for every family); the real
Gripen HUD's own glass extent is not known.

**Rear-view mirrors (fitted, GRI-7).** `~GRIH.PIC` has three flat blue-grey
(117, 138, 154) mirror glasses, one flat colour. `mirrors::seeds` gains the `GRI`
family: centre (640, 44), left (245, 304), right (1040, 304). `--cockpit-sheet OUT
GRIPEN,RAFALE,F18` reports `gripen: stem GRI cockpit ~GRIH.PIC art 1279x490 FITTED
CLR`, centre x 481 y 8 w 316 h 77, left x 169 y 174 w 151 h 267, right x 959 y 174
w 151 h 267, and the cockpit prints `Cockpit mirrors: 3 reviewed regions`. Before the
change it said `NO MIRRORS FITTED` (the guard demanded 1280 wide). Pinned by
`mirrors::tests::the_gripen_picture_is_1279_wide_and_floods_three_mirrors` and the
seed-table test.

**Views (windowed).** Forward cockpit (`~GRIH.PIC` with the HUD inside the glass and
live mirrors), look left and right at 45 degrees (the art turns away with the look),
90 degrees left and right and 60 degrees up (no cockpit art, open sky), F3 Up and
F2 Back all draw as for the other families. F2 Back shows the Gripen's spine and wing
roots from the pilot's eye. The Gripen's own
instrument bezel (`~GRI_P.PIC`, a dark frame with dark buttons) is drawn around the
four windows, or six on the small layout. The standalone pages 0 to 9 render with it.

**Instruments and the single engine.** The Systems window shows one engine readout,
as every aircraft's does: `THR 70%`, `TEMP 0%`, `OIL 100%`, `HYD 100%`, `FUEL 5000
LBS` and `(+ EXT 990 LBS)` with the `F150.GAS` centreline tank (the Gripen's own
record, internal 5,000 lb; 989 LBS after a few live-fire seconds). The Weapons window lists
`570 BK27`, `2 AGM-65`, `4 AIM-120`, `2 AIM-9M`, `30 CHAFF` and `30 FLARE`. HUD
airspeed and altitude read the Gripen's own state (507 kt and 5,000 ft at the
live-fire start).

**Eye point (agent decision, 2026-11-10, left unchanged).** `mirrors::pilot_eye` is
the one fixed offset for every aircraft (7 ft up, 10 ft ahead of the model origin;
see [the Mirage section](#mirage-2000-sensors-hud-and-cockpit-m2k-7-2026-11-10) for
the call sites). Measured on the neutral shape dump (1/3 ft a unit, forward +Y,
up +Z), the topmost faces within 4 units of the centreline are 5 units (1.7 ft)
over the aft canopy, 7 units (2.3 ft) at 20 to 30 units ahead and 8 units (2.7 ft)
at 41 to 47 units ahead of the origin, i.e. a canopy top 2.3 to 2.7 ft above the
origin between 6.7 and 15.7 ft ahead of it. The fixed eye (7 ft up, 10 ft ahead) sits
about 4.5 ft above the canopy and over its middle. The F2 Back frame
(`rafale-view-back-f2-comparison`) shows the spine and wing roots the way the
Rafale's does. Not changed; a seated eye would be a fleet change (one eye table
keyed by aircraft).

**RWR with a threat.** As in the Mirage note: no scripted path puts an emitter on
the RWR; the frames show the window with the jammer lit and the incoming round at the
centre. Evidence and frames: `.local/m3-aircraft/frames/GRI-7/`. Not run: an RWR with
a plotted emitter, the menu and net batteries.


## Gripen flight review (GRI-8, 2026-11-10)

Dated section, additions only. The Gripen flies the hybrid adapter's own `variety`
configuration built from `GRIPEN.PT` (`gri.SH`), one RM12 at the record's
12,140 lbf military and 18,100 lbf afterburner. No envelope, G, weight, fuel, flow,
ceiling or thrust value was changed and no flight constant was touched; the GRI-0
verdict (genuine JAS 39A) stands and John kept the 60,000 ft ceiling and the -4
limit (decision 10).

- Top speeds: the 1 G row is 1,145 kt at 36,000 ft (Mach 2.0, the published
  figure) and 739 kt at 1,000 ft (Mach 1.10, under the published Mach 1.15 to 1.2).
  Full internal fuel settles at 0.952 of the edge: 1,090 kt (Mach 1.90) at
  36,000 ft, 1,065 kt (Mach 1.86) at 50,000 ft, 704 kt at 1,000 ft. Level flight
  is held to 60,000 ft; the published ceiling is 50,000 ft.
- Climb and acceleration: peak specific excess power 445 ft/s at sea level
  (26,700 ft/min), best sustained climb 427 to 432 ft/s, 36,000 ft in 107 to 123 s.
  This is the same shape the Mirage 2000 showed (418 ft/s at the same thrust to
  weight), produced by the shared drag law and the missing ram thrust, not by
  the thrust or weights, which are the real ones. No published C/D climb rate was
  found, so no factor to a published figure is stated. Not changed; a fleet-level
  drag or ram fit remains John's call.
- G: the record's own +9 row gives 8.00 G at 461 kt and 1,000 ft with full fuel
  and 9.03 G nearly dry (F-16A 7.67 to 9.09 G, Mirage 8.0 to 9.09 G), 7.26 G with
  the default load; the push is -3.55 G clean (real limit -3, decision 10 keeps -4).
  The sustained turn at 461 kt and 1,000 ft is 4.8 G, 10.4 deg/s (F-16A 7.15 G,
  15.4 deg/s; Mirage 4.13 G, 9.6 deg/s).
- Fuel: record flows 2 and 14 lb/s kept; 41.7 minutes dry and 6.0 at afterburner
  from 5,000 lb, against about 29 and 9.3 minutes at the RM12's published specific
  fuel consumption.
- Takeoff and landing: liftoff 132.1 kt after 1,097 ft (334 m), approach 147.5 kt,
  touchdown 142.0 kt, rollout 589 m; the record has no hook, correct for the real
  aircraft (Saab dropped the hook, brake chute and thrust reverser).

The liftoff (125 to 150 kt) and approach (140 to 160 kt) ranges in the flight
battery are fitted windows, agent decisions of 2026-11-10. Full tables, sources and
commands are in the
[variety flight baseline](../baselines/variety-flight.md#saab-gripen-validation-2026-11-10).

## Gripen in missions, multiplayer and replay (GRI-9, 2026-11-10)

Implementation and verification, slice GRI-9 of the Saab Gripen port. No game
code changed. `AircraftId::Gripen` was appended to every identity list in B5-1
(checkpoint code 42, `SELECTABLE` index 42, carried in the 9-bit wire field
since protocol 83), `strike_class.rs` lists it in the fighter family (B5-1), and
the AI wing build reads the record's single RM12 engine like any other fighter,
so an AI Gripen carries the record's default load (BK 27, AGM-65G, AIM-120,
AIM-9M and the `F150.GAS` tank). No AI behaviour work was done. The Gripen is
not a parked target, so there is no parked check.

What changed is coverage: a Quick Mission test
(`the_gripen_is_a_quick_mission_player_and_a_wing_choice`) naming `GRIPEN.PT` as
the player's aircraft and in every AI wing field, the
`ai-gripen-quick-mission-2v2` and `replay-rec-gripen-2v2` battery scenarios, the
Gripen row in the Load Ordnance notes loop
(`menus-snap-quick-lobby-ordnance-laser-notes-gripen`: the lobby page warns about
a laser-guided store and offers the pod helper, the same as the Mirage 2000,
the F-16A and the MiG-31), and `ai-gripen-*` in the `ai-fights` battery family.

Results (2026-11-10, throttled, profile `.local/m3-aircraft/profile-gripen`):

- `--ai-roster-probe-ticks 3600`: the four `GRIPEN.PT` lines (Novice, Average,
  Experienced, Ace) each `ticks=3600 models=2 radar=true ir=true stores=5
  gun=true projectiles=0 dropped=0 PASS` (the infrared sensor shows because the
  Gripen's record carries `AAS38.SEE`).
- `ai-gripen-quick-mission-2v2` passes: every wing the Gripen, one missile kill
  by the player, two kills in all, nothing dropped, no anomalies.
- `replay-rec-gripen-2v2` passes: the recording reads back `player.aircraft =
  GRIPEN.PT`, `probe.enemy = GRIPEN.PT` and `draw.models = GRIPEN.PT`, two runs
  are identical and `--verify-render` finds nothing missing or differing.
- `net-content-missing` and `net-window-lobby` pass after rebuilding
  `tore-server` and `tore-bot` (the battery needs both rebuilt after any change
  to the identity list). They take no aircraft argument, so they cover the
  identity list and content code, not a Gripen in flight over the wire.

## F-5E ordnance, default load and countermeasures (F5E-6, 2026-11-10)

Slice F5E-6 confirmed the F-5E's ordnance as the record's own plus the guarded B5-2
and B5-3 defaults, with no gameplay code change. This answers the F-5E review's
"default AIM-7 on the wing (not real)" and "radar and jammer F18 pair" findings (the
corrections are in the B5-2 and B5-3 notes above; the earlier text is kept as
written). Full tables, positions and results are in
[aircraft ordnance](aircraft-ordnance.md#the-f-5e-tiger-f5e-6-2026-11-10).

| Item | Result |
| --- | --- |
| Default load (human, AI, saved loads, lobby) | `M61.JT` x560, `F150.GAS` centreline x1, empty wing pair, `AIM9B.JT` x2 on the wingtips; 15,365 lb of 24,664 lb with 4,400 lb of fuel |
| Gun | 560 rounds equal the real two M39A2 with 280 each (`F5E.INF`); the shared gun cadence (about 32 rounds a second) is unchanged |
| Countermeasures | `F104.ECM`: 20 chaff and 20 flares, shown on the weapons page; one chaff leaves 19, one flare leaves 19 |
| Live fire | gun 320 shots, 47 hits, 1 kill; AIM-9B one shot, one hit; an AGM-65G from the wing pair kills the column's lead truck |
| Release origins | wing pair 8.02 then 6.64 ft out (Mk 82) and 8.18 then 6.48 ft (AGM-65G), 2.8 ft down, 1 ft pylon, left first then alternating; wingtip AIM-9Bs 14.33 ft out, left then right |
| Jettison | the tank (990 lb) drops first, then the four wing-pair bombs |
| Audit | 4 source rows, 1 optional weapon row, 71 weapon placements, 5 tank placements; no pin moved |

Open for John: the wing pair's empty default means a bomb run must name the station
(`--station 2`); the real F-5E fit sources for the dispenser are second hand (see the
B5-3 note); the cadence is shared, not F-5E specific.

## F-5E sensors, HUD and cockpit (F5E-7, 2026-11-10)

Slice F5E-7 (implementation mode) made the X-29's cockpit family work for the
F-5E and confirmed its sensors. John's decision round of 2026-11-10 decided the
review's [HUD and cockpit question](#f-5e-hud-and-cockpit-for-john): **follow the
record**, `f29.HUD`, cockpit stem `F29`, forward art `~F29H.PIC` (the game's own
encyclopedia says the X-29s were built from F-5E fuselages; no F-5E art exists).
That settles what the batch identity slice wired as an interim agent decision, so
the binding is now **opinionated (John, 2026-11-10)**; the stem arm and the
picture names already existed from B5-1 and did not change. The F-5E is the first
aircraft to use the family: TORE has no X-29 (`F29.PT` is not on the roster), and no
other identity names `F29.HUD` or stem `F29` (checked by the new test). Changed:
the glass polygon (`hud_aperture.rs`), a `"F29"` mirror seed row (`mirrors.rs`), two
comments in `aircraft.rs` and a retail-reading test; the docs listed in the log.

**Sensors (spec-derived, retail records and the B5-2 and B5-3 defaults).** The
fixed rows are `VIS340.SEE`, `F104R.SEE` (the F-104's radar as the AN/APQ-159
stand-in, John's pick D1) and `F104.ECM` (20 chaff and 20 flares, no deception
jammer, John's pick D2). `--sensor-summary` reads: `F5E: radar F104R.SEE search 50
nmi 60x60 deg track 25 nmi 45x45 deg look-down 50 BASIC; infrared none; visual
VIS340.SEE search 10 nmi 170x140 deg track 5 nmi 170x140 deg; jammer F104.ECM EARLY
strength 0.00 (no RF mode); signature radar 100 infrared 100`. The radar and
jammer are the F-104's line record for record (`--sensor-summary` of `F104`); the
visual sensor is the F/A-18's (`VIS340.SEE`). Nothing under
`crates/tore-sim/src/sensors` branches on the aircraft, so the radar modes, the lock
and the RWR behave as for the other aircraft. A live-fire run (`--live-fire
--aircraft f5e --weapon-slot 2`) shows the Radar window `TWS 10 ... TRACK`, the
HUD `ARM 1 AIM-9B 89%`, the lock frame on the target and the target camera `F-
5E TIGER 1.1 NM`; at 1,500 ticks the round has flown and hit (`shots=1 hits=1
kills=0 active=0`: a single AIM-9B hits and does not kill the practice aircraft, as
F5E-6 found). `--scope-range 25` redraws the scope at 25. There is no infrared
channel, and no jammer for the RWR window to light (strength 0.00). The diagnostic
incoming round plots at the window centre; no scripted path plots an emitter on
the RWR, as for the Mirage 2000 and the F-15E (the RWR code is aircraft-agnostic and
unit tested). Countermeasure counts show on the Weapons window as `20 CHAFF` and
`20 FLARE` (F5E-6).

**HUD glass (measured, fitted).** `hud_aperture::mask` now has a polygon for the
F-5E, replacing the empty mask the B5-1 interim left (the aperture fails closed
for a picture of any other size). `~F29H.PIC` is 1280 x 490 with a 64-colour
palette. Its glass is the transparent opening between the two canopy struts, under
a thin combiner arc and above the coaming. Measured on the imported art (every
row's transparent run through x 640, plus the first opaque pixel of columns 520 to
760): the strut inner edges are at x 522 and 759, the arc line runs from y 125 at
the struts to y 105 at the centre (one pixel thick), the coaming top is at y 375
to 376 (bumps near x 556 and x 646 rise to y 368 and y 355). The fitted polygon
(522,132) (535,127) (550,120) (565,116) (580,112) (610,108) (640,107) (670,108)
(700,112) (715,116) (730,120) (745,127) (759,132) (759,372) (522,372) runs under the
arc and between the struts; the shared rule keeps only the transparent pixels
inside it, so the struts, the arc line and the coaming bumps stay out. Pinned by
two tests in `hud_aperture.rs`: a synthetic one (centre in, above the arc, outside
the struts and below the coaming out, the Gripen and Tornado still empty) and a
retail-reading one that skips without an install: on the real art the mask never
covers an opaque pixel, has its left edge within x 520 to 530, its right edge within
x 752 to 760, its top within y 105 to 112 and its bottom within y 365 to 372, is one connected
region, and has exactly 58,556 pixels. The HUD draws inside the glass: the
heading tape under the arc, the airspeed and altitude boxes (`497` and `5000` at
the live-fire start) just inside the struts, `ARM 1 AIM-9B 89%` and the lock frame
(3x crop `hud-with-target-lock-aim9b-crop3x`). The HUD reads the F-5E's own state.

**Cockpit views (windowed, every one draws the `~F29*` art).** Front with the HUD
in the glass and the six small-layout windows; look left and right 45 degrees (the
art turns away with the look, the HUD and the struts slide to the frame edge and
the side mirror comes into view); 90 degrees left and right and 60 degrees up (no
cockpit art, open sky and ground, as for the other aircraft); `--flight-view 4` (F3
Up) and `--flight-view 3` (F2 Back, the spine and stabilator plates from the
pilot's eye). `Airframe::load` accepts the family: the cockpit palette prefix has
64 colours, the three overlays `~F29_LH.PIC`, `~F29_CH.PIC` and `~F29_RH.PIC` and
the instrument frame `~F29_P.PIC` (81 x 80) exist and `Hud::parse` of `F29.HUD`
names `~F29_P.PIC`. The family is thirteen pictures in `FA_1.LIB`.

**Mirrors (fitted, agent decision 2026-11-10).** The art has three flat light-blue
mirror fills (top centre and the two lower canopy rails) and the B5-1 interim gave
them no seeds. A `"F29"` row in `mirrors::seeds` now fills them with the shared
rear feed: `--cockpit-sheet` goes from `NO MIRRORS FITTED` to `FITTED CLR` with the
centre at x 477 y 17 w 323 h 84, the left at x 62 y 291 w 116 h 199 and the right
at x 1097 y 287 w 116 h 203. The unit test lists the F-5E among the fitted
families, and the X-31 (`F31`) stays unfitted.
See [mirrors](../baselines/mirrors.md#2026-11-10-f-5e-cockpit-and-mirror-seeds-f5e-7).

**Instruments and two engines.** The Systems window shows `THR 70%`, `TEMP 0%`,
`OIL 100%`, `HYD 100%` and `FUEL 4400 LBS`, with `(+ EXT 990 LBS)` for the default
centreline tank; one engine readout serves every aircraft, so the two-engine F-5E
shows one throttle and one temperature (the panel has no per-engine needle; the
readouts are generated text). Fuel reads the F-5E's own corrected 4,400 lb (B5-2,
John, 2026-11-10), not the record's 6,240. The Weapons window lists `560 M61`, `2
AIM-9B` and `20 CHAFF` / `20 FLARE`. The ten instrument pages (0 to 9) draw with the
`~F29_P.PIC` bezel; the Envelope page draws the F-5E's own envelope.

**Eye point (agent decision, 2026-11-10, left unchanged).** `mirrors::pilot_eye` is
one fixed offset for every aircraft (7 ft above and 10 ft ahead of the model
origin), used only by F2 Back, the rear-view mirrors and the replay views; the
forward cockpit view looks from the origin with 2D art. The record's `viewOffset`
(0, 20, 60) is the F-15C's and the F/A-18's and no host code reads it. Measured on
the neutral `F5E.SH` (the faces within 10 units of the centreline, 1 unit = 1/3 ft;
the fin top at 24 units is excluded, it stands behind the canopy): the canopy top is
10 units, 3.3 ft above the origin, between 37 and 55 units ahead, 12.3 to 18.3 ft
(the windshield base reaches 20 ft). The fixed eye is therefore 3.7 ft above the
canopy top and 2.3 ft behind its aft end; the F-15C's is 1.7 ft above its canopy and
5.0 ft behind its aft end (measured in F15E-7 on the same rule). The eye stands high
over a small canopy, and F2 Back shows the spine, the fin edge-on and the two
stabilator plates. Not changed: a seated eye would be a fleet change (a table keyed
by aircraft through six call sites), a question for John, as in the Mirage 2000
review.

**Test.** `crates/tore-import/tests/f5e_cockpit.rs` (retail-reading, skips without
media): the sensor rows `VIS340.SEE, F104R.SEE, F104.ECM`, `hud()` `F29.HUD`
different from the X-31's, stem `F29` with shape stem `F5E`, `~F29H.PIC`,
`~F29_P.PIC` from `Hud::parse`, the overlays, thirteen `~F29*` pictures, no
other roster aircraft on the family, no `~F5*` picture, and the record's eye offset.
The aperture is pinned in `hud_aperture.rs`.

Provenance: sensor rows, HUD module, panel names and cockpit art spec-derived
(retail); the X-29 cockpit binding opinionated (John, 2026-11-10); the glass polygon,
the mirror seeds and the eye-point reading fitted or agent decisions (2026-11-10).
Evidence and frames: `.local/m3-aircraft/frames/F5E-7/` and
`.local/m3-aircraft/checks/F5E-7/`.

## F-5E flight review (F5E-8, 2026-11-10)

Dated section, additions only. The F-5E flies the hybrid adapter's own `variety`
configuration built from `F5E.PT` (`f5e.SH`), two J85-GE-21 at 7,000 lbf military
and 10,000 lbf afterburner in all, with John's B5-2 corrections (4,400 lb of fuel,
a 51,800 ft ceiling, top row +7, no +8 row). No envelope, G, weight, fuel, flow,
ceiling or thrust value was changed and no flight constant was touched.

- Top speeds: the 1 G row is 1,090 kt at 36,000 ft (Mach 1.90), 17 percent over the
  published Mach 1.63, and 731 kt at 1,000 ft (no published sea-level figure was
  found). Full internal fuel settles at 0.937 of the edge: 1,021 kt (Mach 1.78) at
  36,000 ft, 685 kt at 1,000 ft. Level flight is held to the corrected 51,800 ft
  ceiling and the zoom peaks at 52,214 ft. The fast edge falls steeply above 39,000
  ft (586 kt at 50,000 ft).
- Climb and acceleration: peak specific excess power 332 ft/s at sea level (19,900
  ft/min), best sustained climb 314 to 316 ft/s, 36,000 ft in 138 to 159 s: 55 to 58
  percent of the published 34,500 ft/min, from the shared drag law and the missing
  ram thrust (the thrust and weights are the real ones). Not changed; a fleet-level
  drag or ram fit remains John's call.
- G: the +7 top row gives 5.98 G at 461 kt and 1,000 ft with full fuel and 7.00 G
  nearly dry (never above it; no +8 row), -2.55 to -2.99 G on the push. The sustained
  turn at 461 kt and 1,000 ft is 4.13 G, 9.6 deg/s. The record's high-speed G rows
  are narrow (2.6 G at 593 kt and 1,000 ft), a retail shape, not changed.
- Fuel: record flows 1 and 8 lb/s kept; 73.3 minutes dry and 9.2 at afterburner
  from 4,400 lb, against about 30 and 12.4 minutes at the J85-GE-21's published
  specific fuel consumption.
- Takeoff and landing: liftoff 153.4 kt after 1,763 ft (537 m), approach 166.2 kt,
  touchdown 159.8 kt, rollout 703 m; the record has no hook.
- The scripted `climb` maneuver ends in an overspeed loss after the zoom (the fast
  edge falls to 832 kt at 43,000 ft); the Mirage and the Gripen do not. Reported.

The liftoff (135 to 165 kt) and approach (155 to 175 kt) ranges in the flight
battery are fitted windows, agent decisions of 2026-11-10, from the published 124 kt
landing-configuration stall speed. Full tables, sources and commands are in the
[variety flight baseline](../baselines/variety-flight.md#northrop-f-5e-validation-2026-11-10).

## F-5E in missions, multiplayer and replay (F5E-9, 2026-11-10)

Implementation and verification, slice F5E-9 of the F-5E port. No game code
changed. `AircraftId::F5E` was appended to every identity list in B5-1
(checkpoint code 43, `SELECTABLE` index 43, carried in the 9-bit wire field
since protocol 83), and `strike_class.rs` lists it in the fighter family
(strike class Fighter, B5-1), so the AI wing build needed no change: it reads
the record's two J85 engines (the roster probe's `models=2`) and the default
load of F5E-6 (the M61 with 560 rounds, the `F150.GAS` tank, two wingtip
AIM-9B and the empty wing pair, which the probe prints as
`[M61 x560, AIM-9B x2, AIM-9B x0]`). No AI behaviour work was done (Mirage plan
rule 1).

What changed is coverage: a Quick Mission test
(`the_f5e_is_a_quick_mission_player_and_a_wing_choice`) naming `F5E.PT` as the
player's aircraft and in every AI wing field, and asserting that neither
`F5EE.PT` nor `F5EV.PT` appears in the player or wing lists in its place; a
checkpoint test in `tore-sim` (`the_f5e_round_trips_at_code_43_and_no_identity_shares_it`:
the coded identity is the single byte 43, every identity in `ALL` round-trips,
no two share a code, 46 is unknown); the `ai-f5e-quick-mission-2v2` and
`replay-rec-f5e-2v2` battery scenarios; the F-5E row in the Load Ordnance notes
loop (`menus-snap-quick-lobby-ordnance-laser-notes-f5e`: the lobby page warns
about a laser-guided store and offers the pod helper, measured, the same as the
F-16A, the MiG-31, the Mirage 2000), and `ai-f5e-*` in the `ai-fights` battery
family.

Results (2026-11-10, throttled, profile `.local/m3-aircraft/profile-f5e`, logs
in `.local/m3-aircraft/checks/F5E-9/`):

- `--ai-roster-probe-ticks 3600`: the four `F5E.PT` lines (Novice, Average,
  Experienced, Ace) each `ticks=3600 models=2 radar=true ir=false stores=3
  gun=true projectiles=0 dropped=0 PASS`; the whole roster has no FAIL line.
- `ai-f5e-quick-mission-2v2` passes: every wing the F-5E, nothing dropped, zero
  anomalies. The F-5E's record has no infrared sensor row, so the wing's AIM-9B
  logs `no AIM-9B shot (ACQUIRING)` and the attack changes to the gun: 29 rounds,
  no hit, no kill in this run (the Gripen's run had a missile kill). That is the
  existing weapon-selection behaviour for a record without an infrared sensor;
  it was not tuned (no AI behaviour work). A finding for John, not a defect of
  this slice.
- `replay-rec-f5e-2v2` passes: the recording reads back `player.aircraft =
  F5E.PT`, `probe.enemy = F5E.PT` and `draw.models = F5E.PT`, two runs are
  identical, and `--verify-render` finds `missing=0 differing=0` over 2,401
  ticks.
- `net-content-missing` and `net-window-lobby` pass after rebuilding
  `tore-server` and `tore-bot`. They take no aircraft argument, so they cover
  the identity list and content code, not an F-5E in flight over the wire.

Parked check. `F5EE.PT` (desert) and `F5EV.PT` (snow) are separate records and
parked targets, and are unchanged: `--surface-parked EGY QELAIR` lists two
`F5EE.PT` at `gear 0x639c`, `scale 0.3333`, `origin-up 6.00` (18 units at a
third), `EGY QESAIR` one more, and `VLA QVLAIR` lists two `F5EV.PT` at
`gear 0x614c`, same scale and `origin-up 6.00`; the ignored retail test
`parked_aircraft::import_tests::every_parked_type_reads_and_stands_on_its_gear`
passes. `F5E.PT` is not parked and never replaces either (B6).

## Batch 5 flight maxima and fuel flows (B5-4, 2026-11-11)

Implementation mode, slice B5-4, **opinionated (John, 2026-11-11: "Correct all
flight model maximums and fuel flows per realistic numbers that fit in TORE")**.
It supersedes the earlier "leave as retail" answers for these quantities in the
F-15E, Gripen, F-5E, Tornado and Mirage F1 sections above (each is kept as
written; this section is the current rule). Thrust and weights are untouched. The
rules (G at half fuel, cut-off ceilings, thrust times cited consumption) are in
[aircraft records](../spec/aircraft-records.md#batch-5-flight-maxima-and-fuel-flows-b5-4-2026-11-11);
the before and after measurements are in
[the variety flight baseline](../baselines/variety-flight.md#batch-5-flight-maxima-and-fuel-flows-b5-4-2026-11-11).
Provenance: **cited** is a public page read 2026-11-11 unless dated otherwise;
**measured** is a `performance_probe` or `envelope_probe` run on the local record;
**judgement** is an agent decision.

Audit, per aircraft (record before, game after, real; the provenance of each real
figure in brackets).

| Quantity | F-15E | Gripen | F-5E | Tornado IDS | Mirage F1 |
| --- | --- | --- | --- | --- | --- |
| Top G row | +9, now +10 (half fuel 8.28 to 9.19 G) | +9 kept (half fuel 8.52 G) | +7, now +8 (6.49 to 7.41 G) | +7, now +8 (6.59 to 7.52 G) | +7, now +8 (6.59 to 7.53 G) |
| Real positive G | +9 [cited, Wikipedia F-15E, `F15E.INF`] | +9, +12 if needed [cited, Wikipedia JAS 39] | +7.33 [cited secondhand, T.O. 1F-5E-1 via a forum thread, B5-2] | +7.5 [cited, `TOR.INF`] | +7.5 [cited, MIGFlug, "as commonly published"] |
| Lowest G row | -4 sliver, now -3 (push 2.74 G, unchanged) | -4 (+8 copy), now -3 (3.78 to 2.83 G) | -4 sliver, now -3 (2.77 G, unchanged) | -3 kept (2.81 G) | -3, now -4 (2.80 to 3.74 G) |
| Real negative G | -3 [cited, `F15.INF`] | -3 [cited, Wikipedia JAS 39] | -3.0 [cited secondhand] | -3 [cited, TOR-8] | -3.6 [cited, MIGFlug] |
| Ceiling, `maxAlt` and 1 G polygon top | 100,000, now 60,000 | 60,000, now 50,000 | 51,800 (B5-2) | 60,000, now 50,000 | 66,125 kept |
| Real ceiling | 60,000 [cited, Wikipedia F-15E] | 50,000 [cited, Wikipedia JAS 39] | 51,800 [cited] | 50,000 [cited, TOR-8] | 65,600 to 66,000 [cited, Wikipedia, MIGFlug] |
| Top speed at 36,000 ft, measured (record) | 1,313 kt, Mach 2.29 (1,434) | 1,090 kt, Mach 1.90 (1,145) | 1,021 kt, Mach 1.78 (1,090) | 1,321 kt, Mach 2.30 (1,369) | 1,271 kt, Mach 2.22 (1,345) |
| Real top speed at altitude | Mach 2.5 [cited] | Mach 2 [cited] | Mach 1.63 [cited, B5-2 read] | Mach 2.2 [cited, TOR-8] | Mach 2.2, 1,262 kt [cited] |
| Top speed at 1,000 ft, measured (record) | 750 kt, Mach 1.14 (819) | 704 kt, Mach 1.07 (739) | 685 kt, Mach 1.04 (731) | 793 kt, Mach 1.20 (821) | 762 kt, Mach 1.16 (806) |
| Real top speed at sea level | Mach 1.2, 800 kt [cited] | Mach 1.2, 794 kt [cited] | none found | Mach 1.2 [cited, TOR-8] | 800 kt, Mach 1.2 [cited] |
| Military flow, lb/s (implied lb/(lbf h)) | 4 (0.442), now 7 (0.774) | 2 (0.593), now 3 (0.890) | 1 (0.514), now 2 (1.029) | 3 (0.624), kept | 3 (0.980), kept |
| Afterburner flow, lb/s (implied) | 24 (1.805), now 26 (1.956) | 14 (2.785), now 9 (1.790) | 8 (2.880), now 6 (2.160) | 22 (2.463), kept | 23 (5.216), now 9 (2.041) |
| Cited consumption, lb/(lbf h) | 0.73 / 1.94 [cited, F100] | 0.844 / 1.79 [cited, RM12] | 1.24 / 2.13 [cited, J85-GE-21] | 0.598 / 2.5 [cited, RB.199] | 0.981 / 1.991 [cited, Atar 09K-50] |

**Corrected.** F-15E: `maxAlt` 60,000 ft (polygons cut off there), +10 row,
the -4 sliver retired, flows 7 and 26. Gripen: `maxAlt` 50,000 ft, the -4 row
(a copy of the +8 row) retired, flows 3 and 9. F-5E: +8 row (fitted from +7), the
-4 sliver retired, flows 2 and 6. Tornado: `maxAlt` 50,000 ft, rows +7 and +8,
flows confirmed (B5-3). Mirage F1: rows +8 and -4, afterburner flow 9.

**Declined, with reasons.**

- Top speeds, all five: the record's envelope is the aircraft's own; the gap to
  the real figure comes from the shared drag law (the level speed is 92 to 97
  percent of the record's top edge), which is a fleet question for John; a per-
  record polygon fit was not made. The largest gaps: Gripen at sea level 11 percent
  under, F-15E at 36,000 ft 8 percent under, F-5E at 36,000 ft 9 percent over
  (record 16 percent over).
- Gripen +9: half fuel pulls 8.52 G (0.48 G under); +10 would pull 9.47 G (0.47 over
  the 0.3 G allowance).
- Mirage F1 ceiling, Tornado -3 and top speeds, F-5E ceiling: inside 5 percent or
  half a G.
- Mirage F1 -3.6 is not a whole G; the -4 row is the nearest at half fuel (3.74 G, 0.14
  over).

**Findings.** (1) The model pulls the limit only near empty, so at full fuel the
corrected aircraft are still under the real limit (F-15E 8.41 G, Gripen 8.00 G,
F-5E 6.83 G, Tornado 7.05 G, Mirage F1 7.01 G) and near empty they pass it by up
to 1.1 G (F-15E 10.10 G); the half-fuel reading is John's Mirage 2000 choice. (2)
The F-15E and Gripen `fuelConsumption` values are rounded to whole pounds a
second, a 5 to 6 percent error on the dry flow; the F-5E's is 17 percent. (3) The
F-5E speed polygon is 16 percent over the real top speed; the sea-level figure was
not found in any source read.

## F-5E top-speed trim (B5-5, 2026-11-11)

Implementation mode, slice B5-5, **opinionated (John, 2026-11-11, after B5-4:
"Trim F-5E only")**, factor **fitted** (agent decision). It adds to the F-5E
reviews above (the "F-5E flight review" and B5-4 sections are kept as written;
their "top speed 16 percent over" finding is what this slice answers). The rule,
the measured numbers and the cost are in
[aircraft records](../spec/aircraft-records.md#f-5e-top-speed-trim-b5-5-2026-11-11)
and [the variety flight baseline](../baselines/variety-flight.md#f-5e-top-speed-trim-b5-5-2026-11-11).

**How top speed arises (measured, read from the code).** `F5E.PT` has no
maximum-speed field; the 1 G polygon's fast edge is the record's top speed. The
flight model sets airframe drag equal to full thrust times lapse at that edge
(`Configuration` drag law, `airframe_drag`) and adds the loaded-drag percent for
the fuel carried: at full fuel (4,400 lb over 9,558 lb empty, loaded drag 30
percent) the level top speed is the edge divided by sqrt(1.138) = 0.937 at every
altitude (`envelope_probe`: ratio 0.937 at 0 to 51,800 ft). The overspeed loss
and the instruments' envelope window read the same 1 G edge, so one polygon
change moves the level speed, the overspeed limit and the window together. The
decoded record's 1 G edge: 1,220 ft/s (723 kt) at sea level, 1,475 at 18,000 ft,
1,840 at 36,000 ft (Mach 1.90).

**Change.** `fit_envelopes` (F-5E arm of `aircraft_corrections.rs`), after B5-4's
rows: `trim_top_speed` multiplies the fast sides of the 1 G and 0 G polygons by
`F5E_TOP_SPEED_SCALE` 0.92 (whole ft/s), holds every other row's fast vertices to
the new 1 G edge at their altitudes (only the +2, +3 and -1 rows' high-altitude
vertices move), and runs only while the 1 G polygon still tops out at 1,840 ft/s
and the 0 G polygon at 1,940 ft/s, on top of B5-4's guard (retail range, sliver
and repeat row). Three new unit tests and two changed asserts
(`aircraft_corrections.rs`, `aircraft.rs`). `models/variety.rs` already has a
`SpeedFit::Scale` mechanism (AC-130, V-22, AH-64) that scales fast sides at the
flight-model level; it was not used because it has no guard on the record's
values, which this slice was asked to have.

**Factor.** 0.92 was measured, not assumed: the model flew 1,021.5 kt (Mach 1.78)
at 36,000 ft; the real figure is Mach 1.63 (Wikipedia) to 1.64 (MILAVIA), 935 to
941 kt in TORE's atmosphere, so the scale lies in 0.915 to 0.921; 0.92 gives 939.9
kt, Mach 1.64. At sea level the same factor gives 623 kt (Mach 0.94), 685 to 630
kt at 1,000 ft. **Sea-level figure:** none found (Wikipedia and MILAVIA give
altitude only, aerospaceweb.org lists it as unknown, MIGFlug gives Mach 1.63 at
36,000 ft and a 710 kt indicated never-exceed, all read 2026-11-11). The brief's
sea-level figure stays unsupported; 623 kt is the record's shape at the same
factor.

**Findings.** (1) The shared drag law makes a lower top edge a higher drag at
every speed (+18 percent): climb, sustained turns and acceleration fall (the
sustained turn at 461 kt from 4.13 to 3.63 G; the best climb from 314 to 286
ft/s). The climb was already 55 to 58 percent of the published rate and is now
about 50 percent; John may want the thrust or drag side reviewed separately. (2)
The instantaneous pull is unchanged (6.84 / 7.42 / 7.75 / 8.01 G by fuel, B5-4's
numbers to 0.01 G) because the trim leaves every row below the new 1 G edge
alone. (3) The Gripen (11 percent under at sea level) and F-15E (8 percent under
at 36,000 ft) stay slow; the shared drag law is untouched.

## Mirage F1 gear, airbrake and plume (MF1-4, 2026-11-10)

Implementation, slice MF1-4, Claude Sonnet (sonnet-high). The Mirage F1 rig
(`mf1_animation.rs`) now owns the three device words. Results, with the geometry
and the rule 11 fold table in [Mirage F1 gear, airbrake and
plume](aircraft-control-surfaces.md#mirage-f1-gear-airbrake-and-plume-mf1-4-2026-11-10)
and the player-visible laws in [Mirage F1 gear, airbrake and plume
fit](../spec/variety-animation.md#mirage-f1-gear-airbrake-and-plume-fit):

- **Gear.** The three plates turn as whole rigid pieces: the mains forward and
  inward (95 degrees about an axis on their own top edge, 15 degrees off lateral),
  the nose rearward (110 degrees about a lateral axis through its forward top
  corner) with steering. The F1's mains retract forward and its nose gear
  rearward ([Air Vectors, "The Dassault Mirage F1"](https://airvectors.net/avmirf1.html),
  a general history). Gear 1 is the source pose, grounded at the MF1-2 scaled low
  point (5.98 ft); gear 0 draws nothing.
- **Parked path.** `("MF1.PT", Some(0x5adc))` is unchanged: `--surface-parked FRA
  QFLAIR --seconds 0` lists both parked F1s exactly as before (class `0x8000`,
  gear `0x5adc`, `origin-up` 5.98), and the windowed scene shows them gear down
  on the apron.
- **Airbrake.** The one dorsal plate turns whole about its forward edge
  (`0.707 rad * (1 - brake)` onto the ridge, hidden at 0).
- **Burner and engine count.** One plume from the nozzle exit plane, one nozzle
  glow on the two half discs (`2e92`, `2eb3`) and one contrail from the engine
  count of 1 (`outlet_count(Mf1) = 1`); the shape-end fallback MF1-2 noted is gone.
- **Remaining words.** `0x5ae8`, `0x5aee` and `0x5af4` are pinned and never set
  (the ailerons and the rudder are turned instead); no word is left unaccounted for.
- **Open for John.** (1) The F1's real brakes are paired panels under the
  intakes; only the source's dorsal plate is drawn. (2) The hull model used to
  fit the stow is read off the skin faces; a stowed main is inside it to within
  0.1 unit, but the shape has no gear bay, so nothing is cut open. (3) The main
  foot dips 0.08 ft in the first degrees of travel (inclined axis).

## Mirage F1 stations measured (MF1-5, 2026-11-10)

Implementation and review, slice MF1-5, Claude Sonnet (sonnet-high). Measurement
only: no code, station row or pin changed, and the slice is `[~]` pending John.
The full tables, the per-record alternatives and the options are in [stores on
pylons](../spec/stores-on-pylons.md#the-mirage-f1-mf1-5-2026-11-10). Summary:

| Row | Location | Default | Result on the scaled shape |
| ---: | --- | --- | --- |
| 4 | Centerline | empty (B5-2) | clean when empty; a `F250.GAS` is 0.14 ft off the belly, 0.69 ft off the gear; takes one store |
| 5 | Wing, four stores | `F250.GAS` x4 | **inner tanks cross each other and the main gear** (outer tanks clean) |
| 6 | Wing, paired | `R550.JT` x2 | clean (left 0.05 ft) |
| 7 | Wingtip | `R550.JT` x2 | **1.48 ft entry into the tip** |

Real Mirage F1: five pylons and two wingtip rails (the record's seven positions
agree); the real carries one tank a side where the record's default carries two.
No pod or designator was added. Counts and audit pins unchanged (45 aircraft, 18
recovered stations, 4,652 weapon and 233 tank placements, 80 pods, 220
Cheat-only). Frames: `.local/m3-aircraft/frames/MF1-5/` (local only).

## Mirage F1 sensors, HUD and cockpit (MF1-7, 2026-11-10)

Slice MF1-7 confirmed the Mirage F1 on the sensor records and the Rafale HUD and
cockpit family its record names. John's decision round of 2026-11-10 (decision 25,
"follow the record", opinionated) settled what the batch identity slice had wired
as an interim agent decision: the F1 flies `rafale.HUD`, the same binding as the
Mirage 2000. Only two comments changed (`aircraft.rs`, `hud_aperture.rs`). Added:
a retail-reading test (`crates/tore-import/tests/mf1_cockpit.rs`), an aperture
unit test and documentation. No behaviour changed.

**Sensors (spec-derived, retail records, already reviewed).** `--sensor-summary`
reads: `Mf1: radar F18R.SEE search 90 nmi 60x60 deg track 50 nmi 45x45 deg
look-down 0 ADVANCED; infrared none; visual VIS340.SEE search 10 nmi 170x140 deg
track 5 nmi 170x140 deg; jammer AV8.ECM EARLY strength 0.00 (no RF mode); signature
radar 100 infrared 100`. The radar is the Mirage 2000's and the F/A-18's
(`F18R.SEE`, `Preset::Advanced`); no `sensors/profile.rs` arm was needed. The
visual record is `VIS340.SEE`. The infrared channel is absent because the record
has no `.SEE` infrared row. Nothing under `crates/tore-sim/src/sensors` branches
on the aircraft identity, so radar modes, the scope and the RWR behave as for the
other `F18R.SEE` aircraft. A live-fire run (`--live-fire --aircraft mf1
--weapon-slot 2`, the wing Magic pair; slot 3, the wingtip pair, behaves alike)
shows the Radar window `TWS 10NM 1 CONTACTS TRACK`, the weapon window `R-550
Magic ... LOCK`, the HUD `ARM 1 R-550 ... IN RNG`, and the Magic locks, flies,
hits and kills (`shots=1 hits=1 kills=1` at 700 and 1,500 ticks; at 150 to 450
ticks it is still flying, `hits=0 active=1`). The signature numbers (radar 100,
infrared 100) are the record's own and differ from the Rafale's 50 and 70.

**Countermeasures and the jammer.** `AV8.ECM` is the AV-8B's reviewed internal
suite (`Generation::Early`, decoy chance and counts from the record): it has **no
radio-frequency jammer mode** (`strength 0.00`), only chaff and flares. The
Weapons window shows `30 CHAFF` and `30 FLARE`; `--combat-command chaff` and
`flare` with `--countermeasure-preview 240` read `carried_chaff=29
carried_flares=29 capacity_chaff=30 capacity_flares=30` after one of each, and the
devices draw behind the aircraft. The jammer key (`--jammer-on`) lights `ECM ON`
and `JAM` in the RWR window as for any aircraft, but with strength 0.00 it adds
no radar jamming: this is the one sensor difference from the Mirage 2000, whose
`F18.ECM` is `LATE` at 0.30. The B5-2 default pod `ALQ167.ECM` on the centreline
is empty for play (John, decision 27); the retail record still lists it, and the
test accepts both. The RWR window draws as for every aircraft; no scripted path
plots an emitter or a missile on the player's RWR (the live-fire fixture's target
emits no radar and the incoming-round command plots at the centre), the same
limit as the Mirage 2000 review ([lane-flight](../testing/lane-flight.md)). The
frame shows the window with `JAM` lit and an incoming round.

**HUD and cockpit (opinionated, John, 2026-11-10, decision 25).**
`AircraftId::Mf1.hud()` is `RAFALE.HUD` (the record's `rafale.HUD` resolves
case-insensitively; same file and hash as the Rafale's and the Mirage 2000's), the
cockpit stem is `RAF`, the forward art `~RAFH.PIC` and the instrument window frame
`~RAF_P.PIC` (`Hud::parse` of `RAFALE.HUD` names it). Neither the Rafale's nor
the Mirage 2000's record is aliased: `MF1.PT` stays the identity (pinned in
`mf1_cockpit.rs`, retail-reading, skips without media: the sensor rows
`VIS340.SEE, F18R.SEE, AV8.ECM` and the centreline `ALQ167.ECM`, the first two
equal to the Mirage 2000's and the jammer row present in the AV-8B's, the binding,
the HUD's panel reference, the overlays `~RAF_LH`, `~RAF_CH`, `~RAF_RH`, the record
naming `rafale.HUD`, the record's own eye offset, and that no `~MF*` cockpit
resource exists). `hud_aperture.rs` resolves the stem `RAF` to the Rafale's
measured glass; a new unit test pins that the F1's mask equals the Rafale's and
the Mirage 2000's and is not empty. `--cockpit-sheet OUT_DIR MF1,M2000,RAFALE`
(through `tools/agent-run.sh`; the flag takes no further options) printed `mf1:
stem MF1 cockpit ~RAFH.PIC art 1280x490 FITTED CLR`, centre mirror x 461 y 24 w 360
h 79, left x 140 y 242 w 118 h 248, right x 1024 y 242 w 118 h 248, identical to
the Rafale and the Mirage 2000. No Mirage F1 cockpit art exists, so the F1 flies a
Rafale cockpit, as the Mirage 2000 does. The front frame shows the HUD inside the
Rafale combiner glass with no clipping.

**Views (windowed, `--flight-view`, `--flight-look`).** Forward cockpit with the six
small-layout windows; look left and right 45 degrees (the cockpit art turns away
with the look, the HUD glass and canopy bow at the frame edge); 90 degrees left
and right and 60 degrees up (no cockpit art, open sky, as for the Rafale and the
Mirage 2000); `--flight-view 4` (F3 Up) and `--flight-view 3` (F2 Back). F2 Back
shows the F1's spine, fin and tailplane roots from the pilot's eye. The standalone
instrument pages 0 to 9 (RCS, Envelope, Front View, Other View, Target Cam, RWR,
Nav Info, Systems, Weapons, Radar) draw with the Rafale bezel; the F1's own
envelope polygon is drawn on page 1.

**Instruments and the single engine.** The Systems window shows `THR`, `TEMP`,
`OIL`, `HYD` and `FUEL` / `(+ EXT)`: one engine readout serves every aircraft, so
the one-engine F1 shows one throttle and one temperature and no second engine
needle (the panel art has none; the readouts are generated text). Fuel reads the F1
record's own number: `FUEL 6120 LBS` (the Rafale reads 9,900, the Mirage 2000
6,346), and `(+ EXT 6600 LBS)` with the default four `F250.GAS` of row 5 (1,650 lb
each; the clipping on that row is MF1-5's open question), 6,595 to 6,592 lb after
the first seconds of a run (Update, MF1-5b, 2026-11-11: two tanks now, `(+ EXT 3300
LBS)`). The Weapons window lists `250 DEFA`, `4 R-550` and `30
CHAFF` / `30 FLARE`. Throttle shows 70 percent at the free-flight start, as for
the other aircraft. The HUD airspeed and altitude boxes read the F1's own state
(540 kt and 5,000 ft at the live-fire start).

**Eye point (agent decision, 2026-11-10, left unchanged).** `mirrors::pilot_eye` is
one fixed offset for every aircraft, 7 ft above and 10 ft ahead of the model
origin, used only by F2 Back, the rear-view mirrors and the replay views; the
forward cockpit looks from the origin with 2D art. The record's own `viewOffset`
(x 0, y 20, z 60) equals the Mirage 2000's (the Rafale's z is 80) and no host
code reads it, so there is no per-aircraft eye. Measured on the scaled neutral
shape (the faces within 4 units of the centreline and ahead of the fin): the F1's
roof line is 6 units, 2.1 ft above the origin after the height factor, and runs from
unit 7 to unit 58 forward, 2.8 to 22.8 ft ahead; the shape draws no separate
canopy bubble, so these are the spine and canopy faces together. The fixed eye
therefore stands 4.9 ft above the roof and over its span, against 3.5 ft above
the Mirage 2000's canopy top and 4.0 ft above the Rafale's. The F2 Back frames of
the three show the same kind of picture (spine and fin from above and behind).
Not changed; a seated eye would be a fleet change (a table keyed by aircraft
through six call sites), a question for John as in the Mirage 2000 review.

Evidence and frames: `.local/m3-aircraft/frames/MF1-7/` and
`.local/m3-aircraft/checks/MF1-7/` (local only, no retail bytes committed).

## Mirage F1 flight review (MF1-8, 2026-11-11)

Dated section, additions only. The Mirage F1 flies the hybrid adapter's own
`variety` configuration built from `MF1.PT` (`mf1.SH`): one Atar 9K-50 at 11,020 lbf
military and 15,873 lbf afterburner (B5-2), retail weights and fuel (decision 24),
its own G rows -3 to +7, a 66,125 ft ceiling and a 1,345 kt top row. No envelope, G,
weight, fuel, flow, ceiling or thrust value was changed and no flight constant was
touched.

- Top speeds are the real ones: full internal fuel settles at 0.945 of the edge, 1,271
  kt (Mach 2.22) at 36,000 ft against the published Mach 2.2 and 762 kt at 1,000 ft
  against about 800 kt. Level flight is held to the 66,125 ft ceiling (published 65,600
  ft).
- Climb and acceleration: peak specific excess power 376 ft/s at sea level (22,600
  ft/min), best sustained climb 360 to 375 ft/s: 54 percent of the encyclopedia's
  41,930 ft/min and 47 percent of the 47,800 ft/min of MIGFlug and Wikipedia, from
  the shared drag law and the missing ram thrust (the thrust and weights are the
  real ones). Not changed; a fleet-level drag or ram fit remains John's call.
- G: the +7 top row gives 6.14 G at 461 kt and 1,000 ft with full fuel, 6.59 and 6.84
  G at half and a quarter, and 7.06 G nearly dry (0.06 G past the row), -2.61 to
  -3.00 G on the push; the real limits are +7.5 and -3.6. The sustained turn at 461
  kt and 1,000 ft is 3.63 G, 8.1 deg/s (the B5-2 figure). The rows hold their
  strength at speed (6.1 G at Mach 1.1).
- Fuel: record flows 3 and 23 lb/s kept; the dry flow equals the real Atar's
  specific fuel consumption (0.98 lb/lbf/h) but the afterburner flow is 2.6 times it
  (5.22 against 1.96 to 1.99), so the 6,120 lb tank gives 4.4 minutes at afterburner
  (real consumption: about 11.8). Finding for John: a guarded override of
  `aftFuelConsumption` 23 to about 9 lb/s would be the Tornado's B5-3 correction
  again; not made.
- Default load (29,839 lb, four 250 gallon belly tanks, 99 percent of the maximum):
  5.2 G at 461 kt, a 2.93 G sustained turn, Mach 2.07 at 36,000 ft, liftoff 135.4 kt
  after 504 m and approach 145.6 kt, against 6.1 G, 3.63 G, Mach 2.22, 103.9 kt after
  211 m and 115.9 kt clean. These depend on John's open stations decision (MF1-5).
  Update (MF1-5b, 2026-11-11): the stations decision is made (two belly tanks): the
  default load is 26,143 lb (87 percent of the maximum), liftoff 121.0 kt after
  349 m and approach 127.6 kt; the other default-load figures above were not re-run.
- Takeoff and landing run: published 600 m and 670 m (the game's Jane's text, weights
  unstated); the model gives 211 m and 376 to 384 m clean, 504 m and 584 to 598 m with
  the default load.

The liftoff and approach ranges (125 to 150 kt each) in the flight battery are fitted
windows, agent decisions of 2026-11-11, from the published approach speed of about
125 kt. Full tables, sources and commands are in the
[variety flight baseline](../baselines/variety-flight.md#mirage-f1-validation-2026-11-11).

## Mirage F1 in missions, multiplayer and replay (MF1-9, 2026-11-11)

Implementation and verification, slice MF1-9 of the Mirage F1 port. No game
code changed: `AircraftId::Mf1` was appended to every identity list in B5-1
(checkpoint code 45, `SELECTABLE` index 45, `ALL` 45 entries), and the wire
codes an identity by its 9-bit `SELECTABLE` position since protocol 83, so no
golden, replay or network fixture moved. The AI builds a wing member from the
record like any aircraft: it sees the one Atar 9K-50 of decision 23, and the
default AI load check passes for it. `strike_class.rs` already lists it in the
fighter family, so no AI behaviour work was done (rule 1). The AI default load
is the record's current default load (DEFA gun 250 rounds, four R550 Magic, four
`F250.GAS` belly tanks); the four tanks are MF1-5's open stations question with
John, so anything below that mentions the tanks (the 8,050 lb payload and 6,474
lb external fuel of the unengaged wingman) changes with his answer.

Update (MF1-5b, 2026-11-11): the answer is two belly tanks, so the AI's default load
is the gun, two `F250.GAS` (3,300 lb of external fuel) and four R550 Magic; the
8,050 lb payload and 6,474 lb external fuel above were measured with four tanks and
were not re-measured. No AI code changed.

What changed is coverage: a Quick Mission test
(`the_mf1_is_a_quick_mission_player_and_a_wing_choice`) naming `MF1.PT` as the
player's aircraft and in every AI wing field and asserting it is neither the
Mirage 2000 nor the Rafale, a checkpoint test
(`the_mf1_round_trips_at_code_45_and_no_identity_shares_it`), the
`ai-mf1-quick-mission-2v2` and `replay-rec-mf1-2v2` battery scenarios, the
Mirage F1 row in the Load Ordnance notes loop
(`menus-snap-quick-lobby-ordnance-laser-notes-mf1`: the lobby page warns about a
laser-guided store and offers the pod helper, the same as the Mirage 2000,
because its record has no Pave Spike row), and `ai-mf1-*` in the `ai-fights`
battery family.

Results (2026-11-11, throttled, profile `.local/m3-aircraft/profile-mf1`):

- `--ai-roster-probe-ticks 3600`: the four `MF1.PT` lines (Novice, Average,
  Experienced, Ace) each `ticks=3600 models=2 radar=true ir=false stores=5
  gun=true projectiles=0 dropped=0 PASS` (the DEFA row and four R550 rows), so
  the AI flies it finite with an exact input replay at every skill; the whole
  roster has no FAIL.
- `ai-mf1-quick-mission-2v2` passes: every wing the Mirage F1, the AI fires its
  R550 twice (at 20,744 and 7,810 ft), one missile kill by the player, nothing
  dropped, zero anomalies. Unlike the F-5E's AIM-9B, the F1's Magic is fired
  (the F1 has a radar row, `F18R.SEE`, though no infrared sensor row).
- `replay-rec-mf1-2v2` passes: the recording reads back `player.aircraft =
  MF1.PT`, `probe.enemy = MF1.PT`, `draw.models = MF1.PT`, two runs are
  identical and `--verify-render` finds nothing missing or differing.
- `net-content-missing` and `net-window-lobby` pass with `tore-server` and
  `tore-bot` rebuilt on this branch first.

The parked Mirage F1 path is unchanged and checked: `--surface-parked FRA
QFLAIR` lists two `MF1.PT` (`gear 0x5adc`, `scale 0.3333`, `origin-up 5.98`) and
`QFSAIR` one, and `--surface-scene` draws them gear down on the apron at the
same size as the flown aircraft (see
[parked aircraft](../spec/surface-defenses.md#parked-aircraft)). Frames:
`.local/m3-aircraft/frames/MF1-9/`.

Provenance: identity wiring spec-derived (record) and B5-1 (agent decision,
2026-11-10); the AI behaviour is the existing fighter family, unchanged; test
and scenario coverage agent work, 2026-11-11. Living roster counts are left to
X-10.

## Mirage F1 stations finished (MF1-5b, 2026-11-11)

Implementation, slice MF1-5 finish (round 3), Claude Sonnet (sonnet-high). John's
round-3 answers of 2026-11-11 (M1 and the coordinator's reading M2, opinionated)
applied to the stations the MF1-5 section above measured: the belly row (row 5,
`F250.GAS` x4) holds two stores, one a side, at 3.24 ft out, and the wingtip Magic
(row 7) stands 1.1 ft outboard of the tip. The jammer, the empty centreline and the
per-axis scale are unchanged. Details, tables and the ten-record tip check are in
[stores on pylons](../spec/stores-on-pylons.md#the-mirage-f1-stations-finished-mf1-5b-2026-11-11).

| Row | Location | Default | Result on the scaled shape |
| ---: | --- | --- | --- |
| 4 | Centerline | empty (B5-2) | unchanged; a centreline `F250.GAS` is 1.36 ft clear of the new pair |
| 5 | Wing, capped at two | `F250.GAS` x2 | clean: 1.25 ft off the main gear, 3.10 ft off the ground, no entry, 0.06 ft off the flank |
| 6 | Wing, paired | `R550.JT` x2 | unchanged (left 0.05 ft) |
| 7 | Wingtip | `R550.JT` x2 | clean (0.48 and 0.24 ft off the tip; was 1.48 ft entry); 9 of 10 tip records clean |

Code: `aircraft_stations::adjust_defaults` (capacity four to two, under the same
whole-retail-table guard as the empty centreline), `hardpoints::station_move`
(`Mf1`, row 5, Wing, 3.24 ft) and `hardpoints::tip_offset` (1.1 ft outboard, the
Mirage F1's retail wingtip row only). Audit pins unchanged (45, 18, 4,652, 233, 80,
220). Default load 26,143 lb (was 29,839); battery windows refitted (liftoff 121.0
kt, approach 127.6 kt). Frames: `.local/m3-aircraft/frames/MF1-5b/` (local only).

Update (MF1-6, 2026-11-11): the audit pins of the paragraph above moved to 4,618
weapon placements when MF1-6 refused 34 clipping stores; see the next section.

## Mirage F1 ordnance, default load and countermeasures (MF1-6, 2026-11-11)

Implementation, slice MF1-6, Claude Sonnet (sonnet-high). Details, lists and
numbers are in [aircraft ordnance](aircraft-ordnance.md#the-mirage-f1-mf1-6-2026-11-11),
[stores on pylons](../spec/stores-on-pylons.md#the-mirage-f1-refusals-mf1-6-2026-11-11)
and [the baseline](../baselines/ordnance-availability.md#mirage-f1-ordnance-masks-and-default-load-mf1-6-2026-11-11).

- **Masks (agent decision following John's F-15E A2 0.3 ft line; the F-5E kept its wing entries, C2).**
  Normal loading refuses 33 stores on the belly pair (row 5, entries 0.37 to 3.39 ft)
  and the `AS30.JT` on the paired wing row (row 6, 0.33 ft); the default
  `R550.JT` is kept on row 6 (left 0.05 ft). Weapon placements 4,652 to 4,618.
- **Default load (unchanged by this slice, proved).** Gun 250 `DEFA.JT`, empty
  centreline, two `F250.GAS`, two wing and two wingtip `R550.JT`; 26,143 lb.
  Same for the human, the AI, saved loads, the lobby and restart.
- **Countermeasures.** `AV8.ECM` 30 chaff and 30 flares (O4: keep, John 2026-11-11).
- **Live fire and jettison.** Gun 250 shots (54 hits, one kill), wing and wingtip
  Magic one shot each (hit and kill), the two wing and wingtip groups jettison, two
  tanks then two Mk 82s `PASS released 4`, a belly AGM-65G kills the lead truck.
- **Code.** `aircraft_stations::weapon_allowed` (new), one check in
  `Loadout::capacity_before_store_pairs`, one in the ordnance audit, and in
  `live.rs` the empty-row selection skips a refused store (the belly row falls back
  to `MK82.JT`). New retail-reading test `crates/tore-import/tests/mf1_ordnance.rs`
  and battery scenarios `a2g-release-origin-mf1`, `a2g-jettison-mf1`,
  `a2g-maverick-mf1`, `STATIONS["mf1"] = [250, 2, 2, 0, 0]`.

Open items for John: the centreline (row 4) still lets 36 records enter the belly
and 15 cross the gear (not asked); the gun cadence is the shared one and was not re-measured; the real
chaff and flare counts of the F1 were not sourced.

## Tornado stations measured and the swivelling pylon wiring (TOR-5, 2026-11-10)

Implementation and review mode, slice TOR-5; stopped on clipping (John's standing
rule). The full table, the rule and the options are in
[stores on pylons](../spec/stores-on-pylons.md#the-tornado-ids-swivelling-wing-pylons-and-a-stop-on-clipping-tor-5-2026-11-10).

Update (TOR-5, 2026-11-10) to the TOR-3 pylon note in
[the Tornado control rig](aircraft-control-surfaces.md#tornado-control-rig-tor-3-2026-11-10):
the swivel is now wired. The sweep schedule and pivot moved from the drawing rig to
`tore_sim::combat::swing` (same numbers); inner wing row 7 is the only swivelling
row; the shoulder (row 6) and centreline (row 5) pylons are fixed. The earlier
statement that row 7 sits inboard of the pivot stands: as decoded (3.67 ft out, 3 ft
forward) it hangs from the fuselage side, enters the airframe by 2.61 ft with the
default Mk 82, and with the swivel it stands under no wing at 67 degrees.

Update (TOR-5, 2026-11-10) to the TOR-0 hardpoints table: nothing changed in the
rows; the row 5 tank pair, row 6 Paveways and row 7 Mk 82s overlap each other and the
fuselage under the belly (numbers in the stores specification). Audit pins are
unchanged (45 aircraft, 18 stations, 4,652 weapon placements, 233 tank placements,
220 Cheat-only placements, 80 pod placements).

Update (TOR-5b, 2026-11-11) to the TOR-5 stop on clipping: John's round-3 answers moved
row 7 onto the swinging panel (9.5 ft out, 1.5 ft forward), put one centreline tank in
the default load and added a glove plate; see
[Tornado stations moved, one tank and a glove plate (TOR-5b, 2026-11-11)](#tornado-stations-moved-one-tank-and-a-glove-plate-tor-5b-2026-11-11)
at the end of this file. The measurement above stands as the TOR-5 record.

## Tornado sensors, HUD and cockpit (TOR-7, 2026-11-11)

Slice TOR-7 (implementation mode) made the Eurofighter 2000's cockpit family work
for the Tornado IDS and confirmed its sensors. John's decision round of 2026-11-10
(decision 21) decided the review's [HUD and cockpit
question](#tornado-hud-and-cockpit-for-john): **follow the record**, `e2000.HUD`,
cockpit stem `EF2`, forward art `~EF2H.PIC`. That settles what the batch identity
slice wired as an interim agent decision, so the binding is now **opinionated
(John, 2026-11-10)**; the stem arm, picture and overlay names already existed from
B5-1 and did not change. The Tornado is the first aircraft to use the family: TORE
has no Eurofighter (`E2000.PT` is not on the roster), and no other identity names
`E2000.HUD` or stem `EF2` (checked by the new test). Changed: the glass polygon
(`hud_aperture.rs`), an `"EF2"` mirror seed row (`mirrors.rs`), two comments in
`aircraft.rs` and a retail-reading test (`tore-import/tests/tornado_cockpit.rs`).
Before the slice every cockpit view already drew the `~EF2*` art (the stem and picture
were wired); what was missing was the HUD (the glass mask was empty, so no HUD
drew) and the mirrors (three dead blue-grey panels).

**Sensors (spec-derived, retail records; nothing changed).** The record's fixed
rows stay as retail (decision 22): `VIS240.SEE`, `F18R.SEE`, `MIG31.ECM` and
`PAVESPK.SEE`; the remaining rows are the weapons, the 3,300 lb tank pair and the
gun. `--sensor-summary` reads: `Tornado: radar F18R.SEE search 90 nmi 60x60 deg track
50 nmi 45x45 deg look-down 0 ADVANCED; infrared none; visual VIS240.SEE search 10 nmi
120x90 deg track 5 nmi 120x90 deg; jammer MIG31.ECM LATE strength 0.30; signature
radar 100 infrared 100`. The radar is the F/A-18's `Preset::Advanced` (not the
Tornado's ground-mapping radar, left as retail), the visual sensor is the
Tornado's own narrower `VIS240.SEE` (120 x 90 degrees against the fleet's 170 x
140), and the jammer is `MIG31.ECM` read as the `LateColdWar` generation at
strength 0.30. No match arm branches on the aircraft, so the radar modes, the lock
and the RWR behave as for the other aircraft.

A live-fire run (`--live-fire --aircraft tornado --weapon-slot 1`, the BK27 gun)
shows the Radar window `TWS 10 ... TRACK`, the HUD `LCOS 354 BK27 RADAR` with the lock
circle on the target and the target camera `TORNADO IDS`; `--scope-range 50` redraws
the scope at 50. **There is no AIM-9 on the Tornado:** the record's `AIM9M.JT` row is
the outer wing station that B5-2 retired on John's decision (23b), so the default
store for a lock is the gun, and slots 2 and 3 are `4 GBU-10` and `2 MK-82P3` (the
Weapons window also reads `60 CHAFF` and `60 FLARE`, the `MIG31.ECM` counts).
`--jammer-on --combat-command incoming` lights `JAM` on the RWR window; the incoming
round plots at the window centre, and no scripted path plots an emitter on the RWR,
as for the other aircraft.

**Pave Spike and the laser bomb run (no pod body).** The `PAVESPK.SEE` row is a fixed
internal row (flag `0x8`, position (0, 0, 0), as on the A-10), so the aircraft has
the AC-130 targeting sight's laser designator with nothing hung under it and no
pod drawn (decision 22). With the GBU-10 selected (slot 2) and a ground point pinned
(`--combat-command sight-pin`) the HUD reads `ARM 1`, `3 GBU-10`, `IN RNG`, `LSR 4.6`
and `TTI 0:32` with the laser box on the point, and the target camera shows the
Paveway seeker. With nothing designated it reads `NO LASER`, as for the F-16C. The
default load therefore allows a laser bomb run without any pod station, which the
Tornado does not have in this record. The sensor summary shows `infrared none`
because the Pave Spike is a laser sight, not a radar-window infrared channel.

**HUD glass (measured, fitted).** `hud_aperture::mask` now has a polygon for the
Tornado, replacing the empty mask the B5-1 interim left (the aperture fails closed
for a picture of any other size). `~EF2H.PIC` is 1280 x 490. Its glass is the one
closed region of transparent pixels inside the combiner frame: a thin arc on top
(green, from y 105 at the centre), two thin side lines (x 475 and 803 at the widest, y 209 to
217), two support cables that pinch it from y 225 down, and a green bar at the bottom
(y 365). A flood from the middle (640, 250) over the art's transparency fills exactly
73,769 pixels, x 475 to 803 and y 105 to 364, and stops at the frame, so the glass is
bounded by the art itself. The fitted polygon is 36 vertices every 16 rows along those
lines, taken on the opaque line pixels (one pixel out from the flood), so it never
reaches the transparent sky outside the frame (0 pixels leak) and loses 852 pixels
(1.2 percent) at the chords. The vertices are in `hud_aperture.rs`. Two tests pin
it: a synthetic one (centre in; above the arc, outside the side lines and below the
bottom bar out; an opaque pixel inside is left out; a picture of the wrong size fails
closed) and a retail-reading one that skips without an install: on the real art the
frame still closes exactly 73,769 pixels, the mask covers exactly 72,917 pixels, never
covers an opaque pixel, lies wholly inside the flood-bounded glass, covers at least 98
percent of it and is one connected region. The HUD draws inside the glass: heading tape
under the arc, the airspeed and altitude boxes (`574` and `5000` at the live-fire
start) just inside the side lines, `LCOS`, the weapon and `RADAR` lines and the lock
circle (frames `hud-glass-whole-3x` and `hud-with-target-lock-bk27-crop3x`).

**Cockpit views (windowed; every one draws the `~EF2*` art).** Front with the HUD in
the glass and the six small-layout windows; look left and right 45 degrees (the art
turns away with the look, the side mirror comes into view and shows the rear feed); 90
degrees left and right and 60 degrees up (no cockpit art, open sky and ground, as for
the other aircraft); `--flight-view 4` (F3 Up) and `--flight-view 3` (F2 Back, the
tailplanes and fin from the pilot's eye). `Airframe::load` accepts the family: the
three overlays `~EF2_LH.PIC`, `~EF2_CH.PIC` and `~EF2_RH.PIC` and the instrument frame
`~EF2_P.PIC` (81 x 80) exist, and `Hud::parse` of `E2000.HUD` names `~EF2_P.PIC`. The
family is thirteen pictures in `FA_1.LIB`; no `~TOR*` cockpit picture exists (TOR-0).
The art is the Eurofighter's single-seat cockpit; the Tornado is a two-seater (next
paragraphs).

**Mirrors (fitted, agent decision 2026-11-11).** The art has three flat fills of one
blue-grey (105, 113, 134): the top centre panel and two lower canopy rails. An `"EF2"`
row in `mirrors::seeds` (centre (640, 50), left (200, 330), right (1090, 330)) fills them
with the shared rear feed. `--cockpit-sheet OUT_DIR tornado,rafale,f18` (through
`tools/agent-run.sh`) prints `tornado: stem TOR cockpit ~EF2H.PIC art 1280x490 FITTED
CLR`, the centre at x 479 y 11 w 319 h 78, the left at x 125 y 183 w 153 h 262 and the
right at x 999 y 183 w 153 h 262; the left and right sizes equal the `~EF2_LH` and
`~EF2_RH` overlay pictures (153 x 262, checked in a retail-reading test in
`mirrors.rs`). Each flood stays under the cap (the largest is 19,816 pixels).
See [mirrors](../baselines/mirrors.md#2026-11-11-tornado-cockpit-and-mirror-seeds-tor-7).

**Instruments and two engines.** The Systems window shows `THR 70%`, `TEMP 0%`, `OIL
100%`, `HYD 100%` and `FUEL 11221 LBS`, with `(+ EXT 3300 LBS)` for the default tank
pair: one engine readout serves every aircraft, so the two-engine Tornado shows one
throttle and one temperature (the fleet readout; the panel has no per-engine needle).
Fuel reads the Tornado's own record, 11,221 lb. The Weapons window lists `360 BK27`,
`4 GBU-10`, `2 MK-82P3` and `60 CHAFF` / `60 FLARE`. The ten instrument pages (0 to 9)
draw with the `~EF2_P.PIC` bezel; the Envelope page draws the Tornado's own envelope
(the corrected B5-2 G rows); the Weapons page of the headless `--panel-snapshot` reads
`NO WEAPONS`, as for every aircraft (the loaded Weapons window is in the live frames).

**Eye point: a two-seater (agent decision, 2026-11-11, left unchanged).**
`mirrors::pilot_eye` is one fixed offset for every aircraft (7 ft above and 10 ft
ahead of the model origin), used only by F2 Back, the rear-view mirrors and the replay
views; the forward cockpit view looks from the origin with 2D art. The record's
`viewOffset` (0, 20, 60) is the Su-24 record's and no host code reads it (pinned in the
test). The Tornado shape has no separate canopy faces or canopy state word (TOR-3), only
a raised top: reading the neutral `TOR.SH` faces within 5 units of the centreline, the
top of the fuselage and canopy stands 5 units, 3.3 ft at 2/3 ft a unit, above the origin
from about 0 to 26 units ahead (0 to 17 ft), with one raised piece 15 to 26 units ahead
(10 to 17 ft). The fixed eye (10 ft ahead) is 3.7 ft above that top and sits at the
back of the forward piece, which is plausibly where the front-seat pilot's head is.
There is no rear seat to move to: TORE draws one pilot's cockpit (the Eurofighter art is
single-seat), and the real rear-seat weapon systems officer has no seat, view or
station. A seated or per-aircraft eye would be a fleet change (a table keyed by aircraft
through six call sites), a question for John, as in the Mirage 2000 and F-5E reviews.

**Test.** `crates/tore-import/tests/tornado_cockpit.rs` (retail-reading, skips without
media): the four sensor rows `VIS240.SEE, F18R.SEE, MIG31.ECM, PAVESPK.SEE`, `hud()`
`E2000.HUD` different from the Rafale's, stem `EF2` with shape stem `TOR`,
`~EF2H.PIC`, `~EF2_P.PIC` from `Hud::parse`, the overlays, thirteen `~EF2*` pictures,
no other roster aircraft on the family, no `~TOR*` picture and the record's eye offset.
The aperture is pinned in `hud_aperture.rs` and the mirror seeds in `mirrors.rs`.

Provenance: sensor rows, HUD module, panel names and cockpit art spec-derived (retail);
the Eurofighter cockpit binding opinionated (John, 2026-11-10); the glass polygon, the
mirror seeds and the eye-point reading fitted or agent decisions (2026-11-11). Evidence
and frames: `.local/m3-aircraft/frames/TOR-7/` and `.local/m3-aircraft/checks/TOR-7/`.

## Tornado flight review (TOR-8, 2026-11-11)

Dated section, additions only. The Tornado IDS flies the hybrid adapter's own
`variety` configuration built from `TOR.PT` (`tor.SH`), two RB199 Mk 103 at 17,300
lbf military and 32,150 lbf afterburner in all, empty 31,065 lb, maximum takeoff
61,620 lb, rows -3 to +7 G (the +7 row fitted, real +7.5) and fuel flows 3 and 22
lb/s (B5-2 and B5-3). No envelope, G, weight, fuel, flow, ceiling or thrust value
was changed and no flight constant was touched. Figures are given clean (42,286 lb
with full internal fuel) and with the current default load (56,982 lb gross),
because the stations question (TOR-5) is open with John.

Update (TOR-5b, 2026-11-11) to the TOR-8 default-load figures: the default load now
carries one 250 gallon tank (1,650 lb of external fuel, was 3,300 lb with the pair), so
every default-load figure in this section (gross weight 56,982 lb, liftoff, ground run,
approach, rollout, loaded climb, top speed, turns, pull with stores, endurance with
external fuel) is lighter than measured by the second tank (at least its 1,650 lb of
fuel, plus its empty shell); the clean figures, the record envelope
and the flows do not move. The Quick Mission and AI wings of the TOR-9 section load the
same default.

- Top speeds: afterburner level speed 793 kt (Mach 1.20) at 1,000 ft and 1,321 kt
  (Mach 2.30) at 36,000 ft, 0.965 of the 1 G edge; with the default load 760 kt and
  1,266 kt (Mach 2.21). Published: about Mach 1.2 at sea level, Mach 2.2 at altitude.
  Level flight holds to the record's 60,000 ft, 20 percent over the published 50,000 ft
  service ceiling (the record's own value, not a B5-2 correction; option for John).
- Climb: 30,000 ft in 91 to 101 s clean and 120 to 134 s loaded from a 450 kt start,
  sustained rate 25,400 to 26,000 ft/min, against "less than two minutes" from brake
  release (published); the 30,000 ft/min of the brief was not found in any page read.
- G: 6.17 G at 461 kt and 1,000 ft with full fuel, 7.04 G nearly dry (never more than
  0.04 G over the +7 row; real +7.5), 5.29 to 5.91 G with the default load; the same
  four fuel states for the F-15E, F-5E, Gripen and Mirage F1 are in the baseline. The
  sustained turn is 4.82 G, 10.2 deg/s at 461 kt and 1,000 ft.
- Fuel: 62 minutes at military power and 8.5 at afterburner from 11,221 lb.
- Takeoff and landing (default load): liftoff 150.6 kt after 1,945 ft (593 m),
  approach 160.1 kt, touchdown 158.7 kt, rollout 757 m. The published landing run
  (370 m) uses thrust reversers, which TORE does not model; open for John.
- **Swing wing.** No flight law reads the sweep (TOR-3, TOR-5 draw it and swivel the
  pylons only). The real sweep changes lift and drag a lot; TORE has no sweep term.
  Because the drawn sweep is a function of speed, the Tornado's own envelope rows
  already carry the speed-dependent result; a sweep multiplier would be a fitted
  per-aircraft change, not made. Finding for John.
- The liftoff (135 to 165 kt) and approach (140 to 175 kt) ranges in the flight battery
  are fitted plausibility windows, agent decisions of 2026-11-11 (no primary speed
  was found for the IDS). Full tables, sources and commands are in the
  [variety flight baseline](../baselines/variety-flight.md#panavia-tornado-ids-validation-2026-11-11).

## Tornado IDS in missions, multiplayer and replay (TOR-9, 2026-11-11)

Implementation and verification, slice TOR-9 of the Tornado port. `AircraftId::Tornado`
was appended to every identity list in B5-1 (checkpoint code 44, `SELECTABLE` index 44,
carried in the 9-bit wire field since protocol 83), and `strike_class.rs` lists it as
Fighter (B5-1, interim), so the AI wing build needed no change: it reads the record's
two engines (the roster probe's `models=2`) and the default load (the BK27 gun with 360
rounds, four GBU-10 and two MK-82P3; the probe prints `stores=[BK27 x360, GBU-10 x4,
MK-82P3 x2]`, a friendly wing leader also carries the two tanks, 3,175 lb of external
fuel). The default load depends on the open stations question (TOR-5): the numbers
above are the current default load, and the AI wing, its payload and its range will move
with whatever John decides. No AI behaviour work was done (Mirage plan rule 1).

Added coverage:

- A Quick Mission test, `the_tornado_is_a_quick_mission_player_and_a_wing_choice`,
  naming `TOR.PT` as the player's aircraft and in every AI wing field and asserting
  that `SU24.PT` appears in neither the player nor the wing list in its place (the
  Su-24 stays a parked target, B6).
- A checkpoint test in `tore-sim`,
  `the_tornado_round_trips_at_code_44_and_no_identity_shares_it`: the coded identity is
  the single byte 44, every identity in `ALL` round-trips, no two share a code, 46 is
  unknown. The swing wing's sweep and the swivelling stations' turn are not saved: both
  are pure functions of speed and flaps (`combat::swing`), recomputed each tick.
- `ai-tornado-quick-mission-2v2` and `replay-rec-tornado-2v2` battery scenarios, the
  Tornado row in the Load Ordnance notes loop
  (`menus-snap-quick-lobby-ordnance-laser-notes-tornado`: no laser warning, sensor
  helper shown, as the F-16C and F-15E, because the record has the PAVESPK.SEE row;
  decision 22 keeps that sensor with no drawn pod), and `ai-tornado-*` in the
  `ai-fights` battery family.

**A fix found by the replay check (shared code).** `--verify-render` on the first
Tornado recording failed on every tick (`aircraft 0's stores differ at tick 1`,
`differing=2400`). TOR-5 put the swing wing's turn on each swivelling station
(`Rack::turn`), set by the host from speed and flaps and not recorded; the comparison
between the live picture and the rebuilt one counted it as a difference. The turn is
derived, and the drawing sets it again from the aircraft's own speed and flap devices
(`pylons::swung`), so `tore-session`'s `recording::convert::difference` now leaves
`Rack::turn` out of the stores comparison (`same_stores`). The devices the sweep comes
from (speed, slot 9, and flaps, slot 1) are still compared within their precision on
every tick, which keeps the replayed sweep within about 0.1 degree of the flown one.
The recorded aircraft are drawn over the recorded speed device (`pose.devices`), not the
model's 450 kt start state: the replay rebuilds the devices, and `model_pose` applies
them over the start state, so a replayed or remote Tornado draws its own sweep. The fix
is in shared code (`recording/convert.rs`) but only changes a result where
`Rack::turn` is nonzero, which is the Tornado alone.

**Replay sweep check.** The unit test
`replay::convert::tests::a_replayed_tornado_draws_the_wing_sweep_it_flew` records
Tornados at 700 kt (fully swept, 42 degrees of turn), 425 kt (half), and 700 kt with
half flaps (half, the flaps hold the wing out), reads them back through a replay file,
and checks that the sweep worked out from the rebuilt devices matches the live one
within the devices' precision, that a wrongly recorded speed is reported, and that
stores equal but for the station turn are one picture while stores that differ in
anything else are not. In the real probe recording the AI Tornados fly 545 to 580 kt
(the AI's cruise; about 93 percent swept, 39.9 degrees of turn at 579.6 kt): the
recorded speed device at tick 1320 is 969.76 ft/s for the enemy leader, and the
replayed frame draws the wing swept back (frames below).

Results (2026-11-11, throttled, profile `.local/m3-aircraft/profile-tornado`, logs in
`.local/m3-aircraft/checks/TOR-9/`):

- `--ai-roster-probe-ticks 3600`: the four `TOR.PT` lines (Novice, Average, Experienced,
  Ace) each `ticks=3600 models=2 radar=true ir=false stores=3 gun=true projectiles=0
  dropped=0 PASS`; the whole roster has no FAIL line (180 PASS).
- `ai-tornado-quick-mission-2v2` passes: every wing the Tornado, nothing dropped, no
  warnings, no anomalies. The attack script's stations are `BK27 x360`, `GBU-10 x4 ...
  (not air to air)`, `MK-82P3 x2 ... (not air to air)`, and the Tornado's record has no
  infrared sensor row, so the AI fights with the gun: 32 rounds, no hit, no kill in this
  run. Existing weapon-selection behaviour, not tuned.
- `replay-rec-tornado-2v2` passes: `--verify-render` PASS `ticks=2401 missing=0
  differing=0`, two runs identical, the recording reads back `player.aircraft = TOR.PT`,
  `probe.enemy = TOR.PT`, `draw.models = TOR.PT`, the aircraft listed as `Tornado (TOR.PT)`.
- `net-content-missing` and `net-window-lobby` pass after rebuilding `tore-server` and
  `tore-bot`. They take no aircraft argument, so they cover the identity list and
  content code, not a Tornado in flight over the wire.

**AI strike finding (reported, no behaviour work).** The strike class is Fighter (B5-1),
but the AI's Quick Mission role comes from the record's object class word, and
`TOR.PT` carries `0x4000`, the bomber class (the same as the A-4E, the F-111F and the
Su-24; the F-15C and F/A-18D carry `0x8000`). So an AI Tornado in a Quick Mission with a ground target
set takes the Strike role by default, unlike the AI F-15E, which keeps CAP:
`--a2g-run UKR QUFACT --pilot ai --aircraft tornado --load MK82P3 --quick-wing
--seconds 300` prints `ROLE plane=1 role=strike` and the run ends `releases=3
impacts=3 kills=1 egress=yes` (target hit points 100 of 100 at the summary, one
kill), and on an engage-my-target order against `UKR QUCOL` it releases three,
lands them 345 to 544 ft from the aim, kills two and egresses. So an AI Tornado
bombs by default when the mission has a ground target and the wing is a friendly
wing past the first, and otherwise (no ground target, or an enemy wing) flies as a
fighter with the gun only: its laser-bomb and Mk 82 stations are not air to air. Not
tuned.

Frames (`.local/m3-aircraft/frames/TOR-9/`): `ai-tornado-live-chase-t1320` and its 4x
crop (a live AI Tornado at 580 kt from behind: both wings swept back, twin burners),
`quick-mission-aircraft-list-tornado` (Tornado IDS on page 3 of 4, no Su-24),
`replay-high-speed-sweep-top-t1320` and `-side-t1320` (the replayed Tornado at tick
1320, 580 kt, planform and side), `replay-banner-580kt-t1320` (the viewer's own banner,
580 kt).

Provenance: the AI wing wiring, default load and class word are spec-derived (the
record); the stores comparison change and the replay sweep check are agent decisions of
2026-11-11 (the sweep schedule and swivel rule stay as TOR-3 and TOR-5 left them). No
retail bytes committed.

## Tornado stations moved, one tank and a glove plate (TOR-5b, 2026-11-11)

Implementation and measurement mode, slice TOR-5 finish (round 3). John's answers T1 to T5
of 2026-11-11 (opinionated) were applied except T2, which stopped on clipping again. The
station table, the clearances at 25 and 67 degrees (default load and 22 alternatives), the
option lists and the cited real-aircraft figures are in
[stores on pylons](../spec/stores-on-pylons.md#the-tornado-ids-inner-wing-pair-on-the-wing-one-centreline-tank-a-glove-plate-tor-5b-2026-11-11);
the glove plate is in
[the Tornado control rig](aircraft-control-surfaces.md#tornado-glove-plate-tor-5b-2026-11-11).
Here, the format-level facts:

| Item | Before | After |
| --- | --- | --- |
| Row 7 (`MK82P3.JT` x2, Wing, flags 0x505) | recorded 3.67 ft out, 3 ft forward (fuselage side) | `station_move`: 9.5 ft out, 1.5 ft forward, on the swinging panel; swivels as TOR-5 built it. Update (TOR-12, 2026-11-12): 7.96 ft out, 1.7 ft forward, the same place on the panel after the panel moved Update (TOR-14, 2026-11-13): 0.38 ft aft, the panel's mid-chord there |
| Row 5 default (`F250.GAS`, Wing, x 0, capacity 2) | two tanks, 3,300 lb external | one tank at x 0, 1,650 lb; capacity, mask and place unchanged (`default_counts`, guarded to the retail row) |
| Row 6 (`PAVEWAY.JT` x4) | x 2.00, entering the belly 0.96 ft (fin tips) | unchanged (no outboard move clears it; options with John) |
| Tornado shape | 287 faces | 291: four glove plate faces added by the rig at load. Update (TOR-12, 2026-11-12): the plates are gone, the shape is 279 faces ([the wing placement](aircraft-control-surfaces.md#tornado-wing-placement-tor-12-2026-11-12)) |
| Row 8 | retired | retired |

The retail records are unchanged; row 7's recorded position is still the record's, and the
move is applied by `station_move` only where the record is the Wing row (the guard
`placed_on` applies). The audit pins do not move (45 aircraft, 18 stations, 4,652
weapon placements, 233 tank placements, 220 Cheat-only placements, 80 pod placements):
capacities are untouched and the default rule changes only the quantity that starts loaded.

Provenance: John's answers opinionated (2026-11-11); row 7's 9.5 / 1.5 ft position, the
default-rule form of T3, the plate geometry and every measurement agent decisions or
findings; the real station layout (seven stations, two swivelling pylons a wing, pivots in
the fixed nibs) from the public pages cited in the stores specification, with no offsets
claimed. No retail bytes committed.

## Tornado ordnance masks, BK 27 gun and the live release at both sweeps (TOR-6, 2026-11-11)

Implementation and review mode (Claude Sonnet, sonnet-high). Update to the
[Tornado stations](#tornado-stations-moved-one-tank-and-a-glove-plate-tor-5b-2026-11-11)
section: the 4,652 weapon placements quoted there are 4,616 on this branch after the
refusals below. Details: [aircraft ordnance](aircraft-ordnance.md#the-panavia-tornado-ids-tor-6-2026-11-11),
[stores on pylons](../spec/stores-on-pylons.md#the-tornado-ordnance-masks-and-the-measured-catalog-tor-6-2026-11-11),
[baseline](../baselines/ordnance-availability.md#tornado-ordnance-tor-6-2026-11-11).

- **Default load** unchanged from TOR-5b: `BK27.JT` x360, one `F250.GAS`, four `PAVEWAY.JT`,
  two `MK82P3.JT` packs; 12,848 lb with the tank's fuel, 55,134 of 61,620 lb on the Load
  Ordnance page. The jammer row stays as retail (60 chaff, 60 flares).
- **Refusals** (agent decision applying John's F-15E A2 precedent, 2026-11-11): 33 stores on the
  shoulder pair and 3 on the inner wing pair, each over 0.3 ft into the airframe (measured
  for every catalog store); the default Paveway and the two stores that hang its shape stay.
  Weapon placements 4,652 to 4,616. The air-to-air page keeps only `AA-11B` and `R-60M`.
- **BK 27:** the shared gun path fires it as it does the M61, DEFA and GSh records (no gun
  code changed); 360 rounds equal the real two guns of 180; muzzle velocity and cadence are
  the shared fields (about 19 percent under the real muzzle velocity; 32 rounds a second at
  the bottom of the real two-gun 33 to 57 range); one muzzle point on the starboard side
  (the real aircraft has two guns).
- **Live release at both sweeps:** row 7's packs leave from 9.50 ft out at 25 degrees and from
  the swivelled pylons at 67 degrees (left -10.53 / -0.81 ft, right 10.36 / -1.26 ft); row 6
  never moves; jettison does the same.
- **Tooling:** `--flight-speed` scales the starting velocity too (an Update beside the TOR-3
  text); the `jettison` attack prints each store's offset from the aircraft;
  `SHOTS_PER_ROUND` in `flight.py` counts a Mk 82 pack as three shots and one round.
- Provenance: rows, masks, capacities, gun record, jammer record spec-derived (retail); refusal
  list, thresholds, battery scenarios and findings agent decisions 2026-11-11; real gun facts
  (two BK-27 with 180 rounds each, 1,000 to 1,700 rounds a minute, 1,100 m/s) from public
  pages for plausibility only (aerospaceweb.org, milavia.net, Wikipedia "Mauser BK-27", read
  2026-11-11). No retail bytes committed.

## Tornado AIM-9 side rails (TOR-6b, 2026-11-11)

Implementation and review mode (Claude Sonnet, sonnet-high). Update to the
[TOR-6 section](#tornado-ordnance-masks-bk-27-gun-and-the-live-release-at-both-sweeps-tor-6-2026-11-11):
the Tornado has an AIM-9 again, as one side rail on each inboard swivelling pylon, appended as
row 8 (John, 2026-11-11, opinionated). The weapon placements 4,616 quoted there are 4,623
here, the Cheat-only placements 220 are 222 (the rail row takes the two novelty bombs under
Cheat loading, as the retired outer row did), and `TOR.PT` reads 56 placements and a catalog
of 36 (49 and 31 before). Details: [aircraft ordnance](aircraft-ordnance.md#the-tornados-aim-9-side-rails-tor-6b-2026-11-11),
[stores on pylons](../spec/stores-on-pylons.md#the-tornado-aim-9-side-rails-on-the-inboard-swivelling-pylons-tor-6b-2026-11-11),
[baseline](../baselines/ordnance-availability.md#tornado-aim-9-side-rails-tor-6b-2026-11-11).

- **Mechanism:** the F-14's shoulder rail (`Location::Shoulder`, `Hang::Shoulder`) on row 7's
  swivelling pylon, listed beside it in `swing::swivels`; the rail's 1.25 ft sideways offset
  stays beside the pylon at every sweep (one guarded change in `Rack::side_pylons`).
- **Position:** 10.75 ft out (1.25 ft outboard of the pack's pylon), 1.50 ft forward, fitted;
  no source gives the flank, the outboard one is the F-14's and keeps the missile clear of
  the fuselage.
- **Mask:** AIM-9B, AIM-9M, AIM-9X, AA-2, AA-8, AA-11, AA-11B; PL-7, R.550, AT-12 and the
  GBU-12 refused (0.40 to 1.02 ft into the airframe or not a rail missile).
- **Default:** `AIM9M.JT` x2, the retail default, one a side.
- **Clearance:** every kept missile 0.00 ft into the airframe at 25 and 67 degrees; the
  AIM-9M is 0.06 ft clear of the Mk 82 pack by bounding box; gear legs 5.5 ft (25 degrees) and
  6.3 ft (67); the other row 7 stores cross the missile's box by up to 0.79 ft (not a refusal,
  as at TOR-6).
- **Live:** an AIM-9M leaves from 10.75 ft out, 1.50 ft ahead at 25 degrees and from
  -11.78 / -0.81 (left) and 11.61 / -1.26 ft (right) at 67 degrees; it jettisons from the same
  points. Scenarios `a2g-release-origin-tornado-rail`, `a2g-jettison-tornado-rail`,
  `flight-livefire-tornado-slot4` and `flight-livefire-tornado-slot4-sweep67`.
- Provenance: John's request opinionated; row content spec-derived (the retail row 8); the
  position, flank, refusals, the guarded sim change and the measurements agent decisions or
  findings (2026-11-11). Real carriage (one AIM-9L on the inboard stub pylon of each wing)
  from public pages for plausibility only. No retail bytes committed.

Update (TOR-11, 2026-11-12): the side rail is on the swivelling pylon's **inboard** flank (John:
"The AIM-9 rail on the wing pylon is too long and on the wrong side."), as `Location::InboardRail`
(byte 0x84) with `Hang::InboardRail`, the missile 1.25 ft inboard of the pack's pylon (8.25 ft out at
25 degrees; the Position and Live bullets of the section above are the 2026-11-11 outboard
state), and its drawn arm is 3.0 ft (was 5.6, fitted). The Tornado's wing also starts to sweep at 304 kt
(350 mph, John), not 250. Row 8's location byte in the source record, the mask, the placements
(4,581 on the merged tree) and the Cheat-only 222 do not move. Details: [stores on pylons](../spec/stores-on-pylons.md#the-tornado-aim-9-rail-on-the-inboard-flank-shorter-and-the-sweep-from-350-mph-tor-11-2026-11-12).

## B-52 source review (2026-11-15)

Research mode, slice B52-0 (bomber batch A, the first of three source reviews run
side by side). This reviews `B52.PT` (B-52 Stratofortress) and its dependency
closure before `AircraftId::B52` exists. `B52.PT` is not a parked target
(no `parked_aircraft.rs` entry). No original module was executed. PT, SEE, ECM,
JT and HUD are read as bounded BRF data with the shared field order in
`aircraft_schema.rs`; SH is inspected as data (`shape_inspect`, `parked_inspect`).
Slice done by Claude Sonnet (sonnet-medium). Nothing here changes code or
behaviour. Real-aircraft figures are cited and never establish game behaviour.

Questions open for John, **not decided here**: the variant
([below](#b-52-variant-b-52h-or-b-52g-for-john)), the record template values
([below](#b-52-the-template-copy-list-for-john)), the HUD and cockpit
([below](#b-52-hud-and-cockpit-for-john)), the surfaces that fail or strain the
whole-surface gate ([below](#b-52-surfaces-for-john)), the gear and outriggers
([below](#b-52-gear-and-outriggers-for-john)), the size
([below](#b-52-real-size-for-john)), the gun
([below](#b-52-sensors-jammer-and-gun-for-john)) and the bay and pylon stations
([below](#b-52-bay-and-station-survey-for-john)).

### B-52 build and extraction

`FA_1.LIB` and `FA_2.LIB` are the same pair as the MiG-31 review (see the
[MiG-31 build section](#build-and-extraction)). The closure was extracted locally
to `.local/m3-imports/scratch/b52` with `tore-extract --include 'B52*'
--include '_B52*' --include 'BMBROH.HUD' --include 'B.BI' ...` plus the SEE, ECM,
JT files the record names and sixteen comparison records (`AC130`, `C130`,
`KC135`, `E3`, `AF1`, `B747`, `B707`, `C5`, `C17`, `B1`, `TU95`, `TU26`, `TU160`,
`ATL`, `IL76`, `A310`). Name sweeps found no B-52 cockpit art (`~BMB*`, `~B52*`,
`*B52*`, `~B5*`, `*STRAT*`, `*BMB*`): only the shapes, skins, the `^ACB52.5K`
callout, `B52_0` to `B52_4.PIC` and `B52.INF` (encyclopedia, `disc2/FA_3.LIB`) and
`CUB52.PIC` (one of 66 numbered `CUB0` to `CUB65` pictures, not a B-52 file). The
`SETUP.ESA` installer copy of `FA_2.LIB` holds byte-identical `B52.PT`, `B52*.SH`,
`_B52*.PIC`, `B52.ECM`, `B52R.SEE`, `VIS300.SEE`, `AGM84E.JT`, `M61.JT`, `B.BI` and
`^ACB52.5K`; only `BMBROH.HUD` differs (1 stored byte, a build difference). The usual
five unrelated archive errors appear.

### B-52 identity and record

| Field | Decoded value |
| --- | --- |
| PT | `B52.PT`, 9,123 bytes, SHA-256 `45948ce46e0aebc7f3c68df47658e593d9ed4c0cfb79860bd6591477714709f4` |
| Displayed names | `B-52`, `B-52 Stratofortress`, `B52.PT` |
| Schema | OBJECT + NPC + PLANE; `structType` 5; **`typeSize` 612**; `instanceSize` 592 |
| Object class / year / hit points | `0x4000` (attack) / 1961 / 370; `expType` 30 |
| PLANE flags | `0x15`: base `0x01`, multi-crew `0x04`, seat `0x10`; hook, no-lift and carrier clear |
| Engines | **8** (real: eight TF33), thrust 120,000 lbf total, **no afterburner** (0) |
| Shape | `b52.SH` (33,280 bytes, header scale 2), shadow `b52_s.SH` (16 faces) |
| Damage looks | `B52_A.SH` 372 faces, `B52_B.SH` 116, `B52_C.SH` 387, `B52_D.SH` 84 |
| Skins | `_B52.PIC` 120,964 bytes; `_B52_A/B/C/D.PIC` 139,684 / 60,644 / 131,364 / 86,904 |
| Sounds | `&jet09.11k` loop and second sound, `&powerup.5k`, `&powerdn.5k` (all present) |
| HUD | `bmbroh.HUD` (the AC-130 panel set, see below) |
| AI script | `b.BI` (4,608 bytes, shared with the Tu-95, Tu-26, Tu-160 and the B-2 row) |
| Sensors / jammer / gun | `VIS300.SEE`, `B52R.SEE`, `B52.ECM`, `M61.JT`; stores `AGM84E.JT` |
| PTS | none: no `B52.PTS` exists |
| Radio callout | `^ACB52.5K` exists (5,777 bytes) |
| Encyclopedia | `B52.INF` (titled "B-52G Stratofortress", data lines for the B-52H), `B52_0` to `B52_4.PIC` in `disc2/FA_3.LIB` (Jane's text, reference media) |
| Checkpoint code / CLI key | assigned by BMB-A1 |

The decoded record equals the catalog row in [`fa-aircraft.csv`](fa-aircraft.csv)
field for field (1961, `0x4000`, 8 engines, 235,220 / 199,000 / 488,000 lb,
120,000 lbf, 515 kt top, 385 kt sea-level maximum, 55,000 ft, G -2/+3, 370 hit
points, 7 hardpoints, the default-store list, `bmbroh.HUD`). **BMB-A1 must add a
type-size entry:** `AircraftId::B52 => 612` in the `typeSize` match in
`crates/tore-formats/src/aircraft.rs` (the AC-130, A-4E, MiG-21, F-4B and A-7 use
612); the fall-through 660 would refuse to load. `strike_class.rs` already tests
`B52.PT` as `Heavy` and `sensors/profile.rs` already lists `B52.ECM` as a
transitional jammer label (no identity needed for either).

Hashes of the files later slices depend on:

| Resource | Bytes | SHA-256 |
| --- | ---: | --- |
| `B52.PT` | 9,123 | `45948ce46e0aebc7f3c68df47658e593d9ed4c0cfb79860bd6591477714709f4` |
| `B52.SH` | 33,280 | `5d5816166e529b932382f98741711fb450667225a1ece723eec2ea96dd20eaa2` |
| `B52_S.SH` | 4,608 | `9c34bafc97a42ebd40eef854629e8c225e995872a61273c44a1cc2a37e801509` |
| `B52_A.SH` | 25,088 | `44a9b53b436666482644b7c5b9913a5fb531f6fd85322e2f1a3bc7c218933158` |
| `B52_B.SH` | 12,800 | `48872699f2b3e939f821535f0963d513ff2e456c20d27df7a4be0e5e41108d33` |
| `B52_C.SH` | 29,184 | `c004de4a731f60c94e6e7a913bde03efb0418d68356f8bb640fb317d837576b8` |
| `B52_D.SH` | 8,704 | `14d84b339e2951cabb589116a20caa0add5d4fb180275c4f3bafaa3ab0de3a61` |
| `_B52.PIC` | 120,964 | `a1ca126a5fa1ba7d694333000628f22d1bcba774a4630af574e3154757f863c8` |
| `BMBROH.HUD` | 4,608 | `3eb15857d970c2f3f48f3f9482cbfaa8c1463aef4275a2198d6065d55ef7bbf9` |
| `B52R.SEE` | 469 | `f8ca6916a255625c29a3125b569e363b8ebee5bb9c5412b9d8022ac8dae3899b` |
| `B52.ECM` | 352 | `98816f38e7f9a9540135459dccc4926a072e97c0102110a815679635ca11234d` |
| `VIS300.SEE` | 471 | `2369a8be84e01882666a6d275f82edd02e631fdf0871f7ea66bc8c81f3b71cfb` |
| `AGM84E.JT` | 2,779 | `2f350f561011568d7440c3c3dd11a208ec7103b76d232484164ab2340a7562bf` |
| `M61.JT` | 2,703 | `0b79689a728911d1097569f6e6612050e414ba9703c9f131a51fe1ce9a7eb33d` |
| `B.BI` | 4,608 | `781f27cddf73075f36659fe0910dda009a457cdd3f2386de6c9b8bb78946e7ae` |

### B-52 the template copy list (for John)

`B52.PT` has no single template. Of its 207 non-pointer statements (the 45
system-damage bytes included) it is identical to `AC130.PT` in 169, `TU95.PT`
167, `E3.PT` 167, `C130.PT` 167, `KC135.PT` 166, `IL76.PT` 166, `C5.PT` and
`ATL.PT` 165, `TU26.PT` 162, `TU160.PT` and `C17.PT` 159, `B707.PT` 158, `AF1.PT`
147, `B747.PT` 144, `B1.PT` 139 and `A310.PT` 136. All 45 system-damage bytes
equal every one of the sixteen records. So the **flight-tuning block** (stall,
spin, turn-rate acceleration words, drag and crash words: most of the 169 shared
statements) is the shared large-aircraft family template, closest to the AC-130 (38
differences, listed below). The values below are the B-52's **own** and match no
candidate (none of the sixteen holds them), which is the opposite of the
Tornado/Su-24 case.

Authored for the B-52 (no candidate shares the value): year 1961, view distance
1,451, `sigs[0]`/`sigs[1]` 277, hit points 370, empty weight 235,220, structure
651 / 904, engines 8, thrust 120,000, fuel flow 18, internal fuel 199,000,
`_gpullDrag` 17, wheel-brake drag 76, flaps drag 51, flaps lift 30, loaded
elevator/aileron/rudder 39, maximum takeoff 488,000, view offset z 75, search
frequency 4, the hardpoint rows and all six G rows. Shared with some candidates
(coincidence or family): `bayDrag` 25 (Tu-95, Tu-26, Tu-160), `throttleAcc` 10 (AC-130,
C-130, Tu-95), `dmgDebrisPos.x` -180 (E-3, Tu-160), `sigs[3]` 100 (Tu-26, Tu-160),
`sigs[2]` 200 (C-5), `maxAlt` 55,000 and PLANE flags `0x15` (B-1B), `typeSize` 612,
`instanceSize` 592 and `numHards` 7 (AC-130), sounds `&jet09.11k` (KC-135, E-3,
IL-76, Tu-26), and the sensor/ECM rows (below).

Each authored value against cited real B-52H figures (Wikipedia "Boeing B-52
Stratofortress", Specifications (B-52H), accessed 2026-11-15; Jane's text in
`B52.INF`; aircraftinformation.info B-52H page, accessed 2026-11-15):

| Field | Record | Real | Difference | Verdict |
| --- | ---: | ---: | ---: | --- |
| Empty weight, lb | 235,220 | 185,000 | +27.1% | Not a copy and not real; authored high |
| Maximum takeoff, lb | 488,000 | 488,000 (Jane's and Wikipedia) | 0% | Agrees exactly |
| Internal fuel, lb | 199,000 | 312,197 (47,975 US gal, Wikipedia); Jane's 174,130 L (46,000 US gal, about 308,000 lb) | -36% | Low |
| Engines | 8 | 8 TF33-P-3 | none | Agrees; shape draws four twin pods |
| Military thrust, lbf (total) | 120,000 (15,000 each) | 136,000 (8 x 17,000; Jane's 75.6 kN, Wikipedia 17,000) | -11.8% | Low; no afterburner (correct) |
| Top speed, kt | 515 (1 G row 870 ft/s at 36,000 ft) | 516 (Mach 0.90, Jane's) | -0.2% | Agrees |
| Sea-level maximum, kt | 385 | none cited | | Authored |
| Ceiling, ft | 55,000 | 55,000 (Jane's), 50,000 (Wikipedia, official) | 0% / +10% | Agrees with Jane's |
| G limit | -2 / +3 | no cited limit found | | Authored, not checked |
| Hit points | 370 | none | | Authored |
| Year | 1961 | B-52H service entry 1961 (not examined) | | Agrees with the H |

Options for John (none chosen, the Mirage 2000 and Tornado precedents were real
thrust, real weights where a cited figure exists, labelled opinionated):

1. Keep every value as retail (documented, audit only).
2. Correct thrust to the real eight engines (136,000 lbf total) and leave weights.
3. Also correct empty weight (185,000) and internal fuel (312,197). Fuel changes
   range and endurance, so B52-8 would recheck the flight envelope rows.

Agent note: the 235,220 lb empty weight plus 199,000 lb of fuel is 434,220 lb, so
a clean retail B-52 is only 11% under its 488,000 lb maximum; the bay and pylons
can load about 50,000 lb at the proposed row classes, so the maximum is not the
binding number. At the real 185,000 + 312,197 = 497,197 lb, a full-fuel aircraft
already exceeds the 488,000 lb maximum; corrections 3 would need the host's
overweight rule checked in B52-8.

### B-52 variant (B-52H or B-52G, for John)

Evidence that the record means the **B-52H**: (1) `M61.JT` (the 20 mm Vulcan) is
the tail gun and the record year is 1961 (the H entered service 1961; the G's tail
gun was four .50 calibre machine guns); Wikipedia: "1 x 20 mm M61 Vulcan cannon
originally mounted in a remote-controlled tail turret on the H-model, removed in
1991"; (2) the encyclopedia page titled "B-52G Stratofortress" gives the power
plant as "(B-52H): Eight 75.6 kN (17,000 lb st) Pratt & Whitney TF33-P-3
turbofans" and the armament as "(B-52H): Single 20 mm Vulcan multi-barrel cannon
in tail turret instead of four machineguns", and Jane's lists the H's power
loading beside the G's; (3) the record's eight engines have no afterburner and its
ceiling and top speed are Jane's B-52 numbers (55,000 ft, Mach 0.90). The shape
cannot settle it: both the G and H have the short fin (40 ft 8 in; the drawn fin
tip is 34.0 ft above the belly), the same wing and four twin-engine pods, and the
same underwing tanks. Options (none chosen):

1. **B-52H** (the record's gun, year and power plant); name stays `B-52`
   (standing decision 7).
2. B-52G (four .50 calibre guns, J57 engines): contradicts the record's gun and
   thrust; no retail record or shape feature supports it.
3. Leave the variant unstated, as retail does.

### B-52 G envelope rows

Points are speed in ft/s at altitude in ft. Six rows, `envMin` -2 to `envMax`
+3; the record has `negGLimit` 0.

| G | Points |
| ---: | --- |
| -2 | 350@0, 420@22,000, 505@34,000, 590@37,000, 660@36,500, 680@27,000, 640@0 |
| -1 | 260@0, 325@24,500, 387@38,250, 490@46,250, 600@49,250, 730@49,000, 790@42,500, 810@34,000, 680@0 |
| 0 | 160@0, 220@27,500, 310@44,250, 445@54,250, 600@59,250, 790@59,000, 890@47,000, 900@36,000, 680@0 |
| 1 | 210@0, 265@25,500, 347@41,250, 470@51,250, 590@55,000, 770@55,000, 860@45,500, 870@36,000, 650@0 |
| 2 | same as -1 |
| 3 | 310@0, 380@23,000, 430@35,000, 515@41,000, 600@43,000, 700@42,500, 740@31,000, 610@0 |

The 1 G row tops out at 870 ft/s (515 kt) at 36,000 ft and 650 ft/s (385 kt) at
sea level, the catalog's two speeds, and reaches 55,000 ft (the record ceiling).
The 0 G row goes to 59,250 ft, above `maxAlt`; the host caps altitude at the
record field.

### B-52 resources

| Resource | Role | Note |
| --- | --- | --- |
| `B52.SH` | Shape | 271 runtime faces (single texture `_B52.PIC`), 0 lines, 0 billboards, five state words (`7a20`, `7a26`, `7a2c`, `7a32`, `7a38`); no usable streamer record (the decoded points are noise: 71,168, 1,536, -33,280) |
| `B52_S.SH` | Shadow | 16 faces |
| `B52_A.SH` / `B52_C.SH` | Damage bodies | 372 / 387 faces |
| `B52_B.SH` / `B52_D.SH` | Damage pieces | 116 / 84 faces |
| `VIS300.SEE`, `B52R.SEE`, `B52.ECM`, `M61.JT` | Sensors, jammer, gun | see below |
| `AGM84E.JT` | Default store (row 3) | AGM-84E SLAM, 1,380 lb, in the TORE roster |
| `b.BI` | AI script | shared bomber script |
| `^ACB52.5K` | Radio callout | present |

The header scale of `b52.SH` is **2** (like the MiG-23 and the Tornado; the
B-1B is 4, most aircraft 1), so the aircraft is authored at 2/3 ft per unit.

### B-52 HUD and cockpit (for John)

**Fact.** `B52.PT` names `bmbroh.HUD`, which exists (`BMBROH.HUD`, 4,608 bytes).
It is the **AC-130 HUD module with 7 bytes changed**: the module's panel names
are still `~ac130`, `~ac130h`, `~ac130s`, `~ac130_p`, `~ac130_w` and its
annunciators are GEAR, FLAP, BRAKE and BAY. Of the seven bytes, one is a flag
byte at offset 137, two are at 1082 to 1083 and four rename the fourth
annunciator (the AC-130's `HOOK` becomes `BAY`). The Tu-95 and Tu-26 rows name
the same module; `BMBRCC.HUD` (the B-1B's module) names the `~b2*` panels. The
bomb-sight rule in [the weapons format](weapons.md) says `BMBROH` sets only
HUD byte `+0x23a`, which gives the HAB (high-altitude bombing) sight mode that
draws no HUD sight, so a bomb-flag store shows no CCIP pipper. **No B-52
cockpit art exists** (sweeps above). So retail flies the B-52 with the AC-130
panel family (`~AC130*.PIC`, 4 pictures in `FA_1.LIB`).

Options (none chosen):

1. **Follow the record: `bmbroh.HUD`, the AC-130 stem and panels**, with the BAY
   annunciator driven by the bay word (below). What retail does; no new art; the
   TORE `Hud` mapping already has an AC-130 arm. Bind by name, never alias
   `AC130.PT`.
2. Substitute the B-2 module (`BMBRCC.HUD`, `~b2*`) or another reviewed family;
   opinionated, no retail evidence for the B-52.
3. Author a B-52 panel; invented art, a later decision.

Agent note: the BAY annunciator and the record's `bayDrag` 25 both point at the
bay word, which supports the bay reading of `7a20` below.

### B-52 surfaces (for John)

State words (neutral 271 faces; counts from `Shape::with_state`, geometry in raw
units, x right, y forward, z up; the header scale 2 makes one unit 2/3 ft):

| Word | Faces | Low point | Geometry | Reading | Status |
| --- | ---: | ---: | --- | --- | --- |
| `7a20` | +4 | -13 | Two plates (each in both windings) at x = -7 and x = +7, y -13 to 26, z -13 to -8: they hang 5 units below the belly edge | **Bay doors open** (the HUD module's BAY annunciator and `bayDrag`; slot order not recovered) | Confirmed by position |
| `7a26` | +32 | -14 | Four trucks and two outriggers (next section) | Landing gear down | Confirmed |
| `7a2c` | -2 | none | Removes the left inboard trailing strip (`4f3b`, `4f5a`): x -53 to -9, y -20 to 7 | Left flap word | Slot order |
| `7a32` | -2 | none | Removes the right inboard trailing strip (`4e54`, `4e73`): x 9 to 53, y -20 to 7 | Right flap word | Slot order |
| `7a38` | +2 / -2 | +11 | Swaps the full-height rudder strip (`50f0`, `5117`) for a skewed copy (`5016`, `503d`) | Rudder (positive direction only) | Confirmed by geometry |

There is **no burner word** (jets, no afterburner), **no airbrake word** (the B-52
has none; Jane's lists a 44 ft ribbon braking parachute, not drawn), no hook, no
slat, no spoiler, no aileron, no canopy and no thrust-reverser word, and no
separate geometry for any of them.

Moving-surface map for the rig:

| Surface | Source panel | Rig note |
| --- | --- | --- |
| Stabilizer (all-moving) | One 4-vertex polygon per side at z 5 (`334e`/`3361` right, `3167`/`317a` left; two windings each): (42,-123), (42,-115), (6,-81), (3,-120) | Only the polygon; no word. Turn whole about a lateral spindle through the root chord midpoint (about (4.5, -100.5, 5)). No vertex is shared with any other face. Real: trimmable +9 / -4 degrees (13 degrees, Wikipedia); no separate elevator face |
| Flaps | One strip per side, 2 faces: x 9 to 53 (inner 32% of the 139 semi-span), chord 8.2 units | Source deletes them with the flap words; the rig keeps them drawn and turns them |
| Rudder | Full fin height (z 11 to 42), chord about 8 units | The word swaps in a copy skewed 4 units (top) and 8 units (bottom); see below |
| Bay doors | Two plates | Appear only when open |
| Spoilerons / ailerons / slats | none | Not in the source |
| Gear | Four trucks (6 faces each), two outriggers (4 faces each) | Rigid assemblies |

**Rule 11 face table** (counted from the face dumps; "hinge" is the flap's forward
edge or the rudder's front edge):

| Surface | Face | Vertices | Moved | On hinge/spindle | Fixed |
| --- | --- | ---: | ---: | ---: | ---: |
| Stabilizer right | `334e`, `3361` | 4 each | 4 | 0 | 0 |
| Stabilizer left | `3167`, `317a` | 4 each | 4 | 0 | 0 |
| Flap right (hinge (15,7) to (53,-14)) | `4e54`, `4e73` | 4 each | 2 | 2 | 0 |
| Flap left (hinge (-15,7) to (-53,-14)) | `4f3b`, `4f5a` | 4 each | 2 | 2 | 0 |
| Rudder (hinge (0,-114,42) to (0,-101,11)) | `50f0`, `5117` | 4 each | 2 | 2 | 0 |
| Bay door plates (4 faces) | `4dfb`, `4e12`, `4d5e`, `4d75` | 4 each | 4 | 0 | 0 (appear whole) |
| Outrigger plates (8 faces) | `482c` to `486a`, `4779` to `47d6` | 4 each | whole assembly | | 0 |
| Truck faces (24) | five and four vertex plates | | whole assembly | | 0 |

Verdicts:

- **Stabilizer.** One quad per side that touches no other face, so a whole-panel
  turn folds nothing and opens no seam. The leading root corner (6,-81,5) is
  19.5 units ahead of the spindle, so +9 degrees lifts it 3.0 units (2.0 ft) and
  -4 degrees drops it 1.4 units (0.9 ft); the polygon floats beside the tail cone
  (fuselage half-width 2 to 6 units there), no visible gap needs closing. Options
  (none chosen): (a) whole-panel pitch about the spindle with the real limits
  (+9 / -4 degrees); (b) a smaller fitted pitch term plus the same panel as trim
  only; (c) leave fixed. Roll: none (the B-52 stabilizer does not roll).
- **Flaps.** Whole-strip faces; two vertices sit on the forward edge and two on the
  trailing edge. The trailing corners (9,1) and (47,-20) are shared with fixed faces
  (`2651`, `2664` at the root; `3d8b`, `3e29` at the outer end), so a turn about the
  forward edge opens those two seams by the full chord (8.2 units, 5.5 ft) times
  the sine of the angle (2.7 ft at 30 degrees). The same seam arises with the
  Tornado trailing strips; no face folds. They span only the inner third of the
  wing; the real flaps are large and the source has no outer flap. Options: rig
  the strips only (what the source holds), or leave them as source deletions.
- **Rudder.** Hinge edge shared with the fixed fin faces `3396`, `3434` (both
  hinge vertices on the edge), so nothing folds. Measured: the skewed copy is a
  **rotation about the hinge edge of about 33 degrees** (the top rear vertex
  moves 4 units where it is 7.4 units from the hinge line, the bottom rear 8
  units where it is 14.8 units away: both give a sine of 0.54), one-sided only.
  So, unlike the Mirage, the retail swap is a clean rotation and the rig can be
  exact; the negative direction is a mirror (fitted).
- **Roll.** The source has **no roll surface**: no aileron (the G and H have none;
  Wikipedia "Flight controls"), no spoileron panel (six or seven per wing in
  reality). The wing top is built from a few large polygons (`3c8b`, `3cbb`,
  `3cf3`, `3e4b`), so any spoileron would be a **partial surface** cut from them
  and fails the whole-surface gate. Options: (a) no drawn roll surface (the
  flight model rolls from the record, nothing moves; recommended by the gate);
  (b) fitted whole spoileron plates added above the wing as new geometry (John's
  call, not source geometry); (c) differential flaps for roll (not the real
  control).
- **Elevator.** Not a separate face; real elevator is a narrow-chord surface.
  Leave to the stabilizer.
- **Bay doors.** Only the open pose exists (see the survey).
- **Thrust reversers, airbrakes, burner, hook, slats, canopy, drag chute:** none
  in the source and none invented here.

The whole-surface gate (John, 2026-11-07/08) is met by every face above, with the
flap seam and the missing roll surface the only strains; no diagonal or partial
surface is in the source, so no probe sheet is needed.

### B-52 gear and outriggers (for John)

The gear word `7a26` adds 32 faces, low point -14 (contact offset -28 in the
parked inspector after the header scale). All are flat rigid plates:

| Assembly | Faces | Position (raw) | Note |
| --- | ---: | --- | --- |
| Forward right truck | 6 | x 5 to 13, y 34 to 45, z -14 to -6; pentagon at y 39; side plate x 12; slanted plate x 7 to 10 | Strut and wheel pair stand-ins |
| Forward left truck | 6 | mirror, x -5 to -14 | |
| Aft right truck | 6 | same shape at y -26 to -37, pentagon at y -32 | |
| Aft left truck | 6 | mirror | |
| Right outrigger | 4 | x 108 to 113, y -38 to -32, z -14 to 3: two crossed plates | At wingtip side of the outer engine pod (x 87 to 96) |
| Left outrigger | 4 | mirror | |

At 2/3 ft per unit: four trucks in tandem pairs 47.3 ft apart (fore/aft centres
y +39.5 and -31.5), truck centres 12 ft apart across, outriggers 74 ft either
side. This matches the real layout ("Four individually retracted twin-wheel
main units in tandem pairs ... Small outrigger units retract inward into wings
outboard of outer engine positions", Jane's, `B52.INF`). The retail shape thus
**does draw the outriggers** (as crossed plates reaching the same -14 contact
line as the trucks) and four bicycle-style trucks. There is no nose gear, no
wheel or door geometry and **no crosswind-gear yaw**: the real trucks pivot 20
degrees either way (Wikipedia, "Flight controls"; Jane's: "set at a selected
degree of castor"). The generic rig already supports rigid assemblies, tandem
bogies and outrigger wheels for other identities (see the variety animation
spec), so the shape and the gear model support all six assemblies. The belly
clears the ground by 5 units (3.3 ft) with the gear down, which looks low next
to the real B-52; no cited clearance was found, so that is an observation only.
Options (none chosen):

1. Rig the six assemblies with the retail gear word only (retract each as one
   rigid piece; the outriggers retract inward, as Jane's says).
2. As 1, plus a fitted crosswind crab (rotate each truck about a vertical axis
   through its centre, up to 20 degrees, driven by crosswind or rudder). Fitted,
   not in the source.
3. Leave the outriggers fixed (not retracting). Not recommended; the source has
   them in the gear word.

### B-52 real size (for John)

Measured on the neutral state of `b52.SH` (clean, gear up, bay closed): x -139 to
139, y -138 to 107, z -9 to 42; gear down low -14. The airframe is drawn at the
header scale (2/3 ft per unit):

| Axis | Units | At 1/3 ft | At 2/3 ft (header scale 2) | Real | Verdict at 2/3 |
| --- | ---: | ---: | ---: | ---: | --- |
| Span | 278 | 92.7 ft | 185.3 ft | 185 ft (56.39 m) | +0.2% |
| Length (nose to tail cone) | 245 | 81.7 ft | 163.3 ft | 159 ft 4 in (Wikipedia, 48.5 m); 160.9 ft (Jane's, 49.05 m) | +2.5% / +1.5% |
| Height (fin tip 42 to gear low -14) | 56 | 18.7 ft | 37.3 ft | 40 ft 8 in | -8.2% |

**Verdict.** The shape is true size at the header scale of 2/3 ft per unit: span
within 0.3%, length within 2.5%. If TORE drew it at the default 1/3 ft per unit it
would be half size (a 93 ft B-52), so BMB-A1 or B52-2 needs the entry `B52 => 2/3`
in `tore_sim::combat::debris::scale` beside `Mig23` and the Tornado. The height is
the one short axis; the airframe from belly (z -9) to fin tip (z 42) is 51 units =
34.0 ft, and the gear legs are short by roughly 3 ft against a real aircraft
standing 40.7 ft tall. No per-axis fit is needed. Options (none chosen):

1. **Draw at 2/3 ft per unit** (header scale; MiG-23 and Tornado precedent).
2. Draw at 1/3 ft per unit (half size; not recommended).
3. Draw at 2/3 and lengthen the gear (fitted, about 3 ft) to reach the real
   height; needs a cited real belly clearance first.

Other measured proportions at 2/3: four engine pods in two rows (inboard pods
x 45 to 60, centre 35.3 ft out; outboard pods x 79 to 96, centre 58.3 ft out),
each pod 10 to 11 ft wide (two engines), 18 ft long; two drawn underwing tanks at x =
+/-115 (76.7 ft out, y -55 to -16, 26 ft long); wing leading edge swept 37 to 38
degrees (Jane's: 35 degrees), anhedral (root z 8 to 11, tip z 3). The wing, tail
and pods are all part of the neutral shape.

### B-52 sensors, jammer and gun (for John)

Seven rows. Rows 0 to 2 are fixed internal rows (flag `0x1008`); the rest are in
the station table below.

- **`VIS300.SEE`**: the large-aircraft visual sensor (the same file the transports use).
- **`B52R.SEE`** (469 bytes): the **Tu-160's radar data renamed** (17 bytes differ
  from `TU160R.SEE`, all names; 41 bytes from `TU95R.SEE`). It is a long-range
  radar class (911,400 range, 10,920 search arcs). The real aircraft has the
  Norden strategic radar and an EVS with FLIR and low-light TV (Jane's); keep as
  retail. `AF1.PT` also names `B52R.SEE` and `B52.ECM`.
- **`B52.ECM`** (352 bytes): identical to `AC130.ECM` and `TU160.ECM` apart from 16
  name bytes (the same jammer block).
  Keep as retail; the real defensive suite is large (ALQ-155, ALQ-172, ALE-20/24
  dispensers) and none of it has a record.
- **The gun (row 4): `M61.JT` in a slewed rear mount.** Yes, it is the B-52H's M61
  tail gun: `M61.JT` is the shared M61 Vulcan record (20 mm, the fighters' and gunships' gun), flag `0x180a`
  gun row with slew heading 32,760 (180 degrees, aft) and arcs 5,460 (30 degrees)
  each way in heading and pitch, 2,000 rounds (the Tu-95 and Tu-26 tail-gun rows carry the
  same `0x180a` flags). Real figures: the H's magazine held 1,242 rounds
  (aircraftinformation.info), fed by the gunner's AN/ASG-21; the gunner position
  went in October 1991 and the gun was removed 1991 to 1994 (same page; Wikipedia
  "removed in 1991 from all operational aircraft"). **Position:** the row sits at
  (0, 20, -220) record units = 0 ft right, 6.7 ft up, 73.3 ft aft; the drawn tail
  cone ends at y -138 = 92 ft aft, centre z 3 (2 ft up), so the mount is 19 ft
  forward of and 4.7 ft above the real tail turret position. Options (none
  chosen): (1) keep the retail row (position and 2,000 rounds); (2) move the mount
  to the drawn tail cone (authored, fitted to the shape) and/or set the rounds to
  1,242 (real); (3) delete the gun for a 1991 and later B-52H (the real aircraft
  has none now). The record is the pre-1991 H.

### B-52 hardpoints (record rows)

All seven rows, in original order, decoded with the standard field order.
Location byte: 0 Centerline, 1 Fuselage, 2 Internal Gun, 3 Internal Bay, 4 Wing,
5 Wingtip. Feet divide the record units by three; position is right / up /
forward.

| Row | Location | Flags | x / y / z | Feet right / up / fwd | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x1008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `VIS300.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x1008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `B52R.SEE` | Fixed radar |
| 2 | Fuselage | `0x1008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `B52.ECM` | Fixed jammer |
| 3 | Internal Bay | `0x1100` | 0 / -40 / 39 | 0 / -13.3 / 13 | 12 | 166 | `AGM84E.JT` | Forward bay row, default 12 AGM-84E |
| 4 | Fuselage | `0x180a` | 0 / 20 / -220 | 0 / 6.7 / -73.3 | 2,000 | 0 | `M61.JT` | Tail gun, slewed aft |
| 5 | Internal Bay | `0x1120` | 0 / -40 / 13 | 0 / -13.3 / 4.3 | 12 | 166 | none | Middle bay row |
| 6 | Internal Bay | `0x1120` | 0 / -40 / -13 | 0 / -13.3 / -4.3 | 12 | 166 | none | Aft bay row |

The three bay rows are one-sided (no pair flag), on the centreline, 8.7 ft apart.
Weight class 166 is 16,600 lb per row (12 AGM-84E at 1,380 lb is 16,560 lb), so
the retail bays hold up to 49,800 lb across the three rows. Flag `0x100` admits
bomb-flag stores, rows 5 and 6 also carry `0x20` (active-radar missiles). The
retail catalog default is one `AGM84E.JT` store per default row (row 3 only).
There are **no external rows**: no wing, centreline or tank row exists.

### B-52 bay and station survey (for John)

**1. Shape measurement (2/3 ft per unit).**

- *Bay.* The neutral belly is closed: the underside is plain skin (`22dc`,
  `326b`, x 5 to 7, y -46 to 88, z -9 to -8), no recess, no door seam and no bay
  interior. The only bay evidence is word `7a20`: two plates at x = +/-7 (9.3 ft
  apart; the fuselage is x +/-8), y -13 to 26 (26.0 ft long, from 8.7 ft aft to
  17.3 ft forward of the reference point), hanging 5 units (3.3 ft) below the
  belly edge. Real bay: 28 ft long, 6 ft wide (aircraftinformation.info). The
  drawn opening is 93% of the real length. The three record bay rows sit at 13,
  4.3 and -4.3 ft forward = shape y 19.5, 6.5 and -6.5: all **inside the drawn
  opening, centred on it**, so the rows are fitted to this shape's bay. Their
  height (-13.3 ft) is not: the belly line is at -5.7 ft, the gear contact at
  -9.3 ft, so the retail rows hang 7.6 ft below the belly and 4.0 ft below the
  ground contact (the same record convention that the F-22 bay rows had before
  the fitted magazines of [the F-22 stores](../spec/stores-on-pylons.md)).
- *Underwing pylons.* The shape has **no pylon geometry** between the fuselage and
  the inboard pods (the only pylon-like plates are the engine struts at x 53 and
  87 and the tank pylon at x 115). The free underwing span runs from the fuselage
  side (x 8, 5.3 ft) to the inboard pod (x 45, 30 ft): 37 units = 24.7 ft. At
  x = 26 (17.3 ft out, the middle of the gap) the wing underside is at z 5.7
  (3.8 ft above the reference point) and the chord runs y -8 to 39 (31 ft; middle
  y 15.5 = 10.3 ft forward). The inboard pod's lower edge is z -6 (-4 ft), so a
  store axis 1 ft below the wing underside is level with the pod crown. The two
  underwing tanks the real aircraft carries are **drawn in the shape** at
  x = +/-115 (76.7 ft out), always present.

**2. Real loads from public sources** (aircraftinformation.info, Boeing B-52H
page, and Wikipedia "Boeing B-52 Stratofortress" Armament, both accessed
2026-11-15; Wikipedia "Operation Arc Light" for the Vietnam-era figure):

| Place | Load | Source |
| --- | --- | --- |
| Internal bay | 28 ft x 6 ft; three interconnected door sections each side | aircraftinformation.info |
| Internal, conventional | 27 each of Mk 82 or Snakeye, M117 (750 lb), CBU-52/58/71, Mk 36 or Mk 59/62 mines (three nine-bomb clips, or two four-bomb clips); 18 MJU-1B or M129 leaflet dispensers; 12 Mk 52 2,000 lb mines; 6 CBU-87 or CBU-89; or a clip of 8 Mk 84, Mk 41, Mk 55, Mk 56, Mk 60, Mk 64/65 or AGM-86C | aircraftinformation.info |
| Internal, nuclear | Two B53, or the rotary launcher (CSRL, about 5,000 lb) with 8 AGM-86B/C, 4 B28, or 8 B61/B83 (82 B-52Hs had it) | aircraftinformation.info; Wikipedia |
| Internal, 1760 upgrade | 8 JDAM 2,000 lb, or 24 GBU-38 (500 lb) or 20 GBU-31 (first phase) | Wikipedia "Upgrades" |
| Underwing pylons | One pylon per wing "between the inboard engines and the fuselage"; a long pylon (Hound Dog, AGM-86) and a shorter stub pylon (conventional, with a heavy stores adapter beam) | aircraftinformation.info |
| Pylons, ALCM | 12 AGM-86B/C or AGM-129 (6 per pylon); with the rotary launcher 20 in all | Wikipedia; aircraftinformation.info |
| Pylons, conventional | 12 weapons per stub pylon; heavy stores adapter beam up to 9; table: Mk 82, M117, CBU-52/58/71/89 each 18 (CEM B-52H) or 24 (non-CEM), CBU-87 22 (CEM) or 18, GBU-15 10 (CEM); Harpoon 8 to 12 | aircraftinformation.info |
| Totals | 51 Mk 82 (27 bay + 24 pylon) on the Vietnam B-52F; 12 AGM-84 Harpoon on 30 B-52Gs; about 70,000 lb of mixed ordnance, 62,660 lb for a full AGM-86B load | Wikipedia ("Operation Arc Light", "Armament") |
| Drop tanks | Two 2,650 L (700 US gal) underwing tanks | Jane's, `B52.INF` |

The retail media have **no** AGM-86, AGM-69, AGM-142, JSOW, JASSM or JDAM record
(sweeps of the archives), so the rotary launcher and every cruise-missile load
cannot be offered without a new weapon record. The roster does hold Mk 82
(`MK82.JT`, `MK82AIR.JT`, `MK82P.JT`, `MK82HD.JT`), `MK84.JT`, `CBU87.JT`,
`CBU89.JT`, the GBU family and `AGM84E.JT`; the existing ordnance masks decide
which each row admits.

**3. Proposed authored stations (standing decision 2; not decided; each labelled).**
All distances in feet (record units are three times these).

| Station | Position right / up / fwd | Capacity | Label |
| --- | --- | --- | --- |
| **Bay rows 3, 5, 6** (the three retail rows) | x 0; fwd +13.0 / +4.3 / -4.3 (retail, unchanged); up: store bottoms rest at the belly line (-5.7 ft), axis at floor plus half the larger of fin span and diameter plus 0.08 ft (the F-22 magazine rule), instead of the retail -13.3 | Retail 12 per row, class 166 (16,600 lb); proposed real-bay caps below | Fore/aft **fitted** (retail rows centred in the measured opening); height **fitted** (belly line measured); drawn inside, no pylon |
| Bay capacity caps | Retail is 36 slots (3 x 12) and 49,800 lb, against a real bay of 27 bombs or 8 heavy stores (27 Mk 82 = 13,500 lb; 8 Mk 84 = 16,000 lb) | 9 stores per row (27 in all) and class 70 (7,000 lb per row, 21,000 lb internal): 9 Mk 82 or 9 M117 (6,750 lb) or 3 Mk 84 per row (9 in all, one over the real 8) | **Opinionated** (public loads cited above) |
| **Underwing pylon pair** (new row, pair flag; flags per the roster rules) | +/-17.3 (x = 26 units), up +1.0 (store axis 1 ft below the wing underside, TORE's standard 1 ft pylon depth), fwd +10.3 (mid chord at that station) | 18 stores (9 per pylon, the heavy stores adapter beam limit) at class 200 (20,000 lb) per row; the conventional-table row of 18 (CEM B-52H) | Lateral and fore/aft **fitted** (measured free span and chord); capacity and class **opinionated** |
| Bay doors | open pose = the retail plates, or (fitted) the belly cut and door panels as the F-22 | n/a | Opinionated if cut |
| Drop tanks | stay drawn airframe at +/-76.7 ft; no station | n/a | Fitted (shape draws them) |

Not proposed (no source or no weapon record): ALCM and CSRL loads, a wingtip or
centreline row, pods (there is no pod station on the real aircraft, B7).

**4. Options for John (none chosen).**

1. **Retail rows only** (rows 3, 5, 6 as decoded; no pylons). Lowest risk; the
   stores hang 7.6 ft below the belly (and under the ground contact line, which
   makes them unusable as drawn).
2. **Fitted bay rows (recommended by the evidence) and no pylons**: the three rows
   lifted into the belly as above.
3. **Fitted bay rows plus the authored underwing pair** (the full standing
   decision 2 reading: "an internal bay and external wing pylons").
4. Option 3 with the real-bay caps (27 bombs, 8 heavy) set as opinionated.
5. The default load: the retail default is 12 AGM-84E in the forward bay row, a
   Harpoon load the real aircraft carries on its wing pylons (30 B-52Gs, Wikipedia);
   keep it in the bay, move it to the pylon pair (with option 3) or replace it
   with a Mk 82 default (27 bombs). John chooses.

### B-52 roster note

`B52.PT` is the first aircraft of bomber batch A (before `B2.PT` and `F117.PT`);
identity and checkpoint code are assigned by BMB-A1. `AircraftId` stays
append-only in batch order (from 47 on).

### B-52 unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| Record template (weights, fuel, thrust, G) | Flies as authored, not the real H | John, options above |
| Variant (H or G) | Gun, power plant, naming | John |
| Draw scale (1/3 against header 2/3 ft per unit) | Half size if no table entry | BMB-A1 or B52-2 adds `B52 => 2/3` |
| Bay and pylon stations | Retail bay rows are 7.6 ft below the belly; no pylons | John, survey options |
| Roll surface | None in the source | John, options above |
| Crosswind gear and outrigger rig | Not in the source | John, options above |
| Gun mount 19 ft ahead of the tail cone; 2,000 rounds against 1,242 | Position and count | John |
| HUD and cockpit | AC-130 family, HAB sight | John, three options |
| Real pylon wing station and belly clearance | Only the span between fuselage and inboard pod is cited; no station or clearance was found | A dimensioned drawing, if John wants the exact figure |
| Streamer record | Decoded points are noise | Fit from the drawn tip (139, -48 to -69) |

## B-2 source review (2026-11-15)

Research mode, slice B2-0 (bomber batch A). This reviews `B2.PT` (B-2 Spirit)
and its dependency closure before `AircraftId::B2` exists. `B2.PT` is not a
parked target and has no `parked_aircraft.rs` entry. No original module was
executed. PT, SEE, ECM and JT are read as bounded BRF data with the shared
field order in `aircraft_schema.rs`; SH and HUD are inspected as data. Slice
done by Claude Sonnet (sonnet-medium). Nothing here changes code or behaviour.
Real-aircraft figures are cited and never establish game behaviour.

Questions open for John, **not decided here**: the display name, the record
values (template copies and the game's own encyclopedia figures), the HUD, the
sensor rows, the real-size scale, the surface rig, the bay layout and counts,
and the stealth model. They are listed under "Questions for John" in
`.local/m3-aircraft/PLAN-B2.md`.

### B-2 build and extraction

`FA_1.LIB` and `FA_2.LIB` are the same pair as the MiG-31 review (see the
[MiG-31 build section](#build-and-extraction)). The closure was extracted
locally to `.local/m3-imports/scratch/b2` with `tore-extract --include 'B2*'
--include '_B2*' --include '~B2*'` plus the named sensor, jammer, ordnance and
AI files, and the sibling bomber records (`B52*`, `F117*`, `F22*`) for
comparison; every `*.PT` was extracted once to compare records. The
`SETUP.ESA` installer copy of `FA_2.LIB` holds a byte-identical `B2.PT` and
`B2.SH`; only `B2.HUD` differs (4 bytes, a build difference, as with the
Tornado's `E2000.HUD`). The usual five unrelated archive errors appear.

`shape_inspect` refuses `B2.SH` as built: the shape's state guards use two
conditional-jump opcodes the reader does not know (`0x7c` jump-if-less and
`0x7f` jump-if-greater on the drag-rudder word, below), and it stops with
`unsupported shape state guard`. For this review only, a private copy of
`tore-formats` under `.local/m3-imports/scratch/b2/tf` was given those two
cases (value less than, or greater than, the immediate) and its
`shape_inspect` example was run on the shape. **The repository reader is
unchanged and still rejects `B2.SH`; B2-2 must add the two branches** (a
reader change, so a finding, not done here).

The `~B20.M` to `~B29.M` files that match `~B2*` are Baltics campaign missions
("FIRST LIGHT"), unrelated to the B-2, as the `~Q*STOR.M` files were for the
Tornado.

### B-2 identity and record

| Field | Decoded value |
| --- | --- |
| PT | `B2.PT`, 8,970 bytes, SHA-256 `145d83a848f5f39c241c554c0a802a7b5a39f10591e7f932a65c5505f7300770` |
| Displayed names | `B-2`, `B- 2 Spirit` (stray space), `B2.PT` |
| Schema | OBJECT + NPC + PLANE, 219 statements; `structType` 5; `typeSize` 660; `instanceSize` 626 |
| Object class / year / hit points | `0x4000` / 1998 / 241; `expType` 30; `flags` `0x1806bf3` |
| PLANE flags | `0x15` (as the B-52 and Tornado) |
| Shape | `b2.SH` (25,088 bytes, header exponent 8 so scale 1, CODE 21,668), shadow `b2_s.SH` (5 faces) |
| Damage looks | `B2_A.SH` 248 faces, `B2_B.SH` 114, `B2_C.SH` 254, `B2_D.SH` 74 |
| Skins | `_B2.PIC` 57,524 bytes (the shape's only texture), `_B2_A/B/C/D.PIC` |
| Sounds | `&JET7f.11K` loop and second sound, `&powerup.5k`, `&powerdn.5k` (all present in `FA_2.LIB`) |
| HUD | empty: `hudName` is a null dword (no block, no string) |
| AI script | `b.BI` (4,608 bytes, present; the B-52 names the same file, the F-117 names `f117.BI`) |
| Sensors / jammer | `VIS340.SEE`, `F18R.SEE`, `F18.ECM`, `PAVESPK.SEE`, `AV8IR.SEE` |
| Stores | `MK82.JT` x2 rows, `PAVEWA3.JT` x2 rows (both in the TORE ordnance roster) |
| PTS | `B2.PTS` exists (4,608 bytes, inert) |
| Encyclopedia | `B2.INF` and eleven `B2_*.PIC` in `disc2/FA_3.LIB` (Jane's text, quoted below as reference media) |
| Cockpit art | `~B2.PIC` 60,270, `~B2H.PIC` 319,446, `~B2S.PIC` 492,138, `~B2_P.PIC` 6,864 in `FA_1.LIB` |
| Engines / thrust | 4 engines, thrust 114,000 lbf total, afterburner thrust 0, afterburner fuel 0 |
| Weights | empty 100,000 lb, internal fuel 180,000 lb, max takeoff 400,000 lb |
| Ceiling / G | 49,000 ft; envelope rows -1 to +3 (`envMin` -1, `envMax` 3, `negGLimit` 0) |
| Signatures | `sigs` 220, 220, 15 (IR), 1 (radar), 0 |
| Checkpoint code / CLI key | none yet, reserved for BMB-A1 |

The decoded record equals the catalog row in [`fa-aircraft.csv`](fa-aircraft.csv)
field for field (including the empty HUD column). Hashes of the files later
slices depend on:

| Resource | Bytes | SHA-256 |
| --- | ---: | --- |
| `B2.SH` | 25,088 | `b81457f37ab578c4bc862b3a10a63c0afc15ce34ad21adf30399a383e7b01391` |
| `B2_S.SH` | 4,608 | `b8bfe0c34175f8730bf0773561a23d1c4281d9ab109d66eb537f6ce47329890e` |
| `B2_A.SH` | 16,896 | `93672d4e37ec8e719c8efc841070a8d22d107d82e16f4120a1a030556d0497a3` |
| `B2_B.SH` | 8,704 | `3b71e8cef2a8bebe0ae854c894cbaab7d94b88662cf92987147a1342821880f2` |
| `B2_C.SH` | 20,992 | `d971a215ca8ba1fc8b192927b41755b0051a37c240caab265c607caf524a97d5` |
| `B2_D.SH` | 8,704 | `5182ca4ff0181e8013f69e081b0132f6d20bf88e2ee892dea83534c140c3c104` |
| `B2.HUD` | 4,608 | `b2bf418785cff228b92b6aaee244333abf209f11e9cd1c5acd1287d03de35dee` |
| `B.BI` | 4,608 | `781f27cddf73075f36659fe0910dda009a457cdd3f2386de6c9b8bb78946e7ae` |
| `_B2.PIC` | 57,524 | `6f3ff4f0fb9d765b84eee93b7ee2db16671c6ac62a4ad885171af12bde076292` |
| `~B2H.PIC` | 319,446 | `c085e47594704af8ded512ae12ab4600b26f2d4d265d3a9d6f6e60506891a509` |
| `F18.ECM` | 366 | `0fb851d57d7f88f51a017d9663101b52c681c6a6d5ac55823588f25a942d31a9` |
| `MK82.JT` | 2,703 | `dc548de6c7bb72bed20919b2452d4e4666bc426ea0e1aeca412e4a875a380fe6` |
| `PAVEWA3.JT` | 2,735 | `1bd851487c84f6af4a40a92da279cbd63dc75f49d7297ee1b163ded4454b3537` |

`VIS340.SEE`, `F18R.SEE`, `PAVESPK.SEE` and `AV8IR.SEE` are the shared
records already hashed in the F-15E and Gripen baselines (`AV8IR.SEE`
`d4791bcb...`, `VIS340.SEE` `445c35d1...`; `F18R.SEE` is
`b0086101...`, `PAVESPK.SEE` `7fd9c6d3...`).

The display name's stray space is retail: `B1.PT` has the same pattern
("B- 1B Lancer"); the B-52 and F-117 names have none.

### B-2: the template copy list (for John)

`B2.PT` is **the `B1.PT` (B-1B) record with 48 numeric statements changed**:
160 of 208 non-pointer statements are identical to the B-1B's (the next
closest records share 154 or fewer: `C5.PT` 154, `F117.PT` 153). Everything
the two share is the bomber flight tuning and housekeeping:
`sigs[0]` and `sigs[1]` 220, climb and dive limits, camera and doppler words,
`_bv` linear velocity block, `_brv` y and z axes, `gpullAOA` 20,
`lowAOASpeed`/`lowAOAPitch`, rudder yaw/slip/drag/bank, stall (512, 512, 100),
spin entry/exit and yaw/AOA/bank, crash speeds, `coefDrag` 256, `gearDrag` 23,
`flapsDrag` 38, `flapsLift` 51, `airBrakesDrag` 256, `wheelBrakesDrag` 12,
loaded-handling words, all 45 `systemDamage` bytes, `engines` 4 and
`fuelConsumption` 6. The **five sensor rows are the B-1B's too** (`VIS340.SEE`,
`F18R.SEE`, `F18.ECM`, `PAVESPK.SEE`, `AV8IR.SEE`, flags `0x1008`; the F-117
carries the same four sensors), and the bay rows keep the B-1B's pattern
(four-store `MK82AIR` bay rows at flags `0x1500` there). The 48 statements the
B-2 re-authors, against the B-1B:

| Field | `B2.PT` | `B1.PT` |
| --- | ---: | ---: |
| `year` | 1998 | 1985 |
| `typeSize` / `instanceSize` / `flags` | 660 / 626 / `$1806bf3` | 636 / 609 / `$1002bf3` |
| `maxVisDist` | 912 | 683 |
| `sigs[2]` IR / `sigs[3]` radar | 15 / 1 | 60 / 40 |
| `hitPoints` | 241 | 250 |
| Empty weight (`weight`), lb | 100,000 | 192,000 |
| `maxAlt`, ft | 49,000 | 55,000 |
| `thrust` / `aftThrust`, lbf | 114,000 / 0 | 58,400 / 140,000 |
| `internalFuel`, lb | 180,000 | 105,000 |
| `maxTakeoffWeight`, lb | 400,000 | 447,000 |
| `numHards` | 9 | 8 |
| `envMin` / `envMax` | -1 / 3 | -2 / 5 |
| `structure[0]` / `[1]` | 752 / 841 | 1,136 / 1,427 |
| `turbulencePercent` | 10 | 90 |
| `_brv.x`, `puffRot` ranges and rates | 45/31/63 | 60/51/102 |
| `throttleAcc` / `throttleDacc` | 40 / 60 | 60 / 50 |
| `bayDrag` | 25 | 12 |
| `structureLimit` | 5,120 | 7,680 |
| `stallPitchDown`, `_bankRate`, debris words, `aftFuelConsumption` | 30, 8190, own | 25, 10920, own |

Real figures against the record, with the game's own Jane's text (`B2.INF`,
reference media) and public sources (Air & Space Forces Magazine B-2 page,
aerospaceweb.org B-2 data page, a Whiteman AFB fact sheet as summarised by a
search, all read 2026-11-15; sources disagree where noted):

| Field | Record | Real | Difference | Verdict |
| --- | ---: | ---: | ---: | --- |
| Empty weight, lb | 100,000 | `B2.INF` 100,000 to 110,000; public sources about 158,000 to 160,000 (aerospaceweb 150,000 to 160,000) | -37% vs 158,000 | **Not a template copy**: it is the low end of the game's own Jane's range. The game's number and the public number disagree |
| Internal fuel, lb | 180,000 | `B2.INF` 180,000 to 200,000; Whiteman 167,000; aerospaceweb 200,000 | +7.8% vs 167,000 | Low end of the Jane's range (game's own figure) |
| Max takeoff, lb | 400,000 | 376,000 (`B2.INF`, aerospaceweb); normal 336,500 (`B2.INF`, Air Force) | +6.4% vs 376,000 | Not in the media: B-1B's is 447,000; probably authored |
| Thrust, lbf (total) | 114,000 | four F118-GE-100 at 17,300 = 69,200 (`B2.INF`, Air & Space Forces); Wikipedia says 19,000 each | +64.7% vs 69,200 | Own value, matches no known source and no template (per engine 28,500) |
| Afterburner | 0 | none | | Correct |
| Engines | 4 | 4 | | Correct |
| G limit | -1 / +3 | not found in the sources read | | Own envelope, five rows |
| Ceiling, ft | 49,000 | 50,000 (`B2.INF`, Air Force) | -2.0% | Close |
| Hit points | 241 | none | | Own |
| Year | 1998 | in service 1993 to 1997 | | Not examined |

**Copied from the B-1B:** the whole flight tuning block and the five sensor
rows (160 statements, listed above), not the weights, thrust, fuel, ceiling,
stations, HUD or shape. **Weights, fuel and ceiling track the game's own Jane's
text**, so the empty and fuel numbers are retail-encyclopedia values rather
than errors against the game's media; they are low against public figures.

Options for John (none chosen; the Mirage 2000 and Tornado precedent is that
thrust, weight and G corrections are his call):

1. Keep every value as retail (document only, audit pins).
2. Correct thrust to the real four-engine figure (69,200 lbf total) and leave
   weights, fuel and ceiling.
3. Also correct weights to the public figures (empty about 158,000, normal 336,500,
   max 376,000, fuel 167,000). The game's own `B2.INF` supports the max takeoff
   (376,000) and the fuel range (180,000 to 200,000), but its empty range
   (100,000 to 110,000) is below the public figure.

Agent note: with no afterburner and 114,000 lbf the record gives a thrust to
weight at empty of 1.14 and the climb of a fighter; the corrected 69,200 lbf
gives 0.69. A default bay load is far below the 400,000 lb maximum, so the
maximum weight is never the binding number.

### B-2 G envelope rows

Points are speed in ft/s at altitude in ft. Five rows, G -1 to +3 (the first
word of each row is the G, the second its point count, then the speeds).

| G | Points |
| ---: | --- |
| -1 | 260@0, 300@16,000, 445@35,000, 590@43,000, 700@41,000, 780@34,000, 720@0 |
| 0 | 170@0, 200@15,500, 290@34,250, 420@46,625, 550@52,000, 720@51,500, 855@45,750, 895@36,000, 790@0 |
| 1 | 220@0, 250@15,000, 350@33,500, 460@44,750, 560@49,000, 700@48,000, 800@43,000, 840@36,000, 750@0 |
| 2 | same as -1 |
| 3 | 300@0, 350@16,000, 490@31,500, 610@38,000, 705@35,500, 750@31,000, 720@0 |

The 1 G row tops out at 840 ft/s (498 kt) at 36,000 ft and 750 ft/s (444 kt) at
sea level, the catalog's top and sea-level speeds; the real aircraft is high
subsonic (aerospaceweb 530 mph at 40,000 ft, Mach 0.8; Jane's approach speed 140 kt,
the record's second speed word is 130). The 1 G row differs from the B-1B's
(whose -1 row shares its first four points), so the envelope is authored for
the B-2.

### B-2 dependency closure: shape, HUD, AI, sensors, ECM, stores

| Resource | Role | Note |
| --- | --- | --- |
| `B2.SH` | Shape | 268 runtime faces, 0 lines, 0 billboards, one texture `_B2.PIC`, three state words (`6486`, `648c`, `6498`); **no valid streamer record** (the parse gives a zero pivot and meaningless points: treat as absent, so no wingtip vapor point) |
| `B2_S.SH` | Shadow | 5 faces |
| `B2_A.SH` / `B2_C.SH` | Damage bodies | 248 / 254 faces |
| `B2_B.SH` / `B2_D.SH` | Damage pieces | 114 / 74 faces |
| `B.BI` | AI script | present, shared with the B-52 |
| `VIS340.SEE`, `F18R.SEE`, `F18.ECM`, `PAVESPK.SEE`, `AV8IR.SEE` | Sensors, jammer | the B-1B's rows; see below |
| `MK82.JT`, `PAVEWA3.JT` | Bay stores | both already in the TORE roster (`loadout.rs`, `missiles.rs`; `MK82.SH` real size 7.3 ft long, `store_sizes.rs`) |

**The record's HUD field is empty, and a B-2 HUD exists anyway.** `hudName` is a
null pointer (the F-117 and X-31 records are the same). The media holds
`B2.HUD` (4,608 bytes, panel names `~b2`, `~b2h`, `~b2s`, `~b2_p`, `~b2_w`,
indicator names `GEAR`, `FLAP`, `BRAKE`, `BAY`), and the cockpit pictures
`~B2.PIC`, `~B2H.PIC`, `~B2S.PIC` and `~B2_P.PIC` exist. The B-52 names
`bmbroh.HUD`; `AURORA.PT` and `TU160.PT` both name `b2.HUD`. What TORE does
with an empty HUD field: it never reads the record's `hudName`.
`AircraftId::hud()` (`crates/tore-formats/src/aircraft.rs`, the match near line
488) is a per-identity table, and `crates/tore-app/src/assets.rs` (line 152) puts
`id.hud()` in the dependency closure; the parser accepts the null pointer as an
existing nullable pointer grammar (`docs/formats/aircraft.md`: "F31's null
hudName is accepted ... its player HUD resource is F31.HUD"; "SU35.PT has a null
HUD pointer; SU35.HUD is an explicit player-presentation choice"). So the empty
field blocks nothing; a new identity needs an explicit arm, and nothing selects a
default.

Options (none chosen):

1. **`B2.HUD`, cockpit stem `B2`, `~B2H.PIC`** (the retail B-2's own files;
   the same explicit-choice precedent as the X-31 and Su-35). Needs the HUD
   panel and one stem family; not an alias of any other record.
2. Another reviewed bomber family (`bmbroh.HUD` of the B-52): opinionated.
3. No HUD (the F-5E/Tornado glass-mask outcome until the mask exists).

The sensors: `F18R.SEE` is the F/A-18's air-to-air radar, not the B-2's
Hughes AN/APQ-181 LPI J-band strike radar (`B2.INF`); `F18.ECM` is the F/A-18
jammer (the real aircraft carries the ZSR-63 defensive aids equipment and AN/APR-50
RWR, `B2.INF`); `PAVESPK.SEE` and `AV8IR.SEE` are fixed internal rows at (0, 0, 0)
with flag `0x1008`, the same rows the B-1B and F-117 records carry. **The real
B-2 carries no external pods (B7)**, so these rows are bomber-template rows, not
stations: the Tornado precedent (keep as retail, no pod body, nothing on a
station) applies. Options: keep as retail; drop the Pave Spike and Harrier IR
rows for TORE; keep them presented as the targeting sensor.

### B-2 shape state words (rule 11 fold table)

`B2.SH` has three state words, each a different kind. Counts from
`Shape::with_state` in the private patched reader (neutral 268 faces;
coordinates raw shape units, x right, y forward, z up; the object scale is 1, so
one unit is 1/3 ft, see "real size").

| Word | Values | Faces | Geometry | Reading | Status |
| --- | --- | --- | --- | --- | --- |
| `6486` | 1 | +8 | Four vertical plates (two windings each) at x -27, -3 (left) and 1, 26 (right), y -10 to 48, z -12 down to -25 | Bay doors open: the bay side walls hang below the belly | Slot meaning by position (bay), the word text `BAY` is in `B2.HUD` |
| `648c` | 1 | +14 | Seven plates, two windings each: nose (x -6 to 7, y 97, z -14 to -5; y 71, z -24 to -9; x 0, y 88 to 96, z -30 to -6) and mains (x -71, -65, 62, 69; y -7 to 41; z -33 to -12) | Gear down | Confirmed (low point -33, `GEAR`) |
| `6498` | +1 / -1 | +6 / -2 each | Right (+1): closed pair `465f`, `4679` replaced by six faces (below); left (-1): mirror. Value 0 neutral; **value 2 or more removes the right pair and adds nothing** | Split drag rudder, signed: +1 opens the right one, -1 the left | Confirmed by geometry |

There is **no flame word, no airbrake word, no flap word, no slat word, no
elevon word, no hook word, no canopy word and no reverser word** (the B-2 has no
afterburner; the control surfaces below carry no word). Words `6486` and `648c`
set to 2 change nothing; `6498` is a three-position word, not a flag, which is
the first signed surface word in the bombers (the tail words of the Tornado and
the Mirage are positive only).

**Rule 11 face table** (every face of every moved or word-added surface; the
shared-vertex test is exact coordinates against the neutral faces):

| Surface | Faces | Vertices each | Moved | On hinge/spindle | Fixed off axis |
| --- | --- | ---: | ---: | ---: | ---: |
| Bay plates (`6486`) | `4209 4220 4264 427b 431a 4331 42bf 42d6` | 4 | 4 (the lower edge) | 0 shared, the upper edge at z -12 free | 0 |
| Gear plates (`648c`) | `41ae 41c5 4153 416a 4597 45b6 438d 43a4 449e 44b5 452c 454b 441b 443a` | 4 | all plates rigid | none shared with the skin | 0 |
| Right drag rudder, upper plate | `47db`, `47f5` | 4 | 2 (the free edge) | 2 `(177,-46,0)`, `(215,-71,0)` | 0 |
| Right drag rudder, lower plate | `47a7`, `47c1` | 4 | 2 | 2 `(177,-46,-7)`, `(215,-71,-7)` | 0 |
| Right drag rudder, front plate | `480f` | 4 | 0 | 4 (both hinge lines) | 0 |
| Right drag rudder, inboard cap | `4829` | 3 | 1 `(167,-53,-3)` | 2 on the hinge ends | 0 |
| Left drag rudder | `46d0`/`46ea` upper, `4704`/`471e` lower, `4738` front, `4752` cap | 4, 4, 4, 3 | as the right (mirror, 1 unit off in places) | as the right | 0 |
| Elevon strip, right (neutral, no word) | `4037`, `404e` quads; `4065`, `407a` triangles | 4, 4, 3, 3 | whole body turns | none shared | 0 |
| Elevon strip, left | `40ce`, `40e5`; `40fc`, `4111` | 4, 4, 3, 3 | whole body turns | none shared | 0 |

Verdict: no face folds. Every word plate is a whole four-vertex (or one
three-vertex) face and shares no vertex with any other face except the drag
rudders' hinge lines; every face lies wholly on one side of its hinge.

### B-2 surfaces (for John)

The flying wing has a **double-W trailing edge**; the shape draws two surface
pairs on it. Planform (top view, red elevon strips, blue drag rudders, green
bay panels) was drawn from the faces during this review (scratch only).

- **Elevons: one strip per side, x 63 to 130 (right), -64 to -131 (left).** Each
  is a thin wedge of four whole faces: a top quad at z 0 (hinge side) to z -3,
  a bottom quad at z -6 to z -3, and a small triangle pair at the inboard end
  (`4065`/`407a`). The leading edge runs (63,-70) to (120,-22), 74 units (24.8
  ft) on the diagonal; the trailing edge (69,-80) to (130,-29); the chord is
  about 12 units (4 ft). The strips share **no vertex** with any other face
  (checked by exact coordinates) and nothing lies in front of them at their own
  place: the wedge is the wing skin there. They are neutral geometry: **no state
  word moves them**, so any elevon is a **fitted rig**. Real: "double-W trailing
  edge incorporating elevons and drag rudders outboard of engines" (`B2.INF`);
  the real inboard and outboard elevon pairs are one strip per side in the mesh.
  Rule 12 (delta elevons): both would take pitch plus differential roll; the
  source has one strip per side, so one term per side. Rule 11: the four faces
  turn as one rigid body about the leading edge line (the wedge's z -3 centre
  line), no vertex is fixed off axis; the seam is a hinge-line gap of at most
  3 units times the sine of the deflection against the fixed skin ahead (to be
  measured in B2-3). The whole-surface gate (no diagonal or partial surface, whole
  faces only) is met: every candidate is a complete face.
- **Drag rudders (split rudders) at x 167 to 215 (right) and -169 to -218 (left)**,
  one word, signed. Closed: a wedge of two faces (top quad from z 0 at the leading
  hinge edge down to z -3 at the free edge, bottom quad from z -7 up to z -3).
  Open: the two faces are removed and **six are added**: an upper plate hinged at
  z 0 whose free edge reaches z +7, a lower plate hinged at z -7 whose free edge
  reaches z -14 (each in both windings), a vertical front plate between the
  hinge lines (`480f`, left `4738`) and an inboard triangle cap (`4829`, left
  `4752`). In angles of the chord (12.2 units in plan) that is the upper plate
  from -13.8 to +29.9 degrees and the lower plate from +18.2 to -29.9 degrees:
  about 44 and 48 degrees of swing, 21 units (7 ft) between the free edges. The
  open pose is not an exact rotation of the closed one (free-edge corners move
  1 to 2 units in plan, as the Mirage 2000 rudder swap). It is **a single open
  pose per side, no proportional travel** in the source: a rig that opens
  proportionally is fitted. The sign convention against rudder input (+1 is the
  right) must be confirmed in B2-3. Real: the drag rudders yaw by opening
  one side (the B-2's split rudders also act together as airbrakes), which the
  source cannot show because the word opens only one side at a time.
- **Gust and trailing-edge centre**: the centre beaver tail (x about -60 to 60)
  and the outboard tip pieces are fixed skin; no surface is in the source.
- **No slats, flaps, airbrakes, spoilers or reversers.** The drop-down spoiler
  panels ahead of the bay doors in `B2.INF` do not exist in the mesh (the open
  bay plates are the only geometry), and none is invented here.

Options (none chosen): (a) rig the four elevon faces per side as whole panels
with pitch and differential roll, fitted limit, and the drag rudders as the
source's two-state word; (b) leave the elevons fixed and rig only the drag
rudders and gear; (c) also add a fitted both-sides drag-rudder airbrake (not in
the source). The reader change above is required before any of this runs.

### B-2 gear

Plate gear, no wheels or doors, the state words of other bombers: the nose
(three plate pairs at y 71 to 97, z -30 to -5) and four main plates (x -71,
-65 on the left, 62, 69 on the right; y -7 to 41 for the outer ones, -2 to 20 for
the inner; z -33 to -12). Low point **-33** units, 22 units (7.3 ft) below the
belly skin at -11/-12. At 1/3 ft per unit: track about 46.7 ft over the outer
plates (42.3 ft over the inner), wheelbase about 25 ft (nose plate y 92 to the
main plates' centre y 17); the real gear dimensions were not found, so these
are unverified. Real (`B2.INF`): a tricycle gear adapted from the Boeing
757/767, **inward-retracting four-wheel main bogies** with a large trapezoidal
door, a **rearward-retracting** two-wheel nose unit with a small door with
sawtooth edges, and a gear limit speed of 224 kt. The mesh has the gear wells
as flat belly overlays at z -12 (`3ca3` x -72 to -58, `3cc9` x 55 to 70, y -6 to
42), 15 units wide, which sit exactly under the main plates: the closed wells
are skin, as on the Tornado. Gear rig options: fold the mains inward about a
longitudinal axis at the belly and the nose aft, each plate rigid (whole faces,
no door leaves in the source); the plate pairs are drawn in two windings and
none shares a vertex with the skin, so the fold check passes trivially.

### B-2 real size (for John)

Measured on the neutral state (clean, gear up) and the gear word, raw units, header
exponent 8 so the object scale is **1** (one unit is 1/3 ft, the aircraft
default, so no scale-table entry is needed):

| Axis | Units | At 1/3 ft | Real | Verdict |
| --- | ---: | ---: | ---: | --- |
| Span (x -258 to 258) | 516 | 172.0 ft | 172 ft (`B2.INF`, Air Force) | exact |
| Length (y -87 to 107) | 194 | 64.7 ft | 69 ft | -6.3% |
| Height, gear down (z -33 to 24) | 57 | 19.0 ft | 17 ft | +11.8% |
| Height, gear up (z -12 to 24) | 36 | 12.0 ft | | |

The leading edge from the nose (0, 107) to the tip (258, -59) makes 32.8 degrees
with the lateral axis, against Jane's 33 degrees (`B2.INF`: "swept at 33 degrees"),
a check that the mesh is the true planform. The bay rows' x (-15, +14) match the
bay panel centres (-16, +14) to a unit, which also says the record stations were
authored against this size. Options: (1) draw as is (span exact; the aft 4.3 ft
of the real length and 2 ft of the real height are not modelled; the cockpit
hump at z 24 is likely taller than real); (2) a per-axis fit (the Mirage 2000
mechanism) of about y x 1.07 and z x 0.89 (opinionated, and it would shift the
bay length and gear heights by the same factors).

### B-2 bay survey

**Measured in the shape (units, then feet at 1/3 ft per unit).** Both bays are
flat overlay panels one unit below the belly skin; there is no bay interior and no
door leaf in the mesh.

| | Left bay | Right bay |
| --- | --- | --- |
| Panel face | `3c84` | `3c65` |
| Width x | -28 to -4 (24 units, 8.0 ft) | 2 to 26 (24 units, 8.0 ft) |
| Length y | -9 to 49 (58 units, 19.3 ft) | -9 to 49 (58 units, 19.3 ft) |
| Centre x / y | -16 / 20 (-5.3 / 6.7 ft) | 14 / 20 (4.7 / 6.7 ft) |
| Floor (belly skin) z | -11/-12 (-3.7 / -4.0 ft) | -11/-12 |
| Upper skin above the bay, z | 14 to 17 at the bay centre line (4.8 to 5.7 ft), 3 to 4.5 at the outer edge x -27, 18 to 23 at the inner edge x -4 | 14 to 18 at the centre line, 2 to 4 at the outer edge x 25, 17 to 23 at x 2 |
| Depth available | 26 to 29 units (8.7 to 9.7 ft) at the centre, 15 units (5 ft) at the outer edge | same |

Both bays are 8.0 ft wide, 19.3 ft long, and sit centred about 4.7 to 5.3 ft
either side of the axis (the mesh axis is x -1: the left panel and the right
panel mirror about it). The belly is one 7-vertex polygon `396d` (x -89 to 88,
y -28 to 69) with the panels 1 unit below it: **cutting an opening is a
cut of a whole belly polygon**, not a face swap (the F-22 precedent cut its
belly and intake polygons: `docs/spec/stores-on-pylons.md`, "F-22 naval
stores"). The doors in the shape are the `6486` word: four hanging plates 13
units (4.3 ft) deep at the bay's long edges (x -27, -3 and 1, 26), which leave
the closed panels in place (the word removes nothing). Retail "open" therefore
shows walls while the panels stay drawn over the opening; a door rig is
fitted: the panels as door leaves hinged at an edge, the plates as the bay side
walls. The gear wells beside the bays are the orange panels in the plan view
(skin overlays at z -12, no word).

**The record's nine hardpoints** (all in original order; location byte 0
Centerline, 1 Fuselage, 2 Internal Gun, 3 Internal Bay, 4 Wing, 5 Wingtip;
positions are record units right / up / forward, feet are units divided by 3):

| Row | Location | Flags | x / y / z | Feet right / up / fwd | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x1008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `VIS340.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x1008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `F18R.SEE` | Fixed radar (APG-65 class) |
| 2 | Fuselage | `0x1008` | 0 / 0 / -50 | 0 / 0 / -16.7 | 1 | 0 | `F18.ECM` | Fixed jammer |
| 3 | Fuselage | `0x1008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `PAVESPK.SEE` | Fixed pod row |
| 4 | Fuselage | `0x1008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `AV8IR.SEE` | Fixed IR row |
| 5 | Internal Bay | `0x1504` | -15 / -38 / 34 | -5.0 / -12.7 / 11.3 | 32 | 160 | `MK82.JT` | Left bay, forward |
| 6 | Internal Bay | `0x1504` | 14 / -38 / 35 | 4.7 / -12.7 / 11.7 | 32 | 160 | `MK82.JT` | Right bay, forward |
| 7 | Internal Bay | `0x1504` | -15 / -38 / 6 | -5.0 / -12.7 / 2.0 | 4 | 160 | `PAVEWA3.JT` | Left bay, aft |
| 8 | Internal Bay | `0x1504` | 14 / -38 / 7 | 4.7 / -12.7 / 2.3 | 4 | 160 | `PAVEWA3.JT` | Right bay, aft |

Findings: **the nine hardpoints are five sensor rows and four bay rows; the
record already carries the bays** as `Location::InternalBay` (byte 3), with no
external station and no wing row. Their x (-5.0 / +4.7 ft) and forward (11.3 / 2.0
ft) values fall inside the measured bays (centres -5.3 / +4.7 ft, y -3.0 to 16.3
ft), so rows 5 and 6 are the forward Mk 82 stations and rows 7 and 8 the aft
Paveway III stations, one pair per bay. The up value (-12.7 ft) is **8.7 ft below the belly**
(-4.0 ft): as on the F-22's bay rows (-8.3 ft against its belly at -3 ft) it is
not a measured position, so the placement code, not the raw number, must put the
stores. The rows carry the pair flag `0x4` (the F-22's main bays do not), so a
row at x -15 mirrors to +15: rows 5 and 6 would each put stores in both bays.
Record capacity: 32 + 32 Mk 82 and 4 + 4 Paveway III (64 and 8 against the
real 80 and 16 below); the 0x1000 bit is on every B-2, B-1B and F-117 row and is
not interpreted in the reviewed specs.

**Real bay loads (public sources, cited).** Two internal weapons bays, each
"equipped with a rotary launcher and two bomb-rack assemblies" (Air Force
Technology's B-2 page, via search; Wikipedia: "each holds munitions on a
rotary launcher or on two bomb racks"). Air & Space Forces Magazine B-2 page
(accessed 2026-11-15): 16 B61-7, B61-12 or B83 (or eight B61-11) on rotary
launchers; 80 Mk 82 (500 lb) or 80 GBU-38 JDAMs, or 34 CBU-87/89, on rack
assemblies; or 16 GBU-31, 16 Mk 84 (2,000 lb), 16 AGM-154 JSOW, 16 AGM-158
JASSM, or eight GBU-28; "nearly 60,000 lb in various combinations". Maximum
two GBU-57 massive ordnance penetrators, one per bay, at 30,000 lb each
(Wikipedia, 2026-11-15). The game's own Jane's (`B2.INF`): a Boeing rotary
launcher assembly in each bay, "up to eight large stores each", 16 AGM-129,
16 B61 or B83, 80 Mk 82, 16 JDAM, 16 Mk 84, 36 M117, 36 CBU-87/89/97/98 and 80
Mk 36/Mk 62 mines; stores of 1,000 lb and under on four bomb racks (two per
bay); max weapon load 40,000 lb. No public source gave a bay's dimensions
(searched 2026-11-15), so the measured 8.0 x 19.3 ft are the only figures.

**Mechanism limits (the F-22 bay work).** `Location::InternalBay` rows are
never drawn and never counted as external stores. The F-22's three bay rows
become `Location::Magazine` (byte `0x83`) in `placed_on`
(`crates/tore-sim/src/combat/hardpoints.rs`, `AircraftId::F22 | F22n`, rows
5 to 7) at fitted mounts (right, floor up, release origin forward), and the
magazine places its stores **in one straight lane front to back**, each
separated by its length plus 0.25 ft, forward store first; they are drawn
inside, hidden while the doors are closed, add nothing to the external-store
radar signature, and leave from the same points
(`docs/spec/stores-on-pylons.md`, "F-22 naval stores"; `loading.rs` has the
magazine station kind). The door state is `flight.rs` `bay`/`bay_available()`,
which is **true only for `AircraftModel::F22`** today. A single lane holds, in
a 19.3 ft bay, two Mk 82 (2 x 7.3 + 0.25 = 14.85 ft; three need 22.1 ft) or one
14 to 15 ft Paveway III-class store. The record's 32 per row do not fit that
mechanism; a lane of 32 would run 241 ft.

**Proposed authored bay stations (opinionated, for John; geometry fitted to
the measured bays, counts from the source rows).** Reuse the magazine
mechanism, extended:

| Item | Proposal | Label |
| --- | --- | --- |
| Carriers | The four retail rows 5 to 8 become `Location::Magazine` in `placed_on` for the B-2 (rows 5 and 6 Mk 82, rows 7 and 8 Paveway III), no new rows | opinionated (precedent: the F-22 override) |
| Pair flag | Treat each row as a one-bay carrier (clear flag `0x4` in the loaded definition, as `F22N.PT` rows were corrected to location 3 and its flags), so left rows 5 and 7 and right rows 6 and 8 each load one bay | opinionated, a record edit like F22N's |
| Mount points | Left bay x -5.3 ft, right x +4.7 ft, floor -3.7 ft up, release origin 6.7 ft forward (the bay centre), the same for the Mk 82 and Paveway rows of one bay | fitted to measurement |
| Lane layout | Mk 82: 2 tandem x 4 abreast x 4 tiers = 32 per bay (width 4 x 1.27 = 5.1 ft inside 8.0; height 4 x 0.9 = 3.6 ft inside 5.0 at the bay's outer edge; length 14.85 inside 19.3). Paveway III: 1 tandem x 4 abreast = 4 per bay (width about 4 x 1.3 ft) | opinionated; needs a code change, the lane only knows one line |
| Exclusivity | The Mk 82 and Paveway rows of one bay are alternatives (real: one rotary launcher or two racks per bay), as the F-22 wing row 8 is exclusive with its tanks | opinionated |
| Doors | Retail `6486` plates as the open side walls; the two panels as fitted door leaves hinged at the outboard edge, both bays opening and closing with the release (timing as the F-22 main bays); a hole in the belly polygon `396d` | fitted, rig in B2-3 |
| Signature | Bay stores add nothing to the external-store signature (existing rule); the open-bay term needs `bay_available()` true for the B-2 | needs a code change |
| Counts | Option A: keep the record's 64 Mk 82 and 8 Paveway (source). Option B: raise to the real 80 Mk 82 (40 per bay) and 16 large stores (8 per bay, the rotary launcher count) as authored values | A source, B opinionated |
| Load Ordnance rows | Add rows only where the weapons data already allows (GBU-38, Mk 84 and CBU-87 are in the roster; no B61/B83 weapon exists) | opinionated, B2-6 |

Option without a mechanism change: cap the Mk 82 at 2 and the Paveway at 1 per
bay (a visible but tiny load, 4 Mk 82 and 2 Paveway in total against 64 and 8).
Any of these is John's call; the proposal is what fits the measured bay.

### B-2 stealth finding (shared with the F-117 review)

**The sensor and detection code has a signature concept, and the B-2's record
already fills it.** It is the PT `sigs` words, not a stealth flag:

- `crates/tore-sim/src/sensors/signature.rs`: `SignatureProfile::from_source`
  reads `sigs[3]` as the radar signature and `sigs[2]` as the infrared one
  ("a relative game statistic and not square metres"). `effective_radar`
  returns the PT radar value plus external stores (3 per store body and 2 per
  1,000 lb), times an aspect factor (1 nose or tail, 2 side, 4 top or bottom)
  and a configuration factor (gear 0.25, flaps 0.15, bay 0.50 of the deployed
  fraction). A zero radar signature stays zero.
- `crates/tore-sim/src/sensors/detection.rs` (lines 211, 231, 240): the radar
  range is `nominal * sqrt(effective_signature / 100) * look-down * notch *
  jammer`, capped at the nominal range, the jammer burn-through compares the same
  value, and the IR range uses `sigs[2]`. The same function feeds the radar
  missile seekers (`combat/missiles.rs` line 447, `combat/missiles/seeker.rs`
  line 198), the RCS instrument page (`crates/tore-app/src/scope.rs`, `rcs()`)
  and the store signature (`combat/live/store_signature.rs`).
- `docs/radar.md` ("Agent-authored signature model", the store-weight table and
  its closing paragraph) documents it as agent gameplay tuning, with the F-22
  (`sigs[3]` 10) at 28.5 nmi against a 90 nmi radar.

The records: **`B2.PT` `sigs[3]` is 1 and `sigs[2]` is 15**; `F117.PT` is 1 and
20; `F22.PT` is 10 and 60; `B52.PT` is 100 and 200; `B1.PT` is 40 and 60. A
signature of 1 gives 10% of a radar's nominal range nose-on in clear air (a
90 nmi search radar sees it at 9, a 50 nmi track at 5), 20% from above or
below (factor 4) and 14% abeam; with the bay fully open the configuration
factor is 1.5. So **retail already encodes stealth for the B-2 and F-117 as a
radar signature of 1, and the game's detection code uses it: no fitted
per-aircraft value is needed** (standing decision 4's fallback), and no sensor
change is made or implied here.

Limits of the present model, for John's decision (none acted on):

1. **Surface air-defence radars read no signature.** `docs/radar.md` (the
   closing paragraphs of the store-signature section) says "Surface air-defence
   radars detect by their records' range bands and read no signature at all", so
   a B-2 is as visible to a SAM site as an F-15 today. Only air-to-air radar,
   jammer burn-through, radar missiles and the RCS page see the 1.
2. **The bay-open term only works for the F-22.** `flight.rs` `bay_available()`
   is `matches!(model, AircraftModel::F22(_))`, so a B-2's `bay` fraction is
   always 0 until that is extended (a code change; with signature 1 the
   opening bay would raise it to 1.5).
3. Bay stores add nothing and a store on a (nonexistent) external station would
   add 3, which is three times a clean B-2's whole signature: keep every B-2
   store internal.
4. The real value is reported only: about 0.1 square metres (Wikipedia, "reported
   rather than confirmed"); the model's 1 is a relative statistic, so this is
   cited and never mapped.

### B-2 roster note

`B2.PT` is the 2nd aircraft of bomber batch A. `AircraftId` stays append-only
from 47 on in batch order (B-52, B-2, F-117) and is assigned by BMB-A1; the
wire identity is 9 bits, so there is no capacity constraint.

### B-2 unknowns and next steps

- The 32-versus-lane layout and the `bay_available()`, door and belly-polygon
  work are B2-3 and B2-5 questions for John.
- The reader needs the `0x7c` and `0x7f` state-guard branches before B2-2.
- The nose-wheel and main-gear hinge axes and the engine exhaust positions (the
  real exhausts are two V-shaped overwing outlets) were not examined; no nozzle
  or flame word exists, so contrail and engine-position mapping is a B2-4
  question.
- Real main gear track and wheelbase, the real bay dimensions and a G limit
  were not found in public sources.

## F-117 source review (2026-11-15)

Research mode, slice F117-0 (bomber batch A, plan `.local/m3-aircraft/PLAN-F117.md`).
This reviews `F117.PT` (F-117A Nighthawk) and its dependency closure before
`AircraftId::F117` exists. No original module was executed. PT, SEE, ECM and JT
are read as bounded BRF data with the shared field order in
`aircraft_schema.rs`; SH and HUD are inspected as data (`shape_inspect`). Slice
done by Claude Sonnet (sonnet-medium). Nothing here changes code or behaviour.
Real-aircraft figures are cited and never establish game behaviour. Questions
for John are **not decided here**: they are marked "(for John)" and collected in
the plan's "Questions for John".

### F-117 build and extraction

The closure was extracted locally to `.local/m3-imports/scratch/f117` with
`tore-extract --include 'F117*' --include '*F117*' --include 'VIS340*' ...` (and
`'*.PT'` for the diffs). `FA_2.LIB` holds the whole closure; the installer copy
inside `SETUP.ESA` is byte-identical for the PT and shapes. The usual five
unrelated archive errors appear. Unlike the Tornado, the F-117 has a
**cockpit and encyclopedia of its own**: `~F117.PIC`, `~F117H.PIC`,
`~F117S.PIC`, `~F117_P.PIC` and `ICO2F117.PIC` in `FA_1.LIB`, `F117.HUD`,
`F117.PTS`, `F117.AI` and `F117.BI` in `FA_2.LIB`, `F117.INF` and `F117_0..7`,
`_C`, `_E`, `_F`, `_P` `.PIC` in `disc2/FA_3.LIB`, `^ACF117.5K` (the radio
callout) and `~AF117.M` (a mission file).

### F-117 identity and record

| Field | Decoded value |
| --- | --- |
| PT | `F117.PT`, 11,390 bytes (decoded), SHA-256 `a2774215720dede83601695ca44c319500919c4e7d92afb5f043b78abf64608d` |
| Displayed names | `F-117`, `F-117A Nighthawk`, `F117.PT` |
| Schema | OBJECT + NPC + PLANE, 219 statements; `structType` 5; `typeSize` 588; `instanceSize` 575 |
| Object class / year / hit points | `0x4000` (attack) / 1983 / 131; `expType` 30 |
| PLANE flags | `0x11` (base `0x01`, seat `0x10`; no hook, no multi-crew, no no-lift) |
| Shape | `f117.SH` (20,992 bytes; header exponent 8, so the **aircraft convention of 1/3 ft per unit**), shadow `f117_s.SH` (3 faces) |
| Damage looks | `F117_A.SH` 233 faces, `F117_B.SH` 48, `F117_C.SH` 199, `F117_D.SH` 55 |
| Skins | `_F117.PIC` 117,584 bytes, `_F117_A/B/C/D.PIC` 85,864 / 31,264 / 60,904 / 32,044 |
| Sounds | `&JET2N.11K` loop and second sound, `&powerup.5k`, `&powerdn.5k` (all present) |
| HUD | **empty** in the record (`hudName` is `dword 0`); `F117.HUD` exists |
| AI script | `f117.BI` (12,800 bytes); `F117.AI` (18,823 bytes) is also present |
| Sensors / jammer / weapon | `VIS340.SEE`, `PAVESPK.SEE`, `F18.ECM`, `AV8IR.SEE` (fixed rows 0 to 3) and `PAVEWA3.JT` in two bay rows |
| Engines / thrust | 2 / 21,600 lbf military, **afterburner 0** (non-afterburning); fuel flow 2, afterburner flow 0 |
| Weights / fuel / ceiling | empty 29,500 lb, maximum takeoff 54,000, internal fuel 18,500, ceiling 42,000 ft, minimum altitude 300 ft |
| G / speeds | -3 / +6; 1 G row top 604 kt at 36,000 ft; sea level 592 kt (the catalog) |
| Signatures | `sigs` 162 / 162 / **20 (infrared)** / **1 (radar)** / 0; `maxVisDist` 249 |
| Checkpoint code / CLI key | assigned by BMB-A1 |

The decoded record equals the catalog row in [`fa-aircraft.csv`](fa-aircraft.csv)
field for field. Hashes of the files later slices depend on:

| Resource | SHA-256 |
| --- | --- |
| `F117.PT` | `a2774215720dede83601695ca44c319500919c4e7d92afb5f043b78abf64608d` |
| `F117.SH` | `b936e4c57adbce43d5da600a0016a25cd44c7e46d1c0da5a2f1fba0b56c9ebaf` |
| `F117_S.SH` | `23fd10e7afa464137c85880c6491a8cece639db8f7b02e030fe603185c77b11b` |
| `F117_A.SH` | `db985c2930ca458eb481cbe6066f7cbc62fdf90bc75c9e2981868ec5c765715d` |
| `F117_B.SH` | `66d6d75f18a869fe844c9ab869d91b81e2a893b07a13d294f580b79eaa13ab66` |
| `F117_C.SH` | `5bf8b08b9683a1ba0b2781dac543f568789e08cdf6ef73ca11a9432ec7943692` |
| `F117.HUD` | `b2736a93d9785869b7f418d58fffb83657df53a39275a1a7f50386adbdcc9a02` |
| `F117.BI` | `a350e28c55a53a35939172996319e0c6264c2283166004232a10653a9e060fdc` |
| `F117.AI` | `7d2a9845fbde298597cc5cf5691d3ff6422e6dab3b2508de2557ba29c132cf80` |
| `F117.PTS` | `1d06c52216c77e8cfd2704f236eb57d9816f72d0fceb4aea25040778158cf9ee` |
| `VIS340.SEE` | `445c35d1cc778513b0d38b73d36e10b47b7a4f210ffef87f54a53b17e6cd9efe` |
| `PAVESPK.SEE` | `7fd9c6d3e909015e8715056e5c0f13ce472cdb7fb70ac10bdcd4a54cb6e53b91` |
| `AV8IR.SEE` | `d4791bcb3faf93799cb49472757801834c49c9d0566da6ceb59ef40847ad0f60` |
| `F18.ECM` | `0fb851d57d7f88f51a017d9663101b52c681c6a6d5ac55823588f25a942d31a9` |
| `PAVEWA3.JT` | `1bd851487c84f6af4a40a92da279cbd63dc75f49d7297ee1b163ded4454b3537` |

(`PAVESPK.SEE` is byte-identical to the Tornado's.)

### F-117 dependency closure

| Dependency | State | What TORE does |
| --- | --- | --- |
| Shape `f117.SH`, shadow, four damage looks, `_F117*.PIC` | all present | Normal shape path; two state words only (below) |
| AI script `f117.BI` | present, and `F117.AI` beside it | TORE does not run retail scripts. `BehaviorFamily::F117` already exists (`crates/tore-sim/src/ai/controller.rs`, checkpoint value 1); [spec/ai.md](../spec/ai.md) says the F-117 "shares fighter air combat; ground pass requests a 5000 vertical offset and speed operand 300". `strike_class.rs` names the F-117 as the likely first `OVERRIDES` entry (empty today, "retail flies it at 5,000 ft level") |
| Sensors `VIS340.SEE`, `AV8IR.SEE`, `PAVESPK.SEE`, ECM `F18.ECM` | present | Reviewed records (see the sensor section); **no radar row** (no `F18R.SEE`) |
| Store `PAVEWA3.JT` | present | GBU-27 Paveway III, 2,000 lb, shape `gb27.SH`; already in the TORE roster and flown from the F-16C and Su-25 ([air-to-ground baseline](../baselines/air-to-ground.md)) |
| HUD | record field empty | see below |
| Cockpit art `~F117*.PIC`, `F117.HUD` | present | A cockpit family of its own (`F117` stem); needs a measured glass polygon like the batch-5 aircraft |
| `F117.PTS`, `ICO2F117.PIC` | present | PTS is inert (aircraft.md); the icon is a list-screen picture |
| Sounds | present | Shared with other records |

**An empty HUD field.** `hudName` is `dword 0`, not a pointer, so the record
names no HUD. Other aircraft are in the same case (the F/A-18D, AV-8B, B-2, X-29, X-31, Rafale,
Su-35, Yak-141, AC-130 and the ASTOVL records); the catalog's `hud_resource` column is blank for them.
TORE does not read the field to choose a HUD: `Aircraft::hud()`
(`crates/tore-formats/src/aircraft.rs`) is a per-aircraft table (`F18` gives
`F18.HUD`, `Rafale` gives `RAFALE.HUD`), and
[spec/air-to-ground.md](../spec/air-to-ground.md) records that "several PTs carry
an empty HUD name and were matched by file name". For the F-117 the file-name
match is clean: `F117.HUD` exists (4,608 bytes) and names `~f117`, `~f117h`,
`~f117s`, `~f117_p` and `~f117_w` (strings read from the module), exactly the
`~F117*.PIC` set in `FA_1.LIB`. Nothing is a substitute, unlike the Tornado
(Eurofighter HUD). `hud_aperture.rs` has no F-117 arm, so the glass mask fails
closed (empty) until a slice measures `~F117H.PIC` (the Gripen, F-5E and Tornado
precedent). The bomb sight type of `F117.HUD` is not read here.

### F-117 record against the other records

Of the 219 statements the closest records agree on 182 (`A7`), 180 (`EA6`),
178 (`F4E`), 177 (`A4E`, `A6V`): those overlaps are the generic block shared by
144 of the 145 records (stall, spin, AI timers, crash speeds, damage tables), so
no single record is the template. The F-117 record has these values **shared
with no other record**: empty weight, military thrust, internal fuel, hit points,
`structure[0]` and `[1]`, `turbulencePercent` 135, `wheelBrakesDrag` 48,
`sigs[2]` 20, `maxVisDist` 249, the whole ten-row G envelope (no other record
shares even two rows) and the hardpoint block. It is an authored record, not a
reskin. `bayDrag` 25 is shared only with the other bay aircraft (`B2`, `B52`,
`F22`, `TU26`, `TU95`, `TU160`, `AN74`), so the record knows about its bay.

### F-117: the template copy list (for John)

Real figures: Wikipedia "Lockheed F-117 Nighthawk" (accessed 2026-11-15:
length 65 ft 11 in, span 43 ft 4 in, height 12 ft 5 in, empty 29,500 lb,
maximum takeoff 52,500 lb, two F404-F1D2 at 9,040 lbf each non-afterburning,
maximum speed 594 kn / Mach 0.92 with "541 kn at high altitude" in the text,
ceiling 45,000 ft, two internal bays with one hardpoint each, 5,000 lb internal);
MILAVIA F-117 specifications (accessed 2026-11-15: two F404-GE-F1D2 at 10,800 lb
each, 52,200 lb maximum takeoff, Mach 0.98 / 1,040 km/h, ceiling 50,000 ft
estimated, 5,000 lb in the internal bay); aerospaceweb F-117 data (accessed
2026-11-15: +6 G, 646 mph, 52,500 lb).

| Field | Record | Real | Difference | Verdict |
| --- | ---: | ---: | ---: | --- |
| Empty weight, lb | 29,500 | 29,500 | 0 | **Real**, and unique to this record |
| Military thrust, lbf (both) | 21,600 | 18,080 (Wikipedia, 2 x 9,040) to 21,600 (MILAVIA, 2 x 10,800) | +19.5% / 0 | **Agrees with MILAVIA**; unique to this record. Sources disagree on the rating |
| Afterburner thrust | 0 | none (F404-F1D2 has no afterburner) | 0 | Correct (68 records have 0, so it is also the generic value) |
| Engines | 2 | 2 | | Correct; the shape has two exhaust slots |
| Max takeoff, lb | 54,000 | 52,500 (Wikipedia, aerospaceweb), 52,200 (MILAVIA) | +2.9% / +3.4% | **Copy** of `F105.PT`'s 54,000 (the only record sharing it); close to real |
| Internal fuel, lb | 18,500 | not found in any fetched source | unknown | Unique to this record; unverified |
| Ceiling, ft | 42,000 | 45,000 (Wikipedia), 50,000 estimated (MILAVIA) | -6.7% / -16% | **Copy**: the same 42,000 sits on 15 records (the F-4 family, `A6`, `A7`, `F105`, `F5E`...) |
| G limit | -3 / +6 | +6 (aerospaceweb); negative limit not found | 0 | -3 / +6 is also the value of 12 other records (A-7, EA-6B, Su-24...); **+6 matches real** |
| Top speed (1 G row top), kt | 604 at 36,000 ft | Mach 0.92 is 527 kt at 36,000 ft; Wikipedia text 541 kt; MILAVIA Mach 0.98 is 562 kt; Wikipedia infobox 594 kn | **+11.6% over 541, +14.6% over 527; Mach 1.05** | **Too fast at altitude** (the record is supersonic; the real aircraft is subsonic) |
| Sea-level maximum, kt | 592 | Mach 0.92 at sea level is about 608 kt (Wikipedia 594 kn) | -2.6% to -0.3% | Agrees |
| Hit points | 131 | none | | Unique to this record |
| Year | 1983 | first flight 1981, operational 1983 | 0 | Real (3 other records share 1983) |
| Radar / IR signature | 1 / 20 | stealth airframe | | Unique to the F-117 (the B-2 shares the radar value 1) |

The record's top speed is the fast edge of its 1 G polygon: 1,020 ft/s (604 kt)
at 36,000 ft and 1,000 ft/s (592 kt) at sea level. The speed of sound is 573.6
kt at 36,089 ft, so 604 kt is Mach 1.05 there. With loaded drag 0 and no
afterburner, the level top speed in flight is that edge (see the F-5E trim
section: top speed is the edge over sqrt(1 plus the loaded-drag fraction)).
Options for John (none chosen):

1. Keep the record as retail (documented, audit only).
2. Trim only the high-altitude fast edge to the real Mach (0.92 gives 527 kt at
   36,000 ft; 541 kt keeps the text figure), leaving sea level (592 kt) and
   every lower row as decoded. The 604 / 592 shape of the polygon means a single
   scale (as B5-5's 0.92 for the F-5E, here 0.87 to 0.90) would pull sea level
   to 517 to 530 kt, **below** the cited sea-level maximum, so a scale on the
   altitude vertices only is the closer match. Fitted, agent.
3. Correct maximum takeoff to 52,500 lb and the ceiling to 45,000 ft as well
   (the B5-2 precedent for Tornado values).

Agent note: nothing in the thrust or weights needs correcting; the record is
closer to the real aircraft than the Tornado's was. The G envelope rows are the
F-117's own (not copies).

### F-117 shape: state words and the rule 11 fold table

`f117.SH`: 207 runtime faces in the neutral state, 0 lines, 0 billboards, a
streamer record whose pivot is zero, **two state words only** (`4c56`, `4c5c`).
Measured with `shape_inspect` and a word sweep of values 1 to 8 and -1:

| Word | Value | Effect | Reading | Status |
| --- | ---: | --- | --- | --- |
| `4c56` | 1 | +5 / -1 faces (net +4): the one belly triangle `34e6` is replaced by its reverse winding `351d`, and four faces appear: two vertical walls | **Weapons bay open** | Confirmed by geometry (walls sit on the belly between y -27 and 22) |
| `4c5c` | 1 | +31 faces | **Landing gear down** (nose 9, left main 12, right main 10, counting door skins and wheel plates) | Confirmed by geometry (lowest point z -21) |
| `4c5c` | 4 | +3 degenerate faces (two equal vertices, zero area) | Line-like marks at x +/-28 (y 11 to -25, z -5) and x -1 (y 60 to 37) | **Unmapped**; invisible as faces, possibly wheel-well edges |

There is **no flame, airbrake, flap, rudder, elevon, ruddervator, canopy or
hook word**, and no separate geometry for any of them. The retail F-117 has no
afterburner and no brake, so the first two are expected; the control surfaces
are simply not in the source.

Both words together give 242 faces (207 + 31 + 4); `--branches` gives 212 / 247.

**Rule 11 face table** (counted from the word dumps; the hinge is the top edge
of each piece):

| Piece | Faces | Vertices | Moved | On hinge | Fixed |
| --- | ---: | ---: | ---: | ---: | ---: |
| Bay wall x -3 (`319c`, `31b1`, the same polygon in two windings) | 2 | 6 each | 3 | 3 (z -4/-5 top edge, one vertex 1 unit off) | 0 |
| Bay wall x +1 (`31ff`, `3214`) | 2 | 6 each | 3 | 3 | 0 |
| Belly triangle swap (`34e6` to `351d`) | 1 | 3 | 0 | 0 | 3 (a replacement, not a hinge) |
| Nose gear (`3765`, `3784` strut; `37e8`, `3807`, `3826`, `3845` crossed leg plates; `3897`, `38b0` side plate; `3657` door skin) | 9 | 4 to 5 | | top edge z -4 | 0 |
| Right main (`3286`..`3305` six leg faces; `3351`, `3366` wheel hexagons; `3676`, `36fa` door skins) | 10 | 4 to 6 | | top edge z -4 | 0 |
| Left main (`33d8`..`3457` six; `34a3`, `34b8`; `36a1`, `36c0`, `36df`, `3719` door skins) | 12 | 4 to 7 | | top edge z -4 | 0 |

The gear is rigid plates like the Tornado's (nose leg planes at x 0 and y 40,
main legs at x +/-28 and y -15, tyre plates at y -20). It reuses the shared
gear rig family (rigid pieces fold about the top edge). Fold verdict: no face of
a gear piece has a fixed vertex off its hinge; the bay walls are two planes
two windings each, so they are doors turning about their top edges, not a
folding surface. **The shape has no door faces for the bay**: the closed belly
is one triangle, and the open pose is the two walls (see the bay section).

### F-117 surfaces against the whole-surface gate (for John)

**V-tail (ruddervators).** Each side is one assembly of 10 faces (right:
`22c5`, `22d9`, `231a`, `232e`, `2383`, `244b`, `2460`, `2475`, `24b3`, `2e40`;
left: `23ab`..`255e`): a wedge from the root (1, -55, 3) to the tip (24, -96, 21),
canted about 39 degrees from horizontal. **No vertex of either tail is shared
with any other face of the shape** (the tail is a separate solid), so turning
the whole assembly folds nothing and leaves nothing stretched; the cost is a
seam between the tail root and the fuselage skin that a turn opens by up to
the root offset. This is the whole-panel case the standing decision describes:
all-moving, turning whole about a lateral spindle through the root chord
midpoint tilted by its own cant. The real aircraft's ruddervators are
described as the control surfaces of the V tail (the sources fetched do not say
whether they are all-moving; the lead's plan states all-moving, not re-verified
here). Pitch and yaw are mixed on a V tail (rudder from differential,
elevator from common deflection); rule 12's elevon-mix precedent covers the
sign convention. Options (none chosen): (a) whole assemblies about the cant
axis, pitch plus yaw mix, fitted limit; (b) as (a) with a small fitted limit so
the root seam stays closed; (c) leave the tails fixed.

**Elevons.** The wing has **no separate elevon faces**. The trailing edge is the
aft edge of the same large flat facets as the rest of the wing (`1e69`, `1e91`,
`1ea4`, `1eb6` outboard; `1e0a`, `1e42`, `1ff6`, `2725`, `2ddc` inboard; the
inboard trailing edge runs (0, -77) to (30, -52) and the outboard (35, -48) to
(85, -81)). Turning any one facet would be a diagonal or partial surface and
fails the gate; the facets are chord-wise, not hinge-line strips. Real: elevons,
inboard and outboard, on each wing (the lead's plan; not re-verified). Options
(none chosen): (a) leave the elevons unrigged (the sources have none; the
Tornado left slats and spoilers the same way); (b) author split panels by
cutting the trailing facets along a hinge line (fitted new geometry, John's
call, the Mirage and Tornado precedent for adding geometry); (c) pitch and roll
through the tails only (a V tail can roll with differential ruddervators only
in a small amount). **Real layout and chord ratios were not found in a public
source in this pass.**

**Bay doors.** Covered below (open pose only; no door panel in the source).

### F-117 gear

At 1/3 ft per unit: nose leg at y 36 to 43 (centre y 40), main legs at x
+/-28 to 29, y -8 to -19, axle y -15 to -20. Wheelbase 55 units = 18.3 ft; track
57 units = 19.0 ft (raw). With the per-axis scale below, wheelbase 19.2 ft and
track 14.5 ft. The lowest point with the gear down is z -21 units (7.0 ft raw,
6.2 ft after the height factor). No public source for the real wheelbase or
track was found in this pass. The nose and main legs have no door panels that
open; the `z -6` plates added with the gear are skins 1 unit under the belly.
Retraction direction is not in the source; none was cited. The record holds
`gearDrag` 23 (the 105-record common value).

### F-117 real size (for John)

Measured on the neutral state (gear up) and the gear word: x -85 to 85, y -96 to
93 (the thin nose probe from 89 to 93), z -6 to 21; gear down low -21. Header
scale exponent 8, so 1/3 ft per unit.

| Axis | Units | At 1/3 ft (current default) | Real | Verdict |
| --- | ---: | ---: | ---: | --- |
| Span (wingtips x +/-85) | 170 | 56.7 ft | 43.33 ft | **+30.8%** |
| Length (tail to probe tip) | 189 (185 to the nose) | 63.0 ft (61.7) | 65.92 ft | -4.4% (-6.4% to the nose) |
| Height (gear low to fin tip) | 42 | 14.0 ft | 12.42 ft | +12.7% |

The shape is **too wide**, as the Mirage 2000 was (and unlike the Tornado, whose
header scale of 2 made it true size). A leading-edge sweep check supports a per-axis fit:
the edge runs from (13, 62) to (85, -71); at identity that is 61.6 degrees
from the lateral axis, the real aircraft's commonly quoted sweep is 67.5
degrees (not fetched here), and with the per-axis factors below it is 68.4
degrees. Per-axis factors by the `aircraft_scale.rs` formula
(`real_ft * 3 / extent_units`): **x 0.7647, y 1.0463 (length measured to the
probe tip, as the Mirage F1 was; 1.069 to the nose), z 0.8869** (the gear low
point 21 units becomes 18.6, 6.2 ft). Options for John (none chosen):

1. Keep at identity (a 57-ft wide, 14-ft tall F-117).
2. Per axis to the real span, length and height (the Mirage 2000 and F1
   precedent, "per axis to real"), factors above.
3. Uniform scale only (one factor cannot fit all three: 0.78 for the span, 0.96
   for the length and 0.89 for the height).

Whatever is chosen is a `shape_axis_scale` row and moves stations, the gear low
point, the damage looks (same header), the tail rig and the bay with it.

### F-117 hardpoints and the bay (for John)

Location byte of each row is the last record field (labelled `name` in the
schema): 1 Fuselage, 3 Internal Bay. All six rows in order:

| Row | Location | Flags | x / y / z | Feet right / up / fwd (raw) | Max items | Weight class | Default | Role |
| ---: | --- | ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | Fuselage | `0x1008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `VIS340.SEE` | Fixed visual sensor |
| 1 | Fuselage | `0x1008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `PAVESPK.SEE` | Fixed designator row |
| 2 | Fuselage | `0x1008` | 0 / 0 / -50 | 0 / 0 / -16.7 | 1 | 0 | `F18.ECM` | Fixed jammer |
| 3 | Fuselage | `0x1008` | 0 / 0 / 0 | 0 / 0 / 0 | 1 | 0 | `AV8IR.SEE` | Fixed infrared row |
| 4 | **Internal Bay** | `0x1100` | -10 / -22 / -2 | **-3.3** / -7.3 / -0.7 | 1 | 20 | `PAVEWA3.JT` | Left bay station |
| 5 | **Internal Bay** | `0x1100` | 10 / -22 / -2 | **+3.3** / -7.3 / -0.7 | 1 | 20 | `PAVEWA3.JT` | Right bay station |

The 6 hardpoints are four fixed equipment rows (no stores) and **two bay rows**,
one per side, each one-sided (no pair flag `0x4`, like the F-22's bay rows),
each holding one store of weight class 20 (2,000 lb), the GBU-27. So the record
already says "two 2,000 lb weapons in two bays". Row 4 and 5 map to the two real
trapezes of the lead's description; the sources fetched (Wikipedia, aerospaceweb)
say "two internal weapons bays with one hardpoint each", up to two GBU-10/12/27
laser-guided bombs, two BLU-109, or two GBU-31 JDAM, 5,000 lb in all (MILAVIA:
"one internal weapons bay", Wikipedia text "split internal bay"). Flags `0x1100`
equal the B-52 bay rows' mask; which weapons that mask admits is a TORE
compatibility result for F117-5 or F117-6 (it was not computed here, the F-117
being unimported). The retail row positions are the "where retail releases"
points: the 7.3 ft depth is below the belly; the stores sit inside the airframe.

**The shape's bay (measured).** Open pose (word `4c56` = 1): two vertical walls
(x -3 and x +1, 4 units apart, the six-vertex outline from (-3, 22, -5) to
(-3, -27, -12)) spanning y -27 to 22 and z -4/-5 down to -12. That is a slot
**49 units long, 4 wide and 8 deep: 16.3 by 1.3 by 2.7 ft at identity, 17.1 by
1.0 by 2.4 ft after the per-axis factors**, centred x -1. The closed belly is
one triangle. The record rows sit at x +/-10 units, outside this slot: the slot
is a central gap or keel, not two bays, and cannot hold a 1.2 ft body with a
5.4 ft fin span (GBU-27: 13.9 by 1.2 ft body, 5.4 ft fins, `store_sizes.rs`).
The length fits (a 13.9 ft bomb in 16 to 17 ft); the width does not.
**No door panels exist** and the real door count is unknown here.

**Proposal (none decided): authored bay stations, labelled fitted or opinionated**,
under standing decision 2, reusing the F-22 internal-bay mechanism. What that
mechanism is (read from the code and [stores on
pylons](../spec/stores-on-pylons.md#f-22-naval-stores)): the source's
`Location::InternalBay` rows are re-placed by hand into `Location::Magazine`
(byte `0x83`, `placed_on` in `crates/tore-sim/src/combat/hardpoints.rs`, keyed by
aircraft and row), with a hand-set mount (centre x, floor height, release origin
forward); stores are drawn inside on fitted tandem rails (forward leaves first,
a length plus 0.25 ft apart), not counted as external stores by the radar
signature, and released through doors that the animation opens (fitted 1 s
hold, `OPENING BAY`, `O` key). The belly is clipped over fitted rectangles and
the doors are fitted.

1. **Stations.** Keep the two source rows as the stations (row 4 left, row 5
   right, one 2,000 lb store each: source-derived count and weight class). New
   `F117` arm in `placed_on` with fitted mounts: centre x +/-2.55 ft (the
   record's +/-10 units at the per-axis x factor; +/-3.3 ft at identity), floor
   about 1.5 ft under the reference (belly at z -5 units is -1.5 ft at the z
   factor), release origin about -0.7 ft forward. **Fitted, agent, opinionated
   for John.**
2. **Bay opening.** The shape's slot (1.0 ft wide) is too narrow for the stores:
   cut two fitted rectangles in the belly face `34e6` (the F-22's "source bounds
   extended" approach), each about 2.6 ft wide (a 1.2 ft body with clearance)
   and 17 ft long, centred x +/-2.55 ft, y -27 to 22 units, depth 2.4 ft from
   the walls, with one door per bay hinged at the outboard edge (the F-22 has two
   doors a bay; the F-117's door count is not in any source fetched). The
   `4c56` walls become the recess's inboard sides. **Opinionated, the numbers
   fitted.** Alternative: keep the single central slot and draw one store pair
   straddling it, which clips.
3. **Loads.** Default two `PAVEWA3.JT` (the record). Candidate rows from the real
   load, all within the weapons data (listed, none chosen): GBU-27 (`PAVEWA3`),
   GBU-10 (`GBU10`/`PAVEWAY`), GBU-12, BLU-109 (a 2,000 lb penetrator; no BLU-109
   record exists in the roster, only the GBU-27 which carries one), GBU-31 JDAM
   (if a JDAM record exists), and the Mk 84 (only if a source or John says so).
   The B61 is out of scope. Stores greater than weight class 20 stay refused by
   the row.
4. **Signature.** Under `docs/radar.md` an open bay adds 50 percent to the
   effective signature (`bay_open`) and bay stores add nothing; the bay doors
   would make the F-117's `Configuration.bay` deployed fraction non-zero. No
   new rule.
5. **No external stations.** The real F-117 has none (the sources and the record
   agree); nothing is added, and no pylons or pods (B7). A NASA external-store
   test (Test 461, The Aviationist, 2019) shows the aircraft could be given
   external stores, but it was a test, not a service configuration.

### F-117 sensors, jammer and pods (B7) (for John)

The record has four fixed rows and no radar row. Real: no radar, no external
pods; a turreted forward-looking infrared and a downward-looking infrared with a
laser designator (IRADS, built by Texas Instruments, in a recess beside the
nose gear well, FAS "IRADS"; Wikipedia: "thermal imaging infrared system with a
laser rangefinder/laser designator"), GPS and inertial navigation; whether any
radar warning equipment is fitted "remained classified as of 2008" (Wikipedia).

| Row | Retail record | Real | Options (none chosen) |
| --- | --- | --- | --- |
| `VIS340.SEE` | Visual, 10 / 5 nmi, 170 x 140 degrees (the common fighter eye) | The pilot's eye | Keep |
| `PAVESPK.SEE` | AN/ASQ-153 Pave Spike, fixed row, flag `0x1` at (0, 0, 0), ranges at the maximum (32767) | The real designator is IRADS' laser | (1) **Keep as retail**: it is the aircraft's own laser, and TORE's laser bombs home only on the aircraft's own (or a buddy's) laser spot, so the GBU-27s need it; no pod drawn, nothing on a station (B7 satisfied). (2) Keep and relabel it IRADS in the display text only. (3) Drop it (the bombs would then need a buddy's laser): breaks the real mission |
| `AV8IR.SEE` | Infrared, flag `0x2`, ranges 10,920 / 8,190 units (the table lists AV8IR at 15 / 15 nmi for the X-31 and F-22A) | The FLIR (forward-looking) | (1) Keep as retail (IR channel, ground scan scope; no pod). (2) Relabel as the FLIR/DLIR in text only. (3) Drop (no IR channel; loses the infrared view the F-117 is flown with) |
| `F18.ECM` | `ALQ-161`, mode flags `0x1F0`, radar deception 30 percent, fixed internal | No jammer or deception system is documented (RWR "classified") | (1) Keep as retail (the F-18's ALQ-161 on a stealth aircraft is a template). (2) Drop the row: the J key does nothing, the aircraft relies on its signature (the closest to real; changes the record row). (3) Keep it but it is a John call on whether a deception suite fits the F-117 character |
| Radar | none | none | **Retail already correct**; do not add `F18R.SEE` |

No pod body is ever drawn for these rows: they name no shape. The fixed-pod
body path in `crates/tore-app/src/pylons.rs` draws a fitted body only on
a station; rows 0 to 3 are not stations, so nothing hangs anywhere.

### F-117 stealth finding (standing decision 4)

**The code already has a radar and an infrared signature concept, and the record
carries a stealth signature.**

- `crates/tore-formats/src/parked_aircraft.rs` and
  `crates/tore-sim/src/sensors/signature.rs`: `SignatureProfile::from_source`
  reads `sigs[3]` (radar) and `sigs[2]` (infrared) from every aircraft record
  as relative game statistics, "not square metres". Detection range follows
  `min(nominal, nominal * sqrt(signature / 100) * L * N * J)`
  ([docs/radar.md](../radar.md)). Infrared uses `sqrt(PT_IR_signature / 100)`
  (`docs/radar.md`, IR section).
- The F-117 record: **radar 1, infrared 20** (and `sigs[0]`/`[1]` 162, which
  `tore-sim/src/turbulence.rs` reads as the turbulence input, not a signature).
  The B-2 record: radar 1, infrared 15, visual 220. The F-22: radar 10, infrared
  60. The ordinary fighter value is 100.
- Effect, against a 90 nmi radar nose-on in clear air (the table values): the
  F-117 clean is seen at **9.0 nmi** (the F-22 28.5), 18.0 nmi seen from above
  (the aspect weights 1/2/4), 11.0 nmi with the bay fully open (the `bay` term,
  +50 percent). Bay stores add nothing; external stores would add `3 per store`
  plus `2 per 1,000 lb`, a large effect on a signature of 1 (none on the real
  aircraft). Infrared range is `sqrt(0.2)` = 45 percent of nominal.
- The same function serves radar search and track, the jammer comparison, radar
  and active missile seekers and the RCS contour page.
- **Limits.** Surface air-defence radars "detect by their records' range bands
  and read no signature at all" (`docs/radar.md`), so SAM sites see an F-117 at
  full range. Visual detection ignores signatures. The only retail stealth
  mechanic outside the record is the **night rule**: at night with an F-117 or a
  B-2 in a friendly Quick Mission wing, every AAA site is a ZSU-23 with a
  novice crew (`NIGHT_STEALTH_AIRCRAFT`, `crates/tore-formats/src/quick_template/tables.rs`;
  [surface-defenses.md](../spec/surface-defenses.md), dormant until the aircraft
  is imported; `docs/formats/quick-mission.md`).

So decision 4 resolves without a new fitted value: the **real signature is
already the record's own** (radar 1, retail data; not an invented value). The
B-2 shares it and the review of the two agrees. Remaining questions for John
(no sensor change made): whether surface radars should read the signature
(retail does not), and whether the signature 1 (a tenth of range) is the
wanted strength (the F-22 at 10 is the stealthy baseline).

### F-117 unknowns and next steps

| Unknown | Why it matters | Next step |
| --- | --- | --- |
| Top speed 604 kt at 36,000 ft (Mach 1.05) | A subsonic aircraft flies supersonic | John, options above |
| Draw scale: span +30.8 percent | A wide F-117 | John, per-axis options |
| Bay width (shape slot 1 ft against a 5.4 ft fin span) | Bay stores clip | John, bay proposal; F117-3 and F117-5 |
| Elevons not in the source | No whole surface to turn | John, options above |
| V-tail seam (no shared vertices) | Opens a seam when turned | F117-3 probe |
| Real wheelbase, track, bay size, door count | Not found in public sources fetched | A cited drawing, or fitted |
| `4c5c` = 4 degenerate faces | Unmapped | None needed |
| Mask `0x1100` compatible weapons | Which GBU/BLU the bay offers | F117-5 compatibility probe |
| `F117.HUD` glass polygon and sight type | No aperture entry | F117-7 |
| Internal fuel 18,500 lb | Unverified | F117-8 |
| Strike class override (5,000 ft level pass) | AI strike altitude | F117-9, John's call |
| Surface radars ignore signatures | The F-117 is seen by SAM radars at range | John |

Sources (accessed 2026-11-15): Wikipedia "Lockheed F-117 Nighthawk"; MILAVIA
F-117 specifications (milavia.net/aircraft/f-117/f-117_specs.htm); aerospaceweb
F-117 (aerospaceweb.org/aircraft/bomber/f117/); FAS IRADS
(man.fas.org/dod-101/sys/smart/irads.htm) and FAS GBU-27
(man.fas.org/dod-101/sys/smart/gbu-27.htm), as summarised by the search
results; the real leading-edge sweep and elevon layout are not from a fetched
source.

## Bomber batch A identities (BMB-A1, 2026-11-15)

Implementation, batch slice BMB-A1, Claude Sonnet. The three source reviews above
(B52-0, B2-0 and F117-0, all 2026-11-15) are now identities. `AircraftId` gains
`B52`, `B2` and `F117`, appended in that order after `Mf1`: `ALL` goes from 45 to 48
retail identities and `SELECTABLE` from 47 to 50 entries. The order is append-only
(batch rule B4): the Tornado IDS (Clean) that TOR-13 inserted into `SELECTABLE`
stays where it is (checkpoint code 46, wire position 45), so no existing code or
wire position moves.

| | B-52 | B-2 | F-117 |
| --- | --- | --- | --- |
| Record | `B52.PT` | `B2.PT` | `F117.PT` |
| Parse keys | `b52`, `b52.pt` | `b2`, `b2.pt` | `f117`, `f117.pt` |
| Checkpoint code | 47 | 48 | 49 |
| Wire position (place in `SELECTABLE`, 9 bits) | 47 | 48 | 49 |
| Place in `ALL` | 45 | 46 | 47 |
| `typeSize` | **612 (new arm**, the AC-130's) | 660 (explicit arm) | **588 (new arm**, the MiG-31's) |
| Short / long name | `B-52` / `B-52 Stratofortress` | `B-2` / `B- 2 Spirit` | `F-117` / `F-117A Nighthawk` |
| Shape stem | `B52` | `B2` | `F117` |
| HUD | `BMBROH.HUD` (the record's) | `B2.HUD` (explicit arm: the record's `hudName` is null) | `F117.HUD` (explicit arm: null in the record) |
| Cockpit stem / picture | `AC130` / `~AC130H.PIC` | `B2` / `~B2H.PIC` | `F117` / `~F117H.PIC` |
| PTS companion | none | `B2.PTS` | `F117.PTS` |
| Radar / gun | `B52R.SEE` / `M61.JT` | `F18R.SEE` / none | none / none |
| Live-fire default stores | `M61`, `AGM84E` | `MK82`, `PAVEWA3` | `PAVEWA3` |
| Strike class (rule, no override) | Heavy | Heavy | Attack |
| Interim rig: CODE bytes / runtime faces | 27,204 / 271 | 21,668 / 268 | 15,470 / 207 |
| State words | `7a20 7a26 7a2c 7a32 7a38` | `6486 648c 6498` | `4c56 4c5c` |
| Gear word / faces added | `7a26` +32 | `648c` +14 | `4c5c` +31 |
| Gear low point (shape units) | 14 at 2/3 ft | 33 at 1/3 ft | 21 at 1/3 ft |
| Draw scale | **2/3 ft a unit (header scale 2)** | 1/3 ft a unit | 1/3 ft a unit (identity; F117-2 owns the per-axis fit) |

**No record correction is applied.** Every record value is the retail value; the
corrections John accepted (decisions 5, 16, 23 and the rest) come in BMB-A2. The
wire identity is 9 bits since protocol 83, so 50 selectable identities use codes
0 to 49 of 512 and the compile-time assertion still passes; appending three
identities at the end needs no protocol change.

**John's accepted bindings applied (2026-11-15).**

| Decision | Applied |
| --- | --- |
| 6 (B-52 size) | `debris::scale` lists `B52` beside the Tornado, the MiG-23 and the transports (2/3 ft a unit); the interim rig spec carries 2/3. |
| 14 (B-52 HUD) | Follows the record: `BMBROH.HUD`, the AC-130 panel family (`~AC130H.PIC`, `~AC130_P.PIC`). Bound by name; `AC130.PT` is not aliased. |
| 13 (B-52 gun) | `M61.JT` stays the B-52's gun and is a live-fire default; the move to the tail cone and the 1,242 rounds are BMB-A2 and B52-2. |
| 15, 17 (B-2 name, HUD) | Name stays `B- 2 Spirit`; `B2.HUD`, cockpit stem `B2` and `~B2H.PIC`, by an explicit arm. |
| 28 (F-117 AI, HUD) | `F117.HUD`, stem `F117`. No `strike_class` override: the rule already gives the F-117 Attack (604 kt, 54,000 lb, +6 G), so `OVERRIDES` stays empty. The retail level pass is F117-9's. |
| Strike class | B-52 and B-2 sort Heavy by the rule (488,000 and 400,000 lb). |

**Interim choices (agent decisions 2026-11-15, until the cockpit and rig slices).**

| Choice | Interim | Why |
| --- | --- | --- |
| HUD glass of the B-2 and F-117 | `hud_aperture.rs` fails closed (an empty mask) | `~B2H.PIC` and `~F117H.PIC` have no measured polygon. Their cockpit slices measure them. |
| HUD glass of the B-52 | The AC-130 family's whole-picture aperture (the arm the C-130, E-3, IL-76 and the airliners use) | The B-52 flies the same `~AC130H.PIC` picture, so it takes the shared reviewed glass; it needs no polygon of its own. Switch it to an empty mask if John wants it closed until a B-52 cockpit slice. |
| Rig | Gear word only; the other words pinned, unmapped | As batch 5: the shared fitted gear law until each aircraft's rig slice. |
| B-52 empty bay rows | Rows 5 and 6 keep an empty Mk 82 selection | The Harpoon and the M61 (the only defaults) fit neither row, so without a fallback the audit found the rows hiding compatible weapons (`live.rs`, the same device as the Mirage F1's belly pair). |

**The reader change (B2.SH loads).** `B2.SH`'s state guards use two conditional
jumps the reader refused with `unsupported shape state guard`: `0x7c` (jump if
less) and `0x7f` (jump if greater), both on the signed drag-rudder word `6498`.
`tore-formats/src/shape.rs` now takes them beside `0x74` (equal), `0x75` (not
equal) and `0x7d` (greater or equal): `0x7c` takes the branch when the word is
less than the immediate, `0x7f` when it is greater, signed. Decision 22 put this
in B2-2; the identity probe needed the shape to load. The reader still executes
no original code; it selects inert geometry. Tests: a unit test
(`signed_less_and_greater_guards_select_geometry_by_the_signed_compare`) covers
both opcodes against negative, zero and positive words and a non-zero immediate;
the existing flap-guard test now expects the next unknown opcode, `0x7e`, to be
refused (it had used `0x7f`); a retail-reading test (`bomber_batch_a.rs`) loads
`B2.SH` at 268 faces and checks word `6498` at 0, +1, -1 and 2 (268, 272, 272, 266)
and the gear word (+14).

*Retail-wide effect.* A scratch example (deleted afterwards) loaded every one of
the 1,275 shapes in `.local/m3-imports/scratch/shapes` through `Shape::parse`,
`Shape::with_export_state` and `Shape::scenery`, once with the reader before the
change and once after, and diffed the result lines (outcome, face, line and
billboard counts). **Exactly one line differs: `B2.SH`**, which goes from the
three `unsupported shape state guard` errors to 268 faces (269 in the export
state, 268 scenery). The other 1,274 shapes are identical, including the 17 that
already fail for reasons unrelated to guards (the effect and bullet pieces:
`BULLET`, `CHAFF`, `DEBRIS`, `EXP`, `F8`, `FIRE`, `FLARE`, `MOON`, `MOTH`,
`MOTHB`, `QUE`, `SMOKE`, `STARS`, `SUN`, `TRACER`, `WAVE1`, `WAVE2`). Nothing
else changes because the new arms only turn a former error into a branch.

**What the identity probe found.**

- The B-52's bay rows 5 and 6 (flags `0x1120`, no default store) need a retained
  selection, as above: with it the audit counts the two rows as recovered
  stations.
- The B-2's four bay rows carry flags `0x1504`, which include the decoded
  designator bit `0x400`, so the audit's designator-pod check (pins below) counts
  four pod placements on a B-2 that, on the real aircraft, has no pod station
  (B7). That is a retail row flag; BMB-A2 or the B-2 station slice decides it.
- The B-2 and F-117 cockpits have only the four panel pictures (`~B2.PIC`, `~B2H.PIC`,
  `~B2S.PIC`, `~B2_P.PIC` and the `~F117*.PIC` set), no left, centre or right
  overlay. `cockpit_overlays()` assumed overlays for any stem it did not list, so
  the first headless flight stopped at `aircraft cache missing ~B2_LH.PIC`;
  `B2` and `F117` now join the stems with no overlays (the B-52's `AC130` stem
  already was).
- `B52.PT` is the only one of the three without a `.PTS`, so `has_pts()` is false for
  it alone; `B2.PTS` and `F117.PTS` (inert) are expected.
- The F-117 record has no radar row (no `F18R.SEE`), so `radar()` is `None`; the
  B-2 keeps `F18R.SEE` (the B-1B's row, decision 18).
- The records are not parked targets and `parked_aircraft.rs` is unchanged.
- The deployed gear low point is 14 units at 2/3 ft for the B-52 (9.3 ft), 33
  units for the B-2 (11.0 ft) and 21 units for the F-117 (7.0 ft); real-size
  fits (F-117 per axis, F117-2) move them later.
- The B-52's variety flight model takes the default ground alignment rate (0.7);
  the transport aircraft use 0.4. Flight tuning is B52-8's.

**Audit pins (each with a dated note in `ordnance_audit.rs`).**

| Pin | From | To | Why |
| --- | ---: | ---: | --- |
| Aircraft | 45 | 48 | The three identities |
| Recovered (optional) weapon stations | 18 | 20 | The B-52's two empty bay rows |
| Weapon placements | 4,581 | 4,792 | B-52 71, B-2 102, F-117 38 |
| Tank placements | 233 | 233 | None of the three has a tank row |
| Cheat-only novelty placements | 222 | 240 | 18 more on the bay rows |
| Designator-pod placements | 80 | 84 | The B-2's four bay rows |
| Weapon records, tank records | 82, 9 | 82, 9 | No record added |

First measurements of the retail rows; each aircraft's station slice owns its
count. The `menus-ordnance-availability` and `a2g-catalog` battery expected
lines and `tools/test_battery_a2g.py` follow
([ordnance availability](../baselines/ordnance-availability.md)).

## B-2 shape, size and looks (B2-2, 2026-11-15)

Implementation and measurement, slice B2-2, Claude Sonnet (sonnet-high). John's
decisions of 2026-11-15 ("accept all" of the bomber batch A decisions): 19, the B-2
stays at 1:1, and 22, the reader fix (done in BMB-A1, checked here). No
production code changed: the B-2 already draws at the shape's header scale
through BMB-A1's entries, so this slice is measurement, a retail-reading test
(`crates/tore-import/tests/b2_shape.rs`, skips without media), one battery
scenario (`render-b2-skin`) and this record. Frames are local only, in
`.local/m3-aircraft/frames/B2-2/`.

### Size at 1:1 (decision 19), measured again

`B2.SH` from `FA_2.LIB` (CODE 21,668 bytes, header exponent 8, so `object_scale`
is 1 and a unit is 1/3 ft). The reader now returns the same extents the B2-0
review took from its private patched copy (neutral state, gear down, raw units):

| Axis | Units | Feet at 1/3 | Real | Difference |
| --- | --- | ---: | ---: | ---: |
| Span, x -258 to 258 | 516 | 172.0 | 172 | exact |
| Length, y -87 to 107 | 194 | 64.7 | 69 | -6.3% |
| Height, z -33 (gear) to 24 | 57 | 19.0 | 17 | +11.8% |
| Height gear up, z -12 to 24 | 36 | 12.0 | | |

The leading edge from the nose (0, 107) to the tip (258, -59) is 32.76 degrees
from the lateral axis (Jane's 33). **No scale entry is needed**:
`aircraft::scale::shape_axis_scale(B2)` is the identity `[1, 1, 1]` (pinned by
the walk over `ALL` and by the retail test), `debris::scale(B2)` is 1/3 (a
BMB-A1 test), and the interim rig spec says 1/3. Every consumer of the scale
reads one of those:

| Consumer | At 1/3 ft a unit? |
| --- | --- |
| Drawn aircraft | Yes: `Rig::load` takes `debris::scale`; the generic rig spec says 1/3 (the existing unit test that the generic rig scales equal the simulation scale covers it) |
| Stores preview (`b2` key), station underside, damage looks, damage bodies and pieces | Yes: the same header scale (all four damage shapes have exponent 8, retail test) |
| Breakup piece offsets | Yes: `debris::attachment` multiplies by `scale(B2)`. Variant 0 (nose, cockpit, core hits: `B2_A`, `B2_B`) offsets the right-wing piece 42.8 ft right, 4.0 ft aft and 0.5 ft up; variant 1 (wing and tail hits: `B2_C`, `B2_D`) puts the 31 ft piece 26.8 ft aft and 3.2 ft right (geometric fit, not an original transform). The wing hits use the tail-end piece, as the shared rule does for every aircraft |
| Gear low point | Yes: 33 units, 11.0 ft (BMB-A1 test) |
| Wingtip vapor | Yes, **and the record is valid** (see the update below): the points are the drawn wing tips |
| Parked placement | None: `B2.PT` is not a parked target |
| Contrail and nozzle offsets | Come off the rig scale, 1/3; the nozzle faces are unmapped and the B-2 has no flame word, so B2-4 owns them |
| Hit size, cockpit eye point | Not shape-derived: one 28 ft sphere and one fixed seat offset for every aircraft. Measured at the B-2's size below; unchanged |

Update to the B2-0 review ("B-2 dependency closure" table, 2026-11-15): that
review called the streamer record meaningless and said to treat it as absent.
The reader returns a valid record: zero pivot and both points (right, up, forward
in third-of-a-foot units) 258, -3, -59, which is exactly the drawn wing tip
(x 258, y -59, so 86 ft out and 19.7 ft aft). The B-2 therefore already has wingtip
vapor points on its tips, a retail test pins it, and nothing needs a fix.

### Gear contact and low points

Measured on the plates the gear word adds (14 faces, two windings each, raw
units, z is height):

| Plate group | x | y | Low z | Feet below the origin |
| --- | --- | --- | ---: | ---: |
| Nose pair (two plates) | -6 to 7 | 71 and 97 | -24 and -14 | 8.0 and 4.7 |
| Nose strut and wheel | 0 | 88 to 96 | **-31** | 10.3 |
| Inner main plates | -65 and 62 | -2 to 20 | **-33** | 11.0 |
| Outer main plates | -71 and 69 | -7 to 41 | -27 | 9.0 |

The simulation's gear low point (`ground_clearance_ft`) is the inner mains, 33
units, 11.0 ft. The nose wheel therefore sits 2 units (0.67 ft) above the main
wheels when the aircraft is level, about 1.4 degrees of pitch over the 27.7 ft
wheelbase (nose strut y about 92, mains y about 9); the outer main plates hang
2 ft higher than the inner ones. The ground-start frames (`b2-ground-side`,
`b2-ground-side-zoom`, `b2-ground-front`, `crop3x-ground-gear`) show the mains on
the pavement and the nose wheel at or just above it, with no sinking or floating
that the eye can pick out. The belly clearance at the closed wells is 22 units
(7.3 ft).

### Drag-rudder poses (decision 22, checked)

The BMB-A1 reader fix is complete: word 6498 loads at every value, and the retail
test pins it.

| Value | Faces | Removed (closed pair) | Added | Look |
| ---: | ---: | --- | --- | --- |
| 0 | 268 | none | none | both closed (neutral) |
| +1 | 272 | `465f`, `4679` (right) | six: upper plates `47db` and `47f5`, lower plates `47a7` and `47c1`, front plate `480f`, inboard cap `4829` | right rudder open |
| -1 | 272 | `4600`, `461a` (left) | six: `46d0`, `46ea`, `4704`, `471e`, `4738`, `4752` | left rudder open |
| 2 (and any larger) | 266 | `465f`, `4679` (right) | none | right pair removed, hole |
| -2 (and any smaller) | 266 | `4600`, `461a` (left) | none | left pair removed, hole |

Added geometry, raw units: right x 167 to 215, y -77 to -46, z -14 to 7; left x
-218 to -169, y -78 to -46, z -14 to 8. The two sides mirror about the mesh axis
(x = -1) within 1.5 units. Every added face is a whole quad, except the inboard
triangle cap. The `+2` and `-2` poses extend the B2-0 table (it recorded the
value 2 only): the second is the left-hand counterpart. The bay word 6486 adds
eight faces (four plates, two windings) and composes with the gear and rudder
words (`6486`+`648c`+`6498`=+1 gives 268 + 8 + 14 + 4).

The interim rig does not map the word (that is B2-3), so the poses were drawn
with a temporary, uncommitted, env-gated patch to `variety_animation.rs` that
replaced the loaded shape with `Shape::with_state` at each value (gear word set
too); it was reverted before the checks and is not in the tree. Frames
`rudder-{0,1,m1,2}-top`, `-rear` and `-rear-left` and the 4x crops
`crop4x-rudder-1-right` and `crop4x-rudder-m1-left`: in the open poses a dark upper
plate and a lighter lower plate hang off the trailing edge outboard of the engine
bays, the right in +1 and the left in -1; pose 2 leaves an empty notch where the
right closed wedge was. No wrong winding, no missing faces and no show-through
anywhere in the four poses (the plates are drawn in both windings). The sign
convention (positive rudder opens the right, as the reader reads it) is B2-3's to
tie to the rudder input.

### Skin and decals

- **Twins.** 11 exact-vertex twin groups, all double-sided panels with opposite
  stored normals, so the smooth path pairs and hides the rear members and no
  MiG-21 exception is needed (retail test).
- **Decal slot.** The export path finds one `E0` slot the runtime path omits (269
  against 268 faces): address `2a73`, texture `@indexed:2`, a 7 by 5 unit plate
  (x 21 to 28, y 60 to 65, z 4 to 5) on the top skin right of the cockpit.
  It stays omitted, as on the Mirage 2000, the F-5E and the Tornado: the top view
  (`crop3x-nose-cockpit-top`) shows no blank or skin-filled patch, and the
  cockpit-side markings, the serial `USAF 21066` and the star-and-bar on the right
  wing are textured faces that draw whole. The top and under frames (gear down,
  `b2-top-view`, `b2-under-view`, and `render-b2-skin`) show no underside through
  the top skin and no z-fighting; the under view is the dark underside
  texture with the two gear-well panels and the bay panels.
- **Clipping.** None found: the open-bay plates, gear plates and drag rudders are
  separate whole faces, and the aircraft is a thin wedge with no part that crosses
  another.

### Damage looks

`B2_A.SH` 248, `B2_B.SH` 114, `B2_C.SH` 254 and `B2_D.SH` 74 faces load at the intact
header scale; `DamageArt::load` takes them with no error. Extents (raw units):
A x -237 to 95 (the right outer wing is gone), C x -259 to 257 with the nose 7 units
(2.3 ft) longer than the intact one (y 114 against 107), B x -62 to 158 and D x
-56 to 37 (pieces, smaller than the aircraft). No whole-body variant is registered
(`damage_variant`) for the B-2, so a destroyed aircraft draws the intact skin with
the regional cuts, as the other aircraft (damage 1.0 left-wing section: the left wing
cut, `b2-damage-1.0`). The two body variants
(`b2-damage-1.0-nose-variant` the A body; `b2-damage-1.0-tail-variant` the C body)
and 0.6 (`b2-damage-0.6`) all draw whole; the C body shows the interior, as the
other aircraft's C bodies do. The regional cuts follow the drawn extents (the wing
sections start at 30 percent of the half span), so on the B-2 they act on the whole
visible wing, not on the 26 ft the contact volume covers.

### Hit and damage volumes at true size (finding, nothing changed)

The sim's aircraft contact volume is not derived from the shape: the damage
sections are boxes in units of one 28 ft radius about the model origin
(`aircraft_contact`, `AIRCRAFT_RADIUS_FT`). In feet at 28 ft: core x +-7.8 and
10.6 aft to 5.0 forward; cockpit x +-6.2; nose x +-9.0, 11.8 to 25.8 forward; each
wing x 7.0 to 25.8 out, 7.8 aft to 10.6 forward; tail 25.8 to 9.5 aft. The B-2 is 86 ft to a
wing tip, 35.7 ft to the nose and 29 ft aft to the trailing edge, so **gun and
fragment contact covers only the inner 30 percent of each wing** (to 25.8 ft of 86)
and none of the outer wings, and the nose tip beyond 25.8 ft is outside. A round
that strikes the outer 60 ft of a wing misses; the same is true of a missile fuze,
which reads the same sphere. The regional damage looks and sections (above) are
shape-relative and so cover the whole wing. The same
fixed sphere applies to the B-52 (192 ft span) and the transports, so it is the
shared rule, not a B-2 error; making it per-aircraft (a radius or a box table keyed
by `AircraftId`) changes the simulation for the aircraft it touches (determinism,
`SIMULATION_VERSION`) and is John's decision, not made here.

### Eye point (finding, nothing changed)

The fixed seat is 7 ft up and 10 ft ahead of the model origin, that is shape y 30
and z 21. The B-2's own geometry there: the dorsal spine reaches z 24 (8.0 ft) at
y 40 (13.3 ft ahead), and the glasshouse is further forward (canopy top z 22, 7.3
ft up, y 60 to 70 and falling to z 14 by y 80: 20 to 27 ft ahead). The fixed eye
therefore sits 10 to 17 ft behind the glass and just under the spine skin. As on
the Mirage 2000, the F-15E and the Gripen the cockpit view draws the 2D cockpit
picture and takes no per-aircraft eye, so this does not show; B2-7 owns the cockpit.

Animation probe: `source_units_to_feet` 0.33333334 for the B-2; the four
required failures are the unmapped elevator, rudder, aileron and flaps (B2-3).

Frames (`.local/m3-aircraft/frames/B2-2/`, `.png` with `.ppm`; `cap.sh` and
`capg.sh` record the command: view 1 chase, 200 kt, `--instrument-layout small`,
1280x720, through `tools/agent-run.sh`): `b2-top-view`, `b2-side-view`,
`b2-front-view`, `b2-under-view`, `b2-threequarter`; ground `b2-ground-side`,
`b2-ground-side-zoom`, `b2-ground-front`, `crop3x-ground-gear`; drag rudders
`rudder-{0,1,m1,2}-{top,rear,rear-left}` and the crops; `crop3x-nose-cockpit-top`;
damage `b2-damage-0.6`, `b2-damage-1.0`, `b2-damage-1.0-nose-variant`,
`b2-damage-1.0-tail-variant`.

## Bomber batch A record and station corrections (BMB-A2, 2026-11-15)

Opinionated (John, 2026-11-15, "accept all" of the bomber decision round:
decisions 5, 13, 16, 23 and 26), implemented by slice BMB-A2. Earlier sections
above stay as written for the retail state; this one records what the game now
does. The behaviour and the sources of every changed number are in
[aircraft records](../spec/aircraft-records.md#bomber-batch-a-record-corrections-b-52h-b-2a-f-117a-bmb-a2-2026-11-15);
the audit pins in [ordnance availability](../baselines/ordnance-availability.md).

**Values (retail to game).**

| Aircraft | Field | Retail | Game |
| --- | --- | ---: | ---: |
| B-52H | Thrust, lbf / empty weight, lb / internal fuel, lb | 120,000 / 235,220 / 199,000 | 136,000 / 185,000 / 312,197 |
| B-52H | Fuel flow, lb/s | 18 | 20 (TF33-P-3 TSFC 0.52) |
| B-52H | Tail gun rounds (row 4) | 2,000 | 1,242 |
| B-2A | Thrust, lbf / empty weight, lb | 114,000 / 100,000 | 69,200 / 158,000 |
| B-2A | Internal fuel, lb / maximum take-off, lb | 180,000 / 400,000 | 167,000 / 376,000 |
| B-2A | Bay rows 5 to 8 flags | `0x1504` | `0x1104` |
| F-117A | Maximum take-off, lb / ceiling, ft | 54,000 / 42,000 | 52,500 / 45,000 |
| F-117A | 1 G fast edge at 36,000 ft, ft/s | 1,020 (Mach 1.05) | 891 (Mach 0.92) |
| F-117A | Row 2 default | `F18.ECM` | none |

**The F-117 envelope, point by point (speed ft/s at altitude ft).**

| Row | Retail | Game |
| --- | --- | --- |
| 1 G | 285@0, 305@17000, 367@30500, 458@37750, 570@42000, 715@42000, 847@40000, 1020@36000, 1000@0 | 285@0, 305@17000, 367@30500, 458@37750, 570@45000, 715@45000, 847@40000, 891@36000, 1000@0 |
| 0 G | 250@0, 265@16000, 327@29500, 418@38750, 570@44000, 715@44000, 897@42000, 1090@36000, 1060@0 | the same with the two top vertices at 47000 and 961@36000 |
| -1 G and 2 G | 315@0, 335@17000, 392@25500, 481@30750, 570@34000, 710@35000, 960@34000, 980@0 | 960@34000 becomes 897@34000 (held to the 1 G edge); the rest as retail |

No other row's vertex moves (every other fast side is already inside the new 1 G
edge). Guards: the corrections apply only while `maxAlt`, the 0 G and 1 G rows and the
station rows are the retail ones (`aircraft_corrections.rs`, `aircraft_stations.rs`).

**The B-52 fuel finding (the record validators).** Empty 185,000 plus fuel 312,197
is 497,197 lb, over the 488,000 lb maximum. The record validator (`Aircraft::parse`,
"aircraft empty plus fuel exceeds MTOW") and `Configuration::validate` rejected that
sum; both now reject only an empty weight at or above the maximum. TORE had no fuel
cap: `Loadout::new` filled the tank and the takeoff check refused an overweight load,
so the default B-52 load would have been refused. `Loadout::new` now caps the starting
tank so the default load fits (303,000 lb, whole pounds); no other aircraft is
touched. Free flight and `--headless-flight` use the model's full tank. Loadout
weight counts external stores only (`!s.internal`), so the B-52's and B-2's bay loads
(16,560 lb of Harpoons, up to about 50,000 lb at the row classes) are outside the
takeoff total.

**Station rows.** The B-52's tail gun row (4) takes 1,242 rounds; its position stays at
(0, 6.7, -73.3) ft (B52-5 moves it to the tail cone). The B-2's four bay rows lose the
designator bit, so pods and the several-projectile stores are no longer offered there
(weapon placements 102 to 86, pods 84 to 80). The F-117's row 2 keeps its place,
mask, count and position but no default store, which removes the late-Cold-War
jammer and 30 chaff and 30 flares of `F18.ECM`; no dispenser is added.

**Not corrected, listed for John.** B-2 fuel flow (6 lb/s, the B-1B's; the cited
F118-GE-100 TSFC 0.375 gives 7), F-117 fuel flow (2 lb/s, 0.333 implied; no cited
F404-F1D2 value found), the B-2 and B-52 top speeds and sea-level edges, the B-2
49,000 ft ceiling (real 50,000), and the engine count: `Configuration::from_source`
clamps `engines` to 1 to 4 (`live.rs`), so the B-52's eight engines read as four
(an existing behaviour, not touched here; B52-8's call).

## B-2 rig (B2-3, 2026-11-15)

Implementation, slices B2-3 and B2-4 run together (bomber batch A), Claude Sonnet
(sonnet-high). Binding: John's accepted decision 20 (2026-11-15): rig the elevon
wedges as whole panels (rule 12), drive the signed drag-rudder word from yaw, add
a fitted both-sides speed brake, rig the gear and the interim bay pose. The B-2
now loads through its own rig, `crates/tore-app/src/b2_animation.rs` (the shared
`variety_animation` row keeps only the layout, scale and `supported`), with its own
probe module (`aircraft_animation_probe/b2.rs`) and battery entries. Nothing in the
simulation changed. Frames are local only, in `.local/m3-aircraft/frames/B2-3/`.

Provenance: the face lists, hinge edges, retail open poses and gear plates are
spec-derived (retail `B2.SH`, read at load and refused if they differ). Everything
that moves them (limits, mixing, the speed brake, hinge heights, retraction
direction, the bay drop) is **fitted**, agent decisions of 2026-11-15, labelled
below. No retail bytes are committed.

### Elevons (rule 11 fold check and rule 12 mixing)

One four-face wedge per side with no state word (`4037 404e 4065 407a` right,
`40ce 40e5 40fc 4111` left). Re-checked by exact coordinates: no vertex of either
wedge is shared with any other face (7 distinct vertices per side, 0 neighbours).

| Face | Vertices | Move | On the hinge line | Fixed off the line |
| --- | ---: | ---: | ---: | ---: |
| `4037` top-of-wedge quad (right) | 4 | 4 | 0 (the line is the mid-thickness of the leading edge) | 0 |
| `404e` quad | 4 | 4 | 0 | 0 |
| `4065`, `407a` inboard triangles | 3 each | 3 each | 0 | 0 |
| Left `40ce 40e5 40fc 4111` | 4, 4, 3, 3 | all | 0 | 0 |

The whole wedge turns as one rigid body about its leading-edge mid-line (right
(63, -70, -3) to (120, -22, -3), left (-121, -22, -3) to (-64, -70, -3) in shape
units, a third of a foot each). The two leading-edge vertices at each end (z 0 and
-6) sit 3 units off the line, so the hinge-line opening against the fixed skin ahead
is at most 6 sin(angle/2) units: 0.9 units (0.30 ft) at pure pitch, 0.6 units
(0.20 ft) at pure roll and 1.5 units (0.49 ft) at the clamp. The probe measures the
midpoint of each leading-edge pair staying on the line (largest move 1.6e-7 ft) and
the wedge rigid to 5e-6 ft.

Mixing, rule 12: angle (positive lowers the trailing edge) = -0.30 x pitch
-/+ 0.20 x roll, right sign minus, left plus, clamped to +-0.50 rad. A pull raises
both trailing edges; a positive aileron raises the right and lowers the left. The
limits are the fleet's own fitted values (Mirage 2000, Rafale, MiG-31: 0.30 pitch,
0.20 roll), 17.2 and 11.5 degrees, 28.6 degrees at the clamp. **Fitted; no cited
B-2 elevon limit was found** (the one source found only says the outer elevons are
the primary pitch and roll surfaces and the inner ones low-speed, Virginia Tech
slides, `archive.aoe.vt.edu/mason/Mason_f/B2Spr09.pdf`; the mesh has one strip per
side, so the single wedge takes both terms). The flap lever moves nothing: the
flying wing has no flap panel.

### Drag rudders (word 6498) and the speed brake

The rig never sets word 6498 (a magnitude of 2 or more removes a pair and adds
nothing, B2-2). Each side keeps its two closed quads (`465f` upper, `4679` lower
right; `4600`, `461a` left) and turns each as one rigid plate about its own hinge
edge (z 0 for the upper plate, z -7 for the lower): right (177, -46) to (215, -71),
left (-178, -46) to (-218, -72). The open angle is read at load from the retail
open pose (the free edge's midpoint, about the hinge line), so the fully open rig
reaches the source pose to within 0.6 unit (0.2 ft), the plate's own chord stretch (the retail open
plate is 12.8 units on its chord against 12.1 to 12.4 closed, 5 percent, because the
source open pose is not an exact rotation):

| Plate | Hinge z | Angle to the retail open pose | Free edge z, closed to open |
| --- | ---: | ---: | --- |
| Right upper | 0 | 47.6 degrees up | -3 to +6.4 and +7.0 (retail +7) |
| Right lower | -7 | 52.1 degrees down | -3 to -13.5 and -14.2 (retail -14) |
| Left upper | 0 | 50.3 degrees up | -3 to +7.0 and +7.2 (retail +7 and +8) |
| Left lower | -7 | 52.9 degrees down | -3 to -14.0 and -13.8 (retail -14) |

A closed quad has one winding, so the rig adds a reversed-winding twin (built on
the retail `back` face of the open pose: `47f5 47c1 46ea 471e`) while a side is
open, plus the retail vertical front plate and inboard cap (`480f 4829`, left `4738
4752`), drawn only while that side is open. Rule 11: each plate is a whole four
vertex quad, both hinge vertices on the hinge line (largest hinge move 1.6e-7 ft),
the two plates of a side meet only at the trailing edge, which opens by design.

**Mapping (fitted).** Open fraction of the right side = min(1, brake +
max(rudder, 0)); left = min(1, brake + max(-rudder, 0)). A positive rudder (a right
yaw) opens the right drag rudder and a negative rudder the left, each from closed
to the one open pose in proportion to the command. The real B-2 yaws by opening the
drag rudder on the side it yaws toward, which raises the drag on that wing (Aviationist,
"B-2 control surfaces" close-up of the pre-flight check, 2021-03-10; AeroXplorer,
"How does a B-2 Spirit fly without a tail?"; the flight computer blends them with
differential thrust). This is the same sense as the rig's `+1` (right), the repository's
positive rudder (trailing edge to starboard, yaw right). Probe rows
(`yaw-brake-combinations.csv`, 25 poses): the open sides match the formula, every
plate rigid, direction correct, the right free edge never falls as the brake
grows.

**Speed brake (fitted, decision 20; label fitted).** The shape encodes one side
open at a time (a signed word), so the both-sides pose cannot come from the word.
The rig opens both sides by turning the closed quads, so the pose is the same
geometry the left and right poses already use, mirrored and shown together: the
"compose both open poses" method works because the rig turns plates instead of
choosing a word. It is wired to the existing airbrake control (the `B` key,
`Switch::Airbrake`, `State::brake`, which ramps over the record's deployment time)
and its drag is the existing one (`airBrakesDrag` 256 in `B2.PT`, applied as
`airbrake x brake` by the flight model; the B-2 record is the B-1B's, so the figure is
the template's). Nothing was added to the simulation. With the brake at 1 both
sides are fully open whatever the yaw; at a part brake the yaw side opens further,
so the yaw stays visible below full. Cited use: the same surfaces open together as
air brakes (one source says they open symmetrically at low speed and on takeoff and
landing); that is a weaker citation than the yaw use, hence fitted.

### Gear (word 648c, +14 faces)

Seven plates, both windings each, retracting with the shared gear fraction
(angle = 90 degrees x (1 - gear), hidden at 0, the source pose at 1, so the
timing is the sim's own ramp). All hinge lines are fitted (the shape has none) and
every piece turns as one rigid body:

| Piece | Faces | Hinge (shape units) | Stow |
| --- | --- | --- | --- |
| Main door leaves | `438d 43a4`, `449e 44b5` | line along Y at z -10 (2 units above the leaf's top edge at -12), at x -71 and 69 | inboard, flat inside the closed well |
| Main wheel plates | `441b 443a`, `452c 454b` | line along Y at z -9, at x -65 and 62 | inboard, flat inside the closed well |
| Nose front leaf | `4153 416a` | lateral line (0, 71, -9), its top edge | aft, flat above the belly |
| Nose rear leaf | `41ae 41c5` | lateral line (0, 97, -5), its top edge | aft |
| Nose strut and wheel plate | `4597 45b6` | lateral line (0, 96, -5), its forward top corner; steers about a vertical line through (0, 92) | aft |

The mains' hinge is raised above the leaf's top edge because the belly well panel
is at z -12 (`3cc9`): a plate hinged on its own top edge would stow coplanar with
that skin. Raised, the stowed plates lie at z -10 and -9, inside the closed well.
Direction (inboard mains, aft nose) is fitted, as the Mirage 2000 and the Tornado (no
B-2 gear retraction direction was found in public sources). Probe (`dense-gear.csv`,
202 values plus the near-stow sample): rigid at every value, never below the gear-down
pose, grounded at gear 1 at exactly 33 units (11.0 ft, the inner main plates), every
piece at or above its own hinge height near stow, main plates inboard of their gear-down
x, the strut foot aft of its gear-down y, 203 lines, all passing.

**Wheel brakes.** None exist in the shape (plates only, no wheels, no brake
word) and the simulation has no separate wheel-brake visual for any aircraft, so
"brakes" here is the airbrake and speed brake above.

**Nose wheel (decision for John's information).** The nose strut's lowest point is
33 minus 31 = 2 units (0.67 ft) above the inner main plates', and the simulation
stands every aircraft on one clearance below its model origin (the gear low point,
11.0 ft for the B-2) with the pitch held level on the runway (`start_on_runway`
sets pitch 0). So the B-2 sits level (0.00 degrees) with the nose wheel 0.67 ft
(8 inches) off the pavement; the geometry that would put it down is 1.39 degrees
nose down over the 27.7 ft wheelbase. **Chosen: accept it as the retail sit**, no
fitted nose contact point. Why: a nose point is not a rig change, it is new ground
physics (a per-aircraft contact table or a pitch on the ground) that the one-clearance
contact model does not have for any of the 48 aircraft, it would touch
`SIMULATION_VERSION` and determinism for a 1.4 degree effect, and the retail game drew
the same plates over the same clearance. The frames (`ground-*`) show the nose wheel at
or just off the pavement with no sinking and no visible float at normal distance.

### Bay (word 6486): interim

The four hanging plates (`4209 4220 4264 427b 431a 4331 42bf 42d6`, both
windings, x -27, -3, 1, 26, y -10 to 48, z -12 to -25) are added to the shape and drawn
only while `State::bay` is above 0, dropping from the belly line (z -12, their top edge
stays) to the full 13 units as the bay fraction goes 0 to 1 (fitted, interim). Nothing
raises the bay fraction for the B-2 yet: `State::bay_available` is true only for the
F-22, so in play the plates never appear, and `--flight-bay` refuses the B-2. B2-5
replaces this with real bay cuts, doors and the magazine grid (and owns the
`bay_available`, signature and release-timing change); the frames of the interim pose
(`bay-*`) came from a temporary, reverted patch to the capture option. The probe drives
`State::bay` directly (control `bay`, 5 poses).

### Burner, flame, hook, flaps

The B-2 has no flame word, no nozzle faces and no afterburner thrust (`engine_material`
returns no nozzle, `outlet_count` is 0, `--flight-devices` refuses a non-zero fifth
fraction). The probe marks `exhaust` and `flaps` not applicable and requires that they
move nothing; `hook` is not applicable (no hook). Nothing breaks: the exhaust and flap
levers leave the geometry identical.

### Probe result

`--animation-probe .local/m3-aircraft/checks/B2-3/probe`: `animation probe B2.PT:
268 neutral faces, 0 required/check failures` (B2-2: the same four required failures,
unmapped). Required controls now pass: elevator (the elevons, 5 poses), aileron (the
elevons, 5), rudder (the drag rudders, 5), gear (22), brake (the speed brake, 5) and
bay (5). `flaps` and `exhaust` are not applicable, accepted explicitly (the flying wing
has no flaps and no burner), `hook` not applicable. Grids: `pitch-roll-combinations.csv`
(25 poses), `yaw-brake-combinations.csv` (25 poses), `dense-gear.csv` (202 values): all
pass, rigidity at most 5e-6 ft, hinge gap at most 1.6e-7 ft. Battery entry:
`flight-animation-b2` (expects the three CSVs).

Frames (`.local/m3-aircraft/frames/B2-3/`, png with ppm; `cap.sh` and `batch1.sh` to
`batch4.sh` record the commands; chase view, 200 kt, paused pose): `elevon-{neutral,
pitch-up,pitch-down,roll-right,roll-left}-{rear3q,rear,side}` and 3x crops
`crop3x-elevon-*-rear3q`; `drag-{neutral,yaw-left,yaw-right,speed-brake}-{rear3q,
rear,side}` and `crop3x-drag-*-rear3q`; `gear-{down,up,mid}-{under,side}`,
`gear-mid-{0.25,0.75}-under`; `bay-{open,half}-under`, `bay-open-gear-down-under`;
`ground-{side,nose-side,front3q,rear3q,side-low,front-low}` (a runway start, gear
down, level). Looks: the open drag rudder reads as a dark upper plate and a lighter
lower plate hinged off the trailing edge on the yaw side, both open together for the
brake; the elevon wedge leans visibly at pitch down and at roll; the gear folds
whole and inboard, hidden at 0.

## B-2 sensors, HUD and cockpit (B2-7, 2026-11-15)

Slice B2-7 (implementation mode, bomber batch A) made the B-2's retail cockpit family
work. It applies John's accepted decisions of 2026-11-15: **17**, use `B2.HUD` and the
`~B2*` art (opinionated), and **18**, keep the B-1B's five sensor rows with no pod body
drawn (the Tornado precedent). The earlier B-2 sections stay as written (B11): the
[source review](#b-2-source-review-2026-11-15) described the HUD field as empty and
the eye point as the fixed seat, and BMB-A1 left the glass failing closed and the
eye shared. This section records what the game now does. Changed: the B-2 glass
polygon (`hud_aperture.rs`), a per-aircraft pilot's eye table (`mirrors.rs`, with the
call sites that pass the aircraft type), and two tests (`hud_aperture.rs`, and the
retail-reading `crates/tore-import/tests/b2_cockpit.rs`). Nothing else in the
simulation changed. Frames are under `.local/m3-aircraft/frames/B2-7/`, logs under
`.local/m3-aircraft/checks/B2-7/`.

### Bindings (spec-derived names, opinionated binding)

| Item | Value |
| --- | --- |
| Record `hudName` | null pointer (as retail; TORE never reads it) |
| `AircraftId::hud()` | `B2.HUD`, which names the instrument frame `~b2_p`, read by `Hud::parse` as `~B2_P.PIC` (81 x 80) |
| Cockpit stem, shape stem | `B2`, `B2` |
| Forward art | `~B2H.PIC`, **1394 x 508** (not the 1280 x 490 of the older families, so the glass aperture is keyed to this size) |
| Other pictures of the family | `~B2.PIC` (581 x 214) and `~B2S.PIC` (1737 x 633): the same picture at the other two resolutions, which TORE does not read (as for every family) |
| Overlays | none: `~B2_LH.PIC`, `~B2_CH.PIC` and `~B2_RH.PIC` do not exist, so the B-2 is a no-overlay stem (BMB-A1) |
| Sensor rows (fixed) | `VIS340.SEE`, `F18R.SEE`, `F18.ECM`, `PAVESPK.SEE`, `AV8IR.SEE` |

The family is four pictures in `FA_1.LIB` (`~B2.PIC`, `~B2H.PIC`, `~B2S.PIC`,
`~B2_P.PIC`) and `B2.HUD` in `FA_2.LIB`. No other flyable identity uses the stem or the
HUD (the retail `AURORA.PT` and `TU160.PT` name `b2.HUD`, and neither is on the roster).
The new test pins all of it, and the record's `viewOffset` (0, 30, 60).

Views: front (the art with the HUD in its glass and the six small-layout windows), look
left and right 45 degrees (the art turns away with the look; with no overlay stem nothing
takes its place, as for the F-16 and the AC-130), 90 degrees and 60 degrees up (open
sky, no art), `--flight-view 3` (F2 Back, from the pilot's eye, below) and `--flight-view
4` (F3 Up, which does not use the eye). There is no down view. The instrument windows
draw with the `~B2_P.PIC` bezel; the ten pages (0 to 9) draw (frame
`instrument-pages-0-to-9`).

### The glass aperture (fitted, measured)

`~B2H.PIC` has no combiner glass: it is the open cockpit window of a large aircraft,
like the AC-130's picture that the B-52 flies. Its transparent pixels are three
separate panes, divided by the left diagonal strut and the right strut: the **main
window** (x 141 to 1021, y 91 to 455, 247,572 pixels), a left side pane (x 0 to 322,
y 90 to 440, 67,994 pixels) and a right side pane (x 1087 to 1393, y 91 to 418, 83,066
pixels). `hud_aperture::mask` had failed closed for the B-2 (BMB-A1's interim, an empty
mask, so no HUD drew). It now has a 20-vertex polygon along the main window's boundary,
a pixel outside it: the top beam's lower edge at y 90, the right strut's funnel down
to the monitor housing (x 1006 to 1021), the sloping console top (y 420 at x 540, 456
at x 300 and left) and the left strut, from (140, 456) to (398, 90). The opaque art trims
it, so the mask is exactly the transparent pixels inside: 246,986 pixels, 99.76 percent
of the main window, no opaque pixel, none in either side pane, one connected region. The
HUD symbols sit in the middle of the main window in the frames, well inside the polygon;
the side panes would matter only to symbols far off axis, and a decision to draw the HUD
there too (the whole-picture AC-130 choice) is a one-line change if John wants it. Two
tests pin it: a synthetic one (centre in; above the beam, both side panes and the console
out; an opaque pixel inside left out; any other picture size fails closed, and the F-117's
mask is still empty) and a retail-reading one (the same three panes by flood fill, the
exact coverage, no opaque pixel, no side pane, connected, at least 98 percent).

### How the HUD fits a bomber

`B2.HUD` is a resource of the B-1B and AURORA family (`TU160.PT` names it too), but TORE
reads from any HUD module only the primary colour, the instrument frame name and the
three window text colours (`tore-formats::hud`); the symbols are the one shared
drawn HUD. So the B-2's HUD is the same drawn HUD as every aircraft's, and what changes
for a bomber is what it is offered:

- It draws flight data (heading tape, speed and altitude boxes, flight path marker,
  pitch ladder), the weapon and its count (`31 MK-82`, `4 GBU-27`), `NO TARGET` with
  no designation, and the **bomb sights**: the level-bombing cue for the Mk 82 rows (`LVL`),
  and for the Paveway rows the laser box with `IN RNG`/`REL` and `LSR`, `TTI` and the
  seeker (frame `hud-slot3-gbu27-laser-sightpin`: `REL 0:14`, `LSR 4.5`, the box on the
  pinned point; without a pin the laser row reads `NO LASER`).
- It has no air-to-air weapons, so no lock circle or gun cue appears (the record has no
  gun and no missile). The radar and IR contacts show in the radar window, not as a HUD
  lock.
- The status column on the right of the glass shows `GEAR`, `BRAKE` (the speed brake,
  which opens both drag rudders), `FLAP` and `HOOK` as they apply (frame
  `hud-gear-down-brake` reads `GEAR` and `BRAKE`). `BAY` shows only while a bay is open
  and the aircraft is the F-22 (`State::bay_available`), so it does not show for the
  B-2 yet; B2-5 owns the bay and its cue.

### The eye point (fitted; a per-aircraft table now exists)

The B2-2 finding stands: the shared seat (7 ft up, 10 ft ahead of the origin, shape y 30
and z 21) is 10 to 17 ft behind the B-2's glass, under the spine skin. No per-aircraft
eye existed (`mirrors::pilot_eye` was one fixed offset), so B2-7 adds the smallest table
that leaves every other aircraft unchanged: `mirrors::eye_offset(Option<AircraftId>)`,
(up, forward) in feet, with the shared 7 and 10 for every identity and an unknown type,
and one entry, **the B-2: 10 ft up and 20 ft ahead**. It is fitted and labelled so.

Measured on the neutral `B2.SH` (1/3 ft a unit): the centre-line top surface rises from
z 20.7 (6.9 ft) at y 20 to the spine peak of z 23.8 (7.9 ft) at y 44 (14.7 ft ahead),
falls to 22.0 at y 60 (20 ft), where a steep face drops into the glass cavity, and the
windscreen runs from z 21 at y 64 (21.3 ft) to z 13.7 at y 80 (26.7 ft), then the nose
slope down to z 6.5 at y 88. The seat is at the windscreen's start, 2.7 ft above the
roof (z 22 at y 60), so F2 Back looks over the roof and the spine, not out of the
inside of the skin. A seat inside the skin (6 ft up) was drawn and rejected: Back shows
the inside faces of the spine as a grey blur (frame `f2-back-eye-inside-skin-rejected`).
The record's `viewOffset` (0, 30, 60), which no host code reads, gives the same
figure read at the shape's 1/3 ft a unit (30 up, 60 ahead = 10 ft, 20 ft); that
reading of the field's units is unproven and is not used by the code.

What reads the eye, and what changed (all pass the aircraft type now; none of it moves
for another aircraft):

| Reader | Takes the table |
| --- | --- |
| F2 Back in flight (`aircraft.rs`, own aircraft) | yes |
| F2 Back from another recorded or live aircraft (`flight_views.rs`: `Body` carries the type from its pose) | yes |
| Replay: the cockpit camera (`director.rs`), the HUD camera (`views.rs`) and the listening seat (`viewer.rs`) | yes, so the replay's Back view and its sound sit where flight's do |
| The mirror rear feed (`mirrors::camera`) | no: it stays on the shared seat. Only cockpit families with mirror seeds draw it, and `B2` has none (`--cockpit-sheet` prints `b2 NO MIRRORS FITTED`) |
| The forward cockpit (2D art), the look and zoom controls, the chase and fly-by views, F3 Up, the external camera | no: they never used the eye (the forward view looks from the origin) |
| Padlock and the target camera | no: they do not read the eye |

Tests: `mirrors::tests::only_the_b2_has_its_own_pilot_eye` (the table for every
identity), `flight_views::tests::back_from_a_b2_sits_at_its_own_seat`, and the existing
replay Back test now compares against the typed eye. Every other aircraft's Back eye is
unchanged (the replay and flight view tests pass). The mirror feed for the other
aircraft is untouched.

### Sensors (decision 18: kept as retail, no pod body)

`--sensor-summary` reads: `B2: radar F18R.SEE search 90 nmi 60x60 deg track 50 nmi
45x45 deg look-down 0 ADVANCED; infrared AV8IR.SEE search 15 nmi 60x60 deg track 15 nmi
45x45 deg; visual VIS340.SEE search 10 nmi 170x140 deg track 5 nmi 170x140 deg; jammer
F18.ECM LATE strength 0.30; signature radar 1 infrared 15`.

- **No pod body draws.** `PAVESPK.SEE` and `AV8IR.SEE` are fixed rows at (0, 0, 0)
  with flag `0x1008`, not stations, so no store pose is built for them (the Tornado
  precedent). The external frames show the wing undersides and the nose bare
  (`external-threequarter`, `external-side`, `external-front-under`, `external-under`).
- **The laser designator works through the fixed Pave Spike row.** With the GBU-27 row
  selected and `--combat-command sight-pin` the HUD reads `LSR 4.5`, `REL 0:14` and the
  target camera `PIN`, `ZOOM 3`, `LASE` with the range `4.5 NM` (frame
  `target-cam-gbu27-laser`). Without a designation the HUD reads `NO LASER`.
- **The target camera** (page 4) opens in the small layout (`SLEW`, `ZOOM 3`) and shows
  the pinned point when designating.
- **The IR view.** `--sensor-channel ir` puts the radar window on the `AV8IR.SEE`
  channel: `IR`, 10 nmi, with the target's contact (frame `ir-view-channel-ir-radar-page`).
- **The radar is the F-18's `F18R.SEE`, not the B-2's APQ-181**, kept as the record says.
  The modes it gives a bomber are the shared **air-to-air** ones: RWS search (to 90
  nmi) and TWS with a fire-control track (to 50 nmi), both in the scope at 5, 10, 25, 50,
  100 and 150, plus the IR channel (M cycles them, Y toggles history). There is **no
  air-to-ground mapping mode** in the shared radar (`docs/radar.md`: air-to-ground
  radar awaits ground weapons), so the B-2's strike work is the targeting sight and
  the laser, which the fixed Pave Spike row provides. Frame `radar-mfd-livefire-track`
  reads `TWS 10 ... TRACK`.
- **The RCS page shows the stealth signature.** Page 0 reads `SIG 1` with the exposure
  contour a dot at the centre of the 50 nmi scope, against `SIG 154` and an oval for
  an F/A-18 in the same capture (frames `rcs-page-b2-sig1-crop3x` and
  `rcs-page-f18-sig100-crop3x-comparison`; the F-18's record says 100 and the page
  shows the figure with its aspect and loaded stores). The record's radar signature of 1 and infrared 15 are as
  retail; the RWR and the stealth model are not in this slice.

### Checks

`cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets --locked
-- -D warnings` clean; `cargo test --locked --workspace --no-fail-fast` (counts in the
plan's Session log); `--headless-flight 600 --aircraft b2` (same log); `check_docs`
passes. Frames: `cockpit-front-view` (HUD inside the glass), `cockpit-look-left-45`,
`-right-45`, `-up-60`, `cockpit-view-back-f2` (and the rejected inside-skin eye),
`cockpit-view-up-f3`, `hud-gear-down-brake`, `hud-slot3-gbu27-laser-sightpin`,
`hud-slot3-gbu27-no-laser`, `target-cam-gbu27-laser` and `crop3x-target-cam`,
`radar-mfd-livefire-track` and `crop3x-radar-track`, `ir-view-channel-ir-radar-page`
and `crop3x-ir-channel`, `rcs-page-live` and the two RCS crops, `instrument-pages-0-to-9`,
the four `external-*` views, and `pre-cockpit-front` (before: no HUD).

Not done: the bay cue and open-bay signature (B2-5), the RWR, any B-2-specific
radar or ground-mapping mode (a sensor change, John's call), a battery scenario (the
Mirage 2000 and Tornado cockpit slices added none; the closest, `render-b2-skin` and
`flight-animation-b2`, were rerun).

Provenance: sensor rows, HUD module, panel names and cockpit art are spec-derived
(retail); the binding is opinionated (John, 2026-11-15, decisions 17 and 18); the glass
polygon and the eye table are fitted, agent decisions 2026-11-15.
