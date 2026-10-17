// 3D Paintcraft - Industry Undo / Redo History Stack
// Pure safe Rust

use crate::layer::Layer;
use crate::project::Project;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum HistoryAction {
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
            undo_stack: Vec::with_capacity(32),
            redo_stack: Vec::with_capacity(32),
            max_undo: 30,
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
            4
        } else if max_dim >= 2048 {
            8
        } else if max_dim >= 1024 {
            16
        } else {
            30
        };
        while self.undo_stack.len() > self.max_undo {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self, project: &mut Project) -> Option<String> {
        let entry = self.undo_stack.pop()?;
        let desc = entry.name.clone();

        match &entry.action {
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
        }

        self.redo_stack.push(entry);
        Some(desc)
    }

    pub fn redo(&mut self, project: &mut Project) -> Option<String> {
        let entry = self.redo_stack.pop()?;
        let desc = entry.name.clone();

        match &entry.action {
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
        }

        self.undo_stack.push(entry);
        Some(desc)
    }
}
