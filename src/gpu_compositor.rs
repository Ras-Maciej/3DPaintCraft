use eframe::wgpu;
use std::sync::Arc;

pub struct GpuLayer {
    pub id: u64,
    pub base_color: wgpu::Texture,
    pub normal: wgpu::Texture,
    pub roughness: wgpu::Texture,
    pub metallic: wgpu::Texture,
    pub mask: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
    pub param_buffer: wgpu::Buffer,
    pub param_bind_group: wgpu::BindGroup,
}

pub struct GpuCompositor {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    
    pipeline_color: wgpu::RenderPipeline,
    pipeline_scalar: wgpu::RenderPipeline,
    
    #[allow(dead_code)]
    bg_bind_group_layout: wgpu::BindGroupLayout,
    layer_bind_group_layout: wgpu::BindGroupLayout,
    params_bind_group_layout: wgpu::BindGroupLayout,
    
    sampler: wgpu::Sampler,
    
    // Ping-pong buffers
    pub buffer_a: CompositorTarget,
    pub buffer_b: CompositorTarget,

    // Sub-composite background cache (layers below active painting layer)
    pub cache_target: CompositorTarget,
    pub cached_up_to: Option<usize>,
    
    pub width: u32,
    pub height: u32,
}

pub struct CompositorTarget {
    pub base_color: wgpu::Texture,
    pub normal: wgpu::Texture,
    pub roughness: wgpu::Texture,
    pub metallic: wgpu::Texture,
    pub base_color_view: wgpu::TextureView,
    pub normal_view: wgpu::TextureView,
    pub roughness_view: wgpu::TextureView,
    pub metallic_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

impl GpuCompositor {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compositor Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compositor.wgsl").into()),
        });

        let bg_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compositor BG Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } }, count: None },
            ],
        });

        let layer_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compositor Layer Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 4, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: false } }, count: None },
            ],
        });

        let params_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compositor Params Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering), count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compositor Pipeline Layout"),
            bind_group_layouts: &[&bg_bind_group_layout, &layer_bind_group_layout, &params_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline_color = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Compositor Pipeline Color"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: "vs_main", buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_color",
                targets: &[
                    Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba32Float, blend: None, write_mask: wgpu::ColorWrites::ALL }),
                    Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba32Float, blend: None, write_mask: wgpu::ColorWrites::ALL }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let pipeline_scalar = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Compositor Pipeline Scalar"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: "vs_main", buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_scalar",
                targets: &[
                    Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::R32Float, blend: None, write_mask: wgpu::ColorWrites::ALL }),
                    Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::R32Float, blend: None, write_mask: wgpu::ColorWrites::ALL }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let buffer_a = Self::create_target(&device, width, height, &bg_bind_group_layout);
        let buffer_b = Self::create_target(&device, width, height, &bg_bind_group_layout);
        let cache_target = Self::create_target(&device, width, height, &bg_bind_group_layout);

        Self {
            device, queue, pipeline_color, pipeline_scalar, bg_bind_group_layout, layer_bind_group_layout, params_bind_group_layout,
            sampler, buffer_a, buffer_b, cache_target, cached_up_to: None, width, height
        }
    }

    fn create_target(device: &wgpu::Device, width: u32, height: u32, layout: &wgpu::BindGroupLayout) -> CompositorTarget {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let tex_base_color = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tgt BC"), size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_normal = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tgt N"), size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_roughness = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tgt R"), size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST, view_formats: &[] });
        let tex_metallic = device.create_texture(&wgpu::TextureDescriptor { label: Some("Tgt M"), size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST, view_formats: &[] });

        let base_color_view = tex_base_color.create_view(&Default::default());
        let normal_view = tex_normal.create_view(&Default::default());
        let roughness_view = tex_roughness.create_view(&Default::default());
        let metallic_view = tex_metallic.create_view(&Default::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tgt BG"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&base_color_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&normal_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&roughness_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&metallic_view) },
            ],
        });

        CompositorTarget {
            base_color: tex_base_color, normal: tex_normal, roughness: tex_roughness, metallic: tex_metallic,
            base_color_view, normal_view, roughness_view, metallic_view, bind_group
        }
    }

fn write_rgba_buffer(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    buf: &crate::channel::RgbaBuffer,
    origin: wgpu::Origin3d,
    size: wgpu::Extent3d,
    doc_width: u32,
) {
    if buf.is_sparse() {
        let chunk_rows = 128.min(size.height);
        let chunk = vec![buf.default_val; (size.width * chunk_rows) as usize];
        let chunk_bytes: &[u8] = bytemuck::cast_slice(&chunk);
        let bytes_per_row = size.width * 16;
        let mut y = 0;
        while y < size.height {
            let rows = chunk_rows.min(size.height - y);
            let chunk_size = wgpu::Extent3d {
                width: size.width,
                height: rows,
                depth_or_array_layers: 1,
            };
            let chunk_origin = wgpu::Origin3d {
                x: origin.x,
                y: origin.y + y,
                z: origin.z,
            };
            queue.write_texture(
                wgpu::ImageCopyTexture { texture, mip_level: 0, origin: chunk_origin, aspect: wgpu::TextureAspect::All },
                chunk_bytes,
                wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: Some(rows) },
                chunk_size,
            );
            y += rows;
        }
    } else {
        let byte_offset = ((origin.y * doc_width + origin.x) as u64) * 16;
        let bytes_per_row = doc_width * 16;
        queue.write_texture(
            wgpu::ImageCopyTexture { texture, mip_level: 0, origin, aspect: wgpu::TextureAspect::All },
            bytemuck::cast_slice(&buf.data),
            wgpu::ImageDataLayout { offset: byte_offset, bytes_per_row: Some(bytes_per_row), rows_per_image: None },
            size,
        );
    }
}

fn write_scalar_buffer(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    buf: &crate::channel::ScalarBuffer,
    origin: wgpu::Origin3d,
    size: wgpu::Extent3d,
    doc_width: u32,
) {
    if buf.is_sparse() {
        let chunk_rows = 128.min(size.height);
        let chunk = vec![buf.default_val; (size.width * chunk_rows) as usize];
        let chunk_bytes: &[u8] = bytemuck::cast_slice(&chunk);
        let bytes_per_row = size.width * 4;
        let mut y = 0;
        while y < size.height {
            let rows = chunk_rows.min(size.height - y);
            let chunk_size = wgpu::Extent3d {
                width: size.width,
                height: rows,
                depth_or_array_layers: 1,
            };
            let chunk_origin = wgpu::Origin3d {
                x: origin.x,
                y: origin.y + y,
                z: origin.z,
            };
            queue.write_texture(
                wgpu::ImageCopyTexture { texture, mip_level: 0, origin: chunk_origin, aspect: wgpu::TextureAspect::All },
                chunk_bytes,
                wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: Some(rows) },
                chunk_size,
            );
            y += rows;
        }
    } else {
        let byte_offset = ((origin.y * doc_width + origin.x) as u64) * 4;
        let bytes_per_row = doc_width * 4;
        queue.write_texture(
            wgpu::ImageCopyTexture { texture, mip_level: 0, origin, aspect: wgpu::TextureAspect::All },
            bytemuck::cast_slice(&buf.data),
            wgpu::ImageDataLayout { offset: byte_offset, bytes_per_row: Some(bytes_per_row), rows_per_image: None },
            size,
        );
    }
}

    #[allow(dead_code)]
    pub fn update_layer_full(&self, cpu_layer: &crate::layer::Layer, gpu_layer: &GpuLayer, width: u32, height: u32) {
        self.update_layer_full_selective(cpu_layer, gpu_layer, width, height, cpu_layer.dirty_flags);
    }

    pub fn update_layer_full_selective(
        &self,
        cpu_layer: &crate::layer::Layer,
        gpu_layer: &GpuLayer,
        width: u32,
        height: u32,
        flags: crate::layer::LayerDirtyFlags,
    ) {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let origin = wgpu::Origin3d::ZERO;
        if flags.color {
            Self::write_rgba_buffer(&self.queue, &gpu_layer.base_color, &cpu_layer.base_color_buf, origin, size, width);
        }
        if flags.normal {
            Self::write_rgba_buffer(&self.queue, &gpu_layer.normal, &cpu_layer.normal_buf, origin, size, width);
        }
        if flags.roughness {
            Self::write_scalar_buffer(&self.queue, &gpu_layer.roughness, &cpu_layer.roughness_buf, origin, size, width);
        }
        if flags.metallic {
            Self::write_scalar_buffer(&self.queue, &gpu_layer.metallic, &cpu_layer.metallic_buf, origin, size, width);
        }
        if flags.mask && cpu_layer.has_mask {
            Self::write_scalar_buffer(&self.queue, &gpu_layer.mask, &cpu_layer.mask_buf, origin, size, width);
        }
    }

    pub fn update_layer(&self, cpu_layer: &mut crate::layer::Layer, gpu_layer: &GpuLayer, width: u32, height: u32) {
        if let Some((x0, y0, x1, y1)) = cpu_layer.dirty_rect.take() {
            let flags = if cpu_layer.dirty_flags == crate::layer::LayerDirtyFlags::none() {
                crate::layer::LayerDirtyFlags::all()
            } else {
                cpu_layer.dirty_flags
            };
            cpu_layer.dirty_flags = crate::layer::LayerDirtyFlags::none();

            let rect_w = (x1.saturating_sub(x0) + 1).min(width.saturating_sub(x0));
            let rect_h = (y1.saturating_sub(y0) + 1).min(height.saturating_sub(y0));
            if rect_w == 0 || rect_h == 0 { return; }

            if rect_w == width && rect_h == height && x0 == 0 && y0 == 0 {
                self.update_layer_full_selective(cpu_layer, gpu_layer, width, height, flags);
            } else {
                let size = wgpu::Extent3d { width: rect_w, height: rect_h, depth_or_array_layers: 1 };
                let origin = wgpu::Origin3d { x: x0, y: y0, z: 0 };

                if flags.color {
                    Self::write_rgba_buffer(&self.queue, &gpu_layer.base_color, &cpu_layer.base_color_buf, origin, size, width);
                }
                if flags.normal {
                    Self::write_rgba_buffer(&self.queue, &gpu_layer.normal, &cpu_layer.normal_buf, origin, size, width);
                }
                if flags.roughness {
                    Self::write_scalar_buffer(&self.queue, &gpu_layer.roughness, &cpu_layer.roughness_buf, origin, size, width);
                }
                if flags.metallic {
                    Self::write_scalar_buffer(&self.queue, &gpu_layer.metallic, &cpu_layer.metallic_buf, origin, size, width);
                }
                if flags.mask && cpu_layer.has_mask {
                    Self::write_scalar_buffer(&self.queue, &gpu_layer.mask, &cpu_layer.mask_buf, origin, size, width);
                }
            }
        }
    }

    pub fn create_layer(&self, cpu_layer: &crate::layer::Layer, width: u32, height: u32) -> GpuLayer {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        
        let desc_rgba = wgpu::TextureDescriptor { label: None, size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] };
        let desc_r = wgpu::TextureDescriptor { label: None, size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[] };

        let base_color = self.device.create_texture(&desc_rgba);
        let normal = self.device.create_texture(&desc_rgba);
        let roughness = self.device.create_texture(&desc_r);
        let metallic = self.device.create_texture(&desc_r);
        let mask = self.device.create_texture(&desc_r);

        let origin = wgpu::Origin3d::ZERO;
        Self::write_rgba_buffer(&self.queue, &base_color, &cpu_layer.base_color_buf, origin, size, width);
        Self::write_rgba_buffer(&self.queue, &normal, &cpu_layer.normal_buf, origin, size, width);
        Self::write_scalar_buffer(&self.queue, &roughness, &cpu_layer.roughness_buf, origin, size, width);
        Self::write_scalar_buffer(&self.queue, &metallic, &cpu_layer.metallic_buf, origin, size, width);
        Self::write_scalar_buffer(&self.queue, &mask, &cpu_layer.mask_buf, origin, size, width);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Layer BG"),
            layout: &self.layer_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&base_color.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&normal.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&roughness.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&metallic.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&mask.create_view(&Default::default())) },
            ],
        });

        let param_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Layer Param Buffer"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let param_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Layer Param BG"),
            layout: &self.params_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 1, resource: param_buffer.as_entire_binding() },
            ],
        });

        GpuLayer {
            id: cpu_layer.id,
            base_color,
            normal,
            roughness,
            metallic,
            mask,
            bind_group,
            param_buffer,
            param_bind_group,
        }
    }

    pub fn invalidate_cache(&mut self) {
        self.cached_up_to = None;
    }

    fn clear_target(&self, encoder: &mut wgpu::CommandEncoder, target: &CompositorTarget, label: &'static str) {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment { view: &target.base_color_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } }),
                Some(wgpu::RenderPassColorAttachment { view: &target.normal_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.5, g: 0.5, b: 1.0, a: 0.5 }), store: wgpu::StoreOp::Store } }),
                Some(wgpu::RenderPassColorAttachment { view: &target.roughness_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 1.0, g: 0.0, b: 0.0, a: 0.0 }), store: wgpu::StoreOp::Store } }),
                Some(wgpu::RenderPassColorAttachment { view: &target.metallic_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } }),
            ],
            depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
        });
    }

    fn copy_target(&self, encoder: &mut wgpu::CommandEncoder, src: &CompositorTarget, dst: &CompositorTarget) {
        let size = wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 };
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &src.base_color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &dst.base_color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size,
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &src.normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &dst.normal, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size,
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &src.roughness, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &dst.roughness, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size,
        );
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture { texture: &src.metallic, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyTexture { texture: &dst.metallic, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            size,
        );
    }

    fn update_uniforms_for_layers(&self, all_layers: &[crate::layer::Layer], subset_cpu: &[crate::layer::Layer], subset_gpu: &[GpuLayer]) {
        for (cpu, gpu) in subset_cpu.iter().zip(subset_gpu.iter()) {
            if !cpu.visible || cpu.layer_type == crate::layer::LayerType::Folder { continue; }

            let parent_folder = cpu.parent_folder_id.and_then(|fid| all_layers.iter().find(|l| l.id == fid));
            if let Some(folder) = parent_folder {
                if !folder.visible { continue; }
            }

            let mut data = [0u8; 48];
            let mut opacity = cpu.opacity;
            if let Some(folder) = parent_folder {
                opacity *= folder.opacity;
            }
            let blend_mode = match cpu.blend_mode {
                crate::layer::BlendMode::Normal => 0u32,
                crate::layer::BlendMode::Multiply => 1u32,
                crate::layer::BlendMode::Add => 2u32,
                crate::layer::BlendMode::Subtract => 3u32,
                _ => 0u32,
            };
            let has_mask = if cpu.has_mask { 1u32 } else { 0u32 };
            let mask_inverted = if cpu.mask_inverted { 1u32 } else { 0u32 };
            let chan_color = if cpu.channel_flags.base_color { 1u32 } else { 0u32 };
            let chan_normal = if cpu.channel_flags.normal { 1u32 } else { 0u32 };
            let chan_roughness = if cpu.channel_flags.roughness { 1u32 } else { 0u32 };
            let chan_metallic = if cpu.channel_flags.metallic { 1u32 } else { 0u32 };
            let chan_height = if cpu.channel_flags.height { 1u32 } else { 0u32 };

            data[0..4].copy_from_slice(&opacity.to_le_bytes());
            data[4..8].copy_from_slice(&blend_mode.to_le_bytes());
            data[8..12].copy_from_slice(&has_mask.to_le_bytes());
            data[12..16].copy_from_slice(&mask_inverted.to_le_bytes());
            data[16..20].copy_from_slice(&chan_color.to_le_bytes());
            data[20..24].copy_from_slice(&chan_normal.to_le_bytes());
            data[24..28].copy_from_slice(&chan_roughness.to_le_bytes());
            data[28..32].copy_from_slice(&chan_metallic.to_le_bytes());
            data[32..36].copy_from_slice(&chan_height.to_le_bytes());

            self.queue.write_buffer(&gpu.param_buffer, 0, &data);
        }
    }

    fn render_composite_pass(&self, encoder: &mut wgpu::CommandEncoder, dst: &CompositorTarget, src: &CompositorTarget, gpu: &GpuLayer) {
        {
            let mut pass_color = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Composite Pass Color"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment { view: &dst.base_color_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } }),
                    Some(wgpu::RenderPassColorAttachment { view: &dst.normal_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } }),
                ],
                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
            });

            pass_color.set_pipeline(&self.pipeline_color);
            pass_color.set_bind_group(0, &src.bind_group, &[]);
            pass_color.set_bind_group(1, &gpu.bind_group, &[]);
            pass_color.set_bind_group(2, &gpu.param_bind_group, &[]);
            pass_color.draw(0..3, 0..1);
        }

        {
            let mut pass_scalar = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Composite Pass Scalar"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment { view: &dst.roughness_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } }),
                    Some(wgpu::RenderPassColorAttachment { view: &dst.metallic_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } }),
                ],
                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
            });

            pass_scalar.set_pipeline(&self.pipeline_scalar);
            pass_scalar.set_bind_group(0, &src.bind_group, &[]);
            pass_scalar.set_bind_group(1, &gpu.bind_group, &[]);
            pass_scalar.set_bind_group(2, &gpu.param_bind_group, &[]);
            pass_scalar.draw(0..3, 0..1);
        }
    }

    pub fn composite_all(&mut self, cpu_layers: &[crate::layer::Layer], gpu_layers: &[GpuLayer]) -> &CompositorTarget {
        self.invalidate_cache();
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Compositor All") });

        self.clear_target(&mut encoder, &self.buffer_a, "Clear A");
        self.clear_target(&mut encoder, &self.buffer_b, "Clear B");

        self.update_uniforms_for_layers(cpu_layers, cpu_layers, gpu_layers);

        let mut current_src = &self.buffer_a;
        let mut current_dst = &self.buffer_b;
        let mut using_a_as_src = true;

        for (cpu, gpu) in cpu_layers.iter().zip(gpu_layers.iter()) {
            if !cpu.visible || cpu.layer_type == crate::layer::LayerType::Folder { continue; }
            let parent_folder = cpu.parent_folder_id.and_then(|fid| cpu_layers.iter().find(|l| l.id == fid));
            if let Some(folder) = parent_folder {
                if !folder.visible { continue; }
            }

            self.render_composite_pass(&mut encoder, current_dst, current_src, gpu);

            using_a_as_src = !using_a_as_src;
            if using_a_as_src {
                current_src = &self.buffer_a;
                current_dst = &self.buffer_b;
            } else {
                current_src = &self.buffer_b;
                current_dst = &self.buffer_a;
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        current_src
    }

    /// Hierarchical Sub-Composite DAG: Fast-path for interactive brush painting.
    /// Reuses cached composite of all layers below `active_layer_idx` to achieve O(1) compositing.
    pub fn composite_incremental(
        &mut self,
        cpu_layers: &[crate::layer::Layer],
        gpu_layers: &[GpuLayer],
        active_layer_idx: usize,
    ) -> &CompositorTarget {
        if active_layer_idx == 0 || active_layer_idx >= cpu_layers.len() || cpu_layers.len() != gpu_layers.len() {
            return self.composite_all(cpu_layers, gpu_layers);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Compositor Incremental") });

        // Check if cache for layers below active_layer_idx is valid
        if self.cached_up_to != Some(active_layer_idx) {
            self.clear_target(&mut encoder, &self.buffer_a, "Clear A for Subcomposite");
            self.clear_target(&mut encoder, &self.buffer_b, "Clear B for Subcomposite");

            self.update_uniforms_for_layers(cpu_layers, &cpu_layers[0..active_layer_idx], &gpu_layers[0..active_layer_idx]);

            let mut current_src = &self.buffer_a;
            let mut current_dst = &self.buffer_b;
            let mut using_a = true;

            for (cpu, gpu) in cpu_layers[0..active_layer_idx].iter().zip(gpu_layers[0..active_layer_idx].iter()) {
                if !cpu.visible || cpu.layer_type == crate::layer::LayerType::Folder { continue; }
                let parent_folder = cpu.parent_folder_id.and_then(|fid| cpu_layers.iter().find(|l| l.id == fid));
                if let Some(folder) = parent_folder {
                    if !folder.visible { continue; }
                }

                self.render_composite_pass(&mut encoder, current_dst, current_src, gpu);

                using_a = !using_a;
                if using_a {
                    current_src = &self.buffer_a;
                    current_dst = &self.buffer_b;
                } else {
                    current_src = &self.buffer_b;
                    current_dst = &self.buffer_a;
                }
            }

            self.copy_target(&mut encoder, current_src, &self.cache_target);
            self.cached_up_to = Some(active_layer_idx);
        }

        // Copy cache_target into buffer_a to serve as the background for remaining layers
        self.copy_target(&mut encoder, &self.cache_target, &self.buffer_a);

        self.update_uniforms_for_layers(cpu_layers, &cpu_layers[active_layer_idx..], &gpu_layers[active_layer_idx..]);

        let mut current_src = &self.buffer_a;
        let mut current_dst = &self.buffer_b;
        let mut using_a = true;

        for (cpu, gpu) in cpu_layers[active_layer_idx..].iter().zip(gpu_layers[active_layer_idx..].iter()) {
            if !cpu.visible || cpu.layer_type == crate::layer::LayerType::Folder { continue; }
            let parent_folder = cpu.parent_folder_id.and_then(|fid| cpu_layers.iter().find(|l| l.id == fid));
            if let Some(folder) = parent_folder {
                if !folder.visible { continue; }
            }

            self.render_composite_pass(&mut encoder, current_dst, current_src, gpu);

            using_a = !using_a;
            if using_a {
                current_src = &self.buffer_a;
                current_dst = &self.buffer_b;
            } else {
                current_src = &self.buffer_b;
                current_dst = &self.buffer_a;
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        current_src
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shaders_valid_wgsl() {
        let shaders = [
            ("compositor.wgsl", include_str!("compositor.wgsl")),
            ("viewport.wgsl", include_str!("viewport.wgsl")),
            ("skybox.wgsl", include_str!("skybox.wgsl")),
            ("postprocess.wgsl", include_str!("postprocess.wgsl")),
            ("gpu_brush_stamp.wgsl", include_str!("gpu_brush_stamp.wgsl")),
            ("gpu_baker.wgsl", include_str!("gpu_baker.wgsl")),
        ];

        for (name, source) in shaders {
            let module = wgpu::naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|e| panic!("{} failed to parse: {}", name, e));
            let mut validator = wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::all(),
            );
            validator.validate(&module)
                .unwrap_or_else(|e| panic!("{} failed validation: {:?}", name, e));
        }
    }
}

