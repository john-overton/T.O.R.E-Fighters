// Original implementation for this engine. Fixed world-cell identity, no
// instance data or alpha test. Appended to terrain.wgsl for shared lighting.
struct BladeParams { area:vec4<f32>, grid:vec4<f32>, wind:vec4<f32>, reserved:vec4<f32> }
@group(2) @binding(0) var<uniform> blades:BladeParams;
@group(2) @binding(1) var blade_habitat:texture_2d_array<u32>;
@group(2) @binding(2) var blade_ground:texture_2d<f32>;
fn blade_hash(cell:vec2<i32>,salt:u32)->u32 {
 var h=bitcast<u32>(cell.x)*0x9e3779b9u ^ bitcast<u32>(cell.y)*0x85ebca6bu ^ salt;
 h=(h^(h>>16u))*0x7feb352du;
 h=(h^(h>>15u))*0x846ca68bu;
 return h^(h>>16u);
}
fn blade_random(cell:vec2<i32>,salt:u32)->f32 {return f32(blade_hash(cell,salt)>>16u)/65536.0;}
fn blade_class(world:vec2<f32>)->u32 {
 let sub=vec2<i32>(floor((world-blades.area.xy)/128.0));
 if any(sub<vec2<i32>(0)) || any(sub>=vec2<i32>(blades.area.zw)*4) {return 0u;}
 let block=sub/4;
 let page=block.y/256*i32(blades.grid.z)+block.x/256;
 let across=i32(textureDimensions(blade_habitat).x)/256;
 let slot=page%(across*across);
 let pixel=vec2<i32>(slot%across,slot/across)*256+block%256;
 let packed=textureLoad(blade_habitat,pixel,page/(across*across),0);
 return (packed[sub.y%4]>>u32((sub.x%4)*2))&3u;
}
struct BladeOut {
 @builtin(position) clip:vec4<f32>, @location(0) color:vec3<f32>,
 @location(1) direction:vec3<f32>, @location(2) normal:vec3<f32>,
 @location(3) altitude:f32, @location(4) height:f32
}
@vertex fn blade_vertex(@builtin(vertex_index) vertex:u32,@builtin(instance_index) instance:u32)->BladeOut {
 var out:BladeOut;
 out.clip=vec4<f32>(0.0,0.0,-1.0,1.0);
 let side=u32(blades.grid.x);
 let level=instance/(side*side);
 let slot=instance%(side*side);
 let step=1u<<level;
 let spacing=blades.wind.w*f32(step);
 let center=vec2<i32>(floor(scene.eye.xz/spacing));
 let cell=(center+vec2<i32>(i32(slot%side),i32(slot/side))-vec2<i32>(i32(side/2u)))*i32(step);
 // Every second child in both axes belongs to the next coarser grid. The
 // coarse owner renders it all the way to the centre, without a duplicate.
 let parent_mask=(step*2u)-1u;
 if level<3u && (bitcast<u32>(cell.x)&parent_mask)==0u && (bitcast<u32>(cell.y)&parent_mask)==0u {return out;}
 let jitter=vec2<f32>(blade_random(cell,17u),blade_random(cell,31u));
 let world=(vec2<f32>(cell)+vec2<f32>(0.075)+jitter*0.85)*blades.wind.w;
 let horizontal=world-scene.eye.xz;
 // One-cell guard leaves every entering/leaving snapped row outside the fade.
 let radius=min(blades.grid.y,f32(side/2u-1u)*spacing);
 let distance=length(horizontal);
 if distance>=radius {return out;}
 let ring_coverage=1.0-smoothstep(radius*0.70,radius,distance);

 let habitat=blade_class(world);
 if habitat==0u {return out;}
 let at=vec2<i32>(floor(world/8192.0));
 let size=vec2<i32>(textureDimensions(blade_ground));
 if any(at<vec2<i32>(0)) || any(at+vec2<i32>(1)>=size) {return out;}
 let a=textureLoad(blade_ground,at,0);
 let b=textureLoad(blade_ground,at+vec2<i32>(1,0),0).x;
 let c=textureLoad(blade_ground,at+vec2<i32>(0,1),0).x;
 let d=textureLoad(blade_ground,at+vec2<i32>(1,1),0).x;
 let uv=fract(world/8192.0);
 var y=a.x+(b-a.x)*uv.x+(c-a.x)*uv.y;
 var grad=vec2<f32>(b-a.x,c-a.x)/8192.0;
 if uv.x+uv.y>1.0 {y=d+(c-d)*(1.0-uv.x)+(b-d)*(1.0-uv.y);grad=vec2<f32>(d-c,d-b)/8192.0;}
 if length(grad)>0.9 {return out;}
 let base=vec3<f32>(world.x,y+blades.grid.w,world.y)-scene.origin.xyz;
 let rel=base-scene.local_eye.xyz;
 let eye_distance=length(rel);
 if eye_distance>=blades.grid.y {return out;}
 let coverage=min(ring_coverage,1.0-smoothstep(blades.grid.y*0.70,blades.grid.y,eye_distance));
 let rank=blade_random(cell,47u);
 let grow=smoothstep(rank*0.8,rank*0.8+0.2,coverage)*coverage;
 if grow<=0.0 {return out;}
 // Mild size compensation keeps distant opaque geometry readable. Roots stay
 // on the same terrain plane. No camera-dependent position or population hash.
 let far=smoothstep(40.0,blades.grid.y,eye_distance);
 // GPU-only frustum rejection precedes colour and wind work.
 let forward=dot(rel,scene.forward.xyz);
 // Include the compensated tropical tip, slope and wind lean in the bound.
 let bound=8.0;
 if forward < -bound || abs(dot(rel,scene.right.xyz))>max(forward,0.0)*scene.eye.w/(1.7320508*scene.up.w)+bound
    || abs(dot(rel,scene.up.xyz))>max(forward,0.0)/(1.7320508*scene.up.w)+bound {return out;}
 var density=select(1.0,0.9,habitat==2u);
 if habitat==3u {density=0.35;}
 if scene.canopy.y>0.0 {
  let ca=vec2<i32>(floor((world-scene.canopy_area.xy)/512.0));
  if all(ca>=vec2<i32>(0)) && all(ca<vec2<i32>(scene.canopy_area.zw)) {
   let page=i32(scene.canopy.x)+ca.y/256*i32(scene.canopy.y)+ca.x/128;
   let cover=f32(read_tile(vec2<i32>((ca.x%128)*2,ca.y%256),page))/63.75;
   density*=mix(1.0,0.25,1.0-exp(-cover));
  }
 }
 if blade_random(cell,53u)>=density {return out;}
 var index=u32(a.w);
 if a.y>=0.0 {
  let texel=vec2<i32>(floor(fract(world/32768.0)*256.0));
  var p=vec2<i32>(texel.x,255-texel.y);
  switch u32(a.z)%4u {case 1u:{p=vec2<i32>(255-texel.y,255-texel.x);} case 2u:{p=vec2<i32>(255-texel.x,texel.y);} case 3u:{p=vec2<i32>(texel.y,texel.x);} default:{}}
  index=read_tile(p,i32(a.y));
 }
 if index==255u {return out;}
 let ground=seam_apply(shade(index,base_row()).rgb,terrain_seam_response(world));
 let dry=scene.veg.w==1.0 || scene.veg.w==2.0 || habitat==3u;
 var color=ground*select(vec3<f32>(0.92,1.10,0.83),vec3<f32>(1.12,1.02,0.75),dry);
 if habitat==2u {color*=vec3<f32>(1.12,1.04,0.79);}
 color*=0.82+0.30*blade_random(cell,59u);
 var height=select(2.1,1.2,habitat==3u);
 if habitat==2u {height=1.6;}
 if scene.veg.w==3.0 {height*=1.5;}
 if scene.veg.w==1.0 || scene.veg.w==2.0 {height*=0.65;}
 height*=(0.75+0.50*blade_random(cell,61u))*grow*(1.0+0.35*far);
 let yaw=6.2831853*blade_random(cell,67u);
 let across=vec2<f32>(cos(yaw),sin(yaw));
 let shape=array<vec2<f32>,5>(vec2<f32>(-0.5,0.0),vec2<f32>(0.5,0.0),vec2<f32>(-0.22,0.55),vec2<f32>(0.22,0.55),vec2<f32>(0.0,1.0))[vertex];
 let width=(0.20+0.20*blade_random(cell,71u))*grow*(1.0+0.8*far);
 let speed=length(blades.wind.yz);
 let wind=normalize(blades.wind.yz+vec2<f32>(0.001,0.0007));
 let gust=detail_noise(world/512.0+wind*blades.wind.x*0.01,0,149u);
 let phase=blades.wind.x*(0.7+min(speed,60.0)*0.006)+gust*2.0;
 let wave=sin(dot(world,vec2<f32>(0.009,0.006))+phase)*0.65
         +sin(dot(world,vec2<f32>(-0.004,0.011))+phase*0.71)*0.35;
 let lean=vec2<f32>(blade_random(cell,79u)-0.5,blade_random(cell,83u)-0.5)*0.45;
 let bend=(lean+wind*(0.06+min(speed,60.0)*0.004)*(0.65+0.35*gust)*wave)*height*shape.y*shape.y;
 let offset=across*shape.x*width;
 // Both root vertices follow the same terrain plane, preserving bottom contact.
 let p=base+vec3<f32>(offset.x+bend.x,shape.y*height+dot(offset,grad),offset.y+bend.y);
 let direction=p-scene.local_eye.xyz;
 let placed=placed_vertex(p+scene.origin.xyz,direction,vec2<f32>(0.0),-1.0,vec3<f32>(1.0),-1.0);
 out.clip=placed.clip;out.direction=direction;out.altitude=placed.altitude;
 out.color=color;out.height=shape.y;
 out.normal=normalize(vec3<f32>(-across.y*0.25,1.0,across.x*0.25));
 return out;
}
@fragment fn blade_fragment(in:BladeOut)->@location(0) vec4<f32> {
 let color=in.color*mix(0.55,1.15,in.height);
 var lit=veg_light(color,color,in.direction,in.normal,true,mix(0.6,1.0,in.height));
 if thermal_on() {return vec4<f32>(lit,1.0);}
 lit=aerial_perspective(lit,in.direction,in.altitude);
 return scenery(lit);
}
