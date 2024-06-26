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

struct Vertex {
    @location(0) position: vec2<f32>,
    // Split into two 16-bit values -- one for the actual vertex, and one for the edge connecting it and the next vertex.
    @location(1) highlight: u32,
}

struct Locals {
    @location(0) blur_enabled: u32,
    @location(2) vertex_count: u32,
    @location(3) vertices: array<Vertex>,
}
@group(0) @binding(2) var<storage, read> r_locals: Locals;

const BLUR_RADIUS = 2.0;
const BLUR_ITERATIONS = 2.0;
const OUT_OF_BOX_TINT = vec3<f32>(0.4, 0.4, 0.42);

const BORDER_WIDTH = 1.0;
const BORDER_INNER_COLOR = vec4<f32>(0.482, 0.412, 0.745, 1.0);
const BORDER_OUTER_COLOR = vec4<f32>(0.482, 0.412, 0.745, 0.0);

// Reference: https://www.shadertoy.com/view/wdBXRW
fn polygon_signed_distance(point: vec2<f32>, screen_dimensions: vec2<f32>) -> f32 {
    var screen_point: vec2<f32> = point * screen_dimensions;

    var d: f32 = dot(
        screen_point - r_locals.vertices[0].position * screen_dimensions,
        screen_point - r_locals.vertices[0].position * screen_dimensions
    );
    var s: f32 = 1.0;

    var num = i32(r_locals.vertex_count);
    var last: vec2<f32> = r_locals.vertices[num - 1].position * screen_dimensions;
    for(var i = 0; i < num; i += 1) {
        var current: vec2<f32> = r_locals.vertices[i].position * screen_dimensions;

        // Distance
        var e: vec2<f32> = last - current;
        var w: vec2<f32> = screen_point - current;

        var b: vec2<f32> = w - e * clamp(dot(w,e) / dot(e,e), 0.0, 1.0);
        d = min(d, dot(b,b));

        // Winding number from https://web.archive.org/web/20210228233911/http://geomalgorithms.com/a03-_inclusion.html
        var conditions: vec3<bool> = vec3<bool>(
            screen_point.y >= current.y,
            screen_point.y < last.y,
            e.x*w.y > e.y*w.x
        );
        if(all(conditions) || !any(conditions)) {
            s = -s;
        }

        last = current;
    }
    
    return s * sqrt(d);
}

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

fn alpha_mix(color: vec3<f32>, overlay_color: vec4<f32>) -> vec3<f32> {
    return color * overlay_color.a + overlay_color.rgb * (1.0 - overlay_color.a);
}

fn color_at_position(tex_coord: vec2<f32>, screen_dimensions: vec2<f32>, in_box_color: vec3<f32>, out_of_box_color: vec3<f32>) -> vec3<f32> {
    var in_box: f32 = 0.0;
    var distance = polygon_signed_distance(tex_coord, screen_dimensions);
    if(distance < 0.0) {
        in_box = 1.0;
    }
    return mix(
        // Color for outside the main selection
        alpha_mix(
            // Background color
            out_of_box_color * OUT_OF_BOX_TINT,
            // Border color
            mix(BORDER_OUTER_COLOR, BORDER_INNER_COLOR, min(distance / BORDER_WIDTH, 1.))
        ),
        // Color for inside the main selection
        in_box_color,
        in_box
    );
}

@fragment
fn fs_main(
    @location(0) tex_coord: vec2<f32>
) -> @location(0) vec4<f32> {
    let screen_dimensions = vec2<f32>(textureDimensions(r_tex_color));

    let in_box_color = textureSample(r_tex_color, r_tex_sampler, tex_coord).rgb;
    var out_of_box_color: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    if(r_locals.blur_enabled != 0u) {
        out_of_box_color = get_blurred_color(r_tex_color, r_tex_sampler, tex_coord, screen_dimensions, BLUR_RADIUS);
    } else {
        out_of_box_color = textureSample(r_tex_color, r_tex_sampler, tex_coord).rgb;
    }
    
    return vec4<f32>(
        color_at_position(tex_coord, screen_dimensions, in_box_color, out_of_box_color),
        1.0
    );
}