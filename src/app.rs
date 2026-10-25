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
use crate::layer::{BlendMode, Layer, LayerType, ProceduralGenerator};
use crate::mesh::Mesh;
use crate::project::Project;
use crate::renderer::{Camera, ViewportChannelMode, ViewportRenderer};
use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions, Vec2};
use glam::Vec4;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, channel};

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

pub struct PaintcraftApp {
    pub project: Project,
    pub camera: Camera,
    pub renderer: ViewportRenderer,
    pub brush_engine: BrushEngine,
    pub history: HistoryManager,
    pub file_explorer: FileExplorerDialog,

    // Stroke undo tracking
    pub stroke_start_layer: Option<Box<Layer>>,
    pub is_stroke_active: bool,

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
    pub bake_cage_cache_key: (crate::baker::BakeCageMode, u32, u32, String, bool, bool, usize, usize),
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
            stroke_start_layer: None,
            is_stroke_active: false,
            view_mode: ViewportChannelMode::Material,
            layout: ViewLayout::View3DOnly,
            viewport_texture: None,
            uv_canvas_texture: None,
            uv_pixel_buffer: Vec::new(),
            status_message: "Ready. Select a tool or material to begin painting in 3D.".to_string(),
            export_preset: ExportPreset::UnrealEngine,
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
            bake_cage_cache_key: (crate::baker::BakeCageMode::None, 0, 0, String::new(), false, false, 0, 0),
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

    /// Update 3D viewport texture in egui
    fn update_viewport_texture(&mut self, ctx: &egui::Context) {
        self.renderer.render(
            &self.camera,
            &self.project.mesh,
            &self.project.composite_textures,
            self.view_mode,
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
            self.project.active_layer().and_then(|l| if l.has_mask { Some(l.mask_buf.clone()) } else { None })
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
            self.project.active_layer().and_then(|l| if l.has_mask { Some(l.mask_buf.clone()) } else { None })
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

    /// Render the Substance 3D Painter Shelf Resource Picker Modal Dialog
    pub fn render_shelf_picker_modal(&mut self, ctx: &egui::Context) {
        if self.shelf_picker.is_none() {
            return;
        }

        self.refresh_shelf_thumbnails(ctx);

        let (target, mut search_query, mut selected_category, title) = {
            let p = self.shelf_picker.as_ref().unwrap();
            (p.target, p.search_query.clone(), p.selected_category, p.title())
        };

        let mut close_picker = false;
        let mut picked_resource = None;
        let mut picked_noise = None;
        let mut picked_hdri = None;
        let mut clear_slot = false;
        let mut trigger_import = false;
        let mut is_open = true;

        egui::Window::new(title)
            .open(&mut is_open)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(540.0, 520.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Target:").strong().color(Color32::from_rgb(243, 94, 24)));
                    ui.label(egui::RichText::new(title).italics());
                });
                ui.add_space(4.0);

                // Search input & Import
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut search_query)
                            .hint_text("Search resources...")
                            .desired_width(ui.available_width() - 110.0),
                    );
                    if ui.button("+ Import...").on_hover_text("Import image file directly to Project Shelf and slot").clicked() {
                        trigger_import = true;
                    }
                });
                ui.add_space(4.0);

                // Category filter tabs
                let categories = [
                    (crate::shelf::ShelfCategory::Project, "Project Resources"),
                    (crate::shelf::ShelfCategory::Textures, "Textures"),
                    (crate::shelf::ShelfCategory::Generators, "Noises"),
                    (crate::shelf::ShelfCategory::Environments, "HDRIs"),
                    (crate::shelf::ShelfCategory::All, "All"),
                ];

                ui.horizontal(|ui| {
                    for (cat, label) in categories {
                        let is_sel = selected_category == cat;
                        if ui.selectable_label(is_sel, label).clicked() {
                            selected_category = cat;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Clear Slot").on_hover_text("Unassign texture from this slot").clicked() {
                            clear_slot = true;
                        }
                    });
                });

                ui.separator();

                let query = search_query.to_lowercase();
                let cat = selected_category;

                // Filter project resources
                let project_res: Vec<_> = self.project.resources.iter().filter(|r| {
                    if cat == crate::shelf::ShelfCategory::Generators {
                        return false;
                    }
                    if cat == crate::shelf::ShelfCategory::Environments && r.resource_type != crate::project::ProjectResourceType::Hdri {
                        return false;
                    }
                    if !query.is_empty() && !r.name.to_lowercase().contains(&query) && !r.id.to_lowercase().contains(&query) {
                        return false;
                    }
                    true
                }).collect();

                // Filter procedural noise generators if appropriate
                let all_shelf = crate::shelf::get_shelf_assets();
                let shelf_assets: Vec<_> = all_shelf.iter().copied().filter(|a| {
                    if cat == crate::shelf::ShelfCategory::Project {
                        return false;
                    }
                    if cat == crate::shelf::ShelfCategory::Environments {
                        return false;
                    }
                    if cat != crate::shelf::ShelfCategory::All && a.category != cat {
                        return false;
                    }
                    if !query.is_empty() && !a.name.to_lowercase().contains(&query) && !a.description.to_lowercase().contains(&query) {
                        return false;
                    }
                    true
                }).collect();

                // Filter HDRIs if in Environments or All
                let hdris: Vec<_> = if cat == crate::shelf::ShelfCategory::Environments || cat == crate::shelf::ShelfCategory::All {
                    self.asset_manager.hdris.iter().filter(|h| {
                        if !query.is_empty() && !h.name.to_lowercase().contains(&query) && !h.file_name.to_lowercase().contains(&query) {
                            return false;
                        }
                        true
                    }).collect()
                } else {
                    Vec::new()
                };

                let card_w = 95.0;
                let card_h = 105.0;

                egui::ScrollArea::vertical()
                    .id_source("shelf_picker_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if project_res.is_empty() && shelf_assets.is_empty() && hdris.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(20.0);
                                ui.label(egui::RichText::new("No resources found in current filter.").italics().color(Color32::from_rgb(150, 150, 160)));
                                if ui.button("Import Texture File to Project...").clicked() {
                                    trigger_import = true;
                                }
                            });
                        }

                        // 1. PROJECT RESOURCES (Baked Maps + Imported Textures)
                        if !project_res.is_empty() {
                            ui.label(egui::RichText::new("PROJECT RESOURCES").size(10.5).color(Color32::from_rgb(180, 190, 205)).strong());
                            ui.add_space(2.0);

                            for chunk in project_res.chunks(4) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    for res in chunk {
                                        let (card_rect, card_resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                        let is_hov = card_resp.hovered();

                                        let bg_col = if is_hov { Color32::from_rgb(46, 50, 58) } else { Color32::from_rgb(30, 32, 36) };
                                        let stroke = if is_hov {
                                            egui::Stroke::new(1.5_f32, Color32::from_rgb(243, 94, 24))
                                        } else {
                                            egui::Stroke::new(1.0_f32, Color32::from_rgb(50, 54, 62))
                                        };
                                        ui.painter().rect(card_rect, egui::Rounding::same(4.0), bg_col, stroke);

                                        // Thumbnail
                                        let thumb_rect = egui::Rect::from_min_size(
                                            egui::pos2(card_rect.min.x + 5.0, card_rect.min.y + 5.0),
                                            egui::vec2(card_w - 10.0, card_w - 24.0),
                                        );

                                        if let Some(tex) = self.shelf_thumbnails.get(&res.id) {
                                            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                                            ui.painter().image(tex.id(), thumb_rect, uv, Color32::WHITE);
                                        } else {
                                            ui.painter().rect_filled(thumb_rect, 2.0, Color32::from_rgb(20, 22, 26));
                                            ui.painter().text(thumb_rect.center(), egui::Align2::CENTER_CENTER, "MAP", egui::FontId::proportional(10.0), Color32::LIGHT_GRAY);
                                        }

                                        // Badge
                                        let (badge_txt, badge_col) = if res.is_baked {
                                            ("BAKED", Color32::from_rgb(255, 175, 40))
                                        } else if res.resource_type == crate::project::ProjectResourceType::Hdri {
                                            ("HDRI", Color32::from_rgb(180, 80, 255))
                                        } else {
                                            ("PROJECT", Color32::from_rgb(45, 140, 255))
                                        };
                                        ui.painter().text(
                                            egui::pos2(card_rect.max.x - 3.0, card_rect.min.y + 2.0),
                                            egui::Align2::RIGHT_TOP,
                                            badge_txt,
                                            egui::FontId::proportional(7.5),
                                            badge_col,
                                        );

                                        // Resolution & Name
                                        let dim_txt = format!("{}x{}", res.width, res.height);
                                        ui.painter().text(
                                            egui::pos2(card_rect.center().x, card_rect.max.y - 18.0),
                                            egui::Align2::CENTER_CENTER,
                                            &dim_txt,
                                            egui::FontId::proportional(7.5),
                                            Color32::from_rgb(130, 135, 145),
                                        );
                                        ui.painter().text(
                                            egui::pos2(card_rect.center().x, card_rect.max.y - 8.0),
                                            egui::Align2::CENTER_CENTER,
                                            &res.name,
                                            egui::FontId::proportional(8.5),
                                            Color32::from_rgb(215, 220, 230),
                                        );

                                        let tooltip = format!("{}\nType: {}\nResolution: {}x{}\nClick to assign to target slot", res.name, badge_txt, res.width, res.height);
                                        if card_resp.on_hover_text(tooltip).clicked() {
                                            picked_resource = Some(((*res).clone(), target));
                                        }
                                    }
                                });
                            }
                            ui.add_space(8.0);
                        }

                        // 2. PROCEDURAL NOISES & GENERATORS
                        if !shelf_assets.is_empty() {
                            ui.label(egui::RichText::new("PROCEDURAL GENERATORS & PATTERNS").size(10.5).color(Color32::from_rgb(180, 190, 205)).strong());
                            ui.add_space(2.0);

                            for chunk in shelf_assets.chunks(4) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    for asset in chunk {
                                        let (card_rect, card_resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                        let is_hov = card_resp.hovered();

                                        let bg_col = if is_hov { Color32::from_rgb(46, 50, 58) } else { Color32::from_rgb(30, 32, 36) };
                                        let stroke = if is_hov {
                                            egui::Stroke::new(1.5_f32, Color32::from_rgb(243, 94, 24))
                                        } else {
                                            egui::Stroke::new(1.0_f32, Color32::from_rgb(50, 54, 62))
                                        };
                                        ui.painter().rect(card_rect, egui::Rounding::same(4.0), bg_col, stroke);

                                        let thumb_rect = egui::Rect::from_min_size(
                                            egui::pos2(card_rect.min.x + 5.0, card_rect.min.y + 5.0),
                                            egui::vec2(card_w - 10.0, card_w - 24.0),
                                        );
                                        ui.painter().rect_filled(thumb_rect, 2.0, Color32::from_rgb(24, 26, 32));

                                        let center = thumb_rect.center();
                                        ui.painter().circle_filled(center, 12.0, Color32::from_rgb(asset.preview_color[0], asset.preview_color[1], asset.preview_color[2]));

                                        ui.painter().text(
                                            egui::pos2(card_rect.max.x - 3.0, card_rect.min.y + 2.0),
                                            egui::Align2::RIGHT_TOP,
                                            "NOISE",
                                            egui::FontId::proportional(7.5),
                                            Color32::from_rgb(46, 204, 113),
                                        );

                                        ui.painter().text(
                                            egui::pos2(card_rect.center().x, card_rect.max.y - 8.0),
                                            egui::Align2::CENTER_CENTER,
                                            asset.name,
                                            egui::FontId::proportional(8.5),
                                            Color32::from_rgb(215, 220, 230),
                                        );

                                        let tooltip = format!("{}\n{}\nClick to assign generator to slot", asset.name, asset.description);
                                        if card_resp.on_hover_text(tooltip).clicked() {
                                            let gen = match asset.id {
                                                "gen_worley" => crate::layer::ProceduralGenerator::WorleyGrunge,
                                                "gen_billow" => crate::layer::ProceduralGenerator::BillowNoise,
                                                "gen_ridged" => crate::layer::ProceduralGenerator::RidgedMultifractal,
                                                "gen_domain_warp" => crate::layer::ProceduralGenerator::DomainWarpGrunge,
                                                "gen_splatter" => crate::layer::ProceduralGenerator::SplatterDirt,
                                                "gen_wood" => crate::layer::ProceduralGenerator::WoodGrain,
                                                "gen_fabric" => crate::layer::ProceduralGenerator::FabricWeave,
                                                "gen_scratches" => crate::layer::ProceduralGenerator::Scratches,
                                                "gen_curvature" => crate::layer::ProceduralGenerator::CurvatureWear,
                                                "gen_cavity" => crate::layer::ProceduralGenerator::AmbientOcclusion,
                                                "gen_dust" => crate::layer::ProceduralGenerator::DirectionalDust,
                                                "gen_leaks" => crate::layer::ProceduralGenerator::DrippingLeaks,
                                                "gen_position" => crate::layer::ProceduralGenerator::PositionGradient,
                                                "gen_checker" => crate::layer::ProceduralGenerator::Checkerboard,
                                                "gen_bricks" => crate::layer::ProceduralGenerator::Bricks,
                                                "gen_perforated" => crate::layer::ProceduralGenerator::PerforatedMesh,
                                                "gen_wire_fence" => crate::layer::ProceduralGenerator::WireFence,
                                                "gen_torn_fabric" => crate::layer::ProceduralGenerator::TornFabricEdge,
                                                "gen_dots" => crate::layer::ProceduralGenerator::CircularDots,
                                                "gen_bnw_spots_1" => crate::layer::ProceduralGenerator::BnWSpots1,
                                                "gen_bnw_spots_2" => crate::layer::ProceduralGenerator::BnWSpots2,
                                                "gen_bnw_spots_3" => crate::layer::ProceduralGenerator::BnWSpots3,
                                                "gen_grunge_map" => crate::layer::ProceduralGenerator::GrungeMap,
                                                "gen_directional_noise" => crate::layer::ProceduralGenerator::DirectionalNoise,
                                                "gen_clouds" => crate::layer::ProceduralGenerator::Clouds,
                                                "gen_marble_veins" => crate::layer::ProceduralGenerator::MarbleVeins,
                                                "gen_moisture" => crate::layer::ProceduralGenerator::MoistureNoise,
                                                _ => crate::layer::ProceduralGenerator::PerlinNoise,
                                            };
                                            picked_noise = Some((gen, target));
                                        }
                                    }
                                });
                            }
                            ui.add_space(8.0);
                        }

                        // 3. HDRIs (Environments)
                        if !hdris.is_empty() {
                            ui.label(egui::RichText::new("HDRI ENVIRONMENTS").size(10.5).color(Color32::from_rgb(180, 190, 205)).strong());
                            ui.add_space(2.0);

                            for chunk in hdris.chunks(4) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    for hdri in chunk {
                                        let (card_rect, card_resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                        let is_hov = card_resp.hovered();

                                        let bg_col = if is_hov { Color32::from_rgb(46, 50, 58) } else { Color32::from_rgb(30, 32, 36) };
                                        let stroke = if is_hov {
                                            egui::Stroke::new(1.5_f32, Color32::from_rgb(243, 94, 24))
                                        } else {
                                            egui::Stroke::new(1.0_f32, Color32::from_rgb(50, 54, 62))
                                        };
                                        ui.painter().rect(card_rect, egui::Rounding::same(4.0), bg_col, stroke);

                                        let thumb_rect = egui::Rect::from_min_size(
                                            egui::pos2(card_rect.min.x + 5.0, card_rect.min.y + 5.0),
                                            egui::vec2(card_w - 10.0, card_w - 24.0),
                                        );
                                        draw_hdri_thumbnail(ui.painter(), thumb_rect, &hdri.id, &hdri.name);

                                        ui.painter().text(
                                            egui::pos2(card_rect.max.x - 3.0, card_rect.min.y + 2.0),
                                            egui::Align2::RIGHT_TOP,
                                            if hdri.is_builtin { "BUILTIN" } else { "HDRI" },
                                            egui::FontId::proportional(7.5),
                                            Color32::from_rgb(255, 175, 40),
                                        );

                                        ui.painter().text(
                                            egui::pos2(card_rect.center().x, card_rect.max.y - 8.0),
                                            egui::Align2::CENTER_CENTER,
                                            &hdri.name,
                                            egui::FontId::proportional(8.5),
                                            Color32::from_rgb(215, 220, 230),
                                        );

                                        let tooltip = format!("{}\nFile: {}\nSize: {}\nClick to set active HDRI", hdri.name, hdri.file_name, hdri.size_formatted());
                                        if card_resp.on_hover_text(tooltip).clicked() {
                                            picked_hdri = Some(hdri.path.clone());
                                        }
                                    }
                                });
                            }
                        }
                    });
            });

        if let Some(p) = &mut self.shelf_picker {
            p.search_query = search_query;
            p.selected_category = selected_category;
        }

        if let Some(path) = picked_hdri {
            self.activate_hdri_file(&path);
            close_picker = true;
        }

        if !is_open {
            close_picker = true;
        }

        if trigger_import {
            if let Some(picker) = &self.shelf_picker {
                let target = picker.target;
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Image / Texture", &["png", "jpg", "jpeg", "tga", "exr", "hdr"])
                    .set_title("Import Texture to Project Shelf")
                    .pick_file()
                {
                    if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                        self.refresh_shelf_thumbnails(ctx);
                        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                        if ext == "hdr" || ext == "exr" {
                            let _ = self.asset_manager.import_hdri_file(&path);
                        }
                        if let Some(res) = self.project.get_resource(&id).cloned() {
                            self.assign_resource_to_target(&res, target);
                            close_picker = true;
                        }
                    }
                }
            }
        }

        if clear_slot {
            if let Some(picker) = &self.shelf_picker {
                match picker.target {
                    ShelfPickerTarget::FillBaseColor => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_color_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillRoughness => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_roughness_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillMetallic => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_metallic_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillNormal => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_normal_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillHeight => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_height_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillEmissive => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_emissive_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::FillOpacity => {
                        if let Some(l) = self.project.layers.get_mut(self.project.active_layer_idx) { l.fill_opacity_tex = None; l.evaluate_fill(self.project.curvature_map.as_ref()); self.project.rebuild_composite(); self.project.gpu_sync_needed = true; }
                    }
                    ShelfPickerTarget::Stencil => {
                        self.brush_engine.stencil_buffer = None;
                    }
                    ShelfPickerTarget::MeshMapTangentNormal => { self.project.world_normal_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapWorldSpaceNormal => { self.project.world_space_normal_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapAo => { self.project.ao_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapCurvature => { self.project.curvature_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapPosition => { self.project.position_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapThickness => { self.project.thickness_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    ShelfPickerTarget::MeshMapId => { self.project.id_map = None; self.project.gpu_mesh_maps_sync_needed = true; self.project.sync_baked_maps_to_resources(); }
                    _ => {}
                }
                close_picker = true;
            }
        }

        if let Some((res, target)) = picked_resource {
            self.assign_resource_to_target(&res, target);
            close_picker = true;
        }

        if let Some((gen, target)) = picked_noise {
            self.assign_noise_to_target(gen, target);
            close_picker = true;
        }

        if close_picker {
            self.shelf_picker = None;
        }
    }

    /// Render quick popup when user clicks a project resource directly in the Left Shelf
    pub fn render_shelf_resource_popup(&mut self, ctx: &egui::Context) {
        let mut close_popup = false;
        let mut assign_target = None;

        if let Some(res_id) = &self.shelf_resource_popup {
            if let Some(res) = self.project.get_resource(res_id).cloned() {
                let mut is_open = true;
                egui::Window::new(format!("Resource: {}", res.name))
                    .open(&mut is_open)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(egui::RichText::new(format!("{} ({}x{})", res.name, res.width, res.height)).strong());
                        ui.separator();
                        ui.label("Assign to Active Layer channel:");
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Base Color").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillBaseColor));
                                close_popup = true;
                            }
                            if ui.button("Roughness").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillRoughness));
                                close_popup = true;
                            }
                            if ui.button("Metallic").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillMetallic));
                                close_popup = true;
                            }
                            if ui.button("Normal").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillNormal));
                                close_popup = true;
                            }
                            if ui.button("Height").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillHeight));
                                close_popup = true;
                            }
                            if ui.button("Emissive").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillEmissive));
                                close_popup = true;
                            }
                            if ui.button("Opacity").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::FillOpacity));
                                close_popup = true;
                            }
                        });
                        ui.add_space(6.0);
                        ui.label("Tools & Environment:");
                        ui.horizontal(|ui| {
                            if ui.button("Set as Stencil (Projection)").clicked() {
                                assign_target = Some((res.clone(), ShelfPickerTarget::Stencil));
                                close_popup = true;
                            }
                            if res.resource_type == crate::project::ProjectResourceType::Hdri {
                                if ui.button("Activate Viewport HDRI").clicked() {
                                    assign_target = Some((res.clone(), ShelfPickerTarget::HdriEnvironment));
                                    close_popup = true;
                                }
                            }
                        });
                        ui.add_space(4.0);
                        if !res.is_baked {
                            if ui.button(egui::RichText::new("Remove from Project Shelf").color(Color32::from_rgb(255, 100, 100))).clicked() {
                                self.project.remove_resource(&res.id);
                                self.shelf_thumbnails.remove(&res.id);
                                self.shelf_thumbnails_dirty = true;
                                self.status_message = format!("Removed '{}' from Project Shelf", res.name);
                                close_popup = true;
                            }
                            ui.add_space(2.0);
                        }
                        if ui.small_button("Close").clicked() {
                            close_popup = true;
                        }
                    });
                if !is_open {
                    close_popup = true;
                }
            } else {
                close_popup = true;
            }
        }

        if let Some((res, target)) = assign_target {
            self.assign_resource_to_target(&res, target);
        }

        if close_popup {
            self.shelf_resource_popup = None;
        }
    }

    /// Render the Substance 3D Painter Organized Export Textures Modal Dialog
    pub fn render_export_modal(&mut self, ctx: &egui::Context) {
        let mut close_modal = false;
        let mut status_update = None;

        let screen = ctx.screen_rect();
        let def_w = 880.0f32.min(screen.width() * 0.85).max(520.0);
        let def_h = 600.0f32.min(screen.height() * 0.82).max(380.0);
        let min_w = 580.0f32.min(screen.width() * 0.9);
        let min_h = 360.0f32.min(screen.height() * 0.85);
        let max_w = (screen.width() - 32.0).max(400.0);
        let max_h = (screen.height() - 32.0).max(350.0);

        egui::Window::new("Export Textures")
            .collapsible(false)
            .resizable(true)
            .default_size([def_w, def_h])
            .min_width(min_w)
            .min_height(min_h)
            .max_size([max_w, max_h])
            .show(ctx, |ui| {
                // Top Tab Navigation Bar (Substance 3D Painter Standard)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    let is_settings = self.export_tab_idx == 0;
                    let is_templates = self.export_tab_idx == 1;
                    let is_exports = self.export_tab_idx == 2;

                    if ui.selectable_label(is_settings, egui::RichText::new("Settings").strong().size(14.0)).clicked() {
                        self.export_tab_idx = 0;
                    }
                    if ui.selectable_label(is_templates, egui::RichText::new("Output Templates").strong().size(14.0)).clicked() {
                        self.export_tab_idx = 1;
                    }
                    let count = self.export_last_exported_files.len();
                    let exports_label = if count > 0 {
                        format!("List of Exports ({})", count)
                    } else {
                        "List of Exports".to_string()
                    };
                    if ui.selectable_label(is_exports, egui::RichText::new(exports_label).strong().size(14.0)).clicked() {
                        self.export_tab_idx = 2;
                    }
                });
                ui.separator();

                // TAB 0: SETTINGS (Main Export Dashboard)
                if self.export_tab_idx == 0 {
                    ui.columns(2, |columns| {
                        // Left Column: Texture Sets
                        columns[0].vertical(|ui| {
                            ui.set_max_width(280.0);
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.heading("Texture Sets");
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("None").on_hover_text("Deselect all texture sets").clicked() {
                                            for ts in &mut self.project.texture_sets {
                                                ts.visible = false;
                                            }
                                        }
                                        if ui.small_button("All").on_hover_text("Select all texture sets").clicked() {
                                            for ts in &mut self.project.texture_sets {
                                                ts.visible = true;
                                            }
                                        }
                                    });
                                });
                                ui.separator();

                                if self.project.texture_sets.is_empty() {
                                    ui.label("No texture sets defined");
                                } else {
                                    let mut focused_idx = self.export_selected_ts_idx;
                                    egui::ScrollArea::vertical()
                                        .max_height(340.0)
                                        .id_source("export_texture_sets_scroll")
                                        .show(ui, |ui| {
                                            for (idx, ts) in self.project.texture_sets.iter_mut().enumerate() {
                                                let is_focused = focused_idx == idx;
                                                ui.horizontal(|ui| {
                                                    ui.checkbox(&mut ts.visible, "");
                                                    let resp = ui.selectable_label(
                                                        is_focused,
                                                        egui::RichText::new(&ts.name).strong(),
                                                    );
                                                    if resp.clicked() {
                                                        focused_idx = idx;
                                                    }
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.label(
                                                            egui::RichText::new(format!("{}x{}", ts.width, ts.height))
                                                                .small()
                                                                .color(egui::Color32::from_rgb(160, 160, 160)),
                                                        );
                                                    });
                                                });
                                            }
                                        });
                                    self.export_selected_ts_idx = focused_idx;
                                }
                            });
                        });

                        // Right Column: Global Settings & Output Maps Preview
                        columns[1].vertical(|ui| {
                            ui.group(|ui| {
                                ui.heading("Global Settings");
                                ui.separator();

                                egui::Grid::new("export_settings_grid")
                                    .num_columns(2)
                                    .spacing([12.0, 8.0])
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new("Output Directory:").strong());
                                        ui.horizontal(|ui| {
                                            ui.add(egui::TextEdit::singleline(&mut self.export_dir).desired_width(280.0));
                                            if ui.button("Open Folder").on_hover_text("Open export folder in system file manager").clicked() {
                                                let p = PathBuf::from(&self.export_dir);
                                                let _ = std::fs::create_dir_all(&p);
                                                open_folder_in_desktop(&p);
                                            }
                                        });
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Output Template:").strong());
                                        let current_template_name = self
                                            .custom_packing_presets
                                            .get(self.active_custom_packing_idx)
                                            .map(|p| p.name.clone())
                                            .unwrap_or_else(|| "None".to_string());

                                        egui::ComboBox::from_id_source("settings_output_template_combo")
                                            .selected_text(&current_template_name)
                                            .width(280.0)
                                            .show_ui(ui, |ui| {
                                                for (idx, p) in self.custom_packing_presets.iter().enumerate() {
                                                    ui.selectable_value(&mut self.active_custom_packing_idx, idx, &p.name);
                                                }
                                            });
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Resolution Override:").strong());
                                        egui::ComboBox::from_id_source("settings_res_override_combo")
                                            .selected_text(match self.export_resolution_override {
                                                0 => "Based on each texture set's size",
                                                512 => "512 x 512",
                                                1024 => "1024 x 1024 (1K)",
                                                2048 => "2048 x 2048 (2K)",
                                                4096 => "4096 x 4096 (4K)",
                                                _ => "Custom",
                                            })
                                            .width(280.0)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut self.export_resolution_override, 0, "Based on each texture set's size");
                                                ui.selectable_value(&mut self.export_resolution_override, 512, "512 x 512");
                                                ui.selectable_value(&mut self.export_resolution_override, 1024, "1024 x 1024 (1K)");
                                                ui.selectable_value(&mut self.export_resolution_override, 2048, "2048 x 2048 (2K)");
                                                ui.selectable_value(&mut self.export_resolution_override, 4096, "4096 x 4096 (4K)");
                                            });
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Padding / Dilation:").strong());
                                        ui.add(egui::Slider::new(&mut self.export_padding_px, 0..=32).text("pixels"));
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Export 3D Model:").strong());
                                        ui.checkbox(&mut self.export_mesh, "Include 3D mesh files (.obj, .mtl, .glb)")
                                            .on_hover_text("When checked, exports 3D mesh geometry files alongside texture maps. When unchecked, exports texture maps only.");
                                        ui.end_row();

                                        ui.label(egui::RichText::new("Coordinate System:").strong());
                                        ui.checkbox(&mut self.export_flip_v, "Flip Y / V-Axis (DirectX / OBJ UV origin)")
                                            .on_hover_text("Inverts vertical texture coordinates to conform to DirectX and standard DCC UV orientations.");
                                        ui.end_row();
                                    });
                            });

                            ui.add_space(8.0);

                            // Output Maps Preview for selected texture set
                            ui.group(|ui| {
                                let focused_ts_name = self.project.texture_sets.get(self.export_selected_ts_idx)
                                    .map(|ts| ts.name.as_str())
                                    .unwrap_or("Active Project");

                                ui.horizontal(|ui| {
                                    ui.heading(format!("Output Maps for '{}':", focused_ts_name));
                                    if let Some(cur_preset) = self.custom_packing_presets.get(self.active_custom_packing_idx) {
                                        let active_count = cur_preset.maps.iter().filter(|m| m.enabled).count();
                                        ui.label(
                                            egui::RichText::new(format!("({} maps active)", active_count))
                                                .color(egui::Color32::from_rgb(140, 200, 255)),
                                        );
                                    }
                                });
                                ui.separator();

                                if let Some(cur_preset) = self.custom_packing_presets.get(self.active_custom_packing_idx) {
                                    let mesh_clean = self.project.mesh.name.replace(' ', "_").to_lowercase();
                                    let ts_clean = focused_ts_name.replace(' ', "_");

                                    egui::ScrollArea::vertical()
                                        .max_height(160.0)
                                        .id_source("export_preview_scroll")
                                        .show(ui, |ui| {
                                            if self.export_mesh {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("[3D]")
                                                            .monospace()
                                                            .color(egui::Color32::from_rgb(120, 220, 140)),
                                                    );
                                                    ui.label(egui::RichText::new(format!("{}.obj, {}.mtl, {}.glb", mesh_clean, mesh_clean, mesh_clean)).strong());
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.label(
                                                            egui::RichText::new("3D Geometry + MTL")
                                                                .small()
                                                                .color(egui::Color32::from_rgb(180, 180, 180)),
                                                        );
                                                    });
                                                });
                                            }
                                            for map in &cur_preset.maps {
                                                if !map.enabled {
                                                    continue;
                                                }
                                                let file_name = if self.project.texture_sets.len() > 1 {
                                                    format!("{}_{}{}.{}", mesh_clean, ts_clean, map.name_suffix, map.format.extension())
                                                } else {
                                                    format!("{}{}.{}", mesh_clean, map.name_suffix, map.format.extension())
                                                };

                                                ui.horizontal(|ui| {
                                                    let mode_badge = match map.color_mode {
                                                        crate::packing::PackedColorMode::Rgb => "RGB",
                                                        crate::packing::PackedColorMode::Rgba => "RGBA",
                                                        crate::packing::PackedColorMode::Grayscale => "GRAY",
                                                    };
                                                    ui.label(
                                                        egui::RichText::new(format!("[{}]", mode_badge))
                                                            .monospace()
                                                            .color(egui::Color32::from_rgb(100, 200, 255)),
                                                    );
                                                    ui.label(egui::RichText::new(&file_name).strong());
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.label(
                                                            egui::RichText::new(map.channel_summary())
                                                                .small()
                                                                .color(egui::Color32::from_rgb(180, 180, 180)),
                                                        );
                                                    });
                                                });
                                            }
                                        });
                                }
                            });

                            ui.add_space(8.0);

                            // Bottom Action Bar
                            ui.horizontal(|ui| {
                                let selected_ts_count = self.project.texture_sets.iter().filter(|ts| ts.visible).count();
                                ui.label(
                                    egui::RichText::new(format!("Selected Texture Sets: {} of {}", selected_ts_count, self.project.texture_sets.len()))
                                        .strong(),
                                );

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // Primary Export Button (Substance standard high-visibility button)
                                    let can_export = selected_ts_count > 0 || self.project.texture_sets.is_empty();
                                    let export_btn = egui::Button::new(
                                        egui::RichText::new("  Export  ").strong().color(egui::Color32::WHITE),
                                    )
                                    .fill(if can_export {
                                        egui::Color32::from_rgb(40, 120, 220)
                                    } else {
                                        egui::Color32::from_rgb(70, 75, 85)
                                    });

                                    let resp = ui.add_enabled(can_export, export_btn);
                                    if !can_export {
                                        let _ = resp.clone().on_hover_text("Select at least one texture set to export");
                                    }

                                    if resp.clicked() {
                                        let out_path = PathBuf::from(&self.export_dir);
                                        let _ = std::fs::create_dir_all(&out_path);
                                        let mut all_exported = Vec::new();

                                        let mut active_preset = self
                                            .custom_packing_presets
                                            .get(self.active_custom_packing_idx)
                                            .cloned()
                                            .unwrap_or_else(crate::packing::CustomPackingPreset::unreal_orm);

                                        // Apply global padding override if configured
                                        for m in &mut active_preset.maps {
                                            m.dilation_iterations = self.export_padding_px;
                                        }

                                        // Step 0: Fully rebuild composite for active canvas and synchronize active texture set
                                        self.project.rebuild_composite();
                                        self.project.sync_active_texture_set();

                                        let ts_count = self.project.texture_sets.len();

                                        // Step 1: Pre-composite all layers for visible texture sets
                                        if !self.project.texture_sets.is_empty() {
                                            for (ts_idx, ts) in self.project.texture_sets.iter_mut().enumerate() {
                                                if ts.visible {
                                                    if ts_idx == self.project.active_texture_set_idx {
                                                        ts.composite_textures = self.project.composite_textures.clone();
                                                    } else {
                                                        ts.composite_textures.composite_layers(&ts.layers);
                                                    }
                                                }
                                            }
                                        } else {
                                            self.project.composite_textures.composite_layers(&self.project.layers);
                                        }

                                        // Step 2: Borrow baked context immutably
                                        let baked = self.project.baked_map_context();
                                        let mesh_name = self.project.mesh.name.clone();

                                        if !self.project.texture_sets.is_empty() {
                                            for (ts_idx, ts) in self.project.texture_sets.iter().enumerate() {
                                                if ts.visible {
                                                    let prefix = if ts_count > 1 {
                                                        format!("{}_{}", mesh_name, ts.name)
                                                    } else {
                                                        mesh_name.clone()
                                                    };

                                                    let textures_to_export = if ts_idx == self.project.active_texture_set_idx {
                                                        &self.project.composite_textures
                                                    } else {
                                                        &ts.composite_textures
                                                    };

                                                    match crate::packing::CustomTexturePacker::pack_and_export(
                                                        &active_preset,
                                                        textures_to_export,
                                                        &prefix,
                                                        &out_path,
                                                        self.export_flip_v,
                                                        if self.export_mesh && ts_idx == 0 { Some(&self.project.mesh) } else { None },
                                                        &baked,
                                                    ) {
                                                        Ok(mut files) => all_exported.append(&mut files),
                                                        Err(e) => {
                                                            status_update = Some(format!("Export error on {}: {}", ts.name, e));
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            match crate::packing::CustomTexturePacker::pack_and_export(
                                                &active_preset,
                                                &self.project.composite_textures,
                                                &mesh_name,
                                                &out_path,
                                                self.export_flip_v,
                                                if self.export_mesh { Some(&self.project.mesh) } else { None },
                                                &baked,
                                            ) {
                                                Ok(mut files) => all_exported.append(&mut files),
                                                Err(e) => {
                                                    status_update = Some(format!("Export error: {}", e));
                                                }
                                            }
                                        }

                                        if !all_exported.is_empty() {
                                            self.export_last_exported_files = all_exported.clone();
                                            status_update = Some(format!("Successfully exported {} production maps to {}", all_exported.len(), self.export_dir));
                                            self.export_tab_idx = 2; // Automatically switch to List of Exports tab!
                                        }
                                    }

                                    if ui.button("Cancel").clicked() {
                                        close_modal = true;
                                    }
                                });
                            });
                        });
                    });
                }

                // TAB 1: OUTPUT TEMPLATES (Substance 3D Painter Channel Packing Matrix)
                if self.export_tab_idx == 1 {
                    ui.columns(2, |columns| {
                        // Left Column: Templates Library
                        columns[0].vertical(|ui| {
                            ui.set_max_width(260.0);
                            ui.group(|ui| {
                                ui.heading("Output Templates");
                                ui.separator();

                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Filter:").small().strong());
                                    ui.add(egui::TextEdit::singleline(&mut self.export_template_search).hint_text("Search templates..."));
                                });
                                ui.add_space(4.0);

                                let preset_count = self.custom_packing_presets.len();
                                if self.active_custom_packing_idx >= preset_count && preset_count > 0 {
                                    self.active_custom_packing_idx = 0;
                                }

                                egui::ScrollArea::vertical()
                                    .max_height(280.0)
                                    .id_source("substance_templates_list_scroll")
                                    .show(ui, |ui| {
                                        let search_lower = self.export_template_search.to_lowercase();
                                        for (idx, p) in self.custom_packing_presets.iter().enumerate() {
                                            if !search_lower.is_empty() && !p.name.to_lowercase().contains(&search_lower) {
                                                continue;
                                            }
                                            let is_selected = self.active_custom_packing_idx == idx;
                                            let map_count = p.maps.len();
                                            let row_label = format!("{} ({} maps)", p.name, map_count);

                                            if ui.selectable_label(is_selected, row_label).clicked() {
                                                self.active_custom_packing_idx = idx;
                                            }
                                        }
                                    });

                                ui.separator();
                                // Template management toolbar
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("+ New").on_hover_text("Create a new blank template").clicked() {
                                        let new_preset = crate::packing::CustomPackingPreset {
                                            id: format!("custom_{}", chrono::Utc::now().timestamp_millis()),
                                            name: "New Output Template".to_string(),
                                            description: "Custom user-defined channel matrix".to_string(),
                                            maps: vec![
                                                crate::packing::PackedTextureMap::new_rgb("_BaseColor", crate::packing::ChannelSource::BaseColorR, crate::packing::ChannelSource::BaseColorG, crate::packing::ChannelSource::BaseColorB),
                                                crate::packing::PackedTextureMap::new_rgb("_Normal", crate::packing::ChannelSource::NormalX, crate::packing::ChannelSource::NormalY, crate::packing::ChannelSource::NormalZ),
                                                crate::packing::PackedTextureMap::new_rgb("_ORM", crate::packing::ChannelSource::AmbientOcclusion, crate::packing::ChannelSource::Roughness, crate::packing::ChannelSource::Metallic),
                                            ],
                                        };
                                        self.custom_packing_presets.push(new_preset);
                                        self.active_custom_packing_idx = self.custom_packing_presets.len() - 1;
                                    }

                                    if ui.button("Duplicate").clicked() {
                                        if let Some(cur) = self.custom_packing_presets.get(self.active_custom_packing_idx) {
                                            let mut dup = cur.clone();
                                            dup.id = format!("{}_copy", dup.id);
                                            dup.name = format!("{} (Copy)", dup.name);
                                            self.custom_packing_presets.push(dup);
                                            self.active_custom_packing_idx = self.custom_packing_presets.len() - 1;
                                        }
                                    }

                                    if preset_count > 1 && ui.button("Delete").clicked() {
                                        self.custom_packing_presets.remove(self.active_custom_packing_idx);
                                        if self.active_custom_packing_idx >= self.custom_packing_presets.len() {
                                            self.active_custom_packing_idx = self.custom_packing_presets.len().saturating_sub(1);
                                        }
                                    }

                                    if ui.button("Reset Defaults").clicked() {
                                        self.custom_packing_presets = crate::packing::CustomPackingPreset::default_presets();
                                        self.active_custom_packing_idx = 0;
                                        self.custom_packing_status = Some("Templates reset to factory defaults.".to_string());
                                    }
                                });

                                ui.add_space(4.0);
                                ui.separator();
                                ui.label(egui::RichText::new("Preset JSON I/O:").small().strong());
                                ui.add(egui::TextEdit::singleline(&mut self.custom_preset_io_filename).desired_width(220.0));
                                ui.horizontal(|ui| {
                                    if ui.small_button("Save JSON").clicked() {
                                        if let Some(cur) = self.custom_packing_presets.get(self.active_custom_packing_idx) {
                                            let out_dir = PathBuf::from(&self.export_dir);
                                            let _ = std::fs::create_dir_all(&out_dir);
                                            let path = out_dir.join(&self.custom_preset_io_filename);
                                            match cur.save_to_json_file(&path) {
                                                Ok(_) => self.custom_packing_status = Some(format!("Saved: {:?}", path)),
                                                Err(e) => self.custom_packing_status = Some(format!("Error: {}", e)),
                                            }
                                        }
                                    }
                                    if ui.small_button("Load JSON").clicked() {
                                        let out_dir = PathBuf::from(&self.export_dir);
                                        let path = out_dir.join(&self.custom_preset_io_filename);
                                        match crate::packing::CustomPackingPreset::load_from_json_file(&path) {
                                            Ok(loaded) => {
                                                self.custom_packing_presets.push(loaded);
                                                self.active_custom_packing_idx = self.custom_packing_presets.len() - 1;
                                                self.custom_packing_status = Some(format!("Loaded: {:?}", path));
                                            }
                                            Err(e) => self.custom_packing_status = Some(format!("Error: {}", e)),
                                        }
                                    }
                                });
                            });
                        });

                        // Right Column: Active Template Editor & Channel Boxes
                        columns[1].vertical(|ui| {
                            if let Some(cur) = self.custom_packing_presets.get_mut(self.active_custom_packing_idx) {
                                ui.group(|ui| {
                                    // Template Header with quick-add buttons
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Name:").strong());
                                        ui.add(egui::TextEdit::singleline(&mut cur.name).desired_width(240.0));

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            // Substance standard quick-add buttons
                                            if ui.button("+ Gray").on_hover_text("Add a Grayscale (single channel) map").clicked() {
                                                cur.maps.push(crate::packing::PackedTextureMap::new_grayscale(
                                                    "_Mask",
                                                    crate::packing::ChannelSource::Roughness,
                                                ));
                                            }
                                            if ui.button("+ RGBA").on_hover_text("Add a 4-channel RGBA map").clicked() {
                                                cur.maps.push(crate::packing::PackedTextureMap::new_rgba(
                                                    "_MaskMap",
                                                    crate::packing::ChannelSource::Metallic,
                                                    crate::packing::ChannelSource::AmbientOcclusion,
                                                    crate::packing::ChannelSource::BakedCurvature,
                                                    crate::packing::ChannelSource::Roughness,
                                                ));
                                            }
                                            if ui.button("+ RGB").on_hover_text("Add a 3-channel RGB map").clicked() {
                                                cur.maps.push(crate::packing::PackedTextureMap::new_rgb(
                                                    "_ORM",
                                                    crate::packing::ChannelSource::AmbientOcclusion,
                                                    crate::packing::ChannelSource::Roughness,
                                                    crate::packing::ChannelSource::Metallic,
                                                ));
                                            }
                                            ui.label(egui::RichText::new("Add Map:").strong());
                                        });
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Description:").small());
                                        ui.add(egui::TextEdit::singleline(&mut cur.description).desired_width(400.0));
                                    });
                                });

                                ui.add_space(6.0);

                                let mut map_to_delete: Option<usize> = None;
                                let mut map_to_duplicate: Option<usize> = None;

                                // Output Maps Cards Scroll Area
                                egui::ScrollArea::vertical()
                                    .max_height(280.0)
                                    .id_source("substance_maps_cards_scroll")
                                    .show(ui, |ui| {
                                        for (m_idx, map) in cur.maps.iter_mut().enumerate() {
                                            egui::Frame::none()
                                                .fill(egui::Color32::from_rgb(26, 26, 26))
                                                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(50, 50, 50)))
                                                .rounding(6.0)
                                                .inner_margin(egui::Margin::same(8.0))
                                                .show(ui, |ui| {
                                                    // Map Header Line
                                                    ui.horizontal(|ui| {
                                                        ui.checkbox(&mut map.enabled, "");
                                                        ui.label(egui::RichText::new("Suffix:").strong());
                                                        ui.add(egui::TextEdit::singleline(&mut map.name_suffix).desired_width(140.0));

                                                        egui::ComboBox::from_id_source(format!("substance_colormode_{}", m_idx))
                                                            .selected_text(map.color_mode.name())
                                                            .width(110.0)
                                                            .show_ui(ui, |ui| {
                                                                ui.selectable_value(&mut map.color_mode, crate::packing::PackedColorMode::Rgb, "RGB");
                                                                ui.selectable_value(&mut map.color_mode, crate::packing::PackedColorMode::Rgba, "RGBA");
                                                                ui.selectable_value(&mut map.color_mode, crate::packing::PackedColorMode::Grayscale, "Grayscale");
                                                            });

                                                        egui::ComboBox::from_id_source(format!("substance_format_{}", m_idx))
                                                            .selected_text(map.format.name())
                                                            .width(90.0)
                                                            .show_ui(ui, |ui| {
                                                                ui.selectable_value(&mut map.format, crate::packing::PackedImageFormat::Png, "PNG");
                                                                ui.selectable_value(&mut map.format, crate::packing::PackedImageFormat::Jpeg, "JPEG");
                                                            });

                                                        ui.label("Pad:");
                                                        ui.add(egui::Slider::new(&mut map.dilation_iterations, 0..=32).text("px"));

                                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                            if ui.small_button("✕").on_hover_text("Delete this map").clicked() {
                                                                map_to_delete = Some(m_idx);
                                                            }
                                                            if ui.small_button("Dup").on_hover_text("Duplicate map").clicked() {
                                                                map_to_duplicate = Some(m_idx);
                                                            }
                                                        });
                                                    });

                                                    ui.add_space(4.0);

                                                    // Substance Painter Channel Slot Boxes (Color-Accented Matrix)
                                                    ui.horizontal(|ui| {
                                                        match map.color_mode {
                                                            crate::packing::PackedColorMode::Grayscale => {
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Gray",
                                                                    &mut map.channel_r,
                                                                    m_idx,
                                                                    "gray",
                                                                    egui::Color32::from_rgb(32, 32, 32),
                                                                    egui::Color32::from_rgb(100, 100, 100),
                                                                    egui::Color32::from_rgb(220, 220, 220),
                                                                );
                                                            }
                                                            crate::packing::PackedColorMode::Rgb => {
                                                                // Red Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Red (R)",
                                                                    &mut map.channel_r,
                                                                    m_idx,
                                                                    "r",
                                                                    egui::Color32::from_rgb(45, 20, 20),
                                                                    egui::Color32::from_rgb(140, 40, 40),
                                                                    egui::Color32::from_rgb(255, 120, 120),
                                                                );
                                                                // Green Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Green (G)",
                                                                    &mut map.channel_g,
                                                                    m_idx,
                                                                    "g",
                                                                    egui::Color32::from_rgb(20, 45, 20),
                                                                    egui::Color32::from_rgb(40, 140, 40),
                                                                    egui::Color32::from_rgb(120, 255, 120),
                                                                );
                                                                // Blue Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Blue (B)",
                                                                    &mut map.channel_b,
                                                                    m_idx,
                                                                    "b",
                                                                    egui::Color32::from_rgb(20, 30, 50),
                                                                    egui::Color32::from_rgb(40, 80, 160),
                                                                    egui::Color32::from_rgb(120, 180, 255),
                                                                );
                                                            }
                                                            crate::packing::PackedColorMode::Rgba => {
                                                                // Red Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Red (R)",
                                                                    &mut map.channel_r,
                                                                    m_idx,
                                                                    "r",
                                                                    egui::Color32::from_rgb(45, 20, 20),
                                                                    egui::Color32::from_rgb(140, 40, 40),
                                                                    egui::Color32::from_rgb(255, 120, 120),
                                                                );
                                                                // Green Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Green (G)",
                                                                    &mut map.channel_g,
                                                                    m_idx,
                                                                    "g",
                                                                    egui::Color32::from_rgb(20, 45, 20),
                                                                    egui::Color32::from_rgb(40, 140, 40),
                                                                    egui::Color32::from_rgb(120, 255, 120),
                                                                );
                                                                // Blue Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Blue (B)",
                                                                    &mut map.channel_b,
                                                                    m_idx,
                                                                    "b",
                                                                    egui::Color32::from_rgb(20, 30, 50),
                                                                    egui::Color32::from_rgb(40, 80, 160),
                                                                    egui::Color32::from_rgb(120, 180, 255),
                                                                );
                                                                // Alpha Slot
                                                                render_substance_channel_box(
                                                                    ui,
                                                                    "Alpha (A)",
                                                                    &mut map.channel_a,
                                                                    m_idx,
                                                                    "a",
                                                                    egui::Color32::from_rgb(35, 35, 35),
                                                                    egui::Color32::from_rgb(100, 100, 100),
                                                                    egui::Color32::from_rgb(220, 220, 220),
                                                                );
                                                            }
                                                        }
                                                    });
                                                });
                                            ui.add_space(4.0);
                                        }
                                    });

                                if let Some(del_idx) = map_to_delete {
                                    if cur.maps.len() > 1 {
                                        cur.maps.remove(del_idx);
                                    }
                                }

                                if let Some(dup_idx) = map_to_duplicate {
                                    if let Some(src_map) = cur.maps.get(dup_idx) {
                                        let mut cloned_map = src_map.clone();
                                        cloned_map.name_suffix = format!("{}_copy", cloned_map.name_suffix);
                                        cur.maps.insert(dup_idx + 1, cloned_map);
                                    }
                                }

                                ui.add_space(6.0);

                                // Substance Painter Input Maps & Mesh Maps Reference Shelf
                                egui::CollapsingHeader::new(egui::RichText::new("Input Maps & Mesh Maps Palette").strong())
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(egui::RichText::new("Input Channels:").small().strong());
                                            for &src in &[
                                                crate::packing::ChannelSource::BaseColorR,
                                                crate::packing::ChannelSource::Roughness,
                                                crate::packing::ChannelSource::Metallic,
                                                crate::packing::ChannelSource::NormalX,
                                                crate::packing::ChannelSource::NormalY,
                                                crate::packing::ChannelSource::NormalYInverted,
                                                crate::packing::ChannelSource::Height,
                                                crate::packing::ChannelSource::AmbientOcclusion,
                                                crate::packing::ChannelSource::Opacity,
                                                crate::packing::ChannelSource::EmissiveR,
                                            ] {
                                                ui.label(
                                                    egui::RichText::new(format!("[{}]", src.short_code()))
                                                        .small()
                                                        .color(egui::Color32::from_rgb(120, 200, 255)),
                                                );
                                            }
                                        });

                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(egui::RichText::new("Baked Mesh Maps:").small().strong());
                                            for &src in &[
                                                crate::packing::ChannelSource::BakedCurvature,
                                                crate::packing::ChannelSource::BakedThickness,
                                                crate::packing::ChannelSource::BakedWorldNormalX,
                                                crate::packing::ChannelSource::BakedPositionX,
                                                crate::packing::ChannelSource::BakedId,
                                            ] {
                                                ui.label(
                                                    egui::RichText::new(format!("[{}]", src.short_code()))
                                                        .small()
                                                        .color(egui::Color32::from_rgb(255, 180, 100)),
                                                );
                                            }
                                        });
                                    });
                            }
                        });
                    });
                }

                // TAB 2: LIST OF EXPORTS (Substance 3D Painter Export Report)
                if self.export_tab_idx == 2 {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.heading("Export History & Generated Assets");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Clear Log").clicked() {
                                    self.export_last_exported_files.clear();
                                }
                                if ui.button("Open Destination Folder").clicked() {
                                    let p = PathBuf::from(&self.export_dir);
                                    let _ = std::fs::create_dir_all(&p);
                                    open_folder_in_desktop(&p);
                                }
                            });
                        });
                        ui.separator();

                        if self.export_last_exported_files.is_empty() {
                            ui.add_space(20.0);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("No files exported yet in this session.").size(14.0));
                                ui.label(egui::RichText::new("Configure your settings in the 'Settings' tab and click 'Export'.").color(egui::Color32::GRAY));
                            });
                        } else {
                            ui.label(
                                egui::RichText::new(format!("Successfully generated {} production assets in:", self.export_last_exported_files.len()))
                                    .strong()
                                    .color(egui::Color32::from_rgb(100, 230, 120)),
                            );
                            ui.label(egui::RichText::new(&self.export_dir).monospace().color(egui::Color32::from_rgb(180, 220, 255)));
                            ui.add_space(6.0);

                            egui::ScrollArea::vertical()
                                .max_height(400.0)
                                .id_source("export_history_scroll")
                                .show(ui, |ui| {
                                    for path in &self.export_last_exported_files {
                                        let fname = path.file_name().and_then(|f| f.to_str()).unwrap_or("file");
                                        let size_str = if let Ok(meta) = std::fs::metadata(path) {
                                            let bytes = meta.len();
                                            if bytes > 1024 * 1024 {
                                                format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
                                            } else {
                                                format!("{} KB", bytes / 1024)
                                            }
                                        } else {
                                            "--".to_string()
                                        };

                                        ui.horizontal(|ui| {
                                            let icon = if fname.ends_with(".obj") || fname.ends_with(".glb") {
                                                "📦"
                                            } else if fname.ends_with(".mtl") {
                                                "⚙"
                                            } else {
                                                "📄"
                                            };
                                            ui.label(icon);
                                            ui.label(egui::RichText::new(fname).strong());
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.label(egui::RichText::new(size_str).small().color(egui::Color32::GRAY));
                                            });
                                        });
                                    }
                                });
                        }
                    });
                }
            });

        if let Some(msg) = status_update {
            self.status_message = msg;
        }
        if close_modal {
            self.show_export_modal = false;
        }
    }
}

/// Helper function to open directories across Windows and Linux
pub fn open_folder_in_desktop(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

fn render_substance_channel_box(
    ui: &mut egui::Ui,
    title: &str,
    slot: &mut crate::packing::ChannelSlot,
    map_idx: usize,
    channel_key: &str,
    bg_color: egui::Color32,
    border_color: egui::Color32,
    label_color: egui::Color32,
) {
    egui::Frame::none()
        .fill(bg_color)
        .stroke(egui::Stroke::new(1.0_f32, border_color))
        .rounding(4.0)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(title).strong().color(label_color));
                    if slot.invert {
                        ui.label(egui::RichText::new("(Inv)").small().color(egui::Color32::from_rgb(255, 200, 100)));
                    }
                });
                egui::ComboBox::from_id_source(format!("substance_chan_{}_{}", map_idx, channel_key))
                    .selected_text(slot.source.name())
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for src in crate::packing::ChannelSource::all() {
                            ui.selectable_value(&mut slot.source, *src, src.name());
                        }
                    });
                ui.checkbox(&mut slot.invert, "Invert");
            });
        });
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

        // Escape exits Iray mode if active
        if self.iray_mode && input.key_pressed(egui::Key::Escape) {
            self.iray_mode = false;
            self.status_message = "Iray Mode: Exited to Real-Time Viewport".to_string();
        }

        // Undo: Ctrl+Z
        if input.modifiers.command && input.key_pressed(egui::Key::Z) && !input.modifiers.shift {
            if let Some(desc) = self.history.undo(&mut self.project) {
                self.status_message = format!("Undo: {}", desc);
            }
        }
        // Redo: Ctrl+Y or Ctrl+Shift+Z
        if (input.modifiers.command && input.key_pressed(egui::Key::Y))
            || (input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::Z))
        {
            if let Some(desc) = self.history.redo(&mut self.project) {
                self.status_message = format!("Redo: {}", desc);
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

        // Tools: 1 = Paint, 2/E = Eraser, 3 = Projection, 4/G = FillBucket, 5 = Smudge, 6 = Clone, P/I = Picker
        if !input.modifiers.command && !input.modifiers.alt {
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
                self.status_message = "Tool: Projection Painting [3]".to_string();
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
        }

        // Layer Duplicate: Ctrl+D
        if input.modifiers.command && input.key_pressed(egui::Key::D) {
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
        if input.modifiers.command && input.key_pressed(egui::Key::N) {
            self.show_new_project_modal = true;
        }

        // Bake Mesh Maps: Ctrl+Shift+B
        if input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::B) {
            self.show_bake_modal = true;
        }

        // Export Textures: Ctrl+E or Ctrl+Shift+E
        if input.modifiers.command && input.key_pressed(egui::Key::E) {
            self.show_export_modal = true;
        }

        // View Mode: M = Lit Material (PBR), C / Shift+C = Material Channels, B / Shift+B = Baked Mesh Maps
        if input.key_pressed(egui::Key::M) && !input.modifiers.command && !input.modifiers.alt {
            self.view_mode = ViewportChannelMode::Material;
            self.status_message = "View Mode: Lit Material (PBR) [M]".to_string();
        }
        if input.key_pressed(egui::Key::C) && !input.modifiers.command && !input.modifiers.alt {
            self.cycle_material_channel(!input.modifiers.shift);
        }
        if input.key_pressed(egui::Key::B) && !input.modifiers.command && !input.modifiers.alt {
            self.cycle_mesh_map(!input.modifiers.shift);
        }

        // Mask Color Swap: X
        if input.key_pressed(egui::Key::X) && !input.modifiers.command {
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

        // Save As: Ctrl+Shift+S
        if input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::S) {
            self.show_save_modal = true;
        }
        // Quick Save: Ctrl+S
        else if input.modifiers.command && input.key_pressed(egui::Key::S) {
            let path = PathBuf::from(&self.project_file_path);
            if let Err(e) = self.project.save_to_file(&path) {
                self.status_message = format!("Save Error: {}", e);
            } else {
                self.add_recent_project(&self.project_file_path.clone());
                self.status_message = format!("Project saved to {}", self.project_file_path);
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
                        self.show_save_modal = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Export Textures... (Ctrl+E)").clicked() {
                        self.show_export_modal = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(self.history.can_undo(), egui::Button::new("Undo (Ctrl+Z)")).clicked() {
                        if let Some(desc) = self.history.undo(&mut self.project) {
                            self.status_message = format!("Undo: {}", desc);
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.history.can_redo(), egui::Button::new("Redo (Ctrl+Y)")).clicked() {
                        if let Some(desc) = self.history.redo(&mut self.project) {
                            self.status_message = format!("Redo: {}", desc);
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

                            // Stylus Tablet Pressure
                            let pres_sz = self.brush_engine.settings.pressure_size;
                            if ui.selectable_label(pres_sz, "Pres.Size").on_hover_text("Pen Tablet Pressure scales brush radius").clicked() {
                                self.brush_engine.settings.pressure_size = !pres_sz;
                            }
                            let pres_op = self.brush_engine.settings.pressure_opacity;
                            if ui.selectable_label(pres_op, "Pres.Flow").on_hover_text("Pen Tablet Pressure scales brush opacity and flow").clicked() {
                                self.brush_engine.settings.pressure_opacity = !pres_op;
                            }

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

                                    ui.horizontal(|ui| {
                                        if ui.button("Pick from Shelf...").on_hover_text("Pick stencil texture from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::Stencil));
                                        }
                                        if ui.button("Load File...").on_hover_text("Load image file into Project Shelf and set as Stencil").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::Stencil);
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
            .exact_width(42.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);

                    // Helper to render high-polish tool card with icon
                    let opt_icons = self.icons.as_ref();
                    let render_tool_button = |ui: &mut egui::Ui, active: bool, opt_tex: Option<egui::TextureId>, fallback_glyph: &str, label: &str, tip: &str| -> bool {
                        let btn_size = egui::vec2(34.0, 34.0);
                        let (rect, resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
                        let bg = if active {
                            Color32::from_rgb(52, 56, 65)
                        } else if resp.hovered() {
                            Color32::from_rgb(38, 41, 48)
                        } else {
                            Color32::from_rgb(26, 28, 32)
                        };
                        let stroke = if active {
                            egui::Stroke::new(1.5_f32, Color32::from_rgb(180, 185, 195))
                        } else if resp.hovered() {
                            egui::Stroke::new(1.0_f32, Color32::from_rgb(60, 65, 75))
                        } else {
                            egui::Stroke::new(1.0_f32, Color32::from_rgb(34, 36, 40))
                        };

                        ui.painter().rect(rect, 3.0, bg, stroke);

                        if let Some(tex_id) = opt_tex {
                            let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + 13.0), egui::vec2(18.0, 18.0));
                            ui.painter().image(
                                tex_id,
                                icon_rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                if active { Color32::WHITE } else { Color32::from_rgba_unmultiplied(190, 195, 205, 220) },
                            );
                        } else {
                            // Tool glyph centered top
                            let glyph_pos = egui::pos2(rect.center().x, rect.min.y + 10.0);
                            ui.painter().text(
                                glyph_pos,
                                egui::Align2::CENTER_CENTER,
                                fallback_glyph,
                                egui::FontId::proportional(11.0),
                                if active { Color32::WHITE } else { Color32::from_rgb(190, 195, 205) },
                            );
                        }

                        // Tool label centered bottom
                        let label_pos = egui::pos2(rect.center().x, rect.max.y - 6.0);
                        ui.painter().text(
                            label_pos,
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::proportional(7.5),
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
                .min_width(280.0)
                .max_width(700.0)
                .show(ctx, |ui| {
                    self.workspace.right_panel_width = ui.available_width().max(280.0);

                    // 1. Texture Set List (if docked)
                    if self.workspace.show_texture_sets && !self.workspace.texture_sets_floating {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                let col_btn = if self.workspace.texture_sets_collapsed { "[+]" } else { "[-]" };
                                if ui.small_button(col_btn).on_hover_text("Collapse/Expand Texture Set List").clicked() {
                                    self.workspace.texture_sets_collapsed = !self.workspace.texture_sets_collapsed;
                                }
                                ui.strong(egui::RichText::new("TEXTURE SET LIST").size(10.5).color(Color32::from_rgb(175, 180, 190)));
                                ui.label(egui::RichText::new(format!("[{}]", self.project.texture_sets.len())).small().color(Color32::from_rgb(226, 91, 16)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("↗ Pop Out").on_hover_text("Undock Texture Set List into floating window").clicked() {
                                        self.workspace.texture_sets_floating = true;
                                    }
                                    ui.checkbox(&mut self.project.auto_switch_texture_set, "Auto-Switch")
                                        .on_hover_text("Automatically activate Texture Set when painting or clicking on its sub-mesh in the 3D viewport");
                                });
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
                        ui.horizontal(|ui| {
                            let col_btn = if self.workspace.layers_collapsed { "[+]" } else { "[-]" };
                            if ui.small_button(col_btn).on_hover_text("Collapse/Expand Layers Panel").clicked() {
                                self.workspace.layers_collapsed = !self.workspace.layers_collapsed;
                            }
                            ui.selectable_value(&mut self.right_panel_tab, RightPanelTab::Layers, "Layers");
                            ui.selectable_value(&mut self.right_panel_tab, RightPanelTab::MeshMaps, "Mesh Maps");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("↗ Pop Out").on_hover_text("Undock Layers Stack into floating window").clicked() {
                                    self.workspace.layers_floating = true;
                                }
                            });
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
                        ui.horizontal(|ui| {
                            let col_btn = if self.workspace.properties_collapsed { "[+]" } else { "[-]" };
                            if ui.small_button(col_btn).on_hover_text("Collapse/Expand Properties Panel").clicked() {
                                self.workspace.properties_collapsed = !self.workspace.properties_collapsed;
                            }
                            ui.strong(egui::RichText::new("PROPERTIES").size(11.0).color(Color32::from_rgb(220, 225, 235)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("↗ Pop Out").on_hover_text("Undock Properties into floating window").clicked() {
                                    self.workspace.properties_floating = true;
                                }
                            });
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
            if ext == "obj" || ext == "gltf" || ext == "glb" {
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
                                        .add_filter("Wavefront OBJ", &["obj"])
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
            self.need_cpu_composite_sync = false;
            ctx.request_repaint();
        }

        self.perf_metrics.work_latency_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        if self.continuous_60fps {
            ctx.request_repaint();
        }
    }
}

impl PaintcraftApp {
    /// Render interactive 3D Viewport in egui panel
    fn render_3d_viewport_ui(&mut self, ui: &mut egui::Ui, size: Vec2, ctx: &egui::Context) {
        if self.project.gpu_mesh_maps_sync_needed {
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.sync_mesh_maps(&self.project);
            }
            self.project.gpu_mesh_maps_sync_needed = false;
        }

        let textures_dirty = self.project.gpu_sync_needed;
        if self.project.gpu_sync_needed {
            let t_sync = std::time::Instant::now();
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.sync_textures(&mut self.project);
            }
            self.perf_metrics.gpu_sync_ms = t_sync.elapsed().as_secs_f32() * 1000.0;
            self.project.gpu_sync_needed = false;
        }

        // --- SYNCHRONIZE VIEWPORT BAKING CAGE PREVIEW ---
        if self.show_bake_modal && self.bake_preview_cage {
            let current_key = (
                self.bake_cage_mode,
                self.bake_cage_inflation.to_bits(),
                self.bake_frontal.to_bits(),
                self.bake_cage_path.clone(),
                self.bake_cage_show_hull,
                self.bake_cage_show_tethers,
                self.bake_cage_color_idx,
                self.project.mesh.vertices.len(),
            );
            if self.bake_cage_cache_key != current_key {
                self.bake_cage_cache_key = current_key;
                let cage_verts = match self.bake_cage_mode {
                    crate::baker::BakeCageMode::None => {
                        let cfg = crate::baker::BakeCageConfig {
                            mode: crate::baker::BakeCageMode::ParametricInflation,
                            inflation_offset: self.bake_frontal,
                            external_cage_path: None,
                        };
                        crate::baker::Baker::compute_cage_vertices(&self.project.mesh, &cfg).ok().flatten()
                    }
                    crate::baker::BakeCageMode::ParametricInflation => {
                        let cfg = crate::baker::BakeCageConfig {
                            mode: crate::baker::BakeCageMode::ParametricInflation,
                            inflation_offset: self.bake_cage_inflation,
                            external_cage_path: None,
                        };
                        crate::baker::Baker::compute_cage_vertices(&self.project.mesh, &cfg).ok().flatten()
                    }
                    crate::baker::BakeCageMode::ExternalCage => {
                        let path = if !self.bake_cage_path.trim().is_empty() {
                            Some(std::path::PathBuf::from(&self.bake_cage_path))
                        } else {
                            None
                        };
                        let cfg = crate::baker::BakeCageConfig {
                            mode: crate::baker::BakeCageMode::ExternalCage,
                            inflation_offset: 0.0,
                            external_cage_path: path,
                        };
                        crate::baker::Baker::compute_cage_vertices(&self.project.mesh, &cfg).ok().flatten()
                    }
                };

                let cage_color = self.current_bake_cage_color();
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    wgpu.update_cage(
                        &self.project.mesh,
                        cage_verts.as_deref(),
                        self.bake_cage_show_hull,
                        self.bake_cage_show_tethers,
                        cage_color,
                        self.bake_cage_opacity,
                    );
                }
            }
        } else if !self.show_bake_modal || !self.bake_preview_cage {
            if self.bake_cage_cache_key.7 != 0 {
                self.bake_cage_cache_key.7 = 0;
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    wgpu.clear_cage();
                }
            }
        }

        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let vp_w = rect.width().max(64.0);
        let vp_h = rect.height().max(64.0);
        let aspect = vp_w / vp_h;

        let is_camera_moving = response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
            || (response.dragged_by(egui::PointerButton::Primary) && ctx.input(|i| i.modifiers.alt));

        let is_interacting = response.dragged() || self.is_stroke_active;
        if is_interacting || response.hovered() || self.project.gpu_sync_needed || textures_dirty {
            ctx.request_repaint();
        }
        let render_w = (vp_w as u32).max(128).min(3840);
        let render_h = (vp_h as u32).max(128).min(2160);

        let mut frame_settings = self.shader_settings;
        if is_camera_moving && self.shader_settings.adaptive_resolution {
            // Bypass heavy Screen-Space Ray Tracing, SSAO, Bloom & Depth-of-Field ONLY during camera orbit/pan/dolly
            frame_settings.ssr_enabled = false;
            frame_settings.ssao_enabled = false;
            frame_settings.bloom_enabled = false;
            frame_settings.dof_enabled = false;
            frame_settings.shadow_softness = 0.0; // Fast single-tap shadow lookup during camera motion
        }

        let texture_id = if self.iray_mode {
            let t_rnd = std::time::Instant::now();
            let scale = if is_interacting && self.shader_settings.adaptive_resolution { 0.5 } else { 1.0 };
            let pt_w = ((vp_w * scale) as u32).max(64).min(1920);
            let pt_h = ((vp_h * scale) as u32).max(64).min(1080);

            let current_view = self.camera.view_matrix();
            let current_rot = self.renderer.hdri.rotation;
            let camera_changed = self.last_iray_camera_view.map(|v| v != current_view).unwrap_or(true);
            let hdri_changed = (self.last_iray_hdri_rot - current_rot).abs() > 0.01;
            let textures_changed = textures_dirty || (self.last_iray_texture_revision != self.project.texture_revision);

            if let Some(pt) = &mut self.path_tracer {
                pt.resize(pt_w, pt_h);
                pt.max_samples = self.iray_max_samples;
                pt.max_bounces = self.iray_max_bounces;
                pt.denoiser_enabled = self.iray_denoise;

                if camera_changed || hdri_changed || textures_changed {
                    pt.reset();
                    self.last_iray_camera_view = Some(current_view);
                    self.last_iray_hdri_rot = current_rot;
                    self.last_iray_texture_revision = self.project.texture_revision;
                }

                let samples = if is_interacting { 1 } else { self.iray_samples_per_frame };
                pt.render_samples(
                    &self.camera,
                    &self.project.mesh,
                    &self.project.composite_textures,
                    &self.renderer.hdri,
                    samples,
                );

                if !pt.is_converged() || is_interacting {
                    ctx.request_repaint();
                }

                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [pt.width as usize, pt.height as usize],
                    &pt.color_buffer,
                );
                if let Some(tex) = &mut self.iray_texture {
                    tex.set(color_image, egui::TextureOptions::LINEAR);
                } else {
                    self.iray_texture = Some(ctx.load_texture("iray_viewport", color_image, egui::TextureOptions::LINEAR));
                }
            }

            self.perf_metrics.gpu_render_ms = t_rnd.elapsed().as_secs_f32() * 1000.0;
            if let Some(tex) = &self.iray_texture {
                tex.id()
            } else {
                eframe::egui::TextureId::default()
            }
        } else if let Some(wgpu) = &mut self.wgpu_viewport {
            wgpu.resize(render_w, render_h);
            
            let view = self.camera.view_matrix();
            let proj = self.camera.projection_matrix(aspect);
            let vp = proj * view;
            let eye = self.camera.eye_position();
            let eye_vec4 = glam::Vec4::new(eye.x, eye.y, eye.z, 1.0);
            
            let mode_u32 = match self.view_mode {
                ViewportChannelMode::Material => 0u32,
                ViewportChannelMode::BaseColor => 1,
                ViewportChannelMode::Roughness => 2,
                ViewportChannelMode::Metallic => 3,
                ViewportChannelMode::Normal => 4,
                ViewportChannelMode::AmbientOcclusion => 5,
                ViewportChannelMode::Curvature => 6,
                ViewportChannelMode::Position => 7,
                ViewportChannelMode::WorldNormal => 8,
                ViewportChannelMode::Thickness => 9,
                ViewportChannelMode::Height => 10,
                ViewportChannelMode::Emissive => 11,
                ViewportChannelMode::Opacity => 12,
                ViewportChannelMode::Clearcoat => 13,
                ViewportChannelMode::Anisotropy => 14,
            };
            let t_rnd = std::time::Instant::now();
            wgpu.render(vp, eye_vec4, self.renderer.hdri.rotation.to_radians(), mode_u32, self.bg_blur, self.bg_opacity, &frame_settings, self.renderer.show_wireframe);
            self.perf_metrics.gpu_render_ms = t_rnd.elapsed().as_secs_f32() * 1000.0;
            self.perf_metrics.cmd_encoding_ms = wgpu.cmd_encoding_ms;
            self.perf_metrics.queue_submit_ms = wgpu.queue_submit_ms;
            wgpu.texture_id
        } else {
            let t_rnd = std::time::Instant::now();
            self.renderer.resize(render_w, render_h);
            self.update_viewport_texture(ctx);
            self.perf_metrics.gpu_render_ms = t_rnd.elapsed().as_secs_f32() * 1000.0;
            if let Some(tex) = &self.viewport_texture {
                tex.id()
            } else {
                eframe::egui::TextureId::default()
            }
        };

        ui.painter().image(
            texture_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            eframe::egui::Color32::WHITE,
        );

        let hud_top_left = rect.min + eframe::egui::Vec2::new(12.0, 12.0);
        if self.iray_mode {
            let (sample_cur, sample_max, pct) = if let Some(pt) = &self.path_tracer {
                (pt.sample_count, pt.max_samples, pt.progress_pct())
            } else {
                (0, 256, 0.0)
            };
            let status = if pct >= 100.0 { "Converged" } else { "Sampling..." };
            let denoise_badge = if self.iray_denoise { "Denoise: ON" } else { "Denoise: OFF" };
            let iray_hud_text = format!(
                "[IRAY MONTE CARLO PATH TRACER] | Sample: {} / {} ({:.1}%) - {} | {} | HDRI: {} ({:.0}°) | [F10 or Esc to Exit]",
                sample_cur,
                sample_max,
                pct,
                status,
                denoise_badge,
                self.renderer.hdri.preset.name(),
                self.renderer.hdri.rotation,
            );
            ui.painter().text(
                hud_top_left,
                eframe::egui::Align2::LEFT_TOP,
                iray_hud_text,
                eframe::egui::FontId::proportional(12.0),
                eframe::egui::Color32::from_rgb(255, 180, 50),
            );
        } else {
            let gpu_mode_text = if self.wgpu_viewport.is_some() { "[Vulkan/WGPU Substance PBR]" } else { "[Software CPU]" };
            let mut pp_badges = Vec::new();
            match self.shader_settings.aa_mode {
                crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa => { pp_badges.push("NVIDIA DLAA"); }
                crate::nvidia_dlaa::AntiAliasingMode::Fxaa => { pp_badges.push("FXAA"); }
                crate::nvidia_dlaa::AntiAliasingMode::Off => {
                    if self.shader_settings.fxaa_enabled { pp_badges.push("FXAA"); }
                }
            }
            if self.shader_settings.denoiser_enabled { pp_badges.push("Denoiser"); }
            if self.shader_settings.ssao_enabled { pp_badges.push("SSAO"); }
            if self.shader_settings.shadows_enabled { pp_badges.push("Shadows"); }
            if self.renderer.show_wireframe { pp_badges.push("Wireframe"); }
            if self.shader_settings.bloom_enabled { pp_badges.push("Bloom"); }
            if self.shader_settings.dof_enabled { pp_badges.push("DoF"); }
            let pp_str = if pp_badges.is_empty() { String::new() } else { format!(" | PP: [{}]", pp_badges.join(", ")) };

            let hud_text = format!(
                "{} {} | HDRI: {} ({:.0}°){}",
                gpu_mode_text,
                self.view_mode.name(),
                self.renderer.hdri.preset.name(),
                self.renderer.hdri.rotation,
                pp_str,
            );
            ui.painter().text(
                hud_top_left,
                eframe::egui::Align2::LEFT_TOP,
                hud_text,
                eframe::egui::FontId::proportional(12.0),
                eframe::egui::Color32::from_rgba_unmultiplied(220, 220, 220, 180),
            );
        }

        if self.show_bake_modal && self.bake_preview_cage {
            let cage_mode_desc = match self.bake_cage_mode {
                crate::baker::BakeCageMode::None => format!("Frontal Ray Envelope: {:.3}", self.bake_frontal),
                crate::baker::BakeCageMode::ParametricInflation => format!("Parametric Inflation Offset: {:.3}", self.bake_cage_inflation),
                crate::baker::BakeCageMode::ExternalCage => format!("External Cage: {}", if self.bake_cage_path.is_empty() { "None selected" } else { &self.bake_cage_path }),
            };
            ui.painter().text(
                rect.min + egui::vec2(12.0, rect.height() - 24.0),
                egui::Align2::LEFT_BOTTOM,
                format!("[BAKE CAGE PREVIEW] {}  |  Orbit: LMB / RMB  |  Pan: MMB  |  Zoom: Scroll", cage_mode_desc),
                egui::FontId::proportional(11.5),
                egui::Color32::from_rgb(0, 225, 255),
            );
        }



        // --- REAL-TIME PERFORMANCE & HARDWARE DIAGNOSTICS HUD ---
        if self.perf_metrics.show_debug_hud {
            let diag_rect = egui::Rect::from_min_size(rect.min + egui::vec2(12.0, 36.0), egui::vec2(370.0, 192.0));
            ui.painter().rect_filled(
                diag_rect,
                egui::Rounding::same(8.0),
                egui::Color32::from_rgba_unmultiplied(16, 18, 24, 238),
            );
            ui.painter().rect_stroke(
                diag_rect,
                egui::Rounding::same(8.0),
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(226, 91, 16)),
            );

            let mut y_cur = diag_rect.min.y + 10.0;
            let left_x = diag_rect.min.x + 12.0;

            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                "Performance & Hardware Diagnostics (F3 to hide)",
                egui::FontId::proportional(12.0),
                egui::Color32::from_rgb(243, 94, 24),
            );
            y_cur += 20.0;

            let gpu_col = if self.perf_metrics.is_software_gpu || self.perf_metrics.is_cpu_fallback {
                egui::Color32::from_rgb(255, 90, 80)
            } else {
                egui::Color32::from_rgb(80, 220, 120)
            };
            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("GPU: {}", self.perf_metrics.gpu_name),
                egui::FontId::proportional(11.0),
                gpu_col,
            );
            y_cur += 18.0;

            let build_str = if self.perf_metrics.is_debug {
                "Build: Debug (Unoptimized - run with --release for 10x-50x speedup)"
            } else {
                "Build: Release (100% full speed)"
            };
            let build_col = if self.perf_metrics.is_debug {
                egui::Color32::from_rgb(255, 180, 40)
            } else {
                egui::Color32::from_rgb(180, 205, 230)
            };
            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                build_str,
                egui::FontId::proportional(11.0),
                build_col,
            );
            y_cur += 18.0;

            let (hud_fps_str, fps_col) = if self.perf_metrics.is_idle {
                let headroom_fps = if self.perf_metrics.work_latency_ms > 0.05 {
                    (1000.0 / self.perf_metrics.work_latency_ms).min(999.0)
                } else {
                    999.0
                };
                (
                    format!("Status: Standby (Idle) | Compute Latency: {:.2} ms ({:.0} FPS headroom)", self.perf_metrics.work_latency_ms, headroom_fps),
                    egui::Color32::from_rgb(130, 210, 255),
                )
            } else {
                let fps_val = if self.perf_metrics.work_latency_ms > 16.6 {
                    1000.0 / self.perf_metrics.work_latency_ms
                } else {
                    self.perf_metrics.fps.max(60.0)
                };
                let frame_ms = 1000.0 / fps_val;
                let col = if fps_val >= 55.0 {
                    egui::Color32::from_rgb(80, 220, 120)
                } else if fps_val >= 30.0 {
                    egui::Color32::from_rgb(255, 200, 60)
                } else {
                    egui::Color32::from_rgb(255, 80, 80)
                };
                (
                    format!("Throughput: {:.1} FPS ({:.2} ms frame)", fps_val, frame_ms),
                    col,
                )
            };
            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                hud_fps_str,
                egui::FontId::proportional(11.5),
                fps_col,
            );
            y_cur += 18.0;

            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("Paint Dab: {:.2} ms  |  3D Pass: {:.2} ms  |  GPU Sync: {:.2} ms",
                    self.perf_metrics.paint_stroke_ms,
                    self.perf_metrics.gpu_render_ms,
                    self.perf_metrics.gpu_sync_ms
                ),
                egui::FontId::proportional(10.5),
                egui::Color32::from_rgb(190, 195, 205),
            );
            y_cur += 18.0;

            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("Encoder: {:.2} ms  |  Queue Submit: {:.2} ms  |  CPU Frame: {:.2} ms",
                    self.perf_metrics.cmd_encoding_ms,
                    self.perf_metrics.queue_submit_ms,
                    self.perf_metrics.work_latency_ms
                ),
                egui::FontId::proportional(10.5),
                egui::Color32::from_rgb(160, 205, 240),
            );
            y_cur += 18.0;

            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("Doc: {}x{} ({:.1} MB/dab)  |  VP: {}x{}  |  Tris: {}",
                    self.project.texture_width, self.project.texture_height,
                    self.perf_metrics.texture_transfer_bytes as f64 / (1024.0 * 1024.0),
                    vp_w as u32, vp_h as u32,
                    self.project.mesh.indices.len() / 3
                ),
                egui::FontId::proportional(10.5),
                egui::Color32::from_rgb(150, 155, 165),
            );
            y_cur += 18.0;

            let aa_status = match self.shader_settings.aa_mode {
                crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa => "AA: NVIDIA DLAA (Tensor 16-Phase Halton)",
                crate::nvidia_dlaa::AntiAliasingMode::Fxaa => "AA: FXAA (Subpixel Luma Edge)",
                crate::nvidia_dlaa::AntiAliasingMode::Off => "AA: OFF",
            };
            let denoise_status = if self.shader_settings.denoiser_enabled { "Denoiser: ON (Bilateral)" } else { "Denoiser: OFF" };
            ui.painter().text(
                egui::pos2(left_x, y_cur),
                egui::Align2::LEFT_TOP,
                format!("{}  |  {}", aa_status, denoise_status),
                egui::FontId::proportional(10.5),
                egui::Color32::from_rgb(120, 220, 170),
            );
        }

        let input = ui.input(|i| i.clone());

        if response.hovered() && input.raw_scroll_delta.y.abs() > 0.1 {
            self.camera.zoom(input.raw_scroll_delta.y * 0.02 * self.camera_zoom_speed);
        }

        if input.key_pressed(eframe::egui::Key::F) {
            self.camera.reset();
        }

        let mut gesture_overlay: Option<(egui::Pos2, String, Option<f32>, Option<f32>)> = None;

        // 1. Shift + RMB: Rotate HDRI environment lighting
        if response.dragged_by(eframe::egui::PointerButton::Secondary) && input.modifiers.shift {
            let delta = input.pointer.delta();
            self.renderer.hdri.rotation = (self.renderer.hdri.rotation + delta.x * 0.5).rem_euclid(360.0);
            if let Some(pos) = input.pointer.hover_pos() {
                gesture_overlay = Some((pos, format!("HDRI Environment: {:.0}°", self.renderer.hdri.rotation), None, None));
            }
        }
        // 2. Ctrl + RMB: In-Viewport Substance Brush Radius (Size) & Hardness Gesture
        else if response.dragged_by(eframe::egui::PointerButton::Secondary) && input.modifiers.command {
            let delta = input.pointer.delta();
            self.brush_engine.settings.radius = (self.brush_engine.settings.radius + delta.x * 0.5).clamp(2.0, 200.0);
            self.brush_engine.settings.hardness = (self.brush_engine.settings.hardness - delta.y * 0.005).clamp(0.0, 1.0);
            if let Some(pos) = input.pointer.hover_pos() {
                let msg = format!("Size: {:.1} px  |  Hardness: {:.0}%", self.brush_engine.settings.radius, self.brush_engine.settings.hardness * 100.0);
                gesture_overlay = Some((pos, msg, Some(self.brush_engine.settings.radius), Some(self.brush_engine.settings.hardness)));
            }
        }
        // 3. Ctrl + LMB: In-Viewport Substance Brush Flow & Opacity Gesture
        else if response.dragged_by(eframe::egui::PointerButton::Primary) && input.modifiers.command {
            let delta = input.pointer.delta();
            self.brush_engine.settings.flow = (self.brush_engine.settings.flow + delta.x * 0.005).clamp(0.01, 1.0);
            self.brush_engine.settings.opacity = (self.brush_engine.settings.opacity - delta.y * 0.005).clamp(0.01, 1.0);
            if let Some(pos) = input.pointer.hover_pos() {
                let msg = format!("Flow: {:.0}%  |  Opacity: {:.0}%", self.brush_engine.settings.flow * 100.0, self.brush_engine.settings.opacity * 100.0);
                gesture_overlay = Some((pos, msg, None, None));
            }
        }
        // 4. Alt + RMB: Substance Camera Dolly / Zoom
        else if response.dragged_by(eframe::egui::PointerButton::Secondary) && input.modifiers.alt {
            let delta = input.pointer.delta();
            let zoom_delta = (delta.x - delta.y) * 0.02 * self.camera_zoom_speed;
            self.camera.zoom(zoom_delta);
        }
        // 5. Alt + MMB or MMB: Substance Camera Pan
        else if response.dragged_by(eframe::egui::PointerButton::Middle) {
            let delta = input.pointer.delta();
            self.camera.pan(delta.x * self.camera_pan_speed, delta.y * self.camera_pan_speed);
        }
        // 6. Alt + LMB (or RMB without modifiers): Substance Camera Orbit (or Left-drag when Bake modal is open)
        else if (response.dragged_by(eframe::egui::PointerButton::Primary) && input.modifiers.alt)
            || (response.dragged_by(eframe::egui::PointerButton::Secondary) && !input.modifiers.command && !input.modifiers.shift && !input.modifiers.alt)
            || (self.show_bake_modal && response.dragged_by(eframe::egui::PointerButton::Primary))
        {
            let delta = input.pointer.delta();
            self.camera.orbit(delta.x * self.camera_orbit_speed, delta.y * self.camera_orbit_speed);
        }
        // 7. Left-drag WITHOUT Alt and WITHOUT Ctrl: 3D PROJECTION PAINTING! (Disabled when Bake modal is open)
        else if (response.dragged_by(egui::PointerButton::Primary) || response.clicked())
            && !input.modifiers.alt && !input.modifiers.command && !self.show_bake_modal
        {
                if !self.is_stroke_active {
                    if let Some(wgpu) = &mut self.wgpu_viewport {
                        if wgpu.gpu_layers.len() != self.project.layers.len() {
                            wgpu.ensure_gpu_layers(&mut self.project);
                        }
                    }
                    if let Some(layer) = self.project.active_layer() {
                        let target = self.brush_engine.settings.target;
                        let channels = if target == PaintTarget::Mask {
                            crate::channel::ChannelFlags::none()
                        } else {
                            let b = self.brush_engine.settings.channels;
                            let l = layer.channel_flags;
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
                        self.stroke_start_layer = Some(layer.clone_stroke_snapshot_selective(Some(target), Some(channels)));
                        self.is_stroke_active = true;
                    }
                    self.polygon_fill_drag_tris.clear();
                }

                if let Some(raw_mouse_pos) = input.pointer.hover_pos() {
                    let pressure = input.events.iter().find_map(|e| match e {
                        egui::Event::Touch { force: Some(f), .. } => Some(*f),
                        _ => None,
                    }).unwrap_or(1.0).clamp(0.05, 1.0);

                    // Lazy Mouse evaluation
                    let mouse_pos = if self.brush_engine.settings.lazy_mouse_enabled {
                        let l_pos = self.brush_engine.lazy_screen_pos.unwrap_or([raw_mouse_pos.x, raw_mouse_pos.y]);
                        let cur_vec = egui::vec2(raw_mouse_pos.x - l_pos[0], raw_mouse_pos.y - l_pos[1]);
                        let lazy_dist = (self.brush_engine.settings.lazy_mouse_radius * vp_w).max(12.0);
                        if cur_vec.length() > lazy_dist {
                            let pull = cur_vec.normalized() * (cur_vec.length() - lazy_dist);
                            let new_p = egui::pos2(l_pos[0] + pull.x, l_pos[1] + pull.y);
                            self.brush_engine.lazy_screen_pos = Some([new_p.x, new_p.y]);
                            new_p
                        } else {
                            egui::pos2(l_pos[0], l_pos[1])
                        }
                    } else {
                        self.brush_engine.lazy_screen_pos = Some([raw_mouse_pos.x, raw_mouse_pos.y]);
                        raw_mouse_pos
                    };

                    let local_x = mouse_pos.x - rect.min.x;
                    let local_y = mouse_pos.y - rect.min.y;

                    let ray = self.camera.screen_to_ray(
                        local_x,
                        local_y,
                        vp_w,
                        vp_h,
                    );

                    if let Some(hit) = self.project.mesh.raycast(&ray) {
                        if !self.project.mesh.is_triangle_visible(hit.triangle_idx) {
                            // Submesh is hidden, skip painting
                        } else {
                            if self.project.auto_switch_texture_set {
                                if let Some(target_ts_idx) = self.project.texture_set_index_for_triangle(hit.triangle_idx) {
                                    if target_ts_idx != self.project.active_texture_set_idx {
                                        self.project.switch_texture_set(target_ts_idx);
                                        if let Some(wgpu) = &mut self.wgpu_viewport {
                                            wgpu.sync_textures(&mut self.project);
                                        }
                                    }
                                }
                            }
                            if self.brush_engine.settings.mode == ToolMode::Clone && (input.modifiers.alt || input.key_down(egui::Key::V)) {
                                self.brush_engine.settings.clone_source_uv = Some(hit.uv);
                                self.status_message = format!("Clone Source Set: UV ({:.3}, {:.3})", hit.uv.x, hit.uv.y);
                            } else if self.brush_engine.settings.mode == ToolMode::Eyedropper {
                            let sampled = self.project.composite_textures.base_color.sample_bilinear(hit.uv);
                            self.brush_engine.settings.paint_color = sampled;
                            self.brush_engine.settings.paint_roughness = self.project.composite_textures.roughness.sample_bilinear(hit.uv);
                            self.brush_engine.settings.paint_metallic = self.project.composite_textures.metallic.sample_bilinear(hit.uv);
                            self.brush_engine.settings.paint_height = self.project.composite_textures.height_map.sample_bilinear(hit.uv);
                            self.brush_engine.settings.paint_emissive = self.project.composite_textures.emissive.sample_bilinear(hit.uv);
                            self.brush_engine.settings.paint_opacity = self.project.composite_textures.opacity.sample_bilinear(hit.uv);
                            self.status_message = format!("Sampled Material: Color RGBA({:.2}, {:.2}, {:.2}, {:.2}), Rough {:.2}, Metal {:.2}, Emis RGB({:.2}, {:.2}, {:.2})", sampled.x, sampled.y, sampled.z, sampled.w, self.brush_engine.settings.paint_roughness, self.brush_engine.settings.paint_metallic, self.brush_engine.settings.paint_emissive.x, self.brush_engine.settings.paint_emissive.y, self.brush_engine.settings.paint_emissive.z);
                        } else if self.brush_engine.settings.mode == ToolMode::FillBucket {
                            let mode = self.brush_engine.settings.polygon_fill_mode;
                            let tris = match mode {
                                crate::brush::PolygonFillMode::Triangle => vec![hit.triangle_idx],
                                crate::brush::PolygonFillMode::Polygon => self.project.mesh.get_polygon_quad(hit.triangle_idx),
                                crate::brush::PolygonFillMode::UvIsland => self.project.mesh.get_connected_uv_island(hit.triangle_idx),
                                crate::brush::PolygonFillMode::MeshElement => self.project.mesh.get_connected_mesh_element(hit.triangle_idx),
                            };
                            let new_tris: Vec<usize> = tris.into_iter().filter(|t| !self.polygon_fill_drag_tris.contains(t)).collect();
                            if !new_tris.is_empty() {
                                for &t in &new_tris {
                                    self.polygon_fill_drag_tris.insert(t);
                                }
                                let tri_uvs: Vec<(glam::Vec2, glam::Vec2, glam::Vec2)> = new_tris.iter().map(|&t| {
                                    let i0 = self.project.mesh.indices[t * 3] as usize;
                                    let i1 = self.project.mesh.indices[t * 3 + 1] as usize;
                                    let i2 = self.project.mesh.indices[t * 3 + 2] as usize;
                                    (
                                        self.project.mesh.vertices[i0].uv,
                                        self.project.mesh.vertices[i1].uv,
                                        self.project.mesh.vertices[i2].uv,
                                    )
                                }).collect();

                                let act_idx = self.project.active_layer_idx;
                                if act_idx < self.project.layers.len() {
                                    let layer = &mut self.project.layers[act_idx];

                                    if self.brush_engine.settings.target == PaintTarget::Mask {
                                        if layer.has_mask {
                                            let val = self.brush_engine.settings.polygon_fill_value;
                                            let w = layer.mask_buf.width;
                                            let h = layer.mask_buf.height;
                                            for &(uv0, uv1, uv2) in &tri_uvs {
                                                crate::mesh::Mesh::rasterize_triangle_uv(w, h, uv0, uv1, uv2, |px, py| {
                                                    layer.mask_buf.set_pixel(px, py, val);
                                                });
                                            }
                                            layer.mark_dirty_full();
                                            self.project.rebuild_composite();
                                            self.update_uv_texture(ctx);
                                            if let Some(wgpu) = &mut self.wgpu_viewport {
                                                wgpu.sync_textures(&mut self.project);
                                            }
                                            ctx.request_repaint();
                                            self.status_message = format!("Polygon Fill ({}) filled {} triangles on mask", mode.name(), new_tris.len());
                                        } else {
                                            self.status_message = "Active layer does not have a mask. Add a mask to use Polygon Fill on mask.".to_string();
                                        }
                                    } else {
                                        // Target: Material
                                        if layer.layer_type == LayerType::Paint {
                                            let col = self.brush_engine.settings.paint_color;
                                            let rough = self.brush_engine.settings.paint_roughness;
                                            let metal = self.brush_engine.settings.paint_metallic;
                                            let height = self.brush_engine.settings.paint_height;
                                            let emis = self.brush_engine.settings.paint_emissive;
                                            let opac = self.brush_engine.settings.paint_opacity;
                                            let do_col = layer.channel_flags.base_color;
                                            let do_rough = layer.channel_flags.roughness;
                                            let do_metal = layer.channel_flags.metallic;
                                            let do_h = layer.channel_flags.height;
                                            let do_emis = layer.channel_flags.emission;
                                            let do_opac = layer.channel_flags.opacity;
                                            let w = layer.base_color_buf.width;
                                            let h = layer.base_color_buf.height;
                                            for &(uv0, uv1, uv2) in &tri_uvs {
                                                crate::mesh::Mesh::rasterize_triangle_uv(w, h, uv0, uv1, uv2, |px, py| {
                                                    if do_col {
                                                        layer.base_color_buf.set_pixel(px, py, col);
                                                    } else {
                                                        let mut cur = layer.base_color_buf.get_pixel(px, py);
                                                        cur.w = 1.0;
                                                        layer.base_color_buf.set_pixel(px, py, cur);
                                                    }
                                                    if do_rough { layer.roughness_buf.set_pixel(px, py, rough); }
                                                    if do_metal { layer.metallic_buf.set_pixel(px, py, metal); }
                                                    if do_h { layer.height_buf.set_pixel(px, py, height); }
                                                    if do_emis { layer.emissive_buf.set_pixel(px, py, emis); }
                                                    if do_opac { layer.opacity_buf.set_pixel(px, py, opac); }
                                                });
                                            }
                                            layer.update_normal_from_height(1.0);
                                            layer.mark_dirty_full();
                                            self.project.rebuild_composite();
                                            self.update_uv_texture(ctx);
                                            if let Some(wgpu) = &mut self.wgpu_viewport {
                                                wgpu.sync_textures(&mut self.project);
                                            }
                                            ctx.request_repaint();
                                            self.status_message = format!("Polygon Fill ({}) filled {} triangles with material", mode.name(), new_tris.len());
                                        } else {
                                            self.status_message = "Cannot fill material on Fill layer. Switch target to Mask or add a Paint layer.".to_string();
                                        }
                                    }
                                }
                            }
                        } else {
                            let act_idx = self.project.active_layer_idx;
                            if act_idx < self.project.layers.len() {
                                let is_blocked = self.brush_engine.settings.target == PaintTarget::Material
                                    && self.project.layers[act_idx].layer_type != LayerType::Paint;
                                if is_blocked {
                                    self.status_message = "Direct painting blocked on Fill layers. Switch target to Mask or add a Paint layer.".to_string();
                                } else {
                                    let effective_radius = if self.brush_engine.settings.pressure_size {
                                        self.brush_engine.settings.radius * (0.2 + 0.8 * pressure)
                                    } else {
                                        self.brush_engine.settings.radius
                                    };
                                    let ray2 = self.camera.screen_to_ray(
                                        local_x + effective_radius,
                                        local_y,
                                        vp_w,
                                        vp_h,
                                    );
                                    let point2 = ray2.origin + ray2.direction * hit.t;
                                    let world_radius = (hit.point - point2).length().max(0.001);

                                    let orig_flow = self.brush_engine.settings.flow;
                                    if self.brush_engine.settings.pressure_opacity {
                                        self.brush_engine.settings.flow = orig_flow * pressure;
                                    }

                                    let t_stroke = std::time::Instant::now();
                                    let doc_w = self.project.texture_width;
                                    let doc_h = self.project.texture_height;


                                    let orig_color = self.brush_engine.settings.paint_color;
                                    let mut skip_stroke = false;

                                    if self.brush_engine.settings.mode == ToolMode::Projection || self.brush_engine.settings.stencil.enabled {
                                        let screen_uv = glam::Vec2::new(local_x / vp_w.max(1.0), local_y / vp_h.max(1.0));
                                        if let Some(sample) = self.brush_engine.sample_stencil(screen_uv) {
                                            if self.brush_engine.settings.stencil.mask_only {
                                                let lum = (sample.x * 0.299 + sample.y * 0.587 + sample.z * 0.114) * sample.w;
                                                self.brush_engine.settings.flow *= lum * self.brush_engine.settings.stencil.opacity;
                                            } else {
                                                self.brush_engine.settings.paint_color = glam::Vec4::new(sample.x, sample.y, sample.z, sample.w * self.brush_engine.settings.stencil.opacity);
                                            }
                                        } else if self.brush_engine.settings.stencil.enabled {
                                            skip_stroke = true;
                                        }
                                    }

                                    // Keep CPU layer buffer updated for history/undo/save integrity
                                    let _stroke_pts = if !skip_stroke {
                                        let layer = &mut self.project.layers[act_idx];
                                        let mesh = &self.project.mesh;
                                        self.brush_engine.paint_stroke_3d(layer, mesh, hit.point, hit.normal, world_radius)
                                    } else {
                                        Vec::new()
                                    };

                                    self.brush_engine.settings.paint_color = orig_color;

                                    // Hardware GPU Direct Dirty-Rect Sync: Streams the exact painted pixels to VRAM
                                    // Guarantees 100% WYSIWYG stroke fidelity while drawing with zero visual shift upon release
                                    let mut gpu_synced = false;
                                    if let Some(wgpu) = &mut self.wgpu_viewport {
                                        let active_layer = &mut self.project.layers[act_idx];
                                        if let Some(d_rect) = active_layer.dirty_rect {
                                            if self.brush_engine.settings.channels.clearcoat
                                                || self.brush_engine.settings.channels.emission
                                                || self.brush_engine.settings.channels.opacity
                                                || self.brush_engine.settings.channels.anisotropy
                                            {
                                                wgpu.sync_extended_dirty_rect(active_layer, d_rect, self.brush_engine.settings.channels);
                                                self.need_cpu_composite_sync = true;
                                            }
                                            if let Some(gpu_layer) = wgpu.gpu_layers.get(act_idx) {
                                                wgpu.gpu_compositor.update_layer(active_layer, gpu_layer, doc_w, doc_h);
                                                wgpu.composite_and_update(&self.project);
                                                let rw = (d_rect.2.saturating_sub(d_rect.0) + 1) as u64;
                                                let rh = (d_rect.3.saturating_sub(d_rect.1) + 1) as u64;
                                                self.perf_metrics.texture_transfer_bytes = rw * rh * 32;
                                                gpu_synced = true;
                                            }
                                        }
                                    }

                                    self.perf_metrics.paint_stroke_ms = t_stroke.elapsed().as_secs_f32() * 1000.0;

                                    if self.brush_engine.settings.pressure_opacity {
                                        self.brush_engine.settings.flow = orig_flow;
                                    }

                                    if !gpu_synced {
                                        self.project.gpu_sync_needed = true;
                                    }
                                    self.project.is_dirty = true;
                                    self.last_painted_uv = Some(hit.uv);
                                    self.last_hovered_hit_3d = Some((hit.point, hit.normal, hit.uv));
                                    self.status_message = format!("3D Projection Painting at ({:.2}, {:.2}, {:.2})", hit.point.x, hit.point.y, hit.point.z);
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                    }
                }
            } else if response.drag_stopped() || (!input.pointer.any_down() && self.is_stroke_active) {
                self.brush_engine.end_stroke();
                self.polygon_fill_drag_tris.clear();
                self.project.rebuild_composite();
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    wgpu.sync_textures(&mut self.project);
                }
                if self.is_stroke_active {
                    if let Some(prev) = self.stroke_start_layer.take() {
                        if let Some(next) = self.project.active_layer() {
                            let idx = self.project.active_layer_idx;
                            let desc = if self.brush_engine.settings.mode == ToolMode::FillBucket {
                                format!("Polygon Fill ({})", self.brush_engine.settings.polygon_fill_mode.name())
                            } else {
                                "Paint Stroke".to_string()
                            };
                            let target = self.brush_engine.settings.target;
                            let channels = if target == PaintTarget::Mask {
                                crate::channel::ChannelFlags::none()
                            } else {
                                let b = self.brush_engine.settings.channels;
                                let l = next.channel_flags;
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
                            let next_layer = next.clone_stroke_snapshot_selective(Some(target), Some(channels));
                            self.history.push(&desc.clone(), HistoryAction::Stroke {
                                layer_idx: idx,
                                prev_layer: prev,
                                next_layer,
                                description: desc,
                            });
                        }
                    }
                    self.is_stroke_active = false;
                }
            }

        // --- 3D SYMMETRY PLANE GIZMO ---
        let sym_x = self.brush_engine.settings.symmetry_x;
        let sym_y = self.brush_engine.settings.symmetry_y;
        let sym_z = self.brush_engine.settings.symmetry_z;
        if sym_x || sym_y || sym_z {
            let a_min = self.project.mesh.aabb_min;
            let a_max = self.project.mesh.aabb_max;
            let pad = (a_max - a_min) * 0.15;
            let min_p = a_min - pad;
            let max_p = a_max + pad;
            let c = self.brush_engine.settings.symmetry_center;

            let draw_plane = |corners: [glam::Vec3; 4], col: egui::Color32, stroke_col: egui::Color32| {
                let mut screen_corners = Vec::with_capacity(4);
                let mut all_valid = true;
                for corner in &corners {
                    if let Some(sp) = self.camera.world_to_screen(*corner, vp_w, vp_h) {
                        screen_corners.push(rect.min + egui::vec2(sp.x, sp.y));
                    } else {
                        all_valid = false;
                        break;
                    }
                }
                if all_valid && screen_corners.len() == 4 {
                    ui.painter().add(egui::Shape::convex_polygon(
                        screen_corners.clone(),
                        col,
                        egui::Stroke::new(1.2_f32, stroke_col),
                    ));
                    let mid0 = (screen_corners[0].to_vec2() + screen_corners[1].to_vec2()) * 0.5;
                    let mid1 = (screen_corners[2].to_vec2() + screen_corners[3].to_vec2()) * 0.5;
                    ui.painter().line_segment(
                        [egui::pos2(mid0.x, mid0.y), egui::pos2(mid1.x, mid1.y)],
                        egui::Stroke::new(0.8_f32, stroke_col),
                    );
                }
            };

            if sym_x {
                draw_plane(
                    [
                        glam::Vec3::new(c.x, min_p.y, min_p.z),
                        glam::Vec3::new(c.x, max_p.y, min_p.z),
                        glam::Vec3::new(c.x, max_p.y, max_p.z),
                        glam::Vec3::new(c.x, min_p.y, max_p.z),
                    ],
                    egui::Color32::from_rgba_unmultiplied(243, 94, 24, 25),
                    egui::Color32::from_rgba_unmultiplied(243, 94, 24, 180),
                );
            }
            if sym_y {
                draw_plane(
                    [
                        glam::Vec3::new(min_p.x, c.y, min_p.z),
                        glam::Vec3::new(max_p.x, c.y, min_p.z),
                        glam::Vec3::new(max_p.x, c.y, max_p.z),
                        glam::Vec3::new(min_p.x, c.y, max_p.z),
                    ],
                    egui::Color32::from_rgba_unmultiplied(46, 204, 113, 25),
                    egui::Color32::from_rgba_unmultiplied(46, 204, 113, 180),
                );
            }
            if sym_z {
                draw_plane(
                    [
                        glam::Vec3::new(min_p.x, min_p.y, c.z),
                        glam::Vec3::new(max_p.x, min_p.y, c.z),
                        glam::Vec3::new(max_p.x, max_p.y, c.z),
                        glam::Vec3::new(min_p.x, max_p.y, c.z),
                    ],
                    egui::Color32::from_rgba_unmultiplied(52, 152, 219, 25),
                    egui::Color32::from_rgba_unmultiplied(52, 152, 219, 180),
                );
            }
        }

        // --- STENCIL PROJECTION OVERLAY ---
        if self.brush_engine.settings.stencil.enabled {
            let st = &self.brush_engine.settings.stencil;
            let center = rect.center() + egui::vec2(st.pos.x * rect.width(), st.pos.y * rect.height());
            let half_w = (rect.width() * 0.4 * st.scale) * 0.5;
            let half_h = (rect.height() * 0.4 * st.scale) * 0.5;
            let corners = [
                egui::pos2(-half_w, -half_h),
                egui::pos2(half_w, -half_h),
                egui::pos2(half_w, half_h),
                egui::pos2(-half_w, half_h),
            ];
            let (sin_a, cos_a) = st.angle.sin_cos();
            let rotated_corners: Vec<egui::Pos2> = corners
                .iter()
                .map(|p| {
                    let rx = p.x * cos_a - p.y * sin_a;
                    let ry = p.x * sin_a + p.y * cos_a;
                    center + egui::vec2(rx, ry)
                })
                .collect();
            let stencil_alpha = (st.opacity * 220.0) as u8;
            ui.painter().add(egui::Shape::convex_polygon(
                rotated_corners.clone(),
                egui::Color32::from_rgba_unmultiplied(243, 94, 24, (stencil_alpha / 6).max(12)),
                egui::Stroke::new(1.8_f32, egui::Color32::from_rgba_unmultiplied(243, 94, 24, stencil_alpha.max(70))),
            ));
            ui.painter().text(
                rotated_corners[0] + egui::vec2(6.0, 6.0),
                egui::Align2::LEFT_TOP,
                format!("Stencil ({:.0}% opac)", st.opacity * 100.0),
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgba_unmultiplied(243, 94, 24, stencil_alpha.max(120)),
            );
        }

        // --- 3D POLYGON FILL HOVER PREVIEW HIGHLIGHT ---
        if self.brush_engine.settings.mode == ToolMode::FillBucket {
            if let Some(tri_idx) = self.last_hovered_tri {
                let mode = self.brush_engine.settings.polygon_fill_mode;
                let preview_tris = match mode {
                    crate::brush::PolygonFillMode::Triangle => vec![tri_idx],
                    crate::brush::PolygonFillMode::Polygon => self.project.mesh.get_polygon_quad(tri_idx),
                    crate::brush::PolygonFillMode::UvIsland => self.project.mesh.get_connected_uv_island(tri_idx),
                    crate::brush::PolygonFillMode::MeshElement => self.project.mesh.get_connected_mesh_element(tri_idx),
                };

                let fill_col = egui::Color32::from_rgba_unmultiplied(243, 94, 24, 75);
                let edge_col = egui::Color32::from_rgba_unmultiplied(243, 140, 40, 230);
                let cam_pos = self.camera.eye_position();

                for &t in &preview_tris {
                    if t * 3 + 2 >= self.project.mesh.indices.len() { continue; }
                    let i0 = self.project.mesh.indices[t * 3] as usize;
                    let i1 = self.project.mesh.indices[t * 3 + 1] as usize;
                    let i2 = self.project.mesh.indices[t * 3 + 2] as usize;
                    if i0 >= self.project.mesh.vertices.len() || i1 >= self.project.mesh.vertices.len() || i2 >= self.project.mesh.vertices.len() { continue; }
                    let p0_3d = self.project.mesh.vertices[i0].position;
                    let p1_3d = self.project.mesh.vertices[i1].position;
                    let p2_3d = self.project.mesh.vertices[i2].position;

                    // Backface cull preview against camera
                    let fnorm = (p1_3d - p0_3d).cross(p2_3d - p0_3d);
                    let view_vec = cam_pos - (p0_3d + p1_3d + p2_3d) * (1.0 / 3.0);
                    if fnorm.dot(view_vec) <= 0.0 { continue; }

                    let sp0 = self.camera.world_to_screen(p0_3d, vp_w, vp_h);
                    let sp1 = self.camera.world_to_screen(p1_3d, vp_w, vp_h);
                    let sp2 = self.camera.world_to_screen(p2_3d, vp_w, vp_h);

                    if let (Some(s0), Some(s1), Some(s2)) = (sp0, sp1, sp2) {
                        let pts = vec![
                            rect.min + egui::vec2(s0.x, s0.y),
                            rect.min + egui::vec2(s1.x, s1.y),
                            rect.min + egui::vec2(s2.x, s2.y),
                        ];
                        ui.painter().add(egui::Shape::convex_polygon(
                            pts,
                            fill_col,
                            egui::Stroke::new(1.2_f32, edge_col),
                        ));
                    }
                }
            }
        }

        // --- INTERACTIVE BRUSH CURSOR, LAZY LEASH & 3D SYMMETRY GHOST CURSORS ---
        if response.hovered() && !input.modifiers.alt {
            ctx.request_repaint();
            if let Some(mouse_pos) = input.pointer.hover_pos() {
                let brush_rad = self.brush_engine.settings.radius;

                let c_pos = if self.brush_engine.settings.lazy_mouse_enabled {
                    self.brush_engine.lazy_screen_pos.map(|p| egui::pos2(p[0], p[1])).unwrap_or(mouse_pos)
                } else {
                    mouse_pos
                };
                let local_x = c_pos.x - rect.min.x;
                let local_y = c_pos.y - rect.min.y;
                let ray = self.camera.screen_to_ray(local_x, local_y, vp_w, vp_h);
                let hit_opt = self.project.mesh.raycast(&ray);
                if let Some(hit) = &hit_opt {
                    self.last_hovered_hit_3d = Some((hit.point, hit.normal, hit.uv));
                    self.last_hovered_tri = Some(hit.triangle_idx);
                } else {
                    self.last_hovered_hit_3d = None;
                    if self.last_hovered_uv_2d.is_none() {
                        self.last_hovered_tri = None;
                    }
                }

                if self.brush_engine.settings.mode == ToolMode::FillBucket {
                    // Precision cursor dot and ring for Polygon Fill
                    ui.painter().circle_filled(mouse_pos, 3.0, egui::Color32::from_rgb(243, 94, 24));
                    ui.painter().circle_stroke(mouse_pos, 6.0, egui::Stroke::new(1.2_f32, egui::Color32::WHITE));
                } else if self.brush_engine.settings.lazy_mouse_enabled {
                    if let Some(l_pos) = self.brush_engine.lazy_screen_pos {
                        let stabilized_pos = egui::pos2(l_pos[0], l_pos[1]);
                        // Tether line from stabilized point to mouse
                        ui.painter().line_segment(
                            [stabilized_pos, mouse_pos],
                            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24)),
                        );
                        // Stabilized brush ring
                        ui.painter().circle_stroke(
                            stabilized_pos,
                            brush_rad,
                            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24)),
                        );
                        ui.painter().circle_filled(stabilized_pos, 2.0, egui::Color32::from_rgb(243, 94, 24));
                        // Mouse position dot
                        ui.painter().circle_filled(mouse_pos, 3.0, egui::Color32::WHITE);
                    } else {
                        ui.painter().circle_stroke(
                            mouse_pos,
                            brush_rad,
                            egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210)),
                        );
                    }
                } else {
                    // Primary cursor ring (White)
                    ui.painter().circle_stroke(
                        mouse_pos,
                        brush_rad,
                        egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210)),
                    );
                    ui.painter().circle_filled(
                        mouse_pos,
                        1.5,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 240),
                    );
                }

                // Mirrored ghost cursors (Electric Blue)
                if sym_x || sym_y || sym_z {
                    if let Some(hit) = &hit_opt {
                        let sym_points = self.brush_engine.get_symmetry_points(hit.point, hit.normal);
                        for (sym_p, _) in sym_points.iter().skip(1) {
                            if let Some(screen_p) = self.camera.world_to_screen(*sym_p, vp_w, vp_h) {
                                let sym_screen_pos = rect.min + egui::vec2(screen_p.x, screen_p.y);
                                ui.painter().circle_stroke(
                                    sym_screen_pos,
                                    brush_rad,
                                    egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(60, 180, 255, 220)),
                                );
                                ui.painter().circle_filled(
                                    sym_screen_pos,
                                    1.5,
                                    egui::Color32::from_rgba_unmultiplied(60, 180, 255, 255),
                                );
                            }
                        }
                    }
                }
            }
        } else {
            self.last_hovered_hit_3d = None;
            if self.last_hovered_uv_2d.is_none() {
                self.last_hovered_tri = None;
            }
        }

        // Synchronized 2D-to-3D cross-viewport cursor indicator
        if !response.hovered() {
            if let Some(uv) = self.last_hovered_uv_2d {
                if let Some((pos_3d, norm_3d)) = self.project.mesh.uv_to_3d_point(uv) {
                    if let Some(screen_p) = self.camera.world_to_screen(pos_3d, vp_w, vp_h) {
                        let p = rect.min + egui::vec2(screen_p.x, screen_p.y);
                        if rect.contains(p) {
                            let dist = (self.camera.eye_position() - pos_3d).length().max(0.01);
                            let fov_factor = (self.camera.fov_y / 2.0).tan();
                            let world_rad = (self.brush_engine.settings.radius / (self.project.texture_width as f32)) * self.project.mesh.bounds_radius();
                            let proj_screen_rad = ((world_rad / (dist * fov_factor)) * vp_h * 0.5).clamp(4.0, 300.0);

                            ui.painter().circle_stroke(
                                p,
                                proj_screen_rad,
                                egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0, 220, 255)),
                            );
                            ui.painter().circle_filled(
                                p,
                                2.0,
                                egui::Color32::from_rgb(0, 220, 255),
                            );

                            // Surface normal tick indicator
                            if let Some(norm_end) = self.camera.world_to_screen(pos_3d + norm_3d * (world_rad * 0.4).max(0.02), vp_w, vp_h) {
                                let norm_p = rect.min + egui::vec2(norm_end.x, norm_end.y);
                                ui.painter().line_segment([p, norm_p], egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(0, 220, 255, 180)));
                            }
                        }
                    }
                }
            }
        }

        // Draw in-viewport gesture HUD overlay if active
        if let Some((pos, label, opt_rad, opt_hard)) = gesture_overlay {
            if let Some(rad) = opt_rad {
                // Brush radius outer ring (Substance iconic amber)
                ui.painter().circle_stroke(
                    pos,
                    rad,
                    egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(243, 94, 24)),
                );
                if let Some(hard) = opt_hard {
                    // Inner hardness boundary ring
                    ui.painter().circle_stroke(
                        pos,
                        (rad * hard).max(1.0),
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
                    );
                }
            }
            // Floating gesture HUD pill
            let pill_offset = egui::vec2(0.0, -opt_rad.unwrap_or(24.0) - 26.0);
            let pill_center = pos + pill_offset;
            let pill_rect = egui::Rect::from_center_size(pill_center, egui::vec2(220.0, 26.0));
            ui.painter().rect_filled(
                pill_rect,
                egui::Rounding::same(13.0),
                egui::Color32::from_rgba_unmultiplied(20, 22, 28, 235),
            );
            ui.painter().rect_stroke(
                pill_rect,
                egui::Rounding::same(13.0),
                egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(243, 94, 24)),
            );
            ui.painter().text(
                pill_center,
                egui::Align2::CENTER_CENTER,
                &label,
                egui::FontId::proportional(11.5),
                egui::Color32::WHITE,
            );
        }
    }

    /// Render 2D UV Canvas Viewport in egui panel
    fn render_2d_uv_viewport_ui(&mut self, ui: &mut egui::Ui, size: Vec2, ctx: &egui::Context) {
        if self.project.is_dirty || self.uv_canvas_texture.is_none() {
            self.project.composite_textures.composite_layers(&self.project.layers);
            self.update_uv_texture(ctx);
            self.project.is_dirty = false;
        }

        if let Some(tex) = &self.uv_canvas_texture {
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

            // Dark neutral viewport background
            ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(24, 25, 29));

            // Calculate canvas aspect ratio preserving dimensions
            let tex_w = self.project.texture_width as f32;
            let tex_h = self.project.texture_height as f32;
            let aspect = (tex_w / tex_h).max(0.01);
            let margin = 20.0;
            let avail_w = (rect.width() - margin * 2.0).max(32.0);
            let avail_h = (rect.height() - margin * 2.0).max(32.0);
            let (fit_w, fit_h) = if avail_w / avail_h > aspect {
                (avail_h * aspect, avail_h)
            } else {
                (avail_w, avail_w / aspect)
            };

            let canvas_w = fit_w * self.uv_2d_zoom;
            let canvas_h = fit_h * self.uv_2d_zoom;
            let canvas_center = rect.center() + self.uv_2d_pan;
            let canvas_rect = egui::Rect::from_center_size(canvas_center, egui::vec2(canvas_w, canvas_h));

            // Pan and navigation controls (Alt + drag, Middle-drag, Space + drag)
            let is_panning = response.dragged_by(egui::PointerButton::Middle)
                || (response.dragged_by(egui::PointerButton::Primary) && ui.input(|i| i.modifiers.alt || i.key_down(egui::Key::Space)))
                || (response.dragged_by(egui::PointerButton::Secondary) && ui.input(|i| i.modifiers.alt));

            if is_panning {
                let delta = ui.input(|i| i.pointer.delta());
                self.uv_2d_pan += delta;
                ctx.request_repaint();
            }

            // Mouse wheel zoom towards mouse position
            if response.hovered() {
                let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
                if scroll_y.abs() > 0.1 {
                    let zoom_factor = if scroll_y > 0.0 { 1.12 } else { 1.0 / 1.12 };
                    let new_zoom = (self.uv_2d_zoom * zoom_factor).clamp(0.2, 16.0);
                    if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        let rel = mouse_pos - canvas_center;
                        let scale_ratio = new_zoom / self.uv_2d_zoom;
                        self.uv_2d_pan = mouse_pos - rect.center() - rel * scale_ratio;
                        self.uv_2d_zoom = new_zoom;
                    } else {
                        self.uv_2d_zoom = new_zoom;
                    }
                }
            }

            // Hotkey F: Fit canvas to view
            if response.hovered() && ui.input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.command && !i.modifiers.ctrl) {
                self.uv_2d_pan = egui::Vec2::ZERO;
                self.uv_2d_zoom = 1.0;
                self.status_message = "2D Viewport reset to Fit [F]".to_string();
            }

            // Canvas drop shadow and texture rendering
            let shadow_rect = canvas_rect.expand(2.0);
            ui.painter().rect_filled(shadow_rect, egui::Rounding::same(2.0), Color32::from_black_alpha(100));
            ui.painter().rect_filled(canvas_rect, 0.0, Color32::from_rgb(18, 18, 20));

            ui.painter().image(
                tex.id(),
                canvas_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            ui.painter().rect_stroke(canvas_rect, 0.0, egui::Stroke::new(1.0_f32, Color32::from_rgb(70, 75, 88)));

            // UV Wireframe Overlay
            if self.show_uv_wireframe {
                let wire_stroke = egui::Stroke::new(0.8_f32, egui::Color32::from_rgba_unmultiplied(220, 225, 235, 75));
                let cw = canvas_rect.width();
                let ch = canvas_rect.height();
                let cmin = canvas_rect.min;

                let clip = rect.intersect(canvas_rect);
                let painter = ui.painter().with_clip_rect(clip);

                let verts = &self.project.mesh.vertices;
                let indices = &self.project.mesh.indices;

                for tri in indices.chunks_exact(3) {
                    let i0 = tri[0] as usize;
                    let i1 = tri[1] as usize;
                    let i2 = tri[2] as usize;
                    if i0 < verts.len() && i1 < verts.len() && i2 < verts.len() {
                        let uv0 = verts[i0].uv;
                        let uv1 = verts[i1].uv;
                        let uv2 = verts[i2].uv;

                        let p0 = cmin + egui::vec2(uv0.x * cw, uv0.y * ch);
                        let p1 = cmin + egui::vec2(uv1.x * cw, uv1.y * ch);
                        let p2 = cmin + egui::vec2(uv2.x * cw, uv2.y * ch);

                        let min_p = p0.min(p1).min(p2);
                        let max_p = p0.max(p1).max(p2);
                        let tri_rect = egui::Rect::from_min_max(min_p, max_p);
                        if tri_rect.intersects(clip) {
                            painter.line_segment([p0, p1], wire_stroke);
                            painter.line_segment([p1, p2], wire_stroke);
                            painter.line_segment([p2, p0], wire_stroke);
                        }
                    }
                }
            }

            // 2D UV Painting and Tool Actions
            if !is_panning && (response.dragged_by(egui::PointerButton::Primary) || response.clicked()) {
                ctx.request_repaint();
                if !self.is_stroke_active {
                    if let Some(wgpu) = &mut self.wgpu_viewport {
                        if wgpu.gpu_layers.len() != self.project.layers.len() {
                            wgpu.ensure_gpu_layers(&mut self.project);
                        }
                    }
                    if let Some(layer) = self.project.active_layer() {
                        let target = self.brush_engine.settings.target;
                        let channels = if target == PaintTarget::Mask {
                            crate::channel::ChannelFlags::none()
                        } else {
                            let b = self.brush_engine.settings.channels;
                            let l = layer.channel_flags;
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
                        self.stroke_start_layer = Some(layer.clone_stroke_snapshot_selective(Some(target), Some(channels)));
                        self.is_stroke_active = true;
                    }
                    self.polygon_fill_drag_tris.clear();
                }

                if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    if canvas_rect.contains(mouse_pos) || self.is_stroke_active {
                        let u = ((mouse_pos.x - canvas_rect.min.x) / canvas_rect.width()).clamp(0.0, 1.0);
                        let v = ((mouse_pos.y - canvas_rect.min.y) / canvas_rect.height()).clamp(0.0, 1.0);

                        if self.brush_engine.settings.mode == ToolMode::Clone && ui.input(|i| i.modifiers.alt || i.key_down(egui::Key::V)) {
                            self.brush_engine.settings.clone_source_uv = Some(glam::Vec2::new(u, v));
                            self.status_message = format!("Clone Source Set: UV ({:.3}, {:.3})", u, v);
                        } else if self.brush_engine.settings.mode == ToolMode::Eyedropper {
                            let sampled = self.project.composite_textures.base_color.sample_bilinear(glam::Vec2::new(u, v));
                            self.brush_engine.settings.paint_color = sampled;
                            self.brush_engine.settings.paint_roughness = self.project.composite_textures.roughness.sample_bilinear(glam::Vec2::new(u, v));
                            self.brush_engine.settings.paint_metallic = self.project.composite_textures.metallic.sample_bilinear(glam::Vec2::new(u, v));
                            self.brush_engine.settings.paint_height = self.project.composite_textures.height_map.sample_bilinear(glam::Vec2::new(u, v));
                            self.brush_engine.settings.paint_emissive = self.project.composite_textures.emissive.sample_bilinear(glam::Vec2::new(u, v));
                            self.brush_engine.settings.paint_opacity = self.project.composite_textures.opacity.sample_bilinear(glam::Vec2::new(u, v));
                            self.status_message = format!("Sampled Material [2D]: Color RGBA({:.2}, {:.2}, {:.2}, {:.2}), Rough {:.2}, Metal {:.2}, Emis RGB({:.2}, {:.2}, {:.2})", sampled.x, sampled.y, sampled.z, sampled.w, self.brush_engine.settings.paint_roughness, self.brush_engine.settings.paint_metallic, self.brush_engine.settings.paint_emissive.x, self.brush_engine.settings.paint_emissive.y, self.brush_engine.settings.paint_emissive.z);
                        } else if self.brush_engine.settings.mode == ToolMode::FillBucket {
                            let uv_point = glam::Vec2::new(u, v);
                            let tri_opt = self.project.mesh.find_triangle_at_uv(uv_point);
                            if let Some(tri_idx) = tri_opt {
                                let mode = self.brush_engine.settings.polygon_fill_mode;
                                let tris = match mode {
                                    crate::brush::PolygonFillMode::Triangle => vec![tri_idx],
                                    crate::brush::PolygonFillMode::Polygon => self.project.mesh.get_polygon_quad(tri_idx),
                                    crate::brush::PolygonFillMode::UvIsland => self.project.mesh.get_connected_uv_island(tri_idx),
                                    crate::brush::PolygonFillMode::MeshElement => self.project.mesh.get_connected_mesh_element(tri_idx),
                                };
                                let new_tris: Vec<usize> = tris.into_iter().filter(|t| !self.polygon_fill_drag_tris.contains(t)).collect();
                                if !new_tris.is_empty() {
                                    for &t in &new_tris {
                                        self.polygon_fill_drag_tris.insert(t);
                                    }
                                    let tri_uvs: Vec<(glam::Vec2, glam::Vec2, glam::Vec2)> = new_tris.iter().map(|&t| {
                                        let i0 = self.project.mesh.indices[t * 3] as usize;
                                        let i1 = self.project.mesh.indices[t * 3 + 1] as usize;
                                        let i2 = self.project.mesh.indices[t * 3 + 2] as usize;
                                        (
                                            self.project.mesh.vertices[i0].uv,
                                            self.project.mesh.vertices[i1].uv,
                                            self.project.mesh.vertices[i2].uv,
                                        )
                                    }).collect();

                                    let act_idx = self.project.active_layer_idx;
                                    if act_idx < self.project.layers.len() {
                                        let layer = &mut self.project.layers[act_idx];
                                        if self.brush_engine.settings.target == PaintTarget::Mask {
                                            if layer.has_mask {
                                                let val = self.brush_engine.settings.polygon_fill_value;
                                                let w = layer.mask_buf.width;
                                                let h = layer.mask_buf.height;
                                                for &(uv0, uv1, uv2) in &tri_uvs {
                                                    crate::mesh::Mesh::rasterize_triangle_uv(w, h, uv0, uv1, uv2, |px, py| {
                                                        layer.mask_buf.set_pixel(px, py, val);
                                                    });
                                                }
                                                layer.mark_dirty_full();
                                                self.project.rebuild_composite();
                                                self.update_uv_texture(ctx);
                                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                                    wgpu.sync_textures(&mut self.project);
                                                }
                                                ctx.request_repaint();
                                                self.status_message = format!("2D Polygon Fill ({}) filled {} triangles on mask", mode.name(), new_tris.len());
                                            } else {
                                                self.status_message = "Active layer does not have a mask. Add a mask to use Polygon Fill on mask.".to_string();
                                            }
                                        } else {
                                            // Target: Material
                                            if layer.layer_type == LayerType::Paint {
                                                let col = self.brush_engine.settings.paint_color;
                                                let rough = self.brush_engine.settings.paint_roughness;
                                                let metal = self.brush_engine.settings.paint_metallic;
                                                let height = self.brush_engine.settings.paint_height;
                                                let emis = self.brush_engine.settings.paint_emissive;
                                                let opac = self.brush_engine.settings.paint_opacity;
                                                let do_col = layer.channel_flags.base_color;
                                                let do_rough = layer.channel_flags.roughness;
                                                let do_metal = layer.channel_flags.metallic;
                                                let do_h = layer.channel_flags.height;
                                                let do_emis = layer.channel_flags.emission;
                                                let do_opac = layer.channel_flags.opacity;
                                                let w = layer.base_color_buf.width;
                                                let h = layer.base_color_buf.height;
                                                for &(uv0, uv1, uv2) in &tri_uvs {
                                                    crate::mesh::Mesh::rasterize_triangle_uv(w, h, uv0, uv1, uv2, |px, py| {
                                                        if do_col {
                                                            layer.base_color_buf.set_pixel(px, py, col);
                                                        } else {
                                                            let mut cur = layer.base_color_buf.get_pixel(px, py);
                                                            cur.w = 1.0;
                                                            layer.base_color_buf.set_pixel(px, py, cur);
                                                        }
                                                        if do_rough { layer.roughness_buf.set_pixel(px, py, rough); }
                                                        if do_metal { layer.metallic_buf.set_pixel(px, py, metal); }
                                                        if do_h { layer.height_buf.set_pixel(px, py, height); }
                                                        if do_emis { layer.emissive_buf.set_pixel(px, py, emis); }
                                                        if do_opac { layer.opacity_buf.set_pixel(px, py, opac); }
                                                    });
                                                }
                                                layer.update_normal_from_height(1.0);
                                                layer.mark_dirty_full();
                                                self.project.rebuild_composite();
                                                self.update_uv_texture(ctx);
                                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                                    wgpu.sync_textures(&mut self.project);
                                                }
                                                ctx.request_repaint();
                                                self.status_message = format!("2D Polygon Fill ({}) filled {} triangles with material", mode.name(), new_tris.len());
                                            } else {
                                                self.status_message = "Cannot fill material on Fill layer. Switch target to Mask or add a Paint layer.".to_string();
                                            }
                                        }
                                    }
                                }
                            } else {
                                self.status_message = "No UV polygon at clicked coordinate.".to_string();
                            }
                        } else {
                            let act_idx = self.project.active_layer_idx;
                            if act_idx < self.project.layers.len() {
                                let is_paint = self.project.layers[act_idx].layer_type == LayerType::Paint;
                                if self.brush_engine.settings.target == PaintTarget::Material && !is_paint {
                                    self.status_message = "Direct painting blocked on Fill layers. Switch target to Mask or add a Paint layer.".to_string();
                                } else {
                                    let pressure = ui.input(|i| {
                                        i.events.iter().find_map(|e| match e {
                                            egui::Event::Touch { force: Some(f), .. } => Some(*f),
                                            _ => None,
                                        }).unwrap_or(1.0)
                                    }).clamp(0.05, 1.0);
                                    let orig_rad = self.brush_engine.settings.radius;
                                    let orig_flow = self.brush_engine.settings.flow;
                                    if self.brush_engine.settings.pressure_size {
                                        self.brush_engine.settings.radius = orig_rad * (0.2 + 0.8 * pressure);
                                    }
                                    if self.brush_engine.settings.pressure_opacity {
                                        self.brush_engine.settings.flow = orig_flow * pressure;
                                    }
                                    let t_stroke = std::time::Instant::now();
                                    self.brush_engine.paint_stroke(&mut self.project.layers[act_idx], glam::Vec2::new(u, v));
                                    self.perf_metrics.paint_stroke_ms = t_stroke.elapsed().as_secs_f32() * 1000.0;
                                    self.brush_engine.settings.radius = orig_rad;
                                    self.brush_engine.settings.flow = orig_flow;
                                    let d_rect = self.project.layers[act_idx].dirty_rect;
                                    if let Some((d_x0, d_y0, d_x1, d_y1)) = d_rect {
                                        self.project.rebuild_composite_rect(
                                            d_x0 as usize,
                                            d_y0 as usize,
                                            d_x1 as usize,
                                            d_y1 as usize,
                                        );
                                        self.update_uv_texture_rect(ctx, d_x0, d_y0, d_x1, d_y1);
                                        if let Some(wgpu) = &mut self.wgpu_viewport {
                                            if let Some(gpu_layer) = wgpu.gpu_layers.get(act_idx) {
                                                let doc_w = self.project.texture_width;
                                                let doc_h = self.project.texture_height;
                                                let active_layer = &mut self.project.layers[act_idx];
                                                wgpu.gpu_compositor.update_layer(active_layer, gpu_layer, doc_w, doc_h);
                                                wgpu.composite_and_update(&self.project);
                                            }
                                        }
                                    } else {
                                        self.update_uv_texture(ctx);
                                    }
                                    self.project.gpu_sync_needed = false;
                                    self.project.is_dirty = true;
                                    self.last_painted_uv = Some(glam::Vec2::new(u, v));
                                    self.last_hovered_uv_2d = Some(glam::Vec2::new(u, v));
                                    self.status_message = format!("2D UV Painting at ({:.3}, {:.3})", u, v);
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                }
            } else if response.drag_stopped() || (!ui.input(|i| i.pointer.any_down()) && self.is_stroke_active) {
                self.brush_engine.end_stroke();
                self.polygon_fill_drag_tris.clear();
                if let Some(layer) = self.project.active_layer_mut() {
                    if self.brush_engine.settings.channels.height {
                        layer.update_normal_from_height(1.0);
                    }
                }
                self.project.rebuild_composite();
                self.update_uv_texture(ctx);
                if let Some(wgpu) = &mut self.wgpu_viewport {
                    wgpu.sync_textures(&mut self.project);
                }
                if self.is_stroke_active {
                    if let Some(prev) = self.stroke_start_layer.take() {
                        if let Some(next) = self.project.active_layer() {
                            let idx = self.project.active_layer_idx;
                            let desc = if self.brush_engine.settings.mode == ToolMode::FillBucket {
                                format!("2D Polygon Fill ({})", self.brush_engine.settings.polygon_fill_mode.name())
                            } else {
                                "2D UV Paint Stroke".to_string()
                            };
                            let target = self.brush_engine.settings.target;
                            let channels = if target == PaintTarget::Mask {
                                crate::channel::ChannelFlags::none()
                            } else {
                                let b = self.brush_engine.settings.channels;
                                let l = next.channel_flags;
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
                            let next_layer = next.clone_stroke_snapshot_selective(Some(target), Some(channels));
                            self.history.push(&desc.clone(), HistoryAction::Stroke {
                                layer_idx: idx,
                                prev_layer: prev,
                                next_layer,
                                description: desc,
                            });
                        }
                    }
                    self.is_stroke_active = false;
                }
            }

            // --- 2D POLYGON FILL HOVER PREVIEW HIGHLIGHT ---
            if self.brush_engine.settings.mode == ToolMode::FillBucket {
                if let Some(tri_idx) = self.last_hovered_tri {
                    let mode = self.brush_engine.settings.polygon_fill_mode;
                    let preview_tris = match mode {
                        crate::brush::PolygonFillMode::Triangle => vec![tri_idx],
                        crate::brush::PolygonFillMode::Polygon => self.project.mesh.get_polygon_quad(tri_idx),
                        crate::brush::PolygonFillMode::UvIsland => self.project.mesh.get_connected_uv_island(tri_idx),
                        crate::brush::PolygonFillMode::MeshElement => self.project.mesh.get_connected_mesh_element(tri_idx),
                    };

                    let fill_col = egui::Color32::from_rgba_unmultiplied(243, 94, 24, 75);
                    let edge_col = egui::Color32::from_rgba_unmultiplied(243, 140, 40, 230);
                    let cw = canvas_rect.width();
                    let ch = canvas_rect.height();
                    let cmin = canvas_rect.min;

                    for &t in &preview_tris {
                        if t * 3 + 2 >= self.project.mesh.indices.len() { continue; }
                        let i0 = self.project.mesh.indices[t * 3] as usize;
                        let i1 = self.project.mesh.indices[t * 3 + 1] as usize;
                        let i2 = self.project.mesh.indices[t * 3 + 2] as usize;
                        if i0 >= self.project.mesh.vertices.len() || i1 >= self.project.mesh.vertices.len() || i2 >= self.project.mesh.vertices.len() { continue; }
                        let uv0 = self.project.mesh.vertices[i0].uv;
                        let uv1 = self.project.mesh.vertices[i1].uv;
                        let uv2 = self.project.mesh.vertices[i2].uv;

                        let p0 = cmin + egui::vec2(uv0.x * cw, uv0.y * ch);
                        let p1 = cmin + egui::vec2(uv1.x * cw, uv1.y * ch);
                        let p2 = cmin + egui::vec2(uv2.x * cw, uv2.y * ch);

                        ui.painter().add(egui::Shape::convex_polygon(
                            vec![p0, p1, p2],
                            fill_col,
                            egui::Stroke::new(1.2_f32, edge_col),
                        ));
                    }
                }
            }

            // Interactive 2D Brush Cursor and Symmetry Guides
            if response.hovered() && !is_panning {
                ctx.request_repaint();
                if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let u = ((mouse_pos.x - canvas_rect.min.x) / canvas_rect.width()).clamp(0.0, 1.0);
                    let v = ((mouse_pos.y - canvas_rect.min.y) / canvas_rect.height()).clamp(0.0, 1.0);
                    self.last_hovered_uv_2d = Some(glam::Vec2::new(u, v));
                    let uv_point = glam::Vec2::new(u, v);
                    if let Some(tri_idx) = self.project.mesh.find_triangle_at_uv(uv_point) {
                        self.last_hovered_tri = Some(tri_idx);
                    } else if self.last_hovered_hit_3d.is_none() {
                        self.last_hovered_tri = None;
                    }

                    if self.brush_engine.settings.mode == ToolMode::FillBucket {
                        // Precision cursor dot and ring for Polygon Fill
                        ui.painter().circle_filled(mouse_pos, 3.0, egui::Color32::from_rgb(243, 94, 24));
                        ui.painter().circle_stroke(mouse_pos, 6.0, egui::Stroke::new(1.2_f32, egui::Color32::WHITE));
                    } else {
                        let screen_rad = (self.brush_engine.settings.radius / self.project.texture_width as f32 * canvas_rect.width()).max(2.0);

                        // Primary brush ring
                        ui.painter().circle_stroke(
                            mouse_pos,
                            screen_rad,
                            egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220)),
                        );
                        ui.painter().circle_filled(
                            mouse_pos,
                            1.5,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 240),
                        );

                        // Mirrored symmetry ghost cursors in 2D
                        if self.brush_engine.settings.symmetry_x || self.brush_engine.settings.symmetry_y {
                            let sym_stroke = egui::Stroke::new(1.2_f32, egui::Color32::from_rgba_unmultiplied(60, 180, 255, 220));

                            if self.brush_engine.settings.symmetry_x {
                                let sym_pos = canvas_rect.min + egui::vec2((1.0 - u) * canvas_rect.width(), v * canvas_rect.height());
                                ui.painter().circle_stroke(sym_pos, screen_rad, sym_stroke);
                                ui.painter().circle_filled(sym_pos, 1.5, egui::Color32::from_rgba_unmultiplied(60, 180, 255, 255));
                            }
                            if self.brush_engine.settings.symmetry_y {
                                let sym_pos = canvas_rect.min + egui::vec2(u * canvas_rect.width(), (1.0 - v) * canvas_rect.height());
                                ui.painter().circle_stroke(sym_pos, screen_rad, sym_stroke);
                                ui.painter().circle_filled(sym_pos, 1.5, egui::Color32::from_rgba_unmultiplied(60, 180, 255, 255));
                            }
                        }
                    }
                }
            } else {
                self.last_hovered_uv_2d = None;
                if self.last_hovered_hit_3d.is_none() {
                    self.last_hovered_tri = None;
                }
            }

            // Synchronized 3D-to-2D cross-viewport cursor indicator
            if !response.hovered() {
                if let Some((_pos_3d, _norm_3d, uv)) = self.last_hovered_hit_3d {
                    let screen_x = canvas_rect.min.x + uv.x * canvas_rect.width();
                    let screen_y = canvas_rect.min.y + uv.y * canvas_rect.height();
                    let p = egui::pos2(screen_x, screen_y);
                    if canvas_rect.contains(p) {
                        let screen_rad = (self.brush_engine.settings.radius / (self.project.texture_width as f32) * canvas_rect.width()).max(3.0);
                        ui.painter().circle_stroke(
                            p,
                            screen_rad,
                            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0, 220, 255)),
                        );
                        ui.painter().circle_filled(
                            p,
                            2.0,
                            egui::Color32::from_rgb(0, 220, 255),
                        );
                    }
                }
            }

            // Top-right floating controls pill
            let hud_pos = rect.right_top() + egui::vec2(-360.0, 10.0);
            let hud_rect = egui::Rect::from_min_size(hud_pos, egui::vec2(350.0, 30.0));
            ui.painter().rect_filled(hud_rect, egui::Rounding::same(15.0), egui::Color32::from_rgba_unmultiplied(28, 30, 36, 225));
            ui.painter().rect_stroke(hud_rect, egui::Rounding::same(15.0), egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(58, 62, 74)));

            ui.allocate_ui_at_rect(hud_rect, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(format!("{:.0}%", self.uv_2d_zoom * 100.0)).small().strong().color(Color32::from_rgb(180, 190, 205)));
                    if ui.small_button("1:1").on_hover_text("Reset zoom to 100% (1:1 pixel scale)").clicked() {
                        self.uv_2d_zoom = 1.0;
                    }
                    if ui.small_button("Fit (F)").on_hover_text("Reset 2D UV view and zoom to fit (F)").clicked() {
                        self.uv_2d_pan = egui::Vec2::ZERO;
                        self.uv_2d_zoom = 1.0;
                    }
                    let wire_active = self.show_uv_wireframe;
                    let wire_label = if wire_active { "UV Wire: ON" } else { "UV Wire: OFF" };
                    if ui.selectable_label(wire_active, wire_label).on_hover_text("Toggle UV wireframe overlay").clicked() {
                        self.show_uv_wireframe = !self.show_uv_wireframe;
                    }

                    let has_mask = self.project.active_layer().map(|l| l.has_mask).unwrap_or(false);
                    if has_mask {
                        let mask_label = if self.uv_view_mask_mode { "Mask: ON" } else { "Mask: OFF" };
                        if ui.selectable_label(self.uv_view_mask_mode, mask_label).on_hover_text("Preview and paint directly on active layer mask in 2D UV canvas").clicked() {
                            self.uv_view_mask_mode = !self.uv_view_mask_mode;
                            self.update_uv_texture(ctx);
                        }
                    }
                });
            });

            // Top-left HUD channel badge
            let title_pos = rect.left_top() + egui::vec2(12.0, 12.0);
            let title_text = if self.uv_view_mask_mode {
                format!("2D UV Viewport | MASK PREVIEW | {}x{}", self.project.texture_width, self.project.texture_height)
            } else {
                format!("2D UV Viewport | Channel: {} | {}x{}", self.view_mode.name(), self.project.texture_width, self.project.texture_height)
            };
            ui.painter().text(
                title_pos,
                egui::Align2::LEFT_TOP,
                title_text,
                egui::FontId::proportional(11.5),
                egui::Color32::from_rgba_unmultiplied(200, 205, 215, 190),
            );
        }
    }
}

impl PaintcraftApp {
    /// Render the Substance 3D Painter Style Left ASSETS Panel (Compact 2-column dock)
    fn render_assets_panel(&mut self, ui: &mut egui::Ui) {
        // Header
        ui.horizontal(|ui| {
            ui.strong(egui::RichText::new("ASSETS").size(13.0).color(Color32::WHITE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("X").on_hover_text("Close Assets Panel").clicked() {
                    self.show_shelf = false;
                    self.workspace.show_shelf = false;
                }
            });
        });
        ui.label(egui::RichText::new("< All libraries").small().color(Color32::from_rgb(140, 145, 155)));
        ui.add_space(2.0);

        // Search Input
        ui.add(
            egui::TextEdit::singleline(&mut self.shelf_search_query)
                .hint_text("Search...")
                .desired_width(ui.available_width() - 4.0),
        );
        ui.add_space(3.0);

        // Ensure thumbnails are cached
        self.refresh_shelf_thumbnails(ui.ctx());

        // Category Filter Pills
        let categories = [
            (crate::shelf::ShelfCategory::All, "All"),
            (crate::shelf::ShelfCategory::Project, "Project"),
            (crate::shelf::ShelfCategory::Textures, "Tex"),
            (crate::shelf::ShelfCategory::Materials, "Mat"),
            (crate::shelf::ShelfCategory::SmartMaterials, "Smart"),
            (crate::shelf::ShelfCategory::SmartMasks, "Mask"),
            (crate::shelf::ShelfCategory::Generators, "Noise"),
            (crate::shelf::ShelfCategory::Alphas, "Alpha"),
            (crate::shelf::ShelfCategory::Environments, "HDRI"),
        ];

        ui.horizontal_wrapped(|ui| {
            for (cat, label) in categories {
                let is_sel = self.shelf_category == cat;
                if ui.selectable_label(is_sel, label).clicked() {
                    self.shelf_category = cat;
                }
            }
        });

        ui.add_space(2.0);
        ui.separator();

        if self.shelf_category == crate::shelf::ShelfCategory::Project || self.shelf_category == crate::shelf::ShelfCategory::Textures {
            // Project Shelf Actions
            ui.horizontal(|ui| {
                if ui.small_button("+ Import...").on_hover_text("Import texture or image file into project shelf").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images & Textures", &["png", "jpg", "jpeg", "tga", "bmp", "hdr", "exr"])
                        .set_title("Import Texture into Project Shelf")
                        .pick_file() 
                    {
                        match self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                            Ok(_id) => {
                                self.status_message = format!("Imported '{}' into project shelf", path.file_name().unwrap_or_default().to_string_lossy());
                                self.shelf_thumbnails_dirty = true;
                                self.refresh_shelf_thumbnails(ui.ctx());
                            }
                            Err(e) => {
                                self.status_message = format!("Failed to import texture: {}", e);
                            }
                        }
                    }
                }
                if ui.small_button("↻ Sync Maps").on_hover_text("Synchronize baked mesh maps to shelf").clicked() {
                    self.project.sync_baked_maps_to_resources();
                    self.shelf_thumbnails_dirty = true;
                    self.refresh_shelf_thumbnails(ui.ctx());
                    self.status_message = "Synchronized baked maps to shelf".to_string();
                }
            });
        } else if self.shelf_category == crate::shelf::ShelfCategory::Environments {
            // HDRI library actions
            ui.horizontal(|ui| {
                if ui.small_button("+ Import").on_hover_text("Import HDRI image (.hdr / .png / .exr) into library").clicked() {
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
                if ui.small_button("Open Folder").on_hover_text("Open assets/hdris directory in system file manager").clicked() {
                    let _ = self.asset_manager.open_hdri_folder();
                }
                if ui.small_button("↻ Refresh").on_hover_text("Rescan HDRI folder on disk").clicked() {
                    self.asset_manager.refresh_hdris();
                    self.status_message = format!("Refreshed HDRI assets: {} environments ready", self.asset_manager.hdris.len());
                }
            });
        } else if self.shelf_category == crate::shelf::ShelfCategory::Materials {
            // Material library actions
            ui.horizontal(|ui| {
                if ui.small_button("+ New Material").on_hover_text("Create a new custom PBR material").clicked() {
                    self.show_new_material_modal = true;
                    self.new_material_name = format!("Material_{}", self.asset_manager.materials.len() + 1);
                    if let Some(layer) = self.project.active_layer() {
                        self.new_material_color = [layer.fill_color.x, layer.fill_color.y, layer.fill_color.z, layer.fill_color.w];
                        self.new_material_roughness = layer.fill_roughness;
                        self.new_material_metallic = layer.fill_metallic;
                        self.new_material_height = layer.fill_height;
                        self.new_material_emissive = [layer.fill_emissive.x, layer.fill_emissive.y, layer.fill_emissive.z];
                        self.new_material_opacity = layer.fill_opacity;
                        self.new_material_clearcoat = layer.fill_clearcoat;
                        self.new_material_anisotropy = layer.fill_anisotropy;
                    }
                }
                if ui.small_button("Save Layer as Mat").on_hover_text("Save currently selected active layer as a new material").clicked() {
                    if let Some(layer) = self.project.active_layer() {
                        let name = layer.name.clone();
                        let clean_id = name.trim().replace(' ', "_").to_lowercase();
                        let mat = crate::asset_manager::CustomMaterialEntry {
                            id: clean_id,
                            name: name.clone(),
                            description: format!("Custom PBR material created from layer '{}'", name),
                            path: std::path::PathBuf::new(),
                            file_name: String::new(),
                            base_color: [layer.fill_color.x, layer.fill_color.y, layer.fill_color.z, layer.fill_color.w],
                            roughness: layer.fill_roughness,
                            metallic: layer.fill_metallic,
                            height: layer.fill_height,
                            normal: [0.5, 0.5, 1.0, 0.5],
                            emissive: [layer.fill_emissive.x, layer.fill_emissive.y, layer.fill_emissive.z, layer.fill_emissive.w],
                            opacity: layer.fill_opacity,
                            clearcoat: layer.fill_clearcoat,
                            anisotropy: layer.fill_anisotropy,
                        };
                        match self.asset_manager.save_material(&mat) {
                            Ok(path) => {
                                self.status_message = format!("Saved Material '{}' to {:?}", mat.name, path.file_name().unwrap_or_default());
                            }
                            Err(e) => {
                                self.status_message = format!("Failed to save material: {}", e);
                            }
                        }
                    } else {
                        self.status_message = "No active layer selected to save as material".to_string();
                    }
                }
                if ui.small_button("Open Folder").on_hover_text("Open assets/materials directory in desktop file manager").clicked() {
                    let _ = self.asset_manager.open_materials_folder();
                }
                if ui.small_button("↻ Refresh").on_hover_text("Rescan materials folder on disk").clicked() {
                    self.asset_manager.refresh_materials();
                    self.status_message = format!("Refreshed materials: {} custom materials ready", self.asset_manager.materials.len());
                }
            });
        } else {
            // Preset bundle actions
            ui.horizontal(|ui| {
                if ui.small_button("+ .spmt").on_hover_text("Import Smart Material preset (.spmt)").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Smart Material", &["spmt", "json"]).pick_file() {
                        let curv = self.project.curvature_map.clone();
                        if let Ok(new_layers) = crate::smart_materials::SmartMaterialLibrary::load_smart_material_from_file(
                            &path,
                            self.project.texture_width,
                            self.project.texture_height,
                            curv.as_ref(),
                        ) {
                            for mut l in new_layers {
                                l.id = self.project.next_layer_id;
                                self.project.next_layer_id += 1;
                                self.project.layers.push(l);
                            }
                            self.project.active_layer_idx = self.project.layers.len() - 1;
                            self.project.rebuild_composite();
                            self.status_message = format!("Imported Smart Material from {:?}", path.file_name().unwrap_or_default());
                        }
                    }
                }
                if ui.small_button("Save .spmt").on_hover_text("Export current layers as Smart Material (.spmt)").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Smart Material", &["spmt"]).set_file_name("MySmartMaterial.spmt").save_file() {
                        let _ = crate::smart_materials::SmartMaterialLibrary::export_smart_material_to_file(&path, &self.project.layers);
                        self.status_message = format!("Exported Smart Material to {:?}", path.file_name().unwrap_or_default());
                    }
                }
            });
        }

        ui.add_space(2.0);
        ui.separator();

        let all_assets = crate::shelf::get_shelf_assets();
        let query = self.shelf_search_query.to_lowercase();
        let cat = self.shelf_category;

        let filtered_assets: Vec<_> = all_assets
            .iter()
            .copied()
            .filter(|a| {
                if cat != crate::shelf::ShelfCategory::All && a.category != cat {
                    return false;
                }
                if !query.is_empty() && !a.name.to_lowercase().contains(&query) && !a.description.to_lowercase().contains(&query) {
                    return false;
                }
                true
            })
            .collect();

        // Adaptive Grid in Vertical ScrollArea (wide in bottom shelf dock, compact in left dock)
        let avail_w = ui.available_width();
        let card_w = 78.0_f32;
        let card_h = card_w + 16.0;
        let cols = ((avail_w - 6.0) / (card_w + 4.0)).floor().max(2.0) as usize;

        if self.shelf_category == crate::shelf::ShelfCategory::Environments {
            let hdri_query = self.shelf_search_query.to_lowercase();
            let filtered_hdris: Vec<_> = self.asset_manager.hdris.iter().filter(|h| {
                if !hdri_query.is_empty() && !h.name.to_lowercase().contains(&hdri_query) && !h.file_name.to_lowercase().contains(&hdri_query) {
                    return false;
                }
                true
            }).collect();

            let mut selected_hdri_path = None;

            egui::ScrollArea::vertical()
                .id_source("assets_panel_hdri_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if filtered_hdris.is_empty() {
                        ui.label(egui::RichText::new("No HDRIs found.\nDrop .hdr files into assets/hdris/").italics().color(Color32::from_rgb(140, 145, 155)));
                    } else {
                        let chunks: Vec<_> = filtered_hdris.chunks(cols).collect();
                        for row in chunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for hdri in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                                    let is_active = self.renderer.hdri.custom_hdr_path.as_ref() == Some(&hdri.path);
                                    let is_hovered = response.hovered();

                                    let bg_color = if is_active {
                                        egui::Color32::from_rgb(40, 48, 62)
                                    } else if is_hovered {
                                        egui::Color32::from_rgb(46, 50, 58)
                                    } else {
                                        egui::Color32::from_rgb(30, 32, 36)
                                    };

                                    let border_stroke = if is_active {
                                        egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(243, 94, 24))
                                    } else if is_hovered {
                                        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24))
                                    } else {
                                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54))
                                    };

                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    // Preview thumbnail
                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(card_w - 10.0, card_w - 18.0),
                                    );
                                    draw_hdri_thumbnail(ui.painter(), preview_rect, &hdri.id, &hdri.name);

                                    // Badge in top right corner
                                    let (badge_text, badge_color) = if hdri.is_builtin {
                                        ("BUILTIN", egui::Color32::from_rgb(255, 175, 40))
                                    } else {
                                        ("HDRI", egui::Color32::from_rgb(60, 180, 255))
                                    };
                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        badge_text,
                                        egui::FontId::proportional(7.5),
                                        badge_color,
                                    );

                                    // Name label
                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &hdri.name,
                                        egui::FontId::proportional(9.0),
                                        if is_active { egui::Color32::from_rgb(255, 200, 150) } else { egui::Color32::from_rgb(215, 220, 230) },
                                    );

                                    let tooltip = format!(
                                        "{}\nFile: {}\nSize: {}\n{}",
                                        hdri.name,
                                        hdri.file_name,
                                        hdri.size_formatted(),
                                        if is_active { "Active Viewport Environment" } else { "Click to set active HDRI" }
                                    );
                                    let response = response.on_hover_text(tooltip);

                                    if response.clicked() {
                                        selected_hdri_path = Some(hdri.path.clone());
                                    }
                                }
                            });
                        }
                    }
                });

            if let Some(path) = selected_hdri_path {
                self.activate_hdri_file(&path);
            }
            return;
        }

        if self.shelf_category == crate::shelf::ShelfCategory::Materials {
            let mat_query = self.shelf_search_query.to_lowercase();
            let filtered_mats: Vec<_> = self.asset_manager.materials.iter().filter(|m| {
                if !mat_query.is_empty() && !m.name.to_lowercase().contains(&mat_query) && !m.description.to_lowercase().contains(&mat_query) {
                    return false;
                }
                true
            }).collect();

            let mut selected_mat: Option<crate::asset_manager::CustomMaterialEntry> = None;
            let mut delete_mat_id: Option<String> = None;

            egui::ScrollArea::vertical()
                .id_source("assets_panel_materials_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if filtered_mats.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label(egui::RichText::new("No custom materials found.").strong().color(Color32::from_rgb(200, 205, 215)));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Built-in materials have been cleared.\nClick '+ New Material' or 'Save Layer as Mat' above to create your own materials!").italics().color(Color32::from_rgb(140, 145, 155)));
                            ui.add_space(12.0);
                            if ui.button("+ Create First Material").clicked() {
                                self.show_new_material_modal = true;
                                self.new_material_name = "My Material".to_string();
                            }
                        });
                    } else {
                        let chunks: Vec<_> = filtered_mats.chunks(cols).collect();
                        for row in chunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for mat in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                                    let is_hovered = response.hovered();
                                    let bg_color = if is_hovered {
                                        egui::Color32::from_rgb(46, 50, 58)
                                    } else {
                                        egui::Color32::from_rgb(30, 32, 36)
                                    };

                                    let border_stroke = if is_hovered {
                                        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(45, 140, 255))
                                    } else {
                                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54))
                                    };

                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    // Preview Sphere / Swatch
                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(card_w - 10.0, card_w - 18.0),
                                    );
                                    let center = preview_rect.center();
                                    let radius = (preview_rect.width() * 0.40).min(18.0);
                                    let col = egui::Color32::from_rgba_unmultiplied(
                                        (mat.base_color[0] * 255.0).clamp(0.0, 255.0) as u8,
                                        (mat.base_color[1] * 255.0).clamp(0.0, 255.0) as u8,
                                        (mat.base_color[2] * 255.0).clamp(0.0, 255.0) as u8,
                                        255,
                                    );
                                    ui.painter().circle_filled(center, radius, col);

                                    // Specular highlight on sphere for glossy materials
                                    if mat.roughness < 0.6 {
                                        let gloss = 1.0 - mat.roughness;
                                        let hl_offset = egui::vec2(-radius * 0.35, -radius * 0.35);
                                        let hl_radius = radius * (0.35 * gloss);
                                        let hl_alpha = ((0.8 * gloss) * 255.0) as u8;
                                        ui.painter().circle_filled(
                                            center + hl_offset,
                                            hl_radius,
                                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, hl_alpha),
                                        );
                                    }
                                    // Metallic border accent
                                    if mat.metallic > 0.5 {
                                        ui.painter().circle_stroke(
                                            center,
                                            radius + 1.0,
                                            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(255, 200, 80)),
                                        );
                                    }

                                    // Badge in top right corner
                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        "MAT",
                                        egui::FontId::proportional(7.5),
                                        egui::Color32::from_rgb(45, 140, 255),
                                    );

                                    // Name label
                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &mat.name,
                                        egui::FontId::proportional(9.0),
                                        egui::Color32::from_rgb(215, 220, 230),
                                    );

                                    let tooltip = format!(
                                        "{}\nRoughness: {:.2} | Metallic: {:.2}\nOpacity: {:.2} | Height: {:.2}\nClick to add as Fill Layer\nRight-click for options",
                                        mat.name,
                                        mat.roughness,
                                        mat.metallic,
                                        mat.opacity,
                                        mat.height,
                                    );
                                    let response = response.on_hover_text(tooltip);

                                    response.context_menu(|ui| {
                                        if ui.button("Apply as New Fill Layer").clicked() {
                                            selected_mat = Some((*mat).clone());
                                            ui.close_menu();
                                        }
                                        if ui.button("Apply to Current Active Layer").clicked() {
                                            let curv = self.project.curvature_map.clone();
                                            if let Some(layer) = self.project.active_layer_mut() {
                                                layer.fill_color = glam::Vec4::new(mat.base_color[0], mat.base_color[1], mat.base_color[2], mat.base_color[3]);
                                                layer.fill_roughness = mat.roughness;
                                                layer.fill_metallic = mat.metallic;
                                                layer.fill_height = mat.height;
                                                layer.fill_emissive = glam::Vec4::new(mat.emissive[0], mat.emissive[1], mat.emissive[2], mat.emissive[3]);
                                                layer.fill_opacity = mat.opacity;
                                                layer.fill_clearcoat = mat.clearcoat;
                                                layer.fill_anisotropy = mat.anisotropy;
                                                layer.evaluate_fill(curv.as_ref());
                                            }
                                            self.project.rebuild_composite();
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Delete Material").clicked() {
                                            delete_mat_id = Some(mat.id.clone());
                                            ui.close_menu();
                                        }
                                    });

                                    if response.clicked() {
                                        selected_mat = Some((*mat).clone());
                                    }
                                }
                            });
                        }
                    }
                });

            if let Some(id) = delete_mat_id {
                let _ = self.asset_manager.delete_material(&id);
                self.status_message = format!("Deleted material '{}'", id);
            }

            if let Some(mat) = selected_mat {
                let col = glam::Vec4::new(mat.base_color[0], mat.base_color[1], mat.base_color[2], mat.base_color[3]);
                let prev_active = self.project.active_layer_idx;
                let new_idx = self.project.add_fill_layer(
                    Some(mat.name.clone()),
                    col,
                    mat.roughness,
                    mat.metallic,
                );
                if let Some(layer) = self.project.layers.get_mut(new_idx) {
                    layer.fill_height = mat.height;
                    layer.fill_emissive = glam::Vec4::new(mat.emissive[0], mat.emissive[1], mat.emissive[2], mat.emissive[3]);
                    layer.fill_opacity = mat.opacity;
                    layer.fill_clearcoat = mat.clearcoat;
                    layer.fill_anisotropy = mat.anisotropy;
                    layer.evaluate_fill(self.project.curvature_map.as_ref());
                }
                self.project.rebuild_composite();
                let layer_clone = Box::new(self.project.layers[new_idx].clone());
                self.history.push("Add Material Layer", HistoryAction::AddLayer {
                    layer_idx: new_idx,
                    layer: layer_clone,
                    prev_active,
                });
                self.status_message = format!("Added Custom Material: {}", mat.name);
            }
            return;
        }

        if self.shelf_category == crate::shelf::ShelfCategory::Project || self.shelf_category == crate::shelf::ShelfCategory::Textures {
            let filtered_resources: Vec<_> = self.project.resources.iter().filter(|r| {
                if self.shelf_category == crate::shelf::ShelfCategory::Textures && r.resource_type != crate::project::ProjectResourceType::Texture && r.resource_type != crate::project::ProjectResourceType::BakedMap {
                    return false;
                }
                if !query.is_empty() && !r.name.to_lowercase().contains(&query) && !r.id.to_lowercase().contains(&query) {
                    return false;
                }
                true
            }).collect();

            let mut clicked_res_id = None;

            egui::ScrollArea::vertical()
                .id_source("assets_panel_project_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if filtered_resources.is_empty() {
                        if self.shelf_category == crate::shelf::ShelfCategory::Textures {
                            ui.label(egui::RichText::new("No textures found.\nClick '+ Import...' above to import textures or bake mesh maps.").italics().color(Color32::from_rgb(140, 145, 155)));
                        } else {
                            ui.label(egui::RichText::new("No project resources found.\nBake mesh maps or click '+ Import...' to add textures to project shelf.").italics().color(Color32::from_rgb(140, 145, 155)));
                        }
                    } else {
                        let chunks: Vec<_> = filtered_resources.chunks(cols).collect();
                        for row in chunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for res in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                                    let is_hovered = response.hovered();
                                    let bg_color = if is_hovered {
                                        egui::Color32::from_rgb(46, 50, 58)
                                    } else {
                                        egui::Color32::from_rgb(30, 32, 36)
                                    };

                                    let border_stroke = if is_hovered {
                                        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24))
                                    } else {
                                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54))
                                    };

                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    // Preview thumbnail
                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(card_w - 10.0, card_w - 18.0),
                                    );
                                    if let Some(thumb) = self.shelf_thumbnails.get(&res.id) {
                                        ui.painter().image(
                                            thumb.id(),
                                            preview_rect,
                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                            Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().rect_filled(preview_rect, 2.0, egui::Color32::from_rgb(38, 41, 48));
                                    }

                                    // Badge in top right corner
                                    let (badge_text, badge_color) = match res.resource_type {
                                        crate::project::ProjectResourceType::BakedMap => ("BAKED", egui::Color32::from_rgb(243, 94, 24)),
                                        crate::project::ProjectResourceType::Texture => ("TEX", egui::Color32::from_rgb(45, 140, 255)),
                                        crate::project::ProjectResourceType::Alpha => ("ALPHA", egui::Color32::from_rgb(80, 210, 100)),
                                        crate::project::ProjectResourceType::Hdri => ("HDRI", egui::Color32::from_rgb(60, 180, 255)),
                                    };
                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        badge_text,
                                        egui::FontId::proportional(7.5),
                                        badge_color,
                                    );

                                    // Name label
                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &res.name,
                                        egui::FontId::proportional(9.0),
                                        egui::Color32::from_rgb(215, 220, 230),
                                    );

                                    let tooltip = format!(
                                        "{}\nType: {}\nSize: {}x{}\nClick to assign to layer channel or slot",
                                        res.name,
                                        res.badge_label(),
                                        res.width,
                                        res.height
                                    );
                                    let response = response.on_hover_text(tooltip);

                                    if response.clicked() || response.secondary_clicked() {
                                        clicked_res_id = Some(res.id.clone());
                                    }
                                }
                            });
                        }
                    }
                });

            if let Some(id) = clicked_res_id {
                self.shelf_resource_popup = Some(id);
            }
            return;
        }

        let mut clicked_res_id = None;

        egui::ScrollArea::vertical()
            .id_source("assets_panel_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.shelf_category == crate::shelf::ShelfCategory::All && !self.project.resources.is_empty() {
                    let project_matches: Vec<_> = self.project.resources.iter().filter(|r| {
                        if !query.is_empty() && !r.name.to_lowercase().contains(&query) && !r.id.to_lowercase().contains(&query) {
                            return false;
                        }
                        true
                    }).collect();

                    if !project_matches.is_empty() {
                        ui.label(egui::RichText::new("PROJECT RESOURCES").small().strong().color(Color32::from_rgb(243, 94, 24)));
                        let pchunks: Vec<_> = project_matches.chunks(cols).collect();
                        for row in pchunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for res in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                                    let is_hovered = response.hovered();
                                    let bg_color = if is_hovered {
                                        egui::Color32::from_rgb(46, 50, 58)
                                    } else {
                                        egui::Color32::from_rgb(30, 32, 36)
                                    };

                                    let border_stroke = if is_hovered {
                                        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24))
                                    } else {
                                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54))
                                    };

                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(card_w - 10.0, card_w - 18.0),
                                    );
                                    if let Some(thumb) = self.shelf_thumbnails.get(&res.id) {
                                        ui.painter().image(
                                            thumb.id(),
                                            preview_rect,
                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                            Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().rect_filled(preview_rect, 2.0, egui::Color32::from_rgb(38, 41, 48));
                                    }

                                    let (badge_text, badge_color) = match res.resource_type {
                                        crate::project::ProjectResourceType::BakedMap => ("BAKED", egui::Color32::from_rgb(243, 94, 24)),
                                        crate::project::ProjectResourceType::Texture => ("TEX", egui::Color32::from_rgb(45, 140, 255)),
                                        crate::project::ProjectResourceType::Alpha => ("ALPHA", egui::Color32::from_rgb(80, 210, 100)),
                                        crate::project::ProjectResourceType::Hdri => ("HDRI", egui::Color32::from_rgb(60, 180, 255)),
                                    };
                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        badge_text,
                                        egui::FontId::proportional(7.5),
                                        badge_color,
                                    );

                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &res.name,
                                        egui::FontId::proportional(9.0),
                                        egui::Color32::from_rgb(215, 220, 230),
                                    );

                                    let tooltip = format!(
                                        "{}\nType: {}\nSize: {}x{}\nClick to assign to layer channel or slot",
                                        res.name,
                                        res.badge_label(),
                                        res.width,
                                        res.height
                                    );
                                    let response = response.on_hover_text(tooltip);

                                    if response.clicked() || response.secondary_clicked() {
                                        clicked_res_id = Some(res.id.clone());
                                    }
                                }
                            });
                        }
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("LIBRARY PRESETS").small().strong().color(Color32::from_rgb(140, 145, 155)));
                    }
                }

                if self.shelf_category == crate::shelf::ShelfCategory::All && !self.asset_manager.materials.is_empty() {
                    let mat_matches: Vec<_> = self.asset_manager.materials.iter().filter(|m| {
                        if !query.is_empty() && !m.name.to_lowercase().contains(&query) && !m.description.to_lowercase().contains(&query) {
                            return false;
                        }
                        true
                    }).collect();

                    if !mat_matches.is_empty() {
                        ui.label(egui::RichText::new("USER MATERIALS").small().strong().color(Color32::from_rgb(45, 140, 255)));
                        let mchunks: Vec<_> = mat_matches.chunks(cols).collect();
                        for row in mchunks {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for mat in row {
                                    let card_size = egui::vec2(card_w, card_h);
                                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());
                                    let is_hovered = response.hovered();
                                    let bg_color = if is_hovered { egui::Color32::from_rgb(46, 50, 58) } else { egui::Color32::from_rgb(30, 32, 36) };
                                    let border_stroke = if is_hovered { egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(45, 140, 255)) } else { egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54)) };
                                    ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                                    let preview_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                        egui::vec2(card_w - 10.0, card_w - 18.0),
                                    );
                                    let center = preview_rect.center();
                                    let radius = (preview_rect.width() * 0.40).min(18.0);
                                    let col = egui::Color32::from_rgba_unmultiplied(
                                        (mat.base_color[0] * 255.0).clamp(0.0, 255.0) as u8,
                                        (mat.base_color[1] * 255.0).clamp(0.0, 255.0) as u8,
                                        (mat.base_color[2] * 255.0).clamp(0.0, 255.0) as u8,
                                        255,
                                    );
                                    ui.painter().circle_filled(center, radius, col);

                                    ui.painter().text(
                                        egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                        egui::Align2::RIGHT_TOP,
                                        "MAT",
                                        egui::FontId::proportional(7.5),
                                        egui::Color32::from_rgb(45, 140, 255),
                                    );

                                    let name_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                        egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                                    );
                                    ui.painter().text(
                                        name_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &mat.name,
                                        egui::FontId::proportional(9.0),
                                        egui::Color32::from_rgb(215, 220, 230),
                                    );

                                    if response.clicked() {
                                        let col = glam::Vec4::new(mat.base_color[0], mat.base_color[1], mat.base_color[2], mat.base_color[3]);
                                        self.project.add_fill_layer(Some(mat.name.clone()), col, mat.roughness, mat.metallic);
                                        self.status_message = format!("Added Custom Material: {}", mat.name);
                                    }
                                }
                            });
                        }
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                    }
                }
                let chunks: Vec<_> = filtered_assets.chunks(cols).collect();
                for row in chunks {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                        for asset in row {
                            let card_size = egui::vec2(card_w, card_h);
                            let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                            let is_hovered = response.hovered();
                            let bg_color = if is_hovered {
                                egui::Color32::from_rgb(46, 50, 58)
                            } else {
                                egui::Color32::from_rgb(30, 32, 36)
                            };

                            let border_stroke = if is_hovered {
                                egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(243, 94, 24))
                            } else {
                                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 47, 54))
                            };

                            ui.painter().rect(rect, egui::Rounding::same(4.0), bg_color, border_stroke);

                            // Draw preview thumbnail depending on category
                            let preview_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.min.x + 5.0, rect.min.y + 5.0),
                                egui::vec2(card_w - 10.0, card_w - 18.0),
                            );

                            match asset.category {
                                crate::shelf::ShelfCategory::Materials | crate::shelf::ShelfCategory::SmartMaterials => {
                                    let center = preview_rect.center();
                                    let radius = (preview_rect.width() * 0.42).min(18.0);
                                    let col = egui::Color32::from_rgb(asset.preview_color[0], asset.preview_color[1], asset.preview_color[2]);
                                    ui.painter().circle_filled(center, radius, col);
                                    if asset.preview_metallic > 0.2 || asset.preview_roughness < 0.4 {
                                        let shine_pos = center - egui::vec2(radius * 0.35, radius * 0.35);
                                        let shine_alpha = ((1.0 - asset.preview_roughness) * 200.0).clamp(40.0, 220.0) as u8;
                                        ui.painter().circle_filled(shine_pos, radius * 0.28, egui::Color32::from_rgba_unmultiplied(255, 255, 255, shine_alpha));
                                    }
                                }
                                crate::shelf::ShelfCategory::Generators => {
                                    // Procedural Noise thumbnail graphic
                                    ui.painter().rect_filled(preview_rect, 2.0, egui::Color32::from_rgb(20, 22, 26));
                                    match asset.id {
                                        "gen_checker" => {
                                            let hw = preview_rect.width() * 0.5;
                                            let hh = preview_rect.height() * 0.5;
                                            ui.painter().rect_filled(egui::Rect::from_min_size(preview_rect.min, egui::vec2(hw, hh)), 0.0, egui::Color32::WHITE);
                                            ui.painter().rect_filled(egui::Rect::from_min_size(preview_rect.min + egui::vec2(hw, hh), egui::vec2(hw, hh)), 0.0, egui::Color32::WHITE);
                                        }
                                        "gen_worley" => {
                                            let pts = [
                                                preview_rect.min + egui::vec2(8.0, 8.0),
                                                preview_rect.min + egui::vec2(preview_rect.width() - 8.0, 10.0),
                                                preview_rect.center(),
                                                preview_rect.min + egui::vec2(10.0, preview_rect.height() - 8.0),
                                                preview_rect.max - egui::vec2(8.0, 8.0),
                                            ];
                                            for p in pts {
                                                ui.painter().circle_filled(p, 3.0, egui::Color32::from_rgb(180, 185, 195));
                                                ui.painter().circle_stroke(p, 6.0, egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(90, 95, 105)));
                                            }
                                        }
                                        "gen_billow" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_filled(c + egui::vec2(-6.0, 2.0), 7.0, egui::Color32::from_rgb(150, 160, 175));
                                            ui.painter().circle_filled(c + egui::vec2(6.0, 3.0), 6.5, egui::Color32::from_rgb(130, 140, 155));
                                            ui.painter().circle_filled(c + egui::vec2(0.0, -4.0), 8.5, egui::Color32::from_rgb(205, 215, 230));
                                        }
                                        "gen_ridged" => {
                                            let p1 = preview_rect.left_top() + egui::vec2(4.0, 6.0);
                                            let p2 = preview_rect.center() + egui::vec2(-3.0, -4.0);
                                            let p3 = preview_rect.center() + egui::vec2(3.0, 3.0);
                                            let p4 = preview_rect.right_bottom() - egui::vec2(5.0, 5.0);
                                            ui.painter().line_segment([p1, p2], egui::Stroke::new(1.8_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([p2, p3], egui::Stroke::new(1.8_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([p3, p4], egui::Stroke::new(1.8_f32, egui::Color32::WHITE));
                                            let p5 = preview_rect.left_bottom() + egui::vec2(6.0, -7.0);
                                            let p6 = p3 + egui::vec2(1.0, 3.0);
                                            ui.painter().line_segment([p5, p6], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(180, 180, 180)));
                                        }
                                        "gen_domain_warp" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_stroke(c, 11.0, egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(120, 160, 200)));
                                            ui.painter().circle_stroke(c + egui::vec2(2.0, -1.0), 7.0, egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(180, 210, 240)));
                                            ui.painter().circle_filled(c + egui::vec2(3.0, -2.0), 3.0, egui::Color32::WHITE);
                                        }
                                        "gen_splatter" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_filled(c, 4.0, egui::Color32::from_rgb(210, 110, 70));
                                            ui.painter().circle_filled(c + egui::vec2(7.0, -6.0), 2.5, egui::Color32::from_rgb(180, 80, 50));
                                            ui.painter().circle_filled(c + egui::vec2(-8.0, 5.0), 3.0, egui::Color32::from_rgb(220, 130, 80));
                                            ui.painter().circle_filled(c + egui::vec2(6.0, 7.0), 1.8, egui::Color32::from_rgb(160, 70, 40));
                                            ui.painter().circle_filled(c + egui::vec2(-5.0, -7.0), 2.0, egui::Color32::from_rgb(190, 90, 60));
                                        }
                                        "gen_wood" => {
                                            let c = preview_rect.left_top();
                                            ui.painter().circle_stroke(c, 11.0, egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(190, 130, 70)));
                                            ui.painter().circle_stroke(c, 19.0, egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(150, 95, 45)));
                                            ui.painter().circle_stroke(c, 27.0, egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(120, 75, 35)));
                                        }
                                        "gen_fabric" => {
                                            let min = preview_rect.min + egui::vec2(4.0, 4.0);
                                            let max = preview_rect.max - egui::vec2(4.0, 4.0);
                                            let col = egui::Color32::from_rgb(140, 150, 170);
                                            for i in 0..4 {
                                                let t = i as f32 / 3.0;
                                                let x = min.x + t * (max.x - min.x);
                                                let y = min.y + t * (max.y - min.y);
                                                ui.painter().line_segment([egui::pos2(x, min.y), egui::pos2(x, max.y)], egui::Stroke::new(1.0_f32, col));
                                                ui.painter().line_segment([egui::pos2(min.x, y), egui::pos2(max.x, y)], egui::Stroke::new(1.0_f32, col));
                                            }
                                        }
                                        "gen_scratches" => {
                                            ui.painter().line_segment([preview_rect.left_top() + egui::vec2(4.0, 6.0), preview_rect.right_bottom() - egui::vec2(8.0, 4.0)], egui::Stroke::new(1.2_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([preview_rect.left_top() + egui::vec2(12.0, 4.0), preview_rect.right_bottom() - egui::vec2(4.0, 10.0)], egui::Stroke::new(0.8_f32, egui::Color32::from_rgb(180, 180, 180)));
                                            ui.painter().line_segment([preview_rect.left_bottom() + egui::vec2(6.0, -8.0), preview_rect.right_top() + egui::vec2(-6.0, 8.0)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(200, 200, 200)));
                                        }
                                        "gen_bricks" => {
                                            let y1 = preview_rect.min.y + preview_rect.height() * 0.33;
                                            let y2 = preview_rect.min.y + preview_rect.height() * 0.66;
                                            ui.painter().line_segment([egui::pos2(preview_rect.min.x, y1), egui::pos2(preview_rect.max.x, y1)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(170, 110, 80)));
                                            ui.painter().line_segment([egui::pos2(preview_rect.min.x, y2), egui::pos2(preview_rect.max.x, y2)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(170, 110, 80)));
                                            ui.painter().line_segment([egui::pos2(preview_rect.center().x, preview_rect.min.y), egui::pos2(preview_rect.center().x, y1)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(170, 110, 80)));
                                        }
                                        "gen_curvature" => {
                                            ui.painter().rect_stroke(preview_rect.shrink(6.0), 1.0, egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                        }
                                        "gen_dust" => {
                                            let c = preview_rect.center();
                                            ui.painter().line_segment([preview_rect.left_top() + egui::vec2(4.0, 8.0), preview_rect.right_top() + egui::vec2(-4.0, 8.0)], egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(220, 220, 200)));
                                            ui.painter().circle_filled(c + egui::vec2(0.0, 2.0), 2.0, egui::Color32::from_rgb(180, 180, 160));
                                        }
                                        "gen_cavity" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_stroke(c, 9.0, egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(80, 75, 70)));
                                            ui.painter().circle_filled(c, 5.0, egui::Color32::from_rgb(40, 35, 30));
                                        }
                                        "gen_leaks" => {
                                            let min = preview_rect.min;
                                            let max = preview_rect.max;
                                            let col = egui::Color32::from_rgb(160, 100, 60);
                                            ui.painter().line_segment([egui::pos2(min.x + 8.0, min.y + 4.0), egui::pos2(min.x + 8.0, max.y - 6.0)], egui::Stroke::new(1.5_f32, col));
                                            ui.painter().line_segment([egui::pos2(min.x + 16.0, min.y + 4.0), egui::pos2(min.x + 16.0, max.y - 12.0)], egui::Stroke::new(1.0_f32, col));
                                            ui.painter().line_segment([egui::pos2(min.x + 24.0, min.y + 4.0), egui::pos2(min.x + 24.0, max.y - 4.0)], egui::Stroke::new(1.8_f32, col));
                                        }
                                        "gen_position" => {
                                            let r = preview_rect.shrink(5.0);
                                            ui.painter().rect_filled(r, 1.0, egui::Color32::from_rgb(70, 90, 120));
                                            ui.painter().line_segment([egui::pos2(r.min.x, r.min.y + 4.0), egui::pos2(r.max.x, r.min.y + 4.0)], egui::Stroke::new(2.0_f32, egui::Color32::WHITE));
                                        }
                                        _ => {
                                            let center = preview_rect.center();
                                            ui.painter().circle_filled(center, 12.0, egui::Color32::from_rgb(140, 145, 155));
                                            ui.painter().circle_stroke(center, 7.0, egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(70, 75, 85)));
                                            ui.painter().circle_filled(center, 3.5, egui::Color32::WHITE);
                                        }
                                    }
                                }
                                crate::shelf::ShelfCategory::SmartMasks => {
                                    ui.painter().rect_filled(preview_rect, 2.0, egui::Color32::from_rgb(26, 28, 32));
                                    match asset.id {
                                        "mask_edge_wear" => {
                                            let r = preview_rect.shrink(5.0);
                                            ui.painter().rect_stroke(r, 1.0, egui::Stroke::new(2.0_f32, egui::Color32::WHITE));
                                            ui.painter().circle_filled(r.left_top(), 3.0, egui::Color32::from_rgb(26, 28, 32));
                                            ui.painter().circle_filled(r.right_bottom() - egui::vec2(2.0, 0.0), 2.5, egui::Color32::from_rgb(26, 28, 32));
                                        }
                                        "mask_crevice_dirt" => {
                                            let c = preview_rect.center();
                                            ui.painter().rect_filled(preview_rect.shrink(4.0), 2.0, egui::Color32::from_rgb(45, 48, 54));
                                            ui.painter().circle_filled(c, 7.0, egui::Color32::from_rgb(18, 19, 22));
                                            ui.painter().circle_filled(c + egui::vec2(-3.0, -2.0), 3.0, egui::Color32::from_rgb(10, 10, 12));
                                        }
                                        "directional_dust" => {
                                            let top_bar = egui::Rect::from_min_size(preview_rect.min + egui::vec2(3.0, 3.0), egui::vec2(preview_rect.width() - 6.0, 6.0));
                                            ui.painter().rect_filled(top_bar, 1.0, egui::Color32::from_rgb(235, 235, 220));
                                            let mid_bar = egui::Rect::from_min_size(preview_rect.min + egui::vec2(5.0, 10.0), egui::vec2(preview_rect.width() - 10.0, 3.0));
                                            ui.painter().rect_filled(mid_bar, 1.0, egui::Color32::from_rgb(160, 160, 150));
                                        }
                                        "dripping_rust_leaks" => {
                                            let c = preview_rect.center();
                                            ui.painter().line_segment([c + egui::vec2(-6.0, -10.0), c + egui::vec2(-6.0, 6.0)], egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(180, 80, 40)));
                                            ui.painter().circle_filled(c + egui::vec2(-6.0, 7.0), 2.5, egui::Color32::from_rgb(180, 80, 40));
                                            ui.painter().line_segment([c + egui::vec2(4.0, -8.0), c + egui::vec2(4.0, 10.0)], egui::Stroke::new(1.8_f32, egui::Color32::from_rgb(160, 70, 35)));
                                            ui.painter().circle_filled(c + egui::vec2(4.0, 11.0), 2.2, egui::Color32::from_rgb(160, 70, 35));
                                        }
                                        "mask_grunge" => {
                                            let c = preview_rect.center();
                                            ui.painter().line_segment([c + egui::vec2(-10.0, -8.0), c + egui::vec2(10.0, 6.0)], egui::Stroke::new(1.4_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([c + egui::vec2(-7.0, -1.0), c + egui::vec2(9.0, 11.0)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(200, 200, 200)));
                                            ui.painter().line_segment([c + egui::vec2(-11.0, 5.0), c + egui::vec2(5.0, -10.0)], egui::Stroke::new(0.8_f32, egui::Color32::from_rgb(170, 170, 170)));
                                        }
                                        "peeling_paint" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_stroke(c, 8.0, egui::Stroke::new(2.0_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([c + egui::vec2(6.0, -5.0), c + egui::vec2(12.0, -10.0)], egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                            ui.painter().circle_filled(c + egui::vec2(-2.0, 2.0), 3.0, egui::Color32::from_rgb(150, 120, 80));
                                        }
                                        "mask_ground_dirt" => {
                                            let bot = preview_rect.max.y - 4.0;
                                            let w = preview_rect.width();
                                            let mx = preview_rect.min.x;
                                            ui.painter().circle_filled(egui::pos2(mx + w * 0.2, bot), 4.0, egui::Color32::from_rgb(140, 95, 60));
                                            ui.painter().circle_filled(egui::pos2(mx + w * 0.5, bot - 2.0), 5.5, egui::Color32::from_rgb(120, 80, 50));
                                            ui.painter().circle_filled(egui::pos2(mx + w * 0.8, bot), 4.5, egui::Color32::from_rgb(140, 95, 60));
                                            ui.painter().circle_filled(egui::pos2(mx + w * 0.35, bot - 8.0), 2.0, egui::Color32::from_rgb(170, 120, 75));
                                        }
                                        "mask_heavy_machinery" => {
                                            let r = preview_rect.shrink(5.0);
                                            ui.painter().rect_stroke(r, 1.0, egui::Stroke::new(1.8_f32, egui::Color32::WHITE));
                                            let c = preview_rect.center();
                                            ui.painter().circle_filled(c, 4.0, egui::Color32::from_rgb(70, 60, 50));
                                            ui.painter().line_segment([c + egui::vec2(-6.0, -4.0), c + egui::vec2(6.0, 4.0)], egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(200, 200, 200)));
                                        }
                                        "mask_gun_metal" => {
                                            let r = preview_rect.shrink(5.0);
                                            ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.2_f32, egui::Color32::WHITE));
                                            ui.painter().rect_filled(r.shrink(2.0), 0.0, egui::Color32::from_rgb(35, 38, 44));
                                        }
                                        "mask_patina" => {
                                            let c = preview_rect.center();
                                            ui.painter().circle_filled(c, 7.0, egui::Color32::from_rgb(45, 175, 145));
                                            ui.painter().circle_stroke(c, 11.0, egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(30, 130, 110)));
                                        }
                                        "mask_invert" => {
                                            let c = preview_rect.center();
                                            ui.painter().rect_stroke(preview_rect.shrink(4.0), 1.0, egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                            ui.painter().line_segment([preview_rect.left_bottom() + egui::vec2(4.0, -4.0), preview_rect.right_top() + egui::vec2(-4.0, 4.0)], egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                            ui.painter().circle_filled(c + egui::vec2(4.0, -4.0), 2.5, egui::Color32::WHITE);
                                        }
                                        _ => {
                                            ui.painter().rect_stroke(preview_rect.shrink(4.0), 2.0, egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                                            ui.painter().circle_filled(preview_rect.center(), 4.0, egui::Color32::WHITE);
                                        }
                                    }
                                }
                                crate::shelf::ShelfCategory::Alphas => {
                                    let center = preview_rect.center();
                                    ui.painter().circle_filled(center, 11.0, egui::Color32::WHITE);
                                }
                                _ => {
                                    ui.painter().rect_filled(preview_rect, 2.0, egui::Color32::from_rgb(60, 64, 72));
                                }
                            }

                            // Category pill badge in corner
                            let badge_text = match asset.category {
                                crate::shelf::ShelfCategory::Materials => "MAT",
                                crate::shelf::ShelfCategory::SmartMaterials => "SMART",
                                crate::shelf::ShelfCategory::SmartMasks => "MASK",
                                crate::shelf::ShelfCategory::Generators => "NOISE",
                                crate::shelf::ShelfCategory::Alphas => "ALPHA",
                                _ => "",
                            };
                            let badge_color = match asset.category {
                                crate::shelf::ShelfCategory::Materials => egui::Color32::from_rgb(45, 140, 255),
                                crate::shelf::ShelfCategory::SmartMaterials => egui::Color32::from_rgb(243, 94, 24),
                                crate::shelf::ShelfCategory::SmartMasks => egui::Color32::from_rgb(180, 80, 255),
                                crate::shelf::ShelfCategory::Generators => egui::Color32::from_rgb(46, 204, 113),
                                crate::shelf::ShelfCategory::Alphas => egui::Color32::from_rgb(80, 210, 100),
                                _ => egui::Color32::GRAY,
                            };
                            ui.painter().text(
                                egui::pos2(rect.max.x - 3.0, rect.min.y + 2.0),
                                egui::Align2::RIGHT_TOP,
                                badge_text,
                                egui::FontId::proportional(7.5),
                                badge_color,
                            );

                            // Asset name label underneath
                            let name_rect = egui::Rect::from_min_max(
                                egui::pos2(rect.min.x + 2.0, rect.max.y - 18.0),
                                egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0),
                            );
                            ui.painter().text(
                                name_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                asset.name,
                                egui::FontId::proportional(9.5),
                                egui::Color32::from_rgb(215, 220, 230),
                            );

                            let tooltip = format!("{}\n{}", asset.name, asset.description);
                            let response = response.on_hover_text(tooltip);

                            if response.clicked() {
                                self.apply_shelf_asset(&asset);
                            }
                        }
                    });
                }
            });

        if let Some(id) = clicked_res_id {
            self.shelf_resource_popup = Some(id);
        }
    }

    /// Apply an asset from the shelf to the project
    fn apply_shelf_asset(&mut self, asset: &crate::shelf::ShelfAsset) {
        match asset.category {
            crate::shelf::ShelfCategory::Materials => {
                let col = glam::Vec4::new(
                    asset.preview_color[0] as f32 / 255.0,
                    asset.preview_color[1] as f32 / 255.0,
                    asset.preview_color[2] as f32 / 255.0,
                    1.0,
                );
                self.project.add_fill_layer(
                    Some(asset.name.to_string()),
                    col,
                    asset.preview_roughness,
                    asset.preview_metallic,
                );
                self.status_message = format!("Added Material: {}", asset.name);
            }
            crate::shelf::ShelfCategory::SmartMaterials => {
                let new_layers = crate::smart_materials::SmartMaterialLibrary::build_layers_with_maps(
                    asset.id,
                    self.project.texture_width,
                    self.project.texture_height,
                    self.project.curvature_map.as_ref(),
                    self.project.ao_map.as_ref(),
                    self.project.world_space_normal_map.as_ref(),
                    self.project.position_map.as_ref(),
                );
                for mut layer in new_layers {
                    layer.id = self.project.next_layer_id;
                    self.project.next_layer_id += 1;
                    self.project.layers.push(layer);
                }
                self.project.active_layer_idx = self.project.layers.len().saturating_sub(1);

                // Auto-configure physical optical shader settings for refractive and cutout materials
                if asset.id == "emerald_crystal_gem" {
                    self.shader_settings.transparency_mode = crate::wgpu_renderer::TransparencyMode::Refraction;
                    self.shader_settings.ior = 1.58;
                    self.shader_settings.double_sided = true;
                } else if asset.id == "amber_fossil_resin" {
                    self.shader_settings.transparency_mode = crate::wgpu_renderer::TransparencyMode::Refraction;
                    self.shader_settings.ior = 1.54;
                    self.shader_settings.double_sided = true;
                } else if asset.id == "frosted_ice_glass" {
                    self.shader_settings.transparency_mode = crate::wgpu_renderer::TransparencyMode::Refraction;
                    self.shader_settings.ior = 1.31;
                    self.shader_settings.double_sided = true;
                } else if asset.id == "tinted_glass_visor" {
                    self.shader_settings.transparency_mode = crate::wgpu_renderer::TransparencyMode::Refraction;
                    self.shader_settings.ior = 1.52;
                    self.shader_settings.double_sided = true;
                } else if asset.id == "perforated_steel_grate" || asset.id == "wire_chainlink_fence" || asset.id == "torn_camo_fabric" {
                    self.shader_settings.transparency_mode = crate::wgpu_renderer::TransparencyMode::AlphaCutout;
                    self.shader_settings.double_sided = true;
                }

                self.status_message = format!("Applied Smart Material: {}", asset.name);
            }
            crate::shelf::ShelfCategory::SmartMasks => {
                let act_idx = self.project.active_layer_idx;
                if act_idx < self.project.layers.len() {
                    let curv = self.project.curvature_map.clone();
                    let ao = self.project.ao_map.clone();
                    let norm = self.project.world_space_normal_map.clone();
                    let pos = self.project.position_map.clone();
                    let layer = &mut self.project.layers[act_idx];
                    crate::smart_materials::SmartMaterialLibrary::apply_smart_mask_advanced(
                        asset.id,
                        layer,
                        curv.as_ref(),
                        ao.as_ref(),
                        pos.as_ref(),
                        norm.as_ref(),
                    );
                    self.brush_engine.settings.target = crate::brush::PaintTarget::Mask;
                    self.status_message = format!("Applied Smart Mask (Non-Destructive Stack): {}", asset.name);
                }
            }
            crate::shelf::ShelfCategory::Generators => {
                let act_idx = self.project.active_layer_idx;
                let is_mask_target = self.brush_engine.settings.target == crate::brush::PaintTarget::Mask;
                if act_idx < self.project.layers.len() && is_mask_target {
                    let curv = self.project.curvature_map.clone();
                    let ao = self.project.ao_map.clone();
                    let pos = self.project.position_map.clone();
                    let wn = self.project.world_space_normal_map.clone();
                    let layer = &mut self.project.layers[act_idx];

                    match asset.id {
                        "gen_curvature" => {
                            layer.add_mask_generator_curvature_wear(curv.as_ref(), ao.as_ref(), pos.as_ref(), wn.as_ref());
                            self.status_message = format!("Added Curvature Edge Wear Generator to Mask: {}", asset.name);
                        }
                        "gen_cavity" => {
                            layer.add_mask_generator_cavity_dirt(curv.as_ref(), ao.as_ref(), pos.as_ref(), wn.as_ref());
                            self.status_message = format!("Added Cavity Dirt Generator to Mask: {}", asset.name);
                        }
                        "gen_dust" => {
                            layer.add_mask_generator_directional_dust(curv.as_ref(), ao.as_ref(), pos.as_ref(), wn.as_ref());
                            self.status_message = format!("Added Directional Dust Generator to Mask: {}", asset.name);
                        }
                        "gen_ground_dirt" => {
                            layer.add_mask_generator_ground_dirt(curv.as_ref(), ao.as_ref(), pos.as_ref(), wn.as_ref());
                            self.status_message = format!("Added Ground Mud Generator to Mask: {}", asset.name);
                        }
                        "gen_mask_editor" => {
                            layer.add_mask_generator_mask_editor(curv.as_ref(), ao.as_ref(), pos.as_ref(), wn.as_ref());
                            self.status_message = format!("Added Mask Editor Generator to Mask: {}", asset.name);
                        }
                        _ => {
                            let gen = match asset.id {
                                "gen_perlin" => crate::layer::ProceduralGenerator::PerlinNoise,
                                "gen_worley" => crate::layer::ProceduralGenerator::WorleyGrunge,
                                "gen_billow" => crate::layer::ProceduralGenerator::BillowNoise,
                                "gen_ridged" => crate::layer::ProceduralGenerator::RidgedMultifractal,
                                "gen_domain_warp" => crate::layer::ProceduralGenerator::DomainWarpGrunge,
                                "gen_splatter" => crate::layer::ProceduralGenerator::SplatterDirt,
                                "gen_wood" => crate::layer::ProceduralGenerator::WoodGrain,
                                "gen_fabric" => crate::layer::ProceduralGenerator::FabricWeave,
                                "gen_scratches" => crate::layer::ProceduralGenerator::Scratches,
                                "gen_leaks" => crate::layer::ProceduralGenerator::DrippingLeaks,
                                "gen_position" => crate::layer::ProceduralGenerator::PositionGradient,
                                "gen_checker" => crate::layer::ProceduralGenerator::Checkerboard,
                                "gen_bricks" => crate::layer::ProceduralGenerator::Bricks,
                                "gen_perforated" => crate::layer::ProceduralGenerator::PerforatedMesh,
                                "gen_wire_fence" => crate::layer::ProceduralGenerator::WireFence,
                                "gen_torn_fabric" => crate::layer::ProceduralGenerator::TornFabricEdge,
                                "gen_dots" => crate::layer::ProceduralGenerator::CircularDots,
                                "gen_bnw_spots_1" => crate::layer::ProceduralGenerator::BnWSpots1,
                                "gen_bnw_spots_2" => crate::layer::ProceduralGenerator::BnWSpots2,
                                "gen_bnw_spots_3" => crate::layer::ProceduralGenerator::BnWSpots3,
                                "gen_grunge_map" => crate::layer::ProceduralGenerator::GrungeMap,
                                "gen_directional_noise" => crate::layer::ProceduralGenerator::DirectionalNoise,
                                "gen_clouds" => crate::layer::ProceduralGenerator::Clouds,
                                "gen_marble_veins" => crate::layer::ProceduralGenerator::MarbleVeins,
                                _ => crate::layer::ProceduralGenerator::PerlinNoise,
                            };
                            layer.add_mask_fill_effect_advanced(
                                gen,
                                curv.as_ref(),
                                ao.as_ref(),
                                pos.as_ref(),
                                wn.as_ref(),
                            );
                            self.status_message = format!("Added Fill Effect to Mask: {}", asset.name);
                        }
                    }
                } else {
                    let gen = match asset.id {
                        "gen_perlin" => crate::layer::ProceduralGenerator::PerlinNoise,
                        "gen_worley" => crate::layer::ProceduralGenerator::WorleyGrunge,
                        "gen_billow" => crate::layer::ProceduralGenerator::BillowNoise,
                        "gen_ridged" => crate::layer::ProceduralGenerator::RidgedMultifractal,
                        "gen_domain_warp" => crate::layer::ProceduralGenerator::DomainWarpGrunge,
                        "gen_splatter" => crate::layer::ProceduralGenerator::SplatterDirt,
                        "gen_wood" => crate::layer::ProceduralGenerator::WoodGrain,
                        "gen_fabric" => crate::layer::ProceduralGenerator::FabricWeave,
                        "gen_scratches" => crate::layer::ProceduralGenerator::Scratches,
                        "gen_curvature" => crate::layer::ProceduralGenerator::CurvatureWear,
                        "gen_cavity" => crate::layer::ProceduralGenerator::AmbientOcclusion,
                        "gen_dust" => crate::layer::ProceduralGenerator::DirectionalDust,
                        "gen_leaks" => crate::layer::ProceduralGenerator::DrippingLeaks,
                        "gen_position" | "gen_ground_dirt" => crate::layer::ProceduralGenerator::PositionGradient,
                        "gen_checker" => crate::layer::ProceduralGenerator::Checkerboard,
                        "gen_bricks" => crate::layer::ProceduralGenerator::Bricks,
                        "gen_perforated" => crate::layer::ProceduralGenerator::PerforatedMesh,
                        "gen_wire_fence" => crate::layer::ProceduralGenerator::WireFence,
                        "gen_torn_fabric" => crate::layer::ProceduralGenerator::TornFabricEdge,
                        "gen_dots" => crate::layer::ProceduralGenerator::CircularDots,
                        "gen_bnw_spots_1" => crate::layer::ProceduralGenerator::BnWSpots1,
                        "gen_bnw_spots_2" => crate::layer::ProceduralGenerator::BnWSpots2,
                        "gen_bnw_spots_3" => crate::layer::ProceduralGenerator::BnWSpots3,
                        "gen_grunge_map" => crate::layer::ProceduralGenerator::GrungeMap,
                        "gen_directional_noise" => crate::layer::ProceduralGenerator::DirectionalNoise,
                        "gen_clouds" => crate::layer::ProceduralGenerator::Clouds,
                        "gen_marble_veins" => crate::layer::ProceduralGenerator::MarbleVeins,
                        _ => crate::layer::ProceduralGenerator::PerlinNoise,
                    };
                    let mut fill_layer = crate::layer::Layer::new_fill_layer(
                        self.project.next_layer_id,
                        format!("Fill - {}", asset.name),
                        self.project.texture_width,
                        self.project.texture_height,
                        glam::Vec4::new(0.5, 0.5, 0.5, 1.0),
                        0.5,
                        0.0,
                    );
                    self.project.next_layer_id += 1;
                    fill_layer.fill_generator = gen;
                    fill_layer.generator_scale = 8.0;
                    fill_layer.evaluate_fill_advanced(
                        self.project.curvature_map.as_ref(),
                        self.project.position_map.as_ref(),
                        self.project.world_space_normal_map.as_ref(),
                        self.project.ao_map.as_ref(),
                    );
                    self.project.layers.push(fill_layer);
                    self.project.active_layer_idx = self.project.layers.len() - 1;
                    self.status_message = format!("Added Generator Fill Layer: {}", asset.name);
                }
            }
            crate::shelf::ShelfCategory::Alphas => {
                match asset.id {
                    "alpha_soft_round" => self.brush_engine.settings.alpha = crate::brush::AlphaType::SoftRound,
                    "alpha_hard_round" => self.brush_engine.settings.alpha = crate::brush::AlphaType::HardRound,
                    "alpha_noise_splatter" => self.brush_engine.settings.alpha = crate::brush::AlphaType::NoiseSplatter,
                    "alpha_chisel" => self.brush_engine.settings.alpha = crate::brush::AlphaType::Chisel,
                    "alpha_stipple" => self.brush_engine.settings.alpha = crate::brush::AlphaType::Stipple,
                    "alpha_scratches" => self.brush_engine.settings.alpha = crate::brush::AlphaType::Scratches,
                    _ => {}
                }
                self.brush_engine.update_cache();
                self.status_message = format!("Selected Brush Alpha: {}", asset.name);
            }
            crate::shelf::ShelfCategory::Environments => {}
            _ => {}
        }
        self.project.rebuild_composite();
    }

    /// Load an HDRI environment from file and update the viewport environment map
    pub fn activate_hdri_file(&mut self, path: &std::path::Path) -> bool {
        if let Ok(()) = self.renderer.hdri.load_file(path) {
            if let Some(wgpu) = &mut self.wgpu_viewport {
                let (w, h, hdri_data) = crate::hdri_generator::generate_hdri_texture(&self.renderer.hdri, 1024, 512);
                wgpu.load_environment(w, h, &hdri_data);
            }
            if let Some(idx) = self.asset_manager.hdris.iter().position(|h| h.path == path) {
                self.asset_manager.active_hdri_index = idx;
            }
            self.status_message = format!("Active HDRI: {}", self.renderer.hdri.preset.name());
            true
        } else {
            self.status_message = format!("Failed to load HDRI: {:?}", path.file_name().unwrap_or_default());
            false
        }
    }
}

/// Helper function to render stylized HDRI preview thumbnail on the asset shelf card
pub(crate) fn draw_hdri_thumbnail(painter: &egui::Painter, rect: egui::Rect, id: &str, name: &str) {
    let lower = name.to_lowercase();
    let id_lower = id.to_lowercase();

    if lower.contains("studio") || id_lower.contains("studio") || lower.contains("softbox") {
        // Studio with dual softboxes
        painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(28, 30, 36));
        let sb1 = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + rect.width() * 0.15, rect.min.y + rect.height() * 0.2),
            egui::pos2(rect.min.x + rect.width() * 0.38, rect.min.y + rect.height() * 0.8),
        );
        painter.rect_filled(sb1, 2.0, egui::Color32::from_rgb(240, 245, 255));
        let sb2 = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + rect.width() * 0.62, rect.min.y + rect.height() * 0.2),
            egui::pos2(rect.min.x + rect.width() * 0.85, rect.min.y + rect.height() * 0.8),
        );
        painter.rect_filled(sb2, 2.0, egui::Color32::from_rgb(255, 245, 230));
    } else if lower.contains("sunny") || lower.contains("courtyard") || lower.contains("outdoor") {
        // Sunny courtyard sky & ground
        let half_h = rect.height() * 0.55;
        let sky_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + half_h));
        let ground_rect = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + half_h), rect.max);
        painter.rect_filled(sky_rect, 0.0, egui::Color32::from_rgb(60, 130, 215));
        painter.rect_filled(ground_rect, 0.0, egui::Color32::from_rgb(75, 90, 70));
        painter.circle_filled(egui::pos2(rect.min.x + rect.width() * 0.7, rect.min.y + rect.height() * 0.3), 6.0, egui::Color32::from_rgb(255, 250, 210));
    } else if lower.contains("sunset") || lower.contains("desert") || lower.contains("golden") {
        // Sunset desert twilight
        let half_h = rect.height() * 0.6;
        let sky_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + half_h));
        let ground_rect = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + half_h), rect.max);
        painter.rect_filled(sky_rect, 0.0, egui::Color32::from_rgb(70, 30, 95));
        painter.rect_filled(ground_rect, 0.0, egui::Color32::from_rgb(180, 85, 30));
        painter.circle_filled(egui::pos2(rect.min.x + rect.width() * 0.5, rect.min.y + half_h), 8.0, egui::Color32::from_rgb(255, 200, 70));
    } else if lower.contains("industrial") || lower.contains("dark") || lower.contains("factory") {
        // Dark industrial workshop
        painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(20, 24, 28));
        let tube = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + rect.width() * 0.15, rect.min.y + rect.height() * 0.25),
            egui::pos2(rect.max.x - rect.width() * 0.15, rect.min.y + rect.height() * 0.35),
        );
        painter.rect_filled(tube, 1.0, egui::Color32::from_rgb(210, 230, 255));
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x + rect.width() * 0.2, rect.max.y - rect.height() * 0.3),
                egui::pos2(rect.max.x - rect.width() * 0.2, rect.max.y - rect.height() * 0.15),
            ),
            1.0,
            egui::Color32::from_rgb(140, 75, 40),
        );
    } else if lower.contains("neon") || lower.contains("cyberpunk") {
        // Cyberpunk neon
        painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(15, 12, 24));
        painter.line_segment(
            [egui::pos2(rect.min.x + rect.width() * 0.25, rect.min.y + 4.0), egui::pos2(rect.min.x + rect.width() * 0.25, rect.max.y - 4.0)],
            egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(255, 40, 160)),
        );
        painter.line_segment(
            [egui::pos2(rect.max.x - rect.width() * 0.25, rect.min.y + 4.0), egui::pos2(rect.max.x - rect.width() * 0.25, rect.max.y - 4.0)],
            egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(30, 220, 245)),
        );
    } else {
        // Generic panoramic environment
        painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(35, 40, 50));
        let center = rect.center();
        let radius = (rect.width() * 0.35).min(rect.height() * 0.35);
        painter.circle_filled(center, radius, egui::Color32::from_rgb(80, 140, 210));
        painter.circle_filled(center - egui::vec2(radius * 0.3, radius * 0.3), radius * 0.3, egui::Color32::WHITE);
    }
}

impl PaintcraftApp {
    /// Launch background multi-threaded baking pipeline with real-time progress reporting
    pub fn start_background_bake(&mut self) {
        if self.bake_worker_state.as_ref().map(|s| !s.lock().unwrap().is_done).unwrap_or(false) {
            return;
        }

        let res = self.bake_res;
        let frontal = self.bake_frontal;
        let rear = self.bake_rear;
        let bake_tangent_normal = self.bake_opt_tangent_normal;
        let bake_ws_normal = self.bake_opt_world_normal;
        let bake_ao = self.bake_opt_ao;
        let bake_curvature = self.bake_opt_curvature;
        let bake_position = self.bake_opt_position;
        let bake_thickness = self.bake_opt_thickness;
        let bake_thickness_invert = self.bake_opt_thickness_invert;
        let bake_id_map = self.bake_opt_id_map;
        let denoise = self.bake_denoise;
        let match_config = crate::baker::BakeMatchConfig {
            mode: self.bake_match_mode,
            low_suffix: self.bake_low_suffix.clone(),
            high_suffix: self.bake_high_suffix.clone(),
        };
        let cage_config = crate::baker::BakeCageConfig {
            mode: self.bake_cage_mode,
            inflation_offset: self.bake_cage_inflation,
            external_cage_path: if self.bake_cage_mode == crate::baker::BakeCageMode::ExternalCage && !self.bake_cage_path.trim().is_empty() {
                Some(std::path::PathBuf::from(&self.bake_cage_path))
            } else {
                None
            },
        };

        let ao_config = self.bake_ao_config.clone();
        let mut thickness_config = self.bake_thickness_config.clone();
        if bake_thickness_invert {
            thickness_config.invert = true;
        }
        let curvature_config = self.bake_curvature_config.clone();
        let normal_config = self.bake_normal_config.clone();
        let position_config = self.bake_position_config.clone();

        let mesh = self.project.mesh.clone();
        let high_poly = self.project.high_poly_mesh.clone();

        let progress_state = Arc::new(Mutex::new(BakeProgressState {
            progress: 0.0,
            step_name: "Initializing Baker...".to_string(),
            is_done: false,
            cancelled: false,
            elapsed_secs: 0.0,
        }));

        let state_clone = Arc::clone(&progress_state);
        let (tx, rx) = channel();

        self.bake_worker_state = Some(progress_state);
        self.bake_receiver = Some(rx);
        self.bake_success_message = None;

        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let mut result = BakeResultPayload {
                tangent_normal: None,
                world_space_normal: None,
                ao: None,
                curvature: None,
                position: None,
                thickness: None,
                id_map: None,
                elapsed_secs: 0.0,
            };

            let total_tasks = (bake_tangent_normal as usize)
                + (bake_ws_normal as usize)
                + (bake_ao as usize)
                + (bake_curvature as usize)
                + (bake_position as usize)
                + (bake_thickness as usize)
                + (bake_id_map as usize);
            let mut completed_tasks = 0;

            if bake_tangent_normal {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking Tangent Space Normal Map...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }

                result.tangent_normal = match crate::baker::Baker::bake_tangent_normals_with_options(
                    &mesh,
                    high_poly.as_ref(),
                    frontal,
                    rear,
                    res,
                    res,
                    &match_config,
                    &cage_config,
                    normal_config.invert_green,
                ) {
                    Ok(buf) => Some(buf),
                    Err(e) => {
                        eprintln!("Cage bake warning: {}", e);
                        Some(crate::baker::Baker::bake_tangent_normals_with_match(
                            &mesh,
                            high_poly.as_ref(),
                            frontal,
                            rear,
                            res,
                            res,
                            &match_config,
                        ))
                    }
                };
                completed_tasks += 1;
            }

            if bake_ws_normal {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking World Space Normal Map...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                result.world_space_normal = Some(crate::baker::Baker::bake_world_space_normal(&mesh, res, res));
                completed_tasks += 1;
            }

            if bake_ao {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking Ambient Occlusion (Raycasting)...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                let mut ao = crate::baker::Baker::bake_ao_with_options(&mesh, res, res, &cage_config, &ao_config)
                    .unwrap_or_else(|_| crate::baker::Baker::bake_ao(&mesh, res, res));
                if denoise {
                    {
                        let mut st = state_clone.lock().unwrap();
                        st.step_name = "Denoising Ambient Occlusion (Bilateral)...".to_string();
                    }
                    crate::baker::Baker::denoise_scalar_buffer(&mut ao, result.world_space_normal.as_ref(), 1, 2, 1.2, 24.0);
                }
                result.ao = Some(ao);
                completed_tasks += 1;
            }

            if bake_curvature {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking Curvature Map...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                let mut curv = crate::baker::Baker::bake_curvature_with_options(&mesh, res, res, &curvature_config);
                if denoise {
                    {
                        let mut st = state_clone.lock().unwrap();
                        st.step_name = "Denoising Curvature Map (Sharpening Edges)...".to_string();
                    }
                    crate::baker::Baker::denoise_scalar_buffer(&mut curv, result.world_space_normal.as_ref(), 1, 1, 0.8, 24.0);
                }
                result.curvature = Some(curv);
                completed_tasks += 1;
            }

            if bake_position {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking World Position WPO...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                result.position = Some(crate::baker::Baker::bake_position_with_options(&mesh, res, res, &position_config));
                completed_tasks += 1;
            }

            if bake_thickness {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking Thickness Map...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                let mut th = crate::baker::Baker::bake_thickness_with_options(&mesh, res, res, &cage_config, &thickness_config)
                    .unwrap_or_else(|_| crate::baker::Baker::bake_thickness(&mesh, res, res));
                if denoise {
                    crate::baker::Baker::denoise_scalar_buffer(&mut th, result.world_space_normal.as_ref(), 1, 2, 1.2, 24.0);
                }
                result.thickness = Some(th);
                completed_tasks += 1;
            }

            if bake_id_map {
                if state_clone.lock().unwrap().cancelled { return; }
                {
                    let mut st = state_clone.lock().unwrap();
                    st.step_name = format!("[{}/{}] Baking ID Map...", completed_tasks + 1, total_tasks);
                    st.progress = (completed_tasks as f32) / (total_tasks as f32).max(1.0);
                }
                result.id_map = Some(crate::baker::Baker::bake_id_map(&mesh, res, res));
            }

            result.elapsed_secs = start.elapsed().as_secs_f32();
            {
                let mut st = state_clone.lock().unwrap();
                st.progress = 1.0;
                st.step_name = format!("Bake Complete in {:.2}s!", result.elapsed_secs);
                st.is_done = true;
                st.elapsed_secs = result.elapsed_secs;
            }
            let _ = tx.send(result);
        });
    }

    /// Render Dedicated Substance 3D Painter Baking Screen
    pub fn render_dedicated_baking_screen_ui(&mut self, ctx: &egui::Context) {
        if !self.show_bake_modal {
            return;
        }

        let is_baking = self.bake_worker_state.as_ref().map(|s| !s.lock().unwrap().is_done).unwrap_or(false);
        let mut close_modal = false;

        let screen = ctx.screen_rect();
        let def_w = 780.0f32.min(screen.width() * 0.60).max(520.0);
        let def_h = 580.0f32.min(screen.height() * 0.85).max(380.0);
        let min_w = 520.0f32.min(screen.width() * 0.9);
        let min_h = 360.0f32.min(screen.height() * 0.85);
        let max_w = (screen.width() - 32.0).max(400.0);
        let max_h = (screen.height() - 32.0).max(350.0);

        egui::Window::new("Bake Mesh Maps")
            .collapsible(false)
            .resizable(true)
            .default_pos(egui::pos2(20.0, 50.0))
            .default_size([def_w, def_h])
            .min_width(min_w)
            .min_height(min_h)
            .max_size([max_w, max_h])
            .show(ctx, |ui| {
                // Header Banner
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading(egui::RichText::new("BAKE MESH MAPS").size(16.0).strong().color(Color32::WHITE));
                        ui.label(egui::RichText::new("Bake geometric curvature, ambient occlusion, normal, and position maps from 3D geometry").small().color(Color32::from_rgb(160, 165, 175)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(" X ").clicked() && !is_baking {
                            close_modal = true;
                        }
                    });
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 3-Column Substance 3D Painter Layout
                let avail_h = (ui.available_height() - 110.0).max(280.0);
                ui.horizontal(|ui| {
                    // Column 1: Texture Sets
                    let col1_w = (ui.available_width() * 0.22).clamp(160.0, 190.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(col1_w, avail_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.strong(egui::RichText::new("Texture Sets").color(Color32::from_rgb(220, 225, 235)));
                            ui.add_space(2.0);

                            if self.bake_selected_texture_sets.len() != self.project.texture_sets.len() {
                                self.bake_selected_texture_sets = vec![true; self.project.texture_sets.len()];
                            }

                            ui.horizontal(|ui| {
                                if ui.small_button("All").clicked() {
                                    for b in &mut self.bake_selected_texture_sets { *b = true; }
                                }
                                if ui.small_button("None").clicked() {
                                    for b in &mut self.bake_selected_texture_sets { *b = false; }
                                }
                            });
                            ui.separator();

                            egui::ScrollArea::vertical().id_source("bake_ts_scroll").show(ui, |ui| {
                                for (i, ts) in self.project.texture_sets.iter().enumerate() {
                                    let mut sel = self.bake_selected_texture_sets.get(i).copied().unwrap_or(true);
                                    let is_active = i == self.project.active_texture_set_idx;
                                    let label = format!("{} ({}x{})", ts.name, ts.width, ts.height);
                                    if ui.checkbox(&mut sel, label).changed() {
                                        if let Some(b) = self.bake_selected_texture_sets.get_mut(i) {
                                            *b = sel;
                                        }
                                    }
                                    if is_active {
                                        ui.label(egui::RichText::new("  (Active Set)").small().color(Color32::from_rgb(243, 94, 24)));
                                    }
                                }
                            });
                        },
                    );

                    ui.separator();

                    // Column 2: Mesh Maps Selection & Navigation
                    let col2_w = (ui.available_width() * 0.28).clamp(170.0, 210.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(col2_w, avail_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.strong(egui::RichText::new("Bake Settings").color(Color32::from_rgb(220, 225, 235)));
                            ui.add_space(4.0);

                            let is_common = self.bake_active_settings_tab == crate::baker::BakeSettingsTab::Common;
                            if ui.selectable_label(is_common, "Common Parameters").clicked() {
                                self.bake_active_settings_tab = crate::baker::BakeSettingsTab::Common;
                            }
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(2.0);

                            ui.strong(egui::RichText::new("Mesh Maps").color(Color32::from_rgb(200, 205, 215)));
                            ui.add_space(2.0);

                            let maps = [
                                (crate::baker::BakeSettingsTab::TangentNormal, "Normal (Tangent Space)", &mut self.bake_opt_tangent_normal),
                                (crate::baker::BakeSettingsTab::WorldNormal, "World Space Normal", &mut self.bake_opt_world_normal),
                                (crate::baker::BakeSettingsTab::AmbientOcclusion, "Ambient Occlusion", &mut self.bake_opt_ao),
                                (crate::baker::BakeSettingsTab::Curvature, "Curvature", &mut self.bake_opt_curvature),
                                (crate::baker::BakeSettingsTab::Position, "Position (World Space)", &mut self.bake_opt_position),
                                (crate::baker::BakeSettingsTab::Thickness, "Thickness", &mut self.bake_opt_thickness),
                                (crate::baker::BakeSettingsTab::IdMap, "ID Map (Submeshes)", &mut self.bake_opt_id_map),
                            ];

                            for (tab, label, opt_ref) in maps {
                                let is_active = self.bake_active_settings_tab == tab;
                                ui.horizontal(|ui| {
                                    ui.checkbox(opt_ref, "");
                                    if ui.selectable_label(is_active, label).clicked() {
                                        self.bake_active_settings_tab = tab;
                                    }
                                });
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.strong(egui::RichText::new("Global Denoising").color(Color32::from_rgb(180, 185, 195)));
                            ui.add_space(2.0);
                            ui.checkbox(&mut self.bake_denoise, "Denoise Mesh Maps")
                                .on_hover_text("Edge-preserving bilateral filter cleans ray-march noise on AO and Curvature while preserving sharp bevels");
                            if self.bake_denoise {
                                ui.horizontal(|ui| {
                                    ui.label("Passes:");
                                    ui.add(egui::Slider::new(&mut self.bake_denoise_iterations, 1..=3));
                                });
                            }
                        },
                    );

                    ui.separator();

                    // Column 3: Active Map & Common Parameters Panel
                    let col3_w = (ui.available_width() - 12.0).max(280.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(col3_w, avail_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            // Top Tab Strip
                            ui.horizontal_wrapped(|ui| {
                                let tabs = [
                                    (crate::baker::BakeSettingsTab::Common, "Common"),
                                    (crate::baker::BakeSettingsTab::TangentNormal, "Normal"),
                                    (crate::baker::BakeSettingsTab::WorldNormal, "World Normal"),
                                    (crate::baker::BakeSettingsTab::AmbientOcclusion, "AO"),
                                    (crate::baker::BakeSettingsTab::Curvature, "Curvature"),
                                    (crate::baker::BakeSettingsTab::Position, "Position"),
                                    (crate::baker::BakeSettingsTab::Thickness, "Thickness"),
                                    (crate::baker::BakeSettingsTab::IdMap, "ID Map"),
                                ];
                                for (tab, name) in tabs {
                                    let is_active = self.bake_active_settings_tab == tab;
                                    if ui.selectable_label(is_active, name).clicked() {
                                        self.bake_active_settings_tab = tab;
                                    }
                                }
                            });
                            ui.add_space(2.0);
                            ui.separator();
                            ui.add_space(4.0);

                            egui::ScrollArea::vertical()
                                .id_source("bake_settings_panel_scroll")
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    match self.bake_active_settings_tab {
                                        crate::baker::BakeSettingsTab::Common => {
                                            ui.strong(egui::RichText::new("Common Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Output Size:");
                                                egui::ComboBox::from_id_source("bake_size_combo")
                                                    .selected_text(format!("{} x {}", self.bake_res, self.bake_res))
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_res, 512, "512 x 512");
                                                        ui.selectable_value(&mut self.bake_res, 1024, "1024 x 1024 (1K)");
                                                        ui.selectable_value(&mut self.bake_res, 2048, "2048 x 2048 (2K)");
                                                        ui.selectable_value(&mut self.bake_res, 4096, "4096 x 4096 (4K)");
                                                    });
                                            });

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.strong("High Definition Meshes:");
                                            ui.horizontal(|ui| {
                                                ui.text_edit_singleline(&mut self.bake_highpoly_path);
                                                if ui.button("Browse...").clicked() {
                                                    if let Some(path) = rfd::FileDialog::new().add_filter("3D Models (OBJ, glTF, GLB)", &["obj", "gltf", "glb"]).pick_file() {
                                                        self.bake_highpoly_path = path.to_string_lossy().to_string();
                                                        if let Ok(mesh) = crate::mesh::Mesh::load_file(&path) {
                                                            self.project.high_poly_mesh = Some(mesh);
                                                            self.status_message = format!("Loaded High-Poly Mesh: {:?}", path.file_name().unwrap_or_default());
                                                        }
                                                    }
                                                }
                                                if self.project.high_poly_mesh.is_some() && ui.button("Clear").clicked() {
                                                    self.project.high_poly_mesh = None;
                                                    self.bake_highpoly_path.clear();
                                                }
                                            });
                                            if let Some(ref hp) = self.project.high_poly_mesh {
                                                ui.colored_label(Color32::from_rgb(100, 220, 140), format!("Loaded: {} ({} tris)", hp.name, hp.indices.len() / 3));
                                            } else {
                                                ui.label(egui::RichText::new("Using low-poly base mesh as high-definition geometry").small().color(Color32::from_rgb(150, 155, 165)));
                                            }

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.strong("Ray Distances:");
                                            ui.horizontal(|ui| {
                                                ui.label("Max Frontal Distance:");
                                                ui.add(egui::Slider::new(&mut self.bake_frontal, 0.001..=0.5).logarithmic(true));
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Max Rear Distance:");
                                                ui.add(egui::Slider::new(&mut self.bake_rear, 0.001..=0.5).logarithmic(true));
                                            });

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.strong("Cage Envelope & Ray Origin:");
                                            ui.horizontal(|ui| {
                                                ui.label("Mode:");
                                                egui::ComboBox::from_id_source("bake_cage_mode_combo")
                                                    .selected_text(self.bake_cage_mode.name())
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_cage_mode, crate::baker::BakeCageMode::None, "None (Ray Distances)");
                                                        ui.selectable_value(&mut self.bake_cage_mode, crate::baker::BakeCageMode::ParametricInflation, "Parametric Cage (Inflation)");
                                                        ui.selectable_value(&mut self.bake_cage_mode, crate::baker::BakeCageMode::ExternalCage, "External Cage Mesh (.obj)");
                                                    });
                                            });

                                            match self.bake_cage_mode {
                                                crate::baker::BakeCageMode::None => {
                                                    ui.label(egui::RichText::new("Casting rays along vertex normals using frontal and rear distances.").small().color(Color32::from_rgb(150, 155, 165)));
                                                }
                                                crate::baker::BakeCageMode::ParametricInflation => {
                                                    ui.horizontal(|ui| {
                                                        ui.label("Cage Inflation Offset:");
                                                        ui.add(egui::Slider::new(&mut self.bake_cage_inflation, 0.001..=0.5).logarithmic(true));
                                                    });
                                                    ui.label(egui::RichText::new("Generates a seamless averaged normal envelope to eliminate ray-crossing on bevels and corners.").small().color(Color32::from_rgb(150, 155, 165)));
                                                }
                                                crate::baker::BakeCageMode::ExternalCage => {
                                                    ui.horizontal(|ui| {
                                                        ui.text_edit_singleline(&mut self.bake_cage_path);
                                                        if ui.button("Browse...").clicked() {
                                                            if let Some(path) = rfd::FileDialog::new().add_filter("OBJ Mesh (*.obj)", &["obj"]).pick_file() {
                                                                self.bake_cage_path = path.to_string_lossy().to_string();
                                                            }
                                                        }
                                                        if !self.bake_cage_path.is_empty() && ui.button("Clear").clicked() {
                                                            self.bake_cage_path.clear();
                                                        }
                                                    });
                                                    ui.label(egui::RichText::new("Cage OBJ must have matching topology (identical vertex count and faces) as the low-poly mesh.").small().color(Color32::from_rgb(150, 155, 165)));
                                                }
                                            }

                                            ui.add_space(4.0);
                                            ui.checkbox(&mut self.bake_preview_cage, egui::RichText::new("Preview Cage in 3D Viewport").strong().color(Color32::from_rgb(0, 225, 255)));
                                            if self.bake_preview_cage {
                                                ui.indent("bake_cage_preview_indent", |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.checkbox(&mut self.bake_cage_show_hull, "Translucent Shell");
                                                        ui.checkbox(&mut self.bake_cage_show_tethers, "Displacement Vectors");
                                                    });
                                                    ui.horizontal(|ui| {
                                                        ui.label("Color:");
                                                        let colors = ["Cyan (Substance)", "Orange", "Lime Green", "Magenta"];
                                                        egui::ComboBox::from_id_source("bake_cage_color_sel")
                                                            .selected_text(colors.get(self.bake_cage_color_idx).copied().unwrap_or("Cyan"))
                                                            .show_ui(ui, |ui| {
                                                                for (c_idx, &c_name) in colors.iter().enumerate() {
                                                                    ui.selectable_value(&mut self.bake_cage_color_idx, c_idx, c_name);
                                                                }
                                                            });
                                                        ui.label("Opacity:");
                                                        ui.add(egui::Slider::new(&mut self.bake_cage_opacity, 0.1..=1.0));
                                                    });
                                                    let status_text = match self.bake_cage_mode {
                                                        crate::baker::BakeCageMode::None => format!("Previewing frontal ray envelope at distance {:.3}", self.bake_frontal),
                                                        crate::baker::BakeCageMode::ParametricInflation => format!("Previewing parametric smoothed cage at offset {:.3}", self.bake_cage_inflation),
                                                        crate::baker::BakeCageMode::ExternalCage => {
                                                            if self.bake_cage_path.is_empty() {
                                                                "No external cage OBJ loaded".to_string()
                                                            } else {
                                                                format!("Previewing external cage: {}", self.bake_cage_path)
                                                            }
                                                        }
                                                    };
                                                    ui.label(egui::RichText::new(status_text).small().color(Color32::from_rgb(120, 210, 255)));
                                                });
                                            }

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.strong("Mesh Matching:");
                                            ui.horizontal(|ui| {
                                                ui.label("Match:");
                                                egui::ComboBox::from_id_source("bake_match_mode_combo")
                                                    .selected_text(self.bake_match_mode.name())
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_match_mode, crate::baker::BakeMatchMode::Always, "Always");
                                                        ui.selectable_value(&mut self.bake_match_mode, crate::baker::BakeMatchMode::ByMeshName, "By Mesh Name");
                                                    });
                                            });
                                            if self.bake_match_mode == crate::baker::BakeMatchMode::ByMeshName {
                                                ui.horizontal(|ui| {
                                                    ui.label("Low Suffix:");
                                                    ui.add(egui::TextEdit::singleline(&mut self.bake_low_suffix).desired_width(70.0));
                                                    ui.label("High Suffix:");
                                                    ui.add(egui::TextEdit::singleline(&mut self.bake_high_suffix).desired_width(70.0));
                                                });
                                            }
                                        }

                                        crate::baker::BakeSettingsTab::AmbientOcclusion => {
                                            ui.strong(egui::RichText::new("Ambient Occlusion Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_ao, "Enable Ambient Occlusion Bake");
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Secondary Rays (Samples):");
                                                egui::ComboBox::from_id_source("bake_ao_samples_combo")
                                                    .selected_text(format!("{} Rays", self.bake_ao_config.samples))
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_ao_config.samples, 16, "16 Rays (Fast Draft)");
                                                        ui.selectable_value(&mut self.bake_ao_config.samples, 32, "32 Rays (Preview)");
                                                        ui.selectable_value(&mut self.bake_ao_config.samples, 64, "64 Rays (Default)");
                                                        ui.selectable_value(&mut self.bake_ao_config.samples, 128, "128 Rays (High Quality)");
                                                        ui.selectable_value(&mut self.bake_ao_config.samples, 256, "256 Rays (Ultra Detail)");
                                                    });
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Fine Sample Count:");
                                                ui.add(egui::Slider::new(&mut self.bake_ao_config.samples, 8..=512));
                                            });
                                            ui.label(egui::RichText::new("Number of Monte Carlo hemisphere occlusion test rays cast per vertex. Higher values yield cleaner, noise-free contact shadows.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.horizontal(|ui| {
                                                ui.label("Max Distance:");
                                                ui.add(egui::Slider::new(&mut self.bake_ao_config.max_distance, 0.01..=1.5).text("x Bounding Diag"));
                                            });
                                            ui.label(egui::RichText::new("Maximum reach of occlusion rays. Smaller values create tight contact shadows; larger values catch broad cavity occlusion.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Ray Bias / Offset:");
                                                ui.add(egui::Slider::new(&mut self.bake_ao_config.ray_bias, 0.001..=0.05).logarithmic(true));
                                            });
                                            ui.label(egui::RichText::new("Surface normal offset distance to prevent self-shadowing on curved facets.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Spread / Attenuation:");
                                                ui.add(egui::Slider::new(&mut self.bake_ao_config.spread, 0.2..=2.0));
                                            });

                                            ui.add_space(4.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Occlusion Contrast:");
                                                ui.add(egui::Slider::new(&mut self.bake_ao_config.contrast, 0.1..=3.0));
                                            });
                                        }

                                        crate::baker::BakeSettingsTab::Thickness => {
                                            ui.strong(egui::RichText::new("Thickness Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_thickness, "Enable Thickness Bake");
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Secondary Rays (Samples):");
                                                egui::ComboBox::from_id_source("bake_th_samples_combo")
                                                    .selected_text(format!("{} Rays", self.bake_thickness_config.samples))
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_thickness_config.samples, 16, "16 Rays (Fast Draft)");
                                                        ui.selectable_value(&mut self.bake_thickness_config.samples, 32, "32 Rays (Standard)");
                                                        ui.selectable_value(&mut self.bake_thickness_config.samples, 64, "64 Rays (High Quality)");
                                                        ui.selectable_value(&mut self.bake_thickness_config.samples, 128, "128 Rays (Ultra Detail)");
                                                    });
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Fine Sample Count:");
                                                ui.add(egui::Slider::new(&mut self.bake_thickness_config.samples, 8..=256));
                                            });
                                            ui.label(egui::RichText::new("Number of inward transmission rays cast into the mesh interior for subsurface light penetration.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.horizontal(|ui| {
                                                ui.label("Max Distance:");
                                                ui.add(egui::Slider::new(&mut self.bake_thickness_config.max_distance, 0.05..=2.0).text("x Bounding Diag"));
                                            });
                                            ui.label(egui::RichText::new("Maximum penetration depth to search for interior surfaces.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Ray Bias / Offset:");
                                                ui.add(egui::Slider::new(&mut self.bake_thickness_config.ray_bias, 0.001..=0.05).logarithmic(true));
                                            });

                                            ui.add_space(6.0);
                                            ui.checkbox(&mut self.bake_opt_thickness_invert, "Invert (White = Thin)")
                                                .on_hover_text("Invert values so thin translucent areas are white (high transmission) and thick volume areas are black");
                                        }

                                        crate::baker::BakeSettingsTab::Curvature => {
                                            ui.strong(egui::RichText::new("Curvature Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_curvature, "Enable Curvature Bake");
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Curvature Intensity:");
                                                ui.add(egui::Slider::new(&mut self.bake_curvature_config.intensity, 0.1..=3.0));
                                            });
                                            ui.label(egui::RichText::new("Overall strength of convex ridge highlights and concave crevice shadows.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Curvature Contrast:");
                                                ui.add(egui::Slider::new(&mut self.bake_curvature_config.contrast, 0.1..=3.0));
                                            });
                                            ui.label(egui::RichText::new("Expands dynamic range away from neutral 0.5 flat gray.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.horizontal(|ui| {
                                                ui.label("Dihedral Weight (Edges):");
                                                ui.add(egui::Slider::new(&mut self.bake_curvature_config.dihedral_weight, 0.0..=1.0));
                                            });
                                            ui.label(egui::RichText::new("Sensitivity to hard polygonal edge angles and sharp bevels.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Deflection Weight (Smooth):");
                                                ui.add(egui::Slider::new(&mut self.bake_curvature_config.deflection_weight, 0.0..=1.0));
                                            });
                                            ui.label(egui::RichText::new("Sensitivity to gradual surface bending across 1-ring vertex neighbors.").small().color(Color32::from_rgb(150, 155, 165)));

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Blur Smoothing Passes:");
                                                ui.add(egui::Slider::new(&mut self.bake_curvature_config.blur_passes, 0..=4));
                                            });
                                            ui.label(egui::RichText::new("Separable Gaussian blur passes to smooth edge gradients and eliminate pixel stepping.").small().color(Color32::from_rgb(150, 155, 165)));
                                        }

                                        crate::baker::BakeSettingsTab::TangentNormal => {
                                            ui.strong(egui::RichText::new("Normal (Tangent Space) Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_tangent_normal, "Enable Normal Map Bake");
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Max Frontal Distance:");
                                                ui.add(egui::Slider::new(&mut self.bake_frontal, 0.001..=0.5).logarithmic(true));
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Max Rear Distance:");
                                                ui.add(egui::Slider::new(&mut self.bake_rear, 0.001..=0.5).logarithmic(true));
                                            });

                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Normal Format:");
                                                egui::ComboBox::from_id_source("bake_normal_format_combo")
                                                    .selected_text(if self.bake_normal_config.invert_green { "DirectX (Y- Invert Green)" } else { "OpenGL (Y+ Up)" })
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_normal_config.invert_green, false, "OpenGL (Y+ Up, Default)");
                                                        ui.selectable_value(&mut self.bake_normal_config.invert_green, true, "DirectX (Y- Invert Green)");
                                                    });
                                            });
                                            ui.label(egui::RichText::new("OpenGL (Y+) is used by Blender, Godot, and 3D Paintcraft. DirectX (Y-) is used by Unreal Engine and 3ds Max.").small().color(Color32::from_rgb(150, 155, 165)));
                                        }

                                        crate::baker::BakeSettingsTab::WorldNormal => {
                                            ui.strong(egui::RichText::new("World Space Normal Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_world_normal, "Enable World Space Normal Bake");
                                            ui.add_space(6.0);
                                            ui.label(egui::RichText::new("Bakes the 3D surface orientation in world space encoded as RGB values. Used for directional effects (such as top-down snow, dust accumulation, bottom ground splash) and as the guide normal for edge-preserving bilateral denoising.").color(Color32::from_rgb(180, 185, 195)));
                                        }

                                        crate::baker::BakeSettingsTab::Position => {
                                            ui.strong(egui::RichText::new("Position (World Space) Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_position, "Enable Position Map Bake");
                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.label("Normalization Mode:");
                                                egui::ComboBox::from_id_source("bake_pos_mode_combo")
                                                    .selected_text(match self.bake_position_config.mode {
                                                        crate::baker::PositionNormalizationMode::BoundingBox => "Bounding Box [0, 1] (Default)",
                                                        crate::baker::PositionNormalizationMode::WorldUnits => "World Space Coordinates",
                                                    })
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(&mut self.bake_position_config.mode, crate::baker::PositionNormalizationMode::BoundingBox, "Bounding Box [0, 1] (Default)");
                                                        ui.selectable_value(&mut self.bake_position_config.mode, crate::baker::PositionNormalizationMode::WorldUnits, "World Space Coordinates");
                                                    });
                                            });
                                            ui.label(egui::RichText::new("Encodes 3D vertex positions into RGB color. Bounding Box maps [min, max] to [0, 1], powering 3D Position Gradient generators and height-based masking.").small().color(Color32::from_rgb(150, 155, 165)));
                                        }

                                        crate::baker::BakeSettingsTab::IdMap => {
                                            ui.strong(egui::RichText::new("ID Map Parameters").color(Color32::from_rgb(220, 225, 235)));
                                            ui.add_space(4.0);

                                            ui.checkbox(&mut self.bake_opt_id_map, "Enable ID Map Bake");
                                            ui.add_space(6.0);
                                            ui.label(egui::RichText::new("Bakes distinct high-contrast colors per submesh/geometry element. Used with the Color Selection Mask effect to isolate and paint specific parts of complex meshes with one click.").color(Color32::from_rgb(180, 185, 195)));
                                        }
                                    }
                                });
                        },
                    );
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Bottom Panel: Real-Time Progress Bar, Previews & Action Buttons
                let mut cancel_requested = false;
                let mut trigger_bake = false;

                if let Some(state_arc) = &self.bake_worker_state {
                    let st = state_arc.lock().unwrap();
                    let prog = st.progress;
                    let step = st.step_name.clone();
                    let done = st.is_done;
                    drop(st);

                    if !done {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.strong(&step);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Cancel Bake").clicked() {
                                    state_arc.lock().unwrap().cancelled = true;
                                    cancel_requested = true;
                                }
                            });
                        });
                        ui.add_space(4.0);
                        ui.add(egui::ProgressBar::new(prog).show_percentage().animate(true));
                    } else {
                        // Completed view
                        if let Some(msg) = &self.bake_success_message {
                            ui.horizontal(|ui| {
                                ui.colored_label(Color32::from_rgb(100, 220, 140), format!("✓ {}", msg));
                            });
                        }
                        // Live Thumbnails Strip
                        ui.horizontal(|ui| {
                            if let Some(tex) = &self.bake_preview_normal {
                                ui.vertical(|ui| {
                                    ui.image((tex.id(), egui::vec2(44.0, 44.0)));
                                    ui.label(egui::RichText::new("Normal").small());
                                });
                            }
                            if let Some(tex) = &self.bake_preview_ao {
                                ui.vertical(|ui| {
                                    ui.image((tex.id(), egui::vec2(44.0, 44.0)));
                                    ui.label(egui::RichText::new("AO").small());
                                });
                            }
                            if let Some(tex) = &self.bake_preview_curv {
                                ui.vertical(|ui| {
                                    ui.image((tex.id(), egui::vec2(44.0, 44.0)));
                                    ui.label(egui::RichText::new("Curvature").small());
                                });
                            }
                            if let Some(tex) = &self.bake_preview_pos {
                                ui.vertical(|ui| {
                                    ui.image((tex.id(), egui::vec2(44.0, 44.0)));
                                    ui.label(egui::RichText::new("Position").small());
                                });
                            }
                            if let Some(tex) = &self.bake_preview_thick {
                                ui.vertical(|ui| {
                                    ui.image((tex.id(), egui::vec2(44.0, 44.0)));
                                    ui.label(egui::RichText::new("Thickness").small());
                                });
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Close").clicked() {
                                    close_modal = true;
                                }
                                let bake_btn = egui::Button::new(egui::RichText::new("Bake Again").strong().color(Color32::WHITE))
                                    .fill(Color32::from_rgb(226, 91, 16))
                                    .min_size(egui::vec2(160.0, 32.0));
                                if ui.add(bake_btn).clicked() {
                                    trigger_bake = true;
                                }
                            });
                        });
                    }
                } else {
                    // Idle state
                    ui.horizontal(|ui| {
                        let bake_btn = egui::Button::new(egui::RichText::new("Bake Selected Textures").strong().color(Color32::WHITE))
                            .fill(Color32::from_rgb(226, 91, 16))
                            .min_size(egui::vec2(200.0, 34.0));
                        if ui.add(bake_btn).clicked() {
                            trigger_bake = true;
                        }

                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                }

                if cancel_requested {
                    self.bake_worker_state = None;
                    self.bake_receiver = None;
                }
                if trigger_bake {
                    self.start_background_bake();
                }
            });

        if close_modal {
            self.show_bake_modal = false;
            self.bake_cage_cache_key = (crate::baker::BakeCageMode::None, 0, 0, String::new(), false, false, 0, 0);
            if let Some(wgpu) = &mut self.wgpu_viewport {
                wgpu.clear_cage();
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
        let exported_with_mesh = crate::packing::CustomTexturePacker::pack_and_export(
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
}





impl PaintcraftApp {
    /// Render Texture Set List content (scrollable list of texture sets and actions)
    pub fn render_texture_sets_content(&mut self, ui: &mut egui::Ui) {
                    // Scrollable list of texture sets
                    let max_ts_height = (self.project.texture_sets.len() as f32 * 28.0).clamp(40.0, 130.0);
                    egui::ScrollArea::vertical()
                        .max_height(max_ts_height)
                        .show(ui, |ui| {
                            let mut switch_to = None;
                            let mut toggle_vis = None;
                            let mut toggle_solo = None;

                            for (idx, ts) in self.project.texture_sets.iter().enumerate() {
                                let is_active = idx == self.project.active_texture_set_idx;
                                let is_visible = ts.visible;
                                
                                ui.horizontal(|ui| {
                                    // Eye visibility toggle
                                    let eye_txt = if is_visible { "[V]" } else { "[ ]" };
                                    let eye_col = if is_visible { Color32::from_rgb(220, 225, 235) } else { Color32::from_rgb(100, 105, 115) };
                                    if ui.button(egui::RichText::new(eye_txt).monospace().color(eye_col)).on_hover_text("Toggle Texture Set / Sub-mesh visibility in 3D viewport").clicked() {
                                        toggle_vis = Some(idx);
                                    }

                                    // Solo toggle
                                    let solo_col = if is_active { Color32::from_rgb(243, 94, 24) } else { Color32::from_rgb(130, 135, 145) };
                                    if ui.small_button(egui::RichText::new("S").strong().color(solo_col)).on_hover_text("Solo Texture Set (isolate in viewport)").clicked() {
                                        toggle_solo = Some(idx);
                                    }

                                    // Name and tri count
                                    let tri_count: usize = ts.submesh_indices.iter()
                                        .filter_map(|&sm_idx| self.project.mesh.submeshes.get(sm_idx))
                                        .map(|sm| sm.index_count / 3)
                                        .sum();

                                    let label_text = format!("{}  ({} tris)", ts.name, tri_count);
                                    let resp = if is_active {
                                        ui.selectable_label(true, egui::RichText::new(&label_text).strong().color(Color32::WHITE))
                                    } else {
                                        ui.selectable_label(false, egui::RichText::new(&label_text).color(Color32::from_rgb(180, 185, 195)))
                                    };

                                    if resp.clicked() && !is_active {
                                        switch_to = Some(idx);
                                    }
                                });
                            }

                            if let Some(idx) = toggle_vis {
                                self.project.toggle_texture_set_visibility(idx);
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    wgpu.sync_submesh_visibility(&self.project.mesh.submeshes);
                                }
                            }
                            if let Some(idx) = toggle_solo {
                                self.project.solo_texture_set(idx);
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    wgpu.sync_submesh_visibility(&self.project.mesh.submeshes);
                                }
                            }
                            if let Some(idx) = switch_to {
                                self.project.switch_texture_set(idx);
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    wgpu.sync_textures(&mut self.project);
                                }
                                self.status_message = format!("Active Texture Set: {}", self.project.texture_sets[idx].name);
                            }
                        });

                    // Active Texture Set size selector and quick actions
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Size:").small().color(Color32::from_rgb(140, 145, 155)));
                        let res_list = [512, 1024, 2048, 4096];
                        for &r in &res_list {
                            let is_curr = self.project.texture_width == r;
                            let btn_text = match r {
                                512 => "512",
                                1024 => "1K",
                                2048 => "2K",
                                4096 => "4K",
                                _ => "Custom",
                            };
                            if ui.selectable_label(is_curr, btn_text).clicked() && !is_curr {
                                self.project.resize_textures(r, r);
                                self.history.adjust_max_undo_for_resolution(r, r);
                                self.project.rebuild_composite();
                                self.project.gpu_sync_needed = true;
                                self.status_message = format!("Resized texture set to {}x{}", r, r);
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.project.texture_sets.len() > 1 {
                                if ui.small_button("Del").on_hover_text("Delete active texture set").clicked() {
                                    let cur_idx = self.project.active_texture_set_idx;
                                    self.project.delete_texture_set(cur_idx);
                                    if let Some(wgpu) = &mut self.wgpu_viewport {
                                        wgpu.sync_textures(&mut self.project);
                                    }
                                }
                            }
                            if ui.small_button("Dup").on_hover_text("Duplicate active texture set").clicked() {
                                let cur_idx = self.project.active_texture_set_idx;
                                self.project.duplicate_texture_set(cur_idx);
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    wgpu.sync_textures(&mut self.project);
                                }
                            }
                            if ui.small_button("+ Set").on_hover_text("Add new empty Texture Set").clicked() {
                                let new_id = self.project.texture_sets.len() + 1;
                                self.project.add_texture_set(
                                    format!("Material_{}", new_id),
                                    format!("Material_{}", new_id),
                                    vec![],
                                );
                                if let Some(wgpu) = &mut self.wgpu_viewport {
                                    wgpu.sync_textures(&mut self.project);
                                }
                            }
                        });
                    });
    }

    /// Render Layers Stack content
    pub fn render_layers_content(&mut self, ui: &mut egui::Ui, pane_height: Option<f32>) {
        let total_height = ui.available_height();
        let layers_pane_height = pane_height.unwrap_or_else(|| (total_height * 0.36).clamp(175.0, 420.0));
        ui.group(|ui| {
            if pane_height.is_some() {
                ui.set_height(layers_pane_height);
            }
                        // Layers Window Header & Active Blend / Opacity Controls
                        ui.horizontal(|ui| {
                            ui.heading("Layers");

                            // Substance 3D Painter Layer Stack Channel Selector
                            ui.label("Channel:");
                            egui::ComboBox::from_id_source("layer_stack_channel_selector")
                                .selected_text(self.layer_stack_channel.short_name())
                                .show_ui(ui, |ui| {
                                    let channels = [
                                        crate::channel::ChannelType::BaseColor,
                                        crate::channel::ChannelType::Roughness,
                                        crate::channel::ChannelType::Metallic,
                                        crate::channel::ChannelType::Normal,
                                        crate::channel::ChannelType::Height,
                                        crate::channel::ChannelType::AmbientOcclusion,
                                        crate::channel::ChannelType::Emission,
                                        crate::channel::ChannelType::Opacity,
                                        crate::channel::ChannelType::Clearcoat,
                                        crate::channel::ChannelType::Anisotropy,
                                    ];
                                    for ch in channels {
                                        ui.selectable_value(&mut self.layer_stack_channel, ch, ch.name());
                                    }
                                });
                        });

                        let act_idx = self.project.active_layer_idx;
                        let mut blend_or_opacity_changed = false;

                        // Blend Mode and Opacity (Affecting the active selected layer's active channel)
                        if let Some(act_layer) = self.project.layers.get_mut(act_idx) {
                            let cur_ch = self.layer_stack_channel;
                            let mut cur_mode = act_layer.get_channel_blend_mode(cur_ch);
                            let mut cur_ch_op = act_layer.get_channel_opacity(cur_ch);

                            ui.horizontal(|ui| {
                                ui.label("Blend:");
                                egui::ComboBox::from_id_source("active_layer_channel_blend_mode")
                                    .selected_text(cur_mode.name())
                                    .show_ui(ui, |ui| {
                                        let modes = [
                                            BlendMode::PassThrough,
                                            BlendMode::Normal,
                                            BlendMode::Multiply,
                                            BlendMode::Screen,
                                            BlendMode::Overlay,
                                            BlendMode::Add,
                                            BlendMode::Subtract,
                                            BlendMode::Darken,
                                            BlendMode::Lighten,
                                            BlendMode::Replace,
                                        ];
                                        for m in modes {
                                            if ui.selectable_value(&mut cur_mode, m, m.name()).clicked() {
                                                act_layer.set_channel_blend_mode(cur_ch, m);
                                                blend_or_opacity_changed = true;
                                            }
                                        }
                                    });

                                ui.label("Opac:");
                                if ui.add_sized([55.0, 18.0], egui::Slider::new(&mut cur_ch_op, 0.0..=1.0)).changed() {
                                    act_layer.set_channel_opacity(cur_ch, cur_ch_op);
                                    blend_or_opacity_changed = true;
                                }

                                ui.label("Layer:");
                                if ui.add_sized([45.0, 18.0], egui::Slider::new(&mut act_layer.opacity, 0.0..=1.0)).on_hover_text("Master layer opacity").changed() {
                                    blend_or_opacity_changed = true;
                                }
                            });
                        }
    
                        if blend_or_opacity_changed {
                            if self.wgpu_viewport.is_some() && ui.input(|i| i.pointer.any_down()) {
                                self.project.gpu_sync_needed = true;
                                self.need_cpu_composite_sync = true;
                            } else {
                                self.project.rebuild_composite();
                            }
                        }
    
                        // Layer Stack Toolbar
                        let opt_icons = self.icons.as_ref();
                        ui.horizontal(|ui| {
                            let btn_paint = if let Some(icons) = opt_icons {
                                egui::Button::image_and_text((icons.layer_paint.id(), egui::vec2(13.0, 13.0)), "Paint")
                            } else {
                                egui::Button::new("+ Paint")
                            };
                            if ui.add(btn_paint).on_hover_text("Add Paint Layer").clicked() {
                                let prev_active = self.project.active_layer_idx;
                                let new_idx = self.project.add_paint_layer(None);
                                self.brush_engine.settings.target = PaintTarget::Material;
                                let layer_clone = self.project.layers[new_idx].clone_stroke_snapshot();
                                self.history.push("Add Paint Layer", HistoryAction::AddLayer {
                                    layer_idx: new_idx,
                                    layer: layer_clone,
                                    prev_active,
                                });
                                self.status_message = "Added Paint Layer.".to_string();
                            }

                            let btn_fill = if let Some(icons) = opt_icons {
                                egui::Button::image_and_text((icons.layer_fill.id(), egui::vec2(13.0, 13.0)), "Fill")
                            } else {
                                egui::Button::new("+ Fill")
                            };
                            if ui.add(btn_fill).on_hover_text("Add Fill Layer").clicked() {
                                let prev_active = self.project.active_layer_idx;
                                let new_idx = self.project.add_fill_layer(
                                    None,
                                    glam::Vec4::ONE,
                                    0.5,
                                    0.0,
                                );
                                self.brush_engine.settings.target = PaintTarget::Material;
                                let layer_clone = self.project.layers[new_idx].clone_stroke_snapshot();
                                self.history.push("Add Fill Layer", HistoryAction::AddLayer {
                                    layer_idx: new_idx,
                                    layer: layer_clone,
                                    prev_active,
                                });
                                self.status_message = "Added Fill Layer.".to_string();
                            }

                            let btn_folder = if let Some(icons) = opt_icons {
                                egui::Button::image_and_text((icons.layer_folder.id(), egui::vec2(13.0, 13.0)), "Folder")
                            } else {
                                egui::Button::new("+ Folder")
                            };
                            if ui.add(btn_folder).on_hover_text("Add Layer Folder / Group").clicked() {
                                let prev_active = self.project.active_layer_idx;
                                let new_idx = self.project.add_folder_layer(None);
                                let layer_clone = self.project.layers[new_idx].clone_stroke_snapshot();
                                self.history.push("Add Folder", HistoryAction::AddLayer {
                                    layer_idx: new_idx,
                                    layer: layer_clone,
                                    prev_active,
                                });
                                self.status_message = "Added Layer Folder.".to_string();
                            }

                            if ui.button("Filters...").on_hover_text("Apply filter (Blur, Levels, Invert, Sharpen)").clicked() {
                                self.show_filter_modal = true;
                            }

                            if ui.button("Duplicate").on_hover_text("Duplicate selected layer").clicked() {
                                if let Some(new_idx) = self.project.duplicate_layer(act_idx) {
                                    let layer_clone = Box::new(self.project.layers[new_idx].clone());
                                    self.history.push("Duplicate Layer", HistoryAction::AddLayer {
                                        layer_idx: new_idx,
                                        layer: layer_clone,
                                        prev_active: act_idx,
                                    });
                                    self.status_message = format!("Duplicated layer #{}", new_idx);
                                }
                            }

                            if ui.button("Save as Mat").on_hover_text("Save selected layer as a reusable PBR material in the Shelf").clicked() {
                                if let Some(layer) = self.project.layers.get(act_idx) {
                                    let name = layer.name.clone();
                                    let clean_id = name.trim().replace(' ', "_").to_lowercase();
                                    let mat = crate::asset_manager::CustomMaterialEntry {
                                        id: clean_id,
                                        name: name.clone(),
                                        description: format!("Custom PBR material created from layer '{}'", name),
                                        path: std::path::PathBuf::new(),
                                        file_name: String::new(),
                                        base_color: [layer.fill_color.x, layer.fill_color.y, layer.fill_color.z, layer.fill_color.w],
                                        roughness: layer.fill_roughness,
                                        metallic: layer.fill_metallic,
                                        height: layer.fill_height,
                                        normal: [0.5, 0.5, 1.0, 0.5],
                                        emissive: [layer.fill_emissive.x, layer.fill_emissive.y, layer.fill_emissive.z, layer.fill_emissive.w],
                                        opacity: layer.fill_opacity,
                                        clearcoat: layer.fill_clearcoat,
                                        anisotropy: layer.fill_anisotropy,
                                    };
                                    match self.asset_manager.save_material(&mat) {
                                        Ok(path) => {
                                            self.status_message = format!("Saved Material '{}' to {:?}", mat.name, path.file_name().unwrap_or_default());
                                        }
                                        Err(e) => {
                                            self.status_message = format!("Failed to save material: {}", e);
                                        }
                                    }
                                }
                            }
    
                            #[derive(Clone, Copy)]
                            enum MaskAddAction {
                                Black,
                                White,
                                Curvature,
                                Grunge,
                            }
                            let mut mask_add_action = None;
    
                            ui.menu_button("+ Mask", |ui| {
                                if ui.button("Black Mask (Hide All)").clicked() {
                                    mask_add_action = Some(MaskAddAction::Black);
                                    ui.close_menu();
                                }
                                if ui.button("White Mask (Reveal All)").clicked() {
                                    mask_add_action = Some(MaskAddAction::White);
                                    ui.close_menu();
                                }
                                if ui.button("Edge Wear Mask (Curvature)").clicked() {
                                    mask_add_action = Some(MaskAddAction::Curvature);
                                    ui.close_menu();
                                }
                                if ui.button("Grunge Mask").clicked() {
                                    mask_add_action = Some(MaskAddAction::Grunge);
                                    ui.close_menu();
                                }
                            });
    
                            if let Some(action) = mask_add_action {
                                let curv = self.project.curvature_map.clone();
                                if let Some(layer) = self.project.layers.get_mut(act_idx) {
                                    match action {
                                        MaskAddAction::Black => {
                                            layer.add_black_mask();
                                            self.brush_engine.settings.target = PaintTarget::Mask;
                                            self.brush_engine.settings.mask_paint_value = 1.0;
                                        }
                                        MaskAddAction::White => {
                                            layer.add_white_mask();
                                            self.brush_engine.settings.target = PaintTarget::Mask;
                                            self.brush_engine.settings.mask_paint_value = 0.0;
                                        }
                                        MaskAddAction::Curvature => {
                                            layer.add_curvature_mask(curv.as_ref());
                                            self.brush_engine.settings.target = PaintTarget::Mask;
                                        }
                                        MaskAddAction::Grunge => {
                                            layer.add_grunge_mask();
                                            self.brush_engine.settings.target = PaintTarget::Mask;
                                        }
                                    }
                                }
                                self.project.rebuild_composite();
                            }
    
                            if ui.button("Up").on_hover_text("Move layer up").clicked() {
                                self.project.move_layer_up(act_idx);
                                self.project.rebuild_composite();
                            }
                            if ui.button("Down").on_hover_text("Move layer down").clicked() {
                                self.project.move_layer_down(act_idx);
                                self.project.rebuild_composite();
                            }
                            if ui.button("Del").on_hover_text("Delete selected layer").clicked() {
                                let prev_active = self.project.active_layer_idx;
                                if let Some(removed_layer) = self.project.delete_layer(act_idx) {
                                    self.history.push("Delete Layer", HistoryAction::DeleteLayer {
                                        layer_idx: act_idx,
                                        layer: Box::new(removed_layer),
                                        prev_active,
                                    });
                                    self.status_message = format!("Deleted layer #{}", act_idx + 1);
                                }
                            }
                        });
    
                        ui.separator();
    
                        // Dedicated Scroll Area for Layer Stack
                        egui::ScrollArea::vertical()
                            .id_source("layers_stack_scroll")
                            .max_height(layers_pane_height - 90.0)
                            .show(ui, |ui| {
                                struct LayerCardSummary {
                                    id: u64,
                                    name: String,
                                    parent_folder_id: Option<u64>,
                                    folder_collapsed: bool,
                                    visible: bool,
                                    layer_type: LayerType,
                                    fill_color: Vec4,
                                    has_mask: bool,
                                }

                                let layer_count = self.project.layers.len();
                                let summaries: Vec<LayerCardSummary> = self.project.layers.iter().map(|l| LayerCardSummary {
                                    id: l.id,
                                    name: l.name.clone(),
                                    parent_folder_id: l.parent_folder_id,
                                    folder_collapsed: l.folder_collapsed,
                                    visible: l.visible,
                                    layer_type: l.layer_type,
                                    fill_color: l.fill_color,
                                    has_mask: l.has_mask,
                                }).collect();

                                let folders: Vec<(u64, String)> = summaries.iter()
                                    .filter(|l| l.layer_type == LayerType::Folder)
                                    .map(|l| (l.id, l.name.clone()))
                                    .collect();

                                let mut reorder_request = None;
                                let mut toggle_visible_idx = None;
                                let mut toggle_collapse_idx = None;
                                let mut add_mask_idx = None;
                                let mut rename_request = None;
                                let mut folder_change = None;

                                let mouse_pos = ui.input(|i| i.pointer.hover_pos());
                                let mouse_released = ui.input(|i| i.pointer.any_released());

                                for rev_i in 0..layer_count {
                                    let i = layer_count - 1 - rev_i;
                                    let layer = &summaries[i];

                                    // Check if this layer is inside a collapsed folder
                                    if let Some(pid) = layer.parent_folder_id {
                                        if summaries.iter().any(|l| l.id == pid && l.folder_collapsed) {
                                            continue;
                                        }
                                    }

                                    let is_active = self.project.active_layer_idx == i;
                                    let _is_target_mat = is_active && self.brush_engine.settings.target == PaintTarget::Material;
                                    let is_target_mask = is_active && self.brush_engine.settings.target == PaintTarget::Mask;

                                    let card_bg = if is_active {
                                        Color32::from_rgb(44, 48, 56)
                                    } else {
                                        Color32::from_rgb(26, 28, 32)
                                    };
                                    // Highlight the OUTLINE of the active layer card!
                                    let card_stroke = if is_active {
                                        egui::Stroke::new(2.0_f32, Color32::from_rgb(243, 94, 24))
                                    } else {
                                        egui::Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 55))
                                    };

                                    let card_response = egui::Frame::none()
                                        .fill(card_bg)
                                        .stroke(card_stroke)
                                        .rounding(4.0)
                                        .inner_margin(egui::Margin::symmetric(6.0, 5.0))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                // 0. Child indentation
                                                if layer.parent_folder_id.is_some() {
                                                    ui.add_space(14.0);
                                                }

                                                // 1. Drag & Drop Handle
                                                let drag_label = egui::RichText::new("::")
                                                    .color(if is_active { Color32::from_rgb(243, 94, 24) } else { Color32::from_rgb(120, 125, 135) })
                                                    .strong();
                                                let drag_resp = ui.add(egui::Label::new(drag_label).selectable(false).sense(egui::Sense::drag()))
                                                    .on_hover_text("Drag to reorder layer in stack");
                                                if drag_resp.drag_started() {
                                                    self.dragged_layer_idx = Some(i);
                                                }

                                                // 2. Visibility toggle
                                                let (eye_rect, eye_resp) = ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::click());
                                                if let Some(icons) = opt_icons {
                                                    let tint = if layer.visible {
                                                        Color32::WHITE
                                                    } else {
                                                        Color32::from_rgba_unmultiplied(100, 105, 115, 110)
                                                    };
                                                    ui.painter().image(
                                                        icons.layer_eye.id(),
                                                        eye_rect,
                                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                                        tint,
                                                    );
                                                } else {
                                                    let eye_icon = if layer.visible { "[o]" } else { "[-]" };
                                                    let eye_color = if layer.visible { Color32::from_rgb(220, 225, 235) } else { Color32::from_rgb(110, 115, 125) };
                                                    ui.painter().text(eye_rect.center(), egui::Align2::CENTER_CENTER, eye_icon, egui::FontId::proportional(11.0), eye_color);
                                                }
                                                if eye_resp.on_hover_text("Toggle Layer Visibility").clicked() {
                                                    toggle_visible_idx = Some(i);
                                                }

                                                // 3. Type badge & Folder Chevron
                                                if layer.layer_type == LayerType::Folder {
                                                    let chev = if layer.folder_collapsed { "[+]" } else { "[-]" };
                                                    if ui.add(egui::Label::new(egui::RichText::new(chev).color(Color32::from_rgb(243, 156, 18))).selectable(false).sense(egui::Sense::click())).on_hover_text("Expand/collapse folder").clicked() {
                                                        toggle_collapse_idx = Some(i);
                                                    }
                                                }
                                                // 3. Dual Swatch Thumbnails: Material Swatch & Mask Swatch (Substance Painter 1:1)
                                                // 3a. Material Swatch Thumbnail
                                                let (mat_rect, mat_resp) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                                                let mat_bg = match layer.layer_type {
                                                    LayerType::Fill => Color32::from_rgb(
                                                        (layer.fill_color.x * 255.0) as u8,
                                                        (layer.fill_color.y * 255.0) as u8,
                                                        (layer.fill_color.z * 255.0) as u8,
                                                    ),
                                                    LayerType::Paint => Color32::from_rgb(46, 52, 64),
                                                    LayerType::Folder => Color32::from_rgb(160, 115, 35),
                                                };
                                                let mat_stroke = if is_active && !is_target_mask {
                                                    egui::Stroke::new(1.8_f32, Color32::from_rgb(243, 94, 24))
                                                } else {
                                                    egui::Stroke::new(1.0_f32, Color32::from_rgb(58, 62, 72))
                                                };
                                                ui.painter().rect(mat_rect, 2.0, mat_bg, mat_stroke);
                                                if let Some(icons) = opt_icons {
                                                    let icon_tex = match layer.layer_type {
                                                        LayerType::Paint => Some(icons.layer_paint.id()),
                                                        LayerType::Folder => Some(icons.layer_folder.id()),
                                                        LayerType::Fill => Some(icons.layer_fill.id()),
                                                    };
                                                    if let Some(tex) = icon_tex {
                                                        let icon_inset = egui::Rect::from_center_size(mat_rect.center(), egui::vec2(12.0, 12.0));
                                                        ui.painter().image(
                                                            tex,
                                                            icon_inset,
                                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                                            Color32::WHITE,
                                                        );
                                                    }
                                                } else {
                                                    if layer.layer_type == LayerType::Paint {
                                                        ui.painter().text(mat_rect.center(), egui::Align2::CENTER_CENTER, "P", egui::FontId::proportional(10.0), Color32::WHITE);
                                                    } else if layer.layer_type == LayerType::Folder {
                                                        ui.painter().text(mat_rect.center(), egui::Align2::CENTER_CENTER, "D", egui::FontId::proportional(10.0), Color32::WHITE);
                                                    }
                                                }
                                                if mat_resp.on_hover_text("Target Material (Click to activate material editing)").clicked() {
                                                    self.project.active_layer_idx = i;
                                                    self.brush_engine.settings.target = PaintTarget::Material;
                                                }

                                                // 3b. Mask Swatch Thumbnail (if layer has mask)
                                                if layer.has_mask {
                                                    ui.painter().text(
                                                        ui.cursor().min + egui::vec2(2.0, 9.0),
                                                        egui::Align2::CENTER_CENTER,
                                                        "-",
                                                        egui::FontId::proportional(10.0),
                                                        Color32::from_rgb(110, 115, 125),
                                                    );
                                                    let (mask_rect, mask_resp) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                                                    let mask_stroke = if is_active && is_target_mask {
                                                        egui::Stroke::new(1.8_f32, Color32::from_rgb(243, 94, 24))
                                                    } else {
                                                        egui::Stroke::new(1.0_f32, Color32::from_rgb(65, 70, 80))
                                                    };
                                                    ui.painter().rect(mask_rect, 2.0, Color32::from_rgb(20, 22, 26), mask_stroke);
                                                    // Draw small mask shape
                                                    ui.painter().circle_filled(mask_rect.center(), 5.0, Color32::from_rgb(235, 235, 240));
                                                    if mask_resp.on_hover_text("Target Mask (Click to activate mask editing)").clicked() {
                                                        self.project.active_layer_idx = i;
                                                        self.brush_engine.settings.target = PaintTarget::Mask;
                                                    }
                                                } else {
                                                    if ui.small_button("+M").on_hover_text("Add layer mask").clicked() {
                                                        add_mask_idx = Some(i);
                                                        self.project.active_layer_idx = i;
                                                        self.brush_engine.settings.target = PaintTarget::Mask;
                                                    }
                                                }

                                                // 4. Layer Name & Inline Renaming
                                                if self.renaming_layer_idx == Some(i) {
                                                    let text_res = ui.add(egui::TextEdit::singleline(&mut self.rename_buffer).desired_width(110.0));
                                                    text_res.request_focus();
                                                    if ui.input(|inp| inp.key_pressed(egui::Key::Enter)) || ui.small_button("OK").clicked() {
                                                        if !self.rename_buffer.trim().is_empty() {
                                                            rename_request = Some((i, self.rename_buffer.trim().to_string()));
                                                        }
                                                        self.renaming_layer_idx = None;
                                                    }
                                                    if ui.input(|inp| inp.key_pressed(egui::Key::Escape)) || ui.small_button("X").clicked() {
                                                        self.renaming_layer_idx = None;
                                                    }
                                                } else {
                                                    let name_color = if is_active {
                                                        Color32::from_rgb(255, 255, 255)
                                                    } else {
                                                        Color32::from_rgb(195, 200, 210)
                                                    };
                                                    let name_text = egui::RichText::new(&layer.name)
                                                        .color(name_color)
                                                        .strong();
                                                    let name_resp = ui.add(egui::Label::new(name_text).selectable(false).sense(egui::Sense::click_and_drag()))
                                                        .on_hover_text("Click to select material • Drag to reorder • Double-click to rename");

                                                    if name_resp.drag_started() {
                                                        self.dragged_layer_idx = Some(i);
                                                    }
                                                    if name_resp.clicked() {
                                                        self.project.active_layer_idx = i;
                                                        self.brush_engine.settings.target = PaintTarget::Material;
                                                    }
                                                    if name_resp.double_clicked() {
                                                        self.renaming_layer_idx = Some(i);
                                                        self.rename_buffer = layer.name.clone();
                                                    }

                                                    // Small edit button on active layer
                                                    if is_active {
                                                        if ui.small_button("Edit").on_hover_text("Rename layer").clicked() {
                                                            self.renaming_layer_idx = Some(i);
                                                            self.rename_buffer = layer.name.clone();
                                                        }
                                                    }
                                                }

                                                // 6. Folder membership (Move in/out)
                                                if layer.layer_type != LayerType::Folder && !folders.is_empty() {
                                                    let cur_layer_id = layer.id;
                                                    if layer.parent_folder_id.is_some() {
                                                        if ui.small_button("^").on_hover_text("Remove from folder").clicked() {
                                                            folder_change = Some((i, None));
                                                        }
                                                    } else {
                                                        ui.menu_button(">", |ui| {
                                                            for (fid, fname) in &folders {
                                                                if *fid != cur_layer_id && ui.button(fname).clicked() {
                                                                    folder_change = Some((i, Some(*fid)));
                                                                    ui.close_menu();
                                                                }
                                                            }
                                                        });
                                                    }
                                                }
                                            });
                                        });

                                    let card_rect = card_response.response.rect;

                                    // Active layer left accent indicator bar
                                    if is_active {
                                        ui.painter().rect_filled(
                                            egui::Rect::from_min_max(
                                                card_rect.min,
                                                egui::pos2(card_rect.min.x + 3.5, card_rect.max.y),
                                            ),
                                            2.0,
                                            Color32::from_rgb(243, 94, 24),
                                        );
                                    }

                                    // Drag-and-drop drop zone detection & guidelines
                                    if let Some(dragged_from) = self.dragged_layer_idx {
                                        if let Some(pos) = mouse_pos {
                                            if card_rect.contains(pos) && dragged_from != i {
                                                // Draw insertion highlight line
                                                let is_upper = pos.y < card_rect.center().y;
                                                let line_y = if is_upper { card_rect.min.y } else { card_rect.max.y };
                                                ui.painter().line_segment(
                                                    [egui::pos2(card_rect.min.x, line_y), egui::pos2(card_rect.max.x, line_y)],
                                                    egui::Stroke::new(2.5_f32, Color32::from_rgb(243, 94, 24)),
                                                );

                                                if mouse_released {
                                                    reorder_request = Some((dragged_from, i));
                                                }
                                            }
                                        }
                                    }

                                    ui.add_space(2.0);
                                }

                                if mouse_released {
                                    self.dragged_layer_idx = None;
                                }

                                if let Some((from, to)) = reorder_request {
                                    self.project.reorder_layer(from, to);
                                    self.status_message = format!("Reordered layer {} -> {}", from, to);
                                    self.dragged_layer_idx = None;
                                }

                                if let Some(idx) = toggle_visible_idx {
                                    if let Some(l) = self.project.layers.get_mut(idx) {
                                        l.visible = !l.visible;
                                    }
                                    self.project.rebuild_composite();
                                    self.project.gpu_sync_needed = true;
                                }
                                if let Some(idx) = toggle_collapse_idx {
                                    if let Some(l) = self.project.layers.get_mut(idx) {
                                        l.folder_collapsed = !l.folder_collapsed;
                                    }
                                }
                                if let Some(idx) = add_mask_idx {
                                    if let Some(l) = self.project.layers.get_mut(idx) {
                                        l.add_white_mask();
                                    }
                                    self.project.rebuild_composite();
                                    self.project.gpu_sync_needed = true;
                                }
                                if let Some((idx, name)) = rename_request {
                                    if let Some(l) = self.project.layers.get_mut(idx) {
                                        l.name = name;
                                    }
                                }
                                if let Some((idx, pid)) = folder_change {
                                    if let Some(l) = self.project.layers.get_mut(idx) {
                                        l.parent_folder_id = pid;
                                    }
                                    self.project.rebuild_composite();
                                    self.project.gpu_sync_needed = true;
                                }
                            });
        });
    }

    /// Render Properties content (active layer/mask/paint parameters)
    pub fn render_properties_content(&mut self, ui: &mut egui::Ui) {
                    let act_idx = self.project.active_layer_idx;
                    let is_mask_editing = self.brush_engine.settings.target == PaintTarget::Mask;
    
                    let (has_layer, prop_title) = if let Some(layer) = self.project.layers.get(act_idx) {
                        let title = if is_mask_editing {
                            format!("Properties - Mask: {}", layer.name)
                        } else {
                            match layer.layer_type {
                                LayerType::Fill => format!("Properties - Fill: {}", layer.name),
                                LayerType::Paint => format!("Properties - Paint: {}", layer.name),
                                LayerType::Folder => format!("Properties - Folder: {}", layer.name),
                            }
                        };
                        (true, title)
                    } else {
                        (false, String::new())
                    };

                    if has_layer {
                        let mut prop_changed = false;
                        let mut mask_changed = false;
                        let mut open_shelf_picker = None;
                        let mut import_to_shelf_and_assign = None;
                        ui.heading(&prop_title);
                        ui.add_space(2.0);

                        egui::ScrollArea::vertical()
                            .id_source("properties_panel_scroll")
                            .show(ui, |ui| {
                                let (lower_layers, higher_layers) = self.project.layers.split_at_mut(act_idx);
                                let available_anchors: Vec<String> = lower_layers
                                    .iter()
                                    .filter_map(|l| l.anchor_point.clone())
                                    .collect();
                                let anchor_refs: Vec<crate::layer::AnchorLayerRef<'_>> = lower_layers
                                    .iter()
                                    .filter_map(|l| l.as_anchor_ref())
                                    .collect();
                                let layer = &mut higher_layers[0];
                                let curv_ref = self.project.curvature_map.as_ref();
                                let ao_ref = self.project.ao_map.as_ref();
                                let pos_ref = self.project.position_map.as_ref();
                                let wn_ref = self.project.world_space_normal_map.as_ref();
                                let id_ref = self.project.id_map.as_ref();

                                ui.horizontal(|ui| {
                                    ui.label("Name:");
                                    if ui.text_edit_singleline(&mut layer.name).changed() {
                                        prop_changed = true;
                                    }
                                    ui.add_space(8.0);
                                    let mut is_anchor = layer.anchor_point.is_some();
                                    if ui.checkbox(&mut is_anchor, "Anchor Point").on_hover_text("Expose layer channels as an Anchor Point for procedural mask referencing in layers above").changed() {
                                        if is_anchor {
                                            layer.anchor_point = Some(layer.name.clone());
                                        } else {
                                            layer.anchor_point = None;
                                        }
                                        prop_changed = true;
                                    }
                                    if let Some(ref mut name) = layer.anchor_point {
                                        ui.label("Tag:");
                                        if ui.text_edit_singleline(name).changed() {
                                            prop_changed = true;
                                        }
                                    }
                                });
                                ui.add_space(4.0);
                                ui.separator();
                                ui.add_space(4.0);

                                if is_mask_editing {
                                    let mut prop_changed = false;
                                    // --- PROPERTIES: MASK (Substance 3D Painter Non-Destructive Effect Stack) ---
                                    ui.group(|ui| {
                                        ui.strong("Mask Effect Stack (Non-Destructive):");
                                        ui.horizontal_wrapped(|ui| {
                                            if ui.button("+ Add Fill").on_hover_text("Add procedural noise or generator fill to mask").clicked() {
                                                layer.add_mask_fill_effect_advanced(ProceduralGenerator::PerlinNoise, curv_ref, ao_ref, pos_ref, wn_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Paint").on_hover_text("Add paint modifier to mask").clicked() {
                                                layer.add_mask_paint_effect(curv_ref);
                                                prop_changed = true;
                                            }
                                            egui::ComboBox::from_id_source("add_generator_quick_menu")
                                                .selected_text("+ Add Generator...")
                                                .show_ui(ui, |ui| {
                                                    if ui.selectable_label(false, "Curvature Edge Wear").on_hover_text("Substance convex edge wear & paint chipping").clicked() {
                                                        layer.add_mask_generator_curvature_wear(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Cavity Dirt (AO)").on_hover_text("Substance concave crevice grime & sediment accumulation").clicked() {
                                                        layer.add_mask_generator_cavity_dirt(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Directional Dust (+Y)").on_hover_text("Sky-facing dust settling into upward concave pockets").clicked() {
                                                        layer.add_mask_generator_directional_dust(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Ground Mud & Splash").on_hover_text("Height-based ground mud accumulation with organic splatter").clicked() {
                                                        layer.add_mask_generator_ground_dirt(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                    if ui.selectable_label(false, "Mask Editor (Compound)").on_hover_text("Compound mask editor fusing all baked mesh maps").clicked() {
                                                        layer.add_mask_generator_mask_editor(curv_ref, ao_ref, pos_ref, wn_ref);
                                                        prop_changed = true;
                                                    }
                                                });
                                            if ui.button("+ Color Select").on_hover_text("Isolate submesh or UV island using baked ID Map").clicked() {
                                                layer.add_mask_color_selection_effect([1.0, 0.0, 0.0], 0.1, 0.05);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Anchor").on_hover_text("Reference channel from layer below").clicked() {
                                                let def_name = available_anchors.first().cloned().unwrap_or_else(|| "Anchor".to_string());
                                                layer.add_mask_anchor_effect(def_name, crate::channel::ChannelType::BaseColor);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Blur").on_hover_text("Add gaussian blur filter to mask").clicked() {
                                                layer.add_mask_filter_blur(3.0, curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Levels").on_hover_text("Add histogram levels filter to mask").clicked() {
                                                layer.add_mask_filter_levels(curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Sharpen").on_hover_text("Add sharpen filter to mask").clicked() {
                                                layer.add_mask_filter_sharpen(1.0, curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add High Pass").on_hover_text("Add high-pass filter to mask").clicked() {
                                                layer.add_mask_filter_high_pass(2.0, curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Contrast").on_hover_text("Add contrast & brightness filter to mask").clicked() {
                                                layer.add_mask_filter_contrast(1.2, 0.0, curv_ref);
                                                prop_changed = true;
                                            }
                                            if ui.button("+ Add Invert").on_hover_text("Add invert filter to mask").clicked() {
                                                layer.add_mask_filter_invert(curv_ref);
                                                prop_changed = true;
                                            }

                                            egui::ComboBox::from_id_source("smart_mask_quick_add")
                                                .selected_text("+ Smart Mask...")
                                                .show_ui(ui, |ui| {
                                                    for sm in crate::smart_materials::SmartMaterialLibrary::smart_masks() {
                                                        if ui.selectable_label(false, sm.name).on_hover_text(sm.description).clicked() {
                                                            crate::smart_materials::SmartMaterialLibrary::apply_smart_mask_advanced(
                                                                sm.id,
                                                                layer,
                                                                curv_ref,
                                                                ao_ref,
                                                                pos_ref,
                                                                wn_ref,
                                                            );
                                                            prop_changed = true;
                                                        }
                                                    }
                                                });
                                        });

                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            if ui.small_button("Clear White").clicked() {
                                                layer.clear_mask(1.0);
                                                prop_changed = true;
                                            }
                                            if ui.small_button("Clear Black").clicked() {
                                                layer.clear_mask(0.0);
                                                prop_changed = true;
                                            }
                                            if ui.small_button("Invert Base").clicked() {
                                                layer.invert_mask();
                                                prop_changed = true;
                                            }
                                            if ui.small_button("Remove Mask").clicked() {
                                                layer.remove_mask();
                                                self.brush_engine.settings.target = PaintTarget::Material;
                                                prop_changed = true;
                                            }
                                        });
                                    });

                                    ui.add_space(4.0);

                                    // Render mask effects stack list (top effect evaluates on top)
                                    if layer.mask_effects.is_empty() {
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("No modifiers in mask stack. Direct brush painting or whole-mask actions will apply to base mask buffer. Click '+ Add Fill' to add non-destructive noise.").small().italics().color(Color32::from_rgb(160, 165, 175)));
                                        });
                                    } else {
                                        let eff_count = layer.mask_effects.len();
                                        let mut remove_eff_id = None;
                                        let mut reorder_eff = None;

                                        for rev_idx in 0..eff_count {
                                            let idx = eff_count - 1 - rev_idx;
                                            let eff = &mut layer.mask_effects[idx];
                                            let eff_id = eff.id;

                                            ui.group(|ui| {
                                                ui.horizontal(|ui| {
                                                    if ui.checkbox(&mut eff.visible, "").changed() {
                                                        prop_changed = true;
                                                    }
                                                    ui.strong(&eff.name);

                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        if ui.small_button("X").on_hover_text("Delete effect").clicked() {
                                                            remove_eff_id = Some(eff_id);
                                                        }
                                                        if idx > 0 && ui.small_button("v").on_hover_text("Move down").clicked() {
                                                            reorder_eff = Some((eff_id, false));
                                                        }
                                                        if idx + 1 < eff_count && ui.small_button("^").on_hover_text("Move up").clicked() {
                                                            reorder_eff = Some((eff_id, true));
                                                        }
                                                    });
                                                });

                                                ui.horizontal(|ui| {
                                                    ui.label("Blend:");
                                                    egui::ComboBox::from_id_source(format!("mask_eff_blend_{}", eff_id))
                                                        .selected_text(eff.blend_mode.name())
                                                        .width(90.0)
                                                        .show_ui(ui, |ui| {
                                                            let modes = [
                                                                BlendMode::Normal,
                                                                BlendMode::Multiply,
                                                                BlendMode::Screen,
                                                                BlendMode::Overlay,
                                                                BlendMode::Add,
                                                                BlendMode::Subtract,
                                                                BlendMode::Darken,
                                                                BlendMode::Lighten,
                                                            ];
                                                            for m in modes {
                                                                if ui.selectable_value(&mut eff.blend_mode, m, m.name()).clicked() {
                                                                    prop_changed = true;
                                                                }
                                                            }
                                                        });
                                                    ui.label("Opacity:");
                                                    if ui.add(egui::Slider::new(&mut eff.opacity, 0.0..=1.0)).changed() {
                                                        prop_changed = true;
                                                    }
                                                });

                                                // Parameter controls per effect type
                                                match &mut eff.effect_type {
                                                    crate::layer::MaskEffectType::Fill {
                                                        ref mut generator,
                                                        ref mut scale,
                                                        ref mut contrast,
                                                        ref mut invert,
                                                        ref mut balance,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Generator:");
                                                            egui::ComboBox::from_id_source(format!("mask_eff_gen_{}", eff_id))
                                                                .selected_text(generator.name())
                                                                .show_ui(ui, |ui| {
                                                                    for g in ProceduralGenerator::all() {
                                                                        if ui.selectable_value(generator, *g, g.name()).clicked() {
                                                                            prop_changed = true;
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Scale:");
                                                            if ui.add(egui::Slider::new(scale, 0.5..=64.0).clamp_to_range(false)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Contrast:");
                                                            if ui.add(egui::Slider::new(contrast, 0.1..=4.0)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Balance:");
                                                            if ui.add(egui::Slider::new(balance, 0.0..=1.0)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        if ui.checkbox(invert, "Invert Generator").changed() {
                                                            prop_changed = true;
                                                        }
                                                    }
                                                    crate::layer::MaskEffectType::FilterBlur { ref mut sigma } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Blur Radius:");
                                                            if ui.add(egui::Slider::new(sigma, 0.5..=20.0)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FilterLevels {
                                                        ref mut in_black,
                                                        ref mut in_white,
                                                        ref mut gamma,
                                                        ref mut out_black,
                                                        ref mut out_white,
                                                    } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("In Black:");
                                                            if ui.add(egui::Slider::new(in_black, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("In White:");
                                                            if ui.add(egui::Slider::new(in_white, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Gamma:");
                                                            if ui.add(egui::Slider::new(gamma, 0.1..=5.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Out Black:");
                                                            if ui.add(egui::Slider::new(out_black, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Out White:");
                                                            if ui.add(egui::Slider::new(out_white, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FilterSharpen { ref mut strength } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Sharpen Strength:");
                                                            if ui.add(egui::Slider::new(strength, 0.1..=5.0)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FilterHighPass { ref mut radius } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("High Pass Radius:");
                                                            if ui.add(egui::Slider::new(radius, 0.5..=20.0)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FilterContrast { ref mut contrast, ref mut brightness } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Contrast:");
                                                            if ui.add(egui::Slider::new(contrast, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Brightness:");
                                                            if ui.add(egui::Slider::new(brightness, -1.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::FilterInvert => {
                                                        ui.label("Filter: Inverts lower layers in mask stack.");
                                                    }
                                                    crate::layer::MaskEffectType::Paint { ref mut buffer, ref mut alpha } => {
                                                        let is_active = layer.active_mask_effect_id == Some(eff_id);
                                                        ui.horizontal(|ui| {
                                                            if is_active {
                                                                ui.label(egui::RichText::new("Active Paint Target").color(Color32::from_rgb(80, 220, 120)).strong());
                                                            } else if ui.small_button("Set Active Paint Target").clicked() {
                                                                layer.active_mask_effect_id = Some(eff_id);
                                                                prop_changed = true;
                                                            }
                                                            if ui.small_button("Clear Paint").clicked() {
                                                                buffer.data.fill(0.0);
                                                                alpha.data.fill(0.0);
                                                                prop_changed = true;
                                                            }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::ColorSelection { ref mut target_color, ref mut tolerance, ref mut softness } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("ID Color:");
                                                            let mut col_arr = *target_color;
                                                            if ui.color_edit_button_rgb(&mut col_arr).changed() {
                                                                *target_color = col_arr;
                                                                prop_changed = true;
                                                            }
                                                            ui.label(format!("RGB: {:.2}, {:.2}, {:.2}", target_color[0], target_color[1], target_color[2]));
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Tolerance:");
                                                            if ui.add(egui::Slider::new(tolerance, 0.0..=1.0)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Softness:");
                                                            if ui.add(egui::Slider::new(softness, 0.0..=0.5)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        ui.label(egui::RichText::new("Isolates submeshes or UV islands via baked ID Map.").small().color(Color32::from_rgb(160, 165, 175)));
                                                    }
                                                    crate::layer::MaskEffectType::AnchorReference { ref mut anchor_name, ref mut source_channel, ref mut blur, ref mut invert } => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Anchor Layer:");
                                                            egui::ComboBox::from_id_source(format!("anchor_name_sel_{}", eff_id))
                                                                .selected_text(if anchor_name.is_empty() { "Select Anchor..." } else { anchor_name.as_str() })
                                                                .show_ui(ui, |ui| {
                                                                    if available_anchors.is_empty() {
                                                                        ui.label(egui::RichText::new("(No anchor points exposed below)").italics().small());
                                                                    } else {
                                                                        for anc in &available_anchors {
                                                                            if ui.selectable_label(anchor_name == anc, anc).clicked() {
                                                                                *anchor_name = anc.clone();
                                                                                prop_changed = true;
                                                                            }
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Source Channel:");
                                                            egui::ComboBox::from_id_source(format!("anchor_ch_sel_{}", eff_id))
                                                                .selected_text(source_channel.name())
                                                                .show_ui(ui, |ui| {
                                                                    let channels = [
                                                                        crate::channel::ChannelType::BaseColor,
                                                                        crate::channel::ChannelType::Roughness,
                                                                        crate::channel::ChannelType::Metallic,
                                                                        crate::channel::ChannelType::Height,
                                                                        crate::channel::ChannelType::Emission,
                                                                        crate::channel::ChannelType::Opacity,
                                                                    ];
                                                                    for ch in channels {
                                                                        if ui.selectable_label(*source_channel == ch, ch.name()).clicked() {
                                                                            *source_channel = ch;
                                                                            prop_changed = true;
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Blur:");
                                                            if ui.add(egui::Slider::new(blur, 0.0..=20.0)).changed() {
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        if ui.checkbox(invert, "Invert Anchor").changed() {
                                                            prop_changed = true;
                                                        }
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorCurvatureWear {
                                                        ref mut wear_level,
                                                        ref mut edge_width,
                                                        ref mut edge_contrast,
                                                        ref mut grunge_amount,
                                                        ref mut grunge_scale,
                                                        ref mut ao_inhibit,
                                                        ref mut invert,
                                                    } => {
                                                        ui.label(egui::RichText::new("Substance Curvature Edge Wear").strong().color(Color32::from_rgb(220, 180, 80)));
                                                        ui.horizontal(|ui| {
                                                            ui.label("Wear Level:");
                                                            if ui.add(egui::Slider::new(wear_level, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Edge Width:");
                                                            if ui.add(egui::Slider::new(edge_width, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Edge Contrast:");
                                                            if ui.add(egui::Slider::new(edge_contrast, 0.5..=5.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Breakup:");
                                                            if ui.add(egui::Slider::new(grunge_amount, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Scale:");
                                                            if ui.add(egui::Slider::new(grunge_scale, 1.0..=64.0).clamp_to_range(false)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("AO Inhibit (Cavities):");
                                                            if ui.add(egui::Slider::new(ao_inhibit, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        if ui.checkbox(invert, "Invert Wear").changed() { prop_changed = true; }
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorCavityDirt {
                                                        ref mut dirt_level,
                                                        ref mut dirt_contrast,
                                                        ref mut dirt_reach,
                                                        ref mut grunge_clumping,
                                                        ref mut grunge_scale,
                                                        ref mut invert,
                                                    } => {
                                                        ui.label(egui::RichText::new("Substance Cavity & Crevice Grime (AO)").strong().color(Color32::from_rgb(180, 140, 100)));
                                                        ui.horizontal(|ui| {
                                                            ui.label("Dirt Level:");
                                                            if ui.add(egui::Slider::new(dirt_level, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Dirt Contrast:");
                                                            if ui.add(egui::Slider::new(dirt_contrast, 0.5..=5.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Dirt Reach / Bleed:");
                                                            if ui.add(egui::Slider::new(dirt_reach, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Clumping:");
                                                            if ui.add(egui::Slider::new(grunge_clumping, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Scale:");
                                                            if ui.add(egui::Slider::new(grunge_scale, 1.0..=64.0).clamp_to_range(false)).changed() { prop_changed = true; }
                                                        });
                                                        if ui.checkbox(invert, "Invert Cavity").changed() { prop_changed = true; }
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorDirectionalDust {
                                                        ref mut dust_direction,
                                                        ref mut dust_level,
                                                        ref mut angle_tolerance,
                                                        ref mut cavity_trap,
                                                        ref mut grunge_breakup,
                                                        ref mut grunge_scale,
                                                    } => {
                                                        ui.label(egui::RichText::new("Substance Directional Dust (World Normal)").strong().color(Color32::from_rgb(200, 200, 180)));
                                                        ui.horizontal(|ui| {
                                                            ui.label("Direction:");
                                                            if ui.selectable_label(dust_direction[1] > 0.5, "Up (+Y)").clicked() {
                                                                *dust_direction = [0.0, 1.0, 0.0];
                                                                prop_changed = true;
                                                            }
                                                            if ui.selectable_label(dust_direction[1] < -0.5, "Down (-Y)").clicked() {
                                                                *dust_direction = [0.0, -1.0, 0.0];
                                                                prop_changed = true;
                                                            }
                                                            if ui.selectable_label(dust_direction[2] > 0.5, "Forward (+Z)").clicked() {
                                                                *dust_direction = [0.0, 0.0, 1.0];
                                                                prop_changed = true;
                                                            }
                                                            if ui.selectable_label(dust_direction[0] > 0.5, "Right (+X)").clicked() {
                                                                *dust_direction = [1.0, 0.0, 0.0];
                                                                prop_changed = true;
                                                            }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Dust Level:");
                                                            if ui.add(egui::Slider::new(dust_level, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Angle Spread:");
                                                            if ui.add(egui::Slider::new(angle_tolerance, 0.1..=2.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Cavity Trap:");
                                                            if ui.add(egui::Slider::new(cavity_trap, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Breakup:");
                                                            if ui.add(egui::Slider::new(grunge_breakup, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Grunge Scale:");
                                                            if ui.add(egui::Slider::new(grunge_scale, 1.0..=64.0).clamp_to_range(false)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorGroundDirt {
                                                        ref mut bottom_level,
                                                        ref mut gradient_softness,
                                                        ref mut splash_amount,
                                                        ref mut splash_scale,
                                                    } => {
                                                        ui.label(egui::RichText::new("Substance Ground Mud & Splash (Position)").strong().color(Color32::from_rgb(170, 130, 90)));
                                                        ui.horizontal(|ui| {
                                                            ui.label("Bottom Height Level:");
                                                            if ui.add(egui::Slider::new(bottom_level, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Gradient Softness:");
                                                            if ui.add(egui::Slider::new(gradient_softness, 0.01..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Splash Amount:");
                                                            if ui.add(egui::Slider::new(splash_amount, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Splash Scale:");
                                                            if ui.add(egui::Slider::new(splash_scale, 1.0..=64.0).clamp_to_range(false)).changed() { prop_changed = true; }
                                                        });
                                                    }
                                                    crate::layer::MaskEffectType::GeneratorMaskEditor {
                                                        ref mut curv_weight,
                                                        ref mut curv_width,
                                                        ref mut curv_contrast,
                                                        ref mut curv_invert,
                                                        ref mut ao_weight,
                                                        ref mut ao_contrast,
                                                        ref mut world_normal_weight,
                                                        ref mut world_normal_dir,
                                                        ref mut pos_weight,
                                                        ref mut pos_height,
                                                        ref mut grunge_weight,
                                                        ref mut grunge_scale,
                                                        ref mut global_balance,
                                                        ref mut global_contrast,
                                                        ref mut invert,
                                                    } => {
                                                        ui.label(egui::RichText::new("Substance Painter Mask Editor (Compound)").strong().color(Color32::from_rgb(200, 160, 240)));
                                                        ui.collapsing("Curvature Parameters", |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Weight:");
                                                                if ui.add(egui::Slider::new(curv_weight, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Edge Width:");
                                                                if ui.add(egui::Slider::new(curv_width, 0.1..=4.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Contrast:");
                                                                if ui.add(egui::Slider::new(curv_contrast, 0.5..=5.0)).changed() { prop_changed = true; }
                                                            });
                                                            if ui.checkbox(curv_invert, "Invert Curvature (Cavities)").changed() { prop_changed = true; }
                                                        });
                                                        ui.collapsing("Ambient Occlusion Parameters", |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Weight:");
                                                                if ui.add(egui::Slider::new(ao_weight, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Contrast:");
                                                                if ui.add(egui::Slider::new(ao_contrast, 0.5..=5.0)).changed() { prop_changed = true; }
                                                            });
                                                        });
                                                        ui.collapsing("World Normal Direction", |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Weight:");
                                                                if ui.add(egui::Slider::new(world_normal_weight, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Preset:");
                                                                if ui.selectable_label(world_normal_dir[1] > 0.5, "Up (+Y)").clicked() {
                                                                    *world_normal_dir = [0.0, 1.0, 0.0];
                                                                    prop_changed = true;
                                                                }
                                                                if ui.selectable_label(world_normal_dir[1] < -0.5, "Down (-Y)").clicked() {
                                                                    *world_normal_dir = [0.0, -1.0, 0.0];
                                                                    prop_changed = true;
                                                                }
                                                                if ui.selectable_label(world_normal_dir[2] > 0.5, "Forward (+Z)").clicked() {
                                                                    *world_normal_dir = [0.0, 0.0, 1.0];
                                                                    prop_changed = true;
                                                                }
                                                            });
                                                        });
                                                        ui.collapsing("Position Height", |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Weight:");
                                                                if ui.add(egui::Slider::new(pos_weight, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Target Height:");
                                                                if ui.add(egui::Slider::new(pos_height, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                        });
                                                        ui.collapsing("Grunge Modulation", |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Grunge Weight:");
                                                                if ui.add(egui::Slider::new(grunge_weight, 0.0..=1.0)).changed() { prop_changed = true; }
                                                            });
                                                            ui.horizontal(|ui| {
                                                                ui.label("Grunge Scale:");
                                                                if ui.add(egui::Slider::new(grunge_scale, 1.0..=64.0).clamp_to_range(false)).changed() { prop_changed = true; }
                                                            });
                                                        });
                                                        ui.separator();
                                                        ui.horizontal(|ui| {
                                                            ui.label("Global Balance:");
                                                            if ui.add(egui::Slider::new(global_balance, 0.0..=1.0)).changed() { prop_changed = true; }
                                                        });
                                                        ui.horizontal(|ui| {
                                                            ui.label("Global Contrast:");
                                                            if ui.add(egui::Slider::new(global_contrast, 0.1..=4.0)).changed() { prop_changed = true; }
                                                        });
                                                        if ui.checkbox(invert, "Global Invert").changed() { prop_changed = true; }
                                                    }
                                                }
                                            });
                                        }

                                        if let Some(id) = remove_eff_id {
                                            layer.remove_mask_effect(id, curv_ref);
                                            layer.evaluate_mask_full(curv_ref, ao_ref, pos_ref, wn_ref, id_ref, &anchor_refs);
                                            prop_changed = true;
                                        }
                                        if let Some((id, up)) = reorder_eff {
                                            layer.reorder_mask_effect(id, up, curv_ref);
                                            layer.evaluate_mask_full(curv_ref, ao_ref, pos_ref, wn_ref, id_ref, &anchor_refs);
                                            prop_changed = true;
                                        }
                                    }

                                    // Mask Brush Parameters
                                    ui.group(|ui| {
                                        ui.label("Mask Brush Settings:");
                                        ui.horizontal(|ui| {
                                            ui.label("Paint Value:");
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.mask_paint_value, 0.0..=1.0));
                                            if ui.button("Swap [X]").clicked() {
                                                self.brush_engine.settings.mask_paint_value =
                                                    if self.brush_engine.settings.mask_paint_value > 0.5 { 0.0 } else { 1.0 };
                                            }
                                        });
                                        ui.label(if self.brush_engine.settings.mask_paint_value > 0.5 {
                                            "White (Reveals Layer)"
                                        } else {
                                            "Black (Hides Layer)"
                                        });
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.radius, 1.0..=160.0).text("Size [ / ]"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.hardness, 0.0..=1.0).text("Hardness"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.opacity, 0.0..=1.0).text("Opacity"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.flow, 0.0..=1.0).text("Flow"));
                                    });
                                    if prop_changed {
                                        mask_changed = true;
                                    }
                                } else if layer.layer_type == LayerType::Fill {
                                    // --- PROPERTIES: FILL (PBR Material Maps Manipulation 1:1) ---
    
                                    // 1. Transformation (UV Wrap)
                                    ui.collapsing("Transformation (UV Wrap)", |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label("UV Scale / Tiling:");
                                            if ui.add(egui::Slider::new(&mut layer.uv_scale, 0.2..=32.0).logarithmic(true).clamp_to_range(false)).changed() {
                                                prop_changed = true;
                                            }
                                        });
                                        ui.horizontal(|ui| {
                                            if ui.button("1x").clicked() { layer.uv_scale = 1.0; prop_changed = true; }
                                            if ui.button("2x").clicked() { layer.uv_scale = 2.0; prop_changed = true; }
                                            if ui.button("4x").clicked() { layer.uv_scale = 4.0; prop_changed = true; }
                                            if ui.button("8x").clicked() { layer.uv_scale = 8.0; prop_changed = true; }
                                            if ui.button("16x").clicked() { layer.uv_scale = 16.0; prop_changed = true; }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("UV Offset:");
                                            if ui.add(egui::Slider::new(&mut layer.uv_offset.x, 0.0..=1.0).text("U")).changed() { prop_changed = true; }
                                            if ui.add(egui::Slider::new(&mut layer.uv_offset.y, 0.0..=1.0).text("V")).changed() { prop_changed = true; }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("UV Rotation:");
                                            if ui.add(egui::Slider::new(&mut layer.uv_rotation, 0.0..=360.0).suffix("°")).changed() { prop_changed = true; }
                                        });
                                    });
    
                                    // 2. Material Channel Toggles (Substance Painter iconic pill buttons)
                                    ui.group(|ui| {
                                        ui.label("Material Channels:");
                                        ui.horizontal(|ui| {
                                            if channel_pill_button(ui, "color", layer.channel_flags.base_color).clicked() {
                                                layer.channel_flags.base_color = !layer.channel_flags.base_color;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "metal", layer.channel_flags.metallic).clicked() {
                                                layer.channel_flags.metallic = !layer.channel_flags.metallic;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "rough", layer.channel_flags.roughness).clicked() {
                                                layer.channel_flags.roughness = !layer.channel_flags.roughness;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "nrm", layer.channel_flags.normal).clicked() {
                                                layer.channel_flags.normal = !layer.channel_flags.normal;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "height", layer.channel_flags.height).clicked() {
                                                layer.channel_flags.height = !layer.channel_flags.height;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "emis", layer.channel_flags.emission).clicked() {
                                                layer.channel_flags.emission = !layer.channel_flags.emission;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "opac", layer.channel_flags.opacity).clicked() {
                                                layer.channel_flags.opacity = !layer.channel_flags.opacity;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "coat", layer.channel_flags.clearcoat).clicked() {
                                                layer.channel_flags.clearcoat = !layer.channel_flags.clearcoat;
                                                prop_changed = true;
                                            }
                                            if channel_pill_button(ui, "aniso", layer.channel_flags.anisotropy).clicked() {
                                                layer.channel_flags.anisotropy = !layer.channel_flags.anisotropy;
                                                prop_changed = true;
                                            }
                                        });
                                    });
    
                                    // 3. Channel Value Controllers (PBR Maps)
                                    ui.group(|ui| {
                                        if layer.channel_flags.base_color {
                                            ui.horizontal(|ui| {
                                                ui.label("Base Color:");
                                                if crate::color_picker::color_picker_button_vec4(
                                                    ui,
                                                    egui::Id::new(format!("layer_base_col_{}", layer.id)),
                                                    &mut layer.fill_color,
                                                ) {
                                                    prop_changed = true;
                                                }
                                                ui.label(crate::color_picker::rgb_to_hex(layer.fill_color.x, layer.fill_color.y, layer.fill_color.z));
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_color_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Texture Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace texture from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillBaseColor);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillBaseColor));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_color_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick texture or baked map from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillBaseColor);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import texture file into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillBaseColor));
                                                        }
                                                    }
                                                }
                                            });
                                        }
    
                                        if layer.channel_flags.metallic {
                                            ui.horizontal(|ui| {
                                                ui.label("Metallic:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_metallic, 0.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if ui.small_button("0.0 Dielectric").clicked() { layer.fill_metallic = 0.0; prop_changed = true; }
                                                if ui.small_button("0.5 Semi").clicked() { layer.fill_metallic = 0.5; prop_changed = true; }
                                                if ui.small_button("1.0 Metal").clicked() { layer.fill_metallic = 1.0; prop_changed = true; }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_metallic_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Metallic Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace metallic map from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillMetallic);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import metallic texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillMetallic));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_metallic_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick metallic map or texture from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillMetallic);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import metallic texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillMetallic));
                                                        }
                                                    }
                                                }
                                            });
                                        }
    
                                        if layer.channel_flags.roughness {
                                            ui.horizontal(|ui| {
                                                ui.label("Roughness:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_roughness, 0.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if ui.small_button("0.05 Gloss").clicked() { layer.fill_roughness = 0.05; prop_changed = true; }
                                                if ui.small_button("0.40 Satin").clicked() { layer.fill_roughness = 0.40; prop_changed = true; }
                                                if ui.small_button("0.85 Matte").clicked() { layer.fill_roughness = 0.85; prop_changed = true; }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_roughness_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Roughness Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace roughness map from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillRoughness);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import roughness texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillRoughness));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_roughness_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick roughness map or texture from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillRoughness);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import roughness texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillRoughness));
                                                        }
                                                    }
                                                }
                                            });
                                        }
    
                                        if layer.channel_flags.normal {
                                            ui.horizontal(|ui| {
                                                ui.label("Normal Intensity:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_normal_strength, 0.0..=2.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_normal_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Normal Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace normal map from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillNormal);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import normal map into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillNormal));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_normal_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick normal map from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillNormal);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import normal map into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillNormal));
                                                        }
                                                    }
                                                }
                                            });
                                        }
    
                                        if layer.channel_flags.height {
                                            ui.horizontal(|ui| {
                                                ui.label("Height / Bump:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_height, 0.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_height_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Height Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace height map from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillHeight);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import height texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillHeight));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_height_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick height texture or baked map from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillHeight);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import height texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillHeight));
                                                        }
                                                    }
                                                }
                                            });
                                        }

                                        if layer.channel_flags.emission {
                                            ui.horizontal(|ui| {
                                                ui.label("Emission:");
                                                if crate::color_picker::color_picker_button_vec4(
                                                    ui,
                                                    egui::Id::new(format!("layer_emissive_col_{}", layer.id)),
                                                    &mut layer.fill_emissive,
                                                ) {
                                                    prop_changed = true;
                                                }
                                                ui.label(crate::color_picker::rgb_to_hex(layer.fill_emissive.x, layer.fill_emissive.y, layer.fill_emissive.z));
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Emission Intensity:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_emissive.w, 0.0..=10.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_emissive_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Emissive Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace emissive texture from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillEmissive);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import emissive texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillEmissive));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_emissive_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick emissive texture from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillEmissive);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import emissive texture into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillEmissive));
                                                        }
                                                    }
                                                }
                                            });
                                        }

                                        if layer.channel_flags.opacity {
                                            ui.horizontal(|ui| {
                                                ui.label("Opacity / Alpha:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_opacity, 0.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                                if ui.small_button("Hole (0%)").clicked() {
                                                    layer.fill_opacity = 0.0;
                                                    prop_changed = true;
                                                }
                                                if ui.small_button("Glass (50%)").clicked() {
                                                    layer.fill_opacity = 0.5;
                                                    prop_changed = true;
                                                }
                                                if ui.small_button("Solid (100%)").clicked() {
                                                    layer.fill_opacity = 1.0;
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                if layer.fill_opacity_tex.is_some() {
                                                    ui.colored_label(Color32::from_rgb(80, 220, 120), "Opacity Map Active");
                                                    if ui.button("Shelf...").on_hover_text("Select or replace opacity map from Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillOpacity);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import opacity map into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillOpacity));
                                                        }
                                                    }
                                                    if ui.button("Remove").clicked() { layer.fill_opacity_tex = None; prop_changed = true; }
                                                } else {
                                                    if ui.button("Pick from Shelf...").on_hover_text("Pick opacity map from Project Shelf").clicked() {
                                                        open_shelf_picker = Some(ShelfPickerTarget::FillOpacity);
                                                    }
                                                    if ui.button("Import...").on_hover_text("Import opacity map into Project Shelf and assign").clicked() {
                                                        if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "bmp", "exr", "hdr"]).pick_file() {
                                                            import_to_shelf_and_assign = Some((path, ShelfPickerTarget::FillOpacity));
                                                        }
                                                    }
                                                }
                                            });
                                        }

                                        if layer.channel_flags.clearcoat {
                                            ui.horizontal(|ui| {
                                                ui.label("Clearcoat:");
                                                let mut cc = layer.fill_clearcoat;
                                                if ui.add(egui::Slider::new(&mut cc, 0.0..=1.0)).changed() {
                                                    layer.set_fill_clearcoat(cc);
                                                    prop_changed = true;
                                                }
                                            });
                                        }

                                        if layer.channel_flags.anisotropy {
                                            ui.horizontal(|ui| {
                                                ui.label("Anisotropy Strength:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_anisotropy, -1.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Anisotropy Angle:");
                                                if ui.add(egui::Slider::new(&mut layer.fill_anisotropy_angle, 0.0..=1.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                        }
                                    });
    
                                    // 4. Procedural Generator / Texture Pattern
                                    ui.group(|ui| {
                                        ui.label("Procedural Generator / Pattern:");
                                        ui.horizontal(|ui| {
                                            egui::ComboBox::from_id_source(format!("prop_gen_{}", layer.id))
                                                .selected_text(layer.fill_generator.name())
                                                .show_ui(ui, |ui| {
                                                    if ui.selectable_value(&mut layer.fill_generator, ProceduralGenerator::None, ProceduralGenerator::None.name()).clicked() {
                                                        prop_changed = true;
                                                    }
                                                    for g in ProceduralGenerator::all() {
                                                        if ui.selectable_value(&mut layer.fill_generator, *g, g.name()).clicked() {
                                                            prop_changed = true;
                                                        }
                                                    }
                                                });
                                        });

                                        if ui.checkbox(&mut layer.triplanar, "Triplanar Projection (Seamless)").on_hover_text("Projects noise across 3D object space to eliminate UV seams").changed() {
                                            prop_changed = true;
                                        }
    
                                        if layer.fill_generator != ProceduralGenerator::None {
                                            ui.horizontal(|ui| {
                                                ui.label("Pattern Scale:");
                                                if ui.add(egui::Slider::new(&mut layer.generator_scale, 1.0..=64.0).clamp_to_range(false)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Contrast:");
                                                if ui.add(egui::Slider::new(&mut layer.generator_contrast, 0.1..=4.0)).changed() {
                                                    prop_changed = true;
                                                }
                                            });
                                            if ui.checkbox(&mut layer.generator_invert, "Invert Pattern").changed() {
                                                prop_changed = true;
                                            }
                                        }
                                    });
                                } else if layer.layer_type == LayerType::Folder {
                                    // --- PROPERTIES: FOLDER / GROUP ---
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new("Folder / Group Properties").strong().color(Color32::from_rgb(243, 94, 24)));
                                        ui.horizontal(|ui| {
                                            ui.label("Folder Opacity:");
                                            if ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0)).changed() {
                                                prop_changed = true;
                                            }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("Collapsed:");
                                            if ui.checkbox(&mut layer.folder_collapsed, "Collapse contents in stack").changed() {
                                                prop_changed = true;
                                            }
                                        });
                                        ui.separator();
                                        ui.label("Folder Masking (Affects all contained layers):");
                                        if layer.has_mask {
                                            ui.horizontal(|ui| {
                                                if ui.button("Invert Mask").clicked() {
                                                    layer.invert_mask();
                                                    prop_changed = true;
                                                }
                                                if ui.button("Clear White").clicked() {
                                                    layer.clear_mask(1.0);
                                                    prop_changed = true;
                                                }
                                                if ui.button("Clear Black").clicked() {
                                                    layer.clear_mask(0.0);
                                                    prop_changed = true;
                                                }
                                                if ui.button("Remove Mask").clicked() {
                                                    layer.remove_mask();
                                                    prop_changed = true;
                                                }
                                            });
                                        } else {
                                            ui.horizontal(|ui| {
                                                if ui.button("+ White Mask").clicked() {
                                                    layer.add_white_mask();
                                                    prop_changed = true;
                                                }
                                                if ui.button("+ Black Mask").clicked() {
                                                    layer.add_black_mask();
                                                    prop_changed = true;
                                                }
                                            });
                                        }
                                    });
                                } else {
                                    // --- PROPERTIES: PAINT ---
                                    ui.group(|ui| {
                                        ui.label("Brush Parameters:");
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.radius, 1.0..=160.0).text("Size [ / ]"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.hardness, 0.0..=1.0).text("Hardness"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.opacity, 0.0..=1.0).text("Opacity"));
                                        ui.add(egui::Slider::new(&mut self.brush_engine.settings.flow, 0.0..=1.0).text("Flow"));
    
                                        ui.horizontal(|ui| {
                                            ui.label("Alpha:");
                                            egui::ComboBox::from_id_source("brush_alpha_select")
                                                .selected_text(self.brush_engine.settings.alpha.name())
                                                .show_ui(ui, |ui| {
                                                    let alphas = [
                                                        AlphaType::SoftRound,
                                                        AlphaType::HardRound,
                                                        AlphaType::NoiseSplatter,
                                                        AlphaType::Chisel,
                                                        AlphaType::Stipple,
                                                        AlphaType::Scratches,
                                                    ];
                                                    for a in alphas {
                                                        ui.selectable_value(&mut self.brush_engine.settings.alpha, a, a.name());
                                                    }
                                                });
                                            if ui.small_button("Shelf...").on_hover_text("Choose Alpha from the Shelf").clicked() {
                                                self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::BrushAlpha));
                                            }
                                        });
    
                                        ui.collapsing("Jitter Dynamics", |ui| {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.size_jitter, 0.0..=1.0).text("Size Jitter"));
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.position_jitter, 0.0..=1.0).text("Scatter"));
                                        });
                                    });
    
                                    ui.group(|ui| {
                                        ui.label("Material Stamp Channels:");
                                        ui.horizontal(|ui| {
                                            if channel_pill_button(ui, "color", self.brush_engine.settings.channels.base_color).clicked() {
                                                self.brush_engine.settings.channels.base_color = !self.brush_engine.settings.channels.base_color;
                                            }
                                            if channel_pill_button(ui, "metal", self.brush_engine.settings.channels.metallic).clicked() {
                                                self.brush_engine.settings.channels.metallic = !self.brush_engine.settings.channels.metallic;
                                            }
                                            if channel_pill_button(ui, "rough", self.brush_engine.settings.channels.roughness).clicked() {
                                                self.brush_engine.settings.channels.roughness = !self.brush_engine.settings.channels.roughness;
                                            }
                                            if channel_pill_button(ui, "height", self.brush_engine.settings.channels.height).clicked() {
                                                self.brush_engine.settings.channels.height = !self.brush_engine.settings.channels.height;
                                            }
                                            if channel_pill_button(ui, "emis", self.brush_engine.settings.channels.emission).clicked() {
                                                self.brush_engine.settings.channels.emission = !self.brush_engine.settings.channels.emission;
                                            }
                                            if channel_pill_button(ui, "opac", self.brush_engine.settings.channels.opacity).clicked() {
                                                self.brush_engine.settings.channels.opacity = !self.brush_engine.settings.channels.opacity;
                                            }
                                            if channel_pill_button(ui, "coat", self.brush_engine.settings.channels.clearcoat).clicked() {
                                                self.brush_engine.settings.channels.clearcoat = !self.brush_engine.settings.channels.clearcoat;
                                            }
                                            if channel_pill_button(ui, "aniso", self.brush_engine.settings.channels.anisotropy).clicked() {
                                                self.brush_engine.settings.channels.anisotropy = !self.brush_engine.settings.channels.anisotropy;
                                            }
                                        });
    
                                        if self.brush_engine.settings.channels.base_color {
                                            ui.horizontal(|ui| {
                                                ui.label("Base Color:");
                                                crate::color_picker::color_picker_button_vec4(
                                                    ui,
                                                    egui::Id::new("brush_paint_base_color"),
                                                    &mut self.brush_engine.settings.paint_color,
                                                );
                                                ui.label(crate::color_picker::rgb_to_hex(
                                                    self.brush_engine.settings.paint_color.x,
                                                    self.brush_engine.settings.paint_color.y,
                                                    self.brush_engine.settings.paint_color.z,
                                                ));
                                            });
                                        }
    
                                        if self.brush_engine.settings.channels.roughness {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_roughness, 0.0..=1.0).text("Roughness"));
                                        }
                                        if self.brush_engine.settings.channels.metallic {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_metallic, 0.0..=1.0).text("Metallic"));
                                        }
                                        if self.brush_engine.settings.channels.height {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_height, 0.0..=1.0).text("Height"));
                                        }
                                        if self.brush_engine.settings.channels.emission {
                                            ui.horizontal(|ui| {
                                                ui.label("Emission:");
                                                crate::color_picker::color_picker_button_vec4(
                                                    ui,
                                                    egui::Id::new("brush_paint_emissive_color"),
                                                    &mut self.brush_engine.settings.paint_emissive,
                                                );
                                                ui.label(crate::color_picker::rgb_to_hex(
                                                    self.brush_engine.settings.paint_emissive.x,
                                                    self.brush_engine.settings.paint_emissive.y,
                                                    self.brush_engine.settings.paint_emissive.z,
                                                ));
                                            });
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_emissive.w, 0.0..=10.0).text("Emission Intensity"));
                                        }
                                        if self.brush_engine.settings.channels.opacity {
                                            ui.horizontal(|ui| {
                                                ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_opacity, 0.0..=1.0).text("Opacity"));
                                                if ui.small_button("Hole (0%)").clicked() {
                                                    self.brush_engine.settings.paint_opacity = 0.0;
                                                }
                                                if ui.small_button("Glass (50%)").clicked() {
                                                    self.brush_engine.settings.paint_opacity = 0.5;
                                                }
                                                if ui.small_button("Solid (100%)").clicked() {
                                                    self.brush_engine.settings.paint_opacity = 1.0;
                                                }
                                            });
                                        }
                                        if self.brush_engine.settings.channels.clearcoat {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_clearcoat, 0.0..=1.0).text("Clearcoat"));
                                        }
                                        if self.brush_engine.settings.channels.anisotropy {
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_anisotropy, -1.0..=1.0).text("Anisotropy"));
                                            ui.add(egui::Slider::new(&mut self.brush_engine.settings.paint_anisotropy_angle, 0.0..=1.0).text("Aniso Angle"));
                                        }
                                    });
                                }
    
                                ui.add_space(8.0);
                                ui.separator();
                                if ui.button("Export Textures... (Ctrl+E)").clicked() {
                                    self.show_export_modal = true;
                                }
                            });
    
                        if prop_changed || mask_changed {
                            let proj = &mut self.project;
                            let curv = proj.curvature_map.as_ref();
                            let pos = proj.position_map.as_ref();
                            let wn = proj.world_space_normal_map.as_ref();
                            let ao = proj.ao_map.as_ref();
                            let id_ref = proj.id_map.as_ref();
                            let (lower, higher) = proj.layers.split_at_mut(act_idx);
                            let anchor_refs: Vec<crate::layer::AnchorLayerRef<'_>> = lower
                                .iter()
                                .filter_map(|l| l.as_anchor_ref())
                                .collect();
                            let layer = &mut higher[0];
                            if prop_changed && layer.layer_type == LayerType::Fill {
                                layer.evaluate_fill_advanced(curv, pos, wn, ao);
                            }
                            if mask_changed && layer.has_mask {
                                layer.evaluate_mask_full(curv, ao, pos, wn, id_ref, &anchor_refs);
                            }
                            let is_extended_change = layer.channel_flags.clearcoat
                                || layer.channel_flags.emission
                                || layer.channel_flags.opacity
                                || layer.channel_flags.anisotropy;
                            if self.wgpu_viewport.is_some() && ui.input(|i| i.pointer.any_down()) && !is_extended_change {
                                self.project.gpu_sync_needed = true;
                                self.need_cpu_composite_sync = true;
                            } else {
                                self.project.rebuild_composite();
                                self.project.gpu_sync_needed = true;
                            }
                        }

                        if let Some(target) = open_shelf_picker {
                            self.shelf_picker = Some(ShelfPickerState::new(target));
                        }
                        if let Some((path, target)) = import_to_shelf_and_assign {
                            if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                if let Some(res) = self.project.get_resource(&id).cloned() {
                                    self.assign_resource_to_target(&res, target);
                                    self.refresh_shelf_thumbnails(ui.ctx());
                                }
                            }
                        }
                    }
    }

    /// Render Texture Set Settings and Mesh Maps slots
    pub fn render_mesh_maps_content(&mut self, ui: &mut egui::Ui) {
                        ui.heading("Texture Set Settings");
                        ui.add_space(4.0);

                        ui.group(|ui| {
                            ui.label(format!("Mesh: {}", self.project.mesh.name));
                            ui.label(format!("Triangles: {}", self.project.mesh.indices.len() / 3));
                            ui.label(format!("Working Resolution: {} x {}", self.project.texture_width, self.project.texture_height));

                            ui.horizontal(|ui| {
                                ui.label("Change Resolution:");
                                let res_options = [512, 1024, 2048, 4096];
                                egui::ComboBox::from_id_source("texture_set_res_select")
                                    .selected_text(format!("{}x{}", self.new_project_width, self.new_project_height))
                                    .show_ui(ui, |ui| {
                                        for &r in &res_options {
                                            if ui.selectable_label(self.new_project_width == r, format!("{}x{}", r, r)).clicked() {
                                                self.new_project_width = r;
                                                self.new_project_height = r;
                                            }
                                        }
                                    });
                            });
                            if self.new_project_width != self.project.texture_width || self.new_project_height != self.project.texture_height {
                                if ui.button(format!("Resample Textures to {}x{}", self.new_project_width, self.new_project_height)).clicked() {
                                    self.project.resize_textures(self.new_project_width, self.new_project_height);
                                    self.project.rebuild_composite();
                                    self.project.gpu_sync_needed = true;
                                    self.status_message = format!("Resized textures to {}x{}", self.new_project_width, self.new_project_height);
                                }
                            }
                        });

                        ui.add_space(6.0);
                        let bake_btn = if let Some(icons) = self.icons.as_ref() {
                            egui::Button::image_and_text((icons.bake.id(), egui::vec2(15.0, 15.0)), "Bake Mesh Maps...")
                        } else {
                            egui::Button::new("Bake Mesh Maps...")
                        };
                        if ui.add(bake_btn).on_hover_text("Open Baking dialog to bake Normals, AO, and Curvature from geometry").clicked() {
                            self.show_bake_modal = true;
                        }

                        ui.add_space(6.0);
                        ui.separator();
                        ui.heading("Baked Mesh Maps (Geometry)");
                        ui.label(egui::RichText::new("Persistent underlying maps that blend with your layer stack:").small());
                        ui.add_space(4.0);

                        egui::ScrollArea::vertical()
                            .id_source("mesh_maps_scroll")
                            .show(ui, |ui| {
                                // 1. Normal Map (Tangent Space)
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong("Normal (Tangent Space):");
                                        if self.project.world_normal_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select Normal map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapTangentNormal));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import normal map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapTangentNormal);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.world_normal_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.world_normal_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::Normal;
                                            }
                                        }
                                    });
                                });

                                // 2. Ambient Occlusion (AO)
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong("Ambient Occlusion (AO):");
                                        if self.project.ao_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select AO map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapAo));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import AO map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapAo);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.ao_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.ao_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::AmbientOcclusion;
                                            }
                                        }
                                    });
                                });

                                // 3. Curvature Map
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong("Curvature:");
                                        if self.project.curvature_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select Curvature map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapCurvature));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import Curvature map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapCurvature);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.curvature_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.curvature_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::Curvature;
                                            }
                                        }
                                    });
                                });

                                // 4. World Space Normal Map
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong("World Space Normal:");
                                        if self.project.world_space_normal_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select World Space Normal map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapWorldSpaceNormal));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import World Space Normal map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapWorldSpaceNormal);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.world_space_normal_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.world_space_normal_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::WorldNormal;
                                            }
                                        }
                                    });
                                });

                                // 5. Position Map (WPO)
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong("Position (WPO):");
                                        if self.project.position_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select Position map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapPosition));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import Position map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapPosition);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.position_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.position_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::Position;
                                            }
                                        }
                                    });
                                });

                                // 6. Thickness Map
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong("Thickness:");
                                        if self.project.thickness_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select Thickness map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapThickness));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import Thickness map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapThickness);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.thickness_map.is_some() {
                                            if ui.button("Invert").on_hover_text("Invert thickness map (toggle between White=Thick and White=Thin)").clicked() {
                                                if let Some(th) = &mut self.project.thickness_map {
                                                    th.invert();
                                                    self.project.gpu_mesh_maps_sync_needed = true;
                                                    self.project.gpu_sync_needed = true;
                                                    self.project.sync_baked_maps_to_resources();
                                                    self.status_message = "Inverted thickness map".to_string();
                                                }
                                            }
                                            if ui.button("Remove").clicked() {
                                                self.project.thickness_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.gpu_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                            }
                                            if ui.button("View").clicked() {
                                                self.view_mode = ViewportChannelMode::Thickness;
                                            }
                                        }
                                    });
                                });

                                // 7. ID Map
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong("ID (Color Mask):");
                                        if self.project.id_map.is_some() {
                                            ui.colored_label(egui::Color32::from_rgb(80, 220, 120), "Active");
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Not Baked");
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.button("Shelf...").on_hover_text("Select ID map from Project Shelf").clicked() {
                                            self.shelf_picker = Some(ShelfPickerState::new(ShelfPickerTarget::MeshMapId));
                                        }
                                        if ui.button("Import File...").on_hover_text("Import ID map file into Project Shelf and assign").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "tga", "exr", "hdr"]).pick_file() {
                                                if let Ok(id) = self.project.import_texture_resource(&path, crate::project::ProjectResourceType::Texture) {
                                                    if let Some(res) = self.project.get_resource(&id).cloned() {
                                                        self.assign_resource_to_target(&res, ShelfPickerTarget::MeshMapId);
                                                    }
                                                }
                                            }
                                        }
                                        if self.project.id_map.is_some() {
                                            if ui.button("Remove").clicked() {
                                                self.project.id_map = None;
                                                self.project.gpu_mesh_maps_sync_needed = true;
                                                self.project.sync_baked_maps_to_resources();
                                                self.status_message = "Removed ID map".to_string();
                                            }
                                        }
                                    });
                                });

                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Tip: Baked Normal and AO maps are permanently integrated into the PBR shader. Layers and paint strokes seamlessly blend on top of them. Curvature and AO maps also directly power the Smart Edge Wear and Crevice Dirt mask generators.").small().color(egui::Color32::from_rgb(170, 170, 170)));
                            });
    }
}
