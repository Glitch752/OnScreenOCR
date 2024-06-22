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
    @location(4) blur_enabled: u32
}
@group(0) @binding(2) var<uniform> r_locals: Locals;

const BLUR_RADIUS = 2.0;
const BLUR_ITERATIONS = 2.0;
const OUT_OF_BOX_TINT = vec3<f32>(0.6, 0.6, 0.6);

const BORDER_WIDTH = 2.0;
const BORDER_COLOR = vec3<f32>(0.482, 0.412, 0.745);

fn get_blurred_color(
    tex_color: texture_2d<f32>,
    tex_sampler: sampler,
    tex_coord: vec2<f32>,
    screen_dimensions: vec2<f32>,
    radius: f32
) -> vec3<f32> {
    var color: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    for (var i: f32 = 0.0; i < BLUR_ITERATIONS; i = i + 1.) {
        color += get_blurred_color_iteration(tex_color, tex_sampler, tex_coord, screen_dimensions, radius);
    }
    return color / BLUR_ITERATIONS;
}

fn get_blurred_color_iteration(
    tex_color: texture_2d<f32>,
    tex_sampler: sampler,
    tex_coord: vec2<f32>,
    screen_dimensions: vec2<f32>,
    radius: f32
) -> vec3<f32> {
    var color: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    var weight: f32 = 0.0;
    for (var x: f32 = -radius; x <= radius; x = x + 1.) {
        for (var y: f32 = -radius; y <= radius; y = y + 1.) {
            let sample_coord = tex_coord + vec2<f32>(x, y) / screen_dimensions;
            let sample_color = textureSample(tex_color, tex_sampler, sample_coord).rgb;
            let distance = length(vec2<f32>(x, y));
            let weight_factor = max(0.0, radius - distance);
            color += sample_color * weight_factor;
            weight += weight_factor;
        }
    }
    return color / weight;
}

@fragment
fn fs_main(
    @location(0) tex_coord: vec2<f32>
) -> @location(0) vec4<f32> {
    let screen_dimensions = vec2<f32>(textureDimensions(r_tex_color));

    let in_box_color = textureSample(r_tex_color, r_tex_sampler, tex_coord).rgb;
    var out_of_box_color: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    if(r_locals.blur_enabled == 1u) {
        out_of_box_color = get_blurred_color(r_tex_color, r_tex_sampler, tex_coord, screen_dimensions, BLUR_RADIUS);
    } else {
        out_of_box_color = textureSample(r_tex_color, r_tex_sampler, tex_coord).rgb;
    }

    let half_horiz_bw = BORDER_WIDTH / screen_dimensions.x / 2.0;
    let half_vert_bw = BORDER_WIDTH / screen_dimensions.y / 2.0;

    let in_border =
        step(r_locals.x - half_horiz_bw, tex_coord.x) *
        step(r_locals.y - half_vert_bw, tex_coord.y) *
        step(tex_coord.x, r_locals.x + r_locals.width + half_horiz_bw) *
        step(tex_coord.y, r_locals.y + r_locals.height + half_vert_bw);
    let in_box =
        step(r_locals.x + half_horiz_bw, tex_coord.x) *
        step(r_locals.y + half_vert_bw, tex_coord.y) *
        step(tex_coord.x, r_locals.x + r_locals.width - half_horiz_bw) *
        step(tex_coord.y, r_locals.y + r_locals.height - half_vert_bw);

    let in_border_color = mix(BORDER_COLOR, in_box_color, in_box);

    return vec4<f32>(mix(out_of_box_color * OUT_OF_BOX_TINT, in_border_color, in_border), 1.0);
}