"""Frame counts, formats, safe areas, repeated pixels, sound, whole radio calls and cut previews."""
import json
import subprocess
import struct
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw
from render import OUT, FPS, run
from edit import cutdown_plan, deliverables, master_plan, END_BADGE, SPF, SPEECH_GRACE


def probe(path):
    return json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-count_frames', '-show_streams', '-show_format', '-of', 'json', str(path)]))


def decoded_hash(path):
    result = subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(path), '-map', '0:v', '-f', 'hash', '-hash', 'sha256', '-'])
    return result.decode().strip()


def loudness(path):
    result = subprocess.run(['ffmpeg', '-hide_banner', '-i', str(path), '-vn', '-af', 'loudnorm=I=-14:TP=-1:LRA=18:print_format=json', '-f', 'null', '-'],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=True)
    return json.JSONDecoder().raw_decode(result.stderr[result.stderr.rfind('{'):])[0]


def faststart(path):
    atoms = []
    with path.open('rb') as f:
        while header := f.read(8):
            length, kind = struct.unpack('>I4s', header)
            used = 8
            if length == 1:
                length = struct.unpack('>Q', f.read(8))[0]
                used = 16
            atoms.append(kind)
            if length == 0:
                break
            assert length >= used
            f.seek(length - used, 1)
    return atoms.index(b'moov') < atoms.index(b'mdat')


def timestamps(path, frames):
    out = subprocess.check_output(['ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_entries', 'frame=best_effort_timestamp_time', '-of', 'csv=p=0', str(path)], text=True)
    ts = [float(line.split(',')[0]) for line in out.splitlines() if line and line[0].isdigit()]
    return len(ts) == frames and max(abs(t - i / FPS) for i, t in enumerate(ts)) < 1e-5


def frames_of(path, numbers, size):
    """Decode chosen frames to RGB arrays. Runs of frames become ranges, so
    long selections stay within ffmpeg's expression limits."""
    runs = []
    for n in sorted(set(numbers)):
        if runs and n == runs[-1][1] + 1:
            runs[-1][1] = n
        else:
            runs.append([n, n])
    select = '+'.join(f'between(n\\,{a}\\,{b})' for a, b in runs)
    raw = subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(path), '-vf', f"select='{select}'", '-fps_mode', 'vfr', '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-'])
    w, h = size
    arr = np.frombuffer(raw, dtype=np.uint8).reshape(-1, h, w, 3)
    return dict(zip(sorted(set(numbers)), arr))


def sheet(path, frames, thumb, title, out):
    """Rows of three frames: before, on and after each requested frame."""
    rows = []
    for cut in frames:
        rows.append([max(0, cut - 1), cut, cut + 1])
    wanted = sorted({f for row in rows for f in row})
    decoded = frames_of(path, wanted, probe_size(path))
    for start in range(0, len(rows), 5):
        chunk = rows[start:start + 5]
        im = Image.new('RGB', (thumb[0] * 3, len(chunk) * (thumb[1] + 26)), (10, 12, 14))
        d = ImageDraw.Draw(im)
        for r, row in enumerate(chunk):
            for c, frame in enumerate(row):
                x, y = c * thumb[0], r * (thumb[1] + 26)
                if frame in decoded:
                    im.paste(Image.fromarray(decoded[frame]).resize(thumb, Image.Resampling.BILINEAR), (x, y))
                d.text((x + 8, y + thumb[1] + 5), f'{title} frame {frame} / {frame / FPS:.3f}s', fill='white')
        im.save(out.with_name(f'{out.stem}-{start // 5 + 1}.jpg'), quality=90)


def probe_size(path):
    v = next(s for s in probe(path)['streams'] if s['codec_type'] == 'video')
    return v['width'], v['height']


def cuts_of(plan):
    """Every frame where a new shot starts in an edit."""
    out, cursor = [], 0
    for shot, first, count in plan:
        out.append(cursor)
        cursor += count
    return out[1:]


def audio_steps(pcm, cuts):
    steps = []
    for cut in cuts:
        at = cut * SPF
        delta = float(np.max(np.abs(pcm[at] - pcm[at - 1])))
        local = float(np.max(np.abs(np.diff(pcm[max(0, at - 2400):at + 2400], axis=0))))
        steps.append({'frame': cut, 'step': round(delta, 5), 'local_max_step': round(local, 5), 'isolated_spike': delta > .06 and delta > .7 * local})
    return steps


def speech_check(name, take='take-a'):
    """Every recorded call in an edit is heard whole or not at all. Checked on
    the mixed speech itself: a kept call matches its source sample for sample
    from its first to its last sound, within its shot plus SPEECH_GRACE; a
    dropped call is silent wherever it would have played."""
    log = json.loads((OUT / 'validation' / f'{name}-speech.json').read_text())
    mixed = np.fromfile(OUT / 'validation' / f'{name}-speech.f32', dtype='<f4')
    for d in log:
        source = np.fromfile(OUT / take / f"{d['clip']}.speech", dtype='<f4')
        a, b = d['window']
        at = d['output_start_sample']
        if d['kept']:
            assert d['start_sample'] >= a and d['end_sample'] <= b + SPEECH_GRACE, d
            whole = source[d['start_sample']:d['end_sample']]
            assert np.allclose(mixed[at:at + len(whole)], whole, atol=1e-7), d
        else:
            stop = d['end_sample'] if d['end_sample'] is not None else len(source)
            lo, hi = max(d['start_sample'], a), min(stop, b)
            assert not mixed[at + lo - d['start_sample']:at + hi - d['start_sample']].any(), d
    return log


def end_hold(path, size, total, end_start):
    """Seconds the finished end card's lines hold still before the edit ends.
    The badge and its sparkles keep twinkling by design, so their square is
    left out of the comparison."""
    w, h = size
    badge, cy = END_BADGE['portrait' if w < h else 'landscape']
    reach = badge * 0.62 + 60
    numbers = list(range(end_start, total))
    decoded = frames_of(path, numbers, size)
    def still(frame):
        a = np.asarray(Image.fromarray(frame).resize((w // 8, h // 8), Image.Resampling.BOX), dtype=np.float32)
        y0, y1 = max(0, int((cy - reach) / 8)), int((cy + reach) / 8) + 1
        x0, x1 = max(0, int((w / 2 - reach) / 8)), int((w / 2 + reach) / 8) + 1
        a[y0:y1, x0:x1] = 0
        return a
    small = {n: still(decoded[n]) for n in numbers}
    last = small[total - 1]
    settled = total - 1
    for n in reversed(numbers):
        if np.abs(small[n] - last).max() > 10:
            break
        settled = n
    return (total - settled) / FPS, settled


def verify(config):
    report = {}
    names = deliverables(config)
    total = config['reel']['duration'] * FPS
    landscape, vertical, cutdown = names['landscape'], names['vertical'], names['cutdown']
    for name, width, height, frames in [(landscape, 1920, 1080, total), (names['webm'], 1920, 1080, total),
                                        (vertical, 1080, 1920, total), (cutdown, 1920, 1080, 900)]:
        path = OUT / name
        p = probe(path)
        v = next(s for s in p['streams'] if s['codec_type'] == 'video')
        a = next(s for s in p['streams'] if s['codec_type'] == 'audio')
        assert (v['width'], v['height'], int(v['nb_read_frames'])) == (width, height, frames), name
        assert v['r_frame_rate'] == '60/1' and v['pix_fmt'] == 'yuv420p', name
        assert v['codec_name'] == ('vp9' if name.endswith('webm') else 'h264'), name
        assert a['codec_name'] == ('opus' if name.endswith('webm') else 'aac'), name
        assert a['sample_rate'] == '48000' and a['channels'] == 2, name
        if name.endswith('mp4'):
            assert faststart(path), f'{name} lacks faststart'
            assert timestamps(path, frames), f'{name} frame timestamps'
        # Opus adds its 6.5 ms decoder pre-roll to the container duration.
        assert abs(float(p['format']['duration']) - frames / FPS) < .04, name
        levels = loudness(path)
        assert -15.5 <= float(levels['input_i']) <= -12.5, (name, levels)
        assert float(levels['input_tp']) <= -1.0, (name, levels)
        run(['ffmpeg', '-v', 'error', '-xerror', '-i', path, '-f', 'null', '-'], OUT / 'validation' / f'{name}.decode.log')
        report[name] = {'frames': frames, 'size': [width, height], 'fps': FPS, 'video': v['codec_name'], 'audio': a['codec_name'],
                        'audio_bit_rate': a.get('bit_rate'), 'duration': p['format']['duration'],
                        'integrated_lufs': float(levels['input_i']), 'true_peak_dbtp': float(levels['input_tp'])}
    first = decoded_hash(OUT / 'landscape-silent.mp4')
    second = decoded_hash(OUT / 'validation/repeat-silent.mp4')
    assert first == second, 'Final encoded video pixels differ'
    report['repeat_encoded'] = {'sha256': first, 'identical': True}
    # Lettering, logos and glyphs: landscape inside 4:3, portrait inside its
    # central square.
    boxes = json.loads((OUT / 'validation/text-boxes.json').read_text())
    errors = []
    for label, x, y, r, b, w, h in boxes:
        left, right, top, bottom = (240, 1680, 54, 1026) if w > h else (48, 1032, 420, 1500)
        if x < left or r > right or y < top or b > bottom:
            errors.append([label, x, y, r, b, w, h])
    (OUT / 'validation/unsafe-text.json').write_text(json.dumps(errors, indent=2) + '\n')
    assert not errors, f'Text outside safe areas: {errors[:8]}'
    report['text_safe'] = {'unique_boxes': len(boxes), 'errors': 0}
    # Cut previews for every edit, before, on and after each cut.
    master_cuts, cutdown_cuts = cuts_of(master_plan(config)), cuts_of(cutdown_plan(config))
    for name, cuts, thumb in [(landscape, master_cuts, (640, 360)), (vertical, master_cuts, (270, 480)), (cutdown, cutdown_cuts, (640, 360))]:
        sheet(OUT / name, cuts, thumb, Path(name).stem, OUT / 'validation' / f'{Path(name).stem}-cuts.jpg')
    report['cuts'] = {'master': master_cuts, 'cutdown': cutdown_cuts}
    # The continuous HUD-to-cockpit pullback, frame by frame.
    cockpit = next(s for s in config['shot'] if s['layout'] == 'cockpit')
    hold, gone = cockpit['tape']
    pull = list(range(cockpit['start'] + hold - 8, cockpit['start'] + gone + 9, 8))
    for name, thumb in [(landscape, (640, 360)), (vertical, (270, 480))]:
        decoded = frames_of(OUT / name, pull, probe_size(OUT / name))
        cols = 4
        im = Image.new('RGB', (thumb[0] * cols, ((len(pull) + cols - 1) // cols) * (thumb[1] + 22)), (10, 12, 14))
        d = ImageDraw.Draw(im)
        for k, n in enumerate(pull):
            x, y = (k % cols) * thumb[0], (k // cols) * (thumb[1] + 22)
            im.paste(Image.fromarray(decoded[n]).resize(thumb, Image.Resampling.BILINEAR), (x, y))
            d.text((x + 6, y + thumb[1] + 4), f'frame {n}', fill='white')
        im.save(OUT / 'validation' / f'{Path(name).stem}-pullback.jpg', quality=90)
    report['pullback_frames'] = pull
    # Pausing stops the very picture that was playing: the freeze capture's
    # scene matches the last moving frame above the transport bar.
    def scene_hash(clip, frame):
        return subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(OUT / 'take-a' / f'{clip}.mkv'), '-vf', f"select='eq(n\\,{frame})',crop=1920:900:0:0",
                                        '-frames:v', '1', '-f', 'hash', '-hash', 'sha256', '-'])
    alt = next(c for c in config['capture'] if c['id'] == 'replay-alt')
    assert scene_hash('replay-alt', alt['frames'] - 1) == scene_hash('replay-freeze', 0), 'Impact freeze jumps to a different picture'
    report['freeze_continuity'] = 'scene above the transport is identical; the real control changes from play to pause'
    # Sound at the edits: no isolated steps, and no phrase cut by an edit.
    for wav, cuts, key in [('master.wav', master_cuts, 'audio_edits'), ('cutdown.wav', cutdown_cuts, 'cutdown_audio_edits')]:
        raw = subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(OUT / wav), '-f', 'f32le', '-acodec', 'pcm_f32le', '-'])
        steps = audio_steps(np.frombuffer(raw, dtype='<f4').reshape(-1, 2), cuts)
        assert not any(s['isolated_spike'] for s in steps), steps
        report[key] = steps
    for name, key in [('master', 'speech_calls'), ('cutdown', 'cutdown_speech_calls')]:
        report[key] = speech_check(name)
    # The music kicks on the first frame of the kick shot in both edits, and
    # rings out before each edit ends.
    end = next(s for s in config['shot'] if s['layout'] == 'end')
    for name, plan, frames in [('master', master_plan(config), total), ('cutdown', cutdown_plan(config), 900)]:
        music = json.loads((OUT / 'validation' / f'{name}-music.json').read_text())
        kick = sum(count for shot, _, count in plan[:[s['id'] for s, _, _ in plan].index(config['music']['kick_shot'])])
        assert music['kick_sample'] == kick * SPF and music['music_end_sample'] <= frames * SPF, (name, music)
        report[f'{name}_music'] = music
    hold_s, settled = end_hold(OUT / landscape, (1920, 1080), total, end['start'])
    hold_v, settled_v = end_hold(OUT / vertical, (1080, 1920), total, end['start'])
    hold_c, settled_c = end_hold(OUT / cutdown, (1920, 1080), 900, 900 - cutdown_plan(config)[-1][2])
    assert min(hold_s, hold_v, hold_c) >= 1.5, (hold_s, hold_v, hold_c)
    report['end_card_hold_seconds'] = {'landscape': round(hold_s, 3), 'portrait': round(hold_v, 3), 'cutdown': round(hold_c, 3),
                                       'settled_frames': [settled, settled_v, settled_c]}
    report['frame_timestamps'] = 'every MP4 frame at exactly n/60 s'
    report['deliverables'] = names
    (OUT / 'validation/report.json').write_text(json.dumps(report, indent=2) + '\n')
    print('All deliverables, repeated pixels, sound, whole radio calls, safe areas and the end-card hold pass.', flush=True)
