# Exact checkpoints: equivalence and cost (stage H, slice H9)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Measured on 2026-10-05 on the development machine (Ryzen 9 7900X, 24
threads, Linux), branch `mp/h9-integrate` on top of every stage H slice. It
records what the [exact checkpoints](../formats/checkpoint.md) cost on a real
15 against 15 mission, against the
[plan's budget](../multiplayer-plan.md#bandwidth-budget), and what the
equivalence tests cover. Windows and macOS were not measured; the equivalence
tests run on every CI platform.

## In short

- **Exact.** Every scenario restores into a fresh world and flies on bit for
  bit. On real data, a 30-aircraft mission restored at nine moments of its
  first five minutes flies 1,200 more ticks into exactly the state the
  original reaches, every time.
- **Size.** 60 KB at the start, 770 KB to 1.0 MB through the first-minute
  furball, about 500 KB after two and five minutes. The plan's budget was 150
  to 600 KB, so the furball is up to 70 percent over its top.
- **Time.** Writing a checkpoint takes 1 to 8 ms and restoring one 2 to 9 ms,
  plus about 40 to 80 ms to build the fresh world. Catching up 10 seconds of
  ticks costs what the ticks cost: 0.3 to 0.9 seconds late in the mission and
  1.3 to 6 seconds in the first 30 seconds on this loaded machine, against the plan's 1
  to 3 seconds.
- **No delta coding now** (agent decision, below): a delta against the
  previous checkpoint would save 2 to 3 percent at the furball peak, which is
  what sets the bandwidth.

## What was run

| What | Command |
| --- | --- |
| The whole-world equivalence, eleven scenarios, in the normal suite | `cargo test --locked -p tore-world --lib a_whole_world_restores -- --nocapture` |
| The twin restore of each section, and each scenario's asserted state | `cargo test --locked -p tore-world --lib checkpoint` |
| The AI golden missions restored mid-flight | `cargo test --locked -p tore-sim --lib golden_tests::ai` |
| Size, time and catch-up on real data, four times in a row | `TORE_DATA_DIR=$PWD/.local/mpb-data-h9-integrate cargo test --release --locked -p tore-session --test checkpoint_cost -- --ignored --nocapture` |

The real-data runs shared the machine with other agents' builds: the load
average was 20 to 39 on 24 threads. The sizes are the same in every run (the
simulation is deterministic); the times are the minimum, median and maximum
of the four runs and are upper bounds for a quiet machine.

## Equivalence

`world::checkpoint_tests::a_whole_world_restores_into_a_fresh_one_and_flies_on_identically`
steps each scenario to its tick N, writes a checkpoint, restores it into a
world fresh from the same build, and steps both copies M more ticks with the
same inputs. Every tick's output must match, and every 30 ticks and at the end
the two worlds must code to the same bytes. It runs in the normal suite: about
6 to 8 seconds for all eleven scenarios in a debug build here. The eleventh
scenario, a human lead's order, was added by this slice so the data link holds
assignments at the checkpoint (one acknowledged, two not yet).

Sizes of the synthetic scenarios at tick N, in bytes:

| Scenario | Whole | Shared records | Combat | AI wings | Cockpits | Data link |
| --- | --- | --- | --- | --- | --- | --- |
| Single player | 28,998 | 577 | 23,520 | 3,865 | 656 | 210 |
| Crowd fight (8 aircraft) | 87,086 | 723 | 61,219 | 22,604 | 1,498 | 845 |
| Open mission with handoffs | 51,499 | 797 | 40,385 | 8,205 | 843 | 655 |
| Damaged aircraft | 83,168 | 723 | 59,612 | 20,290 | 1,514 | 782 |
| Crowd fight with handoffs | 86,860 | 723 | 61,464 | 22,067 | 1,511 | 859 |
| Missile duel | 14,883 | 797 | 12,403 | 91 | 806 | 224 |
| Radio calls pending | 29,144 | 577 | 23,520 | 3,865 | 656 | 210 |
| AI landing | 26,038 | 1,069 | 20,342 | 3,501 | 368 | 177 |
| Ground start | 33,665 | 1,069 | 23,104 | 7,820 | 633 | 437 |
| Changing weather | 26,031 | 577 | 20,111 | 3,699 | 705 | 207 |
| A human lead's order | 88,647 | 723 | 56,937 | 29,214 | 680 | 898 |

The other sections (roster, weather, comms, wing status, radio, score) are
under 650 bytes each.

**The AI golden missions.** `golden_tests::ai::restoring_the_ai_missions_mid_flight_changes_no_behaviour`
replaces the engagement and airfield missions of the AI fingerprint tests with
copies restored from their own checkpoints 792 times (every 97 ticks and on
every scripted tick) and requires the same fingerprint as the run that never
restores. It found one gap: an AI actor's last controls were skipped as
scratch, but a destroyed actor stops flying and keeps them. They are now
coded. The recorded fingerprints did not change.

**A field added without coding fails to compile.** Shown by hand on
2026-10-05: a field added to `DataLink` failed to compile in its coder only
(`datalink_checkpoint.rs`: "pattern requires `..`" and "missing field"), then
was removed. Slice H0 showed the same on `Radio`.

## Real data: the 15 against 15 mission

`crates/tore-session/tests/checkpoint_cost.rs`: five F/A-18Ds in each of three
wings against five MiG-29s in each of three, 10 nm apart at 10,000 ft over the
Ukraine theater, an open mission with nobody connected, so all 30 aircraft are
AI. A checkpoint is taken every 10 seconds of the first minute and after two
and five minutes. At each of those, the restored copy and the original both
fly 1,200 ticks, and the two must then code to the same bytes; they did, at
every mark of every run.

### Size

| Mark | Aircraft alive | Whole | Combat | AI wings | Data link | Unchanged since the mark before |
| --- | --- | --- | --- | --- | --- | --- |
| 0 s | 30 | 59,577 | 35,089 | 20,777 | 9 | |
| 10 s | 30 | 769,927 | 208,531 | 550,499 | 7,187 | 17,318 |
| 20 s | 29 | 1,006,019 | 386,112 | 609,282 | 6,907 | 17,309 |
| 30 s | 21 | 1,006,557 | 523,444 | 473,990 | 5,403 | 27,984 |
| 40 s | 14 | 933,354 | 537,855 | 389,662 | 2,118 | 113,936 |
| 50 s | 11 | 841,317 | 528,031 | 308,294 | 1,272 | 176,314 |
| 60 s | 11 | 786,406 | 492,505 | 288,939 | 1,243 | 206,948 |
| 120 s | 11 | 506,298 | 219,885 | 281,499 | 1,195 | 216,927 |
| 300 s | 11 | 499,704 | 225,398 | 269,387 | 1,198 | 225,525 |

Bytes. The shared records are 2,897 bytes (12 records) at every mark; the
other sections total under 1 KB. "Unchanged" is the bytes of the parts below
(each combat list, the aircraft's and the scenery's rows, and each AI actor's
sensors, controller, memory and the rest of it) whose coding is the same as
at the mark before.

Where the bytes go, each part coded alone:

| Mark | Flares and chaff | Smoke | Rewind history and the rest of combat | Rows (aircraft, scenery) | AI sensors (alive, destroyed) | AI memories | AI controllers | Rest of the AI actors |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 s | 0 | 0 | 8,623 | 9,139, 17,307 | 8,565, 0 | 270 | 1,634 | 10,308 |
| 10 s | 22 | 24 | 180,352 | 10,791, 17,307 | 341,163, 0 | 152,841 | 9,018 | 47,477 |
| 20 s | 128,171 | 25,658 | 195,350 | 10,616, 17,307 | 330,471, 10,513 | 149,772 | 8,094 | 110,432 |
| 30 s | 280,731 | 38,939 | 168,671 | 11,007, 17,307 | 210,454, 94,817 | 78,103 | 6,764 | 83,852 |
| 60 s | 310,497 | 33,368 | 117,039 | 11,453, 17,307 | 47,088, 173,419 | 15,338 | 7,440 | 45,654 |
| 300 s | 24 | 118,518 | 76,366 | 11,062, 17,307 | 28,440, 173,419 | 15,705 | 7,442 | 44,381 |

What that shows:

- **The AI's sensor pictures grow with the square of the aircraft count.**
  Each AI actor's sensors hold contacts, plots and trails for every other
  aircraft: about 11 KB per actor with 30 aircraft alive (slice H1 measured
  about 2 KB with 8). Its memory adds about 5 KB in the first 20 seconds.
- **A destroyed actor keeps its sensor picture.** 19 destroyed AI actors
  carry 173 KB of sensors from the minute mark on, a third of the late
  checkpoints, and it never changes.
- **Flares and chaff peak at 314 KB** in the furball (each flare carries its
  smoke puffs), then fall to nothing once they burn out.
- **The rewind history** (the newest 61 frames of every aircraft's hit
  volumes, most of the combat rest) is about 6 KB per aircraft alive, as slice
  H3a estimated.
- **Smoke** from wrecks grows to 118 KB by five minutes.
- The checkpoint is 2 to 34 KB per aircraft, against the plan's estimate of 5
  to 20.

### Time

Milliseconds; minimum, median and maximum of the four runs.

| Mark | Write | Restore | Build the fresh world | Catch-up: restore and 1,200 ticks | The original's 1,200 ticks |
| --- | --- | --- | --- | --- | --- |
| 0 s | 0.9, 1.1, 1.6 | 1.6, 2.0, 2.3 | 39, 53, 56 | 1,489, 2,301, 2,526 | 1,836, 6,564, 6,767 |
| 10 s | 4.9, 6.5, 7.5 | 4.9, 7.0, 7.1 | 40, 55, 62 | 2,789, 3,262, 4,092 | 2,353, 3,513, 4,781 |
| 20 s | 6.0, 7.4, 7.6 | 6.7, 7.6, 8.8 | 41, 47, 63 | 1,748, 2,313, 6,039 | 2,108, 5,450, 5,878 |
| 30 s | 5.3, 7.2, 8.1 | 5.9, 8.7, 8.7 | 41, 67, 77 | 1,300, 2,066, 4,323 | 1,225, 1,953, 2,984 |
| 40 s | 4.9, 6.2, 6.6 | 5.5, 7.7, 7.9 | 41, 57, 62 | 713, 1,617, 1,670 | 696, 1,768, 2,139 |
| 50 s | 4.6, 6.1, 7.1 | 5.2, 7.4, 7.5 | 40, 53, 61 | 537, 1,407, 1,579 | 562, 888, 1,460 |
| 60 s | 4.6, 5.5, 6.4 | 4.9, 6.8, 7.3 | 39, 58, 61 | 478, 956, 1,321 | 479, 687, 1,011 |
| 120 s | 3.0, 4.0, 4.5 | 3.5, 4.1, 5.4 | 39, 45, 62 | 398, 615, 868 | 395, 665, 797 |
| 300 s | 3.0, 3.1, 3.9 | 3.5, 3.7, 4.8 | 39, 40, 54 | 318, 382, 555 | 317, 344, 614 |

The catch-up and the original's own ticks cost the same within the noise of
the load: a restore adds a few milliseconds and nothing else, so catching up
costs exactly what the ticks it replays cost. The first 30 seconds of this
mission are its most expensive ticks (all 30 aircraft acquiring each other,
then the missiles): 1.0 to 5.6 ms a tick here, against the plan's estimate of
1 to 2 ms, which puts the catch-up above the plan's 1 to 3 seconds in the
furball on a loaded machine. Earlier runs at a quieter moment of the same day
measured 1.4 seconds of catch-up at the start, 0.48 at one minute and 0.30 at
five minutes.

## Delta coding: not now (agent decision)

The plan asked for delta coding against the previous checkpoint "if the size
needs it". The size is over the budget in the furball (1.0 MB, or 100 KB/s
per standby at one checkpoint every 10 seconds, against 600 KB and 60 KB/s),
so it is needed in that sense, but **a delta against the previous checkpoint
is the wrong tool**, so it is not built:

- **At the peak almost nothing is unchanged.** Ten seconds apart in the first
  30 seconds, only 17 to 28 KB of the parts code the same (2 to 3 percent):
  the scenery rows and a few actors. The big parts are rewritten within 10
  seconds: the rewind history keeps half a second, flares burn out, live
  sensors and memories follow moving aircraft. Plain bytes agree even less:
  1.4 to 7.9 percent of 64-byte blocks reappear. The furball sets the
  bandwidth a standby needs, and a delta would not bring it under the budget.
- **What a delta would save later is one frozen part.** From the minute mark
  on, 207 to 226 KB (40 to 45 percent) is unchanged, and 173 KB of that is
  the sensor pictures of destroyed AI actors.
- **It costs more than it saves.** Both ends would keep the previous
  checkpoint decoded, every hand-written coder would thread a baseline
  through, and a lost or late checkpoint would break the chain until the next
  full one.

What would bring the peak down, cheapest first, for stage K to choose from
when it builds the standby stream (none is built here):

1. **Pace the stream.** A checkpoint sent over its 10-second interval in the
   background is 100 KB/s (0.8 Mbit/s) per standby at the worst moment, about
   a quarter of the host upload measured for 30 players (3.3 Mbit/s,
   [stage D](net-2026-09-30.md)). With one standby rather than two, or a
   longer interval outside the furball, it may simply fit.
2. **Leave out a destroyed actor's sensor picture**, if nothing reads it after
   the actor is destroyed: 173 KB of every late
   checkpoint. That needs a proof at the skip, as the scratch rule asks.
3. **Code the flare puffs more tightly**: 314 KB at the peak.
4. **Code the AI's sensor trails and memories against each other**, since
   they grow with the square of the aircraft count.

## Not measured

- Windows and macOS timings (the equivalence tests run there in CI).
- A mission with humans connected: their ownships replace AI actors with
  similar sensor state, so the sizes should be close; not run.
- The standby stream's inputs (the plan's estimate of 5 KB/s stands).
- Compression: no general compressor is in the dependencies.
