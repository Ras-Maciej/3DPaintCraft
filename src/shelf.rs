// 3D Paintcraft - Substance 3D Painter Style Asset Shelf
// Pure safe Rust


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShelfCategory {
    All,
    Materials,
    SmartMaterials,
    SmartMasks,
    Generators,
    Alphas,
    Environments,
}

#[allow(dead_code)]
impl ShelfCategory {
    pub fn name(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Materials => "Materials",
            Self::SmartMaterials => "Smart Materials",
            Self::SmartMasks => "Smart Masks",
            Self::Generators => "Noises",
            Self::Alphas => "Alphas",
            Self::Environments => "Environments (HDRIs)",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ShelfAsset {
    pub id: &'static str,
    pub name: &'static str,
    pub category: ShelfCategory,
    pub preview_color: [u8; 3],
    pub preview_metallic: f32,
    pub preview_roughness: f32,
    pub description: &'static str,
}

pub fn get_shelf_assets() -> Vec<ShelfAsset> {
    vec![
        // --- 1. PBR MATERIALS ---
        ShelfAsset {
            id: "mat_gold",
            name: "Pure Gold",
            category: ShelfCategory::Materials,
            preview_color: [255, 215, 0],
            preview_metallic: 1.0,
            preview_roughness: 0.15,
            description: "Polished 24k gold with smooth mirror reflections",
        },
        ShelfAsset {
            id: "mat_chrome",
            name: "Polished Chrome",
            category: ShelfCategory::Materials,
            preview_color: [230, 230, 235],
            preview_metallic: 1.0,
            preview_roughness: 0.05,
            description: "Ultra-reflective mirror chrome finish",
        },
        ShelfAsset {
            id: "mat_copper",
            name: "Rough Copper",
            category: ShelfCategory::Materials,
            preview_color: [205, 115, 55],
            preview_metallic: 1.0,
            preview_roughness: 0.38,
            description: "Raw untreated copper with satin roughness",
        },
        ShelfAsset {
            id: "mat_aluminium",
            name: "Brushed Aluminium",
            category: ShelfCategory::Materials,
            preview_color: [210, 215, 220],
            preview_metallic: 0.95,
            preview_roughness: 0.28,
            description: "Machined aerospace-grade aluminium",
        },
        ShelfAsset {
            id: "mat_cast_iron",
            name: "Rusted Cast Iron",
            category: ShelfCategory::Materials,
            preview_color: [85, 55, 45],
            preview_metallic: 0.2,
            preview_roughness: 0.85,
            description: "Heavy porous iron with surface oxidation",
        },
        ShelfAsset {
            id: "mat_car_paint_red",
            name: "Car Paint (Red)",
            category: ShelfCategory::Materials,
            preview_color: [220, 25, 25],
            preview_metallic: 0.0,
            preview_roughness: 0.08,
            description: "High-gloss automotive clearcoat enamel",
        },
        ShelfAsset {
            id: "mat_military_olive",
            name: "Military Olive",
            category: ShelfCategory::Materials,
            preview_color: [75, 85, 35],
            preview_metallic: 0.0,
            preview_roughness: 0.90,
            description: "Ultra-matte anti-reflective tactical coating",
        },
        ShelfAsset {
            id: "mat_rubber",
            name: "Dark Rubber",
            category: ShelfCategory::Materials,
            preview_color: [35, 35, 35],
            preview_metallic: 0.0,
            preview_roughness: 0.75,
            description: "Dense vulcanized rubber polymer",
        },
        ShelfAsset {
            id: "mat_white_plastic",
            name: "White Plastic",
            category: ShelfCategory::Materials,
            preview_color: [240, 240, 242],
            preview_metallic: 0.0,
            preview_roughness: 0.22,
            description: "Clean injection-molded ABS plastic",
        },
        ShelfAsset {
            id: "mat_oak_wood",
            name: "Oak Wood",
            category: ShelfCategory::Materials,
            preview_color: [135, 88, 38],
            preview_metallic: 0.0,
            preview_roughness: 0.65,
            description: "Natural hardwood with grain finish",
        },
        ShelfAsset {
            id: "mat_obsidian",
            name: "Obsidian Glass",
            category: ShelfCategory::Materials,
            preview_color: [20, 20, 25],
            preview_metallic: 0.1,
            preview_roughness: 0.04,
            description: "Glossy deep-black volcanic mineral",
        },
        ShelfAsset {
            id: "mat_clay",
            name: "Terracotta Clay",
            category: ShelfCategory::Materials,
            preview_color: [195, 110, 80],
            preview_metallic: 0.0,
            preview_roughness: 0.95,
            description: "Rough unglazed pottery clay",
        },

        // --- 2. SMART MATERIALS ---
        // Built-in smart materials removed to allow pure user-defined smart materials

        // --- 3. SMART MASKS ---
        ShelfAsset {
            id: "mask_edge_wear",
            name: "Edge Wear & Chipping",
            category: ShelfCategory::SmartMasks,
            preview_color: [180, 180, 180],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Realistic convex edge wear with Worley flake breakup, ridged fractures, and crevice gating",
        },
        ShelfAsset {
            id: "mask_crevice_dirt",
            name: "Crevice Dirt & Grime (AO)",
            category: ShelfCategory::SmartMasks,
            preview_color: [100, 100, 100],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Deep AO shadow sediment accumulation with organic domain-warped dirt clumps and rust specks",
        },
        ShelfAsset {
            id: "directional_dust",
            name: "Directional Upward Dust (+Y)",
            category: ShelfCategory::SmartMasks,
            preview_color: [160, 160, 140],
            preview_metallic: 0.0,
            preview_roughness: 0.8,
            description: "Sky-facing dust accumulation driven by World Normal +Y, settling into upward concave pockets",
        },
        ShelfAsset {
            id: "dripping_rust_leaks",
            name: "Dripping Rust Leaks (-Y)",
            category: ShelfCategory::SmartMasks,
            preview_color: [140, 70, 40],
            preview_metallic: 0.0,
            preview_roughness: 0.7,
            description: "Gravity-directed fluid runoffs and vertical chemical rust flow streaks originating from cavities",
        },
        ShelfAsset {
            id: "mask_grunge",
            name: "Surface Scratches",
            category: ShelfCategory::SmartMasks,
            preview_color: [140, 140, 140],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Multi-frequency directional scratches modulated by high-contact wear clusters",
        },
        ShelfAsset {
            id: "peeling_paint",
            name: "Peeling & Flaking Paint",
            category: ShelfCategory::SmartMasks,
            preview_color: [200, 180, 150],
            preview_metallic: 0.0,
            preview_roughness: 0.6,
            description: "Organic cellular blisters with sharp jagged chipped paint perimeter lines",
        },
        ShelfAsset {
            id: "mask_ground_dirt",
            name: "Ground Mud & Splash",
            category: ShelfCategory::SmartMasks,
            preview_color: [95, 75, 55],
            preview_metallic: 0.0,
            preview_roughness: 0.9,
            description: "Bottom height gradient with dense mud droplets and turbulent ground contact splash",
        },
        ShelfAsset {
            id: "mask_heavy_machinery",
            name: "Heavy Machinery Weathering",
            category: ShelfCategory::SmartMasks,
            preview_color: [130, 120, 110],
            preview_metallic: 0.0,
            preview_roughness: 0.7,
            description: "Multi-layered compound wear: chipped edges + cavity grease + abrasive scuffs + grunge breakup",
        },
        ShelfAsset {
            id: "mask_gun_metal",
            name: "Gun Metal Precision Edge",
            category: ShelfCategory::SmartMasks,
            preview_color: [175, 175, 185],
            preview_metallic: 0.0,
            preview_roughness: 0.4,
            description: "Fine razor-sharp micro edge highlights and firearm slide contact wear",
        },
        ShelfAsset {
            id: "mask_patina",
            name: "Verdigris & Oxidation Bloom",
            category: ShelfCategory::SmartMasks,
            preview_color: [45, 150, 130],
            preview_metallic: 0.0,
            preview_roughness: 0.8,
            description: "Antique chemical oxidation blooming organically outward from deep crevices",
        },
        ShelfAsset {
            id: "mask_invert",
            name: "Invert Current Mask",
            category: ShelfCategory::SmartMasks,
            preview_color: [220, 220, 220],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Invert grayscale mask values on the selected layer",
        },
        ShelfAsset {
            id: "mask_perforated_mesh",
            name: "Perforated Hex Holes",
            category: ShelfCategory::SmartMasks,
            preview_color: [140, 150, 160],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Industrial sheet metal hexagonal punch hole cutout mask",
        },
        ShelfAsset {
            id: "mask_wire_diamond",
            name: "Diamond Wire Fence Mesh",
            category: ShelfCategory::SmartMasks,
            preview_color: [160, 170, 180],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "45-degree diamond crisscross wire fence mesh cutout",
        },
        ShelfAsset {
            id: "mask_torn_edge",
            name: "Torn Fabric Ragged Cutout",
            category: ShelfCategory::SmartMasks,
            preview_color: [170, 160, 140],
            preview_metallic: 0.0,
            preview_roughness: 0.7,
            description: "Organic ragged cloth tear with fine boundary fiber fringe",
        },

        // --- 4. ALPHAS ---
        ShelfAsset {
            id: "alpha_soft_round",
            name: "Soft Round",
            category: ShelfCategory::Alphas,
            preview_color: [240, 240, 240],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Smooth gaussian falloff brush tip",
        },
        ShelfAsset {
            id: "alpha_hard_round",
            name: "Hard Round",
            category: ShelfCategory::Alphas,
            preview_color: [255, 255, 255],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Sharp crisp circular brush tip",
        },
        ShelfAsset {
            id: "alpha_noise_splatter",
            name: "Noise Splatter",
            category: ShelfCategory::Alphas,
            preview_color: [170, 170, 170],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Organic dirt and liquid splatter droplets",
        },
        ShelfAsset {
            id: "alpha_chisel",
            name: "Chisel / Calligraphy",
            category: ShelfCategory::Alphas,
            preview_color: [200, 200, 200],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Directional flat chisel stroke",
        },
        ShelfAsset {
            id: "alpha_stipple",
            name: "Stipple Spray",
            category: ShelfCategory::Alphas,
            preview_color: [160, 160, 160],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Fine airbrush stipple grain",
        },
        ShelfAsset {
            id: "alpha_scratches",
            name: "Micro Scratches",
            category: ShelfCategory::Alphas,
            preview_color: [180, 180, 180],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Fine directional hairline scratches",
        },

        // --- 5. PROCEDURAL NOISES & GENERATORS ---
        ShelfAsset {
            id: "gen_perlin",
            name: "3D Perlin Noise",
            category: ShelfCategory::Generators,
            preview_color: [130, 130, 130],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Classic continuous organic 3D gradient noise",
        },
        ShelfAsset {
            id: "gen_worley",
            name: "3D Worley Noise",
            category: ShelfCategory::Generators,
            preview_color: [110, 110, 110],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Cellular Voronoi noise with crystalline structure",
        },
        ShelfAsset {
            id: "gen_billow",
            name: "Billow Noise",
            category: ShelfCategory::Generators,
            preview_color: [135, 140, 150],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Soft rolling puffy cloud and smoke volume shapes",
        },
        ShelfAsset {
            id: "gen_ridged",
            name: "Ridged Multifractal",
            category: ShelfCategory::Generators,
            preview_color: [160, 155, 150],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Sharp jagged fissures, stone cracks, and chiseled ridges",
        },
        ShelfAsset {
            id: "gen_domain_warp",
            name: "Domain Warped Grunge",
            category: ShelfCategory::Generators,
            preview_color: [120, 125, 130],
            preview_metallic: 0.0,
            preview_roughness: 0.6,
            description: "Inigo Quilez turbulent domain warping for organic liquid weathering",
        },
        ShelfAsset {
            id: "gen_splatter",
            name: "Splatter & Rust Spots",
            category: ShelfCategory::Generators,
            preview_color: [140, 90, 60],
            preview_metallic: 0.0,
            preview_roughness: 0.8,
            description: "Organic micro splatter specks, rust pitting, and liquid droplets",
        },
        ShelfAsset {
            id: "gen_wood",
            name: "Wood Grain & Fibers",
            category: ShelfCategory::Generators,
            preview_color: [130, 90, 50],
            preview_metallic: 0.0,
            preview_roughness: 0.65,
            description: "Annual growth rings and longitudinal anisotropic wood fibers",
        },
        ShelfAsset {
            id: "gen_fabric",
            name: "Fabric & Carbon Weave",
            category: ShelfCategory::Generators,
            preview_color: [50, 50, 55],
            preview_metallic: 0.1,
            preview_roughness: 0.7,
            description: "Interlaced textile warp and weft woven pattern",
        },
        ShelfAsset {
            id: "gen_scratches",
            name: "Surface Scratches",
            category: ShelfCategory::Generators,
            preview_color: [140, 140, 140],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Directional abrasion lines and multi-frequency hairline cuts",
        },
        ShelfAsset {
            id: "gen_curvature",
            name: "Edge Wear Generator",
            category: ShelfCategory::Generators,
            preview_color: [160, 160, 160],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Geometric edge chipping and wear driven by baked curvature map",
        },
        ShelfAsset {
            id: "gen_dust",
            name: "Directional Dust",
            category: ShelfCategory::Generators,
            preview_color: [120, 120, 110],
            preview_metallic: 0.0,
            preview_roughness: 0.7,
            description: "Top-facing surface grime and environmental gravity falloff",
        },
        ShelfAsset {
            id: "gen_cavity",
            name: "AO & Cavity Dirt",
            category: ShelfCategory::Generators,
            preview_color: [90, 85, 80],
            preview_metallic: 0.0,
            preview_roughness: 0.85,
            description: "Crevice, cavity, and seam dirt driven by Ambient Occlusion and Curvature",
        },
        ShelfAsset {
            id: "gen_leaks",
            name: "Dripping Leaks",
            category: ShelfCategory::Generators,
            preview_color: [70, 65, 60],
            preview_metallic: 0.0,
            preview_roughness: 0.7,
            description: "Vertical liquid dripping streaks, water runoffs, and rust stains",
        },
        ShelfAsset {
            id: "gen_position",
            name: "Position Gradient",
            category: ShelfCategory::Generators,
            preview_color: [100, 120, 140],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Linear 3D world-position vertical and depth gradient",
        },
        ShelfAsset {
            id: "gen_checker",
            name: "Checkerboard",
            category: ShelfCategory::Generators,
            preview_color: [150, 150, 150],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Procedural geometric alternating grid checkerboard",
        },
        ShelfAsset {
            id: "gen_bricks",
            name: "Bricks & Tiles",
            category: ShelfCategory::Generators,
            preview_color: [160, 110, 90],
            preview_metallic: 0.0,
            preview_roughness: 0.8,
            description: "Modular procedural masonry bond with mortar joints",
        },
        ShelfAsset {
            id: "gen_perforated",
            name: "Perforated Mesh (Hex)",
            category: ShelfCategory::Generators,
            preview_color: [130, 140, 150],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Hexagonal punched circular perforation hole grid",
        },
        ShelfAsset {
            id: "gen_wire_fence",
            name: "Wire Fence Mesh",
            category: ShelfCategory::Generators,
            preview_color: [150, 160, 170],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "45-degree diamond crisscross wire fence mesh",
        },
        ShelfAsset {
            id: "gen_torn_fabric",
            name: "Torn Fabric Cutout",
            category: ShelfCategory::Generators,
            preview_color: [160, 150, 130],
            preview_metallic: 0.0,
            preview_roughness: 0.6,
            description: "Ragged cloth tear with organic frayed boundary fibers",
        },
        ShelfAsset {
            id: "gen_dots",
            name: "Circular Dots / Holes",
            category: ShelfCategory::Generators,
            preview_color: [140, 140, 140],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Regular aligned grid of circular dot perforations",
        },
        ShelfAsset {
            id: "gen_bnw_spots_1",
            name: "BnW Spots 1 (Substance)",
            category: ShelfCategory::Generators,
            preview_color: [125, 125, 130],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Multi-scale Voronoi cellular spots with smooth luminance gradient falloff",
        },
        ShelfAsset {
            id: "gen_bnw_spots_2",
            name: "BnW Spots 2 (Substance)",
            category: ShelfCategory::Generators,
            preview_color: [140, 135, 130],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Irregular clustered cellular spots with high contrast and sharp flake borders",
        },
        ShelfAsset {
            id: "gen_bnw_spots_3",
            name: "BnW Spots 3 (Substance)",
            category: ShelfCategory::Generators,
            preview_color: [150, 150, 150],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "High-density granular pore noise and micro-dots distribution",
        },
        ShelfAsset {
            id: "gen_grunge_map",
            name: "Grunge Map (Substance)",
            category: ShelfCategory::Generators,
            preview_color: [110, 105, 100],
            preview_metallic: 0.0,
            preview_roughness: 0.65,
            description: "Substance Grunge 004/005 style domain-warped dirt grit and cavity smudges",
        },
        ShelfAsset {
            id: "gen_directional_noise",
            name: "Directional Noise / Brushed",
            category: ShelfCategory::Generators,
            preview_color: [130, 130, 135],
            preview_metallic: 0.0,
            preview_roughness: 0.5,
            description: "Stretched anisotropic streaks and fibers for brushed metal and directional textures",
        },
        ShelfAsset {
            id: "gen_clouds",
            name: "Fractal Clouds (Substance)",
            category: ShelfCategory::Generators,
            preview_color: [160, 170, 180],
            preview_metallic: 0.0,
            preview_roughness: 0.45,
            description: "Substance Fractal Clouds atmospheric billowy turbulence noise",
        },
        ShelfAsset {
            id: "gen_marble_veins",
            name: "Marble Veins / Strata",
            category: ShelfCategory::Generators,
            preview_color: [180, 180, 185],
            preview_metallic: 0.0,
            preview_roughness: 0.35,
            description: "Domain-warped ridged multifractal veins with sharp dendritic cracks and strata",
        },
        ShelfAsset {
            id: "gen_moisture",
            name: "Moisture / Condensation",
            category: ShelfCategory::Generators,
            preview_color: [100, 130, 160],
            preview_metallic: 0.0,
            preview_roughness: 0.25,
            description: "Fluid condensation stains with circular meniscus rims and water droplets",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shelf_assets_categories_and_generators() {
        let assets = get_shelf_assets();
        assert!(!assets.is_empty(), "Shelf asset library should not be empty");

        let has_materials = assets.iter().any(|a| a.category == ShelfCategory::Materials);
        let has_smart_masks = assets.iter().any(|a| a.category == ShelfCategory::SmartMasks);
        let has_generators = assets.iter().any(|a| a.category == ShelfCategory::Generators);
        let has_alphas = assets.iter().any(|a| a.category == ShelfCategory::Alphas);

        assert!(has_materials, "Should have material assets");
        assert!(has_smart_masks, "Should have smart mask assets");
        assert!(has_generators, "Should have procedural generator noise assets");
        assert!(has_alphas, "Should have brush alpha assets");

        // Verify specific key procedural noise generators are present
        let generator_ids: Vec<&str> = assets
            .iter()
            .filter(|a| a.category == ShelfCategory::Generators)
            .map(|a| a.id)
            .collect();

        assert!(generator_ids.contains(&"gen_perlin"));
        assert!(generator_ids.contains(&"gen_worley"));
        assert!(generator_ids.contains(&"gen_billow"));
        assert!(generator_ids.contains(&"gen_ridged"));
        assert!(generator_ids.contains(&"gen_domain_warp"));
        assert!(generator_ids.contains(&"gen_splatter"));
        assert!(generator_ids.contains(&"gen_wood"));
        assert!(generator_ids.contains(&"gen_fabric"));
        assert!(generator_ids.contains(&"gen_scratches"));
        assert!(generator_ids.contains(&"gen_curvature"));
        assert!(generator_ids.contains(&"gen_cavity"));
        assert!(generator_ids.contains(&"gen_dust"));
        assert!(generator_ids.contains(&"gen_leaks"));
        assert!(generator_ids.contains(&"gen_position"));
        assert!(generator_ids.contains(&"gen_checker"));
        assert!(generator_ids.contains(&"gen_bricks"));
        assert!(generator_ids.contains(&"gen_bnw_spots_1"));
        assert!(generator_ids.contains(&"gen_bnw_spots_2"));
        assert!(generator_ids.contains(&"gen_bnw_spots_3"));
        assert!(generator_ids.contains(&"gen_grunge_map"));
        assert!(generator_ids.contains(&"gen_directional_noise"));
        assert!(generator_ids.contains(&"gen_clouds"));
        assert!(generator_ids.contains(&"gen_marble_veins"));
        assert!(generator_ids.contains(&"gen_moisture"));
    }
}

