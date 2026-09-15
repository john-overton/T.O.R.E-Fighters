# Native STRIP initialization and shape metadata

2026-09-15, NE-00.1d / E003–E005 under the
[living plan](../native-environment-systems-plan.md). **Native static source**
from the exact [reviewed EXE/SMS](native-flight.md). Only the bounded box reader
and midpoint arithmetic are translated/tested. No runway is runtime-connected;
retail comparison is unavailable. [Validation](../baselines/native-strip.md).

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
Template defaults, reset/removal and all registration-failure cleanup remain
untranslated; a passing box diagnostic is not airport-manager acceptance.

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

Next: recover template defaults, type resolution, final creation/store and failed
registration cleanup; validate all placement fields and collision-channel consumers.
Resolve E004 drawing/texture/palette dependencies independently. Then implement
transactional world construction and ordered queries, including failures after
cache/RNG mutation. Neither aircraft's live contact stop changes in this slice;
carrier remains gated by NE-06 and contact events by NE-07.
