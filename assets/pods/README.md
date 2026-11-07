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
