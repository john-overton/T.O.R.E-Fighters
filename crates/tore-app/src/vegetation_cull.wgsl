// Vegetation objects: thin by distance, cull against the view, and list
// the near ones again as shadow casters. See vegetation_renderer.rs.

struct Inst {
  // Position, yaw.
  a: vec4<f32>,
  // Height, width, ground rise east and north per foot.
  b: vec4<f32>,
  // Tint, rank.
  c: vec4<f32>,
  // Mesh, reach share, radius in feet, hash.
  d: vec4<f32>,
}
struct Mesh {
  // First slot (bits), look, radius over height, slot count (bits).
  info: vec4<f32>,
  // Mean linear colour of a recoloured texture.
  mean: vec4<f32>,
}
struct Params {
  origin: vec4<f32>,
  eye: vec4<f32>,
  forward: vec4<f32>,
  right: vec4<f32>,
  up: vec4<f32>,
  // Half-view tangents, pixels per unit size, eye altitude (feet).
  view: vec4<f32>,
  // Full density distance, draw distance, shadow distance, minimum pixels.
  tiers: vec4<f32>,
  // Instance count, mesh count.
  counts: vec4<u32>,
}
struct Drawn {
  // Position relative to the render origin, hash.
  place: vec4<f32>,
  // Yaw cosine and sine, height, fade.
  turn: vec4<f32>,
  // Width, ground rise east and north, look.
  shape: vec4<f32>,
  tint: vec4<f32>,
  mean: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<Inst>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> drawn: array<Drawn>;
@group(0) @binding(4) var<storage, read_write> casters: array<Drawn>;
@group(0) @binding(5) var<storage, read_write> args: array<u32>;
@group(0) @binding(6) var<storage, read> meshes: array<Mesh>;

// The share of full density kept at `dist` (vegetation.rs `share`).
fn share(dist: f32, full: f32) -> f32 {
  if dist <= full { return 1.0; }
  return pow(full / dist, 1.6);
}

@compute @workgroup_size(256) fn cull(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if i >= params.counts.x { return; }
  let inst = instances[i];
  let mesh = u32(inst.d.x);
  let m = meshes[mesh];
  let first = bitcast<u32>(m.info.x);
  let capacity = bitcast<u32>(m.info.w);
  let height = inst.b.x;
  let rel = inst.a.xyz - params.origin.xyz;
  let d = rel + vec3<f32>(0.0, height * 0.5, 0.0) - params.eye.xyz;
  let dist = length(d);
  if dist >= params.tiers.y { return; }
  // Thin by distance: an object stays while its rank is under the share,
  // fading over the last part, so a farther view keeps a subset.
  var s = share(dist, params.tiers.x * abs(inst.d.y));
  if inst.d.y < 0.0 { s *= smoothstep(0.3, 0.8, dist / params.tiers.x); }
  let rank = inst.c.w;
  if rank >= s { return; }
  let thin = clamp((s - rank) / (0.2 * s + 0.0001), 0.0, 1.0);
  // Gone once the camera is high: the retail tile is the view from altitude.
  let alt = 1.0 - smoothstep(7000.0, 15000.0, params.view.w);
  let edge = clamp((params.tiers.y - dist) / max(params.tiers.y * 0.12, 1.0), 0.0, 1.0);
  let r = max(inst.d.z, height * 0.5);
  let px = max(height, r * 2.0) * params.view.z / max(dist, 1.0);
  if px < params.tiers.w { return; }
  let small = clamp((px - params.tiers.w) / max(params.tiers.w, 1.0), 0.0, 1.0);
  let fade = thin * alt * edge * small;
  if fade <= 0.002 { return; }
  var out: Drawn;
  out.place = vec4<f32>(rel, inst.d.w);
  out.turn = vec4<f32>(cos(inst.a.w), sin(inst.a.w), height, fade);
  out.shape = vec4<f32>(inst.b.y, inst.b.z, inst.b.w, m.info.y);
  out.tint = vec4<f32>(inst.c.xyz, 1.0);
  out.mean = m.mean;
  // Shadow casters: near ones, in view or not (their shadows may be).
  if dist < params.tiers.z && fade > 0.5 {
    let k = atomicAdd(&counts[params.counts.y + mesh], 1u);
    if k < capacity { casters[first + k] = out; }
  }
  let z = dot(d, params.forward.xyz);
  if z < -r { return; }
  if abs(dot(d, params.right.xyz)) > z * params.view.x + r * 1.6 { return; }
  if abs(dot(d, params.up.xyz)) > z * params.view.y + r * 1.6 { return; }
  let k = atomicAdd(&counts[mesh], 1u);
  if k < capacity { drawn[first + k] = out; }
}

@compute @workgroup_size(64) fn finish(@builtin(global_invocation_id) id: vec3<u32>) {
  let p = id.x;
  let n = params.counts.y;
  if p >= n { return; }
  let capacity = bitcast<u32>(meshes[p].info.w);
  args[p * 4u + 1u] = min(atomicLoad(&counts[p]), capacity);
  args[(n + p) * 4u + 1u] = min(atomicLoad(&counts[n + p]), capacity);
}
