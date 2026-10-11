// Experiment AP1: airfield lights of redrawn airports (airfield_lights.rs,
// docs/formats/redrawn-airports.md). Appended after countermeasures.wgsl,
// whose device_* helpers it shares. Additive points that grow a little as
// they near, dim with distance, haze and cloud, and show at night and dusk.
struct AirfieldLightOut {
 @builtin(position) clip:vec4<f32>,
 @location(0) local:vec2<f32>,
 @location(1) @interpolate(flat) radiance:vec4<f32>,
}
// Kinds: 0 edge, 1 threshold, 2 end, 3 approach, 4 approach side row,
// 5 flasher, 6 PAPI, 7 taxiway.
@vertex fn airfield_light_vertex(@builtin(vertex_index) vertex:u32,@location(0) center:vec3<f32>,@location(1) color:vec3<f32>,@location(2) facing:vec2<f32>,@location(3) param:f32,@location(4) range:f32,@location(5) day:f32,@location(6) kind:u32)->AirfieldLightOut {
 var out:AirfieldLightOut;
 out.clip=vec4<f32>(0.0,0.0,-1.0,1.0);
 let p=center-scene.eye.xyz;
 let z=dot(p,scene.forward.xyz);
 let distance=length(p);
 if z<=max(scene.view.x,1.0) || distance>range {return out;}
 var level=mix(day,1.0,device_night());
 // Directional lights fade outside their beam.
 if dot(facing,facing)>0.5 {
  let toward=normalize(-p.xz+vec2<f32>(0.0001,0.0));
  let beam=dot(facing,toward);
  if kind==1u || kind==2u {level*=smoothstep(0.0,0.5,beam);} else {level*=smoothstep(0.866,0.966,beam);}
 }
 var rgb=color;
 if kind==6u {
  // PAPI: white above its switching angle, red below.
  let elevation=degrees(atan2(-p.y,max(length(p.xz),1.0)));
  rgb=mix(vec3<f32>(1.0,0.08,0.05),vec3<f32>(1.0,0.97,0.92),smoothstep(param-0.03,param+0.03,elevation));
 }
 if kind==5u {level*=param;}
 // By day the PAPI shows brighter and larger, as a real one reads on a
 // daylight final; at night it keeps the plain point, so it does not bloom.
 let papi_day=select(0.0,1.0-device_night(),kind==6u);
 level*=1.0+0.8*papi_day;
 level*=clamp(8000.0/max(distance,1.0),0.6,1.0)*(1.0-smoothstep(0.75*range,range,distance));
 if level<=0.002 {return out;}
 let pixel=device_pixel(z);
 let radius=pixel*(2.0+2.5*clamp(800.0/max(distance,1.0),0.0,1.0))*select(1.0,1.5,kind==5u)*(1.0+0.4*papi_day);
 let corners=array<vec2<f32>,6>(vec2(-1.0,-1.0),vec2(1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,1.0));
 let local=corners[vertex]*radius*2.0;
 let q=p+scene.right.xyz*local.x+scene.up.xyz*local.y;
 out.clip=device_clip(q,dot(q,scene.forward.xyz));
 out.local=corners[vertex]*2.0;
 // A light carries through haze about three times as far as a surface
 // (fitted), so it is dimmed as a surface at a third of its distance is.
 out.radiance=vec4<f32>(rgb*device_visibility(p*0.33,center.y)*level,1.0);
 return out;
}
@fragment fn airfield_light_fragment(in:AirfieldLightOut)->@location(0) vec4<f32> {
 let r2=dot(in.local,in.local);
 let core=exp(-r2*2.5)*3.0+exp(-r2*0.5)*0.45;
 return vec4<f32>(in.radiance.rgb*core,0.0);
}
