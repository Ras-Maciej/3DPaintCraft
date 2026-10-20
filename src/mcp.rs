// 3D Paintcraft - Model Context Protocol (MCP) Server & AI Agent Interface
// Native tool exposure for automated studio pipelines and agentic AI workflows

use crate::export::{ExportPreset, TextureExporter};
use crate::mesh::Mesh;
use crate::project::Project;
use crate::smart_materials::SmartMaterialLibrary;
use glam::{Vec2, Vec4};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

pub struct McpServer;

impl McpServer {
    /// Return the list of tools provided by 3D Paintcraft
    pub fn get_tools_manifest() -> Value {
        json!([
            {
                "name": "paintcraft_get_project_info",
                "description": "Get detailed project state including active 3D mesh, dimensions, layers, and baked maps",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "paintcraft_list_smart_materials",
                "description": "List all built-in procedural PBR smart materials",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "paintcraft_apply_smart_material",
                "description": "Apply a smart material preset to the active 3D mesh",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "preset_id": { "type": "string", "description": "e.g. worn_painted_steel, rusted_cast_iron, brushed_gold, matte_polymer, weathered_wood, scifi_hull, gloss_car_paint" }
                    },
                    "required": ["preset_id"]
                }
            },
            {
                "name": "paintcraft_add_layer",
                "description": "Add a new creative paint or fill layer to the layer stack",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_type": { "type": "string", "enum": ["paint", "fill"] },
                        "name": { "type": "string" },
                        "color": { "type": "array", "items": { "type": "number" }, "description": "[R, G, B, A] floats 0..1" },
                        "roughness": { "type": "number", "description": "0.0 to 1.0" },
                        "metallic": { "type": "number", "description": "0.0 to 1.0" }
                    },
                    "required": ["layer_type"]
                }
            },
            {
                "name": "paintcraft_paint_stroke",
                "description": "Stamp paint strokes directly onto UV coordinates of the active layer",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "uv_points": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "u": { "type": "number" },
                                    "v": { "type": "number" }
                                },
                                "required": ["u", "v"]
                            }
                        },
                        "radius": { "type": "number", "description": "Pixel radius" },
                        "color": { "type": "array", "items": { "type": "number" } },
                        "roughness": { "type": "number" },
                        "metallic": { "type": "number" }
                    },
                    "required": ["uv_points"]
                }
            },
            {
                "name": "paintcraft_bake_maps",
                "description": "Bake mesh surface maps (Curvature, Ambient Occlusion, World Space Normal)",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "paintcraft_export_textures",
                "description": "Export production PBR textures to disk for game engines or 3D DCC tools",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "preset": { "type": "string", "enum": ["UnrealEngine", "Unity", "BlenderGltf", "IndividualMaps"] },
                        "output_dir": { "type": "string", "description": "Absolute path to output folder" }
                    },
                    "required": ["preset", "output_dir"]
                }
            },
            {
                "name": "paintcraft_load_primitive",
                "description": "Load a built-in 3D model primitive (shader_ball, rounded_cube, scifi_crate, robot_head, torus, cylinder)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model": { "type": "string", "enum": ["shader_ball", "rounded_cube", "scifi_crate", "robot_head", "torus", "cylinder"] }
                    },
                    "required": ["model"]
                }
            },
            {
                "name": "paintcraft_save_project",
                "description": "Save current project to .paintcraft file",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute destination file path (.paintcraft)" }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "paintcraft_load_project",
                "description": "Load project from .paintcraft file",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute source file path (.paintcraft)" }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "paintcraft_add_layer_mask",
                "description": "Add a mask to the active layer (black, white, or curvature)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "mask_type": { "type": "string", "enum": ["black", "white", "curvature"] }
                    },
                    "required": ["mask_type"]
                }
            },
            {
                "name": "paintcraft_load_custom_obj",
                "description": "Load a custom Wavefront OBJ 3D model from disk",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to .obj file" }
                    },
                    "required": ["path"]
                }
            }
        ])
    }

    /// Dispatch and execute an MCP tool invocation against a Project
    pub fn execute_tool(project: &mut Project, tool_name: &str, arguments: &Value) -> Result<Value, String> {
        match tool_name {
            "paintcraft_get_project_info" => {
                let layers_info: Vec<Value> = project.layers.iter().enumerate().map(|(i, l)| {
                    json!({
                        "index": i,
                        "id": l.id,
                        "name": l.name,
                        "type": match l.layer_type {
                            crate::layer::LayerType::Paint => "paint",
                            crate::layer::LayerType::Fill => "fill",
                            crate::layer::LayerType::Folder => "folder",
                        },
                        "visible": l.visible,
                        "opacity": l.opacity,
                        "blend_mode": l.blend_mode.name(),
                    })
                }).collect();

                Ok(json!({
                    "name": project.name,
                    "mesh": {
                        "name": project.mesh.name,
                        "vertex_count": project.mesh.vertices.len(),
                        "triangle_count": project.mesh.indices.len() / 3,
                    },
                    "texture_resolution": format!("{}x{}", project.texture_width, project.texture_height),
                    "layer_count": project.layers.len(),
                    "active_layer_index": project.active_layer_idx,
                    "layers": layers_info,
                    "has_curvature_bake": project.curvature_map.is_some(),
                    "has_ao_bake": project.ao_map.is_some(),
                }))
            }

            "paintcraft_list_smart_materials" => {
                let list: Vec<Value> = SmartMaterialLibrary::presets().iter().map(|p| {
                    json!({
                        "id": p.id,
                        "name": p.name,
                        "category": p.category,
                        "description": p.description,
                    })
                }).collect();
                Ok(json!({ "smart_materials": list }))
            }

            "paintcraft_apply_smart_material" => {
                let preset_id = arguments.get("preset_id")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required 'preset_id'")?;

                project.apply_smart_material(preset_id);
                Ok(json!({
                    "success": true,
                    "preset_applied": preset_id,
                    "layer_count": project.layers.len(),
                }))
            }

            "paintcraft_add_layer" => {
                let layer_type = arguments.get("layer_type").and_then(|v| v.as_str()).unwrap_or("paint");
                let name = arguments.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());

                let idx = if layer_type == "fill" {
                    let col_arr = arguments.get("color").and_then(|v| v.as_array());
                    let color = if let Some(arr) = col_arr {
                        Vec4::new(
                            arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.8) as f32,
                            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.8) as f32,
                            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.8) as f32,
                            arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        )
                    } else {
                        Vec4::new(0.6, 0.6, 0.6, 1.0)
                    };
                    let rough = arguments.get("roughness").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
                    let metal = arguments.get("metallic").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    project.add_fill_layer(name, color, rough, metal)
                } else {
                    project.add_paint_layer(name)
                };

                Ok(json!({ "success": true, "layer_index": idx }))
            }

            "paintcraft_paint_stroke" => {
                let points = arguments.get("uv_points").and_then(|v| v.as_array()).ok_or("Missing 'uv_points'")?;
                let radius = arguments.get("radius").and_then(|v| v.as_f64()).unwrap_or(20.0) as f32;

                let col_arr = arguments.get("color").and_then(|v| v.as_array());
                let color = if let Some(arr) = col_arr {
                    Vec4::new(
                        arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.9) as f32,
                        arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.2) as f32,
                        arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.2) as f32,
                        arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    )
                } else {
                    Vec4::new(0.9, 0.2, 0.2, 1.0)
                };

                let rough = arguments.get("roughness").and_then(|v| v.as_f64()).unwrap_or(0.3) as f32;
                let metal = arguments.get("metallic").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

                let mut engine = crate::brush::BrushEngine::new();
                engine.settings.radius = radius;
                engine.settings.paint_color = color;
                engine.settings.paint_roughness = rough;
                engine.settings.paint_metallic = metal;

                if let Some(layer) = project.active_layer_mut() {
                    for pt in points {
                        let u = pt.get("u").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
                        let v = pt.get("v").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
                        engine.paint_stroke(layer, Vec2::new(u, v));
                    }
                }
                project.rebuild_composite();

                Ok(json!({ "success": true, "stamps_applied": points.len() }))
            }

            "paintcraft_bake_maps" => {
                project.bake_mesh_maps(2048, 0.1, 0.1);
                project.rebuild_composite();
                Ok(json!({ "success": true, "curvature_baked": true, "ao_baked": true }))
            }

            "paintcraft_export_textures" => {
                let preset_str = arguments.get("preset").and_then(|v| v.as_str()).unwrap_or("UnrealEngine");
                let out_dir_str = arguments.get("output_dir").and_then(|v| v.as_str()).ok_or("Missing output_dir")?;
                let out_dir = PathBuf::from(out_dir_str);

                let preset = match preset_str {
                    "Unity" => ExportPreset::Unity,
                    "BlenderGltf" => ExportPreset::BlenderGltf,
                    "IndividualMaps" => ExportPreset::IndividualMaps,
                    _ => ExportPreset::UnrealEngine,
                };

                let flip_v = arguments.get("flip_v").and_then(|v| v.as_bool()).unwrap_or(true);
                let export_mesh = arguments.get("export_mesh").and_then(|v| v.as_bool()).unwrap_or(true);
                let files = TextureExporter::export_preset(
                    &project.composite_textures,
                    &project.mesh.name,
                    preset,
                    &out_dir,
                    flip_v,
                    if export_mesh { Some(&project.mesh) } else { None },
                )?;
                let file_strings: Vec<String> = files.into_iter().map(|p| p.to_string_lossy().to_string()).collect();

                Ok(json!({ "success": true, "exported_files": file_strings }))
            }

            "paintcraft_load_primitive" => {
                let model = arguments.get("model").and_then(|v| v.as_str()).unwrap_or("shader_ball");
                let new_mesh = match model {
                    "rounded_cube" => Mesh::create_rounded_cube(),
                    "scifi_crate" => Mesh::create_scifi_crate(),
                    "robot_head" => Mesh::create_robot_head(),
                    "torus" => Mesh::create_torus(),
                    "cylinder" => Mesh::create_cylinder(),
                    _ => Mesh::create_shader_ball(),
                };
                project.set_mesh(new_mesh);
                Ok(json!({ "success": true, "model_loaded": model }))
            }

            "paintcraft_save_project" => {
                let path_str = arguments.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?;
                let path = std::path::Path::new(path_str);
                project.save_to_file(path)?;
                Ok(json!({ "success": true, "saved_to": path_str }))
            }

            "paintcraft_load_project" => {
                let path_str = arguments.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?;
                let path = std::path::Path::new(path_str);
                let loaded = Project::load_from_file(path)?;
                *project = loaded;
                Ok(json!({ "success": true, "loaded_from": path_str }))
            }

            "paintcraft_add_layer_mask" => {
                let mask_type = arguments.get("mask_type").and_then(|v| v.as_str()).unwrap_or("black");
                let curv = project.curvature_map.clone();
                if let Some(layer) = project.active_layer_mut() {
                    match mask_type {
                        "white" => layer.add_white_mask(),
                        "curvature" => layer.add_curvature_mask(curv.as_ref()),
                        _ => layer.add_black_mask(),
                    }
                    project.rebuild_composite();
                    Ok(json!({ "success": true, "mask_type": mask_type }))
                } else {
                    Err("No active layer".to_string())
                }
            }

            "paintcraft_load_custom_obj" => {
                let path_str = arguments.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?;
                let path = std::path::Path::new(path_str);
                let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Model");
                let mesh = Mesh::from_obj_str(&content, name)?;
                project.set_mesh(mesh);
                Ok(json!({ "success": true, "model_loaded": name, "triangle_count": project.mesh.indices.len() / 3 }))
            }

            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    /// Run standard stdio MCP protocol loop
    pub fn run_stdio_server(mut project: Project) {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            if line.trim().is_empty() {
                continue;
            }

            let request: McpRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let err_resp = json!({
                        "jsonrpc": "2.0",
                        "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                    });
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&err_resp).unwrap());
                    let _ = stdout.flush();
                    continue;
                }
            };

            let response = match request.method.as_str() {
                "tools/list" => McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({ "tools": Self::get_tools_manifest() })),
                    error: None,
                },
                "tools/call" => {
                    let params = request.params.unwrap_or(Value::Null);
                    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let args = params.get("arguments").unwrap_or(&Value::Null);

                    match Self::execute_tool(&mut project, name, args) {
                        Ok(res) => McpResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: Some(json!({ "content": [{ "type": "text", "text": res.to_string() }] })),
                            error: None,
                        },
                        Err(err) => McpResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(json!({ "code": -32603, "message": err })),
                        },
                    }
                }
                "ping" => McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({ "status": "pong" })),
                    error: None,
                },
                _ => McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(json!({ "code": -32601, "message": "Method not found" })),
                },
            };

            let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
            let _ = stdout.flush();
        }
    }
}
