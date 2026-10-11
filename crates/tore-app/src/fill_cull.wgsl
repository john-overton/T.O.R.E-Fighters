// CF0 city fill: per view, every building is culled and given its tier on
// the GPU, writing the instance lists the indirect draws read. See
// fill_renderer.rs for the tiers.

struct Inst { pos:vec3<f32>, piece:u32, turn:vec2<f32>, hash:f32, flags:u32 }
struct Piece { offset:u32, color:f32, half_x:f32, half_z:f32, height:f32, radius:f32, p0:f32, p1:f32 }
struct Params {
 // Render origin (world), eye relative to the origin, view axes.
 origin:vec4<f32>, eye:vec4<f32>, forward:vec4<f32>, right:vec4<f32>, up:vec4<f32>,
 // Full detail ends, blend ends, draw distance, sink length (feet).
 tiers:vec4<f32>,
 // Half-view tangents across and up, pixels per unit size at unit distance,
 // the smallest drawn size in pixels.
 view:vec4<f32>,
 // Instances, pieces, the smallest full detail size in pixels (f32 bits),
 // the shadow casting distance in feet (f32 bits).
 counts:vec4<u32>,
}
struct Args { vertex_count:u32, instance_count:u32, first_vertex:u32, first_instance:u32 }

@group(0) @binding(0) var<storage,read> instances:array<Inst>;
@group(0) @binding(1) var<storage,read> pieces:array<Piece>;
@group(0) @binding(2) var<uniform> params:Params;
@group(0) @binding(3) var<storage,read_write> counts:array<atomic<u32>>;
@group(0) @binding(4) var<storage,read_write> full:array<vec4<f32>>;
@group(0) @binding(5) var<storage,read_write> boxes:array<vec4<f32>>;
@group(0) @binding(6) var<storage,read_write> args:array<Args>;
// Every drawn building's box, for the shadow maps.
@group(0) @binding(7) var<storage,read_write> shadows:array<vec4<f32>>;

@compute @workgroup_size(256) fn cull(@builtin(global_invocation_id) id:vec3<u32>) {
 let i=id.x;
 if i>=params.counts.x {return;}
 let inst=instances[i];
 if (inst.flags&1u)!=0u {return;}
 let piece=pieces[inst.piece];
 let rel=inst.pos-params.origin.xyz;
 let d=rel+vec3<f32>(0.0,piece.height*0.5,0.0)-params.eye.xyz;
 let dist=length(d);
 if dist>=params.tiers.z {return;}
 let r=piece.radius;
 let z=dot(d,params.forward.xyz);
 if z< -r {return;}
 if abs(dot(d,params.right.xyz))>z*params.view.x+r*1.5 {return;}
 if abs(dot(d,params.up.xyz))>z*params.view.y+r*1.5 {return;}
 // Below the smallest drawn size on screen: not drawn at all.
 if max(2.0*r,piece.height)*params.view.z<params.view.w*max(dist,1.0) {return;}
 var share=clamp((params.tiers.z-dist)/params.tiers.w,0.0,1.0);
 if (inst.flags&2u)!=0u {share*=0.08;}
 // Inside the blend band each building turns to full detail at its own
 // distance (its fixed hash), so the band fills in evenly with no blending.
 let band=max(params.tiers.y-params.tiers.x,1.0);
 var detailed=dist<params.tiers.x || (dist<params.tiers.y && inst.hash<(params.tiers.y-dist)/band);
 // Smaller on screen than the detail size: the far box instead.
 if max(2.0*r,piece.height)*params.view.z<bitcast<f32>(params.counts.z)*max(dist,1.0) {detailed=false;}
 // Drawn buildings inside the shadow distance cast their box's shadow.
 if dist<bitcast<f32>(params.counts.w) {
  let k=atomicAdd(&counts[params.counts.y+1u],1u);
  let at=k*3u;
  shadows[at]=vec4<f32>(rel,piece.color);
  shadows[at+1u]=vec4<f32>(inst.turn,share*piece.height,0.0);
  shadows[at+2u]=vec4<f32>(piece.half_x,piece.half_z,0.0,0.0);
 }
 if detailed {
  let k=atomicAdd(&counts[inst.piece],1u);
  let at=(piece.offset+k)*2u;
  full[at]=vec4<f32>(rel,0.0);
  full[at+1u]=vec4<f32>(inst.turn,share,0.0);
 } else {
  let k=atomicAdd(&counts[params.counts.y],1u);
  let at=k*3u;
  boxes[at]=vec4<f32>(rel,piece.color);
  boxes[at+1u]=vec4<f32>(inst.turn,share*piece.height,0.0);
  boxes[at+2u]=vec4<f32>(piece.half_x,piece.half_z,0.0,0.0);
 }
}

// The indirect draws' instance counts: each piece's solid and textured
// draws, then the far boxes, then the shadow boxes.
@compute @workgroup_size(64) fn finish(@builtin(global_invocation_id) id:vec3<u32>) {
 let p=id.x;
 let pieces_n=params.counts.y;
 if p>pieces_n+1u {return;}
 let n=atomicLoad(&counts[p]);
 if p<pieces_n {
  args[2u*p].instance_count=n;
  args[2u*p+1u].instance_count=n;
 } else {
  args[pieces_n+p].instance_count=n;
 }
}
