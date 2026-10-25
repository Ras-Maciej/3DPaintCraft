// 3D Paintcraft - Industry Undo / Redo History Stack
// Pure safe Rust

use crate::brush::PaintTarget;
use crate::channel::ChannelFlags;
use crate::layer::Layer;
use crate::project::Project;

pub const HISTORY_TILE_SIZE: usize = 128;

/// Sparse tile-delta snapshot capturing only modified channels for a 128x128 sub-rect
#[derive(Clone, Debug)]
pub struct LayerTileDelta {
    pub tile_x: u16,
    pub tile_y: u16,
    pub width: u16,
    pub height: u16,
    pub base_color: Option<Vec<glam::Vec4>>,
    pub normal: Option<Vec<glam::Vec4>>,
    pub emissive: Option<Vec<glam::Vec4>>,
    pub roughness: Option<Vec<f32>>,
    pub metallic: Option<Vec<f32>>,
    pub height_buf: Option<Vec<f32>>,
    pub opacity: Option<Vec<f32>>,
    pub clearcoat: Option<Vec<f32>>,
    pub anisotropy: Option<Vec<f32>>,
    pub anisotropy_angle: Option<Vec<f32>>,
    pub mask: Option<Vec<f32>>,
    pub mask_paint_buf: Option<Vec<f32>>,
    pub mask_paint_alpha: Option<Vec<f32>>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum HistoryAction {
    TileStroke {
        layer_idx: usize,
        undo_tiles: Vec<LayerTileDelta>,
        redo_tiles: Vec<LayerTileDelta>,
        target: PaintTarget,
        channels: ChannelFlags,
        prev_has_mask: bool,
        next_has_mask: bool,
        description: String,
    },
    Stroke {
        layer_idx: usize,
        prev_layer: Box<Layer>,
        next_layer: Box<Layer>,
        description: String,
    },
    LayerStackChange {
        prev_layers: Vec<Layer>,
        next_layers: Vec<Layer>,
        prev_active: usize,
        next_active: usize,
        description: String,
    },
    LayerPropertyChange {
        layer_idx: usize,
        prev_layer: Box<Layer>,
        next_layer: Box<Layer>,
        description: String,
    },
    AddLayer {
        layer_idx: usize,
        layer: Box<Layer>,
        prev_active: usize,
    },
    DeleteLayer {
        layer_idx: usize,
        layer: Box<Layer>,
        prev_active: usize,
    },
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub name: String,
    pub action: HistoryAction,
}

pub struct HistoryManager {
    pub undo_stack: Vec<HistoryEntry>,
    pub redo_stack: Vec<HistoryEntry>,
    pub max_undo: usize,
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self {
            undo_stack: Vec::with_capacity(64),
            redo_stack: Vec::with_capacity(64),
            max_undo: 50,
        }
    }
}

impl HistoryManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn push(&mut self, name: impl Into<String>, action: HistoryAction) {
        if self.undo_stack.len() >= self.max_undo {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(HistoryEntry {
            name: name.into(),
            action,
        });
        self.redo_stack.clear();
    }

    pub fn adjust_max_undo_for_resolution(&mut self, width: u32, height: u32) {
        let max_dim = width.max(height);
        self.max_undo = if max_dim >= 4096 {
            30
        } else if max_dim >= 2048 {
            50
        } else {
            100
        };
        while self.undo_stack.len() > self.max_undo {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self, project: &mut Project) -> Option<String> {
        let entry = self.undo_stack.pop()?;
        let desc = entry.name.clone();

        match &entry.action {
            HistoryAction::TileStroke {
                layer_idx,
                undo_tiles,
                prev_has_mask,
                ..
            } => {
                if let Some(target) = project.layers.get_mut(*layer_idx) {
                    let mut min_x = u32::MAX;
                    let mut min_y = u32::MAX;
                    let mut max_x = 0;
                    let mut max_y = 0;

                    for tile in undo_tiles {
                        target.apply_tile(tile);
                        let x0 = (tile.tile_x as u32) * (HISTORY_TILE_SIZE as u32);
                        let y0 = (tile.tile_y as u32) * (HISTORY_TILE_SIZE as u32);
                        let x1 = x0 + tile.width as u32;
                        let y1 = y0 + tile.height as u32;
                        min_x = min_x.min(x0);
                        min_y = min_y.min(y0);
                        max_x = max_x.max(x1.saturating_sub(1));
                        max_y = max_y.max(y1.saturating_sub(1));
                    }

                    if target.has_mask != *prev_has_mask {
                        target.has_mask = *prev_has_mask;
                    }

                    if min_x <= max_x && min_y <= max_y {
                        target.dirty_rect = Some((min_x, min_y, max_x, max_y));
                        target.dirty_flags = crate::layer::LayerDirtyFlags::all();
                    } else {
                        target.mark_dirty_full();
                    }
                    project.rebuild_composite();
                }
            }
            HistoryAction::Stroke { layer_idx, prev_layer, .. } => {
                if let Some(target) = project.layers.get_mut(*layer_idx) {
                    target.restore_stroke_snapshot(prev_layer);
                    target.mark_dirty_full();
                    project.rebuild_composite();
                }
            }
            HistoryAction::LayerStackChange { prev_layers, prev_active, .. } => {
                project.layers = prev_layers.clone();
                for l in &mut project.layers {
                    l.mark_dirty_full();
                }
                project.active_layer_idx = *prev_active;
                project.rebuild_composite();
            }
            HistoryAction::LayerPropertyChange { layer_idx, prev_layer, .. } => {
                if let Some(target) = project.layers.get_mut(*layer_idx) {
                    *target = (**prev_layer).clone();
                    target.mark_dirty_full();
                    project.rebuild_composite();
                }
            }
            HistoryAction::AddLayer { layer_idx, prev_active, .. } => {
                if *layer_idx < project.layers.len() {
                    project.layers.remove(*layer_idx);
                }
                project.active_layer_idx = (*prev_active).min(project.layers.len().saturating_sub(1));
                project.rebuild_composite();
            }
            HistoryAction::DeleteLayer { layer_idx, layer, .. } => {
                let mut target_layer = (**layer).clone();
                target_layer.mark_dirty_full();
                if *layer_idx <= project.layers.len() {
                    project.layers.insert(*layer_idx, target_layer);
                } else {
                    project.layers.push(target_layer);
                }
                project.active_layer_idx = *layer_idx;
                project.rebuild_composite();
            }
        }

        self.redo_stack.push(entry);
        Some(desc)
    }

    pub fn redo(&mut self, project: &mut Project) -> Option<String> {
        let entry = self.redo_stack.pop()?;
        let desc = entry.name.clone();

        match &entry.action {
            HistoryAction::TileStroke {
                layer_idx,
                redo_tiles,
                next_has_mask,
                ..
            } => {
                if let Some(target) = project.layers.get_mut(*layer_idx) {
                    let mut min_x = u32::MAX;
                    let mut min_y = u32::MAX;
                    let mut max_x = 0;
                    let mut max_y = 0;

                    for tile in redo_tiles {
                        target.apply_tile(tile);
                        let x0 = (tile.tile_x as u32) * (HISTORY_TILE_SIZE as u32);
                        let y0 = (tile.tile_y as u32) * (HISTORY_TILE_SIZE as u32);
                        let x1 = x0 + tile.width as u32;
                        let y1 = y0 + tile.height as u32;
                        min_x = min_x.min(x0);
                        min_y = min_y.min(y0);
                        max_x = max_x.max(x1.saturating_sub(1));
                        max_y = max_y.max(y1.saturating_sub(1));
                    }

                    if target.has_mask != *next_has_mask {
                        target.has_mask = *next_has_mask;
                    }

                    if min_x <= max_x && min_y <= max_y {
                        target.dirty_rect = Some((min_x, min_y, max_x, max_y));
                        target.dirty_flags = crate::layer::LayerDirtyFlags::all();
                    } else {
                        target.mark_dirty_full();
                    }
                    project.rebuild_composite();
                }
            }
            HistoryAction::Stroke { layer_idx, next_layer, .. } => {
                if let Some(target) = project.layers.get_mut(*layer_idx) {
                    target.restore_stroke_snapshot(next_layer);
                    target.mark_dirty_full();
                    project.rebuild_composite();
                }
            }
            HistoryAction::LayerStackChange { next_layers, next_active, .. } => {
                project.layers = next_layers.clone();
                for l in &mut project.layers {
                    l.mark_dirty_full();
                }
                project.active_layer_idx = *next_active;
                project.rebuild_composite();
            }
            HistoryAction::LayerPropertyChange { layer_idx, next_layer, .. } => {
                if let Some(target) = project.layers.get_mut(*layer_idx) {
                    *target = (**next_layer).clone();
                    target.mark_dirty_full();
                    project.rebuild_composite();
                }
            }
            HistoryAction::AddLayer { layer_idx, layer, .. } => {
                let mut target_layer = (**layer).clone();
                target_layer.mark_dirty_full();
                if *layer_idx <= project.layers.len() {
                    project.layers.insert(*layer_idx, target_layer);
                } else {
                    project.layers.push(target_layer);
                }
                project.active_layer_idx = *layer_idx;
                project.rebuild_composite();
            }
            HistoryAction::DeleteLayer { layer_idx, prev_active, .. } => {
                if *layer_idx < project.layers.len() {
                    project.layers.remove(*layer_idx);
                }
                project.active_layer_idx = (*prev_active).min(project.layers.len().saturating_sub(1));
                project.rebuild_composite();
            }
        }

        self.undo_stack.push(entry);
        Some(desc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_add_and_delete_layer_undo_redo() {
        let mut project = Project::new_default(64, 64);
        let mut history = HistoryManager::new();

        assert_eq!(project.layers.len(), 1);

        // 1. Add Fill Layer
        let prev_active = project.active_layer_idx;
        let new_idx = project.add_fill_layer(
            Some("Test Fill".to_string()),
            glam::Vec4::new(1.0, 0.0, 0.0, 1.0),
            0.2,
            0.8,
        );
        let layer_clone = Box::new(project.layers[new_idx].clone());
        history.push("Add Fill Layer", HistoryAction::AddLayer {
            layer_idx: new_idx,
            layer: layer_clone,
            prev_active,
        });

        assert_eq!(project.layers.len(), 2);
        assert_eq!(project.active_layer_idx, 1);

        // Undo adding layer
        let undone = history.undo(&mut project);
        assert!(undone.is_some());
        assert_eq!(project.layers.len(), 1);
        assert_eq!(project.active_layer_idx, 0);

        // Redo adding layer
        let redone = history.redo(&mut project);
        assert!(redone.is_some());
        assert_eq!(project.layers.len(), 2);
        assert_eq!(project.active_layer_idx, 1);

        // 2. Delete Layer
        let prev_active = project.active_layer_idx;
        let removed = project.delete_layer(1).expect("Layer should be deleted");
        history.push("Delete Layer", HistoryAction::DeleteLayer {
            layer_idx: 1,
            layer: Box::new(removed),
            prev_active,
        });

        assert_eq!(project.layers.len(), 1);

        // Undo deleting layer
        let undone_del = history.undo(&mut project);
        assert!(undone_del.is_some());
        assert_eq!(project.layers.len(), 2);
        assert_eq!(project.active_layer_idx, 1);

        // Redo deleting layer
        let redone_del = history.redo(&mut project);
        assert!(redone_del.is_some());
        assert_eq!(project.layers.len(), 1);
    }

    #[test]
    fn test_history_tile_stroke_undo_redo_roundtrip() {
        let mut project = Project::new_default(256, 256);
        let mut history = HistoryManager::new();

        let layer = project.active_layer_mut().unwrap();
        assert_eq!(layer.base_color_buf.get_pixel(10, 10), glam::Vec4::new(1.0, 1.0, 1.0, 1.0));

        // 1. Begin stroke recording on Layer
        let ch = crate::channel::ChannelFlags {
            base_color: true,
            roughness: true,
            metallic: false,
            normal: false,
            height: false,
            ao: false,
            emission: false,
            opacity: false,
            clearcoat: false,
            anisotropy: false,
        };
        layer.begin_stroke_recording(crate::brush::PaintTarget::Material, ch);

        // Record pre-change tiles for rect (5..15, 5..15)
        layer.record_tiles_for_rect(5, 5, 15, 15);

        // Modify pixel
        layer.base_color_buf.set_pixel(10, 10, glam::Vec4::new(1.0, 0.0, 0.0, 1.0));
        layer.roughness_buf.set_pixel(10, 10, 0.123);
        assert_eq!(layer.base_color_buf.get_pixel(10, 10), glam::Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(layer.roughness_buf.get_pixel(10, 10), 0.123);

        // Finish stroke delta
        let action = layer.finish_stroke_delta(0, "Paint Red Dot").expect("Must produce TileStroke action");
        match &action {
            HistoryAction::TileStroke { undo_tiles, redo_tiles, .. } => {
                assert_eq!(undo_tiles.len(), 1, "Touching (10, 10) in 256x256 touches exactly 1 128x128 tile");
                assert_eq!(redo_tiles.len(), 1);
            }
            _ => panic!("Expected TileStroke"),
        }

        history.push("Paint Red Dot", action);
        assert!(history.can_undo());

        // Undo stroke: must restore initial white color and 0.5 roughness
        let undone = history.undo(&mut project);
        assert!(undone.is_some());
        assert_eq!(project.layers[0].base_color_buf.get_pixel(10, 10), glam::Vec4::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(project.layers[0].roughness_buf.get_pixel(10, 10), 0.5);

        // Redo stroke: must re-apply red color and 0.123 roughness
        let redone = history.redo(&mut project);
        assert!(redone.is_some());
        assert_eq!(project.layers[0].base_color_buf.get_pixel(10, 10), glam::Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(project.layers[0].roughness_buf.get_pixel(10, 10), 0.123);
    }

    #[test]
    fn test_tile_delta_memory_efficiency() {
        let mut layer = Layer::new_paint_layer(1, "Test 2K Paint".to_string(), 2048, 2048);
        let ch = crate::channel::ChannelFlags {
            base_color: true,
            roughness: false,
            metallic: false,
            normal: false,
            height: false,
            ao: false,
            emission: false,
            opacity: false,
            clearcoat: false,
            anisotropy: false,
        };

        layer.begin_stroke_recording(crate::brush::PaintTarget::Material, ch);
        // Paint a small 50x50 region in tile (0, 0)
        layer.record_tiles_for_rect(20, 20, 70, 70);
        layer.base_color_buf.set_pixel(50, 50, glam::Vec4::new(0.0, 1.0, 0.0, 1.0));

        let action = layer.finish_stroke_delta(0, "Small Dab").expect("Must produce TileStroke");
        match action {
            HistoryAction::TileStroke { undo_tiles, redo_tiles, .. } => {
                assert_eq!(undo_tiles.len(), 1);
                assert_eq!(redo_tiles.len(), 1);
                let tile = &undo_tiles[0];
                let px_count = tile.width as usize * tile.height as usize;
                assert_eq!(px_count, 128 * 128); // 16,384 pixels

                // The tile delta only holds base_color (16 bytes * 16384 = 262 KB)
                // Compared to 2048x2048 full layer buffers (~325 MB), this is over 1000x smaller!
                assert!(tile.base_color.is_some());
                assert!(tile.normal.is_none());
                assert!(tile.roughness.is_none());
            }
            _ => panic!("Expected TileStroke"),
        }
    }
}
