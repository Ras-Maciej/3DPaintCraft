use eframe::egui_wgpu;
use eframe::wgpu;
use eframe::wgpu::util::DeviceExt;
use glam::{Mat4, Vec4};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaderPerformancePreset {
    UltraQuality,       // Bilateral SSAO, FXAA, Bloom, ACES/AgX
    DynamicInteraction, // Downsample & optimize SSAO during interaction, crisp on idle
    BalancedPbr,        // Lightweight SSAO, high performance 60+ FPS
    MaximumPerformance, // Ultra-fast: Post-FX disabled, instant 60+ FPS
}

impl ShaderPerformancePreset {
    pub fn name(&self) -> &'static str {
        match self {
            Self::UltraQuality => "Ultra Quality (Bilateral SSAO, ACES/AgX)",
            Self::DynamicInteraction => "Dynamic Interaction (Fast Drag / Crisp Idle)",
            Self::BalancedPbr => "Balanced PBR (Fast Viewport, 60 FPS)",
            Self::MaximumPerformance => "Maximum Performance (Ultra-Fast 60+ FPS)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TransparencyMode {
    Dithered = 0,    // Screen-Door / Bayer 8x8 Dithering (Order Independent Semi-Transparency)
    AlphaCutout = 1, // Substance 3D Painter Alpha Cutout (Alpha Test)
    AlphaBlend = 2,  // Smooth Alpha Blending (Translucent Glass)
    Refraction = 3,  // Physical Refraction & Translucency (Snell's Law IOR)
    Opaque = 4,      // Opaque (Ignore Opacity)
}

impl TransparencyMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Dithered => "Dithered Transparency (Screen-Door)",
            Self::AlphaCutout => "Alpha Cutout (Alpha Test)",
            Self::AlphaBlend => "Alpha Blending (Translucent Glass)",
            Self::Refraction => "Physical Refraction / Translucency (IOR Glass & Gems)",
            Self::Opaque => "Opaque (Ignore Opacity)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderSettings {
    pub sun_intensity: f32,
    pub env_intensity: f32,
    pub ao_intensity: f32,
    pub exposure: f32,
    pub normal_format: u32, // 0 = OpenGL (+Y), 1 = DirectX (-Y)
    pub tonemap_mode: u32,  // 0 = Reinhard, 1 = ACES Filmic, 2 = AgX, 3 = Linear
    pub specular_mult: f32,
    pub roughness_bias: f32,

    // Opacity / Transparency Settings
    pub transparency_mode: TransparencyMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    pub ior: f32,
    pub refraction_dispersion: f32,
    pub translucency_depth: f32,

    // Anti-Aliasing (Off / FXAA / NVIDIA DLAA)
    pub aa_mode: crate::nvidia_dlaa::AntiAliasingMode,
    pub fxaa_enabled: bool,

    // Screen-Space Joint Bilateral Normal/Depth Guided Denoiser
    pub denoiser_enabled: bool,
    pub denoiser_radius: f32,
    pub denoiser_normal_weight: f32,
    pub denoiser_depth_weight: f32,
    pub history_blend_weight: f32,

    // Bloom / Specular Highlight Glow
    pub bloom_enabled: bool,
    pub bloom_intensity: f32,
    pub bloom_threshold: f32,
    pub bloom_radius: f32,

    // Depth of Field (DoF)
    pub dof_enabled: bool,
    pub dof_focus_distance: f32,
    pub dof_aperture: f32,
    pub dof_max_blur: f32,

    // Camera Lens Effects
    pub chromatic_aberration: f32,
    pub vignette_strength: f32,

    // Screen-Space Reflections (Deprecated / Off)
    pub ssr_enabled: bool,
    pub ssr_intensity: f32,
    pub ssr_max_roughness: f32,
    pub ssr_step_size: f32,
    pub ssr_max_steps: u32,

    // Screen-Space Ambient Occlusion (SSAO)
    pub ssao_enabled: bool,
    pub ssao_radius: f32,
    pub ssao_intensity: f32,

    // Real-Time Directional Shadows
    pub shadows_enabled: bool,
    pub shadow_bias: f32,
    pub shadow_softness: f32,

    // Wireframe Overlay
    pub wireframe_color: [f32; 4],
    pub wireframe_opacity: f32,

    // Interactive Viewport Performance / Adaptive Resolution
    pub adaptive_resolution: bool,
    pub adaptive_scale: f32,
}

impl ShaderSettings {
    pub fn apply_preset(&mut self, preset: ShaderPerformancePreset) {
        match preset {
            ShaderPerformancePreset::UltraQuality => {
                self.ssr_enabled = false;
                self.ssao_enabled = true;
                self.ssao_intensity = 0.60;
                self.ssao_radius = 0.40;
                self.shadows_enabled = true;
                self.shadow_bias = 0.012;
                self.shadow_softness = 1.2;
                self.aa_mode = crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa;
                self.fxaa_enabled = false;
                self.denoiser_enabled = false;
                self.denoiser_radius = 0.8;
                self.denoiser_normal_weight = 24.0;
                self.denoiser_depth_weight = 0.04;
                self.history_blend_weight = 0.90;
                self.adaptive_resolution = false;
            }
            ShaderPerformancePreset::DynamicInteraction => {
                self.ssr_enabled = false;
                self.ssao_enabled = true;
                self.ssao_intensity = 0.50;
                self.ssao_radius = 0.35;
                self.shadows_enabled = true;
                self.shadow_bias = 0.015;
                self.shadow_softness = 1.0;
                self.aa_mode = crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa;
                self.fxaa_enabled = false;
                self.denoiser_enabled = false;
                self.denoiser_radius = 0.8;
                self.denoiser_normal_weight = 24.0;
                self.denoiser_depth_weight = 0.04;
                self.history_blend_weight = 0.90;
                self.adaptive_resolution = true;
                self.adaptive_scale = 0.5;
            }
            ShaderPerformancePreset::BalancedPbr => {
                self.ssr_enabled = false;
                self.ssao_enabled = true;
                self.ssao_intensity = 0.45;
                self.ssao_radius = 0.30;
                self.shadows_enabled = true;
                self.shadow_bias = 0.015;
                self.shadow_softness = 1.0;
                self.aa_mode = crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa;
                self.fxaa_enabled = false;
                self.denoiser_enabled = false;
                self.denoiser_radius = 0.8;
                self.denoiser_normal_weight = 24.0;
                self.denoiser_depth_weight = 0.04;
                self.history_blend_weight = 0.90;
                self.bloom_enabled = false;
                self.dof_enabled = false;
                self.adaptive_resolution = true;
                self.adaptive_scale = 0.75;
            }
            ShaderPerformancePreset::MaximumPerformance => {
                self.ssr_enabled = false;
                self.ssao_enabled = false;
                self.shadows_enabled = false;
                self.aa_mode = crate::nvidia_dlaa::AntiAliasingMode::Off;
                self.fxaa_enabled = false;
                self.denoiser_enabled = false;
                self.bloom_enabled = false;
                self.dof_enabled = false;
                self.adaptive_resolution = false;
            }
        }
    }
}

impl Default for ShaderSettings {
    fn default() -> Self {
        let mut settings = Self {
            sun_intensity: 1.5,
            env_intensity: 1.0,
            ao_intensity: 1.0,
            exposure: 1.0,
            normal_format: 0,
            tonemap_mode: 1, // ACES Filmic
            specular_mult: 1.0,
            roughness_bias: 0.0,
            transparency_mode: TransparencyMode::Dithered,
            alpha_cutoff: 0.33,
            double_sided: false,
            ior: 1.52,
            refraction_dispersion: 0.015,
            translucency_depth: 1.0,
            aa_mode: crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa,
            fxaa_enabled: true,
            denoiser_enabled: false,
            denoiser_radius: 0.8,
            denoiser_normal_weight: 24.0,
            denoiser_depth_weight: 0.04,
            history_blend_weight: 0.90,
            bloom_enabled: false,
            bloom_intensity: 0.8,
            bloom_threshold: 0.9,
            bloom_radius: 1.5,
            dof_enabled: false,
            dof_focus_distance: 2.5,
            dof_aperture: 1.2,
            dof_max_blur: 0.6,
            chromatic_aberration: 0.0,
            vignette_strength: 0.0,
            ssr_enabled: false,
            ssr_intensity: 1.0,
            ssr_max_roughness: 0.65,
            ssr_step_size: 0.06,
            ssr_max_steps: 24,
            ssao_enabled: true,
            ssao_radius: 0.35,
            ssao_intensity: 0.50,
            shadows_enabled: true,
            shadow_bias: 0.015,
            shadow_softness: 1.0,
            wireframe_color: [0.2, 0.75, 1.0, 1.0],
            wireframe_opacity: 0.85,
            adaptive_resolution: true,
            adaptive_scale: 0.5,
        };
        settings.apply_preset(ShaderPerformancePreset::DynamicInteraction);
        settings
    }
}

fn calculate_mesh_bounds(vertices: &[crate::mesh::Vertex]) -> (glam::Vec3, f32) {
    if vertices.is_empty() {
        return (glam::Vec3::ZERO, 1.0);
    }
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    for v in vertices {
        let p = glam::Vec3::from(v.position);
        min = min.min(p);
        max = max.max(p);
    }
    let center = (min + max) * 0.5;
    let radius = ((max - min) * 0.5).length().max(0.5);
    (center, radius)
}

#[derive(Debug)]
pub struct WgpuCageMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub wire_index_buffer: wgpu::Buffer,
    pub wire_index_count: u32,
    pub hull_index_buffer: wgpu::Buffer,
    pub hull_index_count: u32,
    pub show_hull: bool,
    pub color: [f32; 4],
    pub opacity: f32,
}

pub struct WgpuViewport {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    egui_renderer: Arc<eframe::egui::mutex::RwLock<egui_wgpu::Renderer>>,

    pub texture_id: eframe::egui::TextureId,
    color_texture: wgpu::Texture,
    depth_texture: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,

    scene_texture: wgpu::Texture,
    scene_view: wgpu::TextureView,
    scene_normal_texture: wgpu::Texture,
    scene_normal_view: wgpu::TextureView,
    tex_history: wgpu::Texture,
    tex_history_view: wgpu::TextureView,
    pub dlaa_manager: crate::nvidia_dlaa::NvidiaDlaaManager,
    postprocess_pipeline: wgpu::RenderPipeline,
    postprocess_buffer: wgpu::Buffer,
    postprocess_bind_group: wgpu::BindGroup,
    postprocess_bind_group_layout: wgpu::BindGroupLayout,

    width: u32,
    height: u32,

    pipeline: wgpu::RenderPipeline,
    pipeline_double_sided: wgpu::RenderPipeline,
    skybox_pipeline: wgpu::RenderPipeline,
    wireframe_pipeline: wgpu::RenderPipeline,
    cage_wire_pipeline: wgpu::RenderPipeline,
    cage_hull_pipeline: wgpu::RenderPipeline,
    cage_mesh: Option<WgpuCageMesh>,
    shadow_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    shadow_texture: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    #[allow(dead_code)]
    shadow_sampler: wgpu::Sampler,
    mesh_center: glam::Vec3,
    mesh_radius: f32,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    wire_index_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    material_bind_group: wgpu::BindGroup,
    shadow_bind_group: wgpu::BindGroup,
    
    tex_base_color: wgpu::Texture,
    tex_normal: wgpu::Texture,
    tex_roughness: wgpu::Texture,
    tex_metallic: wgpu::Texture,
    tex_baked_normal: wgpu::Texture,
    tex_baked_ao: wgpu::Texture,
    tex_environment: wgpu::Texture,
    tex_emissive: wgpu::Texture,
    tex_opacity: wgpu::Texture,
    tex_clearcoat: wgpu::Texture,
    tex_anisotropy: wgpu::Texture,
    
    material_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    
    pbr_tex_width: u32,
    pbr_tex_height: u32,
    baked_tex_width: u32,
    baked_tex_height: u32,
    
    pub gpu_compositor: crate::gpu_compositor::GpuCompositor,
    pub gpu_layers: Vec<crate::gpu_compositor::GpuLayer>,
    pub gpu_brush: crate::gpu_brush::GpuBrushEngine,
    pub submeshes: Vec<crate::mesh::SubMeshRange>,
    index_count: u32,
    wire_index_count: u32,
    pub adapter_info: Option<eframe::wgpu::AdapterInfo>,
    pub cmd_encoding_ms: f32,
    pub queue_submit_ms: f32,
    pub profiler: crate::gpu_profiler::GpuProfiler,
    aniso_staging: Vec<f32>,
}

impl WgpuViewport {
    pub fn new(
        render_state: &egui_wgpu::RenderState,
        mesh: &crate::mesh::Mesh,
    ) -> Self {
        let device = render_state.device.clone();
        let queue = render_state.queue.clone();
        let egui_renderer = render_state.renderer.clone();

        let width = 800;
        let height = 600;

        let (color_texture, color_view, texture_id) = Self::create_color_target(
            &device,
            &egui_renderer,
            width,
            height,
        );

        let (scene_texture, scene_view) = Self::create_scene_target(&device, width, height);
        let (scene_normal_texture, scene_normal_view) = Self::create_scene_normal_target(&device, width, height);
        let (depth_texture, depth_view) = Self::create_depth_target(&device, width, height);
        let (tex_history, tex_history_view) = Self::create_history_target(&device, width, height);
        let dlaa_manager = crate::nvidia_dlaa::NvidiaDlaaManager::new(Some(&render_state.adapter.get_info().name));
        let profiler = crate::gpu_profiler::GpuProfiler::new(&device, &queue, &render_state.adapter);

        let (mesh_center, mesh_radius) = calculate_mesh_bounds(&mesh.vertices);

        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Directional Shadow Map Texture"),
            size: wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Directional Shadow Map View"),
            ..Default::default()
        });
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Comparison Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Viewport Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("viewport.wgsl").into()),
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: 352,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
            ],
        });

        // Directional Shadow Bind Group Layout (Group 2 for Main PBR Shading)
        let shadow_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Bind Group"),
            layout: &shadow_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
            ],
        });

        // Material Bind Group Layout
        let material_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { // Base Color
                    binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Normal
                    binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Roughness
                    binding: 2, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Metallic
                    binding: 3, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Sampler
                    binding: 4, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // HDRI Environment
                    binding: 5, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Baked Normal
                    binding: 6, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Baked AO
                    binding: 7, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Emissive
                    binding: 8, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Opacity
                    binding: 9, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Clearcoat
                    binding: 10, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Anisotropy
                    binding: 11, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
            ],
        });

        let tex_size = wgpu::Extent3d { width: 512, height: 512, depth_or_array_layers: 1 };
        let tex_base_color = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex BC"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_normal = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex N"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_roughness = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex R"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_metallic = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex M"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_baked_normal = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex BN"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_baked_ao = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex BAO"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_emissive = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex Emis"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_opacity = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex Opac"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_clearcoat = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex Clearcoat"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_anisotropy = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex Aniso"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        
        let env_size = wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 };
        let tex_environment = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex Env"), size: env_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        
        // Write a gray placeholder environment
        queue.write_texture(
            wgpu::ImageCopyTexture { texture: &tex_environment, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            bytemuck::cast_slice(&[0.1f32, 0.1f32, 0.1f32, 1.0f32]),
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(16), rows_per_image: Some(1) },
            env_size,
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &material_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&tex_base_color.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&tex_normal.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&tex_roughness.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&tex_metallic.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&tex_environment.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&tex_baked_normal.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&tex_baked_ao.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&tex_emissive.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(&tex_opacity.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::TextureView(&tex_clearcoat.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 11, resource: wgpu::BindingResource::TextureView(&tex_anisotropy.create_view(&Default::default())) },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Viewport Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &material_bind_group_layout, &shadow_bind_group_layout],
            push_constant_ranges: &[],
        });

        let skybox_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skybox Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &material_bind_group_layout],
            push_constant_ranges: &[],
        });

        let wireframe_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Wireframe Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_layouts = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::mesh::GpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![
                0 => Float32x3, // pos
                1 => Float32x3, // norm
                2 => Float32x4, // tang
                3 => Float32x2, // uv
            ],
        }];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Viewport Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &vertex_layouts,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let pipeline_double_sided = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PBR Pipeline (Double-Sided / Cull Off)"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &vertex_layouts,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let wireframe_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Wireframe Pipeline"),
            layout: Some(&wireframe_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_wire",
                buffers: &vertex_layouts,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_wire",
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::empty(),
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -2,
                    slope_scale: -1.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let cage_wire_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cage Wire Pipeline"),
            layout: Some(&wireframe_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_wire",
                buffers: &vertex_layouts,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_cage_wire",
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::empty(),
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -3,
                    slope_scale: -1.5,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let cage_hull_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cage Hull Pipeline"),
            layout: Some(&wireframe_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_wire",
                buffers: &vertex_layouts,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_cage_hull",
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::empty(),
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -1,
                    slope_scale: -0.5,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let shadow_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Directional Shadow Pipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_shadow",
                buffers: &vertex_layouts,
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let skybox_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skybox Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("skybox.wgsl").into()),
        });

        let skybox_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox Pipeline"),
            layout: Some(&skybox_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &skybox_shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &skybox_shader,
                entry_point: "fs_main",
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING), // In case bg_opacity < 1.0
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always, // we draw it first
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let gpu_vertices: Vec<crate::mesh::GpuVertex> = mesh.vertices.iter().map(|v| v.into()).collect();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&gpu_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let wire_indices = mesh.generate_wireframe_indices();
        let wire_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Wire Index Buffer"),
            contents: bytemuck::cast_slice(&wire_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let wire_index_count = wire_indices.len() as u32;
        
        let gpu_compositor = crate::gpu_compositor::GpuCompositor::new(device.clone(), queue.clone(), 512, 512);

        let postprocess_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PostProcess Buffer"),
            size: 272,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let postprocess_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PostProcess Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let postprocess_bind_group = Self::create_postprocess_bind_group(
            &device,
            &postprocess_bind_group_layout,
            &postprocess_buffer,
            &sampler,
            &scene_view,
            &depth_view,
            &scene_normal_view,
            &tex_history_view,
        );

        let postprocess_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PostProcess Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("postprocess.wgsl").into()),
        });

        let postprocess_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PostProcess Pipeline Layout"),
            bind_group_layouts: &[&postprocess_bind_group_layout],
            push_constant_ranges: &[],
        });

        let postprocess_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PostProcess Pipeline"),
            layout: Some(&postprocess_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &postprocess_shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &postprocess_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let gpu_brush = crate::gpu_brush::GpuBrushEngine::new(device.clone(), queue.clone(), 512, 512);

        Self {
            device,
            queue,
            egui_renderer,
            texture_id,
            color_texture,
            color_view,
            depth_texture,
            depth_view,
            scene_texture,
            scene_view,
            scene_normal_texture,
            scene_normal_view,
            tex_history,
            tex_history_view,
            dlaa_manager,
            postprocess_pipeline,
            postprocess_buffer,
            postprocess_bind_group,
            postprocess_bind_group_layout,
            width,
            height,
            pipeline,
            pipeline_double_sided,
            skybox_pipeline,
            wireframe_pipeline,
            cage_wire_pipeline,
            cage_hull_pipeline,
            cage_mesh: None,
            shadow_pipeline,
            shadow_texture,
            shadow_view,
            shadow_sampler,
            mesh_center,
            mesh_radius,
            vertex_buffer,
            index_buffer,
            wire_index_buffer,
            camera_buffer,
            camera_bind_group,
            material_bind_group,
            shadow_bind_group,
            tex_base_color,
            tex_normal,
            tex_roughness,
            tex_metallic,
            tex_baked_normal,
            tex_baked_ao,
            tex_environment,
            tex_emissive,
            tex_opacity,
            tex_clearcoat,
            tex_anisotropy,
            material_bind_group_layout,
            sampler,
            pbr_tex_width: 512,
            pbr_tex_height: 512,
            baked_tex_width: 512,
            baked_tex_height: 512,
            gpu_compositor,
            gpu_layers: Vec::new(),
            gpu_brush,
            submeshes: mesh.submeshes.clone(),
            index_count: mesh.indices.len() as u32,
            wire_index_count,
            adapter_info: Some(render_state.adapter.get_info()),
            cmd_encoding_ms: 0.0,
            queue_submit_ms: 0.0,
            profiler,
            aniso_staging: Vec::new(),
        }
    }

    pub fn rebuild_material_bind_group(&mut self) {
        self.material_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group Reloaded"),
            layout: &self.material_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.tex_base_color.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.tex_normal.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.tex_roughness.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.tex_metallic.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.tex_environment.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&self.tex_baked_normal.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.tex_baked_ao.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&self.tex_emissive.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(&self.tex_opacity.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::TextureView(&self.tex_clearcoat.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 11, resource: wgpu::BindingResource::TextureView(&self.tex_anisotropy.create_view(&Default::default())) },
            ],
        });
    }

    pub fn ensure_pbr_textures(&mut self, w: u32, h: u32) {
        if self.pbr_tex_width == w && self.pbr_tex_height == h {
            return;
        }
        self.pbr_tex_width = w;
        self.pbr_tex_height = h;

        let tex_size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
        self.tex_base_color = self.device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex BC"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        self.tex_normal = self.device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex N"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        self.tex_roughness = self.device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex R"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        self.tex_metallic = self.device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex M"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        self.tex_emissive = self.device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex Emis"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        self.tex_opacity = self.device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex Opac"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        self.tex_clearcoat = self.device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex Clearcoat"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        self.tex_anisotropy = self.device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex Aniso"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });

        self.rebuild_material_bind_group();
        self.gpu_brush.resize(w, h);
    }


    /// Dispatch hardware-accelerated 3D projection stamping on GPU compute cores
    pub fn stamp_3d_gpu(
        &mut self,
        active_layer_idx: usize,
        uniforms: &crate::gpu_brush::GpuBrushUniform,
        triangles: &[crate::gpu_brush::GpuBrushTriangle],
    ) {
        if let Some(gpu_layer) = self.gpu_layers.get(active_layer_idx) {
            let t0 = std::time::Instant::now();
            self.gpu_brush.stamp_3d_gpu(gpu_layer, uniforms, triangles);
            let elapsed = t0.elapsed().as_secs_f32() * 1000.0;
            self.profiler.cpu_record_compute_brush(elapsed);
        }
    }

    /// Dispatch hardware-accelerated 2D UV stamping on GPU compute cores
    pub fn stamp_2d_gpu(
        &mut self,
        active_layer_idx: usize,
        uniforms: &crate::gpu_brush::GpuBrushUniform,
    ) {
        if let Some(gpu_layer) = self.gpu_layers.get(active_layer_idx) {
            let t0 = std::time::Instant::now();
            self.gpu_brush.stamp_3d_gpu(gpu_layer, uniforms, &[]);
            let elapsed = t0.elapsed().as_secs_f32() * 1000.0;
            self.profiler.cpu_record_compute_brush(elapsed);
        }
    }

    /// Read back modified rectangular region from GPU layer into CPU layer (synchronous)
    #[allow(dead_code)]
    pub fn readback_layer_rect(
        &self,
        active_layer_idx: usize,
        cpu_layer: &mut crate::layer::Layer,
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
        channels: crate::channel::ChannelFlags,
    ) {
        if let Some(gpu_layer) = self.gpu_layers.get(active_layer_idx) {
            self.gpu_brush.readback_layer_rect(gpu_layer, cpu_layer, min_x, min_y, max_x, max_y, channels);
        }
    }

    /// Check if an asynchronous staging readback is currently in flight
    pub fn has_pending_readback(&self) -> bool {
        self.gpu_brush.has_pending_readback()
    }

    /// Asynchronously initiate readback of a modified sub-rectangle from GPU layer to staging buffers
    pub fn start_async_readback(
        &self,
        active_layer_idx: usize,
        cpu_layer: &mut crate::layer::Layer,
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
        channels: crate::channel::ChannelFlags,
        description: &str,
    ) {
        if let Some(gpu_layer) = self.gpu_layers.get(active_layer_idx) {
            self.gpu_brush.start_async_readback(
                gpu_layer,
                cpu_layer,
                active_layer_idx,
                min_x,
                min_y,
                max_x,
                max_y,
                channels,
                description,
            );
        }
    }

    /// Non-blocking poll for completed asynchronous staging readback
    pub fn poll_async_readback(&self, project: &mut crate::project::Project) -> Option<crate::gpu_brush::ReadbackCompleted> {
        let layer_idx = self.gpu_brush.pending_readback_layer_idx()?;
        if let Some(cpu_layer) = project.layers.get_mut(layer_idx) {
            self.gpu_brush.poll_async_readback(cpu_layer)
        } else {
            None
        }
    }

    /// Flush and wait for pending asynchronous staging readback to complete immediately
    pub fn flush_async_readback(&self, project: &mut crate::project::Project) -> Option<crate::gpu_brush::ReadbackCompleted> {
        let layer_idx = self.gpu_brush.pending_readback_layer_idx()?;
        if let Some(cpu_layer) = project.layers.get_mut(layer_idx) {
            self.gpu_brush.flush_async_readback(cpu_layer)
        } else {
            None
        }
    }

    /// Composite all layers on GPU and update material textures in VRAM
    #[allow(dead_code)]
    pub fn composite_and_update(&mut self, project: &crate::project::Project) {
        let t0 = std::time::Instant::now();
        let final_target = self.gpu_compositor.composite_all(&project.layers, &self.gpu_layers);
        let size = wgpu::Extent3d { width: self.pbr_tex_width, height: self.pbr_tex_height, depth_or_array_layers: 1 };
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.base_color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_base_color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.roughness, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_roughness, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.metallic, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_metallic, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        self.queue.submit(std::iter::once(encoder.finish()));
        let elapsed = t0.elapsed().as_secs_f32() * 1000.0;
        self.profiler.cpu_record_compositing(elapsed);
    }

    /// Incremental GPU composite: reuses cached background DAG composite for active painting layer
    pub fn composite_and_update_active(&mut self, project: &crate::project::Project, active_idx: usize) {
        let t0 = std::time::Instant::now();
        let final_target = self.gpu_compositor.composite_incremental(&project.layers, &self.gpu_layers, active_idx);
        let size = wgpu::Extent3d { width: self.pbr_tex_width, height: self.pbr_tex_height, depth_or_array_layers: 1 };
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.base_color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_base_color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.roughness, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_roughness, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.metallic, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_metallic, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        self.queue.submit(std::iter::once(encoder.finish()));
        let elapsed = t0.elapsed().as_secs_f32() * 1000.0;
        self.profiler.cpu_record_compositing(elapsed);
    }

    /// Access latest GPU execution stage timings
    pub fn stage_timings(&self) -> crate::gpu_profiler::GpuStageTimings {
        self.profiler.smoothed_timings()
    }

    pub fn ensure_baked_textures(&mut self, w: u32, h: u32) {
        if self.baked_tex_width == w && self.baked_tex_height == h {
            return;
        }
        self.baked_tex_width = w;
        self.baked_tex_height = h;

        let tex_size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
        self.tex_baked_normal = self.device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex BN"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        self.tex_baked_ao = self.device.create_texture(&wgpu::TextureDescriptor { label: Some("Tex BAO"), size: tex_size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] });

        self.rebuild_material_bind_group();
    }

    pub fn sync_textures(&mut self, project: &mut crate::project::Project) {
        self.ensure_gpu_layers(project);

        let w = project.texture_width;
        let h = project.texture_height;
        let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };

        for (cpu_layer, gpu_layer) in project.layers.iter_mut().zip(self.gpu_layers.iter()) {
            if cpu_layer.dirty_rect.is_some() {
                self.gpu_compositor.update_layer(cpu_layer, gpu_layer, w, h);
            }
        }

        let final_target = self.gpu_compositor.composite_all(&project.layers, &self.gpu_layers);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.base_color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_base_color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.roughness, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_roughness, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &final_target.metallic, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &self.tex_metallic, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        if project.gpu_extended_channels_dirty {
            if project.composite_textures.emissive.width == w && project.composite_textures.emissive.height == h {
                self.queue.write_texture(
                    wgpu::ImageCopyTexture { texture: &self.tex_emissive, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                    bytemuck::cast_slice(&project.composite_textures.emissive.data),
                    wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(w * 16), rows_per_image: Some(h) },
                    size,
                );
            }
            if project.composite_textures.opacity.width == w && project.composite_textures.opacity.height == h {
                self.queue.write_texture(
                    wgpu::ImageCopyTexture { texture: &self.tex_opacity, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                    bytemuck::cast_slice(&project.composite_textures.opacity.data),
                    wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
                    size,
                );
            }
            if project.composite_textures.clearcoat.width == w && project.composite_textures.clearcoat.height == h {
                self.queue.write_texture(
                    wgpu::ImageCopyTexture { texture: &self.tex_clearcoat, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                    bytemuck::cast_slice(&project.composite_textures.clearcoat.data),
                    wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
                    size,
                );
            }
            if project.composite_textures.anisotropy.width == w && project.composite_textures.anisotropy.height == h {
                let total = (w * h) as usize;
                let req_len = total * 4;
                if self.aniso_staging.len() != req_len {
                    self.aniso_staging.resize(req_len, 0.0);
                }
                let aniso_data = &project.composite_textures.anisotropy.data;
                let angle_data = &project.composite_textures.anisotropy_angle.data;
                let has_aniso = aniso_data.iter().any(|&v| v > 0.001) || angle_data.iter().any(|&v| v > 0.001);
                if has_aniso {
                    for i in 0..total {
                        let a = if i < aniso_data.len() { aniso_data[i] } else { 0.0 };
                        let ang = if i < angle_data.len() { angle_data[i] } else { 0.0 };
                        self.aniso_staging[i * 4 + 0] = a;
                        self.aniso_staging[i * 4 + 1] = ang;
                        self.aniso_staging[i * 4 + 2] = 0.0;
                        self.aniso_staging[i * 4 + 3] = 1.0;
                    }
                } else {
                    self.aniso_staging.fill(0.0);
                }
                self.queue.write_texture(
                    wgpu::ImageCopyTexture { texture: &self.tex_anisotropy, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                    bytemuck::cast_slice(&self.aniso_staging),
                    wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(w * 16), rows_per_image: Some(h) },
                    size,
                );
            }
            project.gpu_extended_channels_dirty = false;
        }
    }

    /// Fast sub-region upload of extended PBR channels (emission, opacity, clearcoat, anisotropy)
    /// streaming only the bounding-box rectangle over PCIe to avoid full-screen texture re-uploads.
    pub fn sync_extended_dirty_rect(
        &mut self,
        layer: &crate::layer::Layer,
        rect: (u32, u32, u32, u32),
        channels: crate::channel::ChannelFlags,
    ) {
        let w = self.pbr_tex_width;
        let h = self.pbr_tex_height;
        if w == 0 || h == 0 {
            return;
        }

        let x0 = rect.0.min(w.saturating_sub(1));
        let y0 = rect.1.min(h.saturating_sub(1));
        let x1 = rect.2.min(w.saturating_sub(1)).max(x0);
        let y1 = rect.3.min(h.saturating_sub(1)).max(y0);

        let rect_w = x1 - x0 + 1;
        let rect_h = y1 - y0 + 1;
        if rect_w == 0 || rect_h == 0 {
            return;
        }

        let size = wgpu::Extent3d { width: rect_w, height: rect_h, depth_or_array_layers: 1 };
        let origin = wgpu::Origin3d { x: x0, y: y0, z: 0 };
        let byte_offset_16 = ((y0 * w + x0) as u64) * 16;
        let bytes_per_row_16 = w * 16;
        let byte_offset_4 = ((y0 * w + x0) as u64) * 4;
        let bytes_per_row_4 = w * 4;

        if channels.emission && !layer.emissive_buf.is_sparse() && layer.emissive_buf.width == w as u32 && layer.emissive_buf.height == h as u32 {
            self.queue.write_texture(
                wgpu::ImageCopyTexture { texture: &self.tex_emissive, mip_level: 0, origin, aspect: wgpu::TextureAspect::All },
                bytemuck::cast_slice(&layer.emissive_buf.data),
                wgpu::ImageDataLayout { offset: byte_offset_16, bytes_per_row: Some(bytes_per_row_16), rows_per_image: None },
                size,
            );
        }

        if channels.opacity && !layer.opacity_buf.is_sparse() && layer.opacity_buf.width == w as u32 && layer.opacity_buf.height == h as u32 {
            self.queue.write_texture(
                wgpu::ImageCopyTexture { texture: &self.tex_opacity, mip_level: 0, origin, aspect: wgpu::TextureAspect::All },
                bytemuck::cast_slice(&layer.opacity_buf.data),
                wgpu::ImageDataLayout { offset: byte_offset_4, bytes_per_row: Some(bytes_per_row_4), rows_per_image: None },
                size,
            );
        }

        if channels.clearcoat && !layer.clearcoat_buf.is_sparse() && layer.clearcoat_buf.width == w as u32 && layer.clearcoat_buf.height == h as u32 {
            self.queue.write_texture(
                wgpu::ImageCopyTexture { texture: &self.tex_clearcoat, mip_level: 0, origin, aspect: wgpu::TextureAspect::All },
                bytemuck::cast_slice(&layer.clearcoat_buf.data),
                wgpu::ImageDataLayout { offset: byte_offset_4, bytes_per_row: Some(bytes_per_row_4), rows_per_image: None },
                size,
            );
        }

        if channels.anisotropy && layer.anisotropy_buf.width == w as u32 && layer.anisotropy_buf.height == h as u32 {
            let total = (rect_w * rect_h) as usize;
            let mut sub_aniso = vec![0.0f32; total * 4];
            let aniso_data = &layer.anisotropy_buf.data;
            let angle_data = &layer.anisotropy_angle_buf.data;

            for dy in 0..rect_h {
                let sy = y0 as u32 + dy;
                for dx in 0..rect_w {
                    let sx = x0 as u32 + dx;
                    let src_idx = (sy * w as u32 + sx) as usize;
                    let dst_idx = (dy * rect_w + dx) as usize;
                    let a = if src_idx < aniso_data.len() { aniso_data[src_idx] } else { 0.0 };
                    let ang = if src_idx < angle_data.len() { angle_data[src_idx] } else { 0.0 };
                    sub_aniso[dst_idx * 4 + 0] = a;
                    sub_aniso[dst_idx * 4 + 1] = ang;
                    sub_aniso[dst_idx * 4 + 2] = 0.0;
                    sub_aniso[dst_idx * 4 + 3] = 1.0;
                }
            }

            self.queue.write_texture(
                wgpu::ImageCopyTexture { texture: &self.tex_anisotropy, mip_level: 0, origin, aspect: wgpu::TextureAspect::All },
                bytemuck::cast_slice(&sub_aniso),
                wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(rect_w * 16), rows_per_image: None },
                size,
            );
        }
    }

    pub fn ensure_gpu_layers(&mut self, project: &mut crate::project::Project) {
        let w = project.texture_width;
        let h = project.texture_height;
        self.ensure_pbr_textures(w, h);
        if self.gpu_compositor.width != w || self.gpu_compositor.height != h {
            self.gpu_compositor = crate::gpu_compositor::GpuCompositor::new(self.device.clone(), self.queue.clone(), w, h);
            self.gpu_layers.clear();
        }

        let project_ids: Vec<u64> = project.layers.iter().map(|l| l.id).collect();
        let current_gpu_ids: Vec<u64> = self.gpu_layers.iter().map(|l| l.id).collect();

        if project_ids != current_gpu_ids {
            let mut old_layers = std::mem::take(&mut self.gpu_layers);
            let mut new_gpu_layers = Vec::with_capacity(project.layers.len());
            for cpu_layer in &mut project.layers {
                if let Some(pos) = old_layers.iter().position(|gl| gl.id == cpu_layer.id) {
                    new_gpu_layers.push(old_layers.remove(pos));
                } else {
                    new_gpu_layers.push(self.gpu_compositor.create_layer(cpu_layer, w, h));
                    cpu_layer.dirty_rect = None;
                    cpu_layer.dirty_flags = crate::layer::LayerDirtyFlags::none();
                }
            }
            self.gpu_layers = new_gpu_layers;
        }
    }

    pub fn sync_mesh_maps(&mut self, project: &crate::project::Project) {
        let w = project.world_normal_map.as_ref().map(|m| m.width)
            .or_else(|| project.ao_map.as_ref().map(|m| m.width))
            .or_else(|| project.curvature_map.as_ref().map(|m| m.width))
            .or_else(|| project.thickness_map.as_ref().map(|m| m.width))
            .unwrap_or(project.texture_width);

        let h = project.world_normal_map.as_ref().map(|m| m.height)
            .or_else(|| project.ao_map.as_ref().map(|m| m.height))
            .or_else(|| project.curvature_map.as_ref().map(|m| m.height))
            .or_else(|| project.thickness_map.as_ref().map(|m| m.height))
            .unwrap_or(project.texture_height);

        self.ensure_baked_textures(w, h);

        let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
        
        let total_pixels = (w * h) as usize;
        let mut ao_curv_data = vec![255u8; total_pixels * 4];
        for i in 0..total_pixels {
            ao_curv_data[i * 4 + 0] = 255; // Default AO = 1.0
            ao_curv_data[i * 4 + 1] = 128; // Default flat curvature = 0.5
            ao_curv_data[i * 4 + 2] = 255; // Default thickness = 1.0 (solid)
            ao_curv_data[i * 4 + 3] = 255; // Alpha
        }

        if let Some(ao) = &project.ao_map {
            if ao.width == w && ao.height == h {
                for (i, &val) in ao.data.iter().enumerate() {
                    if i < total_pixels {
                        ao_curv_data[i * 4 + 0] = (val.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
            } else {
                for y in 0..h {
                    for x in 0..w {
                        let uv = glam::Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
                        let val = ao.sample_bilinear(uv);
                        let idx = (y * w + x) as usize;
                        if idx < total_pixels {
                            ao_curv_data[idx * 4 + 0] = (val.clamp(0.0, 1.0) * 255.0).round() as u8;
                        }
                    }
                }
            }
        }

        if let Some(curv) = &project.curvature_map {
            if curv.width == w && curv.height == h {
                for (i, &val) in curv.data.iter().enumerate() {
                    if i < total_pixels {
                        ao_curv_data[i * 4 + 1] = (val.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
            } else {
                for y in 0..h {
                    for x in 0..w {
                        let uv = glam::Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
                        let val = curv.sample_bilinear(uv);
                        let idx = (y * w + x) as usize;
                        if idx < total_pixels {
                            ao_curv_data[idx * 4 + 1] = (val.clamp(0.0, 1.0) * 255.0).round() as u8;
                        }
                    }
                }
            }
        }

        if let Some(th) = &project.thickness_map {
            if th.width == w && th.height == h {
                for (i, &val) in th.data.iter().enumerate() {
                    if i < total_pixels {
                        ao_curv_data[i * 4 + 2] = (val.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
            } else {
                for y in 0..h {
                    for x in 0..w {
                        let uv = glam::Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
                        let val = th.sample_bilinear(uv);
                        let idx = (y * w + x) as usize;
                        if idx < total_pixels {
                            ao_curv_data[idx * 4 + 2] = (val.clamp(0.0, 1.0) * 255.0).round() as u8;
                        }
                    }
                }
            }
        }

        self.queue.write_texture(
            wgpu::ImageCopyTexture { texture: &self.tex_baked_ao, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &ao_curv_data,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * size.width), rows_per_image: Some(size.height) },
            size
        );

        if let Some(wn) = &project.world_normal_map {
            if wn.width == w && wn.height == h {
                self.queue.write_texture(
                    wgpu::ImageCopyTexture { texture: &self.tex_baked_normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                    bytemuck::cast_slice(&wn.data),
                    wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * 4 * size.width), rows_per_image: Some(size.height) },
                    size
                );
            } else {
                let mut resampled_wn = Vec::with_capacity(total_pixels);
                for y in 0..h {
                    for x in 0..w {
                        let uv = glam::Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
                        resampled_wn.push(wn.sample_bilinear(uv));
                    }
                }
                self.queue.write_texture(
                    wgpu::ImageCopyTexture { texture: &self.tex_baked_normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                    bytemuck::cast_slice(&resampled_wn),
                    wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * 4 * size.width), rows_per_image: Some(size.height) },
                    size
                );
            }
        } else {
            // Default flat tangent normal
            let def_n = vec![glam::Vec4::new(0.5, 0.5, 1.0, 1.0); total_pixels];
            self.queue.write_texture(
                wgpu::ImageCopyTexture { texture: &self.tex_baked_normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                bytemuck::cast_slice(&def_n),
                wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * 4 * size.width), rows_per_image: Some(size.height) },
                size
            );
        }
    }

fn blur_equirectangular(
    img: &image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
    radius: usize,
) -> image::ImageBuffer<image::Rgba<f32>, Vec<f32>> {
    let w = img.width() as usize;
    let h = img.height() as usize;
    if w == 0 || h == 0 || radius == 0 {
        return img.clone();
    }

    let sigma = (radius as f32) * 0.5;
    let two_sigma_sq = 2.0 * sigma * sigma;
    let mut weights = Vec::with_capacity(radius * 2 + 1);
    let mut weight_sum = 0.0f32;
    for k in -(radius as isize)..=(radius as isize) {
        let weight = (-((k * k) as f32) / two_sigma_sq).exp();
        weights.push(weight);
        weight_sum += weight;
    }
    for w_val in &mut weights {
        *w_val /= weight_sum;
    }

    let raw = img.as_raw();
    let mut temp = vec![0.0f32; w * h * 4];

    // Horizontal pass with equirectangular cylindrical wrapping (x = 0 and x = w-1 wrap around)
    for y in 0..h {
        let row_offset = y * w * 4;
        for x in 0..w {
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut a = 0.0f32;

            for (idx, &weight) in weights.iter().enumerate() {
                let k = idx as isize - radius as isize;
                let sample_x = ((x as isize + k).rem_euclid(w as isize)) as usize;
                let src_idx = row_offset + sample_x * 4;
                r += raw[src_idx] * weight;
                g += raw[src_idx + 1] * weight;
                b += raw[src_idx + 2] * weight;
                a += raw[src_idx + 3] * weight;
            }

            let dst_idx = row_offset + x * 4;
            temp[dst_idx] = r;
            temp[dst_idx + 1] = g;
            temp[dst_idx + 2] = b;
            temp[dst_idx + 3] = a;
        }
    }

    // Vertical pass with edge clamping at poles
    let mut out_data = vec![0.0f32; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut a = 0.0f32;

            for (idx, &weight) in weights.iter().enumerate() {
                let k = idx as isize - radius as isize;
                let sample_y = (y as isize + k).clamp(0, h as isize - 1) as usize;
                let src_idx = (sample_y * w + x) * 4;
                r += temp[src_idx] * weight;
                g += temp[src_idx + 1] * weight;
                b += temp[src_idx + 2] * weight;
                a += temp[src_idx + 3] * weight;
            }

            let dst_idx = (y * w + x) * 4;
            out_data[dst_idx] = r;
            out_data[dst_idx + 1] = g;
            out_data[dst_idx + 2] = b;
            out_data[dst_idx + 3] = a;
        }
    }

    image::ImageBuffer::from_raw(img.width(), img.height(), out_data).unwrap_or_else(|| img.clone())
}

    pub fn load_environment(&mut self, width: u32, height: u32, data_rgba_f32: &[f32]) {
        let mip_level_count = (width.max(height) as f32).log2().floor() as u32 + 1;
        let env_size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        
        self.tex_environment = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Tex Env Loaded"),
            size: env_size,
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        
        // Base level 0
        self.queue.write_texture(
            wgpu::ImageCopyTexture { texture: &self.tex_environment, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            bytemuck::cast_slice(data_rgba_f32),
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * 4 * width), rows_per_image: Some(height) },
            env_size,
        );

        // Generate Mipmaps on CPU with spherical Gaussian blur
        if let Some(base_img) = image::ImageBuffer::<image::Rgba<f32>, Vec<f32>>::from_raw(width, height, data_rgba_f32.to_vec()) {
            let mut current_img = base_img;
            for mip in 1..mip_level_count {
                let mip_w = (width >> mip).max(1);
                let mip_h = (height >> mip).max(1);
                
                let resized_img = image::imageops::resize(&current_img, mip_w, mip_h, image::imageops::FilterType::Triangle);
                let blur_radius = (mip as usize).min(4);
                let next_img = Self::blur_equirectangular(&resized_img, blur_radius);
                
                let mip_size = wgpu::Extent3d { width: mip_w, height: mip_h, depth_or_array_layers: 1 };
                self.queue.write_texture(
                    wgpu::ImageCopyTexture { texture: &self.tex_environment, mip_level: mip, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                    bytemuck::cast_slice(next_img.as_raw()),
                    wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * 4 * mip_w), rows_per_image: Some(mip_h) },
                    mip_size,
                );
                
                current_img = next_img;
            }
        }

        // Recreate the material bind group with the new environment texture!
        self.rebuild_material_bind_group();
    }

    fn create_color_target(
        device: &wgpu::Device,
        egui_renderer: &eframe::egui::mutex::RwLock<egui_wgpu::Renderer>,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView, eframe::egui::TextureId) {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Color Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let texture_id = egui_renderer.write().register_native_texture(
            device,
            &view,
            wgpu::FilterMode::Linear,
        );

        (texture, view, texture_id)
    }

    fn create_history_target(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Temporal History Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_scene_target(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Scene HDR Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_depth_target(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_scene_normal_target(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Scene Normal Roughness Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_postprocess_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        buffer: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        scene_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        scene_normal_view: &wgpu::TextureView,
        history_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PostProcess Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(scene_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(scene_normal_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(history_view) },
            ],
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 && (self.width != width || self.height != height) {
            self.width = width;
            self.height = height;

            self.egui_renderer.write().free_texture(&self.texture_id);

            let (color_texture, color_view, texture_id) = Self::create_color_target(
                &self.device,
                &self.egui_renderer,
                width,
                height,
            );
            self.color_texture = color_texture;
            self.color_view = color_view;
            self.texture_id = texture_id;

            let (scene_texture, scene_view) = Self::create_scene_target(&self.device, width, height);
            self.scene_texture = scene_texture;
            self.scene_view = scene_view;

            let (scene_normal_texture, scene_normal_view) = Self::create_scene_normal_target(&self.device, width, height);
            self.scene_normal_texture = scene_normal_texture;
            self.scene_normal_view = scene_normal_view;

            let (depth_texture, depth_view) = Self::create_depth_target(&self.device, width, height);
            self.depth_texture = depth_texture;
            self.depth_view = depth_view;

            let (tex_history, tex_history_view) = Self::create_history_target(&self.device, width, height);
            self.tex_history = tex_history;
            self.tex_history_view = tex_history_view;
            self.dlaa_manager.reset_history();

            self.postprocess_bind_group = Self::create_postprocess_bind_group(
                &self.device,
                &self.postprocess_bind_group_layout,
                &self.postprocess_buffer,
                &self.sampler,
                &self.scene_view,
                &self.depth_view,
                &self.scene_normal_view,
                &self.tex_history_view,
            );
        }
    }

    pub fn render(
        &mut self,
        view_proj: Mat4,
        eye_pos: Vec4,
        env_rot: f32,
        view_mode: u32,
        bg_blur: f32,
        bg_opacity: f32,
        shader: &ShaderSettings,
        show_wireframe: bool,
    ) {
        let is_dlaa = shader.aa_mode == crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa;
        let jitter = self.dlaa_manager.next_jitter(self.width, self.height, is_dlaa);
        let view_proj_jittered = self.dlaa_manager.jitter_projection(view_proj, is_dlaa);

        // Directional Sun Light Matrix & Frustum (synchronized with HDRI azimuth)
        let cos_r = env_rot.cos();
        let sin_r = env_rot.sin();
        let sun_local = glam::Vec3::new(0.6, 0.7, 0.4).normalize();
        let light_dir = glam::Vec3::new(
            cos_r * sun_local.x + sin_r * sun_local.z,
            sun_local.y,
            -sin_r * sun_local.x + cos_r * sun_local.z,
        ).normalize();
        let light_dist = self.mesh_radius * 2.5;
        let light_pos = self.mesh_center + light_dir * light_dist;
        let up_vec = if light_dir.y.abs() > 0.95 { glam::Vec3::Z } else { glam::Vec3::Y };
        let light_view = glam::Mat4::look_at_rh(light_pos, self.mesh_center, up_vec);
        let box_size = self.mesh_radius * 1.35;
        let light_proj = glam::Mat4::orthographic_rh(
            -box_size, box_size,
            -box_size, box_size,
            0.1, light_dist * 2.0,
        );
        let light_view_proj = light_proj * light_view;

        let t_enc = std::time::Instant::now();
        let mut data = [0u8; 352];
        data[0..64].copy_from_slice(bytemuck::cast_slice(&[view_proj_jittered]));
        data[64..80].copy_from_slice(bytemuck::cast_slice(&[eye_pos]));
        data[80..84].copy_from_slice(bytemuck::cast_slice(&[env_rot]));
        data[84..88].copy_from_slice(bytemuck::cast_slice(&[view_mode]));
        data[88..92].copy_from_slice(bytemuck::cast_slice(&[bg_blur]));
        data[92..96].copy_from_slice(bytemuck::cast_slice(&[bg_opacity]));
        let inv_view_proj = view_proj_jittered.inverse();
        data[96..160].copy_from_slice(bytemuck::cast_slice(&[inv_view_proj]));
        data[160..164].copy_from_slice(bytemuck::cast_slice(&[shader.sun_intensity]));
        data[164..168].copy_from_slice(bytemuck::cast_slice(&[shader.env_intensity]));
        data[168..172].copy_from_slice(bytemuck::cast_slice(&[shader.ao_intensity]));
        data[172..176].copy_from_slice(bytemuck::cast_slice(&[shader.exposure]));
        data[176..180].copy_from_slice(bytemuck::cast_slice(&[shader.normal_format]));
        data[180..184].copy_from_slice(bytemuck::cast_slice(&[shader.tonemap_mode]));
        data[184..188].copy_from_slice(bytemuck::cast_slice(&[shader.specular_mult]));
        data[188..192].copy_from_slice(bytemuck::cast_slice(&[shader.roughness_bias]));
        data[192..196].copy_from_slice(bytemuck::cast_slice(&[shader.alpha_cutoff]));
        let trans_mode = shader.transparency_mode as u32;
        data[196..200].copy_from_slice(bytemuck::cast_slice(&[trans_mode]));
        let dbl_side = if shader.double_sided { 1u32 } else { 0u32 };
        data[200..204].copy_from_slice(bytemuck::cast_slice(&[dbl_side]));
        data[204..208].copy_from_slice(bytemuck::cast_slice(&[shader.ior]));
        data[208..212].copy_from_slice(bytemuck::cast_slice(&[shader.refraction_dispersion]));
        data[212..216].copy_from_slice(bytemuck::cast_slice(&[shader.translucency_depth]));
        data[216..220].copy_from_slice(bytemuck::cast_slice(&[0.0f32]));
        data[220..224].copy_from_slice(bytemuck::cast_slice(&[0.0f32]));
        data[224..288].copy_from_slice(bytemuck::cast_slice(&[light_view_proj]));
        let shadows_u32 = if shader.shadows_enabled { 1u32 } else { 0u32 };
        data[288..292].copy_from_slice(bytemuck::cast_slice(&[shadows_u32]));
        data[292..296].copy_from_slice(bytemuck::cast_slice(&[shader.shadow_bias]));
        data[296..300].copy_from_slice(bytemuck::cast_slice(&[shader.shadow_softness]));
        data[300..304].copy_from_slice(bytemuck::cast_slice(&[shader.wireframe_opacity]));
        data[304..320].copy_from_slice(bytemuck::cast_slice(&shader.wireframe_color));
        let (cage_color, cage_opacity) = if let Some(ref c) = self.cage_mesh {
            (c.color, c.opacity)
        } else {
            ([0.0, 0.88, 1.0, 1.0], 0.0)
        };
        data[320..336].copy_from_slice(bytemuck::cast_slice(&cage_color));
        data[336..340].copy_from_slice(bytemuck::cast_slice(&[cage_opacity]));
        self.queue.write_buffer(&self.camera_buffer, 0, &data);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Viewport Encoder"),
        });

        // --- PASS 0: DIRECTIONAL SUN SHADOW DEPTH PASS ---
        if shader.shadows_enabled {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Directional Shadow Render Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            shadow_pass.set_pipeline(&self.shadow_pipeline);
            shadow_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            shadow_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            shadow_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            if self.submeshes.is_empty() {
                shadow_pass.draw_indexed(0..self.index_count, 0, 0..1);
            } else {
                for sm in &self.submeshes {
                    if sm.visible && sm.index_count > 0 {
                        let start = sm.start_index as u32;
                        let count = sm.index_count as u32;
                        shadow_pass.draw_indexed(start..(start + count), 0, 0..1);
                    }
                }
            }
        }

        // --- PASS 1: RENDER 3D SCENE (SKYBOX + MESH) INTO SCENE_VIEW ---
        self.profiler.begin_frame(&self.device);
        self.profiler.cpu_begin_pbr();
        let pbr_ts_writes = self.profiler.pbr_timestamp_writes();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene Render Pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.scene_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.086, g: 0.090, b: 0.102, a: 1.0, 
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.scene_normal_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.5, g: 0.5, b: 0.5, a: 1.0, 
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: pbr_ts_writes,
                occlusion_query_set: None,
            });

            // Draw Skybox
            if bg_opacity > 0.0 {
                pass.set_pipeline(&self.skybox_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_bind_group(1, &self.material_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            
            // Draw Mesh
            let active_pipeline = if shader.double_sided {
                &self.pipeline_double_sided
            } else {
                &self.pipeline
            };
            pass.set_pipeline(active_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, &self.material_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            if self.submeshes.is_empty() {
                pass.draw_indexed(0..self.index_count, 0, 0..1);
            } else {
                for sm in &self.submeshes {
                    if sm.visible && sm.index_count > 0 {
                        let start = sm.start_index as u32;
                        let count = sm.index_count as u32;
                        pass.draw_indexed(start..(start + count), 0, 0..1);
                    }
                }
            }

            // Draw Wireframe Overlay
            if show_wireframe && self.wire_index_count > 0 {
                pass.set_pipeline(&self.wireframe_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.set_index_buffer(self.wire_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.wire_index_count, 0, 0..1);
            }

            // Draw Baking Cage Preview Overlay (Translucent Shaded Hull and Glowing Wireframe)
            if let Some(ref cage) = self.cage_mesh {
                // 1. Shaded translucent cage hull
                if cage.show_hull && cage.hull_index_count > 0 {
                    pass.set_pipeline(&self.cage_hull_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, cage.vertex_buffer.slice(..));
                    pass.set_index_buffer(cage.hull_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..cage.hull_index_count, 0, 0..1);
                }
                // 2. Glowing cage wireframe & displacement tethers
                if cage.wire_index_count > 0 {
                    pass.set_pipeline(&self.cage_wire_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, cage.vertex_buffer.slice(..));
                    pass.set_index_buffer(cage.wire_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..cage.wire_index_count, 0, 0..1);
                }
            }
        }
        self.profiler.cpu_end_pbr();

        // --- UPDATE POSTPROCESS UNIFORM BUFFER ---
        let mut pp_data = [0u8; 272];
        let screen_size = [self.width as f32, self.height as f32];
        let fxaa_u32 = if shader.fxaa_enabled || shader.aa_mode == crate::nvidia_dlaa::AntiAliasingMode::Fxaa { 1u32 } else { 0u32 };
        let bloom_u32 = if shader.bloom_enabled { 1u32 } else { 0u32 };
        let dof_u32 = if shader.dof_enabled { 1u32 } else { 0u32 };
        let ssr_u32 = if shader.ssr_enabled { 1u32 } else { 0u32 };
        let near_z = 0.05f32;
        let far_z = 100.0f32;

        pp_data[0..64].copy_from_slice(bytemuck::cast_slice(&[view_proj_jittered]));
        pp_data[64..128].copy_from_slice(bytemuck::cast_slice(&[inv_view_proj]));
        pp_data[128..144].copy_from_slice(bytemuck::cast_slice(&[eye_pos]));
        pp_data[144..152].copy_from_slice(bytemuck::cast_slice(&screen_size));
        pp_data[152..156].copy_from_slice(bytemuck::cast_slice(&[fxaa_u32]));
        pp_data[156..160].copy_from_slice(bytemuck::cast_slice(&[bloom_u32]));
        pp_data[160..164].copy_from_slice(bytemuck::cast_slice(&[shader.bloom_intensity]));
        pp_data[164..168].copy_from_slice(bytemuck::cast_slice(&[shader.bloom_threshold]));
        pp_data[168..172].copy_from_slice(bytemuck::cast_slice(&[shader.bloom_radius]));
        pp_data[172..176].copy_from_slice(bytemuck::cast_slice(&[dof_u32]));
        pp_data[176..180].copy_from_slice(bytemuck::cast_slice(&[shader.dof_focus_distance]));
        pp_data[180..184].copy_from_slice(bytemuck::cast_slice(&[shader.dof_aperture]));
        pp_data[184..188].copy_from_slice(bytemuck::cast_slice(&[shader.dof_max_blur]));
        pp_data[188..192].copy_from_slice(bytemuck::cast_slice(&[shader.chromatic_aberration]));
        pp_data[192..196].copy_from_slice(bytemuck::cast_slice(&[shader.vignette_strength]));
        pp_data[196..200].copy_from_slice(bytemuck::cast_slice(&[near_z]));
        pp_data[200..204].copy_from_slice(bytemuck::cast_slice(&[far_z]));
        pp_data[204..208].copy_from_slice(bytemuck::cast_slice(&[ssr_u32]));
        pp_data[208..212].copy_from_slice(bytemuck::cast_slice(&[shader.ssr_intensity]));
        pp_data[212..216].copy_from_slice(bytemuck::cast_slice(&[shader.ssr_max_roughness]));
        pp_data[216..220].copy_from_slice(bytemuck::cast_slice(&[shader.ssr_step_size]));
        pp_data[220..224].copy_from_slice(bytemuck::cast_slice(&[shader.ssr_max_steps]));
        pp_data[224..228].copy_from_slice(bytemuck::cast_slice(&[view_mode]));
        let ssao_u32 = if shader.ssao_enabled { 1u32 } else { 0u32 };
        pp_data[228..232].copy_from_slice(bytemuck::cast_slice(&[ssao_u32]));
        pp_data[232..236].copy_from_slice(bytemuck::cast_slice(&[shader.ssao_radius]));
        pp_data[236..240].copy_from_slice(bytemuck::cast_slice(&[shader.ssao_intensity]));

        let aa_mode_u32 = match shader.aa_mode {
            crate::nvidia_dlaa::AntiAliasingMode::Off => 0u32,
            crate::nvidia_dlaa::AntiAliasingMode::Fxaa => 1u32,
            crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa => 2u32,
        };
        let denoiser_u32 = if shader.denoiser_enabled { 1u32 } else { 0u32 };
        pp_data[240..244].copy_from_slice(bytemuck::cast_slice(&[aa_mode_u32]));
        pp_data[244..248].copy_from_slice(bytemuck::cast_slice(&[denoiser_u32]));
        pp_data[248..252].copy_from_slice(bytemuck::cast_slice(&[shader.denoiser_radius]));
        pp_data[252..256].copy_from_slice(bytemuck::cast_slice(&[shader.denoiser_normal_weight]));
        pp_data[256..260].copy_from_slice(bytemuck::cast_slice(&[shader.denoiser_depth_weight]));
        pp_data[260..264].copy_from_slice(bytemuck::cast_slice(&[shader.history_blend_weight]));
        let jitter_arr = [jitter.0, jitter.1];
        pp_data[264..272].copy_from_slice(bytemuck::cast_slice(&jitter_arr));
        self.queue.write_buffer(&self.postprocess_buffer, 0, &pp_data);

        // --- PASS 2: POST-PROCESSING (FXAA/DLAA, DENOISER, BLOOM, DOF, LENS) INTO COLOR_VIEW ---
        self.profiler.cpu_begin_postprocess();
        let pp_ts_writes = self.profiler.postprocess_timestamp_writes();
        {
            let mut pp_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PostProcess Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: pp_ts_writes,
                occlusion_query_set: None,
            });

            pp_pass.set_pipeline(&self.postprocess_pipeline);
            pp_pass.set_bind_group(0, &self.postprocess_bind_group, &[]);
            pp_pass.draw(0..3, 0..1);
        }
        self.profiler.cpu_end_postprocess();

        // Copy current resolved color buffer to history texture for DLAA temporal reconstruction
        let copy_size = wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        };
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &self.color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &self.tex_history,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            copy_size,
        );
        self.dlaa_manager.history_valid = true;
        self.dlaa_manager.prev_view_proj = view_proj;

        self.profiler.resolve(&mut encoder);

        let cmd_buffer = encoder.finish();
        self.cmd_encoding_ms = t_enc.elapsed().as_secs_f32() * 1000.0;

        let t_sub = std::time::Instant::now();
        self.queue.submit(std::iter::once(cmd_buffer));
        self.queue_submit_ms = t_sub.elapsed().as_secs_f32() * 1000.0;
        self.profiler.on_submitted();
    }
}

impl WgpuViewport {
    pub fn update_mesh(&mut self, mesh: &crate::mesh::Mesh) {
        let (center, radius) = calculate_mesh_bounds(&mesh.vertices);
        self.mesh_center = center;
        self.mesh_radius = radius;

        let gpu_vertices: Vec<crate::mesh::GpuVertex> = mesh.vertices.iter().map(|v| v.into()).collect();
        self.vertex_buffer = self.device.create_buffer_init(&eframe::wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer Replaced"),
            contents: bytemuck::cast_slice(&gpu_vertices),
            usage: eframe::wgpu::BufferUsages::VERTEX,
        });

        self.index_buffer = self.device.create_buffer_init(&eframe::wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer Replaced"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: eframe::wgpu::BufferUsages::INDEX,
        });
        
        let wire_indices = mesh.generate_wireframe_indices();
        self.wire_index_buffer = self.device.create_buffer_init(&eframe::wgpu::util::BufferInitDescriptor {
            label: Some("Wire Index Buffer Replaced"),
            contents: bytemuck::cast_slice(&wire_indices),
            usage: eframe::wgpu::BufferUsages::INDEX,
        });

        self.index_count = mesh.indices.len() as u32;
        self.wire_index_count = wire_indices.len() as u32;
        self.submeshes = mesh.submeshes.clone();
        self.gpu_layers.clear();
    }

    pub fn sync_submesh_visibility(&mut self, submeshes: &[crate::mesh::SubMeshRange]) {
        self.submeshes = submeshes.to_vec();
    }

    pub fn clear_gpu_layers(&mut self) {
        self.gpu_layers.clear();
    }

    #[allow(dead_code)]
    pub fn device(&self) -> Arc<wgpu::Device> {
        Arc::clone(&self.device)
    }

    #[allow(dead_code)]
    pub fn queue(&self) -> Arc<wgpu::Queue> {
        Arc::clone(&self.queue)
    }

    /// Update or construct cage geometry buffers for real-time 3D viewport preview
    pub fn update_cage(
        &mut self,
        mesh: &crate::mesh::Mesh,
        cage_positions: Option<&[glam::Vec3]>,
        show_hull: bool,
        show_tethers: bool,
        color: [f32; 4],
        opacity: f32,
    ) {
        let cage_positions = match cage_positions {
            Some(pos) if !pos.is_empty() && pos.len() == mesh.vertices.len() => pos,
            _ => {
                self.cage_mesh = None;
                return;
            }
        };

        let n = mesh.vertices.len();
        let mut gpu_vertices = Vec::with_capacity(n * 2);

        // 0..N: base mesh vertices (tether origin anchors)
        for v in &mesh.vertices {
            gpu_vertices.push(v.into());
        }

        // N..2N: cage vertices (displaced outward along smoothed normals)
        for (v, &c_pos) in mesh.vertices.iter().zip(cage_positions.iter()) {
            gpu_vertices.push(crate::mesh::GpuVertex {
                position: [c_pos.x, c_pos.y, c_pos.z],
                normal: [v.normal.x, v.normal.y, v.normal.z],
                tangent: [v.tangent.x, v.tangent.y, v.tangent.z, v.tangent.w],
                uv: [v.uv.x, v.uv.y],
            });
        }

        let mut wire_indices = Vec::new();
        let tri_count = mesh.indices.len() / 3;

        // 1. Cage wireframe lines (at N..2N)
        for t in 0..tri_count {
            let i0 = (mesh.indices[t * 3] as usize + n) as u32;
            let i1 = (mesh.indices[t * 3 + 1] as usize + n) as u32;
            let i2 = (mesh.indices[t * 3 + 2] as usize + n) as u32;
            wire_indices.push(i0);
            wire_indices.push(i1);
            wire_indices.push(i1);
            wire_indices.push(i2);
            wire_indices.push(i2);
            wire_indices.push(i0);
        }

        // 2. Displacement tethers connecting base vertex i to cage vertex N + i
        if show_tethers {
            let step = if n > 4000 { (n / 2000).max(1) } else { 1 };
            for i in (0..n).step_by(step) {
                wire_indices.push(i as u32);
                wire_indices.push((i + n) as u32);
            }
        }

        // 3. Cage hull triangle indices (at N..2N)
        let mut hull_indices = Vec::with_capacity(mesh.indices.len());
        for &idx in &mesh.indices {
            hull_indices.push((idx as usize + n) as u32);
        }

        let vertex_buffer = self.device.create_buffer_init(&eframe::wgpu::util::BufferInitDescriptor {
            label: Some("Cage Vertex Buffer"),
            contents: bytemuck::cast_slice(&gpu_vertices),
            usage: eframe::wgpu::BufferUsages::VERTEX,
        });

        let wire_index_buffer = self.device.create_buffer_init(&eframe::wgpu::util::BufferInitDescriptor {
            label: Some("Cage Wire Index Buffer"),
            contents: bytemuck::cast_slice(&wire_indices),
            usage: eframe::wgpu::BufferUsages::INDEX,
        });

        let hull_index_buffer = self.device.create_buffer_init(&eframe::wgpu::util::BufferInitDescriptor {
            label: Some("Cage Hull Index Buffer"),
            contents: bytemuck::cast_slice(&hull_indices),
            usage: eframe::wgpu::BufferUsages::INDEX,
        });

        self.cage_mesh = Some(WgpuCageMesh {
            vertex_buffer,
            wire_index_buffer,
            wire_index_count: wire_indices.len() as u32,
            hull_index_buffer,
            hull_index_count: hull_indices.len() as u32,
            show_hull,
            color,
            opacity,
        });
    }

    /// Clear and disable cage preview overlay in viewport
    pub fn clear_cage(&mut self) {
        self.cage_mesh = None;
    }

    /// Check if cage overlay is currently active
    #[allow(dead_code)]
    pub fn has_cage(&self) -> bool {
        self.cage_mesh.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_settings_defaults_and_postprocess() {
        let settings = ShaderSettings::default();
        assert_eq!(settings.aa_mode, crate::nvidia_dlaa::AntiAliasingMode::NvidiaDlaa);
        assert!(!settings.denoiser_enabled);
        assert_eq!(settings.denoiser_radius, 0.8);
        assert!(!settings.bloom_enabled);
        assert!(!settings.dof_enabled);
        assert!(!settings.ssr_enabled);
        assert!(settings.adaptive_resolution);
        assert_eq!(settings.tonemap_mode, 1);
        assert_eq!(settings.normal_format, 0);
        assert_eq!(settings.transparency_mode, TransparencyMode::Dithered);
        assert!((settings.alpha_cutoff - 0.33).abs() < 1e-4);
        assert!(!settings.double_sided);
        assert!((settings.ior - 1.52).abs() < 1e-4);
        assert!((settings.refraction_dispersion - 0.015).abs() < 1e-4);
        assert!((settings.translucency_depth - 1.0).abs() < 1e-4);
        assert!(settings.ssao_enabled);
        assert!((settings.ssao_intensity - 0.50).abs() < 1e-4);
        assert!((settings.ssao_radius - 0.35).abs() < 1e-4);
        assert!(settings.shadows_enabled);
        assert!((settings.shadow_bias - 0.015).abs() < 1e-5);
        assert!((settings.shadow_softness - 1.0).abs() < 1e-4);
        assert_eq!(settings.wireframe_color, [0.2, 0.75, 1.0, 1.0]);
        assert!((settings.wireframe_opacity - 0.85).abs() < 1e-4);

        assert_eq!(TransparencyMode::Dithered.name(), "Dithered Transparency (Screen-Door)");
        assert_eq!(TransparencyMode::AlphaCutout.name(), "Alpha Cutout (Alpha Test)");
        assert_eq!(TransparencyMode::AlphaBlend.name(), "Alpha Blending (Translucent Glass)");
        assert_eq!(TransparencyMode::Refraction.name(), "Physical Refraction / Translucency (IOR Glass & Gems)");
        assert_eq!(TransparencyMode::Opaque.name(), "Opaque (Ignore Opacity)");
    }

    #[test]
    fn test_wgpu_adapter_info() {
        let instance = eframe::wgpu::Instance::default();
        let adapters = instance.enumerate_adapters(eframe::wgpu::Backends::all());
        for a in adapters {
            let info = a.get_info();
            println!("ADAPTER: {:?}, backend: {:?}, device_type: {:?}", info.name, info.backend, info.device_type);
        }
    }

    #[test]
    fn test_wgpu_gpu_compositor_benchmark() {
        let instance = eframe::wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&eframe::wgpu::RequestAdapterOptions {
            power_preference: eframe::wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })) {
            Some(a) => a,
            None => {
                println!("No GPU adapter available for test");
                return;
            }
        };

        println!("BENCHMARK GPU ADAPTER: {:?}", adapter.get_info().name);
        let (device, queue) = match pollster::block_on(adapter.request_device(
            &eframe::wgpu::DeviceDescriptor {
                label: Some("Bench Device"),
                required_features: eframe::wgpu::Features::FLOAT32_FILTERABLE,
                required_limits: eframe::wgpu::Limits::default(),
            },
            None,
        )) {
            Ok(dq) => dq,
            Err(e) => {
                println!("Failed to create device: {:?}", e);
                return;
            }
        };

        let device = std::sync::Arc::new(device);
        let queue = std::sync::Arc::new(queue);

        let w = 512;
        let h = 512;
        let t0 = std::time::Instant::now();
        let mut compositor = crate::gpu_compositor::GpuCompositor::new(device.clone(), queue.clone(), w, h);
        println!("GPU Compositor init 512x512 took {:?}", t0.elapsed());

        let mut cpu_layer = crate::layer::Layer::new_paint_layer(1, "Test Layer".to_string(), w, h);
        let gpu_layer = compositor.create_layer(&cpu_layer, w, h);

        // Benchmark update_layer 100 times with dirty rect
        let t1 = std::time::Instant::now();
        let iters = 100;
        for _i in 0..iters {
            cpu_layer.dirty_rect = Some((100, 100, 160, 160));
            compositor.update_layer(&mut cpu_layer, &gpu_layer, w, h);
        }
        let update_elapsed = t1.elapsed();
        println!("100 update_layer calls took {:?}, avg {:.3} ms/call", update_elapsed, update_elapsed.as_secs_f64() * 1000.0 / iters as f64);

        // Benchmark composite_all 100 times
        let t2 = std::time::Instant::now();
        let cpu_layers = vec![cpu_layer];
        let gpu_layers = vec![gpu_layer];
        for _ in 0..iters {
            let _ = compositor.composite_all(&cpu_layers, &gpu_layers);
        }
        let comp_elapsed = t2.elapsed();
        println!("100 composite_all calls took {:?}, avg {:.3} ms/call", comp_elapsed, comp_elapsed.as_secs_f64() * 1000.0 / iters as f64);

        // Benchmark multi-layer composite_all vs incremental caching across 5 layers
        let mut multi_cpu_layers = Vec::new();
        let mut multi_gpu_layers = Vec::new();
        for i in 0..5 {
            let l = crate::layer::Layer::new_paint_layer(i as u64 + 10, format!("Layer {}", i), w, h);
            let gl = compositor.create_layer(&l, w, h);
            multi_cpu_layers.push(l);
            multi_gpu_layers.push(gl);
        }

        // Full composite_all benchmark across 5 layers
        let t_full = std::time::Instant::now();
        for _ in 0..iters {
            let _ = compositor.composite_all(&multi_cpu_layers, &multi_gpu_layers);
        }
        let full_elapsed = t_full.elapsed();
        println!("100 composite_all (5 layers) took {:?}, avg {:.3} ms/call", full_elapsed, full_elapsed.as_secs_f64() * 1000.0 / iters as f64);

        // Incremental composite benchmark on top layer (Layer 4)
        let t_inc = std::time::Instant::now();
        for _ in 0..iters {
            let _ = compositor.composite_incremental(&multi_cpu_layers, &multi_gpu_layers, 4);
        }
        let inc_elapsed = t_inc.elapsed();
        println!("100 composite_incremental (5 layers, active layer 4) took {:?}, avg {:.3} ms/call", inc_elapsed, inc_elapsed.as_secs_f64() * 1000.0 / iters as f64);
        assert!(inc_elapsed <= full_elapsed, "Incremental compositing must be faster or equal to full stack compositing");
    }

    #[test]
    fn test_wgpu_viewport_render_headless() {
        let instance = eframe::wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&eframe::wgpu::RequestAdapterOptions {
            power_preference: eframe::wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })) {
            Some(a) => a,
            None => {
                println!("No GPU adapter available for test");
                return;
            }
        };
        println!("TEST ADAPTER: {:?}", adapter.get_info());

        let (device, queue) = match pollster::block_on(adapter.request_device(
            &eframe::wgpu::DeviceDescriptor {
                label: Some("Test Device"),
                required_features: eframe::wgpu::Features::FLOAT32_FILTERABLE,
                required_limits: eframe::wgpu::Limits::default(),
            },
            None,
        )) {
            Ok(dq) => dq,
            Err(e) => {
                println!("Failed to create device: {:?}", e);
                return;
            }
        };

        let device = std::sync::Arc::new(device);
        let queue = std::sync::Arc::new(queue);
        let renderer = std::sync::Arc::new(eframe::epaint::mutex::RwLock::new(
            eframe::egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb, None, 1)
        ));
        let adapter_arc = std::sync::Arc::new(adapter);
        let render_state = eframe::egui_wgpu::RenderState {
            device: device.clone(),
            queue: queue.clone(),
            target_format: wgpu::TextureFormat::Rgba8UnormSrgb,
            renderer,
            available_adapters: std::sync::Arc::new([]),
            adapter: adapter_arc,
        };

        let mesh = crate::mesh::Mesh::create_shader_ball();
        let mut viewport = WgpuViewport::new(&render_state, &mesh);

        let mut project = crate::project::Project::new_default(512, 512);
        project.mesh = mesh;
        viewport.sync_textures(&mut project);
        viewport.sync_mesh_maps(&project);

        let vp = glam::Mat4::IDENTITY;
        let eye = glam::Vec4::new(0.0, 0.0, 3.0, 1.0);
        let shader = ShaderSettings::default();
        
        // Resize before Frame 1
        viewport.resize(1280, 720);

        // Frame 1
        viewport.render(vp, eye, 0.0, 0, 0.0, 1.0, &shader, false);

        // Resize between frames
        viewport.resize(1920, 1080);

        // Frame 2
        viewport.render(vp, eye, 0.0, 0, 0.0, 1.0, &shader, true);

        // Frame 3: Baking Cage Preview
        let cage_cfg = crate::baker::BakeCageConfig {
            mode: crate::baker::BakeCageMode::ParametricInflation,
            inflation_offset: 0.05,
            external_cage_path: None,
        };
        let cage_verts = crate::baker::Baker::compute_cage_vertices(&project.mesh, &cage_cfg).unwrap().unwrap();
        viewport.update_cage(&project.mesh, Some(&cage_verts), true, true, [0.0, 0.88, 1.0, 1.0], 0.85);
        assert!(viewport.has_cage(), "WgpuViewport must report active cage");

        viewport.render(vp, eye, 0.0, 0, 0.0, 1.0, &shader, true);

        // Frame 4: Clear Cage
        viewport.clear_cage();
        assert!(!viewport.has_cage(), "WgpuViewport must report no active cage after clear");
        viewport.render(vp, eye, 0.0, 0, 0.0, 1.0, &shader, false);
    }

    #[test]
    fn test_zero_copy_gpu_compute_brush_stamping_and_readback() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::empty(),
            dx12_shader_compiler: Default::default(),
            gles_minor_version: Default::default(),
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }));
        let adapter = match adapter {
            Some(a) => a,
            None => {
                eprintln!("Skipping test_zero_copy_gpu_compute_brush_stamping_and_readback: no GPU adapter found");
                return;
            }
        };

        let required_features = if adapter.features().contains(wgpu::Features::FLOAT32_FILTERABLE) {
            wgpu::Features::FLOAT32_FILTERABLE
        } else {
            eprintln!("Skipping test_zero_copy_gpu_compute_brush_stamping_and_readback: FLOAT32_FILTERABLE not supported by adapter");
            return;
        };

        let (device, queue) = match pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("GPU Compute Test Device"),
                required_features,
                required_limits: wgpu::Limits::default(),
            },
            None,
        )) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("Skipping test_zero_copy_gpu_compute_brush_stamping_and_readback: {}", e);
                return;
            }
        };

        let device = std::sync::Arc::new(device);
        let queue = std::sync::Arc::new(queue);
        let adapter_arc = std::sync::Arc::new(adapter);
        let renderer = std::sync::Arc::new(eframe::egui::mutex::RwLock::new(
            egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb, None, 1),
        ));

        let render_state = eframe::egui_wgpu::RenderState {
            device: device.clone(),
            queue: queue.clone(),
            target_format: wgpu::TextureFormat::Rgba8UnormSrgb,
            renderer,
            available_adapters: std::sync::Arc::new([]),
            adapter: adapter_arc,
        };

        let mesh = crate::mesh::Mesh::create_cube_mesh("TestCube", 1.0);
        let mut viewport = WgpuViewport::new(&render_state, &mesh);

        let res = 256;
        let mut project = crate::project::Project::new_default(res, res);
        project.mesh = mesh;
        viewport.sync_textures(&mut project);

        let mut engine = crate::brush::BrushEngine::new();
        engine.settings.radius = 16.0;
        engine.settings.flow = 1.0;
        engine.settings.opacity = 1.0;
        engine.settings.paint_color = glam::Vec4::new(0.0, 0.2, 1.0, 1.0);
        engine.settings.paint_roughness = 0.12;
        engine.settings.paint_metallic = 0.88;
        engine.update_cache();

        // 1. Dispatch 2D GPU compute stamp at UV center (0.5, 0.5)
        let uniform_2d = engine.build_gpu_stamp_data_2d(glam::Vec2::new(0.5, 0.5), res, res)
            .expect("2D stamp data must be constructed");
        viewport.stamp_2d_gpu(0, &uniform_2d);

        // 2. Perform incremental GPU composite to update viewport materials directly in VRAM
        viewport.composite_and_update_active(&project, 0);

        // 3. Readback modified stroke bounding rect to CPU layer
        let min_x = uniform_2d.bounds_min[0] as u32;
        let min_y = uniform_2d.bounds_min[1] as u32;
        let max_x = uniform_2d.bounds_max[0] as u32;
        let max_y = uniform_2d.bounds_max[1] as u32;

        let layer = &mut project.layers[0];
        viewport.readback_layer_rect(0, layer, min_x, min_y, max_x, max_y, engine.settings.channels);

        // 4. Assert stamped pixel inside the brush radius received blue color and roughness
        let center_x = res / 2;
        let center_y = res / 2;
        let painted_col = layer.base_color_buf.get_pixel(center_x, center_y);
        let painted_r = layer.roughness_buf.get_pixel(center_x, center_y);
        let painted_m = layer.metallic_buf.get_pixel(center_x, center_y);

        assert!(painted_col.z > 0.7, "Center pixel must receive blue color from GPU compute stamp, got b={}", painted_col.z);
        assert!(painted_r < 0.3, "Center pixel roughness must be updated by GPU stamp, got r={}", painted_r);
        assert!(painted_m > 0.6, "Center pixel metallic must be updated by GPU stamp, got m={}", painted_m);

        // 5. Assert untouched corner pixel (0, 0) remains base material
        let corner_col = layer.base_color_buf.get_pixel(0, 0);
        assert_eq!(corner_col, glam::Vec4::ONE, "Corner pixel outside brush must remain untouched default white");
    }

    #[test]
    fn test_wgpu_viewport_gpu_profiler_telemetry() {
        let instance = wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })) {
            Some(a) => a,
            None => {
                eprintln!("Skipping test_wgpu_viewport_gpu_profiler_telemetry: no GPU adapter");
                return;
            }
        };

        let mut req_features = wgpu::Features::empty();
        if adapter.features().contains(wgpu::Features::FLOAT32_FILTERABLE) {
            req_features |= wgpu::Features::FLOAT32_FILTERABLE;
        } else {
            eprintln!("Skipping test_wgpu_viewport_gpu_profiler_telemetry: FLOAT32_FILTERABLE missing");
            return;
        }
        if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            req_features |= wgpu::Features::TIMESTAMP_QUERY;
        }
        if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES) {
            req_features |= wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES;
        }

        let (device, queue) = match pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Profiler Test Device"),
                required_features: req_features,
                required_limits: wgpu::Limits::default(),
            },
            None,
        )) {
            Ok(dq) => dq,
            Err(e) => {
                eprintln!("Skipping test_wgpu_viewport_gpu_profiler_telemetry: {}", e);
                return;
            }
        };

        let device = std::sync::Arc::new(device);
        let queue = std::sync::Arc::new(queue);
        let adapter_arc = std::sync::Arc::new(adapter);
        let renderer = std::sync::Arc::new(eframe::egui::mutex::RwLock::new(
            egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb, None, 1),
        ));

        let render_state = eframe::egui_wgpu::RenderState {
            device: device.clone(),
            queue: queue.clone(),
            target_format: wgpu::TextureFormat::Rgba8UnormSrgb,
            renderer,
            available_adapters: std::sync::Arc::new([]),
            adapter: adapter_arc,
        };

        let mesh = crate::mesh::Mesh::create_cube_mesh("Cube", 1.0);
        let mut viewport = WgpuViewport::new(&render_state, &mesh);

        let res = 256;
        let mut project = crate::project::Project::new_default(res, res);
        project.mesh = mesh;
        viewport.sync_textures(&mut project);

        // Render 10 consecutive frames to exercise double-buffered lifecycle and readback
        let vp = glam::Mat4::IDENTITY;
        let eye = glam::Vec4::new(0.0, 0.0, 3.0, 1.0);
        let shader = ShaderSettings::default();
        for _ in 0..10 {
            viewport.render(vp, eye, 0.0, 0, 0.0, 1.0, &shader, false);
        }

        let timings = viewport.stage_timings();
        println!("GPU Stage Timings: {:?}", timings);

        // Verify telemetry fields are valid non-negative floats
        assert!(timings.pbr_pass_ms >= 0.0, "PBR pass time must be non-negative");
        assert!(timings.postprocess_pass_ms >= 0.0, "Postprocess pass time must be non-negative");
        assert!(timings.total_gpu_ms >= 0.0, "Total GPU time must be non-negative");
    }

    #[test]
    fn test_async_staging_readback_non_blocking_and_flush() {
        let instance = wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })) {
            Some(a) => a,
            None => {
                eprintln!("Skipping test_async_staging_readback_non_blocking_and_flush: no adapter");
                return;
            }
        };

        let mut req_features = wgpu::Features::FLOAT32_FILTERABLE;
        if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            req_features |= wgpu::Features::TIMESTAMP_QUERY;
        }
        if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES) {
            req_features |= wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES;
        }

        let (device, queue) = match pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Async Readback Test Device"),
                required_features: req_features,
                required_limits: wgpu::Limits::default(),
            },
            None,
        )) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("Skipping test_async_staging_readback_non_blocking_and_flush: {}", e);
                return;
            }
        };

        let device = std::sync::Arc::new(device);
        let queue = std::sync::Arc::new(queue);
        let adapter_arc = std::sync::Arc::new(adapter);
        let renderer = std::sync::Arc::new(eframe::egui::mutex::RwLock::new(
            egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb, None, 1),
        ));

        let render_state = eframe::egui_wgpu::RenderState {
            device: device.clone(),
            queue: queue.clone(),
            target_format: wgpu::TextureFormat::Rgba8UnormSrgb,
            renderer,
            available_adapters: std::sync::Arc::new([]),
            adapter: adapter_arc,
        };

        let mesh = crate::mesh::Mesh::create_cube_mesh("TestCubeAsync", 1.0);
        let mut viewport = WgpuViewport::new(&render_state, &mesh);

        let res = 256;
        let mut project = crate::project::Project::new_default(res, res);
        project.mesh = mesh;
        viewport.sync_textures(&mut project);

        let mut engine = crate::brush::BrushEngine::new();
        engine.settings.radius = 20.0;
        engine.settings.flow = 1.0;
        engine.settings.opacity = 1.0;
        engine.settings.paint_color = glam::Vec4::new(0.0, 1.0, 0.0, 1.0); // Pure Green
        engine.settings.paint_roughness = 0.25;
        engine.settings.paint_metallic = 0.75;
        engine.update_cache();

        let uniform_2d = engine.build_gpu_stamp_data_2d(glam::Vec2::new(0.5, 0.5), res, res)
            .expect("2D stamp data must be constructed");
        viewport.stamp_2d_gpu(0, &uniform_2d);
        viewport.composite_and_update_active(&project, 0);

        let min_x = uniform_2d.bounds_min[0] as u32;
        let min_y = uniform_2d.bounds_min[1] as u32;
        let max_x = uniform_2d.bounds_max[0] as u32;
        let max_y = uniform_2d.bounds_max[1] as u32;

        let layer = &mut project.layers[0];
        assert!(!viewport.has_pending_readback(), "No readback should be in flight before start");

        // Initiate async readback
        viewport.start_async_readback(
            0,
            layer,
            min_x,
            min_y,
            max_x,
            max_y,
            engine.settings.channels,
            "Async Green Stamp",
        );

        assert!(viewport.has_pending_readback(), "Pending readback flag must be true after start_async_readback");

        // Flush and complete readback
        let completed = viewport.flush_async_readback(&mut project)
            .expect("flush_async_readback must return completed readback result");

        assert_eq!(completed.description, "Async Green Stamp");
        assert_eq!(completed.layer_idx, 0);
        assert!(!viewport.has_pending_readback(), "Pending readback flag must be false after completion");

        // Verify pixel data
        let center_x = res / 2;
        let center_y = res / 2;
        let painted_col = project.layers[0].base_color_buf.get_pixel(center_x, center_y);
        let painted_r = project.layers[0].roughness_buf.get_pixel(center_x, center_y);
        let painted_m = project.layers[0].metallic_buf.get_pixel(center_x, center_y);

        assert!(painted_col.y > 0.7, "Center pixel must receive green color from async readback, got g={}", painted_col.y);
        assert!(painted_r < 0.35, "Center pixel roughness must be updated by async readback, got r={}", painted_r);
        assert!(painted_m > 0.65, "Center pixel metallic must be updated by async readback, got m={}", painted_m);
    }
}


