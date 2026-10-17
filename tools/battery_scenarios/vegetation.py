"""CPU-only vegetation camera coverage. Never creates a window or GPU adapter."""
import math
from battery import Scenario


def check_cameras(work, _output):
    lines = (work / 'cameras' / 'extra-cameras.txt').read_text().splitlines()
    expected = {'UKR-field-edge-pass4', 'UKR-mountain-slope-pass4',
                'EGY-delta-candidate-pass4', 'TVIET-jungle-pass4'}
    if {line.split()[0] for line in lines} != expected:
        return ['missing vegetation terrain-class camera']
    for line in lines:
        words = line.split()
        if len(words) != 10 or not all(math.isfinite(float(x)) for x in words[5:]):
            return ['invalid vegetation camera coordinates']
    return []


def scenarios():
    return [Scenario(
        name='vegetation-cpu-cameras', lane='menus',
        args=['--vegetation-preview', '{work}/cameras', 'cameras'],
        env={'TORE_VEGETATION': '1'}, timeout=120,
        outputs=['cameras/extra-cameras.txt'], check_work=check_cameras,
    )]
