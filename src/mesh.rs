// 3D Paintcraft - 3D Mesh, Geometry, BVH Acceleration, Raycasting & Built-in Models
// Pure safe Rust for Linux creative workflows

use glam::{Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tangent: Vec4,
    pub uv: Vec2,
}

#[derive(Clone, Debug)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct RayHit {
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub triangle_idx: usize,
    pub barycentric: Vec3,
}

/// AABB Bounding Box helper
#[inline]
pub fn sphere_intersects_aabb(center: Vec3, radius: f32, aabb_min: Vec3, aabb_max: Vec3) -> bool {
    let closest = center.clamp(aabb_min, aabb_max);
    (center - closest).length_squared() <= radius * radius
}

pub fn ray_intersect_aabb(min: Vec3, max: Vec3, ray: &Ray) -> Option<f32> {
    let inv_d = Vec3::new(
        if ray.direction.x.abs() > 1e-8 { 1.0 / ray.direction.x } else { 1e8 },
        if ray.direction.y.abs() > 1e-8 { 1.0 / ray.direction.y } else { 1e8 },
        if ray.direction.z.abs() > 1e-8 { 1.0 / ray.direction.z } else { 1e8 },
    );

    let t0 = (min - ray.origin) * inv_d;
    let t1 = (max - ray.origin) * inv_d;

    let tmin = t0.min(t1);
    let tmax = t0.max(t1);

    let tenter = tmin.x.max(tmin.y).max(tmin.z);
    let texit = tmax.x.min(tmax.y).min(tmax.z);

    if tenter <= texit && texit >= 0.0 {
        Some(tenter.max(0.0))
    } else {
        None
    }
}

/// Bounding Volume Hierarchy Node for ultra-fast raycasting
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BvhNode {
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub left_child: Option<usize>,
    pub right_child: Option<usize>,
    pub triangle_indices: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bvh {
    pub nodes: Vec<BvhNode>,
}

impl Bvh {
    pub fn build(vertices: &[Vertex], indices: &[u32]) -> Self {
        let tri_count = indices.len() / 3;
        let mut tri_list: Vec<u32> = (0..tri_count as u32).collect();

        let mut nodes = Vec::new();
        if tri_count > 0 {
            Self::build_recursive(&mut nodes, vertices, indices, &mut tri_list);
        }

        Self { nodes }
    }

    fn build_recursive(
        nodes: &mut Vec<BvhNode>,
        vertices: &[Vertex],
        indices: &[u32],
        tri_indices: &mut [u32],
    ) -> usize {
        let node_idx = nodes.len();
        nodes.push(BvhNode {
            aabb_min: Vec3::splat(f32::MAX),
            aabb_max: Vec3::splat(f32::MIN),
            left_child: None,
            right_child: None,
            triangle_indices: Vec::new(),
        });

        // Compute AABB for this node
        let mut min_pos = Vec3::splat(f32::MAX);
        let mut max_pos = Vec3::splat(f32::MIN);
        let mut centroid_min = Vec3::splat(f32::MAX);
        let mut centroid_max = Vec3::splat(f32::MIN);

        for &t_idx in tri_indices.iter() {
            let i0 = indices[t_idx as usize * 3] as usize;
            let i1 = indices[t_idx as usize * 3 + 1] as usize;
            let i2 = indices[t_idx as usize * 3 + 2] as usize;

            let p0 = vertices[i0].position;
            let p1 = vertices[i1].position;
            let p2 = vertices[i2].position;

            min_pos = min_pos.min(p0).min(p1).min(p2);
            max_pos = max_pos.max(p0).max(p1).max(p2);

            let centroid = (p0 + p1 + p2) / 3.0;
            centroid_min = centroid_min.min(centroid);
            centroid_max = centroid_max.max(centroid);
        }

        nodes[node_idx].aabb_min = min_pos;
        nodes[node_idx].aabb_max = max_pos;

        // Leaf threshold: <= 8 triangles
        if tri_indices.len() <= 8 {
            nodes[node_idx].triangle_indices = tri_indices.to_vec();
            return node_idx;
        }

        // Find longest centroid axis
        let extents = centroid_max - centroid_min;
        let axis = if extents.x >= extents.y && extents.x >= extents.z {
            0
        } else if extents.y >= extents.z {
            1
        } else {
            2
        };

        // Sort triangles along chosen axis
        tri_indices.sort_unstable_by(|&a, &b| {
            let get_c = |t: u32| {
                let i0 = indices[t as usize * 3] as usize;
                let i1 = indices[t as usize * 3 + 1] as usize;
                let i2 = indices[t as usize * 3 + 2] as usize;
                (vertices[i0].position + vertices[i1].position + vertices[i2].position) / 3.0
            };
            let ca = get_c(a);
            let cb = get_c(b);
            let val_a = if axis == 0 { ca.x } else if axis == 1 { ca.y } else { ca.z };
            let val_b = if axis == 0 { cb.x } else if axis == 1 { cb.y } else { cb.z };
            val_a.partial_cmp(&val_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mid = tri_indices.len() / 2;
        let (left_slice, right_slice) = tri_indices.split_at_mut(mid);

        let left_child = Self::build_recursive(nodes, vertices, indices, left_slice);
        let right_child = Self::build_recursive(nodes, vertices, indices, right_slice);

        nodes[node_idx].left_child = Some(left_child);
        nodes[node_idx].right_child = Some(right_child);

        node_idx
    }

    pub fn query_sphere(&self, center: Vec3, radius: f32) -> Vec<u32> {
        let mut result = Vec::new();
        if self.nodes.is_empty() {
            return result;
        }

        let mut stack = Vec::with_capacity(32);
        stack.push(0usize);

        while let Some(node_idx) = stack.pop() {
            let node = &self.nodes[node_idx];

            if sphere_intersects_aabb(center, radius, node.aabb_min, node.aabb_max) {
                if !node.triangle_indices.is_empty() {
                    result.extend_from_slice(&node.triangle_indices);
                }
                if let Some(left) = node.left_child {
                    stack.push(left);
                }
                if let Some(right) = node.right_child {
                    stack.push(right);
                }
            }
        }

        result
    }

    pub fn raycast(&self, ray: &Ray, vertices: &[Vertex], indices: &[u32]) -> Option<RayHit> {
        if self.nodes.is_empty() {
            return None;
        }

        let mut closest_hit: Option<RayHit> = None;
        let mut min_t = f32::MAX;

        // Traverse stack
        let mut stack = Vec::with_capacity(32);
        stack.push(0usize);

        while let Some(node_idx) = stack.pop() {
            let node = &self.nodes[node_idx];

            if let Some(t_box) = ray_intersect_aabb(node.aabb_min, node.aabb_max, ray) {
                if t_box >= min_t {
                    continue;
                }

                // If leaf node, test triangles
                if !node.triangle_indices.is_empty() {
                    for &t_idx in &node.triangle_indices {
                        let i0 = indices[t_idx as usize * 3] as usize;
                        let i1 = indices[t_idx as usize * 3 + 1] as usize;
                        let i2 = indices[t_idx as usize * 3 + 2] as usize;

                        let v0 = &vertices[i0];
                        let v1 = &vertices[i1];
                        let v2 = &vertices[i2];

                        let edge1 = v1.position - v0.position;
                        let edge2 = v2.position - v0.position;

                        let h = ray.direction.cross(edge2);
                        let a = edge1.dot(h);

                        if a.abs() < 1e-7 {
                            continue;
                        }

                        let f = 1.0 / a;
                        let s = ray.origin - v0.position;
                        let u = f * s.dot(h);

                        if u < 0.0 || u > 1.0 {
                            continue;
                        }

                        let q = s.cross(edge1);
                        let v = f * ray.direction.dot(q);

                        if v < 0.0 || u + v > 1.0 {
                            continue;
                        }

                        let t = f * edge2.dot(q);
                        if t > 1e-4 && t < min_t {
                            min_t = t;
                            let w = 1.0 - u - v;

                            let point = ray.origin + ray.direction * t;
                            let normal = (v0.normal * w + v1.normal * u + v2.normal * v).normalize_or_zero();
                            let uv = v0.uv * w + v1.uv * u + v2.uv * v;

                            closest_hit = Some(RayHit {
                                t,
                                point,
                                normal,
                                uv,
                                triangle_idx: t_idx as usize,
                                barycentric: Vec3::new(w, u, v),
                            });
                        }
                    }
                } else {
                    // Internal node: push children
                    if let Some(r) = node.right_child {
                        stack.push(r);
                    }
                    if let Some(l) = node.left_child {
                        stack.push(l);
                    }
                }
            }
        }

        closest_hit
    }
}

fn default_mesh_scale() -> f32 {
    1.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mesh {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub bvh: Bvh,
    #[serde(default)]
    pub original_center: Vec3,
    #[serde(default = "default_mesh_scale")]
    pub original_scale: f32,
}

impl Mesh {
    pub fn new(name: String, vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        let mut mesh = Self {
            name,
            vertices,
            indices,
            aabb_min: Vec3::splat(f32::MAX),
            aabb_max: Vec3::splat(f32::MIN),
            bvh: Bvh { nodes: Vec::new() },
            original_center: Vec3::ZERO,
            original_scale: 1.0,
        };
        mesh.recompute_bounds();
        mesh.ensure_tangents();
        mesh.rebuild_bvh();
        mesh
    }

    pub fn rebuild_bvh(&mut self) {
        self.bvh = Bvh::build(&self.vertices, &self.indices);
    }

    /// Generate LineList index buffer for clean wireframe rendering
    pub fn generate_wireframe_indices(&self) -> Vec<u32> {
        let mut seen_edges = std::collections::HashSet::with_capacity(self.indices.len());
        let mut wire_indices = Vec::with_capacity(self.indices.len() * 2);
        for chunk in self.indices.chunks_exact(3) {
            let (i0, i1, i2) = (chunk[0], chunk[1], chunk[2]);
            for &(a, b) in &[(i0, i1), (i1, i2), (i2, i0)] {
                let key = if a < b { (a, b) } else { (b, a) };
                if seen_edges.insert(key) {
                    wire_indices.push(a);
                    wire_indices.push(b);
                }
            }
        }
        wire_indices
    }

    pub fn recompute_bounds(&mut self) {
        if self.vertices.is_empty() {
            self.aabb_min = Vec3::ZERO;
            self.aabb_max = Vec3::ZERO;
            return;
        }

        let mut min_pos = Vec3::splat(f32::MAX);
        let mut max_pos = Vec3::splat(f32::MIN);

        for v in &self.vertices {
            min_pos = min_pos.min(v.position);
            max_pos = max_pos.max(v.position);
        }

        self.aabb_min = min_pos;
        self.aabb_max = max_pos;
    }

    /// Center and normalize mesh to fit nicely within a [-1.0, 1.0] bounding box
    #[allow(dead_code)]
    pub fn normalize_to_unit_box(&mut self) {
        self.recompute_bounds();
        let center = (self.aabb_min + self.aabb_max) * 0.5;
        let extents = self.aabb_max - self.aabb_min;
        let max_extent = extents.x.max(extents.y).max(extents.z);
        let scale = if max_extent > 1e-4 { 2.0 / max_extent } else { 1.0 };

        self.original_center = center;
        self.original_scale = scale;

        for v in &mut self.vertices {
            v.position = (v.position - center) * scale;
        }
        self.recompute_bounds();
        self.rebuild_bvh();
    }

    /// Compute tangents if missing or default
    pub fn ensure_tangents(&mut self) {
        let has_valid_tangents = self.vertices.iter().all(|v| v.tangent.truncate().length_squared() > 0.5);
        if has_valid_tangents && !self.vertices.is_empty() {
            return;
        }

        let tri_count = self.indices.len() / 3;
        let mut tan1 = vec![Vec3::ZERO; self.vertices.len()];
        let mut tan2 = vec![Vec3::ZERO; self.vertices.len()];

        for i in 0..tri_count {
            let i0 = self.indices[i * 3] as usize;
            let i1 = self.indices[i * 3 + 1] as usize;
            let i2 = self.indices[i * 3 + 2] as usize;

            let v0 = &self.vertices[i0];
            let v1 = &self.vertices[i1];
            let v2 = &self.vertices[i2];

            let edge1 = v1.position - v0.position;
            let edge2 = v2.position - v0.position;

            let delta_u1 = v1.uv.x - v0.uv.x;
            let delta_v1 = v1.uv.y - v0.uv.y;
            let delta_u2 = v2.uv.x - v0.uv.x;
            let delta_v2 = v2.uv.y - v0.uv.y;

            let det = delta_u1 * delta_v2 - delta_u2 * delta_v1;
            if det.abs() < 1e-7 {
                continue;
            }
            let f = 1.0 / det;

            let sdir = Vec3::new(
                f * (delta_v2 * edge1.x - delta_v1 * edge2.x),
                f * (delta_v2 * edge1.y - delta_v1 * edge2.y),
                f * (delta_v2 * edge1.z - delta_v1 * edge2.z),
            );

            let tdir = Vec3::new(
                f * (delta_u1 * edge2.x - delta_u2 * edge1.x),
                f * (delta_u1 * edge2.y - delta_u2 * edge1.y),
                f * (delta_u1 * edge2.z - delta_u2 * edge1.z),
            );

            tan1[i0] += sdir;
            tan1[i1] += sdir;
            tan1[i2] += sdir;

            tan2[i0] += tdir;
            tan2[i1] += tdir;
            tan2[i2] += tdir;
        }

        for (i, v) in self.vertices.iter_mut().enumerate() {
            let n = v.normal;
            let t = tan1[i];

            // Gram-Schmidt orthogonalize
            let mut tangent_xyz = (t - n * n.dot(t)).normalize_or_zero();
            if tangent_xyz.length_squared() < 1e-4 {
                let up = if n.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
                tangent_xyz = n.cross(up).normalize_or_zero();
            }
            let w = if n.cross(tangent_xyz).dot(tan2[i]) < 0.0 { -1.0 } else { 1.0 };
            v.tangent = Vec4::new(tangent_xyz.x, tangent_xyz.y, tangent_xyz.z, w);
        }
    }

    /// BVH-Accelerated Raycast: O(log N) complexity for instantaneous 3D painting
    pub fn query_sphere(&self, center: Vec3, radius: f32) -> Vec<u32> {
        self.bvh.query_sphere(center, radius)
    }

    pub fn raycast(&self, ray: &Ray) -> Option<RayHit> {
        self.bvh.raycast(ray, &self.vertices, &self.indices)
    }

    /// Find which triangle in UV space contains the given (u, v) coordinates
    pub fn find_triangle_at_uv(&self, uv: Vec2) -> Option<usize> {
        let tri_count = self.indices.len() / 3;
        for t in 0..tri_count {
            let i0 = self.indices[t * 3] as usize;
            let i1 = self.indices[t * 3 + 1] as usize;
            let i2 = self.indices[t * 3 + 2] as usize;
            let uv0 = self.vertices[i0].uv;
            let uv1 = self.vertices[i1].uv;
            let uv2 = self.vertices[i2].uv;
            let denom = (uv1.y - uv2.y) * (uv0.x - uv2.x) + (uv2.x - uv1.x) * (uv0.y - uv2.y);
            if denom.abs() < 1e-7 {
                continue;
            }
            let inv = 1.0 / denom;
            let w0 = ((uv1.y - uv2.y) * (uv.x - uv2.x) + (uv2.x - uv1.x) * (uv.y - uv2.y)) * inv;
            let w1 = ((uv2.y - uv0.y) * (uv.x - uv2.x) + (uv0.x - uv2.x) * (uv.y - uv2.y)) * inv;
            let w2 = 1.0 - w0 - w1;
            if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                return Some(t);
            }
        }
        None
    }

    /// Flood fill all triangles connected within the same UV island (no crossing UV seams)
    pub fn get_connected_uv_island(&self, start_tri: usize) -> Vec<usize> {
        let tri_count = self.indices.len() / 3;
        if start_tri >= tri_count {
            return Vec::new();
        }

        type EdgeKey = ([i32; 3], [i32; 2], [i32; 3], [i32; 2]);
        let quant_v = |v: &Vertex| -> ([i32; 3], [i32; 2]) {
            (
                [(v.position.x * 1000.0).round() as i32, (v.position.y * 1000.0).round() as i32, (v.position.z * 1000.0).round() as i32],
                [(v.uv.x * 10000.0).round() as i32, (v.uv.y * 10000.0).round() as i32],
            )
        };
        let make_edge = |k0: ([i32; 3], [i32; 2]), k1: ([i32; 3], [i32; 2])| -> EdgeKey {
            if k0 < k1 {
                (k0.0, k0.1, k1.0, k1.1)
            } else {
                (k1.0, k1.1, k0.0, k0.1)
            }
        };

        let mut edge_map: std::collections::HashMap<EdgeKey, Vec<usize>> = std::collections::HashMap::with_capacity(tri_count * 3);
        for t in 0..tri_count {
            let i0 = self.indices[t * 3] as usize;
            let i1 = self.indices[t * 3 + 1] as usize;
            let i2 = self.indices[t * 3 + 2] as usize;
            let k0 = quant_v(&self.vertices[i0]);
            let k1 = quant_v(&self.vertices[i1]);
            let k2 = quant_v(&self.vertices[i2]);
            edge_map.entry(make_edge(k0, k1)).or_default().push(t);
            edge_map.entry(make_edge(k1, k2)).or_default().push(t);
            edge_map.entry(make_edge(k2, k0)).or_default().push(t);
        }

        let mut visited = vec![false; tri_count];
        let mut result = Vec::new();
        let mut queue = std::collections::VecDeque::new();

        visited[start_tri] = true;
        queue.push_back(start_tri);

        while let Some(curr) = queue.pop_front() {
            result.push(curr);
            let i0 = self.indices[curr * 3] as usize;
            let i1 = self.indices[curr * 3 + 1] as usize;
            let i2 = self.indices[curr * 3 + 2] as usize;
            let k0 = quant_v(&self.vertices[i0]);
            let k1 = quant_v(&self.vertices[i1]);
            let k2 = quant_v(&self.vertices[i2]);

            for edge in [make_edge(k0, k1), make_edge(k1, k2), make_edge(k2, k0)] {
                if let Some(neighbors) = edge_map.get(&edge) {
                    for &nbr in neighbors {
                        if !visited[nbr] {
                            visited[nbr] = true;
                            queue.push_back(nbr);
                        }
                    }
                }
            }
        }

        result
    }

    /// Flood fill all triangles connected within the same 3D mesh element (regardless of UV seams)
    pub fn get_connected_mesh_element(&self, start_tri: usize) -> Vec<usize> {
        let tri_count = self.indices.len() / 3;
        if start_tri >= tri_count {
            return Vec::new();
        }

        type PosKey = ([i32; 3], [i32; 3]);
        let quant_p = |v: &Vertex| -> [i32; 3] {
            [(v.position.x * 1000.0).round() as i32, (v.position.y * 1000.0).round() as i32, (v.position.z * 1000.0).round() as i32]
        };
        let make_pos_edge = |p0: [i32; 3], p1: [i32; 3]| -> PosKey {
            if p0 < p1 { (p0, p1) } else { (p1, p0) }
        };

        let mut edge_map: std::collections::HashMap<PosKey, Vec<usize>> = std::collections::HashMap::with_capacity(tri_count * 3);
        for t in 0..tri_count {
            let i0 = self.indices[t * 3] as usize;
            let i1 = self.indices[t * 3 + 1] as usize;
            let i2 = self.indices[t * 3 + 2] as usize;
            let p0 = quant_p(&self.vertices[i0]);
            let p1 = quant_p(&self.vertices[i1]);
            let p2 = quant_p(&self.vertices[i2]);
            edge_map.entry(make_pos_edge(p0, p1)).or_default().push(t);
            edge_map.entry(make_pos_edge(p1, p2)).or_default().push(t);
            edge_map.entry(make_pos_edge(p2, p0)).or_default().push(t);
        }

        let mut visited = vec![false; tri_count];
        let mut result = Vec::new();
        let mut queue = std::collections::VecDeque::new();

        visited[start_tri] = true;
        queue.push_back(start_tri);

        while let Some(curr) = queue.pop_front() {
            result.push(curr);
            let i0 = self.indices[curr * 3] as usize;
            let i1 = self.indices[curr * 3 + 1] as usize;
            let i2 = self.indices[curr * 3 + 2] as usize;
            let p0 = quant_p(&self.vertices[i0]);
            let p1 = quant_p(&self.vertices[i1]);
            let p2 = quant_p(&self.vertices[i2]);

            for edge in [make_pos_edge(p0, p1), make_pos_edge(p1, p2), make_pos_edge(p2, p0)] {
                if let Some(neighbors) = edge_map.get(&edge) {
                    for &nbr in neighbors {
                        if !visited[nbr] {
                            visited[nbr] = true;
                            queue.push_back(nbr);
                        }
                    }
                }
            }
        }

        result
    }

    /// Half-space rasterizer for filling a triangle in 2D UV space into a buffer
    pub fn rasterize_triangle_uv<F: FnMut(u32, u32)>(
        w: u32,
        h: u32,
        uv0: Vec2,
        uv1: Vec2,
        uv2: Vec2,
        mut set_pixel: F,
    ) {
        let p0 = Vec2::new(uv0.x * w as f32, uv0.y * h as f32);
        let p1 = Vec2::new(uv1.x * w as f32, uv1.y * h as f32);
        let p2 = Vec2::new(uv2.x * w as f32, uv2.y * h as f32);

        let min_x = (p0.x.min(p1.x).min(p2.x).floor() as i32).max(0).min(w as i32 - 1) as usize;
        let max_x = (p0.x.max(p1.x).max(p2.x).ceil() as i32).max(0).min(w as i32 - 1) as usize;
        let min_y = (p0.y.min(p1.y).min(p2.y).floor() as i32).max(0).min(h as i32 - 1) as usize;
        let max_y = (p0.y.max(p1.y).max(p2.y).ceil() as i32).max(0).min(h as i32 - 1) as usize;

        let denom = (p1.y - p2.y) * (p0.x - p2.x) + (p2.x - p1.x) * (p0.y - p2.y);
        if denom.abs() < 1e-6 {
            return;
        }
        let inv_denom = 1.0 / denom;

        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let w0 = ((p1.y - p2.y) * (px - p2.x) + (p2.x - p1.x) * (py - p2.y)) * inv_denom;
                let w1 = ((p2.y - p0.y) * (px - p2.x) + (p0.x - p2.x) * (py - p2.y)) * inv_denom;
                let w2 = 1.0 - w0 - w1;

                if w0 >= -0.001 && w1 >= -0.001 && w2 >= -0.001 {
                    set_pixel(x as u32, y as u32);
                }
            }
        }
    }

    pub fn from_obj_str(content: &str, name: &str) -> Result<Self, String> {
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut normals = Vec::new();
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() { continue; }
            match parts[0] {
                "v" => {
                    positions.push(Vec3::new(
                        parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                        parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                        parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    ));
                }
                "vt" => {
                    uvs.push(Vec2::new(
                        parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                        parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    ));
                }
                "vn" => {
                    normals.push(Vec3::new(
                        parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                        parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                        parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    ));
                }
                "f" => {
                    let mut face_verts = Vec::new();
                    for i in 1..parts.len() {
                        let mut p_idx = 0;
                        let mut t_idx = 0;
                        let mut n_idx = 0;
                        
                        let face_parts: Vec<&str> = parts[i].split('/').collect();
                        if face_parts.len() > 0 && !face_parts[0].is_empty() {
                            p_idx = face_parts[0].parse::<usize>().unwrap_or(1).saturating_sub(1);
                        }
                        if face_parts.len() > 1 && !face_parts[1].is_empty() {
                            t_idx = face_parts[1].parse::<usize>().unwrap_or(1).saturating_sub(1);
                        }
                        if face_parts.len() > 2 && !face_parts[2].is_empty() {
                            n_idx = face_parts[2].parse::<usize>().unwrap_or(1).saturating_sub(1);
                        }
                        
                        let idx = vertices.len() as u32;
                        vertices.push(Vertex {
                            position: *positions.get(p_idx).unwrap_or(&Vec3::ZERO),
                            uv: *uvs.get(t_idx).unwrap_or(&Vec2::ZERO),
                            normal: *normals.get(n_idx).unwrap_or(&Vec3::ZERO),
                            tangent: Vec4::ZERO,
                        });
                        face_verts.push(idx);
                    }
                    
                    if face_verts.len() >= 3 {
                        for i in 1..(face_verts.len() - 1) {
                            indices.push(face_verts[0]);
                            indices.push(face_verts[i]);
                            indices.push(face_verts[i + 1]);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut mesh = Self::new(name.to_string(), vertices, indices);

        // If normals are missing or degenerate, compute from triangle topology
        let has_valid_normals = mesh.vertices.iter().any(|v| v.normal.length_squared() > 1e-4);
        if !has_valid_normals {
            let mut computed_normals = vec![Vec3::ZERO; mesh.vertices.len()];
            let tri_count = mesh.indices.len() / 3;
            for i in 0..tri_count {
                let i0 = mesh.indices[i * 3] as usize;
                let i1 = mesh.indices[i * 3 + 1] as usize;
                let i2 = mesh.indices[i * 3 + 2] as usize;
                let v0 = mesh.vertices[i0].position;
                let v1 = mesh.vertices[i1].position;
                let v2 = mesh.vertices[i2].position;
                let fnorm = (v1 - v0).cross(v2 - v0);
                if fnorm.length_squared() > 1e-8 {
                    let n = fnorm.normalize();
                    computed_normals[i0] += n;
                    computed_normals[i1] += n;
                    computed_normals[i2] += n;
                }
            }
            for (v, cn) in mesh.vertices.iter_mut().zip(computed_normals.iter()) {
                if cn.length_squared() > 1e-6 {
                    v.normal = cn.normalize();
                } else {
                    v.normal = Vec3::Y;
                }
            }
        }

        // If UVs are missing (e.g. all 0), generate automatic box projection UVs
        let has_valid_uvs = mesh.vertices.iter().any(|v| v.uv.length_squared() > 1e-6);
        if !has_valid_uvs {
            for v in &mut mesh.vertices {
                let p = v.position;
                let n = v.normal.abs();
                if n.x >= n.y && n.x >= n.z {
                    v.uv = Vec2::new((p.z + 1.0) * 0.5, (p.y + 1.0) * 0.5);
                } else if n.y >= n.x && n.y >= n.z {
                    v.uv = Vec2::new((p.x + 1.0) * 0.5, (p.z + 1.0) * 0.5);
                } else {
                    v.uv = Vec2::new((p.x + 1.0) * 0.5, (p.y + 1.0) * 0.5);
                }
            }
        }

        mesh.normalize_to_unit_box();
        mesh.ensure_tangents();
        mesh.rebuild_bvh();

        Ok(mesh)
    }

    /// Load 3D model from file (auto-detects .obj, .gltf, .glb)
    pub fn load_file(path: &std::path::Path) -> Result<Self, String> {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        if ext == "obj" {
            let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read OBJ file: {}", e))?;
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Model");
            Self::from_obj_str(&content, name)
        } else if ext == "gltf" || ext == "glb" {
            Self::from_gltf_or_glb_file(path)
        } else {
            if let Ok(content) = std::fs::read_to_string(path) {
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Model");
                if let Ok(mesh) = Self::from_obj_str(&content, name) {
                    return Ok(mesh);
                }
            }
            Self::from_gltf_or_glb_file(path)
        }
    }

fn decode_base64_simple(input: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for b in input.bytes() {
        let val = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            b'=' => continue,
            _ => continue,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    out
}

    /// Load 3D model from glTF (JSON) or GLB (binary)
    pub fn from_gltf_or_glb_file(path: &std::path::Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Model");

        let (json_val, bin_buffers) = if bytes.starts_with(b"glTF") {
            if bytes.len() < 20 {
                return Err("GLB file too short".to_string());
            }
            let chunk0_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
            let chunk0_type = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
            if chunk0_type != 0x4E4F534A {
                return Err("Expected JSON chunk in GLB".to_string());
            }
            let json_slice = &bytes[20..20 + chunk0_len];
            let json: serde_json::Value = serde_json::from_slice(json_slice).map_err(|e| e.to_string())?;

            let mut bins = Vec::new();
            let mut offset = 20 + chunk0_len;
            while offset + 8 <= bytes.len() {
                let chunk_len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
                let chunk_type = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
                offset += 8;
                if chunk_type == 0x004E4942 && offset + chunk_len <= bytes.len() {
                    bins.push(bytes[offset..offset + chunk_len].to_vec());
                }
                offset += chunk_len;
            }
            (json, bins)
        } else {
            let json: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let mut bins = Vec::new();
            if let Some(buffers) = json.get("buffers").and_then(|b| b.as_array()) {
                for buf in buffers {
                    if let Some(uri) = buf.get("uri").and_then(|u| u.as_str()) {
                        if uri.starts_with("data:application/octet-stream;base64,") {
                            let b64 = &uri["data:application/octet-stream;base64,".len()..];
                            bins.push(Self::decode_base64_simple(b64));
                        } else {
                            let parent = path.parent().unwrap_or(std::path::Path::new("."));
                            let bin_path = parent.join(uri);
                            if let Ok(b) = std::fs::read(&bin_path) {
                                bins.push(b);
                            } else {
                                bins.push(Vec::new());
                            }
                        }
                    }
                }
            }
            (json, bins)
        };

        Self::build_mesh_from_gltf_json(&json_val, &bin_buffers, name)
    }

    fn build_mesh_from_gltf_json(
        json: &serde_json::Value,
        bins: &[Vec<u8>],
        name: &str,
    ) -> Result<Self, String> {
        let buffer_views = json.get("bufferViews").and_then(|v| v.as_array()).ok_or("Missing bufferViews")?;
        let accessors = json.get("accessors").and_then(|a| a.as_array()).ok_or("Missing accessors")?;

        let read_accessor_f32 = |acc_idx: usize, expected_dim: usize| -> Vec<Vec<f32>> {
            let acc = match accessors.get(acc_idx) {
                Some(a) => a,
                None => return Vec::new(),
            };
            let bv_idx = acc.get("bufferView").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let acc_offset = acc.get("byteOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let count = acc.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

            let bv = match buffer_views.get(bv_idx) {
                Some(v) => v,
                None => return Vec::new(),
            };
            let buf_idx = bv.get("buffer").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let bv_offset = bv.get("byteOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let stride = bv.get("byteStride").and_then(|v| v.as_u64()).map(|s| s as usize).unwrap_or(expected_dim * 4);

            let buffer = match bins.get(buf_idx) {
                Some(b) => b,
                None => return Vec::new(),
            };

            let mut result = Vec::with_capacity(count);
            let start = bv_offset + acc_offset;
            for i in 0..count {
                let elem_offset = start + i * stride;
                if elem_offset + expected_dim * 4 <= buffer.len() {
                    let mut vals = Vec::with_capacity(expected_dim);
                    for d in 0..expected_dim {
                        let b_off = elem_offset + d * 4;
                        let f = f32::from_le_bytes(buffer[b_off..b_off + 4].try_into().unwrap());
                        vals.push(f);
                    }
                    result.push(vals);
                }
            }
            result
        };

        let read_indices = |acc_idx: usize| -> Vec<u32> {
            let acc = match accessors.get(acc_idx) {
                Some(a) => a,
                None => return Vec::new(),
            };
            let bv_idx = acc.get("bufferView").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let acc_offset = acc.get("byteOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let count = acc.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let comp_type = acc.get("componentType").and_then(|v| v.as_u64()).unwrap_or(5125);

            let bv = match buffer_views.get(bv_idx) {
                Some(v) => v,
                None => return Vec::new(),
            };
            let buf_idx = bv.get("buffer").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let bv_offset = bv.get("byteOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let buffer = match bins.get(buf_idx) {
                Some(b) => b,
                None => return Vec::new(),
            };

            let mut result = Vec::with_capacity(count);
            let start = bv_offset + acc_offset;

            match comp_type {
                5121 => {
                    for i in 0..count {
                        if start + i < buffer.len() {
                            result.push(buffer[start + i] as u32);
                        }
                    }
                }
                5123 => {
                    for i in 0..count {
                        let off = start + i * 2;
                        if off + 2 <= buffer.len() {
                            let idx = u16::from_le_bytes(buffer[off..off + 2].try_into().unwrap()) as u32;
                            result.push(idx);
                        }
                    }
                }
                5125 => {
                    for i in 0..count {
                        let off = start + i * 4;
                        if off + 4 <= buffer.len() {
                            let idx = u32::from_le_bytes(buffer[off..off + 4].try_into().unwrap());
                            result.push(idx);
                        }
                    }
                }
                _ => {}
            }
            result
        };

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let meshes_arr = json.get("meshes").and_then(|m| m.as_array()).ok_or("No meshes in glTF")?;
        for mesh_obj in meshes_arr {
            if let Some(prims) = mesh_obj.get("primitives").and_then(|p| p.as_array()) {
                for prim in prims {
                    let attrs = match prim.get("attributes").and_then(|a| a.as_object()) {
                        Some(a) => a,
                        None => continue,
                    };

                    let pos_acc = match attrs.get("POSITION").and_then(|v| v.as_u64()) {
                        Some(p) => p as usize,
                        None => continue,
                    };
                    let positions_raw = read_accessor_f32(pos_acc, 3);
                    let normals_raw = attrs.get("NORMAL").and_then(|v| v.as_u64()).map(|n| read_accessor_f32(n as usize, 3)).unwrap_or_default();
                    let uvs_raw = attrs.get("TEXCOORD_0").and_then(|v| v.as_u64()).map(|u| read_accessor_f32(u as usize, 2)).unwrap_or_default();
                    let tangents_raw = attrs.get("TANGENT").and_then(|v| v.as_u64()).map(|t| read_accessor_f32(t as usize, 4)).unwrap_or_default();

                    let base_vert_idx = vertices.len() as u32;

                    for (i, p) in positions_raw.iter().enumerate() {
                        let pos = Vec3::new(p[0], p[1], p[2]);
                        let norm = if i < normals_raw.len() {
                            Vec3::new(normals_raw[i][0], normals_raw[i][1], normals_raw[i][2])
                        } else {
                            Vec3::Y
                        };
                        let uv = if i < uvs_raw.len() {
                            Vec2::new(uvs_raw[i][0], uvs_raw[i][1])
                        } else {
                            Vec2::ZERO
                        };
                        let tan = if i < tangents_raw.len() {
                            Vec4::new(tangents_raw[i][0], tangents_raw[i][1], tangents_raw[i][2], tangents_raw[i][3])
                        } else {
                            Vec4::ZERO
                        };

                        vertices.push(Vertex {
                            position: pos,
                            normal: norm,
                            uv,
                            tangent: tan,
                        });
                    }

                    if let Some(idx_acc) = prim.get("indices").and_then(|v| v.as_u64()) {
                        let prim_indices = read_indices(idx_acc as usize);
                        for idx in prim_indices {
                            indices.push(base_vert_idx + idx);
                        }
                    } else {
                        for i in 0..positions_raw.len() as u32 {
                            indices.push(base_vert_idx + i);
                        }
                    }
                }
            }
        }

        if vertices.is_empty() {
            return Err("No vertices extracted from glTF".to_string());
        }

        let mut mesh = Self::new(name.to_string(), vertices, indices);
        mesh.recompute_bounds();
        mesh.normalize_to_unit_box();
        mesh.ensure_tangents();
        mesh.rebuild_bvh();

        Ok(mesh)
    }

    pub fn create_sphere(radius: f32, segments: u32, rings: u32) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for i in 0..=rings {
            let v = i as f32 / rings as f32;
            let phi = v * std::f32::consts::PI;
            for j in 0..=segments {
                let u = j as f32 / segments as f32;
                let theta = u * std::f32::consts::PI * 2.0;

                let x = radius * phi.sin() * theta.cos();
                let y = radius * phi.cos();
                let z = radius * phi.sin() * theta.sin();

                let pos = Vec3::new(x, y, z);
                let norm = pos.normalize_or_zero();
                let tan = Vec4::new(-theta.sin(), 0.0, theta.cos(), 1.0);
                vertices.push(Vertex {
                    position: pos,
                    normal: norm,
                    tangent: tan,
                    uv: Vec2::new(u, v),
                });
            }
        }

        for i in 0..rings {
            for j in 0..segments {
                let first = i * (segments + 1) + j;
                let second = first + segments + 1;
                indices.push(first);
                indices.push(first + 1);
                indices.push(second);

                indices.push(second);
                indices.push(first + 1);
                indices.push(second + 1);
            }
        }

        let mut mesh = Self::new("Shader Ball".to_string(), vertices, indices);
        mesh.normalize_to_unit_box();
        mesh.ensure_tangents();
        mesh.rebuild_bvh();
        mesh
    }

    pub fn create_shader_ball() -> Self { 
        Self::create_sphere(1.0, 48, 32) 
    }

    #[allow(dead_code)]
    pub fn create_cube_mesh(name: &str, size: f32) -> Self {
        let h = size * 0.5;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // 6 faces: Normal, Tangent, 4 Corners (BL, BR, TR, TL)
        // Atlas UV coordinates in a 3x2 grid so all faces have unique non-overlapping UV space
        let faces = [
            // Front (+Z)
            (Vec3::new(0.0, 0.0, 1.0), Vec4::new(1.0, 0.0, 0.0, 1.0), [Vec3::new(-h, -h, h), Vec3::new(h, -h, h), Vec3::new(h, h, h), Vec3::new(-h, h, h)], (0.0/3.0, 0.0/2.0)),
            // Back (-Z)
            (Vec3::new(0.0, 0.0, -1.0), Vec4::new(-1.0, 0.0, 0.0, 1.0), [Vec3::new(h, -h, -h), Vec3::new(-h, -h, -h), Vec3::new(-h, h, -h), Vec3::new(h, h, -h)], (1.0/3.0, 0.0/2.0)),
            // Left (-X)
            (Vec3::new(-1.0, 0.0, 0.0), Vec4::new(0.0, 0.0, 1.0, 1.0), [Vec3::new(-h, -h, -h), Vec3::new(-h, -h, h), Vec3::new(-h, h, h), Vec3::new(-h, h, -h)], (2.0/3.0, 0.0/2.0)),
            // Right (+X)
            (Vec3::new(1.0, 0.0, 0.0), Vec4::new(0.0, 0.0, -1.0, 1.0), [Vec3::new(h, -h, h), Vec3::new(h, -h, -h), Vec3::new(h, h, -h), Vec3::new(h, h, h)], (0.0/3.0, 1.0/2.0)),
            // Top (+Y)
            (Vec3::new(0.0, 1.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0), [Vec3::new(-h, h, h), Vec3::new(h, h, h), Vec3::new(h, h, -h), Vec3::new(-h, h, -h)], (1.0/3.0, 1.0/2.0)),
            // Bottom (-Y)
            (Vec3::new(0.0, -1.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0), [Vec3::new(-h, -h, -h), Vec3::new(h, -h, -h), Vec3::new(h, -h, h), Vec3::new(-h, -h, h)], (2.0/3.0, 1.0/2.0)),
        ];

        let uw = 1.0 / 3.0;
        let vh = 1.0 / 2.0;

        for (norm, tan, corners, (u0, v0)) in &faces {
            let base_idx = vertices.len() as u32;
            let uvs = [
                Vec2::new(*u0, *v0),
                Vec2::new(*u0 + uw, *v0),
                Vec2::new(*u0 + uw, *v0 + vh),
                Vec2::new(*u0, *v0 + vh),
            ];

            for (p, uv) in corners.iter().zip(uvs.iter()) {
                vertices.push(Vertex {
                    position: *p,
                    normal: *norm,
                    tangent: *tan,
                    uv: *uv,
                });
            }

            indices.extend_from_slice(&[
                base_idx, base_idx + 1, base_idx + 2,
                base_idx, base_idx + 2, base_idx + 3,
            ]);
        }

        let mut mesh = Self::new(name.to_string(), vertices, indices);
        mesh.normalize_to_unit_box();
        mesh.ensure_tangents();
        mesh.rebuild_bvh();
        mesh
    }

    pub fn create_cylinder_mesh(name: &str, radius: f32, height: f32, segments: u32) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let half_h = height * 0.5;

        // Side cylinder surface
        for i in 0..=segments {
            let u = i as f32 / segments as f32;
            let theta = u * std::f32::consts::PI * 2.0;
            let cos_t = theta.cos();
            let sin_t = theta.sin();

            let norm = Vec3::new(cos_t, 0.0, sin_t);
            let pos_top = Vec3::new(radius * cos_t, half_h, radius * sin_t);
            let pos_bot = Vec3::new(radius * cos_t, -half_h, radius * sin_t);

            // UV mapped to bottom 60% of atlas
            let v_top = 0.40;
            let v_bot = 1.00;
            let tan = Vec4::new(-theta.sin(), 0.0, theta.cos(), 1.0);

            vertices.push(Vertex {
                position: pos_top,
                normal: norm,
                tangent: tan,
                uv: Vec2::new(u, v_top),
            });
            vertices.push(Vertex {
                position: pos_bot,
                normal: norm,
                tangent: tan,
                uv: Vec2::new(u, v_bot),
            });
        }

        for i in 0..segments {
            let base = (i * 2) as u32;
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 1);

            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base + 3);
        }

        // Top Disc Cap
        let top_center_idx = vertices.len() as u32;
        vertices.push(Vertex {
            position: Vec3::new(0.0, half_h, 0.0),
            normal: Vec3::Y,
            tangent: Vec4::new(1.0, 0.0, 0.0, 1.0),
            uv: Vec2::new(0.25, 0.20),
        });

        for i in 0..=segments {
            let theta = (i as f32 / segments as f32) * std::f32::consts::PI * 2.0;
            let x = theta.cos();
            let z = theta.sin();
            vertices.push(Vertex {
                position: Vec3::new(radius * x, half_h, radius * z),
                normal: Vec3::Y,
                tangent: Vec4::new(1.0, 0.0, 0.0, 1.0),
                uv: Vec2::new(0.25 + x * 0.20, 0.20 + z * 0.20),
            });
        }

        for i in 0..segments {
            indices.push(top_center_idx);
            indices.push(top_center_idx + 2 + i);
            indices.push(top_center_idx + 1 + i);
        }

        // Bottom Disc Cap
        let bot_center_idx = vertices.len() as u32;
        vertices.push(Vertex {
            position: Vec3::new(0.0, -half_h, 0.0),
            normal: -Vec3::Y,
            tangent: Vec4::new(1.0, 0.0, 0.0, 1.0),
            uv: Vec2::new(0.75, 0.20),
        });

        for i in 0..=segments {
            let theta = (i as f32 / segments as f32) * std::f32::consts::PI * 2.0;
            let x = theta.cos();
            let z = theta.sin();
            vertices.push(Vertex {
                position: Vec3::new(radius * x, -half_h, radius * z),
                normal: -Vec3::Y,
                tangent: Vec4::new(1.0, 0.0, 0.0, 1.0),
                uv: Vec2::new(0.75 + x * 0.20, 0.20 + z * 0.20),
            });
        }

        for i in 0..segments {
            indices.push(bot_center_idx);
            indices.push(bot_center_idx + 1 + i);
            indices.push(bot_center_idx + 2 + i);
        }

        let mut mesh = Self::new(name.to_string(), vertices, indices);
        mesh.normalize_to_unit_box();
        mesh.ensure_tangents();
        mesh.rebuild_bvh();
        mesh
    }

    pub fn create_torus_mesh(name: &str, r_maj: f32, r_min: f32, seg_maj: u32, seg_min: u32) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for i in 0..=seg_maj {
            let u = i as f32 / seg_maj as f32;
            let theta = u * std::f32::consts::PI * 2.0;
            let cos_t = theta.cos();
            let sin_t = theta.sin();
            let tan = Vec4::new(-sin_t, 0.0, cos_t, 1.0);

            for j in 0..=seg_min {
                let v = j as f32 / seg_min as f32;
                let phi = v * std::f32::consts::PI * 2.0;
                let cos_p = phi.cos();
                let sin_p = phi.sin();

                let x = (r_maj + r_min * cos_p) * cos_t;
                let y = r_min * sin_p;
                let z = (r_maj + r_min * cos_p) * sin_t;

                let norm = Vec3::new(cos_p * cos_t, sin_p, cos_p * sin_t).normalize();

                vertices.push(Vertex {
                    position: Vec3::new(x, y, z),
                    normal: norm,
                    tangent: tan,
                    uv: Vec2::new(u, v),
                });
            }
        }

        for i in 0..seg_maj {
            for j in 0..seg_min {
                let first = i * (seg_min + 1) + j;
                let second = first + (seg_min + 1);

                indices.push(first);
                indices.push(first + 1);
                indices.push(second);

                indices.push(second);
                indices.push(first + 1);
                indices.push(second + 1);
            }
        }

        let mut mesh = Self::new(name.to_string(), vertices, indices);
        mesh.normalize_to_unit_box();
        mesh.ensure_tangents();
        mesh.rebuild_bvh();
        mesh
    }

    pub fn create_rounded_cube() -> Self { 
        Self::create_rounded_cube_mesh("Rounded Cube", 1.8, 0.25, 10) 
    }

    pub fn create_rounded_cube_mesh(name: &str, size: f32, radius: f32, segments: usize) -> Self {
        let half = size * 0.5;
        let r = radius.clamp(0.01, half * 0.5);
        let inner = half - r;
        let segs = segments.max(2);

        let faces = [
            // Front (+Z)
            (Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 1, 1),
            // Back (-Z)
            (Vec3::new(0.0, 0.0, -1.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 3, 1),
            // Right (+X)
            (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0), 2, 1),
            // Left (-X)
            (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 1.0, 0.0), 0, 1),
            // Top (+Y)
            (Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), 1, 0),
            // Bottom (-Y)
            (Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 1, 2),
        ];

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let uv_cw = 1.0 / 4.0;
        let uv_ch = 1.0 / 3.0;

        for (face_norm, face_tan, face_bitan, col, row) in faces {
            let base_idx = vertices.len() as u32;

            for j in 0..=segs {
                let v_frac = j as f32 / segs as f32;
                let v_coord = (v_frac * 2.0 - 1.0) * half;

                for i in 0..=segs {
                    let u_frac = i as f32 / segs as f32;
                    let u_coord = (u_frac * 2.0 - 1.0) * half;

                    let p0 = face_norm * half + face_tan * u_coord + face_bitan * v_coord;
                    let clamped = Vec3::new(
                        p0.x.clamp(-inner, inner),
                        p0.y.clamp(-inner, inner),
                        p0.z.clamp(-inner, inner),
                    );
                    let diff = p0 - clamped;
                    let (norm, pos) = if diff.length_squared() > 1e-6 {
                        let n = diff.normalize();
                        (n, clamped + n * r)
                    } else {
                        (face_norm, p0)
                    };

                    let tan_proj = face_tan - norm * norm.dot(face_tan);
                    let tangent = if tan_proj.length_squared() > 1e-6 {
                        Vec4::new(tan_proj.normalize().x, tan_proj.normalize().y, tan_proj.normalize().z, 1.0)
                    } else {
                        Vec4::new(face_tan.x, face_tan.y, face_tan.z, 1.0)
                    };

                    let uv = Vec2::new(
                        (col as f32 + u_frac) * uv_cw,
                        (row as f32 + (1.0 - v_frac)) * uv_ch,
                    );

                    vertices.push(Vertex {
                        position: pos,
                        normal: norm,
                        tangent,
                        uv,
                    });
                }
            }

            let stride = (segs + 1) as u32;
            for j in 0..segs as u32 {
                for i in 0..segs as u32 {
                    let v00 = base_idx + j * stride + i;
                    let v10 = base_idx + j * stride + (i + 1);
                    let v01 = base_idx + (j + 1) * stride + i;
                    let v11 = base_idx + (j + 1) * stride + (i + 1);

                    // CCW winding: T x B = N
                    indices.push(v00);
                    indices.push(v10);
                    indices.push(v11);

                    indices.push(v00);
                    indices.push(v11);
                    indices.push(v01);
                }
            }
        }

        Self::new(name.to_string(), vertices, indices)
    }

    pub fn create_scifi_crate() -> Self { 
        Self::create_scifi_crate_mesh("Sci-Fi Crate", 1.8) 
    }

    pub fn create_scifi_crate_mesh(name: &str, size: f32) -> Self {
        let half = size * 0.5;
        let border = half * 0.22;
        let inset_depth = 0.08;

        let faces = [
            (Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 1, 1),
            (Vec3::new(0.0, 0.0, -1.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 3, 1),
            (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0), 2, 1),
            (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 1.0, 0.0), 0, 1),
            (Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), 1, 0),
            (Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 1, 2),
        ];

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let uv_cw = 1.0 / 4.0;
        let uv_ch = 1.0 / 3.0;

        for (face_norm, face_tan, face_bitan, col, row) in faces {
            let add_quad = |verts: &mut Vec<Vertex>, inds: &mut Vec<u32>, p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, norm: Vec3, u0: f32, v0: f32, u1: f32, v1: f32| {
                let base = verts.len() as u32;
                let tan_vec = face_tan - norm * norm.dot(face_tan);
                let tan = if tan_vec.length_squared() > 1e-6 {
                    let tn = tan_vec.normalize();
                    Vec4::new(tn.x, tn.y, tn.z, 1.0)
                } else {
                    Vec4::new(face_tan.x, face_tan.y, face_tan.z, 1.0)
                };

                verts.push(Vertex { position: p0, normal: norm, tangent: tan, uv: Vec2::new((col as f32 + u0) * uv_cw, (row as f32 + v0) * uv_ch) });
                verts.push(Vertex { position: p1, normal: norm, tangent: tan, uv: Vec2::new((col as f32 + u1) * uv_cw, (row as f32 + v0) * uv_ch) });
                verts.push(Vertex { position: p2, normal: norm, tangent: tan, uv: Vec2::new((col as f32 + u1) * uv_cw, (row as f32 + v1) * uv_ch) });
                verts.push(Vertex { position: p3, normal: norm, tangent: tan, uv: Vec2::new((col as f32 + u0) * uv_cw, (row as f32 + v1) * uv_ch) });

                inds.push(base);
                inds.push(base + 1);
                inds.push(base + 2);

                inds.push(base);
                inds.push(base + 2);
                inds.push(base + 3);
            };

            let pt = |u: f32, v: f32, d: f32| -> Vec3 {
                face_norm * (half - d) + face_tan * u + face_bitan * v
            };

            let in_h = half - border;
            // 4 Outer rim border bands
            add_quad(&mut vertices, &mut indices, pt(-half, -half, 0.0), pt(half, -half, 0.0), pt(half, -in_h, 0.0), pt(-half, -in_h, 0.0), face_norm, 0.0, 0.0, 1.0, 0.22);
            add_quad(&mut vertices, &mut indices, pt(-half, in_h, 0.0), pt(half, in_h, 0.0), pt(half, half, 0.0), pt(-half, half, 0.0), face_norm, 0.0, 0.78, 1.0, 1.0);
            add_quad(&mut vertices, &mut indices, pt(-half, -in_h, 0.0), pt(-in_h, -in_h, 0.0), pt(-in_h, in_h, 0.0), pt(-half, in_h, 0.0), face_norm, 0.0, 0.22, 0.22, 0.78);
            add_quad(&mut vertices, &mut indices, pt(in_h, -in_h, 0.0), pt(half, -in_h, 0.0), pt(half, in_h, 0.0), pt(in_h, in_h, 0.0), face_norm, 0.78, 0.22, 1.0, 0.78);

            // 4 Beveled sloped walls leading to recessed panel
            let slope_s = border * 0.4;
            let panel_h = in_h - slope_s;

            // Bottom slope
            let n_bot = (face_norm + face_bitan).normalize();
            add_quad(&mut vertices, &mut indices, pt(-in_h, -in_h, 0.0), pt(in_h, -in_h, 0.0), pt(panel_h, -panel_h, inset_depth), pt(-panel_h, -panel_h, inset_depth), n_bot, 0.22, 0.22, 0.78, 0.3);
            // Top slope
            let n_top = (face_norm - face_bitan).normalize();
            add_quad(&mut vertices, &mut indices, pt(-panel_h, panel_h, inset_depth), pt(panel_h, panel_h, inset_depth), pt(in_h, in_h, 0.0), pt(-in_h, in_h, 0.0), n_top, 0.22, 0.7, 0.78, 0.78);
            // Left slope
            let n_left = (face_norm + face_tan).normalize();
            add_quad(&mut vertices, &mut indices, pt(-in_h, -in_h, 0.0), pt(-panel_h, -panel_h, inset_depth), pt(-panel_h, panel_h, inset_depth), pt(-in_h, in_h, 0.0), n_left, 0.22, 0.22, 0.3, 0.78);
            // Right slope
            let n_right = (face_norm - face_tan).normalize();
            add_quad(&mut vertices, &mut indices, pt(panel_h, -panel_h, inset_depth), pt(in_h, -in_h, 0.0), pt(in_h, in_h, 0.0), pt(panel_h, panel_h, inset_depth), n_right, 0.7, 0.22, 0.78, 0.78);

            // Recessed center panel
            add_quad(&mut vertices, &mut indices, pt(-panel_h, -panel_h, inset_depth), pt(panel_h, -panel_h, inset_depth), pt(panel_h, panel_h, inset_depth), pt(-panel_h, panel_h, inset_depth), face_norm, 0.3, 0.3, 0.7, 0.7);
        }

        Self::new(name.to_string(), vertices, indices)
    }

    pub fn create_robot_head() -> Self { 
        Self::create_robot_head_mesh("Robot Head", 1.8) 
    }

    pub fn create_robot_head_mesh(name: &str, size: f32) -> Self {
        // Build base cranium via rounded box, then attach robot visor screen, ear bolts, and antenna
        let mut base = Self::create_rounded_cube_mesh(name, size * 0.88, size * 0.16, 8);
        let ear_radius = size * 0.18;
        let ear_depth = size * 0.12;
        let ear_segs = 16;
        let half_x = size * 0.88 * 0.5;

        // Ear bolts on +X and -X
        for &side in &[1.0f32, -1.0f32] {
            let base_idx = base.vertices.len() as u32;
            let center_x = side * (half_x + ear_depth);
            let root_x = side * half_x;
            let normal_x = Vec3::new(side, 0.0, 0.0);
            let cap_tan = Vec4::new(0.0, 1.0, 0.0, 1.0);
            let mantle_tan = Vec4::new(side, 0.0, 0.0, 1.0);

            // Center cap vertex
            base.vertices.push(Vertex {
                position: Vec3::new(center_x, 0.0, 0.0),
                normal: normal_x,
                tangent: cap_tan,
                uv: Vec2::new(0.5, 0.5),
            });

            for i in 0..=ear_segs {
                let angle = (i as f32 / ear_segs as f32) * std::f32::consts::TAU;
                let (sin_a, cos_a) = angle.sin_cos();
                let y = sin_a * ear_radius;
                let z = cos_a * ear_radius;

                // Rim cap vertex
                base.vertices.push(Vertex {
                    position: Vec3::new(center_x, y, z),
                    normal: normal_x,
                    tangent: cap_tan,
                    uv: Vec2::new(0.5 + 0.3 * cos_a, 0.5 + 0.3 * sin_a),
                });

                // Cylinder mantle root vertex
                let side_norm = Vec3::new(0.0, sin_a, cos_a);
                base.vertices.push(Vertex {
                    position: Vec3::new(root_x, y, z),
                    normal: side_norm,
                    tangent: mantle_tan,
                    uv: Vec2::new(i as f32 / ear_segs as f32, 0.0),
                });
            }

            for i in 0..ear_segs {
                let v_center = base_idx;
                let v_rim_curr = base_idx + 1 + i * 2;
                let v_rim_next = base_idx + 1 + (i + 1) * 2;
                let v_root_curr = base_idx + 2 + i * 2;
                let v_root_next = base_idx + 2 + (i + 1) * 2;

                if side > 0.0 {
                    // +X Cap
                    base.indices.push(v_center);
                    base.indices.push(v_rim_next);
                    base.indices.push(v_rim_curr);

                    // Cylinder mantle
                    base.indices.push(v_root_curr);
                    base.indices.push(v_rim_curr);
                    base.indices.push(v_root_next);

                    base.indices.push(v_root_next);
                    base.indices.push(v_rim_curr);
                    base.indices.push(v_rim_next);
                } else {
                    // -X Cap
                    base.indices.push(v_center);
                    base.indices.push(v_rim_curr);
                    base.indices.push(v_rim_next);

                    // Cylinder mantle
                    base.indices.push(v_root_curr);
                    base.indices.push(v_root_next);
                    base.indices.push(v_rim_curr);

                    base.indices.push(v_root_next);
                    base.indices.push(v_rim_next);
                    base.indices.push(v_rim_curr);
                }
            }
        }

        // Antenna on +Y head
        let ant_base_idx = base.vertices.len() as u32;
        let ant_h = size * 0.28;
        let ant_r = size * 0.045;
        let ant_y_start = size * 0.88 * 0.5;
        let ant_segs = 12;

        for i in 0..=ant_segs {
            let angle = (i as f32 / ant_segs as f32) * std::f32::consts::TAU;
            let (sin_a, cos_a) = angle.sin_cos();
            let x = cos_a * ant_r;
            let z = sin_a * ant_r;
            let n = Vec3::new(cos_a, 0.0, sin_a);
            let tan = Vec4::new(-sin_a, 0.0, cos_a, 1.0);

            base.vertices.push(Vertex {
                position: Vec3::new(x, ant_y_start, z),
                normal: n,
                tangent: tan,
                uv: Vec2::new(i as f32 / ant_segs as f32, 0.0),
            });
            base.vertices.push(Vertex {
                position: Vec3::new(x, ant_y_start + ant_h, z),
                normal: n,
                tangent: tan,
                uv: Vec2::new(i as f32 / ant_segs as f32, 1.0),
            });
        }

        for i in 0..ant_segs {
            let b0 = ant_base_idx + i * 2;
            let t0 = ant_base_idx + i * 2 + 1;
            let b1 = ant_base_idx + (i + 1) * 2;
            let t1 = ant_base_idx + (i + 1) * 2 + 1;

            base.indices.push(b0);
            base.indices.push(t0);
            base.indices.push(b1);

            base.indices.push(b1);
            base.indices.push(t0);
            base.indices.push(t1);
        }

        // Antenna tip sphere/beacon
        let tip_center = Vec3::new(0.0, ant_y_start + ant_h + ant_r * 1.5, 0.0);
        let tip_r = ant_r * 2.2;
        let tip_base_idx = base.vertices.len() as u32;
        let tip_rings = 6;
        let tip_sectors = 10;

        for r in 0..=tip_rings {
            let v_frac = r as f32 / tip_rings as f32;
            let phi = v_frac * std::f32::consts::PI;
            let (sin_phi, cos_phi) = phi.sin_cos();

            for s in 0..=tip_sectors {
                let u_frac = s as f32 / tip_sectors as f32;
                let theta = u_frac * std::f32::consts::TAU;
                let (sin_theta, cos_theta) = theta.sin_cos();

                let n = Vec3::new(sin_phi * cos_theta, cos_phi, sin_phi * sin_theta);
                let pos = tip_center + n * tip_r;
                let tan = Vec4::new(-sin_theta, 0.0, cos_theta, 1.0);

                base.vertices.push(Vertex {
                    position: pos,
                    normal: n,
                    tangent: tan,
                    uv: Vec2::new(u_frac, v_frac),
                });
            }
        }

        let tip_stride = (tip_sectors + 1) as u32;
        for r in 0..tip_rings as u32 {
            for s in 0..tip_sectors as u32 {
                let first = tip_base_idx + r * tip_stride + s;
                let second = tip_base_idx + (r + 1) * tip_stride + s;

                base.indices.push(first);
                base.indices.push(first + 1);
                base.indices.push(second);

                base.indices.push(second);
                base.indices.push(first + 1);
                base.indices.push(second + 1);
            }
        }

        Self::new(name.to_string(), base.vertices, base.indices)
    }

    pub fn create_cylinder() -> Self { 
        Self::create_cylinder_mesh("Cylinder", 0.9, 1.8, 36) 
    }

    pub fn create_torus() -> Self { 
        Self::create_torus_mesh("Torus", 0.8, 0.35, 36, 24) 
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
    pub uv: [f32; 2],
}

impl From<&Vertex> for GpuVertex {
    fn from(v: &Vertex) -> Self {
        Self {
            position: v.position.into(),
            normal: v.normal.into(),
            tangent: v.tangent.into(),
            uv: v.uv.into(),
        }
    }
}

impl Mesh {
    #[allow(dead_code)]
    pub fn from_obj_file(path: &std::path::Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        Self::from_obj_str(&content, &name)
    }

    /// Save 3D model to Wavefront OBJ format matching exported textures
    pub fn save_to_obj_file(&self, path: &std::path::Path) -> Result<(), String> {
        use std::io::Write;
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut writer = std::io::BufWriter::new(file);

        let clean_name = self.name.replace(' ', "_").to_lowercase();
        writeln!(writer, "# 3D Paintcraft OBJ Model Export").map_err(|e| e.to_string())?;
        writeln!(writer, "mtllib {}.mtl", clean_name).map_err(|e| e.to_string())?;
        writeln!(writer, "o {}", self.name).map_err(|e| e.to_string())?;

        let inv_scale = if self.original_scale.abs() > 1e-6 { 1.0 / self.original_scale } else { 1.0 };
        for v in &self.vertices {
            let p = v.position * inv_scale + self.original_center;
            writeln!(writer, "v {:.6} {:.6} {:.6}", p.x, p.y, p.z).map_err(|e| e.to_string())?;
        }

        for v in &self.vertices {
            writeln!(writer, "vt {:.6} {:.6}", v.uv.x, v.uv.y).map_err(|e| e.to_string())?;
        }

        for v in &self.vertices {
            writeln!(writer, "vn {:.6} {:.6} {:.6}", v.normal.x, v.normal.y, v.normal.z).map_err(|e| e.to_string())?;
        }

        writeln!(writer, "usemtl Material").map_err(|e| e.to_string())?;
        writeln!(writer, "s 1").map_err(|e| e.to_string())?;

        for chunk in self.indices.chunks_exact(3) {
            let i0 = chunk[0] + 1;
            let i1 = chunk[1] + 1;
            let i2 = chunk[2] + 1;
            writeln!(writer, "f {0}/{0}/{0} {1}/{1}/{1} {2}/{2}/{2}", i0, i1, i2).map_err(|e| e.to_string())?;
        }

        writer.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_shapes_tangents() {
        let shapes = vec![
            ("Sphere / Shader Ball", Mesh::create_shader_ball()),
            ("Cube", Mesh::create_cube_mesh("Cube", 1.8)),
            ("Rounded Cube", Mesh::create_rounded_cube()),
            ("Sci-Fi Crate", Mesh::create_scifi_crate()),
            ("Robot Head", Mesh::create_robot_head()),
            ("Cylinder", Mesh::create_cylinder_mesh("Cylinder", 0.9, 1.8, 36)),
            ("Torus", Mesh::create_torus_mesh("Torus", 0.8, 0.35, 36, 24)),
        ];

        for (name, mesh) in shapes {
            println!("Testing shape: {}", name);
            let tri_count = mesh.indices.len() / 3;
            let mut inverted_tri_count = 0;
            for t in 0..tri_count {
                let i0 = mesh.indices[t * 3] as usize;
                let i1 = mesh.indices[t * 3 + 1] as usize;
                let i2 = mesh.indices[t * 3 + 2] as usize;
                let v0 = &mesh.vertices[i0];
                let v1 = &mesh.vertices[i1];
                let v2 = &mesh.vertices[i2];
                let fnorm = (v1.position - v0.position).cross(v2.position - v0.position);
                let avg_norm = v0.normal + v1.normal + v2.normal;
                if fnorm.dot(avg_norm) < -1e-5 {
                    inverted_tri_count += 1;
                }
            }
            println!("  Inverted triangles: {} / {}", inverted_tri_count, tri_count);

            for (idx, v) in mesh.vertices.iter().enumerate() {
                assert!(!v.tangent.x.is_nan() && !v.tangent.y.is_nan() && !v.tangent.z.is_nan(),
                    "{}: Vertex {} has NaN tangent", name, idx);
                let tan_len = v.tangent.truncate().length();
                assert!((tan_len - 1.0).abs() < 1e-3,
                    "{}: Vertex {} tangent length is {}, expected ~1.0", name, idx, tan_len);
                assert!((v.tangent.w.abs() - 1.0).abs() < 1e-3,
                    "{}: Vertex {} tangent.w is {}, expected +/-1.0", name, idx, v.tangent.w);
                let dot_tn = v.tangent.truncate().dot(v.normal).abs();
                assert!(dot_tn < 1e-3,
                    "{}: Vertex {} tangent not orthogonal to normal, dot is {}", name, idx, dot_tn);
            }
        }
    }

    #[test]
    fn test_decode_base64_simple() {
        let text = "SGVsbG8gV29ybGQ="; // "Hello World"
        let decoded = Mesh::decode_base64_simple(text);
        assert_eq!(std::str::from_utf8(&decoded).unwrap(), "Hello World");
    }
}
