// Spotting aid for other aircraft. An opinionated host addition requested by
// John on 2026-09-22; the original game had none. Every constant below is an
// agent choice (fitted by eye on captures), not recovered behaviour.
//
// The rim is drawn in its own single-sample pass after the world image is
// resolved, at output resolution, so it is pixel-sharp at any anti-aliasing
// setting: whole pixels in one solid color, never a blend. It reuses the
// aircraft vertex buffers and draws each other aircraft separately;
// per-instance data carries its presented center, its airframe's largest
// dimension in feet (its "extent") and its first vertex. The world pass's
// depth buffer hides it behind terrain, cloud and nearer aircraft, and inside
// the aircraft's own silhouette.

// An aircraft of at least this many pixels gets an outline: eight copies of
// it shifted by one whole output pixel in each direction.
const OUTLINE_FROM_PIXELS:f32=2.0;
// Below this, a solid square mark instead: two pixels wide under 1.5 pixels,
// three pixels wide up to 3. Tail-on, a jet two miles away is a fuselage dot
// and hairline wings, too thin for an outline of its own.
const MARK_BELOW_PIXELS:f32=3.0;
const MARK_LARGE_FROM_PIXELS:f32=1.5;
// Full strength until the aircraft is 1% of the view height (about 7 pixels
// at 720p), none by 10%.
const RIM_FULL_FRACTION:f32=0.01;
const RIM_ZERO_FRACTION:f32=0.10;
// Eyesight limit: full above 4 arcminutes, none below 1.5 (about 8 and 21
// nautical miles for a 56-foot fighter). Radians.
const RIM_FULL_ANGLE:f32=0.0011636;
const RIM_ZERO_ANGLE:f32=0.00043633;
// Against the sky the rim is always dark: an aircraft is a silhouette against
// the light, and a pale ring in a dusk sky reads as a marker. Against ground
// darker than this linear luminance the rim is light.
const RIM_LIGHT_BELOW:f32=0.3;
// At full strength a dark rim is this fraction of the background brightness
// and a light rim this multiple of it (strength scales the step toward 1).
const RIM_DARK_STEP:f32=0.2;
const RIM_LIGHT_STEP:f32=2.5;
// How much of the background's hue the rim keeps; the rest is grey.
const RIM_HUE:f32=0.3;
// Stand-in for terrain under the aircraft, linear RGB.
const RIM_GROUND:vec3<f32>=vec3<f32>(0.07,0.08,0.06);

// The world pass's depth, single-sample or multisampled to match it.
@group(2) @binding(0) var spot_depth:texture_depth_2d;
@group(2) @binding(1) var spot_depth_ms:texture_depth_multisampled_2d;

struct RimOut {
 @builtin(position) clip:vec4<f32>,
 @location(0) @interpolate(flat) color:vec3<f32>,
 // Normalized depth just behind the aircraft.
 @location(1) @interpolate(flat) depth:f32,
 // 1 for a light rim (max blending), 0 for a dark rim (min blending).
 @location(2) @interpolate(flat) light:f32,
}
fn spot_clip(p:vec3<f32>)->vec4<f32> {
 let z=dot(p,scene.forward.xyz);
 let f=1.7320508*scene.up.w;
 return vec4<f32>(dot(p,scene.right.xyz)*f/scene.eye.w,dot(p,scene.up.xyz)*f,world_depth_clip(z),z);
}
fn spot_depth_at(z:f32)->f32 {
 return world_depth_clip(z)/z;
}
// Behind the near plane: whole triangles of collapsed vertices have no area.
fn spot_collapsed()->vec4<f32> {return vec4<f32>(0.0,0.0,-1.0,1.0);}
// Flames and effects are not part of the airframe.
fn spot_skipped(layer:f32)->bool {
 return layer == -8.0 || layer == -6.0 || layer == -7.0;
}
// Output image size of this view in pixels.
fn spot_output()->vec2<f32> {
 return max(scene.viewport.xy/max(scene.viewport.w,0.01),vec2<f32>(1.0));
}
// How much of the aircraft's own contrast survives the air between it and the
// eye: 1 in clear air, 0 when fully hazed out or behind dense cloud.
fn spot_clarity(p:vec3<f32>,distance:f32,altitude:f32)->f32 {
 var clear=1.0-clamp(haze(distance),0.0,1.0);
 if smooth_weather() {clear*=1.0-air_opacity(distance,altitude);}
 let cloud=cloud_occlusion(vec3<f32>(1.0),p,altitude)-cloud_occlusion(vec3<f32>(0.0),p,altitude);
 return clear*clamp(dot(cloud,vec3<f32>(1.0/3.0)),0.0,1.0);
}
// The rim cannot read the image it draws into, so estimate what lies behind
// the aircraft: the sky gradient above the horizon, hazed ground below it.
fn spot_background(ray:vec3<f32>)->vec3<f32> {
 let sky=horizon_color(horizon_index(ray),0,-1);
 if ray.y>=0.0 || scene.eye.y<=0.0 {return sky;}
 let ground=scene.eye.y/(-ray.y);
 var clear=1.0-clamp(haze(ground),0.0,1.0);
 if smooth_weather() {clear*=1.0-air_opacity(ground,0.0);}
 let horizon=horizon_color(horizon_index(vec3<f32>(ray.x,0.0,ray.z)),0,-1);
 return mix(horizon,RIM_GROUND,clear);
}
// `contact` is the aircraft's presented center and extent in feet; `range.x`
// is its first vertex. Every decision uses the center, so all of an
// aircraft's vertices agree.
@vertex fn rim_vertex(@location(0) position:vec3<f32>,@location(2) layer:f32,@location(10) contact:vec4<f32>,@location(11) range:vec4<f32>,@builtin(vertex_index) vertex:u32,@builtin(instance_index) instance:u32)->RimOut {
 var out:RimOut;
 out.color=vec3<f32>(0.0);out.depth=0.0;out.light=0.0;
 let c=contact.xyz-scene.eye.xyz;
 let z=dot(c,scene.forward.xyz);
 let distance=max(length(c),1.0);
 let fraction=contact.w*1.7320508*scene.up.w/(2.0*max(z,1.0));
 let pixels=fraction*spot_output().y;
 let angle=contact.w/distance;
 let size=1.0-smoothstep(RIM_FULL_FRACTION,RIM_ZERO_FRACTION,fraction);
 let sight=smoothstep(RIM_ZERO_ANGLE,RIM_FULL_ANGLE,angle);
 // The rim outlasts the aircraft's own contrast a little, but not past it.
 let strength=scene.quality.x*size*sight*sqrt(spot_clarity(c,distance,contact.y));
 if z<=max(scene.view.x,1.0)+contact.w || strength<0.02 {
  out.clip=spot_collapsed();return out;
 }
 out.depth=spot_depth_at(z+contact.w);
 let corner=vertex-u32(range.x);
 let copy=instance%8u;
 if pixels<MARK_BELOW_PIXELS && copy==0u && corner<6u {
  // The aircraft's first two triangles become a square of whole pixels
  // around its projected center.
  let center=spot_clip(c);
  let output=spot_output();
  let at=vec2<f32>(center.x/center.w*0.5+0.5,0.5-center.y/center.w*0.5)*output;
  let side=select(2.0,3.0,pixels>=MARK_LARGE_FROM_PIXELS);
  let start=select(round(at)-vec2<f32>(1.0),floor(at)-vec2<f32>(1.0),side>2.5);
  let quad=array<vec2<f32>,6>(vec2<f32>(0.0,0.0),vec2<f32>(1.0,0.0),vec2<f32>(1.0,1.0),vec2<f32>(0.0,0.0),vec2<f32>(1.0,1.0),vec2<f32>(0.0,1.0));
  let pixel=start+quad[corner]*side;
  out.clip=vec4<f32>(pixel.x/output.x*2.0-1.0,1.0-pixel.y/output.y*2.0,out.depth,1.0);
 } else if pixels>=OUTLINE_FROM_PIXELS && !spot_skipped(layer) {
  let directions=array<vec2<f32>,8>(vec2<f32>(1.0,0.0),vec2<f32>(-1.0,0.0),vec2<f32>(0.0,1.0),vec2<f32>(0.0,-1.0),
      vec2<f32>(1.0,1.0),vec2<f32>(-1.0,1.0),vec2<f32>(1.0,-1.0),vec2<f32>(-1.0,-1.0));
  let clip=spot_clip(position-scene.eye.xyz);
  let shift=directions[copy]*2.0/spot_output();
  out.clip=vec4<f32>(clip.xy+shift*clip.w,out.depth*clip.w,clip.w);
 } else {
  out.clip=spot_collapsed();return out;
 }
 let background=spot_background(c/distance);
 let luminance=dot(background,vec3<f32>(0.2126,0.7152,0.0722));
 let light=c.y<0.0 && luminance<RIM_LIGHT_BELOW;
 let grey=mix(vec3<f32>(luminance),background,RIM_HUE);
 // Strength is a step in brightness, drawn as a solid color.
 let dark=grey*mix(1.0,RIM_DARK_STEP,strength);
 let bright=min(grey*mix(1.0,RIM_LIGHT_STEP,strength)+vec3<f32>(0.02*strength),vec3<f32>(1.0));
 out.color=select(dark,bright,light);
 out.light=select(0.0,1.0,light);
 return out;
}
// World depth texel for an output pixel; the world image may be at another
// render scale.
fn spot_world_pixel(clip:vec4<f32>,size:vec2<u32>)->vec2<i32> {
 let at=vec2<i32>(floor(clip.xy*scene.viewport.w));
 return clamp(at,vec2<i32>(0),vec2<i32>(size)-vec2<i32>(1));
}
fn rim_color(in:RimOut,scene_depth:f32,light:bool)->vec4<f32> {
 if (in.light>=0.5)!=light || in.depth<=scene_depth {discard;}
 return vec4<f32>(in.color,1.0);
}
// Min and max blending make overlapping copies idempotent, and a background
// that is already darker (or lighter) than the rim is left untouched.
@fragment fn rim_dark_fragment(in:RimOut)->@location(0) vec4<f32> {
 return rim_color(in,textureLoad(spot_depth,spot_world_pixel(in.clip,textureDimensions(spot_depth)),0),false);
}
@fragment fn rim_light_fragment(in:RimOut)->@location(0) vec4<f32> {
 return rim_color(in,textureLoad(spot_depth,spot_world_pixel(in.clip,textureDimensions(spot_depth)),0),true);
}
@fragment fn rim_dark_fragment_ms(in:RimOut)->@location(0) vec4<f32> {
 return rim_color(in,textureLoad(spot_depth_ms,spot_world_pixel(in.clip,textureDimensions(spot_depth_ms)),0),false);
}
@fragment fn rim_light_fragment_ms(in:RimOut)->@location(0) vec4<f32> {
 return rim_color(in,textureLoad(spot_depth_ms,spot_world_pixel(in.clip,textureDimensions(spot_depth_ms)),0),true);
}
