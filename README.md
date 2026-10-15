# 3D Paintcraft 🎨

**3D Paintcraft** is a clean-room recreation of Adobe Substance 3D Painter written in 100% pure, safe Rust for Linux, built using the core philosophy and techniques of the **ArtCraft** creative suite (such as *PhotoCraft* and *ArtCraft* by storytold).

---

## 🌟 Key Features

### 🖌️ 3D Viewport & Interactive Projection Painting
* **Real-time 3D Projection Painting**: Paint strokes directly onto the 3D surface using Möller–Trumbore raycasting into mesh geometry with barycentric UV coordinate interpolation.
* **Dual Viewport Layout**: Switch between 3D Viewport, 2D UV Canvas Viewport, or side-by-side split screen.
* **Physical Cook-Torrance GGX Microfacet Shading**:
  * GGX normal distribution (\(D\))
  * Smith geometric attenuation (\(G\))
  * Fresnel-Schlick approximation (\(F\))
  * Lambertian diffuse with metallic energy conservation
  * Hemisphere ambient sky/ground environment lighting
  * Interactive Sun light direction & elevation control
* **Multi-Channel View Modes**: Inspect final lit material, Base Color, Roughness, Metallic, Normal Map, Ambient Occlusion, Height, or Curvature.
* **Turntable Arcball Camera**: Smooth 3D navigation (Orbit with Right-Drag or Alt+Left-Drag, Pan with Middle-Drag, Zoom with Scroll Wheel, Frame with `F`).

### 📑 Multi-Channel Layer Stack
* **PBR Texture Channels**: Base Color, Roughness, Metallic, Normal, Height, Ambient Occlusion, and Emission.
* **Layer Types**:
  * **Paint Layer**: Raster canvas supporting continuous brush strokes, blending, and eraser mode.
  * **Fill Layer**: Uniform PBR parameters or procedural surface generators.
* **Blend Modes**: Normal, Multiply, Screen, Overlay, Linear Dodge (Add), Subtract, Darken, Lighten.
* **Per-Channel Toggles**: Enable/disable specific channels (e.g., paint only Roughness and Metallic, or only Base Color).
* **Layer Hierarchy**: Reorder layers (Move Up / Down), adjust individual opacity, and toggle visibility.

### ⚙️ Procedural Surface Generators & Smart Materials
* **Built-in Procedural Generators**:
  * Fractional Brownian Motion (fBm) Perlin Noise
  * Worley / Cellular Grunge
  * Directional Micro-Scratches
  * Curvature-driven Edge Wear
* **Smart Materials Presets**:
  * ⚡ **Worn Painted Steel**: Industrial yellow paint with chipped edges exposing bare gunmetal steel and primer.
  * ⚡ **Rusted Cast Iron**: Heavy dark cast iron with procedural surface pitting and orange rust.
  * ⚡ **Brushed Gold**: Luxury gold with fine anisotropic micro-scratches.
  * ⚡ **Matte Tactical Polymer**: High-durability military chassis polymer with fine stippling.
  * ⚡ **Weathered Cedar Wood**: Organic wood grain with tactile height grooves.
  * ⚡ **Sci-Fi Hull Plating**: Armor paneling with glowing cyan energy lines and edge wear.
  * ⚡ **Gloss Crimson Car Paint**: Multi-coat deep metallic crimson undercoat with high-gloss clear coat.

### 📐 Mesh Map Bakers
* **Curvature Baker**: Detects convex edges and concave crevices from face normal angles. Drives procedural edge-wear and smart materials.
* **Ambient Occlusion (AO) Baker**: Approximates geometric self-shadowing and depth occlusion.
* **World Space Normal Baker**: Encodes vertex world normals into RGB texture space.

### 📦 Game Engine Texture Exporter
* **Unreal Engine Preset**:
  * `[Mesh]_BaseColor.png`
  * `[Mesh]_Normal.png`
  * `[Mesh]_ORM.png` (Packed R: Ambient Occlusion, G: Roughness, B: Metallic)
* **Unity URP / Standard Preset**:
  * `[Mesh]_Albedo.png`
  * `[Mesh]_MetallicGloss.png` (R: Metallic, A: Smoothness)
  * `[Mesh]_Normal.png`
* **Blender / glTF Standard Preset**:
  * `[Mesh]_BaseColor.png`
  * `[Mesh]_MetallicRoughness.png` (G: Roughness, B: Metallic)
  * `[Mesh]_Normal.png`
  * `[Mesh]_AO.png`
* **Individual Maps**: Separate PNG exports for each channel.

### 🤖 Model Context Protocol (MCP) AI Agent Server
Following the ArtCraft standard, 3D Paintcraft includes a native **Model Context Protocol (MCP)** server over standard I/O (JSON-RPC 2.0). AI agents (Claude, Gemini, Antigravity, etc.) can inspect, paint, bake, and export assets automatically:
* `paintcraft_get_project_info`
* `paintcraft_list_smart_materials`
* `paintcraft_apply_smart_material`
* `paintcraft_add_layer`
* `paintcraft_paint_stroke`
* `paintcraft_bake_maps`
* `paintcraft_export_textures`
* `paintcraft_load_primitive`

---

## 🚀 Getting Started

### Prerequisites
* Linux (Bazzite / Fedora / Ubuntu / Arch / Debian, etc.)
* Rust toolchain (1.75+ or 1.99+) with Cargo
* Standard graphics libraries (OpenGL / Wayland / X11)

### Building
```bash
cd 3d-paintcraft
cargo build --release
```

The optimized binary will be produced at `target/release/paintcraft`.

---

## 🎮 Usage Modes

### 1. Interactive Desktop GUI (Substance 3D Painter Workflow)
Launch the graphical user interface:
```bash
./target/release/paintcraft
```

#### Controls & Shortcuts:
| Action | Shortcut / Mouse |
|---|---|
| **Paint on 3D Mesh** | Left Click & Drag |
| **Orbit Camera** | Right Click & Drag or `Alt` + Left Click & Drag |
| **Pan Camera** | Middle Click & Drag |
| **Zoom Viewport** | Mouse Scroll Wheel |
| **Reset / Frame Camera** | `F` |
| **Brush Tool** | `B` |
| **Eraser Tool** | `E` |
| **Eyedropper Tool** | `I` |

### 2. Headless CLI Batch Exporter
Bake and export textures without opening a GUI window (ideal for CI/CD and automation):
```bash
./target/release/paintcraft --export UnrealEngine --out ./output_textures --material worn_painted_steel
```

### 3. Model Context Protocol (MCP) Server for AI Assistants
Start the MCP server for AI agent control:
```bash
./target/release/paintcraft --mcp
```

Add to your MCP client config (e.g. `claude_desktop_config.json` or Antigravity configuration):
```json
{
  "mcpServers": {
    "paintcraft": {
      "command": "/path/to/3d-paintcraft/target/release/paintcraft",
      "args": ["--mcp"]
    }
  }
}
```

---

## 🏛️ Project Architecture
```
src/
├── main.rs                 # CLI entry point, GUI launcher & MCP dispatcher
├── app.rs                  # egui / eframe Substance Painter studio UI
├── project.rs              # Document & project model, layers, history
├── mesh.rs                 # 3D Mesh, Raycasting, OBJ parser, primitives
├── channel.rs              # PBR channel types, RgbaBuffer, ScalarBuffer
├── layer.rs                # Multi-channel Layer system, blend modes, compositor
├── brush.rs                # 3D projection & 2D brush engine, symmetry
├── renderer.rs             # Software PBR rasterizer, Cook-Torrance GGX, camera
├── baker.rs                # Curvature, Ambient Occlusion, Normal map bakers
├── smart_materials.rs      # Presets (Worn Metal, Rusted Iron, Brushed Gold, etc.)
├── export.rs               # Texture set exporter (Unreal Engine, Unity, Blender)
├── mcp.rs                  # Model Context Protocol (MCP) JSON-RPC server
└── utils.rs                # Procedural noise (fBm, Voronoi) & color helpers
```

---

## 📜 License
Dual-licensed under **MIT** or **Apache-2.0**, consistent with the ArtCraft open-source ecosystem.
