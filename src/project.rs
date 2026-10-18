// 3D Paintcraft - Document, Project Model & State Management
// Pure safe Rust

use crate::baker::Baker;
use crate::channel::{RgbaBuffer, ScalarBuffer};
use crate::layer::{CompositeTextureSet, Layer};
use crate::mesh::Mesh;
use crate::smart_materials::SmartMaterialLibrary;
use glam::Vec4;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextureSet {
    pub id: u64,
    pub name: String,
    pub material_name: String,
    pub submesh_indices: Vec<usize>,
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub active_layer_idx: usize,
    #[serde(skip)]
    pub composite_textures: CompositeTextureSet,
    #[serde(default = "default_ts_visible")]
    pub visible: bool,
}

fn default_ts_visible() -> bool {
    true
}

impl TextureSet {
    pub fn new(id: u64, name: String, material_name: String, submesh_indices: Vec<usize>, width: u32, height: u32) -> Self {
        let initial_layer = Layer::new_paint_layer(1, "Base Paint".to_string(), width, height);
        let mut composite_textures = CompositeTextureSet::new(width, height);
        composite_textures.composite_layers(&[initial_layer.clone()]);
        Self {
            id,
            name,
            material_name,
            submesh_indices,
            width,
            height,
            layers: vec![initial_layer],
            active_layer_idx: 0,
            composite_textures,
            visible: true,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerializedProject {
    pub name: String,
    pub mesh: Mesh,
    pub texture_width: u32,
    pub texture_height: u32,
    pub layers: Vec<Layer>,
    pub active_layer_idx: usize,
    pub next_layer_id: u64,
    #[serde(default)]
    pub texture_sets: Vec<TextureSet>,
    #[serde(default)]
    pub active_texture_set_idx: usize,
}

pub struct Project {
    pub name: String,
    pub mesh: Mesh,
    pub texture_width: u32,
    pub texture_height: u32,
    pub layers: Vec<Layer>,
    pub active_layer_idx: usize,
    pub composite_textures: CompositeTextureSet,
    pub texture_sets: Vec<TextureSet>,
    pub active_texture_set_idx: usize,
    pub auto_switch_texture_set: bool,

    // Baked maps
    pub curvature_map: Option<ScalarBuffer>,
    pub ao_map: Option<ScalarBuffer>,
    pub world_normal_map: Option<RgbaBuffer>, // Tangent normal map
    pub world_space_normal_map: Option<RgbaBuffer>,
    pub position_map: Option<RgbaBuffer>, // World Position Object (WPO)
    pub thickness_map: Option<ScalarBuffer>,

    pub next_layer_id: u64,
    pub is_dirty: bool,
    pub gpu_sync_needed: bool,
    pub gpu_mesh_maps_sync_needed: bool,
    pub high_poly_mesh: Option<Mesh>,
    #[allow(dead_code)]
    pub file_path: Option<String>,
}

impl Project {
    pub fn new_default(width: u32, height: u32) -> Self {
        let mesh = Mesh::create_shader_ball();
        let mut proj = Self {
            name: "Untitled Project".to_string(),
            mesh,
            texture_width: width,
            texture_height: height,
            layers: Vec::new(),
            active_layer_idx: 0,
            composite_textures: CompositeTextureSet::new(width, height),
            texture_sets: Vec::new(),
            active_texture_set_idx: 0,
            auto_switch_texture_set: true,
            curvature_map: None,
            ao_map: None,
            world_normal_map: None,
            world_space_normal_map: None,
            position_map: None,
            thickness_map: None,
            next_layer_id: 1,
            is_dirty: true,
            gpu_sync_needed: true,
            gpu_mesh_maps_sync_needed: true,
            file_path: None, high_poly_mesh: None,
        };

        // Default to a single base fill layer
        let base_fill = Layer::new_fill_layer(
            proj.next_layer_id, 
            "Base Material".to_string(), 
            width, height, 
            glam::Vec4::new(0.8, 0.8, 0.8, 1.0), 
            0.5, 0.0
        );
        proj.next_layer_id += 1;
        proj.layers.push(base_fill);
        proj.rebuild_composite();

        let default_ts = TextureSet {
            id: 1,
            name: "Material".to_string(),
            material_name: "Material".to_string(),
            submesh_indices: vec![0],
            width,
            height,
            layers: proj.layers.clone(),
            active_layer_idx: 0,
            composite_textures: proj.composite_textures.clone(),
            visible: true,
        };
        proj.texture_sets = vec![default_ts];
        proj
    }

    pub fn bake_mesh_maps(&mut self, res: u32, frontal_dist: f32, rear_dist: f32) {
        self.bake_mesh_maps_advanced(res, frontal_dist, rear_dist, true, true, true, true, true, true);
    }

    pub fn bake_mesh_maps_advanced(
        &mut self,
        res: u32,
        frontal_dist: f32,
        rear_dist: f32,
        bake_tangent_normal: bool,
        bake_ws_normal: bool,
        bake_ao: bool,
        bake_curvature: bool,
        bake_position: bool,
        bake_thickness: bool,
    ) {
        self.bake_mesh_maps_advanced_denoised(
            res, frontal_dist, rear_dist,
            bake_tangent_normal, bake_ws_normal, bake_ao, bake_curvature, bake_position, bake_thickness,
            true, 2,
        );
    }

    pub fn bake_mesh_maps_advanced_denoised(
        &mut self,
        res: u32,
        frontal_dist: f32,
        rear_dist: f32,
        bake_tangent_normal: bool,
        bake_ws_normal: bool,
        bake_ao: bool,
        bake_curvature: bool,
        bake_position: bool,
        bake_thickness: bool,
        denoise: bool,
        denoise_iterations: usize,
    ) {
        let w = res;
        let h = res;

        if bake_tangent_normal {
            let wn = Baker::bake_tangent_normals(&self.mesh, self.high_poly_mesh.as_ref(), frontal_dist, rear_dist, w, h);
            self.world_normal_map = Some(wn);
        }
        if bake_ws_normal {
            let wsn = Baker::bake_world_space_normal(&self.mesh, w, h);
            self.world_space_normal_map = Some(wsn);
        }
        if bake_curvature {
            let mut curv = Baker::bake_curvature(&self.mesh, w, h);
            if denoise && denoise_iterations > 0 {
                Baker::denoise_scalar_buffer(&mut curv, self.world_normal_map.as_ref(), 1, 1, 0.8, 24.0);
            }
            self.curvature_map = Some(curv);
        }
        if bake_ao {
            let mut ao = Baker::bake_ao(&self.mesh, w, h);
            if denoise && denoise_iterations > 0 {
                Baker::denoise_scalar_buffer(&mut ao, self.world_normal_map.as_ref(), denoise_iterations.min(2), 2, 1.2, 24.0);
            }
            self.ao_map = Some(ao);
        }
        if bake_position {
            let pos = Baker::bake_position(&self.mesh, w, h);
            self.position_map = Some(pos);
        }
        if bake_thickness {
            let mut th = Baker::bake_thickness(&self.mesh, w, h);
            if denoise && denoise_iterations > 0 {
                Baker::denoise_scalar_buffer(&mut th, self.world_normal_map.as_ref(), denoise_iterations.min(2), 2, 1.2, 24.0);
            }
            self.thickness_map = Some(th);
        }

        self.gpu_mesh_maps_sync_needed = true;
    }

    pub fn rebuild_texture_sets_from_mesh(&mut self) {
        let mut unique_materials: Vec<(String, Vec<usize>)> = Vec::new();
        for (sm_idx, sm) in self.mesh.submeshes.iter().enumerate() {
            if let Some(pos) = unique_materials.iter().position(|(m, _)| *m == sm.material_name) {
                unique_materials[pos].1.push(sm_idx);
            } else {
                unique_materials.push((sm.material_name.clone(), vec![sm_idx]));
            }
        }

        if unique_materials.is_empty() {
            unique_materials.push(("Material".to_string(), vec![0]));
        }

        let mut sets = Vec::with_capacity(unique_materials.len());
        for (i, (mat_name, submesh_indices)) in unique_materials.into_iter().enumerate() {
            let id = (i + 1) as u64;
            let set_name = mat_name.clone();
            let ts = TextureSet::new(id, set_name, mat_name, submesh_indices, self.texture_width, self.texture_height);
            sets.push(ts);
        }

        self.texture_sets = sets;
        self.active_texture_set_idx = 0;
        if let Some(first) = self.texture_sets.first() {
            self.layers = first.layers.clone();
            self.active_layer_idx = first.active_layer_idx;
            self.composite_textures = first.composite_textures.clone();
        }
    }

    pub fn set_mesh(&mut self, mesh: Mesh) {
        self.mesh = mesh;
        self.rebuild_texture_sets_from_mesh();
        self.bake_mesh_maps(self.texture_width, 0.1, 0.1);
        for layer in &mut self.layers {
            layer.evaluate_fill(self.curvature_map.as_ref());
        }
        self.gpu_mesh_maps_sync_needed = true;
        self.rebuild_composite();
    }

    pub fn add_paint_layer(&mut self, name: Option<String>) -> usize {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        let layer_name = name.unwrap_or_else(|| format!("Paint Layer {}", id));
        let layer = Layer::new_paint_layer(id, layer_name, self.texture_width, self.texture_height);
        self.layers.push(layer);
        self.active_layer_idx = self.layers.len() - 1;
        self.rebuild_composite();
        self.active_layer_idx
    }

    pub fn add_fill_layer(
        &mut self,
        name: Option<String>,
        color: Vec4,
        roughness: f32,
        metallic: f32,
    ) -> usize {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        let layer_name = name.unwrap_or_else(|| format!("Fill Layer {}", id));
        let layer = Layer::new_fill_layer(
            id,
            layer_name,
            self.texture_width,
            self.texture_height,
            color,
            roughness,
            metallic,
        );
        self.layers.push(layer);
        self.active_layer_idx = self.layers.len() - 1;
        self.rebuild_composite();
        self.active_layer_idx
    }

    pub fn add_folder_layer(&mut self, name: Option<String>) -> usize {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        let layer_name = name.unwrap_or_else(|| format!("Folder {}", id));
        let layer = Layer::new_folder_layer(id, layer_name, self.texture_width, self.texture_height);
        self.layers.push(layer);
        self.active_layer_idx = self.layers.len() - 1;
        self.rebuild_composite();
        self.active_layer_idx
    }

    pub fn apply_smart_material(&mut self, preset_id: &str) {
        let new_layers = SmartMaterialLibrary::build_layers_with_maps(
            preset_id,
            self.texture_width,
            self.texture_height,
            self.curvature_map.as_ref(),
            self.ao_map.as_ref(),
            self.world_normal_map.as_ref(),
            self.position_map.as_ref(),
        );

        self.layers = new_layers;
        self.next_layer_id = self.layers.len() as u64 + 1;
        self.active_layer_idx = self.layers.len().saturating_sub(1);
        self.rebuild_composite();
    }

    pub fn delete_layer(&mut self, idx: usize) -> Option<Layer> {
        if idx < self.layers.len() && self.layers.len() > 1 {
            let removed = self.layers.remove(idx);
            if self.active_layer_idx >= self.layers.len() {
                self.active_layer_idx = self.layers.len().saturating_sub(1);
            }
            self.rebuild_composite();
            Some(removed)
        } else {
            None
        }
    }

    pub fn move_layer_up(&mut self, idx: usize) {
        if idx + 1 < self.layers.len() {
            self.layers.swap(idx, idx + 1);
            self.active_layer_idx = idx + 1;
            self.rebuild_composite();
        }
    }

    pub fn move_layer_down(&mut self, idx: usize) {
        if idx > 0 && idx < self.layers.len() {
            self.layers.swap(idx, idx - 1);
            self.active_layer_idx = idx - 1;
            self.rebuild_composite();
        }
    }

    /// Reorder a layer by dragging it from index `from` to `to`
    pub fn reorder_layer(&mut self, from: usize, to: usize) {
        if from < self.layers.len() && to < self.layers.len() && from != to {
            let layer = self.layers.remove(from);
            self.layers.insert(to, layer);
            self.active_layer_idx = to;
            self.rebuild_composite();
        }
    }

    /// Resize all project layers and baked maps to a new resolution
    pub fn resize_textures(&mut self, new_width: u32, new_height: u32) {
        if self.texture_width == new_width && self.texture_height == new_height {
            return;
        }
        self.texture_width = new_width;
        self.texture_height = new_height;
        for layer in &mut self.layers {
            layer.resize(new_width, new_height);
        }
        self.composite_textures = CompositeTextureSet::new(new_width, new_height);
        if let Some(ts) = self.texture_sets.get_mut(self.active_texture_set_idx) {
            ts.width = new_width;
            ts.height = new_height;
            ts.layers = self.layers.clone();
            ts.composite_textures = self.composite_textures.clone();
        }
        if let Some(ref mut curv) = self.curvature_map {
            curv.resize_bilinear(new_width, new_height);
        }
        if let Some(ref mut ao) = self.ao_map {
            ao.resize_bilinear(new_width, new_height);
        }
        if let Some(ref mut wn) = self.world_normal_map {
            wn.resize_bilinear(new_width, new_height);
        }
        if let Some(ref mut wsn) = self.world_space_normal_map {
            wsn.resize_bilinear(new_width, new_height);
        }
        if let Some(ref mut pos) = self.position_map {
            pos.resize_bilinear(new_width, new_height);
        }
        if let Some(ref mut th) = self.thickness_map {
            th.resize_bilinear(new_width, new_height);
        }
        self.gpu_mesh_maps_sync_needed = true;
        self.rebuild_composite();
    }

    pub fn duplicate_layer(&mut self, idx: usize) -> Option<usize> {
        if idx < self.layers.len() {
            let mut new_layer = self.layers[idx].clone();
            new_layer.id = self.next_layer_id;
            self.next_layer_id += 1;
            new_layer.name = format!("{} (Copy)", new_layer.name);
            let insert_idx = idx + 1;
            self.layers.insert(insert_idx, new_layer);
            self.active_layer_idx = insert_idx;
            self.rebuild_composite();
            Some(insert_idx)
        } else {
            None
        }
    }

    pub fn rebuild_composite(&mut self) {
        self.composite_textures.composite_layers(&self.layers);
        self.is_dirty = true;
        self.gpu_sync_needed = true;
    }

    pub fn active_layer_mut(&mut self) -> Option<&mut Layer> {
        self.layers.get_mut(self.active_layer_idx)
    }

    pub fn active_layer(&self) -> Option<&Layer> {
        self.layers.get(self.active_layer_idx)
    }

    pub fn sync_active_texture_set(&mut self) {
        if let Some(ts) = self.texture_sets.get_mut(self.active_texture_set_idx) {
            ts.layers = self.layers.clone();
            ts.active_layer_idx = self.active_layer_idx;
            ts.width = self.texture_width;
            ts.height = self.texture_height;
            ts.composite_textures = self.composite_textures.clone();
        }
    }

    pub fn switch_texture_set(&mut self, new_idx: usize) {
        if new_idx >= self.texture_sets.len() || new_idx == self.active_texture_set_idx {
            return;
        }
        // 1. Sync current state into active set
        self.sync_active_texture_set();

        // 2. Switch active index
        self.active_texture_set_idx = new_idx;

        // 3. Load newly activated texture set
        let target = &self.texture_sets[new_idx];
        self.texture_width = target.width;
        self.texture_height = target.height;
        self.layers = target.layers.clone();
        self.active_layer_idx = target.active_layer_idx;
        self.composite_textures = target.composite_textures.clone();
        self.gpu_sync_needed = true;
        self.is_dirty = true;
        self.rebuild_composite();
    }

    #[allow(dead_code)]
    pub fn active_texture_set(&self) -> Option<&TextureSet> {
        self.texture_sets.get(self.active_texture_set_idx)
    }

    #[allow(dead_code)]
    pub fn active_texture_set_mut(&mut self) -> Option<&mut TextureSet> {
        self.texture_sets.get_mut(self.active_texture_set_idx)
    }

    pub fn texture_set_index_for_triangle(&self, tri_idx: usize) -> Option<usize> {
        let sm_idx = self.mesh.submesh_index_for_triangle(tri_idx)?;
        self.texture_sets.iter().position(|ts| ts.submesh_indices.contains(&sm_idx))
    }

    pub fn toggle_texture_set_visibility(&mut self, idx: usize) {
        if let Some(ts) = self.texture_sets.get_mut(idx) {
            ts.visible = !ts.visible;
            let vis = ts.visible;
            for &sm_idx in &ts.submesh_indices {
                if let Some(sm) = self.mesh.submeshes.get_mut(sm_idx) {
                    sm.visible = vis;
                }
            }
        }
    }

    pub fn solo_texture_set(&mut self, idx: usize) {
        let is_already_solo = self.texture_sets.iter().enumerate().all(|(i, ts)| {
            if i == idx { ts.visible } else { !ts.visible }
        });

        if is_already_solo {
            // Unsolo: make all visible
            for ts in &mut self.texture_sets {
                ts.visible = true;
                for &sm_idx in &ts.submesh_indices {
                    if let Some(sm) = self.mesh.submeshes.get_mut(sm_idx) {
                        sm.visible = true;
                    }
                }
            }
        } else {
            // Solo: make only idx visible
            for (i, ts) in self.texture_sets.iter_mut().enumerate() {
                ts.visible = i == idx;
                let vis = ts.visible;
                for &sm_idx in &ts.submesh_indices {
                    if let Some(sm) = self.mesh.submeshes.get_mut(sm_idx) {
                        sm.visible = vis;
                    }
                }
            }
        }
    }

    pub fn add_texture_set(&mut self, name: String, material_name: String, submesh_indices: Vec<usize>) -> usize {
        self.sync_active_texture_set();
        let next_id = self.texture_sets.iter().map(|ts| ts.id).max().unwrap_or(0) + 1;
        let new_set = TextureSet::new(next_id, name, material_name, submesh_indices, self.texture_width, self.texture_height);
        self.texture_sets.push(new_set);
        let new_idx = self.texture_sets.len() - 1;
        self.switch_texture_set(new_idx);
        new_idx
    }

    pub fn duplicate_texture_set(&mut self, idx: usize) -> usize {
        self.sync_active_texture_set();
        if let Some(src) = self.texture_sets.get(idx).cloned() {
            let next_id = self.texture_sets.iter().map(|ts| ts.id).max().unwrap_or(0) + 1;
            let mut dup = src;
            dup.id = next_id;
            dup.name = format!("{} Copy", dup.name);
            self.texture_sets.push(dup);
            let new_idx = self.texture_sets.len() - 1;
            self.switch_texture_set(new_idx);
            new_idx
        } else {
            self.active_texture_set_idx
        }
    }

    pub fn delete_texture_set(&mut self, idx: usize) -> bool {
        if self.texture_sets.len() <= 1 || idx >= self.texture_sets.len() {
            return false;
        }
        self.sync_active_texture_set();
        self.texture_sets.remove(idx);
        let new_idx = if self.active_texture_set_idx >= self.texture_sets.len() {
            self.texture_sets.len() - 1
        } else if self.active_texture_set_idx > idx {
            self.active_texture_set_idx - 1
        } else {
            self.active_texture_set_idx
        };
        self.active_texture_set_idx = new_idx;
        let target = &self.texture_sets[new_idx];
        self.texture_width = target.width;
        self.texture_height = target.height;
        self.layers = target.layers.clone();
        self.active_layer_idx = target.active_layer_idx;
        self.composite_textures = target.composite_textures.clone();
        self.gpu_sync_needed = true;
        self.is_dirty = true;
        self.rebuild_composite();
        true
    }

    /// Save full project document to a `.paintcraft` file
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let mut texture_sets = self.texture_sets.clone();
        if let Some(ts) = texture_sets.get_mut(self.active_texture_set_idx) {
            ts.layers = self.layers.clone();
            ts.active_layer_idx = self.active_layer_idx;
            ts.width = self.texture_width;
            ts.height = self.texture_height;
        }

        let serialized = SerializedProject {
            name: self.name.clone(),
            mesh: self.mesh.clone(),
            texture_width: self.texture_width,
            texture_height: self.texture_height,
            layers: self.layers.clone(),
            active_layer_idx: self.active_layer_idx,
            next_layer_id: self.next_layer_id,
            texture_sets,
            active_texture_set_idx: self.active_texture_set_idx,
        };

        let file = File::create(path).map_err(|e| format!("Failed to create project file: {}", e))?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, &serialized)
            .map_err(|e| format!("Failed to serialize project: {}", e))?;

        Ok(())
    }

    /// Load full project document from a `.paintcraft` file
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("Failed to open project file: {}", e))?;
        let reader = BufReader::new(file);
        let serialized: SerializedProject = serde_json::from_reader(reader)
            .map_err(|e| format!("Failed to parse project file: {}", e))?;

        let texture_sets = if serialized.texture_sets.is_empty() {
            vec![TextureSet {
                id: 1,
                name: "Material".to_string(),
                material_name: "Material".to_string(),
                submesh_indices: vec![0],
                width: serialized.texture_width,
                height: serialized.texture_height,
                layers: serialized.layers.clone(),
                active_layer_idx: serialized.active_layer_idx,
                composite_textures: CompositeTextureSet::new(serialized.texture_width, serialized.texture_height),
                visible: true,
            }]
        } else {
            serialized.texture_sets
        };
        let active_texture_set_idx = serialized.active_texture_set_idx.min(texture_sets.len().saturating_sub(1));

        let mut proj = Self {
            name: serialized.name,
            mesh: serialized.mesh,
            texture_width: serialized.texture_width,
            texture_height: serialized.texture_height,
            layers: serialized.layers,
            active_layer_idx: serialized.active_layer_idx,
            composite_textures: CompositeTextureSet::new(serialized.texture_width, serialized.texture_height),
            texture_sets,
            active_texture_set_idx,
            auto_switch_texture_set: true,
            curvature_map: None,
            ao_map: None,
            world_normal_map: None,
            world_space_normal_map: None,
            position_map: None,
            thickness_map: None,
            next_layer_id: serialized.next_layer_id,
            is_dirty: false,
            gpu_sync_needed: true,
            gpu_mesh_maps_sync_needed: true,
            file_path: Some(path.to_string_lossy().to_string()), high_poly_mesh: None,
        };

        proj.mesh.rebuild_bvh();
        // proj.bake_mesh_maps();
        proj.rebuild_composite();

        Ok(proj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_set_management_and_isolation() {
        let mut proj = Project::new_default(64, 64);
        assert_eq!(proj.texture_sets.len(), 1);

        // Add a second texture set
        let set2_idx = proj.add_texture_set("Glass".to_string(), "GlassMat".to_string(), vec![1]);
        assert_eq!(set2_idx, 1);
        assert_eq!(proj.texture_sets.len(), 2);
        assert_eq!(proj.active_texture_set_idx, 1);

        // Add a paint layer to the second texture set
        proj.add_paint_layer(Some("Glass Decal".to_string()));
        assert_eq!(proj.layers.len(), 2);

        // Paint a pixel in active layer of set 2
        if let Some(active_layer) = proj.active_layer_mut() {
            active_layer.base_color_buf.set_pixel(5, 5, glam::Vec4::new(0.0, 1.0, 0.0, 1.0));
        }

        // Switch back to texture set 0
        proj.switch_texture_set(0);
        assert_eq!(proj.active_texture_set_idx, 0);
        // Set 0 only has its base fill layer
        assert_eq!(proj.layers.len(), 1);

        // Switch back to texture set 1
        proj.switch_texture_set(1);
        assert_eq!(proj.active_texture_set_idx, 1);
        assert_eq!(proj.layers.len(), 2);
        assert_eq!(proj.layers[1].base_color_buf.get_pixel(5, 5), glam::Vec4::new(0.0, 1.0, 0.0, 1.0));

        // Test duplicate texture set
        let dup_idx = proj.duplicate_texture_set(1);
        assert_eq!(dup_idx, 2);
        assert_eq!(proj.texture_sets.len(), 3);
        assert_eq!(proj.texture_sets[2].name, "Glass Copy");

        // Test delete texture set
        assert!(proj.delete_texture_set(2));
        assert_eq!(proj.texture_sets.len(), 2);
    }

    #[test]
    fn test_texture_set_visibility_and_solo() {
        let mut proj = Project::new_default(64, 64);
        proj.add_texture_set("SecondSet".to_string(), "SecondMat".to_string(), vec![1]);
        assert_eq!(proj.texture_sets.len(), 2);

        // Initially both are visible
        assert!(proj.texture_sets[0].visible);
        assert!(proj.texture_sets[1].visible);

        // Toggle visibility of set 0
        proj.toggle_texture_set_visibility(0);
        assert!(!proj.texture_sets[0].visible);
        assert!(proj.texture_sets[1].visible);

        // Solo set 0
        proj.solo_texture_set(0);
        assert!(proj.texture_sets[0].visible);
        assert!(!proj.texture_sets[1].visible);

        // Un-solo set 0
        proj.solo_texture_set(0);
        assert!(proj.texture_sets[0].visible);
        assert!(proj.texture_sets[1].visible);
    }

    #[test]
    fn test_texture_set_serialization_roundtrip() {
        let mut proj = Project::new_default(64, 64);
        proj.add_texture_set("Armor".to_string(), "ArmorMat".to_string(), vec![1]);
        proj.sync_active_texture_set();

        let serialized = SerializedProject {
            name: proj.name.clone(),
            mesh: proj.mesh.clone(),
            texture_width: proj.texture_width,
            texture_height: proj.texture_height,
            layers: proj.layers.clone(),
            active_layer_idx: proj.active_layer_idx,
            next_layer_id: proj.next_layer_id,
            texture_sets: proj.texture_sets.clone(),
            active_texture_set_idx: proj.active_texture_set_idx,
        };

        let json_str = serde_json::to_string(&serialized).expect("Serialization failed");
        let deserialized: SerializedProject = serde_json::from_str(&json_str).expect("Deserialization failed");

        assert_eq!(deserialized.texture_sets.len(), 2);
        assert_eq!(deserialized.texture_sets[1].name, "Armor");
        assert_eq!(deserialized.active_texture_set_idx, 1);
    }
}

