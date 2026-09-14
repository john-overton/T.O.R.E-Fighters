// Body-fixed forward artwork and HUD; head rotation changes their projection together.
struct Cockpit {
 right:vec4<f32>, up:vec4<f32>, forward:vec4<f32>,
 size:vec4<f32>, art:vec4<f32>, hud:vec4<f32>
}
@group(0) @binding(0) var frame:texture_2d<f32>;
@group(0) @binding(1) var symbols:texture_2d<f32>;
@group(0) @binding(2) var filtering:sampler;
@group(0) @binding(3) var<uniform> cockpit:Cockpit;
struct Output { @builtin(position) position:vec4<f32>, @location(0) screen:vec2<f32> }
@vertex fn vertex(@builtin(vertex_index) index:u32)->Output {
 let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
 var out:Output;out.position=vec4(p[index],0.,1.);out.screen=p[index];return out;
}
@fragment fn fragment(in:Output)->@location(0) vec4<f32> {
 let ray=cockpit.forward.xyz+cockpit.right.xyz*in.screen.x*cockpit.size.x/(2.*cockpit.size.z)
     +cockpit.up.xyz*in.screen.y*cockpit.size.y/(2.*cockpit.size.z);
 if ray.z<=0.00001 { discard; }
 let point=vec2(ray.x,-ray.y)/ray.z*cockpit.size.z;
 let art_uv=vec2(point.x/cockpit.size.w+cockpit.art.x*0.5,
     (point.y+cockpit.size.y*0.5)/cockpit.size.w)/cockpit.art.xy;
 let hud_uv=vec2(0.5)+point/cockpit.hud.xy;
 var color=vec4(0.);
 if cockpit.art.z>0. && all(art_uv>=vec2(0.)) && all(art_uv<=vec2(1.)) {
     color=textureSampleLevel(frame,filtering,art_uv,0.);
 }
 if cockpit.art.w>0. && all(hud_uv>=vec2(0.)) && all(hud_uv<=vec2(1.)) {
     let text=textureSampleLevel(symbols,filtering,hud_uv,0.);
     color=text+color*(1.-text.a);
 }
 return color; // Premultiplied filtering/compositing preserves transparent edge colors.
}
