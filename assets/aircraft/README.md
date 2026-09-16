# Engine artwork

User-supplied artwork added by John on 2026-09-16, separate from imported FA media.

- `engine-texture-full.png`: untouched 1254 × 1254 original.
- `engine-texture.png`: 314 × 314 copy, reduced by 75% in each dimension.
- `engine-texture.rgba`: prepared runtime pixels, not an embedded executable asset.

Regenerate the smaller PNG and runtime file with
`python3 tools/prepare_engine_texture.py` (ImageMagick required for preparation).
Ship this directory under `assets/aircraft` beside the executable. The full-size
PNG is retained for future use; gameplay loads only the prepared reduced copy.
Behavior and the container format are documented in
[the engine material specification](../../docs/spec/engine-material.md).
