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

#[derive(Serialize, Deserialize)]
pub struct SerializedProject {
    pub name: String,
    pub mesh: Mesh,
    pub texture_width: u32,
    pub texture_height: u32,
    pub layers: Vec<Layer>,
    pub active_layer_idx: usize,
    pub next_layer_id: u64,
}

pub struct Project {
    pub name: String,
    pub mesh: Mesh,
    pub texture_width: u32,
    pub texture_height: u32,
    pub layers: Vec<Layer>,
    pub active_layer_idx: usize,
    pub composite_textures: CompositeTextureSet,

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

        // Bake initial mesh maps
        // proj.bake_mesh_maps();

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
                Baker::denoise_scalar_buffer(&mut curv, self.world_normal_map.as_ref(), denoise_iterations.min(2), 2, 1.5, 8.0);
            }
            self.curvature_map = Some(curv);
        }
        if bake_ao {
            let mut ao = Baker::bake_ao(&self.mesh, w, h);
            if denoise && denoise_iterations > 0 {
                Baker::denoise_scalar_buffer(&mut ao, self.world_normal_map.as_ref(), denoise_iterations, 3, 2.0, 16.0);
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
                Baker::denoise_scalar_buffer(&mut th, self.world_normal_map.as_ref(), denoise_iterations, 3, 2.0, 16.0);
            }
            self.thickness_map = Some(th);
        }

        self.gpu_mesh_maps_sync_needed = true;
    }

    pub fn set_mesh(&mut self, mesh: Mesh) {
        self.mesh = mesh;
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
        let mut layer = Layer::new_fill_layer(
            id,
            layer_name,
            self.texture_width,
            self.texture_height,
            color,
            roughness,
            metallic,
        );
        layer.evaluate_fill(self.curvature_map.as_ref());
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

    pub fn delete_layer(&mut self, idx: usize) {
        if idx < self.layers.len() && self.layers.len() > 1 {
            self.layers.remove(idx);
            if self.active_layer_idx >= self.layers.len() {
                self.active_layer_idx = self.layers.len().saturating_sub(1);
            }
            self.rebuild_composite();
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

    /// Save full project document to a `.paintcraft` file
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let serialized = SerializedProject {
            name: self.name.clone(),
            mesh: self.mesh.clone(),
            texture_width: self.texture_width,
            texture_height: self.texture_height,
            layers: self.layers.clone(),
            active_layer_idx: self.active_layer_idx,
            next_layer_id: self.next_layer_id,
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

        let mut proj = Self {
            name: serialized.name,
            mesh: serialized.mesh,
            texture_width: serialized.texture_width,
            texture_height: serialized.texture_height,
            layers: serialized.layers,
            active_layer_idx: serialized.active_layer_idx,
            composite_textures: CompositeTextureSet::new(serialized.texture_width, serialized.texture_height),
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
