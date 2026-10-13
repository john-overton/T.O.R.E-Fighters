# Stores on pylons

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-10-12 (slice BP, bug B3). John reported that
bombs and missiles left from one fixed point ahead and to the right of the
view, and asked for them to drop from their hardpoints, with the pylons and
loaded stores drawn on the aircraft ("I'd love positions and drawn pylons").
Slice PY2 (2026-10-13) made every wing pylon 2 ft deep, put fuselage and
centreline stores on 4 inch rails and wingtip missiles on the tip itself, hung fuselage pairs one
behind another and moved the A-4E's Mk 82s to a centreline rack (John's
morning list, items 25 to 27).

## What the player sees

- **Every store leaves from where it hangs.** A missile, bomb or rocket
  appears exactly where it was drawn a moment before, in the aircraft's
  frame: under its own pylon or rail, at its place on a rack, wingtip
  missiles on the tip, internal bay stores below the bay. Nothing jumps at
  release.
- **Wing pylons are 2 ft deep.** A wing store hangs with its top 2 ft
  under the wing above it, as real pylons are 15 to 30 inches.
- **Fuselage and centreline stores sit on short rails**, their tops about
  4 inches under the body, as launchers and conformal stations are.
- **Wingtip missiles sit on the tip.** No pylon or rail is drawn under the
  tip: the missile's centre line lies on the tip's edge, as an F-16's or
  F/A-18's tip AIM-9 does.
- **Fuselage pairs hang one behind another**, as the F-14's Phoenixes and
  the F-15's and F-4's Sparrows do, and the front pair leaves first.
- **Paired stations alternate.** Two AIM-9s on the wingtips leave left, then
  right; four Mavericks on a wing pair leave left, right, left, right. The
  player, the AI and every seat of a network game release the same way, so a
  remote player's missile appears at that player's pylon.
- **Pylons and loaded stores are drawn** on every aircraft that carries a
  combat loadout, in outside views and the flown aircraft's own views of
  itself, and they empty as stores leave. A pylon stays when its stores are
  gone. In the cockpit view the stores hide with the rest of the flown
  aircraft's model.
- The bomb's fall, sag and drop after release (the weapons pass behaviour) are
  unchanged; only the point they start from moves.

## Rules

| Rule | Value | Provenance |
| --- | --- | --- |
| Station position | The source hardpoint record's x (right) and z (forward), a third of a foot a unit, from the aircraft's reference point ([table](../formats/aircraft-ordnance.md#hardpoint-positions)); its height is the pylon's (below) | spec-derived (the height: opinionated, PY2) |
| Pylon depth | A wing store's top (its lugs, the top of its body) hangs 2 ft under the model's underside over its pylon; a fuselage or centreline store's 4 inches (0.33 ft) under it, on a launcher rail; a rack's beam (0.3 ft) sits between, so a rack's stores hang 2.3 ft (wing) or 0.63 ft (body) under. The underside is the lowest face of the clean model up to 12 ft above the record, straight above the pylon or up to 3 ft inboard, read 0.6 ft either side of it and the lowest taken, so a store beside a nacelle or a body's flank hangs clear of it. A station with nothing of the model above it keeps the record's height | opinionated, John 2026-10-13 ("pylon depth matches real pylons (15-30 inches): 2 ft universally", then the same day: wing pylons 2 ft, fuselage and centreline stations short rails about 4 inches deep); the clearance rule and the samples are agent decisions |
| Wingtip | A wingtip store's centre line lies on the tip's edge: as far out as the model's outermost point beside the record (within 4 ft inboard and 0.3 ft outboard of it, 2.5 ft fore and aft, 3 ft up or down), at the middle height of that edge. No pylon or rail is drawn. With no model read, the record's position | opinionated, John 2026-10-13 ("the missile's centre line sits on the wingtip geometry, no gap"); the search box is an agent decision |
| Where the underside is read | When the loadout is built, from the aircraft shape the simulation already reads (`<ID>.SH`, at the debris scale): over the record, over a two-pylon wing's inboard and outboard pylons (1.4 ft either side), and 7 ft ahead of and behind the record; a tandem pylon's surface is interpolated between the record and the sample on its side. The heights travel with the station (`Rack::over`), into checkpoints and replays | agent decision |
| Released from | The point the store hangs at: its side, its pylon, its place on the pylon's rack and the pylon's height (`Rack::store_point`), for the player, the AI, every network seat and replays. The bomb sight predicts from the same point | opinionated, John 2026-10-13 ("a store never jumps at release") |
| Store size | Length, the height of its lugs above its origin (half its body diameter) and its width across its fins, from the [real sizes](#store-sizes) table by shape name; a pod's or tank's fitted body; 8 by 0.9 ft for a store not listed. The origin is taken as the store's middle | agent decision |
| Paired record | Hardpoint flag 0x4: the record's position is the right pylon, its mirror the left | spec-derived ([release state](../formats/weapons.md#release-state)) |
| Also paired | Every Wing and Wingtip record without flag 0x4 (the Mi-24's AT-2 rails) | agent decision, 2026-10-12 |
| A one-sided wing row by the centreline | A Wing record without flag 0x4 less than 3 ft out (the A-4E's Mk 82 row, 2.7 ft out, which retail releases from one side) is a centreline rack at x 0 ([A-4E](#the-a-4e)) | agent decision, 2026-10-13 |
| Fuselage pairs one behind another | A Fuselage record whose side holds two stores, or four or more, has two pylons on that side, one ahead of the other: their stores' middles are half a store length and 0.5 ft either side of the record's position, so they clear by 1 ft. The front pylon's store leaves first, so four leave left front, right front, left rear, right rear. Four or more a side: two side by side on each pylon's rack | opinionated, John 2026-10-13 ("fuselage missile pairs one behind another: fix"); the gap and the front-first order are agent decisions (the real aircraft's firing order is not published; front first keeps a rear missile from passing under a loaded front one) |
| Fuselage pair on the centreline | A paired Fuselage weapon record at x 0 holding two or more (the F-4E's and F-4G's four Sparrows) hangs 3 ft either side of the centreline, in the Sparrow wells | agent decision, 2026-10-13 (fitted) |
| Never paired | An internal gun, whatever its flags; a record on the centreline (nothing to mirror) | agent decision |
| Which side | The mirror (left) when the whole releases left on the station after this one (rounds over `actualRoundsPerGame`) is odd; so the left side empties first and the right holds the odd store | retail parity rule; whether retail counts before or after the debit is unknown, the order is fitted |
| Unlimited ammunition cheat | The count never moves, so the side follows the release's projectile number (alternates) | agent decision |
| Who | The player, the AI (strike stores, missiles and seeker missiles alike) and every network seat. Retail releases a distant AI aircraft from its centre; T.O.R.E releases every aircraft from its pylon | agent decision |
| Guns | Keep their muzzle (gun mount, AC-130 barrel) | unchanged |
| Bomb packs | Each bomb of a pack keeps its pack stagger, added to the pylon's side | unchanged |
| Two pylons a wing | A Wing record whose side holds two stores, or four or more, has two pylons on that wing, 1.4 ft either side of the record's position (2.8 ft apart, clearing a Maverick's 2.4 ft fins). Slot 0 is inboard, slot 1 outboard, further slots alternate; the outboard pylon's store leaves first, so four Mavericks leave left outboard, right outboard, left inboard, right inboard. Each store leaves from its own pylon. Three on a side hang on one pylon's triple ejector rack, and Fuselage, Centerline and Wingtip records keep one pylon a side | opinionated, John 2026-10-12 ("four stores on a wing row: dual pylons on the wing"); the spread and the rule for other counts are agent decisions |
| Rockets from a two-pylon side | Leave from the pod that is firing (the outboard pod first) | agent decision |

**The bomb sight aims from the pylon the next store leaves** (John,
2026-10-12, slice BP3). The CCIP pipper, the level T and the rocket pipper
predict the release from the pylon of the selected station the next store
will leave, by the same rule the release uses (`Station::release_point`):
the side from the whole releases left on the station once that store has
gone, odd for the mirror (left), even for the record's own position (right),
and on a two-pylon wing side the pylon that store hangs from. So with four
Mk 82s on a wing row the pipper shows the left outboard bomb's fall, then
the right outboard, then the inboard ones. With unlimited ammunition the release's projectile number picks the
side, as the release does. The AI's release gate reads the same solution from
the pylon its next store leaves, so an AI bomb is judged from where it
drops (agent decision: the AI's next side is worked out from its weapon
service's rounds before the debit, which is what the release then debits).

| Sight | Predicts from | Provenance |
| --- | --- | --- |
| CCIP (bombs) | The next store's pylon, pack offset 0 | opinionated, John 2026-10-12 |
| Level T (bombs) | The CCIP's release point, so the same pylon | opinionated, John 2026-10-12 |
| Rocket pipper | The next rocket's pod | opinionated, John 2026-10-12 |
| AI release gate | The AI's next store's pylon | agent decision |
| Guided weapon sights, gun sights | Unchanged (the aircraft, the gun's muzzle) | unchanged |

## Drawing (opinionated, John 2026-10-12)

Retail draws no stores on aircraft (the A2G pass recorded this as decision 20,
"stores on pylons stay undrawn"); John reversed that decision on 2026-10-12.
Every number below is fitted (agent decision).

| Item | Rule |
| --- | --- |
| Stations drawn | External ones (Centerline, Fuselage, Wing, Wingtip) that held something at mission start; not guns or internal bays |
| Store look | The weapon's own shape (the same mesh it flies with after release) refitted to the real store's size ([store sizes](#store-sizes)). Rocket pods: a grey-green eight-sided pod of the real pod's size (LAU-61 5.5 by 1.38 ft, LAU-10 8.3 by 1.25, B-8M1 8.9 by 1.71, B-13L 11.7 by 1.35; any other 8.5 by 1.4). Fuel tanks: a light grey pod 13 ft by 2 ft at 2,000 lb of fuel, scaled by the cube root of its fuel (a 330 US gallon tank is 13.3 by 2.05 ft; the real one is about 12.9 by 2.1). A store with no shape: a plain body 8 ft by 0.9 ft |
| Stores on one pylon | One at the pylon point. Two side by side, 0.55 store widths (fins included) either side. Three as a triple ejector rack: two shoulders and one 0.95 widths below. Four as two side-by-side pairs one behind another (a multiple ejector rack), the front pair first. More: triple racks one behind another, 1.05 store lengths apart. The layout keeps the loaded count's slots; the highest slot empties first. The simulation places them ([released from](#rules)) and the drawing hangs them there |
| Pylon | A grey slab 0.35 ft thick, from the top of the store's body (its lugs, not its fin tips) up into the model's underside (2 ft, see [pylon depth](#pylon-depth)), 0.02 ft into it (retail wings are single faces, so more would show on top). Its length is 0.55 store lengths, at least twice its depth and at most 0.85 store lengths (1.5 to 10 ft), cut short where the underside ends (a pylon does not run out ahead of a leading edge), and its top follows the underside from end to end. The underside is the lowest face of the clean model up to 12 ft above the lugs, straight above or up to 3 ft inboard |
| Rack | Two or more stores on one pylon hang from a beam 0.3 ft deep over them, as wide as the side by side pair and as long as the racks one behind another; a post runs down to each triple rack's lower store |
| Wingtip | Nothing drawn under a wingtip store ([rules](#rules)) |
| Edge rail | A wing station's store with nothing of the model above it, beside a wing's edge less than 4 ft inboard, hangs on a rail from 0.3 ft inside the tip's edge out over the store, as deep as both the tip's edge and the store's upper half, 0.6 store lengths long (3 to 9 ft). With no tip beside it, a 0.35 ft rail |
| Colours | Pylons and racks grey (0.5, 0.52, 0.54); weapon shapes keep their palette colours |

Where the stores come from:

- **Single player, the host, AI aircraft:** the render snapshot carries each
  aircraft's stations and their stores left ([`AircraftPose::stores`]).
- **Network clients (protocol 27, slice BP3, John 2026-10-12):** a client
  draws every aircraft's pylons and stores from its own copy of the
  aircraft's loadout (the session's copy of the mission, which every game
  builds the same), as the loadout built it until the host says otherwise.
  The host sends a **Stores** event for an aircraft when what it draws
  changes (a release that empties a whole store, a jettison, a rearm): the
  aircraft's drawn stations with the whole stores still on each. A player
  seated later, a resumed seat after a migration and an observer are told
  every aircraft not as built when they start. The client puts each told
  state at its tick, so a station empties when the picture reaches the
  release, the same moment the store appears as a weapon. The seat's own
  aircraft is still drawn from its cockpit readout, which is newer. A
  Stores event is about 14 bytes for a fighter's six stations and costs
  nothing while nothing changes ([the protocol](../formats/net-protocol.md#protocol-27-stores)).
- **Mission replays (slice BP3, John 2026-10-12):** a recording keeps each
  aircraft's drawn stations the first time it is recorded and their whole
  stores whenever one empties, as `stores.*` events; the viewer draws each
  aircraft's stores at the tick it shows, forward, backward and after a
  seek. A converted network capture and an observer's live recording keep
  them too. A recording made before them has none, so its aircraft carry no
  stores ([replays](../REPLAYS.md#stores)).

## Store sizes

John, 2026-10-12: "are the weapon renders from retail accurate? They seem big
and bulky. Check weapon renders to make sure they are realistic sizes."
Measured with `tore-app --stores-sizes` (every weapon shape and every
aircraft, as drawn) and `--stores-size-sheet` (silhouettes, retail under
refitted, on a one-foot grid):

- **The aircraft are drawn at their real size**, within about 10 percent:
  F/A-18D 56.0 ft long (real 56.1) and 36.3 ft across the tips (37.5), F-16C
  49.0 by 34.0 (49.4 by 32.7 with tip missiles), F-14 60.0 (62.75), A-10 48.7
  by 54.0 (53.3 by 57.5), Su-25 50.0 (50.9), MiG-29 56.0 (56.8), AV-8 50.0
  (46.3), A-4E 40.0 (40.1).
- **The retail weapon shapes are about the right length but 1.3 to 2 times
  too thick.** Shape coordinates are whole source units of a third of a foot,
  so the thinnest body a shape can draw is 0.67 ft across; an AIM-9 is
  0.42 ft. Bodies and fins are drawn larger still: a Mk 82's body 1.33 ft
  (real 0.9) with 2.0 ft fins (1.27), a Maverick's 1.33 ft (1.0) with 3.3 ft
  wings (2.36), an AIM-54's 2.0 ft (1.25). Lengths are within 10 percent,
  except the dispensers: CBU-87 and CBU-97 drawn 11 and 13 ft (real 7.7),
  Mk 20 10.7 ft (7.7), RBK-250 and RBK-500 11 ft (6.4 and 8.2); the FAB-250
  is drawn short (5.0 ft, real 6.4). There is no unit mix-up: missiles, bombs
  and aircraft all use the same third of a foot, and no shape header carries a
  different scale exponent.
- **The fitted pylon bodies were too long or too fat** for some pods (a
  LAU-61 drawn 8.5 ft, real 5.5).

**Refit (opinionated, John 2026-10-12; rule an agent decision).** Every
listed shape is refitted, on the pylon and in flight, so a store does not
change size when it leaves (the in-flight shapes are wrong by the same
measure). Each axis is mapped separately: the length scales about the
store's origin (its release point) to the real length; across, the body
(the second thinnest of nine cross-sections between a quarter and three
quarters of the length) scales to the real diameter and the fin tips to the
real span, linearly between, so a thin fin stays thin. A shape several
weapons share takes the size of the store it depicts. Unlisted shapes
(surface-launched missiles, the AT-12, the AAM-L) keep their retail size.

| Shape | Weapons | Length ft | Body ft | Span ft | Source |
| --- | --- | ---: | ---: | ---: | --- |
| `SIDEM.SH` | AIM-9M | 9.35 | 0.42 | 2.07 | US Navy fact file; AIM-9L/M 2.85 m, 127 mm, 0.63 m |
| `SIDEB.SH` | AIM-9B | 9.3 | 0.42 | 1.83 | AIM-9B 2.83 m, 127 mm, 0.56 m |
| `AM9X.SH` | AIM-9X | 9.9 | 0.42 | 1.47 | Raytheon; 3.02 m, 127 mm, 0.45 m |
| `AMRAM.SH` | AIM-120 | 12.0 | 0.58 | 1.75 | US Air Force fact sheet; 3.66 m, 178 mm, AIM-120A/B 0.53 m |
| `SPAR.SH` | AIM-7, AIM-7E (and the SS-N-9 and Sea Sparrow records) | 12.0 | 0.67 | 3.3 | US Navy; 3.66 m, 203 mm, 1.0 m |
| `PHENX.SH` | AIM-54 | 13.0 | 1.25 | 3.0 | US Navy; 3.96 m, 380 mm, 0.91 m |
| `MICA.SH` | MICA | 10.2 | 0.53 | 1.83 | MBDA; 3.1 m, 160 mm, 0.56 m |
| `M55.SH` | R-550 Magic | 9.0 | 0.52 | 2.17 | Matra; 2.75 m, 157 mm, 0.66 m |
| `M53.SH` | Super 530D | 12.5 | 0.86 | 2.03 | Matra; 3.8 m, 263 mm, 0.62 m |
| `APHID.SH` | AA-8 (R-60) | 6.9 | 0.39 | 1.28 | Vympel; 2.09 m, 120 mm, 0.39 m |
| `ARCHR.SH`, `AA11B.SH` | AA-11 (R-73) | 9.5 | 0.56 | 1.67 | Vympel; 2.9 m, 170 mm, 0.51 m |
| `ALAMO.SH` | AA-10 (R-27) | 14.8 | 0.75 | 3.2 | Vympel R-27ET; 4.5 m, 230 mm, 0.97 m tail |
| `AMSKI.SH` | AA-12 (R-77) | 11.8 | 0.66 | 2.2 | Vympel; 3.6 m, 200 mm, 0.68 m grid fins |
| `ATOLL.SH` | AA-2 (K-13) | 9.2 | 0.42 | 1.73 | 2.8 m, 127 mm, 0.53 m |
| `ACRID.SH` | AA-6 (R-40) | 20.0 | 1.02 | 4.76 | about 6.1 m, 310 mm, 1.45 m |
| `AMOS.SH` | AA-9 (R-33) | 13.6 | 1.25 | 3.87 | Vympel; 4.15 m, 380 mm, 1.18 m |
| `STGR.SH` | FIM-92 | 5.0 | 0.23 | 0.3 | 1.52 m, 70 mm |
| `MAVRK.SH` | AGM-65A, AGM-65G | 8.2 | 1.0 | 2.36 | US Air Force fact sheet; 2.49 m, 305 mm, 0.72 m |
| `HARM.SH` | AGM-88 (and the AGM-45, 10 ft real, which retail draws with this shape) | 13.7 | 0.83 | 3.7 | US Navy; 4.17 m, 254 mm, 1.12 m |
| `HARPN.SH` | AGM-84A | 12.6 | 1.12 | 3.0 | US Navy; air-launched 3.84 m, 343 mm, 0.91 m |
| `SLAM.SH` | AGM-84E | 14.6 | 1.12 | 3.0 | US Navy; 4.45 m |
| `KERRY.SH` | AS-7 (Kh-23) | 11.6 | 0.9 | 2.58 | 3.53 m, 275 mm, 0.785 m |
| `KEDGE.SH` | AS-14 (Kh-29) | 12.8 | 1.25 | 3.6 | 3.9 m, 380 mm, 1.1 m |
| `KIKBK.SH` | AS-16 (Kh-15) | 15.7 | 1.49 | 3.0 | 4.78 m, 455 mm, 0.92 m |
| `AS12.SH` | AS.15TT | 7.1 | 0.62 | 1.85 | Aerospatiale; 2.16 m, 187 mm, 0.56 m |
| `AS30.SH` | AS.30L | 12.0 | 1.12 | 3.3 | 3.65 m, 342 mm, 1.0 m |
| `EXO.SH` | AM-39 Exocet | 15.4 | 1.15 | 3.6 | MBDA; 4.69 m, 348 mm, 1.1 m |
| `SWATR.SH` | AT-2 | 3.8 | 0.48 | 2.17 | 1.16 m, 148 mm, 0.66 m |
| `MK82.SH`, `MK82A.SH` | Mk 82, Mk 82 P3, Mk 82 AIR | 7.3 | 0.9 | 1.27 | US Navy; 2.22 m, 273 mm; fin span about 15 in (approximate) |
| `SNAKE.SH` | Mk 82 Snakeye | 7.4 | 0.9 | 1.27 carried, 4.0 released | as Mk 82; retail draws its retarding fins open, as they are after release; the open span is approximate |
| `MK84.SH` | Mk 84 | 10.8 | 1.5 | 2.1 | 3.28 m, 458 mm; fin span about 25 in (approximate) |
| `MK20.SH` | Mk 20 Rockeye | 7.7 | 1.1 | 1.45 | 2.34 m, 335 mm |
| `CB87.SH`, `CB89.SH` | CBU-87, CBU-97 | 7.7 | 1.3 | 1.9 | SUU-65 dispenser 2.34 m, 396 mm; fin span approximate |
| `PAVE.SH` | GBU-10 | 14.2 | 1.5 | 5.4 | 4.32 m, 1.65 m wings |
| `GB27.SH` | GBU-27 (and the GBU-29 and GBU-30 retail draws with it) | 13.9 | 1.2 | 5.4 | 4.24 m, BLU-109 370 mm |
| `GB28.SH` | GBU-28 | 19.2 | 1.21 | 2.2 | 5.84 m, 370 mm; fins approximate |
| `F250.SH`, `RBK2.SH` | FAB-250, RBK-250 | 6.4 | 1.07 | 1.35 | M-62 1.96 m, 325 mm |
| `F500.SH` | FAB-500 | 7.9 | 1.3 | 1.6 | M-62 2.4 m, 400 mm |
| `F1000.SH` | FAB-1000 | 9.4 | 2.07 | 2.6 | about 2.87 m, 630 mm |
| `RBK5.SH` | RBK-500 | 8.2 | 1.48 | 1.8 | 2.5 m, 450 mm |

Numbers are public data (manufacturers' and services' published figures, as
collected by Wikipedia and FAS Military Analysis Network), rounded; spans
marked approximate are estimates. Pod sizes are in the drawing table above.

## Pylon depth

Before PY2 a store hung at its hardpoint record's height, so a pylon was as
deep as the gap from the record to the model: about 2 ft on the F/A-18D's
wing, 3.3 ft on the F-16C's and MiG-29's, 4 to 5 ft on the AV-8's and
Su-25's, and 7 ft under the F-14's body. Real pylons are 15 to 30 inches;
John asked for 2 ft everywhere, then the same day for short rails about
4 inches deep at fuselage and centreline stations and wingtip missiles on
the tip itself (2026-10-13). Every store now hangs that far under the
surface over it and leaves from there, so the release origins moved with
it: up by a few feet on most aircraft, by up to 4 ft under the F-14, and
wingtip missiles in to the tip's edge (1 ft on the F/A-18D and F-16C,
1.7 ft on the Rafale).

Measured with `tore-app --stores-preview` (the depth from each store's lugs
straight up to the drawn model, feet; default loads):

| Aircraft | Stations | Depth |
| --- | --- | --- |
| F/A-18D, F-16C, MiG-29, Su-25, A-10, F-15, F-4E, F-4J, Rafale | Wing stations | 2.0 (to 2.05 where the wing slopes under the store) |
| F-14 | Glove AIM-120s, wing AIM-9s | 2.6, 2.2 |
| AV-8 | Wing Mavericks and AIM-9s | 2.1 |
| F-15, F-4E, F-4J | Fuselage Sparrows and AMRAAMs | 0.33 to 0.37 |
| A-4E | Mk 82 centreline rack (rail and beam) | 0.8 to 0.9 (0.63 at the rail, more where the belly curves up at the rack's ends) |
| F/A-18D | Fuselage AIM-120s | 0.8 (the body's flank is lower beside them) |
| Rafale | Fuselage Mavericks | 1.1 to 1.3 |
| F-14 | Belly Phoenixes | 1.5 rear, 3.4 front: they hang clear of the engine ducts beside them, under the tunnel |
| Su-25 | B-13 pods beside the nacelles | 3.2 (clear of the nacelle, under the wing) |
| AV-8 | Fuselage Mavericks | 3.7 (clear of the belly, under the wing root) |
| F/A-18D, F-16C, Rafale | Wingtip missiles | on the tip's edge (0.0 to 0.1) |

Where a store hangs beside a deeper part of the model (a nacelle, a body's
flank), its pylon reaches up past it to the wing or body over the store,
which is the clearance rule in the [rules](#rules): the store sits 2 ft
(wing) or 4 inches (body) under the lowest surface across its body. The record's own
lateral and fore and aft position is kept everywhere except the three
moves listed there.

## The A-4E

Retail's default A-4E load is four Mk 82s on a one-sided wing row 2.7 ft
out, four 150-gallon tanks on the next wing row (5.7 ft out, paired) and two
LAU-61 pods on the outer wing row. The real A-4E has five stations: a
centreline one and two a wing, and carries its tanks on the centreline and
the inner wing pylons. With BP2's two pylons a wing, the four tanks (two a
wing) and the Mk 82s (paired, two a wing) crowded the same stretch of wing.

PY2 (agent decision, 2026-10-13) keeps retail's quantities (four tanks are
four tanks of fuel and weight) and moves the Mk 82 row, which retail
releases from one side next to the fuselage, to the centreline: a rack of
two pairs one behind another on a short rail under the belly, the front pair first, right
then left. The tanks keep two pylons a wing on the inner wing (4.3 and
7.1 ft out), and the LAU-61s the outer row. A real A-4E would carry three
tanks at most (centreline and inner wings); matching that would change the
default fuel, which is retail's, so it is left as an open question for
John.

## Performance

The stores join their aircraft's existing vertex batch (no extra draw call or
pipeline). A pylon is 48 vertices, a fitted pod or tank 144, a weapon shape its
own triangles; the model surfaces pylons meet are looked up once per point and
aircraft type. On a
16-aircraft formation each carrying six stations this is about 10,000 extra
vertices a frame.

## Source notes

- Hardpoint record layout and location names: [aircraft ordnance](../formats/aircraft-ordnance.md).
- Retail release position and the 0x4 mirror rule: [weapons formats](../formats/weapons.md#release-state)
  (FA.EXE 1.02F `HARDPos` `0x4532a0`, `HARDUnrotatedHardPos` `0x453220`).
- Code: `tore_sim::combat::hardpoints` (the rule, two pylons a wing, tandem fuselage pairs,
  `Rack::store_point` and `placed`, the 2 ft pylon), `tore_sim::combat::store_sizes` (the sizes
  stores hang by), `Station::release_point`, `combat::live::cockpit`
  (`release_round`), `tore_world::ai_wings` (AI missiles),
  `tore_world::snapshot::store_poses`, `tore-app/src/pylons.rs` (drawing), `tore-app/src/store_sizes.rs` (real sizes), `tore_world::snapshot::pod_body`.
- Scenarios: `a2g-release-origin`, `net-a2g-stores` (a client sees another
  player's and an AI's stores empty) and `replay-a2g` (the stores recorded
  and replayed) in the battery.
