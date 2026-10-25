// 3D Paintcraft - Substance-style Flood Fill Segmentation & Procedural Variation Engine
// Pure safe Rust with Rayon parallelism and Connected Component Labeling (CCL)

use crate::channel::{RgbaBuffer, ScalarBuffer};
use crate::mesh::Mesh;
use glam::{Vec2, Vec4};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Mode of procedural output generated from flood-filled islands
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FloodFillMode {
    /// Deterministic uniform pseudo-random grayscale value [0.0, 1.0] per island
    RandomGrayscale,
    /// Radial falloff or distance from each island's centroid (dome / circular bevel)
    CentroidDistance,
    /// Normalized horizontal centroid coordinate (U / X) across UV space
    CentroidX,
    /// Normalized vertical centroid coordinate (V / Y) across UV space
    CentroidY,
    /// Normalized bounding box pixel area / size of each island
    BoundingBoxArea,
    /// Directional linear gradient across each island (fixed or randomized angle)
    AngleGradient,
}

impl FloodFillMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::RandomGrayscale => "Random Grayscale",
            Self::CentroidDistance => "Centroid Distance (Dome)",
            Self::CentroidX => "Centroid Position X",
            Self::CentroidY => "Centroid Position Y",
            Self::BoundingBoxArea => "Bounding Box Area",
            Self::AngleGradient => "Angle Gradient",
        }
    }

    pub fn all() -> &'static [FloodFillMode] {
        &[
            Self::RandomGrayscale,
            Self::CentroidDistance,
            Self::CentroidX,
            Self::CentroidY,
            Self::BoundingBoxArea,
            Self::AngleGradient,
        ]
    }
}

/// Statistics and geometry metrics for a single connected component (island)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FloodFillIsland {
    pub id: u32,
    pub pixel_count: usize,
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
    pub centroid_x: f32, // Normalized [0.0, 1.0]
    pub centroid_y: f32, // Normalized [0.0, 1.0]
    pub random_val: f32, // Deterministic pseudo-random [0.0, 1.0]
    pub random_color: [f32; 3], // Deterministic RGB [0.0, 1.0]
    pub random_angle: f32, // Angle in radians [0.0, 2*PI]
}

/// Result of Connected Component Labeling (CCL) segmentation
#[derive(Clone, Debug)]
pub struct FloodFillResult {
    pub width: u32,
    pub height: u32,
    /// 0 indicates background / boundary; 1..=N indicates island ID
    pub labels: Vec<u32>,
    pub islands: HashMap<u32, FloodFillIsland>,
    pub max_area: usize,
}

/// Disjoint Set / Union-Find with path compression and union by rank
struct DisjointSet {
    parent: Vec<u32>,
    rank: Vec<u8>,
}

impl DisjointSet {
    fn new(capacity: usize) -> Self {
        Self {
            parent: (0..capacity as u32).collect(),
            rank: vec![0; capacity],
        }
    }

    fn find(&mut self, x: u32) -> u32 {
        let mut root = x;
        while root != self.parent[root as usize] {
            root = self.parent[root as usize];
        }
        // Path compression
        let mut curr = x;
        while curr != root {
            let nxt = self.parent[curr as usize];
            self.parent[curr as usize] = root;
            curr = nxt;
        }
        root
    }

    fn union(&mut self, x: u32, y: u32) {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx != ry {
            if self.rank[rx as usize] < self.rank[ry as usize] {
                self.parent[rx as usize] = ry;
            } else if self.rank[rx as usize] > self.rank[ry as usize] {
                self.parent[ry as usize] = rx;
            } else {
                self.parent[ry as usize] = rx;
                self.rank[rx as usize] = self.rank[rx as usize].saturating_add(1);
            }
        }
    }
}

/// Wang / Murmur3 32-bit pseudo-random integer hash
#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    x = (x ^ 61) ^ (x >> 16);
    x = x.wrapping_add(x << 3);
    x ^= x >> 4;
    x = x.wrapping_mul(0x27d4eb2d);
    x ^= x >> 15;
    x
}

/// Substance 3D Designer / Painter style Flood Fill Engine
pub struct FloodFillEngine;

impl FloodFillEngine {
    /// Perform 8-connected component labeling from a scalar mask
    ///
    /// Pixels where `(val >= threshold) ^ invert` are treated as foreground islands.
    /// Pixels below threshold are treated as boundaries / background (e.g. tile grout, cracks, gaps).
    pub fn flood_fill_from_mask(
        mask: &ScalarBuffer,
        threshold: f32,
        invert: bool,
        min_area: usize,
        seed: u32,
    ) -> FloodFillResult {
        let w = mask.width;
        let h = mask.height;
        if w == 0 || h == 0 {
            return FloodFillResult {
                width: 0,
                height: 0,
                labels: Vec::new(),
                islands: HashMap::new(),
                max_area: 0,
            };
        }

        let total_pixels = (w * h) as usize;
        let mut labels = vec![0u32; total_pixels];
        // Capacity estimated for max provisional labels
        let mut ds = DisjointSet::new(total_pixels.min(131072) + 1);
        let mut next_label = 1u32;

        let th = threshold.clamp(0.0, 1.0);

        // Pass 1: 8-connected labeling
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let val = mask.data[idx];
                let is_fg = if invert { val < th } else { val >= th };

                if !is_fg {
                    continue;
                }

                // Check already visited 8-neighbors: West, North-West, North, North-East
                let mut neighbor_labels = [0u32; 4];
                let mut n_count = 0;

                // West
                if x > 0 {
                    let lbl = labels[idx - 1];
                    if lbl > 0 {
                        neighbor_labels[n_count] = lbl;
                        n_count += 1;
                    }
                }
                // North-West
                if x > 0 && y > 0 {
                    let lbl = labels[idx - (w as usize) - 1];
                    if lbl > 0 {
                        neighbor_labels[n_count] = lbl;
                        n_count += 1;
                    }
                }
                // North
                if y > 0 {
                    let lbl = labels[idx - (w as usize)];
                    if lbl > 0 {
                        neighbor_labels[n_count] = lbl;
                        n_count += 1;
                    }
                }
                // North-East
                if x + 1 < w && y > 0 {
                    let lbl = labels[idx - (w as usize) + 1];
                    if lbl > 0 {
                        neighbor_labels[n_count] = lbl;
                        n_count += 1;
                    }
                }

                if n_count == 0 {
                    let lbl = next_label;
                    if (lbl as usize) >= ds.parent.len() {
                        let new_cap = ds.parent.len() * 2;
                        ds.parent.reserve(new_cap - ds.parent.len());
                        for i in ds.parent.len()..new_cap {
                            ds.parent.push(i as u32);
                            ds.rank.push(0);
                        }
                    }
                    labels[idx] = lbl;
                    next_label += 1;
                } else {
                    let min_lbl = neighbor_labels[0..n_count].iter().copied().min().unwrap();
                    labels[idx] = min_lbl;
                    for &nl in &neighbor_labels[0..n_count] {
                        if nl != min_lbl {
                            ds.union(min_lbl, nl);
                        }
                    }
                }
            }
        }

        // Pass 2: Root resolution and component pixel counting
        let mut root_counts: HashMap<u32, usize> = HashMap::new();
        for lbl in labels.iter_mut() {
            if *lbl > 0 {
                let root = ds.find(*lbl);
                *lbl = root;
                *root_counts.entry(root).or_insert(0) += 1;
            }
        }

        // Pass 3: Dense re-indexing and area thresholding
        let min_sz = min_area.max(1);
        let mut root_to_id: HashMap<u32, u32> = HashMap::new();
        let mut curr_id = 1u32;
        for (&root, &count) in &root_counts {
            if count >= min_sz {
                root_to_id.insert(root, curr_id);
                curr_id += 1;
            }
        }

        struct IslandStats {
            pixel_count: usize,
            min_x: u32,
            min_y: u32,
            max_x: u32,
            max_y: u32,
            sum_x: u64,
            sum_y: u64,
        }

        let mut island_stats: HashMap<u32, IslandStats> = HashMap::new();

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let root = labels[idx];
                if root > 0 {
                    if let Some(&dense_id) = root_to_id.get(&root) {
                        labels[idx] = dense_id;
                        let stat = island_stats.entry(dense_id).or_insert(IslandStats {
                            pixel_count: 0,
                            min_x: x,
                            min_y: y,
                            max_x: x,
                            max_y: y,
                            sum_x: 0,
                            sum_y: 0,
                        });
                        stat.pixel_count += 1;
                        stat.min_x = stat.min_x.min(x);
                        stat.min_y = stat.min_y.min(y);
                        stat.max_x = stat.max_x.max(x);
                        stat.max_y = stat.max_y.max(y);
                        stat.sum_x += x as u64;
                        stat.sum_y += y as u64;
                    } else {
                        labels[idx] = 0; // Filtered out as noise
                    }
                }
            }
        }

        let mut max_area = 1usize;
        let mut islands = HashMap::new();

        for (id, stat) in island_stats {
            max_area = max_area.max(stat.pixel_count);
            let count_f = stat.pixel_count.max(1) as f32;
            let cx = (stat.sum_x as f32 / count_f + 0.5) / (w as f32);
            let cy = (stat.sum_y as f32 / count_f + 0.5) / (h as f32);

            // Deterministic pseudo-random numbers per island
            let h1 = hash_u32(id.wrapping_add(seed.wrapping_mul(0x9e3779b9)));
            let h2 = hash_u32(h1.wrapping_add(1013));
            let h3 = hash_u32(h2.wrapping_add(3747));

            let random_val = (h1 as f32) / (u32::MAX as f32);
            let r_col = (h1 as f32) / (u32::MAX as f32);
            let g_col = (h2 as f32) / (u32::MAX as f32);
            let b_col = (h3 as f32) / (u32::MAX as f32);
            let random_angle = ((h2 as f32) / (u32::MAX as f32)) * std::f32::consts::TAU;

            islands.insert(
                id,
                FloodFillIsland {
                    id,
                    pixel_count: stat.pixel_count,
                    min_x: stat.min_x,
                    min_y: stat.min_y,
                    max_x: stat.max_x,
                    max_y: stat.max_y,
                    centroid_x: cx.clamp(0.0, 1.0),
                    centroid_y: cy.clamp(0.0, 1.0),
                    random_val,
                    random_color: [r_col, g_col, b_col],
                    random_angle,
                },
            );
        }

        FloodFillResult {
            width: w,
            height: h,
            labels,
            islands,
            max_area,
        }
    }

    /// Extract connected UV islands (UV shells) from 3D Mesh triangles and rasterize into a FloodFillResult
    pub fn flood_fill_from_mesh_uvs(
        mesh: &Mesh,
        width: u32,
        height: u32,
        seed: u32,
    ) -> FloodFillResult {
        let tri_count = mesh.indices.len() / 3;
        if tri_count == 0 || width == 0 || height == 0 {
            return FloodFillResult {
                width,
                height,
                labels: vec![0; (width * height) as usize],
                islands: HashMap::new(),
                max_area: 0,
            };
        }

        // Build triangle adjacency in UV space
        // Two triangles share an edge if they share two vertices whose UVs match within epsilon
        let mut ds = DisjointSet::new(tri_count);

        // Group vertices by quantized UV integer coordinate to find shared seams
        let mut uv_to_tris: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let scale = 10000.0f32;

        for (tri_idx, chunk) in mesh.indices.chunks_exact(3).enumerate() {
            for &vi in chunk {
                if let Some(v) = mesh.vertices.get(vi as usize) {
                    let key = (
                        (v.uv.x * scale).round() as i32,
                        (v.uv.y * scale).round() as i32,
                    );
                    uv_to_tris.entry(key).or_default().push(tri_idx);
                }
            }
        }

        // For each shared UV vertex, triangles sharing it may share edges
        // Union triangles sharing common UV vertices
        for tris in uv_to_tris.values() {
            if tris.len() > 1 {
                let first = tris[0];
                for &t in &tris[1..] {
                    ds.union(first as u32, t as u32);
                }
            }
        }

        // Map triangle root IDs to dense island IDs
        let mut tri_root_to_island: HashMap<u32, u32> = HashMap::new();
        let mut next_island_id = 1u32;
        let mut tri_islands = vec![0u32; tri_count];

        for t in 0..tri_count {
            let root = ds.find(t as u32);
            let island_id = *tri_root_to_island.entry(root).or_insert_with(|| {
                let id = next_island_id;
                next_island_id += 1;
                id
            });
            tri_islands[t] = island_id;
        }

        // Rasterize triangles into UV pixel label buffer
        let total_pixels = (width * height) as usize;
        let mut labels = vec![0u32; total_pixels];

        let w_f = width as f32;
        let h_f = height as f32;

        for (tri_idx, chunk) in mesh.indices.chunks_exact(3).enumerate() {
            let island_id = tri_islands[tri_idx];
            let v0 = mesh.vertices[chunk[0] as usize].uv;
            let v1 = mesh.vertices[chunk[1] as usize].uv;
            let v2 = mesh.vertices[chunk[2] as usize].uv;

            let p0 = Vec2::new(v0.x * w_f, v0.y * h_f);
            let p1 = Vec2::new(v1.x * w_f, v1.y * h_f);
            let p2 = Vec2::new(v2.x * w_f, v2.y * h_f);

            let min_px = (p0.x.min(p1.x).min(p2.x).floor() as i32).clamp(0, width as i32 - 1) as u32;
            let max_px = (p0.x.max(p1.x).max(p2.x).ceil() as i32).clamp(0, width as i32 - 1) as u32;
            let min_py = (p0.y.min(p1.y).min(p2.y).floor() as i32).clamp(0, height as i32 - 1) as u32;
            let max_py = (p0.y.max(p1.y).max(p2.y).ceil() as i32).clamp(0, height as i32 - 1) as u32;

            // 2D barycentric triangle test
            let denom = (p1.y - p2.y) * (p0.x - p2.x) + (p2.x - p1.x) * (p0.y - p2.y);
            if denom.abs() < 1e-5 {
                continue;
            }
            let inv_denom = 1.0 / denom;

            for y in min_py..=max_py {
                let py = y as f32 + 0.5;
                for x in min_px..=max_px {
                    let px = x as f32 + 0.5;
                    let w0 = ((p1.y - p2.y) * (px - p2.x) + (p2.x - p1.x) * (py - p2.y)) * inv_denom;
                    let w1 = ((p2.y - p0.y) * (px - p2.x) + (p0.x - p2.x) * (py - p2.y)) * inv_denom;
                    let w2 = 1.0 - w0 - w1;

                    if w0 >= -0.001 && w1 >= -0.001 && w2 >= -0.001 {
                        labels[(y * width + x) as usize] = island_id;
                    }
                }
            }
        }

        // Calculate statistics for each UV island
        struct IslandStats {
            pixel_count: usize,
            min_x: u32,
            min_y: u32,
            max_x: u32,
            max_y: u32,
            sum_x: u64,
            sum_y: u64,
        }

        let mut island_stats: HashMap<u32, IslandStats> = HashMap::new();

        for y in 0..height {
            for x in 0..width {
                let id = labels[(y * width + x) as usize];
                if id > 0 {
                    let stat = island_stats.entry(id).or_insert(IslandStats {
                        pixel_count: 0,
                        min_x: x,
                        min_y: y,
                        max_x: x,
                        max_y: y,
                        sum_x: 0,
                        sum_y: 0,
                    });
                    stat.pixel_count += 1;
                    stat.min_x = stat.min_x.min(x);
                    stat.min_y = stat.min_y.min(y);
                    stat.max_x = stat.max_x.max(x);
                    stat.max_y = stat.max_y.max(y);
                    stat.sum_x += x as u64;
                    stat.sum_y += y as u64;
                }
            }
        }

        let mut max_area = 1usize;
        let mut islands = HashMap::new();

        for (id, stat) in island_stats {
            max_area = max_area.max(stat.pixel_count);
            let count_f = stat.pixel_count.max(1) as f32;
            let cx = (stat.sum_x as f32 / count_f + 0.5) / w_f;
            let cy = (stat.sum_y as f32 / count_f + 0.5) / h_f;

            let h1 = hash_u32(id.wrapping_add(seed.wrapping_mul(0x9e3779b9)));
            let h2 = hash_u32(h1.wrapping_add(1013));
            let h3 = hash_u32(h2.wrapping_add(3747));

            let random_val = (h1 as f32) / (u32::MAX as f32);
            let r_col = (h1 as f32) / (u32::MAX as f32);
            let g_col = (h2 as f32) / (u32::MAX as f32);
            let b_col = (h3 as f32) / (u32::MAX as f32);
            let random_angle = ((h2 as f32) / (u32::MAX as f32)) * std::f32::consts::TAU;

            islands.insert(
                id,
                FloodFillIsland {
                    id,
                    pixel_count: stat.pixel_count,
                    min_x: stat.min_x,
                    min_y: stat.min_y,
                    max_x: stat.max_x,
                    max_y: stat.max_y,
                    centroid_x: cx.clamp(0.0, 1.0),
                    centroid_y: cy.clamp(0.0, 1.0),
                    random_val,
                    random_color: [r_col, g_col, b_col],
                    random_angle,
                },
            );
        }

        FloodFillResult {
            width,
            height,
            labels,
            islands,
            max_area,
        }
    }
}

impl FloodFillResult {
    /// Render the flood-filled topology into a high-performance Rayon parallelized ScalarBuffer
    pub fn to_scalar_buffer(
        &self,
        mode: FloodFillMode,
        gradient_angle_deg: f32,
        random_angle: bool,
        background_val: f32,
    ) -> ScalarBuffer {
        let w = self.width;
        let h = self.height;
        let mut buf = ScalarBuffer::new(w, h, background_val);
        if w == 0 || h == 0 || self.islands.is_empty() {
            return buf;
        }

        let global_angle_rad = gradient_angle_deg.to_radians();
        let max_area_f = self.max_area.max(1) as f32;

        buf.data
            .par_chunks_mut(w as usize)
            .enumerate()
            .for_each(|(y, row)| {
                let y_u32 = y as u32;
                for (x, pixel) in row.iter_mut().enumerate() {
                    let x_u32 = x as u32;
                    let idx = y * (w as usize) + x;
                    let label = self.labels[idx];

                    if label == 0 {
                        *pixel = background_val;
                        continue;
                    }

                    if let Some(island) = self.islands.get(&label) {
                        match mode {
                            FloodFillMode::RandomGrayscale => {
                                *pixel = island.random_val;
                            }
                            FloodFillMode::CentroidDistance => {
                                let norm_x = (x_u32 as f32 + 0.5) / (w as f32);
                                let norm_y = (y_u32 as f32 + 0.5) / (h as f32);
                                let dx = norm_x - island.centroid_x;
                                let dy = norm_y - island.centroid_y;
                                let dist = (dx * dx + dy * dy).sqrt();

                                let extent_x = (island.max_x.saturating_sub(island.min_x) + 1) as f32 / (w as f32);
                                let extent_y = (island.max_y.saturating_sub(island.min_y) + 1) as f32 / (h as f32);
                                let radius = (extent_x.hypot(extent_y) * 0.5).max(1e-4);

                                let falloff = (1.0 - (dist / radius)).clamp(0.0, 1.0);
                                *pixel = (falloff * falloff * (3.0 - 2.0 * falloff)).clamp(0.0, 1.0);
                            }
                            FloodFillMode::CentroidX => {
                                *pixel = island.centroid_x;
                            }
                            FloodFillMode::CentroidY => {
                                *pixel = island.centroid_y;
                            }
                            FloodFillMode::BoundingBoxArea => {
                                let norm_area = (island.pixel_count as f32 / max_area_f).clamp(0.0, 1.0);
                                *pixel = norm_area;
                            }
                            FloodFillMode::AngleGradient => {
                                let angle = if random_angle {
                                    island.random_angle
                                } else {
                                    global_angle_rad
                                };

                                let bw = (island.max_x.saturating_sub(island.min_x) + 1).max(1) as f32;
                                let bh = (island.max_y.saturating_sub(island.min_y) + 1).max(1) as f32;

                                let u_loc = (x_u32.saturating_sub(island.min_x) as f32 + 0.5) / bw;
                                let v_loc = (y_u32.saturating_sub(island.min_y) as f32 + 0.5) / bh;

                                let centered_x = u_loc - 0.5;
                                let centered_y = v_loc - 0.5;

                                let (sin_a, cos_a) = angle.sin_cos();
                                let proj = centered_x * cos_a + centered_y * sin_a;
                                *pixel = (proj + 0.5).clamp(0.0, 1.0);
                            }
                        }
                    } else {
                        *pixel = background_val;
                    }
                }
            });

        buf
    }

    /// Render the flood-filled topology into a high-performance Rayon parallelized RgbaBuffer (Random Colors per island)
    pub fn to_rgba_buffer(&self, background_color: [f32; 4]) -> RgbaBuffer {
        let w = self.width;
        let h = self.height;
        let bg_vec = Vec4::new(background_color[0], background_color[1], background_color[2], background_color[3]);
        let mut buf = RgbaBuffer::new(w, h, bg_vec);
        if w == 0 || h == 0 || self.islands.is_empty() {
            return buf;
        }

        buf.data
            .par_chunks_mut(w as usize)
            .enumerate()
            .for_each(|(y, row)| {
                for (x, pixel) in row.iter_mut().enumerate() {
                    let idx = y * (w as usize) + x;
                    let label = self.labels[idx];
                    if label == 0 {
                        *pixel = bg_vec;
                    } else if let Some(island) = self.islands.get(&label) {
                        *pixel = Vec4::new(
                            island.random_color[0],
                            island.random_color[1],
                            island.random_color[2],
                            1.0,
                        );
                    } else {
                        *pixel = bg_vec;
                    }
                }
            });

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flood_fill_grid_segmentation() {
        let w = 32u32;
        let h = 32u32;
        let mut mask = ScalarBuffer::new(w, h, 0.0);

        // Create two separate rectangular islands
        // Island 1: [4..12, 4..12]
        for y in 4..12 {
            for x in 4..12 {
                mask.set_pixel(x, y, 1.0);
            }
        }
        // Island 2: [20..28, 20..28]
        for y in 20..28 {
            for x in 20..28 {
                mask.set_pixel(x, y, 1.0);
            }
        }

        let res = FloodFillEngine::flood_fill_from_mask(&mask, 0.5, false, 1, 42);
        assert_eq!(res.islands.len(), 2, "Should identify exactly 2 connected islands");

        // Verify random grayscale output
        let gray = res.to_scalar_buffer(FloodFillMode::RandomGrayscale, 0.0, false, 0.0);
        let val1 = gray.get_pixel(6, 6);
        let val2 = gray.get_pixel(24, 24);
        assert!(val1 > 0.0 && val1 <= 1.0);
        assert!(val2 > 0.0 && val2 <= 1.0);
        assert_ne!(val1, val2, "Separate islands should receive distinct random values");

        // Background should remain 0.0
        assert_eq!(gray.get_pixel(0, 0), 0.0);
    }

    #[test]
    fn test_flood_fill_modes_evaluation() {
        let w = 16u32;
        let h = 16u32;
        let mut mask = ScalarBuffer::new(w, h, 0.0);
        for y in 2..14 {
            for x in 2..14 {
                mask.set_pixel(x, y, 1.0);
            }
        }

        let res = FloodFillEngine::flood_fill_from_mask(&mask, 0.5, false, 1, 101);
        assert_eq!(res.islands.len(), 1);

        for mode in FloodFillMode::all() {
            let buf = res.to_scalar_buffer(*mode, 45.0, false, 0.0);
            assert_eq!(buf.width, w);
            assert_eq!(buf.height, h);
            let center_val = buf.get_pixel(8, 8);
            assert!(center_val >= 0.0 && center_val <= 1.0);
        }

        // Color buffer
        let rgba = res.to_rgba_buffer([0.0, 0.0, 0.0, 1.0]);
        let col = rgba.get_pixel(8, 8);
        assert_eq!(col.w, 1.0);
    }

    #[test]
    fn test_flood_fill_from_mesh_uvs() {
        let cube = Mesh::create_cube_mesh("TestCube", 1.0);
        let res = FloodFillEngine::flood_fill_from_mesh_uvs(&cube, 64, 64, 555);
        assert!(!res.islands.is_empty(), "Mesh cube should produce UV islands");
        let gray = res.to_scalar_buffer(FloodFillMode::RandomGrayscale, 0.0, false, 0.0);
        assert_eq!(gray.width, 64);
        assert_eq!(gray.height, 64);
    }
}
