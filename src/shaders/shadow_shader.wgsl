struct ShadowGenUniform {
    light_view_proj: mat4x4<f32>,
    world_scale: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> gen: ShadowGenUniform;

struct VsIn {
    @location(0) pos : vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> @builtin(position) vec4<f32> {
    return gen.light_view_proj * vec4<f32>(in.pos * gen.world_scale, 1.0);
}
