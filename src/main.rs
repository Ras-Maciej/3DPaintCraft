// 3D Paintcraft - A Clean-Room Recreation of Substance 3D Painter in Pure Rust for Linux
// Part of the ArtCraft Creative Suite

mod app;
mod baker;
mod brush;
mod channel;
mod export;
mod file_dialog;
mod hdri;
mod hdri_generator;
mod gpu_compositor;
mod gpu_brush;
mod history;
mod layer;
mod mcp;
mod mesh;
mod project;
mod renderer;
mod wgpu_renderer;
mod nvidia_dlaa;
mod smart_materials;
mod shelf;
mod asset_manager;
mod utils;
mod updater;

use app::PaintcraftApp;
use export::ExportPreset;
use mcp::McpServer;
use project::Project;
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = env::args().collect();

    // Mode 1: AI Agent Model Context Protocol (MCP) Server
    if args.iter().any(|arg| arg == "--mcp") {
        eprintln!("[3D Paintcraft] Starting ArtCraft Model Context Protocol (MCP) Server...");
        let project = Project::new_default(512, 512);
        McpServer::run_stdio_server(project);
        return Ok(());
    }

    // Mode 2: Headless Command Line Exporter / Automation
    if let Some(export_idx) = args.iter().position(|arg| arg == "--export") {
        let preset_str = args.get(export_idx + 1).map(|s| s.as_str()).unwrap_or("UnrealEngine");
        let out_dir_str = args
            .iter()
            .position(|arg| arg == "--out")
            .and_then(|idx| args.get(idx + 1))
            .map(|s| s.as_str())
            .unwrap_or("./exports");

        let mut project = Project::new_default(1024, 1024);

        if let Some(mat_idx) = args.iter().position(|arg| arg == "--material") {
            if let Some(mat_name) = args.get(mat_idx + 1) {
                project.apply_smart_material(mat_name);
            }
        }

        let preset = match preset_str {
            "Unity" => ExportPreset::Unity,
            "BlenderGltf" => ExportPreset::BlenderGltf,
            "Individual" => ExportPreset::IndividualMaps,
            _ => ExportPreset::UnrealEngine,
        };

        let out_path = PathBuf::from(out_dir_str);
        match export::TextureExporter::export_preset(&project.composite_textures, &project.mesh.name, preset, &out_path, true, Some(&project.mesh)) {
            Ok(files) => {
                println!("[3D Paintcraft] Successfully exported {} texture maps to {:?}", files.len(), out_path);
                for f in files {
                    println!("  -> {}", f.display());
                }
            }
            Err(e) => {
                eprintln!("[3D Paintcraft] Export error: {}", e);
            }
        }
        return Ok(());
    }

    // Mode 3: Hardware Diagnostics & Performance Benchmark
    if args.iter().any(|arg| arg == "--diagnostics" || arg == "--benchmark" || arg == "--profile") {
        run_system_diagnostics();
        return Ok(());
    }

    // Mode 4: Desktop Linux GUI Application (egui / eframe)
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    wgpu_options.power_preference = eframe::wgpu::PowerPreference::HighPerformance;
    let env_present = std::env::var("PAINTCRAFT_PRESENT_MODE").unwrap_or_default().to_lowercase();
    wgpu_options.present_mode = match env_present.as_str() {
        "immediate" => eframe::wgpu::PresentMode::Immediate,
        "mailbox" => eframe::wgpu::PresentMode::Mailbox,
        "no_vsync" | "autonovsync" => eframe::wgpu::PresentMode::AutoNoVsync,
        _ => eframe::wgpu::PresentMode::AutoVsync,
    };
    wgpu_options.device_descriptor = std::sync::Arc::new(|_adapter| {
        let mut desc = eframe::wgpu::DeviceDescriptor::default();
        desc.required_features |= eframe::wgpu::Features::FLOAT32_FILTERABLE;
        desc
    });

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("3D Paintcraft - ArtCraft Suite")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 600.0]),
        wgpu_options,
        ..Default::default()
    };

    eframe::run_native(
        "3D Paintcraft",
        native_options,
        Box::new(|cc| Ok(Box::new(PaintcraftApp::new(cc)))),
    )
}

fn run_system_diagnostics() {
    println!("============================================================");
    println!("  3D PAINTCRAFT SYSTEM PERFORMANCE & HARDWARE DIAGNOSTICS   ");
    println!("============================================================");

    let is_debug = cfg!(debug_assertions);
    if is_debug {
        println!("BUILD PROFILE: Debug (Unoptimized)");
        println!("  CAUTION: Debug builds include full assertions and no inlining.");
        println!("  For maximum 60+ FPS performance, run with: cargo run --release");
    } else {
        println!("BUILD PROFILE: Release (Optimized - 100% full speed)");
    }

    // 1. GPU Probe
    println!("\n--- 1. GPU HARDWARE & ADAPTER PROBE ---");
    let instance = eframe::wgpu::Instance::default();
    let adapters = instance.enumerate_adapters(eframe::wgpu::Backends::all());
    if adapters.is_empty() {
        println!("  ERROR: No WGPU graphics adapters detected on this system!");
    } else {
        println!("  Found {} graphics adapter(s):", adapters.len());
        for (i, a) in adapters.iter().enumerate() {
            let info = a.get_info();
            println!("    [{}] \"{}\" | Backend: {:?} | Type: {:?}", i, info.name, info.backend, info.device_type);
        }
    }

    let high_perf_adapter = pollster::block_on(instance.request_adapter(&eframe::wgpu::RequestAdapterOptions {
        power_preference: eframe::wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }));

    let (device, queue) = if let Some(adapter) = high_perf_adapter {
        let info = adapter.get_info();
        println!("  Selected High-Performance Adapter: \"{}\" ({:?})", info.name, info.backend);
        if info.device_type == eframe::wgpu::DeviceType::Cpu {
            println!("  WARNING: Selected adapter is CPU/Software (llvmpipe)!");
            println!("  Shaders will execute on the CPU via software emulation.");
        } else {
            println!("  Dedicated hardware GPU acceleration ACTIVE.");
        }
        match pollster::block_on(adapter.request_device(
            &eframe::wgpu::DeviceDescriptor {
                label: Some("Diag Device"),
                required_features: eframe::wgpu::Features::FLOAT32_FILTERABLE,
                required_limits: eframe::wgpu::Limits::default(),
            },
            None,
        )) {
            Ok(dq) => (Some(std::sync::Arc::new(dq.0)), Some(std::sync::Arc::new(dq.1))),
            Err(e) => {
                println!("  Device creation error with FLOAT32_FILTERABLE: {:?}", e);
                (None, None)
            }
        }
    } else {
        println!("  No high-performance adapter could be acquired.");
        (None, None)
    };

    // 2. Mesh & BVH Raycasting Benchmark
    println!("\n--- 2. BVH RAYCASTING & GEOMETRY PROBE ---");
    let mesh = crate::mesh::Mesh::create_shader_ball();
    println!("  Test Mesh: \"{}\" ({} vertices, {} triangles)", mesh.name, mesh.vertices.len(), mesh.indices.len() / 3);

    let ray = crate::mesh::Ray {
        origin: glam::Vec3::new(0.0, 0.0, 3.0),
        direction: glam::Vec3::new(0.0, 0.0, -1.0),
    };
    let ray_iters = 10_000;
    let t0 = std::time::Instant::now();
    for _ in 0..ray_iters {
        let _ = mesh.raycast(&ray);
    }
    let ray_elapsed = t0.elapsed();
    let per_ray_us = ray_elapsed.as_micros() as f64 / ray_iters as f64;
    println!("  10,000 Raycasts: {:?} ({:.2} us/raycast, {:.0} rays/sec)", ray_elapsed, per_ray_us, 1_000_000.0 / per_ray_us.max(0.01));

    let query_iters = 10_000;
    let t1 = std::time::Instant::now();
    for _ in 0..query_iters {
        let _ = mesh.query_sphere(glam::Vec3::new(0.0, 0.0, 1.0), 0.15);
    }
    let query_elapsed = t1.elapsed();
    let per_query_us = query_elapsed.as_micros() as f64 / query_iters as f64;
    println!("  10,000 Sphere Queries: {:?} ({:.2} us/query, {:.0} queries/sec)", query_elapsed, per_query_us, 1_000_000.0 / per_query_us.max(0.01));

    // 3. 3D Projection Painting Benchmark
    println!("\n--- 3. 3D PROJECTION PAINTING ENGINE (CPU STAMPING) ---");
    for &res in &[512, 1024, 2048] {
        let mut layer = crate::layer::Layer::new_paint_layer(1, "Bench".to_string(), res, res);
        let mut engine = crate::brush::BrushEngine::new();
        engine.settings.radius = 25.0;
        engine.update_cache();

        let stamp_iters = 100;
        let t = std::time::Instant::now();
        for i in 0..stamp_iters {
            let offset = (i as f32) * 0.001;
            let hit_p = glam::Vec3::new(offset, offset, 1.0);
            let hit_n = glam::Vec3::new(0.0, 0.0, 1.0);
            engine.stamp_3d(&mut layer, &mesh, hit_p, hit_n, 0.15);
        }
        let elapsed = t.elapsed();
        let per_stamp_ms = elapsed.as_secs_f64() * 1000.0 / stamp_iters as f64;
        let stamps_per_sec = stamp_iters as f64 / elapsed.as_secs_f64();
        println!("  {}x{} Document: 100 stamps took {:?} ({:.3} ms/stamp, {:.0} stamps/sec)", res, res, elapsed, per_stamp_ms, stamps_per_sec);
    }

    // 4. GPU Compositor & Stamping Controlled 1K vs 4K Tests
    if let (Some(dev), Some(q)) = (device, queue) {
        println!("\n--- 4. CONTROLLED 1K vs 4K BENCHMARKS (PAINTING ENABLED vs DISABLED) ---");

        for &res in &[1024, 4096] {
            let label = if res == 1024 { "1K (1024x1024)" } else { "4K (4096x4096)" };
            println!("\n>>> BENCHMARKING DOCUMENT RESOLUTION: {} <<<", label);

            // A. Memory & VRAM Footprint Analysis
            let px_count = (res as u64) * (res as u64);
            let layer_vram_bytes = px_count * (16 + 16 + 4 + 4 + 4); // BC, N, R, M, Mask
            let comp_vram_bytes = px_count * (16 + 16 + 4 + 4) * 2;   // Buffer A + B
            let pbr_vram_bytes = px_count * (16 + 16 + 4 + 4 + 16 + 8); // PBR + Baked
            let scratch_vram_bytes = px_count * (16 + 16 + 4 + 4);    // Scratch Brush
            let total_vram_mb = (layer_vram_bytes + comp_vram_bytes + pbr_vram_bytes + scratch_vram_bytes) as f64 / (1024.0 * 1024.0);
            let cpu_layer_ram_mb = (px_count * (16 + 16 + 4 + 4 + 4 + 4)) as f64 / (1024.0 * 1024.0);

            println!("  [Memory Footprint]");
            println!("    - Layer Textures (VRAM): {:.1} MB", layer_vram_bytes as f64 / (1024.0 * 1024.0));
            println!("    - Compositor Double-Buffers (VRAM): {:.1} MB", comp_vram_bytes as f64 / (1024.0 * 1024.0));
            println!("    - Material Textures (VRAM): {:.1} MB", pbr_vram_bytes as f64 / (1024.0 * 1024.0));
            println!("    - Scratch Brush Textures (VRAM): {:.1} MB", scratch_vram_bytes as f64 / (1024.0 * 1024.0));
            println!("    - Total Document VRAM Usage (1 Layer): {:.1} MB", total_vram_mb);
            println!("    - CPU Layer Buffers (RAM): {:.1} MB", cpu_layer_ram_mb);

            // B. Test 1: Viewport Rendering (Painting DISABLED)
            let mut comp = crate::gpu_compositor::GpuCompositor::new(dev.clone(), q.clone(), res, res);
            let cpu_layer = crate::layer::Layer::new_paint_layer(1, "BenchLayer".to_string(), res, res);
            let gpu_layer = comp.create_layer(&cpu_layer, res, res);

            // C. Test 2: Brush Processing & Stamping (Painting ENABLED)
            let mut brush = crate::gpu_brush::GpuBrushEngine::new(dev.clone(), q.clone(), res, res);
            let mut brush_engine = crate::brush::BrushEngine::new();
            brush_engine.settings.radius = 25.0;
            brush_engine.update_cache();

            let stamp_data = brush_engine.build_gpu_stamp_data(
                &mesh,
                glam::Vec3::new(0.0, 0.0, 1.0),
                glam::Vec3::new(0.0, 0.0, 1.0),
                0.15,
                res,
                res,
            );
            let (uniform, tris) = stamp_data.unwrap_or_else(|| {
                (
                    crate::gpu_brush::GpuBrushUniform {
                        hit_pos: [0.0, 0.0, 1.0, 0.15],
                        hit_normal: [0.0, 0.0, 1.0, 1.0],
                        paint_color: [1.0, 0.0, 0.0, 1.0],
                        params: [0.5, 0.0, 0.0, 1.0],
                        bounds_min: [100, 100],
                        bounds_max: [200, 200],
                        tex_size: [res as f32, res as f32],
                        flags: [0, 0],
                        tri_count: 0,
                        _pad0: 0,
                        _pad1: 0,
                        _pad2: 0,
                    },
                    Vec::new(),
                )
            });

            let paint_iters = 10;
            let mut total_stamp_ms = 0.0;
            let mut total_comp_ms = 0.0;
            let mut total_copy_ms = 0.0;
            let mut total_submit_ms = 0.0;
            let bytes_per_full_copy = px_count * (16 + 16 + 4 + 4);

            let cpu_layers_slice = vec![cpu_layer];
            let gpu_layers_slice = vec![gpu_layer];

            for _ in 0..paint_iters {
                let t_stamp = std::time::Instant::now();
                brush.stamp_3d_gpu(&gpu_layers_slice[0], &uniform, &tris);
                total_stamp_ms += t_stamp.elapsed().as_secs_f64() * 1000.0;

                let t_comp = std::time::Instant::now();
                let final_target = comp.composite_all(&cpu_layers_slice, &gpu_layers_slice);
                total_comp_ms += t_comp.elapsed().as_secs_f64() * 1000.0;

                let t_copy = std::time::Instant::now();
                let mut encoder = dev.create_command_encoder(&eframe::wgpu::CommandEncoderDescriptor { label: None });
                let copy_size = eframe::wgpu::Extent3d { width: res, height: res, depth_or_array_layers: 1 };
                encoder.copy_texture_to_texture(
                    eframe::wgpu::ImageCopyTexture { texture: &final_target.base_color, mip_level: 0, origin: eframe::wgpu::Origin3d::ZERO, aspect: eframe::wgpu::TextureAspect::All },
                    eframe::wgpu::ImageCopyTexture { texture: &gpu_layers_slice[0].base_color, mip_level: 0, origin: eframe::wgpu::Origin3d::ZERO, aspect: eframe::wgpu::TextureAspect::All },
                    copy_size
                );
                encoder.copy_texture_to_texture(
                    eframe::wgpu::ImageCopyTexture { texture: &final_target.normal, mip_level: 0, origin: eframe::wgpu::Origin3d::ZERO, aspect: eframe::wgpu::TextureAspect::All },
                    eframe::wgpu::ImageCopyTexture { texture: &gpu_layers_slice[0].normal, mip_level: 0, origin: eframe::wgpu::Origin3d::ZERO, aspect: eframe::wgpu::TextureAspect::All },
                    copy_size
                );
                encoder.copy_texture_to_texture(
                    eframe::wgpu::ImageCopyTexture { texture: &final_target.roughness, mip_level: 0, origin: eframe::wgpu::Origin3d::ZERO, aspect: eframe::wgpu::TextureAspect::All },
                    eframe::wgpu::ImageCopyTexture { texture: &gpu_layers_slice[0].roughness, mip_level: 0, origin: eframe::wgpu::Origin3d::ZERO, aspect: eframe::wgpu::TextureAspect::All },
                    copy_size
                );
                encoder.copy_texture_to_texture(
                    eframe::wgpu::ImageCopyTexture { texture: &final_target.metallic, mip_level: 0, origin: eframe::wgpu::Origin3d::ZERO, aspect: eframe::wgpu::TextureAspect::All },
                    eframe::wgpu::ImageCopyTexture { texture: &gpu_layers_slice[0].metallic, mip_level: 0, origin: eframe::wgpu::Origin3d::ZERO, aspect: eframe::wgpu::TextureAspect::All },
                    copy_size
                );
                total_copy_ms += t_copy.elapsed().as_secs_f64() * 1000.0;

                let t_sub = std::time::Instant::now();
                q.submit(std::iter::once(encoder.finish()));
                total_submit_ms += t_sub.elapsed().as_secs_f64() * 1000.0;
            }

            let avg_stamp = total_stamp_ms / paint_iters as f64;
            let avg_comp = total_comp_ms / paint_iters as f64;
            let avg_copy = total_copy_ms / paint_iters as f64;
            let avg_submit = total_submit_ms / paint_iters as f64;
            let total_paint_ms = avg_stamp + avg_comp + avg_copy + avg_submit;

            println!("  [Painting Enabled - Per Brush Dab Latency]");
            println!("    - GPU Compute Stamping: {:.3} ms", avg_stamp);
            println!("    - GPU Full Layer Compositing: {:.3} ms", avg_comp);
            println!("    - Texture Blit / Copy Encoding: {:.3} ms", avg_copy);
            println!("    - Queue Submission: {:.3} ms", avg_submit);
            println!("    - Texture Transfer Size per Dab: {:.1} MB", bytes_per_full_copy as f64 / (1024.0 * 1024.0));
            println!("    - Total GPU Latency per Dab: {:.3} ms (equiv {:.1} FPS)", total_paint_ms, 1000.0 / total_paint_ms.max(0.001));

            // D. Test 3: CPU Layer Compositing & UV Canvas Generation
            let mut test_project = crate::project::Project::new_default(res, res);
            test_project.add_paint_layer(Some("Paint".to_string()));
            let t_cpu_comp = std::time::Instant::now();
            test_project.composite_textures.composite_layers(&test_project.layers);
            let cpu_comp_ms = t_cpu_comp.elapsed().as_secs_f64() * 1000.0;

            let t_cpu_alloc = std::time::Instant::now();
            let mut uv_pixels = vec![0u8; (res as usize) * (res as usize) * 4];
            let base_col = &test_project.composite_textures.base_color;
            use rayon::prelude::*;
            uv_pixels.par_chunks_exact_mut((res as usize) * 4).enumerate().for_each(|(y, row)| {
                for x in 0..(res as usize) {
                    let idx = x * 4;
                    let col = base_col.get_pixel(x as u32, y as u32);
                    row[idx] = (col.x.clamp(0.0, 1.0) * 255.0) as u8;
                    row[idx + 1] = (col.y.clamp(0.0, 1.0) * 255.0) as u8;
                    row[idx + 2] = (col.z.clamp(0.0, 1.0) * 255.0) as u8;
                    row[idx + 3] = (col.w.clamp(0.0, 1.0) * 255.0) as u8;
                }
            });
            let cpu_uv_ms = t_cpu_alloc.elapsed().as_secs_f64() * 1000.0;

            println!("  [CPU Side Document Overhead]");
            println!("    - CPU Rayon Compositing: {:.2} ms", cpu_comp_ms);
            println!("    - CPU 2D UV Texture Conversion: {:.2} ms", cpu_uv_ms);
            println!("    - Total CPU Stalling per Frame: {:.2} ms", cpu_comp_ms + cpu_uv_ms);
        }
    }

    println!("\n============================================================");
    println!("  DIAGNOSTICS COMPLETE");
    println!("============================================================");
}
