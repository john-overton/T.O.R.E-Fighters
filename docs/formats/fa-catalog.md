# Fighters Anthology aircraft and weapon catalogs

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature, see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


Research mode, 2026-09-18. Two machine readable inventories of the shipped
catalog: [`fa-aircraft.csv`](fa-aircraft.csv), 145 aircraft types, and
[`fa-weapons.csv`](fa-weapons.csv), 135 store types. Every row is decoded from
one source record. Field order comes from the recovered schema in
`crates/tore-formats/src/aircraft_schema.rs`, which stays the one home for it.

The weapon file covers projectile records (JT) only. Sensors (SEE),
countermeasures (ECM) and fuel tanks (GAS) are equipment rather than
projectiles; they appear in the aircraft file's default stores column, and their
recovered fields are described in [aircraft weapons](weapons.md).

Both files come from the supplied installation's `FA_2.LIB`, SHA-256
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`. No other
EALIB archive in that installation, on either disc, contains a PT or JT
resource. The disc 1 `SETUP.ESA` installer container stays unreadable by the
extractor and was not inspected.

## What these files are not

- **Not a flyable roster.** Catalog presence says nothing about which aircraft a
  player can fly. The creator's player filter and its era gates are unresolved;
  see [quick mission](quick-mission.md) and [ordnance menu](ordnance-menu.md).
- **Not a loadout compatibility table.** `aircraft_default_stations` counts the
  stations that name a store as their default, nothing more. Station
  compatibility masks and weight classes remain unresolved, so a store with a
  zero count may still be loadable, and 28 records are named by no aircraft or
  surface type default at all.
- **Not real world specifications.** The numbers are the game's numbers.
  Preserve them even where they differ from the real aircraft or weapon.
- **Not an implementation status.** Delivered behaviour is in
  [the feature matrix](../features.md), not here.

## Regenerating

```sh
python3 tools/catalog_fa.py
```

The tool extracts PT, JT, NT and OT resources through `tools/extract_assets.py`
into `.local/catalog`, decodes them and rewrites both CSVs. `--source` selects
other media, `--extracted` reuses an existing extraction, and `--out` writes the
pair elsewhere. Unknown layouts fail rather than producing a partial row.

## Aircraft columns

Masses are pounds, thrust is pounds force, altitudes are feet, speeds are knots
converted from the source feet per second.

| Column | Contract |
| --- | --- |
| `resource` | Source PT resource name |
| `short_name`, `display_name` | The record's own two display strings, source spelling and spacing retained |
| `year` | Source year field, used by the creator's era gates |
| `object_class` | Raw class word. Bit meanings are unresolved; 79 records carry `0x8000`, 65 carry `0x4000` and the base gun carries `0x0800` |
| `no_lift` | Plane flag 8, read as "no lift" by the flight model. It marks 19 records: every rotary wing type, the tiltrotor and the blimp |
| `engines` | Engine count |
| `empty_weight_lb`, `internal_fuel_lb`, `max_takeoff_weight_lb` | Source masses |
| `military_thrust_lbf`, `afterburner_thrust_lbf` | Total thrust, not per engine. Zero means the record has no afterburner |
| `top_speed_kt` | Fastest point of the 1 G envelope, at whatever altitude that point sits |
| `sea_level_min_kt`, `sea_level_max_kt` | Slowest and fastest 1 G speeds where the envelope crosses zero feet |
| `ceiling_ft` | Source maximum altitude |
| `g_limit_min`, `g_limit_max` | Lowest and highest G rows the record carries |
| `hit_points` | Source hit points |
| `hardpoints` | Station count, including equipment and gun stations |
| `internal_gun` | Default store on a station whose store is classified `gun round` below |
| `default_stores` | Each default store as `RESOURCE:stations`, in station order. Includes sensors (`.SEE`), countermeasures (`.ECM`) and tanks (`.GAS`) |
| `hud_resource` | HUD resource the record names, empty when it names none. Many types share one cockpit |
| `shape` | Main exterior shape resource |
| `archive` | Source archive |

## Weapon columns

Ranges are nautical miles at 6,076 feet per mile, times are seconds at the live
host's quarter second timer unit, speeds are feet per second.

| Column | Contract |
| --- | --- |
| `resource` | Source JT resource name |
| `short_name`, `display_name` | The record's own two display strings |
| `derived_kind` | Mechanical grouping, defined below. A reading aid, not a recovered FA category |
| `year` | Source year field |
| `weight_lb` | Source weight. Gun records weigh 1, the representative round rather than the gun |
| `ordnance_bank` | Load Ordnance category bank: `one` when projectile flag `0x10000` is set, else `two`. The two dial labels are not recovered. In this catalog every gun and air to air missile falls in bank one, and every bomb, rocket and air to ground missile falls in bank two |
| `seeker_signature` | Source `sig` selector. Nonzero enables target guidance; the proposed group meanings and their exceptions are in [missile record interpretation](missiles.md) |
| `launch_min_nmi`, `launch_max_nmi` | Launch permission zone range limits |
| `seeker_max_nmi` | Acquisition zone maximum range |
| `motor_burn_s`, `lifetime_s` | Motor cutoff age and cleanup age, launch relative. Some records clean up before motor cutoff; those are source values, not errors |
| `initial_speed_fts` | Speed the record leaves the rail with, before the launcher scalar is applied |
| `max_speed_fts` | Source speed clamp |
| `projectiles_in_pod` | Pod capacity. Above 1 marks rocket and gun pods |
| `rounds_per_shot` | Source `actualRoundsPerGame`. Its cadence meaning is unresolved; see [weapons](weapons.md) |
| `damage_by_class` | The five source damage values, separated by `;`. The class mapping is unresolved |
| `fuze_radius_ft` | Source fuze radius |
| `aircraft_default_stations` | Aircraft stations across all 145 PTs naming this store as their default. 70 records have a nonzero count, matching the earlier audit in [importer coverage](coverage.md) |
| `surface_type_references` | Ground and ship type records (NT, OT) naming this store |
| `shape`, `fire_sound` | Resources the record names |
| `archive` | Source archive |

### Derived kind rule

Applied in order, from recovered fields only:

1. Motor time above zero and a seeker signature: `guided missile`, 62 records.
2. Motor time above zero and no signature: `rocket`, 6 records.
3. No motor time and a nonzero initial speed: `gun round`, 43 records.
4. No motor time, no initial speed, with a signature: `guided bomb`, 5 records.
5. Otherwise `bomb`, 19 records.

The rule is mechanical, so records the research explicitly holds open land
wherever their fields put them. AT2 has no seeker signature and appears as a
rocket; the open question about its guidance is in
[missile record interpretation](missiles.md). Radar directed guns such as
PHALANX and ZSU23 carry a signature but no motor, so they appear as gun rounds.

## Related documents

Per record guidance and lifetime for the guided candidates, with the
exceptions that must survive classification, are in
[missile record interpretation](missiles.md) and
[the missile specification](../spec/missiles.md). Aircraft record recovery is in
[aircraft recovery](aircraft.md), store and sensor recovery in
[aircraft weapons](weapons.md), and format by format status in
[importer coverage](coverage.md).
