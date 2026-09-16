# Native STRIP initialization and shape metadata

2026-09-15, NE-00.1d/e/f / E003–E005 under the
[living plan](../native-environment-systems-plan.md). **Native static source**
from the exact [reviewed EXE/SMS](native-flight.md). The bounded box reader,
midpoint arithmetic, mission nationality conversion and candidate-list operations
are translated/tested.
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
not host function pointers. Their downstream behavior is outside this slice.
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
conversion translated/tested. Full token/placement loading remains unimplemented.
All inputs still need bounded parsing before use.

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
Conditional controller/multiplayer and other optional post-create effects are
unaccepted; selected construction must establish their predicates rather than
executing them implicitly.

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

The already sourced due-service call at 0x462abc enters 0x462e70. Exploratory
review of its final delay selection at 0x4630b0..0x4631a9 exposes a further
conditional RNG edge: a stopped ordinary object can use **2 + bound-20 draw**;
positive speed uses **2 + bound-8 draw**. Other actor/controller/visibility and
deadline gates can avoid those draws. This does **not** establish that every
STRIP service draws RNG. The complete predicates and callback effects remain
unreviewed; do not implement or activate autonomous branches. This unresolved
ownership edge must be reconciled with E002 before claiming native replay.
