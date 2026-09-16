# Native STRIP initialization and shape metadata

> **Research notes — research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature — see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


2026-09-15, NE-00.1d/e/f / E003–E005 under the
[frozen plan](../research/native-environment-systems-plan.md). **Native static source**
from the exact [reviewed EXE/SMS](native-flight.md). The bounded box reader,
midpoint arithmetic, mission nationality conversion and candidate-list operations
are translated/tested. NE-01.1a adds bounded STRIP definition metadata below.
No runway is runtime-connected; retail comparison is unavailable.
[Metadata validation](../baselines/native-strip.md),
[lifecycle validation](../baselines/native-strip-lifecycle.md),
[placement validation](../baselines/native-strip-placement.md).

## Shape box list

`COLGetBox` at `0x42e100..0x42e134` calls the
[F2 record resolver](native-land-contact.md#shape-relative-contact-offset--e007).
Starting at record +16, each subrecord has:

| Offset | Width | Meaning established by consumers |
| --- | --- | --- |
| +0 | byte | Bit 7 means another box record; other bits retained, not interpreted |
| +1 | byte | Lookup ID; first matching ID wins |
| +2/+4 | signed words | First coordinate pair |
| +6/+8 | signed words | Second coordinate pair |
| +10/+12 | signed words | Third coordinate pair |

Advance 14 bytes for each nonmatching record. A byte with bit 7 clear terminates
the list and lookup returns null. The initializer dereferences required results;
the host must reject absent required records instead of inventing zero boxes.
Duplicate IDs and source order are preserved. `shape::contact_boxes` validates
the complete list, unlike native lookup's early return. Its 4096-record limit
is a host input bound, not a recovered native capacity. Non-F2 returns `None`;
an empty terminated F2 list returns `Some(empty)`; truncation is an error.
The 16-byte parent record and other flag semantics are still only partly known.

## Callback resolution and initialization

`0x463f30..0x463f5c` tries an instance callback selector at object +0x6c first.
If absent or returning null, it calls the type selector at type +0x7d.
`0x463f60..0x463f94` invokes the returned callback for the current object, or
returns zero when no callback exists. Imported code remains inert in this project.

STRIP.OT explicitly selects `_STRIPProc`, `0x4be640..0x4be675`.
Its unsigned byte dispatch uses selector bytes at `0x4be68c` and five target
addresses at `0x4be678`:

| Request | Returned callback | Established scope |
| --- | --- | --- |
| 0 | `0x4be2a0`, STRIPAddProc | Shape point initialization and airport registration |
| 3 | `0x473a40`, OBJEventProc | Shared event handler; downstream effects remain NE-07 |
| 6 | `0x48d3c0`, SAYDefaultSayProc | Shared speech callback; not implemented |
| 7 | `0x48f6a0`, APCommentProc | Comment callback; not implemented |
| 1, 2, 4, 5, >7 | null | No callback from this selector |

The byte selectors are `[0,4,4,1,4,4,2,3]`. No autonomous behavior is scheduled
or implemented by this investigation. Noninitialization callbacks remain explicit
unsupported edges rather than silently treated as successful no-ops.

STRIPAddProc reads box IDs 0x25–0x2c in order, then 0x11, 0x17, 0x12, 0x18.
For the first eight and 0x11/0x12, each coordinate is the signed dword sum of
the pair followed by arithmetic right shift one and narrowing to a word.
Negative odd sums round downward, not toward zero. `ContactBox::midpoint`
preserves that rule. These ten local points populate the static airport template
at `0x50ccc8`, offsets 0x14 + 0x12*i for i=0..8, and offset 0xc2.

IDs **0x17 and 0x18 are orientation records**, not extra position anchors.
The initializer copies each pair's first word to template +0xb6/+0xd4.
The transform helper uses the first two words as optional angle adjustments
through `0x417f00`; it does not transform them as XYZ positions. Units/angle
composition require that consumer's separate contract.

Type flag bits 0x40000/0x80000/0x100000/0x200000 become template bytes
+0xe9..+0xec. Current object ID `0x4f6fbc` becomes template word +0xe6.
`0x4bd950..0x4bdb29` transforms the ten points using current object position
and angles, and copies/composes the two orientations. `RotatedOffset` at
`0x411d10..0x411dda` shifts position inputs by eight, applies the native matrix,
then adds fixed8 object XYZ. This path does not use the preview mesh's scale.
The position inputs are whole feet; first/second/third components feed X/Y/Z.

`0x4ba800..0x4ba867` scans airport records in forward order for the same +0xe6
ID, overwrites an existing match, or appends if count <40. Each record is 0x134
bytes. Full capacity returns false. STRIPAddProc returns zero on success and
one on failure. The creation caller treats a nonzero callback result as failure.
Template defaults and airport registration remain untranslated; reset/removal
and failed creation are further bounded below. A passing box diagnostic is not
airport-manager acceptance.

## Mission and current-object state

`MISSIONTextProc` zeroes a 0x37f-byte object scratch record on `obj`
(`0x482475..0x48248f`). The position branch at `0x4825f6` reads three integers,
shifts each left eight, and stores scratch +0x11/+0x15/+0x19. The angle branch
at `0x48265a` multiplies each low word by 182, stores +0x1d/+0x1f/+0x21, and
preserves word wrapping. Textual degrees are not converted with floating point.
The `.` branch passes the type name, scratch record and zero requested ID to
`0x4a73b0` at `0x482de1`. Full text tokenization/fields are not translated here.

The creator loads the type, initializes a temporary kind-2 instance, and allocates
0xde + type's signed instance-size word. `0x4628b0` selects the current object
and copies its instance/type into globals `0x50ce80`/`0x50d268`; `0x462980`
stores current instance state back and clears the current ID. A host producer
must own typed state rather than exposing these native global scratch buffers.

Initialization copies nationality, controller byte, position and angles. It masks
source flags with 0xff8106f7, initially adds 0x4000 for kind 2/4, and copies type
word +0x49 to instance +0x0e. Outside modes 3/12 it calls `0x4abab0` request 1
with angle output. If original Y <= queried ground, it sets Y to ground; when
original Y is zero, it also adopts query angles and sets an internal marker (later storing word +0x56 = 1).
Other type/mode branches remain excluded until independently traced.

After storing/reloading initial state, type 1 maps to instance kind **0**
(`0x4a762c`, dispatch table `0x4a7a1c`); STRIP is not left as kind 2. Then
`0x4a77e4` registers collision candidates, stores/reloads, and requests callback 0.
Thus initial ground queries precede STRIPAddProc and airport registration.
This exposes an initialization dependency on E001/E002 rather than permitting
text Y=0 to become an arbitrary flat runway.

Collision registration `0x42e540..0x42e5bf` requires instance flag bit 1 and
type flag bit 1. It deduplicates current ID in a 900-entry list. Only a newly
added candidate whose type flags intersect 0x408000 can enter the second,
450-entry list. Capacity skips additions; it does not report an error here.
These source capacities are separate from the 40-entry airport registry.

## Resource and unsupported boundaries

Selected original roots and hashes remain in the
[foundation baseline](../baselines/native-land-foundation.md).
RUNWAY.SH has 23 bounded boxes and all twelve required lookup IDs. Existing
partial static projection produces 63 faces with `_RUNWAY.PIC` and no encountered
state guards. It skips drawing opcodes/branches; this is **not** proof of complete
LOD, shadow, palette or resource closure. The named PIC is now extracted with
archive/hash provenance. Full drawing traversal and visual inspection remain E004.

Next: finish template field consumers, full type-load and scheduling closure;
validate all placement fields and collision-channel consumers.
Resolve E004 drawing/texture/palette dependencies independently. Then implement
transactional world construction and ordered queries, including failures after
cache/RNG mutation. Neither aircraft's live contact stop changes in this slice;
carrier remains gated by NE-06 and contact events by NE-07.

## Type setup, final store and cleanup — NE-00.1e

**Source established for the bounded paths below; candidate-list translation
tested; world construction and live contact remain unconnected.** Same reviewed
EXE/SMS identities. [Validation](../baselines/native-strip-lifecycle.md).

### Type resolution

`T_AddObj` requests its named type through RMAccess `0x4a6ae0` with mode 0x8000.
The resource setup notification at `0x4a6df0` forms a symbol name from `_Setup`
(string VA 0x50a654) and the filename extension returned by `0x4a6860`.
`SMCallByName` at `0x46a570` concatenates those strings, resolves the symbol,
and invokes it if present. Thus the OT setup target is `_SetupOT`, 0x4a6eb0.
This is a source dependency, not permission to execute imported code.
The complete resource-loader/BRF relocation and lookup lifecycle remains open.

`SetupOT` first obtains type data through `0x4a6b10`, then tests byte 0x50a620.
Its static default is 1; a separate metadata-loading path at 0x41cae0 temporarily
sets it to 0 and restores 1 at 0x41caf4. The flag is an explicit dependency;
setup must not be assumed for every resource inspection.

For STRIP class word **0x0100**, both high-byte masks 0xc0 and 0x3e are clear.
The generated damage-name branches are skipped. Setup calls `0x4a71e0` on type
slots **+0x0f, +0x13, +0x17**, in that order. Each nonnull name becomes the
result of RMAccess(name, 0x8000); a null slot remains null. The selected STRIP
has only +0x0f (`runway.SH`) populated. This closes the generated-shape-name
question for this class/path, not SH drawing/LOD/palette closure or arbitrary OT
classes. Do not synthesize RUNWAY_A or RUNWAY_S from reference naming patterns.

### Template and airport list

The static template at **0x50ccc8 is 0x134 bytes**, and is not zero-filled.
The hash-gated static pass exports it as `tables/strip-template.bin`; it remains
inert external diagnostic data. Its first five dwords are callback addresses,
not host function pointers. NE-00.1h below reviews three predicate consumers;
the first two callbacks and complete downstream ownership remain open.
STRIPAddProc overwrites its local position/orientation inputs, type-derived flag
bytes and current object ID, then transforms and copies the full record.
Uninterpreted defaults must be preserved or explicitly unsupported; they cannot
be replaced with guessed zero values. Full field consumers and template mutation
ownership are still open.

`APInit` at `0x4ba7e0..0x4ba7fa` clears the active airport count at 0x58b828
and 0x870 bytes of separate state beginning at 0x58e870. It does **not** clear
the STRIP template or the record storage at 0x58b850 in this routine.
`APDelete` at `0x4ba870..0x4ba8de` searches current records for an ID at +0xe6,
removes the first match, shifts later 0x134-byte records forward in order and
decrements the count. No match leaves the list unchanged. Neither routine
establishes complete mission restart or per-object death cleanup.

### Creation completion and failure

After callback 0 returns, `0x4a7801` tests **AX**, not the full return dword.
Nonzero follows `0x4a7806`: store current state, then call `0x491490`.
That helper decrements the word allocation count at 0x553838 and subtracts the
last allocation's stored word size (table 0x553120) from 0x553828. It does not
clear the object pointer table or unregister collision/airport entries.
Mode 3 returns zero; other modes call the error path 0x44a420 before the zero
return sequence. That error handler's downstream recovery is not established.
The selected object's separately allocated name is another owned resource;
the bounded failed-add path does not visibly free it.

Consequently, **ordinary removal is not the native failed-add rollback path**.
Collision registration already ran before the callback. The host must stage
objects, allocated IDs, owned names, template/airport state, candidates and
query/cache/RNG changes together and discard failed construction. This atomic
host failure contract is required by the frozen plan; it is not a claim that
retail performs complete rollback. No staged world implementation exists yet.

On success, `0x4a7839` calls `0x4beb90`. STRIP type flags include 0x8000,
so that helper returns immediately at 0x4bec5a: it does not perform its later
touching/airport-attachment queries for STRIP. Instance flag mask 0x2 then
selects scheduling registration at 0x4626b0. The selected placement retains
this flag; scheduling is still a required, unaccepted edge. With byte 0x5528bc
clear, creation proceeds directly to `0x4a7a06`, stores current state and returns
the allocated ID. The special-mode branch when that byte is set remains gated.

### Collision candidate removal and diagnostic translation

`0x42e5c0..0x42e679` requires **type flags & 1**, independently of instance
flags. It removes the first current-ID match from the primary list, shifting
following IDs without reordering. It then independently checks the secondary
list if type flags intersect **0x408000**, even if primary had no match.
Changed type flags are not automatically reconciled: clearing that mask can
leave a secondary ID behind. A duplicate primary registration likewise returns
before retrying secondary insertion, even when secondary capacity is available.

`tore_sim::native_objects::CollisionCandidates` translates only these ordered
registration/removal operations, with private lists capped at **900 / 450**.
No parsed object, scheduler, airport callback or live query uses it yet. It is
caller-owned cloneable diagnostic state for later transactional construction;
these tests do not prove whole-world rollback or reset. General object removal
at 0x4627b0 calls collision removal at 0x462835 amid other unresolved lifecycle
effects; those downstream systems remain outside this translation.

## Remaining selected placement fields — NE-00.1f

The mission scratch record begins at stack +0xb8 in MISSIONTextProc. Field
conversion below is **native source established**, with only nationality
conversion initially translated/tested. NE-01.1b below now parses the isolated
selected record and translates its remaining width/scale conversions. Full mission
loading and world construction remain unimplemented.

| Text field | Source path / representation | Remaining boundary |
| --- | --- | --- |
| type | 0x4824b1..0x482583 reads a token, rewrites leading `$` to `~`, uppercases ASCII lowercase letters | Special named substitution and full type loading remain open; selected STRIP.OT does not use substitution |
| alias | 0x4825b5 reads an integer and keeps AX in a separate temporary; 0x482e3d writes it to instance +0x74 **after** T_AddObj succeeds | Not the allocated object ID; default is zero at 0x48249e |
| nationality | 0x4826c7 narrows the integer to a byte; preserves bit 7, increments the low seven-bit value when >=8, recombines, then calls 0x483d50 | Map-name-dependent conversion below; do not copy text nationality directly |
| flags | 0x4827eb stores the integer unchanged at scratch +1 | T_AddObj applies its documented mask/initial bit before registration |
| speed | 0x48282e shifts integer left eight into scratch +0x34 | Zero for selected STRIP; no inferred knot/velocity conversion beyond reviewed fixed8 storage |
| name | 0x483bbd scans between byte-1 delimiters, copies at most 40 bytes, NUL-terminates at min(length,40), advances beyond closing delimiter | Host must bound both delimiter scans; native missing-delimiter behavior is not a safe parser specification |

Nationality remap `0x483d50..0x483dd5` tests the **first byte of the native map
name at 0x4fb1c8**. The map-token path at 0x481fbb supplies that string. Only
`T/t/U/u/K/k` enables the remap; it does not strip a leading tilde. After the
initial increment, low seven-bit IDs map **5→23, 6→24, 13→22, 14→20, 15→21**;
others pass through, with bit 7 preserved. The selector at 0x483df0 has 11
bytes and six target addresses at 0x483dd8; both were checked against consumers.
Raw 127 becomes 128 during increment/recombination, rather than wrapping the
seven-bit portion independently. `mission_nationality` preserves this boundary.

For the selected `map ukr.T2`, textual nationality **137 becomes 138**, then
T_AddObj copies it from scratch +9 to instance +9. The distinct controller byte
at +0x10 remains zero in the selected scratch record. This distinction matters
for later scheduling predicates. No alliance or autonomous behavior is inferred.

Mission post-creation is not finished when T_AddObj returns. The loader reloads
the object, applies separately parsed fields (including alias), then stores again
at **0x482ee5**. For static kind 0 it bypasses the kind-2/4 loadout branch.
For the selected eight-field record, the reviewed `obj` reset at 0x48248f
sets optional words stack +0x1c/+0x20 to 0xffff, alias +0x18 to zero and the
name first byte to zero. The cleared controller bit 0x80 skips 0x4918d0 at
0x482dfe independently of global 0x4eb604. The two 0xffff words skip
0x45e490/0x45f1c0. After alias storage, kind 0 bypasses the kind-4 fuel and
kind-2/4 loadout branches and reaches the final store at 0x482ee5. Thus these
optional effects are excluded for the selected record by source predicates.
Their enabled branches remain unsupported; this does not close earlier creation
callbacks, scheduling or query ownership.

## Scheduling ownership discovered from creation

**Source only; scheduler translation/runtime not accepted.** `0x462600` clears
the two queue heads and separate current-object stack/auxiliary state.
`0x4626b0` removes current membership, sets instance word +0x68 to zero, and
inserts into the primary queue. `0x4626d0` writes the low word of time 0x552928
to +0x66, sets instance flag mask 2 and links through +0x64.

With byte 0x4f6fc0 clear, insertion is at the head. Its static default is 1;
when set, traversal compares +0x68 as **unsigned** and passes existing values
<= the new value before insertion. Kind-6's additional path is outside STRIP.
`0x462620` removes from both queue heads when flag mask 2 is set. Linked-list
ownership, current scratch versus stored instance state, bounded traversal and
restart require a shared staged owner, not unrelated copies of queue vectors.

The due-service call at 0x462abc enters 0x462e70. NE-00.1g below now establishes
the dispatcher and final delay predicates and translates the kind-0 delay selector.
Callback bodies, world/global producers and complete RNG interleaving remain open;
the diagnostic does not establish that every STRIP service draws RNG.

## Bounded STRIP definition metadata — NE-01.1a

`strip::Definition::parse` reuses the bounded BRF grammar and packed OBJECT field
order from [aircraft formats](aircraft.md), requiring a complete root layout,
type 1, type size 166, instance size 0, class 0x0100, three identity strings with
resource identity STRIP.OT, and the inert `_STRIPProc` selector name. It resolves
the identity and main-shape labels from their actual pointer tokens. The reviewed
null +0x13/+0x17 shape slots are required; additional dependencies are rejected.

The reader exposes flags and an uppercase explicit SH name while retaining the
entire BRF token tree, including uninterpreted numeric values and `^` markers.
It does not silently apply scaling to unknown fields. Scaling markers on the
interpreted header, identity or shape fields are unsupported and rejected.
The existing 1 MiB BRF cap, exact root length/kinds and resolved-label checks
apply. The shape name must be one bounded ASCII leaf ending in `.SH`, at most
12 bytes; path separators, parent components and unsupported characters fail.
These are host input limits, not a claim that every native OT name follows them.

`native_strip RUNWAY.SH [STRIP.OT]` optionally validates the definition and
checks its shape name against the supplied SH basename, then runs the existing
box/partial-projection diagnostic. This does not validate source archive identity
by basename alone: keep extraction reports and reviewed hashes with the inputs.
Full OT field semantics, resource-manager behavior, shared app/CLI profile
resolution, placement, templates, scheduling and E004 drawing closure remain
separate gates. No definition token or symbol executes code.
[Validation](../baselines/native-strip-definition.md).

## Service dispatch and delay ownership — NE-00.1g

**Native source established for the dispatcher and priority predicate; only the
kind-0 post-callback delay selector is translated/tested.** No scheduler or
callback is runtime-connected. [Validation](../baselines/native-strip-service.md).
The static extractor now includes aligned `0x462e70..0x4631a9`,
`0x464550..0x464637` and the word-bound RNG wrapper `0x4562f0..0x4562fb`.

The dispatcher has an early special-current-ID branch (word 0x4eb6f0), with
controller-high-bit/kind-4 gates and a call to 0x4164b0. It is outside the selected
STRIP path. Otherwise it snapshots position/orientation and sets instance +0x68
to 0x7fff before callbacks. If controller low seven bits differ from dword
0x4eb608 and type flags lack bit 8, it calls 0x46c520 and skips the ordinary
callback body. The ownership/producer of that global remains required; nationality
+9 must not substitute for controller +0x10.

The ordinary path tests controller bit 0x80 and instance flag 0x80000, otherwise
calls 0x4631b0, then, while instance flag 1 survives, requests callback 2 with
argument 1, calls 0x436b30, requests callback 2 with argument 0, requests callback
5, calls 0x4631f0 with the earlier byte result, and requests callback 7. Flag 1
is rechecked between these operations. STRIP's type selector returns null for
2 and 5 but **returns APCommentProc for 7**. An instance selector could override
these results and is not accepted here. The bodies of 0x436b30, 0x4631f0,
0x46c520 and APCommentProc remain unresolved service dependencies. Their names
or static-object status cannot justify dropping them. No autonomous behavior
is translated or enabled by this ledger.

Kind 0 skips the kind-2 orientation-change test and clears flag 0x800. At the
service tail the following ordered rules apply:

1. If the post-callback word +0x68 differs from 0x7fff, retain that delay.
2. Otherwise, set delay zero if unsigned word +0x6a >= word clock 0x5528c8,
   or controller bit 0x80 is set, or priority helper 0x464550 returns true,
   or nonzero object ID 0x520a1c resolves to an object whose word +0xee equals
   the current ID. Native evaluation short-circuits in this order.
3. For kind 0 only, remaining speed +0x34 >0 (signed dword) requests bound 8;
   zero/negative speed requests bound 20. Add 2 to the returned word.
4. Add word clock 0x5528c8 to the selected delay with 16-bit wrapping and store
   +0x68. This clock is a distinct input from the dword time used by E002.

Kinds 2/4 have additional byte +0xe3 ==0x20/0x21 delay-1 cases; kind 6 has a
delay-2 case. They are not translated by `StripServiceTail`. The diagnostic
accepts post-callback samples and emits either a final `At(word)` or explicit
`Draw { base, upper_bound }`. It makes no object lookup or RNG call and cannot
replace the unresolved service body. Boolean predicate samples do not establish
native short-circuit lookup/failure ordering; a future producer must preserve it.

### Priority helper and shared RNG

`0x464550` returns true when the ID matches word 0x4eb64c or 0x4eb64e, or the
stored instance controller has bit 0x80. It also checks the sum of absolute X/Z
differences from globals 0x4eb650/0x4eb658 against **0x753000 inclusive** using
wrapping 32-bit arithmetic and signed comparison. Y is not used. It then visits
eight ID slots at 0x4eb610..0x4eb61f in order, skipping zero slots. Each can pass
the same X/Z distance test; kind 2/4 entries also pass if their word +0xee equals
the tested ID. The global/ID-table producers remain open. This is not established
as a renderer visibility or Euclidean-distance predicate.

Both delay calls enter wrapper 0x4562f0, which masks the bound to 16 bits and
jumps to the existing shuffled generator 0x4561d0. It mutates the same seed at
0x4f6bc8, shuffle output at 0x4f6bcc and 32-entry table at 0x546838 used by the
previously reviewed native RNG helpers. A separate per-STRIP seed would break
this source sharing. Initial seed, complete service callback draws and global
interleaving remain open, so source sharing does not close replay acceptance.

Next: finish selected post-create optional-field predicates and service-body
closure, trace required airport template consumers, and independently close E004.
Stage E001/E002 only after the required ownership paths are accepted or explicitly
excluded with evidence. Carrier and unsupported callback branches remain gated.

## Bounded isolated placement — NE-01.1b

`strip::Placement::parse` reads one isolated `obj` through `.` record. It requires
exactly one each of `type`, `pos`, `angle`, `nationality`, `flags`, `speed`,
`name` and `alias`; type must equal STRIP.OT ignoring ASCII case. Field order
may vary. Unknown, duplicate, missing, nested or trailing records fail. Special
type substitution, optional post-create fields and general mission parsing are
unsupported. Requiring all eight fields is a host restriction, not native syntax.

The host grammar accepts LF/CRLF, space/tab indentation and blank lines, signed
decimal i32 or `$` followed by one to eight hex digits interpreted as dword bits.
It rejects comments, plus signs, expressions, other numeric encodings and NUL.
The 4096-byte whole-record cap bounds both scans and allocations. These are
explicit safety/subset rules, not a reconstruction of the full native tokenizer.

Source integers and the exact input bytes remain available. Position/speed
conversion wraps a dword shift by eight; angle conversion multiplies the low
word by 182 with word wrapping. Nationality narrows to the raw byte only; the
separate map-dependent `mission_nationality` operation is still required. Flags
remain source bits before T_AddObj's mask. Alias narrows to a word for the later
post-create write, never an object ID. Names require byte-1 delimiters and retain
all payload bytes, including non-UTF-8 and suffixes beyond 40 bytes. Control bytes
inside names are unsupported. `native_name()` exposes at most 40 bytes before
the native trailing NUL. It does not split or reinterpret a source character set.

`native_strip RUNWAY.SH STRIP.OT ISOLATED-PLACEMENT` inspects these inputs.
Selecting a record from a larger MM is a research step, not a full mission
resolver. Zero Y remains an input to the initial native ground query; no world,
airport, candidate, scheduler or query state is constructed by this reader.
[Validation](../baselines/native-strip-record.md).

## Airport ownership and comment preflight — NE-00.1h

**Source established for the bounded routines below; no translation or live
callback activation.** These refinements of E016/E019 add E020 (ordered actor
IDs, lookup/current-object ownership and speech scratch). Same reviewed EXE/SMS
identities; [extraction and validation](../baselines/native-strip-ownership.md).

### Airport records are mutable, independently of the source template

`0x4bd2d0..0x4bd30f` returns the first airport record whose word +0xe6 matches
the supplied ID, or null. It scans count 0x58b828 in forward order with stride
0x134 from 0x58b850. This is the same ordered storage copied by APAdd, not a
lookup into the static template. Native callers can retain pointers into this
compacting storage; host identity/lifetime handling must avoid stale references.

`0x4bd310..0x4bd3c9` clears exactly 18 bytes at +0x111..+0x122 of each active
airport record. It then scans the separate actor-ID list described below and
changes instance +0xe3 in inclusive ranges 1..0x12 or 0x13..0x1e to 0x1f. It
does not clear the template, erase airport records, or clear comment state at
+0x127 onward. The direct caller at 0x4242b5 is a scheduling/lifecycle lead,
not proof that this reset occurs on every service. Its complete caller context
remains open. This must remain distinct from APInit's count/auxiliary reset.

Three initial template callbacks now have bounded consumer evidence. They read
the current instance's airport pointer at +0x231, not the static template:

| Callback | Source-established inputs and predicates | Open dependency |
| --- | --- | --- |
| +8 → 0x4bab20 | Touching query first; null airport then fails. Replace current Y with record +0xae and compare approximate distance to point +0xaa against signed 0x7d00 inclusive | Native touching producer and attached-record ownership |
| +0xc → 0x4bab80 | Null airport / byte +0xea clear fail before touching. Heading delta against +0xda passes 0x4c6614, folds around 0x7ff8 when AX >=0x3ffc, requires signed AX <=0x1554; distance to +0xc8 <=0xc800 inclusive | Angle helper, touching and pointer lifetime |
| +0x10 → 0x4bac00 | Null airport fails. 0x411af0 on point +0xc8 must return signed AX <0x1ffe; 0x4c6614 heading/pitch deltas against +0xda/+0xdc must each return signed EAX <=0x1ffe | Direction helper and airport attachment producer |

Distances use the already reviewed 0x4c66cc approximation (largest absolute
component plus quarters of the others), not Euclidean length. Preserve each
caller's word/dword comparison widths. These are predicate contracts, not
permission to call inert template pointer words or activate approach behavior.
The first two template callbacks and other nonzero defaults remain unaccepted.

### Comment callback selection and observable early exit

`APCommentProc` begins at 0x48f6a0 by calling 0x48d410, which zeroes bytes
0x552ff0 and 0x553050 (two buffer starts). Only then does it compare unsigned
word deadline 0x552fdc with word time 0x5528e0; deadline > time exits. Thus even
a suppressed callback is not an effect-free no-op. Clock/deadline/buffer producers
and later speech dispatch remain required owned state under NE-07a/E020.

If not suppressed, it obtains the current STRIP's airport record through
0x4bd2d0, then scans IDs at 0x5713a8 with signed-word count 0x570ef0. Each ID
resolves through 0x491240. Eligibility requires instance flag 1, controller bit
0x80, byte +0xe3 in 1..0x12 or 0x13..0x1e, the same nationality high bit as
the STRIP, and instance airport pointer +0x231 equal to the looked-up record.
A null record is not checked separately in this prefix; host construction must
validate references rather than inherit unsafe later dereferences. Do not assume
this list is empty simply because autonomous behavior is excluded.

An eligible actor begins with byte rank 0 for the first range or 0x80 for the
second. A successful 0x45e710 call adds its returned byte with byte wrapping.
The first lowest unsigned rank wins; ties retain the earlier ID. With no chosen
ID the callback exits through 0x49008b with the current-object-switch flag clear.
After selection it switches current object via 0x4629e0; the middle action/speech
branches remain unaccepted. The exit tail calls 0x462a20 only if that switch flag
is set. These helpers' complete stack/lookup failure ownership remains open.

The reviewed finish slice 0x490041..0x49009f writes airport word +0x129=current
actor ID, byte +0x12b=current +0xe3, word +0x12c=distance-derived value; it calls
0x48e950 and conditionally schedules +0x127 through 0x48d5e0 plus word time.
Earlier unaccepted action branches also access +0x12e and +0x132. These are
mutable per-airport service fields, not immutable template defaults. No claim
is made that their generators share or do not share the flight RNG yet.

### Ordered service actor list

The bounded list producer at 0x49fa50 scans for a duplicate current ID first:
duplicate returns 0 unchanged even when full; count >=60 returns 1; otherwise
append and increment count, return 0. `0x49d520` removes the first supplied-ID
match and compacts in forward order. `0x49d510` resets count only. These are
separate from the 900/450 collision candidates and 40 airport records. The
registration function address is selected at 0x49fb2e; NE-00.1i below resolves
the plane selector and bounded state/attachment producers. Complete creation
and refresh callers remain open. No autonomous behavior or
aircraft service is translated by recording this list's storage contract.

## Airport slot and attachment producers — NE-00.1i

**Native source established for the bounded slices below; untranslated and not
runtime-connected.** [Validation](../baselines/native-strip-slots.md). This
refines E016/E020; it does not close the full takeoff/landing callback bodies.

### Separate plane registration

The SMS identifies `0x49fb10` as `_PLANEProc`. Its eight-entry dword jump table
at `0x49fb4c` selects request 0 → `0x49fa50` (the ordered actor-list registration),
2 → `0x49f840`, 3 → `0x49df40`, 6 → `0x48d780`, and 7 → `0x48ec40`.
Requests 1/4/5 and unsigned byte values >7 delegate to `0x473db0`; they are not
proven null. The selector's reviewed code ends before the table, at `0x49fb4c`.
This establishes which selector registers the actor list, not all definitions
that use that selector or the complete plane creation lifecycle. In particular,
STRIP's request-0 airport registration is distinct. No empty actor-list assumption
or new aircraft activation follows from this discovery.

### Airport slots and partial failure

Three complete routines use current instance +0x231 (global `0x50d0b1`), current
ID `0x4f6fbc`, airport signed word +0xe0, and word slots beginning +0x111:

- `0x4bd3d0..0x4bd418`: null attachment returns -1; otherwise scan the first
  +0xe0 slots, return the first matching current-ID index, or -1. Nonpositive
  signed counts produce no match.
- `0x4bd420..0x4bd48e`: null attachment returns -1 without releasing anything.
  An existing matching destination slot returns immediately. Otherwise call
  release **before** scanning the destination's slots for the first zero word;
  store the current ID there and return its index, or return -1 if none is free.
- `0x4bd490..0x4bd509`: visit every airport record in order and zero **every**
  matching current-ID slot within each signed +0xe0 count. Leave holes in place;
  do not compact slots. This is distinct from airport-record or actor-list removal.

The reviewed STRIP template's +0xe0 word is **9**, matching the 18-byte reset
at +0x111..+0x122. The selected add/transform path preserves that count. Other
templates/counts are not accepted by this observation. Native code does not
validate these counts against record bounds; a host reader must do so.

A full destination can therefore return failure **after clearing old ownership**.
An existing destination match can leave duplicates in other airports untouched.
Neither behavior is a transactional move or a globally deduplicating operation.
Staged world construction/update must preserve source order within staged state,
and discard the entire staged change on a host failure. Calling release as
rollback would erase more state. No host allocator is connected in this slice.

### Attachment refresh and state transitions

`0x452594..0x452627` is the bounded attachment tail inside SMS
`_FMUpdatePlaneFields@0` (entry `0x452140`), not a complete field-update routine.
It saves the previous attachment, calls `0x45e710` for the current ID, and on
byte result 2 obtains an ID through `0x45e630`, resolves it with `0x491240`, and
copies that instance's +0x231. A nonnull attachment is retained if current +0xe3
is in 1..0x12 or 0x13..0x1e. Otherwise it calls `0x4ba8e0` with current instance,
position, null optional outputs and final argument 1, then stores the result.
If the attachment differs from the saved pointer, it calls slot reserve above;
the return value is not tested here. Group lookup, earlier field updates and
safe attachment lifetime remain required dependencies; this tail cannot be
called independently or treated as full producer acceptance.

SMS `@EnterState@4`, `0x464300..0x46441b`, writes the requested low byte to current
instance +0xe3 after its pre-transition notification. Entering state 0x16 from a
different state, when controller low 7 bits match dword `0x4eb608`, first calls
`0x485a40` with that controller, current ID and instance byte +0x273. For kind 4,
nonnull attachment plus new state in 1..0x12 or 0x13..0x1e reserves a slot;
otherwise release scans all airports. These operations precede the remaining
controller/device effects. With controller bit 0x80 clear, new states 7/0xa/0xd/
0x11 call `0x451b60(1,0)`; leaving range 1..0x12 calls `0x451b60(0,0)` and
`0x451e00(0,0)`. Those device consumers are not accepted by their call addresses.
New state zero additionally calls `0x45e520`, clears word +0x11a and bits 0x30
of dword +0xde, conditionally calls `0x473c10` when byte `0x4f6fe4` is nonzero,
and calls `0x4a04f0` for kind 4. It is not merely an assignment to +0xe3.

### Remaining template callback boundaries

SMS names the first two pointer targets **APTakeoff** (`0x4badb0`) and
**APLanding** (`0x4bc270`). Only their entry gates are reviewed here. Both clear
instance byte +0x215 before examining later conditions. Takeoff resolves the
attached airport's object ID, requires a nonnull object with flag 1 and a true
`0x4747c0` preference result; failing that validation while +0xe3 is 1..8 writes
state 0x1f directly, calls `0x473c10`, zeroes speed +0x34 and returns AL=1.
Other cases continue to the unaccepted body at `0x4bae24`.

Landing first calls `0x463d00` and group lookup `0x45e710`. Group result 2,
false `0x45e8f0`, and state 0x13 or 0x14 call `0x4bbfe0` then return AL=0.
Otherwise it resolves the attachment's airport object and tests flag 1 and
`0x4747c0`, branching to unaccepted exit `0x4bd170` on failure or body `0x4bc310`
on success. These gates do not establish effect-free suppression. Complete
callback selection/callers, bodies, device effects and attachment lifetimes remain
unknown; no autonomous behavior is translated or enabled.

## Current-object switches and speech timing — NE-00.1j

**Native bounded source; only the pure speech-delay arithmetic is translated/
tested.** [Validation](../baselines/native-strip-speech.md). E019/E020 callback,
event dispatch and global clock ownership remain incomplete.

`0x4629e0..0x462a16` saves current ID in the word stack at `0x4f6fc8` indexed by
dword depth `0x4f6fdc`, stores current scratch through `0x462980` only when the
old ID is nonzero, increments depth, then loads the requested object through
`0x4628b0`. `0x462a20..0x462a4b` stores the outgoing object first, reads the
previous ID at depth-1 and decrements depth. A nonzero saved ID reloads its
instance/type; a zero saved ID only clears current ID, leaving scratch content
untouched. Scheduler reset `0x462600` resets depth to zero. Neither switch routine
checks overflow/underflow or lookup validity. Stack capacity is not established
from adjacency of globals. Host validation and staged scratch/store/stack state
must prevent partial writes; this is not an immutable context switch.

### Speech submission and two different deadlines

`0x48e950..0x48ea0e` returns AL=0 immediately when both buffer starts `0x552ff0`
and `0x553050` are zero. Otherwise it copies both NUL-terminated strings into a
local payload, consecutively **including both terminators**. Payload byte count
is `len(first)+1+len(second)+1`. It calls event enqueue `0x4180a0` with the two
caller-supplied low-word IDs, literal arguments 0, 2, 0x8000, 0x24, and the payload
pointer/length. The enqueue result is not checked here. It then sets global word
`0x552fdc` to `word(0x5528e0 + scaled_delay(3))` and returns AL=1. It does not
clear the two global buffers. NE-00.1k below establishes the ordinary event-record copy and its 200-byte
clamp; complete routing/lifetime and dispatch remain required source closure; do not emit borrowed pointers to stack data.
There is no direct RNG call in this wrapper; this does not establish its callees'
RNG behavior or callback-wide draw ordering.

The already reviewed APComment finish calls this wrapper after recording actor
ID/state/distance. Only AL=0 sets **per-airport** word +0x127 to
`word(0x5528e0 + scaled_delay(1))`. Successful submission updates the global
speech deadline instead. Thus global suppression and per-airport retry state
are different owned fields, not one shared timer.

`scaled_delay`, `0x48d5e0..0x48d5f2`, starts from the input low word. If signed
word `0x5528f8` is positive it shifts AX left by CL; otherwise it returns the
input word unchanged. x86 masks this shift count to **five bits**, even for AX:
counts 16..31 zero the word, 32 leaves it unchanged, 33 shifts by one. This is
not Rust `u16::wrapping_shl`, whose mask would use four bits. Diagnostic
`native_objects::speech_delay` shifts a widened dword with the five-bit count,
then narrows. It owns no clock, event, buffer or RNG state.

### Clock initialization boundary

`0x486a10..0x486a80` writes the first signed dword argument to `0x552928`;
word `0x5528e0` is its arithmetic shift-right by 8, narrowed, while word
`0x5528c8` is signed division by 64 truncated toward zero, narrowed. It resets
scale word `0x5528f8` to zero. The second and third dword arguments are combined
with wrapping arithmetic as `(second*60 + third)*60`, stored at `0x5528e4` and
`0x552934`; it copies `0x5528ec` to `0x552940` and sets words `0x55292c` and
`0x5528e8` to 1. No units for those second/third inputs are inferred here.

This establishes initialization, not all clock/scale producers or the native
scheduler's connection to the host's fixed 120 Hz clock. Later changes to scale,
speech reset/deadline producers at `0x48d2b2`/`0x48d5d4`, comment generation and
event consumers remain open. Existing adapted clock/replay claims are unchanged.


## Static-object service and consuming event lookup — NE-00.1k

**Native bounded source established; no new runtime translation or activation.**
Same reviewed EXE/SMS identities; [validation](../baselines/native-strip-events.md).
This closes the kind-0 event-service caller ledger, not its query/event consumers,
movement body, command producers or complete E019/E020 ownership.

### Movement/query ordering

`0x4631b0..0x4631e4` snapshots current XYZ to `0x546bb0` and instance word
+0xee to `0x546ba4`. The movement prefix `0x436b30..0x436c6f` clears byte
`0x546b88`, samples ground through `0x4abab0` request 1 with angle output and
null water output, then copies the 32-byte command block at instance +0x38.
Only kind 4 makes the additional touching call here. It snapshots XYZ to
`0x538278`, calls `0x4780d0`, computes word body-minus-movement angle deltas,
and adjusts the local command copy from its +0x1e flags before command dispatch.
Neither zero speed nor kind 0 bypasses this prefix. The command table and body
beyond `0x436c70`, command initialization/updates and `0x4780d0` remain open.
No autonomous command behavior is implemented.

### Kind-0 event service at 0x4631f0

The complete caller range ends at `0x463721`; only its kind-0 path is accepted
as the selected STRIP ledger. Other kinds' branches remain unsupported.

1. Save instance event mask word +0x58. Consume an event for current ID with
   mask 0x8000 using `0x4185a0`; if present, dispatch `0x463980(0x8000, record)`.
   A nonzero callback byte ends service. Otherwise consume with mask 0x7fff.
2. If that record's +8 equals 0x4000, copy its 34-byte payload at +0x0d into
   local storage, dispatch 0x4000, then run the preference/notification path
   at `0x463275..0x4632d9` before testing the saved callback result. This path
   can call `0x4432d0`; it remains an unaccepted downstream effect.
3. Kind 0 resolves the current type's F2 record with `0x42e0c0`. The first
   signed dword divided by two, truncating toward zero, supplies the query
   radius; the separate word argument is zero. It is not the F2 +8 height offset.
   Build query mask **0x22**, add bit 0x8 if saved event mask has 0x4000 and
   type flags have bit 2; add bit 1 when instance +0x56 bit 1 is clear; add
   bit 0x10 when type flags bit 4 is clear. Kind-4 and kind-6 additions do not apply.
4. Call `0x42b800` with current ID/instance/nationality, the earlier snapshot
   XYZ and current XYZ, radius/word/mask and output pointers. This is a swept
   service query dependency even for a static type, not another GetGround call.
5. Output ID 0xffff enters `0x4635d0`: ground request **0**, without angle/water
   outputs, raises Y only if below the returned height, then dispatches 0x2000.
   Nonzero/non-ffff output plus saved event bit 0x4000 forms a 34-byte hit payload:
   first byte 100 for kind 0, target ID and its type string, final two bytes zero.
   It dispatches 0x4000; a surviving nonzero payload ID is rewritten to current
   ID with current type string and enqueued to the hit ID before the saved
   callback result is tested. Payload mutation and notification ordering matter.
6. The shared tail repeats the ground/event path if Y <0. Byte `0x546b88`
   requests event 0x1000. Kind 0 bypasses kind-2/4 proximity/attachment events.
   A previously consumed 0x400 record requests event 0x400 with its +2 word.
   Finally a nonzero input byte calls `0x463d40`; false dispatches 0x80 through
   `0x4639c0` directly, bypassing the event-mask gate.

The event mask saved at entry and live mask read by the dispatcher are distinct.
Callbacks may change live state. Do not replace this sequence with an unordered
set of event kinds or assume static placement makes every event impossible.
Full query outputs, preference/notification, hit strings, callback mutation and
final command predicate remain dependencies. The query can mutate E002 state;
its effects and event queue effects require one host transaction.

### Event gate and interception

`0x463980..0x4639b2` returns AL=0 unless current instance flags include 1 and
current event mask +0x58 intersects the supplied low-word mask. Otherwise it
calls `0x4639c0` and retains the returned byte.

`0x4639c0..0x463a11` first tries global interceptor `0x4f6fb8` if nonnull,
passing addresses of mask, payload argument and output byte. True interceptor
return uses that output byte immediately. False proceeds with the possibly
modified mask/payload to request-3 callback resolution (`0x463f60`). STRIP's
request 3 resolves OBJEventProc, not a no-op. Scheduler reset clears the global;
mission setup writes it at `0x480ac2`, so reset alone cannot prove absence during
service. Imported function pointers remain inert. Interceptor initialization,
OBJEventProc and reentrant mutation remain unknown/unsupported.

### Event lookup consumes state, including on a null return

`0x4185a0..0x4186d4` scans **120 records of 0xd5 bytes**, starting `0x522d40`.
It skips deadline +6 ==0xffff, nonintersecting mask +8 and unsigned deadline
> word time `0x5528c8`. Ordinary recipient ID must equal record +4. Special
recipient 0x800b additionally accepts nonspecial object IDs whose instance flag
2 is clear; that path needs object lookup and is not an ordinary-ID alias.

On a match it copies the whole record to shared scratch `0x522c60`, requests
forward compaction through `0x4d78d0` with destination=current record,
source=next record and remaining byte count, then marks the final slot's +6
(`0x529049`) 0xffff. The forward-copy path was inspected separately; the whole
C-runtime helper is not added as one code region across its embedded jump table.

Unless record flag 1 suppresses it, selected player/controller predicates call
speech observer `0x48d350` **after removal**. If scratch flag 4 is clear, return
the shared scratch pointer. If flag 4 is set, advance to the next physical
record and continue, even though compaction shifted a record into the removed
slot. A null result therefore does not imply no queue mutation or speech effect.
Scratch ownership/reentrancy must be recovered before retaining a result across
another callback or lookup. Queue reset, observer and full routing remain open.

### Enqueue RNG, copied payload and wakeup boundaries

The prefix `0x4180a0..0x418189` always calls the shared word-bound RNG with
**bound 100** at `0x41815f`, before recipient expansion, filtering or local queue
capacity checks. Its byte becomes event +1. Thus even a full queue or empty
recipient expansion consumes that draw; speech wrapper submission is not RNG-free.
There may be additional draws in downstream consumers, still unreviewed.

The ordinary record writer `0x4182e8..0x418381` establishes this packed layout:

| Offset | Width | Established value |
| --- | --- | --- |
| +0 / +1 | bytes | Flags / shared bound-100 draw |
| +2 / +4 | words | Sender / expanded recipient |
| +6 | word | Word clock 0x5528c8 plus supplied delay, wrapping |
| +8 / +0xa | word / byte | Event mask / subtype |
| +0xb | word | Payload length |
| +0xd | up to 200 bytes | Copied payload |

Length is compared as a **signed word** and clamped only above 200; nonpositive
length skips copying but is still stored. A future host envelope must reject
negative/invalid lengths and own its bytes, not reproduce an unsafe pointer.
This clamp is inside recipient processing; it does not establish bounds for
all fallback paths in the unaccepted complete enqueue routine.

`0x418433..0x4184b5` searches local slots in order for deadline 0xffff; a full
queue skips this recipient. After writing a local ordinary-object event, an
instance with flag 1 clear invalidates the slot again. A live scheduled object (flag 2) whose
unsigned +0x68 exceeds **widened word-clock +1** is switched in, reinserted through
`0x4626b0`, then switched out. At clock 0xffff that comparison uses 65536, not
word zero. Enqueue can consequently mutate current scratch/stores and scheduling
as well as the queue and RNG. Special recipients skip this wakeup path.
NE-00.1n below establishes queue reset, routing caller order and the speech
observer. Expansion helpers, remote transport and output consumers remain open. No event emitter or scheduler is enabled.

## Initial commands and default event response — NE-00.1l

**Native source ledger, with only command-deadline arithmetic translated/tested.**
[Validation](../baselines/native-strip-commands.md). No command interpreter or
object service is runtime-connected. This refines E019/E021 without implementing
autonomous behavior; unknown commands remain unsupported.

### Creation establishes a command, not an empty service

The already reviewed `0x4a73b0` creation path clears its 0x37f-byte scratch before
allocation, then `0x4a7597..0x4a7608` copies body heading/pitch to movement
heading/pitch and initializes the command block at instance +0x38:

- Heading, pitch and bank mode bytes at command +0/+5/+0xa are zero.
- Speed mode +0xf is 1; its dword value remains zero from reset.
- Both condition bytes +0x18/+0x19 are 7, both thresholds +0x1a/+0x1c zero.
- Command flags +0x1e remain zero; instance event mask +0x58 becomes 0xffff.
  The script pointer +0x5e remains zero in this selected creation path.

`0x436eca..0x436eda` handles heading mode 0 by copying the movement heading to
`0x53826c`, then entering the shared movement body. The bounded helpers at
`0x478090`, `0x4780d0` and `0x477d10` return signed type words +0x5f, +0x5d and
+0x67 respectively for kind 0; kind-4/6 paths are outside this selected contract.
The selected zero-valued type inputs and intermediate pitch/bank path are now
established in NE-00.1m below; full loader and service ownership remain open.

`0x43805e..0x438226` first adjusts speed toward its selected target; **only then**
does zero speed skip position integration. Command flag 1 can still replace Y
with the earlier ground sample. It evaluates condition 0, then condition 1 only
if the first is false; either true sets the return byte. Kind 0 skips the final
kind-4 field clears/update. Zero motion alone does not suppress query, command
completion or event service.

### Condition evaluation and saturated command time

`0x4382d0..0x438453` chooses a sampled value from condition low nibble. The
nine-entry table at `0x438454` was read as inert dwords, separately from code.
Values 0/1/2 select movement heading/pitch/body bank; 3 uses speed with threshold
shifted eight; 4 uses altitude crossing; 5/6 call the point/path helper; 7/8 use
**unsigned word clock 0x5528c8**. Additional type/avoidance/crossing effects remain
unsupported; the selected timer branch requires none of those calls.

Thresholds are signed words widened to dwords. The comparison bits are 0x10
for sampled >= threshold and 0x20 for sampled <= threshold. Return true when
all requested bits from condition byte &0x30 are present. No requested bits
therefore returns true, including the initial byte 7/threshold 0. Equality can
satisfy both bits. This is a predicate contract, not approval to execute other
command branches or imported scripts.

`0x463b90..0x463bbb` widens the low words of delay and clock, adds them, and
returns **min(sum, 0x7fff)**. This differs from service/speech word wrapping;
clock >=0x8000 saturates even for delay zero. Diagnostic
`native_objects::command_deadline` preserves that rule without owning any clock
or scheduler state.

### Default event changes subsequent services

The current-object script gate `0x463d40..0x463d62` returns false for a null
script pointer +0x5e or word cursor +0x62 ==0xffff. The nonnull script body can
invoke embedded function pointers and is explicitly unsupported/inert. In the
selected initial null-script state, true command completion reaches the direct
0x80 event dispatch established in NE-00.1k, subject to the global interceptor.

STRIP's OBJEventProc (`0x473a40..0x473b36`) handles 0x80, 0x400, 0x800, 0x1000
and 0x2000 by constructing a replacement command through `0x463a20`:
flags 1, heading/pitch/bank modes 0, speed mode 1/value 0, first condition mode
8/comparison 0x10/value 60. Its final condition is mode 8/comparison 0x10/value
0x7fff. It returns true and sets instance event mask to **0xc000**. Thus initial
command completion changes command flags/deadlines and the event mask; it is
not an ignored event. The ground-following flag now affects later services.

The constructor (`0x463a20..0x463ae5`) first resets current commands when its
destination is the current command buffer, then zeros all 32 command bytes,
writes flags/modes/values and both conditions. Mode-specific value conversions
are source-established but not translated here. Its reset at `0x463e50` sets
script cursor and event mask to 0xffff, condition bytes to 7 and thresholds to
zero; kind 0 returns without the kind-2/4 continuation. The script pointer is
not cleared by this reset.

The condition writer `0x463af0..0x463b73` uses a separate five-dword table at
`0x463b74`. Mode 8 becomes mode 7: values other than dword 0x7fff shift left two,
then current-buffer destinations convert the low word through the saturated
deadline helper. Mode 7 also uses that helper for current-buffer destinations.
The above 60 becomes a **240-clock-unit delay**, not 60 seconds or a wrapping
word deadline. A noncurrent destination retains relative values. The comparison
byte is ORed into the mode; thresholds are stored as words.

OBJEventProc's 0x4000 branch can apply damage (`0x463ec0`), cleanup (`0x473c10`),
payload mutation and notification (`0x443d00`); those consumers remain open.
0x8000 returns false. Both return tails still write event mask 0xc000. Unknown
other event values reach the true tail; that does not authorize dropping their
upstream payload/state effects. Complete movement, command overrides, event
interceptor and damage/speech consumers remain prerequisites for live service.

## Selected stationary movement path — NE-00.1m

**Native source ledger; only the angle-approach helper is translated/tested.**
[Validation](../baselines/native-strip-movement.md). This closes the intermediate
movement path for the established initial/default commands with the selected
zero-speed STRIP inputs. It does not close the surrounding query, event, resource
loader, clock or scheduler producers, or accept arbitrary command overrides.

### Type inputs and selected dispatch

The OBJECT schema places `_turnRate`, `_bankRate`, `_minSpeed` and `maxAlt` at
+0x5d/+0x5f/+0x67/+0x79 respectively. The selected original STRIP.OT has zero in
all four; `maxAlt` is caret-marked zero. Its other movement scalars are also zero.
These are resource observations, not permission to default missing fields or
ignore scaling markers. The existing metadata reader preserves those tokens;
it does not yet produce a typed service configuration. Full resource load and
current-type ownership remain E003/E005 dependencies.

The reviewed kind-0 helpers return the signed turn/bank/minimum-speed words.
Common movement reads the maximum-altitude dword at 0x50d2e1. Type flags 0x208021
exclude the 0x40 bank-dependent rate, 0x1000 low-speed descent and 0x2000 pitch/
speed coupling branches. The local touching byte is false for kind 0 because
only kind 4 calls the prefix's touching predicate. Initial/default command flags
0/1 exclude the other command-rate, avoidance and offset overrides below.

The dispatch tables were inspected as inert data, separately from code:

| Table | Selected index and target |
| --- | --- |
| Heading dwords 0x438228 | 0 → 0x436eca, movement-heading hold |
| Pitch selector bytes 0x438274 and dwords 0x43825c | 0 → selector 0 → 0x4376d6; 11 → selector 4 → 0x4376f5 |
| Bank selector bytes 0x43829c and dwords 0x438280 | 0 → selector 0 → 0x437a84; 11 → selector 5 → 0x437c17 |
| Speed dwords 0x4382a8 | 1 → 0x437ef2, signed low word of command value |

### Intermediate update order

1. `0x4374ac..0x4376d5`: form wrapping target-minus-movement heading. The selected
   hold command has zero difference. Body-minus-movement heading offset, captured
   before the body, approaches its command offset (zero) using half the turn rate.
   Recompose body heading from the retained offset plus movement heading. Command
   flags 4/0x80 and heading mode 5 are not part of the selected commands.
2. Pitch mode 0 reads movement pitch at `0x4376d6`; mode 11 reads the earlier
   ground-angle output at `0x4376f5`. Clamp the target to signed ±0x3ffc. If
   current Y >= type maximum altitude and target pitch is positive, replace the
   target with zero. Command flag 2's avoidance call and kind-4 attachment branch
   are excluded here.
3. `0x4377af..0x437a83`: the minimum-speed helper is called even for kind 0,
   although the local-touching and excluded type predicates bypass its adjustments
   for this STRIP. Approach movement pitch toward the clamped target with the
   turn rate; approach body-minus-movement pitch offset toward its command offset
   (zero) with half that rate; recompose body pitch. Command flags 4/0x100/0x400,
   direct-rate mode 5 and aircraft-specific branches are excluded.
4. Bank mode 0 at `0x437a84` holds body bank; mode 11 at `0x437c17` selects the
   earlier ground bank. `0x437c26..0x437da9` obtains the bank rate, approaches the
   bank target, then bypasses type-0x2000 and kind-4 coupling. No autonomous bank
   command branches are accepted from the surrounding routine.
5. `0x437ecb..0x437efe` selects speed mode 1's signed word, which is zero for both
   established commands. The existing finish slice sees current speed equal to
   target zero, bypassing acceleration/deceleration calls and position integration.
   It still applies command flag 1's Y assignment and evaluates completion.

### Zero rate does not mean an omitted service

`0x411950..0x41199a` takes word current/target angles and a dword step. It forms a
wrapping **word** difference, widens its absolute magnitude through `0x4c6614`,
and snaps to target only when that magnitude <= signed normalized step. Otherwise
it adds/subtracts the low word of the step according to the difference sign.
Step normalization is wrapping dword negation; INT_MIN remains negative. A
half-turn difference 0x8000 has magnitude 32768 and follows the negative branch
when the step is smaller. This is not a float-degree shortest-path approximation.
`native_objects::approach_angle` preserves these boundaries diagnostically.

Consequently, with the selected zero rate fields, initial or replacement commands
do not change movement/body angles merely because the ground-angle target differs.
No angle delta is produced from a zero rate. Replacement flag 1 nevertheless
assigns Y to the earlier ground sample, even at zero speed; initial flags 0 do
not perform that assignment. Both still make the initial ground query and reach
the established completion/event path. These are conditional source deductions,
not a runtime result or permission to use a flat height/skip service callbacks.
Nonzero speed/rates, other commands, mutated type state, and full service entry/
exit ownership require their own accepted contracts. Live contact stays gated.

## Queue routing and speech observation — NE-00.1n

**Native source ledger only.** [Validation](../baselines/native-strip-observer.md).
This extends E020/E021 and adds E022 for owned speech output/resource requests.
It does not execute event callbacks, imported mission code, audio or networking.

### Reset and enqueue routing

`0x418070..0x418097` sets deadline +6 to 0xffff in each of the 120 stride-0xd5
queue slots, then zeros word 0x522d38 and byte 0x522c58. It does not clear other
record bytes or shared returned-record scratch. `0x48d2b0..0x48d2e2` separately
zeros eight speech words: 0x552fdc, 0x55304c, 0x552fd8, 0x552fe4, 0x552fd4,
0x4fef0c, 0x4fef08 and 0x5530cc. This is not the two string-start reset at
0x48d410 and not the sample-handle reset at 0x48d600.

The now contiguous enqueue slices `0x4180a0..0x41859e` establish caller ordering;
recipient expansion helpers and remote transport are still unaccepted consumers:

- The prefix ORs input flags with global byte 0x522c58. If preference dword
  0x4eb6f8 lacks 0x100000 and flags lack 2, subtypes 0x11–0x15, 0x17, 0x1c–0x1e,
  0x22 and 0x24 set suppression flag 1. The established bound-100 draw follows,
  including when suppression applies.
- Only mode word 0x520a50 ==0x10 expands recipients 0x8000/1/2 through group
  helpers 0x45e710/0x45e630/0x45e6e0. An ordinary recipient is appended directly.
  The resulting temporary ID list is consumed **backwards**, not forward.
- IDs 0x8003–0x800b take the special-recipient branch; a special ID minus local
  controller dword 0x4eb608 equal to 0x8003 selects local insertion. Ordinary
  objects select local insertion when controller byte +0x10 &0x7f equals that
  dword; otherwise a temporary record is constructed for transport 0x470640.
- After record construction, qualifying player-sender speech is observed at
  most once per enqueue invocation, before remote/local finalization. The prefix
  qualification requires mode 0x10, nonzero live player ID, true 0x4747c0 and
  kind 4. Observer flag 1 suppresses it. These producers remain explicit inputs,
  not assumed properties of either supported aircraft.
- A nonspecial, nonzero remote sender additionally receives one forwarded record
  through 0x470640: temporarily set recipient to sender and OR flag 4, then
  restore recipient/flags. Recipient transport follows. Neither transport nor
  observer is equivalent to appending one inert event.
- The terminal fallback can construct a self-addressed player-sender observation
  when none was observed inside the loop, **including when local capacity skipped
  all recipients**. It reuses the original draw; it does not draw again. Its
  payload copy is not independently clamped there. Host validation must bound
  the input before any queue/RNG/observer mutation; the ordinary-record clamp
  alone is insufficient evidence of safety.

### Sender-scoped observer and default callback

`0x48d350..0x48d3b2` uses event **sender +2**, not recipient +4. If sender is
nonzero, outside 0x8003–0x800b and differs from current ID, push/switch current
object before dispatch. Ordinary senders dispatch callback request 6 with the
record pointer; zero/special senders call 0x48d3c0 directly. Pop only if switched.
Thus the earlier current-object store/restore contract also applies to queue
observation. STRIP request 6 resolves the same default callback; other selectors
and instance overrides remain separate dependencies.

`0x48d3c0..0x48d401` first clears both speech-buffer starts. For subtype 0x24,
payload +0xd contains two consecutive NUL-terminated strings; the second starts
just after the first terminator. There is no payload-length check in this bounded
callback. It passes the two pointers to `0x48d420`, then always calls 0x48d470.
Host code must bound both terminators within the accepted payload and destination
capacities before staging. No permissive unbounded C-string behavior is imported.

`0x48d420..0x48d46b` appends a nonnull first string to buffer 0x552ff0. A nonnull
second string is appended to 0x553050, with a comma separator if that destination
was already nonempty. The pointer test is distinct from testing string contents.
The native append calls have no capacity argument in this wrapper.

### Output timing, resources and commit boundary

`0x48d470..0x48d5db` first invokes 0x490480 for ordinary senders whose nationality
low seven bits are 20 or 21. That formatter remains unresolved. A nonempty first
buffer goes through sender/player naming and 0x405f50; those display/name
consumers remain unresolved. A nonempty second buffer resets the sample handle
and calls 0x48d610. Regardless of empty buffers, the tail writes global speech
deadline 0x552fdc = word(clock 0x5528e0 + scaled_delay(3)). This differs from the
submission wrapper's empty-buffers early return in NE-00.1j.

`0x48d600..0x48d609` sets handle word 0x4ff058 to 0xffff. The sequence routine
`0x48d610..0x48d6d7` temporarily replaces the next comma with NUL, copies that
sample token, appends `.5K` only if no dot occurs anywhere in the token, and
calls 0x433680. It restores the comma and continues in order. Each call receives
the prior handle and gain `unsigned_word(0x5718ec) * 255 / 100`; returned AX
replaces the handle unless it is 0xffff. Do not infer a universal 0–100 source
range or reuse authored mixer gain as this contract. The sample loader/playback
routine, string bounds and exact referenced sample closure remain open.

E022 requires separating authoritative queue/current-object/buffer/deadline
changes from externally committed display/audio requests. Preserve output order
and handle-return dependencies; irreversible output cannot be rolled back by
restoring a queue snapshot. Full output acceptance belongs to NE-07; its owned
interface is needed before effects are dispatched by staged world service.

### Mission interceptor selection

`0x481f30..0x481f84` matches the mission keyword `code`, reads a name into
0x4fb238 and applies the `.MC` extension through 0x4a6870. The activation gate
`0x480aa0..0x480ac6` calls resource accessor 0x4a6ae0(name, 0x8000) and stores its
result in global interceptor 0x4f6fb8 only when the name begins nonzero and local
controller dword 0x4eb608 is zero. This is separate from the scheduler reset
that clears the pointer. A nonempty code resource stays unsupported/inert;
absence must be established from the chosen world input and reset lifecycle,
not inferred from STRIP identity or the no-AI scope. This does not establish a
full MM parser or complete mission lifecycle.

## Collision hit dispatch and death marking — NE-00.1o

**Native source ledger and diagnostic collision predicate.**
[Validation](../baselines/native-strip-hit.md). This refines E019/E021 and the
NE-07 boundary; it does not connect contact damage or object removal.

### The other object's callback operates on the current victim

`0x463ec0..0x463f23` reads payload word +1. A nonzero ID resolves that object's
request-4 callback through 0x463f30; zero resolves the type resource named at
payload +3 and asks its selector for request 4. It invokes the resulting callback
with the payload if nonnull. **It does not switch the current victim.** Instance
overrides and unresolved type resources remain explicit boundaries; imported
selectors are inert in the host.

After the callback, current victim flag 1, current health word +0xe ==0 and victim
type flag 0x400 together call full removal 0x4627b0. The selected STRIP type flags
0x208021 exclude that last predicate. This does not prove removal is unnecessary
for all objects or all later lifecycle paths.

The plane selector's delegation through `0x473db0..0x473dd7` overrides requests
3/5 and otherwise delegates to `0x473be0..0x473c09`. That base selector maps
request 4 to `0x473b40`; request 3 to OBJEventProc; request 6 to 0x48e8d0; other
requests return null. STRIP has its own selector and returns null for request 4.
Do not confuse the other object's selector with the current victim's event handler.

### Generic collision threshold

`0x473b40..0x473bdc` first suppresses the hit when current victim controller bit
0x80 and global preference bit 1 are both set. Otherwise it resolves the other
object/type exactly as above, reads **signed type hitPoints word +0x49**, and
computes `other_type_hp * 100 / victim_type_hp` with signed division toward zero.
Victim type is the current type at 0x50d268. A result <25 exits; a result >=25
sets current health word +0xe to zero, after two 0x486580 notification calls when
the other ID is nonzero. It does not subtract payload byte +0 or current health
from this ratio. Those notification consumers remain unresolved.

`native_objects::collision_is_lethal` translates only the ratio predicate with
explicit signed words. A zero denominator returns `None` instead of executing
the native divide fault; future staging must reject it before effects. Negative
inputs retain source signed arithmetic for diagnostics, not new runtime eligibility.
Protection, lookup, callback choice, notification and health writes are not part
of the helper. The aircraft's own event handler is separately selected, so this
contract alone establishes neither aircraft crash damage nor runway immunity.

### Death marking is distinct from removal

The already reviewed OBJEventProc 0x4000 branch skips a victim whose current health
is already zero, dispatches the hit, and checks health again. If it became zero,
it calls `0x473c10..0x473d99`. Earlier descriptions calling this routine “cleanup”
were incomplete: it **marks death state**, without removing this STRIP's airport,
collision candidates, scheduling entry or allocation in the selected kind-0 path.

For a locally controlled victim lacking instance flag 0x2000, it first calls
0x486580 and 0x485820 using current ID and the separate instance +0x76 field;
negative +0x76 indexes the native word table near 0x4eb6d8. That field's producer
and notification consumers remain open. A player-ID match lacking flag 0x2000
also performs player-specific notification and a shared bound-4 draw for output;
this branch is not implied for the selected nonplayer STRIP. The embedded
0x473d9c table is data, excluded from the reviewed code range.

The common tail sets current health to zero, ORs instance flag 0x2000, and ORs
0x100000 unless the type's first byte is 5. Kind 0 then returns, bypassing the
kind-2/4 equipment loop. It does **not** clear live/scheduled flags 1/2 here.
Do not replace that state with immediate generic deletion or ordinary candidate
unregister. Full removal and later dead-object service ownership remain open.

Back in OBJEventProc, payload +0x20 becomes the unsigned maximum of its old byte
and victim type expType +0x55; payload XYZ at +0x14 receives current position.
Then 0x443d00 receives victim type craterSize +0x56. Its reviewed zero-size entry
`0x443d00..0x443d20` returns zero before any query, RNG draw or allocation. The
selected STRIP resource's craterSize is zero, excluding that effect under the
reviewed unchanged type input. Nonzero crater creation remains unsupported.
Event mask still becomes 0xc000 at the established return tail.

## Clock and scheduler ownership — NE-00.1p

**Native source ledger; diagnostic arithmetic only.**
[Validation](../baselines/native-strip-clock.md). Existing `object_service_age`,
`frame_clock` and `counter_clock` reviewed regions already establish the central
arithmetic; this continuation reuses them and adds the complete scheduler caller,
merge, frame baseline setter, scale setter and peer-pause predicate. It does not
replace the authored 120 Hz bridge or activate object services.

### Load, service, requeue and merge

The tail `0x462930..0x46295e` belongs to current-object load at 0x4628b0, not an
independent callable entry. After copying instance/type scratch it writes signed
word 0x546ba0 = `max(2, signed_word(low_word(0x552928) - instance[+0x66]))`.
The subtraction wraps before its signed comparison. A difference of 0x8000
therefore becomes two, not 32768. Kind 4 then refreshes aircraft fields through
0x452140; selected kind 0 bypasses that call. Existing `clock_rng::service_ticks`
implements this arithmetic; new tests cover signed half-range and dword/word wrap.
The jump table at 0x462960 remains inert data outside that reviewed tail.

The complete caller `0x462a50..0x462b63`:

1. Clears secondary scheduler head 0x546ba8, then loads primary head 0x546b90.
   Loading precedes testing whether unsigned instance deadline +0x68 is greater
   than word 0x5528c8. A future head ends traversal with that object's scratch
   still loaded; it is not an effect-free peek.
2. For a due head, replaces the primary head with +0x64 and clears instance
   scheduling bit 2. Calls 0x462e70 only when live bit 1 is set.
3. Writes last-service +0x66 from the current low word of 0x552928 after the
   callback (also for an inactive object). For a still-live object, health zero
   plus type flag 0x400 calls removal 0x4627b0; STRIP's 0x208021 excludes this
   automatic-removal predicate, not every possible later removal.
4. If still live, removes any scheduling entry and inserts into the secondary
   list through 0x4626d0, retaining the callback's deadline. That insertion also
   stamps +0x66. Stores current scratch through 0x462980, then repeats.
5. Merges the secondary list with primary through 0x462b70, clears secondary
   head, calls 0x462c91 then 0x462d40, and conditionally invokes nonnull mission
   interceptor 0x4f6fb8. These trailing consumers remain required open edges;
   mission code stays unsupported/inert.

`0x462b70..0x462c90` merges by zero-extended deadline words. It advances past a
primary node only when its deadline is **strictly less** than the secondary
node's; equal deadlines put secondary nodes first. Within either list existing
order is retained. This differs from inserting a new node after existing equal
deadlines in 0x4626d0. It directly changes stored +0x64 links and the primary
head without loading each node into current scratch. IDs, cycles and stale
references need host bounds/validation before staged mutation. The native merge
has no such validation. Neither merge nor the scheduler is translated here.

A separate exploratory writer at 0x442f1d copies frame delta 0x55292c into the
same service-time global 0x546ba0. Its surrounding effect-service body remains
unaccepted. Do not give each object an independent global-time replacement or
claim a complete writer census from the linear reference index.

### Frame clock, scale and different word domains

Existing reviewed TIMEUpdate `0x486aa0..0x486be9` repeatedly refreshes the platform
counter until signed `saved_counter + 4 < current_counter` (dword arithmetic).
It narrows the counter difference to a word. Word 0x5528e8 receives that raw
signed difference clamped to 5..128. Simulation delta 0x55292c instead starts
from the **unclamped raw word**:

- A true 0x46ff70 result or scale word 0x5528f8 ==0x7fff sets simulation delta
  zero. The unscaled delta and saved counter still update.
- Positive scale shifts AX left; negative scale negates CL and shifts AX
  arithmetically right. Both use the x86 five-bit count, even for word operands.
  Counts 16..31 zero a left shift and sign-fill a right shift; count 32 is zero.
- Preference bit 0x400000 applies signed `scaled * 4 / 3`, toward zero,
  **narrows to a word**, then clamps signed to 5..128. Narrowing can reverse the
  sign before clamping; clamping a widened ratio would differ.

`clock_rng::frame_ticks` now accepts the full signed scale-word domain with
explicit five-bit shifts. Its previous +/-15 rejection was a diagnostic support
limit, not native validation. The existing Result API is retained. Tests exercise
pause, raw-before-clamp order, shift counts, negative scale and ratio narrowing;
no live clock or platform timer is changed.

The routine adds the sign-extended final simulation delta to dword 0x552928 with
wrapping arithmetic. It writes word 0x5528e0 = arithmetic shift-right by 8 and
word 0x5528c8 = arithmetic shift-right by 6. Initialization used signed division
by 64 for the latter, so negative values can differ. Before writing those new
words, it computes 0x552934 from base 0x5528e4 plus the **previous unsigned** word
0x5528e0; a signed result >=86400 is reduced by unsigned division/remainder.
This ordering retains the previous second-word sample. Finally it saves the
sampled counter into 0x552940. `0x486a90..0x486a9a` independently sets that saved
counter to 0x5528ec. Full clock-state translation remains open.

Scale setter `0x486c60..0x486c7b` forces scale zero when signed global 0x4eb604 >1,
except the 0x7fff sentinel; otherwise it stores CX unchanged. Other direct scale
writers exist in UI paths and are not accepted by this setter alone.
`0x46ff70..0x46ffbb` immediately returns false when 0x4eb604 ==1. Otherwise it
visits indices 0..count-1 in order, obtains 0x494cb0's record, and returns true on
an enabled bit in its dword +0x158 together with nonzero byte 0x547328[index].
Record/list producers and multiplayer paths remain unsupported; the single-count
exclusion is source-backed, not an assumption from the no-AI scope.

Clock/scheduler state, object scratch, links, callback effects and RNG must share
the future transaction. E015/E019/E021 remain open for trailing special services,
notifications, dead-object behavior and event/output ownership before E001/E002.

## Trailing events and death accounting — NE-00.1q

**Native source ledger only; no new runtime behavior.**
[Validation](../baselines/native-strip-accounting.md). This closes bounded
E019/E021 caller and notification gates, not effect creation or full death parity.

### Events after the scheduler merge

`0x462c91..0x462d38` repeatedly calls consuming event lookup with selector 0x800b,
mask 0xffff. In the previously reviewed lookup, that selector also matches an
ordinary recipient **without scheduled flag 2**, excluding the special range
0x8003..0x800b unless exactly equal. Thus inactive/unscheduled objects do not
necessarily stop receiving events when absent from the main service list.
Recipient validity must be checked by the host; the caller loads the returned
recipient into current scratch without a null guard or a push/pop pair.

Only event mask exactly 0x4000 takes the hit branch: pass payload to 0x463980,
then examine the stored recipient's live flag and 0x4747c0 predicate. A dead or
predicate-false stored recipient selects 1, otherwise 0, for the subsequent
0x4432d0 argument. That effect call uses local controller, payload byte +0x20,
payload XYZ +0x14, recipient ID, zero, the selected value and final 1. It precedes
0x462980 storing scratch. Consequently inspecting the stored object after a
scratch callback is a real ownership distinction; do not substitute scratch
flags without establishing intervening stores. Other returned masks skip that
hit/effect branch but still store scratch. The next lookup can run observers.
After the first loop returns null, a second loop consumes selector 0x800c with
mask 0xffff and ignores returned payloads. A null return may itself follow
consumption/observation of flag-4 events, as established in NE-00.1k.

`0x462d40..0x462e21` then consumes mask 0xffff for word recipient
`local_controller - 0x7ffd` (equivalently +0x8003, wrapping). It dispatches by
subtype byte +0xa, not mask. The six dword targets at 0x462e24 and 37 selector
bytes at 0x462e3c are inert data outside the code slice:

| Subtype | Branch | Accepted caller behavior; downstream remains open |
| --- | --- | --- |
| 0 | 0x462d77 | Payload flag +0x10 can replace payload XYZ +1 from object ID +0xd's stored XYZ; calls GRAPHICAddExp 0x4432d0 using payload type +0, XYZ +1, ID +0xd and byte +0xf |
| 1 | 0x462dcb | Calls GRAPHICAddSmoke 0x443e80 with payload position, byte +0xe and signed words +0xc/+0xf/+0x11 |
| 3 | 0x462dfa | Calls 0x47ceb0 with payload base, byte +0xd, vectors +0xe/+0x1a and signed word +0x26 |
| 2, 4..0x23, 0x24; values >0x24 | Loop | No subtype body; the consuming lookup and its observer have already run |

In particular subtype 0x24 speech has no additional body here; that does **not**
make the event a no-op because lookup can invoke the speech observer before
returning it. Pointer/ID fields, string payloads, effect lifetimes and resources
still require bounds and staged ownership. No visual/sound effect is activated.

### Score gate and separate death counters

`0x486580..0x4865b7` returns -1 immediately for global 0x4eb604 ==1. Otherwise
it also returns -1 when current kind is not 4 and controller bit 0x80 is clear.
The selected ordinary STRIP meets the second exclusion independently of the
first. Thus its generic-hit/death calls do not enter this scoring body under
those reviewed inputs. Other kinds/controllers retain the unsupported body at
0x4865b8; do not generalize this exclusion to aircraft or change the controller.

The distinct `0x485820..0x485a38` receives controller, credited-ID word and
victim-ID word. It exits when global player ID 0x520a1c or credited ID is zero.
For the local controller it calls 0x471400(credited ID, victim ID) **before**
checking whether the credited ID equals the player (counter slot 0) or second
tracked ID 0x520a14 (slot 1). Other IDs exit only after that possible call.
0x471400 remains an open consumer, not a UI-only notification assumption.

For a tracked credited ID it resolves both objects. Equal nationality high bit,
victim flag 0x80 clear and byte 0x54bdb0 !=2 take the counter at 0x54de38, except
that bytes 0x529200 !=0 and 0x5291e0 ==1 together with signed global count >1
require victim controller bit 0x80; otherwise return without increment.
All remaining cases choose the first matching category below, adding one to a
dword at `base + 4*slot` with native wrapping arithmetic:

| First matching victim predicate | Counter base / additional write |
| --- | --- |
| Kind 4 and type dword +0xba bit 8 | 0x54ddf8 |
| Type class word +0xd bit 0x8000 | 0x54dde8; also 0x54de40 unless instance flag 0x8000 |
| Class bit 0x4000 | 0x54ddf0 |
| Class bit 0x2000 | 0x54de00; also 0x54de48 if signed type HP +0x49 >=1000 |
| Class bit 0x1000 / 0x800 / 0x400 / 0x200 / 0x100 / 0x40 | 0x54de08 / 0x54de10 / 0x54de18 / 0x54de20 / 0x54de28 / 0x54de30 |
| No match | No counter write |

STRIP's class 0x0100 selects 0x54de28 only after the preceding gates. Counter
reset, credited-ID lifetime and 0x471400 effects remain open; neither speculative
score names nor an empty credited ID may replace these dependencies.

### Attribution prefix and remaining lifetime

A bounded prefix of PROJDamageProc, `0x4c1870..0x4c18fb`, establishes one writer
of current instance +0x76. Nonzero payload byte +0 is required. Nonzero payload
object ID +1 resolves that object and its +0xe2 word owner ID; zero object ID
loads the named type at payload +3 but supplies owner zero. A nonzero owner
different from current ID writes +0x76: if the owner resolves and its controller
has bit 0x80, write `(controller & 0x7f) - 100` as a word; otherwise write owner
ID. Zero or self owner leaves previous attribution unchanged. That owner field's
producer and the rest of projectile damage are still outside this slice.

The established death caller reads signed +0x76; negative values index the
word table near 0x4eb6d8, nonnegative values become IDs. Do not reinterpret this
as the placement alias +0x74, clear it on every hit, or assume the newly found
prefix is the only writer. Existing fitted combat remains separate. Future
transactional state must own counters/attribution and any notification effects
alongside object scratch, queue and RNG. Full removal, APComment middle,
output/effect resource closure and initial world ownership remain gated.

## Removal caller and notification exclusions — NE-00.1r

**Native bounded source ledger only.**
[Validation](../baselines/native-strip-removal.md). A reviewed removal caller is
not proof that every downstream resource/list is released or that the host can
roll back by calling native-style deletion.

### Ordered logical removal

`0x4627b0..0x4628a9` performs this order on current scratch:

1. Remove scheduling through 0x462620; clear instance live/scheduled bits 1/2.
   Nonnull instance +0x5e calls 0x436040, then becomes zero. Imported script
   contents remain unsupported; the deallocator contract is not accepted here.
2. Repeatedly consume due events for the current ID with mask 0xffff until lookup
   returns null. Observer effects can run during this loop. This is not an
   unconditional purge: future events are handled later.
3. Release airport slots through 0x4bd490; call 0x44baa0; call 0x442da0 with
   controller low seven bits; then call 0x4c3ca0, 0x45e520, 0x45f250, 0x469960,
   0x43e780(current ID), 0x438520(0), in that exact order. These unreviewed
   callees remain open ownership edges, not inferred harmless notifications.
4. Unregister collision candidates through 0x42e5c0. Then invalidate remaining
   queued recipient records through 0x4189e0, then notify through 0x46fdb0.
5. Call 0x412030 on current type pointer +5 with argument 2 and compare its
   returned string through 0x4d9660 against 0x4f6ff0, then 0x4f6fe8. Either zero
   comparison sets byte 0x552810 to 1. The string/classification producer remains
   open; no filename meaning is invented from those addresses.
6. Call death marking 0x473c10 after the preceding cleanup; kind 4 tail-calls
   0x4a04f0, other kinds return. Selected kind 0 excludes that final tail-call.

No direct final scratch store, allocation release or APDelete call occurs in this
bounded caller. That observation does not prove absence inside unresolved
callees. The selected scheduler caller stores scratch afterward; other invocation
contexts must establish their own store ordering. Logical removal remains
separate from the failed-construction last-allocation release at 0x491490.

### Future-event invalidation and retained records

`0x4189e0..0x418a0c` scans all 120 event slots in order, stride 0xd5, starting
0x522d40. For each non-0xffff deadline whose **recipient** +4 equals current ID,
it writes deadline +6 =0xffff. It does not compare current time/mask, compact,
clear payload/sender, run an observer or consume RNG. This second phase removes
future recipient events left by the earlier consuming loop. Sender references
in other recipients' events are not invalidated by this routine.

`0x44baa0..0x44bae0` searches the first matching current-ID word in 175 ten-byte
records starting 0x5445a0. It writes record +6 = word(0x5528e0 +5); if current
ID equals player ID 0x520a1c it instead writes 0x7fff. It returns after the first
match and leaves other record bytes and later duplicates intact. This is expiry
marking, not record compaction/freeing. The pool's producer and expiry consumer
remain open; its count is established by the end pointer 0x544c76.

### Single-count notification gates

Death notification `0x471400..0x47144a` immediately exits when global count
0x4eb604 ==1. Otherwise it constructs a five-byte message: literal 0x2d and two
words obtained by calling 0x4914c0 with each supplied ID and argument 1, then
calls 0x46c0a0 with -1, message pointer, length 5 and argument 1. This closes the
single-count exclusion of the 0x485820 downstream call; ID mapping/transport
remain unsupported outside that gate.

Removal notification `0x46fdb0..0x46fdf2` also exits for count==1, or when current
controller low seven bits differs from local controller 0x4eb608. Otherwise it
sends literal 0x13 plus instance **alias +0x74**, length 3, through 0x46c0a0 with
the same -1/1 arguments. The alias is not allocated ID or damage attribution.
Neither wrapper executes imported code in the host. A selected single-count
world can exclude these calls' effects only after its count producer is accepted.

E019/E021 now have a full bounded removal-call order and separate due-event versus
future-event handling. Remaining callees, pool/resource lifetime, global setup,
APComment, effects and world assembly still gate staged queries/runtime support.
