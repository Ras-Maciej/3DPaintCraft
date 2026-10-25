// 3D Paintcraft - GPU-Accelerated Mesh Map Baker
// Full-speed hardware rasterization and compute ray-tracing for texture maps

use std::sync::Arc;
use eframe::wgpu;
use eframe::wgpu::util::DeviceExt;
use glam::Vec4;
use rayon::prelude::*;

use crate::baker::{AoBakeConfig, CurvatureBakeConfig, PositionBakeConfig, PositionNormalizationMode, ThicknessBakeConfig};
use crate::channel::{RgbaBuffer, ScalarBuffer};
use crate::mesh::{GpuVertex, Mesh};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BakeUniforms {
    pub min_bound: [f32; 4],
    pub max_bound: [f32; 4],
    pub mode: u32,
    pub samples: u32,
    pub invert: u32,
    pub submesh_id: u32,
    pub tex_size: [f32; 2],
    pub pad0: f32,
    pub pad1: f32,
}

#[derive(Clone)]
pub struct GpuBaker {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    render_pipeline: Arc<wgpu::RenderPipeline>,
    render_bind_group_layout: Arc<wgpu::BindGroupLayout>,
}

impl GpuBaker {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GPU Baker Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_baker.wgsl").into()),
        });

        // Render Bind Group Layout (Uniform buffer for vertex & fragment rasterization)
        let render_bind_group_layout = Arc::new(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bake Render BGL"),
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
        }));

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bake Render Pipeline Layout"),
            bind_group_layouts: &[&render_bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 24, shader_location: 2 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 40, shader_location: 3 },
            ],
        };

        let render_pipeline = Arc::new(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Bake Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_bake",
                buffers: &[vertex_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_bake",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Disable culling so all UV islands rasterize regardless of winding
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        }));

        Self {
            device,
            queue,
            render_pipeline,
            render_bind_group_layout,
        }
    }

    fn create_render_target(&self, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bake RGBA Target"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn read_texture_rgba(&self, texture: &wgpu::Texture, width: u32, height: u32) -> RgbaBuffer {
        let unpadded_bytes_per_row = width * 16;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * height) as u64;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bake Staging RGBA"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Bake Readback Encoder"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = sender.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver.recv().expect("Failed to receive GPU map signal").expect("Failed to map GPU staging buffer");

        let data = buffer_slice.get_mapped_range();
        let mut rgba_data = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_end = row_start + unpadded_bytes_per_row as usize;
            let row_slice = &data[row_start..row_end];
            let float_slice: &[f32] = bytemuck::cast_slice(row_slice);
            for pixel in float_slice.chunks_exact(4) {
                rgba_data.push(Vec4::new(pixel[0], pixel[1], pixel[2], pixel[3]));
            }
        }

        drop(data);
        staging_buffer.unmap();

        RgbaBuffer {
            width,
            height,
            default_val: Vec4::ZERO,
            data: rgba_data,
        }
    }

    fn render_mesh_pass(
        &self,
        mesh: &Mesh,
        custom_vertices: Option<&[GpuVertex]>,
        width: u32,
        height: u32,
        mut uniforms: BakeUniforms,
        clear_color: wgpu::Color,
        separate_submeshes: bool,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let (target_tex, target_view) = self.create_render_target(width, height);

        if mesh.indices.is_empty() || mesh.vertices.is_empty() {
            return (target_tex, target_view);
        }

        let gpu_vertices: Vec<GpuVertex> = if let Some(cv) = custom_vertices {
            cv.to_vec()
        } else {
            mesh.vertices.iter().map(GpuVertex::from).collect()
        };

        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bake Pass VBO"),
            contents: bytemuck::cast_slice(&gpu_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bake Pass IBO"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        uniforms.tex_size = [width as f32, height as f32];

        let (_ubos, bind_groups): (Vec<_>, Vec<_>) = if separate_submeshes && !mesh.submeshes.is_empty() {
            mesh.submeshes.iter().enumerate().map(|(sm_idx, _)| {
                let mut sm_uniforms = uniforms;
                sm_uniforms.submesh_id = sm_idx as u32;

                let ubo = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Bake Submesh UBO"),
                    contents: bytemuck::cast_slice(&[sm_uniforms]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Bake Submesh BG"),
                    layout: &self.render_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ubo.as_entire_binding(),
                    }],
                });

                (ubo, bind_group)
            }).unzip()
        } else {
            let ubo = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Bake Pass UBO"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bake Pass BG"),
                layout: &self.render_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ubo.as_entire_binding(),
                }],
            });

            (vec![ubo], vec![bind_group])
        };

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Bake Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bake Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            if separate_submeshes && !mesh.submeshes.is_empty() {
                for (sm_idx, sm) in mesh.submeshes.iter().enumerate() {
                    render_pass.set_bind_group(0, &bind_groups[sm_idx], &[]);
                    let start = sm.start_index as u32;
                    let count = sm.index_count as u32;
                    let total = mesh.indices.len() as u32;
                    let end = (start + count).min(total);
                    if end > start {
                        render_pass.draw_indexed(start..end, 0, 0..1);
                    }
                }
            } else {
                render_pass.set_bind_group(0, &bind_groups[0], &[]);
                render_pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        (target_tex, target_view)
    }

    /// Fast UV Island Dilation across RGBA maps
    pub fn dilate_rgba(buffer: &mut RgbaBuffer, passes: usize) {
        let width = buffer.width as usize;
        let height = buffer.height as usize;
        let mut current = buffer.data.clone();
        let mut scratch = current.clone();

        for _ in 0..passes {
            scratch.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
                for x in 0..width {
                    let idx = y * width + x;
                    if current[idx].w < 0.5 {
                        for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                            let nx = x as isize + dx;
                            let ny = y as isize + dy;
                            if nx >= 0 && nx < width as isize && ny >= 0 && ny < height as isize {
                                let n_idx = (ny as usize) * width + (nx as usize);
                                if current[n_idx].w >= 0.5 {
                                    row[x] = current[n_idx];
                                    break;
                                }
                            }
                        }
                    }
                }
            });
            current.copy_from_slice(&scratch);
        }

        current.par_iter_mut().for_each(|px| {
            px.w = 1.0;
        });
        buffer.data = current;
    }

    /// Fast UV Island Dilation across scalar maps
    pub fn dilate_scalar(buffer: &mut ScalarBuffer, passes: usize, unwritten_val: f32) {
        let width = buffer.width as usize;
        let height = buffer.height as usize;
        let mut current = buffer.data.clone();
        let mut scratch = current.clone();

        for _ in 0..passes {
            scratch.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
                for x in 0..width {
                    let idx = y * width + x;
                    if (current[idx] - unwritten_val).abs() < 1e-4 {
                        for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                            let nx = x as isize + dx;
                            let ny = y as isize + dy;
                            if nx >= 0 && nx < width as isize && ny >= 0 && ny < height as isize {
                                let n_idx = (ny as usize) * width + (nx as usize);
                                if (current[n_idx] - unwritten_val).abs() >= 1e-4 {
                                    row[x] = current[n_idx];
                                    break;
                                }
                            }
                        }
                    }
                }
            });
            current.copy_from_slice(&scratch);
        }
        buffer.data = current;
    }

    /// Hardware GPU-Bake World Space Normal Map
    pub fn bake_world_space_normal(&self, mesh: &Mesh, width: u32, height: u32) -> RgbaBuffer {
        let uniforms = BakeUniforms {
            min_bound: [mesh.aabb_min.x, mesh.aabb_min.y, mesh.aabb_min.z, 1.0],
            max_bound: [mesh.aabb_max.x, mesh.aabb_max.y, mesh.aabb_max.z, 0.01],
            mode: 0, // World Space Normal
            samples: 1,
            invert: 0,
            submesh_id: 0,
            tex_size: [width as f32, height as f32],
            pad0: 0.0,
            pad1: 0.0,
        };

        let (tex, _) = self.render_mesh_pass(
            mesh,
            None,
            width,
            height,
            uniforms,
            wgpu::Color { r: 0.5, g: 0.5, b: 1.0, a: 0.0 },
            false,
        );

        let mut buf = self.read_texture_rgba(&tex, width, height);
        Self::dilate_rgba(&mut buf, 16);
        buf
    }

    /// Hardware GPU-Bake World Position (WPO) / Bounding Box Map
    pub fn bake_position(&self, mesh: &Mesh, width: u32, height: u32, config: &PositionBakeConfig) -> RgbaBuffer {
        let mode = if config.mode == PositionNormalizationMode::BoundingBox { 1 } else { 2 };
        let uniforms = BakeUniforms {
            min_bound: [mesh.aabb_min.x, mesh.aabb_min.y, mesh.aabb_min.z, 1.0],
            max_bound: [mesh.aabb_max.x, mesh.aabb_max.y, mesh.aabb_max.z, 0.01],
            mode,
            samples: 1,
            invert: 0,
            submesh_id: 0,
            tex_size: [width as f32, height as f32],
            pad0: 0.0,
            pad1: 0.0,
        };

        let (tex, _) = self.render_mesh_pass(
            mesh,
            None,
            width,
            height,
            uniforms,
            wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 },
            false,
        );

        let mut buf = self.read_texture_rgba(&tex, width, height);
        Self::dilate_rgba(&mut buf, 16);
        buf
    }

    /// Hardware GPU-Bake Flat Tangent Space Normal Map
    pub fn bake_tangent_normal(&self, mesh: &Mesh, width: u32, height: u32, invert_green: bool) -> RgbaBuffer {
        let uniforms = BakeUniforms {
            min_bound: [0.0; 4],
            max_bound: [0.0; 4],
            mode: 3, // Flat Tangent Normal
            samples: 1,
            invert: if invert_green { 1 } else { 0 },
            submesh_id: 0,
            tex_size: [width as f32, height as f32],
            pad0: 0.0,
            pad1: 0.0,
        };

        let (tex, _) = self.render_mesh_pass(
            mesh,
            None,
            width,
            height,
            uniforms,
            wgpu::Color { r: 0.5, g: 0.5, b: 1.0, a: 0.0 },
            false,
        );

        let mut buf = self.read_texture_rgba(&tex, width, height);
        Self::dilate_rgba(&mut buf, 16);
        buf
    }

    /// Hardware GPU-Bake ID Map (Per-Submesh / Material ID Palette)
    pub fn bake_id_map(&self, mesh: &Mesh, width: u32, height: u32) -> RgbaBuffer {
        let uniforms = BakeUniforms {
            min_bound: [0.0; 4],
            max_bound: [0.0; 4],
            mode: 5, // ID Map
            samples: 1,
            invert: 0,
            submesh_id: 0,
            tex_size: [width as f32, height as f32],
            pad0: 0.0,
            pad1: 0.0,
        };

        let (tex, _) = self.render_mesh_pass(
            mesh,
            None,
            width,
            height,
            uniforms,
            wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 },
            true, // Render submeshes individually with their submesh_id
        );

        let mut buf = self.read_texture_rgba(&tex, width, height);
        Self::dilate_rgba(&mut buf, 16);
        buf
    }

    /// Hardware GPU-Bake Curvature Map with Signed Dihedral & Deflection Analysis
    pub fn bake_curvature(&self, mesh: &Mesh, width: u32, height: u32, config: &CurvatureBakeConfig) -> ScalarBuffer {
        let vert_count = mesh.vertices.len();
        if vert_count == 0 || mesh.indices.is_empty() {
            return ScalarBuffer::new(width, height, 0.5);
        }

        // 1. Evaluate high-quality signed dihedral & normal deflection curvature per vertex
        let vert_curvature = crate::baker::Baker::compute_vertex_curvature(mesh, config);

        // 2. Upload vertex curvature in tangent.w
        let mut gpu_vertices: Vec<GpuVertex> = mesh.vertices.iter().map(GpuVertex::from).collect();
        for (gv, &curv) in gpu_vertices.iter_mut().zip(vert_curvature.iter()) {
            gv.tangent[3] = curv;
        }

        // 3. Hardware GPU UV Rasterization using mode 6 (Scalar Vertex Attribute)
        let uniforms = BakeUniforms {
            min_bound: [0.0; 4],
            max_bound: [0.0; 4],
            mode: 6, // Scalar Vertex Attribute
            samples: 1,
            invert: 0,
            submesh_id: 0,
            tex_size: [width as f32, height as f32],
            pad0: 0.0,
            pad1: 0.0,
        };

        let (tex, _) = self.render_mesh_pass(
            mesh,
            Some(&gpu_vertices),
            width,
            height,
            uniforms,
            wgpu::Color { r: 0.5, g: 0.5, b: 0.5, a: 0.0 },
            false,
        );

        let rgba_buf = self.read_texture_rgba(&tex, width, height);
        let scalar_data: Vec<f32> = rgba_buf.data.iter().map(|c| {
            if c.w < 0.5 {
                -1.0 // Unwritten marker
            } else {
                c.x.clamp(0.0, 1.0)
            }
        }).collect();

        let mut scalar_buf = ScalarBuffer {
            width,
            height,
            default_val: 0.5,
            data: scalar_data,
        };

        // 4. Dilate 16px into unwritten pixels
        Self::dilate_scalar(&mut scalar_buf, 16, -1.0);

        // 5. Apply configurable blur passes
        if config.blur_passes > 0 {
            crate::baker::Baker::blur_scalar(&mut scalar_buf, config.blur_passes);
            Self::dilate_scalar(&mut scalar_buf, 16, -1.0);
        }

        // 6. Replace any remaining unwritten background with neutral 0.5 gray
        scalar_buf.data.par_iter_mut().for_each(|v| {
            if *v < 0.0 {
                *v = 0.5;
            }
        });

        scalar_buf
    }

    /// Hardware GPU-Bake Ambient Occlusion Map
    pub fn bake_ao(&self, mesh: &Mesh, width: u32, height: u32, config: &AoBakeConfig) -> ScalarBuffer {
        let vert_count = mesh.vertices.len();
        if vert_count == 0 || mesh.indices.is_empty() {
            return ScalarBuffer::new(width, height, 1.0);
        }

        // 1. Rayon parallel Monte Carlo hemisphere occlusion against BVH with facet rejection
        let vert_ao = crate::baker::Baker::compute_vertex_ao(
            mesh,
            &crate::baker::BakeCageConfig::default(),
            config,
        ).unwrap_or_else(|_| vec![1.0; vert_count]);

        // 2. Upload vertex AO into tangent.w
        let mut gpu_vertices: Vec<GpuVertex> = mesh.vertices.iter().map(GpuVertex::from).collect();
        for (gv, &ao) in gpu_vertices.iter_mut().zip(vert_ao.iter()) {
            gv.tangent[3] = ao;
        }

        // 3. Hardware GPU UV Rasterization using mode 6 (Scalar Vertex Attribute)
        let uniforms = BakeUniforms {
            min_bound: [0.0; 4],
            max_bound: [0.0; 4],
            mode: 6, // Scalar Vertex Attribute
            samples: 1,
            invert: 0,
            submesh_id: 0,
            tex_size: [width as f32, height as f32],
            pad0: 0.0,
            pad1: 0.0,
        };

        let (tex, _) = self.render_mesh_pass(
            mesh,
            Some(&gpu_vertices),
            width,
            height,
            uniforms,
            wgpu::Color { r: 1.0, g: 1.0, b: 1.0, a: 0.0 },
            false,
        );

        let rgba_buf = self.read_texture_rgba(&tex, width, height);
        let scalar_data: Vec<f32> = rgba_buf.data.iter().map(|c| {
            if c.w < 0.5 { -1.0 } else { c.x.clamp(0.0, 1.0) }
        }).collect();

        let mut scalar_buf = ScalarBuffer { width, height, default_val: 1.0, data: scalar_data };
        Self::dilate_scalar(&mut scalar_buf, 16, -1.0);
        scalar_buf.data.par_iter_mut().for_each(|v| {
            if *v < 0.0 { *v = 1.0; }
        });
        scalar_buf
    }

    /// Hardware GPU-Bake Surface Thickness Map
    pub fn bake_thickness(&self, mesh: &Mesh, width: u32, height: u32, config: &ThicknessBakeConfig) -> ScalarBuffer {
        let vert_count = mesh.vertices.len();
        if vert_count == 0 || mesh.indices.is_empty() {
            let default_val = if config.invert { 1.0 } else { 0.0 };
            return ScalarBuffer::new(width, height, default_val);
        }

        // 1. Rayon parallel inward volumetric depth against BVH
        let vert_thick = crate::baker::Baker::compute_vertex_thickness(
            mesh,
            &crate::baker::BakeCageConfig::default(),
            config,
        ).unwrap_or_else(|_| vec![if config.invert { 1.0 } else { 0.0 }; vert_count]);

        // 2. Upload vertex thickness in tangent.w
        let mut gpu_vertices: Vec<GpuVertex> = mesh.vertices.iter().map(GpuVertex::from).collect();
        for (gv, &th) in gpu_vertices.iter_mut().zip(vert_thick.iter()) {
            gv.tangent[3] = th;
        }

        // 3. Hardware GPU UV Rasterization using mode 6 (Scalar Vertex Attribute)
        let uniforms = BakeUniforms {
            min_bound: [0.0; 4],
            max_bound: [0.0; 4],
            mode: 6, // Scalar Vertex Attribute
            samples: 1,
            invert: 0,
            submesh_id: 0,
            tex_size: [width as f32, height as f32],
            pad0: 0.0,
            pad1: 0.0,
        };

        let default_val = if config.invert { 0.0 } else { 1.0 };
        let (tex, _) = self.render_mesh_pass(
            mesh,
            Some(&gpu_vertices),
            width,
            height,
            uniforms,
            wgpu::Color { r: default_val as f64, g: default_val as f64, b: default_val as f64, a: 0.0 },
            false,
        );

        let rgba_buf = self.read_texture_rgba(&tex, width, height);
        let scalar_data: Vec<f32> = rgba_buf.data.iter().map(|c| {
            if c.w < 0.5 { -1.0 } else { c.x.clamp(0.0, 1.0) }
        }).collect();

        let mut scalar_buf = ScalarBuffer { width, height, default_val, data: scalar_data };
        Self::dilate_scalar(&mut scalar_buf, 16, -1.0);
        scalar_buf.data.par_iter_mut().for_each(|v| {
            if *v < 0.0 { *v = default_val; }
        });
        scalar_buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bake_data_layout() {
        assert_eq!(std::mem::size_of::<BakeUniforms>(), 64, "BakeUniforms must be 64 bytes for WebGPU uniform alignment");
    }

    #[test]
    fn test_dilate_rgba_expands_borders() {
        let mut buf = RgbaBuffer::new(4, 4, Vec4::new(0.0, 0.0, 0.0, 0.0));
        buf.set_pixel(1, 1, Vec4::new(1.0, 0.0, 0.0, 1.0));

        GpuBaker::dilate_rgba(&mut buf, 1);

        assert_eq!(buf.get_pixel(1, 0), Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(buf.get_pixel(0, 1), Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(buf.get_pixel(2, 1), Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(buf.get_pixel(1, 2), Vec4::new(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_dilate_scalar_expands_borders() {
        let mut buf = ScalarBuffer::new(4, 4, -1.0);
        buf.set_pixel(1, 1, 0.75);

        GpuBaker::dilate_scalar(&mut buf, 1, -1.0);

        assert_eq!(buf.get_pixel(1, 0), 0.75);
        assert_eq!(buf.get_pixel(0, 1), 0.75);
        assert_eq!(buf.get_pixel(2, 1), 0.75);
        assert_eq!(buf.get_pixel(1, 2), 0.75);
    }

    #[test]
    fn test_vertex_curvature_evaluation() {
        let cube = Mesh::create_cube_mesh("Cube", 1.0);
        let config = CurvatureBakeConfig::default();
        let vert_curv = crate::baker::Baker::compute_vertex_curvature(&cube, &config);

        assert_eq!(vert_curv.len(), cube.vertices.len());
        let max_curv = vert_curv.iter().cloned().fold(0.0f32, f32::max);
        let min_curv = vert_curv.iter().cloned().fold(1.0f32, f32::min);

        assert!(max_curv > 0.6, "Convex edges must produce highlight, got {}", max_curv);
        assert!(min_curv >= 0.0 && max_curv <= 1.0, "Curvature must be bounded in [0, 1]");
    }

    #[test]
    fn test_vertex_thickness_evaluation() {
        let sphere = Mesh::create_sphere(1.0, 16, 12);
        let config_norm = ThicknessBakeConfig {
            samples: 16,
            max_distance: 0.5,
            ray_bias: 0.005,
            invert: false,
        };
        let config_inv = ThicknessBakeConfig {
            samples: 16,
            max_distance: 0.5,
            ray_bias: 0.005,
            invert: true,
        };

        let thick_norm = crate::baker::Baker::compute_vertex_thickness(
            &sphere,
            &crate::baker::BakeCageConfig::default(),
            &config_norm,
        ).expect("Vertex thickness normal should succeed");

        let thick_inv = crate::baker::Baker::compute_vertex_thickness(
            &sphere,
            &crate::baker::BakeCageConfig::default(),
            &config_inv,
        ).expect("Vertex thickness inverted should succeed");

        for (&vn, &vi) in thick_norm.iter().zip(thick_inv.iter()) {
            assert!((vn + vi - 1.0).abs() < 1e-4, "Inverted vertex thickness must satisfy 1.0 - normal: vn={}, vi={}", vn, vi);
        }
    }

    #[test]
    fn test_gpu_baker_rebake_lifecycle() {
        let instance = eframe::wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&eframe::wgpu::RequestAdapterOptions {
            power_preference: eframe::wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })) {
            Some(a) => a,
            None => return,
        };

        let (device, queue) = match pollster::block_on(adapter.request_device(
            &eframe::wgpu::DeviceDescriptor {
                label: Some("Test Device"),
                required_features: eframe::wgpu::Features::FLOAT32_FILTERABLE,
                required_limits: eframe::wgpu::Limits::default(),
            },
            None,
        )) {
            Ok(dq) => dq,
            Err(_) => return,
        };

        let device = std::sync::Arc::new(device);
        let queue = std::sync::Arc::new(queue);
        let mesh = Mesh::create_cube_mesh("Cube", 1.0);

        // First bake
        let baker1 = GpuBaker::new(Arc::clone(&device), Arc::clone(&queue));
        let _ = baker1.bake_tangent_normal(&mesh, 64, 64, false);
        let _ = baker1.bake_world_space_normal(&mesh, 64, 64);
        let _ = baker1.bake_position(&mesh, 64, 64, &PositionBakeConfig::default());
        let _ = baker1.bake_curvature(&mesh, 64, 64, &CurvatureBakeConfig::default());
        let _ = baker1.bake_ao(&mesh, 64, 64, &AoBakeConfig::default());
        let _ = baker1.bake_thickness(&mesh, 64, 64, &ThicknessBakeConfig::default());
        let _ = baker1.bake_id_map(&mesh, 64, 64);

        // Rebake (second bake)
        let baker2 = GpuBaker::new(Arc::clone(&device), Arc::clone(&queue));
        let _ = baker2.bake_tangent_normal(&mesh, 64, 64, false);
        let _ = baker2.bake_world_space_normal(&mesh, 64, 64);
        let _ = baker2.bake_position(&mesh, 64, 64, &PositionBakeConfig::default());
        let _ = baker2.bake_curvature(&mesh, 64, 64, &CurvatureBakeConfig::default());
        let _ = baker2.bake_ao(&mesh, 64, 64, &AoBakeConfig::default());
        let _ = baker2.bake_thickness(&mesh, 64, 64, &ThicknessBakeConfig::default());
        let _ = baker2.bake_id_map(&mesh, 64, 64);
    }
}

