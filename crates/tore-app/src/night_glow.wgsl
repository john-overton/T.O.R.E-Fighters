// nr-fx3: the night glow of explosions and fires (night_glow.rs,
// docs/spec/explosions.md, "Night glow"). Appended after
// airfield_lights.wgsl; shares countermeasures.wgsl's device_* helpers.
// Additive points with a minimum size on screen and a floor on their
// brightness, so a fire or a blast stays visible to the limit of the night
// scene; close in, a soft halo around the art. Night and dusk only.
// Mirrors of the constants in night_glow.rs.
const GLOW_MIN_PX:f32=4.0;
const GLOW_MAX_PX:f32=9.0;
const GLOW_BASE:f32=0.8;
const GLOW_LEVEL:vec2<f32>=vec2<f32>(1.0,3.0);
const GLOW_SPRITE_PX:vec2<f32>=vec2<f32>(3.0,16.0);
const GLOW_HALO:f32=1.6;
const GLOW_NEAR:f32=0.5;
const GLOW_BLOOM:f32=0.6;
struct GlowOut {
 @builtin(position) clip:vec4<f32>,
 @location(0) local:vec2<f32>,
 @location(1) @interpolate(flat) radiance:vec4<f32>,
}
@vertex fn glow_vertex(@builtin(vertex_index) vertex:u32,@location(0) offset:vec3<f32>,@location(1) width:f32,@location(2) strength:f32,@location(3) heat:f32)->GlowOut {
 var out:GlowOut;
 out.clip=vec4<f32>(0.0,0.0,-1.0,1.0);
 let night=device_night();
 let p=offset;
 let z=dot(p,scene.forward.xyz);
 if night<=0.01 || strength<=0.0 || z<=max(scene.view.x,1.0) {return out;}
 let pixel=device_pixel(z);
 // How many pixels the art spans: the point takes over as it shrinks.
 let across=width/pixel;
 let far=1.0-smoothstep(GLOW_SPRITE_PX.x,GLOW_SPRITE_PX.y,across);
 let s=sqrt(strength);
 let smallest=clamp(GLOW_MIN_PX*(0.8+0.45*s),GLOW_MIN_PX,GLOW_MAX_PX)*pixel;
 let radius=max(width*GLOW_HALO,smallest);
 let level=night*mix(GLOW_NEAR,1.0,far)*clamp(GLOW_BASE*s,GLOW_LEVEL.x,GLOW_LEVEL.y);
 // A flash burns white-yellow, a fire orange.
 let rgb=mix(vec3<f32>(1.0,0.5,0.16),vec3<f32>(1.0,0.86,0.62),clamp(heat,0.0,1.0));
 let corners=array<vec2<f32>,6>(vec2(-1.0,-1.0),vec2(1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,1.0));
 let local=corners[vertex]*radius*2.0;
 let q=p+scene.right.xyz*local.x+scene.up.xyz*local.y;
 out.clip=device_clip(q,dot(q,scene.forward.xyz));
 out.local=corners[vertex]*2.0;
 out.radiance=vec4<f32>(rgb*glow_clear(p,scene.eye.y+p.y)*level,far);
 return out;
}
@fragment fn glow_fragment(in:GlowOut)->@location(0) vec4<f32> {
 let r2=dot(in.local,in.local);
 // Windowed to nothing at the quad's inscribed circle, so no edge shows.
 let window=clamp(1.0-r2*0.25,0.0,1.0);
 // Resolved art supplies its own core. Only distant, unresolved lights
 // need the bright point, otherwise it bleaches the fire's animation.
 // A warm bloom the size of the effect around the flames (nr-nightfx).
 let core=(exp(-r2*2.5)*2.0*in.radiance.a+exp(-r2*1.0)*GLOW_BLOOM)*window*window;
 return thermal_light(vec4<f32>(in.radiance.rgb*core,0.0),0.8);
}
