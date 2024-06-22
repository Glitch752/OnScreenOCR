// Vertex shader bindings

struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = fma(position, vec2<f32>(0.5, -0.5), vec2<f32>(0.5, 0.5));
    out.position = vec4<f32>(position, 0.0, 1.0);
    return out;
}

// Fragment shader bindings

@group(0) @binding(0) var r_tex_color: texture_2d<f32>;
@group(0) @binding(1) var r_tex_sampler: sampler;
struct Locals {
    @location(0) x: f32,
    @location(1) y: f32,
    @location(2) width: f32,
    @location(3) height: f32,
}
@group(0) @binding(2) var<uniform> r_locals: Locals;

@fragment
fn fs_main(
    @location(0) tex_coord: vec2<f32>
) -> @location(0) vec4<f32> {
    let sampled_color = textureSample(r_tex_color, r_tex_sampler, tex_coord);
    // let noise_color = vec3<f32>(random_vec2(tex_coord.xy * vec2<f32>((r_locals.x / 100.) % tau + bias)));
    let noise_color = vec3<f32>(tex_coord, 0.0);

    return vec4<f32>(sampled_color.rgb * noise_color, sampled_color.a);
}