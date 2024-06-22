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
    let in_box_color = textureSample(r_tex_color, r_tex_sampler, tex_coord);
    let out_of_box_color = vec3<f32>(tex_coord, 0.0);
    let in_box =
        step(r_locals.x, tex_coord.x) *
        step(r_locals.y, tex_coord.y) *
        step(tex_coord.x, r_locals.x + r_locals.width) *
        step(tex_coord.y, r_locals.y + r_locals.height);

    return vec4<f32>(mix(out_of_box_color.rgb, in_box_color.rgb, in_box * 0.5 + 0.5), 1.0);
}