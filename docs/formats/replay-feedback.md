# Replay feedback event layout

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Feedback extends the named event vocabulary, not the binary grammar.
**Replay format remains 4.** The existing formats 2, 3 and 4 all encode and
read these ordinary fields. Older builds can decode them as unknown named
events without refusing the recording. No new value tags or sections exist.
See [the replay format](../REPLAYS.md#file-format).

`player.feedback` is stored on the submission frame. Its fields are:

| Field | Existing value type | Meaning |
| --- | --- | --- |
| `flag_tick` | Int | Tick shown when the player opened the form |
| `mission_seconds` | Num | `flag_tick / 120` |
| `submit_tick` | Int | Host tick when accepted |
| `opened_at_ms` | Int | Client UTC Unix milliseconds when opened |
| `submitted_at_ms` | Int | Client UTC Unix milliseconds when submitted |
| `player` | Text | Host-supplied callsign |
| `seat` | Int | Host-supplied seat number |
| `plane` | Int | Host-supplied plane ID |
| `aircraft` | Text | Exact aircraft selection key, such as `F18.PT` |
| `text.0` through `text.4` | Text | Ordered UTF-8 pieces of the complete text |

Each text piece is at most 1,000 bytes and ends at a character boundary.
This preserves the older reader's 1,024-byte string bound while allowing
1,000 Unicode characters. Concatenate the numbered fields in order. The
event's ordinary text is the first piece, so an older viewer has a preview.
An event without numbered text fields uses its ordinary text. Presence of
the event kind protects a file from retention even if its fields are unknown.

The mission header supplies the theater and recording identity. The JSON-lines
feedback export names the source file and carries both ticks, the opening
mission time in seconds, both UTC Unix millisecond times, player, seat,
aircraft, theater and complete text.

The format history is unchanged: 2 introduced the current frame grammar,
3 added surface tracks, and 4 added cockpit display samples. This vocabulary
extension on 2026-10-22 needs no format 5.
