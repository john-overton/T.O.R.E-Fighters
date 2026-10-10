// Opinionated AC-130 muzzle flashes, docs/spec/ac130-linked-guns.md#muzzle-flash.
// Appended after countermeasures.wgsl, whose device_* helpers it uses.
//
// A flash is a flat, hard-edged sprite in the view plane: a white-hot core at
// the barrel tip, a jagged plume out along the barrel and a burst of petals
// that shows when the barrel points toward or away from the eye. Three flat
// color bands, no soft halo and no glare, in the manner of the original's
// explosion sheets. It covers the background like a sprite, rather than
// adding light to it, so its colors hold against a bright sky.
struct FlashOut {
 @builtin(position) clip:vec4<f32>,
 // Feet in the view plane: along the barrel as drawn, then across it.
 @location(0) local:vec2<f32>,
 // Core radius, plume reach, plume half width and petal radius, in feet.
 @location(1) @interpolate(flat) shape:vec4<f32>,
 // Haze, air and cloud dimming, then intensity.
 @location(2) @interpolate(flat) light:vec4<f32>,
 @location(3) @interpolate(flat) seed:u32,
 @location(4) @interpolate(flat) kind:u32,
}
@vertex fn flash_vertex(@builtin(vertex_index) vertex:u32,@location(0) muzzle:vec3<f32>,@location(1) intensity:f32,@location(2) direction:vec3<f32>,@location(3) reach:f32,@location(4) seed:u32,@location(5) kind:u32)->FlashOut {
 var out:FlashOut;
 out.clip=vec4<f32>(0.0,0.0,-1.0,1.0);
 let p=muzzle-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 if z<=max(scene.view.x,1.0) || intensity<=0.0 {return out;}
 let pixel=device_pixel(z);
 // The barrel across the view: its foreshortening shortens the plume and
 // opens the petals.
 let across=vec2<f32>(dot(direction,scene.right.xyz),dot(direction,scene.up.xyz));
 let f=length(across);
 let axis=select(vec2<f32>(1.0,0.0),across/max(f,0.0001),f>0.001);
 var core_k=0.16;
 var width_k=0.26;
 if kind==2u {core_k=0.22;width_k=0.38;}
 // Never smaller than a couple of pixels, so a firing gun twinkles far off.
 let core=max(reach*core_k,1.25*pixel);
 let plume=max(reach*f,core*1.3);
 let width=max(reach*width_k,1.25*pixel);
 let petals=max(reach*(0.3+0.4*(1.0-f)),core*1.4);
 let back=max(core,petals)*1.1;
 let side=max(max(width*1.7,petals),core)*1.1;
 let corners=array<vec2<f32>,6>(vec2(-1.0,-1.0),vec2(1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,1.0));
 let c=corners[vertex];
 let local=vec2<f32>(select(-back,plume+core,c.x>0.0),c.y*side);
 let along=scene.right.xyz*axis.x+scene.up.xyz*axis.y;
 let lateral=scene.right.xyz*(-axis.y)+scene.up.xyz*axis.x;
 let q=p+along*local.x+lateral*local.y;
 out.clip=device_clip(q,dot(q,scene.forward.xyz));
 out.local=local;
 out.shape=vec4<f32>(core,plume,width,petals);
 out.light=vec4<f32>(device_visibility(p,muzzle.y),intensity);
 out.seed=seed;out.kind=kind;
 return out;
}
@fragment fn flash_fragment(in:FlashOut)->@location(0) vec4<f32> {
 let a=in.local.x;
 let b=in.local.y;
 let core=in.shape.x;
 let plume=in.shape.y;
 let width=in.shape.z;
 let petals=in.shape.w;
 let r=length(in.local);
 // The plume: a teardrop along the barrel, its edge cut in jagged steps.
 let t=clamp(a/plume,0.0,1.0);
 let jag=0.65+0.5*device_random(in.seed,u32(floor(t*6.0))+3u);
 let half=width*jag*1.7*sqrt(t)*pow(1.0-t,0.6);
 let plume_d=select(4.0,abs(b)/max(half,0.0001),a>0.0 && a<plume);
 // Petals: a few sharp spikes around the tip, leaning forward.
 let angle=atan2(b,a);
 let count=f32(4u+in.seed%3u);
 let spoke=abs(cos(0.5*count*angle+device_random(in.seed,1u)*6.2831853));
 let lean=0.55+0.45*max(cos(angle),0.0);
 let petal_d=r/max(petals*lean*(0.25+0.75*pow(spoke,8.0)),0.0001);
 let core_d=r/core;
 let d=min(min(core_d,petal_d),plume_d);
 // Hard band edges, antialiased over one pixel. As the flash dies the
 // white-hot heart shrinks first, then the yellow, and the orange rim
 // draws in last.
 let i=in.light.w;
 let w=max(fwidth(d),0.0001);
 let outer=clamp((0.55+0.45*sqrt(i)-d)/w+0.5,0.0,1.0);
 let mid=clamp((0.7*sqrt(i)-d)/w+0.5,0.0,1.0);
 let hot=clamp((0.38*i-d)/w+0.5,0.0,1.0);
 let orange=vec3<f32>(1.0,0.4,0.07)*1.2;
 let yellow=vec3<f32>(1.0,0.76,0.24)*1.7;
 let white=vec3<f32>(1.0,0.97,0.88)*3.0;
 let color=orange*outer+(yellow-orange)*mid+(white-yellow)*hot;
 // The fireball is thick: it covers what is behind it (premultiplied), so
 // its orange rim reads against a bright sky as well as at night.
 let cover=outer*0.85*i*(in.light.x+in.light.y+in.light.z)/3.0;
 return vec4<f32>(color*(0.35+0.65*i)*in.light.xyz,cover);
}
