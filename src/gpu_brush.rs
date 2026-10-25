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
    pub channel_mask: u32,
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

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ReadbackCompleted {
    pub layer_idx: usize,
    pub min_x: u32,
    pub min_y: u32,
    pub rect_w: u32,
    pub rect_h: u32,
    pub channels: crate::channel::ChannelFlags,
    pub description: String,
}

pub struct PendingReadbackBatch {
    pub layer_idx: usize,
    pub min_x: u32,
    pub min_y: u32,
    pub rect_w: u32,
    pub rect_h: u32,
    pub channels: crate::channel::ChannelFlags,
    pub staging_rgba: Option<wgpu::Buffer>,
    pub staging_normal: Option<wgpu::Buffer>,
    pub staging_roughness: Option<wgpu::Buffer>,
    pub staging_metallic: Option<wgpu::Buffer>,
    pub padded_bytes_rgba: u32,
    pub padded_bytes_scalar: u32,
    pub unpadded_bytes_rgba: u32,
    pub unpadded_bytes_scalar: u32,
    pub remaining: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    pub description: String,
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

    pending_readback: std::sync::Mutex<Option<PendingReadbackBatch>>,
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
            pending_readback: std::sync::Mutex::new(None),
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

    /// Dispatch GPU Projection or 2D Stamping into a GPU Layer in VRAM
    pub fn stamp_3d_gpu(
        &mut self,
        gpu_layer: &crate::gpu_compositor::GpuLayer,
        uniforms: &GpuBrushUniform,
        triangles: &[GpuBrushTriangle],
    ) {
        if uniforms.tri_count > 0 && triangles.is_empty() {
            return;
        }

        let tri_count = triangles.len().min(self.max_triangles);
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));
        if tri_count > 0 {
            self.queue.write_buffer(&self.triangle_buffer, 0, bytemuck::cast_slice(&triangles[..tri_count]));
        }

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

    pub fn has_pending_readback(&self) -> bool {
        self.pending_readback.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    pub fn pending_readback_layer_idx(&self) -> Option<usize> {
        self.pending_readback.lock().ok().and_then(|g| g.as_ref().map(|b| b.layer_idx))
    }

    /// Asynchronously initiate readback of a modified sub-rectangle from a GPU layer
    /// to staging buffers without stalling the CPU render thread.
    pub fn start_async_readback(
        &self,
        gpu_layer: &crate::gpu_compositor::GpuLayer,
        cpu_layer: &mut crate::layer::Layer,
        layer_idx: usize,
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
        channels: crate::channel::ChannelFlags,
        description: &str,
    ) {
        // If there was an earlier pending readback that wasn't polled yet, flush it first
        if self.has_pending_readback() {
            let _ = self.flush_async_readback(cpu_layer);
        }

        let min_x = min_x.min(self.width.saturating_sub(1));
        let min_y = min_y.min(self.height.saturating_sub(1));
        let max_x = max_x.min(self.width.saturating_sub(1));
        let max_y = max_y.min(self.height.saturating_sub(1));

        let rect_w = (max_x.saturating_sub(min_x) + 1).min(self.width.saturating_sub(min_x));
        let rect_h = (max_y.saturating_sub(min_y) + 1).min(self.height.saturating_sub(min_y));

        if rect_w == 0 || rect_h == 0 {
            return;
        }

        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

        // Ensure target CPU buffers are allocated
        cpu_layer.ensure_allocated_for_paint(crate::brush::PaintTarget::Material, channels, false);

        let copy_size = wgpu::Extent3d {
            width: rect_w,
            height: rect_h,
            depth_or_array_layers: 1,
        };
        let origin = wgpu::Origin3d { x: min_x, y: min_y, z: 0 };

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("GPU Readback Encoder"),
        });

        let mut num_mappings = 0usize;

        // 1. Staging Base Color (RGBA32Float)
        let unpadded_bytes_rgba = rect_w * 16;
        let padded_bytes_rgba = ((unpadded_bytes_rgba + align - 1) / align) * align;
        let staging_rgba = if channels.base_color {
            num_mappings += 1;
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Staging BaseColor"),
                size: (padded_bytes_rgba * rect_h) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    texture: &gpu_layer.base_color,
                    mip_level: 0,
                    origin,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &buf,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_rgba),
                        rows_per_image: Some(rect_h),
                    },
                },
                copy_size,
            );
            Some(buf)
        } else {
            None
        };

        // 2. Staging Normal (RGBA32Float)
        let staging_normal = if channels.normal {
            num_mappings += 1;
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Staging Normal"),
                size: (padded_bytes_rgba * rect_h) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    texture: &gpu_layer.normal,
                    mip_level: 0,
                    origin,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &buf,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_rgba),
                        rows_per_image: Some(rect_h),
                    },
                },
                copy_size,
            );
            Some(buf)
        } else {
            None
        };

        // 3. Staging Roughness (R32Float)
        let unpadded_bytes_scalar = rect_w * 4;
        let padded_bytes_scalar = ((unpadded_bytes_scalar + align - 1) / align) * align;
        let staging_roughness = if channels.roughness {
            num_mappings += 1;
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Staging Roughness"),
                size: (padded_bytes_scalar * rect_h) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    texture: &gpu_layer.roughness,
                    mip_level: 0,
                    origin,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &buf,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_scalar),
                        rows_per_image: Some(rect_h),
                    },
                },
                copy_size,
            );
            Some(buf)
        } else {
            None
        };

        // 4. Staging Metallic (R32Float)
        let staging_metallic = if channels.metallic {
            num_mappings += 1;
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Staging Metallic"),
                size: (padded_bytes_scalar * rect_h) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    texture: &gpu_layer.metallic,
                    mip_level: 0,
                    origin,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &buf,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_scalar),
                        rows_per_image: Some(rect_h),
                    },
                },
                copy_size,
            );
            Some(buf)
        } else {
            None
        };

        if num_mappings == 0 {
            return;
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        let remaining = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(num_mappings));

        if let Some(buf) = &staging_rgba {
            let r = remaining.clone();
            buf.slice(..).map_async(wgpu::MapMode::Read, move |_res| {
                r.fetch_sub(1, std::sync::atomic::Ordering::Release);
            });
        }
        if let Some(buf) = &staging_normal {
            let r = remaining.clone();
            buf.slice(..).map_async(wgpu::MapMode::Read, move |_res| {
                r.fetch_sub(1, std::sync::atomic::Ordering::Release);
            });
        }
        if let Some(buf) = &staging_roughness {
            let r = remaining.clone();
            buf.slice(..).map_async(wgpu::MapMode::Read, move |_res| {
                r.fetch_sub(1, std::sync::atomic::Ordering::Release);
            });
        }
        if let Some(buf) = &staging_metallic {
            let r = remaining.clone();
            buf.slice(..).map_async(wgpu::MapMode::Read, move |_res| {
                r.fetch_sub(1, std::sync::atomic::Ordering::Release);
            });
        }

        let batch = PendingReadbackBatch {
            layer_idx,
            min_x,
            min_y,
            rect_w,
            rect_h,
            channels,
            staging_rgba,
            staging_normal,
            staging_roughness,
            staging_metallic,
            padded_bytes_rgba,
            padded_bytes_scalar,
            unpadded_bytes_rgba,
            unpadded_bytes_scalar,
            remaining,
            description: description.to_string(),
        };

        if let Ok(mut lock) = self.pending_readback.lock() {
            *lock = Some(batch);
        }
    }

    /// Poll pending asynchronous staging readback. Non-blocking: returns None if GPU mapping is in flight.
    pub fn poll_async_readback(&self, cpu_layer: &mut crate::layer::Layer) -> Option<ReadbackCompleted> {
        // Run a maintain poll to advance asynchronous buffer mappings
        self.device.poll(wgpu::Maintain::Poll);

        let mut lock = self.pending_readback.lock().ok()?;
        let batch = lock.as_ref()?;

        if batch.remaining.load(std::sync::atomic::Ordering::Acquire) > 0 {
            return None;
        }

        // Ready! Extract batch and copy into cpu_layer
        let batch = lock.take().unwrap();
        drop(lock);

        let layer_w = cpu_layer.base_color_buf.width as usize;
        let rect_w = batch.rect_w;
        let rect_h = batch.rect_h;
        let min_x = batch.min_x;
        let min_y = batch.min_y;
        let max_x = min_x + rect_w.saturating_sub(1);
        let max_y = min_y + rect_h.saturating_sub(1);

        if let Some(buf) = &batch.staging_rgba {
            let slice = buf.slice(..);
            let mapped = slice.get_mapped_range();
            for y in 0..rect_h {
                let src_start = (y * batch.padded_bytes_rgba) as usize;
                let src_end = src_start + batch.unpadded_bytes_rgba as usize;
                let row_bytes = &mapped[src_start..src_end];
                let float_slice: &[f32] = bytemuck::cast_slice(row_bytes);
                let dst_row_start = (min_y + y) as usize * layer_w + min_x as usize;
                for (i, chunk) in float_slice.chunks_exact(4).enumerate() {
                    if dst_row_start + i < cpu_layer.base_color_buf.data.len() {
                        cpu_layer.base_color_buf.data[dst_row_start + i] =
                            glam::Vec4::new(chunk[0], chunk[1], chunk[2], chunk[3]);
                    }
                }
            }
            drop(mapped);
            buf.unmap();
        }

        if let Some(buf) = &batch.staging_normal {
            let slice = buf.slice(..);
            let mapped = slice.get_mapped_range();
            for y in 0..rect_h {
                let src_start = (y * batch.padded_bytes_rgba) as usize;
                let src_end = src_start + batch.unpadded_bytes_rgba as usize;
                let row_bytes = &mapped[src_start..src_end];
                let float_slice: &[f32] = bytemuck::cast_slice(row_bytes);
                let dst_row_start = (min_y + y) as usize * layer_w + min_x as usize;
                for (i, chunk) in float_slice.chunks_exact(4).enumerate() {
                    if dst_row_start + i < cpu_layer.normal_buf.data.len() {
                        cpu_layer.normal_buf.data[dst_row_start + i] =
                            glam::Vec4::new(chunk[0], chunk[1], chunk[2], chunk[3]);
                    }
                }
            }
            drop(mapped);
            buf.unmap();
        }

        if let Some(buf) = &batch.staging_roughness {
            let slice = buf.slice(..);
            let mapped = slice.get_mapped_range();
            for y in 0..rect_h {
                let src_start = (y * batch.padded_bytes_scalar) as usize;
                let src_end = src_start + batch.unpadded_bytes_scalar as usize;
                let row_bytes = &mapped[src_start..src_end];
                let float_slice: &[f32] = bytemuck::cast_slice(row_bytes);
                let dst_row_start = (min_y + y) as usize * layer_w + min_x as usize;
                for (i, &val) in float_slice.iter().enumerate() {
                    if dst_row_start + i < cpu_layer.roughness_buf.data.len() {
                        cpu_layer.roughness_buf.data[dst_row_start + i] = val;
                    }
                }
            }
            drop(mapped);
            buf.unmap();
        }

        if let Some(buf) = &batch.staging_metallic {
            let slice = buf.slice(..);
            let mapped = slice.get_mapped_range();
            for y in 0..rect_h {
                let src_start = (y * batch.padded_bytes_scalar) as usize;
                let src_end = src_start + batch.unpadded_bytes_scalar as usize;
                let row_bytes = &mapped[src_start..src_end];
                let float_slice: &[f32] = bytemuck::cast_slice(row_bytes);
                let dst_row_start = (min_y + y) as usize * layer_w + min_x as usize;
                for (i, &val) in float_slice.iter().enumerate() {
                    if dst_row_start + i < cpu_layer.metallic_buf.data.len() {
                        cpu_layer.metallic_buf.data[dst_row_start + i] = val;
                    }
                }
            }
            drop(mapped);
            buf.unmap();
        }

        cpu_layer.dirty_rect = Some((min_x, min_y, max_x, max_y));
        cpu_layer.dirty_flags = crate::layer::LayerDirtyFlags {
            color: batch.channels.base_color,
            roughness: batch.channels.roughness,
            metallic: batch.channels.metallic,
            normal: batch.channels.normal,
            mask: false,
        };

        Some(ReadbackCompleted {
            layer_idx: batch.layer_idx,
            min_x,
            min_y,
            rect_w,
            rect_h,
            channels: batch.channels,
            description: batch.description,
        })
    }

    /// Flush and wait for pending asynchronous staging readback to complete immediately.
    pub fn flush_async_readback(&self, cpu_layer: &mut crate::layer::Layer) -> Option<ReadbackCompleted> {
        if !self.has_pending_readback() {
            return None;
        }
        self.device.poll(wgpu::Maintain::Wait);
        self.poll_async_readback(cpu_layer)
    }

    /// Read back a modified sub-rectangle from a GPU layer directly into the CPU Layer
    /// for history undo tracking and file persistence. Synchronous wrapper.
    #[allow(dead_code)]
    pub fn readback_layer_rect(
        &self,
        gpu_layer: &crate::gpu_compositor::GpuLayer,
        cpu_layer: &mut crate::layer::Layer,
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
        channels: crate::channel::ChannelFlags,
    ) {
        self.start_async_readback(
            gpu_layer,
            cpu_layer,
            0,
            min_x,
            min_y,
            max_x,
            max_y,
            channels,
            "Sync Readback",
        );
        let _ = self.flush_async_readback(cpu_layer);
    }
}
