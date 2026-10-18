// 3D Paintcraft - Industry Brush Engine, Alphas, Jitter, Masks & Symmetry
// Pure safe Rust

use crate::channel::ChannelFlags;
use crate::layer::Layer;
use crate::utils::NoiseGenerator;
use glam::{Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolMode {
    Brush,      // Paint (1 / B)
    Eraser,     // Eraser (2 / E)
    Projection, // Projection (3)
    FillBucket, // Polygon Fill / Bucket (4 / G)
    Smudge,     // Smudge (5)
    Clone,      // Clone (6)
    Eyedropper, // Material Picker / Eyedropper (P / I)
}

impl ToolMode {
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Brush => "Paint Brush [1/B]",
            Self::Eraser => "Eraser [2/E]",
            Self::Projection => "Projection [3]",
            Self::FillBucket => "Fill Bucket [4/G]",
            Self::Smudge => "Smudge [5]",
            Self::Clone => "Clone [6]",
            Self::Eyedropper => "Material Picker [P/I]",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaintTarget {
    Material, // Paints active channels (Color, Roughness, Metallic, etc.)
    Mask,     // Paints directly onto the active layer's mask (Black/White)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlphaType {
    SoftRound,     // Hermite Gaussian smooth falloff
    HardRound,     // Sharp geometric edge
    NoiseSplatter, // Procedural organic grunge splatter
    Chisel,        // Directional square / chisel shape
    Stipple,       // Spray noise stipple
    Scratches,     // Fine fibrous scratches
}

impl AlphaType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SoftRound => "Soft Round",
            Self::HardRound => "Hard Round",
            Self::NoiseSplatter => "Grunge Splatter",
            Self::Chisel => "Chisel Square",
            Self::Stipple => "Stipple Spray",
            Self::Scratches => "Micro Scratches",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolygonFillMode {
    Triangle,
    Polygon,
    UvIsland,
    MeshElement,
}

impl PolygonFillMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Triangle => "Triangle",
            Self::Polygon => "Polygon (Quad)",
            Self::UvIsland => "UV Island",
            Self::MeshElement => "Mesh Element",
        }
    }
}

/// Stencil / Projection painting settings matching Substance 3D Painter
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StencilSettings {
    pub enabled: bool,
    pub pos: Vec2,
    pub scale: f32,
    pub angle: f32,
    pub opacity: f32,
    pub mask_only: bool,
}

impl Default for StencilSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            pos: Vec2::ZERO,
            scale: 1.0,
            angle: 0.0,
            opacity: 0.5,
            mask_only: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrushSettings {
    pub mode: ToolMode,
    pub target: PaintTarget,
    pub alpha: AlphaType,
    pub radius: f32,       // Pixel radius (1.0 - 200.0)
    pub hardness: f32,     // 0.0 (soft) to 1.0 (hard edge)
    pub opacity: f32,      // 0.0 to 1.0
    pub flow: f32,         // 0.0 to 1.0
    pub spacing: f32,      // Fractional spacing (e.g. 0.15 of radius)

    // Jitter dynamics
    pub size_jitter: f32,
    pub angle_jitter: f32,

    // Stroke Stabilization
    pub lazy_mouse_enabled: bool,
    pub lazy_mouse_radius: f32, // fraction of screen or distance
    pub position_jitter: f32,

    // Follow Path / Directional Tangent Rotation
    #[serde(default)]
    pub follow_path: bool,
    #[serde(default)]
    pub follow_path_offset: f32,

    // Tablet Stylus Dynamics
    pub pressure_size: bool,
    pub pressure_opacity: bool,

    // Material to paint with
    pub paint_color: Vec4,
    pub paint_roughness: f32,
    pub paint_metallic: f32,
    pub paint_height: f32,
    pub paint_emissive: Vec4,
    pub paint_opacity: f32,
    pub paint_clearcoat: f32,
    pub paint_anisotropy: f32,
    pub paint_anisotropy_angle: f32,

    // Polygon Fill Settings
    pub polygon_fill_mode: PolygonFillMode,
    pub polygon_fill_value: f32,

    // Mask value (for PaintTarget::Mask)
    pub mask_paint_value: f32, // 1.0 = reveal (white), 0.0 = conceal (black)

    pub channels: ChannelFlags,
    pub symmetry_x: bool,
    pub symmetry_y: bool,
    pub symmetry_z: bool,
    pub symmetry_center: Vec3,

    // Stencil / Projection Settings
    pub stencil: StencilSettings,

    // Smudge Settings
    #[serde(default = "default_smudge_strength")]
    pub smudge_strength: f32,

    // Clone Stamp Settings
    #[serde(default)]
    pub clone_source_uv: Option<Vec2>,
}

fn default_smudge_strength() -> f32 {
    0.5
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            mode: ToolMode::Brush,
            target: PaintTarget::Material,
            alpha: AlphaType::SoftRound,
            radius: 26.0,
            hardness: 0.75,
            opacity: 1.0,
            flow: 1.0,
            spacing: 0.12,
            size_jitter: 0.0,
            angle_jitter: 0.0,
            lazy_mouse_enabled: false,
            lazy_mouse_radius: 0.05,
            position_jitter: 0.0,
            follow_path: false,
            follow_path_offset: 0.0,
            pressure_size: false,
            pressure_opacity: false,
            paint_color: Vec4::new(0.88, 0.22, 0.18, 1.0),
            paint_roughness: 0.35,
            paint_metallic: 0.0,
            paint_height: 0.5,
            paint_emissive: Vec4::new(0.0, 0.0, 0.0, 1.0),
            paint_opacity: 1.0,
            paint_clearcoat: 0.0,
            paint_anisotropy: 0.0,
            paint_anisotropy_angle: 0.0,
            polygon_fill_mode: PolygonFillMode::Triangle,
            polygon_fill_value: 1.0,
            mask_paint_value: 1.0,
            channels: ChannelFlags::default(),
            symmetry_x: false,
            symmetry_y: false,
            symmetry_z: false,
            symmetry_center: Vec3::ZERO,
            stencil: StencilSettings::default(),
            smudge_strength: 0.5,
            clone_source_uv: None,
        }
    }
}

pub struct BrushEngine {
    pub settings: BrushSettings,
    pub last_uv: Option<Vec2>,
    pub stroke_counter: u32,
    
    // Internal stabilization state
    pub lazy_uv: Option<Vec2>,
    pub lazy_screen_pos: Option<[f32; 2]>,
    pub last_hit_pos: Option<Vec3>,
    pub last_hit_normal: Option<Vec3>,
    
    // Precomputed brush tip cache for massive CPU performance boost
    pub cached_tip: Vec<f32>,
    pub cached_tip_radius: f32,
    pub cached_tip_alpha: AlphaType,
    pub cached_tip_hardness: f32,

    // Stencil projection image buffer
    pub stencil_buffer: Option<crate::channel::RgbaBuffer>,

    // Smudge tool state
    pub smudge_color: Option<Vec4>,
    pub smudge_roughness: Option<f32>,
    pub smudge_metallic: Option<f32>,
    pub smudge_height: Option<f32>,
    pub smudge_emissive: Option<Vec4>,

    // Clone stamp tool state
    pub clone_anchor_uv: Option<Vec2>,

    // Dynamic Follow Path tangent angle
    pub follow_path_angle: f32,
}

impl BrushEngine {
    pub fn new() -> Self {
        Self {
            settings: BrushSettings::default(),
            last_uv: None,
            stroke_counter: 0,
            lazy_uv: None,
            lazy_screen_pos: None,
            last_hit_pos: None,
            last_hit_normal: None,
            cached_tip: Vec::new(),
            cached_tip_radius: -1.0,
            cached_tip_alpha: AlphaType::SoftRound,
            cached_tip_hardness: -1.0,
            stencil_buffer: None,
            smudge_color: None,
            smudge_roughness: None,
            smudge_metallic: None,
            smudge_height: None,
            smudge_emissive: None,
            clone_anchor_uv: None,
            follow_path_angle: 0.0,
        }
    }

    /// Sample the stencil texture given normalized screen UV coordinates (0.0 to 1.0)
    pub fn sample_stencil(&self, screen_uv: Vec2) -> Option<Vec4> {
        if let Some(ref buf) = self.stencil_buffer {
            let st = &self.settings.stencil;
            if !st.enabled || buf.width == 0 || buf.height == 0 {
                return None;
            }
            let centered = screen_uv - Vec2::splat(0.5) - st.pos;
            let (sin_a, cos_a) = (-st.angle).sin_cos();
            let rotated = Vec2::new(
                centered.x * cos_a - centered.y * sin_a,
                centered.x * sin_a + centered.y * cos_a,
            );
            let scaled = rotated / st.scale.max(0.01) + Vec2::splat(0.5);
            if scaled.x >= 0.0 && scaled.x <= 1.0 && scaled.y >= 0.0 && scaled.y <= 1.0 {
                Some(buf.sample_bilinear(scaled))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Reset stroke state when mouse is released
    pub fn update_cache(&mut self) {
        if (self.cached_tip_radius - self.settings.radius).abs() < 0.1 
            && self.cached_tip_alpha == self.settings.alpha
            && (self.cached_tip_hardness - self.settings.hardness).abs() < 0.01 {
            return;
        }

        self.cached_tip_radius = self.settings.radius;
        self.cached_tip_alpha = self.settings.alpha;
        self.cached_tip_hardness = self.settings.hardness;

        let rad = self.settings.radius.ceil() as i32;
        let dim = (rad * 2 + 1) as usize;
        self.cached_tip.resize(dim * dim, 0.0);

        let h_param = self.settings.hardness.clamp(0.001, 0.999);
        let soft_radius = self.settings.radius * h_param;
        let falloff_range = self.settings.radius - soft_radius;

        for y in -rad..=rad {
            for x in -rad..=rad {
                let dist = ((x * x + y * y) as f32).sqrt();
                let alpha = if dist > self.settings.radius {
                    0.0
                } else if dist < soft_radius {
                    1.0
                } else {
                    let t = (dist - soft_radius) / falloff_range;
                    // Hermite interpolation (smoothstep)
                    1.0 - (t * t * (3.0 - 2.0 * t))
                };

                let final_alpha = match self.settings.alpha {
                    AlphaType::SoftRound => alpha,
                    AlphaType::HardRound => if dist <= self.settings.radius { 1.0 } else { 0.0 },
                    AlphaType::NoiseSplatter => {
                        let nx = x as f32 * 0.15;
                        let ny = y as f32 * 0.15;
                        let n = NoiseGenerator::value_noise_2d(nx, ny);
                        if n > 0.4 { alpha } else { 0.0 }
                    }
                    AlphaType::Chisel => {
                        let ax = x.abs() as f32;
                        let ay = y.abs() as f32;
                        if ax < self.settings.radius && ay < self.settings.radius * 0.2 { 1.0 } else { 0.0 }
                    }
                    AlphaType::Stipple => {
                        let n = NoiseGenerator::value_noise_2d(x as f32 * 0.5, y as f32 * 0.5);
                        if n > 0.7 { alpha } else { 0.0 }
                    }
                    AlphaType::Scratches => {
                        let n = NoiseGenerator::value_noise_2d(x as f32 * 0.8, y as f32 * 0.1);
                        if n > 0.8 { alpha } else { 0.0 }
                    }
                };

                let ty = (y + rad) as usize;
                let tx = (x + rad) as usize;
                self.cached_tip[ty * dim + tx] = final_alpha;
            }
        }
    }

    pub fn end_stroke(&mut self) {
        self.last_uv = None;
        self.lazy_uv = None;
        self.lazy_screen_pos = None;
        self.last_hit_pos = None;
        self.last_hit_normal = None;
        self.smudge_color = None;
        self.smudge_roughness = None;
        self.smudge_metallic = None;
        self.smudge_height = None;
        self.smudge_emissive = None;
        self.clone_anchor_uv = None;
        self.follow_path_angle = 0.0;
    }

    /// Paint continuous stroke from previous UV to current UV
    pub fn paint_stroke(&mut self, layer: &mut Layer, mouse_uv: Vec2) {
        self.update_cache();
        // Evaluate Lazy Mouse
        let current_uv = if self.settings.lazy_mouse_enabled {
            if let Some(luv) = self.lazy_uv {
                let dist = (mouse_uv - luv).length();
                if dist > self.settings.lazy_mouse_radius {
                    let dir = (mouse_uv - luv).normalize();
                    let new_luv = luv + dir * (dist - self.settings.lazy_mouse_radius);
                    self.lazy_uv = Some(new_luv);
                    new_luv
                } else {
                    // Not pulled enough, do not stamp
                    return;
                }
            } else {
                self.lazy_uv = Some(mouse_uv);
                mouse_uv
            }
        } else {
            mouse_uv
        };

        let uv_list = match self.last_uv {
            Some(prev_uv) => {
                let delta = current_uv - prev_uv;
                if self.settings.follow_path && delta.length_squared() > 1e-9 {
                    let raw_angle = delta.y.atan2(delta.x);
                    self.follow_path_angle = raw_angle + self.settings.follow_path_offset;
                }
                let dist = delta.length();
                let tex_dim = layer.base_color_buf.width as f32;
                let pixel_dist = dist * tex_dim;
                let step_size = (self.settings.radius * self.settings.spacing).max(1.0);
                let steps = ((pixel_dist / step_size).ceil() as usize).clamp(1, 16);

                if steps > 1 {
                    let mut interpolated = Vec::with_capacity(steps);
                    for i in 1..=steps {
                        let t = i as f32 / steps as f32;
                        interpolated.push(prev_uv.lerp(current_uv, t));
                    }
                    interpolated
                } else {
                    vec![current_uv]
                }
            }
            None => vec![current_uv],
        };

        for uv in uv_list {
            self.stroke_counter = self.stroke_counter.wrapping_add(1);
            self.stamp_uv(layer, uv);

            // Symmetry painting
            if self.settings.symmetry_x {
                self.stamp_uv(layer, Vec2::new(1.0 - uv.x, uv.y));
            }
            if self.settings.symmetry_y {
                self.stamp_uv(layer, Vec2::new(uv.x, 1.0 - uv.y));
            }
            if self.settings.symmetry_x && self.settings.symmetry_y {
                self.stamp_uv(layer, Vec2::new(1.0 - uv.x, 1.0 - uv.y));
            }
        }

        self.last_uv = Some(current_uv);
    }

    /// Single brush stamp centered at UV coordinates
    pub fn stamp_uv(&mut self, layer: &mut Layer, uv: Vec2) {
        let w = layer.base_color_buf.width as i32;
        let h = layer.base_color_buf.height as i32;

        let mut target_uv = uv;
        if self.settings.position_jitter > 0.001 {
            let jx = (NoiseGenerator::value_noise_2d(self.stroke_counter as f32 * 1.7, 13.0) - 0.5) * self.settings.position_jitter * 0.05;
            let jy = (NoiseGenerator::value_noise_2d(self.stroke_counter as f32 * 2.3, 47.0) - 0.5) * self.settings.position_jitter * 0.05;
            target_uv += Vec2::new(jx, jy);
        }

        let center_x = (target_uv.x.rem_euclid(1.0) * (w as f32)).round() as i32;
        let center_y = (target_uv.y.rem_euclid(1.0) * (h as f32)).round() as i32;

        let rad = self.cached_tip_radius.ceil() as i32;
        let dim = (rad * 2 + 1) as i32;
        
        let flow = self.settings.flow * self.settings.opacity;

        let min_x = (center_x - rad).max(0);
        let max_x = (center_x + rad).min(w - 1);
        let min_y = (center_y - rad).max(0);
        let max_y = (center_y + rad).min(h - 1);

        let is_eraser = self.settings.mode == ToolMode::Eraser;
        let is_smudge = self.settings.mode == ToolMode::Smudge;
        let is_clone = self.settings.mode == ToolMode::Clone;
        let is_mask_target = self.settings.target == PaintTarget::Mask;
        
        let (paint_buf_ptr, paint_alpha_ptr) = if is_mask_target {
            layer.has_mask = true;
            if !layer.mask_effects.is_empty() {
                let eff = layer.get_or_create_active_mask_paint();
                match &mut eff.effect_type {
                    crate::layer::MaskEffectType::Paint { buffer, alpha } => {
                        if alpha.data.len() < (w * h) as usize {
                            alpha.data.resize((w * h) as usize, 0.0);
                        }
                        if buffer.data.len() < (w * h) as usize {
                            buffer.data.resize((w * h) as usize, 1.0);
                        }
                        (buffer.data.as_mut_ptr(), alpha.data.as_mut_ptr())
                    }
                    _ => (std::ptr::null_mut(), std::ptr::null_mut()),
                }
            } else {
                (std::ptr::null_mut(), std::ptr::null_mut())
            }
        } else {
            (std::ptr::null_mut(), std::ptr::null_mut())
        };

        // Fast paths and inline indexing for max speed!
        let base_col_target = self.settings.paint_color;
        let r_target = self.settings.paint_roughness;
        let m_target = self.settings.paint_metallic;
        let h_target = self.settings.paint_height;
        let emis_target = self.settings.paint_emissive;
        let opac_target = self.settings.paint_opacity;
        let clearcoat_target = self.settings.paint_clearcoat;
        let aniso_target = self.settings.paint_anisotropy;
        let aniso_ang_target = self.settings.paint_anisotropy_angle;
        
        let do_color = self.settings.channels.base_color;
        let do_r = self.settings.channels.roughness;
        let do_m = self.settings.channels.metallic;
        let do_h = self.settings.channels.height;
        let do_emis = self.settings.channels.emission;
        let do_opac = self.settings.channels.opacity;
        let do_clearcoat = self.settings.channels.clearcoat;
        let do_aniso = self.settings.channels.anisotropy;

        // Smudge carrier update
        if is_smudge {
            let cx = center_x.clamp(0, w - 1) as usize;
            let cy = center_y.clamp(0, h - 1) as usize;
            let center_idx = cy * (w as usize) + cx;
            let cur_col = layer.base_color_buf.data[center_idx];
            let cur_r = layer.roughness_buf.data[center_idx];
            let cur_m = layer.metallic_buf.data[center_idx];
            let cur_h = layer.height_buf.data[center_idx];
            let cur_emis = layer.emissive_buf.data[center_idx];

            let str_val = self.settings.smudge_strength.clamp(0.05, 1.0);
            if let Some(sc) = self.smudge_color {
                self.smudge_color = Some(sc.lerp(cur_col, str_val * 0.4));
            } else {
                self.smudge_color = Some(cur_col);
            }
            if let Some(sr) = self.smudge_roughness {
                self.smudge_roughness = Some(sr * (1.0 - str_val * 0.4) + cur_r * (str_val * 0.4));
            } else {
                self.smudge_roughness = Some(cur_r);
            }
            if let Some(sm) = self.smudge_metallic {
                self.smudge_metallic = Some(sm * (1.0 - str_val * 0.4) + cur_m * (str_val * 0.4));
            } else {
                self.smudge_metallic = Some(cur_m);
            }
            if let Some(sh) = self.smudge_height {
                self.smudge_height = Some(sh * (1.0 - str_val * 0.4) + cur_h * (str_val * 0.4));
            } else {
                self.smudge_height = Some(cur_h);
            }
            if let Some(se) = self.smudge_emissive {
                self.smudge_emissive = Some(se.lerp(cur_emis, str_val * 0.4));
            } else {
                self.smudge_emissive = Some(cur_emis);
            }
        }

        // Clone Stamp anchor and offset update
        let clone_source_center = if is_clone {
            if let Some(src_uv) = self.settings.clone_source_uv {
                if self.clone_anchor_uv.is_none() {
                    self.clone_anchor_uv = Some(uv);
                }
                let uv_offset = uv - self.clone_anchor_uv.unwrap();
                let sample_uv = (src_uv + uv_offset).rem_euclid(Vec2::splat(1.0));
                let scx = (sample_uv.x * (w as f32)).round() as i32;
                let scy = (sample_uv.y * (h as f32)).round() as i32;
                Some((scx, scy))
            } else {
                None
            }
        } else {
            None
        };

        let rect_min_x = min_x as u32;
        let rect_max_x = max_x as u32;
        let rect_min_y = min_y as u32;
        let rect_max_y = max_y as u32;

        if let Some((dx0, dy0, dx1, dy1)) = layer.dirty_rect {
            layer.dirty_rect = Some((
                dx0.min(rect_min_x), dy0.min(rect_min_y), dx1.max(rect_max_x), dy1.max(rect_max_y)
            ));
        } else {
            layer.dirty_rect = Some((rect_min_x, rect_min_y, rect_max_x, rect_max_y));
        }

        let buf_w = w as usize;

        let active_angle = if self.settings.follow_path {
            self.follow_path_angle
        } else {
            0.0
        } + if self.settings.angle_jitter > 0.001 {
            (NoiseGenerator::value_noise_2d(self.stroke_counter as f32 * 3.1, 71.0) - 0.5) * 2.0 * std::f32::consts::PI * self.settings.angle_jitter
        } else {
            0.0
        };

        let has_rot = active_angle.abs() > 1e-4;
        let (sin_a, cos_a) = if has_rot { active_angle.sin_cos() } else { (0.0, 1.0) };

        for py in min_y..=max_y {
            let rel_y = (py - center_y) as f32;
            let tip_y_direct = (py - center_y + rad) as usize;
            let buf_y_offset = (py as usize) * buf_w;
            for px in min_x..=max_x {
                let alpha_val = if has_rot {
                    let rel_x = (px - center_x) as f32;
                    let rot_x = rel_x * cos_a + rel_y * sin_a;
                    let rot_y = -rel_x * sin_a + rel_y * cos_a;
                    let tip_x = (rot_x + rad as f32).round() as i32;
                    let tip_y = (rot_y + rad as f32).round() as i32;
                    if tip_x >= 0 && tip_x < dim as i32 && tip_y >= 0 && tip_y < dim as i32 {
                        self.cached_tip[tip_y as usize * (dim as usize) + tip_x as usize]
                    } else {
                        0.0
                    }
                } else {
                    let tip_x_direct = (px - center_x + rad) as usize;
                    self.cached_tip[tip_y_direct * (dim as usize) + tip_x_direct]
                };
                
                let weight = (alpha_val * flow).clamp(0.0, 1.0);
                if weight <= 1e-4 { continue; }

                let idx = buf_y_offset + (px as usize);

                let (eff_base_col, eff_r, eff_m, eff_h, eff_emis) = if let Some((scx, scy)) = clone_source_center {
                    let src_x = (scx + (px - center_x)).rem_euclid(w) as usize;
                    let src_y = (scy + (py - center_y)).rem_euclid(h) as usize;
                    let src_idx = src_y * (w as usize) + src_x;
                    (
                        layer.base_color_buf.data[src_idx],
                        layer.roughness_buf.data[src_idx],
                        layer.metallic_buf.data[src_idx],
                        layer.height_buf.data[src_idx],
                        layer.emissive_buf.data[src_idx],
                    )
                } else if is_smudge {
                    (
                        self.smudge_color.unwrap_or(base_col_target),
                        self.smudge_roughness.unwrap_or(r_target),
                        self.smudge_metallic.unwrap_or(m_target),
                        self.smudge_height.unwrap_or(h_target),
                        self.smudge_emissive.unwrap_or(emis_target),
                    )
                } else {
                    (base_col_target, r_target, m_target, h_target, emis_target)
                };

                unsafe {
                    if is_mask_target {
                        let target_val = if is_eraser { 0.0 } else { self.settings.mask_paint_value };
                        let cur_m = *layer.mask_buf.data.get_unchecked(idx);
                        *layer.mask_buf.data.get_unchecked_mut(idx) = cur_m * (1.0 - weight) + target_val * weight;
                        if !paint_buf_ptr.is_null() {
                            if is_eraser {
                                let cur_a = *paint_alpha_ptr.add(idx);
                                *paint_alpha_ptr.add(idx) = (cur_a * (1.0 - weight)).max(0.0);
                            } else {
                                let cur_p = *paint_buf_ptr.add(idx);
                                *paint_buf_ptr.add(idx) = cur_p * (1.0 - weight) + target_val * weight;
                                let cur_a = *paint_alpha_ptr.add(idx);
                                *paint_alpha_ptr.add(idx) = (cur_a + weight * (1.0 - cur_a)).clamp(0.0, 1.0);
                            }
                        }
                    } else if is_eraser {
                        let cur_col = *layer.base_color_buf.data.get_unchecked(idx);
                        let new_a = (cur_col.w * (1.0 - weight)).max(0.0);
                        layer.base_color_buf.data.get_unchecked_mut(idx).w = new_a;
                        let cur_h = *layer.height_buf.data.get_unchecked(idx);
                        *layer.height_buf.data.get_unchecked_mut(idx) = cur_h * (1.0 - weight) + 0.5 * weight;
                        let cur_emis = *layer.emissive_buf.data.get_unchecked(idx);
                        *layer.emissive_buf.data.get_unchecked_mut(idx) = cur_emis * (1.0 - weight);
                        let cur_opac = *layer.opacity_buf.data.get_unchecked(idx);
                        *layer.opacity_buf.data.get_unchecked_mut(idx) = cur_opac * (1.0 - weight) + 1.0 * weight;
                        let cur_cc = *layer.clearcoat_buf.data.get_unchecked(idx);
                        *layer.clearcoat_buf.data.get_unchecked_mut(idx) = cur_cc * (1.0 - weight);
                        let cur_aniso = *layer.anisotropy_buf.data.get_unchecked(idx);
                        *layer.anisotropy_buf.data.get_unchecked_mut(idx) = cur_aniso * (1.0 - weight);
                        let cur_ang = *layer.anisotropy_angle_buf.data.get_unchecked(idx);
                        *layer.anisotropy_angle_buf.data.get_unchecked_mut(idx) = cur_ang * (1.0 - weight);
                    } else {
                        let cur_col = *layer.base_color_buf.data.get_unchecked(idx);
                        let alpha = (cur_col.w + weight * (1.0 - cur_col.w)).clamp(0.0, 1.0);

                        if do_color {
                            let blended = cur_col.truncate() * (1.0 - weight) + eff_base_col.truncate() * weight;
                            *layer.base_color_buf.data.get_unchecked_mut(idx) = Vec4::new(blended.x, blended.y, blended.z, alpha);
                        } else {
                            layer.base_color_buf.data.get_unchecked_mut(idx).w = alpha;
                        }

                        if do_r {
                            let cur_r = *layer.roughness_buf.data.get_unchecked(idx);
                            *layer.roughness_buf.data.get_unchecked_mut(idx) = cur_r * (1.0 - weight) + eff_r * weight;
                        }
                        if do_m {
                            let cur_m = *layer.metallic_buf.data.get_unchecked(idx);
                            *layer.metallic_buf.data.get_unchecked_mut(idx) = cur_m * (1.0 - weight) + eff_m * weight;
                        }
                        if do_h {
                            let cur_h = *layer.height_buf.data.get_unchecked(idx);
                            *layer.height_buf.data.get_unchecked_mut(idx) = cur_h * (1.0 - weight) + eff_h * weight;
                        }
                        if do_emis {
                            let cur_e = *layer.emissive_buf.data.get_unchecked(idx);
                            let blended_e = cur_e.truncate() * (1.0 - weight) + eff_emis.truncate() * weight;
                            let blended_w = cur_e.w * (1.0 - weight) + eff_emis.w * weight;
                            *layer.emissive_buf.data.get_unchecked_mut(idx) = Vec4::new(blended_e.x, blended_e.y, blended_e.z, blended_w);
                        }
                        if do_opac {
                            let cur_o = *layer.opacity_buf.data.get_unchecked(idx);
                            *layer.opacity_buf.data.get_unchecked_mut(idx) = cur_o * (1.0 - weight) + opac_target * weight;
                        }
                        if do_clearcoat {
                            let cur_c = *layer.clearcoat_buf.data.get_unchecked(idx);
                            *layer.clearcoat_buf.data.get_unchecked_mut(idx) = cur_c * (1.0 - weight) + clearcoat_target * weight;
                        }
                        if do_aniso {
                            let cur_a = *layer.anisotropy_buf.data.get_unchecked(idx);
                            *layer.anisotropy_buf.data.get_unchecked_mut(idx) = cur_a * (1.0 - weight) + aniso_target * weight;
                            let cur_ang = *layer.anisotropy_angle_buf.data.get_unchecked(idx);
                            *layer.anisotropy_angle_buf.data.get_unchecked_mut(idx) = cur_ang * (1.0 - weight) + aniso_ang_target * weight;
                        }
                    }
                }
            }
        }

        if do_h || (is_eraser && self.settings.channels.height) {
            layer.update_normal_from_height_rect(min_x as u32, min_y as u32, max_x as u32, max_y as u32, 4.0);
        }
    }

pub fn barycentric_3d(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> (f32, f32, f32) {
    let v0 = b - a;
    let v1 = c - a;
    let v2 = p - a;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-8 {
        return (0.333, 0.333, 0.334);
    }
    let inv_denom = 1.0 / denom;
    let v = (d11 * d20 - d01 * d21) * inv_denom;
    let w = (d00 * d21 - d01 * d20) * inv_denom;
    let u = 1.0 - v - w;
    (u, v, w)
}

    /// True 3D World-Space Projection Painting (Zero UV Seams, Zero UV Bleeding)
    pub fn stamp_3d(
        &mut self,
        layer: &mut Layer,
        mesh: &crate::mesh::Mesh,
        hit_pos: Vec3,
        hit_normal: Vec3,
        world_radius: f32,
    ) {
        let triangles = mesh.query_sphere(hit_pos, world_radius);
        if triangles.is_empty() {
            return;
        }

        let w = layer.base_color_buf.width as i32;
        let h = layer.base_color_buf.height as i32;
        let w_f = w as f32;
        let h_f = h as f32;
        let buf_w = w as usize;
        let flow = self.settings.flow * self.settings.opacity;
        let world_radius_sq = world_radius * world_radius;

        let is_eraser = self.settings.mode == ToolMode::Eraser;
        let is_smudge = self.settings.mode == ToolMode::Smudge;
        let is_clone = self.settings.mode == ToolMode::Clone;
        let is_mask_target = self.settings.target == PaintTarget::Mask;

        let (paint_buf_ptr, paint_alpha_ptr) = if is_mask_target {
            layer.has_mask = true;
            if !layer.mask_effects.is_empty() {
                let eff = layer.get_or_create_active_mask_paint();
                match &mut eff.effect_type {
                    crate::layer::MaskEffectType::Paint { buffer, alpha } => {
                        if alpha.data.len() < (w * h) as usize {
                            alpha.data.resize((w * h) as usize, 0.0);
                        }
                        if buffer.data.len() < (w * h) as usize {
                            buffer.data.resize((w * h) as usize, 1.0);
                        }
                        (buffer.data.as_mut_ptr(), alpha.data.as_mut_ptr())
                    }
                    _ => (std::ptr::null_mut(), std::ptr::null_mut()),
                }
            } else {
                (std::ptr::null_mut(), std::ptr::null_mut())
            }
        } else {
            (std::ptr::null_mut(), std::ptr::null_mut())
        };

        // Smudge carrier update for 3D stroke
        if is_smudge && !triangles.is_empty() {
            let t0 = triangles[0];
            let i0 = mesh.indices[t0 as usize * 3] as usize;
            let uv0 = mesh.vertices[i0].uv;
            let cx = ((uv0.x.rem_euclid(1.0) * w_f).round() as i32).clamp(0, w - 1) as usize;
            let cy = ((uv0.y.rem_euclid(1.0) * h_f).round() as i32).clamp(0, h - 1) as usize;
            let sample_idx = cy * (w as usize) + cx;
            let cur_col = layer.base_color_buf.data[sample_idx];
            let cur_r = layer.roughness_buf.data[sample_idx];
            let cur_m = layer.metallic_buf.data[sample_idx];
            let cur_h = layer.height_buf.data[sample_idx];
            let cur_emis = layer.emissive_buf.data[sample_idx];

            let str_val = self.settings.smudge_strength.clamp(0.05, 1.0);
            if let Some(sc) = self.smudge_color {
                self.smudge_color = Some(sc.lerp(cur_col, str_val * 0.4));
            } else {
                self.smudge_color = Some(cur_col);
            }
            if let Some(sr) = self.smudge_roughness {
                self.smudge_roughness = Some(sr * (1.0 - str_val * 0.4) + cur_r * (str_val * 0.4));
            } else {
                self.smudge_roughness = Some(cur_r);
            }
            if let Some(sm) = self.smudge_metallic {
                self.smudge_metallic = Some(sm * (1.0 - str_val * 0.4) + cur_m * (str_val * 0.4));
            } else {
                self.smudge_metallic = Some(cur_m);
            }
            if let Some(sh) = self.smudge_height {
                self.smudge_height = Some(sh * (1.0 - str_val * 0.4) + cur_h * (str_val * 0.4));
            } else {
                self.smudge_height = Some(cur_h);
            }
            if let Some(se) = self.smudge_emissive {
                self.smudge_emissive = Some(se.lerp(cur_emis, str_val * 0.4));
            } else {
                self.smudge_emissive = Some(cur_emis);
            }
        }

        let base_col_target = self.settings.paint_color;
        let r_target = self.settings.paint_roughness;
        let m_target = self.settings.paint_metallic;
        let h_target = self.settings.paint_height;
        let emis_target = self.settings.paint_emissive;
        let opac_target = self.settings.paint_opacity;
        let clearcoat_target = self.settings.paint_clearcoat;
        let aniso_target = self.settings.paint_anisotropy;
        let aniso_ang_target = self.settings.paint_anisotropy_angle;

        let do_color = self.settings.channels.base_color;
        let do_r = self.settings.channels.roughness;
        let do_m = self.settings.channels.metallic;
        let do_h = self.settings.channels.height;
        let do_emis = self.settings.channels.emission;
        let do_opac = self.settings.channels.opacity;
        let do_clearcoat = self.settings.channels.clearcoat;
        let do_aniso = self.settings.channels.anisotropy;

        let mut overall_min_x = i32::MAX;
        let mut overall_min_y = i32::MAX;
        let mut overall_max_x = i32::MIN;
        let mut overall_max_y = i32::MIN;

        for tri_idx in triangles {
            let i0 = mesh.indices[tri_idx as usize * 3] as usize;
            let i1 = mesh.indices[tri_idx as usize * 3 + 1] as usize;
            let i2 = mesh.indices[tri_idx as usize * 3 + 2] as usize;

            let v0 = &mesh.vertices[i0];
            let v1 = &mesh.vertices[i1];
            let v2 = &mesh.vertices[i2];

            // Triangle normal
            let edge1 = v1.position - v0.position;
            let edge2 = v2.position - v0.position;
            let tri_normal = edge1.cross(edge2);
            let normal_len = tri_normal.length();
            if normal_len < 1e-7 {
                continue;
            }
            let tri_normal = tri_normal / normal_len;

            // Early plane rejection before any UV work
            let diff = hit_pos - v0.position;
            let dist_to_plane = diff.dot(tri_normal);
            if dist_to_plane.abs() > world_radius {
                continue;
            }

            // Normal falloff check (don't paint on back-facing surfaces)
            let normal_dot = tri_normal.dot(hit_normal);
            if normal_dot < -0.1 {
                continue;
            }
            let angle_weight = (normal_dot + 0.1).clamp(0.0, 1.0);

            // UV seam unwrapping relative to v0.uv
            let mut u0 = v0.uv.x;
            let mut v0_y = v0.uv.y;
            let mut u1 = v1.uv.x;
            let mut v1_y = v1.uv.y;
            let mut u2 = v2.uv.x;
            let mut v2_y = v2.uv.y;

            // Normalize u coordinates relative to u0
            if (u1 - u0) > 0.5 { u1 -= 1.0; } else if (u1 - u0) < -0.5 { u1 += 1.0; }
            if (u2 - u0) > 0.5 { u2 -= 1.0; } else if (u2 - u0) < -0.5 { u2 += 1.0; }

            // Normalize v coordinates relative to v0_y
            if (v1_y - v0_y) > 0.5 { v1_y -= 1.0; } else if (v1_y - v0_y) < -0.5 { v1_y += 1.0; }
            if (v2_y - v0_y) > 0.5 { v2_y -= 1.0; } else if (v2_y - v0_y) < -0.5 { v2_y += 1.0; }

            // Shift cluster into [0.0, 1.0) base
            let shift_u = u0.floor();
            let shift_v = v0_y.floor();
            u0 -= shift_u; u1 -= shift_u; u2 -= shift_u;
            v0_y -= shift_v; v1_y -= shift_v; v2_y -= shift_v;

            let p0_uv = Vec2::new(u0 * w_f, v0_y * h_f);
            let p1_uv = Vec2::new(u1 * w_f, v1_y * h_f);
            let p2_uv = Vec2::new(u2 * w_f, v2_y * h_f);

            // Triangle UV 2D area (cross product of edges in 2D)
            let det = (p1_uv.y - p2_uv.y) * (p0_uv.x - p2_uv.x) + (p2_uv.x - p1_uv.x) * (p0_uv.y - p2_uv.y);
            if det.abs() < 1e-6 {
                continue;
            }
            let inv_det = 1.0 / det;

            // Bounding box of triangle in UV space
            let tri_min_x = p0_uv.x.min(p1_uv.x).min(p2_uv.x).floor() as i32;
            let tri_max_x = p0_uv.x.max(p1_uv.x).max(p2_uv.x).ceil() as i32;
            let tri_min_y = p0_uv.y.min(p1_uv.y).min(p2_uv.y).floor() as i32;
            let tri_max_y = p0_uv.y.max(p1_uv.y).max(p2_uv.y).ceil() as i32;

            // Tight brush sphere bounding in UV space
            let e1_3d = (v1.position - v0.position).length().max(1e-5);
            let e2_3d = (v2.position - v0.position).length().max(1e-5);
            let e1_uv = (p1_uv - p0_uv).length();
            let e2_uv = (p2_uv - p0_uv).length();
            let max_uv_rate = (e1_uv / e1_3d).max(e2_uv / e2_3d);

            let plane_rad = (world_radius_sq - dist_to_plane * dist_to_plane).max(0.0).sqrt();
            let uv_brush_radius = (plane_rad * max_uv_rate * 1.25).ceil() as i32 + 2;

            let proj_pt = hit_pos - tri_normal * dist_to_plane;
            let (bc_u, bc_v, bc_w) = Self::barycentric_3d(proj_pt, v0.position, v1.position, v2.position);
            let center_uv = p0_uv * bc_u.clamp(-0.2, 1.2) + p1_uv * bc_v.clamp(-0.2, 1.2) + p2_uv * bc_w.clamp(-0.2, 1.2);

            let min_x = tri_min_x.max((center_uv.x - uv_brush_radius as f32).floor() as i32).clamp(0, w - 1);
            let max_x = tri_max_x.min((center_uv.x + uv_brush_radius as f32).ceil() as i32).clamp(0, w - 1);
            let min_y = tri_min_y.max((center_uv.y - uv_brush_radius as f32).floor() as i32).clamp(0, h - 1);
            let max_y = tri_max_y.min((center_uv.y + uv_brush_radius as f32).ceil() as i32).clamp(0, h - 1);

            if min_x > max_x || min_y > max_y {
                continue;
            }

            for py in min_y..=max_y {
                let py_f = py as f32 + 0.5;
                let wrap_y = py as usize;
                let buf_y_offset = wrap_y * buf_w;

                for px in min_x..=max_x {
                    let px_f = px as f32 + 0.5;

                    // Barycentric coordinates (w0, w1, w2)
                    let w0 = ((p1_uv.y - p2_uv.y) * (px_f - p2_uv.x) + (p2_uv.x - p1_uv.x) * (py_f - p2_uv.y)) * inv_det;
                    let w1 = ((p2_uv.y - p0_uv.y) * (px_f - p2_uv.x) + (p0_uv.x - p2_uv.x) * (py_f - p2_uv.y)) * inv_det;
                    let w2 = 1.0 - w0 - w1;

                    // If pixel center is inside the triangle (with small epsilon)
                    if w0 >= -0.01 && w1 >= -0.01 && w2 >= -0.01 {
                        // Interpolate 3D position of this pixel
                        let p3d = v0.position * w0.clamp(0.0, 1.0)
                            + v1.position * w1.clamp(0.0, 1.0)
                            + v2.position * w2.clamp(0.0, 1.0);

                        let dist_sq = (p3d - hit_pos).length_squared();
                        if dist_sq <= world_radius_sq {
                            let dist = dist_sq.sqrt();
                            let norm_dist = (dist / world_radius).clamp(0.0, 1.0);
                            let hardness = self.settings.hardness;
                            let falloff = if norm_dist < hardness {
                                1.0
                            } else {
                                1.0 - (norm_dist - hardness) / (1.0 - hardness).max(1e-4)
                            };

                            let weight = (falloff * flow * angle_weight).clamp(0.0, 1.0);
                            if weight <= 1e-4 {
                                continue;
                            }

                            let wrap_x = px as usize;

                            overall_min_x = overall_min_x.min(wrap_x as i32);
                            overall_max_x = overall_max_x.max(wrap_x as i32);
                            overall_min_y = overall_min_y.min(wrap_y as i32);
                            overall_max_y = overall_max_y.max(wrap_y as i32);

                            let idx = buf_y_offset + wrap_x;

                            let (eff_base_col, eff_r, eff_m, eff_h, eff_emis) = if is_clone {
                                if let Some(src_uv) = self.settings.clone_source_uv {
                                    let scx = ((src_uv.x.rem_euclid(1.0) * w_f).round() as i32).clamp(0, w - 1) as usize;
                                    let scy = ((src_uv.y.rem_euclid(1.0) * h_f).round() as i32).clamp(0, h - 1) as usize;
                                    let src_idx = scy * (w as usize) + scx;
                                    (
                                        layer.base_color_buf.data[src_idx],
                                        layer.roughness_buf.data[src_idx],
                                        layer.metallic_buf.data[src_idx],
                                        layer.height_buf.data[src_idx],
                                        layer.emissive_buf.data[src_idx],
                                    )
                                } else {
                                    (base_col_target, r_target, m_target, h_target, emis_target)
                                }
                            } else if is_smudge {
                                (
                                    self.smudge_color.unwrap_or(base_col_target),
                                    self.smudge_roughness.unwrap_or(r_target),
                                    self.smudge_metallic.unwrap_or(m_target),
                                    self.smudge_height.unwrap_or(h_target),
                                    self.smudge_emissive.unwrap_or(emis_target),
                                )
                            } else {
                                (base_col_target, r_target, m_target, h_target, emis_target)
                            };

                            unsafe {
                                if is_mask_target {
                                    let target_val = if is_eraser { 0.0 } else { self.settings.mask_paint_value };
                                    let cur_m = *layer.mask_buf.data.get_unchecked(idx);
                                    *layer.mask_buf.data.get_unchecked_mut(idx) = cur_m * (1.0 - weight) + target_val * weight;
                                    if !paint_buf_ptr.is_null() {
                                        if is_eraser {
                                            let cur_a = *paint_alpha_ptr.add(idx);
                                            *paint_alpha_ptr.add(idx) = (cur_a * (1.0 - weight)).max(0.0);
                                        } else {
                                            let cur_p = *paint_buf_ptr.add(idx);
                                            *paint_buf_ptr.add(idx) = cur_p * (1.0 - weight) + target_val * weight;
                                            let cur_a = *paint_alpha_ptr.add(idx);
                                            *paint_alpha_ptr.add(idx) = (cur_a + weight * (1.0 - cur_a)).clamp(0.0, 1.0);
                                        }
                                    }
                                } else if is_eraser {
                                    let cur_col = *layer.base_color_buf.data.get_unchecked(idx);
                                    let new_a = (cur_col.w * (1.0 - weight)).max(0.0);
                                    layer.base_color_buf.data.get_unchecked_mut(idx).w = new_a;
                                    let cur_h = *layer.height_buf.data.get_unchecked(idx);
                                    *layer.height_buf.data.get_unchecked_mut(idx) = cur_h * (1.0 - weight) + 0.5 * weight;
                                    let cur_emis = *layer.emissive_buf.data.get_unchecked(idx);
                                    *layer.emissive_buf.data.get_unchecked_mut(idx) = cur_emis * (1.0 - weight);
                                    let cur_opac = *layer.opacity_buf.data.get_unchecked(idx);
                                    *layer.opacity_buf.data.get_unchecked_mut(idx) = cur_opac * (1.0 - weight) + 1.0 * weight;
                                    let cur_c = *layer.clearcoat_buf.data.get_unchecked(idx);
                                    *layer.clearcoat_buf.data.get_unchecked_mut(idx) = cur_c * (1.0 - weight);
                                    let cur_a = *layer.anisotropy_buf.data.get_unchecked(idx);
                                    *layer.anisotropy_buf.data.get_unchecked_mut(idx) = cur_a * (1.0 - weight);
                                    let cur_ang = *layer.anisotropy_angle_buf.data.get_unchecked(idx);
                                    *layer.anisotropy_angle_buf.data.get_unchecked_mut(idx) = cur_ang * (1.0 - weight);
                                } else {
                                    let cur_col = *layer.base_color_buf.data.get_unchecked(idx);
                                    let alpha = (cur_col.w + weight * (1.0 - cur_col.w)).clamp(0.0, 1.0);
                                    if do_color {
                                        let blended = cur_col.truncate() * (1.0 - weight) + eff_base_col.truncate() * weight;
                                        *layer.base_color_buf.data.get_unchecked_mut(idx) = Vec4::new(blended.x, blended.y, blended.z, alpha);
                                    } else {
                                        layer.base_color_buf.data.get_unchecked_mut(idx).w = alpha;
                                    }
                                    if do_r {
                                        let cur_r = *layer.roughness_buf.data.get_unchecked(idx);
                                        *layer.roughness_buf.data.get_unchecked_mut(idx) = cur_r * (1.0 - weight) + eff_r * weight;
                                    }
                                    if do_m {
                                        let cur_m = *layer.metallic_buf.data.get_unchecked(idx);
                                        *layer.metallic_buf.data.get_unchecked_mut(idx) = cur_m * (1.0 - weight) + eff_m * weight;
                                    }
                                    if do_h {
                                        let cur_h = *layer.height_buf.data.get_unchecked(idx);
                                        *layer.height_buf.data.get_unchecked_mut(idx) = cur_h * (1.0 - weight) + eff_h * weight;
                                    }
                                    if do_emis {
                                        let cur_e = *layer.emissive_buf.data.get_unchecked(idx);
                                        let blended_e = cur_e.truncate() * (1.0 - weight) + eff_emis.truncate() * weight;
                                        let blended_w = cur_e.w * (1.0 - weight) + eff_emis.w * weight;
                                        *layer.emissive_buf.data.get_unchecked_mut(idx) = Vec4::new(blended_e.x, blended_e.y, blended_e.z, blended_w);
                                    }
                                    if do_opac {
                                        let cur_o = *layer.opacity_buf.data.get_unchecked(idx);
                                        *layer.opacity_buf.data.get_unchecked_mut(idx) = cur_o * (1.0 - weight) + opac_target * weight;
                                    }
                                    if do_clearcoat {
                                        let cur_c = *layer.clearcoat_buf.data.get_unchecked(idx);
                                        *layer.clearcoat_buf.data.get_unchecked_mut(idx) = cur_c * (1.0 - weight) + clearcoat_target * weight;
                                    }
                                    if do_aniso {
                                        let cur_a = *layer.anisotropy_buf.data.get_unchecked(idx);
                                        *layer.anisotropy_buf.data.get_unchecked_mut(idx) = cur_a * (1.0 - weight) + aniso_target * weight;
                                        let cur_ang = *layer.anisotropy_angle_buf.data.get_unchecked(idx);
                                        *layer.anisotropy_angle_buf.data.get_unchecked_mut(idx) = cur_ang * (1.0 - weight) + aniso_ang_target * weight;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if overall_min_x <= overall_max_x && overall_min_y <= overall_max_y {
            let r_min_x = overall_min_x as u32;
            let r_max_x = overall_max_x as u32;
            let r_min_y = overall_min_y as u32;
            let r_max_y = overall_max_y as u32;

            if do_h || (is_eraser && self.settings.channels.height) {
                layer.update_normal_from_height_rect(r_min_x, r_min_y, r_max_x, r_max_y, 4.0);
            }

            if let Some((dx0, dy0, dx1, dy1)) = layer.dirty_rect {
                layer.dirty_rect = Some((
                    dx0.min(r_min_x),
                    dy0.min(r_min_y),
                    dx1.max(r_max_x),
                    dy1.max(r_max_y),
                ));
            } else {
                layer.dirty_rect = Some((r_min_x, r_min_y, r_max_x, r_max_y));
            }
        }
    }

    /// Paint continuous stroke in 3D world space across triangles & UV islands
    pub fn paint_stroke_3d(
        &mut self,
        layer: &mut Layer,
        mesh: &crate::mesh::Mesh,
        hit_pos: Vec3,
        hit_normal: Vec3,
        world_radius: f32,
    ) {
        let (points, normals) = match (self.last_hit_pos, self.last_hit_normal) {
            (Some(prev_pos), Some(prev_norm)) => {
                let delta = hit_pos - prev_pos;
                if self.settings.follow_path && delta.length_squared() > 1e-8 {
                    let n = hit_normal.normalize_or_zero();
                    let tan_delta = delta - n * n.dot(delta);
                    if tan_delta.length_squared() > 1e-8 {
                        let up = if n.y.abs() < 0.95 { Vec3::Y } else { Vec3::X };
                        let t_axis = n.cross(up).normalize();
                        let b_axis = n.cross(t_axis);
                        let u_proj = tan_delta.dot(t_axis);
                        let v_proj = tan_delta.dot(b_axis);
                        self.follow_path_angle = v_proj.atan2(u_proj) + self.settings.follow_path_offset;
                    }
                }
                let dist = delta.length();
                let step_dist = (world_radius * self.settings.spacing).max(0.001);
                let steps = ((dist / step_dist).ceil() as usize).clamp(1, 6);

                if steps > 1 {
                    let mut pts = Vec::with_capacity(steps);
                    let mut nrms = Vec::with_capacity(steps);
                    for i in 1..=steps {
                        let t = i as f32 / steps as f32;
                        pts.push(prev_pos.lerp(hit_pos, t));
                        nrms.push(prev_norm.lerp(hit_normal, t).normalize());
                    }
                    (pts, nrms)
                } else {
                    (vec![hit_pos], vec![hit_normal])
                }
            }
            _ => (vec![hit_pos], vec![hit_normal]),
        };

        let c = self.settings.symmetry_center;
        let sym_x = self.settings.symmetry_x;
        let sym_y = self.settings.symmetry_y;
        let sym_z = self.settings.symmetry_z;

        for (pos, norm) in points.into_iter().zip(normals.into_iter()) {
            self.stroke_counter = self.stroke_counter.wrapping_add(1);
            self.stamp_3d(layer, mesh, pos, norm, world_radius);

            // 1. Single Axis Symmetry
            if sym_x {
                let p = Vec3::new(2.0 * c.x - pos.x, pos.y, pos.z);
                let n = Vec3::new(-norm.x, norm.y, norm.z);
                self.stamp_3d(layer, mesh, p, n, world_radius);
            }
            if sym_y {
                let p = Vec3::new(pos.x, 2.0 * c.y - pos.y, pos.z);
                let n = Vec3::new(norm.x, -norm.y, norm.z);
                self.stamp_3d(layer, mesh, p, n, world_radius);
            }
            if sym_z {
                let p = Vec3::new(pos.x, pos.y, 2.0 * c.z - pos.z);
                let n = Vec3::new(norm.x, norm.y, -norm.z);
                self.stamp_3d(layer, mesh, p, n, world_radius);
            }

            // 2. Dual Axis Symmetry
            if sym_x && sym_y {
                let p = Vec3::new(2.0 * c.x - pos.x, 2.0 * c.y - pos.y, pos.z);
                let n = Vec3::new(-norm.x, -norm.y, norm.z);
                self.stamp_3d(layer, mesh, p, n, world_radius);
            }
            if sym_x && sym_z {
                let p = Vec3::new(2.0 * c.x - pos.x, pos.y, 2.0 * c.z - pos.z);
                let n = Vec3::new(-norm.x, norm.y, -norm.z);
                self.stamp_3d(layer, mesh, p, n, world_radius);
            }
            if sym_y && sym_z {
                let p = Vec3::new(pos.x, 2.0 * c.y - pos.y, 2.0 * c.z - pos.z);
                let n = Vec3::new(norm.x, -norm.y, -norm.z);
                self.stamp_3d(layer, mesh, p, n, world_radius);
            }

            // 3. Triple Axis Symmetry
            if sym_x && sym_y && sym_z {
                let p = Vec3::new(2.0 * c.x - pos.x, 2.0 * c.y - pos.y, 2.0 * c.z - pos.z);
                let n = Vec3::new(-norm.x, -norm.y, -norm.z);
                self.stamp_3d(layer, mesh, p, n, world_radius);
            }
        }

        self.last_hit_pos = Some(hit_pos);
        self.last_hit_normal = Some(hit_normal);
    }

    /// Calculate all active symmetry points (primary + mirrored across X/Y/Z) for 3D projection painting
    pub fn get_symmetry_points(&self, hit_pos: Vec3, hit_normal: Vec3) -> Vec<(Vec3, Vec3)> {
        let mut pts = vec![(hit_pos, hit_normal)];
        let c = self.settings.symmetry_center;
        let sym_x = self.settings.symmetry_x;
        let sym_y = self.settings.symmetry_y;
        let sym_z = self.settings.symmetry_z;

        if !sym_x && !sym_y && !sym_z {
            return pts;
        }

        // 1. Single Axis Symmetry
        if sym_x {
            let p = Vec3::new(2.0 * c.x - hit_pos.x, hit_pos.y, hit_pos.z);
            let n = Vec3::new(-hit_normal.x, hit_normal.y, hit_normal.z);
            pts.push((p, n));
        }
        if sym_y {
            let p = Vec3::new(hit_pos.x, 2.0 * c.y - hit_pos.y, hit_pos.z);
            let n = Vec3::new(hit_normal.x, -hit_normal.y, hit_normal.z);
            pts.push((p, n));
        }
        if sym_z {
            let p = Vec3::new(hit_pos.x, hit_pos.y, 2.0 * c.z - hit_pos.z);
            let n = Vec3::new(hit_normal.x, hit_normal.y, -hit_normal.z);
            pts.push((p, n));
        }

        // 2. Dual Axis Symmetry
        if sym_x && sym_y {
            let p = Vec3::new(2.0 * c.x - hit_pos.x, 2.0 * c.y - hit_pos.y, hit_pos.z);
            let n = Vec3::new(-hit_normal.x, -hit_normal.y, hit_normal.z);
            pts.push((p, n));
        }
        if sym_x && sym_z {
            let p = Vec3::new(2.0 * c.x - hit_pos.x, hit_pos.y, 2.0 * c.z - hit_pos.z);
            let n = Vec3::new(-hit_normal.x, hit_normal.y, -hit_normal.z);
            pts.push((p, n));
        }
        if sym_y && sym_z {
            let p = Vec3::new(hit_pos.x, 2.0 * c.y - hit_pos.y, 2.0 * c.z - hit_pos.z);
            let n = Vec3::new(hit_normal.x, -hit_normal.y, -hit_normal.z);
            pts.push((p, n));
        }

        // 3. Triple Axis Symmetry
        if sym_x && sym_y && sym_z {
            let p = Vec3::new(2.0 * c.x - hit_pos.x, 2.0 * c.y - hit_pos.y, 2.0 * c.z - hit_pos.z);
            let n = Vec3::new(-hit_normal.x, -hit_normal.y, -hit_normal.z);
            pts.push((p, n));
        }

        pts
    }

    /// Collect 3D projection stamping uniform and triangle parameters for GPU compute shader execution
    pub fn build_gpu_stamp_data(
        &self,
        mesh: &crate::mesh::Mesh,
        hit_pos: Vec3,
        hit_normal: Vec3,
        world_radius: f32,
        doc_w: u32,
        doc_h: u32,
    ) -> Option<(crate::gpu_brush::GpuBrushUniform, Vec<crate::gpu_brush::GpuBrushTriangle>)> {
        let triangles = mesh.query_sphere(hit_pos, world_radius);
        if triangles.is_empty() {
            return None;
        }

        let mut gpu_triangles = Vec::with_capacity(triangles.len().min(512));
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        let w_f = doc_w as f32;
        let h_f = doc_h as f32;

        for tri_idx in triangles.into_iter().take(512) {
            let i0 = mesh.indices[tri_idx as usize * 3] as usize;
            let i1 = mesh.indices[tri_idx as usize * 3 + 1] as usize;
            let i2 = mesh.indices[tri_idx as usize * 3 + 2] as usize;

            let v0 = &mesh.vertices[i0];
            let v1 = &mesh.vertices[i1];
            let v2 = &mesh.vertices[i2];

            let mut u0 = v0.uv.x;
            let mut v0_y = v0.uv.y;
            let mut u1 = v1.uv.x;
            let mut v1_y = v1.uv.y;
            let mut u2 = v2.uv.x;
            let mut v2_y = v2.uv.y;

            if (u1 - u0) > 0.5 { u1 -= 1.0; } else if (u1 - u0) < -0.5 { u1 += 1.0; }
            if (u2 - u0) > 0.5 { u2 -= 1.0; } else if (u2 - u0) < -0.5 { u2 += 1.0; }
            if (v1_y - v0_y) > 0.5 { v1_y -= 1.0; } else if (v1_y - v0_y) < -0.5 { v1_y += 1.0; }
            if (v2_y - v0_y) > 0.5 { v2_y -= 1.0; } else if (v2_y - v0_y) < -0.5 { v2_y += 1.0; }

            let shift_u = u0.floor();
            let shift_v = v0_y.floor();
            u0 -= shift_u; u1 -= shift_u; u2 -= shift_u;
            v0_y -= shift_v; v1_y -= shift_v; v2_y -= shift_v;

            let px0 = (u0 * w_f) as i32;
            let py0 = (v0_y * h_f) as i32;
            let px1 = (u1 * w_f) as i32;
            let py1 = (v1_y * h_f) as i32;
            let px2 = (u2 * w_f) as i32;
            let py2 = (v2_y * h_f) as i32;

            min_x = min_x.min(px0).min(px1).min(px2);
            min_y = min_y.min(py0).min(py1).min(py2);
            max_x = max_x.max(px0).max(px1).max(px2);
            max_y = max_y.max(py0).max(py1).max(py2);

            gpu_triangles.push(crate::gpu_brush::GpuBrushTriangle {
                pos0: [v0.position.x, v0.position.y, v0.position.z, 1.0],
                pos1: [v1.position.x, v1.position.y, v1.position.z, 1.0],
                pos2: [v2.position.x, v2.position.y, v2.position.z, 1.0],
                nrm0: [v0.normal.x, v0.normal.y, v0.normal.z, 0.0],
                nrm1: [v1.normal.x, v1.normal.y, v1.normal.z, 0.0],
                nrm2: [v2.normal.x, v2.normal.y, v2.normal.z, 0.0],
                uv0: [u0, v0_y],
                uv1: [u1, v1_y],
                uv2: [u2, v2_y],
                _pad: [0.0, 0.0],
            });
        }

        let pad = 6;
        let clamped_min_x = (min_x - pad).max(0);
        let clamped_min_y = (min_y - pad).max(0);
        let clamped_max_x = (max_x + pad).min(doc_w as i32 - 1);
        let clamped_max_y = (max_y + pad).min(doc_h as i32 - 1);

        let uniform = crate::gpu_brush::GpuBrushUniform {
            hit_pos: [hit_pos.x, hit_pos.y, hit_pos.z, world_radius],
            hit_normal: [hit_normal.x, hit_normal.y, hit_normal.z, self.settings.hardness],
            paint_color: [self.settings.paint_color.x, self.settings.paint_color.y, self.settings.paint_color.z, self.settings.paint_color.w],
            params: [self.settings.paint_roughness, self.settings.paint_metallic, self.settings.paint_height, self.settings.flow * self.settings.opacity],
            bounds_min: [clamped_min_x, clamped_min_y],
            bounds_max: [clamped_max_x, clamped_max_y],
            tex_size: [doc_w as f32, doc_h as f32],
            flags: [if self.settings.mode == ToolMode::Eraser { 1 } else { 0 }, if self.settings.target == PaintTarget::Mask { 1 } else { 0 }],
            tri_count: gpu_triangles.len() as u32,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };

        Some((uniform, gpu_triangles))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_diagnostic_performance_512() {
        let mesh = crate::mesh::Mesh::create_shader_ball();
        let mut layer = crate::layer::Layer::new_paint_layer(1, "Test".to_string(), 512, 512);
        let mut engine = BrushEngine::new();
        engine.settings.radius = 20.0;
        engine.update_cache();

        // 1. Benchmark BVH Raycast
        let ray = crate::mesh::Ray {
            origin: glam::Vec3::new(0.0, 0.0, 3.0),
            direction: glam::Vec3::new(0.0, 0.0, -1.0),
        };
        let t0 = Instant::now();
        let iters = 1000;
        let mut hits = 0;
        for _ in 0..iters {
            if mesh.raycast(&ray).is_some() {
                hits += 1;
            }
        }
        let raycast_elapsed = t0.elapsed();
        let per_raycast_us = raycast_elapsed.as_micros() as f64 / iters as f64;
        println!("DIAGNOSTIC: 1000 Raycasts took {:?}, avg {:.2} us/raycast (hits: {})", raycast_elapsed, per_raycast_us, hits);

        // 2. Benchmark Query Sphere
        let center = glam::Vec3::new(0.0, 0.0, 1.0);
        let t1 = Instant::now();
        for _ in 0..iters {
            let _ = mesh.query_sphere(center, 0.15);
        }
        let query_elapsed = t1.elapsed();
        let per_query_us = query_elapsed.as_micros() as f64 / iters as f64;
        println!("DIAGNOSTIC: 1000 QuerySpheres took {:?}, avg {:.2} us/query", query_elapsed, per_query_us);

        // 3. Benchmark 3D Projection Stamp
        let t2 = Instant::now();
        let stamp_iters = 50;
        for i in 0..stamp_iters {
            let offset = (i as f32) * 0.002;
            let hit_p = glam::Vec3::new(offset, offset, 1.0);
            let hit_n = glam::Vec3::new(0.0, 0.0, 1.0);
            engine.stamp_3d(&mut layer, &mesh, hit_p, hit_n, 0.15);
        }
        let stamp_elapsed = t2.elapsed();
        let per_stamp_ms = stamp_elapsed.as_secs_f64() * 1000.0 / stamp_iters as f64;
        println!("DIAGNOSTIC: 50 3D Stamps at 512x512 took {:?}, avg {:.3} ms/stamp", stamp_elapsed, per_stamp_ms);

        // 4. Benchmark paint_stroke_3d
        let t3 = Instant::now();
        for i in 0..10 {
            let p = glam::Vec3::new(0.05 * i as f32, 0.0, 1.0);
            let n = glam::Vec3::new(0.0, 0.0, 1.0);
            engine.paint_stroke_3d(&mut layer, &mesh, p, n, 0.15);
        }
        let stroke_elapsed = t3.elapsed();
        println!("DIAGNOSTIC: 10 paint_stroke_3d steps took {:?}", stroke_elapsed);
    }

    #[test]
    fn test_emissive_and_opacity_painting_and_erasing() {
        let mut layer = crate::layer::Layer::new_paint_layer(1, "TestLayer".to_string(), 64, 64);
        let mut engine = BrushEngine::new();
        engine.settings.radius = 10.0;
        engine.settings.hardness = 1.0;
        engine.settings.channels.emission = true;
        engine.settings.channels.opacity = true;
        engine.settings.paint_emissive = glam::Vec4::new(1.0, 0.4, 0.1, 2.5);
        engine.settings.paint_opacity = 0.25;
        engine.update_cache();

        // 1. Paint stamp in 2D
        engine.stamp_uv(&mut layer, glam::Vec2::new(0.5, 0.5));
        let painted_emis = layer.emissive_buf.get_pixel(32, 32);
        let painted_opac = layer.opacity_buf.get_pixel(32, 32);

        assert!(painted_emis.x > 0.8, "Emissive red channel should be painted, got {}", painted_emis.x);
        assert!(painted_emis.y > 0.3, "Emissive green channel should be painted, got {}", painted_emis.y);
        assert!(painted_emis.w > 2.0, "Emissive intensity should be stamped, got {}", painted_emis.w);
        assert!(painted_opac < 0.35, "Opacity should be reduced towards 0.25, got {}", painted_opac);

        // 2. Erase stamp in 2D
        engine.settings.mode = ToolMode::Eraser;
        engine.stamp_uv(&mut layer, glam::Vec2::new(0.5, 0.5));
        let erased_emis = layer.emissive_buf.get_pixel(32, 32);
        let erased_opac = layer.opacity_buf.get_pixel(32, 32);

        assert!(erased_emis.x < 0.2, "Emissive should be erased towards 0, got {}", erased_emis.x);
        assert!(erased_emis.w < 0.5, "Emissive intensity should be erased, got {}", erased_emis.w);
        assert!(erased_opac > 0.8, "Opacity should be restored towards 1.0, got {}", erased_opac);
    }

    #[test]
    fn test_symmetry_point_generation() {
        let mut engine = BrushEngine::new();
        let hit_pos = glam::Vec3::new(1.5, 2.0, 3.0);
        let hit_norm = glam::Vec3::new(0.577, 0.577, 0.577);

        // Off
        engine.settings.symmetry_x = false;
        engine.settings.symmetry_y = false;
        engine.settings.symmetry_z = false;
        let pts = engine.get_symmetry_points(hit_pos, hit_norm);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0].0, hit_pos);

        // X axis
        engine.settings.symmetry_x = true;
        let pts_x = engine.get_symmetry_points(hit_pos, hit_norm);
        assert_eq!(pts_x.len(), 2);
        assert_eq!(pts_x[0].0, hit_pos);
        assert_eq!(pts_x[1].0, glam::Vec3::new(-1.5, 2.0, 3.0));
        assert_eq!(pts_x[1].1, glam::Vec3::new(-0.577, 0.577, 0.577));

        // Y axis
        engine.settings.symmetry_x = false;
        engine.settings.symmetry_y = true;
        let pts_y = engine.get_symmetry_points(hit_pos, hit_norm);
        assert_eq!(pts_y.len(), 2);
        assert_eq!(pts_y[1].0, glam::Vec3::new(1.5, -2.0, 3.0));
        assert_eq!(pts_y[1].1, glam::Vec3::new(0.577, -0.577, 0.577));

        // Z axis
        engine.settings.symmetry_y = false;
        engine.settings.symmetry_z = true;
        let pts_z = engine.get_symmetry_points(hit_pos, hit_norm);
        assert_eq!(pts_z.len(), 2);
        assert_eq!(pts_z[1].0, glam::Vec3::new(1.5, 2.0, -3.0));
        assert_eq!(pts_z[1].1, glam::Vec3::new(0.577, 0.577, -0.577));

        // Dual axis (X and Y)
        engine.settings.symmetry_x = true;
        engine.settings.symmetry_y = true;
        engine.settings.symmetry_z = false;
        let pts_xy = engine.get_symmetry_points(hit_pos, hit_norm);
        assert_eq!(pts_xy.len(), 4, "Dual axis symmetry should generate original + X + Y + XY points");

        // Custom center offset
        engine.settings.symmetry_x = true;
        engine.settings.symmetry_y = false;
        engine.settings.symmetry_center = glam::Vec3::new(1.0, 0.0, 0.0);
        let pts_offset = engine.get_symmetry_points(hit_pos, hit_norm);
        // mirrored x around 1.0: 2 * 1.0 - 1.5 = 0.5
        assert_eq!(pts_offset[1].0.x, 0.5);
    }

    #[test]
    fn test_clearcoat_and_anisotropy_painting_and_erasing() {
        let mut engine = BrushEngine::new();
        let mut layer = Layer::new_paint_layer(1, "CoatAnisoTest".to_string(), 64, 64);

        engine.settings.radius = 8.0;
        engine.settings.flow = 1.0;
        engine.settings.opacity = 1.0;
        engine.settings.hardness = 1.0;
        engine.settings.channels.clearcoat = true;
        engine.settings.channels.anisotropy = true;
        engine.settings.paint_clearcoat = 0.95;
        engine.settings.paint_anisotropy = -0.75;
        engine.settings.paint_anisotropy_angle = 0.25;
        engine.update_cache();

        // 1. Paint stamp in 2D
        engine.stamp_uv(&mut layer, glam::Vec2::new(0.5, 0.5));
        let painted_cc = layer.clearcoat_buf.get_pixel(32, 32);
        let painted_aniso = layer.anisotropy_buf.get_pixel(32, 32);
        let painted_ang = layer.anisotropy_angle_buf.get_pixel(32, 32);

        assert!(painted_cc > 0.8, "Clearcoat should be painted, got {}", painted_cc);
        assert!(painted_aniso < -0.6, "Anisotropy should be stamped to -0.75, got {}", painted_aniso);
        assert!((painted_ang - 0.25).abs() < 0.05, "Anisotropy angle should be stamped to 0.25, got {}", painted_ang);

        // 2. Erase stamp in 2D
        engine.settings.mode = ToolMode::Eraser;
        engine.stamp_uv(&mut layer, glam::Vec2::new(0.5, 0.5));
        let erased_cc = layer.clearcoat_buf.get_pixel(32, 32);
        let erased_aniso = layer.anisotropy_buf.get_pixel(32, 32);
        let erased_ang = layer.anisotropy_angle_buf.get_pixel(32, 32);

        assert!(erased_cc < 0.15, "Clearcoat should be erased towards 0.0, got {}", erased_cc);
        assert!(erased_aniso.abs() < 0.15, "Anisotropy should be erased towards 0.0, got {}", erased_aniso);
        assert!(erased_ang < 0.15, "Anisotropy angle should be erased towards 0.0, got {}", erased_ang);
    }

    #[test]
    fn test_stencil_sampling_and_transformations() {
        let mut engine = BrushEngine::new();
        let mut stencil_buf = crate::channel::RgbaBuffer::new(16, 16, Vec4::new(0.0, 0.0, 0.0, 1.0));
        stencil_buf.set_pixel(8, 8, Vec4::new(1.0, 0.5, 0.25, 1.0));
        engine.stencil_buffer = Some(stencil_buf);

        // Disabled by default
        assert!(engine.sample_stencil(Vec2::new(0.5, 0.5)).is_none());

        // Enable stencil
        engine.settings.stencil.enabled = true;
        let sample_center = engine.sample_stencil(Vec2::new(0.5, 0.5));
        assert!(sample_center.is_some(), "Center sample should hit stencil");
        let val = sample_center.unwrap();
        assert!((val.x - 1.0).abs() < 0.1, "Sample at center should be near (1.0, 0.5, 0.25), got x={}", val.x);

        // Outside scaled bounds should return None
        engine.settings.stencil.scale = 0.5;
        let sample_edge = engine.sample_stencil(Vec2::new(0.95, 0.95));
        assert!(sample_edge.is_none(), "Sample outside scaled stencil bounds must return None");
    }

    #[test]
    fn test_smudge_tool_carrier_and_blending() {
        let mut layer = Layer::new_paint_layer(1, "Paint".to_string(), 32, 32);
        // Fill left half with red, right half with blue
        for y in 0..32 {
            for x in 0..16 {
                layer.base_color_buf.set_pixel(x, y, Vec4::new(1.0, 0.0, 0.0, 1.0));
            }
            for x in 16..32 {
                layer.base_color_buf.set_pixel(x, y, Vec4::new(0.0, 0.0, 1.0, 1.0));
            }
        }

        let mut engine = BrushEngine::new();
        engine.settings.mode = ToolMode::Smudge;
        engine.settings.radius = 4.0;
        engine.settings.flow = 1.0;
        engine.settings.opacity = 1.0;
        engine.settings.smudge_strength = 0.8;
        engine.update_cache();

        // Stroke from x=12 (red side) across to x=18 (blue side)
        engine.paint_stroke(&mut layer, Vec2::new(12.0 / 32.0, 16.0 / 32.0));
        engine.paint_stroke(&mut layer, Vec2::new(18.0 / 32.0, 16.0 / 32.0));
        engine.end_stroke();

        let smeared_pixel = layer.base_color_buf.get_pixel(18, 16);
        assert!(smeared_pixel.x > 0.1, "Smudge should smear red color across boundary into blue side, got r={}", smeared_pixel.x);
    }

    #[test]
    fn test_clone_stamp_tool_sampling_and_offset() {
        let mut layer = Layer::new_paint_layer(1, "Paint".to_string(), 32, 32);
        // Place distinct green spot at (8, 8)
        layer.base_color_buf.set_pixel(8, 8, Vec4::new(0.0, 1.0, 0.0, 1.0));
        layer.roughness_buf.set_pixel(8, 8, 0.15);

        let mut engine = BrushEngine::new();
        engine.settings.mode = ToolMode::Clone;
        engine.settings.radius = 2.0;
        engine.settings.flow = 1.0;
        engine.settings.opacity = 1.0;
        engine.settings.clone_source_uv = Some(Vec2::new(8.0 / 32.0, 8.0 / 32.0));
        engine.update_cache();

        // Stamp at target location (24, 24)
        engine.stamp_uv(&mut layer, Vec2::new(24.0 / 32.0, 24.0 / 32.0));

        let cloned_col = layer.base_color_buf.get_pixel(24, 24);
        let cloned_r = layer.roughness_buf.get_pixel(24, 24);
        assert!(cloned_col.y > 0.5, "Clone stamp should copy green color from source, got g={}", cloned_col.y);
        assert!(cloned_r < 0.35, "Clone stamp should copy roughness from source, got r={}", cloned_r);
    }

    #[test]
    fn test_brush_follow_path_dynamics() {
        let mut layer = Layer::new_paint_layer(1, "Paint".to_string(), 64, 64);
        let mut engine = BrushEngine::new();
        engine.settings.radius = 4.0;
        engine.settings.spacing = 0.5;
        engine.settings.follow_path = true;
        engine.settings.follow_path_offset = 0.25;
        engine.update_cache();

        assert_eq!(engine.follow_path_angle, 0.0);

        // 2D stroke: move along +Y direction (delta.x = 0, delta.y = 0.4)
        engine.paint_stroke(&mut layer, Vec2::new(0.3, 0.1));
        engine.paint_stroke(&mut layer, Vec2::new(0.3, 0.5));

        // Angle should be atan2(0.4, 0.0) + 0.25 = PI/2 + 0.25
        let expected_angle = std::f32::consts::FRAC_PI_2 + 0.25;
        assert!(
            (engine.follow_path_angle - expected_angle).abs() < 1e-3,
            "Expected follow path angle ~ {}, got {}",
            expected_angle,
            engine.follow_path_angle
        );

        // 3D stroke: move on XY plane with +Z normal
        engine.end_stroke();
        assert_eq!(engine.follow_path_angle, 0.0, "end_stroke must reset follow_path_angle");

        let mesh = crate::mesh::Mesh::create_cube_mesh("TestCube", 1.0);
        engine.paint_stroke_3d(&mut layer, &mesh, Vec3::new(0.0, 0.0, 0.5), Vec3::Z, 0.1);
        engine.paint_stroke_3d(&mut layer, &mesh, Vec3::new(0.5, 0.0, 0.5), Vec3::Z, 0.1);
        assert!(
            engine.follow_path_angle.abs() > 1e-4,
            "3D paint stroke should compute tangent angle for follow path, got {}",
            engine.follow_path_angle
        );

        engine.end_stroke();
        assert_eq!(engine.follow_path_angle, 0.0);
    }
}