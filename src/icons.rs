// 3D Paintcraft - Native Icon System & Asset Bundler
// Generates and manages high-resolution 64x64 PNG and SVG icon assets

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::path::{Path, PathBuf};

/// Simple 2D software drawing canvas for generating antialiased icons
pub struct IconCanvas {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl IconCanvas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0u8; (width * height * 4) as usize],
        }
    }

    pub fn blend_pixel(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8, a: u8) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 || a == 0 {
            return;
        }
        let idx = ((y as u32 * self.width + x as u32) * 4) as usize;
        let alpha = a as f32 / 255.0;
        let inv_alpha = 1.0 - alpha;

        let prev_r = self.pixels[idx] as f32;
        let prev_g = self.pixels[idx + 1] as f32;
        let prev_b = self.pixels[idx + 2] as f32;
        let prev_a = self.pixels[idx + 3] as f32 / 255.0;

        let out_a = alpha + prev_a * inv_alpha;
        if out_a > 1e-4 {
            let out_r = (r as f32 * alpha + prev_r * prev_a * inv_alpha) / out_a;
            let out_g = (g as f32 * alpha + prev_g * prev_a * inv_alpha) / out_a;
            let out_b = (b as f32 * alpha + prev_b * prev_a * inv_alpha) / out_a;

            self.pixels[idx] = (out_r.clamp(0.0, 255.0)) as u8;
            self.pixels[idx + 1] = (out_g.clamp(0.0, 255.0)) as u8;
            self.pixels[idx + 2] = (out_b.clamp(0.0, 255.0)) as u8;
            self.pixels[idx + 3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
        }
    }

    pub fn draw_circle(&mut self, cx: f32, cy: f32, radius: f32, r: u8, g: u8, b: u8, a: u8) {
        let min_x = (cx - radius - 1.0).floor() as i32;
        let max_x = (cx + radius + 1.0).ceil() as i32;
        let min_y = (cy - radius - 1.0).floor() as i32;
        let max_y = (cy + radius + 1.0).ceil() as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let coverage = (radius + 0.5 - dist).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    self.blend_pixel(x, y, r, g, b, (a as f32 * coverage) as u8);
                }
            }
        }
    }

    pub fn draw_circle_stroke(&mut self, cx: f32, cy: f32, radius: f32, thickness: f32, r: u8, g: u8, b: u8, a: u8) {
        let half_t = thickness * 0.5;
        let outer_r = radius + half_t;
        let _inner_r = (radius - half_t).max(0.0);

        let min_x = (cx - outer_r - 1.0).floor() as i32;
        let max_x = (cx + outer_r + 1.0).ceil() as i32;
        let min_y = (cy - outer_r - 1.0).floor() as i32;
        let max_y = (cy + outer_r + 1.0).ceil() as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let d_from_mid = (dist - radius).abs();
                let coverage = (half_t + 0.5 - d_from_mid).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    self.blend_pixel(x, y, r, g, b, (a as f32 * coverage) as u8);
                }
            }
        }
    }

    pub fn draw_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, r: u8, g: u8, b: u8, a: u8) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-4 {
            self.draw_circle(x0, y0, thickness * 0.5, r, g, b, a);
            return;
        }

        let nx = -dy / len;
        let ny = dx / len;
        let half_t = thickness * 0.5;

        let min_x = x0.min(x1).floor() as i32 - (half_t + 1.0) as i32;
        let max_x = x0.max(x1).ceil() as i32 + (half_t + 1.0) as i32;
        let min_y = y0.min(y1).floor() as i32 - (half_t + 1.0) as i32;
        let max_y = y0.max(y1).ceil() as i32 + (half_t + 1.0) as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5 - x0;
                let py = y as f32 + 0.5 - y0;
                let proj = (px * dx + py * dy) / (len * len);
                if proj >= 0.0 && proj <= 1.0 {
                    let perp_dist = (px * nx + py * ny).abs();
                    let coverage = (half_t + 0.5 - perp_dist).clamp(0.0, 1.0);
                    if coverage > 0.0 {
                        self.blend_pixel(x, y, r, g, b, (a as f32 * coverage) as u8);
                    }
                } else if proj < 0.0 {
                    let dist = (px * px + py * py).sqrt();
                    let coverage = (half_t + 0.5 - dist).clamp(0.0, 1.0);
                    if coverage > 0.0 {
                        self.blend_pixel(x, y, r, g, b, (a as f32 * coverage) as u8);
                    }
                } else {
                    let dpx = x as f32 + 0.5 - x1;
                    let dpy = y as f32 + 0.5 - y1;
                    let dist = (dpx * dpx + dpy * dpy).sqrt();
                    let coverage = (half_t + 0.5 - dist).clamp(0.0, 1.0);
                    if coverage > 0.0 {
                        self.blend_pixel(x, y, r, g, b, (a as f32 * coverage) as u8);
                    }
                }
            }
        }
    }

    pub fn draw_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: u8, g: u8, b: u8, a: u8) {
        let min_x = x0.min(x1).floor() as i32;
        let max_x = x0.max(x1).ceil() as i32;
        let min_y = y0.min(y1).floor() as i32;
        let max_y = y0.max(y1).ceil() as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let cov_x = ((x1 - x as f32).min(x as f32 + 1.0 - x0)).clamp(0.0, 1.0);
                let cov_y = ((y1 - y as f32).min(y as f32 + 1.0 - y0)).clamp(0.0, 1.0);
                let cov = cov_x * cov_y;
                if cov > 0.0 {
                    self.blend_pixel(x, y, r, g, b, (a as f32 * cov) as u8);
                }
            }
        }
    }

    pub fn to_color_image(&self) -> ColorImage {
        ColorImage::from_rgba_unmultiplied([self.width as usize, self.height as usize], &self.pixels)
    }

    pub fn save_png(&self, path: &Path) -> bool {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Some(img) = image::RgbaImage::from_raw(self.width, self.height, self.pixels.clone()) {
            img.save(path).is_ok()
        } else {
            false
        }
    }
}

/// Collection of loaded UI icons as egui TextureHandles
pub struct AppIcons {
    pub brush: TextureHandle,
    pub eraser: TextureHandle,
    pub projection: TextureHandle,
    pub polygon_fill: TextureHandle,
    pub smudge: TextureHandle,
    pub clone: TextureHandle,
    pub picker: TextureHandle,
    pub shelf: TextureHandle,
    pub layer_paint: TextureHandle,
    pub layer_fill: TextureHandle,
    pub layer_folder: TextureHandle,
    pub layer_mask: TextureHandle,
    pub layer_eye: TextureHandle,
    pub shaders: TextureHandle,
    pub viewport_settings: TextureHandle,
    pub bake: TextureHandle,
}

impl AppIcons {
    /// Load all UI icons from assets/icons/ or generate them procedurally into the assets bundle
    pub fn load(ctx: &egui::Context) -> Self {
        let assets_dir = PathBuf::from("assets/icons");
        let _ = std::fs::create_dir_all(&assets_dir);

        let brush = Self::load_or_generate(ctx, &assets_dir, "tool_brush.png", generate_brush_icon);
        let eraser = Self::load_or_generate(ctx, &assets_dir, "tool_eraser.png", generate_eraser_icon);
        let projection = Self::load_or_generate(ctx, &assets_dir, "tool_projection.png", generate_projection_icon);
        let polygon_fill = Self::load_or_generate(ctx, &assets_dir, "tool_polygon_fill.png", generate_polygon_fill_icon);
        let smudge = Self::load_or_generate(ctx, &assets_dir, "tool_smudge.png", generate_smudge_icon);
        let clone = Self::load_or_generate(ctx, &assets_dir, "tool_clone.png", generate_clone_icon);
        let picker = Self::load_or_generate(ctx, &assets_dir, "tool_picker.png", generate_picker_icon);
        let shelf = Self::load_or_generate(ctx, &assets_dir, "tool_shelf.png", generate_shelf_icon);

        let layer_paint = Self::load_or_generate(ctx, &assets_dir, "layer_paint.png", generate_layer_paint_icon);
        let layer_fill = Self::load_or_generate(ctx, &assets_dir, "layer_fill.png", generate_layer_fill_icon);
        let layer_folder = Self::load_or_generate(ctx, &assets_dir, "layer_folder.png", generate_layer_folder_icon);
        let layer_mask = Self::load_or_generate(ctx, &assets_dir, "layer_mask.png", generate_layer_mask_icon);
        let layer_eye = Self::load_or_generate(ctx, &assets_dir, "layer_eye.png", generate_layer_eye_icon);

        let shaders = Self::load_or_generate(ctx, &assets_dir, "icon_shaders.png", generate_shaders_icon);
        let viewport_settings = Self::load_or_generate(ctx, &assets_dir, "icon_viewport_settings.png", generate_viewport_settings_icon);
        let bake = Self::load_or_generate(ctx, &assets_dir, "icon_bake.png", generate_bake_icon);

        Self {
            brush,
            eraser,
            projection,
            polygon_fill,
            smudge,
            clone,
            picker,
            shelf,
            layer_paint,
            layer_fill,
            layer_folder,
            layer_mask,
            layer_eye,
            shaders,
            viewport_settings,
            bake,
        }
    }

    fn load_or_generate<F>(ctx: &egui::Context, dir: &Path, filename: &str, gen_fn: F) -> TextureHandle
    where
        F: Fn() -> IconCanvas,
    {
        let file_path = dir.join(filename);
        if file_path.exists() {
            if let Ok(dyn_img) = image::open(&file_path) {
                let rgba = dyn_img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_img = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba.into_raw());
                return ctx.load_texture(filename, color_img, TextureOptions::LINEAR);
            }
        }


        // Generate procedurally, save to bundle, and load into egui
        let canvas = gen_fn();
        let _ = canvas.save_png(&file_path);
        let color_img = canvas.to_color_image();
        ctx.load_texture(filename, color_img, TextureOptions::LINEAR)
    }
}

// --- PROCEDURAL ICON GENERATORS (64x64 RGBA) ---

pub fn generate_brush_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Wooden Handle
    c.draw_line(14.0, 50.0, 32.0, 32.0, 7.0, 110, 85, 68, 255);
    c.draw_circle(14.0, 50.0, 3.5, 90, 70, 55, 255);
    // Metal Ferrule
    c.draw_line(30.0, 34.0, 38.0, 26.0, 7.5, 160, 170, 185, 255);
    c.draw_line(33.0, 31.0, 35.0, 29.0, 8.0, 190, 200, 215, 255);
    // Bristles Dipped in Substance Orange
    c.draw_line(38.0, 26.0, 48.0, 16.0, 6.5, 243, 94, 24, 255);
    c.draw_circle(49.0, 15.0, 3.2, 255, 161, 102, 255);
    // Wet Paint Droplet
    c.draw_circle(54.0, 24.0, 2.5, 243, 94, 24, 230);
    c.draw_line(53.0, 22.0, 54.0, 24.0, 1.8, 255, 161, 102, 240);
    c
}

pub fn generate_eraser_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Eraser Pink Rubber Block
    c.draw_line(18.0, 46.0, 34.0, 30.0, 14.0, 255, 117, 140, 255);
    // Eraser Top Tip Block
    c.draw_line(34.0, 30.0, 46.0, 18.0, 14.0, 220, 225, 235, 255);
    // Paper Sleeve Band in Blue
    c.draw_line(26.0, 38.0, 32.0, 32.0, 14.5, 58, 123, 213, 255);
    // Motion lines
    c.draw_line(8.0, 52.0, 18.0, 54.0, 2.0, 243, 94, 24, 240);
    c.draw_line(12.0, 56.0, 22.0, 58.0, 2.0, 243, 94, 24, 180);
    // Dust particles
    c.draw_circle(10.0, 44.0, 1.5, 230, 235, 245, 220);
    c.draw_circle(14.0, 48.0, 1.8, 230, 235, 245, 240);
    c
}

pub fn generate_projection_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // 3D Isometric Cube Outline
    // Top Face
    c.draw_line(34.0, 20.0, 52.0, 29.0, 2.0, 113, 128, 150, 255);
    c.draw_line(52.0, 29.0, 34.0, 38.0, 2.0, 113, 128, 150, 255);
    c.draw_line(34.0, 38.0, 16.0, 29.0, 2.0, 113, 128, 150, 255);
    c.draw_line(16.0, 29.0, 34.0, 20.0, 2.0, 113, 128, 150, 255);
    // Vertical Edges
    c.draw_line(16.0, 29.0, 16.0, 49.0, 2.0, 113, 128, 150, 255);
    c.draw_line(34.0, 38.0, 34.0, 58.0, 2.0, 113, 128, 150, 255);
    c.draw_line(52.0, 29.0, 52.0, 49.0, 2.0, 113, 128, 150, 255);
    // Bottom Edges
    c.draw_line(16.0, 49.0, 34.0, 58.0, 2.0, 113, 128, 150, 255);
    c.draw_line(34.0, 58.0, 52.0, 49.0, 2.0, 113, 128, 150, 255);
    // Projector Emitter Lens
    c.draw_circle(10.0, 12.0, 4.0, 243, 94, 24, 255);
    c.draw_line(8.0, 14.0, 18.0, 22.0, 3.5, 255, 161, 102, 230);
    // Projection Rays (Cyan)
    c.draw_line(18.0, 20.0, 30.0, 30.0, 1.5, 0, 210, 255, 220);
    c.draw_line(22.0, 24.0, 28.0, 42.0, 1.5, 0, 210, 255, 220);
    // Surface Stamping Ring
    c.draw_circle_stroke(32.0, 34.0, 5.0, 2.0, 243, 94, 24, 255);
    c
}

pub fn generate_polygon_fill_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Bucket Body
    c.draw_line(14.0, 24.0, 28.0, 10.0, 9.0, 80, 90, 105, 255);
    c.draw_line(20.0, 30.0, 34.0, 16.0, 9.0, 60, 70, 85, 255);
    // Pouring paint stream
    c.draw_line(18.0, 28.0, 30.0, 44.0, 4.0, 243, 94, 24, 255);
    // Polygon triangle grid
    c.draw_line(26.0, 56.0, 42.0, 40.0, 2.0, 113, 128, 150, 255);
    c.draw_line(42.0, 40.0, 56.0, 54.0, 2.0, 113, 128, 150, 255);
    c.draw_line(56.0, 54.0, 26.0, 56.0, 2.0, 113, 128, 150, 255);
    // Filled polygon face (Orange center)
    c.draw_circle(36.0, 46.0, 6.0, 243, 94, 24, 240);
    c.draw_circle(24.0, 38.0, 2.0, 255, 161, 102, 255);
    c
}

pub fn generate_smudge_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Finger/Stylus Body
    c.draw_line(16.0, 46.0, 36.0, 26.0, 8.0, 200, 210, 225, 255);
    c.draw_circle(38.0, 24.0, 4.2, 243, 94, 24, 255);
    // Smear Fluid Arc Waves
    c.draw_circle_stroke(36.0, 26.0, 12.0, 2.5, 243, 94, 24, 220);
    c.draw_circle_stroke(36.0, 26.0, 18.0, 1.8, 0, 210, 255, 180);
    c.draw_circle(50.0, 22.0, 2.0, 255, 161, 102, 255);
    c
}

pub fn generate_clone_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Stamp Knob Handle
    c.draw_circle(32.0, 14.0, 6.5, 140, 110, 90, 255);
    c.draw_circle(32.0, 12.0, 3.0, 180, 145, 120, 255);
    // Stamp Stem
    c.draw_line(32.0, 18.0, 32.0, 34.0, 6.0, 120, 95, 75, 255);
    // Stamp Base Plate
    c.draw_line(18.0, 42.0, 46.0, 42.0, 6.0, 55, 65, 80, 255);
    // Stamped Reticle Ring
    c.draw_circle_stroke(32.0, 54.0, 6.0, 2.0, 243, 94, 24, 255);
    c.draw_circle(32.0, 54.0, 1.8, 243, 94, 24, 255);
    c
}

pub fn generate_picker_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Rubber Bulb
    c.draw_circle(48.0, 14.0, 5.0, 243, 94, 24, 255);
    c.draw_line(45.0, 17.0, 41.0, 21.0, 6.0, 80, 90, 105, 255);
    // Glass Barrel
    c.draw_line(40.0, 22.0, 24.0, 38.0, 5.0, 220, 230, 245, 240);
    // Liquid Inside Barrel
    c.draw_line(34.0, 28.0, 26.0, 36.0, 3.0, 243, 94, 24, 255);
    // Pipette Tapered Tip
    c.draw_line(24.0, 38.0, 16.0, 46.0, 2.5, 180, 190, 205, 255);
    // Hanging Droplet
    c.draw_circle(14.0, 50.0, 2.5, 243, 94, 24, 255);
    // Target Reticle
    c.draw_circle_stroke(14.0, 54.0, 4.0, 1.2, 0, 210, 255, 220);
    c
}

pub fn generate_shelf_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // 2x2 Asset Grid Tiles
    // Top-Left Tile (Sphere)
    c.draw_rect(12.0, 12.0, 28.0, 28.0, 45, 55, 72, 255);
    c.draw_circle(20.0, 20.0, 5.0, 243, 94, 24, 255);
    c.draw_circle(18.0, 18.0, 1.5, 255, 161, 102, 255);
    // Top-Right Tile
    c.draw_rect(36.0, 12.0, 52.0, 28.0, 45, 55, 72, 255);
    c.draw_circle(44.0, 20.0, 4.0, 200, 210, 225, 255);
    // Bottom-Left Tile
    c.draw_rect(12.0, 32.0, 28.0, 48.0, 45, 55, 72, 255);
    c.draw_circle(20.0, 40.0, 4.5, 0, 210, 255, 255);
    // Bottom-Right Tile
    c.draw_rect(36.0, 32.0, 52.0, 48.0, 45, 55, 72, 255);
    c.draw_circle(44.0, 40.0, 4.0, 243, 94, 24, 255);
    // Shelf Ledge Underneath
    c.draw_line(8.0, 54.0, 56.0, 54.0, 4.0, 243, 94, 24, 255);
    c
}

pub fn generate_layer_paint_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Stacked Back Sheet
    c.draw_rect(18.0, 12.0, 50.0, 26.0, 55, 65, 85, 255);
    // Front Document Sheet
    c.draw_rect(10.0, 24.0, 44.0, 42.0, 30, 40, 58, 255);
    // Paintbrush Emblem
    c.draw_line(30.0, 52.0, 44.0, 38.0, 5.0, 130, 100, 80, 255);
    c.draw_line(42.0, 40.0, 50.0, 32.0, 5.0, 243, 94, 24, 255);
    c.draw_circle(52.0, 30.0, 2.5, 255, 161, 102, 255);
    c
}

pub fn generate_layer_fill_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Stacked Back Sheet
    c.draw_rect(18.0, 12.0, 50.0, 26.0, 55, 65, 85, 255);
    // Front Document Sheet
    c.draw_rect(10.0, 24.0, 44.0, 42.0, 30, 40, 58, 255);
    // Fill Bucket Emblem
    c.draw_line(40.0, 40.0, 50.0, 30.0, 7.0, 75, 85, 105, 255);
    c.draw_circle(42.0, 42.0, 3.5, 243, 94, 24, 255);
    c.draw_circle(52.0, 52.0, 4.5, 243, 94, 24, 255);
    c
}

pub fn generate_layer_folder_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Back Folder Tab
    c.draw_rect(12.0, 14.0, 30.0, 24.0, 70, 85, 105, 255);
    c.draw_rect(12.0, 20.0, 52.0, 46.0, 55, 68, 88, 255);
    // Front Folder Flap (Open)
    c.draw_rect(8.0, 28.0, 56.0, 50.0, 30, 42, 60, 255);
    // Folder Line Indicator
    c.draw_line(24.0, 38.0, 40.0, 38.0, 2.5, 243, 94, 24, 255);
    c.draw_circle(40.0, 38.0, 2.5, 243, 94, 24, 255);
    c
}

pub fn generate_layer_mask_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Outer Dark Mask Card
    c.draw_rect(12.0, 12.0, 52.0, 52.0, 30, 42, 60, 255);
    c.draw_circle_stroke(32.0, 32.0, 16.0, 2.0, 243, 94, 24, 255);
    // Inner White Mask Aperture
    c.draw_circle(32.0, 32.0, 13.0, 255, 255, 255, 255);
    // Subtle inner half tone
    c.draw_rect(32.0, 19.0, 45.0, 45.0, 200, 210, 225, 255);
    c
}

pub fn generate_layer_eye_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Outer Eye Contour
    c.draw_circle_stroke(32.0, 32.0, 18.0, 3.0, 210, 220, 235, 255);
    // Iris Circle (Orange)
    c.draw_circle(32.0, 32.0, 9.0, 243, 94, 24, 255);
    // Pupil
    c.draw_circle(32.0, 32.0, 4.5, 15, 23, 42, 255);
    // Specular Highlight
    c.draw_circle(35.0, 29.0, 2.0, 255, 255, 255, 255);
    c
}

pub fn generate_shaders_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Shader Ball Sphere
    c.draw_circle(30.0, 34.0, 17.0, 120, 45, 18, 255);
    c.draw_circle(28.0, 32.0, 14.0, 243, 94, 24, 255);
    c.draw_circle(24.0, 28.0, 7.0, 255, 161, 102, 255);
    c.draw_circle(22.0, 26.0, 3.0, 255, 255, 255, 255);
    // Sparkling Star Glints (Cyan and White)
    c.draw_line(46.0, 12.0, 46.0, 26.0, 2.5, 0, 210, 255, 255);
    c.draw_line(39.0, 19.0, 53.0, 19.0, 2.5, 0, 210, 255, 255);
    c.draw_circle(46.0, 19.0, 2.2, 255, 255, 255, 255);
    c
}

pub fn generate_viewport_settings_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // Mechanical Gear Teeth (8 spokes)
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::PI / 4.0;
        let x0 = 32.0 + angle.cos() * 12.0;
        let y0 = 32.0 + angle.sin() * 12.0;
        let x1 = 32.0 + angle.cos() * 22.0;
        let y1 = 32.0 + angle.sin() * 22.0;
        c.draw_line(x0, y0, x1, y1, 6.0, 140, 155, 180, 255);
    }
    // Gear Outer Rim
    c.draw_circle_stroke(32.0, 32.0, 17.0, 5.0, 120, 135, 160, 255);
    // Gear Dark Recess
    c.draw_circle(32.0, 32.0, 13.0, 30, 40, 58, 255);
    // Center Camera Aperture (Orange)
    c.draw_circle(32.0, 32.0, 7.0, 243, 94, 24, 255);
    c.draw_circle(32.0, 32.0, 3.0, 15, 23, 42, 255);
    c.draw_circle(33.5, 30.5, 1.2, 255, 255, 255, 255);
    c
}

pub fn generate_bake_icon() -> IconCanvas {
    let mut c = IconCanvas::new(64, 64);
    // High-Poly Surface Wave (Upper Orange)
    c.draw_circle_stroke(20.0, 26.0, 10.0, 3.0, 243, 94, 24, 255);
    c.draw_circle_stroke(44.0, 26.0, 10.0, 3.0, 243, 94, 24, 255);
    c.draw_rect(10.0, 26.0, 54.0, 36.0, 0, 0, 0, 0); // Clear lower halves
    // Low-Poly Surface Flat Line (Lower Silver)
    c.draw_line(10.0, 48.0, 54.0, 48.0, 3.5, 160, 175, 195, 255);
    // Baking Rays (Cyan)
    c.draw_line(18.0, 20.0, 18.0, 44.0, 2.0, 0, 210, 255, 255);
    c.draw_line(32.0, 24.0, 32.0, 44.0, 2.0, 0, 210, 255, 255);
    c.draw_line(46.0, 20.0, 46.0, 44.0, 2.0, 0, 210, 255, 255);
    // Ray Hit Dots
    c.draw_circle(18.0, 18.0, 2.5, 255, 161, 102, 255);
    c.draw_circle(32.0, 22.0, 2.5, 255, 161, 102, 255);
    c.draw_circle(46.0, 18.0, 2.5, 255, 161, 102, 255);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_draw_and_save() {
        let mut canvas = IconCanvas::new(32, 32);
        canvas.draw_circle(16.0, 16.0, 10.0, 255, 128, 0, 255);
        let color_img = canvas.to_color_image();
        assert_eq!(color_img.width(), 32);
        assert_eq!(color_img.height(), 32);
    }

    #[test]
    fn test_generators_produce_valid_pixels() {
        let b = generate_brush_icon();
        assert_eq!(b.pixels.len(), 64 * 64 * 4);
        assert!(b.pixels.iter().any(|&p| p > 0));

        let s = generate_shaders_icon();
        assert_eq!(s.pixels.len(), 64 * 64 * 4);
        assert!(s.pixels.iter().any(|&p| p > 0));
    }

    #[test]
    fn test_export_all_png_icons_to_disk() {
        let dir = PathBuf::from("assets/icons");
        let _ = std::fs::create_dir_all(&dir);

        let icons = [
            ("tool_brush.png", generate_brush_icon()),
            ("tool_eraser.png", generate_eraser_icon()),
            ("tool_projection.png", generate_projection_icon()),
            ("tool_polygon_fill.png", generate_polygon_fill_icon()),
            ("tool_smudge.png", generate_smudge_icon()),
            ("tool_clone.png", generate_clone_icon()),
            ("tool_picker.png", generate_picker_icon()),
            ("tool_shelf.png", generate_shelf_icon()),
            ("layer_paint.png", generate_layer_paint_icon()),
            ("layer_fill.png", generate_layer_fill_icon()),
            ("layer_folder.png", generate_layer_folder_icon()),
            ("layer_mask.png", generate_layer_mask_icon()),
            ("layer_eye.png", generate_layer_eye_icon()),
            ("icon_shaders.png", generate_shaders_icon()),
            ("icon_viewport_settings.png", generate_viewport_settings_icon()),
            ("icon_bake.png", generate_bake_icon()),
        ];

        for (filename, canvas) in icons {
            let path = dir.join(filename);
            assert!(canvas.save_png(&path), "Failed to save icon {}", filename);
            assert!(path.exists());
        }
    }
}
