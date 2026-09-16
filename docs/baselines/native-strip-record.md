# Isolated STRIP placement checkpoint

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15, following `0c9d806`. **NE-01.1b complete as a bounded input-reader
slice**, not full mission/world or native contact acceptance.
[Contract](../formats/native-strip.md#bounded-isolated-placement--ne-011b),
[living plan](../research/native-environment-systems-plan.md).

```sh
cargo test --locked -p tore-formats strip::
cargo run --locked -p tore-formats --example native_strip -- RUNWAY.SH STRIP.OT ISOLATED-PLACEMENT
```

The original first UKR.MM object record was isolated byte-for-byte into ignored
`.local/native-environment/strip-record-checks/first-strip.txt`, from the existing
[reviewed resource](native-land-foundation.md). It decodes to fixed8 position
`[306184192, 0, 251658240]`, zero PA angles/speed, raw nationality 137, source
flags 0x4003, alias word 55436 and the original name bytes. Nationality still
needs the map-dependent conversion (138 for UKR); Y=0 still needs initial native
ground sampling. No object is allocated or registered by this diagnostic.
Definition/shape inspection retains 23 boxes, all twelve required IDs and the
incomplete 63-face projection. This is no new drawing/resource-closure claim.

Existing hash-gated source slices `00482443-mission_object_begin.txt` and
`00482df7-mission_object_post_create.txt` establish the selected record's optional
post-create exclusions. Controller high bit clear skips 0x4918d0; reset sentinel
words skip 0x45e490/0x45f1c0; kind 0 skips fuel/loadout branches. Alias precedes
final storage. No new extractor range or broader callback acceptance is claimed.

Four synthetic tests cover native wrapping/low-width conversions, decimal/hex
bounds, LF/CRLF, exact source retention, byte names of lengths 0/39/40/41/100,
non-UTF-8 preservation and malformed/unsupported/duplicate/missing/trailing
records. Whole-record limits and unsupported control/numeric syntax are explicit
host restrictions. Fixtures contain no retail bytes.

Linux validation:

- Formatting, warnings-denied workspace/all-target Clippy, **360 Rust tests**,
  locked workspace build, **24 Python tests**, repo/app/extractor asset guards
  and whitespace checks pass.
- Existing native live replay: **28 cases / 33600 updates**, both reviewed PTs,
  using `strip-service-source/tables` and the existing foundation replay command.
- Creator startup smoke passes on NVIDIA RTX 4070 / Vulkan / Immediate. No
  rendering changed; no new runway visual, handling, performance, viewer or
  cockpit acceptance is claimed.

Rust/replay/GPU logs and the isolated original input remain in ignored
`.local/native-environment/strip-record-checks/`. Physical controls/audio were
not manually tested. Windows/macOS build/runtime and retail comparison remain
unavailable/not run. PM control remains locally ignored and uncommitted.

Next: E016 template ownership/consumers, E019 service bodies/global producers
and E004 full drawing/LOD/palette closure, then staged E001/E002 query state
and late-failure rollback before either aircraft's live contact connection.
No AI, carrier activation, default change or push.
