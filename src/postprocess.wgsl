// 3D Paintcraft - Substance 3D Painter Post-Processing & Screen Space Ray Tracing
// Features: Screen-Space Ray Traced Reflections (SSR), FXAA Anti-Aliasing,
// Bloom / Specular Glow, Depth of Field (DoF), Chromatic Aberration, and Vignette.

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct PostProcessUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    eye_pos: vec4<f32>,
    screen_size: vec2<f32>,
    fxaa_enabled: u32,
    bloom_enabled: u32,
    bloom_intensity: f32,
    bloom_threshold: f32,
    bloom_radius: f32,
    dof_enabled: u32,
    dof_focus_dist: f32,
    dof_aperture: f32,
    dof_max_blur: f32,
    chromatic_aberration: f32,
    vignette_strength: f32,
    near_z: f32,
    far_z: f32,
    ssr_enabled: u32,
    ssr_intensity: f32,
    ssr_max_roughness: f32,
    ssr_step_size: f32,
    ssr_max_steps: u32,
    view_mode: u32,
    ssao_enabled: u32,
    ssao_radius: f32,
    ssao_intensity: f32,
    aa_mode: u32,
    denoiser_enabled: u32,
    denoiser_radius: f32,
    denoiser_normal_weight: f32,
    denoiser_depth_weight: f32,
    history_blend_weight: f32,
    jitter_offset: vec2<f32>,
};

@group(0) @binding(0) var<uniform> params: PostProcessUniform;
@group(0) @binding(1) var samp_linear: sampler;
@group(0) @binding(2) var tex_scene_color: texture_2d<f32>;
@group(0) @binding(3) var tex_scene_depth: texture_depth_2d;
@group(0) @binding(4) var tex_scene_normal: texture_2d<f32>;
@group(0) @binding(5) var tex_history: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vertex_index == 1u) * 4 - 1);
    let y = f32(i32(vertex_index == 2u) * 4 - 1);
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

fn rgb_to_luma(col: vec3<f32>) -> f32 {
    return dot(col, vec3<f32>(0.299, 0.587, 0.114));
}

fn linearize_depth(raw_depth: f32) -> f32 {
    if raw_depth >= 0.9999 {
        return 1000.0;
    }
    return (params.near_z * params.far_z) / (params.far_z - raw_depth * (params.far_z - params.near_z));
}

fn oct_wrap(v: vec2<f32>) -> vec2<f32> {
    return (1.0 - abs(v.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), v >= vec2<f32>(0.0));
}

fn unpack_normal_oct(e: vec2<f32>) -> vec3<f32> {
    let p = e * 2.0 - 1.0;
    let pz = 1.0 - abs(p.x) - abs(p.y);
    let t = clamp(-pz, 0.0, 1.0);
    let nx = select(p.x + t, p.x - t, p.x >= 0.0);
    let ny = select(p.y + t, p.y - t, p.y >= 0.0);
    return normalize(vec3<f32>(nx, ny, pz));
}

// Screen-Space Ray Traced Reflections (SSR)
fn raytrace_ssr(uv: vec2<f32>, base_col: vec3<f32>) -> vec3<f32> {
    if params.ssr_enabled == 0u || params.view_mode != 0u {
        return base_col;
    }

    let pixel_coord = vec2<i32>(clamp(uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
    let raw_depth = textureLoad(tex_scene_depth, pixel_coord, 0);
    if raw_depth >= 0.9999 {
        return base_col;
    }

    let norm_rough = textureLoad(tex_scene_normal, pixel_coord, 0);
    let roughness = norm_rough.w;
    if roughness > params.ssr_max_roughness {
        return base_col;
    }

    let metallic = norm_rough.z;
    let normal = unpack_normal_oct(norm_rough.xy);
    if length(normal) < 0.1 {
        return base_col;
    }

    // Reconstruct 3D world position
    let ndc_x = uv.x * 2.0 - 1.0;
    let ndc_y = 1.0 - uv.y * 2.0;
    let clip_pos = vec4<f32>(ndc_x, ndc_y, raw_depth, 1.0);
    let world_h = params.inv_view_proj * clip_pos;
    let world_pos = world_h.xyz / max(world_h.w, 1e-4);

    let view_dir = normalize(world_pos - params.eye_pos.xyz);
    let reflect_dir = normalize(reflect(view_dir, normal));

    let step_size = params.ssr_step_size;
    var ray_pos = world_pos + normal * 0.02;
    var hit_found = false;
    var hit_uv = vec2<f32>(0.0);

    let max_steps = params.ssr_max_steps;
    for (var i = 0u; i < max_steps; i = i + 1u) {
        let step_stride = step_size * (1.0 + f32(i) * 0.07);
        ray_pos = ray_pos + reflect_dir * step_stride;

        let ray_clip = params.view_proj * vec4<f32>(ray_pos, 1.0);
        if ray_clip.w <= 0.001 {
            break;
        }

        let ray_ndc = ray_clip.xyz / ray_clip.w;
        let s_uv = vec2<f32>((ray_ndc.x + 1.0) * 0.5, (1.0 - ray_ndc.y) * 0.5);

        if s_uv.x < 0.0 || s_uv.x > 1.0 || s_uv.y < 0.0 || s_uv.y > 1.0 || ray_ndc.z < 0.0 || ray_ndc.z > 1.0 {
            break;
        }

        let s_coord = vec2<i32>(clamp(s_uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
        let sample_depth = textureLoad(tex_scene_depth, s_coord, 0);

        // Never hit infinite background or skybox
        if sample_depth >= 0.9999 {
            continue;
        }

        let depth_diff = ray_ndc.z - sample_depth;
        if depth_diff > 0.0001 && depth_diff < 0.04 {
            let hit_norm_data = textureLoad(tex_scene_normal, s_coord, 0);
            let hit_normal = unpack_normal_oct(hit_norm_data.xy);

            // Hit surface must face towards the incoming ray
            if dot(hit_normal, reflect_dir) < -0.05 {
                var refined_pos = ray_pos;
                var min_pos = ray_pos - reflect_dir * step_stride;
                var max_pos = ray_pos;
                for (var b = 0u; b < 3u; b = b + 1u) {
                    let mid_pos = (min_pos + max_pos) * 0.5;
                    let mid_clip = params.view_proj * vec4<f32>(mid_pos, 1.0);
                    let mid_ndc = mid_clip.xyz / max(mid_clip.w, 0.001);
                    let mid_uv = vec2<f32>((mid_ndc.x + 1.0) * 0.5, (1.0 - mid_ndc.y) * 0.5);
                    let mid_coord = vec2<i32>(clamp(mid_uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
                    let mid_depth = textureLoad(tex_scene_depth, mid_coord, 0);
                    if mid_depth < 0.9999 && mid_ndc.z > mid_depth {
                        max_pos = mid_pos;
                        refined_pos = mid_pos;
                    } else {
                        min_pos = mid_pos;
                    }
                }

                let final_clip = params.view_proj * vec4<f32>(refined_pos, 1.0);
                let final_ndc = final_clip.xyz / max(final_clip.w, 0.001);
                hit_uv = vec2<f32>((final_ndc.x + 1.0) * 0.5, (1.0 - final_ndc.y) * 0.5);
                hit_found = true;
                break;
            }
        }
    }

    if hit_found {
        let edge_fade_x = smoothstep(0.0, 0.08, hit_uv.x) * smoothstep(1.0, 0.92, hit_uv.x);
        let edge_fade_y = smoothstep(0.0, 0.08, hit_uv.y) * smoothstep(1.0, 0.92, hit_uv.y);
        let edge_fade = edge_fade_x * edge_fade_y;

        let refl_color = textureSample(tex_scene_color, samp_linear, hit_uv).rgb;
        let gloss = 1.0 - roughness;

        // Fresnel reflection factor
        let n_dot_v = max(dot(normal, -view_dir), 0.0);
        let dielectric_f0 = 0.04;
        let f0_val = mix(dielectric_f0, 1.0, metallic);
        let fresnel = f0_val + (1.0 - f0_val) * pow(clamp(1.0 - n_dot_v, 0.0, 1.0), 5.0);

        // Tint reflection by surface chromaticity for metals
        let max_chan = max(max(base_col.r, max(base_col.g, base_col.b)), 0.02);
        let metal_chroma = clamp(base_col / max_chan, vec3<f32>(0.0), vec3<f32>(1.0));
        let tint = mix(vec3<f32>(1.0), metal_chroma, metallic);

        // On dielectrics: SSR is a physical specular reflection added to the surface
        // On metals: SSR smoothly replaces distant IBL
        let refl_term = refl_color * tint * fresnel * edge_fade * gloss * params.ssr_intensity;
        let blended = select(
            base_col + refl_term,
            mix(base_col, refl_color * tint, clamp(edge_fade * gloss * fresnel * params.ssr_intensity, 0.0, 0.85)),
            metallic > 0.5
        );
        return clamp(blended, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    return base_col;
}

// Fast Approximate Anti-Aliasing (FXAA)
fn sample_fxaa(uv: vec2<f32>) -> vec3<f32> {
    let inv_screen = 1.0 / params.screen_size;
    let m = textureSample(tex_scene_color, samp_linear, uv).rgb;
    let l_m = rgb_to_luma(m);

    let l_n = rgb_to_luma(textureSample(tex_scene_color, samp_linear, uv + vec2<f32>(0.0, -inv_screen.y)).rgb);
    let l_s = rgb_to_luma(textureSample(tex_scene_color, samp_linear, uv + vec2<f32>(0.0, inv_screen.y)).rgb);
    let l_e = rgb_to_luma(textureSample(tex_scene_color, samp_linear, uv + vec2<f32>(inv_screen.x, 0.0)).rgb);
    let l_w = rgb_to_luma(textureSample(tex_scene_color, samp_linear, uv + vec2<f32>(-inv_screen.x, 0.0)).rgb);

    let l_min = min(l_m, min(min(l_n, l_s), min(l_e, l_w)));
    let l_max = max(l_m, max(max(l_n, l_s), max(l_e, l_w)));
    let l_range = l_max - l_min;

    if l_range < max(0.04, l_max * 0.125) {
        return m;
    }

    let l_nw = rgb_to_luma(textureSample(tex_scene_color, samp_linear, uv + vec2<f32>(-inv_screen.x, -inv_screen.y)).rgb);
    let l_ne = rgb_to_luma(textureSample(tex_scene_color, samp_linear, uv + vec2<f32>(inv_screen.x, -inv_screen.y)).rgb);
    let l_sw = rgb_to_luma(textureSample(tex_scene_color, samp_linear, uv + vec2<f32>(-inv_screen.x, inv_screen.y)).rgb);
    let l_se = rgb_to_luma(textureSample(tex_scene_color, samp_linear, uv + vec2<f32>(inv_screen.x, inv_screen.y)).rgb);

    let edge_h = abs(-2.0 * l_m + l_n + l_s) * 2.0 + abs(-2.0 * l_w + l_nw + l_sw) + abs(-2.0 * l_e + l_ne + l_se);
    let edge_v = abs(-2.0 * l_m + l_e + l_w) * 2.0 + abs(-2.0 * l_n + l_ne + l_nw) + abs(-2.0 * l_s + l_se + l_sw);
    let is_horizontal = edge_h >= edge_v;

    var step_dir = vec2<f32>(0.0, inv_screen.y);
    if !is_horizontal {
        step_dir = vec2<f32>(inv_screen.x, 0.0);
    }

    let sample_pos = textureSample(tex_scene_color, samp_linear, uv + step_dir * 0.5).rgb;
    let sample_neg = textureSample(tex_scene_color, samp_linear, uv - step_dir * 0.5).rgb;
    let blend_col = (sample_pos + sample_neg) * 0.5;

    let l_blend = rgb_to_luma(blend_col);
    if l_blend < l_min || l_blend > l_max {
        return (sample_pos + sample_neg + m * 2.0) * 0.25;
    }

    return blend_col;
}

// NVIDIA Deep Learning Anti-Aliasing (DLAA) Temporal AI Reconstruction Filter
// Reconstructs subpixel edges and specular highlights with 3x3 variance color clipping
fn sample_dlaa(uv: vec2<f32>) -> vec3<f32> {
    let inv_screen = 1.0 / params.screen_size;
    let jitter = params.jitter_offset;
    
    // Sample current frame at unjittered location
    let current_uv = clamp(uv - jitter, vec2<f32>(0.0), vec2<f32>(1.0));
    let current_col = textureSample(tex_scene_color, samp_linear, current_uv).rgb;
    
    // 3x3 neighborhood statistics for variance color clipping (eliminates temporal ghosting)
    var m1 = current_col;
    var m2 = current_col * current_col;
    var min_col = current_col;
    var max_col = current_col;
    
    var offsets_3x3 = array<vec2<f32>, 8>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 0.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  0.0),
        vec2<f32>( 1.0,  0.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 0.0,  1.0),
        vec2<f32>( 1.0,  1.0),
    );
    
    for (var i = 0u; i < 8u; i = i + 1u) {
        let n_uv = clamp(current_uv + offsets_3x3[i] * inv_screen, vec2<f32>(0.0), vec2<f32>(1.0));
        let n_col = textureSample(tex_scene_color, samp_linear, n_uv).rgb;
        min_col = min(min_col, n_col);
        max_col = max(max_col, n_col);
        m1 = m1 + n_col;
        m2 = m2 + n_col * n_col;
    }
    
    let mean = m1 / 9.0;
    let variance = max(m2 / 9.0 - mean * mean, vec3<f32>(0.0));
    let sigma = sqrt(variance);
    
    // Tight variance bounding box (1.25 sigma)
    let box_min = max(min_col, mean - 1.25 * sigma);
    let box_max = min(max_col, mean + 1.25 * sigma);
    
    // Sample temporal history buffer
    let history_col = textureSample(tex_history, samp_linear, uv).rgb;
    let clamped_history = clamp(history_col, box_min, box_max);
    
    // Blend with high temporal weight (default 90% history, 10% current)
    let blend_w = clamp(params.history_blend_weight, 0.5, 0.95);
    return mix(current_col, clamped_history, blend_w);
}

// Joint-Bilateral Edge-Preserving Denoiser (Guided by Screen-Space Normals & Depth)
// Removes ray-marched stochastic noise from SSR reflections and SSAO crevices
fn denoise_joint_bilateral(uv: vec2<f32>, center_col: vec3<f32>) -> vec3<f32> {
    if params.denoiser_enabled == 0u {
        return center_col;
    }

    let pixel_coord = vec2<i32>(clamp(uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
    let center_raw_depth = textureLoad(tex_scene_depth, pixel_coord, 0);
    if center_raw_depth >= 0.9999 {
        return center_col;
    }

    let center_norm_data = textureLoad(tex_scene_normal, pixel_coord, 0);
    let center_norm = unpack_normal_oct(center_norm_data.xy);
    let center_z = linearize_depth(center_raw_depth);

    let inv_screen = 1.0 / params.screen_size;
    let radius = max(params.denoiser_radius, 1.0);
    let step_uv = inv_screen * radius;

    // Cross / Diamond 8-tap bilateral kernel with distance falloff
    var offsets = array<vec2<f32>, 8>(
        vec2<f32>(-1.0,  0.0),
        vec2<f32>( 1.0,  0.0),
        vec2<f32>( 0.0, -1.0),
        vec2<f32>( 0.0,  1.0),
        vec2<f32>(-0.707, -0.707),
        vec2<f32>( 0.707, -0.707),
        vec2<f32>(-0.707,  0.707),
        vec2<f32>( 0.707,  0.707),
    );

    var total_col = center_col * 2.0;
    var total_weight = 2.0;

    let normal_exp = max(params.denoiser_normal_weight, 1.0);
    let depth_sens = max(params.denoiser_depth_weight, 0.01);

    for (var i = 0u; i < 8u; i = i + 1u) {
        let sample_uv = clamp(uv + offsets[i] * step_uv, vec2<f32>(0.0), vec2<f32>(1.0));
        let sample_px = vec2<i32>(clamp(sample_uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
        let sample_raw_depth = textureLoad(tex_scene_depth, sample_px, 0);
        if sample_raw_depth >= 0.9999 {
            continue;
        }

        let sample_norm_data = textureLoad(tex_scene_normal, sample_px, 0);
        let sample_norm = unpack_normal_oct(sample_norm_data.xy);
        let sample_z = linearize_depth(sample_raw_depth);

        // Normal alignment weight (preserves sharp geometric edges and creases)
        let n_dot = max(dot(center_norm, sample_norm), 0.0);
        let w_normal = pow(n_dot, normal_exp);

        // Depth distance weight (preserves silhouettes and foreground-background separation)
        let depth_diff = abs(center_z - sample_z);
        let w_depth = exp(-depth_diff / depth_sens);

        let s_col = textureSample(tex_scene_color, samp_linear, sample_uv).rgb;
        // Color difference / photometric weight (strictly preserves painted brush strokes, decals, and texture details)
        let col_diff = length(center_col - s_col);
        let w_color = exp(-(col_diff * col_diff) / 0.03);

        let weight = w_normal * w_depth * w_color;
        if weight > 0.001 {
            total_col = total_col + s_col * weight;
            total_weight = total_weight + weight;
        }
    }

    return total_col / total_weight;
}

// Screen-Space Ambient Occlusion (SSAO) with 16 Fermat Spiral Poisson samples & Interleaved Gradient Noise
fn interleaved_gradient_noise(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

fn compute_ssao(uv: vec2<f32>) -> f32 {
    if params.ssao_enabled == 0u || params.view_mode != 0u {
        return 1.0;
    }

    let pixel_coord = vec2<i32>(clamp(uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
    let raw_depth = textureLoad(tex_scene_depth, pixel_coord, 0);
    if raw_depth >= 0.9999 {
        return 1.0;
    }

    let lin_depth = linearize_depth(raw_depth);
    let norm_rough = textureLoad(tex_scene_normal, pixel_coord, 0);
    let normal = unpack_normal_oct(norm_rough.xy);

    let inv_screen = 1.0 / params.screen_size;
    let radius = clamp(params.ssao_radius / max(lin_depth, 0.35), 0.002, 0.08);

    // Jimenez Interleaved Gradient Noise rotation angle
    let noise = interleaved_gradient_noise(vec2<f32>(pixel_coord));
    let rot_angle = noise * 6.2831853;
    let rot_cos = cos(rot_angle);
    let rot_sin = sin(rot_angle);

    var occlusion = 0.0;
    let sample_count = 16.0;

    for (var i = 0u; i < 16u; i = i + 1u) {
        // Golden ratio Fermat spiral
        let alpha = (f32(i) + 0.5) / sample_count;
        let theta = f32(i) * 2.3999632;
        let r = sqrt(alpha);

        let unrotated = vec2<f32>(cos(theta), sin(theta)) * r;
        let dir = vec2<f32>(
            unrotated.x * rot_cos - unrotated.y * rot_sin,
            unrotated.x * rot_sin + unrotated.y * rot_cos
        );

        let s_uv = clamp(uv + dir * inv_screen * radius * params.screen_size.y * 0.4, vec2<f32>(0.0), vec2<f32>(1.0));
        let s_coord = vec2<i32>(clamp(s_uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
        let s_raw_depth = textureLoad(tex_scene_depth, s_coord, 0);
        let s_depth = linearize_depth(s_raw_depth);

        let depth_diff = lin_depth - s_depth;
        if depth_diff > 0.002 && depth_diff < 0.35 {
            let s_norm = unpack_normal_oct(textureLoad(tex_scene_normal, s_coord, 0).xy);
            let angle_factor = max(dot(normal, s_norm), 0.0);
            // Smoothly curved convex surfaces (like spheres) must not self-occlude
            let is_smooth_neighbor = angle_factor > 0.75 && depth_diff < 0.08;
            if !is_smooth_neighbor {
                let dist_falloff = 1.0 - smoothstep(0.002, 0.35, depth_diff);
                let crevice_factor = clamp(1.0 - angle_factor, 0.0, 1.0);
                occlusion = occlusion + dist_falloff * crevice_factor;
            }
        }
    }

    let raw_ao = clamp(1.0 - (occlusion / sample_count) * params.ssao_intensity * 2.0, 0.0, 1.0);

    // Cross-Bilateral depth & normal aware edge-preserving filter (smooths dither while preserving crevice silhouettes)
    var total_ao = raw_ao;
    var total_weight = 1.0;
    let blur_radius = inv_screen * 1.5;

    // Cross tap: +X
    {
        let b_uv = clamp(uv + vec2<f32>(1.0, 0.0) * blur_radius, vec2<f32>(0.0), vec2<f32>(1.0));
        let b_coord = vec2<i32>(clamp(b_uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
        let b_raw_depth = textureLoad(tex_scene_depth, b_coord, 0);
        if b_raw_depth < 0.9999 {
            let b_depth = linearize_depth(b_raw_depth);
            let b_norm = unpack_normal_oct(textureLoad(tex_scene_normal, b_coord, 0).xy);
            let depth_diff = abs(lin_depth - b_depth);
            let norm_dot = max(dot(normal, b_norm), 0.0);
            if depth_diff < 0.05 && norm_dot > 0.6 {
                let weight = norm_dot * (1.0 - depth_diff * 20.0);
                total_ao = total_ao + raw_ao * weight;
                total_weight = total_weight + weight;
            }
        }
    }
    // Cross tap: -X
    {
        let b_uv = clamp(uv + vec2<f32>(-1.0, 0.0) * blur_radius, vec2<f32>(0.0), vec2<f32>(1.0));
        let b_coord = vec2<i32>(clamp(b_uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
        let b_raw_depth = textureLoad(tex_scene_depth, b_coord, 0);
        if b_raw_depth < 0.9999 {
            let b_depth = linearize_depth(b_raw_depth);
            let b_norm = unpack_normal_oct(textureLoad(tex_scene_normal, b_coord, 0).xy);
            let depth_diff = abs(lin_depth - b_depth);
            let norm_dot = max(dot(normal, b_norm), 0.0);
            if depth_diff < 0.05 && norm_dot > 0.6 {
                let weight = norm_dot * (1.0 - depth_diff * 20.0);
                total_ao = total_ao + raw_ao * weight;
                total_weight = total_weight + weight;
            }
        }
    }
    // Cross tap: +Y
    {
        let b_uv = clamp(uv + vec2<f32>(0.0, 1.0) * blur_radius, vec2<f32>(0.0), vec2<f32>(1.0));
        let b_coord = vec2<i32>(clamp(b_uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
        let b_raw_depth = textureLoad(tex_scene_depth, b_coord, 0);
        if b_raw_depth < 0.9999 {
            let b_depth = linearize_depth(b_raw_depth);
            let b_norm = unpack_normal_oct(textureLoad(tex_scene_normal, b_coord, 0).xy);
            let depth_diff = abs(lin_depth - b_depth);
            let norm_dot = max(dot(normal, b_norm), 0.0);
            if depth_diff < 0.05 && norm_dot > 0.6 {
                let weight = norm_dot * (1.0 - depth_diff * 20.0);
                total_ao = total_ao + raw_ao * weight;
                total_weight = total_weight + weight;
            }
        }
    }
    // Cross tap: -Y
    {
        let b_uv = clamp(uv + vec2<f32>(0.0, -1.0) * blur_radius, vec2<f32>(0.0), vec2<f32>(1.0));
        let b_coord = vec2<i32>(clamp(b_uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
        let b_raw_depth = textureLoad(tex_scene_depth, b_coord, 0);
        if b_raw_depth < 0.9999 {
            let b_depth = linearize_depth(b_raw_depth);
            let b_norm = unpack_normal_oct(textureLoad(tex_scene_normal, b_coord, 0).xy);
            let depth_diff = abs(lin_depth - b_depth);
            let norm_dot = max(dot(normal, b_norm), 0.0);
            if depth_diff < 0.05 && norm_dot > 0.6 {
                let weight = norm_dot * (1.0 - depth_diff * 20.0);
                total_ao = total_ao + raw_ao * weight;
                total_weight = total_weight + weight;
            }
        }
    }

    return total_ao / max(total_weight, 0.01);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let inv_screen = 1.0 / params.screen_size;

    // 1. Anti-Aliasing (Off / FXAA / NVIDIA DLAA)
    var base_color = vec3<f32>(0.0);
    if params.aa_mode == 2u {
        base_color = sample_dlaa(uv);
    } else if params.aa_mode == 1u || params.fxaa_enabled == 1u {
        base_color = sample_fxaa(uv);
    } else {
        base_color = textureSample(tex_scene_color, samp_linear, uv).rgb;
    }

    // 2. High-Quality Screen-Space Ambient Occlusion (Smooth crevice contact shadows)
    let ssao_val = compute_ssao(uv);
    base_color = base_color * ssao_val;

    // 3. Screen-Space Reflections (SSR)
    if params.ssr_enabled == 1u {
        base_color = raytrace_ssr(uv, base_color);
    }

    // 4. Edge-Preserving Normal/Depth Guided Joint Bilateral Denoiser
    if params.denoiser_enabled == 1u {
        base_color = denoise_joint_bilateral(uv, base_color);
    }

    // 3. Depth of Field (DoF) with Bokeh / CoC
    if params.dof_enabled == 1u {
        let pixel_coord = vec2<i32>(clamp(uv * params.screen_size, vec2<f32>(0.0), params.screen_size - vec2<f32>(1.0)));
        let raw_depth = textureLoad(tex_scene_depth, pixel_coord, 0);
        let linear_z = linearize_depth(raw_depth);

        let coc = clamp(abs(linear_z - params.dof_focus_dist) / max(params.dof_aperture, 0.01), 0.0, 1.0) * params.dof_max_blur;
        if coc > 0.001 {
            var dof_acc = base_color;
            var dof_wt = 1.0;

            var dof_offsets = array<vec2<f32>, 8>(
                vec2<f32>(0.0, 1.0),
                vec2<f32>(0.707, 0.707),
                vec2<f32>(1.0, 0.0),
                vec2<f32>(0.707, -0.707),
                vec2<f32>(0.0, -1.0),
                vec2<f32>(-0.707, -0.707),
                vec2<f32>(-1.0, 0.0),
                vec2<f32>(-0.707, 0.707),
            );

            let blur_step = inv_screen * coc * 8.0;
            for (var i = 0u; i < 8u; i = i + 1u) {
                let s_uv = clamp(uv + dof_offsets[i] * blur_step, vec2<f32>(0.0), vec2<f32>(1.0));
                let s_col = textureSample(tex_scene_color, samp_linear, s_uv).rgb;
                dof_acc = dof_acc + s_col;
                dof_wt = dof_wt + 1.0;
            }

            base_color = dof_acc / dof_wt;
        }
    }

    // 4. Bloom / Specular Highlight Glow
    if params.bloom_enabled == 1u {
        var bloom_acc = vec3<f32>(0.0);
        let bloom_step = inv_screen * max(params.bloom_radius, 1.0) * 3.0;

        var bloom_offsets = array<vec2<f32>, 8>(
            vec2<f32>(-1.0, -1.0),
            vec2<f32>( 0.0, -1.414),
            vec2<f32>( 1.0, -1.0),
            vec2<f32>(-1.414, 0.0),
            vec2<f32>( 1.414, 0.0),
            vec2<f32>(-1.0,  1.0),
            vec2<f32>( 0.0,  1.414),
            vec2<f32>( 1.0,  1.0),
        );

        for (var i = 0u; i < 8u; i = i + 1u) {
            let s_uv = clamp(uv + bloom_offsets[i] * bloom_step, vec2<f32>(0.0), vec2<f32>(1.0));
            let s_col = textureSample(tex_scene_color, samp_linear, s_uv).rgb;
            let lum = rgb_to_luma(s_col);
            let excess = max(lum - params.bloom_threshold, 0.0);
            let factor = excess / max(lum, 0.001);
            bloom_acc = bloom_acc + s_col * factor;
        }

        let bloom_color = (bloom_acc / 8.0) * params.bloom_intensity;
        base_color = base_color + bloom_color;
    }

    // 5. Chromatic Aberration
    if params.chromatic_aberration > 0.001 {
        let center_offset = uv - vec2<f32>(0.5);
        let dist = length(center_offset);
        let dir = normalize(center_offset);
        let shift = dir * (dist * params.chromatic_aberration * 0.015);

        let col_r = textureSample(tex_scene_color, samp_linear, clamp(uv + shift, vec2<f32>(0.0), vec2<f32>(1.0))).r;
        let col_b = textureSample(tex_scene_color, samp_linear, clamp(uv - shift, vec2<f32>(0.0), vec2<f32>(1.0))).b;
        base_color.r = mix(base_color.r, col_r, 0.6);
        base_color.b = mix(base_color.b, col_b, 0.6);
    }

    // 6. Lens Vignette
    if params.vignette_strength > 0.001 {
        let center_offset = uv - vec2<f32>(0.5);
        let d = length(center_offset) * 1.414;
        let vignette = clamp(1.0 - d * d * params.vignette_strength * 0.7, 0.0, 1.0);
        base_color = base_color * vignette;
    }

    return vec4<f32>(clamp(base_color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
