"""Lane: ai. The ground pages (docs/spec/air-to-ground.md, "The air-to-ground radar",
"FLIR ground contacts"), slice G1 of the air-to-ground round.

`a2g-ag-radar` runs `--a2g-radar`, which flies a scripted level line at a Quick Mission ground
target and selects a page through the player's own controls. Over the Ukraine column `~QUCOL` an
F/A-18D turns the A-G page on with the A-G command (Ctrl+R): the moving tanks are dots, the
air radar is off (no air contacts, no emission for anti-radiation seekers), T designates the
nearest tank and the target window names it. Retail lists an aircraft on the A-G page only while
it is in an airfield's takeoff or landing sequence, whatever its speed (docs/formats/radar.md,
"A-G page"; the manual's 90 kt split is not in the code): with the nose on the wingman, a fighter
in free flight, the page does not list it; told it is in an airfield sequence it lists it as a
square. Over the defended column the SAM and AAA units are the X.

`a2g-flir` puts an AV-8B on the IR page (M) 4 nmi from the column: every tank is listed (the IR
page does not merge, and lists ground objects out to 4.97 nmi), T designates the nearest and the
target window names it. In fog the infrared range is a tenth of the sensor's 15 nmi, so nothing
is listed from 4 nmi and the nearest tanks come back from 1.2 nmi. The plan named the A-10, whose
imported sensors have no infrared channel.
"""
import re

from battery import Scenario, Step

CONTACT = re.compile(
    r"^a2g-radar: contact (0x[0-9a-f]+) (\S+) class (0x[0-9a-f]+) symbol (\S+) distance (\S+) bearing (\S+) "
    r"speed (\S+) visible (\d) aircraft (\d)$",
    re.M,
)
PAGE = re.compile(
    r"^a2g-radar: t=(\S+) (page|designated|end) mode (\S+) air-ground (\d) contacts (\d+) ground (\d+) "
    r"radar-emitting (\d)$",
    re.M,
)
SELECTED = re.compile(r"^a2g-radar: t=(\S+) selected (0x[0-9a-f]+) (\S+) window (\S+)$", re.M)
TANKS = {"T-72", "T-80", "T-90"}


def runs(output: str) -> list[str]:
    """The output of each `--a2g-radar` run, in order."""
    return re.split(r"^a2g-radar: done at .*$", output, flags=re.M)


def _pages(run: str) -> list[re.Match]:
    return list(PAGE.finditer(run))


def _contacts(run: str, label: str) -> list[re.Match]:
    """The contacts printed after the page line labelled `label`."""
    lines = run.splitlines()
    out, inside = [], False
    for line in lines:
        page = PAGE.match(line)
        if page:
            inside = page.group(2) == label
            continue
        contact = CONTACT.match(line)
        if inside and contact:
            out.append(contact)
    return out


def _nearest_tank_designated(run: str, problems: list[str], what: str) -> None:
    page = _contacts(run, "page")
    tanks = [c for c in page if c.group(2) in TANKS]
    if not tanks:
        problems.append(f"{what}: no tank on the page")
        return
    nearest = min(page, key=lambda c: float(c.group(5)))
    selected = SELECTED.findall(run)
    if not selected or selected[0][1] != nearest.group(1) or selected[0][3] != nearest.group(2):
        problems.append(f"{what}: T should designate the nearest contact {nearest.group(1)} {nearest.group(2)} "
                        f"and the window name it, got {selected[:1]}")
    elif nearest.group(2) not in TANKS:
        problems.append(f"{what}: the nearest contact should be a tank, got {nearest.group(2)}")


def ag_problems(column: str, wingman: str, airfield: str, defended: str) -> list[str]:
    problems = []
    pages = _pages(column)
    if not pages or any(p.group(3) != "A-G" or p.group(4) != "1" for p in pages):
        problems.append("the column run should be on the A-G page throughout")
    if any(p.group(5) != "0" or p.group(7) != "0" for p in pages):
        problems.append("the A-G page turns the air radar off: no air contacts, no emission")
    page = _contacts(column, "page")
    tanks = [c for c in page if c.group(2) in TANKS]
    if len(tanks) < 2 or any(c.group(4) != "dot" for c in tanks):
        problems.append(f"want the column's tanks as dots, got {[(c.group(2), c.group(4)) for c in tanks]}")
    if not any(float(c.group(7)) > 0 for c in tanks):
        problems.append("the moving column should be listed while it moves")
    _nearest_tank_designated(column, problems, "A-G")
    end = SELECTED.findall(column)
    if len(end) < 2 or end[-1][1] != end[0][1]:
        problems.append(f"the designated tank should stay designated to the end, got {end}")
    if any(c.group(9) == "1" for c in CONTACT.finditer(wingman)):
        problems.append("a fighter in free flight must not show on the A-G page")
    squares = [c for c in CONTACT.finditer(airfield) if c.group(9) == "1"]
    if not squares or any(c.group(4) != "square" for c in squares):
        problems.append("an aircraft in an airfield sequence should show as a square")
    defenses = [c for c in _contacts(defended, "page") if int(c.group(3), 16) & 0x1800]
    if not defenses or any(c.group(4) != "x" for c in defenses):
        problems.append(f"SAM and AAA units should be the X, got {[(c.group(2), c.group(4)) for c in defenses]}")
    return problems


def flir_problems(clear: str, fog: str, near: str) -> list[str]:
    problems = []
    pages = _pages(clear)
    if not pages or any(p.group(3) != "IR" or p.group(4) != "0" for p in pages):
        problems.append("the AV-8B run should be on the IR page")
    page = _contacts(clear, "page")
    tanks = [c for c in page if c.group(2) in TANKS]
    if len(tanks) != 9:
        problems.append(f"the IR page should list all nine tanks of the column, got {len(tanks)}")
    if any(float(c.group(5)) > 4.97 for c in page):
        problems.append("the IR page lists ground objects only out to 4.97 nmi")
    _nearest_tank_designated(clear, problems, "IR")
    fogged = [p.group(6) for p in _pages(fog)]
    if not fogged or any(n != "0" for n in fogged):
        problems.append(f"in fog nothing should be listed from 4 nmi, got {fogged}")
    close = _contacts(near, "page")
    if not close or any(float(c.group(5)) > 1.5 for c in close):
        problems.append(f"in fog the page should list only within 1.5 nmi, got {[c.group(5) for c in close]}")
    return problems


def ag_check(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) < 5:
        return [f"{len(parts) - 1} runs finished, want 4"]
    return ag_problems(*parts[:4])


def flir_check(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) < 4:
        return [f"{len(parts) - 1} runs finished, want 3"]
    return flir_problems(*parts[:3])


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="a2g-ag-radar", lane="ai",
            args=["--a2g-radar", "UKR", "QUCOL", "--from", "3", "--seconds", "4"],
            timeout=300,
            then=[
                Step(["--a2g-radar", "UKR", "QUCOL", "--from", "3", "--seconds", "8", "--look-at-ai",
                      "--designate", "none"], timeout=300),
                Step(["--a2g-radar", "UKR", "QUCOL", "--from", "3", "--seconds", "8", "--look-at-ai",
                      "--designate", "none", "--airfield", "nearest-ai"], timeout=300),
                Step(["--a2g-radar", "UKR", "QUCOL", "--from", "6", "--defenses", "3", "3", "--seconds", "3"],
                     timeout=300),
            ],
            expect=[r"^a2g-radar: aircraft F18 radar 1 "],
            check=ag_check,
            notes="F/A-18D A-G page over the Ukraine column: tanks as dots, T nearest first, the air radar off, "
                  "aircraft only in an airfield sequence, SAM and AAA as the X.",
        ),
        Scenario(
            name="a2g-flir", lane="ai",
            args=["--a2g-radar", "UKR", "QUCOL", "--aircraft", "AV8", "--page", "ir", "--from", "4",
                  "--altitude", "3000", "--seconds", "4"],
            timeout=300,
            then=[
                Step(["--a2g-radar", "UKR", "QUCOL", "--aircraft", "AV8", "--page", "ir", "--from", "4",
                      "--altitude", "3000", "--seconds", "4", "--condition", "2"], timeout=300),
                Step(["--a2g-radar", "UKR", "QUCOL", "--aircraft", "AV8", "--page", "ir", "--from", "1.2",
                      "--altitude", "1500", "--seconds", "3", "--condition", "2"], timeout=300),
            ],
            expect=[r"^a2g-radar: aircraft Av8 radar 1 infrared 1 "],
            check=flir_check,
            notes="AV-8B FLIR (IR page) over the column: every tank, T designates the nearest, fog cuts the range "
                  "to a tenth.",
        ),
    ]
