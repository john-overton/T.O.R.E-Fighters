#!/usr/bin/env python3
"""One-command reproducible reel. Runtime media is confined to out/reel."""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'out/reel'
APP = ROOT / ('target/debug/tore-app.exe' if os.name == 'nt' else 'target/debug/tore-app')
FPS = 60
VIEWS = {'external': 0, 'hud': 1, 'cockpit': 2, 'replay': 3}
TRANSITIONS = ['cut', 'flash', 'wobble', 'static', 'tape', 'slam', 'power-on']
LAYOUTS = ['boot', 'full', 'aircraft', 'theater', 'cockpit', 'replay', 'end']


def run(args, log=None, env=None):
    print(' '.join(map(str, args)), flush=True)
    if log:
        with Path(log).open('w') as f:
            result = subprocess.run(list(map(str, args)), cwd=ROOT, env=env, stdout=f, stderr=subprocess.STDOUT)
        if result.returncode:
            raise RuntimeError(f'Command failed ({result.returncode}): {log}\n{Path(log).read_text()[-4000:]}')
    else:
        subprocess.run(list(map(str, args)), cwd=ROOT, env=env, check=True)


def beat_frame(music, beats):
    """Output frame of a grid beat counted from the launch downbeat."""
    return music['downbeat_frame'] + beats * FPS * 60 / music['grid_bpm']


def read_config(path):
    config = tomllib.loads(Path(path).read_text())
    reel, music = config['reel'], config['music']
    assert reel['fps'] == FPS and reel['duration'] > 0
    shots = config['shot']
    cursor = 0
    for shot in shots:
        assert shot['start'] == cursor and shot['frames'] > 0, shot
        assert shot['transition'] in TRANSITIONS and shot['layout'] in LAYOUTS, shot
        cursor += shot['frames']
    assert cursor == reel['duration'] * FPS
    assert len({s['id'] for s in shots}) == len(shots)
    assert len(reel['aircraft_roster']) == 14 and len(reel['theater_roster']) == 16
    captures = {c['id']: c for c in config['capture']}
    scenes = {s['id']: s for s in config['scenario']}
    assert len(captures) == len(config['capture'])
    for c in captures.values():
        assert c['map'] == scenes[Path(c['replay']).stem]['map'], c['id']
        assert c.get('view', 'external') in VIEWS, c['id']
    for shot in shots:
        for name in shot.get('clips', []):
            assert name in captures and captures[name]['frames'] == shot['frames'], name
    # Cuts after the launch land within one frame of the music's beat grid.
    beat = FPS * 60 / music['grid_bpm']
    for shot in shots[1:]:
        beats = (shot['start'] - music['downbeat_frame']) / beat
        assert abs(beats - round(beats)) * beat <= 1.0, (shot['id'], beats)
    by_id = {s['id']: s for s in shots}
    assert music['kick_shot'] in by_id
    segments = config['cutdown']['segments']
    assert sum(n for _, _, n in segments) == 900
    for shot_id, first, count in segments:
        shot = by_id[shot_id]
        assert first >= 0 and count > 0
        assert shot['layout'] == 'end' or first + count <= shot['frames'], (shot_id, first, count)
    return config


def fingerprint(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as f:
        while chunk := f.read(1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def events(name):
    return [json.loads(line) for line in (OUT / 'events' / name / 'log.jsonl').read_text().splitlines()]


def record(config, env, install):
    run(['cargo', 'build', '--locked', '-p', 'tore-app'], OUT / 'logs/build.log', env)
    media_hash = hashlib.sha256()
    # A directory or disc folder is accepted by the original importer. Hash the
    # source files that determine this import, including optional music archives.
    for name in ['FA.EXE', 'FA_1.LIB', 'FA_2.LIB', 'FA_4B.LIB', 'FA_4D.LIB', 'SETUP.ESA']:
        for p in sorted(p for p in install.iterdir() if p.name.upper() == name):
            media_hash.update(name.encode())
            media_hash.update(fingerprint(p).encode())
    identity = media_hash.hexdigest()
    receipt = OUT / 'profile/source.sha256'
    if not receipt.exists() or receipt.read_text() != identity:
        run([APP, '--import', install, '--import-only'], OUT / 'logs/import.log', env)
        receipt.write_text(identity)
    run([APP, '--no-audio', '--validate-maps'], OUT / 'logs/maps.log', env)
    for scene in config['scenario']:
        path = OUT / 'replays' / f"{scene['id']}.tore-replay"
        args = ['--no-audio', '--aircraft', scene['aircraft'], '--theater', scene['map'], '--ai-probe-ticks', str(scene['ticks']), '--probe-flight-model', 'researched', *scene['args'], '--verify-render']
        key = json.dumps([identity, args, scene['time_of_day'], scene.get('wind')], sort_keys=True)
        marker = path.with_suffix('.recipe.json')
        if path.exists() and marker.exists() and marker.read_text() == key:
            continue
        fresh = path.with_suffix('.fresh.tore-replay')
        fresh.unlink(missing_ok=True)
        scene_env = {**env, 'TORE_WEATHER_TIME': scene['time_of_day']}
        if 'wind' in scene:
            scene_env['TORE_WIND'] = scene['wind']
        run([APP, *args, '--record-mission', fresh], OUT / 'logs' / f"{scene['id']}.log", scene_env)
        if 'missing=0 differing=0' not in (OUT / 'logs' / f"{scene['id']}.log").read_text():
            raise RuntimeError(f"Recording failed reconstruction: {scene['id']}")
        fresh.replace(path)
        marker.write_text(key)
        run([APP, '--recording-log', path, '--out', OUT / 'events' / scene['id']], OUT / 'logs' / f"{scene['id']}-export.log", env)
    # Retail score phrases and their cue sheets, for the editorial music bed.
    run([APP, '--reel-music', OUT / 'music', *config['music'].get('effects', [])], OUT / 'logs/music.log', env)
    for name in [config['music']['bed'], config['music']['ending'], *config['music'].get('effects', [])]:
        if not (OUT / 'music' / f'{name}.f32').exists():
            raise RuntimeError(f'Imported media lacks the music phrase {name}')
    # Verify every requested kill and flare against evidence, never manufacture it.
    for claim in config.get('event_check', []):
        if not any(e.get('kind') == claim['kind'] and e.get('tick') == claim['tick'] and e.get('subject') == claim['subject'] for e in events(claim['scenario'])):
            raise RuntimeError(f'Required recorded event is absent: {claim}')
    launch = events('launch-calm')
    poses = [e for e in launch if e.get('type') == 'sample' and e['id'] == 0 and e['t'] <= 15]
    assert poses and max(abs(e['pos_ft'][0] - poses[0]['pos_ft'][0]) for e in poses) < 0.05, 'Runway centerline drift'
    pursuit = events('pursuit-close-mig21')
    assert any(e.get('type') == 'sample' and e['id'] == 1 and 71 <= e['t'] <= 75 and e['att_deg'][2] > 25 for e in pursuit), 'Pursuit lacks a recorded right bank'
    near = {e['id']: e for e in pursuit if e.get('type') == 'sample' and e['t'] == 74}
    assert sum((a - b) ** 2 for a, b in zip(near[1]['pos_ft'], near[2]['pos_ft'])) < 1000 ** 2, 'Pursuit target is not close enough'
    assert any(e.get('kind') == 'ai.target' and e.get('subject') == 1 and e.get('object') == 2 for e in pursuit), 'The bandit was not actually selected'
    night = next(c for c in config['capture'] if c['id'] == 'night')
    span = (night['in_tick'], night['in_tick'] + 2 * (night['frames'] - 1))
    flares = [e for e in events('dogfight') if e.get('kind') == 'combat.countermeasure' and e.get('fields', {}).get('decoy') == 'flare' and span[0] <= e['tick'] <= span[1]]
    assert any(e.get('subject') == night['anchor'] for e in flares), 'Night shot lacks actual flares'


def pchip(xs, ys):
    """Monotone cubic Hermite slopes: smooth through keys without overshoot."""
    n = len(xs)
    if n == 1:
        return [0.]
    h = [xs[i + 1] - xs[i] for i in range(n - 1)]
    d = [(ys[i + 1] - ys[i]) / h[i] for i in range(n - 1)]
    if n == 2:
        return [d[0], d[0]]
    m = [0.] * n
    for i in range(1, n - 1):
        if d[i - 1] * d[i] > 0:
            w1, w2 = 2 * h[i] + h[i - 1], h[i] + 2 * h[i - 1]
            m[i] = (w1 + w2) / (w1 / d[i - 1] + w2 / d[i])
    for end, (h0, h1, d0, d1) in [(0, (h[0], h[1], d[0], d[1])), (n - 1, (h[-1], h[-2], d[-1], d[-2]))]:
        slope = ((2 * h0 + h1) * d0 - h0 * d1) / (h0 + h1)
        if slope * d0 <= 0:
            slope = 0.
        elif d0 * d1 <= 0 and abs(slope) > 3 * abs(d0):
            slope = 3 * d0
        m[end] = slope
    return m


def interpolate(keys, values, i, mode):
    xs = [k['frame'] for k in keys]
    right = next((k for k in range(1, len(keys)) if xs[k] >= i), len(keys) - 1)
    a, b = right - 1, right
    h = xs[b] - xs[a]
    t = (i - xs[a]) / h
    if mode == 'smoothstep':
        s = t * t * (3 - 2 * t)
        return values[a] + (values[b] - values[a]) * s
    if mode == 'linear':
        return values[a] + (values[b] - values[a]) * t
    m = pchip(xs, values)
    return ((2 * t ** 3 - 3 * t ** 2 + 1) * values[a] + (t ** 3 - 2 * t ** 2 + t) * h * m[a]
            + (-2 * t ** 3 + 3 * t ** 2) * values[b] + (t ** 3 - t ** 2) * h * m[b])


def aim_target(eye, point, fov, sx, sy, aspect=16 / 9):
    """A look target that puts `point` at normalised screen (sx, sy), with
    x right and y up in -1..1, for the director's roll-free camera."""
    v = [p - e for p, e in zip(point, eye)]
    dist = math.sqrt(sum(c * c for c in v))
    ty = math.tan(math.radians(fov / 2))
    tx = ty * aspect
    yaw, pitch = math.atan2(v[0], v[2]), math.atan2(v[1], math.hypot(v[0], v[2]))
    for _ in range(40):
        f = (math.sin(yaw) * math.cos(pitch), math.sin(pitch), math.cos(yaw) * math.cos(pitch))
        r = (math.cos(yaw), 0.0, -math.sin(yaw))
        u = (-math.sin(yaw) * math.sin(pitch), math.cos(pitch), -math.cos(yaw) * math.sin(pitch))
        depth = sum(a * b for a, b in zip(v, f))
        x = sum(a * b for a, b in zip(v, r)) / depth / tx
        y = sum(a * b for a, b in zip(v, u)) / depth / ty
        yaw += math.atan((x - sx) * tx) * 0.9
        pitch += math.atan((y - sy) * ty) * 0.9
    return [e + c * dist for e, c in zip(eye, f)]


def zoom_of(fov):
    return math.log(math.tan(math.radians(30)) / math.tan(math.radians(fov / 2)))


def fov_of(zoom):
    return 2 * math.degrees(math.atan(math.tan(math.radians(30)) / math.exp(zoom)))


def frame_plan(capture):
    keys = capture['camera']
    assert keys[0]['frame'] == 0 and keys[-1]['frame'] == capture['frames'] - 1
    assert all(b['frame'] > a['frame'] for a, b in zip(keys, keys[1:]))
    mode = capture.get('interp', 'smoothstep')
    channels = [[k[name][axis] for k in keys] for name in ('eye', 'target') for axis in range(3)]
    zooms = [zoom_of(k['fov']) for k in keys]
    # `aim` keys place the target point on screen instead of centring it.
    aims = [[k['aim'][axis] for k in keys] for axis in range(2)] if 'aim' in keys[0] else None
    assert aims is None or all('aim' in k for k in keys), capture['id']
    clock = capture.get('clock', [-1, -1])
    view = VIEWS[capture.get('view', 'external')]
    rate = 0 if 'freeze_tick' in capture else capture.get('speed', 1)
    eye_tick = capture.get('eye_tick', -1)
    rows = []
    # `tail` frames continue the recording past the shot, for its sound only.
    for i in range(capture['frames'] + capture.get('tail', 0)):
        exact = next((k for k in keys if k['frame'] == min(i, keys[-1]['frame'])), None)
        if exact:
            values = [exact[name][axis] for name in ('eye', 'target') for axis in range(3)]
            fov = exact['fov']
            aim = exact.get('aim')
        else:
            values = [interpolate(keys, channel, i, mode) for channel in channels]
            fov = fov_of(interpolate(keys, zooms, i, mode))
            aim = [interpolate(keys, channel, i, mode) for channel in aims] if aims else None
        if aim:
            values[3:6] = aim_target(values[:3], values[3:6], fov, *aim)
        tick = capture.get('freeze_tick', capture['in_tick'] + i * capture.get('speed', 1) * 2)
        minutes = round(clock[0] + (clock[-1] - clock[0]) * i / max(capture['frames'] - 1, 1))
        row = [tick, capture.get('anchor', 0), *(round(v, 6) for v in values), round(fov, 6), minutes, view, rate, eye_tick]
        rows.append(' '.join(map(str, row)))
    return '\n'.join(rows) + '\n'


def capture(config, env, only=None):
    selected = [c for c in config['capture'] if not only or c['id'] in only.split(',')]
    for take in ['take-a', 'take-b']:
        folder = OUT / take
        folder.mkdir(exist_ok=True)
        for clip in selected:
            plan = OUT / 'plans' / f"{clip['id']}.txt"
            plan.write_text(frame_plan(clip))
            flags = ['--hud-layer'] if clip.get('hud_layer') else []
            run([APP, '--reel-render', OUT / clip['replay'], plan, folder / f"{clip['id']}.mkv", *flags], OUT / 'logs' / f"{take}-{clip['id']}.log", env)

    def hashes(p):
        return [line.split(',')[-1].strip() for line in p.read_text().splitlines() if not line.startswith('#')]
    receipt = OUT / 'validation/capture-repeat.json'
    results = json.loads(receipt.read_text()) if only and receipt.exists() else {}
    for clip in selected:
        a, b = [OUT / take / clip['id'] for take in ['take-a', 'take-b']]
        total = clip['frames'] + clip.get('tail', 0)
        frames = hashes(a.with_suffix('.sha256'))
        assert frames == hashes(b.with_suffix('.sha256')) and len(frames) == total, f"Visual nondeterminism: {clip['id']}"
        entry = {'frames': clip['frames'], 'rgba_sha256': fingerprint(a.with_suffix('.sha256'))}
        for suffix in ['.f32', '.speech', '.speech-starts', '.radio']:
            assert fingerprint(a.with_suffix(suffix)) == fingerprint(b.with_suffix(suffix)), f"Audio nondeterminism: {clip['id']}{suffix}"
            entry[suffix.lstrip('.')] = fingerprint(a.with_suffix(suffix))
        assert a.with_suffix('.f32').stat().st_size == total * 800 * 8
        if clip.get('hud_layer'):
            layer = hashes(a.with_suffix('.hud.sha256'))
            assert layer == hashes(b.with_suffix('.hud.sha256')) and len(layer) == total, f"HUD layer nondeterminism: {clip['id']}"
            entry['hud_layer_sha256'] = fingerprint(a.with_suffix('.hud.sha256'))
        entry['identical'] = True
        results[clip['id']] = entry
    receipt.write_text(json.dumps(results, indent=2) + '\n')
    print(f'Identical GPU frames, layers and PCM in two consecutive captures: {sum(c["frames"] for c in selected)} frames', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('install', type=Path)
    parser.add_argument('--stage', choices=['all', 'record', 'capture', 'edit', 'verify'], default='all')
    parser.add_argument('--only', help='Comma-separated capture ids, development only')
    parser.add_argument('--shots', type=Path, default=ROOT / 'tools/reel/shots.toml')
    args = parser.parse_args()
    os.chdir(ROOT)
    for name in ['logs', 'replays', 'events', 'profile', 'plans', 'validation', 'stills', 'music', 'parts']:
        (OUT / name).mkdir(parents=True, exist_ok=True)
    config = read_config(args.shots)
    env = {k: v for k, v in os.environ.items() if not k.startswith('TORE_')}
    env['TORE_DATA_DIR'] = str(OUT / 'profile')
    install = args.install.resolve(strict=True)
    if args.stage in ['all', 'record']:
        record(config, env, install)
    if args.stage in ['all', 'capture']:
        capture(config, env, args.only)
    if args.stage in ['all', 'edit']:
        from edit import edit
        edit(config)
    if args.stage in ['all', 'verify']:
        from verify import verify
        verify(config)


if __name__ == '__main__':
    main()
