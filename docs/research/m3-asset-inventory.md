# Milestone 3 asset inventory

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** A status census, not a plan and not an
> acceptance gate. Delivered behaviour stays in [the feature matrix](../features.md)
> and [the parity plan](../parity-plan.md); player-visible behaviour is specified
> in [docs/spec/](../spec/).

Research mode, 2026-11-06, requested by John at the start of Milestone 3
(missions and campaigns). Every object record in the supplied Fighters Anthology
installation's twelve EALIB archives (`FA_1`, `FA_2`, `FA_3`, `FA_4B/C/D`,
`FA_7`, `FA_10/10B`, `FA_11/11B`, `swpatch.lib`, deduplicated against the
`SETUP.ESA` copies) is graded on what TORE does with it today.

## Grading

Each column is **Done**, **Partial**, **None** or **N/A**, with a short evidence
note per row.

- **Imported:** the record and its shape load, and the object can appear in game.
- **Animations:** moving parts and visual states that apply to the object (gear,
  surfaces, rotors, turrets, antennas, plumes, dispense, wreck/ruin).
- **AI:** TORE's AI flies, operates or employs it.
- **Logic:** systems behaviour that applies (flight model, sensors, guidance,
  damage, runway services, fuel).

Grades come from reading docs and code, plus scans of the extracted retail
layouts; no build, probe or game run was made for this census. Retail comparison
is unavailable, so Done means TORE behaviour exists and is documented, not that
it is proven identical to retail. Rows marked "unverified" in their notes are the
first ones to confirm.

## Summary

| Object type | Records | Imported D/P/N/NA | Animations D/P/N/NA | AI D/P/N/NA | Logic D/P/N/NA |
|---|---:|---|---|---|---|
| Aircraft (PT) | 145 | 64 / 0 / 81 / 0 | 31 / 33 / 81 / 0 | 26 / 10 / 109 / 0 | 25 / 39 / 81 / 0 |
| Ordnance (JT) | 135 | 111 / 12 / 12 / 0 | 105 / 11 / 19 / 0 | 107 / 2 / 22 / 4 | 110 / 6 / 19 / 0 |
| Equipment (SEE/ECM/GAS) | 85 | 53 / 2 / 30 / 0 | 21 / 0 / 15 / 49 | 24 / 0 / 12 / 49 | 51 / 3 / 31 / 0 |
| Surface units (NT) | 84 | 82 / 2 / 0 / 0 | 13 / 71 / 0 / 0 | 39 / 19 / 3 / 23 | 71 / 12 / 0 / 1 |
| Static objects (OT) | 170 | 157 / 11 / 2 / 0 | 72 / 69 / 3 / 26 | 0 / 0 / 0 / 170 | 140 / 28 / 2 / 0 |
| **Total** | **619** | **467 / 27 / 125 / 0** | **242 / 184 / 118 / 75** | **196 / 31 / 146 / 246** | **397 / 88 / 133 / 1** |

## What remains, by object type

- **Aircraft.** 36 of 145 are flyable by players and AI (13 original roster, 23
  variety). 28 more are parked scenery only (no AI, static pose). 81 are decoded
  in [`fa-aircraft.csv`](../formats/fa-aircraft.csv) and nothing else. Variety
  rotorcraft, VTOL, AWACS and the AC-130 are Partial (blade feathering, fitted
  powered-lift tuning, no contact sharing).
- **Ordnance.** The W1 to W5 passes cover most air-launched stores. Remaining:
  AA6, AA9, PL7, PL10 (profiled, nothing loads them), AEMP1, ASROC, hidden gun
  pods (ADEN, BK27, GAU13, GSH6_23) and guns 20MM_4 and ADENIN, twelve
  decoded-only surface records, and presentation gaps (no bomblet dispense, no
  visible MK82AIR retarder, no GBU28 penetration).
- **Equipment.** Sensors and ECM are gated to reviewed roster aircraft, so 14
  radars, 12 ECM suites and 3 sensors carried only by unported aircraft are None.
  Designator pods PAVESPK and AAS38 have no drawn body. Tanks use a fitted body.
- **Surface units.** All load and the 2026-10-10 surface AI covers detection,
  firing, batteries and emitters. Animation is the gap: only KRIVAK and SOVR
  turrets and four antennas are retail parts, nothing elevates, no wheel, track or
  wake motion, and only five Quick Mission templates move units. Carrier deck
  operations and the deck tractors are open (M3 scope).
- **Static objects.** Nearly all load and buildings, bridges and runways have
  damage and services. Gaps: TREE1/TREE2 have no shape in the media, about 69
  types use a fitted ruin, tower and HAWK antenna spin is not built, and hit
  points for pavement, rocks, flags and carrier islands are unverified. Ten types
  are never placed by the layouts scanned.
- **Mission side.** Campaign `.M` scripting, `.MC` code modules, campaign `.MT`
  sections, `.SEQ`, `.CAM`, campaign `.TXT`, pilot records and saves are not
  started; see [the last section](#mission-side-and-other-formats).

## Documentation found stale

- [ROADMAP.md](../ROADMAP.md) W6 lists SS-N-9 as held, but `SSN9.JT` fires from
  surface units (`combat/missiles.rs`, `SURFACE_TO_SURFACE`).
- The W2 row lists AIM7, AIM7E, AIM9B and AA10 as remaining, but they are in
  `SUPPORTED_WEAPONS` (`combat/loadout.rs`).
- [ai-experience.md](../spec/ai-experience.md) "Currently ported aircraft" says
  twelve; `AircraftId::ALL` holds 36 (13 roster including F22N, plus 23 variety).


## Aircraft (PT)


### Mechanisms found

Ported aircraft are the 13 original-roster `AircraftId` entries (F18, RAFALE, F14, A4E, F31, MIG29, SU27, MIG21, SU25, MIG23, SU35, F22, F22N) with exact PT identity, own rig, cockpit, sensors, stores and hybrid flight (docs/spec/additional-aircraft.md, roster-aircraft.md, aircraft-animation.md). "Variety" is the 23-aircraft batch (docs/spec/aircraft-variety.md): same `AircraftId::ALL` (36 entries, `ai_flyable()` is true for all), all human-playable and AI-flyable, rigs reviewed on CPU (docs/baselines/aircraft-animations.md), rotorcraft/VTOL on the fitted powered-lift physics (powered-lift-flight.md, rotor-presentation.md). AI for all 36 is exercised by `--ai-roster-probe-ticks` (crates/tore-app/src/ai_roster_probe.rs) and binds to the generic fighter/strike family plus `strike_class.rs` (Heavy/Attack/Helicopter/Gunship); overall AI is documented as partial (no campaign routes). "Dummy" (docs/spec/dummy-aircraft.md) is not a record type: it is an opinionated Quick Mission skill option (400 kt constant heading) applied to whatever aircraft the wing already uses, so it adds nothing for unported records. Separately, 28 more PTs are **parked-only**: Quick Mission ground-target templates place them as simulated aircraft on the ground, read through `crates/tore-formats/src/parked_aircraft.rs` (OBJECT block only, no flight/NPC/hardpoints), drawn gear-down, hit points/damage and burnt hulk, no AI actor (docs/spec/surface-defenses.md "Parked aircraft", docs/formats/objects-and-shapes.md "Parked aircraft gear"). The other 81 records (drones, ASTOVL/F-111/B-52 and similar, variants) are only decoded in docs/formats/fa-aircraft.csv; nothing loads them. Grading: Done/Partial reflects docs and code; no retail comparison exists, all animation/flight fits are fitted not original. Logic Partial for rotorcraft/VTOL/AWACS/AC-130 reflects stated open items (human tuning, contact sharing, gunner view).

| Resource | Name | Imported | Animations | AI | Logic | Notes |
|---|---|---|---|---|---|---|
| A4E.PT | A- 4E Skyhawk | Done | Partial | Done | Done | ported roster (AircraftId); forward flap-branch strips unmapped; no burner |
| F14.PT | F- 14D Tomcat | Done | Done | Done | Done | ported roster (AircraftId); wing sweep, hook, brakes animate; AI probe |
| F18.PT | F/A- 18D Hornet | Done | Done | Done | Done | ported roster (AircraftId); gear/flaps/surfaces/hook animate; AI probe x4 skills |
| F22.PT | F- 22A Raptor | Done | Done | Done | Done | ported roster (AircraftId); main bays animate; side-bay sequencing unknown |
| F22N.PT | F- 22N Raptor | Done | Done | Done | Done | ported roster; F22A rig + own hook; AI flies via ALL |
| F31.PT | X-31 EFM | Done | Done | Done | Done | ported roster (AircraftId); vector paddles/plume fitted; AI probe |
| MIG21.PT | MiG-21 Fishbed | Done | Done | Done | Done | ported roster (AircraftId); ventral brake fitted; AI probe; also parkable |
| MIG23.PT | MiG-23 Flogger-B | Done | Done | Done | Done | ported roster (AircraftId); visual wing sweep (no aero effect); AI probe |
| MIG29.PT | MiG-29 Fulcrum-C | Done | Done | Done | Done | ported roster (AircraftId); attachment-corrected; AI probe; also parkable |
| RAFALE.PT | Rafale C | Done | Done | Done | Done | ported roster (AircraftId); attachment-corrected rig; AI probe x4 skills |
| SU25.PT | Su-25 Frogfoot-A | Done | Done | Done | Done | ported roster (AircraftId); no burner; record corrected; AI probe |
| SU27.PT | Su-27 Flanker-B | Done | Done | Done | Done | ported roster (AircraftId); attachment-corrected; AI probe |
| SU35.PT | Su-35 | Done | Done | Done | Done | ported roster (AircraftId); canards animate; null HUD ref, own cockpit chosen |
| A10.PT | A-10 Thunderbolt | Done | Done | Done | Done | variety import (AircraftId); rig reviewed; GAU-8, Pave Spike; AI flyable |
| A310.PT | Airbus 310 | Done | Done | Done | Done | variety import (AircraftId); controls+gear fit; unarmed, no radar; AI hit/evade only |
| A7.PT | A- 7E Corsair II | Done | Done | Done | Done | variety import (AircraftId); surface repair, hook; AI flyable |
| AC130.PT | AC-130U Spectre | Done | Partial | Partial | Partial | variety import (AircraftId); linked guns done; hook geometry unlocated; AI orbit unverified |
| AH64.PT | AH-64 Apache | Done | Partial | Partial | Partial | variety import (AircraftId); rotor/tilt animate; blade feathering missing; fitted physics |
| AV8.PT | Av-8B Harrier II | Done | Done | Partial | Partial | variety import (AircraftId); nozzles animate; AI VTOL law fitted; hover tuning pending |
| B747.PT | Boeing 747 | Done | Done | Done | Done | variety import (AircraftId); controls+gear; unarmed airliner; AI hit/evade |
| C130.PT | C-130 Hercules | Done | Done | Done | Done | variety import (AircraftId); props spin, controls/gear fit; Heavy AI, no strike |
| CH47.PT | CH-47 Chinook | Done | Partial | Partial | Partial | variety import (AircraftId); tandem rotors animate; feathering missing; unarmed AI escort |
| E2.PT | E-2C Hawkeye (AIR) | Done | Done | Partial | Partial | variety import (AircraftId); dome spins, hook; AWACS contact sharing not built |
| E3.PT | E-3 AWACS Sentry (AIR) | Done | Done | Partial | Partial | variety import (AircraftId); dome spins; AWACS contact sharing not built; generic AI |
| F104.PT | F-104N Starfighter | Done | Done | Done | Done | variety import (AircraftId); controls+hook fit; AI flyable |
| F15.PT | F- 15C Eagle | Done | Done | Done | Done | variety import (AircraftId); control/device fit; AI flyable |
| F16C.PT | F- 16C Falcon | Done | Done | Done | Done | variety import (AircraftId); control fit; Pave Spike; AI flyable |
| F4.PT | F-  4G Wild Weasel Phantom | Done | Done | Done | Done | variety import (AircraftId); F-4G; rig reviewed, no hook; AI flyable |
| F4B.PT | F-  4B Phantom II | Done | Done | Done | Done | variety import (AircraftId); F-4 family rig; hook; SUU-16 pod option; AI flyable |
| F4E.PT | F-  4E (Desert) Phantom | Done | Done | Done | Done | variety import (AircraftId); F-4 rig, no hook; also parkable; AI flyable |
| F4J.PT | F-  4J Phantom II | Done | Done | Done | Done | variety import (AircraftId); F-4 rig, hook; SUU-16 pod; AI flyable |
| IL76.PT | IL-76 Mainstay (AIR) | Done | Done | Partial | Partial | variety import (AircraftId); dome spins; AWACS sharing not built; generic AI |
| MI24.PT | Mi-24 Hind-D | Done | Partial | Partial | Partial | variety import (AircraftId); rotor+gear fit; feathering missing; fitted physics |
| MIG17F.PT | MiG-17F Fresco | Done | Done | Done | Partial | variety import (AircraftId); rig reviewed; fitted fuel; drop-tank climb fails (retail too) |
| V22.PT | V-22 Osprey | Done | Done | Partial | Partial | variety import (AircraftId); nacelles tilt 0-97.5deg, props; fitted physics, AI conversion |
| YAK141.PT | Yak-141 Freestyle-A | Done | Done | Partial | Partial | variety import (AircraftId); nozzle animates; AI VTOL fitted; Kiev parking unresolved |
| A37.PT | A-37 Dragonfly | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| AH1.PT | AH- 1 Cobra | Done | Partial | None | Partial | parked target only; skids always drawn; rotors static |
| F16E.PT | F- 16A (Desert) Falcon | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| F5EE.PT | F-  5E (Desert) Tiger | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| F5EV.PT | F-  5E (Snow) Tiger | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| J7E.PT | J-7E Fishbed | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| KA50.PT | Ka-50 Hokum-A | Done | Partial | None | Partial | parked target only; skids always drawn; rotors static |
| M2000.PT | Mirage 2000 | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| M2000E.PT | Mirage (Desert) 2000 | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| M25.PT | MiG-25 Foxbat | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| M5.PT | Mirage 5 | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| MF1.PT | Mirage F1 | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| MI17.PT | Mi-17 Hip | Done | Partial | None | Partial | parked target only; skids always drawn; rotors static |
| MIG21F.PT | MiG-21F Fishbed-C | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| MIG27.PT | MiG-27 Flogger-J | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| MIG29M.PT | MiG-29M | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| MIG29V.PT | MiG-29 (Snow) Fulcrum-C | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| MIG31.PT | MiG-31 Foxhound | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| MR3.PT | Mirage III | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| MR3E.PT | Mirage (Desert) III | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| Q5.PT | Q-5 Fantan | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| RAFALEF.PT | Rafale M | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| SFR.PT | Super Frelon | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| SPE.PT | Super Etendard | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| SU24.PT | Su-24 Fencer-D | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| SU27V.PT | Su-27 (Snow) Flanker-B | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| SU34.PT | Su-34 Platypus | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| SU7.PT | Su- 7 Fitter | Done | Partial | None | Partial | parked target only (OBJECT block+shape); gear-down static; no AI/flight |
| A1.PT | A- 1 Skyraider | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| A6.PT | A- 6E Intruder | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| A6V.PT | A- 6 Intruder | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| A7V.PT | A- 7A Corsair II | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| AF1.PT | Air Force One | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script liner.BI |
| ALPH.PT | Alpha Jet | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| AN74.PT | An-74 Coaler | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script large.BI |
| ASTOVL.PT | X-32 ASTOVL | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| ASTOVLE.PT | X-32 (Desert) ASTOVL | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| ASTOVLF.PT | X-32 (Forest) ASTOVL | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| ASTOVLV.PT | X-32 (Snow) ASTOVL | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| ATL.PT | Atlantique 2 (AIR) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script liner.BI |
| AURORA.PT | Aurora Spy Plane | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script large.BI |
| B1.PT | B- 1B Lancer | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| B2.PT | B- 2 Spirit | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script b.BI |
| B52.PT | B-52 Stratofortress | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script b.BI |
| B707.PT | Boeing 707 | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script liner.BI |
| BLIMP.PT | Anti-Stealth Blimp | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script large.BI |
| BUC.PT | BAe Buccaneer | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| C17.PT | C- 17 Globemaster | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script large.BI |
| C5.PT | C-  5B Galaxy | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script large.BI |
| CKUO.PT | AIDC Ching-Kuo | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| CMCHE.PT | RAH-66 Commanche | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script h.BI |
| DRAK.PT | Saab Draken | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| DRNA1.PT | Attack Drone 1 | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| DRNA2.PT | Attack Drone 2 | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| DRND1.PT | Decoy Drone 1 | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| DRND2.PT | Decoy Drone 2 | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| DRNR1.PT | Recon Drone 1 (GRD) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| DRNR2.PT | Recon Drone 2 (GRD) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| DRNR2E.PT | Recon (Desert) Drone 2 (GRD) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| DRNR3.PT | DarkStar (GRD) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| E2000.PT | EuroFighter 2000 | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| E8.PT | E-8C J-STARS (GRD) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script liner.BI |
| EA6.PT | EA-6B Prowler | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F105.PT | F-105 Thunderchief | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F111.PT | F-111F Aardvark | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F117.PT | F-117A Nighthawk | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f117.BI |
| F15E.PT | F- 15E Strike Eagle | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F15J.PT | F- 15J Eagle | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F16.PT | F- 16A Falcon | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F16F.PT | F- 16A (Forest) Falcon | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F18C.PT | F/A- 18C Hornet | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F260.PT | SIAI-Marchetti SF.260 | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F29.PT | X-29 FSW | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F31E.PT | X-31 (Desert) EFM | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F31F.PT | X-31 (Forest) EFM | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F31V.PT | X-31 (Snow) EFM | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F5E.PT | F-  5E Tiger | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| F8J.PT | F-  8J Crusader | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| FSX.PT | Fighter Support-X | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| GAZ.PT | Gazelle | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script h.BI |
| GAZE.PT | Gazelle (Desert) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script h.BI |
| GAZV.PT | Gazelle (Snow) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script h.BI |
| GRIPEN.PT | Saab Gripen | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| HWK.PT | BAe Hawk | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| IL96.PT | IL-96 | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script liner.BI |
| JAG.PT | SEPECAT Jaguar | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| KC135.PT | KC-135 Stratotanker | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script liner.BI |
| KFIR.PT | IAI Kfir | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| LYNX.PT | Westland Lynx | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script h.BI |
| MTF1.PT | Mitsubishi F-1 | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| P3.PT | P-3C Orion (AIR) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script liner.BI |
| RAFALEE.PT | Rafale (Desert) C | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| SEAHAR.PT | Sea Harrier FRS.2 | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| SFRV.PT | Super Frelon (Snow) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script h.BI |
| SH3A.PT | SH-3A Sea King | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script h.BI |
| SH60.PT | SH-60 Seahawk | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script h.BI |
| STRK.PT | BAe Strikemaster | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| SU33.PT | Su-33 Flanker-D | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| SU35B.PT | Su-35 (Forest) | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| TIGRE.PT | HAC Tiger | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script h.BI |
| TOR.PT | Tornado IDS | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| TU160.PT | Tu-160 Blackjack-A | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script b.BI |
| TU26.PT | Tu- 26 Backfire-C | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script b.BI |
| TU95.PT | Tu- 95 Bear-H | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script b.BI |
| UH60.PT | UH-60 Black Hawk | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script h.BI |
| VIG.PT | Saab Viggen | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script f.BI |
| ~BGUN.PT | Base Defense Gun | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script b.BI |
| ~MOTH.PT | Atomic Moth | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script moth.BI |
| ~QUE.PT | Quetzalcoatlus | None | None | None | None | catalog-decoded only; no shape/AI loaded; ai_script moth.BI |

### Counts (145 records)

| Status | Imported | Animations | AI | Logic |
|---|---|---|---|---|
| Done | 64 | 31 | 26 | 25 |
| Partial | 0 | 33 | 10 | 39 |
| None | 81 | 81 | 109 | 81 |
| N/A | 0 | 0 | 0 | 0 |

## Ordnance and equipment (JT, SEE, ECM, GAS)


Research mode, read-only. Grades are from docs and code only; "unverified" marks anything not confirmed.

### Mechanisms

- **Exposure gate.** A weapon is selectable and flyable only if listed in `SUPPORTED_WEAPONS` (`crates/tore-sim/src/combat/loadout.rs:765`) or the Cheat-only novelty list (Boulder, Moose). Surface-fired records live in `SURFACE_SUPPORTED`, `SURFACE_INFRARED`, `SURFACE_TO_SURFACE` (`crates/tore-sim/src/combat/missiles.rs:107`) plus the AAA tuning table (`combat/surface_guns.rs`, `docs/spec/surface-defenses.md`). Hidden records stay decoded in `docs/formats/fa-weapons.csv` (135) and `docs/features.md` lists them: ADEN, BK27, GAU-13, GSh-6-23, AA-6, AA-9, PL-7, PL-10, AEMP-1.
- **Missiles.** Typed guidance profiles (`crates/tore-formats/src/missile_data.rs`, 59 profiles), finite-energy flight, seekers and decoys (`docs/spec/missiles.md`, `missile-energy.md`, `docs/baselines/missiles.md`). Motor smoke and flame, fin roll and pylon drawing are shared (`docs/features.md`, `docs/spec/stores-on-pylons.md`).
- **Air-to-ground.** The A-G records (40 in twelve classes, the two Cheat-only novelty bombs included) were flown headless on two or more carriers (`docs/baselines/air-to-ground.md`, `docs/spec/air-to-ground.md`). Retarded bombs open their fins in flight (`store_sizes.rs`); cluster bombs give retail splash plus 28 presentation-only flashes; other retarders are drag only.
- **AI.** Strike AI covers bombs, rockets, JDAM, laser stores, Mavericks, radar AGMs, HARM and AT-2 (`docs/spec/ai-strike.md`); guns use the shared burst policy (`docs/spec/ai-gun-employment.md`); surface AI fires SAMs, AAA and SS-N-9 (`surface-defenses.md`). `docs/ROADMAP.md` W1-W6 rows are partly stale: SS-N-9, and the W2 AIM-7, AIM-7E, AIM-9B, AA-10 items, are built.
- **Equipment.** One sensor component serves all 36 roster aircraft plus F/A-XX. It refuses unreviewed radar or ECM records (`crates/tore-sim/src/sensors/profile.rs:106,255,315`), so only records carried by roster aircraft are graded Done. Laser/FLIR sights: `combat/sight.rs`; chaff/flare release, visuals, decoy odds: `docs/spec/countermeasures.md`; tanks: `external-fuel-tanks.md`, `jettison.md` (fitted tank body, no retail shape).
- **Roster basis.** "Roster" means the 36 retail aircraft in `docs/spec/aircraft-variety.md` and `roster-aircraft.md`; carriers were computed from `docs/formats/fa-aircraft.csv` default stores only (stations can allow more).
- **Grade rules.** Imported: Done = reachable in game; Partial = decoded and profiled or hooked but hidden; None = decoded only. Animations/AI use N/A where nothing applies.


### Ordnance (JT)

#### Air-to-air missile (22)

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| AA10.JT | AA-10 Alamo | Done | Done | Done | Done | Live as R-27R identity; R-27 variants derived; fit residual exception |
| AA11.JT | AA-11 Archer | Done | Done | Done | Done | Live; Su-27/Yak-141; IR all-aspect profile |
| AA11B.JT | AA-11B Backfire | Done | Done | Done | Done | Live; Su-35; wide-gimbal IR profile |
| AA12.JT | AA-12 Adder | Done | Done | Done | Done | Live; active radar, 5 nmi pitbull; fit residual exception |
| AA2.JT | AA-2D Atoll | Done | Done | Done | Done | Live; MiG-21; IR profile; fit residual exception |
| AA6.JT | AA-6 Acrid | Partial | Partial | None | Partial | Hidden catalog-only; profile and energy fit exist, no live path |
| AA8.JT | AA-8 Aphid | Done | Done | Done | Done | Live; MiG-29, Su-25; R-60M role; fit residual exception |
| AA9.JT | AA-9 Amos | Partial | Partial | None | Partial | Hidden catalog-only; profile and energy fit exist, no live path |
| AAML.JT | AAM-L Anti-AWACS | Done | Done | Done | Done | Live; Su-35; active radar; no special anti-AWACS AI rule found (unverified) |
| AEMP1.JT | AEMP-1 EM Pulse | Partial | Partial | None | Partial | Hidden; EM pulse effect (radius, no damage) undefined; nothing loads |
| AIM120.JT | AIM-120 AMRAAM | Done | Done | Done | Done | Live; active radar, 5 nmi pitbull; 26 carriers |
| AIM54C.JT | AIM-54C Phoenix | Done | Done | Done | Done | Live; F-14; 10 nmi pitbull; fit residual exception |
| AIM7.JT | AIM-7 Sparrow | Done | Done | Done | Done | Live; semi-active radar, support loss modelled; fit residual |
| AIM7E.JT | AIM-7E Sparrow | Done | Done | Done | Done | Live; semi-active radar; short motor; fit residual exception |
| AIM9B.JT | AIM-9B Sidewinder | Done | Done | Done | Done | Live; IR, early seeker; F-4B/J |
| AIM9M.JT | AIM-9M Sidewinder | Done | Done | Done | Done | Live; IR; most common AAM (27 carriers) |
| AIM9X.JT | AIM-9X Sidewinder | Done | Done | Done | Done | Live; wide-angle IR; F-22/X-31 |
| MICA.JT | Mica | Done | Done | Done | Done | Live; Rafale; MICA-EM radar role, no IR variant added |
| PL10.JT | Chinese PL-10 | Partial | Partial | None | Partial | Hidden catalog-only; radar-concept PL10-R, no live path |
| PL7.JT | Chinese PL-7 | Partial | Partial | None | Partial | Hidden catalog-only; guarded motor fit exists, no live path |
| R530.JT | Super 530 | Done | Done | Done | Done | Live; Rafale; S530D semi-active |
| R550.JT | R-550 Magic 2 | Done | Done | Done | Done | Live; Rafale; IR Magic 2 |

#### Air-to-ground missile (14)

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| AGM45.JT | AGM-45B Shrike | Done | Done | Done | Done | Shrike passive-emitter; HARM page; AI SEAD uses it |
| AGM65A.JT | AGM-65A Maverick | Done | Done | Done | Done | Flown to 8 nmi after FX-M fix; kills tank only after 20/200 left |
| AGM65G.JT | AGM-65G Maverick | Done | Done | Done | Done | Seeker view, CUED/BORE, flown on A-10/F-18/F-16/AV-8B |
| AGM84A.JT | AGM-84A Harpoon | Done | Done | Done | Done | Harpoon active radar vs ship; leaves Krivak 150/500 |
| AGM84E.JT | AGM-84E SLAM | Done | Done | Done | Done | SLAM IR; flown CUED 12 nmi and BORE |
| AGM88.JT | AGM-88 HARM | Done | Done | Done | Done | HARM; keeps homing after radar shutdown; AI SEAD fires |
| AM39.JT | AM-39 Exocet | Done | Done | Done | Done | Exocet active radar vs ship; leaves Krivak 200/500 |
| AS14.JT | AS-14 Kedge | Done | Done | Done | Done | Laser missile; own or buddy spot; AI lases or goes home |
| AS15.JT | AS 15 | Done | Done | Done | Done | Supported radar; manual calls it laser; flown MiG-29/Su-27 |
| AS16.JT | AS-16 Kickback | Done | Done | Done | Done | Kickback active radar; flown on Su-25/Su-35 |
| AS30.JT | AS 30L | Done | Done | Done | Done | Laser missile; own or buddy spot; AI lases or goes home |
| AS7.JT | AS-7 Kerry | Done | Done | Done | Done | Supported radar; needs A-G track held; flown MiG-23 |
| AT12.JT | AT-12 Swinger | Done | Done | Done | Done | Laser missile; hits but does not kill T-80 |
| AT2.JT | AT-2 Swatter | Done | Done | Done | Done | Command guided from launcher; Mi-24 AI hover pop-up; no kill on T-80 |

#### Bomb (24)

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| CBU87.JT | CBU-87/B Cluster | Done | Partial | Done | Done | Retail wide splash 2,200 ft; 28 burst flashes only, no dispenser/bomblet flight |
| CBU89.JT | CBU-97/B Cluster | Done | Partial | Done | Done | Retail wide splash 2,500 ft; 28 burst flashes only, no dispenser/bomblet flight |
| FAB1000.JT | FAB-1000 M62 | Done | Done | Done | Done | Free-fall; flown Su-25/F-4E |
| FAB250.JT | FAB-250 M62 | Done | Done | Done | Done | Free-fall; flown Su-25/MiG-23/MiG-21 |
| FAB500.JT | FAB-500 M62 | Done | Done | Done | Done | Free-fall; flown Su-25/A-10 |
| GBU10.JT | GBU-10E Paveway II | Done | Done | Done | Done | Laser bomb; own/buddy spot, blind when spot ends; flown 2+ carriers |
| GBU10A.JT | GBU-10 Paveway I | Done | Done | Done | Done | Laser bomb; own/buddy spot, blind when spot ends; flown 2+ carriers |
| GBU28.JT | GBU-28/B | Done | Done | Done | Partial | Laser guidance flown; no penetration effect found (unverified) |
| GBU29.JT | GBU-29 JDAM | Done | Done | Done | Done | JDAM flies to stored release point; misses movers; level-T bar built |
| GBU29P.JT | GBU-29/P JDAM | Done | Done | Done | Done | JDAM flies to stored release point; misses movers; level-T bar built |
| GBU30.JT | GBU-30 JDAM | Done | Done | Done | Done | JDAM flies to stored release point; misses movers; level-T bar built |
| MK20.JT | Mk 20 Rockeye II | Done | Partial | Done | Done | Rockeye; splash 1,600 ft; 28 burst flashes only, no dispenser/bomblet flight |
| MK82.JT | MK-82 Slick | Done | Done | Done | Done | Free-fall, fitted bomb model; CCIP/LVL sights; 14 default carriers |
| MK82AIR.JT | MK-82 AIR 7-Pack | Done | Partial | Done | Done | 7-pack retarder is drag only; no visible retarder deploy (unverified) |
| MK82HD.JT | MK-82 Snakeye | Done | Done | Done | Done | Snakeye; fins closed on pylon, open in flight; retarder drag |
| MK82P.JT | MK-82HD 6-Pack | Done | Done | Done | Done | 6-pack Snakeye variant (shape snake.SH); retarder drag; one-tick release |
| MK82P3.JT | MK-82 3-Pack | Done | Done | Done | Done | 3-pack leaves in one tick; flown A-7/F-16C |
| MK84.JT | MK-84 Slick | Done | Done | Done | Done | Free-fall; flown F-15/A-10 |
| PAVEWA3.JT | GBU-27 Paveway III | Done | Done | Done | Done | Laser bomb; own/buddy spot, blind when spot ends; flown 2+ carriers |
| PAVEWAY.JT | GBU-10E Paveway | Done | Done | Done | Done | Laser bomb; own/buddy spot, blind when spot ends; flown 2+ carriers |
| RBK250.JT | RBK-250 Cluster | Done | Partial | Done | Done | Splash 1,500 ft; 28 burst flashes only, no dispenser/bomblet flight |
| RBK500.JT | RBK-500 Cluster | Done | Partial | Done | Done | Splash 2,500 ft; 28 burst flashes only, no dispenser/bomblet flight |
| ~BLDR.JT | Boulder | Done | Done | N/A | Done | Cheat-loading only; falls as free-fall bomb |
| ~MOOSE.JT | Moose | Done | Done | N/A | Done | Cheat-loading only; falls as free-fall bomb |

#### Rocket (5)

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| B13.JT | B-13L Rocket Pod | Done | Done | Done | Done | 5-rocket pod; flown Su-25/A-10/MiG-23; rocket smoke |
| B8.JT | B-8M1 Rocket Pod | Done | Done | Done | Done | 19-rocket pod; flown Su-25/MiG-23; rocket smoke |
| LAU10.JT | LAU-10 Rocket Pod | Done | Done | Done | Done | 4-rocket pod; one per press; rocket smoke |
| LAU61.JT | LAU-61 Rocket Pod | Done | Done | Done | Done | 19-rocket pod; 4/s held; pipper; flown A-4E/AH-64; rocket smoke |
| ~MOTHB.JT | Moth Balls | None | None | N/A | None | Novelty joke record; nothing loads it |

#### Gun round (aircraft, gunship, pod) (24)

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 20MM_4.JT | Colt 20mmx4 | Partial | None | None | None | Hidden; not on any roadmap list; no carrier in roster |
| ADEN.JT | Aden 30mm Cannon Pod | Partial | None | None | None | Hidden; W1 remaining: cannon pods nothing loads |
| ADENIN.JT | Aden 30mm Cannon | Partial | None | None | None | Hidden; internal Aden gun, no roster carrier |
| BK27.JT | 27-mm Mauser BK27 Cannon | Partial | None | None | None | Hidden; W1 remaining: cannon pods nothing loads |
| C_105.JT | 105mm Cannon | Done | Done | Done | Done | AC-130; authored 100 ft splash; muzzle flash, linked fire |
| C_25.JT | 25mm Cannon | Done | Done | Done | Done | AC-130; muzzle flash, linked fire; no splash |
| C_40.JT | 40mm Cannon | Done | Done | Done | Done | AC-130; muzzle flash, linked fire; no splash |
| DEFA.JT | DEFA 30mm | Done | Done | Done | Done | Rafale gun; shared physical-round model, tracers; no fighter muzzle flash |
| GAU12.JT | GAU-12 Vulcan Cannon | Done | Done | Done | Done | AV-8B; physical rounds and tracers; no fighter muzzle flash |
| GAU13.JT | GAU-13 Pave Claw | Partial | None | None | None | Hidden; W1 remaining: cannon pods nothing loads |
| GAU8.JT | GAU-8/A Avenger | Done | Done | Done | Done | A-10; AI gun runs kill 150 HP vehicle; no fighter muzzle flash |
| GSH23.JT | GSh-23 23mmx2 | Done | Done | Done | Done | MiG-21/MiG-17; green tracers; no fighter muzzle flash |
| GSH30.JT | GSh-30 30mmx1 | Done | Done | Done | Done | Yak-141/MiG-17; green tracers; no fighter muzzle flash |
| GSH301.JT | GSh-301 30mmx6 | Done | Done | Done | Done | MiG-29/Su-25/27/35; green tracers; no fighter muzzle flash |
| GSH6_23.JT | GSh-6-23 23mmx6 | Partial | None | None | None | Hidden; W1 remaining: cannon pods nothing loads |
| GSH6_30.JT | GSh-6-30 30mmx6 | Done | Done | Done | Done | MiG-23; green tracers; no fighter muzzle flash |
| M61.JT | M61 Vulcan Cannon | Done | Done | Done | Done | 37 aircraft; shared gun model; no fighter muzzle flash |
| MK12.JT | Mk 12 20mmx2 | Done | Done | Done | Done | A-4E; shared gun model; no fighter muzzle flash |
| SUU16.JT | SUU-16 Gun Pod | Done | Done | Done | Done | Gun pod hangs on F-4 pylon; 1,200 rounds; no fighter muzzle flash |
| T12_4.JT | 12.7mmx4 turret | Done | Done | Partial | Done | Mi-24 12.7 mm; AI turret use unverified |
| T20_1.JT | 20mmx1 turret | None | None | None | None | Decoded only; no roster carrier or unit uses it |
| T23_1.JT | 23mmx1 turret | None | None | None | None | Decoded only; no roster carrier or unit uses it |
| T30_1.JT | 30mmx1 turret | Done | Done | Partial | Done | V-22/Ka-50-style 30 mm; AI turret use unverified |
| ~VOMIT.JT | Vomit | None | None | N/A | None | Novelty joke record; nothing loads it |

#### Surface-launched (46)

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 2S6.JT | 2S6 Tunguska | Done | Done | Done | Done | Tunguska radar gun; tuned burst, magazines; tracers/flash |
| AAA20.JT | 20mm radar AAA | None | None | None | None | 20 mm radar AAA; no unit references it |
| AAA30.JT | 30mm radar AAA | Done | Done | Done | Done | AK-630 class ship gun, radar; tracers/flash |
| AAA30BAD.JT | 30mm visual AAA | Done | Done | Done | Done | AK-230 class ship gun, visual; tracers/flash |
| ASROC.JT | ASROC | Partial | None | None | None | Ticonderoga carries it; held, no submarine target exists |
| A_M1939.JT | M1939 37mm AAA Zone | Done | Done | Done | Done | Barrage zone, 33 percent random fire; tracers/flash |
| BMP2.JT | bmp2 | Done | Done | Done | Done | IFV 30 mm; visual aim; tracers/flash |
| BTR80.JT | btr80 | Done | Done | Done | Done | IFV 14.5 mm; visual aim; tracers/flash |
| FIM92.JT | FIM-92 Stinger | Done | Done | Done | Done | Stinger unit; IR, flare decoy |
| HQ2J.JT | HQ-2J SAM | None | None | None | None | Decoded only; no unit references it |
| HQ61.JT | HQ-61 | None | None | None | None | Decoded only; no unit references it |
| KS12.JT | KS-12 85mm AAA | Done | Done | Done | Done | 85 mm flak; time fuze in 300 m egg, 4,000 ft floor; tracers/flash |
| KS19.JT | KS-19 100mm AAA | Done | Done | Done | Done | 100 mm flak; time fuze in egg, 4,000 ft floor; tracers/flash |
| M1.JT | m1 | Done | Done | Done | Done | M1 tank gun; aircraft within 20 degrees only; tracers/flash |
| M113.JT | m113 | Done | Done | Done | Done | APC .50 cal; visual aim; tracers/flash |
| M163.JT | M163 | None | None | None | None | Unused retail ZSU23 copy; M163 unit mounts PHALANX |
| M1939.JT | M1939 37mm AAA | Done | Done | Done | Done | 37 mm visual AAA; tracers/flash |
| M2.JT | m2 | Done | Done | Done | Done | M2 25 mm APC gun; visual aim; tracers/flash |
| MIM23.JT | MIM-23 HAWK | None | None | None | None | Decoded only; no unit fires it (HAWK/Roland use SA19) |
| MIS.JT | Mistral | Done | Done | Done | Done | Mistral unit; SA-3 envelope copy; surface AI fires, chaff decoys |
| PHALANX.JT | 20mm Phalanx | Done | Done | Done | Done | Ship CIWS and M163 mount; tuned rate; tracers/flash |
| R440.JT | R-440 Matra | Done | Done | Done | Done | ASA5 unit missile; surface AI fires, chaff decoys |
| ROLAND.JT | Roland Euromissile | None | None | None | None | Decoded only; no unit fires it (HAWK/Roland use SA19) |
| SA13.JT | SA-13 Gopher | Done | Done | Done | Done | SA-13 and CHAP; IR |
| SA14.JT | SA-14 Gremlin | Done | Done | Done | Done | SA-14; IR |
| SA15.JT | SA-15 Gauntlet | Done | Done | Done | Done | SA-15 self-contained launcher; surface AI fires, chaff decoys |
| SA16.JT | SA-16 Gimlet | Done | Done | Done | Done | SA-16; IR |
| SA19.JT | Sa-19 | Done | Done | Done | Done | HAWK/Roland/2S6 missile; treated as supported radar (default, pending John); surface AI fires, chaff decoys |
| SA2A.JT | SA-2A Guideline | Done | Done | Done | Done | SA-2 battery; radar support; fitted; surface AI fires, chaff decoys |
| SA3.JT | SA-3 Goa | Done | Done | Done | Done | SA-3 battery; surface AI fires, chaff decoys |
| SA6.JT | SA-6 Gainful | Done | Done | Done | Done | SA-6 battery; surface AI fires, chaff decoys |
| SA7.JT | SA-7 Grail | Done | Done | Done | Done | SA-7; IR, flare decoy |
| SA9.JT | SA-9 Gaskin | Done | Done | Done | Done | SA-9 and SCUD launcher; IR |
| SAN11.JT | SA-N-11 | Done | Done | Done | Done | Kirov; SA-N-11 supported radar (default, pending John); surface AI fires, chaff decoys |
| SAN3.JT | SA-N-3 Goblet | Done | Done | Done | Done | Kiev ship SAM; surface AI fires, chaff decoys |
| SAN4.JT | SA-N-4 Gecko | Done | Done | Done | Done | Ship SAM on 7 ships; surface AI fires, chaff decoys |
| SAN5.JT | SA-N-5 Grail | None | None | None | None | Decoded only; no unit references it |
| SAN7.JT | SA-N-7 Gadfly | Done | Done | Done | Done | Sovremennyy ship SAM; surface AI fires, chaff decoys |
| SAN8.JT | SA-N-8 Gremlin | None | None | None | None | Decoded only; no unit references it |
| SAN9.JT | SA-N-9 Klinok | Done | Done | Done | Done | Kiev ship SAM; surface AI fires, chaff decoys |
| SEA_SPAR.JT | AIM-7 Sea Sparrow | Done | Done | Done | Done | Carrier/Iowa Sea Sparrow; 4 carriers; surface AI fires, chaff decoys |
| SMLARMS.JT | Small Arms fire | Done | Done | Done | Done | Squad small arms; rarely hit jets; tracers/flash |
| SSN9.JT | SS-N-9 | Done | Done | Done | Done | Sarancha anti-ship; surface vs surface since 2026-10-11 (roadmap W6 stale) |
| T72.JT | t72 | Done | Done | Done | Done | T-72/80/90 tank gun; 3 s lay on aircraft; tracers/flash |
| ZSU23.JT | ZSU-23 Shilka | Done | Done | Done | Done | Shilka radar gun; tuned burst; tracers/flash |
| ZSU57.JT | ZSU-57 | Done | Done | Done | Done | ZSU-57/ZIF31 visual gun; tracers/flash |

#### Counts, ordnance (135)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 111 | 105 | 107 | 110 |
| Partial | 12 | 11 | 2 | 6 |
| None | 12 | 19 | 22 | 19 |
| N/A | 0 | 0 | 4 | 0 |
| Total | 135 | 135 | 135 | 135 |

Per category:

**Air-to-air missile** (22)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 17 | 17 | 17 | 17 |
| Partial | 5 | 5 | 0 | 5 |
| None | 0 | 0 | 5 | 0 |
| N/A | 0 | 0 | 0 | 0 |
| Total | 22 | 22 | 22 | 22 |

**Air-to-ground missile** (14)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 14 | 14 | 14 | 14 |
| Partial | 0 | 0 | 0 | 0 |
| None | 0 | 0 | 0 | 0 |
| N/A | 0 | 0 | 0 | 0 |
| Total | 14 | 14 | 14 | 14 |

**Bomb** (24)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 24 | 18 | 22 | 23 |
| Partial | 0 | 6 | 0 | 1 |
| None | 0 | 0 | 0 | 0 |
| N/A | 0 | 0 | 2 | 0 |
| Total | 24 | 24 | 24 | 24 |

**Rocket** (5)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 4 | 4 | 4 | 4 |
| Partial | 0 | 0 | 0 | 0 |
| None | 1 | 1 | 0 | 1 |
| N/A | 0 | 0 | 1 | 0 |
| Total | 5 | 5 | 5 | 5 |

**Gun round (aircraft, gunship, pod)** (24)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 15 | 15 | 13 | 15 |
| Partial | 6 | 0 | 2 | 0 |
| None | 3 | 9 | 8 | 9 |
| N/A | 0 | 0 | 1 | 0 |
| Total | 24 | 24 | 24 | 24 |

**Surface-launched** (46)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 37 | 37 | 37 | 37 |
| Partial | 1 | 0 | 0 | 0 |
| None | 8 | 9 | 9 | 9 |
| N/A | 0 | 0 | 0 | 0 |
| Total | 46 | 46 | 46 | 46 |

### Equipment (SEE/ECM/GAS)

#### Sensor (SEE) (51)

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| A10R.SEE | A-10 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; A-10 |
| A7R.SEE | A-7 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; A-7 |
| AC130R.SEE | AC-130 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; AC-130 |
| AV8R.SEE | AV-8B radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; AV-8B |
| E2R.SEE | E-2 AEW radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; E-2 |
| E3R.SEE | E-3 AWACS radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; E-3, IL-76 |
| F104R.SEE | F-104 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; F-104 |
| F14R.SEE | AWG-9 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; F-14D |
| F15R.SEE | F-15 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; F-15C |
| F18R.SEE | APG-65 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; F-18, Rafale, F-16C, F-4G/E, X-31, AH-64 |
| F22R.SEE | F-22 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; F-22A/N |
| F4BR.SEE | APQ-72 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; A-4E, F-4B |
| F4JR.SEE | F-4J radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; F-4J |
| MIG21R.SEE | Jay Bird radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; MiG-21 |
| MIG27R.SEE | High Lark radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; MiG-23 |
| MIG29R.SEE | Slot Back radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; MiG-29 |
| SU24R.SEE | Su-24 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; Su-25 |
| SU27R.SEE | Su-27 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; Su-27, Su-35 |
| YAK141R.SEE | Yak-141 radar | Done | N/A | N/A | Done | Reviewed preset, shared radar component; Yak-141 |
| A6R.SEE | A-6 radar | None | N/A | N/A | None | No roster carrier; no reviewed preset, import errors if used |
| B52R.SEE | B-52/AF1 radar | None | N/A | N/A | None | No roster carrier; no reviewed preset, import errors if used |
| E6R.SEE | E-6 radar | None | N/A | N/A | None | No carrier at all; no reviewed preset |
| E8R.SEE | E-8 radar | None | N/A | N/A | None | No roster carrier; no reviewed preset, import errors if used |
| EA6R.SEE | EA-6 radar | None | N/A | N/A | None | No roster carrier; no reviewed preset, import errors if used |
| F4R.SEE | F-4 radar | None | N/A | N/A | None | No carrier at all; no reviewed preset |
| F8R.SEE | F-8 radar | None | N/A | N/A | None | No roster carrier; no reviewed preset, import errors if used |
| MIG31R.SEE | MiG-31 radar | None | N/A | N/A | None | No roster carrier; no reviewed preset, import errors if used |
| SEAHARR.SEE | Sea Harrier radar | None | N/A | N/A | None | No roster carrier; no reviewed preset, import errors if used |
| SU35R.SEE | Su-35 radar | None | N/A | N/A | None | No carrier at all; no reviewed preset |
| SU37R.SEE | Su-37 radar | None | N/A | N/A | None | No carrier at all; no reviewed preset |
| TU160R.SEE | Tu-160 radar | None | N/A | N/A | None | No roster carrier; no reviewed preset, import errors if used |
| TU26R.SEE | Tu-26 radar | None | N/A | N/A | None | No roster carrier; no reviewed preset, import errors if used |
| TU95R.SEE | Tu-95 radar | None | N/A | N/A | None | No roster carrier; no reviewed preset, import errors if used |
| AAS38.SEE | AAS-38 FLIR pod | Done | None | N/A | Done | Rafale fixed row; FLIR sight 91,140 ft; pod body drawing unverified |
| AV8IR.SEE | AV-8B internal FLIR | Done | N/A | N/A | Done | X-31, F-22, F-22N, AV-8B, AH-64; 60 degree cone sight |
| AC130I.SEE | AC-130 FLIR | Done | N/A | N/A | Done | AC-130 console, / range 54,684 ft |
| MIG29I.SEE | MiG-29 IRST | Done | N/A | N/A | Partial | IR 9/10 nmi kept; initial vs retained track review open |
| SU27I.SEE | Su-27 IRST | Done | N/A | N/A | Partial | Su-27/35; IR 9/10 nmi kept; track review open |
| SU37I.SEE | Su-37 IRST | None | N/A | N/A | None | No carrier at all |
| AV8L.SEE | AV-8 laser | None | N/A | N/A | None | No carrier at all |
| SU24L.SEE | Su-24 laser | Done | N/A | Done | Done | Su-25 built-in laser; AI lases own bombs |
| KA50L.SEE | Ka-50 laser | Done | N/A | Done | Done | V-22 built-in laser; AI lases |
| SU37L.SEE | Su-37 laser | None | N/A | N/A | None | Carriers Su-34, Tu-160/26/95 not in roster |
| PAVESPK.SEE | Pave Spike laser pod | Done | None | Done | Done | Fixed on A-10/F-16C and loadable on pod stations; body drawing unverified |
| PAVEKNF.SEE | Pave Knife laser pod | Partial | N/A | N/A | Partial | Retired 2026-10-16; old loads become Pave Spike |
| VIS180.SEE | Visual sensor 180 | None | N/A | N/A | None | No carrier at all |
| VIS240.SEE | Visual sensor 240 | Done | N/A | N/A | Done | MiG-17/21/23, Su-25; 10/5 nmi, 120x90 deg |
| VIS300.SEE | Visual sensor 300 | Done | N/A | N/A | Done | Transports, E-2/3, A-7, F-104, Mi-24, Yak-141; reviewed use unverified |
| VIS340.SEE | Visual sensor 340 | Done | N/A | N/A | Done | Most fighters; 10/5 nmi, 170x140 deg |
| GCIR.SEE | GCI radar | Done | N/A | N/A | Done | GCI site sensor; 303,800 ft detection range |
| REDCR.SEE | Red Crown radar | Done | N/A | N/A | Done | Butler picket sensor; 303,800 ft detection range |

#### Countermeasure (ECM) (30)

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| A10.ECM | A10 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; A-10 |
| A7.ECM | A7 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; A-7 |
| AC130.ECM | AC130 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; AC-130 |
| AV8.ECM | AV8 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; AV-8B |
| B52.ECM | B52 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; E-2, E-3, IL-76 |
| F104.ECM | F104 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; F-104 |
| F14.ECM | F14 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; F-14D |
| F15.ECM | F15 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; F-15C |
| F18.ECM | F18 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; F-18, Rafale, F-16C, F-4G/E, X-31, AH-64 |
| F22.ECM | F22 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; F-22A/N |
| F4.ECM | F4 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; A-4E, F-4B/J |
| KA50.ECM | KA50 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; V-22 |
| MIG21.ECM | MIG21 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; MiG-21, MiG-23 |
| MIG29.ECM | MIG29 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; MiG-29 |
| SU24.ECM | SU24 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; Su-25 |
| SU27.ECM | SU27 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; Su-27, Su-35 |
| YAK141.ECM | YAK141 ECM suite | Done | Done | Done | Done | Chaff/flare, decoy odds, jammer generation reviewed; Yak-141 |
| ALQ167.ECM | ALQ-167 jammer pod | Partial | None | N/A | None | AV-8B locked row; generation unreviewed, so jammer unverified |
| A6.ECM | A-6 ECM | None | None | None | None | A-6/F-111 only; non-roster, generation unreviewed |
| ALE40.ECM | ALE-40 dispenser | None | None | None | None | no carrier at all; non-roster, generation unreviewed |
| ALQ72.ECM | ALQ-72 jammer | None | None | None | None | M5 only; non-roster, generation unreviewed |
| EA6.ECM | EA-6 ECM | None | None | None | None | EA-6 only; non-roster, generation unreviewed |
| F8.ECM | F-8 ECM | None | None | None | None | F-8J only; non-roster, generation unreviewed |
| MIG27.ECM | MiG-27 ECM | None | None | None | None | MiG-27 only; non-roster, generation unreviewed |
| MIG31.ECM | MiG-31 ECM | None | None | None | None | MiG-31/Jaguar/Tornado only; non-roster, generation unreviewed |
| SEAHAR.ECM | Sea Harrier ECM | None | None | None | None | Sea Harrier only; non-roster, generation unreviewed |
| SU37.ECM | Su-37 ECM | None | None | None | None | no carrier at all; non-roster, generation unreviewed |
| TU160.ECM | Tu-160 ECM | None | None | None | None | Tu-160 only; non-roster, generation unreviewed |
| TU26.ECM | Tu-26 ECM | None | None | None | None | Tu-26 only; non-roster, generation unreviewed |
| TU95.ECM | Tu-95 ECM | None | None | None | None | Tu-95 only; non-roster, generation unreviewed |

#### Fuel tank (GAS) (4)

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| F150.GAS | 150 gal tank | Done | Done | Done | Done | A-4E, F-16C, F-18, F-4, MiG-17; fitted body, fuel/mass, Shift+K jettison |
| F250.GAS | 250 gal tank | Done | Done | Done | Done | F-14, F-104; fitted body, fuel/mass, Shift+K jettison |
| F350.GAS | 350 gal tank | Done | Done | Done | Done | F-4B/J; loadable on F-18, F-4E, Su-25; fitted body, fuel/mass, Shift+K jettison |
| F500.GAS | 500 gal tank | Done | Done | Done | Done | Loadable on A-7, AV-8B, F-14, F-15, F-16C; fitted body, fuel/mass, Shift+K jettison |

#### Counts, equipment (85)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 53 | 21 | 24 | 51 |
| Partial | 2 | 0 | 0 | 3 |
| None | 30 | 15 | 12 | 31 |
| N/A | 0 | 49 | 49 | 0 |
| Total | 85 | 85 | 85 | 85 |

**Sensor (SEE)** (51)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 32 | 0 | 3 | 30 |
| Partial | 1 | 0 | 0 | 3 |
| None | 18 | 2 | 0 | 18 |
| N/A | 0 | 49 | 48 | 0 |
| Total | 51 | 51 | 51 | 51 |

**Countermeasure (ECM)** (30)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 17 | 17 | 17 | 17 |
| Partial | 1 | 0 | 0 | 0 |
| None | 12 | 13 | 12 | 13 |
| N/A | 0 | 0 | 1 | 0 |
| Total | 30 | 30 | 30 | 30 |

**Fuel tank (GAS)** (4)

| Status | Imported | Animations | AI | Logic |
| --- | --- | --- | --- | --- |
| Done | 4 | 4 | 4 | 4 |
| Partial | 0 | 0 | 0 | 0 |
| None | 0 | 0 | 0 | 0 |
| N/A | 0 | 0 | 0 | 0 |
| Total | 4 | 4 | 4 | 4 |

## Surface units (NT)


Research mode, read-only, 2026-11-06. 84 records from `FA_2.LIB`.

### Mechanisms

An NT is a PT minus its PLANE block: OBJECT and NPC blocks plus hardpoint rows naming JT weapons or SEE sensors; each record names one main shape (`docs/formats/surface-units.md`, reader `tore_formats::surface_unit`). `tore_formats::surface_set::select` imports all 84 plus `DEST.OT`, with shadow, damaged (`_A`), weapon, sensor and sound resources (`docs/spec/import-cache.md`). Placement comes from the 129 `~Q*.M` templates, the executable's equipment lists (`quick_template/tables.rs` `LISTS`) and the base layouts; I confirmed placements by grepping the extracted `.M` files (only OILR is placed nowhere; MULE_A and CATGUY arrive via the carrier part table, EJECT via ejection). Runtime behaviour: `tore_sim::ai::surface` (one controller per mount group or battery radar: detect, prepare, track, fire, magazines, HARM shutdown), `tore_world::surface` (resolve, movement, aim, supply, emitters, muzzle, batteries), spec `docs/spec/surface-defenses.md`, acceptance `docs/baselines/surface-defenses.md`. Surface AI was authorized 2026-10-10 (ROADMAP surface round, W6 built).

Animation is limited by the retail shapes: only KRIV/SOVR turrets, KRIV/SOVR/REDCR/RLD antennas have parts; every other armed land unit turns its whole body (CL-AIM, fitted rates) and shows rail rounds; nothing elevates; no wheel/track/wake animation exists. Destroyed looks: ships swap to `_A` and burn, land units to `DEST.SH`, men removed or lie down. Movement exists only for the five routed templates (tank column ~QUCOL, 3 CARGO2 ships ~QTCARGO, TRUCK in ~QUFACT/~QUBUNK); all other ships are static targets, hydrofoil and carrier programs are open (ROADMAP line 716), ASROC is held, ROADMAP M3 lists "Carrier operations: launch, recovery, deck". Online clients see only gun aim after bursts (protocol 25).

Grading convention: Done = every behaviour applicable to that record that retail evidence or the docs call for is built (including wreck state); Partial = some built, a named gap remains; None = applicable but absent; N/A = not applicable (e.g. AI for an unarmed static target). AI and Logic grades for fitted rules (target ranking, hit chance, movement) are Done where the spec marks them built, with the "fitted" caveat in Notes only when notable. Evidence is from docs and census, not an in-game run; items marked "unverified" were not traced to code.

### Table

Grades: Done / Partial / None / N/A. Category is the first column group heading.

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
|---|---|---|---|---|---|---|
| **SAM** | | | | | | |
| 2S6.NT | 2S6 Tunguska gun+SA-19 | Done | Partial | Done | Done | Gun+missile controllers; body turns, no elevation |
| ASA5.NT | Crotale SAM | Done | Partial | Done | Done | Self-contained R440 x2; whole-body turn |
| CHAP.NT | M48 Chaparral | Done | Partial | Done | Done | Rails empty/refill shown; whole-body turn |
| FIM92.NT | Stinger team (MANPADS) | Done | Partial | Done | Done | Hidden team rule; 5 hp; whole-body turn |
| HAWK.NT | MIM-23 HAWK | Done | Partial | Done | Done | Battery of 6; radar is TORE-defined SRDR2 element |
| MIS.NT | Mistral team (MANPADS) | Done | Partial | Done | Done | Hidden team; 5 hp; shape PMIS |
| ROLAND.NT | Roland SAM | Done | Done | Done | Done | RLD antenna spins (2 s); fights; SA-19 rails |
| SA13.NT | SA-13 Gopher | Done | Partial | Done | Done | IR SAM x4; whole-body turn |
| SA14.NT | SA-14 team (MANPADS) | Done | Partial | Done | Done | Hidden team; 5 hp |
| SA15.NT | SA-15 Gauntlet | Done | Partial | Done | Done | 8 rails, 2-round salvo; fastest reaction |
| SA16.NT | SA-16 team (MANPADS) | Done | Partial | Done | Done | Shares SA14 shape; hidden team |
| SA2A.NT | SA-2 Guideline site | Done | Partial | Done | Done | Six fixed rails; GCI battery; 650 hp; no turn |
| SA3.NT | SA-3 Goa | Done | Partial | Done | Done | Battery with GCI radar; rails shown |
| SA6.NT | SA-6 Gainful | Done | Partial | Done | Done | Battery with SFLUSH radar; 5 deg/s body turn |
| SA7.NT | SA-7 team (MANPADS) | Done | Partial | Done | Done | Hidden team; 5 hp |
| SA9.NT | SA-9 Gaskin | Done | Partial | Done | Done | IR SAM x4; whole-body turn |
| SCUD.NT | SCUD launcher (class SAM) | Done | Partial | Done | Partial | Fires 4 SA-9 only; no SCUD ground strike; rails shown |
| **AAA** | | | | | | |
| A_M1939.NT | M1939 barrage zone (invisible) | Done | Partial | Done | Done | No shape; target volume; no flash; removed on death |
| KS12.NT | KS-12 85 mm flak | Done | Partial | Done | Done | Flak bursts, team retarget; body turn; muzzle table |
| KS19.NT | KS-19 100 mm flak | Done | Partial | Done | Done | Flak bursts, team retarget; body turn |
| M163.NT | M163 Vulcan | Done | Partial | Done | Done | Ranges replaced by AAA tuning (opinionated) |
| M1939.NT | M1939 37 mm | Done | Partial | Done | Done | Gun cadence, mags, resupply; body turn |
| ZIF31.NT | ZIF-31 AAA | Done | Partial | Done | Done | Uses ZSU57 record; body turn |
| ZSU23.NT | ZSU-23-4 Shilka | Done | Partial | Done | Done | Radar gun; night-rule unit; body turn 30 deg/s |
| ZSU57.NT | ZSU-57-2 | Done | Partial | Done | Done | Body turn 15 deg/s; tuned cadence |
| **Armor / tanks** | | | | | | |
| BMP2.NT | BMP-2 IFV | Done | Partial | Done | Done | 30 mm; may fight ground units; column halts to fight; body turns, no turret |
| BTR80.NT | BTR-80 APC | Done | Partial | Done | Done | 14.5 mm; fights air and ground; body turns, no turret |
| M1.NT | M1 Abrams | Done | Partial | Done | Done | 120 mm; 20 deg elevation ceiling vs air; body turns, no turret |
| M113.NT | M113 APC | Done | Partial | Done | Done | Also AAA/SAM placeholder; .50 cal; body turns, no turret |
| M1975.NT | M1975 artillery (no weapon) | Done | Done | N/A | Done | Unarmed tank class; static target, wreck only |
| M2.NT | M2 Bradley | Done | Partial | Done | Done | 25 mm; fights air and ground; body turns, no turret |
| T72.NT | T-72 | Done | Partial | Done | Done | Drives in ~QUCOL column; lanes; fights; body turns, no turret |
| T80.NT | T-80 | Done | Partial | Done | Done | Drives in ~QUCOL column; fights; body turns, no turret |
| T90.NT | T-90 | Done | Partial | Done | Done | Drives in ~QUCOL column; fights; body turns, no turret |
| **Vehicles / radars** | | | | | | |
| HUMVEE.NT | HMMWV (unarmed) | Done | Done | N/A | Done | Passive target; wreck swap |
| LTRACK.NT | Low Blow radar truck | Done | Done | N/A | Done | Always-on RWR emitter by name (agent rule) |
| MISTRK.NT | SAM-carrying truck | Done | Done | Done | Done | Battery supply truck; rearm rails 420 s rule |
| MULE_A.NT | Deck tractor A | Done | Partial | None | Partial | Carrier part by table; static, retail motion untraced |
| MULE_B.NT | Deck tractor B | Done | Partial | None | Partial | Placed in BEXTRA layouts; static, no motion |
| MULE_C.NT | Deck tractor C | Done | Partial | None | Partial | Placed in BEXTRA layouts; static, no motion |
| SFLUSH.NT | Straight Flush radar | Done | Done | Done | Done | SA-6 battery radar; HARM shutdown; RWR square |
| SRDR1.NT | Radar vehicle 1 | Done | Done | N/A | Done | Always-on emitter by name; no sensor in data |
| SRDR2.NT | Radar vehicle 2 (HAWK element) | Done | Done | Done | Done | Drawn as HAWK battery radar (John pick) |
| TANKER.NT | Fuel tanker | Done | Done | N/A | Partial | Passive target; no fuel resupply rule (unverified) |
| TRUCK.NT | Supply truck | Done | Done | Done | Done | Drives routes; SAM/AAA rearm; no wheel parts in shape |
| **Other (structure, men)** | | | | | | |
| CATGUY.NT | Carrier catapult officer | Done | Partial | N/A | N/A | Sprite, 11 frames cycled 4/s (fitted) |
| EJECT.NT | Ejected pilot | Done | Done | N/A | Done | Ejection/chute/landing per ejection.md; unverified detail |
| GCI.NT | GCI/Tall King radar | Done | Done | Done | Done | 50 nm sensor; SA-2/3 battery radar; RWR emitter |
| PLTDWN.NT | Downed pilot | Partial | Partial | N/A | Partial | In ~K17/~U46/UKR27; drawing unverified |
| RUNNER.NT | Runner sprite | Done | Partial | N/A | Done | Sprite taught; run motion unverified |
| SOLDIER.NT | Soldier sprite | Done | Done | N/A | Done | Sprite, 5 hp; removed on death |
| TROOPS.NT | Infantry squad (small arms) | Done | Partial | Done | Done | Cards; damaged copy lies down; fires at 4,000 ft |
| **Ships** | | | | | | |
| BARGE.NT | Barge | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| BUTLER.NT | J.C. Butler Red Crown picket | Done | Partial | Partial | Done | REDCR antenna spins; sensor emits; static |
| CARGO.NT | Merchant cargo ship | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| CARGO2.NT | Cargo ship (target) | Done | Partial | Partial | Done | Sails ~QTCARGO route 9.5 kt; no wake; unarmed |
| CIM.NT | CIM small craft | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| CLEM.NT | Clemenceau carrier | Done | Partial | Partial | Partial | Phalanx+Sea Sparrow; static; no deck ops; _A hull, island |
| CYCL.NT | Cyclone patrol craft | Done | Partial | Partial | Done | AAA30BAD+SA-N-4; static; wreck _A only |
| FISHBT.NT | Fishing boat | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| IOWA.NT | Iowa battleship | Done | Partial | Partial | Done | Phalanx+Sea Sparrow; static; no wake |
| JIANC.NT | Jianghu-C frigate | Done | Partial | Partial | Done | AAA30BAD+SA-N-4; static; wreck _A only |
| JIANE.NT | Jianghu-E frigate | Done | Partial | Partial | Done | AAA30BAD+SA-N-4; static; wreck _A only |
| JUNK.NT | Junk | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| KIEV.NT | Kiev carrier | Done | Partial | Partial | Partial | SA-N-3/9, AAA30; static; no deck ops; _A hull, island |
| KIROV.NT | Kirov cruiser | Done | Partial | Partial | Done | AAA30+SA-N-11; static; no wake |
| KITT.NT | Kitty Hawk carrier | Done | Partial | Partial | Partial | Phalanx+Sea Sparrow; static; no deck ops; _A hull, island |
| KNOX.NT | Knox frigate | Done | Partial | Partial | Done | AAA30BAD+SA-N-4; static; wreck _A only |
| KRIVAK.NT | Krivak frigate | Done | Partial | Partial | Done | Turrets+antenna turn; no ship movement |
| LCAC.NT | LCAC hovercraft | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| NIMZ.NT | Nimitz carrier | Done | Partial | Partial | Partial | Phalanx+Sea Sparrow; static; no deck ops; _A hull, island |
| OILR.NT | Oil rig | Partial | Partial | N/A | Done | Loaded; no template/layout places it found |
| OLEKMA.NT | Olekma tanker | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| OSCAR.NT | Oscar submarine (surfaced) | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| PMORN.NT | Pomornik hovercraft | Done | Partial | Partial | Done | AAA30BAD x2; static, no hovercraft motion |
| RBOAT.NT | River boat | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| SACRAM.NT | Sacramento oiler | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| SARAN.NT | Sarancha hydrofoil | Done | Partial | Partial | Done | SS-N-9 vs ships built; hydrofoil motion open |
| SESHDW.NT | Sea Shadow | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| SL100.NT | SL-100 hovercraft | Done | Partial | N/A | Done | Static passive target; _A wreck; no wake |
| SOVR.NT | Sovremennyy destroyer | Done | Partial | Partial | Done | Turrets+antenna turn; no ship movement |
| TICON.NT | Ticonderoga cruiser | Done | Partial | Partial | Partial | Phalanx fires; ASROC held; static |
| TYPE69.NT | Type 69 frigate | Done | Partial | Partial | Done | AAA30BAD+SA-N-4; static; wreck _A only |
| WASP.NT | Wasp carrier | Done | Partial | Partial | Partial | Phalanx only; static; no deck ops; _A hull, island |

### Counts

| Column | Done | Partial | None | N/A |
|---|---:|---:|---:|---:|
| Imported | 82 | 2 | 0 | 0 |
| Animations | 13 | 71 | 0 | 0 |
| AI | 39 | 19 | 3 | 23 |
| Logic | 71 | 12 | 0 | 1 |

## Static objects (OT)


Research mode, read-only; retail OT/MM files extracted outside the repo only. Grades are by shared code path; "unverified" marks gaps.

### Mechanisms

- **What an OT is.** An OT is the OBJECT block alone: name, flags, class, hit points, explosion/crater words, one explicit main SH. NT/PT add NPC blocks, hardpoints and (PT) plane data; JT is a weapon. 83 structures, 74 other objects and 13 runways (`_STRIPProc`) in the archive. Evidence: docs/formats/objects-and-shapes.md ("NT surface-unit layout"), docs/formats/surface-units.md:82.
- **Import.** All 170 definitions parse; 164 shapes project; TREE1/TREE2 shapes are absent (docs/baselines/retail-terrain-review.md "Object findings"). CRATER.SH reads since OT-1 (2026-11-06): its long native prelude resumes on the decal program and its `dc` face projects. 75 of 75 retail layouts build (docs/formats/airport-placements.md). The city fill kit also uses OT shapes (crates/tore-world/src/city/kit.rs).
- **Scale and footing.** One rule for all (`terrain::placed_shape_scale`, `tore_world::grounding`): a third of retail scale, runways/bridges/roads at 1, stand on lowest point, broad foundations (docs/spec/surface-defenses.md "Grounded objective assemblies", docs/spec/bridges.md).
- **Damage and destruction.** Placed objects are live targets with OT hit points. At 0 HP: `~` variant (`~BNK5/6/8`, `~COLTWR`, carrier islands), else the shape's own damage branch (57 types), else a fitted generated ruin; fire and smoke persist (docs/spec/surface-defenses.md "Destroyed looks", "Wreck fires"). Pavement, roads, lots, crops, rocks and flags stay as they stood. Runways keep craters.
- **Runways/airports.** Landing, tower, ILS, STOL labels, redrawn merged fields (docs/spec/airports.md, docs/formats/redrawn-airports.md). Retail has no airfield lights or windsocks (redrawn-airports.md "Lights"); the lights present are an added redrawn-field feature.
- **Animation.** Retail OT shapes turn only via `_currentTicks` antennas: CTWR1, CTWR2, TOWER, HAWKS.SH. These are drawn still ("not built", objects-and-shapes.md "Turrets and rotating antennas"). No flag, windmill or radar-spin envelope exists in any OT shape.
- **AI.** No OT carries weapons (surface-units.md:82); defences are NT units, so AI is N/A for the whole group.
- **Caveat.** "Imported" Partial for types no retail MM places (checked by scanning the 150 extracted MM/M files for each OT name; types used by the fill kit or carriers are Done). Per-type hit points and target-ability for pavement/rocks/flags are unverified.

### Table

| Resource | Name/role | Imported | Animations | AI | Logic | Notes |
|---|---|---|---|---|---|---|
| **airport/runway** | | | | | | |
| CTOWR.OT | Tower / ctowr.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| CTWR1.OT | Tower / ctwr1.SH | Done | Partial | N/A | Done | Retail damage branch at 0 HP; fire; Antenna spin not built |
| CTWR2.OT | Tower 2 / ctwr2.SH | Done | Partial | N/A | Done | Retail damage branch at 0 HP; fire; Antenna spin not built |
| DTSTRP.OT | Runway / dtstrp.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights |
| FLAGO1.OT | Runway Flag / fgo1.SH | Done | N/A | N/A | Partial | Static; no destroy look; targetable unverified |
| FLAGO2.OT | Runway Flag / fgo2.SH | Done | N/A | N/A | Partial | Static; no destroy look; targetable unverified |
| FLAGR1.OT | Runway Flag / fgr1.SH | Done | N/A | N/A | Partial | Static; no destroy look; targetable unverified |
| FLAGR2.OT | Runway Flag / fgr2.SH | Done | N/A | N/A | Partial | Static; no destroy look; targetable unverified |
| FLAGY1.OT | Runway Flag / fgy1.SH | Done | N/A | N/A | Partial | Static; no destroy look; targetable unverified |
| FLAGY2.OT | Runway Flag / fgy2.SH | Done | N/A | N/A | Partial | Static; no destroy look; targetable unverified |
| HANGR.OT | Hangar / hangr.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| HANGRB.OT | Hangar / hangrb.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| STRIP.OT | Runway / runway.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights |
| STRIP1.OT | Runway 1 / rnwy1.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights |
| STRIP2.OT | Runway 2 / rnwy2.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights |
| STRIP3.OT | Runway 3 / rnwy3.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights |
| STRIP3A.OT | Runway 3a / rnwy3a.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights; independent instance |
| STRIP4.OT | Runway 4 / rnwy4.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights |
| STRIP5.OT | Runway 5 / rnwy5.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights |
| STRIP5A.OT | Runway 5a / rnwy5a.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights; independent instance |
| STRIP6.OT | Runway 6 / rnwy6.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights |
| STRIP6A.OT | Runway 6a / rnwy6a.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights; independent instance |
| STRIP7.OT | Runway 7 / rnwy7.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights |
| STRIP7A.OT | Runway 7a / rnwy7a.SH | Done | Done | N/A | Done | Landing service, ILS, craters kept; retail has no lights; independent instance |
| TOWER.OT | Tower / tower.SH | Done | Partial | N/A | Done | Retail damage branch at 0 HP; fire; Antenna spin not built |
| **military structure** | | | | | | |
| AV8TNT.OT | Tent with Av-8 / av8tnt.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| BARKSA.OT | Barracks / barksa.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BARKSB.OT | Barracks / barksb.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BNK1.OT | HC&C / bnk1.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BNK2.OT | HAS / bnk2.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BNK3.OT | HAS / bnk3.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BNK4.OT | HAS / bnk4.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BNK5.OT | HAS / bnk5.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BNK6.OT | HAS / bnk6.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BNK7.OT | HAS / bnk7.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BNK8.OT | HAS / bnk8.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BNK9.OT | HC&C Super / bnk9.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BUNKER.OT | Bunker / bunkb.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| CASTLE.OT | Castle / cast.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CMHQ1.OT | CMD HQ / cmhq1.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| CMHQ2.OT | CMD HQ / cmhq2.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| COLTWR.OT | Reactor / coltwr.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| COMM.OT | C&C / shelt.SH | Done | Partial | N/A | Done | Branch unverified; fitted ruin |
| HAWKSITE.OT | MIM-23 Site / hawks.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; Antenna spin not built; No weapons |
| HILT.OT | Prison Compound / hilt.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| KING.OT | Radar / king.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; Radar building, no weapons |
| MICRO.OT | Comm Dish / micro.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| MICROM.OT | Comm Tower / microm.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| MTCAMO.OT | Tent / mtcamo.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| NUCCT.OT | Reactor / nucct.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire; burns hard |
| NUCOB.OT | Office / nucob.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire; burns hard |
| NUCRD.OT | Reactor / nucrd.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire; burns hard |
| PRDR1.OT | Radar / prdr1.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire; Radar building, no weapons; burns hard |
| PRDR2.OT | Radar / prdr2.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire; Radar building, no weapons; burns hard |
| REACTB.OT | Reactor / reactb.SH | Done | Partial | N/A | Done | Branch unverified; fitted ruin; burns hard |
| REACTR.OT | Generator / reactr.SH | Done | Partial | N/A | Done | Branch unverified; fitted ruin; burns hard |
| REDOM.OT | Reactor / redom.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| RELAY.OT | Relay / relay.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| SA3SITE.OT | SA-3 Site / sa3sit.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; No weapons |
| SHELT.OT | Shelter / shelt.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| SILO.OT | Missile Silo / silo.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire; burns hard |
| TENT1.OT | Tent / tent1.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| ~BNK5.OT | DS5 / dbk5.SH | Done | Done | N/A | Done | Damaged variant on destruction; burns |
| ~BNK6.OT | DS6 / dbk6.SH | Done | Done | N/A | Done | Damaged variant on destruction; burns |
| ~BNK8.OT | DS8 / dbk8.SH | Done | Done | N/A | Done | Damaged variant on destruction; burns |
| ~COLTWR.OT | Reactor / coltwr.SH | Done | Done | N/A | Done | Damaged variant on destruction; burns |
| **bridge** | | | | | | |
| BR1END.OT | Medium Bridge / br1end.SH | Done | Partial | N/A | Done | Fitted ruin (John); strike target; no river effect |
| BR1MID.OT | Medium Bridge / br1mid.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; strike target |
| BR2END.OT | Small Bridge / br2end.SH | Done | Partial | N/A | Done | Fitted ruin (John); strike target; no river effect |
| BR2MID.OT | Small Bridge / br2mid.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; strike target |
| BR3END.OT | Temp Bridge / br3end.SH | Done | Partial | N/A | Done | Fitted ruin (John); strike target; no river effect |
| BR3MID.OT | Temp Bridge / br3mid.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; strike target |
| BRD1.OT | Bridge 1 / brd1.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; strike target |
| BRD2.OT | Bridge 2 / brd2.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; strike target |
| BRD3.OT | Bridge 3 / brd3.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; strike target |
| BRD4.OT | Bridge 4 / brd4.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; strike target |
| BRDEND.OT | Bridge / brdend.SH | Done | Partial | N/A | Done | Fitted ruin (John); strike target; no river effect |
| BRDMID.OT | Bridge / brdmid.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; strike target |
| **civil building / industry** | | | | | | |
| APTB1.OT | Apartments / aptb1.SH | Partial | Partial | N/A | Done | Fitted ruin, no retail branch; fire; Loads; no retail MM places it |
| APTLA.OT | Apartments / aptla.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| APTLB.OT | Apartments / aptlb.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| APTLC.OT | Apartments / aptlc.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| APTM.OT | Apartment / aptm.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| APTOLD.OT | Apartment / aptold.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| BLDG1.OT | Factory 3 / bld1.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BLDG2.OT | Warehouse / bld2.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| BLDG3.OT | Factory 4 / bld3.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| CGRP1.OT | Comm. Blk 1 / cgrp1.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CGRP2.OT | Comm. Blk 2 / cgrp2.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CGRP3.OT | Comm. Blk 3 / cgrp3.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CITY1.OT | Large City 1 / city1.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CITY2.OT | Large City 2 / city2.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CITY3.OT | Large City 3 / city3.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| COMM1.OT | Shops / comm1.SH | Partial | Partial | N/A | Done | Branch unverified; fitted ruin; Loads; no retail MM places it |
| COMM2.OT | Shops / comm2.SH | Partial | Partial | N/A | Done | Branch unverified; fitted ruin; Loads; no retail MM places it |
| CONT1.OT | Container / cont1.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; burns hard |
| CONT2.OT | Container / cont2.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; burns hard |
| CONT3.OT | Container / cont3.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; burns hard |
| CRANE.OT | Crane / crane.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| CRATE1.OT | Crate / crate.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; burns hard |
| CRATE2.OT | Crate / crats1.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; burns hard |
| CRATE3.OT | Crate / crats2.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; burns hard |
| CRATE4.OT | Crate / crats3.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; burns hard |
| CTYBKA.OT | City Blk 1 / ctybka.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CTYBKB.OT | City Blk 2 / ctybkb.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CTYBKC.OT | City Blk 3 / ctybkc.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CTYBKD.OT | City Blk 4 / ctybkd.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CTYBKE.OT | City Blk 5 / ctybke.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CTYBKF.OT | City Blk 6 / ctybkf.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| CTYBKG.OT | City Blk 7 / ctybkg.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| DKHOS1.OT | Dock House / dkhos1.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| DKHOS2.OT | Dock House / dkhos2.SH | Partial | Done | N/A | Done | Retail damage branch at 0 HP; fire; Loads; no retail MM places it |
| DOCK1.OT | Dock / dock1.SH | Done | Partial | N/A | Done | Branch unverified; fitted ruin |
| FACT1.OT | Factory / fact1.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire; burns hard |
| FACTD.OT | Loading Dock / factd.SH | Done | Partial | N/A | Done | Branch unverified; fitted ruin; burns hard |
| FCTYA.OT | Factory / fctya.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire; burns hard |
| FCTYB.OT | Factory / fctyb.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; burns hard |
| FUEL.OT | Fuel Tank / fuel.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire; burns hard |
| HGRP1.OT | Hooch Blk 1 / hgrp1.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| HGRP2.OT | Hooch Blk 2 / hgrp2.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| HGRP3.OT | Hooch Blk 3 / hgrp3.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| HOOCH.OT | Hooches / hooch.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| HOUS.OT | House / hous.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| HOUSB.OT | House / housb.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| IND1.OT | Industrial / ind1.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; burns hard |
| IND2.OT | Industrial / ind2.SH | Partial | Partial | N/A | Done | Fitted ruin, no retail branch; fire; Loads; no retail MM places it; burns hard |
| OILW.OT | Oil Well / rig1.SH | Partial | Done | N/A | Done | Retail damage branch at 0 HP; fire; Loads; no retail MM places it; burns hard |
| RES1.OT | Residential / res1.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| RES2.OT | Residential / res2.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| SGRP1.OT | Slum Blk 1 / sgrp1.SH | Partial | Partial | N/A | Done | Fitted ruin, no retail branch; fire; Loads; no retail MM places it |
| SGRP2.OT | Slum Blk 2 / sgrp2.SH | Partial | Partial | N/A | Done | Fitted ruin, no retail branch; fire; Loads; no retail MM places it |
| SLUM01.OT | City Slum / slum01.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; City fill kit |
| SLUM02.OT | City Slum / slum02.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire; City fill kit |
| STORE.OT | Storage / store.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire; burns hard |
| SUPPLY.OT | Supply / supply.SH | Done | Partial | N/A | Done | Branch unverified; fitted ruin; burns hard |
| TWNBKA.OT | Town Blk 1 / twnbka.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| TWNBKB.OT | Town Blk 2 / twnbkb.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| TWNBKC.OT | Town Blk 3 / twnbkc.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| TWNBKD.OT | Town Blk 4 / twnbkd.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| TWNBKE.OT | Town Blk 5 / twnbke.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| TWNBKF.OT | Town Blk 6 / twnbkf.SH | Done | Partial | N/A | Done | Fitted ruin, no retail branch; fire |
| WNDMLL.OT | Windmill / wndmll.SH | Done | Done | N/A | Done | Retail damage branch at 0 HP; fire |
| **people/misc (animals, none are people)** | | | | | | |
| MOOSEA.OT | Moose / moosea.SH | Done | None | N/A | Done | Static prop; motion unverified |
| MOOSEB.OT | Moose / mooseb.SH | Done | None | N/A | Done | Static prop; motion unverified |
| WTRBUF.OT | Water Buffalo / wtrbuf.SH | Done | None | N/A | Done | Static prop; motion unverified; No retail MM places it |
| **other** | | | | | | |
| CRATER.OT | Crater / crater.SH | Done | N/A | N/A | Partial | Shape reads and projects (OT-1); retail draws live craters from the effect sheet; burns hard |
| CROPA.OT | Crops / cropa.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| CROPB.OT | Crops / cropb.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| DEST.OT | Destroyed / dest.SH | Done | Done | N/A | Done | Wreck for destroyed vehicles; fire/smoke |
| LOT.OT | Lot / base.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| ROAD.OT | Road, Lrg / road.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified; map-tied |
| ROAD2.OT | Road, Med / road2.SH | Partial | N/A | N/A | Partial | Contact box; stays as stood; HP unverified; map-tied; Loads; no retail MM places it |
| ROAD4.OT | Road, Sml / road4.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified; map-tied |
| ROADC.OT | Road, Curve / roadc.SH | Partial | N/A | N/A | Partial | Contact box; stays as stood; HP unverified; map-tied; Loads; no retail MM places it |
| ROCKA.OT | Rock A / rocka.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| ROCKB.OT | Rock B / rockb.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| ROCKBA.OT | Big Rock A / rockba.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| ROCKBB.OT | Big Rock B / rockbb.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| ROCKBC.OT | Big Rock C / rockbc.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| ROCKBD.OT | Big Rock D / rockbd.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| ROCKBE.OT | Big Rock E / rockbe.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| ROCKC.OT | Rock C / rockc.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| ROCKD.OT | Rock D / rockd.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| ROCKE.OT | Rock E / rocke.SH | Done | N/A | N/A | Partial | Contact box; stays as stood; HP unverified |
| TREE1.OT | (blank) / tree1.SH | None | N/A | N/A | None | Shapes absent from media; separate vegetation system |
| TREE2.OT | (blank) / tree2.SH | None | N/A | N/A | None | Shapes absent from media; separate vegetation system |
| ~CLEMT.OT | Tower / clemt.SH | Done | Done | N/A | Partial | Carrier island damage branch; island HP unverified |
| ~KITTT.OT | KittyHawk Twr / kittt.SH | Done | Done | N/A | Partial | Carrier island damage branch; island HP unverified |
| ~NIMZT.OT | Tower / nimzt.SH | Done | Done | N/A | Partial | Carrier island damage branch; island HP unverified |
| ~WASPT.OT | Wasp Tower / waspt.SH | Done | Done | N/A | Partial | Carrier island damage branch; island HP unverified |

### Counts (170 records)

| Column | Done | Partial | None | N/A |
|---|---|---|---|---|
| Imported | 157 | 11 | 2 | 0 |
| Animations | 72 | 69 | 3 | 26 |
| AI | 0 | 0 | 0 | 170 |
| Logic | 140 | 28 | 2 | 0 |

## Mission-side and other formats


Counts recounted from all.tsv by file extension (9668 rows; rows include `~`-prefixed variants and the
installed/ESA duplicates already deduped, so some counts exceed the first
approximations: PIC 4008, SH 1275, 5K 782, 11K 400, VDO/FBC 355 each, INF 270, PNT 53, CB8 43).
Grades are from docs/formats/coverage.md, docs/features.md, docs/formats/*.md and crates/tore-formats / tore-import.
Archive homes: M/MT/MM/MC/SEQ/CAM/PTS/BI/AI/HUD in FA_2.LIB; VDO+FBC only in disc1/FA_7.LIB;
INF only in disc2/FA_3.LIB (+1 in swpatch.lib); CB8 in FA_4C/FA_10/FA_10B/FA_11/FA_11B; PNT only in disc1/LHX/LHX1.LIB.
The importer (tore-import) reads FA_1 and FA_2 (FA_4B/4D optional music); FA_3, FA_7, FA_10/11, FA_4C are not read.

| Ext | Count | What it is | Import/decode | In-game use | M3 relevance | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| M | 517 | Mission files (text keyword grammar like MM): campaign missions (UKR01-50 + `~U01-50`, `~V`, `~E`, `~B`, `~K`, `~T` series, VIET, KURIL), EXTRA/BEXTRA, MULTI/BMULTI, TRAIN01, `~A<aircraft>.M` aircraft-parking, `~Q*`/`~F*NOTH` Quick Mission templates | Partial. Object-placement reader (`mission.rs`, shared with MM) plus a full Quick Mission template reader (`quick_template.rs`: waypoint2 routes, flags, nationality fields) for the `~Q*` family. Campaign mission scripting keywords (objectives, triggers, `code`, briefing links, win/lose conditions) not decoded | Quick Mission templates: Done (ground targets, routes, defenses fly). Campaign/EXTRA/MULTI/TRAIN missions: None | Critical (core of M3 loader) | coverage.md: "missions and object execution absent"; theater.md notes UKR01.M environment fields and `~UKR6.T2` aliases "future work"; quick-templates.md covers only `~Q*` |
| MT | 363 | Mission text: numbered `.section` blocks (briefing, success/failure debrief, per mission) | Partial. `MissionText::parse` (`mission_text.rs`) is generic over numbered sections | Only `QUICK.MT` sections 3/4 used (Quick Mission debrief). Campaign briefing/debrief sections: None | Critical (briefing + debrief) | debrief.md/spec/debrief.md; `DEBRIEF_DATA` in selection.rs lists only QUICK.MT; coverage.md says MT/campaigns "Not started" and "Raw Ukraine resources preserved" |
| MM | 75 | Theater layouts (airports, base objects per map; the 59 `~` variants plus 16 base) | Done for static layouts (all 75 parse, placements, environment/tmap fields) | Done: 16 theaters selectable; 59 variants developer-only (`--theater ~CODE`) | High (campaign missions reference `~UKR6.T2`-style aliases and layout variants) | coverage.md M/MM row; theater.md; features.md row 183-184 |
| MC | 21 | Campaign mission code modules: PE/PL i386 modules (same container as DLG/MNU/HUD), named by a mission's `code` keyword; UKR01/02/VIET03/K16/K17/U01..U34 etc. | None for content. `module.rs` can read PE/PL sections as inert data only; nothing consumes MC | None. Baseline: "Nothing reads them" | Critical risk: campaign missions may depend on native code, which AGENTS.md forbids executing; needs a research pass to recover player-visible behaviour (events, scripted spawns) into a spec | fa-builds-2026-10-06.md ("campaign mission modules", only timestamp diffs); native-strip.md 1128-1137: `code` keyword -> `.MC`, interceptor installed only if resource non-empty, "nonempty code resource stays unsupported/inert" |
| SEQ | 126 | Presentation sequences (intro/title/medal/promotion/death scenes): `U01I`/`U01O` etc. = per-mission intro/outro of the Ukraine campaign, `KMEDAL0-8`, `KPROM0-1`, `KDEAD/KLOST/KWON/KBAIL`, `TITLE95`, `G_INTRO`, `NF_INTRO` | None. Only names referenced (KBAIL.SEQ listed in `ejection.rs` RESOURCES, TITLE95.SEQ noted in menu.md as referencing a missing `^MF.11K`); grammar not decoded | None | High for campaign flow (per-mission intro/outro, medals, promotions, death/captured screens) | docs/formats/ejection.md, menu.md:97; no SEQ parser in crates |
| CAM | 6 | Per-theater campaign cameras/maps (BALTIC, EGYPT, KURILE, UKRAINE ...) | None (preserved raw only) | None | Medium-high (campaign map screen) | Appears only in fa-builds-2026-10-06.md and esa-installer.md category lists |
| TXT | 8 | Campaign text (per-theater BALTIC/EGYPT/KURILE/UKRAINE TXT, CREDITS, SHWPILOT) | None | None | Medium (campaign narration/briefing text, credits) | not referenced in crates |
| VDO | 355 | Video streams (disc1/FA_7.LIB) | None | None | Low-medium (explicitly "where media is complete" in the roadmap) | coverage.md: "CB8 / VDO / FBC / INF Not started. No video/reference playback"; no references in crates |
| FBC | 355 | Companion file to each VDO (one-to-one, same names e.g. AACA.VDO/AACA.FBC); likely frame/index/palette data | None (purpose not documented in repo) | None | Same as VDO | same; nothing documents the FBC layout - unknown |
| CB8 | 43 | Per-aircraft bitmaps on disc2 and FA_4C (e.g. `A7_D.CB8`, `117_M.CB8`); format and purpose undocumented in repo (likely reference-room media, unverified) | None | None | Low (encyclopedia/reference room) | coverage.md; no references in crates |
| INF | 270 | Reference-room ("encyclopedia") entries per aircraft/weapon/vehicle (`A10.INF`, `2S6.INF`...), in disc2/FA_3.LIB | None; FA_3 not imported | None | Medium (encyclopedia is named in M3 work list) | objects-and-shapes.md 1043: FA.EXE names strings beside the "reference room's `.INF` and picture strings"; no INF reader in crates |
| PNT | 53 | Models/pictures for the bundled LHX helicopter title (disc1/LHX/LHX1.LIB), not FA content | None | None | None (extraction commands exclude `disc1/LHX/*`) | EXTRACTION.md `--exclude-archive 'disc1/LHX/*'` |
| PTS | 37 | Per-aircraft PT companion modules (purpose not documented in repo; unknown) | Partial: preserved and carried as dependency; semantics not decoded | None (only for the two reviewed aircraft, as preserved data) | Low-medium (aircraft batch import in M3) | coverage.md PT/PTS/SH/HUD row: "PTS and complete native HUD/shape VM remain unimplemented"; "PTS module semantics ... remain open" |
| HUD | 46 | Per-aircraft HUD panel module (PE module) | Partial: `Hud::parse` reads panel name, title, button colors for 12 aircraft; 34 other layouts unused | Cockpit HUD panel picture/colors used in flight; native HUD composition not decoded | Medium (remaining aircraft) | hud.rs; fa-builds-2026-10-06.md rows (12 used, 34 "layouts of aircraft the roster does not use") |
| AI | 9 | AI scripts (F, H, B, F117, AC130, LARGE, LINER, MOTH, HYDRO) | Partial research: source labels and constants studied, `chance` literals matched against BI | Not interpreted. Behaviours recreated from prose spec (spec/ai.md B01-B47, executable-confirmed rules, fitted/opinionated values); AI uses authored sensors/controllers | Medium: needed for mission-scripted behaviour, but existing hybrid AI covers combat; campaign roles/strike unauthorized in places | docs/formats/ai.md ("not a proposal to execute retail scripts or reproduce the original VM"); AGENTS.md; no AI interpreter in tore-sim |
| BI | 9 | Compiled AI program modules matching the 9 AI scripts | None (constants cross-checked by byte patterns only); never executed | None | Same as AI | docs/formats/ai.md "corroborates constants only" |
| MNU / DLG | 12 / 92 | Menu trees and dialogs (includes `MC_*` Mission Creator dialogs, `CHOOSEM`, `BRIEFSCR`) | Partial (see coverage.md) | Main/Quick/ordnance menus Done; campaign/briefing/MC dialogs None | High (briefing, campaign, pilot record, mission creator screens) | coverage.md DLG/MNU rows; menu.md; MC_* dialogs listed in fa-builds baseline as not kept/used |
| BIN | 6 | INSIGMAP (insignia map), MIX* (audio mixer tables) | not verified here | MIX tables not confirmed used | Low | referenced only by name in app assets README |
| HGR | 2 | Hangar scenes (H_AIRB, H_AIRB2), carrier/airfield hangar art | None | None | Low (carrier ops / hangar screen) | no references in crates |
| MUS / XMI | 9 / 78 | Music score grammar / MIDI music | MUS Partial (all nine parsed); XMI excluded by scope | Free-flight music Done; campaign/briefing stingers not mapped | Low | coverage.md |
| PIC/SH/5K/11K/FNT/PAL/T2/LAY | n/a | Art, shapes, sounds, fonts, terrain, weather | Partial/Decoded per coverage.md | Done where listed | Supporting (briefing/map/debrief art needs more PIC; campaign screens need many unimported PICs) | coverage.md |
| Pilot/campaign saves | n/a | Not an archive resource: player-generated save/pilot files (and Pro Mission Creator data) | None | None. Existing persistence is Quick Mission "remember" settings and replay/checkpoint formats only | Critical (pilot record, progression, saves) | coverage.md "MT / campaigns / saves / Pro Mission Creator: Not started"; only `quick_mission/remember.rs` found |

### M3 import gaps

- Campaign `.M` scripting grammar is not decoded (objectives, triggers, win/lose, briefing/debrief links, `code` keyword); only placements and the `~Q*` template schema are read. 517 M files, about 400 of them campaign/extra/multi.
- `.MC` (21 mission code modules) have no reader semantics and nothing consumes them; if campaign missions need them, that is a research-mode task to recover behaviour into docs/spec without executing the PE code.
- `.MT` is parsed generically but only `QUICK.MT` is wired; campaign briefing sections and per-mission debriefs are unmapped; section meanings other than 3/4 are not documented.
- `.SEQ` (126) has no parser or grammar notes: Ukraine per-mission intro/outro, medals, promotions, death/captured/won scenes depend on it.
- `.CAM`/`.TXT` campaign map and narrative resources are unparsed; the campaign map/briefing map screen has no data path.
- Campaign/pilot-save structures (pilot record, progression state, medals, saves) are entirely unresearched and absent from the repo; Pro Mission Creator file format is also unresearched (MC_* dialogs only inventoried).
- Per-title campaign data across USNF '97, ATF Gold, NATO Fighters, Marine Fighters, Pro Mission Creator has no census (coverage.md says other title columns "not started"); only the supplied Anthology install is inventoried.
- Importer does not read FA_3 (INF), FA_7 (VDO/FBC), FA_4C/FA_10/FA_11 (CB8); VDO/FBC/CB8/INF layouts are undocumented (FBC purpose unknown), so encyclopedia and video are blocked at the format level.
- AI/BI are recreated from spec, not interpreted; mission-specific behaviours (campaign roles, strike orders, scripted events) beyond Quick Mission are not specified (AI attacking ground targets still gated by "next rounds" in the roadmap).
- Campaign theater aliases (`~UKR6.T2` etc.) and 59 MM variants need runtime resolution; carrier ops/hangar (HGR) and the 34 unused HUD layouts and PTS semantics gate the remaining aircraft batches.
