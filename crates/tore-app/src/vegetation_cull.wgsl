// Vegetation impostors: distance and view culling into one atlas draw.

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
  top: vec4<f32>,
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
  top: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<Inst>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> drawn: array<Drawn>;
@group(0) @binding(4) var<storage, read_write> casters: array<Drawn>;
@group(0) @binding(5) var<storage, read_write> args: array<u32>;
@group(0) @binding(6) var<storage, read> meshes: array<Mesh>;

@group(0) @binding(7) var<storage, read> chunks: array<vec2<u32>>;

// The share of full density kept at `dist` (vegetation.rs `share`).
fn share(dist: f32, full: f32) -> f32 {
  if dist <= full { return 1.0; }
  return pow(full / dist, 1.6);
}

@compute @workgroup_size(256) fn cull(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) local: u32) {
  if group.x >= params.counts.z { return; }
  let chunk = chunks[group.x];
  if local >= chunk.y { return; }
  let i = chunk.x + local;
  let inst = instances[i];
  let mesh = u32(inst.d.x);
  let m = meshes[mesh];
  let first = bitcast<u32>(m.info.x);
  let capacity = bitcast<u32>(m.info.w);
  let height = inst.b.x;
  let rel = inst.a.xyz - params.origin.xyz;
  let d = rel + vec3<f32>(0.0, height * 0.5, 0.0) - params.eye.xyz;
  let dist = length(d.xz);
  if dist >= params.tiers.y { return; }
  // Full population and full coverage throughout the detail circle. No
  // rank, projected-pixel, altitude or species fade can leave a near ghost.
  let edge = 1.0 - smoothstep(params.tiers.x, max(params.tiers.x + 1.0, params.tiers.y), dist);
  let fade = select(edge, 1.0, dist <= params.tiers.x);
  if fade <= 0.0 { return; }
  let r = max(inst.d.z, height * 0.5);
  let z = dot(d, params.forward.xyz);
  if z < -r { return; }
  if abs(dot(d, params.right.xyz)) > z * params.view.x + r * 1.6 { return; }
  if abs(dot(d, params.up.xyz)) > z * params.view.y + r * 1.6 { return; }
  var out: Drawn;
  out.place = vec4<f32>(rel, inst.d.w);
  out.turn = vec4<f32>(cos(inst.a.w), sin(inst.a.w), height, fade);
  out.shape = vec4<f32>(inst.b.y, inst.b.z, inst.b.w, m.info.y);
  out.tint = vec4<f32>(inst.c.xyz, m.info.z * 1.02);
  out.mean = m.mean;
  out.top = m.top;
  let k = atomicAdd(&counts[0], 1u);
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
