struct Camera {
    view_proj : mat4x4<f32>,
    camera_pos : vec4<f32>,
    inv_view_proj : mat4x4<f32>,
    world_params : vec4<f32>,
};

@group(0) @binding(0) var<uniform> cam : Camera;

struct VsIn {
    @location(0) pos : vec3<f32>,
};

struct VsOut {
    @builtin(position) pos : vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out : VsOut;
    out.pos = cam.view_proj * vec4<f32>(in.pos * cam.world_params.x - cam.camera_pos.xyz, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.10, 0.92, 1.0, 1.0);
}
