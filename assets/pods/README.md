# Equipment pod models

Authored low-poly models of the AN/ASQ-153 Pave Spike (`PAVESPK.SEE`), the
AN/AAS-38 NITE Hawk (`AAS38.SEE`) and the AN/ALQ-167 (`ALQ167.ECM`), made at
John's request on 2026-11-07. Fighters Anthology has no shape for these records,
so these are ours, not retail geometry. Behaviour, sizes and colours are in
[stores on pylons](../../docs/spec/stores-on-pylons.md#authored-pod-models-2026-11-07).

- `build_pods.py`: the source. A Blender Python script that builds each model
  from ring and part tables, checks it (under 100 triangles, faces wound
  outward) and exports its triangles.
- The game does not read this directory. The export is compiled in:
  `crates/tore-app/src/pylons/pod_models.rs`, a generated table. Never edit it
  by hand.

Rebuild after changing the script (Blender 4 or later), from the repository root:

```sh
blender --background --factory-startup --python assets/pods/build_pods.py -- \
    --export crates/tore-app/src/pylons/pod_models.rs \
    --thumbs .local/pods --blend .local/pods/pods.blend
cargo test --locked -p tore-app --bin tore-app pylons
```

`--thumbs` renders a flat-shaded picture of each model at the real pod's
proportions and `--blend` saves the meshes for inspection; both are optional
and stay local. Look at the result in game with
`tools/agent-run.sh target/debug/tore-app --stores-preview OUT_DIR`
(the `*-close-pod-*` frames).

## 2026-11-12 (TOR-13a): Tornado IDS Sky Shadow and BOZ-107 models

Two more models, for the RAF Tornado outer wing pylons, keyed by placeholder
names `SKYSHADOW` and `BOZ107` (no retail record exists for them; TOR-13
decides the mapping). Model only: nothing wires them to a station yet, and
`crates/tore-app/src/pylons/pod_models.rs` is unchanged, because the guard test
`authored_pod_models_stay_low_poly_and_inside_their_size` asserts exactly the
three shipped records; TOR-13 exports into it and updates that test.

- Both share John's shape (a long teardrop, pointed nose, widest a little
  forward of the middle, a short cylindrical aft end, four small fins), in RAF
  Tornado grey (Medium Sea Grey, BS 381C 637, sRGB 0.42, 0.47, 0.50, a pick).
- `SKYSHADOW` (ARI 23246 ECM pod, left/port outer pylon): 108 triangles, an
  elongated faceted bulb on each side (0.56 m long, 0.20 m wide, 0.04 m proud,
  John 2026-11-12) with a yellow warning triangle (1.0, 0.82, 0.0) just
  forward of it. About 3.0 m, 200 kg (Wikipedia); diameter fitted, 0.42 m.
- `BOZ107` (chaff and flare dispenser, right/starboard outer pylon): 122
  triangles, nine tiny slightly darker discs (0.27, 0.29, 0.31) on the aft
  face as the dispenser outlets: one on the centre and a ring of eight (John
  asked for about triple the first three, 2026-11-12). About 4 m, 325 kg (secondary database);
  diameter fitted, 0.48 m. `DISPENSER_EXIT` in the script is the exit point,
  model (0, -0.5, 0) (the centre of the aft face, out along -Y).
- The two pods' budget is 130 triangles each (John, 2026-11-12); the first
  three keep 100 (`TRIANGLE_LIMITS`).
- Sources, the fitted dimensions and the sides (John's photo, and the Panavia
  GR4 page) are in the script's docstring.
- `--views` (with `--thumbs`) adds side, other-side, underside, rear and
  close-up pictures. Rebuilding reproduces the first three models' triangles
  byte for byte.
