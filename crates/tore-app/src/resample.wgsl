// Render scale: resample the world image to the output size. Downsampling
// averages bilinear taps across each output pixel's footprint; upsampling is
// plain bilinear. The image is an sRGB texture, so filtering is linear light.
@group(0) @binding(0) var source:texture_2d<f32>;
@group(0) @binding(1) var smooth_sampler:sampler;
struct Out { @builtin(position) clip:vec4<f32>, @location(0) uv:vec2<f32> }
@vertex fn vertex(@builtin(vertex_index) i:u32)->Out {
 let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
 var out:Out;out.clip=vec4(p[i],0.,1.);out.uv=vec2(p[i].x*0.5+0.5,0.5-p[i].y*0.5);return out;
}
@fragment fn fragment(in:Out)->@location(0) vec4<f32>{
 let size=vec2<f32>(textureDimensions(source));
 // Source pixels covered by one output pixel along each axis.
 let footprint=fwidth(in.uv)*size;
 let taps=vec2<i32>(clamp(ceil(footprint),vec2(1.0),vec2(4.0)));
 let step=fwidth(in.uv)/vec2<f32>(taps);
 let start=in.uv-0.5*fwidth(in.uv)+0.5*step;
 var sum=vec4<f32>(0.0);
 for(var y=0;y<taps.y;y++){
  for(var x=0;x<taps.x;x++){
   sum+=textureSampleLevel(source,smooth_sampler,start+step*vec2<f32>(f32(x),f32(y)),0.0);
  }
 }
 return sum/f32(taps.x*taps.y);
}
