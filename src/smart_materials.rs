// 3D Paintcraft - Smart Materials & Smart Masks Library
// Pure safe Rust

use crate::channel::{RgbaBuffer, ScalarBuffer};
use crate::layer::{BlendMode, Layer, MaskEffect, MaskEffectType, ProceduralGenerator};
use glam::Vec4;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SmartMaterialPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SmartMaskPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub struct SmartMaterialLibrary;

impl SmartMaterialLibrary {
    pub fn presets() -> &'static [SmartMaterialPreset] {
        &[
            SmartMaterialPreset {
                id: "worn_painted_steel",
                name: "Worn Painted Steel",
                category: "Metal",
                description: "Heavy industrial yellow machinery paint with red primer and exposed chipped steel edges",
            },
            SmartMaterialPreset {
                id: "rusted_cast_iron",
                name: "Rusted Cast Iron",
                category: "Metal",
                description: "Heavy cast iron with deep pitted cavity rust, surface oxidation, and scratches",
            },
            SmartMaterialPreset {
                id: "smart_rusted_green",
                name: "Rusted Army Metal",
                category: "Metal",
                description: "Pitted rust substrate with chipped weathered olive drab painted coating",
            },
            SmartMaterialPreset {
                id: "bronze_statue_patina",
                name: "Ancient Bronze & Patina",
                category: "Metal",
                description: "Polished antique bronze with oxidized brown crevices, verdigris turquoise patina, and upward dust",
            },
            SmartMaterialPreset {
                id: "scifi_carbon_armor",
                name: "Sci-Fi Composite Armor",
                category: "Sci-Fi",
                description: "Titanium chassis with chipped ceramic white armor plating and glowing cyan conduit accents",
            },
            SmartMaterialPreset {
                id: "desert_tactical_camo",
                name: "Desert Tactical Cerakote",
                category: "Tactical",
                description: "Dark anodized metal with desert tan cerakote, edge wear, camo patterns, and cavity sand",
            },
            SmartMaterialPreset {
                id: "dirty_machine_plastic",
                name: "Dirty Machine ABS Plastic",
                category: "Plastic",
                description: "Durable injection-molded ABS plastic with surface scuffs and cavity grease/grime",
            },
            SmartMaterialPreset {
                id: "gloss_car_paint",
                name: "Gloss Crimson Car Paint",
                category: "Automotive",
                description: "Deep metallic crimson flake coat with high-gloss clearcoat and road grime splatter",
            },
            SmartMaterialPreset {
                id: "weathered_wood",
                name: "Weathered Cedar Wood",
                category: "Organic",
                description: "Aged timber grain with organic color variation, deep tactile grooves, and crevice dirt",
            },
            SmartMaterialPreset {
                id: "brushed_gold",
                name: "Fine Brushed Gold",
                category: "Metal",
                description: "Fine luxury 24K gold with micro anisotropic brush scratches",
            },
            SmartMaterialPreset {
                id: "matte_polymer",
                name: "Matte Tactical Polymer",
                category: "Plastic",
                description: "Durable military tactical weapon and chassis polymer casing",
            },
            SmartMaterialPreset {
                id: "tinted_glass_visor",
                name: "Tinted Glass / Visor",
                category: "Glass",
                description: "Semi-transparent amber/blue tinted visor with smooth specular reflections, finger smudges, and subtle surface scratches",
            },
            SmartMaterialPreset {
                id: "perforated_steel_grate",
                name: "Perforated Steel Grate",
                category: "Metal",
                description: "Industrial steel plate with cutout hexagonal perforations, chipped edge highlights, and crevice dirt",
            },
            SmartMaterialPreset {
                id: "torn_camo_fabric",
                name: "Torn Camo Fabric",
                category: "Fabric",
                description: "Military camouflage cloth with ragged frayed tear holes cut out via opacity noise",
            },
            SmartMaterialPreset {
                id: "wire_chainlink_fence",
                name: "Wire Chainlink Fence",
                category: "Metal",
                description: "Galvanized steel diamond wire mesh with cut-out fence openings and junction oxidation",
            },
            SmartMaterialPreset {
                id: "emerald_crystal_gem",
                name: "Emerald Crystal Gem",
                category: "Gemstone",
                description: "Deep green translucent beryl crystal with physical IOR 1.58 refraction brilliance, clear internal light transmission, and facet edge highlights",
            },
            SmartMaterialPreset {
                id: "amber_fossil_resin",
                name: "Amber Fossil Resin",
                category: "Glass",
                description: "Translucent golden-amber fossil resin (IOR 1.54) with warm honey Beer-Lambert optical depth and microscopic internal clouding",
            },
            SmartMaterialPreset {
                id: "frosted_ice_glass",
                name: "Frosted Glacial Ice",
                category: "Glass",
                description: "Sub-zero glacial ice (IOR 1.31) with frosted microscopic microfacet transmission, cyan tint, and sub-surface fractures",
            },
        ]
    }

    /// Construct the full multi-layer stack for a smart material preset with curvature map only
    #[allow(dead_code)]
    pub fn build_layers(
        preset_id: &str,
        width: u32,
        height: u32,
        curvature_map: Option<&ScalarBuffer>,
    ) -> Vec<Layer> {
        Self::build_layers_with_maps(preset_id, width, height, curvature_map, None, None, None)
    }

    /// Construct full Substance 3D Painter-style multi-layer stacks using baked mesh maps and real masks
    pub fn build_layers_with_maps(
        preset_id: &str,
        width: u32,
        height: u32,
        curvature_map: Option<&ScalarBuffer>,
        ao_map: Option<&ScalarBuffer>,
        world_normal_map: Option<&RgbaBuffer>,
        position_map: Option<&RgbaBuffer>,
    ) -> Vec<Layer> {
        let mut layers = Vec::new();

        match preset_id {
            "worn_painted_steel" | "smart_chipped_steel" => {
                // Layer 1: Bare Gunmetal Steel Base
                let mut steel = Layer::new_fill_layer(
                    1,
                    "Steel Base".to_string(),
                    width,
                    height,
                    Vec4::new(0.56, 0.57, 0.58, 1.0),
                    0.32,
                    1.0,
                );
                steel.fill_height = 0.46;
                steel.fill_generator = ProceduralGenerator::Scratches;
                steel.generator_scale = 28.0;
                steel.generator_contrast = 0.8;
                steel.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(steel);

                // Layer 2: Red Oxide Primer Coat with Curvature Wear
                let mut primer = Layer::new_fill_layer(
                    2,
                    "Red Oxide Primer".to_string(),
                    width,
                    height,
                    Vec4::new(0.38, 0.14, 0.10, 1.0),
                    0.82,
                    0.0,
                );
                primer.fill_height = 0.50;
                primer.has_mask = true;
                primer.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Primer Edge Wear".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::CurvatureWear,
                        scale: 10.0,
                        contrast: 1.6,
                        invert: true,
                        balance: 0.58,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                primer.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Grunge Breakup".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WorleyGrunge,
                        scale: 14.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.5,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Multiply,
                });
                primer.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                primer.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(primer);

                // Layer 3: Machinery Yellow Paint with Chipped Edge Wear Mask
                let mut paint = Layer::new_fill_layer(
                    3,
                    "Machinery Yellow Paint".to_string(),
                    width,
                    height,
                    Vec4::new(0.92, 0.65, 0.08, 1.0),
                    0.36,
                    0.0,
                );
                paint.fill_height = 0.55;
                paint.fill_generator = ProceduralGenerator::PerlinNoise;
                paint.generator_scale = 40.0;
                paint.generator_contrast = 0.25;
                paint.has_mask = true;
                paint.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Chipped Paint Mask".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::CurvatureWear,
                        scale: 10.0,
                        contrast: 1.8,
                        invert: true,
                        balance: 0.44,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                paint.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Peeling Breakup".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::PeelingPaint,
                        scale: 12.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Multiply,
                });
                paint.mask_effects.push(MaskEffect {
                    id: 3,
                    name: "Fine Scratches".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::Scratches,
                        scale: 24.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.35,
                    blend_mode: BlendMode::Subtract,
                });
                paint.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                paint.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(paint);

                // Layer 4: Cavity Grease & Dirt
                let mut dirt = Layer::new_fill_layer(
                    4,
                    "Crevice Grease & Dirt".to_string(),
                    width,
                    height,
                    Vec4::new(0.08, 0.07, 0.06, 1.0),
                    0.88,
                    0.0,
                );
                dirt.fill_height = 0.52;
                dirt.blend_mode = BlendMode::Multiply;
                dirt.opacity = 0.85;
                dirt.has_mask = true;
                dirt.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Cavity Dirt (AO)".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 8.0,
                        contrast: 1.6,
                        invert: false,
                        balance: 0.52,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                dirt.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Dirt Grunge".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WorleyGrunge,
                        scale: 18.0,
                        contrast: 1.2,
                        invert: false,
                        balance: 0.5,
                    },
                    visible: true,
                    opacity: 0.75,
                    blend_mode: BlendMode::Multiply,
                });
                dirt.mask_effects.push(MaskEffect {
                    id: 3,
                    name: "Dirt Splatter".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::SplatterDirt,
                        scale: 24.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.30,
                    blend_mode: BlendMode::Add,
                });
                dirt.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                dirt.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(dirt);
            }

            "rusted_cast_iron" => {
                // Layer 1: Dark Pitted Cast Iron Base
                let mut iron = Layer::new_fill_layer(
                    1,
                    "Cast Iron Base".to_string(),
                    width,
                    height,
                    Vec4::new(0.32, 0.33, 0.35, 1.0),
                    0.52,
                    1.0,
                );
                iron.fill_height = 0.50;
                iron.fill_generator = ProceduralGenerator::WorleyGrunge;
                iron.generator_scale = 24.0;
                iron.generator_contrast = 0.5;
                iron.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(iron);

                // Layer 2: Deep Cavity Rust Layer
                let mut rust = Layer::new_fill_layer(
                    2,
                    "Deep Cavity Rust".to_string(),
                    width,
                    height,
                    Vec4::new(0.20, 0.08, 0.04, 1.0),
                    0.95,
                    0.0,
                );
                rust.fill_height = 0.42;
                rust.fill_generator = ProceduralGenerator::RustPits;
                rust.generator_scale = 16.0;
                rust.has_mask = true;
                rust.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Crevice Cavity Rust".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 10.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.54,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                rust.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Oxidation Craters".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::RustPits,
                        scale: 14.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.52,
                    },
                    visible: true,
                    opacity: 0.70,
                    blend_mode: BlendMode::Add,
                });
                rust.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                rust.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(rust);

                // Layer 3: Surface Rust Leaks & Bloom
                let mut surface_rust = Layer::new_fill_layer(
                    3,
                    "Surface Rust Leaks".to_string(),
                    width,
                    height,
                    Vec4::new(0.56, 0.22, 0.07, 1.0),
                    0.88,
                    0.0,
                );
                surface_rust.fill_height = 0.53;
                surface_rust.has_mask = true;
                surface_rust.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Downward Leaks".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::DrippingLeaks,
                        scale: 12.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.48,
                    },
                    visible: true,
                    opacity: 0.90,
                    blend_mode: BlendMode::Normal,
                });
                surface_rust.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Oxide Bloom".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::DomainWarpGrunge,
                        scale: 16.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.75,
                    blend_mode: BlendMode::Multiply,
                });
                surface_rust.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                surface_rust.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(surface_rust);

                // Layer 4: Polished Contact Edge Sheen
                let mut polish = Layer::new_fill_layer(
                    4,
                    "Polished Edge Sheen".to_string(),
                    width,
                    height,
                    Vec4::new(0.65, 0.66, 0.68, 1.0),
                    0.22,
                    1.0,
                );
                polish.fill_height = 0.51;
                polish.has_mask = true;
                polish.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Friction Highlight".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::MetalEdgeHighlight,
                        scale: 14.0,
                        contrast: 2.0,
                        invert: false,
                        balance: 0.52,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                polish.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                polish.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(polish);
            }

            "smart_rusted_green" => {
                // Layer 1: Steel Undercoat
                let mut steel = Layer::new_fill_layer(
                    1,
                    "Steel Undercoat".to_string(),
                    width,
                    height,
                    Vec4::new(0.54, 0.55, 0.56, 1.0),
                    0.38,
                    1.0,
                );
                steel.fill_height = 0.46;
                steel.fill_generator = ProceduralGenerator::Scratches;
                steel.generator_scale = 24.0;
                steel.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(steel);

                // Layer 2: Pitted Rust Substrate
                let mut rust = Layer::new_fill_layer(
                    2,
                    "Pitted Rust Substrate".to_string(),
                    width,
                    height,
                    Vec4::new(0.35, 0.16, 0.08, 1.0),
                    0.92,
                    0.0,
                );
                rust.fill_height = 0.49;
                rust.fill_generator = ProceduralGenerator::RustPits;
                rust.generator_scale = 18.0;
                rust.has_mask = true;
                rust.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Rust Spread".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::CurvatureWear,
                        scale: 10.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.52,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                rust.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Corrosion Grunge".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::DomainWarpGrunge,
                        scale: 14.0,
                        contrast: 1.2,
                        invert: false,
                        balance: 0.5,
                    },
                    visible: true,
                    opacity: 0.60,
                    blend_mode: BlendMode::Screen,
                });
                rust.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                rust.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(rust);

                // Layer 3: Weathered Olive Drab Paint with Chipped Edge Mask
                let mut green = Layer::new_fill_layer(
                    3,
                    "Army Green Paint".to_string(),
                    width,
                    height,
                    Vec4::new(0.24, 0.30, 0.18, 1.0),
                    0.78,
                    0.0,
                );
                green.fill_height = 0.54;
                green.fill_generator = ProceduralGenerator::PerlinNoise;
                green.generator_scale = 36.0;
                green.has_mask = true;
                green.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Chipped Paint Edge".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::CurvatureWear,
                        scale: 10.0,
                        contrast: 1.8,
                        invert: true,
                        balance: 0.44,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                green.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Peeling Grunge".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::PeelingPaint,
                        scale: 14.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.5,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Multiply,
                });
                green.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                green.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(green);

                // Layer 4: Chemical Rust Dripping Runs
                let mut leaks = Layer::new_fill_layer(
                    4,
                    "Dripping Rust Leaks".to_string(),
                    width,
                    height,
                    Vec4::new(0.52, 0.22, 0.08, 1.0),
                    0.85,
                    0.0,
                );
                leaks.fill_height = 0.52;
                leaks.has_mask = true;
                leaks.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Gravity Drips".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::DrippingLeaks,
                        scale: 12.0,
                        contrast: 1.6,
                        invert: false,
                        balance: 0.46,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Normal,
                });
                leaks.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Grunge Breakup".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WorleyGrunge,
                        scale: 20.0,
                        contrast: 1.2,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.70,
                    blend_mode: BlendMode::Multiply,
                });
                leaks.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                leaks.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(leaks);

                // Layer 5: Field Dust
                let mut dust = Layer::new_fill_layer(
                    5,
                    "Field Sand & Dust".to_string(),
                    width,
                    height,
                    Vec4::new(0.60, 0.54, 0.42, 1.0),
                    0.95,
                    0.0,
                );
                dust.fill_height = 0.55;
                dust.has_mask = true;
                dust.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Upward Dust".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::DirectionalDust,
                        scale: 8.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.75,
                    blend_mode: BlendMode::Normal,
                });
                dust.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                dust.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(dust);
            }

            "bronze_statue_patina" | "smart_ancient_gold" => {
                // Layer 1: Polished Antique Bronze Base
                let mut bronze = Layer::new_fill_layer(
                    1,
                    "Polished Bronze Base".to_string(),
                    width,
                    height,
                    Vec4::new(0.88, 0.62, 0.36, 1.0),
                    0.28,
                    1.0,
                );
                bronze.fill_height = 0.50;
                bronze.fill_generator = ProceduralGenerator::Scratches;
                bronze.generator_scale = 26.0;
                bronze.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(bronze);

                // Layer 2: Oxidized Brown Crevices
                let mut oxidized = Layer::new_fill_layer(
                    2,
                    "Oxidized Brown Crevices".to_string(),
                    width,
                    height,
                    Vec4::new(0.22, 0.14, 0.09, 1.0),
                    0.72,
                    0.20,
                );
                oxidized.fill_height = 0.48;
                oxidized.has_mask = true;
                oxidized.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Oxidation Wear".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::CurvatureWear,
                        scale: 8.0,
                        contrast: 1.4,
                        invert: true,
                        balance: 0.42,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                oxidized.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Cavity Darkening".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 10.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.55,
                    },
                    visible: true,
                    opacity: 0.60,
                    blend_mode: BlendMode::Add,
                });
                oxidized.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                oxidized.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(oxidized);

                // Layer 3: Verdigris Turquoise Patina Crust
                let mut patina = Layer::new_fill_layer(
                    3,
                    "Verdigris Turquoise Patina".to_string(),
                    width,
                    height,
                    Vec4::new(0.18, 0.68, 0.56, 1.0),
                    0.90,
                    0.0,
                );
                patina.fill_height = 0.54;
                patina.fill_generator = ProceduralGenerator::PerlinNoise;
                patina.generator_scale = 32.0;
                patina.generator_contrast = 0.35;
                patina.has_mask = true;
                patina.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Cavity Patina (AO)".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 10.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.54,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                patina.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Upward Weathering".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::DirectionalDust,
                        scale: 12.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.65,
                    blend_mode: BlendMode::Add,
                });
                patina.mask_effects.push(MaskEffect {
                    id: 3,
                    name: "Mineral Breakup".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::DomainWarpGrunge,
                        scale: 16.0,
                        contrast: 1.2,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.80,
                    blend_mode: BlendMode::Multiply,
                });
                patina.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                patina.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(patina);

                // Layer 4: Burnished Highlight Edge Sheen
                let mut highlight = Layer::new_fill_layer(
                    4,
                    "Burnished Highlight Edge Sheen".to_string(),
                    width,
                    height,
                    Vec4::new(0.96, 0.76, 0.46, 1.0),
                    0.14,
                    1.0,
                );
                highlight.fill_height = 0.51;
                highlight.has_mask = true;
                highlight.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Peak Edge Highlight".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::MetalEdgeHighlight,
                        scale: 16.0,
                        contrast: 2.2,
                        invert: false,
                        balance: 0.52,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                highlight.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                highlight.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(highlight);
            }

            "scifi_carbon_armor" | "scifi_hull" => {
                // Layer 1: Carbon Fiber Weave Core
                let mut core = Layer::new_fill_layer(
                    1,
                    "Carbon Fiber Core".to_string(),
                    width,
                    height,
                    Vec4::new(0.12, 0.12, 0.13, 1.0),
                    0.35,
                    0.0,
                );
                core.fill_height = 0.46;
                core.fill_generator = ProceduralGenerator::FabricWeave;
                core.generator_scale = 36.0;
                core.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(core);

                // Layer 2: Dark Titanium Chassis Frame
                let mut chassis = Layer::new_fill_layer(
                    2,
                    "Titanium Chassis Frame".to_string(),
                    width,
                    height,
                    Vec4::new(0.54, 0.56, 0.58, 1.0),
                    0.30,
                    1.0,
                );
                chassis.fill_height = 0.50;
                chassis.fill_generator = ProceduralGenerator::PerlinNoise;
                chassis.generator_scale = 20.0;
                chassis.has_mask = true;
                chassis.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Frame Seams".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::CurvatureWear,
                        scale: 10.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                chassis.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                chassis.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(chassis);

                // Layer 3: White Ceramic Armor Plating
                let mut plating = Layer::new_fill_layer(
                    3,
                    "Ceramic Armor Plating".to_string(),
                    width,
                    height,
                    Vec4::new(0.90, 0.91, 0.93, 1.0),
                    0.24,
                    0.0,
                );
                plating.fill_height = 0.57;
                plating.has_mask = true;
                plating.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Plate Edge Chipping".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::CurvatureWear,
                        scale: 12.0,
                        contrast: 1.8,
                        invert: true,
                        balance: 0.44,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                plating.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Plate Flaking".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WorleyGrunge,
                        scale: 16.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Multiply,
                });
                plating.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                plating.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(plating);

                // Layer 4: Cyan Energy Conduits
                let mut conduit = Layer::new_fill_layer(
                    4,
                    "Cyan Conduit Accents".to_string(),
                    width,
                    height,
                    Vec4::new(0.0, 0.92, 1.0, 1.0),
                    0.12,
                    0.0,
                );
                conduit.fill_height = 0.44;
                conduit.fill_emissive = Vec4::new(0.0, 1.0, 1.0, 1.0);
                conduit.channel_flags.emission = true;
                conduit.emissive_buf.data.fill(Vec4::new(0.0, 1.0, 1.0, 1.0));
                conduit.has_mask = true;
                conduit.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Panel Seams (AO)".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 14.0,
                        contrast: 1.8,
                        invert: false,
                        balance: 0.62,
                    },
                    visible: true,
                    opacity: 0.90,
                    blend_mode: BlendMode::Normal,
                });
                conduit.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                conduit.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(conduit);
            }

            "desert_tactical_camo" => {
                // Layer 1: Dark Parkerized Gunmetal
                let mut metal = Layer::new_fill_layer(
                    1,
                    "Parkerized Gunmetal Base".to_string(),
                    width,
                    height,
                    Vec4::new(0.42, 0.44, 0.46, 1.0),
                    0.36,
                    1.0,
                );
                metal.fill_height = 0.46;
                metal.fill_generator = ProceduralGenerator::Scratches;
                metal.generator_scale = 28.0;
                metal.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(metal);

                // Layer 2: Desert Tan Cerakote Coat
                let mut cerakote = Layer::new_fill_layer(
                    2,
                    "Desert Tan Cerakote".to_string(),
                    width,
                    height,
                    Vec4::new(0.78, 0.66, 0.48, 1.0),
                    0.72,
                    0.0,
                );
                cerakote.fill_height = 0.53;
                cerakote.has_mask = true;
                cerakote.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Edge Wear".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::CurvatureWear,
                        scale: 12.0,
                        contrast: 1.7,
                        invert: true,
                        balance: 0.45,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                cerakote.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Scratches".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::Scratches,
                        scale: 24.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.5,
                    },
                    visible: true,
                    opacity: 0.75,
                    blend_mode: BlendMode::Multiply,
                });
                cerakote.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                cerakote.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(cerakote);

                // Layer 3: Coyote Camo Splotches
                let mut camo = Layer::new_fill_layer(
                    3,
                    "Coyote Camo Splotches".to_string(),
                    width,
                    height,
                    Vec4::new(0.44, 0.36, 0.24, 1.0),
                    0.72,
                    0.0,
                );
                camo.fill_height = 0.53;
                camo.has_mask = true;
                camo.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Camo Patterns".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WorleyGrunge,
                        scale: 4.0,
                        contrast: 2.2,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                camo.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                camo.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(camo);

                // Layer 4: Sand & Dust Accumulation
                let mut sand = Layer::new_fill_layer(
                    4,
                    "Cavity Sand & Dust".to_string(),
                    width,
                    height,
                    Vec4::new(0.82, 0.74, 0.56, 1.0),
                    0.96,
                    0.0,
                );
                sand.fill_height = 0.56;
                sand.has_mask = true;
                sand.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Crevice Sand (AO)".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 10.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.54,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                sand.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Directional Settling".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::DirectionalDust,
                        scale: 14.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.70,
                    blend_mode: BlendMode::Add,
                });
                sand.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                sand.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(sand);
            }

            "dirty_machine_plastic" | "smart_dirty_plastic" => {
                // Layer 1: Molded ABS White Plastic Base
                let mut plastic = Layer::new_fill_layer(
                    1,
                    "ABS White Plastic".to_string(),
                    width,
                    height,
                    Vec4::new(0.88, 0.89, 0.90, 1.0),
                    0.35,
                    0.0,
                );
                plastic.fill_height = 0.50;
                plastic.fill_generator = ProceduralGenerator::PerlinNoise;
                plastic.generator_scale = 32.0;
                plastic.generator_contrast = 0.20;
                plastic.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(plastic);

                // Layer 2: Hand Friction Polish
                let mut polish = Layer::new_fill_layer(
                    2,
                    "Hand Friction Polish".to_string(),
                    width,
                    height,
                    Vec4::new(0.90, 0.91, 0.92, 1.0),
                    0.16,
                    0.0,
                );
                polish.fill_height = 0.50;
                polish.has_mask = true;
                polish.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Friction Zones".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::MetalEdgeHighlight,
                        scale: 12.0,
                        contrast: 1.8,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                polish.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                polish.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(polish);

                // Layer 3: Surface Scuffs & Micro Scratches
                let mut scuffs = Layer::new_fill_layer(
                    3,
                    "Surface Scuffs".to_string(),
                    width,
                    height,
                    Vec4::new(0.72, 0.74, 0.76, 1.0),
                    0.70,
                    0.0,
                );
                scuffs.fill_height = 0.48;
                scuffs.has_mask = true;
                scuffs.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Abrasions".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::Scratches,
                        scale: 22.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.80,
                    blend_mode: BlendMode::Normal,
                });
                scuffs.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                scuffs.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(scuffs);

                // Layer 4: Crevice Oil & Grime (AO)
                let mut grime = Layer::new_fill_layer(
                    4,
                    "Crevice Grease & Grime".to_string(),
                    width,
                    height,
                    Vec4::new(0.12, 0.10, 0.08, 1.0),
                    0.92,
                    0.0,
                );
                grime.fill_height = 0.52;
                grime.blend_mode = BlendMode::Multiply;
                grime.opacity = 0.85;
                grime.has_mask = true;
                grime.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Crevice Grease (AO)".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 8.0,
                        contrast: 1.6,
                        invert: false,
                        balance: 0.54,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                grime.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Oil Grunge".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WorleyGrunge,
                        scale: 18.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.75,
                    blend_mode: BlendMode::Multiply,
                });
                grime.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                grime.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(grime);
            }

            "gloss_car_paint" => {
                // Layer 1: Metallic Crimson Base
                let mut base = Layer::new_fill_layer(
                    1,
                    "Metallic Crimson Base".to_string(),
                    width,
                    height,
                    Vec4::new(0.68, 0.04, 0.08, 1.0),
                    0.32,
                    0.75,
                );
                base.fill_height = 0.50;
                base.fill_generator = ProceduralGenerator::PerlinNoise;
                base.generator_scale = 48.0;
                base.generator_contrast = 0.35;
                base.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(base);

                // Layer 2: High-Gloss Clearcoat Top Layer
                let mut clearcoat = Layer::new_fill_layer(
                    2,
                    "High-Gloss Clearcoat".to_string(),
                    width,
                    height,
                    Vec4::new(1.0, 1.0, 1.0, 0.20),
                    0.06,
                    0.0,
                );
                clearcoat.fill_height = 0.54;
                clearcoat.fill_clearcoat = 1.0;
                clearcoat.channel_flags.clearcoat = true;
                clearcoat.blend_mode = BlendMode::Screen;
                clearcoat.opacity = 0.30;
                clearcoat.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(clearcoat);

                // Layer 3: Lower Body Road Mud & Splash
                let mut road_grime = Layer::new_fill_layer(
                    3,
                    "Road Grime & Splatter".to_string(),
                    width,
                    height,
                    Vec4::new(0.24, 0.19, 0.14, 1.0),
                    0.94,
                    0.0,
                );
                road_grime.fill_height = 0.58;
                road_grime.has_mask = true;
                road_grime.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Lower Mesh Splash".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::GroundSplash,
                        scale: 14.0,
                        contrast: 1.6,
                        invert: false,
                        balance: 0.52,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                road_grime.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Dirt Splatter".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WorleyGrunge,
                        scale: 24.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.80,
                    blend_mode: BlendMode::Multiply,
                });
                road_grime.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                road_grime.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(road_grime);
            }

            "weathered_wood" => {
                // Layer 1: Dark Aged Timber Substrate
                let mut substrate = Layer::new_fill_layer(
                    1,
                    "Dark Timber Substrate".to_string(),
                    width,
                    height,
                    Vec4::new(0.32, 0.20, 0.12, 1.0),
                    0.72,
                    0.0,
                );
                substrate.fill_height = 0.46;
                substrate.fill_generator = ProceduralGenerator::WoodGrain;
                substrate.generator_scale = 10.0;
                substrate.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(substrate);

                // Layer 2: Weathered Cedar Grain
                let mut grain = Layer::new_fill_layer(
                    2,
                    "Weathered Cedar Grain".to_string(),
                    width,
                    height,
                    Vec4::new(0.55, 0.46, 0.36, 1.0),
                    0.62,
                    0.0,
                );
                grain.fill_height = 0.54;
                grain.fill_generator = ProceduralGenerator::PerlinNoise;
                grain.generator_scale = 18.0;
                grain.has_mask = true;
                grain.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Wood Grain Mask".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WoodGrain,
                        scale: 10.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Normal,
                });
                grain.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Weathered Ridge Bleach".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::DirectionalDust,
                        scale: 8.0,
                        contrast: 1.2,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.40,
                    blend_mode: BlendMode::Add,
                });
                grain.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                grain.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(grain);

                // Layer 3: Crevice Moss & Damp Grime
                let mut moss = Layer::new_fill_layer(
                    3,
                    "Crevice Moss & Grime".to_string(),
                    width,
                    height,
                    Vec4::new(0.18, 0.26, 0.12, 1.0),
                    0.94,
                    0.0,
                );
                moss.fill_height = 0.52;
                moss.has_mask = true;
                moss.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Crevice Moisture (AO)".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 10.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.56,
                    },
                    visible: true,
                    opacity: 0.90,
                    blend_mode: BlendMode::Normal,
                });
                moss.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Moss Clumping".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::DomainWarpGrunge,
                        scale: 16.0,
                        contrast: 1.2,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.75,
                    blend_mode: BlendMode::Multiply,
                });
                moss.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                moss.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(moss);
            }

            "brushed_gold" => {
                // Layer 1: Fine 24K Brushed Gold Base
                let mut gold = Layer::new_fill_layer(
                    1,
                    "Fine Brushed Gold".to_string(),
                    width,
                    height,
                    Vec4::new(1.0, 0.76, 0.33, 1.0),
                    0.22,
                    1.0,
                );
                gold.fill_height = 0.50;
                gold.fill_generator = ProceduralGenerator::Scratches;
                gold.generator_scale = 32.0;
                gold.generator_contrast = 0.50;
                gold.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(gold);

                // Layer 2: Polished Edge Highlight Sheen
                let mut sheen = Layer::new_fill_layer(
                    2,
                    "Polished Edge Sheen".to_string(),
                    width,
                    height,
                    Vec4::new(1.0, 0.88, 0.52, 1.0),
                    0.10,
                    1.0,
                );
                sheen.fill_height = 0.51;
                sheen.has_mask = true;
                sheen.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Edge Highlight".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::MetalEdgeHighlight,
                        scale: 16.0,
                        contrast: 2.2,
                        invert: false,
                        balance: 0.52,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                sheen.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                sheen.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(sheen);

                // Layer 3: Micro Hairline Scratches
                let mut scratches = Layer::new_fill_layer(
                    3,
                    "Micro Hairline Scratches".to_string(),
                    width,
                    height,
                    Vec4::new(0.92, 0.70, 0.28, 1.0),
                    0.45,
                    1.0,
                );
                scratches.fill_height = 0.48;
                scratches.has_mask = true;
                scratches.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Hairline Scratches".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::Scratches,
                        scale: 24.0,
                        contrast: 1.6,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.60,
                    blend_mode: BlendMode::Normal,
                });
                scratches.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                scratches.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(scratches);

                // Layer 4: Tarnished Cavity Shadows
                let mut tarnish = Layer::new_fill_layer(
                    4,
                    "Tarnished Cavity Shadows".to_string(),
                    width,
                    height,
                    Vec4::new(0.48, 0.32, 0.12, 1.0),
                    0.65,
                    0.85,
                );
                tarnish.fill_height = 0.49;
                tarnish.has_mask = true;
                tarnish.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Recessed Tarnish (AO)".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 10.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.56,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Normal,
                });
                tarnish.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                tarnish.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(tarnish);
            }

            "matte_polymer" => {
                // Layer 1: Tactical Polymer Base
                let mut polymer = Layer::new_fill_layer(
                    1,
                    "Tactical Polymer Base".to_string(),
                    width,
                    height,
                    Vec4::new(0.18, 0.19, 0.20, 1.0),
                    0.62,
                    0.0,
                );
                polymer.fill_height = 0.50;
                polymer.fill_generator = ProceduralGenerator::PerlinNoise;
                polymer.generator_scale = 36.0;
                polymer.generator_contrast = 0.40;
                polymer.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(polymer);

                // Layer 2: Convex Edge Handling Friction Wear
                let mut wear = Layer::new_fill_layer(
                    2,
                    "Edge Friction Wear".to_string(),
                    width,
                    height,
                    Vec4::new(0.24, 0.25, 0.26, 1.0),
                    0.26,
                    0.0,
                );
                wear.fill_height = 0.49;
                wear.has_mask = true;
                wear.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Edge Rub Wear".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::CurvatureWear,
                        scale: 12.0,
                        contrast: 1.6,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Normal,
                });
                wear.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                wear.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(wear);

                // Layer 3: Field Scratches & Holster Scuffs
                let mut scuffs = Layer::new_fill_layer(
                    3,
                    "Field Scratches".to_string(),
                    width,
                    height,
                    Vec4::new(0.14, 0.15, 0.16, 1.0),
                    0.80,
                    0.0,
                );
                scuffs.fill_height = 0.47;
                scuffs.has_mask = true;
                scuffs.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Scuffs".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::Scratches,
                        scale: 22.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.70,
                    blend_mode: BlendMode::Normal,
                });
                scuffs.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                scuffs.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(scuffs);

                // Layer 4: Cavity Field Dust
                let mut dust = Layer::new_fill_layer(
                    4,
                    "Cavity Field Dust".to_string(),
                    width,
                    height,
                    Vec4::new(0.68, 0.62, 0.50, 1.0),
                    0.94,
                    0.0,
                );
                dust.fill_height = 0.52;
                dust.has_mask = true;
                dust.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Recessed Dust (AO)".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 10.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.55,
                    },
                    visible: true,
                    opacity: 0.65,
                    blend_mode: BlendMode::Normal,
                });
                dust.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                dust.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(dust);
            }

            "tinted_glass_visor" => {
                // Layer 1: Semi-Transparent Visor Glass
                let mut glass = Layer::new_fill_layer(
                    1,
                    "Visor Glass Tint".to_string(),
                    width,
                    height,
                    Vec4::new(0.18, 0.28, 0.40, 1.0),
                    0.06,
                    0.0,
                );
                glass.fill_opacity = 0.52;
                glass.channel_flags.opacity = true;
                glass.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(glass);

                // Layer 2: Fingerprint Smudges & Surface Haze
                let mut smudges = Layer::new_fill_layer(
                    2,
                    "Surface Smudges & Grease".to_string(),
                    width,
                    height,
                    Vec4::new(0.35, 0.42, 0.50, 1.0),
                    0.48,
                    0.0,
                );
                smudges.fill_opacity = 0.70;
                smudges.channel_flags.opacity = false;
                smudges.has_mask = true;
                smudges.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Oil Smudges".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WorleyGrunge,
                        scale: 14.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.48,
                    },
                    visible: true,
                    opacity: 0.65,
                    blend_mode: BlendMode::Normal,
                });
                smudges.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                smudges.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(smudges);

                // Layer 3: Edge Scuffs & Micro Scratches
                let mut scuffs = Layer::new_fill_layer(
                    3,
                    "Edge Scuffs & Scratches".to_string(),
                    width,
                    height,
                    Vec4::new(0.85, 0.88, 0.92, 1.0),
                    0.75,
                    0.0,
                );
                scuffs.fill_opacity = 0.88;
                scuffs.channel_flags.opacity = false;
                scuffs.has_mask = true;
                scuffs.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Edge Scuffs".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::CurvatureWear,
                        scale: 12.0,
                        contrast: 1.6,
                        invert: false,
                        balance: 0.52,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Normal,
                });
                scuffs.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Micro Scratches".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::Scratches,
                        scale: 28.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.5,
                    },
                    visible: true,
                    opacity: 0.70,
                    blend_mode: BlendMode::Add,
                });
                scuffs.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                scuffs.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(scuffs);
            }

            "perforated_steel_grate" => {
                // Layer 1: Cutout Holes (Transparent Substrate)
                let mut holes = Layer::new_fill_layer(
                    1,
                    "Cutout Perforations".to_string(),
                    width,
                    height,
                    Vec4::new(0.1, 0.1, 0.1, 1.0),
                    0.5,
                    0.0,
                );
                holes.fill_height = 0.0;
                holes.fill_opacity = 0.0;
                holes.channel_flags.opacity = true;
                holes.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(holes);

                // Layer 2: Perforated Steel Plate
                let mut plate = Layer::new_fill_layer(
                    2,
                    "PBR Steel Grate".to_string(),
                    width,
                    height,
                    Vec4::new(0.72, 0.74, 0.76, 1.0),
                    0.34,
                    0.96,
                );
                plate.fill_height = 0.54;
                plate.fill_opacity = 1.0;
                plate.channel_flags.opacity = true;
                plate.has_mask = true;
                plate.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Hexagonal Punch Holes".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::PerforatedMesh,
                        scale: 16.0,
                        contrast: 2.0,
                        invert: false,
                        balance: 0.5,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                plate.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                plate.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(plate);

                // Layer 3: Crevice Grime & Edge Wear around holes
                let mut grime = Layer::new_fill_layer(
                    3,
                    "Hole Edge Chipping & Grime".to_string(),
                    width,
                    height,
                    Vec4::new(0.28, 0.20, 0.14, 1.0),
                    0.82,
                    0.10,
                );
                grime.fill_height = 0.49;
                grime.has_mask = true;
                grime.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Cavity Dirt".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 8.0,
                        contrast: 1.5,
                        invert: false,
                        balance: 0.55,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Normal,
                });
                grime.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Oxidation Pits".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::RustPits,
                        scale: 16.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.50,
                    blend_mode: BlendMode::Add,
                });
                grime.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                grime.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(grime);
            }

            "torn_camo_fabric" => {
                // Layer 1: Cutout Holes
                let mut cutout = Layer::new_fill_layer(
                    1,
                    "Cutout Tears".to_string(),
                    width,
                    height,
                    Vec4::new(0.1, 0.1, 0.1, 1.0),
                    0.5,
                    0.0,
                );
                cutout.fill_height = 0.0;
                cutout.fill_opacity = 0.0;
                cutout.channel_flags.opacity = true;
                cutout.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(cutout);

                // Layer 2: Camo Cloth Pattern
                let mut cloth = Layer::new_fill_layer(
                    2,
                    "Torn Camo Fabric".to_string(),
                    width,
                    height,
                    Vec4::new(0.38, 0.42, 0.28, 1.0),
                    0.85,
                    0.0,
                );
                cloth.fill_height = 0.52;
                cloth.fill_opacity = 1.0;
                cloth.channel_flags.opacity = true;
                cloth.fill_generator = ProceduralGenerator::FabricWeave;
                cloth.generator_scale = 32.0;
                cloth.has_mask = true;
                cloth.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Torn Ragged Edge".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::TornFabricEdge,
                        scale: 6.0,
                        contrast: 1.8,
                        invert: false,
                        balance: 0.5,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                cloth.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                cloth.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(cloth);

                // Layer 3: Frayed Edge Threads
                let mut fringe = Layer::new_fill_layer(
                    3,
                    "Frayed Thread Fringe".to_string(),
                    width,
                    height,
                    Vec4::new(0.55, 0.58, 0.44, 1.0),
                    0.95,
                    0.0,
                );
                fringe.fill_height = 0.54;
                fringe.has_mask = true;
                fringe.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Tear Fringe".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::TornFabricEdge,
                        scale: 6.0,
                        contrast: 2.2,
                        invert: false,
                        balance: 0.48,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                fringe.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Weave Threads".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::FabricWeave,
                        scale: 40.0,
                        contrast: 1.6,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Multiply,
                });
                fringe.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                fringe.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(fringe);
            }

            "wire_chainlink_fence" => {
                // Layer 1: Open Space Cutout
                let mut space = Layer::new_fill_layer(
                    1,
                    "Fence Cutout".to_string(),
                    width,
                    height,
                    Vec4::new(0.1, 0.1, 0.1, 1.0),
                    0.5,
                    0.0,
                );
                space.fill_height = 0.0;
                space.fill_opacity = 0.0;
                space.channel_flags.opacity = true;
                space.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(space);

                // Layer 2: Galvanized Zinc Wire
                let mut wire = Layer::new_fill_layer(
                    2,
                    "Galvanized Steel Wire".to_string(),
                    width,
                    height,
                    Vec4::new(0.80, 0.82, 0.85, 1.0),
                    0.36,
                    0.92,
                );
                wire.fill_height = 0.56;
                wire.fill_opacity = 1.0;
                wire.channel_flags.opacity = true;
                wire.has_mask = true;
                wire.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Diamond Wire Mesh".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WireFence,
                        scale: 14.0,
                        contrast: 2.0,
                        invert: false,
                        balance: 0.5,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                wire.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                wire.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(wire);

                // Layer 3: Junction Oxidation Rust
                let mut rust = Layer::new_fill_layer(
                    3,
                    "Junction Oxidation Rust".to_string(),
                    width,
                    height,
                    Vec4::new(0.48, 0.20, 0.08, 1.0),
                    0.88,
                    0.0,
                );
                rust.fill_height = 0.54;
                rust.has_mask = true;
                rust.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Junction Crevice (AO)".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 10.0,
                        contrast: 1.6,
                        invert: false,
                        balance: 0.55,
                    },
                    visible: true,
                    opacity: 0.85,
                    blend_mode: BlendMode::Normal,
                });
                rust.mask_effects.push(MaskEffect {
                    id: 2,
                    name: "Rust Pits".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::RustPits,
                        scale: 16.0,
                        contrast: 1.3,
                        invert: false,
                        balance: 0.50,
                    },
                    visible: true,
                    opacity: 0.60,
                    blend_mode: BlendMode::Add,
                });
                rust.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                rust.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(rust);
            }

            "emerald_crystal_gem" => {
                // Layer 1: Refractive Emerald Crystal Core (IOR 1.58 Transmission)
                let mut gem = Layer::new_fill_layer(
                    1,
                    "Emerald Crystal Body".to_string(),
                    width,
                    height,
                    Vec4::new(0.04, 0.72, 0.32, 1.0),
                    0.04,
                    0.0,
                );
                gem.fill_height = 0.50;
                gem.fill_opacity = 0.15;
                gem.channel_flags.opacity = true;
                gem.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(gem);

                // Layer 2: Facet Edge Reflection & Sheen
                let mut edge = Layer::new_fill_layer(
                    2,
                    "Facet Edge Highlights".to_string(),
                    width,
                    height,
                    Vec4::new(0.35, 0.95, 0.55, 1.0),
                    0.02,
                    0.0,
                );
                edge.fill_height = 0.52;
                edge.fill_opacity = 0.25;
                edge.channel_flags.opacity = true;
                edge.has_mask = true;
                edge.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Facet Edges".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::MetalEdgeHighlight,
                        scale: 14.0,
                        contrast: 2.0,
                        invert: false,
                        balance: 0.52,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
                edge.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                edge.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(edge);

                // Layer 3: Surface Contact Micro-Scuffs
                let mut scuffs = Layer::new_fill_layer(
                    3,
                    "Surface Micro-Scuffs".to_string(),
                    width,
                    height,
                    Vec4::new(0.6, 0.9, 0.7, 1.0),
                    0.38,
                    0.0,
                );
                scuffs.fill_height = 0.49;
                scuffs.channel_flags.base_color = false;
                scuffs.channel_flags.roughness = true;
                scuffs.has_mask = true;
                scuffs.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Fine Scratches".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::Scratches,
                        scale: 24.0,
                        contrast: 2.2,
                        invert: false,
                        balance: 0.45,
                    },
                    visible: true,
                    opacity: 0.40,
                    blend_mode: BlendMode::Normal,
                });
                scuffs.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                scuffs.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(scuffs);
            }

            "amber_fossil_resin" => {
                // Layer 1: Translucent Golden Amber Base (IOR 1.54)
                let mut amber = Layer::new_fill_layer(
                    1,
                    "Golden Amber Resin".to_string(),
                    width,
                    height,
                    Vec4::new(0.92, 0.55, 0.06, 1.0),
                    0.06,
                    0.0,
                );
                amber.fill_height = 0.50;
                amber.fill_opacity = 0.22;
                amber.channel_flags.opacity = true;
                amber.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(amber);

                // Layer 2: Internal Clouded Inclusions
                let mut cloud = Layer::new_fill_layer(
                    2,
                    "Internal Clouding".to_string(),
                    width,
                    height,
                    Vec4::new(0.72, 0.38, 0.04, 1.0),
                    0.26,
                    0.0,
                );
                cloud.fill_height = 0.50;
                cloud.fill_opacity = 0.52;
                cloud.channel_flags.opacity = true;
                cloud.has_mask = true;
                cloud.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Cellular Inclusions".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::WorleyGrunge,
                        scale: 10.0,
                        contrast: 1.6,
                        invert: false,
                        balance: 0.52,
                    },
                    visible: true,
                    opacity: 0.65,
                    blend_mode: BlendMode::Normal,
                });
                cloud.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                cloud.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(cloud);

                // Layer 3: Crevice Sediment (Cavity Grime)
                let mut grime = Layer::new_fill_layer(
                    3,
                    "Cavity Dirt".to_string(),
                    width,
                    height,
                    Vec4::new(0.25, 0.18, 0.12, 1.0),
                    0.88,
                    0.0,
                );
                grime.fill_height = 0.52;
                grime.has_mask = true;
                grime.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Cavity Dirt (AO)".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::AmbientOcclusion,
                        scale: 8.0,
                        contrast: 1.4,
                        invert: false,
                        balance: 0.54,
                    },
                    visible: true,
                    opacity: 0.70,
                    blend_mode: BlendMode::Normal,
                });
                grime.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                grime.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(grime);
            }

            "frosted_ice_glass" => {
                // Layer 1: Frosted Glacial Ice Substrate (IOR 1.31)
                let mut ice = Layer::new_fill_layer(
                    1,
                    "Glacial Ice Core".to_string(),
                    width,
                    height,
                    Vec4::new(0.72, 0.90, 0.96, 1.0),
                    0.38,
                    0.0,
                );
                ice.fill_height = 0.50;
                ice.fill_opacity = 0.28;
                ice.channel_flags.opacity = true;
                ice.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(ice);

                // Layer 2: Sub-surface Ice Fractures & Veins
                let mut fractures = Layer::new_fill_layer(
                    2,
                    "Internal Fractures".to_string(),
                    width,
                    height,
                    Vec4::new(0.92, 0.98, 1.0, 1.0),
                    0.12,
                    0.0,
                );
                fractures.fill_height = 0.48;
                fractures.fill_opacity = 0.60;
                fractures.channel_flags.opacity = true;
                fractures.has_mask = true;
                fractures.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Fracture Lines".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::RidgedMultifractal,
                        scale: 12.0,
                        contrast: 2.4,
                        invert: false,
                        balance: 0.48,
                    },
                    visible: true,
                    opacity: 0.75,
                    blend_mode: BlendMode::Normal,
                });
                fractures.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                fractures.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(fractures);

                // Layer 3: Surface Frost Crystals
                let mut frost = Layer::new_fill_layer(
                    3,
                    "Surface Frost Spores".to_string(),
                    width,
                    height,
                    Vec4::new(1.0, 1.0, 1.0, 1.0),
                    0.92,
                    0.0,
                );
                frost.fill_height = 0.53;
                frost.has_mask = true;
                frost.mask_effects.push(MaskEffect {
                    id: 1,
                    name: "Frost Crystals".to_string(),
                    effect_type: MaskEffectType::Fill {
                        generator: ProceduralGenerator::SplatterDirt,
                        scale: 18.0,
                        contrast: 1.8,
                        invert: false,
                        balance: 0.45,
                    },
                    visible: true,
                    opacity: 0.45,
                    blend_mode: BlendMode::Normal,
                });
                frost.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
                frost.evaluate_fill_advanced(curvature_map, position_map, world_normal_map, ao_map);
                layers.push(frost);
            }

            _ => {
                let default_layer = Layer::new_fill_layer(
                    1,
                    "Default Material".to_string(),
                    width,
                    height,
                    Vec4::new(0.6, 0.6, 0.6, 1.0),
                    0.5,
                    0.0,
                );
                layers.push(default_layer);
            }
        }

        layers
    }

    pub fn smart_masks() -> &'static [SmartMaskPreset] {
        &[
            SmartMaskPreset {
                id: "curvature_edge_wear",
                name: "Edge Wear & Paint Chipping",
                description: "Realistic convex edge wear with Worley flake breakup, ridged fractures, and crevice gating",
            },
            SmartMaskPreset {
                id: "cavity_dirt",
                name: "Cavity Dirt & Crevice Grime (AO)",
                description: "Deep AO shadow sediment accumulation with organic domain-warped dirt clumps and rust specks",
            },
            SmartMaskPreset {
                id: "directional_dust",
                name: "Directional Upward Dust (+Y)",
                description: "Sky-facing dust accumulation driven by World Normal +Y, settling into upward concave pockets",
            },
            SmartMaskPreset {
                id: "dripping_rust_leaks",
                name: "Dripping Rust Leaks (-Y)",
                description: "Gravity-directed fluid runoffs and vertical chemical rust flow streaks originating from cavities",
            },
            SmartMaskPreset {
                id: "surface_scratches",
                name: "Surface Scratches & Abrasions",
                description: "Multi-frequency directional scratches modulated by high-contact wear zones",
            },
            SmartMaskPreset {
                id: "peeling_paint",
                name: "Peeling & Flaking Paint",
                description: "Organic cellular blisters with sharp jagged chipped paint perimeter lines",
            },
            SmartMaskPreset {
                id: "rust_crater_pits",
                name: "Oxidation Rust Pits",
                description: "Corrosion crater clusters with porous oxidation cores and micro pitting",
            },
            SmartMaskPreset {
                id: "ground_dirt_mud",
                name: "Ground Mud & Splash Splatter",
                description: "Bottom height gradient with dense mud droplets and turbulent ground contact splash",
            },
            SmartMaskPreset {
                id: "heavy_machinery_wear",
                name: "Heavy Machinery Weathering",
                description: "Multi-layered compound wear: chipped edges + cavity grease + abrasive scuffs + grunge breakup",
            },
            SmartMaskPreset {
                id: "gun_metal_edge",
                name: "Gun Metal Precision Edge Wear",
                description: "Fine razor-sharp micro edge highlights and firearm slide contact wear",
            },
            SmartMaskPreset {
                id: "patina_oxidation",
                name: "Verdigris & Oxidation Bloom",
                description: "Antique chemical oxidation blooming organically outward from deep crevices",
            },
            SmartMaskPreset {
                id: "mask_perforated_mesh",
                name: "Perforated Hex Holes",
                description: "Industrial punched sheet metal hexagonal hole cutout mask",
            },
            SmartMaskPreset {
                id: "mask_wire_diamond",
                name: "Diamond Wire Fence Mesh",
                description: "45-degree diamond crisscross wire fence mesh cutout",
            },
            SmartMaskPreset {
                id: "mask_torn_edge",
                name: "Torn Fabric Ragged Cutout",
                description: "Organic ragged cloth tear with fine boundary fiber fringe",
            },
        ]
    }

    /// Apply a Smart Mask preset to a layer as a non-destructive mask effect stack
    #[allow(dead_code)]
    pub fn apply_smart_mask(
        mask_id: &str,
        layer: &mut Layer,
        curvature_map: Option<&ScalarBuffer>,
        ao_map: Option<&ScalarBuffer>,
    ) {
        Self::apply_smart_mask_advanced(mask_id, layer, curvature_map, ao_map, None, None);
    }

    /// Full multi-map smart mask application supporting Normal and Position maps
    pub fn apply_smart_mask_advanced(
        mask_id: &str,
        layer: &mut Layer,
        curvature_map: Option<&ScalarBuffer>,
        ao_map: Option<&ScalarBuffer>,
        position_map: Option<&RgbaBuffer>,
        world_normal_map: Option<&RgbaBuffer>,
    ) {
        layer.preserve_base_paint_if_needed();
        layer.has_mask = true;

        match mask_id {
            "curvature_edge_wear" | "mask_edge_wear" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::CurvatureWear, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, ref mut balance, .. } = eff.effect_type {
                        *scale = 10.0;
                        *contrast = 1.8;
                        *balance = 0.52;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::WorleyGrunge, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.85;
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, .. } = eff.effect_type {
                        *scale = 14.0;
                        *contrast = 1.4;
                    }
                }
                let id3 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::Scratches, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id3) {
                    eff.blend_mode = BlendMode::Screen;
                    eff.opacity = 0.35;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 22.0;
                    }
                }
                layer.next_mask_effect_id += 1;
                layer.mask_effects.push(MaskEffect {
                    id: layer.next_mask_effect_id,
                    name: "Edge Levels (Sharpen Boundary)".to_string(),
                    effect_type: MaskEffectType::FilterLevels {
                        in_black: 0.10,
                        in_white: 0.80,
                        gamma: 0.95,
                        out_black: 0.0,
                        out_white: 1.0,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
            }
            "cavity_dirt" | "mask_crevice_dirt" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::AmbientOcclusion, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, ref mut balance, .. } = eff.effect_type {
                        *scale = 8.0;
                        *contrast = 1.4;
                        *balance = 0.54;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::DomainWarpGrunge, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.80;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 12.0;
                    }
                }
                let id3 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::SplatterDirt, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id3) {
                    eff.blend_mode = BlendMode::Add;
                    eff.opacity = 0.40;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 18.0;
                    }
                }
            }
            "directional_dust" | "mask_directional_dust" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::DirectionalDust, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, .. } = eff.effect_type {
                        *scale = 6.0;
                        *contrast = 1.2;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::BillowNoise, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.65;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 8.0;
                    }
                }
                let id3 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::SplatterDirt, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id3) {
                    eff.blend_mode = BlendMode::Add;
                    eff.opacity = 0.25;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 24.0;
                    }
                }
            }
            "dripping_rust_leaks" | "mask_rust_leaks" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::DrippingLeaks, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, ref mut balance, .. } = eff.effect_type {
                        *scale = 8.0;
                        *contrast = 1.7;
                        *balance = 0.46;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::DomainWarpGrunge, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.70;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 10.0;
                    }
                }
                let id3 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::AmbientOcclusion, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id3) {
                    eff.blend_mode = BlendMode::Screen;
                    eff.opacity = 0.45;
                }
            }
            "surface_scratches" | "mask_grunge" | "mask_scratches" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::Scratches, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, .. } = eff.effect_type {
                        *scale = 12.0;
                        *contrast = 1.8;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::DomainWarpGrunge, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.75;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 6.0;
                    }
                }
                let id3 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::Scratches, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id3) {
                    eff.blend_mode = BlendMode::Screen;
                    eff.opacity = 0.45;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 28.0;
                    }
                }
            }
            "peeling_paint" | "mask_peeling_paint" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::PeelingPaint, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, ref mut balance, .. } = eff.effect_type {
                        *scale = 12.0;
                        *contrast = 1.8;
                        *balance = 0.50;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::RidgedMultifractal, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Screen;
                    eff.opacity = 0.45;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 14.0;
                    }
                }
                layer.next_mask_effect_id += 1;
                layer.mask_effects.push(MaskEffect {
                    id: layer.next_mask_effect_id,
                    name: "Peeling Edge Levels".to_string(),
                    effect_type: MaskEffectType::FilterLevels {
                        in_black: 0.15,
                        in_white: 0.75,
                        gamma: 1.1,
                        out_black: 0.0,
                        out_white: 1.0,
                    },
                    visible: true,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                });
            }
            "rust_crater_pits" | "mask_rust_pits" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::RustPits, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, ref mut balance, .. } = eff.effect_type {
                        *scale = 14.0;
                        *contrast = 1.6;
                        *balance = 0.52;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::AmbientOcclusion, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.70;
                }
            }
            "ground_dirt_mud" | "mask_ground_dirt" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::GroundSplash, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    if let MaskEffectType::Fill { ref mut contrast, ref mut balance, .. } = eff.effect_type {
                        *contrast = 1.6;
                        *balance = 0.52;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::SplatterDirt, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Screen;
                    eff.opacity = 0.80;
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, .. } = eff.effect_type {
                        *scale = 12.0;
                        *contrast = 1.8;
                    }
                }
                let id3 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::DomainWarpGrunge, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id3) {
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.70;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 8.0;
                    }
                }
            }
            "heavy_machinery_wear" | "mask_heavy_machinery" => {
                layer.add_mask_fill_effect_advanced(ProceduralGenerator::CurvatureWear, curvature_map, ao_map, position_map, world_normal_map);
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::AmbientOcclusion, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Add;
                    eff.opacity = 0.60;
                }
                let id3 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::DomainWarpGrunge, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id3) {
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.75;
                }
                let id4 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::Scratches, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id4) {
                    eff.blend_mode = BlendMode::Add;
                    eff.opacity = 0.35;
                }
            }
            "gun_metal_edge" | "mask_gun_metal" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::MetalEdgeHighlight, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, ref mut balance, .. } = eff.effect_type {
                        *scale = 16.0;
                        *contrast = 2.2;
                        *balance = 0.50;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::Scratches, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.70;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 32.0;
                    }
                }
            }
            "patina_oxidation" | "mask_patina" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::AmbientOcclusion, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, ref mut balance, .. } = eff.effect_type {
                        *scale = 6.0;
                        *contrast = 1.3;
                        *balance = 0.58;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::DomainWarpGrunge, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.blend_mode = BlendMode::Screen;
                    eff.opacity = 0.65;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 10.0;
                    }
                }
                let id3 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::BillowNoise, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id3) {
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.75;
                }
            }
            "mask_invert" => {
                layer.add_mask_filter_invert(curvature_map);
            }
            "mask_perforated_mesh" | "mask_perforated" => {
                let id = layer.add_mask_fill_effect_advanced(ProceduralGenerator::PerforatedMesh, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id) {
                    eff.name = "Perforated Hex Punch".to_string();
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, .. } = eff.effect_type {
                        *scale = 16.0;
                        *contrast = 2.0;
                    }
                }
            }
            "mask_wire_diamond" => {
                let id = layer.add_mask_fill_effect_advanced(ProceduralGenerator::WireFence, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id) {
                    eff.name = "Diamond Wire Mesh".to_string();
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, .. } = eff.effect_type {
                        *scale = 14.0;
                        *contrast = 2.0;
                    }
                }
            }
            "mask_torn_edge" => {
                let id1 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::TornFabricEdge, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id1) {
                    eff.name = "Torn Ragged Edge".to_string();
                    if let MaskEffectType::Fill { ref mut scale, ref mut contrast, .. } = eff.effect_type {
                        *scale = 6.0;
                        *contrast = 1.8;
                    }
                }
                let id2 = layer.add_mask_fill_effect_advanced(ProceduralGenerator::FabricWeave, curvature_map, ao_map, position_map, world_normal_map);
                if let Some(eff) = layer.mask_effects.iter_mut().find(|e| e.id == id2) {
                    eff.name = "Fabric Fringe".to_string();
                    eff.blend_mode = BlendMode::Multiply;
                    eff.opacity = 0.45;
                    if let MaskEffectType::Fill { ref mut scale, .. } = eff.effect_type {
                        *scale = 32.0;
                    }
                }
            }
            _ => {
                layer.add_mask_fill_effect_advanced(ProceduralGenerator::PerlinNoise, curvature_map, ao_map, position_map, world_normal_map);
            }
        }

        layer.evaluate_mask_advanced(curvature_map, ao_map, position_map, world_normal_map);
    }

    /// Save layer stack into a .spmt JSON smart material bundle file
    pub fn export_smart_material_to_file(path: &std::path::Path, layers: &[Layer]) -> Result<(), String> {
        let json = serde_json::to_string_pretty(layers).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    /// Load layer stack from a .spmt JSON smart material bundle file
    pub fn load_smart_material_from_file(
        path: &std::path::Path,
        width: u32,
        height: u32,
        curvature_map: Option<&ScalarBuffer>,
    ) -> Result<Vec<Layer>, String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut layers: Vec<Layer> = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        for layer in &mut layers {
            layer.base_color_buf = crate::channel::RgbaBuffer::new(width, height, glam::Vec4::ZERO);
            layer.roughness_buf = ScalarBuffer::new(width, height, 0.5);
            layer.metallic_buf = ScalarBuffer::new(width, height, 0.0);
            layer.height_buf = ScalarBuffer::new(width, height, 0.5);
            layer.normal_buf = crate::channel::RgbaBuffer::new(width, height, glam::Vec4::new(0.5, 0.5, 1.0, 0.5));
            layer.emissive_buf = crate::channel::RgbaBuffer::new(width, height, glam::Vec4::ZERO);
            layer.opacity_buf = ScalarBuffer::new(width, height, 1.0);
            layer.clearcoat_buf = ScalarBuffer::new(width, height, 0.0);
            layer.mask_buf = ScalarBuffer::new(width, height, 1.0);
            if layer.layer_type == crate::layer::LayerType::Fill {
                layer.evaluate_fill(curvature_map);
            }
            if layer.has_mask {
                layer.evaluate_mask(curvature_map);
            }
        }
        Ok(layers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::CompositeTextureSet;

    #[test]
    fn test_smart_material_layer_stacks() {
        let presets = SmartMaterialLibrary::presets();
        assert!(!presets.is_empty(), "Preset list should not be empty");

        for preset in presets {
            let layers = SmartMaterialLibrary::build_layers_with_maps(
                preset.id,
                64,
                64,
                None,
                None,
                None,
                None,
            );
            assert!(!layers.is_empty(), "Preset {} should generate layers", preset.id);

            // Presets with multiple layers should have active masks on upper coats
            if layers.len() > 1 && preset.id != "matte_polymer" {
                let has_masked_layer = layers.iter().skip(1).any(|l| l.has_mask);
                assert!(has_masked_layer, "Multi-layer smart material {} should have masked upper layers", preset.id);
            }
        }
    }

    #[test]
    fn test_smart_mask_application() {
        let mut layer = Layer::new_fill_layer(
            1,
            "Test Layer".to_string(),
            64,
            64,
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            0.5,
            0.0,
        );

        for mask in SmartMaterialLibrary::smart_masks() {
            layer.mask_effects.clear();
            SmartMaterialLibrary::apply_smart_mask(mask.id, &mut layer, None, None);
            assert!(layer.has_mask, "Mask {} should enable layer has_mask", mask.id);
            assert!(!layer.mask_effects.is_empty(), "Mask {} should add mask effects", mask.id);
        }
    }

    #[test]
    fn test_opacity_smart_materials_and_masks() {
        let opacity_materials = [
            "tinted_glass_visor",
            "perforated_steel_grate",
            "torn_camo_fabric",
            "wire_chainlink_fence",
            "emerald_crystal_gem",
            "amber_fossil_resin",
            "frosted_ice_glass",
        ];

        for mat_id in opacity_materials {
            let layers = SmartMaterialLibrary::build_layers_with_maps(
                mat_id,
                32,
                32,
                None,
                None,
                None,
                None,
            );
            assert!(!layers.is_empty(), "Material {} should generate layers", mat_id);
            let has_opacity_flag = layers.iter().any(|l| l.channel_flags.opacity);
            assert!(has_opacity_flag, "Material {} must have at least one layer with opacity channel enabled", mat_id);

            let mut comp = CompositeTextureSet::new(32, 32);
            comp.composite_layers(&layers);

            if mat_id == "tinted_glass_visor" {
                let op = comp.opacity.get_pixel(16, 16);
                assert!((op - 0.52).abs() < 1e-3, "Tinted glass visor composite opacity should be ~0.52, got {}", op);
            }
        }

        let opacity_masks = [
            "mask_perforated_mesh",
            "mask_wire_diamond",
            "mask_torn_edge",
        ];

        let mut layer = Layer::new_fill_layer(
            1,
            "Opacity Mask Target".to_string(),
            32,
            32,
            Vec4::ONE,
            0.5,
            0.0,
        );

        for mask_id in opacity_masks {
            layer.mask_effects.clear();
            SmartMaterialLibrary::apply_smart_mask(mask_id, &mut layer, None, None);
            assert!(layer.has_mask, "Mask {} should enable mask", mask_id);
            assert!(!layer.mask_effects.is_empty(), "Mask {} must add mask effects", mask_id);

            layer.evaluate_mask(None);
            assert!(layer.mask_buf.data.iter().all(|&v| (0.0..=1.0).contains(&v)));
        }
    }
}
