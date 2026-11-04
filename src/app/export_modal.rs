use super::*;
use eframe::egui;

impl PaintcraftApp {
    pub fn render_export_modal(&mut self, ctx: &egui::Context) {
        let mut close_modal = false;
        let mut status_update = None;

        let screen = ctx.screen_rect();
        let def_w = 880.0f32.min(screen.width() * 0.85).max(520.0);
        let def_h = 600.0f32.min(screen.height() * 0.82).max(380.0);
        let min_w = 580.0f32.min(screen.width() * 0.9);
        let min_h = 360.0f32.min(screen.height() * 0.85);
        let max_w = (screen.width() - 32.0).max(400.0);
        let max_h = (screen.height() - 32.0).max(350.0);

        egui::Window::new("Export Textures")
            .collapsible(false)
            .resizable(true)
            .default_size([def_w, def_h])
            .min_width(min_w)
            .min_height(min_h)
            .max_size([max_w, max_h])
            .show(ctx, |ui| {
                // Top Tab Navigation Bar (Substance 3D Painter Standard)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    let is_settings = self.export_tab_idx == 0;
                    let is_templates = self.export_tab_idx == 1;
                    let is_exports = self.export_tab_idx == 2;

                    if ui.selectable_label(is_settings, egui::RichText::new("Settings").strong().size(14.0)).clicked() {
                        self.export_tab_idx = 0;
                    }
                    if ui.selectable_label(is_templates, egui::RichText::new("Output Templates").strong().size(14.0)).clicked() {
                        self.export_tab_idx = 1;
                    }
                    let count = self.export_last_exported_files.len();
                    let exports_label = if count > 0 {
                        format!("List of Exports ({})", count)
                    } else {
                        "List of Exports".to_string()
                    };
                    if ui.selectable_label(is_exports, egui::RichText::new(exports_label).strong().size(14.0)).clicked() {
                        self.export_tab_idx = 2;
                    }
                });
                ui.separator();

                // TAB 0: SETTINGS (Main Export Dashboard)
                if self.export_tab_idx == 0 {
                    ui.columns(2, |columns| {
                        // Left Column: Texture Sets
                        columns[0].vertical(|ui| {
                            ui.set_max_width(280.0);
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.heading("Texture Sets");
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("None").on_hover_text("Deselect all texture sets").clicked() {
                                            for ts in &mut self.project.texture_sets {
                                                ts.visible = false;
                                            }
                                        }
                                        if ui.small_button("All").on_hover_text("Select all texture sets").clicked() {
                                            for ts in &mut self.project.texture_sets {
                                                ts.visible = true;
                                            }
                                        }
                                    });
                                });
                                ui.separator();

                                if self.project.texture_sets.is_empty() {
                                    ui.label("No texture sets defined");
                                } else {
                                    let mut focused_idx = self.export_selected_ts_idx;
                                    egui::ScrollArea::vertical()
                                        .max_height(340.0)
                                        .id_source("export_texture_sets_scroll")
                                        .show(ui, |ui| {
                                            for (idx, ts) in self.project.texture_sets.iter_mut().enumerate() {
                                                let is_focused = focused_idx == idx;
                                                ui.horizontal(|ui| {
                                                    ui.checkbox(&mut ts.visible, "");
                                                    let resp = ui.selectable_label(
                                                        is_focused,
                                                        egui::RichText::new(&ts.name).strong(),
                                                    );
                                                    if resp.clicked() {
                                                        focused_idx = idx;
                                                    }
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.label(
                                                            egui::RichText::new(format!("{}x{}", ts.width, ts.height))
                                                                .small()
                                                                .color(egui::Color32::from_rgb(160, 160, 160)),
                                                        );
                                                    });
                                                });
                                            }
                                        });
                                    self.export_selected_ts_idx = focused_idx;
                                }
                            });
                        });

                        // Right Column: Global Settings & Output Maps Preview
                        columns[1].vertical(|ui| {
                            ui.group(|ui| {
                                ui.heading("Global Settings");
                                ui.separator();

                                egui::Grid::new("export_settings_grid")
                                    .num_columns(2)
                                    .spacing([12.0, 8.0])
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new("Output Directory:").strong());
                                        ui.horizontal(|ui| {
                                            ui.add(egui::TextEdit::singleline(&mut self.export_dir).desired_width(230.0));
                                            if ui.button("Browse...").on_hover_text("Select export output directory using system folder picker").clicked() {
                                                let initial_dir = PathBuf::from(&self.export_dir);
                                                let mut dialog = rfd::FileDialog::new().set_title("Select Export Directory");
                                                if initial_dir.is_dir() {
                                                    dialog = dialog.set_directory(&initial_dir);
                                                } else if let Some(parent) = initial_dir.parent() {
                                                    if parent.is_dir() {
                                                        dialog = dialog.set_directory(parent);
                                                    }
                                                }
                                                if let Some(folder) = dialog.pick_folder() {
                                                    self.export_dir = folder.to_string_lossy().to_string();
                                                }
                                            }
                                            if ui.button("Open Folder").on_hover_text("Open export folder in system file manager").clicked() {
                                                let p = PathBuf::from(&self.export_dir);
                                                let _ = std::fs::create_dir_all(&p);
                                                open_folder_in_desktop(&p);
                                            }
                                        });
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Output Template:").strong());
                                        let current_template_name = self
                                            .custom_packing_presets
                                            .get(self.active_custom_packing_idx)
                                            .map(|p| p.name.clone())
                                            .unwrap_or_else(|| "None".to_string());

                                        egui::ComboBox::from_id_source("settings_output_template_combo")
                                            .selected_text(&current_template_name)
                                            .width(280.0)
                                            .show_ui(ui, |ui| {
                                                for (idx, p) in self.custom_packing_presets.iter().enumerate() {
                                                    ui.selectable_value(&mut self.active_custom_packing_idx, idx, &p.name);
                                                }
                                            });
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Resolution Override:").strong());
                                        egui::ComboBox::from_id_source("settings_res_override_combo")
                                            .selected_text(match self.export_resolution_override {
                                                0 => "Based on each texture set's size",
                                                512 => "512 x 512",
                                                1024 => "1024 x 1024 (1K)",
                                                2048 => "2048 x 2048 (2K)",
                                                4096 => "4096 x 4096 (4K)",
                                                _ => "Custom",
                                            })
                                            .width(280.0)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut self.export_resolution_override, 0, "Based on each texture set's size");
                                                ui.selectable_value(&mut self.export_resolution_override, 512, "512 x 512");
                                                ui.selectable_value(&mut self.export_resolution_override, 1024, "1024 x 1024 (1K)");
                                                ui.selectable_value(&mut self.export_resolution_override, 2048, "2048 x 2048 (2K)");
                                                ui.selectable_value(&mut self.export_resolution_override, 4096, "4096 x 4096 (4K)");
                                            });
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Padding / Dilation:").strong());
                                        ui.add(egui::Slider::new(&mut self.export_padding_px, 0..=32).text("pixels"));
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Export 3D Model:").strong());
                                        ui.checkbox(&mut self.export_mesh, "Include 3D mesh files (.obj, .mtl, .glb)")
                                            .on_hover_text("When checked, exports 3D mesh geometry files alongside texture maps. When unchecked, exports texture maps only.");
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Output Format:").strong());
                                        egui::ComboBox::from_id_source("settings_output_format_combo")
                                            .selected_text(self.export_image_format.name())
                                            .width(280.0)
                                            .show_ui(ui, |ui| {
                                                for fmt in crate::packing::PackedImageFormat::all() {
                                                    ui.selectable_value(&mut self.export_image_format, *fmt, fmt.name());
                                                }
                                            });
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Coordinate System:").strong());
                                        ui.checkbox(&mut self.export_flip_v, "Flip Y / V-Axis (DirectX / OBJ UV origin)")
                                            .on_hover_text("Inverts vertical texture coordinates to conform to DirectX and standard DCC UV orientations.");
                                        ui.end_row();

                                        ui.label(egui::RichText::new("UDIM Tiles:").strong());
                                        ui.checkbox(&mut self.export_udim_naming, "UDIM Tile Naming (<UDIM>)")
                                            .on_hover_text("Append standard .<UDIM> tile numbering (e.g. .1001, .1002) before file extension for VFX/Film DCC pipelines");
                                        ui.end_row();
                                    });
                            });

                            ui.add_space(8.0);

                            // Output Maps Preview for selected texture set
                            ui.group(|ui| {
                                let focused_ts_name = self.project.texture_sets.get(self.export_selected_ts_idx)
                                    .map(|ts| ts.name.as_str())
                                    .unwrap_or("Active Project");

                                ui.horizontal(|ui| {
                                    ui.heading(format!("Output Maps for '{}':", focused_ts_name));
                                    if let Some(cur_preset) = self.custom_packing_presets.get(self.active_custom_packing_idx) {
                                        let active_count = cur_preset.maps.iter().filter(|m| m.enabled).count();
                                        ui.label(
                                            egui::RichText::new(format!("({} maps active)", active_count))
                                                .color(egui::Color32::from_rgb(140, 200, 255)),
                                        );
                                    }
                                });
                                ui.separator();

                                if let Some(cur_preset) = self.custom_packing_presets.get(self.active_custom_packing_idx) {
                                    let mesh_clean = self.project.mesh.name.replace(' ', "_").to_lowercase();
                                    let ts_clean = focused_ts_name.replace(' ', "_");
                                    let ext = self.export_image_format.extension();

                                    egui::ScrollArea::vertical()
                                        .max_height(160.0)
                                        .id_source("export_preview_scroll")
                                        .show(ui, |ui| {
                                            if self.export_mesh {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("[3D]")
                                                            .monospace()
                                                            .color(egui::Color32::from_rgb(120, 220, 140)),
                                                    );
                                                    ui.label(egui::RichText::new(format!("{}.obj, {}.mtl, {}.glb", mesh_clean, mesh_clean, mesh_clean)).strong());
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.label(
                                                            egui::RichText::new("3D Geometry + MTL")
                                                                .small()
                                                                .color(egui::Color32::from_rgb(180, 180, 180)),
                                                        );
                                                    });
                                                });
                                            }
                                            for map in &cur_preset.maps {
                                                if !map.enabled {
                                                    continue;
                                                }
                                                let udim_suffix = if self.export_udim_naming { ".1001" } else { "" };
                                                let file_name = if self.project.texture_sets.len() > 1 {
                                                    format!("{}_{}{}{}.{}", mesh_clean, ts_clean, map.name_suffix, udim_suffix, ext)
                                                } else {
                                                    format!("{}{}{}.{}", mesh_clean, map.name_suffix, udim_suffix, ext)
                                                };

                                                ui.horizontal(|ui| {
                                                    let mode_badge = match map.color_mode {
                                                        crate::packing::PackedColorMode::Rgb => "RGB",
                                                        crate::packing::PackedColorMode::Rgba => "RGBA",
                                                        crate::packing::PackedColorMode::Grayscale => "GRAY",
                                                    };
                                                    ui.label(
                                                        egui::RichText::new(format!("[{}]", mode_badge))
                                                            .monospace()
                                                            .color(egui::Color32::from_rgb(100, 200, 255)),
                                                    );
                                                    ui.label(egui::RichText::new(&file_name).strong());
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.label(
                                                            egui::RichText::new(map.channel_summary())
                                                                .small()
                                                                .color(egui::Color32::from_rgb(180, 180, 180)),
                                                        );
                                                    });
                                                });
                                            }
                                        });
                                }
                            });

                            ui.add_space(8.0);

                            // Bottom Action Bar
                            ui.horizontal(|ui| {
                                let selected_ts_count = self.project.texture_sets.iter().filter(|ts| ts.visible).count();
                                ui.label(
                                    egui::RichText::new(format!("Selected Texture Sets: {} of {}", selected_ts_count, self.project.texture_sets.len()))
                                        .strong(),
                                );

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // Primary Export Button (Substance standard high-visibility button)
                                    let can_export = selected_ts_count > 0 || self.project.texture_sets.is_empty();
                                    let export_btn = egui::Button::new(
                                        egui::RichText::new("  Export  ").strong().color(egui::Color32::WHITE),
                                    )
                                    .fill(if can_export {
                                        egui::Color32::from_rgb(40, 120, 220)
                                    } else {
                                        egui::Color32::from_rgb(70, 75, 85)
                                    });

                                    let resp = ui.add_enabled(can_export, export_btn);
                                    if !can_export {
                                        let _ = resp.clone().on_hover_text("Select at least one texture set to export");
                                    }

                                    if resp.clicked() {
                                        let out_path = PathBuf::from(&self.export_dir);
                                        let _ = std::fs::create_dir_all(&out_path);
                                        let mut all_exported = Vec::new();

                                        let udim_tiles = if self.export_udim_naming {
                                            self.project.mesh.get_udim_tiles()
                                        } else {
                                            vec![1001]
                                        };

                                        // Step 0: Fully rebuild composite for active canvas and synchronize active texture set
                                        self.project.rebuild_composite();
                                        self.project.sync_active_texture_set();

                                        let ts_count = self.project.texture_sets.len();

                                        // Step 1: Pre-composite all layers for visible texture sets
                                        if !self.project.texture_sets.is_empty() {
                                            for (ts_idx, ts) in self.project.texture_sets.iter_mut().enumerate() {
                                                if ts.visible {
                                                    if ts_idx == self.project.active_texture_set_idx {
                                                        ts.composite_textures = self.project.composite_textures.clone();
                                                    } else {
                                                        ts.composite_textures.composite_layers(&ts.layers);
                                                    }
                                                }
                                            }
                                        } else {
                                            self.project.composite_textures.composite_layers(&self.project.layers);
                                        }

                                        // Step 2: Borrow baked context immutably
                                        let baked = self.project.baked_map_context();
                                        let mesh_name = self.project.mesh.name.clone();

                                        for (tile_idx, &tile_id) in udim_tiles.iter().enumerate() {
                                            let mut active_preset = self
                                                .custom_packing_presets
                                                .get(self.active_custom_packing_idx)
                                                .cloned()
                                                .unwrap_or_else(crate::packing::CustomPackingPreset::unreal_orm);

                                            // Apply global padding, format override, and UDIM tile suffix if configured
                                            for m in &mut active_preset.maps {
                                                m.dilation_iterations = self.export_padding_px;
                                                m.format = self.export_image_format;
                                                if self.export_udim_naming {
                                                    m.name_suffix = format!("{}.{}", m.name_suffix, tile_id);
                                                }
                                            }

                                            if !self.project.texture_sets.is_empty() {
                                                for (ts_idx, ts) in self.project.texture_sets.iter().enumerate() {
                                                    if ts.visible {
                                                        let prefix = if ts_count > 1 {
                                                            format!("{}_{}", mesh_name, ts.name)
                                                        } else {
                                                            mesh_name.clone()
                                                        };

                                                        let textures_to_export = if ts_idx == self.project.active_texture_set_idx {
                                                            &self.project.composite_textures
                                                        } else {
                                                            &ts.composite_textures
                                                        };

                                                        match crate::packing::CustomTexturePacker::pack_and_export(
                                                            &active_preset,
                                                            textures_to_export,
                                                            &prefix,
                                                            &out_path,
                                                            self.export_flip_v,
                                                            if self.export_mesh && ts_idx == 0 && tile_idx == 0 { Some(&self.project.mesh) } else { None },
                                                            &baked,
                                                        ) {
                                                            Ok(mut files) => all_exported.append(&mut files),
                                                            Err(e) => {
                                                                status_update = Some(format!("Export error on {}: {}", ts.name, e));
                                                            }
                                                        }
                                                    }
                                                }
                                            } else {
                                                match crate::packing::CustomTexturePacker::pack_and_export(
                                                    &active_preset,
                                                    &self.project.composite_textures,
                                                    &mesh_name,
                                                    &out_path,
                                                    self.export_flip_v,
                                                    if self.export_mesh && tile_idx == 0 { Some(&self.project.mesh) } else { None },
                                                    &baked,
                                                ) {
                                                    Ok(mut files) => all_exported.append(&mut files),
                                                    Err(e) => {
                                                        status_update = Some(format!("Export error: {}", e));
                                                    }
                                                }
                                            }
                                        }

                                        if !all_exported.is_empty() {
                                            self.export_last_exported_files = all_exported.clone();
                                            status_update = Some(format!("Successfully exported {} production maps to {}", all_exported.len(), self.export_dir));
                                            self.export_tab_idx = 2; // Automatically switch to List of Exports tab!
                                        }
                                    }

                                    if ui.button("Cancel").clicked() {
                                        close_modal = true;
                                    }
                                });
                            });
                        });
                    });
                }

                // TAB 1: OUTPUT TEMPLATES (Substance 3D Painter Channel Packing Matrix)
                if self.export_tab_idx == 1 {
                    ui.columns(2, |columns| {
                        // Left Column: Templates Library
                        columns[0].vertical(|ui| {
                            ui.set_max_width(260.0);
                            ui.group(|ui| {
                                ui.heading("Output Templates");
                                ui.separator();

                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Filter:").small().strong());
                                    ui.add(egui::TextEdit::singleline(&mut self.export_template_search).hint_text("Search templates..."));
                                });
                                ui.add_space(4.0);

                                let preset_count = self.custom_packing_presets.len();
                                if self.active_custom_packing_idx >= preset_count && preset_count > 0 {
                                    self.active_custom_packing_idx = 0;
                                }

                                egui::ScrollArea::vertical()
                                    .max_height(280.0)
                                    .id_source("substance_templates_list_scroll")
                                    .show(ui, |ui| {
                                        let search_lower = self.export_template_search.to_lowercase();
                                        for (idx, p) in self.custom_packing_presets.iter().enumerate() {
                                            if !search_lower.is_empty() && !p.name.to_lowercase().contains(&search_lower) {
                                                continue;
                                            }
                                            let is_selected = self.active_custom_packing_idx == idx;
                                            let map_count = p.maps.len();
                                            let row_label = format!("{} ({} maps)", p.name, map_count);

                                            if ui.selectable_label(is_selected, row_label).clicked() {
                                                self.active_custom_packing_idx = idx;
                                            }
                                        }
                                    });

                                ui.separator();
                                // Template management toolbar
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("+ New").on_hover_text("Create a new blank template").clicked() {
                                        let new_preset = crate::packing::CustomPackingPreset {
                                            id: format!("custom_{}", chrono::Utc::now().timestamp_millis()),
                                            name: "New Output Template".to_string(),
                                            description: "Custom user-defined channel matrix".to_string(),
                                            maps: vec![
                                                crate::packing::PackedTextureMap::new_rgb("_BaseColor", crate::packing::ChannelSource::BaseColorR, crate::packing::ChannelSource::BaseColorG, crate::packing::ChannelSource::BaseColorB),
                                                crate::packing::PackedTextureMap::new_rgb("_Normal", crate::packing::ChannelSource::NormalX, crate::packing::ChannelSource::NormalY, crate::packing::ChannelSource::NormalZ),
                                                crate::packing::PackedTextureMap::new_rgb("_ORM", crate::packing::ChannelSource::AmbientOcclusion, crate::packing::ChannelSource::Roughness, crate::packing::ChannelSource::Metallic),
                                            ],
                                        };
                                        self.custom_packing_presets.push(new_preset);
                                        self.active_custom_packing_idx = self.custom_packing_presets.len() - 1;
                                    }

                                    if ui.button("Duplicate").clicked() {
                                        if let Some(cur) = self.custom_packing_presets.get(self.active_custom_packing_idx) {
                                            let mut dup = cur.clone();
                                            dup.id = format!("{}_copy", dup.id);
                                            dup.name = format!("{} (Copy)", dup.name);
                                            self.custom_packing_presets.push(dup);
                                            self.active_custom_packing_idx = self.custom_packing_presets.len() - 1;
                                        }
                                    }

                                    if preset_count > 1 && ui.button("Delete").clicked() {
                                        self.custom_packing_presets.remove(self.active_custom_packing_idx);
                                        if self.active_custom_packing_idx >= self.custom_packing_presets.len() {
                                            self.active_custom_packing_idx = self.custom_packing_presets.len().saturating_sub(1);
                                        }
                                    }

                                    if ui.button("Reset Defaults").clicked() {
                                        self.custom_packing_presets = crate::packing::CustomPackingPreset::default_presets();
                                        self.active_custom_packing_idx = 0;
                                        self.custom_packing_status = Some("Templates reset to factory defaults.".to_string());
                                    }
                                });

                                ui.add_space(4.0);
                                ui.separator();
                                ui.label(egui::RichText::new("Preset JSON I/O:").small().strong());
                                ui.add(egui::TextEdit::singleline(&mut self.custom_preset_io_filename).desired_width(220.0));
                                ui.horizontal(|ui| {
                                    if ui.small_button("Save JSON").clicked() {
                                        if let Some(cur) = self.custom_packing_presets.get(self.active_custom_packing_idx) {
                                            let out_dir = PathBuf::from(&self.export_dir);
                                            let _ = std::fs::create_dir_all(&out_dir);
                                            let path = out_dir.join(&self.custom_preset_io_filename);
                                            match cur.save_to_json_file(&path) {
                                                Ok(_) => self.custom_packing_status = Some(format!("Saved: {:?}", path)),
                                                Err(e) => self.custom_packing_status = Some(format!("Error: {}", e)),
                                            }
                                        }
                                    }
                                    if ui.small_button("Load JSON").clicked() {
                                        let out_dir = PathBuf::from(&self.export_dir);
                                        let path = out_dir.join(&self.custom_preset_io_filename);
                                        match crate::packing::CustomPackingPreset::load_from_json_file(&path) {
                                            Ok(loaded) => {
                                                self.custom_packing_presets.push(loaded);
                                                self.active_custom_packing_idx = self.custom_packing_presets.len() - 1;
                                                self.custom_packing_status = Some(format!("Loaded: {:?}", path));
                                            }
                                            Err(e) => self.custom_packing_status = Some(format!("Error: {}", e)),
                                        }
                                    }
                                });
                            });
                        });

                        // Right Column: Active Template Editor & Channel Boxes
                        columns[1].vertical(|ui| {
                            if let Some(cur) = self.custom_packing_presets.get_mut(self.active_custom_packing_idx) {
                                ui.group(|ui| {
                                    // Template Header with quick-add buttons
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Name:").strong());
                                        ui.add(egui::TextEdit::singleline(&mut cur.name).desired_width(240.0));

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            // Substance standard quick-add buttons
                                            if ui.button("+ Gray").on_hover_text("Add a Grayscale (single channel) map").clicked() {
                                                cur.maps.push(crate::packing::PackedTextureMap::new_grayscale(
                                                    "_Mask",
                                                    crate::packing::ChannelSource::Roughness,
                                                ));
                                            }
                                            if ui.button("+ RGBA").on_hover_text("Add a 4-channel RGBA map").clicked() {
                                                cur.maps.push(crate::packing::PackedTextureMap::new_rgba(
                                                    "_MaskMap",
                                                    crate::packing::ChannelSource::Metallic,
                                                    crate::packing::ChannelSource::AmbientOcclusion,
                                                    crate::packing::ChannelSource::BakedCurvature,
                                                    crate::packing::ChannelSource::Roughness,
                                                ));
                                            }
                                            if ui.button("+ RGB").on_hover_text("Add a 3-channel RGB map").clicked() {
                                                cur.maps.push(crate::packing::PackedTextureMap::new_rgb(
                                                    "_ORM",
                                                    crate::packing::ChannelSource::AmbientOcclusion,
                                                    crate::packing::ChannelSource::Roughness,
                                                    crate::packing::ChannelSource::Metallic,
                                                ));
                                            }
                                            ui.label(egui::RichText::new("Add Map:").strong());
                                        });
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Description:").small());
                                        ui.add(egui::TextEdit::singleline(&mut cur.description).desired_width(400.0));
                                    });
                                });

                                ui.add_space(6.0);

                                let mut map_to_delete: Option<usize> = None;
                                let mut map_to_duplicate: Option<usize> = None;

                                // Output Maps Cards Scroll Area
                                egui::ScrollArea::vertical()
                                    .max_height(280.0)
                                    .id_source("substance_maps_cards_scroll")
                                    .show(ui, |ui| {
                                        for (m_idx, map) in cur.maps.iter_mut().enumerate() {
                                            egui::Frame::none()
                                                .fill(egui::Color32::from_rgb(26, 26, 26))
                                                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(50, 50, 50)))
                                                .rounding(6.0)
                                                .inner_margin(egui::Margin::same(8.0))
                                                .show(ui, |ui| {
                                                    // Map Header Line
                                                    ui.horizontal(|ui| {
                                                        ui.checkbox(&mut map.enabled, "");
                                                        ui.label(egui::RichText::new("Suffix:").strong());
                                                        ui.add(egui::TextEdit::singleline(&mut map.name_suffix).desired_width(140.0));

                                                        egui::ComboBox::from_id_source(format!("substance_colormode_{}", m_idx))
                                                            .selected_text(map.color_mode.name())
                                                            .width(110.0)
                                                            .show_ui(ui, |ui| {
                                                                ui.selectable_value(&mut map.color_mode, crate::packing::PackedColorMode::Rgb, "RGB");
                                                                ui.selectable_value(&mut map.color_mode, crate::packing::PackedColorMode::Rgba, "RGBA");
                                                                ui.selectable_value(&mut map.color_mode, crate::packing::PackedColorMode::Grayscale, "Grayscale");
                                                            });

                                                        egui::ComboBox::from_id_source(format!("substance_format_{}", m_idx))
                                                            .selected_text(map.format.name())
                                                            .width(100.0)
                                                            .show_ui(ui, |ui| {
                                                                for fmt in crate::packing::PackedImageFormat::all() {
                                                                    ui.selectable_value(&mut map.format, *fmt, fmt.name());
                                                                }
                                                            });

                                                        ui.label("Pad:");
                                                        ui.add(egui::Slider::new(&mut map.dilation_iterations, 0..=32).text("px"));

                                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                            if ui.small_button("X").on_hover_text("Delete this map").clicked() {
                                                                map_to_delete = Some(m_idx);
                                                            }
                                                            if ui.small_button("Dup").on_hover_text("Duplicate map").clicked() {
                                                                map_to_duplicate = Some(m_idx);
                                                            }
                                                        });
                                                    });

                                                    ui.add_space(4.0);

                                                    // Substance Painter Channel Slot Boxes (Color-Accented Matrix)
                                                    ui.horizontal(|ui| {
                                                        match map.color_mode {
                                                            crate::packing::PackedColorMode::Grayscale => {
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Gray",
                                                                    &mut map.channel_r,
                                                                    m_idx,
                                                                    "gray",
                                                                    egui::Color32::from_rgb(32, 32, 32),
                                                                    egui::Color32::from_rgb(100, 100, 100),
                                                                    egui::Color32::from_rgb(220, 220, 220),
                                                                );
                                                            }
                                                            crate::packing::PackedColorMode::Rgb => {
                                                                // Red Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Red (R)",
                                                                    &mut map.channel_r,
                                                                    m_idx,
                                                                    "r",
                                                                    egui::Color32::from_rgb(45, 20, 20),
                                                                    egui::Color32::from_rgb(140, 40, 40),
                                                                    egui::Color32::from_rgb(255, 120, 120),
                                                                );
                                                                // Green Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Green (G)",
                                                                    &mut map.channel_g,
                                                                    m_idx,
                                                                    "g",
                                                                    egui::Color32::from_rgb(20, 45, 20),
                                                                    egui::Color32::from_rgb(40, 140, 40),
                                                                    egui::Color32::from_rgb(120, 255, 120),
                                                                );
                                                                // Blue Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Blue (B)",
                                                                    &mut map.channel_b,
                                                                    m_idx,
                                                                    "b",
                                                                    egui::Color32::from_rgb(20, 30, 50),
                                                                    egui::Color32::from_rgb(40, 80, 160),
                                                                    egui::Color32::from_rgb(120, 180, 255),
                                                                );
                                                            }
                                                            crate::packing::PackedColorMode::Rgba => {
                                                                // Red Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Red (R)",
                                                                    &mut map.channel_r,
                                                                    m_idx,
                                                                    "r",
                                                                    egui::Color32::from_rgb(45, 20, 20),
                                                                    egui::Color32::from_rgb(140, 40, 40),
                                                                    egui::Color32::from_rgb(255, 120, 120),
                                                                );
                                                                // Green Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Green (G)",
                                                                    &mut map.channel_g,
                                                                    m_idx,
                                                                    "g",
                                                                    egui::Color32::from_rgb(20, 45, 20),
                                                                    egui::Color32::from_rgb(40, 140, 40),
                                                                    egui::Color32::from_rgb(120, 255, 120),
                                                                );
                                                                // Blue Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Blue (B)",
                                                                    &mut map.channel_b,
                                                                    m_idx,
                                                                    "b",
                                                                    egui::Color32::from_rgb(20, 30, 50),
                                                                    egui::Color32::from_rgb(40, 80, 160),
                                                                    egui::Color32::from_rgb(120, 180, 255),
                                                                );
                                                                // Alpha Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Alpha (A)",
                                                                    &mut map.channel_a,
                                                                    m_idx,
                                                                    "a",
                                                                    egui::Color32::from_rgb(35, 35, 35),
                                                                    egui::Color32::from_rgb(100, 100, 100),
                                                                    egui::Color32::from_rgb(220, 220, 220),
                                                                );
                                                            }
                                                        }
                                                    });
                                                });
                                            ui.add_space(4.0);
                                        }
                                    });

                                if let Some(del_idx) = map_to_delete {
                                    if cur.maps.len() > 1 {
                                        cur.maps.remove(del_idx);
                                    }
                                }

                                if let Some(dup_idx) = map_to_duplicate {
                                    if let Some(src_map) = cur.maps.get(dup_idx) {
                                        let mut cloned_map = src_map.clone();
                                        cloned_map.name_suffix = format!("{}_copy", cloned_map.name_suffix);
                                        cur.maps.insert(dup_idx + 1, cloned_map);
                                    }
                                }

                                ui.add_space(6.0);

                                // Substance Painter Input Maps & Mesh Maps Reference Shelf
                                egui::CollapsingHeader::new(egui::RichText::new("Input Maps & Mesh Maps Palette").strong())
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(egui::RichText::new("Input Channels:").small().strong());
                                            for &src in &[
                                                crate::packing::ChannelSource::BaseColorR,
                                                crate::packing::ChannelSource::Roughness,
                                                crate::packing::ChannelSource::Metallic,
                                                crate::packing::ChannelSource::NormalX,
                                                crate::packing::ChannelSource::NormalY,
                                                crate::packing::ChannelSource::NormalYInverted,
                                                crate::packing::ChannelSource::Height,
                                                crate::packing::ChannelSource::AmbientOcclusion,
                                                crate::packing::ChannelSource::Opacity,
                                                crate::packing::ChannelSource::EmissiveR,
                                            ] {
                                                ui.label(
                                                    egui::RichText::new(format!("[{}]", src.short_code()))
                                                        .small()
                                                        .color(egui::Color32::from_rgb(120, 200, 255)),
                                                );
                                            }
                                        });

                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(egui::RichText::new("Baked Mesh Maps:").small().strong());
                                            for &src in &[
                                                crate::packing::ChannelSource::BakedCurvature,
                                                crate::packing::ChannelSource::BakedThickness,
                                                crate::packing::ChannelSource::BakedWorldNormalX,
                                                crate::packing::ChannelSource::BakedPositionX,
                                                crate::packing::ChannelSource::BakedId,
                                            ] {
                                                ui.label(
                                                    egui::RichText::new(format!("[{}]", src.short_code()))
                                                        .small()
                                                        .color(egui::Color32::from_rgb(255, 180, 100)),
                                                );
                                            }
                                        });
                                    });
                            }
                        });
                    });
                }

                // TAB 2: LIST OF EXPORTS (Substance 3D Painter Export Report)
                if self.export_tab_idx == 2 {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.heading("Export History & Generated Assets");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Clear Log").clicked() {
                                    self.export_last_exported_files.clear();
                                }
                                if ui.button("Browse...").on_hover_text("Select destination folder using system file dialog").clicked() {
                                    let initial_dir = PathBuf::from(&self.export_dir);
                                    let mut dialog = rfd::FileDialog::new().set_title("Select Export Directory");
                                    if initial_dir.is_dir() {
                                        dialog = dialog.set_directory(&initial_dir);
                                    } else if let Some(parent) = initial_dir.parent() {
                                        if parent.is_dir() {
                                            dialog = dialog.set_directory(parent);
                                        }
                                    }
                                    if let Some(folder) = dialog.pick_folder() {
                                        self.export_dir = folder.to_string_lossy().to_string();
                                    }
                                }
                                if ui.button("Open Destination Folder").clicked() {
                                    let p = PathBuf::from(&self.export_dir);
                                    let _ = std::fs::create_dir_all(&p);
                                    open_folder_in_desktop(&p);
                                }
                            });
                        });
                        ui.separator();

                        if self.export_last_exported_files.is_empty() {
                            ui.add_space(20.0);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("No files exported yet in this session.").size(14.0));
                                ui.label(egui::RichText::new("Configure your settings in the 'Settings' tab and click 'Export'.").color(egui::Color32::GRAY));
                            });
                        } else {
                            ui.label(
                                egui::RichText::new(format!("Successfully generated {} production assets in:", self.export_last_exported_files.len()))
                                    .strong()
                                    .color(egui::Color32::from_rgb(100, 230, 120)),
                            );
                            ui.label(egui::RichText::new(&self.export_dir).monospace().color(egui::Color32::from_rgb(180, 220, 255)));
                            ui.add_space(6.0);

                            egui::ScrollArea::vertical()
                                .max_height(400.0)
                                .id_source("export_history_scroll")
                                .show(ui, |ui| {
                                    for path in &self.export_last_exported_files {
                                        let fname = path.file_name().and_then(|f| f.to_str()).unwrap_or("file");
                                        let size_str = if let Ok(meta) = std::fs::metadata(path) {
                                            let bytes = meta.len();
                                            if bytes > 1024 * 1024 {
                                                format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
                                            } else {
                                                format!("{} KB", bytes / 1024)
                                            }
                                        } else {
                                            "--".to_string()
                                        };

                                        ui.horizontal(|ui| {
                                            let tag = if fname.ends_with(".obj") || fname.ends_with(".glb") {
                                                "[3D]"
                                            } else if fname.ends_with(".mtl") {
                                                "[MTL]"
                                            } else {
                                                "[TEX]"
                                            };
                                            ui.label(egui::RichText::new(tag).monospace().color(egui::Color32::from_rgb(140, 200, 255)));
                                            ui.label(egui::RichText::new(fname).strong());
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.label(egui::RichText::new(size_str).small().color(egui::Color32::GRAY));
                                            });
                                        });
                                    }
                                });
                        }
                    });
                }
            });

        if let Some(msg) = status_update {
            self.status_message = msg;
        }
        if close_modal {
            self.show_export_modal = false;
        }
    }
}

/// Helper function to open directories across Windows and Linux
pub fn open_folder_in_desktop(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

fn render_substance_channel_box(
    ui: &mut egui::Ui,
    title: &str,
    slot: &mut crate::packing::ChannelSlot,
    map_idx: usize,
    channel_key: &str,
    bg_color: egui::Color32,
    border_color: egui::Color32,
    label_color: egui::Color32,
) {
    egui::Frame::none()
        .fill(bg_color)
        .stroke(egui::Stroke::new(1.0_f32, border_color))
        .rounding(4.0)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(title).strong().color(label_color));
                    if slot.invert {
                        ui.label(egui::RichText::new("(Inv)").small().color(egui::Color32::from_rgb(255, 200, 100)));
                    }
                });
                egui::ComboBox::from_id_source(format!("substance_chan_{}_{}", map_idx, channel_key))
                    .selected_text(slot.source.name())
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for src in crate::packing::ChannelSource::all() {
                            ui.selectable_value(&mut slot.source, *src, src.name());
                        }
                    });
                ui.checkbox(&mut slot.invert, "Invert");
            });
        });
}
