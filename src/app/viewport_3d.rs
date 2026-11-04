use super::*;
use eframe::egui;

impl PaintcraftApp {
    /// Render interactive 3D Viewport in egui panel
    pub(crate) fn render_3d_viewport_ui(&mut self, ui: &mut egui::Ui, size: Vec2, ctx: &egui::Context) {
        if self.project.gpu_mesh_maps_sync_needed {
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.sync_mesh_maps(&self.project);
            }
            self.project.gpu_mesh_maps_sync_needed = false;
        }

        let textures_dirty = self.project.gpu_sync_needed;
        if self.project.gpu_sync_needed {
            let t_sync = std::time::Instant::now();
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.sync_textures(&mut self.project);
                if let Some(preview_id) = self.mask_preview_layer_id {
                    wgpu.sync_mask_preview_full(&self.project, preview_id);
                }
            }
            self.perf_metrics.gpu_sync_ms = t_sync.elapsed().as_secs_f32() * 1000.0;
            self.project.gpu_sync_needed = false;
        }

        // --- SYNCHRONIZE VIEWPORT BAKING CAGE PREVIEW ---
        if self.show_bake_modal && self.bake_preview_cage {
            let current_key = BakeCageCacheKey {
                mode: self.bake_cage_mode,
                inflation_bits: self.bake_cage_inflation.to_bits(),
                frontal_bits: self.bake_frontal.to_bits(),
                rear_bits: self.bake_rear.to_bits(),
                preview_rear: self.bake_cage_preview_rear,
                path: self.bake_cage_path.clone(),
                show_hull: self.bake_cage_show_hull,
                show_tethers: self.bake_cage_show_tethers,
                color_idx: self.bake_cage_color_idx,
                vert_count: self.project.mesh.vertices.len(),
            };
            if self.bake_cage_cache_key != current_key {
                self.bake_cage_cache_key = current_key;
                let cage_verts = match self.bake_cage_mode {
                    crate::baker::BakeCageMode::None => {
                        let offset = if self.bake_cage_preview_rear {
                            -self.bake_rear
                        } else {
                            self.bake_frontal
                        };
                        let cfg = crate::baker::BakeCageConfig {
                            mode: crate::baker::BakeCageMode::ParametricInflation,
                            inflation_offset: offset,
                            external_cage_path: None,
                        };
                        crate::baker::Baker::compute_cage_vertices(&self.project.mesh, &cfg).ok().flatten()
                    }
                    crate::baker::BakeCageMode::ParametricInflation => {
                        let cfg = crate::baker::BakeCageConfig {
                            mode: crate::baker::BakeCageMode::ParametricInflation,
                            inflation_offset: self.bake_cage_inflation,
                            external_cage_path: None,
                        };
                        crate::baker::Baker::compute_cage_vertices(&self.project.mesh, &cfg).ok().flatten()
                    }
                    crate::baker::BakeCageMode::ExternalCage => {
                        let path = if !self.bake_cage_path.trim().is_empty() {
                            Some(std::path::PathBuf::from(&self.bake_cage_path))
                        } else {
                            None
                        };
                        let cfg = crate::baker::BakeCageConfig {
                            mode: crate::baker::BakeCageMode::ExternalCage,
                            inflation_offset: 0.0,
                            external_cage_path: path,
                        };
                        crate::baker::Baker::compute_cage_vertices(&self.project.mesh, &cfg).ok().flatten()
                    }
                };

                let cage_color = self.current_bake_cage_color();
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    wgpu.update_cage(
                        &self.project.mesh,
                        cage_verts.as_deref(),
                        self.bake_cage_show_hull,
                        self.bake_cage_show_tethers,
                        cage_color,
                        self.bake_cage_opacity,
                    );
                }
            }
        } else if !self.show_bake_modal || !self.bake_preview_cage {
            if self.bake_cage_cache_key.vert_count != 0 {
                self.bake_cage_cache_key.vert_count = 0;
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    wgpu.clear_cage();
                }
            }
        }

        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let vp_w = rect.width().max(64.0);
        let vp_h = rect.height().max(64.0);
        let aspect = vp_w / vp_h;

        let is_camera_moving = response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
            || (response.dragged_by(egui::PointerButton::Primary) && ctx.input(|i| i.modifiers.alt));

        let pointer_moved = ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        let is_interacting = response.dragged() || self.is_stroke_active;
        if is_interacting || (response.hovered() && pointer_moved) || self.project.gpu_sync_needed || textures_dirty || self.continuous_60fps {
            ctx.request_repaint();
        }
        let render_w = (vp_w as u32).max(128).min(3840);
        let render_h = (vp_h as u32).max(128).min(2160);

        let mut frame_settings = self.shader_settings;
        if is_camera_moving && self.shader_settings.adaptive_resolution {
            // Bypass heavy Screen-Space Ray Tracing, SSAO, Bloom & Depth-of-Field ONLY during camera orbit/pan/dolly
            frame_settings.ssr_enabled = false;
            frame_settings.ssao_enabled = false;
            frame_settings.bloom_enabled = false;
            frame_settings.dof_enabled = false;
            frame_settings.shadow_softness = 0.0; // Fast single-tap shadow lookup during camera motion
        }

        let effective_wireframe = self.renderer.show_wireframe || self.brush_engine.settings.mode == ToolMode::FillBucket;

        let texture_id = if self.iray_mode {
            let t_rnd = std::time::Instant::now();
            let scale = if is_interacting && self.shader_settings.adaptive_resolution { 0.5 } else { 1.0 };
            let pt_w = ((vp_w * scale) as u32).max(64).min(1920);
            let pt_h = ((vp_h * scale) as u32).max(64).min(1080);

            let current_view = self.camera.view_matrix();
            let current_rot = self.renderer.hdri.rotation;
            let camera_changed = self.last_iray_camera_view.map(|v| v != current_view).unwrap_or(true);
            let hdri_changed = (self.last_iray_hdri_rot - current_rot).abs() > 0.01;
            let textures_changed = textures_dirty || (self.last_iray_texture_revision != self.project.texture_revision);

            if let Some(pt) = &mut self.path_tracer {
                pt.resize(pt_w, pt_h);
                pt.max_samples = self.iray_max_samples;
                pt.max_bounces = self.iray_max_bounces;
                pt.denoiser_enabled = self.iray_denoise;

                if camera_changed || hdri_changed || textures_changed {
                    pt.reset();
                    self.last_iray_camera_view = Some(current_view);
                    self.last_iray_hdri_rot = current_rot;
                    self.last_iray_texture_revision = self.project.texture_revision;
                }

                let samples = if is_interacting { 1 } else { self.iray_samples_per_frame };
                pt.render_samples(
                    &self.camera,
                    &self.project.mesh,
                    &self.project.composite_textures,
                    &self.renderer.hdri,
                    samples,
                );

                if !pt.is_converged() || is_interacting {
                    ctx.request_repaint();
                }

                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [pt.width as usize, pt.height as usize],
                    &pt.color_buffer,
                );
                if let Some(tex) = &mut self.iray_texture {
                    tex.set(color_image, egui::TextureOptions::LINEAR);
                } else {
                    self.iray_texture = Some(ctx.load_texture("iray_viewport", color_image, egui::TextureOptions::LINEAR));
                }
            }

            self.perf_metrics.gpu_render_ms = t_rnd.elapsed().as_secs_f32() * 1000.0;
            if let Some(tex) = &self.iray_texture {
                tex.id()
            } else {
                eframe::egui::TextureId::default()
            }
        } else if let Some(wgpu) = &mut self.wgpu_viewport {
            wgpu.resize(render_w, render_h);
            wgpu.mask_preview_layer_id = self.mask_preview_layer_id;
            
            let view = self.camera.view_matrix();
            let proj = self.camera.projection_matrix(aspect);
            let vp = proj * view;
            let eye = self.camera.eye_position();
            let eye_vec4 = glam::Vec4::new(eye.x, eye.y, eye.z, 1.0);
            
            let mode_u32 = if self.mask_preview_layer_id.is_some() {
                1u32 // Unlit BaseColor mode for mask preview
            } else {
                match self.view_mode {
                    ViewportChannelMode::Material => 0u32,
                    ViewportChannelMode::BaseColor => 1,
                    ViewportChannelMode::Roughness => 2,
                    ViewportChannelMode::Metallic => 3,
                    ViewportChannelMode::Normal => 4,
                    ViewportChannelMode::AmbientOcclusion => 5,
                    ViewportChannelMode::Curvature => 6,
                    ViewportChannelMode::Position => 7,
                    ViewportChannelMode::WorldNormal => 8,
                    ViewportChannelMode::Thickness => 9,
                    ViewportChannelMode::Height => 10,
                    ViewportChannelMode::Emissive => 11,
                    ViewportChannelMode::Opacity => 12,
                    ViewportChannelMode::Clearcoat => 13,
                    ViewportChannelMode::Anisotropy => 14,
                }
            };
            let t_rnd = std::time::Instant::now();
            wgpu.render(vp, eye_vec4, self.renderer.hdri.rotation.to_radians(), mode_u32, self.bg_blur, self.bg_opacity, &frame_settings, effective_wireframe);
            self.perf_metrics.gpu_render_ms = t_rnd.elapsed().as_secs_f32() * 1000.0;
            self.perf_metrics.cmd_encoding_ms = wgpu.cmd_encoding_ms;
            self.perf_metrics.queue_submit_ms = wgpu.queue_submit_ms;
            let timings = wgpu.stage_timings();
            self.perf_metrics.gpu_pbr_ms = timings.pbr_pass_ms;
            self.perf_metrics.gpu_postprocess_ms = timings.postprocess_pass_ms;
            self.perf_metrics.gpu_compute_brush_ms = timings.compute_brush_ms;
            self.perf_metrics.gpu_compositing_ms = timings.compositing_ms;
            self.perf_metrics.gpu_total_hardware_ms = timings.total_gpu_ms;
            self.perf_metrics.hardware_timestamps_active = timings.hardware_timestamps_supported;
            wgpu.texture_id
        } else {
            let t_rnd = std::time::Instant::now();
            self.renderer.resize(render_w, render_h);
            self.update_viewport_texture(ctx);
            self.perf_metrics.gpu_render_ms = t_rnd.elapsed().as_secs_f32() * 1000.0;
            if let Some(tex) = &self.viewport_texture {
                tex.id()
            } else {
                eframe::egui::TextureId::default()
            }
        };

        ui.painter().image(
            texture_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            eframe::egui::Color32::WHITE,
        );

        let hud_top_left = rect.min + eframe::egui::Vec2::new(12.0, 12.0);
        if self.iray_mode {
            let (sample_cur, sample_max, pct) = if let Some(pt) = &self.path_tracer {
                (pt.sample_count, pt.max_samples, pt.progress_pct())
            } else {
                (0, 256, 0.0)
            };
            let status = if pct >= 100.0 { "Converged" } else { "Sampling..." };
            let denoise_badge = if self.iray_denoise { "Denoise: ON" } else { "Denoise: OFF" };
            let iray_hud_text = format!(
                "[IRAY MONTE CARLO PATH TRACER] | Sample: {} / {} ({:.1}%) - {} | {} | HDRI: {} ({:.0}°) | [F10 or Esc to Exit]",
                sample_cur,
                sample_max,
                pct,
                status,
                denoise_badge,
                self.renderer.hdri.preset.name(),
                self.renderer.hdri.rotation,
            );
            ui.painter().text(
                hud_top_left,
                eframe::egui::Align2::LEFT_TOP,
                iray_hud_text,
                eframe::egui::FontId::proportional(12.0),
                eframe::egui::Color32::from_rgb(255, 180, 50),
            );
        } else {
            let gpu_mode_text = if self.wgpu_viewport.is_some() { "[Vulkan/WGPU Substance PBR]" } else { "[Software CPU]" };
            let mut pp_badges = Vec::new();
            match self.shader_settings.aa_mode {
                crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa => { pp_badges.push("NVIDIA DLAA"); }
                crate::nvidia_dlaa::AntiAliasingMode::Fxaa => { pp_badges.push("FXAA"); }
                crate::nvidia_dlaa::AntiAliasingMode::Off => {
                    if self.shader_settings.fxaa_enabled { pp_badges.push("FXAA"); }
                }
            }
            if self.shader_settings.denoiser_enabled { pp_badges.push("Denoiser"); }
            if self.shader_settings.ssao_enabled { pp_badges.push("SSAO"); }
            if self.shader_settings.shadows_enabled { pp_badges.push("Shadows"); }
            if effective_wireframe { pp_badges.push("Wireframe"); }
            if self.shader_settings.bloom_enabled { pp_badges.push("Bloom"); }
            if self.shader_settings.dof_enabled { pp_badges.push("DoF"); }
            let pp_str = if pp_badges.is_empty() { String::new() } else { format!(" | PP: [{}]", pp_badges.join(", ")) };

            let hud_text = format!(
                "{} {} | HDRI: {} ({:.0}°){}",
                gpu_mode_text,
                self.view_mode.name(),
                self.renderer.hdri.preset.name(),
                self.renderer.hdri.rotation,
                pp_str,
            );
            ui.painter().text(
                hud_top_left,
                eframe::egui::Align2::LEFT_TOP,
                hud_text,
                eframe::egui::FontId::proportional(12.0),
                eframe::egui::Color32::from_rgba_unmultiplied(220, 220, 220, 180),
            );
        }

        if self.show_bake_modal && self.bake_preview_cage {
            let cage_mode_desc = match self.bake_cage_mode {
                crate::baker::BakeCageMode::None => {
                    if self.bake_cage_preview_rear {
                        format!("Rear Ray Penetration Depth: {:.3}", self.bake_rear)
                    } else {
                        format!("Frontal Ray Envelope: {:.3}", self.bake_frontal)
                    }
                }
                crate::baker::BakeCageMode::ParametricInflation => format!("Parametric Inflation Offset: {:.3}", self.bake_cage_inflation),
                crate::baker::BakeCageMode::ExternalCage => format!("External Cage: {}", if self.bake_cage_path.is_empty() { "None selected" } else { &self.bake_cage_path }),
            };
            ui.painter().text(
                rect.min + egui::vec2(12.0, rect.height() - 24.0),
                egui::Align2::LEFT_BOTTOM,
                format!("[BAKE CAGE PREVIEW] {}  |  Orbit: LMB / RMB  |  Pan: MMB  |  Zoom: Scroll", cage_mode_desc),
                egui::FontId::proportional(11.5),
                egui::Color32::from_rgb(0, 225, 255),
            );
        }



        // --- REAL-TIME PERFORMANCE & HARDWARE DIAGNOSTICS HUD ---
        if self.perf_metrics.show_debug_hud {
            let diag_rect = egui::Rect::from_min_size(rect.min + egui::vec2(12.0, 36.0), egui::vec2(460.0, 248.0));
            ui.painter().rect_filled(
                diag_rect,
                egui::Rounding::same(8.0),
                egui::Color32::from_rgba_unmultiplied(16, 18, 24, 242),
            );
            ui.painter().rect_stroke(
                diag_rect,
                egui::Rounding::same(8.0),
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(226, 91, 16)),
            );

            let mut y_cur = diag_rect.min.y + 10.0;
            let left_x = diag_rect.min.x + 12.0;

            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                "Performance & Hardware Diagnostics (F3 to hide)",
                egui::FontId::proportional(12.0),
                egui::Color32::from_rgb(243, 94, 24),
            );
            y_cur += 20.0;

            let gpu_col = if self.perf_metrics.is_software_gpu || self.perf_metrics.is_cpu_fallback {
                egui::Color32::from_rgb(255, 90, 80)
            } else {
                egui::Color32::from_rgb(80, 220, 120)
            };
            let ts_badge = if self.perf_metrics.hardware_timestamps_active {
                "Hardware Timestamps: ACTIVE (Vulkan)"
            } else {
                "Hardware Timestamps: CPU Fallback"
            };
            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("GPU: {}  |  {}", self.perf_metrics.gpu_name, ts_badge),
                egui::FontId::proportional(11.0),
                gpu_col,
            );
            y_cur += 18.0;

            let build_str = if self.perf_metrics.is_debug { "Debug (Unoptimized)" } else { "Release (100% full speed)" };
            let build_col = if self.perf_metrics.is_debug { egui::Color32::from_rgb(255, 180, 40) } else { egui::Color32::from_rgb(180, 205, 230) };
            let aa_short = match self.shader_settings.aa_mode {
                crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa => "NVIDIA DLAA",
                crate::nvidia_dlaa::AntiAliasingMode::Fxaa => "FXAA",
                crate::nvidia_dlaa::AntiAliasingMode::Off => "OFF",
            };
            let denoise_short = if self.shader_settings.denoiser_enabled { "Bilateral Denoise" } else { "No Denoise" };
            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("Build: {}  |  AA: {}  |  {}", build_str, aa_short, denoise_short),
                egui::FontId::proportional(10.5),
                build_col,
            );
            y_cur += 18.0;

            let (hud_fps_str, fps_col) = if self.perf_metrics.is_idle {
                let headroom_fps = if self.perf_metrics.work_latency_ms > 0.05 {
                    (1000.0 / self.perf_metrics.work_latency_ms).min(999.0)
                } else {
                    999.0
                };
                (
                    format!("Status: Standby (Idle) | Compute Latency: {:.2} ms ({:.0} FPS headroom)", self.perf_metrics.work_latency_ms, headroom_fps),
                    egui::Color32::from_rgb(130, 210, 255),
                )
            } else {
                let fps_val = if self.perf_metrics.work_latency_ms > 16.6 {
                    1000.0 / self.perf_metrics.work_latency_ms
                } else {
                    self.perf_metrics.fps.max(60.0)
                };
                let frame_ms = 1000.0 / fps_val;
                let col = if fps_val >= 55.0 {
                    egui::Color32::from_rgb(80, 220, 120)
                } else if fps_val >= 30.0 {
                    egui::Color32::from_rgb(255, 200, 60)
                } else {
                    egui::Color32::from_rgb(255, 80, 80)
                };
                (
                    format!("Throughput: {:.1} FPS ({:.2} ms frame latency)", fps_val, frame_ms),
                    col,
                )
            };
            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                hud_fps_str,
                egui::FontId::proportional(11.5),
                fps_col,
            );
            y_cur += 20.0;

            // Microsecond-accurate GPU execution stage breakdown
            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("GPU Stages: PBR: {:.2} ms  |  PostProc: {:.2} ms  |  Brush: {:.2} ms  |  Comp: {:.2} ms",
                    self.perf_metrics.gpu_pbr_ms,
                    self.perf_metrics.gpu_postprocess_ms,
                    self.perf_metrics.gpu_compute_brush_ms,
                    self.perf_metrics.gpu_compositing_ms
                ),
                egui::FontId::proportional(10.5),
                egui::Color32::from_rgb(90, 215, 255),
            );
            y_cur += 18.0;

            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("Host Latency: Encoder: {:.2} ms  |  Queue Submit: {:.2} ms  |  CPU Total: {:.2} ms",
                    self.perf_metrics.cmd_encoding_ms,
                    self.perf_metrics.queue_submit_ms,
                    self.perf_metrics.work_latency_ms
                ),
                egui::FontId::proportional(10.5),
                egui::Color32::from_rgb(160, 205, 240),
            );
            y_cur += 18.0;

            let (pcie_str, pcie_col) = if self.perf_metrics.texture_transfer_bytes == 0 {
                (
                    "PCIe Transfer: 0 B (Zero-Copy Direct VRAM Stamping Active)".to_string(),
                    egui::Color32::from_rgb(120, 240, 140),
                )
            } else {
                (
                    format!("PCIe Transfer: {:.2} MB/dab", self.perf_metrics.texture_transfer_bytes as f64 / (1024.0 * 1024.0)),
                    egui::Color32::from_rgb(180, 185, 195),
                )
            };
            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                pcie_str,
                egui::FontId::proportional(10.5),
                pcie_col,
            );
            y_cur += 18.0;

            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("Doc: {}x{} (SVT Sparse Buffers)  |  VP: {}x{}  |  Tris: {}",
                    self.project.texture_width, self.project.texture_height,
                    vp_w as u32, vp_h as u32,
                    self.project.mesh.indices.len() / 3
                ),
                egui::FontId::proportional(10.5),
                egui::Color32::from_rgb(150, 155, 165),
            );
            y_cur += 18.0;

            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("History: {} undo steps  |  Tile-Delta Storage: Active", self.history.undo_stack.len()),
                egui::FontId::proportional(10.5),
                egui::Color32::from_rgb(140, 190, 230),
            );
            y_cur += 18.0;

            let (rb_str, rb_col) = if self.has_pending_gpu_readback() {
                ("Staging Readback: ASYNC IN-FLIGHT (Non-Blocking)".to_string(), egui::Color32::from_rgb(255, 200, 80))
            } else {
                ("Staging Readback: IDLE (Zero-Stall Non-Blocking Engine)".to_string(), egui::Color32::from_rgb(120, 220, 160))
            };
            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                rb_str,
                egui::FontId::proportional(10.5),
                rb_col,
            );
        }

        let input = ui.input(|i| i.clone());

        if response.hovered() && input.raw_scroll_delta.y.abs() > 0.1 {
            self.camera.zoom(input.raw_scroll_delta.y * 0.02 * self.camera_zoom_speed);
        }

        if response.hovered() && !ctx.wants_keyboard_input() && input.key_pressed(eframe::egui::Key::F) && !input.modifiers.command && !input.modifiers.ctrl {
            self.camera.reset();
        }

        let mut gesture_overlay: Option<(egui::Pos2, String, Option<f32>, Option<f32>)> = None;

        // 1. Shift + RMB: Rotate HDRI environment lighting
        if response.dragged_by(eframe::egui::PointerButton::Secondary) && input.modifiers.shift {
            let delta = input.pointer.delta();
            self.renderer.hdri.rotation = (self.renderer.hdri.rotation + delta.x * 0.5).rem_euclid(360.0);
            if let Some(pos) = input.pointer.hover_pos() {
                gesture_overlay = Some((pos, format!("HDRI Environment: {:.0}°", self.renderer.hdri.rotation), None, None));
            }
        }
        // 2. Ctrl + RMB: In-Viewport Substance Brush Radius (Size) & Hardness Gesture
        else if response.dragged_by(eframe::egui::PointerButton::Secondary) && input.modifiers.command {
            let delta = input.pointer.delta();
            self.brush_engine.settings.radius = (self.brush_engine.settings.radius + delta.x * 0.5).clamp(2.0, 200.0);
            self.brush_engine.settings.hardness = (self.brush_engine.settings.hardness - delta.y * 0.005).clamp(0.0, 1.0);
            if let Some(pos) = input.pointer.hover_pos() {
                let msg = format!("Size: {:.1} px  |  Hardness: {:.0}%", self.brush_engine.settings.radius, self.brush_engine.settings.hardness * 100.0);
                gesture_overlay = Some((pos, msg, Some(self.brush_engine.settings.radius), Some(self.brush_engine.settings.hardness)));
            }
        }
        // 3. Ctrl + LMB: In-Viewport Substance Brush Flow & Opacity Gesture
        else if response.dragged_by(eframe::egui::PointerButton::Primary) && input.modifiers.command {
            let delta = input.pointer.delta();
            self.brush_engine.settings.flow = (self.brush_engine.settings.flow + delta.x * 0.005).clamp(0.01, 1.0);
            self.brush_engine.settings.opacity = (self.brush_engine.settings.opacity - delta.y * 0.005).clamp(0.01, 1.0);
            if let Some(pos) = input.pointer.hover_pos() {
                let msg = format!("Flow: {:.0}%  |  Opacity: {:.0}%", self.brush_engine.settings.flow * 100.0, self.brush_engine.settings.opacity * 100.0);
                gesture_overlay = Some((pos, msg, None, None));
            }
        }
        // 4. Alt + RMB: Substance Camera Dolly / Zoom
        else if response.dragged_by(eframe::egui::PointerButton::Secondary) && input.modifiers.alt {
            let delta = input.pointer.delta();
            let zoom_delta = (delta.x - delta.y) * 0.02 * self.camera_zoom_speed;
            self.camera.zoom(zoom_delta);
        }
        // 5. Alt + MMB or MMB: Substance Camera Pan
        else if response.dragged_by(eframe::egui::PointerButton::Middle) && !input.key_down(egui::Key::S) {
            let delta = input.pointer.delta();
            self.camera.pan(delta.x * self.camera_pan_speed, delta.y * self.camera_pan_speed);
        }
        // 6. Stencil Interactive Transform: Hold S (LMB = Rotate, RMB = Scale, MMB = Pan, Z = Reset)
        else if (self.brush_engine.settings.mode == ToolMode::Projection || self.brush_engine.settings.stencil.enabled)
            && input.key_down(egui::Key::S)
        {
            if input.key_pressed(egui::Key::Z) {
                self.brush_engine.settings.stencil.pos = glam::Vec2::ZERO;
                self.brush_engine.settings.stencil.scale = 1.0;
                self.brush_engine.settings.stencil.angle = 0.0;
                self.status_message = "Stencil transform reset to default".to_string();
                ctx.request_repaint();
            } else if response.dragged_by(eframe::egui::PointerButton::Primary) {
                let delta = input.pointer.delta();
                self.brush_engine.settings.stencil.angle += delta.x * 0.015;
                self.status_message = format!("Stencil Rotation: {:.1} deg", self.brush_engine.settings.stencil.angle.to_degrees());
                ctx.request_repaint();
            } else if response.dragged_by(eframe::egui::PointerButton::Secondary) {
                let delta = input.pointer.delta();
                let factor = 1.0 + (delta.x - delta.y) * 0.01;
                self.brush_engine.settings.stencil.scale = (self.brush_engine.settings.stencil.scale * factor).clamp(0.05, 10.0);
                self.status_message = format!("Stencil Scale: {:.2}x", self.brush_engine.settings.stencil.scale);
                ctx.request_repaint();
            } else if response.dragged_by(eframe::egui::PointerButton::Middle) {
                let delta = input.pointer.delta();
                self.brush_engine.settings.stencil.pos.x += delta.x / rect.width();
                self.brush_engine.settings.stencil.pos.y += delta.y / rect.height();
                self.status_message = format!("Stencil Position: ({:.2}, {:.2})", self.brush_engine.settings.stencil.pos.x, self.brush_engine.settings.stencil.pos.y);
                ctx.request_repaint();
            }
        }
        // 7. Alt + LMB (or RMB without modifiers): Substance Camera Orbit (or Left-drag when Bake modal is open)
        else if ((response.dragged_by(eframe::egui::PointerButton::Primary) && input.modifiers.alt)
            || (response.dragged_by(eframe::egui::PointerButton::Secondary) && !input.modifiers.command && !input.modifiers.shift && !input.modifiers.alt)
            || (self.show_bake_modal && response.dragged_by(eframe::egui::PointerButton::Primary)))
            && !input.key_down(egui::Key::S)
        {
            let delta = input.pointer.delta();
            self.camera.orbit(delta.x * self.camera_orbit_speed, delta.y * self.camera_orbit_speed);
        }
        // 7.5. Vector Path Tool 3D Surface Interaction
        else if self.brush_engine.settings.mode == ToolMode::Path
            && !input.modifiers.alt && !input.modifiers.command && !self.show_bake_modal && !input.key_down(egui::Key::S)
        {
            if input.key_pressed(egui::Key::Enter) {
                self.commit_path_tool(ctx);
            } else if input.key_pressed(egui::Key::Escape) {
                self.brush_engine.settings.path_tool.clear();
                self.sync_and_evaluate_active_path_layer(ctx);
                self.status_message = "Path cleared".to_string();
                ctx.request_repaint();
            } else if (input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace))
                && self.brush_engine.settings.path_tool.selected_point.is_some()
            {
                if let Some(idx) = self.brush_engine.settings.path_tool.selected_point {
                    if idx < self.brush_engine.settings.path_tool.points.len() {
                        self.brush_engine.settings.path_tool.points.remove(idx);
                        self.brush_engine.settings.path_tool.selected_point = None;
                        self.sync_and_evaluate_active_path_layer(ctx);
                        self.status_message = format!("Removed path point {}", idx + 1);
                        ctx.request_repaint();
                    }
                }
            } else if let Some(mouse_pos) = input.pointer.hover_pos() {
                let local_x = mouse_pos.x - rect.min.x;
                let local_y = mouse_pos.y - rect.min.y;
                let ray = self.camera.screen_to_ray(local_x, local_y, vp_w, vp_h);
                let hit_opt = self.project.mesh.raycast(&ray);

                let mut hovered_pt_idx = None;
                for (i, pt) in self.brush_engine.settings.path_tool.points.iter().enumerate() {
                    if let Some(s_pos) = self.camera.world_to_screen(pt.position_3d, vp_w, vp_h) {
                        let screen_p = rect.min + egui::vec2(s_pos.x, s_pos.y);
                        if (screen_p - mouse_pos).length() < 14.0 {
                            hovered_pt_idx = Some(i);
                            break;
                        }
                    }
                }

                if response.clicked() {
                    if let Some(idx) = hovered_pt_idx {
                        self.brush_engine.settings.path_tool.selected_point = Some(idx);
                        self.status_message = format!("Selected path point {}", idx + 1);
                    } else if let Some(hit) = hit_opt {
                        self.brush_engine.settings.path_tool.points.push(crate::brush::PathControlPoint {
                            position_3d: hit.point,
                            normal: hit.normal,
                            uv: hit.uv,
                            radius_scale: 1.0,
                        });
                        let new_idx = self.brush_engine.settings.path_tool.points.len() - 1;
                        self.brush_engine.settings.path_tool.selected_point = Some(new_idx);
                        self.sync_and_evaluate_active_path_layer(ctx);
                        self.status_message = format!("Added path point {} (Enter to commit, Esc to clear)", new_idx + 1);
                    }
                    ctx.request_repaint();
                } else if response.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(idx) = hovered_pt_idx {
                        self.brush_engine.settings.path_tool.selected_point = Some(idx);
                        self.brush_engine.settings.path_tool.is_dragging = true;
                    } else if let Some(hit) = hit_opt {
                        // Start freehand path stroke sketching directly across 3D surface
                        self.brush_engine.settings.path_tool.is_sketching = true;
                        self.brush_engine.settings.path_tool.points.push(crate::brush::PathControlPoint {
                            position_3d: hit.point,
                            normal: hit.normal,
                            uv: hit.uv,
                            radius_scale: 1.0,
                        });
                        let new_idx = self.brush_engine.settings.path_tool.points.len() - 1;
                        self.brush_engine.settings.path_tool.selected_point = Some(new_idx);
                        ctx.request_repaint();
                    }
                } else if response.dragged_by(egui::PointerButton::Primary) {
                    if self.brush_engine.settings.path_tool.is_dragging {
                        if let Some(sel_idx) = self.brush_engine.settings.path_tool.selected_point {
                            if let Some(hit) = hit_opt {
                                if sel_idx < self.brush_engine.settings.path_tool.points.len() {
                                    self.brush_engine.settings.path_tool.points[sel_idx].position_3d = hit.point;
                                    self.brush_engine.settings.path_tool.points[sel_idx].normal = hit.normal;
                                    self.brush_engine.settings.path_tool.points[sel_idx].uv = hit.uv;
                                    ctx.request_repaint();
                                }
                            }
                        }
                    } else if self.brush_engine.settings.path_tool.is_sketching {
                        if let Some(hit) = hit_opt {
                            let doc_w = self.project.texture_width;
                            let world_rad = ((self.brush_engine.settings.radius / (doc_w as f32).max(1.0)) * 2.0).clamp(0.005, 0.5);
                            let min_dist = (world_rad * 1.5).max(0.025);
                            let can_add = self.brush_engine.settings.path_tool.points.last()
                                .map(|last| (hit.point - last.position_3d).length() > min_dist)
                                .unwrap_or(true);
                            if can_add {
                                self.brush_engine.settings.path_tool.points.push(crate::brush::PathControlPoint {
                                    position_3d: hit.point,
                                    normal: hit.normal,
                                    uv: hit.uv,
                                    radius_scale: 1.0,
                                });
                                let new_idx = self.brush_engine.settings.path_tool.points.len() - 1;
                                self.brush_engine.settings.path_tool.selected_point = Some(new_idx);
                                self.status_message = format!("Drawing path stroke ({} points)", self.brush_engine.settings.path_tool.points.len());
                                ctx.request_repaint();
                            }
                        }
                    }
                } else if response.drag_stopped() || !input.pointer.any_down() {
                    let was_interacting = self.brush_engine.settings.path_tool.is_dragging || self.brush_engine.settings.path_tool.is_sketching;
                    self.brush_engine.settings.path_tool.is_dragging = false;
                    self.brush_engine.settings.path_tool.is_sketching = false;
                    if was_interacting {
                        self.sync_and_evaluate_active_path_layer(ctx);
                    }
                }
            }
        }
        // 7.6. Decal Stamp Tool 3D Surface Interaction
        else if self.brush_engine.settings.mode == ToolMode::Stamp
            && !input.modifiers.alt && !input.modifiers.command && !self.show_bake_modal && !input.key_down(egui::Key::S)
        {
            if response.clicked() || response.drag_started_by(egui::PointerButton::Primary) {
                if let Some(mouse_pos) = input.pointer.hover_pos() {
                    let local_x = mouse_pos.x - rect.min.x;
                    let local_y = mouse_pos.y - rect.min.y;
                    let ray = self.camera.screen_to_ray(local_x, local_y, vp_w, vp_h);
                    if let Some(hit) = self.project.mesh.raycast(&ray) {
                        let act_idx = self.project.active_layer_idx;
                        if act_idx < self.project.layers.len() {
                            let is_fill = self.project.layers[act_idx].layer_type != LayerType::Paint;
                            let is_mask = self.brush_engine.settings.target == PaintTarget::Mask;
                            if is_fill && !is_mask {
                                self.status_message = "Cannot stamp on Fill or Path layer. Switch target to Mask or add a Paint layer.".to_string();
                            } else {
                                self.flush_pending_gpu_readbacks(ctx);
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    if wgpu.gpu_layers.len() != self.project.layers.len() {
                                        wgpu.ensure_gpu_layers(&mut self.project);
                                    }
                                }

                                let target = self.brush_engine.settings.target;
                                let channels = if target == PaintTarget::Mask {
                                    crate::channel::ChannelFlags::none()
                                } else {
                                    let b = self.brush_engine.settings.channels;
                                    let l = self.project.layers[act_idx].channel_flags;
                                    crate::channel::ChannelFlags {
                                        base_color: b.base_color && l.base_color,
                                        roughness: b.roughness && l.roughness,
                                        metallic: b.metallic && l.metallic,
                                        height: b.height && l.height,
                                        normal: (b.normal || b.height) && (l.normal || l.height),
                                        emission: b.emission && l.emission,
                                        opacity: b.opacity && l.opacity,
                                        clearcoat: b.clearcoat && l.clearcoat,
                                        anisotropy: b.anisotropy && l.anisotropy,
                                        ao: false,
                                    }
                                };

                                let doc_w = self.project.texture_width;
                                let doc_h = self.project.texture_height;
                                let world_rad = ((self.brush_engine.settings.radius / (doc_w as f32).max(1.0)) * 2.0).clamp(0.005, 0.5);

                                self.project.layers[act_idx].begin_stroke_recording(target, channels);

                                let stamp_state = &mut self.brush_engine.settings.stamp_tool;
                                if stamp_state.randomize_rotation {
                                    let rand_deg = ((std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() % 3600) as f32) / 10.0;
                                    stamp_state.rotation_angle = rand_deg;
                                }

                                let rot_rad = stamp_state.rotation_angle.to_radians();
                                let ref_up = if hit.normal.y.abs() > 0.95 { glam::Vec3::Z } else { glam::Vec3::Y };
                                let bitan = hit.normal.cross(ref_up).normalize();
                                let tan = bitan.cross(hit.normal).normalize();
                                let rot_tan = tan * rot_rad.cos() + bitan * rot_rad.sin();

                                let stroke_style = stamp_state.stroke_style;
                                let decal_type = stamp_state.decal_type;
                                let heat_tint = stamp_state.heat_tint;
                                let height_depth = stamp_state.height_depth;
                                let offset_pos = hit.point + hit.normal * stamp_state.surface_offset;

                                let mesh = self.project.mesh.clone();
                                let layer = &mut self.project.layers[act_idx];

                                self.brush_engine.stamp_3d_decal(
                                    layer,
                                    &mesh,
                                    offset_pos,
                                    hit.normal,
                                    rot_tan,
                                    world_rad,
                                    stroke_style,
                                    decal_type,
                                    heat_tint,
                                    0.0,
                                    height_depth,
                                );

                                layer.update_normal_from_height(1.0);

                                let desc = format!("Stamp Decal: {}", if stroke_style == crate::brush::PathStrokeStyle::SmartDecal { decal_type.name() } else { stroke_style.name() });
                                if let Some(action) = layer.finish_stroke_delta(act_idx, &desc) {
                                    self.history.push(&desc, action);
                                }
                                if let Some(d_rect) = layer.dirty_rect {
                                    if let Some(wgpu) = &mut self.wgpu_viewport {
                                        if let Some(gpu_layer) = wgpu.gpu_layers.get(act_idx) {
                                            wgpu.gpu_compositor.update_layer(layer, gpu_layer, doc_w, doc_h);
                                            wgpu.composite_and_update_active_rect(&self.project, act_idx, Some(d_rect));
                                        }
                                    }
                                    self.update_uv_texture_rect(ctx, d_rect.0, d_rect.1, d_rect.2, d_rect.3);
                                } else {
                                    if let Some(wgpu) = &mut self.wgpu_viewport {
                                        wgpu.sync_textures(&mut self.project);
                                    }
                                    self.update_uv_texture(ctx);
                                }
                                ctx.request_repaint();
                            }
                        }
                    }
                }
            }
        }
        // 8. Left-drag WITHOUT Alt and WITHOUT Ctrl: 3D PROJECTION PAINTING! (Disabled when Bake modal is open)
        else if (response.dragged_by(egui::PointerButton::Primary) || response.clicked())
            && !input.modifiers.alt && !input.modifiers.command && !self.show_bake_modal && !input.key_down(egui::Key::S)
        {
                if !self.is_stroke_active {
                    self.flush_pending_gpu_readbacks(ctx);
                    if let Some(wgpu) = &mut self.wgpu_viewport {
                        if wgpu.gpu_layers.len() != self.project.layers.len() {
                            wgpu.ensure_gpu_layers(&mut self.project);
                        }
                    }
                    if let Some(layer) = self.project.active_layer_mut() {
                        let target = self.brush_engine.settings.target;
                        let channels = if target == PaintTarget::Mask {
                            crate::channel::ChannelFlags::none()
                        } else {
                            let b = self.brush_engine.settings.channels;
                            let l = layer.channel_flags;
                            crate::channel::ChannelFlags {
                                base_color: b.base_color && l.base_color,
                                roughness: b.roughness && l.roughness,
                                metallic: b.metallic && l.metallic,
                                height: b.height && l.height,
                                normal: (b.normal || b.height) && (l.normal || l.height),
                                emission: b.emission && l.emission,
                                opacity: b.opacity && l.opacity,
                                clearcoat: b.clearcoat && l.clearcoat,
                                anisotropy: b.anisotropy && l.anisotropy,
                                ao: false,
                            }
                        };
                        layer.begin_stroke_recording(target, channels);
                        self.is_stroke_active = true;
                        self.gpu_stroke_dirty_rect = None;
                    }
                    self.polygon_fill_drag_tris.clear();
                }

                if let Some(raw_mouse_pos) = input.pointer.hover_pos() {
                    let pressure = input.events.iter().find_map(|e| match e {
                        egui::Event::Touch { force: Some(f), .. } => Some(*f),
                        _ => None,
                    }).unwrap_or(1.0).clamp(0.05, 1.0);
                    self.last_stylus_pressure = pressure;

                    // Lazy Mouse evaluation
                    let mouse_pos = if self.brush_engine.settings.lazy_mouse_enabled {
                        let l_pos = self.brush_engine.lazy_screen_pos.unwrap_or([raw_mouse_pos.x, raw_mouse_pos.y]);
                        let cur_vec = egui::vec2(raw_mouse_pos.x - l_pos[0], raw_mouse_pos.y - l_pos[1]);
                        let lazy_dist = (self.brush_engine.settings.lazy_mouse_radius * vp_w).max(12.0);
                        if cur_vec.length() > lazy_dist {
                            let pull = cur_vec.normalized() * (cur_vec.length() - lazy_dist);
                            let new_p = egui::pos2(l_pos[0] + pull.x, l_pos[1] + pull.y);
                            self.brush_engine.lazy_screen_pos = Some([new_p.x, new_p.y]);
                            new_p
                        } else {
                            egui::pos2(l_pos[0], l_pos[1])
                        }
                    } else {
                        self.brush_engine.lazy_screen_pos = Some([raw_mouse_pos.x, raw_mouse_pos.y]);
                        raw_mouse_pos
                    };

                    let local_x = mouse_pos.x - rect.min.x;
                    let local_y = mouse_pos.y - rect.min.y;

                    let ray = self.camera.screen_to_ray(
                        local_x,
                        local_y,
                        vp_w,
                        vp_h,
                    );

                    if let Some(hit) = self.project.mesh.raycast(&ray) {
                        if !self.project.mesh.is_triangle_visible(hit.triangle_idx) {
                            // Submesh is hidden, skip painting
                        } else {
                            if self.project.auto_switch_texture_set {
                                if let Some(target_ts_idx) = self.project.texture_set_index_for_triangle(hit.triangle_idx) {
                                    if target_ts_idx != self.project.active_texture_set_idx {
                                        self.project.switch_texture_set(target_ts_idx);
                                        if let Some(wgpu) = &mut self.wgpu_viewport {
                                            wgpu.sync_textures(&mut self.project);
                                        }
                                    }
                                }
                            }
                            if self.brush_engine.settings.mode == ToolMode::Clone && (input.modifiers.alt || input.key_down(egui::Key::V)) {
                                self.brush_engine.settings.clone_source_uv = Some(hit.uv);
                                self.status_message = format!("Clone Source Set: UV ({:.3}, {:.3})", hit.uv.x, hit.uv.y);
                            } else if self.brush_engine.settings.mode == ToolMode::Eyedropper {
                            let sampled = self.project.composite_textures.base_color.sample_bilinear(hit.uv);
                            self.brush_engine.settings.paint_color = sampled;
                            self.brush_engine.settings.paint_roughness = self.project.composite_textures.roughness.sample_bilinear(hit.uv);
                            self.brush_engine.settings.paint_metallic = self.project.composite_textures.metallic.sample_bilinear(hit.uv);
                            self.brush_engine.settings.paint_height = self.project.composite_textures.height_map.sample_bilinear(hit.uv);
                            self.brush_engine.settings.paint_emissive = self.project.composite_textures.emissive.sample_bilinear(hit.uv);
                            self.brush_engine.settings.paint_opacity = self.project.composite_textures.opacity.sample_bilinear(hit.uv);
                            self.status_message = format!("Sampled Material: Color RGBA({:.2}, {:.2}, {:.2}, {:.2}), Rough {:.2}, Metal {:.2}, Emis RGB({:.2}, {:.2}, {:.2})", sampled.x, sampled.y, sampled.z, sampled.w, self.brush_engine.settings.paint_roughness, self.brush_engine.settings.paint_metallic, self.brush_engine.settings.paint_emissive.x, self.brush_engine.settings.paint_emissive.y, self.brush_engine.settings.paint_emissive.z);
                        } else if self.brush_engine.settings.mode == ToolMode::FillBucket {
                            let mode = self.brush_engine.settings.polygon_fill_mode;
                            let tris = match mode {
                                crate::brush::PolygonFillMode::Triangle => vec![hit.triangle_idx],
                                crate::brush::PolygonFillMode::Polygon => self.project.mesh.get_polygon_quad(hit.triangle_idx),
                                crate::brush::PolygonFillMode::UvIsland => self.project.mesh.get_connected_uv_island(hit.triangle_idx),
                                crate::brush::PolygonFillMode::MeshElement => self.project.mesh.get_connected_mesh_element(hit.triangle_idx),
                            };
                            let new_tris: Vec<usize> = tris.into_iter().filter(|t| !self.polygon_fill_drag_tris.contains(t)).collect();
                            if !new_tris.is_empty() {
                                for &t in &new_tris {
                                    self.polygon_fill_drag_tris.insert(t);
                                }
                                let tri_uvs: Vec<(glam::Vec2, glam::Vec2, glam::Vec2)> = new_tris.iter().map(|&t| {
                                    let i0 = self.project.mesh.indices[t * 3] as usize;
                                    let i1 = self.project.mesh.indices[t * 3 + 1] as usize;
                                    let i2 = self.project.mesh.indices[t * 3 + 2] as usize;
                                    (
                                        self.project.mesh.vertices[i0].uv,
                                        self.project.mesh.vertices[i1].uv,
                                        self.project.mesh.vertices[i2].uv,
                                    )
                                }).collect();

                                let act_idx = self.project.active_layer_idx;
                                if act_idx < self.project.layers.len() {
                                    let layer = &mut self.project.layers[act_idx];

                                    if self.brush_engine.settings.target == PaintTarget::Mask {
                                        if layer.has_mask {
                                            let val = self.brush_engine.settings.polygon_fill_value;
                                            let w = layer.mask_buf.width;
                                            let h = layer.mask_buf.height;
                                            let mut b_min_x = u32::MAX;
                                            let mut b_min_y = u32::MAX;
                                            let mut b_max_x = 0;
                                            let mut b_max_y = 0;
                                            for &(uv0, uv1, uv2) in &tri_uvs {
                                                let min_x = ((uv0.x.min(uv1.x).min(uv2.x) * w as f32).floor() as i32).clamp(0, w as i32 - 1) as u32;
                                                let max_x = ((uv0.x.max(uv1.x).max(uv2.x) * w as f32).ceil() as i32).clamp(0, w as i32 - 1) as u32;
                                                let min_y = ((uv0.y.min(uv1.y).min(uv2.y) * h as f32).floor() as i32).clamp(0, h as i32 - 1) as u32;
                                                let max_y = ((uv0.y.max(uv1.y).max(uv2.y) * h as f32).ceil() as i32).clamp(0, h as i32 - 1) as u32;
                                                b_min_x = b_min_x.min(min_x);
                                                b_min_y = b_min_y.min(min_y);
                                                b_max_x = b_max_x.max(max_x);
                                                b_max_y = b_max_y.max(max_y);
                                                layer.record_tiles_for_rect(min_x, min_y, max_x, max_y);
                                                crate::mesh::Mesh::rasterize_triangle_uv(w, h, uv0, uv1, uv2, |px, py| {
                                                    layer.mask_buf.set_pixel(px, py, val);
                                                });
                                            }
                                            if b_min_x <= b_max_x && b_min_y <= b_max_y {
                                                layer.mark_dirty_mask_rect(b_min_x, b_min_y, b_max_x, b_max_y);
                                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                                    if let Some(gpu_layer) = wgpu.gpu_layers.get(act_idx) {
                                                        let pw = self.project.texture_width;
                                                        let ph = self.project.texture_height;
                                                        wgpu.gpu_compositor.update_layer(layer, gpu_layer, pw, ph);
                                                        wgpu.composite_and_update_active_rect(&self.project, act_idx, Some((b_min_x, b_min_y, b_max_x, b_max_y)));
                                                        if let Some(preview_id) = self.mask_preview_layer_id {
                                                            wgpu.sync_mask_preview_full(&self.project, preview_id);
                                                        }
                                                    }
                                                }
                                                self.need_cpu_composite_sync = true;
                                                self.update_uv_texture_rect(ctx, b_min_x, b_min_y, b_max_x, b_max_y);
                                            }
                                            ctx.request_repaint();
                                            self.status_message = format!("Polygon Fill ({}) filled {} triangles on mask", mode.name(), new_tris.len());
                                        } else {
                                            self.status_message = "Active layer does not have a mask. Add a mask to use Polygon Fill on mask.".to_string();
                                        }
                                    } else {
                                        // Target: Material
                                        if layer.layer_type == LayerType::Paint {
                                            let col = self.brush_engine.settings.paint_color;
                                            let rough = self.brush_engine.settings.paint_roughness;
                                            let metal = self.brush_engine.settings.paint_metallic;
                                            let height = self.brush_engine.settings.paint_height;
                                            let emis = self.brush_engine.settings.paint_emissive;
                                            let opac = self.brush_engine.settings.paint_opacity;
                                            let do_col = layer.channel_flags.base_color;
                                            let do_rough = layer.channel_flags.roughness;
                                            let do_metal = layer.channel_flags.metallic;
                                            let do_h = layer.channel_flags.height;
                                            let do_emis = layer.channel_flags.emission;
                                            let do_opac = layer.channel_flags.opacity;
                                            let w = layer.base_color_buf.width;
                                            let h = layer.base_color_buf.height;
                                            let mut b_min_x = u32::MAX;
                                            let mut b_min_y = u32::MAX;
                                            let mut b_max_x = 0;
                                            let mut b_max_y = 0;
                                            for &(uv0, uv1, uv2) in &tri_uvs {
                                                let min_x = ((uv0.x.min(uv1.x).min(uv2.x) * w as f32).floor() as i32).clamp(0, w as i32 - 1) as u32;
                                                let max_x = ((uv0.x.max(uv1.x).max(uv2.x) * w as f32).ceil() as i32).clamp(0, w as i32 - 1) as u32;
                                                let min_y = ((uv0.y.min(uv1.y).min(uv2.y) * h as f32).floor() as i32).clamp(0, h as i32 - 1) as u32;
                                                let max_y = ((uv0.y.max(uv1.y).max(uv2.y) * h as f32).ceil() as i32).clamp(0, h as i32 - 1) as u32;
                                                b_min_x = b_min_x.min(min_x);
                                                b_min_y = b_min_y.min(min_y);
                                                b_max_x = b_max_x.max(max_x);
                                                b_max_y = b_max_y.max(max_y);
                                                layer.record_tiles_for_rect(min_x, min_y, max_x, max_y);
                                                crate::mesh::Mesh::rasterize_triangle_uv(w, h, uv0, uv1, uv2, |px, py| {
                                                    if do_col {
                                                        layer.base_color_buf.set_pixel(px, py, col);
                                                    } else {
                                                        let mut cur = layer.base_color_buf.get_pixel(px, py);
                                                        cur.w = 1.0;
                                                        layer.base_color_buf.set_pixel(px, py, cur);
                                                    }
                                                    if do_rough { layer.roughness_buf.set_pixel(px, py, rough); }
                                                    if do_metal { layer.metallic_buf.set_pixel(px, py, metal); }
                                                    if do_h { layer.height_buf.set_pixel(px, py, height); }
                                                    if do_emis { layer.emissive_buf.set_pixel(px, py, emis); }
                                                    if do_opac { layer.opacity_buf.set_pixel(px, py, opac); }
                                                });
                                            }
                                            if b_min_x <= b_max_x && b_min_y <= b_max_y {
                                                if do_h {
                                                    layer.update_normal_from_height(1.0);
                                                }
                                                layer.mark_dirty_rect(b_min_x, b_min_y, b_max_x, b_max_y);
                                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                                    if let Some(gpu_layer) = wgpu.gpu_layers.get(act_idx) {
                                                        let pw = self.project.texture_width;
                                                        let ph = self.project.texture_height;
                                                        wgpu.gpu_compositor.update_layer(layer, gpu_layer, pw, ph);
                                                        wgpu.composite_and_update_active_rect(&self.project, act_idx, Some((b_min_x, b_min_y, b_max_x, b_max_y)));
                                                    }
                                                }
                                                self.need_cpu_composite_sync = true;
                                                self.update_uv_texture_rect(ctx, b_min_x, b_min_y, b_max_x, b_max_y);
                                            }
                                            ctx.request_repaint();
                                            self.status_message = format!("Polygon Fill ({}) filled {} triangles with material", mode.name(), new_tris.len());
                                        } else {
                                            self.status_message = "Cannot fill material on Fill layer. Switch target to Mask or add a Paint layer.".to_string();
                                        }
                                    }
                                }
                            }
                        } else {
                            let act_idx = self.project.active_layer_idx;
                            if act_idx < self.project.layers.len() {
                                let is_blocked = self.brush_engine.settings.target == PaintTarget::Material
                                    && self.project.layers[act_idx].layer_type != LayerType::Paint;
                                if is_blocked {
                                    self.status_message = "Direct painting blocked on Fill and Path layers. Switch target to Mask or add a Paint layer.".to_string();
                                } else {
                                    let (effective_radius, effective_flow, effective_hardness) = self.brush_engine.settings.compute_effective_dynamics(pressure);
                                    let ray2 = self.camera.screen_to_ray(
                                        local_x + effective_radius,
                                        local_y,
                                        vp_w,
                                        vp_h,
                                    );
                                    let point2 = ray2.origin + ray2.direction * hit.t;
                                    let world_radius = (hit.point - point2).length().max(0.001);

                                    let orig_flow = self.brush_engine.settings.flow;
                                    let orig_hardness = self.brush_engine.settings.hardness;
                                    self.brush_engine.settings.flow = effective_flow;
                                    self.brush_engine.settings.hardness = effective_hardness;

                                    let t_stroke = std::time::Instant::now();
                                    let doc_w = self.project.texture_width;
                                    let doc_h = self.project.texture_height;


                                    let orig_color = self.brush_engine.settings.paint_color;
                                    let mut skip_stroke = false;

                                    if self.brush_engine.settings.mode == ToolMode::Projection || self.brush_engine.settings.stencil.enabled {
                                        let screen_uv = glam::Vec2::new(local_x / vp_w.max(1.0), local_y / vp_h.max(1.0));
                                        if let Some(sample) = self.brush_engine.sample_stencil(screen_uv) {
                                            if self.brush_engine.settings.stencil.mask_only {
                                                let lum = (sample.x * 0.299 + sample.y * 0.587 + sample.z * 0.114) * sample.w;
                                                self.brush_engine.settings.flow *= lum * self.brush_engine.settings.stencil.opacity;
                                            } else {
                                                self.brush_engine.settings.paint_color = glam::Vec4::new(sample.x, sample.y, sample.z, sample.w * self.brush_engine.settings.stencil.opacity);
                                            }
                                        } else if self.brush_engine.settings.stencil.enabled {
                                            skip_stroke = true;
                                        }
                                    }

                                    let mut gpu_synced = false;
                                    if !skip_stroke {
                                        let is_gpu_compute = (self.brush_engine.settings.mode == ToolMode::Brush || self.brush_engine.settings.mode == ToolMode::Eraser)
                                            && self.wgpu_viewport.is_some();

                                        if is_gpu_compute {
                                            let (points, normals) = match (self.brush_engine.last_hit_pos, self.brush_engine.last_hit_normal) {
                                                (Some(prev_pos), Some(prev_norm)) => {
                                                    let delta = hit.point - prev_pos;
                                                    if self.brush_engine.settings.follow_path && delta.length_squared() > 1e-8 {
                                                        let n = hit.normal.normalize_or_zero();
                                                        let tan_delta = delta - n * n.dot(delta);
                                                        if tan_delta.length_squared() > 1e-8 {
                                                            let up = if n.y.abs() < 0.95 { glam::Vec3::Y } else { glam::Vec3::X };
                                                            let t_axis = n.cross(up).normalize();
                                                            let b_axis = n.cross(t_axis);
                                                            let u_proj = tan_delta.dot(t_axis);
                                                            let v_proj = tan_delta.dot(b_axis);
                                                            self.brush_engine.follow_path_angle = v_proj.atan2(u_proj) + self.brush_engine.settings.follow_path_offset;
                                                        }
                                                    }
                                                    let dist = delta.length();
                                                    let step_dist = (world_radius * self.brush_engine.settings.spacing).max(0.001);
                                                    let steps = ((dist / step_dist).ceil() as usize).clamp(1, 16);

                                                    if steps > 1 {
                                                        let mut pts = Vec::with_capacity(steps);
                                                        let mut nrms = Vec::with_capacity(steps);
                                                        for i in 1..=steps {
                                                            let t = i as f32 / steps as f32;
                                                            pts.push(prev_pos.lerp(hit.point, t));
                                                            nrms.push(prev_norm.lerp(hit.normal, t).normalize());
                                                        }
                                                        (pts, nrms)
                                                    } else {
                                                        (vec![hit.point], vec![hit.normal])
                                                    }
                                                }
                                                _ => (vec![hit.point], vec![hit.normal]),
                                            };

                                            let wgpu = self.wgpu_viewport.as_mut().unwrap();
                                            let mut any_stamped = false;

                                            for (p_base, n_base) in points.into_iter().zip(normals.into_iter()) {
                                                let sym_pts = self.brush_engine.get_symmetry_points(p_base, n_base);
                                                for (p, n) in sym_pts {
                                                    if let Some((uniform, gpu_tris)) = self.brush_engine.build_gpu_stamp_data(
                                                        &self.project.mesh,
                                                        p,
                                                        n,
                                                        world_radius,
                                                        doc_w,
                                                        doc_h,
                                                    ) {
                                                        let s_min_x = uniform.bounds_min[0].max(0) as u32;
                                                        let s_min_y = uniform.bounds_min[1].max(0) as u32;
                                                        let s_max_x = (uniform.bounds_max[0] as u32).min(doc_w.saturating_sub(1));
                                                        let s_max_y = (uniform.bounds_max[1] as u32).min(doc_h.saturating_sub(1));

                                                        self.gpu_stroke_dirty_rect = Some(match self.gpu_stroke_dirty_rect {
                                                            Some((x0, y0, x1, y1)) => (x0.min(s_min_x), y0.min(s_min_y), x1.max(s_max_x), y1.max(s_max_y)),
                                                            None => (s_min_x, s_min_y, s_max_x, s_max_y),
                                                        });

                                                        wgpu.stamp_3d_gpu(act_idx, &uniform, &gpu_tris);
                                                        any_stamped = true;
                                                    }
                                                }
                                            }

                                            if any_stamped {
                                                wgpu.composite_and_update_active_rect(&self.project, act_idx, self.gpu_stroke_dirty_rect);
                                                self.perf_metrics.texture_transfer_bytes = 0;
                                                gpu_synced = true;
                                            }

                                            self.brush_engine.last_hit_pos = Some(hit.point);
                                            self.brush_engine.last_hit_normal = Some(hit.normal);
                                        } else {
                                            // Software fallback for specialized carrier tools (e.g. smudge, clone) or CPU mode
                                            let layer = &mut self.project.layers[act_idx];
                                            let mesh = &self.project.mesh;
                                            self.brush_engine.paint_stroke_3d(layer, mesh, hit.point, hit.normal, world_radius);

                                            if let Some(wgpu) = &mut self.wgpu_viewport {
                                                let active_layer = &mut self.project.layers[act_idx];
                                                if let Some(d_rect) = active_layer.dirty_rect {
                                                    if self.brush_engine.settings.channels.clearcoat
                                                        || self.brush_engine.settings.channels.emission
                                                        || self.brush_engine.settings.channels.opacity
                                                        || self.brush_engine.settings.channels.anisotropy
                                                    {
                                                        wgpu.sync_extended_dirty_rect(active_layer, d_rect, self.brush_engine.settings.channels);
                                                        self.need_cpu_composite_sync = true;
                                                    }
                                                    if let Some(gpu_layer) = wgpu.gpu_layers.get(act_idx) {
                                                        wgpu.gpu_compositor.update_layer(active_layer, gpu_layer, doc_w, doc_h);
                                                        wgpu.composite_and_update_active_rect(&self.project, act_idx, Some(d_rect));
                                                        let rw = (d_rect.2.saturating_sub(d_rect.0) + 1) as u64;
                                                        let rh = (d_rect.3.saturating_sub(d_rect.1) + 1) as u64;
                                                        self.perf_metrics.texture_transfer_bytes = rw * rh * 32;
                                                        gpu_synced = true;
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    self.brush_engine.settings.paint_color = orig_color;

                                    self.perf_metrics.paint_stroke_ms = t_stroke.elapsed().as_secs_f32() * 1000.0;

                                    self.brush_engine.settings.flow = orig_flow;
                                    self.brush_engine.settings.hardness = orig_hardness;

                                    if !gpu_synced {
                                        self.project.gpu_sync_needed = true;
                                    }
                                    self.project.is_dirty = true;
                                    self.last_painted_uv = Some(hit.uv);
                                    self.last_hovered_hit_3d = Some((hit.point, hit.normal, hit.uv));
                                    self.status_message = format!("3D Projection Painting at ({:.2}, {:.2}, {:.2})", hit.point.x, hit.point.y, hit.point.z);
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                    }
                }
            } else if response.drag_stopped() || (!input.pointer.any_down() && self.is_stroke_active) {
                if self.is_stroke_active {
                    let idx = self.project.active_layer_idx;
                    let desc = if self.brush_engine.settings.mode == ToolMode::FillBucket {
                        format!("Polygon Fill ({})", self.brush_engine.settings.polygon_fill_mode.name())
                    } else {
                        "Paint Stroke".to_string()
                    };

                    if let Some((x0, y0, x1, y1)) = self.gpu_stroke_dirty_rect.take() {
                        if let Some(wgpu) = &mut self.wgpu_viewport {
                            if let Some(layer) = self.project.layers.get_mut(idx) {
                                layer.record_tiles_for_rect(x0, y0, x1, y1);
                                wgpu.start_async_readback(idx, layer, x0, y0, x1, y1, self.brush_engine.settings.channels, &desc);
                            }
                        }
                    } else if let Some(layer) = self.project.layers.get_mut(idx) {
                        if let Some(action) = layer.finish_stroke_delta(idx, &desc) {
                            self.history.push(&desc, action);
                        }
                    }
                    self.is_stroke_active = false;
                }
                self.brush_engine.end_stroke();
                self.polygon_fill_drag_tris.clear();
                ctx.request_repaint();
            }

        // --- 3D SYMMETRY PLANE GIZMO ---
        let sym_x = self.brush_engine.settings.symmetry_x;
        let sym_y = self.brush_engine.settings.symmetry_y;
        let sym_z = self.brush_engine.settings.symmetry_z;
        if sym_x || sym_y || sym_z {
            let a_min = self.project.mesh.aabb_min;
            let a_max = self.project.mesh.aabb_max;
            let pad = (a_max - a_min) * 0.15;
            let min_p = a_min - pad;
            let max_p = a_max + pad;
            let c = self.brush_engine.settings.symmetry_center;

            let draw_plane = |corners: [glam::Vec3; 4], col: egui::Color32, stroke_col: egui::Color32| {
                let mut screen_corners = Vec::with_capacity(4);
                let mut all_valid = true;
                for corner in &corners {
                    if let Some(sp) = self.camera.world_to_screen(*corner, vp_w, vp_h) {
                        screen_corners.push(rect.min + egui::vec2(sp.x, sp.y));
                    } else {
                        all_valid = false;
                        break;
                    }
                }
                if all_valid && screen_corners.len() == 4 {
                    ui.painter().add(egui::Shape::convex_polygon(
                        screen_corners.clone(),
                        col,
                        egui::Stroke::new(1.2_f32, stroke_col),
                    ));
                    let mid0 = (screen_corners[0].to_vec2() + screen_corners[1].to_vec2()) * 0.5;
                    let mid1 = (screen_corners[2].to_vec2() + screen_corners[3].to_vec2()) * 0.5;
                    ui.painter().line_segment(
                        [egui::pos2(mid0.x, mid0.y), egui::pos2(mid1.x, mid1.y)],
                        egui::Stroke::new(0.8_f32, stroke_col),
                    );
                }
            };

            if sym_x {
                draw_plane(
                    [
                        glam::Vec3::new(c.x, min_p.y, min_p.z),
                        glam::Vec3::new(c.x, max_p.y, min_p.z),
                        glam::Vec3::new(c.x, max_p.y, max_p.z),
                        glam::Vec3::new(c.x, min_p.y, max_p.z),
                    ],
                    egui::Color32::from_rgba_unmultiplied(243, 94, 24, 25),
                    egui::Color32::from_rgba_unmultiplied(243, 94, 24, 180),
                );
            }
            if sym_y {
                draw_plane(
                    [
                        glam::Vec3::new(min_p.x, c.y, min_p.z),
                        glam::Vec3::new(max_p.x, c.y, min_p.z),
                        glam::Vec3::new(max_p.x, c.y, max_p.z),
                        glam::Vec3::new(min_p.x, c.y, max_p.z),
                    ],
                    egui::Color32::from_rgba_unmultiplied(46, 204, 113, 25),
                    egui::Color32::from_rgba_unmultiplied(46, 204, 113, 180),
                );
            }
            if sym_z {
                draw_plane(
                    [
                        glam::Vec3::new(min_p.x, min_p.y, c.z),
                        glam::Vec3::new(max_p.x, min_p.y, c.z),
                        glam::Vec3::new(max_p.x, max_p.y, c.z),
                        glam::Vec3::new(min_p.x, max_p.y, c.z),
                    ],
                    egui::Color32::from_rgba_unmultiplied(52, 152, 219, 25),
                    egui::Color32::from_rgba_unmultiplied(52, 152, 219, 180),
                );
            }
        }

        // --- STENCIL PROJECTION OVERLAY ---
        if self.brush_engine.settings.stencil.enabled {
            if self.stencil_overlay_texture.is_none() {
                if let Some(ref buf) = self.brush_engine.stencil_buffer {
                    let mut pixels = Vec::with_capacity((buf.width * buf.height) as usize);
                    for y in 0..buf.height {
                        for x in 0..buf.width {
                            let p = buf.get_pixel(x, y);
                            let r = (p.x.clamp(0.0, 1.0) * 255.0) as u8;
                            let g = (p.y.clamp(0.0, 1.0) * 255.0) as u8;
                            let b = (p.z.clamp(0.0, 1.0) * 255.0) as u8;
                            let a = (p.w.clamp(0.0, 1.0) * 255.0) as u8;
                            pixels.push(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
                        }
                    }
                    let img = egui::ColorImage {
                        size: [buf.width as usize, buf.height as usize],
                        pixels,
                    };
                    self.stencil_overlay_texture = Some(ctx.load_texture("stencil_preview_tex", img, egui::TextureOptions::LINEAR));
                }
            }

            let st = &self.brush_engine.settings.stencil;
            let center = rect.center() + egui::vec2(st.pos.x * rect.width(), st.pos.y * rect.height());
            let half_w = (rect.width() * 0.4 * st.scale) * 0.5;
            let half_h = (rect.height() * 0.4 * st.scale) * 0.5;
            let corners = [
                egui::pos2(-half_w, -half_h),
                egui::pos2(half_w, -half_h),
                egui::pos2(half_w, half_h),
                egui::pos2(-half_w, half_h),
            ];
            let (sin_a, cos_a) = st.angle.sin_cos();
            let rotated_corners: Vec<egui::Pos2> = corners
                .iter()
                .map(|p| {
                    let rx = p.x * cos_a - p.y * sin_a;
                    let ry = p.x * sin_a + p.y * cos_a;
                    center + egui::vec2(rx, ry)
                })
                .collect();
            let stencil_alpha = (st.opacity * 255.0).clamp(10.0, 255.0) as u8;

            // 1. Draw textured stencil image
            if let Some(ref tex) = self.stencil_overlay_texture {
                let mut mesh = egui::Mesh::with_texture(tex.id());
                let tint = egui::Color32::from_white_alpha(stencil_alpha);
                mesh.vertices.push(egui::epaint::Vertex { pos: rotated_corners[0], uv: egui::pos2(0.0, 0.0), color: tint });
                mesh.vertices.push(egui::epaint::Vertex { pos: rotated_corners[1], uv: egui::pos2(1.0, 0.0), color: tint });
                mesh.vertices.push(egui::epaint::Vertex { pos: rotated_corners[2], uv: egui::pos2(1.0, 1.0), color: tint });
                mesh.vertices.push(egui::epaint::Vertex { pos: rotated_corners[3], uv: egui::pos2(0.0, 1.0), color: tint });
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(mesh));
            } else {
                ui.painter().add(egui::Shape::convex_polygon(
                    rotated_corners.clone(),
                    egui::Color32::from_rgba_unmultiplied(243, 94, 24, (stencil_alpha / 6).max(12)),
                    egui::Stroke::NONE,
                ));
            }

            // 2. Draw border and corner grips
            let border_col = if input.key_down(egui::Key::S) {
                egui::Color32::from_rgb(255, 180, 50)
            } else {
                egui::Color32::from_rgba_unmultiplied(243, 94, 24, stencil_alpha.max(90))
            };
            ui.painter().add(egui::Shape::closed_line(
                rotated_corners.clone(),
                egui::Stroke::new(if input.key_down(egui::Key::S) { 2.4_f32 } else { 1.6_f32 }, border_col),
            ));
            for &pt in &rotated_corners {
                ui.painter().circle_filled(pt, 4.0, border_col);
            }

            // 3. Stencil HUD label or interactive shortcut guide
            if input.key_down(egui::Key::S) {
                ui.painter().text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    "[Hold S: LMB=Rotate | RMB=Scale | MMB=Pan | Z=Reset]",
                    egui::FontId::proportional(13.0),
                    egui::Color32::from_rgb(255, 230, 100),
                );
            } else {
                ui.painter().text(
                    rotated_corners[0] + egui::vec2(6.0, 6.0),
                    egui::Align2::LEFT_TOP,
                    format!("Stencil ({:.0}% opac, {:.0} deg) - Hold S to transform", st.opacity * 100.0, st.angle.to_degrees()),
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgba_unmultiplied(243, 94, 24, stencil_alpha.max(140)),
                );
            }
        }

        // --- 3D POLYGON FILL HOVER PREVIEW HIGHLIGHT ---
        if self.brush_engine.settings.mode == ToolMode::FillBucket {
            if let Some(tri_idx) = self.last_hovered_tri {
                let mode = self.brush_engine.settings.polygon_fill_mode;
                let preview_tris = match &self.poly_fill_hover_cache {
                    Some((c_tri, c_mode, c_tris)) if *c_tri == tri_idx && *c_mode == mode => c_tris.clone(),
                    _ => {
                        let tris = match mode {
                            crate::brush::PolygonFillMode::Triangle => vec![tri_idx],
                            crate::brush::PolygonFillMode::Polygon => self.project.mesh.get_polygon_quad(tri_idx),
                            crate::brush::PolygonFillMode::UvIsland => self.project.mesh.get_connected_uv_island(tri_idx),
                            crate::brush::PolygonFillMode::MeshElement => self.project.mesh.get_connected_mesh_element(tri_idx),
                        };
                        self.poly_fill_hover_cache = Some((tri_idx, mode, tris.clone()));
                        tris
                    }
                };

                let fill_col = egui::Color32::from_rgba_unmultiplied(243, 94, 24, 75);
                let cam_pos = self.camera.eye_position();

                    let mut fill_mesh = egui::Mesh::default();
                    for &t in &preview_tris {
                        if t * 3 + 2 >= self.project.mesh.indices.len() { continue; }
                        let i0 = self.project.mesh.indices[t * 3] as usize;
                        let i1 = self.project.mesh.indices[t * 3 + 1] as usize;
                        let i2 = self.project.mesh.indices[t * 3 + 2] as usize;
                        if i0 >= self.project.mesh.vertices.len() || i1 >= self.project.mesh.vertices.len() || i2 >= self.project.mesh.vertices.len() { continue; }
                        let p0_3d = self.project.mesh.vertices[i0].position;
                        let p1_3d = self.project.mesh.vertices[i1].position;
                        let p2_3d = self.project.mesh.vertices[i2].position;

                        // Backface cull preview against camera
                        let fnorm = (p1_3d - p0_3d).cross(p2_3d - p0_3d);
                        let view_vec = cam_pos - (p0_3d + p1_3d + p2_3d) * (1.0 / 3.0);
                        if fnorm.dot(view_vec) <= 0.0 { continue; }

                        let sp0 = self.camera.world_to_screen(p0_3d, vp_w, vp_h);
                        let sp1 = self.camera.world_to_screen(p1_3d, vp_w, vp_h);
                        let sp2 = self.camera.world_to_screen(p2_3d, vp_w, vp_h);

                        if let (Some(s0), Some(s1), Some(s2)) = (sp0, sp1, sp2) {
                            let base = fill_mesh.vertices.len() as u32;
                            fill_mesh.vertices.push(egui::epaint::Vertex { pos: rect.min + egui::vec2(s0.x, s0.y), uv: egui::pos2(0.0, 0.0), color: fill_col });
                            fill_mesh.vertices.push(egui::epaint::Vertex { pos: rect.min + egui::vec2(s1.x, s1.y), uv: egui::pos2(0.0, 0.0), color: fill_col });
                            fill_mesh.vertices.push(egui::epaint::Vertex { pos: rect.min + egui::vec2(s2.x, s2.y), uv: egui::pos2(0.0, 0.0), color: fill_col });
                            fill_mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
                        }
                    }
                    if !fill_mesh.is_empty() {
                        ui.painter().add(egui::Shape::mesh(fill_mesh));
                    }
                }
            }

        // --- 3D VECTOR PATH TOOL SPLINE & CONTROL POINTS OVERLAY ---
        if self.brush_engine.settings.mode == ToolMode::Path || !self.brush_engine.settings.path_tool.points.is_empty() {
            let doc_w = self.project.texture_width;
            let world_rad = ((self.brush_engine.settings.radius / (doc_w as f32).max(1.0)) * 2.0).clamp(0.005, 0.5);

            // 1. Interactive guide line from last point to cursor when hovering
            if self.brush_engine.settings.mode == ToolMode::Path {
                if let Some(mouse_pos) = input.pointer.hover_pos() {
                    if let Some(last_pt) = self.brush_engine.settings.path_tool.points.last() {
                        if !self.brush_engine.settings.path_tool.is_dragging {
                            let local_x = mouse_pos.x - rect.min.x;
                            let local_y = mouse_pos.y - rect.min.y;
                            let ray = self.camera.screen_to_ray(local_x, local_y, vp_w, vp_h);
                            if let Some(hit) = self.project.mesh.raycast(&ray) {
                                if let (Some(s_last), Some(s_cur)) = (
                                    self.camera.world_to_screen(last_pt.position_3d, vp_w, vp_h),
                                    self.camera.world_to_screen(hit.point, vp_w, vp_h),
                                ) {
                                    let p_last = rect.min + egui::vec2(s_last.x, s_last.y);
                                    let p_cur = rect.min + egui::vec2(s_cur.x, s_cur.y);
                                    ui.painter().line_segment(
                                        [p_last, p_cur],
                                        egui::Stroke::new(1.8_f32, egui::Color32::from_rgba_unmultiplied(255, 180, 50, 200)),
                                    );
                                }
                            }
                        }
                    }
                }
            }

            // 2. Draw high-resolution continuous Catmull-Rom spline curve line with drop shadow (batched polyline)
            let mut curve_points = self.brush_engine.settings.path_tool.evaluate_spline_curve(32);
            if self.brush_engine.settings.path_tool.snap_to_surface {
                crate::brush::PathToolState::project_samples_to_mesh(
                    &mut curve_points,
                    &self.project.mesh,
                    world_rad,
                    self.brush_engine.settings.path_tool.surface_offset,
                );
            }
            let screen_pts: Vec<egui::Pos2> = curve_points.iter()
                .filter_map(|pt| self.camera.world_to_screen(pt.position_3d, vp_w, vp_h).map(|s| rect.min + egui::vec2(s.x, s.y)))
                .collect();
            if screen_pts.len() >= 2 {
                ui.painter().add(egui::Shape::line(
                    screen_pts.clone(),
                    egui::Stroke::new(4.5_f32, egui::Color32::from_rgba_unmultiplied(20, 22, 28, 220)),
                ));
                ui.painter().add(egui::Shape::line(
                    screen_pts,
                    egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(255, 115, 30)),
                ));
            }

            // 3. Draw realistic 3D live preview for decals, weld beads, and path stroke styles
            let mut samples = self.brush_engine.settings.path_tool.sample_path(world_rad);
            if self.brush_engine.settings.path_tool.snap_to_surface {
                crate::brush::PathToolState::project_samples_to_mesh(
                    &mut samples,
                    &self.project.mesh,
                    world_rad,
                    self.brush_engine.settings.path_tool.surface_offset,
                );
            }

            let light_dir = glam::Vec3::new(0.4, 0.7, 0.6).normalize();
            let p_col = self.brush_engine.settings.paint_color;
            let base_rgb = egui::Color32::from_rgb(
                (p_col.x * 255.0).clamp(0.0, 255.0) as u8,
                (p_col.y * 255.0).clamp(0.0, 255.0) as u8,
                (p_col.z * 255.0).clamp(0.0, 255.0) as u8,
            );

            let stroke_style = self.brush_engine.settings.path_tool.stroke_style;
            let decal_type = self.brush_engine.settings.path_tool.decal_type;
            let heat_tint = self.brush_engine.settings.path_tool.heat_tint;

            for sample in &samples {
                let norm = sample.normal.normalize_or_zero();
                let norm = if norm.length_squared() > 1e-4 { norm } else { glam::Vec3::Y };

                // Tangent frame on the 3D surface
                let up = if norm.y.abs() < 0.9 { glam::Vec3::Y } else { glam::Vec3::X };
                let initial_tan = norm.cross(up).normalize();
                let initial_bitan = norm.cross(initial_tan).normalize();

                let cos_a = sample.tangent_angle.cos();
                let sin_a = sample.tangent_angle.sin();
                let tan = (initial_tan * cos_a + initial_bitan * sin_a).normalize();
                let bitan = norm.cross(tan).normalize();

                let r_eff = world_rad * sample.radius_scale;
                let n_dot_l = norm.dot(light_dir).max(0.2);

                let shade_col = |factor: f32, alpha: u8| -> egui::Color32 {
                    let r = ((base_rgb.r() as f32 * n_dot_l * factor).clamp(0.0, 255.0)) as u8;
                    let g = ((base_rgb.g() as f32 * n_dot_l * factor).clamp(0.0, 255.0)) as u8;
                    let b = ((base_rgb.b() as f32 * n_dot_l * factor).clamp(0.0, 255.0)) as u8;
                    egui::Color32::from_rgba_unmultiplied(r, g, b, alpha)
                };

                match stroke_style {
                    crate::brush::PathStrokeStyle::SmartDecal => {
                        match decal_type {
                            crate::brush::SmartDecalType::HexBolt => {
                                let mut hex_pts = Vec::with_capacity(6);
                                for i in 0..6 {
                                    let theta = (i as f32) * std::f32::consts::PI / 3.0 + std::f32::consts::PI / 6.0;
                                    let p3d = sample.position_3d + (tan * theta.cos() + bitan * theta.sin()) * r_eff;
                                    if let Some(sp) = self.camera.world_to_screen(p3d, vp_w, vp_h) {
                                        hex_pts.push(rect.min + egui::vec2(sp.x, sp.y));
                                    }
                                }
                                if hex_pts.len() == 6 {
                                    ui.painter().add(egui::Shape::convex_polygon(
                                        hex_pts,
                                        shade_col(1.0, 240),
                                        egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(20, 20, 25, 220)),
                                    ));
                                }
                                if let Some(sp) = self.camera.world_to_screen(sample.position_3d, vp_w, vp_h) {
                                    let c = rect.min + egui::vec2(sp.x, sp.y);
                                    let r_screen = ((r_eff / world_rad) * 6.0).clamp(2.5, 14.0);
                                    ui.painter().circle_filled(c, r_screen * 0.45, egui::Color32::from_rgba_unmultiplied(25, 27, 35, 220));
                                    ui.painter().circle_stroke(c, r_screen * 0.45, egui::Stroke::new(0.8_f32, shade_col(1.4, 200)));
                                }
                            }
                            crate::brush::SmartDecalType::PhillipsScrew => {
                                if let Some(sp) = self.camera.world_to_screen(sample.position_3d, vp_w, vp_h) {
                                    let c = rect.min + egui::vec2(sp.x, sp.y);
                                    let r_screen = ((r_eff / world_rad) * 7.0).clamp(3.0, 16.0);
                                    ui.painter().circle_filled(c, r_screen, shade_col(0.9, 230));
                                    ui.painter().circle_stroke(c, r_screen, egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(15, 15, 20, 200)));
                                    let slot_len = r_screen * 0.7;
                                    let slot_tan = if let Some(p_t) = self.camera.world_to_screen(sample.position_3d + tan * (r_eff * 0.7), vp_w, vp_h) {
                                        ((rect.min + egui::vec2(p_t.x, p_t.y)) - c).normalized()
                                    } else {
                                        egui::vec2(1.0, 0.0)
                                    };
                                    let slot_bitan = egui::vec2(-slot_tan.y, slot_tan.x);
                                    ui.painter().line_segment([c - slot_tan * slot_len, c + slot_tan * slot_len], egui::Stroke::new(2.0_f32, egui::Color32::from_rgba_unmultiplied(20, 22, 28, 240)));
                                    ui.painter().line_segment([c - slot_bitan * slot_len, c + slot_bitan * slot_len], egui::Stroke::new(2.0_f32, egui::Color32::from_rgba_unmultiplied(20, 22, 28, 240)));
                                }
                            }
                            crate::brush::SmartDecalType::FlatheadScrew => {
                                if let Some(sp) = self.camera.world_to_screen(sample.position_3d, vp_w, vp_h) {
                                    let c = rect.min + egui::vec2(sp.x, sp.y);
                                    let r_screen = ((r_eff / world_rad) * 7.0).clamp(3.0, 16.0);
                                    ui.painter().circle_filled(c, r_screen, shade_col(0.9, 230));
                                    ui.painter().circle_stroke(c, r_screen, egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(15, 15, 20, 200)));
                                    let slot_len = r_screen * 0.75;
                                    let slot_tan = if let Some(p_t) = self.camera.world_to_screen(sample.position_3d + tan * (r_eff * 0.75), vp_w, vp_h) {
                                        ((rect.min + egui::vec2(p_t.x, p_t.y)) - c).normalized()
                                    } else {
                                        egui::vec2(1.0, 0.0)
                                    };
                                    ui.painter().line_segment([c - slot_tan * slot_len, c + slot_tan * slot_len], egui::Stroke::new(2.2_f32, egui::Color32::from_rgba_unmultiplied(20, 22, 28, 240)));
                                }
                            }
                            crate::brush::SmartDecalType::TorxScrew => {
                                if let Some(sp) = self.camera.world_to_screen(sample.position_3d, vp_w, vp_h) {
                                    let c = rect.min + egui::vec2(sp.x, sp.y);
                                    let r_screen = ((r_eff / world_rad) * 7.0).clamp(3.0, 16.0);
                                    ui.painter().circle_filled(c, r_screen, shade_col(0.95, 230));
                                    ui.painter().circle_stroke(c, r_screen, egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(15, 15, 20, 200)));
                                    let mut star_pts = Vec::with_capacity(12);
                                    for k in 0..12 {
                                        let ang = (k as f32) * std::f32::consts::PI / 6.0;
                                        let rad = if k % 2 == 0 { r_screen * 0.55 } else { r_screen * 0.32 };
                                        let p3d = sample.position_3d + (tan * ang.cos() + bitan * ang.sin()) * (r_eff * (rad / r_screen));
                                        if let Some(st) = self.camera.world_to_screen(p3d, vp_w, vp_h) {
                                            star_pts.push(rect.min + egui::vec2(st.x, st.y));
                                        }
                                    }
                                    if star_pts.len() == 12 {
                                        ui.painter().add(egui::Shape::convex_polygon(
                                            star_pts,
                                            egui::Color32::from_rgba_unmultiplied(22, 24, 30, 240),
                                            egui::Stroke::new(0.8_f32, shade_col(1.2, 180)),
                                        ));
                                    }
                                }
                            }
                            crate::brush::SmartDecalType::DomedRivet => {
                                if let Some(sp) = self.camera.world_to_screen(sample.position_3d, vp_w, vp_h) {
                                    let c = rect.min + egui::vec2(sp.x, sp.y);
                                    let r_screen = ((r_eff / world_rad) * 6.5).clamp(2.5, 15.0);
                                    ui.painter().circle_filled(c, r_screen, egui::Color32::from_rgba_unmultiplied(25, 25, 30, 160));
                                    ui.painter().circle_filled(c, r_screen * 0.88, shade_col(0.9, 230));
                                    let hl_pos = c - egui::vec2(r_screen * 0.25, r_screen * 0.25);
                                    ui.painter().circle_filled(hl_pos, r_screen * 0.32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180));
                                }
                            }
                            crate::brush::SmartDecalType::ButtonHole | crate::brush::SmartDecalType::GrommetEyelet => {
                                if let Some(sp) = self.camera.world_to_screen(sample.position_3d, vp_w, vp_h) {
                                    let c = rect.min + egui::vec2(sp.x, sp.y);
                                    let r_screen = ((r_eff / world_rad) * 6.5).clamp(2.5, 15.0);
                                    ui.painter().circle_stroke(c, r_screen, egui::Stroke::new(2.2_f32, shade_col(1.1, 230)));
                                    ui.painter().circle_filled(c, r_screen * 0.55, egui::Color32::from_rgba_unmultiplied(10, 10, 14, 255));
                                }
                            }
                            crate::brush::SmartDecalType::AirVentLouver => {
                                let p0 = sample.position_3d - tan * (r_eff * 0.8) - bitan * (r_eff * 0.5);
                                let p1 = sample.position_3d + tan * (r_eff * 0.8) - bitan * (r_eff * 0.5);
                                let p2 = sample.position_3d + tan * (r_eff * 0.8) + bitan * (r_eff * 0.5);
                                let p3 = sample.position_3d - tan * (r_eff * 0.8) + bitan * (r_eff * 0.5);
                                if let (Some(s0), Some(s1), Some(s2), Some(s3)) = (
                                    self.camera.world_to_screen(p0, vp_w, vp_h),
                                    self.camera.world_to_screen(p1, vp_w, vp_h),
                                    self.camera.world_to_screen(p2, vp_w, vp_h),
                                    self.camera.world_to_screen(p3, vp_w, vp_h),
                                ) {
                                    let pts = vec![
                                        rect.min + egui::vec2(s0.x, s0.y),
                                        rect.min + egui::vec2(s1.x, s1.y),
                                        rect.min + egui::vec2(s2.x, s2.y),
                                        rect.min + egui::vec2(s3.x, s3.y),
                                    ];
                                    ui.painter().add(egui::Shape::convex_polygon(
                                        pts,
                                        shade_col(0.7, 210),
                                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(20, 20, 25, 230)),
                                    ));
                                }
                            }
                        }
                    }
                    crate::brush::PathStrokeStyle::WeldSeam => {
                        if let Some(sp) = self.camera.world_to_screen(sample.position_3d, vp_w, vp_h) {
                            let c = rect.min + egui::vec2(sp.x, sp.y);
                            let r_screen = ((r_eff / world_rad) * 6.0).clamp(2.5, 14.0);
                            if heat_tint {
                                ui.painter().circle_filled(c, r_screen * 1.55, egui::Color32::from_rgba_unmultiplied(75, 85, 175, 70));
                                ui.painter().circle_filled(c, r_screen * 1.25, egui::Color32::from_rgba_unmultiplied(215, 165, 55, 120));
                            }
                            ui.painter().circle_filled(c, r_screen, shade_col(1.15, 235));
                            ui.painter().circle_stroke(c, r_screen, egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(35, 38, 48, 190)));
                            ui.painter().circle_stroke(c, r_screen * 0.45, egui::Stroke::new(0.9_f32, shade_col(1.4, 210)));
                        }
                    }
                    crate::brush::PathStrokeStyle::RopeBraid => {
                        let r_screen = ((r_eff / world_rad) * 5.5).clamp(2.2, 12.0);
                        let strand_alt = if (((sample.arc_length / (r_eff * 1.5).max(0.01)) as i32) % 2) == 0 { 1.0 } else { -1.0 };
                        let p_strand = sample.position_3d + bitan * (r_eff * 0.35 * strand_alt);
                        if let Some(sp2) = self.camera.world_to_screen(p_strand, vp_w, vp_h) {
                            let c2 = rect.min + egui::vec2(sp2.x, sp2.y);
                            ui.painter().circle_filled(c2, r_screen * 0.85, shade_col(1.05, 230));
                            ui.painter().circle_stroke(c2, r_screen * 0.85, egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(25, 28, 35, 200)));
                        }
                    }
                    crate::brush::PathStrokeStyle::StitchSeam => {
                        let p_l = sample.position_3d - bitan * (r_eff * 0.6) + tan * (r_eff * 0.2);
                        let p_r = sample.position_3d + bitan * (r_eff * 0.6) - tan * (r_eff * 0.2);
                        if let (Some(s_l), Some(s_r)) = (
                            self.camera.world_to_screen(p_l, vp_w, vp_h),
                            self.camera.world_to_screen(p_r, vp_w, vp_h),
                        ) {
                            let pt_l = rect.min + egui::vec2(s_l.x, s_l.y);
                            let pt_r = rect.min + egui::vec2(s_r.x, s_r.y);
                            ui.painter().circle_filled(pt_l, 2.0, egui::Color32::from_rgba_unmultiplied(15, 15, 20, 240));
                            ui.painter().circle_filled(pt_r, 2.0, egui::Color32::from_rgba_unmultiplied(15, 15, 20, 240));
                            ui.painter().line_segment([pt_l, pt_r], egui::Stroke::new(2.8_f32, shade_col(1.3, 250)));
                        }
                    }
                    crate::brush::PathStrokeStyle::ChainLinks => {
                        if let Some(sp) = self.camera.world_to_screen(sample.position_3d, vp_w, vp_h) {
                            let c = rect.min + egui::vec2(sp.x, sp.y);
                            let r_screen = ((r_eff / world_rad) * 6.0).clamp(2.5, 14.0);
                            let is_flat = (((sample.arc_length / (r_eff * 2.0).max(0.01)) as i32) % 2) == 0;
                            if is_flat {
                                ui.painter().circle_stroke(c, r_screen, egui::Stroke::new(2.4_f32, shade_col(1.2, 240)));
                                ui.painter().circle_stroke(c, r_screen + 1.2, egui::Stroke::new(0.8_f32, egui::Color32::from_rgba_unmultiplied(20, 20, 25, 180)));
                            } else {
                                ui.painter().circle_filled(c, r_screen * 0.65, shade_col(0.8, 240));
                                ui.painter().circle_stroke(c, r_screen * 0.65, egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(20, 20, 25, 200)));
                            }
                        }
                    }
                    crate::brush::PathStrokeStyle::Continuous => {
                        if let Some(sp) = self.camera.world_to_screen(sample.position_3d, vp_w, vp_h) {
                            let c = rect.min + egui::vec2(sp.x, sp.y);
                            let r_screen = ((r_eff / world_rad) * 5.0).clamp(2.0, 12.0);
                            ui.painter().circle_stroke(c, r_screen + 1.0, egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(20, 20, 25, 180)));
                            ui.painter().circle_filled(c, r_screen, shade_col(1.0, 220));
                        }
                    }
                }
            }

            // 4. Draw control point handle rings with index numbers
            let sel_idx = self.brush_engine.settings.path_tool.selected_point;
            for (i, pt) in self.brush_engine.settings.path_tool.points.iter().enumerate() {
                if let Some(s_pos) = self.camera.world_to_screen(pt.position_3d, vp_w, vp_h) {
                    let pt_screen = rect.min + egui::vec2(s_pos.x, s_pos.y);
                    let is_selected = sel_idx == Some(i);

                    if is_selected {
                        ui.painter().circle_stroke(pt_screen, 9.0, egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(255, 230, 60)));
                        ui.painter().circle_filled(pt_screen, 5.0, egui::Color32::from_rgb(243, 94, 24));
                    } else {
                        ui.painter().circle_stroke(pt_screen, 7.5, egui::Stroke::new(2.0_f32, egui::Color32::WHITE));
                        ui.painter().circle_filled(pt_screen, 4.0, egui::Color32::from_rgb(30, 32, 40));
                    }

                    // Index label badge
                    let badge_pos = pt_screen + egui::vec2(12.0, -12.0);
                    ui.painter().text(
                        badge_pos,
                        egui::Align2::LEFT_CENTER,
                        format!("{}", i + 1),
                        egui::FontId::monospace(11.5),
                        if is_selected { egui::Color32::from_rgb(255, 230, 60) } else { egui::Color32::WHITE },
                    );
                }
            }
        }

        // --- 3D DECAL STAMP TOOL HOVER RETICLE ---
        if self.brush_engine.settings.mode == ToolMode::Stamp {
            if let Some(mouse_pos) = input.pointer.hover_pos() {
                if rect.contains(mouse_pos) {
                    let local_x = mouse_pos.x - rect.min.x;
                    let local_y = mouse_pos.y - rect.min.y;
                    let ray = self.camera.screen_to_ray(local_x, local_y, vp_w, vp_h);
                    if let Some(_hit) = self.project.mesh.raycast(&ray) {
                        let rad = self.brush_engine.settings.radius;
                        let rot_deg = self.brush_engine.settings.stamp_tool.rotation_angle;
                        let rot_rad = rot_deg.to_radians();

                        ui.painter().circle_stroke(
                            mouse_pos,
                            rad,
                            egui::Stroke::new(1.8_f32, egui::Color32::from_rgb(255, 175, 40)),
                        );
                        let dir = egui::vec2(rot_rad.cos(), -rot_rad.sin());
                        let pointer_tip = mouse_pos + dir * rad;
                        ui.painter().line_segment(
                            [mouse_pos, pointer_tip],
                            egui::Stroke::new(2.2_f32, egui::Color32::from_rgb(255, 230, 80)),
                        );
                        let left_wing = mouse_pos + dir * (rad - 8.0).max(4.0) + egui::vec2(-dir.y, dir.x) * 5.0;
                        let right_wing = mouse_pos + dir * (rad - 8.0).max(4.0) - egui::vec2(-dir.y, dir.x) * 5.0;
                        ui.painter().line_segment([pointer_tip, left_wing], egui::Stroke::new(1.8_f32, egui::Color32::from_rgb(255, 230, 80)));
                        ui.painter().line_segment([pointer_tip, right_wing], egui::Stroke::new(1.8_f32, egui::Color32::from_rgb(255, 230, 80)));
                    }
                }
            }
        }

        // --- INTERACTIVE BRUSH CURSOR, LAZY LEASH & 3D SYMMETRY GHOST CURSORS ---
        if response.hovered() && !input.modifiers.alt {
            if pointer_moved || self.continuous_60fps {
                ctx.request_repaint();
            }
            if let Some(mouse_pos) = input.pointer.hover_pos() {
                let brush_rad = self.brush_engine.settings.radius;

                let c_pos = if self.brush_engine.settings.lazy_mouse_enabled {
                    self.brush_engine.lazy_screen_pos.map(|p| egui::pos2(p[0], p[1])).unwrap_or(mouse_pos)
                } else {
                    mouse_pos
                };
                let local_x = c_pos.x - rect.min.x;
                let local_y = c_pos.y - rect.min.y;
                let ray = self.camera.screen_to_ray(local_x, local_y, vp_w, vp_h);
                let hit_opt = self.project.mesh.raycast(&ray);
                if let Some(hit) = &hit_opt {
                    self.last_hovered_hit_3d = Some((hit.point, hit.normal, hit.uv));
                    self.last_hovered_tri = Some(hit.triangle_idx);
                } else {
                    self.last_hovered_hit_3d = None;
                    if self.last_hovered_uv_2d.is_none() {
                        self.last_hovered_tri = None;
                    }
                }

                if self.brush_engine.settings.mode == ToolMode::Path {
                    // Precision crosshair and node placement cursor for Path Tool
                    ui.painter().circle_stroke(mouse_pos, 7.0, egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24)));
                    ui.painter().circle_filled(mouse_pos, 2.0, egui::Color32::WHITE);
                    ui.painter().line_segment([mouse_pos + egui::vec2(-10.0, 0.0), mouse_pos + egui::vec2(10.0, 0.0)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(243, 94, 24)));
                    ui.painter().line_segment([mouse_pos + egui::vec2(0.0, -10.0), mouse_pos + egui::vec2(0.0, 10.0)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(243, 94, 24)));
                } else if self.brush_engine.settings.mode == ToolMode::FillBucket {
                    // Precision cursor dot and ring for Polygon Fill
                    ui.painter().circle_filled(mouse_pos, 3.0, egui::Color32::from_rgb(243, 94, 24));
                    ui.painter().circle_stroke(mouse_pos, 6.0, egui::Stroke::new(1.2_f32, egui::Color32::WHITE));
                } else if self.brush_engine.settings.lazy_mouse_enabled {
                    if let Some(l_pos) = self.brush_engine.lazy_screen_pos {
                        let stabilized_pos = egui::pos2(l_pos[0], l_pos[1]);
                        // Tether line from stabilized point to mouse
                        ui.painter().line_segment(
                            [stabilized_pos, mouse_pos],
                            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24)),
                        );
                        // Stabilized brush ring
                        ui.painter().circle_stroke(
                            stabilized_pos,
                            brush_rad,
                            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24)),
                        );
                        ui.painter().circle_filled(stabilized_pos, 2.0, egui::Color32::from_rgb(243, 94, 24));
                        // Mouse position dot
                        ui.painter().circle_filled(mouse_pos, 3.0, egui::Color32::WHITE);
                    } else {
                        ui.painter().circle_stroke(
                            mouse_pos,
                            brush_rad,
                            egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210)),
                        );
                    }
                } else {
                    // Primary cursor ring (White)
                    ui.painter().circle_stroke(
                        mouse_pos,
                        brush_rad,
                        egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210)),
                    );
                    ui.painter().circle_filled(
                        mouse_pos,
                        1.5,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 240),
                    );
                }

                // Mirrored ghost cursors (Electric Blue)
                if sym_x || sym_y || sym_z {
                    if let Some(hit) = &hit_opt {
                        let sym_points = self.brush_engine.get_symmetry_points(hit.point, hit.normal);
                        for (sym_p, _) in sym_points.iter().skip(1) {
                            if let Some(screen_p) = self.camera.world_to_screen(*sym_p, vp_w, vp_h) {
                                let sym_screen_pos = rect.min + egui::vec2(screen_p.x, screen_p.y);
                                ui.painter().circle_stroke(
                                    sym_screen_pos,
                                    brush_rad,
                                    egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(60, 180, 255, 220)),
                                );
                                ui.painter().circle_filled(
                                    sym_screen_pos,
                                    1.5,
                                    egui::Color32::from_rgba_unmultiplied(60, 180, 255, 255),
                                );
                            }
                        }
                    }
                }
            }
        } else {
            self.last_hovered_hit_3d = None;
            if self.last_hovered_uv_2d.is_none() {
                self.last_hovered_tri = None;
            }
        }

        // Synchronized 2D-to-3D cross-viewport cursor indicator
        if !response.hovered() {
            if let Some(uv) = self.last_hovered_uv_2d {
                if let Some((pos_3d, norm_3d)) = self.project.mesh.uv_to_3d_point(uv) {
                    if let Some(screen_p) = self.camera.world_to_screen(pos_3d, vp_w, vp_h) {
                        let p = rect.min + egui::vec2(screen_p.x, screen_p.y);
                        if rect.contains(p) {
                            let dist = (self.camera.eye_position() - pos_3d).length().max(0.01);
                            let fov_factor = (self.camera.fov_y / 2.0).tan();
                            let world_rad = (self.brush_engine.settings.radius / (self.project.texture_width as f32)) * self.project.mesh.bounds_radius();
                            let proj_screen_rad = ((world_rad / (dist * fov_factor)) * vp_h * 0.5).clamp(4.0, 300.0);

                            ui.painter().circle_stroke(
                                p,
                                proj_screen_rad,
                                egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0, 220, 255)),
                            );
                            ui.painter().circle_filled(
                                p,
                                2.0,
                                egui::Color32::from_rgb(0, 220, 255),
                            );

                            // Surface normal tick indicator
                            if let Some(norm_end) = self.camera.world_to_screen(pos_3d + norm_3d * (world_rad * 0.4).max(0.02), vp_w, vp_h) {
                                let norm_p = rect.min + egui::vec2(norm_end.x, norm_end.y);
                                ui.painter().line_segment([p, norm_p], egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(0, 220, 255, 180)));
                            }
                        }
                    }
                }
            }
        }

        // Draw in-viewport gesture HUD overlay if active
        if let Some((pos, label, opt_rad, opt_hard)) = gesture_overlay {
            if let Some(rad) = opt_rad {
                // Brush radius outer ring (Substance iconic amber)
                ui.painter().circle_stroke(
                    pos,
                    rad,
                    egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(243, 94, 24)),
                );
                if let Some(hard) = opt_hard {
                    // Inner hardness boundary ring
                    ui.painter().circle_stroke(
                        pos,
                        (rad * hard).max(1.0),
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
                    );
                }
            }
            // Floating gesture HUD pill
            let pill_offset = egui::vec2(0.0, -opt_rad.unwrap_or(24.0) - 26.0);
            let pill_center = pos + pill_offset;
            let pill_rect = egui::Rect::from_center_size(pill_center, egui::vec2(220.0, 26.0));
            ui.painter().rect_filled(
                pill_rect,
                egui::Rounding::same(13.0),
                egui::Color32::from_rgba_unmultiplied(20, 22, 28, 235),
            );
            ui.painter().rect_stroke(
                pill_rect,
                egui::Rounding::same(13.0),
                egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(243, 94, 24)),
            );
            ui.painter().text(
                pill_center,
                egui::Align2::CENTER_CENTER,
                &label,
                egui::FontId::proportional(11.5),
                egui::Color32::WHITE,
            );
        }

        // Draw Mask Preview HUD banner if active
        if let Some(preview_id) = self.mask_preview_layer_id {
            let layer_name = self.project.layers.iter().find(|l| l.id == preview_id).map(|l| l.name.as_str()).unwrap_or("Mask");
            let banner_w = 340.0;
            let banner_h = 28.0;
            let banner_rect = egui::Rect::from_min_size(
                rect.left_top() + egui::vec2(12.0, 12.0),
                egui::vec2(banner_w, banner_h),
            );
            ui.painter().rect_filled(
                banner_rect,
                egui::Rounding::same(5.0),
                egui::Color32::from_rgba_unmultiplied(22, 24, 30, 230),
            );
            ui.painter().rect_stroke(
                banner_rect,
                egui::Rounding::same(5.0),
                egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0, 210, 255)),
            );
            let banner_text = format!("Mask Preview (Unlit): {} [Alt+Click mask or M/Esc to exit]", layer_name);
            ui.painter().text(
                banner_rect.center(),
                egui::Align2::CENTER_CENTER,
                banner_text,
                egui::FontId::proportional(11.5),
                egui::Color32::from_rgb(240, 248, 255),
            );
        }
    }


}
