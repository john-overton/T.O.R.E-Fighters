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


def check_grass_cameras(work, _output):
    lines = (work / 'cameras' / 'grass-cameras.txt').read_text().splitlines()
    expected = {'UKR-meadow-10ft', 'UKR-meadow-50ft', 'UKR-meadow-100ft', 'UKR-meadow-200ft',
                'UKR-pine-8deg', 'UKR-pine-25deg', 'UKR-pine-45deg', 'UKR-pine-70deg',
                'UKR-field-edge-50ft', 'UKR-riverbank-50ft',
                'TVIET-grassland-50ft', 'EGY-water-belt-50ft'}
    if {line.split()[0] for line in lines} != expected:
        return ['missing grass comparison camera']
    if any(len(line.split()) != 10 or
           not all(math.isfinite(float(v)) for v in line.split()[5:]) for line in lines):
        return ['invalid grass comparison camera']
    return []


def scenarios():
    return [Scenario(
        name='vegetation-tile-seams', lane='menus',
        args=['--vegetation-preview', '{work}/seams', 'seam-check'],
        env={'TORE_VEGETATION': '1', 'TORE_TERRAIN_SEAMS': '1'}, timeout=600,
        outputs=['seams/seam-metrics.tsv', 'seams/seam-cameras.txt',
                 'seams/camera-check.tsv', 'seams/seam-cost-cameras.txt',
                 'seams/UKR-LAND-line.csv', 'seams/line-check.txt',
                 'seams/UKR-mottle-statistics.csv', 'seams/FRA-mottle-statistics.csv',
                 'seams/BAL-mottle-statistics.csv', 'seams/UKR-texture-band.csv',
                 'seams/UKR-texture-check.txt', 'seams/FRA-texture-check.txt',
                 'seams/BAL-texture-check.txt'],
        expect=['terrain seams: PASS UKR', 'terrain seams: PASS FRA',
                'terrain seams: PASS BAL', 'exact cache and portable limits'],
    ), Scenario(
        name='vegetation-theater-limits', lane='menus',
        args=['--vegetation-preview', '{work}/limits', 'limits-check'],
        env={'TORE_VEGETATION': '1', 'TORE_WORKERS': '4'}, timeout=3600,
        outputs=['limits/theater-limits.tsv', 'limits/limits-check.txt'],
        expect=['vegetation limits: PASS PGU', 'all 16 base theaters and every imported variant'],
    ), Scenario(
        name='vegetation-grass-cameras', lane='menus',
        args=['--vegetation-preview', '{work}/cameras', 'grass-cameras'],
        env={'TORE_VEGETATION': '1', 'TORE_WORKERS': '4'}, timeout=240,
        outputs=['cameras/grass-cameras.txt', 'cameras/grass-lowpass-cameras.txt',
                 'cameras/grass-camera-sites.txt'], check_work=check_grass_cameras,
    ), Scenario(
        name='vegetation-cache-roundtrip', lane='menus',
        args=['--vegetation-preview', '{work}/cache', 'cache-check'],
        env={'TORE_VEGETATION': '1', 'TORE_WORKERS': '4'}, timeout=180,
        outputs=['cache/cache-check.txt'],
        expect=['vegetation cache: PASS', 'menu without vegetation'],
    ), Scenario(
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
        if int(count) < 2 or float(crown) <= 0:
            errors.append(f'{source}: missing individual crowns')
        if source in {'GROVPA.SH', 'GROVPB.SH', 'GROVPC.SH'}:
            if float(grove) != 0:
                errors.append(f'{source}: detached pine top disk still enabled')
        elif not 0 < float(crown) < float(grove):
            errors.append(f'{source}: a single crown still spans the grove')
        if not 0<int(transparent)<65536:
            errors.append(f'{source}: missing crown cutouts or forest-floor gaps')
    return errors
