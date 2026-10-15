// 3D Paintcraft - GPU Compute Shader 3D Projection Stamping
// Hardware-accelerated 3D world-space projection stamping directly in VRAM.

struct BrushUniform {
    hit_pos: vec4<f32>,     // xyz = hit point, w = world_radius
    hit_normal: vec4<f32>,  // xyz = hit normal, w = hardness
    paint_color: vec4<f32>, // rgba
    params: vec4<f32>,      // x = roughness, y = metallic, z = height, w = flow * opacity
    bounds_min: vec2<i32>,  // min_x, min_y
    bounds_max: vec2<i32>,  // max_x, max_y
    tex_size: vec2<f32>,    // width, height
    flags: vec2<u32>,       // x = is_eraser, y = is_mask_target
    tri_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

struct BrushTriangle {
    pos0: vec4<f32>,
    pos1: vec4<f32>,
    pos2: vec4<f32>,
    nrm0: vec4<f32>,
    nrm1: vec4<f32>,
    nrm2: vec4<f32>,
    uv0: vec2<f32>,
    uv1: vec2<f32>,
    uv2: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> brush: BrushUniform;
@group(0) @binding(1) var<storage, read> triangles: array<BrushTriangle>;
@group(0) @binding(2) var in_color: texture_2d<f32>;
@group(0) @binding(3) var in_normal: texture_2d<f32>;
@group(0) @binding(4) var in_roughness: texture_2d<f32>;
@group(0) @binding(5) var in_metallic: texture_2d<f32>;

@group(0) @binding(6) var out_color: texture_storage_2d<rgba32float, write>;
@group(0) @binding(7) var out_normal: texture_storage_2d<rgba32float, write>;
@group(0) @binding(8) var out_roughness: texture_storage_2d<r32float, write>;
@group(0) @binding(9) var out_metallic: texture_storage_2d<r32float, write>;

@compute @workgroup_size(16, 16)
fn cs_stamp(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let px = brush.bounds_min.x + i32(global_id.x);
    let py = brush.bounds_min.y + i32(global_id.y);

    if (px > brush.bounds_max.x || py > brush.bounds_max.y || px < 0 || py < 0) {
        return;
    }
    if (f32(px) >= brush.tex_size.x || f32(py) >= brush.tex_size.y) {
        return;
    }

    let pixel_coord = vec2<i32>(px, py);
    let uv = (vec2<f32>(f32(px), f32(py)) + 0.5) / brush.tex_size;

    var best_weight: f32 = 0.0;
    var hit_found: bool = false;

    let tri_n = brush.tri_count;
    for (var i = 0u; i < tri_n; i = i + 1u) {
        let tri = triangles[i];
        let p0 = tri.uv0;
        let p1 = tri.uv1;
        let p2 = tri.uv2;

        let det = (p1.y - p2.y) * (p0.x - p2.x) + (p2.x - p1.x) * (p0.y - p2.y);
        if (abs(det) < 1e-6) {
            continue;
        }
        let inv_det = 1.0 / det;

        let w0 = ((p1.y - p2.y) * (uv.x - p2.x) + (p2.x - p1.x) * (uv.y - p2.y)) * inv_det;
        let w1 = ((p2.y - p0.y) * (uv.x - p2.x) + (p0.x - p2.x) * (uv.y - p2.y)) * inv_det;
        let w2 = 1.0 - w0 - w1;

        if (w0 >= -0.01 && w1 >= -0.01 && w2 >= -0.01) {
            let cw0 = clamp(w0, 0.0, 1.0);
            let cw1 = clamp(w1, 0.0, 1.0);
            let cw2 = clamp(w2, 0.0, 1.0);

            let pos3d = tri.pos0.xyz * cw0 + tri.pos1.xyz * cw1 + tri.pos2.xyz * cw2;
            let nrm3d = normalize(tri.nrm0.xyz * cw0 + tri.nrm1.xyz * cw1 + tri.nrm2.xyz * cw2);

            let dist = length(pos3d - brush.hit_pos.xyz);
            if (dist <= brush.hit_pos.w) {
                let n_dot = dot(nrm3d, brush.hit_normal.xyz);
                if (n_dot >= -0.1) {
                    let angle_w = clamp(n_dot + 0.1, 0.0, 1.0);
                    let norm_dist = clamp(dist / brush.hit_pos.w, 0.0, 1.0);
                    let hardness = brush.hit_normal.w;
                    var falloff: f32 = 1.0;
                    if (norm_dist > hardness) {
                        falloff = 1.0 - (norm_dist - hardness) / max(1.0 - hardness, 0.0001);
                    }
                    let w = clamp(falloff * brush.params.w * angle_w, 0.0, 1.0);
                    if (w > best_weight) {
                        best_weight = w;
                        hit_found = true;
                    }
                }
            }
        }
    }

    let cur_col = textureLoad(in_color, pixel_coord, 0);
    let cur_norm = textureLoad(in_normal, pixel_coord, 0);
    let cur_r = textureLoad(in_roughness, pixel_coord, 0).r;
    let cur_m = textureLoad(in_metallic, pixel_coord, 0).r;

    if (!hit_found || best_weight <= 0.0001) {
        textureStore(out_color, pixel_coord, cur_col);
        textureStore(out_normal, pixel_coord, cur_norm);
        textureStore(out_roughness, pixel_coord, vec4<f32>(cur_r, 0.0, 0.0, 0.0));
        textureStore(out_metallic, pixel_coord, vec4<f32>(cur_m, 0.0, 0.0, 0.0));
        return;
    }

    if (brush.flags.x == 1u) {
        // Eraser mode
        let new_a = max(cur_col.a * (1.0 - best_weight), 0.0);
        textureStore(out_color, pixel_coord, vec4<f32>(cur_col.rgb, new_a));
        textureStore(out_normal, pixel_coord, cur_norm);
        textureStore(out_roughness, pixel_coord, vec4<f32>(cur_r, 0.0, 0.0, 0.0));
        textureStore(out_metallic, pixel_coord, vec4<f32>(cur_m, 0.0, 0.0, 0.0));
    } else {
        // Paint mode
        let new_rgb = mix(cur_col.rgb, brush.paint_color.rgb, best_weight);
        let new_a = clamp(cur_col.a + best_weight * (1.0 - cur_col.a), 0.0, 1.0);
        textureStore(out_color, pixel_coord, vec4<f32>(new_rgb, new_a));
        textureStore(out_normal, pixel_coord, cur_norm);

        let new_r = mix(cur_r, brush.params.x, best_weight);
        textureStore(out_roughness, pixel_coord, vec4<f32>(new_r, 0.0, 0.0, 0.0));

        let new_m = mix(cur_m, brush.params.y, best_weight);
        textureStore(out_metallic, pixel_coord, vec4<f32>(new_m, 0.0, 0.0, 0.0));
    }
}
