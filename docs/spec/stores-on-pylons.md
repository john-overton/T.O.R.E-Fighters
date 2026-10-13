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

## What the player sees

- **Every store leaves from its own pylon.** A missile, bomb or rocket appears
  at the station that carried it, in the aircraft's frame: wing stations out
  on the wing, fuselage stations under the body, centreline stores on the
  centreline, wingtip missiles at the tip, internal bay stores below the bay.
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
| Station position | The source hardpoint record's x / y / z, a third of a foot a unit, right / up / forward of the aircraft's reference point ([table](../formats/aircraft-ordnance.md#hardpoint-positions)) | spec-derived |
| Paired record | Hardpoint flag 0x4: the record's position is the right pylon, its mirror the left | spec-derived ([release state](../formats/weapons.md#release-state)) |
| Also paired | Every Wing and Wingtip record without flag 0x4 (the A-4E's inner Mk 82 row, the Mi-24's AT-2 rails) | agent decision, 2026-10-12 |
| Never paired | An internal gun, whatever its flags; a record on the centreline (nothing to mirror) | agent decision |
| Which side | The mirror (left) when the whole releases left on the station after this one (rounds over `actualRoundsPerGame`) is odd; so the left side empties first and the right holds the odd store | retail parity rule; whether retail counts before or after the debit is unknown, the order is fitted |
| Unlimited ammunition cheat | The count never moves, so the side follows the release's projectile number (alternates) | agent decision |
| Who | The player, the AI (strike stores, missiles and seeker missiles alike) and every network seat. Retail releases a distant AI aircraft from its centre; T.O.R.E releases every aircraft from its pylon | agent decision |
| Guns | Keep their muzzle (gun mount, AC-130 barrel) | unchanged |
| Bomb packs | Each bomb of a pack keeps its pack stagger, added to the pylon's side | unchanged |

The bomb sight (CCIP and the level T) still predicts from the record's
right-hand position. A left-side release lands up to the station's span
(about 8 to 14 ft on a fighter wing) to the left of the pipper; this is inside
the bomb scatter and is left for a later pass.

## Drawing (opinionated, John 2026-10-12)

Retail draws no stores on aircraft (the A2G pass recorded this as decision 20,
"stores on pylons stay undrawn"); John reversed that decision on 2026-10-12.
Every number below is fitted (agent decision).

| Item | Rule |
| --- | --- |
| Stations drawn | External ones (Centerline, Fuselage, Wing, Wingtip) that held something at mission start; not guns or internal bays |
| Store look | The weapon's own shape (the same mesh it flies with after release). Rocket pods: a grey-green eight-sided pod 8.5 ft by 1.4 ft. Fuel tanks: a light grey pod 13 ft by 2 ft. A store with no shape: a plain body 8 ft by 0.9 ft |
| Stores on one side | One at the pylon point. Two side by side, 0.55 diameters either side. Three as a triple ejector rack: two shoulders and one 0.95 diameters below. More: such racks one behind another, 1.05 store lengths apart. The layout keeps the loaded count's slots; the highest slot empties first |
| Pylon | A grey slab 0.35 ft thick and 0.55 store lengths long (1.5 to 8 ft), from the top of the store up to the model's underside straight above the hardpoint, or up to 3 ft inboard of it (found from the clean model, up to 12 ft). With nothing above (a wingtip rail beside the wing) a 0.35 ft rail. A rack of several stores is as wide as the rack, at most 1.4 ft |
| Colours | Pylons and racks grey (0.5, 0.52, 0.54); weapon shapes keep their palette colours |

Where the stores come from:

- **Single player, the host, AI aircraft:** the render snapshot carries each
  aircraft's stations and their stores left ([`AircraftPose::stores`]).
- **Network clients:** a client draws its own aircraft's stores from its
  loadout (the session's copy of the mission) and the rounds and start-load
  bits of its cockpit readout, which the wire already carries. The wire
  carries no per-station counts for any other aircraft, and this slice does
  not change the protocol (protocol 26 belongs to slice BF), so a client
  draws no stores on other aircraft. It still sees every store leave from the
  right pylon, because projectiles come from the host. Drawing them needs the
  per-station counts of every aircraft on the wire: open.
- **Mission replays:** recordings carry no loadout, so a replay draws no
  stores. Recording the stores needs a replay format change: open.

## Performance

The stores join their aircraft's existing vertex batch (no extra draw call or
pipeline). A pylon is 48 vertices, a fitted pod or tank 144, a weapon shape its
own triangles; the pylon heights are worked out once per aircraft type. On a
16-aircraft formation each carrying six stations this is about 10,000 extra
vertices a frame.

## Source notes

- Hardpoint record layout and location names: [aircraft ordnance](../formats/aircraft-ordnance.md).
- Retail release position and the 0x4 mirror rule: [weapons formats](../formats/weapons.md#release-state)
  (FA.EXE 1.02F `HARDPos` `0x4532a0`, `HARDUnrotatedHardPos` `0x453220`).
- Code: `tore_sim::combat::hardpoints` (the rule), `combat::live::cockpit`
  (`release_round`), `tore_world::ai_wings` (AI missiles),
  `tore_world::snapshot::store_poses`, `tore-app/src/pylons.rs` (drawing).
- Scenario: `stores-release-origin` in the battery.
