# Native STRIP initialization and shape metadata

2026-09-15, NE-00.1d/e/f / E003–E005 under the
[living plan](../native-environment-systems-plan.md). **Native static source**
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
host failure contract is required by the living plan; it is not a claim that
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
registration function address is selected at 0x49fb2e; full selector/caller and
instance +0xe3/+0x231 producers remain to trace. No autonomous behavior or
aircraft service is translated by recording this list's storage contract.
