// Aircraft-forward datum translates the flat art and HUD together during head-look.
struct Cockpit {
 placement:vec4<f32>, size:vec4<f32>, art:vec4<f32>, hud:vec4<f32>, mirrors:array<vec4<f32>,3>
}
@group(0) @binding(0) var frame:texture_2d<u32>;
@group(0) @binding(1) var symbols:texture_2d<f32>;
@group(0) @binding(2) var filtering:sampler;
@group(0) @binding(3) var<uniform> cockpit:Cockpit;
@group(0) @binding(4) var mirror_mask:texture_2d<u32>;
@group(0) @binding(5) var rear:texture_2d<f32>;
@group(0) @binding(6) var art_palette:texture_2d<f32>;
fn frame_sample(uv:vec2<f32>)->vec4<f32> {
 let size=vec2<i32>(textureDimensions(frame));
 let p=uv*vec2<f32>(size)-vec2(0.5);let base=floor(p);let f=p-base;
 var color=vec4(0.0);
 for(var y=0;y<2;y++){for(var x=0;x<2;x++){
  let source=textureLoad(frame,clamp(vec2<i32>(base)+vec2(x,y),vec2(0),size-vec2(1)),0).rg;
  let c=textureLoad(art_palette,vec2(i32(source.r),0),0);
  color+=c*f32(source.g)*select(1.-f.x,f.x,x==1)*select(1.-f.y,f.y,y==1);
 }}
 return color;
}
struct Output { @builtin(position) position:vec4<f32>, @location(0) screen:vec2<f32> }
@vertex fn vertex(@builtin(vertex_index) index:u32)->Output {
 let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
 var out:Output;out.position=vec4(p[index],0.,1.);out.screen=p[index];return out;
}
@fragment fn fragment(in:Output)->@location(0) vec4<f32> {
 let pixel=vec2((in.screen.x+1.)*.5,(1.-in.screen.y)*.5)*cockpit.size.xy;
 let origin=vec2((cockpit.size.x-cockpit.art.x*cockpit.placement.z)*.5,
     max(cockpit.size.y-cockpit.art.y*cockpit.placement.z,
         (cockpit.size.y-cockpit.art.y*cockpit.placement.z)*.5))+cockpit.placement.xy;
 let art_uv=(pixel-origin)/(cockpit.art.xy*cockpit.placement.z);
 // Zoom in about the eye line; zoom out keeps the lower frame on screen.
 let hud_center=vec2(cockpit.size.x*.5,cockpit.size.y*(.5+max(0.,.5*(1.-cockpit.size.z))))
     +cockpit.placement.xy;
 let hud_uv=vec2(.5)+(pixel-hud_center)/cockpit.hud.xy;
 var color=vec4(0.);
 if cockpit.art.z>0. && all(art_uv>=vec2(0.)) && all(art_uv<=vec2(1.)) {
     color=frame_sample(art_uv);
     let source=art_uv*cockpit.art.xy;
     let id=textureLoad(mirror_mask,vec2<i32>(source),0).r;
     if id>0u && id<=3u {
         let rect=cockpit.mirrors[id-1u];
         let uv=(source-rect.xy)/rect.zw;
         // Aspect-preserving crops of one horizontally reflected rear panorama.
         let crop_width=min(1.,rect.z/rect.w/2.);
         let crop_height=min(1.,2.*rect.w/rect.z);
         let center=array<f32,3>(.5,.8,.2)[id-1u];
         let start=clamp(center-crop_width*.5,0.,1.-crop_width);
         color=vec4(textureSampleLevel(rear,filtering,vec2(start+(1.-uv.x)*crop_width,.5+(uv.y-.5)*crop_height),0.).rgb,1.);
     }
 }
 if cockpit.art.w>0. && all(hud_uv>=vec2(0.)) && all(hud_uv<=vec2(1.)) {
     let text=textureSampleLevel(symbols,filtering,hud_uv,0.);
     color=text+color*(1.-text.a);
 }
 return color*cockpit.placement.w; // Premultiplied filtering/compositing preserves transparent edge colors.
}
