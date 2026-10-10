"""Lane: ai. Surface units: ground target resolution, layout and base-layout air defenses.

`surface-resolve-all` runs `--surface-dump --all`: every offered Quick Mission
ground target template of every theater resolved at every defense level with
three seeds, each theater's base layout, and each template placed in its
theater's scene at heavy defenses. It checks the counts against the retail
survey (template objects, `<sam>` and `<aaa>` slots, targets per template,
base-layout SAM and AAA by side), the 0, 25, 60 and 100 percent rolls, and that
a second run repeats every digest.

`surface-relocate-sweep` runs `--surface-dump --sweep`: every offered template
placed at heavy defenses with seeds 1 to 20. Every placement passes the site
rules, the anchored templates are exactly the expected ones and never move, at
least 60 percent of the other placements relocate, the batteries and trucks
keep their rules, each base layout forms its batteries, and a second run
repeats the digests.

`surface-start-placement` runs `--surface-dump --starts`: one mission per
theater with a ground target and a 50 nm enemy distance. Red starts within
5 nm of the target; Blue starts 50 nm from Red toward its own side, heading
at the target; both on the map. See
docs/spec/surface-defenses.md.

The engagement scenarios run `--surface-trace`, which flies the player on a
scripted straight line past one surface unit and prints what its controller
does (phases, shots, bursts, radar, HARM rolls, battery changes), the RWR's
ground squares, locks and tone, decoy rolls and a summary line. They read the
retail records the import does not keep yet from the retail media, as the dump
does. Numbers they check are the record's and the AAA tuning table's
(docs/spec/surface-defenses.md).
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


TRACE = re.compile(r"^surface-trace: t=([0-9.]+) (.*)$", re.M)
SUMMARY = re.compile(r"^surface-trace: summary (.*)$", re.M)
SUBJECT = re.compile(r"^surface-trace: subject (0x[0-9a-f]+) (\S+) side (\d+)", re.M)
RADAR = re.compile(r"^surface-trace: radar (0x[0-9a-f]+)$", re.M)


def runs(output: str) -> list[str]:
    """The output of each run: the main command, then each `then` step."""
    return re.split(r"^\$ then \d+:.*$", output, flags=re.M)


def summary(run: str) -> dict[str, str]:
    m = SUMMARY.search(run)
    if not m:
        return {}
    words = m.group(1).split()
    return dict(zip(words[::2], words[1::2]))


def events(run: str) -> list[tuple[float, str]]:
    return [(float(m.group(1)), m.group(2)) for m in TRACE.finditer(run)]


def subject(run: str) -> str:
    m = SUBJECT.search(run)
    return m.group(1) if m else "?"


def first(run: str, pattern: str) -> float | None:
    for t, text in events(run):
        if re.search(pattern, text):
            return t
    return None


def near(a: float | None, b: float, tolerance: float = 0.02) -> bool:
    return a is not None and abs(a - b) <= tolerance


def sa6_engage_problems(output: str) -> list[str]:
    """SA-6 (NPC 10 s search, 36 s first preparation; record 5 s lock, 15 s
    between salvos): the phases at the record times, a launch, the RWR's
    ground square, the lock tone and then the missile tone."""
    problems: list[str] = []
    run = runs(output)[0]
    unit = subject(run)
    # A launcher in a battery fights through its battery's controller.
    prepare = first(run, rf"^(phase {unit} w0|battery \d+) Prepare")
    track = first(run, rf"^(phase {unit} w0|battery \d+) Track")
    shots = [t for t, text in events(run) if text.startswith(f"shot {unit} ") and " rounds 1 " in text]
    if prepare is None or not near(track, prepare + 36.0):
        problems.append(f"preparation {prepare} to lock {track}: expected 36 s")
    if not shots or not near(shots[0], (track or 0) + 5.0):
        problems.append(f"first launch {shots[:1]} not 5 s after the lock at {track}")
    if len(shots) < 2 or not near(shots[1] - shots[0], 15.0):
        problems.append(f"launches {shots}: expected 15 s apart")
    s = summary(run)
    for key in ("ground-square", "radar-lock-tone", "inbound-tone"):
        if s.get(key) != "true":
            problems.append(f"{key} {s.get(key)}")
    lock = first(run, r"^rwr .*tone Some\(RadarLock\)")
    inbound = first(run, r"^rwr .*tone Some\(RadarInbound\)")
    if lock is None or inbound is None or lock >= inbound:
        problems.append(f"lock tone at {lock} should come before the missile tone at {inbound}")
    return problems


def sa2_low_problems(output: str) -> list[str]:
    """Under the SA-2's 3,000 ft seeker floor: its radar is on and the RWR shows
    it, but it never launches."""
    problems: list[str] = []
    run = runs(output)[0]
    m = RADAR.search(run)
    radar = m.group(1) if m else subject(run)
    if first(run, rf"^radar {radar} on") is None:
        problems.append("the SA-2's radar never came on")
    s = summary(run)
    if s.get("ground-square") != "true":
        problems.append("no RWR ground square for the SA-2")
    if s.get("missiles") != "0":
        problems.append(f"{s.get('missiles')} missiles launched under the floor")
    return problems


def chaff_flare_problems(output: str) -> list[str]:
    """SA-6 and SA-7 launches against a jet dispensing chaff and flares: the
    radar missile rolls against chaff, the infrared one against flares, and
    some are decoyed."""
    problems: list[str] = []
    run = runs(output)[0]
    rolls = [text for _, text in events(run) if text.startswith("decoy ")]
    if not any(r.startswith("decoy Chaff") and " SA6.JT " in r for r in rolls):
        problems.append("no chaff roll against an SA-6")
    if not any(r.startswith("decoy Flare") and " SA7.JT " in r for r in rolls):
        problems.append("no flare roll against an SA-7")
    if any(r.startswith("decoy Chaff") and (" SA7.JT " in r or " SA13.JT " in r or " SA14.JT " in r) for r in rolls):
        problems.append("chaff rolled against an infrared missile")
    if any(r.startswith("decoy Flare") and " SA6.JT " in r for r in rolls):
        problems.append("a flare rolled against a radar missile")
    if not any(r.endswith("decoyed true") for r in rolls):
        problems.append("nothing was decoyed")
    return problems


def harm_problems(output: str) -> list[str]:
    """An AGM-88 at an SA-6 battery's Straight Flush: an ace radar shuts down,
    the battery goes Blind for 30 s and comes back; a killed radar blinds it
    for good."""
    problems: list[str] = []
    harm, killed = (runs(output) + ["", ""])[:2]
    m = RADAR.search(harm)
    radar = m.group(1) if m else "?"
    shut = first(harm, rf"^harm-roll {radar} .* shutdown$")
    if shut is None:
        problems.append("no HARM shutdown roll succeeded")
    else:
        if not near(first(harm, rf"^radar {radar} off"), shut):
            problems.append("the radar did not go off with the shutdown")
        if not near(first(harm, r"^battery \d+ Blind"), shut):
            problems.append("the battery did not go Blind with the shutdown")
        back = [t for t, text in events(harm) if text == f"radar {radar} on" and t > shut]
        if not back or not near(back[0], shut + 30.0):
            problems.append(f"the radar came back at {back[:1]}, expected 30 s after {shut}")
    kill = first(killed, r"^killed ")
    blind = first(killed, r"^battery \d+ Blind")
    if kill is None or not near(blind, kill):
        problems.append(f"killing the radar at {kill} did not blind the battery ({blind})")
    after = [text for t, text in events(killed) if kill is not None and t > kill and text.startswith("battery ")]
    if after:
        problems.append(f"the battery left Blind after its radar died: {after[:2]}")
    if any(t > (kill or 0) for t, text in events(killed) if text.startswith("shot ")):
        problems.append("a launch after the radar died")
    return problems


def battery_sa2_problems(output: str) -> list[str]:
    """A North Vietnam SA-2 site with its GCI radar as a battery: the radar is
    the RWR's square, never the launcher; with the radar killed in daylight it
    launches on its optical backup with no lock tone; at night it does not."""
    problems: list[str] = []
    live, optical, night = (runs(output) + ["", "", ""])[:3]
    unit = subject(live)
    m = RADAR.search(live)
    radar = m.group(1) if m else "?"
    squares = [text for _, text in events(live) if text.startswith("rwr ground-squares")]
    if not any(f'"{radar}"' in text for text in squares):
        problems.append(f"the radar {radar} never showed on the RWR")
    if any(f'"{unit}"' in text for text in squares):
        problems.append(f"the launcher {unit} showed on the RWR")
    if int(summary(live).get("missiles", "0")) == 0 or summary(live).get("radar-lock-tone") != "true":
        problems.append(f"the battery did not lock and launch: {summary(live)}")
    s = summary(optical)
    if int(s.get("missiles", "0")) == 0:
        problems.append("no optical launch in daylight")
    if s.get("lock-tone") != "false":
        problems.append("a lock tone from a blind battery")
    if first(optical, r"^battery \d+ .*optical") is None:
        problems.append("the battery never went to its optical backup")
    if summary(night).get("missiles") != "0" or first(night, r"^battery \d+ Blind") is None:
        problems.append(f"at night a blind SA-2 battery fired or did not go Blind: {summary(night)}")
    return problems


def zsu23_problems(output: str) -> list[str]:
    """ZSU-23 (AAA tuning row: 99-round bursts at 3,400 rpm, a 2,000-round
    magazine, every third round a tracer): bursts, tracers, hits, magazine."""
    problems: list[str] = []
    run = runs(output)[0]
    unit = subject(run)
    bursts = [text for _, text in events(run) if text.startswith(f"burst {unit} ")]
    full = [b for b in bursts if " rounds 99 " in b]
    if not full:
        problems.append(f"no full 99-round burst: {bursts[:3]}")
    for b in full:
        rpm = float(b.rsplit("rpm ", 1)[1])
        if abs(rpm - 3400) > 34:
            problems.append(f"burst rate {rpm} rpm, table 3,400")
    s = summary(run)
    rounds, tracers = int(s.get("rounds", "0")), int(s.get("tracers", "0"))
    if abs(tracers - rounds / 3) > 1:
        problems.append(f"{tracers} tracers in {rounds} rounds, every third")
    if int(s.get("hits", "0")) == 0:
        problems.append("no hits registered")
    m = re.search(rf"^surface-trace: stock {unit} m0 loaded (\d+) reserve (\d+)$", run, re.M)
    if not m or int(m.group(1)) + rounds != 2000 or m.group(2) != "2":
        problems.append(f"magazine: {m.group(0) if m else None} after {rounds} rounds of 2,000")
    return problems


def flak_problems(output: str) -> list[str]:
    """KS-19 over North Vietnam's AAA emplacement: an eight-shell opening
    barrage, single shells after, flak bursts high and no tracers; at 3,000 ft,
    under the 4,000 ft floor, it holds fire."""
    problems: list[str] = []
    high, low = (runs(output) + ["", ""])[:2]
    unit = subject(high)
    shots = [text for _, text in events(high) if text.startswith(f"shot {unit} ")]
    opening = [s for s in shots if " opening" in s]
    if len(opening) != 8 or shots[:8] != opening:
        problems.append(f"opening barrage {len(opening)} shells, expected the first 8")
    if len(shots) <= 8:
        problems.append("no shells after the opening barrage")
    s = summary(high)
    if s.get("tracers") != "0":
        problems.append(f"{s.get('tracers')} flak tracers")
    if int(s.get("flak-bursts", "0")) == 0 or s.get("min-burst-ft") == "none" or float(s["min-burst-ft"]) < 4000:
        problems.append(f"flak bursts {s.get('flak-bursts')} lowest {s.get('min-burst-ft')} ft")
    if any(text.startswith(f"shot {subject(low)} ") for _, text in events(low)):
        problems.append("the KS-19 fired below its 4,000 ft floor")
    return problems


def barrage_problems(output: str) -> list[str]:
    """North Vietnam SAM sites' barrage zone: it fires about a third of its
    bursts (random fire 33 percent) and sleeps once the jet leaves."""
    problems: list[str] = []
    run = runs(output)[0]
    unit = subject(run)
    tried = sum(1 for _, text in events(run) if text.startswith(f"phase {unit} w0 Fire"))
    fired = sum(1 for _, text in events(run) if text.startswith(f"burst {unit} "))
    if tried < 20 or not 0.1 <= fired / tried <= 0.6:
        problems.append(f"{fired} of {tried} bursts fired, retail 33 percent")
    if first(run, rf"^phase {unit} w0 Idle") is None:
        problems.append("the zone never went back to sleep")
    if summary(run).get("ground-square") != "false":
        problems.append("the barrage zone showed on the RWR")
    return problems


def base_defenses_problems(output: str) -> list[str]:
    """Cuba's base layout: an enemy SA-2 engages a Blue jet, and Blue's M163s
    engage a Redfor one, whose RWR shows their radar."""
    problems: list[str] = []
    red, blue = (runs(output) + ["", ""])[:2]
    m = SUBJECT.search(red)
    if not m or m.group(3) != "2" or int(summary(red).get("missiles", "0")) == 0:
        problems.append(f"the Redfor SA-2 did not engage: {summary(red)}")
    m = SUBJECT.search(blue)
    if not m or m.group(3) != "1" or int(summary(blue).get("rounds", "0")) == 0:
        problems.append(f"the Blue M163 did not engage a Redfor jet: {summary(blue)}")
    # Guantanamo's M163 stands low on a slope: its radar must still show
    # (reception ends at its antenna, above the ground).
    if summary(blue).get("ground-square") != "true":
        problems.append("the Blue M163's radar never showed on the Redfor jet's RWR")
    return problems


TRACE_NOTES = ("Reads the surface records the import does not keep yet from the retail media "
               "(TORE_GAME_DIR, the remembered source or the gameassets link).")


def trace(*args: str) -> list[str]:
    return ["--surface-trace", *args]
# Templates that stay at their retail spot, by the anchoring rule over their
# contents (docs/spec/surface-defenses.md, "Relocation"), measured on the
# retail data (surface-AI round, slice L1, 2026-10-10): the plan's 31 with
# ~QPGSRUN by its target beside a dirt strip, plus ~QTCARGO and ~QUFACT
# (routes, lead ruling) and ~QCCMHQ (its centroid 0.6 nm from a dirt strip).
ANCHORED = {
    "QAPFAIR": "runway", "QAPHELO": "runway", "QBAIR": "runway", "QBBRD": "bridge-or-road",
    "QBFAIR": "strip", "QCCMHQ": "runway", "QCFAIR": "runway", "QELAIR": "runway",
    "QESAIR": "runway", "QFFACT": "strip", "QFLAIR": "runway", "QFSAIR": "runway",
    "QGRSAIR": "strip", "QIRFAIR": "runway", "QKPLNGR": "runway", "QLFFAIR": "runway",
    "QNSFAIR": "runway", "QPGFAIR": "runway", "QPGSRUN": "town", "QSPFAIR": "runway",
    "QTBARG": "town", "QTBRDG": "bridge-or-road", "QTBUNK": "strip", "QTCARGO": "route",
    "QTSTRG": "town", "QTTRUCK": "bridge-or-road", "QUBRI": "bridge-or-road", "QUCITY": "town",
    "QUCOL": "route", "QUFACT": "route", "QUSTRIP": "strip", "QVLAIR": "runway",
    "QVSAIR": "runway", "QWTFAIR": "runway",
}

# Base-layout batteries per theater (SA-2, SA-3, SA-6, HAWK), from the plan's
# list (section 3.11): Cuba 5 SA-2 and 2 SA-6, North Vietnam 13 SA-2, the
# Baltics' 4 HAWK in two sites, Panama 3 SA-6 and a HAWK pair, the SA-6 sites
# of Iraq, Pakistan, the Persian Gulf, South Korea and Taiwan.
BASE_BATTERIES = {
    "BAL": (0, 0, 0, 2), "CUB": (5, 0, 2, 0), "IRA": (0, 0, 4, 0), "TVIET": (13, 0, 0, 0),
    "SPA": (0, 0, 2, 0), "APA": (0, 0, 3, 1), "PGU": (0, 0, 3, 0), "NSK": (0, 0, 4, 0),
    "WTA": (0, 0, 2, 0),
}

SWEEP_LINE = re.compile(
    r"^surface-sweep: (\S+) (\S+) seed (\d+) anchor (\S+) moved-ft (\d+) rotation (\d+) units (\d+) "
    r"trucks (\d+) batteries (\d+) problems (\d+) digest (0x[0-9a-f]+)$",
    re.M,
)
BASE_BATTERY_LINE = re.compile(
    r"^surface-base-batteries: (\S+) batteries (\d+) sa2 (\d+) sa3 (\d+) sa6 (\d+) hawk (\d+) adopted (\d+) "
    r"added (\d+) launchers (\d+) in-batteries (\d+) ",
    re.M,
)


def relocate_sweep_problems(output: str) -> list[str]:
    """Site rules, anchors, relocation share, base batteries, repeated digests."""
    problems: list[str] = []
    first, _, again = output.partition("$ then 1:")
    digests: dict[tuple, str] = {}
    anchors: dict[str, str] = {}
    free = moved = 0
    for m in SWEEP_LINE.finditer(first):
        stem, seed, anchor, distance = m.group(2), int(m.group(3)), m.group(4), int(m.group(5))
        digests[(stem, seed)] = m.group(11)
        anchors.setdefault(stem, anchor)
        if anchors[stem] != anchor:
            problems.append(f"{stem}: anchor {anchor} with seed {seed}, {anchors[stem]} before")
        if anchor == "none":
            free += 1
            moved += distance > 0
        elif distance:
            problems.append(f"{stem}: anchored ({anchor}) but moved {distance} ft with seed {seed}")
    if len(anchors) != len(SURVEY) - 16:
        problems.append(f"{len(anchors)} templates swept, expected {len(SURVEY) - 16}")
    want = {stem: anchor for stem, anchor in ANCHORED.items()}
    got = {stem: anchor for stem, anchor in anchors.items() if anchor != "none"}
    if got != want:
        extra = sorted(set(got.items()) - set(want.items()))
        missing = sorted(set(want.items()) - set(got.items()))
        problems.append(f"anchored templates differ: extra {extra}, missing {missing}")
    if free and moved < 0.6 * free:
        problems.append(f"only {moved} of {free} free placements relocated")
    for line in re.findall(r"^surface-sweep-problem: .*$", first, re.M)[:20]:
        problems.append(line)
    bases = {m.group(1): m for m in BASE_BATTERY_LINE.finditer(first)}
    for theater, m in bases.items():
        counts = tuple(int(m.group(i)) for i in range(3, 7))
        if counts != BASE_BATTERIES.get(theater, (0, 0, 0, 0)):
            problems.append(f"{theater} base batteries (SA-2, SA-3, SA-6, HAWK) {counts}, expected {BASE_BATTERIES.get(theater, (0, 0, 0, 0))}")
        if m.group(9) != m.group(10):
            problems.append(f"{theater}: {m.group(10)} of {m.group(9)} battery launchers in batteries")
    if len(bases) != 16:
        problems.append(f"{len(bases)} base layouts, expected 16")
    repeats = 0
    for m in SWEEP_LINE.finditer(again):
        key = (m.group(2), int(m.group(3)))
        repeats += 1
        if digests.get(key) != m.group(11):
            problems.append(f"{key}: digest {digests.get(key)} then {m.group(11)}")
    if repeats != 2 * (len(SURVEY) - 16):
        problems.append(f"second run placed {repeats} times, expected {2 * (len(SURVEY) - 16)}")
    return problems


START_LINE = re.compile(
    r"^surface-start: (\S+) (\S+) red-target-nm (-?[\d.]+) blue-red-nm ([\d.]+) separation-nm (\d+) "
    r"side-off-deg ([\d.]+) heading-off-deg ([\d.]+) blue-on-map (\d) red-on-map (\d) front (\d)$",
    re.M,
)


def start_placement_problems(output: str) -> list[str]:
    """Red within 5 nm of the target; Blue the separation from Red toward its side, at the target; on the map."""
    problems: list[str] = []
    lines = list(START_LINE.finditer(output))
    if len(lines) != 16:
        problems.append(f"{len(lines)} theaters checked, expected 16")
    for m in lines:
        theater = m.group(1)
        red, blue, separation, side_off, heading_off = (float(m.group(i)) for i in (3, 4, 5, 6, 7))
        if not 0.0 <= red <= 5.01:
            problems.append(f"{theater}: Red starts {red} nm from the target")
        # A map too small for the separation shortens it; none of the 16 is.
        if abs(blue - separation) > 0.05:
            problems.append(f"{theater}: Blue starts {blue} nm from Red, separation {separation:.0}")
        # The spread is 30 degrees about a whole-degree bearing.
        if m.group(10) == "1" and side_off > 31.0:
            problems.append(f"{theater}: Blue starts {side_off} degrees off its side of the front")
        # Blue's heading is a whole degree.
        if heading_off > 0.6:
            problems.append(f"{theater}: Blue heads {heading_off} degrees off the target")
        if (m.group(8), m.group(9)) != ("1", "1"):
            problems.append(f"{theater}: a start is off the map")
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
        Scenario(
            name="surface-sa6-engage", lane="ai", timeout=300,
            args=trace("IRA", "--over", "SA6", "--altitude", "15000", "--from", "20", "--seconds", "150",
                       "--invulnerable"),
            forbid=[r"^surface-trace: t=\S+ player crashed"],
            check=sa6_engage_problems, notes=TRACE_NOTES,
        ),
        Scenario(
            name="surface-sa2-low", lane="ai", timeout=300,
            args=trace("TVIET", "--over", "SA2A", "--altitude", "1000", "--from", "20", "--seconds", "150",
                       "--invulnerable", "--quiet-shots"),
            forbid=[r"^surface-trace: t=\S+ player crashed"],
            check=sa2_low_problems, notes=TRACE_NOTES,
        ),
        Scenario(
            name="surface-chaff-flare", lane="ai", timeout=300,
            args=trace("UKR", "QUFACT", "--surface-seed", "2", "--unit", "0x5000000e", "--near", "2000",
                       "--altitude", "5000", "--speed", "300", "--from", "6", "--seconds", "120",
                       "--chaff", "1", "--flares", "1", "--invulnerable"),
            check=chaff_flare_problems, notes=TRACE_NOTES + " Seed 2 mans the factory's SA-6 and SA-7 slots.",
        ),
        Scenario(
            name="surface-harm", lane="ai", timeout=400,
            args=trace("VLA", "QVRDRI", "--surface-seed", "4", "--unit", "0x5000000d",
                       "--skill", "3", "--rng", "2", "--aircraft", "f16c", "--altitude", "20000",
                       "--from", "25", "--harm-at", "10", "--seconds", "200", "--invulnerable"),
            then=[Step(trace("VLA", "QVRDRI", "--surface-seed", "4", "--unit", "0x5000000d",
                             "--skill", "3", "--aircraft", "f16c", "--altitude", "20000", "--from", "25",
                             "--kill-at", "140", "--seconds", "220", "--invulnerable"), timeout=300)],
            check=harm_problems,
            notes=TRACE_NOTES + " The AGM-88 is put in flight from the jet directly (no designation of ground "
                  "emitters yet).",
        ),
        Scenario(
            name="surface-battery-sa2", lane="ai", timeout=400,
            args=trace("TVIET", "--over", "SA2A", "--index", "6", "--altitude", "20000",
                       "--from", "20", "--seconds", "200", "--invulnerable", "--quiet-shots"),
            then=[
                Step(trace("TVIET", "--over", "SA2A", "--index", "6", "--altitude", "20000",
                           "--from", "20", "--seconds", "200", "--invulnerable", "--quiet-shots",
                           "--kill-at", "1"), timeout=300),
                Step(trace("TVIET", "--over", "SA2A", "--index", "6", "--altitude", "20000",
                           "--from", "20", "--seconds", "200", "--invulnerable", "--quiet-shots",
                           "--kill-at", "1", "--condition", "night"), timeout=300),
            ],
            check=battery_sa2_problems,
            notes=TRACE_NOTES + " The North Vietnam SA-2 whose battery adopted a GCI.",
        ),
        Scenario(
            name="surface-zsu23-gun", lane="ai", timeout=300,
            args=trace("BAL", "--over", "ZSU23", "--altitude", "1500", "--speed", "250", "--from", "4",
                       "--seconds", "120", "--quiet-shots"),
            check=zsu23_problems, notes=TRACE_NOTES,
        ),
        Scenario(
            name="surface-flak-tviet", lane="ai", timeout=400,
            args=trace("TVIET", "QTAAA", "--over", "KS19", "--aircraft", "a10", "--altitude", "15000",
                       "--speed", "300", "--from", "10", "--seconds", "120", "--invulnerable"),
            then=[Step(trace("TVIET", "QTAAA", "--over", "KS19", "--aircraft", "a10", "--altitude", "3000",
                             "--speed", "300", "--from", "10", "--seconds", "120", "--invulnerable"),
                       timeout=300)],
            check=flak_problems, notes=TRACE_NOTES,
        ),
        Scenario(
            name="surface-barrage-zone", lane="ai", timeout=300,
            args=trace("TVIET", "QTSAM", "--over", "A_M1939", "--altitude", "3000", "--speed", "250",
                       "--from", "6", "--pass", "2000", "--seconds", "300", "--quiet-shots", "--invulnerable"),
            check=barrage_problems, notes=TRACE_NOTES,
        ),
        Scenario(
            name="surface-base-defenses", lane="ai", timeout=400,
            args=trace("CUB", "--over", "SA2A", "--altitude", "20000", "--from", "20", "--seconds", "150",
                       "--quiet-shots", "--invulnerable"),
            then=[Step(trace("CUB", "--over", "M163", "--player-side", "red", "--altitude", "1500",
                             "--speed", "300", "--from", "4", "--seconds", "90", "--quiet-shots",
                             "--invulnerable"), timeout=300)],
            check=base_defenses_problems, notes=TRACE_NOTES,
        ),
        Scenario(
            name="surface-relocate-sweep", lane="ai",
            args=["--surface-dump", "--sweep"], timeout=600,
            expect=[r"^surface-sweep: 2160 placements, \d+ relocated, 0 problems, 0 errors$"],
            forbid=[r"^surface-sweep: error"],
            then=[Step(["--surface-dump", "--sweep", "--seeds", "2"], timeout=300)],
            check=relocate_sweep_problems,
            notes="Reads the templates from the retail media until the import keeps them.",
        ),
        Scenario(
            name="surface-start-placement", lane="ai",
            args=["--surface-dump", "--starts"], timeout=300,
            expect=[r"^surface-start: 16 theaters$"],
            check=start_placement_problems,
            notes="Reads the templates from the retail media until the import keeps them.",
        ),
    ]
