// How much of the sun's disc is in view. One workgroup samples the world
// depth at 64 points over the disc (a golden-angle spiral, the same as
// `disc_point` in lens_flare.rs). Only sky is left at the cleared depth 0:
// terrain, the city, trees, aircraft and the canopy all wrote more. Points off
// the view count as `outside` (the terrain's share, worked out on the CPU).
// The fraction eases into `state[slot]` by `ease`, which the lens flare reads.
struct Probe {
 centre:vec2<f32>,
 radius:f32,
 outside:f32,
 view:vec2<f32>,
 scale:vec2<f32>,
 ease:f32,
 slot:f32,
}
@group(0) @binding(0) var depth:texture_depth_2d;
@group(0) @binding(1) var depth_ms:texture_depth_multisampled_2d;
@group(1) @binding(0) var<uniform> probe:Probe;
@group(1) @binding(1) var<storage,read_write> state:array<f32,8>;
var<workgroup> seen:array<f32,64>;

fn disc_point(i:u32)->vec2<f32> {
 let r=sqrt((f32(i)+0.5)/64.0)*probe.radius;
 let a=f32(i)*2.3999632;
 return probe.centre+r*vec2<f32>(cos(a),sin(a));
}
fn on_view(p:vec2<f32>)->bool {
 return p.x>=0.0&&p.y>=0.0&&p.x<probe.view.x&&p.y<probe.view.y;
}
fn finish(i:u32,value:f32) {
 seen[i]=value;
 workgroupBarrier();
 if i==0u {
  var total=0.0;
  for(var k=0u;k<64u;k++){total+=seen[k];}
  let slot=u32(probe.slot);
  state[slot]=mix(state[slot],total/64.0,probe.ease);
 }
}
@compute @workgroup_size(64) fn measure(@builtin(local_invocation_index) i:u32) {
 let p=disc_point(i);
 var value=probe.outside;
 if on_view(p) {
  let last=vec2<i32>(textureDimensions(depth))-vec2<i32>(1);
  let pixel=min(vec2<i32>(p*probe.scale),last);
  value=select(0.0,1.0,textureLoad(depth,pixel,0)<=0.0);
 }
 finish(i,value);
}
@compute @workgroup_size(64) fn measure_ms(@builtin(local_invocation_index) i:u32) {
 let p=disc_point(i);
 var value=probe.outside;
 if on_view(p) {
  let last=vec2<i32>(textureDimensions(depth_ms))-vec2<i32>(1);
  let pixel=min(vec2<i32>(p*probe.scale),last);
  let sample=i32(i%textureNumSamples(depth_ms));
  value=select(0.0,1.0,textureLoad(depth_ms,pixel,sample)<=0.0);
 }
 finish(i,value);
}
