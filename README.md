# 3D Paintcraft

High-Performance PBR Texture Painting, Mesh Map Baking, and Path Tracing Suite

3D Paintcraft is an independent, production-grade 3D texture painting and mesh map baking application built from the ground up in pure, safe Rust. Designed for technical artists, game studios, and graphics engineers, 3D Paintcraft delivers a real-time, non-destructive authoring environment with hardware-accelerated GPU compute pipelines, BVH-accelerated spatial queries, Cook-Torrance microfacet BRDF shading, and an unbiased Iray path tracer.

---

## Core Capabilities

### 1. Viewport and Real-Time Shading Engine
- **Hardware-Accelerated Viewport**: Native WGPU backend supporting Vulkan (Linux/Windows) and DirectX 12 (Windows) with high-efficiency resource binding and zero-copy texture transfers.
- **Physical Cook-Torrance Specular BRDF**:
  - GGX normal distribution function (NDF) with microfacet heightfield simulation.
  - Smith geometric masking-shadowing function with correlated height-direction terms.
  - Fresnel-Schlick approximation with metallic energy conservation and custom F0 reflectance.
  - High Dynamic Range (HDR) Image-Based Lighting (IBL) with spherical harmonics and split-sum environmental radiance convolution.
- **Extended Physical Material Channels**:
  - Base Color (sRGB and Linear albedo)
  - Metallic and Roughness
  - Tangent-Space Normal Map (DirectX and OpenGL green-channel conventions)
  - Height and Displacement Relief
  - Ambient Occlusion (AO)
  - Emissive Radiance
  - Opacity (Stochastic Bayer dither, alpha cutout, and double-sided blending)
  - Clearcoat (Secondary reflective specular lobe with independent roughness)
  - Anisotropy (Directional highlight elongation with tangent-space vector alignment)
  - Translucency and Subsurface Scattering (Beer-Lambert volumetric absorption)
- **Unbiased Iray Path Tracer**:
  - Monte Carlo path tracing with importance-sampled GGX specular and cosine-weighted diffuse lobes.
  - ACES filmic tonemapping for wide-gamut dynamic range reproduction.
  - Normal-guided bilateral edge-preserving denoiser for interactive progressive convergence.

### 2. Interactive 3D Projection Painting Engine
- **Raycast Geometry Intersection**: Real-time Möller-Trumbore raycasting into Bounding Volume Hierarchy (BVH) acceleration structures with barycentric UV interpolation.
- **Zero-Latency Stroke Pipeline**: Dual-buffered GPU compute stamping achieving sub-millisecond dab latencies (0.67 ms at 4K resolution) for uninterrupted high-resolution stylus and tablet interaction.
- **Dual Viewport Synchronization**: Seamless concurrent editing between interactive 3D perspective viewport and unwrapped 2D UV texture canvas.
- **Mirror Symmetry and Brush Dynamics**: Real-time planar reflection symmetry across world axes, stroke distance tracking, follow-path tangent direction alignment, and procedural jitter.

### 3. Non-Destructive Multi-Channel Layer Stack
- **Layer Architecture**:
  - Paint Layers: Full-resolution raster canvases supporting multi-channel brush strokes and eraser blending.
  - Fill Layers: Parametric uniform surface properties or procedural noise generators.
  - Folder Layers: Group hierarchy with folder-level opacity, channel gating, and PassThrough compositing mode.
  - Mask Stacks: Grayscale mask layers driven by curvature, ambient occlusion, paint strokes, or procedural filters.
- **Compositing Blend Modes**: Normal, Multiply, Screen, Overlay, Linear Dodge (Add), Subtract, Darken, Lighten, and PassThrough.
- **Procedural Generators**: Fractional Brownian Motion (fBm) Perlin noise, Cellular and Worley grunge, directional micro-scratches, and procedural edge wear.
- **Smart Materials**: Physics-based layered material presets driving wear, rust, dirt, and edge chips through baked geometric curvature and ambient occlusion.

### 4. High-Performance Mesh Map Baking Pipeline
- **BVH-Accelerated Normal Map Projection**: High-poly to low-poly tangent normal baking utilizing O(log N) ray traversal, replacing traditional volumetric spatial grids to eliminate combinatorial complexity.
- **Mesh-Name Matching**: Automatic submesh pairing via configurable suffix filtering (e.g. `_low` to `_high`) preventing cross-projection artifacts on exploded assemblies.
- **Cage Support**: Parametric inflation with vertex normal welding and external cage Wavefront OBJ loading.
- **Ambient Occlusion and Thickness Baking**: Distance-pruned hemisphere raycasting against pre-computed BVH nodes with curvature-guided self-intersection rejection.
- **Dihedral Curvature Evaluation**: Fast topological face-adjacency angle calculation identifying acute ridges and crevices.
- **Accelerated UV Seam Dilation**: Multi-threaded parallel edge dilation expanding texture boundaries across UV seams to eliminate MIP-mapping bleeding artifacts.

### 5. Production Pipeline and Engine Interchange
- **Mesh Asset Support**: Wavefront OBJ, glTF 2.0 (JSON and binary GLB) with automatic vertex normal and tangent-space orthogonalization.
- **Custom Texture Packing System**:
  - Fully configurable interactive channel matrix routing arbitrary source channels (Base Color R/G/B/A/Luma, Roughness, Smoothness, Metallic, AO, Height, Tangent Normals with DirectX/OpenGL switch, Baked Curvature/Thickness/World Normal/Position/ID, Emissive, and constants) into target output channels.
  - Per-channel inversion toggles (`1 - x`) for direct Smoothness/Roughness and DirectX/OpenGL normal conventions.
  - Flexible color modes: RGB (3-channel 24bpp), RGBA (4-channel 32bpp), and Grayscale (single-channel 8bpp).
  - Formats: Lossless PNG and JPEG outputs with configurable parallel UV seam dilation padding (0 to 32 pixels).
  - JSON Preset Management: Save, load, duplicate, and share studio-specific output packing presets (`.json`).
  - Pre-configured production templates: Unreal Engine 4/5 ORM, Unity HDRP MaskMap, Unity URP MetallicSmoothness, Godot 4 StandardMaterial3D, Blender/glTF 2.0, CryEngine DDNA, and Universal Isolated Channels.
- **Headless CLI Batch Processing**: Full automation support for CI/CD pipelines, automated asset ingestion, and smart material application without graphical display requirements.
- **Model Context Protocol (MCP)**: Native JSON-RPC 2.0 interface enabling external pipeline scripts and AI agents to query project state, apply materials, bake mesh maps, and trigger exports.

---

## Performance Benchmarks

Hardware Environment: NVIDIA GeForce RTX 4060 (Discrete GPU, Vulkan backend), AMD/Intel x86_64 Host Processor.

| Benchmark Operation | Document Resolution | Execution Time | Throughput |
| :--- | :--- | :--- | :--- |
| **GPU Brush Dab Compute** | 1024 x 1024 (1K) | 0.313 ms | 3,190 FPS equivalent |
| **GPU Brush Dab Compute** | 4096 x 4096 (4K) | 0.672 ms | 1,489 FPS equivalent |
| **BVH Raycast Query** | 3,072 Triangles | 0.22 microseconds | 4,500,000 rays/sec |
| **BVH Sphere Query** | 3,072 Triangles | 0.37 microseconds | 2,728,000 queries/sec |
| **Mesh Map Baking (Max Distance)** | High-Poly Sphere | 0.01 seconds | Instantaneous O(log N) |
| **UV Seam Dilation** | 2048 x 2048 (2K) | 14.8 milliseconds | Parallel Rayon Chunks |

---

## Downloads and Installation

Pre-compiled standalone releases are available for Windows and Linux on the [Releases](https://github.com/Ras-Maciej/3DPaintCraft/releases) page.

### Linux (AppImage and Tarball)
```bash
# Make the standalone AppImage executable
chmod +x Paintcraft-v0.2.0-x86_64.AppImage

# Launch application
./Paintcraft-v0.2.0-x86_64.AppImage
```

### Windows (Standalone Distribution)
1. Download `paintcraft-v0.2.0-windows-x86_64.zip`.
2. Extract the archive to your preferred directory.
3. Launch `paintcraft.exe`.

---

## Building from Source

### Prerequisites
- Rust compiler and Cargo (version 1.75 or later)
- C/C++ build tools and standard Vulkan or DirectX graphics drivers

### Build Steps
```bash
# Clone the repository
git clone https://github.com/Ras-Maciej/3DPaintCraft.git
cd 3DPaintCraft

# Build optimized release binary
cargo build --release

# The compiled binary will be located at:
# target/release/paintcraft (Linux) or target/release/paintcraft.exe (Windows)
```

### Cross-Compiling for Windows from Linux
```bash
# Add the Windows GNU target
rustup target add x86_64-pc-windows-gnu

# Compile via MinGW toolchain
cargo build --release --target x86_64-pc-windows-gnu
```

---

## Command-Line Interface and Automation

### Headless Batch Export
```bash
paintcraft --export UnrealEngine --out ./dist/textures --material worn_painted_steel
```

### Headless Custom Texture Packing
Pack textures using pre-configured engine templates or custom JSON preset configurations:
```bash
# Export using built-in Unreal Engine ORM packing
paintcraft --pack unreal_orm --out ./dist/textures

# Export using Unity HDRP MaskMap packing (Metallic, AO, Curvature, Smoothness)
paintcraft --pack unity_hdrp --out ./dist/textures

# Export using a studio-specific custom JSON packing definition
paintcraft --pack ./my_studio_preset.json --out ./dist/textures
```

### Model Context Protocol (MCP) Server
Launch standard I/O JSON-RPC interface for automation tooling:
```bash
paintcraft --mcp
```

### Hardware Diagnostics
Run comprehensive GPU adapter, memory, and raycast performance benchmarks:
```bash
paintcraft --diagnostics
```

---

## Architecture Overview

```
src/
├── main.rs                 # Application entry point, CLI parser, and window manager
├── app.rs                  # Studio interface, docking panels, and tool interaction
├── project.rs              # Document state machine, layer stack, and undo/redo history
├── mesh.rs                 # Geometry representations, BVH spatial hierarchy, OBJ/glTF parser
├── baker.rs                # Normal, AO, Curvature, and Thickness map baking engines
├── gpu_baker.rs            # Compute shader hardware baking rasterizer and pipelines
├── gpu_compositor.rs       # WGPU hardware compositing and layer blend pipelines
├── wgpu_renderer.rs        # Real-time PBR viewport renderer and camera controller
├── path_tracer.rs          # Unbiased progressive Monte Carlo path tracer with ACES tonemapping
├── channel.rs              # Multi-channel buffer data structures (RgbaBuffer, ScalarBuffer)
├── layer.rs                # Non-destructive layer evaluation, masking, and blend modes
├── brush.rs                # 3D projection stamping engine and stroke geometry
├── smart_materials.rs      # Physics-based PBR smart material preset library
├── asset_manager.rs        # Asynchronous asset discovery and material library management
├── file_dialog.rs          # Integrated visual filesystem navigation and model importer
├── packing.rs              # Custom texture packing matrix engine, preset management, and channel routing
├── export.rs               # Multi-engine texture export pipeline and channel packing
├── mcp.rs                  # Model Context Protocol (MCP) server implementation
└── utils.rs                # Procedural noise generation (fBm, Worley) and math utilities
```

---

## License

This project is licensed under either the MIT License or the Apache License (Version 2.0) at your option.
