use super::*;
use eframe::egui::{self, Color32};

impl PaintcraftApp {
    /// Render Texture Set List content (scrollable list of texture sets and actions)
    pub fn render_texture_sets_content(&mut self, ui: &mut egui::Ui) {
                    // Scrollable list of texture sets
                    let max_ts_height = (self.project.texture_sets.len() as f32 * 28.0).clamp(40.0, 130.0);
                    egui::ScrollArea::vertical()
                        .max_height(max_ts_height)
                        .show(ui, |ui| {
                            let mut switch_to = None;
                            let mut toggle_vis = None;
                            let mut toggle_solo = None;

                            for (idx, ts) in self.project.texture_sets.iter().enumerate() {
                                let is_active = idx == self.project.active_texture_set_idx;
                                let is_visible = ts.visible;
                                
                                ui.horizontal_wrapped(|ui| {
                                    // Eye visibility toggle
                                    let eye_txt = if is_visible { "[V]" } else { "[ ]" };
                                    let eye_col = if is_visible { Color32::from_rgb(220, 225, 235) } else { Color32::from_rgb(100, 105, 115) };
                                    if ui.button(egui::RichText::new(eye_txt).monospace().color(eye_col)).on_hover_text("Toggle Texture Set / Sub-mesh visibility in 3D viewport").clicked() {
                                        toggle_vis = Some(idx);
                                    }

                                    // Solo toggle
                                    let solo_col = if is_active { Color32::from_rgb(243, 94, 24) } else { Color32::from_rgb(130, 135, 145) };
                                    if ui.small_button(egui::RichText::new("S").strong().color(solo_col)).on_hover_text("Solo Texture Set (isolate in viewport)").clicked() {
                                        toggle_solo = Some(idx);
                                    }

                                    // Name and tri count
                                    let tri_count: usize = ts.submesh_indices.iter()
                                        .filter_map(|&sm_idx| self.project.mesh.submeshes.get(sm_idx))
                                        .map(|sm| sm.index_count / 3)
                                        .sum();

                                    let label_text = format!("{}  ({} tris)", ts.name, tri_count);
                                    let resp = if is_active {
                                        ui.selectable_label(true, egui::RichText::new(&label_text).strong().color(Color32::WHITE))
                                    } else {
                                        ui.selectable_label(false, egui::RichText::new(&label_text).color(Color32::from_rgb(180, 185, 195)))
                                    };

                                    if resp.clicked() && !is_active {
                                        switch_to = Some(idx);
                                    }
                                });
                            }

                            if let Some(idx) = toggle_vis {
                                self.project.toggle_texture_set_visibility(idx);
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    wgpu.sync_submesh_visibility(&self.project.mesh.submeshes);
                                }
                            }
                            if let Some(idx) = toggle_solo {
                                self.project.solo_texture_set(idx);
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    wgpu.sync_submesh_visibility(&self.project.mesh.submeshes);
                                }
                            }
                            if let Some(idx) = switch_to {
                                self.project.switch_texture_set(idx);
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    wgpu.sync_textures(&mut self.project);
                                }
                                self.status_message = format!("Active Texture Set: {}", self.project.texture_sets[idx].name);
                            }
                        });

                    // Active Texture Set size selector and quick actions
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("Size:").small().color(Color32::from_rgb(140, 145, 155)));
                        let res_list = [512, 1024, 2048, 4096];
                        for &r in &res_list {
                            let is_curr = self.project.texture_width == r;
                            let btn_text = match r {
                                512 => "512",
                                1024 => "1K",
                                2048 => "2K",
                                4096 => "4K",
                                _ => "Custom",
                            };
                            if ui.selectable_label(is_curr, btn_text).clicked() && !is_curr {
                                self.project.resize_textures(r, r);
                                self.history.adjust_max_undo_for_resolution(r, r);
                                self.project.rebuild_composite();
                                self.project.gpu_sync_needed = true;
                                self.status_message = format!("Resized texture set to {}x{}", r, r);
                            }
                        }

                        if self.project.texture_sets.len() > 1 {
                            if ui.small_button("Del").on_hover_text("Delete active texture set").clicked() {
                                let cur_idx = self.project.active_texture_set_idx;
                                self.project.delete_texture_set(cur_idx);
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    wgpu.sync_textures(&mut self.project);
                                }
                            }
                        }
                        if ui.small_button("Dup").on_hover_text("Duplicate active texture set").clicked() {
                            let cur_idx = self.project.active_texture_set_idx;
                            self.project.duplicate_texture_set(cur_idx);
                            if let Some(wgpu) = &mut self.wgpu_viewport {
                                wgpu.sync_textures(&mut self.project);
                            }
                        }
                        if ui.small_button("+ Set").on_hover_text("Add new empty Texture Set").clicked() {
                            let new_id = self.project.texture_sets.len() + 1;
                            self.project.add_texture_set(
                                format!("Material_{}", new_id),
                                format!("Material_{}", new_id),
                                vec![],
                            );
                            if let Some(wgpu) = &mut self.wgpu_viewport {
                                wgpu.sync_textures(&mut self.project);
                            }
                        }
                    });
    }

    /// Render Layers Stack content
    pub fn render_layers_content(&mut self, ui: &mut egui::Ui, pane_height: Option<f32>) {
        let total_height = ui.available_height();
        let layers_pane_height = pane_height.unwrap_or_else(|| (total_height * 0.36).clamp(175.0, 420.0));
        ui.group(|ui| {
            if pane_height.is_some() {
                ui.set_height(layers_pane_height);
            }
                        // Layers Window Header & Active Blend / Opacity Controls
                        ui.horizontal_wrapped(|ui| {
                            ui.heading("Layers");

                            // Substance 3D Painter Layer Stack Channel Selector
                            ui.label("Channel:");
                            egui::ComboBox::from_id_source("layer_stack_channel_selector")
                                .selected_text(self.layer_stack_channel.short_name())
                                .show_ui(ui, |ui| {
                                    let channels = [
                                        crate::channel::ChannelType::BaseColor,
                                        crate::channel::ChannelType::Roughness,
                                        crate::channel::ChannelType::Metallic,
                                        crate::channel::ChannelType::Normal,
                                        crate::channel::ChannelType::Height,
                                        crate::channel::ChannelType::AmbientOcclusion,
                                        crate::channel::ChannelType::Emission,
                                        crate::channel::ChannelType::Opacity,
                                        crate::channel::ChannelType::Clearcoat,
                                        crate::channel::ChannelType::Anisotropy,
                                    ];
                                    for ch in channels {
                                        ui.selectable_value(&mut self.layer_stack_channel, ch, ch.name());
                                    }
                                });
                        });

                        let act_idx = self.project.active_layer_idx;
                        let mut blend_or_opacity_changed = false;

                        // Blend Mode and Opacity (Affecting the active selected layer's active channel)
                        if let Some(act_layer) = self.project.layers.get_mut(act_idx) {
                            let cur_ch = self.layer_stack_channel;
                            let mut cur_mode = act_layer.get_channel_blend_mode(cur_ch);
                            let mut cur_ch_op = act_layer.get_channel_opacity(cur_ch);

                            ui.horizontal_wrapped(|ui| {
                                ui.label("Blend:");
                                egui::ComboBox::from_id_source("active_layer_channel_blend_mode")
                                    .selected_text(cur_mode.name())
                                    .show_ui(ui, |ui| {
                                        let modes = [
                                            BlendMode::PassThrough,
                                            BlendMode::Normal,
                                            BlendMode::Multiply,
                                            BlendMode::Screen,
                                            BlendMode::Overlay,
                                            BlendMode::Add,
                                            BlendMode::Subtract,
                                            BlendMode::Darken,
                                            BlendMode::Lighten,
                                            BlendMode::Replace,
                                        ];
                                        for m in modes {
                                            if ui.selectable_value(&mut cur_mode, m, m.name()).clicked() {
                                                act_layer.set_channel_blend_mode(cur_ch, m);
                                                blend_or_opacity_changed = true;
                                            }
                                        }
                                    });

                                ui.label("Opac:");
                                if ui.add_sized([50.0, 18.0], egui::Slider::new(&mut cur_ch_op, 0.0..=1.0)).changed() {
                                    act_layer.set_channel_opacity(cur_ch, cur_ch_op);
                                    blend_or_opacity_changed = true;
                                }

                                ui.label("Lyr:");
                                if ui.add_sized([42.0, 18.0], egui::Slider::new(&mut act_layer.opacity, 0.0..=1.0)).on_hover_text("Master layer opacity").changed() {
                                    blend_or_opacity_changed = true;
                                }
                            });
                        }

                        if blend_or_opacity_changed {
                            if self.wgpu_viewport.is_some() && ui.input(|i| i.pointer.any_down()) {
                                self.project.gpu_sync_needed = true;
                                self.need_cpu_composite_sync = true;
                            } else {
                                self.project.rebuild_composite();
                            }
                        }

                        // Layer Stack Toolbar
                        let mut delete_layer_requested = false;

                        #[derive(Clone, Copy)]
                        enum MaskAddAction {
                            Black,
                            White,
                            Curvature,
                            Grunge,
                            Invert,
                            Remove,
                        }
                        let mut mask_add_action = None;
                        let mut duplicate_layer_requested = false;
                        let mut save_as_mat_requested = false;

                        // Modern Uniform Icon Toolbar for Layers Stack
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);

                            let render_tb_btn = |ui: &mut egui::Ui, tex_id: Option<egui::TextureId>, fallback_glyph: &str, tip: &str| -> egui::Response {
                                let btn_size = egui::vec2(28.0, 24.0);
                                let (rect, resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
                                let is_down = resp.is_pointer_button_down_on();
                                let bg = if is_down {
                                    Color32::from_rgb(52, 56, 66)
                                } else if resp.hovered() {
                                    Color32::from_rgb(42, 46, 56)
                                } else {
                                    Color32::from_rgb(28, 30, 36)
                                };
                                let stroke = if resp.hovered() {
                                    egui::Stroke::new(1.0_f32, Color32::from_rgb(90, 96, 112))
                                } else {
                                    egui::Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 56))
                                };
                                ui.painter().rect(rect, 3.0, bg, stroke);

                                if let Some(tex) = tex_id {
                                    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(15.0, 15.0));
                                    let tint = if resp.hovered() {
                                        Color32::WHITE
                                    } else {
                                        Color32::from_rgba_unmultiplied(200, 205, 218, 225)
                                    };
                                    ui.painter().image(
                                        tex,
                                        icon_rect,
                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        tint,
                                    );
                                } else {
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        fallback_glyph,
                                        egui::FontId::proportional(11.0),
                                        Color32::from_rgb(210, 215, 225),
                                    );
                                }
                                resp.on_hover_text(tip)
                            };

                            let icons = self.icons.as_ref();
                            let paint_tex = icons.map(|ic| ic.layer_paint.id());
                            let fill_tex = icons.map(|ic| ic.layer_fill.id());
                            let folder_tex = icons.map(|ic| ic.layer_folder.id());
                            let mask_tex = icons.map(|ic| ic.layer_mask.id());
                            let shaders_tex = icons.map(|ic| ic.shaders.id());
                            let dup_tex = icons.map(|ic| ic.layer_duplicate.id());
                            let del_tex = icons.map(|ic| ic.layer_delete.id());

                            // 1. Add Paint Layer
                            if render_tb_btn(ui, paint_tex, "+P", "Add Paint Layer").clicked() {
                                let prev_active = self.project.active_layer_idx;
                                let new_idx = self.project.add_paint_layer(None);
                                self.select_layer(new_idx, ui.ctx());
                                let layer_clone = self.project.layers[new_idx].clone_stroke_snapshot();
                                self.history.push("Add Paint Layer", HistoryAction::AddLayer {
                                    layer_idx: new_idx,
                                    layer: layer_clone,
                                    prev_active,
                                });
                                self.status_message = "Added Paint Layer.".to_string();
                            }

                            // 2. Add Fill Layer
                            if render_tb_btn(ui, fill_tex, "+F", "Add Fill Layer").clicked() {
                                let prev_active = self.project.active_layer_idx;
                                let new_idx = self.project.add_fill_layer(
                                    None,
                                    glam::Vec4::ONE,
                                    0.5,
                                    0.0,
                                );
                                self.select_layer(new_idx, ui.ctx());
                                let layer_clone = self.project.layers[new_idx].clone_stroke_snapshot();
                                self.history.push("Add Fill Layer", HistoryAction::AddLayer {
                                    layer_idx: new_idx,
                                    layer: layer_clone,
                                    prev_active,
                                });
                                self.status_message = "Added Fill Layer.".to_string();
                            }

                            // 2b. Add Path Layer (Non-destructive Vector Path Layer)
                            if render_tb_btn(ui, paint_tex, "+Path", "Add Vector Path Layer (Always live and editable)").clicked() {
                                let prev_active = self.project.active_layer_idx;
                                let new_idx = self.project.add_path_layer(None);
                                self.select_layer(new_idx, ui.ctx());
                                let layer_clone = self.project.layers[new_idx].clone_stroke_snapshot();
                                self.history.push("Add Path Layer", HistoryAction::AddLayer {
                                    layer_idx: new_idx,
                                    layer: layer_clone,
                                    prev_active,
                                });
                                self.status_message = "Added Vector Path Layer (always editable).".to_string();
                            }

                            // 3. Add Folder
                            if render_tb_btn(ui, folder_tex, "+D", "Add Layer Folder / Group").clicked() {
                                let prev_active = self.project.active_layer_idx;
                                let new_idx = self.project.add_folder_layer(None);
                                self.select_layer(new_idx, ui.ctx());
                                let layer_clone = self.project.layers[new_idx].clone_stroke_snapshot();
                                self.history.push("Add Folder", HistoryAction::AddLayer {
                                    layer_idx: new_idx,
                                    layer: layer_clone,
                                    prev_active,
                                });
                                self.status_message = "Added Layer Folder.".to_string();
                            }

                            // 4. Add Mask Menu
                            let mask_popup_id = ui.make_persistent_id("layers_toolbar_mask_menu_popup");
                            let mask_btn_resp = render_tb_btn(ui, mask_tex, "+M", "Add Layer Mask / Mask Options");
                            if mask_btn_resp.clicked() {
                                ui.memory_mut(|mem| mem.toggle_popup(mask_popup_id));
                            }
                            let has_active_mask = self.project.layers.get(act_idx).map(|l| l.has_mask).unwrap_or(false);
                            egui::popup_below_widget(ui, mask_popup_id, &mask_btn_resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                                ui.set_min_width(170.0);
                                if ui.button("Black Mask (Hide All)").clicked() {
                                    mask_add_action = Some(MaskAddAction::Black);
                                }
                                if ui.button("White Mask (Reveal All)").clicked() {
                                    mask_add_action = Some(MaskAddAction::White);
                                }
                                if ui.button("Edge Wear Mask (Curvature)").clicked() {
                                    mask_add_action = Some(MaskAddAction::Curvature);
                                }
                                if ui.button("Grunge Mask").clicked() {
                                    mask_add_action = Some(MaskAddAction::Grunge);
                                }
                                if has_active_mask {
                                    ui.separator();
                                    if ui.button("Invert Mask").clicked() {
                                        mask_add_action = Some(MaskAddAction::Invert);
                                    }
                                    if ui.button("Remove Mask").clicked() {
                                        mask_add_action = Some(MaskAddAction::Remove);
                                    }
                                }
                            });

                            // 5. Effects / Filters / Layer Options
                            let fx_popup_id = ui.make_persistent_id("layers_toolbar_fx_menu_popup");
                            let fx_btn_resp = render_tb_btn(ui, shaders_tex, "FX", "Layer Effects, Filters & Organization");
                            if fx_btn_resp.clicked() {
                                ui.memory_mut(|mem| mem.toggle_popup(fx_popup_id));
                            }
                            egui::popup_below_widget(ui, fx_popup_id, &fx_btn_resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                                ui.set_min_width(180.0);
                                if ui.button("Filters... (Blur, Levels, Sharp)").clicked() {
                                    self.show_filter_modal = true;
                                }
                                ui.separator();
                                if ui.button("Move Layer Up").clicked() {
                                    self.project.move_layer_up(act_idx);
                                    self.project.rebuild_composite();
                                }
                                if ui.button("Move Layer Down").clicked() {
                                    self.project.move_layer_down(act_idx);
                                    self.project.rebuild_composite();
                                }
                                ui.separator();
                                if ui.button("Save as Shelf Material").clicked() {
                                    save_as_mat_requested = true;
                                }
                            });

                            // 6. Duplicate Layer
                            if render_tb_btn(ui, dup_tex, "Dup", "Duplicate selected layer (Ctrl+D)").clicked() {
                                duplicate_layer_requested = true;
                            }

                            // 7. Delete Layer
                            if render_tb_btn(ui, del_tex, "Del", "Delete selected layer (Delete or Backspace)").clicked() {
                                delete_layer_requested = true;
                            }
                        });

                        if let Some(action) = mask_add_action {
                            let curv = self.project.curvature_map.clone();
                            if let Some(layer) = self.project.layers.get_mut(act_idx) {
                                match action {
                                    MaskAddAction::Black => {
                                        layer.add_black_mask();
                                        self.brush_engine.settings.target = PaintTarget::Mask;
                                        self.brush_engine.settings.mask_paint_value = 1.0;
                                    }
                                    MaskAddAction::White => {
                                        layer.add_white_mask();
                                        self.brush_engine.settings.target = PaintTarget::Mask;
                                        self.brush_engine.settings.mask_paint_value = 0.0;
                                    }
                                    MaskAddAction::Curvature => {
                                        layer.add_curvature_mask(curv.as_ref());
                                        self.brush_engine.settings.target = PaintTarget::Mask;
                                    }
                                    MaskAddAction::Grunge => {
                                        layer.add_grunge_mask();
                                        self.brush_engine.settings.target = PaintTarget::Mask;
                                    }
                                    MaskAddAction::Invert => {
                                        layer.invert_mask();
                                    }
                                    MaskAddAction::Remove => {
                                        layer.has_mask = false;
                                        if self.mask_preview_layer_id == Some(layer.id) {
                                            self.mask_preview_layer_id = None;
                                            self.uv_view_mask_mode = false;
                                            if let Some(wgpu) = &mut self.wgpu_viewport {
                                                wgpu.mask_preview_layer_id = None;
                                                wgpu.sync_textures(&mut self.project);
                                            }
                                        }
                                        self.brush_engine.settings.target = PaintTarget::Material;
                                    }
                                }
                            }
                            self.project.rebuild_composite();
                            self.project.gpu_sync_needed = true;
                            self.need_cpu_composite_sync = true;
                        }

                        if duplicate_layer_requested {
                            if let Some(new_idx) = self.project.duplicate_layer(act_idx) {
                                let layer_clone = Box::new(self.project.layers[new_idx].clone());
                                self.history.push("Duplicate Layer", HistoryAction::AddLayer {
                                    layer_idx: new_idx,
                                    layer: layer_clone,
                                    prev_active: act_idx,
                                });
                                self.status_message = format!("Duplicated layer #{}", new_idx);
                            }
                        }

                        if save_as_mat_requested {
                            if let Some(layer) = self.project.layers.get(act_idx) {
                                let name = layer.name.clone();
                                let clean_id = name.trim().replace(' ', "_").to_lowercase();
                                let mat = crate::asset_manager::CustomMaterialEntry {
                                    id: clean_id,
                                    name: name.clone(),
                                    description: format!("Custom PBR material created from layer '{}'", name),
                                    path: std::path::PathBuf::new(),
                                    file_name: String::new(),
                                    base_color: [layer.fill_color.x, layer.fill_color.y, layer.fill_color.z, layer.fill_color.w],
                                    roughness: layer.fill_roughness,
                                    metallic: layer.fill_metallic,
                                    height: layer.fill_height,
                                    normal: [0.5, 0.5, 1.0, 0.5],
                                    emissive: [layer.fill_emissive.x, layer.fill_emissive.y, layer.fill_emissive.z, layer.fill_emissive.w],
                                    opacity: layer.fill_opacity,
                                    clearcoat: layer.fill_clearcoat,
                                    anisotropy: layer.fill_anisotropy,
                                };
                                match self.asset_manager.save_material(&mat) {
                                    Ok(path) => {
                                        self.status_message = format!("Saved Material '{}' to {:?}", mat.name, path.file_name().unwrap_or_default());
                                    }
                                    Err(e) => {
                                        self.status_message = format!("Failed to save material: {}", e);
                                    }
                                }
                            }
                        }

                        if delete_layer_requested {
                            let ctx = ui.ctx().clone();
                            self.delete_selected_layer(&ctx);
                            ctx.request_repaint();
                        }
    
                        ui.separator();
    
                        let mut toggle_mask_preview_id = None;
                        // Dedicated Scroll Area for Layer Stack
                        egui::ScrollArea::vertical()
                            .id_source("layers_stack_scroll")
                            .max_height(layers_pane_height - 90.0)
                            .show(ui, |ui| {
                                struct LayerCardSummary {
                                    id: u64,
                                    name: String,
                                    parent_folder_id: Option<u64>,
                                    folder_collapsed: bool,
                                    visible: bool,
                                    layer_type: LayerType,
                                    fill_color: Vec4,
                                    has_mask: bool,
                                }

                                let layer_count = self.project.layers.len();
                                let summaries: Vec<LayerCardSummary> = self.project.layers.iter().map(|l| LayerCardSummary {
                                    id: l.id,
                                    name: l.name.clone(),
                                    parent_folder_id: l.parent_folder_id,
                                    folder_collapsed: l.folder_collapsed,
                                    visible: l.visible,
                                    layer_type: l.layer_type,
                                    fill_color: l.fill_color,
                                    has_mask: l.has_mask,
                                }).collect();

                                let folders: Vec<(u64, String)> = summaries.iter()
                                    .filter(|l| l.layer_type == LayerType::Folder)
                                    .map(|l| (l.id, l.name.clone()))
                                    .collect();

                                let mut reorder_request = None;
                                let mut toggle_visible_idx = None;
                                let mut toggle_collapse_idx = None;
                                let mut add_mask_idx = None;
                                let mut rename_request = None;
                                let mut folder_change = None;

                                let mouse_pos = ui.input(|i| i.pointer.hover_pos());
                                let mouse_released = ui.input(|i| i.pointer.any_released());

                                let (paint_icon, fill_icon, folder_icon, mask_icon_id, eye_icon_id) = if let Some(ic) = self.icons.as_ref() {
                                    (Some(ic.layer_paint.id()), Some(ic.layer_fill.id()), Some(ic.layer_folder.id()), Some(ic.layer_mask.id()), Some(ic.layer_eye.id()))
                                } else {
                                    (None, None, None, None, None)
                                };

                                for rev_i in 0..layer_count {
                                    let i = layer_count - 1 - rev_i;
                                    let layer = &summaries[i];

                                    // Check if this layer is inside a collapsed folder
                                    if let Some(pid) = layer.parent_folder_id {
                                        if summaries.iter().any(|l| l.id == pid && l.folder_collapsed) {
                                            continue;
                                        }
                                    }

                                    let is_active = self.project.active_layer_idx == i;
                                    let is_target_mat = is_active && self.brush_engine.settings.target == PaintTarget::Material;
                                    let is_target_mask = is_active && self.brush_engine.settings.target == PaintTarget::Mask;

                                    let card_bg = if is_active {
                                        Color32::from_rgb(42, 46, 56)
                                    } else {
                                        Color32::from_rgb(26, 28, 34)
                                    };
                                    // Highlight the OUTLINE of the active layer card!
                                    let card_stroke = if is_active {
                                        egui::Stroke::new(1.8_f32, Color32::from_rgb(243, 94, 24))
                                    } else {
                                        egui::Stroke::new(1.0_f32, Color32::from_rgb(46, 50, 60))
                                    };

                                    let card_response = egui::Frame::none()
                                        .fill(card_bg)
                                        .stroke(card_stroke)
                                        .rounding(4.0)
                                        .inner_margin(egui::Margin::symmetric(6.0, 5.0))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                // 0. Child indentation
                                                if layer.parent_folder_id.is_some() {
                                                    ui.add_space(14.0);
                                                }

                                                // 1. Drag & Drop Handle
                                                let drag_label = egui::RichText::new("⠿")
                                                    .color(if is_active { Color32::from_rgb(243, 94, 24) } else { Color32::from_rgb(110, 115, 125) })
                                                    .size(12.0);
                                                let drag_resp = ui.add(egui::Label::new(drag_label).selectable(false).sense(egui::Sense::drag()))
                                                    .on_hover_text("Drag to reorder layer in stack");
                                                if drag_resp.drag_started() {
                                                    self.dragged_layer_idx = Some(i);
                                                }

                                                // 2. Visibility toggle
                                                let (eye_rect, eye_resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                                                if let Some(eye_id) = eye_icon_id {
                                                    let tint = if layer.visible {
                                                        Color32::WHITE
                                                    } else {
                                                        Color32::from_rgba_unmultiplied(100, 105, 115, 110)
                                                    };
                                                    ui.painter().image(
                                                        eye_id,
                                                        eye_rect,
                                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                                        tint,
                                                    );
                                                } else {
                                                    let eye_icon = if layer.visible { "👁" } else { "⊘" };
                                                    let eye_color = if layer.visible { Color32::from_rgb(220, 225, 235) } else { Color32::from_rgb(110, 115, 125) };
                                                    ui.painter().text(eye_rect.center(), egui::Align2::CENTER_CENTER, eye_icon, egui::FontId::proportional(11.0), eye_color);
                                                }
                                                if eye_resp.on_hover_text("Toggle Layer Visibility").clicked() {
                                                    toggle_visible_idx = Some(i);
                                                }

                                                // 3. Folder Chevron
                                                if layer.layer_type == LayerType::Folder {
                                                    let chev = if layer.folder_collapsed { "▶" } else { "▼" };
                                                    if ui.add(egui::Label::new(egui::RichText::new(chev).color(Color32::from_rgb(245, 166, 35)).size(10.0)).selectable(false).sense(egui::Sense::click())).on_hover_text("Expand/collapse folder").clicked() {
                                                        toggle_collapse_idx = Some(i);
                                                    }
                                                }

                                                // 3. Dual Swatch Buttons: Material Swatch & Mask Swatch (Directly swap between Mask & Layer)
                                                // 3a. Material Swatch Button
                                                let (mat_rect, mat_resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                                                if mat_resp.hovered() {
                                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                }
                                                let mat_bg = match layer.layer_type {
                                                    LayerType::Fill => Color32::from_rgb(
                                                        (layer.fill_color.x * 255.0) as u8,
                                                        (layer.fill_color.y * 255.0) as u8,
                                                        (layer.fill_color.z * 255.0) as u8,
                                                    ),
                                                    LayerType::Paint => if mat_resp.hovered() { Color32::from_rgb(58, 64, 78) } else { Color32::from_rgb(46, 52, 64) },
                                                    LayerType::Folder => Color32::from_rgb(160, 115, 35),
                                                    LayerType::Path => if mat_resp.hovered() { Color32::from_rgb(38, 70, 105) } else { Color32::from_rgb(28, 55, 85) },
                                                };
                                                let mat_stroke = if is_target_mat {
                                                    egui::Stroke::new(2.0_f32, Color32::from_rgb(243, 94, 24))
                                                } else if mat_resp.hovered() {
                                                    egui::Stroke::new(1.5_f32, Color32::from_rgb(140, 145, 160))
                                                } else {
                                                    egui::Stroke::new(1.0_f32, Color32::from_rgb(58, 62, 72))
                                                };
                                                ui.painter().rect(mat_rect, 3.0, mat_bg, mat_stroke);
                                                let icon_tex = match layer.layer_type {
                                                    LayerType::Paint => paint_icon,
                                                    LayerType::Folder => folder_icon,
                                                    LayerType::Fill => fill_icon,
                                                    LayerType::Path => paint_icon,
                                                };
                                                if let Some(tex) = icon_tex {
                                                    let icon_inset = egui::Rect::from_center_size(mat_rect.center(), egui::vec2(14.0, 14.0));
                                                    ui.painter().image(
                                                        tex,
                                                        icon_inset,
                                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                                        Color32::WHITE,
                                                    );
                                                } else {
                                                    if layer.layer_type == LayerType::Paint {
                                                        ui.painter().text(mat_rect.center(), egui::Align2::CENTER_CENTER, "P", egui::FontId::proportional(10.0), Color32::WHITE);
                                                    } else if layer.layer_type == LayerType::Folder {
                                                        ui.painter().text(mat_rect.center(), egui::Align2::CENTER_CENTER, "D", egui::FontId::proportional(10.0), Color32::WHITE);
                                                    } else if layer.layer_type == LayerType::Path {
                                                        ui.painter().text(mat_rect.center(), egui::Align2::CENTER_CENTER, "~", egui::FontId::proportional(12.0), Color32::from_rgb(0, 200, 255));
                                                    }
                                                }
                                                if mat_resp.on_hover_text("Layer / Material (Click to switch to Layer painting)").clicked() {
                                                    self.select_layer(i, ui.ctx());
                                                }

                                                // 3b. Mask Swatch Button (if layer has mask)
                                                if layer.has_mask {
                                                    let (mask_rect, mask_resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                                                    if mask_resp.hovered() {
                                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                    }
                                                    let is_mask_previewing = self.mask_preview_layer_id == Some(layer.id);
                                                    let mask_stroke = if is_mask_previewing {
                                                        egui::Stroke::new(2.0_f32, Color32::from_rgb(0, 215, 255))
                                                    } else if is_active && is_target_mask {
                                                        egui::Stroke::new(2.0_f32, Color32::from_rgb(243, 94, 24))
                                                    } else if mask_resp.hovered() {
                                                        egui::Stroke::new(1.5_f32, Color32::from_rgb(140, 145, 160))
                                                    } else {
                                                        egui::Stroke::new(1.0_f32, Color32::from_rgb(60, 65, 75))
                                                    };
                                                    let mask_bg = if mask_resp.hovered() { Color32::from_rgb(32, 36, 44) } else { Color32::from_rgb(18, 20, 24) };
                                                    ui.painter().rect(mask_rect, 3.0, mask_bg, mask_stroke);

                                                    if let Some(tex) = mask_icon_id {
                                                        let icon_inset = egui::Rect::from_center_size(mask_rect.center(), egui::vec2(15.0, 15.0));
                                                        let tint = if is_mask_previewing {
                                                            Color32::from_rgb(0, 215, 255)
                                                        } else if is_active && is_target_mask {
                                                            Color32::WHITE
                                                        } else if mask_resp.hovered() {
                                                            Color32::from_rgb(230, 235, 245)
                                                        } else {
                                                            Color32::from_rgb(175, 180, 192)
                                                        };
                                                        ui.painter().image(
                                                            tex,
                                                            icon_inset,
                                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                                            tint,
                                                        );
                                                    } else {
                                                        let circle_color = if is_mask_previewing {
                                                            Color32::from_rgb(0, 215, 255)
                                                        } else {
                                                            Color32::from_rgb(235, 235, 240)
                                                        };
                                                        ui.painter().circle_filled(mask_rect.center(), 5.5, circle_color);
                                                    }

                                                    let hover_text = if is_mask_previewing {
                                                        "Mask Preview Active • Click to switch to Mask painting • Alt+Click to exit preview"
                                                    } else {
                                                        "Mask (Click to switch to Mask painting • Alt+Click to isolate preview unlit)"
                                                    };
                                                    if mask_resp.on_hover_text(hover_text).clicked() {
                                                        if ui.input(|inp| inp.modifiers.alt) {
                                                            toggle_mask_preview_id = Some(layer.id);
                                                        } else {
                                                            self.select_layer(i, ui.ctx());
                                                            self.brush_engine.settings.target = PaintTarget::Mask;
                                                            self.status_message = format!("Target: Mask (Layer '{}')", layer.name);
                                                        }
                                                        ui.ctx().request_repaint();
                                                    }
                                                } else {
                                                    // Elegant add-mask placeholder slot
                                                    let (slot_rect, slot_resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                                                    let slot_bg = if slot_resp.hovered() { Color32::from_rgb(38, 42, 50) } else { Color32::from_rgba_unmultiplied(24, 26, 32, 140) };
                                                    let slot_stroke = if slot_resp.hovered() {
                                                        egui::Stroke::new(1.0_f32, Color32::from_rgb(100, 108, 125))
                                                    } else {
                                                        egui::Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(50, 54, 65, 140))
                                                    };
                                                    ui.painter().rect(slot_rect, 3.0, slot_bg, slot_stroke);
                                                    ui.painter().text(
                                                        slot_rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "+",
                                                        egui::FontId::proportional(11.0),
                                                        if slot_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(110, 115, 128) },
                                                    );
                                                    if slot_resp.on_hover_text("Add Mask to Layer").clicked() {
                                                        add_mask_idx = Some(i);
                                                        self.select_layer(i, ui.ctx());
                                                        self.brush_engine.settings.target = PaintTarget::Mask;
                                                    }
                                                }

                                                // 4. Layer Name & Inline Renaming
                                                if self.renaming_layer_idx == Some(i) {
                                                    let text_res = ui.add(egui::TextEdit::singleline(&mut self.rename_buffer).desired_width(110.0));
                                                    text_res.request_focus();
                                                    if ui.input(|inp| inp.key_pressed(egui::Key::Enter)) || ui.small_button("OK").clicked() {
                                                        if !self.rename_buffer.trim().is_empty() {
                                                            rename_request = Some((i, self.rename_buffer.trim().to_string()));
                                                        }
                                                        self.renaming_layer_idx = None;
                                                    }
                                                    if ui.input(|inp| inp.key_pressed(egui::Key::Escape)) || ui.small_button("X").clicked() {
                                                        self.renaming_layer_idx = None;
                                                    }
                                                } else {
                                                    let name_color = if is_active {
                                                        Color32::from_rgb(255, 255, 255)
                                                    } else {
                                                        Color32::from_rgb(195, 200, 210)
                                                    };
                                                    let name_text = egui::RichText::new(&layer.name)
                                                        .color(name_color)
                                                        .strong();
                                                    let name_resp = ui.add(egui::Label::new(name_text).truncate().selectable(false).sense(egui::Sense::click()))
                                                        .on_hover_text("Click to select layer • Double-click to rename");

                                                    if name_resp.clicked() {
                                                        self.select_layer(i, ui.ctx());
                                                    }
                                                    if name_resp.double_clicked() {
                                                        self.renaming_layer_idx = Some(i);
                                                        self.rename_buffer = layer.name.clone();
                                                    }
                                                }

                                                // 5. Folder membership (Move in/out)
                                                if layer.layer_type != LayerType::Folder && !folders.is_empty() {
                                                    let cur_layer_id = layer.id;
                                                    if layer.parent_folder_id.is_some() {
                                                        if ui.add(egui::Button::new(egui::RichText::new("▲").size(10.0)).fill(Color32::TRANSPARENT)).on_hover_text("Remove from folder").clicked() {
                                                            folder_change = Some((i, None));
                                                        }
                                                    } else {
                                                        ui.menu_button("📁", |ui| {
                                                            for (fid, fname) in &folders {
                                                                if *fid != cur_layer_id && ui.button(fname).clicked() {
                                                                    folder_change = Some((i, Some(*fid)));
                                                                    ui.close_menu();
                                                                }
                                                            }
                                                        });
                                                    }
                                                }

                                                // 6. Fill remaining card width for selecting layer & dragging
                                                let avail = ui.available_size_before_wrap();
                                                if avail.x > 0.0 && avail.y > 0.0 {
                                                    let (_, empty_resp) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
                                                    if empty_resp.clicked() {
                                                        self.select_layer(i, ui.ctx());
                                                    }
                                                    if empty_resp.drag_started() {
                                                        self.dragged_layer_idx = Some(i);
                                                    }
                                                }
                                            });
                                        });

                                    let card_rect = card_response.response.rect;
                                    let card_interact = ui.interact(card_rect, ui.id().with(("card_row_interact", i)), egui::Sense::click());
                                    if card_interact.clicked() && toggle_visible_idx.is_none() && toggle_collapse_idx.is_none() && self.dragged_layer_idx.is_none() {
                                        self.select_layer(i, ui.ctx());
                                    }

                                    // Full Right-Click Layer Context Menu
                                    card_response.response.context_menu(|ui| {
                                        ui.set_min_width(170.0);
                                        if ui.button("Duplicate Layer").clicked() {
                                            if let Some(new_idx) = self.project.duplicate_layer(i) {
                                                let layer_clone = Box::new(self.project.layers[new_idx].clone());
                                                self.history.push("Duplicate Layer", HistoryAction::AddLayer {
                                                    layer_idx: new_idx,
                                                    layer: layer_clone,
                                                    prev_active: i,
                                                });
                                                self.status_message = format!("Duplicated layer #{}", new_idx);
                                            }
                                            ui.close_menu();
                                        }
                                        if ui.button("Delete Layer (Del)").clicked() {
                                            self.project.active_layer_idx = i;
                                            delete_layer_requested = true;
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if layer.has_mask {
                                            let is_editing_mask = self.brush_engine.settings.target == PaintTarget::Mask && self.project.active_layer_idx == i;
                                            if is_editing_mask {
                                                if ui.button("Target: Edit Material (Shift+M)").clicked() {
                                                    self.project.active_layer_idx = i;
                                                    self.brush_engine.settings.target = PaintTarget::Material;
                                                    if self.mask_preview_layer_id.is_some() {
                                                        let ctx = ui.ctx().clone();
                                                        self.exit_mask_preview(&ctx);
                                                    }
                                                    self.status_message = format!("Target: Material (Layer '{}')", layer.name);
                                                    ui.close_menu();
                                                }
                                            } else {
                                                if ui.button("Target: Edit Mask (Shift+M)").clicked() {
                                                    self.project.active_layer_idx = i;
                                                    self.brush_engine.settings.target = PaintTarget::Mask;
                                                    self.status_message = format!("Target: Mask (Layer '{}')", layer.name);
                                                    ui.close_menu();
                                                }
                                            }
                                            let is_prev = self.mask_preview_layer_id == Some(layer.id);
                                            let preview_label = if is_prev { "Exit Mask Preview (Esc)" } else { "Preview Mask (Unlit)" };
                                            if ui.button(preview_label).clicked() {
                                                toggle_mask_preview_id = Some(layer.id);
                                                ui.close_menu();
                                            }
                                            if ui.button("Invert Mask").clicked() {
                                                if let Some(l) = self.project.layers.get_mut(i) {
                                                    l.invert_mask();
                                                }
                                                self.project.rebuild_composite();
                                                self.project.gpu_sync_needed = true;
                                                ui.close_menu();
                                            }
                                            if ui.button("Remove Mask").clicked() {
                                                if let Some(l) = self.project.layers.get_mut(i) {
                                                    l.has_mask = false;
                                                }
                                                if self.mask_preview_layer_id == Some(layer.id) {
                                                    self.mask_preview_layer_id = None;
                                                    self.uv_view_mask_mode = false;
                                                    if let Some(wgpu) = &mut self.wgpu_viewport {
                                                        wgpu.mask_preview_layer_id = None;
                                                        wgpu.sync_textures(&mut self.project);
                                                    }
                                                }
                                                self.brush_engine.settings.target = PaintTarget::Material;
                                                self.project.rebuild_composite();
                                                self.project.gpu_sync_needed = true;
                                                ui.close_menu();
                                            }
                                        } else {
                                            if ui.button("Add White Mask").clicked() {
                                                add_mask_idx = Some(i);
                                                self.project.active_layer_idx = i;
                                                self.brush_engine.settings.target = PaintTarget::Mask;
                                                ui.close_menu();
                                            }
                                            if ui.button("Add Black Mask").clicked() {
                                                if let Some(l) = self.project.layers.get_mut(i) {
                                                    l.add_black_mask();
                                                }
                                                self.project.active_layer_idx = i;
                                                self.brush_engine.settings.target = PaintTarget::Mask;
                                                self.brush_engine.settings.mask_paint_value = 1.0;
                                                self.project.rebuild_composite();
                                                self.project.gpu_sync_needed = true;
                                                ui.close_menu();
                                            }
                                        }
                                        ui.separator();
                                        if ui.button("Filters...").clicked() {
                                            self.project.active_layer_idx = i;
                                            self.show_filter_modal = true;
                                            ui.close_menu();
                                        }
                                        if ui.button("Rename Layer").clicked() {
                                            self.renaming_layer_idx = Some(i);
                                            self.rename_buffer = layer.name.clone();
                                            ui.close_menu();
                                        }
                                        if ui.button("Save as Shelf Material").clicked() {
                                            self.project.active_layer_idx = i;
                                            save_as_mat_requested = true;
                                            ui.close_menu();
                                        }
                                    });

                                    let card_rect = card_response.response.rect;

                                    // Active layer left accent indicator bar
                                    if is_active {
                                        ui.painter().rect_filled(
                                            egui::Rect::from_min_max(
                                                card_rect.min,
                                                egui::pos2(card_rect.min.x + 3.5, card_rect.max.y),
                                            ),
                                            2.0,
                                            Color32::from_rgb(243, 94, 24),
                                        );
                                    }

                                    // Drag-and-drop drop zone detection & guidelines
                                    if let Some(dragged_from) = self.dragged_layer_idx {
                                        if let Some(pos) = mouse_pos {
                                            if card_rect.contains(pos) && dragged_from != i {
                                                // Draw insertion highlight line
                                                let is_upper = pos.y < card_rect.center().y;
                                                let line_y = if is_upper { card_rect.min.y } else { card_rect.max.y };
                                                ui.painter().line_segment(
                                                    [egui::pos2(card_rect.min.x, line_y), egui::pos2(card_rect.max.x, line_y)],
                                                    egui::Stroke::new(2.5_f32, Color32::from_rgb(243, 94, 24)),
                                                );

                                                if mouse_released {
                                                    reorder_request = Some((dragged_from, i));
                                                }
                                            }
                                        }
                                    }

                                    ui.add_space(2.0);
                                }

                                if mouse_released {
                                    self.dragged_layer_idx = None;
                                }

                                if let Some((from, to)) = reorder_request {
                                    self.project.reorder_layer(from, to);
                                    self.status_message = format!("Reordered layer {} -> {}", from, to);
                                    self.dragged_layer_idx = None;
                                }

                                if let Some(idx) = toggle_visible_idx {
                                    if let Some(l) = self.project.layers.get_mut(idx) {
                                        l.visible = !l.visible;
                                    }
                                    self.project.rebuild_composite();
                                    self.project.gpu_sync_needed = true;
                                }
                                if let Some(idx) = toggle_collapse_idx {
                                    if let Some(l) = self.project.layers.get_mut(idx) {
                                        l.folder_collapsed = !l.folder_collapsed;
                                    }
                                }
                                if let Some(idx) = add_mask_idx {
                                    if let Some(l) = self.project.layers.get_mut(idx) {
                                        l.add_white_mask();
                                    }
                                    self.project.rebuild_composite();
                                    self.project.gpu_sync_needed = true;
                                }
                                if let Some((idx, name)) = rename_request {
                                    if let Some(l) = self.project.layers.get_mut(idx) {
                                        l.name = name;
                                    }
                                }
                                if let Some((idx, pid)) = folder_change {
                                    if let Some(l) = self.project.layers.get_mut(idx) {
                                        l.parent_folder_id = pid;
                                    }
                                    self.project.rebuild_composite();
                                    self.project.gpu_sync_needed = true;
                                }
                            });

                        if let Some(mask_id) = toggle_mask_preview_id {
                            let ctx = ui.ctx().clone();
                            self.toggle_mask_preview(mask_id, &ctx);
                        }
        });
    }

    /// Render Properties content (active layer/mask/paint parameters)
    pub fn render_properties_content(&mut self, ui: &mut egui::Ui) {
                    let act_idx = self.project.active_layer_idx;
                    let is_mask_editing = self.brush_engine.settings.target == PaintTarget::Mask;
    
                    let (has_layer, has_mask, prop_title) = if let Some(layer) = self.project.layers.get(act_idx) {
                        let title = if is_mask_editing {
                            format!("Properties - Mask: {}", layer.name)
                        } else {
                            match layer.layer_type {
                                LayerType::Fill => format!("Properties - Fill: {}", layer.name),
                                LayerType::Paint => format!("Properties - Paint: {}", layer.name),
                                LayerType::Folder => format!("Properties - Folder: {}", layer.name),
                                LayerType::Path => format!("Properties - Path: {}", layer.name),
                            }
                        };
                        (true, layer.has_mask, title)
                    } else {
                        (false, false, String::new())
                    };

                    if has_layer {
                        let mut prop_changed = false;
                        let mut mask_changed = false;
                        let mut open_shelf_picker = None;
                        let mut import_to_shelf_and_assign = None;
                        ui.horizontal(|ui| {
                            ui.heading(&prop_title);
                            if has_mask {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.selectable_label(is_mask_editing, "Mask").on_hover_text("Target Layer Mask (Shift+M)").clicked() {
                                        self.brush_engine.settings.target = PaintTarget::Mask;
                                        ui.ctx().request_repaint();
                                    }
                                    if ui.selectable_label(!is_mask_editing, "Material").on_hover_text("Target Layer Material (Shift+M)").clicked() {
                                        self.brush_engine.settings.target = PaintTarget::Material;
                                        if self.mask_preview_layer_id.is_some() {
                                            let ctx = ui.ctx().clone();
                                            self.exit_mask_preview(&ctx);
                                        }
                                        ui.ctx().request_repaint();
                                    }
                                    ui.label(egui::RichText::new("Target:").strong().size(11.0));
                                });
                            }
                        });
                        ui.add_space(2.0);

                        egui::ScrollArea::vertical()
                            .id_source("properties_panel_scroll")
                            .show(ui, |ui| {
                                let (lower_layers, higher_layers) = self.project.layers.split_at_mut(act_idx);
                                let available_anchors: Vec<String> = lower_layers
                                    .iter()
                                    .filter_map(|l| l.anchor_point.clone())
                                    .collect();
                                let anchor_refs: Vec<crate::layer::AnchorLayerRef<'_>> = lower_layers
                                    .iter()
                                    .filter_map(|l| l.as_anchor_ref())
                                    .collect();
                                let layer = &mut higher_layers[0];
                                let curv_ref = self.project.curvature_map.as_ref();
                                let ao_ref = self.project.ao_map.as_ref();
                                let pos_ref = self.project.position_map.as_ref();
                                let wn_ref = self.project.world_space_normal_map.as_ref();
                                let id_ref = self.project.id_map.as_ref();

                                ui.horizontal(|ui| {
                                    ui.label("Name:");
                                    if ui.text_edit_singleline(&mut layer.name).changed() {
                                        prop_changed = true;
                                    }
                                    ui.add_space(8.0);
                                    let mut is_anchor = layer.anchor_point.is_some();
                                    if ui.checkbox(&mut is_anchor, "Anchor Point").on_hover_text("Expose layer channels as an Anchor Point for procedural mask referencing in layers above").changed() {
                                        if is_anchor {
                                            layer.anchor_point = Some(layer.name.clone());
                                        } else {
                                            layer.anchor_point = None;
                                        }
                                        prop_changed = true;
                                    }
                                    if let Some(ref mut name) = layer.anchor_point {
                                        ui.label("Tag:");
                                        if ui.text_edit_singleline(name).changed() {
                                            prop_changed = true;
                                        }
                                    }
                                });
                                ui.add_space(4.0);
                                ui.separator();
                                ui.add_space(4.0);

                                if is_mask_editing {
                                    let mut prop_changed = false;
                                    // --- PROPERTIES: MASK (Substance 3D Painter Non-Destructive Effect Stack) ---
                                    ui.group(|ui| {
                                        ui.strong("Mask Effect Stack (Non-Destructive):");
                                        ui.horizontal_wrapped(|ui| {
                                            if ui.button("+ Add Fill").on_hover_text("Add procedural noise or generator fill to mask").clicked() {
                                                layer.add_mask_fill_effect_advanced(ProceduralGenerator::PerlinNoise, curv_ref, ao_ref, pos_ref, wn_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Paint").on_hover_text("Add paint modifier to mask").clicked() {
                                                layer.add_mask_paint_effect(curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Flood Fill").on_hover_text("Substance Flood Fill: segment connected components to random grayscale, gradients, or centroid domes").clicked() {
                                                layer.add_mask_flood_fill(crate::flood_fill::FloodFillMode::RandomGrayscale, 0.5, 0.0, false, false, curv_ref, ao_ref, pos_ref, wn_ref);
                                                prop_changed = true;
                                            }
                                            egui::ComboBox::from_id_source("add_generator_quick_menu")
                                                .selected_text("+ Add Generator...")
                                                .show_ui(ui, |ui| {
                                                    if ui.selectable_label(false, "Peeling Paint (Hyperrealistic)").on_hover_text("Authentic mechanical peeling with curled perimeter lips and micro-fractures (No Voronoi cells)").clicked() {
                                                        layer.add_mask_generator_peeling_paint(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Curvature Edge Wear").on_hover_text("Substance convex edge wear & authentic contact chipping").clicked() {
                                                        layer.add_mask_generator_curvature_wear(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Cavity Dirt (AO)").on_hover_text("Substance concave crevice grime & sediment accumulation").clicked() {
                                                        layer.add_mask_generator_cavity_dirt(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Gravity Leaks (-Y)").on_hover_text("Fluid gravity dripping and rust runoffs with capillary meandering").clicked() {
                                                        layer.add_mask_generator_gravity_leaks(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Directional Dust (+Y)").on_hover_text("Sky-facing dust settling into upward concave pockets").clicked() {
                                                        layer.add_mask_generator_directional_dust(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Ground Mud & Splash").on_hover_text("Height-based ground mud accumulation with organic impact splatter").clicked() {
                                                        layer.add_mask_generator_ground_mud(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Mask Editor (Compound)").on_hover_text("Compound mask editor fusing all baked mesh maps").clicked() {
                                                        layer.add_mask_generator_mask_editor(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Directional Light 3D").on_hover_text("3D surface lighting mask calculated from baked World Normal map").clicked() {
                                                        layer.add_mask_generator_light_3d(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Position 3D Gradient").on_hover_text("Linear or radial 3D coordinates mask calculated from baked World Position map").clicked() {
                                                        layer.add_mask_generator_position_3d(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Subsurface Thickness").on_hover_text("Subsurface transmission and thin geometry penetration mask with curvature rim").clicked() {
                                                        layer.add_mask_generator_subsurface_thickness(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                });
                                            if ui.button("+ Color Select").on_hover_text("Isolate submesh or UV island using baked ID Map").clicked() {
                                                layer.add_mask_color_selection_effect([1.0, 0.0, 0.0], 0.1, 0.05);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Anchor").on_hover_text("Reference channel from layer below").clicked() {
                                                let def_name = available_anchors.first().cloned().unwrap_or_else(|| "Anchor".to_string());
                                                layer.add_mask_anchor_effect(def_name, crate::channel::ChannelType::BaseColor);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Blur").on_hover_text("Add gaussian blur filter to mask").clicked() {
                                                layer.add_mask_filter_blur(3.0, curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Levels").on_hover_text("Add histogram levels filter to mask").clicked() {
                                                layer.add_mask_filter_levels(curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Sharpen").on_hover_text("Add sharpen filter to mask").clicked() {
                                                layer.add_mask_filter_sharpen(1.0, curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add High Pass").on_hover_text("Add high-pass filter to mask").clicked() {
                                                layer.add_mask_filter_high_pass(2.0, curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Contrast").on_hover_text("Add contrast & brightness filter to mask").clicked() {
                                                layer.add_mask_filter_contrast(1.2, 0.0, curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Invert").on_hover_text("Add invert filter to mask").clicked() {
                                                layer.add_mask_filter_invert(curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Warp").on_hover_text("Substance Warp filter: organic domain vector distortion and directional wave displacement").clicked() {
                                                layer.add_mask_filter_warp(0.5, 4.0, 0.0, curv_ref);
                                                prop_changed = true;
                                            }

                                            egui::ComboBox::from_id_source("smart_mask_quick_add")
                                                .selected_text("+ Smart Mask...")
                                                .show_ui(ui, |ui| {
                                                    for sm in crate::smart_materials::SmartMaterialLibrary::smart_masks() {
                                                        if ui.selectable_label(false, sm.name).on_hover_text(sm.description).clicked() {
                                                            crate::smart_materials::SmartMaterialLibrary::apply_smart_mask_advanced(
                                                                sm.id,
                                                                layer,
                                                                curv_ref,
                                                                ao_ref,
                                                                pos_ref,
                                                                wn_ref,
                                                            );
                                                            prop_changed = true;
                                                        }
                                                    }
                                                });
                                        });

                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            if ui.small_button("Clear White").clicked() {
                                                layer.clear_mask(1.0);
                                                prop_changed = true;
                                            }
                                            if ui.small_button("Clear Black").clicked() {
                                                layer.clear_mask(0.0);
                                                prop_changed = true;
                                            }
                                            if ui.small_button("Invert Base").clicked() {
                                                layer.invert_mask();
                                                prop_changed = true;
                                            }
                                            if ui.small_button("Remove Mask").clicked() {
                                                layer.remove_mask();
                                                self.brush_engine.settings.target = PaintTarget::Material;
                                                prop_changed = true;
                                            }
                                        });
                                    });

                                    ui.add_space(4.0);

                                    // Render mask effects stack list (top effect evaluates on top)
                                    if layer.mask_effects.is_empty() {
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("No modifiers in mask stack. Direct brush painting or whole-mask actions will apply to base mask buffer. Click '+ Add Fill' to add non-destructive noise.").small().italics().color(Color32::from_rgb(160, 165, 175)));
                                        });
                                    } else {
                                        let eff_count = layer.mask_effects.len();
                                        let mut remove_eff_id = None;
                                        let mut reorder_eff = None;

                                        for rev_idx in 0..eff_count {
                                            let idx = eff_count - 1 - rev_idx;
                                            let eff = &mut layer.mask_effects[idx];
                                            let eff_id = eff.id;

                                            ui.group(|ui| {
                                                ui.horizontal(|ui| {
                                                    if ui.checkbox(&mut eff.visible, "").changed() {
                                                        prop_changed = true;
                                                    }
                                                    ui.strong(&eff.name);

                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        if ui.small_button("X").on_hover_text("Delete effect").clicked() {
                                                            remove_eff_id = Some(eff_id);
                                                        }
                                                        if idx > 0 && ui.small_button("v").on_hover_text("Move down").clicked() {
                                                            reorder_eff = Some((eff_id, false));
                                                        }
                                                        if idx + 1 < eff_count && ui.small_button("^").on_hover_text("Move up").clicked() {
                                                            reorder_eff = Some((eff_id, true));
                                                        }
                                                    });
                                                });

                                                ui.horizontal(|ui| {
                                                    ui.label("Blend:");
                                                    egui::ComboBox::from_id_source(format!("mask_eff_blend_{}", eff_id))
                                                        .selected_text(eff.blend_mode.name())
                                                        .width(90.0)
                                                        .show_ui(ui, |ui| {
                                                            let modes = [
                                                                BlendMode::Normal,
                                                                BlendMode::Multiply,
                                                                BlendMode::Screen,
                                                                BlendMode::Overlay,
                                                                BlendMode::Add,
                                                                BlendMode::Subtract,
                                                                BlendMode::Darken,
                                                                BlendMode::Lighten,
                                                            ];
                                                            for m in modes {
                                                                if ui.selectable_value(&mut eff.blend_mode, m, m.name()).clicked() {
                                                                    prop_changed = true;
                                                                }
                                                            }
                                                        });
                                                    ui.label("Opacity:");
                                                    if ui.add(egui::Slider::new(&mut eff.opacity, 0.0..=1.0)).changed() {
                                                        prop_changed = true;
                                                    }
                                                });

                                                // Parameter controls per effect type
                                                match &mut eff.effect_type {
                                                    crate::layer::MaskEffectType::Fill {
                                                        ref mut generator,
                                                        ref mut scale,
                                                        ref mut contrast,
                                                        ref mut invert,
                                                        ref mut balance,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Generator:");
                                                            egui::ComboBox::from_id_source(format!("mask_eff_gen_{}", eff_id))
                                                                .selected_text(generator.name())
                                                                .show_ui(ui, |ui| {
                                                                    for g in ProceduralGenerator::all() {
                                                                        if ui.selectable_value(generator, *g, g.name()).clicked() {
                                                                            prop_changed = true;
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Scale:");
                                                            if ui.add(egui::Slider::new(scale, 0.5..=64.0).clamp_to_range(false)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Contrast:");
                                                            if ui.add(egui::Slider::new(contrast, 0.1..=4.0)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Balance:");
                                                            if ui.add(egui::Slider::new(balance, 0.0..=1.0)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        if ui.checkbox(invert, "Invert Generator").changed() {
                                                            prop_changed = true;
                                                        }
                                                    }
                                                    crate::layer::MaskEffectType::FilterBlur { ref mut sigma } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Blur Radius:");
                                                            if ui.add(egui::Slider::new(sigma, 0.5..=20.0)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FilterLevels {
                                                        ref mut in_black,
                                                        ref mut in_white,
                                                        ref mut gamma,
                                                        ref mut out_black,
                                                        ref mut out_white,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("In Black:");
                                                            if ui.add(egui::Slider::new(in_black, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("In White:");
                                                            if ui.add(egui::Slider::new(in_white, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Gamma:");
                                                            if ui.add(egui::Slider::new(gamma, 0.1..=5.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Out Black:");
                                                            if ui.add(egui::Slider::new(out_black, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Out White:");
                                                            if ui.add(egui::Slider::new(out_white, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FilterSharpen { ref mut strength } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Sharpen Strength:");
                                                            if ui.add(egui::Slider::new(strength, 0.1..=5.0)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FilterHighPass { ref mut radius } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("High Pass Radius:");
                                                            if ui.add(egui::Slider::new(radius, 0.5..=20.0)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FilterContrast { ref mut contrast, ref mut brightness } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Contrast:");
                                                            if ui.add(egui::Slider::new(contrast, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Brightness:");
                                                            if ui.add(egui::Slider::new(brightness, -1.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FilterWarp { ref mut warp_intensity, ref mut warp_scale, ref mut warp_angle } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Intensity:");
                                                            if ui.add(egui::Slider::new(warp_intensity, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Noise Scale:");
                                                            if ui.add(egui::Slider::new(warp_scale, 0.5..=32.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Angle:");
                                                            if ui.add(egui::Slider::new(warp_angle, 0.0..=360.0).suffix(" deg")).changed() { prop_changed = true; }
                                                        });
                                                        ui.label(egui::RichText::new("Substance Warp: organic domain vector displacement breaking up procedural edges.").small().color(Color32::from_rgb(160, 165, 175)));
                                                    }
                                                    crate::layer::MaskEffectType::FilterInvert => {
                                                        ui.label("Filter: Inverts lower layers in mask stack.");
                                                    }
                                                    crate::layer::MaskEffectType::Paint { ref mut buffer, ref mut alpha } => {
                                                        let is_active = layer.active_mask_effect_id == Some(eff_id);
                                                        ui.horizontal(|ui| {
                                                            if is_active {
                                                                ui.label(egui::RichText::new("Active Paint Target").color(Color32::from_rgb(80, 220, 120)).strong());
                                                            } else if ui.small_button("Set Active Paint Target").clicked() {
                                                                layer.active_mask_effect_id = Some(eff_id);
                                                                prop_changed = true;
                                                            }
                                                            if ui.small_button("Clear Paint").clicked() {
                                                                buffer.data.fill(0.0);
                                                                alpha.data.fill(0.0);
                                                                prop_changed = true;
                                                            }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::ColorSelection { ref mut target_color, ref mut tolerance, ref mut softness } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("ID Color:");
                                                            let mut col_arr = *target_color;
                                                            if ui.color_edit_button_rgb(&mut col_arr).changed() {
                                                                *target_color = col_arr;
                                                                prop_changed = true;
                                                            }
                                                            ui.label(format!("RGB: {:.2}, {:.2}, {:.2}", target_color[0], target_color[1], target_color[2]));
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Tolerance:");
                                                            if ui.add(egui::Slider::new(tolerance, 0.0..=1.0)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Softness:");
                                                            if ui.add(egui::Slider::new(softness, 0.0..=0.5)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        ui.label(egui::RichText::new("Isolates submeshes or UV islands via baked ID Map.").small().color(Color32::from_rgb(160, 165, 175)));
                                                    }
                                                    crate::layer::MaskEffectType::AnchorReference { ref mut anchor_name, ref mut source_channel, ref mut blur, ref mut invert } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Anchor Layer:");
                                                            egui::ComboBox::from_id_source(format!("anchor_name_sel_{}", eff_id))
                                                                .selected_text(if anchor_name.is_empty() { "Select Anchor..." } else { anchor_name.as_str() })
                                                                .show_ui(ui, |ui| {
                                                                    if available_anchors.is_empty() {
                                                                        ui.label(egui::RichText::new("(No anchor points exposed below)").italics().small());
                                                                    } else {
                                                                        for anc in &available_anchors {
                                                                            if ui.selectable_label(anchor_name == anc, anc).clicked() {
                                                                                *anchor_name = anc.clone();
                                                                                prop_changed = true;
                                                                            }
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Source Channel:");
                                                            egui::ComboBox::from_id_source(format!("anchor_ch_sel_{}", eff_id))
                                                                .selected_text(source_channel.name())
                                                                .show_ui(ui, |ui| {
                                                                    let channels = [
                                                                        crate::channel::ChannelType::BaseColor,
                                                                        crate::channel::ChannelType::Roughness,
                                                                        crate::channel::ChannelType::Metallic,
                                                                        crate::channel::ChannelType::Height,
                                                                        crate::channel::ChannelType::Emission,
                                                                        crate::channel::ChannelType::Opacity,
                                                                    ];
                                                                    for ch in channels {
                                                                        if ui.selectable_label(*source_channel == ch, ch.name()).clicked() {
                                                                            *source_channel = ch;
                                                                            prop_changed = true;
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Blur:");
                                                            if ui.add(egui::Slider::new(blur, 0.0..=20.0)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        if ui.checkbox(invert, "Invert Anchor").changed() {
                                                            prop_changed = true;
                                                        }
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorCurvatureWear {
                                                        ref mut wear_level,
                                                        ref mut edge_width,
                                                        ref mut edge_contrast,
                                                        ref mut grunge_amount,
                                                        ref mut grunge_scale,
                                                        ref mut ao_inhibit,
                                                        ref mut invert,
                                                    } => {
                                                        ui.label(egui::RichText::new("Substance Curvature Edge Wear").strong().color(Color32::from_rgb(220, 180, 80)));
                                                        ui.horizontal(|ui| {
                                                            ui.label("Wear Level:");
                                                            if ui.add(egui::Slider::new(wear_level, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Edge Width:");
                                                            if ui.add(egui::Slider::new(edge_width, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Edge Contrast:");
                                                            if ui.add(egui::Slider::new(edge_contrast, 0.5..=5.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Breakup:");
                                                            if ui.add(egui::Slider::new(grunge_amount, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Scale:");
                                                            if ui.add(egui::Slider::new(grunge_scale, 1.0..=64.0).clamp_to_range(false)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("AO Inhibit (Cavities):");
                                                            if ui.add(egui::Slider::new(ao_inhibit, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        if ui.checkbox(invert, "Invert Wear").changed() { prop_changed = true; }
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorCavityDirt {
                                                        ref mut dirt_level,
                                                        ref mut dirt_contrast,
                                                        ref mut dirt_reach,
                                                        ref mut grunge_clumping,
                                                        ref mut grunge_scale,
                                                        ref mut invert,
                                                    } => {
                                                        ui.label(egui::RichText::new("Substance Cavity & Crevice Grime (AO)").strong().color(Color32::from_rgb(180, 140, 100)));
                                                        ui.horizontal(|ui| {
                                                            ui.label("Dirt Level:");
                                                            if ui.add(egui::Slider::new(dirt_level, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Dirt Contrast:");
                                                            if ui.add(egui::Slider::new(dirt_contrast, 0.5..=5.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Dirt Reach / Bleed:");
                                                            if ui.add(egui::Slider::new(dirt_reach, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Clumping:");
                                                            if ui.add(egui::Slider::new(grunge_clumping, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Scale:");
                                                            if ui.add(egui::Slider::new(grunge_scale, 1.0..=64.0).clamp_to_range(false)).changed() { prop_changed = true; }
                                                        });
                                                        if ui.checkbox(invert, "Invert Cavity").changed() { prop_changed = true; }
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorDirectionalDust {
                                                        ref mut dust_direction,
                                                        ref mut dust_level,
                                                        ref mut angle_tolerance,
                                                        ref mut cavity_trap,
                                                        ref mut grunge_breakup,
                                                        ref mut grunge_scale,
                                                    } => {
                                                        ui.label(egui::RichText::new("Substance Directional Dust (World Normal)").strong().color(Color32::from_rgb(200, 200, 180)));
                                                        ui.horizontal(|ui| {
                                                            ui.label("Direction:");
                                                            if ui.selectable_label(dust_direction[1] > 0.5, "Up (+Y)").clicked() {
                                                                *dust_direction = [0.0, 1.0, 0.0];
                                                                prop_changed = true;
                                                            }
                                                            if ui.selectable_label(dust_direction[1] < -0.5, "Down (-Y)").clicked() {
                                                                *dust_direction = [0.0, -1.0, 0.0];
                                                                prop_changed = true;
                                                            }
                                                            if ui.selectable_label(dust_direction[2] > 0.5, "Forward (+Z)").clicked() {
                                                                *dust_direction = [0.0, 0.0, 1.0];
                                                                prop_changed = true;
                                                            }
                                                            if ui.selectable_label(dust_direction[0] > 0.5, "Right (+X)").clicked() {
                                                                *dust_direction = [1.0, 0.0, 0.0];
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Dust Level:");
                                                            if ui.add(egui::Slider::new(dust_level, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Angle Spread:");
                                                            if ui.add(egui::Slider::new(angle_tolerance, 0.1..=2.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Cavity Trap:");
                                                            if ui.add(egui::Slider::new(cavity_trap, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Breakup:");
                                                            if ui.add(egui::Slider::new(grunge_breakup, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Scale:");
                                                            if ui.add(egui::Slider::new(grunge_scale, 1.0..=64.0).clamp_to_range(false)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorGroundDirt {
                                                        ref mut bottom_level,
                                                        ref mut gradient_softness,
                                                        ref mut splash_amount,
                                                        ref mut splash_scale,
                                                    } => {
                                                        ui.label(egui::RichText::new("Substance Ground Mud & Splash (Position)").strong().color(Color32::from_rgb(170, 130, 90)));
                                                        ui.horizontal(|ui| {
                                                            ui.label("Bottom Height Level:");
                                                            if ui.add(egui::Slider::new(bottom_level, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Gradient Softness:");
                                                            if ui.add(egui::Slider::new(gradient_softness, 0.01..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Splash Amount:");
                                                            if ui.add(egui::Slider::new(splash_amount, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Splash Scale:");
                                                            if ui.add(egui::Slider::new(splash_scale, 1.0..=64.0).clamp_to_range(false)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorMaskEditor {
                                                        ref mut curv_weight,
                                                        ref mut curv_width,
                                                        ref mut curv_contrast,
                                                        ref mut curv_invert,
                                                        ref mut ao_weight,
                                                        ref mut ao_contrast,
                                                        ref mut world_normal_weight,
                                                        ref mut world_normal_dir,
                                                        ref mut pos_weight,
                                                        ref mut pos_height,
                                                        ref mut grunge_weight,
                                                        ref mut grunge_scale,
                                                        ref mut global_balance,
                                                        ref mut global_contrast,
                                                        ref mut invert,
                                                    } => {
                                                        ui.label(egui::RichText::new("Substance Painter Mask Editor (Compound)").strong().color(Color32::from_rgb(200, 160, 240)));
                                                        ui.collapsing("Curvature Parameters", |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Weight:");
                                                                if ui.add(egui::Slider::new(curv_weight, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Edge Width:");
                                                                if ui.add(egui::Slider::new(curv_width, 0.1..=4.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Contrast:");
                                                                if ui.add(egui::Slider::new(curv_contrast, 0.5..=5.0)).changed() { prop_changed = true; }
                                                            });
                                                            if ui.checkbox(curv_invert, "Invert Curvature (Cavities)").changed() { prop_changed = true; }
                                                        });
                                                        ui.collapsing("Ambient Occlusion Parameters", |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Weight:");
                                                                if ui.add(egui::Slider::new(ao_weight, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Contrast:");
                                                                if ui.add(egui::Slider::new(ao_contrast, 0.5..=5.0)).changed() { prop_changed = true; }
                                                            });
                                                        });
                                                        ui.collapsing("World Normal Direction", |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Weight:");
                                                                if ui.add(egui::Slider::new(world_normal_weight, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Preset:");
                                                                if ui.selectable_label(world_normal_dir[1] > 0.5, "Up (+Y)").clicked() {
                                                                    *world_normal_dir = [0.0, 1.0, 0.0];
                                                                    prop_changed = true;
                                                                }
                                                                if ui.selectable_label(world_normal_dir[1] < -0.5, "Down (-Y)").clicked() {
                                                                    *world_normal_dir = [0.0, -1.0, 0.0];
                                                                    prop_changed = true;
                                                                }
                                                                if ui.selectable_label(world_normal_dir[2] > 0.5, "Forward (+Z)").clicked() {
                                                                    *world_normal_dir = [0.0, 0.0, 1.0];
                                                                    prop_changed = true;
                                                                }
                                                            });
                                                        });
                                                        ui.collapsing("Position Height", |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Weight:");
                                                                if ui.add(egui::Slider::new(pos_weight, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Target Height:");
                                                                if ui.add(egui::Slider::new(pos_height, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                        });
                                                        ui.collapsing("Grunge Modulation", |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Grunge Weight:");
                                                                if ui.add(egui::Slider::new(grunge_weight, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Grunge Scale:");
                                                                if ui.add(egui::Slider::new(grunge_scale, 1.0..=64.0).clamp_to_range(false)).changed() { prop_changed = true; }
                                                            });
                                                        });
                                                        ui.separator();
                                                        ui.horizontal(|ui| {
                                                            ui.label("Global Balance:");
                                                            if ui.add(egui::Slider::new(global_balance, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Global Contrast:");
                                                            if ui.add(egui::Slider::new(global_contrast, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        if ui.checkbox(invert, "Global Invert").changed() { prop_changed = true; }
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorPeelingPaint {
                                                        ref mut peeling_amount,
                                                        ref mut flake_scale,
                                                        ref mut edge_curl_lip,
                                                        ref mut micro_fracture,
                                                        ref mut substrate_exposure,
                                                        ref mut invert,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Peeling Level:");
                                                            if ui.add(egui::Slider::new(peeling_amount, 0.01..=0.99)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Flake Scale:");
                                                            if ui.add(egui::Slider::new(flake_scale, 1.0..=32.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Edge Curl Lip:");
                                                            if ui.add(egui::Slider::new(edge_curl_lip, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Micro-Fracture Detail:");
                                                            if ui.add(egui::Slider::new(micro_fracture, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Substrate Exposure:");
                                                            if ui.add(egui::Slider::new(substrate_exposure, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        if ui.checkbox(invert, "Invert Peeling").changed() { prop_changed = true; }
                                                        ui.label(egui::RichText::new("Physically authentic adhesive detachment with curled perimeter lips and micro-fractures (No Voronoi cells).").small().color(Color32::from_rgb(160, 165, 175)));
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorGravityLeaks {
                                                        ref mut drip_intensity,
                                                        ref mut drip_scale,
                                                        ref mut streak_length,
                                                        ref mut meander_amount,
                                                        ref mut ao_source_boost,
                                                        gravity_dir: _,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Drip Intensity:");
                                                            if ui.add(egui::Slider::new(drip_intensity, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Drip Scale:");
                                                            if ui.add(egui::Slider::new(drip_scale, 1.0..=32.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Streak Length:");
                                                            if ui.add(egui::Slider::new(streak_length, 0.1..=3.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Meander (Capillary):");
                                                            if ui.add(egui::Slider::new(meander_amount, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("AO Crevice Boost:");
                                                            if ui.add(egui::Slider::new(ao_source_boost, 0.0..=2.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.label(egui::RichText::new("Fluid gravity runoffs originating from concave crevices with lateral wandering.").small().color(Color32::from_rgb(160, 165, 175)));
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorGroundMud {
                                                        ref mut mud_height,
                                                        ref mut height_falloff,
                                                        ref mut splash_intensity,
                                                        ref mut splash_scale,
                                                        ref mut splash_reach,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Mud Height:");
                                                            if ui.add(egui::Slider::new(mud_height, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Height Falloff:");
                                                            if ui.add(egui::Slider::new(height_falloff, 0.05..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Splash Intensity:");
                                                            if ui.add(egui::Slider::new(splash_intensity, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Splash Scale:");
                                                            if ui.add(egui::Slider::new(splash_scale, 1.0..=32.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Splash Reach:");
                                                            if ui.add(egui::Slider::new(splash_reach, 0.05..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FloodFill {
                                                        ref mut mode,
                                                        ref mut threshold,
                                                        ref mut invert_input,
                                                        ref mut gradient_angle,
                                                        ref mut random_angle,
                                                        ref mut seed,
                                                        ref mut use_uv_islands,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Output Mode:");
                                                            egui::ComboBox::from_id_source(format!("flood_fill_mode_{}", eff_id))
                                                                .selected_text(mode.name())
                                                                .show_ui(ui, |ui| {
                                                                    for m in crate::flood_fill::FloodFillMode::all() {
                                                                        if ui.selectable_value(mode, *m, m.name()).clicked() {
                                                                            prop_changed = true;
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Island Source:");
                                                            let source_name = if *use_uv_islands { "3D Mesh UV Shells" } else { "Mask Threshold" };
                                                            egui::ComboBox::from_id_source(format!("flood_fill_source_{}", eff_id))
                                                                .selected_text(source_name)
                                                                .show_ui(ui, |ui| {
                                                                    if ui.selectable_label(!*use_uv_islands, "Mask Threshold (Input Mask)").clicked() {
                                                                        *use_uv_islands = false;
                                                                        prop_changed = true;
                                                                    }
                                                                    if ui.selectable_label(*use_uv_islands, "3D Mesh UV Shells").clicked() {
                                                                        *use_uv_islands = true;
                                                                        prop_changed = true;
                                                                    }
                                                                });
                                                        });
                                                        if !*use_uv_islands {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Island Threshold:");
                                                                if ui.add(egui::Slider::new(threshold, 0.01..=0.99)).changed() { prop_changed = true; }
                                                            });
                                                            if ui.checkbox(invert_input, "Invert Threshold Input").changed() {
                                                                prop_changed = true;
                                                            }
                                                        }
                                                        if *mode == crate::flood_fill::FloodFillMode::AngleGradient {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Gradient Angle:");
                                                                if ui.add(egui::Slider::new(gradient_angle, 0.0..=360.0).suffix(" deg")).changed() { prop_changed = true; }
                                                            });
                                                            if ui.checkbox(random_angle, "Random Island Angle").changed() {
                                                                prop_changed = true;
                                                            }
                                                        }
                                                        ui.horizontal(|ui| {
                                                            ui.label("Random Seed:");
                                                            if ui.add(egui::DragValue::new(seed).range(0..=9999)).changed() { prop_changed = true; }
                                                        });
                                                        ui.label(egui::RichText::new("Substance Flood Fill: segments islands into unique random values, gradients, or centroid domes.").small().color(Color32::from_rgb(160, 165, 175)));
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorLight3D {
                                                        ref mut light_direction,
                                                        ref mut angular_wrap,
                                                        ref mut contrast,
                                                        ref mut grunge_breakup,
                                                        ref mut grunge_scale,
                                                        ref mut invert,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Light Dir X:");
                                                            if ui.add(egui::Slider::new(&mut light_direction[0], -1.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Light Dir Y:");
                                                            if ui.add(egui::Slider::new(&mut light_direction[1], -1.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Light Dir Z:");
                                                            if ui.add(egui::Slider::new(&mut light_direction[2], -1.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Angular Wrap:");
                                                            if ui.add(egui::Slider::new(angular_wrap, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Contrast:");
                                                            if ui.add(egui::Slider::new(contrast, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Breakup:");
                                                            if ui.add(egui::Slider::new(grunge_breakup, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Scale:");
                                                            if ui.add(egui::Slider::new(grunge_scale, 1.0..=32.0)).changed() { prop_changed = true; }
                                                        });
                                                        if ui.checkbox(invert, "Invert Light Mask").changed() { prop_changed = true; }
                                                        ui.label(egui::RichText::new("3D directional light projection using baked world normals with wrap falloff.").small().color(Color32::from_rgb(160, 165, 175)));
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorPosition3D {
                                                        ref mut axis,
                                                        ref mut offset,
                                                        ref mut spread,
                                                        ref mut contrast,
                                                        ref mut grunge_breakup,
                                                        ref mut grunge_scale,
                                                        ref mut invert,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Axis:");
                                                            let axis_name = match *axis {
                                                                0 => "X (Lateral)",
                                                                1 => "Y (Vertical)",
                                                                2 => "Z (Depth)",
                                                                _ => "Radial (Center-Out)",
                                                            };
                                                            egui::ComboBox::from_id_source(format!("pos3d_axis_{}", eff_id))
                                                                .selected_text(axis_name)
                                                                .show_ui(ui, |ui| {
                                                                    if ui.selectable_value(axis, 0, "X (Lateral)").clicked() { prop_changed = true; }
                                                                    if ui.selectable_value(axis, 1, "Y (Vertical)").clicked() { prop_changed = true; }
                                                                    if ui.selectable_value(axis, 2, "Z (Depth)").clicked() { prop_changed = true; }
                                                                    if ui.selectable_value(axis, 3, "Radial (Center-Out)").clicked() { prop_changed = true; }
                                                                });
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Offset:");
                                                            if ui.add(egui::Slider::new(offset, -1.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Spread:");
                                                            if ui.add(egui::Slider::new(spread, 0.05..=2.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Contrast:");
                                                            if ui.add(egui::Slider::new(contrast, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Breakup:");
                                                            if ui.add(egui::Slider::new(grunge_breakup, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Scale:");
                                                            if ui.add(egui::Slider::new(grunge_scale, 1.0..=32.0)).changed() { prop_changed = true; }
                                                        });
                                                        if ui.checkbox(invert, "Invert Position Gradient").changed() { prop_changed = true; }
                                                        ui.label(egui::RichText::new("3D coordinate gradient mask along world axes or radial distance.").small().color(Color32::from_rgb(160, 165, 175)));
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorSubsurfaceThickness {
                                                        ref mut thickness_reach,
                                                        ref mut contrast,
                                                        ref mut rim_accent,
                                                        ref mut grunge_breakup,
                                                        ref mut grunge_scale,
                                                        ref mut invert,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Thickness Reach:");
                                                            if ui.add(egui::Slider::new(thickness_reach, 0.05..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Contrast:");
                                                            if ui.add(egui::Slider::new(contrast, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Rim Accent:");
                                                            if ui.add(egui::Slider::new(rim_accent, 0.0..=2.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Breakup:");
                                                            if ui.add(egui::Slider::new(grunge_breakup, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Scale:");
                                                            if ui.add(egui::Slider::new(grunge_scale, 1.0..=32.0)).changed() { prop_changed = true; }
                                                        });
                                                        if ui.checkbox(invert, "Invert Thickness").changed() { prop_changed = true; }
                                                        ui.label(egui::RichText::new("Subsurface light diffusion through thin geometric sections and rim highlights.").small().color(Color32::from_rgb(160, 165, 175)));
                                                    }
                                                }
                                            });
                                        }

                                        if let Some(id) = remove_eff_id {
                                            layer.remove_mask_effect(id, curv_ref);
                                            layer.evaluate_mask_full_with_mesh(curv_ref, ao_ref, pos_ref, wn_ref, id_ref, &anchor_refs, Some(&self.project.mesh));
                                            prop_changed = true;
                                        }
                                        if let Some((id, up)) = reorder_eff {
                                            layer.reorder_mask_effect(id, up, curv_ref);
                                            layer.evaluate_mask_full_with_mesh(curv_ref, ao_ref, pos_ref, wn_ref, id_ref, &anchor_refs, Some(&self.project.mesh));
                                            prop_changed = true;
                                        }
                                    }

                                    // Mask Brush Parameters
                                    ui.group(|ui| {
                                        ui.label("Mask Brush Settings:");
                                        ui.horizontal(|ui| {
                                            ui.label("Paint Value:");
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.mask_paint_value, 0.0..=1.0));
                                            if ui.button("Swap [X]").clicked() {
                                                self.brush_engine.settings.mask_paint_value =
                                                    if self.brush_engine.settings.mask_paint_value > 0.5 { 0.0 } else { 1.0 };
                                            }
                                        });
                                        ui.label(if self.brush_engine.settings.mask_paint_value > 0.5 {
                                            "White (Reveals Layer)"
                                        } else {
                                            "Black (Hides Layer)"
                                        });
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.radius, 1.0..=160.0).text("Size [ / ]"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.hardness, 0.0..=1.0).text("Hardness"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.opacity, 0.0..=1.0).text("Opacity"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.flow, 0.0..=1.0).text("Flow"));
                                    });
                                    if prop_changed {
                                        mask_changed = true;
                                    }
                                } else if layer.layer_type == LayerType::Fill {
                                    // --- PROPERTIES: FILL (PBR Material Maps Manipulation 1:1) ---
    
                                    // 1. Transformation (UV Wrap & Tri-Planar)
                                    ui.collapsing("Projection & Transformation", |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label("Projection:");
                                            let proj_label = if layer.triplanar { "Tri-Planar (World 3D)" } else { "UV Projection" };
                                            egui::ComboBox::from_id_source(format!("fill_proj_mode_{}", layer.id))
                                                .selected_text(proj_label)
                                                .show_ui(ui, |ui| {
                                                    if ui.selectable_label(!layer.triplanar, "UV Projection").clicked() {
                                                        layer.triplanar = false;
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(layer.triplanar, "Tri-Planar (World 3D)").on_hover_text("Substance Tri-Planar: 3-axis box projection eliminating UV seams on 3D geometry").clicked() {
                                                        layer.triplanar = true;
                                                        prop_changed = true;
                                                    }
                                                });
                                        });
                                        if layer.triplanar {
                                            ui.horizontal(|ui| {
                                                ui.label("Blend Hardness:");
                                                if ui.add(egui::Slider::new(&mut layer.triplanar_hardness, 1.0..=16.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                        }
                                        ui.horizontal(|ui| {
                                            ui.label("Scale / Tiling:");
                                            if ui.add(egui::Slider::new(&mut layer.uv_scale, 0.2..=32.0).logarithmic(true).clamp_to_range(false)).changed() {
                                                prop_changed = true;
                                            }
                                        });
                                        ui.horizontal(|ui| {
                                            if ui.button("1x").clicked() { layer.uv_scale = 1.0; prop_changed = true; }
                                            if ui.button("2x").clicked() { layer.uv_scale = 2.0; prop_changed = true; }
                                            if ui.button("4x").clicked() { layer.uv_scale = 4.0; prop_changed = true; }
                                            if ui.button("8x").clicked() { layer.uv_scale = 8.0; prop_changed = true; }
                                            if ui.button("16x").clicked() { layer.uv_scale = 16.0; prop_changed = true; }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("UV Offset:");
                                            if ui.add(egui::Slider::new(&mut layer.uv_offset.x, 0.0..=1.0).text("U")).changed() { prop_changed = true; }
                                            if ui.add(egui::Slider::new(&mut layer.uv_offset.y, 0.0..=1.0).text("V")).changed() { prop_changed = true; }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("UV Rotation:");
                                            if ui.add(egui::Slider::new(&mut layer.uv_rotation, 0.0..=360.0).suffix("°")).changed() { prop_changed = true; }
                                        });
                                    });
    
                                    // 2. Material Channel Toggles (Substance Painter iconic pill buttons)
                                    ui.group(|ui| {
                                        ui.label("Material Channels:");
                                        ui.horizontal_wrapped(|ui| {
                                            if channel_pill_button(ui, "color", layer.channel_flags.base_color).clicked() {
                                                layer.channel_flags.base_color = !layer.channel_flags.base_color;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "metal", layer.channel_flags.metallic).clicked() {
                                                layer.channel_flags.metallic = !layer.channel_flags.metallic;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "rough", layer.channel_flags.roughness).clicked() {
                                                layer.channel_flags.roughness = !layer.channel_flags.roughness;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "nrm", layer.channel_flags.normal).clicked() {
                                                layer.channel_flags.normal = !layer.channel_flags.normal;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "height", layer.channel_flags.height).clicked() {
                                                layer.channel_flags.height = !layer.channel_flags.height;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "emis", layer.channel_flags.emission).clicked() {
                                                layer.channel_flags.emission = !layer.channel_flags.emission;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "opac", layer.channel_flags.opacity).clicked() {
                                                layer.channel_flags.opacity = !layer.channel_flags.opacity;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "coat", layer.channel_flags.clearcoat).clicked() {
                                                layer.channel_flags.clearcoat = !layer.channel_flags.clearcoat;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "aniso", layer.channel_flags.anisotropy).clicked() {
                                                layer.channel_flags.anisotropy = !layer.channel_flags.anisotropy;
                                                prop_changed = true;
                                            }
                                        });
                                    });
    
                                    // 3. Channel Value Controllers (PBR Maps)
                                    ui.group(|ui| {
                                        if layer.channel_flags.base_color {
                                            ui.horizontal(|ui| {
                                                ui.label("Base Color:");
                                                if crate::color_picker::color_picker_button_vec4(
                                                    ui,
                                                    egui::Id::new(format!("layer_base_col_{}", layer.id)),
                                                    &mut layer.fill_color,
                                                ) {
                                                    prop_changed = true;
                                                }
                                                ui.label(crate::color_picker::rgb_to_hex(layer.fill_color.x, layer.fill_color.y, layer.fill_color.z));
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_color_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Texture Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace texture from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillBaseColor);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillBaseColor));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_color_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick texture or baked map from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillBaseColor);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import texture file into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillBaseColor));
                                                        }
                                                    }
                                                }
                                            });
                                        }
    
                                        if layer.channel_flags.metallic {
                                            ui.horizontal(|ui| {
                                                ui.label("Metallic:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_metallic, 0.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if ui.small_button("0.0 Dielectric").clicked() { layer.fill_metallic = 0.0; prop_changed = true; }
                                                if ui.small_button("0.5 Semi").clicked() { layer.fill_metallic = 0.5; prop_changed = true; }
                                                if ui.small_button("1.0 Metal").clicked() { layer.fill_metallic = 1.0; prop_changed = true; }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_metallic_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Metallic Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace metallic map from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillMetallic);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import metallic texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillMetallic));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_metallic_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick metallic map or texture from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillMetallic);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import metallic texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillMetallic));
                                                        }
                                                    }
                                                }
                                            });
                                        }
    
                                        if layer.channel_flags.roughness {
                                            ui.horizontal(|ui| {
                                                ui.label("Roughness:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_roughness, 0.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if ui.small_button("0.05 Gloss").clicked() { layer.fill_roughness = 0.05; prop_changed = true; }
                                                if ui.small_button("0.40 Satin").clicked() { layer.fill_roughness = 0.40; prop_changed = true; }
                                                if ui.small_button("0.85 Matte").clicked() { layer.fill_roughness = 0.85; prop_changed = true; }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_roughness_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Roughness Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace roughness map from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillRoughness);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import roughness texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillRoughness));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_roughness_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick roughness map or texture from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillRoughness);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import roughness texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillRoughness));
                                                        }
                                                    }
                                                }
                                            });
                                        }
    
                                        if layer.channel_flags.normal {
                                            ui.horizontal(|ui| {
                                                ui.label("Normal Intensity:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_normal_strength, 0.0..=2.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_normal_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Normal Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace normal map from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillNormal);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import normal map into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillNormal));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_normal_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick normal map from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillNormal);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import normal map into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillNormal));
                                                        }
                                                    }
                                                }
                                            });
                                        }
    
                                        if layer.channel_flags.height {
                                            ui.horizontal(|ui| {
                                                ui.label("Height / Bump:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_height, 0.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_height_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Height Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace height map from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillHeight);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import height texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillHeight));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_height_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick height texture or baked map from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillHeight);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import height texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillHeight));
                                                        }
                                                    }
                                                }
                                            });
                                        }

                                        if layer.channel_flags.emission {
                                            ui.horizontal(|ui| {
                                                ui.label("Emission:");
                                                if crate::color_picker::color_picker_button_vec4(
                                                    ui,
                                                    egui::Id::new(format!("layer_emissive_col_{}", layer.id)),
                                                    &mut layer.fill_emissive,
                                                ) {
                                                    prop_changed = true;
                                                }
                                                ui.label(crate::color_picker::rgb_to_hex(layer.fill_emissive.x, layer.fill_emissive.y, layer.fill_emissive.z));
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Emission Intensity:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_emissive.w, 0.0..=10.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_emissive_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Emissive Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace emissive texture from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillEmissive);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import emissive texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillEmissive));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_emissive_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick emissive texture from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillEmissive);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import emissive texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillEmissive));
                                                        }
                                                    }
                                                }
                                            });
                                        }

                                        if layer.channel_flags.opacity {
                                            ui.horizontal(|ui| {
                                                ui.label("Opacity / Alpha:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_opacity, 0.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                                if ui.small_button("Hole (0%)").clicked() {
                                                    layer.fill_opacity = 0.0;
                                                    prop_changed = true;
                                                }
                                                if ui.small_button("Glass (50%)").clicked() {
                                                    layer.fill_opacity = 0.5;
                                                    prop_changed = true;
                                                }
                                                if ui.small_button("Solid (100%)").clicked() {
                                                    layer.fill_opacity = 1.0;
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_opacity_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Opacity Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace opacity map from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillOpacity);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import opacity map into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillOpacity));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_opacity_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick opacity map from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillOpacity);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import opacity map into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillOpacity));
                                                        }
                                                    }
                                                }
                                            });
                                        }

                                        if layer.channel_flags.clearcoat {
                                            ui.horizontal(|ui| {
                                                ui.label("Clearcoat:");
                                                let mut cc = layer.fill_clearcoat;
                                                if ui.add(egui::Slider::new(&mut cc, 0.0..=1.0)).changed() {
                                                    layer.set_fill_clearcoat(cc);
                                                    prop_changed = true;
                                                }
                                            });
                                        }

                                        if layer.channel_flags.anisotropy {
                                            ui.horizontal(|ui| {
                                                ui.label("Anisotropy Strength:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_anisotropy, -1.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Anisotropy Angle:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_anisotropy_angle, 0.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                        }
                                    });
    
                                    // 4. Procedural Generator / Texture Pattern
                                    ui.group(|ui| {
                                        ui.label("Procedural Generator / Pattern:");
                                        ui.horizontal(|ui| {
                                            egui::ComboBox::from_id_source(format!("prop_gen_{}", layer.id))
                                                .selected_text(layer.fill_generator.name())
                                                .show_ui(ui, |ui| {
                                                    if ui.selectable_value(&mut layer.fill_generator, ProceduralGenerator::None, ProceduralGenerator::None.name()).clicked() {
                                                        prop_changed = true;
                                                    }
                                                    for g in ProceduralGenerator::all() {
                                                        if ui.selectable_value(&mut layer.fill_generator, *g, g.name()).clicked() {
                                                            prop_changed = true;
                                                        }
                                                    }
                                                });
                                        });

                                        if ui.checkbox(&mut layer.triplanar, "Triplanar Projection (Seamless)").on_hover_text("Projects noise across 3D object space to eliminate UV seams").changed() {
                                            prop_changed = true;
                                        }
    
                                        if layer.fill_generator != ProceduralGenerator::None {
                                            ui.horizontal(|ui| {
                                                ui.label("Pattern Scale:");
                                                if ui.add(egui::Slider::new(&mut layer.generator_scale, 1.0..=64.0).clamp_to_range(false)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Contrast:");
                                                if ui.add(egui::Slider::new(&mut layer.generator_contrast, 0.1..=4.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            if ui.checkbox(&mut layer.generator_invert, "Invert Pattern").changed() {
                                                prop_changed = true;
                                            }
                                        }
                                    });
                                } else if layer.layer_type == LayerType::Folder {
                                    // --- PROPERTIES: FOLDER / GROUP ---
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new("Folder / Group Properties").strong().color(Color32::from_rgb(243, 94, 24)));
                                        ui.horizontal(|ui| {
                                            ui.label("Folder Opacity:");
                                            if ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0)).changed() {
                                                prop_changed = true;
                                            }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("Collapsed:");
                                            if ui.checkbox(&mut layer.folder_collapsed, "Collapse contents in stack").changed() {
                                                prop_changed = true;
                                            }
                                        });
                                        ui.separator();
                                        ui.label("Folder Masking (Affects all contained layers):");
                                        if layer.has_mask {
                                            ui.horizontal(|ui| {
                                                if ui.button("Invert Mask").clicked() {
                                                    layer.invert_mask();
                                                    prop_changed = true;
                                                }
                                                if ui.button("Clear White").clicked() {
                                                    layer.clear_mask(1.0);
                                                    prop_changed = true;
                                                }
                                                if ui.button("Clear Black").clicked() {
                                                    layer.clear_mask(0.0);
                                                    prop_changed = true;
                                                }
                                                if ui.button("Remove Mask").clicked() {
                                                    layer.remove_mask();
                                                    prop_changed = true;
                                                }
                                            });
                                        } else {
                                            ui.horizontal(|ui| {
                                                if ui.button("+ White Mask").clicked() {
                                                    layer.add_white_mask();
                                                    prop_changed = true;
                                                }
                                                if ui.button("+ Black Mask").clicked() {
                                                    layer.add_black_mask();
                                                    prop_changed = true;
                                                }
                                            });
                                        }
                                    });
                                } else if layer.layer_type == LayerType::Path {
                                    // --- PROPERTIES: NON-DESTRUCTIVE VECTOR PATH LAYER ---
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new("Vector Path Layer (Always Live & Editable)").strong().color(Color32::from_rgb(0, 180, 255)));
                                        ui.label("Control points and decals stay live on this layer. Shift, smooth, reverse, or re-tune PBR anytime.");
                                        ui.separator();

                                        if layer.path_state.is_none() {
                                            layer.path_state = Some(crate::brush::PathToolState::default());
                                        }
                                        let ps = layer.path_state.as_mut().unwrap();

                                        ui.horizontal(|ui| {
                                            ui.label(format!("Paths: {} | Points: {}", ps.total_path_count(), ps.total_point_count()));
                                            if ui.button("+ New Path (N)").on_hover_text("Start a new disconnected path stroke").clicked() {
                                                ps.start_new_path();
                                                prop_changed = true;
                                            }
                                            if ui.button("Reverse").on_hover_text("Reverse stroke point order").clicked() {
                                                ps.reverse();
                                                prop_changed = true;
                                            }
                                            if ui.button("Smooth").on_hover_text("Apply Laplacian smoothing to points").clicked() {
                                                ps.smooth();
                                                prop_changed = true;
                                            }
                                            if ui.button("Clear All").clicked() {
                                                ps.clear();
                                                prop_changed = true;
                                            }
                                        });

                                        if !ps.other_paths.is_empty() {
                                            ui.horizontal(|ui| {
                                                ui.label("Active Path:");
                                                let total = ps.total_path_count();
                                                for p_idx in 0..total {
                                                    let is_cur = p_idx == total - 1;
                                                    let btn_txt = format!("#{}", p_idx + 1);
                                                    let btn = if is_cur {
                                                        egui::Button::new(egui::RichText::new(&btn_txt).color(Color32::from_rgb(255, 230, 60))).fill(Color32::from_rgb(50, 75, 110))
                                                    } else {
                                                        egui::Button::new(&btn_txt)
                                                    };
                                                    if ui.add(btn).clicked() {
                                                        if p_idx < ps.other_paths.len() {
                                                            ps.activate_other_path(p_idx);
                                                            prop_changed = true;
                                                        }
                                                    }
                                                }
                                            });
                                        }

                                        ui.horizontal(|ui| {
                                            if ui.checkbox(&mut ps.closed, "Loop Path").changed() {
                                                prop_changed = true;
                                            }
                                            if ui.checkbox(&mut ps.snap_to_surface, "Snap to Surface").changed() {
                                                prop_changed = true;
                                            }
                                            if ui.checkbox(&mut ps.follow_tangent, "Follow Tangent").changed() {
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Surface Offset:");
                                            if ui.add(egui::Slider::new(&mut ps.surface_offset, -0.05..=0.05)).changed() {
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Shift Along Normal:");
                                            if ui.button("+0.005").clicked() {
                                                ps.shift_along_normal(0.005);
                                                prop_changed = true;
                                            }
                                            if ui.button("-0.005").clicked() {
                                                ps.shift_along_normal(-0.005);
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Shift Position:");
                                            if ui.button("+X").clicked() { ps.shift(glam::Vec3::new(0.05, 0.0, 0.0)); prop_changed = true; }
                                            if ui.button("-X").clicked() { ps.shift(glam::Vec3::new(-0.05, 0.0, 0.0)); prop_changed = true; }
                                            if ui.button("+Y").clicked() { ps.shift(glam::Vec3::new(0.0, 0.05, 0.0)); prop_changed = true; }
                                            if ui.button("-Y").clicked() { ps.shift(glam::Vec3::new(0.0, -0.05, 0.0)); prop_changed = true; }
                                            if ui.button("+Z").clicked() { ps.shift(glam::Vec3::new(0.0, 0.0, 0.05)); prop_changed = true; }
                                            if ui.button("-Z").clicked() { ps.shift(glam::Vec3::new(0.0, 0.0, -0.05)); prop_changed = true; }
                                        });

                                        ui.separator();
                                        ui.label(egui::RichText::new("Stroke & Decal Style").strong());

                                        ui.horizontal(|ui| {
                                            ui.label("Style:");
                                            egui::ComboBox::from_id_source("path_layer_stroke_style")
                                                .selected_text(ps.stroke_style.name())
                                                .show_ui(ui, |ui| {
                                                    for style in &[
                                                        crate::brush::PathStrokeStyle::Continuous,
                                                        crate::brush::PathStrokeStyle::WeldSeam,
                                                        crate::brush::PathStrokeStyle::RopeBraid,
                                                        crate::brush::PathStrokeStyle::StitchSeam,
                                                        crate::brush::PathStrokeStyle::ChainLinks,
                                                        crate::brush::PathStrokeStyle::SmartDecal,
                                                    ] {
                                                        if ui.selectable_value(&mut ps.stroke_style, *style, style.name()).clicked() {
                                                            prop_changed = true;
                                                        }
                                                    }
                                                });
                                        });

                                        if ps.stroke_style == crate::brush::PathStrokeStyle::SmartDecal {
                                            ui.horizontal(|ui| {
                                                ui.label("Decal:");
                                                egui::ComboBox::from_id_source("path_layer_decal_type")
                                                    .selected_text(ps.decal_type.name())
                                                    .show_ui(ui, |ui| {
                                                        for decal in &[
                                                            crate::brush::SmartDecalType::ButtonHole,
                                                            crate::brush::SmartDecalType::PhillipsScrew,
                                                            crate::brush::SmartDecalType::FlatheadScrew,
                                                            crate::brush::SmartDecalType::TorxScrew,
                                                            crate::brush::SmartDecalType::HexBolt,
                                                            crate::brush::SmartDecalType::DomedRivet,
                                                            crate::brush::SmartDecalType::GrommetEyelet,
                                                            crate::brush::SmartDecalType::AirVentLouver,
                                                        ] {
                                                            if ui.selectable_value(&mut ps.decal_type, *decal, decal.name()).clicked() {
                                                                prop_changed = true;
                                                            }
                                                        }
                                                    });
                                            });

                                            ui.horizontal(|ui| {
                                                ui.label("Spacing:");
                                                if ui.add(egui::Slider::new(&mut ps.decal_spacing, 0.2..=5.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                        } else if ps.stroke_style == crate::brush::PathStrokeStyle::WeldSeam {
                                            if ui.checkbox(&mut ps.heat_tint, "HAZ Heat Oxidation Bands").changed() {
                                                prop_changed = true;
                                            }
                                        }

                                        ui.horizontal(|ui| {
                                            ui.label("Height Depth:");
                                            if ui.add(egui::Slider::new(&mut ps.height_depth, -1.0..=1.0)).changed() {
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Taper Start:");
                                            if ui.add(egui::Slider::new(&mut ps.taper_start, 0.0..=1.0)).changed() {
                                                prop_changed = true;
                                            }
                                            ui.label("End:");
                                            if ui.add(egui::Slider::new(&mut ps.taper_end, 0.0..=1.0)).changed() {
                                                prop_changed = true;
                                            }
                                        });

                                        ui.separator();
                                        ui.label(egui::RichText::new("Full PBR Material Channels").strong());

                                        ui.horizontal(|ui| {
                                            ui.label("Radius:");
                                            if ui.add(egui::Slider::new(&mut self.brush_engine.settings.radius, 1.0..=150.0).text("px")).changed() {
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Base Color:");
                                            let mut col = [
                                                self.brush_engine.settings.paint_color.x,
                                                self.brush_engine.settings.paint_color.y,
                                                self.brush_engine.settings.paint_color.z,
                                                self.brush_engine.settings.paint_color.w,
                                            ];
                                            if ui.color_edit_button_rgba_unmultiplied(&mut col).changed() {
                                                self.brush_engine.settings.paint_color = glam::Vec4::new(col[0], col[1], col[2], col[3]);
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Roughness:");
                                            if ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_roughness, 0.0..=1.0)).changed() {
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Metallic:");
                                            if ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_metallic, 0.0..=1.0)).changed() {
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Height:");
                                            if ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_height, 0.0..=1.0)).changed() {
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Emissive:");
                                            let mut em_col = [
                                                self.brush_engine.settings.paint_emissive.x,
                                                self.brush_engine.settings.paint_emissive.y,
                                                self.brush_engine.settings.paint_emissive.z,
                                                self.brush_engine.settings.paint_emissive.w,
                                            ];
                                            if ui.color_edit_button_rgba_unmultiplied(&mut em_col).changed() {
                                                self.brush_engine.settings.paint_emissive = glam::Vec4::new(em_col[0], em_col[1], em_col[2], em_col[3]);
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Clearcoat:");
                                            if ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_clearcoat, 0.0..=1.0)).changed() {
                                                prop_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Anisotropy:");
                                            if ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_anisotropy, -1.0..=1.0)).changed() {
                                                prop_changed = true;
                                            }
                                        });

                                        if prop_changed {
                                            self.brush_engine.settings.path_tool = ps.clone();
                                        }
                                    });
                                } else {
                                    // --- PROPERTIES: PAINT ---
                                    ui.group(|ui| {
                                        ui.label("Brush Parameters:");
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.radius, 1.0..=160.0).text("Size [ / ]"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.hardness, 0.0..=1.0).text("Hardness"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.opacity, 0.0..=1.0).text("Opacity"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.flow, 0.0..=1.0).text("Flow"));
    
                                        ui.horizontal(|ui| {
                                            ui.label("Alpha:");
                                            egui::ComboBox::from_id_source("brush_alpha_select")
                                                .selected_text(self.brush_engine.settings.alpha.name())
                                                .show_ui(ui, |ui| {
                                                    let alphas = [
                                                        AlphaType::SoftRound,
                                                        AlphaType::HardRound,
                                                        AlphaType::NoiseSplatter,
                                                        AlphaType::Chisel,
                                                        AlphaType::Stipple,
                                                        AlphaType::Scratches,
                                                    ];
                                                    for a in alphas {
                                                        ui.selectable_value(&mut self.brush_engine.settings.alpha, a, a.name());
                                                    }
                                                });
                                            if ui.small_button("Shelf...").on_hover_text("Choose Alpha from the Shelf").clicked() {
                                                self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::BrushAlpha));
                                            }
                                        });
    
                                        ui.collapsing("Jitter Dynamics", |ui| {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.size_jitter, 0.0..=1.0).text("Size Jitter"));
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.position_jitter, 0.0..=1.0).text("Scatter"));
                                        });
                                    });
    
                                    ui.group(|ui| {
                                        ui.label("Material Stamp Channels:");
                                        ui.horizontal_wrapped(|ui| {
                                            if channel_pill_button(ui, "color", self.brush_engine.settings.channels.base_color).clicked() {
                                                self.brush_engine.settings.channels.base_color = !self.brush_engine.settings.channels.base_color;
                                            }
                                            if channel_pill_button(ui, "metal", self.brush_engine.settings.channels.metallic).clicked() {
                                                self.brush_engine.settings.channels.metallic = !self.brush_engine.settings.channels.metallic;
                                            }
                                            if channel_pill_button(ui, "rough", self.brush_engine.settings.channels.roughness).clicked() {
                                                self.brush_engine.settings.channels.roughness = !self.brush_engine.settings.channels.roughness;
                                            }
                                            if channel_pill_button(ui, "nrm", self.brush_engine.settings.channels.normal).clicked() {
                                                self.brush_engine.settings.channels.normal = !self.brush_engine.settings.channels.normal;
                                            }
                                            if channel_pill_button(ui, "height", self.brush_engine.settings.channels.height).clicked() {
                                                self.brush_engine.settings.channels.height = !self.brush_engine.settings.channels.height;
                                            }
                                            if channel_pill_button(ui, "emis", self.brush_engine.settings.channels.emission).clicked() {
                                                self.brush_engine.settings.channels.emission = !self.brush_engine.settings.channels.emission;
                                            }
                                            if channel_pill_button(ui, "opac", self.brush_engine.settings.channels.opacity).clicked() {
                                                self.brush_engine.settings.channels.opacity = !self.brush_engine.settings.channels.opacity;
                                            }
                                            if channel_pill_button(ui, "coat", self.brush_engine.settings.channels.clearcoat).clicked() {
                                                self.brush_engine.settings.channels.clearcoat = !self.brush_engine.settings.channels.clearcoat;
                                            }
                                            if channel_pill_button(ui, "aniso", self.brush_engine.settings.channels.anisotropy).clicked() {
                                                self.brush_engine.settings.channels.anisotropy = !self.brush_engine.settings.channels.anisotropy;
                                            }
                                        });

                                        if self.brush_engine.settings.channels.base_color {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label("Base Color:");
                                                crate::color_picker::color_picker_button_vec4(
                                                    ui,
                                                    egui::Id::new("brush_paint_base_color"),
                                                    &mut self.brush_engine.settings.paint_color,
                                                );
                                                ui.label(crate::color_picker::rgb_to_hex(
                                                    self.brush_engine.settings.paint_color.x,
                                                    self.brush_engine.settings.paint_color.y,
                                                    self.brush_engine.settings.paint_color.z,
                                                ));
                                            });
                                        }

                                        if self.brush_engine.settings.channels.roughness {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_roughness, 0.0..=1.0).text("Roughness"));
                                        }
                                        if self.brush_engine.settings.channels.metallic {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_metallic, 0.0..=1.0).text("Metallic"));
                                        }
                                        if self.brush_engine.settings.channels.normal {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_normal_intensity, 0.0..=2.0).text("Normal Intensity"));
                                                if ui.small_button("Reset (1.0)").clicked() {
                                                    self.brush_engine.settings.paint_normal_intensity = 1.0;
                                                }
                                            });
                                        }
                                        if self.brush_engine.settings.channels.height {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_height, 0.0..=1.0).text("Height"));
                                        }
                                        if self.brush_engine.settings.channels.emission {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label("Emission:");
                                                crate::color_picker::color_picker_button_vec4(
                                                    ui,
                                                    egui::Id::new("brush_paint_emissive_color"),
                                                    &mut self.brush_engine.settings.paint_emissive,
                                                );
                                                ui.label(crate::color_picker::rgb_to_hex(
                                                    self.brush_engine.settings.paint_emissive.x,
                                                    self.brush_engine.settings.paint_emissive.y,
                                                    self.brush_engine.settings.paint_emissive.z,
                                                ));
                                            });
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_emissive.w, 0.0..=10.0).text("Emission Intensity"));
                                        }
                                        if self.brush_engine.settings.channels.opacity {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_opacity, 0.0..=1.0).text("Opacity"));
                                                if ui.small_button("Hole (0%)").clicked() {
                                                    self.brush_engine.settings.paint_opacity = 0.0;
                                                }
                                                if ui.small_button("Glass (50%)").clicked() {
                                                    self.brush_engine.settings.paint_opacity = 0.5;
                                                }
                                                if ui.small_button("Solid (100%)").clicked() {
                                                    self.brush_engine.settings.paint_opacity = 1.0;
                                                }
                                            });
                                        }
                                        if self.brush_engine.settings.channels.clearcoat {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_clearcoat, 0.0..=1.0).text("Clearcoat"));
                                        }
                                        if self.brush_engine.settings.channels.anisotropy {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_anisotropy, -1.0..=1.0).text("Anisotropy"));
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_anisotropy_angle, 0.0..=1.0).text("Aniso Angle"));
                                        }
                                    });
                                }
    
                                ui.add_space(8.0);
                                ui.separator();
                                if ui.button("Export Textures... (Ctrl+E)").clicked() {
                                    self.show_export_modal = true;
                                }
                            });
    
                        if prop_changed || mask_changed {
                            let proj = &mut self.project;
                            let curv = proj.curvature_map.as_ref();
                            let pos = proj.position_map.as_ref();
                            let wn = proj.world_space_normal_map.as_ref();
                            let ao = proj.ao_map.as_ref();
                            let id_ref = proj.id_map.as_ref();
                            let (lower, higher) = proj.layers.split_at_mut(act_idx);
                            let anchor_refs: Vec<crate::layer::AnchorLayerRef<'_>> = lower
                                .iter()
                                .filter_map(|l| l.as_anchor_ref())
                                .collect();
                            let layer = &mut higher[0];
                            if prop_changed && layer.layer_type == LayerType::Fill {
                                layer.evaluate_fill_advanced(curv, pos, wn, ao);
                            } else if prop_changed && layer.layer_type == LayerType::Path {
                                if let Some(ps) = &layer.path_state {
                                    self.brush_engine.settings.path_tool = ps.clone();
                                }
                                layer.evaluate_path_layer(&proj.mesh, &mut self.brush_engine);
                            }
                            if mask_changed && layer.has_mask {
                                layer.evaluate_mask_full_with_mesh(curv, ao, pos, wn, id_ref, &anchor_refs, Some(&proj.mesh));
                            }
                            let is_extended_change = layer.channel_flags.clearcoat
                                || layer.channel_flags.emission
                                || layer.channel_flags.opacity
                                || layer.channel_flags.anisotropy;
                            if self.wgpu_viewport.is_some() && ui.input(|i| i.pointer.any_down()) && !is_extended_change {
                                self.project.gpu_sync_needed = true;
                                self.need_cpu_composite_sync = true;
                            } else {
                                self.project.rebuild_composite();
                                self.project.gpu_sync_needed = true;
                                self.update_uv_texture(ui.ctx());
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    wgpu.sync_textures(&mut self.project);
                                    if let Some(preview_id) = self.mask_preview_layer_id {
                                        wgpu.sync_mask_preview_full(&self.project, preview_id);
                                    }
                                }
                            }
                            ui.ctx().request_repaint();
                        }

                        if let Some(target) = open_shelf_picker {
                            self.shelf_picker = Some(ShelfPickerState::new(target));
                        }
                        if let Some((path, target)) = import_to_shelf_and_assign {
                            if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                if let Some(res) = self.project.get_resource(&id).cloned() {
                                    self.assign_resource_to_target(&res, target);
                                    self.refresh_shelf_thumbnails(ui.ctx());
                                }
                            }
                        }
                    }
    }

    /// Render Texture Set Settings and Mesh Maps slots
    pub fn render_mesh_maps_content(&mut self, ui: &mut egui::Ui) {
                        ui.heading("Texture Set Settings");
                        ui.add_space(4.0);

                        ui.group(|ui| {
                            ui.label(format!("Mesh: {}", self.project.mesh.name));
                            ui.label(format!("Triangles: {}", self.project.mesh.indices.len() / 3));
                            ui.label(format!("Working Resolution: {} x {}", self.project.texture_width, self.project.texture_height));

                            ui.horizontal_wrapped(|ui| {
                                ui.label("Change Resolution:");
                                let res_options = [512, 1024, 2048, 4096];
                                egui::ComboBox::from_id_source("texture_set_res_select")
                                    .selected_text(format!("{}x{}", self.new_project_width, self.new_project_height))
                                    .show_ui(ui, |ui| {
                                        for &r in &res_options {
                                            if ui.selectable_label(self.new_project_width == r, format!("{}x{}", r, r)).clicked() {
                                                self.new_project_width = r;
                                                self.new_project_height = r;
                                            }
                                        }
                                    });
                            });
                            if self.new_project_width != self.project.texture_width || self.new_project_height != self.project.texture_height {
                                if ui.button(format!("Resample Textures to {}x{}", self.new_project_width, self.new_project_height)).clicked() {
                                    self.project.resize_textures(self.new_project_width, self.new_project_height);
                                    self.project.rebuild_composite();
                                    self.project.gpu_sync_needed = true;
                                    self.status_message = format!("Resized textures to {}x{}", self.new_project_width, self.new_project_height);
                                }
                            }
                        });

                        ui.add_space(6.0);
                        let bake_btn = if let Some(icons) = self.icons.as_ref() {
                            egui::Button::image_and_text((icons.bake.id(), egui::vec2(15.0, 15.0)), "Bake Mesh Maps...")
                        } else {
                            egui::Button::new("Bake Mesh Maps...")
                        };
                        if ui.add(bake_btn).on_hover_text("Open Baking dialog to bake Normals, AO, and Curvature from geometry").clicked() {
                            self.show_bake_modal = true;
                        }

                        ui.add_space(6.0);
                        ui.separator();
                        ui.heading("Baked Mesh Maps (Geometry)");
                        ui.label(egui::RichText::new("Persistent underlying maps that blend with your layer stack:").small());
                        ui.add_space(4.0);

                        egui::ScrollArea::vertical()
                            .id_source("mesh_maps_scroll")
                            .show(ui, |ui| {
                                // 1. Normal Map (Tangent Space)
                                ui.group(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.strong("Normal (Tangent Space):");
                                        if self.project.world_normal_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select Normal map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapTangentNormal));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import normal map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapTangentNormal);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.world_normal_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.world_normal_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::Normal;
                                            }
                                        }
                                    });
                                });

                                // 2. Ambient Occlusion (AO)
                                ui.group(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.strong("Ambient Occlusion (AO):");
                                        if self.project.ao_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select AO map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapAo));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import AO map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapAo);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.ao_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.ao_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::AmbientOcclusion;
                                            }
                                        }
                                    });
                                });

                                // 3. Curvature Map
                                ui.group(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.strong("Curvature:");
                                        if self.project.curvature_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select Curvature map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapCurvature));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import Curvature map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapCurvature);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.curvature_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.curvature_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::Curvature;
                                            }
                                        }
                                    });
                                });

                                // 4. World Space Normal Map
                                ui.group(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.strong("World Space Normal:");
                                        if self.project.world_space_normal_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select World Space Normal map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapWorldSpaceNormal));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import World Space Normal map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapWorldSpaceNormal);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.world_space_normal_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.world_space_normal_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::WorldNormal;
                                            }
                                        }
                                    });
                                });

                                // 5. Position Map (WPO)
                                ui.group(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.strong("Position (WPO):");
                                        if self.project.position_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select Position map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapPosition));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import Position map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapPosition);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.position_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.position_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::Position;
                                            }
                                        }
                                    });
                                });

                                // 6. Thickness Map
                                ui.group(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.strong("Thickness:");
                                        if self.project.thickness_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select Thickness map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapThickness));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import Thickness map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapThickness);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.thickness_map.is_some() {
                                            if ui.button("Invert").on_hover_text("Invert thickness map (toggle between White=Thick and White=Thin)").clicked() {
                                                if let Some(th) = &mut self.project.thickness_map {
                                                    th.invert();
                                                    self.project.gpu_mesh_maps_sync_needed = true;
                                                    self.project.gpu_sync_needed = true;
                                                    self.project.sync_baked_maps_to_resources();
                                                    self.status_message = "Inverted thickness map".to_string();
                                                }
                                            }
                                            if ui.button("Remove").clicked() {
                                                self.project.thickness_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::Thickness;
                                            }
                                        }
                                    });
                                });

                                // 7. ID Map
                                ui.group(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.strong("ID (Color Mask):");
                                        if self.project.id_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select ID map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapId));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import ID map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapId);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.id_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.id_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                                self.status_message = "Removed ID map".to_string();
                                            }
                                        }
                                    });
                                });

                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Tip: Baked Normal and AO maps are permanently integrated into the PBR shader. Layers and paint strokes seamlessly blend on top of them. Curvature and AO maps also directly power the Smart Edge Wear and Crevice Dirt mask generators.").small().color(egui::Color32::from_rgb(170, 170, 170)));
                            });
    }
}
