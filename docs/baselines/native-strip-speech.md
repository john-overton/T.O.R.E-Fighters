# Current-object and speech timing checkpoint

> **T.O.R.E — we trace what the player does, not what the code did.**
> This project reverse-engineers *player interaction*: what you press, see, hear
> and feel in Fighters Anthology, and the numbers behind it. It does not
> reproduce the original program byte by byte. Anything here about the original
> executable is evidence toward a behaviour spec — never a specification for what
> we build. If a sentence below reads like an instruction to reproduce the
> original's internals, it is out of date.
> <!-- tore-header v1 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15, following `2a7708b`. **NE-00.1j completes bounded source recovery
and one diagnostic arithmetic helper**, not full E019/E020.
[Contract](../formats/native-strip.md#current-object-switches-and-speech-timing--ne-001j),
[frozen plan](../research/native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-speech-source
cargo test --locked -p tore-sim speech_delay
```

The same reviewed EXE/SMS identities produce **159 reviewed regions**, 107 selected
symbol spans and 3829 symbols. Five additional aligned regions cover push/pop,
speech delay, submission and clock initialization. Repeat extraction passes.
No source bytes are committed or executed.

The delay test checks negative/zero scales, low-word truncation, five-bit shift
counts at 15/16/31/32/33/255/256 and signed maximum, zero input and wrapping
word deadline addition. In particular, a native AX shift by 16 is zero; Rust's
word `wrapping_shl(16)` would be incorrect. The helper uses widened arithmetic
and explicit narrowing. It changes no event, clock or runtime state.

Current-object switches store outgoing scratch and can leave scratch populated
when restoring ID zero. Speech submission carries two terminated strings and
sets a global deadline; the comment caller sets a separate airport retry timer
when no payload is submitted. Complete payload lifetime/event dispatch, callback
content/RNG and ongoing clock/scale producers remain open. No speech playback,
autonomous behavior, live contact or carrier support is activated.

Validation: Linux fmt, workspace/all-target Clippy with warnings denied,
**361 Rust tests**, **24 Python tests**, locked workspace build, repeat extraction,
repo/app/extractor asset guards, whitespace and changed-document file-link checks
pass. Both-aircraft replay passes 28 cases / 33,600 updates. Creator smoke presents
on RTX 4070 / Vulkan / Immediate. Logs remain in ignored
`.local/native-environment/strip-speech-{tests,replay,gpu}.log`.
These are diagnostic/regression checks, not full callback execution or retail
parity. No new runway visual, handling or performance acceptance; physical
input/audio and Windows/macOS runtime are not tested, retail comparison unavailable.

Next: E019 selected static-object movement/event-service paths, E020 remaining
speech/clock/event producers and E016/E004 closure before staged E001/E002. No push.
