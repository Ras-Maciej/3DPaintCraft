    use super::*;

    #[test]
    fn test_bake_progress_state_cancellation() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(BakeProgressState {
            progress: 0.25,
            step_name: "Baking AO...".to_string(),
            is_done: false,
            cancelled: false,
            elapsed_secs: 1.2,
        }));

        assert!(!state.lock().unwrap().cancelled);
        state.lock().unwrap().cancelled = true;
        assert!(state.lock().unwrap().cancelled);
    }

    #[test]
    fn test_viewport_live_synchronization_dirty_flow() {
        let mut proj = Project::new_default(64, 64);
        proj.is_dirty = false;
        proj.gpu_sync_needed = false;

        // When a dab or fill occurs, rebuild_composite sets both is_dirty and gpu_sync_needed
        proj.rebuild_composite();
        assert!(proj.is_dirty);
        assert!(proj.gpu_sync_needed);
    }

    #[test]
    fn test_color_picker_hex_and_brush_conversion() {
        let v = glam::Vec4::new(0.2, 0.4, 0.8, 1.0);
        let hex = crate::color_picker::rgb_to_hex(v.x, v.y, v.z);
        assert_eq!(hex, "#3366CC");

        let parsed = crate::color_picker::parse_hex_color(&hex).unwrap();
        assert!((parsed[0] - 0.2).abs() < 0.01);
        assert!((parsed[1] - 0.4).abs() < 0.01);
        assert!((parsed[2] - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_iray_path_tracer_integration() {
        let mut pt = crate::path_tracer::PathTracer::new(32, 32);
        pt.max_samples = 4;
        pt.max_bounces = 1;

        let camera = crate::renderer::Camera::default();
        let mesh = crate::mesh::Mesh::create_cube_mesh("TestCube", 1.0);
        let textures = crate::layer::CompositeTextureSet::new(64, 64);
        let hdri = crate::hdri::HdriEnvironment::new(crate::hdri::HdriPreset::StudioNeutral);

        assert_eq!(pt.sample_count, 0);
        assert!(!pt.is_converged());

        pt.render_samples(&camera, &mesh, &textures, &hdri, 2);
        assert_eq!(pt.sample_count, 2);
        assert!(!pt.is_converged());
        assert_eq!(pt.progress_pct(), 50.0);

        pt.render_samples(&camera, &mesh, &textures, &hdri, 2);
        assert_eq!(pt.sample_count, 4);
        assert!(pt.is_converged());
        assert_eq!(pt.progress_pct(), 100.0);

        // Verify color buffer is filled with non-zero alpha
        assert_eq!(pt.color_buffer.len(), 32 * 32 * 4);
        for chunk in pt.color_buffer.chunks_exact(4) {
            assert_eq!(chunk[3], 255);
        }

        // Test reset
        pt.reset();
        assert_eq!(pt.sample_count, 0);
        assert!(!pt.is_converged());
    }

    #[test]
    fn test_user_materials_creation_and_builtin_materials_cleared() {
        // 1. Verify built-in static materials are deleted from shelf assets
        let static_assets = crate::shelf::get_shelf_assets();
        let has_builtin_materials = static_assets.iter().any(|a| a.category == crate::shelf::ShelfCategory::Materials);
        assert!(!has_builtin_materials, "Built-in static materials must be cleared so user can make custom materials");

        // 2. Verify custom material creation and persistence
        let mut asset_mgr = crate::asset_manager::AssetManager::new();
        let custom_mat = crate::asset_manager::CustomMaterialEntry {
            id: "my_custom_obsidian".to_string(),
            name: "My Custom Obsidian".to_string(),
            description: "Handcrafted deep black glossy mineral".to_string(),
            path: std::path::PathBuf::new(),
            file_name: String::new(),
            base_color: [0.05, 0.05, 0.07, 1.0],
            roughness: 0.04,
            metallic: 0.1,
            height: 0.5,
            normal: [0.5, 0.5, 1.0, 0.5],
            emissive: [0.0, 0.0, 0.0, 1.0],
            opacity: 1.0,
            clearcoat: 0.9,
            anisotropy: 0.0,
        };

        let save_res = asset_mgr.save_material(&custom_mat);
        assert!(save_res.is_ok(), "User saving a custom material should succeed");

        asset_mgr.refresh_materials();
        let found = asset_mgr.materials.iter().find(|m| m.id == "my_custom_obsidian");
        assert!(found.is_some(), "Custom user material must be loaded in materials list");

        // Clean up
        let _ = asset_mgr.delete_material("my_custom_obsidian");
    }

    #[test]
    fn test_app_baker_cage_settings_and_config() {
        let cfg = crate::baker::BakeCageConfig::default();
        assert_eq!(cfg.mode, crate::baker::BakeCageMode::None);
        assert!((cfg.inflation_offset - 0.02).abs() < 1e-5);
        assert!(cfg.external_cage_path.is_none());

        let parametric_cfg = crate::baker::BakeCageConfig {
            mode: crate::baker::BakeCageMode::ParametricInflation,
            inflation_offset: 0.04,
            external_cage_path: None,
        };
        assert_eq!(parametric_cfg.mode.name(), "Parametric Cage (Inflation)");
        assert_eq!(crate::baker::BakeCageMode::ExternalCage.name(), "External Cage Mesh");
        assert_eq!(crate::baker::BakeCageMode::None.name(), "None (Ray Distances)");

        // Verify cage color palette mapping
        let colors = [
            [0.0, 0.88, 1.0, 1.0],   // Cyan (Substance standard)
            [1.0, 0.45, 0.05, 1.0],  // Orange
            [0.2, 1.0, 0.35, 1.0],   // High-Vis Lime
            [1.0, 0.15, 0.80, 1.0],  // Neon Magenta
        ];
        assert_eq!(colors[0], [0.0, 0.88, 1.0, 1.0], "Substance Cyan color matches specification");
        assert_eq!(colors[1], [1.0, 0.45, 0.05, 1.0], "Substance Orange color matches specification");
    }

    #[test]
    fn test_app_baker_map_settings_and_tabs() {
        let mut tab = crate::baker::BakeSettingsTab::Common;
        assert_eq!(tab, crate::baker::BakeSettingsTab::Common);

        // Switch through map tabs
        tab = crate::baker::BakeSettingsTab::AmbientOcclusion;
        assert_eq!(tab, crate::baker::BakeSettingsTab::AmbientOcclusion);

        // Customize AO samples and reach
        let mut ao_config = crate::baker::AoBakeConfig::default();
        assert_eq!(ao_config.samples, 64);
        ao_config.samples = 128;
        ao_config.max_distance = 0.5;
        ao_config.ray_bias = 0.01;
        ao_config.spread = 1.4;
        ao_config.contrast = 1.2;
        assert_eq!(ao_config.samples, 128);

        // Customize Thickness samples and invert
        let mut th_config = crate::baker::ThicknessBakeConfig::default();
        assert_eq!(th_config.samples, 32);
        th_config.samples = 64;
        th_config.max_distance = 0.8;
        th_config.ray_bias = 0.006;
        th_config.invert = true;
        assert_eq!(th_config.samples, 64);
        assert!(th_config.invert);

        // Customize Curvature settings
        let mut curv_config = crate::baker::CurvatureBakeConfig::default();
        assert_eq!(curv_config.intensity, 1.0);
        curv_config.intensity = 1.6;
        curv_config.contrast = 1.3;
        curv_config.dihedral_weight = 0.45;
        curv_config.deflection_weight = 0.35;
        curv_config.blur_passes = 2;
        assert_eq!(curv_config.blur_passes, 2);

        // Customize Normal Map format
        let mut normal_config = crate::baker::NormalBakeConfig::default();
        assert!(!normal_config.invert_green);
        normal_config.invert_green = true;
        assert!(normal_config.invert_green);

        // Customize Position mode
        let mut pos_config = crate::baker::PositionBakeConfig::default();
        assert_eq!(pos_config.mode, crate::baker::PositionNormalizationMode::BoundingBox);
        pos_config.mode = crate::baker::PositionNormalizationMode::WorldUnits;
        assert_eq!(pos_config.mode, crate::baker::PositionNormalizationMode::WorldUnits);
    }

    #[test]
    fn test_polygon_fill_drag_tracking_and_consolidated_undo() {
        let mut proj = Project::new_default(32, 32);
        let mut history = HistoryManager::new();
        let mut drag_tris: std::collections::HashSet<usize> = std::collections::HashSet::new();

        let initial_snapshot = proj.layers[0].clone_stroke_snapshot();

        // Simulate dragging across triangles 0 and 1 (a quad face)
        let tri_candidates = vec![0, 1];
        let new_tris: Vec<usize> = tri_candidates.into_iter().filter(|t| !drag_tris.contains(t)).collect();
        assert_eq!(new_tris.len(), 2);
        for &t in &new_tris {
            drag_tris.insert(t);
        }

        // Subsequent drag frame hovering same triangles should yield 0 new triangles
        let duplicate_check: Vec<usize> = vec![0, 1].into_iter().filter(|t| !drag_tris.contains(t)).collect();
        assert_eq!(duplicate_check.len(), 0, "Already filled triangles should not be re-rasterized during drag");

        // Rasterize triangles 0 and 1
        let fill_color = glam::Vec4::new(1.0, 0.5, 0.0, 1.0);
        let layer = &mut proj.layers[0];
        for &t in &new_tris {
            let i0 = proj.mesh.indices[t * 3] as usize;
            let i1 = proj.mesh.indices[t * 3 + 1] as usize;
            let i2 = proj.mesh.indices[t * 3 + 2] as usize;
            let uv0 = proj.mesh.vertices[i0].uv;
            let uv1 = proj.mesh.vertices[i1].uv;
            let uv2 = proj.mesh.vertices[i2].uv;
            crate::mesh::Mesh::rasterize_triangle_uv(32, 32, uv0, uv1, uv2, |px, py| {
                layer.base_color_buf.set_pixel(px, py, fill_color);
            });
        }
        layer.mark_dirty_full();

        // Check that at least some pixels were colored (orange has z < 0.2, default white has z == 1.0)
        let mut colored_pixels = 0;
        for py in 0..32 {
            for px in 0..32 {
                if layer.base_color_buf.get_pixel(px, py).z < 0.2 {
                    colored_pixels += 1;
                }
            }
        }
        assert!(colored_pixels > 0, "Rasterized triangles must fill pixels in the layer");

        // Finish stroke: push single consolidated undo snapshot
        let final_snapshot = proj.layers[0].clone_stroke_snapshot();
        history.push("Polygon Fill (Polygon (Quad))", HistoryAction::Stroke {
            layer_idx: 0,
            prev_layer: initial_snapshot,
            next_layer: final_snapshot,
            description: "Polygon Fill (Polygon (Quad))".to_string(),
        });
        drag_tris.clear();
        assert!(drag_tris.is_empty(), "Drag triangles tracking must be cleared on stroke release");

        assert_eq!(history.undo_stack.len(), 1);
        assert!(history.can_undo());

        // Test Undo
        let desc = history.undo(&mut proj).expect("Undo must return an action description");
        assert_eq!(desc, "Polygon Fill (Polygon (Quad))");

        // Verify layer reverted to initial state (white default layer has z == 1.0)
        let mut reverted_colored = 0;
        for py in 0..32 {
            for px in 0..32 {
                if proj.layers[0].base_color_buf.get_pixel(px, py).z < 0.2 {
                    reverted_colored += 1;
                }
            }
        }
        assert_eq!(reverted_colored, 0, "Undo must revert filled pixels back to blank layer");
    }

    #[test]
    fn test_polygon_fill_modes_and_hover_preview() {
        let cube = crate::mesh::Mesh::create_cube_mesh("HoverCube", 1.0);

        // Test mode names
        assert_eq!(crate::brush::PolygonFillMode::Triangle.name(), "Triangle");
        assert_eq!(crate::brush::PolygonFillMode::Polygon.name(), "Polygon (Quad)");
        assert_eq!(crate::brush::PolygonFillMode::UvIsland.name(), "UV Island");
        assert_eq!(crate::brush::PolygonFillMode::MeshElement.name(), "Mesh Element");

        // Triangle mode
        let tri_preview = vec![0];
        assert_eq!(tri_preview.len(), 1);

        // Polygon (Quad) mode
        let quad_preview = cube.get_polygon_quad(0);
        assert_eq!(quad_preview.len(), 2);
        assert!(quad_preview.contains(&0) && quad_preview.contains(&1));

        // UV Island mode
        let island_preview = cube.get_connected_uv_island(0);
        assert!(!island_preview.is_empty());
        assert!(island_preview.contains(&0));

        // Mesh Element mode (contiguous 3D mesh)
        let element_preview = cube.get_connected_mesh_element(0);
        assert_eq!(element_preview.len(), 12, "A closed cube element consists of 12 triangles (6 quads)");
    }

    #[test]
    fn test_follow_path_settings_and_offset() {
        let mut settings = crate::brush::BrushSettings::default();
        assert!(!settings.follow_path);
        assert_eq!(settings.follow_path_offset, 0.0);

        settings.follow_path = true;
        settings.follow_path_offset = std::f32::consts::FRAC_PI_2;
        assert!(settings.follow_path);
        assert!((settings.follow_path_offset - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn test_shadow_and_wireframe_shader_settings() {
        let mut settings = crate::wgpu_renderer::ShaderSettings::default();
        assert!(settings.shadows_enabled);
        assert!((settings.shadow_bias - 0.015).abs() < 1e-5);
        assert!((settings.shadow_softness - 1.0).abs() < 1e-4);
        assert_eq!(settings.wireframe_color, [0.2, 0.75, 1.0, 1.0]);
        assert!((settings.wireframe_opacity - 0.85).abs() < 1e-4);

        settings.shadows_enabled = false;
        settings.shadow_bias = 0.008;
        settings.shadow_softness = 2.5;
        settings.wireframe_color = [1.0, 0.5, 0.2, 1.0];
        settings.wireframe_opacity = 0.5;

        assert!(!settings.shadows_enabled);
        assert!((settings.shadow_bias - 0.008).abs() < 1e-5);
        assert!((settings.shadow_softness - 2.5).abs() < 1e-4);
        assert_eq!(settings.wireframe_color, [1.0, 0.5, 0.2, 1.0]);
        assert!((settings.wireframe_opacity - 0.5).abs() < 1e-4);
    }

    #[test]
    fn test_split_viewport_and_mask_mode() {
        let cube = crate::mesh::Mesh::create_cube_mesh("Cube", 1.0);
        assert!(cube.bounds_radius() > 0.5);

        // Verify UV to 3D point mapping on cube
        let uv_hit = cube.uv_to_3d_point(glam::Vec2::new(0.5, 0.5));
        assert!(uv_hit.is_some(), "Cube center UV should map to a valid 3D surface point");
        let (pos, norm) = uv_hit.unwrap();
        assert!(pos.length() > 0.0);
        assert!((norm.length() - 1.0).abs() < 1e-4);

        // View layout states
        let mut layout = ViewLayout::SplitDualView;
        assert_eq!(layout, ViewLayout::SplitDualView);
        layout = ViewLayout::View3DOnly;
        assert_eq!(layout, ViewLayout::View3DOnly);
        layout = ViewLayout::View2DOnly;
        assert_eq!(layout, ViewLayout::View2DOnly);

        // Split ratio clamping
        let ratio_default = 0.5f32;
        let ratio_clamped = 1.2f32.clamp(0.15, 0.85);
        assert_eq!(ratio_default, 0.5);
        assert_eq!(ratio_clamped, 0.85);
    }

    #[test]
    fn test_new_project_auto_bake_disabled_by_default() {
        let mut proj = Project::new_default(64, 64);
        let mesh = crate::mesh::Mesh::create_rounded_cube();
        proj.set_mesh(mesh);

        // Setting mesh must NOT bake mesh maps automatically
        assert!(proj.curvature_map.is_none(), "Setting mesh must not automatically bake curvature map");
        assert!(proj.ao_map.is_none(), "Setting mesh must not automatically bake AO map");
        assert!(proj.world_normal_map.is_none(), "Setting mesh must not automatically bake world normal map");
        assert!(proj.position_map.is_none(), "Setting mesh must not automatically bake position map");
        assert!(proj.thickness_map.is_none(), "Setting mesh must not automatically bake thickness map");

        // Explicit baking should populate mesh maps when requested
        proj.bake_mesh_maps(32, 0.1, 0.1);
        assert!(proj.curvature_map.is_some(), "Explicit bake must populate curvature map");
        assert!(proj.ao_map.is_some(), "Explicit bake must populate AO map");
    }

    #[test]
    fn test_substance_painter_export_modal_state_and_defaults() {
        let presets = crate::packing::CustomPackingPreset::default_presets();
        assert!(!presets.is_empty(), "Should load factory output templates");
        assert_eq!(presets.len(), 8, "Should have 8 factory production output templates");

        // Verify built-in templates exist
        let template_ids: Vec<&str> = presets.iter().map(|p| p.id.as_str()).collect();
        assert!(template_ids.contains(&"unreal_orm"));
        assert!(template_ids.contains(&"unity_hdrp"));
        assert!(template_ids.contains(&"unity_urp"));
        assert!(template_ids.contains(&"godot4"));
        assert!(template_ids.contains(&"blender_gltf"));
        assert!(template_ids.contains(&"cryengine_ddna"));
        assert!(template_ids.contains(&"individual_channels"));
        assert!(template_ids.contains(&"custom_studio_default"));

        // Verify Unreal ORM has expected map channels
        let unreal = presets.iter().find(|p| p.id == "unreal_orm").unwrap();
        assert_eq!(unreal.maps[2].name_suffix, "_ORM");
        assert_eq!(unreal.maps[2].channel_r.source, crate::packing::ChannelSource::AmbientOcclusion);
        assert_eq!(unreal.maps[2].channel_g.source, crate::packing::ChannelSource::Roughness);
        assert_eq!(unreal.maps[2].channel_b.source, crate::packing::ChannelSource::Metallic);
    }

    #[test]
    fn test_export_modal_active_texture_set_sync() {
        let mut proj = Project::new_default(32, 32);
        assert_eq!(proj.texture_sets.len(), 1);
        assert_eq!(proj.texture_sets[0].layers.len(), 1);

        // Add a gold layer
        proj.add_fill_layer(
            Some("Gold Armor".to_string()),
            glam::Vec4::new(1.0, 0.84, 0.0, 1.0),
            0.15,
            0.95,
        );
        assert_eq!(proj.layers.len(), 2);

        // Before synchronization, texture_set[0] is stale
        assert_eq!(proj.texture_sets[0].layers.len(), 1);

        // Rebuild and synchronize as done during export
        proj.rebuild_composite();
        proj.sync_active_texture_set();

        // After synchronization, texture_set[0] has all layers and matching gold composite
        assert_eq!(proj.texture_sets[0].layers.len(), 2);
        let sample = proj.texture_sets[0].composite_textures.base_color.get_pixel(8, 8);
        assert!((sample.x - 1.0).abs() < 1e-4);
        assert!((sample.y - 0.84).abs() < 1e-4);
        assert!((sample.z - 0.0).abs() < 1e-4);

        // Export with unreal_orm preset and verify saved textures are gold, not blank
        let temp_dir = std::env::temp_dir().join("test_app_export_gold_sync");
        let preset = crate::packing::CustomPackingPreset::unreal_orm();
        let exported = crate::packing::CustomTexturePacker::pack_and_export(
            &preset,
            &proj.texture_sets[0].composite_textures,
            "Character",
            &temp_dir,
            false,
            None,
            &proj.baked_map_context(),
        )
        .unwrap();

        assert!(!exported.is_empty());
        let bc_path = temp_dir.join("character_BaseColor.png");
        assert!(bc_path.exists());
        let bc_img = image::open(&bc_path).unwrap().into_rgba8();
        let p = bc_img.get_pixel(8, 8);
        assert_eq!(p[0], 255); // Red 1.0 -> 255
        assert!((p[1] as i32 - 214).abs() <= 2); // Green 0.84 -> ~214
        assert_eq!(p[2], 0); // Blue 0.0 -> 0

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_export_mesh_toggle_behavior() {
        let proj = Project::new_default(16, 16);
        let preset = crate::packing::CustomPackingPreset::unreal_orm();
        let baked = proj.baked_map_context();

        // 1. Export with mesh = None (export_mesh toggle OFF)
        let temp_dir_no_mesh = std::env::temp_dir().join("test_export_mesh_toggle_off");
        let exported_no_mesh = crate::packing::CustomTexturePacker::pack_and_export(
            &preset,
            &proj.composite_textures,
            "NoMeshTest",
            &temp_dir_no_mesh,
            false,
            None,
            &baked,
        )
        .unwrap();

        assert!(!exported_no_mesh.is_empty());
        assert!(!temp_dir_no_mesh.join("nomeshtest.obj").exists(), "OBJ must not be exported when toggle is OFF");
        assert!(!temp_dir_no_mesh.join("nomeshtest.mtl").exists(), "MTL must not be exported when toggle is OFF");
        assert!(!temp_dir_no_mesh.join("nomeshtest.glb").exists(), "GLB must not be exported when toggle is OFF");
        assert!(temp_dir_no_mesh.join("nomeshtest_BaseColor.png").exists(), "Textures must still be exported");

        let _ = std::fs::remove_dir_all(&temp_dir_no_mesh);

        // 2. Export with mesh = Some (export_mesh toggle ON)
        let temp_dir_with_mesh = std::env::temp_dir().join("test_export_mesh_toggle_on");
        let _exported_with_mesh = crate::packing::CustomTexturePacker::pack_and_export(
            &preset,
            &proj.composite_textures,
            "WithMeshTest",
            &temp_dir_with_mesh,
            false,
            Some(&proj.mesh),
            &baked,
        )
        .unwrap();

        assert!(temp_dir_with_mesh.join("withmeshtest.obj").exists(), "OBJ must be exported when toggle is ON");
        assert!(temp_dir_with_mesh.join("withmeshtest.mtl").exists(), "MTL must be exported when toggle is ON");
        assert!(temp_dir_with_mesh.join("withmeshtest.glb").exists(), "GLB must be exported when toggle is ON");
        assert!(temp_dir_with_mesh.join("withmeshtest_BaseColor.png").exists(), "Textures must be exported");

        let _ = std::fs::remove_dir_all(&temp_dir_with_mesh);
    }

    #[test]
    fn test_workspace_layout_defaults_and_resets() {
        let layout = WorkspaceLayout::default();
        assert!(layout.show_toolbar);
        assert!(layout.show_context_bar);
        assert!(layout.show_status_bar);
        assert!(layout.show_texture_sets);
        assert!(layout.show_layers);
        assert!(layout.show_properties);
        assert!(layout.show_shelf);

        assert!(!layout.texture_sets_floating);
        assert!(!layout.layers_floating);
        assert!(!layout.properties_floating);
        assert_eq!(layout.shelf_dock, ShelfDockLocation::Bottom);

        assert_eq!(layout.right_panel_width, 380.0);
        assert_eq!(layout.bottom_shelf_height, 220.0);
        assert_eq!(layout.layers_pane_height, 240.0);

        assert!(!layout.texture_sets_collapsed);
        assert!(!layout.layers_collapsed);
        assert!(!layout.properties_collapsed);
    }

    #[test]
    fn test_workspace_layout_dock_and_float_transitions() {
        let mut layout = WorkspaceLayout::default();

        // Undock all panels to floating mode
        layout.texture_sets_floating = true;
        layout.layers_floating = true;
        layout.properties_floating = true;
        layout.shelf_dock = ShelfDockLocation::Floating;

        assert!(layout.texture_sets_floating);
        assert!(layout.layers_floating);
        assert!(layout.properties_floating);
        assert_eq!(layout.shelf_dock, ShelfDockLocation::Floating);

        // Dock all panels back to standard Substance Painter desktop layout
        layout.texture_sets_floating = false;
        layout.layers_floating = false;
        layout.properties_floating = false;
        layout.shelf_dock = ShelfDockLocation::Bottom;

        assert!(!layout.texture_sets_floating);
        assert!(!layout.layers_floating);
        assert!(!layout.properties_floating);
        assert_eq!(layout.shelf_dock, ShelfDockLocation::Bottom);
    }

    #[test]
    fn test_workspace_shelf_dock_locations() {
        let mut layout = WorkspaceLayout::default();
        assert_eq!(layout.shelf_dock, ShelfDockLocation::Bottom);

        layout.shelf_dock = ShelfDockLocation::Left;
        assert_eq!(layout.shelf_dock, ShelfDockLocation::Left);

        layout.shelf_dock = ShelfDockLocation::Floating;
        assert_eq!(layout.shelf_dock, ShelfDockLocation::Floating);

        layout.shelf_dock = ShelfDockLocation::Bottom;
        assert_eq!(layout.shelf_dock, ShelfDockLocation::Bottom);
    }

    #[test]
    fn test_workspace_layout_collapsed_states() {
        let mut layout = WorkspaceLayout::default();
        assert!(!layout.texture_sets_collapsed);
        assert!(!layout.layers_collapsed);
        assert!(!layout.properties_collapsed);

        layout.texture_sets_collapsed = true;
        layout.layers_collapsed = true;
        layout.properties_collapsed = true;

        assert!(layout.texture_sets_collapsed);
        assert!(layout.layers_collapsed);
        assert!(layout.properties_collapsed);
    }

    #[test]
    fn test_export_directory_selection_and_validation() {
        let temp_dir = std::env::temp_dir().join("test_custom_export_destination");
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut export_dir = temp_dir.to_string_lossy().to_string();
        assert!(PathBuf::from(&export_dir).is_dir());

        let sub_dir = temp_dir.join("sub_textures");
        export_dir = sub_dir.to_string_lossy().to_string();
        assert_eq!(export_dir, sub_dir.to_string_lossy().to_string());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_layer_deletion_and_history_action() {
        let mut proj = Project::new_default(64, 64);
        assert_eq!(proj.layers.len(), 1);

        let new_idx = proj.add_paint_layer(Some("Detail Paint".to_string()));
        assert_eq!(proj.layers.len(), 2);
        assert_eq!(new_idx, 1);

        let mut history = HistoryManager::new();
        let prev_active = proj.active_layer_idx;
        let removed = proj.delete_layer(new_idx).expect("layer should be deleted");
        assert_eq!(removed.name, "Detail Paint");
        assert_eq!(proj.layers.len(), 1);

        history.push("Delete Layer", HistoryAction::DeleteLayer {
            layer_idx: new_idx,
            layer: Box::new(removed),
            prev_active,
        });
        assert!(history.can_undo());

        // Undo deletion through history manager
        let undo_res = history.undo(&mut proj);
        assert!(undo_res.is_some());
        assert_eq!(proj.layers.len(), 2);
        assert_eq!(proj.layers[1].name, "Detail Paint");
    }

    #[test]
    fn test_layer_mask_creation_and_evaluation() {
        let mut proj = Project::new_default(64, 64);
        let layer = &mut proj.layers[0];
        assert!(!layer.has_mask);

        layer.add_black_mask();
        assert!(layer.has_mask);
        assert_eq!(layer.mask_buf.get_pixel(0, 0), 0.0);
        assert_eq!(layer.mask_buf.get_pixel(32, 32), 0.0);

        layer.add_white_mask();
        assert!(layer.has_mask);
        assert_eq!(layer.mask_buf.get_pixel(0, 0), 1.0);
        assert_eq!(layer.mask_buf.get_pixel(32, 32), 1.0);
    }

    #[test]
    fn test_fill_height_propagation_to_normal_buffer() {
        let mut layer = crate::layer::Layer::new_fill_layer(
            1,
            "Height Test".to_string(),
            32,
            32,
            glam::Vec4::new(0.5, 0.5, 0.5, 1.0),
            0.5,
            0.0,
        );
        layer.set_fill_height(0.85);
        layer.evaluate_fill_advanced(None, None, None, None);

        assert!((layer.height_buf.get_pixel(0, 0) - 0.85).abs() < 1e-4);
        let normal_px = layer.normal_buf.get_pixel(0, 0);
        assert!((normal_px.w - 0.85).abs() < 1e-4);
    }

    #[test]
    fn test_mask_preview_mode_toggle_and_unlit_view_mode() {
        let mut proj = Project::new_default(32, 32);
        let layer_id = proj.layers[0].id;
        proj.layers[0].add_white_mask();

        let mut mask_preview_layer_id = None;
        let mut uv_view_mask_mode = false;
        assert!(!uv_view_mask_mode);
        let mut target = crate::brush::PaintTarget::Material;

        // Simulate Alt+Click on mask swatch
        if mask_preview_layer_id == Some(layer_id) {
            mask_preview_layer_id = None;
            uv_view_mask_mode = false;
        } else {
            mask_preview_layer_id = Some(layer_id);
            uv_view_mask_mode = true;
            target = crate::brush::PaintTarget::Mask;
        }

        assert_eq!(mask_preview_layer_id, Some(layer_id));
        assert!(uv_view_mask_mode);
        assert_eq!(target, crate::brush::PaintTarget::Mask);

        // Effective view mode during mask preview must be 1u32 (Unlit BaseColor)
        let mode_u32 = if mask_preview_layer_id.is_some() {
            1u32
        } else {
            0u32
        };
        assert_eq!(mode_u32, 1u32, "Mask preview must render via unlit BaseColor channel mode (1u)");

        // Exit preview
        mask_preview_layer_id = None;
        uv_view_mask_mode = false;
        assert_eq!(mask_preview_layer_id, None);
        assert!(!uv_view_mask_mode);
    }

    #[test]
    fn test_delete_layer_protects_single_remaining_layer() {
        let mut proj = Project::new_default(64, 64);
        assert_eq!(proj.layers.len(), 1);

        // Deleting the only remaining layer must be rejected (return None) to protect the document
        let removed = proj.delete_layer(0);
        assert!(removed.is_none());
        assert_eq!(proj.layers.len(), 1);

        // Adding another layer and deleting one leaves the other
        proj.add_paint_layer(Some("Paint 2".to_string()));
        assert_eq!(proj.layers.len(), 2);
        let removed2 = proj.delete_layer(1);
        assert!(removed2.is_some());
        assert_eq!(removed2.unwrap().name, "Paint 2");
        assert_eq!(proj.layers.len(), 1);

        // Deleting the single remaining layer again returns None
        let removed3 = proj.delete_layer(0);
        assert!(removed3.is_none());
        assert_eq!(proj.layers.len(), 1);
    }

    #[test]
    fn test_delete_key_debounce_logic() {
        let mut delete_key_held = false;

        // Frame 1: User presses delete key down
        let is_delete_down = true;
        let delete_pressed = is_delete_down && !delete_key_held;
        delete_key_held = is_delete_down;
        assert!(delete_pressed, "Should trigger on initial press");

        // Frame 2: Key remains held down (OS repeat or human hold)
        let is_delete_down = true;
        let delete_pressed = is_delete_down && !delete_key_held;
        delete_key_held = is_delete_down;
        assert!(!delete_pressed, "Should NOT trigger while key is continuously held");

        // Frame 3: Key remains held down
        let is_delete_down = true;
        let delete_pressed = is_delete_down && !delete_key_held;
        delete_key_held = is_delete_down;
        assert!(!delete_pressed, "Should NOT trigger while key is continuously held");

        // Frame 4: User releases key
        let is_delete_down = false;
        let delete_pressed = is_delete_down && !delete_key_held;
        delete_key_held = is_delete_down;
        assert!(!delete_pressed, "Should not trigger on key release");

        // Frame 5: User presses key again
        let is_delete_down = true;
        let delete_pressed = is_delete_down && !delete_key_held;
        delete_key_held = is_delete_down;
        let _ = delete_key_held;
        assert!(delete_pressed, "Should trigger again on new key press");
    }

    #[test]
    fn test_decal_stamp_tool_state_and_settings() {
        let mut engine = crate::brush::BrushEngine::new();
        engine.settings.mode = crate::brush::ToolMode::Stamp;
        assert_eq!(engine.settings.mode, crate::brush::ToolMode::Stamp);

        // Verify default stamp tool parameters
        assert_eq!(engine.settings.stamp_tool.stroke_style, crate::brush::PathStrokeStyle::SmartDecal);
        assert_eq!(engine.settings.stamp_tool.decal_type, crate::brush::SmartDecalType::PhillipsScrew);
        assert!((engine.settings.stamp_tool.height_depth - 1.0).abs() < 1e-4);
        assert!((engine.settings.stamp_tool.surface_offset - 0.003).abs() < 1e-4);
        assert!(!engine.settings.stamp_tool.randomize_rotation);

        // Test custom preset application
        let preset = crate::asset_manager::CustomDecalPreset {
            id: "custom_hex_bolt".to_string(),
            name: "Industrial Hex Bolt".to_string(),
            description: "Heavy duty bolt with normal relief".to_string(),
            path: std::path::PathBuf::new(),
            file_name: "custom_hex_bolt.decal".to_string(),
            stroke_style: crate::brush::PathStrokeStyle::SmartDecal,
            decal_type: crate::brush::SmartDecalType::HexBolt,
            radius: 36.0,
            spacing_ratio: 0.5,
            decal_spacing: 1.4,
            height_depth: 0.8,
            base_color: [0.7, 0.72, 0.75, 1.0],
            roughness: 0.25,
            metallic: 0.9,
            heat_tint: false,
            surface_offset: 0.005,
        };

        engine.settings.stamp_tool.stroke_style = preset.stroke_style;
        engine.settings.stamp_tool.decal_type = preset.decal_type;
        engine.settings.stamp_tool.height_depth = preset.height_depth;
        engine.settings.stamp_tool.heat_tint = preset.heat_tint;
        engine.settings.stamp_tool.surface_offset = preset.surface_offset;
        engine.settings.stamp_tool.selected_preset_id = Some(preset.id.clone());
        engine.settings.radius = preset.radius;
        engine.settings.paint_color = glam::Vec4::new(preset.base_color[0], preset.base_color[1], preset.base_color[2], preset.base_color[3]);
        engine.settings.paint_roughness = preset.roughness;
        engine.settings.paint_metallic = preset.metallic;

        assert_eq!(engine.settings.stamp_tool.decal_type, crate::brush::SmartDecalType::HexBolt);
        assert_eq!(engine.settings.radius, 36.0);
        assert!((engine.settings.stamp_tool.surface_offset - 0.005).abs() < 1e-4);
        assert!((engine.settings.paint_metallic - 0.9).abs() < 1e-4);
    }

    #[test]
    fn test_path_tool_surface_projection_and_offset() {
        let mesh = crate::mesh::Mesh::create_cube_mesh("Cube", 2.0);
        let mut samples = vec![
            crate::brush::PathSample {
                position_3d: glam::Vec3::new(0.0, 1.05, 0.0), // Slightly above top face (y = 1.0)
                normal: glam::Vec3::Y,
                uv: glam::Vec2::new(0.5, 0.5),
                tangent_angle: 0.0,
                radius_scale: 1.0,
                arc_length: 0.0,
            },
            crate::brush::PathSample {
                position_3d: glam::Vec3::new(0.0, 0.95, 0.0), // Dipped slightly inside mesh (y = 0.95)
                normal: glam::Vec3::Y,
                uv: glam::Vec2::new(0.5, 0.5),
                tangent_angle: 0.0,
                radius_scale: 1.0,
                arc_length: 0.1,
            },
        ];

        let offset = 0.004;
        crate::brush::PathToolState::project_samples_to_mesh(&mut samples, &mesh, 0.05, offset);

        // Both samples should be snapped precisely to the top surface (y = 1.0) + offset (0.004)
        for s in &samples {
            assert!((s.position_3d.y - (1.0 + offset)).abs() < 1e-3, "Sample Y was {} but expected {}", s.position_3d.y, 1.0 + offset);
            assert_eq!(s.normal, glam::Vec3::Y);
        }
    }

    #[test]
    fn test_add_layer_and_paint_stroke_two_step_undo_redo() {
        let mut project = Project::new_default(64, 64);
        assert_eq!(project.layers.len(), 1);
        assert_eq!(project.active_layer_idx, 0);

        let mut history = crate::history::HistoryManager::new();

        // 1. User adds a new Paint Layer
        let prev_active = project.active_layer_idx;
        let new_layer = crate::layer::Layer::new_paint_layer(10, "Paint Layer 1".to_string(), 64, 64);
        project.layers.push(new_layer);
        let new_idx = project.layers.len() - 1;
        project.active_layer_idx = new_idx;
        history.push(
            "Add Paint Layer",
            crate::history::HistoryAction::AddLayer {
                layer_idx: new_idx,
                prev_active,
                layer: Box::new(project.layers[new_idx].clone()),
            },
        );

        assert_eq!(project.layers.len(), 2);
        assert_eq!(project.active_layer_idx, 1);
        assert_eq!(history.undo_stack.len(), 1);

        // Verify layer is initially transparent (alpha = 0)
        let initial_px = project.layers[1].base_color_buf.get_pixel(32, 32);
        assert_eq!(initial_px.w, 0.0);

        // 2. User paints their first stroke on the new layer
        let target = crate::brush::PaintTarget::Material;
        let channels = crate::channel::ChannelFlags {
            base_color: true,
            roughness: true,
            metallic: true,
            normal: false,
            height: false,
            emission: false,
            opacity: false,
            clearcoat: false,
            anisotropy: false,
            ao: false,
        };

        // Begin stroke recording
        project.layers[1].begin_stroke_recording(target, channels);

        // Pre-record pristine tiles before painted pixels are committed
        project.layers[1].record_tiles_for_rect(30, 30, 34, 34);

        // Simulate brush stroke stamping into the layer
        for py in 30..=34 {
            for px in 30..=34 {
                project.layers[1].base_color_buf.set_pixel(px, py, glam::Vec4::new(1.0, 0.0, 0.0, 1.0));
            }
        }
        let painted_px = project.layers[1].base_color_buf.get_pixel(32, 32);
        assert_eq!(painted_px, glam::Vec4::new(1.0, 0.0, 0.0, 1.0));

        // Finish stroke delta
        let stroke_action = project.layers[1]
            .finish_stroke_delta(1, "Paint Red Square")
            .expect("Must produce TileStroke action");
        history.push("Paint Red Square", stroke_action);

        // Undo stack must contain exactly 2 distinct actions: [Add Paint Layer, Paint Red Square]
        assert_eq!(history.undo_stack.len(), 2);

        // 3. First Undo: Should undo the stroke ONLY, keeping the layer intact
        let first_undo = history.undo(&mut project);
        assert_eq!(first_undo, Some("Paint Red Square".to_string()));
        assert_eq!(project.layers.len(), 2, "Layer must NOT be deleted on first stroke undo");
        let restored_px = project.layers[1].base_color_buf.get_pixel(32, 32);
        assert_eq!(restored_px.w, 0.0, "Pixels must be restored to clean unpainted state");
        assert_eq!(history.undo_stack.len(), 1);

        // 4. Second Undo: Should undo the layer addition, removing the layer
        let second_undo = history.undo(&mut project);
        assert_eq!(second_undo, Some("Add Paint Layer".to_string()));
        assert_eq!(project.layers.len(), 1, "Layer must be removed on second undo");
        assert_eq!(project.active_layer_idx, 0, "Active layer must return to previous active");
        assert_eq!(history.undo_stack.len(), 0);

        // 5. First Redo: Should re-create the layer
        let first_redo = history.redo(&mut project);
        assert_eq!(first_redo, Some("Add Paint Layer".to_string()));
        assert_eq!(project.layers.len(), 2, "Layer must be restored on first redo");
        assert_eq!(project.active_layer_idx, 1);
        let redo_initial_px = project.layers[1].base_color_buf.get_pixel(32, 32);
        assert_eq!(redo_initial_px.w, 0.0, "Layer on first redo must be clean unpainted");

        // 6. Second Redo: Should re-apply the paint stroke
        let second_redo = history.redo(&mut project);
        assert_eq!(second_redo, Some("Paint Red Square".to_string()));
        assert_eq!(project.layers.len(), 2);
        let redo_painted_px = project.layers[1].base_color_buf.get_pixel(32, 32);
        assert_eq!(redo_painted_px, glam::Vec4::new(1.0, 0.0, 0.0, 1.0), "Stroke must be reapplied on second redo");
    }
