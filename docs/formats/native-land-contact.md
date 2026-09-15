# Native land-contact foundation

2026-09-15, NE-00.1a under the [living plan](../native-environment-systems-plan.md).
**Native static source**, with diagnostic translations only where identified.
Uses the exact reviewed FA EXE/SMS hashes in [native flight](native-flight.md).
No imported code executes. [Validation](../baselines/native-land-foundation.md).

## Query order and transaction boundary

`GetGround` at `0x47af20..0x47af70` first calls touching predicate `0x411910`,
which calls `0x4abab0` with request 1 and no angle/water output. It stores the
touching result, then calls `0x4abab0` again with request 1 and angle/water
outputs, initializing the requested heading from cp+0x1d. It converts the
returned pitch PA word through `0x4c6638` after storing the returned height.
Touching compares aircraft fixed8 Y against returned height plus 256, inclusive.
Thus the diagnostic `ContactQueries::ground` represents **two native queries**,
not one sample. Its future implementation must preserve their cache ordering.

The collision dispatcher `0x42b800..0x42bd2e` is not read-only. Its cache path
requires mode word `0x520a50 == 16`, an object, both segment endpoints matching
the object's X/Z, and a negative endpoint Y. A signed deadline comparison
`object+0x27 > now` permits reuse. Expiry sets a refresh marker. Cache reuse
clears masks 0x1 and 0x2 after recovering terrain position, angles and class.
Do not infer that mode 16 denotes a carrier.

On marked refresh the dispatcher writes **terrain-channel** height plus the
resolved type offset back to object+0x2f, two angle words to +0x2b/+0x2d and class
byte to +0x33. Those writes use the terrain channel even if the returned result
selected the other collision channel. The type offset comes from signed word
+8 of `0x42e0c0`'s resolved record, shifted by eight; its complete type mapping
is still unknown. It must not be replaced with the adapter's eight-foot clearance.

Deadline precedence at `0x42bc5c..0x42bd24`:

| Predicate, in order | Stored deadline / RNG |
| --- | --- |
| byte +0x10 bit 0x80 | now + 1; no draw |
| instance flags +1 bit 4 clear | absolute 0x7fff00; no draw |
| signed speed +0x34 <= 0 and instance kind byte +0 != 4 | now + (8 + bound-4 draw) * 256; exactly one draw |
| otherwise | now + 256, plus 256 at Y >= 10,000 feet, plus 512 at Y >= 20,000 feet; no draw |

`queries::CacheLifetime` translates only that deadline decision with an explicit
optional draw and wrapping dword addition. It rejects missing, extra or out-of-range
draw samples. It neither writes a world cache nor assigns a native global stream.
The existing diagnostic's state/RNG rollback does **not** cover external query
mutation. The world-cache candidate and any query RNG must be staged before
live connection; a late failure must discard them together with flight state.

## Collision channels and unresolved producers

Dispatcher order is terrain/cache, optional mask-4 branch, then mask-0x0a branch.
Terrain includes grid traversal `0x42bdc0`, cell construction `0x42bfc0`, plane
intersection `0x42c1a0` and the zero-height fallback `0x42dda0`. These are research
leads, **not translated geometry**. Terrain class comes from T2 byte +1; class 1
sets the water output. Fixed renderer triangles are not a substitute.

`0x42de60` keeps separate nearest terrain and object candidates using signed
approximate distance. Equal distance retains the earlier candidate in its channel.
Final dispatch selects terrain only when the object candidate is absent or
terrain distance + `0x1f400` is strictly less than object distance; otherwise
it selects the object channel. `0x1f400` is 500 fixed8 feet, not a runway width.
If both distance sentinels remain `0x7fffffff`, only output id is cleared: the
caller cannot safely invent a height from that path. Full class, normal,
no-hit initialization and mask-specific object producers remain to recover.

## Landing-object preference

`0x4747c0..0x4747f6` returns true iff instance flags bit 1 is set, signed word
+0x0e is nonzero, instance flag 0x2000 is clear, and instance kinds 2/4 have
nonzero byte +0xe3. Other instance kinds do not test +0xe3. Field names remain
neutral where initialization/meaning is unreviewed. OT structure type and mutable
instance kind are distinct domains.

`queries::landing_object_preferred` translates this predicate. The existing
reverse-inventory nearest-surface selector still requires separately resolved
type flag 0x8000 and candidate instances; this helper creates neither.

## Selected original runway lead

UKR.MM explicitly places STRIP.OT, including the first named Zaporizhzhya object
at textual position `(1196032, 0, 983040)`, angle `(0,0,0)`, flags 0x4003 and alias
-10100. These are source text fields; initialization/elevation and world conversion
are not accepted merely from this record. STRIP.OT has BRF structure type 1,
size 166, flags 0x208021 (including 0x8000), shape pointer RUNWAY.SH and symbol
`_STRIPProc`. The symbol resolves to VA `0x4be640`; its dispatch and downstream
contracts require review. No complete bounded OT schema or runway instance
producer is claimed. The SH import/re-entry byte-pattern inspection reports no
candidates; it does not prove the absence of visual/dynamic dependencies.

Next: recover the vertical land-query geometry and type offset, trace STRIP
initialization/placement and drawing dependencies, then integrate staged query
state with source-order traces and late-failure rollback. Carrier stays gated.
