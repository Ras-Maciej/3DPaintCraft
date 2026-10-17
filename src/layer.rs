use rayon::prelude::*;

// 3D Paintcraft - Multi-channel Layer System & Compositor
// Pure safe Rust

use crate::channel::{ChannelFlags, RgbaBuffer, ScalarBuffer};
use crate::utils::NoiseGenerator;
use glam::{Vec3, Vec4};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Add,
    Subtract,
    Darken,
    Lighten,
    Replace,
}

impl BlendMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::Add => "Linear Dodge (Add)",
            Self::Subtract => "Subtract",
            Self::Darken => "Darken",
            Self::Lighten => "Lighten",
            Self::Replace => "Replace",
        }
    }

    #[inline]
    pub fn blend_color(&self, dst: Vec4, src: Vec4, opacity: f32) -> Vec4 {
        let eff_alpha = src.w * opacity;
        if eff_alpha <= 1e-5 {
            return dst;
        }

        let blended_rgb = match self {
            Self::Normal => src.truncate(),
            Self::Replace => src.truncate(),
            Self::Multiply => dst.truncate() * src.truncate(),
            Self::Screen => {
                let d = dst.truncate();
                let s = src.truncate();
                Vec3::ONE - (Vec3::ONE - d) * (Vec3::ONE - s)
            }
            Self::Overlay => {
                let d = dst.truncate();
                let s = src.truncate();
                let mut out = Vec3::ZERO;
                for i in 0..3 {
                    let di = d[i];
                    let si = s[i];
                    out[i] = if di < 0.5 {
                        2.0 * di * si
                    } else {
                        1.0 - 2.0 * (1.0 - di) * (1.0 - si)
                    };
                }
                out
            }
            Self::Add => dst.truncate() + src.truncate(),
            Self::Subtract => (dst.truncate() - src.truncate()).max(Vec3::ZERO),
            Self::Darken => dst.truncate().min(src.truncate()),
            Self::Lighten => dst.truncate().max(src.truncate()),
        };

        let final_rgb = dst.truncate() * (1.0 - eff_alpha) + blended_rgb * eff_alpha;
        let final_a = (dst.w + eff_alpha * (1.0 - dst.w)).clamp(0.0, 1.0);
        Vec4::new(final_rgb.x, final_rgb.y, final_rgb.z, final_a)
    }

    #[inline]
    pub fn blend_scalar(&self, dst: f32, src: f32, opacity: f32) -> f32 {
        let blended = match self {
            Self::Normal | Self::Replace => src,
            Self::Multiply => dst * src,
            Self::Screen => 1.0 - (1.0 - dst) * (1.0 - src),
            Self::Overlay => {
                if dst < 0.5 {
                    2.0 * dst * src
                } else {
                    1.0 - 2.0 * (1.0 - dst) * (1.0 - src)
                }
            }
            Self::Add => (dst + src).min(1.0),
            Self::Subtract => (dst - src).max(0.0),
            Self::Darken => dst.min(src),
            Self::Lighten => dst.max(src),
        };

        dst * (1.0 - opacity) + blended * opacity
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerType {
    Paint,
    Fill,
    Folder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProceduralGenerator {
    None,
    PerlinNoise,
    WorleyGrunge,
    BillowNoise,
    RidgedMultifractal,
    DomainWarpGrunge,
    SplatterDirt,
    WoodGrain,
    FabricWeave,
    Scratches,
    CurvatureWear,
    AmbientOcclusion,
    DirectionalDust,
    PositionGradient,
    DrippingLeaks,
    Checkerboard,
    Bricks,
    PerforatedMesh,
    WireFence,
    TornFabricEdge,
    CircularDots,
    PeelingPaint,
    RustPits,
    MetalEdgeHighlight,
    GroundSplash,
}

impl ProceduralGenerator {
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "Solid Uniform",
            Self::PerlinNoise => "Perlin Noise (FBM)",
            Self::WorleyGrunge => "Worley Grunge (Cellular)",
            Self::BillowNoise => "Billow Noise (Clouds / Puffs)",
            Self::RidgedMultifractal => "Ridged Multifractal (Cracks / Fissures)",
            Self::DomainWarpGrunge => "Domain Warped Grunge (Fluid)",
            Self::SplatterDirt => "Splatter & Rust Spots",
            Self::WoodGrain => "Wood Grain & Fibers",
            Self::FabricWeave => "Fabric & Carbon Weave",
            Self::Scratches => "Micro Scratches",
            Self::CurvatureWear => "Edge Wear (Curvature)",
            Self::AmbientOcclusion => "Cavity Dirt (AO)",
            Self::DirectionalDust => "Directional Dust (Normal & Pos)",
            Self::PositionGradient => "Position Gradient (3D Height)",
            Self::DrippingLeaks => "Dripping Leaks (Vertical Streaks)",
            Self::Checkerboard => "Checkerboard",
            Self::Bricks => "Bricks / Tiles",
            Self::PerforatedMesh => "Perforated Mesh (Hex Holes)",
            Self::WireFence => "Wire Fence (Diamond Mesh)",
            Self::TornFabricEdge => "Torn Fabric (Ragged Cutout)",
            Self::CircularDots => "Circular Dots / Holes",
            Self::PeelingPaint => "Peeling Paint (Chipped Flakes)",
            Self::RustPits => "Rust Pits & Oxidation Craters",
            Self::MetalEdgeHighlight => "Metal Edge Highlight (Polished)",
            Self::GroundSplash => "Ground Dirt & Mud Splash",
        }
    }

    pub fn all() -> &'static [ProceduralGenerator] {
        &[
            Self::PerlinNoise,
            Self::WorleyGrunge,
            Self::BillowNoise,
            Self::RidgedMultifractal,
            Self::DomainWarpGrunge,
            Self::SplatterDirt,
            Self::WoodGrain,
            Self::FabricWeave,
            Self::Scratches,
            Self::CurvatureWear,
            Self::AmbientOcclusion,
            Self::DirectionalDust,
            Self::DrippingLeaks,
            Self::PositionGradient,
            Self::Checkerboard,
            Self::Bricks,
            Self::PerforatedMesh,
            Self::WireFence,
            Self::TornFabricEdge,
            Self::CircularDots,
            Self::PeelingPaint,
            Self::RustPits,
            Self::MetalEdgeHighlight,
            Self::GroundSplash,
        ]
    }
}

/// A non-destructive modifier/effect in a layer mask stack
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MaskEffectType {
    Fill {
        generator: ProceduralGenerator,
        scale: f32,
        contrast: f32,
        invert: bool,
        balance: f32,
    },
    Paint {
        buffer: ScalarBuffer,
        #[serde(default)]
        alpha: ScalarBuffer,
    },
    FilterLevels {
        in_black: f32,
        in_white: f32,
        gamma: f32,
        out_black: f32,
        out_white: f32,
    },
    FilterBlur {
        sigma: f32,
    },
    FilterInvert,
    FilterSharpen {
        strength: f32,
    },
    FilterHighPass {
        radius: f32,
    },
    FilterContrast {
        contrast: f32,
        brightness: f32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaskEffect {
    pub id: u64,
    pub name: String,
    pub effect_type: MaskEffectType,
    pub visible: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
}

/// A multi-channel creative layer (analogous to Substance Painter Layer)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub layer_type: LayerType,
    pub visible: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub channel_flags: ChannelFlags,

    // Fill layer material properties (PBR maps)
    pub fill_color: Vec4,
    pub fill_roughness: f32,
    pub fill_metallic: f32,
    pub fill_height: f32,
    pub fill_normal_strength: f32,
    pub fill_emissive: Vec4,
    pub fill_opacity: f32,
    pub fill_clearcoat: f32,
    pub fill_generator: ProceduralGenerator,
    pub generator_scale: f32,
    pub generator_contrast: f32,
    pub generator_invert: bool,
    pub triplanar: bool,

    // UV Transformation parameters
    pub uv_scale: f32,
    pub uv_offset: glam::Vec2,
    pub uv_rotation: f32,

    // Texture Maps (if loaded) override uniform values
    #[serde(skip)]
    pub fill_color_tex: Option<RgbaBuffer>,
    #[serde(skip)]
    pub fill_roughness_tex: Option<ScalarBuffer>,
    #[serde(skip)]
    pub fill_metallic_tex: Option<ScalarBuffer>,
    #[serde(skip)]
    pub fill_normal_tex: Option<RgbaBuffer>,
    #[serde(skip)]
    pub fill_emissive_tex: Option<RgbaBuffer>,
    #[serde(skip)]
    pub fill_opacity_tex: Option<ScalarBuffer>,

    // Raster buffers for paint layers
    pub base_color_buf: RgbaBuffer,
    pub roughness_buf: ScalarBuffer,
    pub metallic_buf: ScalarBuffer,
    pub height_buf: ScalarBuffer,
    pub normal_buf: RgbaBuffer,
    pub emissive_buf: RgbaBuffer,
    pub opacity_buf: ScalarBuffer,
    pub clearcoat_buf: ScalarBuffer,

    // Optional layer mask & Non-destructive mask effect stack
    pub has_mask: bool,
    pub mask_buf: ScalarBuffer,
    pub mask_inverted: bool,
    #[serde(default = "default_base_mask_val")]
    pub base_mask_value: f32,
    pub mask_effects: Vec<MaskEffect>,
    pub next_mask_effect_id: u64,
    #[serde(default)]
    pub active_mask_effect_id: Option<u64>,
    #[serde(skip)]
    pub dirty_rect: Option<(u32, u32, u32, u32)>,

    // Folder / Group hierarchy
    pub parent_folder_id: Option<u64>,
    pub folder_collapsed: bool,
}

fn default_base_mask_val() -> f32 {
    1.0
}

impl Layer {
    pub fn new_paint_layer(id: u64, name: String, width: u32, height: u32) -> Self {
        Self {
            id,
            name,
            layer_type: LayerType::Paint,
            visible: true,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            channel_flags: ChannelFlags::default(),

            fill_color: Vec4::new(0.8, 0.8, 0.8, 1.0),
            fill_roughness: 0.5,
            fill_metallic: 0.0,
            fill_height: 0.5,
            fill_normal_strength: 1.0,
            fill_emissive: Vec4::new(0.0, 0.0, 0.0, 1.0),
            fill_opacity: 1.0,
            fill_clearcoat: 0.0,
            fill_generator: ProceduralGenerator::None,
            generator_scale: 8.0,
            generator_contrast: 1.0,
            generator_invert: false,
            triplanar: false,

            uv_scale: 1.0,
            uv_offset: glam::Vec2::ZERO,
            uv_rotation: 0.0,
            fill_color_tex: None,
            fill_roughness_tex: None,
            fill_metallic_tex: None,
            fill_normal_tex: None,
            fill_emissive_tex: None,
            fill_opacity_tex: None,

            // Transparent paint buffers
            base_color_buf: RgbaBuffer::new(width, height, Vec4::ZERO),
            roughness_buf: ScalarBuffer::new(width, height, 0.5),
            metallic_buf: ScalarBuffer::new(width, height, 0.0),
            height_buf: ScalarBuffer::new(width, height, 0.5),
            normal_buf: RgbaBuffer::new(width, height, Vec4::new(0.5, 0.5, 1.0, 0.5)),
            emissive_buf: RgbaBuffer::new(width, height, Vec4::ZERO),
            opacity_buf: ScalarBuffer::new(width, height, 1.0),
            clearcoat_buf: ScalarBuffer::new(width, height, 0.0),

            has_mask: false,
            mask_buf: ScalarBuffer::new(width, height, 1.0),
            mask_inverted: false,
            base_mask_value: 1.0,
            mask_effects: Vec::new(),
            next_mask_effect_id: 0,
            active_mask_effect_id: None,
            dirty_rect: None,

            parent_folder_id: None,
            folder_collapsed: false,
        }
    }

    pub fn new_folder_layer(id: u64, name: String, width: u32, height: u32) -> Self {
        let mut layer = Self::new_paint_layer(id, name, width, height);
        layer.layer_type = LayerType::Folder;
        layer
    }

    pub fn new_fill_layer(
        id: u64,
        name: String,
        width: u32,
        height: u32,
        color: Vec4,
        roughness: f32,
        metallic: f32,
    ) -> Self {
        let mut layer = Self::new_paint_layer(id, name, width, height);
        layer.layer_type = LayerType::Fill;
        layer.fill_color = color;
        layer.fill_roughness = roughness;
        layer.fill_metallic = metallic;
        layer.fill_height = 0.5;
        layer.fill_normal_strength = 1.0;
        layer.channel_flags = ChannelFlags {
            base_color: true,
            roughness: true,
            metallic: true,
            normal: true,
            height: true,
            ao: false,
            emission: false,
            opacity: false,
            clearcoat: false,
        };
        layer.evaluate_fill(None);
        layer.mark_dirty_full();
        layer
    }

    pub fn mark_dirty_full(&mut self) {
        let w = self.base_color_buf.width;
        let h = self.base_color_buf.height;
        if w > 0 && h > 0 {
            self.dirty_rect = Some((0, 0, w - 1, h - 1));
        }
    }

    /// Resize all internal channel buffers to a new resolution
    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        self.base_color_buf.resize_bilinear(new_width, new_height);
        self.normal_buf.resize_bilinear(new_width, new_height);
        self.roughness_buf.resize_bilinear(new_width, new_height);
        self.metallic_buf.resize_bilinear(new_width, new_height);
        self.height_buf.resize_bilinear(new_width, new_height);
        self.emissive_buf.resize_bilinear(new_width, new_height);
        self.opacity_buf.resize_bilinear(new_width, new_height);
        self.clearcoat_buf.resize_bilinear(new_width, new_height);
        self.mask_buf.resize_bilinear(new_width, new_height);
        self.mark_dirty_full();
    }

    /// Create a lightweight snapshot of this layer for undo/redo stroke tracking.
    /// Excludes heavy static texture maps (fill_color_tex, fill_normal_tex, etc.)
    /// to avoid copying tens/hundreds of megabytes of read-only textures on every stroke.
    pub fn clone_stroke_snapshot(&self) -> Box<Layer> {
        Box::new(Layer {
            id: self.id,
            name: self.name.clone(),
            layer_type: self.layer_type,
            visible: self.visible,
            opacity: self.opacity,
            blend_mode: self.blend_mode,
            channel_flags: self.channel_flags,

            fill_color: self.fill_color,
            fill_roughness: self.fill_roughness,
            fill_metallic: self.fill_metallic,
            fill_height: self.fill_height,
            fill_normal_strength: self.fill_normal_strength,
            fill_emissive: self.fill_emissive,
            fill_opacity: self.fill_opacity,
            fill_clearcoat: self.fill_clearcoat,
            fill_generator: self.fill_generator,
            generator_scale: self.generator_scale,
            generator_contrast: self.generator_contrast,
            generator_invert: self.generator_invert,
            triplanar: self.triplanar,

            uv_scale: self.uv_scale,
            uv_offset: self.uv_offset,
            uv_rotation: self.uv_rotation,

            fill_color_tex: None,
            fill_roughness_tex: None,
            fill_metallic_tex: None,
            fill_normal_tex: None,
            fill_emissive_tex: None,
            fill_opacity_tex: None,

            base_color_buf: self.base_color_buf.clone(),
            roughness_buf: self.roughness_buf.clone(),
            metallic_buf: self.metallic_buf.clone(),
            height_buf: self.height_buf.clone(),
            normal_buf: self.normal_buf.clone(),
            emissive_buf: self.emissive_buf.clone(),
            opacity_buf: self.opacity_buf.clone(),
            clearcoat_buf: self.clearcoat_buf.clone(),

            has_mask: self.has_mask,
            mask_buf: self.mask_buf.clone(),
            mask_inverted: self.mask_inverted,
            base_mask_value: self.base_mask_value,
            mask_effects: self.mask_effects.clone(),
            next_mask_effect_id: self.next_mask_effect_id,
            active_mask_effect_id: self.active_mask_effect_id,
            dirty_rect: self.dirty_rect,
            parent_folder_id: self.parent_folder_id,
            folder_collapsed: self.folder_collapsed,
        })
    }

    /// Restore raster buffer state from a stroke snapshot, preserving loaded fill textures
    pub fn restore_stroke_snapshot(&mut self, snapshot: &Layer) {
        self.base_color_buf = snapshot.base_color_buf.clone();
        self.roughness_buf = snapshot.roughness_buf.clone();
        self.metallic_buf = snapshot.metallic_buf.clone();
        self.height_buf = snapshot.height_buf.clone();
        self.normal_buf = snapshot.normal_buf.clone();
        self.emissive_buf = snapshot.emissive_buf.clone();
        self.opacity_buf = snapshot.opacity_buf.clone();
        self.clearcoat_buf = snapshot.clearcoat_buf.clone();
        self.mask_buf = snapshot.mask_buf.clone();
        self.has_mask = snapshot.has_mask;
        self.mask_inverted = snapshot.mask_inverted;
        self.base_mask_value = snapshot.base_mask_value;
        self.mask_effects = snapshot.mask_effects.clone();
        self.active_mask_effect_id = snapshot.active_mask_effect_id;
        self.channel_flags = snapshot.channel_flags;
        self.dirty_rect = snapshot.dirty_rect;
    }

    pub fn add_black_mask(&mut self) {
        self.has_mask = true;
        self.mask_inverted = false;
        self.mask_buf.data.fill(0.0);
        self.mark_dirty_full();
    }

    pub fn add_white_mask(&mut self) {
        self.has_mask = true;
        self.mask_inverted = false;
        self.mask_buf.data.fill(1.0);
        self.mark_dirty_full();
    }

    pub fn add_curvature_mask(&mut self, curvature_map: Option<&ScalarBuffer>) {
        self.has_mask = true;
        self.mask_inverted = false;
        let w = self.mask_buf.width;
        let h = self.mask_buf.height;
        
        if let Some(curv) = curvature_map {
            for y in 0..h {
                for x in 0..w {
                    let u = (x as f32 + 0.5) / w as f32;
                    let v = (y as f32 + 0.5) / h as f32;
                    
                    // Sample curvature map using normalized UV coordinates for resolution independence
                    let c = curv.sample_bilinear(glam::Vec2::new(u, v));
                    
                    // High-frequency grunge noise
                    let (n1, n2) = NoiseGenerator::voronoi_2d(u * 24.0, v * 24.0);
                    let fbm = NoiseGenerator::fbm_2d(u * 16.0, v * 16.0, 4, 2.0, 0.5);
                    
                    // Organic wear pattern
                    let noise = ((n2 - n1) * 0.5 + fbm * 0.5).clamp(0.0, 1.0);
                    
                    // Curvature drives the wear, noise breaks it up
                    // c > 0.5 is convex (edges)
                    let edge_mask = if c > 0.51 { ((c - 0.51) * 4.0).clamp(0.0, 1.0) } else { 0.0 };
                    
                    // The wear only appears on edges, modulated by noise
                    let final_wear = (edge_mask * noise * 2.5).clamp(0.0, 1.0);
                    
                    // Smoothstep for harsh chipped paint look
                    let wear = final_wear * final_wear * (3.0 - 2.0 * final_wear); // smoothstep
                    
                    self.mask_buf.set_pixel(x, y, wear);
                }
            }
        }
        self.mark_dirty_full();
    }

    #[allow(dead_code)]
    pub fn add_grunge_mask(&mut self) {
        self.has_mask = true;
        self.mask_inverted = false;
        let w = self.mask_buf.width;
        let h = self.mask_buf.height;
        for y in 0..h {
            for x in 0..w {
                let u = x as f32 / w as f32;
                let v = y as f32 / h as f32;
                
                let (d1, d2) = NoiseGenerator::voronoi_2d(u * 8.0, v * 8.0);
                let scratches = (d2 - d1).clamp(0.0, 1.0);
                let fbm = NoiseGenerator::fbm_2d(u * 12.0, v * 12.0, 5, 2.0, 0.5);
                
                // Mix scratches with dirt patches
                let mask = (scratches * 0.4 + fbm * 0.8).clamp(0.0, 1.0);
                let contrast = mask * mask * (3.0 - 2.0 * mask); // smoothstep
                
                self.mask_buf.set_pixel(x, y, contrast);
            }
        }
        self.mark_dirty_full();
    }
    
    #[allow(dead_code)]
    pub fn add_ao_dirt_mask(&mut self, ao_map: Option<&ScalarBuffer>) {
        self.has_mask = true;
        self.mask_inverted = false;
        let w = self.mask_buf.width;
        let h = self.mask_buf.height;
        
        if let Some(ao) = ao_map {
            for y in 0..h {
                for x in 0..w {
                    let u = (x as f32 + 0.5) / w as f32;
                    let v = (y as f32 + 0.5) / h as f32;
                    
                    // Base AO sampled with normalized UV coordinates
                    let a = ao.sample_bilinear(glam::Vec2::new(u, v));
                    
                    // Dirt goes in crevices (invert AO)
                    let dirt_mask = 1.0 - a;
                    
                    let fbm = NoiseGenerator::fbm_2d(u * 20.0, v * 20.0, 3, 2.0, 0.5);
                    
                    // Dirt builds up strongly in crevices and fades out randomly
                    let final_dirt = (dirt_mask * 1.5 * fbm).clamp(0.0, 1.0);
                    
                    self.mask_buf.set_pixel(x, y, final_dirt);
                }
            }
        }
        self.mark_dirty_full();
    }

    pub fn remove_mask(&mut self) {
        self.has_mask = false;
        self.mask_buf.data.fill(1.0);
        self.mark_dirty_full();
    }

    pub fn invert_mask(&mut self) {
        self.mask_inverted = !self.mask_inverted;
        self.mark_dirty_full();
    }

    pub fn clear_mask(&mut self, value: f32) {
        let v = value.clamp(0.0, 1.0);
        self.has_mask = true;
        self.base_mask_value = v;
        self.mask_buf.data.fill(v);
        self.mask_effects.clear();
        self.mark_dirty_full();
    }

    /// Evaluates non-destructive procedural mask generator
    pub fn generate_mask_procedural(
        generator: ProceduralGenerator,
        scale: f32,
        contrast: f32,
        invert: bool,
        balance: f32,
        width: u32,
        height: u32,
        curvature_map: Option<&ScalarBuffer>,
        ao_map: Option<&ScalarBuffer>,
        position_map: Option<&RgbaBuffer>,
        world_normal_map: Option<&RgbaBuffer>,
    ) -> ScalarBuffer {
        let mut buf = ScalarBuffer::new(width, height, 1.0);
        let sc = scale.max(0.1);
        for y in 0..height {
            for x in 0..width {
                let u = x as f32 / width as f32;
                let v = y as f32 / height as f32;
                let uv_mesh = glam::Vec2::new(u, v);
                let raw_val = match generator {
                    ProceduralGenerator::None => 1.0,
                    ProceduralGenerator::PerlinNoise => {
                        NoiseGenerator::fbm_perlin_2d(u * sc, v * sc, 4, 2.0, 0.5)
                    }
                    ProceduralGenerator::WorleyGrunge => {
                        let (d1, d2) = NoiseGenerator::voronoi_2d(u * sc, v * sc);
                        let noise = NoiseGenerator::fbm_2d(u * sc * 2.0, v * sc * 2.0, 3, 2.0, 0.5);
                        ((d2 - d1) * (0.35 + 0.65 * noise)).clamp(0.0, 1.0)
                    }
                    ProceduralGenerator::BillowNoise => {
                        NoiseGenerator::billow_2d(u * sc, v * sc, 4, 2.0, 0.5)
                    }
                    ProceduralGenerator::RidgedMultifractal => {
                        NoiseGenerator::ridged_fbm_2d(u * sc, v * sc, 4, 2.0, 0.5)
                    }
                    ProceduralGenerator::DomainWarpGrunge => {
                        NoiseGenerator::domain_warp_2d(u * sc * 0.5, v * sc * 0.5)
                    }
                    ProceduralGenerator::SplatterDirt => {
                        NoiseGenerator::splatter_spots_2d(u * sc * 2.0, v * sc * 2.0)
                    }
                    ProceduralGenerator::WoodGrain => {
                        NoiseGenerator::wood_grain_2d(u * sc, v * sc)
                    }
                    ProceduralGenerator::FabricWeave => {
                        NoiseGenerator::fabric_weave_2d(u * sc, v * sc)
                    }
                    ProceduralGenerator::Scratches => {
                        let raw = NoiseGenerator::scratches_2d(u * sc, v * sc);
                        let fine = NoiseGenerator::fbm_2d(u * sc * 8.0, v * sc * 8.0, 2, 2.0, 0.5) * 0.15;
                        (raw * 1.25 + fine * raw).clamp(0.0, 1.0)
                    }
                    ProceduralGenerator::CurvatureWear => {
                        let noise = NoiseGenerator::fbm_perlin_2d(u * sc, v * sc, 4, 2.0, 0.5);
                        let (w1, w2) = NoiseGenerator::voronoi_2d(u * sc * 2.5, v * sc * 2.5);
                        let chip = (w2 - w1).clamp(0.0, 1.0);
                        let ridge = NoiseGenerator::ridged_fbm_2d(u * sc * 3.5, v * sc * 3.5, 3, 2.0, 0.5);

                        let raw_edge = if let Some(cm) = curvature_map {
                            let curv = cm.sample_bilinear(uv_mesh);
                            if curv > 0.505 {
                                let t = (curv - 0.505) * 6.0;
                                t.clamp(0.0, 1.0).powf(0.7)
                            } else {
                                0.0
                            }
                        } else {
                            let edge_synth = (ridge * 0.7 + chip * 0.3).powf(2.0);
                            edge_synth * 0.85
                        };

                        let ao = ao_map.map(|am| am.sample_bilinear(uv_mesh)).unwrap_or(1.0);
                        let ao_gate = (ao * 1.3 - 0.15).clamp(0.1, 1.0);

                        let wear_core = (raw_edge * 1.8).clamp(0.0, 1.0);
                        let wear_fringe = (raw_edge * 2.5 - 0.3).clamp(0.0, 1.0) * (0.35 * noise + 0.45 * chip + 0.20 * ridge);
                        ((wear_core * 0.65 + wear_fringe * 1.3) * ao_gate).clamp(0.0, 1.0)
                    }
                    ProceduralGenerator::AmbientOcclusion => {
                        let ao_val = ao_map.map(|am| am.sample_bilinear(uv_mesh)).unwrap_or(0.85);
                        let curv_val = curvature_map.map(|cm| cm.sample_bilinear(uv_mesh)).unwrap_or(0.5);
                        let grunge = NoiseGenerator::domain_warp_2d(u * sc * 1.2, v * sc * 1.2);
                        let splatter = NoiseGenerator::splatter_spots_2d(u * sc * 2.5, v * sc * 2.5);
                        let fine_fbm = NoiseGenerator::fbm_2d(u * sc * 4.0, v * sc * 4.0, 3, 2.0, 0.5);

                        let crevice_ao = (1.0 - ao_val).clamp(0.0, 1.0).powf(0.8);
                        let crevice_curv = if curv_val < 0.495 { ((0.495 - curv_val) * 4.5).clamp(0.0, 1.0).powf(0.7) } else { 0.0 };
                        let crevice = (crevice_ao * 1.5 + crevice_curv * 1.2).clamp(0.0, 1.0);

                        let sediment = 0.40 + 0.35 * grunge + 0.15 * splatter + 0.10 * fine_fbm;
                        (crevice * sediment * 1.6).clamp(0.0, 1.0)
                    }
                    ProceduralGenerator::DirectionalDust => {
                        let norm_y = world_normal_map
                            .map(|nm| nm.sample_bilinear(uv_mesh).y * 2.0 - 1.0)
                            .unwrap_or(0.85);
                        let ao = ao_map.map(|am| am.sample_bilinear(uv_mesh)).unwrap_or(1.0);
                        let billow = NoiseGenerator::billow_2d(u * sc * 1.5, v * sc * 1.5, 3, 2.0, 0.5);
                        let fine_dust = NoiseGenerator::splatter_spots_2d(u * sc * 4.0, v * sc * 4.0);
                        let perlin = NoiseGenerator::fbm_perlin_2d(u * sc * 2.0, v * sc * 2.0, 3, 2.0, 0.5);

                        let facing_up = (norm_y - 0.1).max(0.0).powf(0.6).clamp(0.0, 1.0);
                        let pocket_boost = 0.7 + (1.0 - ao) * 0.6;
                        (facing_up * (0.35 + 0.40 * billow + 0.15 * fine_dust + 0.10 * perlin) * pocket_boost).clamp(0.0, 1.0)
                    }
                    ProceduralGenerator::PositionGradient => {
                        let pos_y = position_map
                            .map(|pm| pm.sample_bilinear(uv_mesh).y)
                            .unwrap_or(v);
                        pos_y.clamp(0.0, 1.0)
                    }
                    ProceduralGenerator::DrippingLeaks => {
                        let drip = NoiseGenerator::dripping_leaks_2d(u * sc, v * sc);
                        let ao = ao_map.map(|am| am.sample_bilinear(uv_mesh)).unwrap_or(0.8);
                        let source_boost = 1.0 + (1.0 - ao) * 0.8;
                        (drip * source_boost).clamp(0.0, 1.0)
                    }
                    ProceduralGenerator::PeelingPaint => {
                        NoiseGenerator::peeling_paint_2d(u * sc, v * sc)
                    }
                    ProceduralGenerator::RustPits => {
                        let pit = NoiseGenerator::rust_crater_2d(u * sc, v * sc);
                        let ao = ao_map.map(|am| am.sample_bilinear(uv_mesh)).unwrap_or(0.85);
                        let crevice_boost = 0.8 + (1.0 - ao) * 0.8;
                        (pit * crevice_boost).clamp(0.0, 1.0)
                    }
                    ProceduralGenerator::MetalEdgeHighlight => {
                        let curv = curvature_map.map(|cm| cm.sample_bilinear(uv_mesh)).unwrap_or(0.5);
                        let fine_noise = NoiseGenerator::fbm_perlin_2d(u * sc * 3.0, v * sc * 3.0, 3, 2.0, 0.5);
                        let edge = if curv > 0.515 {
                            ((curv - 0.515) * 8.0).clamp(0.0, 1.0).powf(1.4)
                        } else {
                            0.0
                        };
                        (edge * (0.8 + 0.2 * fine_noise)).clamp(0.0, 1.0)
                    }
                    ProceduralGenerator::GroundSplash => {
                        let pos_y = position_map.map(|pm| pm.sample_bilinear(uv_mesh).y).unwrap_or(v);
                        let height_gradient = (1.0 - pos_y).clamp(0.0, 1.0).powf(1.8);
                        let splatter = NoiseGenerator::splatter_spots_2d(u * sc * 2.0, v * sc * 2.0);
                        let warp = NoiseGenerator::domain_warp_2d(u * sc * 0.8, v * sc * 0.8);
                        let splash = height_gradient * (0.5 + 0.35 * splatter + 0.15 * warp);
                        (splash * 1.5).clamp(0.0, 1.0)
                    }
                    ProceduralGenerator::Checkerboard => {
                        let cx = (u * sc).floor() as i32;
                        let cy = (v * sc).floor() as i32;
                        if (cx + cy) % 2 == 0 { 1.0 } else { 0.0 }
                    }
                    ProceduralGenerator::Bricks => {
                        let scaled_y = v * sc;
                        let row = scaled_y.floor() as i32;
                        let offset_x = if row % 2 != 0 { 0.5 } else { 0.0 };
                        let scaled_x = u * sc + offset_x;
                        let fx = (scaled_x - scaled_x.floor()).abs();
                        let fy = (scaled_y - scaled_y.floor()).abs();
                        if fx < 0.07 || fy < 0.07 { 0.0 } else { 1.0 }
                    }
                    ProceduralGenerator::PerforatedMesh => {
                        let scaled_y = v * sc * 1.73205;
                        let row = scaled_y.floor() as i32;
                        let offset_x = if row % 2 != 0 { 0.5 } else { 0.0 };
                        let scaled_x = u * sc + offset_x;
                        let fx = (scaled_x - scaled_x.floor()) - 0.5;
                        let fy = (scaled_y - scaled_y.floor()) - 0.5;
                        let dist = (fx * fx + fy * fy).sqrt();
                        if dist < 0.36 { 0.0 } else { 1.0 }
                    }
                    ProceduralGenerator::WireFence => {
                        let rot_u = (u + v) * 0.7071 * sc;
                        let rot_v = (v - u) * 0.7071 * sc;
                        let fx = (rot_u - rot_u.floor() - 0.5).abs();
                        let fy = (rot_v - rot_v.floor() - 0.5).abs();
                        let wire_thickness = 0.09;
                        let wire = (wire_thickness - fx).max(wire_thickness - fy);
                        if wire > 0.0 { 1.0 } else { 0.0 }
                    }
                    ProceduralGenerator::TornFabricEdge => {
                        let fbm = NoiseGenerator::fbm_2d(u * sc * 3.0, v * sc * 3.0, 4, 2.0, 0.5);
                        let fibers = NoiseGenerator::ridged_fbm_2d(u * sc * 12.0, v * sc * 12.0, 3, 2.0, 0.5);
                        let v_center = (v - 0.5).abs() * 2.0;
                        let tear = v_center + (fbm - 0.5) * 0.7 + (fibers - 0.5) * 0.25;
                        if tear < 0.5 { 1.0 } else { 0.0 }
                    }
                    ProceduralGenerator::CircularDots => {
                        let fx = (u * sc - (u * sc).floor()) - 0.5;
                        let fy = (v * sc - (v * sc).floor()) - 0.5;
                        let dist = (fx * fx + fy * fy).sqrt();
                        if dist < 0.32 { 0.0 } else { 1.0 }
                    }
                };
                let with_balance = (raw_val + (balance - 0.5) * 1.5).clamp(0.0, 1.0);
                let contrasted = ((with_balance - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
                let final_val = if invert { 1.0 - contrasted } else { contrasted };
                buf.set_pixel(x, y, final_val);
            }
        }
        buf
    }

    /// Preserves existing hand-painted mask strokes by moving them into a base Paint effect
    pub fn preserve_base_paint_if_needed(&mut self) {
        if self.mask_effects.is_empty() && self.has_mask {
            let has_paint = self.mask_buf.data.iter().any(|&v| (v - self.base_mask_value).abs() > 0.02);
            if has_paint {
                let w = self.mask_buf.width;
                let h = self.mask_buf.height;
                self.next_mask_effect_id += 1;
                let id = self.next_mask_effect_id;
                self.mask_effects.push(MaskEffect {
                    id,
                    name: "Base Paint".to_string(),
                    effect_type: MaskEffectType::Paint {
                        buffer: self.mask_buf.clone(),
                        alpha: ScalarBuffer::new(w, h, 1.0),
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
            }
        }
    }

    /// Evaluates the non-destructive mask effect stack from bottom to top
    pub fn evaluate_mask(&mut self, curvature_map: Option<&ScalarBuffer>) {
        self.evaluate_mask_advanced(curvature_map, None, None, None);
    }

    /// Full multi-map evaluation of the mask effect stack
    pub fn evaluate_mask_advanced(
        &mut self,
        curvature_map: Option<&ScalarBuffer>,
        ao_map: Option<&ScalarBuffer>,
        position_map: Option<&RgbaBuffer>,
        world_normal_map: Option<&RgbaBuffer>,
    ) {
        if !self.has_mask {
            return;
        }

        let w = self.mask_buf.width;
        let h = self.mask_buf.height;
        if w == 0 || h == 0 {
            return;
        }

        if self.mask_effects.is_empty() {
            self.mark_dirty_full();
            return;
        }

        let mut accum = ScalarBuffer::new(w, h, self.base_mask_value);
        let mut first = true;

        for effect in &mut self.mask_effects {
            if !effect.visible {
                continue;
            }

            match &mut effect.effect_type {
                MaskEffectType::Fill { generator, scale, contrast, invert, balance } => {
                    let fill_buf = Self::generate_mask_procedural(
                        *generator,
                        *scale,
                        *contrast,
                        *invert,
                        *balance,
                        w,
                        h,
                        curvature_map,
                        ao_map,
                        position_map,
                        world_normal_map,
                    );
                    if first && effect.blend_mode == BlendMode::Normal {
                        accum = fill_buf;
                        first = false;
                    } else {
                        first = false;
                        for i in 0..(w * h) as usize {
                            accum.data[i] = effect.blend_mode.blend_scalar(accum.data[i], fill_buf.data[i], effect.opacity);
                        }
                    }
                }
                MaskEffectType::Paint { buffer, alpha } => {
                    first = false;
                    if alpha.data.len() < (w * h) as usize {
                        alpha.data.resize((w * h) as usize, 0.0);
                    }
                    for i in 0..(w * h) as usize {
                        let a = (alpha.data[i] * effect.opacity).clamp(0.0, 1.0);
                        if a > 0.001 {
                            accum.data[i] = effect.blend_mode.blend_scalar(accum.data[i], buffer.data[i], a);
                        }
                    }
                }
                MaskEffectType::FilterLevels { in_black, in_white, gamma, out_black, out_white } => {
                    accum.filter_levels(*in_black, *in_white, *gamma, *out_black, *out_white);
                }
                MaskEffectType::FilterBlur { sigma } => {
                    accum.filter_gaussian_blur(*sigma);
                }
                MaskEffectType::FilterInvert => {
                    accum.filter_invert();
                }
                MaskEffectType::FilterSharpen { strength } => {
                    accum.filter_sharpen(*strength);
                }
                MaskEffectType::FilterHighPass { radius } => {
                    accum.filter_high_pass(*radius);
                }
                MaskEffectType::FilterContrast { contrast, brightness } => {
                    accum.filter_contrast_brightness(*contrast, *brightness);
                }
            }
        }

        self.mask_buf = accum;
        self.mark_dirty_full();
    }

    #[allow(dead_code)]
    pub fn add_mask_fill_effect(&mut self, generator: ProceduralGenerator, curvature_map: Option<&ScalarBuffer>) -> u64 {
        self.add_mask_fill_effect_advanced(generator, curvature_map, None, None, None)
    }

    pub fn add_mask_fill_effect_advanced(
        &mut self,
        generator: ProceduralGenerator,
        curvature_map: Option<&ScalarBuffer>,
        ao_map: Option<&ScalarBuffer>,
        position_map: Option<&RgbaBuffer>,
        world_normal_map: Option<&RgbaBuffer>,
    ) -> u64 {
        self.preserve_base_paint_if_needed();
        self.next_mask_effect_id += 1;
        let id = self.next_mask_effect_id;
        let name = format!("Fill - {}", generator.name());
        let blend_mode = if self.mask_effects.is_empty() {
            BlendMode::Normal
        } else {
            BlendMode::Add
        };
        self.mask_effects.push(MaskEffect {
            id,
            name,
            effect_type: MaskEffectType::Fill {
                generator,
                scale: 8.0,
                contrast: 1.0,
                invert: false,
                balance: 0.5,
            },
            visible: true,
            opacity: 1.0,
            blend_mode,
        });
        self.has_mask = true;
        self.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
        id
    }

    pub fn add_mask_paint_effect(&mut self, curvature_map: Option<&ScalarBuffer>) -> u64 {
        self.add_mask_paint_effect_advanced(curvature_map, None, None, None)
    }

    pub fn add_mask_paint_effect_advanced(
        &mut self,
        curvature_map: Option<&ScalarBuffer>,
        ao_map: Option<&ScalarBuffer>,
        position_map: Option<&RgbaBuffer>,
        world_normal_map: Option<&RgbaBuffer>,
    ) -> u64 {
        self.preserve_base_paint_if_needed();
        self.next_mask_effect_id += 1;
        let id = self.next_mask_effect_id;
        let w = self.mask_buf.width;
        let h = self.mask_buf.height;
        self.mask_effects.push(MaskEffect {
            id,
            name: "Paint Effect".to_string(),
            effect_type: MaskEffectType::Paint {
                buffer: ScalarBuffer::new(w, h, 1.0),
                alpha: ScalarBuffer::new(w, h, 0.0),
            },
            visible: true,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        });
        self.has_mask = true;
        self.active_mask_effect_id = Some(id);
        self.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
        id
    }

    pub fn add_mask_filter_blur(&mut self, sigma: f32, curvature_map: Option<&ScalarBuffer>) -> u64 {
        self.preserve_base_paint_if_needed();
        self.next_mask_effect_id += 1;
        let id = self.next_mask_effect_id;
        self.mask_effects.push(MaskEffect {
            id,
            name: "Blur Filter".to_string(),
            effect_type: MaskEffectType::FilterBlur { sigma },
            visible: true,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        });
        self.has_mask = true;
        self.evaluate_mask(curvature_map);
        id
    }

    pub fn add_mask_filter_levels(&mut self, curvature_map: Option<&ScalarBuffer>) -> u64 {
        self.preserve_base_paint_if_needed();
        self.next_mask_effect_id += 1;
        let id = self.next_mask_effect_id;
        self.mask_effects.push(MaskEffect {
            id,
            name: "Levels Filter".to_string(),
            effect_type: MaskEffectType::FilterLevels {
                in_black: 0.0,
                in_white: 1.0,
                gamma: 1.0,
                out_black: 0.0,
                out_white: 1.0,
            },
            visible: true,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        });
        self.has_mask = true;
        self.evaluate_mask(curvature_map);
        id
    }

    pub fn add_mask_filter_invert(&mut self, curvature_map: Option<&ScalarBuffer>) -> u64 {
        self.preserve_base_paint_if_needed();
        self.next_mask_effect_id += 1;
        let id = self.next_mask_effect_id;
        self.mask_effects.push(MaskEffect {
            id,
            name: "Invert Filter".to_string(),
            effect_type: MaskEffectType::FilterInvert,
            visible: true,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        });
        self.has_mask = true;
        self.evaluate_mask(curvature_map);
        id
    }

    pub fn add_mask_filter_sharpen(&mut self, strength: f32, curvature_map: Option<&ScalarBuffer>) -> u64 {
        self.preserve_base_paint_if_needed();
        self.next_mask_effect_id += 1;
        let id = self.next_mask_effect_id;
        self.mask_effects.push(MaskEffect {
            id,
            name: "Sharpen Filter".to_string(),
            effect_type: MaskEffectType::FilterSharpen { strength },
            visible: true,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        });
        self.has_mask = true;
        self.evaluate_mask(curvature_map);
        id
    }

    pub fn add_mask_filter_high_pass(&mut self, radius: f32, curvature_map: Option<&ScalarBuffer>) -> u64 {
        self.preserve_base_paint_if_needed();
        self.next_mask_effect_id += 1;
        let id = self.next_mask_effect_id;
        self.mask_effects.push(MaskEffect {
            id,
            name: "High Pass Filter".to_string(),
            effect_type: MaskEffectType::FilterHighPass { radius },
            visible: true,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        });
        self.has_mask = true;
        self.evaluate_mask(curvature_map);
        id
    }

    pub fn add_mask_filter_contrast(&mut self, contrast: f32, brightness: f32, curvature_map: Option<&ScalarBuffer>) -> u64 {
        self.preserve_base_paint_if_needed();
        self.next_mask_effect_id += 1;
        let id = self.next_mask_effect_id;
        self.mask_effects.push(MaskEffect {
            id,
            name: "Contrast Filter".to_string(),
            effect_type: MaskEffectType::FilterContrast { contrast, brightness },
            visible: true,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        });
        self.has_mask = true;
        self.evaluate_mask(curvature_map);
        id
    }

    pub fn remove_mask_effect(&mut self, id: u64, curvature_map: Option<&ScalarBuffer>) {
        self.mask_effects.retain(|e| e.id != id);
        if self.active_mask_effect_id == Some(id) {
            self.active_mask_effect_id = None;
        }
        self.evaluate_mask(curvature_map);
    }

    pub fn reorder_mask_effect(&mut self, id: u64, move_up: bool, curvature_map: Option<&ScalarBuffer>) {
        if let Some(pos) = self.mask_effects.iter().position(|e| e.id == id) {
            if move_up && pos + 1 < self.mask_effects.len() {
                self.mask_effects.swap(pos, pos + 1);
            } else if !move_up && pos > 0 {
                self.mask_effects.swap(pos, pos - 1);
            }
            self.evaluate_mask(curvature_map);
        }
    }

    pub fn apply_filter_blur(&mut self, radius: f32, target_mask: bool) {
        if target_mask {
            if self.has_mask {
                self.mask_buf.filter_gaussian_blur(radius);
            }
        } else {
            if self.channel_flags.base_color { self.base_color_buf.filter_gaussian_blur(radius); }
            if self.channel_flags.roughness { self.roughness_buf.filter_gaussian_blur(radius); }
            if self.channel_flags.metallic { self.metallic_buf.filter_gaussian_blur(radius); }
            if self.channel_flags.normal { self.normal_buf.filter_gaussian_blur(radius); }
            if self.channel_flags.height { self.height_buf.filter_gaussian_blur(radius); }
        }
        self.mark_dirty_full();
    }

    pub fn apply_filter_levels(&mut self, in_b: f32, in_w: f32, gamma: f32, out_b: f32, out_w: f32, target_mask: bool) {
        if target_mask {
            if self.has_mask {
                self.mask_buf.filter_levels(in_b, in_w, gamma, out_b, out_w);
            }
        } else {
            if self.channel_flags.base_color { self.base_color_buf.filter_levels(in_b, in_w, gamma, out_b, out_w); }
            if self.channel_flags.roughness { self.roughness_buf.filter_levels(in_b, in_w, gamma, out_b, out_w); }
            if self.channel_flags.metallic { self.metallic_buf.filter_levels(in_b, in_w, gamma, out_b, out_w); }
            if self.channel_flags.height { self.height_buf.filter_levels(in_b, in_w, gamma, out_b, out_w); }
        }
        self.mark_dirty_full();
    }

    pub fn apply_filter_invert(&mut self, target_mask: bool) {
        if target_mask {
            if self.has_mask {
                self.mask_buf.filter_invert();
            }
        } else {
            if self.channel_flags.base_color { self.base_color_buf.filter_invert(); }
            if self.channel_flags.roughness { self.roughness_buf.filter_invert(); }
            if self.channel_flags.metallic { self.metallic_buf.filter_invert(); }
            if self.channel_flags.height { self.height_buf.filter_invert(); }
        }
        self.mark_dirty_full();
    }

    pub fn apply_filter_sharpen(&mut self, strength: f32, target_mask: bool) {
        if target_mask {
            if self.has_mask {
                self.mask_buf.filter_sharpen(strength);
            }
        } else {
            if self.channel_flags.base_color { self.base_color_buf.filter_sharpen(strength); }
            if self.channel_flags.roughness { self.roughness_buf.filter_sharpen(strength); }
            if self.channel_flags.metallic { self.metallic_buf.filter_sharpen(strength); }
            if self.channel_flags.height { self.height_buf.filter_sharpen(strength); }
        }
        self.mark_dirty_full();
    }

    pub fn apply_filter_contrast_brightness(&mut self, contrast: f32, brightness: f32, target_mask: bool) {
        if target_mask {
            if self.has_mask {
                self.mask_buf.filter_contrast_brightness(contrast, brightness);
            }
        } else {
            if self.channel_flags.base_color { self.base_color_buf.filter_contrast_brightness(contrast, brightness); }
            if self.channel_flags.roughness { self.roughness_buf.filter_contrast_brightness(contrast, brightness); }
            if self.channel_flags.metallic { self.metallic_buf.filter_contrast_brightness(contrast, brightness); }
            if self.channel_flags.height { self.height_buf.filter_contrast_brightness(contrast, brightness); }
        }
        self.mark_dirty_full();
    }

    pub fn apply_filter_high_pass(&mut self, radius: f32, target_mask: bool) {
        if target_mask {
            if self.has_mask {
                self.mask_buf.filter_high_pass(radius);
            }
        } else {
            if self.channel_flags.base_color { self.base_color_buf.filter_high_pass(radius); }
            if self.channel_flags.roughness { self.roughness_buf.filter_high_pass(radius); }
            if self.channel_flags.metallic { self.metallic_buf.filter_high_pass(radius); }
            if self.channel_flags.height { self.height_buf.filter_high_pass(radius); }
        }
        self.mark_dirty_full();
    }

    /// Finds or creates an active Paint effect in the mask effect stack
    pub fn get_or_create_active_mask_paint(&mut self) -> &mut MaskEffect {
        if let Some(act_id) = self.active_mask_effect_id {
            if let Some(pos) = self.mask_effects.iter().position(|e| e.id == act_id && matches!(e.effect_type, MaskEffectType::Paint { .. })) {
                return &mut self.mask_effects[pos];
            }
        }
        if let Some(pos) = self.mask_effects.iter().rposition(|e| matches!(e.effect_type, MaskEffectType::Paint { .. })) {
            self.active_mask_effect_id = Some(self.mask_effects[pos].id);
            return &mut self.mask_effects[pos];
        }
        let w = self.mask_buf.width;
        let h = self.mask_buf.height;
        self.next_mask_effect_id += 1;
        let id = self.next_mask_effect_id;
        self.mask_effects.push(MaskEffect {
            id,
            name: "Paint".to_string(),
            effect_type: MaskEffectType::Paint {
                buffer: ScalarBuffer::new(w, h, 1.0),
                alpha: ScalarBuffer::new(w, h, 0.0),
            },
            visible: true,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        });
        self.active_mask_effect_id = Some(id);
        let last_idx = self.mask_effects.len() - 1;
        &mut self.mask_effects[last_idx]
    }

    /// Regenerate procedural fill buffers (all PBR maps) with optional mesh maps
    pub fn evaluate_fill(&mut self, curvature_map: Option<&ScalarBuffer>) {
        self.evaluate_fill_advanced(curvature_map, None, None, None);
    }

    /// Full procedural fill evaluation with curvature, position (WPO), world normal, and AO support
    pub fn evaluate_fill_advanced(
        &mut self,
        curvature_map: Option<&ScalarBuffer>,
        position_map: Option<&RgbaBuffer>,
        normal_map: Option<&RgbaBuffer>,
        ao_map: Option<&ScalarBuffer>,
    ) {
        if self.layer_type != LayerType::Fill {
            return;
        }

        let w = self.base_color_buf.width;
        let h = self.base_color_buf.height;
        let uv_scale = self.uv_scale.max(0.001);

        let rad = self.uv_rotation.to_radians();
        let (sin_r, cos_r) = rad.sin_cos();

        for y in 0..h {
            for x in 0..w {
                // UV coordinates with scale, offset, and centered rotation
                let u_base = (x as f32 / w as f32) + self.uv_offset.x;
                let v_base = (y as f32 / h as f32) + self.uv_offset.y;

                let cu = u_base - 0.5;
                let cv = v_base - 0.5;
                let ru = (cu * cos_r - cv * sin_r + 0.5) * uv_scale;
                let rv = (cu * sin_r + cv * cos_r + 0.5) * uv_scale;
                let uv_mesh = glam::Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);

                let (noise_factor, bump_factor) = if self.triplanar && position_map.is_some() {
                    let pos_sample = position_map.unwrap().sample_bilinear(uv_mesh);
                    let norm_sample = normal_map.map(|nm| nm.sample_bilinear(uv_mesh)).unwrap_or(glam::Vec4::new(0.5, 0.5, 1.0, 1.0));
                    let p3 = glam::Vec3::new(pos_sample.x, pos_sample.y, pos_sample.z) * self.generator_scale * 4.0;
                    let n = (norm_sample.truncate() * 2.0 - glam::Vec3::ONE).normalize_or_zero();
                    let mut weights = glam::Vec3::new(n.x.abs().powi(4), n.y.abs().powi(4), n.z.abs().powi(4));
                    let sum = weights.x + weights.y + weights.z;
                    if sum > 1e-4 { weights /= sum; } else { weights = glam::Vec3::new(0.33, 0.34, 0.33); }

                    let nx = NoiseGenerator::fbm_2d(p3.y, p3.z, 4, 2.0, 0.5);
                    let ny = NoiseGenerator::fbm_2d(p3.x, p3.z, 4, 2.0, 0.5);
                    let nz = NoiseGenerator::fbm_2d(p3.x, p3.y, 4, 2.0, 0.5);
                    let tri = weights.x * nx + weights.y * ny + weights.z * nz;
                    (tri, tri)
                } else {
                    match self.fill_generator {
                        ProceduralGenerator::None => (1.0, 0.5),
                        ProceduralGenerator::PerlinNoise => {
                            let n = NoiseGenerator::fbm_perlin_2d(ru * self.generator_scale, rv * self.generator_scale, 4, 2.0, 0.5);
                            (n, n)
                        }
                        ProceduralGenerator::WorleyGrunge => {
                            let (d1, d2) = NoiseGenerator::voronoi_2d(ru * self.generator_scale, rv * self.generator_scale);
                            let edge = (d2 - d1).clamp(0.0, 1.0);
                            (edge, 1.0 - edge)
                        }
                        ProceduralGenerator::BillowNoise => {
                            let n = NoiseGenerator::billow_2d(ru * self.generator_scale, rv * self.generator_scale, 4, 2.0, 0.5);
                            (n, n)
                        }
                        ProceduralGenerator::RidgedMultifractal => {
                            let n = NoiseGenerator::ridged_fbm_2d(ru * self.generator_scale, rv * self.generator_scale, 4, 2.0, 0.5);
                            (n, n)
                        }
                        ProceduralGenerator::DomainWarpGrunge => {
                            let n = NoiseGenerator::domain_warp_2d(ru * self.generator_scale * 0.5, rv * self.generator_scale * 0.5);
                            (n, n)
                        }
                        ProceduralGenerator::SplatterDirt => {
                            let n = NoiseGenerator::splatter_spots_2d(ru * self.generator_scale * 2.0, rv * self.generator_scale * 2.0);
                            (n, n)
                        }
                        ProceduralGenerator::WoodGrain => {
                            let n = NoiseGenerator::wood_grain_2d(ru * self.generator_scale, rv * self.generator_scale);
                            (n, n)
                        }
                        ProceduralGenerator::FabricWeave => {
                            let n = NoiseGenerator::fabric_weave_2d(ru * self.generator_scale, rv * self.generator_scale);
                            (n, n)
                        }
                        ProceduralGenerator::Scratches => {
                            let s = NoiseGenerator::scratches_2d(ru * self.generator_scale, rv * self.generator_scale);
                            (1.0 - s * 0.75, 0.5 - s * 0.35)
                        }
                        ProceduralGenerator::CurvatureWear => {
                            let noise = NoiseGenerator::fbm_perlin_2d(ru * self.generator_scale, rv * self.generator_scale, 4, 2.0, 0.5);
                            let (w1, w2) = NoiseGenerator::voronoi_2d(ru * self.generator_scale * 2.5, rv * self.generator_scale * 2.5);
                            let chip = (w2 - w1).clamp(0.0, 1.0);
                            let ridge = NoiseGenerator::ridged_fbm_2d(ru * self.generator_scale * 3.5, rv * self.generator_scale * 3.5, 3, 2.0, 0.5);

                            let raw_edge = if let Some(cm) = curvature_map {
                                let curv = cm.sample_bilinear(uv_mesh);
                                if curv > 0.505 { ((curv - 0.505) * 6.0).clamp(0.0, 1.0).powf(0.7) } else { 0.0 }
                            } else {
                                (ridge * 0.7 + chip * 0.3).powf(2.0) * 0.85
                            };

                            let wear_core = (raw_edge * 1.8).clamp(0.0, 1.0);
                            let wear_fringe = (raw_edge * 2.5 - 0.3).clamp(0.0, 1.0) * (0.35 * noise + 0.45 * chip + 0.20 * ridge);
                            let wear = (wear_core * 0.65 + wear_fringe * 1.3).clamp(0.0, 1.0);
                            (1.0 - wear * 0.75, 0.5 + wear * 0.3)
                        }
                        ProceduralGenerator::PositionGradient => {
                            let pos_y = position_map
                                .map(|pm| pm.sample_bilinear(uv_mesh).y)
                                .unwrap_or(rv);
                            (pos_y, pos_y)
                        }
                        ProceduralGenerator::DirectionalDust => {
                            let norm_y = normal_map
                                .map(|nm| nm.sample_bilinear(uv_mesh).y * 2.0 - 1.0)
                                .unwrap_or(0.85);
                            let ao = ao_map.map(|am| am.sample_bilinear(uv_mesh)).unwrap_or(1.0);
                            let billow = NoiseGenerator::billow_2d(ru * self.generator_scale * 1.5, rv * self.generator_scale * 1.5, 3, 2.0, 0.5);
                            let fine_dust = NoiseGenerator::splatter_spots_2d(ru * self.generator_scale * 4.0, rv * self.generator_scale * 4.0);

                            let facing_up = (norm_y - 0.1).max(0.0).powf(0.6).clamp(0.0, 1.0);
                            let pocket_boost = 0.7 + (1.0 - ao) * 0.6;
                            let dust = (facing_up * (0.35 + 0.40 * billow + 0.25 * fine_dust) * pocket_boost).clamp(0.0, 1.0);
                            (1.0 - dust * 0.4, 0.5 + dust * 0.1)
                        }
                        ProceduralGenerator::Checkerboard => {
                            let cx = (ru * self.generator_scale).floor() as i32;
                            let cy = (rv * self.generator_scale).floor() as i32;
                            let val = if (cx + cy) % 2 == 0 { 0.9 } else { 0.2 };
                            (val, val)
                        }
                        ProceduralGenerator::Bricks => {
                            let scaled_y = rv * self.generator_scale;
                            let row = scaled_y.floor() as i32;
                            let offset_x = if row % 2 != 0 { 0.5 } else { 0.0 };
                            let scaled_x = ru * self.generator_scale + offset_x;
                            let fx = (scaled_x - scaled_x.floor()).abs();
                            let fy = (scaled_y - scaled_y.floor()).abs();
                            let is_mortar = fx < 0.07 || fy < 0.07;
                            let val = if is_mortar { 0.2 } else { 0.85 };
                            (val, if is_mortar { 0.1 } else { 0.6 })
                        }
                        ProceduralGenerator::AmbientOcclusion => {
                            let ao_val = ao_map.map(|am| am.sample_bilinear(uv_mesh)).unwrap_or(0.85);
                            let curv_val = curvature_map.map(|cm| cm.sample_bilinear(uv_mesh)).unwrap_or(0.5);
                            let grunge = NoiseGenerator::domain_warp_2d(ru * self.generator_scale * 1.2, rv * self.generator_scale * 1.2);
                            let splatter = NoiseGenerator::splatter_spots_2d(ru * self.generator_scale * 2.5, rv * self.generator_scale * 2.5);

                            let crevice_ao = (1.0 - ao_val).clamp(0.0, 1.0).powf(0.8);
                            let crevice_curv = if curv_val < 0.495 { ((0.495 - curv_val) * 4.5).clamp(0.0, 1.0).powf(0.7) } else { 0.0 };
                            let crevice = (crevice_ao * 1.5 + crevice_curv * 1.2).clamp(0.0, 1.0);

                            let sediment = 0.40 + 0.35 * grunge + 0.25 * splatter;
                            let factor = (crevice * sediment * 1.6).clamp(0.0, 1.0);
                            (1.0 - factor * 0.7, 0.5 + factor * 0.2)
                        }
                        ProceduralGenerator::DrippingLeaks => {
                            let drip = NoiseGenerator::dripping_leaks_2d(ru * self.generator_scale, rv * self.generator_scale);
                            (1.0 - drip * 0.6, 0.5 - drip * 0.1)
                        }
                        ProceduralGenerator::PeelingPaint => {
                            let peel = NoiseGenerator::peeling_paint_2d(ru * self.generator_scale, rv * self.generator_scale);
                            (1.0 - peel * 0.35, 0.5 + peel * 0.25)
                        }
                        ProceduralGenerator::RustPits => {
                            let pit = NoiseGenerator::rust_crater_2d(ru * self.generator_scale, rv * self.generator_scale);
                            (1.0 - pit * 0.5, (0.5 - pit * 0.35).clamp(0.0, 1.0))
                        }
                        ProceduralGenerator::MetalEdgeHighlight => {
                            let curv = curvature_map.map(|cm| cm.sample_bilinear(uv_mesh)).unwrap_or(0.5);
                            let edge = if curv > 0.515 { ((curv - 0.515) * 8.0).clamp(0.0, 1.0).powf(1.4) } else { 0.0 };
                            (1.0 + edge * 0.3, 0.5 + edge * 0.15)
                        }
                        ProceduralGenerator::GroundSplash => {
                            let pos_y = position_map.map(|pm| pm.sample_bilinear(uv_mesh).y).unwrap_or(rv);
                            let splash = (1.0 - pos_y).clamp(0.0, 1.0).powf(1.8) * NoiseGenerator::splatter_spots_2d(ru * self.generator_scale, rv * self.generator_scale);
                            (1.0 - splash * 0.4, 0.5 + splash * 0.2)
                        }
                        ProceduralGenerator::PerforatedMesh => {
                            let scaled_y = rv * self.generator_scale * 1.73205;
                            let row = scaled_y.floor() as i32;
                            let offset_x = if row % 2 != 0 { 0.5 } else { 0.0 };
                            let scaled_x = ru * self.generator_scale + offset_x;
                            let fx = (scaled_x - scaled_x.floor()) - 0.5;
                            let fy = (scaled_y - scaled_y.floor()) - 0.5;
                            let dist = (fx * fx + fy * fy).sqrt();
                            let is_hole = dist < 0.36;
                            let val = if is_hole { 0.1 } else { 0.95 };
                            (val, if is_hole { 0.1 } else { 0.6 })
                        }
                        ProceduralGenerator::WireFence => {
                            let rot_u = (ru + rv) * 0.7071 * self.generator_scale;
                            let rot_v = (rv - ru) * 0.7071 * self.generator_scale;
                            let fx = (rot_u - rot_u.floor() - 0.5).abs();
                            let fy = (rot_v - rot_v.floor() - 0.5).abs();
                            let wire_thickness = 0.09;
                            let wire = (wire_thickness - fx).max(wire_thickness - fy);
                            let is_wire = wire > 0.0;
                            let val = if is_wire { 0.95 } else { 0.1 };
                            (val, if is_wire { 0.7 } else { 0.2 })
                        }
                        ProceduralGenerator::TornFabricEdge => {
                            let fbm = NoiseGenerator::fbm_2d(ru * self.generator_scale * 3.0, rv * self.generator_scale * 3.0, 4, 2.0, 0.5);
                            let fibers = NoiseGenerator::ridged_fbm_2d(ru * self.generator_scale * 12.0, rv * self.generator_scale * 12.0, 3, 2.0, 0.5);
                            let v_center = (rv - 0.5).abs() * 2.0;
                            let tear = v_center + (fbm - 0.5) * 0.7 + (fibers - 0.5) * 0.25;
                            let is_solid = tear >= 0.5;
                            (if is_solid { 0.9 } else { 0.1 }, if is_solid { 0.5 + fibers * 0.2 } else { 0.1 })
                        }
                        ProceduralGenerator::CircularDots => {
                            let fx = (ru * self.generator_scale - (ru * self.generator_scale).floor()) - 0.5;
                            let fy = (rv * self.generator_scale - (rv * self.generator_scale).floor()) - 0.5;
                            let dist = (fx * fx + fy * fy).sqrt();
                            let is_hole = dist < 0.32;
                            (if is_hole { 0.1 } else { 0.9 }, if is_hole { 0.1 } else { 0.55 })
                        }
                    }
                };

                let (eff_noise, eff_bump) = if self.fill_generator != ProceduralGenerator::None {
                    let contrasted = ((noise_factor - 0.5) * self.generator_contrast + 0.5).clamp(0.0, 1.0);
                    let final_noise = if self.generator_invert { 1.0 - contrasted } else { contrasted };
                    let final_bump = if self.generator_invert { 1.0 - bump_factor } else { bump_factor };
                    (final_noise, final_bump)
                } else {
                    (1.0, 0.5)
                };

                let col = if self.fill_generator == ProceduralGenerator::None {
                    self.fill_color
                } else {
                    Vec4::new(
                        (self.fill_color.x * eff_noise).clamp(0.0, 1.0),
                        (self.fill_color.y * eff_noise).clamp(0.0, 1.0),
                        (self.fill_color.z * eff_noise).clamp(0.0, 1.0),
                        self.fill_color.w,
                    )
                };

                let rough = if self.fill_generator == ProceduralGenerator::None {
                    self.fill_roughness
                } else {
                    (self.fill_roughness * (0.65 + 0.7 * eff_noise)).clamp(0.0, 1.0)
                };

                let metal = self.fill_metallic;

                let height = if self.fill_generator == ProceduralGenerator::None {
                    self.fill_height
                } else {
                    (self.fill_height + (eff_bump - 0.5) * 0.4).clamp(0.0, 1.0)
                };

                // UV coords for texture sampling (already includes scale/offset/rotation)
                let tx = (ru.rem_euclid(1.0) * w as f32).clamp(0.0, w as f32 - 1.0) as u32;
                let ty = (rv.rem_euclid(1.0) * h as f32).clamp(0.0, h as f32 - 1.0) as u32;

                let final_col = if let Some(ref tex) = self.fill_color_tex {
                    let t = tex.get_pixel(tx, ty);
                    Vec4::new(
                        (t.x * eff_noise).clamp(0.0, 1.0),
                        (t.y * eff_noise).clamp(0.0, 1.0),
                        (t.z * eff_noise).clamp(0.0, 1.0),
                        t.w,
                    )
                } else {
                    col
                };

                let final_rough = if let Some(ref tex) = self.fill_roughness_tex {
                    tex.get_pixel(tx, ty)
                } else {
                    rough
                };

                let final_metal = if let Some(ref tex) = self.fill_metallic_tex {
                    tex.get_pixel(tx, ty)
                } else {
                    metal
                };

                let final_emissive = if let Some(ref tex) = self.fill_emissive_tex {
                    let t = tex.get_pixel(tx, ty);
                    Vec4::new(
                        t.x * self.fill_emissive.x,
                        t.y * self.fill_emissive.y,
                        t.z * self.fill_emissive.z,
                        t.w * self.fill_emissive.w,
                    )
                } else {
                    self.fill_emissive
                };

                let final_opacity = if let Some(ref tex) = self.fill_opacity_tex {
                    tex.get_pixel(tx, ty) * self.fill_opacity
                } else {
                    self.fill_opacity
                };

                self.base_color_buf.set_pixel(x, y, final_col);
                self.roughness_buf.set_pixel(x, y, final_rough);
                self.metallic_buf.set_pixel(x, y, final_metal);
                self.height_buf.set_pixel(x, y, height);
                self.emissive_buf.set_pixel(x, y, final_emissive);
                self.opacity_buf.set_pixel(x, y, final_opacity);
                self.clearcoat_buf.set_pixel(x, y, self.fill_clearcoat);
            }
        }

        // Generate tangent normal map from height variations and imported normal texture
        self.update_normal_from_height(self.fill_normal_strength);
        self.mark_dirty_full();
    }

    /// Update normal_buf from height_buf across the entire layer
    pub fn update_normal_from_height(&mut self, strength: f32) {
        let w = self.height_buf.width;
        let h = self.height_buf.height;
        if w > 0 && h > 0 {
            self.update_normal_from_height_rect(0, 0, w - 1, h - 1, strength);
        }
    }

    /// Update normal_buf from height_buf within a bounded bounding box with 8-neighbor Sobel filtering and RNM blending
    pub fn update_normal_from_height_rect(&mut self, min_x: u32, min_y: u32, max_x: u32, max_y: u32, strength: f32) {
        let w = self.height_buf.width;
        let h = self.height_buf.height;
        if w < 2 || h < 2 {
            return;
        }

        let x_start = min_x.saturating_sub(1);
        let x_end = (max_x + 1).min(w - 1);
        let y_start = min_y.saturating_sub(1);
        let y_end = (max_y + 1).min(h - 1);

        let rad = self.uv_rotation.to_radians();
        let (sin_r, cos_r) = rad.sin_cos();
        let uv_scale = self.uv_scale.max(0.001);

        for y in y_start..=y_end {
            let ym = if y > 0 { y - 1 } else { h - 1 };
            let yp = if y + 1 < h { y + 1 } else { 0 };

            for x in x_start..=x_end {
                let xm = if x > 0 { x - 1 } else { w - 1 };
                let xp = if x + 1 < w { x + 1 } else { 0 };

                // 8-neighbor Sobel gradient for smooth, noise-free height-to-normal conversion
                let h_tl = self.height_buf.get_pixel(xm, yp);
                let h_tc = self.height_buf.get_pixel(x, yp);
                let h_tr = self.height_buf.get_pixel(xp, yp);
                let h_ml = self.height_buf.get_pixel(xm, y);
                let h_mr = self.height_buf.get_pixel(xp, y);
                let h_bl = self.height_buf.get_pixel(xm, ym);
                let h_bc = self.height_buf.get_pixel(x, ym);
                let h_br = self.height_buf.get_pixel(xp, ym);

                let sobel_x = (h_tr + 2.0 * h_mr + h_br) - (h_tl + 2.0 * h_ml + h_bl);
                let sobel_y = (h_tl + 2.0 * h_tc + h_tr) - (h_bl + 2.0 * h_bc + h_br);

                let dx = (sobel_x / 8.0) * strength * 4.0;
                let dy = (sobel_y / 8.0) * strength * 4.0;
                let bump_n = glam::Vec3::new(-dx, -dy, 1.0).normalize();

                // If imported normal texture is present, blend via Reoriented Normal Mapping (RNM)
                let final_n = if let Some(ref norm_tex) = self.fill_normal_tex {
                    let u_base = (x as f32 / w as f32) + self.uv_offset.x;
                    let v_base = (y as f32 / h as f32) + self.uv_offset.y;
                    let cu = u_base - 0.5;
                    let cv = v_base - 0.5;
                    let ru = (cu * cos_r - cv * sin_r + 0.5) * uv_scale;
                    let rv = (cu * sin_r + cv * cos_r + 0.5) * uv_scale;
                    let tx = (ru.rem_euclid(1.0) * norm_tex.width as f32).clamp(0.0, norm_tex.width as f32 - 1.0) as u32;
                    let ty = (rv.rem_euclid(1.0) * norm_tex.height as f32).clamp(0.0, norm_tex.height as f32 - 1.0) as u32;
                    let base_sample = norm_tex.get_pixel(tx, ty);
                    let base_n = (base_sample.truncate() * 2.0 - glam::Vec3::ONE).normalize_or_zero();

                    if base_n.length_squared() > 1e-4 {
                        let t = base_n + glam::Vec3::Z;
                        let u = glam::Vec3::new(-bump_n.x, -bump_n.y, bump_n.z);
                        let nz = t.z.max(0.001);
                        (t * (t.dot(u) / nz) - u).normalize()
                    } else {
                        bump_n
                    }
                } else {
                    bump_n
                };

                let cur_h = self.height_buf.get_pixel(x, y);
                self.normal_buf.set_pixel(
                    x,
                    y,
                    Vec4::new(final_n.x * 0.5 + 0.5, final_n.y * 0.5 + 0.5, final_n.z * 0.5 + 0.5, cur_h),
                );
            }
        }
    }
}

/// Composite texture set holding final blended PBR maps
#[derive(Clone, Debug)]
pub struct CompositeTextureSet {
    pub width: u32,
    pub height: u32,
    pub base_color: RgbaBuffer,
    pub roughness: ScalarBuffer,
    pub metallic: ScalarBuffer,
    pub normal: RgbaBuffer,
    pub height_map: ScalarBuffer,
    pub ao: ScalarBuffer,
    pub emissive: RgbaBuffer,
    pub opacity: ScalarBuffer,
    pub clearcoat: ScalarBuffer,
}

impl CompositeTextureSet {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            base_color: RgbaBuffer::new(width, height, Vec4::new(0.6, 0.6, 0.6, 1.0)),
            roughness: ScalarBuffer::new(width, height, 0.45),
            metallic: ScalarBuffer::new(width, height, 0.0),
            normal: RgbaBuffer::new(width, height, Vec4::new(0.5, 0.5, 1.0, 0.5)),
            height_map: ScalarBuffer::new(width, height, 0.5),
            ao: ScalarBuffer::new(width, height, 1.0),
            emissive: RgbaBuffer::new(width, height, Vec4::ZERO),
            opacity: ScalarBuffer::new(width, height, 1.0),
            clearcoat: ScalarBuffer::new(width, height, 0.0),
        }
    }
}

impl Default for CompositeTextureSet {
    fn default() -> Self {
        Self::new(512, 512)
    }
}

impl CompositeTextureSet {
    pub fn composite_layers(&mut self, layers: &[Layer]) {
        let w = self.width as usize;
        let _h = self.height as usize;

        let base_col_data = &mut self.base_color.data;
        let rough_data = &mut self.roughness.data;
        let metal_data = &mut self.metallic.data;
        let norm_data = &mut self.normal.data;
        let height_data = &mut self.height_map.data;
        let ao_data = &mut self.ao.data;
        let emissive_data = &mut self.emissive.data;
        let opacity_data = &mut self.opacity.data;
        let clearcoat_data = &mut self.clearcoat.data;

        base_col_data.par_chunks_mut(w)
            .zip(rough_data.par_chunks_mut(w))
            .zip(metal_data.par_chunks_mut(w))
            .zip(norm_data.par_chunks_mut(w))
            .zip(height_data.par_chunks_mut(w))
            .zip(ao_data.par_chunks_mut(w))
            .zip(emissive_data.par_chunks_mut(w))
            .zip(opacity_data.par_chunks_mut(w))
            .zip(clearcoat_data.par_chunks_mut(w))
            .enumerate()
            .for_each(|(y, ((((((((base_row, rough_row), metal_row), norm_row), height_row), ao_row), emissive_row), opacity_row), clearcoat_row))| {
                for x in 0..w {
                    base_row[x] = glam::Vec4::new(0.5, 0.5, 0.55, 1.0);
                    rough_row[x] = 0.5;
                    metal_row[x] = 0.0;
                    norm_row[x] = glam::Vec4::new(0.5, 0.5, 1.0, 0.5);
                    height_row[x] = 0.5;
                    ao_row[x] = 1.0;
                    emissive_row[x] = glam::Vec4::ZERO;
                    opacity_row[x] = 1.0;
                    clearcoat_row[x] = 0.0;
                }

                for layer in layers {
                    if !layer.visible || layer.opacity <= 1e-4 || layer.layer_type == LayerType::Folder {
                        continue;
                    }

                    let parent_folder = layer.parent_folder_id.and_then(|fid| layers.iter().find(|l| l.id == fid));
                    if let Some(folder) = parent_folder {
                        if !folder.visible || folder.opacity <= 1e-4 {
                            continue;
                        }
                    }
                    
                    let mode = layer.blend_mode;
                    let flags = layer.channel_flags;
                    let mut layer_op = layer.opacity;
                    if let Some(folder) = parent_folder {
                        layer_op *= folder.opacity;
                    }
                    let is_paint_layer = layer.layer_type == LayerType::Paint;

                    for x in 0..w {
                        let idx2 = y * w + x;
                        let mut mask_val = if layer.has_mask {
                            let m = layer.mask_buf.data[idx2];
                            if layer.mask_inverted { 1.0 - m } else { m }
                        } else {
                            1.0
                        };

                        if let Some(folder) = parent_folder {
                            if folder.has_mask {
                                let fm = folder.mask_buf.data[idx2];
                                let f_val = if folder.mask_inverted { 1.0 - fm } else { fm };
                                mask_val *= f_val;
                            }
                        }

                        let paint_alpha = if is_paint_layer {
                            layer.base_color_buf.data[idx2].w
                        } else {
                            1.0
                        };

                        let effective_opacity = layer_op * mask_val * paint_alpha;
                        if effective_opacity <= 1e-4 { continue; }

                        if flags.base_color {
                            let src = layer.base_color_buf.data[idx2];
                            base_row[x] = mode.blend_color(base_row[x], src, effective_opacity);
                        }

                        if flags.roughness {
                            let src = layer.roughness_buf.data[idx2];
                            rough_row[x] = mode.blend_scalar(rough_row[x], src, effective_opacity);
                        }

                        if flags.metallic {
                            let src = layer.metallic_buf.data[idx2];
                            metal_row[x] = mode.blend_scalar(metal_row[x], src, effective_opacity);
                        }

                        if flags.normal {
                            let src = layer.normal_buf.data[idx2];
                            norm_row[x] = mode.blend_color(norm_row[x], src, effective_opacity);
                        }

                        if flags.height {
                            let src = layer.height_buf.data[idx2];
                            height_row[x] = mode.blend_scalar(height_row[x], src, effective_opacity);
                        }

                        if flags.emission {
                            let src = layer.emissive_buf.data[idx2];
                            emissive_row[x] = mode.blend_color(emissive_row[x], src, effective_opacity);
                        }

                        if flags.opacity {
                            let src = layer.opacity_buf.data[idx2];
                            opacity_row[x] = mode.blend_scalar(opacity_row[x], src, effective_opacity);
                        }

                        if flags.clearcoat {
                            let src = layer.clearcoat_buf.data[idx2];
                            clearcoat_row[x] = mode.blend_scalar(clearcoat_row[x], src, effective_opacity);
                        }
                    }
                }

                for x in 0..w {
                    norm_row[x].w = height_row[x];
                }
            });

        // Convert composite height map to tangent normal bump and blend with layer normals (Adobe 3D Painter standard)
        self.apply_height_to_normal_bump(4.0);
    }

    /// Converts composite height map into a tangent normal bump map and blends with the layer normal map using RNM
    pub fn apply_height_to_normal_bump(&mut self, bump_strength: f32) {
        let w = self.width;
        let h = self.height;
        if w < 2 || h < 2 {
            return;
        }

        let height_map = &self.height_map;
        let norm_data = &mut self.normal.data;

        norm_data
            .par_chunks_exact_mut(w as usize)
            .enumerate()
            .for_each(|(y, norm_row)| {
                let y = y as u32;
                let ym = if y > 0 { y - 1 } else { h - 1 };
                let yp = if y + 1 < h { y + 1 } else { 0 };

                for x in 0..w {
                    let xm = if x > 0 { x - 1 } else { w - 1 };
                    let xp = if x + 1 < w { x + 1 } else { 0 };

                    let h_tl = height_map.get_pixel(xm, yp);
                    let h_tc = height_map.get_pixel(x, yp);
                    let h_tr = height_map.get_pixel(xp, yp);
                    let h_ml = height_map.get_pixel(xm, y);
                    let h_mr = height_map.get_pixel(xp, y);
                    let h_bl = height_map.get_pixel(xm, ym);
                    let h_bc = height_map.get_pixel(x, ym);
                    let h_br = height_map.get_pixel(xp, ym);

                    let sobel_x = (h_tr + 2.0 * h_mr + h_br) - (h_tl + 2.0 * h_ml + h_bl);
                    let sobel_y = (h_tl + 2.0 * h_tc + h_tr) - (h_bl + 2.0 * h_bc + h_br);

                    let dx = (sobel_x / 8.0) * bump_strength * 4.0;
                    let dy = (sobel_y / 8.0) * bump_strength * 4.0;
                    let bump_n = glam::Vec3::new(-dx, -dy, 1.0).normalize();

                    let cur_norm_raw = norm_row[x as usize];
                    let cur_n = (cur_norm_raw.truncate() * 2.0 - glam::Vec3::ONE).normalize_or_zero();

                    let final_n = if cur_n.length_squared() > 1e-4 {
                        let t = cur_n + glam::Vec3::Z;
                        let u = glam::Vec3::new(-bump_n.x, -bump_n.y, bump_n.z);
                        let nz = t.z.max(0.001);
                        (t * (t.dot(u) / nz) - u).normalize()
                    } else {
                        bump_n
                    };

                    norm_row[x as usize] = glam::Vec4::new(
                        final_n.x * 0.5 + 0.5,
                        final_n.y * 0.5 + 0.5,
                        final_n.z * 0.5 + 0.5,
                        cur_norm_raw.w,
                    );
                }
            });
    }

    /// Produce a standalone combined normal map (RGBA buffer) with height converted to normal
    pub fn generate_combined_normal(&self, bump_strength: f32) -> RgbaBuffer {
        let mut combined = self.normal.clone();
        let w = self.width;
        let h = self.height;
        if w < 2 || h < 2 {
            return combined;
        }

        let height_map = &self.height_map;
        combined
            .data
            .par_chunks_exact_mut(w as usize)
            .enumerate()
            .for_each(|(y, norm_row)| {
                let y = y as u32;
                let ym = if y > 0 { y - 1 } else { h - 1 };
                let yp = if y + 1 < h { y + 1 } else { 0 };

                for x in 0..w {
                    let xm = if x > 0 { x - 1 } else { w - 1 };
                    let xp = if x + 1 < w { x + 1 } else { 0 };

                    let h_tl = height_map.get_pixel(xm, yp);
                    let h_tc = height_map.get_pixel(x, yp);
                    let h_tr = height_map.get_pixel(xp, yp);
                    let h_ml = height_map.get_pixel(xm, y);
                    let h_mr = height_map.get_pixel(xp, y);
                    let h_bl = height_map.get_pixel(xm, ym);
                    let h_bc = height_map.get_pixel(x, ym);
                    let h_br = height_map.get_pixel(xp, ym);

                    let sobel_x = (h_tr + 2.0 * h_mr + h_br) - (h_tl + 2.0 * h_ml + h_bl);
                    let sobel_y = (h_tl + 2.0 * h_tc + h_tr) - (h_bl + 2.0 * h_bc + h_br);

                    let dx = (sobel_x / 8.0) * bump_strength * 4.0;
                    let dy = (sobel_y / 8.0) * bump_strength * 4.0;
                    let bump_n = glam::Vec3::new(-dx, -dy, 1.0).normalize();

                    let cur_norm_raw = norm_row[x as usize];
                    let cur_n = (cur_norm_raw.truncate() * 2.0 - glam::Vec3::ONE).normalize_or_zero();

                    let final_n = if cur_n.length_squared() > 1e-4 {
                        let t = cur_n + glam::Vec3::Z;
                        let u = glam::Vec3::new(-bump_n.x, -bump_n.y, bump_n.z);
                        let nz = t.z.max(0.001);
                        (t * (t.dot(u) / nz) - u).normalize()
                    } else {
                        bump_n
                    };

                    norm_row[x as usize] = glam::Vec4::new(
                        final_n.x * 0.5 + 0.5,
                        final_n.y * 0.5 + 0.5,
                        final_n.z * 0.5 + 0.5,
                        1.0,
                    );
                }
            });
        combined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_layer_initialization_and_evaluation() {
        let mut fill = Layer::new_fill_layer(
            1,
            "Base Fill".to_string(),
            64,
            64,
            Vec4::new(0.9, 0.2, 0.1, 1.0),
            0.35,
            0.85,
        );

        // Verify initial buffer values match fill settings
        let c = fill.base_color_buf.get_pixel(32, 32);
        assert!((c.x - 0.9).abs() < 1e-4);
        assert!((c.y - 0.2).abs() < 1e-4);
        assert!((c.z - 0.1).abs() < 1e-4);

        let r = fill.roughness_buf.get_pixel(32, 32);
        assert!((r - 0.35).abs() < 1e-4);

        let m = fill.metallic_buf.get_pixel(32, 32);
        assert!((m - 0.85).abs() < 1e-4);

        // Manipulate properties like in Substance 3D Painter
        fill.fill_color = Vec4::new(0.1, 0.5, 0.9, 1.0);
        fill.fill_roughness = 0.75;
        fill.fill_metallic = 0.0;
        fill.fill_generator = ProceduralGenerator::Checkerboard;
        fill.generator_scale = 4.0;
        fill.uv_scale = 2.0;
        fill.evaluate_fill(None);

        // Buffers should reflect the new procedural evaluation
        let new_c = fill.base_color_buf.get_pixel(10, 10);
        assert!(new_c.x > 0.0);
        assert!(fill.roughness_buf.get_pixel(10, 10) > 0.0);
    }

    #[test]
    fn test_channel_flags_compositing_isolation() {
        let mut comp = CompositeTextureSet::new(32, 32);

        // Layer 1: Red plastic
        let mut l1 = Layer::new_fill_layer(
            1,
            "Red Plastic".to_string(),
            32,
            32,
            Vec4::new(1.0, 0.0, 0.0, 1.0),
            0.8,
            0.0,
        );
        l1.evaluate_fill(None);

        // Layer 2: Gold metallic, but BaseColor channel DISABLED
        let mut l2 = Layer::new_fill_layer(
            2,
            "Gold Metal Specular Only".to_string(),
            32,
            32,
            Vec4::new(1.0, 0.84, 0.0, 1.0),
            0.2,
            1.0,
        );
        l2.channel_flags.base_color = false; // Disabled! Leaves lower color intact
        l2.channel_flags.metallic = true;
        l2.channel_flags.roughness = true;
        l2.evaluate_fill(None);

        comp.composite_layers(&[l1, l2]);

        // Base color should still be red from layer 1!
        let c = comp.base_color.get_pixel(16, 16);
        assert!((c.x - 1.0).abs() < 1e-4, "Base color was not preserved from layer 1");
        assert!(c.y < 0.1);

        // But metallic should be 1.0 and roughness 0.2 from layer 2!
        let m = comp.metallic.get_pixel(16, 16);
        assert!((m - 1.0).abs() < 1e-4);
        let r = comp.roughness.get_pixel(16, 16);
        assert!((r - 0.2).abs() < 1e-4);
    }

    #[test]
    fn test_mask_gating_behavior() {
        let mut comp = CompositeTextureSet::new(32, 32);

        // Base layer: Blue
        let mut l1 = Layer::new_fill_layer(
            1,
            "Blue Base".to_string(),
            32,
            32,
            Vec4::new(0.0, 0.0, 1.0, 1.0),
            0.5,
            0.0,
        );
        l1.evaluate_fill(None);

        // Top layer: Green with black mask (completely hidden)
        let mut l2 = Layer::new_fill_layer(
            2,
            "Hidden Green".to_string(),
            32,
            32,
            Vec4::new(0.0, 1.0, 0.0, 1.0),
            0.5,
            0.0,
        );
        l2.add_black_mask();
        l2.evaluate_fill(None);

        comp.composite_layers(&[l1.clone(), l2.clone()]);
        let c_black = comp.base_color.get_pixel(16, 16);
        assert!((c_black.z - 1.0).abs() < 1e-4, "Black mask should hide top layer");

        // Invert mask -> should reveal green!
        l2.invert_mask();
        comp.composite_layers(&[l1, l2]);
        let c_inv = comp.base_color.get_pixel(16, 16);
        assert!((c_inv.y - 1.0).abs() < 1e-4, "Inverted mask should reveal top layer");
    }

    #[test]
    fn test_curvature_resolution_independence() {
        // High resolution 128x128 curvature map with an edge band across the center
        let mut curv = ScalarBuffer::new(128, 128, 0.5);
        for y in 60..=68 {
            for x in 0..128 {
                curv.set_pixel(x, y, 1.0); // Ridge edge across horizontal center
            }
        }

        // Lower resolution 32x32 layer
        let mut layer = Layer::new_fill_layer(
            1,
            "Worn Layer".to_string(),
            32,
            32,
            Vec4::ONE,
            0.5,
            0.0,
        );
        layer.add_curvature_mask(Some(&curv));

        // The edge at y = 16 in the 32x32 buffer should detect wear from the 128x128 map
        let mask_at_edge = layer.mask_buf.get_pixel(16, 16);
        let mask_at_flat = layer.mask_buf.get_pixel(16, 4);

        assert!(mask_at_edge > 0.1, "Edge wear should be detected across differing resolutions");
        assert_eq!(mask_at_flat, 0.0, "Flat areas should have zero edge wear");
    }

    #[test]
    fn test_layer_folder_hierarchy_and_compositing() {
        let mut comp = CompositeTextureSet::new(16, 16);
        let mut l_bg = Layer::new_fill_layer(1, "Background".to_string(), 16, 16, Vec4::new(0.0, 0.0, 1.0, 1.0), 0.5, 0.0);
        l_bg.evaluate_fill(None);

        let mut folder = Layer::new_folder_layer(2, "Folder".to_string(), 16, 16);
        folder.opacity = 0.5;

        let mut l_child = Layer::new_fill_layer(3, "Child".to_string(), 16, 16, Vec4::new(1.0, 0.0, 0.0, 1.0), 0.5, 0.0);
        l_child.parent_folder_id = Some(2);
        l_child.evaluate_fill(None);

        comp.composite_layers(&[l_bg, folder, l_child]);
        let pixel = comp.base_color.get_pixel(8, 8);
        // With 50% opacity folder, red is blended over blue
        assert!((pixel.x - 0.5).abs() < 1e-3, "Red channel should be scaled by folder opacity");
        assert!((pixel.z - 0.5).abs() < 1e-3, "Blue channel should remain blended");
    }

    #[test]
    fn test_procedural_filters_invert_and_levels() {
        let mut layer = Layer::new_fill_layer(1, "Test".to_string(), 16, 16, Vec4::new(0.2, 0.4, 0.8, 1.0), 0.5, 0.0);
        layer.evaluate_fill(None);

        // Invert channels (target_mask: false)
        layer.apply_filter_invert(false);
        let pix = layer.base_color_buf.get_pixel(4, 4);
        assert!((pix.x - 0.8).abs() < 1e-3, "Inverted 0.2 should be 0.8");
        assert!((pix.y - 0.6).abs() < 1e-3, "Inverted 0.4 should be 0.6");

        // Levels (target_mask: false)
        layer.apply_filter_levels(0.0, 1.0, 2.0, 0.0, 1.0, false);
        let pix_lev = layer.base_color_buf.get_pixel(4, 4);
        assert!((pix_lev.x - 0.8_f32.powf(0.5)).abs() < 1e-3, "Gamma adjustment of 2.0 (inv_gamma 0.5) should be applied");
    }

    #[test]
    fn test_mask_stack_additive_modifiers() {
        let mut layer = Layer::new_fill_layer(1, "Masked".to_string(), 16, 16, Vec4::ONE, 0.5, 0.0);
        layer.clear_mask(0.0);

        // Add first fill effect
        layer.add_mask_fill_effect_advanced(ProceduralGenerator::PerlinNoise, None, None, None, None);
        let val_1 = layer.mask_buf.get_pixel(8, 8);
        assert!(val_1 > 0.0, "First mask effect should generate non-zero values");

        // Add second fill effect: should default to Additive blend mode
        layer.add_mask_fill_effect_advanced(ProceduralGenerator::WorleyGrunge, None, None, None, None);
        assert_eq!(layer.mask_effects.len(), 2);
        assert_eq!(layer.mask_effects[1].blend_mode, BlendMode::Add, "Subsequent mask effect should default to Additive");

        let val_2 = layer.mask_buf.get_pixel(8, 8);
        assert!(val_2 >= val_1, "Additive second effect must add onto the mask without replacing or zeroing the old one");
    }

    #[test]
    fn test_mask_paint_preservation() {
        let mut layer = Layer::new_fill_layer(1, "HandPaintedMask".to_string(), 16, 16, Vec4::ONE, 0.5, 0.0);
        layer.clear_mask(0.0);
        // Paint a stroke directly on the mask
        layer.mask_buf.set_pixel(4, 4, 0.95);

        // Now add a procedural effect or smart mask: existing paint must be preserved
        layer.preserve_base_paint_if_needed();
        assert_eq!(layer.mask_effects.len(), 1, "Hand painted mask should be preserved into a Base Paint effect");
        assert_eq!(layer.mask_effects[0].name, "Base Paint");

        // Add a blur filter onto the mask stack
        layer.add_mask_filter_blur(1.5, None);
        assert_eq!(layer.mask_effects.len(), 2);
        let pix = layer.mask_buf.get_pixel(4, 4);
        assert!(pix > 0.1, "Preserved painted stroke must still be present after evaluating stack");
    }

    #[test]
    fn test_mask_filters_and_generators() {
        let mut layer = Layer::new_fill_layer(1, "FilterTest".to_string(), 16, 16, Vec4::new(0.5, 0.5, 0.5, 1.0), 0.5, 0.0);
        layer.clear_mask(0.5);

        // Test contrast & brightness
        layer.apply_filter_contrast_brightness(2.0, 0.1, true);
        let val = layer.mask_buf.get_pixel(0, 0);
        assert!((val - 0.7).abs() < 1e-2, "0.5 + 0.1 brightness, then (0.6 - 0.5)*2 + 0.5 = 0.7");

        // Test high-pass
        layer.apply_filter_high_pass(1.0, true);
        let val_hp = layer.mask_buf.get_pixel(0, 0);
        assert!((val_hp - 0.5).abs() < 1e-2, "Flat buffer high-pass should isolate neutral 0.5");

        // Test AO and Leaks generators in procedural mask evaluation
        let ao_map = ScalarBuffer::new(16, 16, 0.2); // deep crevice
        let gen_ao = Layer::generate_mask_procedural(
            ProceduralGenerator::AmbientOcclusion,
            4.0,
            1.0,
            false,
            0.5,
            16,
            16,
            None,
            Some(&ao_map),
            None,
            None,
        );
        let ao_crevice = gen_ao.get_pixel(8, 8);
        assert!(ao_crevice > 0.3, "Ambient occlusion generator should detect crevice from ao_map");

        // Verify all generators in ProceduralGenerator::all() evaluate without panics
        for gen in ProceduralGenerator::all() {
            let buf = Layer::generate_mask_procedural(*gen, 8.0, 1.0, false, 0.5, 16, 16, None, None, None, None);
            assert_eq!(buf.width, 16);
            assert_eq!(buf.height, 16);
            assert!(buf.data.iter().all(|&v| v >= 0.0 && v <= 1.0));
        }
    }

    #[test]
    fn test_opacity_channel_procedurals_and_fill() {
        let opacity_generators = [
            ProceduralGenerator::PerforatedMesh,
            ProceduralGenerator::WireFence,
            ProceduralGenerator::TornFabricEdge,
            ProceduralGenerator::CircularDots,
        ];

        for gen in opacity_generators {
            let buf = Layer::generate_mask_procedural(gen, 10.0, 1.5, false, 0.5, 32, 32, None, None, None, None);
            assert_eq!(buf.width, 32);
            assert_eq!(buf.height, 32);
            assert!(buf.data.iter().all(|&v| (0.0..=1.0).contains(&v)), "Generator {:?} produced out-of-range values", gen);
        }

        // Test fill layer with opacity channel
        let mut layer = Layer::new_fill_layer(
            1,
            "Opacity Fill".to_string(),
            32,
            32,
            Vec4::ONE,
            0.2,
            0.0,
        );
        layer.fill_opacity = 0.42;
        layer.channel_flags.opacity = true;
        layer.evaluate_fill(None);

        assert!((layer.opacity_buf.get_pixel(16, 16) - 0.42).abs() < 1e-4, "Fill opacity buffer did not reflect fill_opacity");

        let mut comp = CompositeTextureSet::new(32, 32);
        comp.composite_layers(&[layer]);
        assert!((comp.opacity.get_pixel(16, 16) - 0.42).abs() < 1e-4, "Composite opacity did not blend layer opacity");
    }

    #[test]
    fn test_stroke_snapshot_optimization() {
        let mut layer = Layer::new_fill_layer(
            1,
            "Optimized Layer".to_string(),
            32,
            32,
            Vec4::ONE,
            0.5,
            0.0,
        );
        // Simulate heavy static texture map
        let heavy_tex = RgbaBuffer::new(32, 32, Vec4::new(0.8, 0.2, 0.1, 1.0));
        layer.fill_color_tex = Some(heavy_tex);

        // Paint a pixel
        layer.base_color_buf.set_pixel(10, 10, Vec4::new(1.0, 0.0, 0.0, 1.0));

        // Create stroke snapshot
        let snapshot = layer.clone_stroke_snapshot();

        // Verify heavy static texture was skipped in snapshot (saving memory and latency)
        assert!(snapshot.fill_color_tex.is_none(), "Stroke snapshot should not duplicate heavy static fill textures");
        assert_eq!(snapshot.base_color_buf.get_pixel(10, 10), Vec4::new(1.0, 0.0, 0.0, 1.0));

        // Now mutate original layer (simulate active painting stroke)
        layer.base_color_buf.set_pixel(10, 10, Vec4::new(0.0, 1.0, 0.0, 1.0));
        assert_eq!(layer.base_color_buf.get_pixel(10, 10), Vec4::new(0.0, 1.0, 0.0, 1.0));

        // Restore stroke snapshot (simulate undo)
        layer.restore_stroke_snapshot(&snapshot);

        // Verify painted buffer was restored while target's static texture map remained intact
        assert_eq!(layer.base_color_buf.get_pixel(10, 10), Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert!(layer.fill_color_tex.is_some(), "Restoring stroke snapshot must preserve target's fill textures");
    }
}

