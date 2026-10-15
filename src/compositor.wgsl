// 3D Paintcraft - GPU Layer Compositor (Ping-Pong Render Pass)

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let u = f32((in_vertex_index << 1u) & 2u);
    let v = f32(in_vertex_index & 2u);
    var out: VertexOutput;
    out.uv = vec2<f32>(u, v);
    out.clip_pos = vec4<f32>(u * 2.0 - 1.0, 1.0 - v * 2.0, 0.0, 1.0);
    return out;
}

struct CompositorUniforms {
    opacity: f32,
    blend_mode: u32,
    has_mask: u32,
    mask_inverted: u32,
    chan_color: u32,
    chan_normal: u32,
    chan_roughness: u32,
    chan_metallic: u32,
    chan_height: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
};

@group(0) @binding(0) var tex_bg_color: texture_2d<f32>;
@group(0) @binding(1) var tex_bg_normal: texture_2d<f32>;
@group(0) @binding(2) var tex_bg_roughness: texture_2d<f32>;
@group(0) @binding(3) var tex_bg_metallic: texture_2d<f32>;

@group(1) @binding(0) var tex_src_color: texture_2d<f32>;
@group(1) @binding(1) var tex_src_normal: texture_2d<f32>;
@group(1) @binding(2) var tex_src_roughness: texture_2d<f32>;
@group(1) @binding(3) var tex_src_metallic: texture_2d<f32>;
@group(1) @binding(4) var tex_src_mask: texture_2d<f32>;

@group(2) @binding(0) var samp_nearest: sampler;
@group(2) @binding(1) var<uniform> params: CompositorUniforms;

fn get_effective_opacity(uv: vec2<f32>) -> f32 {
    var mask_val = 1.0;
    if params.has_mask == 1u {
        mask_val = textureSample(tex_src_mask, samp_nearest, uv).r;
        if params.mask_inverted == 1u {
            mask_val = 1.0 - mask_val;
        }
    }
    return params.opacity * mask_val;
}

struct FragmentColorOutput {
    @location(0) color: vec4<f32>,
    @location(1) normal: vec4<f32>,
};

@fragment
fn fs_color(in: VertexOutput) -> FragmentColorOutput {
    let bg_color = textureSample(tex_bg_color, samp_nearest, in.uv);
    let src_color = textureSample(tex_src_color, samp_nearest, in.uv);
    
    let bg_normal = textureSample(tex_bg_normal, samp_nearest, in.uv);
    let src_normal = textureSample(tex_src_normal, samp_nearest, in.uv);

    let stroke_alpha = src_color.a;
    let base_opacity = get_effective_opacity(in.uv) * stroke_alpha;
    let eff_color_op = select(0.0, base_opacity, params.chan_color == 1u);
    let eff_norm_op  = select(0.0, base_opacity, (params.chan_normal == 1u) || (params.chan_height == 1u));
    let eff_height_op = select(0.0, base_opacity, params.chan_height == 1u);

    var blended_color = bg_color.rgb;
    if eff_color_op > 0.0 {
        if params.blend_mode == 0u { // Normal
            blended_color = mix(bg_color.rgb, src_color.rgb, eff_color_op);
        } else if params.blend_mode == 1u { // Multiply
            blended_color = mix(bg_color.rgb, bg_color.rgb * src_color.rgb, eff_color_op);
        } else if params.blend_mode == 2u { // Add
            blended_color = bg_color.rgb + (src_color.rgb * eff_color_op);
        } else if params.blend_mode == 3u { // Subtract
            blended_color = max(vec3<f32>(0.0), bg_color.rgb - (src_color.rgb * eff_color_op));
        }
    }

    let out_normal_rgb = mix(bg_normal.rgb, src_normal.rgb, eff_norm_op);
    let out_height = mix(bg_normal.a, src_normal.a, eff_height_op);

    var out: FragmentColorOutput;
    out.color = vec4<f32>(blended_color, 1.0);
    out.normal = vec4<f32>(out_normal_rgb, out_height);
    return out;
}

struct FragmentScalarOutput {
    @location(0) roughness: f32,
    @location(1) metallic: f32,
};

@fragment
fn fs_scalar(in: VertexOutput) -> FragmentScalarOutput {
    let bg_roughness = textureSample(tex_bg_roughness, samp_nearest, in.uv).r;
    let src_roughness = textureSample(tex_src_roughness, samp_nearest, in.uv).r;

    let bg_metallic = textureSample(tex_bg_metallic, samp_nearest, in.uv).r;
    let src_metallic = textureSample(tex_src_metallic, samp_nearest, in.uv).r;

    let stroke_alpha = textureSample(tex_src_color, samp_nearest, in.uv).a;
    let base_opacity = get_effective_opacity(in.uv) * stroke_alpha;
    let eff_rough_op = select(0.0, base_opacity, params.chan_roughness == 1u);
    let eff_metal_op = select(0.0, base_opacity, params.chan_metallic == 1u);

    var out: FragmentScalarOutput;
    out.roughness = mix(bg_roughness, src_roughness, eff_rough_op);
    out.metallic = mix(bg_metallic, src_metallic, eff_metal_op);
    return out;
}
