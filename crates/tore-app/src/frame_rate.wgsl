// The frame-rate readout: the same glyph rectangles as ui_text.wgsl, in
// window pixels instead of the 640 by 480 menu canvas.
struct Instance {
    @location(0) dst: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) color: vec4<f32>,
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texel: vec2<f32>,
    @location(1) px: vec2<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) color: vec4<f32>,
};
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
@group(0) @binding(2) var<uniform> frame_size: vec4<f32>;
@vertex fn vertex(@builtin(vertex_index) index: u32, instance: Instance) -> VertexOutput {
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let corner = corners[index];
    let px = instance.dst.xy + corner * instance.dst.zw;
    let size = max(frame_size.xy, vec2<f32>(1.0));
    var result: VertexOutput;
    result.position = vec4(px.x / size.x * 2.0 - 1.0, 1.0 - px.y / size.y * 2.0, 0.0, 1.0);
    result.texel = mix(instance.uv.xy, instance.uv.zw, corner);
    result.px = px;
    result.clip = instance.clip;
    result.color = instance.color;
    return result;
}
@fragment fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let coverage = textureSample(atlas, atlas_sampler, input.texel / vec2<f32>(textureDimensions(atlas))).r;
    let inside = input.px.x >= input.clip.x && input.px.x < input.clip.z
        && input.px.y >= input.clip.y && input.px.y < input.clip.w;
    return vec4(input.color.rgb, select(0.0, input.color.a * coverage, inside));
}
