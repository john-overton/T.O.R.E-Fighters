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


def check_pass5_cameras(work, _output):
    lines=(work/'cameras'/'pass5-cameras.txt').read_text().splitlines()
    expected={'UKR-forest-popin','UKR-coast','TVIET-coast','EGY-acacias',
              'UKR-forest-far-1000ft','UKR-forest-far-3000ft','UKR-top-2000ft','EGY-top-2000ft'}
    if {line.split()[0] for line in lines} != expected:
        return ['missing Pass 5 coast/forest/top camera']
    if any(len(line.split())!=10 or not all(math.isfinite(float(v)) for v in line.split()[5:]) for line in lines):
        return ['invalid Pass 5 camera']
    return []


def scenarios():
    return [Scenario(
        name='vegetation-top-crowns', lane='menus',
        args=['--vegetation-art', '{work}/art'], timeout=120,
        outputs=['art/top-crowns.tsv'], check_work=check_top_crowns,
    ), Scenario(
        name='vegetation-stream-convergence', lane='menus',
        args=['--vegetation-preview', '{work}/stream', 'stream-check'],
        env={'TORE_VEGETATION': '1', 'TORE_VEG_LOWPASS_THEATERS': 'UKR',
             'TORE_VEG_LOWPASS_DENSITIES': 'high'}, timeout=240,
        outputs=['stream/stream-check.txt', 'stream/UKR-High-stream.csv'],
        expect=['UKR High: converged=', 'idle=120 frames'],
    ), Scenario(
        name='vegetation-cpu-cameras', lane='menus',
        args=['--vegetation-preview', '{work}/cameras', 'cameras'],
        env={'TORE_VEGETATION': '1'}, timeout=120,
        outputs=['cameras/extra-cameras.txt'], check_work=check_cameras,
    ), Scenario(
        name='vegetation-pass5-cameras', lane='menus',
        args=['--vegetation-preview', '{work}/cameras', 'pass5-cameras'],
        env={'TORE_VEGETATION': '1'}, timeout=240,
        outputs=['cameras/pass5-cameras.txt'], check_work=check_pass5_cameras,
    )]


def check_top_crowns(work, _output):
    rows=(work/'art'/'top-crowns.tsv').read_text().splitlines()[1:]
    if not rows:
        return ['no retail grove crown audit']
    errors=[]
    for row in rows:
        source,count,grove,crown,transparent=row.split('\t')
        if int(count)<2 or not 0<float(crown)<float(grove):
            errors.append(f'{source}: a single crown still spans the grove')
        if not 0<int(transparent)<65536:
            errors.append(f'{source}: missing crown cutouts or forest-floor gaps')
    return errors
