// Vertex shader bindings
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) opacity: f32
}

const ICON_Z: f32 = 0.1;

struct Matrix {
    v: mat4x4<f32>,
}

@group(0) @binding(2)
var<uniform> ortho: Matrix;
@group(0) @binding(3)
var<uniform> atlas_dimensions: vec2<u32>;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) instance_position: vec4<f32>, // Icon x, icon y, icon width, icon height
    @location(2) icon_state: vec3<f32> // Icon atlas position, icon opacity
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = position / vec2<f32>(atlas_dimensions) + icon_state.xy;
    out.clip_position = ortho.v * vec4<f32>(position * instance_position.zw + instance_position.xy, ICON_Z, 1.0);
    out.opacity = icon_state.z;
    return out;
}

// Fragment shader bindings
@group(0) @binding(0) var r_icon_sampler: sampler;
@group(0) @binding(1) var r_icon_texture: texture_2d<f32>;

@fragment
fn fs_main(
    @location(0) tex_coord: vec2<f32>,
    @location(1) opacity: f32
) -> @location(0) vec4<f32> {
    var color = textureSample(r_icon_texture, r_icon_sampler, tex_coord);
    color.a *= opacity;
    return color;
}