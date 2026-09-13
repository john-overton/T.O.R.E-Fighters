@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};
@vertex fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    let points = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    let p = points[index];
    var result: VertexOutput;
    result.position = vec4(p, 0.0, 1.0);
    result.uv = vec2((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return result;
}
@fragment fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(picture, picture_sampler, input.uv);
}
