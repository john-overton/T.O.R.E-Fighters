struct Band { info:vec4<f32>, ramp:vec4<f32> }
struct Scene { eye:vec4<f32>, right:vec4<f32>, up:vec4<f32>, forward:vec4<f32>, sky:vec4<f32>, fog:vec4<f32>, deck_a:vec4<f32>, deck_b:vec4<f32>, sun:vec4<f32>, circles:array<vec4<f32>,8>, ray:vec4<f32>, bands:array<Band,32> }
@group(0) @binding(0) var<uniform> scene:Scene;
// Retail terrain and sky artwork is stored as weather-palette indices, so it is
// uploaded unresolved and the live palette is applied here every frame.
@group(0) @binding(1) var tiles:texture_2d_array<u32>;
@group(0) @binding(2) var palette:texture_2d<f32>;
@group(0) @binding(3) var weather_tiles:texture_2d_array<u32>;
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
 @location(1) @interpolate(flat) layer:f32, @location(2) color:vec3<f32>, @location(3) distance:f32, @location(4) @interpolate(flat) own_color:f32, @location(5) altitude:f32, @location(6) direction:vec3<f32>
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
// FA 0x4b31f0: target remap first, then view remap. Adjacent layers
// split integer distance at the source high-altitude boundary.
fn band_at(altitude:i32)->i32 {
 for(var i=0;i<i32(scene.ray.x);i++){
  if f32(altitude)>=scene.bands[i].info.x && f32(altitude)<=scene.bands[i].info.y {return i;}
 }
 return -1;
}
fn restrict_ramp(r:vec4<i32>,v:vec4<i32>)->vec4<i32>{return vec4<i32>(min(r.x,v.x),max(r.y,v.y),min(r.z,v.z),max(r.w,v.w));}
fn ramp_row(info:vec4<f32>,r:vec4<i32>,distance:i32)->i32 {
 var density=r.y;
 if distance>r.x {if distance>=r.z {density=r.w;}else {density=r.y+(r.w-r.y)*(distance-r.x)/(r.z-r.x);}}
 return i32(info.z)+min((clamp(density,0,256)*i32(info.w))>>8,i32(info.w)-1);
}
fn ray_rows(distance:f32,altitude:f32)->vec2<i32>{
 if textureDimensions(palette).y<=1u {return vec2<i32>(-1);}
 let view=max(0,i32(floor(scene.eye.y)));let target_alt=max(0,i32(floor(altitude)));
 let vi=band_at(view);let ti=band_at(target_alt);
 if vi<0 || ti<0 {return vec2<i32>(-1);}
 let overlap=vi+1<i32(scene.ray.x) && scene.bands[vi+1].info.x<=f32(view);
 var vr=vec4<i32>(scene.bands[vi].ramp);var tr=vec4<i32>(scene.bands[ti].ramp);
 if overlap {
  let blended=vec4<i32>(i32(scene.fog.x/256.0),i32(scene.fog.z*256.0),i32(scene.fog.y/256.0),i32(scene.fog.w*256.0));
  vr=restrict_ramp(vr,blended);tr=restrict_ramp(tr,blended);
 }
 let distance_units=i32(max(0.0,floor(distance*256.0)+scene.ray.y))>>16;
 let d=(distance_units<<16)>>16;
 var vd=d;var td=0;
 if vi!=ti {
  if ti+1<vi || ti>vi+1 || (overlap && ti<vi) {vd=65535;td=65535;}
  else if vi>ti {vd=(view-i32(scene.bands[ti].info.y))*d/(view-target_alt);td=d-vd;}
  else {td=(target_alt-i32(scene.bands[vi].info.y))*d/(target_alt-view);vd=d-td;}
 }
 var row=-1;if td>0 {row=ramp_row(scene.bands[ti].info,tr,td);}
 return vec2<i32>(ramp_row(scene.bands[vi].info,vr,vd),row);
}
fn ray_index(index:u32,rows:vec2<i32>)->u32 {
 var result=index;
 if rows.y>=0 {result=textureLoad(weather_tiles,vec2<i32>(i32(result),rows.y),i32(scene.deck_a.w),0).r;}
 if rows.x>=0 {result=textureLoad(weather_tiles,vec2<i32>(i32(result),rows.x),i32(scene.deck_a.w),0).r;}
 return result;
}
// Manual bilinear: indices cannot be filtered, so each of the four texels is
// resolved through the palette first and the colors are blended premultiplied.
fn sample_tile(uv:vec2<f32>,layer:i32,row:i32,sun_passes:i32,core:i32,remaps:vec2<i32>)->vec4<f32>{
 let size=vec2<i32>(textureDimensions(tiles));
 let p=uv*vec2<f32>(size)-vec2<f32>(0.5);
 let base=floor(p);
 let f=p-base;
 var sum=vec4<f32>(0.0);
 for(var j=0;j<2;j++){
  for(var i=0;i<2;i++){
   let at=clamp(vec2<i32>(base)+vec2<i32>(i,j),vec2<i32>(0),size-vec2<i32>(1));
   let original=textureLoad(tiles,at,layer,0).r;
   var index=original;
   var palette_row=row;
   if sun_passes>=0 {
    index=textureLoad(tiles,vec2<i32>(i32(index),i32(scene.deck_b.w)+row-1),i32(scene.deck_a.w),0).r;
    for(var n=0;n<sun_passes;n++){ index=textureLoad(tiles,vec2<i32>(i32(index),0),i32(scene.deck_a.w),0).r; }
    if core>=0 {index=u32(core);}
    palette_row=0;
   }
   if remaps.x>=0 {index=ray_index(index,remaps);palette_row=0;}
   var c=shade(index,palette_row);
   // Cutout belongs to the original texel, before any shade remap.
   c.a=select(1.0,0.0,original==255u);
   let w=select(1.0-f.x,f.x,i==1)*select(1.0-f.y,f.y,j==1);
   sum+=vec4<f32>(c.rgb*c.a,c.a)*w;
  }
 }
 if sum.a<=0.0 { return vec4<f32>(0.0); }
 return vec4<f32>(sum.rgb/sum.a,sum.a);
}
fn tile(uv:vec2<f32>,layer:i32,row:i32)->vec4<f32>{return sample_tile(uv,layer,row,-1,-1,vec2<i32>(-1));}
@vertex fn vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>,@location(4) index:f32)->VertexOut {
 let p=position-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 let near=1.0;let far=2200000.0;let f=1.7320508*scene.up.w;
 var out:VertexOut;
 out.clip=vec4<f32>(dot(p,scene.right.xyz)*f/scene.eye.w,dot(p,scene.up.xyz)*f,far/(far-near)*z-near*far/(far-near),z);
 out.direction=p;out.altitude=position.y;out.uv=uv;out.layer=layer;out.own_color=select(0.0,1.0,index<0.0);
 // A negative index means the vertex carries its own color; terrain carries a
 // source palette index instead, resolved per frame and then Gouraud blended.
 if index>=0.0 { out.color=shade(ray_index(u32(index),ray_rows(length(p),position.y)),0).rgb; } else { out.color=linear(color); }
 out.distance=length(p);return out;
}
@fragment fn fragment(in:VertexOut)->@location(0) vec4<f32>{
 var color=in.color;
 if in.layer>=0.0 || in.layer == -2.0 {
  let tex=sample_tile(in.uv,i32(max(in.layer,0.0)),0,-1,-1,ray_rows(in.distance,in.altitude));
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
// GouraudHorizon has camera-relative horizontal depth 32767/32,
// upper Y=130..0 and lower Y=5..-clamp(130*alt/15000,10,130).
// Interpolate palette indices, as the native indexed Gouraud program does.
fn horizon_height(ray:vec3<f32>)->f32 {
 var head=scene.forward.xz;
 if length(head)<0.0001 {head=scene.up.xz;}
 return ray.y*1024.0/max(0.0001,dot(ray.xz,normalize(head)));
}
fn horizon_index(ray:vec3<f32>)->u32 {
 let y=horizon_height(ray);
 let flags=i32(scene.ray.w);
 if (flags&2)!=0 && y<=5.0 {return u32(clamp(mix(237.0,252.0,clamp((5.0-y)/(5.0+scene.ray.z),0.0,1.0)),0.0,254.0));}
 if (flags&1)!=0 && y>=0.0 {return u32(mix(236.0,229.0,clamp(y/130.0,0.0,1.0)));}
 return 240u;
}
@fragment fn sky_fragment(in:SkyOut)->@location(0) vec4<f32>{
 let ray=normalize(scene.forward.xyz+scene.right.xyz*in.screen.x*scene.eye.w/(1.7320508*scene.up.w)+scene.up.xyz*in.screen.y/(1.7320508*scene.up.w));
 // Source deck planes: world feet, power-of-two tiling, reversed north axis.
 // The GPU ray/plane intersection replaces the source scanline rasterizer.
 var passes=0;var core=-1;
 if scene.sun.w>0.0 && ray.y>=0.0 && !((i32(scene.ray.w)&2)!=0 && horizon_height(ray)<=5.0) {
  let cosine=dot(ray,scene.sun.xyz);
  if cosine>0.0 {
   let tangent=sqrt(max(0.0,1.0-cosine*cosine))/cosine;
   for(var n=0;n<i32(scene.sun.w);n++){
    if tangent<=scene.circles[n].x {
     if scene.circles[n].y==267.0 { passes++; } else {core=i32(scene.circles[n].y);passes=0;}
    }
   }
  }
 }
 var background=horizon_index(ray);
 for(var n=0;n<passes;n++){background=textureLoad(tiles,vec2<i32>(i32(background),0),i32(scene.deck_a.w),0).r;}
 if core>=0 {background=u32(core);}
 var color=shade(background,0).rgb;
 let decks=array<vec4<f32>,2>(scene.deck_a,scene.deck_b);
 var nearest=1e30;
 for(var i=0;i<2;i++){
  let deck=decks[i];
  if deck.z<0.0 || abs(ray.y)<0.000001 { continue; }
  let distance=(deck.x-scene.eye.y)/ray.y;
  if distance<=0.0 || distance>=nearest || distance>=2000000.0 { continue; }
  let hit=scene.eye.xyz+ray*distance;
  let uv=fract(vec2<f32>(hit.x,-hit.z)/deck.y);
  let tex=sample_tile(uv,i32(deck.z),fog_row(distance),passes,core,vec2<i32>(-1));
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

@vertex fn celestial_vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>,@location(4) index:f32)->VertexOut {
 let z=dot(position,scene.forward.xyz);let f=1.7320508*scene.up.w;
 var out:VertexOut;
 out.clip=vec4<f32>(dot(position,scene.right.xyz)*f/scene.eye.w,dot(position,scene.up.xyz)*f,z,z);
 out.uv=uv;out.layer=layer;out.color=shade(u32(index),0).rgb;out.distance=position.y;out.own_color=0.;out.altitude=position.y;out.direction=position;return out;
}
@fragment fn celestial_fragment(in:VertexOut)->@location(0) vec4<f32>{
 if in.distance<0.0 || ((i32(scene.ray.w)&2)!=0 && horizon_height(normalize(in.direction))<=5.0) {discard;}
 if in.layer>=0.0 {let tex=tile(in.uv,i32(in.layer),0);if tex.a<0.01 {discard;}return tex;}
 return vec4<f32>(in.color,1.0);
}

@fragment fn cloud_fragment(in:VertexOut)->@location(0) vec4<f32>{
 let tex=sample_tile(in.uv,i32(in.layer),0,-1,-1,ray_rows(in.distance,in.altitude));
 if tex.a<0.5 {discard;}
 return vec4<f32>(tex.rgb,1.0);
}
