"""Lane: ai. Surface units: ground target resolution and base-layout air defenses.

`surface-resolve-all` runs `--surface-dump --all`: every offered Quick Mission
ground target template of every theater resolved at every defense level with
three seeds, each theater's base layout, and each template placed in its
theater's scene at heavy defenses. It checks the counts against the retail
survey (template objects, `<sam>` and `<aaa>` slots, targets per template,
base-layout SAM and AAA by side), the 0, 25, 60 and 100 percent rolls, and that
a second run repeats every digest. See docs/spec/surface-defenses.md.
"""
import re

from battery import Scenario, Step

# Per template: (objects, targets before the rolls, <sam> slots, <aaa> slots),
# from the retail survey of the extracted FA_2.LIB (surface-AI round,
# 2026-10-10). Targets count the parked aircraft flagged as targets.
SURVEY = {
    "QBNOTH": (0, 0, 0, 0), "QBFLT": (20, 1, 0, 0), "QBAIR": (66, 7, 10, 10), "QBBRD": (34, 1, 10, 10),
    "QBXING": (48, 6, 10, 10), "QBACOL": (36, 8, 10, 10), "QBFAIR": (55, 9, 10, 10), "QBSPPY": (45, 10, 10, 10),
    "QBSHAR": (47, 1, 10, 10), "QCNOTH": (0, 0, 0, 0), "QCFAIR": (62, 5, 10, 10), "QCSCUD": (66, 6, 10, 10),
    "QCSUB": (36, 4, 10, 10), "QCLST": (32, 12, 10, 10), "QCCARG": (6, 4, 0, 0), "QCCMHQ": (51, 4, 10, 10),
    "QENOTH": (0, 0, 0, 0), "QESFLT": (7, 3, 0, 0), "QESAIR": (32, 4, 10, 10), "QELAIR": (45, 8, 10, 10),
    "QECMHQ": (31, 5, 10, 10), "QERDRI": (32, 7, 10, 10), "QEARMOR": (28, 5, 10, 10), "QECDEF": (34, 3, 10, 10),
    "QLFNOTH": (0, 0, 0, 0), "QLFCARG": (27, 3, 10, 10), "QLFPATR": (24, 4, 10, 10), "QLFSAM": (75, 6, 10, 10),
    "QLFFAIR": (60, 5, 10, 10), "QLFSTOR": (64, 17, 10, 10), "QLFCMHQ": (65, 1, 10, 10), "QFNOTH": (0, 0, 0, 0),
    "QFFLT": (14, 1, 0, 0), "QFSAIR": (37, 4, 10, 10), "QFLAIR": (44, 6, 10, 10), "QFSUP": (29, 5, 10, 10),
    "QFRDRI": (36, 8, 10, 10), "QFCMHQ": (31, 6, 10, 10), "QFFACT": (38, 9, 10, 10), "QGRNOTH": (0, 0, 0, 0),
    "QGRSAIR": (85, 10, 10, 10), "QGRPATR": (4, 4, 0, 0), "QGRRDR": (113, 4, 10, 10), "QGRCARG": (10, 3, 0, 0),
    "QGRSTOR": (116, 7, 10, 10), "QIRNOTH": (0, 0, 0, 0), "QIRRDR": (87, 6, 10, 10), "QIRFAIR": (85, 6, 10, 10),
    "QIRPOW": (47, 5, 10, 10), "QIRCCC": (105, 6, 10, 10), "QIRARM": (74, 7, 10, 10), "QIRSCUD": (94, 4, 10, 10),
    "QIRCWP": (65, 6, 10, 10), "QIRRETR": (94, 8, 10, 10), "QKNOTH": (0, 0, 0, 0), "QKSFLT": (5, 1, 0, 0),
    "QKLFLT": (16, 1, 0, 0), "QKSCFT": (26, 6, 10, 10), "QKSUB": (15, 4, 6, 5), "QKPLNGR": (22, 4, 10, 8),
    "QKSILO": (34, 7, 10, 10), "QKARMOR": (28, 8, 10, 10), "QTNOTH": (0, 0, 0, 0), "QTBARG": (49, 5, 4, 5),
    "QTCARGO": (17, 3, 5, 0), "QTBRDG": (46, 1, 6, 4), "QTBUNK": (63, 6, 7, 8), "QTCOMM": (36, 2, 6, 9),
    "QTSTRG": (36, 6, 6, 6), "QTTRUCK": (48, 13, 6, 4), "QTAAA": (66, 8, 5, 0), "QTSAM": (69, 4, 0, 4),
    "QSPNOTH": (0, 0, 0, 0), "QSPFAIR": (82, 8, 10, 10), "QSPSAM": (98, 9, 10, 10), "QSPASA": (70, 12, 10, 10),
    "QSPFRU": (86, 5, 10, 10), "QSPSUP": (50, 15, 10, 10), "QSPCMHQ": (61, 4, 10, 10), "QAPNOTH": (0, 0, 0, 0),
    "QAPFAIR": (59, 5, 10, 10), "QAPBLK": (27, 2, 10, 10), "QAPPATR": (55, 5, 10, 10), "QAPHELO": (67, 8, 10, 10),
    "QAPSAM": (76, 4, 10, 10), "QAPCMHQ": (99, 15, 10, 10), "QPGNOTH": (0, 0, 0, 0), "QPGPATR": (24, 4, 10, 10),
    "QPGFAIR": (74, 4, 10, 10), "QPGSAM": (127, 9, 10, 10), "QPGSRUN": (103, 9, 10, 10), "QPGRDR": (70, 5, 10, 10),
    "QPGWSHP": (36, 2, 10, 10), "QNSNOTH": (0, 0, 0, 0), "QNSFAIR": (91, 7, 10, 10), "QNSARM": (86, 8, 10, 10),
    "QNSFOA": (97, 8, 10, 10), "QNSBORD": (70, 10, 10, 10), "QNSCOL": (46, 8, 10, 10), "QNSSUP": (90, 14, 10, 10),
    "QWTNOTH": (0, 0, 0, 0), "QWTFAIR": (66, 9, 10, 10), "QWTPATR": (6, 6, 0, 0), "QWTHYDO": (11, 5, 0, 0),
    "QWTWARS": (11, 2, 0, 0), "QWTCARG": (9, 3, 0, 0), "QWTLAND": (75, 13, 10, 10), "QUNOTH": (0, 0, 0, 0),
    "QUSFLT": (5, 1, 0, 0), "QULFLT": (16, 1, 0, 0), "QUCITY": (39, 39, 12, 11), "QUFACT": (19, 3, 4, 3),
    "QUSTRIP": (58, 10, 10, 10), "QUCOL": (29, 3, 10, 10), "QUNUKE": (34, 6, 15, 11), "QUBRI": (15, 1, 3, 7),
    "QVNOTH": (0, 0, 0, 0), "QVSFLT": (7, 3, 0, 0), "QVSAIR": (43, 4, 10, 10), "QVLAIR": (55, 11, 10, 10),
    "QVCMHQ": (27, 6, 10, 10), "QVARMOR": (28, 8, 10, 10), "QVRDRI": (30, 5, 10, 10), "QVSUP": (29, 5, 10, 10),
}

# Defense slots that are themselves targets: `~QUCITY` flags 11 <AAA> and 12
# <SAM> slots, so with defenses at none it has 16 targets.
DEFENSE_TARGETS = {"QUCITY": 23}

# Base-layout SAM and AAA units, (Redfor, Blue), from the same survey; all
# 321 go active on both sides (John, 2026-10-10).
BASE_DEFENSES = {
    "BAL": (10, 20), "CUB": (19, 4), "EGY": (0, 0), "LFA": (5, 2), "FRA": (0, 0),
    "GRE": (10, 14), "IRA": (13, 10), "KURILE": (2, 0), "TVIET": (98, 0), "SPA": (8, 14),
    "APA": (12, 9), "PGU": (13, 9), "NSK": (14, 16), "WTA": (5, 14), "UKR": (0, 0), "VLA": (0, 0),
}

ALL_LINE = re.compile(
    r"^surface-all: (\S+) (\S+) level (\d) seed (\d+) objects (\d+) sam-slots (\d+) aaa-slots (\d+) "
    r"targets (\d+) manned (\d+) removed (\d+) units (\d+) parked (\d+) left-out (\d+) digest (0x[0-9a-f]+)$",
    re.M,
)
BASE_LINE = re.compile(r"^surface-base: (\S+) units (\d+) sam-aaa-red (\d+) sam-aaa-blue (\d+) ", re.M)


def resolve_all_problems(output: str) -> list[str]:
    """Counts against the survey, the roll shares, and digests that repeat."""
    problems: list[str] = []
    first, _, again = output.partition("$ then 1:")
    digests: dict[tuple, str] = {}
    shares = {1: [0, 0], 2: [0, 0]}
    for m in ALL_LINE.finditer(first):
        stem, level, seed = m.group(2), int(m.group(3)), int(m.group(4))
        objects, sam, aaa, targets, manned, removed = (int(m.group(i)) for i in range(5, 11))
        digests[(stem, level, seed)] = m.group(14)
        expect = SURVEY.get(stem)
        if expect is None:
            problems.append(f"{stem}: not in the retail survey")
            continue
        want_objects, want_targets, want_sam, want_aaa = expect
        if (objects, sam, aaa) != (want_objects, want_sam, want_aaa):
            problems.append(f"{stem}: {objects} objects, {sam}/{aaa} slots; survey {want_objects}, {want_sam}/{want_aaa}")
        if manned + removed != sam + aaa:
            problems.append(f"{stem} level {level} seed {seed}: {manned} manned and {removed} removed of {sam + aaa}")
        if level == 3 and (targets, manned, removed) != (want_targets, sam + aaa, 0):
            problems.append(f"{stem} heavy: {targets} targets, {manned} manned; survey {want_targets}, all {sam + aaa}")
        if level == 0:
            want = want_targets - DEFENSE_TARGETS.get(stem, 0)
            if (targets, manned) != (want, 0):
                problems.append(f"{stem} none: {targets} targets, {manned} manned; expected {want}, 0")
        if level in shares:
            shares[level][0] += manned
            shares[level][1] += sam + aaa
    missing = sorted(set(SURVEY) - {stem for stem, _, _ in digests})
    if missing:
        problems.append(f"templates never resolved: {missing}")
    for level, percent in ((1, 25.0), (2, 60.0)):
        manned, slots = shares[level]
        if slots and abs(100.0 * manned / slots - percent) > 3.0:
            problems.append(f"level {level}: {100.0 * manned / slots:.1f} percent of {slots} slots manned, retail {percent:.0f}")
    bases = {m.group(1): (int(m.group(3)), int(m.group(4))) for m in BASE_LINE.finditer(first)}
    for theater, want in BASE_DEFENSES.items():
        if bases.get(theater) != want:
            problems.append(f"{theater} base layout SAM and AAA (Redfor, Blue) {bases.get(theater)}, survey {want}")
    # The second run (seed 1 only) gives the same digests.
    repeats = 0
    for m in ALL_LINE.finditer(again):
        key = (m.group(2), int(m.group(3)), int(m.group(4)))
        repeats += 1
        if digests.get(key) != m.group(14):
            problems.append(f"{key}: digest {digests.get(key)} then {m.group(14)}")
    if repeats != len(SURVEY) * 4:
        problems.append(f"second run resolved {repeats} times, expected {len(SURVEY) * 4}")
    return problems


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="surface-resolve-all", lane="ai",
            args=["--surface-dump", "--all"], timeout=300,
            expect=[r"^surface-all: 1488 resolutions, 124 templates, 0 errors$"],
            forbid=[r"^surface-all: error"],
            then=[Step(["--surface-dump", "--all", "--seeds", "1"], timeout=300)],
            check=resolve_all_problems,
            notes="Reads the templates from the retail media (TORE_GAME_DIR, the remembered source or the "
                  "gameassets link) until the import keeps them.",
        ),
    ]
