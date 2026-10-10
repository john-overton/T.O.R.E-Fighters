# Surface defenses

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation contract for the surface-AI round (SAMs, AAA, flak, ship and
vehicle guns), started 2026-10-10. This file is built up slice by slice. The
section below, the AAA tuning table, belongs to slice G1; the rest of the
contract is written by slice R1.

## AAA tuning

Provenance: **opinionated (John, 2026-10-10)** for the rules and the magazine
reload times, **retail data** for the values marked `R`, **fitted** (agent
choices after real-world figures) for the values marked `F`.

For a player: anti-aircraft guns and flak fire real, physical shells. Every
gun type has its own rate of fire, magazine size and reload time, so a Shilka
chews through a magazine in seconds and goes quiet to reload, while a flak
battery lobs a shell every few seconds with no tracer. Magazines empty on the
guns the mission creator's defended targets stand on, and a supply truck close
by (0.1 mile) fills them again.

What the retail game gives each gun is a short burst, a pause and an unlimited
stock: a ZSU-23-4 fires 4 rounds in a quarter second, waits one second and
never runs out. TORE keeps the retail pause and opening barrage where they
fit, replaces the rest with real-world figures, and labels every value:

- **Rate** is the cyclic rate in rounds a minute while a burst fires (fitted
  after the real gun). A single-shell gun (flak, tank guns) has no burst, so
  its rate is one shell per loading cycle.
- **Burst rounds** is how many physical rounds leave before the pause, and
  the burst time follows from the rate. **Pause** is the retail `reloadT`
  (quarter seconds) where marked `R`.
- **Opening** is the retail startup barrage: the first shots at a new target
  (the two flak guns fire eight).
- **Magazine** is the rounds a mount fires before it must reload (fitted;
  retail stock is unlimited). **Magazine reload** is John's rule: 60 seconds
  for towed guns and small vehicles (including infantry and tanks), 120
  seconds for self-propelled AA guns and ship guns. It replaces the 300 and
  600 second figures the first plan proposed.
- **Damage per round.** The tuned guns fire many more rounds a second than
  retail. Each retail game round's damage is split over `1/N` physical
  rounds (`actualRoundsPerGame`), so damage per second of sustained fire
  matches retail within about 12 percent (the last column). Guns whose rate is
  close to retail keep the retail damage per round. Tank guns fire slower than
  retail and keep the retail damage, so they do less damage per second.
- **Muzzle velocity** is fitted after the real gun; the shell keeps the
  retail record's range and life, so reach is the muzzle velocity times the
  life. Both flak guns reach their ceilings (15,000 and 25,000 feet) well
  inside their life (10 and 15 seconds). Flak shells burst on a time fuze or
  within the fuze radius, so the retail fire-zone range beyond a shell's
  reach is not used.
- **Tracer**: radar and visual AA guns mark every third round; flak, tank
  guns and small arms have none (flak shells burst with a flash of light
  instead).
- The M163 has its own Vulcan row though it shares `PHALANX.JT` with the
  ships' Phalanx. The Butler class stays on the retail `AAA30.JT` record (no
  Bofors row).

Numbers in the table are generated from `TABLE` in
`crates/tore-sim/src/combat/surface_guns.rs`; a test fails if they drift.
Regenerate with
`TORE_UPDATE_SURFACE_GUNS_DOC=1 cargo test -p tore-sim surface_guns_doc`.
Tags: `R` retail record value, `F` fitted, `O` opinionated (John).

<!-- aaa-tuning-table:start -->
| Gun | Record (units) | Retail burst / pause s / opening / muzzle ft/s | Rate rpm | Burst rounds (s) | Pause s | Opening | Magazine | Magazine reload s | Muzzle ft/s | Tracer | Damage per round | Damage per second vs retail |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | ---: |
| ZSU-23-4 Shilka, 4 x 23 mm 2A7 | ZSU23 | 4 in 0.25 s / 1.0 s / 0 / 3,666 | 3,400 F | 99 F (1.75 s) | 1.0 R | 0 R | 2,000 F | 120 O | 3,180 F | every 3rd | 1/11 of retail F | 1.02 |
| 2S6 Tunguska, 2 x 30 mm 2A38M | 2S6 | 4 in 0.25 s / 1.0 s / 0 / 3,666 | 5,000 F | 105 F (1.25 s) | 1.0 R | 0 R | 1,904 F | 120 O | 3,150 F | every 3rd | 1/15 of retail F | 0.97 |
| M163 VADS, 20 mm M168 Vulcan | PHALANX (M163) | 6 in 0.25 s / 1.0 s / 0 / 3,666 | 3,000 F | 50 F (1.0 s) | 1.0 R | 0 R | 1,100 F | 120 O | 3,380 F | every 3rd | 1/5 of retail F | 1.04 |
| ZSU-57-2, twin 57 mm S-68 | ZSU57 (ZSU57, ZIF31) | 4 in 0.5 s / 3.0 s / 0 / 3,666 | 240 F | 5 F (1.25 s) | 3.0 R | 0 R | 300 F | 120 O | 3,280 F | every 3rd | retail R | 1.03 |
| Phalanx CIWS, 20 mm M61A1 | PHALANX (NIMZ, KITT, CLEM, WASP, IOWA, TICON) | 6 in 0.25 s / 1.0 s / 0 / 3,666 | 4,500 F | 150 F (2.0 s) | 1.0 R | 0 R | 1,550 F | 120 O | 3,600 F | every 3rd | 1/10 of retail F | 1.04 |
| AK-630 class, 30 mm six-barrel | AAA30 (KIROV, SOVR, KIEV, SARAN, BUTLER) | 4 in 0.25 s / 1.0 s / 0 / 3,666 | 4,000 F | 150 F (2.25 s) | 1.0 R | 0 R | 2,000 F | 120 O | 2,950 F | every 3rd | 1/15 of retail F | 0.96 |
| AK-230 class, twin 30 mm | AAA30BAD (TYPE69, KNOX, JIANC, JIANE, KRIVAK, CYCL, PMORN) | 4 in 0.25 s / 1.0 s / 0 / 3,666 | 2,000 F | 42 F (1.25 s) | 1.0 R | 0 R | 1,000 F | 120 O | 3,440 F | every 3rd | 1/6 of retail F | 0.97 |
| 61-K 37 mm M1939 | M1939 | 4 in 0.5 s / 3.0 s / 0 / 3,960 | 160 F | 6 F (2.25 s) | 3.0 R | 0 R | 200 F | 60 O | 2,890 F | every 3rd | retail R | 1.00 |
| 61-K 37 mm M1939, barrage zone | A_M1939 | 4 in 0.5 s / 3.0 s / 0 / 3,960 | 160 F | 6 F (2.25 s) | 3.0 R | 0 R | 200 F | 60 O | 2,890 F | every 3rd | retail R | 1.00 |
| 52-K 85 mm (KS-12) flak | KS12 | 1 in 0.5 s / 4.0 s / 8 / 3,520 | 14 F | 1 R (single shot) | 4.0 R | 8 R | 60 F | 60 O | 2,620 F | none | retail R | 1.06 |
| KS-19 100 mm flak | KS19 | 1 in 0.5 s / 4.0 s / 8 / 4,400 | 14 F | 1 R (single shot) | 4.0 R | 8 R | 60 F | 60 O | 2,950 F | none | retail R | 1.06 |
| M256 120 mm tank gun | M1 | 1 in 0.25 s / 4.0 s / 0 / 5,866 | 6 F | 1 R (single shot) | 9.75 F | 0 R | 34 F | 60 O | 5,866 R | none | retail R | 0.43 |
| 2A46 125 mm tank gun | T72 (T72, T80, T90) | 1 in 0.25 s / 4.0 s / 0 / 5,866 | 8 F | 1 R (single shot) | 7.25 F | 0 R | 22 F | 60 O | 5,866 R | none | retail R | 0.57 |
| 2A42 30 mm | BMP2 | 2 in 0.5 s / 3.0 s / 0 / 3,666 | 300 F | 20 F (4.0 s) | 3.0 R | 0 R | 500 F | 60 O | 3,150 F | every 3rd | 1/5 of retail F | 1.00 |
| KPVT 14.5 mm | BTR80 | 2 in 0.5 s / 3.0 s / 0 / 3,666 | 600 F | 10 F (1.0 s) | 3.0 R | 0 R | 500 F | 60 O | 3,280 F | every 3rd | 1/5 of retail F | 0.88 |
| M2HB .50 cal | M113 | 2 in 0.5 s / 3.0 s / 0 / 3,666 | 500 F | 21 F (2.5 s) | 3.0 R | 0 R | 2,000 F | 60 O | 2,910 F | every 3rd | 1/7 of retail F | 0.95 |
| M242 25 mm | M2 | 2 in 0.5 s / 3.0 s / 0 / 3,666 | 200 F | 20 F (6.0 s) | 3.0 R | 0 R | 300 F | 60 O | 3,600 F | every 3rd | 1/4 of retail F | 0.97 |
| Squad small arms | SMLARMS (TROOPS) | 4 in 0.25 s / 1.0 s / 0 / 3,666 | 600 F | 5 F (0.5 s) | 1.0 R | 0 R | 1,000 F | 60 O | 3,000 F | none | retail R | 1.04 |
<!-- aaa-tuning-table:end -->
