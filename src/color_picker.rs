// 3D Paintcraft - Intuitive HSV/HSL Color Wheel & Palette Widget
// High-performance GPU-mesh color disc with hex code parsing, copying, and pasting

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use std::f32::consts::PI;

/// Convert RGB (0.0..=1.0) to HSV (0.0..=1.0)
pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let mut h = 0.0;
    let s = if max > 1e-5 { delta / max } else { 0.0 };
    let v = max;

    if delta > 1e-5 {
        if (max - r).abs() < 1e-5 {
            h = (g - b) / delta;
            if h < 0.0 {
                h += 6.0;
            }
        } else if (max - g).abs() < 1e-5 {
            h = 2.0 + (b - r) / delta;
        } else {
            h = 4.0 + (r - g) / delta;
        }
        h /= 6.0;
    }
    (h.fract().max(0.0), s.clamp(0.0, 1.0), v.clamp(0.0, 1.0))
}

/// Convert HSV (0.0..=1.0) to RGB (0.0..=1.0)
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    if s <= 1e-5 {
        return (v, v, v);
    }
    let h_norm = (h.fract() + 1.0).fract() * 6.0;
    let i = h_norm.floor() as i32;
    let f = h_norm - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

/// Parse hex string (#RGB, #RRGGBB, #RRGGBBAA, RGB, RRGGBB) into RGB [0.0..=1.0]
pub fn parse_hex_color(hex_str: &str) -> Option<[f32; 3]> {
    let clean = hex_str.trim().trim_start_matches('#');
    if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
        let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
        let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
        Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
    } else if clean.len() == 3 {
        let r = u8::from_str_radix(&clean[0..1], 16).ok()?;
        let g = u8::from_str_radix(&clean[1..2], 16).ok()?;
        let b = u8::from_str_radix(&clean[2..3], 16).ok()?;
        Some([(r * 17) as f32 / 255.0, (g * 17) as f32 / 255.0, (b * 17) as f32 / 255.0])
    } else if clean.len() == 8 {
        let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
        let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
        let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
        Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
    } else {
        None
    }
}

/// Format RGB [0.0..=1.0] as #RRGGBB
pub fn rgb_to_hex(r: f32, g: f32, b: f32) -> String {
    let ir = (r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let ig = (g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let ib = (b.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", ir, ig, ib)
}

/// Copy text to system clipboard via egui context and wl-copy
pub fn copy_to_clipboard(ctx: &egui::Context, text: &str) {
    ctx.output_mut(|o| o.copied_text = text.to_string());
    // Also broadcast to wl-copy on Linux if running in Wayland
    let _ = std::process::Command::new("wl-copy")
        .arg(text)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Read text from system clipboard via wl-paste or egui paste events
pub fn read_from_clipboard(ctx: &egui::Context) -> Option<String> {
    // 1. Try reading from wl-paste
    if let Ok(output) = std::process::Command::new("wl-paste")
        .arg("--no-newline")
        .output()
    {
        if output.status.success() {
            if let Ok(s) = String::from_utf8(output.stdout) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    // 2. Fallback to egui input events
    let mut egui_paste = None;
    ctx.input(|i| {
        for event in &i.events {
            if let egui::Event::Paste(text) = event {
                egui_paste = Some(text.trim().to_string());
            }
        }
    });
    egui_paste
}

/// Persistent state for an active color picker popup instance
#[derive(Clone, Debug)]
pub struct ColorPickerState {
    pub is_open: bool,
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    pub hex_input: String,
    pub initial_rgb: [f32; 3],
    pub dragging_hue: bool,
    pub dragging_sv: bool,
    pub popup_pos: Option<Pos2>,
}

impl Default for ColorPickerState {
    fn default() -> Self {
        Self {
            is_open: false,
            hue: 0.0,
            saturation: 1.0,
            value: 1.0,
            hex_input: "#FFFFFF".to_string(),
            initial_rgb: [1.0, 1.0, 1.0],
            dragging_hue: false,
            dragging_sv: false,
            popup_pos: None,
        }
    }
}

/// Standard artist palette swatches for quick material painting
pub const PALETTE_SWATCHES: &[(&str, &str)] = &[
    ("White", "#FFFFFF"),
    ("Lt Gray", "#B4B4B4"),
    ("Mid Gray", "#808080"),
    ("Dark Slate", "#2A2D34"),
    ("Black", "#000000"),
    ("Red", "#E53935"),
    ("Orange", "#F35E18"),
    ("Yellow", "#FBC02D"),
    ("Green", "#43A047"),
    ("Cyan", "#00ACC1"),
    ("Blue", "#1E88E5"),
    ("Purple", "#8E24AA"),
    ("Skin", "#E8BEAC"),
    ("Copper", "#B87333"),
    ("Silver", "#A0AAB2"),
    ("Rust", "#A0522D"),
];

/// Show the interactive Color Wheel popup content. Returns true if color was changed.
pub fn show_color_wheel_content(
    ui: &mut egui::Ui,
    state: &mut ColorPickerState,
    rgb: &mut [f32; 3],
) -> bool {
    let mut changed = false;

    // Synchronize HSV from RGB when not actively dragging
    if !state.dragging_hue && !state.dragging_sv {
        let (h, s, v) = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
        // Preserve hue if saturation/value was zero
        if s > 1e-4 && v > 1e-4 {
            state.hue = h;
        }
        state.saturation = s;
        state.value = v;
        state.hex_input = rgb_to_hex(rgb[0], rgb[1], rgb[2]);
    }

    ui.vertical(|ui| {
        // --- 1. HEADER: COLOR COMPARISON & QUICK CONTROLS ---
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("COLOR").small().strong().color(Color32::from_rgb(155, 160, 175)));
            ui.add_space(2.0);

            // Previous vs Current color split preview swatch
            let (preview_rect, _) = ui.allocate_exact_size(Vec2::new(56.0, 18.0), egui::Sense::hover());
            let half_w = preview_rect.width() * 0.5;
            let left_rect = Rect::from_min_max(preview_rect.min, Pos2::new(preview_rect.min.x + half_w, preview_rect.max.y));
            let right_rect = Rect::from_min_max(Pos2::new(preview_rect.min.x + half_w, preview_rect.min.y), preview_rect.max);

            let init_c = Color32::from_rgb(
                (state.initial_rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                (state.initial_rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                (state.initial_rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            );
            let cur_c = Color32::from_rgb(
                (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            );

            ui.painter().rect_filled(left_rect, egui::Rounding { nw: 3.0, sw: 3.0, ne: 0.0, se: 0.0 }, init_c);
            ui.painter().rect_filled(right_rect, egui::Rounding { nw: 0.0, sw: 0.0, ne: 3.0, se: 3.0 }, cur_c);
            ui.painter().rect_stroke(preview_rect, egui::Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(60, 65, 75)));

            if ui.small_button("↺ Revert").on_hover_text("Revert to initial color").clicked() {
                *rgb = state.initial_rgb;
                let (h, s, v) = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
                state.hue = h;
                state.saturation = s;
                state.value = v;
                state.hex_input = rgb_to_hex(rgb[0], rgb[1], rgb[2]);
                changed = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✕").on_hover_text("Close color picker (Esc)").clicked() {
                    state.is_open = false;
                }
            });
        });

        ui.add_space(6.0);

        // --- 2. PERFECTLY CENTERED HUE RING & SATURATION-VALUE SQUARE ---
        let wheel_size = 176.0f32;
        let avail_w = ui.available_width();
        let offset_x = ((avail_w - wheel_size) * 0.5).max(0.0);

        let mut center = Pos2::ZERO;
        let r_outer = wheel_size * 0.48; // 84.48
        let r_inner = wheel_size * 0.36; // 63.36
        let sq_half = 43.0f32;           // 86x86 px box, corners at 43 * 1.414 = 60.8 < 63.36
        let mut sq_rect = Rect::NOTHING;
        let mut wheel_resp = None;

        ui.horizontal(|ui| {
            ui.add_space(offset_x);
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(wheel_size), egui::Sense::click_and_drag());
            center = rect.center();
            sq_rect = Rect::from_center_size(center, Vec2::splat(sq_half * 2.0));
            wheel_resp = Some(resp);
        });

        let response = wheel_resp.unwrap();

        // Interactive pointer handling
        let pointer_pos = response.interact_pointer_pos();
        if response.drag_started() {
            if let Some(pos) = pointer_pos {
                let dist = (pos - center).length();
                if dist >= r_inner - 4.0 && dist <= r_outer + 8.0 {
                    state.dragging_hue = true;
                } else if sq_rect.expand(4.0).contains(pos) {
                    state.dragging_sv = true;
                }
            }
        }
        if response.drag_stopped() {
            state.dragging_hue = false;
            state.dragging_sv = false;
        }

        if let Some(pos) = pointer_pos {
            if state.dragging_hue || (response.clicked() && (pos - center).length() >= r_inner - 2.0) {
                let angle = (pos.y - center.y).atan2(pos.x - center.x);
                let mut h = angle / (2.0 * PI);
                if h < 0.0 {
                    h += 1.0;
                }
                state.hue = h.clamp(0.0, 1.0);
                let (nr, ng, nb) = hsv_to_rgb(state.hue, state.saturation, state.value);
                *rgb = [nr, ng, nb];
                state.hex_input = rgb_to_hex(nr, ng, nb);
                changed = true;
            } else if state.dragging_sv || (response.clicked() && sq_rect.contains(pos)) {
                let s = ((pos.x - sq_rect.min.x) / sq_rect.width()).clamp(0.0, 1.0);
                let v = (1.0 - (pos.y - sq_rect.min.y) / sq_rect.height()).clamp(0.0, 1.0);
                state.saturation = s;
                state.value = v;
                let (nr, ng, nb) = hsv_to_rgb(state.hue, state.saturation, state.value);
                *rgb = [nr, ng, nb];
                state.hex_input = rgb_to_hex(nr, ng, nb);
                changed = true;
            }
        }

        // --- Render Hue Ring (64 Segment Smooth Gradient Mesh) ---
        let mut mesh = egui::epaint::Mesh::default();
        let segments = 64;
        for i in 0..segments {
            let a0 = (i as f32) / (segments as f32) * 2.0 * PI;
            let a1 = ((i + 1) as f32) / (segments as f32) * 2.0 * PI;

            let h0 = (i as f32) / (segments as f32);
            let h1 = ((i + 1) as f32) / (segments as f32);

            let (r0, g0, b0) = hsv_to_rgb(h0, 1.0, 1.0);
            let (r1, g1, b1) = hsv_to_rgb(h1, 1.0, 1.0);

            let c0 = Color32::from_rgb((r0 * 255.0) as u8, (g0 * 255.0) as u8, (b0 * 255.0) as u8);
            let c1 = Color32::from_rgb((r1 * 255.0) as u8, (g1 * 255.0) as u8, (b1 * 255.0) as u8);

            let dir0 = Vec2::new(a0.cos(), a0.sin());
            let dir1 = Vec2::new(a1.cos(), a1.sin());

            let p_in0 = center + dir0 * r_inner;
            let p_out0 = center + dir0 * r_outer;
            let p_in1 = center + dir1 * r_inner;
            let p_out1 = center + dir1 * r_outer;

            let idx = mesh.vertices.len() as u32;
            mesh.vertices.push(egui::epaint::Vertex { pos: p_in0, uv: Pos2::ZERO, color: c0 });
            mesh.vertices.push(egui::epaint::Vertex { pos: p_out0, uv: Pos2::ZERO, color: c0 });
            mesh.vertices.push(egui::epaint::Vertex { pos: p_in1, uv: Pos2::ZERO, color: c1 });
            mesh.vertices.push(egui::epaint::Vertex { pos: p_out1, uv: Pos2::ZERO, color: c1 });

            mesh.indices.extend_from_slice(&[idx, idx + 1, idx + 2, idx + 1, idx + 3, idx + 2]);
        }

        // --- Render Saturation/Value Bilinear Quad ---
        let (hr, hg, hb) = hsv_to_rgb(state.hue, 1.0, 1.0);
        let hue_color = Color32::from_rgb((hr * 255.0) as u8, (hg * 255.0) as u8, (hb * 255.0) as u8);
        let white_color = Color32::WHITE;
        let black_color = Color32::BLACK;

        let p_tl = sq_rect.left_top();
        let p_tr = sq_rect.right_top();
        let p_bl = sq_rect.left_bottom();
        let p_br = sq_rect.right_bottom();

        let sq_idx = mesh.vertices.len() as u32;
        mesh.vertices.push(egui::epaint::Vertex { pos: p_tl, uv: Pos2::ZERO, color: white_color });
        mesh.vertices.push(egui::epaint::Vertex { pos: p_tr, uv: Pos2::ZERO, color: hue_color });
        mesh.vertices.push(egui::epaint::Vertex { pos: p_bl, uv: Pos2::ZERO, color: black_color });
        mesh.vertices.push(egui::epaint::Vertex { pos: p_br, uv: Pos2::ZERO, color: black_color });

        mesh.indices.extend_from_slice(&[sq_idx, sq_idx + 1, sq_idx + 2, sq_idx + 1, sq_idx + 3, sq_idx + 2]);

        ui.painter().add(mesh);

        // Ring and Square subtle border outlines
        ui.painter().circle_stroke(center, r_outer, Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 56)));
        ui.painter().circle_stroke(center, r_inner, Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 56)));
        ui.painter().rect_stroke(sq_rect, egui::Rounding::ZERO, Stroke::new(1.0_f32, Color32::from_rgb(60, 65, 75)));

        // --- Reticle: Hue cursor on ring ---
        let hue_angle = state.hue * 2.0 * PI;
        let hue_r_mid = (r_inner + r_outer) * 0.5;
        let hue_cursor_pos = center + Vec2::new(hue_angle.cos(), hue_angle.sin()) * hue_r_mid;
        ui.painter().circle_stroke(hue_cursor_pos, 5.0, Stroke::new(2.0_f32, Color32::BLACK));
        ui.painter().circle_stroke(hue_cursor_pos, 4.0, Stroke::new(1.5_f32, Color32::WHITE));

        // --- Reticle: SV cursor on square ---
        let sv_cursor_pos = Pos2::new(
            sq_rect.min.x + state.saturation * sq_rect.width(),
            sq_rect.min.y + (1.0 - state.value) * sq_rect.height(),
        );
        ui.painter().circle_stroke(sv_cursor_pos, 5.0, Stroke::new(2.0_f32, Color32::BLACK));
        ui.painter().circle_stroke(sv_cursor_pos, 4.0, Stroke::new(1.5_f32, Color32::WHITE));

        ui.add_space(8.0);

        // --- 3. DIRECT HEX EDITING & CLIPBOARD INTEGRATION ---
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Hex:").small().strong().color(Color32::from_rgb(160, 165, 175)));
            let hex_resp = ui.add_sized(
                [76.0, 20.0],
                egui::TextEdit::singleline(&mut state.hex_input)
                    .hint_text("#RRGGBB")
                    .font(egui::TextStyle::Monospace),
            );

            if hex_resp.changed() {
                if let Some(parsed) = parse_hex_color(&state.hex_input) {
                    *rgb = parsed;
                    let (h, s, v) = rgb_to_hsv(parsed[0], parsed[1], parsed[2]);
                    state.hue = h;
                    state.saturation = s;
                    state.value = v;
                    changed = true;
                }
            }

            if ui.button("Copy").on_hover_text("Copy hex code to clipboard").clicked() {
                let hex_str = rgb_to_hex(rgb[0], rgb[1], rgb[2]);
                copy_to_clipboard(ui.ctx(), &hex_str);
            }

            if ui.button("Paste").on_hover_text("Paste hex code from clipboard").clicked() {
                if let Some(clip_text) = read_from_clipboard(ui.ctx()) {
                    if let Some(parsed) = parse_hex_color(&clip_text) {
                        *rgb = parsed;
                        let (h, s, v) = rgb_to_hsv(parsed[0], parsed[1], parsed[2]);
                        state.hue = h;
                        state.saturation = s;
                        state.value = v;
                        state.hex_input = rgb_to_hex(parsed[0], parsed[1], parsed[2]);
                        changed = true;
                    }
                }
            }
        });

        ui.add_space(4.0);

        // --- 4. PRECISE RGB CHANNEL VALUES ---
        let mut r_int = (rgb[0] * 255.0).round() as u8;
        let mut g_int = (rgb[1] * 255.0).round() as u8;
        let mut b_int = (rgb[2] * 255.0).round() as u8;

        ui.horizontal(|ui| {
            ui.colored_label(Color32::from_rgb(255, 110, 110), "R");
            if ui.add_sized([42.0, 18.0], egui::DragValue::new(&mut r_int).range(0..=255)).changed() {
                rgb[0] = r_int as f32 / 255.0;
                let (h, s, v) = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
                state.hue = h;
                state.saturation = s;
                state.value = v;
                state.hex_input = rgb_to_hex(rgb[0], rgb[1], rgb[2]);
                changed = true;
            }
            ui.colored_label(Color32::from_rgb(110, 230, 110), "G");
            if ui.add_sized([42.0, 18.0], egui::DragValue::new(&mut g_int).range(0..=255)).changed() {
                rgb[1] = g_int as f32 / 255.0;
                let (h, s, v) = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
                state.hue = h;
                state.saturation = s;
                state.value = v;
                state.hex_input = rgb_to_hex(rgb[0], rgb[1], rgb[2]);
                changed = true;
            }
            ui.colored_label(Color32::from_rgb(110, 170, 255), "B");
            if ui.add_sized([42.0, 18.0], egui::DragValue::new(&mut b_int).range(0..=255)).changed() {
                rgb[2] = b_int as f32 / 255.0;
                let (h, s, v) = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
                state.hue = h;
                state.saturation = s;
                state.value = v;
                state.hex_input = rgb_to_hex(rgb[0], rgb[1], rgb[2]);
                changed = true;
            }
        });

        ui.add_space(5.0);

        // --- 5. CENTERED 2x8 PALETTE SWATCHES GRID ---
        let swatch_w = 20.0f32;
        let swatch_h = 16.0f32;
        let swatch_spacing = 4.0f32;
        let total_swatches_w = 8.0 * swatch_w + 7.0 * swatch_spacing; // 188px
        let swatches_offset = ((avail_w - total_swatches_w) * 0.5).max(0.0);

        for row in 0..2 {
            ui.horizontal(|ui| {
                ui.add_space(swatches_offset);
                ui.spacing_mut().item_spacing = Vec2::new(swatch_spacing, 0.0);
                for col in 0..8 {
                    let idx = row * 8 + col;
                    if let Some(&(name, hex)) = PALETTE_SWATCHES.get(idx) {
                        if let Some(swatch_rgb) = parse_hex_color(hex) {
                            let c = Color32::from_rgb(
                                (swatch_rgb[0] * 255.0) as u8,
                                (swatch_rgb[1] * 255.0) as u8,
                                (swatch_rgb[2] * 255.0) as u8,
                            );
                            let (swatch_rect, resp) = ui.allocate_exact_size(Vec2::new(swatch_w, swatch_h), egui::Sense::click());
                            ui.painter().rect_filled(swatch_rect, 2.0, c);
                            let border_col = if resp.hovered() { Color32::WHITE } else { Color32::from_rgb(55, 60, 70) };
                            ui.painter().rect_stroke(swatch_rect, 2.0, Stroke::new(1.0_f32, border_col));

                            if resp.on_hover_text(format!("{} ({})", name, hex)).clicked() {
                                *rgb = swatch_rgb;
                                let (h, s, v) = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
                                state.hue = h;
                                state.saturation = s;
                                state.value = v;
                                state.hex_input = hex.to_string();
                                changed = true;
                            }
                        }
                    }
                }
            });
            if row == 0 {
                ui.add_space(swatch_spacing);
            }
        }
    });

    changed
}

/// Standard color picker button for `[f32; 3]` (linear or sRGB 0.0..=1.0).
/// Renders an elegant swatch button with hex label. Clicking opens the Color Wheel popup window.
pub fn color_picker_button_rgb(ui: &mut egui::Ui, id: egui::Id, rgb: &mut [f32; 3]) -> bool {
    let mut state = ui.data(|d| d.get_temp::<ColorPickerState>(id)).unwrap_or_else(|| {
        let (h, s, v) = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
        ColorPickerState {
            is_open: false,
            hue: h,
            saturation: s,
            value: v,
            hex_input: rgb_to_hex(rgb[0], rgb[1], rgb[2]),
            initial_rgb: *rgb,
            dragging_hue: false,
            dragging_sv: false,
            popup_pos: None,
        }
    });

    let swatch_size = Vec2::new(26.0, 18.0);
    let (btn_rect, response) = ui.allocate_exact_size(swatch_size, egui::Sense::click());

    let c = Color32::from_rgb(
        (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8,
    );

    let stroke = if response.hovered() || state.is_open {
        Stroke::new(1.5_f32, Color32::WHITE)
    } else {
        Stroke::new(1.0_f32, Color32::from_rgb(65, 70, 80))
    };

    ui.painter().rect_filled(btn_rect, 3.0, c);
    ui.painter().rect_stroke(btn_rect, 3.0, stroke);

    if response.clicked() {
        state.is_open = !state.is_open;
        if state.is_open {
            state.initial_rgb = *rgb;
            let (h, s, v) = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
            state.hue = h;
            state.saturation = s;
            state.value = v;
            state.hex_input = rgb_to_hex(rgb[0], rgb[1], rgb[2]);

            // Calculate anchored flyout spawn position adjacent to the button
            let screen_rect = ui.ctx().screen_rect();
            let popup_w = 236.0f32;
            let popup_h = 430.0f32;

            let pos = if btn_rect.min.x > screen_rect.center().x {
                // Button is on the right half of the screen (e.g. Inspector/Properties panel) -> spawn to the left
                Pos2::new(btn_rect.min.x - popup_w - 8.0, btn_rect.min.y)
            } else if btn_rect.max.y < screen_rect.min.y + 140.0 {
                // Button is in top toolbar -> spawn below
                Pos2::new(btn_rect.min.x, btn_rect.max.y + 6.0)
            } else {
                // Default: spawn to the right of the button
                Pos2::new(btn_rect.max.x + 8.0, btn_rect.min.y)
            };

            let min_x = screen_rect.min.x + 10.0;
            let max_x = (screen_rect.max.x - popup_w - 10.0).max(min_x);
            let min_y = screen_rect.min.y + 10.0;
            let max_y = (screen_rect.max.y - popup_h - 10.0).max(min_y);

            let clamped_pos = Pos2::new(pos.x.clamp(min_x, max_x), pos.y.clamp(min_y, max_y));
            state.popup_pos = Some(clamped_pos);
        }
    }

    let mut changed = false;

    if state.is_open {
        let popup_title = format!("Color Wheel ###{}", id.value());
        let spawn_pos = state.popup_pos.unwrap_or_else(|| {
            let screen_rect = ui.ctx().screen_rect();
            let popup_w = 236.0f32;
            let popup_h = 430.0f32;
            let pos = if btn_rect.min.x > screen_rect.center().x {
                Pos2::new(btn_rect.min.x - popup_w - 8.0, btn_rect.min.y)
            } else if btn_rect.max.y < screen_rect.min.y + 140.0 {
                Pos2::new(btn_rect.min.x, btn_rect.max.y + 6.0)
            } else {
                Pos2::new(btn_rect.max.x + 8.0, btn_rect.min.y)
            };
            let min_x = screen_rect.min.x + 10.0;
            let max_x = (screen_rect.max.x - popup_w - 10.0).max(min_x);
            let min_y = screen_rect.min.y + 10.0;
            let max_y = (screen_rect.max.y - popup_h - 10.0).max(min_y);
            Pos2::new(pos.x.clamp(min_x, max_x), pos.y.clamp(min_y, max_y))
        });

        let win_resp = egui::Window::new(popup_title)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(236.0, 430.0))
            .current_pos(spawn_pos)
            .order(egui::Order::Foreground)
            .frame(
                egui::Frame::window(ui.style())
                    .fill(Color32::from_rgb(26, 28, 33))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 74)))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(10.0)),
            )
            .show(ui.ctx(), |ui| {
                if show_color_wheel_content(ui, &mut state, rgb) {
                    changed = true;
                }
            });

        if let Some(win_resp) = win_resp {
            let win_rect = win_resp.response.rect;

            // Auto-close on Escape key
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                state.is_open = false;
            }

            // Auto-close on click outside window and button
            if !state.dragging_hue && !state.dragging_sv && ui.input(|i| i.pointer.any_pressed()) {
                if let Some(ptr) = ui.input(|i| i.pointer.interact_pos()) {
                    if !win_rect.contains(ptr) && !btn_rect.contains(ptr) {
                        state.is_open = false;
                    }
                }
            }
        }
    }

    ui.data_mut(|d| d.insert_temp(id, state));
    changed
}

/// Standard color picker button for `glam::Vec4` (RGB channels used, W preserved).
pub fn color_picker_button_vec4(ui: &mut egui::Ui, id: egui::Id, color: &mut glam::Vec4) -> bool {
    let mut rgb = [color.x, color.y, color.z];
    if color_picker_button_rgb(ui, id, &mut rgb) {
        color.x = rgb[0];
        color.y = rgb[1];
        color.z = rgb[2];
        true
    } else {
        false
    }
}

/// Standard color picker button for `[u8; 3]` (sRGB bytes 0..=255).
pub fn color_picker_button_srgb(ui: &mut egui::Ui, id: egui::Id, srgb: &mut [u8; 3]) -> bool {
    let mut rgb = [
        srgb[0] as f32 / 255.0,
        srgb[1] as f32 / 255.0,
        srgb[2] as f32 / 255.0,
    ];
    if color_picker_button_rgb(ui, id, &mut rgb) {
        srgb[0] = (rgb[0] * 255.0).round() as u8;
        srgb[1] = (rgb[1] * 255.0).round() as u8;
        srgb[2] = (rgb[2] * 255.0).round() as u8;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_hsv_and_back() {
        let test_colors = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.5, 0.5, 0.5],
            [0.95, 0.37, 0.1],
        ];

        for c in test_colors {
            let (h, s, v) = rgb_to_hsv(c[0], c[1], c[2]);
            let (r, g, b) = hsv_to_rgb(h, s, v);
            assert!((r - c[0]).abs() < 1e-3, "Red mismatch for {:?}", c);
            assert!((g - c[1]).abs() < 1e-3, "Green mismatch for {:?}", c);
            assert!((b - c[2]).abs() < 1e-3, "Blue mismatch for {:?}", c);
        }
    }

    #[test]
    fn test_hex_parsing_and_formatting() {
        assert_eq!(parse_hex_color("#FFFFFF"), Some([1.0, 1.0, 1.0]));
        assert_eq!(parse_hex_color("000000"), Some([0.0, 0.0, 0.0]));
        assert_eq!(parse_hex_color("#F00"), Some([1.0, 0.0, 0.0]));
        assert_eq!(parse_hex_color("0F0"), Some([0.0, 1.0, 0.0]));
        assert_eq!(parse_hex_color("#0000FFFF"), Some([0.0, 0.0, 1.0]));
        assert_eq!(parse_hex_color("invalid_hex"), None);

        assert_eq!(rgb_to_hex(1.0, 0.5, 0.0), "#FF8000");
        assert_eq!(rgb_to_hex(0.0, 1.0, 0.0), "#00FF00");
    }

    #[test]
    fn test_palette_swatches_valid() {
        for &(name, hex) in PALETTE_SWATCHES {
            let parsed = parse_hex_color(hex);
            assert!(parsed.is_some(), "Invalid palette swatch {}: {}", name, hex);
        }
    }
}
