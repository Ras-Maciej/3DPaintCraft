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
        // Built-in smart material presets cleared to allow user-created custom materials
        &[]
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
    /// Construct full Substance 3D Painter-style multi-layer stacks
    pub fn build_layers_with_maps(
        _preset_id: &str,
        width: u32,
        height: u32,
        _curvature_map: Option<&ScalarBuffer>,
        _ao_map: Option<&ScalarBuffer>,
        _world_normal_map: Option<&RgbaBuffer>,
        _position_map: Option<&RgbaBuffer>,
    ) -> Vec<Layer> {
        let default_layer = Layer::new_fill_layer(
            1,
            "Default Material".to_string(),
            width,
            height,
            Vec4::new(0.6, 0.6, 0.6, 1.0),
            0.5,
            0.0,
        );
        vec![default_layer]
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
                id: "mask_editor",
                name: "Substance Mask Editor (Multi-Map)",
                description: "Compound weathering fusing Curvature, AO, World Normal direction, Position height, and Grunge",
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
                layer.add_mask_generator_curvature_wear(curvature_map, ao_map, position_map, world_normal_map);
            }
            "cavity_dirt" | "mask_crevice_dirt" => {
                layer.add_mask_generator_cavity_dirt(curvature_map, ao_map, position_map, world_normal_map);
            }
            "directional_dust" | "mask_directional_dust" => {
                layer.add_mask_generator_directional_dust(curvature_map, ao_map, position_map, world_normal_map);
            }
            "mask_editor" | "mask_editor_compound" => {
                layer.add_mask_generator_mask_editor(curvature_map, ao_map, position_map, world_normal_map);
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
                layer.add_mask_generator_ground_dirt(curvature_map, ao_map, position_map, world_normal_map);
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
            layer.anisotropy_buf = ScalarBuffer::new(width, height, 0.0);
            layer.anisotropy_angle_buf = ScalarBuffer::new(width, height, 0.0);
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
        assert!(presets.is_empty(), "Built-in smart materials should be cleared for user-created materials");

        let default_layers = SmartMaterialLibrary::build_layers_with_maps(
            "custom_user_mat",
            64,
            64,
            None,
            None,
            None,
            None,
        );
        assert!(!default_layers.is_empty(), "Fallback should generate valid layer");

        // Verify custom user smart material serialization & roundtrip
        let temp_dir = std::env::temp_dir();
        let spmt_path = temp_dir.join("test_user_mat.spmt");

        let mut user_layer1 = Layer::new_fill_layer(
            1,
            "Base Metal".to_string(),
            64,
            64,
            Vec4::new(0.8, 0.7, 0.2, 1.0),
            0.25,
            1.0,
        );
        user_layer1.fill_anisotropy = 0.85;
        user_layer1.fill_anisotropy_angle = 0.25;

        let mut user_layer2 = Layer::new_fill_layer(
            2,
            "Clear Lacquer".to_string(),
            64,
            64,
            Vec4::ONE,
            0.05,
            0.0,
        );
        user_layer2.fill_clearcoat = 1.0;
        user_layer2.channel_flags.clearcoat = true;

        let layers_to_save = vec![user_layer1, user_layer2];
        let save_res = SmartMaterialLibrary::export_smart_material_to_file(&spmt_path, &layers_to_save);
        assert!(save_res.is_ok(), "Exporting user smart material should succeed");

        let load_res = SmartMaterialLibrary::load_smart_material_from_file(&spmt_path, 64, 64, None);
        assert!(load_res.is_ok(), "Loading user smart material should succeed");
        let loaded = load_res.unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].fill_anisotropy, 0.85);
        assert_eq!(loaded[1].fill_clearcoat, 1.0);
        let _ = std::fs::remove_file(spmt_path);
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
        // Test custom opacity layers with composite blending
        let mut base_layer = Layer::new_fill_layer(
            1,
            "Opaque Background".to_string(),
            32,
            32,
            Vec4::new(0.2, 0.2, 0.2, 1.0),
            0.5,
            0.0,
        );
        base_layer.evaluate_fill(None);

        let mut glass_layer = Layer::new_fill_layer(
            2,
            "Custom Glass Top".to_string(),
            32,
            32,
            Vec4::new(0.18, 0.45, 0.72, 1.0),
            0.06,
            0.0,
        );
        glass_layer.fill_opacity = 0.52;
        glass_layer.channel_flags.opacity = true;
        glass_layer.evaluate_fill(None);

        let layers = vec![base_layer, glass_layer];
        let mut comp = CompositeTextureSet::new(32, 32);
        comp.composite_layers(&layers);

        let op = comp.opacity.get_pixel(16, 16);
        assert!((op - 0.52).abs() < 1e-3, "Composite opacity should match top layer opacity, got {}", op);

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
