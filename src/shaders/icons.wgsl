// Vertex shader bindings
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

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
    @location(2) icon_state: vec2<f32> // Icon atlas position
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = position / vec2<f32>(atlas_dimensions) + icon_state;
    out.clip_position = ortho.v * vec4<f32>(position * instance_position.zw + instance_position.xy, 0.0, 1.0);
    return out;
}

// Fragment shader bindings
@group(0) @binding(0) var r_icon_sampler: sampler;
@group(0) @binding(1) var r_icon_texture: texture_2d<f32>;

@fragment
fn fs_main(
    @location(0) tex_coord: vec2<f32>
) -> @location(0) vec4<f32> {
    return textureSample(r_icon_texture, r_icon_sampler, tex_coord);
}