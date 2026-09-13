struct Scene { eye:vec4<f32>, right:vec4<f32>, up:vec4<f32>, forward:vec4<f32>, sky:vec4<f32> }
@group(0) @binding(0) var<uniform> scene:Scene;
@group(0) @binding(1) var tiles:texture_2d_array<f32>;
@group(0) @binding(2) var tile_sampler:sampler;
struct VertexOut {
 @builtin(position) clip:vec4<f32>, @location(0) uv:vec2<f32>,
 @location(1) @interpolate(flat) layer:f32, @location(2) color:vec3<f32>, @location(3) distance:f32
}
@vertex fn vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>)->VertexOut {
 let p=position-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 let near=1.0;let far=2200000.0;let f=1.7320508*scene.up.w;
 var out:VertexOut;
 out.clip=vec4<f32>(dot(p,scene.right.xyz)*f/scene.eye.w,dot(p,scene.up.xyz)*f,far/(far-near)*z-near*far/(far-near),z);
 out.uv=uv;out.layer=layer;out.color=color;out.distance=length(p);return out;
}
fn linear(c:vec3<f32>)->vec3<f32>{return pow((c+vec3<f32>(0.055))/1.055,vec3<f32>(2.4));}
@fragment fn fragment(in:VertexOut)->@location(0) vec4<f32>{
 var color=linear(in.color);
 if in.layer>=0.0 || in.layer == -2.0 {
  let tex=textureSample(tiles,tile_sampler,in.uv,i32(max(in.layer,0.0)));
  if in.layer == -2.0 && tex.a < 0.5 { discard; }
  color=mix(color,tex.rgb,tex.a);
 }
 // First GPU atmosphere: source palette, authored distance fog. Native LAY scheduling pending.
 let fog=1.0-exp(-in.distance*scene.sky.w);
 return vec4<f32>(mix(color,linear(scene.sky.rgb),fog),1.0);
}
struct SkyOut { @builtin(position) clip:vec4<f32>, @location(0) screen:vec2<f32> }
@vertex fn sky_vertex(@builtin(vertex_index) i:u32)->SkyOut {
 let p=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0));
 var out:SkyOut;out.clip=vec4<f32>(p[i],1.0,1.0);out.screen=p[i];return out;
}
@fragment fn sky_fragment(in:SkyOut)->@location(0) vec4<f32>{
 let ray=normalize(scene.forward.xyz+scene.right.xyz*in.screen.x*scene.eye.w/(1.7320508*scene.up.w)+scene.up.xyz*in.screen.y/(1.7320508*scene.up.w));
 let uv=vec2<f32>(fract(atan2(ray.x,ray.z)/6.2831853+0.5),clamp(0.5-asin(ray.y)/3.14159265,0.0,1.0));
 let tex=textureSample(tiles,tile_sampler,uv,i32(scene.right.w)).rgb;
 return vec4<f32>(mix(linear(scene.sky.rgb),tex,smoothstep(0.0,0.4,ray.y)),1.0);
}
