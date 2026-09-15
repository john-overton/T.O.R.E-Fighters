struct Scene { eye:vec4<f32>, right:vec4<f32>, up:vec4<f32>, forward:vec4<f32>, sky:vec4<f32>, fog:vec4<f32>, deck_a:vec4<f32>, deck_b:vec4<f32> }
@group(0) @binding(0) var<uniform> scene:Scene;
// Retail terrain and sky artwork is stored as weather-palette indices, so it is
// uploaded unresolved and the live palette is applied here every frame.
@group(0) @binding(1) var tiles:texture_2d_array<u32>;
@group(0) @binding(2) var palette:texture_2d<f32>;
// 0x4b3410: haze density is a piecewise-linear ramp between two recovered
// distances, flat outside them. Imported shade tables supply discrete index
// remaps before color lookup for terrain; authored-color effects remain separate.
fn haze(distance:f32)->f32{
 if distance<=scene.fog.x { return scene.fog.z; }
 if distance>=scene.fog.y { return scene.fog.w; }
 return scene.fog.z+(scene.fog.w-scene.fog.z)*(distance-scene.fog.x)/(scene.fog.y-scene.fog.x);
}
struct VertexOut {
 @builtin(position) clip:vec4<f32>, @location(0) uv:vec2<f32>,
 @location(1) @interpolate(flat) layer:f32, @location(2) color:vec3<f32>, @location(3) distance:f32, @location(4) @interpolate(flat) own_color:f32
}
fn linear(c:vec3<f32>)->vec3<f32>{return pow((c+vec3<f32>(0.055))/1.055,vec3<f32>(2.4));}
// Index 255 is the native water/cutout test at 0x4aa739 and stays transparent.
fn fog_row(distance:f32)->i32{
 if textureDimensions(palette).y<=1u { return 0; }
 let levels=i32(scene.forward.w);
 return 1+clamp(i32(clamp(haze(distance),0.0,1.0)*f32(levels)),0,levels-1);
}
fn shade(index:u32,row:i32)->vec4<f32>{
 let c=textureLoad(palette,vec2<i32>(i32(index),row),0).rgb;
 return vec4<f32>(linear(c),select(1.0,0.0,index==255u));
}
// Manual bilinear: indices cannot be filtered, so each of the four texels is
// resolved through the palette first and the colors are blended premultiplied.
fn tile(uv:vec2<f32>,layer:i32,row:i32)->vec4<f32>{
 let size=vec2<i32>(textureDimensions(tiles));
 let p=uv*vec2<f32>(size)-vec2<f32>(0.5);
 let base=floor(p);
 let f=p-base;
 var sum=vec4<f32>(0.0);
 for(var j=0;j<2;j++){
  for(var i=0;i<2;i++){
   let at=clamp(vec2<i32>(base)+vec2<i32>(i,j),vec2<i32>(0),size-vec2<i32>(1));
   let c=shade(textureLoad(tiles,at,layer,0).r,row);
   let w=select(1.0-f.x,f.x,i==1)*select(1.0-f.y,f.y,j==1);
   sum+=vec4<f32>(c.rgb*c.a,c.a)*w;
  }
 }
 if sum.a<=0.0 { return vec4<f32>(0.0); }
 return vec4<f32>(sum.rgb/sum.a,sum.a);
}
@vertex fn vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>,@location(4) index:f32)->VertexOut {
 let p=position-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 let near=1.0;let far=2200000.0;let f=1.7320508*scene.up.w;
 var out:VertexOut;
 out.clip=vec4<f32>(dot(p,scene.right.xyz)*f/scene.eye.w,dot(p,scene.up.xyz)*f,far/(far-near)*z-near*far/(far-near),z);
 out.uv=uv;out.layer=layer;out.own_color=select(0.0,1.0,index<0.0);
 // A negative index means the vertex carries its own color; terrain carries a
 // source palette index instead, resolved per frame and then Gouraud blended.
 if index>=0.0 { out.color=shade(u32(index),fog_row(length(p))).rgb; } else { out.color=linear(color); }
 out.distance=length(p);return out;
}
@fragment fn fragment(in:VertexOut)->@location(0) vec4<f32>{
 var color=in.color;
 if in.layer>=0.0 || in.layer == -2.0 {
  let tex=tile(in.uv,i32(max(in.layer,0.0)),fog_row(in.distance));
  if in.layer == -2.0 && tex.a < 0.5 { discard; }
  color=mix(color,tex.rgb,tex.a);
 }
 if textureDimensions(palette).y<=1u || (in.own_color>0.0 && in.layer<0.0 && in.layer != -2.0) { color=mix(color,linear(scene.sky.rgb),haze(in.distance)); }
 return vec4<f32>(color,1.0);
}
struct SkyOut { @builtin(position) clip:vec4<f32>, @location(0) screen:vec2<f32> }
@vertex fn sky_vertex(@builtin(vertex_index) i:u32)->SkyOut {
 let p=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0));
 var out:SkyOut;out.clip=vec4<f32>(p[i],1.0,1.0);out.screen=p[i];return out;
}
@fragment fn sky_fragment(in:SkyOut)->@location(0) vec4<f32>{
 let ray=normalize(scene.forward.xyz+scene.right.xyz*in.screen.x*scene.eye.w/(1.7320508*scene.up.w)+scene.up.xyz*in.screen.y/(1.7320508*scene.up.w));
 // Source deck planes: world feet, power-of-two tiling, reversed north axis.
 // The GPU ray/plane intersection replaces the source scanline rasterizer.
 var color=linear(scene.sky.rgb);
 let decks=array<vec4<f32>,2>(scene.deck_a,scene.deck_b);
 var nearest=1e30;
 for(var i=0;i<2;i++){
  let deck=decks[i];
  if deck.z<0.0 || abs(ray.y)<0.000001 { continue; }
  let distance=(deck.x-scene.eye.y)/ray.y;
  if distance<=0.0 || distance>=nearest { continue; }
  let hit=scene.eye.xyz+ray*distance;
  let uv=fract(vec2<f32>(hit.x,-hit.z)/deck.y);
  let tex=tile(uv,i32(deck.z),fog_row(distance));
  color=mix(color,tex.rgb,tex.a);
  nearest=distance;
 }
 return vec4<f32>(color,1.0);
}
struct VaporOut { @builtin(position) clip:vec4<f32>, @location(0) color:vec4<f32>, @location(1) distance:f32 }
@vertex fn vapor_vertex(@location(0) position:vec3<f32>,@location(1) color:vec4<f32>)->VaporOut {
 let p=position-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 let near=1.0;let far=2200000.0;let f=1.7320508*scene.up.w;
 var out:VaporOut;
 out.clip=vec4<f32>(dot(p,scene.right.xyz)*f/scene.eye.w,dot(p,scene.up.xyz)*f,far/(far-near)*z-near*far/(far-near),z);
 out.color=color;out.distance=length(p);return out;
}
@fragment fn vapor_fragment(in:VaporOut)->@location(0) vec4<f32>{
 // Vapor sits in the same atmosphere as everything else, so haze thins it too.
 return vec4<f32>(linear(in.color.rgb),in.color.a*(1.0-haze(in.distance)));
}
