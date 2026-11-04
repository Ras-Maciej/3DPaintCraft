use super::*;
use eframe::egui::{self, Color32};

impl PaintcraftApp {
    pub(crate) fn render_2d_uv_viewport_ui(&mut self, ui: &mut egui::Ui, size: Vec2, ctx: &egui::Context) {
        if self.project.is_dirty || self.uv_canvas_texture.is_none() {
            self.project.composite_textures.composite_layers(&self.project.layers);
            self.update_uv_texture(ctx);
            self.project.is_dirty = false;
        }

        if let Some(tex) = &self.uv_canvas_texture {
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

            // Dark neutral viewport background
            ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(24, 25, 29));

            // Calculate canvas aspect ratio preserving dimensions
            let tex_w = self.project.texture_width as f32;
            let tex_h = self.project.texture_height as f32;
            let aspect = (tex_w / tex_h).max(0.01);
            let margin = 20.0;
            let avail_w = (rect.width() - margin * 2.0).max(32.0);
            let avail_h = (rect.height() - margin * 2.0).max(32.0);
            let (fit_w, fit_h) = if avail_w / avail_h > aspect {
                (avail_h * aspect, avail_h)
            } else {
                (avail_w, avail_w / aspect)
            };

            let canvas_w = fit_w * self.uv_2d_zoom;
            let canvas_h = fit_h * self.uv_2d_zoom;
            let canvas_center = rect.center() + self.uv_2d_pan;
            let canvas_rect = egui::Rect::from_center_size(canvas_center, egui::vec2(canvas_w, canvas_h));

            // Pan and navigation controls (Alt + drag, Middle-drag, Space + drag)
            let is_panning = response.dragged_by(egui::PointerButton::Middle)
                || (response.dragged_by(egui::PointerButton::Primary) && ui.input(|i| i.modifiers.alt || i.key_down(egui::Key::Space)))
                || (response.dragged_by(egui::PointerButton::Secondary) && ui.input(|i| i.modifiers.alt));

            if is_panning {
                let delta = ui.input(|i| i.pointer.delta());
                self.uv_2d_pan += delta;
                ctx.request_repaint();
            }

            // Mouse wheel zoom towards mouse position
            if response.hovered() {
                let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
                if scroll_y.abs() > 0.1 {
                    let zoom_factor = if scroll_y > 0.0 { 1.12 } else { 1.0 / 1.12 };
                    let new_zoom = (self.uv_2d_zoom * zoom_factor).clamp(0.2, 16.0);
                    if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        let rel = mouse_pos - canvas_center;
                        let scale_ratio = new_zoom / self.uv_2d_zoom;
                        self.uv_2d_pan = mouse_pos - rect.center() - rel * scale_ratio;
                        self.uv_2d_zoom = new_zoom;
                    } else {
                        self.uv_2d_zoom = new_zoom;
                    }
                }
            }

            // Hotkey F: Fit canvas to view
            if response.hovered() && ui.input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.command && !i.modifiers.ctrl) {
                self.uv_2d_pan = egui::Vec2::ZERO;
                self.uv_2d_zoom = 1.0;
                self.status_message = "2D Viewport reset to Fit [F]".to_string();
            }

            // Canvas drop shadow and texture rendering
            let shadow_rect = canvas_rect.expand(2.0);
            ui.painter().rect_filled(shadow_rect, egui::Rounding::same(2.0), Color32::from_black_alpha(100));
            ui.painter().rect_filled(canvas_rect, 0.0, Color32::from_rgb(18, 18, 20));

            ui.painter().image(
                tex.id(),
                canvas_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );

            // UDIM Multi-Tile Grid & Label Badges
            let udim_tiles = self.project.mesh.get_udim_tiles();
            let cw = canvas_rect.width();
            let ch = canvas_rect.height();
            let cmin = canvas_rect.min;

            for &tile_id in &udim_tiles {
                let u_idx = (tile_id - 1001) % 10;
                let v_idx = (tile_id - 1001) / 10;
                let tile_min = cmin + egui::vec2(u_idx as f32 * cw, v_idx as f32 * ch);
                let tile_rect = egui::Rect::from_min_size(tile_min, egui::vec2(cw, ch));

                let border_col = if tile_id == 1001 {
                    Color32::from_rgb(70, 75, 88)
                } else {
                    Color32::from_rgb(55, 60, 72)
                };
                ui.painter().rect_stroke(tile_rect, 0.0, egui::Stroke::new(1.0_f32, border_col));

                let badge_rect = egui::Rect::from_min_size(tile_min + egui::vec2(6.0, 6.0), egui::vec2(78.0, 18.0));
                ui.painter().rect_filled(badge_rect, egui::Rounding::same(3.0), Color32::from_rgba_unmultiplied(20, 22, 26, 200));
                ui.painter().text(
                    badge_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("UDIM {}", tile_id),
                    egui::FontId::monospace(11.0),
                    if tile_id == 1001 { Color32::from_rgb(240, 140, 40) } else { Color32::from_rgb(160, 175, 195) },
                );
            }

            // UV Wireframe Overlay (Auto-enabled when in Polygon Fill mode)
            if self.show_uv_wireframe || self.brush_engine.settings.mode == ToolMode::FillBucket {
                let wire_stroke = egui::Stroke::new(0.8_f32, egui::Color32::from_rgba_unmultiplied(220, 225, 235, 75));
                let clip = rect;
                let painter = ui.painter().with_clip_rect(clip);

                let verts = &self.project.mesh.vertices;
                let indices = &self.project.mesh.indices;
                let half_w = (wire_stroke.width * 0.5).max(0.4);
                let col = wire_stroke.color;

                let mut wire_mesh = egui::Mesh::default();
                let mut seen_edges = std::collections::HashSet::with_capacity(indices.len());

                for tri in indices.chunks_exact(3) {
                    let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
                    if i0 < verts.len() && i1 < verts.len() && i2 < verts.len() {
                        for &(a, b) in &[(i0, i1), (i1, i2), (i2, i0)] {
                            let key = if a < b { (a, b) } else { (b, a) };
                            if !seen_edges.insert(key) {
                                continue;
                            }

                            let p0 = cmin + egui::vec2(verts[a].uv.x * cw, verts[a].uv.y * ch);
                            let p1 = cmin + egui::vec2(verts[b].uv.x * cw, verts[b].uv.y * ch);

                            let edge_rect = egui::Rect::from_two_pos(p0, p1);
                            if !edge_rect.intersects(clip) {
                                continue;
                            }

                            let delta = p1 - p0;
                            let len = delta.length();
                            if len < 0.2 {
                                continue;
                            }
                            let norm = egui::vec2(-delta.y, delta.x) * (half_w / len);
                            let base = wire_mesh.vertices.len() as u32;
                            wire_mesh.vertices.push(egui::epaint::Vertex { pos: p0 + norm, uv: egui::pos2(0.0, 0.0), color: col });
                            wire_mesh.vertices.push(egui::epaint::Vertex { pos: p0 - norm, uv: egui::pos2(0.0, 0.0), color: col });
                            wire_mesh.vertices.push(egui::epaint::Vertex { pos: p1 - norm, uv: egui::pos2(0.0, 0.0), color: col });
                            wire_mesh.vertices.push(egui::epaint::Vertex { pos: p1 + norm, uv: egui::pos2(0.0, 0.0), color: col });
                            wire_mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                        }
                    }
                }

                if !wire_mesh.is_empty() {
                    painter.add(egui::Shape::mesh(wire_mesh));
                }
            }

            // 2D Vector Path Tool Interaction
            if self.brush_engine.settings.mode == ToolMode::Path && !is_panning {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.commit_path_tool(ctx);
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.brush_engine.settings.path_tool.clear();
                    self.status_message = "Path cleared".to_string();
                    ctx.request_repaint();
                } else if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
                    if let Some(idx) = self.brush_engine.settings.path_tool.selected_point {
                        if idx < self.brush_engine.settings.path_tool.points.len() {
                            self.brush_engine.settings.path_tool.points.remove(idx);
                            self.brush_engine.settings.path_tool.selected_point = None;
                            self.status_message = format!("Removed path point {}", idx + 1);
                            ctx.request_repaint();
                        }
                    }
                } else if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    if canvas_rect.contains(mouse_pos) {
                        let u = ((mouse_pos.x - canvas_rect.min.x) / cw).clamp(0.0, 1.0);
                        let v = ((mouse_pos.y - canvas_rect.min.y) / ch).clamp(0.0, 1.0);

                        let mut hovered_pt_idx = None;
                        for (i, pt) in self.brush_engine.settings.path_tool.points.iter().enumerate() {
                            let pt_2d = cmin + egui::vec2(pt.uv.x * cw, pt.uv.y * ch);
                            if (pt_2d - mouse_pos).length() < 14.0 {
                                hovered_pt_idx = Some(i);
                                break;
                            }
                        }

                        if response.clicked() {
                            if let Some(idx) = hovered_pt_idx {
                                self.brush_engine.settings.path_tool.selected_point = Some(idx);
                                self.status_message = format!("Selected path point {}", idx + 1);
                            } else {
                                let (pos_3d, norm_3d) = self.project.mesh.uv_to_3d_point(glam::Vec2::new(u, v)).unwrap_or((glam::Vec3::new(u, v, 0.0), glam::Vec3::Y));
                                self.brush_engine.settings.path_tool.points.push(crate::brush::PathControlPoint {
                                    position_3d: pos_3d,
                                    normal: norm_3d,
                                    uv: glam::Vec2::new(u, v),
                                    radius_scale: 1.0,
                                });
                                let new_idx = self.brush_engine.settings.path_tool.points.len() - 1;
                                self.brush_engine.settings.path_tool.selected_point = Some(new_idx);
                                self.status_message = format!("Added path point {} on UV canvas (Enter to commit, Esc to clear)", new_idx + 1);
                            }
                            ctx.request_repaint();
                        } else if response.drag_started_by(egui::PointerButton::Primary) {
                            if let Some(idx) = hovered_pt_idx {
                                self.brush_engine.settings.path_tool.selected_point = Some(idx);
                                self.brush_engine.settings.path_tool.is_dragging = true;
                            } else {
                                // Start freehand path stroke sketching directly across 2D UV canvas
                                let (pos_3d, norm_3d) = self.project.mesh.uv_to_3d_point(glam::Vec2::new(u, v)).unwrap_or((glam::Vec3::new(u, v, 0.0), glam::Vec3::Y));
                                self.brush_engine.settings.path_tool.is_sketching = true;
                                self.brush_engine.settings.path_tool.points.push(crate::brush::PathControlPoint {
                                    position_3d: pos_3d,
                                    normal: norm_3d,
                                    uv: glam::Vec2::new(u, v),
                                    radius_scale: 1.0,
                                });
                                let new_idx = self.brush_engine.settings.path_tool.points.len() - 1;
                                self.brush_engine.settings.path_tool.selected_point = Some(new_idx);
                                ctx.request_repaint();
                            }
                        } else if response.dragged_by(egui::PointerButton::Primary) {
                            if self.brush_engine.settings.path_tool.is_dragging {
                                if let Some(sel_idx) = self.brush_engine.settings.path_tool.selected_point {
                                    if sel_idx < self.brush_engine.settings.path_tool.points.len() {
                                        let (pos_3d, norm_3d) = self.project.mesh.uv_to_3d_point(glam::Vec2::new(u, v)).unwrap_or((glam::Vec3::new(u, v, 0.0), glam::Vec3::Y));
                                        self.brush_engine.settings.path_tool.points[sel_idx].position_3d = pos_3d;
                                        self.brush_engine.settings.path_tool.points[sel_idx].normal = norm_3d;
                                        self.brush_engine.settings.path_tool.points[sel_idx].uv = glam::Vec2::new(u, v);
                                        ctx.request_repaint();
                                    }
                                }
                            } else if self.brush_engine.settings.path_tool.is_sketching {
                                let doc_w = self.project.texture_width;
                                let uv_rad = (self.brush_engine.settings.radius / (doc_w as f32).max(1.0)).clamp(0.002, 0.25);
                                let min_uv_dist = (uv_rad * 1.5).max(0.02);
                                let can_add = self.brush_engine.settings.path_tool.points.last()
                                    .map(|last| (glam::Vec2::new(u, v) - last.uv).length() > min_uv_dist)
                                    .unwrap_or(true);
                                if can_add {
                                    let (pos_3d, norm_3d) = self.project.mesh.uv_to_3d_point(glam::Vec2::new(u, v)).unwrap_or((glam::Vec3::new(u, v, 0.0), glam::Vec3::Y));
                                    self.brush_engine.settings.path_tool.points.push(crate::brush::PathControlPoint {
                                        position_3d: pos_3d,
                                        normal: norm_3d,
                                        uv: glam::Vec2::new(u, v),
                                        radius_scale: 1.0,
                                    });
                                    let new_idx = self.brush_engine.settings.path_tool.points.len() - 1;
                                    self.brush_engine.settings.path_tool.selected_point = Some(new_idx);
                                    self.status_message = format!("Drawing UV path stroke ({} points)", self.brush_engine.settings.path_tool.points.len());
                                    ctx.request_repaint();
                                }
                            }
                        } else if response.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                            self.brush_engine.settings.path_tool.is_dragging = false;
                            self.brush_engine.settings.path_tool.is_sketching = false;
                        }
                    }
                }
                if response.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                    self.brush_engine.settings.path_tool.is_dragging = false;
                    self.brush_engine.settings.path_tool.is_sketching = false;
                }
            }
            // 2D Decal Stamp Tool Interaction
            else if self.brush_engine.settings.mode == ToolMode::Stamp && !is_panning {
                if response.clicked() || response.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        if canvas_rect.contains(mouse_pos) {
                            let u = ((mouse_pos.x - canvas_rect.min.x) / cw).clamp(0.0, 1.0);
                            let v = ((mouse_pos.y - canvas_rect.min.y) / ch).clamp(0.0, 1.0);

                            let act_idx = self.project.active_layer_idx;
                            if act_idx < self.project.layers.len() {
                                let is_fill = self.project.layers[act_idx].layer_type != LayerType::Paint;
                                let is_mask = self.brush_engine.settings.target == PaintTarget::Mask;
                                if is_fill && !is_mask {
                                    self.status_message = "Cannot stamp on Fill layer. Switch target to Mask or add a Paint layer.".to_string();
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
                                    let uv_rad = (self.brush_engine.settings.radius / (doc_w as f32).max(1.0)).clamp(0.002, 0.4);

                                    self.project.layers[act_idx].begin_stroke_recording(target, channels);

                                    let stamp_state = &mut self.brush_engine.settings.stamp_tool;
                                    if stamp_state.randomize_rotation {
                                        let rand_deg = ((std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() % 3600) as f32) / 10.0;
                                        stamp_state.rotation_angle = rand_deg;
                                    }

                                    let rot_rad = stamp_state.rotation_angle.to_radians();
                                    let stroke_style = stamp_state.stroke_style;
                                    let decal_type = stamp_state.decal_type;
                                    let heat_tint = stamp_state.heat_tint;
                                    let height_depth = stamp_state.height_depth;

                                    let layer = &mut self.project.layers[act_idx];
                                    self.brush_engine.stamp_uv_decal(
                                        layer,
                                        glam::Vec2::new(u, v),
                                        rot_rad,
                                        uv_rad,
                                        stroke_style,
                                        decal_type,
                                        heat_tint,
                                        0.0,
                                        height_depth,
                                    );

                                    layer.update_normal_from_height(1.0);

                                    let desc = format!("Stamp Decal UV: {}", if stroke_style == crate::brush::PathStrokeStyle::SmartDecal { decal_type.name() } else { stroke_style.name() });
                                    if let Some(action) = layer.finish_stroke_delta(act_idx, &desc) {
                                        self.history.push(&desc, action);
                                    }
                                    if let Some(d_rect) = layer.dirty_rect {
                                        if let Some(wgpu) = &mut self.wgpu_viewport {
                                            if let Some(gpu_layer) = wgpu.gpu_layers.get(act_idx) {
                                                wgpu.gpu_compositor.update_layer(layer, gpu_layer, doc_w, doc_h);
                                                wgpu.composite_and_update_active(&self.project, act_idx);
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
            // 2D UV Painting and Tool Actions
            else if !is_panning && (response.dragged_by(egui::PointerButton::Primary) || response.clicked()) {
                ctx.request_repaint();
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

                if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    if canvas_rect.contains(mouse_pos) || self.is_stroke_active {
                        let u = ((mouse_pos.x - canvas_rect.min.x) / canvas_rect.width()).clamp(0.0, 1.0);
                        let v = ((mouse_pos.y - canvas_rect.min.y) / canvas_rect.height()).clamp(0.0, 1.0);

                        if self.brush_engine.settings.mode == ToolMode::Clone && ui.input(|i| i.modifiers.alt || i.key_down(egui::Key::V)) {
                            self.brush_engine.settings.clone_source_uv = Some(glam::Vec2::new(u, v));
                            self.status_message = format!("Clone Source Set: UV ({:.3}, {:.3})", u, v);
                        } else if self.brush_engine.settings.mode == ToolMode::Eyedropper {
                            let sampled = self.project.composite_textures.base_color.sample_bilinear(glam::Vec2::new(u, v));
                            self.brush_engine.settings.paint_color = sampled;
                            self.brush_engine.settings.paint_roughness = self.project.composite_textures.roughness.sample_bilinear(glam::Vec2::new(u, v));
                            self.brush_engine.settings.paint_metallic = self.project.composite_textures.metallic.sample_bilinear(glam::Vec2::new(u, v));
                            self.brush_engine.settings.paint_height = self.project.composite_textures.height_map.sample_bilinear(glam::Vec2::new(u, v));
                            self.brush_engine.settings.paint_emissive = self.project.composite_textures.emissive.sample_bilinear(glam::Vec2::new(u, v));
                            self.brush_engine.settings.paint_opacity = self.project.composite_textures.opacity.sample_bilinear(glam::Vec2::new(u, v));
                            self.status_message = format!("Sampled Material [2D]: Color RGBA({:.2}, {:.2}, {:.2}, {:.2}), Rough {:.2}, Metal {:.2}, Emis RGB({:.2}, {:.2}, {:.2})", sampled.x, sampled.y, sampled.z, sampled.w, self.brush_engine.settings.paint_roughness, self.brush_engine.settings.paint_metallic, self.brush_engine.settings.paint_emissive.x, self.brush_engine.settings.paint_emissive.y, self.brush_engine.settings.paint_emissive.z);
                        } else if self.brush_engine.settings.mode == ToolMode::FillBucket {
                            let uv_point = glam::Vec2::new(u, v);
                            let tri_opt = self.project.mesh.find_triangle_at_uv(uv_point);
                            if let Some(tri_idx) = tri_opt {
                                let mode = self.brush_engine.settings.polygon_fill_mode;
                                let tris = match mode {
                                    crate::brush::PolygonFillMode::Triangle => vec![tri_idx],
                                    crate::brush::PolygonFillMode::Polygon => self.project.mesh.get_polygon_quad(tri_idx),
                                    crate::brush::PolygonFillMode::UvIsland => self.project.mesh.get_connected_uv_island(tri_idx),
                                    crate::brush::PolygonFillMode::MeshElement => self.project.mesh.get_connected_mesh_element(tri_idx),
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
                                                            wgpu.composite_and_update_active(&self.project, act_idx);
                                                            if let Some(preview_id) = self.mask_preview_layer_id {
                                                                wgpu.sync_mask_preview_full(&self.project, preview_id);
                                                            }
                                                        }
                                                    }
                                                    self.need_cpu_composite_sync = true;
                                                    self.update_uv_texture_rect(ctx, b_min_x, b_min_y, b_max_x, b_max_y);
                                                }
                                                ctx.request_repaint();
                                                self.status_message = format!("2D Polygon Fill ({}) filled {} triangles on mask", mode.name(), new_tris.len());
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
                                                            wgpu.composite_and_update_active(&self.project, act_idx);
                                                        }
                                                    }
                                                    self.need_cpu_composite_sync = true;
                                                    self.update_uv_texture_rect(ctx, b_min_x, b_min_y, b_max_x, b_max_y);
                                                }
                                                ctx.request_repaint();
                                                self.status_message = format!("2D Polygon Fill ({}) filled {} triangles with material", mode.name(), new_tris.len());
                                            } else {
                                                self.status_message = "Cannot fill material on Fill layer. Switch target to Mask or add a Paint layer.".to_string();
                                            }
                                        }
                                    }
                                }
                            } else {
                                self.status_message = "No UV polygon at clicked coordinate.".to_string();
                            }
                        } else {
                            let act_idx = self.project.active_layer_idx;
                            if act_idx < self.project.layers.len() {
                                let is_paint = self.project.layers[act_idx].layer_type == LayerType::Paint;
                                if self.brush_engine.settings.target == PaintTarget::Material && !is_paint {
                                    self.status_message = "Direct painting blocked on Fill layers. Switch target to Mask or add a Paint layer.".to_string();
                                } else {
                                    let pressure = ui.input(|i| {
                                        i.events.iter().find_map(|e| match e {
                                            egui::Event::Touch { force: Some(f), .. } => Some(*f),
                                            _ => None,
                                        }).unwrap_or(1.0)
                                    }).clamp(0.05, 1.0);
                                    self.last_stylus_pressure = pressure;
                                    let (effective_radius, effective_flow, effective_hardness) = self.brush_engine.settings.compute_effective_dynamics(pressure);
                                    let orig_rad = self.brush_engine.settings.radius;
                                    let orig_flow = self.brush_engine.settings.flow;
                                    let orig_hard = self.brush_engine.settings.hardness;
                                    self.brush_engine.settings.radius = effective_radius;
                                    self.brush_engine.settings.flow = effective_flow;
                                    self.brush_engine.settings.hardness = effective_hardness;
                                    let t_stroke = std::time::Instant::now();
                                    let is_gpu_compute = (self.brush_engine.settings.mode == ToolMode::Brush || self.brush_engine.settings.mode == ToolMode::Eraser)
                                        && self.wgpu_viewport.is_some();

                                    if is_gpu_compute {
                                        let doc_w = self.project.texture_width;
                                        let doc_h = self.project.texture_height;
                                        let wgpu = self.wgpu_viewport.as_mut().unwrap();
                                        if let Some(uniform) = self.brush_engine.build_gpu_stamp_data_2d(
                                            glam::Vec2::new(u, v),
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

                                            wgpu.stamp_2d_gpu(act_idx, &uniform);
                                            wgpu.composite_and_update_active(&self.project, act_idx);
                                            self.perf_metrics.texture_transfer_bytes = 0;
                                        }
                                    } else {
                                        self.brush_engine.paint_stroke(&mut self.project.layers[act_idx], glam::Vec2::new(u, v));
                                        let d_rect = self.project.layers[act_idx].dirty_rect;
                                        if let Some((d_x0, d_y0, d_x1, d_y1)) = d_rect {
                                            self.project.rebuild_composite_rect(
                                                d_x0 as usize,
                                                d_y0 as usize,
                                                d_x1 as usize,
                                                d_y1 as usize,
                                            );
                                            self.update_uv_texture_rect(ctx, d_x0, d_y0, d_x1, d_y1);
                                            if let Some(wgpu) = &mut self.wgpu_viewport {
                                                if let Some(gpu_layer) = wgpu.gpu_layers.get(act_idx) {
                                                    let doc_w = self.project.texture_width;
                                                    let doc_h = self.project.texture_height;
                                                    let active_layer = &mut self.project.layers[act_idx];
                                                    wgpu.gpu_compositor.update_layer(active_layer, gpu_layer, doc_w, doc_h);
                                                    wgpu.composite_and_update_active(&self.project, act_idx);
                                                }
                                            }
                                        } else {
                                            self.update_uv_texture(ctx);
                                        }
                                    }
                                    self.perf_metrics.paint_stroke_ms = t_stroke.elapsed().as_secs_f32() * 1000.0;
                                    self.brush_engine.settings.radius = orig_rad;
                                    self.brush_engine.settings.flow = orig_flow;
                                    self.brush_engine.settings.hardness = orig_hard;
                                    self.project.gpu_sync_needed = false;
                                    self.project.is_dirty = true;
                                    self.last_painted_uv = Some(glam::Vec2::new(u, v));
                                    self.last_hovered_uv_2d = Some(glam::Vec2::new(u, v));
                                    self.status_message = format!("2D UV Painting at ({:.3}, {:.3})", u, v);
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                }
            } else if response.drag_stopped() || (!ui.input(|i| i.pointer.any_down()) && self.is_stroke_active) {
                if self.is_stroke_active {
                    let idx = self.project.active_layer_idx;
                    let desc = if self.brush_engine.settings.mode == ToolMode::FillBucket {
                        format!("2D Polygon Fill ({})", self.brush_engine.settings.polygon_fill_mode.name())
                    } else {
                        "2D UV Paint Stroke".to_string()
                    };

                    if let Some((x0, y0, x1, y1)) = self.gpu_stroke_dirty_rect.take() {
                        if let Some(wgpu) = &mut self.wgpu_viewport {
                            if let Some(layer) = self.project.layers.get_mut(idx) {
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

            // --- 2D POLYGON FILL HOVER PREVIEW HIGHLIGHT ---
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
                    let cw = canvas_rect.width();
                    let ch = canvas_rect.height();
                    let cmin = canvas_rect.min;

                    let mut fill_mesh = egui::Mesh::default();
                    for &t in &preview_tris {
                        if t * 3 + 2 >= self.project.mesh.indices.len() { continue; }
                        let i0 = self.project.mesh.indices[t * 3] as usize;
                        let i1 = self.project.mesh.indices[t * 3 + 1] as usize;
                        let i2 = self.project.mesh.indices[t * 3 + 2] as usize;
                        if i0 >= self.project.mesh.vertices.len() || i1 >= self.project.mesh.vertices.len() || i2 >= self.project.mesh.vertices.len() { continue; }
                        let uv0 = self.project.mesh.vertices[i0].uv;
                        let uv1 = self.project.mesh.vertices[i1].uv;
                        let uv2 = self.project.mesh.vertices[i2].uv;

                        let p0 = cmin + egui::vec2(uv0.x * cw, uv0.y * ch);
                        let p1 = cmin + egui::vec2(uv1.x * cw, uv1.y * ch);
                        let p2 = cmin + egui::vec2(uv2.x * cw, uv2.y * ch);

                        let base = fill_mesh.vertices.len() as u32;
                        fill_mesh.vertices.push(egui::epaint::Vertex { pos: p0, uv: egui::pos2(0.0, 0.0), color: fill_col });
                        fill_mesh.vertices.push(egui::epaint::Vertex { pos: p1, uv: egui::pos2(0.0, 0.0), color: fill_col });
                        fill_mesh.vertices.push(egui::epaint::Vertex { pos: p2, uv: egui::pos2(0.0, 0.0), color: fill_col });
                        fill_mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
                    }
                    if !fill_mesh.is_empty() {
                        ui.painter().add(egui::Shape::mesh(fill_mesh));
                    }
                }
            }

            // --- 2D VECTOR PATH TOOL SPLINE & CONTROL POINTS OVERLAY ---
            if self.brush_engine.settings.mode == ToolMode::Path || !self.brush_engine.settings.path_tool.points.is_empty() {
                let doc_w = self.project.texture_width;
                let uv_rad = (self.brush_engine.settings.radius / (doc_w as f32).max(1.0)).clamp(0.002, 0.25);

                // 1. Interactive guide line from last point to cursor when hovering in Path mode
                if self.brush_engine.settings.mode == ToolMode::Path {
                    if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        if canvas_rect.contains(mouse_pos) && !self.brush_engine.settings.path_tool.is_dragging {
                            if let Some(last_pt) = self.brush_engine.settings.path_tool.points.last() {
                                let last_2d = cmin + egui::vec2(last_pt.uv.x * cw, last_pt.uv.y * ch);
                                ui.painter().line_segment(
                                    [last_2d, mouse_pos],
                                    egui::Stroke::new(1.8_f32, egui::Color32::from_rgba_unmultiplied(255, 180, 50, 200)),
                                );
                            }
                        }
                    }
                }

                // 2. Draw high-resolution continuous Catmull-Rom spline curve line with drop shadow (batched polyline)
                let curve_points = self.brush_engine.settings.path_tool.evaluate_spline_curve(32);
                let screen_pts: Vec<egui::Pos2> = curve_points.iter()
                    .map(|pt| cmin + egui::vec2(pt.uv.x * cw, pt.uv.y * ch))
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

                // 3. Draw discrete stamp indicators along the path line
                let samples = self.brush_engine.settings.path_tool.sample_path(uv_rad);
                for sample in &samples {
                    let cur_2d_pt = cmin + egui::vec2(sample.uv.x * cw, sample.uv.y * ch);
                    let r = match self.brush_engine.settings.path_tool.stroke_style {
                        crate::brush::PathStrokeStyle::SmartDecal => (4.0 * sample.radius_scale).clamp(3.0, 10.0),
                        crate::brush::PathStrokeStyle::WeldSeam => (2.8 * sample.radius_scale).clamp(1.8, 8.0),
                        _ => (2.2 * sample.radius_scale).clamp(1.2, 7.0),
                    };
                    ui.painter().circle_stroke(cur_2d_pt, r + 1.0, egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(20, 20, 25, 180)));
                    ui.painter().circle_filled(cur_2d_pt, r, egui::Color32::from_rgb(255, 215, 120));
                }

                // 4. Draw control point handle rings with index numbers
                let sel_idx = self.brush_engine.settings.path_tool.selected_point;
                for (i, pt) in self.brush_engine.settings.path_tool.points.iter().enumerate() {
                    let pt_2d = cmin + egui::vec2(pt.uv.x * cw, pt.uv.y * ch);
                    let is_selected = sel_idx == Some(i);

                    if is_selected {
                        ui.painter().circle_stroke(pt_2d, 9.0, egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(255, 230, 60)));
                        ui.painter().circle_filled(pt_2d, 5.0, egui::Color32::from_rgb(243, 94, 24));
                    } else {
                        ui.painter().circle_stroke(pt_2d, 7.5, egui::Stroke::new(2.0_f32, egui::Color32::WHITE));
                        ui.painter().circle_filled(pt_2d, 4.0, egui::Color32::from_rgb(30, 32, 40));
                    }

                    let badge_pos = pt_2d + egui::vec2(12.0, -12.0);
                    ui.painter().text(
                        badge_pos,
                        egui::Align2::LEFT_CENTER,
                        format!("{}", i + 1),
                        egui::FontId::monospace(11.0),
                        if is_selected { egui::Color32::from_rgb(255, 230, 60) } else { egui::Color32::WHITE },
                    );
                }
            }

            // Interactive 2D Brush Cursor and Symmetry Guides
            if response.hovered() && !is_panning {
                let pointer_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
                if pointer_moved || self.continuous_60fps {
                    ctx.request_repaint();
                }
                if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let u = ((mouse_pos.x - canvas_rect.min.x) / canvas_rect.width()).clamp(0.0, 1.0);
                    let v = ((mouse_pos.y - canvas_rect.min.y) / canvas_rect.height()).clamp(0.0, 1.0);
                    self.last_hovered_uv_2d = Some(glam::Vec2::new(u, v));
                    let uv_point = glam::Vec2::new(u, v);
                    if let Some(tri_idx) = self.project.mesh.find_triangle_at_uv(uv_point) {
                        self.last_hovered_tri = Some(tri_idx);
                    } else if self.last_hovered_hit_3d.is_none() {
                        self.last_hovered_tri = None;
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
                    } else if self.brush_engine.settings.mode == ToolMode::Stamp {
                        let screen_rad = (self.brush_engine.settings.radius / self.project.texture_width as f32 * canvas_rect.width()).max(2.0);
                        let rot_deg = self.brush_engine.settings.stamp_tool.rotation_angle;
                        let rot_rad = rot_deg.to_radians();

                        ui.painter().circle_stroke(
                            mouse_pos,
                            screen_rad,
                            egui::Stroke::new(1.8_f32, egui::Color32::from_rgb(255, 175, 40)),
                        );
                        let dir = egui::vec2(rot_rad.cos(), -rot_rad.sin());
                        let pointer_tip = mouse_pos + dir * screen_rad;
                        ui.painter().line_segment(
                            [mouse_pos, pointer_tip],
                            egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(255, 230, 80)),
                        );
                        let left_wing = mouse_pos + dir * (screen_rad - 6.0).max(3.0) + egui::vec2(-dir.y, dir.x) * 4.0;
                        let right_wing = mouse_pos + dir * (screen_rad - 6.0).max(3.0) - egui::vec2(-dir.y, dir.x) * 4.0;
                        ui.painter().line_segment([pointer_tip, left_wing], egui::Stroke::new(1.6_f32, egui::Color32::from_rgb(255, 230, 80)));
                        ui.painter().line_segment([pointer_tip, right_wing], egui::Stroke::new(1.6_f32, egui::Color32::from_rgb(255, 230, 80)));
                    } else {
                        let screen_rad = (self.brush_engine.settings.radius / self.project.texture_width as f32 * canvas_rect.width()).max(2.0);

                        // Primary brush ring
                        ui.painter().circle_stroke(
                            mouse_pos,
                            screen_rad,
                            egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220)),
                        );
                        ui.painter().circle_filled(
                            mouse_pos,
                            1.5,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 240),
                        );

                        // Mirrored symmetry ghost cursors in 2D
                        if self.brush_engine.settings.symmetry_x || self.brush_engine.settings.symmetry_y {
                            let sym_stroke = egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(60, 180, 255, 220));

                            if self.brush_engine.settings.symmetry_x {
                                let sym_pos = canvas_rect.min + egui::vec2((1.0 - u) * canvas_rect.width(), v * canvas_rect.height());
                                ui.painter().circle_stroke(sym_pos, screen_rad, sym_stroke);
                                ui.painter().circle_filled(sym_pos, 1.5, egui::Color32::from_rgba_unmultiplied(60, 180, 255, 255));
                            }
                            if self.brush_engine.settings.symmetry_y {
                                let sym_pos = canvas_rect.min + egui::vec2(u * canvas_rect.width(), (1.0 - v) * canvas_rect.height());
                                ui.painter().circle_stroke(sym_pos, screen_rad, sym_stroke);
                                ui.painter().circle_filled(sym_pos, 1.5, egui::Color32::from_rgba_unmultiplied(60, 180, 255, 255));
                            }
                        }
                    }
                }
            } else {
                self.last_hovered_uv_2d = None;
                if self.last_hovered_hit_3d.is_none() {
                    self.last_hovered_tri = None;
                }
            }

            // Synchronized 3D-to-2D cross-viewport cursor indicator
            if !response.hovered() {
                if let Some((_pos_3d, _norm_3d, uv)) = self.last_hovered_hit_3d {
                    let screen_x = canvas_rect.min.x + uv.x * canvas_rect.width();
                    let screen_y = canvas_rect.min.y + uv.y * canvas_rect.height();
                    let p = egui::pos2(screen_x, screen_y);
                    if canvas_rect.contains(p) {
                        let screen_rad = (self.brush_engine.settings.radius / (self.project.texture_width as f32) * canvas_rect.width()).max(3.0);
                        ui.painter().circle_stroke(
                            p,
                            screen_rad,
                            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0, 220, 255)),
                        );
                        ui.painter().circle_filled(
                            p,
                            2.0,
                            egui::Color32::from_rgb(0, 220, 255),
                        );
                    }
                }
            }

            // Top-right floating controls pill
            let hud_pos = rect.right_top() + egui::vec2(-360.0, 10.0);
            let hud_rect = egui::Rect::from_min_size(hud_pos, egui::vec2(350.0, 30.0));
            ui.painter().rect_filled(hud_rect, egui::Rounding::same(15.0), egui::Color32::from_rgba_unmultiplied(28, 30, 36, 225));
            ui.painter().rect_stroke(hud_rect, egui::Rounding::same(15.0), egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(58, 62, 74)));

            ui.allocate_ui_at_rect(hud_rect, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(format!("{:.0}%", self.uv_2d_zoom * 100.0)).small().strong().color(Color32::from_rgb(180, 190, 205)));
                    if ui.small_button("1:1").on_hover_text("Reset zoom to 100% (1:1 pixel scale)").clicked() {
                        self.uv_2d_zoom = 1.0;
                    }
                    if ui.small_button("Fit (F)").on_hover_text("Reset 2D UV view and zoom to fit (F)").clicked() {
                        self.uv_2d_pan = egui::Vec2::ZERO;
                        self.uv_2d_zoom = 1.0;
                    }
                    let wire_active = self.show_uv_wireframe;
                    let wire_label = if wire_active { "UV Wire: ON" } else { "UV Wire: OFF" };
                    if ui.selectable_label(wire_active, wire_label).on_hover_text("Toggle UV wireframe overlay").clicked() {
                        self.show_uv_wireframe = !self.show_uv_wireframe;
                    }

                    let has_mask = self.project.active_layer().map(|l| l.has_mask).unwrap_or(false);
                    if has_mask {
                        let mask_label = if self.uv_view_mask_mode { "Mask: ON" } else { "Mask: OFF" };
                        if ui.selectable_label(self.uv_view_mask_mode, mask_label).on_hover_text("Preview and paint directly on active layer mask in 2D UV canvas").clicked() {
                            self.uv_view_mask_mode = !self.uv_view_mask_mode;
                            self.update_uv_texture(ctx);
                        }
                    }
                });
            });

            // Top-left HUD channel badge
            let title_pos = rect.left_top() + egui::vec2(12.0, 12.0);
            let title_text = if self.uv_view_mask_mode {
                format!("2D UV Viewport | MASK PREVIEW | {}x{}", self.project.texture_width, self.project.texture_height)
            } else {
                format!("2D UV Viewport | Channel: {} | {}x{}", self.view_mode.name(), self.project.texture_width, self.project.texture_height)
            };
            ui.painter().text(
                title_pos,
                egui::Align2::LEFT_TOP,
                title_text,
                egui::FontId::proportional(11.5),
                egui::Color32::from_rgba_unmultiplied(200, 205, 215, 190),
            );
        }
    }
}
