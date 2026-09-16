# Native land-contact foundation

> **Research notes — research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature — see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


2026-09-15, NE-00.1a/b/c under the [frozen plan](../research/native-environment-systems-plan.md).
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
+8 of `0x42e0c0`'s resolved record, shifted by eight; its shape-relative mapping is established below; type/instance loading
remains unconnected. It must not be replaced with the adapter's eight-foot clearance.

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
intersection `0x42c1a0` and the zero-height fallback `0x42dda0`. The vertical cell arithmetic is now translated below; full dispatcher
assembly remains unconnected. Terrain class comes from T2 byte +1; class 1
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
`_STRIPProc`. The symbol resolves to VA `0x4be640`; its selector and add callback are now sourced in
[STRIP initialization](native-strip.md); full lifecycle closure remains open. No complete bounded OT schema or runway instance
producer is claimed. The SH import/re-entry byte-pattern inspection reports no
candidates; it does not prove the absence of visual/dynamic dependencies.

## Vertical terrain arithmetic — NE-00.1b

Aligned static ranges: `0x42bdc0..0x42bfb9` traversal,
`0x42bfc0..0x42c1a0` cell, `0x42c1a0..0x42c413` plane,
`0x42dda0..0x42de5d` horizontal intersection, `0x4a8d30..0x4a8e4a`
normal, `0x4d65c4..0x4d663d` square root, and
`0x4c6040..0x4c60e8` T2 lookup. No `0x42bd00` mid-instruction range is used.

Traversal rejects either endpoint outside `[0,cols<<21)` / `[0,rows<<21)`.
It clips into cells, exits on the first terrain-channel candidate, or after at
most 20 cells. The vertical query visits one cell; the clipper's both-inside
branch (`0x42c420..0x42c545`) leaves endpoints unchanged. General multi-cell
segment clipping is **not translated** by this slice.

Fine lookup with step zero returns the three-byte cell at `(row*cols+col)*3`.
Out-of-grid corners return the static fallback at `0x50ce4c`: color 255, class 1,
elevation zero. They do not clamp to the last sample like the preview's `cell()`.
The native negative-coordinate coarse division behavior is outside this fine
lookup slice. A full contact producer must preserve the explicit fallback.

Cell construction uses signed-word positions in 256-foot units: horizontal
coordinates `col<<5`, `row<<5`, with wrapping word additions for neighbors;
heights are unsigned T2 elevation bytes. Let A/B/C/D be `(x,z)`, `(x+1,z)`,
`(x,z+1)`, `(x+1,z+1)`. The normals are computed in order `(C,D,A)`, `(A,D,B)`.
Origin A is shifted 16 into fixed8 feet; class comes from A. Both endpoints
strictly above the maximum corner height (using signed low-word `Y>>16`) skip
the cell. Equal normal words merge the two planes. Otherwise local Z >= local X
belongs to the first triangle, and local Z < local X to the second. Cell maxima
are exclusive; minima inclusive. Candidate ties preserve the earlier plane.

Normal generation computes `(third-second) cross (first-second)` with wrapping
32-bit products, then arithmetic-right-shifts all components until each native
signed absolute value is <=20,000. It sums wrapping squares and calls the native
square-root helper. The final components truncate `component*32767 / (root&65535)`
and narrow to signed words. Degenerate division faults are explicit Rust errors.

The square-root helper reads **1024 unsigned dwords at VA 0x51d624**. It chooses
index shift / seed shift `(22,13)`, `(16,16)`, `(10,19)`, `(4,22)`, or `(0,24)`
from the highest nonzero mask `fc000000`, `03f00000`, `000fc000`, `00003c00`.
A zero seed returns zero; otherwise it performs exactly `(n/seed + seed)>>1`,
with unsigned dword addition. It is not an exact host square root. The seed table
is extracted to ignored local data, never regenerated or embedded as retail bytes.

Horizontal intersection subtracts the type offset from the plane height. If the
start is below, it returns start X/Z with Y clamped up to the plane. Otherwise
an endpoint on or above the plane is no hit. For crossing, it halves numerator
and denominator arithmetically until both <=200 and uses wrapping **32-bit**
horizontal products. A zero denominator falls back to the clamped start.

Sloped intersection computes signed integer-foot plane distances, including
`offset>>8`; multiplications wrap before signed division by normal Y. Two
nonnegative distances are no hit; two negative distances return the unchanged
start. A crossing ratio is halved until both values are <10,000. Interpolation
uses **64-bit** products, truncates toward zero, then clamps interpolated Y to
zero. Bounds/diagonal tests follow all intersection paths. These distinct rounding
and below-plane rules must not be unified into a floating-point ray cast.

`terrain_contact` translates these pure helpers and vertical cell construction.
It returns a point and normal, **not** a complete ground sample. The downstream
candidate reducer converts normals through `0x411a40` (which calls `0x4c6c30`,
square root and atan) and subtracts PA `0x3ffc`. That angle producer and heading projection are translated as described below.

## Shape-relative contact offset — E007

`0x42e0c0..0x42e0f4` takes the resolved type pointer. It reads the shape pointer
at type+0x0f. A null shape or a non-F2 word at shape+0x0e returns the zero fallback
record at `0x4f1690`. With F2, an **unsigned** word at shape+0x10 locates the
record at `shape+0x12+link`. The ground offset is its signed word +8, shifted
8 by the dispatcher. This is a shape contact field, not PT gear height or fitted
CG clearance. The selected RUNWAY.SH has a record at CODE offset 3460 and offset 0.

`shape::contact_offset` uses the bounded PL/PE CODE reader and bounds all bytes
through the consumed word. It returns `None` for absent F2; a truncated/out-of-range
link errors instead of becoming the native zero fallback. A missing shape pointer
is handled by the future type resolver, not by passing an empty byte buffer.
The remaining record fields and collision subrecords are not decoded here.

Next: trace STRIP initialization,
placement and drawing dependencies, then integrate staged query state with
source-order traces and late-failure rollback. Carrier stays gated.


## Candidate and requested-heading angles — NE-00.1c

`0x42de60` passes zero origin (`0x4eb710`) and `normal<<16` to
`0x411a40..0x411aec`. The difference reducer at `0x4c6c30..0x4c6d5f` ORs
wrapping absolute dword magnitudes. Twice, if the mask is unsigned >=0x40000,
it arithmetic-shifts the vector and mask by four. It then shifts by two while
the mask is signed >=0x4000, finally narrowing the vector to signed words.
The helper's shift-count return is unused by this caller.

Heading is native atan(X,Z). Horizontal length is abs(Z) when X=0, abs(X)
when Z=0, otherwise the imported-table square root of X²+Z². Pitch is
atan(Y,horizontal-as-word), clamped to [-0x3ffc,0x3ffc]. The candidate reducer
subtracts 0x3ffc from that pitch with word wrap and initializes roll to zero.
`candidate_angles` translates precisely this zero-origin normal caller, not an
unrestricted position-to-position service.

`project_angles` translates `0x42bd30..0x42bdb1`: let sine/cosine come from the
imported table at requested heading minus candidate heading (word wrapping).
Output pitch is `pitch*cos/32767 + roll*sin/32767`; output roll is
`roll*cos/32767 - pitch*sin/32767`. Each signed product divides separately,
truncating toward zero, then word additions/subtractions wrap. Requested heading
is retained. The query cache stores candidate angles before this projection.
This closes E008's arithmetic dependency, not dispatcher/cache or world ownership.
[Validation](../baselines/native-land-angles.md).
