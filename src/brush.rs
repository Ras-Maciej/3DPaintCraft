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
    Path,       // Vector Path Tool (7 / T)
    Stamp,      // Decal Stamp Tool (8 / D)
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
            Self::Path => "Path Tool [7/T]",
            Self::Stamp => "Stamp Tool [8/D]",
        }
    }
}

/// A 3D/2D control point on the spline path
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathControlPoint {
    pub position_3d: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    #[serde(default = "default_radius_scale")]
    pub radius_scale: f32,
}

fn default_radius_scale() -> f32 {
    1.0
}

/// Stroke generation style for the Substance 3D Painter-style Vector Path Tool
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathStrokeStyle {
    Continuous, // Standard continuous brush stamps
    WeldSeam,   // Industrial TIG/MIG weld puddle ripple beads with temper heat tint
    RopeBraid,  // Multi-strand twisted helical cord / rope
    StitchSeam, // Alternating textile thread stitches with puncture holes
    ChainLinks, // Interlocking chain links
    SmartDecal, // Discrete hardware decal stamps (uses SmartDecalType)
}

impl Default for PathStrokeStyle {
    fn default() -> Self {
        Self::Continuous
    }
}

impl PathStrokeStyle {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Continuous => "Standard Brush",
            Self::WeldSeam => "TIG Weld Seam",
            Self::RopeBraid => "Twisted Rope",
            Self::StitchSeam => "Stitch Seam",
            Self::ChainLinks => "Chain Links",
            Self::SmartDecal => "Smart Decal Stamp",
        }
    }
}

/// Specialized hardware decal and fastener stamp types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmartDecalType {
    ButtonHole,    // Tailored fabric button slit with perimeter bar-tack embroidery
    PhillipsScrew, // Machine screw with 4-point cross (+) drive recess
    FlatheadScrew, // Slotted machine screw head (-)
    TorxScrew,     // 6-point star drive recess screw
    HexBolt,       // Industrial hexagonal bolt head
    DomedRivet,    // Hemispherical structural aircraft rivet with compression ring
    GrommetEyelet, // Metallic ring with center fabric cutout hole
    AirVentLouver, // Mechanical cooling ventilation slats
}

impl Default for SmartDecalType {
    fn default() -> Self {
        Self::PhillipsScrew
    }
}

impl SmartDecalType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ButtonHole => "Button Hole & Bar-Tack",
            Self::PhillipsScrew => "Phillips Machine Screw",
            Self::FlatheadScrew => "Flathead Slotted Screw",
            Self::TorxScrew => "Torx Star Screw",
            Self::HexBolt => "Hexagonal Bolt Head",
            Self::DomedRivet => "Structural Domed Rivet",
            Self::GrommetEyelet => "Metal Grommet Eyelet",
            Self::AirVentLouver => "Air Vent Louver",
        }
    }
}

/// Evaluated PBR material properties for a procedural decal or path stroke style
#[derive(Clone, Copy, Debug)]
pub struct DecalPixel {
    pub color: Vec4,
    pub height: f32,
    pub roughness: f32,
    pub metallic: f32,
    pub opacity: f32,
    pub normal: Vec3, // Tangent-space analytical normal vector
    pub alpha: f32, // Footprint weight (0.0 to 1.0)
}

/// Evaluates multi-channel PBR material properties for a procedural decal or path stroke style
pub fn evaluate_decal_pixel(
    style: PathStrokeStyle,
    decal: SmartDecalType,
    u: f32, // Along tangent (-1.0 to 1.0)
    v: f32, // Across bitangent (-1.0 to 1.0)
    base_color: Vec4,
    heat_tint: bool,
    arc_length: f32,
    world_radius: f32,
    height_depth: f32,
) -> DecalPixel {
    let r2 = u * u + v * v;
    if r2 > 1.05 {
        return DecalPixel {
            color: base_color,
            height: 0.5,
            roughness: 0.5,
            metallic: 0.0,
            opacity: 1.0,
            normal: Vec3::Z,
            alpha: 0.0,
        };
    }
    let r = r2.sqrt();
    let theta = v.atan2(u);

    let (col, raw_h, rough, metal, opac, raw_n, alpha) = match style {
        PathStrokeStyle::Continuous => {
            let edge = (1.0 - r).clamp(0.0, 1.0);
            let alpha = edge * edge * (3.0 - 2.0 * edge);
            let h = 0.5 + 0.3 * (1.0 - r * r).max(0.0).sqrt();
            let norm = Vec3::new(-u * 0.75, -v * 0.75, (1.0 - r * r).max(0.04).sqrt()).normalize();
            (base_color, h, 0.35, 0.0, 1.0, norm, alpha)
        }
        PathStrokeStyle::WeldSeam => {
            // Overlapping scalloped weld puddle ripple beads
            let bead_scale = (world_radius * 0.45).max(0.01);
            let bead_phase = (arc_length / bead_scale + u * 1.8) * std::f32::consts::PI * 2.0;
            let v_norm = (v / 0.85).clamp(-1.0, 1.0);
            let ripple_prof = (1.0 - v_norm * v_norm).max(0.0).sqrt();
            let ripple = ripple_prof * (0.8 + 0.32 * bead_phase.cos());
            let h = (0.5 + 0.38 * ripple).clamp(0.0, 1.0);

            let dh_dv = -0.38 * v_norm * (0.8 + 0.32 * bead_phase.cos());
            let dh_du = -0.38 * ripple_prof * 0.32 * bead_phase.sin() * 1.8 * std::f32::consts::PI * 2.0;
            let norm = Vec3::new(-dh_du * 0.4, -dh_dv * 0.5, 1.0).normalize();

            // Thermal oxidation colors across distance from center bead
            let abs_v = v.abs();
            let (col, rough, metal) = if !heat_tint || abs_v < 0.40 {
                // Molten metallic steel center puddle
                (Vec4::new(0.80, 0.82, 0.85, 1.0), 0.22, 0.95)
            } else if abs_v < 0.60 {
                // Straw yellow temper band
                let t = (abs_v - 0.40) / 0.20;
                let c = Vec4::new(0.80, 0.82, 0.85, 1.0).lerp(Vec4::new(0.88, 0.74, 0.32, 1.0), t);
                (c, 0.35, 0.85)
            } else if abs_v < 0.80 {
                // Purple / Magenta temper band
                let t = (abs_v - 0.60) / 0.20;
                let c = Vec4::new(0.88, 0.74, 0.32, 1.0).lerp(Vec4::new(0.62, 0.24, 0.62, 1.0), t);
                (c, 0.48, 0.70)
            } else {
                // Iridescent dark blue temper fringe
                let t = ((abs_v - 0.80) / 0.20).clamp(0.0, 1.0);
                let c = Vec4::new(0.62, 0.24, 0.62, 1.0).lerp(Vec4::new(0.20, 0.38, 0.82, 1.0), t);
                (c, 0.65, 0.55)
            };

            let edge_falloff = ((1.0 - abs_v) / 0.20).clamp(0.0, 1.0);
            let u_falloff = ((1.0 - u.abs()) / 0.25).clamp(0.0, 1.0);
            let alpha = (edge_falloff * u_falloff).clamp(0.0, 1.0);

            (col, h, rough, metal, 1.0, norm, alpha)
        }
        PathStrokeStyle::RopeBraid => {
            // 3-strand helical twisted rope / cord
            let twist_rate = (world_radius * 0.75).max(0.01);
            let phi = (arc_length / twist_rate) * std::f32::consts::PI * 2.0 + u * 1.5;
            let v0 = 0.42 * phi.sin();
            let v1 = 0.42 * (phi + 2.094395).sin();
            let v2 = 0.42 * (phi + 4.188790).sin();

            let d0 = (v - v0).abs() / 0.42;
            let d1 = (v - v1).abs() / 0.42;
            let d2 = (v - d2_v_calc(v2)).abs() / 0.42;

            let (closest_strand, min_d, v_center) = if d0 <= d1 && d0 <= d2 {
                (0, d0, v0)
            } else if d1 <= d2 {
                (1, d1, v1)
            } else {
                (2, d2, v2)
            };

            let h0 = (1.0 - d0 * d0).max(0.0).sqrt();
            let h1 = (1.0 - d1 * d1).max(0.0).sqrt();
            let h2 = (1.0 - d2 * d2).max(0.0).sqrt();
            let max_h = h0.max(h1).max(h2);

            let h = (0.42 + 0.44 * max_h).clamp(0.0, 1.0);
            let cord_col = Vec4::new(0.72, 0.62, 0.48, 1.0).lerp(base_color, 0.25);
            let edge_falloff = ((1.0 - v.abs() / 0.90) / 0.15).clamp(0.0, 1.0);
            let u_falloff = ((1.0 - u.abs() / 0.95) / 0.15).clamp(0.0, 1.0);
            let alpha = (max_h.min(1.0) * edge_falloff * u_falloff).clamp(0.0, 1.0);

            let dv = (v - v_center) / 0.42;
            let strand_phi = phi + match closest_strand {
                0 => 0.0,
                1 => 2.094395,
                _ => 4.188790,
            };
            let du = -0.42 * strand_phi.cos() * 1.5;
            let norm = Vec3::new(-du * dv * 0.5, -dv * 0.85, (1.0 - min_d * min_d).max(0.04).sqrt()).normalize();

            (cord_col, h, 0.88, 0.0, 1.0, norm, alpha)
        }
        PathStrokeStyle::StitchSeam => {
            // Alternating textile thread stitches with puncture holes
            let stitch_scale = (world_radius * 1.3).max(0.01);
            let cycle = (arc_length / stitch_scale).rem_euclid(1.0);

            if cycle >= 0.12 && cycle <= 0.88 {
                let t_thread = (cycle - 0.5) / 0.38;
                let v_thread = v / 0.35;
                let h_profile = (1.0 - v_thread * v_thread).max(0.0).sqrt() * (1.0 - t_thread * t_thread).max(0.0).sqrt();
                let h = 0.5 + 0.34 * h_profile;
                let alpha = (h_profile * 2.0).clamp(0.0, 1.0);
                let norm = Vec3::new(-t_thread * 0.5, -v_thread * 0.85, (1.0 - v_thread * v_thread).max(0.04).sqrt()).normalize();
                (base_color, h, 0.82, 0.0, 1.0, norm, alpha)
            } else {
                let hole_dist = (v * v + (u * 2.0).powi(2)).sqrt();
                let h = 0.5 - 0.35 * (1.0 - hole_dist / 0.35).max(0.0);
                let alpha = (1.0 - hole_dist / 0.35).max(0.0);
                let norm = Vec3::new(u * 2.2, v * 2.2, 1.0).normalize();
                (Vec4::new(0.12, 0.12, 0.14, 1.0), h, 0.95, 0.0, 1.0, norm, alpha)
            }
        }
        PathStrokeStyle::ChainLinks => {
            let chain_scale = (world_radius * 1.5).max(0.01);
            let cycle = (arc_length / chain_scale).rem_euclid(2.0);
            if cycle < 1.0 {
                let u_ring = (cycle - 0.5) / 0.45;
                let v_ring = v / 0.55;
                let r_ring = (u_ring * u_ring + v_ring * v_ring).sqrt().max(1e-4);
                let d_ring = ((r_ring - 0.70) / 0.30).abs();
                let h_ring = (1.0 - d_ring * d_ring).max(0.0).sqrt();
                let alpha = (h_ring * 2.5).clamp(0.0, 1.0);
                let norm = Vec3::new(-(u_ring / r_ring) * d_ring * 0.85, -(v_ring / r_ring) * d_ring * 0.85, (1.0 - d_ring * d_ring).max(0.04).sqrt()).normalize();
                (Vec4::new(0.78, 0.79, 0.82, 1.0), 0.5 + 0.30 * h_ring, 0.28, 0.94, 1.0, norm, alpha)
            } else {
                let v_bar = v / 0.28;
                let h_bar = (1.0 - v_bar * v_bar).max(0.0).sqrt();
                let alpha = (h_bar * 2.5).clamp(0.0, 1.0);
                let norm = Vec3::new(0.0, -v_bar * 0.85, (1.0 - v_bar * v_bar).max(0.04).sqrt()).normalize();
                (Vec4::new(0.78, 0.79, 0.82, 1.0), 0.5 + 0.42 * h_bar, 0.28, 0.94, 1.0, norm, alpha)
            }
        }
        PathStrokeStyle::SmartDecal => {
            match decal {
                SmartDecalType::ButtonHole => {
                    let inside_slit = u.abs() < 0.65 && v.abs() < 0.08;
                    if inside_slit {
                        let v_slit = (v / 0.08).clamp(-1.0, 1.0);
                        let norm = Vec3::new(0.0, -v_slit * 0.90, 0.43).normalize();
                        (Vec4::new(0.08, 0.08, 0.09, 1.0), 0.12, 0.95, 0.0, 1.0, norm, 1.0)
                    } else if u.abs() <= 0.82 && v.abs() <= 0.36 {
                        let zigzag = (u * 32.0).sin() * 0.14;
                        let h_edge = (1.0 - (v / 0.36).powi(2)).max(0.0).sqrt();
                        let h = 0.5 + 0.26 * h_edge + zigzag;
                        let alpha = ((1.0 - u.abs() / 0.82) * 4.0).clamp(0.0, 1.0).min(h_edge * 2.0);
                        let v_stitch = (v / 0.36).clamp(-1.0, 1.0);
                        let u_stitch = (u * 32.0).cos() * 0.35;
                        let norm = Vec3::new(-u_stitch, -v_stitch * 0.80, 0.8).normalize();
                        (base_color, h, 0.85, 0.0, 1.0, norm, alpha)
                    } else {
                        (base_color, 0.5, 0.5, 0.0, 1.0, Vec3::Z, 0.0)
                    }
                }
                SmartDecalType::PhillipsScrew => {
                    if r > 0.94 {
                        (base_color, 0.5, 0.5, 0.0, 1.0, Vec3::Z, 0.0)
                    } else {
                        let h_head = 0.5 + 0.32 * (1.0 - (r / 0.94).powi(2)).max(0.0).sqrt();
                        let is_cross = (u.abs() < 0.11 && v.abs() < 0.52) || (v.abs() < 0.11 && u.abs() < 0.52);
                        let (col, h, r_val, m_val, norm) = if is_cross {
                            let n = if u.abs() < 0.11 && v.abs() < 0.52 {
                                let s = if u >= 0.0 { 1.0 } else { -1.0 };
                                Vec3::new(-s * 0.88, 0.0, 0.47).normalize()
                            } else if v.abs() < 0.11 && u.abs() < 0.52 {
                                let s = if v >= 0.0 { 1.0 } else { -1.0 };
                                Vec3::new(0.0, -s * 0.88, 0.47).normalize()
                            } else {
                                Vec3::Z
                            };
                            (Vec4::new(0.18, 0.18, 0.20, 1.0), 0.18, 0.55, 0.85, n)
                        } else {
                            let n = Vec3::new(-u / 0.94, -v / 0.94, (1.0 - (r / 0.94).powi(2)).max(0.04).sqrt()).normalize();
                            (Vec4::new(0.82, 0.83, 0.86, 1.0), h_head, 0.24, 0.95, n)
                        };
                        let alpha = ((0.94 - r) / 0.06).clamp(0.0, 1.0);
                        (col, h, r_val, m_val, 1.0, norm, alpha)
                    }
                }
                SmartDecalType::FlatheadScrew => {
                    if r > 0.94 {
                        (base_color, 0.5, 0.5, 0.0, 1.0, Vec3::Z, 0.0)
                    } else {
                        let h_head = 0.5 + 0.32 * (1.0 - (r / 0.94).powi(2)).max(0.0).sqrt();
                        let is_slot = v.abs() < 0.11 && u.abs() < 0.72;
                        let (col, h, r_val, m_val, norm) = if is_slot {
                            let s = if v >= 0.0 { 1.0 } else { -1.0 };
                            let n = Vec3::new(0.0, -s * 0.90, 0.43).normalize();
                            (Vec4::new(0.18, 0.18, 0.20, 1.0), 0.18, 0.55, 0.85, n)
                        } else {
                            let n = Vec3::new(-u / 0.94, -v / 0.94, (1.0 - (r / 0.94).powi(2)).max(0.04).sqrt()).normalize();
                            (Vec4::new(0.82, 0.83, 0.86, 1.0), h_head, 0.24, 0.95, n)
                        };
                        let alpha = ((0.94 - r) / 0.06).clamp(0.0, 1.0);
                        (col, h, r_val, m_val, 1.0, norm, alpha)
                    }
                }
                SmartDecalType::TorxScrew => {
                    if r > 0.94 {
                        (base_color, 0.5, 0.5, 0.0, 1.0, Vec3::Z, 0.0)
                    } else {
                        let h_head = 0.5 + 0.32 * (1.0 - (r / 0.94).powi(2)).max(0.0).sqrt();
                        let r_star = 0.36 + 0.12 * (6.0 * theta).cos();
                        let is_star = r < r_star;
                        let (col, h, r_val, m_val, norm) = if is_star {
                            let n = Vec3::new(-theta.cos() * 0.88, -theta.sin() * 0.88, 0.47).normalize();
                            (Vec4::new(0.18, 0.18, 0.20, 1.0), 0.18, 0.55, 0.85, n)
                        } else {
                            let n = Vec3::new(-u / 0.94, -v / 0.94, (1.0 - (r / 0.94).powi(2)).max(0.04).sqrt()).normalize();
                            (Vec4::new(0.82, 0.83, 0.86, 1.0), h_head, 0.24, 0.95, n)
                        };
                        let alpha = ((0.94 - r) / 0.06).clamp(0.0, 1.0);
                        (col, h, r_val, m_val, 1.0, norm, alpha)
                    }
                }
                SmartDecalType::HexBolt => {
                    let pi_div_3 = std::f32::consts::PI / 3.0;
                    let hex_angle = (theta.rem_euclid(pi_div_3) - std::f32::consts::PI / 6.0).cos().max(0.01);
                    let r_hex = 0.82 / hex_angle;

                    if r > r_hex + 0.06 {
                        (base_color, 0.5, 0.5, 0.0, 1.0, Vec3::Z, 0.0)
                    } else {
                        let h_base = 0.5 + 0.35 * (1.0 - (r / r_hex).min(1.0) * 0.35);
                        let h = if r < 0.32 { h_base - 0.12 } else { h_base };
                        let norm = if r > r_hex * 0.65 {
                            Vec3::new(-theta.cos() * 0.707, -theta.sin() * 0.707, 0.707).normalize()
                        } else if r < 0.32 {
                            Vec3::new(theta.cos() * 0.6, theta.sin() * 0.6, 0.8).normalize()
                        } else {
                            Vec3::Z
                        };
                        let alpha = ((r_hex + 0.06 - r) / 0.06).clamp(0.0, 1.0);
                        (Vec4::new(0.76, 0.77, 0.80, 1.0), h, 0.26, 0.95, 1.0, norm, alpha)
                    }
                }
                SmartDecalType::DomedRivet => {
                    if r > 0.96 {
                        (base_color, 0.5, 0.5, 0.0, 1.0, Vec3::Z, 0.0)
                    } else {
                        let (h, col, m_val, r_val, norm) = if r <= 0.82 {
                            let dome_h = 0.5 + 0.42 * (1.0 - (r / 0.82).powi(2)).max(0.0).sqrt();
                            let n = Vec3::new(-u / 0.82, -v / 0.82, (1.0 - (r / 0.82).powi(2)).max(0.04).sqrt()).normalize();
                            (dome_h, Vec4::new(0.78, 0.80, 0.83, 1.0), 0.92, 0.28, n)
                        } else {
                            let n = Vec3::new(theta.cos() * 0.35, theta.sin() * 0.35, 0.9).normalize();
                            (0.40, Vec4::new(0.45, 0.47, 0.50, 1.0), 0.75, 0.55, n)
                        };
                        let alpha = ((0.96 - r) / 0.08).clamp(0.0, 1.0);
                        (col, h, r_val, m_val, 1.0, norm, alpha)
                    }
                }
                SmartDecalType::GrommetEyelet => {
                    if r > 0.94 {
                        (base_color, 0.5, 0.5, 0.0, 1.0, Vec3::Z, 0.0)
                    } else {
                        let (h, col, m_val, r_val, opac, norm) = if r <= 0.38 {
                            (0.12, Vec4::new(0.06, 0.06, 0.08, 1.0), 0.0, 0.95, 0.0, Vec3::Z)
                        } else {
                            let ring_d = ((r - 0.65) / 0.27).clamp(-1.0, 1.0);
                            let ring_h = 0.5 + 0.36 * (1.0 - ring_d * ring_d).max(0.0).sqrt();
                            let n = Vec3::new(-theta.cos() * ring_d * 0.85, -theta.sin() * ring_d * 0.85, (1.0 - ring_d * ring_d).max(0.04).sqrt()).normalize();
                            let brass_col = Vec4::new(0.86, 0.74, 0.40, 1.0);
                            (ring_h, brass_col, 0.95, 0.22, 1.0, n)
                        };
                        let alpha = ((0.94 - r) / 0.06).clamp(0.0, 1.0);
                        (col, h, r_val, m_val, opac, norm, alpha)
                    }
                }
                SmartDecalType::AirVentLouver => {
                    if u.abs() > 0.88 || v.abs() > 0.88 {
                        (base_color, 0.5, 0.5, 0.0, 1.0, Vec3::Z, 0.0)
                    } else {
                        let slat_v = (v * 2.5 + 10.0).rem_euclid(1.0);
                        let (h, col, r_val, norm) = if slat_v < 0.82 {
                            let ramp_h = 0.5 + 0.32 * slat_v;
                            let n = Vec3::new(0.0, -0.574, 0.819).normalize();
                            (ramp_h, Vec4::new(0.72, 0.74, 0.77, 1.0), 0.32, n)
                        } else {
                            let n = Vec3::new(0.0, 0.85, 0.52).normalize();
                            (0.20, Vec4::new(0.12, 0.12, 0.14, 1.0), 0.85, n)
                        };
                        let border_u = ((0.88 - u.abs()) / 0.08).clamp(0.0, 1.0);
                        let border_v = ((0.88 - v.abs()) / 0.08).clamp(0.0, 1.0);
                        let alpha = border_u * border_v;
                        (col, h, r_val, 0.88, 1.0, norm, alpha)
                    }
                }
            }
        }
    };

    let final_h = (0.5 + (raw_h - 0.5) * height_depth).clamp(0.0, 1.0);
    let final_n = Vec3::new(raw_n.x * height_depth, raw_n.y * height_depth, raw_n.z).normalize_or_zero();
    let norm_res = if final_n.length_squared() > 1e-4 { final_n } else { Vec3::Z };

    DecalPixel {
        color: col,
        height: final_h,
        roughness: rough,
        metallic: metal,
        opacity: opac,
        normal: norm_res,
        alpha,
    }
}

#[inline(always)]
fn d2_v_calc(v2: f32) -> f32 {
    v2
}

/// A continuous stamp position along the evaluated path curve
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathSample {
    pub position_3d: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub tangent_angle: f32, // in radians
    pub radius_scale: f32,
    #[serde(default)]
    pub arc_length: f32,
}

/// State of the Substance 3D Painter-style Vector Path Tool
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathToolState {
    pub points: Vec<PathControlPoint>,
    pub closed: bool,
    pub spacing_ratio: f32, // Spacing as fraction of brush radius (default 0.35)
    pub follow_tangent: bool, // Align stamp rotation to path tangent (default true)
    pub taper_start: f32, // Taper at start of path (0.0 to 1.0, default 1.0)
    pub taper_end: f32, // Taper at end of path (0.0 to 1.0, default 1.0)
    pub selected_point: Option<usize>,
    pub is_dragging: bool,
    #[serde(default)]
    pub is_sketching: bool,
    #[serde(default)]
    pub stroke_style: PathStrokeStyle,
    #[serde(default)]
    pub decal_type: SmartDecalType,
    #[serde(default = "default_decal_spacing")]
    pub decal_spacing: f32,
    #[serde(default = "default_heat_tint")]
    pub heat_tint: bool,
    #[serde(default = "default_surface_offset")]
    pub surface_offset: f32,
    #[serde(default = "default_height_depth")]
    pub height_depth: f32,
    #[serde(default = "default_snap_to_surface")]
    pub snap_to_surface: bool,
}

fn default_decal_spacing() -> f32 {
    1.2
}

fn default_heat_tint() -> bool {
    true
}

fn default_surface_offset() -> f32 {
    0.003
}

fn default_height_depth() -> f32 {
    1.0
}

fn default_snap_to_surface() -> bool {
    true
}

impl Default for PathToolState {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            closed: false,
            spacing_ratio: 0.35,
            follow_tangent: true,
            taper_start: 1.0,
            taper_end: 1.0,
            selected_point: None,
            is_dragging: false,
            is_sketching: false,
            stroke_style: PathStrokeStyle::Continuous,
            decal_type: SmartDecalType::PhillipsScrew,
            decal_spacing: 1.2,
            heat_tint: true,
            surface_offset: 0.003,
            height_depth: 1.0,
            snap_to_surface: true,
        }
    }
}

/// State of the Substance 3D Painter-style Decal Stamp Tool
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StampToolState {
    pub stroke_style: PathStrokeStyle,
    pub decal_type: SmartDecalType,
    pub rotation_angle: f32, // Degrees 0.0 to 360.0
    pub randomize_rotation: bool,
    pub height_depth: f32,
    pub heat_tint: bool,
    pub surface_offset: f32,
    pub selected_preset_id: Option<String>,
}

impl Default for StampToolState {
    fn default() -> Self {
        Self {
            stroke_style: PathStrokeStyle::SmartDecal,
            decal_type: SmartDecalType::PhillipsScrew,
            rotation_angle: 0.0,
            randomize_rotation: false,
            height_depth: 1.0,
            heat_tint: true,
            surface_offset: 0.003,
            selected_preset_id: None,
        }
    }
}

impl PathToolState {
    pub fn clear(&mut self) {
        self.points.clear();
        self.selected_point = None;
        self.is_dragging = false;
        self.is_sketching = false;
    }

    /// Project spline samples onto the true 3D mesh surface so chords never dip inside curved geometry
    pub fn project_samples_to_mesh(samples: &mut [PathSample], mesh: &crate::mesh::Mesh, world_radius: f32, offset: f32) {
        for sample in samples.iter_mut() {
            let ray_dist = (world_radius * 4.0).max(0.20);
            let ray = crate::mesh::Ray {
                origin: sample.position_3d + sample.normal * ray_dist,
                direction: -sample.normal,
            };
            if let Some(hit) = mesh.raycast(&ray) {
                sample.position_3d = hit.point + hit.normal * offset;
                sample.normal = hit.normal;
            } else if let Some((p, n)) = mesh.uv_to_3d_point(sample.uv) {
                sample.position_3d = p + n * offset;
                sample.normal = n;
            } else {
                sample.position_3d += sample.normal * offset;
            }
        }
    }

    /// Evaluates smooth Catmull-Rom spline curves across control points and returns
    /// a dense polyline sequence of curve points for rendering the vector line itself.
    pub fn evaluate_spline_curve(&self, sub_steps_per_segment: usize) -> Vec<PathSample> {
        let n = self.points.len();
        if n == 0 {
            return Vec::new();
        }
        if n == 1 {
            let pt = &self.points[0];
            return vec![PathSample {
                position_3d: pt.position_3d,
                normal: pt.normal,
                uv: pt.uv,
                tangent_angle: 0.0,
                radius_scale: pt.radius_scale * self.taper_start,
                arc_length: 0.0,
            }];
        }

        let num_segments = if self.closed { n } else { n - 1 };
        let steps = sub_steps_per_segment.max(8);
        let mut curve = Vec::with_capacity(num_segments * steps + 1);
        let mut cur_dist = 0.0f32;
        let mut prev_pos: Option<Vec3> = None;

        for seg in 0..num_segments {
            let p1 = &self.points[seg];
            let p2 = &self.points[(seg + 1) % n];

            let p0 = if seg == 0 {
                if self.closed {
                    &self.points[n - 1]
                } else {
                    p1
                }
            } else {
                &self.points[seg - 1]
            };

            let p3 = if seg + 1 >= num_segments {
                if self.closed {
                    &self.points[(seg + 2) % n]
                } else {
                    p2
                }
            } else {
                &self.points[(seg + 2) % n]
            };

            for step in 0..steps {
                let t = step as f32 / steps as f32;
                let t2 = t * t;
                let t3 = t2 * t;

                // Catmull-Rom spline basis
                let b0 = -t3 + 2.0 * t2 - t;
                let b1 = 3.0 * t3 - 5.0 * t2 + 2.0;
                let b2 = -3.0 * t3 + 4.0 * t2 + t;
                let b3 = t3 - t2;

                let pos = 0.5 * (p0.position_3d * b0 + p1.position_3d * b1 + p2.position_3d * b2 + p3.position_3d * b3);
                let norm = (0.5 * (p0.normal * b0 + p1.normal * b1 + p2.normal * b2 + p3.normal * b3)).normalize_or_zero();
                let uv = 0.5 * (p0.uv * b0 + p1.uv * b1 + p2.uv * b2 + p3.uv * b3);

                // Derivative for tangent vector
                let d0 = -3.0 * t2 + 4.0 * t - 1.0;
                let d1 = 9.0 * t2 - 10.0 * t;
                let d2 = -9.0 * t2 + 8.0 * t + 1.0;
                let d3 = 3.0 * t2 - 2.0 * t;

                let d_uv = 0.5 * (p0.uv * d0 + p1.uv * d1 + p2.uv * d2 + p3.uv * d3);
                let tangent_angle = d_uv.y.atan2(d_uv.x);

                let r_scale = p1.radius_scale * (1.0 - t) + p2.radius_scale * t;

                if let Some(prev) = prev_pos {
                    cur_dist += (pos - prev).length();
                }
                prev_pos = Some(pos);

                curve.push(PathSample {
                    position_3d: pos,
                    normal: norm,
                    uv,
                    tangent_angle,
                    radius_scale: r_scale,
                    arc_length: cur_dist,
                });
            }
        }

        // Add final point for open curves
        if !self.closed {
            let last = &self.points[n - 1];
            let prev = &self.points[n - 2];
            let d_uv = last.uv - prev.uv;
            if let Some(prev_p) = prev_pos {
                cur_dist += (last.position_3d - prev_p).length();
            }
            curve.push(PathSample {
                position_3d: last.position_3d,
                normal: last.normal,
                uv: last.uv,
                tangent_angle: d_uv.y.atan2(d_uv.x),
                radius_scale: last.radius_scale,
                arc_length: cur_dist,
            });
        }

        curve
    }

    /// Evaluates smooth Catmull-Rom spline curves across control points and returns
    /// sampled stroke points spaced evenly by (world_radius * spacing_ratio).
    pub fn sample_path(&self, brush_radius: f32) -> Vec<PathSample> {
        let n = self.points.len();
        if n == 0 {
            return Vec::new();
        }
        if n == 1 {
            let pt = &self.points[0];
            return vec![PathSample {
                position_3d: pt.position_3d,
                normal: pt.normal,
                uv: pt.uv,
                tangent_angle: 0.0,
                radius_scale: pt.radius_scale * self.taper_start,
                arc_length: 0.0,
            }];
        }

        // Auto-scale radius if passed in pixel coordinates (> 1.0)
        let eff_radius = if brush_radius > 1.0 {
            (brush_radius / 1024.0) * 2.0
        } else {
            brush_radius
        }.clamp(0.002, 1.0);

        let dense_eval = self.evaluate_spline_curve(32);
        if dense_eval.is_empty() {
            return Vec::new();
        }

        let total_len = dense_eval.last().map(|s| s.arc_length).unwrap_or(0.0);
        if total_len < 1e-4 {
            let s0 = &dense_eval[0];
            return vec![PathSample {
                position_3d: s0.position_3d,
                normal: s0.normal,
                uv: s0.uv,
                tangent_angle: s0.tangent_angle,
                radius_scale: s0.radius_scale * self.taper_start,
                arc_length: 0.0,
            }];
        }

        let step_dist = match self.stroke_style {
            PathStrokeStyle::SmartDecal => (eff_radius * self.decal_spacing.max(0.25) * 2.0).max(0.005),
            PathStrokeStyle::WeldSeam => (eff_radius * 0.22).max(0.002),
            PathStrokeStyle::RopeBraid => (eff_radius * 0.15).max(0.002),
            PathStrokeStyle::StitchSeam => (eff_radius * 0.25).max(0.002),
            PathStrokeStyle::ChainLinks => (eff_radius * 0.35).max(0.002),
            PathStrokeStyle::Continuous => (eff_radius * self.spacing_ratio.max(0.05)).max(0.002),
        };

        let mut samples = Vec::new();
        let mut cur_dist = 0.0f32;
        let mut dense_idx = 0;

        while cur_dist <= total_len {
            while dense_idx + 1 < dense_eval.len() && dense_eval[dense_idx + 1].arc_length < cur_dist {
                dense_idx += 1;
            }

            let (pos, norm, uv, angle, base_r) = if dense_idx + 1 < dense_eval.len() {
                let d0 = dense_eval[dense_idx].arc_length;
                let d1 = dense_eval[dense_idx + 1].arc_length;
                let frac = if (d1 - d0).abs() > 1e-5 {
                    ((cur_dist - d0) / (d1 - d0)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let a = &dense_eval[dense_idx];
                let b = &dense_eval[dense_idx + 1];
                (
                    a.position_3d.lerp(b.position_3d, frac),
                    a.normal.lerp(b.normal, frac).normalize_or_zero(),
                    a.uv.lerp(b.uv, frac),
                    a.tangent_angle,
                    a.radius_scale * (1.0 - frac) + b.radius_scale * frac,
                )
            } else {
                let last = &dense_eval[dense_eval.len() - 1];
                (last.position_3d, last.normal, last.uv, last.tangent_angle, last.radius_scale)
            };

            let norm_pos = (cur_dist / total_len).clamp(0.0, 1.0);
            let taper = if norm_pos < 0.25 {
                let f = norm_pos / 0.25;
                self.taper_start * (1.0 - f) + 1.0 * f
            } else if norm_pos > 0.75 {
                let f = (norm_pos - 0.75) / 0.25;
                1.0 * (1.0 - f) + self.taper_end * f
            } else {
                1.0
            };

            samples.push(PathSample {
                position_3d: pos,
                normal: norm,
                uv,
                tangent_angle: angle,
                radius_scale: base_r * taper,
                arc_length: cur_dist,
            });

            cur_dist += step_dist;
        }

        samples
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
    #[serde(default)]
    pub pressure_hardness: bool,
    #[serde(default = "default_min_pressure_size")]
    pub min_pressure_size: f32,
    #[serde(default = "default_min_pressure_opacity")]
    pub min_pressure_opacity: f32,
    #[serde(default = "default_min_pressure_hardness")]
    pub min_pressure_hardness: f32,
    #[serde(default)]
    pub pressure_curve: PressureCurve,

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

    // Vector Path Tool Settings
    #[serde(default)]
    pub path_tool: PathToolState,

    // Decal Stamp Tool Settings
    #[serde(default)]
    pub stamp_tool: StampToolState,
}

fn default_smudge_strength() -> f32 {
    0.5
}

fn default_min_pressure_size() -> f32 {
    0.1
}

fn default_min_pressure_opacity() -> f32 {
    0.05
}

fn default_min_pressure_hardness() -> f32 {
    0.2
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressureCurve {
    Linear,
    Soft,
    Firm,
}

impl Default for PressureCurve {
    fn default() -> Self {
        Self::Linear
    }
}

impl PressureCurve {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Soft => "Soft (Sensitive)",
            Self::Firm => "Firm (Controlled)",
        }
    }

    pub fn apply(&self, raw: f32) -> f32 {
        let p = raw.clamp(0.0, 1.0);
        match self {
            Self::Linear => p,
            Self::Soft => p.powf(0.6),
            Self::Firm => p.powf(1.7),
        }
    }
}

impl BrushSettings {
    /// Calculate effective radius, flow, and hardness modulated by hardware stylus pressure dynamics.
    pub fn compute_effective_dynamics(&self, raw_pressure: f32) -> (f32, f32, f32) {
        let p = self.pressure_curve.apply(raw_pressure);
        let eff_radius = if self.pressure_size {
            self.radius * (self.min_pressure_size + (1.0 - self.min_pressure_size) * p)
        } else {
            self.radius
        };
        let eff_flow = if self.pressure_opacity {
            self.flow * (self.min_pressure_opacity + (1.0 - self.min_pressure_opacity) * p)
        } else {
            self.flow
        };
        let eff_hardness = if self.pressure_hardness {
            self.hardness * (self.min_pressure_hardness + (1.0 - self.min_pressure_hardness) * p)
        } else {
            self.hardness
        };
        (eff_radius, eff_flow, eff_hardness)
    }
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
            pressure_hardness: false,
            min_pressure_size: 0.1,
            min_pressure_opacity: 0.05,
            min_pressure_hardness: 0.2,
            pressure_curve: PressureCurve::Linear,
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
            path_tool: PathToolState::default(),
            stamp_tool: StampToolState::default(),
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
        
        layer.ensure_allocated_for_paint(self.settings.target, self.settings.channels, is_eraser);
        if is_smudge || is_clone {
            layer.base_color_buf.ensure_allocated();
        }

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
            let cx = center_x.clamp(0, w - 1) as u32;
            let cy = center_y.clamp(0, h - 1) as u32;
            let cur_col = layer.base_color_buf.get_pixel(cx, cy);
            let cur_r = layer.roughness_buf.get_pixel(cx, cy);
            let cur_m = layer.metallic_buf.get_pixel(cx, cy);
            let cur_h = layer.height_buf.get_pixel(cx, cy);
            let cur_emis = layer.emissive_buf.get_pixel(cx, cy);

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

        layer.record_tiles_for_rect(rect_min_x, rect_min_y, rect_max_x, rect_max_y);

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
                    let src_x = (scx + (px - center_x)).rem_euclid(w) as u32;
                    let src_y = (scy + (py - center_y)).rem_euclid(h) as u32;
                    (
                        layer.base_color_buf.get_pixel(src_x, src_y),
                        layer.roughness_buf.get_pixel(src_x, src_y),
                        layer.metallic_buf.get_pixel(src_x, src_y),
                        layer.height_buf.get_pixel(src_x, src_y),
                        layer.emissive_buf.get_pixel(src_x, src_y),
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
                            let blended = if cur_col.w <= 1e-4 {
                                eff_base_col.truncate()
                            } else {
                                cur_col.truncate().lerp(eff_base_col.truncate(), (weight / alpha).clamp(0.0, 1.0))
                            };
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

        if do_color { layer.dirty_flags.color = true; }
        if do_r { layer.dirty_flags.roughness = true; }
        if do_m { layer.dirty_flags.metallic = true; }
        if do_h || (is_eraser && self.settings.channels.height) || self.settings.channels.normal { layer.dirty_flags.normal = true; }
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

        layer.ensure_allocated_for_paint(self.settings.target, self.settings.channels, is_eraser);
        if is_smudge || is_clone {
            layer.base_color_buf.ensure_allocated();
        }

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
            let cx = ((uv0.x.rem_euclid(1.0) * w_f).round() as i32).clamp(0, w - 1) as u32;
            let cy = ((uv0.y.rem_euclid(1.0) * h_f).round() as i32).clamp(0, h - 1) as u32;
            let cur_col = layer.base_color_buf.get_pixel(cx, cy);
            let cur_r = layer.roughness_buf.get_pixel(cx, cy);
            let cur_m = layer.metallic_buf.get_pixel(cx, cy);
            let cur_h = layer.height_buf.get_pixel(cx, cy);
            let cur_emis = layer.emissive_buf.get_pixel(cx, cy);

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
            if normal_dot < -0.2 {
                continue;
            }
            let t_norm = ((normal_dot - (-0.2)) / (0.25 - (-0.2))).clamp(0.0, 1.0);
            let angle_weight = t_norm * t_norm * (3.0 - 2.0 * t_norm);

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

            let (min_x, max_x, min_y, max_y) = if (tri_max_x - tri_min_x) > 128 || (tri_max_y - tri_min_y) > 128 {
                // For unusually large triangles (e.g. huge ground planes), compute conservative sphere bounds
                let e1_3d = (v1.position - v0.position).length().max(1e-5);
                let e2_3d = (v2.position - v0.position).length().max(1e-5);
                let e3_3d = (v2.position - v1.position).length().max(1e-5);
                let e1_uv = (p1_uv - p0_uv).length();
                let e2_uv = (p2_uv - p0_uv).length();
                let e3_uv = (p2_uv - p1_uv).length();
                let max_uv_rate = (e1_uv / e1_3d).max(e2_uv / e2_3d).max(e3_uv / e3_3d);

                let plane_rad = (world_radius_sq - dist_to_plane * dist_to_plane).max(0.0).sqrt();
                let uv_brush_radius = (plane_rad * max_uv_rate * 2.0).ceil() as i32 + 8;

                let proj_pt = hit_pos - tri_normal * dist_to_plane;
                let (bc_u, bc_v, bc_w) = Self::barycentric_3d(proj_pt, v0.position, v1.position, v2.position);
                let center_uv = p0_uv * bc_u + p1_uv * bc_v + p2_uv * bc_w;

                (
                    tri_min_x.max((center_uv.x - uv_brush_radius as f32).floor() as i32).clamp(0, w - 1),
                    tri_max_x.min((center_uv.x + uv_brush_radius as f32).ceil() as i32).clamp(0, w - 1),
                    tri_min_y.max((center_uv.y - uv_brush_radius as f32).floor() as i32).clamp(0, h - 1),
                    tri_max_y.min((center_uv.y + uv_brush_radius as f32).ceil() as i32).clamp(0, h - 1),
                )
            } else {
                // For typical mesh triangles, use the triangle's UV bounds directly.
                // This guarantees the smooth spherical distance test (p3d - hit_pos).length_squared() <= world_radius_sq
                // defines the brush boundary without ANY artificial axis-aligned staircase/zigzag truncation.
                (
                    tri_min_x.clamp(0, w - 1),
                    tri_max_x.clamp(0, w - 1),
                    tri_min_y.clamp(0, h - 1),
                    tri_max_y.clamp(0, h - 1),
                )
            };

            if min_x > max_x || min_y > max_y {
                continue;
            }

            layer.record_tiles_for_rect(min_x as u32, min_y as u32, max_x as u32, max_y as u32);

            let dw0_dx = (p1_uv.y - p2_uv.y) * inv_det;
            let dw1_dx = (p2_uv.y - p0_uv.y) * inv_det;
            let dw0_dy = (p2_uv.x - p1_uv.x) * inv_det;
            let dw1_dy = (p0_uv.x - p2_uv.x) * inv_det;

            let start_x_f = min_x as f32 + 0.5;
            let start_y_f = min_y as f32 + 0.5;
            let mut row_w0 = ((p1_uv.y - p2_uv.y) * (start_x_f - p2_uv.x) + (p2_uv.x - p1_uv.x) * (start_y_f - p2_uv.y)) * inv_det;
            let mut row_w1 = ((p2_uv.y - p0_uv.y) * (start_x_f - p2_uv.x) + (p0_uv.x - p2_uv.x) * (start_y_f - p2_uv.y)) * inv_det;

            for py in min_y..=max_y {
                let wrap_y = py as usize;
                let buf_y_offset = wrap_y * buf_w;

                let mut w0 = row_w0;
                let mut w1 = row_w1;

                for px in min_x..=max_x {
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
                                        let blended = if cur_col.w <= 1e-4 {
                                            eff_base_col.truncate()
                                        } else {
                                            cur_col.truncate().lerp(eff_base_col.truncate(), (weight / alpha).clamp(0.0, 1.0))
                                        };
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
                    w0 += dw0_dx;
                    w1 += dw1_dx;
                }
                row_w0 += dw0_dy;
                row_w1 += dw1_dy;
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

            if do_color { layer.dirty_flags.color = true; }
            if do_r { layer.dirty_flags.roughness = true; }
            if do_m { layer.dirty_flags.metallic = true; }
            if do_h || (is_eraser && self.settings.channels.height) || self.settings.channels.normal { layer.dirty_flags.normal = true; }

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

    /// Stamps a multi-channel procedural decal, weld seam, rope, stitch, or hardware fastener in 3D world space
    pub fn stamp_3d_decal(
        &mut self,
        layer: &mut Layer,
        mesh: &crate::mesh::Mesh,
        hit_pos: Vec3,
        hit_normal: Vec3,
        tangent: Vec3,
        world_radius: f32,
        style: PathStrokeStyle,
        decal: SmartDecalType,
        heat_tint: bool,
        arc_length: f32,
        height_depth: f32,
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
        let world_radius_sq = world_radius * world_radius;

        layer.ensure_allocated_for_paint(PaintTarget::Material, self.settings.channels, false);
        if self.settings.channels.normal || self.settings.channels.height {
            layer.normal_buf.ensure_allocated();
            layer.height_buf.ensure_allocated();
        }

        // Orthonormal tangent space frame on surface: Normal, Tangent, Bitangent
        let n = hit_normal.normalize_or_zero();
        let tan_proj = tangent - n * n.dot(tangent);
        let t_axis = if tan_proj.length_squared() > 1e-6 {
            tan_proj.normalize()
        } else {
            let up = if n.y.abs() < 0.95 { Vec3::Y } else { Vec3::X };
            n.cross(up).normalize()
        };
        let b_axis = n.cross(t_axis).normalize();

        let base_col_target = self.settings.paint_color;
        let do_color = self.settings.channels.base_color;
        let do_r = self.settings.channels.roughness;
        let do_m = self.settings.channels.metallic;
        let do_h = self.settings.channels.height;
        let do_opac = self.settings.channels.opacity;

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

            let edge1 = v1.position - v0.position;
            let edge2 = v2.position - v0.position;
            let tri_normal = edge1.cross(edge2);
            let normal_len = tri_normal.length();
            if normal_len < 1e-7 {
                continue;
            }
            let tri_normal = tri_normal / normal_len;

            let diff = hit_pos - v0.position;
            let dist_to_plane = diff.dot(tri_normal);
            if dist_to_plane.abs() > world_radius {
                continue;
            }

            let normal_dot = tri_normal.dot(hit_normal);
            if normal_dot < -0.2 {
                continue;
            }
            let t_norm = ((normal_dot - (-0.2)) / (0.25 - (-0.2))).clamp(0.0, 1.0);
            let angle_weight = t_norm * t_norm * (3.0 - 2.0 * t_norm);

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

            let p0_uv = Vec2::new(u0 * w_f, v0_y * h_f);
            let p1_uv = Vec2::new(u1 * w_f, v1_y * h_f);
            let p2_uv = Vec2::new(u2 * w_f, v2_y * h_f);

            let det = (p1_uv.y - p2_uv.y) * (p0_uv.x - p2_uv.x) + (p2_uv.x - p1_uv.x) * (p0_uv.y - p2_uv.y);
            if det.abs() < 1e-6 {
                continue;
            }
            let inv_det = 1.0 / det;

            let tri_min_x = p0_uv.x.min(p1_uv.x).min(p2_uv.x).floor() as i32;
            let tri_max_x = p0_uv.x.max(p1_uv.x).max(p2_uv.x).ceil() as i32;
            let tri_min_y = p0_uv.y.min(p1_uv.y).min(p2_uv.y).floor() as i32;
            let tri_max_y = p0_uv.y.max(p1_uv.y).max(p2_uv.y).ceil() as i32;

            let (min_x, max_x, min_y, max_y) = if (tri_max_x - tri_min_x) > 128 || (tri_max_y - tri_min_y) > 128 {
                let e1_3d = (v1.position - v0.position).length().max(1e-5);
                let e2_3d = (v2.position - v0.position).length().max(1e-5);
                let e3_3d = (v2.position - v1.position).length().max(1e-5);
                let e1_uv = (p1_uv - p0_uv).length();
                let e2_uv = (p2_uv - p0_uv).length();
                let e3_uv = (p2_uv - p1_uv).length();
                let max_uv_rate = (e1_uv / e1_3d).max(e2_uv / e2_3d).max(e3_uv / e3_3d);

                let plane_rad = (world_radius_sq - dist_to_plane * dist_to_plane).max(0.0).sqrt();
                let uv_brush_radius = (plane_rad * max_uv_rate * 2.0).ceil() as i32 + 8;

                let proj_pt = hit_pos - tri_normal * dist_to_plane;
                let (bc_u, bc_v, bc_w) = Self::barycentric_3d(proj_pt, v0.position, v1.position, v2.position);
                let center_uv = p0_uv * bc_u + p1_uv * bc_v + p2_uv * bc_w;

                (
                    tri_min_x.max((center_uv.x - uv_brush_radius as f32).floor() as i32).clamp(0, w - 1),
                    tri_max_x.min((center_uv.x + uv_brush_radius as f32).ceil() as i32).clamp(0, w - 1),
                    tri_min_y.max((center_uv.y - uv_brush_radius as f32).floor() as i32).clamp(0, h - 1),
                    tri_max_y.min((center_uv.y + uv_brush_radius as f32).ceil() as i32).clamp(0, h - 1),
                )
            } else {
                (
                    tri_min_x.clamp(0, w - 1),
                    tri_max_x.clamp(0, w - 1),
                    tri_min_y.clamp(0, h - 1),
                    tri_max_y.clamp(0, h - 1),
                )
            };

            if min_x > max_x || min_y > max_y {
                continue;
            }

            layer.record_tiles_for_rect(min_x as u32, min_y as u32, max_x as u32, max_y as u32);

            let dw0_dx = (p1_uv.y - p2_uv.y) * inv_det;
            let dw1_dx = (p2_uv.y - p0_uv.y) * inv_det;
            let dw0_dy = (p2_uv.x - p1_uv.x) * inv_det;
            let dw1_dy = (p0_uv.x - p2_uv.x) * inv_det;

            let start_x_f = min_x as f32 + 0.5;
            let start_y_f = min_y as f32 + 0.5;
            let mut row_w0 = ((p1_uv.y - p2_uv.y) * (start_x_f - p2_uv.x) + (p2_uv.x - p1_uv.x) * (start_y_f - p2_uv.y)) * inv_det;
            let mut row_w1 = ((p2_uv.y - p0_uv.y) * (start_x_f - p2_uv.x) + (p0_uv.x - p2_uv.x) * (start_y_f - p2_uv.y)) * inv_det;

            for py in min_y..=max_y {
                let wrap_y = py as usize;
                let buf_y_offset = wrap_y * buf_w;

                let mut w0 = row_w0;
                let mut w1 = row_w1;

                for px in min_x..=max_x {
                    let w2 = 1.0 - w0 - w1;

                    if w0 >= -0.01 && w1 >= -0.01 && w2 >= -0.01 {
                        let p3d = v0.position * w0.clamp(0.0, 1.0)
                            + v1.position * w1.clamp(0.0, 1.0)
                            + v2.position * w2.clamp(0.0, 1.0);

                        let offset = p3d - hit_pos;
                        let dist_sq = offset.length_squared();
                        if dist_sq <= world_radius_sq * 1.05 {
                            let u_coord = offset.dot(t_axis) / world_radius;
                            let v_coord = offset.dot(b_axis) / world_radius;

                            let decal_px = evaluate_decal_pixel(
                                style,
                                decal,
                                u_coord,
                                v_coord,
                                base_col_target,
                                heat_tint,
                                arc_length,
                                world_radius,
                                height_depth,
                            );

                            let weight = (decal_px.alpha * angle_weight).clamp(0.0, 1.0);
                            if weight > 1e-4 {
                                let wrap_x = px as usize;
                                overall_min_x = overall_min_x.min(wrap_x as i32);
                                overall_max_x = overall_max_x.max(wrap_x as i32);
                                overall_min_y = overall_min_y.min(wrap_y as i32);
                                overall_max_y = overall_max_y.max(wrap_y as i32);

                                let idx = buf_y_offset + wrap_x;

                                unsafe {
                                    let cur_col = *layer.base_color_buf.data.get_unchecked(idx);
                                    let alpha = (cur_col.w + weight * (1.0 - cur_col.w)).clamp(0.0, 1.0);
                                    if do_color {
                                        let blended = if cur_col.w <= 1e-4 {
                                            decal_px.color.truncate()
                                        } else {
                                            cur_col.truncate().lerp(decal_px.color.truncate(), (weight / alpha).clamp(0.0, 1.0))
                                        };
                                        *layer.base_color_buf.data.get_unchecked_mut(idx) = Vec4::new(blended.x, blended.y, blended.z, alpha);
                                    } else {
                                        layer.base_color_buf.data.get_unchecked_mut(idx).w = alpha;
                                    }

                                    if do_r {
                                        let cur_r = *layer.roughness_buf.data.get_unchecked(idx);
                                        *layer.roughness_buf.data.get_unchecked_mut(idx) = cur_r * (1.0 - weight) + decal_px.roughness * weight;
                                    }
                                    if do_m {
                                        let cur_m = *layer.metallic_buf.data.get_unchecked(idx);
                                        *layer.metallic_buf.data.get_unchecked_mut(idx) = cur_m * (1.0 - weight) + decal_px.metallic * weight;
                                    }
                                    if do_h {
                                        let cur_h = *layer.height_buf.data.get_unchecked(idx);
                                        *layer.height_buf.data.get_unchecked_mut(idx) = cur_h * (1.0 - weight) + decal_px.height * weight;
                                    }
                                    if self.settings.channels.normal || do_h {
                                        let cur_packed = *layer.normal_buf.data.get_unchecked(idx);
                                        let cur_n = Vec3::new(cur_packed.x * 2.0 - 1.0, cur_packed.y * 2.0 - 1.0, cur_packed.z * 2.0 - 1.0);
                                        let blended_n = Vec3::new(
                                            cur_n.x + decal_px.normal.x * weight,
                                            cur_n.y + decal_px.normal.y * weight,
                                            cur_n.z * decal_px.normal.z,
                                        ).normalize_or_zero();
                                        let final_n = if blended_n.length_squared() > 1e-4 { blended_n } else { Vec3::Z };
                                        *layer.normal_buf.data.get_unchecked_mut(idx) = Vec4::new(
                                            final_n.x * 0.5 + 0.5,
                                            final_n.y * 0.5 + 0.5,
                                            final_n.z * 0.5 + 0.5,
                                            *layer.height_buf.data.get_unchecked(idx),
                                        );
                                    }
                                    if do_opac {
                                        let cur_o = *layer.opacity_buf.data.get_unchecked(idx);
                                        *layer.opacity_buf.data.get_unchecked_mut(idx) = cur_o * (1.0 - weight) + decal_px.opacity * weight;
                                    }
                                }
                            }
                        }
                    }
                    w0 += dw0_dx;
                    w1 += dw1_dx;
                }
                row_w0 += dw0_dy;
                row_w1 += dw1_dy;
            }
        }

        if overall_min_x <= overall_max_x && overall_min_y <= overall_max_y {
            let r_min_x = overall_min_x as u32;
            let r_max_x = overall_max_x as u32;
            let r_min_y = overall_min_y as u32;
            let r_max_y = overall_max_y as u32;

            if do_h {
                layer.update_normal_from_height_rect(r_min_x, r_min_y, r_max_x, r_max_y, 4.0);
            }

            if do_color { layer.dirty_flags.color = true; }
            if do_r { layer.dirty_flags.roughness = true; }
            if do_m { layer.dirty_flags.metallic = true; }
            if do_h || self.settings.channels.normal { layer.dirty_flags.normal = true; }

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

    /// Stamps a multi-channel procedural decal or path stroke style on a 2D UV canvas
    #[allow(dead_code)]
    pub fn stamp_uv_decal(
        &mut self,
        layer: &mut Layer,
        center_uv: Vec2,
        tangent_angle: f32,
        uv_radius: f32,
        style: PathStrokeStyle,
        decal: SmartDecalType,
        heat_tint: bool,
        arc_length: f32,
        height_depth: f32,
    ) {
        let w = layer.base_color_buf.width as i32;
        let h = layer.base_color_buf.height as i32;
        let w_f = w as f32;
        let h_f = h as f32;
        let buf_w = w as usize;

        layer.ensure_allocated_for_paint(PaintTarget::Material, self.settings.channels, false);
        if self.settings.channels.normal || self.settings.channels.height {
            layer.normal_buf.ensure_allocated();
            layer.height_buf.ensure_allocated();
        }

        let cx = center_uv.x * w_f;
        let cy = center_uv.y * h_f;
        let r_px = uv_radius * w_f;
        let r_px_sq = r_px * r_px;

        let min_x = ((cx - r_px).floor() as i32).clamp(0, w - 1);
        let max_x = ((cx + r_px).ceil() as i32).clamp(0, w - 1);
        let min_y = ((cy - r_px).floor() as i32).clamp(0, h - 1);
        let max_y = ((cy + r_px).ceil() as i32).clamp(0, h - 1);

        if min_x > max_x || min_y > max_y {
            return;
        }

        layer.record_tiles_for_rect(min_x as u32, min_y as u32, max_x as u32, max_y as u32);

        let cos_a = tangent_angle.cos();
        let sin_a = tangent_angle.sin();

        let base_col_target = self.settings.paint_color;
        let do_color = self.settings.channels.base_color;
        let do_r = self.settings.channels.roughness;
        let do_m = self.settings.channels.metallic;
        let do_h = self.settings.channels.height;
        let do_opac = self.settings.channels.opacity;

        for py in min_y..=max_y {
            let dy = py as f32 + 0.5 - cy;
            let row_offset = py as usize * buf_w;

            for px in min_x..=max_x {
                let dx = px as f32 + 0.5 - cx;
                let d2 = dx * dx + dy * dy;
                if d2 <= r_px_sq {
                    let norm_dx = dx / r_px;
                    let norm_dy = dy / r_px;

                    let u = norm_dx * cos_a + norm_dy * sin_a;
                    let v = -norm_dx * sin_a + norm_dy * cos_a;

                    let decal_px = evaluate_decal_pixel(
                        style,
                        decal,
                        u,
                        v,
                        base_col_target,
                        heat_tint,
                        arc_length,
                        uv_radius,
                        height_depth,
                    );

                    let weight = decal_px.alpha.clamp(0.0, 1.0);
                    if weight > 1e-4 {
                        let idx = row_offset + px as usize;
                        unsafe {
                            let cur_col = *layer.base_color_buf.data.get_unchecked(idx);
                            let alpha = (cur_col.w + weight * (1.0 - cur_col.w)).clamp(0.0, 1.0);
                            if do_color {
                                let blended = if cur_col.w <= 1e-4 {
                                    decal_px.color.truncate()
                                } else {
                                    cur_col.truncate().lerp(decal_px.color.truncate(), (weight / alpha).clamp(0.0, 1.0))
                                };
                                *layer.base_color_buf.data.get_unchecked_mut(idx) = Vec4::new(blended.x, blended.y, blended.z, alpha);
                            } else {
                                layer.base_color_buf.data.get_unchecked_mut(idx).w = alpha;
                            }

                            if do_r {
                                let cur_r = *layer.roughness_buf.data.get_unchecked(idx);
                                *layer.roughness_buf.data.get_unchecked_mut(idx) = cur_r * (1.0 - weight) + decal_px.roughness * weight;
                            }
                            if do_m {
                                let cur_m = *layer.metallic_buf.data.get_unchecked(idx);
                                *layer.metallic_buf.data.get_unchecked_mut(idx) = cur_m * (1.0 - weight) + decal_px.metallic * weight;
                            }
                            if do_h {
                                let cur_h = *layer.height_buf.data.get_unchecked(idx);
                                *layer.height_buf.data.get_unchecked_mut(idx) = cur_h * (1.0 - weight) + decal_px.height * weight;
                            }
                            if self.settings.channels.normal || do_h {
                                let cur_packed = *layer.normal_buf.data.get_unchecked(idx);
                                let cur_n = Vec3::new(cur_packed.x * 2.0 - 1.0, cur_packed.y * 2.0 - 1.0, cur_packed.z * 2.0 - 1.0);
                                let rot_u = decal_px.normal.x * cos_a - decal_px.normal.y * sin_a;
                                let rot_v = decal_px.normal.x * sin_a + decal_px.normal.y * cos_a;
                                let blended_n = Vec3::new(
                                    cur_n.x + rot_u * weight,
                                    cur_n.y + rot_v * weight,
                                    cur_n.z * decal_px.normal.z,
                                ).normalize_or_zero();
                                let final_n = if blended_n.length_squared() > 1e-4 { blended_n } else { Vec3::Z };
                                *layer.normal_buf.data.get_unchecked_mut(idx) = Vec4::new(
                                    final_n.x * 0.5 + 0.5,
                                    final_n.y * 0.5 + 0.5,
                                    final_n.z * 0.5 + 0.5,
                                    *layer.height_buf.data.get_unchecked(idx),
                                );
                            }
                            if do_opac {
                                let cur_o = *layer.opacity_buf.data.get_unchecked(idx);
                                *layer.opacity_buf.data.get_unchecked_mut(idx) = cur_o * (1.0 - weight) + decal_px.opacity * weight;
                            }
                        }
                    }
                }
            }
        }

        let r_min_x = min_x as u32;
        let r_max_x = max_x as u32;
        let r_min_y = min_y as u32;
        let r_max_y = max_y as u32;

        if do_h {
            layer.update_normal_from_height_rect(r_min_x, r_min_y, r_max_x, r_max_y, 4.0);
        }

        if do_color { layer.dirty_flags.color = true; }
        if do_r { layer.dirty_flags.roughness = true; }
        if do_m { layer.dirty_flags.metallic = true; }
        if do_h || self.settings.channels.normal { layer.dirty_flags.normal = true; }

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

    /// Paint continuous stroke in 3D world space across triangles & UV islands
    pub fn paint_stroke_3d(
        &mut self,
        layer: &mut Layer,
        mesh: &crate::mesh::Mesh,
        hit_pos: Vec3,
        hit_normal: Vec3,
        world_radius: f32,
    ) -> Vec<(Vec3, Vec3)> {
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
                let steps = ((dist / step_dist).ceil() as usize).clamp(1, 16);

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

        let stroke_pts: Vec<(Vec3, Vec3)> = points.into_iter().zip(normals.into_iter()).collect();

        for &(pos, norm) in &stroke_pts {
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

        stroke_pts
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

        let mut channel_mask = 0u32;
        if self.settings.channels.base_color { channel_mask |= 1; }
        if self.settings.channels.roughness { channel_mask |= 2; }
        if self.settings.channels.metallic { channel_mask |= 4; }
        if self.settings.channels.height { channel_mask |= 8; }

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
            channel_mask,
            _pad1: 0,
            _pad2: 0,
        };

        Some((uniform, gpu_triangles))
    }

    /// Collect 2D UV stamping uniform parameters for GPU compute shader execution
    pub fn build_gpu_stamp_data_2d(
        &self,
        center_uv: Vec2,
        doc_w: u32,
        doc_h: u32,
    ) -> Option<crate::gpu_brush::GpuBrushUniform> {
        let w_f = doc_w as f32;
        let h_f = doc_h as f32;
        let px = center_uv.x * w_f;
        let py = center_uv.y * h_f;
        let rad = self.settings.radius;

        let clamped_min_x = (px - rad).floor().max(0.0) as i32;
        let clamped_min_y = (py - rad).floor().max(0.0) as i32;
        let clamped_max_x = (px + rad).ceil().min(w_f - 1.0) as i32;
        let clamped_max_y = (py + rad).ceil().min(h_f - 1.0) as i32;

        if clamped_min_x > clamped_max_x || clamped_min_y > clamped_max_y {
            return None;
        }

        let mut channel_mask = 0u32;
        if self.settings.channels.base_color { channel_mask |= 1; }
        if self.settings.channels.roughness { channel_mask |= 2; }
        if self.settings.channels.metallic { channel_mask |= 4; }
        if self.settings.channels.height { channel_mask |= 8; }

        Some(crate::gpu_brush::GpuBrushUniform {
            hit_pos: [px, py, 0.0, rad],
            hit_normal: [0.0, 0.0, 1.0, self.settings.hardness],
            paint_color: [self.settings.paint_color.x, self.settings.paint_color.y, self.settings.paint_color.z, self.settings.paint_color.w],
            params: [self.settings.paint_roughness, self.settings.paint_metallic, self.settings.paint_height, self.settings.flow * self.settings.opacity],
            bounds_min: [clamped_min_x, clamped_min_y],
            bounds_max: [clamped_max_x, clamped_max_y],
            tex_size: [w_f, h_f],
            flags: [if self.settings.mode == ToolMode::Eraser { 1 } else { 0 }, if self.settings.target == PaintTarget::Mask { 1 } else { 0 }],
            tri_count: 0,
            channel_mask,
            _pad1: 0,
            _pad2: 0,
        })
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

        // Stencil rotation (90 degrees)
        engine.settings.stencil.scale = 1.0;
        engine.settings.stencil.angle = std::f32::consts::FRAC_PI_2;
        let sample_rot = engine.sample_stencil(Vec2::new(0.5, 0.5));
        assert!(sample_rot.is_some(), "Center sample under rotation should still hit center");

        // Reset transform verification
        engine.settings.stencil.pos = Vec2::new(0.2, -0.1);
        engine.settings.stencil.angle = 1.2;
        engine.settings.stencil.scale = 2.5;
        engine.settings.stencil.pos = Vec2::ZERO;
        engine.settings.stencil.angle = 0.0;
        engine.settings.stencil.scale = 1.0;
        assert_eq!(engine.settings.stencil.pos, Vec2::ZERO);
        assert_eq!(engine.settings.stencil.angle, 0.0);
        assert_eq!(engine.settings.stencil.scale, 1.0);
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

    #[test]
    fn test_stamp_3d_smooth_circular_coverage() {
        let robot_head = crate::mesh::Mesh::create_robot_head_mesh("RobotHead", 1.8);
        assert!(!robot_head.indices.is_empty(), "Robot head must have geometry");
        
        // BVH sphere query on dense head
        let hit_pos = Vec3::new(0.0, 0.0, 0.88);
        let hit_normal = Vec3::Z;
        let triangles = robot_head.query_sphere(hit_pos, 0.25);
        assert!(!triangles.is_empty(), "BVH query_sphere must return triangles without stack dropping");

        let mut layer = Layer::new_paint_layer(1, "Paint".to_string(), 128, 128);
        let mut engine = BrushEngine::new();
        engine.settings.paint_color = Vec4::new(1.0, 0.0, 0.0, 1.0); // Pure Red
        engine.settings.opacity = 1.0;
        engine.settings.flow = 1.0;
        engine.settings.hardness = 0.5;

        // Perform 3D projection stamping
        engine.stamp_3d(&mut layer, &robot_head, hit_pos, hit_normal, 0.25);

        // Verify painted pixels exist and color is straight (red > 0.9, not premultiplied with black)
        let mut painted_pixel_count = 0;
        let mut max_red = 0.0f32;
        let mut max_alpha = 0.0f32;
        for &pixel in &layer.base_color_buf.data {
            if pixel.w > 0.01 {
                painted_pixel_count += 1;
                max_red = max_red.max(pixel.x);
                max_alpha = max_alpha.max(pixel.w);
                // Straight color test: red channel should NOT be crushed toward zero
                assert!(pixel.x >= 0.8, "Painted pixel should preserve straight red color, got x={}", pixel.x);
            }
        }

        assert!(painted_pixel_count > 10, "Stamp 3D must cover surface, found {} pixels", painted_pixel_count);
        assert!(max_red > 0.95, "Stamp 3D maximum red should be ~1.0, got {}", max_red);
        assert!(max_alpha > 0.95, "Stamp 3D maximum alpha should be ~1.0, got {}", max_alpha);
    }

    #[test]
    fn test_stylus_pressure_curves_and_dynamics() {
        let mut settings = BrushSettings::default();
        settings.radius = 50.0;
        settings.flow = 1.0;
        settings.hardness = 0.8;
        settings.min_pressure_size = 0.2;
        settings.min_pressure_opacity = 0.1;
        settings.min_pressure_hardness = 0.25;

        // 1. Without pressure toggles enabled, settings remain constant
        let (r0, f0, h0) = settings.compute_effective_dynamics(0.5);
        assert_eq!(r0, 50.0);
        assert_eq!(f0, 1.0);
        assert_eq!(h0, 0.8);

        // 2. Enable size, flow, and hardness pressure dynamics
        settings.pressure_size = true;
        settings.pressure_opacity = true;
        settings.pressure_hardness = true;

        // Full pressure (1.0) -> maximum values
        let (r_max, f_max, h_max) = settings.compute_effective_dynamics(1.0);
        assert!((r_max - 50.0).abs() < 1e-4);
        assert!((f_max - 1.0).abs() < 1e-4);
        assert!((h_max - 0.8).abs() < 1e-4);

        // Zero pressure (0.0) -> min threshold values
        let (r_min, f_min, h_min) = settings.compute_effective_dynamics(0.0);
        assert!((r_min - 10.0).abs() < 1e-4, "Min radius should be 50.0 * 0.2 = 10.0");
        assert!((f_min - 0.1).abs() < 1e-4, "Min flow should be 1.0 * 0.1 = 0.1");
        assert!((h_min - 0.2).abs() < 1e-4, "Min hardness should be 0.8 * 0.25 = 0.2");

        // 3. Test Curve characteristics
        // Linear: P = 0.5 -> 0.5
        assert_eq!(PressureCurve::Linear.apply(0.5), 0.5);

        // Soft: P = 0.5 -> 0.5^0.6 ~ 0.65975 (higher output for light touch)
        let soft_p = PressureCurve::Soft.apply(0.5);
        assert!(soft_p > 0.5 && soft_p < 0.7);

        // Firm: P = 0.5 -> 0.5^1.7 ~ 0.30778 (lower output for deliberate firm strokes)
        let firm_p = PressureCurve::Firm.apply(0.5);
        assert!(firm_p < 0.5 && firm_p > 0.2);

        // Compare effective radius across curves at half pressure
        settings.pressure_curve = PressureCurve::Soft;
        let (r_soft, _, _) = settings.compute_effective_dynamics(0.5);
        settings.pressure_curve = PressureCurve::Linear;
        let (r_linear, _, _) = settings.compute_effective_dynamics(0.5);
        settings.pressure_curve = PressureCurve::Firm;
        let (r_firm, _, _) = settings.compute_effective_dynamics(0.5);

        assert!(r_soft > r_linear, "Soft curve must produce larger radius than Linear at 0.5 pressure");
        assert!(r_linear > r_firm, "Linear curve must produce larger radius than Firm at 0.5 pressure");
    }

    #[test]
    fn test_brush_3d_glancing_angle_falloff_and_interpolation() {
        // 1. Verify smoothstep cosine falloff for grazing angles
        let calc_angle_weight = |normal_dot: f32| -> f32 {
            if normal_dot < -0.2 {
                return 0.0;
            }
            let t_norm = ((normal_dot - (-0.2)) / (0.25 - (-0.2))).clamp(0.0, 1.0);
            t_norm * t_norm * (3.0 - 2.0 * t_norm)
        };

        assert_eq!(calc_angle_weight(1.0), 1.0);
        assert_eq!(calc_angle_weight(0.25), 1.0);
        assert_eq!(calc_angle_weight(-0.2), 0.0);
        assert_eq!(calc_angle_weight(-0.5), 0.0);
        let mid = calc_angle_weight(0.025);
        assert!((mid - 0.5).abs() < 1e-4, "Midpoint between -0.2 and 0.25 must evaluate to 0.5");

        // 2. Verify 3D stroke step distance interpolation
        let prev_pos = glam::Vec3::new(0.0, 0.0, 0.0);
        let _prev_norm = glam::Vec3::new(0.0, 1.0, 0.0);
        let hit_pos = glam::Vec3::new(0.1, 0.0, 0.0);
        let _hit_norm = glam::Vec3::new(0.0, 1.0, 0.0);
        let world_radius = 0.05f32;
        let spacing = 0.2f32; // step_dist = 0.01

        let delta = hit_pos - prev_pos;
        let dist = delta.length();
        let step_dist = (world_radius * spacing).max(0.001);
        let steps = ((dist / step_dist).ceil() as usize).clamp(1, 16);
        assert_eq!(steps, 10, "10 steps should be generated for dist=0.1 with step_dist=0.01");

        let mut pts = Vec::with_capacity(steps);
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            pts.push(prev_pos.lerp(hit_pos, t));
        }
        assert_eq!(pts.len(), 10);
        assert!((pts.last().unwrap().x - 0.1).abs() < 1e-5);
    }

    #[test]
    fn test_path_tool_spline_sampling() {
        let mut path = PathToolState::default();
        assert!(path.sample_path(10.0).is_empty());

        // Single point
        path.points.push(PathControlPoint {
            position_3d: Vec3::new(0.0, 0.0, 0.0),
            normal: Vec3::Y,
            uv: Vec2::new(0.1, 0.1),
            radius_scale: 1.0,
        });
        let samples = path.sample_path(10.0);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].uv, Vec2::new(0.1, 0.1));

        // Add more points along a curve
        path.points.push(PathControlPoint {
            position_3d: Vec3::new(1.0, 0.5, 0.0),
            normal: Vec3::Y,
            uv: Vec2::new(0.5, 0.3),
            radius_scale: 1.0,
        });
        path.points.push(PathControlPoint {
            position_3d: Vec3::new(2.0, 0.0, 0.0),
            normal: Vec3::Y,
            uv: Vec2::new(0.9, 0.1),
            radius_scale: 1.0,
        });

        path.spacing_ratio = 0.2;
        path.taper_start = 0.2;
        path.taper_end = 0.4;
        let samples = path.sample_path(0.1);
        assert!(samples.len() > 5, "Should generate multiple equidistant samples along the Catmull-Rom spline");

        // Verify start and end radius tapering
        let first = &samples[0];
        let last = samples.last().unwrap();
        assert!((first.radius_scale - 0.2).abs() < 0.15, "Start radius should be tapered close to taper_start");
        assert!((last.radius_scale - 0.4).abs() < 0.15, "End radius should be tapered close to taper_end");

        // Test closed loop
        path.closed = true;
        let closed_samples = path.sample_path(0.1);
        assert!(closed_samples.len() > samples.len(), "Closed loop path should generate samples for return segment");
    }

    #[test]
    fn test_decal_pixel_evaluation() {
        let base_col = Vec4::new(0.5, 0.5, 0.5, 1.0);

        // Test all PathStrokeStyle variants
        for style in &[
            PathStrokeStyle::Continuous,
            PathStrokeStyle::WeldSeam,
            PathStrokeStyle::RopeBraid,
            PathStrokeStyle::StitchSeam,
            PathStrokeStyle::ChainLinks,
        ] {
            let px = evaluate_decal_pixel(*style, SmartDecalType::PhillipsScrew, 0.0, 0.0, base_col, true, 0.5, 0.1, 1.0);
            assert!(px.alpha > 0.0, "Center alpha should be non-zero for style {:?}", style);
            assert!(px.height >= 0.0 && px.height <= 1.0, "Height should be normalized for style {:?}", style);
            assert!(px.roughness >= 0.0 && px.roughness <= 1.0, "Roughness should be normalized for style {:?}", style);
            assert!(px.metallic >= 0.0 && px.metallic <= 1.0, "Metallic should be normalized for style {:?}", style);
            assert!(px.opacity >= 0.0 && px.opacity <= 1.0, "Opacity should be normalized for style {:?}", style);
            assert!((px.normal.length() - 1.0).abs() < 1e-3, "Normal must be normalized for style {:?}", style);
        }

        // Test all SmartDecalType variants
        for decal in &[
            SmartDecalType::ButtonHole,
            SmartDecalType::PhillipsScrew,
            SmartDecalType::FlatheadScrew,
            SmartDecalType::TorxScrew,
            SmartDecalType::HexBolt,
            SmartDecalType::DomedRivet,
            SmartDecalType::GrommetEyelet,
            SmartDecalType::AirVentLouver,
        ] {
            let px = evaluate_decal_pixel(PathStrokeStyle::SmartDecal, *decal, 0.0, 0.0, base_col, true, 0.0, 0.1, 1.0);
            assert!(px.alpha > 0.0, "Center alpha should be non-zero for decal {:?}", decal);
            assert!(px.height >= 0.0 && px.height <= 1.0, "Height should be normalized for decal {:?}", decal);
            assert!(px.roughness >= 0.0 && px.roughness <= 1.0, "Roughness should be normalized for decal {:?}", decal);
            assert!(px.metallic >= 0.0 && px.metallic <= 1.0, "Metallic should be normalized for decal {:?}", decal);
            assert!(px.opacity >= 0.0 && px.opacity <= 1.0, "Opacity should be normalized for decal {:?}", decal);
            assert!((px.normal.length() - 1.0).abs() < 1e-3, "Normal must be normalized for decal {:?}", decal);
        }

        // Test outside boundary
        let outside_px = evaluate_decal_pixel(PathStrokeStyle::SmartDecal, SmartDecalType::PhillipsScrew, 1.5, 1.5, base_col, true, 0.0, 0.1, 1.0);
        assert_eq!(outside_px.alpha, 0.0, "Outside boundary alpha must be zero");
    }

    #[test]
    fn test_path_tool_styles_and_decals_sampling() {
        let mut path = PathToolState::default();
        path.points.push(PathControlPoint {
            position_3d: Vec3::new(0.0, 0.0, 0.0),
            normal: Vec3::Y,
            uv: Vec2::new(0.0, 0.5),
            radius_scale: 1.0,
        });
        path.points.push(PathControlPoint {
            position_3d: Vec3::new(1.0, 0.0, 0.0),
            normal: Vec3::Y,
            uv: Vec2::new(0.5, 0.5),
            radius_scale: 1.0,
        });
        path.points.push(PathControlPoint {
            position_3d: Vec3::new(2.0, 0.0, 0.0),
            normal: Vec3::Y,
            uv: Vec2::new(1.0, 0.5),
            radius_scale: 1.0,
        });

        // Test WeldSeam style sampling
        path.stroke_style = PathStrokeStyle::WeldSeam;
        let weld_samples = path.sample_path(0.1);
        assert!(!weld_samples.is_empty());
        for i in 1..weld_samples.len() {
            assert!(weld_samples[i].arc_length > weld_samples[i - 1].arc_length, "Arc length must be strictly monotonic");
        }

        // Test SmartDecal style spacing
        path.stroke_style = PathStrokeStyle::SmartDecal;
        path.decal_spacing = 2.0;
        let decal_samples = path.sample_path(0.1);
        assert!(!decal_samples.is_empty());
        assert!(decal_samples.len() < weld_samples.len(), "Decals with 2.0 spacing should have fewer samples than continuous weld ripple beads");
    }

    #[test]
    fn test_stamp_uv_and_3d_decal() {
        let mut engine = BrushEngine::new();
        engine.settings.channels.base_color = true;
        engine.settings.channels.roughness = true;
        engine.settings.channels.metallic = true;
        engine.settings.channels.height = true;

        let mut layer = Layer::new_paint_layer(1, "Test Layer".to_string(), 64, 64);

        // Test 2D UV decal stamping
        engine.stamp_uv_decal(
            &mut layer,
            Vec2::new(0.5, 0.5),
            0.0,
            0.25,
            PathStrokeStyle::SmartDecal,
            SmartDecalType::PhillipsScrew,
            true,
            0.0,
            1.0,
        );

        assert!(layer.dirty_rect.is_some(), "Dirty rect should be populated after stamping");
        let center_idx = 32 * 64 + 32;
        assert!(layer.base_color_buf.data[center_idx].w > 0.0, "Center pixel should have painted alpha");
        assert!(layer.metallic_buf.data[center_idx] > 0.5, "Screw center should have metallic response");

        // Test 3D decal stamping onto a unit cube mesh
        let mesh = crate::mesh::Mesh::create_cube_mesh("Cube", 2.0);
        engine.stamp_3d_decal(
            &mut layer,
            &mesh,
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::Y,
            Vec3::X,
            0.4,
            PathStrokeStyle::WeldSeam,
            SmartDecalType::PhillipsScrew,
            true,
            0.5,
            1.0,
        );
        assert!(layer.dirty_flags.color);
        assert!(layer.dirty_flags.normal);
    }
}