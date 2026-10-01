"""Deterministic CRT finish, tape HUD, titles, reframing and the offline edit."""
from __future__ import annotations
from concurrent.futures import ProcessPoolExecutor
from functools import lru_cache
import hashlib
import json
import math
from pathlib import Path
import subprocess
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont
from render import ROOT, OUT, FPS, run

PHOSPHOR = (126, 255, 158)
AMBER = (255, 186, 74)
ORANGE = (255, 132, 38)
WHITE = (236, 238, 228)
INK = (6, 9, 11)
FONTS = ROOT / 'tools/reel/fonts'
WORKERS = 10
CHUNK = 40
LANDSCAPE, PORTRAIT = (1920, 1080), (1080, 1920)


# ---------------------------------------------------------------- lettering

@lru_cache(maxsize=64)
def font(size):
    return ImageFont.truetype(str(FONTS / 'LiberationMono-Bold.ttf'), size)


@lru_cache(maxsize=1024)
def glyph(words, size, color):
    """Bitmap lettering: rasterised at a third of its size, then nearest-neighbour
    tripled, as a low-resolution character generator would draw it."""
    small = max(7, size // 3)
    f = font(small)
    box = f.getbbox(words)
    im = Image.new('RGBA', (max(1, box[2] - box[0] + 2), box[3] - box[1] + 3))
    ImageDraw.Draw(im).text((1 - box[0], 1 - box[1]), words, font=f, fill=color)
    return im.resize((im.width * 3, im.height * 3), Image.Resampling.NEAREST)


class Canvas:
    """An RGBA overlay drawn over a frame. Records every lettering extent for
    the safe-area audit."""
    def __init__(self, size, boxes):
        self.im = Image.new('RGBA', size, (0, 0, 0, 0))
        self.draw = ImageDraw.Draw(self.im)
        self.boxes = boxes

    def text(self, words, x, y, size=30, color=WHITE, anchor='left', limit=None, shadow=True):
        g = glyph(words, size, color)
        if limit and g.width > limit:
            for smaller in range(size - 1, 14, -1):
                g = glyph(words, smaller, color)
                if g.width <= limit:
                    break
        if anchor == 'center':
            x -= g.width / 2
        elif anchor == 'right':
            x -= g.width
        x, y = round(x), round(y)
        if shadow:
            dark = Image.new('RGBA', g.size, (0, 0, 0, 0))
            dark.putalpha(g.getchannel('A').point(lambda v: v * 3 // 4))
            self.im.alpha_composite(dark, (x + 3, y + 3))
        self.im.alpha_composite(g, (x, y))
        self.boxes.append([words, x, y, x + g.width, y + g.height, *self.im.size])
        return g.width, g.height

    def stamp(self, art, x, y, label):
        x, y = round(x), round(y)
        self.im.alpha_composite(art, (x, y))
        self.boxes.append([label, x, y, x + art.width, y + art.height, *self.im.size])

    def panel(self, box, alpha=150, rule=AMBER):
        x0, y0, x1, y1 = box
        self.draw.rectangle(box, fill=(4, 8, 10, alpha))
        self.draw.rectangle((x0, y0, x0 + 3, y1), fill=(*rule, 255))

    def array(self):
        return np.asarray(self.im, dtype=np.float32) * (1 / 255)


@lru_cache(maxsize=8)
def brand(kind, width):
    filename = 'tore-fighters-logo.png' if kind == 'patch' else 'tore-icon-fullsize.png'
    im = Image.open(ROOT / 'docs/images' / filename).convert('RGBA')
    return im.resize((width, round(im.height * width / im.width)), Image.Resampling.LANCZOS)


@lru_cache(maxsize=4)
def platforms(height):
    """Hand-drawn Windows, Linux and Apple glyphs for the supported desktop
    platforms, in the reel's lettering colours. Drawn at four times size."""
    k = 4
    unit = height * k / 120
    def box(x0, y0, x1, y1, dx):
        return (round((x0 + dx) * unit), round(y0 * unit), round((x1 + dx) * unit), round(y1 * unit))
    glyph_w, gap = 100, 70
    im = Image.new('RGBA', (round((3 * glyph_w + 2 * gap) * unit), round(120 * unit)), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    light = (*WHITE, 255)
    # Windows: four panes.
    for x0, y0 in [(4, 14), (54, 14), (4, 64), (54, 64)]:
        d.rectangle(box(x0, y0, x0 + 44, y0 + 44, 0), fill=light)
    # Linux: the penguin, with amber beak and feet.
    dx = glyph_w + gap
    dark, rim, amber = (30, 34, 38, 255), light, (*AMBER, 255)
    for grow, colour in [(3, rim), (0, dark)]:
        d.ellipse(box(14 - grow, 30 - grow, 86 + grow, 112 + grow, dx), fill=colour)
        d.ellipse(box(26 - grow, 2 - grow, 74 + grow, 52 + grow, dx), fill=colour)
        d.ellipse(box(6 - grow, 52 - grow, 30 + grow, 96 + grow, dx), fill=colour)
        d.ellipse(box(70 - grow, 52 - grow, 94 + grow, 96 + grow, dx), fill=colour)
    d.ellipse(box(28, 44, 72, 110, dx), fill=light)
    d.ellipse(box(35, 14, 48, 31, dx), fill=light)
    d.ellipse(box(52, 14, 65, 31, dx), fill=light)
    d.ellipse(box(40, 20, 46, 29, dx), fill=dark)
    d.ellipse(box(55, 20, 61, 29, dx), fill=dark)
    d.ellipse(box(38, 29, 62, 42, dx), fill=amber)
    d.ellipse(box(14, 104, 48, 120, dx), fill=amber)
    d.ellipse(box(52, 104, 86, 120, dx), fill=amber)
    # Apple: two lobes, a bite and a leaf.
    dx = 2 * (glyph_w + gap)
    apple = Image.new('L', im.size, 0)
    a = ImageDraw.Draw(apple)
    a.ellipse(box(6, 34, 62, 114, dx), fill=255)
    a.ellipse(box(38, 34, 94, 114, dx), fill=255)
    a.ellipse(box(41, 22, 59, 42, dx), fill=0)
    a.ellipse(box(42, 106, 58, 122, dx), fill=0)
    a.ellipse(box(78, 50, 112, 82, dx), fill=0)
    leaf = Image.new('L', (round(20 * unit), round(36 * unit)), 0)
    ImageDraw.Draw(leaf).ellipse((0, 0, leaf.width - 1, leaf.height - 1), fill=255)
    leaf = leaf.rotate(-38, expand=True, resample=Image.Resampling.BICUBIC)
    apple.paste(255, (round((dx + 44) * unit), round(-2 * unit)), leaf)
    im.paste(light, (0, 0), apple)
    return im.resize((im.width // k, im.height // k), Image.Resampling.LANCZOS)


# ------------------------------------------------------------ pixel helpers

def to_f(im):
    return np.asarray(im, dtype=np.float32) * (1 / 255)


def to_u8(a):
    return (np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8)


def box_h(a, radius):
    """Horizontal box blur by cumulative sums, edges clamped."""
    if radius < 1:
        return a
    pad = np.concatenate([np.repeat(a[:, :1], radius + 1, axis=1), a, np.repeat(a[:, -1:], radius, axis=1)], axis=1)
    c = np.cumsum(pad, axis=1, dtype=np.float32)
    return (c[:, 2 * radius + 1:] - c[:, :-2 * radius - 1]) * (1 / (2 * radius + 1))


def soft_h(a, radius):
    """Three box passes approximate a Gaussian of about this radius."""
    r = max(1, round(radius / 1.7))
    return box_h(box_h(box_h(a, r), r), r)


def shift_h(a, pixels):
    pixels = int(round(pixels))
    if pixels == 0:
        return a
    out = np.empty_like(a)
    if pixels > 0:
        out[:, pixels:] = a[:, :-pixels]
        out[:, :pixels] = a[:, :1]
    else:
        out[:, :pixels] = a[:, -pixels:]
        out[:, pixels:] = a[:, -1:]
    return out


def glow(a, radius, scale=4):
    """Gaussian glow of an RGB float image, computed at reduced size."""
    h, w = a.shape[:2]
    small = Image.fromarray(to_u8(a)).resize((w // scale, h // scale), Image.Resampling.BOX)
    small = small.filter(ImageFilter.GaussianBlur(radius / scale))
    return to_f(small.resize((w, h), Image.Resampling.BILINEAR))


def luma(a):
    return a[..., 0] * 0.299 + a[..., 1] * 0.587 + a[..., 2] * 0.114


@lru_cache(maxsize=4)
def vignette(size):
    w, h = size
    y, x = np.ogrid[-1:1:complex(h), -1:1:complex(w)]
    ax, ay = w / max(w, h), h / max(w, h)
    r = np.sqrt((x * x * ax * ax + y * y * ay * ay) / (ax * ax + ay * ay))
    return np.clip(1 - 0.16 * r ** 2.6, 0, 1).astype(np.float32)[..., None]


@lru_cache(maxsize=4)
def scan_profile(height):
    rows = np.array([1.0, 0.955, 0.89], dtype=np.float32)[np.arange(height) % 3]
    return rows[:, None, None]


@lru_cache(maxsize=4)
def grain_bank(size):
    w, h = size
    rng = np.random.default_rng(1996)
    return rng.standard_normal((6, h, w), dtype=np.float32)


def grain(size, index, salt=0):
    bank = grain_bank(size)
    tile = bank[(index * 5 + salt) % len(bank)]
    return np.roll(tile, ((index * 7919 + salt * 131) % size[1], (index * 104729 + salt * 31) % size[0]), axis=(0, 1))


# ----------------------------------------------------------------- reframing

def fit(im, size):
    """Scale a source frame to cover `size`, centred."""
    w, h = size
    sw, sh = im.size
    scale = max(w / sw, h / sh)
    nw, nh = round(sw * scale), round(sh * scale)
    im = im.resize((nw, nh), Image.Resampling.LANCZOS) if (nw, nh) != im.size else im
    return im.crop(((nw - w) // 2, (nh - h) // 2, (nw - w) // 2 + w, (nh - h) // 2 + h))


def keyed(keys, i, default):
    if not keys:
        return default
    for (fa, va), (fb, vb) in zip(keys, keys[1:]):
        if fa <= i <= fb:
            t = (i - fa) / max(1, fb - fa)
            t = t * t * (3 - 2 * t)
            return va + (vb - va) * t
    return keys[0][1] if i < keys[0][0] else keys[-1][1]


def band(im, cx, cy, zoom, top=540):
    """Portrait showcase: a landscape band cut around the subject (never
    enlarged) over a darkened, blurred fill of the same picture."""
    w, h = round(1920 / zoom), round(1080 / zoom)
    x = min(max(round(cx * 1920 - w / 2), 0), 1920 - w)
    y = min(max(round(cy * 1080 - h / 2), 0), 1080 - h)
    strip = to_f(im.crop((x, y, x + w, y + h)).resize((1080, 608), Image.Resampling.LANCZOS))
    fill = to_f(fit(im, (270, 480)).resize(PORTRAIT, Image.Resampling.BILINEAR)) * 0.28 + 0.01
    fill[top - 4:top + 612] = 0.0
    fill[top:top + 608] = strip
    return fill


def portrait(im, cx):
    """A deliberate 9:16 crop of the landscape picture around a moving centre."""
    w = 608
    x = min(max(round(cx * 1920 - w / 2), 0), 1920 - w)
    return im.crop((x, 0, x + w, 1080)).resize(PORTRAIT, Image.Resampling.LANCZOS)


def square_window(cx):
    x = min(max(round(cx * 1920 - 540), 0), 1920 - 1080)
    return (x, 0, x + 1080, 1080)


def square(im, cx):
    """Portrait gameplay: a full-resolution 1080 square cut around the subject,
    in the portrait's central square, over a darkened blur of the picture."""
    fill = to_f(fit(im, (135, 240)).resize(PORTRAIT, Image.Resampling.BILINEAR)) * 0.26 + 0.01
    fill[414:1506] = 0.0
    fill[420:1500] = to_f(im.crop(square_window(cx)))
    return fill


# ----------------------------------------------------------- tape recording

def tape(a, hud, strength, index, frame_in_shot):
    """An old videotape recording of the HUD camera: soft luma with edge
    ringing, smeared and delayed chroma, colour fringing, grain, dropouts,
    line jitter, a tracking band and head-switching at the bottom."""
    if strength <= 0:
        return a
    h, w = a.shape[:2]
    rng = np.random.default_rng(9000 + index)
    src = a
    if hud is not None:
        # The recorder sees the symbols bright, with a soft halo; screen-blended
        # so a bright sky cannot clip them to white.
        sym = np.clip(glow(hud[..., :3], 7) * 0.55 + hud[..., :3] * 0.18, 0, 1)
        a = 1 - (1 - np.clip(a, 0, 1)) * (1 - sym)
    y = luma(a)
    i_ = a[..., 0] * 0.596 - a[..., 1] * 0.274 - a[..., 2] * 0.322
    q_ = a[..., 0] * 0.211 - a[..., 1] * 0.523 + a[..., 2] * 0.312
    yb = soft_h(y, 2.2)
    y2 = yb + 0.4 * (yb - soft_h(yb, 6))
    i2 = shift_h(soft_h(i_, 11), 5) * 0.74 + 0.012
    q2 = shift_h(soft_h(q_, 11), 5) * 0.74 - 0.004
    # Lifted blacks and a soft shoulder: tape never reaches black or full white.
    y2 = 0.06 + 0.84 * y2
    y2 = y2 - 0.35 * np.clip(y2 - 0.72, 0, None) ** 1.5
    y2 = y2 + grain((w, h), index, 3) * 0.045 + soft_h(grain((w, h), index, 4), 3) * 0.03
    i2 = i2 + soft_h(grain((w, h), index, 5), 7) * 0.025
    out = np.stack([y2 + 0.956 * i2 + 0.621 * q2, y2 - 0.272 * i2 - 0.647 * q2, y2 - 1.106 * i2 + 1.703 * q2], axis=-1)
    out[..., 0] = shift_h(out[..., 0], 3)
    out[..., 2] = shift_h(out[..., 2], -3)
    # Time-base error: each line slips a little, more in a slowly rolling band.
    jitter = soft_h(rng.standard_normal((1, h)), 4)[0] * 1.6
    band_y = (frame_in_shot * 9 + 760) % (h + 260) - 130
    rows = np.arange(h)
    band = np.exp(-((rows - band_y) / 34.0) ** 2)
    jitter += band * rng.standard_normal(h) * 9
    lift = band[:, None] * 0.12
    # Head switching: the last lines tear sideways.
    tear = np.clip((rows - (h - 18)) / 18, 0, 1)
    jitter += tear * (14 + rng.random() * 8)
    idx = np.clip(np.arange(w)[None, :] - np.round(jitter)[:, None].astype(np.int32), 0, w - 1)
    out = out[rows[:, None], idx]
    noise_band = band[:, None, None] * np.abs(grain((w, h), index, 6))[..., None] * 0.22
    out = out + lift[..., None] * 0.6 + noise_band + tear[:, None, None] * 0.08
    # Dropouts: short white streaks where the oxide flaked.
    for _ in range(rng.integers(0, 3)):
        yy, xx, ln = int(rng.integers(0, h)), int(rng.integers(0, w)), int(rng.integers(30, 260))
        out[yy:yy + 2, xx:xx + ln] = out[yy:yy + 2, xx:xx + ln] * 0.3 + 0.75
    return src + (out - src) * strength


def tape_strength(shot, i):
    hold, gone = shot.get('tape', [0, 0])
    if i <= hold:
        return 1.0
    if i >= gone:
        return 0.0
    t = (i - hold) / (gone - hold)
    return 1 - t * t * (3 - 2 * t)


# --------------------------------------------------------------- transitions

def wobble(a, amount, index):
    """Brief horizontal sync wobble across a cut."""
    if amount <= 0:
        return a
    h, w = a.shape[:2]
    rows = np.arange(h)
    phase = index * 1.7
    offset = amount * (np.sin(rows / 37.0 + phase) * 0.7 + np.sin(rows / 11.0 - phase * 2.3) * 0.3)
    idx = np.clip(np.arange(w)[None, :] - np.round(offset)[:, None].astype(np.int32), 0, w - 1)
    out = a[rows[:, None], idx]
    vertical = int(round(amount * 0.35))
    if vertical:
        out = np.concatenate([np.repeat(out[:1], vertical, axis=0), out[:-vertical]], axis=0)
    return out


def static(a, amount, index):
    if amount <= 0:
        return a
    h, w = a.shape[:2]
    snow = np.abs(grain((w, h), index, 11))[..., None] * 0.55
    rng = np.random.default_rng(4000 + index)
    tear = np.repeat(rng.standard_normal((h // 6 + 1, 1, 1)) * 0.08, 6, axis=0)[:h]
    return a * (1 - amount) + (snow + tear) * amount


def transition(shot, i, frames_left, next_shot, a, index):
    kind = shot['transition']
    if kind == 'wobble' and i < 6:
        a = wobble(a, 16 * 0.62 ** i, index)
    elif kind == 'static' and i < 4:
        a = static(a, [0.8, 0.5, 0.24, 0.08][i], index)
    elif kind == 'flash' and i < 6:
        a = a + np.array([1.0, 0.96, 0.88], dtype=np.float32) * [0.42, 0.24, 0.12, 0.05, 0.02, 0.008][i]
    # The outgoing shot starts to lose sync just before the cut.
    if next_shot and frames_left < 2:
        nk = next_shot['transition']
        if nk == 'wobble':
            a = wobble(a, [9, 4][frames_left], index)
        elif nk == 'static':
            a = static(a, [0.4, 0.15][frames_left], index)
    return a


# ---------------------------------------------------------------- CRT finish

def crt(a, index, bloom=1.0):
    """Restrained display finish: halation, scanlines, vignette and grain. The
    encoder adds the curvature and radial colour fringe."""
    h, w = a.shape[:2]
    a = np.clip(a, 0, 1.6)
    l = luma(a)
    knee = np.clip((l - 0.84) / 0.2, 0, 1)
    bright = a * (knee * knee)[..., None]
    if bloom > 0:
        a = a + (glow(bright, 10) * 0.5 + glow(bright, 34, 8) * 0.5) * 0.26 * bloom
    # A gentle grade: a touch more contrast and colour than the raw palette.
    mean = luma(a)[..., None]
    a = mean + (a - mean) * 1.08
    a = a + (a - 0.45) * 0.06
    # Lift the black level slightly, as a lit tube does, and round the highlights.
    a = 0.012 + a * 0.985
    a = a - 0.06 * np.clip(a - 0.85, 0, None)
    profile = scan_profile(h)
    lum = np.clip(luma(a), 0, 1)[..., None]
    a = a * (profile + (1 - profile) * lum ** 1.6 * 0.7) * (1 / 0.948)
    a = a * vignette((w, h))
    a = a + grain((w, h), index)[..., None] * 0.009
    return a


# ------------------------------------------------------------------ layouts

BOOT_LINES = ['T.O.R.E. ENGINE  v0.1', 'RETAIL DATA ...... FIGHTERS ANTHOLOGY', 'SIMULATION ....... 120 HZ FIXED STEP',
              'THEATERS ......... 16', 'AIRCRAFT ......... 14 FLYABLE', '', 'READY FOR FIRST FLIGHT']
BOOT_PORTRAIT = ['T.O.R.E. ENGINE v0.1', 'RETAIL DATA ..... FOUND', 'SIM ....... 120 HZ FIXED', 'THEATERS .......... 16',
                 'AIRCRAFT .......... 14', '', 'READY FOR FIRST FLIGHT']
BOOT_TYPE_FROM, BOOT_RATE = 15, 5


def boot_line_frames():
    """Boot frames on which each landscape listing line finishes typing."""
    done, chars = [], 0
    for line in BOOT_LINES:
        chars += len(line) + 2
        if line:
            done.append(BOOT_TYPE_FROM + math.ceil((chars - 2) / BOOT_RATE))
    return done

def boot(size, i, canvas, vertical):
    """Power-on, a short boot listing in phosphor green and the aircraft icon."""
    w, h = size
    a = np.zeros((h, w, 3), dtype=np.float32) + np.array([0.012, 0.018, 0.02], dtype=np.float32)
    if i < 7:
        # A dot, then a bright line spreading across the tube.
        extent = [0, 0.02, 0.08, 0.3, 0.7, 1.0, 1.0][i]
        if extent:
            x0, x1 = round(w * (0.5 - extent / 2)), round(w * (0.5 + extent / 2))
            a[h // 2 - 2:h // 2 + 2, x0:x1] = [1.4, 1.4, 1.5]
        a[h // 2 - 4:h // 2 + 4, w // 2 - 4:w // 2 + 4] += 1.0
        return a
    if i < 14:
        # The raster opens vertically from the centre line.
        t = (i - 6) / 8
        half = round(h / 2 * t * t)
        a[h // 2 - half - 2:h // 2 + half + 2] += 0.08 * (1 - t) + 0.02
        a[h // 2 - 2:h // 2 + 2] += 0.9 * (1 - t)
        return a
    left = 100 if vertical else 300
    top = 520 if vertical else 300
    lines = BOOT_PORTRAIT if vertical else BOOT_LINES
    chars = max(0, (i - BOOT_TYPE_FROM) * BOOT_RATE)
    y = top
    for n, line in enumerate(lines):
        shown = line[:max(0, min(len(line), chars))]
        chars -= len(line) + 2
        if shown:
            canvas.text(shown, left, y, 30 if n else 36, PHOSPHOR if n != 6 else AMBER, limit=(w - 2 * left))
        y += 48 if n else 62
        if chars < 0 and shown != line:
            break
    if (i // 9) % 2 == 0 and i > 20:
        canvas.draw.rectangle((left, y + 4, left + 18, y + 30), fill=(*PHOSPHOR, 255))
    if i >= 20:
        icon = brand('icon', 230 if vertical else 300)
        develop = min(1.0, (i - 20) / 14)
        art = icon.copy()
        cut = round(art.height * develop)
        if cut < art.height:
            art.paste((0, 0, 0, 0), (0, cut, art.width, art.height))
        x = (w - art.width) // 2 if vertical else 1340
        canvas.stamp(art, x, 1090 if vertical else 300, 'T.O.R.E. icon')
    return a


def roster(canvas, size, names, active, heading, i, vertical, appear):
    """The full list, highlighted on the aircraft or theater shown."""
    w, h = size
    shown = len(names) if not appear else min(len(names), max(0, (i - 2) // 2 + 1))
    if vertical:
        x0, y0, x1, y1 = 60, 1178, 1020, 1484
        canvas.panel((x0, y0, x1, y1), 168)
        canvas.text(heading, x0 + 26, y0 + 14, 27, PHOSPHOR, limit=x1 - x0 - 50)
        half = (len(names) + 1) // 2
        for n, name in enumerate(names[:shown]):
            x = x0 + 26 + (n // half) * 470
            y = y0 + 60 + (n % half) * 29
            if name == active:
                canvas.draw.rectangle((x - 8, y - 3, x + 446, y + 25), fill=(*AMBER, 235))
            canvas.text(name, x, y, 21, INK if name == active else WHITE, limit=430, shadow=name != active)
        return
    x0, y0, x1, y1 = 1262, 214, 1662, 866
    canvas.panel((x0, y0, x1, y1), 150)
    canvas.text(heading, x0 + 24, y0 + 20, 30, PHOSPHOR, limit=x1 - x0 - 44)
    for n, name in enumerate(names[:shown]):
        y = y0 + 80 + n * 34
        if name == active:
            canvas.draw.rectangle((x0 + 12, y - 5, x1 - 14, y + 29), fill=(*AMBER, 235))
        canvas.text(name, x0 + 26, y, 24, INK if name == active else WHITE, limit=x1 - x0 - 50, shadow=name != active)


def replay_window(src, size):
    """The real replay viewer at its full aspect, inside a dark tube border,
    so curvature cannot clip its transport, timeline or labels."""
    w, h = size
    if w > h:
        rw = 1824
        rh = round(rw * 9 / 16)
        back = np.zeros((h, w, 3), dtype=np.float32) + 0.015
        win = to_f(src.resize((rw, rh), Image.Resampling.LANCZOS))
        back[(h - rh) // 2:(h - rh) // 2 + rh, (w - rw) // 2:(w - rw) // 2 + rw] = win
        return back
    rw, rh = 1080, 608
    blurred = to_f(fit(src.crop((0, 0, 1920, 860)), (1080, 1920)).resize((34, 60), Image.Resampling.BILINEAR).resize((1080, 1920), Image.Resampling.BILINEAR))
    back = blurred * 0.16 + 0.01
    top = (h - rh) // 2
    back[top - 6:top + rh + 6] = 0.0
    back[top:top + rh] = to_f(src.resize((rw, rh), Image.Resampling.LANCZOS))
    return back


# Badge size and centre: the badge and its lines are centred as one group,
# landscape in the frame and portrait in its central square.
END_BADGE = {'landscape': (400, 331), 'portrait': (470, 722)}
FLIP_X = [0.06, 0.22, 0.48, 0.78, 1.02, 1.09, 1.05, 1.0]
FLIP_Y = [1.10, 1.08, 1.06, 1.04, 1.03, 1.02, 1.01, 1.0]
FLICKER = [0.55, 1.25, 0.8, 1.15]
FLASH = {4: 0.65, 5: 0.35, 6: 0.15}


@lru_cache(maxsize=2)
def end_glow(size):
    """A faint warm pool of light behind the badge, like a lit title card."""
    w, h = size
    _, cy = END_BADGE['portrait' if w < h else 'landscape']
    y, x = np.ogrid[0:h, 0:w]
    r2 = ((x - w / 2) / (w * 0.34)) ** 2 + ((y - cy) / (h * (0.3 if w > h else 0.17))) ** 2
    return (np.exp(-r2 * 1.6)[..., None] * np.array([0.085, 0.045, 0.014], dtype=np.float32)).astype(np.float32)


@lru_cache(maxsize=32)
def sparkle(size):
    """A cheesy four-point star glint with two short diagonal rays and a soft
    hot core, the classic 1990s logo sparkle."""
    v, u = np.mgrid[-1:1:complex(size), -1:1:complex(size)]
    def ray(a, b, width, length):
        along = np.abs(a) / length
        return np.clip(1 - np.abs(b) / (width * np.clip(1 - along, 0, 1) + 1e-6), 0, 1) * (along < 1)
    d1, d2 = (u + v) / np.sqrt(2), (u - v) / np.sqrt(2)
    star = np.maximum.reduce([ray(u, v, 0.09, 1.0), ray(v, u, 0.09, 1.0), 0.7 * ray(d1, d2, 0.07, 0.5), 0.7 * ray(d2, d1, 0.07, 0.5)])
    r2 = u * u + v * v
    alpha = np.clip(star + np.exp(-r2 / 0.012) + 0.3 * np.exp(-r2 / 0.08), 0, 1)
    rgba = np.zeros((size, size, 4), dtype=np.uint8)
    rgba[..., 0], rgba[..., 1], rgba[..., 2] = 255, 248, 226
    rgba[..., 3] = (alpha * 255).astype(np.uint8)
    return Image.fromarray(rgba, 'RGBA')


def sparkles(i, sync, vertical):
    """Sparkle sprites alive at end-card frame i: a burst as the badge lands,
    a twinkle on every beat of the ending and a burst on its final stinger.
    Positions are seeded by event, so every render matches."""
    size, _ = END_BADGE['portrait' if vertical else 'landscape']
    events = [(4, 'burst', 7)]
    beats = [b for b in sync.get('beats', []) if b >= 9]
    step = (beats[1] - beats[0]) if len(beats) > 1 else 25.35
    stinger = sync.get('stinger')
    last = beats[-1] if beats else 9
    beats += [last + step * k for k in range(1, 40)]
    events += [(round(b), 'beat', 2) for b in beats if stinger is None or abs(b - stinger) > step / 2]
    if stinger is not None:
        events.append((round(stinger), 'burst', 9))
    alive = []
    for start, kind, count in events:
        rng = np.random.default_rng(start * 7 + (1 if kind == 'burst' else 0))
        for k in range(count):
            delay = int(rng.integers(0, 4)) if kind == 'burst' else k * 3
            life = 18 if kind == 'burst' else 15
            t = i - start - delay
            if not 0 <= t < life:
                continue
            angle = rng.uniform(0, 2 * np.pi)
            rim = kind == 'burst' or rng.random() < 0.85
            radius = (0.47 + rng.uniform(-0.03, 0.05)) if rim else rng.uniform(0.12, 0.4)
            big = rng.uniform(70, 110) if kind == 'burst' else rng.uniform(56, 84)
            phase = np.sin(np.pi * t / life) ** 0.7
            alive.append((np.cos(angle) * radius * size, np.sin(angle) * radius * size, big * phase, t * 5 + angle * 57))
    return alive


def badge(i, sync, vertical):
    """The project patch flicked in like a spun coin, with a flicker, a white
    flash as it lands and a shine sweeping across it after the landing and on
    the final stinger."""
    size, _ = END_BADGE['portrait' if vertical else 'landscape']
    art = brand('patch', size)
    xs = FLIP_X[i] if i < len(FLIP_X) else 1.0
    ys = FLIP_Y[i] if i < len(FLIP_Y) else 1.0
    a = np.asarray(art, dtype=np.float32) * (1 / 255)
    rgb, alpha = a[..., :3], a[..., 3:]
    light = (FLICKER[i] if i < len(FLICKER) else 1.0) * (0.6 + 0.4 * min(1.0, xs))
    rgb = rgb * light
    if i in FLASH:
        rgb = rgb + (1 - rgb) * FLASH[i]
    sweeps = [10] + ([round(sync['stinger'])] if sync.get('stinger') is not None else [])
    yy, xx = np.mgrid[0:size, 0:size] * (1 / size)
    for begin in sweeps:
        t = (i - begin) / 18
        if 0 <= t <= 1:
            band = np.exp(-(((xx + yy) / 2 - (t * 1.5 - 0.25)) ** 2) / 0.0035)
            rgb = rgb + band[..., None] * 0.75
    out = np.concatenate([np.clip(rgb, 0, 1) * 255, alpha * 255], axis=2).astype(np.uint8)
    im = Image.fromarray(out, 'RGBA')
    shape = (max(2, round(size * xs)), round(size * ys))
    return im.resize(shape, Image.Resampling.LANCZOS) if shape != (size, size) else im


def end_card(size, i, frames, canvas, vertical, residue, sync):
    """The badge flicks in with sprite sparkles, then the required lines type
    on. Every line is in place by frame 49 and then holds."""
    w, h = size
    a = np.zeros((h, w, 3), dtype=np.float32) + np.array([0.01, 0.014, 0.018], dtype=np.float32)
    a = a + end_glow(size)
    if residue is not None and i < 24:
        # A brief phosphor afterglow of the last replay frame.
        a = a + residue * (0.3 * math.exp(-i / 4))
    base, cy = END_BADGE['portrait' if vertical else 'landscape']
    art = badge(i, sync, vertical)
    shake = round(8 * math.sin(i * 2.7)) if 4 <= i < 9 else 0
    canvas.stamp(art, (w - art.width) / 2 + shake, cy - art.height / 2, 'T.O.R.E. Fighters badge')
    for dx, dy, big, spin in sparkles(i, sync, vertical):
        side = max(4, round(big))
        star = sparkle(side).rotate(spin % 90, resample=Image.Resampling.BICUBIC)
        x, y = round(w / 2 + dx - side / 2), round(cy + dy - side / 2)
        if 0 <= x and x + side <= w and 0 <= y and y + side <= h:
            canvas.im.alpha_composite(star, (x, y))
    # The badge already reads T.O.R.E. FIGHTERS, so no title line repeats it.
    lines = [('v0.1 FIRST FLIGHT', 42 if vertical else 44, PHOSPHOR),
             ('Free and open source', 32 if vertical else 33, WHITE), ('Bring your own Fighters Anthology', 28 if vertical else 30, WHITE)]
    y = cy + base / 2 + 24
    gaps = [60 if vertical else 66, 48 if vertical else 50, 56 if vertical else 58]
    start = 12
    for n, (words, size_, color) in enumerate(lines):
        chars = (i - start - n * 5) * 6
        if chars > 0:
            canvas.text(words[:chars], w / 2, y, size_, color, 'center', limit=(w - 140) if vertical else 1300)
        y += gaps[n]
    repo = ['github.com/john-overton/', 'T.O.R.E-Fighters'] if vertical else ['github.com/john-overton/T.O.R.E-Fighters']
    chars = (i - 34) * 8
    for line in repo:
        if chars > 0:
            canvas.text(line[:chars], w / 2, y, 27, AMBER, 'center')
        chars -= len(line)
        y += 38
    y += 12
    # The three desktop platforms the game is built and checked for.
    icons = platforms(48 if vertical else 50)
    if i >= 38:
        rise = min(1.0, (i - 37) / 4)
        art = icons.copy()
        art.putalpha(art.getchannel('A').point(lambda v: round(v * rise)))
        canvas.stamp(art, (w - icons.width) / 2, y, 'Windows, Linux and macOS glyphs')
    y += icons.height + 22
    if i >= 41:
        canvas.text('MULTIPLAYER INBOUND', w / 2, y, 34 if vertical else 36, PHOSPHOR, 'center')
    y += 62 if vertical else 64
    note = ['Fan project. Not affiliated with', "Electronic Arts or Jane's."] if vertical else ["Fan project. Not affiliated with Electronic Arts or Jane's."]
    if i >= 45:
        for line in note:
            canvas.text(line, w / 2, y, 24, (196, 200, 192), 'center')
            y += 34
    return a


# ------------------------------------------------------------ composition

class Reader:
    def __init__(self, path, first=0, alpha=False):
        self.alpha = alpha
        vf = f'select=gte(n\\,{first})' if first else 'null'
        self.proc = subprocess.Popen(['ffmpeg', '-v', 'error', '-threads', '2', '-i', str(path), '-vf', vf, '-fps_mode', 'passthrough',
                                      '-f', 'rawvideo', '-pix_fmt', 'rgba' if alpha else 'rgb24', 'pipe:1'],
                                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        # Readers stop early by design; a short read raises below instead.

    def frame(self):
        n = 1920 * 1080 * (4 if self.alpha else 3)
        data = self.proc.stdout.read(n)
        if len(data) != n:
            raise RuntimeError('Capture ended before shot did')
        return Image.frombytes('RGBA' if self.alpha else 'RGB', (1920, 1080), data)

    def close(self):
        self.proc.stdout.close()
        self.proc.kill()
        self.proc.wait()


def draw_frame(shot, i, frames, next_shot, sources, layer, size, config, boxes, index, residue):
    w, h = size
    vertical = w < h
    layout = shot['layout']
    canvas = Canvas(size, boxes)
    hud = None
    if layout == 'boot':
        a = boot(size, i, canvas, vertical)
    elif layout == 'end':
        a = end_card(size, i, frames, canvas, vertical, residue, shot.get('sync', {}))
    elif layout == 'replay':
        a = replay_window(sources[0], size)
    else:
        src = sources[0]
        if layout == 'cockpit' and layer is not None:
            hud = layer
        cx = keyed(shot.get('vcrop'), i, 0.5)
        if vertical and layout in ('aircraft', 'theater'):
            a = band(src, cx, shot.get('vcy', 0.45), shot.get('vzoom', 1.0))
        elif vertical:
            a = square(src, cx)
        else:
            a = to_f(src)
        if hud is not None:
            if vertical:
                layer_full = np.zeros((1920, 1080, 4), dtype=np.float32)
                layer_full[420:1500] = to_f(hud.crop(square_window(cx)))
                hud = layer_full
            else:
                hud = to_f(hud)
        if layout == 'cockpit':
            a = tape(a, hud, tape_strength(shot, i), index, i)
            if shot['transition'] == 'tape' and i < 8:
                # The tape locks in: picture rolls down into place, colour last.
                roll = round((1 - i / 8) ** 2 * h * 0.35)
                if roll:
                    a = np.concatenate([a[-roll:] * 0.25, a[:-roll]], axis=0)
                if i < 3:
                    g = luma(a)[..., None]
                    a = np.repeat(g, 3, axis=2)
        if layout in ('aircraft', 'theater'):
            aircraft = layout == 'aircraft'
            names = config['reel']['aircraft_roster' if aircraft else 'theater_roster']
            heading = '14 FLYABLE AIRCRAFT' if aircraft else '16 THEATERS'
            appear = shot['id'] in ('hero-f14', 'vista-gre')
            roster(canvas, size, names, shot['overlay'], heading, i, vertical, appear)
    a = transition(shot, i, frames - 1 - i, next_shot, a, index)
    over = canvas.array()
    if over[..., 3].any():
        a = a * (1 - over[..., 3:4]) + over[..., :3] * over[..., 3:4]
        # Phosphor lettering glows a little into the glass around it.
        a = a + glow(over[..., :3] * over[..., 3:4], 7) * 0.32
    if layout == 'end' and i < 6:
        a = a + np.array([1.0, 0.95, 0.85], dtype=np.float32) * [0.38, 0.2, 0.1, 0.05, 0.02, 0.008][i]
    return crt(a, index, shot.get('bloom', 1.0))


def compose_part(task):
    """One shot of one edit into a lossless part. Runs in a worker process."""
    edit_name, order, shot, first, count, seg_end, next_shot, size, take, config, base = task
    boxes = []
    readers = [Reader(OUT / take / f'{c}.mkv', first) for c in shot.get('clips', [])]
    layer = Reader(OUT / take / f"{shot['clips'][0]}.hud.mkv", first, True) if shot['layout'] == 'cockpit' else None
    residue = None
    if shot['layout'] == 'end':
        tail = OUT / 'parts' / edit_name / 'residue.npy'
        residue = np.load(tail) if tail.exists() else None
    part = OUT / 'parts' / edit_name / f'{order:02}-{shot["id"]}.mkv'
    enc = subprocess.Popen(['ffmpeg', '-y', '-v', 'error', '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-s', f'{size[0]}x{size[1]}', '-r', '60', '-i', 'pipe:0',
                            '-c:v', 'ffv1', '-level', '3', '-threads', '2', '-pix_fmt', 'gbrp', str(part)], stdin=subprocess.PIPE)
    hashes = []
    try:
        for k in range(count):
            i = first + k
            sources = [r.frame() for r in readers]
            hud = layer.frame() if layer else None
            pixels = to_u8(draw_frame(shot, i, seg_end, next_shot, sources, hud, size, config, boxes, base + k, residue)).tobytes()
            hashes.append(hashlib.sha256(pixels).hexdigest())
            enc.stdin.write(pixels)
            if k == count - 1 and shot['layout'] == 'replay':
                np.save(OUT / 'parts' / edit_name / f'residue-{order:02}.npy', to_f(Image.frombytes('RGB', size, pixels)))
    finally:
        for r in readers + ([layer] if layer else []):
            r.close()
        enc.stdin.close()
        enc.wait()
    if enc.returncode:
        raise RuntimeError(f'Part encoder failed: {part}')
    unique = sorted({tuple(b) for b in boxes})
    return order, hashes, [list(b) for b in unique], str(part)


def geometry(size):
    """Tube curvature with a slightly different curvature per gun: a radial
    colour fringe at the edges and none at the centre."""
    k2 = 0.004
    k = {'r': 0.0145, 'g': 0.0125, 'b': 0.0105}
    planes = ''.join(f'[{c}0]extractplanes={c},lenscorrection=k1={k[c]}:k2={k2}:i=bilinear[{c}1];' for c in 'rgb')
    # Lossless parts carry millisecond container timestamps; every frame is
    # re-timed to exactly n/60 s before encoding.
    return f'setpts=N/60/TB,format=gbrp,split=3[r0][g0][b0];{planes}[g1][b1][r1]mergeplanes=0x001020:gbrp,format=yuv420p'


def compose(config, edit_name, plan, size, take, destination, uhd=None):
    """Compose an edit (a list of shot segments) in parallel parts, then encode
    once through the tube geometry. Returns the per-frame composed hashes."""
    folder = OUT / 'parts' / edit_name
    folder.mkdir(parents=True, exist_ok=True)
    for old in folder.glob('*'):
        old.unlink()
    # Long shots are split into chunks so the workers stay evenly loaded. The
    # chunk boundaries never change the pixels: every frame is drawn from its
    # own index alone.
    tasks, base, order = [], 0, 0
    for n, (shot, first, count) in enumerate(plan):
        next_shot = plan[n + 1][0] if n + 1 < len(plan) else None
        for k in range(0, count, CHUNK):
            part = min(CHUNK, count - k)
            tasks.append((edit_name, order, shot, first + k, part, first + count, next_shot, size, take, config, base + k))
            order += 1
        base += count
    # The end card's phosphor residue needs the last replay frame first.
    end_tasks = [t for t in tasks if t[2]['layout'] == 'end']
    body = [t for t in tasks if t[2]['layout'] != 'end']
    results = {}
    boxes = []
    with ProcessPoolExecutor(WORKERS) as pool:
        for order, hashes, part_boxes, part in pool.map(compose_part, body):
            results[order] = (hashes, part)
            boxes += part_boxes
        last_replay = max(t[1] for t in body if t[2]['layout'] == 'replay')
        residue = folder / f'residue-{last_replay:02}.npy'
        if residue.exists():
            residue.replace(folder / 'residue.npy')
        for order, hashes, part_boxes, part in pool.map(compose_part, end_tasks):
            results[order] = (hashes, part)
            boxes += part_boxes
    listing = folder / 'parts.txt'
    listing.write_text(''.join(f"file '{results[o][1]}'\n" for o in sorted(results)))
    run(['ffmpeg', '-y', '-v', 'error', '-f', 'concat', '-safe', '0', '-i', listing, '-c', 'copy', folder / 'lossless.mkv'])
    for o in results:
        Path(results[o][1]).unlink()
    run(['ffmpeg', '-y', '-v', 'error', '-i', folder / 'lossless.mkv', '-filter_complex', geometry(size), '-c:v', 'libx264', '-preset', 'slow', '-crf', '20',
         '-threads', '6', '-pix_fmt', 'yuv420p', '-an', '-movflags', '+faststart', destination])
    if uhd:
        # 4K: the lossless composition doubled with Lanczos, then the same
        # tube curvature drawn at full size. The engine captures at 1080p.
        w, h = size
        run(['ffmpeg', '-y', '-v', 'error', '-i', folder / 'lossless.mkv', '-filter_complex', f'scale={2 * w}:{2 * h}:flags=lanczos,{geometry(size)}',
             '-c:v', 'libx264', '-preset', 'slow', '-crf', '20', '-profile:v', 'high', '-level:v', '5.2', '-threads', '12', '-pix_fmt', 'yuv420p',
             '-an', '-movflags', '+faststart', uhd])
    hashes = [h for o in sorted(results) for h in results[o][0]]
    return hashes, boxes


def with_sync(config, plan):
    """Hand the end card this edit's ending beats and stinger, relative to its
    first frame, so its sparkles land on the music."""
    total = sum(count for _, _, count in plan)
    cursor, kick, end_start = 0, None, None
    for shot, first, count in plan:
        if shot['id'] == config['music']['kick_shot']:
            kick = cursor
        if shot['layout'] == 'end':
            end_start = cursor - first
        cursor += count
    ending = ending_plan(config, total, kick)
    sync = {'beats': [b - end_start for b in ending['beat_frames'] if b >= end_start], 'stinger': ending['stinger_frame'] - end_start}
    return [({**shot, 'sync': sync} if shot['layout'] == 'end' else shot, first, count) for shot, first, count in plan]


def master_plan(config):
    return with_sync(config, [(s, 0, s['frames']) for s in config['shot']])


def cutdown_plan(config):
    by_id = {s['id']: s for s in config['shot']}
    return with_sync(config, [(by_id[shot_id], first, count) for shot_id, first, count in config['cutdown']['segments']])


# ---------------------------------------------------------------------- audio

RATE = 48000
SPF = RATE // FPS


def ramp(n, length):
    e = np.ones(n, dtype=np.float32)
    k = min(length, n // 2)
    if k:
        e[:k] = np.linspace(0, 1, k, dtype=np.float32)
        e[-k:] = np.linspace(1, 0, k, dtype=np.float32)
    return e


# A recorded call may ring this far past its cut; any longer and it is left out.
SPEECH_GRACE = 960


def voices(take, clip):
    """Each speech recording the capture played, as [start, end) samples in
    capture time. `end` is None when the recording outlasts the capture."""
    speech = np.fromfile(OUT / take / f'{clip}.speech', dtype='<f4')
    starts = [int(x) for x in (OUT / take / f'{clip}.speech-starts').read_text().split()]
    found = []
    for n, start in enumerate(starts):
        stop = starts[n + 1] if n + 1 < len(starts) else len(speech)
        sounding = np.nonzero(np.abs(speech[start:stop]) > 1e-6)[0]
        end = start + int(sounding[-1]) + 1 if len(sounding) else start
        if n + 1 == len(starts) and end > len(speech) - 480:
            end = None
        found.append((start, end))
    return found


def speech_window(take, shot, clip, first, count):
    """The shot's radio: only recordings that start inside the shot and finish
    within SPEECH_GRACE of its cut are kept, whole. Returns the speech (which
    may run SPEECH_GRACE past the cut) and the decision for each recording."""
    speech = np.fromfile(OUT / take / f'{clip}.speech', dtype='<f4')
    offset = shot.get('audio_offset', 0)
    a, b = (first + offset) * SPF, (first + offset + count) * SPF
    out = np.zeros(b - a + SPEECH_GRACE, dtype=np.float32)
    decisions = []
    for start, end in voices(take, clip):
        if start >= b or (end is not None and end <= a):
            continue
        whole = end is not None and start >= a and end <= b + SPEECH_GRACE
        if whole:
            out[start - a:end - a] = speech[start:end]
        decisions.append({'shot': shot['id'], 'clip': clip, 'start_sample': start, 'end_sample': end,
                          'window': [a, b], 'kept': whole,
                          'reason': 'finishes in its shot' if whole else 'would be cut by the edit'})
    return out, decisions


def clip_audio(take, clip, first, count):
    effects = np.fromfile(OUT / take / f'{clip}.f32', dtype='<f4').reshape(-1, 2)
    speech = np.fromfile(OUT / take / f'{clip}.speech', dtype='<f4')
    a, b = first * SPF, (first + count) * SPF
    assert len(effects) >= b and len(speech) >= b, (clip, first, count)
    return effects[a:b], speech[a:b]


def follower(x, attack, release):
    """Peak envelope follower with separate attack and release, in seconds."""
    env = np.zeros(len(x), dtype=np.float32)
    level = 0.0
    up, down = 1 - math.exp(-1 / (RATE * attack)), 1 - math.exp(-1 / (RATE * release))
    mag = np.abs(x)
    # Block-wise for speed: 48 samples per step.
    step = 48
    peaks = mag[:len(mag) // step * step].reshape(-1, step).max(axis=1)
    out = np.zeros(len(peaks), dtype=np.float32)
    up_b, down_b = 1 - (1 - up) ** step, 1 - (1 - down) ** step
    for n, p in enumerate(peaks):
        level += (p - level) * (up_b if p > level else down_b)
        out[n] = level
    env[:len(out) * step] = np.repeat(out, step)
    env[len(out) * step:] = level
    return env


def limit(x, ceiling, lookahead=0.004, release=0.08):
    """A transparent look-ahead peak limiter with a smooth release, so the
    loudness pass can stay linear."""
    peak = np.abs(x).max(axis=1)
    n = len(peak)
    ahead = round(lookahead * RATE)
    # Peak over the look-ahead window, in blocks for speed.
    step = 16
    blocks = np.pad(peak, (0, (-n) % step)).reshape(-1, step).max(axis=1)
    span = ahead // step + 1
    window = np.max(np.lib.stride_tricks.sliding_window_view(np.pad(blocks, (0, span - 1)), span), axis=1)
    want = np.minimum(1.0, ceiling / np.maximum(window, 1e-9))
    gain = np.empty_like(want)
    level, rel = 1.0, 1 - math.exp(-step / (RATE * release))
    for k, g in enumerate(want):
        level = g if g < level else level + (g - level) * rel
        gain[k] = level
    gain = np.repeat(gain, step)[:n].astype(np.float32)
    return x * gain[:, None]


def stretched_bed(m):
    """The bed, moved onto the cut grid by a pitch-preserving rubberband
    stretch when the recipe asks; cached beside the exported phrases."""
    source = OUT / 'music' / f"{m['bed']}.f32"
    if not m.get('stretch_bed'):
        return np.fromfile(source, dtype='<f4'), m['bed_bpm'], m['bed_first_beat']
    ratio = m['grid_bpm'] / m['bed_bpm']
    stretched = OUT / 'music' / f"{m['bed']}.x{ratio:.6f}.f32"
    if not stretched.exists():
        run(['ffmpeg', '-y', '-v', 'error', '-f', 'f32le', '-ar', str(RATE), '-ac', '1', '-i', source,
             '-af', f'rubberband=tempo={ratio:.6f}:transients=crisp', '-f', 'f32le', '-ar', str(RATE), '-ac', '1', stretched])
    return np.fromfile(stretched, dtype='<f4'), m['grid_bpm'], m['bed_first_beat'] / ratio


def ending_plan(config, total_frames, kick_frame):
    """Where the ending falls in one edit: as many whole bars from the kick as
    leave the stinger room to ring out, its beats and its stinger, in frames."""
    m = config['music']
    length = (OUT / 'music' / f"{m['ending']}.f32").stat().st_size // 4
    stinger = length - round(m['ending_stinger'] * RATE)
    bar = 4 * 60 / m['ending_bpm'] * RATE
    bars = max(1, int((total_frames * SPF - kick_frame * SPF - stinger) // bar))
    beat_frames = [kick_frame + k * 60 / m['ending_bpm'] * FPS for k in range(bars * 4)]
    return {'bars': bars, 'stinger_frame': kick_frame + bars * bar / SPF, 'beat_frames': beat_frames,
            'stinger_samples': stinger, 'bar_samples': bar}


def music_bed(config, total_frames, downbeat_frame, kick_frame):
    """The score for one edit: the bed from its pickup, its first downbeat on
    the launch cut, until the kick; then the ending's whole bars from its kick
    bar, spliced on the beat into its own final stinger."""
    m = config['music']
    n = total_frames * SPF
    out = np.zeros(n, dtype=np.float32)
    bed, bpm, first = stretched_bed(m)
    bed = bed * np.float32(10 ** (m['bed_gain_db'] / 20))
    entry = first + m['bed_entry_bar'] * 4 * 60 / bpm
    start = round((downbeat_frame / FPS - entry) * RATE)
    kick = kick_frame * SPF
    # The bed stops 15 ms before the kick, so its own next downbeat can never
    # flam against the ending's; the brief gap reads as a breath before the hit.
    stop = kick - round(0.015 * RATE)
    src0, dst0 = max(0, -start), max(0, start)
    length = min(stop - dst0, len(bed) - src0)
    seg = bed[src0:src0 + length].copy()
    seg[-480:] *= np.linspace(1, 0, 480, dtype=np.float32)
    if src0:
        seg[:48] *= np.linspace(0, 1, 48, dtype=np.float32)
    out[dst0:dst0 + length] = seg
    ending = np.fromfile(OUT / 'music' / f"{m['ending']}.f32", dtype='<f4') * np.float32(10 ** (m['ending_gain_db'] / 20))
    plan = ending_plan(config, total_frames, kick_frame)
    pre = round(0.005 * RATE)
    downbeat = round((m['ending_first_beat'] + m['ending_kick_bar'] * 4 * 60 / m['ending_bpm']) * RATE)
    body_len = round(plan['bars'] * plan['bar_samples'])
    body = ending[downbeat - pre:downbeat - pre + body_len].copy()
    body[:96] *= np.linspace(0, 1, 96, dtype=np.float32)
    body[-288:] *= np.linspace(1, 0, 288, dtype=np.float32)
    out[kick - pre:kick - pre + len(body)] += body
    stinger = ending[round(m['ending_stinger'] * RATE):].copy()
    stinger[:48] *= np.linspace(0, 1, 48, dtype=np.float32)
    at = kick - pre + body_len
    k = min(len(stinger), n - at)
    out[at:at + k] += stinger[:k]
    return out, {'kick_sample': int(kick), 'bars': plan['bars'], 'stinger_sample': int(at + pre),
                 'music_end_sample': int(at + k), 'bed_start_sample': int(dst0)}


def mix(config, plan, take, name, with_music=True):
    """Effects and speech from the captures, the score bed, ducking and Foley.
    Writes the unmastered mix and returns its path. Without music, the same
    effects, speech and Foley go through the same limiter."""
    total = sum(count for _, _, count in plan)
    n = total * SPF
    effects = np.zeros((n, 2), dtype=np.float32)
    speech = np.zeros(n, dtype=np.float32)
    cursor = 0
    starts = {}
    speech_log = []
    downbeat = None
    for shot, first, count in plan:
        starts[shot['id']] = cursor
        if shot['id'] == 'launch':
            downbeat = cursor
        for clip in shot.get('clips', [])[:1]:
            # `audio_offset` advances the shot's sound by whole frames, and
            # `audio_tail` lets it ring on over the next shot; both read the
            # capture's own extra recorded frames.
            offset, tail = shot.get('audio_offset', 0), shot.get('audio_tail', 0)
            fx, sp = clip_audio(take, clip, first + offset, count + tail)
            a = cursor * SPF
            edge = ramp(len(fx), 288)
            if tail:
                edge[count * SPF:] *= np.linspace(1, 0, tail * SPF, dtype=np.float32) ** 1.5
                edge[:288] = np.linspace(0, 1, 288, dtype=np.float32)
            k = min(len(fx), n - a)
            effects[a:a + k] += fx[:k] * edge[:k, None]
            if shot.get('speech', 'all') != 'none':
                sp, decisions = speech_window(take, shot, clip, first, count)
                k = min(len(sp), n - a)
                speech[a:a + k] += sp[:k]
                for d in decisions:
                    d['output_start_sample'] = a + d['start_sample'] - d['window'][0]
                speech_log.extend(decisions)
        cursor += count
    m = config['music']
    kick = starts[m['kick_shot']]
    env = follower(speech, 0.012, 0.2)
    if with_music:
        bed, info = music_bed(config, total, downbeat, kick)
        depth = np.full(n, m['duck_db'], dtype=np.float32)
        depth[kick * SPF:] = m['ending_duck_db']
        duck = 1 - (1 - 10 ** (-depth / 20)) * np.clip(env / 0.08, 0, 1)
        music = bed * duck * m.get('level', 0.4)
    else:
        info, music = {'music': False}, np.zeros(n, dtype=np.float32)
    # Boot Foley: the retail cockpit switch as the tube powers on, and button
    # clicks under the boot listing.
    foley = np.zeros(n, dtype=np.float32)
    def place(clip, at, level):
        x = np.fromfile(OUT / 'music' / f'{clip}.f32', dtype='<f4') * level
        a = round(at * RATE)
        k = min(len(x), n - a)
        foley[a:a + k] += x[:k]
    place('&SWITCH.11K', 0.0, 0.55)
    for frame in boot_line_frames():
        place('&BUTTON.11K', frame / FPS, 0.14)
    # A short burst of tube static at the night cut, authored.
    rng = np.random.default_rng(77)
    for shot, first, count in plan:
        if shot['transition'] == 'static':
            a = starts[shot['id']] * SPF - round(0.03 * RATE)
            burst = rng.standard_normal(round(0.09 * RATE)).astype(np.float32) * 0.05
            burst *= np.linspace(1, 0, len(burst), dtype=np.float32) ** 2
            foley[a:a + len(burst)] += burst
    mixed = effects * 1.6 + (music + speech * 2.0 + foley)[:, None]
    # Close the edit gently after the cadence rings out.
    tail = round(0.35 * RATE)
    mixed[-tail:] *= np.linspace(1, 0, tail, dtype=np.float32)[:, None] ** 2
    mixed = limit(mixed, 10 ** (-3.6 / 20))
    out = OUT / f'{name}-mix.f32'
    mixed.astype('<f4').tofile(out)
    np.savez_compressed(OUT / 'validation' / f'{name}-stems.npz', effects=effects[::8], music=music[::8], speech=speech[::8], foley=foley[::8])
    np.save(OUT / 'validation' / f'{name}-speech-envelope.npy', env[::SPF])
    speech.astype('<f4').tofile(OUT / 'validation' / f'{name}-speech.f32')
    (OUT / 'validation' / f'{name}-speech.json').write_text(json.dumps(speech_log, indent=2) + '\n')
    (OUT / 'validation' / f'{name}-music.json').write_text(json.dumps({**info, 'downbeat_frame': downbeat, 'kick_frame': kick}, indent=2) + '\n')
    return out


def master(mix_path, wav, gain_db=None):
    """Linear mastering: one measured gain to -14 LUFS integrated, or the given
    gain. The limiter in the mix leaves the true peak below -1.5 dBTP; this
    fails otherwise."""
    command = ['ffmpeg', '-hide_banner', '-f', 'f32le', '-ar', str(RATE), '-ac', '2', '-i', str(mix_path), '-af', 'loudnorm=I=-14:TP=-1.5:LRA=18:print_format=json', '-f', 'null', '-']
    measured = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=True).stderr
    vals = json.JSONDecoder().raw_decode(measured[measured.rfind('{'):])[0]
    gain = -14.0 - float(vals['input_i']) if gain_db is None else gain_db
    predicted = float(vals['input_tp']) + gain
    vals.update({'applied_gain_db': round(gain, 3), 'predicted_true_peak_dbtp': round(predicted, 3)})
    (OUT / 'validation' / f'{Path(wav).stem}-loudness-analysis.json').write_text(json.dumps(vals, indent=2) + '\n')
    assert predicted <= -1.5, f'Mastering would exceed -1.5 dBTP: {vals}'
    run(['ffmpeg', '-y', '-v', 'error', '-f', 'f32le', '-ar', str(RATE), '-ac', '2', '-i', mix_path, '-af', f'volume={gain:.3f}dB', '-ar', str(RATE), '-c:a', 'pcm_s24le', wav])


# ----------------------------------------------------------------- the edit

def edit(config):
    for old in (OUT / 'stills').glob('tore-*.png'):
        old.unlink()
    plan = master_plan(config)
    a, boxes_a = compose(config, 'landscape-a', plan, LANDSCAPE, 'take-a', OUT / 'landscape-silent.mp4', OUT / 'landscape-silent-4k.mp4')
    keep = OUT / 'parts/landscape-a/lossless.mkv'
    keep.replace(OUT / 'landscape-lossless.mkv')
    b, _ = compose(config, 'landscape-b', plan, LANDSCAPE, 'take-b', OUT / 'validation/repeat-silent.mp4')
    assert a == b, 'Composed pictures differ between consecutive renders'
    v, boxes_v = compose(config, 'portrait', plan, PORTRAIT, 'take-a', OUT / 'vertical-silent.mp4', OUT / 'vertical-silent-4k.mp4')
    c, boxes_c = compose(config, 'cutdown', cutdown_plan(config), LANDSCAPE, 'take-a', OUT / 'cutdown-silent.mp4', OUT / 'cutdown-silent-4k.mp4')
    # Only the landscape master stays lossless, for the stills and the WebM.
    for name in ['landscape-b', 'portrait', 'cutdown']:
        (OUT / 'parts' / name / 'lossless.mkv').unlink(missing_ok=True)
    (OUT / 'validation/composition-repeat.json').write_text(json.dumps({
        'frames': len(a), 'first_sha256': hashlib.sha256(''.join(a).encode()).hexdigest(),
        'second_sha256': hashlib.sha256(''.join(b).encode()).hexdigest(), 'identical': a == b}, indent=2) + '\n')
    unique = sorted({tuple(x) for x in boxes_a + boxes_v + boxes_c})
    (OUT / 'validation/text-boxes.json').write_text(json.dumps([list(x) for x in unique], indent=2) + '\n')
    for frame in config['reel']['stills']:
        run(['ffmpeg', '-y', '-v', 'error', '-i', OUT / 'landscape-lossless.mkv', '-filter_complex',
             f"select='eq(n\\,{frame})',{geometry(LANDSCAPE).replace(',format=yuv420p', ',format=rgb24')}", '-frames:v', '1', OUT / 'stills' / f'tore-{frame:04}.png'])
    finish(config)


def deliverables(config):
    """Output names, by the master's length in whole seconds."""
    d = config['reel']['duration']
    return {'landscape': f'tore_reel_{d}s_1080p.mp4', 'webm': f'tore_reel_{d}s_1080p.webm',
            'vertical': f'tore_reel_{d}s_vertical.mp4', 'cutdown': 'tore_reel_15s_1080p.mp4',
            'landscape_4k': f'tore_reel_{d}s_4k.mp4', 'vertical_4k': f'tore_reel_{d}s_vertical_4k.mp4', 'cutdown_4k': 'tore_reel_15s_4k.mp4'}


def no_music(config):
    """Dialogue and effects without the score, for cutting in other music: the
    same mix with the music removed, at the finished film's levels (the music
    master's own gain, not renormalised), as lossless WAV and as MP4."""
    names = deliverables(config)
    d = config['reel']['duration']
    jobs = [(master_plan(config), 'master', f'tore_reel_{d}s_no_music.wav', [('landscape-silent.mp4', names['landscape']), ('vertical-silent.mp4', names['vertical']),
                                                                             ('landscape-silent-4k.mp4', names['landscape_4k']), ('vertical-silent-4k.mp4', names['vertical_4k'])], d),
            (cutdown_plan(config), 'cutdown', 'tore_reel_15s_no_music.wav', [('cutdown-silent.mp4', names['cutdown']), ('cutdown-silent-4k.mp4', names['cutdown_4k'])], 15)]
    for plan, mix_name, wav, videos, seconds in jobs:
        gain = json.loads((OUT / 'validation' / f'{mix_name}-loudness-analysis.json').read_text())['applied_gain_db']
        master(mix(config, plan, 'take-a', f'{mix_name}-no-music', with_music=False), OUT / wav, gain_db=gain)
        for silent, dest in videos:
            # Without the score the gun transients stand bare; a 16 kHz low-pass
            # (the game's sources stop near 5.5 kHz) keeps AAC from overshooting.
            run(['ffmpeg', '-y', '-v', 'error', '-i', OUT / silent, '-i', OUT / wav, '-map', '0:v', '-map', '1:a', '-c:v', 'copy',
                 '-af', 'lowpass=f=16000:p=2', '-c:a', 'aac', '-b:a', '192k', '-cutoff', '18000', '-movflags', '+faststart', '-t', str(seconds),
                 OUT / dest.replace('.mp4', '_no_music.mp4')])


def finish(config):
    master(mix(config, master_plan(config), 'take-a', 'master'), OUT / 'master.wav')
    master(mix(config, cutdown_plan(config), 'take-a', 'cutdown'), OUT / 'cutdown.wav')
    names = deliverables(config)
    d = config['reel']['duration']
    for silent, wav, dest, seconds in [('landscape-silent.mp4', 'master.wav', names['landscape'], d), ('vertical-silent.mp4', 'master.wav', names['vertical'], d),
                                       ('cutdown-silent.mp4', 'cutdown.wav', names['cutdown'], 15),
                                       ('landscape-silent-4k.mp4', 'master.wav', names['landscape_4k'], d), ('vertical-silent-4k.mp4', 'master.wav', names['vertical_4k'], d),
                                       ('cutdown-silent-4k.mp4', 'cutdown.wav', names['cutdown_4k'], 15)]:
        # AAC priming ends the audio stream a little early, so cap by time
        # rather than by the shorter stream; every video frame is kept. The
        # 18 kHz encoder cutoff stops the default coder overshooting on the
        # sharpest recorded gun transients; the game audio holds nothing above.
        run(['ffmpeg', '-y', '-v', 'error', '-i', OUT / silent, '-i', OUT / wav, '-map', '0:v', '-map', '1:a', '-c:v', 'copy', '-c:a', 'aac', '-b:a', '192k',
             '-cutoff', '18000', '-movflags', '+faststart', '-t', str(seconds), OUT / dest])
    run(['ffmpeg', '-y', '-v', 'error', '-i', OUT / 'landscape-lossless.mkv', '-i', OUT / 'master.wav',
         '-filter_complex', f"[0:v]{geometry(LANDSCAPE)}[v]", '-map', '[v]', '-map', '1:a', '-c:v', 'libvpx-vp9', '-b:v', '0', '-crf', '30', '-row-mt', '1', '-threads', '8',
         '-deadline', 'good', '-cpu-used', '2', '-c:a', 'libopus', '-b:a', '192k', OUT / names['webm']])
    no_music(config)
