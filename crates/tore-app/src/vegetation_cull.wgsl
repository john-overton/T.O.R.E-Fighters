// Vegetation objects: cull against the view and split full meshes from
// the cheap crossed impostor. See vegetation_renderer.rs.

struct Inst {
  pos: vec3<f32>,
  yaw: f32,
  scale: vec2<f32>,
  radius: f32,
  kind: f32,
  color: vec3<f32>,
  hash: f32,
}
struct Params {
  origin: vec4<f32>,
  eye: vec4<f32>,
  forward: vec4<f32>,
  right: vec4<f32>,
  up: vec4<f32>,
  // Detail distance, draw distance, fade band, minimum pixels.
  tiers: vec4<f32>,
  // Half-view tangents, pixels per unit size, eye altitude (feet).
  view: vec4<f32>,
  // Instance count, slot count, max per slot, impostor slot.
  counts: vec4<u32>,
}
struct Drawn {
  place: vec4<f32>,
  turn: vec4<f32>,
  look: vec4<f32>,
  color: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<Inst>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> drawn: array<Drawn>;
@group(0) @binding(4) var<storage, read_write> args: array<u32>;

@compute @workgroup_size(256) fn cull(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if i >= params.counts.x { return; }
  let inst = instances[i];
  let mesh = u32(inst.kind) % 16u;
  let trunk = select(0.0, 1.0, inst.kind >= 16.0);
  let rel = inst.pos - params.origin.xyz;
  let d = rel + vec3<f32>(0.0, inst.scale.y * 0.5, 0.0) - params.eye.xyz;
  let dist = length(d);
  if dist >= params.tiers.y { return; }
  let r = max(inst.radius, 4.0);
  let z = dot(d, params.forward.xyz);
  if z < -r { return; }
  if abs(dot(d, params.right.xyz)) > z * params.view.x + r * 1.6 { return; }
  if abs(dot(d, params.up.xyz)) > z * params.view.y + r * 1.6 { return; }
  let px = max(inst.scale.y, inst.scale.x) * params.view.z / max(dist, 1.0);
  if px < params.tiers.w { return; }
  // Gone once the camera is high: the retail tile is the view from altitude.
  let alt = 1.0 - smoothstep(4500.0, 10500.0, params.view.w);
  let edge = clamp((params.tiers.y - dist) / max(params.tiers.z, 1.0), 0.0, 1.0);
  let small = clamp((px - params.tiers.w) / max(params.tiers.w, 1.0), 0.0, 1.0);
  let fade = edge * small * alt;
  if fade <= 0.001 { return; }
  // Grass, dunes and hedgerows keep their mesh. Trees, bushes, rocks and
  // dead wood become a crossed card past the detail distance.
  let shaped = mesh == 4u || mesh >= 7u;
  let slot = select(mesh, params.counts.w, dist > params.tiers.x && !shaped);
  let k = atomicAdd(&counts[slot], 1u);
  if k >= params.counts.z { return; }
  let at = slot * params.counts.z + k;
  let c = cos(inst.yaw);
  let s = sin(inst.yaw);
  drawn[at].place = vec4<f32>(rel, inst.hash);
  drawn[at].turn = vec4<f32>(c, s, inst.scale.y, fade);
  drawn[at].look = vec4<f32>(inst.scale.x, trunk, 0.0, 0.0);
  drawn[at].color = vec4<f32>(inst.color, 1.0);
}

@compute @workgroup_size(16) fn finish(@builtin(global_invocation_id) id: vec3<u32>) {
  let p = id.x;
  if p >= params.counts.y { return; }
  let n = min(atomicLoad(&counts[p]), params.counts.z);
  args[p * 4u + 1u] = n;
}
