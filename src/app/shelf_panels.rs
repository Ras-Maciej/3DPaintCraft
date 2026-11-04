use super::*;
use eframe::egui::{self, Color32};

impl PaintcraftApp {
    pub fn render_shelf_picker_modal(&mut self, ctx: &egui::Context) {
        if self.shelf_picker.is_none() {
            return;
        }

        self.refresh_shelf_thumbnails(ctx);

        let (target, mut search_query, mut selected_category, title) = {
            let p = self.shelf_picker.as_ref().unwrap();
            (p.target, p.search_query.clone(), p.selected_category, p.title())
        };

        let mut close_picker = false;
        let mut picked_resource = None;
        let mut picked_noise = None;
        let mut picked_hdri = None;
        let mut clear_slot = false;
        let mut trigger_import = false;
        let mut is_open = true;

        egui::Window::new(title)
            .open(&mut is_open)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(540.0, 520.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Target:").strong().color(Color32::from_rgb(243, 94, 24)));
                    ui.label(egui::RichText::new(title).italics());
                });
                ui.add_space(4.0);

                // Search input & Import
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut search_query)
                            .hint_text("Search resources...")
                            .desired_width(ui.available_width() - 110.0),
                    );
                    if ui.button("+ Import...").on_hover_text("Import image file directly to Project Shelf and slot").clicked() {
                        trigger_import = true;
                    }
                });
                ui.add_space(4.0);

                // Category filter tabs
                let categories = [
                    (crate::shelf::ShelfCategory::Project, "Project Resources"),
                    (crate::shelf::ShelfCategory::Textures, "Textures"),
                    (crate::shelf::ShelfCategory::Generators, "Noises"),
                    (crate::shelf::ShelfCategory::Environments, "HDRIs"),
                    (crate::shelf::ShelfCategory::All, "All"),
                ];

                ui.horizontal(|ui| {
                    for (cat, label) in categories {
                        let is_sel = selected_category == cat;
                        if ui.selectable_label(is_sel, label).clicked() {
                            selected_category = cat;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Clear Slot").on_hover_text("Unassign texture from this slot").clicked() {
                            clear_slot = true;
                        }
                    });
                });

                ui.separator();

                let query = search_query.to_lowercase();
                let cat = selected_category;

                // Filter project resources
                let project_res: Vec<_> = self.project.resources.iter().filter(|r| {
                    if cat == crate::shelf::ShelfCategory::Generators {
                        return false;
                    }
                    if cat == crate::shelf::ShelfCategory::Environments && r.resource_type != crate::project::ProjectResourceType::Hdri {
                        return false;
                    }
                    if !query.is_empty() && !r.name.to_lowercase().contains(&query) && !r.id.to_lowercase().contains(&query) {
                        return false;
                    }
                    true
                }).collect();

                // Filter procedural noise generators if appropriate
                let all_shelf = crate::shelf::get_shelf_assets();
                let shelf_assets: Vec<_> = all_shelf.iter().copied().filter(|a| {
                    if cat == crate::shelf::ShelfCategory::Project {
                        return false;
                    }
                    if cat == crate::shelf::ShelfCategory::Environments {
                        return false;
                    }
                    if cat != crate::shelf::ShelfCategory::All && a.category != cat {
                        return false;
                    }
                    if !query.is_empty() && !a.name.to_lowercase().contains(&query) && !a.description.to_lowercase().contains(&query) {
                        return false;
                    }
                    true
                }).collect();

                // Filter HDRIs if in Environments or All
                let hdris: Vec<_> = if cat == crate::shelf::ShelfCategory::Environments || cat == crate::shelf::ShelfCategory::All {
                    self.asset_manager.hdris.iter().filter(|h| {
                        if !query.is_empty() && !h.name.to_lowercase().contains(&query) && !h.file_name.to_lowercase().contains(&query) {
                            return false;
                        }
                        true
                    }).collect()
                } else {
                    Vec::new()
                };

                let card_w = 95.0_f32;
                let thumb_w = card_w - 10.0;
                let card_h = thumb_w + 24.0;

                egui::ScrollArea::vertical()
                    .id_source("shelf_picker_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if project_res.is_empty() && shelf_assets.is_empty() && hdris.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(20.0);
                                ui.label(egui::RichText::new("No resources found in current filter.").italics().color(Color32::from_rgb(150, 150, 160)));
                                if ui.button("Import Texture File to Project...").clicked() {
                                    trigger_import = true;
                                }
                            });
                        }

                        // 1. PROJECT RESOURCES (Baked Maps + Imported Textures)
                        if !project_res.is_empty() {
                            ui.label(egui::RichText::new("PROJECT RESOURCES").size(10.5).color(Color32::from_rgb(180, 190, 205)).strong());
                            ui.add_space(2.0);

                            for chunk in project_res.chunks(4) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    for res in chunk {
                                        let (card_rect, card_resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                        let is_hov = card_resp.hovered();

                                        let bg_col = if is_hov { Color32::from_rgb(46, 50, 58) } else { Color32::from_rgb(30, 32, 36) };
                                        let stroke = if is_hov {
                                            egui::Stroke::new(1.5_f32, Color32::from_rgb(243, 94, 24))
                                        } else {
                                            egui::Stroke::new(1.0_f32, Color32::from_rgb(50, 54, 62))
                                        };
                                        ui.painter().rect(card_rect, egui::Rounding::same(4.0), bg_col, stroke);

                                        // Thumbnail
                                        let thumb_rect = egui::Rect::from_min_size(
                                            egui::pos2(card_rect.min.x + 5.0, card_rect.min.y + 5.0),
                                            egui::vec2(thumb_w, thumb_w),
                                        );

                                        if let Some(tex) = self.shelf_thumbnails.get(&res.id) {
                                            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                                            ui.painter().image(tex.id(), thumb_rect, uv, Color32::WHITE);
                                        } else {
                                            ui.painter().rect_filled(thumb_rect, 2.0, Color32::from_rgb(20, 22, 26));
                                            ui.painter().text(thumb_rect.center(), egui::Align2::CENTER_CENTER, "MAP", egui::FontId::proportional(10.0), Color32::LIGHT_GRAY);
                                        }

                                        // Badge
                                        let (badge_txt, badge_col) = if res.is_baked {
                                            ("BAKED", Color32::from_rgb(255, 175, 40))
                                        } else if res.resource_type == crate::project::ProjectResourceType::Hdri {
                                            ("HDRI", Color32::from_rgb(180, 80, 255))
                                        } else {
                                            ("PROJECT", Color32::from_rgb(45, 140, 255))
                                        };
                                        ui.painter().text(
                                            egui::pos2(card_rect.max.x - 3.0, card_rect.min.y + 2.0),
                                            egui::Align2::RIGHT_TOP,
                                            badge_txt,
                                            egui::FontId::proportional(7.5),
                                            badge_col,
                                        );

                                        // Resolution & Name
                                        let dim_txt = format!("{}x{}", res.width, res.height);
                                        ui.painter().text(
                                            egui::pos2(card_rect.center().x, card_rect.max.y - 18.0),
                                            egui::Align2::CENTER_CENTER,
                                            &dim_txt,
                                            egui::FontId::proportional(7.5),
                                            Color32::from_rgb(130, 135, 145),
                                        );
                                        ui.painter().text(
                                            egui::pos2(card_rect.center().x, card_rect.max.y - 8.0),
                                            egui::Align2::CENTER_CENTER,
                                            &res.name,
                                            egui::FontId::proportional(8.5),
                                            Color32::from_rgb(215, 220, 230),
                                        );

                                        let tooltip = format!("{}\nType: {}\nResolution: {}x{}\nClick to assign to target slot", res.name, badge_txt, res.width, res.height);
                                        if card_resp.on_hover_text(tooltip).clicked() {
                                            picked_resource = Some(((*res).clone(), target));
                                        }
                                    }
                                });
                            }
                            ui.add_space(8.0);
                        }

                        // 2. PROCEDURAL NOISES & GENERATORS
                        if !shelf_assets.is_empty() {
                            ui.label(egui::RichText::new("PROCEDURAL GENERATORS & PATTERNS").size(10.5).color(Color32::from_rgb(180, 190, 205)).strong());
                            ui.add_space(2.0);

                            for chunk in shelf_assets.chunks(4) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    for asset in chunk {
                                        let (card_rect, card_resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                        let is_hov = card_resp.hovered();

                                        let bg_col = if is_hov { Color32::from_rgb(46, 50, 58) } else { Color32::from_rgb(30, 32, 36) };
                                        let stroke = if is_hov {
                                            egui::Stroke::new(1.5_f32, Color32::from_rgb(243, 94, 24))
                                        } else {
                                            egui::Stroke::new(1.0_f32, Color32::from_rgb(50, 54, 62))
                                        };
                                        ui.painter().rect(card_rect, egui::Rounding::same(4.0), bg_col, stroke);

                                        let thumb_rect = egui::Rect::from_min_size(
                                            egui::pos2(card_rect.min.x + 5.0, card_rect.min.y + 5.0),
                                            egui::vec2(thumb_w, thumb_w),
                                        );
                                        ui.painter().rect_filled(thumb_rect, 2.0, Color32::from_rgb(24, 26, 32));

                                        let center = thumb_rect.center();
                                        ui.painter().circle_filled(center, 12.0, Color32::from_rgb(asset.preview_color[0], asset.preview_color[1], asset.preview_color[2]));

                                        ui.painter().text(
                                            egui::pos2(card_rect.max.x - 3.0, card_rect.min.y + 2.0),
                                            egui::Align2::RIGHT_TOP,
                                            "NOISE",
                                            egui::FontId::proportional(7.5),
                                            Color32::from_rgb(46, 204, 113),
                                        );

                                        ui.painter().text(
                                            egui::pos2(card_rect.center().x, card_rect.max.y - 8.0),
                                            egui::Align2::CENTER_CENTER,
                                            asset.name,
                                            egui::FontId::proportional(8.5),
                                            Color32::from_rgb(215, 220, 230),
                                        );

                                        let tooltip = format!("{}\n{}\nClick to assign generator to slot", asset.name, asset.description);
                                        if card_resp.on_hover_text(tooltip).clicked() {
                                            let gen = match asset.id {
                                                "gen_worley" => crate::layer::ProceduralGenerator::WorleyGrunge,
                                                "gen_billow" => crate::layer::ProceduralGenerator::BillowNoise,
                                                "gen_ridged" => crate::layer::ProceduralGenerator::RidgedMultifractal,
                                                "gen_domain_warp" => crate::layer::ProceduralGenerator::DomainWarpGrunge,
                                                "gen_splatter" => crate::layer::ProceduralGenerator::SplatterDirt,
                                                "gen_wood" => crate::layer::ProceduralGenerator::WoodGrain,
                                                "gen_fabric" => crate::layer::ProceduralGenerator::FabricWeave,
                                                "gen_scratches" => crate::layer::ProceduralGenerator::Scratches,
                                                "gen_curvature" => crate::layer::ProceduralGenerator::CurvatureWear,
                                                "gen_cavity" => crate::layer::ProceduralGenerator::AmbientOcclusion,
                                                "gen_dust" => crate::layer::ProceduralGenerator::DirectionalDust,
                                                "gen_leaks" => crate::layer::ProceduralGenerator::DrippingLeaks,
                                                "gen_position" => crate::layer::ProceduralGenerator::PositionGradient,
                                                "gen_checker" => crate::layer::ProceduralGenerator::Checkerboard,
                                                "gen_bricks" => crate::layer::ProceduralGenerator::Bricks,
                                                "gen_perforated" => crate::layer::ProceduralGenerator::PerforatedMesh,
                                                "gen_wire_fence" => crate::layer::ProceduralGenerator::WireFence,
                                                "gen_torn_fabric" => crate::layer::ProceduralGenerator::TornFabricEdge,
                                                "gen_dots" => crate::layer::ProceduralGenerator::CircularDots,
                                                "gen_bnw_spots_1" => crate::layer::ProceduralGenerator::BnWSpots1,
                                                "gen_bnw_spots_2" => crate::layer::ProceduralGenerator::BnWSpots2,
                                                "gen_bnw_spots_3" => crate::layer::ProceduralGenerator::BnWSpots3,
                                                "gen_grunge_map" => crate::layer::ProceduralGenerator::GrungeMap,
                                                "gen_directional_noise" => crate::layer::ProceduralGenerator::DirectionalNoise,
                                                "gen_clouds" => crate::layer::ProceduralGenerator::Clouds,
                                                "gen_marble_veins" => crate::layer::ProceduralGenerator::MarbleVeins,
                                                "gen_moisture" => crate::layer::ProceduralGenerator::MoistureNoise,
                                                _ => crate::layer::ProceduralGenerator::PerlinNoise,
                                            };
                                            picked_noise = Some((gen, target));
                                        }
                                    }
                                });
                            }
                            ui.add_space(8.0);
                        }

                        // 3. HDRIs (Environments)
                        if !hdris.is_empty() {
                            ui.label(egui::RichText::new("HDRI ENVIRONMENTS").size(10.5).color(Color32::from_rgb(180, 190, 205)).strong());
                            ui.add_space(2.0);

                            for chunk in hdris.chunks(4) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    for hdri in chunk {
                                        let (card_rect, card_resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                        let is_hov = card_resp.hovered();

                                        let bg_col = if is_hov { Color32::from_rgb(46, 50, 58) } else { Color32::from_rgb(30, 32, 36) };
                                        let stroke = if is_hov {
                                            egui::Stroke::new(1.5_f32, Color32::from_rgb(243, 94, 24))
                                        } else {
                                            egui::Stroke::new(1.0_f32, Color32::from_rgb(50, 54, 62))
                                        };
                                        ui.painter().rect(card_rect, egui::Rounding::same(4.0), bg_col, stroke);

                                        let thumb_rect = egui::Rect::from_min_size(
                                            egui::pos2(card_rect.min.x + 5.0, card_rect.min.y + 5.0),
                                            egui::vec2(thumb_w, thumb_w),
                                        );
                                        draw_hdri_thumbnail(ui.painter(), thumb_rect, &hdri.id, &hdri.name);

                                        ui.painter().text(
                                            egui::pos2(card_rect.max.x - 3.0, card_rect.min.y + 2.0),
                                            egui::Align2::RIGHT_TOP,
                                            if hdri.is_builtin { "BUILTIN" } else { "HDRI" },
                                            egui::FontId::proportional(7.5),
                                            Color32::from_rgb(255, 175, 40),
                                        );

                                        ui.painter().text(
                                            egui::pos2(card_rect.center().x, card_rect.max.y - 8.0),
                                            egui::Align2::CENTER_CENTER,
                                            &hdri.name,
                                            egui::FontId::proportional(8.5),
                                            Color32::from_rgb(215, 220, 230),
                                        );

                                        let tooltip = format!("{}\nFile: {}\nSize: {}\nClick to set active HDRI", hdri.name, hdri.file_name, hdri.size_formatted());
                                        if card_resp.on_hover_text(tooltip).clicked() {
                                            picked_hdri = Some(hdri.path.clone());
                                        }
                                    }
                                });
                            }
                        }
                    });
            });

        if let Some(p) = &mut self.shelf_picker {
            p.search_query = search_query;
            p.selected_category = selected_category;
        }

        if let Some(path) = picked_hdri {
            self.activate_hdri_file(&path);
            close_picker = true;
        }

        if !is_open {
            close_picker = true;
        }

        if trigger_import {
            if let Some(picker) = &self.shelf_picker {
                let target = picker.target;
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Image / Texture", &["png", "jpg", "jpeg", "tga", "exr", "hdr"])
                    .set_title("Import Texture to Project Shelf")
                    .pick_file()
                {
                    if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                        self.refresh_shelf_thumbnails(ctx);
                        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                        if ext == "hdr" || ext == "exr" {
                            let _ = self.asset_manager.import_hdri_file(&path);
                        }
                        if let Some(res) = self.project.get_resource(&id).cloned() {
                            self.assign_resource_to_target(&res, target);
                            close_picker = true;
                        }
                    }
                }
            }
        }

        if clear_slot {
            if let Some(picker) = &self.shelf_picker {
                match picker.target {
                    ShelfPickerTarget::FillBaseColor => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_color_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillRoughness => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_roughness_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillMetallic => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_metallic_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillNormal => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_normal_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillHeight => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_height_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillEmissive => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_emissive_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillOpacity => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_opacity_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::Stencil => {
                        self.brush_engine.stencil_buffer = None;
                    }
                    ShelfPickerTarget::MeshMapTangentNormal => { self.project.world_normal_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapWorldSpaceNormal => { self.project.world_space_normal_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapAo => { self.project.ao_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapCurvature => { self.project.curvature_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapPosition => { self.project.position_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapThickness => { self.project.thickness_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapId => { self.project.id_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    _ => {}
                }
                close_picker = true;
            }
        }

        if let Some((res, target)) = picked_resource {
            self.assign_resource_to_target(&res, target);
            close_picker = true;
        }

        if let Some((gen, target)) = picked_noise {
            self.assign_noise_to_target(gen, target);
            close_picker = true;
        }

        if close_picker {
            self.shelf_picker = None;
        }
    }

    /// Render quick popup when user clicks a project resource directly in the Left Shelf
    pub fn render_shelf_resource_popup(&mut self, ctx: &egui::Context) {
        let mut close_popup = false;
        let mut assign_target = None;

        if let Some(res_id) = &self.shelf_resource_popup {
            if let Some(res) = self.project.get_resource(res_id).cloned() {
                let mut is_open = true;
                egui::Window::new(format!("Resource: {}", res.name))
                    .open(&mut is_open)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(egui::RichText::new(format!("{} ({}x{})", res.name, res.width, res.height)).strong());
                        ui.separator();
                        ui.label("Assign to Active Layer channel:");
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Base Color").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillBaseColor));
                                close_popup = true;
                            }
                            if ui.button("Roughness").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillRoughness));
                                close_popup = true;
                            }
                            if ui.button("Metallic").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillMetallic));
                                close_popup = true;
                            }
                            if ui.button("Normal").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillNormal));
                                close_popup = true;
                            }
                            if ui.button("Height").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillHeight));
                                close_popup = true;
                            }
                            if ui.button("Emissive").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillEmissive));
                                close_popup = true;
                            }
                            if ui.button("Opacity").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillOpacity));
                                close_popup = true;
                            }
                        });
                        ui.add_space(6.0);
                        ui.label("Tools & Environment:");
                        ui.horizontal(|ui| {
                            if ui.button("Set as Stencil (Projection)").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::Stencil));
                                close_popup = true;
                            }
                            if res.resource_type == crate::project::ProjectResourceType::Hdri {
                                if ui.button("Activate Viewport HDRI").clicked() {
                                    assign_target = Some((res.clone(), ShelfPickerTarget::HdriEnvironment));
                                    close_popup = true;
                                }
                            }
                        });
                        ui.add_space(4.0);
                        if !res.is_baked {
                            if ui.button(egui::RichText::new("Remove from Project Shelf").color(Color32::from_rgb(255, 100, 100))).clicked() {
                                self.project.remove_resource(&res.id);
                                self.shelf_thumbnails.remove(&res.id);
                                self.shelf_thumbnails_dirty = true;
                                self.status_message = format!("Removed '{}' from Project Shelf", res.name);
                                close_popup = true;
                            }
                            ui.add_space(2.0);
                        }
                        if ui.small_button("Close").clicked() {
                            close_popup = true;
                        }
                    });
                if !is_open {
                    close_popup = true;
                }
            } else {
                close_popup = true;
            }
        }

        if let Some((res, target)) = assign_target {
            self.assign_resource_to_target(&res, target);
        }

        if close_popup {
            self.shelf_resource_popup = None;
        }
    }


}

impl PaintcraftApp {
    /// Render the Substance 3D Painter Style Left ASSETS Panel (Compact 2-column dock)
    pub(crate) fn render_assets_panel(&mut self, ui: &mut egui::Ui) {
        // Header
        ui.horizontal(|ui| {
            ui.strong(egui::RichText::new("ASSETS").size(13.0).color(Color32::WHITE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("X").on_hover_text("Close Assets Panel").clicked() {
                    self.show_shelf = false;
                    self.workspace.show_shelf = false;
                }
            });
        });
        ui.label(egui::RichText::new("< All libraries").small().color(Color32::from_rgb(140, 145, 155)));
        ui.add_space(2.0);

        // Search Input
        ui.add(
            egui::TextEdit::singleline(&mut self.shelf_search_query)
                .hint_text("Search...")
                .desired_width(ui.available_width() - 4.0),
        );
        ui.add_space(3.0);

        // Ensure thumbnails are cached
        self.refresh_shelf_thumbnails(ui.ctx());

        // Category Filter Pills
        let categories = [
            (crate::shelf::ShelfCategory::All, "All"),
            (crate::shelf::ShelfCategory::Project, "Project"),
            (crate::shelf::ShelfCategory::Textures, "Tex"),
            (crate::shelf::ShelfCategory::Materials, "Mat"),
            (crate::shelf::ShelfCategory::SmartMaterials, "Smart"),
            (crate::shelf::ShelfCategory::SmartMasks, "Mask"),
            (crate::shelf::ShelfCategory::Generators, "Noise"),
            (crate::shelf::ShelfCategory::Alphas, "Alpha"),
            (crate::shelf::ShelfCategory::Decals, "Decal"),
            (crate::shelf::ShelfCategory::Environments, "HDRI"),
        ];

        ui.horizontal_wrapped(|ui| {
            for (cat, label) in categories {
                let is_sel = self.shelf_category == cat;
                if ui.selectable_label(is_sel, label).clicked() {
                    self.shelf_category = cat;
                }
            }
        });

        ui.add_space(2.0);
        ui.separator();

        if self.shelf_category == crate::shelf::ShelfCategory::Project || self.shelf_category == crate::shelf::ShelfCategory::Textures {
            // Project Shelf Actions
            ui.horizontal(|ui| {
                if ui.small_button("+ Import...").on_hover_text("Import texture or image file into project shelf").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images & Textures", &["png", "jpg", "jpeg", "tga", "bmp", "hdr", "exr"])
                        .set_title("Import Texture into Project Shelf")
                        .pick_file() 
                    {
                        match self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                            Ok(_id) => {
                                self.status_message = format!("Imported '{}' into project shelf", path.file_name().unwrap_or_default().to_string_lossy());
                                self.shelf_thumbnails_dirty = true;
                                self.refresh_shelf_thumbnails(ui.ctx());
                            }
                            Err(e) => {
                                self.status_message = format!("Failed to import texture: {}", e);
                            }
                        }
                    }
                }
                if ui.small_button("↻ Sync Maps").on_hover_text("Synchronize baked mesh maps to shelf").clicked() {
                    self.project.sync_baked_maps_to_resources();
                    self.shelf_thumbnails_dirty = true;
                    self.refresh_shelf_thumbnails(ui.ctx());
                    self.status_message = "Synchronized baked maps to shelf".to_string();
                }
            });
        } else if self.shelf_category == crate::shelf::ShelfCategory::Environments {
            // HDRI library actions
            ui.horizontal(|ui| {
                if ui.small_button("+ Import").on_hover_text("Import HDRI image (.hdr / .png / .exr) into library").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("HDRI Environments", &["hdr", "png", "jpg", "jpeg", "exr"])
                        .set_title("Import HDRI to assets/hdris")
                        .pick_file() 
                    {
                        if let Ok(entry) = self.asset_manager.import_hdri_file(&path) {
                            self.activate_hdri_file(&entry.path);
                            self.status_message = format!("Imported and activated HDRI: {}", entry.name);
                        }
                    }
                }
                if ui.small_button("Open Folder").on_hover_text("Open assets/hdris directory in system file manager").clicked() {
                    let _ = self.asset_manager.open_hdri_folder();
                }
                if ui.small_button("↻ Refresh").on_hover_text("Rescan HDRI folder on disk").clicked() {
                    self.asset_manager.refresh_hdris();
                    self.status_message = format!("Refreshed HDRI assets: {} environments ready", self.asset_manager.hdris.len());
                }
            });
        } else if self.shelf_category == crate::shelf::ShelfCategory::Materials {
            // Material library actions
            ui.horizontal(|ui| {
                if ui.small_button("+ New Material").on_hover_text("Create a new custom PBR material").clicked() {
                    self.show_new_material_modal = true;
                    self.new_material_name = format!("Material_{}", self.asset_manager.materials.len() + 1);
                    if let Some(layer) = self.project.active_layer() {
                        self.new_material_color = [layer.fill_color.x, layer.fill_color.y, layer.fill_color.z, layer.fill_color.w];
                        self.new_material_roughness = layer.fill_roughness;
                        self.new_material_metallic = layer.fill_metallic;
                        self.new_material_height = layer.fill_height;
                        self.new_material_emissive = [layer.fill_emissive.x, layer.fill_emissive.y, layer.fill_emissive.z];
                        self.new_material_opacity = layer.fill_opacity;
                        self.new_material_clearcoat = layer.fill_clearcoat;
                        self.new_material_anisotropy = layer.fill_anisotropy;
                    }
                }
                if ui.small_button("Save Layer as Mat").on_hover_text("Save currently selected active layer as a new material").clicked() {
                    if let Some(layer) = self.project.active_layer() {
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
                    } else {
                        self.status_message = "No active layer selected to save as material".to_string();
                    }
                }
                if ui.small_button("Open Folder").on_hover_text("Open assets/materials directory in desktop file manager").clicked() {
                    let _ = self.asset_manager.open_materials_folder();
                }
                if ui.small_button("↻ Refresh").on_hover_text("Rescan materials folder on disk").clicked() {
                    self.asset_manager.refresh_materials();
                    self.status_message = format!("Refreshed materials: {} custom materials ready", self.asset_manager.materials.len());
                }
            });
        } else if self.shelf_category == crate::shelf::ShelfCategory::Decals {
            // Decal preset library actions
            ui.horizontal(|ui| {
                if ui.small_button("+ New Decal").on_hover_text("Create or save a new custom Decal / Path preset").clicked() {
                    self.show_new_decal_modal = true;
                    self.new_decal_name = format!("Decal_{}", self.asset_manager.decals.len() + 1);
                    self.new_decal_description = "Custom hardware decal preset".to_string();
                }
                if ui.small_button("Open Folder").on_hover_text("Open assets/decals directory in file manager").clicked() {
                    let _ = self.asset_manager.open_decals_folder();
                }
                if ui.small_button("↻ Refresh").on_hover_text("Rescan decals folder on disk").clicked() {
                    self.asset_manager.refresh_decals();
                    self.status_message = format!("Refreshed decals: {} custom presets ready", self.asset_manager.decals.len());
                }
            });
        } else {
            // Preset bundle actions
            ui.horizontal(|ui| {
                if ui.small_button("+ .spmt").on_hover_text("Import Smart Material preset (.spmt)").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Smart Material", &["spmt", "json"]).pick_file() {
                        let curv = self.project.curvature_map.clone();
                        if let Ok(new_layers) = crate::smart_materials::SmartMaterialLibrary::load_smart_material_from_file(
                            &path,
                            self.project.texture_width,
                            self.project.texture_height,
                            curv.as_ref(),
                        ) {
                            for mut l in new_layers {
                                l.id = self.project.next_layer_id;
                                self.project.next_layer_id += 1;
                                self.project.layers.push(l);
                            }
                            self.project.active_layer_idx = self.project.layers.len() - 1;
                            self.project.rebuild_composite();
                            self.status_message = format!("Imported Smart Material from {:?}", path.file_name().unwrap_or_default());
                        }
                    }
                }
                if ui.small_button("Save .spmt").on_hover_text("Export current layers as Smart Material (.spmt)").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Smart Material", &["spmt"]).set_file_name("MySmartMaterial.spmt").save_file() {
                        let _ = crate::smart_materials::SmartMaterialLibrary::export_smart_material_to_file(&path, &self.project.layers);
                        self.status_message = format!("Exported Smart Material to {:?}", path.file_name().unwrap_or_default());
                    }
                }
            });
        }

        ui.add_space(2.0);
        ui.separator();

        let all_assets = crate::shelf::get_shelf_assets();
        let query = self.shelf_search_query.to_lowercase();
        let cat = self.shelf_category;

        let filtered_assets: Vec<_> = all_assets
            .iter()
            .copied()
            .filter(|a| {
                if cat != crate::shelf::ShelfCategory::All && a.category != cat {
                    return false;
                }
                if !query.is_empty() && !a.name.to_lowercase().contains(&query) && !a.description.to_lowercase().contains(&query) {
                    return false;
                }
                true
            })
            .collect();

        // Adaptive Grid in Vertical ScrollArea (wide in bottom shelf dock, compact in left dock)
        let avail_w = ui.available_width();
        let card_w = 78.0_f32;
        let thumb_w = card_w - 10.0;
        let card_h = thumb_w + 24.0;
        let cols = ((avail_w - 6.0) / (card_w + 4.0)).floor().max(2.0) as usize;

        if self.shelf_category == crate::shelf::ShelfCategory::Environments {
            let hdri_query = self.shelf_search_query.to_lowercase();
            let filtered_hdris: Vec<_> = self.asset_manager.hdris.iter().filter(|h| {
                if !hdri_query.is_empty() && !h.name.to_lowercase().contains(&hdri_query) && !h.file_name.to_lowercase().contains(&hdri_query) {
                    return false;
                }
                true
            }).collect();

            let mut selected_hdri_path = None;

            egui::ScrollArea::vertical()
                .id_source("assets_panel_hdri_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if filtered_hdris.is_empty() {
                        ui.label(egui::RichText::new("No HDRIs found.\nDrop .hdr files into assets/hdris/").italics().color(Color32::from_rgb(140, 145, 155)));
                    } else {
                        let chunks: Vec<_> = filtered_hdris.chunks(cols).collect();
                        for row in chunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for hdri in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                                    let is_active = self.renderer.hdri.custom_hdr_path.as_ref() == Some(&hdri.path);
                                    let is_hovered = response.hovered();

                                    let bg_color = if is_active {
                                        egui::Color32::from_rgb(40, 48, 62)
                                    } else if is_hovered {
                                        egui::Color32::from_rgb(46, 50, 58)
                                    } else {
                                        egui::Color32::from_rgb(30, 32, 36)
                                    };

                                    let border_stroke = if is_active {
                                        egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(243, 94, 24))
                                    } else if is_hovered {
                                        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24))
                                    } else {
                                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54))
                                    };

                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    // Preview thumbnail
                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(thumb_w, thumb_w),
                                    );
                                    draw_hdri_thumbnail(ui.painter(), preview_rect, &hdri.id, &hdri.name);

                                    // Badge in top right corner
                                    let (badge_text, badge_color) = if hdri.is_builtin {
                                        ("BUILTIN", egui::Color32::from_rgb(255, 175, 40))
                                    } else {
                                        ("HDRI", egui::Color32::from_rgb(60, 180, 255))
                                    };
                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        badge_text,
                                        egui::FontId::proportional(7.5),
                                        badge_color,
                                    );

                                    // Name label
                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &hdri.name,
                                        egui::FontId::proportional(9.0),
                                        if is_active { egui::Color32::from_rgb(255, 200, 150) } else { egui::Color32::from_rgb(215, 220, 230) },
                                    );

                                    let tooltip = format!(
                                        "{}\nFile: {}\nSize: {}\n{}",
                                        hdri.name,
                                        hdri.file_name,
                                        hdri.size_formatted(),
                                        if is_active { "Active Viewport Environment" } else { "Click to set active HDRI" }
                                    );
                                    let response = response.on_hover_text(tooltip);

                                    if response.clicked() {
                                        selected_hdri_path = Some(hdri.path.clone());
                                    }
                                }
                            });
                        }
                    }
                });

            if let Some(path) = selected_hdri_path {
                self.activate_hdri_file(&path);
            }
            return;
        }

        if self.shelf_category == crate::shelf::ShelfCategory::Materials {
            let mat_query = self.shelf_search_query.to_lowercase();
            let filtered_mats: Vec<_> = self.asset_manager.materials.iter().filter(|m| {
                if !mat_query.is_empty() && !m.name.to_lowercase().contains(&mat_query) && !m.description.to_lowercase().contains(&mat_query) {
                    return false;
                }
                true
            }).collect();

            let mut selected_mat: Option<crate::asset_manager::CustomMaterialEntry> = None;
            let mut delete_mat_id: Option<String> = None;

            egui::ScrollArea::vertical()
                .id_source("assets_panel_materials_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if filtered_mats.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label(egui::RichText::new("No custom materials found.").strong().color(Color32::from_rgb(200, 205, 215)));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Built-in materials have been cleared.\nClick '+ New Material' or 'Save Layer as Mat' above to create your own materials!").italics().color(Color32::from_rgb(140, 145, 155)));
                            ui.add_space(12.0);
                            if ui.button("+ Create First Material").clicked() {
                                self.show_new_material_modal = true;
                                self.new_material_name = "My Material".to_string();
                            }
                        });
                    } else {
                        let chunks: Vec<_> = filtered_mats.chunks(cols).collect();
                        for row in chunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for mat in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                                    let is_hovered = response.hovered();
                                    let bg_color = if is_hovered {
                                        egui::Color32::from_rgb(46, 50, 58)
                                    } else {
                                        egui::Color32::from_rgb(30, 32, 36)
                                    };

                                    let border_stroke = if is_hovered {
                                        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(45, 140, 255))
                                    } else {
                                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54))
                                    };

                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    // Preview Sphere / Swatch
                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(thumb_w, thumb_w),
                                    );
                                    let center = preview_rect.center();
                                    let radius = (preview_rect.width() * 0.40).min(18.0);
                                    let col = egui::Color32::from_rgba_unmultiplied(
                                        (mat.base_color[0] * 255.0).clamp(0.0, 255.0) as u8,
                                        (mat.base_color[1] * 255.0).clamp(0.0, 255.0) as u8,
                                        (mat.base_color[2] * 255.0).clamp(0.0, 255.0) as u8,
                                        255,
                                    );
                                    ui.painter().circle_filled(center, radius, col);

                                    // Specular highlight on sphere for glossy materials
                                    if mat.roughness < 0.6 {
                                        let gloss = 1.0 - mat.roughness;
                                        let hl_offset = egui::vec2(-radius * 0.35, -radius * 0.35);
                                        let hl_radius = radius * (0.35 * gloss);
                                        let hl_alpha = ((0.8 * gloss) * 255.0) as u8;
                                        ui.painter().circle_filled(
                                            center + hl_offset,
                                            hl_radius,
                                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, hl_alpha),
                                        );
                                    }
                                    // Metallic border accent
                                    if mat.metallic > 0.5 {
                                        ui.painter().circle_stroke(
                                            center,
                                            radius + 1.0,
                                            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(255, 200, 80)),
                                        );
                                    }

                                    // Badge in top right corner
                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        "MAT",
                                        egui::FontId::proportional(7.5),
                                        egui::Color32::from_rgb(45, 140, 255),
                                    );

                                    // Name label
                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &mat.name,
                                        egui::FontId::proportional(9.0),
                                        egui::Color32::from_rgb(215, 220, 230),
                                    );

                                    let tooltip = format!(
                                        "{}\nRoughness: {:.2} | Metallic: {:.2}\nOpacity: {:.2} | Height: {:.2}\nClick to add as Fill Layer\nRight-click for options",
                                        mat.name,
                                        mat.roughness,
                                        mat.metallic,
                                        mat.opacity,
                                        mat.height,
                                    );
                                    let response = response.on_hover_text(tooltip);

                                    response.context_menu(|ui| {
                                        if ui.button("Apply as New Fill Layer").clicked() {
                                            selected_mat = Some((*mat).clone());
                                            ui.close_menu();
                                        }
                                        if ui.button("Apply to Current Active Layer").clicked() {
                                            let curv = self.project.curvature_map.clone();
                                            if let Some(layer) = self.project.active_layer_mut() {
                                                layer.fill_color = glam::Vec4::new(mat.base_color[0], mat.base_color[1], mat.base_color[2], mat.base_color[3]);
                                                layer.fill_roughness = mat.roughness;
                                                layer.fill_metallic = mat.metallic;
                                                layer.fill_height = mat.height;
                                                layer.fill_emissive = glam::Vec4::new(mat.emissive[0], mat.emissive[1], mat.emissive[2], mat.emissive[3]);
                                                layer.fill_opacity = mat.opacity;
                                                layer.fill_clearcoat = mat.clearcoat;
                                                layer.fill_anisotropy = mat.anisotropy;
                                                layer.evaluate_fill(curv.as_ref());
                                            }
                                            self.project.rebuild_composite();
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Delete Material").clicked() {
                                            delete_mat_id = Some(mat.id.clone());
                                            ui.close_menu();
                                        }
                                    });

                                    if response.clicked() {
                                        selected_mat = Some((*mat).clone());
                                    }
                                }
                            });
                        }
                    }
                });

            if let Some(id) = delete_mat_id {
                let _ = self.asset_manager.delete_material(&id);
                self.status_message = format!("Deleted material '{}'", id);
            }

            if let Some(mat) = selected_mat {
                let col = glam::Vec4::new(mat.base_color[0], mat.base_color[1], mat.base_color[2], mat.base_color[3]);
                let prev_active = self.project.active_layer_idx;
                let new_idx = self.project.add_fill_layer(
                    Some(mat.name.clone()),
                    col,
                    mat.roughness,
                    mat.metallic,
                );
                if let Some(layer) = self.project.layers.get_mut(new_idx) {
                    layer.fill_height = mat.height;
                    layer.fill_emissive = glam::Vec4::new(mat.emissive[0], mat.emissive[1], mat.emissive[2], mat.emissive[3]);
                    layer.fill_opacity = mat.opacity;
                    layer.fill_clearcoat = mat.clearcoat;
                    layer.fill_anisotropy = mat.anisotropy;
                    layer.evaluate_fill(self.project.curvature_map.as_ref());
                }
                self.project.rebuild_composite();
                let layer_clone = Box::new(self.project.layers[new_idx].clone());
                self.history.push("Add Material Layer", HistoryAction::AddLayer {
                    layer_idx: new_idx,
                    layer: layer_clone,
                    prev_active,
                });
                self.status_message = format!("Added Custom Material: {}", mat.name);
            }
            return;
        }

        if self.shelf_category == crate::shelf::ShelfCategory::Project || self.shelf_category == crate::shelf::ShelfCategory::Textures {
            let filtered_resources: Vec<_> = self.project.resources.iter().filter(|r| {
                if self.shelf_category == crate::shelf::ShelfCategory::Textures && r.resource_type != crate::project::ProjectResourceType::Texture && r.resource_type != crate::project::ProjectResourceType::BakedMap {
                    return false;
                }
                if !query.is_empty() && !r.name.to_lowercase().contains(&query) && !r.id.to_lowercase().contains(&query) {
                    return false;
                }
                true
            }).collect();

            let mut clicked_res_id = None;

            egui::ScrollArea::vertical()
                .id_source("assets_panel_project_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if filtered_resources.is_empty() {
                        if self.shelf_category == crate::shelf::ShelfCategory::Textures {
                            ui.label(egui::RichText::new("No textures found.\nClick '+ Import...' above to import textures or bake mesh maps.").italics().color(Color32::from_rgb(140, 145, 155)));
                        } else {
                            ui.label(egui::RichText::new("No project resources found.\nBake mesh maps or click '+ Import...' to add textures to project shelf.").italics().color(Color32::from_rgb(140, 145, 155)));
                        }
                    } else {
                        let chunks: Vec<_> = filtered_resources.chunks(cols).collect();
                        for row in chunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for res in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                                    let is_hovered = response.hovered();
                                    let bg_color = if is_hovered {
                                        egui::Color32::from_rgb(46, 50, 58)
                                    } else {
                                        egui::Color32::from_rgb(30, 32, 36)
                                    };

                                    let border_stroke = if is_hovered {
                                        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24))
                                    } else {
                                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54))
                                    };

                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    // Preview thumbnail
                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(thumb_w, thumb_w),
                                    );
                                    if let Some(thumb) = self.shelf_thumbnails.get(&res.id) {
                                        ui.painter().image(
                                            thumb.id(),
                                            preview_rect,
                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                            Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().rect_filled(preview_rect, 2.0, egui::Color32::from_rgb(38, 41, 48));
                                    }

                                    // Badge in top right corner
                                    let (badge_text, badge_color) = match res.resource_type {
                                        crate::project::ProjectResourceType::BakedMap => ("BAKED", egui::Color32::from_rgb(243, 94, 24)),
                                        crate::project::ProjectResourceType::Texture => ("TEX", egui::Color32::from_rgb(45, 140, 255)),
                                        crate::project::ProjectResourceType::Alpha => ("ALPHA", egui::Color32::from_rgb(80, 210, 100)),
                                        crate::project::ProjectResourceType::Hdri => ("HDRI", egui::Color32::from_rgb(60, 180, 255)),
                                    };
                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        badge_text,
                                        egui::FontId::proportional(7.5),
                                        badge_color,
                                    );

                                    // Name label
                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &res.name,
                                        egui::FontId::proportional(9.0),
                                        egui::Color32::from_rgb(215, 220, 230),
                                    );

                                    let tooltip = format!(
                                        "{}\nType: {}\nSize: {}x{}\nClick to assign to layer channel or slot",
                                        res.name,
                                        res.badge_label(),
                                        res.width,
                                        res.height
                                    );
                                    let response = response.on_hover_text(tooltip);

                                    if response.clicked() || response.secondary_clicked() {
                                        clicked_res_id = Some(res.id.clone());
                                    }
                                }
                            });
                        }
                    }
                });

            if let Some(id) = clicked_res_id {
                self.shelf_resource_popup = Some(id);
            }
            return;
        }

        let mut clicked_res_id = None;

        egui::ScrollArea::vertical()
            .id_source("assets_panel_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.shelf_category == crate::shelf::ShelfCategory::All && !self.project.resources.is_empty() {
                    let project_matches: Vec<_> = self.project.resources.iter().filter(|r| {
                        if !query.is_empty() && !r.name.to_lowercase().contains(&query) && !r.id.to_lowercase().contains(&query) {
                            return false;
                        }
                        true
                    }).collect();

                    if !project_matches.is_empty() {
                        ui.label(egui::RichText::new("PROJECT RESOURCES").small().strong().color(Color32::from_rgb(243, 94, 24)));
                        let pchunks: Vec<_> = project_matches.chunks(cols).collect();
                        for row in pchunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for res in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                                    let is_hovered = response.hovered();
                                    let bg_color = if is_hovered {
                                        egui::Color32::from_rgb(46, 50, 58)
                                    } else {
                                        egui::Color32::from_rgb(30, 32, 36)
                                    };

                                    let border_stroke = if is_hovered {
                                        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24))
                                    } else {
                                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54))
                                    };

                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(thumb_w, thumb_w),
                                    );
                                    if let Some(thumb) = self.shelf_thumbnails.get(&res.id) {
                                        ui.painter().image(
                                            thumb.id(),
                                            preview_rect,
                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                            Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().rect_filled(preview_rect, 2.0, egui::Color32::from_rgb(38, 41, 48));
                                    }

                                    let (badge_text, badge_color) = match res.resource_type {
                                        crate::project::ProjectResourceType::BakedMap => ("BAKED", egui::Color32::from_rgb(243, 94, 24)),
                                        crate::project::ProjectResourceType::Texture => ("TEX", egui::Color32::from_rgb(45, 140, 255)),
                                        crate::project::ProjectResourceType::Alpha => ("ALPHA", egui::Color32::from_rgb(80, 210, 100)),
                                        crate::project::ProjectResourceType::Hdri => ("HDRI", egui::Color32::from_rgb(60, 180, 255)),
                                    };
                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        badge_text,
                                        egui::FontId::proportional(7.5),
                                        badge_color,
                                    );

                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &res.name,
                                        egui::FontId::proportional(9.0),
                                        egui::Color32::from_rgb(215, 220, 230),
                                    );

                                    let tooltip = format!(
                                        "{}\nType: {}\nSize: {}x{}\nClick to assign to layer channel or slot",
                                        res.name,
                                        res.badge_label(),
                                        res.width,
                                        res.height
                                    );
                                    let response = response.on_hover_text(tooltip);

                                    if response.clicked() || response.secondary_clicked() {
                                        clicked_res_id = Some(res.id.clone());
                                    }
                                }
                            });
                        }
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("LIBRARY PRESETS").small().strong().color(Color32::from_rgb(140, 145, 155)));
                    }
                }

                if self.shelf_category == crate::shelf::ShelfCategory::All && !self.asset_manager.materials.is_empty() {
                    let mat_matches: Vec<_> = self.asset_manager.materials.iter().filter(|m| {
                        if !query.is_empty() && !m.name.to_lowercase().contains(&query) && !m.description.to_lowercase().contains(&query) {
                            return false;
                        }
                        true
                    }).collect();

                    if !mat_matches.is_empty() {
                        ui.label(egui::RichText::new("USER MATERIALS").small().strong().color(Color32::from_rgb(45, 140, 255)));
                        let mchunks: Vec<_> = mat_matches.chunks(cols).collect();
                        for row in mchunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for mat in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());
                                    let is_hovered = response.hovered();
                                    let bg_color = if is_hovered { egui::Color32::from_rgb(46, 50, 58) } else { egui::Color32::from_rgb(30, 32, 36) };
                                    let border_stroke = if is_hovered { egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(45, 140, 255)) } else { egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54)) };
                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(thumb_w, thumb_w),
                                    );
                                    let center = preview_rect.center();
                                    let radius = (preview_rect.width() * 0.40).min(18.0);
                                    let col = egui::Color32::from_rgba_unmultiplied(
                                        (mat.base_color[0] * 255.0).clamp(0.0, 255.0) as u8,
                                        (mat.base_color[1] * 255.0).clamp(0.0, 255.0) as u8,
                                        (mat.base_color[2] * 255.0).clamp(0.0, 255.0) as u8,
                                        255,
                                    );
                                    ui.painter().circle_filled(center, radius, col);

                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        "MAT",
                                        egui::FontId::proportional(7.5),
                                        egui::Color32::from_rgb(45, 140, 255),
                                    );

                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &mat.name,
                                        egui::FontId::proportional(9.0),
                                        egui::Color32::from_rgb(215, 220, 230),
                                    );

                                    if response.clicked() {
                                        let col = glam::Vec4::new(mat.base_color[0], mat.base_color[1], mat.base_color[2], mat.base_color[3]);
                                        self.project.add_fill_layer(Some(mat.name.clone()), col, mat.roughness, mat.metallic);
                                        self.status_message = format!("Added Custom Material: {}", mat.name);
                                    }
                                }
                            });
                        }
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                    }
                }

                if (self.shelf_category == crate::shelf::ShelfCategory::All || self.shelf_category == crate::shelf::ShelfCategory::Decals) && !self.asset_manager.decals.is_empty() {
                    let decal_matches: Vec<_> = self.asset_manager.decals.iter().filter(|d| {
                        if !query.is_empty() && !d.name.to_lowercase().contains(&query) && !d.description.to_lowercase().contains(&query) {
                            return false;
                        }
                        true
                    }).collect();

                    if !decal_matches.is_empty() {
                        ui.label(egui::RichText::new("USER DECALS & HARDWARE").small().strong().color(Color32::from_rgb(255, 140, 30)));
                        let dchunks: Vec<_> = decal_matches.chunks(cols).collect();
                        let mut delete_decal_id: Option<String> = None;

                        for row in dchunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for decal in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());
                                    let is_hovered = response.hovered();
                                    let bg_color = if is_hovered { egui::Color32::from_rgb(46, 50, 58) } else { egui::Color32::from_rgb(30, 32, 36) };
                                    let border_stroke = if is_hovered { egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(255, 140, 30)) } else { egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54)) };
                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(thumb_w, thumb_w),
                                    );
                                    let center = preview_rect.center();
                                    let radius = (preview_rect.width() * 0.38).min(17.0);

                                    ui.painter().circle_stroke(center, radius, egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(255, 175, 40)));
                                    ui.painter().line_segment([center + egui::vec2(-radius * 0.6, 0.0), center + egui::vec2(radius * 0.6, 0.0)], egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(255, 230, 80)));
                                    ui.painter().line_segment([center + egui::vec2(0.0, -radius * 0.6), center + egui::vec2(0.0, radius * 0.6)], egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(255, 230, 80)));

                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        "DECAL",
                                        egui::FontId::proportional(7.5),
                                        egui::Color32::from_rgb(255, 140, 30),
                                    );

                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &decal.name,
                                        egui::FontId::proportional(9.0),
                                        egui::Color32::from_rgb(215, 220, 230),
                                    );

                                    let tooltip = format!("Decal: {}\nStyle: {}\nType: {}\nRadius: {:.0}px | Depth: {:.2}\nRight-click to delete preset",
                                        decal.name, decal.stroke_style.name(), decal.decal_type.name(), decal.radius, decal.height_depth
                                    );
                                    let response = response.on_hover_text(tooltip);

                                    if response.clicked() {
                                        if self.brush_engine.settings.mode != crate::brush::ToolMode::Stamp {
                                            self.brush_engine.settings.mode = crate::brush::ToolMode::Stamp;
                                        }
                                        self.brush_engine.settings.path_tool.stroke_style = decal.stroke_style;
                                        self.brush_engine.settings.path_tool.decal_type = decal.decal_type;
                                        self.brush_engine.settings.path_tool.spacing_ratio = decal.spacing_ratio;
                                        self.brush_engine.settings.path_tool.decal_spacing = decal.decal_spacing;
                                        self.brush_engine.settings.path_tool.height_depth = decal.height_depth;
                                        self.brush_engine.settings.path_tool.heat_tint = decal.heat_tint;
                                        self.brush_engine.settings.path_tool.surface_offset = decal.surface_offset;
                                        self.brush_engine.settings.stamp_tool.stroke_style = decal.stroke_style;
                                        self.brush_engine.settings.stamp_tool.decal_type = decal.decal_type;
                                        self.brush_engine.settings.stamp_tool.height_depth = decal.height_depth;
                                        self.brush_engine.settings.stamp_tool.heat_tint = decal.heat_tint;
                                        self.brush_engine.settings.stamp_tool.surface_offset = decal.surface_offset;
                                        self.brush_engine.settings.stamp_tool.selected_preset_id = Some(decal.id.clone());
                                        self.brush_engine.settings.radius = decal.radius;
                                        self.brush_engine.settings.paint_color = glam::Vec4::new(decal.base_color[0], decal.base_color[1], decal.base_color[2], decal.base_color[3]);
                                        self.brush_engine.settings.paint_roughness = decal.roughness;
                                        self.brush_engine.settings.paint_metallic = decal.metallic;
                                        self.brush_engine.settings.paint_emissive = glam::Vec4::new(decal.emissive[0], decal.emissive[1], decal.emissive[2], decal.emissive[3]);
                                        self.brush_engine.settings.paint_opacity = decal.opacity;
                                        self.brush_engine.settings.paint_clearcoat = decal.clearcoat;
                                        self.brush_engine.settings.paint_anisotropy = decal.anisotropy;
                                        self.status_message = format!("Loaded Custom Decal Preset: {}", decal.name);
                                    }

                                    if response.secondary_clicked() {
                                        delete_decal_id = Some(decal.id.clone());
                                    }
                                }
                            });
                        }

                        if let Some(del_id) = delete_decal_id {
                            let _ = self.asset_manager.delete_decal(&del_id);
                            self.status_message = format!("Deleted Decal Preset: {}", del_id);
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                    }
                }
                let chunks: Vec<_> = filtered_assets.chunks(cols).collect();
                for row in chunks {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                        for asset in row {
                            let card_size = egui::vec2(card_w, card_h);
                            let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                            let is_hovered = response.hovered();
                            let bg_color = if is_hovered {
                                egui::Color32::from_rgb(46, 50, 58)
                            } else {
                                egui::Color32::from_rgb(30, 32, 36)
                            };

                            let border_stroke = if is_hovered {
                                egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24))
                            } else {
                                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54))
                            };

                            ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                            // Draw preview thumbnail depending on category
                            let preview_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                egui::vec2(thumb_w, thumb_w),
                            );

                            match asset.category {
                                crate::shelf::ShelfCategory::Materials | crate::shelf::ShelfCategory::SmartMaterials => {
                                    let center = preview_rect.center();
                                    let radius = (preview_rect.width() * 0.42).min(18.0);
                                    let col = egui::Color32::from_rgb(asset.preview_color[0], asset.preview_color[1], asset.preview_color[2]);
                                    ui.painter().circle_filled(center, radius, col);
                                    if asset.preview_metallic > 0.2 || asset.preview_roughness < 0.4 {
                                        let shine_pos = center - egui::vec2(radius * 0.35, radius * 0.35);
                                        let shine_alpha = ((1.0 - asset.preview_roughness) * 200.0).clamp(40.0, 220.0) as u8;
                                        ui.painter().circle_filled(shine_pos, radius * 0.28, egui::Color32::from_rgba_unmultiplied(255, 255, 255, shine_alpha));
                                    }
                                }
                                crate::shelf::ShelfCategory::Generators => {
                                    // Procedural Noise thumbnail graphic
                                    ui.painter().rect_filled(preview_rect, 2.0, egui::Color32::from_rgb(20, 22, 26));
                                    match asset.id {
                                        "gen_checker" => {
                                            let hw = preview_rect.width() * 0.5;
                                            let hh = preview_rect.height() * 0.5;
                                            ui.painter().rect_filled(egui::Rect::from_min_size(preview_rect.min, egui::vec2(hw, hh)), 0.0, egui::Color32::WHITE);
                                            ui.painter().rect_filled(egui::Rect::from_min_size(preview_rect.min + egui::vec2(hw, hh), egui::vec2(hw, hh)), 0.0, egui::Color32::WHITE);
                                        }
                                        "gen_worley" => {
                                            let pts = [
                                                preview_rect.min + egui::vec2(8.0, 8.0),
                                                preview_rect.min + egui::vec2(preview_rect.width() - 8.0, 10.0),
                                                preview_rect.center(),
                                                preview_rect.min + egui::vec2(10.0, preview_rect.height() - 8.0),
                                                preview_rect.max - egui::vec2(8.0, 8.0),
                                            ];
                                            for p in pts {
                                                ui.painter().circle_filled(p, 3.0, egui::Color32::from_rgb(180, 185, 195));
                                                ui.painter().circle_stroke(p, 6.0, egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(90, 95, 105)));
                                            }
                                        }
                                        "gen_billow" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_filled(c + egui::vec2(-6.0, 2.0), 7.0, egui::Color32::from_rgb(150, 160, 175));
                                            ui.painter().circle_filled(c + egui::vec2(6.0, 3.0), 6.5, egui::Color32::from_rgb(130, 140, 155));
                                            ui.painter().circle_filled(c + egui::vec2(0.0, -4.0), 8.5, egui::Color32::from_rgb(205, 215, 230));
                                        }
                                        "gen_ridged" => {
                                            let p1 = preview_rect.left_top() + egui::vec2(4.0, 6.0);
                                            let p2 = preview_rect.center() + egui::vec2(-3.0, -4.0);
                                            let p3 = preview_rect.center() + egui::vec2(3.0, 3.0);
                                            let p4 = preview_rect.right_bottom() - egui::vec2(5.0, 5.0);
                                            ui.painter().line_segment([p1, p2], egui::Stroke::new(1.8_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([p2, p3], egui::Stroke::new(1.8_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([p3, p4], egui::Stroke::new(1.8_f32, egui::Color32::WHITE));
                                            let p5 = preview_rect.left_bottom() + egui::vec2(6.0, -7.0);
                                            let p6 = p3 + egui::vec2(1.0, 3.0);
                                            ui.painter().line_segment([p5, p6], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(180, 180, 180)));
                                        }
                                        "gen_domain_warp" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_stroke(c, 11.0, egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(120, 160, 200)));
                                            ui.painter().circle_stroke(c + egui::vec2(2.0, -1.0), 7.0, egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(180, 210, 240)));
                                            ui.painter().circle_filled(c + egui::vec2(3.0, -2.0), 3.0, egui::Color32::WHITE);
                                        }
                                        "gen_splatter" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_filled(c, 4.0, egui::Color32::from_rgb(210, 110, 70));
                                            ui.painter().circle_filled(c + egui::vec2(7.0, -6.0), 2.5, egui::Color32::from_rgb(180, 80, 50));
                                            ui.painter().circle_filled(c + egui::vec2(-8.0, 5.0), 3.0, egui::Color32::from_rgb(220, 130, 80));
                                            ui.painter().circle_filled(c + egui::vec2(6.0, 7.0), 1.8, egui::Color32::from_rgb(160, 70, 40));
                                            ui.painter().circle_filled(c + egui::vec2(-5.0, -7.0), 2.0, egui::Color32::from_rgb(190, 90, 60));
                                        }
                                        "gen_wood" => {
                                            let c = preview_rect.left_top();
                                            ui.painter().circle_stroke(c, 11.0, egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(190, 130, 70)));
                                            ui.painter().circle_stroke(c, 19.0, egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(150, 95, 45)));
                                            ui.painter().circle_stroke(c, 27.0, egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(120, 75, 35)));
                                        }
                                        "gen_fabric" => {
                                            let min = preview_rect.min + egui::vec2(4.0, 4.0);
                                            let max = preview_rect.max - egui::vec2(4.0, 4.0);
                                            let col = egui::Color32::from_rgb(140, 150, 170);
                                            for i in 0..4 {
                                                let t = i as f32 / 3.0;
                                                let x = min.x + t * (max.x - min.x);
                                                let y = min.y + t * (max.y - min.y);
                                                ui.painter().line_segment([egui::pos2(x, min.y), egui::pos2(x, max.y)], egui::Stroke::new(1.0_f32, col));
                                                ui.painter().line_segment([egui::pos2(min.x, y), egui::pos2(max.x, y)], egui::Stroke::new(1.0_f32, col));
                                            }
                                        }
                                        "gen_scratches" => {
                                            ui.painter().line_segment([preview_rect.left_top() + egui::vec2(4.0, 6.0), preview_rect.right_bottom() - egui::vec2(8.0, 4.0)], egui::Stroke::new(1.2_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([preview_rect.left_top() + egui::vec2(12.0, 4.0), preview_rect.right_bottom() - egui::vec2(4.0, 10.0)], egui::Stroke::new(0.8_f32, egui::Color32::from_rgb(180, 180, 180)));
                                            ui.painter().line_segment([preview_rect.left_bottom() + egui::vec2(6.0, -8.0), preview_rect.right_top() + egui::vec2(-6.0, 8.0)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(200, 200, 200)));
                                        }
                                        "gen_bricks" => {
                                            let y1 = preview_rect.min.y + preview_rect.height() * 0.33;
                                            let y2 = preview_rect.min.y + preview_rect.height() * 0.66;
                                            ui.painter().line_segment([egui::pos2(preview_rect.min.x, y1), egui::pos2(preview_rect.max.x, y1)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(170, 110, 80)));
                                            ui.painter().line_segment([egui::pos2(preview_rect.min.x, y2), egui::pos2(preview_rect.max.x, y2)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(170, 110, 80)));
                                            ui.painter().line_segment([egui::pos2(preview_rect.center().x, preview_rect.min.y), egui::pos2(preview_rect.center().x, y1)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(170, 110, 80)));
                                        }
                                        "gen_curvature" => {
                                            ui.painter().rect_stroke(preview_rect.shrink(6.0), 1.0, egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                        }
                                        "gen_dust" => {
                                            let c = preview_rect.center();
                                            ui.painter().line_segment([preview_rect.left_top() + egui::vec2(4.0, 8.0), preview_rect.right_top() + egui::vec2(-4.0, 8.0)], egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(220, 220, 200)));
                                            ui.painter().circle_filled(c + egui::vec2(0.0, 2.0), 2.0, egui::Color32::from_rgb(180, 180, 160));
                                        }
                                        "gen_cavity" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_stroke(c, 9.0, egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(80, 75, 70)));
                                            ui.painter().circle_filled(c, 5.0, egui::Color32::from_rgb(40, 35, 30));
                                        }
                                        "gen_leaks" => {
                                            let min = preview_rect.min;
                                            let max = preview_rect.max;
                                            let col = egui::Color32::from_rgb(160, 100, 60);
                                            ui.painter().line_segment([egui::pos2(min.x + 8.0, min.y + 4.0), egui::pos2(min.x + 8.0, max.y - 6.0)], egui::Stroke::new(1.5_f32, col));
                                            ui.painter().line_segment([egui::pos2(min.x + 16.0, min.y + 4.0), egui::pos2(min.x + 16.0, max.y - 12.0)], egui::Stroke::new(1.0_f32, col));
                                            ui.painter().line_segment([egui::pos2(min.x + 24.0, min.y + 4.0), egui::pos2(min.x + 24.0, max.y - 4.0)], egui::Stroke::new(1.8_f32, col));
                                        }
                                        "gen_position" => {
                                            let r = preview_rect.shrink(5.0);
                                            ui.painter().rect_filled(r, 1.0, egui::Color32::from_rgb(70, 90, 120));
                                            ui.painter().line_segment([egui::pos2(r.min.x, r.min.y + 4.0), egui::pos2(r.max.x, r.min.y + 4.0)], egui::Stroke::new(2.0_f32, egui::Color32::WHITE));
                                        }
                                        _ => {
                                            let center = preview_rect.center();
                                            ui.painter().circle_filled(center, 12.0, egui::Color32::from_rgb(140, 145, 155));
                                            ui.painter().circle_stroke(center, 7.0, egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(70, 75, 85)));
                                            ui.painter().circle_filled(center, 3.5, egui::Color32::WHITE);
                                        }
                                    }
                                }
                                crate::shelf::ShelfCategory::SmartMasks => {
                                    ui.painter().rect_filled(preview_rect, 2.0, egui::Color32::from_rgb(26, 28, 32));
                                    match asset.id {
                                        "mask_edge_wear" => {
                                            let r = preview_rect.shrink(5.0);
                                            ui.painter().rect_stroke(r, 1.0, egui::Stroke::new(2.0_f32, egui::Color32::WHITE));
                                            ui.painter().circle_filled(r.left_top(), 3.0, egui::Color32::from_rgb(26, 28, 32));
                                            ui.painter().circle_filled(r.right_bottom() - egui::vec2(2.0, 0.0), 2.5, egui::Color32::from_rgb(26, 28, 32));
                                        }
                                        "mask_crevice_dirt" => {
                                            let c = preview_rect.center();
                                            ui.painter().rect_filled(preview_rect.shrink(4.0), 2.0, egui::Color32::from_rgb(45, 48, 54));
                                            ui.painter().circle_filled(c, 7.0, egui::Color32::from_rgb(18, 19, 22));
                                            ui.painter().circle_filled(c + egui::vec2(-3.0, -2.0), 3.0, egui::Color32::from_rgb(10, 10, 12));
                                        }
                                        "directional_dust" => {
                                            let top_bar = egui::Rect::from_min_size(preview_rect.min + egui::vec2(3.0, 3.0), egui::vec2(preview_rect.width() - 6.0, 6.0));
                                            ui.painter().rect_filled(top_bar, 1.0, egui::Color32::from_rgb(235, 235, 220));
                                            let mid_bar = egui::Rect::from_min_size(preview_rect.min + egui::vec2(5.0, 10.0), egui::vec2(preview_rect.width() - 10.0, 3.0));
                                            ui.painter().rect_filled(mid_bar, 1.0, egui::Color32::from_rgb(160, 160, 150));
                                        }
                                        "dripping_rust_leaks" => {
                                            let c = preview_rect.center();
                                            ui.painter().line_segment([c + egui::vec2(-6.0, -10.0), c + egui::vec2(-6.0, 6.0)], egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(180, 80, 40)));
                                            ui.painter().circle_filled(c + egui::vec2(-6.0, 7.0), 2.5, egui::Color32::from_rgb(180, 80, 40));
                                            ui.painter().line_segment([c + egui::vec2(4.0, -8.0), c + egui::vec2(4.0, 10.0)], egui::Stroke::new(1.8_f32, egui::Color32::from_rgb(160, 70, 35)));
                                            ui.painter().circle_filled(c + egui::vec2(4.0, 11.0), 2.2, egui::Color32::from_rgb(160, 70, 35));
                                        }
                                        "mask_grunge" => {
                                            let c = preview_rect.center();
                                            ui.painter().line_segment([c + egui::vec2(-10.0, -8.0), c + egui::vec2(10.0, 6.0)], egui::Stroke::new(1.4_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([c + egui::vec2(-7.0, -1.0), c + egui::vec2(9.0, 11.0)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(200, 200, 200)));
                                            ui.painter().line_segment([c + egui::vec2(-11.0, 5.0), c + egui::vec2(5.0, -10.0)], egui::Stroke::new(0.8_f32, egui::Color32::from_rgb(170, 170, 170)));
                                        }
                                        "peeling_paint" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_stroke(c, 8.0, egui::Stroke::new(2.0_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([c + egui::vec2(6.0, -5.0), c + egui::vec2(12.0, -10.0)], egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                            ui.painter().circle_filled(c + egui::vec2(-2.0, 2.0), 3.0, egui::Color32::from_rgb(150, 120, 80));
                                        }
                                        "mask_ground_dirt" => {
                                            let bot = preview_rect.max.y - 4.0;
                                            let w = preview_rect.width();
                                            let mx = preview_rect.min.x;
                                            ui.painter().circle_filled(egui::pos2(mx + w * 0.2, bot), 4.0, egui::Color32::from_rgb(140, 95, 60));
                                            ui.painter().circle_filled(egui::pos2(mx + w * 0.5, bot - 2.0), 5.5, egui::Color32::from_rgb(120, 80, 50));
                                            ui.painter().circle_filled(egui::pos2(mx + w * 0.8, bot), 4.5, egui::Color32::from_rgb(140, 95, 60));
                                            ui.painter().circle_filled(egui::pos2(mx + w * 0.35, bot - 8.0), 2.0, egui::Color32::from_rgb(170, 120, 75));
                                        }
                                        "mask_heavy_machinery" => {
                                            let r = preview_rect.shrink(5.0);
                                            ui.painter().rect_stroke(r, 1.0, egui::Stroke::new(1.8_f32, egui::Color32::WHITE));
                                            let c = preview_rect.center();
                                            ui.painter().circle_filled(c, 4.0, egui::Color32::from_rgb(70, 60, 50));
                                            ui.painter().line_segment([c + egui::vec2(-6.0, -4.0), c + egui::vec2(6.0, 4.0)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(200, 200, 200)));
                                        }
                                        "mask_gun_metal" => {
                                            let r = preview_rect.shrink(5.0);
                                            ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.2_f32, egui::Color32::WHITE));
                                            ui.painter().rect_filled(r.shrink(2.0), 0.0, egui::Color32::from_rgb(35, 38, 44));
                                        }
                                        "mask_patina" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_filled(c, 7.0, egui::Color32::from_rgb(45, 175, 145));
                                            ui.painter().circle_stroke(c, 11.0, egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(30, 130, 110)));
                                        }
                                        "mask_invert" => {
                                            let c = preview_rect.center();
                                            ui.painter().rect_stroke(preview_rect.shrink(4.0), 1.0, egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([preview_rect.left_bottom() + egui::vec2(4.0, -4.0), preview_rect.right_top() + egui::vec2(-4.0, 4.0)], egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                            ui.painter().circle_filled(c + egui::vec2(4.0, -4.0), 2.5, egui::Color32::WHITE);
                                        }
                                        _ => {
                                            ui.painter().rect_stroke(preview_rect.shrink(4.0), 2.0, egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                            ui.painter().circle_filled(preview_rect.center(), 4.0, egui::Color32::WHITE);
                                        }
                                    }
                                }
                                crate::shelf::ShelfCategory::Alphas => {
                                    let center = preview_rect.center();
                                    ui.painter().circle_filled(center, 11.0, egui::Color32::WHITE);
                                }
                                crate::shelf::ShelfCategory::Decals => {
                                    let center = preview_rect.center();
                                    ui.painter().circle_stroke(center, 9.0, egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(255, 140, 70)));
                                    ui.painter().line_segment([egui::pos2(center.x - 5.0, center.y), egui::pos2(center.x + 5.0, center.y)], egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                    ui.painter().line_segment([egui::pos2(center.x, center.y - 5.0), egui::pos2(center.x, center.y + 5.0)], egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                }
                                _ => {
                                    ui.painter().rect_filled(preview_rect, 2.0, egui::Color32::from_rgb(60, 64, 72));
                                }
                            }

                            // Category pill badge in corner
                            let badge_text = match asset.category {
                                crate::shelf::ShelfCategory::Materials => "MAT",
                                crate::shelf::ShelfCategory::SmartMaterials => "SMART",
                                crate::shelf::ShelfCategory::SmartMasks => "MASK",
                                crate::shelf::ShelfCategory::Generators => "NOISE",
                                crate::shelf::ShelfCategory::Alphas => "ALPHA",
                                crate::shelf::ShelfCategory::Decals => "DECAL",
                                _ => "",
                            };
                            let badge_color = match asset.category {
                                crate::shelf::ShelfCategory::Materials => egui::Color32::from_rgb(45, 140, 255),
                                crate::shelf::ShelfCategory::SmartMaterials => egui::Color32::from_rgb(243, 94, 24),
                                crate::shelf::ShelfCategory::SmartMasks => egui::Color32::from_rgb(180, 80, 255),
                                crate::shelf::ShelfCategory::Generators => egui::Color32::from_rgb(46, 204, 113),
                                crate::shelf::ShelfCategory::Alphas => egui::Color32::from_rgb(80, 210, 100),
                                crate::shelf::ShelfCategory::Decals => egui::Color32::from_rgb(255, 120, 60),
                                _ => egui::Color32::GRAY,
                            };
                            ui.painter().text(
                                egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                egui::Align2::RIGHT_TOP,
                                badge_text,
                                egui::FontId::proportional(7.5),
                                badge_color,
                            );

                            // Asset name label underneath
                            let name_rect = egui::Rect::from_min_max(
                                egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                            );
                            ui.painter().text(
                                name_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                asset.name,
                                egui::FontId::proportional(9.5),
                                egui::Color32::from_rgb(215, 220, 230),
                            );

                            let tooltip = format!("{}\n{}", asset.name, asset.description);
                            let response = response.on_hover_text(tooltip);

                            if response.clicked() {
                                self.apply_shelf_asset(&asset, ui.ctx());
                            }
                        }
                    });
                }
            });

        if let Some(id) = clicked_res_id {
            self.shelf_resource_popup = Some(id);
        }
    }

    /// Apply an asset from the shelf to the project
    pub(crate) fn apply_shelf_asset(&mut self, asset: &crate::shelf::ShelfAsset, ctx: &egui::Context) {
        match asset.category {
            crate::shelf::ShelfCategory::Materials => {
                let col = glam::Vec4::new(
                    asset.preview_color[0] as f32 / 255.0,
                    asset.preview_color[1] as f32 / 255.0,
                    asset.preview_color[2] as f32 / 255.0,
                    1.0,
                );
                self.project.add_fill_layer(
                    Some(asset.name.to_string()),
                    col,
                    asset.preview_roughness,
                    asset.preview_metallic,
                );
                self.status_message = format!("Added Material: {}", asset.name);
            }
            crate::shelf::ShelfCategory::SmartMaterials => {
                let new_layers = crate::smart_materials::SmartMaterialLibrary::build_layers_with_maps(
                    asset.id,
                    self.project.texture_width,
                    self.project.texture_height,
                    self.project.curvature_map.as_ref(),
                    self.project.ao_map.as_ref(),
                    self.project.world_space_normal_map.as_ref(),
                    self.project.position_map.as_ref(),
                );
                for mut layer in new_layers {
                    layer.id = self.project.next_layer_id;
                    self.project.next_layer_id += 1;
                    self.project.layers.push(layer);
                }
                self.project.active_layer_idx = self.project.layers.len().saturating_sub(1);

                // Auto-configure physical optical shader settings for refractive and cutout materials
                if asset.id == "emerald_crystal_gem" {
                    self.shader_settings.transparency_mode = crate::wgpu_renderer::TransparencyMode::Refraction;
                    self.shader_settings.ior = 1.58;
                    self.shader_settings.double_sided = true;
                } else if asset.id == "amber_fossil_resin" {
                    self.shader_settings.transparency_mode = crate::wgpu_renderer::TransparencyMode::Refraction;
                    self.shader_settings.ior = 1.54;
                    self.shader_settings.double_sided = true;
                } else if asset.id == "frosted_ice_glass" {
                    self.shader_settings.transparency_mode = crate::wgpu_renderer::TransparencyMode::Refraction;
                    self.shader_settings.ior = 1.31;
                    self.shader_settings.double_sided = true;
                } else if asset.id == "tinted_glass_visor" {
                    self.shader_settings.transparency_mode = crate::wgpu_renderer::TransparencyMode::Refraction;
                    self.shader_settings.ior = 1.52;
                    self.shader_settings.double_sided = true;
                } else if asset.id == "perforated_steel_grate" || asset.id == "wire_chainlink_fence" || asset.id == "torn_camo_fabric" {
                    self.shader_settings.transparency_mode = crate::wgpu_renderer::TransparencyMode::AlphaCutout;
                    self.shader_settings.double_sided = true;
                }

                self.status_message = format!("Applied Smart Material: {}", asset.name);
            }
            crate::shelf::ShelfCategory::SmartMasks => {
                let act_idx = self.project.active_layer_idx;
                if act_idx < self.project.layers.len() {
                    let curv = self.project.curvature_map.clone();
                    let ao = self.project.ao_map.clone();
                    let norm = self.project.world_space_normal_map.clone();
                    let pos = self.project.position_map.clone();
                    let layer = &mut self.project.layers[act_idx];
                    crate::smart_materials::SmartMaterialLibrary::apply_smart_mask_advanced(
                        asset.id,
                        layer,
                        curv.as_ref(),
                        ao.as_ref(),
                        pos.as_ref(),
                        norm.as_ref(),
                    );
                    self.brush_engine.settings.target = crate::brush::PaintTarget::Mask;
                    self.status_message = format!("Applied Smart Mask (Non-Destructive Stack): {}", asset.name);
                }
            }
            crate::shelf::ShelfCategory::Generators => {
                let act_idx = self.project.active_layer_idx;
                let is_mask_target = self.brush_engine.settings.target == crate::brush::PaintTarget::Mask;
                if act_idx < self.project.layers.len() && is_mask_target {
                    let curv = self.project.curvature_map.clone();
                    let ao = self.project.ao_map.clone();
                    let pos = self.project.position_map.clone();
                    let wn = self.project.world_space_normal_map.clone();
                    let layer = &mut self.project.layers[act_idx];

                    match asset.id {
                        "gen_curvature" => {
                            layer.add_mask_generator_curvature_wear(curv.as_ref(), ao.as_ref(), pos.as_ref(), wn.as_ref());
                            self.status_message = format!("Added Curvature Edge Wear Generator to Mask: {}", asset.name);
                        }
                        "gen_cavity" => {
                            layer.add_mask_generator_cavity_dirt(curv.as_ref(), ao.as_ref(), pos.as_ref(), wn.as_ref());
                            self.status_message = format!("Added Cavity Dirt Generator to Mask: {}", asset.name);
                        }
                        "gen_dust" => {
                            layer.add_mask_generator_directional_dust(curv.as_ref(), ao.as_ref(), pos.as_ref(), wn.as_ref());
                            self.status_message = format!("Added Directional Dust Generator to Mask: {}", asset.name);
                        }
                        "gen_ground_dirt" => {
                            layer.add_mask_generator_ground_dirt(curv.as_ref(), ao.as_ref(), pos.as_ref(), wn.as_ref());
                            self.status_message = format!("Added Ground Mud Generator to Mask: {}", asset.name);
                        }
                        "gen_mask_editor" => {
                            layer.add_mask_generator_mask_editor(curv.as_ref(), ao.as_ref(), pos.as_ref(), wn.as_ref());
                            self.status_message = format!("Added Mask Editor Generator to Mask: {}", asset.name);
                        }
                        _ => {
                            let gen = match asset.id {
                                "gen_perlin" => crate::layer::ProceduralGenerator::PerlinNoise,
                                "gen_worley" => crate::layer::ProceduralGenerator::WorleyGrunge,
                                "gen_billow" => crate::layer::ProceduralGenerator::BillowNoise,
                                "gen_ridged" => crate::layer::ProceduralGenerator::RidgedMultifractal,
                                "gen_domain_warp" => crate::layer::ProceduralGenerator::DomainWarpGrunge,
                                "gen_splatter" => crate::layer::ProceduralGenerator::SplatterDirt,
                                "gen_wood" => crate::layer::ProceduralGenerator::WoodGrain,
                                "gen_fabric" => crate::layer::ProceduralGenerator::FabricWeave,
                                "gen_scratches" => crate::layer::ProceduralGenerator::Scratches,
                                "gen_leaks" => crate::layer::ProceduralGenerator::DrippingLeaks,
                                "gen_position" => crate::layer::ProceduralGenerator::PositionGradient,
                                "gen_checker" => crate::layer::ProceduralGenerator::Checkerboard,
                                "gen_bricks" => crate::layer::ProceduralGenerator::Bricks,
                                "gen_perforated" => crate::layer::ProceduralGenerator::PerforatedMesh,
                                "gen_wire_fence" => crate::layer::ProceduralGenerator::WireFence,
                                "gen_torn_fabric" => crate::layer::ProceduralGenerator::TornFabricEdge,
                                "gen_dots" => crate::layer::ProceduralGenerator::CircularDots,
                                "gen_bnw_spots_1" => crate::layer::ProceduralGenerator::BnWSpots1,
                                "gen_bnw_spots_2" => crate::layer::ProceduralGenerator::BnWSpots2,
                                "gen_bnw_spots_3" => crate::layer::ProceduralGenerator::BnWSpots3,
                                "gen_grunge_map" => crate::layer::ProceduralGenerator::GrungeMap,
                                "gen_directional_noise" => crate::layer::ProceduralGenerator::DirectionalNoise,
                                "gen_clouds" => crate::layer::ProceduralGenerator::Clouds,
                                "gen_marble_veins" => crate::layer::ProceduralGenerator::MarbleVeins,
                                _ => crate::layer::ProceduralGenerator::PerlinNoise,
                            };
                            layer.add_mask_fill_effect_advanced(
                                gen,
                                curv.as_ref(),
                                ao.as_ref(),
                                pos.as_ref(),
                                wn.as_ref(),
                            );
                            self.status_message = format!("Added Fill Effect to Mask: {}", asset.name);
                        }
                    }
                } else {
                    let gen = match asset.id {
                        "gen_perlin" => crate::layer::ProceduralGenerator::PerlinNoise,
                        "gen_worley" => crate::layer::ProceduralGenerator::WorleyGrunge,
                        "gen_billow" => crate::layer::ProceduralGenerator::BillowNoise,
                        "gen_ridged" => crate::layer::ProceduralGenerator::RidgedMultifractal,
                        "gen_domain_warp" => crate::layer::ProceduralGenerator::DomainWarpGrunge,
                        "gen_splatter" => crate::layer::ProceduralGenerator::SplatterDirt,
                        "gen_wood" => crate::layer::ProceduralGenerator::WoodGrain,
                        "gen_fabric" => crate::layer::ProceduralGenerator::FabricWeave,
                        "gen_scratches" => crate::layer::ProceduralGenerator::Scratches,
                        "gen_curvature" => crate::layer::ProceduralGenerator::CurvatureWear,
                        "gen_cavity" => crate::layer::ProceduralGenerator::AmbientOcclusion,
                        "gen_dust" => crate::layer::ProceduralGenerator::DirectionalDust,
                        "gen_leaks" => crate::layer::ProceduralGenerator::DrippingLeaks,
                        "gen_position" | "gen_ground_dirt" => crate::layer::ProceduralGenerator::PositionGradient,
                        "gen_checker" => crate::layer::ProceduralGenerator::Checkerboard,
                        "gen_bricks" => crate::layer::ProceduralGenerator::Bricks,
                        "gen_perforated" => crate::layer::ProceduralGenerator::PerforatedMesh,
                        "gen_wire_fence" => crate::layer::ProceduralGenerator::WireFence,
                        "gen_torn_fabric" => crate::layer::ProceduralGenerator::TornFabricEdge,
                        "gen_dots" => crate::layer::ProceduralGenerator::CircularDots,
                        "gen_bnw_spots_1" => crate::layer::ProceduralGenerator::BnWSpots1,
                        "gen_bnw_spots_2" => crate::layer::ProceduralGenerator::BnWSpots2,
                        "gen_bnw_spots_3" => crate::layer::ProceduralGenerator::BnWSpots3,
                        "gen_grunge_map" => crate::layer::ProceduralGenerator::GrungeMap,
                        "gen_directional_noise" => crate::layer::ProceduralGenerator::DirectionalNoise,
                        "gen_clouds" => crate::layer::ProceduralGenerator::Clouds,
                        "gen_marble_veins" => crate::layer::ProceduralGenerator::MarbleVeins,
                        _ => crate::layer::ProceduralGenerator::PerlinNoise,
                    };
                    let mut fill_layer = crate::layer::Layer::new_fill_layer(
                        self.project.next_layer_id,
                        format!("Fill - {}", asset.name),
                        self.project.texture_width,
                        self.project.texture_height,
                        glam::Vec4::new(0.5, 0.5, 0.5, 1.0),
                        0.5,
                        0.0,
                    );
                    self.project.next_layer_id += 1;
                    fill_layer.fill_generator = gen;
                    fill_layer.generator_scale = 8.0;
                    fill_layer.evaluate_fill_advanced(
                        self.project.curvature_map.as_ref(),
                        self.project.position_map.as_ref(),
                        self.project.world_space_normal_map.as_ref(),
                        self.project.ao_map.as_ref(),
                    );
                    self.project.layers.push(fill_layer);
                    self.project.active_layer_idx = self.project.layers.len() - 1;
                    self.status_message = format!("Added Generator Fill Layer: {}", asset.name);
                }
            }
            crate::shelf::ShelfCategory::Alphas => {
                let alpha_type = match asset.id {
                    "alpha_soft_round" => crate::brush::AlphaType::SoftRound,
                    "alpha_hard_round" => crate::brush::AlphaType::HardRound,
                    "alpha_noise_splatter" => crate::brush::AlphaType::NoiseSplatter,
                    "alpha_chisel" => crate::brush::AlphaType::Chisel,
                    "alpha_stipple" => crate::brush::AlphaType::Stipple,
                    "alpha_scratches" => crate::brush::AlphaType::Scratches,
                    _ => crate::brush::AlphaType::SoftRound,
                };
                self.brush_engine.settings.alpha = alpha_type;
                self.brush_engine.update_cache();

                self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::CustomAlpha;
                self.brush_engine.settings.path_tool.custom_alpha = alpha_type;
                self.brush_engine.settings.path_tool.custom_resource_id = Some(asset.id.to_string());
                self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::CustomAlpha;
                self.brush_engine.settings.stamp_tool.custom_alpha = alpha_type;
                self.brush_engine.settings.stamp_tool.selected_preset_id = Some(asset.id.to_string());

                let is_path_layer = self.project.active_layer().map_or(false, |l| l.layer_type == crate::layer::LayerType::Path);
                if is_path_layer {
                    self.sync_and_evaluate_active_path_layer(ctx);
                }
                self.status_message = format!("Selected Brush Alpha: {}", asset.name);
            }
            crate::shelf::ShelfCategory::Decals => {
                if self.brush_engine.settings.mode != crate::brush::ToolMode::Stamp {
                    self.brush_engine.settings.mode = crate::brush::ToolMode::Path;
                }
                // Enable multi-channel PBR painting
                self.brush_engine.settings.channels.base_color = true;
                self.brush_engine.settings.channels.roughness = true;
                self.brush_engine.settings.channels.metallic = true;
                self.brush_engine.settings.channels.height = true;
                self.brush_engine.settings.channels.normal = true;

                if let Some(custom) = self.asset_manager.decals.iter().find(|d| d.id == asset.id) {
                    self.brush_engine.settings.path_tool.stroke_style = custom.stroke_style;
                    self.brush_engine.settings.path_tool.decal_type = custom.decal_type;
                    self.brush_engine.settings.path_tool.spacing_ratio = custom.spacing_ratio;
                    self.brush_engine.settings.path_tool.decal_spacing = custom.decal_spacing;
                    self.brush_engine.settings.path_tool.height_depth = custom.height_depth;
                    self.brush_engine.settings.path_tool.heat_tint = custom.heat_tint;
                    self.brush_engine.settings.path_tool.surface_offset = custom.surface_offset;
                    self.brush_engine.settings.stamp_tool.stroke_style = custom.stroke_style;
                    self.brush_engine.settings.stamp_tool.decal_type = custom.decal_type;
                    self.brush_engine.settings.stamp_tool.height_depth = custom.height_depth;
                    self.brush_engine.settings.stamp_tool.heat_tint = custom.heat_tint;
                    self.brush_engine.settings.stamp_tool.surface_offset = custom.surface_offset;
                    self.brush_engine.settings.stamp_tool.selected_preset_id = Some(custom.id.clone());
                    self.brush_engine.settings.radius = custom.radius;
                    self.brush_engine.settings.paint_color = glam::Vec4::new(custom.base_color[0], custom.base_color[1], custom.base_color[2], custom.base_color[3]);
                    self.brush_engine.settings.paint_roughness = custom.roughness;
                    self.brush_engine.settings.paint_metallic = custom.metallic;
                    self.brush_engine.settings.paint_emissive = glam::Vec4::new(custom.emissive[0], custom.emissive[1], custom.emissive[2], custom.emissive[3]);
                    self.brush_engine.settings.paint_opacity = custom.opacity;
                    self.brush_engine.settings.paint_clearcoat = custom.clearcoat;
                    self.brush_engine.settings.paint_anisotropy = custom.anisotropy;
                } else {
                    match asset.id {
                        "weld_seam" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::WeldSeam;
                            self.brush_engine.settings.path_tool.heat_tint = true;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::WeldSeam;
                            self.brush_engine.settings.stamp_tool.heat_tint = true;
                            self.brush_engine.settings.radius = 24.0;
                        }
                        "rope_braid" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::RopeBraid;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::RopeBraid;
                            self.brush_engine.settings.radius = 32.0;
                        }
                        "stitch_seam" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::StitchSeam;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::StitchSeam;
                            self.brush_engine.settings.radius = 16.0;
                        }
                        "chain_links" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::ChainLinks;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::ChainLinks;
                            self.brush_engine.settings.radius = 36.0;
                        }
                        "decal_button_hole" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.path_tool.decal_type = crate::brush::SmartDecalType::ButtonHole;
                            self.brush_engine.settings.path_tool.decal_spacing = 1.4;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.stamp_tool.decal_type = crate::brush::SmartDecalType::ButtonHole;
                            self.brush_engine.settings.radius = 28.0;
                        }
                        "decal_phillips_screw" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.path_tool.decal_type = crate::brush::SmartDecalType::PhillipsScrew;
                            self.brush_engine.settings.path_tool.decal_spacing = 1.2;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.stamp_tool.decal_type = crate::brush::SmartDecalType::PhillipsScrew;
                            self.brush_engine.settings.radius = 24.0;
                        }
                        "decal_flathead_screw" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.path_tool.decal_type = crate::brush::SmartDecalType::FlatheadScrew;
                            self.brush_engine.settings.path_tool.decal_spacing = 1.2;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.stamp_tool.decal_type = crate::brush::SmartDecalType::FlatheadScrew;
                            self.brush_engine.settings.radius = 24.0;
                        }
                        "decal_torx_screw" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.path_tool.decal_type = crate::brush::SmartDecalType::TorxScrew;
                            self.brush_engine.settings.path_tool.decal_spacing = 1.2;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.stamp_tool.decal_type = crate::brush::SmartDecalType::TorxScrew;
                            self.brush_engine.settings.radius = 24.0;
                        }
                        "decal_hex_bolt" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.path_tool.decal_type = crate::brush::SmartDecalType::HexBolt;
                            self.brush_engine.settings.path_tool.decal_spacing = 1.3;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.stamp_tool.decal_type = crate::brush::SmartDecalType::HexBolt;
                            self.brush_engine.settings.radius = 28.0;
                        }
                        "decal_domed_rivet" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.path_tool.decal_type = crate::brush::SmartDecalType::DomedRivet;
                            self.brush_engine.settings.path_tool.decal_spacing = 1.1;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.stamp_tool.decal_type = crate::brush::SmartDecalType::DomedRivet;
                            self.brush_engine.settings.radius = 20.0;
                        }
                        "decal_grommet" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.path_tool.decal_type = crate::brush::SmartDecalType::GrommetEyelet;
                            self.brush_engine.settings.path_tool.decal_spacing = 1.3;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.stamp_tool.decal_type = crate::brush::SmartDecalType::GrommetEyelet;
                            self.brush_engine.settings.radius = 28.0;
                        }
                        "decal_air_vent" => {
                            self.brush_engine.settings.path_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.path_tool.decal_type = crate::brush::SmartDecalType::AirVentLouver;
                            self.brush_engine.settings.path_tool.decal_spacing = 1.5;
                            self.brush_engine.settings.stamp_tool.stroke_style = crate::brush::PathStrokeStyle::SmartDecal;
                            self.brush_engine.settings.stamp_tool.decal_type = crate::brush::SmartDecalType::AirVentLouver;
                            self.brush_engine.settings.radius = 36.0;
                        }
                        _ => {}
                    }
                }
                let is_path_layer = self.project.active_layer().map_or(false, |l| l.layer_type == crate::layer::LayerType::Path);
                if is_path_layer {
                    self.sync_and_evaluate_active_path_layer(ctx);
                }
                self.status_message = format!("Selected Decal Preset: {}", asset.name);
            }
            crate::shelf::ShelfCategory::Environments => {}
            _ => {}
        }
        self.project.rebuild_composite();
        self.project.gpu_sync_needed = true;
        self.update_uv_texture(ctx);
        if let Some(wgpu) = &mut self.wgpu_viewport {
            wgpu.sync_textures(&mut self.project);
        }
        ctx.request_repaint();
    }

    /// Load an HDRI environment from file and update the viewport environment map
    pub fn activate_hdri_file(&mut self, path: &std::path::Path) -> bool {
        if let Ok(()) = self.renderer.hdri.load_file(path) {
            if let Some(wgpu) = &mut self.wgpu_viewport {
                let (w, h, hdri_data) = crate::hdri_generator::generate_hdri_texture(&self.renderer.hdri, 1024, 512);
                wgpu.load_environment(w, h, &hdri_data);
            }
            if let Some(idx) = self.asset_manager.hdris.iter().position(|h| h.path == path) {
                self.asset_manager.active_hdri_index = idx;
            }
            self.status_message = format!("Active HDRI: {}", self.renderer.hdri.preset.name());
            true
        } else {
            self.status_message = format!("Failed to load HDRI: {:?}", path.file_name().unwrap_or_default());
            false
        }
    }
}

/// Helper function to render stylized HDRI preview thumbnail on the asset shelf card
pub(crate) fn draw_hdri_thumbnail(painter: &egui::Painter, rect: egui::Rect, id: &str, name: &str) {
    let lower = name.to_lowercase();
    let id_lower = id.to_lowercase();

    if lower.contains("studio") || id_lower.contains("studio") || lower.contains("softbox") {
        // Studio with dual softboxes
        painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(28, 30, 36));
        let sb1 = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + rect.width() * 0.15, rect.min.y + rect.height() * 0.2),
            egui::pos2(rect.min.x + rect.width() * 0.38, rect.min.y + rect.height() * 0.8),
        );
        painter.rect_filled(sb1, 2.0, egui::Color32::from_rgb(240, 245, 255));
        let sb2 = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + rect.width() * 0.62, rect.min.y + rect.height() * 0.2),
            egui::pos2(rect.min.x + rect.width() * 0.85, rect.min.y + rect.height() * 0.8),
        );
        painter.rect_filled(sb2, 2.0, egui::Color32::from_rgb(255, 245, 230));
    } else if lower.contains("sunny") || lower.contains("courtyard") || lower.contains("outdoor") {
        // Sunny courtyard sky & ground
        let half_h = rect.height() * 0.55;
        let sky_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + half_h));
        let ground_rect = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + half_h), rect.max);
        painter.rect_filled(sky_rect, 0.0, egui::Color32::from_rgb(60, 130, 215));
        painter.rect_filled(ground_rect, 0.0, egui::Color32::from_rgb(75, 90, 70));
        painter.circle_filled(egui::pos2(rect.min.x + rect.width() * 0.7, rect.min.y + rect.height() * 0.3), 6.0, egui::Color32::from_rgb(255, 250, 210));
    } else if lower.contains("sunset") || lower.contains("desert") || lower.contains("golden") {
        // Sunset desert twilight
        let half_h = rect.height() * 0.6;
        let sky_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + half_h));
        let ground_rect = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + half_h), rect.max);
        painter.rect_filled(sky_rect, 0.0, egui::Color32::from_rgb(70, 30, 95));
        painter.rect_filled(ground_rect, 0.0, egui::Color32::from_rgb(180, 85, 30));
        painter.circle_filled(egui::pos2(rect.min.x + rect.width() * 0.5, rect.min.y + half_h), 8.0, egui::Color32::from_rgb(255, 200, 70));
    } else if lower.contains("industrial") || lower.contains("dark") || lower.contains("factory") {
        // Dark industrial workshop
        painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(20, 24, 28));
        let tube = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + rect.width() * 0.15, rect.min.y + rect.height() * 0.25),
            egui::pos2(rect.max.x - rect.width() * 0.15, rect.min.y + rect.height() * 0.35),
        );
        painter.rect_filled(tube, 1.0, egui::Color32::from_rgb(210, 230, 255));
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x + rect.width() * 0.2, rect.max.y - rect.height() * 0.3),
                egui::pos2(rect.max.x - rect.width() * 0.2, rect.max.y - rect.height() * 0.15),
            ),
            1.0,
            egui::Color32::from_rgb(140, 75, 40),
        );
    } else if lower.contains("neon") || lower.contains("cyberpunk") {
        // Cyberpunk neon
        painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(15, 12, 24));
        painter.line_segment(
            [egui::pos2(rect.min.x + rect.width() * 0.25, rect.min.y + 4.0), egui::pos2(rect.min.x + rect.width() * 0.25, rect.max.y - 4.0)],
            egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(255, 40, 160)),
        );
        painter.line_segment(
            [egui::pos2(rect.max.x - rect.width() * 0.25, rect.min.y + 4.0), egui::pos2(rect.max.x - rect.width() * 0.25, rect.max.y - 4.0)],
            egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(30, 220, 245)),
        );
    } else {
        // Generic panoramic environment
        painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(35, 40, 50));
        let center = rect.center();
        let radius = (rect.width() * 0.35).min(rect.height() * 0.35);
        painter.circle_filled(center, radius, egui::Color32::from_rgb(80, 140, 210));
        painter.circle_filled(center - egui::vec2(radius * 0.3, radius * 0.3), radius * 0.3, egui::Color32::WHITE);
    }
}

