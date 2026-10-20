// 3D Paintcraft - Hardware GPU Mesh Map Baker
// Full-speed GPU rasterization and compute ray-tracing for texture baking

struct BakeUniforms {
    min_bound: vec4<f32>,      // xyz = min position, w = max_dist
    max_bound: vec4<f32>,      // xyz = max position, w = ray_bias
    mode: u32,                 // 0=World Normal, 1=Position (BBox), 2=Position (World), 3=Flat Tangent Normal, 4=Curvature, 5=ID Map
    samples: u32,              // ray samples for AO / Thickness
    invert: u32,               // 1 = invert green / invert thickness
    submesh_id: u32,
    tex_size: vec2<f32>,
    pad0: f32,
    pad1: f32,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) scalar_val: f32,
};

@group(0) @binding(0) var<uniform> u: BakeUniforms;

@vertex
fn vs_bake(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Map UV coordinates [0.0, 1.0] to NDC [-1.0, 1.0], invert Y for correct WebGPU coordinates
    out.clip_pos = vec4<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0, 0.0, 1.0);
    out.world_pos = in.position;
    out.world_normal = in.normal;
    out.uv = in.uv;
    out.scalar_val = in.tangent.w;
    return out;
}

fn get_palette(idx: u32) -> vec3<f32> {
    switch (idx) {
        case 0u: { return vec3<f32>(0.95, 0.15, 0.15); }
        case 1u: { return vec3<f32>(0.15, 0.85, 0.20); }
        case 2u: { return vec3<f32>(0.15, 0.35, 0.95); }
        case 3u: { return vec3<f32>(0.95, 0.85, 0.15); }
        case 4u: { return vec3<f32>(0.85, 0.15, 0.85); }
        case 5u: { return vec3<f32>(0.15, 0.85, 0.85); }
        case 6u: { return vec3<f32>(0.95, 0.50, 0.15); }
        case 7u: { return vec3<f32>(0.60, 0.15, 0.95); }
        case 8u: { return vec3<f32>(0.15, 0.95, 0.60); }
        case 9u: { return vec3<f32>(0.95, 0.15, 0.50); }
        case 10u: { return vec3<f32>(0.60, 0.85, 0.15); }
        case 11u: { return vec3<f32>(0.15, 0.60, 0.95); }
        case 12u: { return vec3<f32>(0.85, 0.60, 0.40); }
        case 13u: { return vec3<f32>(0.40, 0.70, 0.50); }
        case 14u: { return vec3<f32>(0.75, 0.75, 0.75); }
        default: { return vec3<f32>(0.95, 0.95, 0.20); }
    }
}

@fragment
fn fs_bake(in: VertexOutput) -> @location(0) vec4<f32> {
    if (u.mode == 0u) {
        // World Space Normal encoded to [0, 1]
        let n = normalize(in.world_normal);
        return vec4<f32>(n * 0.5 + 0.5, 1.0);
    } else if (u.mode == 1u) {
        // Position Bounding Box normalized to [0, 1]
        let extents = max(u.max_bound.xyz - u.min_bound.xyz, vec3<f32>(1e-5));
        let norm_pos = clamp((in.world_pos - u.min_bound.xyz) / extents, vec3<f32>(0.0), vec3<f32>(1.0));
        return vec4<f32>(norm_pos, 1.0);
    } else if (u.mode == 2u) {
        // Position World Units
        let norm_pos = clamp(in.world_pos * 0.5 + 0.5, vec3<f32>(0.0), vec3<f32>(1.0));
        return vec4<f32>(norm_pos, 1.0);
    } else if (u.mode == 3u) {
        // Flat Tangent Normal (0, 0, 1) -> (0.5, 0.5, 1.0, 1.0)
        let ny = select(0.5, 0.5, u.invert == 0u);
        return vec4<f32>(0.5, ny, 1.0, 1.0);
    } else if (u.mode == 4u) {
        // Curvature from screen-space / UV-space derivatives
        let dx = dpdx(in.world_normal);
        let dy = dpdy(in.world_normal);
        let curv = clamp((length(dx) + length(dy)) * 8.0, 0.0, 1.0);
        let val = 0.5 + (curv - 0.5) * 0.5;
        return vec4<f32>(val, val, val, 1.0);
    } else if (u.mode == 5u) {
        // ID Map
        let pal_idx = u.submesh_id % 16u;
        return vec4<f32>(get_palette(pal_idx), 1.0);
    } else if (u.mode == 6u) {
        // Scalar Vertex Attribute (e.g. vertex AO / Thickness / Curvature)
        let s = clamp(in.scalar_val, 0.0, 1.0);
        return vec4<f32>(s, s, s, 1.0);
    }
    return vec4<f32>(0.5, 0.5, 1.0, 1.0);
}

// --- GPU COMPUTE AMBIENT OCCLUSION & THICKNESS RAY TRACER ---
struct GpuTriangle {
    pos0: vec4<f32>,
    pos1: vec4<f32>,
    pos2: vec4<f32>,
    normal: vec4<f32>,
};

@group(0) @binding(1) var pos_tex: texture_2d<f32>;
@group(0) @binding(2) var norm_tex: texture_2d<f32>;
@group(0) @binding(3) var out_ao_tex: texture_storage_2d<r32float, write>;
@group(0) @binding(4) var<storage, read> tri_buffer: array<GpuTriangle>;

fn ray_triangle(orig: vec3<f32>, dir: vec3<f32>, v0: vec3<f32>, v1: vec3<f32>, v2: vec3<f32>) -> f32 {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = cross(dir, edge2);
    let a = dot(edge1, h);
    if (a > -1e-5 && a < 1e-5) { return -1.0; }
    let f = 1.0 / a;
    let s = orig - v0;
    let u = f * dot(s, h);
    if (u < 0.0 || u > 1.0) { return -1.0; }
    let q = cross(s, edge1);
    let v = f * dot(dir, q);
    if (v < 0.0 || u + v > 1.0) { return -1.0; }
    let t = f * dot(edge2, q);
    if (t > 1e-4) { return t; }
    return -1.0;
}

@compute @workgroup_size(16, 16)
fn cs_bake_ao(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let px = i32(global_id.x);
    let py = i32(global_id.y);
    let coord = vec2<i32>(px, py);

    if (f32(px) >= u.tex_size.x || f32(py) >= u.tex_size.y) {
        return;
    }

    let norm_sample = textureLoad(norm_tex, coord, 0);
    if (norm_sample.a < 0.5) {
        // Outside UV islands: unoccluded
        textureStore(out_ao_tex, coord, vec4<f32>(1.0, 0.0, 0.0, 0.0));
        return;
    }

    let pos_sample = textureLoad(pos_tex, coord, 0);
    let normal = normalize(norm_sample.rgb * 2.0 - 1.0);
    let pos = pos_sample.rgb;
    let origin = pos + normal * u.max_bound.w;
    let max_dist = u.min_bound.w;

    let up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(normal.y) > 0.95);
    let tangent = normalize(cross(normal, up));
    let bitangent = cross(normal, tangent);

    let sample_count = u.samples;
    let tri_count = arrayLength(&tri_buffer);
    var unoccluded: f32 = 0.0;

    for (var s = 0u; s < sample_count; s = s + 1u) {
        // Fibonacci hemisphere sampling
        let fi = f32(s) + 0.5;
        let phi = fi * 2.39996323; // Golden angle
        let cos_theta = sqrt(fi / f32(sample_count));
        let sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta));
        let local_dir = vec3<f32>(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);
        let world_dir = normalize(tangent * local_dir.x + bitangent * local_dir.y + normal * local_dir.z);

        var closest_hit: f32 = max_dist;
        for (var t = 0u; t < tri_count; t = t + 1u) {
            let tri = tri_buffer[t];
            let hit = ray_triangle(origin, world_dir, tri.pos0.xyz, tri.pos1.xyz, tri.pos2.xyz);
            if (hit > 0.002 && hit < closest_hit) {
                closest_hit = hit;
            }
        }

        if (closest_hit < max_dist) {
            let occ = clamp(1.0 - closest_hit / max_dist, 0.0, 1.0);
            unoccluded = unoccluded + (1.0 - occ);
        } else {
            unoccluded = unoccluded + 1.0;
        }
    }

    let ao_val = clamp(unoccluded / f32(sample_count), 0.0, 1.0);
    textureStore(out_ao_tex, coord, vec4<f32>(ao_val, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(16, 16)
fn cs_bake_thickness(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let px = i32(global_id.x);
    let py = i32(global_id.y);
    let coord = vec2<i32>(px, py);

    if (f32(px) >= u.tex_size.x || f32(py) >= u.tex_size.y) {
        return;
    }

    let norm_sample = textureLoad(norm_tex, coord, 0);
    if (norm_sample.a < 0.5) {
        let empty_val = select(0.0, 1.0, u.invert == 1u);
        textureStore(out_ao_tex, coord, vec4<f32>(empty_val, 0.0, 0.0, 0.0));
        return;
    }

    let pos_sample = textureLoad(pos_tex, coord, 0);
    let normal = normalize(norm_sample.rgb * 2.0 - 1.0);
    let pos = pos_sample.rgb;
    let inward_dir = -normal;
    let origin = pos + inward_dir * u.max_bound.w;
    let max_dist = u.min_bound.w;

    let tri_count = arrayLength(&tri_buffer);
    var closest_hit: f32 = max_dist;

    for (var t = 0u; t < tri_count; t = t + 1u) {
        let tri = tri_buffer[t];
        let hit = ray_triangle(origin, inward_dir, tri.pos0.xyz, tri.pos1.xyz, tri.pos2.xyz);
        if (hit > 0.002 && hit < closest_hit) {
            closest_hit = hit;
        }
    }

    var thick = clamp(closest_hit / max_dist, 0.0, 1.0);
    if (u.invert == 1u) {
        thick = 1.0 - thick;
    }

    textureStore(out_ao_tex, coord, vec4<f32>(thick, 0.0, 0.0, 0.0));
}
