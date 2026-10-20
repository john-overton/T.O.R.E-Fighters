struct Band { info:vec4<f32>, ramp:vec4<f32> }
// scene.view.z is the shared CPU night factor (night_glow::night_factor).
struct Scene { eye:vec4<f32>, right:vec4<f32>, up:vec4<f32>, forward:vec4<f32>, sky:vec4<f32>, fog:vec4<f32>, deck_a:vec4<f32>, deck_b:vec4<f32>, sun:vec4<f32>, circles:array<vec4<f32>,8>, ray:vec4<f32>, bands:array<Band,32>, ocean:vec4<f32>, cloud_reflection:vec4<f32>, view:vec4<f32>, quality:vec4<f32>, viewport:vec4<f32>, origin:vec4<f32>, local_eye:vec4<f32>, veg:vec4<f32>, canopy_area:vec4<f32>, canopy:vec4<f32>, canopy_detail:vec4<f32>, sky_mix:vec4<f32>, sky_mix_b:vec4<f32>, stars:vec4<f32>, moon:vec4<f32>, haze_rows:vec4<f32>, band_shades:array<vec4<f32>,32> }
// veg: ground-detail mode (0 off, 1 on, 2 fine), detail distance in feet,
// unused, climate (0 temperate, 1 desert, 2 arctic, 3 tropical).
// quality: spotting aid strength (0 off), terrain filtering (0/1), thermal
// sensor picture (0/1, see "Thermal sensor picture" at the end), the thermal
// picture's reference brightness (sim_renderer.rs `thermal_reference`). viewport: world image width and height in pixels, MSAA
// samples, render scale. See graphics.rs.
// sky_mix / sky_mix_b: per deck slot (high sky, low ocean) the drawn
// opacity, a second texture it crosses into (-1 none) and that texture's
// share, then the upper / lower horizon gradient weight (sky_mix.rs; smooth
// weather fades decks and gradients through dawn and dusk instead of switching).
// stars: sine and cosine of the theater latitude, the sky's rotation angle and
// the starfield's visibility; moon: its direction and visibility (night_sky.rs).
// haze_rows: a second shade remap row for weather art (deck_b.w is the first)
// and its share, so the haze tables cross over instead of switching.
// origin: the frame's render origin, a whole-foot point near the camera that
// moving objects' vertices are relative to; local_eye: the camera relative to
// it, exact, since 32-bit world coordinates step 1/8 foot in a theater.
@group(0) @binding(0) var<uniform> scene:Scene;
// Only target-camera readbacks use alpha as scenery/subject coverage.
fn scenery(color:vec3<f32>)->vec4<f32> {
 return vec4<f32>(color,select(1.0,0.0,scene.view.y>0.5));
}
// Retail terrain and sky artwork is stored as weather-palette indices, so it is
// uploaded unresolved and the live palette is applied here every frame.
@group(0) @binding(1) var tiles:texture_2d_array<u32>;
@group(0) @binding(2) var palette:texture_2d<f32>;
@group(0) @binding(3) var weather_tiles:texture_2d_array<u32>;
@group(0) @binding(4) var engine_art:texture_2d<f32>;
@group(0) @binding(5) var<uniform> tile_storage:vec4<u32>;
@group(0) @binding(6) var seam_records:texture_2d<u32>;
@group(0) @binding(7) var seam_tints:texture_2d_array<f32>;
@group(0) @binding(8) var seam_sampler:sampler;
fn tile_size()->vec2<u32> {
 if tile_storage.x!=0u {return vec2<u32>(256u);}
 return textureDimensions(tiles);
}
fn page_coord(at:vec2<i32>,layer:i32)->vec2<i32> {
 return at+vec2<i32>(layer%4,(layer%16)/4)*256;
}
fn read_tile(at:vec2<i32>,layer:i32)->u32 {
 if tile_storage.x!=0u {return textureLoad(tiles,page_coord(at,layer),layer/16,0).r;}
 return textureLoad(tiles,at,layer,0).r;
}
fn weather_index(at:vec2<i32>,layer:i32)->u32 {
 if textureDimensions(weather_tiles).x==1024u {return textureLoad(weather_tiles,page_coord(at,layer),layer/16,0).r;}
 return textureLoad(weather_tiles,at,layer,0).r;
}
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
 @builtin(position) @invariant clip:vec4<f32>, @location(0) uv:vec2<f32>,
 @location(1) @interpolate(flat) layer:f32, @location(2) color:vec3<f32>, @location(3) distance:f32, @location(4) @interpolate(flat) own_color:f32, @location(5) altitude:f32, @location(6) direction:vec3<f32>, @location(7) @interpolate(flat) fog_enabled:u32, @location(8) @interpolate(flat) light_row:i32, @location(9) terrain_normal:vec3<f32>,
 // Color before the weather's shade remaps, for flare light.
 @location(10) albedo:vec3<f32>,
 // What the thermal picture takes the surface for: 0 the ground, a building
 // or the airfield; 1 something running warm (aircraft, vehicles, men,
 // weapons and debris: everything built relative to the render origin).
 @location(11) @interpolate(flat) heat:f32
}
// Exact sRGB decoding includes the dark linear segment; source black stays zero.
fn linear(c:vec3<f32>)->vec3<f32>{return select(pow((c+vec3<f32>(0.055))/1.055,vec3<f32>(2.4)),c/12.92,c<=vec3<f32>(0.04045));}
// Index 255 is the native water/cutout test at 0x4aa739 and stays transparent.
fn fog_row(distance:f32)->i32{
 if textureDimensions(palette).y<=1u { return 0; }
 let levels=i32(scene.forward.w);
 return 1+clamp(i32(clamp(haze(distance),0.0,1.0)*f32(levels)),0,levels-1);
}
// The daylight palette row: the art's colors in the weather's brightest
// record, which flare light shows at night. Falls back to the live palette
// where it is absent.
const BASE_ROW:i32=11;
fn base_row()->i32 {return select(0,BASE_ROW,i32(textureDimensions(palette).y)>BASE_ROW);}
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
 if rows.y>=0 {result=weather_index(vec2<i32>(i32(result),rows.y),i32(scene.deck_a.w));}
 if rows.x>=0 {result=weather_index(vec2<i32>(i32(result),rows.x),i32(scene.deck_a.w));}
 return result;
}
// Authored presentation mode: interpolate neighboring original remap results.
fn smooth_weather()->bool {return (i32(scene.ray.w)&8)!=0;}
fn continuous_row(info:vec4<f32>,r:vec4<f32>,distance:f32)->f32 {
 var density=r.y;
 if distance>r.x {if distance>=r.z {density=r.w;}else {density=mix(r.y,r.w,(distance-r.x)/(r.z-r.x));}}
 return info.z+clamp(density/256.0*info.w,0.0,info.w-1.0);
}
fn smooth_ray_rows(distance:f32,altitude:f32)->vec2<f32>{
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
// A remap seen through one band reads that band's colour rows
// (scene.band_shades[band].w, from LUT_ROW of the palette texture): its shade
// table's colours, crossed on the CPU with a second record's table while
// smooth weather crosses records, so one load replaces the remap and colour
// reads. ray_rows marks such rows with rows.y=-(LUT_CODE+offset), the
// offset taking the band's shade row to its colour row, so rows.x and its
// fraction stay exactly what the atlas path reads. A remap through two
// bands keeps the atlas, and the crossing for it rides in ray_cross: the
// second rows and their share. remap_color reads ray_cross set by the latest
// ray_rows call, so take the rows and use them before calling ray_rows again.
const LUT_CODE:f32=1000.0;
const LUT_ROW:i32=12;
var<private> ray_cross:vec4<f32>;
fn second_row(row:f32,band:i32)->f32 {
 let s=scene.band_shades[band];let info=scene.bands[band].info;
 return s.x+(row-info.z)*max(s.y-1.0,0.0)/max(info.w-1.0,1.0);
}
fn ray_rows(distance:f32,altitude:f32)->vec2<f32>{
 var rows=vec2<f32>(native_ray_rows(distance,altitude));
 if smooth_weather() {rows=smooth_ray_rows(distance,altitude);}
 ray_cross=vec4<f32>(0.0);
 if rows.x<0.0 {return rows;}
 // rows.x is seen through the eye's band, rows.y through the target's.
 let bx=band_at(i32(max(scene.eye.y,0.0)));var by=-1;
 if rows.y>=0.0 {by=band_at(i32(max(altitude,0.0)));}
 if bx<0 {return rows;}
 let sx=scene.band_shades[bx];
 if rows.y<0.0 && sx.w>=0.0 {return vec2<f32>(rows.x,-(LUT_CODE+sx.w-scene.bands[bx].info.z));}
 var sy=vec4<f32>(0.0);if by>=0 {sy=scene.band_shades[by];}
 let share=max(sx.z,sy.z);
 if share>0.0 {
  ray_cross=vec4<f32>(select(rows.x,second_row(rows.x,bx),sx.z>0.0),select(rows.y,second_row(rows.y,max(by,0)),sy.z>0.0),share,0.0);
 }
 return rows;
}
fn remap_rows(index:u32,rows:vec2<f32>)->vec3<f32>{
 let lo=vec2<i32>(floor(rows));let hi=vec2<i32>(ceil(rows));let t=fract(rows);
 let a=shade(ray_index(index,lo),0).rgb;
 if all(lo==hi) {return a;}
 let b=shade(ray_index(index,vec2<i32>(hi.x,lo.y)),0).rgb;
 let c=shade(ray_index(index,vec2<i32>(lo.x,hi.y)),0).rgb;
 let d=shade(ray_index(index,hi),0).rgb;
 return mix(mix(a,b,t.x),mix(c,d,t.x),t.y);
}
fn remap_color(index:u32,rows:vec2<f32>)->vec3<f32>{
 if rows.y<-2.0 {
  let offset=LUT_ROW+i32(-rows.y-LUT_CODE);
  let lo=i32(floor(rows.x));let hi=i32(ceil(rows.x));
  let a=linear(textureLoad(palette,vec2<i32>(i32(index),offset+lo),0).rgb);
  if lo==hi {return a;}
  return mix(a,linear(textureLoad(palette,vec2<i32>(i32(index),offset+hi),0).rgb),fract(rows.x));
 }
 let c=remap_rows(index,rows);
 if ray_cross.z<=0.0 {return c;}
 return mix(c,remap_rows(index,ray_cross.xy),ray_cross.z);
}
// Remap one original index before resolving RGB; transparency tests the source.
fn texel(at:vec2<i32>,layer:i32,row:i32,sun_passes:i32,core:i32,remaps:vec2<f32>)->vec4<f32>{
   let original=read_tile(at,layer);
   var index=original;
   var palette_row=row;
   if sun_passes<0 && core>=0 { index=weather_index(vec2<i32>(i32(index),core),i32(scene.deck_a.w)); }
   var second=-1;
   if sun_passes>=0 {
    if scene.haze_rows.y>0.0 && scene.haze_rows.x>=0.0 {
     var other=read_tile(vec2<i32>(i32(index),i32(scene.haze_rows.x)+row-1),i32(scene.deck_a.w));
     for(var n=0;n<sun_passes;n++){ other=read_tile(vec2<i32>(i32(other),0),i32(scene.deck_a.w)); }
     if core>=0 {other=u32(core);}
     second=i32(other);
    }
    index=read_tile(vec2<i32>(i32(index),i32(scene.deck_b.w)+row-1),i32(scene.deck_a.w));
    for(var n=0;n<sun_passes;n++){ index=read_tile(vec2<i32>(i32(index),0),i32(scene.deck_a.w)); }
    if core>=0 {index=u32(core);}
    palette_row=0;
   }
   var c=shade(index,palette_row);
   if second>=0 {c=mix(c,shade(u32(second),0),scene.haze_rows.y);}
   if remaps.x>=0 {c=vec4<f32>(remap_color(index,remaps),1.0);}
   // Cutout belongs to the original texel, before any shade remap.
   c.a=select(1.0,0.0,original==255u);
   return c;
}
// Reviewed weather raster reads one texel, without mixing palette colors.
// Float UV projection remains an adaptation of native fixed-point scanlines.
fn weather_tile(uv:vec2<f32>,layer:i32,row:i32,sun_passes:i32,core:i32,remaps:vec2<f32>)->vec4<f32>{
 let size=vec2<i32>(tile_size());
 let at=clamp(vec2<i32>(floor(uv*vec2<f32>(size))),vec2<i32>(0),size-vec2<i32>(1));
 return texel(at,layer,row,sun_passes,core,remaps);
}
// Manual bilinear: indices cannot be filtered, so each of the four texels is
// resolved through the palette first and the colors are blended premultiplied.
fn sample_tile(uv:vec2<f32>,layer:i32,row:i32,sun_passes:i32,core:i32,remaps:vec2<f32>)->vec4<f32>{
 let size=vec2<i32>(tile_size());
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
// Opinionated terrain filtering (graphics option, scene.quality.y). Palette
// indices cannot be averaged and there are no mipmaps, so each coarser level is
// a virtual lattice: one texel per 2^level square block, at a fixed hashed
// position inside that block. Its index resolves through the live palette and
// remaps like any texel, and only resolved colors are blended. The lattice is
// fixed to the texture, so moving the camera changes blend weights smoothly
// instead of picking new texels each frame.
const FILTER_PROBES:i32=4;
fn lattice_hash(v:u32)->u32 {
 let s=v*747796405u+2891336453u;
 let w=((s>>((s>>28u)+4u))^s)*277803737u;
 return (w>>22u)^w;
}
fn lattice_texel(block:vec2<i32>,level:i32,layer:i32,sun_passes:i32,core:i32,remaps:vec2<f32>)->vec4<f32>{
 let blocks=vec2<i32>(tile_size())>>vec2<u32>(u32(level));
 let at=clamp(block,vec2<i32>(0),blocks-vec2<i32>(1));
 if level==0 {return texel(at,layer,0,sun_passes,core,remaps);}
 let h=lattice_hash(u32(at.x)|(u32(at.y)<<9u)|(u32(level)<<18u));
 let mask=(1u<<u32(level))-1u;
 let offset=vec2<i32>(vec2<u32>(h&mask,(h>>9u)&mask));
 return texel((at<<vec2<u32>(u32(level)))+offset,layer,0,sun_passes,core,remaps);
}
// Premultiplied bilinear blend of four lattice texels around p (base texels).
fn lattice_bilinear(p:vec2<f32>,level:i32,layer:i32,sun_passes:i32,core:i32,remaps:vec2<f32>)->vec4<f32>{
 let q=p*exp2(-f32(level))-vec2<f32>(0.5);
 let base=vec2<i32>(floor(q));let f=q-floor(q);
 var sum=vec4<f32>(0.0);
 for(var j=0;j<2;j++){
  for(var i=0;i<2;i++){
   let c=lattice_texel(base+vec2<i32>(i,j),level,layer,sun_passes,core,remaps);
   let w=select(1.0-f.x,f.x,i==1)*select(1.0-f.y,f.y,j==1);
   sum+=vec4<f32>(c.rgb*c.a,c.a)*w;
  }
 }
 return sum;
}
// grad holds dpdx(uv) and dpdy(uv), taken by the caller in uniform control
// flow. The pixel footprint ellipse gives a major and a minor axis in texels.
// Up to FILTER_PROBES probes run along the major axis; each blends two
// adjacent lattice levels sized to the remaining footprint. Footprints of one
// texel or less take the unfiltered path unchanged.
fn filtered_tile(uv:vec2<f32>,grad:vec4<f32>,layer:i32,sun_passes:i32,core:i32,remaps:vec2<f32>)->vec4<f32>{
 let size=vec2<f32>(tile_size());
 let dx=grad.xy*size;let dy=grad.zw*size;
 let m11=dx.x*dx.x+dy.x*dy.x;let m22=dx.y*dx.y+dy.y*dy.y;let m12=dx.x*dx.y+dy.x*dy.y;
 let mid=0.5*(m11+m22);let spread=sqrt(0.25*(m11-m22)*(m11-m22)+m12*m12);
 let major=sqrt(mid+spread);let minor=sqrt(max(mid-spread,0.0));
 if scene.quality.y<=0.5 || major<=1.0 {return sample_tile(uv,layer,0,sun_passes,core,remaps);}
 var axis=vec2<f32>(1.0,0.0);
 if spread>0.000001 {axis=normalize(vec2<f32>(m12,mid+spread-m11));}
 if abs(m12)<=0.000001 {axis=select(vec2<f32>(0.0,1.0),vec2<f32>(1.0,0.0),m11>=m22);}
 let probes=clamp(i32(ceil(major/max(minor,1.0))),1,FILTER_PROBES);
 let footprint=max(minor,major/f32(probes));
 let top=i32(log2(size.x));
 let lod=clamp(log2(max(footprint,1.0)),0.0,f32(top));
 let fine=min(i32(floor(lod)),top);let coarse=min(fine+1,top);let t=lod-f32(fine);
 let center=uv*size;
 var sum=vec4<f32>(0.0);
 for(var n=0;n<probes;n++){
  let p=center+axis*major*((f32(n)+0.5)/f32(probes)-0.5);
  sum+=lattice_bilinear(p,fine,layer,sun_passes,core,remaps)*(1.0-t);
  if t>0.0 {sum+=lattice_bilinear(p,coarse,layer,sun_passes,core,remaps)*t;}
 }
 sum/=f32(probes);
 if sum.a<=0.0 {return vec4<f32>(0.0);}
 return vec4<f32>(sum.rgb/sum.a,sum.a);
}
// Reversed floating-point depth preserves small surface separations at
// distance. Keep the existing finite far plane and the per-camera near plane.
fn world_depth_clip(z:f32)->f32 {
 let near=max(scene.view.x,1.0);let far=2200000.0;
 return near*((far-z)/(far-near));
}
// World geometry reaches the camera through the origin, so a static vertex
// loses no more than its own storage precision.
fn world_vertex(position:vec3<f32>,uv:vec2<f32>,layer:f32,color:vec3<f32>,index:f32)->VertexOut {
 return placed_vertex(position,(position-scene.origin.xyz)-scene.local_eye.xyz,uv,layer,color,index);
}
fn placed_vertex(position:vec3<f32>,p:vec3<f32>,uv:vec2<f32>,layer:f32,color:vec3<f32>,index:f32)->VertexOut {
 let z=dot(p,scene.forward.xyz);
 let f=1.7320508*scene.up.w;
 var out:VertexOut;
 out.clip=vec4<f32>(dot(p,scene.right.xyz)*f/scene.eye.w,dot(p,scene.up.xyz)*f,world_depth_clip(z),z);
 let fog_mode=(u32(max(index,0.0))/256u)%4u;
 out.light_row=i32(max(index,0.0))/1024-1;
 let fog_enabled=fog_mode==0u || (fog_mode==2u && (i32(scene.ray.w)&4)==0);
 out.fog_enabled=select(0u,1u,fog_enabled);
 out.direction=p;out.altitude=position.y;out.uv=uv;out.layer=layer;out.own_color=select(0.0,1.0,index<0.0);
 // A negative index means the vertex carries its own color; terrain carries a
 // source palette index instead, resolved per frame and then Gouraud blended.
 out.albedo=linear(color);
 if index>=0.0 { var source_index=u32(index)%256u; if out.light_row>=0 {source_index=weather_index(vec2<i32>(i32(source_index),out.light_row),i32(scene.deck_a.w));} out.albedo=shade(source_index,base_row()).rgb; out.color=shade(source_index,0).rgb; if fog_enabled {out.color=remap_color(source_index,ray_rows(length(p),position.y));} } else { out.color=linear(color); }
 out.distance=length(p);return out;
}
@vertex fn vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>,@location(4) index:f32)->VertexOut {
 return world_vertex(position,uv,layer,color,index);
}
// Aircraft, pilots, weapons and debris: `position` is relative to the origin.
@vertex fn object_vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>,@location(4) index:f32)->VertexOut {
 let gun_ribbon=layer == -8.0 && index < -2.0;
 // Signed side vectors are geometry, not sRGB; do not decode them as art.
 var out=placed_vertex(position+scene.origin.xyz,position-scene.local_eye.xyz,uv,layer,select(color,vec3<f32>(1.0),gun_ribbon),index);
 out.heat=1.0;
 if gun_ribbon {
  // Gun ribbon payload, shared by live, network and replay snapshots.
  let green=index < -22.0;
  let tag=index+select(0.0,20.0,green);
  let end_on=tag < -12.0;
  let brightness=-tag-2.0-select(0.0,10.0,end_on);
  let width=length(color);
  let night=tracer_night();
  let p=position-scene.local_eye.xyz;
  let z=dot(p,scene.forward.xyz);
  let f=1.7320508*scene.up.w;
  let pixel=2.0*max(z,1.0)/(max(scene.viewport.y,1.0)*f);
  // Keep the narrow hot ribbon, surround it with a bounded soft halo.
  let core_width=mix(1.2*width,max(1.2*width,pixel),night);
  let half_width=mix(core_width,max(core_width,8.0*pixel),night);
  var offset=normalize(color)*uv.y*(half_width-1.2*width);
  let along=select(0.0,1.0,uv.x>0.0);
  out.uv.x=along;
  let projected_length=2.0*abs(uv.x);
  var longitudinal=scene.up.xyz;
  if !end_on {longitudinal=normalize(cross(color,p));}
  // Side-on unresolved rounds remain short streaks, not square dots.
  let core_length=projected_length+night*max(select(6.0,2.0,end_on)*pixel-projected_length,0.0);
  let glow_length=mix(core_length,max(core_length,16.0*pixel),night);
  offset+=longitudinal*(along-0.5)*(glow_length-projected_length);
  let q=p+offset;
  if night>0.0 {
   out.clip=vec4<f32>(dot(q,scene.right.xyz)*f/scene.eye.w,dot(q,scene.up.xyz)*f,world_depth_clip(dot(q,scene.forward.xyz)),dot(q,scene.forward.xyz));
  }
  out.color=vec3<f32>(select(0.0,1.0,green),brightness,0.0);
  out.heat=2.0;
  out.terrain_normal.x=night*clamp(1.0-1.2*width/pixel,0.0,1.0);
  out.terrain_normal.y=half_width/core_width;
  out.terrain_normal.z=glow_length/max(core_length,0.0001);
 }
 return out;
}
// City fill: a kit piece drawn per instance. `place.xyz` is the base
// relative to the render origin and `place.w` the building's fixed hash;
// `turn` holds the heading's cosine and sine, the height share (a stub when
// flattened) and the coverage while fading in. The coverage and hash ride
// in `terrain_normal`, which only the terrain's own fragment shader reads.
@vertex fn fill_vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>,@location(4) index:f32,@location(5) place:vec4<f32>,@location(6) turn:vec4<f32>)->VertexOut {
 let local=vec3<f32>(position.x,position.y*turn.z,position.z);
 let p=place.xyz+vec3<f32>(local.x*turn.x+local.z*turn.y,local.y,local.z*turn.x-local.x*turn.y);
 var out=placed_vertex(p+scene.origin.xyz,p-scene.local_eye.xyz,uv,layer,color,index);
 out.terrain_normal=vec3<f32>(turn.w,place.w,0.0);
 return out;
}
// City fill far level of detail: the unit box scaled to a building's half
// footprint (`size.xy`) and height (`turn.z`, feet), in the colour index
// `place.w`, with its coverage `turn.w` and hash `size.z` in `uv` (a box
// has no texture).
@vertex fn fill_box_vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>,@location(4) index:f32,@location(5) place:vec4<f32>,@location(6) turn:vec4<f32>,@location(7) size:vec4<f32>)->VertexOut {
 let local=vec3<f32>(position.x*size.x,position.y*turn.z,position.z*size.y);
 let p=place.xyz+vec3<f32>(local.x*turn.x+local.z*turn.y,local.y,local.z*turn.x-local.x*turn.y);
 var out=placed_vertex(p+scene.origin.xyz,p-scene.local_eye.xyz,uv,-1.0,color,place.w);
 out.uv=vec2<f32>(turn.w,size.z);
 return out;
}
// The world pass's sample count, set when the city fill's pipelines are
// built.
override FILL_SAMPLES:u32=4u;
// The 4 by 4 ordered dither matrix.
fn bayer4(p:vec2<u32>)->u32 {
 var m=array<u32,16>(0u,8u,2u,10u,12u,4u,14u,6u,3u,11u,1u,9u,15u,7u,13u,5u);
 return m[(p.y&3u)*4u+(p.x&3u)];
}
// A fading city fill building's coverage as a sample mask: each sample of
// the pixel has a fixed threshold, the 4 by 4 ordered pattern (shifted by
// the building's hash) interleaved across the samples, and is kept while
// the threshold is below `alpha`. With 4 samples a pixel takes 1, 2, 3 then
// 4 samples as `alpha` grows, so a resolved pixel blends smoothly; with one
// sample it is a screen-door dither. The same pixel and hash always give
// the same mask, so a still view does not shimmer, and a smaller `alpha`
// keeps a subset of a larger one's samples.
fn fill_coverage(clip:vec2<f32>,alpha:f32,hash:f32)->u32 {
 if alpha>=1.0 {return 0xffffffffu;}
 let n=clamp(FILL_SAMPLES,1u,8u);
 let h=u32(hash*65536.0);
 let cell=bayer4(vec2<u32>(clip)+vec2<u32>(h&3u,(h>>2u)&3u));
 var mask=0u;
 for(var s=0u;s<n;s++){
  if (f32(s*16u+cell)+0.5)/f32(16u*n)<alpha {mask|=1u<<s;}
 }
 return mask;
}
struct FillOut { @location(0) color:vec4<f32>, @builtin(sample_mask) mask:u32 }
// CF0 city fill shadow casters: the unit box per building, one entry per
// shadow cascade (instance_index is the building here, not the cascade).
fn fill_shadow(position:vec3<f32>,place:vec4<f32>,turn:vec4<f32>,size:vec4<f32>,cascade:u32)->ShadowOut {
 let local=vec3<f32>(position.x*size.x,position.y*turn.z,position.z*size.y);
 let p=place.xyz+vec3<f32>(local.x*turn.x+local.z*turn.y,local.y,local.z*turn.x-local.x*turn.y);
 return shadow_out(p-scene.local_eye.xyz,vec2<f32>(0.0),-1.0,cascade);
}
@vertex fn fill_shadow_vertex0(@location(0) position:vec3<f32>,@location(5) place:vec4<f32>,@location(6) turn:vec4<f32>,@location(7) size:vec4<f32>)->ShadowOut {return fill_shadow(position,place,turn,size,0u);}
@vertex fn fill_shadow_vertex1(@location(0) position:vec3<f32>,@location(5) place:vec4<f32>,@location(6) turn:vec4<f32>,@location(7) size:vec4<f32>)->ShadowOut {return fill_shadow(position,place,turn,size,1u);}
@vertex fn fill_shadow_vertex2(@location(0) position:vec3<f32>,@location(5) place:vec4<f32>,@location(6) turn:vec4<f32>,@location(7) size:vec4<f32>)->ShadowOut {return fill_shadow(position,place,turn,size,2u);}
@vertex fn terrain_vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>,@location(4) index:f32,@location(5) normal:vec3<f32>)->VertexOut {
 var out=world_vertex(position,uv,layer,color,index);
 out.terrain_normal=normal;
 out.own_color=select(0.0,1.0,flat_page(i32(layer)));return out;
}
// Terrain cutouts expose the already rendered ocean/horizon, never the T2
// land color. Keep this separate from aircraft's base-color texture blending.
// One RG32Uint record holds both palette mixtures and run-end flags.
// Masks include LAND fallback edges. All coordinates remain world feet.
fn seam_record(tile:i32,axis:i32,segment:i32)->vec2<u32> {
 let address=(tile*2+axis)*256+segment;
 let width=i32(textureDimensions(seam_records).x);
 return textureLoad(seam_records,vec2<i32>(address%width,address/width),0).rg;
}
// Baked from the source material and the noisy 500 ft reference blend.
// No source page is replaced. A neutral tint is returned outside the band.
struct SeamResponse { delta:vec3<f32>, luminance:f32, mask:f32 }
fn material_record(address:i32)->vec2<u32> {
 let width=i32(textureDimensions(seam_records).x);
 return textureLoad(seam_records,vec2<i32>(address%width,address/width),0).rg;
}
fn flat_page(layer:i32)->bool {
 let size=vec2<i32>(scene.canopy_detail.zw);
 if layer<0 || size.x<=0 || size.y<=0 {return false;}
 return material_record(size.x*size.y*513+layer).x!=0u;
}
fn terrain_flat_response(world:vec2<f32>)->SeamResponse {
 let size=vec2<i32>(scene.canopy_detail.zw);let cell=vec2<i32>(floor(world/32768.0));
 if size.x<=0 || size.y<=0 || any(cell<vec2<i32>(0)) || any(cell>=size) {return SeamResponse(vec3<f32>(0.0),1.0,0.0);}
 let record=material_record(size.x*size.y*512+cell.y*size.x+cell.x);
 if record.x==0u {return SeamResponse(vec3<f32>(0.0),1.0,0.0);}
 let slot=record.x-1u;let uv=world/32768.0-vec2<f32>(cell);
 let pixel=vec2<f32>(f32(slot%10u),f32((slot%100u)/10u))*96.0+vec2<f32>(0.5)+uv*95.0;
 let value=textureSampleLevel(seam_tints,seam_sampler,pixel/vec2<f32>(textureDimensions(seam_tints)),i32(slot/100u),0.0);
 let distances=vec4<f32>(uv.x,1.0-uv.x,uv.y,1.0-uv.y)*32768.0;
 var fade=1.0;
 for(var i=0u;i<4u;i++) {if (record.y&(1u<<i))!=0u {fade=min(fade,smoothstep(0.0,250.0,distances[i]));}}
 return SeamResponse(value.rgb*fade,value.a,1.0);
}
fn terrain_seam_response(world:vec2<f32>)->SeamResponse {
 let size=vec2<i32>(scene.canopy_detail.zw);
 if size.x<=0 || size.y<=0 {return SeamResponse(vec3<f32>(0.0),1.0,0.0);}
 let cell=vec2<i32>(floor(world/32768.0));
 if any(cell<vec2<i32>(0)) || any(cell>=size) {return SeamResponse(vec3<f32>(0.0),1.0,0.0);}
 let local=world-vec2<f32>(cell)*32768.0;
 let near=min(local,vec2<f32>(32768.0)-local);let axis=select(0,1,near.y<near.x);
 if near[axis]>=250.0 {return SeamResponse(vec3<f32>(0.0),1.0,0.0);}
 let negative=local[axis]<16384.0;let adjacent=cell[axis]+select(1,-1,negative);
 if adjacent<0 || adjacent>=size[axis] {return SeamResponse(vec3<f32>(0.0),1.0,0.0);}
 let owner=cell.y*size.x+cell.x-select(0,select(1,size.x,axis==1),negative);
 let along=local[1-axis]/128.0;let t=fract(along);
 let record=seam_record(owner,axis,i32(floor(along)));let flags=record.x>>24u;
 if (flags&1u)==0u {return SeamResponse(vec3<f32>(0.0),1.0,0.0);}
 var strength=1.0;
 if (flags&8u)==0u {strength=smoothstep(0.0,1.0,t);}
 if (flags&16u)==0u {strength=min(strength,smoothstep(0.0,1.0,1.0-t));}
 let column=record.y-1u;
 let layer=column/57344u;
 let row=(column/1024u)%56u;
 let pixel=vec2<f32>(f32(column%1024u)+t, f32(row*18u)+select(0.0,9.0,negative)+0.5+near[axis]*8.0/250.0);
 let value=textureSampleLevel(seam_tints,seam_sampler,pixel/vec2<f32>(textureDimensions(seam_tints)),i32(layer),0.0);
 return SeamResponse(value.rgb*strength,value.a,strength);
}
fn seam_apply(color:vec3<f32>,response:SeamResponse)->vec3<f32> {
 if response.mask<=0.0 {return color;}
 let scale=dot(color,vec3<f32>(0.2126,0.7152,0.0722))/max(response.luminance,0.000001);
 return max(color+response.delta*scale,vec3<f32>(0.0));
}
// Forest horizon: expected crown coverage and mean art colour from CPU placement.
// World-anchored texture replaces the missing crown share as objects thin/fade.
fn canopy_cell(at:vec2<i32>,row:i32,remaps:vec2<f32>)->vec4<f32> {
 if any(at<vec2<i32>(0)) || any(at>=vec2<i32>(scene.canopy_area.zw)) {return vec4<f32>(0.0);}
 let page=i32(scene.canopy.x)+(at.y/256)*i32(scene.canopy.y)+at.x/128;
 let pixel=vec2<i32>((at.x%128)*2,at.y%256);
 let density=f32(read_tile(pixel,page))/63.75;
 let index=read_tile(pixel+vec2<i32>(1,0),page);
 var color=shade(index,row).rgb;
 if remaps.x>=0.0 {color=remap_color(index,remaps);}
 return vec4<f32>(color*density,density);
}
fn forest_canopy(color:vec3<f32>,direction:vec3<f32>,row:i32)->vec3<f32> {
 if scene.canopy.w<=0.0 || scene.canopy.y<1.0 {return color;}
 let full=max(scene.canopy_detail.x,1.0);
 let missing=select(smoothstep(full,max(full+1.0,scene.canopy.z),length(direction.xz)),1.0,scene.canopy.z<full);
 if missing<=0.0 {return color;}
 let dist=length(direction);
 let world=scene.origin.xz+scene.local_eye.xz+direction.xz;
 let cell=(world-scene.canopy_area.xy)/512.0-vec2<f32>(0.5);
 let at=vec2<i32>(floor(cell));let f=fract(cell);
 var remaps=vec2<f32>(-1.0);
 if row==0 {remaps=ray_rows(dist,scene.eye.y+direction.y);}
 let c=mix(mix(canopy_cell(at,row,remaps),canopy_cell(at+vec2<i32>(1,0),row,remaps),f.x),
           mix(canopy_cell(at+vec2<i32>(0,1),row,remaps),canopy_cell(at+vec2<i32>(1,1),row,remaps),f.x),f.y);
 if c.a<0.001 {return color;}

 let footprint=detail_footprint(direction,dist);
 let grain=detail_noise(world/76.0,0,74u)-0.5;
 let crowns=1.0+grain*0.38*detail_band(100.0,footprint);
 let coverage=(1.0-exp(-c.a*scene.canopy.w))*missing;
 return mix(color,c.rgb/c.a*crowns,coverage);
}
@fragment fn terrain_fragment(in:VertexOut)->@location(0) vec4<f32>{
 // Derivatives must be taken before the untextured early return.
 let grad=vec4<f32>(dpdx(in.uv),dpdy(in.uv));
 let world=scene.origin.xz+scene.local_eye.xz+in.direction.xz;

 let receiver=surface_normal(in.direction);
 let normal=select(receiver,normalize(in.terrain_normal),dot(in.terrain_normal,in.terrain_normal)>0.1 && smooth_weather());
 if in.layer<0.0 {
  if thermal_on() {return scenery(thermal_grey(thermal_ground(in.albedo,normal,in.direction,in.distance)));}
  let forest=forest_canopy(in.color,in.direction,0);
  // The albedo only lights flares (surface_color multiplies it by zero
  // otherwise), so its canopy is skipped when none burn.
  var bare=in.albedo;
  if flares_burning() {bare=forest_canopy(in.albedo,in.direction,base_row());}
  let detail=ground_detail(in.color,in.direction,normal);
  return scenery(aerial_perspective(surface_color(forest*detail.tint,in.direction,detail.normal,false,receiver,bare*detail.tint),in.direction,in.altitude));
 }
 var remaps=vec2<f32>(-1.0);
 if in.fog_enabled!=0u {remaps=ray_rows(in.distance,in.altitude);}
 var tex=filtered_tile(in.uv,grad,i32(in.layer),-1,in.light_row,remaps);
 if scene.canopy_detail.y>2.5 {tex=sample_tile(in.uv,i32(in.layer),base_row(),-1,-1,vec2<f32>(-1.0));}
 // Fitted bilinear coverage boundary; discarded water writes no depth.
 if tex.a<0.5 {discard;}
 if thermal_on() {
  var bare=sample_tile(in.uv,i32(in.layer),base_row(),-1,-1,vec2<f32>(-1.0)).rgb;
  if in.own_color>0.5 {bare=seam_apply(bare,terrain_flat_response(world));}
  bare=seam_apply(bare,terrain_seam_response(world));
  return scenery(thermal_grey(thermal_ground(bare,normal,in.direction,in.distance)));
 }
 // One detail term for the shaded colour and the flare-lit albedo, so a
 // dynamic light (flare, afterburner, fire, gun flash) only adds light.
 let seam_response=terrain_seam_response(world);
 if scene.canopy_detail.y>1.5 && scene.canopy_detail.y<2.5 {return vec4<f32>(vec3<f32>(seam_response.mask),1.0);}
 var flat_response=SeamResponse(vec3<f32>(0.0),1.0,0.0);
 if in.own_color>0.5 {flat_response=terrain_flat_response(world);}
 if scene.canopy_detail.y>3.5 {return vec4<f32>(vec3<f32>(select(0.0,1.0,any(abs(flat_response.delta)>vec3<f32>(0.0000001)))),1.0);}
 let field_color=seam_apply(seam_apply(tex.rgb,flat_response),seam_response);
 if scene.canopy_detail.y>2.5 {return scenery(field_color);}
 let detail=ground_detail(field_color,in.direction,normal);
 var albedo=field_color;
 if flares_burning() {albedo=seam_apply(seam_apply(sample_tile(in.uv,i32(in.layer),base_row(),-1,in.light_row,vec2<f32>(-1.0)).rgb,flat_response),seam_response);}
 let forest=forest_canopy(field_color,in.direction,0);
 if flares_burning() {albedo=forest_canopy(albedo,in.direction,base_row());}
 return scenery(aerial_perspective(surface_color(forest*detail.tint,in.direction,detail.normal,false,receiver,albedo*detail.tint),in.direction,in.altitude));
}
// Close-range ground detail (exploratory vegetation, 2026-10-16; desert and
// band-limited rework in pass 2). Presentation only. Colours stay the
// tile's own: the detail is a tint around 1 and a bent lighting normal.
// Fine patterns run on world coordinates wrapped to DETAIL_Q (one tile), so
// they keep their precision anywhere in a theater; every fine pattern
// repeats exactly on that period, so the wrap never shows. Coarse fields
// read the plain world position. Each pattern fades out before it is
// smaller than a pixel. vegetation.rs mirrors detail_hash, detail_noise,
// sand_sea and wadi to put objects on the same ground.
const DETAIL_Q:f32=32768.0;
const DETAIL_TAU:f32=6.2831853;
struct Detail { tint:vec3<f32>, normal:vec3<f32> }
fn detail_hash(c:vec2<i32>,salt:u32)->f32 {
 var h=(bitcast<u32>(c.x)*0x8da6b343u)^(bitcast<u32>(c.y)*0xd8163841u)^(salt*0xcb1ab31fu);
 h^=h>>15u;h*=0x2c1b3c6du;h^=h>>12u;h*=0x297a2d39u;h^=h>>15u;
 return f32(h>>8u)/16777216.0;
}
// Smooth value noise in 0..1, `period` cells (a power of two) or 0.
fn detail_noise(p:vec2<f32>,period:i32,salt:u32)->f32 {
 let i=floor(p);let f=p-i;let u=f*f*(3.0-2.0*f);
 let m=vec2<i32>(select(-1,period-1,period>0));
 let c=vec2<i32>(i);
 let a=detail_hash(c&m,salt);
 let b=detail_hash((c+vec2<i32>(1,0))&m,salt);
 let d=detail_hash((c+vec2<i32>(0,1))&m,salt);
 let e=detail_hash((c+vec2<i32>(1,1))&m,salt);
 return mix(mix(a,b,u.x),mix(d,e,u.x),u.y);
}
// Wrapped fine noise, `scale` feet per cell (a power of two up to DETAIL_Q).
fn fine_noise(q:vec2<f32>,scale:f32,salt:u32)->f32 {
 return detail_noise(q/scale,i32(DETAIL_Q/scale),salt);
}
// Feet of ground one pixel covers along the view at this point.
fn detail_footprint(direction:vec3<f32>,dist:f32)->f32 {
 let pixel=2.0/(max(scene.viewport.y,1.0)*1.7320508*max(scene.up.w,0.0001));
 return dist*pixel/max(abs(direction.y)/max(dist,1.0),0.03);
}
// 1 while a feature of `size` feet spans several pixels, 0 before it aliases.
fn detail_band(size:f32,footprint:f32)->f32 {
 return 1.0-smoothstep(0.3*size,1.1*size,footprint);
}
// A wave of `wavelength` feet along `angle`, its lattice vector rounded so
// the wave repeats exactly on DETAIL_Q. Phase in cycles.
fn detail_wave(q:vec2<f32>,angle:f32,wavelength:f32)->f32 {
 let m=round(vec2<f32>(cos(angle),sin(angle))*DETAIL_Q/wavelength);
 return dot(q,m)/DETAIL_Q;
}
fn detail_lattice(angle:f32,wavelength:f32)->vec2<f32> {
 return round(vec2<f32>(cos(angle),sin(angle))*DETAIL_Q/wavelength)/DETAIL_Q;
}
// 0..1: how much a point lies in a sand sea, before the tile's colour says
// whether it is sand at all. Fields a few nautical miles across.
fn sand_sea(w:vec2<f32>)->f32 {
 let n=detail_noise(w/36000.0,0,11u)*0.7+detail_noise(w/9000.0,0,12u)*0.3;
 return smoothstep(0.42,0.60,n);
}
// 0..1: a dry wadi bed, contours of a warped noise field, about 150 ft
// wide, meandering a few thousand feet apart.
fn wadi(w:vec2<f32>,widen:f32)->f32 {
 let warp=vec2<f32>(detail_noise(w/5200.0,0,13u),detail_noise(w/5200.0,0,14u))-vec2<f32>(0.5);
 let n=detail_noise(w/14000.0+warp*0.9,0,15u)-0.5;
 let width=(0.008+0.006*detail_noise(w/3000.0,0,16u))*widen;
 return 1.0-smoothstep(width*0.45,width,abs(n));
}
// Transverse dune profile at phase t: a flat corridor, a long convex
// windward rise, a sharp crest and a steep slip face. Height share and its
// rate per cycle.
fn dune_profile(t:f32)->vec2<f32> {
 let flat=0.22;let crest=0.86;
 if t<flat {return vec2<f32>(0.0);}
 if t<crest {
  let s=(t-flat)/(crest-flat);
  return vec2<f32>(1.0-(1.0-s)*(1.0-s),2.0*(1.0-s)/(crest-flat));
 }
 let r=(t-crest)/(1.0-crest);
 return vec2<f32>(1.0-r,-1.0/(1.0-crest));
}
// Ripple profile: windward rise, short lee drop. Height share and rate.
fn ripple_profile(t:f32)->vec2<f32> {
 if t<0.74 {return vec2<f32>(t/0.74,1.0/0.74);}
 return vec2<f32>((1.0-t)/0.26,-1.0/0.26);
}
// Height in feet of the barchan field: at most one crescent per 1,600 ft
// cell, horns trailing downwind. A rounded windward body, cut on its lee
// by a bowl whose rim is the crest and whose side is the slip face.
fn barchan(w:vec2<f32>,along:vec2<f32>,across:vec2<f32>)->f32 {
 let cell=1600.0;
 let p=vec2<f32>(dot(w,along),dot(w,across))/cell;
 let c=floor(p);let ci=vec2<i32>(c);
 if detail_hash(ci,70u)<0.3 {return 0.0;}
 let size=0.14+0.14*detail_hash(ci,71u);
 let room=0.5-1.45*size;
 let centre=c+vec2<f32>(0.5)+(vec2<f32>(detail_hash(ci,72u),detail_hash(ci,73u))-vec2<f32>(0.5))*2.0*room;
 let l=(p-centre)/size;
 let x=select(l.x,l.x/1.5,l.x>0.0);
 let r=sqrt(x*x+l.y*l.y);
 if r>=1.0 {return 0.0;}
 let body=1.0-smoothstep(0.0,1.0,r);
 let d=length(l-vec2<f32>(0.6,0.0));
 let fall=clamp(1.0-(0.7-d)/0.22,0.0,1.0);
 let height=size*cell*(0.16+0.06*detail_hash(ci,74u));
 return height*body*fall;
}
// The distance is the interpolated offset's length: the vertices' own
// distance, interpolated across a terrain triangle thousands of feet wide,
// reads far too long near the camera.
fn ground_detail(color:vec3<f32>,direction:vec3<f32>,normal:vec3<f32>)->Detail {
 var out=Detail(vec3<f32>(1.0),normal);
 let dist=length(direction);
 let mode=scene.veg.x;
 let reach=scene.veg.y;
 if mode<0.5 || reach<1.0 {return out;}
 let alt=scene.eye.y;
 // Near patterns follow the Graphics distance; the coarse colour fields
 // and dune relief reach farther and fade slowly, and average out to the
 // tile's colour, so their edge never shows as a ring.
 let near=(1.0-smoothstep(reach*0.45,reach,dist))*(1.0-smoothstep(2200.0,7500.0,alt));
 let far=(1.0-smoothstep(reach*2.0,reach*6.0,dist))*(1.0-smoothstep(9000.0,26000.0,alt));
 if far<0.01 {return out;}
 let fine=mode>1.5;
 let w=scene.origin.xz+scene.local_eye.xz+direction.xz;
 let q=scene.origin.xz-floor(scene.origin.xz/DETAIL_Q)*DETAIL_Q+scene.local_eye.xz+direction.xz;
 let foot=detail_footprint(direction,dist);
 let luma=dot(color,vec3<f32>(0.299,0.587,0.114));
 let sat=max(color.r,max(color.g,color.b))-min(color.r,min(color.g,color.b));
 let slope=length(normal.xz);
 let climate=scene.veg.w;
 var tint=vec3<f32>(1.0);
 var bend=vec2<f32>(0.0);
 // Ratios of linear colour: sand is red over blue, with green well under red.
 let warm=smoothstep(1.25,1.7,color.r/max(color.b,0.0001))*smoothstep(0.26,0.38,color.g/max(color.r,0.0001));
 let green=smoothstep(1.02,1.22,color.g/max(color.r,0.0001));
 let grey=1.0-smoothstep(0.03,0.09,sat/max(luma,0.02)*0.25);
 if climate>0.5 && climate<1.5 {
  // Desert. Sand is the warm, not green colours; greens are oases and
  // the river's fields, greys rock or town.
  let sand=warm*(1.0-green)*(1.0-grey*0.7);
  let stony=clamp(max(smoothstep(0.05,0.18,slope),grey*0.8)+(1.0-smoothstep(0.10,0.22,luma))*0.5,0.0,1.0);
  let sea=sand_sea(w)*sand*(1.0-stony);
  let gravel=(1.0-sea)*sand*smoothstep(0.35,0.7,detail_noise(w/7000.0,0,20u));
  // Coarse colour, miles across: ochre, red and pale sands, grey serir.
  let n1=detail_noise(w/11000.0,0,30u);let n2=detail_noise(w/4200.0,0,31u);let n3=detail_noise(w/26000.0,0,32u);
  var coarse=vec3<f32>(1.0);
  coarse*=mix(vec3<f32>(1.0),vec3<f32>(1.16,0.94,0.74),smoothstep(0.42,0.75,n3));
  coarse*=mix(vec3<f32>(1.0),vec3<f32>(1.22,0.86,0.68),smoothstep(0.50,0.80,n1));
  coarse*=mix(vec3<f32>(1.0),vec3<f32>(1.12,1.10,1.06),smoothstep(0.52,0.82,n2)*(1.0-gravel));
  coarse=mix(coarse,coarse*vec3<f32>(0.80,0.77,0.74),gravel*0.85);
  // Desert varnish: dark brown patina on stony ground.
  let varnish=stony*smoothstep(0.40,0.72,detail_noise(w/1800.0,0,21u));
  coarse=mix(coarse,coarse*vec3<f32>(0.58,0.48,0.42),varnish*0.8);
  // Wadis: a pale silt bed, darker banks.
  let bed=wadi(w,1.0);let bank=max(wadi(w,2.6)-bed,0.0);
  coarse=mix(coarse,vec3<f32>(1.12,1.10,1.06),bed*0.75);
  coarse=mix(coarse,vec3<f32>(0.78,0.75,0.70),bank*0.5);
  tint=mix(vec3<f32>(1.0),coarse,far);
  let wind=0.65;
  let along=vec2<f32>(cos(wind),sin(wind));
  let across=vec2<f32>(-along.y,along.x);
  // Wind streaks: long pale and dark bands along the wind on open sand.
  let streak=detail_noise(vec2<f32>(dot(w,along)/1400.0,dot(w,across)/110.0),0,33u)-0.5;
  tint*=vec3<f32>(1.0+streak*0.10*sand*(1.0-stony)*far*detail_band(220.0,foot));
  // Dunes, as relief on the lighting plus a little form shading, so they
  // read under a high sun: transverse ridges in the sand seas, barchan
  // crescents on their edges.
  var slip=0.0;
  var crest=0.0;
  let dune_w=far*detail_band(160.0,foot);
  if sea>0.01 && dune_w>0.01 {
   let seg=smoothstep(0.25,0.7,detail_noise(vec2<f32>(dot(w,along)/900.0,dot(w,across)/2600.0),0,22u));
   let shift=along*(detail_noise(w/2600.0,0,23u)-0.5)*900.0;
   let k1=detail_lattice(wind,1150.0);
   let p1=dune_profile(fract(dot(q+shift,k1)));
   let core=smoothstep(0.35,0.8,sea);
   let a1=80.0*core*(0.35+0.65*seg);
   let k2=detail_lattice(wind+0.55,1750.0);
   let p2=dune_profile(fract(dot(q+shift*0.7,k2)));
   let a2=46.0*core*(0.4+0.6*(1.0-seg));
   var g=k1*a1*p1.y;var h=p1.x*a1;var lee=p1.y<0.0;
   if p2.x*a2>h {g=k2*a2*p2.y;h=p2.x*a2;lee=p2.y<0.0;}
   bend-=g*dune_w;
   crest=max(crest,h/80.0);
   slip=max(slip,select(0.0,1.0,lee)*min(h/25.0,1.0));
  }
  let fringe=sand*(1.0-stony)*smoothstep(0.02,0.25,sand_sea(w))*(1.0-smoothstep(0.6,0.9,sea));
  if fringe>0.01 && dune_w>0.01 {
   let e=6.0;
   let h0=barchan(w,along,across);
   let hx=barchan(w+vec2<f32>(e,0.0),along,across);
   let hz=barchan(w+vec2<f32>(0.0,e),along,across);
   let g=vec2<f32>(hx-h0,hz-h0)/e;
   bend-=g*fringe*dune_w;
   crest=max(crest,h0/70.0*fringe);
   // The slip face is the steep side facing downwind.
   slip=max(slip,smoothstep(0.25,0.5,-dot(g,along))*min(h0/12.0,1.0)*fringe);
  }
  // Interdune corridors show darker gravel; crests are clean and pale.
  let form=mix(vec3<f32>(0.92,0.89,0.86),vec3<f32>(1.05,1.03,1.0),smoothstep(0.0,0.9,crest));
  tint*=mix(vec3<f32>(1.0),form,max(sea,fringe)*dune_w);
  tint*=mix(vec3<f32>(1.0),vec3<f32>(1.03,0.96,0.88),slip*dune_w*0.8);
  if near>0.01 {
   // Ripples on sand sheets and windward flanks, never a carpet over a
   // whole dune field, and never on a slip face. Three wind directions,
   // spacing 20 to 40 ft, soft enough that the dune's own light still
   // reads. Crests are slightly paler than the troughs.
   let sheet=smoothstep(0.5,0.85,detail_noise(w/2600.0,0,24u))*sand*(1.0-gravel)*(1.0-stony);
   let flank=max(sea,fringe)*(1.0-slip)*smoothstep(0.2,0.6,detail_noise(w/2200.0,0,29u));
   let strength=max(sheet,flank)*0.45;
   if strength>0.01 {
    let shift=vec2<f32>(detail_noise(w/700.0,0,25u),detail_noise(w/700.0,0,26u))*40.0;
    let wa=detail_noise(w/5000.0,0,27u);let wb=detail_noise(w/5000.0,0,28u);
    let weights=vec3<f32>(wa,1.0-wa,wb)*vec3<f32>(1.0,1.0,0.8);
    let angles=vec3<f32>(wind-0.4,wind+0.05,wind+0.45);
    let lengths=vec3<f32>(24.0,31.0,40.0);
    var rg=vec2<f32>(0.0);var shade=0.0;var total=0.0;
    for(var i=0;i<3;i++){
     let k=detail_lattice(angles[i],lengths[i]);
     let p=ripple_profile(fract(dot(q+shift,k)));
     let band=detail_band(lengths[i],foot);
     rg+=k*p.y*0.035*lengths[i]*weights[i]*band;
     shade+=(p.x-0.5)*weights[i]*band;
     total+=weights[i];
    }
    if fine {
     let k=detail_lattice(wind+0.2,9.0);
     let p=ripple_profile(fract(dot(q+shift,k)));
     let band=detail_band(9.0,foot);
     rg+=k*p.y*0.06*9.0*band*0.6;
     shade+=(p.x-0.5)*0.4*band;
    }
    let s=strength*near/max(total,0.5);
    bend-=rg*s;
    tint*=vec3<f32>(1.0+shade*0.06*s);
   }
   // Grit: soft grains, never blocks, and pebbles on gravel and stony ground.
   var grain=(fine_noise(q,4.0,40u)-0.5)*0.14*detail_band(4.0,foot)
       +(fine_noise(q,1.0,41u)-0.5)*0.16*detail_band(1.0,foot);
   if fine {grain+=(fine_noise(q,0.5,42u)-0.5)*0.12*detail_band(0.5,foot);}
   let stones=(fine_noise(q,2.0,43u)-0.5)*0.34*detail_band(2.0,foot)
       +(fine_noise(q,8.0,44u)-0.5)*0.18*detail_band(8.0,foot);
   let rough=max(gravel,stony*0.8);
   grain+=stones*rough;
   // Pebbles: sparse dark and pale stones about a foot across, one 2 ft
   // cell each.
   let cell=vec2<i32>(floor(q/2.0))&vec2<i32>(i32(DETAIL_Q/2.0)-1);
   let roll=detail_hash(cell,45u);
   let at=vec2<f32>(detail_hash(cell,46u),detail_hash(cell,47u))*0.4+vec2<f32>(0.3);
   let spot=length(fract(q/2.0)-at)*2.0;
   let pebble=(1.0-smoothstep(0.12+0.2*detail_hash(cell,48u),0.3+0.2*detail_hash(cell,48u),spot))*step(1.0-(0.02+0.3*rough),roll)*detail_band(1.0,foot);
   grain-=pebble*select(0.22,-0.14,roll>0.995-0.05*rough);
   tint*=vec3<f32>(1.0+grain*near);
  }
 } else {
  // Temperate, tropical and arctic ground.
  let field=smoothstep(0.80,0.95,color.r/max(color.g,0.0001))*smoothstep(1.25,1.6,color.r/max(color.b,0.0001));
  let wood=(1.0-field)*green*(1.0-smoothstep(0.08,0.15,luma));
  let grass=(1.0-field)*(1.0-wood)*green;
  let rock=(1.0-green)*(1.0-field)*grey;
  let n1=detail_noise(w/9000.0,0,50u);let n2=detail_noise(w/3200.0,0,51u);
  var coarse=vec3<f32>(1.0);
  coarse*=mix(vec3<f32>(1.0),vec3<f32>(1.06,1.03,0.86),smoothstep(0.55,0.85,n1)*(grass+field));
  coarse*=mix(vec3<f32>(1.0),vec3<f32>(0.88,0.94,0.88),smoothstep(0.55,0.85,n2)*(grass+wood));
  // Fields: a patchwork of plots, each its own crop shade.
  let plot=vec2<i32>(floor(w/1024.0));
  let plot_edge=min(min(fract(w.x/1024.0),1.0-fract(w.x/1024.0)),min(fract(w.y/1024.0),1.0-fract(w.y/1024.0)));
  let plot_blend=smoothstep(0.0,0.035,plot_edge);
  let crop=mix(detail_noise(w/1024.0,0,52u),detail_hash(plot,52u),plot_blend);
  coarse*=mix(vec3<f32>(1.0),vec3<f32>(0.9+0.2*crop,0.92+0.14*mix(detail_noise(w/1024.0,0,53u),detail_hash(plot,53u),plot_blend),0.9+0.1*crop),field);
  tint=mix(vec3<f32>(1.0),coarse,far);
  if near>0.01 {
   var v=0.0;
   v+=((fine_noise(q,16.0,54u)-0.5)*0.16+(fine_noise(q,4.0,55u)-0.5)*0.12*detail_band(4.0,foot)
      +(fine_noise(q,1.0,56u)-0.5)*0.08*detail_band(1.0,foot))*grass;
   v+=((fine_noise(q,32.0,57u)-0.5)*0.22+(fine_noise(q,8.0,58u)-0.5)*0.26*detail_band(8.0,foot))*wood;
   // Furrows run one of four ways per plot.
   let way=f32(u32(crop*4.0))*0.785398;
   let furrow=sin(DETAIL_TAU*dot(q,detail_lattice(way,12.0)));
   v+=(furrow*plot_blend*0.10*detail_band(12.0,foot)+(fine_noise(q,2.0,59u)-0.5)*0.08*detail_band(2.0,foot))*field;
   v+=((fine_noise(q,8.0,60u)-0.5)*0.22+(fine_noise(q,2.0,61u)-0.5)*0.14*detail_band(2.0,foot))*rock;
   if fine {v+=(fine_noise(q,0.5,62u)-0.5)*0.06*detail_band(0.5,foot)*(grass+field);}
   if climate>1.5 && climate<2.5 {v*=0.5;}
   tint*=vec3<f32>(1.0+v*near);
  }
 }
 if dot(bend,bend)>0.0 {
  out.normal=normalize(normal+vec3<f32>(bend.x,0.0,bend.y));
 }
 out.tint=tint;
 return out;
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
fn aircraft_color(in:VertexOut,normal:vec3<f32>)->vec4<f32>{
 var color=in.color;
 var albedo=in.albedo;
 // Afterburner flames glow: they keep their daylight colors at night instead
 // of the weather's darkening remaps (docs/spec/engine-material.md).
 let flame=in.layer == -6.0 || in.layer == -7.0;
 if flame {color=in.albedo;}
 if in.layer<=-3.0 && in.layer>=-4.0 {color=engine_color(in.uv,clamp(-in.layer-3.0,0.0,1.0));albedo=color;}
 if in.layer>=0.0 || in.layer == -2.0 || in.layer == -5.0 || in.layer == -7.0 {
  var remaps=vec2<f32>(-1.0);if in.fog_enabled!=0u && !flame {remaps=ray_rows(in.distance,in.altitude);}
  let tex=sample_tile(in.uv,i32(max(in.layer,0.0)),select(0,base_row(),flame),-1,in.light_row,remaps);
  if (in.layer == -2.0 || in.layer == -7.0) && tex.a < 0.5 { discard; }
  color=mix(color,tex.rgb,tex.a);
  if flares_burning() {
   let bare=sample_tile(in.uv,i32(max(in.layer,0.0)),base_row(),-1,in.light_row,vec2<f32>(-1.0));
   albedo=mix(albedo,bare.rgb,bare.a);
  } else {albedo=color;}
  if in.layer == -5.0 {
   let luminance=dot(color,vec3<f32>(0.2126,0.7152,0.0722));
   color=mix(color,vec3<f32>(0.95,0.45,0.08)*(0.3+0.7*luminance),0.75);
  }
 }
 if thermal_on() {return vec4<f32>(thermal_grey(thermal_object(in,albedo,normal)),1.0);}
 if textureDimensions(palette).y<=1u || (in.own_color>0.0 && in.layer<0.0 && in.layer != -2.0 && in.layer != -5.0) { color=mix(color,linear(scene.sky.rgb),haze(in.distance)); }
 if in.layer != -6.0 && in.layer != -7.0 && !(in.layer < -3.0 && in.layer >= -4.0) {color=surface_color(color,in.direction,normal,true,normal,albedo);}
 if in.fog_enabled!=0u {color=aerial_perspective(color,in.direction,in.altitude);}
 else {color=cloud_occlusion(color,in.direction,in.altitude);}
 return vec4<f32>(color,1.0);
}
// Glazing composites after opaque geometry. The depth prepass selects one
// nearest surface, avoiding compounded opacity through overlapping glass.
@fragment fn fragment(in:VertexOut)->@location(0) vec4<f32>{
 let normal=surface_normal(in.direction);
 if in.layer == -5.0 || in.layer == -8.0 {discard;}
 return aircraft_color(in,normal);
}
// Fitted optical glow. Additive radiance is independent of surface lighting.
// Visible-light adaptation only. Thermal keeps its existing heat response;
// NVG receives this visible scene through its own logarithmic gain and bloom.
fn tracer_night()->f32 {
 return select(device_night(),0.0,thermal_on());
}
@fragment fn tracer_fragment(in:VertexOut)->@location(0) vec4<f32>{
 if in.layer != -8.0 {discard;}
 let gun=in.heat==2.0;
 let night=select(0.0,tracer_night(),gun);
 var uv=in.uv;
 if night>0.0 {
  uv=vec2<f32>((uv.x-0.5)*in.terrain_normal.z+0.5,uv.y*in.terrain_normal.y);
 }
 let edge=1.0-smoothstep(0.8,1.0,abs(uv.y));
 let ends=smoothstep(0.0,0.12,uv.x)*(1.0-smoothstep(0.88,1.0,uv.x));
 let unresolved=select(0.0,in.terrain_normal.x,gun);
 let core=3.0*exp(-mix(120.0,6.0,unresolved)*uv.y*uv.y);
 let halo=0.65*exp(-5.0*uv.y*uv.y);
 let cloud=clamp(cloud_occlusion(vec3<f32>(1.0),in.direction,in.altitude)
     -cloud_occlusion(vec3<f32>(0.0),in.direction,in.altitude),vec3<f32>(0.0),vec3<f32>(1.0));
 let distance=in.distance/(1.0+5.0*night);
 let visibility=(1.0-clamp(haze(distance),0.0,1.0))*(1.0-air_opacity(distance,in.altitude));
 var core_color=vec3<f32>(1.0,0.92,0.68);
 var body_color=vec3<f32>(1.0,0.25,0.025);
 var glow_color=vec3<f32>(1.0,0.06,0.025);
 if night>0.0 {
  core_color=mix(core_color,vec3<f32>(1.0,0.72,0.38),night);
  body_color=mix(body_color,vec3<f32>(1.0,0.09,0.025),night);
 }
 if gun && in.color.r>0.5 && !thermal_on() {
  core_color=mix(vec3<f32>(0.78,1.0,0.68),vec3<f32>(0.72,1.0,0.38),night);
  body_color=vec3<f32>(0.09,1.0,0.025);
  glow_color=vec3<f32>(0.06,1.0,0.025);
 }
 // Red daytime arithmetic is unchanged. Keep the 105's 1.5 radiance factor.
 var radiance=(core*core_color+halo*body_color)*edge*ends*visibility*cloud*in.color.g*(1.0+night);
 if night>0.0 {
  let local=vec2<f32>(2.0*in.uv.x-1.0,in.uv.y);
  let r=length(local);
  // Flare glare's two soft lobes, without its canopy cross-streaks.
  let soft=1.1*exp(-r*r*10.0)+0.4*exp(-r*3.2);
  let window=1.0-smoothstep(0.85,1.0,max(abs(local.x),abs(local.y)));
  // 75% of the flare halo profile peak, before distance attenuation.
  // Use the tracer visibility envelope only: close-range glare falloff
  // erased the halo at 4 nm even though the streak remained visible.
  let strength=0.75*night;
  radiance+=glow_color*soft*window*strength*visibility*cloud*in.color.g;
 }
 return thermal_light(vec4<f32>(radiance,0.0),1.0);
}
// Static world textures use index255/mask cutouts without opaque backing color.
@fragment fn airport_solid_fragment(in:VertexOut)->@location(0) vec4<f32>{
 if in.layer>=0.0 {discard;}
 // A ground shadow decal (retail's colour 6 quad under the suspension
 // bridge and the crane): dark, and translucent by keeping half the pixels
 // in the 4 by 4 ordered pattern.
 if in.layer==-9.0 && bayer4(vec2<u32>(in.clip.xy))>=8u {discard;}
 return aircraft_color(in,surface_normal(in.direction));
}
// City fill: only untextured triangles reach this pass, so it needs no
// discard and keeps the early depth test.
@fragment fn fill_solid_fragment(in:VertexOut)->@location(0) vec4<f32>{
 return aircraft_color(in,surface_normal(in.direction));
}
// City fill full detail while fading in at the draw distance (a view whose
// full detail reaches it, such as the target display): the coverage mask.
@fragment fn fill_solid_fade_fragment(in:VertexOut)->FillOut{
 return FillOut(aircraft_color(in,surface_normal(in.direction)),fill_coverage(in.clip.xy,in.terrain_normal.x,in.terrain_normal.y));
}
@fragment fn fill_decal_fade_fragment(in:VertexOut)->FillOut{
 return FillOut(airport_color(in),fill_coverage(in.clip.xy,in.terrain_normal.x,in.terrain_normal.y));
}
// Vegetation objects (vegetation_renderer.rs). A mesh is normalised to a
// height of 1; `turn` holds the yaw's cosine and sine, the height in feet
// and the fade, `shape` the width, the ground rise east and north and the
// look (0 foliage, 1 solid, 3 recoloured retail rock). Solid objects lean
// with the ground; trees stand upright.
struct VegOut {
 @builtin(position) @invariant clip:vec4<f32>, @location(0) uv:vec2<f32>, @location(1) @interpolate(flat) layer:f32,
 @location(2) color:vec3<f32>, @location(3) albedo:vec3<f32>, @location(4) direction:vec3<f32>,
 @location(5) normal:vec3<f32>, @location(6) @interpolate(flat) fade:vec2<f32>, @location(7) @interpolate(flat) look:f32,
 @location(8) @interpolate(flat) mean:vec3<f32>, @location(9) height:f32, @location(10) altitude:f32,
 @location(11) @interpolate(flat) fog_enabled:u32,
 @location(12) @interpolate(flat) overhead:u32,
 @location(13) @interpolate(flat) reach:f32
}
// Foliage and rock share distance coverage without changing their size.
fn veg_turn(turn:vec4<f32>,shape:vec4<f32>)->vec4<f32> {
 // All kinds use the same per-instance coverage fade, including rocks.
 return turn;
}
fn veg_rotate(local:vec2<f32>,turn:vec2<f32>)->vec2<f32> {
 return vec2<f32>(local.x*turn.x+local.y*turn.y,local.y*turn.x-local.x*turn.y);
}
fn veg_place(position:vec3<f32>,place:vec4<f32>,turn:vec4<f32>,shape:vec4<f32>)->vec3<f32> {
 let local=vec3<f32>(position.x*turn.z*shape.x,position.y*turn.z,position.z*turn.z*shape.x);
 let flat=veg_rotate(local.xz,turn.xy);
 let lean=select(0.0,1.0,shape.w>0.5);
 return place.xyz+vec3<f32>(flat.x,local.y+lean*dot(flat,shape.yz),flat.y);
}
@vertex fn veg_vertex(@builtin(vertex_index) vertex:u32,@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) normal:vec3<f32>,@location(4) part:f32,@location(5) place:vec4<f32>,@location(6) spin:vec4<f32>,@location(7) shape:vec4<f32>,@location(8) tint:vec4<f32>,@location(9) mean:vec4<f32>,@location(10) top:vec4<f32>)->VegOut {
 let turn=veg_turn(spin,shape);
 // Stable seeded crosses, optional horizontal canopy, or a facing card.
 let mode=u32(scene.veg.z);
 let card=vertex/6u;
 let overhead=(mode==2u && card==2u)||(mode==3u && card==1u);
 let facing=scene.local_eye.xz-place.xz;
 var right=normalize(vec2<f32>(facing.y,-facing.x)+vec2<f32>(0.00001,0.0));
 if mode==1u || mode==2u {
  right=veg_rotate(vec2<f32>(1.0,0.0),spin.xy);
  if card==1u {right=veg_rotate(vec2<f32>(0.0,-1.0),spin.xy);}
 }
 var offset=right*position.x*turn.z*shape.x*tint.w;
 var p=place.xyz+vec3<f32>(offset.x,position.y*turn.z,offset.y);
 let azimuth=atan2(facing.x,facing.y)-atan2(spin.y,spin.x);
 var view=u32(floor(azimuth/1.570796327+0.5)+8.0)%4u;
 if mode==1u || mode==2u {view=card%2u;}
 var code=mean.w;
 var coverage=turn.w;
 let crown=place.xyz+vec3<f32>(0.0,turn.z*top.y,0.0);
 let ray=scene.local_eye.xyz-crown;
 let has_top=all(top.zw>vec2<f32>(0.0));
 let top_share=select(0.0,smoothstep(0.57357644,0.90630779,ray.y/max(length(ray),0.0001)),has_top);
 if mode==2u || mode==3u { coverage*=select(1.0-top_share,top_share,overhead); }
 if overhead {
  code=top.x; view=0u;
  // The crown sits inside the upright silhouette, relative to its grounded base.
  // Opaque side-crown extents, with the same seeded height/width as the side.
  // Coverage and canopy height do not change this world size.
  let local=vec2<f32>(position.x,position.y*2.0-1.0)*turn.z*shape.x*top.zw;
  offset=veg_rotate(local,spin.xy);
  p=crown+vec3<f32>(offset.x,0.0,offset.y);
 }
 var sprite=code+f32(view);
 var sprite_uv=uv;
 if code<0.0 {
  let packed=u32(-code-1.0);
  sprite=f32(packed/4u);
  let cell=packed%4u;
  sprite_uv=(vec2<f32>(f32(cell%2u),f32(cell/2u))*128.0+vec2<f32>(2.5)+uv*123.0)/256.0;
 }
 let rel=p-scene.local_eye.xyz;
 let base=placed_vertex(p+scene.origin.xyz,rel,sprite_uv,sprite,vec3<f32>(1.0),select(-1.0,part,part>=0.0));
 var out:VegOut;
 out.clip=base.clip;
 // A fully hidden card is degenerate in both passes, avoiding all cutout
 // texture and lighting work during the side/top angular hand-off.
 if coverage<=0.0 {out.clip=vec4<f32>(0.0,0.0,0.0,1.0);}
 out.uv=sprite_uv;out.layer=sprite;out.direction=rel;out.altitude=base.altitude;
 out.fog_enabled=base.fog_enabled;out.overhead=select(0u,1u,overhead);
 let n=vec3<f32>(normal.x*turn.x+normal.z*turn.y,normal.y,normal.z*turn.x-normal.x*turn.y);
 out.normal=normalize(vec3<f32>(n.x,n.y*max(shape.x,0.2),n.z));
 // A baked rock card has no useful face normal. Light it with its ground
 // slope, not the arbitrary vertical shared quad normal.
 if shape.w>0.5 {out.normal=normalize(vec3<f32>(-shape.y,1.0,-shape.z));}
 if overhead {out.normal=vec3<f32>(0.0,1.0,0.0);}
 out.fade=vec2<f32>(coverage,place.w);out.look=shape.w;out.mean=mean.xyz;out.height=select(position.y,1.0,overhead);
 // Half the tree's shadow card width; rocks cast no shadow card.
 out.reach=select(0.5*turn.z*shape.x*tint.w,0.0,shape.w>0.5);
 // Untextured colour: a retail face's palette colour, or a drawn part.
 out.color=base.color;out.albedo=base.albedo;
 if part<0.0 {
  var c=tint.xyz;
  if part>-1.5 {c=vec3<f32>(0.036,0.10,0.024)*tint.xyz;}
  else if part>-2.5 {c=vec3<f32>(0.075,0.052,0.034)*(0.6+0.4*tint.y/max(tint.y,0.0001));}
  out.color=c;out.albedo=c;
 } else if layer<0.0 {
  out.color=base.color*tint.xyz;out.albedo=base.albedo*tint.xyz;
 } else {
  out.color=tint.xyz;out.albedo=tint.xyz;
 }
 return out;
}
// Sun visibility for vegetation: one bilinear shadow tap instead of the
// terrain's soft search, which would be too costly for overlapping cards.
// When trees cast (surface.quality.z), a tree looks up its light from just
// sunward of its own shadow card (`reach` is half its width), so it is not
// shadowed by itself; other trees and objects still shade it.
fn veg_visibility(relative:vec3<f32>,reach:f32)->f32 {
 if surface.origin.w<=0.0 {return 1.0;}
 var at=relative;
 let level=length(surface.sun.xz);
 if surface.quality.z>0.0 && reach>0.0 && level>0.0001 {
  at+=vec3<f32>(surface.sun.x,0.0,surface.sun.z)/level*reach*1.25;
 }
 for(var cascade=0;cascade<3;cascade++) {
  let m=surface.maps[cascade];
  let p=shadow_project(at+surface.sun.xyz*m.scale.x*1.5,cascade);
  if max(abs(p.x),abs(p.y))<0.98 && p.z>0.0 && p.z<1.0 {
   let uv=p.xy*vec2<f32>(0.5,-0.5)+vec2<f32>(0.5);
   return 1.0-shadow_tap(uv,p.z,vec2<f32>(0.0),cascade).x;
  }
 }
 return 1.0;
}
// Light on vegetation. Foliage takes its light mostly from above and
// wraps around, so a leaf card edge-on to the sun is not black; `occlusion`
// darkens the inside and the foot of a crown.
fn veg_light(color:vec3<f32>,albedo:vec3<f32>,relative:vec3<f32>,normal:vec3<f32>,foliage:bool,occlusion:f32,reach:f32)->vec3<f32> {
 if thermal_on() {return thermal_grey(thermal_ground(albedo,normal,relative,length(relative)));}
 if surface.sun.w<=0.0 {return color;}
 var n=normal;
 if dot(n,relative)>0.0 {n=-n;}
 // Rock is lit as the ground it stands on is.
 if !foliage {return surface_color(color,relative,n,false,n,albedo);}
 var facing=max(dot(n,surface.sun.xyz),0.0);
 if foliage {
  n=normalize(mix(n,vec3<f32>(0.0,1.0,0.0),0.55));
  facing=clamp((dot(n,surface.sun.xyz)+0.4)/1.4,0.0,1.0);
 }
 let day=smoothstep(-0.104528,0.0348995,surface.reserved.y);
 let visibility=veg_visibility(relative,reach)*sunlight_transmission(surface.origin.y+relative.y)*surface.reserved.x;
 let tint=mix(vec3<f32>(1.0),solar_tint(surface.reserved.y),0.65);
 let skyward=clamp(n.y*0.5+0.5,0.0,1.0);
 let ambient=mix(vec3<f32>(0.12,0.14,0.18),vec3<f32>(0.28,0.32,0.39),skyward);
 let night=vec3<f32>(mix(0.20,0.32,skyward)+0.30*max(dot(n,surface.moon.xyz),0.0));
 let illumination=mix(night,ambient,day)*occlusion+0.90*facing*visibility*tint*mix(1.0,occlusion,0.5);
 let lit=color*illumination+albedo*flare_light(relative,n);
 return mix(color,lit,1.0-clamp(haze(length(relative)),0.0,1.0));
}
// Match colour-pass coverage exactly, including multisample cutouts and fade.
// Palette shading cannot alter alpha, so this skips both palette and lighting.
fn veg_alpha(uv:vec2<f32>,layer:i32)->f32 {
 let size=vec2<i32>(tile_size());
 let p=uv*vec2<f32>(size)-vec2<f32>(0.5);
 let base=floor(p);
 let f=p-base;
 var alpha=0.0;
 for(var j=0;j<2;j++){
  for(var i=0;i<2;i++){
   let at=clamp(vec2<i32>(base)+vec2<i32>(i,j),vec2<i32>(0),size-vec2<i32>(1));
   let a=select(1.0,0.0,read_tile(at,layer)==255u);
   let w=select(1.0-f.x,f.x,i==1)*select(1.0-f.y,f.y,j==1);
   alpha+=a*w;
  }
 }
 return alpha;
}
// Side and top occupy complementary samples, not just complementary alpha
// fractions. At full distance opacity their overlapping interiors cover every
// sample exactly once, including halfway through the angular hand-off.
fn veg_coverage(in:VegOut,alpha:f32)->u32 {
 if in.overhead!=0u {return ~fill_coverage(in.clip.xy,1.0-alpha,in.fade.y);}
 return fill_coverage(in.clip.xy,alpha,in.fade.y);
}
@fragment fn veg_depth(in:VegOut)->FillOut {
 var alpha=in.fade.x;
 if in.layer>=0.0 {
  let a=veg_alpha(in.uv,i32(in.layer));
  alpha*=clamp((a-0.5)/max(fwidth(a),0.0001)+0.5,0.0,1.0);
 }
 let mask=veg_coverage(in,alpha);
 if mask==0u {discard;}
 return FillOut(vec4<f32>(0.0),mask);
}
@fragment fn veg_fragment(in:VegOut)->FillOut {
 var color=in.color;
 var albedo=in.albedo;
 var alpha=in.fade.x;
 if in.layer>=0.0 {
  var remaps=vec2<f32>(-1.0);
  if in.fog_enabled!=0u {remaps=ray_rows(length(in.direction),in.altitude);}
  let tex=sample_tile(in.uv,i32(in.layer),0,-1,-1,remaps);
  // Cut-out edges resolve through the sample mask, not a hard step.
  let edge=clamp((tex.a-0.5)/max(fwidth(tex.a),0.0001)+0.5,0.0,1.0);
  alpha*=edge;
  var base=tex.rgb;
  var bare=base;
  if flares_burning() || thermal_on() {bare=sample_tile(in.uv,i32(in.layer),base_row(),-1,-1,vec2<f32>(-1.0)).rgb;}
  if in.look>2.5 {
   // Retail rock in the local ground's colour, keeping its shading.
   let k=dot(base,vec3<f32>(0.2126,0.7152,0.0722))/max(dot(in.mean,vec3<f32>(0.2126,0.7152,0.0722)),0.0001);
   base=in.color*k;bare=in.albedo*k;
  } else {
   base*=in.color;bare*=in.albedo;
  }
  color=base;albedo=bare;
 }
 let mask=veg_coverage(in,alpha);
 if mask==0u {discard;}
 let foliage=in.look<0.5;
 let occlusion=select(1.0,mix(0.55,1.0,clamp(in.height*1.2,0.0,1.0)),foliage);
 var lit=veg_light(color,albedo,in.direction,in.normal,foliage,occlusion,in.reach);
 if thermal_on() {return FillOut(vec4<f32>(lit,1.0),mask);}
 if in.fog_enabled!=0u {lit=aerial_perspective(lit,in.direction,in.altitude);}
 else {lit=cloud_occlusion(lit,in.direction,in.altitude);}
 return FillOut(scenery(lit),mask);
}
// Tree shadow casters (vegetation_renderer.rs): each near tree's first card
// turned to face the sun, showing the baked view nearest the sun's side, so
// its shadow keeps one shape as the camera moves. One entry per cascade:
// the instance index is the tree.
fn veg_shadow(position:vec3<f32>,uv:vec2<f32>,place:vec4<f32>,spin:vec4<f32>,shape:vec4<f32>,tint:vec4<f32>,mean:vec4<f32>,cascade:u32)->ShadowOut {
 let sun=surface.sun.xz;
 let facing=select(vec2<f32>(0.0,1.0),normalize(sun),dot(sun,sun)>0.00000001);
 let right=vec2<f32>(facing.y,-facing.x);
 let offset=right*position.x*spin.z*shape.x*tint.w;
 let p=place.xyz+vec3<f32>(offset.x,position.y*spin.z,offset.y);
 let azimuth=atan2(facing.x,facing.y)-atan2(spin.y,spin.x);
 let view=u32(floor(azimuth/1.570796327+0.5)+8.0)%4u;
 var sprite=mean.w+f32(view);
 var sprite_uv=uv;
 if mean.w<0.0 {
  let packed=u32(-mean.w-1.0);
  sprite=f32(packed/4u);
  let cell=packed%4u;
  sprite_uv=(vec2<f32>(f32(cell%2u),f32(cell/2u))*128.0+vec2<f32>(2.5)+uv*123.0)/256.0;
 }
 return shadow_out(p-scene.local_eye.xyz,sprite_uv,sprite,cascade);
}
@vertex fn veg_shadow_vertex0(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(5) place:vec4<f32>,@location(6) spin:vec4<f32>,@location(7) shape:vec4<f32>,@location(8) tint:vec4<f32>,@location(9) mean:vec4<f32>)->ShadowOut {return veg_shadow(position,uv,place,spin,shape,tint,mean,0u);}
@vertex fn veg_shadow_vertex1(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(5) place:vec4<f32>,@location(6) spin:vec4<f32>,@location(7) shape:vec4<f32>,@location(8) tint:vec4<f32>,@location(9) mean:vec4<f32>)->ShadowOut {return veg_shadow(position,uv,place,spin,shape,tint,mean,1u);}
@vertex fn veg_shadow_vertex2(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(5) place:vec4<f32>,@location(6) spin:vec4<f32>,@location(7) shape:vec4<f32>,@location(8) tint:vec4<f32>,@location(9) mean:vec4<f32>)->ShadowOut {return veg_shadow(position,uv,place,spin,shape,tint,mean,2u);}
// Grass (vegetation_renderer.rs): one quad per tuft, its blades drawn
// here. A patch is 32 ft square: `a` its centre relative to the render
// origin and its cover, `b` the ground rise east and north, the style and
// the blade height, `c` the ground colour and a hash, `d` the tufts in
// this ring, the tufts at full detail and the grass radius. Tufts sit on
// an R2 sequence, so any prefix spreads evenly: farther rings draw fewer,
// wider tufts and the count still covers the patch.
struct GrassOut {
 @builtin(position) clip:vec4<f32>, @location(0) uv:vec2<f32>, @location(1) color:vec3<f32>,
 @location(2) direction:vec3<f32>, @location(3) @interpolate(flat) fade:vec2<f32>,
 @location(4) @interpolate(flat) style:vec2<f32>, @location(5) altitude:f32
}
@vertex fn grass_vertex(@builtin(vertex_index) vertex:u32,@location(5) a:vec4<f32>,@location(6) b:vec4<f32>,@location(7) c:vec4<f32>,@location(8) d:vec4<f32>)->GrassOut {
 var out:GrassOut;
 let tuft=vertex/6u;
 let corner=array<vec2<f32>,6>(vec2<f32>(0.0,0.0),vec2<f32>(1.0,0.0),vec2<f32>(1.0,1.0),vec2<f32>(0.0,0.0),vec2<f32>(1.0,1.0),vec2<f32>(0.0,1.0))[vertex%6u];
 let seed=u32(c.w*65535.0)*977u+tuft*7919u;
 let h0=f32((seed*0x9e3779b1u)>>8u)/16777216.0;
 let h1=f32(((seed^0x5bd1e995u)*0x85ebca6bu)>>8u)/16777216.0;
 let h2=f32(((seed+0x68e31da4u)*0xc2b2ae35u)>>8u)/16777216.0;
 // Cover thins a patch by dropping tufts.
 if h2>a.w {out.clip=vec4<f32>(0.0,0.0,-1.0,1.0);return out;}
 let fi=f32(tuft)+c.w*97.0;
 let jitter=vec2<f32>(fract(0.5+fi*0.7548776662),fract(0.5+fi*0.5698402910))-vec2<f32>(0.5);
 let along=(jitter+(vec2<f32>(h0,h1)-vec2<f32>(0.5))*0.06)*32.0;
 let base=vec3<f32>(a.x+along.x,a.y+dot(along,b.xy),a.z+along.y);
 let dist=length(base-scene.local_eye.xyz);
 let radius=d.z;
 // Fewer tufts farther out are widened to keep the cover; the outer part
 // of the radius shrinks them into the ground.
 let widen=clamp(sqrt(d.y/max(d.x,1.0)),1.0,2.6);
 let shrink=1.0-smoothstep(radius*0.7,radius,dist);
 if shrink<=0.0 {out.clip=vec4<f32>(0.0,0.0,-1.0,1.0);return out;}
 let yaw=h0*6.2831853;
 let side=vec3<f32>(cos(yaw),0.0,sin(yaw));
 let width=2.4*widen*(0.8+0.4*h1);
 let height=b.w*(0.75+0.5*h2)*mix(0.35,1.0,shrink);
 // Lean with a fixed breeze, more at the tip.
 let lean=vec3<f32>(0.35,0.0,0.2)*height*corner.y*corner.y;
 let p=base+side*(corner.x-0.5)*width+vec3<f32>(0.0,corner.y*height,0.0)+lean;
 let rel=p-scene.local_eye.xyz;
 let placed=placed_vertex(p+scene.origin.xyz,rel,vec2<f32>(0.0),-1.0,vec3<f32>(1.0),-1.0);
 out.clip=placed.clip;
 out.uv=corner;
 out.direction=rel;
 out.altitude=placed.altitude;
 out.fade=vec2<f32>(shrink,h1);
 out.style=vec2<f32>(b.z,f32(seed%1024u)/1024.0);
 out.color=c.xyz;
 return out;
}
@fragment fn grass_fragment(in:GrassOut)->FillOut {
 // Five tapered blades per tuft, each its own lean and height.
 let u=in.uv.x;let v=in.uv.y;
 var cover=0.0;var shade=0.0;
 let px=max(fwidth(u),0.0001);
 for(var i=0u;i<5u;i++){
  let h=fract(in.style.y*7.31+f32(i)*0.618034);
  let k=fract(in.style.y*3.17+f32(i)*0.414214);
  let top=0.55+0.45*h;
  if v>top {continue;}
  let x=(f32(i)+0.5)/5.0+(k-0.5)*0.12+(h-0.5)*0.45*v;
  let half=0.055*(1.0-v/top)+0.004;
  let c=clamp((half-abs(u-x))/px+0.5,0.0,1.0);
  if c>cover {cover=c;shade=h;}
 }
 let mask=fill_coverage(in.clip.xy,cover*in.fade.x,in.fade.y);
 if mask==0u {discard;}
 let style=in.style.x;
 var color=in.color;
 // Darker at the root, lighter at the tip, each blade its own shade.
 color*=mix(0.55,1.25,v)*(0.82+0.3*shade);
 if style>0.5 && style<1.5 {color=mix(color,color*vec3<f32>(1.18,1.05,0.7),0.5);}
 if style>2.5 && style<3.5 {
  let l=dot(in.color,vec3<f32>(0.2126,0.7152,0.0722));
  // Dry grass: straw in the ground's own hue, paler toward the tips.
  color=mix(in.color*vec3<f32>(1.05,0.95,0.72),vec3<f32>(0.80,0.66,0.40)*max(l,0.08)*1.6,0.4)*mix(0.7,1.2,v)*(0.88+0.24*shade);
 }
 if style>1.5 && style<2.5 {color*=0.8;}
 if style>4.5 {color=mix(color,vec3<f32>(0.30,0.26,0.18)*mix(0.6,1.1,v),0.5);}
 let normal=normalize(vec3<f32>(0.0,1.0,0.0)-normalize(in.direction)*0.35);
 var lit=veg_light(color,color,in.direction,normal,true,mix(0.6,1.0,v),0.0);
 if thermal_on() {return FillOut(vec4<f32>(lit,1.0),mask);}
 lit=aerial_perspective(lit,in.direction,in.altitude);
 return FillOut(scenery(lit),mask);
}
// City fill far boxes: the surface lighting without the shadow lookups,
// flare light or panel sheen, so a distant box costs little per pixel. A box
// fading in, or fading out over its full detail building, keeps only its
// coverage's samples.
@fragment fn fill_far_fragment(in:VertexOut)->FillOut{
 let mask=fill_coverage(in.clip.xy,in.uv.x,in.uv.y);
 if mask==0u {discard;}
 return FillOut(fill_far_color(in),mask);
}
fn fill_far_color(in:VertexOut)->vec4<f32>{
 let normal=surface_normal(in.direction);
 if thermal_on() {return vec4<f32>(thermal_grey(thermal_object(in,in.albedo,normal)),1.0);}
 var color=in.color;
 if surface.sun.w>0.0 {
  let day=smoothstep(-0.104528,0.0348995,surface.reserved.y);
  let facing=max(dot(normal,surface.sun.xyz),0.0);
  let visibility=sunlight_transmission(surface.origin.y+in.direction.y)*surface.reserved.x;
  let tint=mix(vec3<f32>(1.0),solar_tint(surface.reserved.y),0.65);
  let skyward=clamp(normal.y*0.5+0.5,0.0,1.0);
  let ambient=mix(vec3<f32>(0.12,0.14,0.18),vec3<f32>(0.28,0.32,0.39),skyward);
  let night=vec3<f32>(mix(0.20,0.32,skyward)+0.30*max(dot(normal,surface.moon.xyz),0.0));
  color=color*(mix(night,ambient,day)+0.90*facing*visibility*tint);
 }
 if in.fog_enabled!=0u {color=aerial_perspective(color,in.direction,in.altitude);}
 else {color=cloud_occlusion(color,in.direction,in.altitude);}
 return vec4<f32>(color,1.0);
}
@fragment fn airport_fragment(in:VertexOut)->@location(0) vec4<f32>{
 return airport_color(in);
}
fn airport_color(in:VertexOut)->vec4<f32>{
 if in.layer<0.0 {discard;}
 if in.layer>=0.0 {
  let tex=sample_tile(in.uv,i32(in.layer),0,-1,in.light_row,vec2<f32>(-1.0));
  if tex.a<0.5 {discard;}
 }
 return aircraft_color(in,surface_normal(in.direction));
}
@fragment fn canopy_fragment(in:VertexOut)->@location(0) vec4<f32>{
 let normal=surface_normal(in.direction);
 if in.layer != -5.0 {discard;}
 return vec4<f32>(aircraft_color(in,normal).rgb,0.75);
}
struct SkyOut { @builtin(position) clip:vec4<f32>, @location(0) screen:vec2<f32> }
@vertex fn sky_vertex(@builtin(vertex_index) i:u32)->SkyOut {
 let p=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0));
 var out:SkyOut;out.clip=vec4<f32>(p[i],0.0,1.0);out.screen=p[i];return out;
}
// GouraudHorizon has camera-relative horizontal depth 32767/32,
// upper Y=130..0 and lower Y=5..-clamp(130*alt/15000,10,130).
// Interpolate palette indices, as the native indexed Gouraud program does.
fn horizon_height(ray:vec3<f32>)->f32 {
 var head=scene.forward.xz;
 if length(head)<0.0001 {head=scene.up.xz;}
 return ray.y*1024.0/max(0.0001,dot(ray.xz,normalize(head)));
}
fn horizon_index(ray:vec3<f32>)->f32 {return horizon_index_weighted(ray,scene.sky_mix.w);}
// Smooth weather weights each gradient instead of switching it: a weight
// below one draws the gradient compressed toward its flat 240 middle.
fn horizon_index_weighted(ray:vec3<f32>,upper:f32)->f32 {
 let y=horizon_height(ray);
 let flags=i32(scene.ray.w);
 if smooth_weather() {
  if y>=0.0 {return mix(240.0,mix(240.0,229.0,smoothstep(0.0,130.0,y)),upper);}
  return mix(240.0,mix(240.0,252.0,smoothstep(0.0,max(scene.ray.z,1.0),-y)),scene.sky_mix_b.w);
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
 for(var n=0;n<passes;n++){index=read_tile(vec2<i32>(i32(index),0),i32(scene.deck_a.w));}
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
 let size=vec2<i32>(tile_size());
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
 // Light must reach this water point. Darkening finished water alone leaves a
 // false glint in land shadows, especially when only part of the sun is visible.
 let illumination=geometric_visibility(hit-surface.origin.xyz,vec3<f32>(0.0,1.0,0.0))*sunlight_transmission(hit.y);
 return mix(color,shade(index,0).rgb,0.85*coverage*visible*transmission*illumination);
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
  let cover=select(1.0,scene.sky_mix.x,smooth_weather());
  sky_color=mix(sky_color,sky_tex,smoothstep(0.15,0.45,reflected.y)*cover);
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
// Burning flares light the water and leave a rippled reflection of
// themselves, docs/spec/countermeasures.md.
fn water_flares(color:vec3<f32>,ray:vec3<f32>,hit:vec3<f32>)->vec3<f32> {
 if surface.sun.w<=0.0 || surface.reserved.w<1.0 {return color;}
 let relative=hit-scene.eye.xyz;
 let up=vec3<f32>(0.0,1.0,0.0);
 var glint=0.0;
 let mirror=reflect(ray,up);
 for(var i=0;i<i32(surface.reserved.w);i++) {
  let to=surface.flares[i].source.xyz-relative;
  let d2=dot(to,to);
  glint+=surface.flares[i].source.w*pow(max(dot(mirror,to*inverseSqrt(max(d2,0.000001))),0.0),80.0)/(d2+FLARE_SOFTENING);
 }
 // Night shade remaps leave the resolved water nearly black; a flare shows
 // a fitted dark sea color instead.
 return color+vec3<f32>(0.06,0.09,0.11)*flare_light(relative,up)+FLARE_COLOR*glint*0.15;
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
 let emission=solar_tint(scene.sun.y)*strength;
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
 let tint=solar_tint(scene.sun.y);
 let emission=color*tint*strength;
 return color+(vec3<f32>(1.0)-color)*(vec3<f32>(1.0)-exp(-emission));
}
// One smooth-weather deck texel, interpolated across the haze rows.
fn deck_tile(uv:vec2<f32>,layer:f32,distance:f32,passes:i32,core:i32)->vec4<f32>{
 let row=1.0+clamp(haze(distance)*scene.forward.w,0.0,scene.forward.w-1.0);
 let a=weather_tile(uv,i32(layer),i32(floor(row)),passes,core,vec2<f32>(-1.0));
 let b=weather_tile(uv,i32(layer),i32(ceil(row)),passes,core,vec2<f32>(-1.0));
 return mix(a,b,fract(row));
}
// Crossfade two cutout texels by coverage, so a cut texel lends no colour.
fn crossfade(a:vec4<f32>,b:vec4<f32>,t:f32)->vec4<f32>{
 let alpha=mix(a.a,b.a,t);
 if alpha<=0.0 {return vec4<f32>(a.rgb,0.0);}
 return vec4<f32>((a.rgb*a.a*(1.0-t)+b.rgb*b.a*t)/alpha,alpha);
}
// How much of the high sky deck covers a direction in smooth weather: the
// moon sets behind its clouds rather than drawing over them.
fn sky_deck_cover(ray:vec3<f32>)->f32 {
 let deck=scene.deck_a;
 if !smooth_weather() || deck.z<0.0 || scene.sky_mix.x<=0.0 || ray.y<=0.000001 || scene.eye.y>=deck.x || scene.ocean.y>0.0 {return 0.0;}
 let distance=(deck.x-scene.eye.y)/ray.y;
 let hit=scene.eye.xyz+ray*distance;
 let uv=fract(vec2<f32>(hit.x,-hit.z)/deck.y);
 var tex=weather_tile(uv,i32(deck.z),0,-1,-1,vec2<f32>(-1.0));
 if scene.sky_mix.y>=0.0 && scene.sky_mix.z>0.0 {tex=crossfade(tex,weather_tile(uv,i32(scene.sky_mix.y),0,-1,-1,vec2<f32>(-1.0)),scene.sky_mix.z);}
 let fade=smoothstep(264000.0,2000000.0,length(hit.xz-scene.eye.xz));
 return tex.a*scene.sky_mix.x*(1.0-fade);
}
// Night sky (night_sky.rs): a procedural starfield fixed to the celestial
// sphere, turning with the mission clock about the theater's pole. Each cell
// of a cube over the sphere may hold one star, placed and coloured by a fixed
// hash; a star lights one block of 1/540-of-height pixels (a cross for the
// brightest), so it stays a crisp retro point at any resolution.
fn star_hash(p:vec3<u32>)->vec4<f32> {
 // PCG3D (Jarzynski and Olano 2020), plus a fourth lane from the mix.
 var v=p*1664525u+1013904223u;
 v.x+=v.y*v.z;v.y+=v.z*v.x;v.z+=v.x*v.y;
 v=v^(v>>vec3<u32>(16u));
 v.x+=v.y*v.z;v.y+=v.z*v.x;v.z+=v.x*v.y;
 let w=(v.x^(v.y>>5u)^(v.z<<3u))*747796405u;
 return vec4<f32>(vec4<u32>(v,w^(w>>16u))>>vec4<u32>(8u))/16777216.0;
}
const STAR_CELLS:f32=96.0;
fn starfield(color:vec3<f32>,ray:vec3<f32>,pixel:vec2<f32>,screen:vec2<f32>)->vec3<f32> {
 // Derivatives first, in uniform control flow: pixels per NDC unit.
 let per_ndc=vec2<f32>(1.0/dpdx(screen.x),1.0/dpdy(screen.y));
 if !smooth_weather() || scene.stars.w<=0.0 || ray.y<=0.0 || thermal_on() {return color;}
 // World (x east, y up, z north) into the turning celestial frame.
 let pole=vec3<f32>(0.0,scene.stars.x,scene.stars.y);
 let meridian=vec3<f32>(0.0,scene.stars.y,-scene.stars.x);
 let c=vec3<f32>(ray.x,dot(ray,meridian),dot(ray,pole));
 let s=sin(scene.stars.z);let k=cos(scene.stars.z);
 let sky=vec3<f32>(c.x*k+c.y*s,c.y*k-c.x*s,c.z);
 // Cube face and an angle-even grid on it.
 let m=abs(sky);
 var face=0u;var uv=sky.yz/m.x;var major=sky.x;
 if m.y>=m.x && m.y>=m.z {face=2u;uv=sky.xz/m.y;major=sky.y;}
 else if m.z>=m.x && m.z>=m.y {face=4u;uv=sky.xy/m.z;major=sky.z;}
 if major<0.0 {face+=1u;}
 let even=atan(uv)*(4.0/3.14159265)*0.5+0.5;
 let cell=min(floor(even*STAR_CELLS),vec2<f32>(STAR_CELLS-1.0));
 let h=star_hash(vec3<u32>(vec2<u32>(cell),face*977u+31u));
 // Fewer stars where the even grid's cells are small, more in the band.
 let galactic=dot(normalize(vec3<f32>(-0.87,-0.20,0.45)),sky);
 let chance=0.20+0.55*exp(-galactic*galactic/0.025);
 if h.x>=chance {return color;}
 let h2=star_hash(vec3<u32>(vec2<u32>(cell)+vec2<u32>(4099u,7919u),face*613u+5u));
 let at=(cell+vec2<f32>(0.18)+0.64*h2.xy)/STAR_CELLS;
 let face_uv=tan((at-0.5)*2.0*(3.14159265/4.0));
 var local=vec3<f32>(sign(major),face_uv.x,face_uv.y);
 if face>=4u {local=vec3<f32>(face_uv.x,face_uv.y,sign(major));}
 else if face>=2u {local=vec3<f32>(face_uv.x,sign(major),face_uv.y);}
 let d=normalize(local);
 // Back to the world, then onto the screen.
 let w0=vec3<f32>(d.x*k-d.y*s,d.x*s+d.y*k,d.z);
 let star=vec3<f32>(w0.x,0.0,0.0)+meridian*w0.y+pole*w0.z;
 let depth=dot(star,scene.forward.xyz);
 if depth<=0.0 || star.y<=0.0 {return color;}
 let ndc=vec2<f32>(dot(star,scene.right.xyz)/depth*1.7320508*scene.up.w/scene.eye.w,dot(star,scene.up.xyz)/depth*1.7320508*scene.up.w);
 let star_pixel=pixel+(ndc-screen)*per_ndc;
 let block=max(1.0,round(scene.viewport.y/540.0));
 let offset=floor(star_pixel/block)-floor(pixel/block);
 // Brightness: most stars faint, a few bright; five retro levels.
 let magnitude=pow(h2.z,9.0);
 let level=floor((0.18+0.82*magnitude)*5.0+0.5)/5.0;
 var lit=0.0;
 if all(offset==vec2<f32>(0.0)) {lit=level;}
 else if magnitude>0.6 && abs(offset.x)+abs(offset.y)==1.0 {lit=level*0.35;}
 if lit<=0.0 {return color;}
 // Tint by a few stellar colours.
 var tint=vec3<f32>(1.0);
 if h2.w<0.18 {tint=vec3<f32>(0.75,0.85,1.0);}
 else if h2.w<0.32 {tint=vec3<f32>(1.0,0.93,0.78);}
 else if h2.w<0.37 {tint=vec3<f32>(1.0,0.72,0.55);}
 // Hidden by a bright sky, low near the horizon, washed out near the moon.
 let sky_light=dot(color,vec3<f32>(0.2126,0.7152,0.0722));
 var seen=scene.stars.w*(1.0-smoothstep(0.15,0.4,sky_light))*smoothstep(0.0,0.09,ray.y);
 if scene.moon.w>0.0 {
  let apart=acos(clamp(dot(ray,scene.moon.xyz),-1.0,1.0));
  seen*=1.0-0.85*scene.moon.w*exp(-apart*apart/0.03);
 }
 return color+tint*lit*seen;
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
 color=starfield(color,ray,in.clip.xy,in.screen);
 let decks=array<vec4<f32>,2>(scene.deck_a,scene.deck_b);
 let deck_fades=array<vec4<f32>,2>(scene.sky_mix,scene.sky_mix_b);
 var nearest=1e30;
 for(var i=0;i<2;i++){
  let deck=decks[i];
  if deck.z<0.0 || (smooth_weather() && deck_fades[i].x<=0.0) || abs(ray.y)<0.000001 || (i==1 && scene.eye.y<=deck.x) { continue; }
  let distance=(deck.x-scene.eye.y)/ray.y;
  var head=scene.forward.xz;
  if length(head)<0.0001 {head=scene.up.xz;}
  let scanline_distance=distance*dot(ray.xz,normalize(head));
  if distance<=0.0 || distance>=nearest || scanline_distance>=2000000.0 { continue; }
  let hit=scene.eye.xyz+ray*distance;
  let uv=fract(vec2<f32>(hit.x,-hit.z)/deck.y);
  let fade=deck_fades[i];
  var tex:vec4<f32>;
  if scene.ocean[i+1]>0.0 {
   tex=ocean_surface(hit,deck,distance,passes,core);
   if fade.y>=0.0 && fade.z>0.0 {
    tex=crossfade(tex,ocean_surface(hit,vec4<f32>(deck.xy,fade.y,deck.w),distance,passes,core),fade.z);
   }
   tex=vec4<f32>(aerial_perspective(tex.rgb,ray*distance,deck.x),tex.a);
   tex=vec4<f32>(water_flares(water_shadow(water_sun(tex.rgb,ray,hit,distance),hit),ray,hit),tex.a);
  } else {
  tex=weather_tile(uv,i32(deck.z),fog_row(distance),passes,core,vec2<f32>(-1.0));
  if smooth_weather() {
   tex=deck_tile(uv,deck.z,distance,passes,core);
   if fade.y>=0.0 && fade.z>0.0 {tex=crossfade(tex,deck_tile(uv,fade.y,distance,passes,core),fade.z);}
  }
  }
  if smooth_weather() {tex.a*=fade.x;}
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
 if smooth_weather() {
  if scene.sky_mix_b.w>0.0 && horizon_height(ray)<=5.0 {color=mix(color,horizon_color(horizon_index_weighted(ray,1.0),0,-1),scene.sky_mix_b.w);}
 } else if (i32(scene.ray.w)&2)!=0 && horizon_height(ray)<=5.0 {color=horizon_color(horizon_index(ray),0,-1);}
 // Smooth weather: as much of the open water as neither deck covers.
 var open=select(0.0,1.0,scene.deck_a.z<0.0 && scene.deck_b.z<0.0);
 if smooth_weather() {open=(1.0-select(scene.sky_mix.x,0.0,scene.deck_a.z<0.0))*(1.0-select(scene.sky_mix_b.x,0.0,scene.deck_b.z<0.0));}
 if open>0.0 {
  let plain=color;
  color=overcast_water(color,ray);
  if scene.cloud_reflection.z>0.0 && ray.y< -0.000001 && scene.eye.y>0.0 {
   let distance=-scene.eye.y/ray.y;
   if distance<2000000.0 {
    let hit=scene.eye.xyz+ray*distance;
    let reflected=water_flares(water_shadow(water_sun(color,ray,hit,distance),hit),ray,hit);
    color=mix(reflected,color,smoothstep(1800000.0,2000000.0,distance));
   }
  }
  color=mix(plain,color,open);
 }
 var cloud_distance=2000000.0;
 if ray.y< -0.000001 {cloud_distance=min(cloud_distance,max(scene.eye.y,0.0)/(-ray.y));}
 color=cloud_occlusion(color,ray*cloud_distance,scene.eye.y+ray.y*cloud_distance);
 if thermal_on() {return scenery(thermal_grey(thermal_sky(ray)));}
 return scenery(color);
}
struct VaporOut { @builtin(position) clip:vec4<f32>, @location(0) color:vec4<f32>, @location(1) distance:f32 }
@vertex fn vapor_vertex(@location(0) position:vec3<f32>,@location(1) color:vec4<f32>)->VaporOut {
 let p=position-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 let f=1.7320508*scene.up.w;
 var out:VaporOut;
 out.clip=vec4<f32>(dot(p,scene.right.xyz)*f/scene.eye.w,dot(p,scene.up.xyz)*f,world_depth_clip(z),z);
 out.color=color;out.distance=length(p);return out;
}
@fragment fn vapor_fragment(in:VaporOut)->@location(0) vec4<f32>{
 if thermal_on() {return vec4<f32>(thermal_grey(0.34),in.color.a*0.2*(1.0-haze(in.distance)));}
 // Vapor sits in the same atmosphere as everything else, so haze thins it too.
 // Resolve the fitted sky-entry material in this camera palette.
 return vec4<f32>(shade(254u,0).rgb*in.color.rgb,in.color.a*(1.0-haze(in.distance)));
}

@vertex fn celestial_vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@location(3) color:vec3<f32>,@location(4) index:f32)->VertexOut {
 let z=dot(position,scene.forward.xyz);let f=1.7320508*scene.up.w;
 var out:VertexOut;
 out.clip=vec4<f32>(dot(position,scene.right.xyz)*f/scene.eye.w,dot(position,scene.up.xyz)*f,0.0,z);
 // Textured quads (the moon) carry their opacity in the first colour slot.
 out.light_row=-1;out.fog_enabled=0u;out.uv=uv;out.layer=layer;out.color=select(shade(u32(index),0).rgb,color,layer>=0.0);out.distance=position.y;out.own_color=0.;out.altitude=position.y;out.direction=position;return out;
}
@fragment fn celestial_fragment(in:VertexOut)->@location(0) vec4<f32>{
 if celestial_occluded(normalize(in.direction)) || thermal_on() {discard;}
 if in.layer>=0.0 {
  let tex=weather_tile(in.uv,i32(in.layer),0,-1,-1,vec2<f32>(-1.0));
  let alpha=tex.a*in.color.x*(1.0-sky_deck_cover(normalize(in.direction)));
  if alpha<0.01 {discard;}
  return vec4<f32>(tex.rgb,alpha);
 }
 return vec4<f32>(in.color,1.0);
}

// Shared non-emissive lighting for clouds and every smoke/contrail family.
fn cloud_lighting(color:vec3<f32>,direction:vec3<f32>,altitude:f32)->vec3<f32> {
 let ray=direction/max(length(direction),0.00001);
 let lit=cloud_solar_glow(color,ray,1.0-clamp(haze(length(direction)),0.0,1.0));
 return aerial_perspective(lit,direction,altitude);
}

@fragment fn cloud_fragment(in:VertexOut)->@location(0) vec4<f32>{
 let tex=weather_tile(in.uv,i32(in.layer),0,-1,-1,ray_rows(in.distance,in.altitude));
 if tex.a<0.5 {discard;}
 var flares=vec3<f32>(0.0);
 if thermal_on() {return scenery(thermal_grey(thermal_air(0.27,in.distance*0.5)));}
 if flares_burning() {
  let bare=weather_tile(in.uv,i32(in.layer),base_row(),-1,-1,vec2<f32>(-1.0)).rgb;
  flares=bare*flare_light(in.direction,vec3<f32>(0.0));
 }
 return scenery(cloud_lighting(tex.rgb+flares,in.direction,in.altitude));
}

// glow: light from nearby burning flares, evaluated per puff on the CPU.
struct SmokeOut { @builtin(position) clip:vec4<f32>, @location(0) uv:vec2<f32>, @location(1) opacity:f32, @location(2) distance:f32, @location(3) direction:vec3<f32>, @location(4) altitude:f32, @location(5) glow:vec3<f32>, @location(6) tint:vec3<f32>, @location(7) local:vec2<f32> }
@vertex fn smoke_vertex(@builtin(vertex_index) vertex:u32,@location(0) center:vec3<f32>,@location(1) radius:f32,@location(2) cell:f32,@location(3) opacity:f32,@location(4) glow:vec3<f32>,@location(5) tint:vec3<f32>)->SmokeOut {
 let corners=array<vec2<f32>,6>(vec2(0.0,0.0),vec2(1.0,0.0),vec2(1.0,1.0),vec2(0.0,0.0),vec2(1.0,1.0),vec2(0.0,1.0));
 let corner=corners[vertex];
 let offset=vec2<f32>(corner.x*2.0-1.0,1.0-corner.y*2.0)*radius;
 let uv=vec2<f32>((cell+0.5+corner.x*42.0)/256.0,(0.5+corner.y*42.0)/43.0);
 let position=center+scene.right.xyz*offset.x+scene.up.xyz*offset.y;
 let p=position-scene.eye.xyz;let z=dot(p,scene.forward.xyz);
 let f=1.7320508*scene.up.w;
 var out:SmokeOut;
 out.clip=vec4<f32>(dot(p,scene.right.xyz)*f/scene.eye.w,dot(p,scene.up.xyz)*f,world_depth_clip(z),z);
 out.uv=uv;out.opacity=opacity;out.distance=length(p);out.direction=p;out.altitude=position.y;out.glow=glow;out.tint=tint;out.local=vec2<f32>(corner.x*2.0-1.0,1.0-corner.y*2.0);return out;
}
@fragment fn smoke_fragment(in:SmokeOut)->@location(0) vec4<f32> {
 // A blast's dust multiplies the white puff by its tint (tan when dry),
 // and dims after dark to 35 percent, so it does not glow over the night
 // ground (blast_climate.rs).
 let art=sample_tile(in.uv,0,0,-1,-1,ray_rows(in.distance,in.altitude));
 var dim=1.0;
 if any(in.tint<vec3<f32>(1.0)) {dim=mix(0.35,1.0,smoothstep(-0.10,0.10,scene.sun.y));}
 // A dust puff is drawn large, so its square's edge is feathered away.
 var edge=1.0;
 if any(in.tint<vec3<f32>(1.0)) {edge=1.0-smoothstep(0.6,1.0,length(in.local));}
 let color=vec4<f32>(art.rgb*in.tint*dim,art.a*edge);
 let alpha=color.a*in.opacity*(1.0-clamp(haze(in.distance),0.0,1.0));
 // A transparent texel adds nothing under the premultiplied blend.
 if alpha==0.0 {discard;}
 // Glow lights the puff's own undarkened art, as flare light does elsewhere.
 var bare=color.rgb;
 if any(in.glow>vec3<f32>(0.0)) {bare=sample_tile(in.uv,0,base_row(),-1,-1,vec2<f32>(-1.0)).rgb*in.tint;}
 if thermal_on() {
  // A thermal camera sees through most smoke and dust; a puff lit by fire
  // is warm.
  let thin=alpha*0.35;
  let v=thermal_air(0.30+min(thermal_luma(in.glow),0.2),in.distance);
  return vec4<f32>(thermal_grey(v)*thin,thin);
 }
 let lit=cloud_lighting(color.rgb,in.direction,in.altitude)+bare*in.glow*(1.0-clamp(haze(in.distance),0.0,1.0));
 return vec4<f32>(lit*alpha,alpha);
}

// Original explosion, fire and crater sheets (effect_renderer.rs). Mode 0
// faces the camera centered on its point, mode 1 faces the camera with its
// base on its point, mode 2 lies flat on the ground. Explosions and fire glow in their unlit colors; craters take
// the scene's light. Flat marks stay at their actual ground depth. A small
// reversed-depth raster bias resolves coplanar terrain ties without
// pulling a mark feet toward the eye and over vehicles in a magnified view.
struct EffectOut { @builtin(position) clip:vec4<f32>, @location(0) uv:vec2<f32>, @location(1) opacity:f32, @location(2) distance:f32, @location(3) direction:vec3<f32>, @location(4) altitude:f32, @location(5) @interpolate(flat) layer:i32, @location(6) emissive:f32, @location(7) tint:vec3<f32> }
@vertex fn effect_vertex(@builtin(vertex_index) vertex:u32,@location(0) offset:vec3<f32>,@location(1) extent:vec2<f32>,@location(2) mode:f32,@location(3) cell:vec4<f32>,@location(4) layer:f32,@location(5) opacity:f32,@location(6) emissive:f32,@location(7) tint:vec3<f32>,@location(8) ground_heights:vec4<f32>)->EffectOut {
 let corners=array<vec2<f32>,6>(vec2(0.0,0.0),vec2(1.0,0.0),vec2(1.0,1.0),vec2(0.0,0.0),vec2(1.0,1.0),vec2(0.0,1.0));
 let corner=corners[vertex];
 let across=(corner.x*2.0-1.0)*extent.x;
 // `offset` is the sprite's point relative to the eye.
 var p=offset;
 if mode<0.5 {
  p=offset+scene.right.xyz*across+scene.up.xyz*(1.0-corner.y*2.0)*extent.y;
 } else if mode<1.5 {
  p=offset+scene.right.xyz*across+scene.up.xyz*(1.0-corner.y)*extent.y;
 } else {
  let ground_corner=array<u32,6>(0u,1u,2u,0u,2u,3u);
  p=offset+vec3<f32>(across,ground_heights[ground_corner[vertex]],(corner.y*2.0-1.0)*extent.y);
 }
 let size=vec2<f32>(textureDimensions(tiles).xy);
 let uv=(cell.xy+vec2<f32>(0.5)+corner*(cell.zw-vec2<f32>(1.0)))/size;
 let z=dot(p,scene.forward.xyz);
 let f=1.7320508*scene.up.w;
 var out:EffectOut;
 out.clip=vec4<f32>(dot(p,scene.right.xyz)*f/scene.eye.w,dot(p,scene.up.xyz)*f,world_depth_clip(z),z);
 out.uv=uv;out.opacity=opacity;out.distance=length(p);out.direction=p;out.altitude=p.y+scene.eye.y;out.layer=i32(layer);out.emissive=emissive;out.tint=tint;return out;
}
const GLOW_HAZE_SHARE:f32=0.1;
// How much of a glow reaches the eye through cloud, haze and air: a light
// at night is seen through the haze about ten times as far as the ground
// (fitted, nr-fx3), so the haze counts at a tenth of the distance, and only
// its ramp counts (the haze already over everything near at night does not
// dim a light). A cloud between hides it as it hides a flare.
fn glow_clear(p:vec3<f32>,altitude:f32)->vec3<f32> {
 let cloud=clamp(cloud_occlusion(vec3<f32>(1.0),p,altitude)-cloud_occlusion(vec3<f32>(0.0),p,altitude),vec3<f32>(0.0),vec3<f32>(1.0));
 let d=length(p)*GLOW_HAZE_SHARE;
 let ramp=clamp((haze(d)-scene.fog.z)/max(scene.fog.w-scene.fog.z,0.001),0.0,1.0);
 return cloud*(1.0-ramp)*(1.0-air_opacity(d,altitude));
}
@fragment fn effect_fragment(in:EffectOut)->@location(0) vec4<f32> {
 let clear=1.0-clamp(haze(in.distance),0.0,1.0);
 if thermal_on() {return thermal_effect(in);}
 if in.emissive>0.5 {
  let color=sample_tile(in.uv,in.layer,base_row(),-1,-1,vec2<f32>(-1.0));
  if device_night()<=0.0 {
   let alpha=color.a*in.opacity*clear;
   return vec4<f32>(color.rgb*alpha,alpha);
  }
  // Preserve day exactly. At night the entire art emits through the same
  // atmosphere as its additive halo, rather than retaining only a point.
  let alpha=color.a*in.opacity;
  // A transparent texel adds nothing under the premultiplied blend.
  if alpha==0.0 {discard;}
  let visibility=mix(vec3<f32>(clear),glow_clear(in.direction,in.altitude),device_night());
  return vec4<f32>(color.rgb*alpha*visibility,alpha*max(visibility.r,max(visibility.g,visibility.b)));
 }
 let color=sample_tile(in.uv,in.layer,0,-1,-1,ray_rows(in.distance,in.altitude));
 let alpha=color.a*in.opacity*clear;
 if alpha==0.0 {discard;}
 // A shockwave's dust is its puff art times the climate's colour.
 return vec4<f32>(aerial_perspective(color.rgb*in.tint,in.direction,in.altitude)*alpha,alpha);
}

// Source scenery lines are one-pixel strokes with the shared live palette/haze.
@fragment fn scenery_line_fragment(in:VertexOut)->@location(0) vec4<f32> {
 if thermal_on() {return scenery(thermal_grey(thermal_object(in,in.albedo,surface_normal(in.direction))));}
 return scenery(aerial_perspective(in.color,in.direction,in.altitude));
}

// ---- Thermal sensor picture (AC-130 sensor console, nr-fltv) -------------
// With scene.quality.z set (`Camera::thermal`) every world fragment writes
// its apparent temperature as a grey level: 0 coldest, 1 hottest, as the
// display shows it in white-hot (the console's CRT pass inverts it for
// black-hot). The levels are agent decisions (nr-fltv, 2026-10-13), fitted
// on offscreen renders for a readable picture by day and by night: the
// sky coldest; the ground and buildings cool with the texture's own
// brightness as contrast, warmer where the sun falls by day; water cooler
// than the land by day and warmer by night; anything running (aircraft,
// vehicles, men) warm; engines, fires, explosions, muzzle flashes, tracers
// and flares hot; a fresh crater warm, cooling over three minutes.
fn thermal_on()->bool {return scene.quality.z>0.5;}
// 0 at night, 1 by day, from the sun's height.
fn thermal_day()->f32 {return smoothstep(-0.08,0.20,scene.sun.y);}
fn thermal_luma(c:vec3<f32>)->f32 {return dot(c,vec3<f32>(0.2126,0.7152,0.0722));}
// An art colour's brightness against the weather's daylight palette, so a
// night weather's darker art keeps its contrast: 0.18 is the palette's
// mean; the ground's art sits near 0.13 on this scale.
fn thermal_albedo(c:vec3<f32>)->f32 {
 return pow(clamp(0.18*thermal_luma(c)/max(scene.quality.w,0.0001),0.0,1.0),0.45);
}
// A level as the colour the target stores: the display grey, decoded so
// the sRGB target shows it as written.
fn thermal_grey(v:f32)->vec3<f32> {return linear(vec3<f32>(clamp(v,0.0,1.0)));}
// The air in between: a thermal camera sees far through haze, but the
// picture flattens toward the air's own level with distance.
fn thermal_air(v:f32,distance:f32)->f32 {
 let air=mix(0.30,0.36,thermal_day());
 return mix(v,air,0.6*(1.0-exp(-distance/80000.0)));
}
// Patches a few hundred feet across (fields, soil, damp ground), -1..1:
// value noise on the world position, so the ground reads as ground even
// where its texture is plain.
fn thermal_patches(direction:vec3<f32>)->f32 {
 let p=(scene.eye.xz+direction.xz)/900.0;
 let i=floor(p);let f=fract(p);let u=f*f*(3.0-2.0*f);
 let h=vec4<f32>(water_hash(i),water_hash(i+vec2<f32>(1.0,0.0)),water_hash(i+vec2<f32>(0.0,1.0)),water_hash(i+vec2<f32>(1.0,1.0)));
 return mix(mix(h.x,h.y,u.x),mix(h.z,h.w,u.x),u.y)*2.0-1.0;
}
// Open ground: the daylight texture's brightness as contrast (stretched:
// a thermal camera's gain is set to the scene), patches, the sunny side
// warmer by day, flatter at night.
fn thermal_ground(albedo:vec3<f32>,normal:vec3<f32>,direction:vec3<f32>,distance:f32)->f32 {
 let day=thermal_day();
 let l=thermal_albedo(albedo);
 let sun=max(dot(normal,normalize(scene.sun.xyz)),0.0)*day;
 let patches=thermal_patches(direction)*mix(0.02,0.035,day);
 let v=mix(0.33+0.8*(l-0.13),0.28+1.3*(l-0.13)+0.05*(sun-0.5),day)+patches;
 return thermal_air(clamp(v,0.08,0.62),distance);
}
// Shapes: the warm class (see VertexOut.heat) and the cool one, with the
// engines' own heat, flames and glass.
fn thermal_object(in:VertexOut,albedo:vec3<f32>,normal:vec3<f32>)->f32 {
 if in.layer == -6.0 || in.layer == -7.0 {return 1.0;}
 if in.layer<=-3.0 && in.layer>=-4.0 {return thermal_air(0.86+0.14*clamp(-in.layer-3.0,0.0,1.0),in.distance);}
 if in.layer == -5.0 {return thermal_air(0.40,in.distance);}
 let day=thermal_day();
 let l=thermal_albedo(albedo);
 let sun=max(dot(normal,normalize(scene.sun.xyz)),0.0)*day;
 var v=mix(0.33+0.5*(l-0.13),0.32+0.7*(l-0.13)+0.08*(sun-0.5),day);
 if in.heat>0.5 {v=0.68+0.3*(l-0.13)+0.05*sun;}
 return thermal_air(v,in.distance);
}
// The sky cold, warmer toward the horizon; water below it.
fn thermal_sky(ray:vec3<f32>)->f32 {
 let day=thermal_day();
 if ray.y<0.0 {
  let distance=max(scene.eye.y,1.0)/max(-ray.y,0.00001);
  return thermal_air(mix(0.30,0.17,day),distance);
 }
 return 0.06+0.26*pow(1.0-clamp(ray.y,0.0,1.0),8.0);
}
// Light that adds to the picture (tracers, glows, flashes, flares): its
// brightness, as heat.
fn thermal_light(c:vec4<f32>,scale:f32)->vec4<f32> {
 if !thermal_on() {return c;}
 return vec4<f32>(thermal_grey(min(thermal_luma(c.rgb)*scale,1.0)),c.a);
}
// Explosions and fires hot; a crater warm while fresh (the instance's
// emissive slot carries minus its freshness, effect_renderer.rs); dust
// thin and cool.
fn thermal_effect(in:EffectOut)->vec4<f32> {
 if in.emissive>0.5 {
  let color=sample_tile(in.uv,in.layer,base_row(),-1,-1,vec2<f32>(-1.0));
  let alpha=color.a*in.opacity;
  let v=thermal_air(0.80+0.20*thermal_luma(color.rgb),in.distance);
  return vec4<f32>(thermal_grey(v)*alpha,alpha);
 }
 let color=sample_tile(in.uv,in.layer,base_row(),-1,-1,vec2<f32>(-1.0));
 if in.emissive<0.0 {
  let alpha=color.a*in.opacity;
  let v=thermal_air(0.30+0.10*thermal_luma(color.rgb)+0.18*min(-in.emissive,1.0),in.distance);
  return vec4<f32>(thermal_grey(v)*alpha,alpha);
 }
 let alpha=color.a*in.opacity*0.4;
 return vec4<f32>(thermal_grey(thermal_air(0.34,in.distance))*alpha,alpha);
}
