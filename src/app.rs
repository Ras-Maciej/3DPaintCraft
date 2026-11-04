// 3D Paintcraft - High-Performance PBR Texture Painting & Baking Suite
// Built with pure, safe Rust for Windows and Linux

use crate::brush::{AlphaType, BrushEngine, PaintTarget, ToolMode};
#[allow(unused_imports)]
use crate::export::{ExportPreset, TextureExporter};
#[allow(unused_imports)]
use crate::packing::{
    ChannelSlot, ChannelSource, CustomPackingPreset, CustomTexturePacker,
    PackedColorMode, PackedImageFormat, PackedTextureMap,
};
use crate::file_dialog::FileExplorerDialog;
use crate::history::{HistoryAction, HistoryManager};
use crate::layer::{BlendMode, LayerType, ProceduralGenerator};
use crate::mesh::Mesh;
use crate::project::Project;
use crate::renderer::{Camera, ViewportChannelMode, ViewportRenderer};
use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions, Vec2};
use glam::Vec4;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, channel};

mod baking_screen;
mod dock_panels;
mod export_modal;
mod shelf_panels;
mod viewport_2d;
mod viewport_3d;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewLayout {
    View3DOnly,
    View2DOnly,
    SplitDualView,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RightPanelTab {
    Layers,
    MeshMaps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShelfDockLocation {
    Bottom,
    Left,
    Floating,
}

#[derive(Clone, Debug)]
pub struct WorkspaceLayout {
    pub show_toolbar: bool,
    pub show_context_bar: bool,
    pub show_status_bar: bool,

    // Panel visibility
    pub show_texture_sets: bool,
    pub show_layers: bool,
    pub show_properties: bool,
    pub show_shelf: bool,

    // Floating / undocked states
    pub texture_sets_floating: bool,
    pub layers_floating: bool,
    pub properties_floating: bool,
    pub shelf_dock: ShelfDockLocation,

    // Resizable panel dimensions
    pub right_panel_width: f32,
    pub bottom_shelf_height: f32,
    pub left_shelf_width: f32,
    pub layers_pane_height: f32,

    // Collapsed states when docked
    pub texture_sets_collapsed: bool,
    pub layers_collapsed: bool,
    pub properties_collapsed: bool,
}

impl Default for WorkspaceLayout {
    fn default() -> Self {
        Self {
            show_toolbar: true,
            show_context_bar: true,
            show_status_bar: true,

            show_texture_sets: true,
            show_layers: true,
            show_properties: true,
            show_shelf: true,

            texture_sets_floating: false,
            layers_floating: false,
            properties_floating: false,
            shelf_dock: ShelfDockLocation::Bottom,

            right_panel_width: 380.0,
            bottom_shelf_height: 220.0,
            left_shelf_width: 210.0,
            layers_pane_height: 240.0,

            texture_sets_collapsed: false,
            layers_collapsed: false,
            properties_collapsed: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BakeProgressState {
    pub progress: f32,
    pub step_name: String,
    pub is_done: bool,
    pub cancelled: bool,
    pub elapsed_secs: f32,
}

pub struct BakeResultPayload {
    pub tangent_normal: Option<crate::channel::RgbaBuffer>,
    pub world_space_normal: Option<crate::channel::RgbaBuffer>,
    pub ao: Option<crate::channel::ScalarBuffer>,
    pub curvature: Option<crate::channel::ScalarBuffer>,
    pub position: Option<crate::channel::RgbaBuffer>,
    pub thickness: Option<crate::channel::ScalarBuffer>,
    pub id_map: Option<crate::channel::RgbaBuffer>,
    pub elapsed_secs: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShelfPickerTarget {
    FillBaseColor,
    FillRoughness,
    FillMetallic,
    FillNormal,
    FillHeight,
    FillEmissive,
    FillOpacity,
    Stencil,
    BrushAlpha,
    MeshMapTangentNormal,
    MeshMapWorldSpaceNormal,
    MeshMapAo,
    MeshMapCurvature,
    MeshMapPosition,
    MeshMapThickness,
    MeshMapId,
    HdriEnvironment,
}

#[derive(Clone, Debug)]
pub struct ShelfPickerState {
    pub target: ShelfPickerTarget,
    pub search_query: String,
    pub selected_category: crate::shelf::ShelfCategory,
}

impl ShelfPickerState {
    pub fn new(target: ShelfPickerTarget) -> Self {
        let selected_category = match target {
            ShelfPickerTarget::FillBaseColor
            | ShelfPickerTarget::FillRoughness
            | ShelfPickerTarget::FillMetallic
            | ShelfPickerTarget::FillNormal
            | ShelfPickerTarget::FillHeight
            | ShelfPickerTarget::FillEmissive
            | ShelfPickerTarget::FillOpacity => crate::shelf::ShelfCategory::Project,
            ShelfPickerTarget::Stencil => crate::shelf::ShelfCategory::Project,
            ShelfPickerTarget::BrushAlpha => crate::shelf::ShelfCategory::Alphas,
            ShelfPickerTarget::MeshMapTangentNormal
            | ShelfPickerTarget::MeshMapWorldSpaceNormal
            | ShelfPickerTarget::MeshMapAo
            | ShelfPickerTarget::MeshMapCurvature
            | ShelfPickerTarget::MeshMapPosition
            | ShelfPickerTarget::MeshMapThickness
            | ShelfPickerTarget::MeshMapId => crate::shelf::ShelfCategory::Project,
            ShelfPickerTarget::HdriEnvironment => crate::shelf::ShelfCategory::Environments,
        };

        Self {
            target,
            search_query: String::new(),
            selected_category,
        }
    }

    pub fn title(&self) -> &'static str {
        match self.target {
            ShelfPickerTarget::FillBaseColor => "Select Texture for Base Color",
            ShelfPickerTarget::FillRoughness => "Select Texture for Roughness",
            ShelfPickerTarget::FillMetallic => "Select Texture for Metallic",
            ShelfPickerTarget::FillNormal => "Select Texture for Normal",
            ShelfPickerTarget::FillHeight => "Select Texture for Height",
            ShelfPickerTarget::FillEmissive => "Select Texture for Emissive",
            ShelfPickerTarget::FillOpacity => "Select Texture for Opacity",
            ShelfPickerTarget::Stencil => "Select Stencil Texture (Projection)",
            ShelfPickerTarget::BrushAlpha => "Select Brush Alpha",
            ShelfPickerTarget::MeshMapTangentNormal => "Select Tangent Normal Map",
            ShelfPickerTarget::MeshMapWorldSpaceNormal => "Select World Space Normal Map",
            ShelfPickerTarget::MeshMapAo => "Select Ambient Occlusion (AO) Map",
            ShelfPickerTarget::MeshMapCurvature => "Select Curvature Map",
            ShelfPickerTarget::MeshMapPosition => "Select Position Map",
            ShelfPickerTarget::MeshMapThickness => "Select Thickness Map",
            ShelfPickerTarget::MeshMapId => "Select ID Map",
            ShelfPickerTarget::HdriEnvironment => "Select HDRI Environment",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct BakeCageCacheKey {
    pub mode: crate::baker::BakeCageMode,
    pub inflation_bits: u32,
    pub frontal_bits: u32,
    pub rear_bits: u32,
    pub preview_rear: bool,
    pub path: String,
    pub show_hull: bool,
    pub show_tethers: bool,
    pub color_idx: usize,
    pub vert_count: usize,
}

pub struct PaintcraftApp {
    pub project: Project,
    pub camera: Camera,
    pub renderer: ViewportRenderer,
    pub brush_engine: BrushEngine,
    pub history: HistoryManager,
    pub file_explorer: FileExplorerDialog,

    // Stroke undo tracking
    pub is_stroke_active: bool,
    pub gpu_stroke_dirty_rect: Option<(u32, u32, u32, u32)>,

    // View & layout state
    pub view_mode: ViewportChannelMode,
    pub layout: ViewLayout,

    // Texture handles for egui rendering
    pub viewport_texture: Option<TextureHandle>,
    pub uv_canvas_texture: Option<TextureHandle>,
    pub uv_pixel_buffer: Vec<u8>,

    // UI state
    pub status_message: String,
    #[allow(dead_code)]
    pub export_preset: ExportPreset,
    pub export_image_format: crate::packing::PackedImageFormat,
    pub last_stylus_pressure: f32,
    pub export_dir: String,
    pub export_flip_v: bool,
    pub export_mesh: bool,
    pub export_tab_idx: usize,
    pub custom_packing_presets: Vec<crate::packing::CustomPackingPreset>,
    pub active_custom_packing_idx: usize,
    pub custom_packing_status: Option<String>,
    pub custom_preset_io_filename: String,
    pub export_template_search: String,
    pub export_selected_ts_idx: usize,
    pub export_last_exported_files: Vec<PathBuf>,
    pub export_resolution_override: u32,
    pub export_padding_px: u32,
    pub export_udim_naming: bool,
    pub project_file_path: String,

    pub show_about_modal: bool,
    pub show_export_modal: bool,
    pub show_save_modal: bool,
    pub show_bake_modal: bool,
    pub show_viewport_settings_modal: bool,
    pub show_shader_settings_modal: bool,
    pub show_filter_modal: bool,
    pub filter_type_idx: usize,
    pub filter_target_mask: bool,
    pub filter_blur_radius: f32,
    pub filter_levels_in_black: f32,
    pub filter_levels_in_white: f32,
    pub filter_levels_gamma: f32,
    pub filter_levels_out_black: f32,
    pub filter_levels_out_white: f32,
    pub filter_sharpen_strength: f32,
    pub filter_contrast: f32,
    pub filter_brightness: f32,
    pub filter_high_pass_radius: f32,
    pub bake_highpoly_path: String,

    // Substance 3D Painter Startup & New Project Dialog
    pub show_new_project_modal: bool,
    pub new_proj_template: usize,
    pub new_proj_mesh_type: usize,
    pub new_proj_primitive_idx: usize,
    pub new_proj_custom_path: String,
    pub new_proj_resolution: u32,
    pub new_proj_normal_format: u32,
    pub new_proj_auto_bake: bool,
    pub new_proj_bake_res: u32,

    // Layer Stack UI State
    pub dragged_layer_idx: Option<usize>,
    pub renaming_layer_idx: Option<usize>,
    pub rename_buffer: String,
    pub delete_key_held: bool,

    // Shader & Viewport Settings State
    pub shader_settings: crate::wgpu_renderer::ShaderSettings,
    pub viewport_bg_color: [f32; 3],
    pub camera_orbit_speed: f32,
    pub camera_pan_speed: f32,
    pub camera_zoom_speed: f32,

    // Project Settings State
    pub project_name_edit: String,
    pub new_project_width: u32,
    pub new_project_height: u32,

    pub last_painted_uv: Option<glam::Vec2>,
    pub bake_res: u32,
    pub bake_frontal: f32,
    pub bake_rear: f32,
    pub bake_opt_tangent_normal: bool,
    pub bake_opt_world_normal: bool,
    pub bake_opt_ao: bool,
    pub bake_opt_curvature: bool,
    pub bake_opt_position: bool,
    pub bake_opt_thickness: bool,
    pub bake_opt_thickness_invert: bool,
    pub bake_opt_id_map: bool,
    pub bake_denoise: bool,
    pub bake_denoise_iterations: usize,
    pub bake_match_mode: crate::baker::BakeMatchMode,
    pub bake_low_suffix: String,
    pub bake_high_suffix: String,
    pub bake_cage_mode: crate::baker::BakeCageMode,
    pub bake_cage_inflation: f32,
    pub bake_cage_path: String,
    pub bake_preview_cage: bool,
    pub bake_cage_show_hull: bool,
    pub bake_cage_show_tethers: bool,
    pub bake_cage_color_idx: usize,
    pub bake_cage_opacity: f32,
    pub bake_cage_preview_rear: bool,
    pub bake_cage_cache_key: BakeCageCacheKey,
    pub bake_active_settings_tab: crate::baker::BakeSettingsTab,
    pub bake_ao_config: crate::baker::AoBakeConfig,
    pub bake_thickness_config: crate::baker::ThicknessBakeConfig,
    pub bake_curvature_config: crate::baker::CurvatureBakeConfig,
    pub bake_normal_config: crate::baker::NormalBakeConfig,
    pub bake_position_config: crate::baker::PositionBakeConfig,
    pub bg_blur: f32,
    pub bg_opacity: f32,
    pub layer_stack_channel: crate::channel::ChannelType,
    #[allow(dead_code)]
    pub stencil_image_path: String,
    #[allow(dead_code)]
    pub stencil_overlay_texture: Option<TextureHandle>,
    pub right_panel_tab: RightPanelTab,
    pub workspace: WorkspaceLayout,
    pub show_shelf: bool,
    pub shelf_category: crate::shelf::ShelfCategory,
    pub shelf_search_query: String,
    pub shelf_picker: Option<ShelfPickerState>,
    pub shelf_thumbnails: std::collections::HashMap<String, TextureHandle>,
    pub shelf_thumbnails_dirty: bool,
    pub shelf_resource_popup: Option<String>,
    pub asset_manager: crate::asset_manager::AssetManager,
    pub wgpu_viewport: Option<crate::wgpu_renderer::WgpuViewport>,
    pub updater: crate::updater::UpdateNotifier,

    // 2D UV Viewport Navigation & Wireframe State
    pub uv_2d_pan: egui::Vec2,
    pub uv_2d_zoom: f32,
    pub show_uv_wireframe: bool,
    pub split_view_ratio: f32,
    pub uv_view_mask_mode: bool,
    pub mask_preview_layer_id: Option<u64>,

    // Recent Projects
    pub recent_projects: Vec<String>,

    // Substance 3D Painter Zen Mode (Tab)
    pub zen_mode: bool,
    pub continuous_60fps: bool,

    // Active Shader Performance Preset
    pub shader_preset: crate::wgpu_renderer::ShaderPerformancePreset,

    // Real-Time Hardware Performance & Diagnostics Metrics
    pub perf_metrics: PerformanceMetrics,

    // Synchronized 2D/3D Viewport Hover State & Polygon Fill Drag Tracking
    pub last_hovered_hit_3d: Option<(glam::Vec3, glam::Vec3, glam::Vec2)>,
    pub last_hovered_uv_2d: Option<glam::Vec2>,
    pub last_hovered_tri: Option<usize>,
    pub polygon_fill_drag_tris: std::collections::HashSet<usize>,
    pub poly_fill_hover_cache: Option<(usize, crate::brush::PolygonFillMode, Vec<usize>)>,

    // Dedicated Baking Screen & Background Worker State
    pub bake_worker_state: Option<Arc<Mutex<BakeProgressState>>>,
    pub bake_receiver: Option<Receiver<BakeResultPayload>>,
    pub bake_selected_texture_sets: Vec<bool>,
    pub bake_preview_normal: Option<TextureHandle>,
    pub bake_preview_ao: Option<TextureHandle>,
    pub bake_preview_curv: Option<TextureHandle>,
    pub bake_preview_pos: Option<TextureHandle>,
    pub bake_preview_thick: Option<TextureHandle>,
    pub bake_success_message: Option<String>,
    pub icons: Option<crate::icons::AppIcons>,

    // Interactive Progressive Path Tracer (Iray Mode - F10)
    pub iray_mode: bool,
    pub path_tracer: Option<crate::path_tracer::PathTracer>,
    pub iray_texture: Option<TextureHandle>,
    pub last_iray_camera_view: Option<glam::Mat4>,
    pub last_iray_hdri_rot: f32,
    pub iray_max_samples: u32,
    pub iray_max_bounces: u32,
    pub iray_samples_per_frame: u32,
    pub iray_denoise: bool,
    pub last_iray_texture_revision: u64,

    // Custom Material Creation Modal state
    pub show_new_material_modal: bool,
    pub new_material_name: String,
    pub new_material_color: [f32; 4],
    pub new_material_roughness: f32,
    pub new_material_metallic: f32,
    pub new_material_height: f32,
    pub new_material_emissive: [f32; 3],
    pub new_material_opacity: f32,
    pub new_material_clearcoat: f32,
    pub new_material_anisotropy: f32,

    // Custom Decal / Path Preset Creation Modal state
    pub show_new_decal_modal: bool,
    pub new_decal_name: String,
    pub new_decal_description: String,
    pub need_cpu_composite_sync: bool,
}

#[derive(Clone, Debug)]
pub struct PerformanceMetrics {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub paint_stroke_ms: f32,
    pub gpu_sync_ms: f32,
    pub gpu_render_ms: f32,
    pub cmd_encoding_ms: f32,
    pub queue_submit_ms: f32,
    pub texture_transfer_bytes: u64,
    pub gpu_name: String,
    pub is_software_gpu: bool,
    pub is_cpu_fallback: bool,
    pub is_debug: bool,
    pub show_debug_hud: bool,
    pub is_idle: bool,
    pub work_latency_ms: f32,
    pub last_frame_time: std::time::Instant,
    pub frame_count: u32,
    pub fps_accum: f32,
    pub fps_update_timer: std::time::Instant,
    pub gpu_pbr_ms: f32,
    pub gpu_postprocess_ms: f32,
    pub gpu_compute_brush_ms: f32,
    pub gpu_compositing_ms: f32,
    pub gpu_total_hardware_ms: f32,
    pub hardware_timestamps_active: bool,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            fps: 60.0,
            frame_time_ms: 16.6,
            paint_stroke_ms: 0.0,
            gpu_sync_ms: 0.0,
            gpu_render_ms: 0.0,
            cmd_encoding_ms: 0.0,
            queue_submit_ms: 0.0,
            texture_transfer_bytes: 0,
            gpu_name: "Detecting...".to_string(),
            is_software_gpu: false,
            is_cpu_fallback: false,
            is_debug: cfg!(debug_assertions),
            show_debug_hud: false,
            is_idle: false,
            work_latency_ms: 0.6,
            last_frame_time: std::time::Instant::now(),
            frame_count: 0,
            fps_accum: 0.0,
            fps_update_timer: std::time::Instant::now(),
            gpu_pbr_ms: 0.0,
            gpu_postprocess_ms: 0.0,
            gpu_compute_brush_ms: 0.0,
            gpu_compositing_ms: 0.0,
            gpu_total_hardware_ms: 0.0,
            hardware_timestamps_active: false,
        }
    }
}

pub fn apply_substance_theme(ctx: &eframe::egui::Context) {
    let mut visuals = eframe::egui::Visuals::dark();
    visuals.window_fill = eframe::egui::Color32::from_rgb(26, 28, 32);
    visuals.panel_fill = eframe::egui::Color32::from_rgb(30, 32, 36);
    visuals.faint_bg_color = eframe::egui::Color32::from_rgb(22, 23, 26);
    visuals.extreme_bg_color = eframe::egui::Color32::from_rgb(18, 19, 22);
    visuals.window_stroke = eframe::egui::Stroke::new(1.0_f32, eframe::egui::Color32::from_rgb(46, 50, 58));
    
    // Selection (Substance Painter deep steel blue)
    visuals.selection.bg_fill = eframe::egui::Color32::from_rgba_unmultiplied(45, 120, 230, 110);
    visuals.selection.stroke = eframe::egui::Stroke::new(1.0_f32, eframe::egui::Color32::from_rgb(70, 150, 255));
    
    // Widgets styling
    visuals.widgets.noninteractive.bg_fill = eframe::egui::Color32::from_rgb(30, 32, 36);
    visuals.widgets.noninteractive.fg_stroke = eframe::egui::Stroke::new(1.0_f32, eframe::egui::Color32::from_rgb(175, 180, 192));
    visuals.widgets.inactive.bg_fill = eframe::egui::Color32::from_rgb(38, 41, 48);
    visuals.widgets.inactive.fg_stroke = eframe::egui::Stroke::new(1.0_f32, eframe::egui::Color32::from_rgb(205, 210, 220));
    visuals.widgets.inactive.rounding = eframe::egui::Rounding::same(2.5);
    visuals.widgets.hovered.bg_fill = eframe::egui::Color32::from_rgb(52, 56, 66);
    visuals.widgets.hovered.fg_stroke = eframe::egui::Stroke::new(1.0_f32, eframe::egui::Color32::from_rgb(255, 255, 255));
    visuals.widgets.hovered.rounding = eframe::egui::Rounding::same(2.5);
    visuals.widgets.active.bg_fill = eframe::egui::Color32::from_rgb(226, 91, 16);
    visuals.widgets.active.fg_stroke = eframe::egui::Stroke::new(1.0_f32, eframe::egui::Color32::WHITE);
    visuals.widgets.active.rounding = eframe::egui::Rounding::same(2.5);

    visuals.window_rounding = eframe::egui::Rounding::same(3.5);
    visuals.menu_rounding = eframe::egui::Rounding::same(2.5);
    visuals.window_shadow = eframe::egui::epaint::Shadow {
        offset: eframe::egui::vec2(0.0, 4.0),
        blur: 12.0,
        spread: 0.0,
        color: eframe::egui::Color32::from_black_alpha(150),
    };
    visuals.popup_shadow = eframe::egui::epaint::Shadow {
        offset: eframe::egui::vec2(0.0, 3.0),
        blur: 8.0,
        spread: 0.0,
        color: eframe::egui::Color32::from_black_alpha(140),
    };
    visuals.window_stroke = eframe::egui::Stroke::new(1.0_f32, eframe::egui::Color32::from_rgb(52, 56, 66));
    visuals.resize_corner_size = 9.0;

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = eframe::egui::vec2(5.0, 5.0);
    style.spacing.button_padding = eframe::egui::vec2(6.0, 3.5);
    style.spacing.window_margin = eframe::egui::Margin::same(6.0);
    ctx.set_style(style);
}

impl PaintcraftApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_substance_theme(&cc.egui_ctx);
        let default_tex_size = 512;
        let project = Project::new_default(default_tex_size, default_tex_size);
        let camera = Camera::default();
        let mut renderer = ViewportRenderer::new(760, 560);
        let brush_engine = BrushEngine::new();
        let mut history = HistoryManager::new();
        history.adjust_max_undo_for_resolution(default_tex_size, default_tex_size);
        let file_explorer = FileExplorerDialog::new_open_model();

        let asset_manager = crate::asset_manager::AssetManager::new();
        // Load the first available built-in HDRI from the assets folder directly
        if let Some(first_hdri) = asset_manager.hdris.first() {
            let _ = renderer.hdri.load_file(&first_hdri.path);
        }

        let wgpu_viewport = if let Some(render_state) = &cc.wgpu_render_state {
            let mut vp = crate::wgpu_renderer::WgpuViewport::new(render_state, &project.mesh);
            let (w, h, hdri_data) = crate::hdri_generator::generate_hdri_texture(&renderer.hdri, 1024, 512);
            vp.load_environment(w, h, &hdri_data);
            Some(vp)
        } else {
            None
        };

        let gpu_name = if let Some(vp) = &wgpu_viewport {
            if let Some(info) = &vp.adapter_info {
                format!("{} ({:?})", info.name, info.backend)
            } else {
                "Hardware WGPU Active".to_string()
            }
        } else {
            "Software CPU Rasterizer (NO GPU)".to_string()
        };
        let is_software_gpu = if let Some(vp) = &wgpu_viewport {
            if let Some(info) = &vp.adapter_info {
                info.device_type == eframe::wgpu::DeviceType::Cpu
            } else {
                false
            }
        } else {
            true
        };
        let is_cpu_fallback = wgpu_viewport.is_none();

        Self {
            project,
            camera,
            renderer,
            brush_engine,
            history,
            file_explorer,
            is_stroke_active: false,
            gpu_stroke_dirty_rect: None,
            view_mode: ViewportChannelMode::Material,
            layout: ViewLayout::View3DOnly,
            viewport_texture: None,
            uv_canvas_texture: None,
            uv_pixel_buffer: Vec::new(),
            status_message: "Ready. Select a tool or material to begin painting in 3D.".to_string(),
            export_preset: ExportPreset::UnrealEngine,
            export_image_format: crate::packing::PackedImageFormat::Png,
            last_stylus_pressure: 1.0,
            export_dir: {
                let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                dir.push("exports");
                dir.to_string_lossy().to_string()
            },
            export_flip_v: true,
            export_mesh: false,
            export_tab_idx: 0,
            custom_packing_presets: crate::packing::CustomPackingPreset::default_presets(),
            active_custom_packing_idx: 0,
            custom_packing_status: None,
            custom_preset_io_filename: "my_custom_preset.json".to_string(),
            export_template_search: String::new(),
            export_selected_ts_idx: 0,
            export_last_exported_files: Vec::new(),
            export_resolution_override: 0,
            export_padding_px: 16,
            export_udim_naming: false,
            project_file_path: {
                let mut p = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                p.push("project.paintcraft");
                p.to_string_lossy().to_string()
            },
            show_about_modal: false,
            show_export_modal: false,
            show_save_modal: false,
            show_bake_modal: false,
            show_viewport_settings_modal: false,
            show_shader_settings_modal: false,
            show_filter_modal: false,
            filter_type_idx: 0,
            filter_target_mask: false,
            filter_blur_radius: 3.0,
            filter_levels_in_black: 0.0,
            filter_levels_in_white: 1.0,
            filter_levels_gamma: 1.0,
            filter_levels_out_black: 0.0,
            filter_levels_out_white: 1.0,
            filter_sharpen_strength: 1.0,
            filter_contrast: 1.2,
            filter_brightness: 0.0,
            filter_high_pass_radius: 2.0,
            bake_highpoly_path: String::new(),
            show_new_project_modal: true,
            new_proj_template: 0,
            new_proj_mesh_type: 0,
            new_proj_primitive_idx: 0,
            new_proj_custom_path: String::new(),
            new_proj_resolution: 2048,
            new_proj_normal_format: 0,
            new_proj_auto_bake: false,
            new_proj_bake_res: 1024,
            dragged_layer_idx: None,
            renaming_layer_idx: None,
            rename_buffer: String::new(),
            delete_key_held: false,
            shader_settings: crate::wgpu_renderer::ShaderSettings::default(),
            viewport_bg_color: [0.086, 0.090, 0.102],
            camera_orbit_speed: 1.0,
            camera_pan_speed: 1.0,
            camera_zoom_speed: 1.0,
            project_name_edit: "Untitled Project".to_string(),
            new_project_width: default_tex_size,
            new_project_height: default_tex_size,
            last_painted_uv: None,
            bake_res: 2048,
            bake_frontal: 0.1,
            bake_rear: 0.1,
            bake_opt_tangent_normal: true,
            bake_opt_world_normal: true,
            bake_opt_ao: true,
            bake_opt_curvature: true,
            bake_opt_position: true,
            bake_opt_thickness: true,
            bake_opt_thickness_invert: false,
            bake_opt_id_map: true,
            bake_denoise: false,
            bake_denoise_iterations: 1,
            bake_match_mode: crate::baker::BakeMatchMode::Always,
            bake_low_suffix: "_low".to_string(),
            bake_high_suffix: "_high".to_string(),
            bake_cage_mode: crate::baker::BakeCageMode::None,
            bake_cage_inflation: 0.02,
            bake_cage_path: String::new(),
            bake_preview_cage: true,
            bake_cage_show_hull: true,
            bake_cage_show_tethers: true,
            bake_cage_color_idx: 0,
            bake_cage_opacity: 0.85,
            bake_cage_preview_rear: false,
            bake_cage_cache_key: BakeCageCacheKey::default(),
            bake_active_settings_tab: crate::baker::BakeSettingsTab::Common,
            bake_ao_config: crate::baker::AoBakeConfig::default(),
            bake_thickness_config: crate::baker::ThicknessBakeConfig::default(),
            bake_curvature_config: crate::baker::CurvatureBakeConfig::default(),
            bake_normal_config: crate::baker::NormalBakeConfig::default(),
            bake_position_config: crate::baker::PositionBakeConfig::default(),
            bg_blur: 0.0,
            bg_opacity: 1.0,
            layer_stack_channel: crate::channel::ChannelType::BaseColor,
            stencil_image_path: String::new(),
            stencil_overlay_texture: None,
            right_panel_tab: RightPanelTab::Layers,
            workspace: WorkspaceLayout::default(),
            show_shelf: true,
            shelf_category: crate::shelf::ShelfCategory::All,
            shelf_search_query: String::new(),
            shelf_picker: None,
            shelf_thumbnails: std::collections::HashMap::new(),
            shelf_thumbnails_dirty: true,
            shelf_resource_popup: None,
            asset_manager,
            wgpu_viewport,
            updater: crate::updater::UpdateNotifier::new(),
            uv_2d_pan: egui::Vec2::ZERO,
            uv_2d_zoom: 1.0,
            show_uv_wireframe: false,
            split_view_ratio: 0.5,
            uv_view_mask_mode: false,
            mask_preview_layer_id: None,
            recent_projects: Self::load_recent_projects(),
            zen_mode: false,
            continuous_60fps: true,
            shader_preset: crate::wgpu_renderer::ShaderPerformancePreset::DynamicInteraction,
            perf_metrics: PerformanceMetrics {
                gpu_name,
                is_software_gpu,
                is_cpu_fallback,
                ..Default::default()
            },
            last_hovered_hit_3d: None,
            last_hovered_uv_2d: None,
            last_hovered_tri: None,
            polygon_fill_drag_tris: std::collections::HashSet::new(),
            poly_fill_hover_cache: None,
            bake_worker_state: None,
            bake_receiver: None,
            bake_selected_texture_sets: Vec::new(),
            bake_preview_normal: None,
            bake_preview_ao: None,
            bake_preview_curv: None,
            bake_preview_pos: None,
            bake_preview_thick: None,
            bake_success_message: None,
            icons: None,
            iray_mode: false,
            path_tracer: Some(crate::path_tracer::PathTracer::new(512, 512)),
            iray_texture: None,
            last_iray_camera_view: None,
            last_iray_hdri_rot: 0.0,
            iray_max_samples: 256,
            iray_max_bounces: 2,
            iray_samples_per_frame: 1,
            iray_denoise: true,
            last_iray_texture_revision: 0,
            show_new_material_modal: false,
            new_material_name: "My Material".to_string(),
            new_material_color: [0.8, 0.8, 0.8, 1.0],
            new_material_roughness: 0.35,
            new_material_metallic: 0.0,
            new_material_height: 0.5,
            new_material_emissive: [0.0, 0.0, 0.0],
            new_material_opacity: 1.0,
            new_material_clearcoat: 0.0,
            new_material_anisotropy: 0.0,
            show_new_decal_modal: false,
            new_decal_name: "My Decal Preset".to_string(),
            new_decal_description: "Custom hardware decal preset".to_string(),
            need_cpu_composite_sync: false,
        }
    }

    /// Load recent projects from disk
    pub fn load_recent_projects() -> Vec<String> {
        let path = std::path::Path::new("recent_projects.json");
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(&data) {
                return list;
            }
        }
        Vec::new()
    }

    /// Save recent projects to disk
    pub fn save_recent_projects(&self) {
        let path = std::path::Path::new("recent_projects.json");
        if let Ok(json) = serde_json::to_string_pretty(&self.recent_projects) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Add a path to the recent projects list and persist
    pub fn add_recent_project(&mut self, path_str: &str) {
        if path_str.trim().is_empty() {
            return;
        }
        self.recent_projects.retain(|p| p != path_str);
        self.recent_projects.insert(0, path_str.to_string());
        if self.recent_projects.len() > 8 {
            self.recent_projects.truncate(8);
        }
        self.save_recent_projects();
    }

    /// Current color configured for baking cage visualization
    pub fn current_bake_cage_color(&self) -> [f32; 4] {
        match self.bake_cage_color_idx {
            0 => [0.0, 0.88, 1.0, 1.0],   // Vibrant Substance Cyan
            1 => [1.0, 0.45, 0.05, 1.0],  // Substance Orange
            2 => [0.2, 1.0, 0.35, 1.0],   // High-Vis Lime
            3 => [1.0, 0.15, 0.80, 1.0],  // Neon Magenta
            _ => [0.0, 0.88, 1.0, 1.0],
        }
    }

    /// Open a project file and synchronize GPU viewport
    pub fn open_project_file(&mut self, path: &std::path::Path) {
        match Project::load_from_file(path) {
            Ok(p) => {
                self.project = p;
                self.history.adjust_max_undo_for_resolution(self.project.texture_width, self.project.texture_height);
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    wgpu.update_mesh(&self.project.mesh);
                    wgpu.clear_gpu_layers();
                    wgpu.sync_textures(&mut self.project);
                    wgpu.sync_mesh_maps(&self.project);
                }
                self.camera.reset();
                self.project_file_path = path.to_string_lossy().to_string();
                self.add_recent_project(&self.project_file_path.clone());
                self.status_message = format!("Opened project from {}", self.project_file_path);
            }
            Err(e) => {
                self.status_message = format!("Open Error: {}", e);
            }
        }
    }

    /// Delete currently selected layer with undo history and GPU synchronization
    pub fn delete_selected_layer(&mut self, ctx: &egui::Context) {
        if self.project.layers.len() <= 1 {
            self.status_message = "Cannot delete the only remaining layer in the project".to_string();
            return;
        }
        let act_idx = self.project.active_layer_idx;
        if act_idx < self.project.layers.len() {
            let prev_active = self.project.active_layer_idx;
            let layer_id = self.project.layers[act_idx].id;
            if self.mask_preview_layer_id == Some(layer_id) {
                self.mask_preview_layer_id = None;
                self.uv_view_mask_mode = false;
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    wgpu.mask_preview_layer_id = None;
                }
            }
            if let Some(removed_layer) = self.project.delete_layer(act_idx) {
                let layer_name = removed_layer.name.clone();
                self.history.push("Delete Layer", HistoryAction::DeleteLayer {
                    layer_idx: act_idx,
                    layer: Box::new(removed_layer),
                    prev_active,
                });
                self.project.rebuild_composite();
                self.project.gpu_sync_needed = true;
                self.need_cpu_composite_sync = true;
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    wgpu.sync_textures(&mut self.project);
                }
                self.update_uv_texture(ctx);
                self.status_message = format!("Deleted layer '{}'", layer_name);
                ctx.request_repaint();
            }
        }
    }

    /// Toggle Alt + Left Click mask preview mode for a layer
    pub fn toggle_mask_preview(&mut self, layer_id: u64, ctx: &egui::Context) {
        if self.mask_preview_layer_id == Some(layer_id) {
            self.mask_preview_layer_id = None;
            self.uv_view_mask_mode = false;
            self.status_message = "Exited Mask Preview Mode".to_string();
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.mask_preview_layer_id = None;
                wgpu.sync_textures(&mut self.project);
            }
        } else {
            self.mask_preview_layer_id = Some(layer_id);
            self.uv_view_mask_mode = true;
            self.brush_engine.settings.target = PaintTarget::Mask;
            let name = self.project.layers.iter().find(|l| l.id == layer_id).map(|l| l.name.clone()).unwrap_or_default();
            self.status_message = format!("Mask Preview (Unlit): '{}' [Alt+Click mask or press M/Esc to return to Material]", name);
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.mask_preview_layer_id = Some(layer_id);
                wgpu.sync_mask_preview_full(&self.project, layer_id);
            }
        }
        self.project.gpu_sync_needed = true;
        self.need_cpu_composite_sync = true;
        self.update_uv_texture(ctx);
        ctx.request_repaint();
    }

    /// Exit mask preview mode and restore full material shading
    pub fn exit_mask_preview(&mut self, ctx: &egui::Context) {
        if self.mask_preview_layer_id.is_some() {
            self.mask_preview_layer_id = None;
            self.uv_view_mask_mode = false;
            self.status_message = "Exited Mask Preview Mode".to_string();
            self.project.gpu_sync_needed = true;
            self.need_cpu_composite_sync = true;
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.mask_preview_layer_id = None;
                wgpu.sync_textures(&mut self.project);
            }
            self.update_uv_texture(ctx);
            ctx.request_repaint();
        }
    }

    /// Robust layer selection: saves outgoing vector path state if switching from a Path layer,
    /// configures tool mode and active state for the incoming layer, and cleans up phantom previews.
    pub fn select_layer(&mut self, new_idx: usize, ctx: &egui::Context) {
        if new_idx >= self.project.layers.len() {
            return;
        }

        let old_idx = self.project.active_layer_idx;
        if old_idx < self.project.layers.len() && self.project.layers[old_idx].layer_type == LayerType::Path {
            self.project.layers[old_idx].path_state = Some(self.brush_engine.settings.path_tool.clone());
        }

        self.project.active_layer_idx = new_idx;
        let layer_type = self.project.layers[new_idx].layer_type;
        let layer_name = self.project.layers[new_idx].name.clone();

        match layer_type {
            LayerType::Path => {
                if let Some(ps) = &self.project.layers[new_idx].path_state {
                    self.brush_engine.settings.path_tool = ps.clone();
                } else {
                    self.brush_engine.settings.path_tool = crate::brush::PathToolState::default();
                    self.project.layers[new_idx].path_state = Some(self.brush_engine.settings.path_tool.clone());
                }
                self.brush_engine.settings.mode = ToolMode::Path;
                self.brush_engine.settings.target = PaintTarget::Material;
                self.status_message = format!("Selected Vector Path Layer '{}' (Paths live & editable)", layer_name);
            }
            LayerType::Paint => {
                if self.brush_engine.settings.mode == ToolMode::Path {
                    self.brush_engine.settings.mode = ToolMode::Brush;
                }
                self.brush_engine.settings.path_tool.clear();
                self.brush_engine.settings.target = PaintTarget::Material;
                self.status_message = format!("Selected Paint Layer '{}'", layer_name);
            }
            LayerType::Fill => {
                if self.brush_engine.settings.mode == ToolMode::Path {
                    self.brush_engine.settings.mode = ToolMode::Brush;
                }
                self.brush_engine.settings.path_tool.clear();
                self.brush_engine.settings.target = PaintTarget::Material;
                self.status_message = format!("Selected Fill Layer '{}'", layer_name);
            }
            LayerType::Folder => {
                if self.brush_engine.settings.mode == ToolMode::Path {
                    self.brush_engine.settings.mode = ToolMode::Brush;
                }
                self.brush_engine.settings.path_tool.clear();
                self.status_message = format!("Selected Folder '{}'", layer_name);
            }
        }

        if self.mask_preview_layer_id.is_some() {
            self.exit_mask_preview(ctx);
        }

        ctx.request_repaint();
    }

    /// Syncs vector path state into active Path layer, re-evaluates all spline decals non-destructively, and updates viewport textures
    pub fn sync_and_evaluate_active_path_layer(&mut self, ctx: &egui::Context) {
        let act_idx = self.project.active_layer_idx;
        if act_idx < self.project.layers.len() && self.project.layers[act_idx].layer_type == LayerType::Path {
            self.project.layers[act_idx].path_state = Some(self.brush_engine.settings.path_tool.clone());
            self.project.layers[act_idx].evaluate_path_layer(&self.project.mesh, &mut self.brush_engine);
            self.project.rebuild_composite();
            self.project.gpu_sync_needed = true;
            self.update_uv_texture(ctx);
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.sync_textures(&mut self.project);
                if let Some(preview_id) = self.mask_preview_layer_id {
                    wgpu.sync_mask_preview_full(&self.project, preview_id);
                }
            }
            self.project.is_dirty = true;
            ctx.request_repaint();
        }
    }

    /// Commit all stamps along the Catmull-Rom spline path into the active layer or mask
    pub fn commit_path_tool(&mut self, ctx: &egui::Context) {
        if self.brush_engine.settings.path_tool.is_empty() {
            return;
        }

        let act_idx = self.project.active_layer_idx;
        if act_idx >= self.project.layers.len() {
            return;
        }

        if self.project.layers[act_idx].layer_type == LayerType::Path {
            self.sync_and_evaluate_active_path_layer(ctx);
            let style_label = self.brush_engine.settings.path_tool.stroke_style.name();
            self.status_message = format!("{}: path updated on Path Layer (always editable)", style_label);
            return;
        }

        let is_fill = self.project.layers[act_idx].layer_type != LayerType::Paint;
        let is_mask = self.brush_engine.settings.target == PaintTarget::Mask;
        if is_fill && !is_mask {
            self.status_message = "Cannot paint material on Fill or Folder layer. Switch target to Mask or add a Paint or Path layer.".to_string();
            return;
        }

        let doc_w = self.project.texture_width;
        let world_rad = ((self.brush_engine.settings.radius / (doc_w as f32).max(1.0)) * 2.0).clamp(0.005, 0.5);
        let mut grouped_samples = self.brush_engine.settings.path_tool.sample_all_paths_grouped(world_rad);
        if grouped_samples.is_empty() {
            return;
        }

        if self.brush_engine.settings.path_tool.snap_to_surface {
            for samples in &mut grouped_samples {
                crate::brush::PathToolState::project_samples_to_mesh(
                    samples,
                    &self.project.mesh,
                    world_rad,
                    self.brush_engine.settings.path_tool.surface_offset,
                );
            }
        }

        self.flush_pending_gpu_readbacks(ctx);
        if let Some(wgpu) = &mut self.wgpu_viewport {
            if wgpu.gpu_layers.len() != self.project.layers.len() {
                wgpu.ensure_gpu_layers(&mut self.project);
            }
        }

        let target = self.brush_engine.settings.target;
        let channels = if target == PaintTarget::Mask {
            crate::channel::ChannelFlags::none()
        } else {
            let b = self.brush_engine.settings.channels;
            let l = self.project.layers[act_idx].channel_flags;
            crate::channel::ChannelFlags {
                base_color: b.base_color && l.base_color,
                roughness: b.roughness && l.roughness,
                metallic: b.metallic && l.metallic,
                height: b.height && l.height,
                normal: (b.normal || b.height) && (l.normal || l.height),
                emission: b.emission && l.emission,
                opacity: b.opacity && l.opacity,
                clearcoat: b.clearcoat && l.clearcoat,
                anisotropy: b.anisotropy && l.anisotropy,
                ao: false,
            }
        };

        self.project.layers[act_idx].begin_stroke_recording(target, channels);

        let orig_radius = self.brush_engine.settings.radius;
        let orig_follow_path = self.brush_engine.settings.follow_path;
        let orig_follow_angle = self.brush_engine.follow_path_angle;

        if self.brush_engine.settings.path_tool.follow_tangent {
            self.brush_engine.settings.follow_path = true;
        }

        let mesh = self.project.mesh.clone();
        let doc_h = self.project.texture_height;

        let stroke_style = self.brush_engine.settings.path_tool.stroke_style;
        let decal_type = self.brush_engine.settings.path_tool.decal_type;
        let heat_tint = self.brush_engine.settings.path_tool.heat_tint;
        let height_depth = self.brush_engine.settings.path_tool.height_depth;
        let mut total_stamps = 0;

        for samples in grouped_samples {
            self.brush_engine.last_hit_pos = None;
            self.brush_engine.last_hit_normal = None;
            self.brush_engine.last_uv = None;

            for (i, sample) in samples.iter().enumerate() {
                total_stamps += 1;
                self.brush_engine.settings.radius = (orig_radius * sample.radius_scale).clamp(1.0, 300.0);
                if self.brush_engine.settings.path_tool.follow_tangent {
                    self.brush_engine.follow_path_angle = sample.tangent_angle;
                }

                let sample_world_rad = ((self.brush_engine.settings.radius / (doc_w as f32).max(1.0)) * 2.0).max(0.005);
                let layer = &mut self.project.layers[act_idx];

                if stroke_style == crate::brush::PathStrokeStyle::Continuous {
                    self.brush_engine.paint_stroke_3d(layer, &mesh, sample.position_3d, sample.normal, sample_world_rad);
                } else {
                    let next_idx = if i + 1 < samples.len() { i + 1 } else { i };
                    let prev_idx = if i > 0 { i - 1 } else { i };
                    let tan_3d = if next_idx != prev_idx {
                        samples[next_idx].position_3d - samples[prev_idx].position_3d
                    } else if i + 1 < samples.len() {
                        samples[i + 1].position_3d - sample.position_3d
                    } else if i > 0 {
                        sample.position_3d - samples[i - 1].position_3d
                    } else {
                        glam::Vec3::X
                    };

                    self.brush_engine.stamp_3d_decal(
                        layer,
                        &mesh,
                        sample.position_3d,
                        sample.normal,
                        tan_3d,
                        sample_world_rad,
                        stroke_style,
                        decal_type,
                        heat_tint,
                        sample.arc_length,
                        height_depth,
                    );
                }
            }
        }

        if stroke_style != crate::brush::PathStrokeStyle::Continuous {
            let layer = &mut self.project.layers[act_idx];
            layer.update_normal_from_height(1.0);
        }

        self.brush_engine.settings.radius = orig_radius;
        self.brush_engine.settings.follow_path = orig_follow_path;
        self.brush_engine.follow_path_angle = orig_follow_angle;
        self.brush_engine.last_hit_pos = None;
        self.brush_engine.last_hit_normal = None;
        self.brush_engine.last_uv = None;

        let style_label = stroke_style.name();
        let desc = format!("{} Path ({} stamps)", style_label, total_stamps);
        if let Some(layer) = self.project.layers.get_mut(act_idx) {
            if let Some(action) = layer.finish_stroke_delta(act_idx, &desc) {
                self.history.push(&desc, action);
            }
            if let Some(d_rect) = layer.dirty_rect {
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    if let Some(gpu_layer) = wgpu.gpu_layers.get(act_idx) {
                        wgpu.gpu_compositor.update_layer(layer, gpu_layer, doc_w, doc_h);
                        wgpu.composite_and_update_active_rect(&self.project, act_idx, Some(d_rect));
                    }
                }
                self.update_uv_texture_rect(ctx, d_rect.0, d_rect.1, d_rect.2, d_rect.3);
            } else {
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    wgpu.sync_textures(&mut self.project);
                }
                self.update_uv_texture(ctx);
            }
        }

        self.brush_engine.settings.path_tool.clear();
        self.status_message = format!("{}: {} stamps committed", style_label, total_stamps);
        self.project.is_dirty = true;
        ctx.request_repaint();
    }

    /// Update 3D viewport texture in egui
    fn update_viewport_texture(&mut self, ctx: &egui::Context) {
        let eff_mode = if self.mask_preview_layer_id.is_some() {
            ViewportChannelMode::BaseColor
        } else {
            self.view_mode
        };
        self.renderer.render(
            &self.camera,
            &self.project.mesh,
            &self.project.composite_textures,
            eff_mode,
            self.project.thickness_map.as_ref(),
        );

        let w = self.renderer.width as usize;
        let h = self.renderer.height as usize;
        let color_image = ColorImage::from_rgba_unmultiplied([w, h], &self.renderer.color_buffer);

        if let Some(tex) = &mut self.viewport_texture {
            tex.set(color_image, TextureOptions::LINEAR);
        } else {
            self.viewport_texture = Some(ctx.load_texture("viewport_3d", color_image, TextureOptions::LINEAR));
        }
    }

    /// Update 2D UV canvas texture in egui
    fn update_uv_texture(&mut self, ctx: &egui::Context) {
        if self.layout == ViewLayout::View3DOnly {
            return;
        }

        let w = self.project.texture_width as usize;
        let h = self.project.texture_height as usize;
        let total_bytes = w * h * 4;
        if self.uv_pixel_buffer.len() != total_bytes {
            self.uv_pixel_buffer.resize(total_bytes, 0);
        }
        let pixels = &mut self.uv_pixel_buffer;

        use rayon::prelude::*;

        let view_mode = self.view_mode;
        let is_mask_mode = self.uv_view_mask_mode;
        let active_mask_buf = if is_mask_mode {
            if let Some(preview_id) = self.mask_preview_layer_id {
                self.project.layers.iter().find(|l| l.id == preview_id && l.has_mask).map(|l| l.mask_buf.clone())
            } else {
                self.project.active_layer().and_then(|l| if l.has_mask { Some(l.mask_buf.clone()) } else { None })
            }
        } else {
            None
        };
        let base_col = &self.project.composite_textures.base_color;
        let rough = &self.project.composite_textures.roughness;
        let metal = &self.project.composite_textures.metallic;
        let normal = &self.project.composite_textures.normal;
        let height_map = &self.project.composite_textures.height_map;
        let emis_map = &self.project.composite_textures.emissive;
        let opac_map = &self.project.composite_textures.opacity;
        let cc_map = &self.project.composite_textures.clearcoat;
        let aniso_map = &self.project.composite_textures.anisotropy;
        let aniso_ang_map = &self.project.composite_textures.anisotropy_angle;
        let ao_map = self.project.ao_map.as_ref();
        let curv_map = self.project.curvature_map.as_ref();
        let pos_map = self.project.position_map.as_ref();
        let wn_map = self.project.world_space_normal_map.as_ref();
        let thick_map = self.project.thickness_map.as_ref();

        pixels.par_chunks_exact_mut(w * 4).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                let idx = x * 4;
                if let Some(mask) = &active_mask_buf {
                    let m = (mask.get_pixel(x as u32, y as u32).clamp(0.0, 1.0) * 255.0) as u8;
                    row[idx] = m;
                    row[idx + 1] = m;
                    row[idx + 2] = m;
                    row[idx + 3] = 255;
                } else {
                    match view_mode {
                        ViewportChannelMode::Material | ViewportChannelMode::BaseColor => {
                            let col = base_col.get_pixel(x as u32, y as u32);
                            row[idx] = (col.x.clamp(0.0, 1.0) * 255.0) as u8;
                            row[idx + 1] = (col.y.clamp(0.0, 1.0) * 255.0) as u8;
                            row[idx + 2] = (col.z.clamp(0.0, 1.0) * 255.0) as u8;
                            row[idx + 3] = (col.w.clamp(0.0, 1.0) * 255.0) as u8;
                        }
                    ViewportChannelMode::Roughness => {
                        let r = (rough.get_pixel(x as u32, y as u32).clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx] = r; row[idx + 1] = r; row[idx + 2] = r; row[idx + 3] = 255;
                    }
                    ViewportChannelMode::Metallic => {
                        let m = (metal.get_pixel(x as u32, y as u32).clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx] = m; row[idx + 1] = m; row[idx + 2] = m; row[idx + 3] = 255;
                    }
                    ViewportChannelMode::Normal => {
                        let n = normal.get_pixel(x as u32, y as u32);
                        row[idx] = (n.x.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 1] = (n.y.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 2] = (n.z.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 3] = 255;
                    }
                    ViewportChannelMode::Height => {
                        let h = (height_map.get_pixel(x as u32, y as u32).clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx] = h; row[idx + 1] = h; row[idx + 2] = h; row[idx + 3] = 255;
                    }
                    ViewportChannelMode::AmbientOcclusion => {
                        let val = if let Some(ref ao_buf) = ao_map {
                            (ao_buf.get_pixel(x as u32, y as u32).clamp(0.0, 1.0) * 255.0) as u8
                        } else {
                            255
                        };
                        row[idx] = val; row[idx + 1] = val; row[idx + 2] = val; row[idx + 3] = 255;
                    }
                    ViewportChannelMode::Curvature => {
                        let val = if let Some(ref curv_buf) = curv_map {
                            (curv_buf.get_pixel(x as u32, y as u32).clamp(0.0, 1.0) * 255.0) as u8
                        } else {
                            128
                        };
                        row[idx] = val; row[idx + 1] = val; row[idx + 2] = val; row[idx + 3] = 255;
                    }
                    ViewportChannelMode::Position => {
                        let col = if let Some(ref p_buf) = pos_map {
                            p_buf.get_pixel(x as u32, y as u32)
                        } else {
                            glam::Vec4::new(0.5, 0.5, 0.5, 1.0)
                        };
                        row[idx] = (col.x.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 1] = (col.y.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 2] = (col.z.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 3] = 255;
                    }
                    ViewportChannelMode::WorldNormal => {
                        let col = if let Some(ref wn_buf) = wn_map {
                            wn_buf.get_pixel(x as u32, y as u32)
                        } else {
                            glam::Vec4::new(0.5, 0.5, 1.0, 1.0)
                        };
                        row[idx] = (col.x.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 1] = (col.y.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 2] = (col.z.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 3] = 255;
                    }
                    ViewportChannelMode::Thickness => {
                        let val = if let Some(ref th_buf) = thick_map {
                            (th_buf.get_pixel(x as u32, y as u32).clamp(0.0, 1.0) * 255.0) as u8
                        } else {
                            255
                        };
                        row[idx] = val; row[idx + 1] = val; row[idx + 2] = val; row[idx + 3] = 255;
                    }
                    ViewportChannelMode::Emissive => {
                        let col = emis_map.get_pixel(x as u32, y as u32);
                        row[idx] = (col.x.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 1] = (col.y.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 2] = (col.z.clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx + 3] = 255;
                    }
                    ViewportChannelMode::Opacity => {
                        let o = (opac_map.get_pixel(x as u32, y as u32).clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx] = o; row[idx + 1] = o; row[idx + 2] = o; row[idx + 3] = 255;
                    }
                    ViewportChannelMode::Clearcoat => {
                        let c = (cc_map.get_pixel(x as u32, y as u32).clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx] = c; row[idx + 1] = c; row[idx + 2] = c; row[idx + 3] = 255;
                    }
                    ViewportChannelMode::Anisotropy => {
                        let a = ((aniso_map.get_pixel(x as u32, y as u32) * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                        let ang = (aniso_ang_map.get_pixel(x as u32, y as u32).clamp(0.0, 1.0) * 255.0) as u8;
                        row[idx] = a; row[idx + 1] = ang; row[idx + 2] = 0; row[idx + 3] = 255;
                    }
                }
                }
            }
        });

        let color_image = ColorImage::from_rgba_unmultiplied([w, h], pixels);
        if let Some(tex) = &mut self.uv_canvas_texture {
            tex.set(color_image, TextureOptions::LINEAR);
        } else {
            self.uv_canvas_texture = Some(ctx.load_texture("uv_canvas_2d", color_image, TextureOptions::LINEAR));
        }
    }

    /// Fast sub-rectangle update of the 2D UV canvas texture in egui using set_partial
    fn update_uv_texture_rect(&mut self, ctx: &egui::Context, min_x: u32, min_y: u32, max_x: u32, max_y: u32) {
        if self.layout == ViewLayout::View3DOnly {
            return;
        }
        if self.uv_canvas_texture.is_none() {
            self.update_uv_texture(ctx);
            return;
        }
        let w = self.project.texture_width;
        let h = self.project.texture_height;
        if min_x >= w || min_y >= h {
            return;
        }
        let max_x = max_x.min(w - 1);
        let max_y = max_y.min(h - 1);
        let rw = (max_x - min_x + 1) as usize;
        let rh = (max_y - min_y + 1) as usize;
        if rw == 0 || rh == 0 {
            return;
        }

        let is_mask_mode = self.uv_view_mask_mode;
        let active_mask_buf = if is_mask_mode {
            if let Some(preview_id) = self.mask_preview_layer_id {
                self.project.layers.iter().find(|l| l.id == preview_id && l.has_mask).map(|l| l.mask_buf.clone())
            } else {
                self.project.active_layer().and_then(|l| if l.has_mask { Some(l.mask_buf.clone()) } else { None })
            }
        } else {
            None
        };
        let base_col = &self.project.composite_textures.base_color;
        let rough = &self.project.composite_textures.roughness;
        let metal = &self.project.composite_textures.metallic;
        let normal = &self.project.composite_textures.normal;
        let height_map = &self.project.composite_textures.height_map;
        let emis_map = &self.project.composite_textures.emissive;
        let opac_map = &self.project.composite_textures.opacity;
        let cc_map = &self.project.composite_textures.clearcoat;
        let aniso_map = &self.project.composite_textures.anisotropy;
        let aniso_ang_map = &self.project.composite_textures.anisotropy_angle;
        let ao_map = self.project.ao_map.as_ref();
        let curv_map = self.project.curvature_map.as_ref();
        let pos_map = self.project.position_map.as_ref();
        let wn_map = self.project.world_space_normal_map.as_ref();
        let thick_map = self.project.thickness_map.as_ref();
        let view_mode = self.view_mode;

        let mut sub_pixels = vec![0u8; rw * rh * 4];
        for local_y in 0..rh {
            let y = min_y + local_y as u32;
            let row_offset = local_y * rw * 4;
            for local_x in 0..rw {
                let x = min_x + local_x as u32;
                let idx = row_offset + local_x * 4;

                if let Some(mask) = &active_mask_buf {
                    let m = (mask.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                    sub_pixels[idx] = m;
                    sub_pixels[idx + 1] = m;
                    sub_pixels[idx + 2] = m;
                    sub_pixels[idx + 3] = 255;
                } else {
                    match view_mode {
                        ViewportChannelMode::Material | ViewportChannelMode::BaseColor => {
                            let col = base_col.get_pixel(x, y);
                            sub_pixels[idx] = (col.x.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 1] = (col.y.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 2] = (col.z.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 3] = (col.w.clamp(0.0, 1.0) * 255.0) as u8;
                        }
                        ViewportChannelMode::Roughness => {
                            let r = (rough.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx] = r; sub_pixels[idx + 1] = r; sub_pixels[idx + 2] = r; sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::Metallic => {
                            let m = (metal.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx] = m; sub_pixels[idx + 1] = m; sub_pixels[idx + 2] = m; sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::Normal => {
                            let n = normal.get_pixel(x, y);
                            sub_pixels[idx] = (n.x.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 1] = (n.y.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 2] = (n.z.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::Height => {
                            let h = (height_map.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx] = h; sub_pixels[idx + 1] = h; sub_pixels[idx + 2] = h; sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::AmbientOcclusion => {
                            let val = if let Some(ref ao_buf) = ao_map {
                                (ao_buf.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8
                            } else {
                                255
                            };
                            sub_pixels[idx] = val; sub_pixels[idx + 1] = val; sub_pixels[idx + 2] = val; sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::Curvature => {
                            let val = if let Some(ref curv_buf) = curv_map {
                                (curv_buf.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8
                            } else {
                                128
                            };
                            sub_pixels[idx] = val; sub_pixels[idx + 1] = val; sub_pixels[idx + 2] = val; sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::Position => {
                            let col = if let Some(ref p_buf) = pos_map {
                                p_buf.get_pixel(x, y)
                            } else {
                                glam::Vec4::new(0.5, 0.5, 0.5, 1.0)
                            };
                            sub_pixels[idx] = (col.x.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 1] = (col.y.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 2] = (col.z.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::WorldNormal => {
                            let col = if let Some(ref wn_buf) = wn_map {
                                wn_buf.get_pixel(x, y)
                            } else {
                                glam::Vec4::new(0.5, 0.5, 1.0, 1.0)
                            };
                            sub_pixels[idx] = (col.x.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 1] = (col.y.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 2] = (col.z.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::Thickness => {
                            let val = if let Some(ref th_buf) = thick_map {
                                (th_buf.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8
                            } else {
                                255
                            };
                            sub_pixels[idx] = val; sub_pixels[idx + 1] = val; sub_pixels[idx + 2] = val; sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::Emissive => {
                            let col = emis_map.get_pixel(x, y);
                            sub_pixels[idx] = (col.x.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 1] = (col.y.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 2] = (col.z.clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::Opacity => {
                            let o = (opac_map.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx] = o; sub_pixels[idx + 1] = o; sub_pixels[idx + 2] = o; sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::Clearcoat => {
                            let c = (cc_map.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx] = c; sub_pixels[idx + 1] = c; sub_pixels[idx + 2] = c; sub_pixels[idx + 3] = 255;
                        }
                        ViewportChannelMode::Anisotropy => {
                            let a = ((aniso_map.get_pixel(x, y) * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                            let ang = (aniso_ang_map.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                            sub_pixels[idx] = a; sub_pixels[idx + 1] = ang; sub_pixels[idx + 2] = 0; sub_pixels[idx + 3] = 255;
                        }
                    }
                }
            }
        }

        let color_image = ColorImage::from_rgba_unmultiplied([rw, rh], &sub_pixels);
        if let Some(tex) = &mut self.uv_canvas_texture {
            tex.set_partial([min_x as usize, min_y as usize], color_image, TextureOptions::LINEAR);
        }
    }

    /// Create small preview thumbnail texture for baked RGBA maps
    pub fn create_rgba_thumbnail(ctx: &egui::Context, id: &str, buf: &crate::channel::RgbaBuffer, thumb_w: u32, thumb_h: u32) -> TextureHandle {
        let mut pixels = vec![0u8; (thumb_w * thumb_h * 4) as usize];
        for y in 0..thumb_h {
            let v = y as f32 / thumb_h as f32;
            for x in 0..thumb_w {
                let u = x as f32 / thumb_w as f32;
                let col = buf.sample_bilinear(glam::Vec2::new(u, v));
                let idx = ((y * thumb_w + x) * 4) as usize;
                pixels[idx] = (col.x.clamp(0.0, 1.0) * 255.0) as u8;
                pixels[idx + 1] = (col.y.clamp(0.0, 1.0) * 255.0) as u8;
                pixels[idx + 2] = (col.z.clamp(0.0, 1.0) * 255.0) as u8;
                pixels[idx + 3] = 255;
            }
        }
        let img = ColorImage::from_rgba_unmultiplied([thumb_w as usize, thumb_h as usize], &pixels);
        ctx.load_texture(id, img, TextureOptions::LINEAR)
    }

    /// Create small preview thumbnail texture for baked scalar maps
    pub fn create_scalar_thumbnail(ctx: &egui::Context, id: &str, buf: &crate::channel::ScalarBuffer, thumb_w: u32, thumb_h: u32) -> TextureHandle {
        let mut pixels = vec![0u8; (thumb_w * thumb_h * 4) as usize];
        for y in 0..thumb_h {
            let v = y as f32 / thumb_h as f32;
            for x in 0..thumb_w {
                let u = x as f32 / thumb_w as f32;
                let val = buf.sample_bilinear(glam::Vec2::new(u, v));
                let byte_val = (val.clamp(0.0, 1.0) * 255.0) as u8;
                let idx = ((y * thumb_w + x) * 4) as usize;
                pixels[idx] = byte_val;
                pixels[idx + 1] = byte_val;
                pixels[idx + 2] = byte_val;
                pixels[idx + 3] = 255;
            }
        }
        let img = ColorImage::from_rgba_unmultiplied([thumb_w as usize, thumb_h as usize], &pixels);
        ctx.load_texture(id, img, TextureOptions::LINEAR)
    }

    /// Refresh cached egui thumbnails for all project shelf resources
    pub fn refresh_shelf_thumbnails(&mut self, ctx: &egui::Context) {
        if !self.shelf_thumbnails_dirty {
            return;
        }
        for res in &self.project.resources {
            if !self.shelf_thumbnails.contains_key(&res.id) {
                if let Some(rgba) = &res.rgba_data {
                    let thumb = Self::create_rgba_thumbnail(ctx, &format!("thumb_{}", res.id), rgba, 64, 64);
                    self.shelf_thumbnails.insert(res.id.clone(), thumb);
                } else if let Some(scalar) = &res.scalar_data {
                    let thumb = Self::create_scalar_thumbnail(ctx, &format!("thumb_{}", res.id), scalar, 64, 64);
                    self.shelf_thumbnails.insert(res.id.clone(), thumb);
                }
            }
        }
        self.shelf_thumbnails_dirty = false;
    }

    /// Assign a project shelf resource to a target channel, tool, or environment slot
    pub fn assign_resource_to_target(&mut self, res: &crate::project::ProjectTextureResource, target: ShelfPickerTarget) {
        match target {
            ShelfPickerTarget::FillBaseColor => {
                if let Some(rgba) = res.to_rgba_buffer() {
                    if let Some(layer) = self.project.layers.get_mut(self.project.active_layer_idx) {
                        layer.fill_color_tex = Some(rgba);
                        layer.evaluate_fill(self.project.curvature_map.as_ref());
                        self.project.rebuild_composite();
                        self.project.gpu_sync_needed = true;
                        self.status_message = format!("Assigned '{}' to Base Color", res.name);
                    }
                }
            }
            ShelfPickerTarget::FillRoughness => {
                if let Some(scalar) = res.to_scalar_buffer() {
                    if let Some(layer) = self.project.layers.get_mut(self.project.active_layer_idx) {
                        layer.fill_roughness_tex = Some(scalar);
                        layer.evaluate_fill(self.project.curvature_map.as_ref());
                        self.project.rebuild_composite();
                        self.project.gpu_sync_needed = true;
                        self.status_message = format!("Assigned '{}' to Roughness", res.name);
                    }
                }
            }
            ShelfPickerTarget::FillMetallic => {
                if let Some(scalar) = res.to_scalar_buffer() {
                    if let Some(layer) = self.project.layers.get_mut(self.project.active_layer_idx) {
                        layer.fill_metallic_tex = Some(scalar);
                        layer.evaluate_fill(self.project.curvature_map.as_ref());
                        self.project.rebuild_composite();
                        self.project.gpu_sync_needed = true;
                        self.status_message = format!("Assigned '{}' to Metallic", res.name);
                    }
                }
            }
            ShelfPickerTarget::FillNormal => {
                if let Some(rgba) = res.to_rgba_buffer() {
                    if let Some(layer) = self.project.layers.get_mut(self.project.active_layer_idx) {
                        layer.fill_normal_tex = Some(rgba);
                        layer.evaluate_fill(self.project.curvature_map.as_ref());
                        self.project.rebuild_composite();
                        self.project.gpu_sync_needed = true;
                        self.status_message = format!("Assigned '{}' to Normal", res.name);
                    }
                }
            }
            ShelfPickerTarget::FillHeight => {
                if let Some(scalar) = res.to_scalar_buffer() {
                    if let Some(layer) = self.project.layers.get_mut(self.project.active_layer_idx) {
                        layer.fill_height_tex = Some(scalar);
                        layer.evaluate_fill(self.project.curvature_map.as_ref());
                        self.project.rebuild_composite();
                        self.project.gpu_sync_needed = true;
                        self.status_message = format!("Assigned '{}' to Height", res.name);
                    }
                }
            }
            ShelfPickerTarget::FillEmissive => {
                if let Some(rgba) = res.to_rgba_buffer() {
                    if let Some(layer) = self.project.layers.get_mut(self.project.active_layer_idx) {
                        layer.fill_emissive_tex = Some(rgba);
                        layer.evaluate_fill(self.project.curvature_map.as_ref());
                        self.project.rebuild_composite();
                        self.project.gpu_sync_needed = true;
                        self.status_message = format!("Assigned '{}' to Emissive", res.name);
                    }
                }
            }
            ShelfPickerTarget::FillOpacity => {
                if let Some(scalar) = res.to_scalar_buffer() {
                    if let Some(layer) = self.project.layers.get_mut(self.project.active_layer_idx) {
                        layer.fill_opacity_tex = Some(scalar);
                        layer.evaluate_fill(self.project.curvature_map.as_ref());
                        self.project.rebuild_composite();
                        self.project.gpu_sync_needed = true;
                        self.status_message = format!("Assigned '{}' to Opacity", res.name);
                    }
                }
            }
            ShelfPickerTarget::Stencil => {
                if let Some(rgba) = res.to_rgba_buffer() {
                    self.brush_engine.stencil_buffer = Some(rgba);
                    self.status_message = format!("Assigned '{}' to Stencil", res.name);
                }
            }
            ShelfPickerTarget::BrushAlpha => {
                self.brush_engine.settings.alpha = AlphaType::NoiseSplatter;
                self.status_message = format!("Set Alpha to '{}'", res.name);
            }
            ShelfPickerTarget::MeshMapTangentNormal => {
                if let Some(rgba) = res.to_rgba_buffer() {
                    self.project.world_normal_map = Some(rgba);
                    self.project.gpu_mesh_maps_sync_needed = true;
                    self.project.gpu_sync_needed = true;
                    self.project.sync_baked_maps_to_resources();
                    self.status_message = format!("Assigned '{}' as Tangent Normal Map", res.name);
                }
            }
            ShelfPickerTarget::MeshMapWorldSpaceNormal => {
                if let Some(rgba) = res.to_rgba_buffer() {
                    self.project.world_space_normal_map = Some(rgba);
                    self.project.gpu_mesh_maps_sync_needed = true;
                    self.project.gpu_sync_needed = true;
                    self.project.sync_baked_maps_to_resources();
                    self.status_message = format!("Assigned '{}' as World Space Normal Map", res.name);
                }
            }
            ShelfPickerTarget::MeshMapAo => {
                if let Some(scalar) = res.to_scalar_buffer() {
                    self.project.ao_map = Some(scalar);
                    self.project.gpu_mesh_maps_sync_needed = true;
                    self.project.gpu_sync_needed = true;
                    self.project.sync_baked_maps_to_resources();
                    self.status_message = format!("Assigned '{}' as Ambient Occlusion Map", res.name);
                }
            }
            ShelfPickerTarget::MeshMapCurvature => {
                if let Some(scalar) = res.to_scalar_buffer() {
                    self.project.curvature_map = Some(scalar);
                    self.project.gpu_mesh_maps_sync_needed = true;
                    self.project.gpu_sync_needed = true;
                    self.project.sync_baked_maps_to_resources();
                    self.status_message = format!("Assigned '{}' as Curvature Map", res.name);
                }
            }
            ShelfPickerTarget::MeshMapPosition => {
                if let Some(rgba) = res.to_rgba_buffer() {
                    self.project.position_map = Some(rgba);
                    self.project.gpu_mesh_maps_sync_needed = true;
                    self.project.gpu_sync_needed = true;
                    self.project.sync_baked_maps_to_resources();
                    self.status_message = format!("Assigned '{}' as Position Map", res.name);
                }
            }
            ShelfPickerTarget::MeshMapThickness => {
                if let Some(scalar) = res.to_scalar_buffer() {
                    self.project.thickness_map = Some(scalar);
                    self.project.gpu_mesh_maps_sync_needed = true;
                    self.project.gpu_sync_needed = true;
                    self.project.sync_baked_maps_to_resources();
                    self.status_message = format!("Assigned '{}' as Thickness Map", res.name);
                }
            }
            ShelfPickerTarget::MeshMapId => {
                if let Some(rgba) = res.to_rgba_buffer() {
                    self.project.id_map = Some(rgba);
                    self.project.gpu_mesh_maps_sync_needed = true;
                    self.project.sync_baked_maps_to_resources();
                    self.status_message = format!("Assigned '{}' as ID Map", res.name);
                }
            }
            ShelfPickerTarget::HdriEnvironment => {
                if let Some(path_str) = &res.file_path {
                    let p = std::path::Path::new(path_str);
                    self.activate_hdri_file(p);
                }
            }
        }
    }

    /// Assign a procedural generator from the shelf to a target slot
    pub fn assign_noise_to_target(&mut self, gen: crate::layer::ProceduralGenerator, target: ShelfPickerTarget) {
        match target {
            ShelfPickerTarget::FillBaseColor
            | ShelfPickerTarget::FillRoughness
            | ShelfPickerTarget::FillMetallic
            | ShelfPickerTarget::FillNormal
            | ShelfPickerTarget::FillHeight
            | ShelfPickerTarget::FillEmissive
            | ShelfPickerTarget::FillOpacity => {
                if let Some(layer) = self.project.layers.get_mut(self.project.active_layer_idx) {
                    layer.fill_generator = gen;
                    layer.evaluate_fill(self.project.curvature_map.as_ref());
                    self.project.rebuild_composite();
                    self.project.gpu_sync_needed = true;
                    self.status_message = format!("Assigned procedural '{}' to fill layer", gen.name());
                }
            }
            ShelfPickerTarget::Stencil => {
                let w = 512;
                let h = 512;
                let mut buf = crate::channel::RgbaBuffer::new(w, h, Vec4::ONE);
                for y in 0..h {
                    for x in 0..w {
                        let u = x as f32 / w as f32;
                        let v = y as f32 / h as f32;
                        let val = match gen {
                            crate::layer::ProceduralGenerator::WorleyGrunge => {
                                let (d1, d2) = crate::utils::NoiseGenerator::voronoi_2d(u * 8.0, v * 8.0);
                                (d2 - d1).clamp(0.0, 1.0)
                            }
                            crate::layer::ProceduralGenerator::Scratches => {
                                crate::utils::NoiseGenerator::scratches_2d(u * 6.0, v * 6.0)
                            }
                            crate::layer::ProceduralGenerator::GrungeMap => {
                                crate::utils::NoiseGenerator::grunge_map_2d(u * 6.0, v * 6.0)
                            }
                            crate::layer::ProceduralGenerator::Checkerboard => {
                                let cu = (u * 8.0).floor() as i32;
                                let cv = (v * 8.0).floor() as i32;
                                if (cu + cv) % 2 == 0 { 1.0 } else { 0.0 }
                            }
                            _ => crate::utils::NoiseGenerator::fbm_perlin_2d(u * 6.0, v * 6.0, 4, 2.0, 0.5),
                        };
                        buf.set_pixel(x, y, Vec4::new(val, val, val, 1.0));
                    }
                }
                self.brush_engine.stencil_buffer = Some(buf);
                self.status_message = format!("Generated stencil preset: '{}'", gen.name());
            }
            _ => {}
        }
    }
}

impl PaintcraftApp {
    /// Cycle through material channels (Substance standard shortcut 'C' / 'Shift+C')
    pub fn cycle_material_channel(&mut self, forward: bool) {
        let order = [
            ViewportChannelMode::Material,
            ViewportChannelMode::BaseColor,
            ViewportChannelMode::Roughness,
            ViewportChannelMode::Metallic,
            ViewportChannelMode::Normal,
            ViewportChannelMode::Height,
            ViewportChannelMode::AmbientOcclusion,
            ViewportChannelMode::Emissive,
            ViewportChannelMode::Opacity,
            ViewportChannelMode::Clearcoat,
            ViewportChannelMode::Anisotropy,
        ];
        let cur_idx = order.iter().position(|&m| m == self.view_mode).unwrap_or(0);
        let next_idx = if forward {
            (cur_idx + 1) % order.len()
        } else {
            (cur_idx + order.len() - 1) % order.len()
        };
        self.view_mode = order[next_idx];
        self.status_message = format!("Channel View: {} [C / Shift+C]", self.view_mode.name());
    }

    /// Check if an asynchronous staging readback is currently in flight
    pub fn has_pending_gpu_readback(&self) -> bool {
        self.wgpu_viewport.as_ref().map(|w| w.has_pending_readback()).unwrap_or(false)
    }

    /// Non-blocking poll for completed asynchronous staging readback.
    /// Updates CPU layer, tile undo history, and sub-composite without stalling the main render thread.
    pub fn poll_pending_gpu_readbacks(&mut self, ctx: &egui::Context) {
        if let Some(wgpu) = &mut self.wgpu_viewport {
            if let Some(res) = wgpu.poll_async_readback(&mut self.project) {
                let x0 = res.min_x;
                let y0 = res.min_y;
                let x1 = x0 + res.rect_w.saturating_sub(1);
                let y1 = y0 + res.rect_h.saturating_sub(1);
                if let Some(layer) = self.project.layers.get_mut(res.layer_idx) {
                    if let Some(action) = layer.finish_stroke_delta(res.layer_idx, &res.description) {
                        self.history.push(&res.description, action);
                    }
                }
                self.project.rebuild_composite_rect(x0 as usize, y0 as usize, x1 as usize, y1 as usize);
                self.update_uv_texture_rect(ctx, x0, y0, x1, y1);
            }
        }
    }

    /// Flush and wait for pending asynchronous staging readback to complete immediately.
    /// Used before Undo, Redo, Save, Export, or starting a new stroke.
    pub fn flush_pending_gpu_readbacks(&mut self, ctx: &egui::Context) {
        if let Some(wgpu) = &mut self.wgpu_viewport {
            if wgpu.has_pending_readback() {
                if let Some(res) = wgpu.flush_async_readback(&mut self.project) {
                    let x0 = res.min_x;
                    let y0 = res.min_y;
                    let x1 = x0 + res.rect_w.saturating_sub(1);
                    let y1 = y0 + res.rect_h.saturating_sub(1);
                    if let Some(layer) = self.project.layers.get_mut(res.layer_idx) {
                        if let Some(action) = layer.finish_stroke_delta(res.layer_idx, &res.description) {
                            self.history.push(&res.description, action);
                        }
                    }
                    self.project.rebuild_composite_rect(x0 as usize, y0 as usize, x1 as usize, y1 as usize);
                    self.update_uv_texture_rect(ctx, x0, y0, x1, y1);
                }
            }
        }
    }

    /// Cycle through baked mesh maps (Substance standard shortcut 'B' / 'Shift+B')
    pub fn cycle_mesh_map(&mut self, forward: bool) {
        let order = [
            ViewportChannelMode::Normal,
            ViewportChannelMode::WorldNormal,
            ViewportChannelMode::AmbientOcclusion,
            ViewportChannelMode::Curvature,
            ViewportChannelMode::Position,
            ViewportChannelMode::Thickness,
        ];
        let cur_idx = order.iter().position(|&m| m == self.view_mode).unwrap_or(0);
        let next_idx = if forward {
            (cur_idx + 1) % order.len()
        } else {
            (cur_idx + order.len() - 1) % order.len()
        };
        self.view_mode = order[next_idx];
        self.status_message = format!("Mesh Map View: {} [B / Shift+B]", self.view_mode.name());
    }

    /// Legacy alias for channel cycling
    #[allow(dead_code)]
    pub fn cycle_channel_view(&mut self) {
        self.cycle_material_channel(true);
    }

    /// Create project from the Substance 3D Painter New Project Startup Dialog
    pub fn create_project_from_dialog(&mut self) {
        let mesh = if self.new_proj_mesh_type == 1 {
            let path = std::path::Path::new(&self.new_proj_custom_path);
            if path.exists() {
                match crate::mesh::Mesh::load_file(path) {
                    Ok(m) => m,
                    Err(e) => {
                        self.status_message = format!("Failed to load Mesh: {}. Using Shader Ball instead.", e);
                        crate::mesh::Mesh::create_shader_ball()
                    }
                }
            } else {
                self.status_message = "Custom mesh file not found. Using Shader Ball instead.".to_string();
                crate::mesh::Mesh::create_shader_ball()
            }
        } else {
            match self.new_proj_primitive_idx {
                0 => crate::mesh::Mesh::create_shader_ball(),
                1 => crate::mesh::Mesh::create_rounded_cube(),
                2 => crate::mesh::Mesh::create_scifi_crate(),
                3 => crate::mesh::Mesh::create_robot_head(),
                4 => crate::mesh::Mesh::create_cylinder(),
                5 => crate::mesh::Mesh::create_torus(),
                _ => crate::mesh::Mesh::create_shader_ball(),
            }
        };

        let res = self.new_proj_resolution;
        let mut proj = Project::new_default(res, res);
        proj.name = if self.project_name_edit.trim().is_empty() {
            mesh.name.clone()
        } else {
            self.project_name_edit.trim().to_string()
        };
        proj.set_mesh(mesh);

        self.shader_settings.normal_format = self.new_proj_normal_format;

        if self.new_proj_auto_bake {
            proj.bake_mesh_maps(self.new_proj_bake_res, 0.1, 0.1);
        }

        self.project = proj;
        self.camera.reset();
        self.history.clear();

        if let Some(wgpu) = &mut self.wgpu_viewport {
            wgpu.update_mesh(&self.project.mesh);
            wgpu.clear_gpu_layers();
            wgpu.sync_textures(&mut self.project);
            wgpu.sync_mesh_maps(&self.project);
        }

        self.show_new_project_modal = false;
        self.status_message = format!("Created project '{}' ({}x{}) with {} tris.", self.project.mesh.name, res, res, self.project.mesh.indices.len() / 3);
    }
}

/// Substance 3D Painter iconic channel toggle pill button
fn channel_pill_button(ui: &mut egui::Ui, name: &str, active: bool) -> egui::Response {
    let text = egui::RichText::new(name)
        .size(11.0)
        .color(if active { Color32::WHITE } else { Color32::from_rgb(140, 145, 155) })
        .strong();
    let bg_color = if active {
        Color32::from_rgb(226, 91, 16) // Substance iconic amber
    } else {
        Color32::from_rgb(34, 36, 42) // Dark muted graphite
    };
    let stroke_color = if active {
        Color32::from_rgb(255, 125, 45)
    } else {
        Color32::from_rgb(52, 55, 64)
    };

    let btn = egui::Button::new(text)
        .fill(bg_color)
        .stroke(egui::Stroke::new(1.0_f32, stroke_color))
        .rounding(3.0);
    ui.add(btn)
}

impl eframe::App for PaintcraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll pending non-blocking staging readbacks
        self.poll_pending_gpu_readbacks(ctx);
        if self.has_pending_gpu_readback() {
            ctx.request_repaint();
        }

        // Track frame delta and FPS
        let frame_start = std::time::Instant::now();
        let delta = frame_start.duration_since(self.perf_metrics.last_frame_time).as_secs_f32();
        self.perf_metrics.last_frame_time = frame_start;

        let is_idle_delta = delta > 0.05;
        self.perf_metrics.is_idle = is_idle_delta && !self.is_stroke_active;

        if !is_idle_delta {
            self.perf_metrics.frame_count += 1;
            self.perf_metrics.fps_accum += 1.0 / delta.max(0.001);
        }

        if self.perf_metrics.fps_update_timer.elapsed().as_millis() >= 250 {
            if self.perf_metrics.frame_count > 0 {
                self.perf_metrics.fps = (self.perf_metrics.fps_accum / self.perf_metrics.frame_count as f32).clamp(1.0, 999.0);
                self.perf_metrics.frame_time_ms = (1000.0 / self.perf_metrics.fps).clamp(1.0, 999.0);
            } else if !self.perf_metrics.is_idle {
                self.perf_metrics.fps = 60.0;
                self.perf_metrics.frame_time_ms = 16.67;
            }
            self.perf_metrics.frame_count = 0;
            self.perf_metrics.fps_accum = 0.0;
            self.perf_metrics.fps_update_timer = std::time::Instant::now();
        }

        // Non-intrusive background check for GitHub updates on startup
        if !self.updater.auto_checked_startup {
            self.updater.auto_checked_startup = true;
            self.updater.check_for_updates(false);
        }

        // Initialize UI icons
        if self.icons.is_none() {
            self.icons = Some(crate::icons::AppIcons::load(ctx));
        }

        // --- POLL BACKGROUND BAKE WORKER ---
        if let Some(rx) = &self.bake_receiver {
            match rx.try_recv() {
                Ok(res) => {
                    if let Some(wn) = res.tangent_normal {
                        self.bake_preview_normal = Some(Self::create_rgba_thumbnail(ctx, "prev_wn", &wn, 64, 64));
                        self.project.world_normal_map = Some(wn);
                    }
                    if let Some(wsn) = res.world_space_normal {
                        self.project.world_space_normal_map = Some(wsn);
                    }
                    if let Some(ao) = res.ao {
                        self.bake_preview_ao = Some(Self::create_scalar_thumbnail(ctx, "prev_ao", &ao, 64, 64));
                        self.project.ao_map = Some(ao);
                    }
                    if let Some(curv) = res.curvature {
                        self.bake_preview_curv = Some(Self::create_scalar_thumbnail(ctx, "prev_curv", &curv, 64, 64));
                        self.project.curvature_map = Some(curv);
                    }
                    if let Some(pos) = res.position {
                        self.bake_preview_pos = Some(Self::create_rgba_thumbnail(ctx, "prev_pos", &pos, 64, 64));
                        self.project.position_map = Some(pos);
                    }
                    if let Some(th) = res.thickness {
                        self.bake_preview_thick = Some(Self::create_scalar_thumbnail(ctx, "prev_th", &th, 64, 64));
                        self.project.thickness_map = Some(th);
                    }
                    if let Some(id_m) = res.id_map {
                        self.project.id_map = Some(id_m);
                    }

                    self.project.sync_baked_maps_to_resources();
                    self.shelf_thumbnails_dirty = true;
                    self.refresh_shelf_thumbnails(ctx);
                    self.project.gpu_mesh_maps_sync_needed = true;
                    for layer in &mut self.project.layers {
                        layer.evaluate_fill_advanced(
                            self.project.curvature_map.as_ref(),
                            self.project.position_map.as_ref(),
                            self.project.world_normal_map.as_ref(),
                            self.project.ao_map.as_ref(),
                        );
                    }
                    self.project.rebuild_composite();
                    self.update_uv_texture(ctx);
                    if let Some(wgpu) = &mut self.wgpu_viewport {
                        wgpu.sync_mesh_maps(&self.project);
                        wgpu.sync_textures(&mut self.project);
                    }
                    self.bake_success_message = Some(format!("Baking completed successfully in {:.2}s!", res.elapsed_secs));
                    self.status_message = format!("Mesh maps baked in {:.2}s", res.elapsed_secs);
                    self.bake_receiver = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if self.bake_worker_state.as_ref().map(|s| !s.lock().unwrap().is_done).unwrap_or(false) {
                        ctx.request_repaint();
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.bake_receiver = None;
                    if let Some(state) = &self.bake_worker_state {
                        let mut st = state.lock().unwrap();
                        st.is_done = true;
                        st.step_name = "Bake finished or interrupted".to_string();
                    }
                    self.status_message = "Bake worker finished".to_string();
                }
            }
        }

        // --- GLOBAL KEYBOARD SHORTCUTS ---
        let input = ctx.input(|i| i.clone());
        let has_cmd = input.modifiers.command || input.modifiers.ctrl || input.modifiers.mac_cmd;
        let has_alt = input.modifiers.alt;

        let is_delete_down = input.key_down(egui::Key::Delete) || input.key_down(egui::Key::Backspace);
        let delete_pressed = is_delete_down && !self.delete_key_held;
        self.delete_key_held = is_delete_down;

        // When user is typing into text edits or modals, bypass single-key shortcuts so inputs never clash
        if !ctx.wants_keyboard_input() {
            // Substance 3D Painter Standard Viewport Shortcuts (F1: Split, F2: 3D, F3: 2D)
            if input.key_pressed(egui::Key::F1) {
                self.layout = ViewLayout::SplitDualView;
                self.status_message = "Viewport Layout: 3D | 2D Split View [F1]".to_string();
            }
            if input.key_pressed(egui::Key::F2) {
                self.layout = ViewLayout::View3DOnly;
                self.status_message = "Viewport Layout: 3D Viewport Only [F2]".to_string();
            }
            if input.key_pressed(egui::Key::F3) {
                self.layout = ViewLayout::View2DOnly;
                self.status_message = "Viewport Layout: 2D UV Canvas Only [F3]".to_string();
            }

            // Performance Diagnostic HUD Toggle: F12
            if input.key_pressed(egui::Key::F12) {
                self.perf_metrics.show_debug_hud = !self.perf_metrics.show_debug_hud;
                self.status_message = format!("Performance Diagnostic HUD: {} [F12]", if self.perf_metrics.show_debug_hud { "ON" } else { "OFF" });
            }

            // Wireframe Overlay Toggle: F4
            if input.key_pressed(egui::Key::F4) {
                self.renderer.show_wireframe = !self.renderer.show_wireframe;
                self.show_uv_wireframe = self.renderer.show_wireframe;
                self.status_message = format!("Wireframe Overlay: {}", if self.renderer.show_wireframe { "ON" } else { "OFF" });
            }

            // Progressive Path Tracer Viewport (Iray Mode): F10
            if input.key_pressed(egui::Key::F10) {
                self.iray_mode = !self.iray_mode;
                if self.iray_mode {
                    if let Some(pt) = &mut self.path_tracer {
                        pt.reset();
                    }
                    self.status_message = "Iray Mode: Path Tracer Active [F10 or Esc to Exit]".to_string();
                } else {
                    self.status_message = "Iray Mode: Exited to Real-Time Viewport".to_string();
                }
            }

            // Escape exits Mask Preview mode or Iray mode
            if input.key_pressed(egui::Key::Escape) {
                if self.mask_preview_layer_id.is_some() {
                    self.exit_mask_preview(ctx);
                } else if self.iray_mode {
                    self.iray_mode = false;
                    self.status_message = "Iray Mode: Exited to Real-Time Viewport".to_string();
                }
            }

            // Delete selected layer or mask effect: Delete or Backspace key (edge-triggered debounce)
            if self.renaming_layer_idx.is_none() && delete_pressed {
                ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Delete);
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace);
                });
                let act_idx = self.project.active_layer_idx;
                let mut eff_removed = false;
                if self.brush_engine.settings.target == PaintTarget::Mask && act_idx < self.project.layers.len() {
                    let layer = &mut self.project.layers[act_idx];
                    if let Some(eff_id) = layer.active_mask_effect_id {
                        layer.remove_mask_effect(eff_id, self.project.curvature_map.as_ref());
                        let curv = self.project.curvature_map.as_ref();
                        let pos = self.project.position_map.as_ref();
                        let wn = self.project.world_space_normal_map.as_ref();
                        let ao = self.project.ao_map.as_ref();
                        let id_ref = self.project.id_map.as_ref();
                        layer.evaluate_mask_full_with_mesh(curv, ao, pos, wn, id_ref, &[], Some(&self.project.mesh));
                        self.status_message = "Removed selected mask effect".to_string();
                        eff_removed = true;
                    }
                }
                if eff_removed {
                    self.project.rebuild_composite();
                    self.project.gpu_sync_needed = true;
                    if let Some(preview_id) = self.mask_preview_layer_id {
                        if let Some(wgpu) = &mut self.wgpu_viewport {
                            wgpu.sync_mask_preview_full(&self.project, preview_id);
                        }
                    }
                    self.update_uv_texture(ctx);
                    ctx.request_repaint();
                } else {
                    self.delete_selected_layer(ctx);
                }
            }

            // Undo: Ctrl+Z
            if has_cmd && input.key_pressed(egui::Key::Z) && !input.modifiers.shift {
                self.flush_pending_gpu_readbacks(ctx);
                if let Some(desc) = self.history.undo(&mut self.project) {
                    self.status_message = format!("Undo: {}", desc);
                    self.project.gpu_sync_needed = true;
                    if let Some(wgpu) = &mut self.wgpu_viewport {
                        wgpu.sync_textures(&mut self.project);
                    }
                    ctx.request_repaint();
                }
            }
            // Redo: Ctrl+Y or Ctrl+Shift+Z
            if (has_cmd && input.key_pressed(egui::Key::Y))
                || (has_cmd && input.modifiers.shift && input.key_pressed(egui::Key::Z))
            {
                self.flush_pending_gpu_readbacks(ctx);
                if let Some(desc) = self.history.redo(&mut self.project) {
                    self.status_message = format!("Redo: {}", desc);
                    self.project.gpu_sync_needed = true;
                    if let Some(wgpu) = &mut self.wgpu_viewport {
                        wgpu.sync_textures(&mut self.project);
                    }
                    ctx.request_repaint();
                }
            }

            // Zen Mode: Tab (Hides all panels for immersive fullscreen painting)
            if input.key_pressed(egui::Key::Tab) {
                self.zen_mode = !self.zen_mode;
                self.status_message = if self.zen_mode {
                    "Zen Mode Enabled [Tab to Exit]".to_string()
                } else {
                    "Zen Mode Disabled".to_string()
                };
            }

            // Layer Duplicate: Ctrl+D
            if has_cmd && input.key_pressed(egui::Key::D) {
                let act_idx = self.project.active_layer_idx;
                if let Some(new_idx) = self.project.duplicate_layer(act_idx) {
                    let layer_clone = Box::new(self.project.layers[new_idx].clone());
                    self.history.push("Duplicate Layer", HistoryAction::AddLayer {
                        layer_idx: new_idx,
                        layer: layer_clone,
                        prev_active: act_idx,
                    });
                    self.status_message = format!("Duplicated layer #{} [Ctrl+D]", new_idx);
                }
            }

            // New Project: Ctrl+N
            if has_cmd && input.key_pressed(egui::Key::N) {
                self.show_new_project_modal = true;
            }

            // Bake Mesh Maps: Ctrl+Shift+B
            if has_cmd && input.modifiers.shift && input.key_pressed(egui::Key::B) {
                self.show_bake_modal = true;
            }

            // Export Textures: Ctrl+E
            if has_cmd && input.key_pressed(egui::Key::E) {
                self.show_export_modal = true;
            }

            // Save As: Ctrl+Shift+S
            if has_cmd && input.modifiers.shift && input.key_pressed(egui::Key::S) {
                self.show_save_modal = true;
            }
            // Quick Save: Ctrl+S
            else if has_cmd && input.key_pressed(egui::Key::S) {
                let path = PathBuf::from(&self.project_file_path);
                if let Err(e) = self.project.save_to_file(&path) {
                    self.status_message = format!("Save Error: {}", e);
                } else {
                    self.add_recent_project(&self.project_file_path.clone());
                    self.status_message = format!("Project saved to {}", self.project_file_path);
                }
            }

            // Tools and View modes (single-key shortcuts without Ctrl / Alt modifiers)
            if !has_cmd && !has_alt {
                if input.key_pressed(egui::Key::Num1) {
                    self.brush_engine.settings.mode = ToolMode::Brush;
                    self.status_message = "Tool: Paint Brush [1]".to_string();
                }
                if input.key_pressed(egui::Key::Num2) || input.key_pressed(egui::Key::E) {
                    self.brush_engine.settings.mode = ToolMode::Eraser;
                    self.status_message = "Tool: Eraser [2/E]".to_string();
                }
                if input.key_pressed(egui::Key::Num3) {
                    self.brush_engine.settings.mode = ToolMode::Projection;
                    self.brush_engine.settings.stencil.enabled = true;
                    if self.brush_engine.stencil_buffer.is_none() {
                        let mut buf = crate::channel::RgbaBuffer::new(512, 512, glam::Vec4::new(1.0, 1.0, 1.0, 1.0));
                        for y in 0..512 {
                            for x in 0..512 {
                                let u = x as f32 / 512.0;
                                let v = y as f32 / 512.0;
                                let val = crate::utils::NoiseGenerator::grunge_map_2d(u * 6.0, v * 6.0);
                                buf.set_pixel(x, y, glam::Vec4::new(val, val, val, 1.0));
                            }
                        }
                        self.brush_engine.stencil_buffer = Some(buf);
                        self.stencil_overlay_texture = None;
                    }
                    self.status_message = "Tool: Projection Painting [3] (Hold S: LMB=Rotate, RMB=Scale, MMB=Pan, S+Z=Reset)".to_string();
                }
                if input.key_pressed(egui::Key::Num4) || input.key_pressed(egui::Key::G) {
                    self.brush_engine.settings.mode = ToolMode::FillBucket;
                    self.status_message = "Tool: Polygon Fill / Bucket [4/G]".to_string();
                }
                if input.key_pressed(egui::Key::Num5) {
                    self.brush_engine.settings.mode = ToolMode::Smudge;
                    self.status_message = "Tool: Smudge [5]".to_string();
                }
                if input.key_pressed(egui::Key::Num6) {
                    self.brush_engine.settings.mode = ToolMode::Clone;
                    self.status_message = "Tool: Clone [6]".to_string();
                }
                if input.key_pressed(egui::Key::P) || input.key_pressed(egui::Key::I) {
                    self.brush_engine.settings.mode = ToolMode::Eyedropper;
                    self.status_message = "Tool: Material Picker [P/I]".to_string();
                }
                if input.key_pressed(egui::Key::Num7) || input.key_pressed(egui::Key::T) {
                    self.brush_engine.settings.mode = ToolMode::Path;
                    self.status_message = "Tool: Path Tool [7/T] (Click mesh to place points, Enter to commit, Esc to clear)".to_string();
                }
                if input.key_pressed(egui::Key::Num8) || input.key_pressed(egui::Key::D) {
                    self.brush_engine.settings.mode = ToolMode::Stamp;
                    self.status_message = "Tool: Decal Stamp Tool [8/D] (Click to stamp decal, rotate with slider or bracket keys)".to_string();
                }

                // Shift+M = Toggle Target between Layer Material and Mask
                // M = View Mode: Lit Material (PBR) and exit mask preview
                if input.modifiers.shift && input.key_pressed(egui::Key::M) {
                    let act_idx = self.project.active_layer_idx;
                    let layer_info = self.project.layers.get(act_idx).map(|l| (l.has_mask, l.name.clone()));
                    if let Some((has_mask, layer_name)) = layer_info {
                        if has_mask {
                            if self.brush_engine.settings.target == PaintTarget::Mask {
                                self.brush_engine.settings.target = PaintTarget::Material;
                                if self.mask_preview_layer_id.is_some() {
                                    self.exit_mask_preview(ctx);
                                }
                                self.status_message = format!("Target: Material (Layer '{}') [Shift+M]", layer_name);
                            } else {
                                self.brush_engine.settings.target = PaintTarget::Mask;
                                self.status_message = format!("Target: Mask (Layer '{}') [Shift+M]", layer_name);
                            }
                            ctx.request_repaint();
                        } else {
                            self.status_message = format!("Layer '{}' does not have a mask", layer_name);
                        }
                    }
                } else if input.key_pressed(egui::Key::M) {
                    if self.mask_preview_layer_id.is_some() {
                        self.exit_mask_preview(ctx);
                    }
                    self.view_mode = ViewportChannelMode::Material;
                    self.status_message = "View Mode: Lit Material (PBR) [M]".to_string();
                }
                // C / Shift+C = Material Channels
                if input.key_pressed(egui::Key::C) {
                    if self.mask_preview_layer_id.is_some() {
                        self.exit_mask_preview(ctx);
                    }
                    self.cycle_material_channel(!input.modifiers.shift);
                }
                // B / Shift+B = Baked Mesh Maps
                if input.key_pressed(egui::Key::B) {
                    if self.mask_preview_layer_id.is_some() {
                        self.exit_mask_preview(ctx);
                    }
                    self.cycle_mesh_map(!input.modifiers.shift);
                }

                // Mask Color Swap: X
                if input.key_pressed(egui::Key::X) {
                    self.brush_engine.settings.mask_paint_value =
                        if self.brush_engine.settings.mask_paint_value > 0.5 { 0.0 } else { 1.0 };
                    self.status_message = format!("Mask Paint Value: {:.1} [X]", self.brush_engine.settings.mask_paint_value);
                }

                // Brush Radius: [ and ]
                if input.key_pressed(egui::Key::OpenBracket) {
                    self.brush_engine.settings.radius = (self.brush_engine.settings.radius - 4.0).max(2.0);
                }
                if input.key_pressed(egui::Key::CloseBracket) {
                    self.brush_engine.settings.radius = (self.brush_engine.settings.radius + 4.0).min(200.0);
                }
            }
        }

        // --- TOP MENU BAR ---
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Project... (Ctrl+N)").clicked() {
                        self.show_new_project_modal = true;
                        ui.close_menu();
                    }
                    if ui.button("Open Project (.paintcraft)... (Ctrl+O)").clicked() {
                        self.file_explorer = FileExplorerDialog::new_open_project();
                        self.file_explorer.open();
                        ui.close_menu();
                    }
                    if ui.button("Import 3D Model (OBJ, glTF, PLY, STL)...").clicked() {
                        self.file_explorer = FileExplorerDialog::new_open_model();
                        self.file_explorer.open();
                        ui.close_menu();
                    }
                    ui.menu_button("Open Recent", |ui| {
                        if self.recent_projects.is_empty() {
                            ui.label(egui::RichText::new("No recent projects").small().color(Color32::GRAY));
                        } else {
                            let mut opened_path = None;
                            for p in &self.recent_projects {
                                let path_obj = std::path::Path::new(p);
                                let name = path_obj.file_name().and_then(|n| n.to_str()).unwrap_or(p);
                                if ui.button(name).on_hover_text(p).clicked() {
                                    opened_path = Some(p.clone());
                                    ui.close_menu();
                                }
                            }
                            if let Some(path_to_open) = opened_path {
                                self.open_project_file(std::path::Path::new(&path_to_open));
                            }
                            ui.separator();
                            if ui.button("Clear Recent Projects").clicked() {
                                self.recent_projects.clear();
                                self.save_recent_projects();
                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();
                    if ui.button("Save Project (Ctrl+S)").clicked() {
                        self.flush_pending_gpu_readbacks(ctx);
                        let path = PathBuf::from(&self.project_file_path);
                        if let Err(e) = self.project.save_to_file(&path) {
                            self.status_message = format!("Save Error: {}", e);
                        } else {
                            self.add_recent_project(&self.project_file_path.clone());
                            self.status_message = format!("Project saved to {}", self.project_file_path);
                        }
                        ui.close_menu();
                    }
                    if ui.button("Save Project As...").clicked() {
                        self.flush_pending_gpu_readbacks(ctx);
                        self.show_save_modal = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Export Textures... (Ctrl+E)").clicked() {
                        self.flush_pending_gpu_readbacks(ctx);
                        self.show_export_modal = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(self.history.can_undo(), egui::Button::new("Undo (Ctrl+Z)")).clicked() {
                        self.flush_pending_gpu_readbacks(ctx);
                        if let Some(desc) = self.history.undo(&mut self.project) {
                            self.status_message = format!("Undo: {}", desc);
                            self.project.gpu_sync_needed = true;
                            if let Some(wgpu) = &mut self.wgpu_viewport {
                                wgpu.sync_textures(&mut self.project);
                            }
                            ctx.request_repaint();
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.history.can_redo(), egui::Button::new("Redo (Ctrl+Y)")).clicked() {
                        self.flush_pending_gpu_readbacks(ctx);
                        if let Some(desc) = self.history.redo(&mut self.project) {
                            self.status_message = format!("Redo: {}", desc);
                            self.project.gpu_sync_needed = true;
                            if let Some(wgpu) = &mut self.wgpu_viewport {
                                wgpu.sync_textures(&mut self.project);
                            }
                            ctx.request_repaint();
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Bake Mesh Maps... (Ctrl+Shift+B)").clicked() {
                        self.show_bake_modal = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Reset Camera (F)").clicked() {
                        self.camera.reset();
                        ui.close_menu();
                    }
                    let is_ortho = self.camera.is_orthographic;
                    let ortho_label = if is_ortho { "Switch to Perspective Projection" } else { "Switch to Orthographic Projection" };
                    if ui.button(ortho_label).clicked() {
                        self.camera.is_orthographic = !is_ortho;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Viewport & Camera Settings...").clicked() {
                        self.show_viewport_settings_modal = true;
                        ui.close_menu();
                    }
                    if ui.button("Shader & Lighting Settings...").clicked() {
                        self.show_shader_settings_modal = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Select HDRI from Shelf...").clicked() {
                        self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::HdriEnvironment));
                        ui.close_menu();
                    }
                    if ui.button("Import HDRI to Library...").clicked() {
                        ui.close_menu();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("HDRI Environments", &["hdr", "png", "jpg", "jpeg", "exr"])
                            .set_title("Import HDRI to assets/hdris")
                            .pick_file() 
                        {
                            if let Ok(entry) = self.asset_manager.import_hdri_file(&path) {
                                if let Ok(()) = self.renderer.hdri.load_file(&entry.path) {
                                    if let Some(wgpu) = &mut self.wgpu_viewport {
                                        let (w, h, hdri_data) = crate::hdri_generator::generate_hdri_texture(&self.renderer.hdri, 1024, 512);
                                        wgpu.load_environment(w, h, &hdri_data);
                                    }
                                    self.status_message = format!("Imported and activated HDRI: {}", entry.name);
                                }
                            } else {
                                self.status_message = "Failed to import HDRI image file.".to_string();
                            }
                        }
                    }
                    if ui.button("Open HDRI Folder (assets/hdris)...").clicked() {
                        let _ = self.asset_manager.open_hdri_folder();
                        ui.close_menu();
                    }
                    if ui.button("↻ Refresh HDRI Folder").clicked() {
                        self.asset_manager.refresh_hdris();
                        self.status_message = format!("Refreshed HDRI assets: {} environments ready", self.asset_manager.hdris.len());
                        ui.close_menu();
                    }
                });

                ui.menu_button("Window", |ui| {
                    if ui.button("Reset Workspace to Default Layout").clicked() {
                        self.workspace = WorkspaceLayout::default();
                        self.show_shelf = true;
                        self.status_message = "Reset workspace to Substance 3D Painter default layout".to_string();
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("Panels & Views").strong().color(Color32::from_rgb(226, 91, 16)));
                    
                    if ui.checkbox(&mut self.workspace.show_toolbar, "Tools Toolbar (Left)").clicked() {
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut self.workspace.show_context_bar, "Context Tool Bar (Top)").clicked() {
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut self.workspace.show_status_bar, "Status Bar (Bottom)").clicked() {
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.checkbox(&mut self.workspace.show_texture_sets, "Texture Set List").clicked() {
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut self.workspace.show_layers, "Layers Stack").clicked() {
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut self.workspace.show_properties, "Properties Panel").clicked() {
                        ui.close_menu();
                    }
                    let mut shelf_vis = self.show_shelf;
                    if ui.checkbox(&mut shelf_vis, "Assets & Shelf").clicked() {
                        self.show_shelf = shelf_vis;
                        self.workspace.show_shelf = shelf_vis;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("Docking & Floating").strong().color(Color32::from_rgb(226, 91, 16)));
                    if ui.button("Dock All Panels").clicked() {
                        self.workspace.texture_sets_floating = false;
                        self.workspace.layers_floating = false;
                        self.workspace.properties_floating = false;
                        self.workspace.shelf_dock = ShelfDockLocation::Bottom;
                        self.status_message = "Docked all panels".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Undock All Panels (Pop Out)").clicked() {
                        self.workspace.texture_sets_floating = true;
                        self.workspace.layers_floating = true;
                        self.workspace.properties_floating = true;
                        self.workspace.shelf_dock = ShelfDockLocation::Floating;
                        self.status_message = "Popped out all panels into floating windows".to_string();
                        ui.close_menu();
                    }
                    ui.menu_button("Shelf Dock Placement", |ui| {
                        if ui.selectable_value(&mut self.workspace.shelf_dock, ShelfDockLocation::Bottom, "Bottom Dock (Substance Default)").clicked() {
                            self.workspace.show_shelf = true;
                            self.show_shelf = true;
                            ui.close_menu();
                        }
                        if ui.selectable_value(&mut self.workspace.shelf_dock, ShelfDockLocation::Left, "Left Dock").clicked() {
                            self.workspace.show_shelf = true;
                            self.show_shelf = true;
                            ui.close_menu();
                        }
                        if ui.selectable_value(&mut self.workspace.shelf_dock, ShelfDockLocation::Floating, "Floating Window").clicked() {
                            self.workspace.show_shelf = true;
                            self.show_shelf = true;
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    ui.label(egui::RichText::new("Viewport Layout").strong().color(Color32::from_rgb(226, 91, 16)));
                    if ui.selectable_value(&mut self.layout, ViewLayout::SplitDualView, "3D | 2D Split View (F1)").clicked() {
                        ui.close_menu();
                    }
                    if ui.selectable_value(&mut self.layout, ViewLayout::View3DOnly, "3D Viewport Only (F2)").clicked() {
                        ui.close_menu();
                    }
                    if ui.selectable_value(&mut self.layout, ViewLayout::View2DOnly, "2D UV Canvas Only (F3)").clicked() {
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("Check for Updates...").clicked() {
                        self.updater.check_for_updates(true);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("About 3D Paintcraft").clicked() {
                        self.show_about_modal = true;
                        ui.close_menu();
                    }
                });

                if self.updater.has_update_available() {
                    ui.add_space(8.0);
                    let update_badge = egui::Button::new(
                        egui::RichText::new("Update Available!").color(egui::Color32::from_rgb(46, 204, 113)).strong()
                    ).small();
                    if ui.add(update_badge).clicked() {
                        self.updater.show_dialog = true;
                    }
                }
            });
        });

        // --- SUBSTANCE 3D PAINTER CONTEXTUAL & VIEWPORT TOOLBAR ---
        egui::TopBottomPanel::top("viewport_controls")
            .exact_height(34.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(4.0);

                    // Left: Dynamic Tool Parameters based on active tool
                    match self.brush_engine.settings.mode {
                        ToolMode::Brush | ToolMode::Eraser | ToolMode::Projection | ToolMode::Smudge | ToolMode::Clone => {
                            let tool_name = match self.brush_engine.settings.mode {
                                ToolMode::Brush => "Paint",
                                ToolMode::Eraser => "Eraser",
                                ToolMode::Projection => "Projection",
                                ToolMode::Smudge => "Smudge",
                                ToolMode::Clone => "Clone",
                                _ => "Paint",
                            };
                            ui.label(egui::RichText::new(tool_name).strong().color(Color32::from_rgb(243, 94, 24)));
                            ui.spacing_mut().slider_width = 46.0;
                            ui.add_space(4.0);

                            ui.label("Size:");
                            ui.add_sized(
                                [65.0, 18.0],
                                egui::Slider::new(&mut self.brush_engine.settings.radius, 1.0..=180.0)
                                    .logarithmic(true)
                                    .suffix("px"),
                            );

                            ui.label("Flow:");
                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.flow, 0.01..=1.0));

                            ui.label("Opacity:");
                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.opacity, 0.01..=1.0));

                            ui.label("Hard:");
                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.hardness, 0.0..=1.0));

                            ui.label("Spacing:");
                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.spacing, 0.02..=0.5));

                            ui.separator();
                            // Target: Material vs Mask (when selected layer has mask)
                            let act_idx = self.project.active_layer_idx;
                            let has_mask = self.project.layers.get(act_idx).map(|l| l.has_mask).unwrap_or(false);
                            if has_mask {
                                ui.label("Target:");
                                let is_mat = self.brush_engine.settings.target == PaintTarget::Material;
                                if ui.selectable_label(is_mat, "Material").clicked() {
                                    self.brush_engine.settings.target = PaintTarget::Material;
                                }
                                if ui.selectable_label(!is_mat, "Mask").clicked() {
                                    self.brush_engine.settings.target = PaintTarget::Mask;
                                }
                                ui.separator();
                            }

                            // Symmetry controls
                            let any_sym = self.brush_engine.settings.symmetry_x || self.brush_engine.settings.symmetry_y || self.brush_engine.settings.symmetry_z;
                            let sym_label_color = if any_sym { Color32::from_rgb(243, 94, 24) } else { Color32::from_rgb(140, 145, 155) };
                            ui.label(egui::RichText::new("Symmetry:").color(sym_label_color));

                            let sym_off = !any_sym;
                            if ui.selectable_label(sym_off, "Off").on_hover_text("Disable Symmetry Painting").clicked() {
                                self.brush_engine.settings.symmetry_x = false;
                                self.brush_engine.settings.symmetry_y = false;
                                self.brush_engine.settings.symmetry_z = false;
                            }
                            let sym_x = self.brush_engine.settings.symmetry_x;
                            if ui.selectable_label(sym_x, "X").on_hover_text("Mirror X (Left/Right plane)").clicked() {
                                self.brush_engine.settings.symmetry_x = !sym_x;
                            }
                            let sym_y = self.brush_engine.settings.symmetry_y;
                            if ui.selectable_label(sym_y, "Y").on_hover_text("Mirror Y (Top/Bottom plane)").clicked() {
                                self.brush_engine.settings.symmetry_y = !sym_y;
                            }
                            let sym_z = self.brush_engine.settings.symmetry_z;
                            if ui.selectable_label(sym_z, "Z").on_hover_text("Mirror Z (Front/Back plane)").clicked() {
                                self.brush_engine.settings.symmetry_z = !sym_z;
                            }
                            if any_sym {
                                if ui.small_button("Center").on_hover_text("Center symmetry plane on active mesh bounds").clicked() {
                                    self.brush_engine.settings.symmetry_center = (self.project.mesh.aabb_min + self.project.mesh.aabb_max) * 0.5;
                                    self.status_message = "Symmetry plane centered on mesh".to_string();
                                }
                            }

                            ui.separator();
                            // Stabilizer / Lazy Mouse
                            let lazy = self.brush_engine.settings.lazy_mouse_enabled;
                            if ui.selectable_label(lazy, "Lazy").on_hover_text("Lazy Mouse Stroke Stabilizer").clicked() {
                                self.brush_engine.settings.lazy_mouse_enabled = !lazy;
                            }
                            if self.brush_engine.settings.lazy_mouse_enabled {
                                ui.add(egui::Slider::new(&mut self.brush_engine.settings.lazy_mouse_radius, 0.01..=0.2).text("Dist"));
                            }

                            // Stylus Tablet Pressure Dynamics
                            let pres_sz = self.brush_engine.settings.pressure_size;
                            if ui.selectable_label(pres_sz, "Pres.Size").on_hover_text("Pen Tablet Pressure scales brush radius").clicked() {
                                self.brush_engine.settings.pressure_size = !pres_sz;
                            }
                            let pres_op = self.brush_engine.settings.pressure_opacity;
                            if ui.selectable_label(pres_op, "Pres.Flow").on_hover_text("Pen Tablet Pressure scales brush opacity and flow").clicked() {
                                self.brush_engine.settings.pressure_opacity = !pres_op;
                            }
                            let pres_hd = self.brush_engine.settings.pressure_hardness;
                            if ui.selectable_label(pres_hd, "Pres.Hard").on_hover_text("Pen Tablet Pressure scales brush edge hardness").clicked() {
                                self.brush_engine.settings.pressure_hardness = !pres_hd;
                            }
                            egui::ComboBox::from_id_source("stylus_pressure_curve_combo")
                                .selected_text(self.brush_engine.settings.pressure_curve.name())
                                .width(65.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.brush_engine.settings.pressure_curve, crate::brush::PressureCurve::Linear, "Linear");
                                    ui.selectable_value(&mut self.brush_engine.settings.pressure_curve, crate::brush::PressureCurve::Soft, "Soft (Sens)");
                                    ui.selectable_value(&mut self.brush_engine.settings.pressure_curve, crate::brush::PressureCurve::Firm, "Firm (Heavy)");
                                });
                            ui.label(
                                egui::RichText::new(format!("P:{:.0}%", self.last_stylus_pressure * 100.0))
                                    .monospace()
                                    .color(egui::Color32::from_rgb(140, 220, 255)),
                            );

                            // Follow Path Brush Dynamics
                            let follow = self.brush_engine.settings.follow_path;
                            if ui.selectable_label(follow, "Follow Path").on_hover_text("Rotate brush stamp along stroke movement tangent vector (for stitches, seams, tracks)").clicked() {
                                self.brush_engine.settings.follow_path = !follow;
                            }
                            if self.brush_engine.settings.follow_path {
                                ui.add(egui::Slider::new(&mut self.brush_engine.settings.follow_path_offset, -std::f32::consts::PI..=std::f32::consts::PI).text("Offset"));
                            }

                            if self.brush_engine.settings.mode == ToolMode::Smudge {
                                ui.separator();
                                ui.label(egui::RichText::new("Smudge:").strong().color(Color32::from_rgb(243, 94, 24)));
                                ui.add(egui::Slider::new(&mut self.brush_engine.settings.smudge_strength, 0.05..=1.0).text("Strength"));
                            }

                            if self.brush_engine.settings.mode == ToolMode::Clone {
                                ui.separator();
                                ui.label(egui::RichText::new("Clone Stamp:").strong().color(Color32::from_rgb(243, 94, 24)));
                                if let Some(src) = self.brush_engine.settings.clone_source_uv {
                                    ui.label(format!("Src: ({:.2}, {:.2})", src.x, src.y));
                                    if ui.small_button("Clear").clicked() {
                                        self.brush_engine.settings.clone_source_uv = None;
                                    }
                                } else {
                                    ui.label(egui::RichText::new("[Hold V or Alt + Click]").small().color(Color32::from_rgb(243, 160, 40)));
                                }
                            }

                            if self.brush_engine.settings.mode == ToolMode::Projection {
                                ui.separator();
                                ui.label(egui::RichText::new("Stencil:").strong().color(Color32::from_rgb(243, 94, 24)));
                                ui.checkbox(&mut self.brush_engine.settings.stencil.enabled, "Enable");
                                if self.brush_engine.settings.stencil.enabled {
                                    ui.add(egui::Slider::new(&mut self.brush_engine.settings.stencil.scale, 0.1..=4.0).text("Scale"));
                                    ui.add(egui::Slider::new(&mut self.brush_engine.settings.stencil.angle, -3.1415..=3.1415).text("Angle"));
                                    ui.add(egui::Slider::new(&mut self.brush_engine.settings.stencil.opacity, 0.05..=1.0).text("Opacity"));
                                    ui.checkbox(&mut self.brush_engine.settings.stencil.mask_only, "Mask Only");
                                    if ui.small_button("Reset (S+Z)").on_hover_text("Reset stencil position, scale, and rotation to default").clicked() {
                                        self.brush_engine.settings.stencil.pos = glam::Vec2::ZERO;
                                        self.brush_engine.settings.stencil.scale = 1.0;
                                        self.brush_engine.settings.stencil.angle = 0.0;
                                    }

                                    ui.horizontal(|ui| {
                                        if ui.button("Pick from Shelf...").on_hover_text("Pick stencil texture from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::Stencil));
                                        }
                                        if ui.button("Load File...").on_hover_text("Load image file into Project Shelf and set as Stencil").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::Stencil);
                                                        self.stencil_overlay_texture = None;
                                                    }
                                                }
                                            }
                                        }
                                    });

                                    egui::ComboBox::from_id_source("stencil_preset_combo")
                                        .selected_text("Preset Stencils...")
                                        .show_ui(ui, |ui| {
                                            if ui.selectable_label(false, "Grunge Map").clicked() {
                                                let mut buf = crate::channel::RgbaBuffer::new(512, 512, glam::Vec4::new(1.0, 1.0, 1.0, 1.0));
                                                for y in 0..512 {
                                                    for x in 0..512 {
                                                        let u = x as f32 / 512.0;
                                                        let v = y as f32 / 512.0;
                                                        let val = crate::utils::NoiseGenerator::grunge_map_2d(u * 6.0, v * 6.0);
                                                        buf.set_pixel(x, y, glam::Vec4::new(val, val, val, 1.0));
                                                    }
                                                }
                                                self.brush_engine.stencil_buffer = Some(buf);
                                                self.stencil_overlay_texture = None;
                                                self.status_message = "Loaded Grunge Stencil preset".to_string();
                                            }
                                            if ui.selectable_label(false, "Scratches").clicked() {
                                                let mut buf = crate::channel::RgbaBuffer::new(512, 512, glam::Vec4::new(1.0, 1.0, 1.0, 1.0));
                                                for y in 0..512 {
                                                    for x in 0..512 {
                                                        let u = x as f32 / 512.0;
                                                        let v = y as f32 / 512.0;
                                                        let val = crate::utils::NoiseGenerator::directional_noise_2d(u * 8.0, v * 8.0, 0.4);
                                                        buf.set_pixel(x, y, glam::Vec4::new(val, val, val, 1.0));
                                                    }
                                                }
                                                self.brush_engine.stencil_buffer = Some(buf);
                                                self.stencil_overlay_texture = None;
                                                self.status_message = "Loaded Scratches Stencil preset".to_string();
                                            }
                                            if ui.selectable_label(false, "Spots & Splatter").clicked() {
                                                let mut buf = crate::channel::RgbaBuffer::new(512, 512, glam::Vec4::new(1.0, 1.0, 1.0, 1.0));
                                                for y in 0..512 {
                                                    for x in 0..512 {
                                                        let u = x as f32 / 512.0;
                                                        let v = y as f32 / 512.0;
                                                        let val = crate::utils::NoiseGenerator::bnw_spots_1_2d(u * 5.0, v * 5.0);
                                                        buf.set_pixel(x, y, glam::Vec4::new(val, val, val, 1.0));
                                                    }
                                                }
                                                self.brush_engine.stencil_buffer = Some(buf);
                                                self.stencil_overlay_texture = None;
                                                self.status_message = "Loaded Spots & Splatter Stencil preset".to_string();
                                            }
                                            if ui.selectable_label(false, "Checkerboard").clicked() {
                                                let mut buf = crate::channel::RgbaBuffer::new(512, 512, glam::Vec4::new(1.0, 1.0, 1.0, 1.0));
                                                for y in 0..512 {
                                                    for x in 0..512 {
                                                        let cx = (x / 32) % 2;
                                                        let cy = (y / 32) % 2;
                                                        let val = if cx ^ cy == 0 { 1.0 } else { 0.0 };
                                                        buf.set_pixel(x, y, glam::Vec4::new(val, val, val, 1.0));
                                                    }
                                                }
                                                self.brush_engine.stencil_buffer = Some(buf);
                                                self.stencil_overlay_texture = None;
                                                self.status_message = "Loaded Checkerboard Stencil preset".to_string();
                                            }
                                        });
                                }
                            }
                        }
                        ToolMode::FillBucket => {
                            ui.label(egui::RichText::new("Polygon Fill").strong().color(Color32::from_rgb(243, 94, 24)));
                            ui.add_space(4.0);

                            // Mode Selection: Triangle, UV Island, Mesh Element
                            ui.label("Mode:");
                            let poly_mode = self.brush_engine.settings.polygon_fill_mode;
                            if ui.selectable_label(poly_mode == crate::brush::PolygonFillMode::Triangle, "Triangle").on_hover_text("Fill individual triangle under cursor").clicked() {
                                self.brush_engine.settings.polygon_fill_mode = crate::brush::PolygonFillMode::Triangle;
                            }
                            if ui.selectable_label(poly_mode == crate::brush::PolygonFillMode::Polygon, "Polygon (Quad)").on_hover_text("Fill quad / coplanar polygon pair").clicked() {
                                self.brush_engine.settings.polygon_fill_mode = crate::brush::PolygonFillMode::Polygon;
                            }
                            if ui.selectable_label(poly_mode == crate::brush::PolygonFillMode::UvIsland, "UV Island").on_hover_text("Fill entire connected UV island").clicked() {
                                self.brush_engine.settings.polygon_fill_mode = crate::brush::PolygonFillMode::UvIsland;
                            }
                            if ui.selectable_label(poly_mode == crate::brush::PolygonFillMode::MeshElement, "Mesh Element").on_hover_text("Fill contiguous 3D mesh element").clicked() {
                                self.brush_engine.settings.polygon_fill_mode = crate::brush::PolygonFillMode::MeshElement;
                            }
                            ui.separator();

                            // Target Selection: Material or Mask
                            let target = self.brush_engine.settings.target;
                            if ui.selectable_label(target == PaintTarget::Material, "Material").clicked() {
                                self.brush_engine.settings.target = PaintTarget::Material;
                            }
                            if ui.selectable_label(target == PaintTarget::Mask, "Mask").clicked() {
                                self.brush_engine.settings.target = PaintTarget::Mask;
                            }
                            ui.separator();

                            if target == PaintTarget::Mask {
                                ui.label("Value:");
                                if ui.small_button("1.0 White (Reveal)").clicked() {
                                    self.brush_engine.settings.polygon_fill_value = 1.0;
                                    self.brush_engine.settings.mask_paint_value = 1.0;
                                }
                                if ui.small_button("0.0 Black (Hide)").clicked() {
                                    self.brush_engine.settings.polygon_fill_value = 0.0;
                                    self.brush_engine.settings.mask_paint_value = 0.0;
                                }
                                ui.add_sized([55.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.polygon_fill_value, 0.0..=1.0));
                            } else {
                                ui.label("Color:");
                                crate::color_picker::color_picker_button_vec4(
                                    ui,
                                    egui::Id::new("poly_fill_color_btn"),
                                    &mut self.brush_engine.settings.paint_color,
                                );
                                ui.label("Rough:");
                                ui.add_sized([55.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.paint_roughness, 0.0..=1.0));
                                ui.label("Metal:");
                                ui.add_sized([55.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.paint_metallic, 0.0..=1.0));
                                ui.label("Height:");
                                ui.add_sized([55.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.paint_height, 0.0..=1.0));
                            }
                        }
                        ToolMode::Eyedropper => {
                            ui.label(egui::RichText::new("Picker").strong().color(Color32::from_rgb(220, 225, 235)));
                            ui.label("Click surface in viewport to pick material");
                            let col = [
                                (self.brush_engine.settings.paint_color.x * 255.0) as u8,
                                (self.brush_engine.settings.paint_color.y * 255.0) as u8,
                                (self.brush_engine.settings.paint_color.z * 255.0) as u8,
                            ];
                            let (sq_rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            ui.painter().rect_filled(sq_rect, 2.0, Color32::from_rgb(col[0], col[1], col[2]));
                        }
                        ToolMode::Path => {
                            ui.label(egui::RichText::new("Vector Path").strong().color(Color32::from_rgb(243, 94, 24)));
                            let num_paths = self.brush_engine.settings.path_tool.total_path_count();
                            let num_pts = self.brush_engine.settings.path_tool.total_point_count();
                            ui.label(format!("Paths: {} | Pts: {}", num_paths, num_pts));

                            if ui.button("+ New Path (N)").on_hover_text("Start a new disconnected path stroke (Shortcut: N)").clicked() {
                                self.brush_engine.settings.path_tool.start_new_path();
                                self.sync_and_evaluate_active_path_layer(ctx);
                                self.status_message = format!("Started new path #{}", self.brush_engine.settings.path_tool.total_path_count());
                            }

                            let has_any = !self.brush_engine.settings.path_tool.is_empty();
                            if ui.add_enabled(has_any, egui::Button::new(egui::RichText::new("Commit (Enter)").color(Color32::WHITE)).fill(Color32::from_rgb(45, 120, 60))).clicked() {
                                self.commit_path_tool(ctx);
                            }
                            if ui.add_enabled(has_any, egui::Button::new("Clear (Esc)")).clicked() {
                                self.brush_engine.settings.path_tool.clear();
                                self.sync_and_evaluate_active_path_layer(ctx);
                                self.status_message = "Path cleared".to_string();
                            }
                            ui.separator();

                            let cur_style = self.brush_engine.settings.path_tool.stroke_style;
                            egui::ComboBox::from_id_source("top_bar_path_stroke_style")
                                .selected_text(cur_style.name())
                                .width(120.0)
                                .show_ui(ui, |ui| {
                                    for style in &[
                                        crate::brush::PathStrokeStyle::Continuous,
                                        crate::brush::PathStrokeStyle::WeldSeam,
                                        crate::brush::PathStrokeStyle::RopeBraid,
                                        crate::brush::PathStrokeStyle::StitchSeam,
                                        crate::brush::PathStrokeStyle::ChainLinks,
                                        crate::brush::PathStrokeStyle::SmartDecal,
                                    ] {
                                        ui.selectable_value(&mut self.brush_engine.settings.path_tool.stroke_style, *style, style.name());
                                    }
                                });

                            if self.brush_engine.settings.path_tool.stroke_style == crate::brush::PathStrokeStyle::SmartDecal {
                                let cur_decal = self.brush_engine.settings.path_tool.decal_type;
                                egui::ComboBox::from_id_source("top_bar_path_decal_type")
                                    .selected_text(cur_decal.name())
                                    .width(135.0)
                                    .show_ui(ui, |ui| {
                                        for decal in &[
                                            crate::brush::SmartDecalType::ButtonHole,
                                            crate::brush::SmartDecalType::PhillipsScrew,
                                            crate::brush::SmartDecalType::FlatheadScrew,
                                            crate::brush::SmartDecalType::TorxScrew,
                                            crate::brush::SmartDecalType::HexBolt,
                                            crate::brush::SmartDecalType::DomedRivet,
                                            crate::brush::SmartDecalType::GrommetEyelet,
                                            crate::brush::SmartDecalType::AirVentLouver,
                                        ] {
                                            ui.selectable_value(&mut self.brush_engine.settings.path_tool.decal_type, *decal, decal.name());
                                        }
                                    });

                                ui.label("Spacing:");
                                ui.add_sized([50.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.path_tool.decal_spacing, 0.4..=3.0).text("x"));
                            } else if self.brush_engine.settings.path_tool.stroke_style == crate::brush::PathStrokeStyle::WeldSeam {
                                ui.checkbox(&mut self.brush_engine.settings.path_tool.heat_tint, "HAZ Tint");
                            }

                            ui.separator();
                            ui.checkbox(&mut self.brush_engine.settings.path_tool.closed, "Loop");
                            ui.checkbox(&mut self.brush_engine.settings.path_tool.follow_tangent, "Tangent");

                            if self.brush_engine.settings.path_tool.stroke_style == crate::brush::PathStrokeStyle::Continuous {
                                ui.label("Spacing:");
                                ui.add_sized([50.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.path_tool.spacing_ratio, 0.05..=1.5).text(""));
                            }

                            ui.label("Taper:");
                            ui.add_sized([45.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.path_tool.taper_start, 0.0..=1.0).text("S"));
                            ui.add_sized([45.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.path_tool.taper_end, 0.0..=1.0).text("E"));

                            ui.label("Depth:");
                            ui.add_sized([45.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.path_tool.height_depth, -1.0..=1.0).text(""));

                            ui.label("Offset:");
                            ui.add_sized([45.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.path_tool.surface_offset, -0.05..=0.05).text(""));

                            ui.checkbox(&mut self.brush_engine.settings.path_tool.snap_to_surface, "Snap");

                            ui.separator();
                            ui.label("Radius:");
                            ui.add_sized([55.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.radius, 1.0..=100.0).text("px"));

                            let mut path_col = [
                                self.brush_engine.settings.paint_color.x,
                                self.brush_engine.settings.paint_color.y,
                                self.brush_engine.settings.paint_color.z,
                                self.brush_engine.settings.paint_color.w,
                            ];
                            if ui.color_edit_button_rgba_unmultiplied(&mut path_col).on_hover_text("Path Decal Color").changed() {
                                self.brush_engine.settings.paint_color = glam::Vec4::new(path_col[0], path_col[1], path_col[2], path_col[3]);
                            }
                            ui.label("R:");
                            ui.add_sized([40.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.paint_roughness, 0.0..=1.0).text(""));
                            ui.label("M:");
                            ui.add_sized([40.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.paint_metallic, 0.0..=1.0).text(""));

                            if ui.small_button("+ Save Decal...").on_hover_text("Save current decal/path parameters as a custom preset").clicked() {
                                self.show_new_decal_modal = true;
                                self.new_decal_name = format!("Decal_{}", self.asset_manager.decals.len() + 1);
                            }
                        }
                        ToolMode::Stamp => {
                            ui.label(egui::RichText::new("Decal Stamp").strong().color(Color32::from_rgb(255, 175, 40)));

                            let cur_style = self.brush_engine.settings.stamp_tool.stroke_style;
                            egui::ComboBox::from_id_source("top_bar_stamp_stroke_style")
                                .selected_text(cur_style.name())
                                .width(115.0)
                                .show_ui(ui, |ui| {
                                    for style in &[
                                        crate::brush::PathStrokeStyle::Continuous,
                                        crate::brush::PathStrokeStyle::WeldSeam,
                                        crate::brush::PathStrokeStyle::RopeBraid,
                                        crate::brush::PathStrokeStyle::StitchSeam,
                                        crate::brush::PathStrokeStyle::ChainLinks,
                                        crate::brush::PathStrokeStyle::SmartDecal,
                                    ] {
                                        ui.selectable_value(&mut self.brush_engine.settings.stamp_tool.stroke_style, *style, style.name());
                                    }
                                });

                            if self.brush_engine.settings.stamp_tool.stroke_style == crate::brush::PathStrokeStyle::SmartDecal {
                                let cur_decal = self.brush_engine.settings.stamp_tool.decal_type;
                                egui::ComboBox::from_id_source("top_bar_stamp_decal_type")
                                    .selected_text(cur_decal.name())
                                    .width(130.0)
                                    .show_ui(ui, |ui| {
                                        for decal in &[
                                            crate::brush::SmartDecalType::ButtonHole,
                                            crate::brush::SmartDecalType::PhillipsScrew,
                                            crate::brush::SmartDecalType::FlatheadScrew,
                                            crate::brush::SmartDecalType::TorxScrew,
                                            crate::brush::SmartDecalType::HexBolt,
                                            crate::brush::SmartDecalType::DomedRivet,
                                            crate::brush::SmartDecalType::GrommetEyelet,
                                            crate::brush::SmartDecalType::AirVentLouver,
                                        ] {
                                            ui.selectable_value(&mut self.brush_engine.settings.stamp_tool.decal_type, *decal, decal.name());
                                        }
                                    });
                            } else if self.brush_engine.settings.stamp_tool.stroke_style == crate::brush::PathStrokeStyle::WeldSeam {
                                ui.checkbox(&mut self.brush_engine.settings.stamp_tool.heat_tint, "HAZ");
                            }

                            if !self.asset_manager.decals.is_empty() {
                                let selected_label = self.brush_engine.settings.stamp_tool.selected_preset_id
                                    .as_deref()
                                    .and_then(|id| self.asset_manager.decals.iter().find(|d| d.id == id).map(|d| d.name.as_str()))
                                    .unwrap_or("Custom Presets");
                                egui::ComboBox::from_id_source("top_bar_stamp_custom_presets")
                                    .selected_text(selected_label)
                                    .width(115.0)
                                    .show_ui(ui, |ui| {
                                        for preset in &self.asset_manager.decals {
                                            let is_sel = self.brush_engine.settings.stamp_tool.selected_preset_id.as_deref() == Some(&preset.id);
                                            if ui.selectable_label(is_sel, &preset.name).clicked() {
                                                self.brush_engine.settings.stamp_tool.stroke_style = preset.stroke_style;
                                                self.brush_engine.settings.stamp_tool.decal_type = preset.decal_type;
                                                self.brush_engine.settings.stamp_tool.height_depth = preset.height_depth;
                                                self.brush_engine.settings.stamp_tool.heat_tint = preset.heat_tint;
                                                self.brush_engine.settings.stamp_tool.surface_offset = preset.surface_offset;
                                                self.brush_engine.settings.stamp_tool.selected_preset_id = Some(preset.id.clone());
                                                self.brush_engine.settings.radius = preset.radius;
                                                self.brush_engine.settings.paint_color = glam::Vec4::new(preset.base_color[0], preset.base_color[1], preset.base_color[2], preset.base_color[3]);
                                                self.brush_engine.settings.paint_roughness = preset.roughness;
                                                self.brush_engine.settings.paint_metallic = preset.metallic;
                                                self.brush_engine.settings.paint_emissive = glam::Vec4::new(preset.emissive[0], preset.emissive[1], preset.emissive[2], preset.emissive[3]);
                                                self.brush_engine.settings.paint_opacity = preset.opacity;
                                                self.brush_engine.settings.paint_clearcoat = preset.clearcoat;
                                                self.brush_engine.settings.paint_anisotropy = preset.anisotropy;
                                            }
                                        }
                                    });
                            }

                            ui.separator();
                            ui.label("Size:");
                            ui.add_sized([55.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.radius, 1.0..=150.0).text("px"));

                            ui.label("Rot:");
                            ui.add_sized([55.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.stamp_tool.rotation_angle, 0.0..=360.0).text("°"));
                            ui.checkbox(&mut self.brush_engine.settings.stamp_tool.randomize_rotation, "Random");

                            ui.label("Depth:");
                            ui.add_sized([45.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.stamp_tool.height_depth, -1.0..=1.0).text(""));

                            ui.label("Offset:");
                            ui.add_sized([45.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.stamp_tool.surface_offset, -0.05..=0.05).text(""));

                            let mut stamp_col = [
                                self.brush_engine.settings.paint_color.x,
                                self.brush_engine.settings.paint_color.y,
                                self.brush_engine.settings.paint_color.z,
                                self.brush_engine.settings.paint_color.w,
                            ];
                            if ui.color_edit_button_rgba_unmultiplied(&mut stamp_col).on_hover_text("Decal Base Color").changed() {
                                self.brush_engine.settings.paint_color = glam::Vec4::new(stamp_col[0], stamp_col[1], stamp_col[2], stamp_col[3]);
                            }
                            ui.label("R:");
                            ui.add_sized([40.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.paint_roughness, 0.0..=1.0).text(""));
                            ui.label("M:");
                            ui.add_sized([40.0, 18.0], egui::Slider::new(&mut self.brush_engine.settings.paint_metallic, 0.0..=1.0).text(""));

                            ui.separator();
                            if ui.small_button("+ Save Decal...").on_hover_text("Save current decal parameters as a custom preset").clicked() {
                                self.show_new_decal_modal = true;
                                self.new_decal_name = format!("Decal_{}", self.asset_manager.decals.len() + 1);
                            }
                        }
                    }

                    // Right: Viewport Settings, Shaders, Layout & Channel Controls
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let opt_icons = self.icons.as_ref();

                        // 0. Iray Progressive Path Tracer Toggle & Quick Controls
                        let iray_label = if self.iray_mode { "Iray ON (F10)" } else { "Iray (F10)" };
                        if ui.selectable_label(self.iray_mode, iray_label).on_hover_text("Toggle Interactive Progressive Path Tracer Viewport (F10)").clicked() {
                            self.iray_mode = !self.iray_mode;
                            if self.iray_mode {
                                if let Some(pt) = &mut self.path_tracer {
                                    pt.reset();
                                }
                                self.status_message = "Iray Mode: Path Tracer Active [F10 or Esc to Exit]".to_string();
                            } else {
                                self.status_message = "Iray Mode: Exited to Real-Time Viewport".to_string();
                            }
                        }
                        if self.iray_mode {
                            let denoise_label = if self.iray_denoise { "Denoise: ON" } else { "Denoise: OFF" };
                            if ui.selectable_label(self.iray_denoise, denoise_label).on_hover_text("Toggle Normal-Guided Bilateral Denoiser for Iray").clicked() {
                                self.iray_denoise = !self.iray_denoise;
                                if let Some(pt) = &mut self.path_tracer {
                                    pt.denoiser_enabled = self.iray_denoise;
                                    pt.update_color_buffer();
                                }
                            }
                            if ui.small_button("Save PNG").on_hover_text("Save converged path-traced render to PNG file").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("PNG Image", &["png"])
                                    .set_file_name("paintcraft_iray_render.png")
                                    .save_file()
                                {
                                    if let Some(pt) = &self.path_tracer {
                                        match pt.save_to_png(&path) {
                                            Ok(()) => self.status_message = format!("Saved Iray render to {}", path.display()),
                                            Err(err) => self.status_message = format!("Failed to save render: {}", err),
                                        }
                                    }
                                }
                            }
                            if ui.small_button("Reset").on_hover_text("Restart Iray accumulation").clicked() {
                                if let Some(pt) = &mut self.path_tracer {
                                    pt.reset();
                                }
                            }
                        }

                        ui.separator();

                        // 1. Shelf Toggle Button
                        let shelf_btn_text = if self.show_shelf { "Shelf [v]" } else { "Shelf [^]" };
                        if ui.selectable_label(self.show_shelf, shelf_btn_text).on_hover_text("Toggle Shelf Assets Panel").clicked() {
                            self.show_shelf = !self.show_shelf;
                        }

                        ui.separator();

                        // 2. Viewport Settings Button
                        let vp_btn = if let Some(icons) = opt_icons {
                            egui::Button::image_and_text((icons.viewport_settings.id(), egui::vec2(13.0, 13.0)), "Viewport Settings")
                        } else {
                            egui::Button::new("Viewport Settings")
                        };
                        if ui.add(vp_btn).on_hover_text("Open Viewport Camera, Navigation, HDRI, and Display settings").clicked() {
                            self.show_viewport_settings_modal = true;
                        }

                        // 3. Shaders Button
                        let shaders_btn = if let Some(icons) = opt_icons {
                            egui::Button::image_and_text((icons.shaders.id(), egui::vec2(13.0, 13.0)), "Shaders")
                        } else {
                            egui::Button::new("Shaders")
                        };
                        if ui.add(shaders_btn).on_hover_text("Open Shader, Presets, and Post-Processing settings").clicked() {
                            self.show_shader_settings_modal = true;
                        }

                        ui.separator();

                        // 4. Viewport Layout Mode (3D | 3D|2D | 2D)
                        if ui.selectable_label(self.layout == ViewLayout::View3DOnly, "3D").on_hover_text("3D Viewport Only [F2]").clicked() {
                            self.layout = ViewLayout::View3DOnly;
                        }
                        if ui.selectable_label(self.layout == ViewLayout::SplitDualView, "3D|2D").on_hover_text("Split 3D & 2D Dual View [F1]").clicked() {
                            self.layout = ViewLayout::SplitDualView;
                        }
                        if ui.selectable_label(self.layout == ViewLayout::View2DOnly, "2D").on_hover_text("2D UV Texture Canvas Only [F3]").clicked() {
                            self.layout = ViewLayout::View2DOnly;
                        }

                        ui.separator();

                        // 5. View Mode / Channel Dropdown
                        egui::ComboBox::from_id_source("viewport_mode_select")
                            .selected_text(self.view_mode.name())
                            .width(115.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Material, "Lit Material (PBR) [M]");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::BaseColor, "Base Color");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Roughness, "Roughness");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Metallic, "Metallic");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Normal, "Normal Map");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Height, "Height / Bump");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::AmbientOcclusion, "Ambient Occlusion (AO)");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Curvature, "Curvature Map");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Position, "Position (WPO)");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::WorldNormal, "World Space Normal");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Thickness, "Thickness");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Emissive, "Emissive");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Opacity, "Opacity");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Clearcoat, "Clearcoat");
                                ui.selectable_value(&mut self.view_mode, ViewportChannelMode::Anisotropy, "Anisotropy");
                            });
                        ui.label("Channel:");

                        // 6. Camera reset quick button
                        if ui.small_button("Cam [F]").on_hover_text("Reset Camera (F)").clicked() {
                            self.camera.reset();
                        }

                        // 7. Wireframe overlay quick toggle button
                        if ui.selectable_label(self.renderer.show_wireframe, "Wireframe [F4]")
                            .on_hover_text("Toggle Wireframe Overlay (F4)")
                            .clicked()
                        {
                            self.renderer.show_wireframe = !self.renderer.show_wireframe;
                            self.show_uv_wireframe = self.renderer.show_wireframe;
                        }

                        // 8. Real-time shadows quick toggle button
                        if ui.selectable_label(self.shader_settings.shadows_enabled, "Shadows")
                            .on_hover_text("Toggle Real-Time Directional Sun Shadows")
                            .clicked()
                        {
                            self.shader_settings.shadows_enabled = !self.shader_settings.shadows_enabled;
                        }
                    });
                });
            });

        // --- BOTTOM STATUS BAR (Clean and minimal) ---
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(22.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.small(&self.status_message);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small("3D Paintcraft • Pure Rust Studio Engine (Linux)");
                    });
                });
            });

        // --- SUBSTANCE 3D PAINTER ICONIC VERTICAL LEFT TOOLS TOOLBAR ---
        if !self.zen_mode {
            egui::SidePanel::left("left_toolbar")
            .resizable(false)
            .exact_width(46.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);

                    // Helper to render high-polish tool card with icon
                    let opt_icons = self.icons.as_ref();
                    let render_tool_button = |ui: &mut egui::Ui, active: bool, opt_tex: Option<egui::TextureId>, fallback_glyph: &str, label: &str, tip: &str| -> bool {
                        let btn_size = egui::vec2(38.0, 42.0);
                        let (rect, resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
                        let bg = if active {
                            Color32::from_rgb(52, 56, 65)
                        } else if resp.hovered() {
                            Color32::from_rgb(38, 41, 48)
                        } else {
                            Color32::from_rgb(26, 28, 32)
                        };
                        let stroke = if active {
                            egui::Stroke::new(1.5_f32, Color32::from_rgb(243, 94, 24))
                        } else if resp.hovered() {
                            egui::Stroke::new(1.0_f32, Color32::from_rgb(60, 65, 75))
                        } else {
                            egui::Stroke::new(1.0_f32, Color32::from_rgb(34, 36, 40))
                        };

                        ui.painter().rect(rect, 4.0, bg, stroke);

                        if let Some(tex_id) = opt_tex {
                            let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + 16.0), egui::vec2(22.0, 22.0));
                            ui.painter().image(
                                tex_id,
                                icon_rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                if active { Color32::WHITE } else { Color32::from_rgba_unmultiplied(190, 195, 205, 220) },
                            );
                        } else {
                            // Tool glyph centered top
                            let glyph_pos = egui::pos2(rect.center().x, rect.min.y + 14.0);
                            ui.painter().text(
                                glyph_pos,
                                egui::Align2::CENTER_CENTER,
                                fallback_glyph,
                                egui::FontId::proportional(12.0),
                                if active { Color32::WHITE } else { Color32::from_rgb(190, 195, 205) },
                            );
                        }

                        // Tool label centered bottom
                        let label_pos = egui::pos2(rect.center().x, rect.max.y - 7.0);
                        ui.painter().text(
                            label_pos,
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::proportional(8.0),
                            if active { Color32::from_rgb(235, 240, 250) } else { Color32::from_rgb(130, 135, 145) },
                        );

                        resp.on_hover_text(tip).clicked()
                    };

                    // 1. Paint [1 / B]
                    let is_brush = self.brush_engine.settings.mode == ToolMode::Brush;
                    let brush_tex = opt_icons.map(|i| i.brush.id());
                    if render_tool_button(ui, is_brush, brush_tex, "B", "Paint", "Paint Brush [1 / B]") {
                        self.brush_engine.settings.mode = ToolMode::Brush;
                        self.status_message = "Tool: Paint Brush [1/B]".to_string();
                    }
                    ui.add_space(2.0);

                    // 2. Eraser [2 / E]
                    let is_eraser = self.brush_engine.settings.mode == ToolMode::Eraser;
                    let eraser_tex = opt_icons.map(|i| i.eraser.id());
                    if render_tool_button(ui, is_eraser, eraser_tex, "E", "Erase", "Eraser [2 / E]") {
                        self.brush_engine.settings.mode = ToolMode::Eraser;
                        self.status_message = "Tool: Eraser [2/E]".to_string();
                    }
                    ui.add_space(2.0);

                    // 3. Projection [3]
                    let is_proj = self.brush_engine.settings.mode == ToolMode::Projection;
                    let proj_tex = opt_icons.map(|i| i.projection.id());
                    if render_tool_button(ui, is_proj, proj_tex, "3D", "Proj", "3D Projection Painting [3]") {
                        self.brush_engine.settings.mode = ToolMode::Projection;
                        self.status_message = "Tool: Projection Painting [3]".to_string();
                    }
                    ui.add_space(2.0);

                    // 4. Polygon Fill Tool [4 / G]
                    let is_fill = self.brush_engine.settings.mode == ToolMode::FillBucket;
                    let fill_tex = opt_icons.map(|i| i.polygon_fill.id());
                    if render_tool_button(ui, is_fill, fill_tex, "4", "Poly", "Polygon Fill Tool [4 / G]") {
                        self.brush_engine.settings.mode = ToolMode::FillBucket;
                        self.status_message = "Tool: Polygon Fill Tool [4/G]".to_string();
                    }
                    ui.add_space(2.0);

                    // 5. Smudge [5]
                    let is_smudge = self.brush_engine.settings.mode == ToolMode::Smudge;
                    let smudge_tex = opt_icons.map(|i| i.smudge.id());
                    if render_tool_button(ui, is_smudge, smudge_tex, "S", "Smudge", "Smudge Tool [5]") {
                        self.brush_engine.settings.mode = ToolMode::Smudge;
                        self.status_message = "Tool: Smudge [5]".to_string();
                    }
                    ui.add_space(2.0);

                    // 6. Clone [6]
                    let is_clone = self.brush_engine.settings.mode == ToolMode::Clone;
                    let clone_tex = opt_icons.map(|i| i.clone.id());
                    if render_tool_button(ui, is_clone, clone_tex, "C", "Clone", "Clone Stamp Tool [6]") {
                        self.brush_engine.settings.mode = ToolMode::Clone;
                        self.status_message = "Tool: Clone [6]".to_string();
                    }
                    ui.add_space(2.0);

                    // 7. Material Picker / Eyedropper [P / I]
                    let is_eye = self.brush_engine.settings.mode == ToolMode::Eyedropper;
                    let picker_tex = opt_icons.map(|i| i.picker.id());
                    if render_tool_button(ui, is_eye, picker_tex, "I", "Pick", "Material Picker / Eyedropper [P / I]") {
                        self.brush_engine.settings.mode = ToolMode::Eyedropper;
                        self.status_message = "Tool: Material Picker [P/I]".to_string();
                    }
                    ui.add_space(2.0);

                    // 8. Vector Path Tool [7 / T]
                    let is_path = self.brush_engine.settings.mode == ToolMode::Path;
                    let path_tex = opt_icons.map(|i| i.path_tool.id());
                    if render_tool_button(ui, is_path, path_tex, "7", "Path", "Vector Path Tool [7 / T]") {
                        self.brush_engine.settings.mode = ToolMode::Path;
                        self.status_message = "Tool: Path Tool [7/T]".to_string();
                    }
                    ui.add_space(2.0);

                    // 9. Decal Stamp Tool [8 / D]
                    let is_stamp = self.brush_engine.settings.mode == ToolMode::Stamp;
                    let stamp_tex = opt_icons.map(|i| i.stamp.id());
                    if render_tool_button(ui, is_stamp, stamp_tex, "8", "Stamp", "Decal Stamp Tool [8 / D]") {
                        self.brush_engine.settings.mode = ToolMode::Stamp;
                        self.status_message = "Tool: Decal Stamp Tool [8/D]".to_string();
                    }
                    ui.add_space(5.0);
                    ui.separator();
                    ui.add_space(3.0);

                    // 8. Assets / Shelf Toggle Button
                    let shelf_tex = opt_icons.map(|i| i.shelf.id());
                    if render_tool_button(ui, self.show_shelf, shelf_tex, "#", "Assets", "Toggle Left Assets Shelf") {
                        self.show_shelf = !self.show_shelf;
                    }
                });
            });
        }

        // --- SUBSTANCE 3D PAINTER LEFT ASSETS DOCK (When Docked Left) ---
        if !self.zen_mode && self.show_shelf && self.workspace.shelf_dock == ShelfDockLocation::Left {
            egui::SidePanel::left("left_assets_panel")
                .resizable(true)
                .default_width(self.workspace.left_shelf_width)
                .min_width(170.0)
                .max_width(450.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(egui::RichText::new("ASSETS").size(11.5).color(Color32::WHITE));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("X").on_hover_text("Close Assets Panel").clicked() {
                                self.show_shelf = false;
                                self.workspace.show_shelf = false;
                            }
                            if ui.small_button("↗ Pop Out").on_hover_text("Undock Shelf into floating window").clicked() {
                                self.workspace.shelf_dock = ShelfDockLocation::Floating;
                            }
                            if ui.small_button("↓ Dock Bottom").on_hover_text("Dock Shelf to Bottom Dock").clicked() {
                                self.workspace.shelf_dock = ShelfDockLocation::Bottom;
                            }
                        });
                    });
                    ui.separator();
                    self.render_assets_panel(ui);
                });
        }

        // --- RIGHT SIDE PANEL: DOCKED TEXTURE SETS, LAYERS & PROPERTIES ---
        let show_right_dock = !self.zen_mode && (
            (self.workspace.show_texture_sets && !self.workspace.texture_sets_floating) ||
            (self.workspace.show_layers && !self.workspace.layers_floating) ||
            (self.workspace.show_properties && !self.workspace.properties_floating)
        );

        if show_right_dock {
            egui::SidePanel::right("right_panel")
                .resizable(true)
                .default_width(self.workspace.right_panel_width)
                .min_width(180.0)
                .max_width(700.0)
                .show(ctx, |ui| {
                    self.workspace.right_panel_width = ui.available_width().max(180.0);

                    // 1. Texture Set List (if docked)
                    if self.workspace.show_texture_sets && !self.workspace.texture_sets_floating {
                        ui.group(|ui| {
                            ui.horizontal_wrapped(|ui| {
                                let col_btn = if self.workspace.texture_sets_collapsed { "[+]" } else { "[-]" };
                                if ui.small_button(col_btn).on_hover_text("Collapse/Expand Texture Set List").clicked() {
                                    self.workspace.texture_sets_collapsed = !self.workspace.texture_sets_collapsed;
                                }
                                ui.strong(egui::RichText::new("TEXTURE SETS").size(10.5).color(Color32::from_rgb(175, 180, 190)));
                                ui.label(egui::RichText::new(format!("[{}]", self.project.texture_sets.len())).small().color(Color32::from_rgb(226, 91, 16)));
                                ui.checkbox(&mut self.project.auto_switch_texture_set, "Auto")
                                    .on_hover_text("Automatically switch texture set on 3D viewport click");
                                if ui.small_button("↗").on_hover_text("Undock Texture Set List into floating window").clicked() {
                                    self.workspace.texture_sets_floating = true;
                                }
                            });

                            if !self.workspace.texture_sets_collapsed {
                                self.render_texture_sets_content(ui);
                            }
                        });
                        ui.add_space(3.0);
                    }

                    // 2. Layers / Mesh Maps Tabs (if docked)
                    let layers_docked = self.workspace.show_layers && !self.workspace.layers_floating;
                    let props_docked = self.workspace.show_properties && !self.workspace.properties_floating;

                    if layers_docked {
                        ui.horizontal_wrapped(|ui| {
                            let col_btn = if self.workspace.layers_collapsed { "[+]" } else { "[-]" };
                            if ui.small_button(col_btn).on_hover_text("Collapse/Expand Layers Panel").clicked() {
                                self.workspace.layers_collapsed = !self.workspace.layers_collapsed;
                            }
                            ui.selectable_value(&mut self.right_panel_tab, RightPanelTab::Layers, "Layers");
                            ui.selectable_value(&mut self.right_panel_tab, RightPanelTab::MeshMaps, "Mesh Maps");
                            if ui.small_button("↗").on_hover_text("Undock Layers Stack into floating window").clicked() {
                                self.workspace.layers_floating = true;
                            }
                        });
                        ui.separator();

                        if !self.workspace.layers_collapsed {
                            match self.right_panel_tab {
                                RightPanelTab::Layers => {
                                    self.render_layers_content(ui, Some(self.workspace.layers_pane_height));
                                }
                                RightPanelTab::MeshMaps => {
                                    self.render_mesh_maps_content(ui);
                                }
                            }
                        }
                    }

                    // 3. Resizable Splitter Handle between Layers and Properties
                    if layers_docked && props_docked && !self.workspace.layers_collapsed && !self.workspace.properties_collapsed && self.right_panel_tab == RightPanelTab::Layers {
                        let (splitter_rect, splitter_resp) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 6.0),
                            egui::Sense::click_and_drag(),
                        );
                        let splitter_resp = splitter_resp.on_hover_cursor(egui::CursorIcon::ResizeVertical);
                        if splitter_resp.dragged() {
                            self.workspace.layers_pane_height = (self.workspace.layers_pane_height + splitter_resp.drag_delta().y).clamp(120.0, 600.0);
                        }
                        if splitter_resp.double_clicked() {
                            self.workspace.layers_pane_height = 240.0;
                        }
                        let is_hover = splitter_resp.hovered() || splitter_resp.dragged();
                        let bar_color = if is_hover {
                            egui::Color32::from_rgb(243, 94, 24)
                        } else {
                            egui::Color32::from_rgb(45, 48, 56)
                        };
                        ui.painter().rect_filled(splitter_rect, 1.0, bar_color);
                        ui.add_space(2.0);
                    }

                    // 4. Properties Panel (if docked and RightPanelTab is Layers)
                    if props_docked && self.right_panel_tab == RightPanelTab::Layers {
                        ui.horizontal_wrapped(|ui| {
                            let col_btn = if self.workspace.properties_collapsed { "[+]" } else { "[-]" };
                            if ui.small_button(col_btn).on_hover_text("Collapse/Expand Properties Panel").clicked() {
                                self.workspace.properties_collapsed = !self.workspace.properties_collapsed;
                            }
                            ui.strong(egui::RichText::new("PROPERTIES").size(11.0).color(Color32::from_rgb(220, 225, 235)));
                            if ui.small_button("↗").on_hover_text("Undock Properties into floating window").clicked() {
                                self.workspace.properties_floating = true;
                            }
                        });
                        ui.separator();

                        if !self.workspace.properties_collapsed {
                            self.render_properties_content(ui);
                        }
                    }
                });
        }

        // --- SUBSTANCE 3D PAINTER BOTTOM ASSETS DOCK (Substance 3D Painter Default) ---
        if !self.zen_mode && self.show_shelf && self.workspace.shelf_dock == ShelfDockLocation::Bottom {
            egui::TopBottomPanel::bottom("bottom_shelf")
                .resizable(true)
                .default_height(self.workspace.bottom_shelf_height)
                .min_height(120.0)
                .max_height(600.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(egui::RichText::new("ASSETS & SHELF").size(11.5).color(Color32::WHITE));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("X").on_hover_text("Close Shelf").clicked() {
                                self.show_shelf = false;
                                self.workspace.show_shelf = false;
                            }
                            if ui.small_button("↗ Pop Out").on_hover_text("Undock Shelf into floating window").clicked() {
                                self.workspace.shelf_dock = ShelfDockLocation::Floating;
                            }
                            if ui.small_button("← Dock Left").on_hover_text("Dock Shelf to Left Sidebar").clicked() {
                                self.workspace.shelf_dock = ShelfDockLocation::Left;
                            }
                        });
                    });
                    ui.separator();
                    self.render_assets_panel(ui);
                });
        }

        // --- FLOATING WINDOWS (Pop-Out Mode) ---
        if !self.zen_mode {
            // 1. Floating Texture Set List
            if self.workspace.texture_sets_floating && self.workspace.show_texture_sets {
                let mut open = true;
                egui::Window::new("Texture Set List")
                    .default_size(egui::vec2(340.0, 260.0))
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            if ui.small_button("↙ Dock to Right Panel").clicked() {
                                self.workspace.texture_sets_floating = false;
                            }
                        });
                        ui.separator();
                        self.render_texture_sets_content(ui);
                    });
                if !open {
                    self.workspace.show_texture_sets = false;
                }
            }

            // 2. Floating Layers Stack
            if self.workspace.layers_floating && self.workspace.show_layers {
                let mut open = true;
                egui::Window::new("Layers Stack")
                    .default_size(egui::vec2(360.0, 420.0))
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            if ui.small_button("↙ Dock to Right Panel").clicked() {
                                self.workspace.layers_floating = false;
                            }
                        });
                        ui.separator();
                        self.render_layers_content(ui, None);
                    });
                if !open {
                    self.workspace.show_layers = false;
                }
            }

            // 3. Floating Properties Panel
            if self.workspace.properties_floating && self.workspace.show_properties {
                let mut open = true;
                let title = match self.right_panel_tab {
                    RightPanelTab::Layers => "Properties",
                    RightPanelTab::MeshMaps => "Mesh Maps & Baker",
                };
                egui::Window::new(title)
                    .default_size(egui::vec2(380.0, 520.0))
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            if ui.small_button("↙ Dock to Right Panel").clicked() {
                                self.workspace.properties_floating = false;
                            }
                            ui.separator();
                            ui.selectable_value(&mut self.right_panel_tab, RightPanelTab::Layers, "Properties");
                            ui.selectable_value(&mut self.right_panel_tab, RightPanelTab::MeshMaps, "Mesh Maps");
                        });
                        ui.separator();
                        match self.right_panel_tab {
                            RightPanelTab::Layers => self.render_properties_content(ui),
                            RightPanelTab::MeshMaps => self.render_mesh_maps_content(ui),
                        }
                    });
                if !open {
                    self.workspace.show_properties = false;
                }
            }

            // 4. Floating Assets & Shelf
            if self.workspace.shelf_dock == ShelfDockLocation::Floating && self.workspace.show_shelf {
                let mut open = true;
                egui::Window::new("Assets & Shelf")
                    .default_size(egui::vec2(680.0, 380.0))
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            if ui.small_button("↙ Dock Bottom").clicked() {
                                self.workspace.shelf_dock = ShelfDockLocation::Bottom;
                            }
                            if ui.small_button("↙ Dock Left").clicked() {
                                self.workspace.shelf_dock = ShelfDockLocation::Left;
                            }
                        });
                        ui.separator();
                        self.render_assets_panel(ui);
                    });
                if !open {
                    self.workspace.show_shelf = false;
                    self.show_shelf = false;
                }
            }
        }

        // --- CENTRAL VIEWPORT AREA ---
        egui::CentralPanel::default().show(ctx, |ui| {
            let available_size = ui.available_size();

            match self.layout {
                ViewLayout::View3DOnly => {
                    self.render_3d_viewport_ui(ui, available_size, ctx);
                }
                ViewLayout::View2DOnly => {
                    self.render_2d_uv_viewport_ui(ui, available_size, ctx);
                }
                ViewLayout::SplitDualView => {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                        let splitter_w = 6.0f32;
                        let total_w = (available_size.x - splitter_w).max(200.0);
                        let left_w = (total_w * self.split_view_ratio).clamp(120.0, (total_w - 120.0).max(120.0));
                        let right_w = (total_w - left_w).max(120.0);

                        // 1. Left Viewport (3D Viewport)
                        let left_size = egui::vec2(left_w, available_size.y);
                        ui.allocate_ui_with_layout(left_size, egui::Layout::top_down(egui::Align::Min), |left_ui| {
                            self.render_3d_viewport_ui(left_ui, left_size, ctx);
                        });

                        // 2. Interactive Draggable Splitter Handle
                        let (splitter_rect, splitter_response) = ui.allocate_exact_size(
                            egui::vec2(splitter_w, available_size.y),
                            egui::Sense::click_and_drag(),
                        );
                        let splitter_response = splitter_response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);

                        if splitter_response.dragged() {
                            let delta_x = splitter_response.drag_delta().x;
                            self.split_view_ratio = ((left_w + delta_x) / total_w).clamp(0.15, 0.85);
                            ctx.request_repaint();
                        }

                        if splitter_response.double_clicked() {
                            self.split_view_ratio = 0.5;
                            self.status_message = "Split view reset to 50/50".to_string();
                            ctx.request_repaint();
                        }

                        // Splitter visual rendering
                        let is_hovered = splitter_response.hovered();
                        let is_dragged = splitter_response.dragged();
                        let bar_color = if is_dragged {
                            egui::Color32::from_rgb(243, 94, 24) // Substance 3D orange
                        } else if is_hovered {
                            egui::Color32::from_rgb(100, 100, 110)
                        } else {
                            egui::Color32::from_rgb(35, 36, 40)
                        };
                        ui.painter().rect_filled(splitter_rect, egui::Rounding::ZERO, bar_color);

                        // Gripper notches in the center of the splitter bar
                        let mid_y = splitter_rect.center().y;
                        let notch_color = if is_hovered || is_dragged {
                            egui::Color32::from_rgb(255, 255, 255)
                        } else {
                            egui::Color32::from_rgb(80, 80, 88)
                        };
                        for i in -2..=2 {
                            let ny = mid_y + (i as f32) * 6.0;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(splitter_rect.min.x + 1.5, ny),
                                    egui::pos2(splitter_rect.max.x - 1.5, ny),
                                ],
                                egui::Stroke::new(1.0_f32, notch_color),
                            );
                        }

                        // 3. Right Viewport (2D UV Canvas)
                        let right_size = egui::vec2(right_w, available_size.y);
                        ui.allocate_ui_with_layout(right_size, egui::Layout::top_down(egui::Align::Min), |right_ui| {
                            self.render_2d_uv_viewport_ui(right_ui, right_size, ctx);
                        });
                    });
                }
            }
        });

        // --- VISUAL FILE EXPLORER MODAL (For opening 3D models or projects) ---
        if let Some(selected_path) = self.file_explorer.ui(ctx) {
            let ext = selected_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext == "fbx" || ext == "obj" || ext == "gltf" || ext == "glb" || ext == "ply" || ext == "stl" {
                let stem = selected_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Model");
                match Mesh::load_file(&selected_path) {
                    Ok(mesh) => {
                        self.project.set_mesh(mesh);
                        if let Some(wgpu) = &mut self.wgpu_viewport {
                            wgpu.update_mesh(&self.project.mesh);
                            wgpu.clear_gpu_layers();
                        }
                        self.camera.reset();
                        self.status_message = format!("Loaded custom mesh: {}", stem);
                    }
                    Err(err) => {
                        self.status_message = format!("Mesh Load Error: {}", err);
                    }
                }
            } else if ext == "paintcraft" {
                self.open_project_file(&selected_path);
            }
        }

        // --- SUBSTANCE 3D PAINTER SHELF MODALS ---
        self.render_shelf_picker_modal(ctx);
        self.render_shelf_resource_popup(ctx);

        // --- EXPORT MODAL DIALOG ---
        if self.show_export_modal {
            self.render_export_modal(ctx);
        }

        // --- SAVE PROJECT MODAL ---
        if self.show_save_modal {
            egui::Window::new("Save Project (.paintcraft)")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("Project Destination:");
                    ui.text_edit_singleline(&mut self.project_file_path);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            let path = PathBuf::from(&self.project_file_path);
                            if let Err(e) = self.project.save_to_file(&path) {
                                self.status_message = format!("Save Error: {}", e);
                            } else {
                                self.status_message = format!("Saved project to {}", self.project_file_path);
                                self.show_save_modal = false;
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_save_modal = false;
                        }
                    });
                });
        }

        // --- DEDICATED BAKING SCREEN (Substance 3D Painter Style) ---
        self.render_dedicated_baking_screen_ui(ctx);

        // --- PROCEDURAL FILTERS MODAL (Substance Adjustments Stack) ---
        if self.show_filter_modal {
            let mut close_modal = false;
            let act_idx = self.project.active_layer_idx;
            let layer_has_mask = self.project.layers.get(act_idx).map(|l| l.has_mask).unwrap_or(false);
            let layer_name = self.project.layers.get(act_idx).map(|l| l.name.clone()).unwrap_or_else(|| "None".to_string());

            egui::Window::new("Procedural Filters & Adjustments")
                .collapsible(false)
                .resizable(false)
                .default_width(420.0)
                .show(ctx, |ui| {
                    ui.label(format!("Active Layer: {}", layer_name));
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Filter Type:");
                        egui::ComboBox::from_id_source("filter_type_select")
                            .selected_text(match self.filter_type_idx {
                                0 => "Gaussian Blur",
                                1 => "Levels / Histogram",
                                2 => "Invert",
                                3 => "Sharpen",
                                4 => "Contrast & Brightness",
                                5 => "High-Pass Filter",
                                _ => "Gaussian Blur",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.filter_type_idx, 0, "Gaussian Blur");
                                ui.selectable_value(&mut self.filter_type_idx, 1, "Levels / Histogram");
                                ui.selectable_value(&mut self.filter_type_idx, 2, "Invert");
                                ui.selectable_value(&mut self.filter_type_idx, 3, "Sharpen");
                                ui.selectable_value(&mut self.filter_type_idx, 4, "Contrast & Brightness");
                                ui.selectable_value(&mut self.filter_type_idx, 5, "High-Pass Filter");
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Apply To:");
                        ui.selectable_value(&mut self.filter_target_mask, false, "Layer Channels (RGB/PBR)");
                        if layer_has_mask {
                            ui.selectable_value(&mut self.filter_target_mask, true, "Layer Mask");
                        }
                    });

                    ui.separator();

                    match self.filter_type_idx {
                        0 => {
                            ui.heading("Gaussian Blur");
                            ui.add(egui::Slider::new(&mut self.filter_blur_radius, 1.0..=32.0).text("Blur Radius (px)"));
                        }
                        1 => {
                            ui.heading("Levels Adjustment");
                            ui.add(egui::Slider::new(&mut self.filter_levels_in_black, 0.0..=1.0).text("Input Black"));
                            ui.add(egui::Slider::new(&mut self.filter_levels_in_white, 0.0..=1.0).text("Input White"));
                            ui.add(egui::Slider::new(&mut self.filter_levels_gamma, 0.1..=5.0).logarithmic(true).text("Gamma"));
                            ui.add(egui::Slider::new(&mut self.filter_levels_out_black, 0.0..=1.0).text("Output Black"));
                            ui.add(egui::Slider::new(&mut self.filter_levels_out_white, 0.0..=1.0).text("Output White"));
                        }
                        2 => {
                            ui.heading("Invert");
                            ui.label("Inverts pixel values across all active channels or mask.");
                        }
                        3 => {
                            ui.heading("Sharpen");
                            ui.add(egui::Slider::new(&mut self.filter_sharpen_strength, 0.1..=3.0).text("Strength"));
                        }
                        4 => {
                            ui.heading("Contrast & Brightness");
                            ui.add(egui::Slider::new(&mut self.filter_contrast, 0.1..=4.0).text("Contrast"));
                            ui.add(egui::Slider::new(&mut self.filter_brightness, -1.0..=1.0).text("Brightness"));
                        }
                        5 => {
                            ui.heading("High-Pass Filter");
                            ui.add(egui::Slider::new(&mut self.filter_high_pass_radius, 0.5..=20.0).text("Radius (px)"));
                        }
                        _ => {}
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Apply Filter").clicked() {
                            if let Some(layer) = self.project.layers.get_mut(act_idx) {
                                match self.filter_type_idx {
                                    0 => layer.apply_filter_blur(self.filter_blur_radius, self.filter_target_mask),
                                    1 => layer.apply_filter_levels(
                                        self.filter_levels_in_black,
                                        self.filter_levels_in_white,
                                        self.filter_levels_gamma,
                                        self.filter_levels_out_black,
                                        self.filter_levels_out_white,
                                        self.filter_target_mask,
                                    ),
                                    2 => layer.apply_filter_invert(self.filter_target_mask),
                                    3 => layer.apply_filter_sharpen(self.filter_sharpen_strength, self.filter_target_mask),
                                    4 => layer.apply_filter_contrast_brightness(self.filter_contrast, self.filter_brightness, self.filter_target_mask),
                                    5 => layer.apply_filter_high_pass(self.filter_high_pass_radius, self.filter_target_mask),
                                    _ => {}
                                }
                                self.project.rebuild_composite();
                                self.status_message = "Filter applied successfully.".to_string();
                            }
                            close_modal = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if close_modal {
                self.show_filter_modal = false;
            }
        }

        // --- VIEWPORT SETTINGS MODAL ---
        if self.show_viewport_settings_modal {
            egui::Window::new("Viewport Settings")
                .collapsible(false)
                .resizable(true)
                .default_width(380.0)
                .show(ctx, |ui| {
                    ui.heading("Camera Projection & FOV");
                    ui.horizontal(|ui| {
                        ui.label("Projection:");
                        if ui.selectable_label(!self.camera.is_orthographic, "Perspective").clicked() {
                            self.camera.is_orthographic = false;
                        }
                        if ui.selectable_label(self.camera.is_orthographic, "Orthographic").clicked() {
                            self.camera.is_orthographic = true;
                        }
                    });

                    if !self.camera.is_orthographic {
                        let mut fov_deg = self.camera.fov_y.to_degrees();
                        if ui.add(egui::Slider::new(&mut fov_deg, 20.0..=120.0).suffix("° FOV")).changed() {
                            self.camera.fov_y = fov_deg.to_radians();
                        }
                    }

                    ui.separator();
                    ui.heading("Orbit & Navigation Controls");
                    ui.add(egui::Slider::new(&mut self.camera_orbit_speed, 0.2..=3.0).text("Orbit Sensitivity"));
                    ui.add(egui::Slider::new(&mut self.camera_pan_speed, 0.2..=3.0).text("Pan Sensitivity"));
                    ui.add(egui::Slider::new(&mut self.camera_zoom_speed, 0.2..=3.0).text("Zoom Sensitivity"));

                    ui.horizontal(|ui| {
                        if ui.button("Fit Model to View (F)").clicked() {
                            self.camera.reset();
                        }
                        if ui.button("Reset Camera Default").clicked() {
                            self.camera = Camera::default();
                        }
                    });

                    ui.separator();
                    ui.heading("Environment Backdrop & HDRI");
                    ui.add(egui::Slider::new(&mut self.bg_opacity, 0.0..=1.0).text("HDRI Backdrop Opacity"));
                    ui.add(egui::Slider::new(&mut self.bg_blur, 0.0..=1.0).text("HDRI Background Blur"));

                    // HDRI Quick Preset & Asset Library Selector
                    let mut hdri_to_load = None;
                    let mut trigger_import = false;
                    let mut trigger_open_folder = false;
                    let mut trigger_refresh = false;

                    let active_name = self.asset_manager.active_entry().map(|e| e.name.clone()).unwrap_or_else(|| self.renderer.hdri.preset.name());
                    ui.horizontal(|ui| {
                        ui.label("Environment Map:");
                        egui::ComboBox::from_id_source("vp_modal_hdri_select")
                            .selected_text(&active_name)
                            .width(180.0)
                            .show_ui(ui, |ui| {
                                ui.label(egui::RichText::new("DISK LIBRARY (assets/hdris)").small().strong().color(Color32::from_rgb(140, 145, 155)));
                                for entry in &self.asset_manager.hdris {
                                    let is_active = self.renderer.hdri.custom_hdr_path.as_ref() == Some(&entry.path);
                                    let tag = if entry.is_builtin { "Built-in" } else { &entry.size_formatted() };
                                    let label = format!("{} ({})", entry.name, tag);
                                    if ui.selectable_label(is_active, label).clicked() {
                                        hdri_to_load = Some(entry.path.clone());
                                    }
                                }

                                ui.separator();
                                if ui.button("+ Import HDRI...").clicked() {
                                    trigger_import = true;
                                    ui.close_menu();
                                }
                                if ui.button("Open HDRI Folder (assets/hdris)...").clicked() {
                                    trigger_open_folder = true;
                                    ui.close_menu();
                                }
                                if ui.button("↻ Refresh Folder").clicked() {
                                    trigger_refresh = true;
                                    ui.close_menu();
                                }
                            });
                        if ui.button("Shelf...").on_hover_text("Select HDRI environment from Shelf").clicked() {
                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::HdriEnvironment));
                        }
                    });

                    if let Some(path) = hdri_to_load {
                        self.activate_hdri_file(&path);
                    }
                    if trigger_import {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("HDRI Environments", &["hdr", "png", "jpg", "jpeg", "exr"])
                            .set_title("Import HDRI to assets/hdris")
                            .pick_file()
                        {
                            if let Ok(entry) = self.asset_manager.import_hdri_file(&path) {
                                self.activate_hdri_file(&entry.path);
                                self.status_message = format!("Imported and activated HDRI: {}", entry.name);
                            }
                        }
                    }
                    if trigger_open_folder {
                        let _ = self.asset_manager.open_hdri_folder();
                    }
                    if trigger_refresh {
                        self.asset_manager.refresh_hdris();
                        self.status_message = format!("Refreshed HDRI assets: {} environments ready", self.asset_manager.hdris.len());
                    }

                    ui.horizontal(|ui| {
                        ui.label("Clear Backdrop Color:");
                        crate::color_picker::color_picker_button_rgb(ui, egui::Id::new("vp_clear_color"), &mut self.viewport_bg_color);
                        ui.label(crate::color_picker::rgb_to_hex(self.viewport_bg_color[0], self.viewport_bg_color[1], self.viewport_bg_color[2]));
                    });

                    ui.separator();
                    ui.heading("Mesh Overlays");
                    if ui.checkbox(&mut self.renderer.show_wireframe, "Wireframe Overlay (F4)").changed() {
                        self.show_uv_wireframe = self.renderer.show_wireframe;
                    }
                    if self.renderer.show_wireframe {
                        ui.horizontal(|ui| {
                            ui.label("Wireframe Line Color:");
                            let mut wire_rgb = [
                                self.shader_settings.wireframe_color[0],
                                self.shader_settings.wireframe_color[1],
                                self.shader_settings.wireframe_color[2],
                            ];
                            if crate::color_picker::color_picker_button_rgb(ui, egui::Id::new("vp_wire_color"), &mut wire_rgb) {
                                self.shader_settings.wireframe_color[0] = wire_rgb[0];
                                self.shader_settings.wireframe_color[1] = wire_rgb[1];
                                self.shader_settings.wireframe_color[2] = wire_rgb[2];
                            }
                            ui.label(crate::color_picker::rgb_to_hex(wire_rgb[0], wire_rgb[1], wire_rgb[2]));
                        });
                        ui.add(egui::Slider::new(&mut self.shader_settings.wireframe_opacity, 0.05..=1.0).text("Line Opacity"));
                    }

                    ui.separator();
                    ui.heading("Iray Monte Carlo Path Tracer (F10)");
                    if ui.checkbox(&mut self.iray_mode, "Enable Iray Path Tracer Viewport (F10)").changed() {
                        if self.iray_mode {
                            if let Some(pt) = &mut self.path_tracer {
                                pt.reset();
                            }
                        }
                    }
                    if self.iray_mode {
                        ui.add(egui::Slider::new(&mut self.iray_max_samples, 16..=1024).text("Max Samples"));
                        ui.add(egui::Slider::new(&mut self.iray_max_bounces, 1..=6).text("Max Bounces"));
                        ui.add(egui::Slider::new(&mut self.iray_samples_per_frame, 1..=8).text("Samples / Frame"));
                        if ui.checkbox(&mut self.iray_denoise, "Enable Normal-Guided Bilateral Denoiser").changed() {
                            if let Some(pt) = &mut self.path_tracer {
                                pt.denoiser_enabled = self.iray_denoise;
                                pt.update_color_buffer();
                            }
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Restart Render").clicked() {
                                if let Some(pt) = &mut self.path_tracer {
                                    pt.reset();
                                }
                            }
                            if ui.button("Save Image to PNG...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("PNG Image", &["png"])
                                    .set_file_name("paintcraft_iray_render.png")
                                    .save_file()
                                {
                                    if let Some(pt) = &self.path_tracer {
                                        match pt.save_to_png(&path) {
                                            Ok(()) => self.status_message = format!("Saved Iray render to {}", path.display()),
                                            Err(err) => self.status_message = format!("Failed to save render: {}", err),
                                        }
                                    }
                                }
                            }
                        });
                    }

                    ui.separator();
                    ui.heading("Rendering & Diagnostics");
                    ui.checkbox(&mut self.continuous_60fps, "Continuous 60+ FPS Rendering")
                        .on_hover_text("Forces continuous frame repaints for game-engine smoothness instead of reactive on-demand rendering.");
                    ui.checkbox(&mut self.perf_metrics.show_debug_hud, "Performance Diagnostics HUD (F3)")
                        .on_hover_text("Toggles the real-time throughput, work latency, and texture transfer diagnostics HUD.");

                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        self.show_viewport_settings_modal = false;
                    }
                });
        }

        // --- SHADER & POST-PROCESSING SETTINGS MODAL ---
        if self.show_shader_settings_modal {
            egui::Window::new("Shader & Post-Processing Settings")
                .collapsible(false)
                .resizable(true)
                .default_width(440.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                        ui.group(|ui| {
                            ui.strong("Shader Performance Presets (Instant GPU Profile):");
                            let presets = [
                                (crate::wgpu_renderer::ShaderPerformancePreset::DynamicInteraction, "Dynamic (Fast Drag / Crisp Idle)"),
                                (crate::wgpu_renderer::ShaderPerformancePreset::BalancedPbr, "Balanced PBR (Fast Viewport, 60 FPS)"),
                                (crate::wgpu_renderer::ShaderPerformancePreset::MaximumPerformance, "Max Performance (Post-FX Off, Ultra-Fast)"),
                                (crate::wgpu_renderer::ShaderPerformancePreset::UltraQuality, "Ultra Quality (Bilateral SSAO, Full Post-FX)"),
                            ];
                            for (p, label) in presets {
                                let is_sel = self.shader_preset == p;
                                if ui.selectable_label(is_sel, label).clicked() {
                                    self.shader_preset = p;
                                    self.shader_settings.apply_preset(p);
                                    self.status_message = format!("Applied Preset: {}", p.name());
                                }
                            }
                        });
                        ui.add_space(6.0);

                        ui.collapsing("Lighting & Environment", |ui| {
                            ui.add(egui::Slider::new(&mut self.shader_settings.sun_intensity, 0.0..=5.0).text("Direct Sun Intensity"));
                            ui.add(egui::Slider::new(&mut self.shader_settings.env_intensity, 0.0..=5.0).text("HDRI IBL Intensity"));
                            ui.add(egui::Slider::new(&mut self.shader_settings.ao_intensity, 0.0..=3.0).text("Ambient Occlusion Strength"));
                            ui.add(egui::Slider::new(&mut self.shader_settings.exposure, 0.1..=4.0).text("Camera Exposure"));
                        });

                        ui.collapsing("PBR Shading Parameters", |ui| {
                            ui.add(egui::Slider::new(&mut self.shader_settings.specular_mult, 0.0..=2.5).text("Specular Multiplier"));
                            ui.add(egui::Slider::new(&mut self.shader_settings.roughness_bias, -0.5..=0.5).text("Roughness Bias"));

                            ui.horizontal(|ui| {
                                ui.label("Normal Convention:");
                                if ui.selectable_label(self.shader_settings.normal_format == 0, "OpenGL (+Y)").on_hover_text("OpenGL standard (+Y Green) - Substance Painter, Blender, Maya, glTF").clicked() {
                                    self.shader_settings.normal_format = 0;
                                }
                                if ui.selectable_label(self.shader_settings.normal_format == 1, "DirectX (-Y)").on_hover_text("DirectX standard (-Y Inverted Green) - Unreal Engine").clicked() {
                                    self.shader_settings.normal_format = 1;
                                }
                            });

                            egui::ComboBox::from_id_source("tonemap_mode_combo")
                                .selected_text(match self.shader_settings.tonemap_mode {
                                    0 => "Exposure / Reinhard (Neutral)",
                                    1 => "ACES Filmic (Substance Industry Standard)",
                                    2 => "AgX (Blender 4.0 Standard)",
                                    _ => "Linear / Uncompressed",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.shader_settings.tonemap_mode, 1, "ACES Filmic (Substance Industry Standard)");
                                    ui.selectable_value(&mut self.shader_settings.tonemap_mode, 2, "AgX (Blender 4.0 Standard)");
                                    ui.selectable_value(&mut self.shader_settings.tonemap_mode, 0, "Exposure / Reinhard (Neutral)");
                                    ui.selectable_value(&mut self.shader_settings.tonemap_mode, 3, "Linear / Uncompressed");
                                });
                        });

                        ui.collapsing("Translucency, Refraction & Opacity (IOR)", |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Mode:");
                                egui::ComboBox::from_id_source("transparency_mode_combo")
                                    .selected_text(self.shader_settings.transparency_mode.name())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut self.shader_settings.transparency_mode,
                                            crate::wgpu_renderer::TransparencyMode::Dithered,
                                            crate::wgpu_renderer::TransparencyMode::Dithered.name(),
                                        );
                                        ui.selectable_value(
                                            &mut self.shader_settings.transparency_mode,
                                            crate::wgpu_renderer::TransparencyMode::AlphaCutout,
                                            crate::wgpu_renderer::TransparencyMode::AlphaCutout.name(),
                                        );
                                        ui.selectable_value(
                                            &mut self.shader_settings.transparency_mode,
                                            crate::wgpu_renderer::TransparencyMode::AlphaBlend,
                                            crate::wgpu_renderer::TransparencyMode::AlphaBlend.name(),
                                        );
                                        ui.selectable_value(
                                            &mut self.shader_settings.transparency_mode,
                                            crate::wgpu_renderer::TransparencyMode::Refraction,
                                            crate::wgpu_renderer::TransparencyMode::Refraction.name(),
                                        );
                                        ui.selectable_value(
                                            &mut self.shader_settings.transparency_mode,
                                            crate::wgpu_renderer::TransparencyMode::Opaque,
                                            crate::wgpu_renderer::TransparencyMode::Opaque.name(),
                                        );
                                    });
                            });

                            if self.shader_settings.transparency_mode == crate::wgpu_renderer::TransparencyMode::AlphaCutout {
                                ui.horizontal(|ui| {
                                    ui.label("Alpha Cutoff:");
                                    ui.add(egui::Slider::new(&mut self.shader_settings.alpha_cutoff, 0.01..=0.99));
                                });
                            }

                            ui.separator();
                            ui.label(egui::RichText::new("Physical Refraction & Optics (IOR):").strong());

                            ui.horizontal(|ui| {
                                ui.label("Index of Refraction:");
                                ui.add(egui::Slider::new(&mut self.shader_settings.ior, 1.00..=3.00).step_by(0.01));
                            });

                            ui.horizontal_wrapped(|ui| {
                                ui.label("Presets:");
                                if ui.small_button("Air 1.00").clicked() { self.shader_settings.ior = 1.00; }
                                if ui.small_button("Water 1.33").clicked() { self.shader_settings.ior = 1.333; }
                                if ui.small_button("Acrylic 1.49").clicked() { self.shader_settings.ior = 1.49; }
                                if ui.small_button("Glass 1.52").clicked() { self.shader_settings.ior = 1.52; }
                                if ui.small_button("Flint 1.62").clicked() { self.shader_settings.ior = 1.62; }
                                if ui.small_button("Sapphire 1.77").clicked() { self.shader_settings.ior = 1.77; }
                                if ui.small_button("Diamond 2.42").clicked() { self.shader_settings.ior = 2.417; }
                            });

                            ui.horizontal(|ui| {
                                ui.label("Chromatic Dispersion:");
                                ui.add(egui::Slider::new(&mut self.shader_settings.refraction_dispersion, 0.0..=0.08).step_by(0.005))
                                    .on_hover_text("Wavelength-dependent ray splitting (Cauchy dispersion) creating rainbow prismatic color fringing.");
                            });

                            ui.horizontal(|ui| {
                                ui.label("Absorption Depth (Beer-Lambert):");
                                ui.add(egui::Slider::new(&mut self.shader_settings.translucency_depth, 0.0..=5.0).step_by(0.1))
                                    .on_hover_text("Controls physical Beer-Lambert light attenuation through colored translucent media.");
                            });

                            ui.separator();
                            ui.checkbox(&mut self.shader_settings.double_sided, "Double-Sided Rendering (Two-Sided Shading)")
                                .on_hover_text("Renders both front and back faces with flipped normals on backfaces. Essential for wire mesh, cloth tears, and transparent glass visors.");

                            match self.shader_settings.transparency_mode {
                                crate::wgpu_renderer::TransparencyMode::Refraction => {
                                    ui.label(egui::RichText::new("Physical Refraction & Translucency. Bends background and environment light through geometry using Snell's Law with physical IOR, chromatic dispersion, and Beer-Lambert absorption.").small().color(Color32::from_rgb(160, 165, 175)));
                                }
                                crate::wgpu_renderer::TransparencyMode::Dithered => {
                                    ui.label(egui::RichText::new("Bayer 8x8 screen-door dithering provides order-independent gradual semi-transparency with physical IOR and proper depth testing.").small().color(Color32::from_rgb(160, 165, 175)));
                                }
                                crate::wgpu_renderer::TransparencyMode::AlphaCutout => {
                                    ui.label(egui::RichText::new("Substance 3D Painter alpha cutout discarding. Fragments with opacity below the cutoff threshold are discarded.").small().color(Color32::from_rgb(160, 165, 175)));
                                }
                                crate::wgpu_renderer::TransparencyMode::AlphaBlend => {
                                    ui.label(egui::RichText::new("Translucent glass alpha blending against the viewport skybox and backdrop with physical IOR refraction.").small().color(Color32::from_rgb(160, 165, 175)));
                                }
                                crate::wgpu_renderer::TransparencyMode::Opaque => {
                                    ui.label(egui::RichText::new("Transparency disabled in viewport. The surface is rendered fully opaque with IOR-derived physical dielectric F0.").small().color(Color32::from_rgb(160, 165, 175)));
                                }
                            }
                        });

                        ui.collapsing("Environment Specular Occlusion (PBR)", |ui| {
                            ui.label(egui::RichText::new("Analytical Specular Occlusion is active. Prevents environment reflections from leaking into deep crevices and seams without noisy screen-space raymarching streaks.").small().color(Color32::from_rgb(160, 165, 175)));
                        });

                        ui.collapsing("Screen-Space Ambient Occlusion (SSAO)", |ui| {
                            ui.checkbox(&mut self.shader_settings.ssao_enabled, "Enable Screen-Space Ambient Occlusion");
                            if self.shader_settings.ssao_enabled {
                                ui.add(egui::Slider::new(&mut self.shader_settings.ssao_intensity, 0.0..=2.5).text("SSAO Intensity"));
                                ui.add(egui::Slider::new(&mut self.shader_settings.ssao_radius, 0.1..=2.0).text("Sample Radius"));
                            }
                            ui.label(egui::RichText::new("Substance Painter real-time contact shadowing and crevice darkening in the viewport.").small().color(Color32::from_rgb(160, 165, 175)));
                        });

                        ui.collapsing("Directional Sun Shadows (PCF)", |ui| {
                            ui.checkbox(&mut self.shader_settings.shadows_enabled, "Enable Real-Time Directional Shadows");
                            if self.shader_settings.shadows_enabled {
                                ui.add(egui::Slider::new(&mut self.shader_settings.shadow_softness, 0.2..=3.0).text("Shadow Penumbra Softness"));
                                ui.add(egui::Slider::new(&mut self.shader_settings.shadow_bias, 0.0005..=0.015).text("Normal Offset Bias"));
                            }
                            ui.label(egui::RichText::new("2048x2048 depth shadow map with 16-tap Poisson disk percentage-closer filtering (PCF) and slope-scaled normal offset bias.").small().color(Color32::from_rgb(160, 165, 175)));
                        });

                        ui.collapsing("Viewport Denoiser (Joint-Bilateral Filter)", |ui| {
                            ui.checkbox(&mut self.shader_settings.denoiser_enabled, "Enable Screen-Space Joint-Bilateral Denoiser");
                            if self.shader_settings.denoiser_enabled {
                                ui.add(egui::Slider::new(&mut self.shader_settings.denoiser_radius, 0.5..=5.0).text("Filter Radius"));
                                ui.add(egui::Slider::new(&mut self.shader_settings.denoiser_normal_weight, 1.0..=32.0).text("Normal Edge Preservation"));
                                ui.add(egui::Slider::new(&mut self.shader_settings.denoiser_depth_weight, 0.01..=0.5).text("Depth Silhouette Preservation"));
                            }
                            ui.label(egui::RichText::new("8-tap screen-space normal and depth-guided cross-bilateral filter. Eliminates ray-marched noise and dither from SSAO and reflections while keeping geometric edges razor sharp.").small().color(Color32::from_rgb(160, 165, 175)));
                        });

                        ui.collapsing("Anti-Aliasing (FXAA & NVIDIA DLAA)", |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Mode:");
                                egui::ComboBox::from_id_source("aa_mode_select")
                                    .selected_text(self.shader_settings.aa_mode.name())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.shader_settings.aa_mode, crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa, crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa.name());
                                        ui.selectable_value(&mut self.shader_settings.aa_mode, crate::nvidia_dlaa::AntiAliasingMode::Fxaa, crate::nvidia_dlaa::AntiAliasingMode::Fxaa.name());
                                        ui.selectable_value(&mut self.shader_settings.aa_mode, crate::nvidia_dlaa::AntiAliasingMode::Off, crate::nvidia_dlaa::AntiAliasingMode::Off.name());
                                    });
                            });
                            self.shader_settings.fxaa_enabled = self.shader_settings.aa_mode == crate::nvidia_dlaa::AntiAliasingMode::Fxaa;

                            if self.shader_settings.aa_mode == crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa {
                                if let Some(vp) = &self.wgpu_viewport {
                                    let hw = &vp.dlaa_manager.hardware_info;
                                    let hw_text = if hw.tensor_cores_supported {
                                        format!("NVIDIA Architecture: {} | Tensor Cores DLAA Ready", hw.device_name)
                                    } else if hw.is_nvidia {
                                        format!("NVIDIA GPU: {} | Halton Temporal Reconstruction", hw.device_name)
                                    } else {
                                        format!("GPU: {} | Halton Temporal Reconstruction", hw.device_name)
                                    };
                                    ui.label(egui::RichText::new(hw_text).small().color(Color32::from_rgb(110, 220, 140)));
                                    if hw.ngx_library_found {
                                        ui.label(egui::RichText::new("Linux NVIDIA NGX Driver Core: Active").small().color(Color32::from_rgb(120, 200, 255)));
                                    }
                                }
                                ui.add(egui::Slider::new(&mut self.shader_settings.history_blend_weight, 0.5..=0.98).text("History Temporal Weight"));
                                if ui.button("Reset Temporal History").clicked() {
                                    if let Some(vp) = &mut self.wgpu_viewport {
                                        vp.dlaa_manager.reset_history();
                                    }
                                }
                                ui.label(egui::RichText::new("NVIDIA DLAA applies 16-phase radical inverse Halton subpixel jitter with Catmull-Rom variance color clipping to produce native ultra-crisp edges without aliasing artifacts.").small().color(Color32::from_rgb(160, 165, 175)));
                            } else if self.shader_settings.aa_mode == crate::nvidia_dlaa::AntiAliasingMode::Fxaa {
                                ui.label(egui::RichText::new("Fast Approximate Anti-Aliasing (FXAA) screen-space luminance contrast edge smoothing.").small().color(Color32::from_rgb(160, 165, 175)));
                            } else {
                                ui.label(egui::RichText::new("Anti-aliasing disabled. Pixels rendered at 1:1 raw sample rate.").small().color(Color32::from_rgb(160, 165, 175)));
                            }
                        });

                        ui.collapsing("Bloom / Specular Glow", |ui| {
                            ui.checkbox(&mut self.shader_settings.bloom_enabled, "Enable Bloom");
                            if self.shader_settings.bloom_enabled {
                                ui.add(egui::Slider::new(&mut self.shader_settings.bloom_intensity, 0.1..=3.0).text("Intensity"));
                                ui.add(egui::Slider::new(&mut self.shader_settings.bloom_threshold, 0.2..=2.0).text("Luminance Threshold"));
                                ui.add(egui::Slider::new(&mut self.shader_settings.bloom_radius, 0.5..=5.0).text("Blur Radius"));
                            }
                        });

                        ui.collapsing("Depth of Field (DoF)", |ui| {
                            ui.checkbox(&mut self.shader_settings.dof_enabled, "Enable Depth of Field");
                            if self.shader_settings.dof_enabled {
                                ui.add(egui::Slider::new(&mut self.shader_settings.dof_focus_distance, 0.2..=10.0).text("Focus Distance"));
                                ui.add(egui::Slider::new(&mut self.shader_settings.dof_aperture, 0.1..=5.0).text("Aperture / CoC Width"));
                                ui.add(egui::Slider::new(&mut self.shader_settings.dof_max_blur, 0.1..=2.0).text("Bokeh Blur Amount"));
                            }
                        });

                        ui.collapsing("Camera Lens Effects", |ui| {
                            ui.add(egui::Slider::new(&mut self.shader_settings.chromatic_aberration, 0.0..=2.0).text("Chromatic Aberration"));
                            ui.add(egui::Slider::new(&mut self.shader_settings.vignette_strength, 0.0..=1.5).text("Vignette"));
                        });

                        ui.collapsing("Viewport Performance & Frame Pacing", |ui| {
                            ui.checkbox(&mut self.shader_settings.adaptive_resolution, "Adaptive Resolution during Interaction");
                            if self.shader_settings.adaptive_resolution {
                                ui.add(egui::Slider::new(&mut self.shader_settings.adaptive_scale, 0.5..=0.95).text("Downsampling Scale"));
                            }
                            ui.label(egui::RichText::new("Smooths viewport interaction while keeping 60 FPS texturing response.").small().color(Color32::from_rgb(160, 165, 175)));
                        });
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Reset Defaults").clicked() {
                            self.shader_settings = crate::wgpu_renderer::ShaderSettings::default();
                        }
                        if ui.button("Close").clicked() {
                            self.show_shader_settings_modal = false;
                        }
                    });
                });
        }

        // --- SUBSTANCE 3D PAINTER NEW PROJECT / STARTUP MODAL ---
        if self.show_new_project_modal {
            egui::Window::new("New Project - 3D Paintcraft Studio")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .fixed_size(egui::vec2(550.0, 520.0))
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);

                    ui.heading("New Project Configuration");
                    ui.label(egui::RichText::new("Configure 3D mesh model, document resolution, and texturing workflow.").small().color(Color32::from_rgb(160, 165, 175)));
                    ui.separator();

                    // Quick Sample Models Row
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Quick Sample:").strong());
                        let samples = [
                            (0, "Shader Ball"),
                            (2, "Sci-Fi Crate"),
                            (3, "Robot Head"),
                            (1, "Rounded Cube"),
                        ];
                        for (idx, name) in samples {
                            let is_selected = self.new_proj_mesh_type == 0 && self.new_proj_primitive_idx == idx;
                            if ui.selectable_label(is_selected, name).clicked() {
                                self.new_proj_mesh_type = 0;
                                self.new_proj_primitive_idx = idx;
                                if self.project_name_edit == "Untitled Project" || self.project_name_edit.is_empty() {
                                    self.project_name_edit = name.to_string();
                                }
                            }
                        }
                    });

                    // Recent Projects Row (if any exist)
                    if !self.recent_projects.is_empty() {
                        let mut opened_recent = None;
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Recent:").strong());
                            for p in self.recent_projects.iter().take(3) {
                                let path_obj = std::path::Path::new(p);
                                let name = path_obj.file_name().and_then(|n| n.to_str()).unwrap_or(p);
                                if ui.small_button(name).on_hover_text(p).clicked() {
                                    opened_recent = Some(p.clone());
                                }
                            }
                        });
                        if let Some(recent_to_open) = opened_recent {
                            self.open_project_file(std::path::Path::new(&recent_to_open));
                            self.show_new_project_modal = false;
                        }
                    }

                    ui.horizontal(|ui| {
                        ui.label("Project Name:");
                        ui.add(egui::TextEdit::singleline(&mut self.project_name_edit).desired_width(280.0));
                    });

                    // Group 1: Template
                    ui.group(|ui| {
                        ui.strong("Template:");
                        egui::ComboBox::from_id_source("new_proj_template_combo")
                            .selected_text(match self.new_proj_template {
                                0 => "PBR - Metallic Roughness (starter set)",
                                1 => "PBR - Metallic Roughness (Alpha Blend)",
                                2 => "PBR - Specular Glossiness",
                                3 => "Unreal Engine 5 (ORM Packed)",
                                _ => "PBR - Metallic Roughness",
                            })
                            .width(ui.available_width() - 20.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.new_proj_template, 0, "PBR - Metallic Roughness (starter set)");
                                ui.selectable_value(&mut self.new_proj_template, 1, "PBR - Metallic Roughness (Alpha Blend)");
                                ui.selectable_value(&mut self.new_proj_template, 2, "PBR - Specular Glossiness");
                                ui.selectable_value(&mut self.new_proj_template, 3, "Unreal Engine 5 (ORM Packed)");
                            });
                    });

                    // Group 2: File (3D Mesh)
                    ui.group(|ui| {
                        ui.strong("File (3D Mesh):");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.new_proj_mesh_type, 0, "Built-in Sample Mesh");
                            ui.selectable_value(&mut self.new_proj_mesh_type, 1, "Custom Wavefront (.obj)");
                        });

                        if self.new_proj_mesh_type == 0 {
                            ui.horizontal(|ui| {
                                ui.label("Sample Mesh:");
                                egui::ComboBox::from_id_source("new_proj_prim_select")
                                    .selected_text(match self.new_proj_primitive_idx {
                                        0 => "Shader Ball (Material Sample)",
                                        1 => "Rounded Cube",
                                        2 => "Sci-Fi Crate",
                                        3 => "Robot Head",
                                        4 => "Cylinder",
                                        5 => "Torus",
                                        _ => "Shader Ball",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.new_proj_primitive_idx, 0, "Shader Ball (Material Sample)");
                                        ui.selectable_value(&mut self.new_proj_primitive_idx, 1, "Rounded Cube");
                                        ui.selectable_value(&mut self.new_proj_primitive_idx, 2, "Sci-Fi Crate");
                                        ui.selectable_value(&mut self.new_proj_primitive_idx, 3, "Robot Head");
                                        ui.selectable_value(&mut self.new_proj_primitive_idx, 4, "Cylinder");
                                        ui.selectable_value(&mut self.new_proj_primitive_idx, 5, "Torus");
                                    });
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.new_proj_custom_path).hint_text("/path/to/model.obj").desired_width(320.0));
                                if ui.button("Browse...").clicked() {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("3D Models (FBX, OBJ, glTF, GLB, PLY, STL)", &["fbx", "obj", "gltf", "glb", "ply", "stl"])
                                        .set_title("Select 3D Mesh Model")
                                        .pick_file()
                                    {
                                        self.new_proj_custom_path = path.to_string_lossy().to_string();
                                    }
                                }
                            });
                        }
                    });

                    // Group 3: Document Resolution
                    ui.group(|ui| {
                        ui.strong("Document Resolution:");
                        ui.horizontal(|ui| {
                            let res_options = [
                                (512, "512 x 512"),
                                (1024, "1024 (1K)"),
                                (2048, "2048 (2K - Default)"),
                                (4096, "4096 (4K Ultra)"),
                            ];
                            for (res, label) in res_options {
                                if ui.selectable_label(self.new_proj_resolution == res, label).clicked() {
                                    self.new_proj_resolution = res;
                                }
                            }
                        });
                    });

                    // Group 4: Normal Map Format
                    ui.group(|ui| {
                        ui.strong("Normal Map Format:");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.new_proj_normal_format, 0, "OpenGL (+Y Standard)")
                                .on_hover_text("Substance Painter, Blender, Maya, glTF standard");
                            ui.selectable_value(&mut self.new_proj_normal_format, 1, "DirectX (-Y Inverted)")
                                .on_hover_text("Unreal Engine standard");
                        });
                    });

                    // Group 5: Mesh Maps Baking on Creation
                    ui.group(|ui| {
                        ui.checkbox(&mut self.new_proj_auto_bake, "Auto-Bake Mesh Maps on Creation (Curvature, AO, Tangent Normals)");
                        if self.new_proj_auto_bake {
                            ui.horizontal(|ui| {
                                ui.label("Bake Resolution:");
                                ui.selectable_value(&mut self.new_proj_bake_res, 1024, "1024 (1K)");
                                ui.selectable_value(&mut self.new_proj_bake_res, 2048, "2048 (2K)");
                            });
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Dialog Actions (Start Texturing / Open / Cancel)
                    ui.horizontal(|ui| {
                        let ok_btn = egui::Button::new(egui::RichText::new("Start Texturing").strong().color(Color32::WHITE))
                            .fill(Color32::from_rgb(226, 91, 16))
                            .min_size(egui::vec2(160.0, 32.0));
                        if ui.add(ok_btn).clicked() {
                            self.create_project_from_dialog();
                        }

                        let open_btn = egui::Button::new("Open Existing...").min_size(egui::vec2(120.0, 32.0));
                        if ui.add(open_btn).clicked() {
                            self.file_explorer = FileExplorerDialog::new_open_project();
                            self.file_explorer.open();
                            self.show_new_project_modal = false;
                        }

                        let cancel_btn = egui::Button::new("Cancel").min_size(egui::vec2(70.0, 32.0));
                        if ui.add(cancel_btn).clicked() {
                            self.show_new_project_modal = false;
                        }
                    });
                });
        }

        // --- NEW CUSTOM MATERIAL MODAL ---
        if self.show_new_material_modal {
            let mut close_modal = false;
            let mut save_material_req = false;

            egui::Window::new("Create New Custom PBR Material")
                .collapsible(false)
                .resizable(false)
                .default_width(340.0)
                .show(ctx, |ui| {
                    ui.heading("New PBR Material");
                    ui.label("Configure material attributes to save into your custom library.");
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.new_material_name);
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Base Color:");
                        ui.color_edit_button_rgba_unmultiplied(&mut self.new_material_color);
                    });

                    ui.add_space(4.0);
                    ui.add(egui::Slider::new(&mut self.new_material_roughness, 0.0..=1.0).text("Roughness"));
                    ui.add(egui::Slider::new(&mut self.new_material_metallic, 0.0..=1.0).text("Metallic"));
                    ui.add(egui::Slider::new(&mut self.new_material_height, 0.0..=1.0).text("Height"));
                    ui.add(egui::Slider::new(&mut self.new_material_opacity, 0.0..=1.0).text("Opacity"));
                    ui.add(egui::Slider::new(&mut self.new_material_clearcoat, 0.0..=1.0).text("Clearcoat"));
                    ui.add(egui::Slider::new(&mut self.new_material_anisotropy, 0.0..=1.0).text("Anisotropy"));

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        let save_btn = egui::Button::new(egui::RichText::new("Save Material").strong().color(Color32::WHITE))
                            .fill(Color32::from_rgb(45, 140, 255));
                        if ui.add(save_btn).clicked() {
                            save_material_req = true;
                            close_modal = true;
                        }

                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if save_material_req {
                let name = if self.new_material_name.trim().is_empty() {
                    format!("Material_{}", self.asset_manager.materials.len() + 1)
                } else {
                    self.new_material_name.trim().to_string()
                };
                let clean_id = name.replace(' ', "_").to_lowercase();
                let mat = crate::asset_manager::CustomMaterialEntry {
                    id: clean_id,
                    name: name.clone(),
                    description: format!("Custom user PBR material: {}", name),
                    path: std::path::PathBuf::new(),
                    file_name: String::new(),
                    base_color: self.new_material_color,
                    roughness: self.new_material_roughness,
                    metallic: self.new_material_metallic,
                    height: self.new_material_height,
                    normal: [0.5, 0.5, 1.0, 0.5],
                    emissive: [self.new_material_emissive[0], self.new_material_emissive[1], self.new_material_emissive[2], 1.0],
                    opacity: self.new_material_opacity,
                    clearcoat: self.new_material_clearcoat,
                    anisotropy: self.new_material_anisotropy,
                };
                match self.asset_manager.save_material(&mat) {
                    Ok(path) => {
                        self.status_message = format!("Created and saved Material '{}' to {:?}", mat.name, path.file_name().unwrap_or_default());
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to save material: {}", e);
                    }
                }
            }

            if close_modal {
                self.show_new_material_modal = false;
            }
        }

        // --- NEW CUSTOM DECAL PRESET MODAL ---
        if self.show_new_decal_modal {
            let mut close_modal = false;
            let mut save_decal_req = false;

            egui::Window::new("Save Custom Decal / Path Preset")
                .collapsible(false)
                .resizable(false)
                .default_width(340.0)
                .show(ctx, |ui| {
                    ui.heading("Save Custom Decal Preset");
                    ui.label("Save current decal parameters into your assets library for single-click stamping or vector paths.");
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.new_decal_name);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Description:");
                        ui.text_edit_singleline(&mut self.new_decal_description);
                    });

                    ui.add_space(6.0);
                    let is_stamp = self.brush_engine.settings.mode == ToolMode::Stamp;
                    let style = if is_stamp { self.brush_engine.settings.stamp_tool.stroke_style } else { self.brush_engine.settings.path_tool.stroke_style };
                    let decal = if is_stamp { self.brush_engine.settings.stamp_tool.decal_type } else { self.brush_engine.settings.path_tool.decal_type };
                    let depth = if is_stamp { self.brush_engine.settings.stamp_tool.height_depth } else { self.brush_engine.settings.path_tool.height_depth };
                    let offset = if is_stamp { self.brush_engine.settings.stamp_tool.surface_offset } else { self.brush_engine.settings.path_tool.surface_offset };

                    ui.label(format!("Style: {} | Decal: {}", style.name(), decal.name()));
                    ui.label(format!("Radius: {:.0}px | Depth: {:.2} | Offset: {:.3}", self.brush_engine.settings.radius, depth, offset));

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        let save_btn = egui::Button::new(egui::RichText::new("Save Decal Preset").strong().color(Color32::WHITE))
                            .fill(Color32::from_rgb(255, 140, 30));
                        if ui.add(save_btn).clicked() {
                            save_decal_req = true;
                            close_modal = true;
                        }

                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if save_decal_req {
                let name = if self.new_decal_name.trim().is_empty() {
                    format!("Decal_{}", self.asset_manager.decals.len() + 1)
                } else {
                    self.new_decal_name.trim().to_string()
                };
                let clean_id = name.replace(' ', "_").to_lowercase();
                let (style, decal, rad, spacing_r, decal_sp, h_depth, heat, offset) = if self.brush_engine.settings.mode == ToolMode::Stamp {
                    let st = &self.brush_engine.settings.stamp_tool;
                    (st.stroke_style, st.decal_type, self.brush_engine.settings.radius, 0.4, 1.2, st.height_depth, st.heat_tint, st.surface_offset)
                } else {
                    let pt = &self.brush_engine.settings.path_tool;
                    (pt.stroke_style, pt.decal_type, self.brush_engine.settings.radius, pt.spacing_ratio, pt.decal_spacing, pt.height_depth, pt.heat_tint, pt.surface_offset)
                };
                let preset = crate::asset_manager::CustomDecalPreset {
                    id: clean_id,
                    name: name.clone(),
                    description: self.new_decal_description.clone(),
                    path: std::path::PathBuf::new(),
                    file_name: String::new(),
                    stroke_style: style,
                    decal_type: decal,
                    radius: rad,
                    spacing_ratio: spacing_r,
                    decal_spacing: decal_sp,
                    height_depth: h_depth,
                    base_color: [self.brush_engine.settings.paint_color.x, self.brush_engine.settings.paint_color.y, self.brush_engine.settings.paint_color.z, self.brush_engine.settings.paint_color.w],
                    roughness: self.brush_engine.settings.paint_roughness,
                    metallic: self.brush_engine.settings.paint_metallic,
                    heat_tint: heat,
                    surface_offset: offset,
                    emissive: [self.brush_engine.settings.paint_emissive.x, self.brush_engine.settings.paint_emissive.y, self.brush_engine.settings.paint_emissive.z, self.brush_engine.settings.paint_emissive.w],
                    opacity: self.brush_engine.settings.paint_opacity,
                    clearcoat: self.brush_engine.settings.paint_clearcoat,
                    anisotropy: self.brush_engine.settings.paint_anisotropy,
                };
                match self.asset_manager.save_decal(&preset) {
                    Ok(path) => {
                        self.status_message = format!("Created and saved Decal Preset '{}' to {:?}", preset.name, path.file_name().unwrap_or_default());
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to save decal preset: {}", e);
                    }
                }
            }

            if close_modal {
                self.show_new_decal_modal = false;
            }
        }

        // --- ABOUT MODAL ---
        if self.show_about_modal {
            egui::Window::new("About 3D Paintcraft")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.heading("3D Paintcraft");
                    ui.label("High-performance PBR texture painting and mesh map baking suite written in pure Rust.");
                    ui.label("Cross-platform support for Windows and Linux.");
                    ui.separator();
                    ui.label("Features:");
                    ui.label("• Real-time BVH-accelerated 3D Projection Painting");
                    ui.label("• Cook-Torrance GGX Shading with TBN Normal Mapping & HDRI IBL");
                    ui.label("• Selectable HDRI Environments (Studio Neutral, Tomoko, Sunset, Courtyard, Cyberpunk)");
                    ui.label("• Perspective-Correct Rasterization & Orthographic Camera Mode");
                    ui.label("• Integrated Visual File Explorer for 3D Models and Projects");
                    ui.label("• Substance 3D Painter UI Layout with Left Toolbar and Bottom Shelf");
                    ui.label("• Grayscale Layer Masks (Black, White, Curvature Edge Wear)");
                    ui.label("• Non-Destructive Undo / Redo History (Ctrl+Z / Ctrl+Y)");
                    ui.label("• Unreal Engine ORM, Unity, and Blender/glTF PBR Exporters");
                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        self.show_about_modal = false;
                    }
                });
        }

        // --- GITHUB UPDATE NOTIFIER DIALOG ---
        self.updater.render_dialog(ctx);

        // --- DEFERRED CPU COMPOSITING SYNC ON POINTER RELEASE ---
        if self.need_cpu_composite_sync && !ctx.input(|i| i.pointer.any_down()) {
            self.project.rebuild_composite();
            self.update_uv_texture(ctx);
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.sync_textures(&mut self.project);
            }
            self.need_cpu_composite_sync = false;
            ctx.request_repaint();
        }

        self.perf_metrics.work_latency_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        if self.continuous_60fps {
            ctx.request_repaint();
        }
    }
}
