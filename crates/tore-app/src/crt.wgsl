// CRT display look (crt.rs): a source picture and an optional overlay layer
// composed and shown as on a small monochrome monitor: mild pixelation, a
// soft focus, slight chromatic aberration toward the edges, scanlines and a
// slow rolling interlace band, a vignette, fine noise and, when the static
// level is up, bursts of snow and line tearing. The CPU twin in crt.rs
// (`apply_cpu`) follows the same rules for small instrument pages.
struct Crt {
 // time (s), static level 0..1, aberration (output px at 1080 rows), pixel block (px at 1080 rows)
 a:vec4<f32>,
 // scanline depth, vignette, noise, soft focus radius (px at 1080 rows)
 b:vec4<f32>,
 // monochrome (0/1), invert the source (0/1), source gain, source lift
 c:vec4<f32>,
 // output width, output height, seed, overlay present (0/1)
 d:vec4<f32>,
}
@group(0) @binding(0) var crt_source:texture_2d<f32>;
@group(0) @binding(1) var crt_overlay:texture_2d<f32>;
@group(0) @binding(2) var crt_sampler:sampler;
@group(0) @binding(3) var<uniform> crt:Crt;
struct CrtOut { @builtin(position) clip:vec4<f32>, @location(0) uv:vec2<f32> }
@vertex fn crt_vertex(@builtin(vertex_index) i:u32)->CrtOut {
 let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
 var out:CrtOut;out.clip=vec4(p[i],0.,1.);out.uv=vec2(p[i].x*0.5+0.5,0.5-p[i].y*0.5);return out;
}
// The integer hash crt.rs uses too, so both draw the same snow.
fn crt_hash(v:u32)->u32 {
 var x=v*747796405u+2891336453u;
 x=((x>>((x>>28u)+4u))^x)*277803737u;
 return (x>>22u)^x;
}
fn crt_random(a:u32,b:u32,c:u32)->f32 {
 return f32(crt_hash(a^crt_hash(b^crt_hash(c)))&0xffffu)/65535.0;
}
// Rows of the output scaled to a 1080-row screen.
fn crt_unit()->f32 {return crt.d.y/1080.0;}
// The source picture at `uv`: monochrome, gain and lift applied, inverted,
// softened by a four-tap focus spread.
fn crt_picture(uv:vec2<f32>)->vec3<f32> {
 let r=crt.b.w*crt_unit()/crt.d.xy;
 var c=vec3<f32>(0.0);
 c+=textureSampleLevel(crt_source,crt_sampler,uv+vec2(r.x,0.0),0.0).rgb;
 c+=textureSampleLevel(crt_source,crt_sampler,uv-vec2(r.x,0.0),0.0).rgb;
 c+=textureSampleLevel(crt_source,crt_sampler,uv+vec2(0.0,r.y),0.0).rgb;
 c+=textureSampleLevel(crt_source,crt_sampler,uv-vec2(0.0,r.y),0.0).rgb;
 c*=0.25;
 if crt.c.x>0.5 {c=vec3<f32>(dot(c,vec3<f32>(0.2126,0.7152,0.0722)));}
 c=clamp(c*crt.c.z+vec3<f32>(crt.c.w),vec3<f32>(0.0),vec3<f32>(1.0));
 // Inverted (black hot) at 85 percent, so a cool scene is not a white page.
 if crt.c.y>0.5 {c=(vec3<f32>(1.0)-c)*0.85;}
 return c;
}
// Source and overlay composed at `uv` (straight alpha overlay).
fn crt_compose(uv:vec2<f32>)->vec3<f32> {
 let block=max(crt.a.w*crt_unit(),1.0);
 let snapped=(floor(uv*crt.d.xy/block)+0.5)*block/crt.d.xy;
 var c=crt_picture(snapped);
 if crt.d.w>0.5 {
  let o=textureSampleLevel(crt_overlay,crt_sampler,uv,0.0);
  c=mix(c,o.rgb,o.a);
 }
 return c;
}
@fragment fn crt_fragment(in:CrtOut)->@location(0) vec4<f32> {
 let px=in.uv*crt.d.xy;
 let unit=crt_unit();
 let level=clamp(crt.a.y,0.0,1.0);
 let seed=u32(crt.d.z);
 let frame=u32(crt.a.x*60.0);
 var uv=in.uv;
 // Static tears the picture sideways in bands of about 6 rows.
 if level>0.0 {
  let band=u32(px.y/(6.0*unit));
  let tear=(crt_random(band,frame,seed)-0.5)*0.04*level*level;
  uv.x+=tear*step(0.55,crt_random(band,frame+7u,seed));
 }
 // Chromatic aberration: red and blue pulled apart radially, more toward the edges.
 let from_centre=uv-vec2<f32>(0.5);
 let shift=from_centre*2.0*crt.a.z*unit/crt.d.xy*(0.35+length(from_centre)*1.3);
 var c=vec3<f32>(crt_compose(uv+shift).r,crt_compose(uv).g,crt_compose(uv-shift).b);
 // Scanlines, a period of 3 rows at 1080, and a faint band rolling down.
 let line=0.5+0.5*cos(6.2831853*px.y/(3.0*unit));
 c*=1.0-crt.b.x*line;
 let roll=fract(in.uv.y-crt.a.x*0.07);
 c*=1.0-0.06*crt.b.x/0.12*smoothstep(0.0,0.04,roll)*(1.0-smoothstep(0.04,0.09,roll));
 // Vignette.
 let edge=length(from_centre*vec2<f32>(1.0,crt.d.y/crt.d.x)*1.6);
 c*=1.0-crt.b.y*pow(clamp(edge,0.0,1.4),2.4);
 // Fine noise every frame, in 2 px grains.
 let grain=vec2<u32>(px/max(2.0*unit,1.0));
 c+=vec3<f32>((crt_random(grain.x,grain.y,frame^seed)-0.5)*crt.b.z);
 // Static: snow over the picture and the odd bright or dark line.
 if level>0.0 {
  let snow=crt_random(grain.x,grain.y,(frame*31u)^seed^0x5bd1e995u);
  c=mix(c,vec3<f32>(snow),0.6*level);
  let row=u32(px.y/max(unit,1.0));
  let streak=crt_random(row,frame,seed^0x68e31da4u);
  if streak>1.0-0.04*level {c=mix(c,vec3<f32>(1.0),0.6);}
  if streak<0.03*level {c*=0.2;}
 }
 return vec4<f32>(clamp(c,vec3<f32>(0.0),vec3<f32>(1.0)),1.0);
}
