# Ocean and whitecap source contract

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Static research, 2026-09-16. Reviewed FA.EXE SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`,
FA.SMS SHA-256
`e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`.
No original code was executed. [Weather](weather.md) supplies the deck and
256-unit clock contracts; [behavior](../spec/ocean.md) separates the native
facts below from GPU adaptations and the requested new ripple effect.

## Assets and frame selection

FA_1.LIB contains OCEAN0 through OCEAN6.PIC plus WAVE01.PIC and WAVE02.PIC.
FA_2.LIB contains WAVE1.SH and WAVE2.SH. The wave pictures are 256-square raw
indexed atlases, without an embedded palette. Index 255 supplies cutout coverage.
Each atlas has sixteen 64-square cells, with UV coordinates inset two pixels.

| Shape | SHA-256 | Texture |
| --- | --- | --- |
| WAVE1.SH | `a7c5800a9383cead2ebeffdccd2e7967868e3a1102dd9a1e64b866559b96d83b` | WAVE01.PIC |
| WAVE2.SH | `348d8701f058b97991874aeb0c4efb0282a56f640aff1eeb27e2b5f3ce8581a6` | WAVE02.PIC |

Both modules have 932-byte CODE sections. Four `0x7a` vertices at CODE+0x24,
0x2e, 0x38 and 0x42 describe a flat square with coordinates -400/+400 and
scale exponent 8: an 800-foot sheet. CODE+0x50 names the texture. The embedded
branch at +0x68..+0x95 combines imported position values, shifts by 17 and
masks to 15, then adds `_currentTicks >> 6` and masks to 15 again. The time
component therefore advances four frames per second and repeats in four seconds.
Imported position symbols include viewer_x, viewer_z, xv32 and zv32. The exact
view/local-position conversion and native spatial phase remain unverified.

The sixteen selected `0xe4` UV records traverse columns left to right and rows
bottom to top: frame n has origin `(64*(n%4)+2, 64*(3-n/4)+2)`, spanning 60 pixels
in each axis. The subsequent polygon references the four sheet vertices.
The shape disables ordinary fog with `0xca 0` before drawing, restoring it later.
The generic static shape reader cannot render this program's embedded x86
branches. The removed trial implemented the documented effect directly; the
reader was never broadened to run imported code.

## Weather and repeated placement

The parsed LAY record's shape field at +0x153 names WAVE1.SH in the reviewed
modules. `_T_InitWaterProc` at 0x4a8a70 copies the active name from 0x583a93 into
0x580bd0 and initializes descriptors at 0x50c180 when nonempty. `T_WaterProc`
at 0x4a8ab0 dispatches them through the same repeat machinery as cloud patches.

Nine 26-byte descriptors alternate mask 1/2, ending with mask 1. They point to
the dynamic name and have zero height/yaw. Their X/Z coordinates in feet are
(1536,5632), (2048,1536), (2560,3584), (3072,0), (4096,5120), (4608,2560),
(6144,1024), (7168,6656), (7680,3072). A zero descriptor terminates the list.
The repeat interval is 0x200000 in 24.8 feet, or 8,192 feet. The dispatcher
branches at 1,000 and 2,000 feet; the upper branch restricts the mask to 2.
Detail-setting interaction, native culling and full surface/caller dispatch
remain research gaps. The removed GPU trial used the pattern on visible ocean
pixels only and did not establish exact placement/range parity.

## Runtime coverage

The whitecap trial was removed at John's request on 2026-09-16. These recovered
facts remain research evidence. Wave SH/PIC resources can be explicitly extracted
with `--include 'WAVE*'`; they are no longer selected by the shared theater
profile, required by app caches, uploaded or drawn. Previously imported caches
can retain those files without rendering them. No general SH animation support
was added.

Current ocean presentation is **user-directed opinionated**: short procedural
wave slopes, near pixelation and progressive filtering, retaining retail ocean
and sky textures and the weather palette. See [the current spec](../spec/ocean.md)
and [acceptance](../baselines/ocean.md). Native wind/sea-state coupling is
**unknown** and unchanged. Retail comparison remains unavailable.
