use std::collections::{HashMap, HashSet};
use crate::channel::{RgbaBuffer, ScalarBuffer};
use crate::mesh::{Mesh, Ray};
use glam::{Vec2, Vec3, Vec4};
use rayon::prelude::*;

struct SpatialHash {
    cell_size: f32,
    min_bound: Vec3,
    cells: HashMap<[i32; 3], Vec<usize>>,
}

impl SpatialHash {
    fn new(mesh: &Mesh) -> Self {
        let mut min_bound = Vec3::splat(f32::MAX);
        let mut max_bound = Vec3::splat(f32::MIN);
        for v in &mesh.vertices {
            min_bound = min_bound.min(v.position);
            max_bound = max_bound.max(v.position);
        }
        
        let extents = max_bound - min_bound;
        let volume = extents.x * extents.y * extents.z;
        let tri_count = mesh.indices.len() / 3;
        
        let cell_volume = volume / (tri_count as f32 / 10.0).max(1.0);
        let cell_size = cell_volume.powf(1.0 / 3.0).max(0.005);
        
        let mut cells: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
        
        for t in 0..tri_count {
            let v0 = mesh.vertices[mesh.indices[t * 3] as usize].position;
            let v1 = mesh.vertices[mesh.indices[t * 3 + 1] as usize].position;
            let v2 = mesh.vertices[mesh.indices[t * 3 + 2] as usize].position;
            
            let t_min = v0.min(v1).min(v2);
            let t_max = v0.max(v1).max(v2);
            
            let min_coord = [
                ((t_min.x - min_bound.x) / cell_size).floor() as i32,
                ((t_min.y - min_bound.y) / cell_size).floor() as i32,
                ((t_min.z - min_bound.z) / cell_size).floor() as i32,
            ];
            let max_coord = [
                ((t_max.x - min_bound.x) / cell_size).floor() as i32,
                ((t_max.y - min_bound.y) / cell_size).floor() as i32,
                ((t_max.z - min_bound.z) / cell_size).floor() as i32,
            ];
            
            for z in min_coord[2]..=max_coord[2] {
                for y in min_coord[1]..=max_coord[1] {
                    for x in min_coord[0]..=max_coord[0] {
                        cells.entry([x, y, z]).or_default().push(t);
                    }
                }
            }
        }
        
        Self { cell_size, cells, min_bound }
    }

    fn ray_triangle_intersect(orig: Vec3, dir: Vec3, v0: Vec3, v1: Vec3, v2: Vec3) -> Option<(f32, f32, f32)> {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = dir.cross(edge2);
    let a = edge1.dot(h);
    if a > -1e-5 && a < 1e-5 { return None; }
    let f = 1.0 / a;
    let s = orig - v0;
    let u = f * s.dot(h);
    if u < 0.0 || u > 1.0 { return None; }
    let q = s.cross(edge1);
    let v = f * dir.dot(q);
    if v < 0.0 || u + v > 1.0 { return None; }
    let t = f * edge2.dot(q);
    if t > 1e-5 { Some((t, u, v)) } else { None }
}

    pub fn query_ray_closest(&self, origin: Vec3, dir: Vec3, max_dist: f32, mesh: &Mesh) -> Option<(f32, Vec3)> {
        let end_pos = origin + dir * max_dist;
        let r_min = origin.min(end_pos);
        let r_max = origin.max(end_pos);
        
        let min_coord = [
            ((r_min.x - self.min_bound.x) / self.cell_size).floor() as i32,
            ((r_min.y - self.min_bound.y) / self.cell_size).floor() as i32,
            ((r_min.z - self.min_bound.z) / self.cell_size).floor() as i32,
        ];
        let max_coord = [
            ((r_max.x - self.min_bound.x) / self.cell_size).floor() as i32,
            ((r_max.y - self.min_bound.y) / self.cell_size).floor() as i32,
            ((r_max.z - self.min_bound.z) / self.cell_size).floor() as i32,
        ];

        let mut closest_t = max_dist;
        let mut best_normal = None;

        for z in min_coord[2]..=max_coord[2] {
            for y in min_coord[1]..=max_coord[1] {
                for x in min_coord[0]..=max_coord[0] {
                    if let Some(triangles) = self.cells.get(&[x, y, z]) {
                        for &t in triangles {
                            let i0 = mesh.indices[t * 3] as usize;
                            let i1 = mesh.indices[t * 3 + 1] as usize;
                            let i2 = mesh.indices[t * 3 + 2] as usize;
                            let v0 = mesh.vertices[i0].position;
                            let v1 = mesh.vertices[i1].position;
                            let v2 = mesh.vertices[i2].position;
                            
                            if let Some((t_hit, u, v)) = SpatialHash::ray_triangle_intersect(origin, dir, v0, v1, v2) {
                                if t_hit > 0.0 && t_hit < closest_t {
                                    closest_t = t_hit;
                                    let n0 = mesh.vertices[i0].normal;
                                    let n1 = mesh.vertices[i1].normal;
                                    let n2 = mesh.vertices[i2].normal;
                                    let n = (n0 * (1.0 - u - v) + n1 * u + n2 * v).normalize_or_zero();
                                    best_normal = Some(n);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        best_normal.map(|n| (closest_t, n))
    }
}


pub struct Baker;

impl Baker {
    /// Bake Curvature map (Convex edges = white, Crevices = black, Flat = 0.5 gray)
    pub fn bake_curvature(mesh: &Mesh, width: u32, height: u32) -> ScalarBuffer {
        let mut buffer = ScalarBuffer::new(width, height, 0.5);
        let vert_count = mesh.vertices.len();
        if vert_count == 0 || mesh.indices.is_empty() {
            return buffer;
        }

        let tri_count = mesh.indices.len() / 3;

        // 1. Quantize 3D positions to weld duplicate seam vertices
        let mut pos_map: HashMap<(i32, i32, i32), usize> = HashMap::new();
        let mut canonical_indices = Vec::with_capacity(vert_count);
        let mut canonical_positions = Vec::new();
        let mut canonical_normals = Vec::new();

        for (_i, v) in mesh.vertices.iter().enumerate() {
            let key = (
                (v.position.x * 10000.0).round() as i32,
                (v.position.y * 10000.0).round() as i32,
                (v.position.z * 10000.0).round() as i32,
            );
            match pos_map.get(&key) {
                Some(&canon) => {
                    canonical_indices.push(canon);
                    canonical_normals[canon] += v.normal;
                }
                None => {
                    let canon = canonical_positions.len();
                    pos_map.insert(key, canon);
                    canonical_indices.push(canon);
                    canonical_positions.push(v.position);
                    canonical_normals.push(v.normal);
                }
            }
        }
        let canon_count = canonical_positions.len();
        for n in &mut canonical_normals {
            *n = n.normalize_or_zero();
        }

        // 2. Precompute face normals and centroids
        let mut face_normals = Vec::with_capacity(tri_count);
        let mut face_centroids = Vec::with_capacity(tri_count);

        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let p0 = mesh.vertices[i0].position;
            let p1 = mesh.vertices[i1].position;
            let p2 = mesh.vertices[i2].position;

            let fnorm = (p1 - p0).cross(p2 - p0).normalize_or_zero();
            let centroid = (p0 + p1 + p2) / 3.0;

            face_normals.push(fnorm);
            face_centroids.push(centroid);
        }

        // 3. Build edge-to-face adjacency map and 1-ring vertex neighbors
        let mut edge_faces: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        let mut vert_neighbors: Vec<HashSet<usize>> = vec![HashSet::new(); canon_count];

        for t in 0..tri_count {
            let c0 = canonical_indices[mesh.indices[t * 3] as usize];
            let c1 = canonical_indices[mesh.indices[t * 3 + 1] as usize];
            let c2 = canonical_indices[mesh.indices[t * 3 + 2] as usize];

            let edges = [(c0, c1), (c1, c2), (c2, c0)];
            for &(a, b) in &edges {
                if a != b {
                    let key = if a < b { (a, b) } else { (b, a) };
                    edge_faces.entry(key).or_default().push(t);
                    vert_neighbors[a].insert(b);
                    vert_neighbors[b].insert(a);
                }
            }
        }

        // 4. Calculate signed dihedral curvature across shared edges
        let mut vert_curv_sum = vec![0.0f32; canon_count];
        let mut vert_curv_weight = vec![0.0f32; canon_count];

        for (&(v_a, v_b), faces) in &edge_faces {
            if faces.len() >= 2 {
                for i in 0..faces.len() {
                    for j in (i + 1)..faces.len() {
                        let fa = faces[i];
                        let fb = faces[j];
                        let na = face_normals[fa];
                        let nb = face_normals[fb];
                        let ca = face_centroids[fa];
                        let cb = face_centroids[fb];

                        let dot = na.dot(nb).clamp(-1.0, 1.0);
                        let angle = dot.acos();
                        if angle > 1e-4 {
                            // Convex vs concave test: (cb - ca) dot (na - nb)
                            // Negative = convex ridge (faces bend away from normals)
                            // Positive = concave crevice (faces bend toward normals)
                            let d = cb - ca;
                            let test = d.dot(na - nb);
                            let sign = if test < -1e-5 {
                                1.0f32
                            } else if test > 1e-5 {
                                -1.0f32
                            } else {
                                0.0f32
                            };

                            let signed_curv = sign * angle;
                            vert_curv_sum[v_a] += signed_curv;
                            vert_curv_weight[v_a] += 1.0;
                            vert_curv_sum[v_b] += signed_curv;
                            vert_curv_weight[v_b] += 1.0;
                        }
                    }
                }
            }
        }

        // 5. Calculate normal deflection curvature on 1-ring neighbors (smooth surfaces)
        let mut vert_deflect_sum = vec![0.0f32; canon_count];
        let mut vert_deflect_count = vec![0.0f32; canon_count];

        for i in 0..canon_count {
            let pi = canonical_positions[i];
            let ni = canonical_normals[i];
            for &j in &vert_neighbors[i] {
                let pj = canonical_positions[j];
                let diff = pj - pi;
                let dist = diff.length();
                if dist > 1e-6 {
                    let dir = diff / dist;
                    let deflect = -ni.dot(dir);
                    vert_deflect_sum[i] += deflect;
                    vert_deflect_count[i] += 1.0;
                }
            }
        }

        // 6. Map to [0.0, 1.0] where 0.5 is flat neutral gray
        let mut canonical_curvature = vec![0.5f32; canon_count];
        for i in 0..canon_count {
            let avg_dih = if vert_curv_weight[i] > 0.0 {
                vert_curv_sum[i] / vert_curv_weight[i]
            } else {
                0.0
            };
            let avg_deflect = if vert_deflect_count[i] > 0.0 {
                vert_deflect_sum[i] / vert_deflect_count[i]
            } else {
                0.0
            };

            let curv_signal = avg_dih * 0.40 + avg_deflect * 0.25;
            let val = (0.5 + curv_signal).clamp(0.0, 1.0);
            canonical_curvature[i] = val;
        }

        // 7. Rasterize triangles into UV buffer with seam wrapping and mask tracking
        let mut written_mask = vec![false; (width * height) as usize];
        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let uv0 = mesh.vertices[i0].uv;
            let uv1 = mesh.vertices[i1].uv;
            let uv2 = mesh.vertices[i2].uv;

            let c0 = canonical_curvature[canonical_indices[i0]];
            let c1 = canonical_curvature[canonical_indices[i1]];
            let c2 = canonical_curvature[canonical_indices[i2]];

            Self::rasterize_scalar_triangle_masked(
                &mut buffer,
                &mut written_mask,
                uv0, uv1, uv2,
                c0, c1, c2,
            );
        }

        // 8. Dilate 16px into empty space so edge blur doesn't bleed into 0.5 background
        Self::dilate_scalar(&mut buffer, &mut written_mask, 16);

        // 9. 2-Pass Separable Gaussian Blur for smooth edge gradients
        let mut blurred = buffer.clone();
        let w = width as i32;
        let h = height as i32;
        let weights = [0.06136f32, 0.24477, 0.38774, 0.24477, 0.06136];

        // Horizontal pass
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0f32;
                for (k, &wt) in weights.iter().enumerate() {
                    let sample_x = (x + (k as i32 - 2)).clamp(0, w - 1) as u32;
                    sum += buffer.get_pixel(sample_x, y as u32) * wt;
                }
                blurred.set_pixel(x as u32, y as u32, sum);
            }
        }

        // Vertical pass
        let mut final_buf = blurred.clone();
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0f32;
                for (k, &wt) in weights.iter().enumerate() {
                    let sample_y = (y + (k as i32 - 2)).clamp(0, h - 1) as u32;
                    sum += blurred.get_pixel(x as u32, sample_y) * wt;
                }
                final_buf.set_pixel(x as u32, y as u32, sum);
            }
        }

        // 10. Dilate again so bilinear filtering outside UV borders is completely smooth
        Self::dilate_scalar(&mut final_buf, &mut written_mask, 16);

        final_buf
    }

    /// Bake Ambient Occlusion map via Monte Carlo Hemisphere Occlusion with smoothed normal consideration
    pub fn bake_ao(mesh: &Mesh, width: u32, height: u32) -> ScalarBuffer {
        let mut buffer = ScalarBuffer::new(width, height, 1.0);
        let tri_count = mesh.indices.len() / 3;

        // 32 sample hemisphere directions via Fibonacci spiral
        let sample_count = 32usize;
        let mut hemisphere_samples = Vec::with_capacity(sample_count);
        for i in 0..sample_count {
            let u1 = (i as f32 + 0.5) / sample_count as f32;
            let u2 = (i as f32 * 0.6180339887).fract();
            let cos_theta = (1.0 - u1).sqrt();
            let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
            let phi = u2 * 2.0 * std::f32::consts::PI;
            let dir = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);
            hemisphere_samples.push(dir);
        }

        let extents = mesh.aabb_max - mesh.aabb_min;
        let diag = extents.length();
        let max_dist = (diag * 0.35).max(0.1);
        let min_self_dist = (diag * 0.03).max(0.008);
        let ray_offset = (diag * 0.008).max(0.003);

        // Rayon parallel computation of vertex AO with smoothed normals
        let vert_ao: Vec<f32> = mesh.vertices.par_iter().map(|v| {
            let normal = if v.normal.length_squared() > 1e-4 {
                v.normal.normalize()
            } else {
                Vec3::Y
            };

            // Build orthonormal basis (tangent, bitangent, normal)
            let up = if normal.y.abs() < 0.95 { Vec3::Y } else { Vec3::X };
            let tangent = normal.cross(up).normalize();
            let bitangent = normal.cross(tangent);

            let origin = v.position + normal * ray_offset;
            let mut unoccluded = 0.0f32;

            for sample in &hemisphere_samples {
                let world_dir = (tangent * sample.x + bitangent * sample.y + normal * sample.z).normalize();
                let ray = Ray {
                    origin,
                    direction: world_dir,
                };
                if let Some(hit) = mesh.raycast(&ray) {
                    // Only count valid occlusions:
                    // 1. Must be a front-facing hit surface: hit.normal must oppose ray direction
                    // 2. Reject self-intersections on convex curved facets:
                    //    If the hit point is very close and its normal roughly matches origin normal,
                    //    it is a facet neighbor on the same smoothly curved mesh, not an occluder!
                    let is_front_facing = hit.normal.dot(world_dir) < -0.05;
                    let is_neighbor_facet = hit.t < min_self_dist && hit.normal.dot(normal) > 0.4;

                    if is_front_facing && !is_neighbor_facet && hit.t > 0.002 && hit.t < max_dist {
                        let occ = (1.0 - hit.t / max_dist).clamp(0.0, 1.0);
                        unoccluded += 1.0 - occ * 0.85;
                    } else {
                        unoccluded += 1.0;
                    }
                } else {
                    unoccluded += 1.0;
                }
            }

            (unoccluded / sample_count as f32).clamp(0.05, 1.0)
        }).collect();

        let mut written_mask = vec![false; (width * height) as usize];
        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let uv0 = mesh.vertices[i0].uv;
            let uv1 = mesh.vertices[i1].uv;
            let uv2 = mesh.vertices[i2].uv;

            Self::rasterize_scalar_triangle_masked(
                &mut buffer,
                &mut written_mask,
                uv0, uv1, uv2,
                vert_ao[i0], vert_ao[i1], vert_ao[i2],
            );
        }

        Self::dilate_scalar(&mut buffer, &mut written_mask, 16);
        buffer
    }

    /// Bake Tangent Space Normal map
    pub fn bake_tangent_normals(mesh: &Mesh, high_poly: Option<&Mesh>, frontal_dist: f32, rear_dist: f32, width: u32, height: u32) -> RgbaBuffer {
        let mut buffer = RgbaBuffer::new(width, height, Vec4::new(0.5, 0.5, 1.0, 1.0));
        let tri_count = mesh.indices.len() / 3;
        
        let spatial_hash = high_poly.map(|hp| SpatialHash::new(hp));

        // Generate position, normal and tangent raster buffers for Low Poly
        let mut pos_buf = vec![None; (width * height) as usize];
        let mut norm_buf = vec![None; (width * height) as usize];
        let mut tan_buf = vec![None; (width * height) as usize];
        
        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let v0 = &mesh.vertices[i0];
            let v1 = &mesh.vertices[i1];
            let v2 = &mesh.vertices[i2];

            Self::rasterize_pos_norm_tan_triangle(
                &mut pos_buf,
                &mut norm_buf,
                &mut tan_buf,
                width,
                height,
                v0.uv, v1.uv, v2.uv,
                v0.position, v1.position, v2.position,
                v0.normal, v1.normal, v2.normal,
                v0.tangent, v1.tangent, v2.tangent,
            );
        }

        // Raycast
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if let (Some(pos), Some(norm)) = (pos_buf[idx], norm_buf[idx]) {
                    if let Some(hp) = high_poly {
                        if let Some(hash) = &spatial_hash {
                            // Cast ray outward by frontal_dist then back towards mesh up to frontal + rear
                            let ray_orig = pos + norm * frontal_dist;
                            let ray_dir = -norm;
                            
                            if let Some((_, hp_norm)) = hash.query_ray_closest(ray_orig, ray_dir, frontal_dist + rear_dist, hp) {
                                if let Some(tan) = tan_buf[idx] {
                                    let n = norm.normalize_or_zero();
                                    let t = (tan.truncate() - n * n.dot(tan.truncate())).normalize_or_zero();
                                    let tw = if tan.w < 0.0 { -1.0 } else { 1.0 };
                                    let b = n.cross(t) * tw;
                                    let ts_norm = Vec3::new(hp_norm.dot(t), hp_norm.dot(b), hp_norm.dot(n)).normalize_or_zero();
                                    buffer.set_pixel(x, y, Vec4::new(ts_norm.x * 0.5 + 0.5, ts_norm.y * 0.5 + 0.5, ts_norm.z * 0.5 + 0.5, 1.0));
                                    continue;
                                }
                            }
                        }
                    }
                    // Low poly fallback: flat tangent normal (0, 0, 1) encoded to (0.5, 0.5, 1.0, 1.0)
                    buffer.set_pixel(x, y, Vec4::new(0.5, 0.5, 1.0, 1.0));
                }
            }
        }
        
        // Edge dilation
        Self::dilate_rgba(&mut buffer, &pos_buf);
        buffer
    }

    /// Bake World Position Object (WPO) / Position map normalized to [0, 1] bounding box
    pub fn bake_position(mesh: &Mesh, width: u32, height: u32) -> RgbaBuffer {
        let mut buffer = RgbaBuffer::new(width, height, Vec4::new(0.5, 0.5, 0.5, 1.0));
        let tri_count = mesh.indices.len() / 3;

        let mut min_bound = Vec3::splat(f32::MAX);
        let mut max_bound = Vec3::splat(f32::MIN);
        for v in &mesh.vertices {
            min_bound = min_bound.min(v.position);
            max_bound = max_bound.max(v.position);
        }
        let extents = (max_bound - min_bound).max(Vec3::splat(1e-5));

        let mut pos_buf = vec![None; (width * height) as usize];
        let mut norm_buf = vec![None; (width * height) as usize];
        let mut tan_buf = vec![None; (width * height) as usize];

        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let v0 = &mesh.vertices[i0];
            let v1 = &mesh.vertices[i1];
            let v2 = &mesh.vertices[i2];

            Self::rasterize_pos_norm_tan_triangle(
                &mut pos_buf,
                &mut norm_buf,
                &mut tan_buf,
                width,
                height,
                v0.uv, v1.uv, v2.uv,
                v0.position, v1.position, v2.position,
                v0.normal, v1.normal, v2.normal,
                v0.tangent, v1.tangent, v2.tangent,
            );
        }

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if let Some(pos) = pos_buf[idx] {
                    let norm_pos = ((pos - min_bound) / extents).clamp(Vec3::ZERO, Vec3::ONE);
                    buffer.set_pixel(x, y, Vec4::new(norm_pos.x, norm_pos.y, norm_pos.z, 1.0));
                }
            }
        }

        Self::dilate_rgba(&mut buffer, &pos_buf);
        buffer
    }

    /// Bake World Space Normal map encoded into RGB [0, 1]
    pub fn bake_world_space_normal(mesh: &Mesh, width: u32, height: u32) -> RgbaBuffer {
        let mut buffer = RgbaBuffer::new(width, height, Vec4::new(0.5, 0.5, 1.0, 1.0));
        let tri_count = mesh.indices.len() / 3;

        let mut pos_buf = vec![None; (width * height) as usize];
        let mut norm_buf = vec![None; (width * height) as usize];
        let mut tan_buf = vec![None; (width * height) as usize];

        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let v0 = &mesh.vertices[i0];
            let v1 = &mesh.vertices[i1];
            let v2 = &mesh.vertices[i2];

            Self::rasterize_pos_norm_tan_triangle(
                &mut pos_buf,
                &mut norm_buf,
                &mut tan_buf,
                width,
                height,
                v0.uv, v1.uv, v2.uv,
                v0.position, v1.position, v2.position,
                v0.normal, v1.normal, v2.normal,
                v0.tangent, v1.tangent, v2.tangent,
            );
        }

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if let Some(norm) = norm_buf[idx] {
                    let n = norm.normalize_or_zero();
                    buffer.set_pixel(x, y, Vec4::new(n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.0));
                }
            }
        }

        Self::dilate_rgba(&mut buffer, &pos_buf);
        buffer
    }

    /// Bake Surface Thickness map for subsurface scattering and light transmission
    pub fn bake_thickness(mesh: &Mesh, width: u32, height: u32) -> ScalarBuffer {
        let mut buffer = ScalarBuffer::new(width, height, 1.0);
        let tri_count = mesh.indices.len() / 3;

        let mut min_bound = Vec3::splat(f32::MAX);
        let mut max_bound = Vec3::splat(f32::MIN);
        for v in &mesh.vertices {
            min_bound = min_bound.min(v.position);
            max_bound = max_bound.max(v.position);
        }
        let max_dist = ((max_bound - min_bound).length() * 0.35).max(0.1);
        let spatial_hash = SpatialHash::new(mesh);

        let mut pos_buf = vec![None; (width * height) as usize];
        let mut norm_buf = vec![None; (width * height) as usize];
        let mut tan_buf = vec![None; (width * height) as usize];

        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let v0 = &mesh.vertices[i0];
            let v1 = &mesh.vertices[i1];
            let v2 = &mesh.vertices[i2];

            Self::rasterize_pos_norm_tan_triangle(
                &mut pos_buf,
                &mut norm_buf,
                &mut tan_buf,
                width,
                height,
                v0.uv, v1.uv, v2.uv,
                v0.position, v1.position, v2.position,
                v0.normal, v1.normal, v2.normal,
                v0.tangent, v1.tangent, v2.tangent,
            );
        }

        let mut written_mask = vec![false; (width * height) as usize];
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if let (Some(pos), Some(norm)) = (pos_buf[idx], norm_buf[idx]) {
                    let n = norm.normalize_or_zero();
                    let ray_orig = pos - n * 0.001;
                    let ray_dir = -n;

                    let thick = if let Some((hit_dist, _)) = spatial_hash.query_ray_closest(ray_orig, ray_dir, max_dist, mesh) {
                        (hit_dist / max_dist).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };

                    buffer.set_pixel(x, y, thick);
                    written_mask[idx] = true;
                }
            }
        }

        Self::dilate_scalar(&mut buffer, &mut written_mask, 16);
        buffer
    }
    
    fn rasterize_pos_norm_tan_triangle(
        pos_buf: &mut [Option<Vec3>],
        norm_buf: &mut [Option<Vec3>],
        tan_buf: &mut [Option<Vec4>],
        width: u32,
        height: u32,
        uv0: Vec2,
        uv1: Vec2,
        uv2: Vec2,
        p0: Vec3,
        p1: Vec3,
        p2: Vec3,
        n0: Vec3,
        n1: Vec3,
        n2: Vec3,
        t0: Vec4,
        t1: Vec4,
        t2: Vec4,
    ) {
        let w = width as f32;
        let h = height as f32;

        let u0 = Vec2::new(uv0.x * w, uv0.y * h);
        let u1 = Vec2::new(uv1.x * w, uv1.y * h);
        let u2 = Vec2::new(uv2.x * w, uv2.y * h);

        let min_x = (u0.x.min(u1.x).min(u2.x).floor() as i32).max(0);
        let max_x = (u0.x.max(u1.x).max(u2.x).ceil() as i32).min(width as i32 - 1);
        let min_y = (u0.y.min(u1.y).min(u2.y).floor() as i32).max(0);
        let max_y = (u0.y.max(u1.y).max(u2.y).ceil() as i32).min(height as i32 - 1);

        let denom = (u1.y - u2.y) * (u0.x - u2.x) + (u2.x - u1.x) * (u0.y - u2.y);
        if denom.abs() < 1e-5 { return; }

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;

                let w0 = ((u1.y - u2.y) * (px - u2.x) + (u2.x - u1.x) * (py - u2.y)) / denom;
                let w1 = ((u2.y - u0.y) * (px - u2.x) + (u0.x - u2.x) * (py - u2.y)) / denom;
                let w2 = 1.0 - w0 - w1;

                if w0 >= -0.01 && w1 >= -0.01 && w2 >= -0.01 {
                    let idx = (y as u32 * width + x as u32) as usize;
                    pos_buf[idx] = Some(p0 * w0 + p1 * w1 + p2 * w2);
                    norm_buf[idx] = Some((n0 * w0 + n1 * w1 + n2 * w2).normalize_or_zero());
                    tan_buf[idx] = Some(t0 * w0 + t1 * w1 + t2 * w2);
                }
            }
        }
    }
    
    fn dilate_rgba(buffer: &mut RgbaBuffer, pos_buf: &[Option<Vec3>]) {
        let original = buffer.data.clone();
        for y in 0..buffer.height {
            for x in 0..buffer.width {
                let idx = (y * buffer.width + x) as usize;
                if pos_buf[idx].is_none() {
                    let mut nearest_val = Vec4::new(0.5, 0.5, 1.0, 1.0);
                    let mut min_dist = 99999;
                    let r = 16isize;
                    let min_dy = (y as isize - r).max(0) as usize;
                    let max_dy = (y as isize + r).min(buffer.height as isize - 1) as usize;
                    let min_dx = (x as isize - r).max(0) as usize;
                    let max_dx = (x as isize + r).min(buffer.width as i32 as isize - 1) as usize;
                    
                    for sy in min_dy..=max_dy {
                        for sx in min_dx..=max_dx {
                            let sidx = sy * buffer.width as usize + sx;
                            if pos_buf[sidx].is_some() {
                                let dist = (x as isize - sx as isize).pow(2) + (y as isize - sy as isize).pow(2);
                                if dist < min_dist {
                                    min_dist = dist;
                                    nearest_val = original[sidx];
                                }
                            }
                        }
                    }
                    buffer.data[idx] = nearest_val;
                }
            }
        }
    }


    /// Dilate scalar buffer by expanding valid mask pixels into empty UV background
    fn dilate_scalar(buffer: &mut ScalarBuffer, mask: &mut [bool], passes: usize) {
        let w = buffer.width as usize;
        let h = buffer.height as usize;
        let offsets: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

        for _ in 0..passes {
            let mut writes = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    let idx = y * w + x;
                    if !mask[idx] {
                        let mut sum = 0.0f32;
                        let mut count = 0usize;
                        for &(dx, dy) in &offsets {
                            let nx = x as isize + dx;
                            let ny = y as isize + dy;
                            if nx >= 0 && nx < w as isize && ny >= 0 && ny < h as isize {
                                let nidx = (ny as usize) * w + (nx as usize);
                                if mask[nidx] {
                                    sum += buffer.data[nidx];
                                    count += 1;
                                }
                            }
                        }
                        if count > 0 {
                            writes.push((idx, sum / count as f32));
                        }
                    }
                }
            }
            if writes.is_empty() {
                break;
            }
            for (idx, val) in writes {
                buffer.data[idx] = val;
                mask[idx] = true;
            }
        }
    }

    /// Helper to rasterize a triangle in UV space for scalar buffers with seam handling and mask
    fn rasterize_scalar_triangle_masked(
        buf: &mut ScalarBuffer,
        mask: &mut [bool],
        uv0: Vec2,
        uv1: Vec2,
        uv2: Vec2,
        v0: f32,
        v1: f32,
        v2: f32,
    ) {
        let w = buf.width as f32;
        let h = buf.height as f32;

        let u0 = uv0;
        let mut u1 = uv1;
        let mut u2 = uv2;

        // Seam-normalization relative to uv0
        if (u1.x - u0.x).abs() > 0.5 {
            if u1.x > u0.x { u1.x -= 1.0; } else { u1.x += 1.0; }
        }
        if (u2.x - u0.x).abs() > 0.5 {
            if u2.x > u0.x { u2.x -= 1.0; } else { u2.x += 1.0; }
        }
        if (u1.y - u0.y).abs() > 0.5 {
            if u1.y > u0.y { u1.y -= 1.0; } else { u1.y += 1.0; }
        }
        if (u2.y - u0.y).abs() > 0.5 {
            if u2.y > u0.y { u2.y -= 1.0; } else { u2.y += 1.0; }
        }

        let p0 = Vec2::new(u0.x * w, u0.y * h);
        let p1 = Vec2::new(u1.x * w, u1.y * h);
        let p2 = Vec2::new(u2.x * w, u2.y * h);

        let min_x = p0.x.min(p1.x).min(p2.x).floor() as i32;
        let max_x = p0.x.max(p1.x).max(p2.x).ceil() as i32;
        let min_y = p0.y.min(p1.y).min(p2.y).floor() as i32;
        let max_y = p0.y.max(p1.y).max(p2.y).ceil() as i32;

        if (max_x - min_x) > buf.width as i32 || (max_y - min_y) > buf.height as i32 {
            return;
        }

        let denom = (p1.y - p2.y) * (p0.x - p2.x) + (p2.x - p1.x) * (p0.y - p2.y);
        if denom.abs() < 1e-5 {
            return;
        }

        let bw = buf.width as i32;
        let bh = buf.height as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;

                let w0 = ((p1.y - p2.y) * (px - p2.x) + (p2.x - p1.x) * (py - p2.y)) / denom;
                let w1 = ((p2.y - p0.y) * (px - p2.x) + (p0.x - p2.x) * (py - p2.y)) / denom;
                let w2 = 1.0 - w0 - w1;

                if w0 >= -0.01 && w1 >= -0.01 && w2 >= -0.01 {
                    let val = (w0 * v0 + w1 * v1 + w2 * v2).clamp(0.0, 1.0);
                    let target_x = x.rem_euclid(bw) as u32;
                    let target_y = y.rem_euclid(bh) as u32;
                    buf.set_pixel(target_x, target_y, val);
                    let idx = (target_y * buf.width + target_x) as usize;
                    mask[idx] = true;
                }
            }
        }
    }

    /// Helper to rasterize a triangle in UV space for scalar buffers
    #[allow(dead_code)]
    fn rasterize_scalar_triangle(
        buf: &mut ScalarBuffer,
        uv0: Vec2,
        uv1: Vec2,
        uv2: Vec2,
        v0: f32,
        v1: f32,
        v2: f32,
    ) {
        let mut dummy_mask = vec![false; (buf.width * buf.height) as usize];
        Self::rasterize_scalar_triangle_masked(buf, &mut dummy_mask, uv0, uv1, uv2, v0, v1, v2);
    }

    /// Helper to rasterize a triangle in UV space for RGBA buffers
    #[allow(dead_code)]
    fn rasterize_rgba_triangle(
        buf: &mut RgbaBuffer,
        uv0: Vec2,
        uv1: Vec2,
        uv2: Vec2,
        c0: Vec4,
        c1: Vec4,
        c2: Vec4,
    ) {
        let w = buf.width as f32;
        let h = buf.height as f32;

        let p0 = Vec2::new(uv0.x * w, uv0.y * h);
        let p1 = Vec2::new(uv1.x * w, uv1.y * h);
        let p2 = Vec2::new(uv2.x * w, uv2.y * h);

        let min_x = (p0.x.min(p1.x).min(p2.x).floor() as i32).max(0);
        let max_x = (p0.x.max(p1.x).max(p2.x).ceil() as i32).min(buf.width as i32 - 1);
        let min_y = (p0.y.min(p1.y).min(p2.y).floor() as i32).max(0);
        let max_y = (p0.y.max(p1.y).max(p2.y).ceil() as i32).min(buf.height as i32 - 1);

        let denom = (p1.y - p2.y) * (p0.x - p2.x) + (p2.x - p1.x) * (p0.y - p2.y);
        if denom.abs() < 1e-5 {
            return;
        }

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;

                let w0 = ((p1.y - p2.y) * (px - p2.x) + (p2.x - p1.x) * (py - p2.y)) / denom;
                let w1 = ((p2.y - p0.y) * (px - p2.x) + (p0.x - p2.x) * (py - p2.y)) / denom;
                let w2 = 1.0 - w0 - w1;

                if w0 >= -0.01 && w1 >= -0.01 && w2 >= -0.01 {
                    let col = c0 * w0 + c1 * w1 + c2 * w2;
                    buf.set_pixel(x as u32, y as u32, col);
                }
            }
        }
    }

    /// Multi-pass edge-preserving bilateral denoiser for scalar baked maps (AO, Curvature, Thickness)
    /// Smooths Monte Carlo noise and polygon faceting while preserving sharp geometric edges
    pub fn denoise_scalar_buffer(
        buf: &mut ScalarBuffer,
        normal_buf: Option<&RgbaBuffer>,
        iterations: usize,
        radius: i32,
        spatial_sigma: f32,
        normal_power: f32,
    ) {
        let w = buf.width as i32;
        let h = buf.height as i32;
        if w <= 1 || h <= 1 || iterations == 0 || radius <= 0 {
            return;
        }

        let two_sigma_sq = (2.0 * spatial_sigma * spatial_sigma).max(1e-4);

        // Precompute spatial weights
        let mut kernel_offsets = Vec::new();
        let mut kernel_weights = Vec::new();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let dist_sq = (dx * dx + dy * dy) as f32;
                if dist_sq <= (radius * radius) as f32 {
                    let w_spatial = (-dist_sq / two_sigma_sq).exp();
                    kernel_offsets.push((dx, dy));
                    kernel_weights.push(w_spatial);
                }
            }
        }

        for _ in 0..iterations {
            let current_data = buf.data.clone();

            buf.data
                .par_chunks_mut(w as usize)
                .enumerate()
                .for_each(|(y_idx, row)| {
                    let y = y_idx as i32;
                    for x in 0..w {
                        let center_idx = (y * w + x) as usize;
                        let center_val = current_data[center_idx];

                        let center_normal = if let Some(normals) = normal_buf {
                            let n_col = normals.data[center_idx];
                            let n = n_col.truncate() * 2.0 - Vec3::ONE;
                            if n.length_squared() > 0.01 {
                                Some(n.normalize())
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let mut total_val = center_val * 2.0;
                        let mut total_weight = 2.0f32;

                        for (&(dx, dy), &spatial_w) in kernel_offsets.iter().zip(kernel_weights.iter()) {
                            let nx = x + dx;
                            let ny = y + dy;
                            if nx >= 0 && nx < w && ny >= 0 && ny < h {
                                let neighbor_idx = (ny * w + nx) as usize;
                                let neighbor_val = current_data[neighbor_idx];

                                let normal_w = if let (Some(cn), Some(normals)) = (center_normal, normal_buf) {
                                    let nn_col = normals.data[neighbor_idx];
                                    let nn = nn_col.truncate() * 2.0 - Vec3::ONE;
                                    if nn.length_squared() > 0.01 {
                                        (cn.dot(nn.normalize()).max(0.0)).powf(normal_power)
                                    } else {
                                        0.0
                                    }
                                } else {
                                    1.0
                                };

                                let weight = spatial_w * normal_w;
                                if weight > 0.0001 {
                                    total_val += neighbor_val * weight;
                                    total_weight += weight;
                                }
                            }
                        }

                        row[x as usize] = total_val / total_weight;
                    }
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baker_curvature_cube() {
        let cube = Mesh::create_cube_mesh("Cube", 1.0);
        let curv = Baker::bake_curvature(&cube, 64, 64);

        let mut min_val = 1.0f32;
        let mut max_val = 0.0f32;
        for &v in &curv.data {
            assert!(!v.is_nan() && !v.is_infinite(), "Curvature must not contain NaN or Inf");
            min_val = min_val.min(v);
            max_val = max_val.max(v);
        }

        assert!(
            max_val > 0.6,
            "Convex edges of cube must produce bright curvature highlight, got {}",
            max_val
        );
    }

    #[test]
    fn test_baker_tangent_normals_flat_fallback() {
        let sphere = Mesh::create_sphere(1.0, 16, 12);
        let nmap = Baker::bake_tangent_normals(&sphere, None, 0.1, 0.1, 32, 32);

        // Every texel must have flat tangent normal (0.5, 0.5, 1.0, 1.0)
        for &px in &nmap.data {
            assert!((px.x - 0.5).abs() < 1e-4, "Expected tangent normal X=0.5, got {}", px.x);
            assert!((px.y - 0.5).abs() < 1e-4, "Expected tangent normal Y=0.5, got {}", px.y);
            assert!((px.z - 1.0).abs() < 1e-4, "Expected tangent normal Z=1.0, got {}", px.z);
        }
    }

    #[test]
    fn test_baker_position_wpo() {
        let cube = Mesh::create_cube_mesh("Cube", 2.0);
        let pos_map = Baker::bake_position(&cube, 32, 32);

        let mut min_val = Vec3::splat(1.0);
        let mut max_val = Vec3::splat(0.0);
        for &px in &pos_map.data {
            assert!(!px.x.is_nan() && !px.y.is_nan() && !px.z.is_nan());
            let v = Vec3::new(px.x, px.y, px.z);
            min_val = min_val.min(v);
            max_val = max_val.max(v);
        }

        // Bounding box normalized coordinates span [0, 1] across all axes
        assert!(min_val.x <= 0.05 && max_val.x >= 0.95, "X axis must span [0, 1]");
        assert!(min_val.y <= 0.05 && max_val.y >= 0.95, "Y axis must span [0, 1]");
        assert!(min_val.z <= 0.05 && max_val.z >= 0.95, "Z axis must span [0, 1]");
    }

    #[test]
    fn test_baker_world_normals() {
        let cube = Mesh::create_cube_mesh("Cube", 1.0);
        let wn_map = Baker::bake_world_space_normal(&cube, 32, 32);

        for &px in &wn_map.data {
            assert!(!px.x.is_nan() && !px.y.is_nan() && !px.z.is_nan());
            // Decoded normals should have length close to 1
            let n = Vec3::new(px.x * 2.0 - 1.0, px.y * 2.0 - 1.0, px.z * 2.0 - 1.0);
            assert!((n.length() - 1.0).abs() < 0.2, "Decoded normal length must be near 1, got {}", n.length());
        }
    }

    #[test]
    fn test_baker_thickness() {
        let sphere = Mesh::create_sphere(1.0, 16, 12);
        let thick_map = Baker::bake_thickness(&sphere, 32, 32);

        for &val in &thick_map.data {
            assert!(!val.is_nan() && !val.is_infinite());
            assert!(val >= 0.0 && val <= 1.0, "Thickness must be normalized in [0, 1]");
        }
    }

    #[test]
    fn test_baker_ao_sphere_smoothness() {
        let sphere = Mesh::create_sphere(1.0, 24, 16);
        let ao_map = Baker::bake_ao(&sphere, 32, 32);

        // A smooth sphere in open space has no concave occluders, so AO values must be >= 0.95 everywhere
        for &val in &ao_map.data {
            assert!(!val.is_nan() && !val.is_infinite());
            assert!(val >= 0.95, "Smooth sphere must not suffer facet self-occlusion, got {}", val);
        }
    }
}
