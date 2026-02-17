struct Uniforms {
    screen_size: vec2<f32>,
    ambient: f32,
    diffuse: f32,
    light_dir: vec3<f32>,
    fast_mode: u32,
    exclusion_rect: vec4<f32>,
};

struct RectInstance {
    rect: vec4<f32>,     // x, y, w, h in pixel space
    color: vec4<f32>,    // linear RGBA
    coeffs: vec4<f32>,   // sx1, sx2, sy1, sy2 in world/pixel space
    info: vec2<f32>,     // size_log_norm, age_norm (reserved)
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

@group(1) @binding(0)
var<storage, read> instances: array<RectInstance>;

struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) coeffs: vec4<f32>,
};

fn quad_vertex(i: u32) -> vec2<f32> {
    let verts = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    return verts[i];
}

fn px_to_ndc(px: vec2<f32>, screen: vec2<f32>) -> vec2<f32> {
    let x = (px.x / screen.x) * 2.0 - 1.0;
    let y = 1.0 - (px.y / screen.y) * 2.0;
    return vec2<f32>(x, y);
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_id: u32,
    @builtin(vertex_index) vertex_id: u32,
) -> VSOut {
    let inst = instances[instance_id];
    let local = quad_vertex(vertex_id);
    let world = inst.rect.xy + local * inst.rect.zw;

    var out: VSOut;
    out.position = vec4<f32>(px_to_ndc(world, u.screen_size), 0.0, 1.0);
    out.world_pos = world;
    out.local_pos = local;
    out.color = inst.color;
    out.coeffs = inst.coeffs;
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    if in.world_pos.x >= u.exclusion_rect.x && in.world_pos.x <= u.exclusion_rect.z &&
       in.world_pos.y >= u.exclusion_rect.y && in.world_pos.y <= u.exclusion_rect.w {
        discard;
    }

    let deriv_x = 2.0 * in.coeffs.y * in.world_pos.x + in.coeffs.x;
    let deriv_y = 2.0 * in.coeffs.w * in.world_pos.y + in.coeffs.z;

    let nn = vec3<f32>(-deriv_x, -deriv_y, 1.0);

    var ndotl: f32;
    if u.fast_mode != 0u {
        // Fast path: approximate normalization with reciprocal sqrt.
        let inv_len = inverseSqrt(max(dot(nn, nn), 1e-5));
        ndotl = max(dot(nn, u.light_dir) * inv_len, 0.0);
    } else {
        let len = max(length(nn), 1e-5);
        ndotl = max(dot(nn, u.light_dir) / len, 0.0);
    }

    // Preserve the cushion's 3D feel while keeping color saturation.
    let brightness = pow(clamp(u.ambient + u.diffuse * ndotl, 0.0, 1.0), 1.16);
    let shadow = in.color.rgb * 0.55;
    let highlight = in.color.rgb * 0.85 + vec3<f32>(0.12, 0.12, 0.12);
    var rgb = mix(shadow, highlight, brightness);
    let sat_boost = 1.0 + 0.14 * brightness;
    rgb = clamp(vec3<f32>(0.5) + (rgb - vec3<f32>(0.5)) * sat_boost, vec3<f32>(0.0), vec3<f32>(1.0));

    // Thin border emphasis improves structure readability.
    let edge = min(min(in.local_pos.x, 1.0 - in.local_pos.x), min(in.local_pos.y, 1.0 - in.local_pos.y));
    let border = smoothstep(0.0, 0.02, edge);
    rgb *= mix(0.82, 1.0, border);

    return vec4<f32>(rgb, in.color.a);
}
