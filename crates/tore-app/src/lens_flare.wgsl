struct Flare { settings:vec4<f32>, circles:array<vec4<f32>,16> }
@group(0) @binding(0) var source:texture_2d<f32>;
@group(0) @binding(1) var palette:texture_2d<f32>;
@group(0) @binding(2) var maps:texture_2d<u32>;
@group(0) @binding(3) var<uniform> flare:Flare;
@vertex fn vertex(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32>{
 let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));return vec4(p[i],0.,1.);
}
@fragment fn fragment(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32>{
 let color=textureLoad(source,vec2<i32>(p.xy),0);
 var index=-1;
 for(var i=0;i<i32(flare.settings.x);i++){
  let c=flare.circles[i];if distance(p.xy,c.xy)>c.z {continue;}
  if index<0 {
   var best=1e30;
   for(var j=0;j<255;j++){
    let delta=color.rgb-textureLoad(palette,vec2(j,0),0).rgb;
    let error=dot(delta,delta);if error<best {best=error;index=j;}
   }
  }
  let remap=textureLoad(maps,vec2(index,0),0).rg;index=i32(remap[i32(c.w)]);
 }
 if index<0 {return color;}
 return vec4(textureLoad(palette,vec2(index,0),0).rgb,color.a);
}
