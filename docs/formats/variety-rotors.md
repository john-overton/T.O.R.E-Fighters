# Aircraft variety rotor geometry

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Reviewed on 2026-10-05 with the existing bounded SH decoder. User-owned
`FA_2.LIB` is local only. Build identities below identify each extracted SH
record, not a claim that its original executable interpolation is understood.
The implementation is the fitted [presentation spec](../spec/rotor-presentation.md).

| SH record | SHA-256 | Scale exponent |
| --- | --- | --- |
| `C130.SH` | `14081bbe20e1d683babfb349f84cc1bd808c654f164e52d0c490ba9fabc25b84` | 9 |
| `AC130.SH` | `70100c0c6795fcd65e0f49f60f6ccc9e0e2663f25284c80d64638ee77e59c30e` | 9 |
| `E2C.SH` | `b00bba4377071c6077f7c1f7d4397da7f19e9c4d248ded85c9fb2231863e39ca` | 9 |
| `V22.SH` | `bfd5e34dd536d1c35f04237c8da120082fa55fd46edbb188077db1a4c35cc725` | 8 |
| `APA.SH` | `c79be574a974d0904865cd7627b6e45d31e35e9e3ab4323d89768b4199db3b9d` | 8 |
| `HIND.SH` | `7535232388e18ee82a629dfc7081e591a8de9f65229136ef2349fb27ec39683e` | 8 |
| `CH47.SH` | `814abfa9bdac184769a344ac8ab1a5af682eabb73a9e3822eaeccd3734c8ae4c` | 8 |

## Selected groups

Addresses below are decoded face instruction offsets in each SH record. Ranges
name the reviewed groups, not arbitrary byte ranges copied from original code.
The implementation selects explicit face addresses, keeping intervening engine
housing, mast and fuselage geometry out of rotor rotation. All coordinates here
are unscaled source X/right, Y/forward, Z/up.

| Shape | Reviewed moving geometry | Source position evidence |
| --- | --- | --- |
| C130 | Propeller groups 0x1b44..0x1c26, 0x204e..0x2130, 0x2385..0x2467, 0x2645..0x2727, eight faces each | Four disks lie in Y=23 planes, about X=+27,-27,-53,+53 and Z=5.5..6 |
| AC130 | Individual blade faces in 0x2b10..0x2d66, 0x2e1d..0x3073, 0x3f7a..0x41d0, 0x4287..0x44dd | Four groups of sixteen blade faces lie in Y=14 planes, about X=-52,-26,+26,+52 and Z=2.5 |
| E2C | Propellers 0x46a9..0x475f and 0x48be..0x49d7, six faces each | Y=18 disks about X=-16,+17, Z=-1 |
| V22 | Left propeller six faces 0x3540,0x3567,0x358e,0x3675,0x369c,0x36c3; right six 0x2fc0,0x2fe7,0x300e,0x30f5,0x311c,0x3143 | Y=51 disks centered at X=-102,+102 and Z=16 |
| V22 | Left nacelle assembly explicit faces 0x33f5..0x377a; right 0x2e75..0x31fa | Outer engine housings occupy X=-110..-94 and +94..+110; propeller hubs extend forward to Y=62..65 |
| APA | Main disk groups 0x3553..0x35f5, 0x41e6..0x429b, 0x44c8..0x457d, eighteen faces total; tail disk 0x4320,0x4343,0x4366,0x4459,0x447c,0x449f | Main disk lies at Z=19 about X=0,Y=5; tail lies at X=-5 about Y=-93,Z=18 |
| HIND | Main disk 0x4d5f,0x4d8a,0x4db9,0x4de4,0x4f2e,0x4f59,0x4f88,0x4fb3; tail 0x2f65,0x2f8c,0x2fab,0x30c1,0x30e8,0x3107 | Main disk near X=0,Y=-3,Z=23; tail at X=-5,Y=-112,Z=27 |
| CH47 | Aft facing groups 0x2067..0x20e8 and 0x3e0c..0x3e8d; front facing groups 0x3ae5..0x3b66 and 0x3c63..0x3ce4, four coincident texture phases per panel | Aft panels occupy X=-68..68,Y=-113..23,Z=25..34. Original front panels occupy X=-68..68,Y=-2..135,Z=10..19. Distinct masts occupy Y=-45,Z=29 and Y=+67,Z=15 |

Shape geometry establishes visible moving parts and their planes. The original
rotor phase, throttle curve, direction, spool time, image-phase selection and
V-22 hinge interpolation remain unknown. Their shipped fitted choices are
specified in the [presentation contract](../spec/rotor-presentation.md).

Rotor atlas review found different UV rectangles on coincident panels, rather
than extra physical blades: V22 and APA carry three image phases, E2C three, HIND main
panels two (tail three), C130 and CH47 four, and AC130 blade polygons two. Original texture index 255 remains
transparent. The images contain the original grey blade blur over that cutout;
overlaying all phases makes a dense noisy white disc. The host selects one
image phase as a documented fitted choice in the linked presentation contract.


The HIND main panels span Y=-91..85 with Z=25..21. Their source plane normal
is proportional to [0,1,44]. CH47 aft panels span Y=-113..23,Z=34..25, giving
normal [0,9,136]; front panels span Y=-2..135,Z=19..10, giving [0,9,137].
These tilted planes are source geometry, not a demand to spin about vertical.
Using their own normals keeps each blade image in its plane throughout spin.
Cyclic limits and the shared mast pivot choices remain fitted in the
[presentation contract](../spec/rotor-presentation.md#helicopter-cyclic-presentation).
