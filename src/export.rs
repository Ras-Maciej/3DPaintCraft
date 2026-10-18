// 3D Paintcraft - Texture Exporter (Unreal Engine, Unity, Blender, glTF, Custom)
// Pure safe Rust

use crate::layer::CompositeTextureSet;
use image::{ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportPreset {
    Godot4,           // Godot 4 StandardMaterial3D (Albedo, Normal, ORM, Height, AO)
    UnrealEngine,     // BaseColor, Normal, Packed ORM (R:AO, G:Rough, B:Metal)
    Unity,            // Albedo, MetallicGloss (R:Metal, A:Smoothness), Normal
    BlenderGltf,      // BaseColor, MetallicRoughness (B:Metal, G:Rough), Normal, AO
    IndividualMaps,   // Separate individual textures
}

impl ExportPreset {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Godot4 => "Godot 4 Engine (StandardMaterial3D / ORM)",
            Self::UnrealEngine => "Unreal Engine (ORM Packed)",
            Self::Unity => "Unity URP / Standard",
            Self::BlenderGltf => "Blender / glTF Standard",
            Self::IndividualMaps => "Individual Channel Maps",
        }
    }
}

pub struct TextureExporter;

impl TextureExporter {
    pub fn export_preset(
        textures: &CompositeTextureSet,
        mesh_name: &str,
        preset: ExportPreset,
        output_dir: &Path,
        flip_v: bool,
        mesh: Option<&crate::mesh::Mesh>,
    ) -> Result<Vec<PathBuf>, String> {
        std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
        let mut generated_files = Vec::new();

        let clean_name = mesh_name.replace(' ', "_").to_lowercase();

        // Convert composite height map to tangent normal bump and blend with layer normals (Adobe 3D Painter standard)
        let combined_normal = textures.generate_combined_normal(4.0);

        match preset {
            ExportPreset::Godot4 => {
                // 1. Albedo
                let albedo_path = output_dir.join(format!("{}_Albedo.png", clean_name));
                Self::save_rgba(&textures.base_color, &albedo_path, flip_v)?;
                generated_files.push(albedo_path);

                // 2. Normal (OpenGL +Y up with blended height bump)
                let n_path = output_dir.join(format!("{}_Normal.png", clean_name));
                Self::save_rgba(&combined_normal, &n_path, flip_v)?;
                generated_files.push(n_path);

                // 3. Packed ORM: R=AO, G=Roughness, B=Metallic
                let orm_path = output_dir.join(format!("{}_ORM.png", clean_name));
                Self::save_packed_orm(&textures.ao, &textures.roughness, &textures.metallic, &orm_path, flip_v)?;
                generated_files.push(orm_path);

                // 4. Height / Displacement
                let height_path = output_dir.join(format!("{}_Height.png", clean_name));
                Self::save_scalar(&textures.height_map, &height_path, flip_v)?;
                generated_files.push(height_path);

                // 5. Individual Roughness
                let rough_path = output_dir.join(format!("{}_Roughness.png", clean_name));
                Self::save_scalar(&textures.roughness, &rough_path, flip_v)?;
                generated_files.push(rough_path);

                // 6. Individual Metallic
                let metal_path = output_dir.join(format!("{}_Metallic.png", clean_name));
                Self::save_scalar(&textures.metallic, &metal_path, flip_v)?;
                generated_files.push(metal_path);

                // 7. Individual AO
                let ao_path = output_dir.join(format!("{}_AO.png", clean_name));
                Self::save_scalar(&textures.ao, &ao_path, flip_v)?;
                generated_files.push(ao_path);

                // 8. Emissive
                let emis_path = output_dir.join(format!("{}_Emissive.png", clean_name));
                Self::save_rgba(&textures.emissive, &emis_path, flip_v)?;
                generated_files.push(emis_path);

                // 9. Opacity
                let opac_path = output_dir.join(format!("{}_Opacity.png", clean_name));
                Self::save_scalar(&textures.opacity, &opac_path, flip_v)?;
                generated_files.push(opac_path);
            }

            ExportPreset::UnrealEngine => {
                // 1. BaseColor
                let bc_path = output_dir.join(format!("{}_BaseColor.png", clean_name));
                Self::save_rgba(&textures.base_color, &bc_path, flip_v)?;
                generated_files.push(bc_path);

                // 2. Normal (DirectX -Y inverted with blended height bump)
                let n_path = output_dir.join(format!("{}_Normal.png", clean_name));
                Self::save_normal_directx(&combined_normal, &n_path, flip_v)?;
                generated_files.push(n_path);

                // 3. Packed ORM: R=AO, G=Roughness, B=Metallic
                let orm_path = output_dir.join(format!("{}_ORM.png", clean_name));
                Self::save_packed_orm(&textures.ao, &textures.roughness, &textures.metallic, &orm_path, flip_v)?;
                generated_files.push(orm_path);

                // 4. Emissive
                let emis_path = output_dir.join(format!("{}_Emissive.png", clean_name));
                Self::save_rgba(&textures.emissive, &emis_path, flip_v)?;
                generated_files.push(emis_path);

                // 5. Opacity
                let opac_path = output_dir.join(format!("{}_Opacity.png", clean_name));
                Self::save_scalar(&textures.opacity, &opac_path, flip_v)?;
                generated_files.push(opac_path);
            }

            ExportPreset::BlenderGltf => {
                // 1. BaseColor
                let bc_path = output_dir.join(format!("{}_BaseColor.png", clean_name));
                Self::save_rgba(&textures.base_color, &bc_path, flip_v)?;
                generated_files.push(bc_path);

                // 2. Normal (OpenGL +Y up with blended height bump)
                let n_path = output_dir.join(format!("{}_Normal.png", clean_name));
                Self::save_rgba(&combined_normal, &n_path, flip_v)?;
                generated_files.push(n_path);

                // 3. MetallicRoughness (glTF standard: G = Roughness, B = Metallic)
                let mr_path = output_dir.join(format!("{}_MetallicRoughness.png", clean_name));
                Self::save_packed_mr(&textures.roughness, &textures.metallic, &mr_path, flip_v)?;
                generated_files.push(mr_path);

                // 4. AO
                let ao_path = output_dir.join(format!("{}_AO.png", clean_name));
                Self::save_scalar(&textures.ao, &ao_path, flip_v)?;
                generated_files.push(ao_path);

                // 5. Emissive
                let emis_path = output_dir.join(format!("{}_Emissive.png", clean_name));
                Self::save_rgba(&textures.emissive, &emis_path, flip_v)?;
                generated_files.push(emis_path);

                // 6. Opacity
                let opac_path = output_dir.join(format!("{}_Opacity.png", clean_name));
                Self::save_scalar(&textures.opacity, &opac_path, flip_v)?;
                generated_files.push(opac_path);
            }

            ExportPreset::Unity => {
                // 1. Albedo
                let albedo_path = output_dir.join(format!("{}_Albedo.png", clean_name));
                Self::save_rgba(&textures.base_color, &albedo_path, flip_v)?;
                generated_files.push(albedo_path);

                // 2. MetallicGloss: R = Metallic, A = Smoothness (1 - roughness)
                let mg_path = output_dir.join(format!("{}_MetallicGloss.png", clean_name));
                Self::save_packed_unity(&textures.metallic, &textures.roughness, &mg_path, flip_v)?;
                generated_files.push(mg_path);

                // 3. Normal (OpenGL +Y up with blended height bump)
                let n_path = output_dir.join(format!("{}_Normal.png", clean_name));
                Self::save_rgba(&combined_normal, &n_path, flip_v)?;
                generated_files.push(n_path);

                // 4. Emissive
                let emis_path = output_dir.join(format!("{}_Emission.png", clean_name));
                Self::save_rgba(&textures.emissive, &emis_path, flip_v)?;
                generated_files.push(emis_path);

                // 5. Opacity / Cutout
                let opac_path = output_dir.join(format!("{}_Opacity.png", clean_name));
                Self::save_scalar(&textures.opacity, &opac_path, flip_v)?;
                generated_files.push(opac_path);
            }

            ExportPreset::IndividualMaps => {
                let maps = [
                    ("BaseColor", &textures.base_color),
                    ("Normal", &combined_normal),
                    ("Emissive", &textures.emissive),
                ];
                for (suffix, buf) in maps {
                    let path = output_dir.join(format!("{}_{}.png", clean_name, suffix));
                    Self::save_rgba(buf, &path, flip_v)?;
                    generated_files.push(path);
                }

                let scalar_maps = [
                    ("Roughness", &textures.roughness),
                    ("Metallic", &textures.metallic),
                    ("Height", &textures.height_map),
                    ("AO", &textures.ao),
                    ("Opacity", &textures.opacity),
                    ("Clearcoat", &textures.clearcoat),
                    ("Anisotropy", &textures.anisotropy),
                    ("AnisotropyAngle", &textures.anisotropy_angle),
                ];
                for (suffix, buf) in scalar_maps {
                    let path = output_dir.join(format!("{}_{}.png", clean_name, suffix));
                    Self::save_scalar(buf, &path, flip_v)?;
                    generated_files.push(path);
                }
            }
        }

        // Export 3D Mesh (.obj + .mtl) alongside textures if requested
        if let Some(mesh) = mesh {
            let obj_path = output_dir.join(format!("{}.obj", clean_name));
            mesh.save_to_obj_file(&obj_path)?;
            generated_files.push(obj_path);

            let mtl_path = output_dir.join(format!("{}.mtl", clean_name));
            let mut mtl_content = format!(
                "# 3D Paintcraft Wavefront Material Definition\nnewmtl Material\nillum 2\nKa 1.0 1.0 1.0\nKd 1.0 1.0 1.0\nKs 0.5 0.5 0.5\n"
            );
            for file in &generated_files {
                if let Some(fname) = file.file_name().and_then(|f| f.to_str()) {
                    if fname.ends_with("_Albedo.png") || fname.ends_with("_BaseColor.png") {
                        mtl_content.push_str(&format!("map_Kd {}\n", fname));
                    } else if fname.ends_with("_Normal.png") {
                        mtl_content.push_str(&format!("map_Bump {}\nbump {}\n", fname, fname));
                    } else if fname.ends_with("_Roughness.png") {
                        mtl_content.push_str(&format!("map_Ns {}\n", fname));
                    } else if fname.ends_with("_Metallic.png") {
                        mtl_content.push_str(&format!("map_Pm {}\n", fname));
                    } else if fname.ends_with("_AO.png") {
                        mtl_content.push_str(&format!("map_Ka {}\n", fname));
                    } else if fname.ends_with("_Emissive.png") || fname.ends_with("_Emission.png") {
                        mtl_content.push_str(&format!("map_Ke {}\n", fname));
                    } else if fname.ends_with("_Opacity.png") {
                        mtl_content.push_str(&format!("map_d {}\n", fname));
                    }
                }
            }
            if std::fs::write(&mtl_path, mtl_content).is_ok() {
                generated_files.push(mtl_path);
            }
        }

        Ok(generated_files)
    }

    pub fn dilate_image_rgba(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, mask: &[bool], iterations: u32) {
        let (w, h) = img.dimensions();
        if mask.len() != (w * h) as usize {
            return;
        }

        // If all pixels are already filled, no dilation needed
        if mask.iter().all(|&v| v) {
            return;
        }

        let mut current_mask = mask.to_vec();
        let mut next_mask = current_mask.clone();

        for _ in 0..iterations {
            let mut changes = 0;
            for y in 0..h {
                for x in 0..w {
                    let idx = (y * w + x) as usize;
                    if !current_mask[idx] {
                        let mut sum_r = 0u32;
                        let mut sum_g = 0u32;
                        let mut sum_b = 0u32;
                        let mut count = 0u32;

                        let neighbors = [
                            (x.wrapping_sub(1), y),
                            (x + 1, y),
                            (x, y.wrapping_sub(1)),
                            (x, y + 1),
                        ];

                        for (nx, ny) in neighbors {
                            if nx < w && ny < h {
                                let n_idx = (ny * w + nx) as usize;
                                if current_mask[n_idx] {
                                    let p = img.get_pixel(nx, ny);
                                    sum_r += p[0] as u32;
                                    sum_g += p[1] as u32;
                                    sum_b += p[2] as u32;
                                    count += 1;
                                }
                            }
                        }

                        if count > 0 {
                            let r = (sum_r / count) as u8;
                            let g = (sum_g / count) as u8;
                            let b = (sum_b / count) as u8;
                            img.put_pixel(x, y, Rgba([r, g, b, 255]));
                            next_mask[idx] = true;
                            changes += 1;
                        }
                    }
                }
            }
            if changes == 0 {
                break;
            }
            current_mask.copy_from_slice(&next_mask);
        }
    }

    fn save_normal_directx(buf: &crate::channel::RgbaBuffer, path: &Path, flip_v: bool) -> Result<(), String> {
        let w = buf.width;
        let h = buf.height;
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);
        let mut mask = vec![false; (w * h) as usize];
        for y in 0..h {
            let dst_y = if flip_v { h - 1 - y } else { y };
            for x in 0..w {
                let c = buf.get_pixel(x, y);
                let r = (c.x.clamp(0.0, 1.0) * 255.0) as u8;
                let g = ((1.0 - c.y).clamp(0.0, 1.0) * 255.0) as u8; // DirectX -Y inverted
                let b = (c.z.clamp(0.0, 1.0) * 255.0) as u8;
                let a = 255u8;
                img.put_pixel(x, dst_y, Rgba([r, g, b, a]));
                mask[(dst_y * w + x) as usize] = c.w > 0.001;
            }
        }
        Self::dilate_image_rgba(&mut img, &mask, 16);
        img.save(path).map_err(|e| e.to_string())
    }

    fn save_rgba(buf: &crate::channel::RgbaBuffer, path: &Path, flip_v: bool) -> Result<(), String> {
        let w = buf.width;
        let h = buf.height;
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);
        let mut mask = vec![false; (w * h) as usize];
        for y in 0..h {
            let dst_y = if flip_v { h - 1 - y } else { y };
            for x in 0..w {
                let c = buf.get_pixel(x, y);
                let r = (c.x.clamp(0.0, 1.0) * 255.0) as u8;
                let g = (c.y.clamp(0.0, 1.0) * 255.0) as u8;
                let b = (c.z.clamp(0.0, 1.0) * 255.0) as u8;
                let a = (c.w.clamp(0.0, 1.0) * 255.0) as u8;
                img.put_pixel(x, dst_y, Rgba([r, g, b, a]));
                mask[(dst_y * w + x) as usize] = c.w > 0.001;
            }
        }
        Self::dilate_image_rgba(&mut img, &mask, 16);
        img.save(path).map_err(|e| e.to_string())
    }

    fn save_scalar(buf: &crate::channel::ScalarBuffer, path: &Path, flip_v: bool) -> Result<(), String> {
        let w = buf.width;
        let h = buf.height;
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);
        for y in 0..h {
            let dst_y = if flip_v { h - 1 - y } else { y };
            for x in 0..w {
                let val = (buf.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                img.put_pixel(x, dst_y, Rgba([val, val, val, 255]));
            }
        }
        img.save(path).map_err(|e| e.to_string())
    }

    fn save_packed_orm(
        ao: &crate::channel::ScalarBuffer,
        roughness: &crate::channel::ScalarBuffer,
        metallic: &crate::channel::ScalarBuffer,
        path: &Path,
        flip_v: bool,
    ) -> Result<(), String> {
        let w = ao.width;
        let h = ao.height;
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);
        for y in 0..h {
            let dst_y = if flip_v { h - 1 - y } else { y };
            for x in 0..w {
                let a = (ao.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                let r = (roughness.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                let m = (metallic.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                img.put_pixel(x, dst_y, Rgba([a, r, m, 255]));
            }
        }
        img.save(path).map_err(|e| e.to_string())
    }

    fn save_packed_mr(
        roughness: &crate::channel::ScalarBuffer,
        metallic: &crate::channel::ScalarBuffer,
        path: &Path,
        flip_v: bool,
    ) -> Result<(), String> {
        let w = roughness.width;
        let h = roughness.height;
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);
        for y in 0..h {
            let dst_y = if flip_v { h - 1 - y } else { y };
            for x in 0..w {
                let r = (roughness.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                let m = (metallic.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                img.put_pixel(x, dst_y, Rgba([0, r, m, 255]));
            }
        }
        img.save(path).map_err(|e| e.to_string())
    }

    fn save_packed_unity(
        metallic: &crate::channel::ScalarBuffer,
        roughness: &crate::channel::ScalarBuffer,
        path: &Path,
        flip_v: bool,
    ) -> Result<(), String> {
        let w = metallic.width;
        let h = metallic.height;
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);
        for y in 0..h {
            let dst_y = if flip_v { h - 1 - y } else { y };
            for x in 0..w {
                let m = (metallic.get_pixel(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                let smoothness = ((1.0 - roughness.get_pixel(x, y)).clamp(0.0, 1.0) * 255.0) as u8;
                img.put_pixel(x, dst_y, Rgba([m, 0, 0, smoothness]));
            }
        }
        img.save(path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_preset_godot4() {
        let textures = CompositeTextureSet::new(16, 16);
        let temp_dir = std::env::temp_dir().join("test_paintcraft_godot4_export");
        let exported = TextureExporter::export_preset(&textures, "ShaderBall", ExportPreset::Godot4, &temp_dir, false, None).unwrap();

        assert_eq!(exported.len(), 9, "Godot 4 preset should generate 9 textures (including Emissive and Opacity)");
        for path in &exported {
            assert!(path.exists(), "Exported file {:?} must exist on disk", path);
        }
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_height_to_normal_export_conversion() {
        let mut textures = CompositeTextureSet::new(16, 16);
        // Fill left half with 0.0 and right half with 1.0 to create a step gradient
        for y in 0..16 {
            for x in 0..8 {
                textures.height_map.set_pixel(x, y, 0.0);
            }
            for x in 8..16 {
                textures.height_map.set_pixel(x, y, 1.0);
            }
        }
        let temp_dir = std::env::temp_dir().join("test_paintcraft_height_normal_export");
        let _exported = TextureExporter::export_preset(&textures, "HeightTest", ExportPreset::Godot4, &temp_dir, false, None).unwrap();

        let normal_path = temp_dir.join("heighttest_Normal.png");
        assert!(normal_path.exists());
        let img = image::open(&normal_path).unwrap().into_rgba8();
        // At step boundary (x=7, y=8), normal X should deviate significantly from 128 (0.5 flat)
        let pixel = img.get_pixel(7, 8);
        assert_ne!(pixel[0], 128, "Normal X should be perturbed by height gradient");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_uv_seam_dilation() {
        let mut img = image::RgbaImage::new(8, 8);
        img.put_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
        let mut mask = vec![false; 64];
        mask[4 * 8 + 4] = true;

        TextureExporter::dilate_image_rgba(&mut img, &mask, 2);
        let neighbor = img.get_pixel(4, 3);
        assert_eq!(neighbor[0], 255, "Neighbor red channel should be dilated");

        let neighbor2 = img.get_pixel(4, 2);
        assert_eq!(neighbor2[0], 255, "Neighbor 2 steps away should be dilated");
    }

    #[test]
    fn test_export_flip_v() {
        let mut textures = CompositeTextureSet::new(8, 8);
        // Set top row (y = 0) to bright red
        for x in 0..8 {
            textures.base_color.set_pixel(x, 0, glam::Vec4::new(1.0, 0.0, 0.0, 1.0));
        }
        let temp_dir_flipped = std::env::temp_dir().join("test_paintcraft_flip_v_true");
        let _ = TextureExporter::export_preset(&textures, "FlipTest", ExportPreset::Godot4, &temp_dir_flipped, true, None).unwrap();
        let flipped_albedo = image::open(temp_dir_flipped.join("fliptest_Albedo.png")).unwrap().into_rgba8();
        // When flip_v is true, row 0 in buffer should end up at row h - 1 (y = 7) in the saved PNG
        assert_eq!(flipped_albedo.get_pixel(4, 7)[0], 255, "Flipped texture should have painted row at the bottom (y=7)");
        let _ = std::fs::remove_dir_all(&temp_dir_flipped);

        let temp_dir_unflipped = std::env::temp_dir().join("test_paintcraft_flip_v_false");
        let _ = TextureExporter::export_preset(&textures, "FlipTest", ExportPreset::Godot4, &temp_dir_unflipped, false, None).unwrap();
        let unflipped_albedo = image::open(temp_dir_unflipped.join("fliptest_Albedo.png")).unwrap().into_rgba8();
        // When flip_v is false, row 0 in buffer stays at row 0 in the saved PNG
        assert_eq!(unflipped_albedo.get_pixel(4, 0)[0], 255, "Unflipped texture should have painted row at the top (y=0)");
        let _ = std::fs::remove_dir_all(&temp_dir_unflipped);
    }

    #[test]
    fn test_export_mesh_obj_and_mtl() {
        let textures = CompositeTextureSet::new(8, 8);
        let mesh = crate::mesh::Mesh::create_cube_mesh("ExportCube", 1.0);
        let temp_dir = std::env::temp_dir().join("test_paintcraft_mesh_export");
        let exported = TextureExporter::export_preset(&textures, "ExportCube", ExportPreset::UnrealEngine, &temp_dir, true, Some(&mesh)).unwrap();

        let obj_path = temp_dir.join("exportcube.obj");
        let mtl_path = temp_dir.join("exportcube.mtl");
        assert!(obj_path.exists(), "Exported .obj file must exist");
        assert!(mtl_path.exists(), "Exported .mtl file must exist");
        assert!(exported.contains(&obj_path));
        assert!(exported.contains(&mtl_path));

        let obj_content = std::fs::read_to_string(&obj_path).unwrap();
        assert!(obj_content.contains("mtllib exportcube.mtl"));
        assert!(obj_content.contains("usemtl Material"));
        assert!(obj_content.contains("v "));
        assert!(obj_content.contains("vt "));
        assert!(obj_content.contains("vn "));
        assert!(obj_content.contains("f "));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_export_emissive_and_opacity_presets() {
        let mut textures = CompositeTextureSet::new(16, 16);
        textures.emissive.set_pixel(8, 8, glam::Vec4::new(1.0, 0.5, 0.2, 1.0));
        textures.opacity.set_pixel(8, 8, 0.5);

        let temp_dir = std::env::temp_dir().join("test_paintcraft_emissive_opacity_export");

        // Test Unreal
        let ue_dir = temp_dir.join("ue");
        let ue_files = TextureExporter::export_preset(&textures, "TestMesh", ExportPreset::UnrealEngine, &ue_dir, false, None).unwrap();
        assert!(ue_files.contains(&ue_dir.join("testmesh_Emissive.png")), "Unreal must export Emissive");
        assert!(ue_files.contains(&ue_dir.join("testmesh_Opacity.png")), "Unreal must export Opacity");

        // Test Blender / glTF
        let blender_dir = temp_dir.join("blender");
        let blender_files = TextureExporter::export_preset(&textures, "TestMesh", ExportPreset::BlenderGltf, &blender_dir, false, None).unwrap();
        assert!(blender_files.contains(&blender_dir.join("testmesh_Emissive.png")), "Blender must export Emissive");
        assert!(blender_files.contains(&blender_dir.join("testmesh_Opacity.png")), "Blender must export Opacity");

        // Test Unity
        let unity_dir = temp_dir.join("unity");
        let unity_files = TextureExporter::export_preset(&textures, "TestMesh", ExportPreset::Unity, &unity_dir, false, None).unwrap();
        assert!(unity_files.contains(&unity_dir.join("testmesh_Emission.png")), "Unity must export Emission");
        assert!(unity_files.contains(&unity_dir.join("testmesh_Opacity.png")), "Unity must export Opacity");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
