"""Compare exported F/A-XX poses to the behavior contract using bounded data readers."""
import argparse
from collections import Counter
import itertools
import json
import math
import os
from pathlib import Path
import subprocess

from export_faxx import DONORS, FLAPS, ROOT
from inspect_shape_effects import inspect


def key(face):
    return json.dumps([face['positions'], face['uv'], face['colors']])


def project(executable, path, states=None):
    report, _ = inspect(path.read_bytes())
    aliases = {name: address for address, name in report['aliases'].items()}
    args = [str(executable), str(path)] + [f'{aliases[k]:x}={v}' for k,v in (states or {}).items()]
    return json.loads(subprocess.check_output(args, env=dict(os.environ, TORE_EXPORT_BRANCHES='1')))


def validate(donors, exported):
    subprocess.run(['cargo', 'build', '--locked', '-q', '-p', 'tore-extract', '--example', 'shape_json', '--target-dir', str(ROOT/'target')], cwd=ROOT, check=True)
    executable = ROOT/'target/debug/examples'/('shape_json.exe' if os.name == 'nt' else 'shape_json')
    stem = 'FAXX' if (exported/'FAXX.SH').is_file() else 'F22'
    if stem == 'FAXX':
        subprocess.run(['cargo', 'run', '--locked', '-q', '-p', 'tore-extract', '--example',
                        'check_faxx_pt', '--', str(donors/'F22.PT'), str(exported/'FAXX.PT')], cwd=ROOT, check=True)
        for suffix in ['_B.SH','_D.SH','_S.SH']:
            assert (donors/('F22'+suffix)).read_bytes() == (exported/(stem+suffix)).read_bytes()
        assert not any((exported/name).exists() for name in DONORS)
    comparisons = 0
    for gear, flap, rudder, hook in itertools.product([0,1], [0,-1], [-1,0,1], [0,1]):
        states = {'_PLgearDown': gear, '_PLgearPos': 0}
        original = project(executable, donors/'F22.SH', states)
        actual = project(executable, exported/(stem+'.SH'), dict(states, _PLleftFlap=flap,
                         _PLrightFlap=flap, _PLrudder=rudder, _PLhook=hook))
        expected = []
        for face in original:
            if face['address'] in DONORS['F22.SH'][1]:
                continue
            if face['address'] not in FLAPS:
                expected.append(face)
                continue
            side = 1 if sum(p[0] for p in face['positions']) > 0 else -1
            midpoint = .4 if flap == -1 else 0.
            angles = [midpoint-.6, midpoint+.6] if side == rudder else [midpoint]
            for angle in angles:
                points = [[float(round(x)), float(round(-22+(y+22)*math.cos(angle)-z*math.sin(angle))),
                           float(round((y+22)*math.sin(angle)+z*math.cos(angle)))]
                          for x,y,z in face['positions']]
                expected.append(dict(face, positions=points))
        assert not any(f['address'] in DONORS['F22.SH'][1] for f in actual), 'fin or decal draw survived'
        expected_count = Counter(map(key, expected))
        actual_count = Counter(map(key, actual))
        missing = expected_count - actual_count
        extra = list((actual_count-expected_count).elements())
        assert not missing, f'geometry mismatch: gear/flap/rudder/hook={gear,flap,rudder,hook}'
        assert len(extra) == (12 if hook else 0), f'unexpected faces: {gear,flap,rudder,hook}'
        if hook:
            hook_faces = [json.loads(k) for k in extra]
            assert min(p[2] for f in hook_faces for p in f[0]) == -23
            assert all(f[2] == [55]*4 for f in hook_faces)
        comparisons += 1
    for name in ['F22_A.SH','F22_C.SH']:
        original = project(executable, donors/name)
        actual = project(executable, exported/name.replace('F22',stem,1))
        assert Counter(map(key,actual)) == Counter(key(f) for f in original if f['address'] not in DONORS[name][1])
    result = {'pose_comparisons': comparisons, 'damage_bodies': 2, 'identity': stem,
              'method': 'bounded static SH data projection, no original module execution',
              'original_game_tested': False, 'kapset_tested': False}
    (exported/'validation.json').write_text(json.dumps(result, indent=2)+'\n')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--donors', type=Path, required=True)
    parser.add_argument('--export', dest='exported', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(validate(args.donors.resolve(), args.exported.resolve()), indent=2))


if __name__ == '__main__':
    main()
