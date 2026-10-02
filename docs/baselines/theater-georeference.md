# Theater georeference calibration

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research and implementation validation, 2026-10-02. Public airport coordinates
were compared with named runway placements from local user-owned FA layouts.
No retail executable or Tacview application was run for this calibration.

## Evidence and method

The public-domain [OurAirports data](https://ourairports.com/data/) download
was retrieved from
[airports.csv](https://davidmegginson.github.io/ourairports-data/airports.csv).
Its SHA-256 is 197c68d0520b01c35f03c7fa15bf3467bf8ed788b3ed5a128e86eccebf4465ef. Reference identities are explicit in
[the calibration manifest](../../tools/data/theater-georeference.json); names
are never matched automatically by approximate spelling or nearest position.
Source layout provenance is in the [airport baseline](ukraine-airports.md).
Retail layouts and downloaded data remain local and are not committed.

The [behavior spec](../spec/tacview-geography.md) defines the north-up linear fit.
The initial exporter used guessed centers and one physical feet-to-degrees
conversion, which could neither locate the game maps nor account for their
compressed geography. The fitted spacing is allowed to differ by theater and
axis. These are agent-selected approximations.

## Measured fit

RMS is the root mean square discrepancy at the fitted reference airports; maximum
is the largest discrepancy there. Neither bounds errors at unmeasured landmarks.
A two-reference fit has zero residual by construction, not verified accuracy.

| Theater | References | Center latitude | Center longitude | RMS km | Maximum km |
| --- | ---: | ---: | ---: | ---: | ---: |
| APA | 5 | 8.700386 | -80.220402 | 16.7 | 20.9 |
| BAL | 5 | 56.651141 | 26.254748 | 20.0 | 28.1 |
| CUB | 4 | 22.374058 | -82.489995 | 14.9 | 22.8 |
| EGY | 8 | 30.637461 | 33.290147 | 13.4 | 24.4 |
| FRA | 12 | 49.173654 | 2.888274 | 67.4 | 163.0 |
| GRE | 5 | 37.679991 | 24.767676 | 18.4 | 29.1 |
| IRA | 4 | 29.240607 | 47.371193 | 27.1 | 34.2 |
| KURILE | 2 | 47.414219 | 151.544076 | 0.0 | 0.0 |
| LFA | 2 | -51.824439 | -59.692008 | 0.0 | 0.0 |
| NSK | 4 | 38.087466 | 127.084886 | 17.6 | 26.3 |
| PGU | 4 | 26.434667 | 55.928480 | 7.8 | 10.6 |
| SPA | 8 | 27.135967 | 69.982516 | 84.5 | 160.8 |
| TVIET | 4 | 20.287869 | 106.548716 | 23.4 | 32.9 |
| UKR | 7 | 46.627963 | 31.470371 | 27.3 | 32.9 |
| VLA | 6 | 42.106862 | 132.094746 | 44.3 | 71.0 |
| WTA | 5 | 23.992343 | 119.768321 | 18.7 | 25.3 |

The Kuril and Falkland fits have only two usable named references. Their geographic
rotation and independent landmark accuracy are unknown. France and Pakistan have
large remaining errors: a single center and spacing cannot reconcile all their
authored placements. Do not treat these coordinates as surveyed airport positions.

Egypt's Luxor and Vladivostok's Jiamusu were excluded from calibration because
their apparent real-world identities conflict substantially with the placement
pattern. Luxor is north of Ras Nasrani in the game but south of it in real
geography. The exact intended sites are unknown. They must be researched before
being admitted as calibration references. No game placements were relocated.

## Reference identities

Airport IDs link to the public coordinate source. Historical names may differ
from today's airport names; uncertain matches were omitted.

- APA: Chitre = [MPCE](https://ourairports.com/airports/MPCE/), Tocumen = [MPTO](https://ourairports.com/airports/MPTO/), Rio Hato = [MPSM](https://ourairports.com/airports/MPSM/), Punta Cocos = [MP26](https://ourairports.com/airports/MP26/), Penonome = [MP18](https://ourairports.com/airports/MP18/).
- BAL: Liepaja = [EVLA](https://ourairports.com/airports/EVLA/), Vilnius = [EYVI](https://ourairports.com/airports/EYVI/), Parnu = [EEPU](https://ourairports.com/airports/EEPU/), Pskov = [ULOO](https://ourairports.com/airports/ULOO/), Siauliai = [EYSA](https://ourairports.com/airports/EYSA/).
- CUB: Key West = [KNQX](https://ourairports.com/airports/KNQX/), San Julian = [MUSJ](https://ourairports.com/airports/MUSJ/), Varadero = [MUVR](https://ourairports.com/airports/MUVR/), Cienfuegos = [MUCF](https://ourairports.com/airports/MUCF/).
- EGY: Cairo = [HECA](https://ourairports.com/airports/HECA/), Bengurion = [LLBG](https://ourairports.com/airports/LLBG/), Ras Nasrani = [HESH](https://ourairports.com/airports/HESH/), Al Arish = [HEAR](https://ourairports.com/airports/HEAR/), Fayid = [EG-0006](https://ourairports.com/airports/EG-0006/), Bur Sa'id = [HEPS](https://ourairports.com/airports/HEPS/), Aqaba = [OJAQ](https://ourairports.com/airports/OJAQ/), Hatserim = [LLHB](https://ourairports.com/airports/LLHB/).
- FRA: Bournemouth = [EGHH](https://ourairports.com/airports/EGHH/), Morlaix = [LFRU](https://ourairports.com/airports/LFRU/), Reims = [FR-1241](https://ourairports.com/airports/FR-1241/), Brussels = [EBBR](https://ourairports.com/airports/EBBR/), Chateaudun = [LFOC](https://ourairports.com/airports/LFOC/), Florennes = [EBFS](https://ourairports.com/airports/EBFS/), St Dizier = [LFSI](https://ourairports.com/airports/LFSI/), Amiens = [LFAY](https://ourairports.com/airports/LFAY/), Rouen = [LFOP](https://ourairports.com/airports/LFOP/), Oostende = [EBOS](https://ourairports.com/airports/EBOS/), Southampton = [EGHI](https://ourairports.com/airports/EGHI/), Le Touquet = [LFAT](https://ourairports.com/airports/LFAT/).
- GRE: Athinai = [GR-0098](https://ourairports.com/airports/GR-0098/), Mikonos = [LGMK](https://ourairports.com/airports/LGMK/), Limnos = [LGLM](https://ourairports.com/airports/LGLM/), Iraklion = [LGIR](https://ourairports.com/airports/LGIR/), Maritza = [LGRD](https://ourairports.com/airports/LGRD/).
- IRA: Basrah = [ORMM](https://ourairports.com/airports/ORMM/), Ali Al Salem = [OKAS](https://ourairports.com/airports/OKAS/), Ahvaz = [OIAW](https://ourairports.com/airports/OIAW/), Al Qaysumah = [OEPA](https://ourairports.com/airports/OEPA/).
- KURILE: Burevestnik = [UHSB](https://ourairports.com/airports/UHSB/), Urup = [RU-0088](https://ourairports.com/airports/RU-0088/).
- LFA: Stanley = [SFAL](https://ourairports.com/airports/SFAL/), Walker Creek = [FK-0025](https://ourairports.com/airports/FK-0025/).
- NSK: Sunan = [ZKPY](https://ourairports.com/airports/ZKPY/), Kimpo = [RKSS](https://ourairports.com/airports/RKSS/), Wonsan = [ZKWS](https://ourairports.com/airports/ZKWS/), Kangnung = [RKNN](https://ourairports.com/airports/RKNN/).
- PGU: Sharjah = [OMSJ](https://ourairports.com/airports/OMSJ/), Fujairah = [OMFJ](https://ourairports.com/airports/OMFJ/), Bandar Abbas = [OIKB](https://ourairports.com/airports/OIKB/), Khasab = [OOKB](https://ourairports.com/airports/OOKB/).
- SPA: Jaisalmer = [VIJR](https://ourairports.com/airports/VIJR/), Nawabshah = [OPNH](https://ourairports.com/airports/OPNH/), Multan = [OPMT](https://ourairports.com/airports/OPMT/), Jodhpur = [VIJO](https://ourairports.com/airports/VIJO/), Bahawalpur = [OPBW](https://ourairports.com/airports/OPBW/), Bikaner = [VIBK](https://ourairports.com/airports/VIBK/), Hyderabad = [OPKD](https://ourairports.com/airports/OPKD/), Sukkur = [OPSK](https://ourairports.com/airports/OPSK/).
- TVIET: Vinh = [VVVH](https://ourairports.com/airports/VVVH/), Haiphong = [VVCI](https://ourairports.com/airports/VVCI/), Hanoi = [VVGL](https://ourairports.com/airports/VVGL/), Yen Bai = [VN-0004](https://ourairports.com/airports/VN-0004/).
- UKR: L'viv = [UKLL](https://ourairports.com/airports/UKLL/), Kiev = [UKBB](https://ourairports.com/airports/UKBB/), Simferopol = [UKFF](https://ourairports.com/airports/UKFF/), Rostov = [RU-9991](https://ourairports.com/airports/RU-9991/), KharKiv = [UKHH](https://ourairports.com/airports/UKHH/), Voronezh = [UUOO](https://ourairports.com/airports/UUOO/), Odesa = [UKOO](https://ourairports.com/airports/UKOO/).
- VLA: Spassk Dalniy = [RU-0331](https://ourairports.com/airports/RU-0331/), Mudanjiang = [ZYMD](https://ourairports.com/airports/ZYMD/), Vozdvizhenka = [RU-0327](https://ourairports.com/airports/RU-0327/), Nikolayevka = [RU-0538](https://ourairports.com/airports/RU-0538/), Varfolomeyevka = [RU-0299](https://ourairports.com/airports/RU-0299/), Khoral = [RU-0329](https://ourairports.com/airports/RU-0329/).
- WTA: Chiang Kai Shek = [RCTP](https://ourairports.com/airports/RCTP/), Hua-Lien = [RCYU](https://ourairports.com/airports/RCYU/), Tai-Nan = [RCNN](https://ourairports.com/airports/RCNN/), Qing Yang = [ZSQZ](https://ourairports.com/airports/ZSQZ/), Tai-Tung = [RCFN](https://ourairports.com/airports/RCFN/).

## Reproduce

Run from the repository root with the downloaded public CSV and local extracted
layouts. Output includes center, degrees per foot, reference count and residuals.

```sh
python3 tools/calibrate_theaters.py --layouts .local/airport-research/layouts/FA_2.LIB --airports .local/flight-replay-followup/airports.csv
python3 -m unittest discover -s tools -p test_calibrate_theaters.py
```

Synthetic tests recover known centers and unequal axis spacing and reject
degenerate, reversed, nonfinite and ambiguous inputs. Export tests verify geographic
scaling independently of preserved native U/V and Heading. The full synthetic ACMI
golden was regenerated and reviewed for geographic coordinates and reference time.
An actual Tacview map overlay remains a manual check.
