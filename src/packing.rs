// 3D Paintcraft - Custom Texture Packing System
// High-performance user-configurable channel packing and export matrix
// Pure safe Rust

use crate::channel::{RgbaBuffer, ScalarBuffer};
use crate::export::TextureExporter;
use crate::layer::CompositeTextureSet;
use crate::mesh::Mesh;
use image::{ImageBuffer, Rgba};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Available source channels for routing into packed texture channels
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelSource {
    // Base Color Channels
    BaseColorR,
    BaseColorG,
    BaseColorB,
    BaseColorA,
    BaseColorLuma,

    // Surface & PBR Properties
    Roughness,
    Smoothness,
    Metallic,
    AmbientOcclusion,
    Height,
    Opacity,
    Clearcoat,
    Anisotropy,
    AnisotropyAngle,

    // Tangent Space Normals (Blended with procedural height bump)
    NormalX,
    NormalY,
    NormalYInverted,
    NormalZ,

    // Baked Mesh Maps (from project mesh baking pipeline)
    BakedCurvature,
    BakedThickness,
    BakedWorldNormalX,
    BakedWorldNormalY,
    BakedWorldNormalZ,
    BakedPositionX,
    BakedPositionY,
    BakedPositionZ,
    BakedId,

    // Emissive Channels
    EmissiveR,
    EmissiveG,
    EmissiveB,
    EmissiveLuma,

    // Constant Numeric Offsets
    ConstantBlack,
    ConstantWhite,
    ConstantGray,
}

impl ChannelSource {
    pub fn name(&self) -> &'static str {
        match self {
            Self::BaseColorR => "Base Color: Red (R)",
            Self::BaseColorG => "Base Color: Green (G)",
            Self::BaseColorB => "Base Color: Blue (B)",
            Self::BaseColorA => "Base Color: Alpha (A)",
            Self::BaseColorLuma => "Base Color: Luminance (Gray)",

            Self::Roughness => "Roughness",
            Self::Smoothness => "Smoothness (1 - Roughness)",
            Self::Metallic => "Metallic",
            Self::AmbientOcclusion => "Ambient Occlusion (AO)",
            Self::Height => "Height / Displacement",
            Self::Opacity => "Opacity / Cutout",
            Self::Clearcoat => "Clearcoat Intensity",
            Self::Anisotropy => "Anisotropy Intensity",
            Self::AnisotropyAngle => "Anisotropy Angle",

            Self::NormalX => "Normal: Tangent X (R)",
            Self::NormalY => "Normal: Tangent Y (+Y OpenGL)",
            Self::NormalYInverted => "Normal: Tangent Y (-Y DirectX)",
            Self::NormalZ => "Normal: Tangent Z (B)",

            Self::BakedCurvature => "Baked: Curvature",
            Self::BakedThickness => "Baked: Thickness / SSS",
            Self::BakedWorldNormalX => "Baked: World Normal X",
            Self::BakedWorldNormalY => "Baked: World Normal Y",
            Self::BakedWorldNormalZ => "Baked: World Normal Z",
            Self::BakedPositionX => "Baked: Position X",
            Self::BakedPositionY => "Baked: Position Y",
            Self::BakedPositionZ => "Baked: Position Z",
            Self::BakedId => "Baked: Mesh / Material ID",

            Self::EmissiveR => "Emissive: Red (R)",
            Self::EmissiveG => "Emissive: Green (G)",
            Self::EmissiveB => "Emissive: Blue (B)",
            Self::EmissiveLuma => "Emissive: Luminance (Gray)",

            Self::ConstantBlack => "Constant: 0.0 (Black)",
            Self::ConstantWhite => "Constant: 1.0 (White)",
            Self::ConstantGray => "Constant: 0.5 (Gray)",
        }
    }

    pub fn short_code(&self) -> &'static str {
        match self {
            Self::BaseColorR => "BC.R",
            Self::BaseColorG => "BC.G",
            Self::BaseColorB => "BC.B",
            Self::BaseColorA => "BC.A",
            Self::BaseColorLuma => "BC.Luma",
            Self::Roughness => "Rough",
            Self::Smoothness => "Smooth",
            Self::Metallic => "Metal",
            Self::AmbientOcclusion => "AO",
            Self::Height => "Height",
            Self::Opacity => "Opacity",
            Self::Clearcoat => "Clearcoat",
            Self::Anisotropy => "Aniso",
            Self::AnisotropyAngle => "AnisoAng",
            Self::NormalX => "Norm.X",
            Self::NormalY => "Norm.Y",
            Self::NormalYInverted => "Norm.-Y",
            Self::NormalZ => "Norm.Z",
            Self::BakedCurvature => "Curv",
            Self::BakedThickness => "Thick",
            Self::BakedWorldNormalX => "WN.X",
            Self::BakedWorldNormalY => "WN.Y",
            Self::BakedWorldNormalZ => "WN.Z",
            Self::BakedPositionX => "Pos.X",
            Self::BakedPositionY => "Pos.Y",
            Self::BakedPositionZ => "Pos.Z",
            Self::BakedId => "ID",
            Self::EmissiveR => "Emis.R",
            Self::EmissiveG => "Emis.G",
            Self::EmissiveB => "Emis.B",
            Self::EmissiveLuma => "Emis.Luma",
            Self::ConstantBlack => "0.0",
            Self::ConstantWhite => "1.0",
            Self::ConstantGray => "0.5",
        }
    }

    pub fn all() -> &'static [ChannelSource] {
        &[
            Self::BaseColorR,
            Self::BaseColorG,
            Self::BaseColorB,
            Self::BaseColorA,
            Self::BaseColorLuma,
            Self::Roughness,
            Self::Smoothness,
            Self::Metallic,
            Self::AmbientOcclusion,
            Self::Height,
            Self::Opacity,
            Self::Clearcoat,
            Self::Anisotropy,
            Self::AnisotropyAngle,
            Self::NormalX,
            Self::NormalY,
            Self::NormalYInverted,
            Self::NormalZ,
            Self::BakedCurvature,
            Self::BakedThickness,
            Self::BakedWorldNormalX,
            Self::BakedWorldNormalY,
            Self::BakedWorldNormalZ,
            Self::BakedPositionX,
            Self::BakedPositionY,
            Self::BakedPositionZ,
            Self::BakedId,
            Self::EmissiveR,
            Self::EmissiveG,
            Self::EmissiveB,
            Self::EmissiveLuma,
            Self::ConstantBlack,
            Self::ConstantWhite,
            Self::ConstantGray,
        ]
    }
}

/// A single channel routing slot with inversion control
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelSlot {
    pub source: ChannelSource,
    pub invert: bool,
}

impl Default for ChannelSlot {
    fn default() -> Self {
        Self {
            source: ChannelSource::ConstantBlack,
            invert: false,
        }
    }
}

impl ChannelSlot {
    pub fn new(source: ChannelSource) -> Self {
        Self {
            source,
            invert: false,
        }
    }

    pub fn new_inverted(source: ChannelSource) -> Self {
        Self {
            source,
            invert: true,
        }
    }

    pub fn label(&self) -> String {
        if self.invert {
            format!("Inv({})", self.source.short_code())
        } else {
            self.source.short_code().to_string()
        }
    }

    #[inline]
    pub fn sample(
        &self,
        x: u32,
        y: u32,
        textures: &CompositeTextureSet,
        combined_normal: &RgbaBuffer,
        baked: &BakedMapContext,
    ) -> f32 {
        let val = sample_channel_source(self.source, x, y, textures, combined_normal, baked);
        let out = if self.invert { 1.0 - val } else { val };
        out.clamp(0.0, 1.0)
    }
}

/// Color encoding and channel layout mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackedColorMode {
    Rgb,
    Rgba,
    Grayscale,
}

impl PackedColorMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rgb => "RGB (3 Channels)",
            Self::Rgba => "RGBA (4 Channels)",
            Self::Grayscale => "Grayscale (1 Channel)",
        }
    }
}

/// File container and compression format
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackedImageFormat {
    Png,
    Jpeg,
}

impl PackedImageFormat {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Png => "PNG (.png)",
            Self::Jpeg => "JPEG (.jpg)",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }
}

/// Definition of an exported packed texture file
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackedTextureMap {
    pub name_suffix: String,
    pub color_mode: PackedColorMode,
    pub format: PackedImageFormat,
    pub channel_r: ChannelSlot,
    pub channel_g: ChannelSlot,
    pub channel_b: ChannelSlot,
    pub channel_a: ChannelSlot,
    pub dilation_iterations: u32,
    pub enabled: bool,
}

impl PackedTextureMap {
    pub fn new_rgb(suffix: &str, r: ChannelSource, g: ChannelSource, b: ChannelSource) -> Self {
        Self {
            name_suffix: suffix.to_string(),
            color_mode: PackedColorMode::Rgb,
            format: PackedImageFormat::Png,
            channel_r: ChannelSlot::new(r),
            channel_g: ChannelSlot::new(g),
            channel_b: ChannelSlot::new(b),
            channel_a: ChannelSlot::new(ChannelSource::ConstantWhite),
            dilation_iterations: 16,
            enabled: true,
        }
    }

    pub fn new_rgba(suffix: &str, r: ChannelSource, g: ChannelSource, b: ChannelSource, a: ChannelSource) -> Self {
        Self {
            name_suffix: suffix.to_string(),
            color_mode: PackedColorMode::Rgba,
            format: PackedImageFormat::Png,
            channel_r: ChannelSlot::new(r),
            channel_g: ChannelSlot::new(g),
            channel_b: ChannelSlot::new(b),
            channel_a: ChannelSlot::new(a),
            dilation_iterations: 16,
            enabled: true,
        }
    }

    pub fn new_grayscale(suffix: &str, source: ChannelSource) -> Self {
        Self {
            name_suffix: suffix.to_string(),
            color_mode: PackedColorMode::Grayscale,
            format: PackedImageFormat::Png,
            channel_r: ChannelSlot::new(source),
            channel_g: ChannelSlot::new(ChannelSource::ConstantBlack),
            channel_b: ChannelSlot::new(ChannelSource::ConstantBlack),
            channel_a: ChannelSlot::new(ChannelSource::ConstantWhite),
            dilation_iterations: 16,
            enabled: true,
        }
    }

    pub fn channel_summary(&self) -> String {
        match self.color_mode {
            PackedColorMode::Grayscale => {
                format!("Gray: {}", self.channel_r.label())
            }
            PackedColorMode::Rgb => {
                format!(
                    "R: {} | G: {} | B: {}",
                    self.channel_r.label(),
                    self.channel_g.label(),
                    self.channel_b.label()
                )
            }
            PackedColorMode::Rgba => {
                format!(
                    "R: {} | G: {} | B: {} | A: {}",
                    self.channel_r.label(),
                    self.channel_g.label(),
                    self.channel_b.label(),
                    self.channel_a.label()
                )
            }
        }
    }
}

/// A complete custom packing preset containing multiple texture map rules
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomPackingPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub maps: Vec<PackedTextureMap>,
}

impl CustomPackingPreset {
    pub fn to_json_string(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn from_json_string(json_str: &str) -> Result<Self, String> {
        serde_json::from_str(json_str).map_err(|e| e.to_string())
    }

    pub fn save_to_json_file(&self, path: &Path) -> Result<(), String> {
        let json_str = self.to_json_string()?;
        std::fs::write(path, json_str).map_err(|e| e.to_string())
    }

    pub fn load_from_json_file(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::from_json_string(&content)
    }

    /// Unreal Engine ORM Packed Preset (BaseColor, Normal DirectX, ORM, Emissive, Opacity)
    pub fn unreal_orm() -> Self {
        Self {
            id: "unreal_orm".to_string(),
            name: "Unreal Engine 4/5 (ORM Packed)".to_string(),
            description: "Industry standard ORM map with Occlusion in Red, Roughness in Green, and Metallic in Blue.".to_string(),
            maps: vec![
                PackedTextureMap::new_rgb("_BaseColor", ChannelSource::BaseColorR, ChannelSource::BaseColorG, ChannelSource::BaseColorB),
                PackedTextureMap::new_rgb("_Normal", ChannelSource::NormalX, ChannelSource::NormalYInverted, ChannelSource::NormalZ),
                PackedTextureMap::new_rgb("_ORM", ChannelSource::AmbientOcclusion, ChannelSource::Roughness, ChannelSource::Metallic),
                PackedTextureMap::new_rgb("_Emissive", ChannelSource::EmissiveR, ChannelSource::EmissiveG, ChannelSource::EmissiveB),
                PackedTextureMap::new_grayscale("_Opacity", ChannelSource::Opacity),
            ],
        }
    }

    /// Unity HDRP MaskMap Preset (BaseColor Alpha Cutout, Normal OpenGL, MaskMap [R:Metallic, G:AO, B:Curvature, A:Smoothness])
    pub fn unity_hdrp() -> Self {
        let mut mask_map = PackedTextureMap::new_rgba(
            "_MaskMap",
            ChannelSource::Metallic,
            ChannelSource::AmbientOcclusion,
            ChannelSource::BakedCurvature,
            ChannelSource::Roughness,
        );
        mask_map.channel_a.invert = true; // Roughness inverted -> Smoothness

        Self {
            id: "unity_hdrp".to_string(),
            name: "Unity HDRP (MaskMap)".to_string(),
            description: "Unity High Definition Render Pipeline MaskMap: Metallic (R), AO (G), Detail/Curvature (B), Smoothness (A).".to_string(),
            maps: vec![
                PackedTextureMap::new_rgba("_BaseColor", ChannelSource::BaseColorR, ChannelSource::BaseColorG, ChannelSource::BaseColorB, ChannelSource::Opacity),
                PackedTextureMap::new_rgb("_Normal", ChannelSource::NormalX, ChannelSource::NormalY, ChannelSource::NormalZ),
                mask_map,
                PackedTextureMap::new_rgb("_Emissive", ChannelSource::EmissiveR, ChannelSource::EmissiveG, ChannelSource::EmissiveB),
            ],
        }
    }

    /// Unity URP / Standard Preset (Albedo Alpha Cutout, MetallicGloss, Normal OpenGL, Occlusion)
    pub fn unity_urp() -> Self {
        let mut metallic_gloss = PackedTextureMap::new_rgba(
            "_MetallicGloss",
            ChannelSource::Metallic,
            ChannelSource::ConstantBlack,
            ChannelSource::ConstantBlack,
            ChannelSource::Roughness,
        );
        metallic_gloss.channel_a.invert = true; // Smoothness

        Self {
            id: "unity_urp".to_string(),
            name: "Unity URP / Standard (MetallicGloss)".to_string(),
            description: "Universal Render Pipeline layout with Metallic in Red and Smoothness in Alpha.".to_string(),
            maps: vec![
                PackedTextureMap::new_rgba("_Albedo", ChannelSource::BaseColorR, ChannelSource::BaseColorG, ChannelSource::BaseColorB, ChannelSource::Opacity),
                metallic_gloss,
                PackedTextureMap::new_rgb("_Normal", ChannelSource::NormalX, ChannelSource::NormalY, ChannelSource::NormalZ),
                PackedTextureMap::new_grayscale("_Occlusion", ChannelSource::AmbientOcclusion),
                PackedTextureMap::new_rgb("_Emission", ChannelSource::EmissiveR, ChannelSource::EmissiveG, ChannelSource::EmissiveB),
            ],
        }
    }

    /// Godot 4 StandardMaterial3D Preset
    pub fn godot4() -> Self {
        Self {
            id: "godot4".to_string(),
            name: "Godot 4 Engine (StandardMaterial3D)".to_string(),
            description: "Godot 4 PBR layout with ORM packed texture and individual scalar maps.".to_string(),
            maps: vec![
                PackedTextureMap::new_rgb("_Albedo", ChannelSource::BaseColorR, ChannelSource::BaseColorG, ChannelSource::BaseColorB),
                PackedTextureMap::new_rgb("_Normal", ChannelSource::NormalX, ChannelSource::NormalY, ChannelSource::NormalZ),
                PackedTextureMap::new_rgb("_ORM", ChannelSource::AmbientOcclusion, ChannelSource::Roughness, ChannelSource::Metallic),
                PackedTextureMap::new_grayscale("_Height", ChannelSource::Height),
                PackedTextureMap::new_grayscale("_Roughness", ChannelSource::Roughness),
                PackedTextureMap::new_grayscale("_Metallic", ChannelSource::Metallic),
                PackedTextureMap::new_grayscale("_AO", ChannelSource::AmbientOcclusion),
                PackedTextureMap::new_rgb("_Emissive", ChannelSource::EmissiveR, ChannelSource::EmissiveG, ChannelSource::EmissiveB),
                PackedTextureMap::new_grayscale("_Opacity", ChannelSource::Opacity),
            ],
        }
    }

    /// Blender / glTF 2.0 PBR Standard Preset
    pub fn blender_gltf() -> Self {
        Self {
            id: "blender_gltf".to_string(),
            name: "Blender / glTF 2.0 Standard".to_string(),
            description: "glTF 2.0 PBR format with Roughness in Green and Metallic in Blue.".to_string(),
            maps: vec![
                PackedTextureMap::new_rgba("_BaseColor", ChannelSource::BaseColorR, ChannelSource::BaseColorG, ChannelSource::BaseColorB, ChannelSource::Opacity),
                PackedTextureMap::new_rgb("_Normal", ChannelSource::NormalX, ChannelSource::NormalY, ChannelSource::NormalZ),
                PackedTextureMap::new_rgb("_MetallicRoughness", ChannelSource::ConstantBlack, ChannelSource::Roughness, ChannelSource::Metallic),
                PackedTextureMap::new_grayscale("_AO", ChannelSource::AmbientOcclusion),
                PackedTextureMap::new_rgb("_Emissive", ChannelSource::EmissiveR, ChannelSource::EmissiveG, ChannelSource::EmissiveB),
            ],
        }
    }

    /// CryEngine / Lumberyard DDNA Preset
    pub fn cryengine_ddna() -> Self {
        let mut ddna = PackedTextureMap::new_rgba(
            "_DDNA",
            ChannelSource::NormalX,
            ChannelSource::NormalY,
            ChannelSource::NormalZ,
            ChannelSource::Roughness,
        );
        ddna.channel_a.invert = true; // Gloss in Alpha

        Self {
            id: "cryengine_ddna".to_string(),
            name: "CryEngine / Lumberyard (DDNA)".to_string(),
            description: "Normal map with Glossiness packed into the Alpha channel alongside Diffuse and Specular.".to_string(),
            maps: vec![
                PackedTextureMap::new_rgb("_Diffuse", ChannelSource::BaseColorR, ChannelSource::BaseColorG, ChannelSource::BaseColorB),
                ddna,
                PackedTextureMap::new_rgb("_Specular", ChannelSource::Metallic, ChannelSource::Metallic, ChannelSource::Metallic),
            ],
        }
    }

    /// Individual Isolated Channels Preset
    pub fn individual_channels() -> Self {
        Self {
            id: "individual_channels".to_string(),
            name: "Individual Channel Maps (Unpacked)".to_string(),
            description: "Exports every surface property as an individual standalone image file.".to_string(),
            maps: vec![
                PackedTextureMap::new_rgb("_BaseColor", ChannelSource::BaseColorR, ChannelSource::BaseColorG, ChannelSource::BaseColorB),
                PackedTextureMap::new_rgb("_Normal", ChannelSource::NormalX, ChannelSource::NormalY, ChannelSource::NormalZ),
                PackedTextureMap::new_grayscale("_Roughness", ChannelSource::Roughness),
                PackedTextureMap::new_grayscale("_Metallic", ChannelSource::Metallic),
                PackedTextureMap::new_grayscale("_AO", ChannelSource::AmbientOcclusion),
                PackedTextureMap::new_grayscale("_Height", ChannelSource::Height),
                PackedTextureMap::new_rgb("_Emissive", ChannelSource::EmissiveR, ChannelSource::EmissiveG, ChannelSource::EmissiveB),
                PackedTextureMap::new_grayscale("_Opacity", ChannelSource::Opacity),
                PackedTextureMap::new_grayscale("_Clearcoat", ChannelSource::Clearcoat),
                PackedTextureMap::new_grayscale("_Anisotropy", ChannelSource::Anisotropy),
            ],
        }
    }

    /// Custom Studio Default Template (Starter template for user editing)
    pub fn custom_studio_default() -> Self {
        Self {
            id: "custom_studio_default".to_string(),
            name: "Custom Studio Pipeline (Editable)".to_string(),
            description: "Fully customizable channel packing matrix. Add, edit, or re-route any channel.".to_string(),
            maps: vec![
                PackedTextureMap::new_rgb("_Albedo", ChannelSource::BaseColorR, ChannelSource::BaseColorG, ChannelSource::BaseColorB),
                PackedTextureMap::new_rgb("_Normal", ChannelSource::NormalX, ChannelSource::NormalYInverted, ChannelSource::NormalZ),
                PackedTextureMap::new_rgba("_PackedPBR", ChannelSource::AmbientOcclusion, ChannelSource::Roughness, ChannelSource::Metallic, ChannelSource::Height),
            ],
        }
    }

    /// All default built-in presets
    pub fn default_presets() -> Vec<Self> {
        vec![
            Self::unreal_orm(),
            Self::unity_hdrp(),
            Self::unity_urp(),
            Self::godot4(),
            Self::blender_gltf(),
            Self::cryengine_ddna(),
            Self::individual_channels(),
            Self::custom_studio_default(),
        ]
    }
}

/// Baked mesh map context passed to the packing engine
#[derive(Clone, Copy, Default, Debug)]
pub struct BakedMapContext<'a> {
    pub world_normal_map: Option<&'a RgbaBuffer>,
    pub world_space_normal_map: Option<&'a RgbaBuffer>,
    pub ao_map: Option<&'a ScalarBuffer>,
    pub curvature_map: Option<&'a ScalarBuffer>,
    pub position_map: Option<&'a RgbaBuffer>,
    pub thickness_map: Option<&'a ScalarBuffer>,
    pub id_map: Option<&'a RgbaBuffer>,
}

impl<'a> BakedMapContext<'a> {
    pub fn none() -> Self {
        Self::default()
    }
}

/// Core evaluation helper for sampling a channel source at pixel (x, y)
#[inline]
pub fn sample_channel_source(
    source: ChannelSource,
    x: u32,
    y: u32,
    textures: &CompositeTextureSet,
    combined_normal: &RgbaBuffer,
    baked: &BakedMapContext,
) -> f32 {
    match source {
        ChannelSource::BaseColorR => textures.base_color.get_pixel(x, y).x,
        ChannelSource::BaseColorG => textures.base_color.get_pixel(x, y).y,
        ChannelSource::BaseColorB => textures.base_color.get_pixel(x, y).z,
        ChannelSource::BaseColorA => textures.base_color.get_pixel(x, y).w,
        ChannelSource::BaseColorLuma => {
            let c = textures.base_color.get_pixel(x, y);
            0.2126 * c.x + 0.7152 * c.y + 0.0722 * c.z
        }

        ChannelSource::Roughness => textures.roughness.get_pixel(x, y),
        ChannelSource::Smoothness => 1.0 - textures.roughness.get_pixel(x, y),
        ChannelSource::Metallic => textures.metallic.get_pixel(x, y),
        ChannelSource::AmbientOcclusion => {
            if let Some(bao) = baked.ao_map {
                bao.get_pixel(x, y) * textures.ao.get_pixel(x, y)
            } else {
                textures.ao.get_pixel(x, y)
            }
        }
        ChannelSource::Height => textures.height_map.get_pixel(x, y),
        ChannelSource::Opacity => textures.opacity.get_pixel(x, y),
        ChannelSource::Clearcoat => textures.clearcoat.get_pixel(x, y),
        ChannelSource::Anisotropy => textures.anisotropy.get_pixel(x, y),
        ChannelSource::AnisotropyAngle => textures.anisotropy_angle.get_pixel(x, y),

        ChannelSource::NormalX => combined_normal.get_pixel(x, y).x,
        ChannelSource::NormalY => combined_normal.get_pixel(x, y).y,
        ChannelSource::NormalYInverted => 1.0 - combined_normal.get_pixel(x, y).y,
        ChannelSource::NormalZ => combined_normal.get_pixel(x, y).z,

        ChannelSource::BakedCurvature => {
            if let Some(curv) = baked.curvature_map {
                curv.get_pixel(x, y)
            } else {
                0.5
            }
        }
        ChannelSource::BakedThickness => {
            if let Some(thick) = baked.thickness_map {
                thick.get_pixel(x, y)
            } else {
                1.0
            }
        }
        ChannelSource::BakedWorldNormalX => {
            if let Some(wn) = baked.world_space_normal_map.or(baked.world_normal_map) {
                wn.get_pixel(x, y).x
            } else {
                0.5
            }
        }
        ChannelSource::BakedWorldNormalY => {
            if let Some(wn) = baked.world_space_normal_map.or(baked.world_normal_map) {
                wn.get_pixel(x, y).y
            } else {
                0.5
            }
        }
        ChannelSource::BakedWorldNormalZ => {
            if let Some(wn) = baked.world_space_normal_map.or(baked.world_normal_map) {
                wn.get_pixel(x, y).z
            } else {
                1.0
            }
        }
        ChannelSource::BakedPositionX => {
            if let Some(pos) = baked.position_map {
                pos.get_pixel(x, y).x
            } else {
                0.5
            }
        }
        ChannelSource::BakedPositionY => {
            if let Some(pos) = baked.position_map {
                pos.get_pixel(x, y).y
            } else {
                0.5
            }
        }
        ChannelSource::BakedPositionZ => {
            if let Some(pos) = baked.position_map {
                pos.get_pixel(x, y).z
            } else {
                0.5
            }
        }
        ChannelSource::BakedId => {
            if let Some(id) = baked.id_map {
                let p = id.get_pixel(x, y);
                0.2126 * p.x + 0.7152 * p.y + 0.0722 * p.z
            } else {
                0.0
            }
        }

        ChannelSource::EmissiveR => textures.emissive.get_pixel(x, y).x,
        ChannelSource::EmissiveG => textures.emissive.get_pixel(x, y).y,
        ChannelSource::EmissiveB => textures.emissive.get_pixel(x, y).z,
        ChannelSource::EmissiveLuma => {
            let c = textures.emissive.get_pixel(x, y);
            0.2126 * c.x + 0.7152 * c.y + 0.0722 * c.z
        }

        ChannelSource::ConstantBlack => 0.0,
        ChannelSource::ConstantWhite => 1.0,
        ChannelSource::ConstantGray => 0.5,
    }
}

/// Execution engine for custom texture packing
pub struct CustomTexturePacker;

impl CustomTexturePacker {
    pub fn pack_and_export(
        preset: &CustomPackingPreset,
        textures: &CompositeTextureSet,
        mesh_name: &str,
        output_dir: &Path,
        flip_v: bool,
        mesh: Option<&Mesh>,
        baked: &BakedMapContext,
    ) -> Result<Vec<PathBuf>, String> {
        std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
        let mut generated_files = Vec::new();

        let clean_name = mesh_name.replace(' ', "_").to_lowercase();
        let w = textures.width;
        let h = textures.height;

        if w == 0 || h == 0 {
            return Err("Texture dimensions must be greater than zero".to_string());
        }

        // Blends procedural height bump map into the tangent normal map
        let combined_normal = textures.generate_combined_normal(4.0);

        for map in &preset.maps {
            if !map.enabled {
                continue;
            }

            let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);
            let mut mask = vec![false; (w * h) as usize];

            // Parallel line-by-line rasterization of custom packed channels
            let raw_pixels: Vec<(u32, u32, [u8; 4], bool)> = (0..h)
                .into_par_iter()
                .flat_map(|y| {
                    let dst_y = if flip_v { h - 1 - y } else { y };
                    let mut row_pixels = Vec::with_capacity(w as usize);

                    for x in 0..w {
                        let active = textures.base_color.get_pixel(x, y).w > 0.001
                            || textures.opacity.get_pixel(x, y) > 0.001
                            || textures.metallic.get_pixel(x, y) > 0.001
                            || (textures.roughness.get_pixel(x, y) - 0.5).abs() > 0.001
                            || (textures.height_map.get_pixel(x, y) - 0.5).abs() > 0.001
                            || textures.emissive.get_pixel(x, y).truncate().length_squared() > 1e-4;

                        let pixel_bytes = match map.color_mode {
                            PackedColorMode::Grayscale => {
                                let val = (map.channel_r.sample(x, y, textures, &combined_normal, baked) * 255.0).clamp(0.0, 255.0) as u8;
                                [val, val, val, 255]
                            }
                            PackedColorMode::Rgb => {
                                let r = (map.channel_r.sample(x, y, textures, &combined_normal, baked) * 255.0).clamp(0.0, 255.0) as u8;
                                let g = (map.channel_g.sample(x, y, textures, &combined_normal, baked) * 255.0).clamp(0.0, 255.0) as u8;
                                let b = (map.channel_b.sample(x, y, textures, &combined_normal, baked) * 255.0).clamp(0.0, 255.0) as u8;
                                [r, g, b, 255]
                            }
                            PackedColorMode::Rgba => {
                                let r = (map.channel_r.sample(x, y, textures, &combined_normal, baked) * 255.0).clamp(0.0, 255.0) as u8;
                                let g = (map.channel_g.sample(x, y, textures, &combined_normal, baked) * 255.0).clamp(0.0, 255.0) as u8;
                                let b = (map.channel_b.sample(x, y, textures, &combined_normal, baked) * 255.0).clamp(0.0, 255.0) as u8;
                                let a = (map.channel_a.sample(x, y, textures, &combined_normal, baked) * 255.0).clamp(0.0, 255.0) as u8;
                                [r, g, b, a]
                            }
                        };

                        row_pixels.push((x, dst_y, pixel_bytes, active));
                    }
                    row_pixels
                })
                .collect();

            for (x, dst_y, rgba, active) in raw_pixels {
                img.put_pixel(x, dst_y, Rgba(rgba));
                mask[(dst_y * w + x) as usize] = active;
            }

            // Perform UV island edge dilation across seams
            if map.dilation_iterations > 0 {
                TextureExporter::dilate_image_rgba(&mut img, &mask, map.dilation_iterations);
            }

            // Save according to selected file format
            let file_name = format!("{}{}.{}", clean_name, map.name_suffix, map.format.extension());
            let target_path = output_dir.join(&file_name);

            match map.format {
                PackedImageFormat::Png => match map.color_mode {
                    PackedColorMode::Grayscale => {
                        image::DynamicImage::ImageRgba8(img)
                            .to_luma8()
                            .save(&target_path)
                            .map_err(|e| e.to_string())?;
                    }
                    PackedColorMode::Rgb => {
                        image::DynamicImage::ImageRgba8(img)
                            .to_rgb8()
                            .save(&target_path)
                            .map_err(|e| e.to_string())?;
                    }
                    PackedColorMode::Rgba => {
                        img.save(&target_path).map_err(|e| e.to_string())?;
                    }
                },
                PackedImageFormat::Jpeg => {
                    image::DynamicImage::ImageRgba8(img)
                        .to_rgb8()
                        .save(&target_path)
                        .map_err(|e| e.to_string())?;
                }
            }

            generated_files.push(target_path);
        }

        // Export 3D Mesh (.obj + .mtl, and binary .glb) alongside packed textures if requested
        if let Some(mesh) = mesh {
            let glb_path = output_dir.join(format!("{}.glb", clean_name));
            if mesh.save_to_glb_file(&glb_path).is_ok() {
                generated_files.push(glb_path);
            }

            let obj_path = output_dir.join(format!("{}.obj", clean_name));
            mesh.save_to_obj_file(&obj_path)?;
            generated_files.push(obj_path);

            let mtl_path = output_dir.join(format!("{}.mtl", clean_name));
            let mut mtl_content = format!(
                "# 3D Paintcraft Wavefront Material Definition\nnewmtl Material\nillum 2\nKa 1.0 1.0 1.0\nKd 1.0 1.0 1.0\nKs 0.5 0.5 0.5\n"
            );

            for file in &generated_files {
                if let Some(fname) = file.file_name().and_then(|f| f.to_str()) {
                    let lower = fname.to_lowercase();
                    if lower.contains("albedo") || lower.contains("basecolor") || lower.contains("diffuse") {
                        mtl_content.push_str(&format!("map_Kd {}\n", fname));
                    } else if lower.contains("normal") || lower.contains("ddna") {
                        mtl_content.push_str(&format!("map_Bump {}\nbump {}\n", fname, fname));
                    } else if lower.contains("roughness") || lower.contains("gloss") {
                        mtl_content.push_str(&format!("map_Ns {}\n", fname));
                    } else if lower.contains("metallic") || lower.contains("specular") {
                        mtl_content.push_str(&format!("map_Pm {}\n", fname));
                    } else if lower.contains("ao") || lower.contains("occlusion") {
                        mtl_content.push_str(&format!("map_Ka {}\n", fname));
                    } else if lower.contains("emissive") || lower.contains("emission") {
                        mtl_content.push_str(&format!("map_Ke {}\n", fname));
                    } else if lower.contains("opacity") {
                        mtl_content.push_str(&format!("map_d {}\n", fname));
                    }
                }
            }

            if std::fs::write(&mtl_path, mtl_content).is_ok() {
                generated_files.push(mtl_path);
            }
        }

        Ok(generated_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_packing_preset_serialization() {
        let preset = CustomPackingPreset::unreal_orm();
        let json_str = preset.to_json_string().unwrap();
        assert!(json_str.contains("unreal_orm"));
        assert!(json_str.contains("_ORM"));

        let restored = CustomPackingPreset::from_json_string(&json_str).unwrap();
        assert_eq!(restored.id, preset.id);
        assert_eq!(restored.maps.len(), preset.maps.len());
        assert_eq!(restored.maps[2].name_suffix, "_ORM");
        assert_eq!(restored.maps[2].channel_r.source, ChannelSource::AmbientOcclusion);
        assert_eq!(restored.maps[2].channel_g.source, ChannelSource::Roughness);
        assert_eq!(restored.maps[2].channel_b.source, ChannelSource::Metallic);
    }

    #[test]
    fn test_custom_packing_execute_unreal_orm() {
        let mut textures = CompositeTextureSet::new(16, 16);
        textures.ao.set_pixel(4, 4, 0.8);
        textures.roughness.set_pixel(4, 4, 0.4);
        textures.metallic.set_pixel(4, 4, 0.9);

        let temp_dir = std::env::temp_dir().join("test_custom_pack_unreal");
        let preset = CustomPackingPreset::unreal_orm();
        let exported = CustomTexturePacker::pack_and_export(
            &preset,
            &textures,
            "Weapon",
            &temp_dir,
            false,
            None,
            &BakedMapContext::none(),
        )
        .unwrap();

        assert_eq!(exported.len(), 5);
        let orm_path = temp_dir.join("weapon_ORM.png");
        assert!(orm_path.exists());

        let img = image::open(&orm_path).unwrap().into_rgba8();
        let p = img.get_pixel(4, 4);
        // AO ~0.8 -> ~204, Roughness ~0.4 -> ~102, Metallic ~0.9 -> ~229
        assert!((p[0] as i32 - (0.8 * 255.0) as i32).abs() <= 2);
        assert!((p[1] as i32 - (0.4 * 255.0) as i32).abs() <= 2);
        assert!((p[2] as i32 - (0.9 * 255.0) as i32).abs() <= 2);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_custom_packing_unity_hdrp_maskmap_inversion() {
        let mut textures = CompositeTextureSet::new(8, 8);
        textures.metallic.set_pixel(2, 2, 0.7);
        textures.ao.set_pixel(2, 2, 0.85);
        textures.roughness.set_pixel(2, 2, 0.3); // Inverted into Smoothness -> 1.0 - 0.3 = 0.7

        let temp_dir = std::env::temp_dir().join("test_custom_pack_unity_hdrp");
        let preset = CustomPackingPreset::unity_hdrp();
        let exported = CustomTexturePacker::pack_and_export(
            &preset,
            &textures,
            "Vehicle",
            &temp_dir,
            false,
            None,
            &BakedMapContext::none(),
        )
        .unwrap();

        assert_eq!(exported.len(), 4);
        let mask_path = temp_dir.join("vehicle_MaskMap.png");
        assert!(mask_path.exists());

        let img = image::open(&mask_path).unwrap().into_rgba8();
        let p = img.get_pixel(2, 2);
        // Metallic in R ~ 0.7 * 255 = 178
        assert!((p[0] as i32 - (0.7 * 255.0) as i32).abs() <= 2);
        // AO in G ~ 0.85 * 255 = 216
        assert!((p[1] as i32 - (0.85 * 255.0) as i32).abs() <= 2);
        // Alpha is Smoothness (1 - 0.3 = 0.7)
        assert!((p[3] as i32 - (0.7 * 255.0) as i32).abs() <= 2);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_custom_packing_grayscale_and_constants() {
        let textures = CompositeTextureSet::new(8, 8);
        let custom_preset = CustomPackingPreset {
            id: "test_constants".to_string(),
            name: "Test Constants".to_string(),
            description: "Test".to_string(),
            maps: vec![
                PackedTextureMap {
                    name_suffix: "_ConstantTest".to_string(),
                    color_mode: PackedColorMode::Rgb,
                    format: PackedImageFormat::Png,
                    channel_r: ChannelSlot::new(ChannelSource::ConstantBlack),
                    channel_g: ChannelSlot::new(ChannelSource::ConstantGray),
                    channel_b: ChannelSlot::new(ChannelSource::ConstantWhite),
                    channel_a: ChannelSlot::new(ChannelSource::ConstantWhite),
                    dilation_iterations: 0,
                    enabled: true,
                },
                PackedTextureMap::new_grayscale("_HeightGray", ChannelSource::Height),
            ],
        };

        let temp_dir = std::env::temp_dir().join("test_custom_pack_constants");
        let exported = CustomTexturePacker::pack_and_export(
            &custom_preset,
            &textures,
            "Prop",
            &temp_dir,
            false,
            None,
            &BakedMapContext::none(),
        )
        .unwrap();

        assert_eq!(exported.len(), 2);
        let const_img = image::open(temp_dir.join("prop_ConstantTest.png")).unwrap().into_rgba8();
        let p = const_img.get_pixel(0, 0);
        assert_eq!(p[0], 0);
        assert_eq!(p[1], 127);
        assert_eq!(p[2], 255);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pack_and_export_preserves_painted_colors_and_orm() {
        let mut proj = crate::project::Project::new_default(16, 16);
        // Add a custom fill layer with distinct blue color, low roughness, and high metallic
        proj.add_fill_layer(
            Some("Blue Steel".to_string()),
            glam::Vec4::new(0.1, 0.4, 0.9, 1.0),
            0.25,
            0.85,
        );
        proj.rebuild_composite();
        proj.sync_active_texture_set();

        let temp_dir = std::env::temp_dir().join("test_export_painted_blue_steel");
        let preset = CustomPackingPreset::unreal_orm();
        let exported = CustomTexturePacker::pack_and_export(
            &preset,
            &proj.composite_textures,
            "BlueSteelMesh",
            &temp_dir,
            false,
            None,
            &proj.baked_map_context(),
        )
        .unwrap();

        assert!(!exported.is_empty(), "Exported files list should not be empty");

        // Verify BaseColor is painted blue, NOT blank/white default
        let bc_path = temp_dir.join("bluesteelmesh_BaseColor.png");
        assert!(bc_path.exists(), "BaseColor file must exist on disk");
        let bc_img = image::open(&bc_path).unwrap().into_rgba8();
        let p_bc = bc_img.get_pixel(4, 4);
        assert!((p_bc[0] as i32 - (0.1 * 255.0) as i32).abs() <= 2, "Red channel must match painted 0.1");
        assert!((p_bc[1] as i32 - (0.4 * 255.0) as i32).abs() <= 2, "Green channel must match painted 0.4");
        assert!((p_bc[2] as i32 - (0.9 * 255.0) as i32).abs() <= 2, "Blue channel must match painted 0.9");

        // Verify ORM is packed correctly with painted Roughness (G) and Metallic (B)
        let orm_path = temp_dir.join("bluesteelmesh_ORM.png");
        assert!(orm_path.exists(), "ORM file must exist on disk");
        let orm_img = image::open(&orm_path).unwrap().into_rgba8();
        let p_orm = orm_img.get_pixel(4, 4);
        assert_eq!(p_orm[0], 255, "AO should default to 1.0 (255)");
        assert!((p_orm[1] as i32 - (0.25 * 255.0) as i32).abs() <= 2, "Roughness in Green must match painted 0.25");
        assert!((p_orm[2] as i32 - (0.85 * 255.0) as i32).abs() <= 2, "Metallic in Blue must match painted 0.85");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rgba_dilation_preserves_alpha_channel() {
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(4, 4);
        let mut mask = vec![false; 16];

        // Set pixel (1, 1) active with custom alpha 120
        img.put_pixel(1, 1, Rgba([200, 150, 100, 120]));
        mask[1 * 4 + 1] = true;

        TextureExporter::dilate_image_rgba(&mut img, &mask, 1);

        // Neighbor (2, 1) should be dilated from (1, 1) and receive alpha 120 (not forced 255)
        let dilated_p = img.get_pixel(2, 1);
        assert_eq!(dilated_p[0], 200);
        assert_eq!(dilated_p[1], 150);
        assert_eq!(dilated_p[2], 100);
        assert_eq!(dilated_p[3], 120, "Dilation on RGBA must preserve the alpha channel value");
    }
}

