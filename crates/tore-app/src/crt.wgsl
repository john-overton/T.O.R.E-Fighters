// CRT display look (crt.rs): a source picture and an optional overlay layer
// composed and shown as on a small monochrome monitor: mild pixelation, a
// soft focus, slight chromatic aberration toward the edges, scanlines and a
// slow rolling interlace band, a vignette, fine noise and, when the static
// level is up, bursts of snow and line tearing. The CPU twin in crt.rs
// (`apply_cpu`) follows the same rules for small instrument pages.
// The share of the aberration's colour fringes kept (crt.rs).
const ABERRATION_COLOUR:f32=0.4;
struct Crt {
 // time (s), static level 0..1, aberration (output px at 1080 rows), pixel block (px at 1080 rows)
 a:vec4<f32>,
 // scanline depth, vignette, noise, soft focus radius (px at 1080 rows)
 b:vec4<f32>,
 // monochrome (0/1), invert the source (0/1), source gain, source lift
 c:vec4<f32>,
 // output width, output height, seed, overlay present (0/1)
 d:vec4<f32>,
 // phosphor tint of the picture (rgb), colour picture (0/1: fringes fade toward the unshifted picture, not grey)
 e:vec4<f32>,
 // bloom strength, soft shoulder (0/1), unused x2
 f:vec4<f32>,
 // Tape (crt.rs `Tape`): luma blur, chroma blur, chroma delay (px at 1080 rows), saturation
 g:vec4<f32>,
 // black level, white level, edge sharpening, bloom
 h:vec4<f32>,
 // cast I, cast Q, line jitter (px), frame jitter (px)
 i:vec4<f32>,
 // head switching band (share of height), tracking band strength, its height (0 top..1 bottom), chroma noise
 j:vec4<f32>,
 // tape on (0/1), dropout row (0..1, negative none), dropout start, dropout length (shares of the width)
 k:vec4<f32>,
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
 c=c*crt.c.z+vec3<f32>(crt.c.w);
 // Bloom: what the gain lifts past full scale spills over the picture
 // round it (eight taps on two rings), an intensifier's halo.
 if crt.f.x>0.0 {
  var halo=0.0;
  for(var i=0;i<8;i++){
   let a=f32(i)*0.785398;
   let ring=select(9.0,18.0,(i&1)==1)*crt_unit();
   let o=vec2<f32>(cos(a),sin(a))*ring/crt.d.xy;
   let t=textureSampleLevel(crt_source,crt_sampler,uv+o,0.0).rgb;
   halo+=max(dot(t,vec3<f32>(0.2126,0.7152,0.0722))*crt.c.z+crt.c.w-0.9,0.0);
  }
  c+=vec3<f32>(halo*crt.f.x/8.0);
 }
 // A logarithmic response (night vision): the gained picture is shown as
 // log(1 + v) / log(1 + gain), so the dark ground comes up while a light
 // many times brighter keeps its own falloff instead of clipping into a
 // flat plate.
 if crt.f.y>0.0 {c=log2(vec3<f32>(1.0)+max(c,vec3<f32>(0.0)))/log2(1.0+max(crt.c.z,1.0));}
 c=clamp(c,vec3<f32>(0.0),vec3<f32>(1.0));
 // Inverted (black hot) at 85 percent, so a cool scene is not a white page.
 if crt.c.y>0.5 {c=(vec3<f32>(1.0)-c)*0.85;}
 // The phosphor's colour, on the picture only (the symbology keeps its own).
 return c*crt.e.xyz;
}
// Source and overlay composed at `uv` moved by the aberration `shift`
// (straight alpha overlay). The source's block is chosen from `uv` alone, so
// all three colours read the same block: a block edge never splits into
// colours, only the aberration's own sub-pixel shift does.
fn crt_compose(uv:vec2<f32>,shift:vec2<f32>)->vec3<f32> {
 let block=max(crt.a.w*crt_unit(),1.0);
 let snapped=(floor(uv*crt.d.xy/block)+0.5)*block/crt.d.xy;
 var c=crt_picture(snapped+shift);
 if crt.d.w>0.5 {
  let o=textureSampleLevel(crt_overlay,crt_sampler,uv+shift,0.0);
  c=mix(c,o.rgb,o.a);
 }
 return c;
}
@fragment fn crt_fragment(in:CrtOut)->@location(0) vec4<f32> {
 if crt.k.x>0.5 {return tape_fragment(in);}
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
 var c=vec3<f32>(crt_compose(uv,shift).r,crt_compose(uv,vec2<f32>(0.0)).g,crt_compose(uv,-shift).b);
 // The fringes stay faint: a monochrome tube shows a hint of colour, not a
 // rainbow (40 percent of their colour is kept).
 // A colour source fades its fringes toward its own unshifted colours.
 if crt.e.w>0.5 {
  c=mix(crt_compose(uv,vec2<f32>(0.0)),c,ABERRATION_COLOUR);
 } else {
  c=mix(vec3<f32>(dot(c,vec3<f32>(0.2126,0.7152,0.0722))),c,ABERRATION_COLOUR);
 }
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

// ---------------------------------------------------------------------------
// The videotape look (crt.rs `Tape`, the replay's HUD camera): the picture and
// its overlay recorded on VHS. The tape keeps little horizontal detail, and
// far less for colour than for brightness, so each line is low-passed in YIQ:
// brightness gently, colour widely and a little late. Then the grade (lifted
// blacks, crushed and blooming highlights, less colour with a green-cyan
// cast), the recorder's edge ringing, the time-base wobble of each line, the
// head-switching tear at the bottom, an occasional tracking band and dropout,
// grain and chroma noise, the tape's 240-line scanlines and a vignette. The
// noise and events come from the field number (the replay tick), so a still
// at a tick is always the same picture.
fn tape_composite(uv:vec2<f32>)->vec3<f32> {
 // Half a row down: the linear sampler blends each line with the next, the
 // tape's soft vertical detail (240 lines a field).
 let at=uv+vec2<f32>(0.0,0.5*crt_unit()/crt.d.y);
 var c=textureSampleLevel(crt_source,crt_sampler,at,0.0).rgb;
 if crt.d.w>0.5 {
  let o=textureSampleLevel(crt_overlay,crt_sampler,at,0.0);
  c=mix(c,o.rgb,o.a);
 }
 // The tape's signal is gamma encoded, as video is: the grade works on it.
 return pow(max(c,vec3<f32>(0.0)),vec3<f32>(1.0/2.2));
}
fn tape_y(c:vec3<f32>)->f32 {return dot(c,vec3<f32>(0.299,0.587,0.114));}
fn tape_iq(c:vec3<f32>)->vec2<f32> {
 return vec2<f32>(dot(c,vec3<f32>(0.596,-0.274,-0.322)),dot(c,vec3<f32>(0.211,-0.523,0.312)));
}
fn tape_rgb(y:f32,iq:vec2<f32>)->vec3<f32> {
 return vec3<f32>(y+0.956*iq.x+0.621*iq.y,y-0.272*iq.x-0.647*iq.y,y-1.106*iq.x+1.703*iq.y);
}
// The recorded signal at `uv`: low-passed brightness with its edge ring, the
// wide and late colour, graded.
fn tape_signal(uv:vec2<f32>)->vec3<f32> {
 let unit=crt_unit();
 let step_y=crt.g.x*unit/crt.d.x;
 // Brightness: five taps of a Gaussian of the luma blur, and two wider ones
 // for the ring of the recorder's edge enhancement.
 let w=array<f32,5>(0.0545,0.2442,0.4026,0.2442,0.0545);
 var y=0.0;
 for(var n=0;n<5;n++){
  y+=w[n]*tape_y(tape_composite(uv+vec2<f32>(f32(n-2)*step_y,0.0)));
 }
 let wide=0.5*(tape_y(tape_composite(uv+vec2<f32>(3.0*step_y,0.0)))+tape_y(tape_composite(uv-vec2<f32>(3.0*step_y,0.0))));
 y=y+crt.h.z*(y-mix(wide,y,0.4));
 // Colour: seven taps over the chroma blur, delayed to the right.
 let step_c=crt.g.y*unit/crt.d.x*0.7;
 let late=crt.g.z*unit/crt.d.x;
 let wc=array<f32,7>(0.0366,0.1113,0.2167,0.2707,0.2167,0.1113,0.0366);
 var iq=vec2<f32>(0.0);
 for(var n=0;n<7;n++){
  iq+=wc[n]*tape_iq(tape_composite(uv+vec2<f32>(f32(n-3)*step_c-late,0.0)));
 }
 // The grade: lifted blacks and a soft shoulder under the white level.
 y=crt.h.x+(crt.h.y-crt.h.x)*clamp(y,0.0,1.1);
 y=y-0.35*pow(max(y-0.74,0.0),1.5);
 iq=iq*crt.g.w+crt.i.xy;
 return tape_rgb(y,iq);
}
fn tape_fragment(in:CrtOut)->vec4<f32> {
 let unit=crt_unit();
 let field=u32(crt.a.x*60.0+0.5);
 let seed=u32(crt.d.z);
 let size=crt.d.xy;
 var uv=in.uv;
 // The whole frame shakes a little from field to field.
 let shake=vec2<f32>(crt_random(field,11u,seed)-0.5,crt_random(field,12u,seed)-0.5)*2.0*crt.i.w*unit;
 uv+=shake*vec2<f32>(1.0,0.5)/size;
 let px=uv*size;
 let row=u32(max(px.y,0.0)/max(unit,1.0));
 // Each line slips sideways (time-base error), much more in a tracking band.
 var slip=(crt_random(row,field,seed^0x2c1bu)-0.5)*2.0*crt.i.z*unit;
 let band=crt.j.y*exp(-pow((in.uv.y-crt.j.z)/0.035,2.0));
 slip+=band*(crt_random(row/2u,field,seed^0x77u)-0.5)*9.0*unit;
 // Head switching: the last lines of the picture tear to the right.
 let tear=clamp((in.uv.y-(1.0-crt.j.x))/crt.j.x,0.0,1.0);
 slip+=tear*(10.0+6.0*crt_random(row/3u,field,seed^0x19u))*unit;
 uv.x-=slip/size.x;
 // Chromatic aberration: red and blue pulled apart, more toward the edges.
 let from_centre=uv-vec2<f32>(0.5);
 let shift=from_centre*2.0*crt.a.z*unit/size*(0.35+length(from_centre)*1.3);
 let centre=tape_signal(uv);
 var c=vec3<f32>(tape_signal(uv+shift).r,centre.g,tape_signal(uv-shift).b);
 c=mix(centre,c,0.75);
 // Highlights bloom over their surroundings (eight taps on two rings).
 var halo=0.0;
 for(var n=0;n<8;n++){
  let a=f32(n)*0.785398;
  let ring=select(7.0,15.0,(n&1)==1)*unit;
  let t=tape_composite(uv+vec2<f32>(cos(a),sin(a))*ring/size);
  halo+=max(tape_y(t)-0.72,0.0);
 }
 c+=vec3<f32>(1.0,0.97,0.9)*halo*crt.h.w/8.0;
 // Grain in the brightness, and blotchy coloured noise in the chroma.
 let grain=vec2<u32>(px/max(2.0*unit,1.0));
 let g=crt_random(grain.x,grain.y,field^seed)-0.5;
 let blot=vec2<u32>(u32(px.x/max(7.0*unit,1.0)),u32(px.y/max(2.0*unit,1.0)));
 let n_i=crt_random(blot.x,blot.y,field^seed^0x3a1u)-0.5;
 let n_q=crt_random(blot.x,blot.y,field^seed^0x3a2u)-0.5;
 c+=vec3<f32>(g*crt.b.z)+tape_rgb(0.0,vec2<f32>(n_i,n_q)*2.0*crt.j.w);
 // The tracking band: a bright, noisy smear across the picture.
 if band>0.01 {
  let spark=crt_random(u32(px.x/max(3.0*unit,1.0)),row,field^seed^0x99u);
  c=c*(1.0-0.25*band)+vec3<f32>(band*(0.12+0.5*pow(spark,6.0)));
 }
 // Head switching's band is noisy and a little bright.
 if tear>0.0 {
  c=mix(c,vec3<f32>(crt_random(grain.x,row,field^seed^0x41u)*0.6+0.2),0.35*tear);
 }
 // A dropout: a short white streak on two lines.
 if crt.k.y>=0.0 {
  let dy=abs(in.uv.y-crt.k.y)*size.y;
  if dy<1.5*unit && in.uv.x>=crt.k.z && in.uv.x<=crt.k.z+crt.k.w {
   c=mix(c,vec3<f32>(0.92),0.7);
  }
 }
 // The tape's 240 lines a field (a period of 4.5 rows at 1080), the field
 // that is not shown a little darker (interlace).
 let line=0.5+0.5*cos(6.2831853*px.y/(4.5*unit));
 c*=1.0-crt.b.x*line;
 let odd=f32((u32(px.y/(2.25*unit))+field)&1u);
 c*=1.0-0.02*odd;
 // Vignette.
 let edge=length(from_centre*vec2<f32>(1.0,size.y/size.x)*1.6);
 c*=1.0-crt.b.y*pow(clamp(edge,0.0,1.4),2.4);
 return vec4<f32>(pow(clamp(c,vec3<f32>(0.0),vec3<f32>(1.0)),vec3<f32>(2.2)),1.0);
}
