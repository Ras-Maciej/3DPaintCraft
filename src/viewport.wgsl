// 3D Paintcraft - Substance 3D Painter Metallic/Roughness PBR Shader (WGSL)

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
};

struct CameraUniform {
    view_proj: mat4x4<f32>,
    eye_pos: vec4<f32>,
    env_rot: f32,
    view_mode: u32,
    bg_blur: f32,
    bg_opacity: f32,
    inv_view_proj: mat4x4<f32>,
    sun_intensity: f32,
    env_intensity: f32,
    ao_intensity: f32,
    exposure: f32,
    normal_format: u32,
    tonemap_mode: u32,
    specular_mult: f32,
    roughness_bias: f32,
    alpha_cutoff: f32,
    transparency_mode: u32,
    double_sided: u32,
    ior: f32,
    refraction_dispersion: f32,
    translucency_depth: f32,
    pad0: f32,
    pad1: f32,
    light_view_proj: mat4x4<f32>,
    shadows_enabled: u32,
    shadow_bias: f32,
    shadow_softness: f32,
    wireframe_opacity: f32,
    wireframe_color: vec4<f32>,
    cage_color: vec4<f32>,
    cage_opacity: f32,
    pad_cage0: f32,
    pad_cage1: f32,
    pad_cage2: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

// Substance 3D Painter Surface Parameters
@group(1) @binding(0) var tex_base_color: texture_2d<f32>;
@group(1) @binding(1) var tex_normal: texture_2d<f32>;
@group(1) @binding(2) var tex_roughness: texture_2d<f32>;
@group(1) @binding(3) var tex_metallic: texture_2d<f32>;
@group(1) @binding(4) var samp_linear: sampler;
@group(1) @binding(5) var tex_environment: texture_2d<f32>; // HDRI
@group(1) @binding(6) var tex_baked_normal: texture_2d<f32>;
@group(1) @binding(7) var tex_baked_ao: texture_2d<f32>;
@group(1) @binding(8) var tex_emissive: texture_2d<f32>;
@group(1) @binding(9) var tex_opacity: texture_2d<f32>;
@group(1) @binding(10) var tex_clearcoat: texture_2d<f32>;
@group(1) @binding(11) var tex_anisotropy: texture_2d<f32>;

// Directional Shadow Map (Separated from camera buffer to prevent depth-stencil write hazard)
@group(2) @binding(0) var samp_shadow: sampler_comparison;
@group(2) @binding(1) var tex_shadow_map: texture_depth_2d;

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    out.world_pos = model.position;
    out.world_normal = model.normal;
    out.world_tangent = model.tangent;
    out.uv = model.uv; 
    return out;
}

const PI: f32 = 3.14159265359;

fn ndf_ggx(n_dot_h: f32, alpha2: f32) -> f32 {
    let denom = (n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0);
    return alpha2 / (PI * denom * denom);
}

fn ndf_ggx_aniso(t_dot_h: f32, b_dot_h: f32, n_dot_h: f32, ax: f32, ay: f32) -> f32 {
    let term_x = t_dot_h / max(ax, 0.001);
    let term_y = b_dot_h / max(ay, 0.001);
    let d = term_x * term_x + term_y * term_y + n_dot_h * n_dot_h;
    return 1.0 / (PI * ax * ay * max(d * d, 1e-7));
}

// Lazarov / Karis Split-Sum Environment BRDF Polynomial Approximation
fn env_brdf_approx(f0: vec3<f32>, roughness: f32, n_dot_v: f32) -> vec3<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    let fab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * max(fab.x, 0.0) + vec3<f32>(max(fab.y, 0.0));
}

// Convert 3D direction to equirectangular 2D UV
fn dir_to_equirectangular(dir: vec3<f32>) -> vec2<f32> {
    let u = atan2(dir.z, dir.x) / (2.0 * PI) + 0.5;
    // WGPU textures have V=0 at the top, so we flip the Y axis mapping
    let v = 0.5 - (asin(dir.y) / PI);
    return vec2<f32>(u, v);
}

fn oct_wrap(v: vec2<f32>) -> vec2<f32> {
    return (1.0 - abs(v.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), v >= vec2<f32>(0.0));
}

fn pack_normal_oct(n: vec3<f32>) -> vec2<f32> {
    let p = n.xy * (1.0 / (abs(n.x) + abs(n.y) + abs(n.z)));
    let oct = select(oct_wrap(p), p, n.z >= 0.0);
    return oct * 0.5 + 0.5;
}

// 8x8 Bayer Ordered Dither Matrix for Screen-Door Order-Independent Transparency
fn bayer_8x8(coord: vec2<u32>) -> f32 {
    let x = coord.x % 8u;
    let y = coord.y % 8u;
    let xor_xy = x ^ y;
    let v = (((xor_xy >> 0u) & 1u) << 5u)
          | (((y >> 0u) & 1u) << 4u)
          | (((xor_xy >> 1u) & 1u) << 3u)
          | (((y >> 1u) & 1u) << 2u)
          | (((xor_xy >> 2u) & 1u) << 1u)
          | (((y >> 2u) & 1u) << 0u);
    return f32(v) / 64.0;
}

struct ShadowVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_shadow(model: VertexInput) -> ShadowVertexOutput {
    var out: ShadowVertexOutput;
    out.clip_position = camera.light_view_proj * vec4<f32>(model.position, 1.0);
    return out;
}

// Directional Shadow Mapping with 16-tap Poisson Disk PCF & Slope-Scaled Normal Offset Bias
fn calculate_directional_shadow(world_pos: vec3<f32>, world_normal: vec3<f32>, light_dir: vec3<f32>) -> f32 {
    if camera.shadows_enabled == 0u {
        return 1.0;
    }

    let n_dot_l = dot(world_normal, light_dir);
    if n_dot_l <= -0.05 {
        return 0.0;
    }

    // Normal offset bias scales with slope to eliminate shadow acne on curved geometry
    let slope_factor = clamp(1.0 - max(n_dot_l, 0.0), 0.0, 1.0);
    let normal_offset = world_normal * (camera.shadow_bias * 0.4 + slope_factor * camera.shadow_bias);
    let biased_world_pos = world_pos + normal_offset;

    let light_clip = camera.light_view_proj * vec4<f32>(biased_world_pos, 1.0);
    if light_clip.w <= 0.0 {
        return 1.0;
    }

    // Transform from NDC [-1, 1] to UV texture coordinates [0, 1]
    let proj_coords = light_clip.xyz / light_clip.w;
    let shadow_uv = vec2<f32>(proj_coords.x * 0.5 + 0.5, 0.5 - proj_coords.y * 0.5);
    let current_depth = proj_coords.z - 0.0005;

    // Out of bounds check: points outside the shadow frustum are unshadowed
    if shadow_uv.x < 0.001 || shadow_uv.x > 0.999 || shadow_uv.y < 0.001 || shadow_uv.y > 0.999 || current_depth > 1.0 || current_depth < 0.0 {
        return 1.0;
    }

    // 16-tap Poisson disk PCF kernel with hardware comparison filtering
    let softness = max(camera.shadow_softness, 0.5);
    let texel = (1.0 / 2048.0) * softness;

    var pcf_samples = array<vec2<f32>, 16>(
        vec2<f32>(-0.942, -0.399),
        vec2<f32>( 0.945, -0.198),
        vec2<f32>(-0.094, -0.929),
        vec2<f32>( 0.344,  0.293),
        vec2<f32>(-0.915,  0.457),
        vec2<f32>(-0.382,  0.276),
        vec2<f32>( 0.974,  0.756),
        vec2<f32>( 0.443, -0.975),
        vec2<f32>( 0.537, -0.473),
        vec2<f32>(-0.264, -0.418),
        vec2<f32>( 0.791,  0.190),
        vec2<f32>(-0.241,  0.997),
        vec2<f32>(-0.814, -0.638),
        vec2<f32>(-0.570, -0.002),
        vec2<f32>( 0.315, -0.014),
        vec2<f32>( 0.199,  0.786),
    );

    var shadow_accum = 0.0;
    for (var i = 0u; i < 16u; i = i + 1u) {
        let sample_uv = shadow_uv + pcf_samples[i] * texel;
        shadow_accum = shadow_accum + textureSampleCompare(tex_shadow_map, samp_shadow, sample_uv, current_depth);
    }

    return shadow_accum / 16.0;
}

// Substance 3D Painter Standard Metallic/Roughness PBR Pipeline

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @location(1) normal_roughness: vec4<f32>,
};

@fragment
fn fs_main(@builtin(front_facing) is_front: bool, in: VertexOutput) -> FragmentOutput {
    let opacity = textureSample(tex_opacity, samp_linear, in.uv).r;

    // View mode 12 is raw opacity inspection map - never discard
    if camera.view_mode != 12u {
        if camera.transparency_mode == 0u {
            // Mode 0: Screen-Door / Bayer 8x8 Ordered Dithering (Order-Independent Semi-Transparency)
            let screen_coord = vec2<u32>(in.clip_position.xy);
            let dither_threshold = bayer_8x8(screen_coord);
            if opacity <= dither_threshold {
                discard;
            }
        } else if camera.transparency_mode == 1u {
            // Mode 1: Substance 3D Painter Alpha Cutout (Alpha Test)
            if opacity < camera.alpha_cutoff {
                discard;
            }
        } else if camera.transparency_mode == 2u {
            // Mode 2: Alpha Blending early-out for fully transparent pixels
            if opacity < 0.005 {
                discard;
            }
        }
        // Mode 3: Opaque (Ignore Opacity channel, render solid)
    }

    let emissive_sample = textureSample(tex_emissive, samp_linear, in.uv);
    let emissive_intensity = max(emissive_sample.a, 1.0);
    let emissive_radiance = pow(max(emissive_sample.rgb, vec3<f32>(0.0)), vec3<f32>(2.2)) * emissive_intensity * 2.5;

    let base_color_srgb = textureSample(tex_base_color, samp_linear, in.uv).rgb;
    let base_color = pow(base_color_srgb, vec3<f32>(2.2));
    
    let norm_sample = textureSample(tex_normal, samp_linear, in.uv);
    var layer_n = norm_sample.rgb * 2.0 - 1.0;
    let center_h = norm_sample.a;

    // Sobel height bump perturbation from composite height in alpha channel
    let tex_dims = vec2<f32>(textureDimensions(tex_normal));
    let inv_dim = 1.0 / max(tex_dims, vec2<f32>(1.0));

    let h_r = textureSample(tex_normal, samp_linear, in.uv + vec2<f32>(inv_dim.x, 0.0)).a;
    let h_l = textureSample(tex_normal, samp_linear, in.uv - vec2<f32>(inv_dim.x, 0.0)).a;
    let h_u = textureSample(tex_normal, samp_linear, in.uv + vec2<f32>(0.0, inv_dim.y)).a;
    let h_d = textureSample(tex_normal, samp_linear, in.uv - vec2<f32>(0.0, inv_dim.y)).a;

    let dh_dx = (h_r - h_l) * 4.0;
    let dh_dy = (h_u - h_d) * 4.0;
    let bump_n = normalize(vec3<f32>(-dh_dx, -dh_dy, 1.0));

    // Reoriented Normal Mapping (RNM) blending between bump_n and layer_n
    let t_bump = bump_n + vec3<f32>(0.0, 0.0, 1.0);
    let u_bump = layer_n * vec3<f32>(-1.0, -1.0, 1.0);
    let nz_bump = select(max(t_bump.z, 0.001), max(abs(t_bump.z), 0.001), t_bump.z <= 0.0);
    layer_n = normalize(t_bump * dot(t_bump, u_bump) / nz_bump - u_bump);

    if camera.normal_format == 1u {
        layer_n.y = -layer_n.y;
    }
    if abs(layer_n.x) < 0.005 { layer_n.x = 0.0; }
    if abs(layer_n.y) < 0.005 { layer_n.y = 0.0; }
    let layer_normal_map = normalize(select(vec3<f32>(0.0, 0.0, 1.0), layer_n, length(layer_n) > 0.01));

    var baked_n = textureSample(tex_baked_normal, samp_linear, in.uv).rgb * 2.0 - 1.0;
    if camera.normal_format == 1u {
        baked_n.y = -baked_n.y;
    }
    if abs(baked_n.x) < 0.005 { baked_n.x = 0.0; }
    if abs(baked_n.y) < 0.005 { baked_n.y = 0.0; }
    let baked_normal_map = normalize(select(vec3<f32>(0.0, 0.0, 1.0), baked_n, length(baked_n) > 0.01));
    
    // RNM / Partial derivative normal blend for correct overlay
    let t_n = baked_normal_map + vec3<f32>(0.0, 0.0, 1.0);
    let u_n = layer_normal_map * vec3<f32>(-1.0, -1.0, 1.0);
    let nz = select(max(t_n.z, 0.001), max(abs(t_n.z), 0.001), t_n.z <= 0.0);
    let normal_map = normalize(t_n * dot(t_n, u_n) / nz - u_n);
    
    let roughness_raw = textureSample(tex_roughness, samp_linear, in.uv).r;
    let roughness = clamp(roughness_raw + camera.roughness_bias, 0.02, 1.0);
    let metallic   = clamp(textureSample(tex_metallic, samp_linear, in.uv).r, 0.0, 1.0);
    let baked_maps = textureSample(tex_baked_ao, samp_linear, in.uv);
    let ao         = pow(max(baked_maps.r, 0.001), max(camera.ao_intensity, 0.01));
    let curvature  = baked_maps.g;
    let thickness  = baked_maps.b;
    let clearcoat  = clamp(textureSample(tex_clearcoat, samp_linear, in.uv).r, 0.0, 1.0);
    let aniso_sample = textureSample(tex_anisotropy, samp_linear, in.uv);
    let anisotropy = clamp(aniso_sample.r, -1.0, 1.0);
    let aniso_angle = aniso_sample.g;

    // TBN Mapping with robust degenerate tangent handling & Gram-Schmidt orthogonalization
    let n_len = length(in.world_normal);
    var n = select(vec3<f32>(0.0, 1.0, 0.0), normalize(in.world_normal), n_len > 1e-4);
    if !is_front {
        n = -n;
    }
    let t_len = length(in.world_tangent.xyz);
    var t = select(vec3<f32>(1.0, 0.0, 0.0), normalize(in.world_tangent.xyz), t_len > 1e-4);
    if t_len <= 1e-4 {
        let up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.y) > 0.9);
        t = normalize(cross(up, n));
    }
    // Gram-Schmidt orthogonalize tangent against vertex normal
    t = normalize(t - n * dot(n, t));
    let tw = select(1.0, sign(in.world_tangent.w), abs(in.world_tangent.w) > 0.01);
    let b = normalize(cross(n, t) * tw);
    let tbn = mat3x3<f32>(t, b, n);

    let world_normal = normalize(tbn * normal_map);

    let specular_weight = 1.0 * camera.specular_mult; 
    
    let v = normalize(camera.eye_pos.xyz - in.world_pos);
    let n_dot_v = max(dot(world_normal, v), 1e-4);

    // Anisotropic tangent frame rotation & parameters
    let theta = aniso_angle * 2.0 * PI;
    let cos_th = cos(theta);
    let sin_th = sin(theta);
    let aniso_t = normalize(t * cos_th + b * sin_th);
    let aniso_b = normalize(cross(world_normal, aniso_t));

    let aniso_abs = abs(anisotropy);
    let aspect = sqrt(max(1.0 - 0.9 * aniso_abs, 0.001));
    var ax = roughness / aspect;
    var ay = roughness * aspect;
    if anisotropy < 0.0 {
        let tmp = ax;
        ax = ay;
        ay = tmp;
    }
    let ax2 = max(ax * ax, 0.001);
    let ay2 = max(ay * ay, 0.001);

    // Substance 3D Painter Standard Metallic/Roughness PBR with Physical Index of Refraction (IOR)
    let ior = max(camera.ior, 1.0);
    let ior_f0 = pow((ior - 1.0) / (ior + 1.0), 2.0);
    let dielectric_f0 = vec3<f32>(ior_f0) * specular_weight;
    let f0 = mix(dielectric_f0, base_color, metallic);
    
    // Dielectrics retain 100% of their base color in diffuse; conductors have 0% diffuse.
    let diffuse_color = base_color * (1.0 - metallic);
    
    // Environment Rotation
    let rot_y = camera.env_rot;
    let cos_y = cos(rot_y);
    let sin_y = sin(rot_y);
    let rot_mat = mat3x3<f32>(
        vec3<f32>(cos_y, 0.0, sin_y),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(-sin_y, 0.0, cos_y)
    );
    
    // Image Based Lighting (HDRI)
    let reflection_dir = reflect(-v, world_normal);
    // Microfacet dominant specular reflection vector bends towards macroscopic surface normal as roughness increases
    let dominant_spec_dir = normalize(mix(reflection_dir, world_normal, roughness * roughness * 0.85));

    let aniso_dir = select(aniso_b, aniso_t, anisotropy >= 0.0);
    let aniso_tangent_refl = cross(cross(aniso_dir, v), aniso_dir);
    let aniso_refl = normalize(mix(dominant_spec_dir, aniso_tangent_refl, aniso_abs * (1.0 - roughness * 0.5)));
    let effective_refl = select(dominant_spec_dir, aniso_refl, aniso_abs > 0.01);
    let rotated_refl = rot_mat * effective_refl;
    let env_uv_spec = dir_to_equirectangular(rotated_refl);
    
    let rotated_norm = rot_mat * world_normal;
    let env_uv_diff = dir_to_equirectangular(rotated_norm);
    
    // Mip levels: roughness 0 -> level 0 (mirror), roughness 1 -> level 9 (diffuse blur)
    let mip_level = clamp(roughness * 9.0, 0.0, 9.0);
    
    let blurred_specular = textureSampleLevel(tex_environment, samp_linear, env_uv_spec, mip_level).rgb;
    let irradiance = textureSampleLevel(tex_environment, samp_linear, env_uv_diff, 8.0).rgb;
    
    // Substance Painter Split-Sum Environment Reflection (tinted by f0 for metals; subtle physical Fresnel with grazing sheen for dielectrics)
    let env_brdf = env_brdf_approx(f0, roughness, n_dot_v);

    // Physical Matte Attenuation:
    // For dielectrics (metallic = 0.0), as roughness approaches 1.0, microfacets completely scatter
    // specular reflections into diffuse scattering. Completely eliminates shiny softbox glare at roughness = 1.0.
    let matte_factor = mix(pow(clamp(1.0 - roughness, 0.0, 1.0), 1.5), 1.0, metallic);

    // Analytical Specular Ambient Occlusion (prevents reflection leaking inside crevices and seams)
    let specular_ao = clamp((n_dot_v + ao) * (n_dot_v + ao) - 1.0 + ao, 0.0, 1.0);
    let ibl_specular = blurred_specular * env_brdf * specular_ao * camera.env_intensity * matte_factor;
    let ibl_diffuse = diffuse_color * irradiance * ao * camera.env_intensity;
    
    // Directional Sun Light (synchronized with HDRI azimuth)
    let sun_local = normalize(vec3<f32>(0.6, 0.7, 0.4));
    let light_dir = normalize(transpose(rot_mat) * sun_local);
    let h = normalize(v + light_dir);
    let n_dot_l = max(dot(world_normal, light_dir), 0.0);
    let n_dot_h = max(dot(world_normal, h), 0.0);
    let v_dot_h = max(dot(v, h), 0.0);
    
    let alpha = max(roughness * roughness, 0.001);
    let alpha2 = alpha * alpha;
    let d_iso = ndf_ggx(n_dot_h, alpha2);
    let t_dot_h = dot(aniso_t, h);
    let b_dot_h = dot(aniso_b, h);
    let d_aniso = ndf_ggx_aniso(t_dot_h, b_dot_h, n_dot_h, ax2, ay2);
    let d = mix(d_iso, d_aniso, aniso_abs);
    
    // Smith GGX direct light shadowing
    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    let gl = n_dot_l / (n_dot_l * (1.0 - k) + k);
    let gv = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let g = gl * gv;
    
    let f90 = mix(vec3<f32>(1.0 - roughness), f0, metallic);
    let f_dir = f0 + (f90 - f0) * pow(clamp(1.0 - v_dot_h, 0.0, 1.0), 5.0);
    let specular_brdf = (d * g * f_dir * specular_weight) / max(4.0 * n_dot_l * n_dot_v, 1e-4);
    
    let shadow_factor = calculate_directional_shadow(in.world_pos, world_normal, light_dir);
    let sun_color = vec3<f32>(1.2, 1.18, 1.15);
    let dir_specular = specular_brdf * sun_color * n_dot_l * camera.sun_intensity * matte_factor * shadow_factor;
    let dir_diffuse = diffuse_color * (sun_color * n_dot_l / PI) * camera.sun_intensity * shadow_factor;

    // Translucency & Physical Refraction (Snell's Law with Chromatic Dispersion & Beer-Lambert Absorption)
    var transmitted_color = vec3<f32>(0.0);
    let transmission = (1.0 - opacity) * (1.0 - metallic);
    if transmission > 0.001 && camera.transparency_mode != 4u {
        let disp = max(camera.refraction_dispersion, 0.0);
        let eta = 1.0 / max(ior, 1.001);
        let eta_r = 1.0 / max(ior - disp, 1.001);
        let eta_g = eta;
        let eta_b = 1.0 / (ior + disp);

        let refr_r = refract(-v, world_normal, eta_r);
        let refr_g = refract(-v, world_normal, eta_g);
        let refr_b = refract(-v, world_normal, eta_b);

        // Fallback to reflection if total internal reflection (TIR) occurs
        let dir_r = select(reflection_dir, normalize(refr_r), length(refr_r) > 0.01);
        let dir_g = select(reflection_dir, normalize(refr_g), length(refr_g) > 0.01);
        let dir_b = select(reflection_dir, normalize(refr_b), length(refr_b) > 0.01);

        let uv_r = dir_to_equirectangular(rot_mat * dir_r);
        let uv_g = dir_to_equirectangular(rot_mat * dir_g);
        let uv_b = dir_to_equirectangular(rot_mat * dir_b);

        let refr_mip = clamp(roughness * 9.0, 0.0, 9.0);
        let t_r = textureSampleLevel(tex_environment, samp_linear, uv_r, refr_mip).r;
        let t_g = textureSampleLevel(tex_environment, samp_linear, uv_g, refr_mip).g;
        let t_b = textureSampleLevel(tex_environment, samp_linear, uv_b, refr_mip).b;
        let transmitted_env = vec3<f32>(t_r, t_g, t_b) * camera.env_intensity;

        // Beer-Lambert physical absorption law: colored medium absorbs transmitted wavelengths
        let absorption = exp(- max(vec3<f32>(1.0) - base_color, vec3<f32>(0.0)) * camera.translucency_depth * 2.5);
        transmitted_color = transmitted_env * absorption;
    }

    // For dielectrics, refracted transmission replaces diffuse scattering as transmission increases
    let total_diffuse = ibl_diffuse + dir_diffuse;
    let final_diffuse = mix(total_diffuse, transmitted_color, transmission);

    // Clearcoat Specular Layer
    var clearcoat_specular = vec3<f32>(0.0);
    if clearcoat > 0.001 {
        let cc_normal = n; // macroscopic surface normal
        let cc_n_dot_l = max(dot(cc_normal, light_dir), 0.0);
        let cc_n_dot_v = max(dot(cc_normal, v), 1e-4);
        let cc_n_dot_h = max(dot(cc_normal, h), 0.0);
        let cc_v_dot_h = max(dot(v, h), 0.0);

        let cc_alpha = 0.08 * 0.08;
        let cc_d = ndf_ggx(cc_n_dot_h, cc_alpha * cc_alpha);
        let cc_f = 0.04 + (1.0 - 0.04) * pow(clamp(1.0 - cc_v_dot_h, 0.0, 1.0), 5.0);
        let cc_v_term = 0.25 / max(cc_v_dot_h * cc_v_dot_h, 1e-4);
        let cc_spec_brdf = cc_d * cc_f * cc_v_term;
        let dir_clearcoat = sun_color * cc_spec_brdf * cc_n_dot_l * camera.sun_intensity;

        let cc_refl = rot_mat * reflect(-v, cc_normal);
        let cc_env_uv = dir_to_equirectangular(cc_refl);
        let cc_ibl_spec = textureSampleLevel(tex_environment, samp_linear, cc_env_uv, 0.5).rgb;
        let cc_env_f = 0.04 + (1.0 - 0.04) * pow(clamp(1.0 - cc_n_dot_v, 0.0, 1.0), 5.0);
        let ibl_clearcoat = cc_ibl_spec * cc_env_f * camera.env_intensity;

        clearcoat_specular = (dir_clearcoat + ibl_clearcoat) * clearcoat;
    }

    let base_lit = ibl_specular + dir_specular + final_diffuse;
    let lit_color = base_lit * (1.0 - clearcoat * 0.04) + clearcoat_specular + emissive_radiance;
    
    // Tone mapping modes
    var final_color = lit_color;
    let exp_val = camera.exposure;
    if camera.tonemap_mode == 0u {
        // Reinhard Tone mapping
        final_color = (lit_color * exp_val) / (lit_color * exp_val + vec3<f32>(1.0));
    } else if camera.tonemap_mode == 1u {
        // ACES Filmic Tone mapping
        let x = lit_color * exp_val;
        let mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
        final_color = clamp(mapped, vec3<f32>(0.0), vec3<f32>(1.0));
    } else if camera.tonemap_mode == 2u {
        // AgX Tone mapping (Blender 4.0 Standard - avoids oversaturation and highlight hue clipping)
        let val_clamped = max(lit_color * exp_val, vec3<f32>(1e-5));
        let val_log = clamp((log2(val_clamped) + 10.0) / 16.5, vec3<f32>(0.0), vec3<f32>(1.0));
        let x2 = val_log * val_log;
        let x3 = x2 * val_log;
        let agx = 15.5 * x3 * x2 - 40.14 * x2 * x2 + 31.96 * x3 - 6.868 * x2 + 0.4298 * val_log + 0.1191;
        final_color = clamp(agx, vec3<f32>(0.0), vec3<f32>(1.0));
    } else {
        // Linear / Unclamped
        final_color = lit_color * exp_val;
    }

    var out: FragmentOutput;
    out.normal_roughness = vec4<f32>(pack_normal_oct(world_normal), metallic, roughness);

    if camera.view_mode == 1u {
        out.color = vec4<f32>(base_color_srgb, 1.0);
        return out;
    } else if camera.view_mode == 2u {
        out.color = vec4<f32>(vec3<f32>(roughness), 1.0);
        return out;
    } else if camera.view_mode == 3u {
        out.color = vec4<f32>(vec3<f32>(metallic), 1.0);
        return out;
    } else if camera.view_mode == 4u {
        out.color = vec4<f32>(normal_map * 0.5 + 0.5, 1.0);
        return out;
    } else if camera.view_mode == 5u {
        out.color = vec4<f32>(vec3<f32>(ao), 1.0);
        return out;
    } else if camera.view_mode == 6u {
        out.color = vec4<f32>(vec3<f32>(curvature), 1.0);
        return out;
    } else if camera.view_mode == 7u {
        // World Position Object / Position map visualization (normalized to [0, 1])
        let norm_pos = in.world_pos * 0.5 + 0.5;
        out.color = vec4<f32>(clamp(norm_pos, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
        return out;
    } else if camera.view_mode == 8u {
        // World Space Normal
        out.color = vec4<f32>(world_normal * 0.5 + 0.5, 1.0);
        return out;
    } else if camera.view_mode == 9u {
        // Real baked thickness map inspection
        out.color = vec4<f32>(vec3<f32>(thickness), 1.0);
        return out;
    } else if camera.view_mode == 10u {
        // Height / Bump channel
        out.color = vec4<f32>(vec3<f32>(center_h), 1.0);
        return out;
    } else if camera.view_mode == 11u {
        // Emissive channel
        out.color = vec4<f32>(emissive_sample.rgb, 1.0);
        return out;
    } else if camera.view_mode == 12u {
        // Opacity channel
        out.color = vec4<f32>(vec3<f32>(opacity), 1.0);
        return out;
    } else if camera.view_mode == 13u {
        // Clearcoat channel
        out.color = vec4<f32>(vec3<f32>(clearcoat), 1.0);
        return out;
    } else if camera.view_mode == 14u {
        // Anisotropy channel: R = strength (-1..1 mapped to 0..1), G = angle (0..1)
        out.color = vec4<f32>(anisotropy * 0.5 + 0.5, aniso_angle, 0.0, 1.0);
        return out;
    }

    var out_alpha = 1.0;
    if camera.transparency_mode == 2u {
        out_alpha = opacity;
    }
    out.color = vec4<f32>(final_color, out_alpha);
    return out;
}

@vertex
fn vs_wire(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    // Depth offset to eliminate z-fighting on line rasterization against triangles
    out.clip_position.z -= 0.0004 * out.clip_position.w;
    out.world_pos = model.position;
    out.world_normal = model.normal;
    out.world_tangent = model.tangent;
    out.uv = model.uv;
    return out;
}

struct WireFragmentOutput {
    @location(0) color: vec4<f32>,
    @location(1) normal_roughness: vec4<f32>,
};

@fragment
fn fs_wire(in: VertexOutput) -> WireFragmentOutput {
    var out: WireFragmentOutput;
    // Technical overlay wireframe line color with configurable color and opacity
    out.color = vec4<f32>(camera.wireframe_color.rgb, camera.wireframe_color.a * camera.wireframe_opacity);
    out.normal_roughness = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    return out;
}

@fragment
fn fs_cage_wire(in: VertexOutput) -> WireFragmentOutput {
    var out: WireFragmentOutput;
    // Glowing Substance cyan/orange cage wireframe and displacement tethers
    out.color = vec4<f32>(camera.cage_color.rgb, camera.cage_color.a * camera.cage_opacity);
    out.normal_roughness = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    return out;
}

@fragment
fn fs_cage_hull(in: VertexOutput) -> WireFragmentOutput {
    var out: WireFragmentOutput;
    let n = normalize(in.world_normal);
    let v = normalize(camera.eye_pos.xyz - in.world_pos);
    let ndotv = abs(dot(n, v));
    // Subtle silhouette rim lighting on translucent cage shell
    let rim = pow(1.0 - clamp(ndotv, 0.0, 1.0), 2.5);
    let alpha = clamp(camera.cage_color.a * camera.cage_opacity * (0.16 + 0.38 * rim), 0.0, 1.0);
    let rgb = camera.cage_color.rgb * (0.85 + 0.35 * rim);
    out.color = vec4<f32>(rgb, alpha);
    out.normal_roughness = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    return out;
}
