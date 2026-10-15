// 3D Paintcraft - Hardware GPU Compute Shader 3D Projection Stamping Engine
// Executes 3D world-space triangle stamping directly on GPU cores in VRAM.

use bytemuck::{Pod, Zeroable};
use eframe::wgpu;
use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuBrushUniform {
    pub hit_pos: [f32; 4],     // xyz = hit point, w = world_radius
    pub hit_normal: [f32; 4],  // xyz = hit normal, w = hardness
    pub paint_color: [f32; 4], // rgba
    pub params: [f32; 4],      // x = roughness, y = metallic, z = height, w = flow * opacity
    pub bounds_min: [i32; 2],  // min_x, min_y
    pub bounds_max: [i32; 2],  // max_x, max_y
    pub tex_size: [f32; 2],    // width, height
    pub flags: [u32; 2],       // x = is_eraser, y = is_mask_target
    pub tri_count: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuBrushTriangle {
    pub pos0: [f32; 4],
    pub pos1: [f32; 4],
    pub pos2: [f32; 4],
    pub nrm0: [f32; 4],
    pub nrm1: [f32; 4],
    pub nrm2: [f32; 4],
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
    pub uv2: [f32; 2],
    pub _pad: [f32; 2],
}

pub struct GpuBrushEngine {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    triangle_buffer: wgpu::Buffer,
    max_triangles: usize,

    // Scratch textures for ping-pong compute output
    scratch_color: wgpu::Texture,
    scratch_normal: wgpu::Texture,
    scratch_roughness: wgpu::Texture,
    scratch_metallic: wgpu::Texture,
    scratch_color_view: wgpu::TextureView,
    scratch_normal_view: wgpu::TextureView,
    scratch_roughness_view: wgpu::TextureView,
    scratch_metallic_view: wgpu::TextureView,

    pub width: u32,
    pub height: u32,
}

impl GpuBrushEngine {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GPU Brush Stamp Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_brush_stamp.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GPU Brush Stamp Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GPU Brush Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("GPU Brush Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "cs_stamp",
            compilation_options: Default::default(),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Brush Uniform Buffer"),
            size: std::mem::size_of::<GpuBrushUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let max_triangles = 512;
        let triangle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Brush Triangle Buffer"),
            size: (max_triangles * std::mem::size_of::<GpuBrushTriangle>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (scratch_color, scratch_normal, scratch_roughness, scratch_metallic,
             scratch_color_view, scratch_normal_view, scratch_roughness_view, scratch_metallic_view) =
            Self::create_scratch_textures(&device, width, height);

        Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            uniform_buffer,
            triangle_buffer,
            max_triangles,
            scratch_color,
            scratch_normal,
            scratch_roughness,
            scratch_metallic,
            scratch_color_view,
            scratch_normal_view,
            scratch_roughness_view,
            scratch_metallic_view,
            width,
            height,
        }
    }

    fn create_scratch_textures(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (
        wgpu::Texture,
        wgpu::Texture,
        wgpu::Texture,
        wgpu::Texture,
        wgpu::TextureView,
        wgpu::TextureView,
        wgpu::TextureView,
        wgpu::TextureView,
    ) {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let desc_rgba = wgpu::TextureDescriptor {
            label: Some("GPU Brush Scratch RGBA"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let desc_r = wgpu::TextureDescriptor {
            label: Some("GPU Brush Scratch R"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let scratch_color = device.create_texture(&desc_rgba);
        let scratch_normal = device.create_texture(&desc_rgba);
        let scratch_roughness = device.create_texture(&desc_r);
        let scratch_metallic = device.create_texture(&desc_r);

        let scratch_color_view = scratch_color.create_view(&Default::default());
        let scratch_normal_view = scratch_normal.create_view(&Default::default());
        let scratch_roughness_view = scratch_roughness.create_view(&Default::default());
        let scratch_metallic_view = scratch_metallic.create_view(&Default::default());

        (
            scratch_color,
            scratch_normal,
            scratch_roughness,
            scratch_metallic,
            scratch_color_view,
            scratch_normal_view,
            scratch_roughness_view,
            scratch_metallic_view,
        )
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;

        let (sc_col, sc_norm, sc_r, sc_m, sc_col_v, sc_norm_v, sc_r_v, sc_m_v) =
            Self::create_scratch_textures(&self.device, width, height);

        self.scratch_color = sc_col;
        self.scratch_normal = sc_norm;
        self.scratch_roughness = sc_r;
        self.scratch_metallic = sc_m;
        self.scratch_color_view = sc_col_v;
        self.scratch_normal_view = sc_norm_v;
        self.scratch_roughness_view = sc_r_v;
        self.scratch_metallic_view = sc_m_v;
    }

    /// Dispatch GPU 3D Projection Stamping into a GPU Layer in VRAM
    pub fn stamp_3d_gpu(
        &mut self,
        gpu_layer: &crate::gpu_compositor::GpuLayer,
        uniforms: &GpuBrushUniform,
        triangles: &[GpuBrushTriangle],
    ) {
        if triangles.is_empty() {
            return;
        }

        let tri_count = triangles.len().min(self.max_triangles);
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));
        self.queue.write_buffer(&self.triangle_buffer, 0, bytemuck::cast_slice(&triangles[..tri_count]));

        let in_col_view = gpu_layer.base_color.create_view(&Default::default());
        let in_norm_view = gpu_layer.normal.create_view(&Default::default());
        let in_r_view = gpu_layer.roughness.create_view(&Default::default());
        let in_m_view = gpu_layer.metallic.create_view(&Default::default());

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GPU Brush Stamp Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.triangle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&in_col_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&in_norm_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&in_r_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&in_m_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&self.scratch_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&self.scratch_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&self.scratch_roughness_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&self.scratch_metallic_view),
                },
            ],
        });

        let min_x = uniforms.bounds_min[0].max(0) as u32;
        let min_y = uniforms.bounds_min[1].max(0) as u32;
        let max_x = (uniforms.bounds_max[0] as u32).min(self.width.saturating_sub(1));
        let max_y = (uniforms.bounds_max[1] as u32).min(self.height.saturating_sub(1));

        let bounds_w = (max_x.saturating_sub(min_x) + 1).min(self.width);
        let bounds_h = (max_y.saturating_sub(min_y) + 1).min(self.height);

        if bounds_w == 0 || bounds_h == 0 {
            return;
        }

        let dispatch_x = (bounds_w + 15) / 16;
        let dispatch_y = (bounds_h + 15) / 16;

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("GPU Brush Stamp Encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("GPU Brush Stamp Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }

        // Copy the modified bounding box back from scratch texture to layer texture in VRAM
        let copy_size = wgpu::Extent3d {
            width: bounds_w,
            height: bounds_h,
            depth_or_array_layers: 1,
        };
        let origin = wgpu::Origin3d {
            x: min_x,
            y: min_y,
            z: 0,
        };

        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &self.scratch_color,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &gpu_layer.base_color,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            copy_size,
        );

        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &self.scratch_normal,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &gpu_layer.normal,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            copy_size,
        );

        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &self.scratch_roughness,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &gpu_layer.roughness,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            copy_size,
        );

        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &self.scratch_metallic,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &gpu_layer.metallic,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            copy_size,
        );

        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
