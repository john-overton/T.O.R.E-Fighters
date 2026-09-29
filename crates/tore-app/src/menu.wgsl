@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
// G-effect veils on the flight canvas: x redout, y blackout, both 0..1 levels.
// Zero everywhere except live flight.
@group(0) @binding(2) var<uniform> veil: vec4<f32>;
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
fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3(0.0031308));
}
fn from_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}
// Same darkening as GEffects::coverage: the edges go first, all of it at full loss.
fn coverage(level: f32, radius: f32) -> f32 {
    return clamp(level * 1.5 - 0.5 * (1.0 - clamp(radius, 0.0, 1.0)), 0.0, 1.0);
}
// Straight-alpha "over" in the encoded (byte) values the veil has always used,
// so the result still blends onto the world.
fn over(pixel: vec4<f32>, color: vec3<f32>, a: f32) -> vec4<f32> {
    let under = pixel.a * (1.0 - a);
    let out_a = a + under;
    return vec4((color * a + pixel.rgb * under) / max(out_a, 1e-6), out_a);
}
@fragment fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(picture, picture_sampler, input.uv);
    if veil.x <= 0.0 && veil.y <= 0.0 {
        return sampled;
    }
    let size = vec2<f32>(textureDimensions(picture));
    let radius = length((input.uv - vec2(0.5)) * size) / max(length(size * 0.5), 1.0);
    var pixel = vec4(to_srgb(sampled.rgb), sampled.a);
    let red = coverage(veil.x, radius);
    if red > 0.0 {
        pixel = over(pixel, vec3(150.0 / 255.0, 0.0, 0.0), red);
    }
    let black = coverage(veil.y, radius);
    if black > 0.0 {
        pixel = over(pixel, vec3(0.0), black);
    }
    return vec4(from_srgb(pixel.rgb), pixel.a);
}
