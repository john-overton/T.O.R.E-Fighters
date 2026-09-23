// Shared surface response and geometry shadows, docs/spec/surface-lighting.md.
struct ShadowProjection { right:vec4<f32>, up:vec4<f32>, depth:vec4<f32>, scale:vec4<f32> }
struct SurfaceLight { maps:array<ShadowProjection,3>, origin:vec4<f32>, sun:vec4<f32>, moon:vec4<f32>, reserved:vec4<f32> }
@group(1) @binding(0) var<uniform> surface:SurfaceLight;
@group(1) @binding(1) var shadows:texture_depth_2d_array;
@group(1) @binding(2) var shadow_sampler:sampler_comparison;
fn shadow_project(relative:vec3<f32>,cascade:i32)->vec3<f32> {
 let p=vec4<f32>(relative,1.0);let m=surface.maps[cascade];
 return vec3<f32>(dot(m.right,p),dot(m.up,p),dot(m.depth,p));
}
struct ShadowOut { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32>, @location(1) @interpolate(flat) layer:f32 }
@vertex fn shadow_vertex(@location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) layer:f32,@builtin(instance_index) cascade:u32)->ShadowOut {
 var out:ShadowOut;
 out.position=vec4<f32>(shadow_project(position-surface.origin.xyz,i32(cascade)),1.0);
 out.uv=uv;out.layer=layer;return out;
}
fn shadow_cutout(uv:vec2<f32>,layer:i32) {
 let size=vec2<i32>(tile_size());
 let at=clamp(vec2<i32>(uv*vec2<f32>(size)),vec2<i32>(0),size-vec2<i32>(1));
 if read_tile(at,layer)==255u {discard;}
}
@fragment fn shadow_fragment(in:ShadowOut) {
 // Glass, flame sheets and emissive effects must not cast solid silhouettes.
 if in.layer == -5.0 || in.layer == -6.0 || in.layer == -7.0 || in.layer == -8.0 {discard;}
 if in.layer == -2.0 {shadow_cutout(in.uv,0);}
 // Ordinary aircraft textures blend transparent texels with their base polygon.
}
@fragment fn terrain_shadow_fragment(in:ShadowOut) {
 if in.layer>=0.0 {shadow_cutout(in.uv,i32(in.layer));}
}
// Manual bilinear depth evaluation: each texel gets its own receiver depth.
// Hardware comparison filtering uses one depth for all four neighbors, which
// causes sloped tiles to intermittently classify themselves as blockers.
fn shadow_tap(uv:vec2<f32>,depth:f32,gradient:vec2<f32>,cascade:i32)->vec2<f32> {
 let m=surface.maps[cascade];let pixel=uv*2048.0-vec2<f32>(0.5);
 let base=vec2<i32>(floor(pixel));let f=fract(pixel);
 let tolerance=max(0.125,m.scale.x*0.002);
 var result=vec2<f32>(0.0);
 for(var y=0;y<2;y++) {for(var x=0;x<2;x++) {
  let at=base+vec2<i32>(x,y);
  if any(at<vec2<i32>(0)) || any(at>=vec2<i32>(2048)) {continue;}
  let sample_uv=(vec2<f32>(at)+vec2<f32>(0.5))/2048.0;
  let stored=textureLoad(shadows,at,cascade,0);
  let gap=(depth+dot(gradient,sample_uv-uv)-stored)*2.0*m.scale.z;
  let weight=select(1.0-f.x,f.x,x==1)*select(1.0-f.y,f.y,y==1);
  let blocked=smoothstep(tolerance,2.0*tolerance,gap)*select(1.0,0.0,stored>=1.0);
  result+=vec2<f32>(blocked,max(gap,0.0)*blocked)*weight;
 }}
 return result;
}
fn shadow_sample(relative:vec3<f32>,normal:vec3<f32>,cascade:i32)->f32 {
 let m=surface.maps[cascade];
 let facing=clamp(dot(normal,surface.sun.xyz),0.0,1.0);
 let p=shadow_project(relative+normal*m.scale.x*0.20*(1.0-facing),cascade);
 let uv=p.xy*vec2<f32>(0.5,-0.5)+vec2<f32>(0.5);
 if any(uv<vec2<f32>(0.001)) || any(uv>vec2<f32>(0.999)) || p.z<=0.0 || p.z>=1.0 {return 1.0;}
 // Correct each tap to the receiver plane. Comparing every sample against
 // the center's depth makes grazing terrain appear to block its own sunlight.
 let light_dot=dot(normal,surface.sun.xyz);
 let denominator=select(-1.0,1.0,light_dot>=0.0)*max(abs(light_dot),0.0001);
 let gradient=vec2<f32>(dot(normal,m.right.xyz),-dot(normal,m.up.xyz))*m.scale.y*m.scale.y/(m.scale.z*denominator);
 var separation=0.0;var blockers=0.0;
 for(var i=0;i<17;i++) {
  var offset=vec2<f32>(0.0);
  if i>0 {
   let ring=(i-1)/4;let axis=(i-1)%4;
   let radius=pow(4.0,f32(ring));
   let directions=array<vec2<f32>,4>(vec2<f32>(1.0,0.0),vec2<f32>(-1.0,0.0),vec2<f32>(0.0,1.0),vec2<f32>(0.0,-1.0));
   offset=directions[axis]*radius/2048.0;
  }
  let tap=shadow_tap(uv+offset,p.z+dot(gradient,offset),gradient,cascade);
  separation+=tap.y;blockers+=tap.x;
 }
 // The confidence fade below makes this empty-search fast path continuous.
 if blockers<=0.0 {return 1.0;}
 let base_radius=mix(2.0,0.65,smoothstep(500.0,12000.0,length(relative)));
 let physical=separation/max(blockers,0.000001)*surface.reserved.z/m.scale.x;
 let radius=clamp(mix(base_radius,max(base_radius,physical),smoothstep(0.0,1.0,blockers)),base_radius,64.0);
 var visibility=0.0;
 for(var i=0;i<32;i++) {
  let angle=f32(i)*2.39996323;
  let offset=vec2<f32>(cos(angle),sin(angle))*sqrt((f32(i)+0.5)/32.0)*radius/2048.0;
  let at=uv+offset;
  if any(at<vec2<f32>(0.0)) || any(at>vec2<f32>(1.0)) {visibility+=1.0;continue;}
  visibility+=1.0-shadow_tap(at,p.z+dot(gradient,offset),gradient,cascade).x;
 }
 return mix(1.0,visibility/32.0,smoothstep(0.0,1.0,blockers));
}
fn geometric_visibility(relative:vec3<f32>,normal:vec3<f32>)->f32 {
 if surface.origin.w<=0.0 {return 1.0;}
 for(var cascade=0;cascade<3;cascade++) {
  let p=shadow_project(relative,cascade);
  let edge=max(abs(p.x),abs(p.y));
  if edge<1.0 && p.z>0.0 && p.z<1.0 {
   let near=shadow_sample(relative,normal,cascade);
   if edge<=0.85 {return near;}
   var far=1.0;
   if cascade<2 {far=shadow_sample(relative,normal,cascade+1);}
   return mix(near,far,smoothstep(0.85,1.0,edge));
  }
 }
 return 1.0;
}
// Shared with sky/cloud scattering rather than a separate sunset color.
fn solar_tint(elevation:f32)->vec3<f32> {
 let low=1.0-smoothstep(0.104528,0.422618,elevation);
 return mix(vec3<f32>(1.0,0.88,0.65),vec3<f32>(1.0,0.32,0.075),low);
}
fn sunlight_transmission(altitude:f32)->f32 {
 var path=0.0;
 for(var i=0;i<i32(scene.ray.x);i++) {
  let band=scene.bands[i];
  let dense=smoothstep(0.9,1.0,band.ramp.w/256.0)*(1.0-smoothstep(8000.0,16000.0,band.ramp.z*256.0));
  path+=max(band.info.y-max(altitude,band.info.x),0.0)*dense/max(surface.sun.y,0.02);
 }
 return exp(-path/600.0);
}
// Derivatives recover triangle geometry, independent of original face flags.
fn surface_normal(relative:vec3<f32>)->vec3<f32> {
 let n=cross(dpdx(relative),dpdy(relative));
 let unit=n*inverseSqrt(max(dot(n,n),0.00000001));
 return select(unit,-unit,dot(unit,relative)>0.0);
}
fn surface_color(color:vec3<f32>,relative:vec3<f32>,normal:vec3<f32>,panels:bool,receiver_normal:vec3<f32>)->vec3<f32> {
 if surface.sun.w<=0.0 {return color;}
 let day=smoothstep(-0.104528,0.0348995,surface.reserved.y);
 let facing=max(dot(normal,surface.sun.xyz),0.0);
 let visibility=geometric_visibility(relative,receiver_normal)*sunlight_transmission(surface.origin.y+relative.y)*surface.reserved.x;
 let tint=mix(vec3<f32>(1.0),solar_tint(surface.reserved.y),0.65);
 let skyward=clamp(normal.y*0.5+0.5,0.0,1.0);
 var ambient=mix(vec3<f32>(0.12,0.14,0.18),vec3<f32>(0.28,0.32,0.39),skyward);
 if !panels {
  // Land has less low-sun sky fill than reflective water or painted panels.
  // Preserve exposed ridge brightness; obtain contrast by darkening shade.
  let low=1.0-smoothstep(0.104528,0.422618,surface.reserved.y);
  let shaded=1.0-smoothstep(0.0,0.35,facing*visibility);
  ambient*=1.0-0.55*low*shaded;
 }
 let night=vec3<f32>(mix(0.20,0.32,skyward)+0.30*max(dot(normal,surface.moon.xyz),0.0));
 let illumination=mix(night,ambient,day)+0.90*facing*visibility*tint;
 var lit=color*illumination;
 if panels {
  let view=normalize(-relative);
  let grazing=pow(1.0-clamp(dot(normal,view),0.0,1.0),3.0);
  let reflected=reflect(-view,normal);
  let sky=mix(vec3<f32>(0.18,0.20,0.24),vec3<f32>(0.48,0.58,0.75),clamp(reflected.y*0.5+0.5,0.0,1.0));
  let half_vector=surface.sun.xyz+view;
  let half_normal=half_vector*inverseSqrt(max(dot(half_vector,half_vector),0.000001));
  let highlight=0.22*pow(max(dot(normal,half_normal),0.0),32.0)*facing*visibility;
  lit+=day*(sky*0.08*(0.25+0.75*grazing)+tint*highlight);
 }
 // Imported fog is already resolved; suppress lighting contrast in that fog.
 return mix(color,lit,1.0-clamp(haze(length(relative)),0.0,1.0));
}
fn water_shadow(color:vec3<f32>,hit:vec3<f32>)->vec3<f32> {
 if surface.sun.w<=0.0 {return color;}
 let visible=geometric_visibility(hit-surface.origin.xyz,vec3<f32>(0.0,1.0,0.0));
 let amount=surface.reserved.x*sunlight_transmission(hit.y)*(1.0-clamp(haze(length(hit-surface.origin.xyz)),0.0,1.0));
 return color*mix(1.0,0.30+0.70*visible,amount);
}
