use std::collections::{HashMap, HashSet};
use crate::channel::{RgbaBuffer, ScalarBuffer};
use crate::mesh::{Mesh, Ray};
use glam::{Vec2, Vec3, Vec4};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BakeMatchMode {
    Always,
    ByMeshName,
}

impl BakeMatchMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Always => "Always",
            Self::ByMeshName => "By Mesh Name",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakeMatchConfig {
    pub mode: BakeMatchMode,
    pub low_suffix: String,
    pub high_suffix: String,
}

impl Default for BakeMatchConfig {
    fn default() -> Self {
        Self {
            mode: BakeMatchMode::Always,
            low_suffix: "_low".to_string(),
            high_suffix: "_high".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BakeCageMode {
    None,
    ParametricInflation,
    ExternalCage,
}

impl BakeCageMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "None (Ray Distances)",
            Self::ParametricInflation => "Parametric Cage (Inflation)",
            Self::ExternalCage => "External Cage Mesh",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BakeCageConfig {
    pub mode: BakeCageMode,
    pub inflation_offset: f32,
    pub external_cage_path: Option<std::path::PathBuf>,
}

impl Default for BakeCageConfig {
    fn default() -> Self {
        Self {
            mode: BakeCageMode::None,
            inflation_offset: 0.02,
            external_cage_path: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BakeSettingsTab {
    Common,
    TangentNormal,
    WorldNormal,
    AmbientOcclusion,
    Curvature,
    Position,
    Thickness,
    IdMap,
}

impl BakeSettingsTab {
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Common => "Common Parameters",
            Self::TangentNormal => "Normal (Tangent Space)",
            Self::WorldNormal => "World Space Normal",
            Self::AmbientOcclusion => "Ambient Occlusion",
            Self::Curvature => "Curvature",
            Self::Position => "Position (World Space)",
            Self::Thickness => "Thickness",
            Self::IdMap => "ID Map",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AoBakeConfig {
    pub samples: usize,
    pub max_distance: f32,
    pub ray_bias: f32,
    pub spread: f32,
    pub contrast: f32,
}

impl Default for AoBakeConfig {
    fn default() -> Self {
        Self {
            samples: 64,
            max_distance: 0.35,
            ray_bias: 0.008,
            spread: 1.0,
            contrast: 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThicknessBakeConfig {
    pub samples: usize,
    pub max_distance: f32,
    pub ray_bias: f32,
    pub invert: bool,
}

impl Default for ThicknessBakeConfig {
    fn default() -> Self {
        Self {
            samples: 32,
            max_distance: 0.60,
            ray_bias: 0.005,
            invert: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurvatureBakeConfig {
    pub intensity: f32,
    pub contrast: f32,
    pub dihedral_weight: f32,
    pub deflection_weight: f32,
    pub blur_passes: usize,
}

impl Default for CurvatureBakeConfig {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            contrast: 1.0,
            dihedral_weight: 0.40,
            deflection_weight: 0.25,
            blur_passes: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NormalBakeConfig {
    pub frontal_distance: f32,
    pub rear_distance: f32,
    pub invert_green: bool,
}

impl Default for NormalBakeConfig {
    fn default() -> Self {
        Self {
            frontal_distance: 0.1,
            rear_distance: 0.1,
            invert_green: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionNormalizationMode {
    BoundingBox,
    WorldUnits,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionBakeConfig {
    pub mode: PositionNormalizationMode,
}

impl Default for PositionBakeConfig {
    fn default() -> Self {
        Self {
            mode: PositionNormalizationMode::BoundingBox,
        }
    }
}


pub fn strip_mesh_suffix(name: &str, suffix: &str) -> String {
    let lower_name = name.to_lowercase();
    let lower_suffix = suffix.to_lowercase();
    if !lower_suffix.is_empty() && lower_name.ends_with(&lower_suffix) {
        name[..name.len() - suffix.len()].trim_end_matches('_').to_string()
    } else if lower_name.ends_with("_low") {
        name[..name.len() - 4].trim_end_matches('_').to_string()
    } else if lower_name.ends_with("_high") {
        name[..name.len() - 5].trim_end_matches('_').to_string()
    } else {
        name.to_string()
    }
}

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

    pub fn query_ray_closest_filtered(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_dist: f32,
        mesh: &Mesh,
        allowed_triangles: Option<&HashSet<usize>>,
    ) -> Option<(f32, Vec3)> {
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
                            if let Some(allowed) = allowed_triangles {
                                if !allowed.contains(&t) {
                                    continue;
                                }
                            }
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

    #[allow(dead_code)]
    pub fn query_ray_closest(&self, origin: Vec3, dir: Vec3, max_dist: f32, mesh: &Mesh) -> Option<(f32, Vec3)> {
        self.query_ray_closest_filtered(origin, dir, max_dist, mesh, None)
    }
}


pub struct Baker;

impl Baker {
    /// Compute cage vertices according to BakeCageConfig:
    /// - None: returns Ok(None)
    /// - ParametricInflation: welds split-normal seam vertices and inflates along continuous smoothed normals
    /// - ExternalCage: loads external OBJ file from external_cage_path and verifies identical topology
    pub fn compute_cage_vertices(mesh: &Mesh, cage_config: &BakeCageConfig) -> Result<Option<Vec<Vec3>>, String> {
        match cage_config.mode {
            BakeCageMode::None => Ok(None),
            BakeCageMode::ParametricInflation => {
                let offset = cage_config.inflation_offset;
                if offset.abs() < 1e-7 {
                    return Ok(None);
                }

                // 1. Group vertices by spatial position to compute continuous smoothed outward normals
                let mut pos_normal_map: HashMap<[i64; 3], Vec3> = HashMap::new();
                let quant_scale = 10_000.0f32;

                for v in &mesh.vertices {
                    let k = [
                        (v.position.x * quant_scale).round() as i64,
                        (v.position.y * quant_scale).round() as i64,
                        (v.position.z * quant_scale).round() as i64,
                    ];
                    let n = if v.normal.length_squared() > 1e-4 {
                        v.normal.normalize()
                    } else {
                        Vec3::Y
                    };
                    let entry = pos_normal_map.entry(k).or_insert(Vec3::ZERO);
                    *entry += n;
                }

                // 2. Displace each vertex along the averaged smooth normal
                let cage_positions = mesh.vertices.iter().map(|v| {
                    let k = [
                        (v.position.x * quant_scale).round() as i64,
                        (v.position.y * quant_scale).round() as i64,
                        (v.position.z * quant_scale).round() as i64,
                    ];
                    let smooth_n = pos_normal_map.get(&k)
                        .copied()
                        .unwrap_or(v.normal)
                        .normalize_or_zero();
                    let final_n = if smooth_n.length_squared() > 1e-4 {
                        smooth_n
                    } else if v.normal.length_squared() > 1e-4 {
                        v.normal.normalize()
                    } else {
                        Vec3::Y
                    };
                    v.position + final_n * offset
                }).collect();

                Ok(Some(cage_positions))
            }
            BakeCageMode::ExternalCage => {
                let path = cage_config.external_cage_path.as_ref().ok_or_else(|| {
                    "External cage mode selected but no cage file path was specified.".to_string()
                })?;

                let cage_mesh = Mesh::from_obj_file(path).map_err(|e| {
                    format!("Failed to load external cage OBJ '{:?}': {}", path, e)
                })?;

                if cage_mesh.vertices.len() != mesh.vertices.len() {
                    return Err(format!(
                        "External cage vertex count ({}) does not match low-poly mesh vertex count ({}). External cage OBJ must share identical topology.",
                        cage_mesh.vertices.len(),
                        mesh.vertices.len()
                    ));
                }

                let cage_positions = cage_mesh.vertices.into_iter().map(|v| v.position).collect();
                Ok(Some(cage_positions))
            }
        }
    }

    /// Bake Curvature map (Convex edges = white, Crevices = black, Flat = 0.5 gray)
    pub fn bake_curvature(mesh: &Mesh, width: u32, height: u32) -> ScalarBuffer {
        Self::bake_curvature_with_options(mesh, width, height, &CurvatureBakeConfig::default())
    }

    /// Bake Curvature map with custom algorithm weights, intensity, contrast and blur passes
    pub fn bake_curvature_with_options(
        mesh: &Mesh,
        width: u32,
        height: u32,
        config: &CurvatureBakeConfig,
    ) -> ScalarBuffer {
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

            let curv_signal = (avg_dih * config.dihedral_weight + avg_deflect * config.deflection_weight) * config.intensity;
            let val = if (config.contrast - 1.0).abs() > 0.01 {
                (0.5 + curv_signal * config.contrast).clamp(0.0, 1.0)
            } else {
                (0.5 + curv_signal).clamp(0.0, 1.0)
            };
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

        // 9. Configurable Separable Gaussian Blur for smooth edge gradients
        let mut current_buf = buffer.clone();
        if config.blur_passes > 0 {
            let w = width as i32;
            let h = height as i32;
            let weights = [0.06136f32, 0.24477, 0.38774, 0.24477, 0.06136];

            for _ in 0..config.blur_passes {
                let mut temp_h = current_buf.clone();
                // Horizontal pass
                for y in 0..h {
                    for x in 0..w {
                        let mut sum = 0.0f32;
                        for (k, &wt) in weights.iter().enumerate() {
                            let sample_x = (x + (k as i32 - 2)).clamp(0, w - 1) as u32;
                            sum += current_buf.get_pixel(sample_x, y as u32) * wt;
                        }
                        temp_h.set_pixel(x as u32, y as u32, sum);
                    }
                }

                // Vertical pass
                for y in 0..h {
                    for x in 0..w {
                        let mut sum = 0.0f32;
                        for (k, &wt) in weights.iter().enumerate() {
                            let sample_y = (y + (k as i32 - 2)).clamp(0, h - 1) as u32;
                            sum += temp_h.get_pixel(x as u32, sample_y) * wt;
                        }
                        current_buf.set_pixel(x as u32, y as u32, sum);
                    }
                }
            }
        }

        // 10. Dilate again so bilinear filtering outside UV borders is completely smooth
        Self::dilate_scalar(&mut current_buf, &mut written_mask, 16);

        current_buf
    }

    /// Bake Ambient Occlusion map via Monte Carlo Hemisphere Occlusion with smoothed normal consideration
    pub fn bake_ao(mesh: &Mesh, width: u32, height: u32) -> ScalarBuffer {
        Self::bake_ao_with_options(mesh, width, height, &BakeCageConfig::default(), &AoBakeConfig::default())
            .unwrap_or_else(|_| ScalarBuffer::new(width, height, 1.0))
    }

    /// Bake Ambient Occlusion map with optional cage envelope support
    #[allow(dead_code)]
    pub fn bake_ao_with_cage(
        mesh: &Mesh,
        width: u32,
        height: u32,
        cage_config: &BakeCageConfig,
    ) -> Result<ScalarBuffer, String> {
        Self::bake_ao_with_options(mesh, width, height, cage_config, &AoBakeConfig::default())
    }

    /// Bake Ambient Occlusion map with customizable samples, distances, bias, spread and contrast
    pub fn bake_ao_with_options(
        mesh: &Mesh,
        width: u32,
        height: u32,
        cage_config: &BakeCageConfig,
        ao_config: &AoBakeConfig,
    ) -> Result<ScalarBuffer, String> {
        let mut buffer = ScalarBuffer::new(width, height, 1.0);
        let tri_count = mesh.indices.len() / 3;
        if tri_count == 0 || mesh.vertices.is_empty() {
            return Ok(buffer);
        }

        let cage_verts = Self::compute_cage_vertices(mesh, cage_config)?;

        let sample_count = ao_config.samples.clamp(4, 1024);
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
        let max_dist = (diag * ao_config.max_distance).max(0.01);
        let ray_offset = (diag * ao_config.ray_bias).max(0.001);
        let min_self_dist = (ray_offset * 3.75).max(0.004);

        // Rayon parallel computation of vertex AO with smoothed normals and cage envelope
        let vert_ao: Vec<f32> = mesh.vertices.par_iter().enumerate().map(|(vi, v)| {
            let normal = if v.normal.length_squared() > 1e-4 {
                v.normal.normalize()
            } else {
                Vec3::Y
            };

            // Build orthonormal basis (tangent, bitangent, normal)
            let up = if normal.y.abs() < 0.95 { Vec3::Y } else { Vec3::X };
            let tangent = normal.cross(up).normalize();
            let bitangent = normal.cross(tangent);

            let origin = if let Some(ref cv) = cage_verts {
                cv.get(vi).copied().unwrap_or(v.position + normal * ray_offset)
            } else {
                v.position + normal * ray_offset
            };
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
                        let raw_occ = (1.0 - hit.t / max_dist).clamp(0.0, 1.0);
                        let shaped_occ = if (ao_config.spread - 1.0).abs() > 0.01 {
                            raw_occ.powf(ao_config.spread.max(0.05))
                        } else {
                            raw_occ
                        };
                        let occ = (shaped_occ * 0.85 * ao_config.contrast).clamp(0.0, 1.0);
                        unoccluded += 1.0 - occ;
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
        Ok(buffer)
    }

    /// Bake Tangent Space Normal map
    pub fn bake_tangent_normals(mesh: &Mesh, high_poly: Option<&Mesh>, frontal_dist: f32, rear_dist: f32, width: u32, height: u32) -> RgbaBuffer {
        Self::bake_tangent_normals_with_cage(mesh, high_poly, frontal_dist, rear_dist, width, height, &BakeMatchConfig::default(), &BakeCageConfig::default())
            .unwrap_or_else(|_| RgbaBuffer::new(width, height, Vec4::new(0.5, 0.5, 1.0, 1.0)))
    }

    /// Bake Tangent Space Normal map with customizable matching mode (_low / _high)
    pub fn bake_tangent_normals_with_match(
        mesh: &Mesh,
        high_poly: Option<&Mesh>,
        frontal_dist: f32,
        rear_dist: f32,
        width: u32,
        height: u32,
        match_config: &BakeMatchConfig,
    ) -> RgbaBuffer {
        Self::bake_tangent_normals_with_cage(mesh, high_poly, frontal_dist, rear_dist, width, height, match_config, &BakeCageConfig::default())
            .unwrap_or_else(|_| RgbaBuffer::new(width, height, Vec4::new(0.5, 0.5, 1.0, 1.0)))
    }

    /// Bake Tangent Space Normal map with cage envelope projection and customizable matching mode
    pub fn bake_tangent_normals_with_cage(
        mesh: &Mesh,
        high_poly: Option<&Mesh>,
        frontal_dist: f32,
        rear_dist: f32,
        width: u32,
        height: u32,
        match_config: &BakeMatchConfig,
        cage_config: &BakeCageConfig,
    ) -> Result<RgbaBuffer, String> {
        Self::bake_tangent_normals_with_options(
            mesh,
            high_poly,
            frontal_dist,
            rear_dist,
            width,
            height,
            match_config,
            cage_config,
            false,
        )
    }

    /// Bake Tangent Space Normal map with cage envelope projection, matching mode, and optional DirectX green channel inversion
    pub fn bake_tangent_normals_with_options(
        mesh: &Mesh,
        high_poly: Option<&Mesh>,
        frontal_dist: f32,
        rear_dist: f32,
        width: u32,
        height: u32,
        match_config: &BakeMatchConfig,
        cage_config: &BakeCageConfig,
        invert_green: bool,
    ) -> Result<RgbaBuffer, String> {
        let mut buffer = RgbaBuffer::new(width, height, Vec4::new(0.5, 0.5, 1.0, 1.0));
        let tri_count = mesh.indices.len() / 3;

        let cage_vertices = Self::compute_cage_vertices(mesh, cage_config)?;
        let has_cage = cage_vertices.is_some();

        let spatial_hash = high_poly.map(|hp| SpatialHash::new(hp));

        // Generate position, normal, tangent, cage and triangle index raster buffers for Low Poly
        let mut pos_buf = vec![None; (width * height) as usize];
        let mut norm_buf = vec![None; (width * height) as usize];
        let mut tan_buf = vec![None; (width * height) as usize];
        let mut tri_buf = vec![None; (width * height) as usize];
        let mut cage_buf = if has_cage {
            Some(vec![None; (width * height) as usize])
        } else {
            None
        };

        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let v0 = &mesh.vertices[i0];
            let v1 = &mesh.vertices[i1];
            let v2 = &mesh.vertices[i2];

            let cage_pts = cage_vertices.as_ref().map(|cv| (cv[i0], cv[i1], cv[i2]));

            Self::rasterize_pos_norm_tan_triangle(
                &mut pos_buf,
                &mut norm_buf,
                &mut tan_buf,
                Some(&mut tri_buf),
                cage_buf.as_deref_mut(),
                t,
                width,
                height,
                v0.uv, v1.uv, v2.uv,
                v0.position, v1.position, v2.position,
                v0.normal, v1.normal, v2.normal,
                v0.tangent, v1.tangent, v2.tangent,
                cage_pts,
            );
        }

        // Precompute allowed high-poly triangle sets per low-poly submesh for ByMeshName matching
        let submesh_allowed_hp_tris: Vec<Option<HashSet<usize>>> = if match_config.mode == BakeMatchMode::ByMeshName {
            if let Some(hp) = high_poly {
                mesh.submeshes.iter().map(|sm_low| {
                    let base_low = strip_mesh_suffix(&sm_low.material_name, &match_config.low_suffix);
                    let mut matched_tris = HashSet::new();
                    for sm_hp in &hp.submeshes {
                        let base_hp = strip_mesh_suffix(&sm_hp.material_name, &match_config.high_suffix);
                        if base_low.eq_ignore_ascii_case(&base_hp) {
                            let tri_start = sm_hp.start_index / 3;
                            let tri_count = sm_hp.index_count / 3;
                            for t in tri_start..(tri_start + tri_count) {
                                matched_tris.insert(t);
                            }
                        }
                    }
                    if !matched_tris.is_empty() {
                        Some(matched_tris)
                    } else {
                        None
                    }
                }).collect()
            } else {
                vec![None; mesh.submeshes.len()]
            }
        } else {
            vec![None; mesh.submeshes.len()]
        };

        // Raycast
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if let (Some(pos), Some(norm)) = (pos_buf[idx], norm_buf[idx]) {
                    if let Some(hp) = high_poly {
                        if let Some(hash) = &spatial_hash {
                            // When cage buffer is present, ray originates from cage envelope down to low-poly
                            let (ray_orig, ray_dir, max_ray_dist) = if let Some(cage_pos) = cage_buf.as_ref().and_then(|cb| cb[idx]) {
                                let to_mesh = pos - cage_pos;
                                let dist_to_surface = to_mesh.length();
                                if dist_to_surface > 1e-6 {
                                    let dir = to_mesh / dist_to_surface;
                                    (cage_pos, dir, dist_to_surface + rear_dist)
                                } else {
                                    (pos + norm * frontal_dist, -norm, frontal_dist + rear_dist)
                                }
                            } else {
                                (pos + norm * frontal_dist, -norm, frontal_dist + rear_dist)
                            };

                            let allowed = if match_config.mode == BakeMatchMode::ByMeshName {
                                tri_buf[idx].and_then(|t| mesh.submesh_index_for_triangle(t))
                                    .and_then(|sm_idx| submesh_allowed_hp_tris.get(sm_idx))
                                    .and_then(|opt| opt.as_ref())
                            } else {
                                None
                            };

                            if let Some((_, hp_norm)) = hash.query_ray_closest_filtered(ray_orig, ray_dir, max_ray_dist, hp, allowed) {
                                if let Some(tan) = tan_buf[idx] {
                                    let n = norm.normalize_or_zero();
                                    let t = (tan.truncate() - n * n.dot(tan.truncate())).normalize_or_zero();
                                    let tw = if tan.w < 0.0 { -1.0 } else { 1.0 };
                                    let b = n.cross(t) * tw;
                                    let ts_norm = Vec3::new(hp_norm.dot(t), hp_norm.dot(b), hp_norm.dot(n)).normalize_or_zero();
                                    let ny = if invert_green { -ts_norm.y } else { ts_norm.y };
                                    buffer.set_pixel(x, y, Vec4::new(ts_norm.x * 0.5 + 0.5, ny * 0.5 + 0.5, ts_norm.z * 0.5 + 0.5, 1.0));
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
        Ok(buffer)
    }


    /// Bake World Position Object (WPO) / Position map normalized to [0, 1] bounding box
    pub fn bake_position(mesh: &Mesh, width: u32, height: u32) -> RgbaBuffer {
        Self::bake_position_with_options(mesh, width, height, &PositionBakeConfig::default())
    }

    /// Bake World Position Object (WPO) / Position map with configurable normalization mode
    pub fn bake_position_with_options(
        mesh: &Mesh,
        width: u32,
        height: u32,
        config: &PositionBakeConfig,
    ) -> RgbaBuffer {
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
                None,
                None,
                t,
                width,
                height,
                v0.uv, v1.uv, v2.uv,
                v0.position, v1.position, v2.position,
                v0.normal, v1.normal, v2.normal,
                v0.tangent, v1.tangent, v2.tangent,
                None,
            );
        }

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if let Some(pos) = pos_buf[idx] {
                    let norm_pos = match config.mode {
                        PositionNormalizationMode::BoundingBox => {
                            ((pos - min_bound) / extents).clamp(Vec3::ZERO, Vec3::ONE)
                        }
                        PositionNormalizationMode::WorldUnits => {
                            (pos * 0.5 + Vec3::splat(0.5)).clamp(Vec3::ZERO, Vec3::ONE)
                        }
                    };
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
                None,
                None,
                t,
                width,
                height,
                v0.uv, v1.uv, v2.uv,
                v0.position, v1.position, v2.position,
                v0.normal, v1.normal, v2.normal,
                v0.tangent, v1.tangent, v2.tangent,
                None,
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

    /// Bake discrete Color ID map for sub-mesh / material selection masks
    pub fn bake_id_map(mesh: &Mesh, width: u32, height: u32) -> RgbaBuffer {
        let mut buffer = RgbaBuffer::new(width, height, Vec4::new(0.0, 0.0, 0.0, 1.0));
        let tri_count = mesh.indices.len() / 3;
        if tri_count == 0 {
            return buffer;
        }

        const PALETTE: [Vec3; 16] = [
            Vec3::new(0.95, 0.15, 0.15), // Red
            Vec3::new(0.15, 0.85, 0.15), // Green
            Vec3::new(0.15, 0.35, 0.95), // Blue
            Vec3::new(0.95, 0.85, 0.15), // Yellow
            Vec3::new(0.15, 0.90, 0.90), // Cyan
            Vec3::new(0.90, 0.15, 0.90), // Magenta
            Vec3::new(0.95, 0.50, 0.10), // Orange
            Vec3::new(0.55, 0.15, 0.95), // Purple
            Vec3::new(0.50, 0.95, 0.15), // Lime
            Vec3::new(0.10, 0.55, 0.55), // Teal
            Vec3::new(0.95, 0.40, 0.65), // Pink
            Vec3::new(0.95, 0.70, 0.10), // Amber
            Vec3::new(0.30, 0.60, 0.95), // Sky Blue
            Vec3::new(0.60, 0.35, 0.15), // Brown
            Vec3::new(0.20, 0.80, 0.50), // Emerald
            Vec3::new(0.85, 0.85, 0.85), // Silver
        ];

        let has_submeshes = mesh.submeshes.len() > 1;
        let mut id_colors = vec![Vec3::ZERO; tri_count];

        if has_submeshes {
            for t in 0..tri_count {
                let sm_idx = mesh.submesh_index_for_triangle(t).unwrap_or(0);
                id_colors[t] = PALETTE[sm_idx % PALETTE.len()];
            }
        } else {
            let mut visited = vec![false; tri_count];
            let mut island_idx = 0;
            for t in 0..tri_count {
                if !visited[t] {
                    let island = mesh.get_connected_uv_island(t);
                    let col = PALETTE[island_idx % PALETTE.len()];
                    for &tri in &island {
                        if tri < tri_count {
                            visited[tri] = true;
                            id_colors[tri] = col;
                        }
                    }
                    island_idx += 1;
                }
            }
        }

        let mut mask_buf: Vec<Option<Vec3>> = vec![None; (width * height) as usize];

        for t in 0..tri_count {
            let i0 = mesh.indices[t * 3] as usize;
            let i1 = mesh.indices[t * 3 + 1] as usize;
            let i2 = mesh.indices[t * 3 + 2] as usize;

            let uv0 = mesh.vertices[i0].uv;
            let uv1 = mesh.vertices[i1].uv;
            let uv2 = mesh.vertices[i2].uv;
            let color = id_colors[t];

            let min_u = uv0.x.min(uv1.x).min(uv2.x).max(0.0);
            let max_u = uv0.x.max(uv1.x).max(uv2.x).min(1.0);
            let min_v = uv0.y.min(uv1.y).min(uv2.y).max(0.0);
            let max_v = uv0.y.max(uv1.y).max(uv2.y).min(1.0);

            let min_x = (min_u * (width as f32)).floor() as u32;
            let max_x = ((max_u * (width as f32)).ceil() as u32).min(width.saturating_sub(1));
            let min_y = (min_v * (height as f32)).floor() as u32;
            let max_y = ((max_v * (height as f32)).ceil() as u32).min(height.saturating_sub(1));

            let p0 = Vec2::new(uv0.x * width as f32, uv0.y * height as f32);
            let p1 = Vec2::new(uv1.x * width as f32, uv1.y * height as f32);
            let p2 = Vec2::new(uv2.x * width as f32, uv2.y * height as f32);

            let edge_func = |a: Vec2, b: Vec2, c: Vec2| -> f32 {
                (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)
            };
            let area = edge_func(p0, p1, p2);
            if area.abs() < 1e-5 {
                continue;
            }

            for py in min_y..=max_y {
                for px in min_x..=max_x {
                    let p = Vec2::new(px as f32 + 0.5, py as f32 + 0.5);
                    let w0 = edge_func(p1, p2, p);
                    let w1 = edge_func(p2, p0, p);
                    let w2 = edge_func(p0, p1, p);

                    let inside = if area > 0.0 {
                        w0 >= -0.01 && w1 >= -0.01 && w2 >= -0.01
                    } else {
                        w0 <= 0.01 && w1 <= 0.01 && w2 <= 0.01
                    };

                    if inside {
                        let idx = (py * width + px) as usize;
                        mask_buf[idx] = Some(color);
                        buffer.set_pixel(px, py, Vec4::new(color.x, color.y, color.z, 1.0));
                    }
                }
            }
        }

        Self::dilate_rgba(&mut buffer, &mask_buf);
        buffer
    }

    /// Bake Surface Thickness map for subsurface scattering and light transmission
    pub fn bake_thickness(mesh: &Mesh, width: u32, height: u32) -> ScalarBuffer {
        Self::bake_thickness_with_options(mesh, width, height, &BakeCageConfig::default(), &ThicknessBakeConfig::default())
            .unwrap_or_else(|_| ScalarBuffer::new(width, height, 1.0))
    }

    /// Bake Surface Thickness map with optional cage envelope support
    #[allow(dead_code)]
    pub fn bake_thickness_with_cage(
        mesh: &Mesh,
        width: u32,
        height: u32,
        cage_config: &BakeCageConfig,
    ) -> Result<ScalarBuffer, String> {
        Self::bake_thickness_with_options(mesh, width, height, cage_config, &ThicknessBakeConfig::default())
    }

    /// Bake Surface Thickness map with customizable sample count, ray reach, bias, and invert option
    pub fn bake_thickness_with_options(
        mesh: &Mesh,
        width: u32,
        height: u32,
        cage_config: &BakeCageConfig,
        config: &ThicknessBakeConfig,
    ) -> Result<ScalarBuffer, String> {
        let mut buffer = ScalarBuffer::new(width, height, 1.0);
        let tri_count = mesh.indices.len() / 3;
        if tri_count == 0 || mesh.vertices.is_empty() {
            return Ok(buffer);
        }

        let cage_verts = Self::compute_cage_vertices(mesh, cage_config)?;

        let sample_count = config.samples.clamp(4, 1024);
        let mut hemisphere_samples = Vec::with_capacity(sample_count);
        for i in 0..sample_count {
            let u1 = (i as f32 + 0.5) / sample_count as f32;
            let u2 = (i as f32 * 0.6180339887).fract();
            let cos_theta = (1.0 - u1).sqrt();
            let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
            let phi = u2 * 2.0 * std::f32::consts::PI;
            // Pointing inward into the mesh (negative z along -normal)
            let dir = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), -cos_theta);
            hemisphere_samples.push(dir);
        }

        let extents = mesh.aabb_max - mesh.aabb_min;
        let diag = extents.length();
        let max_dist = (diag * config.max_distance).max(0.01);
        let ray_offset = (diag * config.ray_bias).max(0.001);
        let min_self_dist = (ray_offset * 2.0).max(0.003);

        // Rayon parallel computation of vertex thickness
        let vert_thickness: Vec<f32> = mesh.vertices.par_iter().enumerate().map(|(vi, v)| {
            let normal = if v.normal.length_squared() > 1e-4 {
                v.normal.normalize()
            } else {
                Vec3::Y
            };

            // Build orthonormal basis (tangent, bitangent, normal)
            let up = if normal.y.abs() < 0.95 { Vec3::Y } else { Vec3::X };
            let tangent = normal.cross(up).normalize();
            let bitangent = normal.cross(tangent);

            let offset_dist = if let Some(ref cv) = cage_verts {
                let c_pos = cv.get(vi).copied().unwrap_or(v.position);
                (c_pos - v.position).length().max(ray_offset)
            } else {
                ray_offset
            };

            // Start slightly inside the surface along -normal
            let origin = v.position - normal * offset_dist.min(ray_offset * 2.0);
            let mut accum_dist = 0.0f32;

            for sample in &hemisphere_samples {
                let inward_dir = (tangent * sample.x + bitangent * sample.y + normal * sample.z).normalize();
                let ray = Ray {
                    origin,
                    direction: inward_dir,
                };

                if let Some(hit) = mesh.raycast(&ray) {
                    if hit.t > min_self_dist && hit.t < max_dist {
                        accum_dist += hit.t;
                    } else if hit.t >= max_dist {
                        accum_dist += max_dist;
                    } else {
                        // Very close hit rejected as self-intersection
                        accum_dist += max_dist;
                    }
                } else {
                    // No exit hit found within BVH; volume is at least max_dist thick
                    accum_dist += max_dist;
                }
            }

            let avg_dist = accum_dist / (sample_count as f32);
            (avg_dist / max_dist).clamp(0.0, 1.0)
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
                vert_thickness[i0], vert_thickness[i1], vert_thickness[i2],
            );
        }

        Self::dilate_scalar(&mut buffer, &mut written_mask, 16);
        if config.invert {
            buffer.invert();
        }
        Ok(buffer)
    }

    
    fn rasterize_pos_norm_tan_triangle(
        pos_buf: &mut [Option<Vec3>],
        norm_buf: &mut [Option<Vec3>],
        tan_buf: &mut [Option<Vec4>],
        mut tri_buf: Option<&mut [Option<usize>]>,
        mut cage_buf: Option<&mut [Option<Vec3>]>,
        t_idx: usize,
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
        cage_pts: Option<(Vec3, Vec3, Vec3)>,
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
                    if let Some(ref mut tb) = tri_buf {
                        tb[idx] = Some(t_idx);
                    }
                    if let (Some(ref mut cb), Some((c0, c1, c2))) = (&mut cage_buf, cage_pts) {
                        cb[idx] = Some(c0 * w0 + c1 * w1 + c2 * w2);
                    }
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

        let torus = Mesh::create_torus();
        let torus_thick = Baker::bake_thickness(&torus, 32, 32);

        for &val in &thick_map.data {
            assert!(!val.is_nan() && !val.is_infinite());
            assert!(val >= 0.0 && val <= 1.0, "Thickness must be normalized in [0, 1]");
        }

        let sphere_avg = thick_map.data.iter().sum::<f32>() / thick_map.data.len() as f32;
        let torus_avg = torus_thick.data.iter().sum::<f32>() / torus_thick.data.len() as f32;
        assert!(torus_avg < sphere_avg, "Thin torus tube must have lower thickness than solid sphere: torus={}, sphere={}", torus_avg, sphere_avg);
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

    #[test]
    fn test_baker_id_map() {
        let cube = Mesh::create_cube_mesh("Cube", 1.0);
        let id_map = Baker::bake_id_map(&cube, 32, 32);

        assert_eq!(id_map.width, 32);
        assert_eq!(id_map.height, 32);

        let mut has_non_zero = false;
        for &px in &id_map.data {
            assert!(!px.x.is_nan() && !px.y.is_nan() && !px.z.is_nan());
            assert_eq!(px.w, 1.0, "Alpha must be 1.0");
            if px.x > 0.01 || px.y > 0.01 || px.z > 0.01 {
                has_non_zero = true;
            }
        }
        assert!(has_non_zero, "ID map must contain rasterized palette colors for cube faces");
    }

    #[test]
    fn test_baker_match_by_mesh_name() {
        assert_eq!(strip_mesh_suffix("Arm_Armor_low", "_low"), "Arm_Armor");
        assert_eq!(strip_mesh_suffix("Arm_Armor_high", "_high"), "Arm_Armor");
        assert_eq!(strip_mesh_suffix("Helmet_LOW", "_low"), "Helmet");
        assert_eq!(strip_mesh_suffix("Weapon_HIGH", "_high"), "Weapon");

        let cube_low = Mesh::create_cube_mesh("Cube_low", 1.0);
        let cube_high = Mesh::create_cube_mesh("Cube_high", 1.0);

        let config_by_name = BakeMatchConfig {
            mode: BakeMatchMode::ByMeshName,
            low_suffix: "_low".to_string(),
            high_suffix: "_high".to_string(),
        };

        let normal_map = Baker::bake_tangent_normals_with_match(&cube_low, Some(&cube_high), 0.1, 0.1, 16, 16, &config_by_name);
        assert_eq!(normal_map.width, 16);
        assert_eq!(normal_map.height, 16);
        for &px in &normal_map.data {
            assert!(!px.x.is_nan() && !px.y.is_nan() && !px.z.is_nan());
        }
    }

    #[test]
    fn test_baker_parametric_cage_inflation() {
        let cube = Mesh::create_cube_mesh("Cube", 1.0);
        let orig_vert_count = cube.vertices.len();

        // 1. None mode
        let none_config = BakeCageConfig {
            mode: BakeCageMode::None,
            inflation_offset: 0.05,
            external_cage_path: None,
        };
        let none_res = Baker::compute_cage_vertices(&cube, &none_config).unwrap();
        assert!(none_res.is_none(), "None mode must return None");

        // 2. Parametric inflation mode
        let inflation_offset = 0.05;
        let cage_config = BakeCageConfig {
            mode: BakeCageMode::ParametricInflation,
            inflation_offset,
            external_cage_path: None,
        };
        let cage_verts = Baker::compute_cage_vertices(&cube, &cage_config).unwrap().expect("Parametric mode must produce cage vertices");
        assert_eq!(cage_verts.len(), orig_vert_count);

        // Verify that vertices have been pushed outward
        for (i, &cv) in cage_verts.iter().enumerate() {
            let orig_pos = cube.vertices[i].position;
            assert!(cv.length() > orig_pos.length(), "Inflated vertex must be further from center than original");
        }

        // Verify that co-located corner vertices weld to the exact same continuous cage coordinate
        for i in 0..orig_vert_count {
            for j in (i + 1)..orig_vert_count {
                if (cube.vertices[i].position - cube.vertices[j].position).length() < 1e-4 {
                    let diff = (cage_verts[i] - cage_verts[j]).length();
                    assert!(diff < 1e-4, "Seam corner vertices must weld smoothly without gaps or cracks in the cage envelope");
                }
            }
        }
    }

    #[test]
    fn test_baker_external_cage_loading_and_validation() {
        let cube_raw = Mesh::create_cube_mesh("Cube", 1.0);

        // Create temporary directory for OBJ files
        let tmp_dir = std::env::temp_dir();
        let valid_cage_path = tmp_dir.join("test_valid_cage.obj");
        let invalid_cage_path = tmp_dir.join("test_invalid_cage.obj");

        // Save a valid cage with matching topology and load low-poly mesh from OBJ
        cube_raw.save_to_obj_file(&valid_cage_path).expect("Failed to save test cage OBJ");
        let cube = Mesh::from_obj_file(&valid_cage_path).expect("Failed to load cube OBJ");

        // Save a mismatching sphere OBJ
        let sphere = Mesh::create_sphere(1.0, 8, 8);
        sphere.save_to_obj_file(&invalid_cage_path).expect("Failed to save mismatch test cage OBJ");

        // Test valid cage
        let valid_config = BakeCageConfig {
            mode: BakeCageMode::ExternalCage,
            inflation_offset: 0.0,
            external_cage_path: Some(valid_cage_path.clone()),
        };
        let valid_res = Baker::compute_cage_vertices(&cube, &valid_config);
        assert!(valid_res.is_ok(), "Valid matching cage OBJ must succeed");
        let valid_verts = valid_res.unwrap().unwrap();
        assert_eq!(valid_verts.len(), cube.vertices.len());

        // Test invalid mismatching cage topology
        let invalid_config = BakeCageConfig {
            mode: BakeCageMode::ExternalCage,
            inflation_offset: 0.0,
            external_cage_path: Some(invalid_cage_path.clone()),
        };
        let invalid_res = Baker::compute_cage_vertices(&cube, &invalid_config);
        assert!(invalid_res.is_err(), "Mismatching cage topology must return an error");
        let err_msg = invalid_res.unwrap_err();
        assert!(err_msg.contains("does not match low-poly mesh vertex count"), "Error message must report topology mismatch: {}", err_msg);

        // Test missing file path
        let missing_path_config = BakeCageConfig {
            mode: BakeCageMode::ExternalCage,
            inflation_offset: 0.0,
            external_cage_path: None,
        };
        let missing_res = Baker::compute_cage_vertices(&cube, &missing_path_config);
        assert!(missing_res.is_err(), "Missing file path must error");

        // Cleanup
        let _ = std::fs::remove_file(valid_cage_path);
        let _ = std::fs::remove_file(invalid_cage_path);
    }

    #[test]
    fn test_baker_tangent_normals_and_maps_with_cage() {
        let cube_low = Mesh::create_cube_mesh("Cube_low", 1.0);
        let cube_high = Mesh::create_cube_mesh("Cube_high", 1.0);

        let cage_config = BakeCageConfig {
            mode: BakeCageMode::ParametricInflation,
            inflation_offset: 0.03,
            external_cage_path: None,
        };

        let normal_map = Baker::bake_tangent_normals_with_cage(
            &cube_low,
            Some(&cube_high),
            0.1,
            0.1,
            16,
            16,
            &BakeMatchConfig::default(),
            &cage_config,
        ).expect("Tangent normal bake with cage must succeed");

        assert_eq!(normal_map.width, 16);
        assert_eq!(normal_map.height, 16);
        for &px in &normal_map.data {
            assert!(!px.x.is_nan() && !px.y.is_nan() && !px.z.is_nan());
            assert_eq!(px.w, 1.0);
        }

        let ao_map = Baker::bake_ao_with_cage(&cube_low, 16, 16, &cage_config).expect("AO bake with cage must succeed");
        assert_eq!(ao_map.width, 16);
        assert_eq!(ao_map.height, 16);

        let thick_map = Baker::bake_thickness_with_cage(&cube_low, 16, 16, &cage_config).expect("Thickness bake with cage must succeed");
        assert_eq!(thick_map.width, 16);
        assert_eq!(thick_map.height, 16);
    }

    #[test]
    fn test_baker_configurable_map_options() {
        let sphere = Mesh::create_sphere(1.0, 16, 12);

        // Test configurable AO samples and parameters
        let ao_low = Baker::bake_ao_with_options(
            &sphere,
            16,
            16,
            &BakeCageConfig::default(),
            &AoBakeConfig {
                samples: 16,
                max_distance: 0.2,
                ray_bias: 0.01,
                spread: 1.2,
                contrast: 1.5,
            },
        ).expect("AO with options should succeed");
        assert_eq!(ao_low.width, 16);
        assert_eq!(ao_low.height, 16);

        let ao_high = Baker::bake_ao_with_options(
            &sphere,
            16,
            16,
            &BakeCageConfig::default(),
            &AoBakeConfig {
                samples: 128,
                max_distance: 0.5,
                ray_bias: 0.005,
                spread: 0.8,
                contrast: 1.0,
            },
        ).expect("AO with 128 samples should succeed");
        assert_eq!(ao_high.width, 16);
        assert_eq!(ao_high.height, 16);

        // Test configurable Thickness samples and invert
        let th_norm = Baker::bake_thickness_with_options(
            &sphere,
            16,
            16,
            &BakeCageConfig::default(),
            &ThicknessBakeConfig {
                samples: 16,
                max_distance: 0.5,
                ray_bias: 0.005,
                invert: false,
            },
        ).expect("Thickness normal should succeed");

        let th_inv = Baker::bake_thickness_with_options(
            &sphere,
            16,
            16,
            &BakeCageConfig::default(),
            &ThicknessBakeConfig {
                samples: 16,
                max_distance: 0.5,
                ray_bias: 0.005,
                invert: true,
            },
        ).expect("Thickness inverted should succeed");

        // First non-border pixel should satisfy invert
        let val_norm = th_norm.get_pixel(8, 8);
        let val_inv = th_inv.get_pixel(8, 8);
        assert!((val_norm + val_inv - 1.0).abs() < 1e-4, "Inverted thickness should equal 1.0 - normal");

        // Test Curvature options
        let curv_custom = Baker::bake_curvature_with_options(
            &sphere,
            16,
            16,
            &CurvatureBakeConfig {
                intensity: 1.5,
                contrast: 1.2,
                dihedral_weight: 0.5,
                deflection_weight: 0.3,
                blur_passes: 2,
            },
        );
        assert_eq!(curv_custom.width, 16);

        // Test Position options
        let pos_world = Baker::bake_position_with_options(
            &sphere,
            16,
            16,
            &PositionBakeConfig {
                mode: PositionNormalizationMode::WorldUnits,
            },
        );
        assert_eq!(pos_world.width, 16);
    }
}

