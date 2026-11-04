use super::*;
use eframe::egui::{self, Color32};

impl PaintcraftApp {
    /// Launch background multi-threaded baking pipeline with real-time progress reporting
    pub fn start_background_bake(&mut self) {
        if self.bake_worker_state.as_ref().map(|s| !s.lock().unwrap().is_done).unwrap_or(false) {
            return;
        }

        let res = self.bake_res;
        let frontal = self.bake_frontal;
        let rear = self.bake_rear;
        let bake_tangent_normal = self.bake_opt_tangent_normal;
        let bake_ws_normal = self.bake_opt_world_normal;
        let bake_ao = self.bake_opt_ao;
        let bake_curvature = self.bake_opt_curvature;
        let bake_position = self.bake_opt_position;
        let bake_thickness = self.bake_opt_thickness;
        let bake_thickness_invert = self.bake_opt_thickness_invert;
        let bake_id_map = self.bake_opt_id_map;
        let denoise = self.bake_denoise;
        let match_config = crate::baker::BakeMatchConfig {
            mode: self.bake_match_mode,
            low_suffix: self.bake_low_suffix.clone(),
            high_suffix: self.bake_high_suffix.clone(),
        };
        let cage_config = crate::baker::BakeCageConfig {
            mode: self.bake_cage_mode,
            inflation_offset: self.bake_cage_inflation,
            external_cage_path: if self.bake_cage_mode == crate::baker::BakeCageMode::ExternalCage && !self.bake_cage_path.trim().is_empty() {
                Some(std::path::PathBuf::from(&self.bake_cage_path))
            } else {
                None
            },
        };

        let ao_config = self.bake_ao_config.clone();
        let mut thickness_config = self.bake_thickness_config.clone();
        if bake_thickness_invert {
            thickness_config.invert = true;
        }
        let curvature_config = self.bake_curvature_config.clone();
        let normal_config = self.bake_normal_config.clone();
        let position_config = self.bake_position_config.clone();

        let mesh = self.project.mesh.clone();
        let high_poly = self.project.high_poly_mesh.clone();

        let progress_state = Arc::new(Mutex::new(BakeProgressState {
            progress: 0.0,
            step_name: "Initializing Baker...".to_string(),
            is_done: false,
            cancelled: false,
            elapsed_secs: 0.0,
        }));

        let state_clone = Arc::clone(&progress_state);
        let (tx, rx) = channel();

        self.bake_worker_state = Some(progress_state);
        self.bake_receiver = Some(rx);
        self.bake_success_message = None;

        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let mut result = BakeResultPayload {
                tangent_normal: None,
                world_space_normal: None,
                ao: None,
                curvature: None,
                position: None,
                thickness: None,
                id_map: None,
                elapsed_secs: 0.0,
            };

            let total_tasks = (bake_tangent_normal as usize)
                + (bake_ws_normal as usize)
                + (bake_ao as usize)
                + (bake_curvature as usize)
                + (bake_position as usize)
                + (bake_thickness as usize)
                + (bake_id_map as usize);
            let mut completed_tasks = 0;

            if bake_tangent_normal {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking Tangent Space Normal Map...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }

                result.tangent_normal = match crate::baker::Baker::bake_tangent_normals_with_options(
                    &mesh,
                    high_poly.as_ref(),
                    frontal,
                    rear,
                    res,
                    res,
                    &match_config,
                    &cage_config,
                    normal_config.invert_green,
                ) {
                    Ok(buf) => Some(buf),
                    Err(e) => {
                        eprintln!("Cage bake warning: {}", e);
                        Some(crate::baker::Baker::bake_tangent_normals_with_match(
                            &mesh,
                            high_poly.as_ref(),
                            frontal,
                            rear,
                            res,
                            res,
                            &match_config,
                        ))
                    }
                };
                completed_tasks += 1;
            }

            if bake_ws_normal {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking World Space Normal Map...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                result.world_space_normal = Some(crate::baker::Baker::bake_world_space_normal_with_high_poly(
                    &mesh,
                    high_poly.as_ref(),
                    frontal,
                    rear,
                    res,
                    res,
                    &match_config,
                    &cage_config,
                ));
                completed_tasks += 1;
            }

            if bake_ao {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking Ambient Occlusion (Raycasting)...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                let mut ao = crate::baker::Baker::bake_ao_with_options_high_poly_and_distances(
                    &mesh,
                    high_poly.as_ref(),
                    frontal,
                    rear,
                    res,
                    res,
                    &match_config,
                    &cage_config,
                    &ao_config,
                ).unwrap_or_else(|_| crate::baker::Baker::bake_ao(&mesh, res, res));
                if denoise {
                    {
                        let mut st = state_clone.lock().unwrap();
                        st.step_name = "Denoising Ambient Occlusion (Bilateral)...".to_string();
                    }
                    crate::baker::Baker::denoise_scalar_buffer(&mut ao, result.world_space_normal.as_ref(), 1, 2, 1.2, 24.0);
                }
                result.ao = Some(ao);
                completed_tasks += 1;
            }

            if bake_curvature {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking Curvature Map...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                let mut curv = crate::baker::Baker::bake_curvature_with_options_and_high_poly(
                    &mesh,
                    high_poly.as_ref(),
                    result.tangent_normal.as_ref(),
                    frontal,
                    rear,
                    res,
                    res,
                    &match_config,
                    &cage_config,
                    &curvature_config,
                );
                if denoise {
                    {
                        let mut st = state_clone.lock().unwrap();
                        st.step_name = "Denoising Curvature Map (Sharpening Edges)...".to_string();
                    }
                    crate::baker::Baker::denoise_scalar_buffer(&mut curv, result.world_space_normal.as_ref(), 1, 1, 0.8, 24.0);
                }
                result.curvature = Some(curv);
                completed_tasks += 1;
            }

            if bake_position {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking World Position WPO...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                result.position = Some(crate::baker::Baker::bake_position_with_options_and_high_poly(
                    &mesh,
                    high_poly.as_ref(),
                    frontal,
                    rear,
                    res,
                    res,
                    &match_config,
                    &cage_config,
                    &position_config,
                ));
                completed_tasks += 1;
            }

            if bake_thickness {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking Thickness Map...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                let mut th = crate::baker::Baker::bake_thickness_with_options_high_poly_and_distances(
                    &mesh,
                    high_poly.as_ref(),
                    frontal,
                    rear,
                    res,
                    res,
                    &match_config,
                    &cage_config,
                    &thickness_config,
                ).unwrap_or_else(|_| crate::baker::Baker::bake_thickness(&mesh, res, res));
                if denoise {
                    crate::baker::Baker::denoise_scalar_buffer(&mut th, result.world_space_normal.as_ref(), 1, 2, 1.2, 24.0);
                }
                result.thickness = Some(th);
                completed_tasks += 1;
            }

            if bake_id_map {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking ID Map...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                result.id_map = Some(crate::baker::Baker::bake_id_map(&mesh, res, res));
            }

            result.elapsed_secs = start.elapsed().as_secs_f32();
            {
                let mut st = state_clone.lock().unwrap();
                st.progress = 1.0;
                st.step_name = format!("Bake Complete in {:.2}s!", result.elapsed_secs);
                st.is_done = true;
                st.elapsed_secs = result.elapsed_secs;
            }
            let _ = tx.send(result);
        });
    }

    /// Render Dedicated Substance 3D Painter Baking Screen
    pub fn render_dedicated_baking_screen_ui(&mut self, ctx: &egui::Context) {
        if !self.show_bake_modal {
            return;
        }

        let is_baking = self.bake_worker_state.as_ref().map(|s| !s.lock().unwrap().is_done).unwrap_or(false);
        let mut close_modal = false;

        let screen = ctx.screen_rect();
        let def_w = 780.0f32.min(screen.width() * 0.60).max(520.0);
        let def_h = 580.0f32.min(screen.height() * 0.85).max(380.0);
        let min_w = 520.0f32.min(screen.width() * 0.9);
        let min_h = 360.0f32.min(screen.height() * 0.85);
        let max_w = (screen.width() - 32.0).max(400.0);
        let max_h = (screen.height() - 32.0).max(350.0);

        egui::Window::new("Bake Mesh Maps")
            .collapsible(false)
            .resizable(true)
            .default_pos(egui::pos2(20.0, 50.0))
            .default_size([def_w, def_h])
            .min_width(min_w)
            .min_height(min_h)
            .max_size([max_w, max_h])
            .show(ctx, |ui| {
                // Header Banner
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading(egui::RichText::new("BAKE MESH MAPS").size(16.0).strong().color(Color32::WHITE));
                        ui.label(egui::RichText::new("Bake geometric curvature, ambient occlusion, normal, and position maps from 3D geometry").small().color(Color32::from_rgb(160, 165, 175)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(" X ").clicked() && !is_baking {
                            close_modal = true;
                        }
                    });
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 3-Column Substance 3D Painter Layout
                let avail_h = (ui.available_height() - 110.0).max(280.0);
                ui.horizontal(|ui| {
                    // Column 1: Texture Sets
                    let col1_w = (ui.available_width() * 0.22).clamp(160.0, 190.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(col1_w, avail_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.strong(egui::RichText::new("Texture Sets").color(Color32::from_rgb(220, 225, 235)));
                            ui.add_space(2.0);

                            if self.bake_selected_texture_sets.len() != self.project.texture_sets.len() {
                                self.bake_selected_texture_sets = vec![true; self.project.texture_sets.len()];
                            }

                            ui.horizontal(|ui| {
                                if ui.small_button("All").clicked() {
                                    for b in &mut self.bake_selected_texture_sets { *b = true; }
                                }
                                if ui.small_button("None").clicked() {
                                    for b in &mut self.bake_selected_texture_sets { *b = false; }
                                }
                            });
                            ui.separator();

                            egui::ScrollArea::vertical().id_source("bake_ts_scroll").show(ui, |ui| {
                                for (i, ts) in self.project.texture_sets.iter().enumerate() {
                                    let mut sel = self.bake_selected_texture_sets.get(i).copied().unwrap_or(true);
                                    let is_active = i == self.project.active_texture_set_idx;
                                    let label = format!("{} ({}x{})", ts.name, ts.width, ts.height);
                                    if ui.checkbox(&mut sel, label).changed() {
                                        if let Some(b) = self.bake_selected_texture_sets.get_mut(i) {
                                            *b = sel;
                                        }
                                    }
                                    if is_active {
                                        ui.label(egui::RichText::new("  (Active Set)").small().color(Color32::from_rgb(243, 94, 24)));
                                    }
                                }
                            });
                        },
                    );

                    ui.separator();

                    // Column 2: Mesh Maps Selection & Navigation
                    let col2_w = (ui.available_width() * 0.28).clamp(170.0, 210.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(col2_w, avail_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.strong(egui::RichText::new("Bake Settings").color(Color32::from_rgb(220, 225, 235)));
                            ui.add_space(4.0);

                            let is_common = self.bake_active_settings_tab == crate::baker::BakeSettingsTab::Common;
                            if ui.selectable_label(is_common, "Common Parameters").clicked() {
                                self.bake_active_settings_tab = crate::baker::BakeSettingsTab::Common;
                            }
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(2.0);

                            ui.strong(egui::RichText::new("Mesh Maps").color(Color32::from_rgb(200, 205, 215)));
                            ui.add_space(2.0);

                            let maps = [
                                (crate::baker::BakeSettingsTab::TangentNormal, "Normal (Tangent Space)", &mut self.bake_opt_tangent_normal),
                                (crate::baker::BakeSettingsTab::WorldNormal, "World Space Normal", &mut self.bake_opt_world_normal),
                                (crate::baker::BakeSettingsTab::AmbientOcclusion, "Ambient Occlusion", &mut self.bake_opt_ao),
                                (crate::baker::BakeSettingsTab::Curvature, "Curvature", &mut self.bake_opt_curvature),
                                (crate::baker::BakeSettingsTab::Position, "Position (World Space)", &mut self.bake_opt_position),
                                (crate::baker::BakeSettingsTab::Thickness, "Thickness", &mut self.bake_opt_thickness),
                                (crate::baker::BakeSettingsTab::IdMap, "ID Map (Submeshes)", &mut self.bake_opt_id_map),
                            ];

                            for (tab, label, opt_ref) in maps {
                                let is_active = self.bake_active_settings_tab == tab;
                                ui.horizontal(|ui| {
                                    ui.checkbox(opt_ref, "");
                                    if ui.selectable_label(is_active, label).clicked() {
                                        self.bake_active_settings_tab = tab;
                                    }
                                });
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.strong(egui::RichText::new("Global Denoising").color(Color32::from_rgb(180, 185, 195)));
                            ui.add_space(2.0);
                            ui.checkbox(&mut self.bake_denoise, "Denoise Mesh Maps")
                                .on_hover_text("Edge-preserving bilateral filter cleans ray-march noise on AO and Curvature while preserving sharp bevels");
                            if self.bake_denoise {
                                ui.horizontal(|ui| {
                                    ui.label("Passes:");
                                    ui.add(egui::Slider::new(&mut self.bake_denoise_iterations, 1..=3));
                                });
                            }
                        },
                    );

                    ui.separator();

                    // Column 3: Active Map & Common Parameters Panel
                    let col3_w = (ui.available_width() - 12.0).max(280.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(col3_w, avail_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            // Top Tab Strip
                            ui.horizontal_wrapped(|ui| {
                                let tabs = [
                                    (crate::baker::BakeSettingsTab::Common, "Common"),
                                    (crate::baker::BakeSettingsTab::TangentNormal, "Normal"),
                                    (crate::baker::BakeSettingsTab::WorldNormal, "World Normal"),
                                    (crate::baker::BakeSettingsTab::AmbientOcclusion, "AO"),
                                    (crate::baker::BakeSettingsTab::Curvature, "Curvature"),
                                    (crate::baker::BakeSettingsTab::Position, "Position"),
                                    (crate::baker::BakeSettingsTab::Thickness, "Thickness"),
                                    (crate::baker::BakeSettingsTab::IdMap, "ID Map"),
                                ];
                                for (tab, name) in tabs {
                                    let is_active = self.bake_active_settings_tab == tab;
                                    if ui.selectable_label(is_active, name).clicked() {
                                        self.bake_active_settings_tab = tab;
                                    }
                                }
                            });
                            ui.add_space(2.0);
                            ui.separator();
                            ui.add_space(4.0);

                            egui::ScrollArea::vertical()
                                .id_source("bake_settings_panel_scroll")
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    match self.bake_active_settings_tab {
                                        crate::baker::BakeSettingsTab::Common => {
                                            ui.strong(egui::RichText::new("Common Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Output Size:");
                                                egui::ComboBox::from_id_source("bake_size_combo")
                                                    .selected_text(format!("{} x {}", self.bake_res, self.bake_res))
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_res, 512, "512 x 512");
                                                        ui.selectable_value(&mut self.bake_res, 1024, "1024 x 1024 (1K)");
                                                        ui.selectable_value(&mut self.bake_res, 2048, "2048 x 2048 (2K)");
                                                        ui.selectable_value(&mut self.bake_res, 4096, "4096 x 4096 (4K)");
                                                    });
                                            });

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.strong("High Definition Meshes:");
                                            ui.horizontal(|ui| {
                                                ui.text_edit_singleline(&mut self.bake_highpoly_path);
                                                if ui.button("Browse...").clicked() {
                                                    if let Some(path) = rfd::FileDialog::new().add_filter("3D Models (FBX, OBJ, glTF, GLB, PLY, STL)", &["fbx", "obj", "gltf", "glb", "ply", "stl"]).pick_file() {
                                                        self.bake_highpoly_path = path.to_string_lossy().to_string();
                                                        if let Ok(mesh) = crate::mesh::Mesh::load_file(&path) {
                                                            self.project.high_poly_mesh = Some(mesh);
                                                            self.status_message = format!("Loaded High-Poly Mesh: {:?}", path.file_name().unwrap_or_default());
                                                        }
                                                    }
                                                }
                                                if self.project.high_poly_mesh.is_some() && ui.button("Clear").clicked() {
                                                    self.project.high_poly_mesh = None;
                                                    self.bake_highpoly_path.clear();
                                                }
                                            });
                                            if let Some(ref hp) = self.project.high_poly_mesh {
                                                ui.colored_label(Color32::from_rgb(100, 220, 140), format!("Loaded: {} ({} tris)", hp.name, hp.indices.len() / 3));
                                            } else {
                                                ui.label(egui::RichText::new("Using low-poly base mesh as high-definition geometry").small().color(Color32::from_rgb(150, 155, 165)));
                                            }

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.strong("Ray Distances:");
                                            ui.horizontal(|ui| {
                                                ui.label("Max Frontal Distance:");
                                                ui.add(
                                                    egui::Slider::new(&mut self.bake_frontal, 0.0001..=50.0)
                                                        .logarithmic(true)
                                                        .clamp_to_range(false)
                                                        .custom_formatter(|n, _| format!("{:.4}", n))
                                                );
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Max Rear Distance:");
                                                ui.add(
                                                    egui::Slider::new(&mut self.bake_rear, 0.0001..=50.0)
                                                        .logarithmic(true)
                                                        .clamp_to_range(false)
                                                        .custom_formatter(|n, _| format!("{:.4}", n))
                                                );
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new("Rear Presets:").small().color(Color32::from_rgb(160, 165, 175)));
                                                for preset in [0.01_f32, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 25.0] {
                                                    let is_selected = (self.bake_rear - preset).abs() < 1e-4;
                                                    let text = if preset >= 1.0 {
                                                        format!("{:.0}", preset)
                                                    } else {
                                                        format!("{}", preset)
                                                    };
                                                    if ui.selectable_label(is_selected, text).clicked() {
                                                        self.bake_rear = preset;
                                                    }
                                                }
                                            });
                                            self.bake_rear = self.bake_rear.max(0.0001);
                                            self.bake_frontal = self.bake_frontal.max(0.0001);
                                            self.bake_normal_config.frontal_distance = self.bake_frontal;
                                            self.bake_normal_config.rear_distance = self.bake_rear;

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.strong("Cage Envelope & Ray Origin:");
                                            ui.horizontal(|ui| {
                                                ui.label("Mode:");
                                                egui::ComboBox::from_id_source("bake_cage_mode_combo")
                                                    .selected_text(self.bake_cage_mode.name())
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_cage_mode, crate::baker::BakeCageMode::None, "None (Ray Distances)");
                                                        ui.selectable_value(&mut self.bake_cage_mode, crate::baker::BakeCageMode::ParametricInflation, "Parametric Cage (Inflation)");
                                                        ui.selectable_value(&mut self.bake_cage_mode, crate::baker::BakeCageMode::ExternalCage, "External Cage Mesh (.obj)");
                                                    });
                                            });

                                            match self.bake_cage_mode {
                                                crate::baker::BakeCageMode::None => {
                                                    ui.label(egui::RichText::new("Casting rays along vertex normals using frontal and rear distances.").small().color(Color32::from_rgb(150, 155, 165)));
                                                }
                                                crate::baker::BakeCageMode::ParametricInflation => {
                                                    ui.horizontal(|ui| {
                                                        ui.label("Cage Inflation Offset:");
                                                        ui.add(
                                                            egui::Slider::new(&mut self.bake_cage_inflation, 0.0001..=50.0)
                                                                .logarithmic(true)
                                                                .clamp_to_range(false)
                                                                .custom_formatter(|n, _| format!("{:.4}", n))
                                                        );
                                                    });
                                                    self.bake_cage_inflation = self.bake_cage_inflation.max(0.0);
                                                    ui.label(egui::RichText::new("Generates a seamless averaged normal envelope to eliminate ray-crossing on bevels and corners.").small().color(Color32::from_rgb(150, 155, 165)));
                                                }
                                                crate::baker::BakeCageMode::ExternalCage => {
                                                    ui.horizontal(|ui| {
                                                        ui.text_edit_singleline(&mut self.bake_cage_path);
                                                        if ui.button("Browse...").clicked() {
                                                             if let Some(path) = rfd::FileDialog::new().add_filter("3D Models (FBX, OBJ, glTF, GLB, PLY, STL)", &["fbx", "obj", "gltf", "glb", "ply", "stl"]).pick_file() {
                                                                self.bake_cage_path = path.to_string_lossy().to_string();
                                                            }
                                                        }
                                                        if !self.bake_cage_path.is_empty() && ui.button("Clear").clicked() {
                                                            self.bake_cage_path.clear();
                                                        }
                                                    });
                                                    ui.label(egui::RichText::new("Cage mesh must have matching topology (identical vertex count and faces) as the low-poly mesh.").small().color(Color32::from_rgb(150, 155, 165)));
                                                }
                                            }

                                            ui.add_space(4.0);
                                            ui.checkbox(&mut self.bake_preview_cage, egui::RichText::new("Preview Cage in 3D Viewport").strong().color(Color32::from_rgb(0, 225, 255)));
                                            if self.bake_preview_cage {
                                                ui.indent("bake_cage_preview_indent", |ui| {
                                                    if self.bake_cage_mode == crate::baker::BakeCageMode::None {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Envelope Preview:");
                                                            ui.selectable_value(&mut self.bake_cage_preview_rear, false, "Frontal (Outer)");
                                                            ui.selectable_value(&mut self.bake_cage_preview_rear, true, "Rear (Inward Penetration)");
                                                        });
                                                    }
                                                    ui.horizontal(|ui| {
                                                        ui.checkbox(&mut self.bake_cage_show_hull, "Translucent Shell");
                                                        ui.checkbox(&mut self.bake_cage_show_tethers, "Displacement Vectors");
                                                    });
                                                    ui.horizontal(|ui| {
                                                        ui.label("Color:");
                                                        let colors = ["Cyan (Substance)", "Orange", "Lime Green", "Magenta"];
                                                        egui::ComboBox::from_id_source("bake_cage_color_sel")
                                                            .selected_text(colors.get(self.bake_cage_color_idx).copied().unwrap_or("Cyan"))
                                                            .show_ui(ui, |ui| {
                                                                for (c_idx, &c_name) in colors.iter().enumerate() {
                                                                    ui.selectable_value(&mut self.bake_cage_color_idx, c_idx, c_name);
                                                                }
                                                            });
                                                        ui.label("Opacity:");
                                                        ui.add(egui::Slider::new(&mut self.bake_cage_opacity, 0.1..=1.0));
                                                    });
                                                    let status_text = match self.bake_cage_mode {
                                                        crate::baker::BakeCageMode::None => {
                                                            if self.bake_cage_preview_rear {
                                                                format!("Previewing rear ray penetration at depth {:.3}", self.bake_rear)
                                                            } else {
                                                                format!("Previewing frontal ray envelope at distance {:.3}", self.bake_frontal)
                                                            }
                                                        }
                                                        crate::baker::BakeCageMode::ParametricInflation => format!("Previewing parametric smoothed cage at offset {:.3}", self.bake_cage_inflation),
                                                        crate::baker::BakeCageMode::ExternalCage => {
                                                            if self.bake_cage_path.is_empty() {
                                                                "No external cage OBJ loaded".to_string()
                                                            } else {
                                                                format!("Previewing external cage: {}", self.bake_cage_path)
                                                            }
                                                        }
                                                    };
                                                    ui.label(egui::RichText::new(status_text).small().color(Color32::from_rgb(120, 210, 255)));
                                                });
                                            }

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.strong("Mesh Matching:");
                                            ui.horizontal(|ui| {
                                                ui.label("Match:");
                                                egui::ComboBox::from_id_source("bake_match_mode_combo")
                                                    .selected_text(self.bake_match_mode.name())
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_match_mode, crate::baker::BakeMatchMode::Always, "Always");
                                                        ui.selectable_value(&mut self.bake_match_mode, crate::baker::BakeMatchMode::ByMeshName, "By Mesh Name");
                                                    });
                                            });
                                            if self.bake_match_mode == crate::baker::BakeMatchMode::ByMeshName {
                                                ui.horizontal(|ui| {
                                                    ui.label("Low Suffix:");
                                                    ui.add(egui::TextEdit::singleline(&mut self.bake_low_suffix).desired_width(70.0));
                                                    ui.label("High Suffix:");
                                                    ui.add(egui::TextEdit::singleline(&mut self.bake_high_suffix).desired_width(70.0));
                                                });
                                            }
                                        }

                                        crate::baker::BakeSettingsTab::AmbientOcclusion => {
                                            ui.strong(egui::RichText::new("Ambient Occlusion Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_ao, "Enable Ambient Occlusion Bake");
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Secondary Rays (Samples):");
                                                egui::ComboBox::from_id_source("bake_ao_samples_combo")
                                                    .selected_text(format!("{} Rays", self.bake_ao_config.samples))
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_ao_config.samples, 16, "16 Rays (Fast Draft)");
                                                        ui.selectable_value(&mut self.bake_ao_config.samples, 32, "32 Rays (Preview)");
                                                        ui.selectable_value(&mut self.bake_ao_config.samples, 64, "64 Rays (Default)");
                                                        ui.selectable_value(&mut self.bake_ao_config.samples, 128, "128 Rays (High Quality)");
                                                        ui.selectable_value(&mut self.bake_ao_config.samples, 256, "256 Rays (Ultra Detail)");
                                                    });
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Fine Sample Count:");
                                                ui.add(egui::Slider::new(&mut self.bake_ao_config.samples, 8..=512));
                                            });
                                            ui.label(egui::RichText::new("Number of Monte Carlo hemisphere occlusion test rays cast per vertex. Higher values yield cleaner, noise-free contact shadows.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.horizontal(|ui| {
                                                ui.label("Max Distance:");
                                                ui.add(egui::Slider::new(&mut self.bake_ao_config.max_distance, 0.01..=1.5).text("x Bounding Diag"));
                                            });
                                            ui.label(egui::RichText::new("Maximum reach of occlusion rays. Smaller values create tight contact shadows; larger values catch broad cavity occlusion.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Ray Bias / Offset:");
                                                ui.add(egui::Slider::new(&mut self.bake_ao_config.ray_bias, 0.001..=0.05).logarithmic(true));
                                            });
                                            ui.label(egui::RichText::new("Surface normal offset distance to prevent self-shadowing on curved facets.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Spread / Attenuation:");
                                                ui.add(egui::Slider::new(&mut self.bake_ao_config.spread, 0.2..=2.0));
                                            });

                                            ui.add_space(4.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Occlusion Contrast:");
                                                ui.add(egui::Slider::new(&mut self.bake_ao_config.contrast, 0.1..=3.0));
                                            });
                                        }

                                        crate::baker::BakeSettingsTab::Thickness => {
                                            ui.strong(egui::RichText::new("Thickness Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_thickness, "Enable Thickness Bake");
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Secondary Rays (Samples):");
                                                egui::ComboBox::from_id_source("bake_th_samples_combo")
                                                    .selected_text(format!("{} Rays", self.bake_thickness_config.samples))
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_thickness_config.samples, 16, "16 Rays (Fast Draft)");
                                                        ui.selectable_value(&mut self.bake_thickness_config.samples, 32, "32 Rays (Standard)");
                                                        ui.selectable_value(&mut self.bake_thickness_config.samples, 64, "64 Rays (High Quality)");
                                                        ui.selectable_value(&mut self.bake_thickness_config.samples, 128, "128 Rays (Ultra Detail)");
                                                    });
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Fine Sample Count:");
                                                ui.add(egui::Slider::new(&mut self.bake_thickness_config.samples, 8..=256));
                                            });
                                            ui.label(egui::RichText::new("Number of inward transmission rays cast into the mesh interior for subsurface light penetration.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.horizontal(|ui| {
                                                ui.label("Max Distance:");
                                                ui.add(egui::Slider::new(&mut self.bake_thickness_config.max_distance, 0.05..=2.0).text("x Bounding Diag"));
                                            });
                                            ui.label(egui::RichText::new("Maximum penetration depth to search for interior surfaces.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Ray Bias / Offset:");
                                                ui.add(egui::Slider::new(&mut self.bake_thickness_config.ray_bias, 0.001..=0.05).logarithmic(true));
                                            });

                                            ui.add_space(6.0);
                                            ui.checkbox(&mut self.bake_opt_thickness_invert, "Invert (White = Thin)")
                                                .on_hover_text("Invert values so thin translucent areas are white (high transmission) and thick volume areas are black");
                                        }

                                        crate::baker::BakeSettingsTab::Curvature => {
                                            ui.strong(egui::RichText::new("Curvature Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_curvature, "Enable Curvature Bake");
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Curvature Intensity:");
                                                ui.add(egui::Slider::new(&mut self.bake_curvature_config.intensity, 0.1..=3.0));
                                            });
                                            ui.label(egui::RichText::new("Overall strength of convex ridge highlights and concave crevice shadows.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Curvature Contrast:");
                                                ui.add(egui::Slider::new(&mut self.bake_curvature_config.contrast, 0.1..=3.0));
                                            });
                                            ui.label(egui::RichText::new("Expands dynamic range away from neutral 0.5 flat gray.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.horizontal(|ui| {
                                                ui.label("Dihedral Weight (Edges):");
                                                ui.add(egui::Slider::new(&mut self.bake_curvature_config.dihedral_weight, 0.0..=1.0));
                                            });
                                            ui.label(egui::RichText::new("Sensitivity to hard polygonal edge angles and sharp bevels.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Deflection Weight (Smooth):");
                                                ui.add(egui::Slider::new(&mut self.bake_curvature_config.deflection_weight, 0.0..=1.0));
                                            });
                                            ui.label(egui::RichText::new("Sensitivity to gradual surface bending across 1-ring vertex neighbors.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Blur Smoothing Passes:");
                                                ui.add(egui::Slider::new(&mut self.bake_curvature_config.blur_passes, 0..=4));
                                            });
                                            ui.label(egui::RichText::new("Separable Gaussian blur passes to smooth edge gradients and eliminate pixel stepping.").small().color(Color32::from_rgb(150, 155, 165)));
                                        }

                                        crate::baker::BakeSettingsTab::TangentNormal => {
                                            ui.strong(egui::RichText::new("Normal (Tangent Space) Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_tangent_normal, "Enable Normal Map Bake");
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Max Frontal Distance:");
                                                ui.add(
                                                    egui::Slider::new(&mut self.bake_frontal, 0.0001..=50.0)
                                                        .logarithmic(true)
                                                        .clamp_to_range(false)
                                                        .custom_formatter(|n, _| format!("{:.4}", n))
                                                );
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Max Rear Distance:");
                                                ui.add(
                                                    egui::Slider::new(&mut self.bake_rear, 0.0001..=50.0)
                                                        .logarithmic(true)
                                                        .clamp_to_range(false)
                                                        .custom_formatter(|n, _| format!("{:.4}", n))
                                                );
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new("Rear Presets:").small().color(Color32::from_rgb(160, 165, 175)));
                                                for preset in [0.01_f32, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 25.0] {
                                                    let is_selected = (self.bake_rear - preset).abs() < 1e-4;
                                                    let text = if preset >= 1.0 {
                                                        format!("{:.0}", preset)
                                                    } else {
                                                        format!("{}", preset)
                                                    };
                                                    if ui.selectable_label(is_selected, text).clicked() {
                                                        self.bake_rear = preset;
                                                    }
                                                }
                                            });
                                            self.bake_rear = self.bake_rear.max(0.0001);
                                            self.bake_frontal = self.bake_frontal.max(0.0001);
                                            self.bake_normal_config.frontal_distance = self.bake_frontal;
                                            self.bake_normal_config.rear_distance = self.bake_rear;

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Normal Format:");
                                                egui::ComboBox::from_id_source("bake_normal_format_combo")
                                                    .selected_text(if self.bake_normal_config.invert_green { "DirectX (Y- Invert Green)" } else { "OpenGL (Y+ Up)" })
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_normal_config.invert_green, false, "OpenGL (Y+ Up, Default)");
                                                        ui.selectable_value(&mut self.bake_normal_config.invert_green, true, "DirectX (Y- Invert Green)");
                                                    });
                                            });
                                            ui.label(egui::RichText::new("OpenGL (Y+) is used by Blender, Godot, and 3D Paintcraft. DirectX (Y-) is used by Unreal Engine and 3ds Max.").small().color(Color32::from_rgb(150, 155, 165)));
                                        }

                                        crate::baker::BakeSettingsTab::WorldNormal => {
                                            ui.strong(egui::RichText::new("World Space Normal Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_world_normal, "Enable World Space Normal Bake");
                                            ui.add_space(6.0);
                                            ui.label(egui::RichText::new("Bakes the 3D surface orientation in world space encoded as RGB values. Used for directional effects (such as top-down snow, dust accumulation, bottom ground splash) and as the guide normal for edge-preserving bilateral denoising.").color(Color32::from_rgb(180, 185, 195)));
                                        }

                                        crate::baker::BakeSettingsTab::Position => {
                                            ui.strong(egui::RichText::new("Position (World Space) Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_position, "Enable Position Map Bake");
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Normalization Mode:");
                                                egui::ComboBox::from_id_source("bake_pos_mode_combo")
                                                    .selected_text(match self.bake_position_config.mode {
                                                        crate::baker::PositionNormalizationMode::BoundingBox => "Bounding Box [0, 1] (Default)",
                                                        crate::baker::PositionNormalizationMode::WorldUnits => "World Space Coordinates",
                                                    })
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_position_config.mode, crate::baker::PositionNormalizationMode::BoundingBox, "Bounding Box [0, 1] (Default)");
                                                        ui.selectable_value(&mut self.bake_position_config.mode, crate::baker::PositionNormalizationMode::WorldUnits, "World Space Coordinates");
                                                    });
                                            });
                                            ui.label(egui::RichText::new("Encodes 3D vertex positions into RGB color. Bounding Box maps [min, max] to [0, 1], powering 3D Position Gradient generators and height-based masking.").small().color(Color32::from_rgb(150, 155, 165)));
                                        }

                                        crate::baker::BakeSettingsTab::IdMap => {
                                            ui.strong(egui::RichText::new("ID Map Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_id_map, "Enable ID Map Bake");
                                            ui.add_space(6.0);
                                            ui.label(egui::RichText::new("Bakes distinct high-contrast colors per submesh/geometry element. Used with the Color Selection Mask effect to isolate and paint specific parts of complex meshes with one click.").color(Color32::from_rgb(180, 185, 195)));
                                        }
                                    }
                                });
                        },
                    );
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Bottom Panel: Real-Time Progress Bar, Previews & Action Buttons
                let mut cancel_requested = false;
                let mut trigger_bake = false;

                if let Some(state_arc) = &self.bake_worker_state {
                    let st = state_arc.lock().unwrap();
                    let prog = st.progress;
                    let step = st.step_name.clone();
                    let done = st.is_done;
                    drop(st);

                    if !done {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.strong(&step);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Cancel Bake").clicked() {
                                    state_arc.lock().unwrap().cancelled = true;
                                    cancel_requested = true;
                                }
                            });
                        });
                        ui.add_space(4.0);
                        ui.add(egui::ProgressBar::new(prog).show_percentage().animate(true));
                    } else {
                        // Completed view
                        if let Some(msg) = &self.bake_success_message {
                            ui.horizontal(|ui| {
                                ui.colored_label(Color32::from_rgb(100, 220, 140), format!("[OK] {}", msg));
                            });
                        }
                        // Live Thumbnails Strip
                        ui.horizontal(|ui| {
                            if let Some(tex) = &self.bake_preview_normal {
                                ui.vertical(|ui| {
                                    ui.image((tex.id(), egui::vec2(44.0, 44.0)));
                                    ui.label(egui::RichText::new("Normal").small());
                                });
                            }
                            if let Some(tex) = &self.bake_preview_ao {
                                ui.vertical(|ui| {
                                    ui.image((tex.id(), egui::vec2(44.0, 44.0)));
                                    ui.label(egui::RichText::new("AO").small());
                                });
                            }
                            if let Some(tex) = &self.bake_preview_curv {
                                ui.vertical(|ui| {
                                    ui.image((tex.id(), egui::vec2(44.0, 44.0)));
                                    ui.label(egui::RichText::new("Curvature").small());
                                });
                            }
                            if let Some(tex) = &self.bake_preview_pos {
                                ui.vertical(|ui| {
                                    ui.image((tex.id(), egui::vec2(44.0, 44.0)));
                                    ui.label(egui::RichText::new("Position").small());
                                });
                            }
                            if let Some(tex) = &self.bake_preview_thick {
                                ui.vertical(|ui| {
                                    ui.image((tex.id(), egui::vec2(44.0, 44.0)));
                                    ui.label(egui::RichText::new("Thickness").small());
                                });
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Close").clicked() {
                                    close_modal = true;
                                }
                                let bake_btn = egui::Button::new(egui::RichText::new("Bake Again").strong().color(Color32::WHITE))
                                    .fill(Color32::from_rgb(226, 91, 16))
                                    .min_size(egui::vec2(160.0, 32.0));
                                if ui.add(bake_btn).clicked() {
                                    trigger_bake = true;
                                }
                            });
                        });
                    }
                } else {
                    // Idle state
                    ui.horizontal(|ui| {
                        let bake_btn = egui::Button::new(egui::RichText::new("Bake Selected Textures").strong().color(Color32::WHITE))
                            .fill(Color32::from_rgb(226, 91, 16))
                            .min_size(egui::vec2(200.0, 34.0));
                        if ui.add(bake_btn).clicked() {
                            trigger_bake = true;
                        }

                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                }

                if cancel_requested {
                    self.bake_worker_state = None;
                    self.bake_receiver = None;
                }
                if trigger_bake {
                    self.start_background_bake();
                }
            });

        if close_modal {
            self.show_bake_modal = false;
            self.bake_cage_cache_key = BakeCageCacheKey::default();
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.clear_cage();
            }
        }
    }
}

