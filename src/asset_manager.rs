// 3D Paintcraft - Substance 3D Painter Style Asset Paths & HDRI Manager
// Pure safe Rust for Linux studio creative workflows

use glam::Vec3;
use image::codecs::hdr::HdrEncoder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HdriAssetEntry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub file_name: String,
    pub is_builtin: bool,
    pub file_size_bytes: u64,
}

impl HdriAssetEntry {
    pub fn size_formatted(&self) -> String {
        if self.file_size_bytes >= 1024 * 1024 {
            format!("{:.1} MB", self.file_size_bytes as f64 / (1024.0 * 1024.0))
        } else if self.file_size_bytes >= 1024 {
            format!("{:.0} KB", self.file_size_bytes as f64 / 1024.0)
        } else {
            format!("{} B", self.file_size_bytes)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetPaths {
    pub root_dir: PathBuf,
    pub hdri_dir: PathBuf,
    pub alphas_dir: PathBuf,
    pub materials_dir: PathBuf,
    pub smart_materials_dir: PathBuf,
    pub custom_dirs: Vec<PathBuf>,
}

impl Default for AssetPaths {
    fn default() -> Self {
        let base = if Path::new("assets").exists() {
            PathBuf::from("assets")
        } else if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            PathBuf::from(manifest_dir).join("assets")
        } else if let Ok(current) = std::env::current_dir() {
            current.join("assets")
        } else {
            PathBuf::from("./assets")
        };

        Self {
            hdri_dir: base.join("hdris"),
            alphas_dir: base.join("alphas"),
            materials_dir: base.join("materials"),
            smart_materials_dir: base.join("smart_materials"),
            root_dir: base,
            custom_dirs: Vec::new(),
        }
    }
}

pub struct AssetManager {
    pub paths: AssetPaths,
    pub hdris: Vec<HdriAssetEntry>,
    pub active_hdri_index: usize,
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetManager {
    pub fn new() -> Self {
        let paths = AssetPaths::default();
        let mut manager = Self {
            paths,
            hdris: Vec::new(),
            active_hdri_index: 0,
        };
        manager.init();
        manager
    }

    /// Initialize directories, synthesize built-in .hdr files on disk if missing, and scan
    pub fn init(&mut self) {
        self.ensure_directories();
        self.ensure_builtin_hdris();
        self.refresh_hdris();
    }

    /// Ensure all asset folders exist on disk
    pub fn ensure_directories(&self) {
        let _ = fs::create_dir_all(&self.paths.root_dir);
        let _ = fs::create_dir_all(&self.paths.hdri_dir);
        let _ = fs::create_dir_all(&self.paths.alphas_dir);
        let _ = fs::create_dir_all(&self.paths.materials_dir);
        let _ = fs::create_dir_all(&self.paths.smart_materials_dir);
        for custom in &self.paths.custom_dirs {
            let _ = fs::create_dir_all(custom);
        }
    }

    /// Synthesize and write compliant Radiance .hdr files for stock environments if not present
    pub fn ensure_builtin_hdris(&self) {
        let builtins = [
            ("Studio_Softboxes.hdr", crate::hdri::HdriPreset::StudioNeutral),
            ("Tomoko_Studio.hdr", crate::hdri::HdriPreset::TomokoStudio),
            ("Courtyard_Sunny.hdr", crate::hdri::HdriPreset::CourtyardSunny),
            ("Sunset_Desert.hdr", crate::hdri::HdriPreset::SunsetDesert),
            ("Dark_Industrial.hdr", crate::hdri::HdriPreset::DarkIndustrial),
            ("Cyberpunk_Neon.hdr", crate::hdri::HdriPreset::CyberpunkAlley),
        ];

        for (filename, preset) in builtins {
            let target_path = self.paths.hdri_dir.join(filename);
            if !target_path.exists() {
                let _ = Self::write_procedural_hdr(&target_path, &preset, 1024, 512);
            }
        }
    }

    /// Write an equirectangular environment panorama as a standard Radiance .hdr file
    pub fn write_procedural_hdr(
        out_path: &Path,
        preset: &crate::hdri::HdriPreset,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let env = crate::hdri::HdriEnvironment::new(preset.clone());
        let total_pixels = (width * height) as usize;
        let mut raw_pixels = vec![image::Rgb([0.0f32, 0.0f32, 0.0f32]); total_pixels];

        raw_pixels.par_chunks_mut(width as usize).enumerate().for_each(|(y, row)| {
            let v = (y as f32 + 0.5) / height as f32;
            let phi = std::f32::consts::PI * 0.5 - v * std::f32::consts::PI;
            for (x, pixel) in row.iter_mut().enumerate() {
                let u = (x as f32 + 0.5) / width as f32;
                let theta = u * 2.0 * std::f32::consts::PI - std::f32::consts::PI;
                let dir = Vec3::new(
                    phi.cos() * theta.cos(),
                    phi.sin(),
                    phi.cos() * theta.sin(),
                );
                let col = env.evaluate_specular(dir, 0.08);
                *pixel = image::Rgb([col.x, col.y, col.z]);
            }
        });

        let file = fs::File::create(out_path).map_err(|e| format!("Failed to create HDR file {:?}: {}", out_path, e))?;
        let writer = std::io::BufWriter::new(file);
        let encoder = HdrEncoder::new(writer);
        encoder.encode(&raw_pixels, width as usize, height as usize).map_err(|e| format!("Failed to encode HDR: {}", e))?;

        Ok(())
    }

    /// Rescan the HDRI asset folder for all .hdr, .png, .jpg, and .exr files
    pub fn refresh_hdris(&mut self) {
        let mut found = Vec::new();
        let mut scan_dirs = vec![self.paths.hdri_dir.clone()];
        scan_dirs.extend(self.paths.custom_dirs.clone());

        for dir in scan_dirs {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                            let ext_lower = ext.to_lowercase();
                            if ext_lower == "hdr" || ext_lower == "png" || ext_lower == "jpg" || ext_lower == "jpeg" || ext_lower == "exr" {
                                let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                                let clean_name = Self::format_display_name(&stem);
                                let is_builtin = file_name.starts_with("Studio_")
                                    || file_name.starts_with("Tomoko_")
                                    || file_name.starts_with("Courtyard_")
                                    || file_name.starts_with("Sunset_")
                                    || file_name.starts_with("Dark_")
                                    || file_name.starts_with("Cyberpunk_");
                                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                                found.push(HdriAssetEntry {
                                    id: stem,
                                    name: clean_name,
                                    path: path.clone(),
                                    file_name,
                                    is_builtin,
                                    file_size_bytes: size,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Sort: built-in presets first in standard order, then user custom files alphabetically
        found.sort_by(|a, b| {
            match (a.is_builtin, b.is_builtin) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        self.hdris = found;
        if self.active_hdri_index >= self.hdris.len() {
            self.active_hdri_index = 0;
        }
    }

    /// Convert file stems into clean presentation titles
    pub fn format_display_name(stem: &str) -> String {
        let replaced = stem.replace('_', " ").replace('-', " ");
        let mut words: Vec<String> = Vec::new();
        for word in replaced.split_whitespace() {
            let mut c = word.chars();
            match c.next() {
                None => {}
                Some(f) => {
                    let capitalized = f.to_uppercase().collect::<String>() + c.as_str();
                    words.push(capitalized);
                }
            }
        }
        if words.is_empty() {
            stem.to_string()
        } else {
            words.join(" ")
        }
    }

    /// Open HDRI asset folder in Linux Desktop file manager
    pub fn open_hdri_folder(&self) -> Result<(), String> {
        let path = &self.paths.hdri_dir;
        if !path.exists() {
            let _ = fs::create_dir_all(path);
        }

        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(path)
                .spawn()
                .map_err(|e| format!("Failed to open folder via xdg-open: {}", e))?;
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err("Unsupported OS for folder open".to_string())
        }
    }

    /// Copy a user-selected external HDRI file into the assets folder and rescan
    pub fn import_hdri_file(&mut self, src: &Path) -> Result<HdriAssetEntry, String> {
        if !src.exists() {
            return Err("Source file does not exist".to_string());
        }

        let file_name = src.file_name().ok_or_else(|| "Invalid file name".to_string())?;
        let dest = self.paths.hdri_dir.join(file_name);
        fs::copy(src, &dest).map_err(|e| format!("Failed to copy file to assets/hdris: {}", e))?;
        self.refresh_hdris();

        let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let entry = self.hdris.iter().find(|h| h.id == stem).cloned().ok_or_else(|| "Failed to find imported HDRI".to_string())?;

        Ok(entry)
    }

    pub fn active_entry(&self) -> Option<&HdriAssetEntry> {
        self.hdris.get(self.active_hdri_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_display_name() {
        assert_eq!(AssetManager::format_display_name("studio_softboxes"), "Studio Softboxes");
        assert_eq!(AssetManager::format_display_name("kloofendal-48d-partly-cloudy"), "Kloofendal 48d Partly Cloudy");
    }

    #[test]
    fn test_asset_paths_init() {
        let manager = AssetManager::new();
        assert!(manager.paths.hdri_dir.exists());
        assert!(!manager.hdris.is_empty());
        let studio = manager.hdris.iter().find(|h| h.file_name == "Studio_Softboxes.hdr");
        assert!(studio.is_some(), "Built-in Studio_Softboxes.hdr must be written to disk");
        assert!(manager.active_entry().is_some());
        assert!(!manager.hdris[0].size_formatted().is_empty());
    }
}
