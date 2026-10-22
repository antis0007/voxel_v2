struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
    world_params: vec4<f32>,
    prev_view_proj: mat4x4<f32>,
    camera_forward: vec4<f32>,
}
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = camera.view_proj * vec4<f32>(model.pos * camera.world_params.x - camera.camera_pos.xyz, 1.0);
    out.color = model.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
