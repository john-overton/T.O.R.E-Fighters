// Opinionated chaff and flare presentation, docs/spec/countermeasures.md.
// Appended to surface_lighting.wgsl, terrain.wgsl and spotting.wgsl.

fn device_hash(value:u32)->u32 {
 var v=value*747796405u+2891336453u;
 v=((v>>((v>>28u)+4u))^v)*277803737u;
 return (v>>22u)^v;
}
fn device_random(seed:u32,stream:u32)->f32 {
 return f32(device_hash(seed^device_hash(stream))>>8u)/16777216.0;
}
// World feet covered by one pixel of the world image at view depth z.
fn device_pixel(z:f32)->f32 {
 return 2.0*z/(1.7320508*scene.up.w*max(scene.viewport.y,1.0));
}
fn device_clip(p:vec3<f32>,z:f32)->vec4<f32> {
 let f=1.7320508*scene.up.w;
 return vec4<f32>(dot(p,scene.right.xyz)*f/scene.eye.w,dot(p,scene.up.xyz)*f,world_depth_clip(z),z);
}
// Haze, air and cloud dimming of emitted light, shared with gun tracers.
fn device_visibility(p:vec3<f32>,altitude:f32)->vec3<f32> {
 let distance=length(p);
 let cloud=clamp(cloud_occlusion(vec3<f32>(1.0),p,altitude)-cloud_occlusion(vec3<f32>(0.0),p,altitude),vec3<f32>(0.0),vec3<f32>(1.0));
 return cloud*(1.0-clamp(haze(distance),0.0,1.0))*(1.0-air_opacity(distance,altitude));
}
// 0 in full daylight, 1 at night, from the sun's elevation.
fn device_night()->f32 {
 return 1.0-smoothstep(-0.104528,0.0348995,surface.reserved.y);
}

// ---- Burning flare: a white core inside an orange and yellow flame --------
struct FlareOut {
 @builtin(position) clip:vec4<f32>,
 @location(0) local:vec2<f32>,
 // Core radius, flame radius, flame stretch behind, age in seconds.
 @location(1) @interpolate(flat) sizes:vec4<f32>,
 @location(2) @interpolate(flat) axis:vec2<f32>,
 @location(3) @interpolate(flat) light:vec4<f32>,
 @location(4) @interpolate(flat) seed:u32,
}
@vertex fn flare_vertex(@builtin(vertex_index) vertex:u32,@location(0) center:vec3<f32>,@location(1) intensity:f32,@location(2) motion:vec3<f32>,@location(3) age:f32,@location(4) seed:u32)->FlareOut {
 var out:FlareOut;
 let p=center-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 if z<=max(scene.view.x,1.0) || intensity<=0.0 {out.clip=vec4<f32>(0.0,0.0,-1.0,1.0);return out;}
 let pixel=device_pixel(z);
 let core=max(0.75,1.5*pixel);
 let flame=max(3.0,2.5*pixel);
 // The flame trails the flare's motion across the view.
 let across=vec2<f32>(dot(motion,scene.right.xyz),dot(motion,scene.up.xyz));
 let speed=length(across);
 let axis=select(vec2<f32>(0.0,-1.0),across/max(speed,0.0001),speed>1.0);
 let stretch=clamp(speed/60.0,0.0,2.5);
 let extent=max(core*1.5,flame*(1.3+stretch));
 let corners=array<vec2<f32>,6>(vec2(-1.0,-1.0),vec2(1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,1.0));
 let local=corners[vertex]*extent;
 let q=p+scene.right.xyz*local.x+scene.up.xyz*local.y;
 out.clip=device_clip(q,dot(q,scene.forward.xyz));
 out.local=local;out.sizes=vec4<f32>(core,flame,stretch,age);out.axis=axis;
 out.light=vec4<f32>(device_visibility(p,center.y),intensity);out.seed=seed;
 return out;
}
@fragment fn flare_fragment(in:FlareOut)->@location(0) vec4<f32> {
 let r=length(in.local);
 let along=dot(in.local,in.axis);
 let side=dot(in.local,vec2<f32>(-in.axis.y,in.axis.x));
 // Compress the trailing half so the flame reaches back along the motion.
 let back=select(along,along/(1.0+in.sizes.z),along<0.0);
 let shape=length(vec2<f32>(back,side))/in.sizes.y;
 let t=in.sizes.w;
 let angle=atan2(side,back);
 let lick=0.5+0.5*sin(angle*5.0+t*31.0+device_random(in.seed,1u)*6.28)*sin(angle*3.0-t*23.0);
 let flame=exp(-shape*shape*(1.3+1.1*lick));
 let heat=exp(-shape*shape*4.0);
 let flame_color=mix(vec3<f32>(1.0,0.32,0.04),vec3<f32>(1.0,0.78,0.22),heat);
 let core=1.0-smoothstep(0.55*in.sizes.x,in.sizes.x,r);
 let radiance=(core*vec3<f32>(1.0,0.96,0.86)*4.0+flame*flame_color*2.4)*in.light.w*in.light.xyz;
 return vec4<f32>(radiance,0.0);
}

// ---- Glare: drawn over the finished image, visible while the core is -----
struct GlareOut {
 @builtin(position) clip:vec4<f32>,
 // Offset from the flare in output pixels.
 @location(0) local:vec2<f32>,
 // Halo radius in pixels, strength, streak weight, core coverage.
 @location(1) @interpolate(flat) glare:vec4<f32>,
}
fn glare_setup(vertex:u32,center:vec3<f32>,intensity:f32)->GlareOut {
 var out:GlareOut;
 out.clip=vec4<f32>(0.0,0.0,-1.0,1.0);
 let p=center-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 if z<=max(scene.view.x,1.0) || intensity<=0.0 {return out;}
 let clip=device_clip(p,z);
 let ndc=clip.xy/z;
 let output=scene.viewport.xy/max(scene.viewport.w,0.0001);
 let night=device_night();
 let distance=length(p);
 // A point source: its halo weakens with distance but never vanishes.
 let strength=intensity*mix(0.35,1.0,night)/(1.0+pow(distance/3000.0,2.0));
 let core_pixels=max(0.75/device_pixel(z)*output.y/max(scene.viewport.y,1.0),1.5);
 let radius=mix(max(3.0*core_pixels,0.008*output.y),0.06*output.y,night)*(0.6+0.4*sqrt(min(strength,1.0)));
 let extent=radius*mix(1.0,1.8,night);
 let corners=array<vec2<f32>,6>(vec2(-1.0,-1.0),vec2(1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,1.0));
 let local=corners[vertex]*extent;
 out.clip=vec4<f32>(ndc+local*2.0/output*vec2<f32>(1.0,-1.0),0.0,1.0);
 out.local=local;
 let visible=device_visibility(p,center.y);
 out.glare=vec4<f32>(radius,strength*(visible.x+visible.y+visible.z)/3.0,night,0.0);
 return out;
}
// Depth samples across the core: its world-image texel for sample i of 25
// and its own reversed depth. The core is seen where nothing nearer is.
struct CoreProbe { at:vec2<f32>, spacing:f32, depth:f32 }
fn core_probe(center:vec3<f32>)->CoreProbe {
 let p=center-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 let ndc=device_clip(p,z).xy/z;
 var probe:CoreProbe;
 probe.at=(vec2<f32>(ndc.x,-ndc.y)*0.5+vec2<f32>(0.5))*scene.viewport.xy;
 probe.spacing=max(0.75/device_pixel(z),1.0)*0.5;
 probe.depth=world_depth_clip(z)/z;
 return probe;
}
fn core_texel(probe:CoreProbe,i:i32,size:vec2<u32>)->vec2<i32> {
 let offset=vec2<f32>(f32(i%5-2),f32(i/5-2))*probe.spacing;
 return vec2<i32>(floor(probe.at+offset));
}
fn core_inside(texel:vec2<i32>,size:vec2<u32>)->bool {
 return all(texel>=vec2<i32>(0)) && all(texel<vec2<i32>(size));
}
@vertex fn glare_vertex(@builtin(vertex_index) vertex:u32,@location(0) center:vec3<f32>,@location(1) intensity:f32)->GlareOut {
 var out=glare_setup(vertex,center,intensity);
 let probe=core_probe(center);let size=textureDimensions(spot_depth);
 var seen=0.0;
 for(var i=0;i<25;i++) {
  let texel=core_texel(probe,i,size);
  if core_inside(texel,size) && probe.depth>=textureLoad(spot_depth,texel,0) {seen+=1.0;}
 }
 out.glare.w=seen/25.0;
 return out;
}
@vertex fn glare_vertex_ms(@builtin(vertex_index) vertex:u32,@location(0) center:vec3<f32>,@location(1) intensity:f32)->GlareOut {
 var out=glare_setup(vertex,center,intensity);
 let probe=core_probe(center);let size=textureDimensions(spot_depth_ms);
 var seen=0.0;
 for(var i=0;i<25;i++) {
  let texel=core_texel(probe,i,size);
  if core_inside(texel,size) && probe.depth>=textureLoad(spot_depth_ms,texel,0) {seen+=1.0;}
 }
 out.glare.w=seen/25.0;
 return out;
}
@fragment fn glare_fragment(in:GlareOut)->@location(0) vec4<f32> {
 let r=length(in.local)/in.glare.x;
 let halo=1.1*exp(-r*r*10.0)+0.4*exp(-r*3.2);
 // Four soft streaks at night, like a bright light through a canopy.
 let d=abs(in.local)/in.glare.x;
 let streaks=(exp(-d.y*40.0)*exp(-d.x*2.6)+exp(-d.x*40.0)*exp(-d.y*2.6))*0.35*in.glare.z;
 let fade=1.0-smoothstep(0.85,1.0,max(abs(in.local.x),abs(in.local.y))/(in.glare.x*mix(1.0,1.8,in.glare.z)));
 let radiance=vec3<f32>(1.0,0.86,0.66)*(halo+streaks)*fade*in.glare.y*in.glare.w;
 return vec4<f32>(radiance,0.0);
}

// ---- Chaff: hundreds of tumbling foil strips per cartridge ---------------
struct ChaffOut {
 @builtin(position) clip:vec4<f32>,
 @location(0) @interpolate(flat) color:vec4<f32>,
}
@vertex fn chaff_vertex(@builtin(vertex_index) vertex:u32,@location(0) center:vec3<f32>,@location(1) seconds:f32,@location(2) seed:u32,@location(3) opacity:f32)->ChaffOut {
 var out:ChaffOut;
 out.clip=vec4<f32>(0.0,0.0,-1.0,1.0);
 let strip=vertex/6u;
 let r=vec4<f32>(device_random(seed,strip*8u),device_random(seed,strip*8u+1u),device_random(seed,strip*8u+2u),device_random(seed,strip*8u+3u));
 // Uniform in a ball that blooms quickly, then keeps spreading slowly.
 let z_dir=r.x*2.0-1.0;
 let a=r.y*6.2831853;
 let direction=vec3<f32>(sqrt(1.0-z_dir*z_dir)*cos(a),z_dir*0.7,sqrt(1.0-z_dir*z_dir)*sin(a));
 let bloom=3.0+32.0*(1.0-exp(-seconds/0.5))+1.5*seconds;
 let fall=(2.0+4.0*r.w)-4.0;
 let flutter=0.6*sin(seconds*(2.0+3.0*r.z)+r.w*6.28);
 let position=center+direction*bloom*pow(r.z,0.333)+vec3<f32>(flutter,-fall*seconds,flutter*0.7);
 let p=position-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 if z<=max(scene.view.x,1.0) {return out;}
 let pixel=device_pixel(z);
 let extent=max(0.25,0.5*pixel);
 let coverage=min(1.0,0.5/pixel);
 let corners=array<vec2<f32>,6>(vec2(-1.0,-1.0),vec2(1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,1.0));
 let corner=corners[vertex%6u]*extent;
 let q=p+scene.right.xyz*corner.x+scene.up.xyz*corner.y;
 out.clip=device_clip(q,dot(q,scene.forward.xyz));
 // Each strip spins about its own axis; its face flashes when it mirrors
 // the sun toward the camera.
 let spin_axis=normalize(vec3<f32>(device_random(seed,strip*8u+4u),device_random(seed,strip*8u+5u),device_random(seed,strip*8u+6u))-vec3<f32>(0.5)+vec3<f32>(0.0001));
 let other=select(vec3<f32>(1.0,0.0,0.0),vec3<f32>(0.0,1.0,0.0),abs(spin_axis.x)>0.9);
 let u=normalize(cross(spin_axis,other));
 let v=cross(spin_axis,u);
 let rate=6.2831853*(2.0+6.0*device_random(seed,strip*8u+7u));
 let turn=seconds*rate+r.x*6.2831853;
 let face=u*cos(turn)+v*sin(turn);
 let view=-p/max(length(p),0.001);
 let day=1.0-device_night();
 let sun=surface.sun.xyz;
 let half_vector=normalize(sun+view);
 let sunlit=surface.reserved.x*sunlight_transmission(position.y);
 let glint=pow(abs(dot(face,half_vector)),40.0)*sunlit*day*8.0;
 let ambient=mix(0.04,0.28,day)+0.3*abs(dot(face,sun))*sunlit*day;
 let lit=vec3<f32>(0.62,0.64,0.68)*ambient+flare_light(p,vec3<f32>(0.0))*0.8;
 let color=lit+vec3<f32>(1.0,0.97,0.9)*glint;
 let visibility=device_visibility(p,position.y);
 let alpha=clamp(opacity*coverage*0.85*(1.0-smoothstep(12000.0,20000.0,length(p))),0.0,1.0);
 out.color=vec4<f32>(color*visibility*alpha,alpha);
 return out;
}
@fragment fn chaff_fragment(in:ChaffOut)->@location(0) vec4<f32> {
 return in.color;
}
