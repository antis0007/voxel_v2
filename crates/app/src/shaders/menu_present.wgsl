@group(0) @binding(0) var scene: texture_2d<f32>;
struct Vertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return Vertex(vec4<f32>(uv * vec2<f32>(2.0,-2.0) + vec2<f32>(-1.0,1.0),0.0,1.0),uv);
}
// Every texel covers the same whole number of screen pixels. The host sizes
// the texture as ceil(viewport / scale) with scale = ceil(height / 360); the
// viewport size comes back out of the uv derivatives, so no uniform is needed.
@fragment fn fs_main(v: Vertex) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(scene));
    let view = round(vec2<f32>(1.0 / dpdx(v.uv.x), 1.0 / abs(dpdy(v.uv.y))));
    let scale = max(ceil(view.y / 360.0), 1.0);
    return textureLoad(scene, clamp(vec2<i32>(floor(v.uv * view / scale)), vec2<i32>(0), size - vec2<i32>(1)), 0);
}
