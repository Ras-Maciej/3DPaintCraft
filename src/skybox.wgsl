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
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(5) var tex_environment: texture_2d<f32>;
@group(1) @binding(4) var samp_linear: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) view_ray: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let u = f32((in_vertex_index << 1u) & 2u);
    let v = f32(in_vertex_index & 2u);
    
    // NDC coordinates (-1 to 1)
    let x = u * 2.0 - 1.0;
    let y = 1.0 - v * 2.0;
    
    var out: VertexOutput;
    out.clip_pos = vec4<f32>(x, y, 1.0, 1.0); // Z=1.0 puts it exactly at the far clipping plane
    
    let unprojected = camera.inv_view_proj * vec4<f32>(x, y, 1.0, 1.0);
    let world_pos = unprojected.xyz / unprojected.w;
    out.view_ray = world_pos - camera.eye_pos.xyz;
    
    return out;
}

const PI: f32 = 3.14159265359;

fn dir_to_equirectangular(dir: vec3<f32>) -> vec2<f32> {
    let u = atan2(dir.z, dir.x) / (2.0 * PI) + 0.5;
    // WGPU textures have V=0 at the top, so we flip the Y axis mapping
    let v = 0.5 - (asin(dir.y) / PI);
    return vec2<f32>(u, v);
}

struct SkyboxOutput {
    @location(0) color: vec4<f32>,
    @location(1) normal_roughness: vec4<f32>,
};

@fragment
fn fs_main(in: VertexOutput) -> SkyboxOutput {
    let ray_dir = normalize(in.view_ray);
    
    // Apply environment rotation
    let rot_y = camera.env_rot;
    let cos_y = cos(rot_y);
    let sin_y = sin(rot_y);
    let rot_mat = mat3x3<f32>(
        vec3<f32>(cos_y, 0.0, sin_y),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(-sin_y, 0.0, cos_y)
    );
    let rotated_dir = rot_mat * ray_dir;
    
    let env_uv = dir_to_equirectangular(rotated_dir);
    
    var color: vec3<f32>;
    
    if (camera.bg_blur <= 0.005) {
        color = textureSample(tex_environment, samp_linear, env_uv).rgb;
    } else {
        // True spherical Gaussian blur across equirectangular HDRI
        // Latitude metric compensation: adjust horizontal sampling step by 1 / cos(lat)
        let lat = (env_uv.y - 0.5) * 3.14159265359;
        let metric_x = 1.0 / max(cos(lat), 0.15);
        
        let mip_level = camera.bg_blur * 4.5;
        let radius = camera.bg_blur * 0.035;
        
        var total_color = vec3<f32>(0.0);
        var total_weight = 0.0;
        
        // Center tap (weight 0.20)
        let w_center = 0.20;
        total_color += textureSampleLevel(tex_environment, samp_linear, env_uv, mip_level).rgb * w_center;
        total_weight += w_center;
        
        // Ring 1: 8 samples at radius * 0.5 (Gaussian weight ~ exp(-0.5^2 / 2sigma^2) -> 0.065 each)
        let r1 = radius * 0.5;
        let w1 = 0.065;
        let pi = 3.14159265359;
        for (var i = 0u; i < 8u; i = i + 1u) {
            let angle = f32(i) * (pi / 4.0);
            let offset = vec2<f32>(cos(angle) * r1 * metric_x, sin(angle) * r1);
            let uv_sample = vec2<f32>(fract(env_uv.x + offset.x), clamp(env_uv.y + offset.y, 0.001, 0.999));
            total_color += textureSampleLevel(tex_environment, samp_linear, uv_sample, mip_level).rgb * w1;
            total_weight += w1;
        }
        
        // Ring 2: 8 samples at radius * 1.0 (Gaussian weight ~ exp(-1.0^2 / 2sigma^2) -> 0.035 each)
        let r2 = radius * 1.0;
        let w2 = 0.035;
        for (var i = 0u; i < 8u; i = i + 1u) {
            let angle = (f32(i) + 0.5) * (pi / 4.0);
            let offset = vec2<f32>(cos(angle) * r2 * metric_x, sin(angle) * r2);
            let uv_sample = vec2<f32>(fract(env_uv.x + offset.x), clamp(env_uv.y + offset.y, 0.001, 0.999));
            total_color += textureSampleLevel(tex_environment, samp_linear, uv_sample, mip_level).rgb * w2;
            total_weight += w2;
        }
        
        color = total_color / total_weight;
    }
    
    // Tone mapping (consistent with viewport.wgsl modes)
    let exp_val = camera.exposure * camera.env_intensity;
    let x = color * exp_val;
    var mapped = x;
    if camera.tonemap_mode == 0u {
        // Reinhard Tone mapping
        mapped = x / (x + vec3<f32>(1.0));
    } else if camera.tonemap_mode == 1u {
        // ACES Filmic Tone mapping
        mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
    }
    let final_color = clamp(mapped, vec3<f32>(0.0), vec3<f32>(1.0));
    
    var out: SkyboxOutput;
    out.color = vec4<f32>(final_color, camera.bg_opacity);
    out.normal_roughness = vec4<f32>(0.5, 0.5, 0.0, 1.0);
    return out;
}
