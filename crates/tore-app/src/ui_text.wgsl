// Sharp UI text: one instance per glyph rectangle (see `ui_text.rs`), drawn
// over the stretched 640 by 480 menu canvas at the window's own resolution.
// The vertex position is in canvas pixels; the pass's viewport is the canvas's
// place in the window, so the whole viewport is the canvas.
struct Instance {
    @location(0) dst: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) color: vec4<f32>,
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texel: vec2<f32>,
    @location(1) canvas: vec2<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) color: vec4<f32>,
};
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
const CANVAS = vec2<f32>(640.0, 480.0);
@vertex fn vertex(@builtin(vertex_index) index: u32, instance: Instance) -> VertexOutput {
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let corner = corners[index];
    let canvas = instance.dst.xy + corner * instance.dst.zw;
    var result: VertexOutput;
    result.position = vec4(canvas.x / CANVAS.x * 2.0 - 1.0, 1.0 - canvas.y / CANVAS.y * 2.0, 0.0, 1.0);
    result.texel = mix(instance.uv.xy, instance.uv.zw, corner);
    result.canvas = canvas;
    result.clip = instance.clip;
    result.color = instance.color;
    return result;
}
@fragment fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sampled before the clip test so the derivatives stay uniform.
    let coverage = textureSample(atlas, atlas_sampler, input.texel / vec2<f32>(textureDimensions(atlas))).r;
    let inside = input.canvas.x >= input.clip.x && input.canvas.x < input.clip.z
        && input.canvas.y >= input.clip.y && input.canvas.y < input.clip.w;
    return vec4(input.color.rgb, select(0.0, input.color.a * coverage, inside));
}
