struct Band { info:vec4<f32>, ramp:vec4<f32> }
struct Scene { eye:vec4<f32>, right:vec4<f32>, up:vec4<f32>, forward:vec4<f32>, sky:vec4<f32>, fog:vec4<f32>, deck_a:vec4<f32>, deck_b:vec4<f32>, sun:vec4<f32>, circles:array<vec4<f32>,8>, ray:vec4<f32>, bands:array<Band,32>, ocean:vec4<f32>, cloud_reflection:vec4<f32> }
@group(0) @binding(0) var<uniform> scene:Scene;
// Retail terrain and sky artwork is stored as weather-palette indices, so it is
// uploaded unresolved and the live palette is applied here every frame.
@group(0) @binding(1) var tiles:texture_2d_array<u32>;
@group(0) @binding(2) var palette:texture_2d<f32>;
@group(0) @binding(3) var weather_tiles:texture_2d_array<u32>;
@group(0) @binding(4) var engine_art:texture_2d<f32>;
// 0x4b3410: haze density is a piecewise-linear ramp between two recovered
// distances, flat outside them. Imported shade tables supply discrete index
// remaps before color lookup for terrain; authored-color effects remain separate.
fn haze(distance:f32)->f32{
 if distance<=scene.fog.x { return scene.fog.z; }
 if distance>=scene.fog.y { return scene.fog.w; }
 return scene.fog.z+(scene.fog.w-scene.fog.z)*(distance-scene.fog.x)/(scene.fog.y-scene.fog.x);
}
// Opinionated aerial perspective, docs/spec/atmospheric-distance.md.
// Integral of a 500-foot smoothstep, including its constant tail.
fn air_step_integral(height:f32)->f32 {
 let t=clamp(height/500.0,0.0,1.0);
 return 500.0*(t*t*t-0.5*t*t*t*t)+max(height-500.0,0.0);
}
fn air_band_weight(height:f32,lo:f32,hi:f32)->f32 {
 return smoothstep(lo-250.0,lo+250.0,height)-smoothstep(hi-250.0,hi+250.0,height);
}
fn air_band_integral(height:f32,lo:f32,hi:f32)->f32 {
 return air_step_integral(height-lo+250.0)-air_step_integral(height-hi+250.0);
}
fn air_opacity(distance:f32,altitude:f32)->f32 {
 let a=max(scene.eye.y,0.0);let b=max(altitude,0.0);
 let low=min(a,b);let high=max(a,b);let span=high-low;
 var density=exp(-(a+b)*0.5/18000.0);
 if span>1.0 {density=18000.0*(exp(-low/18000.0)-exp(-high/18000.0))/span;}
 var moisture_rate=0.0;
 for(var i=0;i<i32(scene.ray.x);i++) {
  let band=scene.bands[i];
  // Strong source visibility ramps add moisture; clear-day ramps add none.
  let source=clamp(band.ramp.w/256.0,0.0,1.0)/max(band.ramp.z*256.0,1000.0);
  let moisture=min(max(source-0.8/182283.0,0.0)*0.20,1.0/12000.0);
  if moisture<=0.0 {continue;}
  var weight=air_band_weight((a+b)*0.5,band.info.x,band.info.y);
  if span>1.0 {weight=(air_band_integral(high,band.info.x,band.info.y)-air_band_integral(low,band.info.x,band.info.y))/span;}
  moisture_rate+=moisture*clamp(weight,0.0,1.0);
 }
 let optical_depth=max(distance-264000.0,0.0)*density/900000.0
     +max(distance-3000.0,0.0)*moisture_rate;
 return 1.0-exp(-optical_depth);
}
// Dense weather must hide surface contrast, including palette light pixels.
fn cloud_occlusion(color:vec3<f32>,direction:vec3<f32>,altitude:f32)->vec3<f32> {
 if !smooth_weather() {return color;}
 let a=max(scene.eye.y,0.0);let b=max(altitude,0.0);
 let low=min(a,b);let high=max(a,b);let span=high-low;
 var depth=0.0;
 for(var i=0;i<i32(scene.ray.x);i++) {
  let band=scene.bands[i];
  let dense=smoothstep(0.9,1.0,band.ramp.w/256.0)
      *(1.0-smoothstep(8000.0,16000.0,band.ramp.z*256.0));
  if dense<=0.0 {continue;}
  var weight=air_band_weight((a+b)*0.5,band.info.x,band.info.y);
  if span>1.0 {weight=(air_band_integral(high,band.info.x,band.info.y)-air_band_integral(low,band.info.x,band.info.y))/span;}
  let d=length(direction)*clamp(weight,0.0,1.0)*dense/150.0;
  depth+=d;
 }
 if depth<=0.0 {return color;}
 let transmission=exp(-depth)*(1.0-smoothstep(3.0,4.0,depth));
 return mix(horizon_color(240.0,0,-1),color,transmission);
}
fn aerial_perspective(color:vec3<f32>,direction:vec3<f32>,altitude:f32)->vec3<f32> {
 if !smooth_weather() {return color;}
 let horizontal=vec3<f32>(direction.x,0.0,direction.z);
 let horizon=horizon_color(horizon_index(horizontal),0,-1);
 return cloud_occlusion(mix(color,horizon,air_opacity(length(direction),altitude)),direction,altitude);
}
struct VertexOut {
 @builtin(position) clip:vec4<f32>, @location(0) uv:vec2<f32>,
 @location(1) @interpolate(flat) layer:f32, @location(2) color:vec3<f32>, @location(3) distance:f32, @location(4) @interpolate(flat) own_color:f32, @location(5) altitude:f32, @location(6) direction:vec3<f32>, @location(7) @interpolate(flat) fog_enabled:u32, @location(8) @interpolate(flat) light_row:i32
}
// Exact sRGB decoding includes the dark linear segment; source black stays zero.
fn linear(c:vec3<f32>)->vec3<f32>{return select(pow((c+vec3<f32>(0.055))/1.055,vec3<f32>(2.4)),c/12.92,c<=vec3<f32>(0.04045));}
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
fn native_ray_rows(distance:f32,altitude:f32)->vec2<i32>{
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
// Authored presentation mode: interpolate neighboring original remap results.
fn smooth_weather()->bool {return (i32(scene.ray.w)&8)!=0;}
fn continuous_row(info:vec4<f32>,r:vec4<f32>,distance:f32)->f32 {
 var density=r.y;
 if distance>r.x {if distance>=r.z {density=r.w;}else {density=mix(r.y,r.w,(distance-r.x)/(r.z-r.x));}}
 return info.z+clamp(density/256.0*info.w,0.0,info.w-1.0);
}
fn ray_rows(distance:f32,altitude:f32)->vec2<f32>{
 if !smooth_weather() {return vec2<f32>(native_ray_rows(distance,altitude));}
 let view=max(scene.eye.y,0.0);let target_height=max(altitude,0.0);
 let vi=band_at(i32(view));let ti=band_at(i32(target_height));
 if vi<0 || ti<0 {return vec2<f32>(-1.0);}
 let overlap=vi+1<i32(scene.ray.x) && scene.bands[vi+1].info.x<=view;
 var vr=scene.bands[vi].ramp;var tr=scene.bands[ti].ramp;
 if overlap {
  let r=vec4<f32>(scene.fog.x/256.0,scene.fog.z*256.0,scene.fog.y/256.0,scene.fog.w*256.0);
  vr=vec4<f32>(min(vr.x,r.x),max(vr.y,r.y),min(vr.z,r.z),max(vr.w,r.w));
  tr=vec4<f32>(min(tr.x,r.x),max(tr.y,r.y),min(tr.z,r.z),max(tr.w,r.w));
 }
 let d=max(0.0,(distance+scene.ray.y/256.0)/256.0);
 var vd=d;var td=0.0;
 if vi!=ti {
  if ti+1<vi || ti>vi+1 || (overlap && ti<vi) {vd=65535.0;td=65535.0;}
  else if vi>ti {vd=(view-scene.bands[ti].info.y)*d/(view-target_height);td=d-vd;}
  else {td=(target_height-scene.bands[vi].info.y)*d/(target_height-view);vd=d-td;}
 }
 var row=-1.0;if td>0.0 {row=continuous_row(scene.bands[ti].info,tr,td);}
 return vec2<f32>(continuous_row(scene.bands[vi].info,vr,vd),row);
}
fn remap_color(index:u32,rows:vec2<f32>)->vec3<f32>{
 let lo=vec2<i32>(floor(rows));let hi=vec2<i32>(ceil(rows));let t=fract(rows);
 let a=shade(ray_index(index,lo),0).rgb;
 if all(lo==hi) {return a;}
 let b=shade(ray_index(index,vec2<i32>(hi.x,lo.y)),0).rgb;
 let c=shade(ray_index(index,vec2<i32>(lo.x,hi.y)),0).rgb;
 let d=shade(ray_index(index,hi),0).rgb;
 return mix(mix(a,b,t.x),mix(c,d,t.x),t.y);
}
// Remap one original index before resolving RGB; transparency tests the source.
fn texel(at:vec2<i32>,layer:i32,row:i32,sun_passes:i32,core:i32,remaps:vec2<f32>)->vec4<f32>{
   let original=textureLoad(tiles,at,layer,0).r;
   var index=original;
   var palette_row=row;
   if sun_passes<0 && core>=0 { index=textureLoad(weather_tiles,vec2<i32>(i32(index),core),i32(scene.deck_a.w),0).r; }
   if sun_passes>=0 {
    index=textureLoad(tiles,vec2<i32>(i32(index),i32(scene.deck_b.w)+row-1),i32(scene.deck_a.w),0).r;
    for(var n=0;n<sun_passes;n++){ index=textureLoad(tiles,vec2<i32>(i32(index),0),i32(scene.deck_a.w),0).r; }
    if core>=0 {index=u32(core);}
    palette_row=0;
   }
   var c=shade(index,palette_row);
   if remaps.x>=0 {c=vec4<f32>(remap_color(index,remaps),1.0);}
   // Cutout belongs to the original texel, before any shade remap.
   c.a=select(1.0,0.0,original==255u);
   return c;
}
// Reviewed weather raster reads one texel, without mixing palette colors.
// Float UV projection remains an adaptation of native fixed-point scanlines.
fn weather_tile(uv:vec2<f32>,layer:i32,row:i32,sun_passes:i32,core:i32,remaps:vec2<f32>)->vec4<f32>{
 let size=vec2<i32>(textureDimensions(tiles));
 let at=clamp(vec2<i32>(floor(uv*vec2<f32>(size))),vec2<i32>(0),size-vec2<i32>(1));
 return texel(at,layer,row,sun_passes,core,remaps);
}
// Manual bilinear: indices cannot be filtered, so each of the four texels is
// resolved through the palette first and the colors are blended premultiplied.
fn sample_tile(uv:vec2<f32>,layer:i32,row:i32,sun_passes:i32,core:i32,remaps:vec2<f32>)->vec4<f32>{
 let size=vec2<i32>(textureDimensions(tiles));
 let p=uv*vec2<f32>(size)-vec2<f32>(0.5);
 let base=floor(p);
 let f=p-base;
 var sum=vec4<f32>(0.0);
 for(var j=0;j<2;j++){
  for(var i=0;i<2;i++){
   let at=clamp(vec2<i32>(base)+vec2<i32>(i,j),vec2<i32>(0),size-vec2<i32>(1));
   let c=texel(at,layer,row,sun_passes,core,remaps);
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
 let fog_mode=(u32(max(index,0.0))/256u)%4u;
 out.light_row=i32(max(index,0.0))/1024-1;
 let fog_enabled=fog_mode==0u || (fog_mode==2u && (i32(scene.ray.w)&4)==0);
 out.fog_enabled=select(0u,1u,fog_enabled);
 out.direction=p;out.altitude=position.y;out.uv=uv;out.layer=layer;out.own_color=select(0.0,1.0,index<0.0);
 // A negative index means the vertex carries its own color; terrain carries a
 // source palette index instead, resolved per frame and then Gouraud blended.
 if index>=0.0 { var source_index=u32(index)%256u; if out.light_row>=0 {source_index=textureLoad(weather_tiles,vec2<i32>(i32(source_index),out.light_row),i32(scene.deck_a.w),0).r;} out.color=shade(source_index,0).rgb; if fog_enabled {out.color=remap_color(source_index,ray_rows(length(p),position.y));} } else { out.color=linear(color); }
 out.distance=length(p);return out;
}
// Terrain cutouts expose the already rendered ocean/horizon, never the T2
// land color. Keep this separate from aircraft's base-color texture blending.
@fragment fn terrain_fragment(in:VertexOut)->@location(0) vec4<f32>{
 if in.layer<0.0 {return vec4<f32>(aerial_perspective(in.color,in.direction,in.altitude),1.0);}
 var remaps=vec2<f32>(-1.0);
 if in.fog_enabled!=0u {remaps=ray_rows(in.distance,in.altitude);}
 let tex=sample_tile(in.uv,i32(in.layer),0,-1,in.light_row,remaps);
 // Fitted bilinear coverage boundary; discarded water writes no depth.
 if tex.a<0.5 {discard;}
 return vec4<f32>(aerial_perspective(tex.rgb,in.direction,in.altitude),1.0);
}
// User-requested material. Pink is a mask; metal pixels retain their source RGB.
fn engine_texel(at:vec2<i32>,heat:f32)->vec3<f32> {
 let size=vec2<i32>(textureDimensions(engine_art));
 let c=textureLoad(engine_art,clamp(at,vec2<i32>(0),size-vec2<i32>(1)),0).rgb;
 let mask=smoothstep(0.15,0.5,(min(c.r,c.b)-c.g)/max(max(c.r,c.b),0.001));
 let cold=vec3<f32>(c.r*0.35);
 let red=vec3<f32>(c.r,0.025*c.r,0.008*c.r);
 let core=smoothstep(0.45,0.95,c.r);
 let hot=mix(red,vec3<f32>(1.0,0.92,0.86),core);
 let glow=mix(mix(cold,red,min(heat/0.65,1.0)),hot,smoothstep(0.65,1.0,heat));
 return linear(mix(c,glow,mask));
}
fn engine_color(uv:vec2<f32>,heat:f32)->vec3<f32> {
 let at=uv*vec2<f32>(textureDimensions(engine_art))-vec2<f32>(0.5);
 let base=vec2<i32>(floor(at));let t=fract(at);
 return mix(mix(engine_texel(base,heat),engine_texel(base+vec2<i32>(1,0),heat),t.x),
            mix(engine_texel(base+vec2<i32>(0,1),heat),engine_texel(base+vec2<i32>(1,1),heat),t.x),t.y);
}
@fragment fn fragment(in:VertexOut)->@location(0) vec4<f32>{
 var color=in.color;
 if in.layer<=-3.0 && in.layer>=-4.0 {color=engine_color(in.uv,clamp(-in.layer-3.0,0.0,1.0));}
 if in.layer>=0.0 || in.layer == -2.0 {
  var remaps=vec2<f32>(-1.0);if in.fog_enabled!=0u {remaps=ray_rows(in.distance,in.altitude);}
  let tex=sample_tile(in.uv,i32(max(in.layer,0.0)),0,-1,in.light_row,remaps);
  if in.layer == -2.0 && tex.a < 0.5 { discard; }
  color=mix(color,tex.rgb,tex.a);
 }
 if textureDimensions(palette).y<=1u || (in.own_color>0.0 && in.layer<0.0 && in.layer != -2.0) { color=mix(color,linear(scene.sky.rgb),haze(in.distance)); }
 if in.fog_enabled!=0u {color=aerial_perspective(color,in.direction,in.altitude);}
 else {color=cloud_occlusion(color,in.direction,in.altitude);}
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
fn horizon_index(ray:vec3<f32>)->f32 {
 let y=horizon_height(ray);
 let flags=i32(scene.ray.w);
 if smooth_weather() {
  if y>=0.0 && (flags&1)!=0 {return mix(240.0,229.0,smoothstep(0.0,130.0,y));}
  if y<0.0 && (flags&2)!=0 {return mix(240.0,252.0,smoothstep(0.0,max(scene.ray.z,1.0),-y));}
  return 240.0;
 }
 if (flags&2)!=0 && y<=5.0 {return clamp(mix(237.0,252.0,clamp((5.0-y)/(5.0+scene.ray.z),0.0,1.0)),0.0,254.0);}
 if (flags&1)!=0 && y>=0.0 {return mix(236.0,229.0,clamp(y/130.0,0.0,1.0));}
 return 240.0;
}
// FA 0x447f2f / 0x4481a0 finds two unrolled scanline boundaries:
// deck intersection at 2,000,000 ft, ground intersection at 8,000,000 ft.
// 0x448585 interpolates source indices between their projected screen edges.
// Analytic projection replaces the native 16-step integer search and its 1/2
// pixel edge padding. These are geometry limits, not a fitted haze ramp.
fn deck_transition(ray:vec3<f32>,altitude:f32,endpoint:f32)->f32 {
 let horizontal=length(scene.forward.xz);
 if horizontal<0.0001 {return -1;}
 let depth=dot(ray.xz,scene.forward.xz/horizontal);
 if depth<=0.0 {return -1;}
 let near_slope=(altitude-scene.eye.y)/2000000.0;
 let far_slope=-scene.eye.y/8000000.0;
 let slope=ray.y/depth;
 if slope<min(near_slope,far_slope) || slope>max(near_slope,far_slope) {return -1;}
 let denominators=vec3<f32>(horizontal)+scene.forward.y*vec3<f32>(near_slope,far_slope,slope);
 if any(denominators<=vec3<f32>(0.0001)) {return -1;}
 let projected=(vec3<f32>(near_slope,far_slope,slope)*horizontal-vec3<f32>(scene.forward.y))/denominators;
 if abs(projected.x-projected.y)<0.000001 {return -1;}
 let t=clamp((projected.z-projected.y)/(projected.x-projected.y),0.0,1.0);
 return mix(240.0,endpoint,t);
}
fn sun_index(original:u32,passes:i32,core:i32)->u32 {
 var index=original;
 for(var n=0;n<passes;n++){index=textureLoad(tiles,vec2<i32>(i32(index),0),i32(scene.deck_a.w),0).r;}
 if core>=0 {index=u32(core);}
 return index;
}
fn horizon_color(index:f32,passes:i32,core:i32)->vec3<f32>{
 let lo=u32(floor(index));let hi=min(lo+1u,254u);
 let a=shade(sun_index(lo,passes,core),0).rgb;
 if !smooth_weather() {return a;}
 return mix(a,shade(sun_index(hi,passes,core),0).rgb,fract(index));
}
// Celestial primitives precede the lower horizon/deck draw in 0x4aacf0.
// Clip against those consumers rather than an invented zero-elevation cutoff.
fn celestial_occluded(ray:vec3<f32>)->bool {
 if smooth_weather() {return ray.y<0.0;}
 if (i32(scene.ray.w)&2)!=0 {return horizon_height(ray)<=5.0;}
 if scene.deck_a.z>=0.0 && scene.eye.y>=scene.deck_a.x {
  // SolidHorizon adds the size/inversion offset to its half-Q15 plane.
  // Float camera projection here retains the source rule, not pixel rounding.
  return ray.y+scene.sky.w*dot(ray,scene.forward.xyz)<=0.0;
 }
 if scene.deck_b.z>=0.0 && scene.eye.y>scene.deck_b.x {
  var head=scene.forward.xz;
  if length(head)<0.0001 {head=scene.up.xz;}
  let depth=dot(ray.xz,normalize(head));
  return ray.y<=-scene.eye.y/8000000.0*max(depth,0.0);
 }
 return ray.y<0.0;
}
// User-requested ocean presentation. Resolve indexed art before filtering;
// wrap every bilinear tap so the repeating water has no tile-edge seams.
fn ocean_sample(uv:vec2<f32>,layer:i32,distance:f32,passes:i32,core:i32)->vec4<f32>{
 let size=vec2<i32>(textureDimensions(tiles));
 let p=fract(uv)*vec2<f32>(size)-vec2<f32>(0.5);
 let base=vec2<i32>(floor(p));let f=fract(p);
 var sum=vec4<f32>(0.0);
 var row=f32(fog_row(distance));
 if smooth_weather() {row=1.0+clamp(haze(distance)*scene.forward.w,0.0,scene.forward.w-1.0);}
 for(var y=0;y<2;y++){for(var x=0;x<2;x++){
  let at=(base+vec2<i32>(x,y)+size)%size;
  let a=texel(at,layer,i32(floor(row)),passes,core,vec2<f32>(-1.0));
  var c=a;
  if fract(row)>0.0 {c=mix(a,texel(at,layer,i32(ceil(row)),passes,core,vec2<f32>(-1.0)),fract(row));}
  let w=select(1.0-f.x,f.x,x==1)*select(1.0-f.y,f.y,y==1);
  sum+=vec4<f32>(c.rgb*c.a,c.a)*w;
 }}
 return vec4<f32>(sum.rgb/max(sum.a,0.00001),sum.a);
}
// Authored short-wave slope field. The reference project's useful separation
// is surface normals from water/sky lighting; this implementation uses analytic
// noise gradients rather than importing its FFT, mesh or optical color model.
fn water_hash(p:vec2<f32>)->f32 {
 var q=fract(vec3<f32>(p.xyx)*0.1031);
 q+=dot(q,q.yzx+vec3<f32>(33.33));
 return fract((q.x+q.y)*q.z);
}
fn water_gradient(p:vec2<f32>)->vec2<f32> {
 let angle=water_hash(p)*6.2831853;
 return vec2<f32>(cos(angle),sin(angle));
}
// Gradient noise and analytic derivatives. Unlike value noise, its slopes do
// not flatten at every cell boundary and reveal a regular lattice in reflection.
fn water_noise(p:vec2<f32>)->vec3<f32> {
 let i=floor(p);let f=fract(p);
 let u=f*f*f*(f*(f*6.0-vec2<f32>(15.0))+vec2<f32>(10.0));
 let du=30.0*f*f*(f-vec2<f32>(1.0))*(f-vec2<f32>(1.0));
 let ga=water_gradient(i);let gb=water_gradient(i+vec2<f32>(1,0));
 let gc=water_gradient(i+vec2<f32>(0,1));let gd=water_gradient(i+vec2<f32>(1,1));
 let a=dot(ga,f);let b=dot(gb,f-vec2<f32>(1,0));
 let c=dot(gc,f-vec2<f32>(0,1));let d=dot(gd,f-vec2<f32>(1,1));
 let g=mix(mix(ga,gb,u.x),mix(gc,gd,u.x),u.y);
 return vec3<f32>(mix(mix(a,b,u.x),mix(c,d,u.x),u.y),
                  g.x+du.x*mix(b-a,d-c,u.y),g.y+du.y*mix(c-a,d-b,u.x));
}
fn water_slopes(world:vec2<f32>,footprint:f32)->vec2<f32> {
 let t=scene.ocean.x*6.2831853;
 let along=vec2<f32>(0.8,0.6);let across=vec2<f32>(-0.6,0.8);
 let p=vec2<f32>(dot(world,along),dot(world,across));
 let n=water_noise(p/vec2<f32>(100.0,40.0)+vec2<f32>(cos(t/12.0),sin(t/12.0))*0.25);
 let m=water_noise(p/vec2<f32>(52.0,25.0)+vec2<f32>(sin(t/8.0),cos(t/8.0))*0.20+vec2<f32>(17.3));
 // Gradually reduce unresolved contrast, with no pixel-size cutoff or larger waves.
 let coarse=inverseSqrt(1.0+footprint/80.0);
 let fine=inverseSqrt(1.0+footprint/40.0);
 return (along*n.y*0.16+across*n.z*0.40)*coarse
       +(along*m.y*0.09+across*m.z*0.18)*fine;
}
// Sun energy spans the visible water, independent of the short-ripple fade.
fn water_sun(color:vec3<f32>,ray:vec3<f32>,hit:vec3<f32>,distance:f32)->vec3<f32> {
 if !smooth_weather() || scene.sun.w<=0.0 {return color;}
 for(var i=0;i<i32(scene.ray.x);i++) {
  let band=scene.bands[i];
  if band.info.y>hit.y && band.ramp.w>=256.0 && band.ramp.z*256.0<=8000.0 {return color;}
 }
 var radius=0.0;var index=254u;
 for(var i=0;i<i32(scene.sun.w);i++) {
  if scene.circles[i].y!=267.0 && scene.circles[i].x>radius {
   radius=scene.circles[i].x;index=u32(scene.circles[i].y);
  }
 }
 if radius<=0.0 {return color;}
 let angular_radius=atan(radius);
 let elevation=asin(clamp(scene.sun.y,-1.0,1.0));
 let q=clamp(elevation/angular_radius,-1.0,1.0);
 let visible=(acos(-q)+q*sqrt(max(0.0,1.0-q*q)))/3.14159265;
 if visible<=0.0 {return color;}
 let footprint=distance*scene.ocean.w/max(abs(ray.y),0.025);
 let slope=water_slopes(hit.xz,footprint);
 let reflected=reflect(ray,normalize(vec3<f32>(-slope.x,1.0,-slope.y)));
 let angle=acos(clamp(dot(reflected,scene.sun.xyz),-1.0,1.0));
 let direct=(1.0-smoothstep(angular_radius*0.8,angular_radius*1.2,angle))*smoothstep(0.0,0.01,reflected.y);
 let flat=vec3<f32>(ray.x,-ray.y,ray.z);
 let azimuth=acos(clamp(dot(flat.xz,scene.sun.xz)/max(length(flat.xz)*length(scene.sun.xz),0.00001),-1.0,1.0));
 let vertical=asin(clamp(flat.y,-1.0,1.0))-max(elevation,0.0);
 let scatter=0.55*exp(-pow(azimuth/(angular_radius+0.026180),2.0)-pow(vertical/(angular_radius+0.104720),2.0));
 let coverage=mix(direct,scatter,smoothstep(40.0,800.0,footprint));
 let transmission=1.0-air_opacity(distance,hit.y);
 return mix(color,shade(index,0).rgb,0.85*coverage*visible*transmission);
}
// CLOUD weather has palette water rather than a named OCEAN plane.
fn overcast_water(color:vec3<f32>,ray:vec3<f32>)->vec3<f32> {
 if !smooth_weather() || scene.cloud_reflection.z<=0.0 || scene.cloud_reflection.x<0.0 || ray.y>=-0.000001 || scene.eye.y<=0.0 {return color;}
 let distance=-scene.eye.y/ray.y;
 let hit=scene.eye.xyz+ray*distance;
 let opacity=1.0-smoothstep(2700.0,26400.0,length(hit.xz-scene.eye.xz));
 if opacity<=0.0 {return color;}
 let motion=1.0-smoothstep(3500.0,16000.0,scene.eye.y);
 let footprint=distance*scene.ocean.w/max(abs(ray.y),0.025);
 let slope=water_slopes(hit.xz,footprint)*motion;
 let normal=normalize(vec3<f32>(-slope.x,1.0,-slope.y));
 let reflected=reflect(ray,normal);
 var sky=mix(shade(240u,0).rgb,shade(229u,0).rgb,smoothstep(0.0,0.6,reflected.y));
 if reflected.y>0.02 && scene.cloud_reflection.x>=0.0 {
  let cloud_hit=hit+reflected*(scene.cloud_reflection.y/reflected.y);
  let uv=fract(vec2<f32>(cloud_hit.x,-cloud_hit.z)/32768.0);
  let cloud=sample_tile(uv,i32(scene.cloud_reflection.x),0,-1,-1,vec2<f32>(-1.0));
  sky=mix(sky,cloud.rgb,cloud.a*smoothstep(0.02,0.15,reflected.y));
 }
 let facing=clamp(dot(normal,-ray),0.0,1.0);
 let strength=(0.08+0.47*pow(1.0-facing,3.0))*(scene.cloud_reflection.w/0.55);
 let visibility=1.0-clamp(haze(distance),0.0,1.0);
 return mix(color,sky,strength*visibility*opacity*opacity);
}
fn ocean_surface(hit:vec3<f32>,deck:vec4<f32>,distance:f32,passes:i32,core:i32)->vec4<f32>{
 let ray=normalize(hit-scene.eye.xyz);
 let footprint=distance*scene.ocean.w/max(abs(ray.y),0.025);
 let altitude=abs(scene.eye.y-deck.x);
 // User trial: fade the whole effect across a five-statute-mile ground radius.
 let ground_distance=length(hit.xz-scene.eye.xz);
 let opacity=1.0-smoothstep(2700.0,26400.0,ground_distance);
 let motion=1.0-smoothstep(3500.0,16000.0,altitude);
 let uv=vec2<f32>(hit.x,-hit.z)/deck.y;
 if opacity<=0.0 {return ocean_sample(uv,i32(deck.z),distance,passes,core);}
 // Four-foot world cells are visible nearby, continuously blended to smooth
 // normals as altitude or the projected size of a pixel increases.
 let block=(1.0-smoothstep(400.0,2200.0,altitude))*(1.0-smoothstep(1.0,4.0,footprint));
 var slope=vec2<f32>(0.0);
 if motion>0.0 {slope=water_slopes(hit.xz,footprint);}
 if block>0.0 && motion>0.0 {
  let pixel=water_slopes((floor(hit.xz/4.0)+vec2<f32>(0.5))*4.0,footprint);
  slope=mix(slope,pixel,block);
 }
 slope*=motion;
 let normal=normalize(vec3<f32>(-slope.x,1.0,-slope.y));
 let offset=slope*6.0;
 let tex=ocean_sample(uv+vec2<f32>(offset.x,-offset.y)/deck.y,i32(deck.z),distance,passes,core);
 // All colors still come from the original ocean and weather-resolved sky.
 // No synthetic teal, whitecap sheet, replacement texture or new sky model.
 let reflected=reflect(ray,normal);
 var sky_color=shade(240u,0).rgb;
 if scene.deck_a.z>=0.0 && scene.ocean.y==0.0 && reflected.y>0.01 && scene.deck_a.x>hit.y {
  let sky_hit=hit+reflected*((scene.deck_a.x-hit.y)/reflected.y);
  let sky_uv=fract(vec2<f32>(sky_hit.x,-sky_hit.z)/scene.deck_a.y);
  let sky_tex=ocean_sample(sky_uv,i32(scene.deck_a.z),distance,0,-1).rgb;
  sky_color=mix(sky_color,sky_tex,smoothstep(0.15,0.45,reflected.y));
 }
 let facing=clamp(dot(normal,-ray),0.0,1.0);
 let fresnel=0.02+0.98*pow(1.0-facing,5.0);
 let visibility=1.0-clamp(haze(distance),0.0,1.0);
 // Fade reflected contrast as well as whole-effect opacity. The base art
 // keeps its original brightness; distant highlights receive opacity squared.
 let peak_scale=select(0.75,scene.cloud_reflection.w/0.25,smooth_weather());
 let shaded=mix(tex.rgb,sky_color,min(fresnel,0.25)*peak_scale*visibility*opacity);
 let base=ocean_sample(uv,i32(deck.z),distance,passes,core);
 return vec4<f32>(mix(base.rgb,shaded,opacity),base.a);
}
// Opinionated directional scattering approximation, docs/spec/sun-glow.md.
fn solar_glow(color:vec3<f32>,ray:vec3<f32>)->vec3<f32> {
 if !smooth_weather() || scene.sun.w<=0.0 || celestial_occluded(ray) {return color;}
 let cosine=clamp(dot(ray,scene.sun.xyz),-1.0,1.0);
 if cosine<=0.0 {return color;}
 let angle=acos(cosine);
 let low=1.0-smoothstep(0.104528,0.422618,scene.sun.y);
 let narrow=exp(-pow(angle/0.122173,2.0));
 let wide=exp(-pow(angle/0.314159,2.0));
 let azimuth=acos(clamp(dot(ray.xz,scene.sun.xz)/max(length(ray.xz)*length(scene.sun.xz),0.00001),-1.0,1.0));
 let elevation=asin(clamp(ray.y,-1.0,1.0))-asin(clamp(scene.sun.y,-1.0,1.0));
 let wash=exp(-pow(azimuth/0.785398,2.0)-pow(elevation/0.244346,2.0));
 let strength=(mix(0.30,0.85,low)*narrow+mix(0.06,0.24,low)*wide+0.16*low*wash)
     *smoothstep(0.0,0.2,cosine)*scene.circles[0].z;
 let emission=mix(vec3<f32>(1.0,0.88,0.65),vec3<f32>(1.0,0.32,0.075),low)*strength;
 return color+(vec3<f32>(1.0)-color)*(vec3<f32>(1.0)-exp(-emission));
}
// Per-pixel angular cloud lighting. World direction avoids per-tile seams;
// texture modulation retains the original cloud detail and transparent holes.
fn cloud_solar_glow(color:vec3<f32>,ray:vec3<f32>,visibility:f32)->vec3<f32> {
 if !smooth_weather() || scene.sun.w<=0.0 {return color;}
 let cosine=clamp(dot(ray,scene.sun.xyz),-1.0,1.0);
 if cosine<=0.0 {return color;}
 let low=1.0-smoothstep(0.104528,0.422618,scene.sun.y);
 let rising=smoothstep(-0.087156,0.034899,scene.sun.y);
 let angular=exp(-pow(acos(cosine)/0.610865,2.0))*smoothstep(0.0,0.2,cosine);
 let strength=0.90*low*rising*angular*scene.circles[0].z*visibility;
 let tint=mix(vec3<f32>(1.0,0.88,0.65),vec3<f32>(1.0,0.32,0.075),low);
 let emission=color*tint*strength;
 return color+(vec3<f32>(1.0)-color)*(vec3<f32>(1.0)-exp(-emission));
}
@fragment fn sky_fragment(in:SkyOut)->@location(0) vec4<f32>{
 let ray=normalize(scene.forward.xyz+scene.right.xyz*in.screen.x*scene.eye.w/(1.7320508*scene.up.w)+scene.up.xyz*in.screen.y/(1.7320508*scene.up.w));
 // Source deck planes: world feet, power-of-two tiling, reversed north axis.
 // The GPU ray/plane intersection replaces the source scanline rasterizer.
 var passes=0;var core=-1;
 if scene.sun.w>0.0 && !celestial_occluded(ray) {
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
 if !smooth_weather() && scene.deck_a.z>=0.0 && scene.eye.y>=scene.deck_a.x {
  // Empty-name, mode-1 above-sky call at 0x4ab00c writes only a transition.
  let virtual_index=deck_transition(ray,25600000.0,243.0);
  if virtual_index>=0 && horizon_height(ray)<0.0 {background=virtual_index;}
  if celestial_occluded(ray) {background=229.0;}
 }
 let transitions=array<vec4<f32>,2>(scene.deck_a,scene.deck_b);
 for(var i=0;i<2;i++){
  let deck=transitions[i];
  if deck.z<0.0 || (i==1 && scene.eye.y<=deck.x) {continue;}
  let upper=select(243.0,241.0,scene.deck_a.z<0.0);
  let lower=select(244.0,241.0,scene.deck_b.z<0.0);
  let index=deck_transition(ray,deck.x,select(upper,lower,scene.eye.y>deck.x));
  if index>=0 && !smooth_weather() {background=index;}
 }
 var color=solar_glow(horizon_color(background,passes,core),ray);
 let decks=array<vec4<f32>,2>(scene.deck_a,scene.deck_b);
 var nearest=1e30;
 for(var i=0;i<2;i++){
  let deck=decks[i];
  if deck.z<0.0 || abs(ray.y)<0.000001 || (i==1 && scene.eye.y<=deck.x) { continue; }
  let distance=(deck.x-scene.eye.y)/ray.y;
  var head=scene.forward.xz;
  if length(head)<0.0001 {head=scene.up.xz;}
  let scanline_distance=distance*dot(ray.xz,normalize(head));
  if distance<=0.0 || distance>=nearest || scanline_distance>=2000000.0 { continue; }
  let hit=scene.eye.xyz+ray*distance;
  let uv=fract(vec2<f32>(hit.x,-hit.z)/deck.y);
  var tex:vec4<f32>;
  if scene.ocean[i+1]>0.0 {
   tex=ocean_surface(hit,deck,distance,passes,core);
   tex=vec4<f32>(aerial_perspective(tex.rgb,ray*distance,deck.x),tex.a);
   tex=vec4<f32>(water_sun(tex.rgb,ray,hit,distance),tex.a);
  } else {
  tex=weather_tile(uv,i32(deck.z),fog_row(distance),passes,core,vec2<f32>(-1.0));
  if smooth_weather() {
   let row=1.0+clamp(haze(distance)*scene.forward.w,0.0,scene.forward.w-1.0);
   let a=weather_tile(uv,i32(deck.z),i32(floor(row)),passes,core,vec2<f32>(-1.0));
   let b=weather_tile(uv,i32(deck.z),i32(ceil(row)),passes,core,vec2<f32>(-1.0));
   tex=mix(a,b,fract(row));
  }
  }
  if scene.ocean[i+1]<=0.0 {tex=vec4<f32>(solar_glow(cloud_solar_glow(tex.rgb,ray,1.0),ray),tex.a);}
  // Fade to the actual backdrop before the source plane cutoff. Using the
  // same destination for RGB and coverage avoids a separate blue horizon seam.
  if smooth_weather() {
   let fade=smoothstep(264000.0,2000000.0,max(length(hit.xz-scene.eye.xz),scanline_distance));
   if scene.ocean[i+1]<=0.0 {tex=vec4<f32>(mix(tex.rgb,color,fade*0.5),tex.a);}
   tex.a*=1.0-fade;
  }
  color=mix(color,tex.rgb,tex.a);
  nearest=distance;
 }
 // Source lower Gouraud is drawn after the sky and celestial primitives when
 // there is no visible ocean plane. Its upper edge may cover sky texture too.
 if (i32(scene.ray.w)&2)!=0 && horizon_height(ray)<=5.0 {color=horizon_color(horizon_index(ray),0,-1);}
 if scene.deck_a.z<0.0 && scene.deck_b.z<0.0 {
  color=overcast_water(color,ray);
  if scene.cloud_reflection.z>0.0 && ray.y< -0.000001 && scene.eye.y>0.0 {
   let distance=-scene.eye.y/ray.y;
   if distance<2000000.0 {
    let reflected=water_sun(color,ray,scene.eye.xyz+ray*distance,distance);
    color=mix(reflected,color,smoothstep(1800000.0,2000000.0,distance));
   }
  }
 }
 var cloud_distance=2000000.0;
 if ray.y< -0.000001 {cloud_distance=min(cloud_distance,max(scene.eye.y,0.0)/(-ray.y));}
 color=cloud_occlusion(color,ray*cloud_distance,scene.eye.y+ray.y*cloud_distance);
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
 // Resolve the fitted sky-entry material in this camera palette.
 return vec4<f32>(shade(254u,0).rgb*in.color.rgb,in.color.a*(1.0-haze(in.distance)));
}

@vertex fn celestial_vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>,@location(4) index:f32)->VertexOut {
 let z=dot(position,scene.forward.xyz);let f=1.7320508*scene.up.w;
 var out:VertexOut;
 out.clip=vec4<f32>(dot(position,scene.right.xyz)*f/scene.eye.w,dot(position,scene.up.xyz)*f,z,z);
 out.light_row=-1;out.fog_enabled=0u;out.uv=uv;out.layer=layer;out.color=shade(u32(index),0).rgb;out.distance=position.y;out.own_color=0.;out.altitude=position.y;out.direction=position;return out;
}
@fragment fn celestial_fragment(in:VertexOut)->@location(0) vec4<f32>{
 if celestial_occluded(normalize(in.direction)) {discard;}
 if in.layer>=0.0 {let tex=weather_tile(in.uv,i32(in.layer),0,-1,-1,vec2<f32>(-1.0));if tex.a<0.01 {discard;}return tex;}
 return vec4<f32>(in.color,1.0);
}

@fragment fn cloud_fragment(in:VertexOut)->@location(0) vec4<f32>{
 let tex=weather_tile(in.uv,i32(in.layer),0,-1,-1,ray_rows(in.distance,in.altitude));
 if tex.a<0.5 {discard;}
 let lit=cloud_solar_glow(tex.rgb,normalize(in.direction),1.0-clamp(haze(length(in.direction)),0.0,1.0));
 return vec4<f32>(aerial_perspective(lit,in.direction,in.altitude),1.0);
}
